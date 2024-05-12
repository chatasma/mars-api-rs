use serde::{Serialize, Deserialize};
use rocket::{response::{self, Response, Responder}, Request, http::{Status, ContentType}, serde::json::Json};
use strum_macros::Display;

pub struct ApiErrorResponder {
    pub status: Status,
    pub error: ApiErrorV2
}

impl ApiErrorResponder {
    pub fn create_anonymous_error(status: Status, message: &str) -> Self {
        Self::create_api_error_responder(status, &ApiExceptionType::Anonymous, message)
    }

    fn create_api_error_responder(status: Status, api_exception_type: &ApiExceptionType, message: &str) -> Self {
        ApiErrorResponder {
            status,
            error: ApiErrorV2 { code: api_exception_type.to_owned(), message: String::from(message), error: true }
        }
    }

    pub fn rank_not_present() -> Self {
        ApiErrorResponder::create_api_error_responder(
            Status::NotFound, 
            &ApiExceptionType::RankNotPresent, 
            "The rank is not present in the list"
        )
    }

    pub fn rank_already_present() -> Self {
        ApiErrorResponder::create_api_error_responder(
            Status::Conflict, 
            &ApiExceptionType::RankAlreadyPresent, 
            "The rank is already present in the list"
        )
    }

    pub fn session_not_found() -> Self {
        ApiErrorResponder::create_api_error_responder(
            Status::Conflict, 
            &ApiExceptionType::SessionMissing, 
            "Session not found"
        )
    }

    pub fn session_inactive() -> Self {
        ApiErrorResponder::create_api_error_responder(
            Status::Conflict, 
            &ApiExceptionType::SessionInactive, 
            "Session inactive"
        )
    }

    pub fn rank_confict() -> Self {
        ApiErrorResponder::create_api_error_responder(
            Status::Conflict, 
            &ApiExceptionType::RankConflict, 
            "A rank already exists with that name"
        )
    }

    pub fn missing_rank() -> Self {
        ApiErrorResponder::create_api_error_responder(
            Status::NotFound, 
            &ApiExceptionType::RankMissing, 
            "The rank does not exist"
        )
    }

    pub fn missing_map() -> Self {
        ApiErrorResponder::create_api_error_responder(
            Status::NotFound, 
            &ApiExceptionType::MapMissing, 
            "The map does not exist"
        )
    }

    pub fn missing_player() -> Self {
        ApiErrorResponder::create_api_error_responder(
            Status::NotFound, 
            &ApiExceptionType::PlayerMissing, 
            "The player does not exist"
        )
    }

    pub fn missing_punishment() -> Self {
        ApiErrorResponder::create_api_error_responder(
            Status::NotFound, 
            &ApiExceptionType::PunishmentMissing, 
            "The punishment does not exist"
        )
    }

    pub fn validation_error() -> Self {
        Self::validation_error_with_message("Validation failed")
    }

    pub fn validation_error_with_message(message: &str) -> Self {
        ApiErrorResponder::create_api_error_responder(
            Status::BadRequest, 
            &ApiExceptionType::ValidationError, 
            message
        )
    }

    pub fn unauthorized() -> Self {
        ApiErrorResponder::create_api_error_responder(
            Status::Unauthorized, 
            &ApiExceptionType::UnauthorizedException, 
            "API credentials are missing or invalid"
        )
    }

    pub fn note_missing() -> Self {
        ApiErrorResponder::create_api_error_responder(
            Status::NotFound, 
            &ApiExceptionType::NoteMissing, 
            "The note does not exist"
        )
    }

    pub fn tag_missing_from_player() -> Self {
        ApiErrorResponder::create_api_error_responder(
            Status::NotFound, 
            &ApiExceptionType::TagNotPresent, 
            "The tag is not present in the list"
        )
    }

    pub fn tag_missing() -> Self {
        ApiErrorResponder::create_api_error_responder(
            Status::NotFound, 
            &ApiExceptionType::TagMissing, 
            "The tag does not exist"
        )
    }

    pub fn tag_already_present() -> Self {
        ApiErrorResponder::create_api_error_responder(
            Status::NotFound, 
            &ApiExceptionType::TagAlreadyPresent, 
            "The tag is already present in the list"
        )
    }

    pub fn tag_conflict() -> Self {
        ApiErrorResponder::create_api_error_responder(
            Status::Conflict, 
            &ApiExceptionType::TagConflict, 
            "A tag already exists with that name"
        )
    }

    pub fn achievement_missing() -> Self {
        ApiErrorResponder::create_api_error_responder(
            Status::NotFound, 
            &ApiExceptionType::AchievementMising, 
            "The achievement does not exist"
        )
    }

    pub fn achievement_conflict() -> Self {
        ApiErrorResponder::create_api_error_responder(
            Status::Conflict,
            &ApiExceptionType::AchievementConflict, 
            "An achievement already exists with that name"
        )
    }
}

impl<'r> Responder<'r, 'static> for ApiErrorResponder {
    fn respond_to(self, req: &'r Request<'_>) -> response::Result<'static> {
        let data = Json(self.error);
        Response::build_from(data.respond_to(req)?)
            .header(ContentType::JSON)
            .status(self.status)
            .ok()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiErrorV2 {
    code: ApiExceptionType,
    message: String,
    error: bool
}

#[derive(Responder)]
pub enum ApiError {
    #[response(status = 400)]
    Validation(String),
    #[response(status = 400)]
    PlayerMissing(String),
    #[response(status = 404)]
    SessionNotFound(String),
    #[response(status = 404)]
    SessionInactive(String)
}

impl ApiError {
    pub fn missing_player(name: &str) -> Self {
        Self::PlayerMissing(format!("Player could not be found: {}", name))
    }

    pub fn session_not_found() -> Self {
        Self::SessionNotFound(String::from("The session does not exist"))
    }

    pub fn session_inactive() -> Self {
        Self::SessionInactive(String::from("The session is not active"))
    }
}

#[derive(Serialize, Deserialize, Display, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
enum ApiExceptionType {
    InternalServerError,
    UnauthorizedException,
    ValidationError,
    SessionMissing,
    SessionInactive,
    PlayerMissing,
    RankConflict,
    RankMissing,
    RankAlreadyPresent,
    RankNotPresent,
    TagConflict,
    TagMissing,
    TagAlreadyPresent,
    TagNotPresent,
    MapMissing,
    AchievementConflict,
    AchievementMising,
    PunishmentMissing,
    NoteMissing,
    Anonymous
}
