
use std::sync::{Arc, Mutex};
use std::io::Error as IoError;

use bytes::BytesMut;
use futures::prelude::*;

use tokio_uds::{UnixStream};
use tokio_io::codec::length_delimited;

use error::DaemonError;

/// Client implements a daemon client
/// This connects to an IPC socket and sends and receives messages from a connected daemon
#[derive(Debug)]
pub struct Client {
    pub socket: Arc<Mutex<length_delimited::Framed<UnixStream>>>,
}

impl Clone for Client {
    fn clone(&self) -> Self {
        Client{socket: self.socket.clone()}
    }
}

impl Client {
    /// New creates a new client connected to the provided IPC socket
    pub fn new(path: &str) -> Result<Client, DaemonError> {
        println!("[daemon client] creating connection (socket: {})", path);
        // Create the socket future
        let socket = UnixStream::connect(path).wait()?;
        // Wrap to a length delimited framed socket
        let socket = length_delimited::Framed::new(socket);

        Ok(Client { socket: Arc::new(Mutex::new(socket)) })
    }

    /// Fetch the stream associated with the client
    pub fn split(&self) -> (Outgoing, Incoming){
        (Outgoing{inner: self.clone()}, Incoming{inner: self.clone()})
    }

    /// Close consumes the client connector and closes the socket
    pub fn close(self) -> Result<(), DaemonError> {
        println!("[daemon client] closing connection");
        Ok(())
    }
}

pub struct Incoming {
    inner: Client
}

impl Stream for Incoming {
    type Item = BytesMut;
    type Error = IoError;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        println!("[daemon client] poll");
        self.inner.socket.lock().unwrap().poll()
    }
}

pub struct Outgoing {
    inner: Client
}

impl Sink for Outgoing {
    type SinkItem = BytesMut;
    type SinkError = IoError;

    fn start_send(&mut self, item: Self::SinkItem) -> Result<AsyncSink<Self::SinkItem>, Self::SinkError> {
        println!("[daemon client] start_send");
         self.inner.socket.lock().unwrap().start_send(item)
    }

    fn poll_complete(&mut self) -> Result<Async<()>, Self::SinkError> {
        println!("[daemon client] poll_complete");
        self.inner.socket.lock().unwrap().poll_complete()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, fs};
    use std::time::{Duration, Instant};
    use std::net::Shutdown;

    use tokio::prelude::*;
    use tokio::io::*;
    use tokio::runtime::Builder;
    use tokio::executor::thread_pool;
    use tokio_uds::{UnixListener, UnixStream};
    use Client;

    use tempfile::NamedTempFile;

    #[test]
    fn client_ping_pong() {
        let mut threadpool_builder = thread_pool::Builder::new();
        threadpool_builder
            .name_prefix("my-runtime-worker-")
            .pool_size(4);
        
        // build Runtime
        let mut runtime = Builder::new()
            .threadpool_builder(threadpool_builder)
            .build().unwrap();

        let path = format!("{}rust-daemon4.sock", env::temp_dir().to_str().unwrap());
        println!("[TEST] Socket: {}", path);

        let listener = UnixListener::bind(&path).unwrap();;
        let client = Client::new(&path).unwrap();

        let tokio_server = listener
            .incoming()
            .for_each(move |socket| {
                let (reader, writer) = socket.split();
                let amt = copy(reader, writer);

                let msg = amt.then(move |result| {
                    match result {
                        Ok((amt, _, _)) => println!("wrote {} bytes", amt),
                        Err(e) => println!("error on: {}", e),
                    }

                    Ok(())
                });

                tokio::spawn(msg);

                Ok(())
            })
            .map_err(|err| {
                println!("[daemon server] accept error: {}", err);
            });
        runtime.spawn(tokio_server);

        runtime.spawn(future::lazy(move ||{
            let (mut tx, mut rx) = client.split();

            println!("[TEST] Writing Data");
            let out = "abcd1234\n";
            tx.send(BytesMut::from(out)).wait().unwrap();

            std::thread::sleep(Duration::from_secs(2));

            println!("[TEST] Reading Data");
            let client_handle = rx.for_each(move |d| {
                println!("client incoming: {:?}", d);
                assert_eq!(d, out.as_bytes());
                Ok(())
            }).map_err(|_e| () );
            tokio::spawn(client_handle);

            Ok(())
        }));

        std::thread::sleep(Duration::from_secs(2));

        //runtime.shutdown_now().wait().unwrap();

        let _e = fs::remove_file(path);

    }
}