// src/ui/theme_file.rs
//
// File-based theme loading.  Themes are TOML documents that live in
// %APPDATA%\wtop\themes\<slug>.toml.  The seven built-in themes are
// embedded in the binary via include_str! so the app is always self-
// contained, but they are also exported to the user's themes directory on
// first launch so they have working examples to copy and edit.
//
// Schema quick-reference
// ──────────────────────
//   name         = "Display Name"
//   border_style = "plain" | "rounded" | "thick" | "double"
//   gauge_style  = "block"  | "line"   | "segmented" | "ascii"
//   spark_chars  = "unicode" | "ascii"
//   panel_bg     = "<color>"   # optional - set for light themes
//
//   [palette]
//   my_color = "#rrggbb"     # named aliases for use in [colors]
//
//   [colors]
//   border = "my_color"      # palette key, hex (#rrggbb / #rgb), or
//   title  = "#cba6f7"       # terminal color name (cyan, dark_gray, …)
//   …

use std::collections::HashMap;
use std::path::PathBuf;

use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols;
use serde::Deserialize;

use crate::config::GaugeStyle;
use super::theme::{Theme, SPARK_CHARS, ASCII_SPARK_CHARS};

// ── Embedded built-in themes ─────────────────────────────────────────────────

const THEME_DARK:           &str = include_str!("../../themes/dark.toml");
const THEME_LIGHT:          &str = include_str!("../../themes/light.toml");
const THEME_DRACULA:        &str = include_str!("../../themes/dracula.toml");
const THEME_GRUVBOX:        &str = include_str!("../../themes/gruvbox.toml");
const THEME_CATPPUCCIN:     &str = include_str!("../../themes/catppuccin_mocha.toml");
const THEME_NORD:           &str = include_str!("../../themes/nord.toml");
const THEME_TOKYO_NIGHT:    &str = include_str!("../../themes/tokyo_night.toml");
const THEME_SOLARIZED_DARK: &str = include_str!("../../themes/solarized_dark.toml");
const THEME_ONE_DARK:       &str = include_str!("../../themes/one_dark.toml");
const THEME_MONOKAI:        &str = include_str!("../../themes/monokai.toml");
const THEME_CYBERPUNK:      &str = include_str!("../../themes/cyberpunk.toml");

/// Slug → embedded TOML source for every built-in theme.
/// The order here is also the default cycle order.
pub const BUILTIN_THEMES: &[(&str, &str)] = &[
    ("dark",             THEME_DARK),
    ("light",            THEME_LIGHT),
    ("catppuccin_mocha", THEME_CATPPUCCIN),
    ("cyberpunk",        THEME_CYBERPUNK),
    ("dracula",          THEME_DRACULA),
    ("gruvbox",          THEME_GRUVBOX),
    ("monokai",          THEME_MONOKAI),
    ("nord",             THEME_NORD),
    ("one_dark",         THEME_ONE_DARK),
    ("solarized_dark",   THEME_SOLARIZED_DARK),
    ("tokyo_night",      THEME_TOKYO_NIGHT),
];

// ── Path helpers ─────────────────────────────────────────────────────────────

/// `%APPDATA%\wtop\themes\` on Windows; falls back to `./themes/`.
pub fn themes_dir() -> PathBuf {
    let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
    PathBuf::from(appdata).join("wtop").join("themes")
}

// ── Public API ────────────────────────────────────────────────────────────────

/// All available theme slugs: built-ins first, then any extra *.toml files
/// the user has placed in the themes directory.
pub fn available_themes() -> Vec<String> {
    let mut themes: Vec<String> = BUILTIN_THEMES
        .iter()
        .map(|(slug, _)| (*slug).to_string())
        .collect();

    if let Ok(entries) = std::fs::read_dir(themes_dir()) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "toml") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    let slug = stem.to_string();
                    if !themes.contains(&slug) {
                        themes.push(slug);
                    }
                }
            }
        }
    }

    themes
}

/// Result of loading a theme - always contains a usable theme even on error.
pub struct ThemeLoadResult {
    pub theme: Theme,
    /// Human-readable display name from the `name =` field, or a formatted slug.
    pub display_name: String,
    /// From the `author =` field.
    pub author: Option<String>,
    /// From the `version =` field - shown by `--list-themes`.
    pub version: Option<String>,
    /// From the `description =` field - shown by `--list-themes`.
    pub description: Option<String>,
    /// From the `homepage =` field - link to the upstream color scheme.
    pub homepage: Option<String>,
    /// Set when the TOML parsed but had errors, or the file could not be read.
    pub error: Option<String>,
}

