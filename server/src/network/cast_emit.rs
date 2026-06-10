//! plan-HUD-v1 §4 cast 状态机 server side。
//!
//! 三件事：
//! 1. `tick_casts_or_interrupt` 系统：每 tick 检查所有 `Casting` 实体，受击中断
//!    优先于自然完成，发对应 `cast_sync` payload 并 remove component。
//! 2. `push_cast_sync_to_client` 公共函数：handler 接收 `use_quick_slot`
//!    intent 时同样调它推 `cast_sync(Casting)`。
//! 3. `cast_sync_payload` 帮助构造完整 payload。
//!
//! 当前 v1 限制：
//! - 只做受击中断（contam）；移动 / 控制效果 / 主动取消 留 TODO
//! - 完成时按绑定物品消耗库存并应用已支持的物品效果
//! - duration 来自 client intent / 默认 1500ms（无 QuickSlotBindings 时）

use std::time::{SystemTime, UNIX_EPOCH};

use valence::prelude::{
    Client, Commands, Entity, EventWriter, Mut, ParamSet, Position, Query, Res, Username,
};

use crate::alchemy::pill::apply_wound_heal;
use crate::combat::components::{
    BodyPart, CastSource, Casting, QuickSlotBindings, SkillBarBindings, StatusEffects, Wounds,
};
use crate::combat::events::{ApplyStatusEffectIntent, StatusEffectKind};
use crate::combat::yidao::YidaoCastCompleteEvent;
use crate::combat::CombatClock;
use crate::cultivation::components::{
    recover_current_qi, Contamination, Cultivation, MeridianSystem,
};
use crate::cultivation::lifespan::LifespanExtensionIntent;
use crate::cultivation::poison_trait::{ConsumePoisonPillIntent, PoisonPillKind};
use crate::inventory::food::{consume_food, ConsumeFoodResult};
use crate::inventory::{ItemEffect, ItemRegistry, PlayerInventory};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::audio_trigger::{
    emit_recipe_audio_with_context, AudioEmitContext, AudioEmitWriter,
};
use crate::network::inventory_snapshot_emit::send_inventory_snapshot_to_client;
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::player::state::PlayerState;
use crate::schema::combat_hud::{CastOutcomeV1, CastPhaseV1, CastSyncV1};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use crate::shelflife::DecayProfileRegistry;

/// Cooldown 默认值（plan §4.4）。中断后短冷却 0.5s（10 tick）；
/// 完成后冷却来自 ItemTemplate.cooldown_ms（折算到 Casting.complete_cooldown_ticks）。
pub const CAST_INTERRUPT_COOLDOWN_TICKS: u64 = 10;
/// plan §4.3 移动中断阈值（米）。超过即视为主动位移中断。
pub const CAST_MOVEMENT_INTERRUPT_THRESHOLD_M: f64 = 0.3;

type CastTickQueryItem<'a> = (
    Entity,
    &'a mut Client,
    &'a Username,
    &'a Casting,
    &'a mut Wounds,
    &'a Position,
    &'a mut PlayerInventory,
    &'a PlayerState,
    &'a mut QuickSlotBindings,
    &'a mut SkillBarBindings,
    Option<&'a StatusEffects>,
    Option<&'a mut Cultivation>,
    Option<&'a mut MeridianSystem>,
    Option<&'a mut Contamination>,
);

struct CastItemEffectTargets<'a> {
    cultivation: Option<&'a mut Cultivation>,
    meridians: Option<Mut<'a, MeridianSystem>>,
    contamination: Option<Mut<'a, Contamination>>,
    wounds: Option<&'a mut Wounds>,
}

struct CastItemEffectContext<'a> {
    issued_at_tick: u64,
    username: &'a str,
    entity: Entity,
    /// plan-food-v1 P2 — 消费 FoodRegen 时从 ItemInstance 克隆的 freshness（已 clone，无借用冲突）。
    /// `None` = 无 freshness（非食物或尚未挂 freshness）。
    item_freshness: Option<crate::shelflife::Freshness>,
    /// plan-food-v1 P2 — DecayProfileRegistry 引用，用于 freshness 门控。
    decay_profiles: Option<&'a crate::shelflife::DecayProfileRegistry>,
}

#[allow(clippy::too_many_arguments)]
pub fn tick_casts_or_interrupt(
    clock: Res<CombatClock>,
    mut commands: Commands,
    item_registry: Res<ItemRegistry>,
    decay_profiles: Option<Res<DecayProfileRegistry>>,
    mut audio_events: AudioEmitWriter,
    mut yidao_complete_events: EventWriter<YidaoCastCompleteEvent>,
    mut effect_intents: ParamSet<(
        EventWriter<ApplyStatusEffectIntent>,
        EventWriter<LifespanExtensionIntent>,
        EventWriter<ConsumePoisonPillIntent>,
    )>,
    mut clients: Query<CastTickQueryItem<'_>>,
) {
    let mut audio_events = audio_events.context();
    for (
        entity,
        mut client,
        username,
        casting,
        mut wounds,
        position,
        mut inventory,
        player_state,
        mut bindings,
        mut skillbar_bindings,
        status_effects,
        mut cultivation,
        meridians,
        contamination,
    ) in &mut clients
    {
        // plan §4.3 控制中断（Stunned）—— 优先级最高：玩家根本动不了。
        let stunned = status_effects.is_some_and(|se| {
            se.active
                .iter()
                .any(|e| e.kind == StatusEffectKind::Stunned && e.remaining_ticks > 0)
        });
        if stunned {
            commands.entity(entity).remove::<Casting>();
            set_cast_cooldown(
                casting,
                &mut bindings,
                &mut skillbar_bindings,
                casting.slot,
                clock.tick.saturating_add(CAST_INTERRUPT_COOLDOWN_TICKS),
            );
            push_cast_sync(
                &mut client,
                CastSyncV1 {
                    phase: CastPhaseV1::Interrupt,
                    slot: casting.slot,
                    duration_ms: casting.duration_ms,
                    started_at_ms: casting.started_at_ms,
                    outcome: CastOutcomeV1::InterruptControl,
                },
                username.0.as_str(),
                entity,
            );
            emit_cast_interrupt_audio(&mut audio_events, entity, position.get(), casting);
            tracing::info!(
                "[bong][network][cast] control interrupt entity={entity:?} `{}` slot={} (Stunned)",
                username.0,
                casting.slot
            );
            continue;
        }
        // 受击中断：本 tick 新增的 wound。
        let damaged_this_tick = wounds
            .entries
            .iter()
            .any(|w| w.created_at_tick == clock.tick);
        if damaged_this_tick {
            commands.entity(entity).remove::<Casting>();
            set_cast_cooldown(
                casting,
                &mut bindings,
                &mut skillbar_bindings,
                casting.slot,
                clock.tick.saturating_add(CAST_INTERRUPT_COOLDOWN_TICKS),
            );
            push_cast_sync(
                &mut client,
                CastSyncV1 {
                    phase: CastPhaseV1::Interrupt,
                    slot: casting.slot,
                    duration_ms: casting.duration_ms,
                    started_at_ms: casting.started_at_ms,
                    outcome: CastOutcomeV1::InterruptContam,
                },
                username.0.as_str(),
                entity,
            );
            emit_cast_interrupt_audio(&mut audio_events, entity, position.get(), casting);
            continue;
        }
        // 移动中断（plan §4.3）：当前位置与 cast 起始位置距离超阈值。
        let moved_distance = position.get().distance(casting.start_position);
        if moved_distance > CAST_MOVEMENT_INTERRUPT_THRESHOLD_M {
            commands.entity(entity).remove::<Casting>();
            set_cast_cooldown(
                casting,
                &mut bindings,
                &mut skillbar_bindings,
                casting.slot,
                clock.tick.saturating_add(CAST_INTERRUPT_COOLDOWN_TICKS),
            );
            push_cast_sync(
                &mut client,
                CastSyncV1 {
                    phase: CastPhaseV1::Interrupt,
                    slot: casting.slot,
                    duration_ms: casting.duration_ms,
                    started_at_ms: casting.started_at_ms,
                    outcome: CastOutcomeV1::InterruptMovement,
                },
                username.0.as_str(),
                entity,
            );
            emit_cast_interrupt_audio(&mut audio_events, entity, position.get(), casting);
            tracing::info!(
                "[bong][network][cast] movement interrupt entity={entity:?} `{}` slot={} moved={:.3}m",
                username.0,
                casting.slot,
                moved_distance
            );
            continue;
        }
        // 自然完成
        if clock.tick >= casting.started_at_tick + casting.duration_ticks {
            commands.entity(entity).remove::<Casting>();
            if let Some(skill_id) = casting
                .skill_id
                .as_deref()
                .filter(|skill_id| skill_id.starts_with("yidao."))
            {
                yidao_complete_events.send(YidaoCastCompleteEvent {
                    caster: entity,
                    slot: casting.slot,
                    skill_id: skill_id.to_string(),
                    completed_at_tick: clock.tick,
                });
            }
            // 1) 消耗：物品快捷槽找到绑定 instance_id，stack -= 1；技能栏只进入冷却。
            let mut effect_to_apply: Option<ItemEffect> = None;
            // plan-food-v1 P2：在 consume_one_stack 借走 inventory 之前，先 clone freshness
            // （clone 避免生命周期冲突）。
            let mut cast_item_freshness: Option<crate::shelflife::Freshness> = None;
            if casting.source == CastSource::QuickSlot {
                if let Some(id) = casting.bound_instance_id {
                    if let Some(template_id) = lookup_template_id(&inventory, id) {
                        if let Some(template) = item_registry.get(&template_id) {
                            effect_to_apply = template.effect.clone();
                        }
                    }
                    // 克隆 freshness（在 consume_one_stack 可变借用前做）
                    if let Some(inst) = clone_item_at_for_freshness(&inventory, id) {
                        cast_item_freshness = inst;
                    }
                }
            }

            // plan-food-v1 P2 CriticalBlock 门控：FoodRegen 食物在扣库存前先判 freshness。
            // CriticalBlock → 不扣库存，给玩家"太腐败无法食用"日志反馈，直接跳过 cast 完成。
            if let Some(ItemEffect::FoodRegen {
                bonus_factor,
                duration_ticks,
            }) = effect_to_apply.as_ref()
            {
                let freshness_pair = cast_item_freshness.as_ref().and_then(|f| {
                    decay_profiles
                        .as_deref()
                        .and_then(|reg| reg.get(&f.profile))
                        .map(|profile| (f, profile))
                });
                let pre_check = consume_food(
                    freshness_pair,
                    *bonus_factor,
                    *duration_ticks,
                    clock.tick,
                    1.0,
                );
                if matches!(pre_check, ConsumeFoodResult::CriticalBlock { .. }) {
                    tracing::warn!(
                        "[bong][network][cast] FoodRegen CriticalBlock: 食物已极度腐败，拒绝消费（库存不扣）for `{}` ({:?})",
                        username.0,
                        entity
                    );
                    // 不扣库存，不应用效果，直接结束本次 cast（进入冷却）。
                    set_cast_cooldown(
                        casting,
                        &mut bindings,
                        &mut skillbar_bindings,
                        casting.slot,
                        clock.tick.saturating_add(casting.complete_cooldown_ticks),
                    );
                    push_cast_sync(
                        &mut client,
                        CastSyncV1 {
                            phase: CastPhaseV1::Complete,
                            slot: casting.slot,
                            duration_ms: casting.duration_ms,
                            started_at_ms: casting.started_at_ms,
                            outcome: CastOutcomeV1::Completed,
                        },
                        username.0.as_str(),
                        entity,
                    );
                    continue;
                }
            }

            let consumed = if casting.source == CastSource::QuickSlot {
                casting
                    .bound_instance_id
                    .map(|id| consume_one_stack(&mut inventory, id))
                    .unwrap_or(false)
            } else {
                false
            };
            // 2) 应用效果
            if let Some(effect) = effect_to_apply.as_ref() {
                apply_cast_item_effect(
                    effect,
                    CastItemEffectTargets {
                        cultivation: cultivation.as_deref_mut(),
                        meridians,
                        contamination,
                        wounds: Some(&mut *wounds),
                    },
                    &mut effect_intents,
                    CastItemEffectContext {
                        issued_at_tick: clock.tick,
                        username: &username.0,
                        entity,
                        item_freshness: cast_item_freshness,
                        decay_profiles: decay_profiles.as_deref(),
                    },
                );
            }
            // 3) 设置完成冷却（来自 ItemTemplate.cooldown_ms 折算后的 ticks）
            set_cast_cooldown(
                casting,
                &mut bindings,
                &mut skillbar_bindings,
                casting.slot,
                clock.tick.saturating_add(casting.complete_cooldown_ticks),
            );
            // 4) 推 cast_sync(Complete)
            push_cast_sync(
                &mut client,
                CastSyncV1 {
                    phase: CastPhaseV1::Complete,
                    slot: casting.slot,
                    duration_ms: casting.duration_ms,
                    started_at_ms: casting.started_at_ms,
                    outcome: CastOutcomeV1::Completed,
                },
                username.0.as_str(),
                entity,
            );
            // 5) 同步 inventory（消耗后）
            if consumed {
                let default_cultivation = Cultivation::default();
                let cultivation = cultivation.as_deref().unwrap_or(&default_cultivation);
                send_inventory_snapshot_to_client(
                    entity,
                    &mut client,
                    username.0.as_str(),
                    &inventory,
                    player_state,
                    cultivation,
                    "cast_complete_consume",
                );
                emit_recipe_audio_with_context(
                    &mut audio_events,
                    "pill_consume",
                    entity,
                    position.get(),
                    None,
                    0.8,
                );
            }
            if casting
                .skill_id
                .as_deref()
                .is_some_and(|skill_id| skill_id.contains("xue_beng_bu"))
            {
                emit_recipe_audio_with_context(
                    &mut audio_events,
                    "phase_shift_in",
                    entity,
                    position.get(),
                    None,
                    0.8,
                );
            }
        }
    }
}

