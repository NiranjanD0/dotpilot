import { invoke } from "@tauri-apps/api/core";

import type {
  EnvironmentSummary,
  EnvironmentView,
  EntryView,
  SnapshotView,
  TextDiffResult,
  TrackView,
} from "./types";

/** Call a backend command and surface failures as plain `Error`s. */
async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw new Error(typeof error === "string" ? error : String(error));
  }
}

export const api = {
  environment: () => call<EnvironmentView>("get_environment"),
  environmentSummary: () => call<EnvironmentSummary>("get_environment_summary"),

  listEntries: () => call<EntryView[]>("list_entries"),
  trackPath: (path: string) => call<TrackView>("track_path", { path }),
  untrackPath: (id: string, purge: boolean) =>
    call<string | null>("untrack_path", { id, purge }),
  syncToStore: (id: string) => call<boolean>("sync_to_store", { id }),
  syncFromStore: (id: string) => call<string | null>("sync_from_store", { id }),

  createSnapshot: (message: string) => call<string>("create_snapshot", { message }),
  listSnapshots: () => call<SnapshotView[]>("list_snapshots"),
  diffSnapshot: (from: string, to?: string) =>
    call<string>("diff_snapshot", { from, to: to ?? null }),
  restoreSnapshot: (hash: string) => call<void>("restore_snapshot", { hash }),

  checkGitAvailable: () => call<boolean>("check_git_available"),
  diffEntry: (id: string) => call<TextDiffResult>("preview_entry_diff", { id }),
};
