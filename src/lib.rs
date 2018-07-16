

#[macro_use]
extern crate log;
extern crate libc;
extern crate users;
#[macro_use]
extern crate serde_derive;
extern crate serde;

use std::io::{Write, Read};

use std::io::Error as IoError;
use std::io::ErrorKind as IoErrorKind;
use std::ops::Deref;
use std::fs;

use std::thread;
use std::thread::{JoinHandle};
use std::net::Shutdown;

use std::os::unix::net::{UnixStream, UnixListener};


mod permissions;
use permissions::get_fd_user_groups;

#[derive(Debug)]
pub enum DaemonError {
    IoError(IoError),
    GetPeerIdError(usize),
}

impl From<IoError> for DaemonError {
    fn from(e: IoError) -> DaemonError {
        return DaemonError::IoError(e);
    }
}

// none_acl implements an ACL that always returns true
pub fn none_acl(_user: &String, _groups: &[String]) -> bool { true }

pub struct Server {
    path: String,
    listener: UnixListener,
    acl: Box<FnMut(&String, &[String]) -> bool>,
    children: Vec<JoinHandle<()>>,
}

pub struct Message<T, V> {
    id: T,
    size: T,
    data: V,
}

pub struct User {
    id: usize,
    name: String,
    groups: Vec<String>,
}

pub type MessageId = usize;

pub enum Event {
    Connect(User),
    Disconnect(User),
    Message(User, MessageId, Vec<u8>)
}

impl Server {
    pub fn new<ACL: 'static + FnMut(&String, &[String]) -> bool>(path: &str, acl: ACL) -> Result<Server, DaemonError> {
        info!("[daemon] creating server");
        fs::remove_file(path)?;

        let listener = UnixListener::bind(path)?;

        Ok(Server{path: path.to_string(), listener, acl: Box::new(acl), children: Vec::new()})
    }

    pub fn run(&mut self) {
        info!("[daemon] starting socket server");
        for stream in self.listener.incoming() {
            match stream {
                Ok(mut s) => {
                    // Fetch user and group memberships
                    let (user, groups) = get_fd_user_groups(&s).unwrap();
                    
                    // Run connect ACL and skip connecting if ACL denied
                    if !(self.acl)(&user, groups.deref()) {
                        info!("[daemon] acl denied connection to user: {}", user);
                        continue;
                    }

                    // Spawn polling thread
                    let child = thread::spawn(move || {
                        loop {
                            let mut buff = [0u8; 1024];
                            let n = s.read(&mut buff).unwrap();
                            s.write(&buff[0..n]);
                        }
                    });

                    // Save thread handle
                    self.children.push(child);
                }
                Err(err) => {
                    info!("[daemon] socker listener error: {}", err);
                    break;
                }
            }
        }
    }

    pub fn close(mut self) -> Result<(), DaemonError>{
        info!("Closing socket server");

        // Stop listener thread

        // Close listener socket?
        //self.listener.shutdown(Shutdown::Both)?;

        // Close open sockets / threads
        let results: Vec<_> = self.children.into_iter().map(|c| c.join() ).collect();

        fs::remove_file(self.path)?;

        Ok(())
    }
}

pub struct Client {
    socket: UnixStream
}

impl Client {
    pub fn new(path: &str) -> Result<Client, DaemonError> {
        info!("[daemon] creating client connection");
        let socket = UnixStream::connect(path)?;

        Ok(Client{socket})
    }

    pub fn send(&mut self, data: &[u8]) -> Result<(), DaemonError> {
        self.socket.write(data)?;
        Ok(())
    }

    pub fn receive<'a>(&mut self, buff: &'a mut [u8]) -> Result<&'a [u8], DaemonError> {
        self.socket.read(buff)?;
        Ok(buff)
    }

    pub fn close(mut self) -> Result<(), DaemonError>{
        info!("[daemon] closing client connection");
        self.socket.shutdown(Shutdown::Both)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use ::{none_acl, Server};

    #[test]
    fn it_works() {
        let socket = format!("{}rust-daemon.sock", env::temp_dir().to_str().unwrap());
        println!("Socket: {}", socket);

        let server = Server::new(&socket, none_acl).unwrap();

        server.close().unwrap();
    }
}
