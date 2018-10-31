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
extern crate bytes;

extern crate tokio;
use tokio::prelude::*;

extern crate tokio_io;
extern crate tokio_codec;
extern crate tokio_uds;
extern crate tokio_tcp;
extern crate tokio_timer;

extern crate serde;
use serde::{Serialize, Deserialize};

extern crate serde_json;

#[cfg(test)]
#[macro_use]
extern crate serde_derive;

extern crate tokio_serde_json_mirror as tokio_serde_json;

#[macro_use]
extern crate log;
extern crate uuid;


/// Connection implements a network connection with a given codec
/// This is used to implement clients (ie. for a command line utility)
pub mod connection;
pub use connection::Connection;

/// Server implements a network server with connection management
/// This is used to implement daemon servers (ie. long running processes w/ network communication)
pub mod server;
pub use server::Server;

/// Error implements errors returned by the daemon
pub mod error;
pub use error::Error as DaemonError;

/// Codecs implement protocol handling over connectors
pub mod codecs;


/// JsonClient convenience wrapper to create clients with the JSON codec
pub type JsonClient<T, REQ, RESP> = Connection<T, codecs::json::JsonCodec<REQ, RESP, codecs::json::JsonError>>;

impl <T, REQ, RESP>JsonClient<T, REQ, RESP> 
where 
    T: AsyncWrite + AsyncRead + Send + 'static,
    REQ: Clone + Send + Serialize + 'static,
    for<'de> RESP: Clone + Send + Deserialize<'de> + 'static,
{
    pub fn new_json(stream: T) -> JsonClient<T, REQ, RESP> {
        Connection::<_, codecs::json::JsonCodec<REQ, RESP, codecs::json::JsonError>>::new(stream)
    }
}


mod user;
pub use user::User;

