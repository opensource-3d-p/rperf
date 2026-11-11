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

use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use std::error::Error;
type BoxResult<T> = Result<T, Box<dyn Error>>;

/* This module contains structures used to represent and collect the results of tests.
 * Since everything is basically just a data-container with representation methods,
 * it isn't extensively documented.
 */

#[derive(Debug, PartialEq)]
pub enum IntervalResultKind {
    ClientDone,
    ClientFailed,
    ServerDone,
    ServerFailed,
    TcpReceive,
    TcpSend,
    UdpReceive,
    UdpSend,
}

pub fn get_unix_timestamp() -> f64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(n) => n.as_secs_f64(),
        Err(_) => panic!("SystemTime before UNIX epoch"),
    }
}

pub trait IntervalResult {
    fn kind(&self) -> IntervalResultKind;

    fn get_stream_idx(&self) -> u8;

    fn to_json(&self) -> serde_json::Value;

    //produces test-results in tabular form
    fn to_string(&self, bit: bool) -> String;
}

pub struct ClientDoneResult {
    pub stream_idx: u8,
}
impl IntervalResult for ClientDoneResult {
    fn kind(&self) -> IntervalResultKind {
        IntervalResultKind::ClientDone
    }

    fn get_stream_idx(&self) -> u8 {
        self.stream_idx
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "kind": "done",
            "origin": "client",
            "stream_idx": self.stream_idx,
        })
    }

    fn to_string(&self, _bit: bool) -> String {
        format!(
            "----------\n\
                 End of stream from client | stream: {}",
            self.stream_idx,
        )
    }
}
pub struct ServerDoneResult {
    pub stream_idx: u8,
}
impl IntervalResult for ServerDoneResult {
    fn kind(&self) -> IntervalResultKind {
        IntervalResultKind::ServerDone
    }

    fn get_stream_idx(&self) -> u8 {
        self.stream_idx
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "kind": "done",
            "origin": "server",
            "stream_idx": self.stream_idx,
        })
    }

    fn to_string(&self, _bit: bool) -> String {
        format!(
            "----------\n\
                 End of stream from server | stream: {}",
            self.stream_idx,
        )
    }
}

pub struct ClientFailedResult {
    pub stream_idx: u8,
}
impl IntervalResult for ClientFailedResult {
    fn kind(&self) -> IntervalResultKind {
        IntervalResultKind::ClientFailed
    }

    fn get_stream_idx(&self) -> u8 {
        self.stream_idx
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "kind": "failed",
            "origin": "client",
            "stream_idx": self.stream_idx,
        })
    }

    fn to_string(&self, _bit: bool) -> String {
        format!(
            "----------\n\
                 Failure in client stream | stream: {}",
            self.stream_idx,
        )
    }
}
pub struct ServerFailedResult {
    pub stream_idx: u8,
}
impl IntervalResult for ServerFailedResult {
    fn kind(&self) -> IntervalResultKind {
        IntervalResultKind::ServerFailed
    }

    fn get_stream_idx(&self) -> u8 {
        self.stream_idx
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "kind": "failed",
            "origin": "server",
            "stream_idx": self.stream_idx,
        })
    }

    fn to_string(&self, _bit: bool) -> String {
        format!(
            "----------\n\
                 Failure in server stream | stream: {}",
            self.stream_idx,
        )
    }
}

#[derive(Serialize, Deserialize)]
pub struct TcpReceiveResult {
    pub timestamp: f64,

    pub stream_idx: u8,

    pub duration: f32,

    pub bytes_received: u64,
}
impl TcpReceiveResult {
    fn from_json(value: serde_json::Value) -> BoxResult<TcpReceiveResult> {
        let receive_result: TcpReceiveResult = serde_json::from_value(value)?;
        Ok(receive_result)
    }

    fn to_result_json(&self) -> serde_json::Value {
        let mut serialised = serde_json::to_value(self).unwrap();
        serialised.as_object_mut().unwrap().remove("stream_idx");
        serialised
    }
}
impl IntervalResult for TcpReceiveResult {
    fn kind(&self) -> IntervalResultKind {
        IntervalResultKind::TcpReceive
    }

    fn get_stream_idx(&self) -> u8 {
        self.stream_idx
    }

    fn to_json(&self) -> serde_json::Value {
        let mut serialised = serde_json::to_value(self).unwrap();
        serialised["family"] = serde_json::json!("tcp");
        serialised["kind"] = serde_json::json!("receive");
        serialised
    }

    fn to_string(&self, bit: bool) -> String {
        let duration_divisor = if self.duration == 0.0 {
            //avoid zerodiv, which can happen if the stream fails
            1.0
        } else {
            self.duration
        };

        let bytes_per_second = self.bytes_received as f32 / duration_divisor;

        let throughput = match bit {
            true => format!(
                "megabits/second: {:.3}",
                bytes_per_second / (1_000_000.00 / 8.0)
            ),
            false => format!("megabytes/second: {:.3}", bytes_per_second / 1_000_000.00),
        };

        format!(
            "----------\n\
                 TCP receive result over {:.2}s | stream: {}\n\
                 bytes: {} | per second: {:.3} | {}",
            self.duration, self.stream_idx, self.bytes_received, bytes_per_second, throughput,
        )
    }
}

#[derive(Serialize, Deserialize)]
pub struct TcpSendResult {
    pub timestamp: f64,

    pub stream_idx: u8,

    pub duration: f32,

    pub bytes_sent: u64,
    pub sends_blocked: u64,
}
impl TcpSendResult {
    fn from_json(value: serde_json::Value) -> BoxResult<TcpSendResult> {
        let mut local_value = value.clone();
        if local_value.get("sends_blocked").is_none() {
            //pre-0.1.8 peer
            local_value["sends_blocked"] = serde_json::json!(0_u64); //report pre-0.1.8 status
        }

        let send_result: TcpSendResult = serde_json::from_value(local_value)?;
        Ok(send_result)
    }

    fn to_result_json(&self) -> serde_json::Value {
        let mut serialised = serde_json::to_value(self).unwrap();
        serialised.as_object_mut().unwrap().remove("stream_idx");
        serialised
    }
}
impl IntervalResult for TcpSendResult {
    fn kind(&self) -> IntervalResultKind {
        IntervalResultKind::TcpSend
    }

    fn get_stream_idx(&self) -> u8 {
        self.stream_idx
    }

    fn to_json(&self) -> serde_json::Value {
        let mut serialised = serde_json::to_value(self).unwrap();
        serialised["family"] = serde_json::json!("tcp");
        serialised["kind"] = serde_json::json!("send");
        serialised
    }

    fn to_string(&self, bit: bool) -> String {
        let duration_divisor = if self.duration == 0.0 {
            //avoid zerodiv, which can happen if the stream fails
            1.0
        } else {
            self.duration
        };

        let bytes_per_second = self.bytes_sent as f32 / duration_divisor;

        let throughput = match bit {
            true => format!(
                "megabits/second: {:.3}",
                bytes_per_second / (1_000_000.00 / 8.0)
            ),
            false => format!("megabytes/second: {:.3}", bytes_per_second / 1_000_000.00),
        };

        let mut output = format!(
            "----------\n\
                 TCP send result over {:.2}s | stream: {}\n\
                 bytes: {} | per second: {:.3} | {}",
            self.duration, self.stream_idx, self.bytes_sent, bytes_per_second, throughput,
        );
        if self.sends_blocked > 0 {
            output.push_str(&format!(
                "\nstalls due to full send-buffer: {}",
                self.sends_blocked
            ));
        }
        output
    }
}

