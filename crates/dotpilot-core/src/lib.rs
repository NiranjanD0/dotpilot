//! DotPilot core: distribution-independent dotfile management logic.
//!
//! This crate deliberately contains no user-interface and no desktop
//! framework. Everything here is plain Rust so that it can be unit tested on
//! any platform, and exercised inside distribution containers in CI without
//! a display server.
//!
//! The [`platform`] module discovers the environment; the [`store`] module
//! implements the dotfile workflow on top of it.

pub mod error;
pub mod platform;
pub mod store;
pub mod util;

pub use error::{CoreError, Result};
pub use platform::{AppPaths, Capabilities, Distro, Environment, Family, XdgDirs};
pub use store::{Entry, EntryKind, Manifest, Store, TextDiffResult, TrackResult};
