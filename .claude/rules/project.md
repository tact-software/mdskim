---
description: プロジェクト全般のルール
---

# プロジェクトルール

## ツール

- ツール管理・タスクランナーは **mise** を使うこと
- コマンド実行時は直接 `cargo` を叩かず、**mise タスク**を使うこと

```
mise run build      # cargo build
mise run release    # cargo build --release
mise run run        # cargo run
mise run test       # cargo test
mise run lint       # cargo clippy -- -D warnings
mise run fmt        # cargo fmt
mise run check      # fmt + clippy + test（全チェック）
mise run setup      # git hooks の初期設定
```

## 依存管理

- できるだけRustライブラリ（クレート）を活用すること
- 車輪の再発明を避け、成熟したクレートを優先する
- 新しいクレートを追加する際は、メンテナンス状況とダウンロード数を考慮する

## Git

- コミット前に `mise run check` が通ることを確認する
- pre-commit hook（`.githooks/pre-commit`）で自動チェックされる
- 初回セットアップ時は `mise run setup` を実行して hooks を有効化する
