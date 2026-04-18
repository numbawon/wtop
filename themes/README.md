# wtop themes

wtop supports file-based themes written in TOML. Built-in themes are embedded in the binary and exported to your themes directory on first launch so you have working examples to copy and edit.

## Theme directory

| Platform | Path |
|----------|------|
| Windows  | `%APPDATA%\wtop\themes\` |
| Fallback | `./themes/` (next to the binary) |

Drop any `.toml` file in this directory and it will appear in the theme cycle (`T` key) immediately - no restart required. The file is live-reloaded whenever it changes on disk.

## Built-in themes

| Slug | Name | Author |
|------|------|--------|
| `dark` | Dark | Chad Collins |
| `light` | Light | Chad Collins |
| `catppuccin_mocha` | Catppuccin Mocha | Catppuccin Org |
| `cyberpunk` | Cyberpunk | Chad Collins |
| `dracula` | Dracula | Zeno Rocha |
| `gruvbox` | Gruvbox | Pavel Pertsev (morhetz) |
| `monokai` | Monokai | Wimer Hazenberg |
| `nord` | Nord | Arctic Ice Studio (Sven Greb) |
| `one_dark` | One Dark | Bram de Haan |
| `solarized_dark` | Solarized Dark | Ethan Schoonover |
| `tokyo_night` | Tokyo Night | Folke Lemaitre (folke) |

## TOML schema

```toml
# ── Metadata (all optional, shown by --list-themes and in the settings panel) ──
name        = "My Theme"
author      = "Your Name"
version     = "1.0"
description = "Short description."
homepage    = "https://example.com"

# ── Appearance ──────────────────────────────────────────────────────────────────
border_style = "plain"     # plain | rounded | thick | double
gauge_style  = "block"     # block | line | segmented | ascii
spark_chars  = "unicode"   # unicode | ascii

# panel_bg - set an explicit background color for overlay panels and table rows.
# Required for light themes so black text is readable on any terminal background.
# panel_bg = "white"

# ── Named color aliases (reference by name in [colors]) ─────────────────────────
[palette]
my_blue  = "#61afef"
my_green = "#98c379"
# Any name you like; hex only in [palette].

# ── Role assignments ────────────────────────────────────────────────────────────
[colors]
# Borders & titles
border           = "my_blue"    # palette key, hex (#rrggbb / #rgb), or terminal name
border_focused   = "my_blue"
title            = "my_blue"
header           = "yellow"     # terminal color name

# Gauge bars (CPU, memory, disk)
gauge_low        = "green"
gauge_medium     = "yellow"
gauge_high       = "red"

# Process table rows
row_bg           = "white"      # optional: explicit bg for normal/thread/spike rows
                                # (required for light themes)
row_normal_fg    = "my_blue"
row_zebra_fg     = "my_blue"
row_zebra_bg     = "#21252b"    # inline hex is fine too
row_selected_fg  = "black"
row_selected_bg  = "my_blue"
row_thread       = "dark_gray"
row_suspicious   = "red"
row_spike        = "orange"

# Process status column
status_running   = "green"
status_suspended = "yellow"
status_other     = "dark_gray"

# Filter bar
filter_active_fg = "black"
filter_active_bg = "yellow"
filter_inactive  = "dark_gray"

# General text
text_dim         = "dark_gray"
text_normal      = "white"
text_bright      = "my_blue"

# Sparkline gradient
spark_low        = "green"
spark_mid        = "yellow"
spark_high       = "red"
```

### Accepted color formats

| Format | Example | Notes |
|--------|---------|-------|
| 6-digit hex | `#61afef` | Most precise |
| 3-digit hex | `#6af` | Expands to `#66aaff` |
| Terminal name | `cyan` | Uses terminal's palette |

**Terminal color names:** `black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`,
`gray`, `dark_gray`, `white`, `light_red`, `light_green`, `light_yellow`, `light_blue`,
`light_magenta`, `light_cyan`, `reset`

### Tips

- All `[colors]` fields are optional - any omitted field falls back to the built-in dark theme equivalent.
- Use `[palette]` to define color aliases and keep hex values DRY across roles.
- Use `panel_bg` and `row_bg` together when making a light theme (see `light.toml`).
- Use `--list-themes` to see all available themes with their metadata.
- Use `--export-themes` to (re-)export the built-in themes if you want a fresh starting point.
