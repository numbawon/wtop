#![allow(clippy::manual_c_str_literals)]
use crate::models::thread::{ThreadEntry, ThreadState};
use std::collections::HashMap;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Thread32First, Thread32Next, THREADENTRY32, TH32CS_SNAPTHREAD,
};
use windows::Win32::System::Threading::{
    GetThreadTimes, OpenThread, THREAD_QUERY_INFORMATION,
};
use windows::Win32::Foundation::{CloseHandle, FILETIME};
use windows::core::Result as WinResult;

// ──────────────────────────────────────────────────────────────────────────────
// NtQuerySystemInformation structures (64-bit layout, verified sizes below)
// ──────────────────────────────────────────────────────────────────────────────

/// Minimal view of UNICODE_STRING on 64-bit (16 bytes).
#[repr(C)]
struct UnicodeString {
    length: u16,
    maximum_length: u16,
    _pad: u32,
    buffer: usize,
}

/// Minimal fixed-size header of SYSTEM_PROCESS_INFORMATION (256 bytes on x64).
/// The variable-length `Threads[]` array immediately follows in the buffer.
#[repr(C)]
struct SysProcInfo {
    next_entry_offset: u32,
    number_of_threads: u32,
    _spare: [i64; 3],
    _create_time: i64,
    _user_time: i64,
    _kernel_time: i64,
    _image_name: UnicodeString,
    _base_priority: i32,
    _pad1: u32,
    _unique_process_id: usize,
    _inherited_from: usize,
    _handle_count: u32,
    _session_id: u32,
    _unique_process_key: usize,
    _peak_virtual_size: usize,
    _virtual_size: usize,
    _page_fault_count: u32,
    _pad2: u32,
    _peak_ws: usize,
    _ws: usize,
    _quota_peak_paged: usize,
    _quota_paged: usize,
    _quota_peak_nonpaged: usize,
    _quota_nonpaged: usize,
    _pagefile: usize,
    _peak_pagefile: usize,
    _private_page_count: usize,
    _read_ops: i64,
    _write_ops: i64,
    _other_ops: i64,
    _read_bytes: i64,
    _write_bytes: i64,
    _other_bytes: i64,
}

/// SYSTEM_THREAD_INFORMATION (80 bytes on x64).
#[repr(C)]
#[derive(Copy, Clone)]
struct SysThreadInfo {
    _kernel_time: i64,
    _user_time: i64,
    _create_time: i64,
    _wait_time: u32,
    _pad: u32,
    _start_address: usize,
    _client_pid: usize,
    client_tid: usize,
    _priority: i32,
    _base_priority: i32,
    _context_switches: u32,
    thread_state: u32,
    wait_reason: u32,
    _pad2: u32,
}

// Compile-time layout assertions.
const _: () = assert!(std::mem::size_of::<UnicodeString>() == 16);
const _: () = assert!(std::mem::size_of::<SysProcInfo>() == 256);
const _: () = assert!(std::mem::size_of::<SysThreadInfo>() == 80);

type NtQuerySystemInformationFn = unsafe extern "system" fn(
    u32,
    *mut std::ffi::c_void,
    u32,
    *mut u32,
) -> i32;

