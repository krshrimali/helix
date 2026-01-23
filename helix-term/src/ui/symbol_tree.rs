use helix_core::{Position, Uri};
use helix_lsp::{lsp, OffsetEncoding};
use helix_view::{
    editor::Action,
    graphics::{CursorKind, Rect},
    DocumentId, Editor,
};

use crate::{
    compositor::{self, Component, Context, EventResult},
    ctrl, key,
};

use tui::buffer::Buffer as Surface;
use tui::widgets::Widget;

pub const ID: &str = "symbol-tree";

/// A node in the symbol tree hierarchy
#[derive(Debug, Clone)]
pub struct SymbolTreeNode {
    /// The symbol name
    pub name: String,
    /// Symbol kind (function, class, method, etc.)
    pub kind: lsp::SymbolKind,
    /// The range enclosing this symbol
    pub range: lsp::Range,
    /// The range that should be selected when navigating to this symbol
    pub selection_range: lsp::Range,
    /// URI of the document containing this symbol
    pub uri: Uri,
    /// Offset encoding from the language server
    pub offset_encoding: OffsetEncoding,
    /// Children symbols (nested classes, methods, etc.)
    pub children: Vec<SymbolTreeNode>,
    /// Whether this node is expanded in the UI
    pub expanded: bool,
}

impl SymbolTreeNode {
    /// Create a new symbol tree node from an LSP DocumentSymbol
    pub fn from_document_symbol(
        symbol: lsp::DocumentSymbol,
        uri: Uri,
        offset_encoding: OffsetEncoding,
    ) -> Self {
        let children = symbol
            .children
            .unwrap_or_default()
            .into_iter()
            .map(|child| Self::from_document_symbol(child, uri.clone(), offset_encoding))
            .collect();

        Self {
            name: symbol.name,
            kind: symbol.kind,
            range: symbol.range,
            selection_range: symbol.selection_range,
            uri,
            offset_encoding,
            children,
            expanded: true, // Expand all nodes by default
        }
    }

    /// Check if this node has children
    pub fn has_children(&self) -> bool {
        !self.children.is_empty()
    }
}

/// A flattened representation of a tree node for rendering
#[derive(Debug)]
struct FlattenedNode {
    /// Index path to this node in the tree (e.g., [0, 2, 1] means roots[0].children[2].children[1])
    path: Vec<usize>,
    /// Depth level in the tree (for indentation)
    depth: usize,
    /// Whether this node has children
    has_children: bool,
    /// Whether this node is expanded
    is_expanded: bool,
    /// The symbol name
    name: String,
    /// Symbol kind
    kind: lsp::SymbolKind,
    /// URI for navigation
    uri: Uri,
    /// Full range enclosing this symbol (for containment checks)
    range: lsp::Range,
    /// Selection range for navigation
    selection_range: lsp::Range,
    /// Offset encoding
    offset_encoding: OffsetEncoding,
}

/// Symbol tree view component
pub struct SymbolTreeView {
    /// Root-level symbols
    roots: Vec<SymbolTreeNode>,
    /// Currently selected index in the flattened view
    cursor: usize,
    /// Document ID this tree corresponds to (reserved for future use)
    #[allow(dead_code)]
    doc_id: DocumentId,
    /// Cached flattened nodes for rendering
    flattened: Vec<FlattenedNode>,
    /// Scroll offset for tall trees
    scroll_offset: usize,
    /// Width of the tree view
    width: u16,
    /// Whether to render on the right (true) or bottom (false)
    render_right: bool,
}

impl SymbolTreeView {
    /// Create a new symbol tree view
    pub fn new(roots: Vec<SymbolTreeNode>, doc_id: DocumentId, render_right: bool) -> Self {
        Self::new_at_position(roots, doc_id, render_right, None)
    }

