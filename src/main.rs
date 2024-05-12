#[macro_use] extern crate rocket;

use std::{marker::PhantomData, sync::Arc, env, net::{Ipv4Addr, IpAddr}};

use anyhow::anyhow;
use config::{deserialize_mars_config, MarsConfig};
use database::{Database, cache::{Cache, get_redis_pool, RedisAdapter}, models::{player::Player, r#match::Match}};
use rocket::{Build, Rocket, Shutdown, Config, figment::Figment};
use socket::leaderboard::MarsLeaderboards;

use crate::socket::socket_handler::{SocketState, setup_socket};

mod util;
mod config;
mod database;
mod http;
mod socket;

fn setup_logger() -> Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{} [{}] [{}] {}",
                chrono::Local::now().format("[%Y-%m-%d] [%H:%M:%S]"),
                record.target(),
                record.level(),
                message
            ))
        })
        .level(log::LevelFilter::Debug)
        .chain(std::io::stdout())
        // .chain(fern::log_file("application.mars.log")?)
        .apply()?;
    Ok(())
}

// smart pointers to share for websocket and http
// can derive clone as well
#[derive(Clone)]
pub struct MarsAPIState {
    pub config: Arc<MarsConfig>,
    pub database: Arc<Database>,
    pub redis: Arc<RedisAdapter>,
    pub player_cache: Arc<Cache<Player>>,
    pub match_cache: Arc<Cache<Match>>,
    pub leaderboards: Arc<MarsLeaderboards>,
}

fn rocket(state: MarsAPIState) -> Rocket<Build> {
    let mounts : Vec<&dyn Fn(Rocket<Build>) -> Rocket<Build>> = vec![
        &http::broadcast::mount,
        &http::tag::mount,
        &http::status::mount,
        &http::player::mount,
        &http::server::mount,
        &http::level::mount,
        &http::map::mount,
        &http::rank::mount,
        &http::punishment::mount,
        &http::perks::mount,
        &http::leaderboard::mount,
        &http::report::mount,
        &http::r#match::mount,
        &http::achievements::mount
    ];
    let is_debug = env::var("MARS_DEBUG").unwrap_or("false".to_owned()).parse::<bool>().unwrap_or(false);
    let http_port = env::var("MARS_HTTP_PORT").unwrap_or("8000".to_owned()).parse::<u32>().unwrap_or(8000);
    let config : Config = Figment::from(
        if is_debug { Config::debug_default() } else { Config::release_default() }
    )
        .merge::<(&str, IpAddr)>(("address", Ipv4Addr::new(0, 0, 0, 0).into()))
        .merge(("port", http_port))
        .extract().unwrap();
    let mut rocket_build = rocket::custom(config).manage(state);

    rocket_build = mounts.iter().fold(rocket_build, |mut build, mount_fn| {
        build = (mount_fn)(build);
        build
    });

    rocket_build
}

async fn spawn_rocket(state: MarsAPIState) -> Result<Shutdown, String> {
    let rocket = match rocket(state).ignite().await {
        Ok(rocket) => rocket,
        Err(rocket_err) => return Err(format!("{}", rocket_err))
    };
    let shutdown_handle = rocket.shutdown();

    // Main I/O bound task scheduled as task for core thread, 
    // awaiting this will await until graceful shutdown or I/O / system errors starting rocket
    match rocket::tokio::spawn(rocket.launch()).await {
        Ok(_) => {},
        Err(rocket_err) => return Err(format!("{}", rocket_err))
    };
    Ok(shutdown_handle)
}

async fn setup_rocket(state: MarsAPIState) -> anyhow::Result<()> {
    // spawn rocket
    let shutdown_rocket = match spawn_rocket(state).await {
        Ok(shutdown_handle) => Some(shutdown_handle),
        Err(e) => return Err(anyhow!(e))
    };

    // gracefully notify rocket to shutdown
    if shutdown_rocket.is_some() {
        shutdown_rocket.unwrap().notify();
    };
    Ok(())
}


#[tokio::main]
async fn main() -> Result<(), String> {
    // config
    let mars_config = Arc::new(match deserialize_mars_config().await {
        Ok(config) => config,
        Err(parse_error) => return Err(format!("Config Error: {}", parse_error))
    });

    // setup runtime global logger
    match setup_logger() {
        Ok(_) => (),
        Err(e) => return Err(format!("Logger Setup Error: {}", e)),
    }

    // setup db pool
    let database = Arc::new(match database::connect(&mars_config.options.mongo_url, Some(2), Some(8)).await {
        Ok(db) => db,
        Err(db_err) => return Err(format!("Mongo Error: {}", db_err))
    });

    // setup redis pool
    let redis_adapter = Arc::new(match get_redis_pool(&mars_config.options.redis_host).await {
        Ok(adapter) => adapter,
        Err(redis_error) => return Err(format!("Redis Error: {}", redis_error))
    });

    // redis player cache
    let player_cache = Arc::new(Cache {
        redis: Arc::clone(&redis_adapter),
        resource_name: String::from("player"),
        lifetime_ms: 10_800_000,
        resource_type: PhantomData
    });

    // redis match cache
    let match_cache = Arc::new(Cache {
        redis: Arc::clone(&redis_adapter),
        resource_name: String::from("match"),
        lifetime_ms: 86_400_000,
        resource_type: PhantomData
    });

    // leaderboards
    let leaderboards = Arc::new(MarsLeaderboards::new(Arc::clone(&redis_adapter), Arc::clone(&database)));

    // immutable state for rocket to manage
    let state = MarsAPIState { 
        config: Arc::clone(&mars_config), 
        database: Arc::clone(&database), 
        redis: Arc::clone(&redis_adapter), 
        player_cache, 
        match_cache,
        leaderboards
    };

    let ws_port = env::var("MARS_WS_PORT").unwrap_or("7000".to_owned()).parse::<u32>().unwrap_or(7000);
    let res = tokio::try_join!(
        setup_rocket(state.clone()), 
        setup_socket(
            SocketState { 
                api_state: Arc::new(state.clone())
            }, ws_port
        )
    );

    if let Err(e) = res {
        warn!("{}", e);
    };

    Ok(())
}
