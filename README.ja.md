# Heats

macOS 向け rofi 風の拡張可能なファジーランチャー。Rust + [iced](https://iced.rs) で構築。

## 特徴

- グローバルホットキーでトグル（デフォルト: `Cmd+;`）
- [nucleo](https://github.com/helix-editor/nucleo) による高速ファジーマッチング
- キーボード操作: 矢印キーで移動、Enter で実行、Escape で閉じる
- AeroSpace / タイリング WM 対応 — ネイティブ NSWindow の show/hide でちらつきなし
- 2つのウィンドウモード:
  - **Normal** — キーボードフォーカスのあるディスプレイに表示
  - **Fixed** — 指定ディスプレイに固定（タイリング WM 向け）
- `~/.config/heats/config.toml` で設定
- 複数モード対応（ランチャー、ウィンドウ、クリップボードなど）
- `Ctrl+Tab` / `Ctrl+Shift+Tab` でモード切り替え（ホットキーなしのモードも可）
- パイプラインコマンド — `sh -c` 不要でコマンドをパイプ接続
- プロバイダごとの代替アクション（例: Finder で表示、パスをコピー）
- Evaluator — クエリ駆動の結果表示（例: 電卓）
- `heats-from-table` — 任意の CLI ツールをプロバイダとして使える汎用 TSV/CSV → JSONL 変換ツール
- macOS アプリケーション検索（`/Applications`、`/System/Applications`）

## インストール

### ソースからビルド

必要環境: Rust ツールチェイン、macOS

```bash
cargo install --path .
```

## 設定

`~/.config/heats/config.toml` を作成:

```toml
[window]
width = 600.0
height = 400.0
mode = "normal"   # "normal" = キーボードフォーカスに追従, "fixed" = ディスプレイ固定

# --- Provider: source (一覧取得) + action (選択時の実行) ---
# コマンドはパイプラインとして二重配列で指定: [["cmd1", "arg"], ["cmd2"]] → cmd1 arg | cmd2
# field の値の渡し方は自動判定:
#   {} がある → その位置に値を展開 (arg モード)
#   {} がない → 最初のコマンドの stdin に値を渡す (stdin モード)

[provider.open-apps]
source = [["heats-list-apps"]]
action = [["open", "-a", "{}"]]
field = "data.path"
cache_interval = 3600

# 代替アクション (キーバインドで実行)
[provider.open-apps.actions.reveal]
command = [["open", "-R", "{}"]]

# クリップボード履歴 (要 pbring: https://github.com/td72/pbring)
[provider.clipboard]
source = [["pbring", "list"], ["heats-from-table", "--title", "4", "--subtitle", "3,2", "--data-field", "id=1"]]
action = [["pbring", "get"], ["pbcopy"]]
field = "data.id"

# Evaluator: クエリ駆動の結果
[evaluator.calculator]
source = [["heats-eval-calc"]]
action = [["pbcopy"]]
field = "data"

# --- Mode: ホットキー → providers + evaluators のマッピング ---
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

設定の全例は [`config.example.toml`](config.example.toml) を参照。

## キーバインド

| キー | アクション |
|------|-----------|
| `Cmd+;` | ランチャーの表示/非表示（モードごとに設定可能） |
| `↑` / `↓` | 結果の移動 |
| `Enter` | 実行（デフォルトアクション） |
| `Escape` | ランチャーを閉じる |
| `Ctrl+Tab` | 次のモードへ切り替え |
| `Ctrl+Shift+Tab` | 前のモードへ切り替え |

## heats-from-table

TSV/CSV を DmenuItem JSONL に変換する汎用ツール。表形式の出力を持つ任意の CLI コマンドを heats のプロバイダとして利用可能にします。

```bash
# pbring のクリップボード履歴を heats アイテムに変換
pbring list | heats-from-table --title 4 --subtitle 3,2 --data-field id=1

# プロセス一覧 (--collapse でスペース区切りの可変幅列に対応)
ps aux | heats-from-table --header --delimiter ' ' --collapse --title 11 --subtitle 1,3,4 --data-field pid=2
```

オプション: `--title <col>`, `--subtitle <col>[,col...]`, `--data-field <key>=<col>`, `--delimiter <char>` (デフォルト: タブ), `--header` (1行目をスキップ), `--collapse` (連続デリミタをまとめ、最大列以降を最後の列に結合)

カラム番号は 1 始まりです。

## 開発

### セットアップ

```bash
mise install   # prek をインストール
mise exec -- prek install   # pre-commit hooks をインストール
```

### Pre-commit hooks

[prek](https://github.com/j178/prek) で管理:

- `cargo fmt --check`
- `cargo clippy`
- 末尾スペース、EOF 修正、TOML チェック、マージコンフリクトチェック、大きいファイルチェック

### CI

GitHub Actions が `main` への push と pull request で実行:

- prek hooks (fmt + clippy)
- `cargo build`
- `cargo test`

## ライセンス

MIT
