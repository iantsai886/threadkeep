//! Threadkeep core. One implementation of every rule, shared by the command line, the MCP
//! server AI apps launch, and the desktop app. Contracts: docs/CONTRACTS.md.

pub mod access;
pub mod backup;
pub mod connectors;
pub mod home;
pub mod markdown;
pub mod mcp;
pub mod memory;
pub mod ops;
pub mod pending;
pub mod server;
pub mod search;
pub mod spaces;
pub mod threads;

use std::fmt;

/// Errors are plain sentences meant to be shown to the user as they are.
#[derive(Debug, Clone, PartialEq)]
pub struct Error(pub String);

pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error(e.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error(e.to_string())
    }
}

pub fn err<T>(message: impl Into<String>) -> Result<T> {
    Err(Error(message.into()))
}

#[macro_export]
macro_rules! bail {
    ($($arg:tt)*) => { return Err($crate::Error(format!($($arg)*))) };
}
