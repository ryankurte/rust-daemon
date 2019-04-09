//#![feature(await_macro, async_await, futures_api)]

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

#[macro_use]
extern crate tokio;
extern crate tokio_io;
extern crate tokio_codec;
extern crate tokio_uds;
extern crate tokio_tcp;
extern crate tokio_udp;
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

trait GenericServer {
    type Address;
    type Request;
    type Response;
    
    fn send(&mut self, _: Self::Address);
    //fn incoming(&mut self) -> Option<UnboundedReceiver<Request<>>>;
}

/// Server provides a generic server over a stream and codec
/// This is used to implement daemon servers (ie. long running processes w/ network communication)
pub mod server;
pub use crate::server::Server;

/// Connection provides a generic connection over a stream and codec
/// This is used to implement clients (ie. for a command line utility)
pub mod connection;
pub use crate::connection::Connection;

/// Codecs implement protocol handling over connectors
pub mod codecs;

/// TCP implements a TCP socket server and connection
pub mod tcp;
pub use crate::tcp::{TcpServer, TcpInfo, TcpConnection};

/// UDP implements a UDP socket connection
/// As UDP is connection-less, no server is required
pub mod udp;
pub use crate::udp::{UdpConnection, UdpInfo};

/// Unix implements a Unix socket server and connection
pub mod unix;
pub use crate::unix::{UnixServer, UnixInfo, UnixConnection};

/// Error implements errors returned by the daemon
pub mod error;
pub use crate::error::Error as DaemonError;


/// JsonCodec re-exports the JSON codec for convenience
/// This is an alias of JsonCodec with default JsonError, use codecs::json::JsonCodec to specify error type manually
pub type JsonCodec<Req, Resp> = codecs::json::JsonCodec<Req, Resp, codecs::json::JsonError>;

