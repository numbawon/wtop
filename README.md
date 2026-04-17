<div align="center">

# wtop

<strong>htop for Windows. In your terminal. No install.</strong>

<br>

![Platform](https://img.shields.io/badge/platform-Windows-blue)
![Rust](https://img.shields.io/badge/built%20with-Rust-orange)
![License](https://img.shields.io/badge/license-MIT-green)

<br>

![wtop main view](docs/screenshots/main.png)

</div>

<br>

Another tool in the belt. When Task Manager is too noisy and Process Explorer isn't installed, this is what you reach for. Already in a terminal — stay there.

<br>

<h2>What it shows</h2>

| Panel | |
|-------|-|
| **CPU** | Per-core usage with sparkline history |
| **Memory** | RAM and commit charge with history |
| **Disk** | Read/write bytes per second, utilization per physical disk |
| **Network** | Rx/Tx per adapter, live |
| **Processes** | Sortable — CPU%, memory, threads, status, owner |

<br>

Hit <kbd>Enter</kbd> on any process to expand it and see its threads.

![Thread expansion](docs/screenshots/threads_expanded.png)

Each thread shows what it's actually waiting on — `Sleep`, `Mutex`, `LpcReceive`, `Queue`, not just "Waiting". Start address resolves to a module name; anything that doesn't map to a loaded module gets flagged.

<br>

<h2>Why</h2>

- Something is eating CPU and you need to know which *thread*
- You want disk I/O without pulling up Sysinternals
- You're in the terminal already — stay there
- Kill something and confirm it's gone without switching windows

Usernames come from the Windows token API directly — real names, not SID strings. Run as Administrator to see more. Some processes (antimalware, lsass) are PPL and will always show `?` — that's Windows, not a bug.

<br>

<h2>Build</h2>

Rust stable, Windows x86-64.

<pre>
cargo build --release
</pre>

One binary — `target\release\wtop.exe`. Copy it wherever.

<pre>
wtop                                                  # defaults
wtop --interval 500 --theme gruvbox --nerd-glyphs     # faster, themed, with icons
wtop --ascii                                          # basic terminal or CI
</pre>

<br>

<h2>Options</h2>

| Flag | Default | |
|------|---------|---|
| `-i, --interval <ms>` | `1000` | Refresh rate in ms · 250–5000 |
| `-t, --theme <name>` | `dark` | Color theme slug |
| `--nerd-glyphs` | | Enable Nerd Font icons (auto-detected in Windows Terminal) |
| `--no-nerd-glyphs` | | Force off |
| `--ascii` | | ASCII-only borders and sparklines |
| `--list-themes` | | Print all available themes with author info and exit |
| `--export-themes` | | Re-export built-in themes to the themes directory and exit |
| `--log-level <lvl>` | `warn` | `off` · `error` · `warn` · `info` · `debug` · `trace` |

Logs go to `%TEMP%\wtop.log`.

<br>

<h2>Keys</h2>

<details>
<summary><strong>Navigation</strong></summary>
<br>

| Key | |
|-----|---|
| <kbd>↑</kbd> <kbd>k</kbd> &nbsp;/&nbsp; <kbd>↓</kbd> <kbd>j</kbd> | Move up / down |
| <kbd>PgUp</kbd> / <kbd>PgDn</kbd> | Jump 20 rows |
| <kbd>Home</kbd> / <kbd>End</kbd> | Top / bottom |
| <kbd>Tab</kbd> / <kbd>Shift</kbd><kbd>Tab</kbd> | Cycle panel focus |
| <kbd>Enter</kbd> | Expand / collapse threads |

</details>

<details>
<summary><strong>Filtering &amp; sorting</strong></summary>
<br>

| Key | |
|-----|---|
| <kbd>f</kbd> | Open name filter — type to search, <kbd>Esc</kbd> to close |
| <kbd>p</kbd> | Toggle system processes |
| <kbd>u</kbd> | Show only your processes |
| <kbd>s</kbd> / <kbd>Shift</kbd><kbd>S</kbd> | Next / prev sort column |
| <kbd>r</kbd> | Flip sort order |

</details>

<details>
<summary><strong>Actions</strong></summary>
<br>

| Key | |
|-----|---|
| <kbd>Shift</kbd><kbd>K</kbd> | Kill selected process (asks first) |
| <kbd>+</kbd> / <kbd>-</kbd> | Faster / slower refresh |
| <kbd>q</kbd> / <kbd>Ctrl</kbd><kbd>C</kbd> | Quit |

</details>

<details>
<summary><strong>Display</strong></summary>
<br>

| Key | |
|-----|---|
| <kbd>Shift</kbd><kbd>L</kbd> | Cycle layout |
| <kbd>Shift</kbd><kbd>T</kbd> | Cycle theme |
| <kbd>d</kbd> | Toggle disk panel |
| <kbd>n</kbd> | Toggle network panel |
| <kbd>c</kbd> | Toggle disk I/O columns |
| <kbd>g</kbd> | Toggle Nerd Font glyphs |
| <kbd>w</kbd> | Windows Terminal panel |
| <kbd>?</kbd> <kbd>h</kbd> | Help overlay |

</details>

<br>

![Filter and kill confirm](docs/screenshots/filter_kill.png)

<br>

<h2>Themes</h2>

`--theme <name>` at launch, or cycle at runtime with <kbd>Shift</kbd><kbd>T</kbd>.

`dark` · `light` · `catppuccin_mocha` · `cyberpunk` · `dracula` · `gruvbox` · `monokai` · `nord` · `one_dark` · `solarized_dark` · `tokyo_night`

Themes are TOML files in `%APPDATA%\wtop\themes\`. Built-ins are exported there on first launch — copy and edit to make your own. Drop any `.toml` in the directory and it appears in the cycle immediately, live-reloaded as you edit. See [`themes/README.md`](themes/README.md) for the full schema.

![Themes](docs/screenshots/themes.gif)

<br>

<h2>Layouts</h2>

Cycle with <kbd>Shift</kbd><kbd>L</kbd>.

| | |
|-|-|
| **Auto** | Wide if the terminal is wide enough, compact otherwise |
| **Wide** | All panels side by side above the process list |
| **Compact** | Panels stacked left, process list right |
| **Stacked** | Single column — process list gets the most room |

![Layouts](docs/screenshots/layouts.png)

<br>

<h2>Windows Terminal</h2>

Press <kbd>w</kbd> to open the WT panel. If you haven't set a Nerd Font yet, wtop can write the setting — press <kbd>f</kbd>, confirm, restart WT.

![Windows Terminal panel](docs/screenshots/wt_panel.png)

<br>

<h2>License</h2>

MIT
