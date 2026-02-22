# CLAUDE.md

## Concept

Heats is a **rofi-like extensible fuzzy launcher for macOS**, built with Rust + iced.

- Runs as a background daemon with a global hotkey toggle (Cmd+;)
- Fuzzy search via nucleo (helix-editor derived)
- Native NSWindow API for show/hide (AeroSpace / tiling WM compatible)
- Two window modes: Normal (follow keyboard focus) and Fixed (pin to a named display)

## Build & Test

```bash
cargo build                          # workspace全体ビルド
cargo clippy                         # lint
cargo build -p heats-daemon          # daemon単体ビルド
cargo build -p heats-providers       # プロバイダ単体ビルド
cargo run -p heats-daemon            # run daemon (debug)
RUST_LOG=heats=debug cargo run -p heats-daemon  # run with debug logging
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

### Key Architecture (Workspace)

4-crate workspace: `heats-core` (共有ライブラリ), `heats-client` (CLI client), `heats-daemon` (daemon binary), `heats-providers` (軽量バイナリ群)

#### heats-core (lib) — 共有型 + プラットフォーム API + IPC + config
- `crates/heats-core/src/source/` — DmenuItem, SourceItem, IconData, scan_apps, scan_windows
- `crates/heats-core/src/config.rs` — Config, ModeConfig, ProviderConfig, WindowConfig
- `crates/heats-core/src/platform/macos.rs` — macOS native APIs (NSWindow, NSScreen, focus_window)
- `crates/heats-core/src/ipc/` — socket_path, PID management

#### heats-client (bin: heats) — dmenu 互換 IPC クライアント
- `crates/heats-client/src/lib.rs` — IPC client (send_and_receive, read_stdin_items)
- `crates/heats-client/src/main.rs` — CLI entry point

#### heats-daemon (bin: heatsd) — iced + fuzzy matching + hotkey
- `crates/heats-daemon/src/main.rs` — Entry point: hotkey init + iced daemon startup
- `crates/heats-daemon/src/app.rs` — Iced Daemon: State, Message, update, view, subscription
- `crates/heats-daemon/src/command.rs` — Provider command execution + item loading
- `crates/heats-daemon/src/hotkey.rs` — global-hotkey → iced Subscription bridge
- `crates/heats-daemon/src/ipc_server.rs` — Unix socket server for dmenu protocol
- `crates/heats-daemon/src/matcher/` — nucleo fuzzy matching wrapper
- `crates/heats-daemon/src/ui/` — UI components (search_input, result_list, theme)

#### heats-providers (bins: heats-list-apps, heats-list-windows, heats-focus-window)
- Lightweight binaries that do NOT depend on iced/nucleo/global-hotkey
- `crates/heats-providers/src/bin/` — source/action providers
