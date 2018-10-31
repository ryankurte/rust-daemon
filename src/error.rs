/**
 * rust-daemon
 * Error types
 *
 * https://github.com/ryankurte/rust-daemon
 * Copyright 2018 Ryan Kurte
 */
use std::io::Error as IoError;

#[derive(Debug)]
pub enum Error {
    IoError(IoError),
    GetPeerIdError(usize),
}

impl From<IoError> for Error {
    fn from(e: IoError) -> Error {
        return Error::IoError(e);
    }
}
