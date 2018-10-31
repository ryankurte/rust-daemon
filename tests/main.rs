/**
 * rust-daemon
 * Integration test / example
 *
 * https://github.com/ryankurte/rust-daemon
 * Copyright 2018 Ryan Kurte
 */

use std::env;
use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use std::thread::sleep;
use std::time::{Duration};

extern crate tokio;
use tokio::prelude::*;
use tokio::{run, spawn};

extern crate tokio_uds;
use tokio_uds::UnixStream;

extern crate tokio_tcp;
use tokio_tcp::TcpStream;

#[macro_use]
extern crate serde_derive;

extern crate daemon_engine;
use daemon_engine::{Connection, Server};
use daemon_engine::codecs::json::{JsonCodec, JsonError};


#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct Test {
    text: String,
}

// These tests are disabled as they are unreliable on test infrastructure :-(
// TODO: work out why and fix it
#[cfg(e2e_tests)]
#[test]
fn client_server_unix() {
    let path = format!("{}rust-daemon.sock", env::temp_dir().to_str().unwrap());
    println!("[TEST UNIX] socket path: {}", path);

    let server_path = path.clone();
    let test = future::lazy(move || {
        println!("[TEST UNIX] Creating server");
        let mut server = Server::<_, JsonCodec<Test, Test, JsonError>>::new_unix(&server_path).unwrap();

        println!("[TEST UNIX] Awaiting connect");
        let server_handle = server.incoming().unwrap()
            .for_each(move |req| {
                let data = req.data();
                req.send(data).wait().unwrap();
                Ok(())
            }).map_err(|_e| ());
        spawn(server_handle);

        println!("[TEST UNIX] Creating client");
        let stream = UnixStream::connect(path.clone()).wait().unwrap();
        let client = Connection::<_, JsonCodec<Test, Test, JsonError>>::new(stream);

        let (tx, rx) = client.split();

        println!("[TEST UNIX] Writing Data");
        let out = Test {
            text: "test text".to_owned(),
        };
        tx.send(out.clone()).wait().unwrap();

        sleep(Duration::from_secs(2));

        println!("[TEST UNIX] Reading Data");
        rx.map(|d| -> Result<(), ()> {
            println!("[TEST UNIX] Received: {:?}", d);
            assert_eq!(d, out);
            Ok(())
        }).wait()
        .next();

        server.close();

        Ok(())
    }).then(|_: Result<(), ()>| -> Result<(), ()> {
        println!("[TEST UNIX] Done");

        Ok(())
    });

    run(test);
}

// These tests are disabled as they are unreliable on test infrastructure :-(
// TODO: work out why and fix it
#[cfg(e2e_tests)]
#[test]
fn client_server_tcp() {
    let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 8111);
    println!("[TEST TCP] socket bind address: {}", socket);

    let test = future::lazy(move || {
        println!("[TEST TCP] Creating server");
        let mut server = Server::<_, JsonCodec<Test, Test, JsonError>>::new_tcp(&socket).unwrap();

        println!("[TEST TCP] Awaiting connect");
        let server_handle = server.incoming().unwrap()
            .for_each(move |req| {
                let data = req.data();
                req.send(data).wait().unwrap();
                Ok(())
            }).map_err(|_e| ());
        spawn(server_handle);

        println!("[TEST TCP] Creating client");
        let stream = TcpStream::connect(&socket.clone()).wait().unwrap();
        let client = Connection::<_, JsonCodec<Test, Test, JsonError>>::new(stream);

        let (tx, rx) = client.split();

        println!("[TEST TCP] Writing Data");
        let out = Test {
            text: "test text".to_owned(),
        };
        tx.send(out.clone()).wait().unwrap();

        sleep(Duration::from_secs(2));

        println!("[TEST TCP] Reading Data");
        rx.map(|d| -> Result<(), ()> {
            println!("[TEST TCP] Received: {:?}", d);
            assert_eq!(d, out);
            Ok(())
        }).wait()
        .next();

        server.close();

        Ok(())
    }).then(|_: Result<(), ()>| -> Result<(), ()> {
        println!("[TEST TCP] Done");

        Ok(())
    });

    run(test);
}