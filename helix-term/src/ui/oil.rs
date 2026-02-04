//! Oil - A buffer-based file manager UI for Helix
//!
//! This module provides the UI component for the oil file manager,
//! allowing users to edit directories as buffers.

use crate::compositor::{Component, Context, Event, EventResult};
use crate::job::Callback;
use crate::ui::overlay::overlaid;
use helix_core::Position;
use helix_view::graphics::{CursorKind, Rect};
use helix_view::keyboard::{KeyCode, KeyModifiers};
use helix_view::oil::{DirectoryBuffer, FileOperation, OilConfig};
use helix_view::Editor;
use std::path::PathBuf;
use tui::buffer::Buffer as Surface;
use tui::text::{Span, Spans};
use tui::widgets::{Block, Borders, Widget};

/// The main oil directory editor component
pub struct OilEditor {
    /// The directory buffer being edited
    buffer: DirectoryBuffer,
    /// Current cursor line (0-indexed)
    cursor_line: usize,
    /// Scroll offset
    scroll_offset: usize,
    /// Whether to show the help overlay
    show_help: bool,
    /// Whether to show the confirmation dialog
    show_confirmation: bool,
    /// Pending operations for confirmation
    pending_ops: Vec<FileOperation>,
    /// Status message
    status: Option<String>,
    /// Whether the buffer has been modified
    modified: bool,
}

impl OilEditor {
    /// Create a new oil editor for the given directory
    pub fn new(path: PathBuf, config: &OilConfig) -> std::io::Result<Self> {
        let buffer = DirectoryBuffer::new(path, config)?;
        Ok(Self {
            buffer,
            cursor_line: 2, // Start at first entry (after header and ..)
            scroll_offset: 0,
            show_help: false,
            show_confirmation: false,
            pending_ops: Vec::new(),
            status: None,
            modified: false,
        })
    }

    /// Get the directory path
    pub fn path(&self) -> &PathBuf {
        &self.buffer.path
    }

    /// Navigate to parent directory
    fn goto_parent(&mut self, cx: &mut Context) {
        if let Some(parent) = self.buffer.parent_path() {
            let config = cx.editor.config().oil.clone();
            match DirectoryBuffer::new(parent, &config) {
                Ok(new_buffer) => {
                    self.buffer = new_buffer;
                    self.cursor_line = 2;
                    self.scroll_offset = 0;
                    self.modified = false;
                }
                Err(e) => {
                    self.status = Some(format!("Error: {}", e));
                }
            }
        }
    }

    /// Navigate into a directory or open a file
    fn enter_or_open(&mut self, cx: &mut Context) {
        let line = self.cursor_line;

        // Line 1 is ".." - go to parent
        if line == 1 {
            self.goto_parent(cx);
            return;
        }

        // Get the entry at this line
        if let Some(entry) = self.buffer.entry_at_line(line) {
            let entry_path = self.buffer.path.join(&entry.original_name);

            if entry.is_directory {
                // Navigate into directory
                let config = cx.editor.config().oil.clone();
                match DirectoryBuffer::new(entry_path, &config) {
                    Ok(new_buffer) => {
                        self.buffer = new_buffer;
                        self.cursor_line = 2;
                        self.scroll_offset = 0;
                        self.modified = false;
                    }
                    Err(e) => {
                        self.status = Some(format!("Error: {}", e));
                    }
                }
            } else {
                // Open the file - we need to close oil and open the file
                let path = entry_path.clone();
                let callback = Box::pin(async move {
                    let call: Callback =
                        Callback::EditorCompositor(Box::new(move |editor, compositor| {
                            // Pop the oil layer
                            compositor.pop();
                            // Open the file
                            if let Err(e) = editor.open(&path, helix_view::editor::Action::Replace)
                            {
                                editor.set_error(format!("Error opening file: {}", e));
                            }
                        }));
                    Ok(call)
                });
                cx.jobs.callback(callback);
            }
        }
    }

    /// Toggle hidden files
    fn toggle_hidden(&mut self, cx: &mut Context) {
        let config = cx.editor.config().oil.clone();
        if let Err(e) = self.buffer.toggle_hidden(&config) {
            self.status = Some(format!("Error: {}", e));
        }
    }

