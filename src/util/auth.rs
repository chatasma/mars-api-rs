use rocket::{request::{FromRequest, self}, Request, http::Status};

use crate::MarsAPIState;

struct TokenType;
impl TokenType {
    pub const BEARER: &'static str = "Bearer";
    pub const API_TOKEN: &'static str = "API-Token";
}

pub struct AuthorizationToken {
    pub server_id: String
}

pub struct AuthorizationError {
    problem: String
}

impl std::fmt::Debug for AuthorizationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Authorization Error: {}", self.problem)
    }
}

fn create_failure_outcome(status: Status, error: String) -> request::Outcome<AuthorizationToken, AuthorizationError> {
    request::Outcome::Error((status, AuthorizationError { problem: error }))
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthorizationToken {
    type Error = AuthorizationError;

    async fn from_request(req: &'r Request<'_>) -> request::Outcome<Self, AuthorizationError> {
        let header_map = req.headers();
        let server_id = if let Some(id) = header_map.get_one("Mars-Server-ID") { Some(String::from(id)) } else { None };
        let actual_token = if let Some(state) = req.rocket().state::<MarsAPIState>() { 
            &state.config.token 
        } else {
            return create_failure_outcome(Status::InternalServerError, String::from("Internal error"))
        };
        match header_map.get_one("Authorization") {
            Some(value) => {
                let parts = value.split(" ").collect::<Vec<&str>>();
                if parts.len() < 2 {
                    return create_failure_outcome(Status::Unauthorized, String::from("Malformed Authorization header"));
                };
                let token_type = parts[0];
                let provided_token = parts[1];

                match token_type {
                    TokenType::API_TOKEN => {
                        if server_id.is_none() {
                            return create_failure_outcome(Status::Unauthorized, String::from("Missing server ID"));
                        } else if actual_token != provided_token {
                            return create_failure_outcome(Status::Unauthorized, String::from("Wrong token bro"));
                        };
                        request::Outcome::Success(AuthorizationToken { server_id: server_id.unwrap() })
                    },
                    TokenType::BEARER => create_failure_outcome(Status::Unauthorized, String::from("Unsupported token type")),
                    _ => create_failure_outcome(Status::Unauthorized, String::from("Unknown token type"))
                }
            },
            None => create_failure_outcome(Status::Unauthorized, String::from("Did not provide authorization header"))
        }
    }
}
