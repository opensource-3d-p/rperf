/// rperf, validates network throughput capacity and reliability,
/// https://github.com/opensource-3d-p/rperf
#[derive(clap::Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// the port used for client-server interactions
    #[arg(short, long, value_name = "number", default_value_t = 5199)]
    pub port: u16,

    /// specify logical CPUs, delimited by commas, across which to round-robin affinity;
    /// not supported on all systems
    #[arg(short = 'A', long, value_name = "numbers", default_value = "")]
    pub affinity: String,

    /// bind to the interface associated with the address <host>
    #[arg(
        short = 'B',
        long,
        conflicts_with = "client",
        default_value_if("version6", "true", Some("::")),
        default_value = "0.0.0.0",
        value_name = "host"
    )]
    pub bind: std::net::IpAddr,

    /// emit debug-level logging on stderr; default is info and above
    #[arg(short, long)]
    pub debug: bool,

    /// run in server mode
    #[arg(short, long, conflicts_with = "client")]
    pub server: bool,

    /// enable IPv6 on the server (on most hosts, this will allow both IPv4 and IPv6,
    /// but it might limit to just IPv6 on some)
    #[arg(short = '6', long, conflicts_with = "client")]
    pub version6: bool,

    /// limit the number of concurrent clients that can be processed by a server;
    /// any over this count will be immediately disconnected
    #[arg(long, value_name = "number", default_value = "0", conflicts_with = "client")]
    pub client_limit: usize,

    /// run in client mode; value is the server's address
    #[arg(short, long, value_name = "host", conflicts_with = "server")]
    pub client: Option<String>,

    /// run in reverse-mode (server sends, client receives)
    #[arg(short = 'R', long, conflicts_with = "server")]
    pub reverse: bool,

    /// the format in which to deplay information (json, megabit/sec, megabyte/sec)
    #[arg(
        short,
        long,
        value_enum,
        value_name = "format",
        default_value = "megabit",
        conflicts_with = "server"
    )]
    pub format: Format,

    /// use UDP rather than TCP
    #[arg(short, long, conflicts_with = "server")]
    pub udp: bool,

    /// target bandwidth in bytes/sec; this value is applied to each stream,
    /// with a default target of 1 megabit/second for all protocols (note: megabit, not mebibit);
    /// the suffixes kKmMgG can also be used for xbit and xbyte, respectively
    #[arg(short, long, default_value = "125000", value_name = "bytes/sec", conflicts_with = "server")]
    pub bandwidth: String,

    /// the time in seconds for which to transmit
    #[arg(short, long, default_value = "10.0", value_name = "seconds", conflicts_with = "server")]
    pub time: f64,

    /// the interval at which to send batches of data, in seconds, between [0.0 and 1.0);
    /// this is used to evenly spread packets out over time
    #[arg(long, default_value = "0.05", value_name = "seconds", conflicts_with = "server")]
    pub send_interval: f64,

    /// length of the buffer to exchange; for TCP, this defaults to 32 kibibytes; for UDP, it's 1024 bytes
    #[arg(
        short,
        long,
        conflicts_with = "server",
        default_value = "32768",
        default_value_if("udp", "true", Some("1024")),
        value_name = "bytes"
    )]
    pub length: usize,

    /// send buffer, in bytes (only supported on some platforms;
    /// if set too small, a 'resource unavailable' error may occur;
    /// affects UDP and TCP window-size)
    #[arg(long, default_value = "0", value_name = "bytes", conflicts_with = "server")]
    pub send_buffer: usize,

    /// receive buffer, in bytes (only supported on some platforms;
    /// if set too small, a 'resource unavailable' error may occur; affects UDP)
    #[arg(long, default_value = "0", value_name = "bytes", conflicts_with = "server")]
    pub receive_buffer: usize,

    /// the number of parallel data-streams to use
    #[arg(short = 'P', long, value_name = "number", default_value = "1", conflicts_with = "server")]
    pub parallel: usize,

    /// omit a number of seconds from the start of calculations,
    /// primarily to avoid including TCP ramp-up in averages;
    /// using this option may result in disagreement between bytes sent and received,
    /// since data can be in-flight across time-boundaries
    #[arg(short = 'O', long, default_value = "0", value_name = "seconds", conflicts_with = "server")]
    pub omit: usize,

    /// use no-delay mode for TCP tests, disabling Nagle's Algorithm
    #[arg(short = 'N', long, conflicts_with = "server")]
    pub no_delay: bool,

    /// an optional pool of IPv4 TCP ports over which data will be accepted;
    /// if omitted, any OS-assignable port is used; format: 1-10,19,21
    #[arg(long, value_name = "ports", default_value = "")]
    pub tcp_port_pool: String,

    /// an optional pool of IPv6 TCP ports over which data will be accepted;
    /// if omitted, any OS-assignable port is used; format: 1-10,19,21
    #[arg(long, value_name = "ports", default_value = "")]
    pub tcp6_port_pool: String,

    /// an optional pool of IPv4 UDP ports over which data will be accepted;
    /// if omitted, any OS-assignable port is used; format: 1-10,19,21
    #[arg(long, value_name = "ports", default_value = "")]
    pub udp_port_pool: String,

    /// an optional pool of IPv6 UDP ports over which data will be accepted;
    /// if omitted, any OS-assignable port is used; format: 1-10,19,21
    #[arg(long, value_name = "ports", default_value = "")]
    pub udp6_port_pool: String,

    /// Verbosity level
    #[arg(short, long, value_name = "level", value_enum, default_value = "info")]
    pub verbosity: ArgVerbosity,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, clap::ValueEnum, Default)]
pub enum Format {
    #[default]
    Json,
    Megabit,
    Megabyte,
}

impl std::fmt::Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Format::Json => write!(f, "json"),
            Format::Megabit => write!(f, "megabit/sec"),
            Format::Megabyte => write!(f, "megabyte/sec"),
        }
    }
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, clap::ValueEnum)]
pub enum ArgVerbosity {
    Off,
    Error,
    Warn,
    #[default]
    Info,
    Debug,
    Trace,
}

impl std::fmt::Display for ArgVerbosity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArgVerbosity::Off => write!(f, "off"),
            ArgVerbosity::Error => write!(f, "error"),
            ArgVerbosity::Warn => write!(f, "warn"),
            ArgVerbosity::Info => write!(f, "info"),
            ArgVerbosity::Debug => write!(f, "debug"),
            ArgVerbosity::Trace => write!(f, "trace"),
        }
    }
}
