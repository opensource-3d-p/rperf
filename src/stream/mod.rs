pub mod tcp;
pub mod udp;

use std::error::Error;
type BoxResult<T> = Result<T,Box<dyn Error>>;

pub const INTERVAL:std::time::Duration = std::time::Duration::from_secs(1);

/// tests in this system are self-contained iterator-likes, where run_interval() may be invoked multiple times
/// until it returns None, indicating that the test has run its course; each invocation blocks for up to approximately
/// INTERVAL while gathering data.
pub trait TestStream {
    /// gather data; returns None when the test is over
    fn run_interval(&mut self) -> Option<BoxResult<Box<dyn crate::protocol::results::IntervalResult + Sync + Send>>>;
    /// return the port associated with the test-stream; this may vary over the test's lifetime
    fn get_port(&self) -> BoxResult<u16>;
    /// returns the index of the test, used to match client and server data
    fn get_idx(&self) -> u8;
    /// stops a running test
    fn stop(&mut self);
}
