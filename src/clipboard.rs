//! Win32 clipboard write + context-sensitive copy-text helpers.

use crate::app::InspectTab;
use crate::models::inspect::ProcessInspectData;

/// Write `text` to the Windows clipboard as CF_UNICODETEXT.
/// Silently does nothing on failure.
pub fn write_clipboard(text: &str) {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE,
    };

    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let byte_len = wide.len() * 2;

    unsafe {
        if OpenClipboard(None).is_err() {
            return;
        }
        let _ = EmptyClipboard();

        let hmem = match GlobalAlloc(GMEM_MOVEABLE, byte_len) {
            Ok(h) => h,
            Err(_) => {
                let _ = CloseClipboard();
                return;
            }
        };

        let ptr = GlobalLock(hmem);
        if ptr.is_null() {
            let _ = CloseClipboard();
            return;
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr as *mut u16, wide.len());
        let _ = GlobalUnlock(hmem);

        // CF_UNICODETEXT = 13
        let _ = SetClipboardData(13, HANDLE(hmem.0));
        let _ = CloseClipboard();
    }
}

/// Cursor positions for each inspect tab - passed to `get_copy_text`.
pub struct InspectCursors {
    pub info: usize,
    pub module: usize,
    pub handle: usize,
    pub env: usize,
    pub thread: usize,
    pub network: usize,
}

/// Return the string to copy for the current inspect tab + cursor state.
/// Returns None if there is nothing copyable at that position.
pub fn get_copy_text(
    tab: InspectTab,
    data: &ProcessInspectData,
    cursors: &InspectCursors,
) -> Option<String> {
    match tab {
        InspectTab::Info => {
            let copyable = info_copyable_values(data);
            copyable.get(cursors.info).cloned()
        }
        InspectTab::Threads => {
            data.threads.get(cursors.thread).map(|t| {
                format!("TID:{} module:{} start:0x{:016x}", t.tid, t.start_module, t.start_address)
            })
        }
        InspectTab::Modules => {
            data.modules.get(cursors.module).map(|m| m.path.clone())
        }
        InspectTab::Handles => {
            data.open_handles.get(cursors.handle).map(|h| {
                if h.name.is_empty() {
                    format!("0x{:04x}  {}", h.handle_value, h.type_name)
                } else {
                    h.name.clone()
                }
            })
        }
        InspectTab::Network => {
            data.open_connections.get(cursors.network).map(|c| {
                if c.remote_addr.is_empty() {
                    format!("{} {} {}", c.proto, c.local_addr, c.state)
                } else {
                    format!("{} {} → {} {}", c.proto, c.local_addr, c.remote_addr, c.state)
                }
            })
        }
        InspectTab::Env => {
            data.env_vars.get(cursors.env).map(|(k, v)| format!("{}={}", k, v))
        }
    }
}

/// The ordered list of copyable values on the Info tab.
/// Must stay in sync with the rendering order in `build_info_rows`.
pub fn info_copyable_values(data: &ProcessInspectData) -> Vec<String> {
    let mut out = Vec::new();
    out.push(data.exe_path.clone());
    out.push(data.cmdline.clone());
    if data.parent_pid > 0 {
        out.push(format!("{} (PID {})", data.parent_name, data.parent_pid));
    }
    if let Some(ref v) = data.file_version  { out.push(v.clone()); }
    if let Some(ref v) = data.company_name  { out.push(v.clone()); }
    if let Some(ref v) = data.file_description { out.push(v.clone()); }
    if let Some(ref t) = data.window_title  { out.push(t.clone()); }
    out
}
