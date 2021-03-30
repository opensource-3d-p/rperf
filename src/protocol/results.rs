extern crate log;

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt::format;

use serde::{Serialize, Deserialize};

type BoxResult<T> = Result<T,Box<dyn Error>>;


#[derive(PartialEq)]
pub enum IntervalResultKind {
    Done,
    TcpReceive,
    TcpSend,
    UdpReceive,
    UdpSend,
}

pub trait IntervalResult {
    fn kind(&self) -> IntervalResultKind;
    
    fn get_stream_idx(&self) -> u8;
    
    fn to_json(&self) -> serde_json::Value;
    
    //produces test-results in tabular form
    fn to_string(&self, bit:bool) -> String;
}

pub struct DoneResult {
    pub stream_idx: u8,
}
impl IntervalResult for DoneResult {
    fn kind(&self) -> IntervalResultKind{
        IntervalResultKind::Done
    }
    
    fn get_stream_idx(&self) -> u8 {
        self.stream_idx
    }
    
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "kind": "done",
            "stream_idx": self.stream_idx,
        })
    }
    
    fn to_string(&self, bit:bool) -> String {
        format!("----------\n\
                 End of stream | stream: {}",
                self.stream_idx,
        )
    }
}

#[derive(Serialize, Deserialize)]
pub struct UdpReceiveResult {
    pub stream_idx: u8,
    
    pub duration: f32,
    
    pub bytes_received: u64,
    pub packets_received: u64,
    pub lost_packets: i64,
    pub out_of_order_packets: u64,
    pub duplicate_packets: u64,
    
    pub unbroken_sequence: u64,
    pub jitter_seconds: Option<f32>,
}
fn udp_receive_result_from_json(value:serde_json::Value) -> BoxResult<UdpReceiveResult> {
    let receive_result:UdpReceiveResult = serde_json::from_value(value)?;
    Ok(receive_result)
}
impl IntervalResult for UdpReceiveResult {
    fn kind(&self) -> IntervalResultKind{
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
    
    fn to_string(&self, bit:bool) -> String {
        let duration_divisor;
        if self.duration == 0.0 { //avoid zerodiv, which should be impossible, but safety
            duration_divisor = 1.0;
        } else {
            duration_divisor = self.duration;
        }
        
        let bytes_per_second = self.bytes_received as f32 / duration_divisor;
        
        let throughput = match bit {
            true => format!("megabits/second: {:.3}", bytes_per_second / (1_000_000.00 / 8.0)),
            false => format!("megabytes/second: {:.3}", bytes_per_second / 1_000_000.00),
        };
        
        let mut output = format!("----------\n\
                 UDP receive result over {:.2}s | stream: {}\n\
                 bytes: {} | per second: {:.3} | {}\n\
                 packets: {} | lost: {} | out-of-order: {} | duplicate: {} | per second: {:.3}",
                self.duration, self.stream_idx,
                self.bytes_received, bytes_per_second, throughput,
                self.packets_received, self.lost_packets, self.out_of_order_packets, self.duplicate_packets, self.packets_received as f32 / duration_divisor,
        );
        if self.jitter_seconds.is_some() {
            output.push_str(&format!("\njitter: {:.6}s over {} packets", self.jitter_seconds.unwrap(), self.unbroken_sequence));
        }
        output
    }
}

#[derive(Serialize, Deserialize)]
pub struct UdpSendResult {
    pub stream_idx: u8,
    
    pub duration: f32,
    
