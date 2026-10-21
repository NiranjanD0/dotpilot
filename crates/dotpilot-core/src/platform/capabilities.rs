//! Runtime capability probing.
//!
//! Rather than inferring what a system allows from its distribution name,
//! these probes perform the actual operation in a scratch directory and
//! report the observed result. That keeps the behaviour correct on
//! exotic filesystems, containers and sandboxes where a name-based guess
//! would be wrong.

use std::path::{Path, PathBuf};

/// The result of probing a specific filesystem capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Support {
    /// The operation was attempted and succeeded.
    Yes,
    /// The operation was attempted and failed.
    No,
    /// The operation could not be attempted on this system.
    Unknown,
}

impl Support {
    /// Whether the capability is present.
    pub fn is_yes(self) -> bool {
        matches!(self, Support::Yes)
    }
}

/// Why a system may be read-only or otherwise restricted.
///
/// These are detected from concrete on-disk markers rather than from the
/// distribution name, because the same distribution ships in both mutable and
/// immutable variants.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Restriction {
    /// An ostree-based system (Silverblue, Bazzite, MicroOS).
    Ostree,
    /// A Nix-based system with an immutable `/nix/store`.
    Nix,
    /// A read-only root filesystem.
    ReadOnlyRoot,
    /// Running inside a container.
    Container,
    /// Running inside a Flatpak sandbox.
    Flatpak,
    /// A directory DotPilot needs is not writable.
    NotWritable(PathBuf),
}

impl Restriction {
    /// A short human readable explanation.
    pub fn describe(&self) -> String {
        match self {
            Restriction::Ostree => {
                "ostree-based system: /usr is managed by image updates".to_string()
            }
            Restriction::Nix => "nix-based system: /nix/store is immutable".to_string(),
            Restriction::ReadOnlyRoot => "root filesystem is read-only".to_string(),
            Restriction::Container => "running inside a container".to_string(),
            Restriction::Flatpak => "running inside a Flatpak sandbox".to_string(),
            Restriction::NotWritable(path) => {
                format!("{} is not writable", path.display())
            }
        }
    }
}

/// Observed capabilities of the current system.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Capabilities {
    /// Whether symbolic links can be created.
    pub symlinks: Support,
    /// Whether the user's home directory is writable.
    pub home_writable: Support,
    /// Restrictions detected on this system.
    pub restrictions: Vec<Restriction>,
}

impl Default for Capabilities {
    fn default() -> Self {
        Capabilities {
            symlinks: Support::Unknown,
            home_writable: Support::Unknown,
            restrictions: Vec::new(),
        }
    }
}

impl Capabilities {
    /// Probe the current system.
    pub fn probe() -> Capabilities {
        Capabilities::probe_with(std::env::temp_dir(), home_dir())
    }

    /// Probe using an explicit scratch directory and home directory.
    ///
    /// Splitting this out keeps the logic testable without touching the real
    /// home directory.
    pub fn probe_with(scratch: PathBuf, home: Option<PathBuf>) -> Capabilities {
        let mut capabilities = Capabilities::default();

        capabilities.symlinks = probe_symlinks(&scratch);
        capabilities.home_writable = match &home {
            Some(home) => probe_writable(home),
            None => Support::Unknown,
        };

        capabilities.restrictions = detect_restrictions(&home);
        capabilities
    }

    /// Whether the system is a restricted or immutable variant.
    pub fn is_restricted(&self) -> bool {
        !self.restrictions.is_empty()
    }
}

/// Determine whether symlinks can be created in `dir`.
///
/// The probe creates a real symlink and removes it again. A directory that
/// cannot be written to yields `Unknown` rather than `No`, because the
/// symlink support itself was never really tested.
fn probe_symlinks(dir: &Path) -> Support {
    let probe_dir = dir.join(".dotpilot-symlink-probe");
    if std::fs::create_dir_all(&probe_dir).is_err() {
        return Support::Unknown;
    }

    let target = probe_dir.join("target");
    let link = probe_dir.join("link");
    let result = std::os::unix::fs::symlink(&target, &link);
    let _ = std::fs::remove_file(&link);
    let _ = std::fs::remove_dir(&probe_dir);

    match result {
        Ok(()) => Support::Yes,
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => Support::Unknown,
        Err(_) => Support::No,
    }
}

/// Determine whether `dir` accepts new files.
fn probe_writable(dir: &Path) -> Support {
    if !dir.exists() {
        return Support::Unknown;
    }
    let probe = dir.join(".dotpilot-write-probe");
    match std::fs::File::create(&probe) {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            Support::Yes
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => Support::No,
        Err(_) => Support::No,
    }
}

