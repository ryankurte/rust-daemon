use std::io::Error as IoError;
use std::sync::{Arc, Mutex};

use tokio::io::{ReadHalf, WriteHalf};
use tokio::prelude::*;
use tokio_io::codec::length_delimited::{FramedRead, FramedWrite};
use tokio_uds::UnixStream;

use tokio_serde_json::{ReadJson, WriteJson};

use serde::{Deserialize, Serialize};

use error::DaemonError;

pub type Receive<T> = Arc<Mutex<ReadJson<FramedRead<ReadHalf<UnixStream>>, T>>>;
pub type Transmit<T> = Arc<Mutex<WriteJson<FramedWrite<WriteHalf<UnixStream>>, T>>>;

/// Client implements a client for communicating with a daemon service
/// This connects to an IPC socket and sends and receives messages from a connected daemon
/// This acts as a Sink for requests and a Source for responses
#[derive(Clone)]
pub struct Client<REQ, RESP> {
    pub(crate) receive: Receive<RESP>,
    pub(crate) transmit: Transmit<REQ>,
}

impl<REQ, RESP> Client<REQ, RESP>
where
    for<'de> REQ: Serialize + Deserialize<'de> + Clone + Send + 'static,
    for<'de> RESP: Serialize + Deserialize<'de> + Clone + Send + 'static,
{
    /// Create a new client connected to the provided unix socket
    pub fn new(path: &str) -> Result<Client<REQ, RESP>, DaemonError> {
        println!("[daemon client] creating connection (socket: {})", path);
        // Create the socket future
        let socket = UnixStream::connect(path).wait()?;
        // Create the socket instance
        Ok(Client::from_sock(socket))
    }

    /// Create a client from an existing UnixStream socket
    pub fn from_sock(socket: UnixStream) -> Client<REQ, RESP> {
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

    /// Split a client into Transmit and Receive streams
    pub fn split<'a>(&'a self) -> (Transmit<REQ>, Receive<RESP>) {
        (self.transmit.clone(), self.receive.clone())
    }

    /// Close consumes the client connector and closes the socket
    pub fn close(self) -> Result<(), DaemonError> {
        println!("[daemon client] closing connection");
        Ok(())
    }
}

/// Sink implementation allows sending messages to the server
impl<REQ, RESP> Sink for Client<REQ, RESP>
where
    REQ: Clone + Serialize,
{
    type SinkItem = REQ;
    type SinkError = IoError;

    fn start_send(
        &mut self,
        item: Self::SinkItem,
    ) -> Result<AsyncSink<Self::SinkItem>, Self::SinkError> {
        println!("[daemon client] start send");
        self.transmit.lock().unwrap().start_send(item)
    }

    fn poll_complete(&mut self) -> Result<Async<()>, Self::SinkError> {
        println!("[daemon client] send complete");
        self.transmit.lock().unwrap().poll_complete()
    }
}

/// Stream implementation allows receiving messages from the server
impl<REQ, RESP> Stream for Client<REQ, RESP>
where
    for<'de> RESP: Clone + Deserialize<'de>,
{
    type Item = RESP;
    type Error = IoError;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        println!("[daemon client] poll receive");
        self.receive.lock().unwrap().poll()
    }
}
