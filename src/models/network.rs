/// Per-network-adapter snapshot.
#[derive(Clone, Debug, Default)]
pub struct NetSnapshot {
    /// Internal GUID-style name.
    pub adapter_name: String,
    /// Human-readable description from MIB_IF_ROW2.
    pub display_name: String,
    pub rx_bps: u64,
    pub tx_bps: u64,
    pub rx_total_bytes: u64,
    pub tx_total_bytes: u64,
    pub is_up: bool,
    pub mac_address: String,
}
