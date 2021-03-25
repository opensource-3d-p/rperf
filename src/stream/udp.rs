extern crate log;

use std::error::Error;
use std::fmt::{format};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::thread::{sleep};

use chrono::{NaiveDateTime};

use mio::net::UdpSocket;
use mio::{Events, Ready, Poll, PollOpt, Token};

use protocol::results::{UdpReceiveResult, UdpSendResult};

type BoxResult<T> = Result<T,Box<dyn Error>>;

//250ms timeout
const POLL_TIMEOUT = Duration::new(0, 250_000_000);
const UPDATE_INTERVAL = Duration::new(1, 0);

pub struct UdpTestDefinition {
    //throughput target, in bytes/sec
    pub throughput: u64,
    //the total number of bytes to be transferred
    pub bytes: u64,
    //the length of the buffer to exchange
    pub length: u16,
    //the interval, in seconds, at which to send data
    pub send_interval: f32,
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
        
        pub fn get_port(&self) -> BoxResult<u16> {
            let sock_addr = self.socket.local_addr()?;
            Ok(sock_addr.port())
        }
        
        pub fn stop(&mut self) {
            self.active = false;
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
        
        fn process_packet(&mut self, packet:&[u8]) {
            //the first eight bytes are the packet's ID, in big-endian order
            let packet_id = u64::from_be_bytes(&packet[0..8]);
            
            //except for the timestamp, nothing else in the packet actually matters
            
            self.history.packets_received += 1;
            if self.process_packets_ordering(&packet_id) {
                //the second eight are the number of seconds since the UNIX epoch, in big-endian order
                let origin_seconds = i64::from_be_bytes(&packet[8..16]);
                //and the following four are the number of nanoseconds since the UNIX epoch
                let origin_nanoseconds = u32::from_be_bytes(&packet[16..20]);
                let source_timestamp = NaiveDateTime::from_timestamp(origin_seconds, origin_nanoseconds);
                
                self.history.unbroken_sequence += 1;
                self.process_jitter(source_timestamp);
            } else {
                self.history.unbroken_sequence = 0;
                self.history.jitter_seconds = None;
                debug!("packets rcv:{} lst:{} ooo:{} dup:{}", self.history.packets_received, self.history.lost_packets, self.history.out_of_order_packets, self.history.duplicate_packets);
            }
        }
    }
    impl Iterator for UdpReceiver {
        fn next(&self) -> Option<BoxResult<UdpReceiveResult>> {
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
                                    if packet.length() < 20 { //rperf's protocol data is 20 bytes in size
                                        error!("received malformed packet with size {}", packet_size);
                                        continue
                                    }
                                    
                                    bytes_received += packet_size + self.framing_size;
                                    
                                    self.process_packet(&buf);
                                    
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
    }
}



mod sender {
    pub struct UdpSender {
        active: bool,
        test_definition: UdpTestDefinition,
        
        mut socket: UdpSocket,
        
        //the fixed number of bytes that frame each packet
        framing_size: u64,
        
        mut remaining_bytes u64,
        mut next_packet_id: u64,
        mut staged_packet: [u8],
    }
    pub impl UdpSender {
        pub fn new(test_definition:UdpTestDefinition, ip_version:&u8, port:&u16, receiver_host:String, receiver_port:&u16) -> BoxResult<UdpSender> {
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
            for i in 0..(staged_packet.len()) { //fill the packet with a fixed sequence
                staged_packet[i] = i %256;
            }
            
            Ok(UdpSender{
                active: true,
                test_definition: test_definition,
                
                socket: socket,
                
                framing_size: framing_size,
                
                remaining_bytes: test_definition.bytes,
                next_packet_id: 0,
                staged_packet: staged_packet,
            })
        }
        
        pub fn get_port(&self) -> BoxResult<u16> {
            let sock_addr = self.socket.local_addr()?;
            Ok(sock_addr.port())
        }
        
        pub fn stop(&mut self) {
            self.active = false;
        }
        
        fn prepare_packet(&mut self) {
            let now = SystemTime::now().duration_since(UNIX_EPOCH).expect("system time before UNIX epoch");
            
            //the first eight bytes are the packet's ID, in big-endian order
            self.staged_packet[..8].copy_from_slice(self.next_packet_id.to_be_bytes());
            
            //the next eight are the seconds part of the UNIX timestamp and the following four are the nanoseconds
            self.staged_packet[8..16].copy_from_slice(now.as_secs().to_be_bytes());
            self.staged_packet[16..20].copy_from_slice(now.subsec_nanos().to_be_bytes());
            
            //prepare for the next packet
            self.next_packet_id += 1;
        }
    }
    impl Iterator for UdpSender {
        fn next(&self) -> Option<BoxResult<UdpSentResult>> {
            let interval_duration = Duration::from_secs_f32(self.test_definition.send_interval);
            let bytes_per_interval = ((self.test_definition.throughput as f64) * self.test_definition.send_interval) as u64;
            let mut bytes_per_interval_remaining = bytes_per_interval;
            
            let mut packets_sent:u64 = 0;
            let mut bytes_sent:u64 = 0;
            
            let start = Instant::now();
            
            while self.active {
                self.prepare_packet();
                match self.socket.send(self.staged_packet) {
                    Ok(packet_size) => {
                        let bytes_written = packet_size + self.framing_size
                        self.remaining_bytes -= bytes_written;
                        bytes_sent += bytes_written;
                        bytes_per_interval_remaining -= bytes_written;
                        
                        let elapsed_time = start.elapsed();
                        if elapsed_time >= UPDATE_INTERVAL {
                            return Some(Ok(UdpSendResult{
                                duration: elapsed_time.as_secs_f64(),
                                
                                bytes_sent: bytes_sent,
                                packets_sent: packets_sent,
                            }))
                        }
                    },
                    Err(e) => {
                        return Some(Err(e));
                    },
                }
                
                if self.remaining_bytes < self.test_definition.length + self.framing_size {
                    break; //sending any more would exceed the requested total
                }
                
                if bytes_per_interval_remaining <= 0 {
                    bytes_per_interval_remaining = bytes_per_interval;
                    sleep(interval_duration);
                }
            }
            if bytes_sent > 0 {
                Some(Ok(UdpReceiveResult{
                    duration: start.elapsed().as_secs_f64(),
                    
                    bytes_sent: bytes_sent,
                    packets_sent: packets_sent,
                }))
            } else {
                None
            }
        }
    }
}
