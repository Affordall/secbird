use crate::proto::{Command, Status};

#[derive(Debug)]
pub enum Error {
    Hid(hidapi::HidError),
    /// The device answered with a non-success status.
    Device { cmd: Command, status: Status },
    /// The device stayed busy or echoed a different command.
    NoReply(Command),
    NotFound(String),
    Unsupported(String),
    InvalidArg(String),
    Db(String),
    /// A Windows API call or system command failed.
    System(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Hid(e) => write!(f, "HID error: {e}"),
            Error::Device { cmd, status } => write!(f, "device rejected {cmd}: {status:?}"),
            Error::NoReply(cmd) => write!(f, "no valid reply to {cmd}"),
            Error::NotFound(s) => write!(f, "not found: {s}"),
            Error::Unsupported(s) => write!(f, "unsupported: {s}"),
            Error::InvalidArg(s) => write!(f, "invalid argument: {s}"),
            Error::Db(s) => write!(f, "device database: {s}"),
            Error::System(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<hidapi::HidError> for Error {
    fn from(e: hidapi::HidError) -> Self {
        Error::Hid(e)
    }
}
