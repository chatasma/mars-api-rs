use anyhow::Context;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::default::Default;
use std::{str, env};
use crate::database::models::punishment::PunishmentType;
use crate::util::webhook::WebhookUtils;

use super::database::models::level_color::LevelColor;
use super::database::models::join_sound::JoinSound;
use super::database::models::broadcast::Broadcast;
use super::util::file::{read_file, deserialize_properties_file};

#[derive(Debug)]
pub enum ConfigDeserializeError {
    IOError(std::io::Error),
    ParseError(ConfigParseError)
}

#[derive(Debug)]
pub struct ConfigParseError {
    pub file_path: String,
    pub parse_error: serde_yaml::Error
}

#[derive(Debug)]
pub struct ConfigMissingFieldError {
    pub field_name: String
}

impl std::fmt::Display for ConfigMissingFieldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = format!("Missing required field '{}'", self.field_name);
        write!(f, "{}", message)
    }
}
impl std::error::Error for ConfigMissingFieldError {}

impl ToString for ConfigParseError {
    fn to_string(&self) -> String {
        format!("File {} could not be parsed: {}", self.file_path, self.parse_error)
    }
}

impl std::fmt::Display for ConfigDeserializeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::IOError(io_err) => io_err.to_string(),
            Self::ParseError(parse_err) => parse_err.to_string()
        };
        write!(f, "{}", message)
    }
}

impl std::error::Error for ConfigDeserializeError {
}

impl From<std::io::Error> for ConfigDeserializeError {
    fn from(e: std::io::Error) -> Self {
        ConfigDeserializeError::IOError(e)
    }
}

impl From<ConfigParseError> for ConfigDeserializeError {
    fn from(e: ConfigParseError) -> Self {
        ConfigDeserializeError::ParseError(e)
    }
}

const TOKEN_ENV_VARIABLE : &'static str = "MARS_API_TOKEN";

pub async fn deserialize_mars_config() -> anyhow::Result<MarsConfig> {
    let token = env::var(TOKEN_ENV_VARIABLE).context(format!("Missing API environment variable {}", TOKEN_ENV_VARIABLE))?;
    let (options, data) = tokio::try_join!(
        deserialize_mars_options(),
        deserialize_mars_data()
    )?;
    let webhooks = WebhookUtils::new(
        &(if options.reports_webhook_url.is_empty() { None } else { Some(options.reports_webhook_url.clone()) }), 
        &(if options.punishments_webhook_url.is_empty() { None } else { Some(options.punishments_webhook_url.clone()) }), 
        &(if options.notes_webhook_url.is_empty() { None } else { Some(options.notes_webhook_url.clone()) })
    );
    Ok(MarsConfig { token, options, data, webhooks })
}

async fn deserialize_mars_options() -> Result<MarsConfigOptions, ConfigDeserializeError> {
    let config_path = env::var("MARS_CONFIG_PATH").unwrap_or("./config.properties".to_string());
    let map = deserialize_properties_file(&config_path).await?;
    let mut config = MarsConfigOptions::default();
    map.iter().for_each(|(k, v)| {
        match k.as_str() {
            "listen-port" => { if let Ok(i) = v.to_string().parse::<u32>() { config.port = i; } },
            "listen-host" => { config.host = v.to_string(); },
            "mongo-url" => {config.mongo_url = v.to_string();},
            "redis-host" => { config.redis_host = Some(v.to_string()); },
            "enable-ip-hashing" => { if let Ok(b) = v.to_string().parse::<bool>() { config.enable_ip_hashing = b; } },
            "webhooks.punishments" => { config.punishments_webhook_url = v.to_string(); },
            "webhooks.reports" => { config.reports_webhook_url = v.to_string(); },
            "webhooks.notes" => { config.notes_webhook_url = v.to_string(); },
            "webhooks.debug" => { config.debug_log_webhook_url = v.to_string(); },
            "enable-exponential-exp" => { if let Ok(b) = v.to_string().parse::<bool>() { config.use_exponential_exp = b; } }
            _ => {}
        }
    });
    Ok(config)
}

async fn deserialize_mars_data() -> Result<MarsConfigData, ConfigDeserializeError> {
    let level_colors_path = env::var("MARS_LEVEL_COLORS_PATH").unwrap_or("./level_colors.yml".to_string());
    let join_sounds_path = env::var("MARS_JOIN_SOUNDS_PATH").unwrap_or("./join_sounds.yml".to_string());
    let broadcasts_path = env::var("MARS_BROADCASTS_PATH").unwrap_or("./broadcasts.yml".to_string());
    let pun_types_path = env::var("MARS_PUNTYPES_PATH").unwrap_or("./punishment_types.yml".to_string());

    let (
        level_colors, 
        join_sounds, 
        broadcasts, 
        punishment_types
    ) = match tokio::try_join!(
        deserialize_mars_data_component::<Vec<LevelColor>>(&level_colors_path),
        deserialize_mars_data_component::<Vec<JoinSound>>(&join_sounds_path),
        deserialize_mars_data_component::<Vec<Broadcast>>(&broadcasts_path),
        deserialize_mars_data_component::<Vec<PunishmentType>>(&pun_types_path)
    ) {
        Ok(values) => values,
        Err(e) => return Err(e)
    };
    Ok(MarsConfigData { 
        level_colors,
        join_sounds,
        broadcasts,
        punishment_types
    })
}

async fn deserialize_mars_data_component<T: DeserializeOwned>(
    file_path: &String,
) -> Result<T, ConfigDeserializeError> {
    let content = read_file(&file_path).await?;
    let data = match serde_yaml::from_str::<T>(&content) {
        Ok(data) => data,
        Err(e) => return Err(ConfigDeserializeError::ParseError(ConfigParseError { 
            file_path: file_path.clone(), 
            parse_error: e 
        })),
    };
    Ok(data)
}

pub struct MarsConfig {
    pub token: String,
    pub options: MarsConfigOptions,
    pub data: MarsConfigData,
    pub webhooks: WebhookUtils
}

// impl Default for MarsConfig {
//     fn default() -> Self {
//         MarsConfig {
//             token: String::from(""),
//             options: MarsConfigOptions::default(),
//             data: MarsConfigData::default(),

//         }
//     }
// }

pub struct MarsConfigOptions {
    pub port: u32,
    pub host: String,
    pub mongo_url: String,
    pub redis_host: Option<String>,
    pub enable_ip_hashing: bool,
    pub punishments_webhook_url: String,
    pub reports_webhook_url: String,
    pub notes_webhook_url: String,
    pub debug_log_webhook_url: String,
    pub use_exponential_exp: bool
}

impl Default for MarsConfigOptions {
    fn default() -> Self {
        MarsConfigOptions { 
            mongo_url: String::new(), 
            port: 3000, 
            host: String::new(), 
            redis_host: None, 
            enable_ip_hashing: false,
            punishments_webhook_url: String::new(),
            reports_webhook_url: String::new(),
            notes_webhook_url: String::new(),
            debug_log_webhook_url: String::new(),
            use_exponential_exp: false
        }
    }
}

#[derive(Deserialize, Default)]
pub struct MarsConfigData {
    pub level_colors: Vec<LevelColor>,
    pub join_sounds: Vec<JoinSound>,
    pub broadcasts: Vec<Broadcast>,
    pub punishment_types: Vec<PunishmentType>
}
