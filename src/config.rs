use serde;
use serde::{Deserialize, Deserializer};
use std::env;

use sentry_integration::SentryConfig;

use config_crate::{Config as RawConfig, ConfigError, Environment, File};

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub server: Server,
    pub client: Client,
    #[serde(deserialize_with = "deserialize_mode")]
    pub mode: Mode,
    pub sentry: Option<SentryConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Server {
    pub host: String,
    pub port: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Database {
    pub url: String,
    pub thread_pool_size: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Client {
    pub dns_threads: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub enum Mode {
    Nightly,
    Stable,
    Stage,
    Production,
}

fn deserialize_mode<'de, D>(de: D) -> Result<Mode, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(de)?;
    match s.as_ref() {
        "nightly" => Ok(Mode::Nightly),
        "stable" => Ok(Mode::Stable),
        "stage" => Ok(Mode::Stage),
        "production" => Ok(Mode::Production),
        other => Err(serde::de::Error::custom(format!("unknown mode: {}", other))),
    }
}

impl Config {
    pub fn new() -> Result<Self, ConfigError> {
        let mut s = RawConfig::new();
        s.merge(File::with_name("config/base"))?;

        // Merge development.toml if RUN_MODE variable is not set
        let env = env::var("RUN_MODE").unwrap_or_else(|_| "development".into());
        s.merge(File::with_name(&format!("config/{}", env)).required(false))?;

        s.merge(Environment::with_prefix("STQ_PAYMENTS"))?;
        s.try_into()
    }
}
