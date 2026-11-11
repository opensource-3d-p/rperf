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

use std::error::Error;
type BoxResult<T> = Result<T, Box<dyn Error>>;

/// prepares a message used to tell the server to begin operations
pub fn prepare_begin() -> serde_json::Value {
    serde_json::json!({
        "kind": "begin",
    })
}

/// prepares a message used to tell the client to connect its test-streams
pub fn prepare_connect(stream_ports: &[u16]) -> serde_json::Value {
    serde_json::json!({
        "kind": "connect",

        "stream_ports": stream_ports,
    })
}

/// prepares a message used to tell the client that the server is ready to connect to its test-streams
pub fn prepare_connect_ready() -> serde_json::Value {
    serde_json::json!({
        "kind": "connect-ready",
    })
}

/// prepares a message used to tell the server that testing is finished
pub fn prepare_end() -> serde_json::Value {
    serde_json::json!({
        "kind": "end",
    })
}

/// prepares a message used to describe the upload role of a TCP test
fn prepare_configuration_tcp_upload(
    test_id: &[u8; 16],
    streams: u8,
    bandwidth: u64,
    seconds: f32,
    length: usize,
    send_interval: f32,
    send_buffer: u32,
    no_delay: bool,
) -> serde_json::Value {
    serde_json::json!({
        "kind": "configuration",

        "family": "tcp",
        "role": "upload",

        "test_id": test_id,
        "streams": validate_streams(streams),

        "bandwidth": validate_bandwidth(bandwidth),
        "duration": seconds,
        "length": calculate_length_tcp(length),
        "send_interval": validate_send_interval(send_interval),

        "send_buffer": send_buffer,
        "no_delay": no_delay,
    })
}

/// prepares a message used to describe the download role of a TCP test
fn prepare_configuration_tcp_download(
    test_id: &[u8; 16],
    streams: u8,
    length: usize,
    receive_buffer: u32,
) -> serde_json::Value {
    serde_json::json!({
        "kind": "configuration",

        "family": "tcp",
        "role": "download",

        "test_id": test_id,
        "streams": validate_streams(streams),

        "length": calculate_length_tcp(length),
        "receive_buffer": receive_buffer,
    })
}

/// prepares a message used to describe the upload role of a UDP test
fn prepare_configuration_udp_upload(
    test_id: &[u8; 16],
    streams: u8,
    bandwidth: u64,
    seconds: f32,
    length: u16,
    send_interval: f32,
    send_buffer: u32,
) -> serde_json::Value {
    serde_json::json!({
        "kind": "configuration",

        "family": "udp",
        "role": "upload",

        "test_id": test_id,
        "streams": validate_streams(streams),

        "bandwidth": validate_bandwidth(bandwidth),
        "duration": seconds,
        "length": calculate_length_udp(length),
        "send_interval": validate_send_interval(send_interval),

        "send_buffer": send_buffer,
    })
}

/// prepares a message used to describe the download role of a UDP test
fn prepare_configuration_udp_download(
    test_id: &[u8; 16],
    streams: u8,
    length: u16,
    receive_buffer: u32,
) -> serde_json::Value {
    serde_json::json!({
        "kind": "configuration",

        "family": "udp",
        "role": "download",

        "test_id": test_id,
        "streams": validate_streams(streams),

        "length": calculate_length_udp(length),
        "receive_buffer": receive_buffer,
    })
}

fn validate_streams(streams: u8) -> u8 {
    if streams > 0 {
        streams
    } else {
        log::warn!("parallel streams not specified; defaulting to 1");
        1
    }
}

/// the default bandwidth target, in bytes/second (1 megabit/second),
/// shared with the CLI so the two can't drift
pub const DEFAULT_BANDWIDTH: u64 = 125000;

fn validate_bandwidth(bandwidth: u64) -> u64 {
    if bandwidth > 0 {
        bandwidth
    } else {
        log::warn!(
            "bandwidth was not specified; defaulting to {} bytes/second ({} megabit/second)",
            DEFAULT_BANDWIDTH,
            DEFAULT_BANDWIDTH / DEFAULT_BANDWIDTH,
        );
        DEFAULT_BANDWIDTH
    }
}

fn validate_send_interval(send_interval: f32) -> f32 {
    if send_interval > 0.0 && send_interval <= 1.0 {
        send_interval
    } else {
        log::warn!("send-interval was invalid or not specified; defaulting to once per second");
        1.0
    }
}

