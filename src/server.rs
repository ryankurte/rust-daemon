use std::fs;
use std::sync::{Arc, Mutex};
use std::io::Error as IoError;

use uuid::Uuid;

use tokio::io::{ReadHalf, WriteHalf};
use tokio::prelude::*;
use tokio::runtime::Runtime;
use tokio_uds::{UnixListener, UnixStream};

use bytes::BytesMut;
use tokio_io::codec::length_delimited;

use error::DaemonError;
use user::User;

pub struct Connection {
    pub id: String,
    pub user: User,
    pub receive: length_delimited::FramedRead<ReadHalf<UnixStream>>,
    pub transmit: length_delimited::FramedWrite<WriteHalf<UnixStream>>,
}

impl Connection {
    pub fn new(user: User, socket: UnixStream) -> Self {
        let (receive, transmit) = socket.split();
        let receive = length_delimited::FramedRead::new(receive);
        let transmit = length_delimited::FramedWrite::new(transmit);
        Connection {
            id: Uuid::new_v4().to_string(),
            user,
            receive,
            transmit,
        }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {}
}

/// Server implements a daemon server
/// This creates an IPC socket and listens for messages from connected clients
pub struct Server {
    path: String,
    connections: Arc<Mutex<Vec<Connection>>>,
}

impl Server {
    pub fn new(handle: &mut Runtime, path: &str) -> Result<Server, DaemonError> {
        println!("[daemon server] creating server");
        let _res = fs::remove_file(path);

        // Create client rx sockets
        //let (tx, rx) = futures::sync::mpsc::unbounded();

        // Create listener and client list
        let listener = UnixListener::bind(path)?;
        let connections = Arc::new(Mutex::new(Vec::new()));

        // Handle incoming connections
        let client_list = connections.clone();
        let tokio_server = listener
            .incoming()
            .for_each(move |socket| {
                // Execute ACL
                let p = socket.peer_cred().unwrap();
                let u = User::from_uid(p.uid).unwrap();

                // Create connection
                let c = Connection::new(u, socket);

                // Add to client list
                client_list.lock().unwrap().push(c);

                Ok(())
            })
            .map_err(|err| {
                println!("[daemon server] accept error: {}", err);
            });

        let s = Server { path: path.to_string(), connections };

        handle.spawn(tokio_server);

        Ok(s)
    }



    pub fn incoming(self) -> Incoming{
        Incoming{inner: self}
    }

    pub fn close(self) -> Result<(), DaemonError> {
        println!("[daemon] closing socket server");

        // Close listener socket?
        //self.listener.shutdown(Shutdown::Both)?;

        // Close open sockets / threads
        let mut connections = self.connections.lock().unwrap();
        let _results: Vec<_> = connections.drain(0..).collect();

        let _e = fs::remove_file(self.path);

        Ok(())
    }
}

pub struct Incoming {
    inner: Server
}

impl Stream for Incoming {
    type Item = BytesMut;
    type Error = IoError;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        println!("[daemon server] poll");
        for c in self.inner.connections.lock().unwrap().iter_mut() {
            match c.receive.poll() {
                Ok(Async::Ready(t)) => {
                    return Ok(Async::Ready(t));
                }
                Ok(Async::NotReady) => {
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
