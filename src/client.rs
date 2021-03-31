use std::error::Error;
use std::net::{Shutdown};
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
    prepare_configuration_udp_upload, prepare_configuration_udp_download,
};

use crate::protocol::results::{IntervalResult, IntervalResultKind};

use crate::stream::TestStream;
use crate::stream::tcp;
use crate::stream::udp;

type BoxResult<T> = Result<T,Box<dyn Error>>;

static ALIVE:AtomicBool = AtomicBool::new(true);
static KILL_TIMER:AtomicU64 = AtomicU64::new(0);
const KILL_TIMEOUT:u64 = 5; //once testing finishes, allow a few seconds for the server to respond

fn prepare_upload_config(args:&ArgMatches, test_id:&[u8; 16]) -> BoxResult<serde_json::Value> {
    let parallel_streams:u8 = args.value_of("parallel").unwrap().parse()?;
    let bandwidth:u64 = args.value_of("bandwidth").unwrap().parse()?;
    let bytes:u64 = args.value_of("bytes").unwrap().parse()?;
    let mut seconds:f32 = args.value_of("time").unwrap().parse()?;
    let mut send_interval:f32 = args.value_of("sendinterval").unwrap().parse()?;
    let mut length:u32 = args.value_of("length").unwrap().parse()?;
    
    if seconds <= 0.0 {
        log::warn!("time was not in an acceptable range and has been set to 0.0");
        seconds = 0.0
    }
    
    if send_interval > 1.0 || send_interval <= 0.0 {
        log::warn!("send-interval was not in an acceptable range and has been set to 1.0");
        send_interval = 1.0
    }
    
    if args.is_present("udp") {
        log::debug!("preparing UDP download config");
        if length == 0 {
            length = 1024;
        }
        Ok(prepare_configuration_udp_upload(test_id, parallel_streams, bandwidth, bytes, seconds, length as u16, send_interval))
    } else {
        log::debug!("preparing TCP download config");
        if length == 0 {
            length = 128 * 1024;
        }
        Ok(serde_json::json!({}))
    }
}
fn prepare_download_config(args:&ArgMatches, test_id:&[u8; 16]) -> BoxResult<serde_json::Value> {
    let parallel_streams:u8 = args.value_of("parallel").unwrap().parse()?;
    let mut length:u32 = args.value_of("length").unwrap().parse()?;
    
    if args.is_present("udp") {
        log::debug!("preparing UDP download config");
        if length == 0 {
            length = 1024;
        }
        Ok(prepare_configuration_udp_download(test_id, parallel_streams, length as u16))
    } else {
        log::debug!("preparing TCP download config");
        if length == 0 {
            length = 128 * 1024;
        }
        Ok(serde_json::json!({}))
    }
}
            

