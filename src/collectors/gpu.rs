use windows::Win32::Devices::DeviceAndDriverInstallation::{
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInfo, SetupDiGetClassDevsW,
    SetupDiGetDeviceRegistryPropertyW, DIGCF_ALLCLASSES, DIGCF_PRESENT, HDEVINFO,
    SP_DEVINFO_DATA, SPDRP_DEVICEDESC, SPDRP_FRIENDLYNAME,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIAdapter1, IDXGIFactory1,
    DXGI_ADAPTER_FLAG_SOFTWARE,
};
use windows::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData,
    PdhGetFormattedCounterArrayW, PdhOpenQueryW, PDH_FMT_COUNTERVALUE_ITEM_W,
    PDH_FMT_DOUBLE,
};
use windows::core::PCWSTR;

use crate::models::gpu::GpuAdapter;
use crate::models::npu::NpuAdapter;
use crate::models::RingBuffer;

const PDH_MORE_DATA: u32 = 0x800007D2;

struct AdapterInfo {
    name: String,
    /// "luid_0xHHHHHHHH_0xLLLLLLLL" - prefix used to match PDH counter instances.
    luid_prefix: String,
    vram_total_bytes: u64,
}

pub struct GpuCollector {
    adapters: Vec<AdapterInfo>,
    npu_adapters: Vec<AdapterInfo>,
    query: isize,
    counter_vram: isize,
    counter_util: isize,
    /// False when no GPU adapters are present (VRAM counter was not added).
    has_vram: bool,
    valid: bool,
    scratch_buf: Vec<u8>,
}

unsafe impl Send for GpuCollector {}

impl GpuCollector {
    pub fn new(history_len: usize) -> Self {
        let _ = history_len;
        let dead = Self {
            adapters: Vec::new(),
            npu_adapters: Vec::new(),
            query: 0,
            counter_vram: 0,
            counter_util: 0,
            has_vram: false,
            valid: false,
            scratch_buf: Vec::new(),
        };

        let (gpu_adapters, npu_adapters) = match enumerate_adapters() {
            Ok(pair) if !pair.0.is_empty() || !pair.1.is_empty() => pair,
            _ => return dead,
        };

        unsafe {
            let mut query: isize = 0;
            if PdhOpenQueryW(PCWSTR::null(), 0, &mut query) != 0 {
                tracing::warn!("GpuCollector: PdhOpenQueryW failed");
                return dead;
            }

            let mut counter_vram: isize = 0;
            let has_vram = !gpu_adapters.is_empty() && {
                PdhAddEnglishCounterW(
                    query,
                    windows::core::w!(r"\GPU Adapter Memory(*)\Dedicated Usage"),
                    0,
                    &mut counter_vram,
                ) == 0
            };

            if !gpu_adapters.is_empty() && !has_vram {
                tracing::warn!("GpuCollector: failed to add VRAM counter");
                PdhCloseQuery(query);
                return dead;
            }

            let mut counter_util: isize = 0;
            if PdhAddEnglishCounterW(
                query,
                windows::core::w!(r"\GPU Engine(*)\Utilization Percentage"),
                0,
                &mut counter_util,
            ) != 0
            {
                tracing::warn!("GpuCollector: failed to add utilization counter");
                PdhCloseQuery(query);
                return dead;
            }

            PdhCollectQueryData(query);

            Self {
                adapters: gpu_adapters,
                npu_adapters,
                query,
                counter_vram,
                counter_util,
                has_vram,
                valid: true,
                scratch_buf: Vec::new(),
            }
        }
    }

