#[macro_use] extern crate log;

use mio::net::{TcpStream};
use mio::{Events, Poll, PollOpt, Ready, Token};

//2s keepalive
pub const KEEPALIVE_DURATION = Duration::new(2, 0); //communication over the protocol link will normally happen once per second

//50ms timeout
const POLL_TIMEOUT = Duration::new(0, 50_000_000);


pub fn communication_get_length(&mut stream:TcpStream) -> Result<u16> {
    let mio_token = Token(0);
    let poll = Poll::new()?;
    poll.register(
        &stream,
        mio_token,
        Ready::readable(),
        PollOpt::edge(),
    )?;
    let mut events = Events::with_capacity(1); //only interacting with one stream
    
    let mut length_bytes_read = 0;
    let mut length_spec = [u8; 2];
    while alive { //waiting to find out how long the next message is
        poll.poll(&mut events, POLL_TIMEOUT)?;
        for event in events {
            match event.token() {
                mio_token => loop {
                    match stream.read(&mut length_spec[length_bytes_read..]) {
                        Ok(size) => {
                            length_bytes_read += size;
                            if length == 2 {
                                return Ok(u16::from_be_bytes(length_spec));
                            }
                        },
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => { //nothing left to process
                            break;
                        },
                        Err(e) => {
                            return Err(e);
                        },
                    }
                },
            }
        }
    }
    Err("system shutting down")
}

pub fn communication_get_payload(&mut stream:TcpStream, length:u16) -> Result<serde_json::Value> {
    let mio_token = Token(0);
    let poll = Poll::new()?;
    poll.register(
        &stream,
        mio_token,
        Ready::readable(),
        PollOpt::edge(),
    )?;
    let mut events = Events::with_capacity(1); //only interacting with one stream
    
    let mut bytes_read = 0;
    let mut buffer = [u8; length];
    while alive { //waiting to receive the payload
        poll.poll(&mut events, POLL_TIMEOUT)?;
        for event in events {
            match event.token() {
                mio_token => loop {
                    match stream.read(&mut buffer[bytes_read..]) {
                        Ok(size) => {
                            bytes_read += size;
                            if bytes_read == length {
                                return serde_json::from_slice(&buffer);
                            }
                        },
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => { //nothing left to process
                            break;
                        },
                        Err(e) => {
                            return Err(e);
                        },
                    }
                },
            }
        }
    }
    Err("system shutting down")
}
