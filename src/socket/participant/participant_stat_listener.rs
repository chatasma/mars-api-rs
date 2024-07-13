use crate::{socket::{player::{player_listener::PlayerListener, player_events::{PlayerDeathData, PlayerChatData, ChatChannel}}, r#match::match_events::{MatchEndData, BigStats}, server::server_context::ServerContext}, util::{time::get_u64_time_millis}, database::models::{death::DamageCause, participant::{Duel, Participant}, r#match::{Match, DestroyableGoal}}};


use async_trait::async_trait;

pub struct ParticipantStatListener {}

#[async_trait]
impl PlayerListener for ParticipantStatListener {
    type Context = Participant;

    async fn on_kill(
        &self,
        _server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context, 
        data: &mut PlayerDeathData, 
        _first_blood: bool
    ) { 
        context.stats.kills += 1;

        let weapon_kills = context.stats.weapon_kills.get(&data.safe_weapon()).unwrap_or(&0).to_owned();
        context.stats.weapon_kills.insert(data.safe_weapon(), weapon_kills + 1);

        let mut duel = match context.stats.duels.get(&data.victim.id) {
            Some(duel) => duel.clone(),
            None => Duel::default()
        };
        duel.kills += 1;
        context.stats.duels.insert(data.victim.id.clone(), duel);

        if data.cause == DamageCause::Void {
            context.stats.void_kills += 1;
        };
    }

    async fn on_death(
        &self,
        _server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context, 
        data: &mut PlayerDeathData, 
        _first_blood: bool
    ) { 
        context.stats.deaths += 1;

        if data.cause == DamageCause::Void {
            context.stats.void_deaths += 1;
        };

        if data.is_murder() {
            let weapon_deaths = context.stats.weapon_deaths.get(&data.safe_weapon()).unwrap_or(&0).to_owned();
            context.stats.weapon_deaths.insert(data.safe_weapon(), weapon_deaths + 1);

            let mut duel = match data.attacker.as_ref() {
                Some(attacker) => {
                    match context.stats.duels.get(&attacker.id) {
                        Some(duel) => duel.clone(),
                        None => Duel::default()
                    }
                },
                None => Duel::default()
            };
            duel.deaths += 1;
            match data.attacker.as_ref() {
                Some(attacker) => {
                    context.stats.duels.insert(attacker.id.clone(), duel);
                }
                None => {}
            };
        };
    }

    async fn on_chat(
        &self,
        _server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context, 
        data: &mut PlayerChatData
    ) {
        match &data.channel {
            ChatChannel::Global => { context.stats.messages.global += 1; },
            ChatChannel::Team => { context.stats.messages.team += 1; },
            ChatChannel::Staff => { context.stats.messages.staff += 1; }
        };
    }

    async fn on_killstreak(
        &self,
        _server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context, 
        amount: u32
    ) {
        let current_amount = context.stats.killstreaks.get(&amount.to_string()).unwrap_or(&0).to_owned();
        context.stats.killstreaks.insert(amount.to_string(), current_amount + 1);
    }

    async fn on_killstreak_end(
        &self,
        _server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context, 
        amount: u32
    ) {
        let current_amount = context.stats.killstreaks_ended.get(&amount.to_string()).unwrap_or(&0).to_owned();
        context.stats.killstreaks_ended.insert(amount.to_string(), current_amount + 1);
    }

    async fn on_party_join(
        &self,
        _server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context, 
        _party_name: String
    ) {
        if context.last_left_party_at.is_some() {
            let time_away = get_u64_time_millis() - context.last_left_party_at.unwrap();
            context.stats.time_away += time_away;
        };
    }

    async fn on_party_leave(
        &self,
        _server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context
    ) {
        context.stats.game_playtime += get_u64_time_millis().saturating_sub(context.joined_party_at.unwrap());
    }

    async fn on_core_leak(
        &self, 
        _server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context,
        _percentage: f32, 
        block_count: u32
    ) {
        context.stats.objectives.core_leaks += 1;
        context.stats.objectives.core_block_destroys += block_count;
    }

    async fn on_control_point_capture(
        &self, 
        _server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context,
        _contributors: u32
    ) {
        context.stats.objectives.control_point_captures += 1;
    }

    async fn on_flag_place(
        &self, 
        _server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context, 
        held_time: u64, 
    ) {
        context.stats.objectives.flag_captures += 1;
        context.stats.objectives.total_flag_hold_time += held_time;
    }

    async fn on_flag_pickup(
        &self, 
        _server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context
    ) {
        context.stats.objectives.flag_pickups += 1;
    }

    async fn on_flag_drop(
        &self, 
        _server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context,
        held_time: u64, 
    ) {
        context.stats.objectives.flag_drops += 1;
        context.stats.objectives.total_flag_hold_time += held_time;
    }

    async fn on_flag_defend(
        &self, 
        _server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context
    ) {
        context.stats.objectives.flag_defends += 1;
    }

    async fn on_wool_place(
        &self, 
        _server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context, 
        _held_time: u64, 
    ) {
        context.stats.objectives.wool_captures += 1;
    }

    async fn on_wool_pickup(
        &self, 
        _server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context
    ) {
        context.stats.objectives.wool_pickups += 1;
    }

    async fn on_wool_drop(
        &self, 
        _server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context,
        _held_time: u64, 
    ) {
        context.stats.objectives.wool_drops += 1;
    }

    async fn on_wool_defend(
        &self, 
        _server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context
    ) {
        context.stats.objectives.wool_defends += 1;
    }

    async fn on_destroyable_damage(
        &self, 
        _server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context, 
        _destroyable: &DestroyableGoal, 
        block_count: u32
    ) {
        context.stats.objectives.destroyable_block_destroys += block_count;
    }

    async fn on_destroyable_destroy(
        &self, 
        _server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context, 
        _percentage: f32, 
        _block_count: u32
    ) {
        context.stats.objectives.destroyable_destroys += 1;
    }

    async fn on_match_end_v2(
        &self,
        _server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context, 
        end_data: &mut MatchEndData
    ) {
        let participant_id = &context.id;

        let _big_stats_default = BigStats::default();
        // let big_stats = end_data.big_stats.get(participant_id).unwrap_or(&big_stats_default);
        let big_stats = end_data.get_stats_for_participant(participant_id);

        let blocks = &big_stats.blocks;
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

        let is_playing = context.party_name.is_some();
        let joined_party_at = context.joined_party_at;
        if is_playing && joined_party_at.is_some() {
            context.stats.game_playtime += current_match.ended_at.unwrap() - joined_party_at.unwrap();
        };
    }
}
