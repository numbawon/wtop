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
    query: isize,
    counter_vram: isize,
    counter_util: isize,
    valid: bool,
    scratch_buf: Vec<u8>,
}

unsafe impl Send for GpuCollector {}

impl GpuCollector {
    pub fn new(history_len: usize) -> Self {
        let _ = history_len;
        let dead = Self {
            adapters: Vec::new(),
            query: 0,
            counter_vram: 0,
            counter_util: 0,
            valid: false,
            scratch_buf: Vec::new(),
        };

        let adapters = match enumerate_adapters() {
            Ok(a) if !a.is_empty() => a,
            _ => return dead,
        };

        unsafe {
            let mut query: isize = 0;
            if PdhOpenQueryW(PCWSTR::null(), 0, &mut query) != 0 {
                tracing::warn!("GpuCollector: PdhOpenQueryW failed");
                return dead;
            }

            let mut counter_vram: isize = 0;
            let mut counter_util: isize = 0;

            if PdhAddEnglishCounterW(
                query,
                windows::core::w!(r"\GPU Adapter Memory(*)\Dedicated Usage"),
                0,
                &mut counter_vram,
            ) != 0
            {
                tracing::warn!("GpuCollector: failed to add VRAM counter");
                PdhCloseQuery(query);
                return dead;
            }

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
                adapters,
                query,
                counter_vram,
                counter_util,
                valid: true,
                scratch_buf: Vec::new(),
            }
        }
    }

    pub fn collect(&mut self, history_len: usize) -> Vec<GpuAdapter> {
        if !self.valid {
            return Vec::new();
        }

        unsafe {
            if PdhCollectQueryData(self.query) != 0 {
                return Vec::new();
            }
        }

        let vram_samples = self.sample_counter(self.counter_vram);
        let util_samples = self.sample_counter(self.counter_util);

        let mut result: Vec<GpuAdapter> = self.adapters.iter().map(|a| {
            GpuAdapter {
                name: a.name.clone(),
                vram_total_bytes: a.vram_total_bytes,
                vram_used_bytes: 0,
                utilization_pct: 0.0,
                util_history: RingBuffer::new(history_len),
            }
        }).collect();

        for (instance, bytes) in &vram_samples {
            let lower = instance.to_lowercase();
            for (i, adapter) in self.adapters.iter().enumerate() {
                if lower.contains(&adapter.luid_prefix) {
                    result[i].vram_used_bytes = *bytes;
                    break;
                }
            }
        }

        // Sum utilization from all 3D engine instances per adapter LUID.
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
        for (i, gpu) in result.iter_mut().enumerate() {
            if util_counts[i] > 0 {
                gpu.utilization_pct = (util_sums[i] / util_counts[i] as f64).clamp(0.0, 100.0) as f32;
            }
        }

        result
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

fn enumerate_adapters() -> windows::core::Result<Vec<AdapterInfo>> {
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }?;
    let mut adapters = Vec::new();

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

        let name = String::from_utf16_lossy(&desc.Description)
            .trim_end_matches('\0')
            .to_string();

        if name.is_empty() || name.contains("Microsoft Basic Render Driver") {
            continue;
        }

        let luid = desc.AdapterLuid;
        // PDH instance format: "luid_0x<HighPart>_0x<LowPart>_..."
        let luid_prefix = format!(
            "luid_0x{:08x}_0x{:08x}",
            luid.HighPart as u32,
            luid.LowPart,
        );

        adapters.push(AdapterInfo {
            name,
            luid_prefix,
            vram_total_bytes: desc.DedicatedVideoMemory as u64,
        });
    }

    Ok(adapters)
}