/// Load a theme by slug.
///
/// Resolution order:
///   1. `%APPDATA%\wtop\themes\<slug>.toml`  (user file - wins over built-ins)
///   2. Embedded built-in TOML for that slug
///   3. Compiled-in `Theme::default_dark()` as a last resort
///
/// Always returns a valid theme; parse errors are reported in `ThemeLoadResult::error`.
pub fn load_theme(name: &str) -> ThemeLoadResult {
    let slug = normalize_slug(name);

    // 1. User file override.
    let user_path = themes_dir().join(format!("{slug}.toml"));
    if let Ok(src) = std::fs::read_to_string(&user_path) {
        match parse_theme_file(&src) {
            Ok(file) => {
                let display_name = file.name.clone()
                    .unwrap_or_else(|| format_theme_label(&slug));
                let author      = file.author.clone();
                let version     = file.version.clone();
                let description = file.description.clone();
                let homepage    = file.homepage.clone();
                return ThemeLoadResult {
                    theme: file.into(),
                    display_name,
                    author,
                    version,
                    description,
                    homepage,
                    error: None,
                };
            }
            Err(e) => {
                tracing::warn!("Could not parse user theme '{}': {e}", user_path.display());
                let builtin = load_builtin(&slug);
                return ThemeLoadResult {
                    error: Some(format!("'{}': {e}", user_path.file_name()
                        .and_then(|n| n.to_str()).unwrap_or(&slug))),
                    ..builtin
                };
            }
        }
    }

    // 2. Embedded built-in / compiled-in fallback.
    load_builtin(&slug)
}

fn load_builtin(slug: &str) -> ThemeLoadResult {
    for (builtin_slug, src) in BUILTIN_THEMES {
        if *builtin_slug == slug {
            if let Ok(file) = parse_theme_file(src) {
                let display_name = file.name.clone()
                    .unwrap_or_else(|| format_theme_label(slug));
                let author      = file.author.clone();
                let version     = file.version.clone();
                let description = file.description.clone();
                let homepage    = file.homepage.clone();
                return ThemeLoadResult {
                    theme: file.into(),
                    display_name,
                    author,
                    version,
                    description,
                    homepage,
                    error: None,
                };
            }
        }
    }
    ThemeLoadResult {
        theme:        Theme::default_dark(),
        display_name: "Dark".to_string(),
        author:       None,
        version:      None,
        description:  None,
        homepage:     None,
        error:        None,
    }
}

/// Return the last-modified time of the user override file for `slug`, if it exists.
pub fn check_theme_mtime(slug: &str) -> Option<std::time::SystemTime> {
    let slug = normalize_slug(slug);
    themes_dir()
        .join(format!("{slug}.toml"))
        .metadata()
        .ok()?
        .modified()
        .ok()
}

/// Export every built-in theme to the user's themes directory.
/// Existing files are **not** overwritten so user edits are preserved.
/// Called automatically on first launch.
pub fn export_builtin_themes() {
    let dir = themes_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        tracing::warn!("Could not create themes directory: {e}");
        return;
    }
    for (slug, src) in BUILTIN_THEMES {
        let path = dir.join(format!("{slug}.toml"));
        if !path.exists() {
            if let Err(e) = std::fs::write(&path, src) {
                tracing::warn!("Could not write theme file {}: {e}", path.display());
            }
        }
    }
}

/// Like `export_builtin_themes` but overwrites existing files.
/// Used by `--export-themes` to let users reset a built-in they've edited.
pub fn export_builtin_themes_force() -> std::path::PathBuf {
    let dir = themes_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        tracing::warn!("Could not create themes directory: {e}");
        return dir;
    }
    for (slug, src) in BUILTIN_THEMES {
        let path = dir.join(format!("{slug}.toml"));
        if let Err(e) = std::fs::write(&path, src) {
            tracing::warn!("Could not write theme file {}: {e}", path.display());
        }
    }
    dir
}

