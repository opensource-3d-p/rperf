#[macro_use] extern crate log;

use std::collections::HashSet;
use std::fmt::{format};
use std::net::{Shutdown};
use std::sync::Mutex;
use std::thread::{sleep};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mio::net::{TcpListener, TcpStream};
use mio::{Events, Interest, Poll, PollOpt, Ready, Token};

use serde::Deserialize;

//500ms timeout
const POLL_TIMEOUT = Duration::new(0, 500_000_000);

mod server {
    let mut alive = true;
    let mut clients = HashSet::new();
    let clients_lock = Mutex::new(0);
    
    fn handle_client_get_length(&mut stream:TcpStream) -> Result<u16> {
        let mio_token = Token(0);
        let poll = Poll::new()?;
        poll.register(
            &stream,
            mio_token,
            Ready::readable(),
            PollOpt::edge(),
        )?;
        
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
    
    fn handle_client_get_payload(&mut stream:TcpStream, length:u16) -> Result<serde_json::Value> {
        let mio_token = Token(0);
        let poll = Poll::new()?;
        poll.register(
            &stream,
            mio_token,
            Ready::readable(),
            PollOpt::edge(),
        )?;
        
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
    
    fn handle_client(mut stream:TcpStream) {
        let mut events = Events::with_capacity(1); //only interacting with one source
        while alive {
            let length = handle_client_get_length(&stream);
            if length.is_err() {
                error!("lost connection to {}: {:?}", stream.peer_addr().unwrap(), length.err());
                stream.shutdown(Shutdown::Both).unwrap();
                break;
            }
            
            let payload = handle_client_get_payload(&stream, length.unwrap());
            if payload.is_err() {
                error!("lost connection to {}: {:?}", stream.peer_addr().unwrap(), payload.err());
                stream.shutdown(Shutdown::Both).unwrap();
                break;
            }
            
            //TODO: process payload
            
            //it will probably want to start a test, which entails spawning all of the parallel streams and giving them a callback function
            //to write to the stream
            //meanwhile, this thread needs to continue to monitor for events, specifically "bye" (everything else can be discarded)
            
        }
        
        {
            let lock_raii = clients_lock.lock();
            clients.remove(stream.peer_addr().unwrap());
        }
    }
    
    pub fn serve(ip_version:&u8, &u16 port) -> Result<()> {
        let listener:TcpListener;
        if ip_version == 4 {
            listener = TcpListener::bind(format!("0.0.0.0:{}", port).parse()?).expect(format!("failed to bind TCP socket, port {}", port));
        } else if ip_version == 6 {
            listener = TcpListener::bind(format!(":::{}", port).parse()?).expect(format!("failed to bind TCP socket, port {}", port));
        } else {
            return Err(format!("unsupported IP version: {}", ip_version));
        }
        info!(format!("server listening on port {}", port));
        
        let mio_token = Token(0);
        let poll = Poll::new()?;
        poll.register(
            &listener,
            mio_token,
            Ready::readable(),
            PollOpt::edge(),
        )?;
        
        let mut events = Events::with_capacity(8); //there should really only be one client trying to connect at a time
        loop {
            if !alive {
                break;
            }
            
            poll.poll(&mut events, POLL_TIMEOUT)?;
            for event in events {
                match event.token() {
                    mio_token => loop {
                        match listener.accept() {
                            Ok((stream, address)) => {
                                info!(format!("connection from {}", address));
                                
                                stream.set_nonblocking(true).expect("cannot make client connection non-blocking");
                                
                                {
                                    let lock_raii = clients_lock.lock();
                                    clients.insert(address);
                                }
                                thread::spawn(move || {
                                    handle_client(stream)
                                });
                            },
                            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => { //nothing to do
                                break;
                            },
                            Err(e) => {
                                return Err(e);
                            },
                        }
                    },
                    _ => {
                        warn!("got event for unbound token: {:?}", event);
                    },
                }
            }
        }
        
        //wait until all clients have been disconnected
        while clients.len() > 0 {
            info!("waiting for {} clients to finish...", clients.len());
            sleep(POLL_TIMEOUT);
        }
    }
    
    pub fn kill() {
        alive = false;
    }
}
