/*
 * Copyright (C) 2021 Evtech Solutions, Ltd., dba 3D-P
 * Copyright (C) 2021 Neil Tallim <neiltallim@3d-p.com>
 *
 * This file is part of rperf.
 *
 * rperf is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * rperf is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with rperf.  If not, see <https://www.gnu.org/licenses/>.
 */

use std::io::{self, Read, Write};
use std::time::Duration;

use mio::net::TcpStream;
use mio::{Events, Interest, Poll};

use crate::BoxResult;

/// how long to wait for keepalive events
/// the communications channels typically exchange data every second, so 2s is reasonable to avoid excess noise
pub const KEEPALIVE_DURATION: Duration = Duration::from_secs(2);

/// how long to block on polling operations
const POLL_TIMEOUT: Duration = Duration::from_millis(50);

/// sends JSON data over a client-server communications stream
pub fn send(stream: &mut TcpStream, message: &serde_json::Value) -> BoxResult<()> {
    let serialised_message = serde_json::to_vec(message)?;

    log::debug!(
        "sending message of length {}, {:?}, to {}...",
        serialised_message.len(),
        message,
        stream.peer_addr()?
    );
    let mut output_buffer = vec![0_u8; serialised_message.len() + 2];
    output_buffer[..2].copy_from_slice(&(serialised_message.len() as u16).to_be_bytes());
    output_buffer[2..].copy_from_slice(serialised_message.as_slice());
    Ok(stream.write_all(&output_buffer)?)
}

/// receives the length-count of a pending message over a client-server communications stream
fn receive_length(stream: &mut TcpStream, alive_check: fn() -> bool, results_handler: &mut dyn FnMut() -> BoxResult<()>) -> BoxResult<u16> {
    let mio_token = crate::get_global_token();
    let mut poll = Poll::new()?;
    poll.registry().register(stream, mio_token, Interest::READABLE)?;
    let mut events = Events::with_capacity(1); //only interacting with one stream

    let mut length_bytes_read = 0;
    let mut length_spec: [u8; 2] = [0; 2];
    let result: BoxResult<u16> = 'exiting: loop {
        if !alive_check() {
            break 'exiting Ok(0);
        }
        //waiting to find out how long the next message is
        results_handler()?; //send any outstanding results between cycles
        poll.poll(&mut events, Some(POLL_TIMEOUT))?;
        for event in events.iter() {
            event.token();
            loop {
                let size = match stream.read(&mut length_spec[length_bytes_read..]) {
                    Ok(size) => size,
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        //nothing left to process
                        break;
                    }
                    Err(e) => {
                        break 'exiting Err(Box::new(e));
                    }
                };

                if size == 0 {
                    if alive_check() {
                        break 'exiting Err(Box::new(simple_error::simple_error!("connection lost")));
                    } else {
                        //shutting down; a disconnect is expected
                        break 'exiting Err(Box::new(simple_error::simple_error!("local shutdown requested")));
                    }
                }

                length_bytes_read += size;
                if length_bytes_read == 2 {
                    let length = u16::from_be_bytes(length_spec);
                    log::debug!("received length-spec of {} from {}", length, stream.peer_addr()?);
                    break 'exiting Ok(length);
                } else {
                    log::debug!("received partial length-spec from {}", stream.peer_addr()?);
                }
            }
        }
    };
    poll.registry().deregister(stream)?;
    result
}

/// receives the data-value of a pending message over a client-server communications stream
fn receive_payload(
    stream: &mut TcpStream,
    alive_check: fn() -> bool,
    results_handler: &mut dyn FnMut() -> BoxResult<()>,
    length: u16,
) -> BoxResult<serde_json::Value> {
    let mio_token = crate::get_global_token();
    let mut poll = Poll::new()?;
    poll.registry().register(stream, mio_token, Interest::READABLE)?;
    let mut events = Events::with_capacity(1); //only interacting with one stream

    let mut bytes_read = 0;
    let mut buffer = vec![0_u8; length.into()];
    let result: BoxResult<serde_json::Value> = 'exiting: loop {
        if !alive_check() {
            break 'exiting Ok(serde_json::from_slice(&buffer[0..0])?);
        }
        //waiting to receive the payload
        results_handler()?; //send any outstanding results between cycles
        poll.poll(&mut events, Some(POLL_TIMEOUT))?;
        for event in events.iter() {
            event.token();
            loop {
                let size = match stream.read(&mut buffer[bytes_read..]) {
                    Ok(size) => size,
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        // nothing left to process
                        break;
                    }
                    Err(e) => {
                        break 'exiting Err(Box::new(e));
                    }
                };

                if size == 0 {
                    if alive_check() {
                        break 'exiting Err(Box::new(simple_error::simple_error!("connection lost")));
                    } else {
                        // shutting down; a disconnect is expected
                        break 'exiting Err(Box::new(simple_error::simple_error!("local shutdown requested")));
                    }
                }

                bytes_read += size;
                if bytes_read == length as usize {
                    match serde_json::from_slice(&buffer) {
                        Ok(v) => {
                            log::debug!("received {:?} from {}", v, stream.peer_addr()?);
                            break 'exiting Ok(v);
                        }
                        Err(e) => {
                            break 'exiting Err(Box::new(e));
                        }
                    }
                } else {
                    log::debug!("received partial payload from {}", stream.peer_addr()?);
                }
            }
        }
    };
    poll.registry().deregister(stream)?;
    result
}

/// handles the full process of retrieving a message from a client-server communications stream
pub fn receive(
    stream: &mut TcpStream,
    alive_check: fn() -> bool,
    results_handler: &mut dyn FnMut() -> BoxResult<()>,
) -> BoxResult<serde_json::Value> {
    log::debug!("awaiting length-value from {}...", stream.peer_addr()?);
    let length = receive_length(stream, alive_check, results_handler)?;
    log::debug!("awaiting payload from {}...", stream.peer_addr()?);
    receive_payload(stream, alive_check, results_handler, length)
}
