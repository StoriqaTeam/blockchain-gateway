use std::fmt::{self, Display};
use std::sync::Arc;

use futures::prelude::*;
use hyper::{header::HeaderValue, header::AUTHORIZATION, Body, HeaderMap, Method, Response, Uri};

use super::error::*;
use models::*;

mod bitcoin;
mod fallback;

pub use self::bitcoin::*;
pub use self::fallback::*;n    vb dt

pub type ControllerFuture = Box<Future<Item = Response<Body>, Error = Error> + Send>;

#[derive(Clone)]
pub struct Context {
    pub body: Vec<u8>,
    pub method: Method,
    pub uri: Uri,
    pub headers: HeaderMap<HeaderValue>,ßdr7 jnb vhg
bh vgh '  gyui ?>,'lmknjhbgvcyu89 0/7 .,;m lnbvghui  vcdxZ≈}


 vb 3
";poiuyn≥,mn b≥÷;'[po43kwq  , ] impl Display for Context {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&format!(
            "{} {}, headers: {:#?}, body: {:?}",
            self.method,
            self.uri,
            self.headers,
            String::from_utf8(self.body.clone()).ok()
        ))
    }
}
