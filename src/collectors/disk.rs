use crate::models::disk::DiskSnapshot;
use windows::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData,
    PdhGetFormattedCounterArrayW, PdhOpenQueryW, PDH_FMT_COUNTERVALUE_ITEM_W,
    PDH_FMT_DOUBLE,
};
use windows::core::PCWSTR;

const PDH_MORE_DATA: u32 = 0x800007D2;

/// Disk I/O collector backed by Windows PDH (Performance Data Helper).
///
/// PDH rate counters need two `PdhCollectQueryData` calls before they return
/// meaningful bytes/sec values. We prime the first sample in `new()`, so the
/// first call to `collect()` already returns valid (though possibly zero for
/// idle disks) rates.
pub struct DiskCollector {
    query: isize,
    counter_read: isize,
    counter_write: isize,
    counter_util: isize,
    valid: bool,
    /// Reusable byte buffer for PDH counter array reads — grows to the
    /// largest observed buffer size, then stays there.
    scratch_buf: Vec<u8>,
}

// PDH handles are not Send by default (Windows raw handles / isize).
// DiskCollector lives exclusively on its own background thread, so it is
// safe to mark it Send here.
unsafe impl Send for DiskCollector {}

impl DiskCollector {
    pub fn new() -> Self {
        let dead = Self {
            query: 0,
            counter_read: 0,
            counter_write: 0,
            counter_util: 0,
            valid: false,
            scratch_buf: Vec::new(),
        };

        unsafe {
            let mut query: isize = 0;
            if PdhOpenQueryW(PCWSTR::null(), 0, &mut query) != 0 {
                tracing::warn!("DiskCollector: PdhOpenQueryW failed");
                return dead;
            }

            let mut counter_read: isize = 0;
            let mut counter_write: isize = 0;
            let mut counter_util: isize = 0;

            if PdhAddEnglishCounterW(
                query,
                windows::core::w!(r"\PhysicalDisk(*)\Disk Read Bytes/sec"),
                0,
                &mut counter_read,
            ) != 0
            {
                tracing::warn!("DiskCollector: failed to add read counter");
                PdhCloseQuery(query);
                return dead;
            }
            if PdhAddEnglishCounterW(
                query,
                windows::core::w!(r"\PhysicalDisk(*)\Disk Write Bytes/sec"),
                0,
                &mut counter_write,
            ) != 0
            {
                tracing::warn!("DiskCollector: failed to add write counter");
                PdhCloseQuery(query);
                return dead;
            }
            if PdhAddEnglishCounterW(
                query,
                windows::core::w!(r"\PhysicalDisk(*)\% Disk Time"),
                0,
                &mut counter_util,
            ) != 0
            {
                tracing::warn!("DiskCollector: failed to add utilization counter");
                PdhCloseQuery(query);
                return dead;
            }

            // First collection primes the rate counters. The next call to
            // collect() will return valid bytes/sec deltas.
            PdhCollectQueryData(query);

            Self {
                query,
                counter_read,
                counter_write,
                counter_util,
                valid: true,
                scratch_buf: Vec::new(),
            }
        }
    }

    pub fn collect(&mut self) -> Vec<DiskSnapshot> {
        if !self.valid {
            return Vec::new();
        }

        unsafe {
            if PdhCollectQueryData(self.query) != 0 {
                return Vec::new();
            }
        }

        // Copy the counter handles (isize = Copy) before the &mut self borrow.
        let (cr, cw, cu) = (self.counter_read, self.counter_write, self.counter_util);
        let reads = self.sample_counter(cr);
        let writes = self.sample_counter(cw);
        let utils = self.sample_counter(cu);

        // Merge the three counter arrays by instance name, dropping the
        // synthetic "_Total" rollup that PDH always includes.
        let mut result = Vec::new();
        for (name, read_bps) in &reads {
            if name == "_Total" {
                continue;
            }
            let write_bps = writes
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| *v)
                .unwrap_or(0);
            let util_pct = utils
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| *v as f32)
                .unwrap_or(0.0)
                .clamp(0.0, 100.0);

            result.push(DiskSnapshot {
                device_name: name.clone(),
                read_bps: *read_bps,
                write_bps,
                utilization_pct: util_pct,
            });
        }

        result
    }

    /// Read all instances of a wildcard PDH counter as `(instance_name, u64_value)`.
    fn sample_counter(&mut self, counter: isize) -> Vec<(String, u64)> {
        unsafe {
            let mut buf_size: u32 = 0;
            let mut item_count: u32 = 0;

            // Sizing call: PDH returns PDH_MORE_DATA with the required buffer size.
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

            // Reuse the scratch buffer — grows to the high-water mark and stays there.
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
                let name =
                    String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
                // doubleValue is the bytes/sec (or percent for util).
                // Clamp before cast: casting f64 > u64::MAX is UB in Rust.
                let value = item.FmtValue.Anonymous.doubleValue
                    .max(0.0)
                    .min(u64::MAX as f64) as u64;
                out.push((name, value));
            }

            out
        }
    }
}

impl Drop for DiskCollector {
    fn drop(&mut self) {
        if self.valid {
            unsafe {
                PdhCloseQuery(self.query);
            }
        }
    }
}
