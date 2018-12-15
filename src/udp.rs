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

use futures::prelude::*;
use futures::sync::mpsc;
use futures::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use futures::sync::oneshot;

use tokio::prelude::*;
use tokio_codec::{Encoder, Decoder};
use tokio::spawn;
use tokio::net::udp::{UdpSocket, UdpFramed};

use connection::Connection;
use error::Error;


/// UdpConnection is a Connection implementation over UdpSocket
/// 
/// ```no_run
/// use std::net::{SocketAddr, IpAddr, Ipv4Addr};
/// 
/// extern crate tokio;
/// use tokio::prelude::*;
/// use tokio::{spawn, run};
/// 
/// #[macro_use]
/// extern crate serde_derive;
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
/// let client = UdpConnection::<JsonCodec<Request, Response>>::new(&addr).unwrap();
/// let (tx, rx) = client.split();
/// // Send data
/// tx.send(Request{}).wait().unwrap();
/// 
/// // Receive data
/// rx.map(|resp| -> Result<(), DaemonError> {
///    println!("Response: {:?}", resp);
///    Ok(())
/// }).wait().next();
/// # }
/// ```

pub type IncomingRx<REQ> = UnboundedReceiver<(REQ, SocketAddr)>;
pub type OutgoingTx<RESP> = UnboundedSender<(RESP, SocketAddr)>;

/// Connection type implemented on top of AsyncRead + AsyncWrite and an Encoder/Decoder
/// This provides a simple / generic base object for managing tokio connections
pub struct UdpConnection<CODEC: Encoder + Decoder> 
{
    incoming_rx: Arc<Mutex<Option<IncomingRx<<CODEC as Decoder>::Item>>>>,
    outgoing_tx: Arc<Mutex<OutgoingTx<<CODEC as Encoder>::Item>>>,
    pub(crate) exit_tx: Arc<Mutex<Option<(oneshot::Sender<()>, oneshot::Sender<()>)>>>,
}

impl <CODEC> From<UdpSocket> for UdpConnection<CODEC> 
where
    CODEC: Encoder + Decoder + Default + Send + 'static,
    <CODEC as Decoder>::Item: Send + Debug,
    <CODEC as Decoder>::Error: Send + Debug,
    <CODEC as Encoder>::Item: Send + Debug,
    <CODEC as Encoder>::Error: Send + Debug,
{
    fn from(socket: UdpSocket) -> UdpConnection<CODEC> {

        let framed = UdpFramed::new(socket, CODEC::default());
        let (tx, rx) = framed.split();

        let (incoming_tx, incoming_rx) = mpsc::unbounded::<_>();
        let (incoming_exit_tx, incoming_exit_rx) = oneshot::channel::<()>();

        // Handle incoming messages
        let rx_handle = rx.for_each(move |(data, addr)| {
            debug!("[udp connection] receive from: '{:?}' data: '{:?}'", addr, data);
            incoming_tx.clone().send((data, addr));
            Ok(())
        })
        .map_err(|e| panic!("[udp connection] error: {:?}", e))
        .select2(incoming_exit_rx)
        .then(|_| {
            info!("[udp connection] closing incoming handler");
            Ok(())
        });
        spawn(rx_handle);

        let (outgoing_tx, outgoing_rx) = mpsc::unbounded::<_>();
        let (outgoing_exit_tx, outgoing_exit_rx) = oneshot::channel::<()>();

        let tx_handle = outgoing_rx.for_each(move |(data, addr)| {
            debug!("[udp connection] send to: '{:?}' data: '{:?}'", addr, data);
            let _r = tx.start_send((data, addr));
            Ok(())
        })
        .select2(outgoing_exit_rx)
        .then(|_| {
            info!("[udp connection] closing outgoing handler");
            Ok(())
        });;
        spawn(tx_handle);
        

        // Build connection object
        UdpConnection{
            incoming_rx: Arc::new(Mutex::new(Some(incoming_rx))),
            outgoing_tx: Arc::new(Mutex::new(outgoing_tx)),
            exit_tx: Arc::new(Mutex::new(Some((incoming_exit_tx, outgoing_exit_tx)))),
        }
    }
}

impl <CODEC> UdpConnection<CODEC> 
where
    CODEC: Encoder + Decoder + Default + Send + 'static,
    <CODEC as Decoder>::Item: Send,
    <CODEC as Decoder>::Error: Send + Debug,
{
    /// Take the incoming data handle.
    /// 
    /// You can then use `for_each` to iterate over received requests as in
    /// the examples
    pub fn incoming(&mut self) -> Option<IncomingRx<<CODEC as Decoder>::Item>> {
        self.incoming_rx.lock().unwrap().take()
    }

    pub fn send(&mut self, addr: SocketAddr, data: <CODEC as Encoder>::Item) {
        let unlocked = self.outgoing_tx.lock().unwrap();

        let _err = unlocked.unbounded_send((data, addr));
    }

    fn shutdown(self) {
        // Close the stream
        //self.socket.lock().unwrap().get_mut().shutdown().unwrap()
    }
}

/// Blank send
unsafe impl<CODEC> Send for UdpConnection<CODEC> 
where
    CODEC: Encoder + Decoder + Default, 
{}

/// UdpInfo is an information object associated with a given UdpServer connection.
/// This is passed to the server request handler to allow ACLs and connection tracking
#[derive(Clone, Debug)]
pub struct UdpInfo {
    pub address: SocketAddr,
}