
#[macro_use]
extern crate clap;
use clap::{App, Arg};

extern crate tokio;
use tokio::runtime::Runtime;

extern crate rust_daemon;
use rust_daemon::Server;

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
                .default_value("/var/rustd.sock"),
        )
        .get_matches();

    let mut runtime = Runtime::new().unwrap();
    let addr = matches.value_of("Socket Address").unwrap();

    let s = Server::new(&mut runtime, addr).unwrap();
}
