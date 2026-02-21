# Development Guide

## Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [mise](https://mise.jdx.dev/) (tool version manager)
- macOS (heats is macOS-only)

```bash
mise install        # Install tools (prek, etc.)
prek install        # Register Git pre-commit hooks
```

## Build & Test

```bash
cargo build              # Debug build
cargo build --release    # Optimized build (strip + LTO enabled)
cargo clippy             # Lint
cargo fmt                # Format
```

### Running

```bash
# Start daemon
RUST_LOG=heats=debug cargo run --bin heatsd

# Daemon management
heatsd stop              # Stop daemon
heatsd restart           # Restart daemon
heatsd service install   # Install launchd service
heatsd service uninstall # Uninstall launchd service

# Dmenu mode (pipe items to fuzzy selector)
echo -e "foo\nbar\nbaz" | cargo run --bin heats
```

## Pre-commit Hooks (prek)

`prek.toml` defines the following hooks that run on every commit:

**Builtin:**
- trailing-whitespace
- end-of-file-fixer
- check-toml
- check-merge-conflict
- check-added-large-files

**Local:**
- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`

To run all hooks manually:

```bash
prek run --all-files
```

## CI/CD

### CI (`ci.yml`)

Runs on push to `main` and all pull requests.

| Job | Runner | Description |
|---|---|---|
| **lint** | macos-latest | prek (fmt, clippy, whitespace, etc.) |
| **build** | 2-platform matrix | `cargo build` |

Build target: `aarch64-apple-darwin` (macos-latest)

### Build (`build.yml`)

Runs on all pull requests (build check only) and tag push (`v*`, full packaging + upload).

On tag push, each platform produces:
- `heats-<target>.tar.gz` (heatsd + heats binaries + LICENSE + THIRDPARTY.yml)
- `heats-<target>.tar.gz.sha256`

Artifacts are uploaded to the corresponding GitHub Release.

### Release (`release.yml`)

Two-stage release process:

1. **create-tag**: Triggered when a `release/*` branch PR is merged to `main`. Creates a Git tag and GitHub Release using a GitHub App token.
2. **publish**: Triggered by the release event. Publishes the crate to crates.io.

#### Release Procedure

```bash
# 1. Create a release branch
git checkout -b release/0.2.0 main

# 2. Update version in Cargo.toml
# 3. Push and create a PR to main
git push -u origin release/0.2.0
gh pr create --title "Release v0.2.0"

# 4. Merge the PR → tag + GitHub Release are created automatically
# 5. Tag push triggers build.yml → artifacts uploaded to Release
# 6. Release event triggers publish job → crate published to crates.io
```

## Infrastructure Setup (Manual)

The following must be configured manually before the release workflow will function:

### 1. GitHub App

Add `heats` to the GitHub App's repository access (same app used by slafling).

Required secrets in the repository:
- `APP_ID` — GitHub App ID
- `APP_PRIVATE_KEY` — GitHub App private key (PEM)

### 2. GitHub Environment

Create a `release` environment in the repository settings (Settings > Environments).

This environment is used by the `publish` job to gate crates.io publishing.

### 3. crates.io Authentication

Choose one of:

- **Trusted publishing** (recommended): Configure in crates.io settings for the `heats` crate
- **Token-based**: Set `CARGO_REGISTRY_TOKEN` secret in the `release` environment
