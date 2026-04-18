#![allow(clippy::manual_c_str_literals, clippy::missing_transmute_annotations)]
//! On-demand collector for the process detail overlay (Group B).
//!
//! Called synchronously from the UI thread when the user presses `i`.
//! All operations complete in well under a frame budget - the only
//! potentially slow call is GetFileVersionInfoW which reads the PE
//! version resource, but Windows caches this after the first read.

use windows::Win32::Foundation::{CloseHandle, FILETIME, HANDLE, HMODULE};
use windows::Win32::Security::{
    GetSidSubAuthority, GetSidSubAuthorityCount,
    GetTokenInformation,
    TokenIntegrityLevel, TOKEN_MANDATORY_LABEL, TOKEN_QUERY,
};
use windows::Win32::System::Threading::OpenProcessToken;
use windows::Win32::System::ProcessStatus::{
    EnumProcessModules, GetModuleFileNameExW, GetModuleInformation, MODULEINFO,
    GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
};
use windows::Win32::System::Threading::{
    GetProcessMitigationPolicy, GetProcessTimes, IsWow64Process, OpenProcess,
    PROCESS_MITIGATION_POLICY, PROCESS_DUP_HANDLE,
    PROCESS_NAME_WIN32, PROCESS_QUERY_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_VM_READ, QueryFullProcessImageNameW, GetPriorityClass,
    ABOVE_NORMAL_PRIORITY_CLASS, BELOW_NORMAL_PRIORITY_CLASS, HIGH_PRIORITY_CLASS,
    IDLE_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS, REALTIME_PRIORITY_CLASS,
};
use windows::Win32::Storage::FileSystem::{
    GetFinalPathNameByHandleW, GetFileType, FILE_NAME_NORMALIZED,
    FILE_TYPE_CHAR, FILE_TYPE_DISK, FILE_TYPE_PIPE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
};
use windows::core::PWSTR;
use chrono::{Local, TimeZone};

use crate::models::inspect::{HandleEntry, ModuleEntry, NetworkEntry, ProcessInspectData};
use crate::models::process::IntegrityLevel;

/// Collect all available detail for `pid`. Never panics; fields that cannot
/// be read (access denied, kernel process, etc.) are filled with `"?"` or `None`.
pub fn collect_inspect(pid: u32, name: &str) -> ProcessInspectData {
    let exe_path = get_exe_path(pid).unwrap_or_else(|| "?".into());
    let cmdline   = get_cmdline(pid).unwrap_or_else(|| "?".into());
    let (uptime_secs, start_time_str, cpu_user_ms, cpu_kernel_ms) = get_times(pid);
    let (file_version, product_version, company_name, file_description) =
        if exe_path != "?" { get_version_strings(&exe_path) }
        else { (None, None, None, None) };

    let modules = get_modules(pid);
    let (parent_pid, parent_name) = get_parent_info(pid);
    let integrity = get_integrity_level(pid);
    let (dep_enabled, aslr_enabled, cfg_enabled) = get_mitigation_flags(pid);
    let window_title = get_window_title(pid);
    let open_handles = get_open_handles(pid);
    let open_connections = get_network_connections(pid);
    let (mem_working_set, mem_peak_ws, mem_page_faults) = get_memory_info(pid);
    let arch_x86 = get_is_wow64(pid);
    let priority_class = get_priority_class(pid);
    let env_vars = get_env_vars(pid);
    let threads = crate::collectors::thread::collect_threads(pid);

    ProcessInspectData {
        pid,
        name: name.to_string(),
        exe_path,
        cmdline,
        uptime_secs,
        start_time_str,
        cpu_user_ms,
        cpu_kernel_ms,
        mem_working_set,
        mem_peak_ws,
        mem_page_faults,
        arch_x86,
        priority_class,
        file_version,
        product_version,
        company_name,
        file_description,
        parent_pid,
        parent_name,
        integrity,
        dep_enabled,
        aslr_enabled,
        cfg_enabled,
        window_title,
        modules,
        open_handles,
        open_connections,
        env_vars,
        threads,
    }
}

