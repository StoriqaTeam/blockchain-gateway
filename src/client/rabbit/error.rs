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
    #[fail(display = "rabbit error - internal error")]
    Internal,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorSource {
    #[fail(display = "rabbit error source - io error")]
    Io,
    #[fail(display = "rabbit error source - timeout error")]
    Timeout,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorContext {
    #[fail(display = "rabbit error context - error establishing TCP/IP connection")]
    TcpConnection,
    #[fail(display = "rabbit error context - error establishing RabbitMQ connection")]
    RabbitConnection,
    #[fail(display = "rabbit error context - error acquiring heartbeat handle")]
    HeartbeatHandle,
    #[fail(display = "rabbit error context - connection timeout")]
    ConnectionTimeout,
}

derive_error_impls!();
