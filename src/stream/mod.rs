extern crate log;

pub mod tcp;
pub mod udp;

use std::error::Error;

type BoxResult<T> = Result<T,Box<dyn Error>>;

pub trait TestStream {
    fn run_interval(&mut self) -> Option<BoxResult<Box<dyn crate::protocol::results::IntervalResult>>>;
    fn get_port(&self) -> BoxResult<u16>;
    fn stop(&mut self);
}
