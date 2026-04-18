mod app;
mod clipboard;
mod collectors;
mod config;
mod glyphs;
mod input;
mod models;
mod ui;
mod wt;

use clap::Parser;
use config::Config;
use tracing_subscriber::EnvFilter;

/// wtop - Windows terminal system monitor
#[derive(Parser, Debug)]
#[command(name = "wtop", about = "htop-style system monitor for Windows")]
struct Args {
    /// Refresh interval in milliseconds (250–5000)
    #[arg(short, long)]
    interval: Option<u64>,

    /// Color theme: dark, light, dracula, gruvbox, catppuccin, nord, tokyo_night
    #[arg(short, long)]
    theme: Option<String>,

    /// Log verbosity (off, error, warn, info, debug, trace)
    #[arg(long, default_value = "warn")]
    log_level: String,

    /// Enable Nerd Font glyphs for panel icons (requires a Nerd Font in your terminal)
    #[arg(long)]
    nerd_glyphs: bool,

    /// Disable Nerd Font glyphs (overrides --nerd-glyphs)
    #[arg(long)]
    no_nerd_glyphs: bool,

    /// Force ASCII-only borders and sparklines (for minimal/legacy terminals)
    #[arg(long)]
    ascii: bool,

    /// List all available themes and exit
    #[arg(long)]
    list_themes: bool,

    /// Re-export all built-in themes to the themes directory (overwrites existing) and exit
    #[arg(long)]
    export_themes: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    init_logging(&args.log_level)?;

    if args.list_themes {
        let themes = ui::theme_file::available_themes();
        for slug in &themes {
            let r = ui::theme_file::load_theme(slug);
            let author_ver = match (r.author.as_deref(), r.version.as_deref()) {
                (Some(a), Some(v)) => format!("{a} (v{v})"),
                (Some(a), None)    => a.to_string(),
                (None,    Some(v)) => format!("v{v}"),
                (None,    None)    => String::new(),
            };
            let desc   = r.description.as_deref().unwrap_or("");
            let home   = r.homepage.as_deref().unwrap_or("");
            println!("{:<20} {:<22} {:<36} {}", slug, r.display_name, author_ver, desc);
            if !home.is_empty() {
                println!("{:<20} {}", "", home);
            }
        }
        return Ok(());
    }

    if args.export_themes {
        let dir = ui::theme_file::export_builtin_themes_force();
        println!("Built-in themes exported to {}", dir.display());
        return Ok(());
    }

    let mut config = Config::load();

    if let Some(ms) = args.interval {
        config.refresh_interval_ms = ms.clamp(250, 5000);
    }
    if let Some(ref theme) = args.theme {
        config.theme = theme.to_lowercase();
    }
    if args.nerd_glyphs    { config.nerd_glyphs = true; }
    if args.no_nerd_glyphs { config.nerd_glyphs = false; }
    if args.ascii          { config.ascii_mode = true; }
    if std::env::var("NO_COLOR").is_ok() { config.ascii_mode = true; }

    if !args.nerd_glyphs && !args.no_nerd_glyphs && std::env::var("WT_SESSION").is_ok() {
        config.nerd_glyphs = true;
    }

    app::run(config)
}

fn init_logging(level: &str) -> anyhow::Result<()> {
    let log_dir = std::env::temp_dir();
    let file_appender = tracing_appender::rolling::never(&log_dir, "wtop.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    std::mem::forget(_guard);

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(level))
        .with_writer(non_blocking)
        .with_ansi(false)
        .init();

    Ok(())
}
