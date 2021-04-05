use std::error::Error;
type BoxResult<T> = Result<T,Box<dyn Error>>;

/// prepares a message used to tell the server to begin operations
pub fn prepare_begin() -> serde_json::Value {
    serde_json::json!({
        "kind": "begin",
    })
}

/// prepares a message used to tell the client to connect its test-streams
pub fn prepare_connect(stream_ports:&[u16]) -> serde_json::Value {
    serde_json::json!({
        "kind": "connect",
        
        "streamPorts": stream_ports,
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
fn prepare_configuration_tcp_upload(test_id:&[u8; 16], streams:u8, bandwidth:u64, seconds:f32, length:usize, send_interval:f32, send_buffer:u32, no_delay:bool) -> serde_json::Value {
    serde_json::json!({
        "kind": "configuration",
        
        "family": "tcp",
        "role": "upload",
        
        "testId": test_id,
        "streams": validate_streams(streams),
        
        "bandwidth": validate_bandwidth(bandwidth),
        "duration": seconds,
        "length": calculate_length_tcp(length),
        "sendInterval": validate_send_interval(send_interval),
        
        "sendBuffer": send_buffer,
        "noDelay": no_delay,
    })
}

/// prepares a message used to describe the download role of a TCP test
fn prepare_configuration_tcp_download(test_id:&[u8; 16], streams:u8, length:usize, receive_buffer:u32) -> serde_json::Value {
    serde_json::json!({
        "kind": "configuration",
        
        "family": "tcp",
        "role": "download",
        
        "testId": test_id,
        "streams": validate_streams(streams),
        
        "length": calculate_length_tcp(length),
        "receiveBuffer": receive_buffer,
    })
}

/// prepares a message used to describe the upload role of a UDP test
fn prepare_configuration_udp_upload(test_id:&[u8; 16], streams:u8, bandwidth:u64, seconds:f32, length:u16, send_interval:f32, send_buffer:u32) -> serde_json::Value {
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

/// prepares a message used to describe the download role of a UDP test
fn prepare_configuration_udp_download(test_id:&[u8; 16], streams:u8, length:u16, receive_buffer:u32) -> serde_json::Value {
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




fn validate_streams(streams:u8) -> u8 {
    if streams > 0 {
        streams
    } else {
        log::warn!("parallel streams not specified; defaulting to 1");
        1
    }
}

fn validate_bandwidth(bandwidth:u64) -> u64 {
    if bandwidth > 0 {
        bandwidth
    } else {
        log::warn!("bandwidth was not specified; defaulting to 1024 bytes/second");
        1024
    }
}

fn validate_send_interval(send_interval:f32) -> f32 {
    if send_interval > 0.0 {
        send_interval
    } else {
        log::warn!("send-interval was not specified; defaulting to once per second");
        1.0
    }
}

fn calculate_length_tcp(length:usize) -> usize {
    if length < crate::stream::tcp::TEST_HEADER_SIZE { //length must be at least enough to hold the test data
        crate::stream::tcp::TEST_HEADER_SIZE
    } else {
        length
    }
}
fn calculate_length_udp(length:u16) -> u16 {
    if length < crate::stream::udp::TEST_HEADER_SIZE { //length must be at least enough to hold the test data
        crate::stream::udp::TEST_HEADER_SIZE
    } else {
        length
    }
}


/// prepares a message used to describe the upload role in a test
pub fn prepare_upload_configuration(args:&clap::ArgMatches, test_id:&[u8; 16]) -> BoxResult<serde_json::Value> {
    let parallel_streams:u8 = args.value_of("parallel").unwrap().parse()?;
    let bandwidth:u64 = args.value_of("bandwidth").unwrap().parse()?;
    let mut seconds:f32 = args.value_of("time").unwrap().parse()?;
    let mut send_interval:f32 = args.value_of("sendinterval").unwrap().parse()?;
    let mut length:u32 = args.value_of("length").unwrap().parse()?;
    
    let mut send_buffer:u32 = args.value_of("send_buffer").unwrap().parse()?;
    
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
        Ok(prepare_configuration_udp_upload(test_id, parallel_streams, bandwidth, seconds, length as u16, send_interval, send_buffer))
    } else {
        log::debug!("preparing TCP upload config...");
        if length == 0 {
            length = 128 * 1024;
        }
        if send_buffer != 0 && send_buffer < length {
            log::warn!("requested send-buffer, {}, is too small to hold the data to be exchanged; it will be increased to {}", send_buffer, length * 2);
            send_buffer = length * 2;
        }
        
        let no_delay:bool = args.is_present("no_delay");
        
        Ok(prepare_configuration_tcp_upload(test_id, parallel_streams, bandwidth, seconds, length as usize, send_interval, send_buffer, no_delay))
    }
}
/// prepares a message used to describe the download role in a test
pub fn prepare_download_configuration(args:&clap::ArgMatches, test_id:&[u8; 16]) -> BoxResult<serde_json::Value> {
    let parallel_streams:u8 = args.value_of("parallel").unwrap().parse()?;
    let mut length:u32 = args.value_of("length").unwrap().parse()?;
    let mut receive_buffer:u32 = args.value_of("receive_buffer").unwrap().parse()?;
    
    if args.is_present("udp") {
        log::debug!("preparing UDP download config...");
        if length == 0 {
            length = 1024;
        }
        if receive_buffer != 0 && receive_buffer < length {
            log::warn!("requested receive-buffer, {}, is too small to hold the data to be exchanged; it will be increased to {}", receive_buffer, length * 2);
            receive_buffer = length * 2;
        }
        Ok(prepare_configuration_udp_download(test_id, parallel_streams, length as u16, receive_buffer))
    } else {
        log::debug!("preparing TCP download config...");
        if length == 0 {
            length = 128 * 1024;
        }
        if receive_buffer != 0 && receive_buffer < length {
            log::warn!("requested receive-buffer, {}, is too small to hold the data to be exchanged; it will be increased to {}", receive_buffer, length * 2);
            receive_buffer = length * 2;
        }
        Ok(prepare_configuration_tcp_download(test_id, parallel_streams, length as usize, receive_buffer))
    }
}
