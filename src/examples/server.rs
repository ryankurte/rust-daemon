/**
 * rust-daemon
 * Server example
 * 
 * https://github.com/ryankurte/rust-daemon
 * Copyright 2018 Ryan Kurte
 */


use std::collections::HashMap;
use std::sync::Mutex;

#[macro_use]
extern crate clap;
use clap::{App, Arg};

extern crate tokio;
use tokio::prelude::*;

extern crate serde;
#[macro_use]
extern crate serde_derive;

extern crate daemon;
use daemon::Server;

mod common;
use common::{Request, Response};

fn main() {
    let matches = App::new("rustd-server")
        .author("Ryan Kurte <diot@kurte.nz>")
        .about("rust-daemon example server")
        .version(crate_version!())
        .arg(
            Arg::with_name("Socket address")
                .short("s")
                .long("socket-address")
                .help("Sets unix socket address")
                .takes_value(true)
                .default_value("/tmp/rustd.sock"),
        )
        .get_matches();

    let addr = matches.value_of("Socket address").unwrap().to_owned();

    let server = future::lazy(move || {
        let s = Server::<Request, Response>::new(&addr).unwrap();
        let m = Mutex::new(HashMap::<String, String>::new());

        let server_handle =
            s.incoming().for_each(move |r| {
                println!("Request: {:?}", r.data());
                let data = r.data();
                match data {
                    Request::Get(k) => match m.lock().unwrap().get(&k) {
                        Some(v) => r.send(Response::Value(v.to_string())),
                        None => r.send(Response::None),
                    },
                    Request::Set(k, v) => {
                        m.lock().unwrap().insert(k, v.clone());
                        r.send(Response::Value(v.to_string()))
                    }
                }.wait()
                    .unwrap();

                Ok(())
            }).map_err(|_e| ());
        tokio::spawn(server_handle);
        Ok(())
    });

    tokio::run(server);

    println!("Done!");
}
