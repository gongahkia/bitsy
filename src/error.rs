// Custom error types for Bitsy

use std::io;

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Utf8(std::string::FromUtf8Error),
    ParseError(String),
    EditorError(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(err) => write!(f, "IO error: {}", err),
            Error::Utf8(err) => write!(f, "UTF-8 error: {}", err),
            Error::ParseError(msg) => write!(f, "Parse error: {}", msg),
            Error::EditorError(msg) => write!(f, "Editor error: {}", msg),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::Io(err)
    }
}

impl From<std::string::FromUtf8Error> for Error {
    fn from(err: std::string::FromUtf8Error) -> Self {
        Error::Utf8(err)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
