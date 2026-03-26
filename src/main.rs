mod app;
mod cli;
mod config;
mod document;
mod export;
mod input;
mod math;
mod mermaid;
mod parser;
mod setup;
mod theme;
mod ui;
mod util;

#[cfg(test)]
mod test_helpers;

use std::io::{self, IsTerminal, Read};

use anyhow::{Context, Result, bail};
use clap::Parser;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use ratatui_image::picker::Picker;

use app::AppState;
use config::{Config, Keymap};

fn main() -> Result<()> {
    let cli = cli::Cli::parse();

    // Handle subcommands
    if let Some(cli::Commands::Setup { mermaid, math, pdf }) = cli.command {
        return setup::run(mermaid, math, pdf);
    }

    let args = cli;
    let cfg = Config::load();

    // Determine first file to load
    let first_file = args.files.first().cloned();
    let source = load_source(&first_file)?;
    let syntax_dir = cfg.syntax_dir.as_deref().map(expand_tilde);
    let doc = parser::parse_markdown(&source, &cfg.headings, syntax_dir.as_deref());

    // Resolve render mode: CLI > config > default (full)
    let fast_mode = match &args.render_mode {
        Some(cli::RenderMode::Fast) => true,
        Some(cli::RenderMode::Full) => false,
        None => cfg.render_mode.as_ref() == Some(&cli::RenderMode::Fast),
    };

    // Clean up old cache files (7+ days old)
    util::evict_old_cache();

    // Export mode
    let export_requested = args.export_html.is_some() || args.export_pdf.is_some();
    if export_requested {
        let base_dir = first_file
            .as_ref()
            .and_then(|p| p.canonicalize().ok())
            .and_then(|p| p.parent().map(|d| d.to_path_buf()));
        if let Some(path) = &args.export_html {
            let export_theme = match args.theme {
                cli::ThemeChoice::Dark => export::ExportTheme::Dark,
                cli::ThemeChoice::Light => export::ExportTheme::Light,
            };
            let custom_css = cfg
                .export_css
                .as_ref()
                .and_then(|p| std::fs::read_to_string(expand_tilde(p)).ok());
            let html = export::to_html(
                &doc,
                export_theme,
                custom_css.as_deref(),
                base_dir.as_deref(),
            );
            std::fs::write(path, html)
                .with_context(|| format!("Failed to write HTML: {}", path.display()))?;
            eprintln!("Exported to {}", path.display());
        }

        if let Some(path) = &args.export_pdf {
            let pdf_css = cfg
                .export_css
                .as_ref()
                .and_then(|p| std::fs::read_to_string(expand_tilde(p)).ok());
            export::pdf::generate(
                &doc,
                path,
                pdf_css.as_deref(),
                base_dir.as_deref(),
                args.no_sandbox,
            )
            .with_context(|| format!("Failed to export PDF: {}", path.display()))?;
            eprintln!("Exported to {}", path.display());
        }

        return Ok(());
    }

    let file_path = first_file.as_ref().and_then(|p| p.canonicalize().ok());

    // Plain text fallback
    if !io::stdout().is_terminal() {
        for line in &doc.lines {
            let text: String = line.spans.iter().map(|s| s.content.as_str()).collect();
            println!("{}", text);
        }
        return Ok(());
    }

    let theme_choice = if std::env::var("NO_COLOR").is_ok() {
        None
    } else {
        Some(cfg.theme.unwrap_or(args.theme))
    };
    let theme = match theme_choice {
        None => theme::Theme::no_color(),
        Some(cli::ThemeChoice::Dark) => theme::Theme::dark(),
        Some(cli::ThemeChoice::Light) => theme::Theme::light(),
    };

    let keymap = Keymap::from_config(&cfg.keybindings);

    let mut state = AppState::new(doc, file_path, theme, cfg.headings);
    state.syntax_dir = syntax_dir;
    if let Some(warning) = cfg.warning {
        state.overlay.status_message = Some(warning);
    }

    // Set up multi-file list
    if args.files.len() > 1 {
        state.files.file_list = args
            .files
            .iter()
            .filter_map(|p| p.canonicalize().ok())
            .collect();
        state.files.file_index = 0;
    }

    state.check_tool_availability();
    if !fast_mode {
        state.prerender_mermaid();
        state.prerender_math();
    }
    state.prerender_images();
    run_tui(&mut state, keymap)
}

fn run_tui(state: &mut AppState, keymap: Keymap) -> Result<()> {
    // Query terminal image protocol before entering raw mode / alternate screen,
    // because Picker::from_query_stdio() needs normal stdio to probe the terminal.
    let picker = Picker::from_query_stdio().ok();

    enable_raw_mode().context("Failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("Failed to enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(&mut terminal, state, keymap, picker);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut AppState,
    keymap: Keymap,
    picker: Option<Picker>,
) -> Result<()> {
    let mut input_handler = input::InputHandler::new(keymap);
    let mut img_state = ui::ImageState::new(picker);
    img_state.preload_protocols(&state.render_cache);
    let mut needs_redraw = true;
    loop {
        if needs_redraw {
            terminal.draw(|frame| ui::draw(frame, state, &input_handler, &mut img_state))?;
            needs_redraw = false;
        }

        let mode = input::InputMode {
            toc_mode: state.overlay.show_toc,
            links_mode: state.overlay.show_links,
            toc_pane_focus: state.overlay.toc_pane_focus,
        };

        if let Some(action) = input_handler.poll(&mode)? {
            let prev_gen = state.render_cache.generation;
            state.apply(action);
            if state.render_cache.generation != prev_gen {
                img_state.preload_protocols(&state.render_cache);
            }
            needs_redraw = true;
        }

        if state.overlay.should_quit {
            return Ok(());
        }
    }
}

const MAX_FILE_SIZE: u64 = 50 * 1024 * 1024; // 50 MB

fn load_source(file: &Option<std::path::PathBuf>) -> Result<String> {
    match file {
        Some(path) => {
            if !path.exists() {
                bail!("File not found: {}", path.display());
            }
            let metadata = std::fs::metadata(path)
                .with_context(|| format!("Cannot read: {}", path.display()))?;
            if metadata.len() > MAX_FILE_SIZE {
                bail!(
                    "File too large ({:.1} MB): {}. Maximum supported size is {} MB.",
                    metadata.len() as f64 / 1024.0 / 1024.0,
                    path.display(),
                    MAX_FILE_SIZE / 1024 / 1024
                );
            }
            let bytes =
                std::fs::read(path).with_context(|| format!("Cannot read: {}", path.display()))?;
            String::from_utf8(bytes)
                .map_err(|_| anyhow::anyhow!("Not a valid UTF-8 file: {}", path.display()))
        }
        None => {
            if io::stdin().is_terminal() {
                bail!("No input. Usage: mdskim <file> [file2...] or pipe via stdin");
            }
            let mut buf = String::new();
            io::stdin()
                .take(MAX_FILE_SIZE)
                .read_to_string(&mut buf)
                .context("Failed to read from stdin")?;
            if buf.len() as u64 >= MAX_FILE_SIZE {
                bail!(
                    "Stdin input too large (>= {} MB). Maximum supported size is {} MB.",
                    MAX_FILE_SIZE / 1024 / 1024,
                    MAX_FILE_SIZE / 1024 / 1024
                );
            }
            Ok(buf)
        }
    }
}

fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/")
        && let Ok(home) = std::env::var("HOME")
    {
        return format!("{home}/{rest}");
    }
    path.to_string()
}