fn emit_cast_interrupt_audio(
    audio_events: &mut AudioEmitContext<'_, '_>,
    entity: Entity,
    origin: valence::prelude::DVec3,
    casting: &Casting,
) {
    if casting.source == CastSource::SkillBar {
        emit_recipe_audio_with_context(audio_events, "cast_interrupt", entity, origin, None, 1.0);
    }
}

fn set_cast_cooldown(
    casting: &Casting,
    quick_bindings: &mut QuickSlotBindings,
    skillbar_bindings: &mut SkillBarBindings,
    slot: u8,
    until_tick: u64,
) {
    match casting.source {
        CastSource::QuickSlot => quick_bindings.set_cooldown(slot, until_tick),
        CastSource::SkillBar => skillbar_bindings.set_cooldown(slot, until_tick),
    }
}

fn lookup_template_id(inv: &PlayerInventory, instance_id: u64) -> Option<String> {
    for c in &inv.containers {
        if let Some(p) = c
            .items
            .iter()
            .find(|p| p.instance.instance_id == instance_id)
        {
            return Some(p.instance.template_id.clone());
        }
    }
    if let Some(item) = inv
        .equipped
        .values()
        .find(|item| item.instance_id == instance_id)
    {
        return Some(item.template_id.clone());
    }
    inv.hotbar
        .iter()
        .flatten()
        .find(|item| item.instance_id == instance_id)
        .map(|item| item.template_id.clone())
}

pub(crate) fn apply_item_effect(
    effect: &ItemEffect,
    cultivation: Option<&mut Cultivation>,
    meridians: Option<valence::prelude::Mut<MeridianSystem>>,
    contamination: Option<valence::prelude::Mut<Contamination>>,
    wounds: Option<&mut Wounds>,
    username: &str,
    entity: Entity,
) {
    match effect {
        ItemEffect::MeridianHeal {
            magnitude,
            target: _,
        } => {
            // v1: 跨所有经脉，advance 第一条尚未愈合的裂痕。
            // 不区分 target = "any_meridian" vs 具体经脉 id（后续接入 MeridianId
            // 解析时再细化）。
            let Some(mut meridians) = meridians else {
                tracing::debug!(
                    "[bong][network][cast] MeridianHeal noop: entity {entity:?} `{username}` has no MeridianSystem"
                );
                return;
            };
            let mut healed_count = 0usize;
            for m in meridians.iter_mut() {
                let mut local_healed = 0usize;
                for crack in m.cracks.iter_mut() {
                    if crack.healing_progress < crack.severity {
                        crack.healing_progress =
                            (crack.healing_progress + magnitude).clamp(0.0, crack.severity);
                        if crack.healing_progress >= crack.severity {
                            local_healed += 1;
                        }
                    }
                }
                m.cracks.retain(|c| c.healing_progress < c.severity);
                if local_healed > 0 {
                    m.integrity = (m.integrity + 0.05 * local_healed as f64).min(1.0);
                    healed_count += local_healed;
                }
            }
            tracing::info!(
                "[bong][network][cast] MeridianHeal magnitude={magnitude} for `{username}` ({entity:?}) — {healed_count} crack(s) sealed"
            );
        }
        ItemEffect::ContaminationCleanse { magnitude } => {
            let Some(mut contamination) = contamination else {
                tracing::debug!(
                    "[bong][network][cast] ContaminationCleanse noop: entity {entity:?} `{username}` has no Contamination"
                );
                return;
            };
            let mut remaining = *magnitude;
            for entry in contamination.entries.iter_mut() {
                if remaining <= 0.0 {
                    break;
                }
                let take = entry.amount.min(remaining);
                entry.amount -= take;
                remaining -= take;
            }
            contamination.entries.retain(|e| e.amount > f64::EPSILON);
            tracing::info!(
                "[bong][network][cast] ContaminationCleanse magnitude={magnitude} for `{username}` ({entity:?}) — {:.3} cleansed",
                magnitude - remaining
            );
        }
        ItemEffect::QiRecovery { amount } => {
            let Some(cultivation) = cultivation else {
                tracing::debug!(
                    "[bong][network][cast] QiRecovery noop: entity {entity:?} `{username}` has no Cultivation"
                );
                return;
            };
            let qi_max_before = cultivation.qi_max;
            let recovered = recover_current_qi(cultivation, *amount);
            tracing::info!(
                "[bong][network][cast] QiRecovery amount={amount} for `{username}` ({entity:?}) — recovered {recovered:.1}, qi_max stays {qi_max_before:.1}"
            );
        }
        ItemEffect::ComposureRestore { magnitude } => {
            let Some(cultivation) = cultivation else {
                tracing::debug!(
                    "[bong][network][cast] ComposureRestore noop: entity {entity:?} `{username}` has no Cultivation"
                );
                return;
            };
            let before = cultivation.composure;
            cultivation.composure = (cultivation.composure + magnitude).clamp(0.0, 1.0);
            tracing::info!(
                "[bong][network][cast] ComposureRestore magnitude={magnitude} for `{username}` ({entity:?}) — {before:.3} → {:.3}",
                cultivation.composure
            );
        }
        ItemEffect::WoundHeal { magnitude, target } => {
            let Some(wounds) = wounds else {
                tracing::debug!(
                    "[bong][network][cast] WoundHeal noop: entity {entity:?} `{username}` has no Wounds"
                );
                return;
            };
            let grades = wound_heal_grades(*magnitude);
            let changed = apply_wound_heal_targets(wounds, target.as_deref(), grades);
            tracing::info!(
                "[bong][network][cast] WoundHeal magnitude={magnitude} target={:?} grades={grades} for `{username}` ({entity:?}) — {changed} wound(s) changed",
                target
            );
        }
        ItemEffect::BreakthroughBonus { magnitude } => {
            // v1 不存 buff state（缺 Component）。仅 log。
            tracing::info!(
                "[bong][network][cast] BreakthroughBonus magnitude={magnitude} for `{username}` ({entity:?}) — no-op (buff state TODO)"
            );
        }
        ItemEffect::LifespanExtension { years, source } => {
            tracing::info!(
                "[bong][network][cast] LifespanExtension years={years} source={source} for `{username}` ({entity:?}) — handled by take_pill path"
            );
        }
        ItemEffect::AntiSpiritPressure { duration_ticks } => {
            tracing::info!(
                "[bong][network][cast] AntiSpiritPressure duration_ticks={duration_ticks} for `{username}` ({entity:?}) — handled by take_pill path"
            );
        }
        ItemEffect::PoisonPill { pill_item_id } => {
            tracing::info!(
                "[bong][network][cast] PoisonPill `{pill_item_id}` for `{username}` ({entity:?}) — handled by take_pill path"
            );
        }
        ItemEffect::CombatPill { pill_item_id } => {
            tracing::info!(
                "[bong][network][cast] CombatPill `{pill_item_id}` for `{username}` ({entity:?}) — handled by take_pill path"
            );
        }
        ItemEffect::FoodRegen {
            bonus_factor,
            duration_ticks,
        } => {
            // plan-food-v1 P2: FoodRegen 在 apply_item_effect 路径无 StatusEffects 可写，
            // 由 apply_cast_item_effect 路径通过 ApplyStatusEffectIntent 处理。此处仅 log。
            tracing::debug!(
                "[bong][network][cast] FoodRegen bonus={bonus_factor} duration={duration_ticks} for `{username}` ({entity:?}) — handled by cast_item_effect path"
            );
        }
        ItemEffect::BeastCoreAbsorption {
            breakthrough_magnitude,
            hallucination_duration_ticks,
        } => {
            // plan-fauna-stitched-beast-v1 P3: 兽核吸收在 take_pill 路径处理（emit S2C + narration）。
            // apply_item_effect 路径仅 log，不重复处理。
            tracing::info!(
                "[bong][network][cast] BeastCoreAbsorption magnitude={breakthrough_magnitude} hallucination={hallucination_duration_ticks}t \
                 for `{username}` ({entity:?}) — handled by take_pill path"
            );
        }
    }
}

