extern crate log;
extern crate env_logger;
extern crate lazy_static;

use clap::{App, Arg};

mod client;
mod server;
mod protocol;

fn main() {
    env_logger::init();
    
    let matches = App::new("rperf")
        .arg(
            Arg::with_name("version")
                .help("display version")
                .takes_value(false)
                .long("version")
                .short("v")
                .required(false)
        )
        .arg(
            Arg::with_name("port")
                .help("the port used for client-server interactions")
                .takes_value(true)
                .long("port")
                .short("p")
                .required(false)
                .default_value("5201")
        )
        .arg(
            Arg::with_name("controltimeout")
                .help("the number of seconds to wait before assuming the control-channel has been broken")
                .takes_value(true)
                .long("control-timeout")
                .required(false)
                .default_value("2.5")
        )
        .arg(
            Arg::with_name("affinity")
                .help("specify logical CPUs, delimited by commas, across which to round-robin affinity; not supported on all systems")
                .takes_value(true)
                .long("file")
                .short("A")
                .required(false)
                .multiple(true)
                .default_value("-1")
        )
        .arg(
            Arg::with_name("verbose")
                .help("provide more information with any non-JSON output")
                .takes_value(false)
                .long("verbose")
                .short("V")
                .required(false)
        )
        .arg(
            Arg::with_name("debug")
                .help("emit debug-level logging on stderr; default is info and above")
                .takes_value(false)
                .long("debug")
                .short("d")
                .required(false)
        )
        
        
        .arg(
            Arg::with_name("server")
                .help("run in server mode")
                .takes_value(false)
                .long("server")
                .short("s")
                .required(false)
        )
        
        
        .arg(
            Arg::with_name("format")
                .help("the format in which to deplay information (json, megabit/sec, megabyte/sec)")
                .takes_value(true)
                .long("format")
                .short("f")
                .required(false)
                .default_value("json")
                .possible_values(&["json", "bit", "byte"])
        )
        .arg(
            Arg::with_name("interval")
                .help("the number of seconds to wait between periodic status reports in non-JSON modes; 0 to disable")
                .takes_value(true)
                .long("interval")
                .short("i")
                .required(false)
                .default_value("0")
        )
        .arg(
            Arg::with_name("udp")
                .help("use UDP rather than TCP")
                .takes_value(false)
                .long("udp")
                .short("u")
                .required(false)
        )
        .arg(
            Arg::with_name("bandwidth")
                .help("target bandwidth in bits/sec; this value is applied to each stream, with a default target of 1 megabit/second for all protocols (note: megabit, not mebibit, and unlike iperf3, data is evenly distributed by resolution, not frontloaded)")
                .takes_value(true)
                .long("bandwidth")
                .short("b")
                .required(false)
                .default_value("1000000")
        )
        .arg(
            Arg::with_name("time")
                .help("the time in seconds for which to transmit, as an alternative to bytes; time is used by default")
                .takes_value(true)
                .long("time")
                .short("t")
                .required(false)
                .default_value("10.0")
                .conflicts_with("bytes")
        )
        .arg(
            Arg::with_name("bytes")
                .help("the number of bytes to transmit, as an alternative to time")
                .takes_value(true)
                .long("bytes")
                .short("y")
                .required(false)
                .default_value("0")
                .conflicts_with("time")
        )
        .arg(
            Arg::with_name("omit")
                .help("omit a number of seconds from the start of calculations, in non-JSON modes, to avoid including TCP ramp-up in averages")
                .takes_value(true)
                .long("omit")
                .short("O")
                .default_value("0.0")
                .required(false)
        )
        .arg(
            Arg::with_name("length")
                .help("length of the buffer to exchange; for TCP, this defaults to 128kilobytes; for UDP, it's based on MTU and IP family")
                .takes_value(true)
                .long("blockcount")
                .short("k")
                .required(false)
                .default_value("0")
        )
        .arg(
            Arg::with_name("parallel")
                .help("the number of parallel data-streams to use")
                .takes_value(true)
                .long("parallel")
                .short("P")
                .required(false)
                .default_value("1")
        )
        .arg(
            Arg::with_name("reverse")
                .help("run in reverse-mode (server sends, client receives)")
                .takes_value(false)
                .long("reverse")
                .short("R")
                .required(false)
        )
        .arg(
            Arg::with_name("window")
                .help("window-size, in bytes, for TCP tests")
                .takes_value(false)
                .long("window")
                .short("w")
                .required(false)
        )
        .arg(
            Arg::with_name("mss")
                .help("maximum segment-size, for TCP tests (default is based on MTU)")
                .takes_value(false)
                .long("mss")
                .short("M")
                .required(false)
        )
        .arg(
            Arg::with_name("nodelay")
                .help("use no-delay mode for TCP tests, deisabling Nagle's Algorithm")
                .takes_value(false)
                .long("no-delay")
                .short("N")
                .required(false)
        )
        .arg(
            Arg::with_name("congestion")
                .help("use a specific congestion-control algorithm for traffic-shaping")
                .takes_value(false)
                .long("congestion")
                .short("C")
                .required(false)
        )
        .arg(
            Arg::with_name("version6")
                .help("use IPv6")
                .takes_value(false)
                .long("version6")
                .short("6")
                .required(false)
        )
        .arg(
            Arg::with_name("sendinterval")
                .help("the interval at which to send data, in seconds")
                .takes_value(true)
                .long("send-interval")
                .required(false)
                .default_value("0.01")
        )
    .get_matches();
    
    
    if matches.is_present("server") {
        debug!("registering SIGINT handler...");
        ctrlc::set_handler(move || {
            if server::kill() {
                log::warn!("shutdown requested; please allow a moment for any in-progress tests to stop");
            } else {
                log::warn!("forcing shutdown immediately");
                std::process::exit(3);
            }
        }).expect("unable to set SIGINT handler");
        
        debug!("beginning normal operation...");
        let service = server::serve(&(50002 as u16), &(4 as u8));
        if service.is_err() {
            log::error!("unable to run server: {:?}", service.unwrap_err());
        }
    } else {
        debug!("registering SIGINT handler...");
        ctrlc::set_handler(move || {
            if client::kill() {
                log::warn!("shutdown requested; please allow a moment for any in-progress tests to stop");
            } else {
                log::warn!("forcing shutdown immediately");
                std::process::exit(3);
            }
        }).expect("unable to set SIGINT handler");
        
        debug!("connecting to server...");
        let execution = client::execute("127.0.0.1", &(50002 as u16), &(4 as u8));
        if execution.is_err() {
            log::error!("unable to run client: {:?}", execution.unwrap_err());
        }
    }
}
