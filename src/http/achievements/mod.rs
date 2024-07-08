use futures::future::join_all;
use mongodb::bson::doc;
use mongodb::results::DeleteResult;
use rocket::http::Status;
use rocket::{State, Rocket, Build, serde::json::Json};
use uuid::Uuid;

use crate::database::Database;
use crate::util::error::ApiErrorResponder;
use crate::util::r#macro::unwrap_helper;
use crate::util::responder::JsonResponder;
use crate::database::models::achievement::achievement_metadata_preset_to_metadata;
use crate::database::models::achievement::Achievement;
use crate::http::achievements::payload::AchievementCreateRequest;
use crate::MarsAPIState;
use crate::{util::auth::AuthorizationToken};

mod payload;

#[post("/", format = "json", data = "<achievement_create_req>")]
async fn add_achievement(
    state: &State<MarsAPIState>,
    achievement_create_req: Json<AchievementCreateRequest>,
    _auth_guard: AuthorizationToken
) -> Result<JsonResponder<Achievement>, ApiErrorResponder> {
    match state.database.find_by_id_or_name::<Achievement>(&achievement_create_req.name).await {
        Some(_tag) => return Err(ApiErrorResponder::achievement_conflict()),
        None => {},
    };

    let AchievementCreateRequest { name, description, metadata_preset, agent } = achievement_create_req.0;
    let metadata = match metadata_preset {
        Some(preset) => Some(achievement_metadata_preset_to_metadata(preset)),
        None => None,
    };

    let new_achievement = Achievement { 
        id: Uuid::new_v4().to_string(), 
        name,
        description, 
        metadata, 
        agent,
        first_completion: None
    };
    state.database.save::<Achievement>(&new_achievement).await;
    return Ok(JsonResponder::from(new_achievement, Status::Ok));
}

#[delete("/<achievement_id>")]
async fn delete_achievement(
    state: &State<MarsAPIState>,
    achievement_id: &str,
    _auth_guard: AuthorizationToken
) -> Result<(), ApiErrorResponder> {
    match state.database.delete_by_id::<Achievement>(achievement_id).await {
        Some(DeleteResult { deleted_count: 0, .. }) | None => {
            return Err(ApiErrorResponder::achievement_missing());
        },
        _ => {}
    };
    let players_with_achievement_query = doc! {format!("stats.achievements.{}", achievement_id): {"$exists": true}};
    let mut players_with_achievement = Database::consume_cursor_into_owning_vec_option(
        state.database.players.find(players_with_achievement_query.clone(), None).await.ok()
    ).await;
    state.database.players.update_many(
        players_with_achievement_query.clone(), 
        doc! {"$unset": {format!("stats.achievements.{}", achievement_id): ""}}, 
        None
    ).await;
    {
        let mut remove_from_cache_futures : Vec<_> = Vec::new();
        for player in players_with_achievement.iter_mut() {
            player.stats.achievements.remove(&achievement_id.to_owned());
            remove_from_cache_futures.push(state.player_cache.set(&state.database, &player.name, player, false));
        };
        join_all(remove_from_cache_futures).await;
    }
    Ok(())
}

#[get("/<achievement_id>")]
async fn get_achievement_by_id(
    state: &State<MarsAPIState>,
    achievement_id: &str
) -> Result<JsonResponder<Achievement>, ApiErrorResponder> {
    Ok(JsonResponder::ok(
        unwrap_helper::return_default!(
            state.database.find_by_id_or_name(achievement_id).await,
            Err(ApiErrorResponder::tag_missing())
        )
    ))
}

#[get("/")]
async fn get_achievements(
    state: &State<MarsAPIState>,
) -> Json<Vec<Achievement>> {
    Json(state.database.get_all_documents::<Achievement>().await)
}

pub fn mount(rocket_build: Rocket<Build>) -> Rocket<Build> {
    rocket_build.mount("/mc/achievements", routes![
       get_achievements,
       get_achievement_by_id,
       add_achievement,
       delete_achievement,
    ])
}
