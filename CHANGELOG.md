# Changelog

## [0.4.2] - 2026-04-21

### Fixed

- **Phantom second selection in expanded process rows** - when scrolling the cursor into thread sub-rows of an expanded process, a second row highlight appeared on an unrelated process below. Root cause: `is_selected` used `display_row` (process-only counter) instead of the true rendered row index, which includes thread sub-rows.

---

## [0.4.1] - 2026-04-18

### Added

- **`y` copy shortcut** - copies the selected item to the clipboard from the process list, services panel, and all inspect overlay tabs. Status bar and service panel hint updated accordingly.
- **Double-click = Enter** - double-clicking a process row expands/collapses threads inline, matching keyboard <kbd>Enter</kbd> behavior. 400 ms window, same position required.
- **Mouse in services overlay** - scroll wheel navigates the list; left-clicking a row moves the cursor to that service; clicking outside the popup closes it.
- **Mouse in inspect overlay** - scroll wheel moves the per-tab cursor; clicking a tab label (`[ Info ]`, `[ Threads ]`, etc.) switches to that tab; clicking outside the panel closes it.
- **Mouse in settings panel** - scroll wheel moves the settings cursor; clicking a setting row selects and activates it (toggle/cycle); clicking outside the panel closes it.
- **Help overlay** - clicking anywhere closes it.
- **NPU panel toggle** - `show_npu` config field added; NPU panel can be hidden independently via the Settings panel. When either GPU or NPU is hidden the other takes the full panel width automatically.

### Fixed

- **NPU name doubled** - DXGI `Description` buffer contained garbage after the first null byte; `split('\0').next()` now used instead of `trim_end_matches('\0')`, preventing the doubled-name display. Same fix applied to the SetupAPI `read_device_property` helper.
- **Duplicate NPU entries** - SetupAPI PCI-bus scan dedup now checks against all previously found NPU entries (not just DXGI entries), preventing duplicate rows when multiple PCI device nodes point to the same hardware.
- **Settings menu navigation order** - all setting item indices were non-sequential, causing ↑↓ to jump non-linearly through the menu. All 25 items renumbered 0–24 to match visual top-to-bottom order.
- **Tab cycles through hidden panels** - pressing <kbd>Tab</kbd>/<kbd>Shift+Tab</kbd> now skips Disk, Network, and GPU panels when they are hidden.
- **`prev_cpu_pct` HashMap unbounded growth** - the per-PID CPU% tracking map accumulated entries for dead processes and never shrank. Now rebuilt from the live process list each update cycle.
- **SCM handle leak on error** - `EnumServicesStatusExW` failure path now explicitly closes the SCM handle before returning instead of leaking it via `?`.
- **Stacked layout disk/network not side-by-side** - compact and stacked layouts now split Disk and Network into a 50/50 horizontal row, completing the 0.4.0 intent ("all layout modes"). Network adapter name column min-width reduced 28 → 10 to fit the narrower half.
- **Column sort click off by selection gutter** - `col_sort_field_at_x` now subtracts the 2-char selection highlight before mapping click X to a column, and adds 1-char inter-column spacing between each pair. Previously clicking CPU% could land on Name, etc.

### Changed

- **Nerd Font glyphs updated** - Memory icon changed from `nf-fa-microchip` (`\u{f538}`) to `nf-fa-memory` (`\u{efc5}`); GPU icon changed from `\u{f878}` to `nf-md-monitor_screenshot` (`\u{f0e51}`); NPU panel now uses `nf-fa-robot` (`\u{ee0d}`). All panel icons now carry a double trailing space for consistent padding.
- **Statusbar key hints** - `u:UserFilter` and `K:Kill` tips swapped to match the logical frequency order; `y:Copy` tip added.
- **Services start-type caching** - `query_start_type` (one `OpenServiceW` + `QueryServiceConfigW` per service) is now called once per service name and cached for the process lifetime. Eliminates ~300+ redundant Win32 calls on every 5-second services poll on a typical Windows system.
- **GPU `collect()` allocations** - throwaway adapters returned by `GpuCollector::collect()` now use `RingBuffer::new(0)` (no heap allocation). New-adapter initialization in the merge loop creates the properly-sized ring buffer directly.
- **`services_filter_lower` cached** - services filter lowercase is now pre-computed on each keystroke and stored alongside the raw string, eliminating repeated `to_lowercase()` allocations at every use site.

---

## [0.4.0] - 2026-04-18

### Added

