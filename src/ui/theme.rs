use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols;

use crate::config::{GaugeStyle, ThemeName};

/// Unicode sub-cell block chars used in the CPU sparkline.
const SPARK_CHARS: &[&str] = &[" ", "▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
/// Pure-ASCII fallback sparkline chars for no-color / minimal terminals.
const ASCII_SPARK_CHARS: &[&str] = &[" ", ".", "-", "-", "=", "=", "+", "#", "#"];

#[derive(Clone, Debug)]
pub struct Theme {
    pub border: Style,
    pub border_focused: Style,
    pub title: Style,
    pub header: Style,

    // Gauge / bar fills
    pub gauge_low: Style,     // 0–60%
    pub gauge_medium: Style,  // 60–85%
    pub gauge_high: Style,    // 85–100%

    // Process table
    pub row_normal: Style,
    pub row_zebra: Style,      // subtle alternate background for odd rows
    pub row_selected: Style,
    pub row_thread: Style,
    pub row_suspicious: Style,
    /// Brief flash style for rows whose CPU% jumped >15 pp since last sample.
    pub row_spike: Style,

    // Status text
    pub status_running: Style,
    pub status_suspended: Style,
    pub status_other: Style,

    // Filter bar
    pub filter_active: Style,
    pub filter_inactive: Style,

    pub text_dim: Style,
    pub text_normal: Style,
    pub text_bright: Style,

    /// Background applied to overlay panels and unset table rows.
    /// Set to an explicit white on light themes so black text is always visible
    /// regardless of the terminal's default background colour.
    pub panel_bg: Style,

    /// Box-drawing characters used for all panel borders.
    pub border_set: symbols::border::Set,
    /// How CPU / memory gauge bars are rendered.
    pub gauge_style: GaugeStyle,

    /// Sparkline gradient — low / mid / high fill colours.
    pub spark_low:  Color,
    pub spark_mid:  Color,
    pub spark_high: Color,
    /// Block characters for the sparkline (Unicode or ASCII fallback).
    pub spark_chars: &'static [&'static str],
}

impl Theme {
    /// Return the theme for the given name, honouring NO_COLOR if set.
    pub fn for_name(name: &ThemeName) -> Self {
        if std::env::var("NO_COLOR").is_ok() {
            return Self::no_color();
        }
        match name {
            ThemeName::Dark            => Self::default_dark(),
            ThemeName::Light           => Self::default_light(),
            ThemeName::Dracula         => Self::dracula(),
            ThemeName::Gruvbox         => Self::gruvbox(),
            ThemeName::CatppuccinMocha => Self::catppuccin_mocha(),
            ThemeName::Nord            => Self::nord(),
            ThemeName::TokyoNight      => Self::tokyo_night(),
        }
    }

    /// Gradient colour for the sparkline bar at the given percentage (0–100).
    pub fn spark_color(&self, pct: f64) -> Color {
        if pct >= 85.0 { self.spark_high }
        else if pct >= 60.0 { self.spark_mid }
        else { self.spark_low }
    }

    /// Pick a gauge style based on percentage (0–100).
    pub fn gauge_for_pct(&self, pct: f64) -> Style {
        if pct >= 85.0 {
            self.gauge_high
        } else if pct >= 60.0 {
            self.gauge_medium
        } else {
            self.gauge_low
        }
    }

    // ── Themes ────────────────────────────────────────────────────────────────

    pub fn default_dark() -> Self {
        Self {
            border:         Style::default().fg(Color::DarkGray),
            border_focused: Style::default().fg(Color::Cyan),
            title:          Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            header:         Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),

            gauge_low:    Style::default().fg(Color::Green),
            gauge_medium: Style::default().fg(Color::Yellow),
            gauge_high:   Style::default().fg(Color::Red),

            row_normal:    Style::default().fg(Color::White),
            row_zebra:     Style::default().fg(Color::White).bg(Color::Rgb(22, 22, 32)),
            row_selected:  Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD),
            row_thread:    Style::default().fg(Color::DarkGray),
            row_suspicious: Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            row_spike:     Style::default().fg(Color::Rgb(255, 200, 0)).add_modifier(Modifier::BOLD),

            status_running:   Style::default().fg(Color::Green),
            status_suspended: Style::default().fg(Color::Yellow),
            status_other:     Style::default().fg(Color::Gray),

