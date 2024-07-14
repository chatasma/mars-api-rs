use std::collections::{HashSet, HashMap};

use mars_api_rs_derive::IdentifiableDocument;
use mars_api_rs_macro::IdentifiableDocument;
use serde::{Serialize, Deserialize};

use crate::{database::CollectionOwner, util::time::get_u64_time_millis, socket::{participant::participant_context::PlayerMatchResult, r#match::match_events::MatchEndData}};

use super::{player::SimplePlayer, level::{Level, LevelGamemode}, participant::{Participant}};

#[derive(Serialize, Deserialize, IdentifiableDocument)]
#[serde(rename_all = "camelCase")]
pub struct Match {
    #[id]
    #[serde(rename = "_id")]
    pub id: String,
    pub loaded_at: u64,
    #[serde(default)]
    pub started_at: Option<u64>,
    #[serde(default)]
    pub ended_at: Option<u64>,
    pub level: Level,
    pub parties: HashMap<String, Party>,
    pub participants: HashMap<String, Participant>,
    pub server_id: String,
    pub first_blood: Option<FirstBlood>
}

impl Match {
    pub fn is_tracking_stats(&self) -> bool {
        !self.level.gamemodes.contains(&LevelGamemode::Arcade)
    }

    pub fn get_state(&self) -> MatchState {
        if let None = self.started_at {
            MatchState::Pre
        } else if let None = self.ended_at {
            MatchState::InProgress
        } else {
            MatchState::Post
        }
    }

    pub fn save_participants(&mut self, participants: Vec<Participant>) {
        for participant in participants {
            self.participants.insert(participant.id.clone(), participant);
        }
    }

    pub fn get_length(&self) -> u64 {
        let start = self.started_at.unwrap_or(0);
        let end = self.ended_at.unwrap_or(get_u64_time_millis());
        end - start
    }

    pub fn get_participant_match_result(&self, participant: &Participant, end: &MatchEndData) -> PlayerMatchResult {
        let is_playing = participant.party_name.is_some();
        if !is_playing {
            return PlayerMatchResult::Intermediate;
        };

        if end.is_tie(&self) {
            PlayerMatchResult::Tie
        } else if end.winning_parties.contains(participant.party_name.as_ref().unwrap_or(&String::new()))  {
            PlayerMatchResult::Win
        } else {
            PlayerMatchResult::Lose
        }
    }

    pub fn get_participant(&self, id: &String) -> &Participant {
        self.participants.get(id).unwrap()
    }
}

impl CollectionOwner<Match> for Match {
    fn get_collection(database: &crate::database::Database) -> &mongodb::Collection<Match> {
        &database.matches
    }

    fn get_collection_name() -> &'static str {
        "match"
    }
}

#[derive(PartialEq)]
pub enum MatchState {
    Pre,
    InProgress,
    Post
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FirstBlood {
    pub attacker: SimplePlayer,
    pub victim: SimplePlayer,
    pub date: u64
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Party {
    pub name: String,
    pub alias: String,
    pub color: String,
    pub min: u32,
    pub max: u32
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoalCollection {
    pub cores: Vec<CoreGoal>,
    pub destroyables: Vec<DestroyableGoal>,
    pub flags: Vec<FlagGoal>,
    pub wools: Vec<WoolGoal>,
    pub control_points: Vec<ControlPointGoal>
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreGoal {
    pub id: String,
    pub name: String,
    pub owner_name: Option<String>,
    pub material: String,
    #[serde(default = "default_simpleplayer_set")]
    pub contributors: HashSet<SimplePlayer>
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DestroyableGoal {
    pub id: String,
    pub name: String,
    pub owner_name: Option<String>,
    pub material: String,
    pub block_count: u32,
    pub breaks_required: u32,
    #[serde(default = "default_simpleplayer_set")]
    pub contributors: HashSet<SimplePlayer>
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlagGoal {
    pub id: String,
    pub name: String,
    pub owner_name: Option<String>,
    pub color: String
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WoolGoal {
    pub id: String,
    pub name: String,
    pub owner_name: Option<String>,
    pub color: String
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlPointGoal {
    pub id: String,
    pub name: String
}

fn default_simpleplayer_set() -> HashSet<SimplePlayer> { HashSet::new() }
