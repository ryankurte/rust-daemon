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

extern crate tokio;
extern crate tokio_codec;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_uds;
extern crate tokio_timer;

extern crate serde;
extern crate serde_json;

#[cfg(test)]
#[macro_use]
extern crate serde_derive;

extern crate tokio_serde_json;

#[macro_use]
extern crate log;
extern crate bytes;
extern crate tempfile;
extern crate uuid;

/// Client implements a daemon client (ie. command line utility)
pub mod client;
pub use client::Client;

/// Server implements a daemon server (ie. long running process)
pub mod server;
pub use server::Server;

/// Error implements errors returned by the daemon
pub mod error;
pub use error::DaemonError;

mod user;
pub use user::User;

#[cfg(test)]
mod tests {
    use std::env;
    use std::thread::sleep;
    use std::time::{Duration, Instant};
    use tokio::prelude::*;
    use tokio::{run, spawn};
    use {Client, Server};

    #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
    struct Test {
        text: String,
    }

    // TODO: re-enable when exiting is a viable option I guess?
    //#[test]
    fn it_works() {
        let path = format!("{}/rust-daemon.sock", env::temp_dir().to_str().unwrap());
        println!("[TEST] Socket path: {}", path);

        let server_path = path.clone();
        let test = future::lazy(move || {
            println!("[TEST] Creating server");
            let server = Server::<Test, Test>::new(server_path).unwrap();

            println!("[TEST] Awaiting connect");
            let server_handle = server
                .incoming()
                .for_each(move |r| {
                    let data = r.data();
                    println!("server incoming: {:?}", data);
                    r.send(data).wait().unwrap();
                    Ok(())
                }).map_err(|_e| ());
            spawn(server_handle);

            println!("[TEST] Creating client");
            let client = Client::<_, Test, Test>::new(path.clone()).unwrap();

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

            Ok(())
        }).then(|_: Result<(), ()>| -> Result<(), ()> {
            println!("[TEST] Done");
            
            Ok(())
        });

        // TODO: this needs to timeout somehow / finish somehow

        run(test);
    }
}
