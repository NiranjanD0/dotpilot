/** Types mirroring the JSON shapes the Tauri commands return. */

export type EntryKind = "file" | "directory";

export interface EntryView {
  id: string;
  target: string;
  linked: boolean;
  present: boolean;
  kind: EntryKind;
  addedAt: string;
}

export interface TrackView {
  entry: EntryView;
  backup: string | null;
}

export interface SnapshotView {
  hash: string;
  subject: string;
}

export interface EnvironmentSummary {
  distroName: string;
  distroId: string;
  family: string;
  familyLabel: string;
  version: string | null;
  symlinksSupported: boolean;
  homeWritable: boolean;
  restrictions: string[];
  restricted: boolean;
  osReleasePath: string | null;
}

export interface Distro {
  id: string;
  idLike: string[];
  name: string;
  versionId: string | null;
  versionCodename: string | null;
  family: string;
  prettyName: string | null;
}

export interface Capabilities {
  symlinks: "Yes" | "No" | "Unknown";
  homeWritable: "Yes" | "No" | "Unknown";
  restrictions: string[];
}

export interface EnvironmentView {
  distro: Distro;
  capabilities: Capabilities;
  configDir: string;
  dataDir: string;
  stateDir: string;
  cacheDir: string;
  storeDir: string;
}

export type LineKind = "context" | "removed" | "added";

export interface DiffLine {
  kind: LineKind;
  text: string;
  originalLine: number | null;
  modifiedLine: number | null;
}

export interface TextDiffResult {
  lines: DiffLine[];
  added: number;
  removed: number;
}
