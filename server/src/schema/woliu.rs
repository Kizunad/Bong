use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VortexFieldStateV1 {
    pub caster: String,
    pub active: bool,
    pub center: [f64; 3],
    pub radius: f32,
    pub delta: f32,
    pub env_qi_at_cast: f32,
    pub maintain_remaining_ticks: u64,
    pub intercepted_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VortexBackfireCauseV1 {
    EnvQiTooLow,
    ExceedMaintainMax,
    ExceedDeltaCap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VortexBackfireEventV1 {
    pub caster: String,
    pub cause: VortexBackfireCauseV1,
    pub meridian_severed: String,
    pub tick: u64,
    pub env_qi: f32,
    pub delta: f32,
    pub resisted: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectileQiDrainedEventV1 {
    pub field_caster: String,
    pub projectile: String,
    pub owner: Option<String>,
    pub drained_amount: f32,
    pub remaining_payload: f32,
    pub delta: f32,
    pub tick: u64,
}
