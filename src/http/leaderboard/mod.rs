use std::str::FromStr;

use rocket::{Rocket, Build, State, serde::json::Json};

use crate::{MarsAPIState, socket::leaderboard::{ScoreType, LeaderboardEntry, LeaderboardPeriod}, util::{r#macro::unwrap_helper, error::ApiErrorResponder}};
use crate::util::string::enumify;

const PUBLIC_SCORE_TYPES : &[ScoreType] = &[
    ScoreType::Kills,
    ScoreType::Deaths,
    ScoreType::FirstBloods,
    ScoreType::Wins,
    ScoreType::Losses,
    ScoreType::Ties,
    ScoreType::Xp,
    ScoreType::CoreLeaks,
    ScoreType::CoreBlockDestroys,
    ScoreType::DestroyableDestroys,
    ScoreType::DestroyableBlockDestroys,
    ScoreType::FlagCaptures,
    ScoreType::FlagDrops,
    ScoreType::FlagPickups,
    ScoreType::FlagDefends,
    ScoreType::FlagHoldTime,
    ScoreType::WoolCaptures,
    ScoreType::WoolDrops,
    ScoreType::WoolPickups,
    ScoreType::WoolDefends,
    ScoreType::ControlPointCaptures,
    ScoreType::HighestKillstreak
];

#[get("/<score_type>/<period>?<limit>")]
async fn get_leaderboard_entries(
    state: &State<MarsAPIState>, 
    score_type: &str, 
    period: &str, 
    limit: Option<u32>
) -> Result<Json<Vec<LeaderboardEntry>>, ApiErrorResponder> {
    let score_type = unwrap_helper::return_default!(ScoreType::from_str(enumify(score_type).as_str()).ok(), Err(ApiErrorResponder::validation_error()));
    if !PUBLIC_SCORE_TYPES.contains(&score_type) {
        return Err(ApiErrorResponder::unauthorized());
    };
    let period = unwrap_helper::return_default!(LeaderboardPeriod::from_str(enumify(period).as_str()).ok(), Err(ApiErrorResponder::validation_error()));
    let limit = limit.unwrap_or(10);
    let leaderboard = score_type.to_leaderboard(&state.leaderboards).fetch_top(&period, if limit > 50 { 50 } else { limit }).await;
    Ok(Json(leaderboard))
}

pub fn mount(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket.mount("/mc/leaderboards", routes![get_leaderboard_entries])
}
