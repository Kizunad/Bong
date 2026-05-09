use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CarrierKindV1 {
    BoneChip,
    YibianShougu,
    LingmuArrow,
    DyedBone,
    FenglingheBone,
    ShangguBone,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnqiSkillKindV1 {
    SingleSnipe,
    MultiShot,
    SoulInject,
    ArmorPierce,
    EchoFractal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnqiContainerKindV1 {
    HandSlot,
    Quiver,
    PocketPouch,
    Fenglinghe,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MultiShotEventV1 {
    pub caster: String,
    pub carrier_kind: CarrierKindV1,
    pub projectile_count: u8,
    pub cone_degrees: f64,
    pub tracking_degrees: f64,
    pub tick: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QiInjectionEventV1 {
    pub caster: String,
    pub target: Option<String>,
    pub skill: AnqiSkillKindV1,
    pub carrier_kind: CarrierKindV1,
    pub payload_qi: f64,
    pub wound_qi: f64,
    pub contamination_qi: f64,
    pub overload_ratio: f64,
    pub triggers_overload_tear: bool,
    pub tick: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EchoFractalEventV1 {
    pub caster: String,
    pub carrier_kind: CarrierKindV1,
    pub local_qi_density: f64,
    pub threshold: f64,
    pub echo_count: u32,
    pub damage_per_echo: f64,
    pub tick: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AbrasionDirectionV1 {
    Store,
    Draw,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CarrierAbrasionEventV1 {
    pub carrier: String,
    pub container: AnqiContainerKindV1,
    pub direction: AbrasionDirectionV1,
    pub lost_qi: f64,
    pub after_qi: f64,
    pub tick: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContainerSwapEventV1 {
    pub carrier: String,
    pub from: AnqiContainerKindV1,
    pub to: AnqiContainerKindV1,
    pub switching_until_tick: u64,
    pub tick: u64,
}
