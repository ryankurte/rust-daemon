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
use std::sync::{Arc, Mutex};

use futures::sync::mpsc;
use futures::sync::mpsc::{UnboundedReceiver, UnboundedSender, SendError};
use futures::sync::oneshot;

use tokio::prelude::*;
use tokio_codec::{Encoder, Decoder};
use tokio::spawn;
use tokio::net::udp::{UdpSocket, UdpFramed};

use crate::error::Error;

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
/// use daemon_engine::{UdpConnection, JsonCodec, DaemonError};
/// 
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct Request {}
/// 
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct Response {}
/// 
/// # fn main() {
/// let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8111);
/// let client = UdpConnection::<JsonCodec<Request, Response>>::new(&addr, JsonCodec::new()).wait().unwrap();
/// let (tx, rx) = client.split();
/// // Send data
/// tx.send((Request{}, addr.clone())).wait().unwrap();
/// 
/// // Receive data
/// rx.map(|resp| -> Result<(), DaemonError> {
///    println!("Response: {:?}", resp);
///    Ok(())
/// }).wait().next();
/// # }
/// ```

pub type IncomingRx<Req> = UnboundedReceiver<(Req, SocketAddr)>;
pub type OutgoingTx<Resp> = UnboundedSender<(Resp, SocketAddr)>;

/// UdpConnection is a wrapper around UdpSocket to provide a consistent ish interface for UDP clients and servers.
/// TODO: Ideally it should be possible to genericise this and merge back with the Connection and Server components
#[derive(Clone)]
pub struct UdpConnection<Codec: Encoder + Decoder> 
{
    incoming_rx: Arc<Mutex<IncomingRx<<Codec as Decoder>::Item>>>,
    outgoing_tx: Arc<Mutex<OutgoingTx<<Codec as Encoder>::Item>>>,
    exit_tx: Arc<Mutex<Option<(oneshot::Sender<()>, oneshot::Sender<()>)>>>,
}

impl <Codec> UdpConnection<Codec> 
where
    Codec: Encoder + Decoder + Clone + Send + 'static,
    <Codec as Encoder>::Item: Send + Debug,
    <Codec as Encoder>::Error: Send + Debug,
    <Codec as Decoder>::Item: Send + Debug,
    <Codec as Decoder>::Error: Send + Debug,
{
    /// Create a new connection by binding to the specified UDP socket
    pub fn new(addr: &SocketAddr, codec: Codec) -> impl Future<Item=UdpConnection<Codec>, Error=Error> {
        debug!("[connector] creating connection (udp address: {})", addr);
        // Create the socket future
        let socket = match UdpSocket::bind(&addr) {
            Ok(s) => s,
            Err(e) => return futures::future::err(e.into()),
        };
        // Create the socket instance
        futures::future::ok(UdpConnection::from_socket(socket, codec))
    }

    /// Create a new connection from an existing udp socket
    fn from_socket(socket: UdpSocket, codec: Codec) -> UdpConnection<Codec> {
        let framed = UdpFramed::new(socket, codec);
        let (tx, rx) = framed.split();

        let (incoming_tx, incoming_rx) = mpsc::unbounded::<_>();
        let (incoming_exit_tx, incoming_exit_rx) = oneshot::channel::<()>();

        // Handle incoming messages
        let rx_handle = rx.for_each(move |(data, addr)| {
            trace!("[udp connection] receive from: '{:?}' data: '{:?}'", addr, data);
            incoming_tx.clone().send((data, addr)).map(|_| () ).map_err(|e| panic!("[udp connection] send error: {:?}", e))
        })
        .map_err(|e| panic!("[udp connection] error: {:?}", e))
        .select2(incoming_exit_rx)
        .then(|_| {
            debug!("[udp connection] closing incoming handler");
            Ok(())
        });
        spawn(rx_handle);

        let (outgoing_tx, outgoing_rx) = mpsc::unbounded::<_>();
        let (outgoing_exit_tx, outgoing_exit_rx) = oneshot::channel::<()>();

        let tx_handle = tx.send_all(outgoing_rx.map_err(|_| panic!() ))
        .select2(outgoing_exit_rx)
        .then(|_| {
            debug!("[udp connection] closing outgoing handler");
            Ok(())
        });
        spawn(tx_handle);

        // Build connection object
        UdpConnection{
            incoming_rx: Arc::new(Mutex::new(incoming_rx)),
            outgoing_tx: Arc::new(Mutex::new(outgoing_tx)),
            exit_tx: Arc::new(Mutex::new(Some((incoming_exit_tx, outgoing_exit_tx)))),
        }
    }

    pub fn send(&mut self, addr: SocketAddr, data: <Codec as Encoder>::Item) {
        let unlocked = self.outgoing_tx.lock().unwrap();

        let _err = unlocked.unbounded_send((data, addr));
    }

    pub fn shutdown(self) {
        // Send listener exit signal
        let tx = self.exit_tx.lock().unwrap().take().unwrap();
        let _ = tx.0.send(());
        let _ = tx.1.send(());

        // Close the stream
        //self.socket.lock().unwrap().get_mut().shutdown().unwrap()
    }
}

/// Blank send
unsafe impl<Codec> Send for UdpConnection<Codec> 
where
    Codec: Encoder + Decoder + Clone, 
{}

/// Sink implementation allows sending messages over a connection
impl<Codec> Sink for UdpConnection<Codec>
where
    Codec: Encoder + Decoder + Clone, 
{
    type SinkItem = (<Codec as Encoder>::Item, SocketAddr);
    type SinkError = SendError<(<Codec as Encoder>::Item, SocketAddr)>;

    fn start_send(
        &mut self,
        item: Self::SinkItem,
    ) -> Result<AsyncSink<Self::SinkItem>, Self::SinkError> {
        trace!("[connection] start send");
        self.outgoing_tx.lock().unwrap().start_send(item)
    }

    fn poll_complete(&mut self) -> Result<Async<()>, Self::SinkError> {
        trace!("[connection] send complete");
        self.outgoing_tx.lock().unwrap().poll_complete()
    }
}

/// Stream implementation allows receiving messages from a connection
impl<Codec> Stream for UdpConnection<Codec>
where
    Codec: Encoder + Decoder + Clone, 
{
    type Item = (<Codec as Decoder>::Item, SocketAddr);
    type Error = ();

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        trace!("[connection] poll receive");
        self.incoming_rx.lock().unwrap().poll()
    }
}

/// UdpInfo is an information object associated with a given UdpServer connection.
/// This is passed to the server request handler to allow ACLs and connection tracking
#[derive(Clone, Debug)]
pub struct UdpInfo {
    pub address: SocketAddr,
}