fn calculate_length_tcp(length: usize) -> usize {
    if length < crate::stream::tcp::TEST_HEADER_SIZE {
        //length must be at least enough to hold the test data
        crate::stream::tcp::TEST_HEADER_SIZE
    } else {
        length
    }
}
fn calculate_length_udp(length: u16) -> u16 {
    if length < crate::stream::udp::TEST_HEADER_SIZE {
        //length must be at least enough to hold the test data
        crate::stream::udp::TEST_HEADER_SIZE
    } else {
        length
    }
}

/// prepares a message used to describe the upload role in a test
pub fn prepare_upload_configuration(
    args: &clap::ArgMatches,
    test_id: &[u8; 16],
) -> BoxResult<serde_json::Value> {
    let parallel_streams: u8 = args.value_of("parallel").unwrap().parse()?;
    let mut seconds: f32 = args.value_of("time").unwrap().parse()?;
    let mut send_interval: f32 = args.value_of("send_interval").unwrap().parse()?;
    let mut length: u32 = args.value_of("length").unwrap().parse()?;

    let mut send_buffer: u32 = args.value_of("send_buffer").unwrap().parse()?;

    let mut bandwidth_string = args.value_of("bandwidth").unwrap();
    let bandwidth: u64;
    let bandwidth_multiplier: f64;
    match bandwidth_string.chars().last() {
        Some(v) => {
            match v {
                'k' => {
                    //kilobits
                    bandwidth_multiplier = 1000.0 / 8.0;
                }
                'K' => {
                    //kilobytes
                    bandwidth_multiplier = 1000.0;
                }
                'm' => {
                    //megabits
                    bandwidth_multiplier = 1000.0 * 1000.0 / 8.0;
                }
                'M' => {
                    //megabytes
                    bandwidth_multiplier = 1000.0 * 1000.0;
                }
                'g' => {
                    //gigabits
                    bandwidth_multiplier = 1000.0 * 1000.0 * 1000.0 / 8.0;
                }
                'G' => {
                    //gigabytes
                    bandwidth_multiplier = 1000.0 * 1000.0 * 1000.0;
                }
                _ => {
                    bandwidth_multiplier = 1.0;
                }
            }

            if bandwidth_multiplier != 1.0 {
                //the value uses a suffix
                bandwidth_string = &bandwidth_string[0..(bandwidth_string.len() - 1)];
            }

            match bandwidth_string.parse::<f64>() {
                Ok(v2) => {
                    bandwidth = (v2 * bandwidth_multiplier) as u64;
                }
                Err(_) => {
                    //invalid input; fall back to 1mbps
                    log::warn!(
                        "invalid bandwidth: {}; setting value to 1mbps",
                        args.value_of("bandwidth").unwrap()
                    );
                    bandwidth = 125000;
                }
            }
        }
        None => {
            //invalid input; fall back to 1mbps
            log::warn!(
                "invalid bandwidth: {}; setting value to 1mbps",
                args.value_of("bandwidth").unwrap()
            );
            bandwidth = 125000;
        }
    }

    if seconds <= 0.0 {
        log::warn!("time was not in an acceptable range and has been set to 0.0");
        seconds = 0.0
    }

    if send_interval > 1.0 || send_interval <= 0.0 {
        log::warn!("send-interval was not in an acceptable range and has been set to 0.05");
        send_interval = 0.05
    }

    if args.is_present("udp") {
        log::debug!("preparing UDP upload config...");
        if length == 0 {
            length = 1024;
        }
        if send_buffer != 0 && send_buffer < length {
            log::warn!("requested send-buffer, {}, is too small to hold the data to be exchanged; it will be increased to {}", send_buffer, length * 2);
            send_buffer = length * 2;
        }
        Ok(prepare_configuration_udp_upload(
            test_id,
            parallel_streams,
            bandwidth,
            seconds,
            length as u16,
            send_interval,
            send_buffer,
        ))
    } else {
        log::debug!("preparing TCP upload config...");
        if length == 0 {
            length = 32 * 1024;
        }
        if send_buffer != 0 && send_buffer < length {
            log::warn!("requested send-buffer, {}, is too small to hold the data to be exchanged; it will be increased to {}", send_buffer, length * 2);
            send_buffer = length * 2;
        }

        let no_delay: bool = args.is_present("no_delay");

        Ok(prepare_configuration_tcp_upload(
            test_id,
            parallel_streams,
            bandwidth,
            seconds,
            length as usize,
            send_interval,
            send_buffer,
            no_delay,
        ))
    }
}
/// prepares a message used to describe the download role in a test
pub fn prepare_download_configuration(
    args: &clap::ArgMatches,
    test_id: &[u8; 16],
) -> BoxResult<serde_json::Value> {
    let parallel_streams: u8 = args.value_of("parallel").unwrap().parse()?;
    let mut length: u32 = args.value_of("length").unwrap().parse()?;
    let mut receive_buffer: u32 = args.value_of("receive_buffer").unwrap().parse()?;

    if args.is_present("udp") {
        log::debug!("preparing UDP download config...");
        if length == 0 {
            length = 1024;
        }
        if receive_buffer != 0 && receive_buffer < length {
            log::warn!("requested receive-buffer, {}, is too small to hold the data to be exchanged; it will be increased to {}", receive_buffer, length * 2);
            receive_buffer = length * 2;
        }
        Ok(prepare_configuration_udp_download(
            test_id,
            parallel_streams,
            length as u16,
            receive_buffer,
        ))
    } else {
        log::debug!("preparing TCP download config...");
        if length == 0 {
            length = 32 * 1024;
        }
        if receive_buffer != 0 && receive_buffer < length {
            log::warn!("requested receive-buffer, {}, is too small to hold the data to be exchanged; it will be increased to {}", receive_buffer, length * 2);
            receive_buffer = length * 2;
        }
        Ok(prepare_configuration_tcp_download(
            test_id,
            parallel_streams,
            length as usize,
            receive_buffer,
        ))
    }
}

