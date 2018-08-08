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
extern crate tempfile;


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
    use tokio::runtime::Builder;
    use tokio::executor::thread_pool;
    use {Client, Server};

    #[test]
    fn it_works() {
        let mut threadpool_builder = thread_pool::Builder::new();
        threadpool_builder
            .name_prefix("my-runtime-worker-")
            .pool_size(4);
        
        // build Runtime
        let mut runtime = Builder::new()
            .threadpool_builder(threadpool_builder)
            .build().unwrap();
        

        let path = format!("{}rust-daemon.sock", env::temp_dir().to_str().unwrap());
        println!("[TEST] Socket path: {}", path);

        println!("[TEST] Creating server");
        let server = Server::new(&mut runtime, &path).unwrap();

        println!("[TEST] Creating client");
        let client = Client::new(&path).unwrap();

        println!("[TEST] Awaiting connect");
        let server_handle = server.incoming().for_each(move |d| {
            println!("server incoming: {:?}", d);
            Ok(())
        }).map_err(|_e| () );
        runtime.spawn(server_handle);

        let (tx, rx) = client.split();

        let out = "abcd1234\n";
        println!("[TEST] Writing Data");
        tx.send(BytesMut::from(out)).wait().unwrap();

        println!("[TEST] Reading Data");
        let when = Instant::now() + Duration::from_secs(5);
        let client_handle = rx.for_each(move |d| {
            println!("client incoming: {:?}", d);
            assert_eq!(d, out.as_bytes());
            Ok(())
        }).map_err(|_e| () );
        runtime.spawn(client_handle);

        std::thread::sleep(Duration::from_secs(2));

        runtime.shutdown_now().wait().unwrap();
    }
}
