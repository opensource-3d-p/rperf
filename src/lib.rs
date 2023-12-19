pub mod args;
pub mod client;
pub(crate) mod protocol;
pub mod server;
pub(crate) mod stream;
pub(crate) mod utils;

pub type BoxResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

/// a global token generator
pub(crate) fn get_global_token() -> mio::Token {
    mio::Token(TOKEN_SEED.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1)
}
static TOKEN_SEED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
