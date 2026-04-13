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
    /// True for known virtual/hypervisor/tunnel adapters (Hyper-V, Docker, VMware, WSL, …).
    pub is_virtual: bool,
}

/// Returns true if the adapter description matches known virtual/software adapter patterns.
pub fn detect_virtual(display_name: &str) -> bool {
    let lower = display_name.to_lowercase();
    lower.contains("hyper-v")
        || lower.contains("docker")
        || lower.contains("vmware")
        || lower.contains("virtualbox")
        || lower.contains("wsl")
        || lower.contains("vethernet")
        || lower.contains("tap-windows")
        || lower.contains("tap-nordvpn")
        || lower.contains("tap adapter")
        || lower.contains("bluetooth")
        || lower.contains("teredo")
        || lower.contains("isatap")
        || lower.contains("loopback")
        || lower.contains("pseudo")
        || lower.contains("tunnel")
        || lower.contains("6to4")
        || lower.contains("nordvpn")
        || lower.contains("nordlynx")
        || lower.contains("openvpn")
        || lower.contains("wireguard")
        || lower.contains("data channel offload")
        || lower.contains("virtual ethernet")
        || lower.contains("virtual switch")
        || lower.contains("wan miniport")
        || lower.contains("ip-https")
        || lower.contains("microsoft teredo")
        || lower.contains("microsoft 6to4")
        || lower.contains("microsoft ip-https")
        || lower.contains("microsoft tunnel")
}