    /// Cycle sort mode
    fn cycle_sort(&mut self, cx: &mut Context) {
        let config = cx.editor.config().oil.clone();
        if let Err(e) = self.buffer.cycle_sort(&config) {
            self.status = Some(format!("Error: {}", e));
        }
    }

    /// Refresh the directory
    fn refresh(&mut self, cx: &mut Context) {
        let config = cx.editor.config().oil.clone();
        if let Err(e) = self.buffer.refresh(&config) {
            self.status = Some(format!("Error: {}", e));
        } else {
            self.status = Some("Refreshed".to_string());
            self.modified = false;
        }
    }

    /// Apply pending operations
    fn apply_operations(&mut self, cx: &mut Context) {
        if self.pending_ops.is_empty() {
            self.status = Some("No pending changes".to_string());
            return;
        }

        let config = cx.editor.config().oil.clone();

        // Execute operations
        self.buffer.pending_operations = self.pending_ops.clone();
        match self.buffer.execute_operations() {
            Ok(successes) => {
                let count = successes.len();
                self.status = Some(format!("{} operation(s) completed successfully", count));
                // Refresh the buffer
                if let Err(e) = self.buffer.refresh(&config) {
                    self.status = Some(format!("Refresh error: {}", e));
                }
                self.modified = false;
            }
            Err(errors) => {
                self.status = Some(format!("Errors: {}", errors.join(", ")));
            }
        }

        self.pending_ops.clear();
        self.show_confirmation = false;
    }

    /// Parse buffer content and prepare operations
    fn prepare_operations(&mut self, content: &str, cx: &mut Context) {
        let config = cx.editor.config().oil.clone();
        let ops = self.buffer.parse_buffer_content(content, &config);

        if ops.is_empty() {
            self.status = Some("No changes detected".to_string());
            return;
        }

        self.pending_ops = ops;

        // Check if we should skip confirmation
        if config.skip_confirm_simple && self.pending_ops.len() == 1 {
            self.apply_operations(cx);
        } else if config.confirm_changes {
            self.show_confirmation = true;
        } else {
            self.apply_operations(cx);
        }
    }

    /// Move cursor up
    fn cursor_up(&mut self) {
        if self.cursor_line > 0 {
            self.cursor_line = self.cursor_line.saturating_sub(1);
        }
    }

    /// Move cursor down
    fn cursor_down(&mut self, max_lines: usize) {
        if self.cursor_line < max_lines.saturating_sub(1) {
            self.cursor_line += 1;
        }
    }

    /// Get the total number of lines
    fn total_lines(&self) -> usize {
        // Header + ".." + entries
        2 + self.buffer.entries.len()
    }

    /// Render the help overlay
    fn render_help(&self, area: Rect, surface: &mut Surface) {
        let help_text = vec![
            "",
            "  Navigation:",
            "    Enter    Open file/directory",
            "    -        Go to parent directory",
            "    j/k      Move cursor down/up",
            "    g/G      Go to first/last entry",
            "    .",
            "  Operations:",
            "    Edit     Rename file (change the name)",
            "    dd       Delete file",
            "    o        Create new file",
            "    O        Create new directory",
            "    yy       Yank (copy) file",
            "    p        Paste file",
            "",
            "  Commands:",
            "    Ctrl-s   Apply pending changes",
            "    Ctrl-r   Refresh directory",
            "    .        Toggle hidden files",
            "    ?        Toggle this help",
            "    q/Esc    Close oil",
            "",
            "  Press any key to close",
        ];

        let width = 50.min(area.width.saturating_sub(4));
        let height = (help_text.len() as u16 + 2).min(area.height.saturating_sub(4));
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;

        let help_area = Rect::new(x, y, width, height);

        // Clear the area manually
        for row in help_area.y..help_area.y + help_area.height {
            for col in help_area.x..help_area.x + help_area.width {
                if let Some(cell) = surface.get_mut(col, row) {
                    cell.reset();
                }
            }
        }

        let block = Block::default()
            .title(" Oil Mode Help ")
            .borders(Borders::ALL);

        let inner = block.inner(help_area);
        block.render(help_area, surface);

        for (i, line) in help_text.iter().enumerate() {
            if i as u16 >= inner.height {
                break;
            }
            let span = Span::raw(*line);
            surface.set_spans(inner.x, inner.y + i as u16, &Spans::from(span), inner.width);
        }
    }

