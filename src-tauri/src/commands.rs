//! Tauri commands.
//!
//! These are the only functions the frontend can invoke. They translate
//! between the frontend's JSON-shaped types and the core crate, and turn
//! core errors into messages a user can act on.

use dotpilot_core::platform::{Capabilities, Distro, Environment};
use dotpilot_core::store::{diff_files, Entry, Manifest, TrackResult};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

/// A serialisable command result.
///
/// Errors travel as plain strings so the frontend can render them without
/// needing to know the core error taxonomy.
pub type CommandResult<T> = Result<T, String>;

/// A tracked entry, shaped for the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryView {
    /// Stable identifier.
    pub id: String,
    /// Absolute path in the user's home.
    pub target: String,
    /// Whether the target is currently a symlink into the store.
    pub linked: bool,
    /// Whether the target exists on disk.
    pub present: bool,
    /// `file` or `directory`.
    pub kind: String,
    /// When the entry was added.
    pub added_at: String,
}

/// The result of tracking a path.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackView {
    /// The manifest entry.
    pub entry: EntryView,
    /// Where the displaced original was saved, if it was displaced.
    pub backup: Option<String>,
}

/// A snapshot in the history.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotView {
    /// The commit hash.
    pub hash: String,
    /// The commit subject.
    pub subject: String,
}

/// A concise description of the running environment.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentSummary {
    /// Distribution name, e.g. `Arch Linux`.
    pub distro_name: String,
    /// Distribution identifier, e.g. `arch`.
    pub distro_id: String,
    /// Distribution family identifier, e.g. `arch`.
    pub family: String,
    /// Human readable family label.
    pub family_label: String,
    /// Distribution version, when known.
    pub version: Option<String>,
    /// Whether symlinks can be created.
    pub symlinks_supported: bool,
    /// Whether the home directory is writable.
    pub home_writable: bool,
    /// Restrictions detected on this system.
    pub restrictions: Vec<String>,
    /// Whether the system is restricted or immutable.
    pub restricted: bool,
    /// The path of the `os-release` file the detection used.
    pub os_release_path: Option<String>,
}

/// The full environment, for the diagnostics view.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentView {
    /// The detected distribution.
    pub distro: Distro,
    /// The detected capabilities.
    pub capabilities: Capabilities,
    /// The resolved application paths.
    pub config_dir: String,
    /// The resolved data directory.
    pub data_dir: String,
    /// The resolved state directory.
    pub state_dir: String,
    /// The resolved cache directory.
    pub cache_dir: String,
    /// The store repository path.
    pub store_dir: String,
}

/// Return the full detected environment.
#[tauri::command]
pub fn get_environment(state: State<'_, AppState>) -> CommandResult<EnvironmentView> {
    let environment: &Environment = &state.environment;
    let paths = &environment.app_paths;
    Ok(EnvironmentView {
        distro: environment.distro.clone(),
        capabilities: environment.capabilities.clone(),
        config_dir: paths.config_dir.display().to_string(),
        data_dir: paths.data_dir.display().to_string(),
        state_dir: paths.state_dir.display().to_string(),
        cache_dir: paths.cache_dir.display().to_string(),
        store_dir: paths.store_repo_dir().display().to_string(),
    })
}

/// Return a concise summary of the environment.
#[tauri::command]
pub fn get_environment_summary(state: State<'_, AppState>) -> CommandResult<EnvironmentSummary> {
    let environment = &state.environment;
    let distro = &environment.distro;
    Ok(EnvironmentSummary {
        distro_name: distro.name.clone(),
        distro_id: distro.id.clone(),
        family: distro.family.id().to_string(),
        family_label: distro.family.label().to_string(),
        version: distro.version_id.clone(),
        symlinks_supported: environment.capabilities.symlinks.is_yes(),
        home_writable: environment.capabilities.home_writable.is_yes(),
        restrictions: environment
            .capabilities
            .restrictions
            .iter()
            .map(|r| r.describe())
            .collect(),
        restricted: environment.capabilities.is_restricted(),
        os_release_path: dotpilot_core::platform::distro::os_release_path()
            .map(|p| p.display().to_string()),
    })
}