/// Force-close a specific handle in `pid` using DuplicateHandle + DUPLICATE_CLOSE_SOURCE.
/// Returns an error string on failure.
pub fn force_close_handle(pid: u32, handle_value: u64) -> Result<(), String> {
    use windows::Win32::Foundation::{DuplicateHandle, DUPLICATE_CLOSE_SOURCE};
    use windows::Win32::System::Threading::GetCurrentProcess;

    let target_proc = unsafe {
        OpenProcess(PROCESS_DUP_HANDLE, false, pid)
            .map_err(|e| format!("OpenProcess failed: {e}"))?
    };

    let src = HANDLE(handle_value as *mut _);
    let mut dup = HANDLE::default();
    let result = unsafe {
        DuplicateHandle(
            target_proc,
            src,
            GetCurrentProcess(),
            &mut dup,
            0,
            false,
            DUPLICATE_CLOSE_SOURCE,
        )
    };
    unsafe { let _ = CloseHandle(target_proc); }

    result.map_err(|e| format!("DuplicateHandle failed: {e}"))?;
    // dup is now a copy in our process - close it
    unsafe { let _ = CloseHandle(dup); }
    Ok(())
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

// ── Exe path ──────────────────────────────────────────────────────────────────

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

// ── Uptime / CPU times / start time ──────────────────────────────────────────

/// Returns (uptime_secs, start_time_str, cpu_user_ms, cpu_kernel_ms).
fn get_times(pid: u32) -> (u64, String, u64, u64) {
    let fallback = (0u64, String::new(), 0u64, 0u64);
    let proc = match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok() } {
        Some(h) => h,
        None => return fallback,
    };
    let mut creation = FILETIME::default();
    let mut exit     = FILETIME::default();
    let mut kernel   = FILETIME::default();
    let mut user     = FILETIME::default();
    let ok = unsafe {
        GetProcessTimes(proc, &mut creation, &mut exit, &mut kernel, &mut user).is_ok()
    };
    unsafe { let _ = CloseHandle(proc); }
    if !ok { return fallback; }

    let ft_to_u64 = |ft: FILETIME| -> u64 {
        ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64
    };

    // FILETIME = 100-ns intervals since 1601-01-01 UTC
    const WIN_TO_UNIX: u64 = 11_644_473_600;
    let creation_100ns = ft_to_u64(creation);
    let creation_secs  = creation_100ns / 10_000_000;
    if creation_secs < WIN_TO_UNIX { return fallback; }
    let creation_unix = creation_secs - WIN_TO_UNIX;

    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let uptime = now_unix.saturating_sub(creation_unix);

    let start_str = Local.timestamp_opt(creation_unix as i64, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_default();

    let cpu_user_ms   = ft_to_u64(user)   / 10_000;
    let cpu_kernel_ms = ft_to_u64(kernel) / 10_000;

    (uptime, start_str, cpu_user_ms, cpu_kernel_ms)
}

// ── Memory ────────────────────────────────────────────────────────────────────

fn get_memory_info(pid: u32) -> (u64, u64, u32) {
    let proc = match unsafe {
        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ, false, pid)
            .or_else(|_| OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid))
            .ok()
    } {
        Some(h) => h,
        None => return (0, 0, 0),
    };
    let mut pmc = PROCESS_MEMORY_COUNTERS::default();
    let ok = unsafe {
        GetProcessMemoryInfo(
            proc,
            &mut pmc,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        )
        .is_ok()
    };
    unsafe { let _ = CloseHandle(proc); }
    if ok {
        (pmc.WorkingSetSize as u64, pmc.PeakWorkingSetSize as u64, pmc.PageFaultCount)
    } else {
        (0, 0, 0)
    }
}

// ── Architecture ──────────────────────────────────────────────────────────────

fn get_is_wow64(pid: u32) -> bool {
    let proc = match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok() } {
        Some(h) => h,
        None => return false,
    };
    let mut wow = windows::Win32::Foundation::BOOL(0);
    let ok = unsafe { IsWow64Process(proc, &mut wow).is_ok() };
    unsafe { let _ = CloseHandle(proc); }
    ok && wow.as_bool()
}

// ── Priority class ────────────────────────────────────────────────────────────

fn get_priority_class(pid: u32) -> String {
    let proc = match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok() } {
        Some(h) => h,
        None => return "?".into(),
    };
    let cls = unsafe { GetPriorityClass(proc) };
    unsafe { let _ = CloseHandle(proc); }
    match cls {
        x if x == IDLE_PRIORITY_CLASS.0         => "Idle",
        x if x == BELOW_NORMAL_PRIORITY_CLASS.0 => "Below Normal",
        x if x == NORMAL_PRIORITY_CLASS.0       => "Normal",
        x if x == ABOVE_NORMAL_PRIORITY_CLASS.0 => "Above Normal",
        x if x == HIGH_PRIORITY_CLASS.0         => "High",
        x if x == REALTIME_PRIORITY_CLASS.0     => "Realtime",
        0                                        => "?",
        _                                        => "Normal",
    }.to_string()
}

// ── Environment variables ─────────────────────────────────────────────────────

fn get_env_vars(pid: u32) -> Vec<(String, String)> {
    // PEB + ProcessParameters offsets on x64:
    //   PEB+0x020 = ProcessParameters pointer
    //   RTL_USER_PROCESS_PARAMETERS+0x080 = Environment pointer
    //   RTL_USER_PROCESS_PARAMETERS+0x3F0 = EnvironmentSize (SIZE_T, Win8+)
    let proc = match unsafe {
        OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid)
            .or_else(|_| OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ, false, pid))
            .ok()
    } {
        Some(h) => h,
        None => return Vec::new(),
    };

    let result = read_env_block(proc, pid);
    unsafe { let _ = CloseHandle(proc); }
    result
}

