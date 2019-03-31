/**
 * rust-daemon
 * Integration test / example
 *
 * https://github.com/ryankurte/rust-daemon
 * Copyright 2018 Ryan Kurte
 */

use std::env;
use std::net::{SocketAddr, IpAddr, Ipv4Addr};

extern crate tokio;
use tokio::prelude::*;
use tokio::{run, spawn};

#[macro_use]
extern crate serde_derive;

extern crate daemon_engine;
use daemon_engine::{UnixConnection, TcpConnection, UnixServer, TcpServer, JsonCodec};


#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct Test {
    text: String,
}

// This test is disabled as it is unreliable as heck
// TODO: work out why and fix it
#[test]
#[ignore]
fn client_server_unix() {
    let path = format!("{}rust-daemon.sock", env::temp_dir().to_str().unwrap());
    println!("[TEST UNIX] socket path: {}", path);

    let server_path = path.clone();
    let test = future::lazy(move || {
        println!("[TEST UNIX] Creating server");
        let mut server = UnixServer::<JsonCodec<Test, Test>>::new( &server_path, JsonCodec::new() ).unwrap();

        println!("[TEST UNIX] Awaiting connect");
        let server_handle = server.incoming().unwrap()
            .for_each(move |req| {
                let data = req.data();
                req.send(data).wait().unwrap();
                Ok(())
            }).map_err(|_e| ());
        spawn(server_handle);

        println!("[TEST UNIX] Creating client");
        let client = UnixConnection::<JsonCodec<Test, Test>>::new(&path, JsonCodec::new() ).wait().unwrap();

        let (tx, rx) = client.clone().split();

        println!("[TEST UNIX] Writing Data");
        let out = Test {
            text: "test text".to_owned(),
        };
        tx.send(out.clone()).wait().unwrap();

        println!("[TEST UNIX] Reading Data");
        rx.map(|d| -> Result<(), ()> {
            println!("[TEST UNIX] Received: {:?}", d);
            assert_eq!(d, out);
            Ok(())
        }).wait()
        .next();

        server.close();
        client.shutdown(); 

        Ok(())
    }).then(|_: Result<(), ()>| -> Result<(), ()> {
        println!("[TEST UNIX] Done");

        Ok(())
    });

    run(test);
}

// These tests are disabled as they are unreliable on test infrastructure :-(
// TODO: work out why and fix it
#[test]
#[ignore]
fn client_server_tcp() {
    let server_socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 8111);
    let client_socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8111);

    println!("[TEST TCP] socket bind address: {}", server_socket);

    let test = future::lazy(move || {
        println!("[TEST TCP] Creating server");
        let mut server = TcpServer::<JsonCodec<Test, Test>>::new( &server_socket, JsonCodec::new() ).unwrap();

        println!("[TEST TCP] Awaiting connect");
        let server_handle = server.incoming().unwrap()
            .for_each(move |req| {
                let data = req.data();
                req.send(data).wait().unwrap();
                Ok(())
            }).map_err(|_e| ());
        spawn(server_handle);

        println!("[TEST TCP] Creating client");
        let client = TcpConnection::<JsonCodec<Test, Test>>::new( &client_socket, JsonCodec::new() ).wait().unwrap();

        let (tx, rx) = client.clone().split();

        println!("[TEST TCP] Writing Data");
        let out = Test {
            text: "test text".to_owned(),
        };
        tx.send(out.clone()).wait().unwrap();

        println!("[TEST TCP] Reading Data");
        rx.map(|d| -> Result<(), ()> {
            println!("[TEST TCP] Received: {:?}", d);
            assert_eq!(d, out);
            Ok(())
        }).wait()
        .next();

        client.shutdown();
        server.close();

        Ok(())
    }).then(|_: Result<(), ()>| -> Result<(), ()> {
        println!("[TEST TCP] Done");

        Ok(())
    });

    run(test);
}
