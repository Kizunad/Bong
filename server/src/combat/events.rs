use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Entity, Event};

use crate::combat::components::{BodyPart, WoundKind};
use crate::player::gameplay::CombatAction;
use crate::schema::death_insight::DeathInsightRequestV1;

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
    #[serde(default)]
    pub source: AttackSource,
    pub debug_command: Option<CombatAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AttackSource {
    #[default]
    Melee,
    BurstMeridian,
}

#[derive(Debug, Clone, Event, Serialize, Deserialize)]
pub struct DefenseIntent {
    pub defender: Entity,
    pub issued_at_tick: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DefenseKind {
    JieMai,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StatusEffectKind {
    Bleeding,
    Slowed,
    Stunned,
    DamageAmp,
    DamageReduction,
    /// plan-cultivation-v1 §3.1：服用突破辅助丹药后附加的临时 buff。
    /// `magnitude` 作 material_bonus（0.0..=0.30），突破事务聚合后一次性消费。
    BreakthroughBoost,
    /// plan-social-v1 §6.1：切磋失败后的 5 分钟谦抑状态。
    Humility,
    /// plan-fauna-v1 §4 / §7 P4：变异核心震荡感知系统，短暂制造幻觉。
    InsightHallucination,
    /// plan-woliu-v1 §3.1.C：绝灵涡流持涡态，立即降速并阻断主动攻防。
    VortexCasting,
    /// plan-woliu-v1 §3.2.C：抗灵压丹，降低涡流反噬触发概率。
    AntiSpiritPressurePill,
    /// plan-lifespan-v1 §4：风烛状态。`magnitude` 记录真元回复削减比例。
    Frailty,
    /// plan-alchemy-v2 P0：丹药副作用提供短时回气增益。
    QiRegenBoost,
    /// plan-alchemy-v2 P0：丹药副作用触发一次顿悟机会，同时保留短时状态标记。
    InsightFlash,
    /// plan-alchemy-v2 P0：永久压低真元上限的副作用标记。
    QiCapPermMinus,
    /// plan-alchemy-v2 P0：施毒类丹药副作用，增加污染/中毒压力。
    ContaminationBoost,
    /// plan-alchemy-v2 P0：未知 side_effect tag 的兼容兜底，保留原始 tag 便于观测。
    AlchemyBuff(String),
    /// plan-zhenmai-v1 §3.1.C：截脉震爆触发后的半息僵直。
    ParryRecovery,
}

pub const HALLUCINATION_DURATION_TICKS: u64 = 20 * 5;

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
    pub defense_kind: Option<DefenseKind>,
    pub defense_effectiveness: Option<f32>,
    pub defense_contam_reduced: Option<f64>,
    pub defense_wound_severity: Option<f32>,
}

/// plan-tsy-loot-v1 §6 — 死亡事件，附带攻击者链路（Option，因为环境死亡 / 修炼自爆没有"凶手"）。
///
/// `attacker` 是 ECS Entity（适合 server 内部 query），`attacker_player_id` 是
/// canonical player id（如 `"offline:Foo"`），适合 IPC / agent 消费。两者独立维护：
/// PVP 死亡两者都填；NPC 揍死玩家只填 attacker；环境死亡（tsy_drain / bleed_out
/// without source）两者都 None。
#[derive(Debug, Clone, Event, Serialize, Deserialize)]
pub struct DeathEvent {
    pub target: Entity,
    pub cause: String,
    pub attacker: Option<Entity>,
    pub attacker_player_id: Option<String>,
    pub at_tick: u64,
}

#[derive(Debug, Clone, Event, Serialize, Deserialize)]
pub struct DeathInsightRequested {
    pub payload: DeathInsightRequestV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RevivalActionKind {
    Reincarnate,
    Terminate,
    CreateNewCharacter,
}

#[derive(Debug, Clone, Event, Serialize, Deserialize)]
pub struct RevivalActionIntent {
    pub entity: Entity,
    pub action: RevivalActionKind,
    pub issued_at_tick: u64,
}

/// plan-combat-no_ui §13 C1 — 调试命令注入通道 (`/wound add` / `/health set` / `/stamina set`)。
///
/// 由 `cmd::dev` 命令 handler 写入，`combat::debug::apply_debug_combat_commands`
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
    /// 设置玩家重生锚点（灵龛坐标）。
    ///
    /// 仅 dev/MVP：由 chat dev command 写入，用于验证「灵龛 > 出生点」与运数期条件。
    SetSpawnAnchor(Option<[f64; 3]>),
}