pub fn execute(args:ArgMatches) -> BoxResult<()> {
    let mut complete = false;
    
    let cpu_affinity_manager = Arc::new(Mutex::new(super::cpu_affinity::CpuAffinityManager::new(args.value_of("affinity").unwrap())?));
    
    let display_json:bool;
    let display_bit:bool;
    match args.value_of("format").unwrap() {
        "json" => {
            display_json = true;
            display_bit = false;
        },
        "bit" => {
            display_json = false;
            display_bit = true;
        },
        "byte" => {
            display_json = false;
            display_bit = false;
        },
        _ => {
            log::error!("unsupported display-mode; defaulting to JSON");
            display_json = true;
            display_bit = false;
        },
    }
    
    let ip_version:u8;
    if args.is_present("version6") {
        ip_version = 6;
    } else {
        ip_version = 4;
    }
    let port:u16 = args.value_of("port").unwrap().parse()?;
    let server_address = args.value_of("client").unwrap();
    
    let test_id = uuid::Uuid::new_v4();
    
    let mut upload_config = prepare_upload_config(&args, test_id.as_bytes())?;
    let download_config = prepare_download_config(&args, test_id.as_bytes())?;
    
    
    log::info!("connecting to server at {}:{}...", server_address, port);
    let stream_result = TcpStream::connect(&format!("{}:{}", server_address, port).parse()?);
    if stream_result.is_err() {
        return Err(Box::new(simple_error::simple_error!("unable to connect: {}", stream_result.unwrap_err())));
    }
    let mut stream = stream_result.unwrap();
    log::info!("connected to server");
    
    stream.set_nodelay(true).expect("cannot disable Nagle's algorithm");
    stream.set_keepalive(Some(KEEPALIVE_DURATION)).expect("unable to set TCP keepalive");
    
    let stream_count = download_config.get("streams").unwrap().as_i64().unwrap() as usize;
    let mut parallel_streams:Vec<Arc<Mutex<(dyn TestStream + Sync + Send)>>> = Vec::with_capacity(stream_count);
    let mut parallel_streams_joinhandles = Vec::with_capacity(stream_count);
    
    let test_results:Mutex<Box<dyn crate::protocol::results::TestResults>>;
    if args.is_present("udp") {
        let mut udp_test_results = crate::protocol::results::UdpTestResults::new();
        for i in 0..stream_count {
            udp_test_results.prepare_index(&(i as u8));
        }
        test_results = Mutex::new(Box::new(udp_test_results));
    } else { //TCP
        //FIXME: needs to be TCP
        test_results = Mutex::new(Box::new(crate::protocol::results::UdpTestResults::new()));
    }
    
    let (results_tx, results_rx):(std::sync::mpsc::Sender<Box<dyn IntervalResult + Sync + Send>>, std::sync::mpsc::Receiver<Box<dyn IntervalResult + Sync + Send>>) = channel();
    
    let mut results_handler = || -> BoxResult<()> {
        loop {
            match results_rx.try_recv() {
                Ok(result) => {
                    if !display_json {
                        println!("{}", result.to_string(display_bit));
                    }
                    
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
    
    
    if args.is_present("reverse") {
        log::debug!("running in reverse-mode: server will be uploading data");
        
        let mut stream_ports = Vec::new();
        
        if args.is_present("udp") {
            let test_definition = udp::build_udp_test_definition(&download_config)?;
            for i in 0..stream_count {
                let test = udp::receiver::UdpReceiver::new(test_definition.clone(), &(i as u8), &ip_version, &0)?;
                stream_ports.push(test.get_port()?);
                parallel_streams.push(Arc::new(Mutex::new(test)));
            }
        } else { //TCP
            
        }
        
        upload_config["streamPorts"] = serde_json::json!(stream_ports);
        
        send(&mut stream, &upload_config)?;
    } else {
        log::debug!("running in forward-mode: server will be receiving data");
        
        send(&mut stream, &download_config)?;
        //NOTE: we don't prepare to send data at this point; that happens in the loop below, after the server signals that it's ready
    }
    
    let connection_payload = receive(&mut stream, is_alive, &mut results_handler)?;
    match connection_payload.get("kind") {
        Some(kind) => {
            match kind.as_str().unwrap_or_default() {
                "connect" => { //we need to connect to the server
                    if args.is_present("udp") {
                        let test_definition = udp::build_udp_test_definition(&upload_config)?;
                        for (i, port) in connection_payload.get("streamPorts").unwrap().as_array().unwrap().iter().enumerate() {
                            let test = udp::sender::UdpSender::new(
                                test_definition.clone(), &(i as u8),
                                &ip_version, &0, server_address.to_string(), &(port.as_i64().unwrap() as u16),
                                &(upload_config["duration"].as_f64().unwrap() as f32),
                                &(upload_config["sendInterval"].as_f64().unwrap() as f32),
                            )?;
                            parallel_streams.push(Arc::new(Mutex::new(test)));
                        }
                    } else { //TCP
                        
                    }
                },
                "connected" => { //server has connected to us
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
    
    if is_alive() {
        //tell the server to start
        send(&mut stream, &prepare_begin())?;
        
        log::debug!("spawning stream-threads");
        //begin the test-streams
        for parallel_stream in parallel_streams.iter_mut() {
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
    
    //assume this is a controlled shutdown
    send(&mut stream, &prepare_end()).unwrap_or_default();
    thread::sleep(Duration::from_millis(500)); //wait a moment for the shutdown to finish cleanly
    stream.shutdown(Shutdown::Both).unwrap_or_default();
    
    //ensure everything has ended
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
    for jh in parallel_streams_joinhandles {
        match jh.join() {
            Ok(_) => (),
            Err(e) => log::error!("error in parallel stream: {:?}", e),
        }
    }
    
    {
        let tr = test_results.lock().unwrap();
        if display_json {
            println!("{}", tr.to_json_string());
        } else {
            println!("{}", tr.to_string(display_bit));
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