/// Detect restrictions using concrete filesystem and runtime markers.
fn detect_restrictions(home: &Option<PathBuf>) -> Vec<Restriction> {
    let mut restrictions = Vec::new();

    if Path::new("/ostree").is_dir() || Path::new("/run/ostree-booted").exists() {
        restrictions.push(Restriction::Ostree);
    }
    if Path::new("/nix/store").is_dir() {
        restrictions.push(Restriction::Nix);
    }
    if Path::new("/.dockerenv").exists()
        || Path::new("/run/.containerenv").exists()
        || Path::new("/run/systemd/container").is_dir()
    {
        restrictions.push(Restriction::Container);
    }
    if std::env::var_os("FLATPAK_ID").is_some() {
        restrictions.push(Restriction::Flatpak);
    }
    if is_root_read_only() {
        restrictions.push(Restriction::ReadOnlyRoot);
    }
    if let Some(home) = home {
        if probe_writable(home) == Support::No {
            restrictions.push(Restriction::NotWritable(home.clone()));
        }
    }

    restrictions
}

/// Check whether the root filesystem is mounted read-only.
///
/// `/proc/self/mountinfo` is parsed because it distinguishes a read-only
/// mount from a read-only directory, and it is the only interface available
/// without additional privileges.
fn is_root_read_only() -> bool {
    let Ok(mountinfo) = std::fs::read_to_string("/proc/self/mountinfo") else {
        return false;
    };
    for line in mountinfo.lines() {
        let fields: Vec<&str> = line.split(' ').collect();
        // mountinfo: id parent major:minor root mountpoint options ...
        if fields.len() > 5 && fields[4] == "/" {
            return fields[5].split(',').any(|opt| opt == "ro");
        }
    }
    false
}

/// The user's home directory, when it can be determined.
pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn symlinks_are_supported_where_they_are_allowed() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(probe_symlinks(tmp.path()), Support::Yes);
    }

    #[test]
    fn symlink_probe_leaves_no_residue() {
        let tmp = tempfile::tempdir().unwrap();
        probe_symlinks(tmp.path());
        let leftovers: Vec<_> = std::fs::read_dir(tmp.path()).unwrap().collect();
        assert!(leftovers.is_empty(), "probe left files behind");
    }

    #[test]
    fn unwritable_directory_reports_no_write_access() {
        let tmp = tempfile::tempdir().unwrap();
        let locked = tmp.path().join("locked");
        std::fs::create_dir(&locked).unwrap();
        let mut perms = std::fs::metadata(&locked).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o500);
        std::fs::set_permissions(&locked, perms).unwrap();

        let result = probe_writable(&locked);

        // Restore permissions so the temp dir can be cleaned up.
        let mut perms = std::fs::metadata(&locked).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o700);
        std::fs::set_permissions(&locked, perms).unwrap();

        // Running as root bypasses permission checks, so the outcome depends
        // on the effective user.
        if unsafe { libc_geteuid() } == 0 {
            assert_eq!(result, Support::Yes);
        } else {
            assert_eq!(result, Support::No);
        }
    }

    #[test]
    fn missing_directory_is_unknown() {
        assert_eq!(
            probe_writable(Path::new("/nonexistent-dotpilot-probe")),
            Support::Unknown
        );
    }

    #[test]
    fn writable_directory_reports_yes() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(probe_writable(tmp.path()), Support::Yes);
    }

    #[test]
    fn probe_symlinks_works_in_a_missing_directory() {
        // The probe creates the scratch directory itself.
        let tmp = tempfile::tempdir().unwrap();
        let nested = tmp.path().join("a/b/c");
        assert_eq!(probe_symlinks(&nested), Support::Yes);
    }

    #[test]
    fn capabilities_report_no_restrictions_for_a_plain_temp_home() {
        let tmp = tempfile::tempdir().unwrap();
        let capabilities = Capabilities::probe_with(tmp.path().to_path_buf(), Some(tmp.path().to_path_buf()));
        assert_eq!(capabilities.symlinks, Support::Yes);
        assert_eq!(capabilities.home_writable, Support::Yes);
    }

    #[test]
    fn restrictions_have_descriptions() {
        assert!(Restriction::Ostree.describe().contains("ostree"));
        assert!(Restriction::Container.describe().contains("container"));
        assert!(Restriction::NotWritable(PathBuf::from("/tmp")).describe().contains("/tmp"));
    }

    #[test]
    fn scratch_file_is_not_left_behind() {
        let tmp = tempfile::tempdir().unwrap();
        let mut file = std::fs::File::create(tmp.path().join(".dotpilot-write-probe")).unwrap();
        file.write_all(b"x").unwrap();
        drop(file);
        let _ = std::fs::remove_file(tmp.path().join(".dotpilot-write-probe"));
    }

    /// Minimal `geteuid` shim so the test binary needs no extra dependency.
    fn libc_geteuid() -> u32 {
        extern "C" {
            fn geteuid() -> u32;
        }
        unsafe { geteuid() }
    }
}
