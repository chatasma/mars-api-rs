use crate::{socket::{r#match::match_events::MatchEndData, server::server_context::ServerContext, participant::participant_context::{PlayerMatchResult}}, database::models::{death::DamageCause, player::Player, r#match::{Match, DestroyableGoal}}};

use super::{player_context::{send_message_to_player}, player_listener::PlayerListener, player_events::{PlayerDeathData, PlayerChatData, ChatChannel}};
use async_trait::async_trait;

pub struct PlayerStatListener {}

#[async_trait]
impl PlayerListener for PlayerStatListener {
    type Context = Player;

    async fn on_kill(
        &self,
        _server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context, 
        data: &mut PlayerDeathData, 
        first_blood: bool
    ) { 
        {
            if !current_match.is_tracking_stats() {
                return;
            };

            context.stats.kills += 1;

            if first_blood { context.stats.first_bloods += 1; };
            if data.cause == DamageCause::Void { context.stats.void_kills += 1; };

            let weapon_kills = context.stats.weapon_kills.get(&data.safe_weapon()).unwrap_or(&0).to_owned();
            context.stats.weapon_kills.insert(data.safe_weapon(), weapon_kills + 1);
        };
    }

    async fn on_chat(
        &self,
        _server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context, 
        data: &mut PlayerChatData
    ) {
        {
            if !current_match.is_tracking_stats() {
                return;
            };
        };
        match &data.channel {
            ChatChannel::Global => { context.stats.messages.global += 1; },
            ChatChannel::Team => { context.stats.messages.team += 1; },
            ChatChannel::Staff => { context.stats.messages.staff += 1; }
        };
    }

    async fn on_death(
        &self,
        _server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context, 
        data: &mut PlayerDeathData, 
        first_blood: bool
    ) { 
        {
            if !current_match.is_tracking_stats() {
                return;
            };

            context.stats.deaths += 1;

            if data.cause == DamageCause::Void { context.stats.void_deaths += 1; };

            if first_blood { context.stats.first_bloods_suffered += 1; };

            if data.is_murder() {
                let weapon_deaths = context.stats.weapon_deaths.get(&data.safe_weapon()).unwrap_or(&0).to_owned();
                context.stats.weapon_deaths.insert(data.safe_weapon(), weapon_deaths + 1);
            };
        };
    }

    async fn on_killstreak(
        &self,
        _server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context, 
        amount: u32
    ) {
        {
            if !current_match.is_tracking_stats() {
                return;
            };

            let current_killstreak_count = context.stats.killstreaks.get(&amount.to_string()).unwrap_or(&0).to_owned();
            context.stats.killstreaks.insert(amount.to_string(), current_killstreak_count + 1);
        };
    }

    async fn on_killstreak_end(
        &self,
        _server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context, 
        amount: u32
    ) {
        {
            if !current_match.is_tracking_stats() {
                return;
            };

            let current_killstreak_count = context.stats.killstreaks_ended.get(&amount.to_string()).unwrap_or(&0).to_owned();
            context.stats.killstreaks_ended.insert(amount.to_string(), current_killstreak_count + 1);
        };
    }

    async fn on_destroyable_damage(
        &self, 
        _server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context, 
        _destroyable: &DestroyableGoal, 
        block_count: u32
    ) {
        if !current_match.is_tracking_stats() {
            return;
        };
        context.stats.objectives.destroyable_block_destroys += block_count;
    }

    async fn on_destroyable_destroy(
        &self, 
        _server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context, 
        _percentage: f32, 
        _block_count: u32
    ) {
        if !current_match.is_tracking_stats() {
            return;
        };
        context.stats.objectives.destroyable_destroys += 1;
    }

    async fn on_core_leak(
        &self, 
        _server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context, 
        _percentage: f32, 
        block_count: u32
    ) {
        if !current_match.is_tracking_stats() {
            return;
        };
        context.stats.objectives.core_leaks += 1;
        context.stats.objectives.core_block_destroys += block_count;
    }

    async fn on_control_point_capture(
        &self, 
        _server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context, 
        _contributors: u32, 
    ) {
        if !current_match.is_tracking_stats() {
            return;
        };

        context.stats.objectives.control_point_captures += 1;
    }

    async fn on_flag_place(
        &self, 
        _server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context, 
        held_time: u64, 
    ) {
        if !current_match.is_tracking_stats() {
            return;
        };

        context.stats.objectives.flag_captures += 1;
        context.stats.objectives.total_flag_hold_time += held_time;
    }

    async fn on_flag_pickup(
        &self, 
        _server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context
    ) {
        if !current_match.is_tracking_stats() {
            return;
        };

        context.stats.objectives.flag_pickups += 1;
    }

    async fn on_flag_drop(
        &self, 
        _server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context,
        held_time: u64, 
    ) {
        if !current_match.is_tracking_stats() {
            return;
        };

        context.stats.objectives.flag_drops += 1;
        context.stats.objectives.total_flag_hold_time += held_time;
    }

    async fn on_flag_defend(
        &self, 
        _server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context
    ) {
        if !current_match.is_tracking_stats() {
            return;
        };

        context.stats.objectives.flag_defends += 1;
    }

    async fn on_wool_place(
        &self, 
        _server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context, 
        _held_time: u64, 
    ) {
        if !current_match.is_tracking_stats() {
            return;
        };

        context.stats.objectives.wool_captures += 1;
    }

    async fn on_wool_pickup(
        &self, 
        _server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context
    ) {
        if !current_match.is_tracking_stats() {
            return;
        };

        context.stats.objectives.wool_pickups += 1;
    }

    async fn on_wool_drop(
        &self, 
        _server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context,
        _held_time: u64, 
    ) {
        if !current_match.is_tracking_stats() {
            return;
        };

        context.stats.objectives.wool_drops += 1;
    }

    async fn on_wool_defend(
        &self, 
        _server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context
    ) {
        if !current_match.is_tracking_stats() {
            return;
        };

        context.stats.objectives.wool_defends += 1;
    }

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
            };

            // let participant = context.get_participant();
            let participant = current_match.participants.get_mut(&context.id).expect("Participant should exist").clone();
            let match_result = participant.get_match_result(current_match, end_data);

            let participant_id = &context.id;
            // let big_stats = end_data.big_stats.get_mut(participant_id).unwrap();
            let big_stats = end_data.get_stats_for_participant(participant_id);

            let blocks = &mut big_stats.blocks;
            blocks.blocks_broken.iter().for_each(|interaction| {
                context.stats.blocks_broken.insert(interaction.0.clone(), interaction.1.clone());
            });
            blocks.blocks_placed.iter().for_each(|interaction| {
                context.stats.blocks_placed.insert(interaction.0.clone(), interaction.1.clone());
            });

            context.stats.bow_shots_taken = big_stats.bow_shots_taken;
            context.stats.bow_shots_hit = big_stats.bow_shots_hit;
            context.stats.damage_given = big_stats.damage_given;
            context.stats.damage_taken = big_stats.damage_taken;
            context.stats.damage_given_bow = big_stats.damage_given_bow;

            let min_playtime = (0.10 * (current_match.get_length() as f64)).min(60_000.0);
            let is_playing = participant.party_name.is_some();
            if (participant.stats.game_playtime as f64) > min_playtime {
                match match_result {
                    PlayerMatchResult::Tie => context.stats.ties += 1,
                    PlayerMatchResult::Win => context.stats.wins += 1,
                    PlayerMatchResult::Lose => context.stats.losses += 1,
                    _ => {}
                }
            } else {
                // server_context.send_message(&context, "Your stats were not affected by the outcome of this match as you did not participate for long enough.");
                send_message_to_player(server_context, context, "Your stats were not affected by the outcome of this match as you did not participate for long enough.", Option::None).await;
                // context.send_message(server_context, "Your stats were not affected by the outcome of this match as you did not participate for long enough.", Option::None).await;
            };

            let time_elapsed_before_joining = participant.first_joined_match_at.saturating_sub(current_match.started_at.unwrap_or(0));

            if (participant.stats.game_playtime as f64) > min_playtime {
                context.stats.matches += 1;
            };

            if (time_elapsed_before_joining as f64) < min_playtime {
                context.stats.matches_present_start += 1;
            };

            if participant.stats.time_away < 20000 && is_playing {
                context.stats.matches_present_full += 1;
            };

            if is_playing {
                context.stats.matches_present_end += 1;
            };

            context.stats.game_playtime += participant.stats.game_playtime;
        };
    }
}
