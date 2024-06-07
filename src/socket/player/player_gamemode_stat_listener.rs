use crate::{socket::{r#match::match_events::MatchEndData, server::server_context::ServerContext, participant::participant_context::PlayerMatchResult}, database::models::{level::LevelGamemode, player::{GamemodeStats, Player}, death::DamageCause, r#match::{Match, DestroyableGoal}}};

use super::{player_listener::PlayerListener, player_events::PlayerDeathData};

pub struct PlayerGamemodeStatListener {}

#[async_trait]
impl PlayerListener for PlayerGamemodeStatListener {
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
            let gamemodes = if !current_match.is_tracking_stats() { vec![LevelGamemode::Arcade] } else { current_match.level.gamemodes.clone() };
            for gamemode in gamemodes {
                let mut default_gamemode_stats = GamemodeStats::default();
                let stats = context.gamemode_stats.get_mut(&gamemode).unwrap_or(&mut default_gamemode_stats);

                stats.kills += 1;

                if first_blood {
                    stats.first_bloods += 1;
                };
                if data.cause == DamageCause::Void {
                    stats.void_kills += 1;
                };

                let weapon_name = data.weapon.as_ref().unwrap_or(&String::from("NONE")).to_owned();
                let weapon_kills = stats.weapon_kills.get(&weapon_name).unwrap_or(&0).to_owned();
                stats.weapon_kills.insert(weapon_name.clone(), weapon_kills + 1);
            };
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
            let gamemodes = if !current_match.is_tracking_stats() { vec![LevelGamemode::Arcade] } else { current_match.level.gamemodes.clone() };
            for gamemode in gamemodes {
                let mut default_gamemode_stats = GamemodeStats::default();
                let stats = context.gamemode_stats.get_mut(&gamemode).unwrap_or(&mut default_gamemode_stats);

                stats.deaths += 1;

                if data.cause == DamageCause::Void {
                    stats.void_deaths += 1;
                };

                if first_blood {
                    stats.first_bloods_suffered += 1;
                };

                if data.is_murder() {
                    let weapon_name = data.weapon.as_ref().unwrap_or(&String::from("NONE")).to_owned();
                    let weapon_deaths = stats.weapon_deaths.get(&weapon_name).unwrap_or(&0).to_owned();
                    stats.weapon_deaths.insert(weapon_name.clone(), weapon_deaths + 1);
                };
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
            let gamemodes = if !current_match.is_tracking_stats() { vec![LevelGamemode::Arcade] } else { current_match.level.gamemodes.clone() };
            for gamemode in gamemodes {
                let mut default_gamemode_stats = GamemodeStats::default();
                let stats = context.gamemode_stats.get_mut(&gamemode).unwrap_or(&mut default_gamemode_stats);

                let prev_amount = stats.killstreaks.get(&amount.to_string()).unwrap_or(&0).to_owned();
                stats.killstreaks.insert(amount.to_string(), prev_amount + 1);
            };
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
        let gamemodes = if !current_match.is_tracking_stats() { vec![LevelGamemode::Arcade] } else { current_match.level.gamemodes.clone() };
        for gamemode in gamemodes {
            let mut default_gamemode_stats = GamemodeStats::default();
            let stats = context.gamemode_stats.get_mut(&gamemode).unwrap_or(&mut default_gamemode_stats);

            stats.objectives.destroyable_block_destroys += block_count;
        };
    }

    async fn on_destroyable_destroy(
        &self, 
        _server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context, 
        _percentage: f32, 
        _block_count: u32
    ) {
        let gamemodes = if !current_match.is_tracking_stats() { vec![LevelGamemode::Arcade] } else { current_match.level.gamemodes.clone() };
        for gamemode in gamemodes {
            let mut default_gamemode_stats = GamemodeStats::default();
            let stats = context.gamemode_stats.get_mut(&gamemode).unwrap_or(&mut default_gamemode_stats);

            stats.objectives.destroyable_destroys += 1;
        };
    }

    async fn on_core_leak(
        &self,
        _server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context, 
        _percentage: f32,
        block_count: u32,
    ) {
        {
            let gamemodes = if !current_match.is_tracking_stats() { vec![LevelGamemode::Arcade] } else { current_match.level.gamemodes.clone() };
            for gamemode in gamemodes {
                let mut default_gamemode_stats = GamemodeStats::default();
                let stats = context.gamemode_stats.get_mut(&gamemode).unwrap_or(&mut default_gamemode_stats);

                stats.objectives.core_leaks += 1;
                stats.objectives.core_block_destroys += block_count;
            };
        };
    }

    async fn on_control_point_capture(
        &self,
        _server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context, 
        _contributors: u32
    ) {
        {
            let gamemodes = if !current_match.is_tracking_stats() { vec![LevelGamemode::Arcade] } else { current_match.level.gamemodes.clone() };
            for gamemode in gamemodes {
                let mut default_gamemode_stats = GamemodeStats::default();
                let stats = context.gamemode_stats.get_mut(&gamemode).unwrap_or(&mut default_gamemode_stats);

                stats.objectives.control_point_captures += 1;
            };
        };
    }

    async fn on_flag_place(
        &self,
        _server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context, 
        held_time: u64
    ) {
        {
            let gamemodes = if !current_match.is_tracking_stats() { vec![LevelGamemode::Arcade] } else { current_match.level.gamemodes.clone() };
            for gamemode in gamemodes {
                let mut default_gamemode_stats = GamemodeStats::default();
                let stats = context.gamemode_stats.get_mut(&gamemode).unwrap_or(&mut default_gamemode_stats);

                stats.objectives.flag_captures += 1;
                stats.objectives.total_flag_hold_time += held_time;
            };
        };
    }

    async fn on_flag_pickup(
        &self,
        _server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context
    ) {
        {
            let gamemodes = if !current_match.is_tracking_stats() { vec![LevelGamemode::Arcade] } else { current_match.level.gamemodes.clone() };
            for gamemode in gamemodes {
                let mut default_gamemode_stats = GamemodeStats::default();
                let stats = context.gamemode_stats.get_mut(&gamemode).unwrap_or(&mut default_gamemode_stats);

                stats.objectives.flag_pickups += 1;
            };
        };
    }

    async fn on_flag_drop(
        &self,
        _server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context, 
        held_time: u64
    ) {
        {
            let gamemodes = if !current_match.is_tracking_stats() { vec![LevelGamemode::Arcade] } else { current_match.level.gamemodes.clone() };
            for gamemode in gamemodes {
                let mut default_gamemode_stats = GamemodeStats::default();
                let stats = context.gamemode_stats.get_mut(&gamemode).unwrap_or(&mut default_gamemode_stats);

                stats.objectives.flag_drops += 1;
                stats.objectives.total_flag_hold_time += held_time;
            };
        };
    }

    async fn on_flag_defend(
        &self,
        _server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context
    ) {
        {
            let gamemodes = if !current_match.is_tracking_stats() { vec![LevelGamemode::Arcade] } else { current_match.level.gamemodes.clone() };
            for gamemode in gamemodes {
                let mut default_gamemode_stats = GamemodeStats::default();
                let stats = context.gamemode_stats.get_mut(&gamemode).unwrap_or(&mut default_gamemode_stats);

                stats.objectives.flag_defends += 1;
            };
        };
    }

    async fn on_wool_place(
        &self,
        _server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context,
        _held_time: u64
    ) {
        {
            let gamemodes = if !current_match.is_tracking_stats() { vec![LevelGamemode::Arcade] } else { current_match.level.gamemodes.clone() };
            for gamemode in gamemodes {
                let mut default_gamemode_stats = GamemodeStats::default();
                let stats = context.gamemode_stats.get_mut(&gamemode).unwrap_or(&mut default_gamemode_stats);

                stats.objectives.wool_captures += 1;
            };
        };
    }

    async fn on_wool_pickup(
        &self,
        _server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context
    ) {
        {
            let gamemodes = if !current_match.is_tracking_stats() { vec![LevelGamemode::Arcade] } else { current_match.level.gamemodes.clone() };
            for gamemode in gamemodes {
                let mut default_gamemode_stats = GamemodeStats::default();
                let stats = context.gamemode_stats.get_mut(&gamemode).unwrap_or(&mut default_gamemode_stats);

                stats.objectives.wool_pickups += 1;
            };
        };
    }

    async fn on_wool_drop(
        &self,
        _server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context,
        _held_time: u64
    ) {
        {
            let gamemodes = if !current_match.is_tracking_stats() { vec![LevelGamemode::Arcade] } else { current_match.level.gamemodes.clone() };
            for gamemode in gamemodes {
                let mut default_gamemode_stats = GamemodeStats::default();
                let stats = context.gamemode_stats.get_mut(&gamemode).unwrap_or(&mut default_gamemode_stats);

                stats.objectives.wool_drops += 1;
            };
        };
    }

    async fn on_wool_defend(
        &self,
        _server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context
    ) {
        {
            let gamemodes = if !current_match.is_tracking_stats() { vec![LevelGamemode::Arcade] } else { current_match.level.gamemodes.clone() };
            for gamemode in gamemodes {
                let mut default_gamemode_stats = GamemodeStats::default();
                let stats = context.gamemode_stats.get_mut(&gamemode).unwrap_or(&mut default_gamemode_stats);

                stats.objectives.wool_defends += 1;
            };
        };
    }

    async fn on_match_end_v2(
        &self,
        _server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context, 
        end_data: &mut MatchEndData
    ) { 
        {
            let gamemodes = if !current_match.is_tracking_stats() { vec![LevelGamemode::Arcade] } else { current_match.level.gamemodes.clone() };
            for gamemode in gamemodes {
                let mut default_gamemode_stats = GamemodeStats::default();
                let stats = context.gamemode_stats.get_mut(&gamemode).unwrap_or(&mut default_gamemode_stats);
                let big_stats = end_data.get_stats_for_participant(&context.id);
                for (block, freq) in big_stats.blocks.blocks_broken.iter() {
                    stats.blocks_broken.insert(block.to_owned(), freq.to_owned());
                };
                for (block, freq) in big_stats.blocks.blocks_placed.iter() {
                    stats.blocks_placed.insert(block.to_owned(), freq.to_owned());
                };

                stats.bow_shots_taken += big_stats.bow_shots_taken;
                stats.bow_shots_hit += big_stats.bow_shots_hit;
                stats.damage_given += big_stats.damage_given;
                stats.damage_taken += big_stats.damage_taken;
                stats.damage_given_bow += big_stats.damage_given_bow;

                let participant = current_match.get_participant(&context.id);

                let minimum_playtime = (0.10 * (current_match.get_length() as f64)).min(60_000.0);
                let is_playing = participant.party_name.is_some();

                let match_result = participant.get_match_result(&*current_match, end_data);
                let f64_game_playtime = participant.stats.game_playtime as f64;
                if f64_game_playtime > minimum_playtime {
                    match match_result {
                        PlayerMatchResult::Tie => { stats.ties += 1; },
                        PlayerMatchResult::Win => { stats.wins += 1; },
                        PlayerMatchResult::Lose => { stats.losses += 1; }
                        _ => {}
                    }
                };

                let time_elapsed_before_joining = (participant.first_joined_match_at - current_match.started_at.unwrap()).max(0);
                let present_at_start = (time_elapsed_before_joining as f64) < minimum_playtime;

                if f64_game_playtime > minimum_playtime { stats.matches += 1; }
                if present_at_start { stats.matches_present_start += 1; }
                if participant.stats.time_away < 20_000 && is_playing { stats.matches_present_full += 1; }
                if is_playing { stats.matches_present_end += 1; }

                stats.game_playtime += participant.stats.game_playtime;
            };
        };
    }
}
