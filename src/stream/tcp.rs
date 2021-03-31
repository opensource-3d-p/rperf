extern crate log;
extern crate nix;

use std::error::Error;
use std::time::{Duration};

use crate::protocol::results::{IntervalResult, TcpReceiveResult, TcpSendResult};
use nix::sys::socket::{setsockopt, sockopt::RcvBuf, sockopt::SndBuf};

type BoxResult<T> = Result<T,Box<dyn Error>>;

pub const TEST_HEADER_SIZE:u16 = 16;

const POLL_TIMEOUT:Duration = Duration::from_millis(250);
const UPDATE_INTERVAL:Duration = Duration::from_secs(1);
const RECEIVE_TIMEOUT:Duration = Duration::from_secs(3);

#[derive(Clone)]
pub struct TcpTestDefinition {
    //a UUID used to identify packets associated with this test
    pub test_id: [u8; 16],
    //bandwidth target, in bytes/sec
    pub bandwidth: u64,
    //the length of the buffer to exchange
    pub length: u32,
}
impl TcpTestDefinition {
    pub fn new(details:&serde_json::Value) -> super::BoxResult<TcpTestDefinition> {
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
        
        Ok(TcpTestDefinition{
            test_id: test_id_bytes,
            bandwidth: details.get("bandwidth").unwrap_or(&serde_json::json!(0.0)).as_f64().unwrap() as u64,
            length: length,
        })
    }
}


pub mod receiver {
    use std::convert::TryInto;
    use std::time::{Instant, SystemTime, UNIX_EPOCH};
    
    use mio::net::{TcpListener, TcpStream};
    use mio::{Events, Ready, Poll, PollOpt, Token};
    
    pub struct TcpReceiver {
        active: bool,
        test_definition: super::TcpTestDefinition,
        stream_idx: u8,,
        
        listener: Option<TcpListener>,
        stream: Option<TcpStream>,
        mio_poll_token: Token,
        mio_poll: Poll,
        
        receive_buffer: u32,
    }
    impl TcpReceiver {
        pub fn new(test_definition:super::TcpTestDefinition, stream_idx:&u8, ip_version:&u8, port:&u16, receive_buffer:&u32) -> super::BoxResult<TcpReceiver> {
            let mut listener:TcpListener;
            if *ip_version == 4 {
                listener = TcpListener::bind(&format!("0.0.0.0:{}", port).parse()?).expect(format!("failed to bind TCP socket, port {}", port).as_str());
            } else if *ip_version == 6 {
                listener = TcpListener::bind(&format!(":::{}", port).parse()?).expect(format!("failed to bind TCP socket, port {}", port).as_str());
            } else {
                return Err(Box::new(simple_error::simple_error!("unsupported IP version: {}", ip_version)));
            }
            
            let mio_poll_token = Token(0);
            let mio_poll = Poll::new()?;
            
            Ok(TcpReceiver{
                active: true,
                test_definition: test_definition,
                stream_idx: stream_idx.to_owned(),
                
                listener: Some(listener),
                stream: None,
                mio_poll_token: mio_poll_token,
                mio_poll: mio_poll,
                
                receive_buffer: receive_buffer.to_owned(),
            })
        }
        