    pub bytes_sent: u64,
    pub packets_sent: u64,
}
fn udp_send_result_from_json(value:serde_json::Value) -> BoxResult<UdpSendResult> {
    let send_result:UdpSendResult = serde_json::from_value(value)?;
    Ok(send_result)
}
impl IntervalResult for UdpSendResult {
    fn kind(&self) -> IntervalResultKind{
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
    
    fn to_string(&self, bit:bool) -> String {
        let duration_divisor;
        if self.duration == 0.0 { //avoid zerodiv, which should be impossible, but safety
            duration_divisor = 1.0;
        } else {
            duration_divisor = self.duration;
        }
        
        let bytes_per_second = self.bytes_sent as f32 / duration_divisor;
        
        let throughput = match bit {
            true => format!("megabits/second: {:.3}", bytes_per_second / (1_000_000.00 / 8.0)),
            false => format!("megabytes/second: {:.3}", bytes_per_second / 1_000_000.00),
        };
        
        format!("----------\n\
                 UDP send result over {:.2}s | stream: {}\n\
                 bytes: {} | per second: {:.3} | {}\n\
                 packets: {} per second: {:.3}",
                self.duration, self.stream_idx,
                self.bytes_sent, bytes_per_second, throughput,
                self.packets_sent, self.packets_sent as f32 / duration_divisor,
        )
    }
}


pub fn interval_result_from_json(value:serde_json::Value) -> BoxResult<Box<dyn IntervalResult>> {
    match value.get("family") {
        Some(f) => match f.as_str() {
            Some(family) => match family {
                "udp" => match value.get("kind") {
                    Some(k) => match k.as_str() {
                        Some(kind) => match kind {
                            "receive" => Ok(Box::new(udp_receive_result_from_json(value)?)),
                            "send" => Ok(Box::new(udp_send_result_from_json(value)?)),
                            _ => Err(Box::new(simple_error::simple_error!("unsupported interval-result kind: {}", kind))),
                        },
                        None => Err(Box::new(simple_error::simple_error!("interval-result's kind is not a string"))),
                    },
                    None => Err(Box::new(simple_error::simple_error!("interval-result has no kind"))),
                },
                _ => Err(Box::new(simple_error::simple_error!("unsupported interval-result family: {}", family))),
            },
            None => Err(Box::new(simple_error::simple_error!("interval-result's family is not a string"))),
        },
        None => Err(Box::new(simple_error::simple_error!("interval-result has no family"))),
    }
}


pub trait StreamResults {
    fn update_from_json(&mut self, value:serde_json::Value) -> BoxResult<()>;
}

struct UdpStreamResults {
    receive_results: Vec<UdpReceiveResult>,
    send_results: Vec<UdpSendResult>,
}
impl StreamResults for UdpStreamResults {
    fn update_from_json(&mut self, value:serde_json::Value) -> BoxResult<()> {
        match value.get("kind") {
            Some(k) => match k.as_str() {
                Some(kind) => match kind {
                    "send" => Ok(self.send_results.push(udp_send_result_from_json(value)?)),
                    "receive" => Ok(self.receive_results.push(udp_receive_result_from_json(value)?)),
                    _ => Err(Box::new(simple_error::simple_error!("unsupported kind for UDP stream-result: {}", kind))),
                },
                None => Err(Box::new(simple_error::simple_error!("kind must be a string for UDP stream-result"))),
            },
            None => Err(Box::new(simple_error::simple_error!("no kind specified for UDP stream-result"))),
        }
    }
}


pub trait TestResults {
    fn count_in_progress_streams(&mut self) -> u8;
    fn mark_stream_done(&mut self, idx:&u8);
    
    fn update_from_json(&mut self, value:serde_json::Value) -> BoxResult<()>;
    
    fn to_json(&self) -> serde_json::Value;
    
    //produces a pretty-printed JSON string with the test results
    fn to_json_string(&self, value:serde_json::Value) -> String {
        serde_json::to_string_pretty(&value).unwrap()
    }
    
    //produces test-results in tabular form
    fn to_string(&self, bit:bool) -> String;
}

pub struct UdpTestResults {
    stream_results: HashMap<u8, UdpStreamResults>,
    pending_tests: HashSet<u8>,
}
impl UdpTestResults {
    pub fn new() -> UdpTestResults {
        UdpTestResults{
            stream_results: HashMap::new(),
            pending_tests: HashSet::new(),
        }
    }
    pub fn prepare_index(&mut self, idx:&u8) {
        self.stream_results.insert(*idx, UdpStreamResults{
            receive_results: Vec::new(),
            send_results: Vec::new(),
        });
        self.pending_tests.insert(*idx);
    }
}
impl TestResults for UdpTestResults {
    fn count_in_progress_streams(&mut self) -> u8 {
        return self.pending_tests.len() as u8;
    }
    fn mark_stream_done(&mut self, idx:&u8) {
        self.pending_tests.remove(idx);
    }
    
    fn update_from_json(&mut self, value:serde_json::Value) -> BoxResult<()> {
        match value.get("family") {
            Some(f) => match f.as_str() {
                Some(family) => match family {
                    "udp" => match value.get("stream_idx") {
                        Some(idx) => match idx.as_i64() {
                            Some(idx64) => match self.stream_results.get_mut(&(idx64 as u8)) {
                                Some(stream_results) => stream_results.update_from_json(value),
                                None => Err(Box::new(simple_error::simple_error!("stream-index {} is not a valid identifier", idx64))),
                            },
                            None => Err(Box::new(simple_error::simple_error!("stream-index is not an integer"))),
                        },
                        None => Err(Box::new(simple_error::simple_error!("no stream-index specified"))),
                    },
                    _ => Err(Box::new(simple_error::simple_error!("unsupported family for UDP stream-result: {}", family))),
                },
                None => Err(Box::new(simple_error::simple_error!("kind must be a string for UDP stream-result"))),
            },
            None => Err(Box::new(simple_error::simple_error!("no kind specified for UDP stream-result"))),
        }
    }
    
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({})
    }
    
    fn to_string(&self, bit:bool) -> String {
        "".to_string()
    }
}
