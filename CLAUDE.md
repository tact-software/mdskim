# mdskim

Terminal-based Markdown viewer with Vim-like navigation and Mermaid support.

## Rules

詳細なルールは `.claude/rules/` を参照:

- `lint.md` — Lint・フォーマットルール（変更禁止）
- `project.md` — プロジェクト全般（mise、依存管理、Git）
- `rust.md` — Rustコーディングルール

## Tech Stack

- clap, anyhow, crossterm, ratatui, pulldown-cmark, syntect, serde, toml
- ratatui-image, image (inline terminal images)
- mermaid, puppeteer-core, mathjax-full (Node.js, via `mdskim setup`)

## Config

設定ファイル: `~/.config/mdskim/config.toml`（`config.example.toml` 参照）

## Quick Start

```
mise run setup      # git hooks 有効化（初回のみ）
mise run check      # fmt + clippy + test
mise run run -- README.md
```
