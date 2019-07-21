/**
 * rust-daemon
 * Client example
 *
 * https://github.com/ryankurte/rust-daemon
 * Copyright 2018 Ryan Kurte
 */

#[macro_use]
extern crate clap;
use clap::{App, Arg};

extern crate tokio;
use tokio::prelude::*;

extern crate tokio_uds;

extern crate serde;
use serde::{Serialize, Deserialize};

extern crate daemon_engine;
use daemon_engine::{UnixConnection, DaemonError, JsonCodec};

/// Example request object
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Request {
    Get(String),
    Set(String, String),
}

/// Example response object
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Response {
    None,
    Value(String),
}


fn main() {
    let matches = App::new("rustd-client")
        .author("Ryan Kurte <diot@kurte.nz>")
        .about("rust-daemon example client")
        .version(crate_version!())
        .arg(
            Arg::with_name("Socket Address")
                .short("s")
                .long("socket-address")
                .help("Sets unix socket address")
                .takes_value(true)
                .default_value("/tmp/rustd.sock"),
        ).arg(
            Arg::with_name("Key")
                .short("k")
                .long("key")
                .help("key to set / get")
                .takes_value(true),
        ).arg(
            Arg::with_name("Value")
                .short("v")
                .long("value")
                .help("value to set")
                .takes_value(true),
        ).get_matches();

    // Parse arguments
    let addr = matches.value_of("Socket Address").unwrap().to_owned();
    let key = match matches.value_of("Key") {
        Some(k) => k.to_string(),
        None => panic!("--key,-k argument required"),
    };

    // Create client connector
    let client = UnixConnection::<JsonCodec<Request, Response>>::new(&addr, JsonCodec::new()).wait().unwrap();
    let (tx, rx) = client.split();

    match matches.value_of("Value") {
        Some(value) => {
            println!("Set key: '{}'", key);
            tx.send(Request::Set(key, value.to_string()))
        }
        None => {
            println!("Get key: '{}'", key);
            tx.send(Request::Get(key))
        }
    }.wait()
    .unwrap();

    rx.map(|resp| -> Result<(), DaemonError> {
        println!("Response: {:?}", resp);
        Ok(())
    }).wait()
    .next();
}
