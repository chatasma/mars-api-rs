use mars_api_rs_macro::IdentifiableDocument;
use mars_api_rs_derive::IdentifiableDocument;
use serde::{Serialize, Deserialize};

use crate::database::CollectionOwner;

use super::{r#match::GoalCollection, player::{PlayerRecord, ProjectileRecord, FirstBloodRecord}};

#[derive(Serialize, Deserialize, IdentifiableDocument)]
#[serde(rename_all = "camelCase")]
pub struct Level {
    #[id]
    #[serde(rename = "_id")]
    pub id: String,
    pub loaded_at: u64,
    pub name: String,
    pub name_lower: String,
    pub version: String,
    pub gamemodes: Vec<LevelGamemode>,
    pub updated_at: u64,
    pub authors: Vec<LevelContributor>,
    pub contributors: Vec<LevelContributor>,
    #[serde(default)]
    pub goals: Option<GoalCollection>,
    #[serde(default)]
    pub last_match_id: Option<String>,
    pub records: LevelRecords
}

impl CollectionOwner<Level> for Level {
    fn get_collection(database: &crate::database::Database) -> &mongodb::Collection<Level> {
        &database.levels
    }

    fn get_collection_name() -> &'static str {
        "level"
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelRecords {
    #[serde(default)]
    pub highest_killstreak: Option<PlayerRecord<u32>>,
    #[serde(default)]
    pub longest_projectile_kill: Option<ProjectileRecord>,
    #[serde(default)]
    pub fastest_wool_capture: Option<PlayerRecord<u64>>,
    #[serde(default)]
    pub fastest_flag_capture: Option<PlayerRecord<u64>>,
    #[serde(default)]
    pub fastest_first_blood: Option<FirstBloodRecord>,
    #[serde(default)]
    pub kills_in_match: Option<PlayerRecord<u32>>,
    #[serde(default)]
    pub deaths_in_match: Option<PlayerRecord<u32>>
}

impl Default for LevelRecords {
    fn default() -> Self {
        Self { 
            highest_killstreak: Default::default(), 
            longest_projectile_kill: Default::default(), 
            fastest_wool_capture: Default::default(), 
            fastest_flag_capture: Default::default(), 
            fastest_first_blood: Default::default(), 
            kills_in_match: Default::default(), 
            deaths_in_match: Default::default() 
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelContributor {
    uuid: String,
    contribution: Option<String>
}

#[derive(Debug, Serialize, Deserialize, Clone, strum_macros::EnumProperty, Hash, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum LevelGamemode {
    #[strum(props(fancy = "Attack/Defend"))]
    AttackDefend,
    #[strum(props(fancy = "Arcade"))]
    Arcade,
    #[strum(props(fancy = "Bed Wars"))]
    Bedwars,
    #[strum(props(fancy = "Blitz"))]
    Blitz,
    #[strum(props(fancy = "Blitz: Rage"))]
    BlitzRage,
    #[strum(props(fancy = "Bridge"))]
    Bridge,
    #[strum(props(fancy = "Capture the Flag"))]
    CaptureTheFlag,
    #[strum(props(fancy = "Control the Point"))]
    ControlThePoint,
    #[strum(props(fancy = "Capture the Wool"))]
    CaptureTheWool,
    #[strum(props(fancy = "Destroy the Core"))]
    DestroyTheCore,
    #[strum(props(fancy = "Destroy the Monument"))]
    DestroyTheMonument,
    #[strum(props(fancy = "Free For All"))]
    FreeForAll,
    #[strum(props(fancy = "Flag Football"))]
    FlagFootball,
    #[strum(props(fancy = "Infection"))]
    Infection,
    #[strum(props(fancy = "King of the Hill"))]
    KingOfTheHill,
    #[strum(props(fancy = "King of the Flag"))]
    KingOfTheFlag,
    #[strum(props(fancy = "Mixed"))]
    Mixed,
    #[strum(props(fancy = "Payload"))]
    Payload,
    #[strum(props(fancy = "Rage"))]
    Rage,
    #[strum(props(fancy = "Race for Wool"))]
    RaceForWool,
    #[strum(props(fancy = "Scorebox"))]
    Scorebox,
    #[strum(props(fancy = "Skywars"))]
    Skywars,
    #[strum(props(fancy = "Survival Games"))]
    SurvivalGames,
    #[strum(props(fancy = "Deathmatch"))]
    Deathmatch
}
