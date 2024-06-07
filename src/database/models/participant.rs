use std::collections::HashMap;

use serde::{Serialize, Deserialize};

use crate::{util::time::get_u64_time_millis, MarsAPIState, socket::{r#match::match_events::MatchEndData, participant::participant_context::PlayerMatchResult}};

use super::{player::{PlayerObjectiveStatistics, PlayerMessages, Player, SimplePlayer}, r#match::Match};

#[derive(Serialize, Deserialize, Hash, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SimpleParticipant {
    pub name: String,
    pub id: String,
    pub party_name: Option<String>
}

#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Participant {
    pub name: String,
    pub id: String,
    pub party_name: Option<String>,
    pub last_party_name: Option<String>,
    pub first_joined_match_at: u64,
    pub joined_party_at: Option<u64>,
    pub last_left_party_at: Option<u64>,
    pub stats: ParticipantStats
}

impl Participant {
    pub fn get_match_result(&self, current_match: &Match, end: &MatchEndData) -> PlayerMatchResult {
        let is_playing = self.party_name.is_some();
        if !is_playing {
            return PlayerMatchResult::Intermediate;
        };

        if end.is_tie(current_match) {
            PlayerMatchResult::Tie
        } else if end.winning_parties.contains(self.party_name.as_ref().unwrap_or(&String::new()))  {
            PlayerMatchResult::Win
        } else {
            PlayerMatchResult::Lose
        }
    }

    pub fn from_simple(simple: SimpleParticipant) -> Self {
        let time_millis = get_u64_time_millis();
        Participant {
            name: simple.name,
            id: simple.id,
            party_name: simple.party_name.clone(),
            last_party_name: simple.party_name,
            first_joined_match_at: time_millis,
            joined_party_at: Some(time_millis),
            last_left_party_at: None,
            stats: Default::default(),
        }
    }

    pub async fn get_player(&self, state: &MarsAPIState) -> Player {
        state.player_cache.get(state.database.as_ref(), &self.name.to_lowercase()).await.expect("Expected player in cache")
    }

    pub fn get_name_lower(&self) -> String {
        self.name.to_lowercase()
    }

    pub async fn set_player(&self, state: &MarsAPIState, player: &Player) {
        state.player_cache.set(&state.database, &self.get_name_lower(), player, false).await;
    }

    pub fn get_simple_player(&self) -> SimplePlayer {
        return SimplePlayer { name: self.name.clone(), id: self.id.clone() }
    }

    pub fn get_id_name(&self) -> String {
        format!("{}/{}", &self.id, &self.name)
    }
}

#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct ParticipantStats {
    pub game_playtime: u64,
    pub time_away: u64,
    pub kills: u32,
    pub deaths: u32,
    pub void_kills: u32,
    pub void_deaths: u32,
    pub objectives: PlayerObjectiveStatistics,
    pub bow_shots_taken: u32,
    pub bow_shots_hit: u32,
    pub blocks_placed: HashMap<String, u32>,
    pub blocks_broken: HashMap<String, u32>,
    pub damage_taken: f64,
    pub damage_given: f64,
    pub damage_given_bow: f64,
    pub messages: PlayerMessages,
    pub weapon_kills: HashMap<String, u32>,
    pub weapon_deaths: HashMap<String, u32>,
    pub killstreaks: HashMap<String, u32>,
    pub killstreaks_ended: HashMap<String, u32>,
    pub duels: HashMap<String, Duel>
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Duel {
    #[serde(default)]
    pub kills: u32,
    #[serde(default)]
    pub deaths: u32
}

impl Default for Duel {
    fn default() -> Self {
        Duel {
            kills: 0,
            deaths: 0,
        }
    }
}
