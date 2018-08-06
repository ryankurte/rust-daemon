use std::io::Error as IoError;
use std::io::ErrorKind as IoErrorKind;

use std::ffi::{CString};

use users::{get_group_by_gid, get_user_by_uid};

use libc::{c_int, getgrouplist, getpeereid, gid_t, uid_t};
use std::os::unix::io::{AsRawFd};

/// User object for asserting permissions
#[derive(Debug, PartialEq, Clone)]
pub struct User {
    pub id: u32,
    pub group_id: u32,
    pub name: String,
    pub groups: Vec<String>,
}

impl User {
    /// Create a user object from a given UID
    pub fn from_uid(uid: u32) -> Result<User, IoError> {
        let user = get_user_by_uid(uid).unwrap();
        let groups = get_groups(user.name(), user.primary_group_id() as i32).unwrap();

        Ok(User {
            id: uid,
            name: user.name().to_string(),
            group_id: user.primary_group_id(),
            groups: groups,
        })
    }

    /// Create a user object from a given unix socket file descriptor
    pub fn from_fd<FD>(fd: &FD) -> Result<User, IoError>
    where
        FD: AsRawFd,
    {
        let (uid, _gid) = get_fd_effective_uid_gid(fd)?;
        Self::from_uid(uid)
    }
}

/// Fetches the effective uid and gid for a file descriptor
/// This is particularly useful with unix domain sockets
fn get_fd_effective_uid_gid<FD>(fd: &FD) -> Result<(u32, u32), IoError>
where
    FD: AsRawFd,
{
    let cid: c_int = fd.as_raw_fd();
    let mut uid: uid_t = 0;
    let mut gid: gid_t = 0;

    unsafe {
        let res = getpeereid(cid, &mut uid, &mut gid);
        if res < 0 {
            return Err(IoError::new(
                IoErrorKind::Other,
                format!("libc::getpeerid error: {}", res),
            ));
        }
    }

    Ok((uid, gid))
}

/// Fetch groups for a given username and primary group id
fn get_groups(username: &str, gid: i32) -> Result<Vec<String>, IoError> {
    unsafe {
        let mut groups: Vec<i32> = vec![0; 1024];

        let name = CString::new(username).unwrap();
        let mut count = groups.len() as c_int;

        let res = getgrouplist(name.as_ptr(), gid, groups.as_mut_ptr(), &mut count);
        if res < 0 {
            return Err(IoError::new(
                IoErrorKind::Other,
                format!("libc::getgrouplist error: {}", res),
            ));
        }

        let mut names: Vec<String> = Vec::new();
        for i in 0..count {
            let g = get_group_by_gid(groups[i as usize] as u32);
            match g {
                Some(g) => names.push(g.name().to_string()),
                None => (),
            }
        }

        Ok(names)
    }
}

#[cfg(test)]
mod tests {
    use user::User;

    use users::{get_current_uid};

    #[test]
    fn test_get_groups() {
        let uid = get_current_uid();
        let user = User::from_uid(uid).unwrap();
        println!("Groups: {:?}", user.groups);
        assert!(user.groups.len() > 0);
    }
}
