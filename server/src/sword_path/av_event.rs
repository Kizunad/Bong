//! plan-sword-path-v2 P4 — 剑道五招的 AV（粒子 / 音效 / 动画）触发事件。
//!
//! 五招的 `cast_*` SkillFn（见 `skill_register`）在 cast **成功结算**后 emit
//! `SwordPathSkillCastEvent`；化虚一击的释放 AV 由 `heaven_gate_cast_system`
//! 单独 emit（charge 阶段在 cast 时、release 阶段在结算 system）。
//!
//! 视听呈现走与 woliu / baomai / tuike 完全一致的双系统模式：
//! - `network::vfx_animation_trigger::emit_sword_path_visual_triggers` 读本事件 →
//!   发 `VfxEventRequest`（PlayAnim + SpawnParticle），引用客户端已注册的
//!   `SwordPathVfxPlayer` 粒子 event_id 与 `BongAnimations` 动画 id。
//! - `network::audio_trigger::emit_sword_path_audio_triggers` 读本事件 →
//!   发 `PlaySoundRecipeRequest`，引用客户端已注册的 `audio_recipes/sword_*.json`。
//!
//! **纯 cosmetic**：本事件不参与任何战斗数值 / qi_physics ledger / 命中结算。
//! 战斗逻辑仍由 `cast_*` 发出的 `AttackIntent` / `ApplyStatusEffectIntent` /
//! `HeavenGateCastEvent` 驱动；本事件只为"看得见 / 听得见"。

use valence::prelude::{bevy_ecs, DVec3, Entity, Event};

/// 剑道五招的稳定标识，用于 AV 系统 match 出各招专属的粒子 / 音效 / 动画。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SwordPathSkillId {
    /// 剑意·凝锋
    CondenseEdge,
    /// 剑气·斩
    QiSlash,
    /// 共鸣·剑鸣
    Resonance,
    /// 归一·剑意化形
    Manifest,
    /// 天门·一剑开天（蓄力阶段——cast 触发时）
    HeavenGateCharge,
    /// 天门·一剑开天（释放阶段——结算 system 触发时）
    HeavenGateRelease,
}

/// 剑道某一招 cast 成功 → 触发该招的 AV 呈现。
///
/// `center` 是粒子原点（cast 者所在位置，或招式中心）；`direction` 用于
/// 朝向类粒子（剑气斩的 line trail），无方向时为 `None`。
#[derive(Debug, Clone, Event, PartialEq)]
pub struct SwordPathSkillCastEvent {
    pub skill: SwordPathSkillId,
    pub caster: Entity,
    pub center: DVec3,
    pub direction: Option<DVec3>,
    pub tick: u64,
}
