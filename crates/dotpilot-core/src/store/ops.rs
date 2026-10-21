//! Safe file operations for the managed store.
//!
//! Every operation that can destroy or overwrite user data follows the same
//! rules:
//!
//! * An existing target is never overwritten without an explicit backup.
//! * A target that already points into the store is never clobbered.
//! * Operations confined to the user's own home directory never require
//!   elevated privileges.
//! * Destructive actions report what they did, so the UI can undo.

use std::path::{Path, PathBuf};

use crate::error::{CoreError, Result};
use crate::store::diff::{diff_files, TextDiffResult};
use crate::store::git::Git;
use crate::store::manifest::{entry_id, Entry, EntryKind, Manifest};
use crate::util::time::{now_iso8601, now_unix};

/// Directory inside the store repository that holds the payload files.
pub const PAYLOAD_DIR: &str = "files";

/// The outcome of tracking a path.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrackResult {
    /// The manifest entry that was created.
    pub entry: Entry,
    /// The path the original file was moved to, when one was displaced.
    pub backup: Option<PathBuf>,
}

/// A managed dotfile store backed by a Git repository.
///
/// The store owns a repository whose `files/` directory holds copies of the
/// managed files. The user's home directory contains symlinks into that
/// repository, so a snapshot is a Git commit of the whole managed set.
#[derive(Debug)]
pub struct Store {
    /// Root of the Git repository holding the payload.
    repo_root: PathBuf,
    /// The user's home directory.
    home: PathBuf,
    /// The manifest of tracked entries.
    manifest: Manifest,
    /// Handle to the Git repository.
    git: Git,
}

impl Store {
    /// Open the store, creating the repository if it does not exist yet.
    pub fn open(repo_root: impl Into<PathBuf>, home: impl Into<PathBuf>) -> Result<Store> {
        let repo_root = repo_root.into();
        let home = home.into();
        std::fs::create_dir_all(&repo_root).map_err(|e| CoreError::io(&repo_root, e))?;
        let payload = repo_root.join(PAYLOAD_DIR);
        std::fs::create_dir_all(&payload).map_err(|e| CoreError::io(&payload, e))?;

        let git = Git::init_or_open(&repo_root)?;
        let manifest_path = repo_root.join("manifest.json");
        let manifest = crate::store::manifest::load(&manifest_path)?;

        Ok(Store {
            repo_root,
            home,
            manifest,
            git,
        })
    }

    /// The repository root.
    pub fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    /// The user's home directory.
    pub fn home(&self) -> &Path {
        &self.home
    }

    /// The manifest of tracked entries.
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// The Git handle for the store.
    pub fn git(&self) -> &Git {
        &self.git
    }

    /// Persist the manifest to disk.
    pub fn save_manifest(&mut self) -> Result<()> {
        let path = self.repo_root.join("manifest.json");
        crate::store::manifest::write_atomic(&path, &self.manifest)
    }

    /// Compute the store-relative payload path for a target path.
    ///
    /// Paths inside the home directory keep their relative layout. Paths
    /// outside it are stored under `_external/` with a flattened name, so
    /// that tracking something like `/etc/hosts` is possible without
    /// clobbering unrelated files in the repository.
    fn payload_path(&self, target: &Path) -> PathBuf {
        match target.strip_prefix(&self.home) {
            Ok(relative) if !relative.as_os_str().is_empty() => {
                PathBuf::from(PAYLOAD_DIR).join(relative)
            }
            _ => {
                let flattened = target
                    .to_string_lossy()
                    .trim_start_matches('/')
                    .replace('/', "__");
                PathBuf::from(PAYLOAD_DIR).join("_external").join(flattened)
            }
        }
    }

