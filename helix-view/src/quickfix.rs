//! Quickfix list implementation for Helix editor.
//!
//! The quickfix list stores a collection of file locations that can be navigated
//! and searched. It's similar to Vim's quickfix list and is populated from picker
//! results, search results, or other sources.

use helix_core::Uri;

/// The name for the quickfix buffer.
pub const QUICKFIX_BUFFER_NAME: &str = "[quickfix]";

/// A single item in the quickfix list representing a location in a file.
#[derive(Debug, Clone)]
pub struct QuickfixItem {
    /// The URI of the file containing this location.
    pub uri: Uri,
    /// The line number (0-indexed).
    pub line: usize,
    /// The column number (0-indexed).
    pub col: usize,
    /// Optional end position for range highlighting.
    pub end_line: Option<usize>,
    pub end_col: Option<usize>,
    /// A text description or preview of this location.
    pub text: String,
    /// Optional type/kind label (e.g., "error", "warning", "match").
    pub kind: Option<String>,
}

impl QuickfixItem {
    /// Create a new quickfix item.
    pub fn new(uri: Uri, line: usize, col: usize, text: String) -> Self {
        Self {
            uri,
            line,
            col,
            end_line: None,
            end_col: None,
            text,
            kind: None,
        }
    }

    /// Create a quickfix item with a range.
    pub fn with_range(
        uri: Uri,
        line: usize,
        col: usize,
        end_line: usize,
        end_col: usize,
        text: String,
    ) -> Self {
        Self {
            uri,
            line,
            col,
            end_line: Some(end_line),
            end_col: Some(end_col),
            text,
            kind: None,
        }
    }

    /// Set the kind/type of this quickfix item.
    pub fn with_kind(mut self, kind: impl Into<String>) -> Self {
        self.kind = Some(kind.into());
        self
    }

    /// Get the file path if the URI represents a file.
    pub fn path(&self) -> Option<&std::path::Path> {
        self.uri.as_path()
    }

    /// Get the display path (relative if possible).
    pub fn display_path(&self) -> String {
        self.path()
            .map(|p| helix_stdx::path::get_relative_path(p).to_string_lossy().into_owned())
            .unwrap_or_else(|| self.uri.to_string())
    }

    /// Get a formatted location string like "path:line:col".
    pub fn location_string(&self) -> String {
        format!("{}:{}:{}", self.display_path(), self.line + 1, self.col + 1)
    }
}

/// The quickfix list containing all quickfix items and navigation state.
#[derive(Debug, Clone, Default)]
pub struct QuickfixList {
    /// The items in the quickfix list.
    items: Vec<QuickfixItem>,
    /// Current position in the list (0-indexed).
    current: usize,
    /// Optional title/label for this quickfix list.
    title: Option<String>,
}

impl QuickfixList {
    /// Create a new empty quickfix list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a quickfix list with items.
    pub fn with_items(items: Vec<QuickfixItem>) -> Self {
        Self {
            items,
            current: 0,
            title: None,
        }
    }

