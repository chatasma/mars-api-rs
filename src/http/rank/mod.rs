use std::vec;

use futures::future::join_all;
use mongodb::bson::doc;
use rocket::{Build, Rocket, serde::json::Json, State};
use uuid::Uuid;

use crate::{database::{Database, models::{player::Player, rank::Rank}}, http::rank::payload::RankCreateRequest, MarsAPIState, util::{auth::AuthorizationToken, error::ApiErrorResponder, r#macro::unwrap_helper, time::get_u64_time_millis}};
use crate::database::models::player::SimplePlayer;

use self::payload::RankUpdateRequest;

mod payload;

#[post("/", format = "json", data = "<create_req>")]
async fn create_rank(
    state: &State<MarsAPIState>, 
    create_req: Json<RankCreateRequest>,
    _auth_guard: AuthorizationToken
) -> Result<Json<Rank>, ApiErrorResponder> {
    let data = create_req.0;
    let conflict = state.database.find_by_name::<Rank>(&data.name).await;
    if let Some(_) = conflict {
        return Err(ApiErrorResponder::rank_confict());
    };

    let lowercase_name = data.name.to_lowercase();
    let mut perms = data.permissions;
    perms.dedup();
    let rank = Rank { 
        id: Uuid::new_v4().to_string(), 
        name: data.name, 
        name_lower: lowercase_name, 
        display_name: data.display_name, 
        prefix: data.prefix, 
        priority: data.priority, 
        permissions: perms, 
        staff: data.staff, 
        apply_on_join: data.apply_on_join, 
        created_at: get_u64_time_millis() as f64 
    };

    state.database.save(&rank).await;

    Ok(Json(rank))
}

#[get("/")]
async fn get_ranks(state: &State<MarsAPIState>) -> Json<Vec<Rank>> {
    Json(state.database.get_all_documents::<Rank>().await)
}

#[get("/<rank_id>")]
async fn get_rank_by_id(state: &State<MarsAPIState>, rank_id: &str) -> Result<Json<Rank>, ApiErrorResponder> {
    let rank = unwrap_helper::return_default!(Database::find_by_id(&state.database.ranks, rank_id).await, Err(ApiErrorResponder::missing_rank()));
    Ok(Json(rank))
}

#[get("/<rank_id>/players")]
async fn get_players_by_rank_by_id(state: &State<MarsAPIState>, rank_id: &str) -> Result<Json<Vec<SimplePlayer>>, ApiErrorResponder> {
    let rank = unwrap_helper::return_default!(Database::find_by_id(&state.database.ranks, rank_id).await, Err(ApiErrorResponder::missing_rank()));
    if rank.apply_on_join || !rank.staff {
        return Ok(Json(vec![]));
    };
    let players = Database::get_players_by_rank(&state.database, &rank).await;
    Ok(Json(players))
}


#[delete("/<rank_id>")]
async fn delete_rank(state: &State<MarsAPIState>, rank_id: &str, _auth_guard: AuthorizationToken) -> Result<(), ApiErrorResponder> {
    let delete_count = match state.database.delete_by_id::<Rank>(rank_id).await {
        Some(delete_result) => delete_result.deleted_count,
        None => 0
    };
    if delete_count == 0 {
        return Err(ApiErrorResponder::missing_rank());
    };

    // we love loading every player into memory
    let mut players_with_rank = Database::consume_cursor_into_owning_vec_option(state.database.players.find(doc! {"rankIds": rank_id}, None).await.ok()).await;
    // compute this early before vector swap remove
    let formatted_player_names = players_with_rank.iter().map(|player| { format!("{} ({})", player.id, player.name) }).collect::<Vec<String>>().join(",");

    let mut cache_updates : Vec<_> = Vec::new();
    for i in 0..players_with_rank.len() {
        let mut player = players_with_rank.swap_remove(i); // move out of vector
        player.rank_ids.retain(|existing_rank_id| existing_rank_id != rank_id);
        // move player into closure, then move closure into vector
        let wrapper = |player: Player| async move {
            state.player_cache.set(&state.database, &player.name, &player, true).await;
        };
        cache_updates.push(wrapper(player));
    }
    join_all(cache_updates).await;

    info!("Rank '{}' was deleted. Affected players: {}", rank_id, formatted_player_names);
    Ok(())
}

#[put("/<rank_id>", format = "json", data = "<rank_update_req>")]
async fn update_rank(
    state: &State<MarsAPIState>, 
    rank_update_req: Json<RankUpdateRequest>, 
    rank_id: &str, 
    _auth_guard: AuthorizationToken
) -> Result<Json<Rank>, ApiErrorResponder> {
    let data = rank_update_req.0;
    let existing_rank = unwrap_helper::return_default!(Database::find_by_id(&state.database.ranks, rank_id).await, Err(ApiErrorResponder::missing_rank()));
    let conflict_rank = state.database.ranks.find_one(doc! {"_id": {"$ne": &existing_rank.id}, "nameLower": data.name.to_lowercase()}, None).await.ok().unwrap_or(None);
    if conflict_rank.is_some() {
        return Err(ApiErrorResponder::rank_confict());
    };

    let rank_lower_name = data.name.to_lowercase();
    let mut perms = data.permissions;
    perms.dedup();
    let updated_rank = Rank { 
        id: existing_rank.id, 
        name: data.name, 
        name_lower: rank_lower_name,
        display_name: data.display_name, 
        prefix: data.prefix, 
        priority: data.priority, 
        permissions: perms, 
        staff: data.staff, 
        apply_on_join: data.apply_on_join, 
        created_at: existing_rank.created_at
    };

    state.database.save(&updated_rank).await;
    Ok(Json(updated_rank))
}

pub fn mount(rocket: Rocket<Build>) -> Rocket<Build>  {
    rocket.mount("/mc/ranks", routes![create_rank, get_ranks, get_rank_by_id, get_players_by_rank_by_id, delete_rank, update_rank])
}
