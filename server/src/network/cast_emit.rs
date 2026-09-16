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
    Client, Commands, Entity, EventWriter, Mut, ParamSet, Position, Query, Res, UniqueId, Username,
};

use crate::alchemy::pill::apply_wound_heal;
use crate::combat::body_conditioning::{GuangboTicaoPracticeEvent, GUANGBO_TICAO_ID};
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
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::player::state::PlayerState;
use crate::schema::combat_hud::{CastOutcomeV1, CastPhaseV1, CastSyncV1};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::shelflife::DecayProfileRegistry;

/// Cooldown 默认值（plan §4.4）。中断后短冷却 0.5s（10 tick）；
/// 完成后冷却来自 ItemTemplate.cooldown_ms（折算到 Casting.complete_cooldown_ticks）。
pub const CAST_INTERRUPT_COOLDOWN_TICKS: u64 = 10;
/// plan §4.3 移动中断阈值（米）。超过即视为主动位移中断。
pub const CAST_MOVEMENT_INTERRUPT_THRESHOLD_M: f64 = 0.3;
/// plan-skill-anim-fidelity-v1 P2 后半（§8.1 #3）：打断分支停循环蓄力段动画的淡出。
const CAST_LOOP_ANIM_INTERRUPT_FADE_OUT_TICKS: u8 = 3;
/// 自然完成分支的循环段停止淡出（release 段随各招完成系统接力播出）。
const CAST_LOOP_ANIM_COMPLETE_FADE_OUT_TICKS: u8 = 2;

/// 通用 skill-bar 生命周期的自然完成消费者。启动校验与完成分派共用此判定，
/// 防止目录条目被允许进入 generic cast 后只上报 Completed 却没有任何 gameplay 结算。
pub fn has_direct_generic_completion_consumer(skill_id: &str) -> bool {
    skill_id == GUANGBO_TICAO_ID
}

/// plan-skill-anim-fidelity-v1 P2 后半（§8.1 #3）——走通用 `Casting` 状态机、
/// 起手播**循环蓄力段**动画的招式 → 循环 anim_id 查表（最小侵入方案：按
/// skill_id 查表，不给 `Casting` 组件加字段）。`tick_casts_or_interrupt` 的
/// 三打断分支与自然完成分支据此发 StopAnim；查表 miss（非循环段招式）不发。
///
/// 新增循环蓄力段招式时**必须**在此登记，否则打断后循环动画永卡在玩家身上
/// （§13 #6 停止路径红线——无停止路径的循环动画不予合入）。
fn looping_cast_anim_id(skill_id: &str) -> Option<&'static str> {
    match skill_id {
        crate::combat::sword_basics::SWORD_INFUSE_SKILL_ID => {
            Some(crate::combat::sword_basics::ANIM_SWORD_INFUSE_CHARGE)
        }
        // P4：yidao 5 招全部为长引导循环蓄力段（分表见 yidao.rs，映射由
        // YidaoSkillId::loop_anim_id 单源派生），非 yidao 前缀查表 miss 返 None。
        _ => crate::combat::yidao::yidao_loop_anim_for_skill_id(skill_id),
    }
}

/// 打断类停止的统一 fade_out（三打断分支 + 用户主动取消施法共用）。
pub(crate) const CAST_LOOP_ANIM_CANCEL_FADE_OUT_TICKS: u8 = CAST_LOOP_ANIM_INTERRUPT_FADE_OUT_TICKS;

/// 构造循环蓄力段的 `StopAnim` 请求；招式未登记循环段时返回 `None`。
///
/// 抽出为独立构造器是因为停止路径有两类调用方，各自持有的事件通道类型不同：
/// `tick_casts_or_interrupt` 用 `EventWriter`，而 `client_request_handler`
/// 的用户主动取消路径只能拿到 `&mut Events`（同一 system 内已有 `ResMut`，
/// 再加一个会触发 Bevy 资源冲突）。两侧共用本构造器保证 payload 一致。
pub(crate) fn cast_loop_stop_anim_request(
    skill_id: Option<&str>,
    unique_id: &UniqueId,
    position: valence::prelude::DVec3,
    fade_out_ticks: u8,
) -> Option<VfxEventRequest> {
    let anim_id = skill_id.and_then(looping_cast_anim_id)?;
    Some(VfxEventRequest::new(
        position,
        VfxEventPayloadV1::StopAnim {
            target_player: unique_id.0.to_string(),
            anim_id: anim_id.to_string(),
            fade_out_ticks: Some(fade_out_ticks),
        },
    ))
}