fn read_env_block(proc: HANDLE, pid: u32) -> Vec<(String, String)> {
    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    use windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;
    use windows::core::PCSTR;

    type NtQueryInfoFn = unsafe extern "system" fn(
        HANDLE, u32, *mut std::ffi::c_void, u32, *mut u32,
    ) -> i32;

    #[repr(C)]
    struct ProcessBasicInfo {
        exit_status: u32,
        _pad1: u32,
        peb_base: u64,
        affinity: usize,
        priority: i32,
        _pad2: u32,
        unique_pid: usize,
        parent_pid: usize,
    }

    let ntdll = match unsafe { GetModuleHandleW(windows::core::w!("ntdll.dll")).ok() } {
        Some(h) => h,
        None => return Vec::new(),
    };
    let fn_ptr = match unsafe {
        GetProcAddress(ntdll, PCSTR(b"NtQueryInformationProcess\0".as_ptr()))
    } {
        Some(f) => f,
        None => return Vec::new(),
    };
    let nt_query: NtQueryInfoFn = unsafe { std::mem::transmute(fn_ptr) };

    // 1. Get PEB address
    let mut pbi = ProcessBasicInfo {
        exit_status: 0, _pad1: 0, peb_base: 0,
        affinity: 0, priority: 0, _pad2: 0,
        unique_pid: 0, parent_pid: 0,
    };
    let mut ret: u32 = 0;
    let st = unsafe {
        nt_query(proc, 0, &mut pbi as *mut _ as *mut _, std::mem::size_of::<ProcessBasicInfo>() as u32, &mut ret)
    };
    if st != 0 || pbi.peb_base == 0 { return Vec::new(); }

    let read_mem = |addr: u64, buf: &mut [u8]| -> bool {
        let mut read = 0usize;
        unsafe {
            ReadProcessMemory(proc, addr as *const _, buf.as_mut_ptr() as *mut _, buf.len(), Some(&mut read)).is_ok()
                && read == buf.len()
        }
    };

    // 2. PEB+0x20 = ProcessParameters*
    let mut pp_ptr_buf = [0u8; 8];
    if !read_mem(pbi.peb_base + 0x20, &mut pp_ptr_buf) { return Vec::new(); }
    let pp_ptr = u64::from_le_bytes(pp_ptr_buf);
    if pp_ptr == 0 { return Vec::new(); }

    // 3. ProcessParameters+0x80 = Environment*
    let mut env_ptr_buf = [0u8; 8];
    if !read_mem(pp_ptr + 0x80, &mut env_ptr_buf) { return Vec::new(); }
    let env_ptr = u64::from_le_bytes(env_ptr_buf);
    if env_ptr == 0 { return Vec::new(); }

    // 4. ProcessParameters+0x3F0 = EnvironmentSize (SIZE_T, available Win8+)
    let mut env_size_buf = [0u8; 8];
    let env_size: usize = if read_mem(pp_ptr + 0x3F0, &mut env_size_buf) {
        let sz = usize::from_le_bytes(env_size_buf);
        if sz > 0 && sz < 2 * 1024 * 1024 { sz } else { 64 * 1024 }
    } else {
        64 * 1024
    };

    // 5. Read the env block (wide chars, double-null terminated)
    let mut raw = vec![0u8; env_size];
    let mut actually_read = 0usize;
    let ok = unsafe {
        ReadProcessMemory(proc, env_ptr as *const _, raw.as_mut_ptr() as *mut _, raw.len(), Some(&mut actually_read)).is_ok()
    };
    if !ok || actually_read < 4 { return Vec::new(); }
    raw.truncate(actually_read);

    // 6. Parse wide-char "KEY=VALUE\0" pairs until double-null
    let words: Vec<u16> = raw.chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();

    let mut vars: Vec<(String, String)> = Vec::new();
    let mut start = 0;
    while start < words.len() {
        let end = words[start..].iter().position(|&c| c == 0)
            .map(|p| start + p)
            .unwrap_or(words.len());
        if end == start { break; } // double-null
        let entry = String::from_utf16_lossy(&words[start..end]);
        if let Some(eq) = entry.find('=') {
            let key = entry[..eq].to_string();
            let val = entry[eq + 1..].to_string();
            if !key.is_empty() {
                vars.push((key, val));
            }
        }
        start = end + 1;
    }

    tracing::debug!("get_env_vars: pid={} count={}", pid, vars.len());
    vars.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    vars
}

// ── Parent process ────────────────────────────────────────────────────────────

fn get_parent_info(pid: u32) -> (u32, String) {
    let parent_pid = get_parent_pid(pid).unwrap_or(0);
    if parent_pid == 0 {
        return (0, "?".into());
    }
    let parent_name = get_exe_path(parent_pid)
        .and_then(|p| p.rsplit('\\').next().map(|s| s.to_string()))
        .unwrap_or_else(|| "?".into());
    (parent_pid, parent_name)
}

fn get_parent_pid(pid: u32) -> Option<u32> {
    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    use windows::core::PCSTR;

    type NtQueryInfoFn = unsafe extern "system" fn(HANDLE, u32, *mut std::ffi::c_void, u32, *mut u32) -> i32;

    // PROCESS_BASIC_INFORMATION (x64): ExitStatus(4)+pad(4)+PebBase(8)+Affinity(8)+Priority(4)+pad(4)+UniqueId(8)+ParentId(8)
    #[repr(C)]
    struct ProcessBasicInformation {
        exit_status: u32,
        _pad1: u32,
        peb_base: *mut std::ffi::c_void,
        affinity_mask: usize,
        base_priority: i32,
        _pad2: u32,
        unique_process_id: usize,
        inherited_from_unique_process_id: usize,
    }

    let ntdll = unsafe { GetModuleHandleW(windows::core::w!("ntdll.dll")).ok()? };
    let fn_ptr = unsafe {
        GetProcAddress(ntdll, PCSTR(b"NtQueryInformationProcess\0".as_ptr()))
    }?;
    let nt_query: NtQueryInfoFn = unsafe { std::mem::transmute(fn_ptr) };

    let proc = unsafe {
        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?
    };

    let mut pbi = ProcessBasicInformation {
        exit_status: 0,
        _pad1: 0,
        peb_base: std::ptr::null_mut(),
        affinity_mask: 0,
        base_priority: 0,
        _pad2: 0,
        unique_process_id: 0,
        inherited_from_unique_process_id: 0,
    };
    let mut ret_len: u32 = 0;
    let status = unsafe {
        nt_query(
            proc,
            0, // ProcessBasicInformation
            &mut pbi as *mut _ as *mut _,
            std::mem::size_of::<ProcessBasicInformation>() as u32,
            &mut ret_len,
        )
    };
    unsafe { let _ = CloseHandle(proc); }

    if status != 0 { None } else { Some(pbi.inherited_from_unique_process_id as u32) }
}

