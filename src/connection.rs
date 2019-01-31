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
use tokio_codec::{Encoder, Decoder, Framed};


/// Connection type implemented on top of AsyncRead + AsyncWrite and an Encoder/Decoder
/// This provides a simple / generic base object for managing tokio connections
pub struct Connection<T: AsyncRead + AsyncWrite, Codec: Encoder + Decoder> 
{
    stream: Arc<Mutex<Framed<T, Codec>>>,
    pub(crate) exit_rx: Arc<Mutex<Option<oneshot::Receiver<()>>>>,
    pub(crate) exit_tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

impl <T, Codec> Connection<T, Codec>
where 
    T: AsyncWrite + AsyncRead + Send + 'static,
    Codec: Encoder + Decoder + Clone + Send + 'static,
    <Codec as Decoder>::Item: Send,
    <Codec as Decoder>::Error: Send + Debug,
{
    /// Create a new connection instance over an arbitrary stream
    pub fn from_socket(stream: T, codec: Codec) -> Connection<T, Codec> {
        // Setup stream and exit channels
        let (exit_tx, exit_rx) = oneshot::channel::<()>();

        // Build connection object
        Connection{
            stream: Arc::new(Mutex::new(Framed::new(stream, codec))),
            exit_rx: Arc::new(Mutex::new(Some(exit_rx))),
            exit_tx: Arc::new(Mutex::new(Some(exit_tx))),
        }
    }
}

impl <T, Codec>Connection<T, Codec>
where 
    T: AsyncWrite + AsyncRead + Send + 'static,
    Codec: Encoder + Decoder + Clone + Send + 'static,
    <Codec as Decoder>::Item: Send,
    <Codec as Decoder>::Error: Send + Debug,
{
    /// Exit closes the handler task if bound
    /// note this will panic if exit has already been called
    pub fn shutdown(self) {
        info!("[connection] exit called");

        // Send exit signal
        if let Some(c) = self.exit_tx.lock().unwrap().take() {
            c.send(()).unwrap();
        }

        // Close the stream
        self.stream.lock().unwrap().get_mut().shutdown().unwrap();
    }
}

/// Blank send
unsafe impl<T, Codec> Send for Connection<T, Codec> 
where
    T: AsyncWrite + AsyncRead,
    Codec: Encoder + Decoder, 
{}


/// Sink implementation allows sending messages over a connection
impl<T, Codec> Sink for Connection<T, Codec>
where
    T: AsyncWrite + AsyncRead,
    Codec: Encoder + Decoder, 
{
    type SinkItem = <Codec as tokio_codec::Encoder>::Item;
    type SinkError = <Codec as tokio_codec::Encoder>::Error;

    fn start_send(
        &mut self,
        item: Self::SinkItem,
    ) -> Result<AsyncSink<Self::SinkItem>, Self::SinkError> {
        trace!("[connection] start send");
        self.stream.lock().unwrap().start_send(item)
    }

    fn poll_complete(&mut self) -> Result<Async<()>, Self::SinkError> {
        trace!("[connection] send complete");
        self.stream.lock().unwrap().poll_complete()
    }
}

/// Stream implementation allows receiving messages from a connection
impl<T, Codec> Stream for Connection<T, Codec>
where
    T: AsyncWrite + AsyncRead,
    Codec: Encoder + Decoder, 
{
    type Item = <Codec as tokio_codec::Decoder>::Item;
    type Error = <Codec as tokio_codec::Decoder>::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        trace!("[connection] poll receive");
        self.stream.lock().unwrap().poll()
    }
}

/// Clone over generic connector
/// All instances of a given connector contain the same arc/mutex protected information
impl<T, Codec> Clone for Connection<T, Codec>
where
    T: AsyncWrite + AsyncRead,
    Codec: Encoder + Decoder, 
{
    fn clone(&self) -> Self {
        Connection {
            stream: self.stream.clone(),
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
    #[ignore]
    fn client_ping_pong() {
        let test = future::lazy(move || {
            // Build client pair
            let (a, b) = UnixStream::pair().unwrap();
            let client_a = Connection::<UnixStream, TestCodec>::from_socket(a, TestCodec{});
            let client_b = Connection::<UnixStream, TestCodec>::from_socket(b, TestCodec{});

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
