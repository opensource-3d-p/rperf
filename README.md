# rperf

_rperf_ is a Rust-based _iperf_ clone, aiming to avoid some reliability and
consistency issues found in _iperf3_, while simultaneously providing richer
metrics data, with a focus on operation in a loss-tolerant, more IoT-like
environment. While it can be used as a near-drop-in replacement for _iperf_,
and there may be benefits to doing so, its focus is on periodic data-collection
in a monitoring capacity.


## usage

_rperf_ works much like _iperf3_, sharing a lot of concepts and even
command-line flags. One key area where it differs is that the client drives
all of the configuration process while the server just complies to the best
of its ability and provides a stream of results. This means that the server
will not present test-results directly and also that TCP and UDP tests can
be run against the same instance, potentially by many clients in parallel.

In its normal mode of operation, the client will upload data to the server;
when the `reverse` flag is set, the client will receive data.

Unlike _iperf3_, _rperf_ does not make use of a reserved port-range. This is
so it can support an arbitrary number of clients in parallel without
resource contention on what can only practically be a small number of
contiguous ports. In its intended capacity, this shouldn't be a problem, but
it does make `reverse` incompatible with most non-permissive firewalls and
NAT setups.

There also isn't a concept of testing throughput relative to a fixed quantity
of data. Rather, the sole focus is on measuring throughput over a roughly
known period of time.

Also of relevance is that, if the server is running in IPv6 mode and its
host supports IPv4-mapping in a dual-stack configuration, both IPv4 and IPv6
clients can connect to the same instance.


## building

_rperf_ uses _cargo_. The typical process will be `cargo build --release`. The
resulting binary, at `target/release/rperf`, will still be reasonably large,
but it can be stripped to a much more portable size.


## theory of operation

Like its contemporaries, _rperf_'s core concept is firing a stream of TCP or
UDP data at an IP target at a pre-arranged target speed. The amount of data
actually received is observed and used to gauge the capacity of a network link.

Within those domains, additional data about the quality of the exchange is
gathered and made available for review.

Architecturally, _rperf_ has clients establish a TCP connection to the server,
after which the client sends details about the test to be performed and the
server obliges, sending observation results to the client during the entire
testing process.

The client may request that multiple parallel streams be used for testing, which
is facilitated by establishing multiple TCP connections or UDP sockets with
their own dedicated thread on either side, which may be further pinned to a
single logical CPU core to reduce the impact of page-faults on the
data-exchange.


### implementation details

The client-server relationship is treated as a very central aspect of this
design, in contrast to _iperf3_ where they're more like peers and _zapwireless_
where each participant runs its own daemon and a third process orchestrates
communication.

Notably, all data-gathering, calculation, and display happens client-side, with
the server simply returning what it observed. This can lead to some drift in
recordings, particularly where time is concerned (server intervals being a
handful of milliseconds longer than their corresponding client values is not
at all uncommon). Assuming the connection wasn't lost, however, totals for data
observed will match up in all modes of operation.

The server uses three layers of threading: one for the main thread, one for each
client being served, and one more for each stream that communicates with the
client. On the client side, the main thread is used to communicate with the
server and it spawns an additional thread for each stream that communicates with
the server.

When the server receives a request from a client, it spawns a thread that
handles that client's specific request; internally, each stream for the test
produces an iterator-like handler on either side. Both the client and server run
these iterator-analogues against each other asynchronously until the test period
ends, at which point the sender indicates completion within its stream.

To reliably handle the possibility of disconnects at the stream level, a
keepalive mechanism in the client-server stream, over which test-results are
sent from the server at regular intervals, will terminate outstanding
connections after a few seconds of inactivity.

The host OS's TCP and UDP mechanisms are used for all actual traffic exchanged,
with some tuning parameters exposed. This approach was chosen over a userspace
implementation on top of layer-2 or layer-3 because it most accurately
represents the way real-world applications will behave.