    pub fn collect(&mut self, _history_len: usize) -> (Vec<GpuAdapter>, Vec<NpuAdapter>) {
        if !self.valid {
            return (Vec::new(), Vec::new());
        }

        unsafe {
            if PdhCollectQueryData(self.query) != 0 {
                return (Vec::new(), Vec::new());
            }
        }

        let vram_samples = if self.has_vram {
            self.sample_counter(self.counter_vram)
        } else {
            Vec::new()
        };
        let util_samples = self.sample_counter(self.counter_util);

        // ── GPU adapters ──────────────────────────────────────────────────────
        let mut gpu_result: Vec<GpuAdapter> = self.adapters.iter().map(|a| {
            GpuAdapter {
                name: a.name.clone(),
                vram_total_bytes: a.vram_total_bytes,
                vram_used_bytes: 0,
                utilization_pct: 0.0,
                util_history: RingBuffer::new(0),
            }
        }).collect();

        for (instance, bytes) in &vram_samples {
            let lower = instance.to_lowercase();
            for (i, adapter) in self.adapters.iter().enumerate() {
                if lower.contains(&adapter.luid_prefix) {
                    gpu_result[i].vram_used_bytes = *bytes;
                    break;
                }
            }
        }

        let mut util_sums: Vec<f64> = vec![0.0; self.adapters.len()];
        let mut util_counts: Vec<u32> = vec![0; self.adapters.len()];
        for (instance, pct_u64) in &util_samples {
            let lower = instance.to_lowercase();
            if !lower.contains("engtype_3d") {
                continue;
            }
            let pct = *pct_u64 as f64 / 100.0;
            for (i, adapter) in self.adapters.iter().enumerate() {
                if lower.contains(&adapter.luid_prefix) {
                    util_sums[i] += pct;
                    util_counts[i] += 1;
                    break;
                }
            }
        }
        for (i, gpu) in gpu_result.iter_mut().enumerate() {
            if util_counts[i] > 0 {
                gpu.utilization_pct = (util_sums[i] / util_counts[i] as f64).clamp(0.0, 100.0) as f32;
            }
        }

        // ── NPU adapters - sum all engine types for the LUID ─────────────────
        let mut npu_result: Vec<NpuAdapter> = self.npu_adapters.iter().map(|a| {
            NpuAdapter {
                name: a.name.clone(),
                utilization_pct: 0.0,
                util_history: RingBuffer::new(0),
            }
        }).collect();

        let mut npu_sums: Vec<f64> = vec![0.0; self.npu_adapters.len()];
        let mut npu_counts: Vec<u32> = vec![0; self.npu_adapters.len()];
        for (instance, pct_u64) in &util_samples {
            let lower = instance.to_lowercase();
            let pct = *pct_u64 as f64 / 100.0;
            for (i, adapter) in self.npu_adapters.iter().enumerate() {
                if !adapter.luid_prefix.is_empty() && lower.contains(&adapter.luid_prefix) {
                    npu_sums[i] += pct;
                    npu_counts[i] += 1;
                    break;
                }
            }
        }
        for (i, npu) in npu_result.iter_mut().enumerate() {
            if npu_counts[i] > 0 {
                npu.utilization_pct = (npu_sums[i] / npu_counts[i] as f64).clamp(0.0, 100.0) as f32;
            }
        }

        (gpu_result, npu_result)
    }

    fn sample_counter(&mut self, counter: isize) -> Vec<(String, u64)> {
        unsafe {
            let mut buf_size: u32 = 0;
            let mut item_count: u32 = 0;

            let status = PdhGetFormattedCounterArrayW(
                counter,
                PDH_FMT_DOUBLE,
                &mut buf_size,
                &mut item_count,
                None,
            );

            if item_count == 0 || buf_size == 0 {
                return Vec::new();
            }
            if status != PDH_MORE_DATA && status != 0 {
                return Vec::new();
            }

            self.scratch_buf.clear();
            self.scratch_buf.resize(buf_size as usize, 0u8);
            let items_ptr = self.scratch_buf.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;

            let status2 = PdhGetFormattedCounterArrayW(
                counter,
                PDH_FMT_DOUBLE,
                &mut buf_size,
                &mut item_count,
                Some(items_ptr),
            );
            if status2 != 0 {
                return Vec::new();
            }

            let items = std::slice::from_raw_parts(items_ptr, item_count as usize);
            let mut out = Vec::with_capacity(item_count as usize);

            for item in items {
                let ptr = item.szName.0;
                if ptr.is_null() {
                    continue;
                }
                let len = (0usize..).take_while(|&i| *ptr.add(i) != 0).count();
                let name = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
                let value = item.FmtValue.Anonymous.doubleValue
                    .max(0.0)
                    .min(u64::MAX as f64) as u64;
                out.push((name, value));
            }

            out
        }
    }
}

impl Drop for GpuCollector {
    fn drop(&mut self) {
        if self.valid {
            unsafe {
                PdhCloseQuery(self.query);
            }
        }
    }
}

