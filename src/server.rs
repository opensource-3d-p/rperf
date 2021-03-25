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

use protocol::{communication_get_length, communication_get_payload, KEEPALIVE_DURATION};

mod server {
    //500ms timeout
    const POLL_TIMEOUT = Duration::new(0, 500_000_000);
    
    let mut alive = true;
    let mut clients = HashSet::new();
    let clients_lock = Mutex::new(0);
    
    fn handle_client(mut stream:TcpStream) {
        let mut events = Events::with_capacity(1); //only interacting with one source
        while alive {
            let length = communication_get_length(&stream);
            if length.is_err() {
                error!("lost connection to {}: {:?}", stream.peer_addr().unwrap(), length.err());
                stream.shutdown(Shutdown::Both).unwrap();
                break;
            }
            
            let payload = communication_get_payload(&stream, length.unwrap());
            if payload.is_err() {
                error!("lost connection to {}: {:?}", stream.peer_addr().unwrap(), payload.err());
                stream.shutdown(Shutdown::Both).unwrap();
                break;
            }
            
            //TODO: process payload
            
            //it will probably want to start a test, which entails spawning all of the parallel streams and giving them a callback function
            //to write to the stream (this uses a queue that's managed by this thread; specifically, the queue should be passed to
            //communication_get_length and communication_get_payload so they can dump into it between poll cycles)
            
            //std::sync::mpsc
            
            //meanwhile, this thread needs to continue to monitor for events, specifically "begin" and "end"
            
        }
        
        //if any iterators are still running, kill them (this is because we may have disconnected from the server prematurely)
        
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
        let mut events = Events::with_capacity(32);
        
        while alive {
            poll.poll(&mut events, POLL_TIMEOUT)?;
            for event in events {
                match event.token() {
                    mio_token => loop {
                        match listener.accept() {
                            Ok((stream, address)) => {
                                info!(format!("connection from {}", address));
                                
                                stream.set_nonblocking(true).expect("cannot make client connection non-blocking");
                                stream.set_keepalive(KEEPALIVE_DURATION).expect("unable to set TCP keepalive");
                                
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
