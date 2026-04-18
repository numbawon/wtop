//! Glyph sets for panel decoration.
//!
//! `Glyphs::plain()` uses only standard Unicode code-points that render
//! everywhere. `Glyphs::nerd()` swaps in Nerd Font private-use code-points
//! for richer icons. Toggle at runtime with `g`; set permanently with
//! `--nerd-glyphs` / `--no-nerd-glyphs`.

#[derive(Clone, Debug)]
pub struct Glyphs {
    // ── Panel title icons ────────────────────────────────────────────────────
    /// Prefix for the CPU panel title (e.g. "" or "\u{f4bc} ").
    pub cpu_icon: &'static str,
    /// Prefix for the Memory panel title.
    pub mem_icon: &'static str,
    /// Prefix for the Disk I/O panel title.
    pub disk_icon: &'static str,
    /// Prefix for the Network panel title.
    pub net_icon: &'static str,
    /// Prefix for the GPU panel title.
    pub gpu_icon: &'static str,
    /// Prefix for the Processes panel title.
    pub proc_icon: &'static str,

    // ── Process table ────────────────────────────────────────────────────────
    /// Shown next to a process whose thread list is expanded.
    pub expand_open: &'static str,
    /// Shown next to a process with threads that is not yet expanded.
    pub expand_closed: &'static str,
    /// Shown next to a process that has no threads (padding space).
    pub expand_none: &'static str,
    /// Appended to a thread entry when it looks suspicious.
    pub suspicious: &'static str,
    /// The `highlight_symbol` string passed to ratatui's `Table`.
    pub row_cursor: &'static str,

    // ── Network adapter status ────────────────────────────────────────────────
    /// Shown in the UP column when the adapter is active.
    pub net_up: &'static str,
    /// Shown in the UP column when the adapter is down.
    pub net_down: &'static str,
}

impl Glyphs {
    /// Standard Unicode only - works in every terminal.
    pub fn plain() -> Self {
        Self {
            cpu_icon: "",
            mem_icon: "",
            disk_icon: "",
            net_icon: "",
            gpu_icon: "",
            proc_icon: "",
            expand_open: "▼",
            expand_closed: "▶",
            expand_none: " ",
            suspicious: " ⚠",
            row_cursor: "» ",
            net_up: "✓",
            net_down: "✗",
        }
    }

    /// Nerd Font private-use glyphs - requires a Nerd Font to be active in
    /// the terminal (e.g. CaskaydiaCove Nerd Font Mono).
    ///
    /// Codepoints used:
    /// - `\u{f4bc}` nf-md-chip          → CPU
    /// - `\u{f538}` nf-fa-microchip     → Memory
    /// - `\u{f0a0}` nf-fa-hdd_o         → Disk
    /// - `\u{f1eb}` nf-fa-wifi          → Network
    /// - `\u{f085}` nf-fa-cogs          → Processes
    /// - `\u{f078}` nf-fa-chevron_down  → expand open
    /// - `\u{f054}` nf-fa-chevron_right → expand closed
    /// - `\u{f071}` nf-fa-warning       → suspicious thread
    /// - `\u{f00c}` nf-fa-check         → adapter up
    /// - `\u{f00d}` nf-fa-times         → adapter down
    pub fn nerd() -> Self {
        Self {
            cpu_icon:     "\u{f4bc} ",
            mem_icon:     "\u{f538} ",
            disk_icon:    "\u{f0a0} ",
            net_icon:     "\u{f1eb} ",
            gpu_icon:     "\u{f878} ",
            proc_icon:    "\u{f085} ",
            expand_open:  "\u{f078}",
            expand_closed: "\u{f054}",
            expand_none:  " ",
            suspicious:   " \u{f071}",
            row_cursor:   "\u{e0b1} ",
            net_up:       "\u{f00c}",
            net_down:     "\u{f00d}",
        }
    }

    /// Return the appropriate set based on the `nerd_glyphs` flag.
    pub fn for_config(nerd: bool) -> Self {
        if nerd { Self::nerd() } else { Self::plain() }
    }
}