            filter_active:   Style::default().fg(Color::Black).bg(Color::Yellow),
            filter_inactive: Style::default().fg(Color::DarkGray),

            text_dim:    Style::default().fg(Color::DarkGray),
            text_normal: Style::default().fg(Color::White),
            text_bright: Style::default().fg(Color::White).add_modifier(Modifier::BOLD),

            panel_bg:   Style::default(),
            border_set: symbols::border::PLAIN,
            gauge_style: GaugeStyle::Block,
            spark_low:  Color::Green,
            spark_mid:  Color::Yellow,
            spark_high: Color::Red,
            spark_chars: SPARK_CHARS,
        }
    }

    pub fn default_light() -> Self {
        let white     = Color::White;
        let black     = Color::Black;
        let blue      = Color::Blue;
        let gray      = Color::Gray;
        let dark_gray = Color::DarkGray;
        let green     = Color::Rgb(0, 140, 0);
        let amber     = Color::Rgb(180, 120, 0);
        let red       = Color::Red;
        let zebra_bg  = Color::Rgb(230, 235, 250);

        Self {
            border:         Style::default().fg(gray),
            border_focused: Style::default().fg(blue),
            title:          Style::default().fg(blue).add_modifier(Modifier::BOLD),
            header:         Style::default().fg(Color::Rgb(130, 70, 0)).add_modifier(Modifier::BOLD).bg(white),

            gauge_low:    Style::default().fg(green),
            gauge_medium: Style::default().fg(amber),
            gauge_high:   Style::default().fg(red),

            // Explicit white background on every row style so black text is
            // readable regardless of the terminal's default background colour.
            row_normal:    Style::default().fg(black).bg(white),
            row_zebra:     Style::default().fg(black).bg(zebra_bg),
            row_selected:  Style::default().fg(white).bg(blue).add_modifier(Modifier::BOLD),
            row_thread:    Style::default().fg(dark_gray).bg(white),
            row_suspicious: Style::default().fg(red).add_modifier(Modifier::BOLD).bg(white),
            row_spike:     Style::default().fg(Color::Rgb(180, 100, 0)).add_modifier(Modifier::BOLD).bg(white),

            status_running:   Style::default().fg(green),
            status_suspended: Style::default().fg(amber),
            status_other:     Style::default().fg(gray),

            filter_active:   Style::default().fg(white).bg(blue),
            filter_inactive: Style::default().fg(gray),

            text_dim:    Style::default().fg(dark_gray),
            text_normal: Style::default().fg(black),
            text_bright: Style::default().fg(black).add_modifier(Modifier::BOLD),

            // White panel background ensures overlay text is always readable.
            panel_bg:   Style::default().bg(white),
            border_set: symbols::border::PLAIN,
            gauge_style: GaugeStyle::Block,
            spark_low:  green,
            spark_mid:  amber,
            spark_high: red,
            spark_chars: SPARK_CHARS,
        }
    }

    /// Dracula — https://draculatheme.com/
    /// Background: #282a36  Foreground: #f8f8f2
    /// Purple: #bd93f9  Cyan: #8be9fd  Green: #50fa7b
    /// Orange: #ffb86c  Red: #ff5555   Yellow: #f1fa8c
    pub fn dracula() -> Self {
        let purple  = Color::Rgb(189, 147, 249);
        let cyan    = Color::Rgb(139, 233, 253);
        let green   = Color::Rgb(80,  250, 123);
        let orange  = Color::Rgb(255, 184, 108);
        let red     = Color::Rgb(255, 85,  85);
        let yellow  = Color::Rgb(241, 250, 140);
        let fg      = Color::Rgb(248, 248, 242);
        let comment = Color::Rgb(98,  114, 164);
        let bg_sel  = Color::Rgb(68,  71,  90);

        Self {
            border:         Style::default().fg(comment),
            border_focused: Style::default().fg(purple),
            title:          Style::default().fg(purple).add_modifier(Modifier::BOLD),
            header:         Style::default().fg(yellow).add_modifier(Modifier::BOLD),

            gauge_low:    Style::default().fg(green),
            gauge_medium: Style::default().fg(orange),
            gauge_high:   Style::default().fg(red),


            row_normal:    Style::default().fg(fg),
            row_zebra:     Style::default().fg(fg).bg(Color::Rgb(40, 42, 54)),
            row_selected:  Style::default().fg(fg).bg(bg_sel).add_modifier(Modifier::BOLD),
            row_thread:    Style::default().fg(comment),
            row_suspicious: Style::default().fg(red).add_modifier(Modifier::BOLD),
            row_spike:     Style::default().fg(orange).add_modifier(Modifier::BOLD),

            status_running:   Style::default().fg(green),
            status_suspended: Style::default().fg(orange),
            status_other:     Style::default().fg(comment),

            filter_active:   Style::default().fg(Color::Black).bg(yellow),
            filter_inactive: Style::default().fg(comment),


            text_dim:    Style::default().fg(comment),
            text_normal: Style::default().fg(fg),
            text_bright: Style::default().fg(cyan).add_modifier(Modifier::BOLD),

            panel_bg:   Style::default(),
            border_set: symbols::border::ROUNDED,
            gauge_style: GaugeStyle::Line,
            spark_low:  green,
            spark_mid:  orange,
            spark_high: red,
            spark_chars: SPARK_CHARS,
        }
    }

    /// Gruvbox Dark — https://github.com/morhetz/gruvbox
    /// bg0: #282828  fg: #ebdbb2
    /// red: #cc241d/bright #fb4934  green: #98971a/bright #b8bb26
    /// yellow: #d79921/bright #fabd2f  blue: #458588/bright #83a598
    /// aqua: #689d6a/bright #8ec07c  orange: #d65d0e/bright #fe8019
    pub fn gruvbox() -> Self {
        let orange  = Color::Rgb(254, 128, 25);
        let yellow  = Color::Rgb(250, 189, 47);
        let green   = Color::Rgb(184, 187, 38);
        let _aqua   = Color::Rgb(142, 192, 124);
        let red     = Color::Rgb(251, 73,  52);
        let blue    = Color::Rgb(131, 165, 152);
        let fg      = Color::Rgb(235, 219, 178);
        let gray    = Color::Rgb(168, 153, 132);
        let bg_sel  = Color::Rgb(80,  73,  69);

        Self {
            border:         Style::default().fg(gray),
            border_focused: Style::default().fg(yellow),
            title:          Style::default().fg(yellow).add_modifier(Modifier::BOLD),
            header:         Style::default().fg(orange).add_modifier(Modifier::BOLD),

            gauge_low:    Style::default().fg(green),
            gauge_medium: Style::default().fg(yellow),
            gauge_high:   Style::default().fg(red),


            row_normal:    Style::default().fg(fg),
            row_zebra:     Style::default().fg(fg).bg(Color::Rgb(50, 48, 47)),
            row_selected:  Style::default().fg(fg).bg(bg_sel).add_modifier(Modifier::BOLD),
            row_thread:    Style::default().fg(gray),
            row_suspicious: Style::default().fg(red).add_modifier(Modifier::BOLD),
            row_spike:     Style::default().fg(orange).add_modifier(Modifier::BOLD),

            status_running:   Style::default().fg(green),
            status_suspended: Style::default().fg(yellow),
            status_other:     Style::default().fg(gray),

            filter_active:   Style::default().fg(Color::Black).bg(yellow),
            filter_inactive: Style::default().fg(gray),


            text_dim:    Style::default().fg(gray),
            text_normal: Style::default().fg(fg),
            text_bright: Style::default().fg(blue).add_modifier(Modifier::BOLD),

            panel_bg:   Style::default(),
            border_set: symbols::border::PLAIN,
            gauge_style: GaugeStyle::Block,
            spark_low:  green,
            spark_mid:  yellow,
            spark_high: red,
            spark_chars: SPARK_CHARS,
        }
    }

    /// Catppuccin Mocha — https://github.com/catppuccin/catppuccin
    /// Base: #1e1e2e  Text: #cdd6f4
    /// Mauve: #cba6f7  Blue: #89b4fa  Sapphire: #74c7ec
    /// Green: #a6e3a1  Yellow: #f9e2af  Red: #f38ba8  Peach: #fab387
    /// Overlay0: #6c7086  Overlay2: #9399b2
    pub fn catppuccin_mocha() -> Self {
        let mauve    = Color::Rgb(203, 166, 247);
        let blue     = Color::Rgb(137, 180, 250);
        let _sapphire = Color::Rgb(116, 199, 236);
        let green    = Color::Rgb(166, 227, 161);
        let yellow   = Color::Rgb(249, 226, 175);
        let red      = Color::Rgb(243, 139, 168);
        let peach    = Color::Rgb(250, 179, 135);
        let text     = Color::Rgb(205, 214, 244);
        let overlay0 = Color::Rgb(108, 112, 134);
        let surface1 = Color::Rgb(69,  71,  90);

        Self {
            border:         Style::default().fg(overlay0),
            border_focused: Style::default().fg(mauve),
            title:          Style::default().fg(mauve).add_modifier(Modifier::BOLD),
            header:         Style::default().fg(yellow).add_modifier(Modifier::BOLD),

            gauge_low:    Style::default().fg(green),
            gauge_medium: Style::default().fg(peach),
            gauge_high:   Style::default().fg(red),


            row_normal:    Style::default().fg(text),
            row_zebra:     Style::default().fg(text).bg(Color::Rgb(24, 24, 37)),
            row_selected:  Style::default().fg(text).bg(surface1).add_modifier(Modifier::BOLD),
            row_thread:    Style::default().fg(overlay0),
            row_suspicious: Style::default().fg(red).add_modifier(Modifier::BOLD),
            row_spike:     Style::default().fg(peach).add_modifier(Modifier::BOLD),

            status_running:   Style::default().fg(green),
            status_suspended: Style::default().fg(peach),
            status_other:     Style::default().fg(overlay0),

            filter_active:   Style::default().fg(Color::Black).bg(yellow),
            filter_inactive: Style::default().fg(overlay0),


            text_dim:    Style::default().fg(overlay0),
            text_normal: Style::default().fg(text),
            text_bright: Style::default().fg(blue).add_modifier(Modifier::BOLD),

            panel_bg:   Style::default(),
            border_set: symbols::border::ROUNDED,
            gauge_style: GaugeStyle::Segmented,
            spark_low:  green,
            spark_mid:  peach,
            spark_high: red,
            spark_chars: SPARK_CHARS,
        }
    }

    /// Nord — https://www.nordtheme.com/
    /// Polar Night: #2e3440 #3b4252 #434c5e #4c566a
    /// Snow Storm:  #d8dee9 #e5e9f0 #eceff4
    /// Frost:       #8fbcbb #88c0d0 #81a1c1 #5e81ac
    /// Aurora:      #bf616a #d08770 #ebcb8b #a3be8c #b48ead
    pub fn nord() -> Self {
        let _frost1  = Color::Rgb(143, 188, 187); // #8fbcbb
        let frost2  = Color::Rgb(136, 192, 208); // #88c0d0
        let frost3  = Color::Rgb(129, 161, 193); // #81a1c1
        let red     = Color::Rgb(191, 97,  106); // #bf616a
        let orange  = Color::Rgb(208, 135, 112); // #d08770
        let yellow  = Color::Rgb(235, 203, 139); // #ebcb8b
        let green   = Color::Rgb(163, 190, 140); // #a3be8c
        let snow1   = Color::Rgb(216, 222, 233); // #d8dee9
        let polar3  = Color::Rgb(67,  76,  94);  // #434c5e
        let polar4  = Color::Rgb(76,  86,  106); // #4c566a

        Self {
            border:         Style::default().fg(polar4),
            border_focused: Style::default().fg(frost2),
            title:          Style::default().fg(frost2).add_modifier(Modifier::BOLD),
            header:         Style::default().fg(yellow).add_modifier(Modifier::BOLD),

            gauge_low:    Style::default().fg(green),
            gauge_medium: Style::default().fg(orange),
            gauge_high:   Style::default().fg(red),


            row_normal:    Style::default().fg(snow1),
            row_zebra:     Style::default().fg(snow1).bg(Color::Rgb(59, 66, 82)),
            row_selected:  Style::default().fg(snow1).bg(polar3).add_modifier(Modifier::BOLD),
            row_thread:    Style::default().fg(polar4),
            row_suspicious: Style::default().fg(red).add_modifier(Modifier::BOLD),
            row_spike:     Style::default().fg(orange).add_modifier(Modifier::BOLD),

            status_running:   Style::default().fg(green),
            status_suspended: Style::default().fg(yellow),
            status_other:     Style::default().fg(polar4),

            filter_active:   Style::default().fg(Color::Black).bg(frost2),
            filter_inactive: Style::default().fg(polar4),


            text_dim:    Style::default().fg(polar4),
            text_normal: Style::default().fg(snow1),
            text_bright: Style::default().fg(frost3).add_modifier(Modifier::BOLD),

            panel_bg:   Style::default(),
            border_set: symbols::border::ROUNDED,
            gauge_style: GaugeStyle::Line,
            spark_low:  green,
            spark_mid:  orange,
            spark_high: red,
            spark_chars: SPARK_CHARS,
        }
    }

    /// Tokyo Night — https://github.com/folke/tokyonight.nvim (night variant)
    /// bg: #1a1b26  fg: #c0caf5
    /// blue: #7aa2f7  cyan: #7dcfff  purple: #9d7cd8
    /// green: #9ece6a  yellow: #e0af68  red: #f7768e  orange: #ff9e64
    /// comment: #565f89  selection: #283457
    pub fn tokyo_night() -> Self {
        let blue     = Color::Rgb(122, 162, 247);
        let _cyan    = Color::Rgb(125, 207, 255);
        let purple   = Color::Rgb(157, 124, 216);
        let green    = Color::Rgb(158, 206, 106);
        let yellow   = Color::Rgb(224, 175, 104);
        let red      = Color::Rgb(247, 118, 142);
        let orange   = Color::Rgb(255, 158, 100);
        let fg       = Color::Rgb(192, 202, 245);
        let comment  = Color::Rgb(86,  95,  137);
        let sel      = Color::Rgb(40,  52,  87);

        Self {
            border:         Style::default().fg(comment),
            border_focused: Style::default().fg(blue),
            title:          Style::default().fg(blue).add_modifier(Modifier::BOLD),
            header:         Style::default().fg(yellow).add_modifier(Modifier::BOLD),

            gauge_low:    Style::default().fg(green),
            gauge_medium: Style::default().fg(orange),
            gauge_high:   Style::default().fg(red),


            row_normal:    Style::default().fg(fg),
            row_zebra:     Style::default().fg(fg).bg(Color::Rgb(26, 27, 38)),
            row_selected:  Style::default().fg(fg).bg(sel).add_modifier(Modifier::BOLD),
            row_thread:    Style::default().fg(comment),
            row_suspicious: Style::default().fg(red).add_modifier(Modifier::BOLD),
            row_spike:     Style::default().fg(orange).add_modifier(Modifier::BOLD),

            status_running:   Style::default().fg(green),
            status_suspended: Style::default().fg(yellow),
            status_other:     Style::default().fg(comment),

            filter_active:   Style::default().fg(Color::Black).bg(yellow),
            filter_inactive: Style::default().fg(comment),


            text_dim:    Style::default().fg(comment),
            text_normal: Style::default().fg(fg),
            text_bright: Style::default().fg(purple).add_modifier(Modifier::BOLD),

            panel_bg:   Style::default(),
            border_set: symbols::border::ROUNDED,
            gauge_style: GaugeStyle::Segmented,
            spark_low:  green,
            spark_mid:  orange,
            spark_high: red,
            spark_chars: SPARK_CHARS,
        }
    }

    /// Monochrome fallback when NO_COLOR is set — no colour at all.
    pub fn no_color() -> Self {
        let normal = Style::default();
        let bold   = Style::default().add_modifier(Modifier::BOLD);
        let dim    = Style::default().add_modifier(Modifier::DIM);
        let invert = Style::default().add_modifier(Modifier::REVERSED);

        Self {
            border:         normal,
            border_focused: bold,
            title:          bold,
            header:         bold,

            gauge_low:    normal,
            gauge_medium: normal,
            gauge_high:   bold,


            row_normal:    normal,
            row_zebra:     normal,
            row_selected:  invert,
            row_thread:    dim,
            row_suspicious: bold,
            row_spike:     bold,

            status_running:   normal,
            status_suspended: dim,
            status_other:     dim,

            filter_active:   invert,
            filter_inactive: dim,


            text_dim:    dim,
            text_normal: normal,
            text_bright: bold,

            border_set: symbols::border::Set {
                top_left:         "+",
                top_right:        "+",
                bottom_left:      "+",
                bottom_right:     "+",
                vertical_left:    "|",
                vertical_right:   "|",
                horizontal_top:   "-",
                horizontal_bottom: "-",
            },
            panel_bg:   Style::default(),
            gauge_style: GaugeStyle::Ascii,
            spark_low:  Color::Reset,
            spark_mid:  Color::Reset,
            spark_high: Color::Reset,
            spark_chars: ASCII_SPARK_CHARS,
        }
    }
}
