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

use rperf::{args, client, server, BoxResult};

fn main() -> BoxResult<()> {
    use clap::Parser;
    let args = args::Args::parse();

    let default = args.verbosity.to_string();
    let mut env = env_logger::Env::default().filter_or("RUST_LOG", &default);
    if args.debug {
        env = env.filter_or("RUST_LOG", "debug");
    }
    env_logger::init_from_env(env);

    if args.server {
        log::debug!("registering SIGINT handler...");
        let exiting = ctrlc2::set_handler(move || {
            if server::kill() {
                log::warn!("shutdown requested; please allow a moment for any in-progress tests to stop");
            } else {
                log::warn!("forcing shutdown immediately");
                std::process::exit(3);
            }
            true
        })?;

        log::debug!("beginning normal operation...");
        server::serve(&args)?;
        exiting.join().expect("unable to join SIGINT handler thread");
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
        })?;

        log::debug!("connecting to server...");
        client::execute(&args)?;
    } else {
        use clap::CommandFactory;
        let mut cmd = args::Args::command();
        cmd.print_help().unwrap();
    }
    Ok(())
}
