//! You can find the errors that can occur during server startup here

use std::fmt::{Display, Formatter};
use std::io;

use actix_web::cookie::KeyError;

/// The errors that can occur during server startup
#[derive(Debug)]
pub enum StartServerError {
    /// IO error that can occur
    IO(io::Error),
    /// Invalid secret key was specified
    InvalidSecretKey,
    /// Invalid admin token was found
    InvalidAdminToken,
}

impl Display for StartServerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            StartServerError::IO(err) => write!(f, "{err}"),
            StartServerError::InvalidSecretKey => write!(
                f,
                "Invalid parameter SecretKey. \
                    Consider using the subcommand keygen and update your configuration file"
            ),
            StartServerError::InvalidAdminToken => write!(f, "Invalid admin token was specified"),
        }
    }
}

impl From<io::Error> for StartServerError {
    fn from(value: io::Error) -> Self {
        Self::IO(value)
    }
}

impl From<base64::DecodeError> for StartServerError {
    fn from(_value: base64::DecodeError) -> Self {
        Self::InvalidSecretKey
    }
}

impl From<KeyError> for StartServerError {
    fn from(_value: KeyError) -> Self {
        Self::InvalidSecretKey
    }
}