    /// Create a new symbol tree view focused on the symbol at the given cursor line
    pub fn new_at_position(
        roots: Vec<SymbolTreeNode>,
        doc_id: DocumentId,
        render_right: bool,
        cursor_line: Option<u32>,
    ) -> Self {
        let mut view = Self {
            roots,
            cursor: 0,
            doc_id,
            flattened: Vec::new(),
            scroll_offset: 0,
            width: 30, // Default width
            render_right,
        };
        view.rebuild_flattened();

        // If cursor position provided, find and focus on the containing symbol
        if let Some(line) = cursor_line {
            if let Some(idx) = view.find_deepest_symbol_at_line(line) {
                view.cursor = idx;
            }
        }

        view
    }

    /// Find the deepest (most specific) symbol that contains the given line
    fn find_deepest_symbol_at_line(&self, line: u32) -> Option<usize> {
        let mut best_match: Option<(usize, u32)> = None; // (index, range_size)

        for (idx, node) in self.flattened.iter().enumerate() {
            // Use the full range for containment check (not selection_range)
            let start = node.range.start.line;
            let end = node.range.end.line;

            // Check if line is within this symbol's range
            if line >= start && line <= end {
                let range_size = end - start;
                // Prefer smaller (more specific) ranges - deeper nesting
                match best_match {
                    None => best_match = Some((idx, range_size)),
                    Some((_, prev_size)) if range_size <= prev_size => {
                        best_match = Some((idx, range_size));
                    }
                    _ => {}
                }
            }
        }

        best_match.map(|(idx, _)| idx)
    }

    /// Rebuild the flattened view from the tree structure
    fn rebuild_flattened(&mut self) {
        self.flattened.clear();
        // Collect nodes to flatten first to avoid borrow issues
        let roots: Vec<_> = self.roots.iter().cloned().collect();
        for (i, root) in roots.iter().enumerate() {
            Self::flatten_node_recursive(&mut self.flattened, root, vec![i], 0);
        }
        // Clamp cursor to valid range
        if !self.flattened.is_empty() && self.cursor >= self.flattened.len() {
            self.cursor = self.flattened.len() - 1;
        }
    }

    /// Recursively flatten a node and its expanded children
    fn flatten_node_recursive(
        flattened: &mut Vec<FlattenedNode>,
        node: &SymbolTreeNode,
        path: Vec<usize>,
        depth: usize,
    ) {
        flattened.push(FlattenedNode {
            path: path.clone(),
            depth,
            has_children: node.has_children(),
            is_expanded: node.expanded,
            name: node.name.clone(),
            kind: node.kind,
            uri: node.uri.clone(),
            range: node.range,
            selection_range: node.selection_range,
            offset_encoding: node.offset_encoding,
        });

        if node.expanded {
            for (i, child) in node.children.iter().enumerate() {
                let mut child_path = path.clone();
                child_path.push(i);
                Self::flatten_node_recursive(flattened, child, child_path, depth + 1);
            }
        }
    }

    /// Get a mutable reference to a node by its path
    fn get_node_mut(&mut self, path: &[usize]) -> Option<&mut SymbolTreeNode> {
        if path.is_empty() {
            return None;
        }

        let mut node = self.roots.get_mut(path[0])?;
        for &idx in &path[1..] {
            node = node.children.get_mut(idx)?;
        }
        Some(node)
    }

