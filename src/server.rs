/**
 * rust-daemon
 * Generic server implementation
 *
 * https://github.com/ryankurte/rust-daemon
 * Copyright 2018 Ryan Kurte
 */

use std::sync::{Arc, Mutex};
use std::fmt::{Debug};
use std::clone::{Clone};

use futures::sync::mpsc;
use futures::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use futures::sync::oneshot;

use tokio::prelude::*;
use tokio::spawn;
use tokio_codec::{Encoder, Decoder};

use crate::connection::Connection;


/// Server provides a generic base for building stream servers.
/// 
/// This is generic over T, a stream reader and writer, C, and encoder and decoder, and I, and information object.
///
/// You probably want to be looking at TcpServer and UnixServer implementations
pub struct Server<T: AsyncRead + AsyncWrite, C: Encoder + Decoder, I> {
    connections: Arc<Mutex<Vec<Connection<T, C>>>>,
    incoming_tx: Arc<Mutex<UnboundedSender<Request<T, C, I>>>>,
    incoming_rx: Arc<Mutex<Option<UnboundedReceiver<Request<T, C, I>>>>>,
    pub(crate) exit_tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
    pub(crate) exit_rx: Arc<Mutex<Option<oneshot::Receiver<()>>>>,
    codec: C,
    info: std::marker::PhantomData<I>,
}

impl<T, C, I> Server<T, C, I>
where
    T: AsyncWrite + AsyncRead + Send + Sync + 'static,
    C: Encoder + Decoder + Clone + Send + 'static,
    I: Clone + Send + Debug + 'static,
    <C as Decoder>::Item: Clone + Send + Debug,
    <C as Decoder>::Error: Send + Debug,
    <C as Encoder>::Item: Clone + Send + Debug,
    <C as Encoder>::Error: Send + Debug,
{
    /// Create a new base server with defined request and response message types.
    /// 
    /// This sets up internal resources however requires implementation to handle
    /// creating listeners and binding connections
    /// See TcpServer and UnixServer for examples
    pub fn base(codec: C) -> Server<T, C, I> {
        // Setup internal state and communication channels
        let connections = Arc::new(Mutex::new(Vec::new()));
        let (incoming_tx, incoming_rx) = mpsc::unbounded::<Request<T, C, I>>();
        let (exit_tx, exit_rx) = oneshot::channel::<()>();

        Server {
            connections,
            incoming_tx: Arc::new(Mutex::new(incoming_tx)),
            incoming_rx: Arc::new(Mutex::new(Some(incoming_rx))),
            exit_tx: Arc::new(Mutex::new(Some(exit_tx))),
            exit_rx: Arc::new(Mutex::new(Some(exit_rx))),
            codec,
            info: std::marker::PhantomData,
        }
    }

    /// Take the incoming data handle.
    /// 
    /// You can then use `for_each` to iterate over received requests as in
    /// the examples
    pub fn incoming(&mut self) -> Option<UnboundedReceiver<Request<T, C, I>>> {
        self.incoming_rx.lock().unwrap().take()
    }

    /// Bind a socket to a server.
    /// 
    /// This attaches an rx handler to the server, and can be used both for
    /// server listener implementations as well as to support server-initialised
    /// connections if required
    pub fn bind(&mut self, info: I, socket: T) {
        // Create new connection object
        let conn = Connection::from_socket(socket, self.codec.clone());

        // Add connection to list
        self.connections.lock().unwrap().push(conn.clone());

        // Handle incoming requests
        // This creates a handler task on the connection object
        let inner_tx = self.incoming_tx.clone();
        let exit_rx = conn.exit_rx.lock().unwrap().take();

        let rx_handle = conn.clone().for_each(move |data| {
            let tx = inner_tx.lock().unwrap();
            let req = Request{inner: conn.clone(), info: info.clone(), data: data.clone()};
            tx.clone().send(req).map(|_| () ).map_err(|e| panic!("[server] send error: {:?}", e) )
        })
        .map_err(|e| panic!("[server] error: {:?}", e) )
        .select2(exit_rx)
        .then(|_| {
            debug!("[server] closing handler");
            Ok(())
        });

        spawn(rx_handle);
    }

    /// Close the socket server
    /// 
    /// This sends exit messages to the main task and all connected hosts
    pub fn close(self) {
        debug!("[server] closing");

        // Send listener exit signal
        let tx = self.exit_tx.lock().unwrap().take().unwrap();
        let _ = tx.send(());

        // Send exit signals to client listeners
        let mut connections = self.connections.lock().unwrap();
        let _results: Vec<_> = connections.drain(..).map(|c| c.shutdown() ).collect();
    }
}

/// Clone over generic connector
/// 
/// All instances of a given connector contain the same arc/mutex protected information
impl<T, C, I> Clone for Server<T, C, I>
where
    T: AsyncWrite + AsyncRead + Send + Sync + 'static,
    C: Encoder + Decoder + Clone + Send + 'static,
    I: Clone + Send + Debug + 'static,
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
            codec: self.codec.clone(),
            info: std::marker::PhantomData,
        }
    }
}

/// Request is a request from a client
/// 
/// This contains the Request data and is a Sink for Response data
/// Allowing message handlers to reply directly to the client
pub struct Request<T: AsyncRead + AsyncWrite, C: Encoder + Decoder, I> {
    inner: Connection<T, C>,
    data: <C as Decoder>::Item,
    info: I,
}

/// Allows requests to be cloned
impl<T, C, I> Request<T, C, I>
where
    T: AsyncWrite + AsyncRead + Send + 'static,
    C: Encoder + Decoder + Clone + Send + 'static,
    I: Clone + Send + Debug + 'static,
    <C as Decoder>::Item: Send + Clone,
    <C as Decoder>::Error: Send + Debug,
{
    // Fetch the data for a given request
    pub fn data(&self) -> <C as Decoder>::Item {
        self.data.clone()
    }

    // Fetch connection information for a given request
    pub fn info(&self) -> &I {
        &self.info
    }
}

/// Sink implementation allows responses to requests
impl<T, C, I> Sink for Request<T, C, I>
where
    T: AsyncWrite + AsyncRead + Send + 'static,
    C: Encoder + Decoder + Clone + Send + 'static,
    I: Clone + Send + Debug + 'static,
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
