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

| Feature | Description | Keyboard Shortcuts | Mouse Support | Configuration |
|---------|-------------|-------------------|---------------|---------------|
| **Picker Preview Scrolling** | Scroll through file previews in pickers without changing selection | `Alt+Up`/`Alt+Down` or `Ctrl+y`/`Ctrl+e` | Scroll wheel over preview panel | `picker-preview-scroll = true` (default) |
| **Customizable Picker Keybindings** | Fully customizable keybindings for all picker actions | See picker keybindings section below | N/A | Configure via `[picker-keys]` in config.toml |

### Picker Keybindings

The picker now supports fully customizable keybindings! You can customize any picker action in your `config.toml`:

```toml
[picker-keys]
# Navigation
up = "move_prev"
down = "move_next"
"C-n" = "move_next"
"C-p" = "move_prev"
"C-d" = "page_down"
"C-u" = "page_up"
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