#[derive(Serialize, Deserialize)]
pub struct UdpReceiveResult {
    pub timestamp: f64,

    pub stream_idx: u8,

    pub duration: f32,

    pub bytes_received: u64,
    pub packets_received: u64,
    pub packets_lost: i64,
    pub packets_out_of_order: u64,
    pub packets_duplicated: u64,

    pub unbroken_sequence: u64,
    pub jitter_seconds: Option<f32>,
}
impl UdpReceiveResult {
    fn from_json(value: serde_json::Value) -> BoxResult<UdpReceiveResult> {
        let receive_result: UdpReceiveResult = serde_json::from_value(value)?;
        Ok(receive_result)
    }

    fn to_result_json(&self) -> serde_json::Value {
        let mut serialised = serde_json::to_value(self).unwrap();
        serialised.as_object_mut().unwrap().remove("stream_idx");
        serialised
    }
}
impl IntervalResult for UdpReceiveResult {
    fn kind(&self) -> IntervalResultKind {
        IntervalResultKind::UdpReceive
    }

    fn get_stream_idx(&self) -> u8 {
        self.stream_idx
    }

    fn to_json(&self) -> serde_json::Value {
        let mut serialised = serde_json::to_value(self).unwrap();
        serialised["family"] = serde_json::json!("udp");
        serialised["kind"] = serde_json::json!("receive");
        serialised
    }

    fn to_string(&self, bit: bool) -> String {
        let duration_divisor = if self.duration == 0.0 {
            //avoid zerodiv, which can happen if the stream fails
            1.0
        } else {
            self.duration
        };

        let bytes_per_second = self.bytes_received as f32 / duration_divisor;

        let throughput = match bit {
            true => format!(
                "megabits/second: {:.3}",
                bytes_per_second / (1_000_000.00 / 8.0)
            ),
            false => format!("megabytes/second: {:.3}", bytes_per_second / 1_000_000.00),
        };

        let mut output = format!("----------\n\
                                  UDP receive result over {:.2}s | stream: {}\n\
                                  bytes: {} | per second: {:.3} | {}\n\
                                  packets: {} | lost: {} | out-of-order: {} | duplicate: {} | per second: {:.3}",
                                self.duration, self.stream_idx,
                                self.bytes_received, bytes_per_second, throughput,
                                self.packets_received, self.packets_lost, self.packets_out_of_order, self.packets_duplicated, self.packets_received as f32 / duration_divisor,
        );
        if let Some(jitter_seconds) = self.jitter_seconds {
            output.push_str(&format!(
                "\njitter: {:.6}s over {} consecutive packets",
                jitter_seconds, self.unbroken_sequence
            ));
        }
        output
    }
}

#[derive(Serialize, Deserialize)]
pub struct UdpSendResult {
    pub timestamp: f64,

    pub stream_idx: u8,

    pub duration: f32,

    pub bytes_sent: u64,
    pub packets_sent: u64,
    pub sends_blocked: u64,
}
impl UdpSendResult {
    fn from_json(value: serde_json::Value) -> BoxResult<UdpSendResult> {
        let mut local_value = value.clone();
        if local_value.get("sends_blocked").is_none() {
            //pre-0.1.8 peer
            local_value["sends_blocked"] = serde_json::json!(0_u64); //report pre-0.1.8 status
        }

        let send_result: UdpSendResult = serde_json::from_value(local_value)?;
        Ok(send_result)
    }

    fn to_result_json(&self) -> serde_json::Value {
        let mut serialised = serde_json::to_value(self).unwrap();
        serialised.as_object_mut().unwrap().remove("stream_idx");
        serialised
    }
}
impl IntervalResult for UdpSendResult {
    fn kind(&self) -> IntervalResultKind {
        IntervalResultKind::UdpSend
    }

    fn get_stream_idx(&self) -> u8 {
        self.stream_idx
    }

    fn to_json(&self) -> serde_json::Value {
        let mut serialised = serde_json::to_value(self).unwrap();
        serialised["family"] = serde_json::json!("udp");
        serialised["kind"] = serde_json::json!("send");
        serialised
    }

    fn to_string(&self, bit: bool) -> String {
        let duration_divisor = if self.duration == 0.0 {
            //avoid zerodiv, which can happen if the stream fails
            1.0
        } else {
            self.duration
        };

        let bytes_per_second = self.bytes_sent as f32 / duration_divisor;

        let throughput = match bit {
            true => format!(
                "megabits/second: {:.3}",
                bytes_per_second / (1_000_000.00 / 8.0)
            ),
            false => format!("megabytes/second: {:.3}", bytes_per_second / 1_000_000.00),
        };

        let mut output = format!(
            "----------\n\
                 UDP send result over {:.2}s | stream: {}\n\
                 bytes: {} | per second: {:.3} | {}\n\
                 packets: {} per second: {:.3}",
            self.duration,
            self.stream_idx,
            self.bytes_sent,
            bytes_per_second,
            throughput,
            self.packets_sent,
            self.packets_sent as f32 / duration_divisor,
        );
        if self.sends_blocked > 0 {
            output.push_str(&format!(
                "\nstalls due to full send-buffer: {}",
                self.sends_blocked
            ));
        }
        output
    }
}

pub fn interval_result_from_json(value: serde_json::Value) -> BoxResult<Box<dyn IntervalResult>> {
    match value.get("family") {
        Some(f) => match f.as_str() {
            Some(family) => match family {
                "tcp" => match value.get("kind") {
                    Some(k) => match k.as_str() {
                        Some(kind) => match kind {
                            "receive" => Ok(Box::new(TcpReceiveResult::from_json(value)?)),
                            "send" => Ok(Box::new(TcpSendResult::from_json(value)?)),
                            _ => Err(Box::new(simple_error::simple_error!(
                                "unsupported interval-result kind: {}",
                                kind
                            ))),
                        },
                        None => Err(Box::new(simple_error::simple_error!(
                            "interval-result's kind is not a string"
                        ))),
                    },
                    None => Err(Box::new(simple_error::simple_error!(
                        "interval-result has no kind"
                    ))),
                },
                "udp" => match value.get("kind") {
                    Some(k) => match k.as_str() {
                        Some(kind) => match kind {
                            "receive" => Ok(Box::new(UdpReceiveResult::from_json(value)?)),
                            "send" => Ok(Box::new(UdpSendResult::from_json(value)?)),
                            _ => Err(Box::new(simple_error::simple_error!(
                                "unsupported interval-result kind: {}",
                                kind
                            ))),
                        },
                        None => Err(Box::new(simple_error::simple_error!(
                            "interval-result's kind is not a string"
                        ))),
                    },
                    None => Err(Box::new(simple_error::simple_error!(
                        "interval-result has no kind"
                    ))),
                },
                _ => Err(Box::new(simple_error::simple_error!(
                    "unsupported interval-result family: {}",
                    family
                ))),
            },
            None => Err(Box::new(simple_error::simple_error!(
                "interval-result's family is not a string"
            ))),
        },
        None => Err(Box::new(simple_error::simple_error!(
            "interval-result has no family"
        ))),
    }
}