/// Format a slug for human display ("tokyo_night" → "Tokyo Night").
pub fn format_theme_label(slug: &str) -> String {
    slug.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None    => String::new(),
                Some(f) => f.to_uppercase().to_string() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Normalise theme names coming from old configs or CLI flags.
fn normalize_slug(name: &str) -> String {
    match name.to_lowercase().replace(['-', ' '], "_").as_str() {
        "catppuccin" | "catppuccinmocha" | "catppuccin mocha" => "catppuccin_mocha".into(),
        "tokyonight" | "tokyo night"                          => "tokyo_night".into(),
        other => other.to_string(),
    }
}

fn parse_theme_file(src: &str) -> Result<ThemeFile, String> {
    toml::from_str::<ThemeFile>(src).map_err(|e| e.to_string())
}

// ── Serde types ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ThemeFile {
    pub name: Option<String>,
    pub author: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub homepage: Option<String>,
    #[serde(default = "default_border_style")]
    pub border_style: String,
    #[serde(default = "default_gauge_style")]
    pub gauge_style: String,
    #[serde(default = "default_spark_chars")]
    pub spark_chars: String,
    /// Explicit panel background - set for light themes.
    pub panel_bg: Option<String>,
    /// Named color aliases referenced in [colors].
    #[serde(default)]
    pub palette: HashMap<String, String>,
    #[serde(default)]
    pub colors: ThemeColors,
}

fn default_border_style() -> String { "plain".into() }
fn default_gauge_style()  -> String { "block".into() }
fn default_spark_chars()  -> String { "unicode".into() }

/// All fields are optional - missing values fall back to the dark theme equivalent.
#[derive(Debug, Default, Deserialize)]
pub struct ThemeColors {
    pub border:           Option<String>,
    pub border_focused:   Option<String>,
    pub title:            Option<String>,
    pub header:           Option<String>,

    pub gauge_high:       Option<String>,

    /// If set, applied as the background of normal / thread / suspicious / spike
    /// rows.  Use `"white"` for light themes.
    pub row_bg:           Option<String>,
    pub row_normal_fg:    Option<String>,
    pub row_zebra_fg:     Option<String>,
    pub row_zebra_bg:     Option<String>,
    pub row_selected_fg:  Option<String>,
    pub row_selected_bg:  Option<String>,
    pub row_thread:       Option<String>,
    pub row_suspicious:   Option<String>,
    pub row_spike:        Option<String>,

    pub status_running:   Option<String>,
    pub status_suspended: Option<String>,
    pub status_other:     Option<String>,

    pub filter_active_fg: Option<String>,
    pub filter_active_bg: Option<String>,
    pub filter_inactive:  Option<String>,

    pub text_dim:         Option<String>,
    pub text_normal:      Option<String>,
    pub text_bright:      Option<String>,

}

// ── ThemeFile → Theme conversion ──────────────────────────────────────────────

impl From<ThemeFile> for Theme {
    fn from(f: ThemeFile) -> Theme {
        let c = &f.colors;
        let pal = &f.palette;

        // Resolve a color string: palette lookup → hex/name parse → fallback.
        let resolve = |opt: &Option<String>, fallback: Color| -> Color {
            let s = match opt {
                None    => return fallback,
                Some(s) => s.as_str(),
            };
            // If the value is a palette key, dereference it.
            let hex = pal.get(s).map_or(s, |v| v.as_str());
            parse_color(hex).unwrap_or(fallback)
        };

        // ── Border set ────────────────────────────────────────────────────────
        let border_set = match f.border_style.as_str() {
            "rounded" => symbols::border::ROUNDED,
            "thick"   => symbols::border::THICK,
            "double"  => symbols::border::DOUBLE,
            _         => symbols::border::PLAIN,
        };

        // ── Gauge style ───────────────────────────────────────────────────────
        let gauge_style = match f.gauge_style.as_str() {
            "line"      => GaugeStyle::Line,
            "segmented" => GaugeStyle::Segmented,
            "ascii"     => GaugeStyle::Ascii,
            _           => GaugeStyle::Block,
        };

        // ── Spark chars ───────────────────────────────────────────────────────
        let spark_chars: &'static [&'static str] = if f.spark_chars == "ascii" {
            ASCII_SPARK_CHARS
        } else {
            SPARK_CHARS
        };

        // ── Panel background ──────────────────────────────────────────────────
        let panel_bg = match &f.panel_bg {
            None    => Style::default(),
            Some(s) => {
                let hex = pal.get(s.as_str()).map_or(s.as_str(), |v| v.as_str());
                match parse_color(hex) {
                    Some(bg) => Style::default().bg(bg),
                    None     => Style::default(),
                }
            }
        };

        // ── row_bg - optional explicit background for plain rows ──────────────
        let row_bg: Option<Color> = c.row_bg.as_ref().and_then(|s| {
            let hex = pal.get(s.as_str()).map_or(s.as_str(), |v| v.as_str());
            parse_color(hex)
        });
        let with_row_bg = |style: Style| -> Style {
            match row_bg {
                Some(bg) => style.bg(bg),
                None     => style,
            }
        };