    /// Track a file or directory, moving the original into the store and
    /// replacing it with a symlink.
    ///
    /// Tracking relocates the file rather than overwriting it, so the
    /// content is preserved in the store payload. A backup copy is still
    /// taken so that a mistaken track can be undone.
    ///
    /// A path that is a symlink pointing outside the store is refused: it
    /// most likely belongs to a separate dotfiles repository, and replacing
    /// it would silently break that setup.
    pub fn track(&mut self, target: &Path) -> Result<TrackResult> {
        let target = absolute(target);
        if !target.exists() {
            return Err(CoreError::NotFound(target));
        }
        if let Some(existing) = self.manifest.find_by_target(&target) {
            let existing = existing.clone();
            return Ok(TrackResult {
                entry: existing,
                backup: None,
            });
        }

        if target.is_symlink() {
            let resolved = std::fs::canonicalize(&target).unwrap_or_else(|_| target.clone());
            if resolved.starts_with(&self.repo_root) {
                return Err(CoreError::Conflict(format!(
                    "{} is already managed by DotPilot",
                    target.display()
                )));
            }
            return Err(CoreError::Conflict(format!(
                "{} is a symlink to {}; DotPilot does not replace links that it does not own",
                target.display(),
                resolved.display()
            )));
        }

        let kind = EntryKind::of(&target)?;
        let store_path = self.payload_path(&target);
        let absolute_store = self.repo_root.join(&store_path);

        if let Some(parent) = absolute_store.parent() {
            std::fs::create_dir_all(parent).map_err(|e| CoreError::io(parent, e))?;
        }

        // Preserve a copy of the original so the operation is reversible.
        let backup = self.backup_original(&target)?;

        // Move the payload into the store.
        if kind == EntryKind::Directory {
            copy_dir(&target, &absolute_store)?;
            std::fs::remove_dir_all(&target).map_err(|e| CoreError::io(&target, e))?;
        } else {
            move_file(&target, &absolute_store)?;
        }

        // Link the target back to the store.
        let link_target = absolute_store.canonicalize().unwrap_or(absolute_store.clone());
        std::os::unix::fs::symlink(&link_target, &target).map_err(|e| CoreError::io(&target, e))?;

        let entry = Entry {
            id: entry_id(&target),
            target,
            store_path,
            kind,
            added_at: now_iso8601(),
        };
        self.manifest.upsert(entry.clone());
        self.save_manifest()?;

        Ok(TrackResult {
            entry,
            backup,
        })
    }

    /// Remove a tracked entry from the manifest.
    ///
    /// The payload in the store is left untouched unless `purge` is set, so
    /// the operation is reversible.
    pub fn untrack(&mut self, id: &str, purge: bool) -> Result<Option<PathBuf>> {
        let Some(entry) = self.manifest.remove(id) else {
            return Ok(None);
        };

        // Replace the symlink with a real copy of the payload so the user
        // keeps a working file, rather than leaving a dangling link.
        let restored = self.materialize(&entry)?;

        if purge {
            let absolute_store = self.repo_root.join(&entry.store_path);
            if entry.kind == EntryKind::Directory {
                let _ = std::fs::remove_dir_all(&absolute_store);
            } else {
                let _ = std::fs::remove_file(&absolute_store);
            }
        }

        self.save_manifest()?;
        Ok(restored)
    }

    /// Replace a symlink with a real copy of its payload.
    fn materialize(&self, entry: &Entry) -> Result<Option<PathBuf>> {
        if !entry.target.is_symlink() {
            return Ok(None);
        }
        let source = self.repo_root.join(&entry.store_path);
        if !source.exists() {
            return Ok(None);
        }
        std::fs::remove_file(&entry.target).map_err(|e| CoreError::io(&entry.target, e))?;
        if entry.kind == EntryKind::Directory {
            copy_dir(&source, &entry.target)?;
        } else {
            std::fs::copy(&source, &entry.target).map_err(|e| CoreError::io(&entry.target, e))?;
        }
        Ok(Some(entry.target.clone()))
    }

