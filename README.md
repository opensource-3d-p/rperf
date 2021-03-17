# rperf

rperf is a Rust-based iperf clone, aiming to avoid some reliability and
consistency issues found in iperf3, while simultaneously providing richer
metrics data, with a focus on operation in a more IoT-like environment.


## theory of operation




### implementation details

The client-server relationship is treated as a very contral aspect of this
design:

- all data-formatting and display happens on the client side, with the server
  being intended for use as a service, rather than a peer process.

The server uses three layers of threading: one for the main thread, one for each
client being served, and one more for each stream from the client. On the client
side, the main thread is used to communicate with the server and it spawns an
additional thread for each stream.

When the server receives a request from a client, it spawns a thread that
handles that client's specific request; internally, each stream for the test
produces an iterator, which emits JSON-serialised data at every specified update
interval, which is sent back to the client for processing. The iterator is
exhausted when either the test concludes or it encounters an error, such as a
period of time without receiving a TCP ACK or some other feedback from its peer.
