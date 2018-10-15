use std::fmt;
use std::fmt::Display;

use client::ErrorKind as ClientErrorKind;
use failure::{Backtrace, Context, Fail};
use validator::ValidationErrors;

#[derive(Debug)]
pub struct Error {
    inner: Context<ErrorKind>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Fail)]
pub enum ErrorKind {
    #[fail(display = "service error - unauthorized")]
    Unauthorized,
    #[fail(display = "service error - malformed input")]
    MalformedInput,
    #[fail(display = "service error - not found")]
    NotFound,
    #[fail(display = "service error - invalid input, errors: {}", _0)]
    InvalidInput(ValidationErrors),
    #[fail(display = "service error - internal error")]
    Internal,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorSource {
    #[fail(display = "service error source - r2d2")]
    R2D2,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorContext {
    #[fail(display = "service error context - no auth token received")]
    NoAuthToken,
    #[fail(display = "service error context - tried to access resources that doesn't belong to user")]
    NotOwnResources,
    #[fail(display = "service error context - no wallet with this address found")]
    NoWallet,
    #[fail(display = "service error context - signing transaction")]
    SigningTransaction,
}

derive_error_impls!();

impl From<ClientErrorKind> for ErrorKind {
    fn from(e: ClientErrorKind) -> ErrorKind {
        match e {
            ClientErrorKind::BadGateway => ErrorKind::Internal,
            ClientErrorKind::BadRequest => ErrorKind::MalformedInput,
            ClientErrorKind::GatewayTimeout => ErrorKind::Internal,
            ClientErrorKind::Internal => ErrorKind::Internal,
            ClientErrorKind::InternalServer => ErrorKind::Internal,
            ClientErrorKind::NotFound => ErrorKind::NotFound,
            ClientErrorKind::Unauthorized => ErrorKind::Unauthorized,
            ClientErrorKind::UnknownServerError => ErrorKind::Internal,
            ClientErrorKind::UnprocessableEntity => ErrorKind::MalformedInput,
        }
    }
}
