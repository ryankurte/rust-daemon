/**
 * rust-daemon
 * TCP Server and Connection Implementations
 *
 * https://github.com/ryankurte/rust-daemon
 * Copyright 2018 Ryan Kurte
 */

use std::fmt::{Debug};
use std::clone::{Clone};
use std::net::SocketAddr;

use tokio::prelude::*;
use tokio::spawn;
use tokio_codec::{Encoder, Decoder};

use tokio_tcp::{TcpListener, TcpStream};

use crate::server::Server;
use crate::connection::Connection;
use crate::error::Error;


/// TcpServer is a Server implementation over TcpStream and TcpInfo types with a generic codec
/// 
/// ```no_run
/// use std::net::{SocketAddr, IpAddr, Ipv4Addr};
/// 
/// extern crate tokio;
/// use tokio::prelude::*;
/// use tokio::{spawn, run};
/// 
/// #[macro_use]
/// extern crate serde;
/// 
/// extern crate daemon_engine;
/// use daemon_engine::{TcpServer, JsonCodec};
/// 
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct Request {}
/// 
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct Response {}
/// 
/// # fn main() {
/// 
/// let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 8111);
/// let server = future::lazy(move || {
///     let mut s = TcpServer::<JsonCodec<Response, Request>>::new(&addr, JsonCodec::new()).unwrap();
///     let server_handle = s
///         .incoming()
///         .unwrap()
///         .for_each(move |r| {
///             println!("Request data {:?} info: {:?}", r.data(), r.info());
///             r.send(Response{}).wait().unwrap();
///             Ok(())
///         }).map_err(|_e| ());
///     spawn(server_handle);
///     Ok(())
/// });
/// run(server);
/// 
/// # }
/// ```
pub type TcpServer<C> = Server<TcpStream, C, TcpInfo>;

/// TcpConnection is a Connection implementation over TcpStream
/// 
/// ```no_run
/// use std::net::{SocketAddr, IpAddr, Ipv4Addr};
/// 
/// extern crate tokio;
/// use tokio::prelude::*;
/// use tokio::{spawn, run};
/// 
/// #[macro_use]
/// extern crate serde;
/// 
/// extern crate daemon_engine;
/// use daemon_engine::{TcpConnection, JsonCodec, DaemonError};
/// 
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct Request {}
/// 
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct Response {}
/// 
/// # fn main() {
/// let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8111);
/// let client = TcpConnection::<JsonCodec<Request, Response>>::new(&addr, JsonCodec::new()).wait().unwrap();
/// let (tx, rx) = client.split();
/// // Send data
/// tx.send(Request{}).wait().unwrap();
/// 
/// // Receive data
/// rx.map(|resp| -> Result<(), DaemonError> {
///    println!("Response: {:?}", resp);
///    Ok(())
/// }).wait().next();
/// # }
/// ```
pub type TcpConnection<C> = Connection<TcpStream, C>;

impl <C> TcpConnection<C> 
where
    C: Encoder + Decoder + Clone + Send + 'static,
    <C as Decoder>::Item: Send,
    <C as Decoder>::Error: Send + Debug,
{
    /// Create a new client connected to the provided TCP socket address
    pub fn new(addr: &SocketAddr, codec: C) -> impl Future<Item=Connection<TcpStream, C>, Error=Error> {
        debug!("[connector] creating connection (tcp address: {})", addr);
        // Create the socket future
        TcpStream::connect(&addr).map(move |s| {
            Connection::from_socket(s, codec)
        }).map_err(|e| e.into() )
    }

    pub fn close(self) {
        self.shutdown();
    }
}

/// TcpInfo is an information object associated with a given TcpServer connection.
/// This is passed to the server request handler to allow ACLs and connection tracking
#[derive(Clone, Debug)]
pub struct TcpInfo {
    pub address: SocketAddr,
}

/// TCP server implementation.
impl<C> TcpServer<C>
where
    C: Encoder + Decoder + Clone + Send + 'static,
    <C as Decoder>::Item: Clone + Send + Debug,
    <C as Decoder>::Error: Send + Debug,
    <C as Encoder>::Item: Clone + Send + Debug,
    <C as Encoder>::Error: Send + Debug,
{
    
    pub fn new(address: &SocketAddr, codec: C) -> Result<TcpServer<C>, Error> {

        // Create base server instance
        let server = Server::base(codec);

        // Create listener socket
        let socket = TcpListener::bind(&address)?;

        let exit_rx = server.exit_rx.lock().unwrap().take();
        let mut server_int = server.clone();

        // Create listening thread
        let tokio_server = socket
            .incoming()
            .for_each(move |s| {
                debug!("[server] accept connection: {:?}", s);
                let info = TcpInfo{address: s.peer_addr().unwrap()};
                server_int.bind(info, s); 
                Ok(())
             })
            .map_err(|err| {
                error!("[server] accept error: {:?}", err);
            })
            .select2(exit_rx)
            .then(|_| {
                debug!("[server] closing listener");
                Ok(())
            });
        spawn(tokio_server);

        // Return new connector instance
        Ok(server)
    }

    // Connect to a TCP socket
    pub fn connect(&mut self, address: SocketAddr) -> impl Future<Item=(), Error=Error> {
        let mut s = self.clone();
        TcpStream::connect(&address).map(move |socket| {
            let info = TcpInfo{address: address.clone()};
            s.bind(info, socket);
            ()
        }).map_err(|e| e.into() )
    }
}
