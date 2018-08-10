/**
 * rust-daemon
 * Server implementation
 * 
 * https://github.com/ryankurte/rust-daemon
 * Copyright 2018 Ryan Kurte
 */

use std::fs;
use std::io::Error as IoError;
use std::sync::{Arc, Mutex};
use std::fmt::Debug;

use futures::sync::mpsc;

use tokio::prelude::*;
use tokio::io::copy;
use tokio_uds::{UnixListener, UnixStream};
use tokio_io::codec::length_delimited::{FramedRead, FramedWrite};
use tokio_serde_json::{ReadJson, WriteJson};

use serde::{Deserialize, Serialize};

use uuid::Uuid;

use client::Client;
use error::DaemonError;
use user::User;

/// Server implements a daemon server
/// This creates an IPC socket and listens for messages from connected clients
pub struct Server<REQ, RESP> {
    path: String,
    connections: Arc<Mutex<Vec<Connection<REQ, RESP>>>>,
    incoming: mpsc::UnboundedReceiver<Request<REQ, RESP>>,
}

impl<REQ, RESP> Server<REQ, RESP>
where
    for<'de> REQ: Serialize + Deserialize<'de> + Clone + Send + Debug + 'static,
    for<'de> RESP: Serialize + Deserialize<'de> + Clone + Send + Debug + 'static,
{
    /// Create a new server with the defined Request and Response types
    /// This starts a new listening thread using the provided runtime handle
    /// and provides a Source of Requests from connected clients
    pub fn new(path: &str) -> Result<Server<REQ, RESP>, DaemonError> {
        println!("[daemon server] creating server (socket: {})", path);
        let _res = fs::remove_file(path);

        // Create listener and client list
        let listener = UnixListener::bind(path)?;
        let connections = Arc::new(Mutex::new(Vec::new()));

        let (tx, rx) = mpsc::unbounded::<Request<REQ, RESP>>();

        // Handle incoming connections
        let client_list = connections.clone();
        let tokio_server = listener
            .incoming()
            .for_each(move |socket| {
                // Fetch user info
                let p = socket.peer_cred().unwrap();
                let u = User::from_uid(p.uid).unwrap();
                
                // Create client connection
                let client = Client::<_, RESP, REQ>::from(socket);
                let conn = Connection::<REQ, RESP>::new(u, client.clone());

                println!(
                    "[daemon server] new connection user: '{}' id: '{}'",
                    conn.user.name, conn.id
                );

                // Add to client list
                client_list.lock().unwrap().push(conn.clone());
                let incoming = tx.clone();

                // Handle incoming requests
                let rx_handle = client.for_each(move |req| {
                    println!("[daemon server] client rx: {:?}", req);
                    let r = Request{
                        inner: conn.clone(),
                        data: req,
                    };
                    incoming.clone().send(r).wait().unwrap();

                    Ok(())
                }).map_err(|e| panic!("error: {}", e) );
                tokio::spawn(rx_handle);
                
                Ok(())
            })
            .map_err(|err| {
                println!("[daemon server] accept error: {}", err);
            });

        let s = Server {
            path: path.to_string(),
            connections,
            incoming: rx,
        };

        tokio::spawn(tokio_server);

        Ok(s)
    }

    pub fn incoming<'a>(self) -> mpsc::UnboundedReceiver<Request<REQ, RESP>> {
        self.incoming
    }

    pub fn close(self) {
        println!("[daemon server] closing socket server");

        // Close open sockets / threads
        let mut connections = self.connections.lock().unwrap();
        let _results: Vec<_> = connections.drain(0..).collect();

        let _e = fs::remove_file(self.path);
    }
}

/// Request is a request from a client
/// This contains the Request data and is a Sink for Response data
pub struct Request<REQ, RESP> {
    inner: Connection<REQ, RESP>,
    data: REQ,
}

/// Allows requests to be cloned
impl<REQ, RESP> Request<REQ, RESP>
where
    REQ: Clone,
{
    // Fetch the data for a given request
    pub fn data(&self) -> REQ {
        self.data.clone()
    }
}

/// Sink implementation allows responses to requests
impl<REQ, RESP> Sink for Request<REQ, RESP>
where
    RESP: Clone + Serialize,
{
    type SinkItem = RESP;
    type SinkError = IoError;

    fn start_send(
        &mut self,
        item: Self::SinkItem,
    ) -> Result<AsyncSink<Self::SinkItem>, Self::SinkError> {
        println!("[daemon request] start send");
        self.inner.client.start_send(item)
    }

    fn poll_complete(&mut self) -> Result<Async<()>, Self::SinkError> {
        println!("[daemon request] send complete");
        self.inner.client.poll_complete()
    }
}

/// Connection defined a connected client with associated user and a randomly allocated ID
#[derive(Clone)]
struct Connection<REQ, RESP> {
    pub id: String,
    pub user: User,
    pub client: Client<UnixStream, RESP, REQ>,
}

impl<REQ, RESP> Connection<REQ, RESP>
where
    for<'de> REQ: Serialize + Deserialize<'de> + Clone + Send + 'static,
    for<'de> RESP: Serialize + Deserialize<'de> + Clone + Send + 'static,
{
    pub fn new(user: User, client: Client<UnixStream, RESP, REQ>) -> Connection<REQ, RESP> {
        Connection {
            id: Uuid::new_v4().to_string(),
            user,
            client,
        }
    }
}
