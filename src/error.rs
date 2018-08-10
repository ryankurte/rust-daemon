/**
 * rust-daemon
 * Error types
 * 
 * https://github.com/ryankurte/rust-daemon
 * Copyright 2018 Ryan Kurte
 */


use std::io::Error as IoError;

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
