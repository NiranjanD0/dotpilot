//! XDG Base Directory resolution and DotPilot's own application directories.
//!
//! Resolution follows the [XDG Base Directory Specification][spec]. The
//! important detail is that relative paths and empty values are *invalid* and
//! must be ignored in favour of the documented defaults -- implementations
//! commonly get this wrong, and getting it wrong on one machine but not
//! another is exactly the kind of cross-distribution inconsistency this
//! module exists to prevent.
//!
//! [spec]: https://specifications.freedesktop.org/basedir-spec/latest/

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Environment variable names used by the XDG specification.
pub const XDG_CONFIG_HOME: &str = "XDG_CONFIG_HOME";
pub const XDG_DATA_HOME: &str = "XDG_DATA_HOME";
pub const XDG_STATE_HOME: &str = "XDG_STATE_HOME";
pub const XDG_CACHE_HOME: &str = "XDG_CACHE_HOME";
pub const XDG_DATA_DIRS: &str = "XDG_DATA_DIRS";
pub const XDG_CONFIG_DIRS: &str = "XDG_CONFIG_DIRS";
pub const XDG_RUNTIME_DIR: &str = "XDG_RUNTIME_DIR";

/// Resolved locations of the standard XDG base directories.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct XdgDirs {
    /// `$XDG_CONFIG_HOME`, defaulting to `$HOME/.config`.
    pub config_home: PathBuf,
    /// `$XDG_DATA_HOME`, defaulting to `$HOME/.local/share`.
    pub data_home: PathBuf,
    /// `$XDG_STATE_HOME`, defaulting to `$HOME/.local/state`.
    pub state_home: PathBuf,
    /// `$XDG_CACHE_HOME`, defaulting to `$HOME/.cache`.
    pub cache_home: PathBuf,
    /// `$XDG_DATA_DIRS`, defaulting to `/usr/local/share:/usr/share`.
    pub data_dirs: Vec<PathBuf>,
    /// `$XDG_CONFIG_DIRS`, defaulting to `/etc/xdg`.
    pub config_dirs: Vec<PathBuf>,
    /// `$XDG_RUNTIME_DIR`. No default; absent on many systems.
    pub runtime_dir: Option<PathBuf>,
}

impl XdgDirs {
    /// Resolve the XDG directories from the process environment.
    pub fn from_env() -> XdgDirs {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty());
        XdgDirs::resolve(home, &EnvSnapshot::from_env())
    }

    /// Resolve the XDG directories from an explicit home directory and
    /// environment snapshot.
    ///
    /// Splitting this out from [`XdgDirs::from_env`] keeps the logic testable
    /// without mutating the real process environment, which is unsafe to do
    /// from tests running in parallel.
    pub fn resolve(home: Option<PathBuf>, env: &EnvSnapshot) -> XdgDirs {
        let home = || home.clone();
        let config_home = resolve_dir(env.get(XDG_CONFIG_HOME), &home, ".config");
        let data_home = resolve_dir(env.get(XDG_DATA_HOME), &home, ".local/share");
        let state_home = resolve_dir(env.get(XDG_STATE_HOME), &home, ".local/state");
        let cache_home = resolve_dir(env.get(XDG_CACHE_HOME), &home, ".cache");

        let data_dirs = resolve_dir_list(env.get(XDG_DATA_DIRS), &["/usr/local/share", "/usr/share"]);
        let config_dirs = resolve_dir_list(env.get(XDG_CONFIG_DIRS), &["/etc/xdg"]);

        let runtime_dir = env
            .get(XDG_RUNTIME_DIR)
            .filter(|v| Path::new(v).is_absolute())
            .map(PathBuf::from);

        XdgDirs {
            config_home,
            data_home,
            state_home,
            cache_home,
            data_dirs,
            config_dirs,
            runtime_dir,
        }
    }
}

/// DotPilot's own application directories, derived from the XDG base dirs.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AppPaths {
    /// Directory holding user configuration, e.g. `~/.config/dotpilot`.
    pub config_dir: PathBuf,
    /// Directory holding the managed dotfile store and its Git repository.
    pub data_dir: PathBuf,
    /// Directory holding mutable state such as logs and window geometry.
    pub state_dir: PathBuf,
    /// Directory holding regenerable data.
    pub cache_dir: PathBuf,
    /// The home directory the paths were derived from, when known.
    pub home: Option<PathBuf>,
}

impl AppPaths {
    /// Derive the application directories from the current environment.
    pub fn from_env() -> AppPaths {
        let xdg = XdgDirs::from_env();
        AppPaths::from_xdg(&xdg)
    }