pub trait StreamResults {
    fn update_from_json(&mut self, value: serde_json::Value) -> BoxResult<()>;

    fn to_json(&self, omit_seconds: usize) -> serde_json::Value;
}

struct TcpStreamResults {
    receive_results: Vec<TcpReceiveResult>,
    send_results: Vec<TcpSendResult>,
}
impl StreamResults for TcpStreamResults {
    fn update_from_json(&mut self, value: serde_json::Value) -> BoxResult<()> {
        match value.get("kind") {
            Some(k) => match k.as_str() {
                Some(kind) => match kind {
                    "send" => {
                        self.send_results.push(TcpSendResult::from_json(value)?);
                        Ok(())
                    }
                    "receive" => {
                        self.receive_results
                            .push(TcpReceiveResult::from_json(value)?);
                        Ok(())
                    }
                    _ => Err(Box::new(simple_error::simple_error!(
                        "unsupported kind for TCP stream-result: {}",
                        kind
                    ))),
                },
                None => Err(Box::new(simple_error::simple_error!(
                    "kind must be a string for TCP stream-result"
                ))),
            },
            None => Err(Box::new(simple_error::simple_error!(
                "no kind specified for TCP stream-result"
            ))),
        }
    }

    fn to_json(&self, omit_seconds: usize) -> serde_json::Value {
        let mut duration_send: f64 = 0.0;
        let mut bytes_sent: u64 = 0;

        let mut duration_receive: f64 = 0.0;
        let mut bytes_received: u64 = 0;

        for (i, sr) in self.send_results.iter().enumerate() {
            if i < omit_seconds {
                continue;
            }

            duration_send += sr.duration as f64;
            bytes_sent += sr.bytes_sent;
        }

        for (i, rr) in self.receive_results.iter().enumerate() {
            if i < omit_seconds {
                continue;
            }

            duration_receive += rr.duration as f64;
            bytes_received += rr.bytes_received;
        }

        let summary = serde_json::json!({
            "duration_send": duration_send,
            "bytes_sent": bytes_sent,

            "duration_receive": duration_receive,
            "bytes_received": bytes_received,
        });

        serde_json::json!({
            "receive": self.receive_results.iter().map(|rr| rr.to_result_json()).collect::<Vec<serde_json::Value>>(),
            "send": self.send_results.iter().map(|sr| sr.to_result_json()).collect::<Vec<serde_json::Value>>(),
            "summary": summary,
        })
    }
}

struct UdpStreamResults {
    receive_results: Vec<UdpReceiveResult>,
    send_results: Vec<UdpSendResult>,
}
impl StreamResults for UdpStreamResults {
    fn update_from_json(&mut self, value: serde_json::Value) -> BoxResult<()> {
        match value.get("kind") {
            Some(k) => match k.as_str() {
                Some(kind) => match kind {
                    "send" => {
                        self.send_results.push(UdpSendResult::from_json(value)?);
                        Ok(())
                    }
                    "receive" => {
                        self.receive_results
                            .push(UdpReceiveResult::from_json(value)?);
                        Ok(())
                    }
                    _ => Err(Box::new(simple_error::simple_error!(
                        "unsupported kind for UDP stream-result: {}",
                        kind
                    ))),
                },
                None => Err(Box::new(simple_error::simple_error!(
                    "kind must be a string for UDP stream-result"
                ))),
            },
            None => Err(Box::new(simple_error::simple_error!(
                "no kind specified for UDP stream-result"
            ))),
        }
    }

    fn to_json(&self, omit_seconds: usize) -> serde_json::Value {
        let mut duration_send: f64 = 0.0;

        let mut bytes_sent: u64 = 0;
        let mut packets_sent: u64 = 0;

        let mut duration_receive: f64 = 0.0;

        let mut bytes_received: u64 = 0;
        let mut packets_received: u64 = 0;
        let mut packets_out_of_order: u64 = 0;
        let mut packets_duplicated: u64 = 0;

        let mut jitter_calculated = false;
        let mut unbroken_sequence_count: u64 = 0;
        let mut jitter_weight: f64 = 0.0;

        for (i, sr) in self.send_results.iter().enumerate() {
            if i < omit_seconds {
                continue;
            }

            duration_send += sr.duration as f64;

            bytes_sent += sr.bytes_sent;
            packets_sent += sr.packets_sent;
        }

        for (i, rr) in self.receive_results.iter().enumerate() {
            if i < omit_seconds {
                continue;
            }

            duration_receive += rr.duration as f64;

            bytes_received += rr.bytes_received;
            packets_received += rr.packets_received;
            packets_out_of_order += rr.packets_out_of_order;
            packets_duplicated += rr.packets_duplicated;

            if let Some(jitter_seconds) = rr.jitter_seconds {
                jitter_weight += (rr.unbroken_sequence as f64) * (jitter_seconds as f64);
                unbroken_sequence_count += rr.unbroken_sequence;

                jitter_calculated = true;
            }
        }

        let mut summary = serde_json::json!({
            "duration_send": duration_send,

            "bytes_sent": bytes_sent,
            "packets_sent": packets_sent,


            "duration_receive": duration_receive,

            "bytes_received": bytes_received,
            "packets_received": packets_received,
            //packets_received includes duplicates
            "packets_lost": packets_sent.saturating_sub(packets_received.saturating_sub(packets_duplicated)),
            "packets_out_of_order": packets_out_of_order,
            "packets_duplicated": packets_duplicated,
        });
        if let Some(framed_packet_size) = bytes_sent.checked_div(packets_sent) {
            summary["framed_packet_size"] = serde_json::json!(framed_packet_size);
        }
        if jitter_calculated {
            summary["jitter_average"] =
                serde_json::json!(jitter_weight / (unbroken_sequence_count as f64));
            summary["jitter_packets_consecutive"] = serde_json::json!(unbroken_sequence_count);
        }

        serde_json::json!({
            "receive": self.receive_results.iter().map(|rr| rr.to_result_json()).collect::<Vec<serde_json::Value>>(),
            "send": self.send_results.iter().map(|sr| sr.to_result_json()).collect::<Vec<serde_json::Value>>(),
            "summary": summary,
        })
    }
}

pub trait TestResults {
    fn count_in_progress_streams(&self) -> u8;
    fn mark_stream_done(&mut self, idx: &u8, success: bool);

    fn count_in_progress_streams_server(&self) -> u8;
    fn mark_stream_done_server(&mut self, idx: &u8);

    fn is_success(&self) -> bool;

    fn update_from_json(&mut self, value: serde_json::Value) -> BoxResult<()>;

