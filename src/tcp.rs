/**
 * rust-daemon
 * TCP Server Implementation
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

use server::Server;
use connection::Connection;
use error::Error;


/// TcpServer is a Server implementation over TcpStream and TcpInfo types with a generic codec
pub type TcpServer<C> = Server<TcpStream, C, TcpInfo>;

/// TcpClient is a Client implementation over TcpStream
pub type TcpConnection<C> = Connection<TcpStream, C>;

impl <C> TcpConnection<C> 
where
    C: Encoder + Decoder + Default + Send + 'static,
    <C as Decoder>::Item: Send,
    <C as Decoder>::Error: Send + Debug,
{
    /// Create a new client connected to the provided TCP socket address
    pub fn new(addr: &SocketAddr) -> Result<Connection<TcpStream, C>, Error> {
        trace!("[connector] creating connection (tcp address: {})", addr);
        // Create the socket future
        let socket = TcpStream::connect(&addr).wait()?;
        // Create the socket instance
        Ok(Connection::from(socket))
    }
}

/// TcpInfo is an information object associated with a given TcpServer connection
/// This is passed to the server request handler to allow ACLs and connection tracking
#[derive(Clone, Debug)]
pub struct TcpInfo {
    pub address: SocketAddr,
}

/// TCP server implementation
/// This binds to a TCP socket
impl<C> TcpServer<C>
where
    C: Encoder + Decoder + Default + Send + 'static,
    <C as Decoder>::Item: Clone + Send + Debug,
    <C as Decoder>::Error: Send + Debug,
    <C as Encoder>::Item: Clone + Send + Debug,
    <C as Encoder>::Error: Send + Debug,
{
    
    pub fn new_tcp(address: &SocketAddr) -> Result<TcpServer<C>, Error> {

        // Create base server instance
        let server = Server::new();

        // Create listener socket
        let socket = TcpListener::bind(&address)?;

        let exit_rx = server.exit_rx.lock().unwrap().take();
        let mut server_int = server.clone();

        // Create listening thread
        let tokio_server = socket
            .incoming()
            .for_each(move |s| {
                let info = TcpInfo{address: s.peer_addr().unwrap()};
                server_int.bind(info, s); 
                Ok(())
             })
            .map_err(|err| {
                error!("[server] accept error: {:?}", err);
            })
            .select2(exit_rx)
            .then(|_| {
                info!("[server] closing listener");
                Ok(())
            });
        spawn(tokio_server);

        // Return new connector instance
        Ok(server)
    }

    // Connect to a TCP socket
    pub fn connect(&mut self, address: &SocketAddr) -> Result<(), Error> {
        let socket = TcpStream::connect(&address).wait()?;
        let info = TcpInfo{address: address.clone()};
        self.bind(info, socket);

        Ok(())
    }
}
