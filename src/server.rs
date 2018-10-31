/**
 * rust-daemon
 * Server implementation
 *
 * https://github.com/ryankurte/rust-daemon
 * Copyright 2018 Ryan Kurte
 */

use std::fs;
use std::sync::{Arc, Mutex};
use std::fmt::{Debug};
use std::clone::{Clone};
use std::net::SocketAddr;

use futures::sync::mpsc;
use futures::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use futures::sync::oneshot;

use tokio::prelude::*;
use tokio::spawn;
use tokio_codec::{Encoder, Decoder};

use tokio_uds::{UnixListener, UnixStream};
use tokio_tcp::{TcpListener, TcpStream};

use connection::Connection;
use error::Error;


/// Server provdides a base for building stream servers
pub struct Server<T: AsyncRead + AsyncWrite, C: Encoder + Decoder> {
    connections: Arc<Mutex<Vec<Connection<T, C>>>>,
    incoming_tx: Arc<Mutex<UnboundedSender<Request<T, C>>>>,
    incoming_rx: Arc<Mutex<Option<UnboundedReceiver<Request<T, C>>>>>,
    pub(crate) exit_tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
    pub(crate) exit_rx: Arc<Mutex<Option<oneshot::Receiver<()>>>>,
}

impl<T, C> Server<T, C>
where
    T: AsyncWrite + AsyncRead + Send + Sync + 'static,
    C: Encoder + Decoder + Default + Send + 'static,
    <C as Decoder>::Item: Clone + Send + Debug,
    <C as Decoder>::Error: Send + Debug,
    <C as Encoder>::Item: Clone + Send + Debug,
    <C as Encoder>::Error: Send + Debug,
{
    /// Create a new connector with defined request and response message types
    /// This sets up internal resources but does not start a listener until one is provded
    fn new() -> Server<T, C> {
        // Setup internal state and communication channels
        let connections = Arc::new(Mutex::new(Vec::new()));
        let (incoming_tx, incoming_rx) = mpsc::unbounded::<Request<T, C>>();
        let (exit_tx, exit_rx) = oneshot::channel::<()>();

        Server {
            connections,
            incoming_tx: Arc::new(Mutex::new(incoming_tx)),
            incoming_rx: Arc::new(Mutex::new(Some(incoming_rx))),
            exit_tx: Arc::new(Mutex::new(Some(exit_tx))),
            exit_rx: Arc::new(Mutex::new(Some(exit_rx))),
        }
    }

    /// Take the incoming data handle
    /// You can then use `for_each` to iterate over received requests as in
    /// the examples
    pub fn incoming(&mut self) -> Option<UnboundedReceiver<Request<T, C>>> {
        self.incoming_rx.lock().unwrap().take()
    }

    /// Bind a socket to a server
    /// This attaches an rx handler to the server, and can be used both for
    /// server listener implementations as well as to support server-initialised
    /// connections if required
    pub fn bind(&mut self, socket: T) {
        // Create new connection object
        let conn = Connection::new(socket);

        // Add connection to list
        self.connections.lock().unwrap().push(conn.clone());

        // Handle incoming requests
        // This creates a handler task on the connecton object
        let inner_tx = self.incoming_tx.clone();
        let exit_rx = conn.exit_rx.lock().unwrap().take();

        let rx_handle = conn.clone().for_each(move |data| {
            let tx = inner_tx.lock().unwrap();
            let req = Request{inner: conn.clone(), data: data.clone()};
            tx.clone().send(req).wait().unwrap();
            Ok(())
        })
        .map_err(|e| panic!("[server] error: {:?}", e))
        .select2(exit_rx)
        .then(|_| {
            info!("[server] closing handler");
            Ok(())
        });

        spawn(rx_handle);
    }

    /// Close the socket server
    /// This sends exit messages to the main task and all connected hosts
    pub fn close(self) {
        trace!("[server] closing");

        // Send listener exit signal
        let tx = self.exit_tx.lock().unwrap().take().unwrap();
        let _ = tx.send(());

        // Send exit signals to client listeners
        let mut connections = self.connections.lock().unwrap();
        let _results: Vec<_> = connections.drain(..).map(|c| drop(c) ).collect();
    }
}

