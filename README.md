# Heats

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
# "normal" = follow keyboard focus (default)
# "fixed"  = pin to a specific display (for tiling WMs like AeroSpace)
mode = "normal"

# Provider: source (list items) + action (execute on selection)
[provider.open-apps]
source = ["heats-list-apps"]
action = ["open", "-a"]
field = "data.path"
cache_interval = 3600

# Mode: hotkey → providers mapping
[[mode]]
name = "launcher"
hotkey = "Cmd+Semicolon"
providers = ["open-apps"]
```

## Key Bindings

| Key | Action |
|-----|--------|
| `Cmd+;` | Toggle launcher (configurable) |
| `↑` / `↓` | Navigate results |
| `Enter` | Launch selected application |
| `Escape` | Dismiss launcher |

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

## License

MIT