    fn to_json(
        &self,
        omit_seconds: usize,
        upload_config: serde_json::Value,
        download_config: serde_json::Value,
        common_config: serde_json::Value,
        additional_config: serde_json::Value,
    ) -> serde_json::Value;

    //produces a pretty-printed JSON string with the test results
    fn to_json_string(
        &self,
        omit_seconds: usize,
        upload_config: serde_json::Value,
        download_config: serde_json::Value,
        common_config: serde_json::Value,
        additional_config: serde_json::Value,
    ) -> String {
        serde_json::to_string_pretty(&self.to_json(
            omit_seconds,
            upload_config,
            download_config,
            common_config,
            additional_config,
        ))
        .unwrap()
    }

    //produces test-results in tabular form
    fn to_string(&self, bit: bool, omit_seconds: usize) -> String;
}

pub struct TcpTestResults {
    stream_results: HashMap<u8, TcpStreamResults>,
    pending_tests: HashSet<u8>,
    failed_tests: HashSet<u8>,
    server_tests_finished: HashSet<u8>,
}
impl TcpTestResults {
    pub fn new() -> TcpTestResults {
        TcpTestResults {
            stream_results: HashMap::new(),
            pending_tests: HashSet::new(),
            failed_tests: HashSet::new(),
            server_tests_finished: HashSet::new(),
        }
    }
    pub fn prepare_index(&mut self, idx: &u8) {
        self.stream_results.insert(
            *idx,
            TcpStreamResults {
                receive_results: Vec::new(),
                send_results: Vec::new(),
            },
        );
        self.pending_tests.insert(*idx);
    }
}
impl TestResults for TcpTestResults {
    fn count_in_progress_streams(&self) -> u8 {
        self.pending_tests.len() as u8
    }
    fn mark_stream_done(&mut self, idx: &u8, success: bool) {
        self.pending_tests.remove(idx);
        if !success {
            self.failed_tests.insert(*idx);
        }
    }

    fn count_in_progress_streams_server(&self) -> u8 {
        let mut count: u8 = 0;
        for idx in self.stream_results.keys() {
            if !self.server_tests_finished.contains(idx) {
                count += 1;
            }
        }
        count
    }
    fn mark_stream_done_server(&mut self, idx: &u8) {
        self.server_tests_finished.insert(*idx);
    }

    fn is_success(&self) -> bool {
        self.pending_tests.is_empty() && self.failed_tests.is_empty()
    }

    fn update_from_json(&mut self, value: serde_json::Value) -> BoxResult<()> {
        match value.get("family") {
            Some(f) => match f.as_str() {
                Some(family) => match family {
                    "tcp" => match value.get("stream_idx") {
                        Some(idx) => match idx.as_i64() {
                            Some(idx64) => match self.stream_results.get_mut(&(idx64 as u8)) {
                                Some(stream_results) => stream_results.update_from_json(value),
                                None => Err(Box::new(simple_error::simple_error!(
                                    "stream-index {} is not a valid identifier",
                                    idx64
                                ))),
                            },
                            None => Err(Box::new(simple_error::simple_error!(
                                "stream-index is not an integer"
                            ))),
                        },
                        None => Err(Box::new(simple_error::simple_error!(
                            "no stream-index specified"
                        ))),
                    },
                    _ => Err(Box::new(simple_error::simple_error!(
                        "unsupported family for TCP stream-result: {}",
                        family
                    ))),
                },
                None => Err(Box::new(simple_error::simple_error!(
                    "kind must be a string for TCP stream-result"
                ))),
            },
            None => Err(Box::new(simple_error::simple_error!(
                "no kind specified for TCP stream-result"
            ))),
        }
    }

    fn to_json(
        &self,
        omit_seconds: usize,
        upload_config: serde_json::Value,
        download_config: serde_json::Value,
        common_config: serde_json::Value,
        additional_config: serde_json::Value,
    ) -> serde_json::Value {
        let mut duration_send: f64 = 0.0;
        let mut bytes_sent: u64 = 0;

        let mut duration_receive: f64 = 0.0;
        let mut bytes_received: u64 = 0;

        let mut streams = Vec::with_capacity(self.stream_results.len());
        for (idx, stream) in self.stream_results.iter() {
            streams.push(serde_json::json!({
                "intervals": stream.to_json(omit_seconds),
                "abandoned": self.pending_tests.contains(idx),
                "failed": self.failed_tests.contains(idx),
            }));

            for (i, sr) in stream.send_results.iter().enumerate() {
                if i < omit_seconds {
                    continue;
                }

                duration_send += sr.duration as f64;
                bytes_sent += sr.bytes_sent;
            }

            for (i, rr) in stream.receive_results.iter().enumerate() {
                if i < omit_seconds {
                    continue;
                }

                duration_receive += rr.duration as f64;
                bytes_received += rr.bytes_received;
            }
        }

        let summary = serde_json::json!({
            "duration_send": duration_send,
            "bytes_sent": bytes_sent,

            "duration_receive": duration_receive,
            "bytes_received": bytes_received,
        });

        serde_json::json!({
            "config": {
                "upload": upload_config,
                "download": download_config,
                "common": common_config,
                "additional": additional_config,
            },
            "streams": streams,
            "summary": summary,
            "success": self.is_success(),
        })
    }

