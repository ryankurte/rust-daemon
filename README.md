# Rust Daemon

A library to simplify writing daemons in rust, this uses [tokio]() to establish a type-safe [json]() over [unix socket]() interface.

## Status

WIP

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