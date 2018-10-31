# Rust Daemon

A library to simplify communication with daemons in rust, with the goal of hiding as much of the complexity of / minimizing the effort required to use tokio as much as possible.

This consists of a higher level [server]() and [connection]() object to provide typed communication between components, implemented using [tokio](https://github.com/tokio-rs/tokio) using [codecs](https://docs.rs/tokio/0.1.12/tokio/codec/index.html) .

These types objects can be created using any [`AsyncRead`](https://docs.rs/tokio/0.1.12/tokio/prelude/trait.AsyncRead.html) and [`AsyncWrite'](https://docs.rs/tokio/0.1.12/tokio/prelude/trait.AsyncWrite.html) compatible types, for example, [`TCPStream`](https://docs.rs/tokio/0.1.12/tokio/net/struct.TcpStream.html) and [`UnixStream`](https://docs.rs/tokio/0.1.12/tokio/net/struct.UnixStream.html).


A generic [example codec](src/codecs/json.rs) is provided using [serde](https://serde.rs/) and [serde_json](https://github.com/serde-rs/json) to establish a type-safe json interface for client-daemon communication, when using this codec the `ENC` and `DEC` types must implement [serde](https://serde.rs/) `Serialize` and `Deserialize` traits, these may be implemented using [serde_derive](https://serde.rs/derive.html). It is intended that additional codecs will be added as they are required.


## Status

[![GitHub tag](https://img.shields.io/github/tag/ryankurte/daemon-engine.svg)](https://github.com/ryankurte/daemon-engine)
[![Build Status](https://travis-ci.com/ryankurte/rust-daemon.svg?branch=master)](https://travis-ci.com/ryankurte/rust-daemon)
[![Crates.io](https://img.shields.io/crates/v/daemon-engine.svg)](https://crates.io/crates/daemon-engine)
[![Docs.rs](https://docs.rs/daemon-engine/badge.svg)](https://docs.rs/daemon-engine)

[Open Issues](https://github.com/ryankurte/rust-daemon/issues)


## Usage

See [src/examples/server.rs](src/examples/server.rs) and [src/examples/connection.rs](src/examples/client.rs) for an example server and client implementing a simple key-value store.

You can build these examples with `cargo build --features examples`, run the server with `./targets/debug/rustd-server` and interact using `./targets/debug/rustd-client`. `rustd-client -k KEY` fetches the value for a given key, `rustd-client -k KEY -v VALUE` sets the value of the given key, and `rustd-client --help` will display available arguments.


### Client
```rust
extern crate daemon_engine;
use daemon_engine::Connection;

...

// Create client instance
let stream = UnixStream::connect(path.clone()).wait().unwrap();
let client = Connection::<_, JsonCodec<Test, Test, JsonError>>::new(stream);

// Split RX and TX
let (tx, rx) = client.split();

// Send something (remember to .wait())
tx.send(Request::Something).wait().unwrap();

// Receive something (also remember to wait)
rx.map(|resp| -> Result<(), DaemonError> {
    println!("Response: {:?}", resp);
    Ok(())
}).wait().next();
```

### Server
```rust
extern crate tokio;
use tokio::prelude::*;
use tokio::{run, spawn};

extern crate daemon_engine;
use daemon_engine::Server;
use daemon_engine::codecs::json::{JsonCodec, JsonError};

...

let server_handle = future::lazy(move || {
    // Create server instance using the JSON codec, this must be executed from within a tokio context
    let mut server = Server::<_, JsonCodec<Request, Response, JsonError>>::new_unix(&server_path).unwrap();

    // Handle requests from clients
    s.incoming().unwrap().for_each(move |r| {
        println!("Request: {:?}", r.data());
        let data = r.data();
        match data {
            ...
            _ => {
                r.send(Response::Something(v.to_string()))
            }
        // Remember you have to .wait or otherwise prompt for send to occur
        }.wait().unwrap();
        Ok(())
    }).map_err(|_e| ());

    // do more stuff
    ...

    // Close the server when you're done
    s.close();

    Ok(())
});

// Create server task
tokio::run(server_handle);
``


------

If you have any questions, comments, or suggestions, feel free to open an issue or a pull request.