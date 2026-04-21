use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Entity, Event};

use crate::combat::components::{BodyPart, WoundKind};
use crate::player::gameplay::CombatAction;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AttackReach {
    pub base: f32,
    pub step_bonus: f32,
    pub max: f32,
}

impl AttackReach {
    pub const fn new(base: f32, step_bonus: f32) -> Self {
        Self {
            base,
            step_bonus,
            max: base + step_bonus,
        }
    }
}

pub const FIST_REACH: AttackReach = AttackReach::new(0.9, 0.4);
#[allow(dead_code)]
pub const DAGGER_REACH: AttackReach = AttackReach::new(1.2, 0.4);
pub const SWORD_REACH: AttackReach = AttackReach::new(2.0, 0.5);
pub const SPEAR_REACH: AttackReach = AttackReach::new(2.6, 0.4);
#[allow(dead_code)]
pub const STAFF_REACH: AttackReach = AttackReach::new(2.4, 0.4);

#[derive(Debug, Clone, Event, Serialize, Deserialize)]
pub struct AttackIntent {
    pub attacker: Entity,
    pub target: Option<Entity>,
    pub issued_at_tick: u64,
    pub reach: AttackReach,
    pub qi_invest: f32,
    pub wound_kind: WoundKind,
    pub debug_command: Option<CombatAction>,
}

#[derive(Debug, Clone, Event, Serialize, Deserialize)]
pub struct DefenseIntent {
    pub defender: Entity,
    pub issued_at_tick: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StatusEffectKind {
    Bleeding,
    Slowed,
    Stunned,
    DamageAmp,
    DamageReduction,
    /// plan-cultivation-v1 §3.1：服用突破辅助丹药后附加的临时 buff。
    /// `magnitude` 作 material_bonus（0.0..=0.30），突破事务聚合后一次性消费。
    BreakthroughBoost,
}

#[derive(Debug, Clone, Event, Serialize, Deserialize)]
pub struct ApplyStatusEffectIntent {
    pub target: Entity,
    pub kind: StatusEffectKind,
    pub magnitude: f32,
    pub duration_ticks: u64,
    pub issued_at_tick: u64,
}

#[derive(Debug, Clone, Event, Serialize, Deserialize)]
pub struct CombatEvent {
    pub attacker: Entity,
    pub target: Entity,
    pub resolved_at_tick: u64,
    pub body_part: BodyPart,
    pub wound_kind: WoundKind,
    pub damage: f32,
    pub contam_delta: f64,
    pub description: String,
}

#[derive(Debug, Clone, Event, Serialize, Deserialize)]
pub struct DeathEvent {
    pub target: Entity,
    pub cause: String,
    pub at_tick: u64,
}

/// plan-combat-no_ui §13 C1 — 调试命令注入通道 (`!wound add` / `!health set` / `!stamina set`)。
///
/// 由 `chat_collector.rs` 在开发命令分支写入，`combat::debug::apply_debug_combat_commands`
/// 消费并直接改写目标实体的 `Wounds` / `Stamina`。
///
/// **仅调试用** — 不走 AttackIntent 管线，不触发污染/防御/状态效果。
#[derive(Debug, Clone, Event)]
pub struct DebugCombatCommand {
    pub target: Entity,
    pub kind: DebugCombatCommandKind,
}

#[derive(Debug, Clone)]
pub enum DebugCombatCommandKind {
    AddWound {
        location: BodyPart,
        kind: WoundKind,
        severity: f32,
    },
    SetHealth(f32),
    SetStamina(f32),
}