    /// Copy the current content of a tracked file back into the store.
    ///
    /// This is the "pull" direction: the user edited the file in place and
    /// wants the change captured in the store.
    pub fn sync_to_store(&mut self, id: &str) -> Result<bool> {
        let Some(entry) = self.manifest.get(id).cloned() else {
            return Ok(false);
        };
        if entry.target.is_symlink() {
            // The link already points at the store payload, so the content is
            // shared and nothing needs copying.
            return Ok(false);
        }
        if !entry.target.exists() {
            return Ok(false);
        }
        let destination = self.repo_root.join(&entry.store_path);
        if entry.kind == EntryKind::Directory {
            let _ = std::fs::remove_dir_all(&destination);
            copy_dir(&entry.target, &destination)?;
        } else {
            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent).map_err(|e| CoreError::io(parent, e))?;
            }
            std::fs::copy(&entry.target, &destination).map_err(|e| CoreError::io(&entry.target, e))?;
        }
        Ok(true)
    }

    /// Write the store payload back over the target path.
    ///
    /// This is the "push" direction: restoring a tracked file to a known
    /// state. The previous content is backed up first.
    pub fn sync_from_store(&mut self, id: &str) -> Result<Option<PathBuf>> {
        let Some(entry) = self.manifest.get(id).cloned() else {
            return Ok(None);
        };
        let source = self.repo_root.join(&entry.store_path);
        if !source.exists() {
            return Err(CoreError::NotFound(source));
        }

        let backup = self.backup_original(&entry.target)?;

        if entry.target.is_symlink() {
            std::fs::remove_file(&entry.target).map_err(|e| CoreError::io(&entry.target, e))?;
        }
        if entry.kind == EntryKind::Directory {
            if entry.target.exists() {
                std::fs::remove_dir_all(&entry.target).map_err(|e| CoreError::io(&entry.target, e))?;
            }
            copy_dir(&source, &entry.target)?;
        } else {
            if let Some(parent) = entry.target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| CoreError::io(parent, e))?;
            }
            std::fs::copy(&source, &entry.target).map_err(|e| CoreError::io(&entry.target, e))?;
        }
        Ok(backup)
    }

    /// Copy the original of a target into the backup directory.
    ///
    /// Returns the backup path when a backup was actually taken.
    fn backup_original(&self, target: &Path) -> Result<Option<PathBuf>> {
        if !target.exists() {
            return Ok(None);
        }
        // Never back up something that is already a link into our own store.
        if target.is_symlink() {
            if let Ok(resolved) = std::fs::canonicalize(target) {
                if resolved.starts_with(&self.repo_root) {
                    return Ok(None);
                }
            }
        }

        let relative = target
            .strip_prefix(&self.home)
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|_| {
                target
                    .to_string_lossy()
                    .trim_start_matches('/')
                    .replace('/', "__")
                    .into()
            });
        let backup_dir = self.repo_root.join("backups").join(now_unix().to_string());
        let backup_path = backup_dir.join(relative);
        if let Some(parent) = backup_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| CoreError::io(parent, e))?;
        }

        let metadata = std::fs::symlink_metadata(target).map_err(|e| CoreError::io(target, e))?;
        if metadata.is_dir() {
            copy_dir(target, &backup_path)?;
        } else {
            std::fs::copy(target, &backup_path).map_err(|e| CoreError::io(target, e))?;
        }
        Ok(Some(backup_path))
    }

    /// Create a snapshot of the current store contents.
    pub fn snapshot(&self, message: &str) -> Result<String> {
        self.git.commit_all(message)
    }

    /// List the snapshots, newest first.
    pub fn snapshots(&self) -> Result<Vec<(String, String)>> {
        self.git.log(200)
    }

    /// Diff two snapshots, or a snapshot against the working tree.
    pub fn diff(&self, from: &str, to: Option<&str>) -> Result<String> {
        self.git.diff(from, to)
    }

    /// Compare a tracked path with the copy held in the store.
    ///
    /// The store payload is the original side of the diff, so the added lines
    /// are exactly the local edits that [`Store::sync_to_store`] would capture.
    pub fn diff_entry(&self, id: &str) -> Result<TextDiffResult> {
        let Some(entry) = self.manifest.get(id) else {
            return Err(CoreError::NotFound(PathBuf::from(id)));
        };
        if entry.kind == EntryKind::Directory {
            return Err(CoreError::Unsupported(
                "directories cannot be compared as text".to_string(),
            ));
        }
        let payload = self.repo_root.join(&entry.store_path);
        diff_files(&payload, &entry.target)
    }

    /// Restore the store to a snapshot and relink the tracked entries.
    pub fn restore(&mut self, revision: &str) -> Result<()> {
        self.git.checkout(revision)?;
        self.relink()?;
        Ok(())
    }

    /// Recreate the symlinks for every tracked entry.
    ///
    /// Used after a restore, where the payload paths are correct but the
    /// links in the home directory may have been removed.
    pub fn relink(&mut self) -> Result<usize> {
        let mut relinked = 0;
        for entry in self.manifest.entries.clone() {
            let source = self.repo_root.join(&entry.store_path);
            if !source.exists() {
                continue;
            }
            if entry.target.is_symlink() {
                continue;
            }
            if entry.target.exists() {
                // A real file sits where the link should go. Leave it alone
                // rather than destroying the user's data.
                continue;
            }
            if let Some(parent) = entry.target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| CoreError::io(parent, e))?;
            }
            let link_target = source.canonicalize().unwrap_or(source.clone());
            std::os::unix::fs::symlink(&link_target, &entry.target)
                .map_err(|e| CoreError::io(&entry.target, e))?;
            relinked += 1;
        }
        Ok(relinked)
    }
}

