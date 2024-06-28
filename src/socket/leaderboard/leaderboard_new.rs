use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use chrono::{DateTime, FixedOffset, NaiveDate};
use futures::future::join_all;
use futures::{StreamExt, TryFutureExt, TryStreamExt};
use futures::stream::FuturesUnordered;

use mongodb::{bson, ClientSession};
use mongodb::bson::{doc, Document};
use mongodb::error::Error;
use mongodb::options::FindOptions;
use redis::aio::Connection;
use strum::IntoEnumIterator;
use tokio::sync::{RwLock, RwLockReadGuard, TryLockError};

use crate::database::cache::RedisAdapter;
use crate::database::Database;
use crate::database::models::leaderboard_entry::LeaderboardEntry;
use crate::MarsAPIState;
use crate::socket::leaderboard::{get_lb_datetime, LeaderboardLine, LeaderboardPeriod, LeaderboardPeriodDateTimeRange, ScoreType, ScoreTypeAggregation};
use crate::util::r#macro::unwrap_helper;
use crate::util::time::get_u64_time_millis;
use crate::util::validation::verbose_result_ok;

struct LeaderboardViewMetadata {
    pub last_updated: Option<DateTime<FixedOffset>>,
    pub lock: RwLock<()>
}

impl Default for LeaderboardViewMetadata {
    fn default() -> Self {
        LeaderboardViewMetadata {
            last_updated: None,
            lock: RwLock::new(())
        }
    }
}

pub enum LeaderboardFetchError {
    UpdateInProgress,
    DocumentStreamError
}

pub struct LeaderboardV2 {
    pub score_type: ScoreType,
    pub database: Arc<Database>,
    pub cache: Arc<RedisAdapter>,
    lb_metadata: Arc<RwLock<HashMap<LeaderboardPeriod, LeaderboardViewMetadata>>>
}