    /// Set the title of this quickfix list.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Get the title of this quickfix list.
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// Set the title of this quickfix list.
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = Some(title.into());
    }

    /// Clear the quickfix list.
    pub fn clear(&mut self) {
        self.items.clear();
        self.current = 0;
        self.title = None;
    }

    /// Add an item to the quickfix list.
    pub fn push(&mut self, item: QuickfixItem) {
        self.items.push(item);
    }

    /// Replace all items in the quickfix list.
    pub fn set_items(&mut self, items: Vec<QuickfixItem>) {
        self.items = items;
        self.current = 0;
    }

    /// Get the number of items in the quickfix list.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Check if the quickfix list is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Get the current item.
    pub fn current(&self) -> Option<&QuickfixItem> {
        self.items.get(self.current)
    }

    /// Get the current index (0-indexed).
    pub fn current_index(&self) -> usize {
        self.current
    }

    /// Set the current index.
    pub fn set_current(&mut self, index: usize) {
        if !self.items.is_empty() {
            self.current = index.min(self.items.len() - 1);
        }
    }

    /// Move to the next item and return it.
    pub fn next(&mut self) -> Option<&QuickfixItem> {
        if self.items.is_empty() {
            return None;
        }
        self.current = (self.current + 1) % self.items.len();
        self.items.get(self.current)
    }

    /// Move to the previous item and return it.
    pub fn prev(&mut self) -> Option<&QuickfixItem> {
        if self.items.is_empty() {
            return None;
        }
        self.current = if self.current == 0 {
            self.items.len() - 1
        } else {
            self.current - 1
        };
        self.items.get(self.current)
    }

    /// Move forward by count items.
    pub fn forward(&mut self, count: usize) -> Option<&QuickfixItem> {
        if self.items.is_empty() {
            return None;
        }
        self.current = (self.current + count) % self.items.len();
        self.items.get(self.current)
    }

    /// Move backward by count items.
    pub fn backward(&mut self, count: usize) -> Option<&QuickfixItem> {
        if self.items.is_empty() {
            return None;
        }
        let len = self.items.len();
        self.current = (self.current + len - (count % len)) % len;
        self.items.get(self.current)
    }

    /// Go to the first item.
    pub fn first(&mut self) -> Option<&QuickfixItem> {
        if self.items.is_empty() {
            return None;
        }
        self.current = 0;
        self.items.get(self.current)
    }

    /// Go to the last item.
    pub fn last(&mut self) -> Option<&QuickfixItem> {
        if self.items.is_empty() {
            return None;
        }
        self.current = self.items.len() - 1;
        self.items.get(self.current)
    }

    /// Get an iterator over all items.
    pub fn iter(&self) -> impl Iterator<Item = &QuickfixItem> {
        self.items.iter()
    }

    /// Get all items as a slice.
    pub fn items(&self) -> &[QuickfixItem] {
        &self.items
    }

    /// Get a status string like "[1/10]".
    pub fn status(&self) -> String {
        if self.items.is_empty() {
            "[empty]".to_string()
        } else {
            format!("[{}/{}]", self.current + 1, self.items.len())
        }
    }

    /// Generate buffer content for the quickfix list.
    /// Each line format: "path:line:col | text" (optionally with line numbers prefix)
    /// Returns the content string.
    pub fn to_buffer_content(&self, show_line_numbers: bool) -> String {
        let mut content = String::new();

        // Add header with title if present
        if let Some(title) = &self.title {
            content.push_str(&format!("# Quickfix: {} ({} items)\n", title, self.items.len()));
        } else {
            content.push_str(&format!("# Quickfix ({} items)\n", self.items.len()));
        }
        content.push_str("# Press Enter on a line to jump to that location\n");
        content.push('\n');

        for (idx, item) in self.items.iter().enumerate() {
            let kind_str = item.kind.as_deref().map(|k| format!("[{}] ", k)).unwrap_or_default();
            if show_line_numbers {
                content.push_str(&format!(
                    "{:>4} | {}:{}:{} | {}{}\n",
                    idx + 1,
                    item.display_path(),
                    item.line + 1,
                    item.col + 1,
                    kind_str,
                    item.text.lines().next().unwrap_or(&item.text)
                ));
            } else {
                content.push_str(&format!(
                    "{}:{}:{} | {}{}\n",
                    item.display_path(),
                    item.line + 1,
                    item.col + 1,
                    kind_str,
                    item.text.lines().next().unwrap_or(&item.text)
                ));
            }
        }

        content
    }

    /// Parse a line number from the quickfix buffer to get the item index.
    /// Lines 1-3 are header, items start at line 4 (0-indexed: line 3).
    /// Returns None if the line is not a valid item line.
    pub fn line_to_item_index(&self, line: usize) -> Option<usize> {
        // Header is 3 lines (title, help text, blank line)
        if line < 3 {
            return None;
        }
        let idx = line - 3;
        if idx < self.items.len() {
            Some(idx)
        } else {
            None
        }
    }

    /// Get an item by buffer line number.
    pub fn item_at_line(&self, line: usize) -> Option<&QuickfixItem> {
        self.line_to_item_index(line).and_then(|idx| self.items.get(idx))
    }
}
