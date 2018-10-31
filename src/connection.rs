/**
 * rust-daemon
 * Connection implementation
 *
 * https://github.com/ryankurte/rust-daemon
 * Copyright 2018 Ryan Kurte
 */

use std::sync::{Arc, Mutex};
use std::fmt::{Debug};

use futures::sync::oneshot;

use tokio::prelude::*;

use tokio::io::{ReadHalf, WriteHalf};
use tokio_codec::{Encoder, Decoder, FramedRead, FramedWrite};

use tokio_uds::UnixStream;
use crate::error::Error;

/// Receive stream type
pub type Receive<T, D> = Arc<Mutex<FramedRead<ReadHalf<T>, D>>>;

/// Transmit frame type
pub type Transmit<T, E> = Arc<Mutex<FramedWrite<WriteHalf<T>, E>>>;


/// Connection type implemented on top of AsyncRead + AsyncWrite and an Encoder/Decoder
/// This provides a simple / generic base object for managing tokio connections
pub struct Connection<T: AsyncRead + AsyncWrite, C: Encoder + Decoder> 
{
    receive: Receive<T, C>,
    transmit: Transmit<T, C>,
    pub(crate) exit_rx: Arc<Mutex<Option<oneshot::Receiver<()>>>>,
    pub(crate) exit_tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

impl <T, C>Connection<T, C>
where 
    T: AsyncWrite + AsyncRead + Send + 'static,
    C: Encoder + Decoder + Default + Send + 'static,
    <C as Decoder>::Item: Send,
    <C as Decoder>::Error: Send + Debug,
{
    /// Create a new connection instance over an arbitrary stream
    pub fn new(stream: T) -> Connection<T, C> {
        // Setup receive and transmit channels
        let (receive, transmit) = stream.split();
        let transmit = FramedWrite::new(transmit, C::default());
        let receive = FramedRead::new(receive, C::default());

        let receive = Arc::new(Mutex::new(receive));
        let transmit = Arc::new(Mutex::new(transmit));

        let (exit_tx, exit_rx) = oneshot::channel::<()>();

        // Build connection object
        Connection{
            receive, transmit, 
            exit_rx: Arc::new(Mutex::new(Some(exit_rx))),
            exit_tx: Arc::new(Mutex::new(Some(exit_tx))),
        }
    }

    /// Create a new client connected to the provided unix socket
    pub fn new_unix(path: &str) -> Result<Connection<UnixStream, C>, Error> {
        trace!("[connector] creating connection (socket: {})", path);
        // Create the socket future
        let socket = UnixStream::connect(&path).wait()?;
        // Create the socket instance
        Ok(Connection::new(socket))
    }

    /// Exit closes the handler task if bound
    /// note this will panic if exit has already been called
    pub fn exit(self) {
        info!("[connection] exit called");
        if let Some(c) = self.exit_tx.lock().unwrap().take() {
            c.send(()).unwrap();
        }
    }
}

/// Blank send
unsafe impl<T, C> Send for Connection<T, C> 
where
    T: AsyncWrite + AsyncRead,
    C: Encoder + Decoder + Default, 
{}

/// Sink implementation allows sending messages over a connection
impl<T, C> Sink for Connection<T, C>
where
    T: AsyncWrite + AsyncRead,
    C: Encoder + Decoder + Default, 
{
    type SinkItem = <C as tokio_codec::Encoder>::Item;
    type SinkError = <C as tokio_codec::Encoder>::Error;

    fn start_send(
        &mut self,
        item: Self::SinkItem,
    ) -> Result<AsyncSink<Self::SinkItem>, Self::SinkError> {
        trace!("[connection] start send");
        self.transmit.lock().unwrap().start_send(item)
    }

    fn poll_complete(&mut self) -> Result<Async<()>, Self::SinkError> {
        trace!("[connection] send complete");
        self.transmit.lock().unwrap().poll_complete()
    }
}

/// Stream implementation allows receiving messages from a connection
impl<T, C> Stream for Connection<T, C>
where
    T: AsyncWrite + AsyncRead,
    C: Encoder + Decoder + Default, 
{
    type Item = <C as tokio_codec::Decoder>::Item;
    type Error = <C as tokio_codec::Decoder>::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        trace!("[connection] poll receive");
        self.receive.lock().unwrap().poll()
    }
}

/// Clone over generic connector
/// All instances of a given connector contain the same arc/mutex protected information
impl<T, C> Clone for Connection<T, C>
where
    T: AsyncWrite + AsyncRead,
    C: Encoder + Decoder + Default, 
{
    fn clone(&self) -> Self {
        Connection {
            receive: self.receive.clone(),
            transmit: self.transmit.clone(),
            exit_tx: self.exit_tx.clone(),
            exit_rx: self.exit_rx.clone(),
        }
    }
}

#[cfg(test)]
mod tests {

    use tokio::prelude::*;
    use tokio::{spawn, run};
    use tokio_uds::{UnixStream};
    use tokio_codec::{Decoder, Encoder};
    use bytes::{BufMut, BytesMut};

    use super::Connection;
    use crate::error::Error;

    #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
    struct TestCodec {}

    impl Decoder for TestCodec {
        type Item = String;
        type Error = Error;

        fn decode(&mut self, buff: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
            let vec: Vec<u8> = buff.clone().into_iter().collect();
            let val = String::from_utf8(vec).unwrap();
            buff.advance(val.len());
            
            if val.len() > 0 {
                Ok(Some(val))
            } else {
                Ok(None)
            }
        }
    }

    impl Encoder for TestCodec {
        type Item = String;
        type Error = Error;

        fn encode(&mut self, v: Self::Item, buff: &mut BytesMut) -> Result<(), Self::Error> {
            buff.reserve(v.len());
            buff.put_slice(&v.as_bytes());
            Ok(())
        }
    }

    #[test]
    fn client_ping_pong() {
        let test = future::lazy(move || {
            // Build client pair
            let (a, b) = UnixStream::pair().unwrap();
            let client_a = Connection::<UnixStream, TestCodec>::new(a);
            let client_b = Connection::<UnixStream, TestCodec>::new(b);

            // Send a message
            let t = "test string".to_owned();

            client_a.send(t.clone()).wait().unwrap();

            println!("Send message: {:?}", t);

            // Receive a message
            // TODO: this should be, receive ONE message
            // Maybe a once + a timeout would work here?
            let rx_handle = client_b
                .for_each(move |m| {
                    println!("Received message: {:?}", m);
                    assert_eq!(t, m);
                    Ok(())
                }).map_err(|_e| ());
            spawn(rx_handle);
            
            Ok(())
        }).map(|_e| ()); 

        run(test);
    }
}
