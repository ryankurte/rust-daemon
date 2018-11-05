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
extern crate tokio_io;
extern crate tokio_codec;
extern crate tokio_uds;
extern crate tokio_tcp;
extern crate tokio_timer;

extern crate serde;
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

/// TCP implements a TCP socket server and connection
pub mod tcp;
pub use tcp::{TcpServer, TcpInfo, TcpConnection};

/// Unix implements a Unix socket server and connection
pub mod unix;
pub use unix::{UnixServer, UnixInfo, UnixConnection};

/// Error implements errors returned by the daemon
pub mod error;
pub use error::Error as DaemonError;

/// Codecs implement protocol handling over connectors
pub mod codecs;

/// JsonCodec re-exports the JSON codec for convenience
/// This is an alias of JsonCodec with default JsonError, use codecs::json::JsonCodec to specify error type manually
pub type JsonCodec<REQ, RESP> = codecs::json::JsonCodec<REQ, RESP, codecs::json::JsonError>;

