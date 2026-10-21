//! Error types for DotPilot core operations.

use std::path::{Path, PathBuf};

/// Result alias used throughout the core crate.
pub type Result<T> = std::result::Result<T, CoreError>;

/// Errors produced by core operations.
///
/// The `Unsupported` variant exists so that platform-specific limitations are
/// reported to the user instead of failing silently.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("i/o error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{0}")]
    IoPlain(#[from] std::io::Error),

    #[error("not found: {0}")]
    NotFound(PathBuf),

    #[error("operation not supported: {0}")]
    Unsupported(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("invalid data: {0}")]
    Invalid(String),

    #[error("git error: {0}")]
    Git(#[from] git2::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

impl CoreError {
    /// Attach a path to an `io::Error`, preserving the offending location.
    pub fn io(path: impl AsRef<Path>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.as_ref().to_path_buf(),
            source,
        }
    }
}

impl From<CoreError> for String {
    /// Render the error as a user-facing message.
    ///
    /// The desktop shell transports errors as plain strings so the frontend
    /// never needs to know the core error taxonomy.
    fn from(error: CoreError) -> String {
        error.to_string()
    }
}

/// Helper for adding path context to `std::io` results.
pub trait IoContext<T> {
    fn at(self, path: impl AsRef<Path>) -> Result<T>;
}

impl<T> IoContext<T> for std::result::Result<T, std::io::Error> {
    fn at(self, path: impl AsRef<Path>) -> Result<T> {
        self.map_err(|e| CoreError::io(path, e))
    }
}
