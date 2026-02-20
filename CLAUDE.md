# CLAUDE.md

## Concept

Heats is a **rofi-like extensible fuzzy launcher for macOS**, built with Rust + iced.

- Runs as a background daemon with a global hotkey toggle (Cmd+;)
- Fuzzy search via nucleo (helix-editor derived)
- Native NSWindow API for show/hide (AeroSpace / tiling WM compatible)
- Two window modes: Normal (follow keyboard focus) and Fixed (pin to a named display)

## Build & Test

```bash
cargo build           # dev build
cargo clippy          # lint
cargo run             # run (debug)
RUST_LOG=heats=debug cargo run  # run with debug logging
```

## Branch Workflow

Always create a feature branch before making changes. Never commit directly to `main`.
When starting work on an issue, always pull the latest `main` first, then create the branch from it.

```bash
git checkout main && git pull        # update main first
git checkout -b feat/<feature-name>  # create a branch and start working
```

## Conventions

### Issue / Pull Request

When creating an issue or PR, first present the title and body in Japanese for user review. After approval, translate to English and create via `gh` command.

Always assign appropriate labels when creating issues (e.g., `enhancement`, `bug`, `documentation`).

### Copilot Review

After creating a PR or pushing changes (except when pushing fixes for Copilot review comments), request a Copilot review.

**Note:** Copilot cannot be added as a reviewer via CLI/API. The user must add it manually from the GitHub Web UI (PR → Reviewers → Copilot), or configure automatic Copilot review in the repository's Rulesets settings.

### Commit Messages

Use gitmoji prefix: `✨` new feature, `🐛` bug fix, `🩹` minor fix, `♻️` refactor, `🔧` config, `📝` docs, etc.

### Key Architecture

- `src/main.rs` — Entry point: hotkey init + iced daemon startup
- `src/app.rs` — Iced Daemon: State, Message, update, view, subscription
- `src/config.rs` — Config file loading (~/.config/heats/config.toml)
- `src/hotkey.rs` — global-hotkey → iced Subscription bridge
- `src/ui/` — UI components (search_input, result_list, theme)
- `src/source/` — Source trait + ApplicationsSource (extensible)
- `src/matcher/` — nucleo fuzzy matching wrapper
- `src/platform/macos.rs` — macOS native APIs (NSWindow, NSScreen, display detection)
