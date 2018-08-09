#![feature(extern_prelude)]

extern crate libc;
extern crate users;

extern crate futures;

extern crate tokio;
extern crate tokio_codec;
extern crate tokio_io;
extern crate tokio_uds;

extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate tokio_serde_json;

extern crate bytes;
extern crate tempfile;
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
    use bytes::BytesMut;
    use std::env;
    use std::time::{Duration, Instant};
    use tokio::executor::thread_pool;
    use tokio::prelude::*;
    use tokio::runtime::Builder;
    use {Client, Server};

    #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
    struct Test {
        text: String,
    }

    #[test]
    fn it_works() {
        let mut threadpool_builder = thread_pool::Builder::new();
        threadpool_builder
            .name_prefix("my-runtime-worker-")
            .pool_size(4);

        // build Runtime
        let mut runtime = Builder::new()
            .threadpool_builder(threadpool_builder)
            .build()
            .unwrap();

        let path = format!("{}rust-daemon.sock", env::temp_dir().to_str().unwrap());
        println!("[TEST] Socket path: {}", path);

        println!("[TEST] Creating server");
        let server = Server::<Test, Test>::new(&mut runtime, &path).unwrap();

        println!("[TEST] Awaiting connect");
        let server_handle = server
            .for_each(move |mut r| {
                let data = r.data();
                println!("server incoming: {:?}", data);
                r.send(data);
                Ok(())
            })
            .map_err(|_e| ());
        runtime.spawn(server_handle);

        println!("[TEST] Creating client");
        let client = Client::<Test, Test>::new(&path).unwrap();

        runtime.spawn(future::lazy(move || {
            println!("[TEST] Writing Data");
            let out = Test {
                text: "test text".to_owned(),
            };
            client.clone().send(out.clone()).wait().unwrap();

            std::thread::sleep(Duration::from_secs(2));

            println!("[TEST] Reading Data");
            let client_handle = client
                .for_each(move |d| {
                    println!("client incoming: {:?}", d);
                    assert_eq!(d, out);
                    Ok(())
                })
                .map_err(|_e| ());
            tokio::spawn(client_handle);

            Ok(())
        }));

        std::thread::sleep(Duration::from_secs(2));
    }
}