        // ── Assemble Theme ────────────────────────────────────────────────────
        Theme {
            border:         Style::default().fg(resolve(&c.border,         Color::DarkGray)),
            border_focused: Style::default().fg(resolve(&c.border_focused, Color::Cyan)),
            title:          Style::default().fg(resolve(&c.title,          Color::Cyan))
                                            .add_modifier(Modifier::BOLD),
            header:         Style::default().fg(resolve(&c.header,         Color::Yellow))
                                            .add_modifier(Modifier::BOLD),

            gauge_high: Style::default().fg(resolve(&c.gauge_high, Color::Red)),

            row_normal:    with_row_bg(Style::default().fg(resolve(&c.row_normal_fg,   Color::White))),
            row_zebra:     Style::default()
                               .fg(resolve(&c.row_zebra_fg, Color::White))
                               .bg(resolve(&c.row_zebra_bg, Color::Rgb(22, 22, 32))),
            row_selected:  Style::default()
                               .fg(resolve(&c.row_selected_fg, Color::Black))
                               .bg(resolve(&c.row_selected_bg, Color::Cyan))
                               .add_modifier(Modifier::BOLD),
            row_thread:    with_row_bg(Style::default().fg(resolve(&c.row_thread,    Color::DarkGray))),
            row_suspicious: with_row_bg(
                Style::default()
                    .fg(resolve(&c.row_suspicious, Color::Red))
                    .add_modifier(Modifier::BOLD),
            ),
            row_spike:     with_row_bg(
                Style::default()
                    .fg(resolve(&c.row_spike, Color::Rgb(255, 200, 0)))
                    .add_modifier(Modifier::BOLD),
            ),

            status_running:   Style::default().fg(resolve(&c.status_running,   Color::Green)),
            status_suspended: Style::default().fg(resolve(&c.status_suspended, Color::Yellow)),
            status_other:     Style::default().fg(resolve(&c.status_other,     Color::Gray)),

            filter_active:   Style::default()
                                 .fg(resolve(&c.filter_active_fg, Color::Black))
                                 .bg(resolve(&c.filter_active_bg, Color::Yellow)),
            filter_inactive: Style::default().fg(resolve(&c.filter_inactive, Color::DarkGray)),

            text_dim:    Style::default().fg(resolve(&c.text_dim,    Color::DarkGray)),
            text_normal: Style::default().fg(resolve(&c.text_normal, Color::White)),
            text_bright: Style::default()
                             .fg(resolve(&c.text_bright, Color::White))
                             .add_modifier(Modifier::BOLD),

            panel_bg,
            border_set,
            gauge_style,
            spark_chars,
        }
    }
}

// ── Color parsing ─────────────────────────────────────────────────────────────

/// Parse a color string into a ratatui `Color`.
///
/// Accepted formats
/// - Hex: `#rrggbb` or shorthand `#rgb`
/// - Named terminal colors: `black`, `cyan`, `dark_gray`, `white`, …
fn parse_color(s: &str) -> Option<Color> {
    if let Some(hex) = s.strip_prefix('#') {
        return parse_hex(hex);
    }
    match s.to_lowercase().replace('-', "_").as_str() {
        "reset"                             => Some(Color::Reset),
        "black"                             => Some(Color::Black),
        "red"                               => Some(Color::Red),
        "green"                             => Some(Color::Green),
        "yellow"                            => Some(Color::Yellow),
        "blue"                              => Some(Color::Blue),
        "magenta"                           => Some(Color::Magenta),
        "cyan"                              => Some(Color::Cyan),
        "gray" | "grey"                     => Some(Color::Gray),
        "dark_gray" | "darkgray"
        | "dark_grey" | "darkgrey"          => Some(Color::DarkGray),
        "light_red"   | "lightred"          => Some(Color::LightRed),
        "light_green" | "lightgreen"        => Some(Color::LightGreen),
        "light_yellow"| "lightyellow"       => Some(Color::LightYellow),
        "light_blue"  | "lightblue"         => Some(Color::LightBlue),
        "light_magenta"| "lightmagenta"     => Some(Color::LightMagenta),
        "light_cyan"  | "lightcyan"         => Some(Color::LightCyan),
        "white"                             => Some(Color::White),
        _                                   => None,
    }
}

fn parse_hex(hex: &str) -> Option<Color> {
    match hex.len() {
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(Color::Rgb(r, g, b))
        }
        3 => {
            // #rgb → #rrggbb (each nibble doubled: 0xA → 0xAA = 170)
            let r = u8::from_str_radix(&hex[0..1], 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()?;
            Some(Color::Rgb(r * 17, g * 17, b * 17))
        }
        _ => None,
    }
}
