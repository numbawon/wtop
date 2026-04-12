//! Windows Terminal integration — detect WT_SESSION, read/write settings.json.
//!
//! Phase 3A: detects whether wtop is running inside Windows Terminal, resolves
//! the current profile and its font face, and can write a Nerd Font face back to
//! the settings file.
//!
//! NOTE: Windows Terminal's settings.json is JSONC (JSON with C-style comments
//! and trailing commas). This module strips `//` and `/* */` comments before
//! parsing. Trailing commas are not stripped — most default WT settings files
//! do not use them, but if yours does, the write-back will fail gracefully.

use std::path::{Path, PathBuf};

use serde_json::Value;

/// The Nerd Font face we recommend and write into WT settings.
pub const NERD_FONT_FACE: &str = "CaskaydiaCove Nerd Font Mono";

/// Snapshot of Windows Terminal environment, resolved once at startup.
#[derive(Debug, Clone, Default)]
pub struct WtInfo {
    /// True when `WT_SESSION` env var is present (we're inside Windows Terminal).
    pub detected: bool,
    /// Raw value of `WT_PROFILE_ID` (GUID string or profile name).
    pub profile_id: Option<String>,
    /// Human-readable profile name resolved from settings.json.
    pub profile_name: Option<String>,
    /// Font face for the current profile (falls back to `profiles.defaults`).
    pub font_face: Option<String>,
    /// Absolute path to the settings.json that was found and read.
    pub settings_path: Option<PathBuf>,
    /// Non-fatal error encountered while reading/parsing settings.json.
    pub read_error: Option<String>,
}

impl WtInfo {
    /// Detect Windows Terminal presence and read relevant settings.
    pub fn detect() -> Self {
        let mut info = WtInfo {
            detected: std::env::var("WT_SESSION").is_ok(),
            profile_id: std::env::var("WT_PROFILE_ID").ok(),
            ..Default::default()
        };

        if !info.detected {
            return info;
        }

        info.settings_path = find_settings_path();

        if let Some(ref path) = info.settings_path.clone() {
            match read_settings(path) {
                Ok(settings) => {
                    let (name, font) = resolve_profile(&settings, info.profile_id.as_deref());
                    info.profile_name = name;
                    info.font_face = font;
                }
                Err(e) => info.read_error = Some(e),
            }
        }

        info
    }

    /// True if the current font already looks like it includes Nerd Font glyphs.
    pub fn has_nerd_font(&self) -> bool {
        self.font_face
            .as_deref()
            .map(|f| {
                let lower = f.to_lowercase();
                lower.contains("nerd") || lower.contains(" nf") || lower.ends_with("nf")
            })
            .unwrap_or(false)
    }