/// Query thread states for all threads system-wide using
/// NtQuerySystemInformation(SystemProcessInformation=5).
/// Returns a map of TID → ThreadState.
fn query_all_thread_states() -> HashMap<u32, (ThreadState, u32)> {
    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    use windows::core::PCSTR;

    let mut map: HashMap<u32, (ThreadState, u32)> = HashMap::new();

    let ntdll = unsafe { GetModuleHandleW(windows::core::w!("ntdll.dll")).ok() };
    let ntdll = match ntdll {
        Some(h) => h,
        None => return map,
    };

    let proc_addr = unsafe {
        GetProcAddress(ntdll, PCSTR(b"NtQuerySystemInformation\0".as_ptr()))
    };
    let proc_addr = match proc_addr {
        Some(p) => p,
        None => return map,
    };

    // Safety: GetProcAddress returns a type-erased fn pointer. We transmute it to
    // NtQuerySystemInformationFn, whose signature matches the documented
    // NtQuerySystemInformation(Class, Buffer, Length, ReturnLength) -> NTSTATUS
    // "system" (stdcall on x64 = System V AMD64) calling convention. The
    // compile-time size assertions above verify the struct layouts we read from
    // the returned buffer are correct for 64-bit Windows.
    let nt_query: NtQuerySystemInformationFn =
        unsafe { std::mem::transmute(proc_addr) };

    // SystemProcessInformation = 5.
    // Start with 4 MiB; retry with larger buffer if STATUS_INFO_LENGTH_MISMATCH.
    let mut buf_len: u32 = 4 * 1024 * 1024;
    let status_info_length_mismatch: i32 = 0xC0000004u32 as i32;

    loop {
        let mut buf: Vec<u8> = vec![0u8; buf_len as usize];
        let mut returned: u32 = 0;
        let status = unsafe {
            nt_query(5, buf.as_mut_ptr() as _, buf_len, &mut returned)
        };

        if status == status_info_length_mismatch {
            buf_len = returned.max(buf_len * 2);
            if buf_len > 64 * 1024 * 1024 {
                break; // give up
            }
            continue;
        }
        if status != 0 {
            break;
        }

        // Walk the linked list of SYSTEM_PROCESS_INFORMATION entries.
        let mut offset: usize = 0;
        loop {
            if offset + std::mem::size_of::<SysProcInfo>() > buf.len() {
                break;
            }

            let proc_ptr = unsafe {
                &*(buf.as_ptr().add(offset) as *const SysProcInfo)
            };

            let thread_count = proc_ptr.number_of_threads as usize;
            let thread_base = offset + std::mem::size_of::<SysProcInfo>();
            let thread_size = std::mem::size_of::<SysThreadInfo>();

            for t in 0..thread_count {
                let t_offset = thread_base + t * thread_size;
                if t_offset + thread_size > buf.len() {
                    break;
                }
                let t_info = unsafe {
                    &*(buf.as_ptr().add(t_offset) as *const SysThreadInfo)
                };

                let tid = t_info.client_tid as u32;
                let state = map_thread_state(t_info.thread_state, t_info.wait_reason);
                map.insert(tid, (state, t_info.wait_reason));
            }

            if proc_ptr.next_entry_offset == 0 {
                break;
            }
            offset += proc_ptr.next_entry_offset as usize;
        }
        break;
    }

    map
}

fn map_thread_state(state: u32, wait_reason: u32) -> ThreadState {
    match state {
        2 => ThreadState::Running,
        4 => ThreadState::Terminated,
        5 if wait_reason == 5 => ThreadState::Suspended, // WaitReason::Suspended
        5 => ThreadState::Waiting,
        1 | 3 | 6 | 7 => ThreadState::Waiting, // Ready / Standby / Transition / DeferredReady
        _ => ThreadState::Unknown,
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Public thread collection (called on demand when user expands a process row)
// ──────────────────────────────────────────────────────────────────────────────

/// Collect all threads for a given PID using CreateToolhelp32Snapshot,
/// enriching each entry with real thread state from NtQuerySystemInformation.
pub fn collect_threads(pid: u32) -> Vec<ThreadEntry> {
    // Get thread states for all threads in one syscall before taking the snapshot.
    let states = query_all_thread_states();

    let mut entries = Vec::new();

    let snapshot = unsafe {
        match CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) {
            Ok(h) => h,
            Err(e) => {
                tracing::warn!("CreateToolhelp32Snapshot failed: {e}");
                return entries;
            }
        }
    };

    let mut te = THREADENTRY32 {
        dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
        ..Default::default()
    };

    let ok = unsafe { Thread32First(snapshot, &mut te) };
    if ok.is_err() {
        unsafe { let _ = CloseHandle(snapshot); }
        return entries;
    }

    loop {
        if te.th32OwnerProcessID == pid {
            let entry = build_thread_entry(&te, pid, &states);
            entries.push(entry);
        }

        te.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
        if unsafe { Thread32Next(snapshot, &mut te) }.is_err() {
            break;
        }
    }

    unsafe { let _ = CloseHandle(snapshot); }
    entries
}

fn build_thread_entry(
    te: &THREADENTRY32,
    pid: u32,
    states: &HashMap<u32, (ThreadState, u32)>,
) -> ThreadEntry {
    let tid = te.th32ThreadID;
    let priority = te.tpBasePri;

    let (kernel_ms, user_ms, start_address, start_module, name, suspicious) =
        query_thread_details(tid, pid).unwrap_or((0, 0, 0, "?".into(), None, false));

    let (state, wait_reason) = states
        .get(&tid)
        .copied()
        .unwrap_or((ThreadState::Unknown, 0));

    ThreadEntry {
        tid,
        state,
        wait_reason,
        kernel_ms,
        user_ms,
        cpu_pct: 0.0, // filled in by ProcessCollector after delta computation
        priority,
        start_module,
        start_address,
        suspicious,
        name,
    }
}

fn query_thread_details(tid: u32, pid: u32) -> WinResult<(u64, u64, u64, String, Option<String>, bool)> {
    let handle = unsafe { OpenThread(THREAD_QUERY_INFORMATION, false, tid)? };

    let (kernel_ms, user_ms) = get_thread_cpu_times(handle).unwrap_or((0, 0));
    let start_address = get_thread_start_address(handle).unwrap_or(0);
    let name = query_thread_name(handle);
    let (start_module, suspicious) = resolve_module(pid, start_address);

    unsafe { let _ = CloseHandle(handle); }

    Ok((kernel_ms, user_ms, start_address, start_module, name, suspicious))
}

fn get_thread_cpu_times(handle: windows::Win32::Foundation::HANDLE) -> Option<(u64, u64)> {
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();

    unsafe {
        GetThreadTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user).ok()?;
    }

    Some((filetime_to_ms(&kernel), filetime_to_ms(&user)))
}

