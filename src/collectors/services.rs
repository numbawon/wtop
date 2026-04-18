use std::slice;

use windows::Win32::System::Services::{
    CloseServiceHandle, EnumServicesStatusExW, OpenSCManagerW, OpenServiceW,
    QueryServiceConfigW, ENUM_SERVICE_STATUS_PROCESSW, QUERY_SERVICE_CONFIGW,
    SC_ENUM_PROCESS_INFO, SC_MANAGER_ENUMERATE_SERVICE, SERVICE_QUERY_CONFIG,
    SERVICE_STATE_ALL, SERVICE_WIN32,
};
use windows::core::PCWSTR;

use crate::models::services::{ServiceEntry, ServiceStartType, ServiceStatus};

pub struct ServicesCollector;

impl ServicesCollector {
    pub fn new() -> Self { Self }

    pub fn collect(&self) -> Vec<ServiceEntry> {
        unsafe { collect_inner() }.unwrap_or_default()
    }
}

unsafe fn collect_inner() -> windows::core::Result<Vec<ServiceEntry>> {
    let scm = OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_ENUMERATE_SERVICE)?;

    let mut bytes_needed: u32 = 0;
    let mut services_returned: u32 = 0;
    let mut resume_handle: u32 = 0;

    // First call: get required buffer size.
    let _ = EnumServicesStatusExW(
        scm,
        SC_ENUM_PROCESS_INFO,
        SERVICE_WIN32,
        SERVICE_STATE_ALL,
        None,
        &mut bytes_needed,
        &mut services_returned,
        Some(&mut resume_handle),
        PCWSTR::null(),
    );

    if bytes_needed == 0 {
        let _ = CloseServiceHandle(scm);
        return Ok(Vec::new());
    }

    let mut buffer: Vec<u8> = vec![0u8; bytes_needed as usize];
    services_returned = 0;
    resume_handle = 0;

    EnumServicesStatusExW(
        scm,
        SC_ENUM_PROCESS_INFO,
        SERVICE_WIN32,
        SERVICE_STATE_ALL,
        Some(buffer.as_mut_slice()),
        &mut bytes_needed,
        &mut services_returned,
        Some(&mut resume_handle),
        PCWSTR::null(),
    )?;

    let entries: &[ENUM_SERVICE_STATUS_PROCESSW] = slice::from_raw_parts(
        buffer.as_ptr() as *const ENUM_SERVICE_STATUS_PROCESSW,
        services_returned as usize,
    );

    let mut result = Vec::with_capacity(entries.len());

    for entry in entries {
        let name = entry.lpServiceName.to_string().unwrap_or_default();
        let display_name = entry.lpDisplayName.to_string().unwrap_or_default();
        let pid = entry.ServiceStatusProcess.dwProcessId;

        let status = match entry.ServiceStatusProcess.dwCurrentState.0 {
            1 => ServiceStatus::Stopped,
            2 => ServiceStatus::StartPending,
            3 => ServiceStatus::StopPending,
            4 => ServiceStatus::Running,
            5 => ServiceStatus::ContinuePending,
            6 => ServiceStatus::PausePending,
            7 => ServiceStatus::Paused,
            _ => ServiceStatus::Unknown,
        };

        let start_type = query_start_type(scm, entry.lpServiceName);

        result.push(ServiceEntry { name, display_name, status, start_type, pid });
    }

    let _ = CloseServiceHandle(scm);
    Ok(result)
}

unsafe fn query_start_type(
    scm: windows::Win32::System::Services::SC_HANDLE,
    service_name: windows::core::PWSTR,
) -> ServiceStartType {
    let svc = match OpenServiceW(scm, service_name, SERVICE_QUERY_CONFIG) {
        Ok(h) => h,
        Err(_) => return ServiceStartType::Unknown,
    };

    let mut bytes_needed: u32 = 0;
    let _ = QueryServiceConfigW(svc, None, 0, &mut bytes_needed);

    if bytes_needed == 0 {
        let _ = CloseServiceHandle(svc);
        return ServiceStartType::Unknown;
    }

    let mut buf: Vec<u8> = vec![0u8; bytes_needed as usize];
    let cfg_ptr = buf.as_mut_ptr() as *mut QUERY_SERVICE_CONFIGW;

    let result = if QueryServiceConfigW(svc, Some(&mut *cfg_ptr), bytes_needed, &mut bytes_needed).is_ok() {
        match (*cfg_ptr).dwStartType.0 {
            0 => ServiceStartType::Boot,
            1 => ServiceStartType::System,
            2 => ServiceStartType::Auto,
            3 => ServiceStartType::Manual,
            4 => ServiceStartType::Disabled,
            _ => ServiceStartType::Unknown,
        }
    } else {
        ServiceStartType::Unknown
    };

    let _ = CloseServiceHandle(svc);
    result
}
