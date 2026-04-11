use crate::models::thread::{ThreadEntry, ThreadState};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Thread32First, Thread32Next, THREADENTRY32, TH32CS_SNAPTHREAD,
};
use windows::Win32::System::Threading::{
    GetThreadTimes, OpenThread, THREAD_QUERY_INFORMATION,
};
use windows::Win32::Foundation::{CloseHandle, FILETIME};
use windows::core::Result as WinResult;

/// Collect all threads for a given PID using CreateToolhelp32Snapshot.
pub fn collect_threads(pid: u32) -> Vec<ThreadEntry> {
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
            let entry = build_thread_entry(&te, pid);
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

fn build_thread_entry(te: &THREADENTRY32, pid: u32) -> ThreadEntry {
    let tid = te.th32ThreadID;
    let priority = te.tpBasePri as i32;

    let (cpu_time_ms, start_address, start_module, suspicious) =
        query_thread_details(tid).unwrap_or((0, 0, "?".into(), false));

    // Map sysinfo thread state — we don't have the waiting state from Toolhelp,
    // so we use the thread's kernel state field (tpDeltaPri as a heuristic).
    // A full state requires NtQuerySystemInformation which we defer to Phase 2.
    let state = ThreadState::Unknown;

    ThreadEntry {
        tid,
        owner_pid: pid,
        state,
        cpu_time_ms,
        priority,
        start_module,
        start_address,
        suspicious,
    }
}

fn query_thread_details(tid: u32) -> WinResult<(u64, u64, String, bool)> {
    // Safety: tid is a valid thread ID from the snapshot.
    let handle = unsafe { OpenThread(THREAD_QUERY_INFORMATION, false, tid)? };

    let cpu_time_ms = get_thread_cpu_time(handle).unwrap_or(0);

    // Resolve start address via NtQueryInformationThread (loaded dynamically).
    let start_address = get_thread_start_address(handle).unwrap_or(0);
    let (start_module, suspicious) = resolve_module(handle, start_address);

    unsafe { let _ = CloseHandle(handle); }

    Ok((cpu_time_ms, start_address, start_module, suspicious))
}

fn get_thread_cpu_time(handle: windows::Win32::Foundation::HANDLE) -> Option<u64> {
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();

    // Safety: handle is valid; all pointers point to valid FILETIME structs.
    unsafe {
        GetThreadTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user).ok()?;
    }

    let kernel_ms = filetime_to_ms(&kernel);
    let user_ms = filetime_to_ms(&user);
    Some(kernel_ms + user_ms)
}

fn filetime_to_ms(ft: &FILETIME) -> u64 {
    let ticks = ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64;
    ticks / 10_000 // 100-nanosecond intervals → milliseconds
}

fn get_thread_start_address(handle: windows::Win32::Foundation::HANDLE) -> Option<u64> {
    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    use windows::core::PCSTR;

    // Load NtQueryInformationThread dynamically — it's undocumented.
    let ntdll = unsafe { GetModuleHandleW(windows::core::w!("ntdll.dll")).ok()? };

    // SAFETY: "NtQueryInformationThread\0" is a valid null-terminated C string.
    let proc_addr = unsafe {
        GetProcAddress(ntdll, PCSTR(b"NtQueryInformationThread\0".as_ptr()))
    }?;

    // Signature: NtQueryInformationThread(HANDLE, THREADINFOCLASS, PVOID, ULONG, PULONG)
    type NtQueryInformationThreadFn = unsafe extern "system" fn(
        windows::Win32::Foundation::HANDLE,
        u32,
        *mut std::ffi::c_void,
        u32,
        *mut u32,
    ) -> i32;

    let nt_query: NtQueryInformationThreadFn = unsafe { std::mem::transmute(proc_addr) };

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

    if status == 0 {
        Some(start_address)
    } else {
        None
    }
}

fn resolve_module(
    handle: windows::Win32::Foundation::HANDLE,
    _start_address: u64,
) -> (String, bool) {
    // Phase 2 will implement full module enumeration via EnumProcessModules
    // and compare start_address against loaded module ranges.
    // For Phase 1, return a placeholder.
    let _ = handle;
    ("?".into(), false)
}
