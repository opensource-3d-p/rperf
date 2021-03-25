#[macro_use] extern crate log;

use std::fmt::{format};
use std::net::{Shutdown};
use std::sync::Mutex;
use std::thread::{sleep};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mio::net::{TcpStream};
use mio::{Events, Interest, Poll, PollOpt, Ready, Token};

use serde::Deserialize;

use protocol::{communication_get_length, communication_get_payload, KEEPALIVE_DURATION};

mod client {
    let mut alive = true;
    
    pub fn execute(server_address:&str, &u16 port, ip_version:&u8) -> Result<()> {
        info!(format!("connecting to server at {}:{}...", server_address, port));
        let mut stream_result = TcpStream::connect(format!("{}:{}", server_address, port));
        if stream_result.is_err() {
            TcpStream::connect(format!("{}:{}", server_address, port)).expect(format!("unable to connect: {:?}", stream_result.unwrap_err()));
        }
        let mut stream = stream_result.unwrap();
        stream.set_keepalive(KEEPALIVE_DURATION).expect("unable to set TCP keepalive");
        
        //TODO: send test details to server
        //if running in reverse-mode, also prepare the local streams/threads and include their details in this message
        
        let mio_token = Token(0);
        let poll = Poll::new()?;
        poll.register(
            &listener,
            mio_token,
            Ready::readable(),
            PollOpt::edge(),
        )?;
        let mut events = Events::with_capacity(1); //only connected to the server
        
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
            
            //the first response will specify how to connect (or indicate that the server has connected to our streams)
            //if we're uploading, spawn threads/streams here
            
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
    }
    
    pub fn kill() {
        alive = false;
    }
}
