
use std::sync::{Arc, Mutex};
use std::fmt::{Debug};
use std::marker::PhantomData;

use futures::sync::oneshot;

use tokio::prelude::*;
use tokio_codec::{Encoder, Decoder, Framed};

pub trait Inner <REQ, RESP, ERR>: Stream<Item=REQ, Error=ERR> + Sink<SinkItem=RESP, SinkError=ERR> {}

pub struct Connection<REQ, RESP, ERR, T: Inner<REQ, RESP, ERR>> 
{
    stream: Arc<Mutex<T>>,
    exit_rx: Arc<Mutex<Option<oneshot::Receiver<()>>>>,
    exit_tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
    req: PhantomData<REQ>,
    resp: PhantomData<RESP>,
    err: PhantomData<ERR>,
}

impl <REQ, RESP, ERR, T> From<T> for Connection<REQ, RESP, ERR, T>
where 
    T: Inner<REQ, RESP, ERR> + 'static,
{
    /// Create a new connection instance over an arbitrary stream
    fn from(stream: T) -> Connection<REQ, RESP, ERR, T> {
        // Setup stream and exit channels
        let (exit_tx, exit_rx) = oneshot::channel::<()>();

        // Build connection object
        Connection{
            stream: Arc::new(Mutex::new(stream)),
            exit_rx: Arc::new(Mutex::new(Some(exit_rx))),
            exit_tx: Arc::new(Mutex::new(Some(exit_tx))),

            req: PhantomData,
            resp: PhantomData,
            err: PhantomData,
        }
    }
}

impl <REQ, RESP, ERR, T>Connection<REQ, RESP, ERR, T>
where
    T: Inner<REQ, RESP, ERR> + 'static,
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
        //self.stream.lock().unwrap().shutdown().unwrap();
    }
}

/// Blank send
unsafe impl<REQ, RESP, ERR, T> Send for Connection<REQ, RESP, ERR, T> 
where
    T: Inner<REQ, RESP, ERR> + 'static,
{}

/// Sink implementation allows sending messages over a connection
impl<REQ, RESP, ERR, T> Sink for Connection<REQ, RESP, ERR, T>
where
    T: Inner<REQ, RESP, ERR> + 'static,
{
    type SinkItem = RESP;
    type SinkError = ERR;

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
impl<REQ, RESP, ERR, T> Stream for Connection<REQ, RESP, ERR, T>
where
    T: Inner<REQ, RESP, ERR> + 'static,
{
    type Item = REQ;
    type Error = ERR;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        trace!("[connection] poll receive");
        self.stream.lock().unwrap().poll()
    }
}

/// Clone over generic connector
/// All instances of a given connector contain the same arc/mutex protected information
impl<REQ, RESP, ERR, T> Clone for Connection<REQ, RESP, ERR, T>
where
    T: Inner<REQ, RESP, ERR> + 'static,
{
    fn clone(&self) -> Self {
        Connection {
            stream: self.stream.clone(),
            exit_tx: self.exit_tx.clone(),
            exit_rx: self.exit_rx.clone(),

            req: PhantomData,
            resp: PhantomData,
            err: PhantomData,
        }
    }
}


#[cfg(test)]
mod tests {

    use tokio::prelude::*;
    use tokio::{spawn, run};
    use tokio_uds::{UnixStream};
    use tokio_codec::{Decoder, Encoder, Framed};
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
            let client_a = Connection::<String, String, Error, _>::from(Framed::new(a, TestCodec::default()));
            let client_b = Connection::<String, String, Error, _>::from(Framed::new(b, TestCodec::default()));

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