/// Convert a manifest entry into its frontend shape.
fn to_entry_view(entry: &Entry) -> EntryView {
    let present = entry.target.exists() || entry.target.is_symlink();
    EntryView {
        id: entry.id.clone(),
        target: entry.target.display().to_string(),
        linked: entry.is_linked(),
        present,
        kind: match entry.kind {
            dotpilot_core::EntryKind::File => "file".to_string(),
            dotpilot_core::EntryKind::Directory => "directory".to_string(),
        },
        added_at: entry.added_at.clone(),
    }
}

/// List the tracked entries.
#[tauri::command]
pub fn list_entries(state: State<'_, AppState>) -> CommandResult<Vec<EntryView>> {
    state.with_store(|store| {
        Ok(store
            .manifest()
            .entries
            .iter()
            .map(to_entry_view)
            .collect::<Vec<_>>())
    })
}

/// Track a path, moving it into the store and replacing it with a symlink.
#[tauri::command]
pub fn track_path(state: State<'_, AppState>, path: String) -> CommandResult<TrackView> {
    state.with_store(|store| {
        let result: TrackResult = store.track(std::path::Path::new(&path))?;
        Ok(TrackView {
            entry: to_entry_view(&result.entry),
            backup: result.backup.map(|b| b.display().to_string()),
        })
    })
}

/// Stop tracking a path, leaving a real file in its place.
#[tauri::command]
pub fn untrack_path(state: State<'_, AppState>, id: String, purge: bool) -> CommandResult<Option<String>> {
    state.with_store(|store| {
        let restored = store.untrack(&id, purge)?;
        Ok(restored.map(|p| p.display().to_string()))
    })
}

/// Copy local edits into the store.
#[tauri::command]
pub fn sync_to_store(state: State<'_, AppState>, id: String) -> CommandResult<bool> {
    state.with_store(|store| Ok(store.sync_to_store(&id)?))
}

/// Write the store payload back over the target.
#[tauri::command]
pub fn sync_from_store(state: State<'_, AppState>, id: String) -> CommandResult<Option<String>> {
    state.with_store(|store| {
        let backup = store.sync_from_store(&id)?;
        Ok(backup.map(|b| b.display().to_string()))
    })
}

/// Create a snapshot of the current store contents.
#[tauri::command]
pub fn create_snapshot(state: State<'_, AppState>, message: String) -> CommandResult<String> {
    state.with_store(|store| Ok(store.snapshot(&message)?))
}

/// List the snapshots, newest first.
#[tauri::command]
pub fn list_snapshots(state: State<'_, AppState>) -> CommandResult<Vec<SnapshotView>> {
    state.with_store(|store| {
        Ok(store
            .snapshots()?
            .into_iter()
            .map(|(hash, subject)| SnapshotView { hash, subject })
            .collect())
    })
}

/// Diff two snapshots, or a snapshot against the working tree.
#[tauri::command]
pub fn diff_snapshot(
    state: State<'_, AppState>,
    from: String,
    to: Option<String>,
) -> CommandResult<String> {
    state.with_store(|store| Ok(store.diff(&from, to.as_deref())?))
}

/// Restore the store to a snapshot.
#[tauri::command]
pub fn restore_snapshot(state: State<'_, AppState>, hash: String) -> CommandResult<()> {
    state.with_store(|store| Ok(store.restore(&hash)?))
}

/// Report whether Git is available on this system.
#[tauri::command]
pub fn check_git_available() -> CommandResult<bool> {
    Ok(std::process::Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false))
}

/// Compare a tracked path with the copy held in the store.
///
/// The store payload is the original side of the diff, so added lines are the
/// local edits that have not been captured yet.
#[tauri::command]
pub fn preview_entry_diff(state: State<'_, AppState>, id: String) -> CommandResult<dotpilot_core::TextDiffResult> {
    state.with_store(|store| Ok(store.diff_entry(&id)?))
}

/// Compare two files on disk. Used by the diff viewer.
#[tauri::command]
pub fn diff_two_files(original: String, modified: String) -> CommandResult<dotpilot_core::TextDiffResult> {
    let result = diff_files(
        std::path::Path::new(&original),
        std::path::Path::new(&modified),
    )?;
    Ok(result)
}

/// Re-exported for the frontend's benefit in tests.
pub use dotpilot_core::store::TextDiffResult as DiffResult;

/// Placeholder to keep the `Manifest` import meaningful for future commands.
pub type StoreManifest = Manifest;
