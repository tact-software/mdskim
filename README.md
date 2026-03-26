# mdskim

ターミナルベースの Markdown ビューア。Vim ライクな操作、Mermaid ダイアグラム、数式レンダリングに対応。

[English](README.en.md)

## 機能

- **Vim ライクなナビゲーション** — `j`/`k`, `d`/`u`, `g`/`G`, `/` 検索, `]`/`[` で見出しジャンプ
- **シンタックスハイライト** — フェンスドコードブロックの言語自動検出
- **Mermaid ダイアグラム** — フローチャート、シーケンス図、クラス図、ER図、ガントチャート、円グラフ等をインライン表示
- **数式レンダリング** — インライン `$...$`、ディスプレイ `$$...$$`（MathJax 経由）
- **画像表示** — PNG, JPEG, GIF, WebP, SVG を対応ターミナルでインライン表示
- **目次** — サイドペイン（`t`）、セクション折りたたみ（`z`）
- **HTML/PDF エクスポート** — `--export-html`, `--export-pdf`（シンタックスハイライト・ダイアグラム付き）
- **マルチファイル** — 複数ファイルを開いて `Ctrl-n`/`Ctrl-p` で切り替え
- **カスタマイズ** — テーマ、キーバインド、見出し装飾、カスタム CSS

## インストール

### mise（推奨）

```bash
# 最新版
mise use -g github:tact-software/mdskim

# バージョン指定
mise use -g github:tact-software/mdskim@0.1.0
```

### ソースからビルド

```bash
git clone https://github.com/tact-software/mdskim.git
cd mdskim
mise run setup    # git hooks + npm install
mise run release  # リリースビルド
```

バイナリは `target/release/mdskim` に生成されます。`PATH` の通った場所にコピーしてください:

```bash
cp target/release/mdskim ~/.local/bin/
```

### 必要なもの

- **Rust** (1.85+)
- **Node.js** — Mermaid、数式レンダリング、PDF エクスポートに必要
- **mise** — タスクランナー
- **mmdc** (`npm:@mermaid-js/mermaid-cli`) — Mermaid レンダリング（`mise install` で自動導入）
- **mathjax-full** — 数式レンダリング（`mise run setup` で自動導入）
- **puppeteer-core** — PDF エクスポート（`mise run setup` で自動導入）
- **Google Chrome / Chromium** — PDF エクスポートに必要（システムにインストール済みのものを使用）

## 使い方

```bash
# ファイルを表示
mdskim README.md

# 複数ファイル（Ctrl-n / Ctrl-p で切り替え）
mdskim file1.md file2.md

# 標準入力から読み込み
cat README.md | mdskim -

# エクスポート
mdskim doc.md --export-html output.html
mdskim doc.md --export-pdf output.pdf

# ライトテーマ
mdskim doc.md --theme light

# 高速モード（Mermaid/Math レンダリングをスキップ）
mdskim doc.md --render-mode fast

# Docker内でPDF出力
mdskim doc.md --export-pdf out.pdf --no-sandbox
```

## キーバインド

| キー | 動作 |
|------|------|
| `j` / `k` | 下 / 上にスクロール |
| `d` / `u` | 半ページ下 / 上 |
| `Ctrl-f` / `Ctrl-b` | ページ下 / 上 |
| `g g` | 先頭へ |
| `G` | 末尾へ |
| `]` / `[` | 次 / 前の見出し |
| `/` | 検索 |
| `n` / `N` | 次 / 前の検索結果 |
| `t` | 目次の表示/非表示 |
| `s` | 目次ペインにフォーカス |
| `l` | リンク一覧の表示/非表示 |
| `o` | カーソル下のリンクを開く |
| `z` | セクション折りたたみ/展開 |
| `Z` | すべて折りたたみ |
| `U` | すべて展開 |
| `r` | ファイル再読み込み |
| `Ctrl-n` / `Ctrl-p` | 次 / 前のファイル |
| `?` | ヘルプ表示 |
| `q` / `Ctrl-c` | 終了 |

すべてのキーバインドは設定ファイルでカスタマイズ可能です。

## 設定

設定ファイル: `~/.config/mdskim/config.toml`

```toml
theme = "dark"          # "dark" または "light"
export_css = "~/my.css" # HTML/PDF エクスポート用カスタム CSS
syntax_dir = "~/syntaxes" # カスタム .sublime-syntax ファイル

[headings.h1]
decoration = "double_underline"
bold = true

[headings.h2]
decoration = "heavy_underline"
bold = true

[headings.h3]
decoration = "light_underline"

[keybindings.custom]
quit = "q"
scroll_down = "j"
scroll_up = "k"
```

### 見出し装飾スタイル

`double_line`, `double_overline`, `double_underline`, `heavy_line`, `heavy_overline`, `heavy_underline`, `light_line`, `light_overline`, `light_underline`, `dotted_underline`, `dashed_underline`, `none`

## 開発

```bash
mise run setup      # git hooks + npm install（初回のみ）
mise run check      # fmt + clippy + test
mise run run -- README.md  # 引数付きで実行
mise run fmt        # コードフォーマット
mise run lint       # clippy
mise run test       # テスト
```

## ライセンス

MIT
