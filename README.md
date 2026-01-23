<div align="center">

<h1>
<picture>
  <source media="(prefers-color-scheme: dark)" srcset="logo_dark.svg">
  <source media="(prefers-color-scheme: light)" srcset="logo_light.svg">
  <img alt="Helix" height="128" src="logo_light.svg">
</picture>
</h1>

[![Build status](https://github.com/helix-editor/helix/actions/workflows/build.yml/badge.svg)](https://github.com/helix-editor/helix/actions)
[![GitHub Release](https://img.shields.io/github/v/release/helix-editor/helix)](https://github.com/helix-editor/helix/releases/latest)
[![Documentation](https://shields.io/badge/-documentation-452859)](https://docs.helix-editor.com/)
[![GitHub contributors](https://img.shields.io/github/contributors/helix-editor/helix)](https://github.com/helix-editor/helix/graphs/contributors)
[![Matrix Space](https://img.shields.io/matrix/helix-community:matrix.org)](https://matrix.to/#/#helix-community:matrix.org)

</div>

## Custom Features Added

### Feature Summary

| Feature | Description | Default Keybindings | Configuration |
|---------|-------------|---------------------|---------------|
| **Picker Preview Scrolling** | Scroll file previews in pickers | `Alt+Up`/`Alt+Down`, `Ctrl+y`/`Ctrl+e`, or mouse scroll | `picker_preview_scroll = true` |
| **Quickfix List** | Vim-like quickfix for navigating locations | `Space x x` (open), `]q`/`[q` (next/prev) | N/A |
| **Send to Quickfix** | Send picker results to quickfix list | `Alt+q` in any picker | `[picker_keys]` section |
| **Hover Documentation Split** | Open docs in a split instead of popup | `Space h s` (hsplit), `Space h v` (vsplit) | N/A |
| **Symbol Tree** | LSP symbol outline in a side panel | `Space l o` (vsplit), `Space l O` (hsplit) | N/A |
| **Breadcrumbs** | Show symbol hierarchy at cursor | `Space l b` | N/A |
| **Customizable Picker Keys** | Fully customizable picker keybindings | See table below | `[picker_keys]` section |

---

### Quickfix List

A vim-like quickfix list for storing and navigating file locations (e.g., search results, diagnostics).

| Keybinding | Action | Description |
|------------|--------|-------------|
| `Space x x` | `quickfix_picker` | Open quickfix list picker |
| `Space x s` | `quickfix_picker_hsplit` | Open quickfix in horizontal split |
| `Space x v` | `quickfix_picker_vsplit` | Open quickfix in vertical split |
| `]q` | `quickfix_next` | Jump to next quickfix item |
| `[q` | `quickfix_prev` | Jump to previous quickfix item |
| `]Q` | `quickfix_last` | Jump to last quickfix item |
| `[Q` | `quickfix_first` | Jump to first quickfix item |
| `Space x c` | `quickfix_clear` | Clear quickfix list |
| `Space x .` | `quickfix_first` | Jump to first item |
| `Space x ,` | `quickfix_last` | Jump to last item |
| `Enter` | `quickfix_jump_to_location` | Jump to location (in quickfix buffer) |

**Sending to Quickfix:** In any picker, press `Alt+q` to send all filtered results to the quickfix list.

---

### Hover Documentation Split

Open LSP hover documentation in a split window instead of a popup.

| Keybinding | Action | Description |
|------------|--------|-------------|
| `Space h s` | `hover_hsplit` | Open documentation in horizontal split |
| `Space h v` | `hover_vsplit` | Open documentation in vertical split |
| `Space h h` | `select_references_to_symbol_under_cursor` | Select all references to symbol |

---

### Symbol Tree & Breadcrumbs

Navigate code structure using LSP document symbols.

| Keybinding | Action | Description |
|------------|--------|-------------|
| `Space l o` | `symbol_tree` | Open symbol tree in vertical split |
| `Space l O` | `symbol_tree_hsplit` | Open symbol tree in horizontal split |
| `Space l B` | `breadcrumbs` | Show breadcrumbs (symbol hierarchy) picker |

---

### Picker Keybindings

Customize picker keybindings in your `config.toml`:

```toml
[picker_keys]
# Navigation
up = "move_prev"
down = "move_next"
"C-p" = "move_prev"
"C-n" = "move_next"
"C-u" = "page_up"
"C-d" = "page_down"
home = "move_to_start"
end = "move_to_end"

# Preview scrolling
"A-up" = "scroll_preview_up"
"A-down" = "scroll_preview_down"
"C-y" = "scroll_preview_up"
"C-e" = "scroll_preview_down"

# Actions
"C-t" = "toggle_preview"
esc = "close"
"C-c" = "close"
ret = "select"
"A-ret" = "select_alternate"
"C-s" = "select_horizontal_split"
"C-v" = "select_vertical_split"
"A-q" = "send_to_quickfix"
```

#### Available Picker Actions

| Action | Description |
|--------|-------------|
| `move_prev` | Move to previous entry |
| `move_next` | Move to next entry |
| `page_up` | Move one page up |
| `page_down` | Move one page down |
| `move_to_start` | Move to first entry |
| `move_to_end` | Move to last entry |
| `scroll_preview_up` | Scroll preview panel up |
| `scroll_preview_down` | Scroll preview panel down |
| `toggle_preview` | Toggle preview visibility |
| `close` | Close the picker |
| `select` | Select current entry |
| `select_alternate` | Select with alternate action |
| `select_horizontal_split` | Open in horizontal split |
| `select_vertical_split` | Open in vertical split |
| `send_to_quickfix` | Send filtered items to quickfix list |

---

### Editor Configuration

Add to `[editor]` section in `config.toml`:

```toml
[editor]
# Enable mouse scrolling in picker preview panels (default: true)
picker_preview_scroll = true
```

![Screenshot](./screenshot.png)

A [Kakoune](https://github.com/mawww/kakoune) / [Neovim](https://github.com/neovim/neovim) inspired editor, written in Rust.

The editing model is very heavily based on Kakoune; during development I found
myself agreeing with most of Kakoune's design decisions.

For more information, see the [website](https://helix-editor.com) or
[documentation](https://docs.helix-editor.com/).

All shortcuts/keymaps can be found [in the documentation on the website](https://docs.helix-editor.com/keymap.html).

[Troubleshooting](https://github.com/helix-editor/helix/wiki/Troubleshooting)

# Features

- Vim-like modal editing
- Multiple selections
- Built-in language server support
- Smart, incremental syntax highlighting and code editing via tree-sitter

Although it's primarily a terminal-based editor, I am interested in exploring
a custom renderer (similar to Emacs) using wgpu.

Note: Only certain languages have indentation definitions at the moment. Check
`runtime/queries/<lang>/` for `indents.scm`.

# Installation

[Installation documentation](https://docs.helix-editor.com/install.html).

[![Packaging status](https://repology.org/badge/vertical-allrepos/helix-editor.svg?exclude_unsupported=1)](https://repology.org/project/helix-editor/versions)

# Contributing

Contributing guidelines can be found [here](./docs/CONTRIBUTING.md).

# Getting help

Your question might already be answered on the [FAQ](https://github.com/helix-editor/helix/wiki/FAQ).

Discuss the project on the community [Matrix Space](https://matrix.to/#/#helix-community:matrix.org) (make sure to join `#helix-editor:matrix.org` if you're on a client that doesn't support Matrix Spaces yet).

# Credits

Thanks to [@jakenvac](https://github.com/jakenvac) for designing the logo!
