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

mod args;
mod client;
mod protocol;
mod server;
mod stream;
mod utils;

fn main() {
    use clap::Parser;
    let args = args::Args::parse();

    let mut env = env_logger::Env::default().filter_or("RUST_LOG", "info");
    if args.debug {
        env = env.filter_or("RUST_LOG", "debug");
    }
    env_logger::init_from_env(env);

    if args.server {
        log::debug!("registering SIGINT handler...");
        ctrlc2::set_handler(move || {
            if server::kill() {
                log::warn!("shutdown requested; please allow a moment for any in-progress tests to stop");
            } else {
                log::warn!("forcing shutdown immediately");
                std::process::exit(3);
            }
            true
        })
        .expect("unable to set SIGINT handler");

        log::debug!("beginning normal operation...");
        let service = server::serve(&args);
        if service.is_err() {
            log::error!("unable to run server: {}", service.unwrap_err());
            std::process::exit(4);
        }
    } else if args.client.is_some() {
        log::debug!("registering SIGINT handler...");
        ctrlc2::set_handler(move || {
            if client::kill() {
                log::warn!("shutdown requested; please allow a moment for any in-progress tests to stop");
            } else {
                log::warn!("forcing shutdown immediately");
                std::process::exit(3);
            }
            true
        })
        .expect("unable to set SIGINT handler");

        log::debug!("connecting to server...");
        let execution = client::execute(&args);
        if execution.is_err() {
            log::error!("unable to run client: {}", execution.unwrap_err());
            std::process::exit(4);
        }
    } else {
        use clap::CommandFactory;
        let mut cmd = args::Args::command();
        cmd.print_help().unwrap();
        std::process::exit(2);
    }
}
