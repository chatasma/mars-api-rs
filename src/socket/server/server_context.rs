use std::sync::Arc;

use futures::SinkExt;
use serde::{Serialize, Deserialize};
use rocket::serde::json::serde_json;
use tokio::net::TcpStream;
use tokio_tungstenite::{WebSocketStream, tungstenite::Message};

use crate::{database::models::r#match::Match, socket::event_type::EventType, util::string::deflate_string, MarsAPIState};

pub struct ServerContext {
    pub id: String,
    pub api_state: Arc<MarsAPIState>,
    pub stream: WebSocketStream<TcpStream>
}

impl ServerContext {
    pub async fn set_current_match_id(&self, match_id: &String) {
        self.api_state.redis.set(&self.get_current_match_id_key(), match_id).await;
    }

    pub async fn set_last_time_alive(&self, time: u64) {
        self.api_state.redis.set(&self.get_last_alive_time_key(), &time).await;
    }

    pub async fn get_current_match_id(&self) -> Option<String> {
        self.api_state.redis.get(&self.get_current_match_id_key()).await.ok()
    }

    pub async fn get_match(&self) -> Option<Match> {
        self.api_state.redis.get(&format!("match:{}", self.get_current_match_id().await.unwrap_or_else(|| "null".to_owned()))).await.ok()
    }

    pub async fn call<T: Serialize>(&mut self, event_type: &EventType, data: T) {
        let packet = Packet { event: event_type.clone(), data };
        let body = serde_json::to_string(&packet).unwrap();
        debug!("Sending {} to {}", &body, &self.id);
        let binary = Message::Binary(deflate_string(body.as_bytes()).unwrap());
        let _ = self.stream.send(binary).await;
    }

    fn get_current_match_id_key(&self) -> String {
        format!("server:{}:current_match_id", self.id)
    }

    fn get_last_alive_time_key(&self) -> String {
        format!("server:{}:last_alive_time", self.id)
    }
}

#[derive(Serialize, Deserialize)]
struct Packet<T> {
    #[serde(rename = "e")]
    event: EventType,
    #[serde(rename = "d")]
    data: T
}