fn apply_cast_item_effect(
    effect: &ItemEffect,
    targets: CastItemEffectTargets<'_>,
    effect_intents: &mut ParamSet<(
        EventWriter<ApplyStatusEffectIntent>,
        EventWriter<LifespanExtensionIntent>,
        EventWriter<ConsumePoisonPillIntent>,
    )>,
    context: CastItemEffectContext<'_>,
) {
    match effect {
        ItemEffect::LifespanExtension { years, source } => {
            effect_intents.p1().send(LifespanExtensionIntent {
                entity: context.entity,
                requested_years: (*years).max(1),
                source: source.clone(),
            });
            tracing::info!(
                "[bong][network][cast] LifespanExtension years={years} source={source} for `{}` ({:?})",
                context.username,
                context.entity
            );
        }
        ItemEffect::AntiSpiritPressure { duration_ticks } => {
            effect_intents.p0().send(ApplyStatusEffectIntent {
                target: context.entity,
                kind: StatusEffectKind::AntiSpiritPressurePill,
                magnitude: 1.0,
                duration_ticks: (*duration_ticks).max(1),
                issued_at_tick: context.issued_at_tick,
            });
            tracing::info!(
                "[bong][network][cast] AntiSpiritPressure duration_ticks={duration_ticks} for `{}` ({:?})",
                context.username,
                context.entity
            );
        }
        ItemEffect::PoisonPill { pill_item_id } => {
            let Some(pill) = PoisonPillKind::from_item_id(pill_item_id) else {
                tracing::warn!(
                    "[bong][network][cast] PoisonPill `{pill_item_id}` for `{}` ({:?}) has no poison pill kind",
                    context.username,
                    context.entity
                );
                return;
            };
            effect_intents.p2().send(ConsumePoisonPillIntent {
                entity: context.entity,
                pill,
                issued_at_tick: context.issued_at_tick,
            });
            tracing::info!(
                "[bong][network][cast] PoisonPill `{pill_item_id}` for `{}` ({:?}) → PoisonToxicity intent",
                context.username,
                context.entity
            );
        }
        ItemEffect::CombatPill { .. } => {
            tracing::debug!(
                "[bong][network][cast] CombatPill for `{}` ({:?}) ignored on generic cast path",
                context.username,
                context.entity
            );
        }
        ItemEffect::FoodRegen {
            bonus_factor,
            duration_ticks,
        } => {
            // plan-food-v1 P2：灵食修炼加速 + freshness 门控。
            // 1) 用 consume_food 纯函数判定 freshness 状态。
            // 2) CriticalBlock → 拒绝消费，不写 status effect。
            // 3) SpoiledWarn → 降效消费（按折算 magnitude）。
            // 4) FoodApplied / Noop → 正常写入 CultivationAcceleration。
            let freshness_pair = context.item_freshness.as_ref().and_then(|f| {
                context
                    .decay_profiles
                    .and_then(|reg| reg.get(&f.profile))
                    .map(|profile| (f, profile))
            });
            let food_result = consume_food(
                freshness_pair,
                *bonus_factor,
                *duration_ticks,
                context.issued_at_tick,
                1.0, // storage_multiplier：无容器上下文，传 1.0（冰窖走 P3 容器层）
            );
            match &food_result {
                ConsumeFoodResult::CriticalBlock {
                    current_qi,
                    spoil_threshold,
                } => {
                    // plan-food-v1 P2：CriticalBlock 路径已在 tick_casts_or_interrupt 的
                    // 前置门控拦截（不扣库存，直接 continue）。
                    // apply_cast_item_effect 不应再收到 CriticalBlock；若到此分支是防御性保底。
                    tracing::warn!(
                        "[bong][network][cast] FoodRegen CriticalBlock（漏网）: current_qi={current_qi:.3} < 0.1×spoil_threshold={spoil_threshold:.3} for `{}` ({:?}) — 不写 CultivationAcceleration",
                        context.username,
                        context.entity
                    );
                }
                ConsumeFoodResult::SpoiledWarn {
                    reduced_bonus_factor,
                    duration_ticks: eff_duration,
                    current_qi,
                    spoil_threshold,
                } => {
                    if *reduced_bonus_factor > 0.0 {
                        effect_intents.p0().send(ApplyStatusEffectIntent {
                            target: context.entity,
                            kind: StatusEffectKind::CultivationAcceleration,
                            magnitude: *reduced_bonus_factor,
                            duration_ticks: (*eff_duration).max(1),
                            issued_at_tick: context.issued_at_tick,
                        });
                        tracing::warn!(
                            "[bong][network][cast] FoodRegen SpoiledWarn: current_qi={current_qi:.3} spoil_threshold={spoil_threshold:.3} → reduced bonus={reduced_bonus_factor:.3} for `{}` ({:?})",
                            context.username,
                            context.entity
                        );
                    } else {
                        tracing::warn!(
                            "[bong][network][cast] FoodRegen SpoiledWarn reduced_bonus=0 for `{}` ({:?}) — 跳过写入",
                            context.username,
                            context.entity
                        );
                    }
                }
                ConsumeFoodResult::FoodApplied {
                    bonus_factor: eff_bonus,
                    duration_ticks: eff_duration,
                    is_peak,
                } => {
                    effect_intents.p0().send(ApplyStatusEffectIntent {
                        target: context.entity,
                        kind: StatusEffectKind::CultivationAcceleration,
                        magnitude: *eff_bonus,
                        duration_ticks: (*eff_duration).max(1),
                        issued_at_tick: context.issued_at_tick,
                    });
                    tracing::info!(
                        "[bong][network][cast] FoodRegen FoodApplied: bonus={eff_bonus:.3} duration={eff_duration} is_peak={is_peak} for `{}` ({:?}) → CultivationAcceleration intent",
                        context.username,
                        context.entity
                    );
                }
                ConsumeFoodResult::Noop => {
                    // 无 freshness / profile 不匹配 — 退化为直接应用原始 bonus（兼容旧物品）
                    effect_intents.p0().send(ApplyStatusEffectIntent {
                        target: context.entity,
                        kind: StatusEffectKind::CultivationAcceleration,
                        magnitude: *bonus_factor,
                        duration_ticks: (*duration_ticks).max(1),
                        issued_at_tick: context.issued_at_tick,
                    });
                    tracing::info!(
                        "[bong][network][cast] FoodRegen Noop (no freshness) bonus={bonus_factor} duration={duration_ticks} for `{}` ({:?}) → CultivationAcceleration intent",
                        context.username,
                        context.entity
                    );
                }
            }
        }
        _ => apply_item_effect(
            effect,
            targets.cultivation,
            targets.meridians,
            targets.contamination,
            targets.wounds,
            context.username,
            context.entity,
        ),
    }
}

