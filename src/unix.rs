/**
 * rust-daemon
 * Unix Server and Connection Implementations
 *
 * https://github.com/ryankurte/rust-daemon
 * Copyright 2018 Ryan Kurte
 */

use std::fs;
use std::fmt::{Debug};
use std::clone::{Clone};
use libc::{gid_t, uid_t};

use tokio::prelude::*;
use tokio::spawn;
use tokio_codec::{Encoder, Decoder};

use tokio_uds::{UnixListener, UnixStream};

use server::Server;
use connection::Connection;
use error::Error;

use users::{User, Group, get_group_by_gid, get_user_by_uid};

/// UnixServer is a Server implementation over UnixStream and UnixInfo types with a generic codec
/// ```no_run
/// extern crate tokio;
/// use tokio::prelude::*;
/// use tokio::{spawn, run};
/// 
/// #[macro_use]
/// extern crate serde_derive;
/// 
/// extern crate daemon_engine;
/// use daemon_engine::{UnixServer, JsonCodec};
/// 
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct Request {}
/// 
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct Response {}
/// 
/// # fn main() {
/// 
/// let addr = "/var/tmp/test-daemon.sock";
/// let server = future::lazy(move || {
///     let mut s = UnixServer::<JsonCodec<Response, Request>>::new(&addr, JsonCodec::new()).unwrap();
///     let server_handle = s
///         .incoming()
///         .unwrap()
///         .for_each(move |r| {
///             println!("Request data {:?} info: {:?}", r.data(), r.info());
///             r.send(Response{}).wait().unwrap();
///             Ok(())
///         }).map_err(|_e| ());
///     spawn(server_handle);
///     Ok(())
/// });
/// run(server);
/// 
/// # }
/// ```
pub type UnixServer<C> = Server<UnixStream, C, UnixInfo>;

/// UnixConnection is a Connection implementation over UnixStream
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
/// use daemon_engine::{UnixConnection, JsonCodec, DaemonError};
/// 
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct Request {}
/// 
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct Response {}
/// 
/// # fn main() {
/// let addr = "/var/tmp/test-daemon.sock";
/// let client = UnixConnection::<JsonCodec<Request, Response>>::new(&addr, JsonCodec::new()).unwrap();
/// let (tx, rx) = client.split();
/// 
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
pub type UnixConnection<C> = Connection<UnixStream, C>;

impl <C> UnixConnection<C> 
where
    C: Encoder + Decoder + Clone + Send + 'static,
    <C as Decoder>::Item: Send,
    <C as Decoder>::Error: Send + Debug,
{
    /// Create a new client connected to the provided unix socket address
    pub fn new(path: &str, codec: C) -> Result<UnixConnection<C>, Error> {
        trace!("[connector] creating connection (unix path: {})", path);
        // Create the socket future
        let socket = UnixStream::connect(&path).wait()?;
        // Create the socket instance
        Ok(Connection::from_socket(socket, codec))
    }


    pub fn close(self) {
        
    }
}

/// UnixInfo is an information object associated with a given UnixServer connection.
/// 
/// This is passed to the server request handler to allow ACLs and connection tracking
#[derive(Clone, Debug)]
pub struct  UnixInfo {
    pub user: User,
    pub group: Group,
}

impl UnixInfo {
    pub fn new(uid: uid_t, gid: gid_t) -> UnixInfo {
        let user = get_user_by_uid(uid).unwrap();
        let group = get_group_by_gid(gid).unwrap();
        UnixInfo{user, group}
    }
}

/// Unix server implementation
/// 
/// This binds to and listens on a unix domain socket
impl<C> UnixServer<C>
where
    C: Encoder + Decoder + Clone + Send + 'static,
    <C as Decoder>::Item: Clone + Send + Debug,
    <C as Decoder>::Error: Send + Debug,
    <C as Encoder>::Item: Clone + Send + Debug,
    <C as Encoder>::Error: Send + Debug,
{
    pub fn new(path: &str, codec: C) -> Result<UnixServer<C>, Error> {
        // Pre-clear socket file
        let _res = fs::remove_file(&path);

        // Create base server instance
        let server = Server::base(codec);

        // Create listener socket
        let socket = UnixListener::bind(&path)?;

        let exit_rx = server.exit_rx.lock().unwrap().take();
        let mut server_int = server.clone();

        // Create listening thread
        let tokio_server = socket
            .incoming()
            .for_each(move |s| {
                let creds = s.peer_cred().unwrap();
                let info = UnixInfo::new(creds.uid, creds.gid);
                server_int.bind(info, s); 
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

    pub fn shutdown(&self) {

    }
}

