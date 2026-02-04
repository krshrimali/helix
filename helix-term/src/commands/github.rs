//! GitHub PR commands for Helix editor.
//!
//! This module provides commands for browsing and viewing GitHub pull requests,
//! using the `gh` CLI for GitHub API access.

use helix_core::{Rope, Selection};
use helix_view::{
    editor::Action,
    github::{PRDiffState, PRFile, PRFileStatus, PRFilter, PRState, PullRequest, GITHUB_PR_BUFFER_PREFIX},
};
use serde::Deserialize;
use tui::text::Span;

use crate::{
    compositor::Compositor,
    job::Callback,
    ui::{overlay::overlaid, Picker, PickerColumn},
};

use super::Context;

use std::process::Stdio;

/// Run a `gh` CLI command and parse the JSON output.
async fn run_gh_command<T: for<'de> Deserialize<'de>>(args: &[&str]) -> anyhow::Result<T> {
    use tokio::process::Command;

    let output = Command::new("gh")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("gh command failed: {}", stderr);
    }

    let result: T = serde_json::from_slice(&output.stdout)?;
    Ok(result)
}

/// List pull requests with optional filters.
async fn list_prs(filter: &PRFilter) -> anyhow::Result<Vec<PullRequest>> {
    let mut args = vec![
        "pr",
        "list",
        "--json",
        "number,title,body,author,state,isDraft,baseRefName,headRefName,url,additions,deletions,changedFiles,labels,assignees,reviewRequests",
    ];

    let state_arg;
    if let Some(state) = &filter.state {
        state_arg = format!("--state={}", state);
        args.push(&state_arg);
    }

    let author_arg;
    if let Some(author) = &filter.author {
        author_arg = format!("--author={}", author);
        args.push(&author_arg);
    }

    let assignee_arg;
    if let Some(assignee) = &filter.assignee {
        assignee_arg = format!("--assignee={}", assignee);
        args.push(&assignee_arg);
    }

    if filter.review_requested {
        args.push("--search=review-requested:@me");
    }

    let limit_arg;
    if let Some(limit) = filter.limit {
        limit_arg = format!("--limit={}", limit);
        args.push(&limit_arg);
    }

    run_gh_command(&args).await
}

