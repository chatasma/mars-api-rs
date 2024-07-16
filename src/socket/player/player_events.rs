use serde::{Serialize, Deserialize};

use crate::database::models::{player::SimplePlayer, death::DamageCause};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerDeathData {
    pub victim: SimplePlayer,
    pub attacker: Option<SimplePlayer>,
    pub weapon: Option<String>,
    pub entity: Option<String>,
    pub distance: Option<u32>,
    pub key: String,
    pub cause: DamageCause
}

impl PlayerDeathData {
    pub fn is_murder(&self) -> bool {
        self.attacker.is_some() && self.attacker.as_ref().unwrap() != &self.victim
    }

    pub fn safe_weapon(&self) -> String {
        if self.distance.is_some() && self.cause != DamageCause::Fall { 
            String::from("PROJECTILE") 
        } else { 
            self.weapon.as_ref().unwrap_or(&String::from("NONE")).to_owned()
        } 
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerChatData {
    pub player: SimplePlayer,
    pub player_prefix: String,
    pub channel: ChatChannel,
    pub message: String,
    pub server_id: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ChatChannel {
    Staff,
    Global,
    Team
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KillstreakData {
    pub amount: u32,
    pub player: SimplePlayer,
    pub ended: bool
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartyJoinData {
    pub player: SimplePlayer,
    pub party_name: String
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartyLeaveData {
    pub player: SimplePlayer
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageData {
    pub message: String,
    pub sound: Option<String>,
    pub player_ids: Vec<String>
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerXPGainData {
    pub player_id: String,
    pub gain: u32,
    pub reason: String,
    pub notify: bool,
    pub multiplier: Option<f32>
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisconnectPlayerData {
    pub player_id: String,
    pub reason: String
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PlayerAchievementData { 
    pub player: SimplePlayer, 
    pub achievement_id: String, 
    pub completion_time: u64
}
