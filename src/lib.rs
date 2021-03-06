#![allow(proc_macro_derive_resolution_fallback)]

#[macro_use]
extern crate failure;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate log;
#[macro_use]
extern crate http_router;
#[macro_use]
extern crate sentry;

extern crate base64;
extern crate chrono;
extern crate config as config_crate;
extern crate env_logger;
extern crate futures;
extern crate futures_cpupool;
extern crate gelf;
extern crate hyper;
extern crate hyper_tls;
extern crate rand;
extern crate regex;
extern crate rlp;
extern crate serde;
#[macro_use]
extern crate serde_json;
extern crate lapin_async;
extern crate lapin_futures;
extern crate r2d2;
extern crate serde_qs;
extern crate simplelog;
extern crate tokio;
extern crate tokio_core;
extern crate uuid;
extern crate validator;

#[macro_use]
mod macros;
mod api;
mod client;
mod config;
mod logger;
mod models;
mod pollers;
mod prelude;
mod rabbit;
mod sentry_integration;
mod services;
mod utils;

use std::sync::Arc;
use std::time::Duration;

use self::client::{BitcoinClient, BitcoinClientImpl, EthereumClient, EthereumClientImpl, HttpClientImpl};
use self::pollers::{BitcoinPollerService, EthereumPollerService, StoriqaPollerService};
use self::utils::log_error;
use config::Config;
use prelude::*;
use rabbit::{RabbitConnectionManager, TransactionPublisherImpl};

pub fn print_config() {
    println!("Parsed config: {:?}", get_config());
}

pub fn start_server() {
    let config = get_config();
    // Prepare sentry integration
    let _sentry = sentry_integration::init(config.sentry.as_ref());
    // Prepare logger
    logger::init(&config);

    let mut rt = tokio::runtime::Runtime::new().unwrap();

    let http_client = HttpClientImpl::new(&config, log::Level::Trace);
    let bitcoin_client = Arc::new(BitcoinClientImpl::new(
        Arc::new(http_client.clone()),
        config.mode.clone(),
        config.client.bitcoin_rpc_url.clone(),
        config.client.bitcoin_rpc_user.clone(),
        config.client.bitcoin_rpc_password.clone(),
    ));
    let ethereum_client = Arc::new(EthereumClientImpl::new(
        Arc::new(http_client.clone()),
        config.mode.clone(),
        config.client.infura_key.clone(),
        config.client.stq_contract_address.clone(),
        config.client.stq_transfer_topic.clone(),
        config.client.stq_approval_topic.clone(),
        config.client.stq_balance_method.clone(),
    ));

    debug!("Started creating rabbit connection pool");
    let rabbit_connection_manager = rt
        .block_on(RabbitConnectionManager::create(&config))
        .map_err(|e| {
            log_error(&e);
        })
        .expect("Can not create rabbit connection manager");
    debug!("Finished creating rabbit connection manager");
    if config.poller.enabled {
        let channel = Arc::new(rabbit_connection_manager.get_channel().expect("Can not get channel from pool"));
        let publisher = rt
            .block_on(TransactionPublisherImpl::init(channel))
            .map_err(|e| {
                log_error(&e);
            })
            .expect("Can not create rabbit connection manager");

        let publisher = Arc::new(publisher);
        let ethereum_poller = EthereumPollerService::new(
            Duration::from_secs(config.poller.ethereum_interval_secs as u64),
            ethereum_client.clone(),
            publisher.clone(),
            config.poller.ethereum_number_of_tracked_confirmations,
        );
        let storiqa_poller = StoriqaPollerService::new(
            Duration::from_secs(config.poller.storiqa_interval_secs as u64),
            ethereum_client.clone(),
            publisher.clone(),
            config.poller.storiqa_number_of_tracked_confirmations,
        );
        let bitcoin_poller = BitcoinPollerService::new(
            Duration::from_secs(config.poller.bitcoin_interval_secs as u64),
            bitcoin_client.clone(),
            publisher.clone(),
            config.poller.bitcoin_number_of_tracked_confirmations,
        );

        rt.spawn(bitcoin_poller.start());
        rt.spawn(ethereum_poller.start());
        rt.spawn(storiqa_poller.start());
    }

    rt.spawn(api::start_server(config));

    rt.shutdown_on_idle().wait().expect("Tokio runtime shutdown failed");
}