    /// Render the confirmation dialog
    fn render_confirmation(&self, area: Rect, surface: &mut Surface) {
        let mut lines: Vec<Spans> = vec![
            Spans::from("The following operations will be performed:"),
            Spans::from(""),
        ];

        for op in &self.pending_ops {
            lines.push(Spans::from(format!("  {}", op.description())));
        }

        lines.push(Spans::from(""));
        lines.push(Spans::from("[y]es  [n]o  [q]uit"));

        let width = 60.min(area.width.saturating_sub(4));
        let height = (lines.len() as u16 + 2).min(area.height.saturating_sub(4));
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;

        let confirm_area = Rect::new(x, y, width, height);

        // Clear the area manually
        for row in confirm_area.y..confirm_area.y + confirm_area.height {
            for col in confirm_area.x..confirm_area.x + confirm_area.width {
                if let Some(cell) = surface.get_mut(col, row) {
                    cell.reset();
                }
            }
        }

        let block = Block::default()
            .title(" Pending Changes ")
            .borders(Borders::ALL);

        let inner = block.inner(confirm_area);
        block.render(confirm_area, surface);

        for (i, line) in lines.iter().enumerate() {
            if i as u16 >= inner.height {
                break;
            }
            surface.set_spans(inner.x, inner.y + i as u16, line, inner.width);
        }
    }
}

