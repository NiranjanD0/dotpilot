//! Thin wrapper around the `git` command line.
//!
//! The CLI is used deliberately instead of a native Git binding: it keeps
//! DotPilot free of compiled dependencies, so the application builds and
//! behaves identically on every distribution, and it inherits whatever Git
//! version the distribution ships.
//!
//! Every call goes through [`Git::run`], which sets a controlled environment
//! so that a user's global Git configuration cannot alter snapshot behaviour
//! in surprising ways.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{CoreError, Result};

/// A Git repository on disk.
#[derive(Debug, Clone)]
pub struct Git {
    /// The working tree of the repository.
    workdir: PathBuf,
}

/// The result of a Git invocation.
#[derive(Debug, Clone)]
pub struct GitOutput {
    /// Standard output, trimmed of trailing whitespace.
    pub stdout: String,
    /// Standard error, trimmed of trailing whitespace.
    pub stderr: String,
    /// The process exit code.
    pub status: i32,
}

impl GitOutput {
    /// Whether the command succeeded.
    pub fn success(&self) -> bool {
        self.status == 0
    }
}

impl Git {
    /// Open an existing repository.
    pub fn open(workdir: impl Into<PathBuf>) -> Result<Git> {
        let workdir = workdir.into();
        if !workdir.join(".git").exists() {
            return Err(CoreError::NotFound(workdir));
        }
        Ok(Git { workdir })
    }

    /// Create a repository if one does not exist yet, otherwise open it.
    pub fn init_or_open(workdir: impl Into<PathBuf>) -> Result<Git> {
        let workdir = workdir.into();
        std::fs::create_dir_all(&workdir).map_err(|e| CoreError::io(&workdir, e))?;
        let git = Git { workdir };
        if git.workdir.join(".git").exists() {
            return Ok(git);
        }
        let output = git.run(&["init", "--quiet"])?;
        if !output.success() {
            return Err(CoreError::Invalid(format!(
                "failed to initialise git repository: {}",
                output.stderr
            )));
        }
        Ok(git)
    }

    /// The working tree path.
    pub fn workdir(&self) -> &Path {
        &self.workdir
    }

    /// Run a Git command inside the repository.
    pub fn run(&self, args: &[&str]) -> Result<GitOutput> {
        self.run_with_env(args, &[])
    }

    /// Run a Git command with extra environment variables.
    pub fn run_with_env(&self, args: &[&str], env: &[(&str, &str)]) -> Result<GitOutput> {
        let mut command = Command::new("git");
        command.current_dir(&self.workdir);
        command.args(args);

        // A deterministic environment: no user config, no prompts, no
        // credential helpers, and an explicit identity so that snapshots work
        // on systems where Git is installed but never configured.
        command.env("GIT_CONFIG_GLOBAL", "/dev/null");
        command.env("GIT_CONFIG_SYSTEM", "/dev/null");
        command.env("GIT_TERMINAL_PROMPT", "0");
        command.env("GIT_AUTHOR_NAME", "DotPilot");
        command.env("GIT_AUTHOR_EMAIL", "dotpilot@localhost");
        command.env("GIT_COMMITTER_NAME", "DotPilot");
        command.env("GIT_COMMITTER_EMAIL", "dotpilot@localhost");
        command.env("HOME", &self.workdir);
        for (key, value) in env {
            command.env(key, value);
        }

        let output = command.output().map_err(|e| {
            CoreError::Unsupported(format!("could not run git: {}. Is git installed?", e))
        })?;

        Ok(GitOutput {
            stdout: String::from_utf8_lossy(&output.stdout).trim_end().to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim_end().to_string(),
            status: output.status.code().unwrap_or(-1),
        })
    }

    /// Run a Git command, turning a non-zero exit into an error.
    pub fn run_checked(&self, args: &[&str]) -> Result<String> {
        let output = self.run(args)?;
        if !output.success() {
            return Err(CoreError::Invalid(format!(
                "git {} failed: {}",
                args.join(" "),
                if output.stderr.is_empty() {
                    &output.stdout
                } else {
                    &output.stderr
                }
            )));
        }
        Ok(output.stdout)
    }

