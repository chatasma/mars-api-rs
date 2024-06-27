use mars_api_rs_derive::IdentifiableDocument;
use mongodb::{bson, Collection};
use serde::{Deserialize, Serialize};
use crate::database::{CollectionOwner, Database};
use crate::socket::leaderboard::ScoreType;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaderboardEntry {
    pub player_id: String,
    pub timestamp: bson::DateTime,
    pub score_type: ScoreType,
    pub value: u32,
}

impl LeaderboardEntry {
    pub fn get_timestamp_field() -> String {
        String::from("timestamp")
    }
}

impl CollectionOwner<LeaderboardEntry> for LeaderboardEntry {
    fn get_collection(database: &Database) -> &Collection<LeaderboardEntry> {
        &database.lb_entries
    }

    fn get_collection_name() -> &'static str {
        "lb_entry"
    }
}