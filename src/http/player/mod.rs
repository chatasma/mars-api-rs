mod payloads;

use futures::future::join_all;
use mongodb::bson::doc;
use payloads::PlayerPreLoginRequest;
use rocket::{serde::json::Json, Build, Rocket, State, http::Status};
use uuid::Uuid;
use crate::{util::{auth::AuthorizationToken, error::{ApiError, ApiErrorResponder}, string::to_utf8_byte_array, responder::{JsonResponder, EmptyResponse}, time::get_u64_time_millis, r#macro::unwrap_helper}, MarsAPIState, database::{Database, models::{punishment::{Punishment, PunishmentKind, StaffNote}, player::{Player, PlayerStats, SessionRecord}, session::Session, rank::Rank, tag::Tag}}, http::player::payloads::{PlayerLoginRequest, PlayerLookupResponse, PlayerAddNoteRequest, PlayerSetActiveTagRequest}, socket::leaderboard::{Leaderboard, ScoreType, LeaderboardPeriod}};
use sha2::{Sha256, Digest};

use self::payloads::{PlayerPreLoginResponse, PlayerPreLoginResponder, PlayerLoginResponse, PlayerLogoutRequest, PlayerProfileResponder, PlayerProfileResponse, PlayerAltResponse};
use std::{time::{SystemTime, UNIX_EPOCH}, collections::HashMap};
use crate::database::models::ip_identity::IpIdentity;

use super::punishment::payloads::PunishmentIssueRequest;

#[post("/<player_id>/prelogin", format = "json", data = "<prelogin_req>")]
pub async fn prelogin(
    state: &State<MarsAPIState>, 
    prelogin_req: Json<PlayerPreLoginRequest>, 
    player_id: &str, 
    _auth_guard: AuthorizationToken
) -> Result<PlayerPreLoginResponder, ApiErrorResponder> {
    let data = prelogin_req.0;

    if data.player.id != player_id {
        return Err(ApiErrorResponder::validation_error());
    };

    let ip = hash_ip(&state, &data.ip);
    let player_optional = Database::find_by_id(&state.database.players, &data.player.id).await;
    if let Some(mut returning_player) = player_optional {
        returning_player.name = data.player.name.clone();
        returning_player.name_lower = returning_player.name.to_lowercase();
        let new_ip = !returning_player.ips.contains(&ip);
        if new_ip {
            returning_player.ips.push(ip.clone());
        };

        let mut puns : Vec<Punishment> = state.database.get_active_player_punishments(&returning_player).await;
        let ban_pun_optional = puns.iter().find(|pun| pun.action.is_ban());
        let banned = {
            let is_player_banned = ban_pun_optional.is_some();
            if is_player_banned {
                true
            } else {
                let mut ip_punishments : Vec<Punishment> = match state.database.punishments.find(doc! {
                    "targetIps": &ip,
                    "action.kind": PunishmentKind::IpBan.to_string()
                }, None).await {
                    Ok(cursor) => Database::consume_cursor_into_owning_vec(cursor).await,
                    Err(_) => return Err(ApiErrorResponder::validation_error_with_message("Could not load punishments for player"))
                };
                ip_punishments.retain(|p| p.is_active());
                let is_ip_banned = !ip_punishments.is_empty();
                if is_ip_banned {
                    // move ip puns into main punishment vector
                    puns.append(&mut ip_punishments);
                }
                is_ip_banned
            }
        };
        state.player_cache.set(&state.database, &returning_player.name, &returning_player, true).await;
        state.database.ensure_player_name_uniqueness(&data.player.name, &data.player.id).await;
        // denormalize ip player relationship
        if new_ip {
            IpIdentity::add_player_ip(&state.database, &ip,  &returning_player.id).await;
        }

        Ok(PlayerPreLoginResponder { 
            response: PlayerPreLoginResponse {
                new: false, 
                allowed: !banned,
                player: returning_player, 
                active_punishments: puns 
            }
        })
    } else {
        debug!("Could not find player {} in database!", player_id);
        let time_millis : f64 = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as f64;
        let player = Player {
            id: data.player.id.clone(),
            name: data.player.name.clone(),
            name_lower: data.player.name.to_lowercase(),
            ips: vec![ip.clone()],
            first_joined_at: time_millis,
            last_joined_at: time_millis,
            rank_ids: Vec::new(),
            tag_ids: Vec::new(),
            active_tag_id: None,
            stats: PlayerStats::default(),
            gamemode_stats: HashMap::new(),
            notes: Vec::new(),
            last_session_id: None,
            active_join_sound_id: None
        };

        state.player_cache.set(&state.database, &player.name, &player, true).await;
        state.database.ensure_player_name_uniqueness(&data.player.name, &data.player.id).await;
        IpIdentity::add_player_ip(&state.database, &ip, &data.player.id).await;

        Ok(PlayerPreLoginResponder {
            response: PlayerPreLoginResponse {
                new: true,
                allowed: true,
                player,
                active_punishments: Vec::new()
            }
        })
    }
}

macro_rules! extract_player_from_url {
    ( $e:expr, $s:expr ) => {
        if let Some(player) = ($s).player_cache.get(&($s).database, ($e)).await { player } 
        else { return Err(ApiError::missing_player(($e))) }
    }
}

macro_rules! async_extract_player_from_url_v2 {
    ( $e:expr, $s:expr ) => {
        if let Some(player) = ($s).player_cache.get(&($s).database, ($e)).await { player } 
        else { return Err(ApiErrorResponder::missing_player()) }
    }
}


#[post("/<player_id>/login", format = "json", data = "<login_req>")]
pub async fn login(
    state: &State<MarsAPIState>, 
    login_req: Json<PlayerLoginRequest>, 
    player_id: &str, 
    auth_guard: AuthorizationToken
) -> Result<JsonResponder<PlayerLoginResponse>, ApiErrorResponder> {
    let data = login_req.0;
    let mut player : Player = async_extract_player_from_url_v2!(&data.player.name, state);

    if player_id != player.id || player.id != data.player.id { 
        return Err(ApiErrorResponder::validation_error());
    };

    if data.player.id != player_id {
        return Err(ApiErrorResponder::validation_error());
    };

    let time_millis : u64 = u64::try_from(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis()).unwrap_or(u64::MAX);
    let ip = hash_ip(&state, &data.ip);
    let active_session = Session {
        id: Uuid::new_v4().to_string(),
        player: player.to_simple(),
        ip: ip.clone(),
        server_id: auth_guard.server_id,
        created_at: time_millis,
        ended_at: None
    };

    state.database.save(&active_session).await;
    let mut player_ranks = player.rank_ids.clone();
    let mut default_ranks : Vec<String> = Rank::find_default(&state.database).await
        .iter()
        .map(|rank| { rank.id.clone() })
        .collect();
    player_ranks.append(&mut default_ranks);
    player_ranks.dedup();

    player.last_joined_at = time_millis as f64;
    player.last_session_id = Some(active_session.id.clone());

    state.player_cache.set(&state.database, &player.name, &player, true).await;

    Ok(JsonResponder::from(PlayerLoginResponse { active_session }, Status::Created))
}


#[post("/logout", format = "json", data = "<logout_req>")]
pub async fn logout(
    state: &State<MarsAPIState>, 
    logout_req: Json<PlayerLogoutRequest>, 
    _auth_guard: AuthorizationToken
) -> Result<JsonResponder<EmptyResponse>, ApiErrorResponder> {
    let data = logout_req.0;
    let mut player : Player = async_extract_player_from_url_v2!(&data.player.name, state);
    let mut session = if let Some(session) = state.database.find_session_for_player(&player, data.session_id).await {
        session
    } else {
        return Err(ApiErrorResponder::session_not_found())
    };
    if !session.is_active() {
        return Err(ApiErrorResponder::session_inactive())
    };

    let time_millis : u64 = u64::try_from(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis()).unwrap_or(u64::MAX);
    session.ended_at = Some(time_millis);
    player.stats.server_playtime += (data.playtime as i64);

    state.leaderboards.server_playtime.increment(&player.id_name(), Some(u32::try_from(data.playtime).unwrap_or(u32::MAX))).await; // Will break in 2106

    let record_session = if let Some(session_record) = &player.stats.records.longest_session {
        Some(session_record.length.clone())
    } else {
        None
    };
    if record_session.is_none() || data.playtime > record_session.unwrap() {
        player.stats.records.longest_session = Some(SessionRecord { session_id: session.id.clone(), length: data.playtime.clone() });
    };

    state.database.save(&session).await;
    state.player_cache.set(&state.database, &player.name, &player, true).await;

    Ok(JsonResponder::ok(EmptyResponse {}))
}


#[get("/<player_id>?<include_leaderboard_positions>")]
pub async fn profile(
    state: &State<MarsAPIState>, 
    player_id: &str,
    include_leaderboard_positions: bool
) -> Result<PlayerProfileResponder, ApiErrorResponder> {
    let player_id = player_id.to_lowercase();
    let player : Player = async_extract_player_from_url_v2!(&player_id, state);
    let profile = player.sanitized_copy();
    if !include_leaderboard_positions {
        return Ok(PlayerProfileResponder::RawProfile(profile))
    };
    // omitted: messages sent, server + game playtime
    let included_lbs : Vec<&Leaderboard> = vec![
        &state.leaderboards.kills, 
        &state.leaderboards.deaths,
        &state.leaderboards.first_bloods,
        &state.leaderboards.wins,
        &state.leaderboards.losses,
        &state.leaderboards.ties,
        &state.leaderboards.xp,
        &state.leaderboards.matches_played,
        &state.leaderboards.core_leaks,
        &state.leaderboards.core_block_destroys,
        &state.leaderboards.destroyable_destroys,
        &state.leaderboards.destroyable_block_destroys,
        &state.leaderboards.flag_captures,
        &state.leaderboards.flag_pickups,
        &state.leaderboards.flag_drops,
        &state.leaderboards.flag_defends,
        &state.leaderboards.flag_hold_time,
        &state.leaderboards.wool_captures,
        &state.leaderboards.wool_pickups,
        &state.leaderboards.wool_drops,
        &state.leaderboards.wool_defends,
        &state.leaderboards.control_point_captures,
        &state.leaderboards.highest_killstreak
    ];
    let mut positions : HashMap<ScoreType, u64> = HashMap::new();
    let mut lb_position_tasks : Vec<_> = Vec::new();
    for lb in included_lbs.iter() {

        // move owned id string into closure
        let wrapper = |player_id: String| async move {
            (lb.score_type.clone(), lb.get_position(&player_id, &LeaderboardPeriod::AllTime).await)
        };

        lb_position_tasks.push(wrapper(player.id_name()));
    }

    join_all(lb_position_tasks).await.into_iter().filter(|pos_opt| pos_opt.1.is_some()).for_each(|pos| {
        positions.insert(pos.0, pos.1.unwrap());
    });
    Ok(PlayerProfileResponder::ProfileWithLeaderboardPositions(PlayerProfileResponse {
        player: profile,
        leaderboard_positions: positions
    }))
}


// why isn't the url parameter used?
#[post("/<_player_id>/punishments", format = "json", data = "<pun_issue_req>")]
pub async fn issue_punishment(
    state: &State<MarsAPIState>, 
    pun_issue_req: Json<PunishmentIssueRequest>,
    _player_id: &str,
    auth_guard: AuthorizationToken
) -> Result<JsonResponder<Punishment>, ApiErrorResponder> {
    let data = pun_issue_req.0;
    let punishment_id = Uuid::new_v4().to_string();
    let time_millis : u64 = u64::try_from(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis()).unwrap_or(u64::MAX);
    let target_player : Player = async_extract_player_from_url_v2!(&data.target_name, state);
    let punishment = Punishment { 
        id: punishment_id, 
        reason: data.reason, 
        issued_at: time_millis as f64, 
        silent: data.silent, 
        offence: data.offence, 
        action: data.action, 
        note: data.note, 
        punisher: data.punisher, 
        target: target_player.to_simple(), 
        target_ips: data.target_ips, 
        reversion: None, 
        server_id: Some(auth_guard.server_id)
    };
    state.database.insert_one(&punishment).await;
    {
        // take ownership for the spawned task
        let pun_clone = punishment.clone();
        let state_clone = state.config.clone();
        tokio::spawn(async move {
            state_clone.webhooks.send_punishment_webhook(&pun_clone).await;
        });
    }
    Ok(JsonResponder::from(punishment, Status::Created))
}


#[get("/<player_id>/punishments")]
pub async fn get_punishments(
    state: &State<MarsAPIState>, 
    player_id: &str,
    _auth_guard: AuthorizationToken
) -> Result<JsonResponder<Vec<Punishment>>, ApiErrorResponder> {
    let player : Player = async_extract_player_from_url_v2!(&player_id, state);
    Ok(JsonResponder::created(state.database.get_player_punishments(&player).await))
}

pub fn hash_ip(state: &MarsAPIState, digest: &String) -> String {
    if state.config.options.enable_ip_hashing { sha256_hash_formatted(digest) } 
    else { digest.clone() }
}

pub fn sha256_hash_formatted(digest: &String) -> String {
    let mut hasher = Sha256::new();
    hasher.update(to_utf8_byte_array(digest));
    hasher.finalize().iter().fold(String::from(""), |mut hex_str, elem| {
        let formatted = format!("{:02x}", elem);
        hex_str.push_str(&formatted);
        hex_str
    })
}

#[get("/<player_id>/lookup?<alts>")]
pub async fn lookup_player(
    state: &State<MarsAPIState>, 
    player_id: &str,
    alts: bool,
    _auth_guard: AuthorizationToken
) -> Result<JsonResponder<PlayerLookupResponse>, ApiErrorResponder> {
    let player : Player = async_extract_player_from_url_v2!(&player_id, state);
    let player_alts : Vec<PlayerAltResponse> = {
        let mut player_alts : Vec<PlayerAltResponse> = Vec::new();
        if alts {
            let t1 = get_u64_time_millis();
            let fetched_alts = state.database.get_alts_for_player(&player).await;
            let t2 = get_u64_time_millis();
            debug!("Alt lookup for {} took {}ms", &player.name, (t2 - t1));
            let pun_tasks : Vec<_> = fetched_alts.iter().map(|alt| {
                state.database.get_player_punishments(alt)
            }).collect();
            let alt_puns = join_all(pun_tasks).await;
            for (alt, puns) in fetched_alts.into_iter().zip(alt_puns) {
                player_alts.push(PlayerAltResponse { player: alt, punishments: puns });
            }
        };
        player_alts
    };
    Ok(JsonResponder::created(PlayerLookupResponse { player, alts: player_alts }))
}

#[post("/<player_id>/notes", format = "json", data = "<add_note_req>")]
pub async fn add_player_note(
    state: &State<MarsAPIState>, 
    player_id: &str,
    add_note_req: Json<PlayerAddNoteRequest>,
    _auth_guard: AuthorizationToken
) -> Result<JsonResponder<Player>, ApiErrorResponder> {
    let data = add_note_req.0;
    let mut player : Player = async_extract_player_from_url_v2!(&player_id, state);
    let id = player.notes.iter().max_by_key(|note| note.id).map(|note| note.id).unwrap_or(0) + 1;
    let note = StaffNote { id, author: data.author, content: data.content, created_at: get_u64_time_millis() };
    let note_clone = note.clone();
    player.notes.push(note);
    state.player_cache.set(&state.database, player_id, &player, true).await;
    {
        // take ownership for the spawned task
        let state_clone = state.config.clone();
        let player_simple = player.to_simple();
        tokio::spawn(async move {
            state_clone.webhooks.send_new_note_webhook(&player_simple, &note_clone).await;
        });
    }
    Ok(JsonResponder::created(player))
}

#[delete("/<player_id>/notes/<note_id>")]
pub async fn delete_player_note(
    state: &State<MarsAPIState>, 
    player_id: &str,
    note_id: u32,
    _auth_guard: AuthorizationToken
) -> Result<JsonResponder<Player>, ApiErrorResponder> {
    let mut player : Player = async_extract_player_from_url_v2!(&player_id, state);
    let note_index = unwrap_helper::return_default!(player.notes.iter().position(|note| { note.id == note_id }), Err(ApiErrorResponder::note_missing()));
    let note_clone = player.notes[note_index].clone();
    player.notes.remove(note_index);
    state.player_cache.set(&state.database, player_id, &player, true).await;
    {
        // take ownership for the spawned task
        let state_clone = state.config.clone();
        let player_simple = player.to_simple();
        tokio::spawn(async move {
            state_clone.webhooks.send_deleted_note_webhook(&player_simple, &note_clone).await;
        });
    }
    Ok(JsonResponder::created(player))
}

#[put("/<player_id>/active_tag", format = "json", data = "<tag_set_req>")]
async fn set_active_tag(
    state: &State<MarsAPIState>, 
    player_id: &str, 
    tag_set_req: Json<PlayerSetActiveTagRequest>,
    _auth_guard: AuthorizationToken
) -> Result<JsonResponder<Player>, ApiErrorResponder> {
    let tag_id = tag_set_req.active_tag_id.clone();
    let mut player = async_extract_player_from_url_v2!(player_id, state);

    if tag_id == player.active_tag_id {
        return Ok(JsonResponder::from(player, Status::Ok));
    }

    if tag_id.is_none() {
        player.active_tag_id = Option::None;
    } else {
        if !player.tag_ids.contains(tag_id.as_ref().unwrap()) {
            return Err(ApiErrorResponder::tag_missing_from_player());
        }
        player.active_tag_id = tag_id;
    }

    state.player_cache.set(&state.database, &player.name, &player, true).await;
    return Ok(JsonResponder::from(player, Status::Ok));
}

#[put("/<player_id>/tags/<tag_id>")]
async fn add_tag_to_player(
    state: &State<MarsAPIState>, 
    player_id: &str, 
    tag_id: &str, 
    _auth_guard: AuthorizationToken
) -> Result<JsonResponder<Player>, ApiErrorResponder> {
    let mut player = async_extract_player_from_url_v2!(player_id, state);

    let tag = match state.database.find_by_id_or_name::<Tag>(tag_id).await {
        Some(tag) => tag,
        None => return Err(ApiErrorResponder::tag_missing())
    };

    if player.tag_ids.contains(&tag.id) {
        return Err(ApiErrorResponder::tag_already_present());
    }

    player.tag_ids.push(tag.id.clone());
    state.player_cache.set(&state.database, &player.name, &player, true).await;
    return Ok(JsonResponder::from(player, Status::Ok));
}


#[delete("/<player_id>/tags/<tag_id>")]
async fn delete_player_tag(
    state: &State<MarsAPIState>,
    player_id: &str,
    tag_id: &str,
    _auth_guard: AuthorizationToken
) -> Result<JsonResponder<Player>, ApiErrorResponder> {
    let mut player = async_extract_player_from_url_v2!(player_id, state);
    let tag = match state.database.find_by_id_or_name::<Tag>(tag_id).await {
        Some(tag) => tag,
        None => return Err(ApiErrorResponder::tag_missing())
    };
    match player.tag_ids.iter().position(|itag| { itag == &tag.id }) {
        Some(tag_index) => player.tag_ids.swap_remove(tag_index),
        None => return Err(ApiErrorResponder::tag_missing_from_player())
    };
    if player.active_tag_id.is_some() && player.active_tag_id.as_ref().unwrap() == &tag.id {
        player.active_tag_id = Option::None;
    }
    state.player_cache.set(&state.database, &player.name, &player, true).await;
    return Ok(JsonResponder::from(player, Status::Ok));

}

#[put("/<player_id>/ranks/<rank_id>")]
async fn add_player_rank(
    state: &State<MarsAPIState>, 
    player_id: &str, 
    rank_id: &str, 
    _auth_guard: AuthorizationToken
) -> Result<Json<Player>, ApiErrorResponder> {
    let mut player = unwrap_helper::return_default!(state.player_cache.get(&state.database, player_id).await, Err(ApiErrorResponder::missing_player()));
    let rank = unwrap_helper::return_default!(state.database.find_by_id_or_name::<Rank>(rank_id).await, Err(ApiErrorResponder::missing_rank()));

    if player.rank_ids.contains(&rank.id) { return Err(ApiErrorResponder::rank_already_present()); };
    player.rank_ids.push(rank.id);

    state.player_cache.set(&state.database, &player.name, &player, true).await;
    Ok(Json(player))
}

#[delete("/<player_id>/ranks/<rank_id>")]
async fn delete_player_rank(
    state: &State<MarsAPIState>, 
    player_id: &str, 
    rank_id: &str, 
    _auth_guard: AuthorizationToken
) -> Result<Json<Player>, ApiErrorResponder> {
    let mut player = unwrap_helper::return_default!(state.player_cache.get(&state.database, player_id).await, Err(ApiErrorResponder::missing_player()));
    let rank = unwrap_helper::return_default!(state.database.find_by_id_or_name::<Rank>(rank_id).await, Err(ApiErrorResponder::missing_rank()));

    if !player.rank_ids.contains(&rank.id) { return Err(ApiErrorResponder::rank_not_present()); };
    player.rank_ids.retain(|rank_id| { rank_id != rank.id.as_str() });

    state.player_cache.set(&state.database, &player.name, &player, true).await;
    Ok(Json(player))
}

pub fn mount(rocket_build: Rocket<Build>) -> Rocket<Build> {
    rocket_build.mount("/mc/players", routes![
        prelogin, 
        login, 
        logout, 
        profile, 
        issue_punishment, 
        get_punishments,
        lookup_player,
        add_player_note,
        delete_player_note,
        set_active_tag,
        add_tag_to_player,
        delete_player_tag,
        add_player_rank,
        delete_player_rank
    ])
}
