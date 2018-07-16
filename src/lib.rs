

#[macro_use]
extern crate log;
extern crate libc;

use std::io::{Write, Read};

use std::io::Error as IoError;
use std::io::ErrorKind as IoErrorKind;

use std::thread;
use std::net::Shutdown;

use std::os::unix::net::{UnixStream, UnixListener};
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd};

use std::mem;
use std::ffi::{CStr, CString};

use libc::{uid_t, gid_t, c_int, getpwuid_r};

pub enum DaemonError {
    IoError(IoError),
    GetPeerIdError(usize),
}

impl From<IoError> for DaemonError {
    fn from(e: IoError) -> DaemonError {
        return DaemonError::IoError(e);
    }
}


pub struct Server<ACL> {
    listener: UnixListener,
    acl: Option<ACL>
}

fn get_uid_gid(cid: i32) -> Result<(u32, u32), IoError> {
    let mut uid: uid_t = 0;
    let mut gid: gid_t = 0;

    unsafe {
        let res = libc::getpeereid(cid, &mut uid, &mut gid);
        if res < 0 {
            return Err(IoError::new(IoErrorKind::Other, format!("libc::getpeerid error: {}", res)));
        }
    }

    Ok((uid, gid))
}

struct User {
    name: String,
    gid: u32,
}

impl User {
    fn from_uid(uid: u32) -> Result<User, IoError> {
        unsafe {
            let mut passwd: libc::passwd = mem::uninitialized();;
            let mut result: *mut libc::passwd = mem::uninitialized();;
            let mut buff = vec![0i8; 1024];

            let res = libc::getpwuid_r(uid, &mut passwd, buff.as_mut_ptr(), buff.len(), &mut result);
            if res < 0 {
                return Err(IoError::new(IoErrorKind::Other, format!("libc::getpwuid_r error: {}", res)));
            }

            let name = CStr::from_ptr(passwd.pw_name).to_str().unwrap();
            return Ok(User{name: name.to_string(), gid: passwd.pw_gid})
        }
    }

    fn get_groups(&mut self) -> Result<Vec<String>, IoError> {
        unsafe {
            let mut groups: Vec<i32> = vec![0; 1024];

            let username = CString::new(self.name.as_str()).unwrap();
            let gid = self.gid as i32;
            let mut count = groups.len() as c_int;

            let res = libc::getgrouplist(username.as_ptr(), gid, groups.as_mut_ptr(), &mut count);
            if res < 0 {
                return Err(IoError::new(IoErrorKind::Other, format!("libc::getgrouplist error: {}", res)));
            }

            let mut names: Vec<String> = Vec::new();
            for i in 0..count {

            }

            Ok(names)
        }
    }

}



impl <ACL>Server<ACL>
    where ACL: FnMut(usize, usize) -> bool
{
    pub fn new(path: &str) -> Result<Server<ACL>, DaemonError> {
        info!("[daemon] creating server");
        let listener = UnixListener::bind(path)?;



        Ok(Server{listener, acl: None})
    }

    pub fn run(&mut self) {
        info!("[daemon] starting socket server");
        for stream in self.listener.incoming() {
            match stream {
                Ok(stream) => {
                    let cid: c_int = stream.as_raw_fd();
                    let (uid, gid) = get_uid_gid(cid).unwrap();

                    
                    

                    thread::spawn(|| {
                        
                    });
                }
                Err(err) => {
                    break;
                }
            }
        }
    }

    pub fn close(&self) -> Result<(), DaemonError>{
        info!("Closing socket server");
        // Close listener socket?
        // self.listener.shutdown(Shutdown::Both)?;

        // Close open sockets

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

    pub fn close(&self) -> Result<(), DaemonError>{
        info!("[daemon] closing client connection");
        self.socket.shutdown(Shutdown::Both)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
