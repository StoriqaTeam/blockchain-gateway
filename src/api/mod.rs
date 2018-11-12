use std::net::SocketAddr;
use std::sync::Arc;

use client::HttpClientImpl;
use failure::{Compat, Fail};
use futures::future;
use futures::prelude::*;
use hyper;
use hyper::Server;
use hyper::{service::Service, Body, Request, Response};

use super::config::Config;
use super::utils::{log_and_capture_error, log_error, log_warn};
use client::{BitcoinClientImpl, EthereumClientImpl};
use services::{BitcoinServiceImpl, EthereumServiceImpl};
use utils::read_body;

mod controllers;
mod error;
mod requests;
mod responses;
mod utils;

use self::controllers::*;
use self::error::*;
use models::*;
use serde_json;

#[derive(Clone)]
pub struct ApiService {
    server_address: SocketAddr,
    config: Config,
}

impl ApiService {
    fn from_config(config: &Config) -> Result<Self, Error> {
        let server_address = format!("{}:{}", config.server.host, config.server.port)
            .parse::<SocketAddr>()
            .map_err(ectx!(
                try
                ErrorContext::Config,
                ErrorKind::Internal =>
                config.server.host,
                config.server.port
            ))?;
        Ok(ApiService {
            config: config.clone(),
            server_address,
        })
    }
}

impl Service for ApiService {
    type ReqBody = Body;
    type ResBody = Body;
    type Error = Compat<Error>;
    type Future = Box<Future<Item = Response<Body>, Error = Self::Error> + Send>;

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let (parts, http_body) = req.into_parts();
        let config = self.config.clone();
        Box::new(
            read_body(http_body)
                .map_err(ectx!(ErrorSource::Hyper, ErrorKind::Internal))
                .and_then(move |body| {
                    let router = router! {
                        GET /v1/bitcoin/{address: BitcoinAddress}/utxos => get_utxos,
                        POST /v1/bitcoin/transactions/raw => post_bitcoin_transactions,
                        GET /v1/ethereum/{address: EthereumAddress}/nonce => get_nonce,
                        POST /v1/ethereum/transactions/raw => post_ethereum_transactions,
                        _ => not_found,
                    };

                    let http_client = Arc::new(HttpClientImpl::new(&config));
                    let bitcoin_client = Arc::new(BitcoinClientImpl::new(
                        http_client.clone(),
                        config.mode.clone(),
                        config.client.bitcoin_rpc_url.clone(),
                        config.client.bitcoin_rpc_user.clone(),
                        config.client.bitcoin_rpc_password.clone(),
                    ));
                    let ethereum_client = Arc::new(EthereumClientImpl::new(
                        http_client.clone(),
                        config.mode.clone(),
                        config.client.infura_key.clone(),
                        config.client.stq_contract_address.clone(),
                        config.client.stq_transfer_topic.clone(),
                        config.client.stq_approval_topic.clone(),
                    ));

                    let bitcoin_service = Arc::new(BitcoinServiceImpl::new(bitcoin_client));
                    let ethereum_service = Arc::new(EthereumServiceImpl::new(ethereum_client));

                    let ctx = Context {
                        body,
                        method: parts.method.clone(),
                        uri: parts.uri.clone(),
                        headers: parts.headers,
                        bitcoin_service,
                        ethereum_service,
                    };

                    debug!("Received request {}", ctx);

                    router(ctx, parts.method.into(), parts.uri.path())
                }).and_then(|resp| {
                    let (parts, body) = resp.into_parts();
                    read_body(body)
                        .map_err(ectx!(ErrorSource::Hyper, ErrorKind::Internal))
                        .map(|body| (parts, body))
                }).map(|(parts, body)| {
                    debug!(
                        "Sent response with status {}, headers: {:#?}, body: {:?}",
                        parts.status.as_u16(),
                        parts.headers,
                        String::from_utf8(body.clone()).ok()
                    );
                    Response::from_parts(parts, body.into())
                }).or_else(|e| match e.kind() {
                    ErrorKind::BadRequest => {
                        log_error(&e);
                        Ok(Response::builder()
                            .status(400)
                            .header("Content-Type", "application/json")
                            .body(Body::from(r#"{"description": "Bad request"}"#))
                            .unwrap())
                    }
                    ErrorKind::Unauthorized => {
                        log_warn(&e);
                        Ok(Response::builder()
                            .status(401)
                            .header("Content-Type", "application/json")
                            .body(Body::from(r#"{"description": "Unauthorized"}"#))
                            .unwrap())
                    }
                    ErrorKind::NotFound => {
                        log_error(&e);
                        Ok(Response::builder()
                            .status(404)
                            .header("Content-Type", "application/json")
                            .body(Body::from(r#"{"description": "Not found"}"#))
                            .unwrap())
                    }

                    ErrorKind::UnprocessableEntity(errors) => {
                        log_warn(&e);
                        let errors =
                            serde_json::to_string(&errors).unwrap_or(r#"{"message": "unable to serialize validation errors"}"#.to_string());
                        Ok(Response::builder()
                            .status(422)
                            .header("Content-Type", "application/json")
                            .body(Body::from(errors))
                            .unwrap())
                    }
                    ErrorKind::Internal => {
                        log_and_capture_error(e);
                        Ok(Response::builder()
                            .status(500)
                            .header("Content-Type", "application/json")
                            .body(Body::from(r#"{"description": "Internal server error"}"#))
                            .unwrap())
                    }
                }),
        )
    }
}

pub fn start_server(config: Config) {
    hyper::rt::run(future::lazy(move || {
        ApiService::from_config(&config)
            .into_future()
            .and_then(move |api| {
                let api_clone = api.clone();
                let new_service = move || {
                    let res: Result<_, hyper::Error> = Ok(api_clone.clone());
                    res
                };
                let addr = api.server_address.clone();
                let server = Server::bind(&api.server_address)
                    .serve(new_service)
                    .map_err(ectx!(ErrorSource::Hyper, ErrorKind::Internal => addr));
                info!("Listening on http://{}", addr);
                server
            }).map_err(|e: Error| log_error(&e))
    }));
}