pub fn get_btc_blocks(hash: Option<String>, number: u64) {
    let config = get_config();
    let bitcoin_client = create_btc_client(&config);

    let fut = bitcoin_client
        .last_blocks(hash, number)
        .for_each(|block| {
            println!("{:#?}", block);
            Ok(())
        })
        .map_err(|e| {
            log_error(&e);
        });

    let mut core = ::tokio_core::reactor::Core::new().unwrap();
    let _ = core.run(fut);
}

pub fn get_btc_transaction(hash: &str) {
    let config = get_config();
    let bitcoin_client = create_btc_client(&config);

    let fut = bitcoin_client
        .get_transaction(hash.to_string(), 0)
        .map(|tx| {
            println!("{:#?}", tx);
        })
        .map_err(|e| {
            log_error(&e);
        });

    let mut core = ::tokio_core::reactor::Core::new().unwrap();
    let _ = core.run(fut);
}

pub fn get_btc_transactions(hash: Option<String>, number: u64) {
    let config = get_config();
    let bitcoin_client = create_btc_client(&config);

    let fut = bitcoin_client
        .last_transactions(hash, number)
        .for_each(|block| {
            println!("{:#?}", block);
            Ok(())
        })
        .map_err(|e| {
            log_error(&e);
        });

    let mut core = ::tokio_core::reactor::Core::new().unwrap();
    let _ = core.run(fut);
}

pub fn publish_btc_transactions(hash: Option<String>, number: u64) {
    let config = get_config();
    let bitcoin_client = Arc::new(create_btc_client(&config));
    let mut core = ::tokio_core::reactor::Core::new().unwrap();
    let rabbit_connection_manager = core
        .run(RabbitConnectionManager::create(&config))
        .map_err(|e| {
            log_error(&e);
        })
        .expect("Can not create rabbit connection manager");
    let channel = Arc::new(rabbit_connection_manager.get_channel().expect("Can not get channel from pool"));
    let f = TransactionPublisherImpl::init(channel)
        .map_err(|e| {
            log_error(&e);
        })
        .and_then(move |publisher| {
            let btc_poller = BitcoinPollerService::new(
                Duration::from_secs(config.poller.bitcoin_interval_secs as u64),
                bitcoin_client,
                Arc::new(publisher),
                number as usize,
            );
            btc_poller.publish_transactions(hash, number).map_err(|e| {
                log_error(&e);
            })
        });
    let _ = core.run(f);
}

pub fn get_eth_transaction(hash: &str) {
    let config = get_config();
    let ethereum_client = create_eth_client(&config);

    let fut = ethereum_client
        .get_eth_transaction(hash.to_string())
        .map(|tx| {
            println!("{:#?}", tx);
        })
        .map_err(|e| {
            log_error(&e);
        });

    let mut core = ::tokio_core::reactor::Core::new().unwrap();
    let _ = core.run(fut);
}

pub fn get_eth_transactions(hash: Option<String>, number: u64) {
    let config = get_config();
    let ethereum_client = create_eth_client(&config);

    let fut = ethereum_client
        .last_eth_transactions(hash, number)
        .for_each(|block| {
            println!("{:#?}", block);
            Ok(())
        })
        .map_err(|e| {
            log_error(&e);
        });

    let mut core = ::tokio_core::reactor::Core::new().unwrap();
    let _ = core.run(fut);
}

