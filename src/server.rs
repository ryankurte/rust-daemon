use std::fs;
use std::io::Error as IoError;
use std::sync::{Arc, Mutex};

use tokio::prelude::*;
use tokio::runtime::Runtime;
use tokio_uds::UnixListener;

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
}

impl<REQ, RESP> Server<REQ, RESP>
where
    for<'de> REQ: Serialize + Deserialize<'de> + Clone + Send + 'static,
    for<'de> RESP: Serialize + Deserialize<'de> + Clone + Send + 'static,
{
    /// Create a new server with the defined Request and Response types
    /// This starts a new listening thread using the provided runtime handle
    /// and provides a Source of Requests from connected clients
    pub fn new(handle: &mut Runtime, path: &str) -> Result<Server<REQ, RESP>, DaemonError> {
        println!("[daemon server] creating server (socket: {})", path);
        let _res = fs::remove_file(path);

        // Create listener and client list
        let listener = UnixListener::bind(path)?;
        let connections = Arc::new(Mutex::new(Vec::new()));

        // Handle incoming connections
        let client_list = connections.clone();
        let tokio_server = listener
            .incoming()
            .for_each(move |socket| {
                // Fetch user info
                let p = socket.peer_cred().unwrap();
                let u = User::from_uid(p.uid).unwrap();

                // TODO: execute connect ACL

                // Create connection
                let client = Client::<RESP, REQ>::from_sock(socket);
                let conn = Connection::<REQ, RESP>::new(u, client);

                println!(
                    "[daemon server] new connection user: '{}' id: '{}'",
                    conn.user.name, conn.id
                );

                // Add to client list
                client_list.lock().unwrap().push(conn);

                Ok(())
            })
            .map_err(|err| {
                println!("[daemon server] accept error: {}", err);
            });

        let s = Server {
            path: path.to_string(),
            connections,
        };

        handle.spawn(tokio_server);

        Ok(s)
    }

    pub fn close(self) {
        println!("[daemon server] closing socket server");

        // Close open sockets / threads
        let mut connections = self.connections.lock().unwrap();
        let _results: Vec<_> = connections.drain(0..).collect();

        let _e = fs::remove_file(self.path);
    }
}



impl<REQ, RESP> Stream for Server<REQ, RESP>
where
    for<'de> REQ: Clone + Deserialize<'de>,
    RESP: Clone + Serialize,
{
    type Item = Request<REQ, RESP>;
    type Error = IoError;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        let mut connections = self.connections.lock().unwrap();
        println!("[daemon server] poll receive ({} connections)", connections.len());
        for c in connections.iter_mut() {
            match c.client.poll() {
                Ok(Async::Ready(t)) => {
                    println!("[daemon server] ready");
                    let r = Request {
                        inner: c.clone(),
                        request: t.unwrap(),
                    };
                    return Ok(Async::Ready(Some(r)));
                }
                Ok(Async::NotReady) => {
                    println!("[daemon server] not ready");
                    continue;
                }
                Err(e) => {
                    println!("Connection error: {}", e);
                }
            }
        }

        Ok(Async::NotReady)
    }
}

/// Request is a request from a client
/// This contains the Request data and is a Sink for Response data
pub struct Request<REQ, RESP> {
    inner: Connection<REQ, RESP>,
    request: REQ,
}

/// Allows requests to be cloned
impl<REQ, RESP> Request<REQ, RESP>
where
    REQ: Clone,
{
    // Fetch the data for a given request
    pub fn data(&self) -> REQ {
        self.request.clone()
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
    pub client: Client<RESP, REQ>,
}

impl<REQ, RESP> Connection<REQ, RESP>
where
    for<'de> REQ: Serialize + Deserialize<'de> + Clone + Send + 'static,
    for<'de> RESP: Serialize + Deserialize<'de> + Clone + Send + 'static,
{
    pub fn new(user: User, client: Client<RESP, REQ>) -> Connection<REQ, RESP> {
        Connection {
            id: Uuid::new_v4().to_string(),
            user,
            client,
        }
    }
}
