/*
 * Copyright (C) 2026 Evtech Solutions, Ltd., dba 3D-P
 * Copyright (C) 2026 Neil Tallim <neiltallim@3d-p.com>
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

pub mod tcp;
pub mod udp;

use std::error::Error;
type BoxResult<T> = Result<T, Box<dyn Error>>;

pub const INTERVAL: std::time::Duration = std::time::Duration::from_secs(1);

/// tests in this system are self-contained iterator-likes, where run_interval() may be invoked multiple times
/// until it returns None, indicating that the test has run its course; each invocation blocks for up to approximately
/// INTERVAL while gathering data.
pub trait TestStream {
    /// gather data; returns None when the test is over
    fn run_interval(
        &mut self,
    ) -> Option<BoxResult<Box<dyn crate::protocol::results::IntervalResult + Sync + Send>>>;
    /// return the port associated with the test-stream; this may vary over the test's lifetime
    fn get_port(&self) -> BoxResult<u16>;
    /// returns the index of the test, used to match client and server data
    fn get_idx(&self) -> u8;
    /// stops a running test
    fn stop(&mut self);
}

fn parse_port_spec(port_spec: String) -> Vec<u16> {
    let mut ports = Vec::<u16>::new();
    if !port_spec.is_empty() {
        for range in port_spec.split(',') {
            if range.contains('-') {
                let mut range_spec = range.split('-');
                let range_first = range_spec.next().unwrap().parse::<u16>().unwrap();
                let range_last = range_spec.last().unwrap().parse::<u16>().unwrap();

                for port in range_first..=range_last {
                    ports.push(port);
                }
            } else {
                ports.push(range.parse::<u16>().unwrap());
            }
        }

        ports.sort();
    }

    return ports;
}

//AI-generated tests
#[cfg(test)]
mod tests {
    use super::parse_port_spec;

    #[test]
    fn empty_spec_yields_no_ports() {
        assert_eq!(parse_port_spec(String::new()), Vec::<u16>::new());
    }

    #[test]
    fn single_port() {
        assert_eq!(parse_port_spec("5199".into()), vec![5199]);
    }

    #[test]
    fn multiple_ports_are_sorted() {
        assert_eq!(
            parse_port_spec("5201,5197,5199".into()),
            vec![5197, 5199, 5201]
        );
    }

    #[test]
    fn port_range_expands() {
        assert_eq!(parse_port_spec("5197-5199".into()), vec![5197, 5198, 5199]);
    }

    #[test]
    fn mixed_ranges_and_ports_are_sorted() {
        assert_eq!(
            parse_port_spec("5201,5197-5198".into()),
            vec![5197, 5198, 5201]
        );
    }

    #[test]
    #[should_panic]
    fn non_numeric_spec_panics() {
        //parse_port_spec uses unwraps internally; malformed specs are expected to be caught upstream
        let _ = parse_port_spec("abc".into());
    }
}
