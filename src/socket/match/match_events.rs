use std::collections::{HashSet, HashMap};

use serde::{Serialize, Deserialize};

use crate::database::models::{participant::SimpleParticipant, r#match::Match};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchStartData {
    pub participants: HashSet<SimpleParticipant>
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MatchEndData {
    pub winning_parties: Vec<String>,
    pub big_stats: HashMap<String, BigStats>
}

impl MatchEndData {
    pub fn is_tie(&self, i_match: &Match) -> bool {
        self.winning_parties.is_empty() || self.winning_parties.len() == i_match.parties.len()
    }

    pub fn get_stats_for_participant(&mut self, id: &String) -> &mut BigStats {
        if !self.big_stats.contains_key(id) {
            self.big_stats.insert(id.to_owned(), BigStats::default());
        };
        return self.big_stats.get_mut(id).unwrap();
    }
}

#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct BigStats {
    pub blocks: PlayerBlocksData,
    pub bow_shots_taken: u32,
    pub bow_shots_hit: u32,
    pub damage_given: f64,
    pub damage_taken: f64,
    pub damage_given_bow: f64
}

#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct PlayerBlocksData {
    pub blocks_placed: HashMap<String, u32>,
    pub blocks_broken: HashMap<String, u32>
}