impl LeaderboardV2 {
    pub fn new(score_type: ScoreType, redis: Arc<RedisAdapter>, database: Arc<Database>) -> Self {
        // this would require a bit more effort
        // (would need to timestamp our socket requests for more accurate times, as updates as
        // process asynchronously the relative order is not guaranteed in any way)
        if score_type.get_aggregation_type().requires_sequential_consistency() {
            panic!("LeaderboardV2 does not yet support score types which require sequential consistency");
        }
        LeaderboardV2 {
            score_type,
            database,
            cache: redis,
            lb_metadata: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn process_update(&self, player_id: String, value: u32) {
        // ignore 0 deltas
        if self.score_type.get_aggregation_type().is_delta_useless(value) {
            return
        }

        // record time of completion
        let completion_time = get_u64_time_millis();
        let completion_datetime = bson::DateTime::from_millis(completion_time as i64);

        // find our standing
        let daily_standing = self.query_standing_cached(
            &player_id,
            &LeaderboardPeriod::get_most_granular_period()
        ).await.unwrap_or(0);
        // determine if standing is outperformed
        let requires_update = match self.score_type.get_aggregation_type() {
            // a positive non-zero delta is always going to require an update
            // for sum stats
            ScoreTypeAggregation::Sum => true,
            // for max we need to know the current standing before we can
            // decide if an update is needed
            ScoreTypeAggregation::Max => {
                value > daily_standing
            }
        };

        // i.e. when max is not outperformed
        if !requires_update {
            return
        }

        let daily_new_value = self.score_type.get_aggregation_type().compare(daily_standing, value);

        // update standing in lb_entries
        let lb_entry_update_future = async {
            // in a new database transaction:
            // - remove all documents on player_id,score_type,<timestamp within today>
            // - upsert: true on player_id,score_type,timestamp
            // Notice: Unfortuantely transactions in Mongo only work in replica set mode,
            // so just doing this sequentially for now

            // let mut session = self.database.client.start_session(None).await
            //     .expect("acquire client session");
            // session.start_transaction(None).await.expect("To be able to start a transaction");
            // self.database.lb_entries.delete_many_with_session(
            //     self.get_query(
            //         Some(player_id.clone()),
            //         LeaderboardPeriod::get_most_granular_period().get_date_range_with_now_as_upperbound()
            //     ), None, &mut session
            // ).await.expect("No errors when deleting old lb entries");
            // self.database.lb_entries.insert_one_with_session(LeaderboardEntry {
            //     player_id: player_id.clone(),
            //     timestamp: completion_datetime.clone(),
            //     score_type: self.score_type.clone(),
            //     new_value
            // }, None, &mut session).await.expect("No errors when inserting the new lb entry");
            // session.commit_transaction().await.expect("To commit the transaction");

            self.database.lb_entries.delete_many(self.get_query(
                Some(player_id.clone()),
                LeaderboardPeriod::get_most_granular_period().get_date_range_with_now_as_upperbound()
            ), None).await.expect("No errors when deleting old lb entries");
            self.database.lb_entries.insert_one(LeaderboardEntry {
                player_id: player_id.clone(),
                timestamp: completion_datetime.clone(),
                score_type: self.score_type.clone(),
                value: daily_new_value
            }, None).await.expect("No errors when inserting the new lb entry");
        };

        // acquire the writer once to ensure all cached views have a metadata entry with the
        // reader-writer lock initialized
        {
            let mut writer = self.lb_metadata.write().await;
            for period in LeaderboardPeriod::iter() {
                writer.entry(period.clone()).or_insert_with(|| Default::default());
            }
        }

        // update cached Redis leaderboard views per Period
        let cache_updates = join_all(LeaderboardPeriod::iter().map(|period| {
            // Redis update, acq the RwLock in read mode to respect presence of writers
            // i.e. when reconstructing the view an active writer will be present that
            // needs to be respected
            async {
                let lb_metadata = self.lb_metadata.clone();
                let period = period;
                let reader = lb_metadata.read().await;
                let metadata = reader.get(&period).unwrap();
                let lock_borrow = metadata.lock.read().await;

                let new_value = if period == LeaderboardPeriod::get_most_granular_period() {
                    daily_new_value
                } else {
                    let current_period_standing = self.query_standing_cached(
                        &player_id,
                        &period
                    ).await.unwrap_or(0);
                    debug!("[{}/{}] Current standing of player {}: {}",
                        self.score_type.to_string(), period.to_string(),
                        &player_id, &current_period_standing
                    );
                    let new_period_standing = self.score_type.get_aggregation_type().compare(
                        current_period_standing, value
                    );
                    new_period_standing
                };
                // basic ZADD
                debug!("[{}/{}] Setting score of player {} to {} in cached leaderboard",
                    self.score_type.to_string(), period.to_string(),
                    &player_id, new_value
                );
                self.set_for_period(&player_id, new_value, &period).await;
            }
        }).collect::<Vec<_>>());
        // todo, join these somehow -- it seems to be a bit trickier than a join_all...
        cache_updates.await;
        lb_entry_update_future.await;
    }

    // async fn query_standing(&self, player_id: String, period: &LeaderboardPeriod) -> Option<u32> {
    //     // query `lb_entries` on player_id, score_type, time_range as deduced by lb period
    //     // get 1 document representing the current u32 standing
    //     let query = self.get_query(Some(player_id.clone()), period.get_date_range_with_now_as_upperbound());
    //     debug!("[{}/{}] Standing query for {}: {}", self.score_type.to_string(),
    //         period.to_string(),
    //         &player_id, query.to_string()
    //     );
    //     verbose_result_ok(
    //         format!("Failed to retrieve standing for {}", player_id.clone()),
    //         self.database.lb_entries.find_one(query, None).await
    //     ).flatten().map(|lbe| lbe.value)
    // }

    pub async fn query_standing_cached(&self, player_id: &String, period: &LeaderboardPeriod) -> Option<u32> {
        self.cache.submit(|mut conn| async move {
            match redis::cmd("ZSCORE").arg(&self.get_view_id(&period)).arg(player_id).query_async::<Connection, String>(&mut conn).await {
                Ok(res) => { res.parse::<u32>().unwrap() },
                Err(_) => { 0u32 }
            }
        }).await.ok()
    }

    fn get_query(&self, player_id: Option<String>, time_range: LeaderboardPeriodDateTimeRange) -> Document {
        let (start_time_opt, end_time) = time_range;
        let end_time_bson = bson::DateTime::from_millis(end_time.timestamp_millis());

        let mut query_document = Document::new();
        if let Some(player_id) = player_id {
            query_document.insert("playerId", player_id);
        }
        query_document.insert("scoreType", self.score_type.to_string());
        let mut timestamp_range_query = Document::new();
        timestamp_range_query.insert("$lt", end_time_bson);
        if let Some(begin) = time_range.0 {
            let start_time_bson =
                bson::DateTime::from_millis(begin.timestamp_millis());
            timestamp_range_query.insert("$gte", start_time_bson);
        }
        query_document.insert("timestamp", timestamp_range_query);
        query_document
    }

    pub async fn set(&self, id: &String, score: u32) {
        let u64_score = score as u64;
        let _ = self.cache.submit(|mut conn| async move {
            for period in LeaderboardPeriod::iter() {
                let _ = redis::cmd("ZADD").arg(&self.get_view_id(&period)).arg(u64_score).arg(id).query_async::<Connection, ()>(&mut conn).await;
            };
        }).await;
    }

    pub async fn set_for_period(&self, id: &String, score: u32, period: &LeaderboardPeriod) {
        let u64_score = score as u64;
        let _ = self.cache.submit(|mut conn| async move {
            let _ = redis::cmd("ZADD").arg(&self.get_view_id(period)).arg(u64_score).arg(id)
                .query_async::<Connection, ()>(&mut conn).await;
        }).await;
    }

    fn strings_as_leaderboard_entries(raw: Vec<String>) -> Vec<LeaderboardLine> {
        let mut entries : Vec<LeaderboardLine> = Vec::new();
        if raw.len() <= 1 || raw.len() % 2 == 1 {
            return entries;
        };
        for i in (0..=(raw.len() - 2)).step_by(2) {
            let id_name = raw[i].clone();
            let score = raw[i + 1].parse::<u32>().unwrap_or(0);
            let (id, name) = {
                let mut parts = id_name.split("/");
                let id = unwrap_helper::continue_default!(parts.next());
                let name = unwrap_helper::continue_default!(parts.next());
                (id, name)
            };
            entries.push(LeaderboardLine { id: id.to_owned(), name: name.to_owned(), score });
        }
        entries
    }

    pub async fn fetch_top(&self, period: &LeaderboardPeriod, limit: u32) -> Result<Vec<LeaderboardLine>, LeaderboardFetchError> {
        let view_id = self.get_view_id(period);
        // does the cache need to be updated?
        // update if we have entered a new period or never constructed before
        let cache_needs_updating = {
            let reader = self.lb_metadata.read().await;
            match reader.get(period) {
                Some(lvm) => {
                    match lvm.last_updated {
                        None => true,
                        Some(last_update_date) => {
                            let disjoint = period.are_datetimes_disjoint(last_update_date.to_owned(), get_lb_datetime());
                            if disjoint {
                                true
                            } else {
                                // confirm key is in cache
                                let key_exists = self.cache.has_key(view_id.as_str()).await
                                    .unwrap_or(false);
                                if !key_exists {
                                    debug!("Key {} did not exist, cache needs to be created", &view_id);
                                }
                                !key_exists
                            }
                        }
                    }
                }
                None => true
            }
        };

        if cache_needs_updating {
            info!("Populating leaderboard {}/{}...", self.score_type.to_string(), period.to_string());
            // we need writers on:
            // - metadata map to update last updated time
            // - period metadata to block readers from querying while the cache is reconstructed
            debug!("[{}/{}] Acquiring leaderboard metadata writer", self.score_type.to_string(), period.to_string());
            let mut writer_lb_metadata = self.lb_metadata.write().await;
            debug!("[{}/{}] Acquired leaderboard metadata writer", self.score_type.to_string(), period.to_string());
            let metadata = writer_lb_metadata.entry(period.clone()).or_insert(Default::default());
            debug!("[{}/{}] Acquiring period view writer", self.score_type.to_string(), period.to_string());
            let writer_period_view = metadata.lock.write().await;
            debug!("[{}/{}] Acquired period view writer", self.score_type.to_string(), period.to_string());
            debug!("[{}/{}] Deleting cached leaderboard", self.score_type.to_string(), period.to_string());
            let t1 = get_u64_time_millis();
            let deleted = self.cache.del_key(self.get_view_id(period).as_str()).await.expect("To delete the cached leaderboard view successfully");
            debug!("[{}/{}] Was cached leaderboard deleted?: {}", self.score_type.to_string(), period.to_string(), deleted);

            let time = get_lb_datetime();
            let target_query = self.get_query(None, period.get_full_date_range(time));

            // load from DB
            let mut find_options = FindOptions::default();
            // batch-read into memory 50k records at a time
            find_options.batch_size = Some(50_000);
            debug!("[{}/{}] Getting all leaderboard entries for this time frame, query: {}", self.score_type.to_string(), period.to_string(), target_query.to_string());
            let mut entries =
                self.database.lb_entries.find(
                    target_query, Some(find_options)
                ).await.expect("Could not acq cursor for lb entries query");
            let mut standings: HashMap<String, u32> = HashMap::new();

            debug!("[{}/{}] Constructing standings", self.score_type.to_string(), period.to_string());
            while let entry_result = entries.try_next().await {
                match entry_result {
                    Ok(entry_optional) => {
                        match entry_optional {
                            Some(entry) => {
                                let new_value = {
                                    let current = standings.get(&entry.player_id);
                                    match current {
                                        None => {
                                            standings.insert(entry.player_id.clone(), entry.value);
                                            entry.value
                                        }
                                        Some(standing) => {
                                            self.score_type.get_aggregation_type().compare(
                                                standing.to_owned(),
                                                entry.value
                                            )
                                        }
                                    }
                                };
                                debug!("[{}/{}] Populated entry {} to {}",
                                    self.score_type.to_string(), period.to_string(),
                                    &entry.player_id, new_value
                                );
                                standings.insert(entry.player_id.clone(), new_value);
                            }
                            None => {
                                break
                            }
                        }
                    }
                    Err(e) => {
                        return Err(LeaderboardFetchError::DocumentStreamError)
                    }
                };
            }
            debug!("[{}/{}] Constructed standings", self.score_type.to_string(), period.to_string());
            debug!("[{}/{}] Populating standings", self.score_type.to_string(), period.to_string());
            // Redis tasks
            let tasks = standings.iter().map(|(player, record)| {
                self.set_for_period(player, record.clone(), period)
            }).collect::<Vec<_>>();
            join_all(tasks).await;
            debug!("[{}/{}] Populated standings", self.score_type.to_string(), period.to_string());
            metadata.last_updated = Some(time);
            let t2 = get_u64_time_millis();
            info!("Load on {}/{} took {:.2} seconds", self.score_type.to_string(), period.to_string(), (((t2 - t1) as f64)/1000.0f64));
        }

        if self.lb_metadata.try_read().is_err() {
            return Err(LeaderboardFetchError::UpdateInProgress)
        };

        // ZRANGE to get standings
        let lb_top = self.cache.submit(|mut conn| async move {
            let top: Option<Vec<String>> = match redis::cmd("ZRANGE").arg(view_id).arg(0u32).arg(limit - 1).arg("REV").arg("WITHSCORES").query_async::<Connection, Vec<String>>(&mut conn).await {
                Ok(res) => Some(res),
                Err(_) => None
            };
            top.unwrap_or(Vec::new())
        }).await.unwrap_or(Vec::new());
        Ok(Self::strings_as_leaderboard_entries(lb_top))
    }

    fn get_view_id(&self, period: &LeaderboardPeriod) -> String {
        format!("lb_view:{}:{}", self.score_type, period.to_string())
    }
}
