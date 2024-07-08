use mars_api_rs_macro::IdentifiableDocument;
use mars_api_rs_derive::IdentifiableDocument;
use serde::{Deserialize, Serialize};

use crate::database::CollectionOwner;

impl CollectionOwner<Achievement> for Achievement {
    fn get_collection(database: &crate::database::Database) -> &mongodb::Collection<Achievement> {
        &database.achievements
    }

    fn get_collection_name() -> &'static str {
        "achievement"
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, IdentifiableDocument)]
#[serde(rename_all = "camelCase")]
pub struct Achievement {
    #[serde(rename = "_id")] 
    #[id] 
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(rename = "category")]
    pub metadata: Option<AchievementMetadata>,
    pub agent: Agent,
    pub first_completion: Option<String>
}

#[derive(Serialize, Deserialize, Clone, Debug)]
// match kotlinx.serialization
#[serde(tag = "type")]
pub enum Agent {
    #[serde(rename = "TotalKillsAgentParams", rename_all = "camelCase")]
    TotalKillsAgentParams { target_kills: u32 },

    #[serde(rename = "KillStreakAgentParams", rename_all = "camelCase")]
    KillStreakAgentParams { target_streak: u32 },

    #[serde(rename = "FireDeathAgentParams", rename_all = "camelCase")]
    FireDeathAgentParams,

    #[serde(rename = "CaptureNoSprintAgentParams", rename_all = "camelCase")]
    CaptureNoSprintAgentParams,

    // "CompositeAgentParams"
    // CompositeAgentParams { agents: Vec<Agent> },

    #[serde(rename = "ChatMessageAgentParams", rename_all = "camelCase")]
    ChatMessageAgentParams { message: String },

    #[serde(rename = "LevelUpAgentParams", rename_all = "camelCase")]
    LevelUpAgentParams { level: u32 },

    #[serde(rename = "WoolCaptureAgentParams", rename_all = "camelCase")]
    WoolCaptureAgentParams { captures: u32 },

    #[serde(rename = "FirstBloodAgentParams", rename_all = "camelCase")]
    FirstBloodAgentParams { target: u32 },

    #[serde(rename = "BowDistanceAgentParams", rename_all = "camelCase")]
    BowDistanceAgentParams { distance: u64 },

    #[serde(rename = "FlagCaptureAgentParams", rename_all = "camelCase")]
    FlagCaptureAgentParams { captures: u32 },

    #[serde(rename = "FlagDefendAgentParams", rename_all = "camelCase")]
    FlagDefendAgentParams { defends: u32 },

    #[serde(rename = "WoolDefendAgentParams", rename_all = "camelCase")]
    WoolDefendAgentParams { defends: u32 },

    #[serde(rename = "MonumentDamageAgentParams", rename_all = "camelCase")]
    MonumentDamageAgentParams { breaks: u32 },

    #[serde(rename = "MonumentDestroyAgentParams", rename_all = "camelCase")]
    MonumentDestroyAgentParams { destroys: u32 },

    #[serde(rename = "KillConsecutiveAgentParams", rename_all = "camelCase")]
    KillConsecutiveAgentParams { seconds: u64, kills: u32, all_within: bool },

    #[serde(rename = "PlayTimeAgentParams", rename_all = "camelCase")]
    PlayTimeAgentParams { hours: u64 },

    #[serde(rename = "RecordAgentParams", rename_all = "camelCase")]
    RecordAgentParams { record_type: RecordType, threshold: u32 },

    #[serde(rename = "ControlPointCaptureAgentParams", rename_all = "camelCase")]
    ControlPointCaptureAgentParams { captures: u32 },

    #[serde(rename = "TotalWinsAgentParams", rename_all = "camelCase")]
    TotalWinsAgentParams { wins: u32 },

    #[serde(rename = "TotalDeathsAgentParams", rename_all = "camelCase")]
    TotalDeathsAgentParams { deaths: u32 },

