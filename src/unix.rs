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
pub type UnixServer<C> = Server<UnixStream, C, UnixInfo>;

/// TcpClient is a Client implementation over TcpStream
pub type UnixConnection<C> = Connection<UnixStream, C>;

impl <C> UnixConnection<C> 
where
    C: Encoder + Decoder + Default + Send + 'static,
    <C as Decoder>::Item: Send,
    <C as Decoder>::Error: Send + Debug,
{
    /// Create a new client connected to the provided unix socket address
    pub fn new(path: &str) -> Result<UnixConnection<C>, Error> {
        trace!("[connector] creating connection (unix path: {})", path);
        // Create the socket future
        let socket = UnixStream::connect(&path).wait()?;
        // Create the socket instance
        Ok(Connection::from(socket))
    }
}

/// UnixInfo is an information object associated with a given UnixServer connection
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
/// This binds to a unix domain socket
impl<C> UnixServer<C>
where
    C: Encoder + Decoder + Default + Send + 'static,
    <C as Decoder>::Item: Clone + Send + Debug,
    <C as Decoder>::Error: Send + Debug,
    <C as Encoder>::Item: Clone + Send + Debug,
    <C as Encoder>::Error: Send + Debug,
{
    
    pub fn new_unix(path: &str) -> Result<UnixServer<C>, Error> {
        // Pre-clear socket file
        let _res = fs::remove_file(&path);

        // Create base server instance
        let server = Server::new();

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
}
