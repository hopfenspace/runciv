//! You can find the errors that can occur during server startup here

use std::fmt::{Display, Formatter};
use std::io;

/// The errors that can occur during server startup
#[derive(Debug)]
pub enum StartServerError {
    /// IO error that can occur
    IO(io::Error),
}

impl Display for StartServerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            StartServerError::IO(err) => write!(f, "{err}"),
        }
    }
}

impl From<io::Error> for StartServerError {
    fn from(value: io::Error) -> Self {
        Self::IO(value)
    }
}
