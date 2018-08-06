#![feature(extern_prelude)]

extern crate libc;
extern crate users;

extern crate futures;

extern crate tokio;
extern crate tokio_uds;
extern crate tokio_codec;
extern crate tokio_io;

extern crate serde;
extern crate serde_json;
//#[macro_use]
//extern crate serde_derive;
//extern crate tokio_serde_json;

extern crate bytes;
extern crate uuid;


pub mod client;
pub use client::Client;

pub mod server;
pub use server::Server;

pub mod error;
pub use error::DaemonError;

mod user;
pub use user::User;

#[cfg(test)]
mod tests {
    use std::env;
    use tokio::prelude::*;
    use bytes::BytesMut;
    use std::time::{Duration, Instant};
    use tokio::runtime::current_thread::Runtime as LocalRuntime;
    use tokio::runtime::Runtime;
    use {Client, Server};

    #[test]
    fn it_works() {
        let mut runtime = Runtime::new().unwrap();

        let socket = format!("{}rust-daemon.sock", env::temp_dir().to_str().unwrap());
        println!("[TEST] Socket: {}", socket);

        println!("[TEST] Creating server");
        let server = Server::new(&mut runtime, &socket).unwrap();

        println!("[TEST] Creating client");
        let client = Client::new(&socket).unwrap();

        println!("[TEST] Awaiting connect");
        let server_handle = server.incoming().for_each(move |d| {
            println!("server incoming: {:?}", d);
            Ok(())
        }).map_err(|_e| () );
        runtime.spawn(server_handle);

        let (tx, rx) = client.split();

        println!("[TEST] Writing Data");
        let out = "abcd1234\n";
        tx.send(BytesMut::from(out)).wait().unwrap();

        println!("[TEST] Reading Data");
        let when = Instant::now() + Duration::from_secs(5);

        let client_handle = rx.for_each(move |d| {
            println!("client incoming: {:?}", d);
            assert_eq!(d, out.as_bytes());
            Ok(())
        }).map_err(|_e| () );
        runtime.spawn(client_handle);

        assert!(false);

        runtime.shutdown_now().wait().unwrap();
    }
}