fn wound_heal_grades(magnitude: f64) -> u8 {
    magnitude.round().clamp(0.0, f64::from(u8::MAX)) as u8
}

fn apply_wound_heal_targets(wounds: &mut Wounds, target: Option<&str>, grades: u8) -> usize {
    let Some(target) = target.map(str::trim).filter(|target| !target.is_empty()) else {
        return apply_wound_heal(wounds, None, grades);
    };
    let mut remaining_grades = grades;
    let mut changed_total = 0usize;
    for part in target.split('/').filter_map(parse_wound_heal_body_part) {
        if remaining_grades == 0 {
            break;
        }
        let changed = apply_wound_heal(wounds, Some(part), remaining_grades);
        changed_total += changed;
        remaining_grades = remaining_grades.saturating_sub(changed as u8);
    }
    changed_total
}

fn parse_wound_heal_body_part(raw: &str) -> Option<BodyPart> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "head" => Some(BodyPart::Head),
        "chest" => Some(BodyPart::Chest),
        "back" => Some(BodyPart::Back),
        "abdomen" => Some(BodyPart::Abdomen),
        "arm_l" => Some(BodyPart::ArmL),
        "arm_r" => Some(BodyPart::ArmR),
        "leg_l" => Some(BodyPart::LegL),
        "leg_r" => Some(BodyPart::LegR),
        _ => None,
    }
}

/// plan-food-v1 P2 — 从 inventory 克隆指定 item 的 freshness（仅 clone，不 borrow 可变）。
/// 用于在 consume_one_stack 可变借用前先取到 freshness，避免借用冲突。
fn clone_item_at_for_freshness(
    inventory: &PlayerInventory,
    instance_id: u64,
) -> Option<Option<crate::shelflife::Freshness>> {
    for c in &inventory.containers {
        if let Some(p) = c
            .items
            .iter()
            .find(|p| p.instance.instance_id == instance_id)
        {
            return Some(p.instance.freshness.clone());
        }
    }
    for item in inventory.hotbar.iter().flatten() {
        if item.instance_id == instance_id {
            return Some(item.freshness.clone());
        }
    }
    None
}

/// 在 inventory 内找 instance_id 并 stack-=1；归零则移除。返回是否成功扣到。
fn consume_one_stack(inventory: &mut PlayerInventory, instance_id: u64) -> bool {
    inventory.revision =
        crate::inventory::InventoryRevision(inventory.revision.0.saturating_add(1));
    for c in &mut inventory.containers {
        if let Some(idx) = c
            .items
            .iter()
            .position(|p| p.instance.instance_id == instance_id)
        {
            let placed = &mut c.items[idx];
            if placed.instance.stack_count > 1 {
                placed.instance.stack_count -= 1;
            } else {
                c.items.remove(idx);
            }
            return true;
        }
    }
    for slot in inventory.hotbar.iter_mut() {
        if let Some(item) = slot.as_mut() {
            if item.instance_id == instance_id {
                if item.stack_count > 1 {
                    item.stack_count -= 1;
                } else {
                    *slot = None;
                }
                return true;
            }
        }
    }
    // 装备槽内的物品不应在这条路径出现（cast 用的是消耗品而非武器/护甲）。
    false
}

pub fn push_cast_sync(client: &mut Client, state: CastSyncV1, username: &str, entity: Entity) {
    let payload = ServerDataV1::new(ServerDataPayloadV1::CastSync(state));
    let payload_type = payload_type_label(payload.payload_type());
    let payload_bytes = match serialize_server_data_payload(&payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };
    send_server_data_payload(client, payload_bytes.as_slice());
    tracing::info!(
        "[bong][network] sent {} {} payload to entity {entity:?} for `{username}` (phase={:?} slot={} outcome={:?})",
        SERVER_DATA_CHANNEL,
        payload_type,
        state.phase,
        state.slot,
        state.outcome,
    );
}