    /// Move cursor up
    fn move_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.ensure_cursor_visible();
        }
    }

    /// Move cursor down
    fn move_down(&mut self) {
        if self.cursor + 1 < self.flattened.len() {
            self.cursor += 1;
            self.ensure_cursor_visible();
        }
    }

    /// Ensure cursor is visible in the viewport
    fn ensure_cursor_visible(&mut self) {
        // This will be called during render when we know the height
    }

    /// Toggle expand/collapse for the current node
    fn toggle_current(&mut self) {
        if let Some(flat_node) = self.flattened.get(self.cursor) {
            if flat_node.has_children {
                let path = flat_node.path.clone();
                if let Some(node) = self.get_node_mut(&path) {
                    node.expanded = !node.expanded;
                    self.rebuild_flattened();
                }
            }
        }
    }

    /// Expand the current node
    fn expand_current(&mut self) {
        if let Some(flat_node) = self.flattened.get(self.cursor) {
            if flat_node.has_children && !flat_node.is_expanded {
                let path = flat_node.path.clone();
                if let Some(node) = self.get_node_mut(&path) {
                    node.expanded = true;
                    self.rebuild_flattened();
                }
            } else if flat_node.has_children && flat_node.is_expanded {
                // Already expanded, move to first child
                self.move_down();
            }
        }
    }

    /// Collapse the current node or move to parent
    fn collapse_or_parent(&mut self) {
        if let Some(flat_node) = self.flattened.get(self.cursor) {
            if flat_node.has_children && flat_node.is_expanded {
                // Collapse this node
                let path = flat_node.path.clone();
                if let Some(node) = self.get_node_mut(&path) {
                    node.expanded = false;
                    self.rebuild_flattened();
                }
            } else if flat_node.path.len() > 1 {
                // Move to parent
                let parent_path: Vec<usize> =
                    flat_node.path[..flat_node.path.len() - 1].to_vec();
                // Find the parent in the flattened list
                for (i, node) in self.flattened.iter().enumerate() {
                    if node.path == parent_path {
                        self.cursor = i;
                        self.ensure_cursor_visible();
                        break;
                    }
                }
            }
        }
    }

    /// Expand all nodes (reserved for zR keybinding)
    #[allow(dead_code)]
    fn expand_all(&mut self) {
        fn expand_recursive(node: &mut SymbolTreeNode) {
            node.expanded = true;
            for child in &mut node.children {
                expand_recursive(child);
            }
        }

        for root in &mut self.roots {
            expand_recursive(root);
        }
        self.rebuild_flattened();
    }

    /// Collapse all nodes (reserved for zM keybinding)
    #[allow(dead_code)]
    fn collapse_all(&mut self) {
        fn collapse_recursive(node: &mut SymbolTreeNode) {
            node.expanded = false;
            for child in &mut node.children {
                collapse_recursive(child);
            }
        }

        for root in &mut self.roots {
            collapse_recursive(root);
        }
        self.rebuild_flattened();
    }

    /// Get the currently selected node's location for navigation
    fn current_location(&self) -> Option<(Uri, lsp::Range, OffsetEncoding)> {
        self.flattened.get(self.cursor).map(|node| {
            (
                node.uri.clone(),
                node.selection_range,
                node.offset_encoding,
            )
        })
    }

    /// Convert symbol kind to a short display string
    fn symbol_kind_str(kind: lsp::SymbolKind) -> &'static str {
        match kind {
            lsp::SymbolKind::FILE => "fil",
            lsp::SymbolKind::MODULE => "mod",
            lsp::SymbolKind::NAMESPACE => "ns",
            lsp::SymbolKind::PACKAGE => "pkg",
            lsp::SymbolKind::CLASS => "cls",
            lsp::SymbolKind::METHOD => "mth",
            lsp::SymbolKind::PROPERTY => "prp",
            lsp::SymbolKind::FIELD => "fld",
            lsp::SymbolKind::CONSTRUCTOR => "con",
            lsp::SymbolKind::ENUM => "enm",
            lsp::SymbolKind::INTERFACE => "ifc",
            lsp::SymbolKind::FUNCTION => "fn",
            lsp::SymbolKind::VARIABLE => "var",
            lsp::SymbolKind::CONSTANT => "cst",
            lsp::SymbolKind::STRING => "str",
            lsp::SymbolKind::NUMBER => "num",
            lsp::SymbolKind::BOOLEAN => "bol",
            lsp::SymbolKind::ARRAY => "arr",
            lsp::SymbolKind::OBJECT => "obj",
            lsp::SymbolKind::KEY => "key",
            lsp::SymbolKind::NULL => "nul",
            lsp::SymbolKind::ENUM_MEMBER => "enm",
            lsp::SymbolKind::STRUCT => "stc",
            lsp::SymbolKind::EVENT => "evt",
            lsp::SymbolKind::OPERATOR => "opr",
            lsp::SymbolKind::TYPE_PARAMETER => "typ",
            _ => "???",
        }
    }
}

