use crate::{database::models::{r#match::{DestroyableGoal, Match}, player::Player}, socket::{event_type::EventType, r#match::match_events::MatchEndData, player::{player_events::{PlayerChatData, PlayerDeathData}, player_listener::PlayerListener}, server::server_context::ServerContext}};
use async_trait::async_trait;
use serde::{Serialize, Deserialize};

pub struct PlayerUpdateListener {}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PlayerUpdateReason {
    Kill,
    Death,
    Chat,
    Killstreak,
    KillstreakEnd,
    PartyJoin,
    PartyLeave,
    MatchEnd,
    DestroyableDamage,
    DestroyableDestroy,
    CoreLeak,
    FlagPlace,
    FlagDrop,
    FlagPickup,
    FlagDefend,
    WoolPlace,
    WoolDrop,
    WoolPickup,
    WoolDefend,
    ControlPointCapture,
}

#[derive(Serialize, Deserialize)]
// match kotlinx.serialization
#[serde(tag = "type")]
pub enum PlayerUpdateData {
    #[serde(rename = "KillUpdateData", rename_all = "camelCase")]
    KillUpdateData { data: PlayerDeathData, first_blood: bool },
    #[serde(rename = "ChatUpdateData", rename_all = "camelCase")]
    ChatUpdateData { data: PlayerChatData },
    #[serde(rename = "KillstreakUpdateData", rename_all = "camelCase")]
    KillstreakUpdateData { amount: u32 },
    #[serde(rename = "PartyUpdateData", rename_all = "camelCase")]
    PartyUpdateData { party: String },
    #[serde(rename = "MatchEndUpdateData", rename_all = "camelCase")]
    MatchEndUpdateData { data: MatchEndData },
    #[serde(rename = "DestroyableDamageUpdateData", rename_all = "camelCase")]
    DestroyableDamageUpdateData { block_count: u32 },
    #[serde(rename = "DestroyableDestroyUpdateData", rename_all = "camelCase")]
    DestroyableDestroyUpdateData { percentage: f32, block_count: u32 },
    #[serde(rename = "CoreLeakUpdateData", rename_all = "camelCase")]
    CoreLeakUpdateData { percentage: f32, block_count: u32 },
    #[serde(rename = "MonumentPlaceUpdateData", rename_all = "camelCase")]
    MonumentPlaceUpdateData { held_time: u64 },
    #[serde(rename = "MonumentDropUpdateData", rename_all = "camelCase")]
    MonumentDropUpdateData { held_time: u64 },
    #[serde(rename = "ControlPointCaptureUpdateData", rename_all = "camelCase")]
    ControlPointCaptureUpdateData { contributors: u32 },
    #[serde(rename = "NoArgs", rename_all = "camelCase")]
    NoArgs
}

#[derive(Serialize, Deserialize)]
pub struct PlayerUpdate {
    pub updated: Player,
    pub data: PlayerUpdateData,
    pub reason: PlayerUpdateReason
}

impl PlayerUpdateListener {
    async fn send_player_update(
        server_context: &mut ServerContext,
        player: Player,
        data: PlayerUpdateData,
        reason: PlayerUpdateReason
    ) {
        server_context.call(&EventType::PlayerUpdate, PlayerUpdate {updated: player, data, reason }).await;
    }
}

#[async_trait]
impl PlayerListener for PlayerUpdateListener {
    type Context = Player;

    async fn on_kill(
        &self, 
        server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context, 
        data: &mut PlayerDeathData, 
        first_blood: bool
    ) {
        PlayerUpdateListener::send_player_update(
            server_context, context.to_owned(), 
            PlayerUpdateData::KillUpdateData {data: data.to_owned(), first_blood}, 
            PlayerUpdateReason::Kill
        ).await;
    }

    async fn on_death(
        &self, 
        server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context, 
        data: &mut PlayerDeathData, 
        first_blood: bool
    ) {
        PlayerUpdateListener::send_player_update(
            server_context, context.to_owned(), 
            PlayerUpdateData::KillUpdateData {data: data.to_owned(), first_blood}, 
            PlayerUpdateReason::Death
        ).await;
    }

    async fn on_chat(
        &self, 
        server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context, 
        data: &mut PlayerChatData
    ) {
        PlayerUpdateListener::send_player_update(
            server_context, context.to_owned(), 
            PlayerUpdateData::ChatUpdateData {data: data.to_owned()}, 
            PlayerUpdateReason::Chat
        ).await;
    }

    async fn on_killstreak(
        &self, 
        server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context, 
        amount: u32
    ) {
        PlayerUpdateListener::send_player_update(
            server_context, context.to_owned(), 
            PlayerUpdateData::KillstreakUpdateData {amount}, 
            PlayerUpdateReason::Killstreak
        ).await;
    }

    async fn on_killstreak_end(
        &self, 
        server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context, 
        amount: u32
    ) {
        PlayerUpdateListener::send_player_update(
            server_context, context.to_owned(), 
            PlayerUpdateData::KillstreakUpdateData {amount}, 
            PlayerUpdateReason::KillstreakEnd
        ).await;
    }

    async fn on_party_join(
        &self, 
        server_context: &mut ServerContext, 
        _current_match: &mut Match,
        context: &mut Self::Context, 
        party_name: String
    ) {
        PlayerUpdateListener::send_player_update(
            server_context, context.to_owned(), 
            PlayerUpdateData::PartyUpdateData {party: party_name}, 
            PlayerUpdateReason::PartyJoin
        ).await;
    }

    async fn on_party_leave(
        &self, 
        server_context: &mut ServerContext, 
        current_context: &mut Match, 
        context: &mut Self::Context
    ) {
        PlayerUpdateListener::send_player_update(
            server_context, context.to_owned(), 
            PlayerUpdateData::NoArgs,
            PlayerUpdateReason::PartyLeave
        ).await;
    }

    async fn on_match_end_v2(
        &self, 
        server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context, 
        end_data: &mut MatchEndData
    ) {
        PlayerUpdateListener::send_player_update(
            server_context, context.to_owned(), 
            PlayerUpdateData::MatchEndUpdateData { data: end_data.to_owned() },
            PlayerUpdateReason::MatchEnd
        ).await;
    }

    async fn on_destroyable_damage(
        &self, 
        server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context, 
        destroyable: &DestroyableGoal, 
        block_count: u32
    ) {
        PlayerUpdateListener::send_player_update(
            server_context, context.to_owned(), 
            PlayerUpdateData::DestroyableDamageUpdateData { block_count },
            PlayerUpdateReason::DestroyableDamage
        ).await;
    }

    async fn on_destroyable_destroy(
        &self, 
        server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context, 
        percentage: f32, 
        block_count: u32
    ) {
        PlayerUpdateListener::send_player_update(
            server_context, context.to_owned(), 
            PlayerUpdateData::DestroyableDestroyUpdateData { percentage, block_count },
            PlayerUpdateReason::DestroyableDestroy
        ).await;
    }

    async fn on_core_leak(
        &self, 
        server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context, 
        percentage: f32, 
        block_count: u32
    ) {
        PlayerUpdateListener::send_player_update(
            server_context, context.to_owned(), 
            PlayerUpdateData::CoreLeakUpdateData { percentage, block_count },
            PlayerUpdateReason::CoreLeak
        ).await;
    }

    async fn on_control_point_capture(
        &self, 
        server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context, 
        contributors: u32, 
    ) {
        PlayerUpdateListener::send_player_update(
            server_context, context.to_owned(), 
            PlayerUpdateData::ControlPointCaptureUpdateData { contributors },
            PlayerUpdateReason::ControlPointCapture
        ).await;
    }

    async fn on_flag_place(
        &self, 
        server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context, 
        held_time: u64, 
    ) {
        PlayerUpdateListener::send_player_update(
            server_context, context.to_owned(), 
            PlayerUpdateData::MonumentPlaceUpdateData { held_time },
            PlayerUpdateReason::FlagPlace
        ).await;
    }

    async fn on_flag_pickup(
        &self, 
        server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context
    ) {
        PlayerUpdateListener::send_player_update(
            server_context, context.to_owned(), 
            PlayerUpdateData::NoArgs,
            PlayerUpdateReason::FlagPickup
        ).await;
    }

    async fn on_flag_drop(
        &self, 
        server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context,
        held_time: u64, 
    ) {
        PlayerUpdateListener::send_player_update(
            server_context, context.to_owned(), 
            PlayerUpdateData::MonumentDropUpdateData { held_time },
            PlayerUpdateReason::FlagDrop
        ).await;
    }

    async fn on_flag_defend(
        &self, 
        server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context
    ) {
        PlayerUpdateListener::send_player_update(
            server_context, context.to_owned(), 
            PlayerUpdateData::NoArgs,
            PlayerUpdateReason::FlagDefend
        ).await;
    }

    async fn on_wool_place(
        &self, 
        server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context, 
        held_time: u64, 
    ) {
        PlayerUpdateListener::send_player_update(
            server_context, context.to_owned(), 
            PlayerUpdateData::MonumentPlaceUpdateData { held_time },
            PlayerUpdateReason::WoolPlace
        ).await;
    }

    async fn on_wool_pickup(
        &self, 
        server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context
    ) {
        PlayerUpdateListener::send_player_update(
            server_context, context.to_owned(), 
            PlayerUpdateData::NoArgs,
            PlayerUpdateReason::WoolPickup
        ).await;
    }

    async fn on_wool_drop(
        &self, 
        server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context,
        held_time: u64, 
    ) {
        PlayerUpdateListener::send_player_update(
            server_context, context.to_owned(), 
            PlayerUpdateData::MonumentDropUpdateData { held_time },
            PlayerUpdateReason::WoolDrop
        ).await;
    }

    async fn on_wool_defend(
        &self, 
        server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context
    ) {
        PlayerUpdateListener::send_player_update(
            server_context, context.to_owned(), 
            PlayerUpdateData::NoArgs,
            PlayerUpdateReason::WoolDefend
        ).await;
    }
}
