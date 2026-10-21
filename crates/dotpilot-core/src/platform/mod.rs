//! Platform abstraction layer.
//!
//! Everything in this module is about *discovering* the environment, not
//! branching on it. The rest of the core crate is written against these
//! queries so that adding support for a new distribution family does not
//! require touching the dotfile logic.

pub mod capabilities;
pub mod distro;
pub mod paths;

pub use capabilities::{Capabilities, Restriction, Support};
pub use distro::{Distro, Family};
pub use paths::{AppPaths, EnvSnapshot, XdgDirs};

use serde::{Deserialize, Serialize};

/// A complete snapshot of the environment DotPilot is running in.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Environment {
    /// Detected distribution.
    pub distro: Distro,
    /// Detected runtime capabilities.
    pub capabilities: Capabilities,
    /// Resolved XDG base directories.
    pub xdg: XdgDirs,
    /// DotPilot's own application directories.
    pub app_paths: AppPaths,
}

impl Environment {
    /// Detect the current environment.
    pub fn detect() -> Environment {
        let distro = Distro::detect();
        let capabilities = Capabilities::probe();
        let xdg = XdgDirs::from_env();
        let app_paths = AppPaths::from_xdg(&xdg);
        Environment {
            distro,
            capabilities,
            xdg,
            app_paths,
        }
    }
}
