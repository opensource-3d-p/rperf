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

use std::net::{IpAddr, Shutdown, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::channel;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use clap::ArgMatches;

use mio::net::{TcpStream};

use crate::protocol::communication::{receive, send, KEEPALIVE_DURATION};

use crate::protocol::messaging::{
    prepare_begin, prepare_end,
    prepare_upload_configuration, prepare_download_configuration,
};

use crate::protocol::results::{IntervalResult, IntervalResultKind, TestResults, TcpTestResults, UdpTestResults};

use crate::stream::TestStream;
use crate::stream::tcp;
use crate::stream::udp;

use std::error::Error;
type BoxResult<T> = Result<T,Box<dyn Error>>;

/// when false, the system is shutting down
static ALIVE:AtomicBool = AtomicBool::new(true);
/// a deferred kill-switch to handle shutdowns a bit more gracefully in the event of a probable disconnect
static KILL_TIMER:AtomicU64 = AtomicU64::new(0);
const KILL_TIMEOUT:u64 = 5; //once testing finishes, allow a few seconds for the server to respond

const CONNECT_TIMEOUT:Duration = Duration::from_secs(2);

fn connect_to_server(address:&str, port:&u16) -> BoxResult<TcpStream> {
    let destination = format!("{}:{}", address, port);
    log::info!("connecting to server at {}...", destination);
    
    let server_addr = destination.to_socket_addrs()?.next();
    if server_addr.is_none() {
        return Err(Box::new(simple_error::simple_error!("unable to resolve {}", address)));
    }
    let raw_stream = match std::net::TcpStream::connect_timeout(&server_addr.unwrap(), CONNECT_TIMEOUT) {
        Ok(s) => s,
        Err(e) => return Err(Box::new(simple_error::simple_error!("unable to connect: {}", e))),
    };
    let stream = match TcpStream::from_stream(raw_stream) {
        Ok(s) => s,
        Err(e) => return Err(Box::new(simple_error::simple_error!("unable to prepare TCP control-channel: {}", e))),
    };
    log::info!("connected to server");
    
    stream.set_nodelay(true).expect("cannot disable Nagle's algorithm");
    stream.set_keepalive(Some(KEEPALIVE_DURATION)).expect("unable to set TCP keepalive");
    
    Ok(stream)
}

fn prepare_test_results(is_udp:bool, stream_count:u8) -> Mutex<Box<dyn TestResults>> {
    if is_udp { //UDP
        let mut udp_test_results = UdpTestResults::new();
        for i in 0..stream_count {
            udp_test_results.prepare_index(&i);
        }
        Mutex::new(Box::new(udp_test_results))
    } else { //TCP
        let mut tcp_test_results = TcpTestResults::new();
        for i in 0..stream_count {
            tcp_test_results.prepare_index(&i);
        }
        Mutex::new(Box::new(tcp_test_results))
    }
}


pub fn execute(args:ArgMatches) -> BoxResult<()> {
    let mut complete = false;
    
    //config-parsing and pre-connection setup
    let cpu_affinity_manager = Arc::new(Mutex::new(crate::utils::cpu_affinity::CpuAffinityManager::new(args.value_of("affinity").unwrap())?));
    
    let display_json:bool;
    let display_bit:bool;
    match args.value_of("format").unwrap() {
        "json" => {
            display_json = true;
            display_bit = false;
        },
        "megabit" => {
            display_json = false;
            display_bit = true;
        },
        "megabyte" => {
            display_json = false;
            display_bit = false;
        },
        _ => {
            log::error!("unsupported display-mode; defaulting to JSON");
            display_json = true;
            display_bit = false;
        },
    }
    
    let is_udp = args.is_present("udp");
    
    let test_id = uuid::Uuid::new_v4();
    let mut upload_config = prepare_upload_configuration(&args, test_id.as_bytes())?;
    let mut download_config = prepare_download_configuration(&args, test_id.as_bytes())?;
    
    
    //connect to the server
    let mut stream = connect_to_server(&args.value_of("client").unwrap(), &(args.value_of("port").unwrap().parse()?))?;
    let server_addr = stream.peer_addr()?;
    
    
    //scaffolding to track and relay the streams and stream-results associated with this test
    let stream_count = download_config.get("streams").unwrap().as_i64().unwrap() as usize;
    let mut parallel_streams:Vec<Arc<Mutex<(dyn TestStream + Sync + Send)>>> = Vec::with_capacity(stream_count);
    let mut parallel_streams_joinhandles = Vec::with_capacity(stream_count);
    let (results_tx, results_rx):(std::sync::mpsc::Sender<Box<dyn IntervalResult + Sync + Send>>, std::sync::mpsc::Receiver<Box<dyn IntervalResult + Sync + Send>>) = channel();
    
    let test_results:Mutex<Box<dyn TestResults>> = prepare_test_results(is_udp, stream_count as u8);
    
    //a closure used to pass results from stream-handlers to the test-result structure
    let mut results_handler = || -> BoxResult<()> {
        loop { //drain all results every time this closer is invoked
            match results_rx.try_recv() { //see if there's a result to pass on
                Ok(result) => {
                    if !display_json { //since this runs in the main thread, which isn't involved in any testing, render things immediately
                        println!("{}", result.to_string(display_bit));
                    }
                    
                    //update the test-results accordingly
                    let mut tr = test_results.lock().unwrap();
                    match result.kind() {
                        IntervalResultKind::ClientDone | IntervalResultKind::ClientFailed => {
                            if result.kind() == IntervalResultKind::ClientDone {
                                log::info!("stream {} is done", result.get_stream_idx());
                            } else {
                                log::warn!("stream {} failed", result.get_stream_idx());
                            }
                            tr.mark_stream_done(&result.get_stream_idx(), result.kind() == IntervalResultKind::ClientDone);
                            if tr.count_in_progress_streams() == 0 {
                                complete = true;
                                
                                if tr.count_in_progress_streams_server() > 0 {
                                    log::info!("giving the server a few seconds to report results...");
                                    start_kill_timer(KILL_TIMEOUT);
                                } else { //all data gathered from both sides
                                    kill();
                                }
                            }
                        },
                        _ => {
                            tr.update_from_json(result.to_json())?;
                        }
                    }
                },
                Err(_) => break, //whether it's empty or disconnected, there's nothing to do
            }
        }
        Ok(())
    };
    
    //depending on whether this is a forward- or reverse-test, the order of configuring test-streams will differ
    if args.is_present("reverse") {
        log::debug!("running in reverse-mode: server will be uploading data");
        
        //when we're receiving data, we're also responsible for letting the server know where to send it
        let mut stream_ports = Vec::with_capacity(stream_count);
        
        if is_udp { //UDP
            log::info!("preparing for reverse-UDP test with {} streams...", stream_count);
            
            let test_definition = udp::UdpTestDefinition::new(&download_config)?;
            for stream_idx in 0..stream_count {
                log::debug!("preparing UDP-receiver for stream {}...", stream_idx);
                let test = udp::receiver::UdpReceiver::new(
                    test_definition.clone(), &(stream_idx as u8),
                    &0,
                    &server_addr.ip(),
                    &(download_config["receive_buffer"].as_i64().unwrap() as usize),
                )?;
                stream_ports.push(test.get_port()?);
                parallel_streams.push(Arc::new(Mutex::new(test)));
            }
        } else { //TCP
            log::info!("preparing for reverse-TCP test with {} streams...", stream_count);
            
            let test_definition = tcp::TcpTestDefinition::new(&download_config)?;
            for stream_idx in 0..stream_count {
                log::debug!("preparing TCP-receiver for stream {}...", stream_idx);
                let test = tcp::receiver::TcpReceiver::new(
                    test_definition.clone(), &(stream_idx as u8),
                    &0,
                    &server_addr.ip(),
                    &(download_config["receive_buffer"].as_i64().unwrap() as usize),
                )?;
                stream_ports.push(test.get_port()?);
                parallel_streams.push(Arc::new(Mutex::new(test)));
            }        
        }
        
        //add the port-list to the upload-config that the server will receive; this is in stream-index order
        upload_config["stream_ports"] = serde_json::json!(stream_ports);
        
        //let the server know what we're expecting
        send(&mut stream, &upload_config)?;
    } else {
        log::debug!("running in forward-mode: server will be receiving data");
        
        //let the server know to prepare for us to connect
        send(&mut stream, &download_config)?;
        //NOTE: we don't prepare to send data at this point; that happens in the loop below, after the server signals that it's ready
    }
    
    
    //now that the server knows what we need to do, we have to wait for its response
    let connection_payload = receive(&mut stream, is_alive, &mut results_handler)?;
    match connection_payload.get("kind") {
        Some(kind) => {
            match kind.as_str().unwrap_or_default() {
                "connect" => { //we need to connect to the server
                    if is_udp { //UDP
                        log::info!("preparing for UDP test with {} streams...", stream_count);
                        
                        let test_definition = udp::UdpTestDefinition::new(&upload_config)?;
                        for (stream_idx, port) in connection_payload.get("stream_ports").unwrap().as_array().unwrap().iter().enumerate() {
                            log::debug!("preparing UDP-sender for stream {}...", stream_idx);
                            let test = udp::sender::UdpSender::new(
                                test_definition.clone(), &(stream_idx as u8),
                                &0, &server_addr.ip(), &(port.as_i64().unwrap() as u16),
                                &(upload_config["duration"].as_f64().unwrap() as f32),
                                &(upload_config["send_interval"].as_f64().unwrap() as f32),
                                &(upload_config["send_buffer"].as_i64().unwrap() as usize),
                            )?;
                            parallel_streams.push(Arc::new(Mutex::new(test)));
                        }
                    } else { //TCP
                        log::info!("preparing for TCP test with {} streams...", stream_count);
                        
                        let test_definition = tcp::TcpTestDefinition::new(&upload_config)?;
                        for (stream_idx, port) in connection_payload.get("stream_ports").unwrap().as_array().unwrap().iter().enumerate() {
                            log::debug!("preparing TCP-sender for stream {}...", stream_idx);
                            let test = tcp::sender::TcpSender::new(
                                test_definition.clone(), &(stream_idx as u8),
                                &server_addr.ip(), &(port.as_i64().unwrap() as u16),
                                &(upload_config["duration"].as_f64().unwrap() as f32),
                                &(upload_config["send_interval"].as_f64().unwrap() as f32),
                                &(upload_config["send_buffer"].as_i64().unwrap() as usize),
                                &(upload_config["no_delay"].as_bool().unwrap()),
                            )?;
                            parallel_streams.push(Arc::new(Mutex::new(test)));
                        }
                    }
                },
                "connect-ready" => { //server is ready to connect to us
                    //nothing more to do in this flow
                },
                _ => {
                    log::error!("invalid data from {}: {}", stream.peer_addr()?, serde_json::to_string(&connection_payload)?);
                    kill();
                },
            }
        },
        None => {
            log::error!("invalid data from {}: {}", stream.peer_addr()?, serde_json::to_string(&connection_payload)?);
            kill();
        },
    }
    
    
    if is_alive() { //if interrupted while waiting for the server to respond, there's no reason to continue
        log::info!("informing server that testing can begin");
        //tell the server to start
        send(&mut stream, &prepare_begin())?;
        
        log::debug!("spawning stream-threads");
        //begin the test-streams
        for (stream_idx, parallel_stream) in parallel_streams.iter_mut().enumerate() {
            log::info!("beginning execution of stream {}...", stream_idx);
            let c_ps = Arc::clone(&parallel_stream);
            let c_results_tx = results_tx.clone();
            let c_cam = cpu_affinity_manager.clone();
            let handle = thread::spawn(move || {
                { //set CPU affinity, if enabled
                    c_cam.lock().unwrap().set_affinity();
                }
                loop {
                    let mut test = c_ps.lock().unwrap();
                    log::debug!("beginning test-interval for stream {}", test.get_idx());
                    match test.run_interval() {
                        Some(interval_result) => match interval_result {
                            Ok(ir) => match c_results_tx.send(ir) {
                                Ok(_) => (),
                                Err(e) => {
                                    log::error!("unable to report interval-result: {}", e);
                                    break
                                },
                            },
                            Err(e) => {
                                log::error!("unable to process stream: {}", e);
                                match c_results_tx.send(Box::new(crate::protocol::results::ClientFailedResult{stream_idx: test.get_idx()})) {
                                    Ok(_) => (),
                                    Err(e) => log::error!("unable to report interval-failed-result: {}", e),
                                }
                                break;
                            },
                        },
                        None => {
                            match c_results_tx.send(Box::new(crate::protocol::results::ClientDoneResult{stream_idx: test.get_idx()})) {
                                Ok(_) => (),
                                Err(e) => log::error!("unable to report interval-done-result: {}", e),
                            }
                            break;
                        },
                    }
                }
            });
            parallel_streams_joinhandles.push(handle);
        }
        
        //watch for events from the server
        while is_alive() {
            match receive(&mut stream, is_alive, &mut results_handler) {
                Ok(payload) => {
                    match payload.get("kind") {
                        Some(kind) => {
                            match kind.as_str().unwrap_or_default() {
                                "receive" | "send" => { //receive-results from the server
                                    if !display_json {
                                        let result = crate::protocol::results::interval_result_from_json(payload.clone())?;
                                        println!("{}", result.to_string(display_bit));
                                    }
                                    let mut tr = test_results.lock().unwrap();
                                    tr.update_from_json(payload)?;
                                },
                                "done" | "failed" => match payload.get("stream_idx") { //completion-result from the server
                                    Some(stream_idx) => match stream_idx.as_i64() {
                                        Some(idx64) => {
                                            let mut tr = test_results.lock().unwrap();
                                            match kind.as_str().unwrap() {
                                                "done" => {
                                                    log::info!("server reported completion of stream {}", idx64);
                                                },
                                                "failed" => {
                                                    log::warn!("server reported failure with stream {}", idx64);
                                                    tr.mark_stream_done(&(idx64 as u8), false);
                                                },
                                                _ => (), //not possible
                                            }
                                            tr.mark_stream_done_server(&(idx64 as u8));
                                            
                                            if tr.count_in_progress_streams() == 0 && tr.count_in_progress_streams_server() == 0 { //all data gathered from both sides
                                                kill();
                                            }
                                        },
                                        None => log::error!("completion from server did not include a valid stream_idx"),
                                    },
                                    None => log::error!("completion from server did not include stream_idx"),
                                },
                                _ => {
                                    log::error!("invalid data from {}: {}", stream.peer_addr()?, serde_json::to_string(&connection_payload)?);
                                    break;
                                },
                            }
                        },
                        None => {
                            log::error!("invalid data from {}: {}", stream.peer_addr()?, serde_json::to_string(&connection_payload)?);
                            break;
                        },
                    }
                },
                Err(e) => {
                    if !complete { //when complete, this also occurs
                        return Err(e);
                    }
                    break;
                },
            }
        }
    }
    
    //assume this is a controlled shutdown; if it isn't, this is just a very slight waste of time
    send(&mut stream, &prepare_end()).unwrap_or_default();
    thread::sleep(Duration::from_millis(250)); //wait a moment for the "end" message to be queued for delivery to the server
    stream.shutdown(Shutdown::Both).unwrap_or_default();
    
    log::debug!("stopping any still-in-progress streams");
    for ps in parallel_streams.iter_mut() {
        let mut stream = match (*ps).lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                log::error!("a stream-handler was poisoned; this indicates some sort of logic error");
                poisoned.into_inner()
            },
        };
        stream.stop();
    }
    log::debug!("waiting for all streams to end");
    for jh in parallel_streams_joinhandles {
        match jh.join() {
            Ok(_) => (),
            Err(e) => log::error!("error in parallel stream: {:?}", e),
        }
    }
    
    let common_config:serde_json::Value;
    //sanitise the config structures for export
    {
        let upload_config_map = upload_config.as_object_mut().unwrap();
        let cc_family = upload_config_map.remove("family");
        upload_config_map.remove("kind");
        let cc_length = upload_config_map.remove("length");
        upload_config_map.remove("role");
        let cc_streams = upload_config_map.remove("streams");
        upload_config_map.remove("test_id");
        upload_config_map.remove("stream_ports");
        if upload_config_map["send_buffer"].as_i64().unwrap() == 0 {
            upload_config_map.remove("send_buffer");
        }
        
        let download_config_map = download_config.as_object_mut().unwrap();
        download_config_map.remove("family");
        download_config_map.remove("kind");
        download_config_map.remove("length");
        download_config_map.remove("role");
        download_config_map.remove("streams");
        download_config_map.remove("test_id");
        if download_config_map["receive_buffer"].as_i64().unwrap() == 0 {
            download_config_map.remove("receive_buffer");
        }
        
        common_config = serde_json::json!({
            "family": cc_family,
            "length": cc_length,
            "streams": cc_streams,
        });
    }
    
    log::debug!("displaying test results");
    let omit_seconds:usize = args.value_of("omit").unwrap().parse()?;
    {
        let tr = test_results.lock().unwrap();
        if display_json {
            println!("{}", tr.to_json_string(omit_seconds, upload_config, download_config, common_config, serde_json::json!({
                "omit_seconds": omit_seconds,
                "ip_version": match server_addr.ip() {
                    IpAddr::V4(_) => 4,
                    IpAddr::V6(_) => 6,
                },
                "reverse": args.is_present("reverse"),
            })));
        } else {
            println!("{}", tr.to_string(display_bit, omit_seconds));
        }
    }
    
    Ok(())
}

pub fn kill() -> bool {
    ALIVE.swap(false, Ordering::Relaxed)
}
fn start_kill_timer(timeout:u64) {
    KILL_TIMER.swap(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + timeout, Ordering::Relaxed);
}
fn is_alive() -> bool {
    let kill_timer = KILL_TIMER.load(Ordering::Relaxed);
    if kill_timer != 0 { //initialised
        if SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() >= kill_timer {
            return false;
        }
    }
    ALIVE.load(Ordering::Relaxed)
}
