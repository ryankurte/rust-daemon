use std::collections::HashMap;
use std::sync::Mutex;

#[macro_use]
extern crate clap;
use clap::{App, Arg};

extern crate tokio;
use tokio::executor::thread_pool;
use tokio::prelude::*;
use tokio::runtime::Builder;

extern crate serde;
#[macro_use]
extern crate serde_derive;

extern crate rust_daemon;
use rust_daemon::Server;

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

    let addr = matches.value_of("Socket address").unwrap();

    let mut threadpool_builder = thread_pool::Builder::new();
    threadpool_builder.name_prefix("rustd-").pool_size(4);

    // build Runtime
    let mut runtime = Builder::new()
        .threadpool_builder(threadpool_builder)
        .build()
        .unwrap();

    let s = Server::<Request, Response>::new(&mut runtime, addr).unwrap();
    let m = Mutex::new(HashMap::<String, String>::new());

    let server_handle =
        s.for_each(move |r| {
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
    runtime.spawn(server_handle);

    loop {};

    // Run until threads go idle (ie. never)
    runtime.shutdown_on_idle().wait().unwrap();

    println!("Done!");

    let _ = s;
}