    /// Stage everything and create a commit.
    pub fn commit_all(&self, message: &str) -> Result<String> {
        self.run_checked(&["add", "--all"])?;
        // An empty tree cannot be committed; report that as "nothing to
        // snapshot" rather than as a Git error.
        let has_changes = self
            .run(&["diff", "--cached", "--name-only"])?
            .stdout
            .lines()
            .next()
            .is_some();
        if !has_changes {
            return Err(CoreError::Invalid(
                "no changes to snapshot".to_string(),
            ));
        }
        self.run_checked(&["commit", "--quiet", "-m", message])?;
        Ok(self.run_checked(&["rev-parse", "HEAD"])?)
    }

    /// The commit identifiers, newest first, as `(hash, subject)` pairs.
    pub fn log(&self, limit: usize) -> Result<Vec<(String, String)>> {
        let output = self.run_checked(&[
            "log",
            "--pretty=format:%H%x1f%s",
            "-n",
            &limit.to_string(),
        ])?;
        Ok(output
            .lines()
            .filter_map(|line| {
                let (hash, subject) = line.split_once('\u{1f}')?;
                Some((hash.to_string(), subject.to_string()))
            })
            .collect())
    }

    /// The unified diff between two revisions, or between a revision and the
    /// working tree when `to` is `None`.
    pub fn diff(&self, from: &str, to: Option<&str>) -> Result<String> {
        let mut args = vec!["diff", "--no-color", "--no-ext-diff"];
        match to {
            Some(to) => {
                args.push(from);
                args.push(to);
            }
            None => {
                args.push(from);
            }
        }
        self.run_checked(&args)
    }

    /// Restore tracked files to the state of a revision.
    pub fn checkout(&self, revision: &str) -> Result<()> {
        self.run_checked(&["checkout", revision, "--", "."])?;
        Ok(())
    }

    /// Whether the working tree has uncommitted changes.
    pub fn has_changes(&self) -> Result<bool> {
        Ok(!self.run_checked(&["status", "--porcelain"])?.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_creates_a_repository() {
        let tmp = tempfile::tempdir().unwrap();
        let git = Git::init_or_open(tmp.path()).unwrap();
        assert!(tmp.path().join(".git").exists());
        assert_eq!(git.workdir(), tmp.path());
    }

    #[test]
    fn init_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        Git::init_or_open(tmp.path()).unwrap();
        Git::init_or_open(tmp.path()).unwrap();
        assert!(tmp.path().join(".git").exists());
    }

    #[test]
    fn commit_records_a_snapshot() {
        let tmp = tempfile::tempdir().unwrap();
        let git = Git::init_or_open(tmp.path()).unwrap();
        std::fs::write(tmp.path().join("a.txt"), "hello").unwrap();
        let hash = git.commit_all("first").unwrap();
        assert_eq!(hash.len(), 40);

        let log = git.log(10).unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].1, "first");
    }

    #[test]
    fn commit_without_changes_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let git = Git::init_or_open(tmp.path()).unwrap();
        let error = git.commit_all("nothing").unwrap_err();
        assert!(error.to_string().contains("no changes"));
    }

    #[test]
    fn diff_shows_changes_against_a_snapshot() {
        let tmp = tempfile::tempdir().unwrap();
        let git = Git::init_or_open(tmp.path()).unwrap();
        std::fs::write(tmp.path().join("a.txt"), "one\n").unwrap();
        let hash = git.commit_all("first").unwrap();

        std::fs::write(tmp.path().join("a.txt"), "two\n").unwrap();
        let diff = git.diff(&hash, None).unwrap();
        assert!(diff.contains("-one"));
        assert!(diff.contains("+two"));
    }

    #[test]
    fn checkout_restores_previous_content() {
        let tmp = tempfile::tempdir().unwrap();
        let git = Git::init_or_open(tmp.path()).unwrap();
        std::fs::write(tmp.path().join("a.txt"), "one\n").unwrap();
        let hash = git.commit_all("first").unwrap();
        std::fs::write(tmp.path().join("a.txt"), "two\n").unwrap();

        git.checkout(&hash).unwrap();
        assert_eq!(std::fs::read_to_string(tmp.path().join("a.txt")).unwrap(), "one\n");
    }

    #[test]
    fn user_git_config_does_not_affect_commits() {
        // The wrapper sets an explicit identity, so commits succeed even when
        // the ambient environment has no Git configuration at all.
        let tmp = tempfile::tempdir().unwrap();
        let git = Git::init_or_open(tmp.path()).unwrap();
        std::fs::write(tmp.path().join("a.txt"), "x").unwrap();
        assert!(git.commit_all("snapshot").is_ok());
    }

    #[test]
    fn open_fails_for_a_non_repository() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(Git::open(tmp.path()).is_err());
    }
}