    fn to_string(&self, bit: bool, omit_seconds: usize) -> String {
        let stream_count = self.stream_results.len();
        let mut stream_send_durations = vec![0.0; stream_count];
        let mut stream_receive_durations = vec![0.0; stream_count];

        let mut duration_send: f64 = 0.0;
        let mut bytes_sent: u64 = 0;

        let mut duration_receive: f64 = 0.0;
        let mut bytes_received: u64 = 0;

        let mut sends_blocked = false;

        for (stream_idx, stream) in self.stream_results.values().enumerate() {
            for (i, sr) in stream.send_results.iter().enumerate() {
                if i < omit_seconds {
                    continue;
                }

                duration_send += sr.duration as f64;
                stream_send_durations[stream_idx] += sr.duration as f64;

                bytes_sent += sr.bytes_sent;

                sends_blocked |= sr.sends_blocked > 0;
            }

            for (i, rr) in stream.receive_results.iter().enumerate() {
                if i < omit_seconds {
                    continue;
                }

                duration_receive += rr.duration as f64;
                stream_receive_durations[stream_idx] += rr.duration as f64;

                bytes_received += rr.bytes_received;
            }
        }
        stream_send_durations.sort_by(|a, b| a.partial_cmp(b).unwrap());
        stream_receive_durations.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let send_duration_divisor = if duration_send == 0.0 {
            //avoid zerodiv, which can happen if all streams fail
            1.0
        } else {
            duration_send
        };
        let send_bytes_per_second = bytes_sent as f64 / send_duration_divisor;
        let send_throughput = match bit {
            true => format!(
                "megabits/second: {:.3}",
                send_bytes_per_second / (1_000_000.00 / 8.0)
            ),
            false => format!(
                "megabytes/second: {:.3}",
                send_bytes_per_second / 1_000_000.00
            ),
        };
        let total_send_throughput = match bit {
            true => format!(
                "megabits/second: {:.3}",
                (send_bytes_per_second / (1_000_000.00 / 8.0)) * stream_count as f64
            ),
            false => format!(
                "megabytes/second: {:.3}",
                (send_bytes_per_second / 1_000_000.00) * stream_count as f64
            ),
        };

        let receive_duration_divisor = if duration_receive == 0.0 {
            //avoid zerodiv, which can happen if all streams fail
            1.0
        } else {
            duration_receive
        };
        let receive_bytes_per_second = bytes_received as f64 / receive_duration_divisor;
        let receive_throughput = match bit {
            true => format!(
                "megabits/second: {:.3}",
                receive_bytes_per_second / (1_000_000.00 / 8.0)
            ),
            false => format!(
                "megabytes/second: {:.3}",
                receive_bytes_per_second / 1_000_000.00
            ),
        };
        let total_receive_throughput = match bit {
            true => format!(
                "megabits/second: {:.3}",
                (receive_bytes_per_second / (1_000_000.00 / 8.0)) * stream_count as f64
            ),
            false => format!(
                "megabytes/second: {:.3}",
                (receive_bytes_per_second / 1_000_000.00) * stream_count as f64
            ),
        };

        let mut output = format!(
            "==========\n\
                                  TCP send result over {:.2}s | streams: {}\n\
                                  stream-average bytes per second: {:.3} | {}\n\
                                  total bytes: {} | per second: {:.3} | {}\n\
                                  ==========\n\
                                  TCP receive result over {:.2}s | streams: {}\n\
                                  stream-average bytes per second: {:.3} | {}\n\
                                  total bytes: {} | per second: {:.3} | {}",
            stream_send_durations.last().copied().unwrap_or(0.0),
            stream_count,
            send_bytes_per_second,
            send_throughput,
            bytes_sent,
            send_bytes_per_second * stream_count as f64,
            total_send_throughput,
            stream_receive_durations.last().copied().unwrap_or(0.0),
            stream_count,
            receive_bytes_per_second,
            receive_throughput,
            bytes_received,
            receive_bytes_per_second * stream_count as f64,
            total_receive_throughput,
        );
        if sends_blocked {
            output.push_str("\nthroughput throttled by buffer limitations");
        }
        if !self.is_success() {
            output.push_str("\nTESTING DID NOT COMPLETE SUCCESSFULLY");
        }

        output
    }
}

pub struct UdpTestResults {
    stream_results: HashMap<u8, UdpStreamResults>,
    pending_tests: HashSet<u8>,
    failed_tests: HashSet<u8>,
    server_tests_finished: HashSet<u8>,
}
impl UdpTestResults {
    pub fn new() -> UdpTestResults {
        UdpTestResults {
            stream_results: HashMap::new(),
            pending_tests: HashSet::new(),
            failed_tests: HashSet::new(),
            server_tests_finished: HashSet::new(),
        }
    }
    pub fn prepare_index(&mut self, idx: &u8) {
        self.stream_results.insert(
            *idx,
            UdpStreamResults {
                receive_results: Vec::new(),
                send_results: Vec::new(),
            },
        );
        self.pending_tests.insert(*idx);
    }
}
impl TestResults for UdpTestResults {
    fn count_in_progress_streams(&self) -> u8 {
        self.pending_tests.len() as u8
    }
    fn mark_stream_done(&mut self, idx: &u8, success: bool) {
        self.pending_tests.remove(idx);
        if !success {
            self.failed_tests.insert(*idx);
        }
    }

    fn count_in_progress_streams_server(&self) -> u8 {
        let mut count: u8 = 0;
        for idx in self.stream_results.keys() {
            if !self.server_tests_finished.contains(idx) {
                count += 1;
            }
        }
        count
    }
    fn mark_stream_done_server(&mut self, idx: &u8) {
        self.server_tests_finished.insert(*idx);
    }

    fn is_success(&self) -> bool {
        self.pending_tests.is_empty() && self.failed_tests.is_empty()
    }

    fn update_from_json(&mut self, value: serde_json::Value) -> BoxResult<()> {
        match value.get("family") {
            Some(f) => match f.as_str() {
                Some(family) => match family {
                    "udp" => match value.get("stream_idx") {
                        Some(idx) => match idx.as_i64() {
                            Some(idx64) => match self.stream_results.get_mut(&(idx64 as u8)) {
                                Some(stream_results) => stream_results.update_from_json(value),
                                None => Err(Box::new(simple_error::simple_error!(
                                    "stream-index {} is not a valid identifier",
                                    idx64
                                ))),
                            },
                            None => Err(Box::new(simple_error::simple_error!(
                                "stream-index is not an integer"
                            ))),
                        },
                        None => Err(Box::new(simple_error::simple_error!(
                            "no stream-index specified"
                        ))),
                    },
                    _ => Err(Box::new(simple_error::simple_error!(
                        "unsupported family for UDP stream-result: {}",
                        family
                    ))),
                },
                None => Err(Box::new(simple_error::simple_error!(
                    "kind must be a string for UDP stream-result"
                ))),
            },
            None => Err(Box::new(simple_error::simple_error!(
                "no kind specified for UDP stream-result"
            ))),
        }
    }

    fn to_json(
        &self,
        omit_seconds: usize,
        upload_config: serde_json::Value,
        download_config: serde_json::Value,
        common_config: serde_json::Value,
        additional_config: serde_json::Value,
    ) -> serde_json::Value {
        let mut duration_send: f64 = 0.0;

        let mut bytes_sent: u64 = 0;
        let mut packets_sent: u64 = 0;

        let mut duration_receive: f64 = 0.0;

        let mut bytes_received: u64 = 0;
        let mut packets_received: u64 = 0;
        let mut packets_out_of_order: u64 = 0;
        let mut packets_duplicated: u64 = 0;

        let mut jitter_calculated = false;
        let mut unbroken_sequence_count: u64 = 0;
        let mut jitter_weight: f64 = 0.0;

        let mut streams = Vec::with_capacity(self.stream_results.len());
        for (idx, stream) in self.stream_results.iter() {
            streams.push(serde_json::json!({
                "intervals": stream.to_json(omit_seconds),
                "abandoned": self.pending_tests.contains(idx),
                "failed": self.failed_tests.contains(idx),
            }));

            for (i, sr) in stream.send_results.iter().enumerate() {
                if i < omit_seconds {
                    continue;
                }

                duration_send += sr.duration as f64;

                bytes_sent += sr.bytes_sent;
                packets_sent += sr.packets_sent;
            }

            for (i, rr) in stream.receive_results.iter().enumerate() {
                if i < omit_seconds {
                    continue;
                }

                duration_receive += rr.duration as f64;

                bytes_received += rr.bytes_received;
                packets_received += rr.packets_received;
                packets_out_of_order += rr.packets_out_of_order;
                packets_duplicated += rr.packets_duplicated;

                if let Some(jitter_seconds) = rr.jitter_seconds {
                    jitter_weight += (rr.unbroken_sequence as f64) * (jitter_seconds as f64);
                    unbroken_sequence_count += rr.unbroken_sequence;

                    jitter_calculated = true;
                }
            }
        }

        let mut summary = serde_json::json!({
            "duration_send": duration_send,

            "bytes_sent": bytes_sent,
            "packets_sent": packets_sent,


            "duration_receive": duration_receive,

            "bytes_received": bytes_received,
            "packets_received": packets_received,
            //packets_received includes duplicates
            "packets_lost": packets_sent.saturating_sub(packets_received.saturating_sub(packets_duplicated)),
            "packets_out_of_order": packets_out_of_order,
            "packets_duplicated": packets_duplicated,
        });
        if let Some(framed_packet_size) = bytes_sent.checked_div(packets_sent) {
            summary["framed_packet_size"] = serde_json::json!(framed_packet_size);
        }
        if jitter_calculated {
            summary["jitter_average"] =
                serde_json::json!(jitter_weight / (unbroken_sequence_count as f64));
            summary["jitter_packets_consecutive"] = serde_json::json!(unbroken_sequence_count);
        }

        serde_json::json!({
            "config": {
                "upload": upload_config,
                "download": download_config,
                "common": common_config,
                "additional": additional_config,
            },
            "streams": streams,
            "summary": summary,
            "success": self.is_success(),
        })
    }

