use std::{str::FromStr, time::Duration};
use std::collections::HashSet;

use mars_api_rs_macro::IdentifiableDocument;
use mongodb::{options::{ClientOptions, FindOneOptions, UpdateOptions}, Client, Collection, bson::{doc, oid::ObjectId}, Cursor, results::DeleteResult};
use models::tag::Tag;
use rand::Rng;
use rocket::serde::DeserializeOwned;
use futures::StreamExt;
use serde::Serialize;
use anyhow::anyhow;
use futures::stream::FuturesUnordered;
use rocket::form::validate::Contains;

use crate::{database::models::player::Player, util::r#macro::unwrap_helper};
use crate::database::models::ip_identity::IpIdentity;
use crate::util::validation::verbose_result_ok;

use self::models::{achievement::Achievement, death::Death, level::Level, r#match::Match, punishment::Punishment, rank::Rank, session::Session};

pub mod models;
pub mod migrations;
pub mod cache;

pub trait CollectionOwner<T> {
    fn get_collection(database: &Database) -> &Collection<T>;
    fn get_collection_name() -> &'static str;
}

pub struct Database {
    pub mongo: mongodb::Database,
    pub tags: Collection<Tag>,
    pub achievements: Collection<Achievement>,
    pub players: Collection<Player>,
    pub sessions: Collection<Session>,
    pub punishments: Collection<Punishment>,
    pub ranks: Collection<Rank>,
    pub matches: Collection<Match>,
    pub deaths: Collection<Death>,
    pub levels: Collection<Level>,
    pub ip_identities: Collection<IpIdentity>
}

impl Database {
    pub async fn consume_cursor_into_owning_vec_option<T: DeserializeOwned + Unpin + Send + Sync>(cursor: Option<Cursor<T>>) 
        -> Vec<T> {
            match cursor {
                Some(cursor) => Database::consume_cursor_into_owning_vec(cursor).await,
                None => Vec::new()
            }
    }

    pub async fn consume_cursor_into_owning_vec<T: DeserializeOwned + Unpin + Send + Sync>(cursor: Cursor<T>) 
        -> Vec<T> {
        cursor.collect::<Vec<_>>().await.into_iter().filter_map(
            |result| verbose_result_ok(
                String::from("Deserialization error"), result
            )
        ).collect()
    }

    pub async fn get_all_documents<T>(&self) -> Vec<T> 
        where T: DeserializeOwned + Serialize + IdentifiableDocument + CollectionOwner<T> + Unpin + Send + Sync {
        // Self::consume_cursor_into_owning_vec_option(T::get_collection(&self).find(doc! {}, None).await.ok()).await
        let cursor = match T::get_collection(&self).find(None, None).await {
            Ok(cursor) => cursor,
            Err(e) => {
                warn!("Error retrieving documents from '{}': {}", T::get_collection_name(), e);
                return Vec::new();
            }
        };
        Self::consume_cursor_into_owning_vec_option(Some(cursor)).await
    }

    pub async fn find_by_id_or_name<T>(&self, text: &str) -> Option<T>
        where T: DeserializeOwned + Serialize + IdentifiableDocument + CollectionOwner<T> + Unpin + Send + Sync {
            T::get_collection(&self).find_one(doc! {"$or": [{"nameLower": text.to_lowercase() }, {"_id": &text }]}, None).await.ok().unwrap_or(None)
    }

    pub async fn delete_by_id<T>(&self, id: &str) -> Option<DeleteResult> where T: DeserializeOwned + Serialize + IdentifiableDocument + CollectionOwner<T> {
        let response = T::get_collection(&self).delete_one(doc! {"_id": id}, None).await;
        if let Ok(delete_result) = response {
            Some(delete_result)
        } else {
            None
        }
    }

    pub fn get_object_id_from_str(id: &str) -> Option<ObjectId> {
        let object_id = ObjectId::from_str(id);
        if let Err(_) = object_id { 
            return None;
        };
        return Some(object_id.unwrap());
    }

    pub async fn find_by_id<T: DeserializeOwned + Unpin + Send + Sync>(coll: &Collection<T>, id: &str) -> Option<T> {
        // let object_id = if let Some(object_id) = Database::get_object_id_from_str(id) { object_id } else { return None };
        let opts = FindOneOptions::builder().show_record_id(true).build();
        match coll.find_one(doc! { "_id": id }, opts).await {
            Ok(possible_doc) => possible_doc,
            Err(_) => None
        }
    }

    pub async fn ensure_player_name_uniqueness(&self, name: &String, keep_id: &String) {
        let num: u16;
        {
            let mut rng = rand::thread_rng();
            num = rng.gen_range(0..=1000);
        }
        let temp_name = format!(">WZPlayer{}", num);
        let _ = self.players.update_many(doc! {
            "$and": [{"nameLower": name.to_lowercase()}, {"$not": {"_id": &keep_id}}]
        }, doc! {
            "$set": {"name": &temp_name, "nameLower": &temp_name}
        }, None).await;
    }

    pub async fn get_active_player_session(&self, player: &Player) -> Option<Session> {
        match self.sessions.find_one(doc! { "endedAt": null, "player.id": player.id.to_owned() }, None).await {
            Ok(possible_doc) => possible_doc,
            _ => None
        }
    }

    pub async fn get_player_punishments(&self, player: &Player) -> Vec<Punishment> {
        if let Ok(punishments_cursor) = self.punishments.find(doc! { "target.id": player.id.to_owned() }, None).await {
            let mut puns : Vec<Punishment> = vec![];
            for pun_result in punishments_cursor.collect::<Vec<Result<_, _>>>().await.into_iter() {
                if let Ok(pun) = pun_result { 
                    puns.push(pun);
                };
            };
            puns
        } else { vec![] }
    }


    pub async fn get_active_player_punishments(&self, player: &Player) -> Vec<Punishment> {
        let mut puns : Vec<Punishment> = self.get_player_punishments(player).await;
        puns.retain(|p| p.is_active());
        puns.sort_by(|p1, p2| {
            p1.issued_at.partial_cmp(&(p2.issued_at)).unwrap_or(std::cmp::Ordering::Equal)
        });
        puns
    }

    pub async fn find_session_for_player(&self, player: &Player, id: String) -> Option<Session> {
        match self.sessions.find_one(doc! { "_id": id, "player.id": player.id.clone() }, None).await {
            Ok(sesh_opt) => sesh_opt,
            Err(_) => None,
        }
    }

    pub async fn get_alts_for_player(&self, player: &Player) -> Vec<Player> {
        let unordered_futures = FuturesUnordered::new();
        for ip in &player.ips {
            unordered_futures.push(IpIdentity::find_players_for_ip(self, ip));
        }
        let players = {
            let players_with_duplicates = unordered_futures.collect::<Vec<_>>().await
                .into_iter().flatten().collect::<Vec<_>>();
            let mut player_set : Vec<Player> = Vec::new();
            for player in players_with_duplicates {
                if !player_set.iter().map(|p| p.id.clone()).collect::<Vec<_>>().contains(&player.id) {
                    player_set.push(player);
                }
            }
            player_set
        };
        players
        // let cursor = unwrap_helper::result_return_default!(self.players.find(doc! {
        //     "ips": {"$in": &player.ips}, "_id": {"$ne": &player.id}
        // }, None).await, Vec::new());
        // Database::consume_cursor_into_owning_vec(cursor).await
    }

    pub async fn save<R>(&self, record: &R) where R: CollectionOwner<R> + Serialize + IdentifiableDocument {
        let collection = R::get_collection(&self);
        let bson = mongodb::bson::to_bson(record).unwrap();
        let serialized = bson.as_document().unwrap();
        let update_opts = UpdateOptions::builder().upsert(Some(true)).build();
        let _ = collection.update_one(doc! {
            "_id": record.get_id_value()
        }, doc! { "$set": serialized }, Some(update_opts)).await;
    }

    pub async fn insert_one<R>(&self, record: &R) where R: CollectionOwner<R> + Serialize + IdentifiableDocument {
        let collection = R::get_collection(&self);
        // let bson = mongodb::bson::to_bson(record).unwrap();
        // let serialized = bson.as_document().unwrap().clone();
        // let update_opts = UpdateOptions::builder().upsert(Some(true)).build();
        // let doc = doc! {};
        let _ = collection.insert_one(record, None).await;
        // let _ = collection.update_one(doc! {
        //     "_id": record.get_id_value()
        // }, doc! { "$set": serialized }, Some(update_opts)).await;
    }

    pub async fn find_by_name<R>(&self, name: &str) -> Option<R>
        where R: CollectionOwner<R> + Serialize + IdentifiableDocument + DeserializeOwned + Unpin + Send + Sync {
        R::get_collection(&self).find_one(doc! { "nameLower": name.to_lowercase() }, None).await.unwrap_or(None)
    }
}

const DB_NAME: &'static str = "mars-api";

pub async fn ping_database(mongo: &mongodb::Database) -> bool {
    mongo.run_command(doc! { "ping": 1 }, None).await.is_ok()
}

pub async fn connect(db_url: &String, min_pool_size: Option<u32>, max_pool_size: Option<u32>) -> anyhow::Result<Database> {
    let mut client_options = ClientOptions::parse(db_url).await?;
    client_options.min_pool_size = min_pool_size;
    client_options.max_pool_size = max_pool_size;
    client_options.connect_timeout = Some(Duration::new(5, 0));
    client_options.server_selection_timeout = Some(Duration::new(5, 0));


    let client = Client::with_options(client_options)?;
    let db = client.database(DB_NAME);
    if !ping_database(&db).await {
        return Err(anyhow!("Could not connect to the database. Is it running?"));
    };

    let tags = db.collection::<Tag>(Tag::get_collection_name());
    let achievements = db.collection::<Achievement>(Achievement::get_collection_name());
    let players = db.collection::<Player>(Player::get_collection_name());
    let sessions = db.collection::<Session>(Session::get_collection_name());
    let punishments = db.collection::<Punishment>(Punishment::get_collection_name());
    let ranks = db.collection::<Rank>(Rank::get_collection_name());
    let matches = db.collection::<Match>(Match::get_collection_name());
    let levels = db.collection::<Level>(Level::get_collection_name());
    let deaths = db.collection::<Death>(Death::get_collection_name());
    let ip_identities = db.collection::<IpIdentity>(IpIdentity::get_collection_name());

    info!("Connected to database successfully.");
    Ok(Database { 
        mongo: db, tags, achievements, players, sessions, 
        punishments, ranks, matches, levels, deaths, ip_identities
    })
}
