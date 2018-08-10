/**
 * rust-daemon
 * User module for access control use
 *
 * https://github.com/ryankurte/rust-daemon
 * Copyright 2018 Ryan Kurte
 */
use std::io::Error as IoError;
use std::io::ErrorKind as IoErrorKind;

use std::ffi::CString;

use users::{get_group_by_gid, get_user_by_uid};

use libc::{c_int, getgrouplist, gid_t, uid_t};

/// User object for asserting permissions
#[derive(Debug, PartialEq, Clone)]
pub struct User {
    pub id: uid_t,
    pub group_id: uid_t,
    pub name: String,
    pub groups: Vec<String>,
}

impl User {
    /// Create a user object from a given UID
    pub fn from_uid(uid: uid_t) -> Result<User, IoError> {
        let user = get_user_by_uid(uid).unwrap();
        let groups = get_groups(user.name(), user.primary_group_id() as gid_t).unwrap();

        Ok(User {
            id: uid,
            name: user.name().to_string(),
            group_id: user.primary_group_id(),
            groups: groups,
        })
    }
}

/// Fetch groups for a given username and primary group id
fn get_groups(username: &str, gid: gid_t) -> Result<Vec<String>, IoError> {
    unsafe {
        let mut groups: Vec<gid_t> = vec![0; 1024];

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

    use users::get_current_uid;

    #[test]
    fn test_get_groups() {
        let uid = get_current_uid();
        let user = User::from_uid(uid).unwrap();
        println!("Groups: {:?}", user.groups);
        assert!(user.groups.len() > 0);
    }
}
