extern crate log;

use std::error::Error;

type BoxResult<T> = Result<T,Box<dyn Error>>;


//if the server is uploading, each of its iterators will be capped with a "done" signal, which sets a flag in the local iteration results
//if we're uploading, send a "done" to the server under the same conditions, which it will match with its own "done", which we use to update our local state
//for UDP, this is a packet containing only the test ID, 16 bytes in length
//for TCP, it's just closing the stream





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

        
        
*/
