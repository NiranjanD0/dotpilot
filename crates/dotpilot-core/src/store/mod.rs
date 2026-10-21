//! The managed dotfile store.
//!
//! The store is a Git repository holding copies of the files DotPilot
//! manages, plus a manifest describing how they map back into the user's
//! home directory.

pub mod diff;
pub mod git;
pub mod manifest;
pub mod ops;

pub use diff::{diff_files, diff_text, DiffLine, LineKind, TextDiffResult};
pub use git::Git;
pub use manifest::{Entry, EntryKind, Manifest};
pub use ops::{Store, TrackResult};
