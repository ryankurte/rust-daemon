# Rust Daemon

A library to simplify communication with daemons in rust, this uses [tokio]() to establish a type-safe [json]() over [unix socket]() interface for client-daemon communication, and is intended to be extended with other useful daemon-writing features as they are discovered.

This consists of a Server that handles Requests from and issues Responses to Clients, and a Client that issues Requests to and recieves Responses from a Server.

## Status

[![GitHub tag](https://img.shields.io/github/tag/ryankurte/daemon-core.svg)](https://github.com/ryankurte/daemon-core)
[![Build Status](https://travis-ci.com/ryankurte/rust-daemon.svg?branch=master)](https://travis-ci.com/ryankurte/rust-daemon)
[![Crates.io](https://img.shields.io/crates/v/daemon-core.svg)](https://crates.io/crates/daemon-core)
[![Docs.rs](https://docs.rs/daemon-core/badge.svg)](https://docs.rs/daemon-core)


## Usage

See [src/examples/server.rs](src/examples/server.rs) for an example server, and [src/examples/client.rs](src/examples/client.rs) for an example client.

### Client
```rust
// Create client instance
let client = Client::<_, Request, Response>::new(addr).unwrap();
// Split RX and TX
let (tx, rx) = client.split();
// Send something (remember to .wait())
tx.send(Request::Something).wait().unwrap();
// Receive something (also remember to wait)
rx.map(|resp| -> Result<(), DaemonError> {
    println!("Response: {:?}", resp);
    Ok(())
}).wait()
    .next();
```

### Server
```rust
// Create server instance
let s = Server::<Request, Response>::new(&addr).unwrap();

// Handle requests from clients
let server_handle =
    s.incoming*(.for_each(move |r| {
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

// Create server task
tokio::spawn(server_handle);
``


------

If you have any questions, comments, or suggestions, feel free to open an issue or a pull request.