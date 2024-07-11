use futures::stream::FuturesUnordered;
use futures::StreamExt;
use mars_api_rs_derive::IdentifiableDocument;
use mars_api_rs_macro::IdentifiableDocument;
use mongodb::Collection;
use rocket::serde::{Deserialize, Serialize};
use crate::database::{CollectionOwner, Database};
use crate::database::models::player::Player;

#[derive(Deserialize, Serialize, IdentifiableDocument, Clone)]
pub struct IpIdentity {
    #[id]
    #[serde(rename = "_id")]
    pub ip: String,
    pub players: Vec<String>
}

impl IpIdentity {
    pub async fn add_player_ip(database: &Database, ip: &String, player: &String) {
        let collection = &database.ip_identities;
        let ip_identity = match Self::get_ip_identity_by_ip(collection, ip).await {
            Some(mut ip_identity) => {
                if !ip_identity.players.contains(&player) {
                    ip_identity.players.push(player.clone());
                }
                ip_identity
            },
            None => IpIdentity { ip: ip.clone(), players: vec![player.clone()] }
        };
        database.save(&ip_identity).await;
    }

    pub async fn find_players_for_ip(database: &Database, ip: &String) -> Vec<Player> {
        let collection = &database.ip_identities;
        let record = Self::get_ip_identity_by_ip(collection, ip).await;
        match record {
            Some(ip_identity) => {
                let unordered_futures = FuturesUnordered::new();
                for player in ip_identity.players.iter() {
                    unordered_futures.push(
                        Database::find_by_id(&database.players, player)
                    );
                }
                let results : Vec<_> = unordered_futures.collect().await;
                let players : Vec<_> = results.into_iter().filter_map(|r| r).collect();
                players
            }
            None => Vec::new()
        }
    }

    pub async fn get_ip_identity_by_ip(collection: &Collection<IpIdentity>, ip: &String) -> Option<IpIdentity> {
        Database::find_by_id(collection, ip.as_str()).await
    }
}

impl CollectionOwner<IpIdentity> for IpIdentity {
    fn get_collection(database: &Database) -> &Collection<IpIdentity> {
        &database.ip_identities
    }

    fn get_collection_name() -> &'static str {
        "ip_identity"
    }
}