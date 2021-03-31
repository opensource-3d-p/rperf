extern crate log;

use std::error::Error;
use std::time::{Duration};

use crate::protocol::results::{IntervalResult, UdpReceiveResult, UdpSendResult};

type BoxResult<T> = Result<T,Box<dyn Error>>;

pub const TEST_HEADER_SIZE:u16 = 36;

const POLL_TIMEOUT:Duration = Duration::from_millis(250);
const UPDATE_INTERVAL:Duration = Duration::from_secs(1);
const RECEIVE_TIMEOUT:Duration = Duration::from_secs(3);

#[derive(Clone)]
pub struct UdpTestDefinition {
    //a UUID used to identify packets associated with this test
    pub test_id: [u8; 16],
    //bandwidth target, in bytes/sec
    pub bandwidth: u64,
    //the length of the buffer to exchange
    pub length: u16,
}
pub fn build_udp_test_definition(details:&serde_json::Value) -> super::BoxResult<UdpTestDefinition> {
    let mut test_id_bytes = [0_u8; 16];
    for (i, v) in details.get("testId").unwrap_or(&serde_json::json!([])).as_array().unwrap().iter().enumerate() {
        if i >= 16 { //avoid out-of-bounds if given malicious data
            break;
        }
        test_id_bytes[i] = v.as_i64().unwrap_or(0) as u8;
    }
    
    let length = details.get("length").unwrap_or(&serde_json::json!(TEST_HEADER_SIZE)).as_i64().unwrap() as u16;
    if length < TEST_HEADER_SIZE {
        return Err(Box::new(simple_error::simple_error!(std::format!("{} is too short of a length to satisfy testing requirements", length))));
    }
    
    Ok(UdpTestDefinition{
        test_id: test_id_bytes,
        bandwidth: details.get("bandwidth").unwrap_or(&serde_json::json!(0.0)).as_f64().unwrap() as u64,
        length: length,
    })
}

pub mod receiver {
    use std::convert::TryInto;
    use std::time::{Instant, SystemTime, UNIX_EPOCH};
    
    use chrono::{NaiveDateTime};
    
    use mio::net::UdpSocket;
    use mio::{Events, Ready, Poll, PollOpt, Token};
    
    struct UdpReceiverHistory {
        packets_received: u64,
        
        next_packet_id: u64,
        
        packets_lost: i64,
        packets_out_of_order: u64,
        packets_duplicate: u64,
        
        unbroken_sequence: u64,
        jitter_seconds: Option<f32>,
        previous_time_delta_nanoseconds: i64,
    }
    
    pub struct UdpReceiver {
        active: bool,
        test_definition: super::UdpTestDefinition,
        stream_idx: u8,
        history: UdpReceiverHistory,
        
        socket: UdpSocket,
        mio_poll_token: Token,
        mio_poll: Poll,
        
        //the fixed number of bytes that frame each packet
        framing_size: u16,
    }
    impl UdpReceiver {
        pub fn new(test_definition:super::UdpTestDefinition, stream_idx:&u8, ip_version:&u8, port:&u16) -> super::BoxResult<UdpReceiver> {
            let socket:UdpSocket;
            let framing_size:u16;
            if *ip_version == 4 {
                framing_size = 28;
                socket = UdpSocket::bind(&format!("0.0.0.0:{}", port).parse::<std::net::SocketAddr>()?).expect(&format!("failed to bind UDP socket, port {}", port));
            } else if *ip_version == 6 {
                framing_size = 48;
                socket = UdpSocket::bind(&format!(":::{}", port).parse::<std::net::SocketAddr>()?).expect(&format!("failed to bind UDP socket, port {}", port));
            } else {
                return Err(Box::new(simple_error::simple_error!(format!("unsupported IP version: {}", ip_version))));
            }
            
            let mio_poll_token = Token(0);
            let mio_poll = Poll::new()?;
            mio_poll.register(
                &socket,
                mio_poll_token,
                Ready::readable(),
                PollOpt::edge(),
            )?;
            
            Ok(UdpReceiver{
                active: true,
                test_definition: test_definition,
                stream_idx: stream_idx.to_owned(),
                history: UdpReceiverHistory{
                    packets_received: 0,
                    
                    next_packet_id: 0,
                    
                    packets_lost: 0,
                    packets_out_of_order: 0,
                    packets_duplicate: 0,
                    
                    unbroken_sequence: 0,
                    jitter_seconds: None,
                    previous_time_delta_nanoseconds: 0,
                },
                
                socket: socket,
                mio_poll_token: mio_poll_token,
                mio_poll: mio_poll,
                
                framing_size: framing_size,
            })
        }
        