/// Clone over generic connector
/// All instances of a given connector contain the same arc/mutex protected information
impl<T, C> Clone for Server<T, C>
where
    T: AsyncWrite + AsyncRead + Send + Sync + 'static,
    C: Encoder + Decoder + Default + Send + 'static,
    <C as Decoder>::Item: Clone + Send + Debug,
    <C as Decoder>::Error: Send + Debug,
    <C as Encoder>::Item: Clone + Send + Debug,
    <C as Encoder>::Error: Send + Debug,
{
    fn clone(&self) -> Self {
        Server {
            connections: self.connections.clone(),
            incoming_tx: self.incoming_tx.clone(),
            incoming_rx: self.incoming_rx.clone(),
            exit_tx: self.exit_tx.clone(),
            exit_rx: self.exit_rx.clone(),
        }
    }
}


/// Unix server implementation
/// This binds to a unix domain socket
impl<C> Server<UnixStream, C>
where
    C: Encoder + Decoder + Default + Send + 'static,
    <C as Decoder>::Item: Clone + Send + Debug,
    <C as Decoder>::Error: Send + Debug,
    <C as Encoder>::Item: Clone + Send + Debug,
    <C as Encoder>::Error: Send + Debug,
{
    
    pub fn new_unix(path: &str) -> Result<Server<UnixStream, C>, Error> {
        // Pre-clear socket file
        let _res = fs::remove_file(&path);

        // Create base server instance
        let server = Server::new();

        // Create listener socket
        let socket = UnixListener::bind(&path)?;

        let exit_rx = server.exit_rx.lock().unwrap().take();
        let mut server_int = server.clone();

        // Create listening thread
        let tokio_server = socket
            .incoming()
            .for_each(move |s| {
                server_int.bind(s); 
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
}

/// TCP server implementation
/// This binds to a TCP socket
impl<C> Server<TcpStream, C>
where
    C: Encoder + Decoder + Default + Send + 'static,
    <C as Decoder>::Item: Clone + Send + Debug,
    <C as Decoder>::Error: Send + Debug,
    <C as Encoder>::Item: Clone + Send + Debug,
    <C as Encoder>::Error: Send + Debug,
{
    
    pub fn new_tcp(addr: &SocketAddr) -> Result<Server<TcpStream, C>, Error> {

        // Create base server instance
        let server = Server::new();

        // Create listener socket
        let socket = TcpListener::bind(&addr)?;

        let exit_rx = server.exit_rx.lock().unwrap().take();
        let mut server_int = server.clone();

        // Create listening thread
        let tokio_server = socket
            .incoming()
            .for_each(move |s| {
                server_int.bind(s); 
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
}


/// Request is a request from a client
/// This contains the Request data and is a Sink for Response data
/// Allowing message handlers to reply directly to the client
pub struct Request<T: AsyncRead + AsyncWrite, C: Encoder + Decoder> {
    inner: Connection<T, C>,
    data: <C as Decoder>::Item,
}

/// Allows requests to be cloned
impl<T, C> Request<T, C>
where
    T: AsyncWrite + AsyncRead + Send + 'static,
    C: Encoder + Decoder + Default + Send + 'static,
    <C as Decoder>::Item: Send + Clone,
    <C as Decoder>::Error: Send + Debug,
{
    // Fetch the data for a given request
    pub fn data(&self) -> <C as Decoder>::Item {
        self.data.clone()
    }
}

/// Sink implementation allows responses to requests
impl<T, C> Sink for Request<T, C>
where
    T: AsyncWrite + AsyncRead + Send + 'static,
    C: Encoder + Decoder + Default + Send + 'static,
    <C as Decoder>::Item: Send,
    <C as Decoder>::Error: Send + Debug,
{
    type SinkItem = <C as Encoder>::Item;
    type SinkError = <C as Encoder>::Error;

    fn start_send(
        &mut self,
        item: Self::SinkItem,
    ) -> Result<AsyncSink<Self::SinkItem>, Self::SinkError> {
        trace!("[request] start send");
        self.inner.start_send(item)
    }

    fn poll_complete(&mut self) -> Result<Async<()>, Self::SinkError> {
        trace!("[request] send complete");
        self.inner.poll_complete()
    }
}
