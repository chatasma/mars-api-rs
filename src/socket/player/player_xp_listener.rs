use crate::{socket::{server::server_context::ServerContext, r#match::match_events::MatchEndData, participant::participant_context::PlayerMatchResult}, database::models::{player::Player, r#match::{Match, DestroyableGoal}}};

use super::{player_listener::PlayerListener, player_events::PlayerDeathData};

pub struct PlayerXPListener {}

pub static XP_BEGINNER_ASSIST_MAX : u32 = 10;

pub static XP_WIN : u32 = 200;
pub static XP_LOSS : u32 = 100;
pub static XP_DRAW : u32 = 150;
pub static XP_KILL : u32 = 40;
pub static XP_DEATH : u32 = 1;
pub static XP_FIRST_BLOOD : u32 = 7;
pub static XP_WOOL_OBJECTIVE : u32 = 100;
pub static XP_FLAG_OBJECTIVE : u32 = 50;
pub static XP_FLAG_TIME_BOUNS : u32 = 100;
pub static XP_POINT_CAPTURE_MAX : u32 = 40;
pub static XP_DESTROYABLE_WHOLE : u32 = 200;
pub static XP_KILLSTREAK_COEFFICIENT : u32 = 10;

impl PlayerXPListener {
    pub fn gain(xp: u32, level: u32) -> u32 {
        let start_multiplier = if level > XP_BEGINNER_ASSIST_MAX {
            1
        } else {
            u32::max(XP_BEGINNER_ASSIST_MAX.saturating_sub(level), 1)
        };
        xp * start_multiplier
    }
}

#[async_trait]
impl PlayerListener for PlayerXPListener {
    type Context = Player;

    async fn on_kill(
        &self, 
        server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context, 
        _data: &mut PlayerDeathData, 
        first_blood: bool
    ) { 
        context.add_xp(server_context, XP_KILL, &String::from("Kill"), true, false).await;
        if first_blood { context.add_xp(server_context, XP_FIRST_BLOOD, &String::from("First blood"), true, false).await;  };
    }

    async fn on_death(
        &self,
        server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context, 
        _data: &mut PlayerDeathData, 
        _first_blood: bool
    ) { 
        context.add_xp(server_context, XP_DEATH, &String::from("Death"), false, false).await;
    }

    async fn on_killstreak(
        &self,
        server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context, 
        amount: u32
    ) {
        context.add_xp(server_context, XP_KILLSTREAK_COEFFICIENT * amount, &format!("Killstreak x{}", amount.to_string()), true, false).await;
    }

    async fn on_destroyable_damage(
        &self, 
        server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context, 
        destroyable: &DestroyableGoal, 
        block_count: u32
    ) {
        let xp = (XP_DESTROYABLE_WHOLE / destroyable.breaks_required) * block_count;
        context.add_xp(server_context, xp, &String::from("Damaged objective"), true, false).await;
    }

    async fn on_wool_place(
        &self,
        server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context, 
        _held_time: u64
    ) { 
        context.add_xp(server_context, XP_WOOL_OBJECTIVE, &String::from("Captured wool"), true, false).await;
    }

    async fn on_wool_pickup(
        &self,
        server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context
    ) { 
        context.add_xp(server_context, XP_WOOL_OBJECTIVE, &String::from("Picked up wool"), true, false).await;
    }

    async fn on_wool_defend(
        &self,
        server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context
    ) { 
        context.add_xp(server_context, XP_WOOL_OBJECTIVE, &String::from("Defended wool"), true, false).await;
    }

    async fn on_flag_place(
        &self,
        server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context,
        held_time: u64
    ) { 
        let xp = XP_FLAG_OBJECTIVE + (XP_FLAG_TIME_BOUNS.saturating_sub((held_time / 1000) as u32));
        context.add_xp(server_context, xp, &String::from("Captured flag"), true, false).await;
    }

    async fn on_flag_pickup(
        &self,
        server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context
    ) { 
        context.add_xp(server_context, XP_FLAG_OBJECTIVE, &String::from("Picked up flag"), true, false).await;
    }

    async fn on_flag_defend(
        &self,
        server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context
    ) { 
        context.add_xp(server_context, XP_FLAG_OBJECTIVE, &String::from("Defended flag"), true, false).await;
    }

    async fn on_control_point_capture(
        &self, 
        server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context, 
        contributors: u32, 
    ) {
        let others = contributors + 1;
        let xp = u32::max(XP_POINT_CAPTURE_MAX.saturating_sub(others * 10), 20);
        context.add_xp(server_context, xp, &String::from("Captured point"), true, false).await;
    }

    async fn on_core_leak(
        &self, 
        server_context: &mut ServerContext, 
        _current_match: &mut Match, 
        context: &mut Self::Context, 
        percentage: f32, 
        _block_count: u32,
    ) {
        let xp : f32 = percentage * (XP_DESTROYABLE_WHOLE as f32);
        context.add_xp(server_context, xp as u32, &String::from("Leaked core"), true, false).await;
    }

    async fn on_match_end_v2(
        &self,
        server_context: &mut ServerContext, 
        current_match: &mut Match, 
        context: &mut Self::Context, 
        end_data: &mut MatchEndData
    ) { 
        {
            let minimum_playtime = (0.10 * (current_match.get_length() as f64)).min(60_000.0);

            let eligible_for_result_xp = current_match.participants.values().filter(
                |participant| participant.id == context.id && (participant.stats.game_playtime as f64) > minimum_playtime
            ).next();

            if eligible_for_result_xp.is_none() {
                return;
            };

            let participant = eligible_for_result_xp.unwrap();
            let match_result = current_match.get_participant_match_result(participant, end_data);

            match match_result {
                PlayerMatchResult::Win => {
                    context.add_xp(server_context, XP_WIN, &String::from("Victory"), true, true).await;
                },
                PlayerMatchResult::Lose => {
                    context.add_xp(server_context, XP_LOSS, &String::from("Defeat"), true, true).await;
                },
                PlayerMatchResult::Tie => {
                    context.add_xp(server_context, XP_DRAW, &String::from("Tie"), true, true).await;
                },
                _ => {}
            };
        };
    }
}