    fn to_string(&self, bit: bool, omit_seconds: usize) -> String {
        let stream_count = self.stream_results.len();
        let mut stream_send_durations = vec![0.0; stream_count];
        let mut stream_receive_durations = vec![0.0; stream_count];

        let mut duration_send: f64 = 0.0;

        let mut bytes_sent: u64 = 0;
        let mut packets_sent: u64 = 0;

        let mut duration_receive: f64 = 0.0;

        let mut bytes_received: u64 = 0;
        let mut packets_received: u64 = 0;
        let mut packets_out_of_order: u64 = 0;
        let mut packets_duplicated: u64 = 0;

        let mut jitter_calculated = false;
        let mut unbroken_sequence_count: u64 = 0;
        let mut jitter_weight: f64 = 0.0;

        let mut sends_blocked = false;

        for (stream_idx, stream) in self.stream_results.values().enumerate() {
            for (i, sr) in stream.send_results.iter().enumerate() {
                if i < omit_seconds {
                    continue;
                }

                duration_send += sr.duration as f64;
                stream_send_durations[stream_idx] += sr.duration as f64;

                bytes_sent += sr.bytes_sent;
                packets_sent += sr.packets_sent;

                sends_blocked |= sr.sends_blocked > 0;
            }

            for (i, rr) in stream.receive_results.iter().enumerate() {
                if i < omit_seconds {
                    continue;
                }

                duration_receive += rr.duration as f64;
                stream_receive_durations[stream_idx] += rr.duration as f64;

                bytes_received += rr.bytes_received;
                packets_received += rr.packets_received;
                packets_out_of_order += rr.packets_out_of_order;
                packets_duplicated += rr.packets_duplicated;

                if let Some(jitter_seconds) = rr.jitter_seconds {
                    jitter_weight += (rr.unbroken_sequence as f64) * (jitter_seconds as f64);
                    unbroken_sequence_count += rr.unbroken_sequence;

                    jitter_calculated = true;
                }
            }
        }
        stream_send_durations.sort_by(|a, b| a.partial_cmp(b).unwrap());
        stream_receive_durations.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let send_duration_divisor = if duration_send == 0.0 {
            //avoid zerodiv, which can happen if all streams fail
            1.0
        } else {
            duration_send
        };
        let send_bytes_per_second = bytes_sent as f64 / send_duration_divisor;
        let send_throughput = match bit {
            true => format!(
                "megabits/second: {:.3}",
                send_bytes_per_second / (1_000_000.00 / 8.0)
            ),
            false => format!(
                "megabytes/second: {:.3}",
                send_bytes_per_second / 1_000_000.00
            ),
        };
        let total_send_throughput = match bit {
            true => format!(
                "megabits/second: {:.3}",
                (send_bytes_per_second / (1_000_000.00 / 8.0)) * stream_count as f64
            ),
            false => format!(
                "megabytes/second: {:.3}",
                (send_bytes_per_second / 1_000_000.00) * stream_count as f64
            ),
        };

        let receive_duration_divisor = if duration_receive == 0.0 {
            //avoid zerodiv, which can happen if all streams fail
            1.0
        } else {
            duration_receive
        };
        let receive_bytes_per_second = bytes_received as f64 / receive_duration_divisor;
        let receive_throughput = match bit {
            true => format!(
                "megabits/second: {:.3}",
                receive_bytes_per_second / (1_000_000.00 / 8.0)
            ),
            false => format!(
                "megabytes/second: {:.3}",
                receive_bytes_per_second / 1_000_000.00
            ),
        };
        let total_receive_throughput = match bit {
            true => format!(
                "megabits/second: {:.3}",
                (receive_bytes_per_second / (1_000_000.00 / 8.0)) * stream_count as f64
            ),
            false => format!(
                "megabytes/second: {:.3}",
                (receive_bytes_per_second / 1_000_000.00) * stream_count as f64
            ),
        };

        let packets_lost =
            packets_sent.saturating_sub(packets_received.saturating_sub(packets_duplicated));

        let packets_sent_divisor = if packets_sent == 0 {
            //avoid zerodiv, which can happen if all streams fail
            1.0
        } else {
            packets_sent as f64
        };
        let mut output = format!("==========\n\
                                  UDP send result over {:.2}s | streams: {}\n\
                                  stream-average bytes per second: {:.3} | {}\n\
                                  total bytes: {} | per second: {:.3} | {}\n\
                                  packets: {} per second: {:.3}\n\
                                  ==========\n\
                                  UDP receive result over {:.2}s | streams: {}\n\
                                  stream-average bytes per second: {:.3} | {}\n\
                                  total bytes: {} | per second: {:.3} | {}\n\
                                  packets: {} | lost: {} ({:.1}%) | out-of-order: {} | duplicate: {} | per second: {:.3}",
                                stream_send_durations.last().copied().unwrap_or(0.0), stream_count,
                                send_bytes_per_second, send_throughput,
                                bytes_sent, send_bytes_per_second * stream_count as f64, total_send_throughput,
                                packets_sent, (packets_sent as f64 / send_duration_divisor) * stream_count as f64,

                                stream_receive_durations.last().copied().unwrap_or(0.0), stream_count,
                                receive_bytes_per_second, receive_throughput,
                                bytes_received, receive_bytes_per_second * stream_count as f64, total_receive_throughput,
                                packets_received, packets_lost, (packets_lost as f64 / packets_sent_divisor) * 100.0, packets_out_of_order, packets_duplicated, (packets_received as f64 / receive_duration_divisor) * stream_count as f64,
        );
        if jitter_calculated {
            output.push_str(&format!(
                "\njitter: {:.6}s over {} consecutive packets",
                jitter_weight / (unbroken_sequence_count as f64),
                unbroken_sequence_count
            ));
        }
        if sends_blocked {
            output.push_str("\nthroughput throttled by buffer limitations");
        }
        if !self.is_success() {
            output.push_str("\nTESTING DID NOT COMPLETE SUCCESSFULLY");
        }

        output
    }
}

