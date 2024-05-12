use serde::{Deserialize, Serialize};

use crate::database::models::achievement::{AchievementMetadataConstant, Agent};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AchievementCreateRequest {
    pub name: String,
    pub description: String,
    #[serde(rename = "category")]
    pub metadata_preset: Option<AchievementMetadataConstant>,
    pub agent: Agent
}