//AI-generated tests
#[cfg(test)]
mod tests {
    use super::*;

    const TEST_ID: [u8; 16] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];

    #[test]
    fn simple_messages_have_expected_shape() {
        assert_eq!(prepare_begin(), serde_json::json!({"kind": "begin"}));
        assert_eq!(
            prepare_connect_ready(),
            serde_json::json!({"kind": "connect-ready"})
        );
        assert_eq!(prepare_end(), serde_json::json!({"kind": "end"}));
        assert_eq!(
            prepare_connect(&[5199, 5200]),
            serde_json::json!({"kind": "connect", "stream_ports": [5199, 5200]})
        );
    }

    fn upload_args<'a>(extra: &'a [&'a str]) -> clap::ArgMatches<'a> {
        let mut argv: Vec<&str> = vec!["rperf-test"];
        argv.extend_from_slice(extra);
        clap::App::new("rperf-test")
            .arg(
                clap::Arg::with_name("parallel")
                    .takes_value(true)
                    .short("P")
                    .default_value("1"),
            )
            .arg(
                clap::Arg::with_name("time")
                    .takes_value(true)
                    .default_value("10.0"),
            )
            .arg(
                clap::Arg::with_name("send_interval")
                    .takes_value(true)
                    .long("send_interval")
                    .default_value("0.05"),
            )
            .arg(
                clap::Arg::with_name("length")
                    .takes_value(true)
                    .default_value("0"),
            )
            .arg(
                clap::Arg::with_name("send_buffer")
                    .takes_value(true)
                    .long("send_buffer")
                    .default_value("0"),
            )
            .arg(
                clap::Arg::with_name("bandwidth")
                    .takes_value(true)
                    .short("b")
                    .default_value("125000"),
            )
            .arg(clap::Arg::with_name("udp").takes_value(false).long("udp"))
            .arg(
                clap::Arg::with_name("no_delay")
                    .takes_value(false)
                    .short("N"),
            )
            .get_matches_from(argv)
    }

    #[test]
    fn tcp_upload_configuration_defaults() {
        let config = prepare_upload_configuration(&upload_args(&[]), &TEST_ID).unwrap();

        assert_eq!(config["kind"], "configuration");
        assert_eq!(config["family"], "tcp");
        assert_eq!(config["role"], "upload");
        assert_eq!(config["test_id"], serde_json::json!(TEST_ID.to_vec()));
        assert_eq!(config["streams"], 1);
        assert_eq!(config["bandwidth"], 125000);
        //TCP defaults to a 32 kibibyte buffer
        assert_eq!(config["length"], 32 * 1024);
        assert_eq!(config["no_delay"], false);
    }

    #[test]
    fn udp_upload_configuration_defaults() {
        let config = prepare_upload_configuration(&upload_args(&["--udp"]), &TEST_ID).unwrap();

        assert_eq!(config["family"], "udp");
        //UDP defaults to a 1024 byte buffer
        assert_eq!(config["length"], 1024);
    }

    #[test]
    fn bandwidth_suffixes_parse_to_bytes_per_second() {
        //kilobit, kilobyte, megabit, megabyte, gigabit, gigabyte, and plain bytes
        let cases: &[(&str, u64)] = &[
            ("1k", 125),
            ("1K", 1000),
            ("1m", 125000),
            ("1M", 1_000_000),
            ("1g", 125_000_000),
            ("1G", 1_000_000_000),
            ("500000", 500000),
        ];
        for (input, expected) in cases {
            let config =
                prepare_upload_configuration(&upload_args(&["-b", input]), &TEST_ID).unwrap();
            assert_eq!(config["bandwidth"], *expected, "bandwidth input: {}", input);
        }
    }

    #[test]
    fn invalid_bandwidth_falls_back_to_one_mbps() {
        let config = prepare_upload_configuration(&upload_args(&["-b", "abc"]), &TEST_ID).unwrap();
        assert_eq!(config["bandwidth"], 125000);
    }

    #[test]
    fn undersized_send_buffer_is_doubled() {
        //requested buffer is too small for the 32 kibibyte TCP default, so it becomes length * 2
        let config =
            prepare_upload_configuration(&upload_args(&["--send_buffer", "100"]), &TEST_ID)
                .unwrap();
        assert_eq!(config["send_buffer"], 64 * 1024);
    }

    #[test]
    fn no_delay_flag_is_honoured() {
        let config = prepare_upload_configuration(&upload_args(&["-N"]), &TEST_ID).unwrap();
        assert_eq!(config["no_delay"], true);
    }

    #[test]
    fn out_of_range_send_interval_is_clamped() {
        let config =
            prepare_upload_configuration(&upload_args(&["--send_interval", "2.0"]), &TEST_ID)
                .unwrap();
        //f32 serialisation isn't exact, so allow a small tolerance
        assert!((config["send_interval"].as_f64().unwrap() - 0.05).abs() < 1e-6);
    }

    fn download_args<'a>(extra: &'a [&'a str]) -> clap::ArgMatches<'a> {
        let mut argv: Vec<&str> = vec!["rperf-test"];
        argv.extend_from_slice(extra);
        clap::App::new("rperf-test")
            .arg(
                clap::Arg::with_name("parallel")
                    .takes_value(true)
                    .short("P")
                    .default_value("1"),
            )
            .arg(
                clap::Arg::with_name("length")
                    .takes_value(true)
                    .default_value("0"),
            )
            .arg(
                clap::Arg::with_name("receive_buffer")
                    .takes_value(true)
                    .long("receive_buffer")
                    .default_value("0"),
            )
            .arg(clap::Arg::with_name("udp").takes_value(false).long("udp"))
            .get_matches_from(argv)
    }

    #[test]
    fn tcp_download_configuration_defaults() {
        let config = prepare_download_configuration(&download_args(&[]), &TEST_ID).unwrap();

        assert_eq!(config["family"], "tcp");
        assert_eq!(config["role"], "download");
        assert_eq!(config["length"], 32 * 1024);
    }

    #[test]
    fn udp_download_configuration_defaults() {
        let config = prepare_download_configuration(&download_args(&["--udp"]), &TEST_ID).unwrap();

        assert_eq!(config["family"], "udp");
        assert_eq!(config["length"], 1024);
    }

    #[test]
    fn undersized_receive_buffer_is_doubled() {
        let config =
            prepare_download_configuration(&download_args(&["--receive_buffer", "100"]), &TEST_ID)
                .unwrap();
        assert_eq!(config["receive_buffer"], 64 * 1024);
    }

    #[test]
    fn zero_streams_default_to_one() {
        let config = prepare_upload_configuration(&upload_args(&["-P", "0"]), &TEST_ID).unwrap();
        assert_eq!(config["streams"], 1);
    }
}
