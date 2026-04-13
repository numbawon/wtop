//! On-demand collector for the process detail overlay (Group B).
//!
//! Called synchronously from the UI thread when the user presses `i`.
//! All operations complete in well under a frame budget — the only
//! potentially slow call is GetFileVersionInfoW which reads the PE
//! version resource, but Windows caches this after the first read.

use windows::Win32::Foundation::{CloseHandle, FILETIME, HANDLE, HMODULE};
use windows::Win32::System::ProcessStatus::{
    EnumProcessModules, GetModuleFileNameExW, GetModuleInformation, MODULEINFO,
};
use windows::Win32::System::Threading::{
    GetProcessTimes, OpenProcess, QueryFullProcessImageNameW,
    PROCESS_NAME_WIN32, PROCESS_QUERY_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_VM_READ,
};
use windows::core::PWSTR;

use crate::models::inspect::{ModuleEntry, ProcessInspectData};

/// Collect all available detail for `pid`. Never panics; fields that cannot
/// be read (access denied, kernel process, etc.) are filled with `"?"` or `None`.
pub fn collect_inspect(pid: u32, name: &str) -> ProcessInspectData {
    let exe_path = get_exe_path(pid).unwrap_or_else(|| "?".into());
    let cmdline = get_cmdline(pid).unwrap_or_else(|| "?".into());
    let uptime_secs = get_uptime_secs(pid).unwrap_or(0);
    let (file_version, product_version, company_name, file_description) =
        if exe_path != "?" {
            get_version_strings(&exe_path)
        } else {
            (None, None, None, None)
        };

    let modules = get_modules(pid);

    ProcessInspectData {
        pid,
        name: name.to_string(),
        exe_path,
        cmdline,
        uptime_secs,
        file_version,
        product_version,
        company_name,
        file_description,
        modules,
    }
}

// ── Loaded modules ────────────────────────────────────────────────────────────

fn get_modules(pid: u32) -> Vec<ModuleEntry> {
    let proc = match unsafe {
        OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid)
            .or_else(|_| OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ, false, pid))
            .ok()
    } {
        Some(h) => h,
        None => return Vec::new(),
    };

    const MAX_MODS: usize = 1024;
    let mut handles = vec![HMODULE::default(); MAX_MODS];
    let mut needed: u32 = 0;

    let ok = unsafe {
        EnumProcessModules(
            proc,
            handles.as_mut_ptr(),
            (handles.len() * std::mem::size_of::<HMODULE>()) as u32,
            &mut needed,
        )
        .is_ok()
    };

    if !ok {
        unsafe { let _ = CloseHandle(proc); }
        return Vec::new();
    }

    let count = (needed as usize / std::mem::size_of::<HMODULE>()).min(MAX_MODS);
    let mut entries = Vec::with_capacity(count);

    for &module in &handles[..count] {
        let mut info = MODULEINFO {
            lpBaseOfDll: std::ptr::null_mut(),
            SizeOfImage: 0,
            EntryPoint: std::ptr::null_mut(),
        };
        let has_info = unsafe {
            GetModuleInformation(
                proc,
                module,
                &mut info,
                std::mem::size_of::<MODULEINFO>() as u32,
            )
            .is_ok()
        };

        let mut name_buf = [0u16; 260];
        let name_len =
            unsafe { GetModuleFileNameExW(proc, module, &mut name_buf) as usize };

        let path = if name_len > 0 {
            String::from_utf16_lossy(&name_buf[..name_len])
        } else {
            "?".into()
        };
        let name = path
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or("?")
            .to_string();

        entries.push(ModuleEntry {
            name,
            path,
            base: if has_info { info.lpBaseOfDll as u64 } else { 0 },
            size: if has_info { info.SizeOfImage } else { 0 },
        });
    }

    unsafe { let _ = CloseHandle(proc); }
    entries
}

// ── Exe path ─────────────────────────────────────────────────────────────────

