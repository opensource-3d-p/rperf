extern crate log;

use std::error::Error;
use std::io;
use std::io::{Read, Write};
use std::time::Duration;

use mio::net::{TcpStream};
use mio::{Events, Ready, Poll, PollOpt, Token};

type BoxResult<T> = Result<T,Box<dyn Error>>;

pub const KEEPALIVE_DURATION:Duration = Duration::from_secs(2); //communication over the protocol link will normally happen once per second

const POLL_TIMEOUT:Duration = Duration::from_millis(50);

pub fn send(stream:&mut TcpStream, message:&serde_json::Value) -> BoxResult<()> {
    let serialised_message = serde_json::to_vec(message)?;
    
    let mut output_buffer = Vec::with_capacity(serialised_message.len() + 2);
    output_buffer[..2].copy_from_slice(&serialised_message.len().to_be_bytes());
    output_buffer[2..].copy_from_slice(serialised_message.as_slice());
    
    Ok(stream.write_all(&output_buffer)?)
}

fn receive_length(stream:&mut TcpStream, alive_check:fn() -> bool) -> BoxResult<u16> {
    let mio_token = Token(0);
    let poll = Poll::new()?;
    poll.register(
        stream,
        mio_token,
        Ready::readable(),
        PollOpt::edge(),
    )?;
    let mut events = Events::with_capacity(1); //only interacting with one stream
    
    let mut length_bytes_read = 0;
    let mut length_spec:[u8; 2] = [0; 2];
    while alive_check() { //waiting to find out how long the next message is
        poll.poll(&mut events, Some(POLL_TIMEOUT))?;
        for event in events.iter() {
            match event.token() {
                _ => loop {
                    match stream.read(&mut length_spec[length_bytes_read..]) {
                        Ok(size) => {
                            length_bytes_read += size;
                            if length_bytes_read == 2 {
                                return Ok(u16::from_be_bytes(length_spec));
                            }
                        },
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => { //nothing left to process
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
    Err(Box::new(simple_error::simple_error!("system shutting down")))
}
fn receive_payload(stream:&mut TcpStream, alive_check:fn() -> bool, length:u16) -> BoxResult<serde_json::Value> {
    let mio_token = Token(0);
    let poll = Poll::new()?;
    poll.register(
        stream,
        mio_token,
        Ready::readable(),
        PollOpt::edge(),
    )?;
    let mut events = Events::with_capacity(1); //only interacting with one stream
    
    let mut bytes_read = 0;
    let mut buffer = Vec::with_capacity(length.into());
    while alive_check() { //waiting to receive the payload
        poll.poll(&mut events, Some(POLL_TIMEOUT))?;
        for event in events.iter() {
            match event.token() {
                _ => loop {
                    match stream.read(&mut buffer[bytes_read..]) {
                        Ok(size) => {
                            bytes_read += size;
                            if bytes_read == length as usize {
                                match serde_json::from_slice(&buffer) {
                                    Ok(v) => {
                                        return Ok(v);
                                    },
                                    Err(e) => {
                                        return Err(Box::new(e));
                                    },
                                }
                            }
                        },
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => { //nothing left to process
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
    Err(Box::new(simple_error::simple_error!("system shutting down")))
}
pub fn receive(mut stream:&mut TcpStream, alive_check:fn() -> bool) -> BoxResult<serde_json::Value> {
    let length = receive_length(&mut stream, alive_check)?;
    receive_payload(&mut stream, alive_check, length)
}