        fn process_packets_ordering(&mut self, packet_id:u64) -> bool {
            /* the algorithm from iperf3 provides a pretty decent approximation
             * for tracking lost and out-of-order packets efficiently, so it's
             * been minimally reimplemented here, with corrections.
             * 
             * returns true if packet-ordering is as-expected
             */
            if packet_id == self.history.next_packet_id { //expected sequential-ordering case
                self.history.next_packet_id += 1;
                return true;
            } else if packet_id > self.history.next_packet_id { //something was either lost or there's an ordering problem
                self.history.packets_lost += (packet_id - self.history.next_packet_id) as i64; //assume everything in between has been lost
                self.history.next_packet_id = packet_id + 1; //anticipate that ordered receipt will resume
            } else { //a packet with a previous ID was received; this is either a duplicate or an ordering issue
                //CAUTION: this is where the approximation part of the algorithm comes into play
                if self.history.packets_lost > 0 { //assume it's an ordering issue in the common case
                    self.history.packets_lost -= 1;
                    self.history.packets_out_of_order += 1;
                } else { //the only other thing it could be is a duplicate; in practice, duplicates don't tend to show up alongside losses; non-zero is always bad, though
                    self.history.packets_duplicate += 1;
                }
            }
            return false;
        }
        
        fn process_jitter(&mut self, timestamp:&NaiveDateTime) {
            //this is a pretty straightforward implementation of RFC 1889, Appendix 8
            //it works on an assumption that the timestamp delta between sender and receiver
            //will remain effectively constant during the testing window
            
            let now = SystemTime::now().duration_since(UNIX_EPOCH).expect("system time before UNIX epoch");
            let current_timestamp = NaiveDateTime::from_timestamp(now.as_secs() as i64, now.subsec_nanos());
            
            let time_delta = current_timestamp - *timestamp;
            
            let time_delta_nanoseconds = time_delta.num_nanoseconds();
            if time_delta_nanoseconds.is_none() {
                log::warn!("sender and receiver clocks are too out-of-sync to calculate jitter");
                return
            }
            let unwrapped_time_delta_nanoseconds = time_delta_nanoseconds.unwrap();
            
            if self.history.unbroken_sequence > 1 { //do jitter calculation
                let delta_seconds = ((unwrapped_time_delta_nanoseconds - self.history.previous_time_delta_nanoseconds).abs() / 1_000_000_000) as f32;
                
                if self.history.unbroken_sequence > 2 { //normal jitter-calculation, per the RFC
                    let mut jitter_seconds = self.history.jitter_seconds.unwrap();
                    jitter_seconds += (delta_seconds - jitter_seconds) / 16.0;
                    self.history.jitter_seconds = Some(jitter_seconds);
                } else { //first observed transition; use this as the calibration baseline
                    self.history.jitter_seconds = Some(delta_seconds);
                }
            }
            //update time-delta
            self.history.previous_time_delta_nanoseconds = unwrapped_time_delta_nanoseconds;
        }
        
