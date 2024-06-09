use std::collections::{HashMap, HashSet};

use std::io::{Read};
use std::sync::Arc;

use futures::StreamExt;
use log::info;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;


use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response, ErrorResponse};
use tokio_tungstenite::tungstenite::http::Response as HttpResponse;
use flate2::read::ZlibDecoder;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;

use crate::MarsAPIState;
use crate::socket::event_type::EventType;
use crate::socket::socket_router::SocketRouter;
use crate::util::error::ApiErrorResponder;
use crate::util::r#macro::unwrap_helper;
use crate::util::time::get_u64_time_millis;

use rocket::serde::json::{serde_json, Value};

use super::server::server_context::ServerContext;

pub struct SocketState {
    pub api_state: Arc<MarsAPIState>
}

pub struct SocketSession {
    pub server_id: String,
    pub api_state: Arc<MarsAPIState>,
    // concurrent access may be possible
    pub connected_servers: Arc<Mutex<HashSet<ServerContext>>>
}

pub async fn setup_socket(
    socket_state: SocketState, 
    port: u32
) -> anyhow::Result<()> {
    let connected_servers : Arc<Mutex<HashSet<ServerContext>>> = Arc::new(Mutex::new(HashSet::new()));
    let addr = format!("0.0.0.0:{}", port);
    info!("Socket listening on: {}", addr);

    // Create the event loop and TCP listener we'll accept connections on.
    let socket = TcpListener::bind(&addr).await?;

    loop {
        tokio::select! {
            socket_accept_result = socket.accept() => {
                if let Ok((stream, _)) = socket_accept_result {
                    let mut session_state : SocketSession = SocketSession { server_id: "".to_owned(), connected_servers: Arc::clone(&connected_servers), api_state: socket_state.api_state.clone() };
                    let ws_stream = match tokio_tungstenite::accept_hdr_async(stream, |request: &Request, response: Response| -> Result<Response, ErrorResponse> {
                        verify_connection(&socket_state, &mut session_state, request, response)
                    }).await {
                        Ok(ws_stream) => ws_stream,
                        Err(e) => {warn!("{}", e); continue}
                    };
                    tokio::spawn(accept_connection(ws_stream, session_state));
                }
            },
            _ = exit_signal() => {
                info!("Gracefully dropping websocket");
                break;
            },
        };
    }
    Ok(())
}

async fn accept_connection(
    ws_stream: WebSocketStream<TcpStream>, 
    socket_session: SocketSession
) -> anyhow::Result<()> {
    info!("Accepted WebSocket connection from server {}", socket_session.server_id.clone());
    let server_id = socket_session.server_id.clone();
    let server = {
        let server = ServerContext {
            id: socket_session.server_id.clone(), api_state: socket_session.api_state.clone(), stream: ws_stream 
        };
        server
    };
    
    let mut router = SocketRouter::new(server);

    while let Some(msg) = router.server.stream.next().await {
        let msg = unwrap_helper::continue_default!(msg.ok());
        let data = match msg {
            tokio_tungstenite::tungstenite::Message::Binary(data) => data,
            _ => continue
        };

        let mut zlib_decoder = ZlibDecoder::new(data.as_slice());
        let mut decompressed = String::new();
        if let Err(_) = zlib_decoder.read_to_string(&mut decompressed) {
            continue;
        };
        let text = decompressed;


        let json_object : Value = unwrap_helper::continue_default!(serde_json::from_str(&text).ok());
        let event = {
            let e_val = json_object.get("e");
            if e_val.is_none() {
                continue;
            };
            unwrap_helper::continue_default!(serde_json::from_value::<EventType>(e_val.unwrap().to_owned()).ok())
        };
        let socket_data = {
            let d_val = json_object.get("d");
            if d_val.is_none() {
                continue;
            };
            d_val.unwrap().to_owned()
        };
        let socket_data_serialized = socket_data.to_string();

        router.route(&event, socket_data).await;
        router.server.set_last_time_alive(get_u64_time_millis()).await;
        info!("[{}:{}] {}", server_id, event, socket_data_serialized);
    }
    info!("WebSocket connection closed from server {}", socket_session.server_id.clone());
    let _ = router.server.stream.close(Some(CloseFrame { code: CloseCode::Normal, reason: std::borrow::Cow::Borrowed("Connection closed")  })).await;

    Ok(())
}

fn verify_connection(socket_state: &SocketState, socket_session: &mut SocketSession, request: &Request, response: Response) -> Result<Response, ErrorResponse> {
    if request.uri().path() != "/minecraft" {
        return Err(build_response_from_error_responder(ApiErrorResponder::unauthorized()));
    }
    if let Some(query_string) = request.uri().query() {
        let hash_query : HashMap<String, String> = url::form_urlencoded::parse(query_string.as_bytes()).into_owned().collect();
        let server_id = unwrap_helper::return_default!(hash_query.get("id"), Err(build_response_from_error_responder(ApiErrorResponder::unauthorized()))).to_owned();
        let token = unwrap_helper::return_default!(hash_query.get("token"), Err(build_response_from_error_responder(ApiErrorResponder::unauthorized()))).to_owned();
        if token != socket_state.api_state.config.token {
            return Err(build_response_from_error_responder(ApiErrorResponder::unauthorized()));
        };
        socket_session.server_id = server_id;
        return Ok(response);
    } else {
        return Err(build_response_from_error_responder(ApiErrorResponder::unauthorized()));
    };
}

fn build_response_from_error_responder(responder: ApiErrorResponder) -> HttpResponse<Option<String>> {
    let data = rocket::serde::json::serde_json::to_string(&responder.error).unwrap_or_else(|_| "{}".to_owned());
    HttpResponse::builder().status(responder.status.code).body(Some(data)).unwrap()
}

#[cfg(target_family = "unix")]
async fn exit_signal() {
    // https://docs.rs/tokio/latest/src/tokio/signal/unix.rs.html#6
    use tokio::signal::unix::{signal, SignalKind};
    let mut sigterm = signal(SignalKind::terminate()).unwrap();
    tokio::select! {
       _ = tokio::signal::ctrl_c() => {}
       _ = sigterm.recv() => {}
    }
}

#[cfg(not(target_family = "unix"))]
async fn exit_signal() {
    tokio::signal::ctrl_c().await.ok();
}