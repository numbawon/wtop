use crate::models::network::NetSnapshot;
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

        let mut result = Vec::new();
        for entry in &raw {
            let prev_rx = self
                .last_snapshot
                .iter()
                .find(|s| s.adapter_name == entry.adapter_name)
                .map(|s| s.rx_total_bytes)
                .unwrap_or(entry.rx_total_bytes);

            let prev_tx = self
                .last_snapshot
                .iter()
                .find(|s| s.adapter_name == entry.adapter_name)
                .map(|s| s.tx_total_bytes)
                .unwrap_or(entry.tx_total_bytes);

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
            if row.Type == IF_TYPE_SOFTWARE_LOOPBACK as u32 {
                continue;
            }

            let adapter_name = format!("{{{}}}", unsafe {
                format!(
                    "{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
                    row.InterfaceLuid.Value,
                    0u32, 0u32, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8
                )
            });

            // Decode the Description wide string.
            let desc_slice = &row.Description;
            let desc_len = desc_slice.iter().position(|&c| c == 0).unwrap_or(desc_slice.len());
            let display_name = String::from_utf16_lossy(&desc_slice[..desc_len]);

            let mac: Vec<String> = row.PhysicalAddress[..row.PhysicalAddressLength as usize]
                .iter()
                .map(|b| format!("{b:02X}"))
                .collect();

            entries.push(NetSnapshot {
                adapter_name,
                display_name,
                rx_bps: 0,
                tx_bps: 0,
                rx_total_bytes: row.InOctets,
                tx_total_bytes: row.OutOctets,
                is_up: row.OperStatus.0 == IF_OPER_STATUS_UP,
                mac_address: mac.join(":"),
            });
        }

        // Safety: FreeMibTable frees the buffer allocated by GetIfTable2Ex.
        unsafe { FreeMibTable(table_ptr as *mut _) };

        Some(entries)
    }
}
