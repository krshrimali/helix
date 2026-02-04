//! GitHub PR integration for Helix editor.
//!
//! This module provides data structures for GitHub pull request integration,
//! allowing users to browse PRs, view diffs, and navigate changed files.

use serde::{Deserialize, Serialize};

/// Prefix for GitHub PR diff buffer names.
pub const GITHUB_PR_BUFFER_PREFIX: &str = "[pr-diff] ";

/// Pull request state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum PRState {
    Open,
    Closed,
    Merged,
}

impl std::fmt::Display for PRState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PRState::Open => write!(f, "OPEN"),
            PRState::Closed => write!(f, "CLOSED"),
            PRState::Merged => write!(f, "MERGED"),
        }
    }
}

/// File status in a pull request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PRFileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
}

impl std::fmt::Display for PRFileStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PRFileStatus::Added => write!(f, "+"),
            PRFileStatus::Modified => write!(f, "~"),
            PRFileStatus::Deleted => write!(f, "-"),
            PRFileStatus::Renamed => write!(f, "→"),
            PRFileStatus::Copied => write!(f, "⊕"),
        }
    }
}

/// A GitHub user (author, reviewer, assignee).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubUser {
    pub login: String,
}

/// A GitHub label.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubLabel {
    pub name: String,
}

/// A GitHub pull request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub author: GitHubUser,
    pub state: PRState,
    #[serde(default)]
    pub is_draft: bool,
    pub base_ref_name: String,
    pub head_ref_name: String,
    pub url: String,
    pub additions: u64,
    pub deletions: u64,
    pub changed_files: u64,
    #[serde(default)]
    pub labels: Vec<GitHubLabel>,
    #[serde(default)]
    pub assignees: Vec<GitHubUser>,
    #[serde(default)]
    pub review_requests: Vec<ReviewRequest>,
}

impl PullRequest {
    /// Get the author's login name.
    pub fn author_login(&self) -> &str {
        &self.author.login
    }

    /// Get label names as a vector of strings.
    pub fn label_names(&self) -> Vec<&str> {
        self.labels.iter().map(|l| l.name.as_str()).collect()
    }

    /// Get assignee logins as a vector of strings.
    pub fn assignee_logins(&self) -> Vec<&str> {
        self.assignees.iter().map(|u| u.login.as_str()).collect()
    }

    /// Get reviewer logins as a vector of strings.
    pub fn reviewer_logins(&self) -> Vec<&str> {
        self.review_requests
            .iter()
            .filter_map(|r| r.requested_reviewer.as_ref())
            .map(|u| u.login.as_str())
            .collect()
    }

    /// Get a display string for the state (including draft).
    pub fn state_display(&self) -> &'static str {
        if self.is_draft {
            "DRAFT"
        } else {
            match self.state {
                PRState::Open => "OPEN",
                PRState::Closed => "CLOSED",
                PRState::Merged => "MERGED",
            }
        }
    }
}

/// A review request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewRequest {
    pub requested_reviewer: Option<GitHubUser>,
}

/// A file changed in a pull request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PRFile {
    pub path: String,
    pub status: PRFileStatus,
    pub additions: u64,
    pub deletions: u64,
    #[serde(default)]
    pub patch: Option<String>,
}

impl PRFile {
    /// Get the display icon for this file's status.
    pub fn status_icon(&self) -> &'static str {
        match self.status {
            PRFileStatus::Added => "+",
            PRFileStatus::Modified => "~",
            PRFileStatus::Deleted => "-",
            PRFileStatus::Renamed => "→",
            PRFileStatus::Copied => "⊕",
        }
    }
}

/// Filter options for listing pull requests.
#[derive(Debug, Clone, Default)]
pub struct PRFilter {
    /// Filter by PR state (open, closed, merged, all).
    pub state: Option<String>,
    /// Filter by author login.
    pub author: Option<String>,
    /// Filter by assignee login.
    pub assignee: Option<String>,
    /// Filter PRs where I'm requested as a reviewer.
    pub review_requested: bool,
    /// Limit the number of results.
    pub limit: Option<u32>,
}

impl PRFilter {
    /// Create a filter for open PRs only.
    pub fn open() -> Self {
        Self {
            state: Some("open".to_string()),
            ..Default::default()
        }
    }

    /// Create a filter for PRs authored by a specific user.
    pub fn by_author(author: &str) -> Self {
        Self {
            author: Some(author.to_string()),
            ..Default::default()
        }
    }

