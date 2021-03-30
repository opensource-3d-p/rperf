use std::error::Error;
use std::io;
use std::net::{Shutdown};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration};

use chashmap::CHashMap;

use clap::ArgMatches;

use mio::net::{TcpListener, TcpStream};
use mio::{Events, Ready, Poll, PollOpt, Token};

use crate::protocol::communication::{receive, send, KEEPALIVE_DURATION};

use crate::protocol::messaging::{
    prepare_connect, prepare_connected,
};

use crate::stream::TestStream;
use crate::stream::tcp;
use crate::stream::udp;

type BoxResult<T> = Result<T,Box<dyn Error>>;

const POLL_TIMEOUT:Duration = Duration::from_millis(500);

static alive:AtomicBool = AtomicBool::new(true);

lazy_static::lazy_static!{
    static ref clients:CHashMap<String, bool> = {
        let hm = CHashMap::new();
        hm
    };
}

fn handle_client(mut stream:&TcpStream, ip_version:&u8) -> BoxResult<()> {
    let peer_addr = stream.peer_addr()?;
    let mut started = false;
    
    let mut parallel_streams:Vec<Box<(dyn TestStream + Sync + Send)>> = Vec::new();
    let mut parallel_streams_joinhandles = Vec::new();
    
    while is_alive() {
        let payload = receive(&mut stream, is_alive)?;
        match payload.get("kind") {
            Some(kind) => {
                match kind.as_str().unwrap() {
                    "configuration" => { //we either need to connect streams to the client or prepare to receive connections
                        if payload.get("role").unwrap_or(&serde_json::json!("download")).as_str().unwrap() == "download" {
                            log::debug!("running in forward-mode: server will be receiving data");
                            
                            let mut stream_ports = Vec::new();
                            if payload.get("family").unwrap_or(&serde_json::json!("tcp")).as_str().unwrap() == "udp" {
                                let test_definition = udp::build_udp_test_definition(&payload)?;
                                for i in 0..(payload.get("streams").unwrap_or(&serde_json::json!(1)).as_i64().unwrap()) {
                                    let test = udp::receiver::UdpReceiver::new(test_definition.clone(), ip_version, &0)?;
                                    stream_ports.push(test.get_port()?);
                                    parallel_streams.push(Box::new(test));
                                }
                            } else { //TCP
                                
                            }
                            send(&mut stream, &prepare_connect(&stream_ports))?;
                        } else { //upload
                            log::debug!("running in reverse-mode: server will be uploading data");
                            
                            if payload.get("family").unwrap_or(&serde_json::json!("tcp")).as_str().unwrap() == "udp" {
                                let test_definition = udp::build_udp_test_definition(&payload)?;
                                for port in payload.get("streamPorts").unwrap_or(&serde_json::json!([])).as_array().unwrap() {
                                    let test = udp::sender::UdpSender::new(
                                        test_definition,
                                        ip_version, &0, peer_addr.ip().to_string(), &(port.as_i64().unwrap_or(0) as u16),
                                        &(payload.get("duration").unwrap_or(&serde_json::json!(0.0)).as_f64().unwrap() as f32),
                                        &(payload.get("sendInterval").unwrap_or(&serde_json::json!(1.0)).as_f64().unwrap() as f32),
                                    )?;
                                    parallel_streams.push(Box::new(test));
                                }
                            } else { //TCP
                                
                            }
                            send(&mut stream, &prepare_connected())?;
                        }
                    },
                    "begin" => {
                        if !started {
                            for parallel_stream in &parallel_streams {
                                let handle = thread::spawn(move || {
                                    loop {
                                        match parallel_stream.run_interval() {
                                            Some(interval_result) => {
                                                //write the result into an std::sync::mpsc instance, which another thread will harvest and forward to the client
                                            },
                                            None => break,
                                        }
                                    }
                                });
                                parallel_streams_joinhandles.push(handle);
                            }
                            started = true;
                        } else {
                            log::error!("duplicate begin-signal from {}", stream.peer_addr()?);
                            break;
                        }
                    },
                    "end" => {
                        log::info!("end-signal from {}", stream.peer_addr()?);
                        break;
                    },
                    _ => {
                        log::error!("invalid data from {}", stream.peer_addr()?);
                        break;
                    },
                }
            },
            None => {
                log::error!("invalid data from {}", stream.peer_addr()?);
                break;
            },
        }
    }
    
    //ensure everything has ended
    for ps in &parallel_streams {
        ps.stop();
    }
    for jh in &parallel_streams_joinhandles {
        match jh.join() {
            Ok(_) => (),
            Err(e) => log::error!("error in parallel stream: {:?}", e),
        }
    }
    
    clients.remove(&peer_addr.to_string());
    Ok(())
}

pub fn serve(args:ArgMatches) -> BoxResult<()> {
    let ip_version:u8;
    if args.is_present("version6") {
        ip_version = 6;
    } else {
        ip_version = 4;
    }
    let port:u16 = args.value_of("port").unwrap().parse()?;
    
    let mut listener:TcpListener;
    if ip_version == 4 {
        listener = TcpListener::bind(&format!("0.0.0.0:{}", port).parse()?).expect(format!("failed to bind TCP socket, port {}", port).as_str());
    } else if ip_version == 6 {
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
                                match handle_client(&stream, &ip_version) {
                                    Ok(_) => (),
                                    Err(e) => log::error!("error in client-handler: {:?}", e),
                                }
                                stream.shutdown(Shutdown::Both).unwrap_or_default();
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
