
use std::sync::{Arc, Mutex};
use std::io::Error as IoError;

use bytes::BytesMut;
use futures::prelude::*;

use tokio_uds::{UnixStream};
use tokio_io::codec::length_delimited;

use error::DaemonError;

/// Client implements a daemon client
/// This connects to an IPC socket and sends and receives messages from a connected daemon
#[derive(Debug)]
pub struct Client {
    pub socket: Arc<Mutex<length_delimited::Framed<UnixStream>>>,
}

impl Clone for Client {
    fn clone(&self) -> Self {
        Client{socket: self.socket.clone()}
    }
}

impl Client {
    /// New creates a new client connected to the provided IPC socket
    pub fn new(path: &str) -> Result<Client, DaemonError> {
        println!("[daemon client] creating connection (socket: {})", path);
        // Create the socket future
        let socket = UnixStream::connect(path).wait()?;
        // Wrap to a length delimited framed socket
        let socket = length_delimited::Framed::new(socket);

        Ok(Client { socket: Arc::new(Mutex::new(socket)) })
    }

    /// Fetch the stream associated with the client
    pub fn split(&self) -> (Outgoing, Incoming){
        (Outgoing{inner: self.clone()}, Incoming{inner: self.clone()})
    }

    /// Close consumes the client connector and closes the socket
    pub fn close(self) -> Result<(), DaemonError> {
        println!("[daemon client] closing connection");
        Ok(())
    }
}

pub struct Incoming {
    inner: Client
}

impl Stream for Incoming {
    type Item = BytesMut;
    type Error = IoError;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        println!("[daemon client] poll");
        self.inner.socket.lock().unwrap().poll()
    }
}

pub struct Outgoing {
    inner: Client
}

impl Sink for Outgoing {
    type SinkItem = BytesMut;
    type SinkError = IoError;

    fn start_send(&mut self, item: Self::SinkItem) -> Result<AsyncSink<Self::SinkItem>, Self::SinkError> {
        println!("[daemon client] start_send");
         self.inner.socket.lock().unwrap().start_send(item)
    }

    fn poll_complete(&mut self) -> Result<Async<()>, Self::SinkError> {
        println!("[daemon client] poll_complete");
        self.inner.socket.lock().unwrap().poll_complete()
    }
}

