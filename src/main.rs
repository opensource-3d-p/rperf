extern crate log;
extern crate env_logger;
extern crate lazy_static;

use clap::{App, Arg};

mod protocol;
mod stream;
mod utils;
mod client;
mod server;

fn main() {
    let args = App::new("rperf")
        .version("0.0.1")
        .about("validates network throughput capacity and reliability")
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
            Arg::with_name("affinity")
                .help("specify logical CPUs, delimited by commas, across which to round-robin affinity; not supported on all systems")
                .takes_value(true)
                .long("affinity")
                .short("A")
                .required(false)
                .multiple(true)
                .default_value("")
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
            Arg::with_name("version6")
                .help("enable IPv6 on the server (on most hosts, this will allow both IPv4 and IPv6, but it might limit to just IPv6 on some)")
                .takes_value(false)
                .long("version6")
                .short("6")
                .required(false)
        )
        
        .arg(
            Arg::with_name("client")
                .help("run in client mode; value is the server's address")
                .takes_value(true)
                .long("client")
                .short("c")
                .required(false)
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
            Arg::with_name("format")
                .help("the format in which to deplay information (json, megabit/sec, megabyte/sec)")
                .takes_value(true)
                .long("format")
                .short("f")
                .required(false)
                .default_value("megabyte")
                .possible_values(&["json", "megabit", "megabyte"])
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
                .help("target bandwidth in bits/sec; this value is applied to each stream, with a default target of 1 megabit/second for all protocols (note: megabit, not mebibit)")
                .takes_value(true)
                .long("bandwidth")
                .short("b")
                .required(false)
                .default_value("1000000")
        )
        .arg(
            Arg::with_name("time")
                .help("the time in seconds for which to transmit")
                .takes_value(true)
                .long("time")
                .short("t")
                .required(false)
                .default_value("10.0")
        )
        .arg(
            Arg::with_name("sendinterval")
                .help("the interval at which to send batches of data, in seconds; this is used to evenly spread packets out over time")
                .takes_value(true)
                .long("send-interval")
                .required(false)
                .default_value("0.05")
        )
        .arg(
            Arg::with_name("length")
                .help("length of the buffer to exchange; for TCP, this defaults to 32 kibibytes; for UDP, it's 1024 bytes")
                .takes_value(true)
                .long("length")
                .short("l")
                .required(false)
                .default_value("0")
        )
        .arg(
            Arg::with_name("send_buffer")
                .help("send_buffer, in bytes; affects TCP window-size (only supported on some platforms; if set too small, a 'resource unavailable' error may occur)")
                .takes_value(true)
                .long("send-buffer")
                .required(false)
                .default_value("0")
        )
        .arg(
            Arg::with_name("receive_buffer")
                .help("receive_buffer, in bytes; affects TCP window-size (only supported on some platforms; if set too small, a 'resource unavailable' error may occur)")
                .takes_value(true)
                .long("receive-buffer")
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
            Arg::with_name("omit")
                .help("omit a number of seconds from the start of calculations, primarily to avoid including TCP ramp-up in averages")
                .takes_value(true)
                .long("omit")
                .short("O")
                .default_value("0")
                .required(false)
        )
        .arg(
            Arg::with_name("no_delay")
                .help("use no-delay mode for TCP tests, disabling Nagle's Algorithm")
                .takes_value(false)
                .long("no-delay")
                .short("N")
                .required(false)
        )
    .get_matches();
    
    let mut env = env_logger::Env::default()
        .filter_or("RUST_LOG", "info");
    if args.is_present("debug") {
        env = env.filter_or("RUST_LOG", "debug");
    }
    env_logger::init_from_env(env);
    
    if args.is_present("server") {
        log::debug!("registering SIGINT handler...");
        ctrlc::set_handler(move || {
            if server::kill() {
                log::warn!("shutdown requested; please allow a moment for any in-progress tests to stop");
            } else {
                log::warn!("forcing shutdown immediately");
                std::process::exit(3);
            }
        }).expect("unable to set SIGINT handler");
        
        log::debug!("beginning normal operation...");
        let service = server::serve(args);
        if service.is_err() {
            log::error!("unable to run server: {}", service.unwrap_err());
        }
    } else if args.is_present("client") {
        log::debug!("registering SIGINT handler...");
        ctrlc::set_handler(move || {
            if client::kill() {
                log::warn!("shutdown requested; please allow a moment for any in-progress tests to stop");
            } else {
                log::warn!("forcing shutdown immediately");
                std::process::exit(3);
            }
        }).expect("unable to set SIGINT handler");
        
        log::debug!("connecting to server...");
        let execution = client::execute(args);
        if execution.is_err() {
            log::error!("unable to run client: {}", execution.unwrap_err());
        }
    } else {
        std::println!("{}", args.usage());
        std::process::exit(2);
    }
}
