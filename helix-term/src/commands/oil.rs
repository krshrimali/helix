//! Oil file manager commands for Helix editor.
//!
//! This module contains commands for the oil.nvim-like file manager that allows
//! editing the filesystem like a buffer.

use helix_core::find_workspace;
use helix_view::{
    editor::Action,
    oil::{self, OilFileType},
};

use super::Context;

use std::path::PathBuf;

/// Open oil buffer at workspace root.
pub fn oil_open(cx: &mut Context) {
    let directory = find_workspace().0;
    if !directory.exists() {
        cx.editor.set_error("Workspace directory does not exist");
        return;
    }
    open_oil_at(cx, directory);
}

/// Open oil buffer at current working directory.
pub fn oil_open_cwd(cx: &mut Context) {
    let directory = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    open_oil_at(cx, directory);
}

/// Open oil buffer at current buffer's directory.
pub fn oil_open_buffer_dir(cx: &mut Context) {
    let directory = {
        let (_, doc) = current!(cx.editor);
        doc.path()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    };

    open_oil_at(cx, directory);
}

/// Helper to open oil at a specific directory.
fn open_oil_at(cx: &mut Context, directory: PathBuf) {
    match cx.editor.open_oil_buffer(directory, Action::Replace) {
        Ok(_) => {}
        Err(e) => {
            cx.editor.set_error(format!("Failed to open oil: {}", e));
        }
    }
}

/// Open file or navigate into directory in oil buffer.
pub fn oil_enter(cx: &mut Context) {
    // Get document id and line content first
    let (doc_id, line_str) = {
        let (view, doc) = current!(cx.editor);
        let text = doc.text();
        let cursor = doc.selection(view.id).primary().cursor(text.slice(..));
        let line_idx = text.char_to_line(cursor);
        let line = text.line(line_idx);
        (doc.id(), line.to_string())
    };

    // Get oil state (this borrows editor immutably)
    let oil_state = match cx.editor.oil_state(doc_id) {
        Some(state) => state.clone(),
        None => {
            // Not an oil buffer, do nothing
            return;
        }
    };

    // Parse the line
    let parsed = match oil::parse_oil_line(&line_str) {
        Some(p) => p,
        None => return, // Comment or empty line
    };

    // Find the corresponding entry in original entries
    let entry = oil_state
        .original_entries
        .iter()
        .find(|e| e.name == parsed.name);

    let path_from_entry = entry.map(|e| (e.original_path.clone(), e.file_type.clone()));

    if let Some((path, file_type)) = path_from_entry {
        if file_type == OilFileType::Directory {
            // Navigate into the directory
            match cx.editor.open_oil_buffer(path, Action::Replace) {
                Ok(_) => {}
                Err(e) => {
                    cx.editor
                        .set_error(format!("Failed to open directory: {}", e));
                }
            }
        } else {
            // Open the file
            match cx.editor.open(&path, Action::Replace) {
                Ok(_) => {}
                Err(e) => {
                    cx.editor.set_error(format!("Failed to open file: {}", e));
                }
            }
        }
    } else {
        // This might be a newly created entry that doesn't exist yet
        let path = oil_state.directory.join(&parsed.name);
        if parsed.is_dir {
            // Try to navigate into it if it exists
            if path.is_dir() {
                match cx.editor.open_oil_buffer(path, Action::Replace) {
                    Ok(_) => {}
                    Err(e) => {
                        cx.editor
                            .set_error(format!("Failed to open directory: {}", e));
                    }
                }
            } else {
                cx.editor
                    .set_error("Directory does not exist yet. Save the buffer first.");
            }
        } else {
            // Try to open the file if it exists
            if path.is_file() {
                match cx.editor.open(&path, Action::Replace) {
                    Ok(_) => {}
                    Err(e) => {
                        cx.editor.set_error(format!("Failed to open file: {}", e));
                    }
                }
            } else {
                cx.editor
                    .set_error("File does not exist yet. Save the buffer first.");
            }
        }
    }
}

/// Navigate to parent directory in oil buffer.
pub fn oil_parent(cx: &mut Context) {
    let doc_id = {
        let (_, doc) = current!(cx.editor);
        doc.id()
    };

    let oil_state = match cx.editor.oil_state(doc_id) {
        Some(state) => state.clone(),
        None => return, // Not an oil buffer
    };

    // Get parent directory
    let parent = match oil_state.directory.parent() {
        Some(p) => p.to_path_buf(),
        None => {
            cx.editor.set_status("Already at root directory");
            return;
        }
    };

    match cx.editor.open_oil_buffer(parent, Action::Replace) {
        Ok(_) => {}
        Err(e) => {
            cx.editor
                .set_error(format!("Failed to open parent directory: {}", e));
        }
    }
}

/// Toggle hidden files visibility in oil buffer.
pub fn oil_toggle_hidden(cx: &mut Context) {
    let doc_id = {
        let (_, doc) = current!(cx.editor);
        doc.id()
    };

    // Toggle the show_hidden flag
    let new_show_hidden = if let Some(state) = cx.editor.oil_state_mut(doc_id) {
        state.show_hidden = !state.show_hidden;
        state.show_hidden
    } else {
        return; // Not an oil buffer
    };

    // Refresh the buffer
    match cx.editor.refresh_oil_buffer(doc_id) {
        Ok(_) => {
            let msg = if new_show_hidden {
                "Showing hidden files"
            } else {
                "Hiding hidden files"
            };
            cx.editor.set_status(msg);
        }
        Err(e) => {
            cx.editor.set_error(format!("Failed to refresh: {}", e));
        }
    }
}

/// Refresh current oil buffer.
pub fn oil_refresh(cx: &mut Context) {
    let doc_id = {
        let (_, doc) = current!(cx.editor);
        doc.id()
    };

    if !cx.editor.is_oil_buffer(doc_id) {
        return; // Not an oil buffer
    }

    match cx.editor.refresh_oil_buffer(doc_id) {
        Ok(_) => {
            cx.editor.set_status("Oil buffer refreshed");
        }
        Err(e) => {
            cx.editor.set_error(format!("Failed to refresh: {}", e));
        }
    }
}
