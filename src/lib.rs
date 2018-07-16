

#[macro_use]
extern crate log;
extern crate libc;
extern crate users;

use std::io::{Write, Read};

use std::io::Error as IoError;
use std::io::ErrorKind as IoErrorKind;
use std::ops::Deref;

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

pub type ACL = FnMut(&String, &[String]) -> bool;

pub fn NoACL(_user: &String, _groups: &[String]) -> bool { true }

pub struct Server {
    listener: UnixListener,
    acl: Box<ACL>,
    children: Vec<JoinHandle<()>>,
}

impl Server {
    pub fn new<T: 'static + FnMut(&String, &[String]) -> bool>(path: &str, acl: T) -> Result<Server, DaemonError> {
        info!("[daemon] creating server");
        let listener = UnixListener::bind(path)?;

        Ok(Server{listener, acl: Box::new(acl), children: Vec::new()})
    }

    pub fn run(&mut self) {
        info!("[daemon] starting socket server");
        for stream in self.listener.incoming() {
            match stream {
                Ok(stream) => {
                    // Fetch user and group memberships
                    let (user, groups) = get_fd_user_groups(stream).unwrap();
                    
                    // Run connect ACL and skip connecting if ACL denied
                    if !(self.acl)(&user, groups.deref()) {
                        info!("[daemon] acl denied connection to user: {}", user);
                        continue;
                    }

                    // Spawn polling thread
                    let child = thread::spawn(|| {
                        
                    });

                    // Save thread handle
                    self.children.push(child);
                }
                Err(err) => {
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
    use ::{ACL, NoACL, Server};

    #[test]
    fn it_works() {
        let socket = format!("{}/rust-daemon.sock", env::temp_dir().to_str().unwrap());

        let server = Server::new(&socket, NoACL).unwrap();

        server.close().unwrap();
    }
}
