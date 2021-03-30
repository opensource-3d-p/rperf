extern crate log;

use std::collections::HashMap;
use std::error::Error;
use std::fmt::format;

use serde::{Serialize, Deserialize};

type BoxResult<T> = Result<T,Box<dyn Error>>;


pub trait IntervalResult {
    fn to_json(&self) -> serde_json::Value;
    
    //produces test-results in tabular form
    fn to_string(&self, bit:bool) -> String;
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
    fn to_json(&self) -> serde_json::Value {
        let mut serialised = serde_json::to_value(self).unwrap();
        serialised["family"] = serde_json::json!("udp");
        serialised["kind"] = serde_json::json!("receive");
        serialised
    }
    
    fn to_string(&self, bit:bool) -> String {
        let mut duration_divisor = self.duration;
        if duration_divisor == 0.0 { //avoid zerodiv, which should be impossible, but safety
            duration_divisor = 1.0;
        }
        
        let bytes_per_second = self.bytes_received as f32 / duration_divisor;
        
        let throughput = match bit {
            true => format!("megabits/second: {:.3}", bytes_per_second / (1_000_000.00 / 8.0)),
            false => format!("megabytes/second: {:.3}", bytes_per_second / 1_000_000.00),
        };
        
        let mut output = format!("----------\n\
                 UDP send result over {:.2}s\n\
                 bytes: {} | per second: {:.3} | {}\n\
                 packets: {} | lost: {} | out-of-order: {} | duplicate: {} | per second: {:.3}",
                 self.duration,
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
    fn to_json(&self) -> serde_json::Value {
        let mut serialised = serde_json::to_value(self).unwrap();
        serialised["family"] = serde_json::json!("udp");
        serialised["kind"] = serde_json::json!("send");
        serialised
    }
    
    fn to_string(&self, bit:bool) -> String {
        let mut duration_divisor = self.duration;
        if duration_divisor == 0.0 { //avoid zerodiv, which should be impossible, but safety
            duration_divisor = 1.0;
        }
        
        let bytes_per_second = self.bytes_sent as f32 / duration_divisor;
        
        let throughput = match bit {
            true => format!("megabits/second: {:.3}", bytes_per_second / (1_000_000.00 / 8.0)),
            false => format!("megabytes/second: {:.3}", bytes_per_second / 1_000_000.00),
        };
        
        format!("----------\n\
                 UDP receive result over {:.2}s\n\
                 bytes: {} | per second: {:.3} | {}\n\
                 packets: {} per second: {:.3}",
                 self.duration,
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
}
impl UdpTestResults {
    pub fn new() -> UdpTestResults {
        UdpTestResults{
            stream_results: HashMap::new(),
        }
    }
    pub fn prepare_index(&mut self, idx:&u8) {
        self.stream_results.insert(*idx, UdpStreamResults{
            receive_results: Vec::new(),
            send_results: Vec::new(),
        });
    }
}
impl TestResults for UdpTestResults {
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
