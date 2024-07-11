use futures::future::join_all;
use mongodb::bson::doc;
use rocket::{Rocket, Build, State, http::Status, serde::json::Json};

use crate::{MarsAPIState, util::{auth::AuthorizationToken, error::ApiErrorResponder, time::get_u64_time_millis, r#macro::unwrap_helper, responder::JsonResponder}, database::{models::{r#match::Match, session::Session, player::Player, server::ServerEvents}, Database}, http::server::payloads::{ServerStatusResponse, XPMultiplierRequest}};

pub mod payloads;

#[post("/<server_id>/startup")]
async fn server_startup(
    state: &State<MarsAPIState>, 
    server_id: &str, 
    auth_guard: AuthorizationToken
) -> Result<(), ApiErrorResponder> {
    if server_id != auth_guard.server_id {
        return Err(ApiErrorResponder::unauthorized());
    };

    let last_alive_key = format!("server:{}:last_alive_time", server_id);
    let last_alive_time = state.redis.get_unchecked::<u64>(&last_alive_key).await;
    let time_millis : u64 = get_u64_time_millis();
    if last_alive_time.is_none() {
        state.redis.set(&last_alive_key, &time_millis).await;
        return Ok(());
    };

    let last_match_id = unwrap_helper::return_default!(
        state.redis.get_unchecked::<String>(&format!("server:{}:current_match_id", server_id)).await, 
        Ok(())
    );
    let current_match = state.redis.get_unchecked::<Match>(&format!("match:{}", last_match_id)).await;
    if current_match.is_some() {
        let mut current_match = current_match.unwrap();
        current_match.ended_at = Some(last_alive_time.unwrap());
        state.match_cache.set_with_expiry(&state.database, &current_match.id, &current_match, true, Some(3600000)).await;
    };

    let mut hanging_sessions = Database::consume_cursor_into_owning_vec_option(state.database.sessions.find(doc! {
        "serverId": server_id,
        "endedAt": null
    }, None).await.ok()).await;
    let mut sessions_to_write : Vec<Session> = Vec::new();
    let mut players_to_write : Vec<Player> = Vec::new();

    for hanging_session in hanging_sessions.iter_mut() {
        hanging_session.ended_at = Some(last_alive_time.unwrap());
        sessions_to_write.push(hanging_session.to_owned());

        let mut cached_player = unwrap_helper::continue_default!(state.player_cache.get(&state.database, &hanging_session.player.name).await);
        cached_player.stats.server_playtime += (hanging_session.length().unwrap_or(0) as i64);
        players_to_write.push(cached_player);
    }

    // unfortunately rust's mongo driver doesn't support bulk writes yet so that's sad
    { 
        let player_tasks : Vec<_> = players_to_write.iter().map(|player| {
            state.database.players.replace_one(doc! {
                "_id": &player.id
            }, player, None)
        }).collect();
        join_all(player_tasks).await;
        let session_tasks : Vec<_> = sessions_to_write.iter().map(|session| {
            state.database.sessions.replace_one(doc! {
                "_id": &session.id
            }, session, None)
        }).collect();
        join_all(session_tasks).await;
    }

    state.redis.set(&format!("server:{}:last_alive_time", server_id), &get_u64_time_millis()).await;

    info!("Saved {} players, {} sessions on startup '{}'", players_to_write.len(), sessions_to_write.len(), server_id);
    Ok(())
}


#[get("/<server_id>/status")]
async fn server_status(
    state: &State<MarsAPIState>, 
    server_id: &str
) -> Result<JsonResponder<ServerStatusResponse>, ApiErrorResponder> {
    let server_id = server_id.to_lowercase();
    let last_alive_time = unwrap_helper::return_default!(
        state.redis.get_unchecked::<u64>(&format!("server:{}:last_alive_time", server_id)).await, 
        Err(ApiErrorResponder::create_anonymous_error(Status::NotFound, "Last alive time unknown"))
    );
    let current_match_id = unwrap_helper::return_default!(
        state.redis.get_unchecked::<String>(&format!("server:{}:current_match_id", server_id)).await, 
        Err(ApiErrorResponder::create_anonymous_error(Status::NotFound, "No current match"))
    );
    let current_match = unwrap_helper::return_default!(
        state.redis.get_unchecked::<Match>(&format!("match:{}", current_match_id)).await, 
        Err(ApiErrorResponder::create_anonymous_error(Status::NotFound, "No current match"))
    );
    let tracking_stats = current_match.is_tracking_stats();
    Ok(JsonResponder::created(ServerStatusResponse { last_alive_time, current_match, stats_tracking: tracking_stats }))
}

#[get("/<server_id>/events")]
async fn server_events(
    state: &State<MarsAPIState>, 
    server_id: &str
) -> Result<JsonResponder<ServerEvents>, ApiErrorResponder> {
    let server_id = server_id.to_lowercase();
    let events : ServerEvents = state.redis.get_unchecked(&format!("server:{}:events", server_id)).await.unwrap_or(ServerEvents { 
        xp_multiplier: None  
    });
    Ok(JsonResponder::ok(events))
}

#[put("/<server_id>/events/xp_multiplier", format = "json", data = "<xp_multiplier_request>")]
async fn xp_multiplier_event(
    state: &State<MarsAPIState>, 
    server_id: &str,
    xp_multiplier_request: Json<XPMultiplierRequest>,
    auth_guard: AuthorizationToken
) -> Result<JsonResponder<ServerEvents>, ApiErrorResponder> {
    if server_id != auth_guard.server_id {
        return Err(ApiErrorResponder::unauthorized());
    };
    let mut events : ServerEvents = state.redis.get_unchecked(&format!("server:{}:events", server_id)).await.unwrap_or(ServerEvents { 
        xp_multiplier: None  
    });
    events.xp_multiplier = if xp_multiplier_request.value == 1f32 { None } else { Some(xp_multiplier_request.to_xp_multiplier()) };
    state.redis.set(&format!("server:{}:events", server_id), &events).await;
    Ok(JsonResponder::ok(events))
}

pub fn mount(rocket_build: Rocket<Build>) -> Rocket<Build> {
    rocket_build.mount("/mc/servers", routes![server_startup, server_status, server_events, xp_multiplier_event])
}
