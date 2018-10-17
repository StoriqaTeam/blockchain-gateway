#![allow(proc_macro_derive_resolution_fallback)]

#[macro_use]
extern crate failure;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
#[macro_use]
extern crate http_router;
#[macro_use]
extern crate validator_derive;
#[macro_use]
extern crate sentry;

extern crate base64;
extern crate config as config_crate;
extern crate futures;
extern crate hyper;
extern crate hyper_tls;
extern crate rand;
extern crate regex;
extern crate rlp;
extern crate serde;
#[macro_use]
extern crate serde_json;
extern crate lapin_async;
extern crate r2d2;
extern crate serde_qs;
#[cfg(test)]
extern crate tokio_core;
extern crate uuid;
extern crate validator;
#[macro_use]
extern crate lapin_futures;
extern crate tokio;

#[macro_use]
mod macros;
mod api;
mod client;
mod config;
mod models;
mod prelude;
mod sentry_integration;
mod services;
mod utils;

use self::services::EthereumPollerService;
use config::Config;
use std::thread;
use std::time::Duration;

pub fn print_config() {
    println!("Parsed config: {:?}", get_config());
}

pub fn start_server() {
    let config = get_config();
    // Prepare sentry integration
    let _sentry = sentry_integration::init(config.sentry.as_ref());
    let ethereum_poller = EthereumPollerService::new(Duration::from_secs(1));
    thread::spawn(move || {
        ethereum_poller.start();
    });

    api::start_server(config);
}

fn get_config() -> Config {
    config::Config::new().unwrap_or_else(|e| panic!("Error parsing config: {}", e))
}
