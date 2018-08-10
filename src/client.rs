/**
 * rust-daemon
 * Client implementation
 *
 * https://github.com/ryankurte/rust-daemon
 * Copyright 2018 Ryan Kurte
 */
use std::io::Error as IoError;
use std::sync::{Arc, Mutex};

use tokio::io::{ReadHalf, WriteHalf};
use tokio::prelude::*;
use tokio_io::codec::length_delimited::{FramedRead, FramedWrite};
use tokio_serde_json::{ReadJson, WriteJson};
use tokio_uds::UnixStream;

use serde::{Deserialize, Serialize};

use error::DaemonError;

/// Receive stream type
type Receive<T, MSG> = Arc<Mutex<ReadJson<FramedRead<ReadHalf<T>>, MSG>>>;

/// Transmit frame type
type Transmit<T, MSG> = Arc<Mutex<WriteJson<FramedWrite<WriteHalf<T>>, MSG>>>;

/// Client implements a client for communicating with a daemon service
/// This connects to an IPC socket and sends and receives messages from a connected daemon
/// This acts as a Sink for requests and a Source for responses
pub struct Client<T: AsyncRead + AsyncWrite, REQ, RESP> {
    pub(crate) receive: Receive<T, RESP>,
    pub(crate) transmit: Transmit<T, REQ>,
}

/// Methods for UnixStream clients
impl<REQ, RESP> Client<UnixStream, REQ, RESP>
where
    for<'de> REQ: Serialize + Deserialize<'de> + Clone + Send + 'static,
    for<'de> RESP: Serialize + Deserialize<'de> + Clone + Send + 'static,
{
    /// Create a new client connected to the provided unix socket
    pub fn new(path: String) -> Result<Client<UnixStream, REQ, RESP>, DaemonError> {
        trace!("[daemon client] creating connection (socket: {})", path);
        // Create the socket future
        let socket = UnixStream::connect(&path).wait()?;
        // Create the socket instance
        Ok(Client::from(socket))
    }
}

/// Methods for generic clients
impl<T, REQ, RESP> Client<T, REQ, RESP>
where
    T: AsyncRead + AsyncWrite,
    for<'de> REQ: Serialize + Deserialize<'de> + Clone + Send + 'static,
    for<'de> RESP: Serialize + Deserialize<'de> + Clone + Send + 'static,
{
    /// Split a client into Transmit and Receive streams
    pub fn split<'a>(&'a self) -> (Transmit<T, REQ>, Receive<T, RESP>) {
        (self.transmit.clone(), self.receive.clone())
    }

    /// Close consumes the client connector and closes the socket
    pub fn close(self) -> Result<(), DaemonError> {
        trace!("[daemon client] closing connection");
        Ok(())
    }
}

/// Clone over generic client
impl<T, REQ, RESP> Clone for Client<T, REQ, RESP>
where
    T: AsyncRead + AsyncWrite,
{
    fn clone(&self) -> Self {
        Client {
            receive: self.receive.clone(),
            transmit: self.transmit.clone(),
        }
    }
}

/// Create a client from a provided unix stream
impl<REQ, RESP> From<UnixStream> for Client<UnixStream, REQ, RESP>
where
    for<'de> REQ: Serialize + Deserialize<'de> + Clone + Send + 'static,
    for<'de> RESP: Serialize + Deserialize<'de> + Clone + Send + 'static,
{
    /// Create a client from an existing UnixStream socket
    fn from(socket: UnixStream) -> Client<UnixStream, REQ, RESP> {
        // Wrap to a length delimited json encoded framed socket pair
        let (receive, transmit) = socket.split();
        let receive = Arc::new(Mutex::new(ReadJson::<_, RESP>::new(FramedRead::new(
            receive,
        ))));
        let transmit = Arc::new(Mutex::new(WriteJson::<_, REQ>::new(FramedWrite::new(
            transmit,
        ))));

        Client { receive, transmit }
    }
}

/// Sink implementation allows sending messages to the server
impl<T, REQ, RESP> Sink for Client<T, REQ, RESP>
where
    T: AsyncRead + AsyncWrite,
    REQ: Clone + Serialize,
{
    type SinkItem = REQ;
    type SinkError = IoError;

    fn start_send(
        &mut self,
        item: Self::SinkItem,
    ) -> Result<AsyncSink<Self::SinkItem>, Self::SinkError> {
        trace!("[daemon client] start send");
        self.transmit.lock().unwrap().start_send(item)
    }

    fn poll_complete(&mut self) -> Result<Async<()>, Self::SinkError> {
        trace!("[daemon client] send complete");
        self.transmit.lock().unwrap().poll_complete()
    }
}

/// Stream implementation allows receiving messages from the server
impl<T, REQ, RESP> Stream for Client<T, REQ, RESP>
where
    T: AsyncRead + AsyncWrite,
    for<'de> RESP: Clone + Deserialize<'de>,
{
    type Item = RESP;
    type Error = IoError;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        trace!("[daemon client] poll receive");
        self.receive.lock().unwrap().poll()
    }
}

#[cfg(test)]
mod tests {
    use tokio::prelude::*;
    use tokio_uds::{UnixStream};
    use ::Client;
    use tokio::{spawn, run};

    #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
    struct Test {
        text: String,
    }

    #[test]
    fn client_ping_pong() {
        let test = future::lazy(move || {
            // Build client pair
            let (a, b) = UnixStream::pair().unwrap();
            let client_a = Client::<_, Test, Test>::from(a);
            let client_b = Client::<_, Test, Test>::from(b);

            // Send a message
            let t = Test {
                text: "test string".to_owned(),
            };
            client_a.send(t.clone()).wait().unwrap();

            // Receive a message
            // TODO: this should be, receive ONE message
            // Maybe a once + a timeout would work here?
            let rx_handle = client_b
                .for_each(move |m| {
                    trace!("Received message: {:?}", m);
                    assert_eq!(t, m);
                    Ok(())
                }).map_err(|_e| ());
            spawn(rx_handle);

            Ok(())
        }).map(|_e| ());

        run(test);
    }
}
