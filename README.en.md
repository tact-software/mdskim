# mdskim

Terminal-based Markdown viewer with Vim-like navigation, Mermaid diagrams, and math rendering.

[Japanese (README.md)](README.md)

## Features

- **Vim-like navigation** — `j`/`k`, `d`/`u`, `g`/`G`, `/` search, heading jump with `]`/`[`
- **Syntax highlighting** — Fenced code blocks with language detection
- **Mermaid diagrams** — Flowcharts, sequence, class, state, ER, gantt, pie, git graphs rendered inline
- **Math rendering** — Inline `$...$` and display `$$...$$` via MathJax
- **Image display** — PNG, JPEG, GIF, WebP, SVG inline in supported terminals
- **Table of Contents** — Side pane (`t`) with section folding (`z`)
- **HTML/PDF export** — `--export-html`, `--export-pdf` with syntax highlighting and diagrams
- **Multi-file** — Open multiple files, switch with `Ctrl-n`/`Ctrl-p`
- **Configurable** — Theme, keybindings, heading decorations, custom CSS

## Install

```bash
git clone https://github.com/tact-software/mdskim.git
cd mdskim
mise run setup    # git hooks + npm install
mise run release  # build release binary
```

The binary is at `target/release/mdskim`. Copy it to your `PATH`:

```bash
cp target/release/mdskim ~/.local/bin/
```

### Requirements

- **Rust** (1.85+)
- **Node.js** — Required for Mermaid, math rendering, and PDF export
- **mise** — Task runner
- **mmdc** (`npm:@mermaid-js/mermaid-cli`) — Mermaid rendering (auto-installed via `mise install`)
- **mathjax-full** — Math rendering (auto-installed via `mise run setup`)
- **puppeteer-core** — PDF export (auto-installed via `mise run setup`)
- **Google Chrome / Chromium** — Required for PDF export (uses system-installed browser)

## Usage

```bash
# View a file
mdskim README.md

# Multiple files (Ctrl-n / Ctrl-p to switch)
mdskim file1.md file2.md

# Pipe from stdin
cat README.md | mdskim -

# Export
mdskim doc.md --export-html output.html
mdskim doc.md --export-pdf output.pdf

# Light theme
mdskim doc.md --theme light

# 高速モード（Mermaid/Math レンダリングをスキップ）
mdskim doc.md --render-mode fast

# Docker内でPDF出力
mdskim doc.md --export-pdf out.pdf --no-sandbox
```

## Keybindings

| Key | Action |
|-----|--------|
| `j` / `k` | Scroll down / up |
| `d` / `u` | Half-page down / up |
| `Ctrl-f` / `Ctrl-b` | Page down / up |
| `g g` | Go to top |
| `G` | Go to bottom |
| `]` / `[` | Next / previous heading |
| `/` | Search |
| `n` / `N` | Next / previous match |
| `t` | Toggle Table of Contents |
| `s` | Focus TOC pane |
| `l` | Toggle links pane |
| `o` | Open link under cursor |
| `z` | Fold / unfold section |
| `Z` | Fold all |
| `U` | Unfold all |
| `r` | Reload file |
| `Ctrl-n` / `Ctrl-p` | Next / previous file |
| `?` | Toggle help |
| `q` / `Ctrl-c` | Quit |

All keybindings are customizable via config file.

## Configuration

Config file: `~/.config/mdskim/config.toml`

```toml
theme = "dark"          # "dark" or "light"
export_css = "~/my.css" # Custom CSS for HTML/PDF export
syntax_dir = "~/syntaxes" # Custom .sublime-syntax files

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

### Heading decoration styles

`double_line`, `double_overline`, `double_underline`, `heavy_line`, `heavy_overline`, `heavy_underline`, `light_line`, `light_overline`, `light_underline`, `dotted_underline`, `dashed_underline`, `none`

## Development

```bash
mise run setup      # git hooks + npm install (first time)
mise run check      # fmt + clippy + test
mise run run -- README.md  # run with arguments
mise run fmt        # format code
mise run lint       # clippy
mise run test       # tests
```

## License

MIT
