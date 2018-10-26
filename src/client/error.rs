use failure::{Backtrace, Context, Fail};
use std::fmt;
use std::fmt::Display;

#[derive(Debug)]
pub struct Error {
    inner: Context<ErrorKind>,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorKind {
    #[fail(display = "http client error - bad request")]
    BadRequest,
    #[fail(display = "http client error - unauthorized")]
    Unauthorized,
    #[fail(display = "http client error - not found")]
    NotFound,
    #[fail(display = "http client error - unprocessable entity")]
    UnprocessableEntity,
    #[fail(display = "http client error - internal server error")]
    InternalServer,
    #[fail(display = "http client error - bad gateway")]
    BadGateway,
    #[fail(display = "http client error - timeout")]
    GatewayTimeout,
    #[fail(display = "http client error - unknown server error status")]
    UnknownServerError,
    #[fail(display = "http client error - internal error")]
    Internal,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorContext {
    #[fail(display = "http client error - operations overflow")]
    Overflow,
    #[fail(display = "http client error - converting UTF-8 from response bytes")]
    UTF8,
    #[fail(display = "http client error - converting to json struct from string")]
    Json,
    #[fail(display = "http client error - parsing hex string")]
    Hex,
    #[fail(display = "http client error - unexpected number of log topics in ethereum log receipt")]
    Topics,
    #[fail(display = "http client error - error converting rpc transaction into blockchain transaction")]
    BitcoinRpcConversion,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorSource {
    #[fail(display = "http client source - error inside of Hyper library")]
    Hyper,
    #[fail(display = "http client source - server returned response with error")]
    Server,
}

derive_error_impls!();