/// Query the thread description string via NtQueryInformationThread class 38
/// (ThreadNameInformation). Returns None for threads that have no name set
/// or on any error.
fn query_thread_name(handle: windows::Win32::Foundation::HANDLE) -> Option<String> {
    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    use windows::core::PCSTR;

    type NtQueryInformationThreadFn = unsafe extern "system" fn(
        windows::Win32::Foundation::HANDLE,
        u32,
        *mut std::ffi::c_void,
        u32,
        *mut u32,
    ) -> i32;

    let ntdll = unsafe { GetModuleHandleW(windows::core::w!("ntdll.dll")).ok()? };
    let proc_addr = unsafe {
        GetProcAddress(ntdll, PCSTR(b"NtQueryInformationThread\0".as_ptr()))
    }?;

    // Safety: same transmute pattern as get_thread_start_address - documented
    // NtQueryInformationThread calling convention, correct struct layouts.
    let nt_query: NtQueryInformationThreadFn =
        unsafe { std::mem::transmute(proc_addr) };

    // ThreadNameInformation = 38. Returns a UNICODE_STRING header (16 bytes on
    // x64: Length u16, MaximumLength u16, _pad u32, Buffer *mut u16) followed
    // immediately by the UTF-16LE string data in the same allocation.
    // Thread names are rarely longer than 64 chars; 512 bytes is ample.
    let mut buf = vec![0u8; 512];
    let mut ret_len: u32 = 0;

    let status = unsafe {
        nt_query(
            handle,
            38,
            buf.as_mut_ptr() as *mut _,
            buf.len() as u32,
            &mut ret_len,
        )
    };

    if status != 0 {
        return None;
    }

    // UNICODE_STRING layout on x64:
    //   offset 0: Length        u16   (byte count of string, NOT including NUL)
    //   offset 2: MaximumLength u16
    //   offset 4: _pad          u32
    //   offset 8: Buffer        usize (ptr into our buf, right after this header)
    if buf.len() < 16 {
        return None;
    }
    let length_bytes = u16::from_le_bytes([buf[0], buf[1]]) as usize;
    if length_bytes == 0 {
        return None;
    }

    // The Buffer pointer stored at bytes 8..16 points into our allocation.
    // Verify it falls within the buffer before dereferencing.
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

    let name = String::from_utf16_lossy(&chars);
    if name.is_empty() { None } else { Some(name) }
}

fn filetime_to_ms(ft: &FILETIME) -> u64 {
    let ticks = ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64;
    ticks / 10_000
}