    #[serde(rename = "TotalLossesAgentParams", rename_all = "camelCase")]
    TotalLossesAgentParams { losses: u32 }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RecordType {
    LongestSession,
    LongestProjectileKill,
    FastestWoolCapture,
    FastestFlagCapture,
    FastestFirstBlood,
    KillsInMatch,
    DeathsInMatch
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AchievementMetadata {
    pub category: String,
    pub display_name: String,
    pub description: String
}

// some defaults mew specified I guess
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AchievementMetadataConstant {
    NoCategory,
    // Obtain a killstreak of "x"
    BloodBathCategory,
    // Capture "x" wools
    WoolieMammothCategory,
    // Obtain "x" total kills
    PathToGenocideCategory,
    // Shoot and kill a player from "x" blocks away
    MarksmanCategory,
    // Kill "x" players within a span of "y" seconds.
    MercilessCategory,
    // Kill "x" players within "y" seconds of each kill.
    WomboComboCategory,
    // Die to a source of fire.
    BurntToastCategory,
    // Obtain your first first-blood kill.
    BloodGodCategory,
    // Obtain "x" first-blood kills.
    TotalFirstBloodsCategory,
    // Damage "x" monument blocks overall.
    PillarsOfSandCategory,
    // Capture "x" flags overall.
    TouchdownCategory,
    // Stop "x" flag holders from capturing the flag.
    PassInterferenceCategory,
    // Capture "x" control-point objectives overall.
    TerritorialDisputeCategory,
    // Win "x" matches overall.
    VictoryScreechCategory,
    // Reach level "x".
    ChampionRoadCategory,
    // Play for "x" hours in matches overall.
    TouchGrassCategory,
    // Win your first match.
    FirstWinCategory,
    // Obtain your first kill.
    FirstKillCategory,
    // Obtain your first loss.
    FirstLossCategory,
    // Obtain your first death.
    FirstDeathCategory
}

pub fn achievement_metadata_preset_to_metadata(constant: AchievementMetadataConstant) -> AchievementMetadata  {
    return match constant {
        AchievementMetadataConstant::NoCategory => {
            AchievementMetadata {category: String::from("Misc"), display_name: String::from("No Display Name"), description: String::from("No Description")}
        },
        AchievementMetadataConstant::BloodBathCategory => {
            AchievementMetadata {category: String::from("Kills"), display_name: String::from("Blood Bath"), description: String::from("Click here to view this achievement.")}
        },
        AchievementMetadataConstant::WoolieMammothCategory => {
            AchievementMetadata {category: String::from("Objectives"), display_name: String::from("Woolie Mammoth"), description: String::from("Click here to view this achievement.")}
        },
        AchievementMetadataConstant::PathToGenocideCategory => {
            AchievementMetadata {category: String::from("Kills"), display_name: String::from("Path to Genocide"), description: String::from("Click here to view this achievement.")}
        },
        AchievementMetadataConstant::MarksmanCategory => {
            AchievementMetadata {category: String::from("Kills"), display_name: String::from("Marksman"), description: String::from("Click here to view this achievement.")}
        },
        AchievementMetadataConstant::MercilessCategory => {
            AchievementMetadata {category: String::from("Kills"), display_name: String::from("Merciless"), description: String::from("Click here to view this achievement.")}
        },
        AchievementMetadataConstant::WomboComboCategory => {
            AchievementMetadata {category: String::from("Kills"), display_name: String::from("Wombo Combo"), description: String::from("Click here to view this achievement.")}
        },
        AchievementMetadataConstant::BurntToastCategory => {
            AchievementMetadata {category: String::from("Deaths"), display_name: String::from("Burnt Toast"), description: String::from("Click here to view this achievement.")}
        },
        AchievementMetadataConstant::BloodGodCategory => {
            AchievementMetadata {category: String::from("Kills"), display_name: String::from("Blood God"), description: String::from("Click here to view this achievement.")}
        },
        AchievementMetadataConstant::TotalFirstBloodsCategory => {
            AchievementMetadata {category: String::from("Kills"), display_name: String::from("Swift as the Wind"), description: String::from("Click here to view this achievement.")}
        },
        AchievementMetadataConstant::PillarsOfSandCategory => {
            AchievementMetadata {category: String::from("Objectives"), display_name: String::from("Pillars of Sand"), description: String::from("Click here to view this achievement.")}
        },
        AchievementMetadataConstant::TouchdownCategory => {
            AchievementMetadata {category: String::from("Objectives"), display_name: String::from("Touchdown"), description: String::from("Click here to view this achievement.")}
        },
        AchievementMetadataConstant::PassInterferenceCategory => {
            AchievementMetadata {category: String::from("Objectives"), display_name: String::from("Pass Interference"), description: String::from("Click here to view this achievement.")}
        },
        AchievementMetadataConstant::TerritorialDisputeCategory => {
            AchievementMetadata {category: String::from("Objectives"), display_name: String::from("Territorial Dispute"), description: String::from("Click here to view this achievement.")}
        },
        AchievementMetadataConstant::VictoryScreechCategory => {
            AchievementMetadata {category: String::from("Wins"), display_name: String::from("Victory Screech"), description: String::from("Click here to view this achievement.")}
        },
        AchievementMetadataConstant::ChampionRoadCategory => {
            AchievementMetadata {category: String::from("Misc"), display_name: String::from("Champion Road"), description: String::from("Click here to view this achievement.")}
        },
        AchievementMetadataConstant::TouchGrassCategory => {
            AchievementMetadata {category: String::from("Misc"), display_name: String::from("Touch Grass"), description: String::from("Click here to view this achievement.")}
        },
        AchievementMetadataConstant::FirstWinCategory => {
            AchievementMetadata {category: String::from("Wins"), display_name: String::from("Mom, Get the Camera!"), description: String::from("Click here to view this achievement.")}
        },
        AchievementMetadataConstant::FirstKillCategory => {
            AchievementMetadata {category: String::from("Kills"), display_name: String::from("Baby Steps"), description: String::from("Click here to view this achievement.")}
        },
        AchievementMetadataConstant::FirstLossCategory => {
            AchievementMetadata {category: String::from("Losses"), display_name: String::from("My Stats!"), description: String::from("Click here to view this achievement.")}
        },
        AchievementMetadataConstant::FirstDeathCategory => {
            AchievementMetadata {category: String::from("Deaths"), display_name: String::from("Oof!"), description: String::from("Click here to view this achievement.")}
        }
    }
}
