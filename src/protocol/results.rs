#[macro_use] extern crate log;


struct UdpReceiveResult {
    duration: f64,
    
    bytes_received: u64,
    packets_received: u64,
    lost_packets: i64,
    out_of_order_packets: u64,
    duplicate_packets: u64,
    
    unbroken_sequence: u64,
    jitter_seconds: Option<f32>,
}

struct UdpSendResult {
    duration: f64,
    
    bytes_sent: u64,
    packets_sent: u64,
}
