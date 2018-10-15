use failure::{Backtrace, Context, Fail};
use services::ErrorKind as ServiceErrorKind;
use std::fmt;
use std::fmt::Display;
use validator::ValidationErrors;

#[derive(Debug)]
pub struct Error {
    inner: Context<ErrorKind>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Fail)]
pub enum ErrorKind {
    #[fail(display = "bitcoin client error - internal error")]
    Internal,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorContext {
    #[fail(display = "bitcoin client error - converting UTF-8 from response bytes")]
    UTF8,
    #[fail(display = "bitcoin client error - converting to json struct from string")]
    Json,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ErrorSource {
    #[fail(display = "bitcoin client error - error inside of Hyper library")]
    Hyper,
}

derive_error_impls!();