pub fn current_unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::components::{Wound, WoundKind};
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemCategory, ItemInstance, ItemRarity, ItemRegistry,
        ItemTemplate, PlacedItemState, MAIN_PACK_CONTAINER_ID,
    };
    use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest};
    use std::collections::HashMap;
    use valence::prelude::{App, DVec3, Events, Position, Query, Update, With};

    fn make_inventory_with_stack(instance_id: u64, stack: u32) -> PlayerInventory {
        let item = ItemInstance {
            instance_id,
            template_id: "tea".to_string(),
            display_name: "茶".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.1,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: stack,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        };
        PlayerInventory {
            revision: InventoryRevision(5),
            containers: vec![ContainerState {
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 5,
                cols: 7,
                items: vec![PlacedItemState {
                    row: 0,
                    col: 0,
                    instance: item,
                }],
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
        }
    }

    fn make_effect_template(template_id: &str, effect: ItemEffect) -> ItemTemplate {
        ItemTemplate {
            id: template_id.to_string(),
            display_name: template_id.to_string(),
            category: ItemCategory::Misc,
            placeable: None,
            max_stack_count: 16,
            grid_w: 1,
            grid_h: 1,
            base_weight: 0.1,
            rarity: ItemRarity::Common,
            spirit_quality_initial: 0.0,
            description: String::new(),
            effect: Some(effect),
            cast_duration_ms: 1000,
            cooldown_ms: 500,
            weapon_spec: None,
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
            technique_scroll_spec: None,
            recipe_fragment_spec: None,
            container_spec: None,
            shelflife_profile: None,
            shelflife_track: None,
        }
    }

    fn make_hotbar_item(template_id: &str, instance_id: u64, stack_count: u32) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: template_id.to_string(),
            display_name: template_id.to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.1,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count,
            spirit_quality: 0.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }
    }

    fn setup_quickslot_effect_app(template_id: &str, effect: ItemEffect) -> (App, Entity) {
        use crate::combat::CombatClock;
        use crate::player::state::PlayerState;
        use valence::prelude::DVec3;
        use valence::testing::create_mock_client;

        const INSTANCE_ID: u64 = 4242;
        const QUICK_SLOT: u8 = 0;

        let mut templates = HashMap::new();
        templates.insert(
            template_id.to_string(),
            make_effect_template(template_id, effect),
        );

        let mut inventory = PlayerInventory {
            revision: InventoryRevision(1),
            containers: vec![ContainerState {
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 5,
                cols: 7,
                items: Vec::new(),
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
        };
        inventory.hotbar[QUICK_SLOT as usize] = Some(make_hotbar_item(template_id, INSTANCE_ID, 2));

        let mut quick_slot_bindings = QuickSlotBindings::default();
        quick_slot_bindings.slots[QUICK_SLOT as usize] = Some(INSTANCE_ID);
        let casting = Casting {
            source: CastSource::QuickSlot,
            slot: QUICK_SLOT,
            started_at_tick: 0,
            duration_ticks: 1,
            started_at_ms: 0,
            duration_ms: 50,
            bound_instance_id: Some(INSTANCE_ID),
            start_position: DVec3::new(0.0, 64.0, 0.0),
            complete_cooldown_ticks: 20,
            skill_id: None,
            skill_config: None,
        };

        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 10 });
        app.insert_resource(ItemRegistry::from_map(templates));
        app.add_event::<crate::network::audio_event_emit::PlaySoundRecipeRequest>();
        app.add_event::<crate::combat::yidao::YidaoCastCompleteEvent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<crate::cultivation::lifespan::LifespanExtensionIntent>();
        app.add_event::<crate::cultivation::poison_trait::ConsumePoisonPillIntent>();
        app.add_systems(Update, tick_casts_or_interrupt);

        let (client_bundle, _helper) = create_mock_client("TestPlayer");
        let player = app
            .world_mut()
            .spawn(client_bundle)
            .insert((
                Position::new([0.0, 64.0, 0.0]),
                casting,
                Wounds::default(),
                inventory,
                PlayerState::default(),
                quick_slot_bindings,
                SkillBarBindings::default(),
            ))
            .id();

        (app, player)
    }

    fn hotbar_stack_count(app: &mut App, player: Entity) -> u32 {
        app.world_mut()
            .entity(player)
            .get::<PlayerInventory>()
            .and_then(|inventory| inventory.hotbar[0].as_ref())
            .map(|item| item.stack_count)
            .unwrap_or(0)
    }

    #[test]
    fn consume_one_stack_decrements_when_above_one() {
        let mut inv = make_inventory_with_stack(42, 5);
        assert!(consume_one_stack(&mut inv, 42));
        assert_eq!(inv.containers[0].items[0].instance.stack_count, 4);
        assert_eq!(inv.revision, InventoryRevision(6));
    }

    #[test]
    fn consume_one_stack_removes_when_at_one() {
        let mut inv = make_inventory_with_stack(42, 1);
        assert!(consume_one_stack(&mut inv, 42));
        assert!(inv.containers[0].items.is_empty());
    }

    #[test]
    fn consume_one_stack_returns_false_when_missing() {
        let mut inv = make_inventory_with_stack(42, 3);
        assert!(!consume_one_stack(&mut inv, 999));
        assert_eq!(inv.containers[0].items[0].instance.stack_count, 3);
    }

    #[test]
    fn movement_interrupt_threshold_classifies_within_and_beyond() {
        use valence::prelude::DVec3;
        let start = DVec3::new(10.0, 64.0, 20.0);
        let still_ok = DVec3::new(10.2, 64.0, 20.0); // 0.2m → 不中断
        let too_far = DVec3::new(10.5, 64.0, 20.0); // 0.5m → 中断
        assert!(still_ok.distance(start) <= CAST_MOVEMENT_INTERRUPT_THRESHOLD_M);
        assert!(too_far.distance(start) > CAST_MOVEMENT_INTERRUPT_THRESHOLD_M);
    }

    #[test]
    fn cooldown_set_get_round_trip() {
        let mut bindings = QuickSlotBindings::default();
        assert!(!bindings.is_on_cooldown(3, 100));
        bindings.set_cooldown(3, 130);
        assert!(bindings.is_on_cooldown(3, 100));
        assert!(bindings.is_on_cooldown(3, 129));
        assert!(!bindings.is_on_cooldown(3, 130));
        assert!(!bindings.is_on_cooldown(3, 131));
        // out-of-range slot is silently no-op
        assert!(!bindings.is_on_cooldown(99, 0));
        bindings.set_cooldown(99, 100);
        assert!(!bindings.is_on_cooldown(99, 50));
    }

    #[test]
    fn meridian_heal_advances_first_unhealed_crack() {
        use crate::cultivation::components::{CrackCause, MeridianCrack, MeridianSystem};
        let mut meridians = MeridianSystem::default();
        // Inject a crack into the first regular meridian.
        meridians.regular[0].cracks.push(MeridianCrack {
            severity: 0.5,
            healing_progress: 0.0,
            cause: CrackCause::Attack,
            created_at: 0,
        });
        // Manually walk apply_item_effect minus the Mut wrapper using internals.
        let crack = &mut meridians.regular[0].cracks[0];
        crack.healing_progress = (crack.healing_progress + 0.3).clamp(0.0, crack.severity);
        assert!((crack.healing_progress - 0.3).abs() < 1e-9);
        // Healing past severity should retain cull at 0.5.
        crack.healing_progress = (crack.healing_progress + 0.4).clamp(0.0, crack.severity);
        assert!((crack.healing_progress - crack.severity).abs() < 1e-9);
    }

    #[test]
    fn qi_recovery_effect_restores_current_qi_without_raising_cap() {
        let mut cultivation = Cultivation {
            qi_current: 130.0,
            qi_max: 210.0,
            qi_max_frozen: Some(20.0),
            ..Default::default()
        };

        apply_item_effect(
            &ItemEffect::QiRecovery { amount: 120.0 },
            Some(&mut cultivation),
            None,
            None,
            None,
            "Azure",
            Entity::PLACEHOLDER,
        );

        assert_eq!(cultivation.qi_current, 190.0);
        assert_eq!(cultivation.qi_max, 210.0);
        assert_eq!(cultivation.qi_max_frozen, Some(20.0));
    }

    #[test]
    fn composure_restore_clamps_to_full_composure() {
        let mut cultivation = Cultivation {
            composure: 0.80,
            ..Default::default()
        };

        apply_item_effect(
            &ItemEffect::ComposureRestore { magnitude: 0.35 },
            Some(&mut cultivation),
            None,
            None,
            None,
            "Azure",
            Entity::PLACEHOLDER,
        );

        assert_eq!(
            cultivation.composure, 1.0,
            "expected composure to clamp to 1.0 because restored by ComposureRestore magnitude 0.35, actual {}",
            cultivation.composure
        );
    }

    #[test]
    fn wound_heal_targets_all_wounds_when_target_missing() {
        let mut wounds = Wounds::default();
        wounds.entries.push(Wound {
            location: BodyPart::ArmL,
            kind: WoundKind::Cut,
            severity: 0.20,
            bleeding_per_sec: 1.0,
            created_at_tick: 0,
            inflicted_by: None,
        });
        wounds.entries.push(Wound {
            location: BodyPart::LegR,
            kind: WoundKind::Blunt,
            severity: 0.75,
            bleeding_per_sec: 1.0,
            created_at_tick: 0,
            inflicted_by: None,
        });

        apply_item_effect(
            &ItemEffect::WoundHeal {
                magnitude: 1.0,
                target: None,
            },
            None,
            None,
            None,
            Some(&mut wounds),
            "Azure",
            Entity::PLACEHOLDER,
        );

        assert_eq!(
            wounds.entries.len(),
            1,
            "expected one wound to remain because all-target WoundHeal removes only the light cut, actual {}",
            wounds.entries.len()
        );
        assert_eq!(
            wounds.entries[0].location,
            BodyPart::LegR,
            "expected remaining wound to be LegR because ArmL cut was healed below removal threshold, actual {:?}",
            wounds.entries[0].location
        );
        assert!(
            (wounds.entries[0].severity - 0.50).abs() < f32::EPSILON,
            "expected LegR severity 0.50 because WoundHeal magnitude 1 applies one grade, actual {}",
            wounds.entries[0].severity
        );
    }

    #[test]
    fn wound_heal_slash_target_filters_body_part_group() {
        let mut wounds = Wounds::default();
        wounds.entries.push(Wound {
            location: BodyPart::ArmL,
            kind: WoundKind::Blunt,
            severity: 0.40,
            bleeding_per_sec: 1.0,
            created_at_tick: 0,
            inflicted_by: None,
        });
        wounds.entries.push(Wound {
            location: BodyPart::LegL,
            kind: WoundKind::Blunt,
            severity: 0.70,
            bleeding_per_sec: 1.0,
            created_at_tick: 0,
            inflicted_by: None,
        });

        apply_item_effect(
            &ItemEffect::WoundHeal {
                magnitude: 2.0,
                target: Some("arm_l/arm_r".to_string()),
            },
            None,
            None,
            None,
            Some(&mut wounds),
            "Azure",
            Entity::PLACEHOLDER,
        );

        assert_eq!(
            wounds.entries.len(),
            1,
            "expected one wound to remain because arm target group should not heal leg wounds, actual {}",
            wounds.entries.len()
        );
        assert_eq!(
            wounds.entries[0].location,
            BodyPart::LegL,
            "expected remaining wound to be LegL because target was arm_l/arm_r, actual {:?}",
            wounds.entries[0].location
        );
        assert!(
            (wounds.entries[0].severity - 0.70).abs() < f32::EPSILON,
            "expected LegL severity unchanged at 0.70 because target was arm_l/arm_r, actual {}",
            wounds.entries[0].severity
        );
    }

    #[test]
    fn wound_heal_slash_target_shares_grade_budget_across_group() {
        let mut wounds = Wounds::default();
        wounds.entries.push(Wound {
            location: BodyPart::ArmL,
            kind: WoundKind::Blunt,
            severity: 0.75,
            bleeding_per_sec: 1.0,
            created_at_tick: 0,
            inflicted_by: None,
        });
        wounds.entries.push(Wound {
            location: BodyPart::ArmR,
            kind: WoundKind::Blunt,
            severity: 0.75,
            bleeding_per_sec: 1.0,
            created_at_tick: 0,
            inflicted_by: None,
        });

        apply_item_effect(
            &ItemEffect::WoundHeal {
                magnitude: 2.0,
                target: Some("arm_l/arm_r".to_string()),
            },
            None,
            None,
            None,
            Some(&mut wounds),
            "Azure",
            Entity::PLACEHOLDER,
        );

        let arm_l = wounds
            .entries
            .iter()
            .find(|wound| wound.location == BodyPart::ArmL)
            .expect("ArmL wound should remain after shared-budget heal");
        let arm_r = wounds
            .entries
            .iter()
            .find(|wound| wound.location == BodyPart::ArmR)
            .expect("ArmR wound should remain after shared-budget heal");
        assert!(
            (arm_l.severity - 0.25).abs() < f32::EPSILON,
            "expected ArmL severity 0.25 because it consumes the initial two-grade budget, actual {}",
            arm_l.severity
        );
        assert!(
            (arm_r.severity - 0.50).abs() < f32::EPSILON,
            "expected ArmR severity 0.50 because remaining shared budget is one grade, actual {}",
            arm_r.severity
        );
    }

    #[test]
    fn tick_casts_consumable_qi_recovery_consumes_and_applies() {
        let (mut app, player) = setup_quickslot_effect_app(
            "huiyuan_decoction_test",
            ItemEffect::QiRecovery { amount: 40.0 },
        );
        app.world_mut().entity_mut(player).insert(Cultivation {
            qi_current: 10.0,
            qi_max: 100.0,
            ..Default::default()
        });

        app.update();

        let stack_count = hotbar_stack_count(&mut app, player);
        assert_eq!(
            stack_count, 1,
            "expected hotbar stack count 1 because QuickSlot consumable should consume one item, actual {stack_count}"
        );
        let cultivation = app
            .world_mut()
            .entity(player)
            .get::<Cultivation>()
            .expect("Cultivation should remain attached");
        assert_eq!(
            cultivation.qi_current, 50.0,
            "expected qi_current 50.0 because QiRecovery amount 40 applies after consuming one item, actual {}",
            cultivation.qi_current
        );
    }

    #[test]
    fn tick_casts_consumable_composure_restore_consumes_and_applies() {
        let (mut app, player) = setup_quickslot_effect_app(
            "calming_tea_test",
            ItemEffect::ComposureRestore { magnitude: 0.35 },
        );
        app.world_mut().entity_mut(player).insert(Cultivation {
            composure: 0.40,
            ..Default::default()
        });

        app.update();

        let stack_count = hotbar_stack_count(&mut app, player);
        assert_eq!(
            stack_count, 1,
            "expected hotbar stack count 1 because QuickSlot consumable should consume one item, actual {stack_count}"
        );
        let cultivation = app
            .world_mut()
            .entity(player)
            .get::<Cultivation>()
            .expect("Cultivation should remain attached");
        assert!(
            (cultivation.composure - 0.75).abs() < f64::EPSILON,
            "expected composure 0.75 because ComposureRestore magnitude 0.35 applies to 0.40, actual {}",
            cultivation.composure
        );
    }

    #[test]
    fn tick_casts_consumable_wound_heal_consumes_and_applies() {
        let (mut app, player) = setup_quickslot_effect_app(
            "leg_splint_test",
            ItemEffect::WoundHeal {
                magnitude: 2.0,
                target: Some("leg_l/leg_r".to_string()),
            },
        );
        {
            let mut entity = app.world_mut().entity_mut(player);
            let mut wounds = entity
                .get_mut::<Wounds>()
                .expect("Wounds should be present on player");
            wounds.entries.push(Wound {
                location: BodyPart::LegL,
                kind: WoundKind::Blunt,
                severity: 0.40,
                bleeding_per_sec: 1.0,
                created_at_tick: 0,
                inflicted_by: None,
            });
            wounds.entries.push(Wound {
                location: BodyPart::ArmL,
                kind: WoundKind::Blunt,
                severity: 0.40,
                bleeding_per_sec: 1.0,
                created_at_tick: 0,
                inflicted_by: None,
            });
        }

        app.update();

        let stack_count = hotbar_stack_count(&mut app, player);
        assert_eq!(
            stack_count, 1,
            "expected hotbar stack count 1 because QuickSlot consumable should consume one item, actual {stack_count}"
        );
        let wounds = app
            .world_mut()
            .entity(player)
            .get::<Wounds>()
            .expect("Wounds should remain attached");
        assert_eq!(
            wounds.entries.len(),
            1,
            "expected one wound to remain because leg_splint only heals leg_l/leg_r, actual {}",
            wounds.entries.len()
        );
        assert_eq!(
            wounds.entries[0].location,
            BodyPart::ArmL,
            "expected ArmL wound to remain because leg_splint targets only legs, actual {:?}",
            wounds.entries[0].location
        );
        assert!(
            (wounds.entries[0].severity - 0.40).abs() < f32::EPSILON,
            "expected ArmL severity unchanged at 0.40 because leg_splint targets only legs, actual {}",
            wounds.entries[0].severity
        );
    }

    #[test]
    fn cast_poison_pill_effect_emits_consume_intent() {
        fn emit_for_test(
            mut effect_intents: ParamSet<(
                EventWriter<ApplyStatusEffectIntent>,
                EventWriter<LifespanExtensionIntent>,
                EventWriter<ConsumePoisonPillIntent>,
            )>,
        ) {
            apply_cast_item_effect(
                &ItemEffect::PoisonPill {
                    pill_item_id: "poison_pill_qing_lin_man_tuo".to_string(),
                },
                CastItemEffectTargets {
                    cultivation: None,
                    meridians: None,
                    contamination: None,
                    wounds: None,
                },
                &mut effect_intents,
                CastItemEffectContext {
                    issued_at_tick: 42,
                    username: "Azure",
                    entity: Entity::PLACEHOLDER,
                    item_freshness: None,
                    decay_profiles: None,
                },
            );
        }

        let mut app = App::new();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<LifespanExtensionIntent>();
        app.add_event::<ConsumePoisonPillIntent>();
        app.add_systems(Update, emit_for_test);

        app.update();

        let events: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<ConsumePoisonPillIntent>>()
            .drain()
            .collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].pill, PoisonPillKind::QingLinManTuo);
        assert_eq!(events[0].issued_at_tick, 42);
    }

    #[test]
    fn cast_lifespan_and_pressure_effects_emit_runtime_intents() {
        fn emit_for_test(
            mut effect_intents: ParamSet<(
                EventWriter<ApplyStatusEffectIntent>,
                EventWriter<LifespanExtensionIntent>,
                EventWriter<ConsumePoisonPillIntent>,
            )>,
        ) {
            apply_cast_item_effect(
                &ItemEffect::LifespanExtension {
                    years: 0,
                    source: "test_core".to_string(),
                },
                CastItemEffectTargets {
                    cultivation: None,
                    meridians: None,
                    contamination: None,
                    wounds: None,
                },
                &mut effect_intents,
                CastItemEffectContext {
                    issued_at_tick: 7,
                    username: "Azure",
                    entity: Entity::PLACEHOLDER,
                    item_freshness: None,
                    decay_profiles: None,
                },
            );
            apply_cast_item_effect(
                &ItemEffect::AntiSpiritPressure { duration_ticks: 0 },
                CastItemEffectTargets {
                    cultivation: None,
                    meridians: None,
                    contamination: None,
                    wounds: None,
                },
                &mut effect_intents,
                CastItemEffectContext {
                    issued_at_tick: 9,
                    username: "Azure",
                    entity: Entity::PLACEHOLDER,
                    item_freshness: None,
                    decay_profiles: None,
                },
            );
        }

        let mut app = App::new();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<LifespanExtensionIntent>();
        app.add_event::<ConsumePoisonPillIntent>();
        app.add_systems(Update, emit_for_test);

        app.update();

        let lifespan_events: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<LifespanExtensionIntent>>()
            .drain()
            .collect();
        assert_eq!(lifespan_events.len(), 1);
        assert_eq!(lifespan_events[0].requested_years, 1);
        assert_eq!(lifespan_events[0].source, "test_core");

        let status_events: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<ApplyStatusEffectIntent>>()
            .drain()
            .collect();
        assert_eq!(status_events.len(), 1);
        assert_eq!(
            status_events[0].kind,
            StatusEffectKind::AntiSpiritPressurePill
        );
        assert_eq!(status_events[0].duration_ticks, 1);
        assert_eq!(status_events[0].issued_at_tick, 9);
    }

    #[test]
    fn consume_one_stack_finds_in_hotbar() {
        let mut inv = make_inventory_with_stack(99, 10); // unrelated container item
        inv.hotbar[3] = Some(ItemInstance {
            instance_id: 7,
            template_id: "pill".to_string(),
            display_name: "丹".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.1,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 2,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        });
        assert!(consume_one_stack(&mut inv, 7));
        assert_eq!(inv.hotbar[3].as_ref().unwrap().stack_count, 1);
        assert!(consume_one_stack(&mut inv, 7));
        assert!(inv.hotbar[3].is_none());
    }

    // ── plan-food-v1 P2 BLOCKER 2：FoodRegen freshness 门控端到端测试 ──

    /// 辅助：构造带 FoodRegen effect 的 ApplyStatusEffectIntent 触发函数，
    /// 允许注入 freshness 和 decay profile registry。
    fn build_app_for_food_regen_test(
        freshness: Option<crate::shelflife::Freshness>,
        registry: Option<crate::shelflife::DecayProfileRegistry>,
        bonus_factor: f32,
        duration_ticks: u64,
    ) -> App {
        let mut app = App::new();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<LifespanExtensionIntent>();
        app.add_event::<ConsumePoisonPillIntent>();
        if let Some(r) = registry {
            app.insert_resource(r);
        }

        let freshness_clone = freshness.clone();
        app.add_systems(
            Update,
            move |mut effect_intents: ParamSet<(
                EventWriter<ApplyStatusEffectIntent>,
                EventWriter<LifespanExtensionIntent>,
                EventWriter<ConsumePoisonPillIntent>,
            )>,
                  reg: Option<Res<crate::shelflife::DecayProfileRegistry>>| {
                apply_cast_item_effect(
                    &ItemEffect::FoodRegen {
                        bonus_factor,
                        duration_ticks,
                    },
                    CastItemEffectTargets {
                        cultivation: None,
                        meridians: None,
                        contamination: None,
                        wounds: None,
                    },
                    &mut effect_intents,
                    CastItemEffectContext {
                        issued_at_tick: 1000,
                        username: "TestUser",
                        entity: Entity::PLACEHOLDER,
                        item_freshness: freshness_clone.clone(),
                        decay_profiles: reg.as_deref(),
                    },
                );
            },
        );
        app
    }

    /// BLOCKER 2 端到端：新鲜灵果（freshness 状态 Fresh）→ FoodRegen 发全额 bonus CultivationAcceleration intent
    #[test]
    fn food_regen_fresh_ling_guo_emits_full_bonus_intent() {
        use crate::inventory::freshness::GAME_DAY_TICKS;
        use crate::shelflife::types::{DecayFormula, DecayProfile, DecayProfileId, Freshness};

        let profile = DecayProfile::Spoil {
            id: DecayProfileId::new("food_spoil_ling_guo_v1"),
            formula: DecayFormula::Linear {
                decay_per_tick: 1.0 / (GAME_DAY_TICKS as f32 * 2.0),
            },
            spoil_threshold: 0.01,
        };
        let mut registry = crate::shelflife::DecayProfileRegistry::new();
        registry.insert(profile.clone()).unwrap();

        let freshness = Freshness::new(0, 1.0, &profile); // created at tick 0, full qi
                                                          // issued_at_tick=1000 → 已经过 1000 ticks，但 ling_guo 2 GAME_DAY = 48000 ticks，
                                                          // current_qi = 1.0 - 1000/(48000) ≈ 0.979 >> threshold 0.01 → FoodApplied

        let mut app = build_app_for_food_regen_test(Some(freshness), Some(registry), 0.20, 48_000);
        app.update();

        let intents: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<ApplyStatusEffectIntent>>()
            .drain()
            .collect();
        assert_eq!(
            intents.len(),
            1,
            "新鲜灵果应发出 1 条 CultivationAcceleration intent，实际 {}",
            intents.len()
        );
        let intent = &intents[0];
        assert_eq!(
            intent.kind,
            StatusEffectKind::CultivationAcceleration,
            "intent.kind 应为 CultivationAcceleration"
        );
        assert!(
            (intent.magnitude - 0.20).abs() < 1e-3,
            "新鲜灵果 magnitude 应=0.20（全额），实际 {}",
            intent.magnitude
        );
        assert_eq!(
            intent.duration_ticks, 48_000,
            "duration_ticks 应=48000，实际 {}",
            intent.duration_ticks
        );
    }

    /// BLOCKER 2 端到端：腐败到 CriticalBlock 的食物 → 不发 ApplyStatusEffectIntent
    #[test]
    fn food_regen_critical_block_emits_no_intent() {
        use crate::inventory::freshness::GAME_DAY_TICKS;
        use crate::shelflife::types::{DecayFormula, DecayProfile, DecayProfileId, Freshness};

        // spoil_threshold = 0.5，Linear decay_per_tick = 0.5/1
        // issued_at_tick = 1000 → 如果 created_at = 0 → current = 1.0 - 1.0*1000 < 0 → CriticalBlock
        // 但 Linear 会 clamp，我们用 threshold=0.5，decay_per_tick=1.0（每 tick 衰减 1），
        // current = 1.0 - 1000*1.0 ≈ 0（极低） < 0.1×0.5 → CriticalBlock
        let profile = DecayProfile::Spoil {
            id: DecayProfileId::new("test_fast_spoil"),
            formula: DecayFormula::Linear {
                decay_per_tick: 1.0 / (GAME_DAY_TICKS as f32 * 0.1),
            },
            spoil_threshold: 0.5,
        };
        let mut registry = crate::shelflife::DecayProfileRegistry::new();
        registry.insert(profile.clone()).unwrap();

        // issued_at_tick = 1000，created_at = 0 → 已经过 1000 ticks
        // ling_guo_fast: 0.1 GAME_DAY = 2400 ticks → linear threshold 在 ~2400 ticks 内到达 spoil
        // 1000 ticks 时 current = 1 - 1000/2400 ≈ 0.583 > threshold 0.5 → 不是 block
        // 改用大的 now_tick：用 freshness created_at=0 但构造时把 issued_at_tick 设成超大值
        // 更简单：用 Spoil Linear decay，让 (tick - created_at) >> half_life
        let fast_profile = DecayProfile::Spoil {
            id: DecayProfileId::new("food_regen_crit_test"),
            formula: DecayFormula::Linear {
                decay_per_tick: 1.0 / (100.0_f32), // 每 100 tick 耗尽 initial_qi
            },
            spoil_threshold: 0.5,
        };
        let mut registry2 = crate::shelflife::DecayProfileRegistry::new();
        registry2.insert(fast_profile.clone()).unwrap();

        // issued_at_tick = 1000 (in the test app)
        // created_at = 0, decay_per_tick = 1/100 → at tick 1000: current = 1 - (1000/100) = -9 → clamped 0
        // 0 < 0.1 × 0.5 = 0.05 → CriticalBlock
        let freshness = Freshness::new(0, 1.0, &fast_profile);

        let mut app = build_app_for_food_regen_test(Some(freshness), Some(registry2), 0.20, 48_000);
        app.update();

        let intents: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<ApplyStatusEffectIntent>>()
            .drain()
            .collect();
        assert_eq!(
            intents.len(),
            0,
            "CriticalBlock 应不发出任何 CultivationAcceleration intent，实际发出 {} 条",
            intents.len()
        );
    }

    /// BLOCKER 2 端到端：SpoiledWarn 状态食物 → 发出 reduced_bonus intent
    #[test]
    fn food_regen_spoiled_warn_emits_reduced_bonus_intent() {
        use crate::shelflife::types::{DecayFormula, DecayProfile, DecayProfileId, Freshness};

        // spoil_threshold=0.5，Linear decay：每 tick 衰减 1/12000（半衰=12000 ticks = 0.5 GAME_DAY）
        // issued_at_tick=1000, created_at=0
        // tick=1000 → current = 1 - 1000/12000 ≈ 0.917 → Safe（不 warn）
        // 用更快衰减：decay_per_tick = 1/3（每 3 tick 耗完），issued_at_tick=1000
        // current = 1 - 1000/3 ≈ -332 → CriticalBlock（太快）
        // 精心设计：spoil_threshold=0.5, decay_per_tick = 0.75/12000
        // at tick=1000: current = 1.0 - (1000/12000)*0.75 = 1.0 - 0.0625 = 0.9375 → Safe
        // 但我需要 Warn 区间：0.1×spoil_threshold <= current < spoil_threshold = 0.05 ~ 0.5
        // 设 spoil_threshold=0.5, decay so that current ≈ 0.25 at tick=1000
        // 0.25 = 1.0 - decay_per_tick × 1000 → decay_per_tick = 0.00075
        // 0.05 < 0.25 < 0.5 → SpoiledWarn ✓
        let warn_profile = DecayProfile::Spoil {
            id: DecayProfileId::new("food_regen_warn_test"),
            formula: DecayFormula::Linear {
                decay_per_tick: 0.00075,
            },
            spoil_threshold: 0.5,
        };
        let mut registry = crate::shelflife::DecayProfileRegistry::new();
        registry.insert(warn_profile.clone()).unwrap();

        let freshness = Freshness::new(0, 1.0, &warn_profile);

        let mut app = build_app_for_food_regen_test(Some(freshness), Some(registry), 0.20, 48_000);
        app.update();

        let intents: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<ApplyStatusEffectIntent>>()
            .drain()
            .collect();
        assert_eq!(
            intents.len(),
            1,
            "SpoiledWarn 应发出 1 条 intent（降效），实际 {} 条",
            intents.len()
        );
        let intent = &intents[0];
        assert_eq!(intent.kind, StatusEffectKind::CultivationAcceleration);
        assert!(
            intent.magnitude < 0.20,
            "SpoiledWarn intent.magnitude 应 < 0.20（降效），实际 {}",
            intent.magnitude
        );
        assert!(
            intent.magnitude > 0.0,
            "SpoiledWarn intent.magnitude 应 > 0.0（未全腐），实际 {}",
            intent.magnitude
        );
    }

    /// BLOCKER 2 端到端：无 freshness（Noop）→ 退化为原始 bonus intent（兼容旧物品）
    #[test]
    fn food_regen_no_freshness_noop_emits_original_bonus_intent() {
        let mut app = build_app_for_food_regen_test(None, None, 0.20, 48_000);
        app.update();

        let intents: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<ApplyStatusEffectIntent>>()
            .drain()
            .collect();
        assert_eq!(
            intents.len(),
            1,
            "Noop（无 freshness）应退化为发出 1 条原始 bonus intent，实际 {} 条",
            intents.len()
        );
        let intent = &intents[0];
        assert!(
            (intent.magnitude - 0.20).abs() < 1e-4,
            "Noop 路径 magnitude 应=0.20（原始值），实际 {}",
            intent.magnitude
        );
    }

    /// BLOCKER 1+2 全链路：food.toml 加载 → ling_guo effect → freshness gating → status 挂载
    #[test]
    fn food_regen_end_to_end_from_food_toml_ling_guo_fresh_to_intent() {
        use crate::inventory::freshness::GAME_DAY_TICKS;
        use crate::shelflife::types::{DecayFormula, DecayProfile, DecayProfileId, Freshness};

        // 1) 从 food.toml 加载 ling_guo 的 FoodRegen effect
        let registry = crate::inventory::load_item_registry()
            .expect("item registry 应从 assets/items/*.toml 加载成功");
        let ling_guo = registry
            .get("food.spirit_fruit.ling_guo")
            .expect("food.spirit_fruit.ling_guo 必须在 registry 中");
        let (bonus_factor, duration_ticks) = match &ling_guo.effect {
            Some(ItemEffect::FoodRegen {
                bonus_factor,
                duration_ticks,
            }) => (*bonus_factor, *duration_ticks),
            other => panic!("ling_guo.effect 应为 FoodRegen，实际 {other:?}"),
        };

        // 2) 构造 freshness（新鲜：created_at=0，issued_at=100，远未过 spoil）
        let spoil_profile = DecayProfile::Spoil {
            id: DecayProfileId::new("food_spoil_ling_guo_v1"),
            formula: DecayFormula::Linear {
                decay_per_tick: 1.0 / (GAME_DAY_TICKS as f32 * 2.0),
            },
            spoil_threshold: 0.01,
        };
        let mut decay_reg = crate::shelflife::DecayProfileRegistry::new();
        decay_reg.insert(spoil_profile.clone()).unwrap();

        let freshness = Freshness::new(0, 1.0, &spoil_profile);

        // 3) 触发 apply_cast_item_effect + freshness 门控 → 检查 intent
        let mut app = build_app_for_food_regen_test(
            Some(freshness),
            Some(decay_reg),
            bonus_factor,
            duration_ticks,
        );
        app.update();

        let intents: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<ApplyStatusEffectIntent>>()
            .drain()
            .collect();
        assert_eq!(
            intents.len(),
            1,
            "全链路：新鲜 ling_guo 应发出 1 条 CultivationAcceleration intent，实际 {} 条",
            intents.len()
        );
        let intent = &intents[0];
        assert_eq!(intent.kind, StatusEffectKind::CultivationAcceleration);
        assert!(
            (intent.magnitude - 0.20).abs() < 1e-3,
            "全链路：magnitude 应=0.20（food.toml 配置的 bonus_factor），实际 {}",
            intent.magnitude
        );
        assert_eq!(
            intent.duration_ticks, 48_000,
            "全链路：duration_ticks 应=48000（food.toml 配置的 2 GAME_DAY），实际 {}",
            intent.duration_ticks
        );
    }

    // ── plan-food-v1 P2 (opus 补测) — tick_casts_or_interrupt 全链路 CriticalBlock 不扣库存 ──

    /// 全链路：通过 tick_casts_or_interrupt system 验证极腐食物 CriticalBlock 不扣库存。
    ///
    /// 背景：headline fix 的 CriticalBlock 门控在 tick_casts_or_interrupt 内（cast_emit.rs:250-298）。
    /// 原有 food_regen_critical_block_emits_no_intent 直接调 apply_cast_item_effect，绕过了
    /// 真正的库存扣减路径。本测试走完整 system 链路：
    ///   Casting 完成 → CriticalBlock 检测 → 不扣 hotbar slot stack → 不发 ApplyStatusEffectIntent。
    #[test]
    fn tick_casts_or_interrupt_critical_block_does_not_consume_inventory() {
        use crate::combat::components::{
            CastSource, Casting, QuickSlotBindings, SkillBarBindings, Wounds,
        };
        use crate::combat::CombatClock;
        use crate::inventory::{
            ContainerState, InventoryRevision, ItemEffect, ItemInstance, ItemRarity, ItemRegistry,
            ItemTemplate, PlayerInventory, MAIN_PACK_CONTAINER_ID,
        };
        use crate::player::state::PlayerState;
        use crate::shelflife::types::{DecayFormula, DecayProfile, DecayProfileId, Freshness};
        use crate::shelflife::{DecayProfileRegistry, DecayTrack};
        use std::collections::HashMap;
        use valence::prelude::{DVec3, Position};
        use valence::testing::create_mock_client;

        const FOOD_ID: &str = "test.food.critical_rotten";
        const INSTANCE_ID: u64 = 777;
        const QUICK_SLOT: u8 = 0;
        // clock.tick = 1000; started_at_tick = 0, duration_ticks = 1 → 完成条件 1000 >= 1
        const CLOCK_TICK: u64 = 1000;

        // 1) 构造极腐 decay profile：decay_per_tick = 1/100 → at tick=1000, current ≈ 0 → CriticalBlock
        let fast_spoil = DecayProfile::Spoil {
            id: DecayProfileId::new("crit_block_test_profile"),
            formula: DecayFormula::Linear {
                decay_per_tick: 1.0 / 100.0, // 耗尽需 100 ticks；1000 ticks 时已归零
            },
            spoil_threshold: 0.5, // CriticalBlock 阈值: current < 0.1 * 0.5 = 0.05
        };
        let mut decay_reg = DecayProfileRegistry::new();
        decay_reg.insert(fast_spoil.clone()).unwrap();

        // 2) 食物 ItemTemplate（有 FoodRegen effect + shelflife_profile）
        let food_template = ItemTemplate {
            id: FOOD_ID.to_string(),
            display_name: "极腐食物".to_string(),
            category: crate::inventory::ItemCategory::Food,
            placeable: None,
            max_stack_count: 16,
            grid_w: 1,
            grid_h: 1,
            base_weight: 0.1,
            rarity: ItemRarity::Common,
            spirit_quality_initial: 1.0,
            description: String::new(),
            effect: Some(ItemEffect::FoodRegen {
                bonus_factor: 0.20,
                duration_ticks: 48_000,
            }),
            cast_duration_ms: 1000,
            cooldown_ms: 500,
            weapon_spec: None,
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
            technique_scroll_spec: None,
            recipe_fragment_spec: None,
            container_spec: None,
            shelflife_profile: Some("crit_block_test_profile".to_string()),
            shelflife_track: Some(DecayTrack::Spoil),
        };
        let mut templates = HashMap::new();
        templates.insert(FOOD_ID.to_string(), food_template);
        let item_registry = ItemRegistry::from_map(templates);

        // 3) 食物 ItemInstance：created_at=0 → at tick=1000 freshness 已归零 → CriticalBlock
        let food_freshness = Freshness::new(0, 1.0, &fast_spoil);
        let food_item = ItemInstance {
            instance_id: INSTANCE_ID,
            template_id: FOOD_ID.to_string(),
            display_name: "极腐食物".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.1,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 3, // 初始 3；CriticalBlock 路径不应扣减
            spirit_quality: 0.0,
            durability: 1.0,
            freshness: Some(food_freshness),
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        };
        let mut inventory = PlayerInventory {
            revision: InventoryRevision(1),
            containers: vec![ContainerState {
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 5,
                cols: 7,
                items: vec![],
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
        };
        inventory.hotbar[QUICK_SLOT as usize] = Some(food_item);

        // 4) 构造 Casting：QuickSlot，绑定食物 instance，已过完成时间
        let mut quick_slot_bindings = QuickSlotBindings::default();
        quick_slot_bindings.slots[QUICK_SLOT as usize] = Some(INSTANCE_ID);
        let casting = Casting {
            source: CastSource::QuickSlot,
            slot: QUICK_SLOT,
            started_at_tick: 0,
            duration_ticks: 1, // clock.tick=1000 >> 0+1 → 自然完成
            started_at_ms: 0,
            duration_ms: 50,
            bound_instance_id: Some(INSTANCE_ID),
            start_position: DVec3::ZERO,
            complete_cooldown_ticks: 20,
            skill_id: None,
            skill_config: None,
        };

        // 5) 搭 App
        let mut app = App::new();
        // Resources
        app.insert_resource(CombatClock { tick: CLOCK_TICK });
        app.insert_resource(item_registry);
        app.insert_resource(decay_reg);
        // AudioEmitWriter dependencies (all optional → skip if not needed, but
        // PlaySoundRecipeRequest event is required by the SystemParam)
        app.add_event::<crate::network::audio_event_emit::PlaySoundRecipeRequest>();
        app.add_event::<crate::combat::yidao::YidaoCastCompleteEvent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<crate::cultivation::lifespan::LifespanExtensionIntent>();
        app.add_event::<crate::cultivation::poison_trait::ConsumePoisonPillIntent>();
        // 注册 tick_casts_or_interrupt system
        app.add_systems(Update, tick_casts_or_interrupt);

        // 6) 生成 mock player entity（含 Client + Username）
        let (client_bundle, _helper) = create_mock_client("TestPlayer");
        let player = app
            .world_mut()
            .spawn(client_bundle)
            .insert((
                Position::new([0.0, 64.0, 0.0]),
                casting,
                Wounds::default(),
                inventory,
                PlayerState::default(),
                quick_slot_bindings,
                SkillBarBindings::default(),
            ))
            .id();

        // 7) 跑 system
        app.update();

        // 8) 断言：hotbar slot 的 stack_count 不变（CriticalBlock 不扣库存）
        let world = app.world_mut();
        let inv = world
            .entity(player)
            .get::<PlayerInventory>()
            .expect("PlayerInventory should still exist after tick_casts_or_interrupt");
        let actual_stack = inv.hotbar[QUICK_SLOT as usize]
            .as_ref()
            .map(|item| item.stack_count)
            .unwrap_or(0);
        assert_eq!(
            actual_stack, 3,
            "期望 hotbar slot stack_count=3（不变），因为 CriticalBlock 拒食后库存不应扣减；实际 stack={actual_stack}"
        );

        // 9) 断言：无 CultivationAcceleration ApplyStatusEffectIntent 发出
        let intents: Vec<_> = world
            .resource_mut::<Events<ApplyStatusEffectIntent>>()
            .drain()
            .collect();
        let culti_accel_count = intents
            .iter()
            .filter(|i| i.kind == StatusEffectKind::CultivationAcceleration)
            .count();
        assert_eq!(
            culti_accel_count, 0,
            "期望 0 条 CultivationAcceleration intent，因为 CriticalBlock 路径直接 continue 不调 apply_cast_item_effect；实际 {} 条",
            culti_accel_count
        );
    }

    #[test]
    fn cast_interrupt_audio_uses_recipe_attenuation() {
        fn emit_for_test(targets: Query<Entity, With<Position>>, mut audio: AudioEmitWriter) {
            let mut audio = audio.context();
            let entity = targets.single();
            let casting = Casting {
                source: CastSource::SkillBar,
                slot: 0,
                started_at_tick: 0,
                duration_ticks: 20,
                started_at_ms: 0,
                duration_ms: 1000,
                bound_instance_id: None,
                start_position: DVec3::ZERO,
                complete_cooldown_ticks: 20,
                skill_id: Some("xue_beng_bu".to_string()),
                skill_config: None,
            };
            emit_cast_interrupt_audio(&mut audio, entity, DVec3::new(1.0, 64.0, 1.0), &casting);
        }

        let mut app = App::new();
        app.insert_resource(
            crate::audio::SoundRecipeRegistry::load_default().expect("default recipes load"),
        );
        app.init_resource::<crate::audio::implementation::AudioImplementationDedup>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, emit_for_test);
        app.world_mut().spawn(Position::new([1.0, 64.0, 1.0]));

        app.update();

        let emitted: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<PlaySoundRecipeRequest>>()
            .drain()
            .collect();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].recipe_id, "cast_interrupt");
        assert!(matches!(
            emitted[0].recipient,
            AudioRecipient::Radius { .. }
        ));
    }
}
