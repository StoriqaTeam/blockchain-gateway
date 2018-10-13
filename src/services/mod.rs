mod error;
#[cfg(test)]
mod mocks;

pub use self::error::*;
#[cfg(test)]
pub use self::mocks::*;

use prelude::*;

type ServiceFuture<T> = Box<Future<Item = T, Error = Error> + Send>;
