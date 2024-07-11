use std::collections::HashMap;
use std::ffi::c_int;
use futures::{Stream, StreamExt, TryStreamExt};
use futures::stream::FuturesUnordered;
use mongodb::bson::{doc, Document};
use mongodb::error::Error;
use mongodb::options::FindOptions;
use crate::database::Database;
use crate::database::migrations::DatabaseMigration;
use crate::database::models::ip_identity::IpIdentity;
use crate::database::models::player::Player;

pub struct DenormalizeIpIdentitiesMigration {}

#[async_trait]
impl DatabaseMigration for DenormalizeIpIdentitiesMigration {
    fn get_id(&self) -> String {
        String::from("denormalize_ip_identities")
    }

    async fn perform(&self, database: &Database) {
        let mut find_options = FindOptions::default();
        // batch-read into memory 50k records at a time
        find_options.batch_size = Some(50_000);

        let count = database.players.count_documents(doc! {}, None).await.unwrap_or(0);
        info!("{} document(s) in the player collection to migrate", count);

        let mut cursor = database.players.find(doc! {}, Some(find_options)).await.expect("find all players to succeed");
        // aggregate 25k players' worth of IPs into memory at a time before flushing
        let step_size = 25_000u32;
        let mut total_accumulated = 0u32;
        let mut accumulated = 0u32;
        let mut error_count = 0u32;
        let mut ip_map : HashMap<String, Vec<String>> = HashMap::new();
        while let Ok(more) = cursor.advance().await {
            if !more {
                break
            }
            if accumulated >= step_size {
                info!("Flushing batch of IPs to ip identities, accumulation progress: {}/{}", total_accumulated, count);
                Self::flush_ips(database, ip_map).await;
                ip_map = HashMap::new();
                accumulated = 0;
            }
            let player = match cursor.deserialize_current() {
                Ok(p) => p,
                Err(e) => {
                    let doc_result : mongodb::bson::raw::Result<Document> = cursor.current().try_into();
                    let doc = match doc_result {
                        Ok(doc) => doc,
                        Err(e) => {
                            warn!("Error to parse doc: {}", e);
                            return
                        }
                    };
                    warn!("document in question: {:?}", doc);
                    warn!("Deserialization error: {}", e);
                    error_count += 1;
                    continue
                }
            };
            for ip in &player.ips {
                match ip_map.get_mut(ip) {
                    Some(players_for_ip) => {
                        players_for_ip.push(player.id.to_owned());
                    }
                    None => {
                        ip_map.insert(ip.to_owned(), vec![player.id.to_owned()]);
                    }
                }
            }
            accumulated += 1;
            total_accumulated += 1;
        }
        info!("Flushing any remaining IPs...");
        Self::flush_ips(database, ip_map).await;
        info!("Total flushed: {}", total_accumulated);
        info!("Error count: {}", error_count);
    }
}

impl DenormalizeIpIdentitiesMigration {
    async fn flush_ips(database: &Database, ip_map : HashMap<String, Vec<String>>) {
        let unordered_futures = FuturesUnordered::new();
        for (ip, players) in ip_map.into_iter() {
            let task = async {
                let ip_identity_optional = IpIdentity::get_ip_identity_by_ip(&database.ip_identities, &ip).await;
                match ip_identity_optional {
                    Some(mut ip_identity) => {
                        ip_identity.players.extend(players);
                        database.save(&ip_identity).await;
                    }
                    None => {
                        let ip_identity = IpIdentity {
                            ip, players
                        };
                        database.save(&ip_identity).await
                    }
                }
            };
            unordered_futures.push(task);
        }
        unordered_futures.collect::<Vec<_>>().await;
    }
}