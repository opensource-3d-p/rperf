#[macro_use] extern crate log;

mod tcp;
mod udp;

type BoxResult<T> = Result<T,Box<dyn Error>>;

pub trait TestStream {
    fn run_interval(&mut self) -> Option<BoxResult<IntervalResult>>;
    fn get_port(&self) -> BoxResult<u16>;
    fn stop(&mut self);
}