// ── Integrity level ───────────────────────────────────────────────────────────

fn get_integrity_level(pid: u32) -> IntegrityLevel {
    if pid == 0 || pid == 4 {
        return IntegrityLevel::System;
    }
    unsafe { read_integrity(pid) }.unwrap_or(IntegrityLevel::Unknown)
}

unsafe fn read_integrity(pid: u32) -> Option<IntegrityLevel> {
    let proc = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
    let mut token = HANDLE::default();
    if OpenProcessToken(proc, TOKEN_QUERY, &mut token).is_err() {
        let _ = CloseHandle(proc);
        return None;
    }
    let _ = CloseHandle(proc);

    let mut needed: u32 = 0;
    let _ = GetTokenInformation(token, TokenIntegrityLevel, None, 0, &mut needed);
    if needed == 0 {
        let _ = CloseHandle(token);
        return None;
    }
    let mut buf = vec![0u8; needed as usize];
    if GetTokenInformation(
        token,
        TokenIntegrityLevel,
        Some(buf.as_mut_ptr() as *mut _),
        needed,
        &mut needed,
    )
    .is_err()
    {
        let _ = CloseHandle(token);
        return None;
    }
    let _ = CloseHandle(token);

    let tml = &*(buf.as_ptr() as *const TOKEN_MANDATORY_LABEL);
    let sid = tml.Label.Sid;
    let sub_count = *GetSidSubAuthorityCount(sid) as u32;
    if sub_count == 0 {
        return None;
    }
    let level = *GetSidSubAuthority(sid, sub_count - 1);
    Some(match level {
        0x0000          => IntegrityLevel::Untrusted,
        0x1000          => IntegrityLevel::Low,
        0x2000..=0x2FFF => IntegrityLevel::Medium,
        0x3000..=0x3FFF => IntegrityLevel::High,
        _               => IntegrityLevel::System,
    })
}

// ── Mitigation flags (DEP / ASLR / CFG) ──────────────────────────────────────

fn get_mitigation_flags(pid: u32) -> (Option<bool>, Option<bool>, Option<bool>) {
    let proc = match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok() } {
        Some(h) => h,
        None => return (None, None, None),
    };

    // Raw bit-flag structs matching the first DWORD of each mitigation policy struct.
    #[repr(C)] struct DepFlags   { flags: u32, _permanent: i32 }
    #[repr(C)] struct AslrFlags  { flags: u32 }
    #[repr(C)] struct CfgFlags   { flags: u32 }

    let dep = {
        let mut s = DepFlags { flags: 0, _permanent: 0 };
        if unsafe { GetProcessMitigationPolicy(proc, PROCESS_MITIGATION_POLICY(0),
            &mut s as *mut _ as *mut _, std::mem::size_of::<DepFlags>()).is_ok() }
        { Some(s.flags & 1 != 0) } else { None }
    };

    let aslr = {
        let mut s = AslrFlags { flags: 0 };
        if unsafe { GetProcessMitigationPolicy(proc, PROCESS_MITIGATION_POLICY(1),
            &mut s as *mut _ as *mut _, std::mem::size_of::<AslrFlags>()).is_ok() }
        { Some(s.flags & 3 != 0) } else { None }  // bits 0+1 = ForceRelocate + BottomUp
    };

    let cfg = {
        let mut s = CfgFlags { flags: 0 };
        if unsafe { GetProcessMitigationPolicy(proc, PROCESS_MITIGATION_POLICY(7),
            &mut s as *mut _ as *mut _, std::mem::size_of::<CfgFlags>()).is_ok() }
        { Some(s.flags & 1 != 0) } else { None }
    };

    unsafe { let _ = CloseHandle(proc); }
    (dep, aslr, cfg)
}

// ── Window title ──────────────────────────────────────────────────────────────

fn get_window_title(pid: u32) -> Option<String> {
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM};

    struct EnumData { pid: u32, title: Option<String> }

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let data = &mut *(lparam.0 as *mut EnumData);
        let mut win_pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut win_pid));
        if win_pid == data.pid && IsWindowVisible(hwnd).as_bool() {
            let mut buf = [0u16; 512];
            let len = GetWindowTextW(hwnd, &mut buf) as usize;
            if len > 0 {
                let t = String::from_utf16_lossy(&buf[..len]);
                if !t.trim().is_empty() {
                    data.title = Some(t);
                    return BOOL(0); // stop enumeration
                }
            }
        }
        BOOL(1)
    }

    let mut data = EnumData { pid, title: None };
    unsafe {
        let _ = EnumWindows(
            Some(enum_proc),
            LPARAM(&mut data as *mut _ as isize),
        );
    }
    data.title
}

// ── Open handles ──────────────────────────────────────────────────────────────