fn is_npu(name: &str) -> bool {
    let lc = name.to_lowercase();
    lc.contains("npu")
        || lc.contains("neural")
        || lc.contains("ai boost")   // Intel NPU (Meteor Lake)
        || lc.contains("xdna")       // AMD XDNA
        || lc.contains("ryzen ai")   // AMD Ryzen AI branding
        || lc.contains("amd ai")     // AMD AI Engine
        || lc.contains("myriad")     // Intel Myriad VPU
        || lc.contains("vpu")
        || lc.contains("hexagon")    // Qualcomm Hexagon NPU
        // "ipu" intentionally excluded: matches Intel camera Image Processing Units (USB)
}

fn enumerate_adapters() -> windows::core::Result<(Vec<AdapterInfo>, Vec<AdapterInfo>)> {
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }?;
    let mut gpus = Vec::new();
    let mut npus = Vec::new();

    for i in 0u32.. {
        let adapter: IDXGIAdapter1 = match unsafe { factory.EnumAdapters1(i) } {
            Ok(a) => a,
            Err(_) => break,
        };
        let desc = unsafe { adapter.GetDesc1() }?;

        // Skip software adapters (WARP, Microsoft Basic Render Driver).
        if (desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) != 0 {
            continue;
        }

        let raw = String::from_utf16_lossy(&desc.Description);
        let name = raw.split('\0').next().unwrap_or("").trim().to_string();

        if name.is_empty() || name.contains("Microsoft Basic Render Driver") {
            continue;
        }

        let luid = desc.AdapterLuid;
        let luid_prefix = format!(
            "luid_0x{:08x}_0x{:08x}",
            luid.HighPart as u32,
            luid.LowPart,
        );

        let info = AdapterInfo {
            name: name.clone(),
            luid_prefix,
            vram_total_bytes: desc.DedicatedVideoMemory as u64,
        };

        if is_npu(&name) {
            npus.push(info);
        } else {
            gpus.push(info);
        }
    }

    // Supplement with NPUs visible in Device Manager but not in DXGI (e.g. AMD XDNA).
    for name in scan_npu_device_names() {
        let already_found = npus.iter().any(|n| n.name.eq_ignore_ascii_case(&name));
        if !already_found {
            npus.push(AdapterInfo {
                name,
                luid_prefix: String::new(), // no PDH LUID available
                vram_total_bytes: 0,
            });
        }
    }

    Ok((gpus, npus))
}

/// Enumerate present PCI devices via SetupAPI and return names of any NPU-like devices.
/// Restricts to PCI bus to avoid false positives from USB devices. Runs once at startup.
fn scan_npu_device_names() -> Vec<String> {
    let mut result = Vec::new();

    // Enumerate PCI bus only - NPUs are PCI devices; USB/HID devices are excluded.
    let enumerator = windows::core::w!("PCI");
    let devs = match unsafe {
        SetupDiGetClassDevsW(None, enumerator, None, DIGCF_PRESENT | DIGCF_ALLCLASSES)
    } {
        Ok(h) if !h.is_invalid() => h,
        _ => return result,
    };

    let mut dev_info = SP_DEVINFO_DATA {
        cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32,
        ..Default::default()
    };

    let mut i = 0u32;
    loop {
        if unsafe { SetupDiEnumDeviceInfo(devs, i, &mut dev_info) }.is_err() {
            break;
        }
        i += 1;

        // Prefer device description (reliable); friendly name is often vendor-branded.
        let name = read_device_property(devs, &dev_info, SPDRP_DEVICEDESC)
            .or_else(|| read_device_property(devs, &dev_info, SPDRP_FRIENDLYNAME));

        if let Some(n) = name {
            if is_npu(&n) {
                result.push(n);
            }
        }
    }

    unsafe { let _ = SetupDiDestroyDeviceInfoList(devs); }
    result
}

fn read_device_property(
    devs: HDEVINFO,
    dev_info: &SP_DEVINFO_DATA,
    property: windows::Win32::Devices::DeviceAndDriverInstallation::SETUP_DI_REGISTRY_PROPERTY,
) -> Option<String> {
    let mut buf = vec![0u8; 512];
    let ok = unsafe {
        SetupDiGetDeviceRegistryPropertyW(
            devs,
            dev_info as *const SP_DEVINFO_DATA,
            property,
            None,
            Some(&mut buf),
            None,
        )
    };
    if ok.is_err() {
        return None;
    }
    // Buffer is REG_SZ (UTF-16LE).
    let words: Vec<u16> = buf
        .chunks_exact(2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .collect();
    let raw = String::from_utf16_lossy(&words);
    let s = raw.split('\0').next().unwrap_or("").trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}