    /// Derive the application directories from resolved XDG directories.
    pub fn from_xdg(xdg: &XdgDirs) -> AppPaths {
        AppPaths {
            config_dir: xdg.config_home.join("dotpilot"),
            data_dir: xdg.data_home.join("dotpilot"),
            state_dir: xdg.state_home.join("dotpilot"),
            cache_dir: xdg.cache_home.join("dotpilot"),
            home: home_from_xdg(xdg),
        }
    }

    /// The Git repository backing the managed dotfile store.
    pub fn store_repo_dir(&self) -> PathBuf {
        self.data_dir.join("store")
    }

    /// Path of the manifest describing tracked files.
    pub fn manifest_path(&self) -> PathBuf {
        self.data_dir.join("manifest.json")
    }

    /// Create every application directory that does not yet exist.
    ///
    /// All paths are under the user's own home directory, so this never
    /// requires elevated privileges.
    pub fn ensure(&self) -> std::io::Result<()> {
        for dir in [
            &self.config_dir,
            &self.data_dir,
            &self.state_dir,
            &self.cache_dir,
        ] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }
}

/// Recover the home directory from the resolved XDG paths.
///
/// The home directory is not directly recoverable from the XDG variables
/// alone, so it is inferred by stripping the known default suffixes. This is
/// best-effort: when a user overrides `XDG_CONFIG_HOME` the home directory
/// cannot be inferred and is reported as `None`.
fn home_from_xdg(xdg: &XdgDirs) -> Option<PathBuf> {
    for (base, suffix) in [
        (&xdg.config_home, ".config"),
        (&xdg.data_home, ".local/share"),
        (&xdg.state_home, ".local/state"),
        (&xdg.cache_home, ".cache"),
    ] {
        if let Some(parent) = base.parent() {
            if base.file_name().map(|n| n == suffix).unwrap_or(false) {
                return Some(parent.to_path_buf());
            }
        }
    }
    None
}

/// Resolve a single directory environment variable.
///
/// Per the specification the value must be an absolute path; a relative path
/// is invalid and the default is used instead.
fn resolve_dir(value: Option<&str>, home: &dyn Fn() -> Option<PathBuf>, default: &str) -> PathBuf {
    if let Some(value) = value {
        let path = Path::new(value);
        if path.is_absolute() {
            return path.to_path_buf();
        }
    }
    match home() {
        Some(home) => home.join(default),
        // Without a home directory there is no default to fall back to. Use
        // the root so that callers still receive an absolute path and report
        // a clear permission error rather than operating on a relative path.
        None => PathBuf::from("/").join(default.trim_start_matches('/')),
    }
}

/// Resolve a colon-separated directory list variable.
fn resolve_dir_list(value: Option<&str>, default: &[&str]) -> Vec<PathBuf> {
    let Some(value) = value else {
        return default.iter().map(PathBuf::from).collect();
    };
    let mut dirs: Vec<PathBuf> = value
        .split(':')
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .collect();
    if dirs.is_empty() {
        dirs = default.iter().map(PathBuf::from).collect();
    }
    dirs
}

/// An immutable snapshot of environment variables.
///
/// Reading the environment once and passing it around keeps resolution pure
/// and unit-testable.
#[derive(Debug, Clone, Default)]
pub struct EnvSnapshot {
    vars: HashMap<String, String>,
}

impl EnvSnapshot {
    /// Capture the current process environment.
    pub fn from_env() -> EnvSnapshot {
        EnvSnapshot {
            vars: std::env::vars().collect(),
        }
    }

    /// Build a snapshot from explicit key/value pairs.
    pub fn from_pairs<K, V>(pairs: impl IntoIterator<Item = (K, V)>) -> EnvSnapshot
    where
        K: Into<String>,
        V: Into<String>,
    {
        EnvSnapshot {
            vars: pairs
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        }
    }

