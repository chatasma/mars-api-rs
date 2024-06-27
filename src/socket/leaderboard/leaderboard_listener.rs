use crate::{socket::{player::{player_listener::PlayerListener, player_events::PlayerDeathData}, participant::participant_context::{PlayerMatchResult}, r#match::match_events::{MatchEndData}, server::server_context::ServerContext}, database::models::{participant::Participant, r#match::Match}};

pub struct LeaderboardListener {}

#[async_trait]
impl PlayerListener for LeaderboardListener {
    type Context = Participant;

    async fn on_match_end_v2(
        &self,
        server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context, 
        end_data: &mut MatchEndData
    ) { 
        {
            if !current_match.is_tracking_stats() {
                return;
            }

            let match_result = current_match.get_participant_match_result(&context, end_data);

            match match_result {
                PlayerMatchResult::Win => {
                    server_context.api_state.leaderboards.wins.process_update(context.get_id_name(), 1).await;
                },
                PlayerMatchResult::Lose => {
                    server_context.api_state.leaderboards.losses.process_update(context.get_id_name(), 1).await;
                },
                PlayerMatchResult::Tie => {
                    server_context.api_state.leaderboards.ties.process_update(context.get_id_name(), 1).await;
                },
                _ => {} 
            }

            server_context.api_state.leaderboards.matches_played.process_update(context.get_id_name(), 1).await;
            server_context.api_state.leaderboards.messages_sent.process_update(
                context.get_id_name(),
                context.stats.messages.total()
            ).await;
            server_context.api_state.leaderboards.game_playtime.process_update(
                context.get_id_name(),
                u32::try_from(context.stats.game_playtime).unwrap_or(0)
            ).await;
        };
    }


    async fn on_kill(
        &self,
        server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context, 
        _data: &mut PlayerDeathData, 
        first_blood: bool
    ) { 
        {
            if !current_match.is_tracking_stats() {
                return;
            };

            server_context.api_state.leaderboards.kills.process_update(context.get_id_name(), 1).await;
            if first_blood {
                server_context.api_state.leaderboards.first_bloods.process_update(context.get_id_name(), 1).await;
            };
        }
    }

    async fn on_death(
        &self,
        server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context, 
        _data: &mut PlayerDeathData, 
        _first_blood: bool
    ) { 
        {
            if !current_match.is_tracking_stats() {
                return;
            };

            server_context.api_state.leaderboards.deaths.process_update(context.get_id_name(), 1).await;
        };
    }

    async fn on_killstreak(
        &self,
        server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context, 
        amount: u32
    ) {
        {
            if !current_match.is_tracking_stats() {
                return;
            };
            server_context.api_state.leaderboards.highest_killstreak.process_update(context.get_id_name(), amount).await;
        };
    }

    async fn on_destroyable_destroy(
        &self, 
        server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context, 
        _percentage: f32, 
        block_count: u32
    ) {
        if !current_match.is_tracking_stats() {
            return;
        };
        server_context.api_state.leaderboards.destroyable_destroys.process_update(context.get_id_name(), 1).await;
        server_context.api_state.leaderboards.destroyable_block_destroys.process_update(context.get_id_name(), block_count).await;
    }

    async fn on_core_leak(
        &self, 
        server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context, 
        _percentage: f32, 
        _block_count: u32
    ) {
        if !current_match.is_tracking_stats() {
            return;
        };
        server_context.api_state.leaderboards.core_leaks.process_update(context.get_id_name(), 1).await;
        server_context.api_state.leaderboards.core_block_destroys.process_update(context.get_id_name(), 1).await;
    }

    async fn on_flag_place(
        &self, 
        server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context, 
        held_time: u64, 
    ) {
        if !current_match.is_tracking_stats() {
            return;
        };
        server_context.api_state.leaderboards.flag_captures.process_update(context.get_id_name(), 1).await;
        server_context.api_state.leaderboards.flag_hold_time.process_update(context.get_id_name(), u32::try_from(held_time).unwrap()).await;
    }

    async fn on_flag_pickup(
        &self, 
        server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context
    ) {
        if !current_match.is_tracking_stats() {
            return;
        };
        server_context.api_state.leaderboards.flag_pickups.process_update(context.get_id_name(), 1).await;
    }

    async fn on_flag_drop(
        &self, 
        server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context, 
        held_time: u64, 
    ) {
        if !current_match.is_tracking_stats() {
            return;
        };
        server_context.api_state.leaderboards.flag_drops.process_update(context.get_id_name(), 1).await;
        server_context.api_state.leaderboards.flag_hold_time.process_update(context.get_id_name(), u32::try_from(held_time).unwrap()).await;
    }

    async fn on_flag_defend(
        &self, 
        server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context
    ) {
        if !current_match.is_tracking_stats() {
            return;
        };
        server_context.api_state.leaderboards.flag_defends.process_update(context.get_id_name(), 1).await;
    }

    async fn on_wool_place(
        &self, 
        server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context, 
        _held_time: u64, 
    ) {
        if !current_match.is_tracking_stats() {
            return;
        };
        server_context.api_state.leaderboards.wool_captures.process_update(context.get_id_name(), 1).await;
    }

    async fn on_wool_pickup(
        &self, 
        server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context
    ) {
        if !current_match.is_tracking_stats() {
            return;
        };
        server_context.api_state.leaderboards.wool_pickups.process_update(context.get_id_name(), 1).await;
    }

    async fn on_wool_drop(
        &self, 
        server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context, 
        _held_time: u64, 
    ) {
        if !current_match.is_tracking_stats() {
            return;
        };
        server_context.api_state.leaderboards.wool_drops.process_update(context.get_id_name(), 1).await;
    }

    async fn on_wool_defend(
        &self, 
        server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context
    ) {
        if !current_match.is_tracking_stats() {
            return;
        };
        server_context.api_state.leaderboards.wool_defends.process_update(context.get_id_name(), 1).await;
    }

    async fn on_control_point_capture(
        &self, 
        server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context, 
        _contributors: u32, 
    ) {
        if !current_match.is_tracking_stats() {
            return;
        };

        server_context.api_state.leaderboards.control_point_captures.process_update(context.get_id_name(), 1).await;
    }
}
