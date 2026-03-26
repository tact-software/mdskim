use clap::{Parser, Subcommand};
use std::path::PathBuf;

pub use crate::config::{RenderMode, ThemeChoice};

#[derive(Parser, Debug)]
#[command(name = "mdskim", version, about = "A terminal-based Markdown viewer")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Markdown files to view
    pub files: Vec<PathBuf>,

    /// Export as HTML to the specified file path
    #[arg(long = "export-html", value_name = "FILE")]
    pub export_html: Option<PathBuf>,

    /// Export as PDF to the specified file path
    #[arg(long = "export-pdf", value_name = "FILE")]
    pub export_pdf: Option<PathBuf>,

    /// Color theme
    #[arg(long, value_enum, default_value_t = ThemeChoice::Dark)]
    pub theme: ThemeChoice,

    /// Render mode: full (default, Mermaid/Math enabled) or fast (skip rendering)
    #[arg(long, value_enum)]
    pub render_mode: Option<RenderMode>,

    /// Disable Chrome sandbox for PDF export (required when running as root, e.g. Docker)
    #[arg(long)]
    pub no_sandbox: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Install rendering dependencies (Mermaid, Math, PDF)
    Setup {
        /// Install only Mermaid rendering dependencies
        #[arg(long)]
        mermaid: bool,

        /// Install only Math rendering dependencies
        #[arg(long)]
        math: bool,

        /// Install only PDF export dependencies
        #[arg(long)]
        pdf: bool,
    },
}
