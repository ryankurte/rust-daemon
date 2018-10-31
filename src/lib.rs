/**
 * rust-daemon
 * Core module, re-exports client and server components
 *
 * https://github.com/ryankurte/rust-daemon
 * Copyright 2018 Ryan Kurte
 */
extern crate libc;
extern crate users;

extern crate futures;
extern crate bytes;

extern crate tokio;
use tokio::prelude::*;

extern crate tokio_io;
extern crate tokio_codec;
extern crate tokio_uds;
extern crate tokio_timer;

extern crate serde;
use serde::{Serialize, Deserialize};

extern crate serde_json;

#[cfg(test)]
#[macro_use]
extern crate serde_derive;

extern crate tokio_serde_json_mirror as tokio_serde_json;

#[macro_use]
extern crate log;
extern crate uuid;


/// Connection implements a network connection with a given codec
/// This is used to implement clients (ie. for a command line utility)
pub mod connection;
pub use connection::Connection;

/// Server implements a network server with connection management
/// This is used to implement daemon servers (ie. long running processes w/ network communication)
pub mod server;
pub use server::Server;

/// Error implements errors returned by the daemon
pub mod error;
pub use error::Error as DaemonError;

/// Codecs implement protocol handling over connectors
pub mod codecs;


/// JsonClient convenience wrapper to create clients with the JSON codec
pub type JsonClient<T, REQ, RESP> = Connection<T, codecs::json::JsonCodec<REQ, RESP, codecs::json::JsonError>>;

impl <T, REQ, RESP>JsonClient<T, REQ, RESP> 
where 
    T: AsyncWrite + AsyncRead + Send + 'static,
    REQ: Clone + Send + Serialize + 'static,
    for<'de> RESP: Clone + Send + Deserialize<'de> + 'static,
{
    pub fn new_json(stream: T) -> JsonClient<T, REQ, RESP> {
        Connection::<_, codecs::json::JsonCodec<REQ, RESP, codecs::json::JsonError>>::new(stream)
    }
}


mod user;
pub use user::User;

#[cfg(test)]
mod tests {
    use std::env;
    use std::thread::sleep;
    use std::time::{Duration};
    use tokio::prelude::*;
    use tokio::{run, spawn};
    use tokio_uds::UnixStream;

    use {Connection, Server};
    use codecs::json::{JsonCodec, JsonError};


    #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
    struct Test {
        text: String,
    }

    #[test]
    fn it_works() {
        let path = format!("{}/rust-daemon.sock", env::temp_dir().to_str().unwrap());
        println!("[TEST] Socket path: {}", path);

        let server_path = path.clone();
        let test = future::lazy(move || {
            println!("[TEST] Creating server");
            let mut server = Server::<_, JsonCodec<Test, Test, JsonError>>::new_unix(&server_path).unwrap();

            println!("[TEST] Awaiting connect");
            let server_handle = server.incoming().unwrap()
                .for_each(move |req| {
                    let data = req.data();
                    req.send(data).wait().unwrap();
                    Ok(())
                }).map_err(|_e| ());
            spawn(server_handle);

            println!("[TEST] Creating client");
            let stream = UnixStream::connect(path.clone()).wait().unwrap();
            let client = Connection::<_, JsonCodec<Test, Test, JsonError>>::new(stream);

            let (tx, rx) = client.split();

            println!("[TEST] Writing Data");
            let out = Test {
                text: "test text".to_owned(),
            };
            tx.send(out.clone()).wait().unwrap();

            sleep(Duration::from_secs(2));

            println!("[TEST] Reading Data");
            rx.map(|d| -> Result<(), ()> {
                println!("client incoming: {:?}", d);
                assert_eq!(d, out);
                Ok(())
            }).wait()
            .next();

            server.close();

            Ok(())
        }).then(|_: Result<(), ()>| -> Result<(), ()> {
            println!("[TEST] Done");
            
            Ok(())
        });

        run(test);
    }
}
