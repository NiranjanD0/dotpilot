//! DotPilot Tauri application.
//!
//! The desktop shell is deliberately thin. All dotfile logic lives in the
//! `dotpilot-core` crate, which has no desktop dependencies and is tested
//! independently of the UI.

pub mod commands;
pub mod state;

use state::AppState;

/// Build and run the desktop application.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::get_environment,
            commands::get_environment_summary,
            commands::list_entries,
            commands::track_path,
            commands::untrack_path,
            commands::sync_to_store,
            commands::sync_from_store,
            commands::create_snapshot,
            commands::list_snapshots,
            commands::diff_snapshot,
            commands::restore_snapshot,
            commands::check_git_available,
            commands::preview_entry_diff,
        ])
        .run(tauri::generate_context!())
        .expect("error while running DotPilot");
}
