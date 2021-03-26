use std::error::Error;
use std::io;
use std::net::{Shutdown};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration};

use mio::net::{TcpListener, TcpStream};
use mio::{Events, Ready, Poll, PollOpt, Token};

use chashmap::CHashMap;

use crate::protocol::communication::{receive, send, KEEPALIVE_DURATION};

type BoxResult<T> = Result<T,Box<dyn Error>>;

const POLL_TIMEOUT:Duration = Duration::from_millis(500);

static alive:AtomicBool = AtomicBool::new(true);

lazy_static::lazy_static!{
    static ref clients:CHashMap<String, bool> = {
        let hm = CHashMap::new();
        hm
    };
}

fn handle_client(mut stream:TcpStream) {
    let peer_addr = stream.peer_addr().unwrap();
    while is_alive() {
        let payload = receive(&mut stream, is_alive);
        if payload.is_err() {
            log::error!("lost connection to {}: {:?}", peer_addr, payload.err());
            stream.shutdown(Shutdown::Both).unwrap_or_default();
            break;
        }
        
        //TODO: process payload
        
        //it will probably want to start a test, which entails spawning all of the parallel streams and giving them a callback function
        //to write to the stream (this uses a queue that's managed by this thread; specifically, the queue should be passed to
        //communication_get_length and communication_get_payload so they can dump into it between poll cycles)
        //actually, it should be an Option<fn> closure so the function can just call it without any knowledge
        
        //std::sync::mpsc
        
        //meanwhile, this thread needs to continue to monitor for events, specifically "begin" and "end"
        
    }
    
    //if any iterators are still running, kill them (this is because we may have disconnected from the server prematurely)
    
    clients.remove(&peer_addr.to_string());
}

pub fn serve(port:&u16, ip_version:&u8) -> BoxResult<()> {
    let mut listener:TcpListener;
    if *ip_version == 4 {
        listener = TcpListener::bind(&format!("0.0.0.0:{}", port).parse()?).expect(format!("failed to bind TCP socket, port {}", port).as_str());
    } else if *ip_version == 6 {
        listener = TcpListener::bind(&format!(":::{}", port).parse()?).expect(format!("failed to bind TCP socket, port {}", port).as_str());
    } else {
        return Err(Box::new(simple_error::simple_error!("unsupported IP version: {}", ip_version)));
    }
    log::info!("server listening on port {}", port);
    
    let mio_token = Token(0);
    let poll = Poll::new()?;
    poll.register(
        &mut listener,
        mio_token,
        Ready::readable(),
        PollOpt::edge(),
    )?;
    let mut events = Events::with_capacity(32);
    
    while is_alive() {
        poll.poll(&mut events, Some(POLL_TIMEOUT))?;
        for event in events.iter() {
            match event.token() {
                _ => loop {
                    match listener.accept() {
                        Ok((stream, address)) => {
                            log::info!("connection from {}", address);
                            
                            stream.set_nodelay(true).expect("cannot disable Nagle's algorithm");
                            stream.set_keepalive(Some(KEEPALIVE_DURATION)).expect("unable to set TCP keepalive");
                            
                            clients.insert(address.to_string(), false);
                            
                            thread::spawn(move || {
                                handle_client(stream)
                            });
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
    
    //wait until all clients have been disconnected
    while clients.len() > 0 {
        log::info!("waiting for {} clients to finish...", clients.len());
        thread::sleep(POLL_TIMEOUT);
    }
    Ok(())
}

pub fn kill() -> bool {
    alive.swap(false, Ordering::Relaxed)
}
fn is_alive() -> bool {
    alive.load(Ordering::Relaxed)
}
