# Changelog

## [0.2.0] — 2026-04-16

### Added

- **File-based theme system** — themes are now TOML files in `%APPDATA%\wtop\themes\`. Built-ins are embedded in the binary and exported on first launch as editable examples. Drop any `.toml` in the directory; it appears in the cycle immediately with no restart required.
- **Live theme hot-reload** — if you edit a theme file while wtop is running, changes apply on the next tick.
- **4 new built-in themes** — `cyberpunk`, `monokai`, `one_dark`, `solarized_dark` (11 total).
- **`--list-themes`** — prints all available themes with display name, author, version, and description, then exits.
- **`--export-themes`** — re-exports built-in themes to the themes directory (overwrites), useful for resetting edits.
- **Theme attribution in settings panel** — author and homepage shown under the active theme name.
- **`themes/` directory** in the repository with all 11 built-in TOML sources and a full schema reference (`themes/README.md`).

### Changed

- `config.theme` is now a free-form slug string instead of a `ThemeName` enum. Old configs with PascalCase values (`Dark`, `CatppuccinMocha`, etc.) are automatically migrated on load.
- `--theme` CLI flag accepts any slug — unknown names fall back to `dark` gracefully.
- `GaugeStyle` is now serialized to `config.toml` so gauge style set via a theme persists.
- Settings panel height increased to accommodate theme attribution row.

### Fixed

- Theme cycle (`Shift+T`) now rescans the themes directory on each press so files added since startup appear immediately.

---

## [0.1.1] — 2025-xx-xx

See git log for details.

## [0.1.0] — initial release