    /// Write `NERD_FONT_FACE` into the matched profile (or `profiles.defaults`)
    /// in the settings.json. Returns a user-facing error string on failure.
    pub fn apply_nerd_font(&self) -> Result<(), String> {
        let path = self
            .settings_path
            .as_ref()
            .ok_or_else(|| "No Windows Terminal settings.json found".to_string())?;

        let mut settings = read_settings(path)?;

        // Try to find the exact profile by GUID / name first.
        let mut found = false;
        if let Some(id) = &self.profile_id {
            if let Some(list) = settings.pointer_mut("/profiles/list") {
                if let Some(arr) = list.as_array_mut() {
                    for p in arr.iter_mut() {
                        let matches = p["guid"].as_str() == Some(id.as_str())
                            || p["name"].as_str() == Some(id.as_str());
                        if matches {
                            set_font_face(p, NERD_FONT_FACE);
                            found = true;
                            break;
                        }
                    }
                }
            }
        }

        // Fall back to profiles.defaults so the change applies everywhere.
        if !found {
            match settings.pointer_mut("/profiles/defaults") {
                Some(defaults) => set_font_face(defaults, NERD_FONT_FACE),
                None => {
                    // Create the path if it doesn't exist.
                    if let Some(profiles) = settings.pointer_mut("/profiles") {
                        if let Some(obj) = profiles.as_object_mut() {
                            obj.entry("defaults")
                                .or_insert_with(|| serde_json::json!({}));
                        }
                    }
                    if let Some(defaults) = settings.pointer_mut("/profiles/defaults") {
                        set_font_face(defaults, NERD_FONT_FACE);
                    }
                }
            }
        }

        write_settings(path, &settings)
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Search the known installation paths for settings.json, returning the first
/// that exists: stable packaged → preview packaged → unpackaged sideload.
fn find_settings_path() -> Option<PathBuf> {
    let local = std::env::var("LOCALAPPDATA").ok()?;
    let packages = PathBuf::from(&local).join("Packages");

    let candidates = [
        packages
            .join("Microsoft.WindowsTerminal_8wekyb3d8bbwe")
            .join("LocalState")
            .join("settings.json"),
        packages
            .join("Microsoft.WindowsTerminalPreview_8wekyb3d8bbwe")
            .join("LocalState")
            .join("settings.json"),
        PathBuf::from(&local)
            .join("Microsoft")
            .join("Windows Terminal")
            .join("settings.json"),
    ];

    candidates.into_iter().find(|p| p.exists())
}

/// Read and parse settings.json, stripping JSONC comments first.
fn read_settings(path: &Path) -> Result<Value, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
    let stripped = strip_jsonc_comments(&raw);
    serde_json::from_str(&stripped).map_err(|e| format!("JSON parse error: {e}"))
}

/// Write a `Value` back to `path` as pretty-printed JSON.
fn write_settings(path: &Path, value: &Value) -> Result<(), String> {
    let json =
        serde_json::to_string_pretty(value).map_err(|e| format!("Serialize error: {e}"))?;
    std::fs::write(path, json).map_err(|e| format!("Write error: {e}"))
}

/// Strip `//` line comments and `/* */` block comments from JSONC text.
/// Preserves newlines so JSON parse errors report correct line numbers.
fn strip_jsonc_comments(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_string = false;
    let mut escaped = false;
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if escaped {
            out.push(ch);
            escaped = false;
            continue;
        }
        if in_string {
            if ch == '\\' {
                escaped = true;
                out.push(ch);
            } else if ch == '"' {
                in_string = false;
                out.push(ch);
            } else {
                out.push(ch);
            }
            continue;
        }
        // Outside a string literal.
        if ch == '"' {
            in_string = true;
            out.push(ch);
        } else if ch == '/' && chars.peek() == Some(&'/') {
            // Line comment — skip to end of line, preserve the newline.
            for c in chars.by_ref() {
                if c == '\n' {
                    out.push('\n');
                    break;
                }
            }
        } else if ch == '/' && chars.peek() == Some(&'*') {
            // Block comment — skip to closing `*/`, preserve newlines.
            chars.next(); // consume '*'
            let mut prev = ' ';
            for c in chars.by_ref() {
                if prev == '*' && c == '/' {
                    break;
                }
                if c == '\n' {
                    out.push('\n');
                }
                prev = c;
            }
        } else {
            out.push(ch);
        }
    }
    out
}

/// Given the parsed settings `Value` and an optional profile GUID/name,
/// return `(profile_name, font_face)`. Falls back to `profiles.defaults` font.
fn resolve_profile(settings: &Value, profile_id: Option<&str>) -> (Option<String>, Option<String>) {
    let default_font = settings
        .pointer("/profiles/defaults/font/face")
        .and_then(Value::as_str)
        .map(String::from);

    if let Some(id) = profile_id {
        if let Some(list) = settings.pointer("/profiles/list").and_then(Value::as_array) {
            for profile in list {
                let matches = profile["guid"].as_str() == Some(id)
                    || profile["name"].as_str() == Some(id);
                if matches {
                    let name = profile["name"].as_str().map(String::from);
                    let font = profile
                        .pointer("/font/face")
                        .and_then(Value::as_str)
                        .map(String::from)
                        .or(default_font);
                    return (name, font);
                }
            }
        }
    }

    (None, default_font)
}

/// Set `target["font"]["face"] = face`, creating intermediate objects as needed.
fn set_font_face(target: &mut Value, face: &str) {
    if let Some(obj) = target.as_object_mut() {
        let font = obj
            .entry("font".to_string())
            .or_insert_with(|| serde_json::json!({}));
        if let Some(font_obj) = font.as_object_mut() {
            font_obj.insert("face".to_string(), Value::String(face.to_string()));
        }
    }
}
