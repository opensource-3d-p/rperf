/*
 * Copyright (C) 2021 Evtech Solutions, Ltd., dba 3D-P
 * Copyright (C) 2021 Neil Tallim <neiltallim@3d-p.com>
 *
 * This file is part of rperf.
 *
 * rperf is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * rperf is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with rperf.  If not, see <https://www.gnu.org/licenses/>.
 */

extern crate env_logger;
extern crate log;

use clap::{App, Arg};

mod client;
mod protocol;
mod server;
mod stream;
mod utils;

fn main() {
    let args = App::new("rperf")
        .about(clap::crate_description!())
        .author("https://github.com/opensource-3d-p/rperf")
        .name(clap::crate_name!())
        .version(clap::crate_version!())
        .arg(
            Arg::with_name("port")
                .help("the port used for client-server interactions")
                .takes_value(true)
                .long("port")
                .short("p")
                .required(false)
                .default_value("5199")
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
            Arg::with_name("client_limit")
                .help("limit the number of concurrent clients that can be processed by a server; any over this count will be immediately disconnected")
                .takes_value(true)
                .long("client-limit")
                .required(false)
                .default_value("0")
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
                .default_value("megabit")
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
                .help("target bandwidth in bytes/sec; this value is applied to each stream, with a default target of 1 megabit/second for all protocols (note: megabit, not mebibit); the suffixes kKmMgG can also be used for xbit and xbyte, respectively")
                .takes_value(true)
                .long("bandwidth")
                .short("b")
                .required(false)
                .default_value("125000")
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
            Arg::with_name("send_interval")
                .help("the interval at which to send batches of data, in seconds, between [0.0 and 1.0); this is used to evenly spread packets out over time")
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
                .help("send_buffer, in bytes (only supported on some platforms; if set too small, a 'resource unavailable' error may occur; affects TCP window-size)")
                .takes_value(true)
                .long("send-buffer")
                .required(false)
                .default_value("0")
        )
        .arg(
            Arg::with_name("receive_buffer")
                .help("receive_buffer, in bytes (only supported on some platforms; if set too small, a 'resource unavailable' error may occur; affects TCP window-size)")
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
                .help("omit a number of seconds from the start of calculations, primarily to avoid including TCP ramp-up in averages; using this option may result in disagreement between bytes sent and received, since data can be in-flight across time-boundaries")
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
        .arg(
            Arg::with_name("tcp_port_pool")
                .help("an optional pool of IPv4 TCP ports over which data will be accepted; if omitted, any OS-assignable port is used; format: 1-10,19,21")
                .takes_value(true)
                .long("tcp-port-pool")
                .required(false)
                .default_value("")
        )
        .arg(
            Arg::with_name("tcp6_port_pool")
                .help("an optional pool of IPv6 TCP ports over which data will be accepted; if omitted, any OS-assignable port is used; format: 1-10,19,21")
                .takes_value(true)
                .long("tcp6-port-pool")
                .required(false)
                .default_value("")
        )
        .arg(
            Arg::with_name("udp_port_pool")
                .help("an optional pool of IPv4 UDP ports over which data will be accepted; if omitted, any OS-assignable port is used; format: 1-10,19,21")
                .takes_value(true)
                .long("udp-port-pool")
                .required(false)
                .default_value("")
        )
        .arg(
            Arg::with_name("udp6_port_pool")
                .help("an optional pool of IPv6 UDP ports over which data will be accepted; if omitted, any OS-assignable port is used; format: 1-10,19,21")
                .takes_value(true)
                .long("udp6-port-pool")
                .required(false)
                .default_value("")
        )
    .get_matches();

    let mut env = env_logger::Env::default().filter_or("RUST_LOG", "info");
    if args.is_present("debug") {
        env = env.filter_or("RUST_LOG", "debug");
    }
    env_logger::init_from_env(env);

    if args.is_present("server") {
        log::debug!("registering SIGINT handler...");
        ctrlc::set_handler(move || {
            if server::kill() {
                log::warn!(
                    "shutdown requested; please allow a moment for any in-progress tests to stop"
                );
            } else {
                log::warn!("forcing shutdown immediately");
                std::process::exit(3);
            }
        })
        .expect("unable to set SIGINT handler");

        log::debug!("beginning normal operation...");
        let service = server::serve(args);
        if service.is_err() {
            log::error!("unable to run server: {}", service.unwrap_err());
            std::process::exit(4);
        }
    } else if args.is_present("client") {
        log::debug!("registering SIGINT handler...");
        ctrlc::set_handler(move || {
            if client::kill() {
                log::warn!(
                    "shutdown requested; please allow a moment for any in-progress tests to stop"
                );
            } else {
                log::warn!("forcing shutdown immediately");
                std::process::exit(3);
            }
        })
        .expect("unable to set SIGINT handler");

        log::debug!("connecting to server...");
        let execution = client::execute(args);
        if execution.is_err() {
            log::error!("unable to run client: {}", execution.unwrap_err());
            std::process::exit(4);
        }
    } else {
        std::println!("{}", args.usage());
        std::process::exit(2);
    }
}