impl Component for SymbolTreeView {
    fn render(&mut self, area: Rect, surface: &mut Surface, cx: &mut Context) {
        use tui::widgets::{Block, BorderType, Borders};

        let theme = &cx.editor.theme;
        let text_style = theme.get("ui.text");
        let selected_style = theme.get("ui.menu.selected");

        // Calculate the tree area - either right side or bottom
        let tree_area = if self.render_right {
            // Right split - use a portion of the right side
            let tree_width = self.width.min(area.width / 3).max(20);
            Rect::new(
                area.x + area.width - tree_width,
                area.y,
                tree_width,
                area.height,
            )
        } else {
            // Bottom split - use a portion of the bottom
            let tree_height = (area.height / 3).max(5);
            Rect::new(
                area.x,
                area.y + area.height - tree_height,
                area.width,
                tree_height,
            )
        };

        // Draw border
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title("Symbols");

        let inner = block.inner(tree_area);
        block.render(tree_area, surface);

        // Adjust scroll offset to keep cursor visible
        let visible_height = inner.height as usize;
        if self.cursor < self.scroll_offset {
            self.scroll_offset = self.cursor;
        } else if self.cursor >= self.scroll_offset + visible_height {
            self.scroll_offset = self.cursor - visible_height + 1;
        }

        // Render tree nodes
        for (i, node) in self
            .flattened
            .iter()
            .skip(self.scroll_offset)
            .take(visible_height)
            .enumerate()
        {
            let y = inner.y + i as u16;
            if y >= inner.y + inner.height {
                break;
            }

            let is_selected = self.scroll_offset + i == self.cursor;
            let style = if is_selected {
                selected_style
            } else {
                text_style
            };

            // Calculate indentation
            let indent = "  ".repeat(node.depth);

            // Expand/collapse indicator
            let indicator = if node.has_children {
                if node.is_expanded {
                    "▼ "
                } else {
                    "▶ "
                }
            } else {
                "  "
            };

            // Symbol kind
            let kind_str = Self::symbol_kind_str(node.kind);

            // Build the display line
            let line = format!("{}{}{} {}", indent, indicator, kind_str, node.name);

            // Truncate to fit width
            let max_width = inner.width as usize;
            let display_line: String = line.chars().take(max_width).collect();

            // Render the line
            surface.set_stringn(
                inner.x,
                y,
                &display_line,
                max_width,
                style,
            );

            // Fill remaining width with background
            let remaining = max_width.saturating_sub(display_line.chars().count());
            if remaining > 0 && is_selected {
                for x in (inner.x + display_line.chars().count() as u16)..inner.x + inner.width {
                    if let Some(cell) = surface.get_mut(x, y) {
                        cell.set_style(style);
                    }
                }
            }
        }
    }

