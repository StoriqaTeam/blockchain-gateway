use serde;
use serde::{Deserialize, Deserializer};
use std::env;

use config_crate::{Config as RawConfig, ConfigError, Environment, File};
use logger::{FileLogConfig, GrayLogConfig};
use sentry_integration::SentryConfig;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub server: Server,
    pub client: Client,
    #[serde(deserialize_with = "deserialize_mode")]
    pub mode: Mode,
    pub poller: Poller,
    pub rabbit: Rabbit,
    pub sentry: Option<SentryConfig>,
    pub graylog: Option<GrayLogConfig>,
    pub filelog: Option<FileLogConfig>,
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
pub struct Rabbit {
    pub url: String,
    pub thread_pool_size: usize,
    pub connection_timeout_secs: usize,
    pub connection_pool_size: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Client {
    pub dns_threads: usize,
    pub infura_key: String,
    pub infura_secret: String,
    pub stq_contract_address: String,
    pub stq_transfer_topic: String,
    pub stq_approval_topic: String,
    pub bitcoin_rpc_url: String,
    pub bitcoin_rpc_user: String,
    pub bitcoin_rpc_password: String,
    pub stq_balance_method: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Poller {
    pub bitcoin_interval_secs: usize,
    pub bitcoin_number_of_tracked_confirmations: usize,
    pub ethereum_interval_secs: usize,
    pub ethereum_number_of_tracked_confirmations: usize,
    pub storiqa_interval_secs: usize,
    pub storiqa_number_of_tracked_confirmations: usize,
    pub ethereum_start_block: Option<u64>,
    pub storiqa_start_block: Option<u64>,
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
        s.merge(File::with_name("config/secret.toml").required(false))?;

        s.merge(Environment::with_prefix("STQ_PAYMENTS"))?;
        s.try_into()
    }
}