        fn process_packet(&mut self, packet:&[u8]) -> bool {
            //the first sixteen bytes are the test's ID
            if packet[0..16] != self.test_definition.test_id {
                return false
            }
            
            //the next eight bytes are the packet's ID, in big-endian order
            let packet_id = u64::from_be_bytes(packet[16..24].try_into().unwrap());
            
            //except for the timestamp, nothing else in the packet actually matters
            
            self.history.packets_received += 1;
            if self.process_packets_ordering(packet_id) {
                //the second eight are the number of seconds since the UNIX epoch, in big-endian order
                let origin_seconds = i64::from_be_bytes(packet[24..32].try_into().unwrap());
                //and the following four are the number of nanoseconds since the UNIX epoch
                let origin_nanoseconds = u32::from_be_bytes(packet[32..36].try_into().unwrap());
                let source_timestamp = NaiveDateTime::from_timestamp(origin_seconds, origin_nanoseconds);
                
                self.history.unbroken_sequence += 1;
                self.process_jitter(&source_timestamp);
            } else {
                self.history.unbroken_sequence = 0;
                self.history.jitter_seconds = None;
            }
            return true;
        }
    }
    impl crate::stream::TestStream for UdpReceiver {
        fn run_interval(&mut self) -> Option<super::BoxResult<Box<dyn super::IntervalResult + Sync + Send>>> {
            let mut events = Events::with_capacity(1); //only watching one socket
            let mut buf = vec![0_u8; self.test_definition.length.into()];
            
            let mut bytes_received:u64 = 0;
            let initial_packets_received = self.history.packets_received;
            let initial_packets_lost = self.history.packets_lost;
            let initial_packets_out_of_order = self.history.packets_out_of_order;
            let initial_packets_duplicate = self.history.packets_duplicate;
            
            let start = Instant::now();
            
            while self.active {
                if start.elapsed() >= super::RECEIVE_TIMEOUT {
                    return Some(Err(Box::new(simple_error::simple_error!("UDP reception for stream {} timed out, likely because the end-signal was lost", self.stream_idx))));
                }
                
                let poll_result = self.mio_poll.poll(&mut events, Some(super::POLL_TIMEOUT));
                if poll_result.is_err() {
                    return Some(Err(Box::new(poll_result.unwrap_err())));
                }
                for event in events.iter() {
                    if event.token() == self.mio_poll_token {
                        loop {
                            match self.socket.recv(&mut buf) {
                                Ok(packet_size) => {
                                    if packet_size == 16 { //possible end-of-test message
                                        if &buf[0..16] == self.test_definition.test_id { //test's over
                                            self.stop();
                                            break;
                                        }
                                    }
                                    if packet_size < super::TEST_HEADER_SIZE as usize {
                                        log::error!("received malformed packet with size {}", packet_size);
                                        continue
                                    }
                                    
                                    if self.process_packet(&buf) {
                                        bytes_received += packet_size as u64 + self.framing_size as u64;
                                        
                                        let elapsed_time = start.elapsed();
                                        if elapsed_time >= super::UPDATE_INTERVAL {
                                            return Some(Ok(Box::new(super::UdpReceiveResult{
                                                stream_idx: self.stream_idx,
                                                
                                                duration: elapsed_time.as_secs_f32(),
                                                
                                                bytes_received: bytes_received,
                                                packets_received: self.history.packets_received - initial_packets_received,
                                                packets_lost: self.history.packets_lost - initial_packets_lost,
                                                packets_out_of_order: self.history.packets_out_of_order - initial_packets_out_of_order,
                                                packets_duplicate: self.history.packets_duplicate - initial_packets_duplicate,
                                                
                                                unbroken_sequence: self.history.unbroken_sequence,
                                                jitter_seconds: self.history.jitter_seconds,
                                            })))
                                        }
                                    } else {
                                        log::error!("received packet unrelated to the current test");
                                        continue
                                    }
                                },
                                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => { //receive timeout
                                    break;
                                },
                                Err(e) => {
                                    return Some(Err(Box::new(e)));
                                },
                            }
                        }
                    } else {
                        log::warn!("got event for unbound token: {:?}", event);
                    }
                }
            }
            if bytes_received > 0 {
                Some(Ok(Box::new(super::UdpReceiveResult{
                    stream_idx: self.stream_idx,
                    
                    duration: start.elapsed().as_secs_f32(),
                    
                    bytes_received: bytes_received,
                    packets_received: self.history.packets_received - initial_packets_received,
                    packets_lost: self.history.packets_lost - initial_packets_lost,
                    packets_out_of_order: self.history.packets_out_of_order - initial_packets_out_of_order,
                    packets_duplicate: self.history.packets_duplicate - initial_packets_duplicate,
                    
                    unbroken_sequence: self.history.unbroken_sequence,
                    jitter_seconds: self.history.jitter_seconds,
                })))
            } else {
                None
            }
        }
        
