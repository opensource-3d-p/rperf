use std::error::Error;
use std::net::{Shutdown};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use clap::ArgMatches;

use mio::net::{TcpStream};

use crate::protocol::communication::{receive, send, KEEPALIVE_DURATION};

use crate::protocol::messaging::{
    prepare_begin, prepare_end,
    prepare_configuration_udp_upload, prepare_configuration_udp_download,
};

use crate::stream::TestStream;
use crate::stream::tcp;
use crate::stream::udp;

type BoxResult<T> = Result<T,Box<dyn Error>>;

static ALIVE:AtomicBool = AtomicBool::new(true);

fn prepare_upload_config(args:&ArgMatches, test_id:&[u8; 16]) -> BoxResult<serde_json::Value> {
    let parallel_streams:u8 = args.value_of("parallel").unwrap().parse()?;
    let bandwidth:u64 = args.value_of("bandwidth").unwrap().parse()?;
    let bytes:u64 = args.value_of("bytes").unwrap().parse()?;
    let seconds:f32 = args.value_of("time").unwrap().parse()?;
    let length:u32 = args.value_of("length").unwrap().parse()?;
    let send_interval:f32 = args.value_of("sendinterval").unwrap().parse()?;
    
    if args.is_present("udp") {
        log::debug!("preparing UDP download config");
        Ok(prepare_configuration_udp_upload(test_id, parallel_streams, bandwidth, bytes, seconds, length as u16, send_interval))
    } else {
        log::debug!("preparing TCP download config");
        Ok(serde_json::json!({}))
    }
}
fn prepare_download_config(args:&ArgMatches, test_id:&[u8; 16]) -> BoxResult<serde_json::Value> {
    let parallel_streams:u8 = args.value_of("parallel").unwrap().parse()?;
    let length:u32 = args.value_of("length").unwrap().parse()?;
    
    if args.is_present("udp") {
        log::debug!("preparing UDP download config");
        Ok(prepare_configuration_udp_download(test_id, parallel_streams, length as u16))
    } else {
        log::debug!("preparing TCP download config");
        Ok(serde_json::json!({}))
    }
}
            

pub fn execute(args:ArgMatches) -> BoxResult<()> {
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
        return Err(Box::new(simple_error::simple_error!(format!("unable to connect: {:?}", stream_result.unwrap_err()).as_str())));
    }
    let mut stream = stream_result.unwrap();
    log::info!("connected to server");
    
    stream.set_nodelay(true).expect("cannot disable Nagle's algorithm");
    stream.set_keepalive(Some(KEEPALIVE_DURATION)).expect("unable to set TCP keepalive");
    
    let mut parallel_streams:Vec<Arc<Mutex<(dyn TestStream + Sync + Send)>>> = Vec::new();
    let mut parallel_streams_joinhandles = Vec::new();
    
    if args.is_present("reverse") {
        log::debug!("running in reverse-mode: server will be uploading data");
        
        let mut stream_ports = Vec::new();
        
        if args.is_present("udp") {
            let test_definition = udp::build_udp_test_definition(&download_config)?;
            for i in 0..(download_config.get("streams").unwrap().as_i64().unwrap()) {
                let test = udp::receiver::UdpReceiver::new(test_definition.clone(), &ip_version, &0)?;
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
    
    //TODO: prepare the display/result-processing thread
    
    let connection_payload = receive(&mut stream, is_alive)?;
    match connection_payload.get("kind") {
        Some(kind) => {
            match kind.as_str().unwrap_or_default() {
                "connect" => { //we need to connect to the server
                    if args.is_present("udp") {
                        let test_definition = udp::build_udp_test_definition(&upload_config)?;
                        for (i, port) in connection_payload.get("streamPorts").unwrap().as_array().unwrap().iter().enumerate() {
                            let test = udp::sender::UdpSender::new(
                                test_definition.clone(),
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
                    log::error!("invalid data from {}", stream.peer_addr()?);
                    kill();
                },
            }
        },
        None => {
            log::error!("invalid data from {}", stream.peer_addr()?);
            kill();
        },
    }
    
    if is_alive() {
        //tell the server to start
        send(&mut stream, &prepare_begin())?;
        
        //begin the test-streams
        for parallel_stream in parallel_streams.iter_mut() {
            let c_ps = Arc::clone(&parallel_stream);
            let handle = thread::spawn(move || {
                loop {
                    match c_ps.lock().unwrap().run_interval() {
                        Some(interval_result) => {
                            //write the result into an std::sync::mpsc instance, which another thread will harvest and sort as needed
                        },
                        None => break,
                    }
                }
            });
            parallel_streams_joinhandles.push(handle);
        }
        
        //watch for events from the server
        while is_alive() {
            let payload = receive(&mut stream, is_alive)?;
            
            match payload.get("kind") {
                Some(kind) => {
                    match kind.as_str().unwrap_or_default() {
                        "result" => { //result from a test
                            //...
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
            
            
            //after that response is received, send a message to begin; once that message has been sent, tell all of the threads to begin iterating,
            //with a callback function to update the execution data (this is passed back through a queue to prevent them from blocking)
            //if output is non-JSON, this thread is responsible for not just updating the structures, but also formatting and presentation
            
            //std::sync::mpsc
            
            //every subsequent message from the server will be one of its iteration results; when received, treat them the same way as the local
            //iteration results
            
            //if the server is uploading, each of its iterators will be capped with a "done" signal, which sets a flag in the local iteration results
            //if we're uploading, send a "done" to the server under the same conditions, which it will match with its own "done", which we use to update our local state
            //for UDP, this is a packet containing only the test ID, 16 bytes in length
            //for TCP, it's just closing the stream
            
            //when all streams have finished, send an "end" message to the server
            //then break, since everything is synced
        }
    }
    
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
    
    //TODO: display final results
    //this will probably just be joining on the display thread
    
    Ok(())
}

pub fn kill() -> bool {
    ALIVE.swap(false, Ordering::Relaxed)
}
fn is_alive() -> bool {
    ALIVE.load(Ordering::Relaxed)
}




/*


        
        .arg(
            Arg::with_name("omit")
                .help("omit a number of seconds from the start of calculations, in non-JSON modes, to avoid including TCP ramp-up in averages")
                .takes_value(true)
                .long("omit")
                .short("O")
                .default_value("0.0")
                .required(false)
        )
        
        
        .arg(
            Arg::with_name("window")
                .help("window-size, in bytes, for TCP tests")
                .takes_value(false)
                .long("window")
                .short("w")
                .required(false)
        )
        .arg(
            Arg::with_name("mss")
                .help("maximum segment-size, for TCP tests (default is based on MTU)")
                .takes_value(false)
                .long("mss")
                .short("M")
                .required(false)
        )
        .arg(
            Arg::with_name("nodelay")
                .help("use no-delay mode for TCP tests, deisabling Nagle's Algorithm")
                .takes_value(false)
                .long("no-delay")
                .short("N")
                .required(false)
        )
        .arg(
            Arg::with_name("congestion")
                .help("use a specific congestion-control algorithm for traffic-shaping")
                .takes_value(false)
                .long("congestion")
                .short("C")
                .required(false)
        )
        
        
*/
