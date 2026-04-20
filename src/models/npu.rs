use crate::models::RingBuffer;

#[derive(Clone, Debug)]
pub struct NpuAdapter {
    pub name: String,
    pub utilization_pct: f32,
    pub util_history: RingBuffer<f32>,
}