//AI-generated tests
#[cfg(test)]
mod tests {
    use super::*;

    fn tcp_receive(timestamp: f64, stream_idx: u8, duration: f32, bytes: u64) -> TcpReceiveResult {
        TcpReceiveResult {
            timestamp,
            stream_idx,
            duration,
            bytes_received: bytes,
        }
    }

    fn tcp_send(timestamp: f64, stream_idx: u8, duration: f32, bytes: u64) -> TcpSendResult {
        TcpSendResult {
            timestamp,
            stream_idx,
            duration,
            bytes_sent: bytes,
            sends_blocked: 0,
        }
    }

    fn udp_send(
        timestamp: f64,
        stream_idx: u8,
        duration: f32,
        bytes: u64,
        packets: u64,
    ) -> UdpSendResult {
        UdpSendResult {
            timestamp,
            stream_idx,
            duration,
            bytes_sent: bytes,
            packets_sent: packets,
            sends_blocked: 0,
        }
    }

    fn udp_receive(
        timestamp: f64,
        stream_idx: u8,
        duration: f32,
        bytes: u64,
        packets: u64,
        jitter: Option<f32>,
    ) -> UdpReceiveResult {
        UdpReceiveResult {
            timestamp,
            stream_idx,
            duration,
            bytes_received: bytes,
            packets_received: packets,
            packets_lost: 0,
            packets_out_of_order: 0,
            packets_duplicated: 0,
            unbroken_sequence: packets,
            jitter_seconds: jitter,
        }
    }

    #[test]
    fn result_json_roundtrips() {
        let receive = tcp_receive(1.0, 3, 1.0, 1000);
        let roundtripped = interval_result_from_json(receive.to_json()).unwrap();
        assert_eq!(roundtripped.kind(), IntervalResultKind::TcpReceive);
        assert_eq!(roundtripped.get_stream_idx(), 3);
        assert_eq!(roundtripped.to_json()["bytes_received"], 1000);

        let send = tcp_send(2.0, 0, 1.0, 2000);
        let roundtripped = interval_result_from_json(send.to_json()).unwrap();
        assert_eq!(roundtripped.kind(), IntervalResultKind::TcpSend);
        assert_eq!(roundtripped.get_stream_idx(), 0);

        let udp_rx = udp_receive(3.0, 1, 1.0, 4096, 4, Some(0.001));
        let roundtripped = interval_result_from_json(udp_rx.to_json()).unwrap();
        assert_eq!(roundtripped.kind(), IntervalResultKind::UdpReceive);
        assert_eq!(roundtripped.to_json()["packets_received"], 4);

        let udp_tx = udp_send(4.0, 2, 1.0, 8192, 8);
        let roundtripped = interval_result_from_json(udp_tx.to_json()).unwrap();
        assert_eq!(roundtripped.kind(), IntervalResultKind::UdpSend);
    }

    #[test]
    fn udp_summary_packets_lost_ignores_duplicates() {
        //packets_received includes duplicates; if duplicates exceed the send count, a naive
        //subtraction would underflow, so the summary must saturate at zero
        let mut results = UdpTestResults::new();
        results.prepare_index(&0);
        results
            .stream_results
            .get_mut(&0)
            .unwrap()
            .receive_results
            .push(UdpReceiveResult {
                timestamp: 1.0,
                stream_idx: 0,
                duration: 1.0,
                bytes_received: 204800,
                packets_received: 200,
                packets_lost: 0,
                packets_out_of_order: 0,
                packets_duplicated: 150,
                unbroken_sequence: 50,
                jitter_seconds: None,
            });
        results
            .stream_results
            .get_mut(&0)
            .unwrap()
            .send_results
            .push(udp_send(1.0, 0, 1.0, 102400, 100));
        results.mark_stream_done(&0, true);

        let json = results.to_json(
            0,
            serde_json::json!({}),
            serde_json::json!({}),
            serde_json::json!({}),
            serde_json::json!({}),
        );
        //100 sent, 200 received of which 150 are duplicates → 50 unique received, 50 lost;
        //a naive subtraction (100 - 200) would underflow here
        assert_eq!(json["summary"]["packets_lost"], 50);
    }

    #[test]
    fn interval_result_rejects_malformed_input() {
        //no family
        assert!(interval_result_from_json(serde_json::json!({})).is_err());
        //unknown family
        assert!(interval_result_from_json(serde_json::json!({
            "family": "sctp",
            "kind": "receive",
        }))
        .is_err());
        //no kind
        assert!(interval_result_from_json(serde_json::json!({
            "family": "tcp",
        }))
        .is_err());
        //unknown kind
        assert!(interval_result_from_json(serde_json::json!({
            "family": "tcp",
            "kind": "other",
        }))
        .is_err());
    }

    #[test]
    fn udp_send_result_tolerates_missing_sends_blocked() {
        //pre-0.1.8 peers don't report sends_blocked
        let mut value = udp_send(1.0, 0, 1.0, 1024, 1).to_json();
        value.as_object_mut().unwrap().remove("sends_blocked");

        let result = UdpSendResult::from_json(value).unwrap();
        assert_eq!(result.sends_blocked, 0);
    }

    #[test]
    fn result_json_omits_stream_index() {
        //per-interval entries are grouped under their stream, so the index must not be repeated
        assert!(tcp_receive(1.0, 5, 1.0, 10)
            .to_result_json()
            .get("stream_idx")
            .is_none());
        assert!(tcp_send(1.0, 5, 1.0, 10)
            .to_result_json()
            .get("stream_idx")
            .is_none());
        assert!(udp_receive(1.0, 5, 1.0, 10, 1, None)
            .to_result_json()
            .get("stream_idx")
            .is_none());
        assert!(udp_send(1.0, 5, 1.0, 10, 1)
            .to_result_json()
            .get("stream_idx")
            .is_none());
    }

    #[test]
    fn receive_result_to_string_handles_zero_duration() {
        //a failed stream can report a zero duration; this must not divide by zero
        let result = tcp_receive(1.0, 0, 0.0, 0);
        let output = result.to_string(true);
        assert!(output.contains("TCP receive result"));
        assert!(output.contains("per second: 0.000"));
    }

    #[test]
    fn receive_result_to_string_reports_throughput() {
        //1 megabyte/second is 8 megabits/second
        let output = tcp_receive(1.0, 0, 1.0, 1_000_000).to_string(true);
        assert!(output.contains("megabits/second: 8.000"));

        let output = tcp_receive(1.0, 0, 1.0, 1_000_000).to_string(false);
        assert!(output.contains("megabytes/second: 1.000"));
    }

    #[test]
    fn tcp_test_results_lifecycle() {
        let mut results = TcpTestResults::new();
        assert!(results.is_success());
        assert_eq!(results.count_in_progress_streams(), 0);

        results.prepare_index(&0);
        results.prepare_index(&1);
        assert_eq!(results.count_in_progress_streams(), 2);
        assert!(!results.is_success());

        //server-side accounting tracks the same indices
        assert_eq!(results.count_in_progress_streams_server(), 2);
        results.mark_stream_done_server(&0);
        assert_eq!(results.count_in_progress_streams_server(), 1);

        results.mark_stream_done(&0, true);
        assert!(!results.is_success());
        results.mark_stream_done(&1, true);
        assert!(results.is_success());
    }