fn get_thread_start_address(handle: windows::Win32::Foundation::HANDLE) -> Option<u64> {
    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    use windows::core::PCSTR;

    let ntdll = unsafe { GetModuleHandleW(windows::core::w!("ntdll.dll")).ok()? };
    let proc_addr = unsafe {
        GetProcAddress(ntdll, PCSTR(b"NtQueryInformationThread\0".as_ptr()))
    }?;

    type NtQueryInformationThreadFn = unsafe extern "system" fn(
        windows::Win32::Foundation::HANDLE,
        u32,
        *mut std::ffi::c_void,
        u32,
        *mut u32,
    ) -> i32;

    // Safety: GetProcAddress returns a type-erased fn pointer. We transmute it to
    // NtQueryInformationThreadFn, whose signature matches the documented
    // NtQueryInformationThread(ThreadHandle, Class, Information, Length, ReturnLength)
    // -> NTSTATUS "system" calling convention. We only use class 9
    // (ThreadQuerySetWin32StartAddress) which writes a single u64 into a
    // caller-provided buffer - the size check (sizeof::<u64>) is passed explicitly.
    let nt_query: NtQueryInformationThreadFn =
        unsafe { std::mem::transmute(proc_addr) };

    let mut start_address: u64 = 0;
    let mut ret_len: u32 = 0;

    // ThreadQuerySetWin32StartAddress = 9
    let status = unsafe {
        nt_query(
            handle,
            9,
            &mut start_address as *mut u64 as *mut _,
            std::mem::size_of::<u64>() as u32,
            &mut ret_len,
        )
    };

    if status == 0 { Some(start_address) } else { None }
}

fn resolve_module(pid: u32, start_address: u64) -> (String, bool) {
    use windows::Win32::System::ProcessStatus::{
        EnumProcessModules, GetModuleFileNameExW, GetModuleInformation, MODULEINFO,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
    };

    if start_address == 0 {
        return ("?".into(), false);
    }

    let proc = unsafe {
        OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
            false,
            pid,
        )
        .ok()
    };
    let proc = match proc {
        Some(h) => h,
        None => return ("?".into(), false),
    };

    let mut modules: Vec<windows::Win32::Foundation::HMODULE> =
        vec![windows::Win32::Foundation::HMODULE::default(); 512];
    let mut needed: u32 = 0;

    let ok = unsafe {
        EnumProcessModules(
            proc,
            modules.as_mut_ptr(),
            (modules.len() * std::mem::size_of::<windows::Win32::Foundation::HMODULE>())
                as u32,
            &mut needed,
        )
        .is_ok()
    };

    if !ok {
        unsafe { let _ = CloseHandle(proc); }
        return ("?".into(), false);
    }

    let module_count = (needed as usize
        / std::mem::size_of::<windows::Win32::Foundation::HMODULE>())
    .min(modules.len());

    let mut matched_name = String::from("?");
    let mut found = false;

    for &module in &modules[..module_count] {
        let mut info = MODULEINFO {
            lpBaseOfDll: std::ptr::null_mut(),
            SizeOfImage: 0,
            EntryPoint: std::ptr::null_mut(),
        };

        let ok_info = unsafe {
            GetModuleInformation(
                proc,
                module,
                &mut info,
                std::mem::size_of::<MODULEINFO>() as u32,
            )
            .is_ok()
        };
        if !ok_info {
            continue;
        }

        let base = info.lpBaseOfDll as u64;
        let end = base + info.SizeOfImage as u64;

        if start_address >= base && start_address < end {
            // start_address is within this module.
            let mut name_buf = [0u16; 260];
            let len = unsafe {
                GetModuleFileNameExW(proc, module, &mut name_buf) as usize
            };
            if len > 0 {
                let full_path = String::from_utf16_lossy(&name_buf[..len]);
                // Keep only the filename part (e.g. "ntdll.dll").
                matched_name = full_path
                    .rsplit(['\\', '/'])
                    .next()
                    .unwrap_or(&full_path)
                    .to_string();
            }
            found = true;
            break;
        }
    }

    unsafe { let _ = CloseHandle(proc); }

    // If the start address doesn't fall inside any loaded module it's suspicious.
    (matched_name, !found)
}
