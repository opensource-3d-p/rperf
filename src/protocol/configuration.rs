extern crate log;

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

pub fn calculate_duration(bandwidth:u64, bytes:u64, seconds:f32) -> f32 {
    if bytes > 0 { //bytes were requested
        if bandwidth > 0 {
            ((bytes as f64) / (bandwidth as f64)) as f32
        } else {
            log::warn!("bandwidth was not specified; setting duration to 0s");
            0
        }
    } else if seconds > 0.0 { //seconds were specified; just use that
        seconds
    } else {
        log::warn!("neither seconds nor bytes were specified; setting duration to 0s");
        0.0
    }
}

pub fn calculate_length_udp(length:u32) -> u32 {
    if length < crate::stream::udp::TEST_HEADER_SIZE { //length must be at least enough to hold the test data
        crate::stream::udp::TEST_HEADER_SIZE
    } else {
        length
    }
}