- **8-level heat gradient** - CPU, memory, disk, and GPU gauges now use a continuous color scale: Dark Blue → Cyan → Green → Yellow-Green → Yellow → Orange → Red-Orange → Crimson. Replaces the old 3-level low/medium/high system.
- **Memory sparkline** - RAM utilization history displayed as a full-width sparkline when the memory panel is tall enough.
- **Disk sparkline** - per-drive utilization history sparkline in the UTIL% column, replacing the static block bar.
- **Mouse support** - left-click to focus any panel, click column headers in the process table to sort (click again to reverse), scroll wheel to navigate the process list.
- **Column header sort** - clicking a process table column header sets that sort field; clicking the active column toggles ascending/descending.
- **Column visibility in Settings** (`C`) - 10 individual column toggles added under a new `── Columns` section (PID, NAME, CPU%, MEM, MEM%, THDS, STATUS, USER, DISK-R, DISK-W).
- **Services panel** (`v`) - full-screen overlay listing all Windows services via Win32 SCM. Shows status (color-coded), name, display name, start type, and PID. Filter-as-you-type with any letter key; Backspace to erase; Esc to close.
- **GPU panel** (`Shift+G`) - new panel integrated into the main layout showing GPU adapter name, utilization % with sparkline history, and VRAM used/total. Uses DXGI for adapter enumeration and PDH counters for live utilization and VRAM usage. Togglable via `Shift+G` or the Settings panel.
- **NPU detection** - when an NPU is present the GPU panel splits 50/50 horizontally, showing a GPU table on the left and an NPU table (name + utilization sparkline) on the right. Detects AMD XDNA, Intel NPU (Meteor Lake "AI Boost"), Qualcomm Hexagon, AMD Ryzen AI, VPU-class devices, and anything branded "npu" / "neural". AMD XDNA (invisible to DXGI) is discovered via a supplemental SetupAPI PCI-bus scan at startup.
- **Inspect handles - extended name resolution** - registry keys, section objects, events, mutants, semaphores, symbolic links, and directories now show resolved names via `NtQueryObject`. Registry paths are normalized (`\REGISTRY\MACHINE\` → `HKLM\`, etc.).

### Fixed

- **CPU% capped at 100%** - process CPU% was previously reported per-core (Linux `top` style, could exceed 100% on multi-core). Now normalized to logical core count, matching Windows Task Manager.
- **Column header sort click alignment** - clicking a process table column header now correctly maps the click x-coordinate to the right column. Previously the click target was shifted left by the selection-highlight width, so clicking CPU% would sort by Name, etc.
- **UTIL% overflow at 100%** - sparkline percentage column no longer clips the `%` symbol when utilization hits 100%.

### Changed

- `g` (Nerd Font glyphs) now requires `KeyModifiers::NONE`; `Shift+G` is the new GPU panel toggle.
- Settings panel height increased to 36 to accommodate the Columns section.
- `LayoutRects` gains an optional `gpu` field; `layout::compute` takes a `show_gpu` parameter.
- **Side-by-side panel layout** - Disk I/O and Network panels now share one row split 50/50 horizontally in all layout modes (previously stacked vertically in non-wide mode). GPU and NPU panels also split 50/50 when an NPU is detected.

---

## [0.3.0] - 2026-04-17

### Added

- **Tree view** (`t`) - toggle parent/child process hierarchy with `└` connectors and indent levels. Cursor, kill, inspect, and expand all work correctly in tree mode.
- **Inspect - Threads tab** - second tab in the 6-tab process overlay shows all threads with TID, base priority, live CPU%, wait state, and start module. Suspicious threads (unmapped start address) are highlighted.
- **Inspect - Network cursor** - the Network tab in the inspect overlay now has a selectable cursor and supports `y` to copy a connection line to the clipboard.
- **Name search** (`/`) - jump-to-process overlay for partial name match. `Enter` moves the cursor to the first match; `Esc` cancels.
- **Tree view toggle in Settings panel** (`C`) - Tree View row added under the Processes section.

### Changed

- Removed `j`/`k` vim-style navigation bindings. Arrow keys only.

### Internal

- `handle_key` now takes a `ModalState` struct instead of 11 boolean arguments.
- `get_copy_text` now takes an `InspectCursors` struct instead of 6 cursor arguments.
- Zero `cargo clippy` warnings.

---

## [0.2.0] - 2026-04-16

### Added

- **File-based theme system** - themes are now TOML files in `%APPDATA%\wtop\themes\`. Built-ins are embedded in the binary and exported on first launch as editable examples. Drop any `.toml` in the directory; it appears in the cycle immediately with no restart required.
- **Live theme hot-reload** - if you edit a theme file while wtop is running, changes apply on the next tick.
- **4 new built-in themes** - `cyberpunk`, `monokai`, `one_dark`, `solarized_dark` (11 total).
- **`--list-themes`** - prints all available themes with display name, author, version, and description, then exits.
- **`--export-themes`** - re-exports built-in themes to the themes directory (overwrites), useful for resetting edits.
- **Theme attribution in settings panel** - author and homepage shown under the active theme name.
- **`themes/` directory** in the repository with all 11 built-in TOML sources and a full schema reference (`themes/README.md`).

### Changed

- `config.theme` is now a free-form slug string instead of a `ThemeName` enum. Old configs with PascalCase values (`Dark`, `CatppuccinMocha`, etc.) are automatically migrated on load.
- `--theme` CLI flag accepts any slug - unknown names fall back to `dark` gracefully.
- `GaugeStyle` is now serialized to `config.toml` so gauge style set via a theme persists.
- Settings panel height increased to accommodate theme attribution row.

### Fixed

- Theme cycle (`Shift+T`) now rescans the themes directory on each press so files added since startup appear immediately.

---

## [0.1.1] - 2025-xx-xx

See git log for details.

## [0.1.0] - initial release