/// 若 `casting` 的招式登记了循环蓄力段，则对 caster 发 `StopAnim`（fade_out
/// 按分支传入）；未登记 / 实体缺 `UniqueId`（非玩家）时静默跳过。
fn stop_cast_loop_anim(
    casting: &Casting,
    entity: Entity,
    position: &Position,
    unique_ids: &Query<&UniqueId>,
    fade_out_ticks: u8,
    vfx_events: &mut EventWriter<VfxEventRequest>,
) {
    let Ok(unique_id) = unique_ids.get(entity) else {
        return;
    };
    let Some(request) = cast_loop_stop_anim_request(
        casting.skill_id.as_deref(),
        unique_id,
        position.get(),
        fade_out_ticks,
    ) else {
        return;
    };
    vfx_events.send(request);
}

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
    mut guangbo_practice_events: EventWriter<GuangboTicaoPracticeEvent>,
    mut effect_intents: ParamSet<(
        EventWriter<ApplyStatusEffectIntent>,
        EventWriter<LifespanExtensionIntent>,
        EventWriter<ConsumePoisonPillIntent>,
    )>,
    mut clients: Query<CastTickQueryItem<'_>>,
    unique_ids: Query<&UniqueId>,
    mut vfx_events: EventWriter<VfxEventRequest>,
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
            // §8.1 #3：控制打断停循环蓄力段（查表 miss 不发）。
            stop_cast_loop_anim(
                casting,
                entity,
                position,
                &unique_ids,
                CAST_LOOP_ANIM_INTERRUPT_FADE_OUT_TICKS,
                &mut vfx_events,
            );
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
            // §8.1 #3：受击打断停循环蓄力段。
            stop_cast_loop_anim(
                casting,
                entity,
                position,
                &unique_ids,
                CAST_LOOP_ANIM_INTERRUPT_FADE_OUT_TICKS,
                &mut vfx_events,
            );
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
            // §8.1 #3：移动打断停循环蓄力段。
            stop_cast_loop_anim(
                casting,
                entity,
                position,
                &unique_ids,
                CAST_LOOP_ANIM_INTERRUPT_FADE_OUT_TICKS,
                &mut vfx_events,
            );
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
            // §8.1 #3：自然完成也显式停循环蓄力段（防御性兜底——release 段由各招
            // 完成系统同拍接力播出，重复 StopAnim 对不同 anim_id 的 release 无影响）。
            stop_cast_loop_anim(
                casting,
                entity,
                position,
                &unique_ids,
                CAST_LOOP_ANIM_COMPLETE_FADE_OUT_TICKS,
                &mut vfx_events,
            );
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
            // 广播体操（body.guangbo_ticao）：cast 自然完成 = 一次练习。
            // 发 GuangboTicaoPracticeEvent → consume_guangbo_practice_events 走真元门
            // 扣 qi_cost 并递增 proficiency（守恒在消费侧；此处只负责"练习发生了"）。
            // AV（练习姿态 + 轻量正反馈粒子 + 伸展音）纯加法 cosmetic。
            if casting
                .skill_id
                .as_deref()
                .is_some_and(has_direct_generic_completion_consumer)
            {
                guangbo_practice_events.send(GuangboTicaoPracticeEvent { entity });
                emit_recipe_audio_with_context(
                    &mut audio_events,
                    "guangbo_ticao_practice",
                    entity,
                    position.get(),
                    None,
                    0.8,
                );
            }
            // 1) 消耗：物品快捷槽找到绑定 instance_id，stack -= 1；技能栏只进入冷却。
            let mut effect_to_apply: Option<ItemEffect> = None;
            // plan-food-v1 P2：在 consume_one_stack 借走 inventory 之前，先 clone freshness
            // （clone 避免生命周期冲突）。
            let mut cast_item_freshness: Option<crate::shelflife::Freshness> = None;
            if casting.source == CastSource::QuickSlot {
                if let Some(id) = casting.bound_instance_id {
                    if let Some(template_id) = lookup_template_id(&inventory, id) {
                        if let Some(template) = item_registry
                            .get(&template_id)
                            .filter(|template| template.is_quick_use_eligible())
                        {
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
                let freshness_profile = cast_item_freshness.as_ref().and_then(|freshness| {
                    decay_profiles
                        .as_deref()
                        .and_then(|registry| registry.get(&freshness.profile))
                });
                let freshness_pair = cast_item_freshness.as_ref().zip(freshness_profile);
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

            let consumed = if casting.source == CastSource::QuickSlot && effect_to_apply.is_some() {
                casting
                    .bound_instance_id
                    .map(|id| consume_one_stack(&mut inventory, id))
                    .unwrap_or(false)
            } else {
                false
            };
            // 2) 应用效果
            if let Some(effect) = effect_to_apply.as_ref().filter(|_| consumed) {
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
        // bughunt skillbar-rebind-cooldown-reset：SkillBarBindings 的冷却按 skill_id
        // 记账（不再按槽位），这里必须用 Casting.skill_id 而非 slot 作为 key——所有
        // 生产 SkillBar Casting 构造点都无条件填了 skill_id（见 dugu_v2/skills.rs、
        // sword_basics.rs、burst_meridian.rs 等 insert_casting/insert_instant_cast），
        // None 分支只是防御性兜底，理论不可达。
        CastSource::SkillBar => {
            if let Some(skill_id) = casting.skill_id.as_deref() {
                skillbar_bindings.set_cooldown(skill_id, until_tick);
            } else {
                tracing::warn!(
                    "[bong][network][cast] set_cast_cooldown: SkillBar Casting 缺 skill_id \
                     (slot={slot})，无法写入冷却——所有生产路径构造 SkillBar Casting 时都应带 \
                     skill_id，这里出现 None 说明某条构造路径遗漏了该字段"
                );
            }
        }
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
        .flat_map(|s| s.iter_all())
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
            let freshness_profile = context.item_freshness.as_ref().and_then(|freshness| {
                context
                    .decay_profiles
                    .and_then(|registry| registry.get(&freshness.profile))
            });
            let freshness_pair = context.item_freshness.as_ref().zip(freshness_profile);
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
    crate::inventory::consume_item_instance_once(inventory, instance_id).is_ok()
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
#[path = "cast_emit_tests.rs"]
mod tests;
