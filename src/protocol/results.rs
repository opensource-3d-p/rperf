extern crate log;

pub trait IntervalResult {
    fn to_json(&self) -> serde_json::Value;
    
    fn to_json_string(&self) -> String {
        serde_json::to_string(&self.to_json()).unwrap()
    }
}

pub struct UdpReceiveResult {
    duration: f32,
    
    bytes_received: u64,
    packets_received: u64,
    lost_packets: i64,
    out_of_order_packets: u64,
    duplicate_packets: u64,
    
    unbroken_sequence: u64,
    jitter_seconds: Option<f32>,
}
impl IntervalResult for UdpReceiveResult {
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({})
    }
}

pub struct UdpSendResult {
    duration: f32,
    
    bytes_sent: u64,
    packets_sent: u64,
}
impl IntervalResult for UdpSendResult {
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({})
    }
}

struct UdpStreamResult {
    receive_result: UdpReceiveResult,
    send_result: UdpSendResult,
}

struct UdpTestResult {
    stream_results: Vec<UdpStreamResult>,
}


//TODO: functions to assemble UDP results into something complete
//or maybe multiple outputs: one after each transmission cycle and one after each receipt
//with a running tally for the final output
//client.rs should do the actual presentation step
