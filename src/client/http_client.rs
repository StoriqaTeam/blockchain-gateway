use config::Config;
use failure::Fail;
use futures::future::Either;
use futures::prelude::*;
use hyper;
use hyper::{client::HttpConnector, Body, Request, Response};
use hyper_tls::HttpsConnector;
use log::{self, Level};

use super::error::*;
use utils::read_body;

pub trait HttpClient: Send + Sync + 'static {
    fn request(&self, req: Request<Body>) -> Box<Future<Item = Response<Body>, Error = Error> + Send>;
}

#[derive(Clone)]
pub struct HttpClientImpl {
    cli: hyper::Client<HttpsConnector<HttpConnector>>,
    log_level: Level,
}

impl HttpClientImpl {
    pub fn new(config: &Config, log_level: Level) -> Self {
        let connector = HttpsConnector::new(config.client.dns_threads).unwrap();
        // connector.https_only(true);
        let cli = hyper::Client::builder().build(connector);
        Self { cli, log_level }
    }
}

impl HttpClient for HttpClientImpl {
    fn request(&self, req: Request<Body>) -> Box<Future<Item = Response<Body>, Error = Error> + Send> {
        let cli = self.cli.clone();
        let level = log::max_level();
        let self_log_level = self.log_level;
        let self_log_level_2 = self.log_level;
        let fut = if level == Level::Debug || level == Level::Trace {
            let (parts, body) = req.into_parts();
            Either::A(
                read_body(body)
                    .map_err(ectx!(ErrorSource::Hyper, ErrorKind::Internal))
                    .and_then(move |body| {
                        log!(
                            self_log_level,
                            "HttpClient, sent request {} {}, headers: {:#?}, body: {:?}",
                            parts.method,
                            parts.uri,
                            parts.headers,
                            String::from_utf8(body.clone()).ok()
                        );
                        let req = Request::from_parts(parts, body.into());
                        cli.request(req).map_err(ectx!(ErrorSource::Hyper, ErrorKind::Internal))
                    }).and_then(|resp| {
                        let (parts, body) = resp.into_parts();
                        read_body(body)
                            .map_err(ectx!(ErrorSource::Hyper, ErrorKind::Internal))
                            .map(|body| (parts, body))
                    }).map(move |(parts, body)| {
                        log!(
                            self_log_level_2,
                            "HttpClient, recieved response with status {} headers: {:#?} and body: {:?}",
                            parts.status.as_u16(),
                            parts.headers,
                            String::from_utf8(body.clone()).ok()
                        );
                        Response::from_parts(parts, body.into())
                    }),
            )
        } else {
            Either::B(cli.request(req).map_err(ectx!(ErrorSource::Hyper, ErrorKind::Internal)))
        };

        Box::new(fut.and_then(|resp| {
            if resp.status().is_client_error() || resp.status().is_server_error() {
                let kind = match resp.status().as_u16() {
                    400 => ErrorKind::BadRequest,
                    401 => ErrorKind::Unauthorized,
                    404 => ErrorKind::NotFound,
                    500 => ErrorKind::InternalServer,
                    502 => ErrorKind::BadGateway,
                    504 => ErrorKind::GatewayTimeout,
                    _ => ErrorKind::UnknownServerError,
                };
                Either::A(
                    read_body(resp.into_body())
                        .map(|bytes| String::from_utf8(bytes).unwrap_or("Failed to read response body".to_string()))
                        .or_else(|_| Ok("Failed to read response body".to_string()))
                        .and_then(move |body_message| Err(ectx!(err ErrorSource::Server, kind => body_message))),
                )
            } else {
                Either::B(Ok(resp).into_future())
            }
        }))
    }
}
