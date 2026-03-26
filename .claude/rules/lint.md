---
description: Lint・フォーマットに関するルール
---

# Lint ルール

## 変更禁止

- Lintルール・フォーマットルールを変更してはいけない
- `#[allow(...)]` や `#[cfg_attr(...)]` でwarningを抑制してはいけない
- clippy の lint level を緩めてはいけない（`-A` フラグ等）
- `clippy.toml` や `rustfmt.toml` を新規作成・変更してはいけない
- `.githooks/pre-commit` のチェック内容を緩めてはいけない

## 現行ルール

- **clippy**: デフォルトルール + `-D warnings`（全warningをエラー扱い）
- **rustfmt**: デフォルト設定
- pre-commit hook で `cargo fmt --check` と `cargo clippy -- -D warnings` を実行

## 対応方針

- clippy の警告が出たら、コードを修正して対応すること
- dead_code 警告は未使用コードを削除して対応する（`#[allow(dead_code)]` は使わない）
- フォーマットは `mise run fmt` で自動修正する
