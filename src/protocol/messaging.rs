extern crate log;


pub fn prepare_begin() -> serde_json::Value {
    serde_json::json!({
        "kind": "begin",
    })
}

pub fn prepare_connect(stream_ports:&[u16]) -> serde_json::Value {
    serde_json::json!({
        "kind": "connect",
        
        "streamPorts": stream_ports,
    })
}

pub fn prepare_connected() -> serde_json::Value {
    serde_json::json!({
        "kind": "connected",
    })
}

pub fn prepare_end() -> serde_json::Value {
    serde_json::json!({
        "kind": "end",
    })
}

pub fn prepare_configuration_udp_upload(test_id:[u8; 16], streams:u8, bandwidth:u64, bytes:u64, seconds:f32, length:u32, send_interval:f32) -> serde_json::Value {
    serde_json::json!({
        "kind": "configuration",
        
        "family": "udp",
        "role": "upload",
        
        "testId": test_id,
        "streams": configuration::validate_streams(streams),
        
        "bandwidth": configuration::validate_bandwidth(bandwidth),
        "duration": configuration::calculate_duration(bandwidth, bytes, seconds),
        "length": configuration::calculate_length_udp(length),
        "sendInterval": configuration::validate_send_interval(send_interval),
    })
}

pub fn prepare_configuration_udp_download(test_id:[u8; 16], streams:u8, length:u32) -> serde_json::Value {
    serde_json::json!({
        "kind": "configuration",
        
        "family": "udp",
        "role": "download",
        
        "testId": test_id,
        "streams": configuration::validate_streams(streams),
        
        "length": configuration::calculate_length_udp(length),
    })
}
