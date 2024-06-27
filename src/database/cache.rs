use std::{marker::PhantomData, time::Duration, sync::Arc};

use futures::Future;
use mars_api_rs_macro::IdentifiableDocument;
use mobc::{Pool, Manager};
use mongodb::bson::doc;
use redis::{aio::Connection, Client, AsyncCommands, RedisResult};
use rocket::serde::json;
use serde::{Serialize, de::DeserializeOwned};
use anyhow::anyhow;

use crate::{config::ConfigMissingFieldError, util::r#macro::unwrap_helper};

use super::{Database, CollectionOwner};

pub struct Cache<R> {
    pub redis: Arc<RedisAdapter>,
    pub resource_name: String,
    pub lifetime_ms: u64,
    pub resource_type: PhantomData<R>
}

impl<R> Cache<R> where R: CollectionOwner<R> + DeserializeOwned + Unpin + Sync + std::marker::Send
    + Serialize + IdentifiableDocument {
    fn generate_formatted_key(&self, key: &str) -> String {
        format!("{}:{}", self.resource_name, key.to_lowercase())
    }

    pub async fn query(&self, key: &str) -> Option<R> {
        let resource_key = self.generate_formatted_key(key);
        if let Ok(value) = self.redis.get(&resource_key).await { Some(value) } else { None }
    }

    pub async fn get(&self, database: &Database, key: &str) -> Option<R> {
        if let Some(datum) = self.query(key).await { Some(datum) } 
        else {
            let collection = R::get_collection(database);
            let query = doc! {
                "$or": [
                    { "_id": key },
                    { "nameLower": key.to_lowercase() }
                ]
            };
            let result = collection.find_one(query.clone(), None).await;
            match result {
                Ok(opt) => { opt
                }
                Err(e) => {
                    warn!("Encountered error performing query {}", query);
                    warn!("Error: {}", e);
                    None
                }
            }
        }
    }

    pub async fn set(&self, database: &Database, key: &str, value: &R, persist: bool) {
        self.set_with_expiry(database, key, value, persist, None).await;
    }

    pub async fn set_with_expiry(&self, database: &Database, key: &str, value: &R, persist: bool, expiry_ms: Option<usize>) {
        let resource_key = self.generate_formatted_key(key);
        if persist {
            database.save(value).await;
        }
        self.redis.set_with_expiry(&resource_key, value, expiry_ms).await;
    }

    pub async fn persist_cached_value(&self, database: &Database, key: &String) {
        if let Some(record) = self.query(key).await {
            database.save(&record).await;
        }
    }
}

// begin: mobc manager wrapper
pub struct RedisConnectionManager {
    client: Client,
}

impl RedisConnectionManager {
    pub fn new(c: Client) -> Self {
        Self { client: c }
    }
}

#[async_trait]
impl Manager for RedisConnectionManager {
    type Connection = Connection;
    type Error = redis::RedisError;

    async fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let c = self.client.get_async_connection().await?;
        Ok(c)
    }

    async fn check(&self, mut conn: Self::Connection) -> Result<Self::Connection, Self::Error> {
        redis::cmd("PING").query_async(&mut conn).await?;
        Ok(conn)
    }
}
// end: mobc manager wrapper

const CACHE_POOL_MAX_OPEN: u64 = 16; // max connections
const CACHE_POOL_MAX_IDLE: u64 = 8; // max unused connections
const CACHE_POOL_TIMEOUT_SECONDS: u64 = 1; // await a connection from pool @ 1 second max
const CACHE_POOL_EXPIRE_SECONDS: u64 = 60; // inactive connections die after a minute

pub async fn get_redis_pool(redis_host: &Option<String>) -> anyhow::Result<RedisAdapter> {
    match redis_host {
        None => Err(ConfigMissingFieldError {field_name: String::from("redis-host") }.into()),
        Some(redis_host) => {
            let redis_uri = format!("redis://{}", redis_host);
            info!("Connecting to redis at {}", &redis_uri);
            let client = redis::Client::open(redis_uri)?;
            let manager = RedisConnectionManager::new(client);
            let pool = Pool::builder()
                .get_timeout(Some(Duration::from_secs(CACHE_POOL_TIMEOUT_SECONDS)))
                .max_open(CACHE_POOL_MAX_OPEN)
                .max_idle(CACHE_POOL_MAX_IDLE)
                .max_lifetime(Some(Duration::from_secs(CACHE_POOL_EXPIRE_SECONDS)))
                .build(manager);
            let redis_adapter = RedisAdapter { pool };
            if !redis_adapter.ping().await {
                return Err(anyhow!("Could not connect to Redis. Is it running?"));
            };
            info!("Connected to redis successfully.");
            Ok(redis_adapter)
        }
    }
}

pub struct RedisAdapter {
    pub pool: Pool<RedisConnectionManager>
}

impl RedisAdapter {
    pub async fn ping(&self) -> bool {
        let mut conn = unwrap_helper::result_return_default!(self.pool.get().await, false);
        redis::cmd("PING").arg("we love warzone").query_async::<Connection, String>(&mut conn).await.is_ok()
    }

    pub async fn set<T>(&self, key: &str, value: &T) where T: Serialize {
        self.set_with_expiry(key, value, None).await
    }

    pub async fn set_with_expiry<T>(&self, key: &str, value: &T, expiry_ms: Option<usize>) where T: Serialize {
        let mut conn = match self.pool.get().await {
            Ok(conn) => conn,
            Err(_) => return
        };
        match json::to_string(value) {
            Ok(stringified) => { 
                if expiry_ms.is_some() {
                    let _ : RedisResult<String> = conn.pset_ex(&key, &stringified, expiry_ms.unwrap()).await;
                } else {
                    let _ = redis::cmd("SET").arg(key).arg(&stringified).query_async::<Connection, String>(&mut conn).await; 
                };
            },
            _ => {}
        };
    }

    pub async fn get_unchecked<T>(&self, key: &str) -> Option<T> where T: DeserializeOwned {
        match self.get(key).await {
            Ok(val) => Some(val),
            Err(_) => None
        }
    }

    pub async fn get<T>(&self, key: &str) -> anyhow::Result<T> where T: DeserializeOwned {
        let mut conn = self.pool.get().await?;
        let raw : String = redis::cmd("GET").arg(key).query_async::<Connection, String>(&mut conn).await?;
        Ok(json::from_str::<T>(&raw)?)
    }

    pub async fn del_key(&self, key: &str) -> anyhow::Result<bool> {
        let mut conn = self.pool.get().await?;
        let deleted = redis::cmd("DEL").arg(key).query_async::<Connection, u64>(&mut conn).await?;
        Ok(deleted > 0)
    }

    pub async fn has_key(&self, key: &str) -> anyhow::Result<bool> {
        let mut conn = self.pool.get().await?;
        let exists_i = redis::cmd("EXISTS").arg(key).query_async::<Connection, u64>(&mut conn).await?;
        Ok(exists_i == 1)
    }

    pub async fn submit<T, O: Future<Output = T>, F: FnOnce(mobc::Connection<RedisConnectionManager>) -> O>(&self, task: F) -> anyhow::Result<T> {
        let conn : mobc::Connection<RedisConnectionManager> = self.pool.get().await?;
        Ok(task(conn).await)
    }
}
