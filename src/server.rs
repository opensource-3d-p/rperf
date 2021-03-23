#[macro_use]
extern crate log;

use std::fmt::{format};
use std::thread;
use std::time;
use std::net::{TcpListener, TcpStream, Shutdown};
use std::io::{Read, Write};
use std::collections::HashSet;

mod server {
    const SHUTDOWN_CHECK_INTERVAL_MS = 250;
    
    let mut alive = true;
    
    //TODO: use a read/write timeout
    
    fn handle_client(mut stream: TcpStream, mut &clients: HashSet<String>) {
        //TODO: if alive becomes false, send the "bye" directive to the client and disconnect immediately
        
        //this is accomplished using serde_json to serialise an instance of the MessageBye struct with a type of "bye"
        //and the message is prefixed by a number of bytes indicating length
        //two-byte length prefix value
        //this also lets clients potentially be implemented in other languages, if it makes sense to do so
        
        //during the read cycle, read two bytes, determine length, then wait to receive that many before deserialising
        
        //during the write cycle, send Bye if needed; otherwise, supply whatever the client needs
        
        let mut data = [0 as u8; 50]; // using 50 byte buffer
        while match stream.read(&mut data) {
            Ok(size) => {
                // echo everything!
                stream.write(&data[0..size]).unwrap();
                true
            },
            Err(_) => {
                println!("An error occurred, terminating connection with {}", stream.peer_addr().unwrap());
                stream.shutdown(Shutdown::Both).unwrap();
                false
            }
        } {}
        
        clients.remove(stream.peer_addr().unwrap());
    }
    
    fn serve(&i32 port) {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).unwrap();
        info!(format!("server listening on port {}", port));
        listener.set_nonblocking(true).expect("cannot make TCP server non-blocking");
        
        let mut clients = HashSet::new();
        
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let peer_addr = stream.peer_addr().unwrap();
                    info!(format!("connection from {}", peer_addr));
                    
                    stream.set_nonblocking(true).expect("cannot make client connection non-blocking");
                    
                    thread::spawn(move || {
                        handle_client(stream, &handlers)
                    });
                    
                    clients.insert(peer_addr);
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    if !alive { //system is shutting down
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(SHUTDOWN_CHECK_INTERVAL_MS));
                }
                Err(e) => {
                    error!(format!("unable to accept connection: {}", e))
                }
            }
        }
        drop(listener);
        
        //wait until all clients have been disconnected
        while clients.len() > 0 {
            std::thread::sleep(std::time::Duration::from_millis(SHUTDOWN_CHECK_INTERVAL_MS));
        }
    }
    
    fn stop() {
        alive = false;
    }
}
