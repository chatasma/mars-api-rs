use serde::{Deserialize, Serialize};
use strum_macros::{EnumString, Display};

#[derive(Serialize, Deserialize, EnumString, Display, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum EventType {
    // API-bound
    AchievementEarn,
    MatchLoad,
    MatchStart,
    MatchEnd,
    PlayerDeath,
    Killstreak,
    PartyJoin,
    PartyLeave,
    DestroyableDestroy,
    DestroyableDamage,
    CoreLeak,
    CoreDamage, // unused
    FlagCapture,
    FlagPickup,
    FlagDrop,
    FlagDefend,
    WoolCapture,
    WoolPickup,
    WoolDrop,
    WoolDefend,
    ControlPointCapture,

    // bi-directional
    PlayerChat,

    // plugin-bound
    PlayerXpGain,
    ForceMatchEnd,
    Message,
    DisconnectPlayer,
    PlayerUpdate
}
