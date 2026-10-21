//! The manifest of tracked files.
//!
//! The manifest is the authoritative list of what DotPilot manages: which
//! paths in the user's home are linked to the store, and where the payload
//! for each one lives inside the store repository.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};

/// The manifest format version, bumped when the on-disk shape changes.
pub const MANIFEST_VERSION: u32 = 1;

/// How a tracked path is represented on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    /// A single file.
    File,
    /// A directory tree.
    Directory,
}

impl EntryKind {
    /// Determine the kind of an existing path.
    pub fn of(path: &Path) -> Result<EntryKind> {
        let metadata = std::fs::symlink_metadata(path)
            .map_err(|e| CoreError::io(path, e))?;
        if metadata.is_dir() {
            Ok(EntryKind::Directory)
        } else {
            Ok(EntryKind::File)
        }
    }
}

/// A single tracked path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// Stable identifier derived from the target path.
    pub id: String,
    /// Absolute path of the managed file in the user's home.
    pub target: PathBuf,
    /// Path of the payload relative to the store repository root.
    pub store_path: PathBuf,
    /// Whether the payload is a file or a directory.
    pub kind: EntryKind,
    /// ISO-8601 timestamp of when the entry was added.
    pub added_at: String,
}

impl Entry {
    /// Whether the target currently points at the store payload.
    pub fn is_linked(&self) -> bool {
        match std::fs::read_link(&self.target) {
            Ok(link) => link == self.store_path || link.ends_with(&self.store_path),
            Err(_) => false,
        }
    }
}

/// The full manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// Format version.
    pub version: u32,
    /// Tracked entries.
    pub entries: Vec<Entry>,
}

impl Default for Manifest {
    fn default() -> Self {
        Manifest {
            version: MANIFEST_VERSION,
            entries: Vec::new(),
        }
    }
}

impl Manifest {
    /// Find an entry by identifier.
    pub fn get(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Find an entry by its target path.
    pub fn find_by_target(&self, target: &Path) -> Option<&Entry> {
        self.entries.iter().find(|e| e.target == target)
    }

    /// Insert or replace an entry, keyed by identifier.
    pub fn upsert(&mut self, entry: Entry) {
        match self.entries.iter_mut().find(|e| e.id == entry.id) {
            Some(existing) => *existing = entry,
            None => self.entries.push(entry),
        }
    }

    /// Remove an entry by identifier, returning it when found.
    pub fn remove(&mut self, id: &str) -> Option<Entry> {
        let index = self.entries.iter().position(|e| e.id == id)?;
        Some(self.entries.remove(index))
    }
}

/// Derive a stable identifier from an absolute target path.
///
/// A plain hash is used rather than a random UUID so that re-adding the same
/// path produces the same identifier, which keeps the manifest stable across
/// machines and re-syncs.
pub fn entry_id(target: &Path) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in target.to_string_lossy().as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    format!("{:016x}", hash)
}

/// Write a value to a file atomically.
///
/// The content is written to a sibling temporary file and then renamed, so a
/// crash mid-write cannot leave a truncated manifest behind.
pub fn write_atomic<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| CoreError::io(parent, e))?;
    }
    let body = serde_json::to_vec_pretty(value)?;
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, body).map_err(|e| CoreError::io(&temp, e))?;
    std::fs::rename(&temp, path).map_err(|e| CoreError::io(path, e))?;
    Ok(())
}

/// Read a manifest from disk, returning the default when it does not exist.
pub fn load(path: &Path) -> Result<Manifest> {
    if !path.exists() {
        return Ok(Manifest::default());
    }
    let body = std::fs::read_to_string(path).map_err(|e| CoreError::io(path, e))?;
    if body.trim().is_empty() {
        return Ok(Manifest::default());
    }
    let manifest: Manifest = serde_json::from_str(&body)?;
    if manifest.version > MANIFEST_VERSION {
        return Err(CoreError::Invalid(format!(
            "manifest version {} is newer than the supported version {}",
            manifest.version, MANIFEST_VERSION
        )));
    }
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_ids_are_stable_and_distinct() {
        let a = entry_id(Path::new("/home/user/.bashrc"));
        let b = entry_id(Path::new("/home/user/.bashrc"));
        let c = entry_id(Path::new("/home/user/.zshrc"));
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn manifest_round_trips_through_json() {
        let manifest = Manifest {
            version: MANIFEST_VERSION,
            entries: vec![Entry {
                id: "abc".into(),
                target: PathBuf::from("/home/user/.bashrc"),
                store_path: PathBuf::from("files/.bashrc"),
                kind: EntryKind::File,
                added_at: "2026-10-22T00:00:00Z".into(),
            }],
        };
        let json = serde_json::to_string(&manifest).unwrap();
        let parsed: Manifest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, manifest);
    }

    #[test]
    fn upsert_replaces_matching_entries() {
        let mut manifest = Manifest::default();
        let entry = Entry {
            id: "abc".into(),
            target: PathBuf::from("/home/user/.bashrc"),
            store_path: PathBuf::from("files/.bashrc"),
            kind: EntryKind::File,
            added_at: "2026-10-22T00:00:00Z".into(),
        };
        manifest.upsert(entry.clone());
        manifest.upsert(entry);
        assert_eq!(manifest.entries.len(), 1);
    }

    #[test]
    fn remove_returns_the_removed_entry() {
        let mut manifest = Manifest::default();
        manifest.upsert(Entry {
            id: "abc".into(),
            target: PathBuf::from("/home/user/.bashrc"),
            store_path: PathBuf::from("files/.bashrc"),
            kind: EntryKind::File,
            added_at: "2026-10-22T00:00:00Z".into(),
        });
        assert!(manifest.remove("abc").is_some());
        assert!(manifest.remove("abc").is_none());
        assert!(manifest.entries.is_empty());
    }

    #[test]
    fn load_returns_default_when_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let manifest = load(&tmp.path().join("manifest.json")).unwrap();
        assert_eq!(manifest, Manifest::default());
    }

    #[test]
    fn load_rejects_a_newer_version() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("manifest.json");
        std::fs::write(&path, r#"{"version": 99, "entries": []}"#).unwrap();
        assert!(load(&path).is_err());
    }

    #[test]
    fn atomic_write_leaves_no_temp_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("nested").join("manifest.json");
        write_atomic(&path, &Manifest::default()).unwrap();
        assert!(path.exists());
        let leftovers: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty());
    }
}