pub fn get_open_handles(pid: u32) -> Vec<HandleEntry> {
    use windows::Win32::Foundation::{DuplicateHandle, DUPLICATE_SAME_ACCESS};
    use windows::Win32::System::Threading::GetCurrentProcess;
    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    use windows::core::PCSTR;

    type NtQueryInfoFn   = unsafe extern "system" fn(HANDLE, u32, *mut std::ffi::c_void, u32, *mut u32) -> i32;
    type NtQuerySysFn    = unsafe extern "system" fn(u32,   *mut std::ffi::c_void, u32, *mut u32) -> i32;
    type NtQueryObjectFn = unsafe extern "system" fn(HANDLE, u32, *mut std::ffi::c_void, u32, *mut u32) -> i32;

    // PROCESS_HANDLE_TABLE_ENTRY_INFO (x64, 40 bytes) - returned by ProcessHandleInformation=51.
    #[repr(C)]
    struct ProcHandleEntry {
        handle_value:      usize, // 8 - handle value in target process
        handle_count:      usize, // 8
        pointer_count:     usize, // 8
        granted_access:    u32,   // 4
        object_type_index: u32,   // 4
        handle_attributes: u32,   // 4
        _reserved:         u32,   // 4
    }

    // SYSTEM_HANDLE_TABLE_ENTRY_INFO_EX (x64, 40 bytes) - returned by SystemExtendedHandleInformation=64.
    #[repr(C)]
    struct SysHandleEntry {
        _object:           *mut std::ffi::c_void, // 8
        unique_process_id: usize,                 // 8
        handle_value:      usize,                 // 8
        _granted_access:   u32,                   // 4
        _creator_bt:       u16,                   // 2
        _type_index:       u16,                   // 2
        _attributes:       u32,                   // 4
        _reserved:         u32,                   // 4
    }

    const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xC0000004u32 as i32;

    let ntdll = match unsafe { GetModuleHandleW(windows::core::w!("ntdll.dll")).ok() } {
        Some(h) => h,
        None => return Vec::new(),
    };

    let nt_info_raw = unsafe { GetProcAddress(ntdll, PCSTR(b"NtQueryInformationProcess\0".as_ptr())) };
    let nt_sys_raw  = unsafe { GetProcAddress(ntdll, PCSTR(b"NtQuerySystemInformation\0".as_ptr())) };
    let nt_obj_raw  = unsafe { GetProcAddress(ntdll, PCSTR(b"NtQueryObject\0".as_ptr())) };

    let nt_info: Option<NtQueryInfoFn>   = nt_info_raw.map(|f| unsafe { std::mem::transmute(f) });
    let nt_sys:  Option<NtQuerySysFn>    = nt_sys_raw.map( |f| unsafe { std::mem::transmute(f) });
    let nt_obj:  Option<NtQueryObjectFn> = nt_obj_raw.map( |f| unsafe { std::mem::transmute(f) });

    // Open target with the richest access available.
    let target_proc = match unsafe {
        OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_DUP_HANDLE, false, pid)
            .or_else(|_| OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_DUP_HANDLE, false, pid))
            .or_else(|_| OpenProcess(PROCESS_DUP_HANDLE, false, pid))
            .ok()
    } {
        Some(h) => h,
        None => {
            tracing::debug!("get_open_handles: OpenProcess failed pid={}", pid);
            return Vec::new();
        }
    };

    // ── Strategy 1: NtQueryInformationProcess(ProcessHandleInformation=51) ───
    // Direct per-process enumeration - no system-wide scan, no PID filter needed.
    // Requires PROCESS_QUERY_INFORMATION on target_proc (available for most
    // user-space processes when running without additional restrictions).
    let handle_list: Vec<u64> = if let Some(nt_info) = nt_info {
        let mut buf: Vec<u8> = vec![0u8; 16 * 1024];
        let mut ret_len: u32 = 0;
        let mut ok = false;
        for _ in 0..8 {
            let s = unsafe {
                nt_info(target_proc, 51, buf.as_mut_ptr() as *mut _, buf.len() as u32, &mut ret_len)
            };
            if s == STATUS_INFO_LENGTH_MISMATCH {
                let need = if ret_len as usize > buf.len() { ret_len as usize + 1024 } else { buf.len() * 2 };
                buf.resize(need, 0);
                continue;
            }
            if s != 0 {
                tracing::debug!("get_open_handles: NtQueryInformationProcess(51) 0x{:08x} pid={}", s as u32, pid);
                break;
            }
            ok = true;
            break;
        }
        if ok && buf.len() >= 16 {
            let n = usize::from_le_bytes(buf[0..8].try_into().unwrap_or([0u8; 8]));
            tracing::debug!("get_open_handles: ProcessHandleInformation pid={} n={}", pid, n);
            let esz = std::mem::size_of::<ProcHandleEntry>();
            (0..n.min(65_536)).filter_map(|i| {
                let off = 16 + i * esz;
                if off + esz > buf.len() { return None; }
                let e = unsafe { &*(buf.as_ptr().add(off) as *const ProcHandleEntry) };
                Some(e.handle_value as u64)
            }).collect()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    // ── Strategy 2: NtQuerySystemInformation(SystemExtendedHandleInformation=64) ─
    // Fallback when ProcessHandleInformation failed (access-denied or unavailable).
    // Scans all system handles, filters to the target PID.
    let handle_list: Vec<u64> = if !handle_list.is_empty() {
        handle_list
    } else if let Some(nt_sys) = nt_sys {
        let mut buf: Vec<u8> = vec![0u8; 1 << 17]; // 128 KB initial
        let mut ret_len: u32 = 0;
        let mut ok = false;
        for _ in 0..8 {
            let s = unsafe {
                nt_sys(64, buf.as_mut_ptr() as *mut _, buf.len() as u32, &mut ret_len)
            };
            if s == STATUS_INFO_LENGTH_MISMATCH {
                let need = if ret_len as usize > buf.len() { ret_len as usize + 4096 } else { buf.len() * 2 };
                buf.resize(need, 0);
                continue;
            }
            if s != 0 {
                tracing::debug!("get_open_handles: NtQuerySystemInformation(64) 0x{:08x} pid={}", s as u32, pid);
                break;
            }
            ok = true;
            break;
        }
        if ok && buf.len() >= 16 {
            let n = usize::from_le_bytes(buf[0..8].try_into().unwrap_or([0u8; 8]));
            tracing::debug!("get_open_handles: SystemExtendedHandleInfo pid={} total_sys={}", pid, n);
            let esz = std::mem::size_of::<SysHandleEntry>();
            (0..n.min(65_536)).filter_map(|i| {
                let off = 16 + i * esz;
                if off + esz > buf.len() { return None; }
                let e = unsafe { &*(buf.as_ptr().add(off) as *const SysHandleEntry) };
                if e.unique_process_id != pid as usize { return None; }
                Some(e.handle_value as u64)
            }).collect()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    tracing::debug!("get_open_handles: pid={} candidate_handles={}", pid, handle_list.len());

    if handle_list.is_empty() {
        unsafe { let _ = CloseHandle(target_proc); }
        return Vec::new();
    }

    let our_proc = unsafe { GetCurrentProcess() };
    let mut results: Vec<HandleEntry> = Vec::new();
    let mut dup_fail: usize = 0;

    for hv in &handle_list {
        let src = HANDLE(*hv as *mut _);
        let mut dup = HANDLE::default();
        let dup_ok = unsafe {
            DuplicateHandle(target_proc, src, our_proc, &mut dup, 0, false, DUPLICATE_SAME_ACCESS).is_ok()
        };
        if !dup_ok { dup_fail += 1; continue; }

        // Object type name via NtQueryObject(ObjectTypeInformation=2).
        let type_name = if let Some(nt_obj) = nt_obj {
            let mut tbuf = vec![0u8; 512];
            let mut tlen: u32 = 0;
            let ts = unsafe { nt_obj(dup, 2, tbuf.as_mut_ptr() as *mut _, tbuf.len() as u32, &mut tlen) };
            if ts == 0 && tbuf.len() >= 16 {
                let char_len = u16::from_le_bytes([tbuf[0], tbuf[1]]) as usize;
                let ptr_val  = usize::from_le_bytes(tbuf[8..16].try_into().unwrap_or([0u8; 8]));
                let base     = tbuf.as_ptr() as usize;
                if char_len > 0 && ptr_val >= base && ptr_val + char_len <= base + tbuf.len() {
                    let off = ptr_val - base;
                    let chars: Vec<u16> = tbuf[off..off + char_len]
                        .chunks_exact(2)
                        .map(|c| u16::from_le_bytes([c[0], c[1]]))
                        .collect();
                    String::from_utf16_lossy(&chars).to_string()
                } else { String::new() }
            } else { String::new() }
        } else { String::new() };

        // Resolve name: full path for disk files; NT object name for other named types.
        let name = if type_name == "File" {
            let ft = unsafe { GetFileType(dup) };
            if ft == FILE_TYPE_DISK {
                let mut pbuf = [0u16; 1024];
                let plen = unsafe {
                    GetFinalPathNameByHandleW(dup, &mut pbuf, FILE_NAME_NORMALIZED) as usize
                };
                if plen > 0 && plen < pbuf.len() {
                    let raw = String::from_utf16_lossy(&pbuf[..plen]);
                    raw.strip_prefix(r"\\?\").unwrap_or(&raw).to_string()
                } else { String::new() }
            } else if ft == FILE_TYPE_PIPE {
                "[pipe]".into()
            } else if ft == FILE_TYPE_CHAR {
                "[console]".into()
            } else {
                String::new()
            }
        } else if matches!(
            type_name.as_str(),
            "Key" | "Section" | "Event" | "Mutant" | "Semaphore" | "SymbolicLink" | "Directory"
        ) {
            // Query NT object name (ObjectNameInformation = 1).
            // Safe for these types - they do not block.
            nt_obj.map_or(String::new(), |f| query_nt_object_name(f, dup))
        } else {
            String::new()
        };

        unsafe { let _ = CloseHandle(dup); }

        if !type_name.is_empty() {
            results.push(HandleEntry { handle_value: *hv, type_name, name });
        }
    }

    tracing::debug!("get_open_handles: pid={} dup_fail={} resolved={}", pid, dup_fail, results.len());
    unsafe { let _ = CloseHandle(target_proc); }

    // File handles with resolved paths first, then by type, then by name.
    results.sort_by(|a, b| {
        let af = a.type_name == "File" && !a.name.is_empty() && a.name != "[pipe]";
        let bf = b.type_name == "File" && !b.name.is_empty() && b.name != "[pipe]";
        match (af, bf) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.type_name.cmp(&b.type_name).then(a.name.cmp(&b.name)),
        }
    });

    results
}

// ── NT object name query ──────────────────────────────────────────────────────

type NtQueryObjectFnRef = unsafe extern "system" fn(
    windows::Win32::Foundation::HANDLE, u32, *mut std::ffi::c_void, u32, *mut u32
) -> i32;

/// Query ObjectNameInformation (class 1) from a duplicated handle.
/// Returns the NT path string, or empty string on failure.
fn query_nt_object_name(
    nt_obj: NtQueryObjectFnRef,
    dup: windows::Win32::Foundation::HANDLE,
) -> String {
    let mut buf = vec![0u8; 1024];
    let mut ret_len: u32 = 0;
    let s = unsafe { nt_obj(dup, 1, buf.as_mut_ptr() as *mut _, buf.len() as u32, &mut ret_len) };
    if s != 0 || buf.len() < 16 {
        return String::new();
    }
    // UNICODE_STRING: Length(u16), MaxLength(u16), [4-byte pad on x64], Buffer(*u16)
    let char_len = u16::from_le_bytes([buf[0], buf[1]]) as usize;
    if char_len == 0 {
        return String::new();
    }
    let ptr_val = usize::from_le_bytes(buf[8..16].try_into().unwrap_or([0u8; 8]));
    let base = buf.as_ptr() as usize;
    if ptr_val < base || ptr_val + char_len > base + buf.len() {
        return String::new();
    }
    let off = ptr_val - base;
    let chars: Vec<u16> = buf[off..off + char_len]
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let raw = String::from_utf16_lossy(&chars).to_string();
    normalize_nt_path(&raw)
}

/// Convert NT registry paths to friendly names and clean device paths.
fn normalize_nt_path(path: &str) -> String {
    let p = path;
    // Registry: \REGISTRY\MACHINE\... → HKLM\...
    if let Some(rest) = p.strip_prefix(r"\REGISTRY\MACHINE\") {
        return format!("HKLM\\{}", rest);
    }
    // Registry: \REGISTRY\USER\S-..._Classes\... → HKCR\...
    if let Some(rest) = p.strip_prefix(r"\REGISTRY\USER\") {
        if let Some(after_sid) = rest.split_once('\\') {
            if after_sid.0.ends_with("_Classes") {
                return format!("HKCR\\{}", after_sid.1);
            }
            return format!("HKCU\\{}", after_sid.1);
        }
        return format!("HKCU\\{}", rest);
    }
    // Device paths: \Device\HarddiskVolume3\Windows\... → keep as-is (best we can do)
    p.to_string()
}

// ── Network connections (GetExtendedTcpTable / GetExtendedUdpTable) ──────────

fn get_network_connections(pid: u32) -> Vec<NetworkEntry> {
    use windows::Win32::NetworkManagement::IpHelper::{
        GetExtendedTcpTable, GetExtendedUdpTable,
        TCP_TABLE_OWNER_PID_ALL, UDP_TABLE_OWNER_PID,
    };

    const AF_INET:  u32 = 2;
    const AF_INET6: u32 = 23;

    // MIB_TCPROW_OWNER_PID (24 bytes)
    #[repr(C)] struct TcpRow4 { state: u32, local_addr: u32, local_port: u32, remote_addr: u32, remote_port: u32, pid: u32 }
    // MIB_TCP6ROW_OWNER_PID (56 bytes)
    #[repr(C)] struct TcpRow6 { local_addr: [u8;16], local_scope_id: u32, local_port: u32, remote_addr: [u8;16], remote_scope_id: u32, remote_port: u32, state: u32, pid: u32 }
    // MIB_UDPROW_OWNER_PID (12 bytes)
    #[repr(C)] struct UdpRow4 { local_addr: u32, local_port: u32, pid: u32 }
    // MIB_UDP6ROW_OWNER_PID (28 bytes)
    #[repr(C)] struct UdpRow6 { local_addr: [u8;16], local_scope_id: u32, local_port: u32, pid: u32 }

    let mut entries: Vec<NetworkEntry> = Vec::new();

    // Helper macro: probe size, allocate, fill, parse rows for one protocol family.
    macro_rules! collect_tcp {
        ($af:expr, $row:ty, $proto:literal) => {{
            let mut sz: u32 = 0;
            unsafe { GetExtendedTcpTable(None, &mut sz, false, $af, TCP_TABLE_OWNER_PID_ALL, 0) };
            if sz > 0 {
                let mut buf = vec![0u8; sz as usize + 512];
                let mut actual = buf.len() as u32;
                if unsafe { GetExtendedTcpTable(Some(buf.as_mut_ptr() as *mut _), &mut actual, false, $af, TCP_TABLE_OWNER_PID_ALL, 0) } == 0
                    && buf.len() >= 4
                {
                    let n = u32::from_ne_bytes([buf[0],buf[1],buf[2],buf[3]]) as usize;
                    let esz = std::mem::size_of::<$row>();
                    for i in 0..n.min(4096) {
                        let off = 4 + i * esz;
                        if off + esz > buf.len() { break; }
                        let r = unsafe { &*(buf.as_ptr().add(off) as *const $row) };
                        entries.push(r.to_entry(pid, $proto));
                    }
                }
            }
        }};
    }

    macro_rules! collect_udp {
        ($af:expr, $row:ty, $proto:literal) => {{
            let mut sz: u32 = 0;
            unsafe { GetExtendedUdpTable(None, &mut sz, false, $af, UDP_TABLE_OWNER_PID, 0) };
            if sz > 0 {
                let mut buf = vec![0u8; sz as usize + 512];
                let mut actual = buf.len() as u32;
                if unsafe { GetExtendedUdpTable(Some(buf.as_mut_ptr() as *mut _), &mut actual, false, $af, UDP_TABLE_OWNER_PID, 0) } == 0
                    && buf.len() >= 4
                {
                    let n = u32::from_ne_bytes([buf[0],buf[1],buf[2],buf[3]]) as usize;
                    let esz = std::mem::size_of::<$row>();
                    for i in 0..n.min(4096) {
                        let off = 4 + i * esz;
                        if off + esz > buf.len() { break; }
                        let r = unsafe { &*(buf.as_ptr().add(off) as *const $row) };
                        entries.push(r.to_entry(pid, $proto));
                    }
                }
            }
        }};
    }

    // Traits that let each row type produce a NetworkEntry.
    trait TcpToEntry { fn to_entry(&self, filter_pid: u32, proto: &'static str) -> NetworkEntry; }
    trait UdpToEntry { fn to_entry(&self, filter_pid: u32, proto: &'static str) -> NetworkEntry; }

    impl TcpToEntry for TcpRow4 {
        fn to_entry(&self, filter_pid: u32, proto: &'static str) -> NetworkEntry {
            if self.pid != filter_pid { return NetworkEntry { proto, local_addr: String::new(), remote_addr: String::new(), state: "" }; }
            let lp = u16::from_be(self.local_port as u16);
            let rp = u16::from_be(self.remote_port as u16);
            let la = std::net::Ipv4Addr::from(self.local_addr.to_ne_bytes());
            let ra = std::net::Ipv4Addr::from(self.remote_addr.to_ne_bytes());
            NetworkEntry {
                proto,
                local_addr:  format!("{}:{}", la, lp),
                remote_addr: if rp != 0 { format!("{}:{}", ra, rp) } else { String::new() },
                state:       tcp_state(self.state),
            }
        }
    }

    impl TcpToEntry for TcpRow6 {
        fn to_entry(&self, filter_pid: u32, proto: &'static str) -> NetworkEntry {
            if self.pid != filter_pid { return NetworkEntry { proto, local_addr: String::new(), remote_addr: String::new(), state: "" }; }
            let lp = u16::from_be(self.local_port as u16);
            let rp = u16::from_be(self.remote_port as u16);
            let la = std::net::Ipv6Addr::from(self.local_addr);
            let ra = std::net::Ipv6Addr::from(self.remote_addr);
            NetworkEntry {
                proto,
                local_addr:  format!("[{}]:{}", la, lp),
                remote_addr: if rp != 0 { format!("[{}]:{}", ra, rp) } else { String::new() },
                state:       tcp_state(self.state),
            }
        }
    }

    impl UdpToEntry for UdpRow4 {
        fn to_entry(&self, filter_pid: u32, proto: &'static str) -> NetworkEntry {
            if self.pid != filter_pid { return NetworkEntry { proto, local_addr: String::new(), remote_addr: String::new(), state: "" }; }
            let lp = u16::from_be(self.local_port as u16);
            let la = std::net::Ipv4Addr::from(self.local_addr.to_ne_bytes());
            NetworkEntry { proto, local_addr: format!("{}:{}", la, lp), remote_addr: String::new(), state: "" }
        }
    }

    impl UdpToEntry for UdpRow6 {
        fn to_entry(&self, filter_pid: u32, proto: &'static str) -> NetworkEntry {
            if self.pid != filter_pid { return NetworkEntry { proto, local_addr: String::new(), remote_addr: String::new(), state: "" }; }
            let lp = u16::from_be(self.local_port as u16);
            let la = std::net::Ipv6Addr::from(self.local_addr);
            NetworkEntry { proto, local_addr: format!("[{}]:{}", la, lp), remote_addr: String::new(), state: "" }
        }
    }

    collect_tcp!(AF_INET,  TcpRow4, "TCP4");
    collect_tcp!(AF_INET6, TcpRow6, "TCP6");
    collect_udp!(AF_INET,  UdpRow4, "UDP4");
    collect_udp!(AF_INET6, UdpRow6, "UDP6");

    // Drop placeholder entries (pid didn't match) and sort.
    entries.retain(|e| !e.local_addr.is_empty());
    entries.sort_by_key(|e| (sort_rank(e), e.proto, e.local_addr.clone()));
    entries
}

fn tcp_state(s: u32) -> &'static str {
    match s {
        1  => "CLOSED",
        2  => "LISTEN",
        3  => "SYN_SENT",
        4  => "SYN_RCVD",
        5  => "ESTABLISHED",
        6  => "FIN_WAIT1",
        7  => "FIN_WAIT2",
        8  => "CLOSE_WAIT",
        9  => "CLOSING",
        10 => "LAST_ACK",
        11 => "TIME_WAIT",
        _  => "UNKNOWN",
    }
}

fn sort_rank(e: &NetworkEntry) -> u8 {
    match e.state {
        "ESTABLISHED" => 0,
        "LISTEN"      => 1,
        ""            => 2, // UDP
        _             => 3, // closing states
    }
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
    // version.dll is tiny (~50 KB) and always present - we intentionally
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
            // English US / Unicode - covers the vast majority of Windows apps.
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
