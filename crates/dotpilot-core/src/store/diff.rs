//! Textual diffing for the configuration diff viewer.
//!
//! The Git layer produces repository-level diffs. This module produces
//! line-level diffs between two arbitrary files, which is what the viewer
//! shows when a user compares a managed file against its snapshot.

use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};

/// The kind of change represented by a diff line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LineKind {
    /// Present in both versions.
    Context,
    /// Present only in the original.
    Removed,
    /// Present only in the modified version.
    Added,
}

/// A single line of a diff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffLine {
    /// The line's classification.
    pub kind: LineKind,
    /// The text of the line, without its trailing newline.
    pub text: String,
    /// The 1-based line number in the original file, when applicable.
    pub original_line: Option<usize>,
    /// The 1-based line number in the modified file, when applicable.
    pub modified_line: Option<usize>,
}

/// The result of diffing two files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextDiffResult {
    /// The individual diff lines.
    pub lines: Vec<DiffLine>,
    /// The number of added lines.
    pub added: usize,
    /// The number of removed lines.
    pub removed: usize,
}

impl TextDiffResult {
    /// Whether the two texts are identical.
    pub fn is_identical(&self) -> bool {
        self.added == 0 && self.removed == 0
    }
}

/// Diff two text strings.
pub fn diff_text(original: &str, modified: &str) -> TextDiffResult {
    let diff = TextDiff::from_lines(original, modified);
    let mut lines = Vec::new();
    let mut added = 0;
    let mut removed = 0;

    for change in diff.iter_all_changes() {
        let text = change.value().trim_end_matches(['\n', '\r']).to_string();
        match change.tag() {
            ChangeTag::Delete => {
                removed += 1;
                lines.push(DiffLine {
                    kind: LineKind::Removed,
                    text,
                    original_line: change.old_index().map(|i| i + 1),
                    modified_line: None,
                });
            }
            ChangeTag::Insert => {
                added += 1;
                lines.push(DiffLine {
                    kind: LineKind::Added,
                    text,
                    original_line: None,
                    modified_line: change.new_index().map(|i| i + 1),
                });
            }
            ChangeTag::Equal => {
                lines.push(DiffLine {
                    kind: LineKind::Context,
                    text,
                    original_line: change.old_index().map(|i| i + 1),
                    modified_line: change.new_index().map(|i| i + 1),
                });
            }
        }
    }

    TextDiffResult {
        lines,
        added,
        removed,
    }
}

/// Diff two files on disk.
///
/// Missing files are treated as empty text, so diffing a file against a
/// deleted file reports every line as removed rather than failing.
pub fn diff_files(original: &std::path::Path, modified: &std::path::Path) -> crate::error::Result<TextDiffResult> {
    let original_text = read_text(original);
    let modified_text = read_text(modified);
    Ok(diff_text(&original_text, &modified_text))
}

/// Read a file as text, returning an empty string when it cannot be read.
fn read_text(path: &std::path::Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_text_has_no_changes() {
        let result = diff_text("a\nb\n", "a\nb\n");
        assert!(result.is_identical());
        assert_eq!(result.lines.len(), 2);
        assert!(result.lines.iter().all(|l| l.kind == LineKind::Context));
    }

    #[test]
    fn changed_lines_are_classified() {
        let result = diff_text("a\nb\n", "a\nc\n");
        assert_eq!(result.added, 1);
        assert_eq!(result.removed, 1);
        assert!(result.lines.iter().any(|l| l.kind == LineKind::Removed && l.text == "b"));
        assert!(result.lines.iter().any(|l| l.kind == LineKind::Added && l.text == "c"));
    }

    #[test]
    fn added_lines_are_numbered() {
        let result = diff_text("", "new\n");
        assert_eq!(result.added, 1);
        let added = result
            .lines
            .iter()
            .find(|l| l.kind == LineKind::Added)
            .unwrap();
        assert_eq!(added.modified_line, Some(1));
        assert_eq!(added.original_line, None);
    }

    #[test]
    fn diffing_against_a_missing_file_reports_removals() {
        let tmp = tempfile::tempdir().unwrap();
        let original = tmp.path().join("a.txt");
        std::fs::write(&original, "one\ntwo\n").unwrap();
        let missing = tmp.path().join("missing.txt");

        let result = diff_files(&original, &missing).unwrap();
        assert_eq!(result.removed, 2);
        assert_eq!(result.added, 0);
    }

    #[test]
    fn line_endings_are_stripped_from_the_text() {
        let result = diff_text("a\r\nb\r\n", "a\r\nb\r\n");
        assert!(result.lines.iter().all(|l| !l.text.contains('\r')));
    }
}
