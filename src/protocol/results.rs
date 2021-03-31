extern crate log;

use std::collections::{HashMap, HashSet};
use std::error::Error;

use serde::{Serialize, Deserialize};

type BoxResult<T> = Result<T,Box<dyn Error>>;


#[derive(PartialEq)]
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

pub trait IntervalResult {
    fn kind(&self) -> IntervalResultKind;
    
    fn get_stream_idx(&self) -> u8;
    
    fn to_json(&self) -> serde_json::Value;
    
    //produces test-results in tabular form
    fn to_string(&self, bit:bool) -> String;
}

pub struct ClientDoneResult {
    pub stream_idx: u8,
}
impl IntervalResult for ClientDoneResult {
    fn kind(&self) -> IntervalResultKind{
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
    
    fn to_string(&self, _bit:bool) -> String {
        format!("----------\n\
                 End of stream from client | stream: {}",
                self.stream_idx,
        )
    }
}
pub struct ServerDoneResult {
    pub stream_idx: u8,
}
impl IntervalResult for ServerDoneResult {
    fn kind(&self) -> IntervalResultKind{
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
    
    fn to_string(&self, _bit:bool) -> String {
        format!("----------\n\
                 End of stream from server | stream: {}",
                self.stream_idx,
        )
    }
}

pub struct ClientFailedResult {
    pub stream_idx: u8,
}
impl IntervalResult for ClientFailedResult {
    fn kind(&self) -> IntervalResultKind{
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
    
    fn to_string(&self, _bit:bool) -> String {
        format!("----------\n\
                 Failure in client stream | stream: {}",
                self.stream_idx,
        )
    }
}
pub struct ServerFailedResult {
    pub stream_idx: u8,
}
impl IntervalResult for ServerFailedResult {
    fn kind(&self) -> IntervalResultKind{
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
    
    fn to_string(&self, _bit:bool) -> String {
        format!("----------\n\
                 Failure in server stream | stream: {}",
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
    pub packets_lost: i64,
    pub packets_out_of_order: u64,
    pub packets_duplicate: u64,
    
    pub unbroken_sequence: u64,
    pub jitter_seconds: Option<f32>,
}
impl UdpReceiveResult {
    fn from_json(value:serde_json::Value) -> BoxResult<UdpReceiveResult> {
        let receive_result:UdpReceiveResult = serde_json::from_value(value)?;
        Ok(receive_result)
    }
    
    fn to_result_json(&self) -> serde_json::Value {
        let mut serialised = serde_json::to_value(self).unwrap();
        serialised.as_object_mut().unwrap().remove("stream_idx");
        serialised
    }
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
                self.packets_received, self.packets_lost, self.packets_out_of_order, self.packets_duplicate, self.packets_received as f32 / duration_divisor,
        );
        if self.jitter_seconds.is_some() {
            output.push_str(&format!("\njitter: {:.6}s over {} consecutive packets", self.jitter_seconds.unwrap(), self.unbroken_sequence));
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
impl UdpSendResult {
    fn from_json(value:serde_json::Value) -> BoxResult<UdpSendResult> {
        let send_result:UdpSendResult = serde_json::from_value(value)?;
        Ok(send_result)
    }
    
    fn to_result_json(&self) -> serde_json::Value {
        let mut serialised = serde_json::to_value(self).unwrap();
        serialised.as_object_mut().unwrap().remove("stream_idx");
        serialised
    }
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
                            "receive" => Ok(Box::new(UdpReceiveResult::from_json(value)?)),
                            "send" => Ok(Box::new(UdpSendResult::from_json(value)?)),
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
    
    fn to_json(&self) -> serde_json::Value;
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
                    "send" => Ok(self.send_results.push(UdpSendResult::from_json(value)?)),
                    "receive" => Ok(self.receive_results.push(UdpReceiveResult::from_json(value)?)),
                    _ => Err(Box::new(simple_error::simple_error!("unsupported kind for UDP stream-result: {}", kind))),
                },
                None => Err(Box::new(simple_error::simple_error!("kind must be a string for UDP stream-result"))),
            },
            None => Err(Box::new(simple_error::simple_error!("no kind specified for UDP stream-result"))),
        }
    }
    