    /// Create a filter for PRs where I'm requested as reviewer.
    pub fn review_requested() -> Self {
        Self {
            review_requested: true,
            ..Default::default()
        }
    }
}

/// State for a PR diff buffer.
#[derive(Debug, Clone)]
pub struct PRDiffState {
    /// The PR number.
    pub pr_number: u64,
    /// The PR title.
    pub pr_title: String,
    /// Files changed in the PR.
    pub files: Vec<PRFile>,
    /// Current file index being viewed.
    pub current_file_index: usize,
    /// The unified diff content.
    pub unified_diff: String,
    /// Line offsets for each file in the diff.
    pub file_line_offsets: Vec<(usize, usize)>, // (start_line, end_line)
    /// Hunk line numbers for navigation.
    pub hunk_lines: Vec<usize>,
}

impl PRDiffState {
    /// Create a new PR diff state.
    pub fn new(pr_number: u64, pr_title: String) -> Self {
        Self {
            pr_number,
            pr_title,
            files: Vec::new(),
            current_file_index: 0,
            unified_diff: String::new(),
            file_line_offsets: Vec::new(),
            hunk_lines: Vec::new(),
        }
    }

    /// Get the buffer name for this PR diff.
    pub fn buffer_name(&self) -> String {
        format!("{}#{} - {}", GITHUB_PR_BUFFER_PREFIX, self.pr_number, self.pr_title)
    }

    /// Get the current file being viewed.
    pub fn current_file(&self) -> Option<&PRFile> {
        self.files.get(self.current_file_index)
    }

    /// Move to the next file.
    pub fn next_file(&mut self) -> bool {
        if self.current_file_index + 1 < self.files.len() {
            self.current_file_index += 1;
            true
        } else {
            false
        }
    }

    /// Move to the previous file.
    pub fn prev_file(&mut self) -> bool {
        if self.current_file_index > 0 {
            self.current_file_index -= 1;
            true
        } else {
            false
        }
    }

    /// Find the next hunk line after the given line.
    pub fn next_hunk_line(&self, current_line: usize) -> Option<usize> {
        self.hunk_lines.iter().find(|&&line| line > current_line).copied()
    }

    /// Find the previous hunk line before the given line.
    pub fn prev_hunk_line(&self, current_line: usize) -> Option<usize> {
        self.hunk_lines.iter().rev().find(|&&line| line < current_line).copied()
    }

    /// Get the file index for a given line number.
    pub fn file_at_line(&self, line: usize) -> Option<usize> {
        self.file_line_offsets
            .iter()
            .position(|(start, end)| line >= *start && line <= *end)
    }
}

/// Check if a buffer name indicates a GitHub PR diff buffer.
pub fn is_github_pr_buffer_name(name: &str) -> bool {
    name.starts_with(GITHUB_PR_BUFFER_PREFIX)
}

/// Extract the PR number from a GitHub PR buffer name.
pub fn pr_number_from_buffer_name(name: &str) -> Option<u64> {
    name.strip_prefix(GITHUB_PR_BUFFER_PREFIX)
        .and_then(|rest| rest.strip_prefix('#'))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|num| num.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_github_pr_buffer_name() {
        assert!(is_github_pr_buffer_name("[pr-diff] #123 - Fix bug"));
        assert!(!is_github_pr_buffer_name("[oil] /home/user"));
        assert!(!is_github_pr_buffer_name("test.rs"));
    }

    #[test]
    fn test_pr_number_from_buffer_name() {
        assert_eq!(
            pr_number_from_buffer_name("[pr-diff] #123 - Fix bug"),
            Some(123)
        );
        assert_eq!(pr_number_from_buffer_name("[pr-diff] #456"), Some(456));
        assert_eq!(pr_number_from_buffer_name("[oil] /home"), None);
    }

    #[test]
    fn test_pr_state_display() {
        assert_eq!(PRState::Open.to_string(), "OPEN");
        assert_eq!(PRState::Closed.to_string(), "CLOSED");
        assert_eq!(PRState::Merged.to_string(), "MERGED");
    }

    #[test]
    fn test_pr_file_status_icon() {
        let file = PRFile {
            path: "test.rs".to_string(),
            status: PRFileStatus::Added,
            additions: 10,
            deletions: 0,
            patch: None,
        };
        assert_eq!(file.status_icon(), "+");
    }
}