    #[test]
    fn tcp_test_results_tracks_failures() {
        let mut results = TcpTestResults::new();
        results.prepare_index(&0);
        results.mark_stream_done(&0, false);
        assert!(!results.is_success());
    }

    #[test]
    fn tcp_test_results_accepts_and_summarises_intervals() {
        let mut results = TcpTestResults::new();
        results.prepare_index(&0);

        results
            .update_from_json(serde_json::json!({
                "family": "tcp",
                "kind": "send",
                "stream_idx": 0,
                "timestamp": 1.0,
                "duration": 1.0,
                "bytes_sent": 1000,
                "sends_blocked": 0,
            }))
            .unwrap();
        results
            .update_from_json(serde_json::json!({
                "family": "tcp",
                "kind": "receive",
                "stream_idx": 0,
                "timestamp": 1.0,
                "duration": 1.0,
                "bytes_received": 900,
            }))
            .unwrap();

        //unknown stream-indices are rejected
        let unknown = serde_json::json!({
            "family": "tcp",
            "kind": "receive",
            "stream_idx": 7,
            "timestamp": 1.0,
            "duration": 1.0,
            "bytes_received": 1,
        });
        assert!(results.update_from_json(unknown).is_err());

        results.mark_stream_done(&0, true);
        let output = results.to_json(
            0,
            serde_json::json!({"upload": true}),
            serde_json::json!({}),
            serde_json::json!({}),
            serde_json::json!({}),
        );

        assert_eq!(output["success"], true);
        assert_eq!(output["summary"]["bytes_sent"], 1000);
        assert_eq!(output["summary"]["bytes_received"], 900);
        assert_eq!(output["summary"]["duration_send"], 1.0);
        assert_eq!(
            output["config"]["upload"],
            serde_json::json!({"upload": true})
        );
        assert_eq!(output["streams"].as_array().unwrap().len(), 1);
        let stream = &output["streams"][0];
        assert_eq!(stream["abandoned"], false);
        assert_eq!(stream["failed"], false);
        assert_eq!(stream["intervals"]["send"].as_array().unwrap().len(), 1);
        assert_eq!(stream["intervals"]["receive"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn tcp_test_results_omit_seconds_excludes_early_intervals() {
        let mut results = TcpTestResults::new();
        results.prepare_index(&0);
        for i in 0..3 {
            results
                .update_from_json(serde_json::json!({
                    "family": "tcp",
                    "kind": "send",
                    "stream_idx": 0,
                    "timestamp": f64::from(i),
                    "duration": 1.0,
                    "bytes_sent": 1000,
                    "sends_blocked": 0,
                }))
                .unwrap();
        }
        results.mark_stream_done(&0, true);

        let output = results.to_json(
            1,
            serde_json::json!({}),
            serde_json::json!({}),
            serde_json::json!({}),
            serde_json::json!({}),
        );
        //only 2 of the 3 intervals are counted in the summary
        assert_eq!(output["summary"]["bytes_sent"], 2000);
        assert_eq!(output["summary"]["duration_send"], 2.0);
    }

    #[test]
    fn udp_test_results_lifecycle() {
        let mut results = UdpTestResults::new();
        assert!(results.is_success());

        results.prepare_index(&0);
        results.prepare_index(&1);
        assert_eq!(results.count_in_progress_streams(), 2);

        results.mark_stream_done(&0, true);
        results.mark_stream_done(&1, false);
        assert!(!results.is_success());
    }

    #[test]
    fn udp_test_results_summarise_losses_and_jitter() {
        let mut results = UdpTestResults::new();
        results.prepare_index(&0);

        results
            .update_from_json(serde_json::json!({
                "family": "udp",
                "kind": "send",
                "stream_idx": 0,
                "timestamp": 1.0,
                "duration": 1.0,
                "bytes_sent": 2048,
                "packets_sent": 2,
                "sends_blocked": 0,
            }))
            .unwrap();
        //the first interval has no jitter, the second does
        results
            .update_from_json(serde_json::json!({
                "family": "udp",
                "kind": "receive",
                "stream_idx": 0,
                "timestamp": 1.0,
                "duration": 1.0,
                "bytes_received": 1024,
                "packets_received": 1,
                "packets_lost": 0,
                "packets_out_of_order": 0,
                "packets_duplicated": 0,
                "unbroken_sequence": 1,
                "jitter_seconds": null,
            }))
            .unwrap();
        results
            .update_from_json(serde_json::json!({
                "family": "udp",
                "kind": "receive",
                "stream_idx": 0,
                "timestamp": 2.0,
                "duration": 1.0,
                "bytes_received": 1024,
                "packets_received": 1,
                "packets_lost": 1,
                "packets_out_of_order": 0,
                "packets_duplicated": 0,
                "unbroken_sequence": 10,
                "jitter_seconds": 0.1,
            }))
            .unwrap();

        results.mark_stream_done(&0, true);
        let output = results.to_json(
            0,
            serde_json::json!({}),
            serde_json::json!({}),
            serde_json::json!({}),
            serde_json::json!({}),
        );

        let summary = &output["summary"];
        assert_eq!(summary["packets_sent"], 2);
        assert_eq!(summary["packets_received"], 2);
        //loss is calculated as sent minus received
        assert_eq!(summary["packets_lost"], 0);
        assert_eq!(summary["framed_packet_size"], 1024);
        //jitter is weighted by the length of unbroken sequences
        assert!((summary["jitter_average"].as_f64().unwrap() - 0.1).abs() < 1e-6);
        assert_eq!(summary["jitter_packets_consecutive"], 10);
    }

    #[test]
    fn udp_test_results_omit_jitter_fields_when_not_calculated() {
        let mut results = UdpTestResults::new();
        results.prepare_index(&0);
        results
            .update_from_json(serde_json::json!({
                "family": "udp",
                "kind": "send",
                "stream_idx": 0,
                "timestamp": 1.0,
                "duration": 1.0,
                "bytes_sent": 1024,
                "packets_sent": 1,
                "sends_blocked": 0,
            }))
            .unwrap();
        results
            .update_from_json(serde_json::json!({
                "family": "udp",
                "kind": "receive",
                "stream_idx": 0,
                "timestamp": 1.0,
                "duration": 1.0,
                "bytes_received": 1024,
                "packets_received": 1,
                "packets_lost": 0,
                "packets_out_of_order": 0,
                "packets_duplicated": 0,
                "unbroken_sequence": 1,
                "jitter_seconds": null,
            }))
            .unwrap();
        results.mark_stream_done(&0, true);

        let output = results.to_json(
            0,
            serde_json::json!({}),
            serde_json::json!({}),
            serde_json::json!({}),
            serde_json::json!({}),
        );
        assert!(output["summary"].get("jitter_average").is_none());
    }
}