fn get_exe_path(pid: u32) -> Option<String> {
    let proc = unsafe {
        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
            .or_else(|_| OpenProcess(PROCESS_QUERY_INFORMATION, false, pid))
            .ok()?
    };
    let mut buf = [0u16; 1024];
    let mut len = 1024u32;
    let ok = unsafe {
        QueryFullProcessImageNameW(proc, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len)
            .is_ok()
    };
    unsafe { let _ = CloseHandle(proc); }
    if ok && len > 0 {
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    } else {
        None
    }
}

// ── Command line ──────────────────────────────────────────────────────────────

fn get_cmdline(pid: u32) -> Option<String> {
    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    use windows::core::PCSTR;

    // NtQueryInformationProcess(ProcessCommandLineInformation = 60) returns a
    // UNICODE_STRING identical in layout to what query_thread_name reads.
    type NtQueryInformationProcessFn = unsafe extern "system" fn(
        HANDLE,
        u32,
        *mut std::ffi::c_void,
        u32,
        *mut u32,
    ) -> i32;

    let ntdll = unsafe { GetModuleHandleW(windows::core::w!("ntdll.dll")).ok()? };
    let fn_ptr = unsafe {
        GetProcAddress(ntdll, PCSTR(b"NtQueryInformationProcess\0".as_ptr()))
    }?;
    // Safety: same transmute pattern used throughout this codebase for NT
    // functions. The calling convention and parameter layout match the
    // documented NtQueryInformationProcess signature.
    let nt_query: NtQueryInformationProcessFn = unsafe { std::mem::transmute(fn_ptr) };

    let proc = unsafe {
        OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid)
            .or_else(|_| OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid))
            .ok()?
    };

    let mut buf = vec![0u8; 4096];
    let mut ret_len: u32 = 0;
    let status = unsafe {
        nt_query(proc, 60, buf.as_mut_ptr() as *mut _, buf.len() as u32, &mut ret_len)
    };
    unsafe { let _ = CloseHandle(proc); }

    if status != 0 {
        return None;
    }

    // UNICODE_STRING on x64: Length(u16), MaximumLength(u16), _pad(u32), Buffer(*mut u16)
    if buf.len() < 16 {
        return None;
    }
    let length_bytes = u16::from_le_bytes([buf[0], buf[1]]) as usize;
    if length_bytes == 0 {
        return None;
    }
    let buf_ptr = usize::from_le_bytes(buf[8..16].try_into().ok()?);
    let base = buf.as_ptr() as usize;
    if buf_ptr < base || buf_ptr.saturating_add(length_bytes) > base + buf.len() {
        return None;
    }
    let str_offset = buf_ptr - base;
    let str_end = str_offset + length_bytes;
    let chars: Vec<u16> = buf[str_offset..str_end]
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let s = String::from_utf16_lossy(&chars);
    if s.is_empty() { None } else { Some(s) }
}

// ── Uptime ────────────────────────────────────────────────────────────────────

fn get_uptime_secs(pid: u32) -> Option<u64> {
    let proc = unsafe {
        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?
    };
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    let ok = unsafe {
        GetProcessTimes(proc, &mut creation, &mut exit, &mut kernel, &mut user).is_ok()
    };
    unsafe { let _ = CloseHandle(proc); }
    if !ok {
        return None;
    }

    // FILETIME = 100-nanosecond intervals since 1601-01-01 00:00:00 UTC.
    let creation_100ns =
        ((creation.dwHighDateTime as u64) << 32) | creation.dwLowDateTime as u64;
    let creation_secs = creation_100ns / 10_000_000;
    const WIN_EPOCH_TO_UNIX: u64 = 11_644_473_600; // seconds between 1601 and 1970
    if creation_secs < WIN_EPOCH_TO_UNIX {
        return None;
    }
    let creation_unix = creation_secs - WIN_EPOCH_TO_UNIX;

    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();

    now_unix.checked_sub(creation_unix)
}

// ── Version info (GetFileVersionInfoW / VerQueryValueW via version.dll) ───────