    /// Look up a variable.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.vars.get(key).map(|s| s.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dirs(env: &EnvSnapshot) -> XdgDirs {
        XdgDirs::resolve(Some(PathBuf::from("/home/tester")), env)
    }

    #[test]
    fn defaults_follow_the_specification() {
        let xdg = dirs(&EnvSnapshot::default());
        assert_eq!(xdg.config_home, PathBuf::from("/home/tester/.config"));
        assert_eq!(
            xdg.data_home,
            PathBuf::from("/home/tester/.local/share")
        );
        assert_eq!(
            xdg.state_home,
            PathBuf::from("/home/tester/.local/state")
        );
        assert_eq!(xdg.cache_home, PathBuf::from("/home/tester/.cache"));
        assert_eq!(
            xdg.data_dirs,
            vec![PathBuf::from("/usr/local/share"), PathBuf::from("/usr/share")]
        );
        assert_eq!(xdg.config_dirs, vec![PathBuf::from("/etc/xdg")]);
        assert_eq!(xdg.runtime_dir, None);
    }

    #[test]
    fn environment_overrides_defaults() {
        let env = EnvSnapshot::from_pairs([
            (XDG_CONFIG_HOME, "/tmp/cfg"),
            (XDG_DATA_HOME, "/tmp/data"),
            (XDG_STATE_HOME, "/tmp/state"),
            (XDG_CACHE_HOME, "/tmp/cache"),
            (XDG_RUNTIME_DIR, "/run/user/1000"),
        ]);
        let xdg = dirs(&env);
        assert_eq!(xdg.config_home, PathBuf::from("/tmp/cfg"));
        assert_eq!(xdg.data_home, PathBuf::from("/tmp/data"));
        assert_eq!(xdg.state_home, PathBuf::from("/tmp/state"));
        assert_eq!(xdg.cache_home, PathBuf::from("/tmp/cache"));
        assert_eq!(xdg.runtime_dir, Some(PathBuf::from("/run/user/1000")));
    }

    #[test]
    fn relative_paths_are_ignored() {
        // The specification requires absolute paths; a relative value is
        // invalid and the default must be used.
        let env = EnvSnapshot::from_pairs([(XDG_CONFIG_HOME, "relative/path")]);
        let xdg = dirs(&env);
        assert_eq!(xdg.config_home, PathBuf::from("/home/tester/.config"));
    }

    #[test]
    fn empty_values_fall_back_to_defaults() {
        let env = EnvSnapshot::from_pairs([
            (XDG_CONFIG_HOME, ""),
            (XDG_DATA_HOME, ""),
            (XDG_CACHE_HOME, ""),
        ]);
        let xdg = dirs(&env);
        assert_eq!(xdg.config_home, PathBuf::from("/home/tester/.config"));
        assert_eq!(xdg.data_home, PathBuf::from("/home/tester/.local/share"));
        assert_eq!(xdg.cache_home, PathBuf::from("/home/tester/.cache"));
    }

    #[test]
    fn relative_runtime_dir_is_ignored() {
        let env = EnvSnapshot::from_pairs([(XDG_RUNTIME_DIR, "run/user")]);
        assert_eq!(dirs(&env).runtime_dir, None);
    }

    #[test]
    fn data_dirs_are_colon_separated() {
        let env = EnvSnapshot::from_pairs([(XDG_DATA_DIRS, "/opt/share:/srv/share")]);
        assert_eq!(
            dirs(&env).data_dirs,
            vec![PathBuf::from("/opt/share"), PathBuf::from("/srv/share")]
        );
    }

    #[test]
    fn empty_data_dirs_falls_back_to_default() {
        let env = EnvSnapshot::from_pairs([(XDG_DATA_DIRS, "")]);
        assert_eq!(
            dirs(&env).data_dirs,
            vec![PathBuf::from("/usr/local/share"), PathBuf::from("/usr/share")]
        );
    }

    #[test]
    fn app_paths_are_derived_per_xdg_dir() {
        let env = EnvSnapshot::from_pairs([
            (XDG_CONFIG_HOME, "/tmp/cfg"),
            (XDG_DATA_HOME, "/tmp/data"),
            (XDG_STATE_HOME, "/tmp/state"),
            (XDG_CACHE_HOME, "/tmp/cache"),
        ]);
        let app = AppPaths::from_xdg(&dirs(&env));
        assert_eq!(app.config_dir, PathBuf::from("/tmp/cfg/dotpilot"));
        assert_eq!(app.data_dir, PathBuf::from("/tmp/data/dotpilot"));
        assert_eq!(app.state_dir, PathBuf::from("/tmp/state/dotpilot"));
        assert_eq!(app.cache_dir, PathBuf::from("/tmp/cache/dotpilot"));
        assert_eq!(app.store_repo_dir(), PathBuf::from("/tmp/data/dotpilot/store"));
        assert_eq!(app.manifest_path(), PathBuf::from("/tmp/data/dotpilot/manifest.json"));
    }

    #[test]
    fn home_is_inferred_from_default_layout() {
        let app = AppPaths::from_xdg(&dirs(&EnvSnapshot::default()));
        assert_eq!(app.home, Some(PathBuf::from("/home/tester")));
    }

    #[test]
    fn home_is_unknown_when_overridden() {
        let env = EnvSnapshot::from_pairs([(XDG_CONFIG_HOME, "/tmp/cfg")]);
        let app = AppPaths::from_xdg(&dirs(&env));
        // The other three defaults still point at the home directory.
        assert_eq!(app.home, Some(PathBuf::from("/home/tester")));
    }

    #[test]
    fn missing_home_yields_absolute_path() {
        let env = EnvSnapshot::default();
        let xdg = XdgDirs::resolve(None, &env);
        assert!(xdg.config_home.is_absolute());
        assert_eq!(xdg.config_home, PathBuf::from("/.config"));
    }
}