    fn handle_event(&mut self, event: &crate::compositor::Event, cx: &mut Context) -> EventResult {
        use crate::compositor::Event;

        let key_event = match event {
            Event::Key(event) => *event,
            Event::Resize(..) => return EventResult::Consumed(None),
            _ => return EventResult::Ignored(None),
        };

        match key_event {
            // Navigation
            key!('j') | key!(Down) => {
                self.move_down();
                EventResult::Consumed(None)
            }
            key!('k') | key!(Up) => {
                self.move_up();
                EventResult::Consumed(None)
            }
            key!('h') | key!(Left) => {
                self.collapse_or_parent();
                EventResult::Consumed(None)
            }
            key!('l') | key!(Right) => {
                self.expand_current();
                EventResult::Consumed(None)
            }

            // Expand/collapse
            key!(Tab) | key!(Enter) if self.flattened.get(self.cursor).map_or(false, |n| n.has_children) => {
                // If on a node with children and pressing Tab or Enter, toggle expand
                // But only if we're pressing Tab, Enter should navigate
                if matches!(key_event, key!(Tab)) {
                    self.toggle_current();
                    return EventResult::Consumed(None);
                }
                // Fall through to navigation for Enter
                self.jump_to_current(cx)
            }

            // Navigation - jump to symbol
            key!(Enter) => self.jump_to_current(cx),

            // Vim-style fold commands
            key!('z') => {
                // Wait for next key
                EventResult::Consumed(None)
            }

            // Close
            key!('q') | key!(Esc) => {
                let callback: compositor::Callback = Box::new(|compositor, _cx| {
                    compositor.remove(ID);
                });
                EventResult::Consumed(Some(callback))
            }

            // Collapse all
            ctrl!('c') if false => {
                // Placeholder - we'll use zM for collapse all
                EventResult::Consumed(None)
            }

            _ => EventResult::Ignored(None),
        }
    }

    fn cursor(&self, _area: Rect, _editor: &Editor) -> (Option<Position>, CursorKind) {
        // Don't show cursor in the tree view
        (None, CursorKind::Hidden)
    }

    fn required_size(&mut self, viewport: (u16, u16)) -> Option<(u16, u16)> {
        // Request a portion of the screen
        if self.render_right {
            Some((self.width.min(viewport.0 / 3), viewport.1))
        } else {
            Some((viewport.0, viewport.1 / 3))
        }
    }

    fn id(&self) -> Option<&'static str> {
        Some(ID)
    }
}

impl SymbolTreeView {
    /// Jump to the currently selected symbol
    fn jump_to_current(&mut self, cx: &mut Context) -> EventResult {
        use helix_lsp::util::lsp_range_to_range;
        use helix_view::current;

        if let Some((uri, range, offset_encoding)) = self.current_location() {
            let Some(path) = uri.as_path() else {
                cx.editor.set_error(format!("Unable to convert URI to path: {:?}", uri));
                return EventResult::Consumed(None);
            };

            // Open the document
            match cx.editor.open(path, Action::Replace) {
                Ok(_doc_id) => {
                    let scrolloff = cx.editor.config().scrolloff;

                    // Get current view and document
                    let (view, doc) = current!(cx.editor);

                    // Convert LSP range to helix range
                    if let Some(new_range) =
                        lsp_range_to_range(doc.text(), range, offset_encoding)
                    {
                        let selection = helix_core::Selection::single(
                            new_range.anchor,
                            new_range.head,
                        );
                        doc.set_selection(view.id, selection);
                    }

                    // Center the view on the selection
                    view.ensure_cursor_in_view(doc, scrolloff);
                }
                Err(err) => {
                    cx.editor.set_error(format!("Failed to open file: {}", err));
                }
            }
        }
        EventResult::Consumed(None)
    }
}

/// Build a symbol tree from LSP DocumentSymbolResponse
pub fn build_symbol_tree(
    response: lsp::DocumentSymbolResponse,
    uri: Uri,
    offset_encoding: OffsetEncoding,
) -> Vec<SymbolTreeNode> {
    match response {
        lsp::DocumentSymbolResponse::Nested(symbols) => symbols
            .into_iter()
            .map(|s| SymbolTreeNode::from_document_symbol(s, uri.clone(), offset_encoding))
            .collect(),
        lsp::DocumentSymbolResponse::Flat(symbols) => {
            // For flat symbols, create a flat tree (no hierarchy)
            symbols
                .into_iter()
                .map(|s| SymbolTreeNode {
                    name: s.name,
                    kind: s.kind,
                    range: s.location.range,
                    selection_range: s.location.range,
                    uri: uri.clone(),
                    offset_encoding,
                    children: Vec::new(),
                    expanded: true,
                })
                .collect()
        }
    }
}