/// Move a file into the store.
///
/// A rename only works within a single filesystem. The store lives in the
/// user's data directory, so a file that sits on another mount, such as
/// `/etc/hosts` or anything under a tmpfs, has to be copied and then
/// removed.
fn move_file(source: &Path, destination: &Path) -> Result<()> {
    match std::fs::rename(source, destination) {
        Ok(()) => Ok(()),
        // EXDEV: the source and the destination are on different filesystems.
        Err(error) if error.raw_os_error() == Some(18) => {
            std::fs::copy(source, destination).map_err(|e| CoreError::io(destination, e))?;
            std::fs::remove_file(source).map_err(|e| CoreError::io(source, e))?;
            Ok(())
        }
        Err(error) => Err(CoreError::io(source, error)),
    }
}

/// Make a path absolute against the current directory.
fn absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    }
}

/// Recursively copy a directory tree.
pub fn copy_dir(source: &Path, destination: &Path) -> Result<()> {
    std::fs::create_dir_all(destination).map_err(|e| CoreError::io(destination, e))?;
    for entry in std::fs::read_dir(source).map_err(|e| CoreError::io(source, e))? {
        let entry = entry.map_err(|e| CoreError::io(source, e))?;
        let from = entry.path();
        let to = destination.join(entry.file_name());
        let metadata = std::fs::symlink_metadata(&from).map_err(|e| CoreError::io(&from, e))?;
        if metadata.is_dir() {
            copy_dir(&from, &to)?;
        } else if metadata.file_type().is_symlink() {
            let link = std::fs::read_link(&from).map_err(|e| CoreError::io(&from, e))?;
            let _ = std::fs::remove_file(&to);
            std::os::unix::fs::symlink(link, &to).map_err(|e| CoreError::io(&to, e))?;
        } else {
            std::fs::copy(&from, &to).map_err(|e| CoreError::io(&from, e))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (tempfile::TempDir, Store) {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let repo = tmp.path().join("repo");
        std::fs::create_dir_all(&home).unwrap();
        let store = Store::open(repo, home).unwrap();
        (tmp, store)
    }

    #[test]
    fn tracking_a_file_moves_it_into_the_store_and_links_it() {
        let (tmp, mut store) = setup();
        let target = store.home().join(".bashrc");
        std::fs::write(&target, "export PS1='$ '\n").unwrap();

        let result = store.track(&target).unwrap();

        assert!(target.is_symlink());
        let linked = std::fs::read_link(&target).unwrap();
        assert!(linked.starts_with(store.repo_root()));
        assert_eq!(std::fs::read_to_string(&linked).unwrap(), "export PS1='$ '\n");
        assert_eq!(store.manifest().entries.len(), 1);
        // A backup copy is kept so the operation can be undone.
        assert!(result.backup.is_some());
        let _ = tmp;
    }

    #[test]
    fn tracking_a_directory_works() {
        let (_tmp, mut store) = setup();
        let dir = store.home().join(".config").join("nvim");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("init.lua"), "vim.o.number = true\n").unwrap();

        store.track(&dir).unwrap();

        assert!(dir.is_symlink());
        let linked = std::fs::read_link(&dir).unwrap();
        assert!(linked.join("init.lua").exists());
    }

    #[test]
    fn tracking_preserves_the_original_as_a_backup() {
        let (_tmp, mut store) = setup();
        let target = store.home().join("existing.conf");
        std::fs::write(&target, "original\n").unwrap();

        let result = store.track(&target).unwrap();

        let backup = result.backup.expect("a backup should have been taken");
        assert!(backup.exists());
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), "original\n");
    }

    #[test]
    fn tracking_twice_is_a_no_op() {
        let (_tmp, mut store) = setup();
        let target = store.home().join(".bashrc");
        std::fs::write(&target, "x\n").unwrap();

        let first = store.track(&target).unwrap();
        let second = store.track(&target).unwrap();

        assert_eq!(first.entry.id, second.entry.id);
        assert_eq!(store.manifest().entries.len(), 1);
    }

    #[test]
    fn tracking_a_missing_path_fails() {
        let (_tmp, mut store) = setup();
        let missing = store.home().join("nope");
        assert!(store.track(&missing).is_err());
    }

    #[test]
    fn tracking_a_managed_path_is_rejected() {
        let (_tmp, mut store) = setup();
        let target = store.home().join(".bashrc");
        std::fs::write(&target, "x\n").unwrap();
        store.track(&target).unwrap();

        // Linking a managed path as a new target must not create a
        // self-referential link.
        let payload = store.repo_root().join(store.manifest().entries[0].store_path.clone());
        let alias = store.home().join("alias");
        std::os::unix::fs::symlink(&payload, &alias).unwrap();
        assert!(store.track(&alias).is_err());
    }

    #[test]
    fn tracking_a_foreign_symlink_is_refused() {
        let (_tmp, mut store) = setup();
        // A symlink into a separate dotfiles repository must not be
        // hijacked, because replacing it would break the other setup.
        let elsewhere = store.home().join("other-repo").join("config");
        std::fs::create_dir_all(elsewhere.parent().unwrap()).unwrap();
        std::fs::write(&elsewhere, "content\n").unwrap();
        let alias = store.home().join("alias");
        std::os::unix::fs::symlink(&elsewhere, &alias).unwrap();

        let error = store.track(&alias).unwrap_err();
        assert!(error.to_string().contains("symlink"));
        // The original link is untouched.
        assert!(alias.is_symlink());
        assert_eq!(std::fs::read_link(&alias).unwrap(), elsewhere);
    }

    #[test]
    fn untracking_leaves_a_real_file_behind() {
        let (_tmp, mut store) = setup();
        let target = store.home().join(".bashrc");
        std::fs::write(&target, "content\n").unwrap();
        let result = store.track(&target).unwrap();

        store.untrack(&result.entry.id, false).unwrap();

        assert!(!target.is_symlink());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "content\n");
        assert!(store.manifest().entries.is_empty());
    }

    #[test]
    fn untracking_with_purge_removes_the_payload() {
        let (_tmp, mut store) = setup();
        let target = store.home().join(".bashrc");
        std::fs::write(&target, "content\n").unwrap();
        let result = store.track(&target).unwrap();
        let payload = store.repo_root().join(&result.entry.store_path);

        store.untrack(&result.entry.id, true).unwrap();

        assert!(!payload.exists());
    }

    #[test]
    fn snapshot_and_restore_round_trip() {
        let (_tmp, mut store) = setup();
        let target = store.home().join(".bashrc");
        std::fs::write(&target, "first\n").unwrap();
        store.track(&target).unwrap();

        let hash = store.snapshot("initial").unwrap();
        assert_eq!(store.snapshots().unwrap().len(), 1);

        // Change the payload and snapshot again.
        let payload = store.repo_root().join(&store.manifest().entries[0].store_path);
        std::fs::write(&payload, "second\n").unwrap();
        store.snapshot("changed").unwrap();
        assert_eq!(store.snapshots().unwrap().len(), 2);

        // Restore the first snapshot.
        store.restore(&hash).unwrap();
        assert_eq!(std::fs::read_to_string(&payload).unwrap(), "first\n");
    }

    #[test]
    fn diff_between_snapshots_reports_the_change() {
        let (_tmp, mut store) = setup();
        let target = store.home().join(".bashrc");
        std::fs::write(&target, "one\n").unwrap();
        store.track(&target).unwrap();
        let first = store.snapshot("initial").unwrap();

        let payload = store.repo_root().join(&store.manifest().entries[0].store_path);
        std::fs::write(&payload, "two\n").unwrap();
        let second = store.snapshot("changed").unwrap();

        let diff = store.diff(&first, Some(&second)).unwrap();
        assert!(diff.contains("-one"));
        assert!(diff.contains("+two"));
    }

    #[test]
    fn sync_from_store_restores_the_target_content() {
        let (_tmp, mut store) = setup();
        let target = store.home().join(".bashrc");
        std::fs::write(&target, "good\n").unwrap();
        let result = store.track(&target).unwrap();

        // Break the payload and then push it back to the target.
        let payload = store.repo_root().join(&result.entry.store_path);
        std::fs::write(&payload, "bad\n").unwrap();
        store.sync_from_store(&result.entry.id).unwrap();

        assert_eq!(std::fs::read_to_string(&target).unwrap(), "bad\n");
    }

    #[test]
    fn sync_to_store_captures_local_edits() {
        let (_tmp, mut store) = setup();
        let target = store.home().join(".bashrc");
        std::fs::write(&target, "good\n").unwrap();
        let result = store.track(&target).unwrap();

        // Break the link and write a real file, then pull it into the store.
        std::fs::remove_file(&target).unwrap();
        std::fs::write(&target, "edited\n").unwrap();
        assert!(store.sync_to_store(&result.entry.id).unwrap());

        let payload = store.repo_root().join(&result.entry.store_path);
        assert_eq!(std::fs::read_to_string(&payload).unwrap(), "edited\n");
    }

    #[test]
    fn relink_restores_missing_links() {
        let (_tmp, mut store) = setup();
        let target = store.home().join(".bashrc");
        std::fs::write(&target, "content\n").unwrap();
        store.track(&target).unwrap();

        std::fs::remove_file(&target).unwrap();
        assert!(!target.exists());

        assert_eq!(store.relink().unwrap(), 1);
        assert!(target.is_symlink());
    }

    #[test]
    fn relink_leaves_real_files_alone() {
        let (_tmp, mut store) = setup();
        let target = store.home().join(".bashrc");
        std::fs::write(&target, "content\n").unwrap();
        store.track(&target).unwrap();

        std::fs::remove_file(&target).unwrap();
        std::fs::write(&target, "user data\n").unwrap();

        assert_eq!(store.relink().unwrap(), 0);
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "user data\n");
    }

    #[test]
    fn external_paths_are_stored_under_a_separate_directory() {
        let (_tmp, mut store) = setup();
        let external = PathBuf::from("/etc/hosts");
        let payload = store.payload_path(&external);
        assert!(payload.starts_with(PAYLOAD_DIR));
        assert!(payload.to_string_lossy().contains("_external"));
    }

    #[test]
    fn payloads_of_home_paths_keep_their_layout() {
        let (_tmp, store) = setup();
        let payload = store.payload_path(&store.home().join(".config/nvim/init.lua"));
        assert_eq!(
            payload,
            PathBuf::from(PAYLOAD_DIR).join(".config/nvim/init.lua")
        );
    }

    #[test]
    fn diffing_an_entry_reports_the_local_edits() {
        let (_tmp, mut store) = setup();
        let target = store.home().join(".bashrc");
        std::fs::write(&target, "one\n").unwrap();
        store.track(&target).unwrap();

        // Replace the link with a real file to simulate a local edit that has
        // not been captured in the store yet.
        std::fs::remove_file(&target).unwrap();
        std::fs::write(&target, "one\ntwo\n").unwrap();

        let id = store.manifest().entries[0].id.clone();
        let diff = store.diff_entry(&id).unwrap();

        assert_eq!(diff.added, 1);
        assert_eq!(diff.removed, 0);
    }

    #[test]
    fn diffing_a_directory_entry_is_refused() {
        let (_tmp, mut store) = setup();
        let dir = store.home().join(".config");
        std::fs::create_dir_all(&dir).unwrap();
        store.track(&dir).unwrap();

        let id = store.manifest().entries[0].id.clone();
        assert!(store.diff_entry(&id).is_err());
    }

    #[test]
    fn moving_a_file_from_another_filesystem_copies_it() {
        // /dev/shm is a separate mount, so the file cannot simply be renamed
        // into the store.
        let source_dir = PathBuf::from("/dev/shm");
        if !source_dir.is_dir() {
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let source = source_dir.join(format!("dotpilot-move-{}", std::process::id()));
        std::fs::write(&source, "content\n").unwrap();
        let destination = tmp.path().join("moved");

        move_file(&source, &destination).unwrap();

        assert_eq!(std::fs::read_to_string(&destination).unwrap(), "content\n");
        assert!(!source.exists());
    }
}
