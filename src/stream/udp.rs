extern crate log;

use std::error::Error;
use std::fmt::{format};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::thread::{sleep};

use chrono::{NaiveDateTime};

use mio::net::UdpSocket;
use mio::{Events, Ready, Poll, PollOpt, Token};

use protocol::results::{IntervalResult, UdpReceiveResult, UdpSendResult};

type BoxResult<T> = Result<T,Box<dyn Error>>;

pub const TEST_HEADER_SIZE:u16 = 36;

//250ms timeout
const POLL_TIMEOUT = Duration::new(0, 250_000_000);
const UPDATE_INTERVAL = Duration::new(1, 0);

pub struct UdpTestDefinition {
    //a UUID used to identify packets associated with this test
    pub test_id: [u8; 16],
    //bandwidth target, in bytes/sec
    pub bandwidth: u64,
    //the length of the buffer to exchange
    pub length: u16,
}
pub fn build_udp_test_definition(details:&serde_json::Value) -> BoxResult<udp::UdpTestDefinition> {
    let mut test_id_bytes = [u8; 16];
    for (i, v) in details.get("testId")?.as_array()?.iter().enumerate() {
        test_id_bytes[i] = v.as_i64()? as u8;
    }
    
    udp::UdpTestDefinition{
        test_id: test_id_bytes,
        bandwidth: details.get("bandwidth")?.as_f64()? as u64,
        length: details.get("length")?.as_i64()? as u16,
    }
}

mod receiver {
    struct UdpReceiverHistory {
        mut packets_received: u64,
        
        mut next_packet_id: u64,
        
        mut lost_packets: i64,
        mut out_of_order_packets: u64,
        mut duplicate_packets: u64,
        
        mut unbroken_sequence: u64,
        mut jitter_seconds: Option<f32>,
        mut previous_time_delta_nanoseconds: i64,
    }
    
    pub struct UdpReceiver {
        active: bool,
        test_definition: UdpTestDefinition,
        mut history: UdpReceiverHistory,
        
        mut socket: UdpSocket,
        mio_poll_token: Token,
        mut mio_poll: Poll,
        
        //the fixed number of bytes that frame each packet
        framing_size: u64,
    }
    pub impl UdpReceiver {
        pub fn new(test_definition:UdpTestDefinition, ip_version:&u8, port:&u16) -> BoxResult<UdpReceiver> {
            let socket:UdpSocket;
            let framing_size:u64;
            if *ip_version == 4 {
                framing_size = 28;
                socket = UdpSocket::bind(format!("0.0.0.0:{}", port).parse()?).expect(format!("failed to bind UDP socket, port {}", port)));
            } else if *ip_version == 6 {
                framing_size = 48;
                socket = UdpSocket::bind(format!(":::{}", port).parse()?).expect(format!("failed to bind UDP socket, port {}", port)));
            } else {
                return Err(format!("unsupported IP version: {}", ip_version));
            }
            
            let mio_poll_token = Token(0);
            let mut mio_poll = Poll::new()?;
            mio_poll.registry().register(
                &socket,
                mio_poll_token,
                Ready::readable(),
                PollOpt::edge(),
            )?;
            