fn get_version_strings(
    exe_path: &str,
) -> (Option<String>, Option<String>, Option<String>, Option<String>) {
    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
    use windows::core::PCSTR;

    type SizeFn = unsafe extern "system" fn(*const u16, *mut u32) -> u32;
    type InfoFn = unsafe extern "system" fn(*const u16, u32, u32, *mut u8) -> i32;
    type QueryFn =
        unsafe extern "system" fn(*const u8, *const u16, *mut *const u8, *mut u32) -> i32;

    // Prefer an already-loaded handle; fall back to loading the DLL.
    // version.dll is tiny (~50 KB) and always present — we intentionally
    // skip FreeLibrary and let the OS clean up on process exit.
    let lib_handle = unsafe {
        GetModuleHandleW(windows::core::w!("version.dll"))
            .ok()
            .or_else(|| LoadLibraryW(windows::core::w!("version.dll")).ok())
    };
    let lib = match lib_handle {
        Some(h) => h,
        None => return (None, None, None, None),
    };

    macro_rules! load_fn {
        ($name:literal, $ty:ty) => {
            match unsafe { GetProcAddress(lib, PCSTR(concat!($name, "\0").as_ptr())) } {
                Some(p) => unsafe { std::mem::transmute::<_, $ty>(p) },
                None => return (None, None, None, None),
            }
        };
    }

    let get_size: SizeFn = load_fn!("GetFileVersionInfoSizeW", SizeFn);
    let get_info: InfoFn = load_fn!("GetFileVersionInfoW", InfoFn);
    let query_val: QueryFn = load_fn!("VerQueryValueW", QueryFn);

    let path_wide: Vec<u16> = exe_path
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let mut dummy: u32 = 0;
    let size = unsafe { get_size(path_wide.as_ptr(), &mut dummy) };
    if size == 0 {
        return (None, None, None, None);
    }

    let mut info = vec![0u8; size as usize];
    if unsafe { get_info(path_wide.as_ptr(), 0, size, info.as_mut_ptr()) } == 0 {
        return (None, None, None, None);
    }

    // Read the translation table to get the language/codepage pairs.
    let trans_key: Vec<u16> = r"\VarFileInfo\Translation"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut trans_ptr: *const u8 = std::ptr::null();
    let mut trans_len: u32 = 0;

    let translations: Vec<(u16, u16)> =
        if unsafe {
            query_val(
                info.as_ptr(),
                trans_key.as_ptr(),
                &mut trans_ptr,
                &mut trans_len,
            )
        } != 0
            && !trans_ptr.is_null()
            && trans_len >= 4
        {
            (0..(trans_len as usize / 4))
                .map(|i| unsafe {
                    let lang = *(trans_ptr as *const u16).add(i * 2);
                    let cp = *(trans_ptr as *const u16).add(i * 2 + 1);
                    (lang, cp)
                })
                .collect()
        } else {
            // English US / Unicode — covers the vast majority of Windows apps.
            vec![(0x0409u16, 0x04B0u16)]
        };

    let mut r_fv: Option<String> = None;
    let mut r_pv: Option<String> = None;
    let mut r_co: Option<String> = None;
    let mut r_fd: Option<String> = None;

    'outer: for (lang, cp) in &translations {
        for (key_name, slot) in [
            ("FileVersion", &mut r_fv),
            ("ProductVersion", &mut r_pv),
            ("CompanyName", &mut r_co),
            ("FileDescription", &mut r_fd),
        ] {
            if slot.is_some() {
                continue;
            }
            let path = format!(r"\StringFileInfo\{:04X}{:04X}\{}", lang, cp, key_name);
            let path_w: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
            let mut vp: *const u8 = std::ptr::null();
            let mut vl: u32 = 0;
            if unsafe {
                query_val(info.as_ptr(), path_w.as_ptr(), &mut vp, &mut vl)
            } != 0
                && !vp.is_null()
                && vl > 0
            {
                let chars: Vec<u16> = (0..vl as usize)
                    .map(|i| unsafe { *(vp as *const u16).add(i) })
                    .collect();
                let s = String::from_utf16_lossy(&chars)
                    .trim_end_matches('\0')
                    .trim()
                    .to_string();
                if !s.is_empty() {
                    *slot = Some(s);
                }
            }
        }
        if r_fv.is_some() && r_pv.is_some() && r_co.is_some() && r_fd.is_some() {
            break 'outer;
        }
    }

    (r_fv, r_pv, r_co, r_fd)
}