impl Component for OilEditor {
    fn handle_event(&mut self, event: &Event, cx: &mut Context) -> EventResult {
        match event {
            Event::Key(key) => {
                // Handle confirmation dialog
                if self.show_confirmation {
                    match key.code {
                        KeyCode::Char('y') | KeyCode::Char('Y') => {
                            self.apply_operations(cx);
                            return EventResult::Consumed(None);
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Char('q') | KeyCode::Esc => {
                            self.show_confirmation = false;
                            self.pending_ops.clear();
                            self.status = Some("Cancelled".to_string());
                            return EventResult::Consumed(None);
                        }
                        _ => return EventResult::Consumed(None),
                    }
                }

                // Handle help overlay
                if self.show_help {
                    self.show_help = false;
                    return EventResult::Consumed(None);
                }

                // Handle normal mode keys
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        return EventResult::Consumed(Some(Box::new(|compositor, _cx| {
                            compositor.pop();
                        })));
                    }
                    KeyCode::Char('?') => {
                        self.show_help = true;
                    }
                    KeyCode::Char('j') | KeyCode::Down => {
                        self.cursor_down(self.total_lines());
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        self.cursor_up();
                    }
                    KeyCode::Char('g') => {
                        self.cursor_line = 0;
                    }
                    KeyCode::Char('G') => {
                        self.cursor_line = self.total_lines().saturating_sub(1);
                    }
                    KeyCode::Enter => {
                        self.enter_or_open(cx);
                    }
                    KeyCode::Char('-') => {
                        self.goto_parent(cx);
                    }
                    KeyCode::Char('.') => {
                        self.toggle_hidden(cx);
                    }
                    KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        // Generate buffer content and parse for changes
                        let config = cx.editor.config().oil.clone();
                        let content = self.buffer.to_buffer_content(&config);
                        self.prepare_operations(&content, cx);
                    }
                    KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.refresh(cx);
                    }
                    _ => {}
                }
                EventResult::Consumed(None)
            }
            _ => EventResult::Ignored(None),
        }
    }

    fn render(&mut self, area: Rect, surface: &mut Surface, cx: &mut Context) {
        let _config = cx.editor.config().oil.clone();

        // Calculate visible area
        let header_height = 1;
        let status_height = 1;
        let content_height = area.height.saturating_sub(header_height + status_height);

        // Adjust scroll offset
        if self.cursor_line < self.scroll_offset {
            self.scroll_offset = self.cursor_line;
        } else if self.cursor_line >= self.scroll_offset + content_height as usize {
            self.scroll_offset = self.cursor_line.saturating_sub(content_height as usize - 1);
        }

        // Render header
        let header = format!(
            " {} {}",
            self.buffer.path.display(),
            if self.modified { "[+]" } else { "" }
        );
        let header_style = cx.editor.theme.get("ui.statusline");
        surface.set_string(area.x, area.y, &header, header_style);

        // Fill rest of header line
        for x in area.x + header.len() as u16..area.x + area.width {
            surface.set_string(x, area.y, " ", header_style);
        }

        // Render content
        let content_area = Rect::new(
            area.x,
            area.y + header_height,
            area.width,
            content_height,
        );

        let text_style = cx.editor.theme.get("ui.text");
        let dir_style = cx.editor.theme.get("ui.text.directory");
        let _cursor_style = cx.editor.theme.get("ui.cursor");
        let selection_style = cx.editor.theme.get("ui.selection");

        // Build lines
        let mut lines: Vec<(String, bool)> = Vec::new();

        // Parent directory entry
        lines.push(("..".to_string(), true));

        // Directory entries
        for entry in &self.buffer.entries {
            let name = entry.display_name();
            lines.push((name, entry.is_directory));
        }

        // Render visible lines
        for (i, (line, is_dir)) in lines
            .iter()
            .enumerate()
            .skip(self.scroll_offset)
            .take(content_height as usize)
        {
            let y = content_area.y + (i - self.scroll_offset) as u16;
            let line_num = i + 1; // Line numbers are 1-indexed in our model

            // Determine style
            let style = if line_num == self.cursor_line {
                selection_style
            } else if *is_dir {
                dir_style
            } else {
                text_style
            };

            // Render line number
            let line_num_str = format!("{:3} ", line_num);
            let line_num_style = cx.editor.theme.get("ui.linenr");
            surface.set_string(content_area.x, y, &line_num_str, line_num_style);

            // Render line content
            let content_x = content_area.x + line_num_str.len() as u16;
            surface.set_string(content_x, y, line, style);

            // Fill rest of line for cursor line
            if line_num == self.cursor_line {
                for x in content_x + line.len() as u16..content_area.x + content_area.width {
                    surface.set_string(x, y, " ", selection_style);
                }
            }
        }

        // Render status line
        let status_y = area.y + area.height - 1;
        let status_style = cx.editor.theme.get("ui.statusline");

        let status_text = if let Some(ref status) = self.status {
            status.clone()
        } else {
            let pending_count = self.buffer.pending_operations.len();
            if pending_count > 0 {
                format!(
                    " {} | {} pending changes | Ctrl-s to apply",
                    if self.buffer.show_hidden {
                        "hidden: on"
                    } else {
                        "hidden: off"
                    },
                    pending_count
                )
            } else {
                format!(
                    " {} | Press ? for help",
                    if self.buffer.show_hidden {
                        "hidden: on"
                    } else {
                        "hidden: off"
                    }
                )
            }
        };

        surface.set_string(area.x, status_y, &status_text, status_style);
        for x in area.x + status_text.len() as u16..area.x + area.width {
            surface.set_string(x, status_y, " ", status_style);
        }

        // Render overlays
        if self.show_help {
            self.render_help(area, surface);
        }

        if self.show_confirmation {
            self.render_confirmation(area, surface);
        }
    }

    fn cursor(&self, _area: Rect, _cx: &Editor) -> (Option<Position>, CursorKind) {
        (None, CursorKind::Hidden)
    }
}

/// Open the oil file manager at the given path
pub fn oil_open(path: PathBuf, cx: &mut crate::commands::Context) {
    let config = cx.editor.config().oil.clone();
    match OilEditor::new(path, &config) {
        Ok(editor) => {
            cx.push_layer(Box::new(overlaid(editor)));
        }
        Err(e) => {
            cx.editor.set_error(format!("Error opening directory: {}", e));
        }
    }
}

/// Open the oil file manager at the current file's directory
pub fn oil_current(cx: &mut crate::commands::Context) {
    let path = helix_view::doc!(cx.editor)
        .path()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| helix_stdx::env::current_working_dir());

    oil_open(path, cx);
}

/// Open the oil file manager at the workspace root
pub fn oil_workspace(cx: &mut crate::commands::Context) {
    let (root, _) = helix_core::find_workspace();
    oil_open(root, cx);
}