            Ok(UdpReceiver{
                active: true,
                test_definition: test_definition,
                history: UdpReceiverHistory{
                    packets_received: 0,
                    
                    next_packet_id: 0,
                    
                    lost_packets: 0,
                    out_of_order_packets: 0,
                    duplicate_packets: 0,
                    
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
        
        fn process_packets_ordering(&mut self, packet_id:&u64) -> bool {
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
                self.history.lost_packets += packet_id - self.history.next_packet_id; //assume everything in between has been lost
                self.history.next_packet_id = packet_id + 1; //anticipate that ordered receipt will resume
            } else { //a packet with a previous ID was received; this is either a duplicate or an ordering issue
                //CAUTION: this is where the approximation part of the algorithm comes into play
                if self.history.lost_packets > 0 { //assume it's an ordering issue in the common case
                    self.history.lost_packets -= 1;
                    self.history.out_of_order_packets += 1;
                } else { //the only other thing it could be is a duplicate; in practice, duplicates don't tend to show up alongside losses; non-zero is always bad, though
                    self.history.duplicate_packets += 1;
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
            
            let time_delta = current_timestamp - timestamp;
            
            let time_delta_nanoseconds = time_delta.num_nanoseconds();
            if time_delta_nanoseconds.is_none() {
                warn!("sender and receiver clocks are too out-of-sync to calculate jitter");
                return
            }
            let unwrapped_time_delta_nanoseconds = time_delta_nanoseconds.unwrap();
            
            if self.history.unbroken_sequence > 1 { //do jitter calculation
                let delta_seconds = (((unwrapped_time_delta_nanoseconds - self.history.previous_time_delta_nanoseconds).abs() / 1_000_000_000) as f32);
                
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
            let packet_id = u64::from_be_bytes(&packet[16..24]);
            
            //except for the timestamp, nothing else in the packet actually matters
            
            self.history.packets_received += 1;
            if self.process_packets_ordering(&packet_id) {
                //the second eight are the number of seconds since the UNIX epoch, in big-endian order
                let origin_seconds = i64::from_be_bytes(&packet[24..32]);
                //and the following four are the number of nanoseconds since the UNIX epoch
                let origin_nanoseconds = u32::from_be_bytes(&packet[32..36]);
                let source_timestamp = NaiveDateTime::from_timestamp(origin_seconds, origin_nanoseconds);
                
                self.history.unbroken_sequence += 1;
                self.process_jitter(source_timestamp);
            } else {
                self.history.unbroken_sequence = 0;
                self.history.jitter_seconds = None;
                debug!("packets rcv:{} lst:{} ooo:{} dup:{}", self.history.packets_received, self.history.lost_packets, self.history.out_of_order_packets, self.history.duplicate_packets);
            }
            return true;
        }
    }
    impl super::TestStream for UdpReceiver {
        fn run_interval(&self) -> Option<BoxResult<IntervalResult>> {
            let mut events = Events::with_capacity(1); //only watching one socket
            let mut buf = [u8; self.test_definition.length];
            
            let mut bytes_received:u64 = 0
            let initial_packets_received = self.history.packets_received;
            let initial_lost_packets = self.history.lost_packets;
            let initial_out_of_order_packets = self.history.out_of_order_packets;
            let initial_duplicate_packets = self.history.duplicate_packets;
            
            let start = Instant::now();
            
            while self.active {
                self.poll.poll(&mut events, Some(POLL_TIMEOUT))?;
                for event in events.iter() {
                    match event.token() {
                        self.mio_poll_token => loop {
                            match self.socket.recv(&mut buf) {
                                Ok(packet_size) => {
                                    if packet.length() == 16 { //possible end-of-test message
                                        if packet[0..16] == self.test_definition.test_id { //test's over
                                            self.stop();
                                            break;
                                        }
                                    }
                                    if packet.length() < TEST_HEADER_SIZE {
                                        log::error!("received malformed packet with size {}", packet_size);
                                        continue
                                    }
                                    
                                    if self.process_packet(&buf) {
                                        bytes_received += packet_size + self.framing_size;
                                        
                                        let elapsed_time = start.elapsed();
                                        if elapsed_time >= UPDATE_INTERVAL {
                                            return Some(Ok(UdpReceiveResult{
                                                duration: elapsed_time.as_secs_f64(),
                                                
                                                bytes_received: bytes_received,
                                                packets_received: self.history.packets_received - initial_packets_received,
                                                lost_packets: self.history.lost_packets - initial_lost_packets,
                                                out_of_order_packets: self.history.out_of_order_packets - initial_out_of_order_packets,
                                                duplicate_packets: self.history.duplicate_packets - initial_duplicate_packets,
                                                
                                                unbroken_sequence: self.history.unbroken_sequence,
                                                jitter_seconds: self.history.jitter_seconds,
                                            }))
                                        }
                                    } else {
                                        log::error!("received packet unrelated to the current test");
                                        continue
                                    }
                                },
                                Err(e) if e.kind() == io::ErrorKind::WouldBlock => { //receive timeout
                                    break;
                                },
                                Err(e) => {
                                    return Some(Err(e));
                                },
                            }
                        },
                        _ => {
                            warn!("got event for unbound token: {:?}", event);
                        }
                    }
                }
            }
            if bytes_received > 0 {
                Some(Ok(UdpReceiveResult{
                    duration: start.elapsed().as_secs_f64(),
                    
                    bytes_received: bytes_received,
                    packets_received: self.history.packets_received - initial_packets_received,
                    lost_packets: self.history.lost_packets - initial_lost_packets,
                    out_of_order_packets: self.history.out_of_order_packets - initial_out_of_order_packets,
                    duplicate_packets: self.history.duplicate_packets - initial_duplicate_packets,
                    
                    unbroken_sequence: self.history.unbroken_sequence,
                    jitter_seconds: self.history.jitter_seconds,
                }))
            } else {
                None
            }
        }
        
        fn get_port(&self) -> BoxResult<u16> {
            let sock_addr = self.socket.local_addr()?;
            Ok(sock_addr.port())
        }
        
        fn stop(&mut self) {
            self.active = false;
        }
    }
}



mod sender {
    pub struct UdpSender {
        active: bool,
        test_definition: UdpTestDefinition,
        
        mut socket: UdpSocket,
        
        //the interval, in seconds, at which to send data
        send_interval: f32,
        
        //the fixed number of bytes that frame each packet
        framing_size: u64,
        
        mut remaining_duration: f32,
        mut next_packet_id: u64,
        mut staged_packet: [u8],
    }
    pub impl UdpSender {
        pub fn new(test_definition:UdpTestDefinition, ip_version:&u8, port:&u16, receiver_host:String, receiver_port:&u16, send_duration:&f32, send_interval:&f32) -> BoxResult<UdpSender> {
            let socket:UdpSocket;
            let framing_size:u64;
            if ip_version == 4 {
                framing_size = 28;
                socket = UdpSocket::bind(format!("0.0.0.0:{}", port).parse()?).expect("failed to bind socket");
            } else if ip_version == 6 {
                framing_size = 48;
                socket = UdpSocket::bind(format!(":::{}", port).parse()?).expect("failed to bind socket");
            } else {
                return Err(format!("unsupported IP version: {}", ip_version));
            }
            socket.connect(format!("{}:{}", receiver_host, receiver_port))?;
            
            let mut staged_packet = [u8; test_definition.length];
            for i in TEST_HEADER_SIZE..(staged_packet.len()) { //fill the packet with a fixed sequence
                staged_packet[i] = i %256;
            }
            //embed the test ID
            output_buffer[0..16].copy_from_slice(test_definition.test_id);
            
            Ok(UdpSender{
                active: true,
                test_definition: test_definition,
                
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
            self.staged_packet[16..24].copy_from_slice(self.next_packet_id.to_be_bytes());
            
            //the next eight are the seconds part of the UNIX timestamp and the following four are the nanoseconds
            self.staged_packet[24..32].copy_from_slice(now.as_secs().to_be_bytes());
            self.staged_packet[32..36].copy_from_slice(now.subsec_nanos().to_be_bytes());
            
            //prepare for the next packet
            self.next_packet_id += 1;
        }
    }
    impl super::TestStream for UdpSender {
        fn run_interval(&self) -> Option<BoxResult<IntervalResult>> {
            let interval_duration = Duration::from_secs_f32(self.send_interval);
            let bytes_per_interval = ((self.test_definition.bandwidth as f64) * self.send_interval) as u64;
            let mut bytes_per_interval_remaining = bytes_per_interval;
            
            let mut packets_sent:u64 = 0;
            let mut bytes_sent:u64 = 0;
            
            let cycle_start = Instant::now();
            
            while self.active && self.remaining_duration > 0.0 {
                let packet_start = Instant::now();
                
                self.prepare_packet();
                match self.socket.send(self.staged_packet) {
                    Ok(packet_size) => {
                        let bytes_written = packet_size + self.framing_size
                        bytes_sent += bytes_written;
                        bytes_per_interval_remaining -= bytes_written;
                        
                        let elapsed_time = cycle_start.elapsed();
                        if elapsed_time >= UPDATE_INTERVAL {
                            self.remaining_duration -= packet_start.elapsed().as_secs_f32();
                            
                            return Some(Ok(UdpSendResult{
                                duration: elapsed_time.as_secs_f32(),
                                
                                bytes_sent: bytes_sent,
                                packets_sent: packets_sent,
                            }))
                        }
                    },
                    Err(e) => {
                        return Some(Err(e));
                    },
                }
                
                if bytes_per_interval_remaining <= 0 {
                    bytes_per_interval_remaining = bytes_per_interval;
                    let sleep_duration = interval_duration - cycle_start.elapsed();
                    if sleep_duration.as_nanos() > 0 {
                        sleep(sleep_duration);
                    }
                }
                self.remaining_duration -= packet_start.elapsed().as_secs_f32();
            }
            if bytes_sent > 0 {
                Some(Ok(UdpSendResult{
                    duration: cycle_start.elapsed().as_secs_f64(),
                    
                    bytes_sent: bytes_sent,
                    packets_sent: packets_sent,
                }))
            } else {
                //indicate that the test is over by sending the test ID by itself
                for i in 0..4 { //do it a few times in case of loss
                    self.socket.send(self.staged_packet[0..16]);
                }
                
                None
            }
        }
        
        fn get_port(&self) -> BoxResult<u16> {
            let sock_addr = self.socket.local_addr()?;
            Ok(sock_addr.port())
        }
        
        fn stop(&mut self) {
            self.active = false;
        }
    }
}