pub fn publish_eth_transactions(hash: Option<String>, number: u64) {
    let config = get_config();
    let ethereum_client = Arc::new(create_eth_client(&config));
    let mut core = ::tokio_core::reactor::Core::new().unwrap();
    let rabbit_connection_manager = core
        .run(RabbitConnectionManager::create(&config))
        .map_err(|e| {
            log_error(&e);
        })
        .expect("Can not create rabbit connection manager");
    let channel = Arc::new(rabbit_connection_manager.get_channel().expect("Can not get channel from pool"));
    let f = TransactionPublisherImpl::init(channel)
        .map_err(|e| {
            log_error(&e);
        })
        .and_then(move |publisher| {
            let eth_poller = EthereumPollerService::new(
                Duration::from_secs(config.poller.ethereum_interval_secs as u64),
                ethereum_client,
                Arc::new(publisher),
                number as usize,
            );
            eth_poller.publish_transactions(hash, number).map_err(|e| {
                log_error(&e);
            })
        });
    let _ = core.run(f);
}

pub fn get_stq_transaction(hash: &str) {
    let config = get_config();
    let storiqa_client = create_eth_client(&config);

    let fut = storiqa_client
        .get_stq_transactions(hash.to_string())
        .for_each(|tx| {
            println!("{:#?}", tx);
            Ok(())
        })
        .map_err(|e| {
            log_error(&e);
        });

    let mut core = ::tokio_core::reactor::Core::new().unwrap();
    let _ = core.run(fut);
}

pub fn get_stq_transactions(hash: Option<String>, number: u64) {
    let config = get_config();
    let storiqa_client = create_eth_client(&config);

    let fut = storiqa_client
        .last_stq_transactions(hash, number)
        .for_each(|block| {
            println!("{:#?}", block);
            Ok(())
        })
        .map_err(|e| {
            log_error(&e);
        });

    let mut core = ::tokio_core::reactor::Core::new().unwrap();
    let _ = core.run(fut);
}

pub fn publish_stq_transactions(hash: Option<String>, number: u64) {
    let config = get_config();
    let storiqa_client = Arc::new(create_eth_client(&config));
    let mut core = ::tokio_core::reactor::Core::new().unwrap();
    let rabbit_connection_manager = core
        .run(RabbitConnectionManager::create(&config))
        .map_err(|e| {
            log_error(&e);
        })
        .expect("Can not create rabbit connection manager");
    let channel = Arc::new(rabbit_connection_manager.get_channel().expect("Can not get channel from pool"));
    let f = TransactionPublisherImpl::init(channel)
        .map_err(|e| {
            log_error(&e);
        })
        .and_then(move |publisher| {
            let stq_poller = StoriqaPollerService::new(
                Duration::from_secs(config.poller.storiqa_interval_secs as u64),
                storiqa_client,
                Arc::new(publisher),
                number as usize,
            );
            stq_poller.publish_transactions(hash, number).map_err(|e| {
                log_error(&e);
            })
        });
    let _ = core.run(f);
}

fn create_btc_client(config: &Config) -> BitcoinClientImpl {
    let http_client = Arc::new(HttpClientImpl::new(config, log::Level::Debug));
    BitcoinClientImpl::new(
        http_client.clone(),
        config.mode.clone(),
        config.client.bitcoin_rpc_url.clone(),
        config.client.bitcoin_rpc_user.clone(),
        config.client.bitcoin_rpc_password.clone(),
    )
}

fn create_eth_client(config: &Config) -> EthereumClientImpl {
    let http_client = Arc::new(HttpClientImpl::new(config, log::Level::Debug));
    EthereumClientImpl::new(
        http_client.clone(),
        config.mode.clone(),
        config.client.infura_key.clone(),
        config.client.stq_contract_address.clone(),
        config.client.stq_transfer_topic.clone(),
        config.client.stq_approval_topic.clone(),
        config.client.stq_balance_method.clone(),
    )
}

fn get_config() -> Config {
    config::Config::new().unwrap_or_else(|e| panic!("Error parsing config: {}", e))
}
