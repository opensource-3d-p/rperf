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

pub fn prepare_configuration_tcp_upload(test_id:&[u8; 16], streams:u8, bandwidth:u64, seconds:f32, length:u16, send_interval:f32, send_buffer:u32, no_delay:bool) -> serde_json::Value {
    serde_json::json!({
        "kind": "configuration",
        
        "family": "tcp",
        "role": "upload",
        
        "testId": test_id,
        "streams": validate_streams(streams),
        
        "bandwidth": validate_bandwidth(bandwidth),
        "duration": seconds,
        "length": calculate_length_udp(length),
        "sendInterval": validate_send_interval(send_interval),
        
        "sendBuffer": send_buffer,
        "noDelay": no_delay,
    })
}

pub fn prepare_configuration_tcp_download(test_id:&[u8; 16], streams:u8, length:u16, receive_buffer:u32) -> serde_json::Value {
    serde_json::json!({
        "kind": "configuration",
        
        "family": "tcp",
        "role": "download",
        
        "testId": test_id,
        "streams": validate_streams(streams),
        
        "length": calculate_length_udp(length),
        "receiveBuffer": receive_buffer,
    })
}

pub fn prepare_configuration_udp_upload(test_id:&[u8; 16], streams:u8, bandwidth:u64, seconds:f32, length:u16, send_interval:f32, send_buffer:u32) -> serde_json::Value {
    serde_json::json!({
        "kind": "configuration",
        
        "family": "udp",
        "role": "upload",
        
        "testId": test_id,
        "streams": validate_streams(streams),
        
        "bandwidth": validate_bandwidth(bandwidth),
        "duration": seconds,
        "length": calculate_length_udp(length),
        "sendInterval": validate_send_interval(send_interval),
        
        "sendBuffer": send_buffer,
    })
}

pub fn prepare_configuration_udp_download(test_id:&[u8; 16], streams:u8, length:u16, receive_buffer:u32) -> serde_json::Value {
    serde_json::json!({
        "kind": "configuration",
        
        "family": "udp",
        "role": "download",
        
        "testId": test_id,
        "streams": validate_streams(streams),
        
        "length": calculate_length_udp(length),
        "receiveBuffer": receive_buffer,
    })
}






pub fn validate_streams(streams:u8) -> u8 {
    if streams > 0 {
        streams
    } else {
        log::warn!("parallel streams not specified; defaulting to 1");
        1
    }
}

pub fn validate_bandwidth(bandwidth:u64) -> u64 {
    if bandwidth > 0 {
        bandwidth
    } else {
        log::warn!("bandwidth was not specified; defaulting to 1024 bytes/second");
        1024
    }
}

pub fn validate_send_interval(send_interval:f32) -> f32 {
    if send_interval > 0.0 {
        send_interval
    } else {
        log::warn!("send-interval was not specified; defaulting to once per second");
        1.0
    }
}

pub fn calculate_length_udp(length:u16) -> u16 {
    if length < crate::stream::udp::TEST_HEADER_SIZE { //length must be at least enough to hold the test data
        crate::stream::udp::TEST_HEADER_SIZE
    } else {
        length
    }
}
