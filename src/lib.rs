pub mod args;
pub mod client;
pub(crate) mod protocol;
pub mod server;
pub(crate) mod stream;
pub(crate) mod utils;

pub(crate) type BoxResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;
