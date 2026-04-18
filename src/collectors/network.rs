use crate::models::network::{detect_virtual, NetSnapshot};
use std::collections::HashMap;
use std::fmt::Write as FmtWrite;
use std::time::Instant;
use windows::Win32::NetworkManagement::IpHelper::{
    FreeMibTable, GetIfTable2Ex, MIB_IF_TABLE2, MibIfTableNormal,
};
// IF_TYPE_SOFTWARE_LOOPBACK = 24, IfOperStatusUp = 1 (numeric constants from Windows SDK)
const IF_TYPE_SOFTWARE_LOOPBACK: u32 = 24;
const IF_OPER_STATUS_UP: i32 = 1;

pub struct NetCollector {
    last_snapshot: Vec<NetSnapshot>,
    last_time: Option<Instant>,
}

impl NetCollector {
    pub fn new() -> Self {
        Self {
            last_snapshot: Vec::new(),
            last_time: None,
        }
    }

    pub fn collect(&mut self) -> Vec<NetSnapshot> {
        let now = Instant::now();
        let elapsed_secs = self
            .last_time
            .map(|t| t.elapsed().as_secs_f64().max(0.001))
            .unwrap_or(1.0);

        let raw = match self.get_raw_table() {
            Some(r) => r,
            None => return self.last_snapshot.clone(),
        };

        // Build a lookup map once instead of scanning linearly for each adapter.
        let prev_map: HashMap<&str, (u64, u64)> = self
            .last_snapshot
            .iter()
            .map(|s| (s.adapter_name.as_str(), (s.rx_total_bytes, s.tx_total_bytes)))
            .collect();

        let mut result = Vec::new();
        for entry in &raw {
            let (prev_rx, prev_tx) = prev_map
                .get(entry.adapter_name.as_str())
                .copied()
                .unwrap_or((entry.rx_total_bytes, entry.tx_total_bytes));

            let rx_bps = ((entry.rx_total_bytes.saturating_sub(prev_rx)) as f64
                / elapsed_secs) as u64;
            let tx_bps = ((entry.tx_total_bytes.saturating_sub(prev_tx)) as f64
                / elapsed_secs) as u64;

            result.push(NetSnapshot {
                adapter_name: entry.adapter_name.clone(),
                display_name: entry.display_name.clone(),
                rx_bps,
                tx_bps,
                rx_total_bytes: entry.rx_total_bytes,
                tx_total_bytes: entry.tx_total_bytes,
                is_up: entry.is_up,
                mac_address: entry.mac_address.clone(),
                is_virtual: entry.is_virtual,
            });
        }

        self.last_snapshot = result.clone();
        self.last_time = Some(now);
        result
    }

    fn get_raw_table(&self) -> Option<Vec<NetSnapshot>> {
        let mut table_ptr: *mut MIB_IF_TABLE2 = std::ptr::null_mut();

        // Safety: we check the result before dereferencing the pointer.
        let result = unsafe { GetIfTable2Ex(MibIfTableNormal, &mut table_ptr) };
        if result.is_err() || table_ptr.is_null() {
            return None;
        }

        let table = unsafe { &*table_ptr };
        let num_entries = table.NumEntries as usize;

        // Safety: NumEntries is set by the Windows API, Table is a valid array.
        let rows = unsafe {
            std::slice::from_raw_parts(table.Table.as_ptr(), num_entries)
        };

        let mut entries = Vec::new();
        for row in rows {
            // Skip loopback adapters.
            if row.Type == IF_TYPE_SOFTWARE_LOOPBACK {
                continue;
            }

            let g = row.InterfaceGuid;
            let adapter_name = format!(
                "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
                g.data1, g.data2, g.data3,
                g.data4[0], g.data4[1],
                g.data4[2], g.data4[3], g.data4[4],
                g.data4[5], g.data4[6], g.data4[7]
            );

            // Decode the Description wide string.
            let desc_slice = &row.Description;
            let desc_len = desc_slice.iter().position(|&c| c == 0).unwrap_or(desc_slice.len());
            let display_name = String::from_utf16_lossy(&desc_slice[..desc_len]);

            let addr_bytes = &row.PhysicalAddress[..row.PhysicalAddressLength as usize];
            let mut mac = String::with_capacity(addr_bytes.len() * 3);
            for (i, b) in addr_bytes.iter().enumerate() {
                if i > 0 { mac.push(':'); }
                let _ = write!(mac, "{b:02X}");
            }

            let is_virtual = detect_virtual(&display_name);
            entries.push(NetSnapshot {
                adapter_name,
                is_virtual,
                display_name,
                rx_bps: 0,
                tx_bps: 0,
                rx_total_bytes: row.InOctets,
                tx_total_bytes: row.OutOctets,
                is_up: row.OperStatus.0 == IF_OPER_STATUS_UP,
                mac_address: mac,
            });
        }

        // Safety: FreeMibTable frees the buffer allocated by GetIfTable2Ex.
        unsafe { FreeMibTable(table_ptr as *mut _) };

        // Windows creates multiple MIB_IF_ROW2 entries for the same physical
        // adapter - one per protocol binding or filter driver layer - all sharing
        // the same MAC address but with different GUIDs. Use the MAC as the
        // canonical identity for physical adapters so we show one row per device.
        // Software adapters (tunnels, WAN miniports) have no MAC, so fall back
        // to display_name for those.
        let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        let mut deduped: Vec<NetSnapshot> = Vec::new();
        for entry in entries {
            // Physical adapters share a MAC across all their logical interfaces;
            // virtual/no-MAC adapters are keyed by description.
            let key = if !entry.mac_address.is_empty() {
                entry.mac_address.clone()
            } else {
                entry.display_name.clone()
            };
            if let Some(&idx) = seen.get(&key) {
                // Keep whichever entry has seen the most traffic - that's the
                // "real" data-path interface for this device.
                let new_total = entry.rx_total_bytes + entry.tx_total_bytes;
                let old_total = deduped[idx].rx_total_bytes + deduped[idx].tx_total_bytes;
                if new_total > old_total {
                    let old_key = key.clone();
                    deduped[idx] = entry;
                    seen.insert(old_key, idx);
                }
            } else {
                seen.insert(key, deduped.len());
                deduped.push(entry);
            }
        }

        Some(deduped)
    }
}
