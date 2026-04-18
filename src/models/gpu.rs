use crate::models::RingBuffer;

#[derive(Clone, Debug)]
pub struct GpuAdapter {
    pub name: String,
    pub vram_total_bytes: u64,
    pub vram_used_bytes: u64,
    pub utilization_pct: f32,
    pub util_history: RingBuffer<f32>,
}
