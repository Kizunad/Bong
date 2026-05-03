use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CarrierKindV1 {
    YibianShougu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CarrierChargePhaseV1 {
    Idle,
    Charging,
    Charged,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CarrierStateV1 {
    pub carrier: String,
    pub phase: CarrierChargePhaseV1,
    pub progress: f32,
    pub sealed_qi: f32,
    pub sealed_qi_initial: f32,
    pub half_life_remaining_ticks: u64,
    pub item_instance_id: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CarrierChargedEventV1 {
    pub carrier: String,
    pub instance_id: u64,
    pub qi_amount: f32,
    pub qi_color: String,
    pub full_charge: bool,
    pub tick: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CarrierImpactEventV1 {
    pub attacker: String,
    pub target: String,
    pub carrier_kind: CarrierKindV1,
    pub hit_distance: f32,
    pub sealed_qi_initial: f32,
    pub hit_qi: f32,
    pub wound_damage: f32,
    pub contam_amount: f32,
    pub tick: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectileDespawnReasonV1 {
    HitTarget,
    HitBlock,
    OutOfRange,
    NaturalDecay,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectileDespawnedEventV1 {
    pub owner: Option<String>,
    pub projectile: String,
    pub reason: ProjectileDespawnReasonV1,
    pub distance: f32,
    pub qi_evaporated: f32,
    pub residual_qi: f32,
    pub pos: [f64; 3],
    pub tick: u64,
}