    fn to_json(&self) -> serde_json::Value {
        let mut duration_send:f64 = 0.0;
        let mut bytes_sent:u64 = 0;
        let mut packets_sent:u64 = 0;
        
        
        let mut duration_receive:f64 = 0.0;
        
        let mut bytes_received:u64 = 0;
        let mut packets_received:u64 = 0;
        let mut packets_out_of_order:u64 = 0;
        let mut packets_duplicate:u64 = 0;
        
        let mut jitter_calculated = false;
        let mut unbroken_sequence_count:u64 = 0;
        let mut jitter_weight:f64 = 0.0;
        
        for sr in &(self.send_results) {
            duration_send += sr.duration as f64;
            
            bytes_sent += sr.bytes_sent;
            packets_sent += sr.packets_sent;
        }
        
        for rr in &(self.receive_results) {
            duration_receive += rr.duration as f64;
            
            bytes_received += rr.bytes_received;
            packets_received += rr.packets_received;
            packets_out_of_order += rr.packets_out_of_order;
            packets_duplicate += rr.packets_duplicate;
            
            if rr.jitter_seconds.is_some() {
                jitter_weight += (rr.unbroken_sequence as f64) * (rr.jitter_seconds.unwrap() as f64);
                unbroken_sequence_count += rr.unbroken_sequence;
                
                jitter_calculated = true;
            }
        }
        
        let mut summary = serde_json::json!({
            "framed_packet_size": bytes_sent / packets_sent,
            
            "duration_send": duration_send,
            "bytes_sent": bytes_sent,
            "packets_sent": packets_sent,
            
            
            "duration_receive": duration_receive,
            
            "bytes_received": bytes_received,
            "packets_received": packets_received,
            "packets_lost": packets_sent - packets_received,
            "packets_out_of_order": packets_out_of_order,
            "packets_duplicate": packets_duplicate,
        });
        if jitter_calculated {
            summary["jitter_average"] = serde_json::json!(jitter_weight / (unbroken_sequence_count as f64));
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
    fn mark_stream_done(&mut self, idx:&u8, success:bool);
    
    fn count_in_progress_streams_server(&self) -> u8;
    fn mark_stream_done_server(&mut self, idx:&u8);
    
    fn is_success(&self) -> bool;
    
    fn update_from_json(&mut self, value:serde_json::Value) -> BoxResult<()>;
    
    fn to_json(&self) -> serde_json::Value;
    
    //produces a pretty-printed JSON string with the test results
    fn to_json_string(&self) -> String {
        serde_json::to_string_pretty(&self.to_json()).unwrap()
    }
    
    //produces test-results in tabular form
    fn to_string(&self, bit:bool) -> String;
}

pub struct UdpTestResults {
    stream_results: HashMap<u8, UdpStreamResults>,
    pending_tests: HashSet<u8>,
    failed_tests: HashSet<u8>,
    server_tests_finished: HashSet<u8>,
}
impl UdpTestResults {
    pub fn new() -> UdpTestResults {
        UdpTestResults{
            stream_results: HashMap::new(),
            pending_tests: HashSet::new(),
            failed_tests: HashSet::new(),
            server_tests_finished: HashSet::new(),
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
    fn count_in_progress_streams(&self) -> u8 {
        self.pending_tests.len() as u8
    }
    fn mark_stream_done(&mut self, idx:&u8, success:bool) {
        self.pending_tests.remove(idx);
        if !success {
            self.failed_tests.insert(*idx);
        }
    }
    
    fn count_in_progress_streams_server(&self) -> u8 {
        let mut count:u8 = 0;
        for idx in self.stream_results.keys() {
            if !self.server_tests_finished.contains(idx) {
                count += 1;
            }
        }
        count
    }
    fn mark_stream_done_server(&mut self, idx:&u8) {
        self.server_tests_finished.insert(*idx);
    }
    
    fn is_success(&self) -> bool {
        self.pending_tests.len() == 0 && self.failed_tests.len() == 0
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
        let mut duration_send:f64 = 0.0;
        let mut bytes_sent:u64 = 0;
        let mut packets_sent:u64 = 0;
        
        
        let mut duration_receive:f64 = 0.0;
        
        let mut bytes_received:u64 = 0;
        let mut packets_received:u64 = 0;
        let mut packets_out_of_order:u64 = 0;
        let mut packets_duplicate:u64 = 0;
        
        let mut jitter_calculated = false;
        let mut unbroken_sequence_count:u64 = 0;
        let mut jitter_weight:f64 = 0.0;
        
        
        let mut streams = Vec::with_capacity(self.stream_results.len());
        for (idx, stream) in self.stream_results.iter() {
            streams.push(serde_json::json!({
                "intervals": stream.to_json(),
                "abandoned": self.pending_tests.contains(idx),
                "failed": self.failed_tests.contains(idx),
            }));
            
            for sr in &(stream.send_results) {
                duration_send += sr.duration as f64;
                
                bytes_sent += sr.bytes_sent;
                packets_sent += sr.packets_sent;
            }
            
            for rr in &(stream.receive_results) {
                duration_receive += rr.duration as f64;
                
                bytes_received += rr.bytes_received;
                packets_received += rr.packets_received;
                packets_out_of_order += rr.packets_out_of_order;
                packets_duplicate += rr.packets_duplicate;
                
                if rr.jitter_seconds.is_some() {
                    jitter_weight += (rr.unbroken_sequence as f64) * (rr.jitter_seconds.unwrap() as f64);
                    unbroken_sequence_count += rr.unbroken_sequence;
                    
                    jitter_calculated = true;
                }
            }
        }
        
        let mut summary = serde_json::json!({
            "framed_packet_size": bytes_sent / packets_sent,
            
            "duration_send": duration_send,
            "bytes_sent": bytes_sent,
            "packets_sent": packets_sent,
            
            
            "duration_receive": duration_receive,
            
            "bytes_received": bytes_received,
            "packets_received": packets_received,
            "packets_lost": packets_sent - packets_received,
            "packets_out_of_order": packets_out_of_order,
            "packets_duplicate": packets_duplicate,
        });
        if jitter_calculated {
            summary["jitter_average"] = serde_json::json!(jitter_weight / (unbroken_sequence_count as f64));
            summary["jitter_packets_consecutive"] = serde_json::json!(unbroken_sequence_count);
        }
        
        serde_json::json!({
            "streams": streams,
            "summary": summary,
            "success": self.is_success(),
        })
    }
    
    fn to_string(&self, bit:bool) -> String {
        let mut duration_send:f64 = 0.0;
        let mut bytes_sent:u64 = 0;
        let mut packets_sent:u64 = 0;
        
        
        let mut duration_receive:f64 = 0.0;
        
        let mut bytes_received:u64 = 0;
        let mut packets_received:u64 = 0;
        let mut packets_out_of_order:u64 = 0;
        let mut packets_duplicate:u64 = 0;
        
        let mut jitter_calculated = false;
        let mut unbroken_sequence_count:u64 = 0;
        let mut jitter_weight:f64 = 0.0;
        
        
        for stream in self.stream_results.values() {
            for sr in &(stream.send_results) {
                duration_send += sr.duration as f64;
                
                bytes_sent += sr.bytes_sent;
                packets_sent += sr.packets_sent;
            }
            
            for rr in &(stream.receive_results) {
                duration_receive += rr.duration as f64;
                
                bytes_received += rr.bytes_received;
                packets_received += rr.packets_received;
                packets_out_of_order += rr.packets_out_of_order;
                packets_duplicate += rr.packets_duplicate;
                
                if rr.jitter_seconds.is_some() {
                    jitter_weight += (rr.unbroken_sequence as f64) * (rr.jitter_seconds.unwrap() as f64);
                    unbroken_sequence_count += rr.unbroken_sequence;
                    
                    jitter_calculated = true;
                }
            }
        }
        
        let send_duration_divisor;
        if duration_send == 0.0 { //avoid zerodiv, which should be impossible, but safety
            send_duration_divisor = 1.0;
        } else {
            send_duration_divisor = duration_send;
        }
        let send_bytes_per_second = bytes_sent as f64 / send_duration_divisor;
        let send_throughput = match bit {
            true => format!("megabits/second: {:.3}", send_bytes_per_second / (1_000_000.00 / 8.0)),
            false => format!("megabytes/second: {:.3}", send_bytes_per_second / 1_000_000.00),
        };
        
        let receive_duration_divisor;
        if duration_receive == 0.0 { //avoid zerodiv, which should be impossible, but safety
            receive_duration_divisor = 1.0;
        } else {
            receive_duration_divisor = duration_receive;
        }
        let receive_bytes_per_second = bytes_received as f64 / receive_duration_divisor;
        let receive_throughput = match bit {
            true => format!("megabits/second: {:.3}", receive_bytes_per_second / (1_000_000.00 / 8.0)),
            false => format!("megabytes/second: {:.3}", receive_bytes_per_second / 1_000_000.00),
        };
        
        let mut output = format!("==========\n\
                                  UDP send result over {:.2}s | {} streams\n\
                                  bytes: {} | per second: {:.3} | {}\n\
                                  packets: {} per second: {:.3}\n\
                                  ==========\n\
                                  UDP receive result over {:.2}s | {} streams\n\
                                  bytes: {} | per second: {:.3} | {}\n\
                                  packets: {} | lost: {} | out-of-order: {} | duplicate: {} | per second: {:.3}",
                                duration_send, self.stream_results.len(),
                                bytes_sent, send_bytes_per_second, send_throughput,
                                packets_sent, packets_sent as f64 / send_duration_divisor,
                                
                                duration_receive, self.stream_results.len(),
                                bytes_received, receive_bytes_per_second, receive_throughput,
                                packets_received, packets_sent - packets_received, packets_out_of_order, packets_duplicate, packets_received as f64 / receive_duration_divisor,
        );
        if jitter_calculated {
            output.push_str(&format!("\njitter: {:.6}s over {} consecutive packets", jitter_weight / (unbroken_sequence_count as f64), unbroken_sequence_count));
        }
        if !self.is_success() {
            output.push_str(&format!("\nTESTING DID NOT COMPLETE SUCCESSFULLY"));
        }
        
        output
    }
}














/*
//TODO: apply this to UDP, too
Just make it an argument to the serialisers


        
        .arg(
            Arg::with_name("omit")
                .help("omit a number of seconds from the start of calculations to avoid including TCP ramp-up in averages")
                .takes_value(true)
                .long("omit")
                .short("O")
                .default_value("0")
                .required(false)
        )
        
        
*/
