

use std::io::Error as IoError;
use std::io::ErrorKind as IoErrorKind;

use std::mem;
use std::ffi::{CStr, CString};

use users::{get_user_by_uid, get_group_by_gid};

use libc::{uid_t, gid_t, c_int, getpeereid, getpwuid_r, getgrouplist};
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd};

/// Fetches the user and group names for a given file descriptor
pub fn get_fd_user_groups<FD>(fd: FD) -> Result<(String, Vec<String>), IoError> 
where FD: AsRawFd {
    let (uid, gid) = get_fd_effective_uid_gid(fd)?;
    let user = get_user_by_uid(uid).unwrap();
    let groups = get_groups(user.name(), user.primary_group_id() as i32).unwrap();
    Ok((user.name().to_string(), groups))
}

/// Fetches the effective uid and gid for a file descriptor
/// This is particularly useful with unix domain sockets
fn get_fd_effective_uid_gid<FD>(fd: FD) -> Result<(u32, u32), IoError>
where FD: AsRawFd {
    let cid: c_int = fd.as_raw_fd();
    let mut uid: uid_t = 0;
    let mut gid: gid_t = 0;
    
    unsafe {
        let res = getpeereid(cid, &mut uid, &mut gid);
        if res < 0 {
            return Err(IoError::new(IoErrorKind::Other, format!("libc::getpeerid error: {}", res)));
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
            return Err(IoError::new(IoErrorKind::Other, format!("libc::getgrouplist error: {}", res)));
        }

        let mut names: Vec<String> = Vec::new();
        for i in 0..count {
            let g = get_group_by_gid(groups[i as usize] as u32);
            match g {
                Some(g) => names.push(g.name().to_string()),
                None => ()
            }
        }

        Ok(names)
    }
}

#[cfg(test)]
mod tests {
    use permissions::get_groups;

    use users::{get_current_uid, get_user_by_uid};

    #[test]
    fn test_get_groups() {
        let uid = get_current_uid();
        let user = get_user_by_uid(uid).unwrap();
        let groups = get_groups(user.name(), user.primary_group_id() as i32).unwrap();
        println!("Groups: {:?}", groups);
        assert!(groups.len() > 0);
    }
}