        fn get_port(&self) -> super::BoxResult<u16> {
            let sock_addr = self.socket.local_addr()?;
            Ok(sock_addr.port())
        }
        
        fn get_idx(&self) -> u8 {
            return self.stream_idx.to_owned();
        }
        
        fn stop(&mut self) {
            self.active = false;
        }
    }
}



pub mod sender {
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
    
    use mio::net::UdpSocket;
    
    use std::thread::{sleep};
    
    pub struct UdpSender {
        active: bool,
        test_definition: super::UdpTestDefinition,
        stream_idx: u8,
        
        socket: UdpSocket,
        
        //the interval, in seconds, at which to send data
        send_interval: f32,
        
        //the fixed number of bytes that frame each packet
        framing_size: u16,
        
        remaining_duration: f32,
        next_packet_id: u64,
        staged_packet: Vec<u8>,
    }
    impl UdpSender {
        pub fn new(test_definition:super::UdpTestDefinition, stream_idx:&u8, ip_version:&u8, port:&u16, receiver_host:String, receiver_port:&u16, send_duration:&f32, send_interval:&f32) -> super::BoxResult<UdpSender> {
            let socket:UdpSocket;
            let framing_size:u16;
            if *ip_version == 4 {
                framing_size = 28;
                socket = UdpSocket::bind(&format!("0.0.0.0:{}", port).parse::<std::net::SocketAddr>()?).expect("failed to bind socket");
            } else if *ip_version == 6 {
                framing_size = 48;
                socket = UdpSocket::bind(&format!(":::{}", port).parse::<std::net::SocketAddr>()?).expect("failed to bind socket");
            } else {
                return Err(Box::new(simple_error::simple_error!(format!("unsupported IP version: {}", ip_version))));
            }
            socket.connect(format!("{}:{}", receiver_host, receiver_port).parse()?)?;
            
            let mut staged_packet = vec![0_u8; test_definition.length.into()];
            for i in super::TEST_HEADER_SIZE..(staged_packet.len() as u16) { //fill the packet with a fixed sequence
                staged_packet[i as usize] = (i % 256) as u8;
            }
            //embed the test ID
            staged_packet[0..16].copy_from_slice(&test_definition.test_id);
            
            Ok(UdpSender{
                active: true,
                test_definition: test_definition,
                stream_idx: stream_idx.to_owned(),
                
                socket: socket,
                
                send_interval: send_interval.to_owned(),
                
                framing_size: framing_size,
                
                remaining_duration: send_duration.to_owned(),
                next_packet_id: 0,
                staged_packet: staged_packet,
            })
        }
        