/// Get the unified diff for a pull request.
async fn get_pr_diff(number: u64) -> anyhow::Result<String> {
    use tokio::process::Command;

    let output = Command::new("gh")
        .args(["pr", "diff", &number.to_string()])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("gh pr diff failed: {}", stderr);
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Get files changed in a pull request.
async fn get_pr_files(number: u64) -> anyhow::Result<Vec<PRFile>> {
    #[derive(Deserialize)]
    struct GhFile {
        path: String,
        status: String,
        additions: u64,
        deletions: u64,
    }

    let args = [
        "pr",
        "view",
        &number.to_string(),
        "--json",
        "files",
    ];

    #[derive(Deserialize)]
    struct FilesResponse {
        files: Vec<GhFile>,
    }

    let response: FilesResponse = run_gh_command(&args).await?;

    let files = response
        .files
        .into_iter()
        .map(|f| PRFile {
            path: f.path,
            status: match f.status.as_str() {
                "added" => PRFileStatus::Added,
                "removed" | "deleted" => PRFileStatus::Deleted,
                "renamed" => PRFileStatus::Renamed,
                "copied" => PRFileStatus::Copied,
                _ => PRFileStatus::Modified,
            },
            additions: f.additions,
            deletions: f.deletions,
            patch: None,
        })
        .collect();

    Ok(files)
}

/// Get the current GitHub username.
async fn get_current_user() -> anyhow::Result<String> {
    use tokio::process::Command;

    let output = Command::new("gh")
        .args(["api", "user", "--jq", ".login"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("gh api user failed: {}", stderr);
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Picker data for PR listing.
#[derive(Debug)]
struct PRPickerData {
    state_style: helix_view::theme::Style,
    draft_style: helix_view::theme::Style,
    merged_style: helix_view::theme::Style,
    closed_style: helix_view::theme::Style,
}

/// Open the PR picker with the given filter.
fn open_pr_picker(cx: &mut Context, filter: PRFilter) {
    let callback = async move {
        let prs = match list_prs(&filter).await {
            Ok(prs) => prs,
            Err(e) => {
                return Ok(Callback::EditorCompositor(Box::new(
                    move |editor: &mut helix_view::Editor, _compositor: &mut Compositor| {
                        editor.set_error(format!("Failed to list PRs: {}", e));
                    },
                )));
            }
        };

        let call: Callback = Callback::EditorCompositor(Box::new(
            move |editor: &mut helix_view::Editor, compositor: &mut Compositor| {
                let data = PRPickerData {
                    state_style: editor.theme.get("ui.text.info"),
                    draft_style: editor.theme.get("ui.text"),
                    merged_style: editor.theme.get("diff.plus"),
                    closed_style: editor.theme.get("diff.minus"),
                };

                let columns = [
                    PickerColumn::new("#", |pr: &PullRequest, _data: &PRPickerData| {
                        format!("#{}", pr.number).into()
                    }),
                    PickerColumn::new("State", |pr: &PullRequest, data: &PRPickerData| {
                        let style = if pr.is_draft {
                            data.draft_style
                        } else {
                            match pr.state {
                                PRState::Open => data.state_style,
                                PRState::Merged => data.merged_style,
                                PRState::Closed => data.closed_style,
                            }
                        };
                        Span::styled(pr.state_display(), style).into()
                    }),
                    PickerColumn::new("Title", |pr: &PullRequest, _data: &PRPickerData| {
                        pr.title.clone().into()
                    }),
                    PickerColumn::new("Author", |pr: &PullRequest, _data: &PRPickerData| {
                        pr.author_login().to_string().into()
                    }),
                    PickerColumn::new("+/-", |pr: &PullRequest, _data: &PRPickerData| {
                        format!("+{} -{}", pr.additions, pr.deletions).into()
                    }),
                ];

                let picker = Picker::new(
                    columns,
                    2, // Focus on title column
                    prs,
                    data,
                    move |cx, pr: &PullRequest, _action| {
                        open_pr_diff(cx, pr.number, pr.title.clone());
                    },
                )
;

                compositor.push(Box::new(overlaid(picker)));
            },
        ));

        Ok(call)
    };

    cx.jobs.callback(callback);
}

/// Open the diff buffer for a pull request.
fn open_pr_diff(cx: &mut crate::compositor::Context, pr_number: u64, pr_title: String) {
    let callback = async move {
        // Fetch diff and files in parallel
        let (diff_result, files_result) = tokio::join!(
            get_pr_diff(pr_number),
            get_pr_files(pr_number)
        );

        let diff = match diff_result {
            Ok(d) => d,
            Err(e) => {
                return Ok(Callback::EditorCompositor(Box::new(
                    move |editor: &mut helix_view::Editor, _compositor: &mut Compositor| {
                        editor.set_error(format!("Failed to get PR diff: {}", e));
                    },
                )));
            }
        };

        let files = match files_result {
            Ok(f) => f,
            Err(e) => {
                return Ok(Callback::EditorCompositor(Box::new(
                    move |editor: &mut helix_view::Editor, _compositor: &mut Compositor| {
                        editor.set_error(format!("Failed to get PR files: {}", e));
                    },
                )));
            }
        };

        let call: Callback = Callback::EditorCompositor(Box::new(
            move |editor: &mut helix_view::Editor, _compositor: &mut Compositor| {
                // Generate buffer content
                let content = generate_pr_diff_content(pr_number, &pr_title, &files, &diff);

                // Parse hunk lines and file offsets
                let (hunk_lines, file_line_offsets) = parse_diff_structure(&content);

                // Create the document
                let rope = Rope::from(content.as_str());
                let mut doc = helix_view::Document::from(
                    rope,
                    None,
                    editor.config.clone(),
                    editor.syn_loader.clone(),
                );

                // Set custom name
                let buffer_name = format!("{}#{} - {}", GITHUB_PR_BUFFER_PREFIX, pr_number, pr_title);
                doc.set_custom_name(buffer_name);

                // Set syntax to diff
                let loader = editor.syn_loader.load();
                let _ = doc.set_language_by_language_id("diff", &loader);

                // Add document
                let doc_id = editor.new_file_from_document(Action::Replace, doc);

                // Create and store PR diff state
                let mut pr_state = PRDiffState::new(pr_number, pr_title);
                pr_state.files = files;
                pr_state.unified_diff = diff;
                pr_state.hunk_lines = hunk_lines;
                pr_state.file_line_offsets = file_line_offsets;
                editor.github_pr_buffers.insert(doc_id, pr_state);

                // Mark as not modified
                if let Some(doc) = editor.documents.get_mut(&doc_id) {
                    doc.reset_modified();

                    // Position cursor at first diff line (after header)
                    let view_id = editor.tree.focus;
                    doc.ensure_view_init(view_id);
                    let text = doc.text();
                    let start_line = 6.min(text.len_lines().saturating_sub(1)); // Skip header
                    let pos = text.line_to_char(start_line);
                    doc.set_selection(view_id, Selection::point(pos));
                }

                editor.set_status(format!("PR #{} - {} files changed", pr_number, editor.github_pr_buffers.get(&doc_id).map(|s| s.files.len()).unwrap_or(0)));
            },
        ));

        Ok(call)
    };

    cx.jobs.callback(callback);
}

/// Generate the buffer content for a PR diff.
fn generate_pr_diff_content(pr_number: u64, title: &str, files: &[PRFile], diff: &str) -> String {
    let mut content = String::new();

    // Header
    content.push_str(&format!("# PR #{} - {}\n", pr_number, title));
    content.push_str(&format!("# {} files changed\n", files.len()));
    content.push_str("# Keys: [n]ext file | [p]rev file | ]c/[c next/prev hunk | [q]uit\n");
    content.push_str("#\n");

    // File summary
    content.push_str("# Files:\n");
    for file in files {
        content.push_str(&format!(
            "#   {} {} (+{} -{})\n",
            file.status_icon(),
            file.path,
            file.additions,
            file.deletions
        ));
    }
    content.push_str("#\n");
    content.push_str("# ─────────────────────────────────────────────────────────────────────\n");
    content.push('\n');

    // Diff content
    content.push_str(diff);

    content
}

/// Parse the diff content to find hunk lines and file offsets.
fn parse_diff_structure(content: &str) -> (Vec<usize>, Vec<(usize, usize)>) {
    let mut hunk_lines = Vec::new();
    let mut file_line_offsets = Vec::new();
    let mut current_file_start: Option<usize> = None;

    for (line_num, line) in content.lines().enumerate() {
        if line.starts_with("diff --git") {
            // Start of a new file
            if let Some(start) = current_file_start {
                file_line_offsets.push((start, line_num.saturating_sub(1)));
            }
            current_file_start = Some(line_num);
        } else if line.starts_with("@@") {
            // Hunk header
            hunk_lines.push(line_num);
        }
    }

    // Close the last file
    if let Some(start) = current_file_start {
        file_line_offsets.push((start, content.lines().count().saturating_sub(1)));
    }

    (hunk_lines, file_line_offsets)
}

// ============================================================================
// Public command functions
// ============================================================================

/// Open the GitHub PR picker (all PRs).
pub fn github_pr_picker(cx: &mut Context) {
    open_pr_picker(cx, PRFilter::default());
}

/// Open the GitHub PR picker for open PRs only.
pub fn github_pr_picker_open(cx: &mut Context) {
    open_pr_picker(cx, PRFilter::open());
}

/// Open the GitHub PR picker for my PRs.
pub fn github_pr_picker_mine(cx: &mut Context) {
    let callback = async move {
        let username = match get_current_user().await {
            Ok(u) => u,
            Err(e) => {
                return Ok(Callback::EditorCompositor(Box::new(
                    move |editor: &mut helix_view::Editor, _compositor: &mut Compositor| {
                        editor.set_error(format!("Failed to get current user: {}", e));
                    },
                )));
            }
        };

        let filter = PRFilter::by_author(&username);

        let prs = match list_prs(&filter).await {
            Ok(prs) => prs,
            Err(e) => {
                return Ok(Callback::EditorCompositor(Box::new(
                    move |editor: &mut helix_view::Editor, _compositor: &mut Compositor| {
                        editor.set_error(format!("Failed to list PRs: {}", e));
                    },
                )));
            }
        };

        let call: Callback = Callback::EditorCompositor(Box::new(
            move |editor: &mut helix_view::Editor, compositor: &mut Compositor| {
                let data = PRPickerData {
                    state_style: editor.theme.get("ui.text.info"),
                    draft_style: editor.theme.get("ui.text"),
                    merged_style: editor.theme.get("diff.plus"),
                    closed_style: editor.theme.get("diff.minus"),
                };

                let columns = [
                    PickerColumn::new("#", |pr: &PullRequest, _data: &PRPickerData| {
                        format!("#{}", pr.number).into()
                    }),
                    PickerColumn::new("State", |pr: &PullRequest, data: &PRPickerData| {
                        let style = if pr.is_draft {
                            data.draft_style
                        } else {
                            match pr.state {
                                PRState::Open => data.state_style,
                                PRState::Merged => data.merged_style,
                                PRState::Closed => data.closed_style,
                            }
                        };
                        Span::styled(pr.state_display(), style).into()
                    }),
                    PickerColumn::new("Title", |pr: &PullRequest, _data: &PRPickerData| {
                        pr.title.clone().into()
                    }),
                    PickerColumn::new("+/-", |pr: &PullRequest, _data: &PRPickerData| {
                        format!("+{} -{}", pr.additions, pr.deletions).into()
                    }),
                ];

                let picker = Picker::new(
                    columns,
                    2, // Focus on title column
                    prs,
                    data,
                    move |cx, pr: &PullRequest, _action| {
                        open_pr_diff(cx, pr.number, pr.title.clone());
                    },
                );

                compositor.push(Box::new(overlaid(picker)));
            },
        ));

        Ok(call)
    };

    cx.jobs.callback(callback);
}

/// Open the GitHub PR picker for PRs needing my review.
pub fn github_pr_picker_review(cx: &mut Context) {
    open_pr_picker(cx, PRFilter::review_requested());
}

/// Navigate to the next file in a PR diff buffer.
pub fn github_pr_next_file(cx: &mut Context) {
    let doc_id = {
        let (_, doc) = current!(cx.editor);
        doc.id()
    };

    let pr_state = match cx.editor.github_pr_state_mut(doc_id) {
        Some(state) => state,
        None => return, // Not a PR diff buffer
    };

    let next_file_idx = pr_state.current_file_index + 1;
    if next_file_idx >= pr_state.files.len() {
        cx.editor.set_status("Already at last file");
        return;
    }

    if let Some((start_line, _)) = pr_state.file_line_offsets.get(next_file_idx) {
        pr_state.current_file_index = next_file_idx;
        let start_line = *start_line;

        // Move cursor to the file
        let (view, doc) = current!(cx.editor);
        let text = doc.text();
        if start_line < text.len_lines() {
            let pos = text.line_to_char(start_line);
            doc.set_selection(view.id, Selection::point(pos));
        }

        if let Some(file) = cx.editor.github_pr_state(doc_id).and_then(|s| s.current_file()) {
            cx.editor.set_status(format!("File {}/{}: {}", next_file_idx + 1, cx.editor.github_pr_state(doc_id).map(|s| s.files.len()).unwrap_or(0), file.path));
        }
    }
}

/// Navigate to the previous file in a PR diff buffer.
pub fn github_pr_prev_file(cx: &mut Context) {
    let doc_id = {
        let (_, doc) = current!(cx.editor);
        doc.id()
    };

    let pr_state = match cx.editor.github_pr_state_mut(doc_id) {
        Some(state) => state,
        None => return, // Not a PR diff buffer
    };

    if pr_state.current_file_index == 0 {
        cx.editor.set_status("Already at first file");
        return;
    }

    let prev_file_idx = pr_state.current_file_index - 1;

    if let Some((start_line, _)) = pr_state.file_line_offsets.get(prev_file_idx) {
        pr_state.current_file_index = prev_file_idx;
        let start_line = *start_line;

        // Move cursor to the file
        let (view, doc) = current!(cx.editor);
        let text = doc.text();
        if start_line < text.len_lines() {
            let pos = text.line_to_char(start_line);
            doc.set_selection(view.id, Selection::point(pos));
        }

        if let Some(file) = cx.editor.github_pr_state(doc_id).and_then(|s| s.current_file()) {
            cx.editor.set_status(format!("File {}/{}: {}", prev_file_idx + 1, cx.editor.github_pr_state(doc_id).map(|s| s.files.len()).unwrap_or(0), file.path));
        }
    }
}

/// Navigate to the next hunk in a PR diff buffer.
pub fn github_pr_next_hunk(cx: &mut Context) {
    let (doc_id, current_line) = {
        let (view, doc) = current!(cx.editor);
        let text = doc.text();
        let cursor = doc.selection(view.id).primary().cursor(text.slice(..));
        (doc.id(), text.char_to_line(cursor))
    };

    // Get hunk lines from the PR state
    let hunk_lines = match cx.editor.github_pr_state(doc_id) {
        Some(state) => state.hunk_lines.clone(),
        None => return, // Not a PR diff buffer
    };

    // Find next hunk line
    let next_line = hunk_lines.iter().find(|&&line| line > current_line).copied();

    if let Some(line) = next_line {
        let (view, doc) = current!(cx.editor);
        let text = doc.text();
        if line < text.len_lines() {
            let pos = text.line_to_char(line);
            doc.set_selection(view.id, Selection::point(pos));
        }
    } else {
        cx.editor.set_status("No more hunks");
    }
}

/// Navigate to the previous hunk in a PR diff buffer.
pub fn github_pr_prev_hunk(cx: &mut Context) {
    let (doc_id, current_line) = {
        let (view, doc) = current!(cx.editor);
        let text = doc.text();
        let cursor = doc.selection(view.id).primary().cursor(text.slice(..));
        (doc.id(), text.char_to_line(cursor))
    };

    // Get hunk lines from the PR state
    let hunk_lines = match cx.editor.github_pr_state(doc_id) {
        Some(state) => state.hunk_lines.clone(),
        None => return, // Not a PR diff buffer
    };

    // Find previous hunk line
    let prev_line = hunk_lines.iter().rev().find(|&&line| line < current_line).copied();

    if let Some(line) = prev_line {
        let (view, doc) = current!(cx.editor);
        let text = doc.text();
        if line < text.len_lines() {
            let pos = text.line_to_char(line);
            doc.set_selection(view.id, Selection::point(pos));
        }
    } else {
        cx.editor.set_status("No previous hunks");
    }
}
