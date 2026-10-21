//! Shared application state.

use std::path::PathBuf;
use std::sync::Mutex;

use dotpilot_core::platform::{AppPaths, Environment};
use dotpilot_core::store::Store;

/// The mutable state shared by every command.
///
/// The store is opened lazily on first use so that launching the application
/// never fails merely because the data directory cannot be created yet.
pub struct AppState {
    /// The detected environment.
    pub environment: Environment,
    /// The managed store, opened on demand.
    store: Mutex<Option<Store>>,
}

impl AppState {
    /// Build the initial state.
    pub fn new() -> AppState {
        AppState {
            environment: Environment::detect(),
            store: Mutex::new(None),
        }
    }

    /// The user's home directory, when it could be determined.
    pub fn home(&self) -> Option<PathBuf> {
        dotpilot_core::platform::capabilities::home_dir()
    }

    /// The application paths in use.
    pub fn paths(&self) -> &AppPaths {
        &self.environment.app_paths
    }

    /// Run an operation against the store.
    ///
    /// The store is opened on first use and then reused. The mutex serialises
    /// access so that two commands never write the manifest concurrently.
    pub fn with_store<T>(
        &self,
        operation: impl FnOnce(&mut Store) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut guard = self.store.lock().map_err(|e| e.to_string())?;
        if guard.is_none() {
            let paths = self.paths().clone();
            let home = self
                .home()
                .ok_or_else(|| "could not determine the home directory; is HOME set?".to_string())?;
            paths.ensure().map_err(|e| e.to_string())?;
            let store = Store::open(paths.store_repo_dir(), home).map_err(|e| e.to_string())?;
            *guard = Some(store);
        }
        let store = guard.as_mut().expect("store was just initialised");
        operation(store)
    }
}