        fn process_connection(&mut self) -> super::BoxResult<TcpStream> {
            let mio_token = Token(0);
            let poll = Poll::new()?;
            poll.register(
                &mut listener,
                mio_token,
                Ready::readable(),
                PollOpt::edge(),
            )?;
            let mut events = Events::with_capacity(1);
            
            while self.active() {
                poll.poll(&mut events, Some(super::POLL_TIMEOUT))?;
                for event in events.iter() {
                    match event.token() {
                        _ => loop {
                            match listener.accept() {
                                Ok((mut stream, address)) => {
                                    log::info!("connection from {}", address);
                                    
                                    let mut verification_stream = stream.try_clone();
                                    let mio_token2 = Token(0);
                                    let poll2 = Poll::new()?;
                                    poll2.register(
                                        &verification_stream,
                                        mio_token2,
                                        Ready::readable(),
                                        PollOpt::edge(),
                                    )?;
                                    
                                    let mut buffer = [0_u8; 16];
                                    poll2.poll(&mut events, Some(super::RECEIVE_TIMEOUT))?;
                                    for event in events.iter() {
                                        match event.token() {
                                            _ => match verification_stream.read(&mut buffer) {
                                                Ok(size) => {
                                                    if buffer == self.test_definition.test_id {
                                                        if !cfg!(windows) { //NOTE: features unsupported on Windows
                                                            if self.receive_buffer != 0 {
                                                                log::debug("setting receive-buffer to {}...", self.receive_buffer);
                                                                super::setsockopt(stream.as_raw_fd(), super::RcvBuf, self.receive_buffer)?;
                                                            }
                                                        }
                                                        
                                                        self.mio_poll.register(
                                                            &stream,
                                                            self.mio_poll_token,
                                                            Ready::readable(),
                                                            PollOpt::edge(),
                                                        )?;
                                                        return stream;
                                                    }
                                                },
                                                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => { //client didn't provide anything
                                                    break;
                                                },
                                                Err(e) => {
                                                    return Err(Box::new(e));
                                                },
                                            }
                                        }
                                    }
                                    log::warn!("could not verify that {} was part of the test-group", address);
                                },
                                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => { //nothing to do
                                    break;
                                },
                                Err(e) => {
                                    return Err(Box::new(e));
                                },
                            }
                        },
                    }
                }
            }
            Err(simple_error::simple_error!("did not receive a connection"))
        }
    }
    impl crate::stream::TestStream for TcpReceiver {
        fn run_interval(&mut self) -> Option<super::BoxResult<Box<dyn super::IntervalResult + Sync + Send>>> {
            if self.stream.is_none() { //if still in the setup phase, receive the sender
                self.stream = Some(self.process_connection()?);
                self.listener = None; //drop it, closing the socket
            }
            let stream = self.stream.unwrap();
            
            let mut events = Events::with_capacity(1); //only watching one socket
            let mut buf = vec![0_u8; self.test_definition.length.into()];
            
            let mut bytes_received:u64 = 0;
            
            let start = Instant::now();
            
            while self.active {
                if start.elapsed() >= super::RECEIVE_TIMEOUT {
                    return Some(Err(Box::new(simple_error::simple_error!("TCP reception for stream {} timed out", self.stream_idx))));
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
                                    if packet_size == 0 { //test's over
                                        self.stop();
                                        break;
                                    }
                                    
                                    bytes_received += packet_size as u64;
                                    
                                    let elapsed_time = start.elapsed();
                                    if elapsed_time >= super::UPDATE_INTERVAL {
                                        return Some(Ok(Box::new(super::TcpReceiveResult{
                                            stream_idx: self.stream_idx,
                                            
                                            duration: elapsed_time.as_secs_f32(),
                                            
                                            bytes_received: bytes_received,
                                        })))
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
                Some(Ok(Box::new(super::TcpReceiveResult{
                    stream_idx: self.stream_idx,
                    
                    duration: start.elapsed().as_secs_f32(),
                    
                    bytes_received: bytes_received,
                })))
            } else {
                None
            }
        }
        
        fn get_port(&self) -> super::BoxResult<u16> {
            match self.listener {
                Some(listener) => Ok(listener.local_addr()?.port()),
                None => match self.stream {
                    Some(stream) => Ok(stream.local_addr()?.port()),
                    None => Err(simple_error::simple_error!("no port currently bound")),
                }
            }
        }
        
        fn get_idx(&self) -> u8 {
            self.stream_idx.to_owned()
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
            self.stream_idx.to_owned()
        }
        
        fn stop(&mut self) {
            self.active = false;
        }
    }
}



























//if the server is uploading, each of its iterators will be capped with a "done" signal, which sets a flag in the local iteration results
//if we're uploading, send a "done" to the server under the same conditions, which it will match with its own "done", which we use to update our local state
//for UDP, this is a packet containing only the test ID, 16 bytes in length
//for TCP, it's just closing the stream





/*

        
        .arg(
            Arg::with_name("window")
                .help("window-size, in bytes, for TCP tests  (only supported on some platforms)")
                .takes_value(false)
                .long("window")
                .short("w")
                .required(false)
        )
        .arg(
            Arg::with_name("mss")
                .help("maximum segment-size, for TCP tests (default is 1024 bytes)  (only supported on some platforms)")
                .takes_value(false)
                .long("mss")
                .short("M")
                .required(false)
        )
        .arg(
            Arg::with_name("nodelay")
                .help("use no-delay mode for TCP tests, deisabling Nagle's Algorithm")
                .takes_value(false)
                .long("no-delay")
                .short("N")
                .required(false)
        )
        .arg(
            Arg::with_name("congestion")
                .help("specify a congestion-control algorithm to use for TCP tests (only supported on some platforms)")
                .takes_value(true)
                .long("congestion")
                .default_value("")
                .required(false)
        )
        
        
*/
