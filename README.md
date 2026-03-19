# Heats

[日本語](README.ja.md)

A rofi-like extensible fuzzy launcher for macOS, built with Rust + [iced](https://iced.rs).

## Features

- Global hotkey toggle (default: `Cmd+;`)
- Fast fuzzy matching powered by [nucleo](https://github.com/helix-editor/nucleo)
- Keyboard-driven: arrow keys to navigate, Enter to launch, Escape to dismiss
- AeroSpace / tiling WM compatible — native NSWindow show/hide, no flicker
- Two window modes:
  - **Normal** — appears on the display with keyboard focus
  - **Fixed** — pinned to a named display (for tiling WM setups)
- Configurable via `~/.config/heats/config.toml`
- Multiple modes with hotkey switching (e.g., launcher, windows, clipboard)
- Mode switching with `Ctrl+Tab` / `Ctrl+Shift+Tab` (hotkey-less modes supported)
- Provider pipeline commands — chain commands with pipes, no `sh -c` needed
- Alternative actions per provider (e.g., Reveal in Finder, copy path)
- Evaluators — query-driven results (e.g., calculator)
- `heats-from-table` — generic TSV/CSV-to-JSONL converter for using any CLI tool as a provider
- macOS application search (`/Applications`, `/System/Applications`)

## Installation

### Build from source

Requires: Rust toolchain, macOS

```bash
cargo install --path .
```

## Configuration

Create `~/.config/heats/config.toml`:

```toml
[window]
width = 600.0
height = 400.0
mode = "normal"   # "normal" = follow keyboard focus, "fixed" = pin to a display

# --- Provider: source (list items) + action (execute on selection) ---
# Commands are pipelines (nested arrays): [["cmd1", "arg"], ["cmd2"]] → cmd1 arg | cmd2
# Field value passing is auto-detected:
#   {} in args → expanded with field value (arg mode)
#   no {}     → field value passed via stdin (stdin mode)

[provider.open-apps]
source = [["heats-list-apps"]]
action = [["open", "-a", "{}"]]
field = "data.path"
cache_interval = 3600

# Alternative actions (triggered by keybindings)
[provider.open-apps.actions.reveal]
command = [["open", "-R", "{}"]]

# Clipboard history (requires pbring: https://github.com/td72/pbring)
[provider.clipboard]
source = [["pbring", "list"], ["heats-from-table", "--title", "4", "--subtitle", "3,2", "--data-field", "id=1"]]
action = [["pbring", "get"], ["pbcopy"]]
field = "data.id"

# Evaluator: query-driven results
[evaluator.calculator]
source = [["heats-eval-calc"]]
action = [["pbcopy"]]
field = "data"

# --- Mode: hotkey → providers + evaluators ---
[[mode]]
name = "launcher"
hotkey = "Cmd+Semicolon"
providers = ["open-apps"]
evaluators = ["calculator"]

[mode.keybindings]
"Alt+Enter" = "reveal"

[[mode]]
name = "clipboard"
hotkey = "Cmd+Shift+V"
providers = ["clipboard"]
```

See [`config.example.toml`](config.example.toml) for the full example.

## Key Bindings

| Key | Action |
|-----|--------|
| `Cmd+;` | Toggle launcher (configurable per mode) |
| `↑` / `↓` | Navigate results |
| `Enter` | Launch / execute default action |
| `Escape` | Dismiss launcher |
| `Ctrl+Tab` | Switch to next mode |
| `Ctrl+Shift+Tab` | Switch to previous mode |

## heats-from-table

A generic TSV/CSV-to-DmenuItem JSONL converter. Turns any CLI tool that outputs tabular data into a heats provider.

```bash
# Convert pbring clipboard history into heats items
pbring list | heats-from-table --title 4 --subtitle 3,2 --data-field id=1

# Process list (--collapse handles variable-width whitespace columns)
ps aux | heats-from-table --header --delimiter ' ' --collapse --title 11 --subtitle 1,3,4 --data-field pid=2
```

Options: `--title <col>`, `--subtitle <col>[,col...]`, `--data-field <key>=<col>`, `--delimiter <char>` (default: tab), `--header` (skip first line), `--collapse` (merge consecutive delimiters, join remainder into last column).

Column numbers are 1-based.

## Development

### Setup

```bash
mise install   # installs prek
mise exec -- prek install   # installs pre-commit hooks
```

### Pre-commit hooks

Managed by [prek](https://github.com/j178/prek):

- `cargo fmt --check`
- `cargo clippy`
- Trailing whitespace, EOF fixer, TOML check, merge conflict check, large file check

### CI

GitHub Actions runs on push to `main` and pull requests:

- prek hooks (fmt + clippy)
- `cargo build`
- `cargo test`

## License

MIT
