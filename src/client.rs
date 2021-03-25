use std::error::Error;
use std::net::{Shutdown};
use std::sync::atomic::{AtomicBool, Ordering};

use mio::net::{TcpStream};
use mio::{Events, Ready, Poll, PollOpt, Token};

use crate::protocol::communication::{receive, send, KEEPALIVE_DURATION};

use crate::protocol::messaging::{prepare_begin, prepare_end};

type BoxResult<T> = Result<T,Box<dyn Error>>;

const alive:AtomicBool = AtomicBool::new(true);

pub fn execute(server_address:&str, port:&u16, ip_version:&u8) -> BoxResult<()> {
    log::info!("connecting to server at {}:{}...", server_address, port);
    let mut stream_result = TcpStream::connect(&format!("{}:{}", server_address, port).parse()?);
    if stream_result.is_err() {
        return Err(Box::new(simple_error::simple_error!(format!("unable to connect: {:?}", stream_result.unwrap_err()).as_str())));
    }
    let mut stream = stream_result.unwrap();
    stream.set_nodelay(true).expect("cannot disable Nagle's algorithm");
    stream.set_keepalive(Some(KEEPALIVE_DURATION)).expect("unable to set TCP keepalive");
    
    //TODO: send test details to server
    //if running in reverse-mode, also prepare the local streams/threads and include their details in this message
    
    while is_alive() {
        let payload_wrapped = receive(&mut stream, is_alive);
        if payload_wrapped.is_err() {
            log::error!("lost connection to {}: {:?}", stream.peer_addr().unwrap(), payload_wrapped.err());
            stream.shutdown(Shutdown::Both)?;
            break;
        }
        let payload = payload_wrapped.unwrap();
        
        //TODO: process payload
        
        //the first response will specify how to connect (or indicate that the server has connected to our streams)
        //if we're uploading, spawn threads/streams here
        match payload.get("kind") {
            Some(kind) => {
                match kind.as_str().unwrap_or_default() {
                    "connect" => { //we need to connect to the server
                        //TODO: connect to server
                        
                        let send_result = send(&mut stream, &prepare_begin());
                        if send_result.is_err() {
                            log::error!("lost connection to {}: {:?}", stream.peer_addr().unwrap(), send_result.err());
                            stream.shutdown(Shutdown::Both)?;
                            break;
                        }
                        
                        //start all thread iteration
                        
                    },
                    "connected" => { //server has connected to us
                        let send_result = send(&mut stream, &prepare_begin());
                        if send_result.is_err() {
                            log::error!("lost connection to {}: {:?}", stream.peer_addr().unwrap(), send_result.err());
                            stream.shutdown(Shutdown::Both)?;
                            break;
                        }
                        
                        //start all thread iteration
                        
                    },
                    _ => {
                        log::error!("invalid data from {}", stream.peer_addr().unwrap());
                        stream.shutdown(Shutdown::Both)?;
                        break;
                    },
                }
            },
            None => {
                log::error!("invalid data from {}", stream.peer_addr().unwrap());
                stream.shutdown(Shutdown::Both)?;
                break;
            },
        }
        
        //after that response is received, send a message to begin; once that message has been sent, tell all of the threads to begin iterating,
        //with a callback function to update the execution data (this is passed back through a queue to prevent them from blocking)
        //if output is non-JSON, this thread is responsible for not just updating the structures, but also formatting and presentation
        
        //std::sync::mpsc
        
        //every subsequent message from the server will be one of its iteration results; when received, treat them the same way as the local
        //iteration results
        
        //if the server is uploading, each of its iterators will be capped with a "done" message, which sets a flag in the local iteration results
        //if we're uploading, send a "done" to the server under the same conditions, which it will match with its own "done", which we use to update our local state
        
        //when all streams have finished, send an "end" message to the server
        //then break, since everything is synced
    }
    
    //if any iterators are still running, kill them (this is because we may have disconnected from the server prematurely)
    
    //display final results
    
    Ok(())
}

pub fn kill() -> bool {
    let was_alive = is_alive();
    alive.store(false, Ordering::Relaxed);
    was_alive
}
fn is_alive() -> bool {
    alive.load(Ordering::Relaxed)
}