        fn prepare_packet(&mut self) {
            let now = SystemTime::now().duration_since(UNIX_EPOCH).expect("system time before UNIX epoch");
            
            //eight bytes after the test ID are the packet's ID, in big-endian order
            self.staged_packet[16..24].copy_from_slice(&self.next_packet_id.to_be_bytes());
            
            //the next eight are the seconds part of the UNIX timestamp and the following four are the nanoseconds
            self.staged_packet[24..32].copy_from_slice(&now.as_secs().to_be_bytes());
            self.staged_packet[32..36].copy_from_slice(&now.subsec_nanos().to_be_bytes());
            
            //prepare for the next packet
            self.next_packet_id += 1;
        }
    }
    impl crate::stream::TestStream for UdpSender {
        fn run_interval(&mut self) -> Option<super::BoxResult<Box<dyn super::IntervalResult + Sync + Send>>> {
            let interval_duration = Duration::from_secs_f32(self.send_interval);
            let bytes_to_send = ((self.test_definition.bandwidth as f32) * super::UPDATE_INTERVAL.as_secs_f32()) as i64;
            let mut bytes_to_send_remaining = bytes_to_send;
            let bytes_to_send_per_interval_slice = ((bytes_to_send as f32) * self.send_interval) as i64;
            let mut bytes_to_send_per_interval_slice_remaining = bytes_to_send_per_interval_slice;
            
            let mut packets_sent:u64 = 0;
            let mut bytes_sent:u64 = 0;
            
            let cycle_start = Instant::now();
            
            while self.active && self.remaining_duration > 0.0 {
                let packet_start = Instant::now();
                
                self.prepare_packet();
                match self.socket.send(&self.staged_packet) {
                    Ok(packet_size) => {
                        packets_sent += 1;
                        
                        let bytes_written = (packet_size + (self.framing_size as usize)) as i64;
                        bytes_sent += bytes_written as u64;
                        bytes_to_send_remaining -= bytes_written;
                        bytes_to_send_per_interval_slice_remaining -= bytes_written;
                        
                        let elapsed_time = cycle_start.elapsed();
                        if elapsed_time >= super::UPDATE_INTERVAL {
                            self.remaining_duration -= packet_start.elapsed().as_secs_f32();
                            
                            return Some(Ok(Box::new(super::UdpSendResult{
                                stream_idx: self.stream_idx,
                                
                                duration: elapsed_time.as_secs_f32(),
                                
                                bytes_sent: bytes_sent,
                                packets_sent: packets_sent,
                            })))
                        }
                    },
                    Err(e) => {
                        return Some(Err(Box::new(e)));
                    },
                }
                
                if bytes_to_send_remaining <= 0 { //interval's target is exhausted
                    let elapsed_time = cycle_start.elapsed();
                    if super::UPDATE_INTERVAL > elapsed_time {
                        sleep(super::UPDATE_INTERVAL - elapsed_time);
                    }
                } else if bytes_to_send_per_interval_slice_remaining <= 0 { // interval subsection exhausted
                    bytes_to_send_per_interval_slice_remaining = bytes_to_send_per_interval_slice;
                    let elapsed_time = cycle_start.elapsed();
                    if interval_duration > elapsed_time {
                        sleep(interval_duration - elapsed_time);
                    }
                }
                self.remaining_duration -= packet_start.elapsed().as_secs_f32();
            }
            if bytes_sent > 0 {
                Some(Ok(Box::new(super::UdpSendResult{
                    stream_idx: self.stream_idx,
                    
                    duration: cycle_start.elapsed().as_secs_f32(),
                    
                    bytes_sent: bytes_sent,
                    packets_sent: packets_sent,
                })))
            } else {
                //indicate that the test is over by sending the test ID by itself
                for _ in 0..4 { //do it a few times in case of loss
                    let send_result = self.socket.send(&self.staged_packet[0..16]);
                    if send_result.is_err() {
                        return Some(Err(Box::new(send_result.unwrap_err())));
                    }
                }
                None
            }
        }
        
        fn get_port(&self) -> super::BoxResult<u16> {
            let sock_addr = self.socket.local_addr()?;
            Ok(sock_addr.port())
        }
        
        fn get_idx(&self) -> u8 {
            return self.stream_idx.to_owned();
        }
        
        fn stop(&mut self) {
            self.active = false;
        }
    }
}
