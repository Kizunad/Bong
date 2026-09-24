//! plan-sword-path-v2 P1.4 / P1.5 / P1.6 / P2.1 — 剑道五招的 SkillRegistry 入口、
//! 经脉依赖声明、命中效果接入、化虚 runtime。
//!
//! 五招遵循同一骨架：
//! 1. 校验持剑（`WeaponKind::Sword`）+ 冷却 + 体力 + 真元 + 经脉依赖。
//! 2. 校验是否拥有该招式（`KnownTechniques` active + proficiency）。
//! 3. 走 worldview §二 守恒律：真元消耗写 `Cultivation.qi_current`；下注真元到
//!    灵剑容器走 `QiTransfer { reason: Channeling }`；化虚释放走 `ReleaseToZone`。
//! 4. 应用效果（直接 AttackIntent / 状态效果 / 化形实体）。
//! 5. P4 AV 接线：cast 成功后 emit `SwordPathSkillCastEvent`（见 `av_event`），
//!    粒子 / 音效 / 动画由 `network::vfx_animation_trigger` +
//!    `network::audio_trigger` 双系统读事件后 emit，引用 client 已注册的
//!    `SwordPathVfxPlayer` 粒子 / `audio_recipes/sword_*.json` / `BongAnimations`。
//!    纯 cosmetic，不触碰战斗数值 / qi_physics ledger / 命中结算。

use std::collections::HashSet;

use valence::entity::Look;
use valence::prelude::{
    bevy_ecs, Commands, DVec3, Entity, EventReader, EventWriter, Events, Position, Query, Res,
    ResMut,
};

use crate::body_plan::intrinsic_is_humanoid_from_world;
use crate::combat::components::{
    Casting, SkillBarBindings, Stamina, StaminaState, StatusEffects, WoundKind,
};
use crate::combat::events::{
    ApplyStatusEffectIntent, AttackIntent, AttackReach, AttackSource, StatusEffectKind,
};
use crate::combat::weapon::{Weapon, WeaponKind};
use crate::combat::CombatClock;
use crate::cultivation::components::{Cultivation, MeridianId, MeridianSystem, Realm};
use crate::cultivation::known_techniques::{
    KnownTechniques, TechniqueDefinition, TechniqueRegistry,
};
use crate::cultivation::meridian::severed::{
    check_meridian_dependencies, MeridianSeveredPermanent, SkillMeridianDependencies,
};
use crate::cultivation::skill_registry::{CastRejectReason, CastResult, SkillRegistry};
use crate::cultivation::technique_scroll::realm_rank;
use crate::network::cast_emit::current_unix_millis;
use crate::qi_physics::{QiAccountId, QiTransfer, QiTransferReason};
use crate::world::zone::DEFAULT_SPAWN_ZONE_NAME;

use super::av_event::{SwordPathSkillCastEvent, SwordPathSkillId};
use super::bond::{SwordBondComponent, SwordShatterEvent};
#[cfg(test)]
use super::grade::SwordGrade;
use super::heaven_gate::{
    compute_heaven_gate_damage, create_blind_zone_from_cast, HeavenGateCastEvent,
    HeavenGateChanneling, TiandaoBlindZoneRegistry, HEAVEN_GATE_AOE_END, HEAVEN_GATE_CHARGE_END,
    HEAVEN_GATE_CRITICAL_END,
};
use super::shatter::compute_heaven_gate_shatter;
use super::sword_intent_entity::spawn_sword_intent_in_world;
use super::techniques::effects;

pub const SWORD_PATH_CONDENSE_EDGE_ID: &str = "sword_path.condense_edge";
pub const SWORD_PATH_QI_SLASH_ID: &str = "sword_path.qi_slash";
pub const SWORD_PATH_RESONANCE_ID: &str = "sword_path.resonance";
pub const SWORD_PATH_MANIFEST_ID: &str = "sword_path.manifest";
pub const SWORD_PATH_HEAVEN_GATE_ID: &str = "sword_path.heaven_gate";

/// P1.4 — 注册五招 SkillFn 到 `SkillRegistry`。由 `cultivation::skill_registry::init_registry`
/// 在启动期调用。
pub fn register_skills(registry: &mut SkillRegistry) {
    registry.register(SWORD_PATH_CONDENSE_EDGE_ID, cast_condense_edge);
    registry.register(SWORD_PATH_QI_SLASH_ID, cast_qi_slash);
    registry.register(SWORD_PATH_RESONANCE_ID, cast_resonance);
    registry.register(SWORD_PATH_MANIFEST_ID, cast_manifest);
    registry.register(SWORD_PATH_HEAVEN_GATE_ID, cast_heaven_gate);
}

/// P1.5 — 五招的经脉依赖按 worldview §四:286 + plan §P1.5 声明：
/// 凝锋 → 大肠/小肠；剑气斩/共鸣/化形 → +三焦；天门 → +督。
pub fn declare_meridian_dependencies(dependencies: &mut SkillMeridianDependencies) {
    let base = vec![MeridianId::LargeIntestine, MeridianId::SmallIntestine];
    let with_triple = || {
        vec![
            MeridianId::LargeIntestine,
            MeridianId::SmallIntestine,
            MeridianId::TripleEnergizer,
        ]
    };
    dependencies.declare(SWORD_PATH_CONDENSE_EDGE_ID, base);
    dependencies.declare(SWORD_PATH_QI_SLASH_ID, with_triple());
    dependencies.declare(SWORD_PATH_RESONANCE_ID, with_triple());
    dependencies.declare(SWORD_PATH_MANIFEST_ID, with_triple());
    dependencies.declare(
        SWORD_PATH_HEAVEN_GATE_ID,
        vec![
            MeridianId::LargeIntestine,
            MeridianId::SmallIntestine,
            MeridianId::TripleEnergizer,
            MeridianId::Du,
        ],
    );
}

// ─── 凝锋 ────────────────────────────────────────────────────────────────────

fn cast_condense_edge(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    let ctx = match build_cast_context(world, caster, slot, SWORD_PATH_CONDENSE_EDGE_ID) {
        Ok(ctx) => ctx,
        Err(reason) => return CastResult::Rejected { reason },
    };

    // 去掉"目标无效"门禁（Option B）：凝锋是近战剑势，准星没对准也照常挥出，
    // target 透传 Option —— 有目标命中、无目标 resolver 跳过即空挥（不命中、无误伤）。
    let qi_cost = ctx.definition.qi_cost;
    if !drain_qi(world, caster, qi_cost) {
        return CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient,
        };
    }
    apply_cast_costs(world, caster, slot, ctx.now_tick, &ctx.definition);
    let injected = inject_bond_qi(world, caster, qi_cost);
    credit_skill_qi_to_zone(world, caster, qi_cost, injected);

    world.send_event(AttackIntent {
        attacker: caster,
        target,
        issued_at_tick: ctx.now_tick,
        reach: AttackReach::new(ctx.definition.range, 0.5),
        qi_invest: effects::CONDENSE_EDGE_DAMAGE_MULT,
        wound_kind: WoundKind::Cut,
        source: AttackSource::SwordPathCondenseEdge,
        debug_command: None,
    });

    emit_skill_av(
        world,
        caster,
        SwordPathSkillId::CondenseEdge,
        ctx.now_tick,
        None,
    );

    CastResult::Started {
        cooldown_ticks: u64::from(ctx.definition.cooldown_ticks),
        anim_duration_ticks: ctx.definition.cast_ticks,
    }
}

// ─── 剑气斩 ──────────────────────────────────────────────────────────────────

fn cast_qi_slash(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    let ctx = match build_cast_context(world, caster, slot, SWORD_PATH_QI_SLASH_ID) {
        Ok(ctx) => ctx,
        Err(reason) => return CastResult::Rejected { reason },
    };

    // 去掉"目标无效"门禁（Option B）：剑气斩是方向招，准星没对准也照常释放，target
    // 透传 Option —— 有目标命中、无目标 resolver 跳过即空斩；朝向无目标时落到玩家面朝向。
    let qi_cost = ctx.definition.qi_cost;
    if !drain_qi(world, caster, qi_cost) {
        return CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient,
        };
    }
    apply_cast_costs(world, caster, slot, ctx.now_tick, &ctx.definition);
    let injected = inject_bond_qi(world, caster, qi_cost);
    credit_skill_qi_to_zone(world, caster, qi_cost, injected);

    world.send_event(AttackIntent {
        attacker: caster,
        target,
        issued_at_tick: ctx.now_tick,
        reach: AttackReach::new(ctx.definition.range, 0.0),
        qi_invest: ctx.definition.qi_cost as f32,
        wound_kind: WoundKind::Cut,
        source: AttackSource::SwordPathQiSlash,
        debug_command: None,
    });

    // 剑气斩走朝向 line trail：有目标=caster→target，无目标=玩家面朝向，退化落 +X。
    let direction = qi_slash_direction(world, caster, target);
    emit_skill_av(
        world,
        caster,
        SwordPathSkillId::QiSlash,
        ctx.now_tick,
        Some(direction),
    );

    CastResult::Started {
        cooldown_ticks: u64::from(ctx.definition.cooldown_ticks),
        anim_duration_ticks: ctx.definition.cast_ticks,
    }
}

/// 剑气斩朝向：
/// - 有目标：caster → target 单位向量（原行为）。
/// - 无目标（Option B 空斩）：玩家水平面朝向（Look.yaw）。
/// - 任一退化（Position/Look 缺失、两点重合）：落到 +X，绝不 NaN。
fn qi_slash_direction(
    world: &bevy_ecs::world::World,
    caster: Entity,
    target: Option<Entity>,
) -> DVec3 {
    let caster_pos = world.get::<Position>(caster).map(|p| p.get());
    if let (Some(from), Some(target)) = (caster_pos, target) {
        if let Some(to) = world.get::<Position>(target).map(|p| p.get()) {
            let delta = to - from;
            if delta.length_squared() > f64::EPSILON {
                return delta.normalize();
            }
        }
    }
    // 无目标：落到玩家面朝向（与 woliu_v2 朝向数学一致）。
    if let Some(look) = world.get::<Look>(caster) {
        let yaw = f64::from(look.yaw).to_radians();
        let facing = DVec3::new(-yaw.sin(), 0.0, yaw.cos());
        if facing.is_finite() {
            return facing;
        }
    }
    DVec3::new(1.0, 0.0, 0.0)
}

// ─── 剑鸣 ────────────────────────────────────────────────────────────────────

fn cast_resonance(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    _target: Option<Entity>,
) -> CastResult {
    let ctx = match build_cast_context(world, caster, slot, SWORD_PATH_RESONANCE_ID) {
        Ok(ctx) => ctx,
        Err(reason) => return CastResult::Rejected { reason },
    };

    let qi_cost = ctx.definition.qi_cost;
    if !drain_qi(world, caster, qi_cost) {
        return CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient,
        };
    }
    apply_cast_costs(world, caster, slot, ctx.now_tick, &ctx.definition);
    let injected = inject_bond_qi(world, caster, qi_cost);
    credit_skill_qi_to_zone(world, caster, qi_cost, injected);

    // 6 格 AoE：扫范围内有 StatusEffects 的实体打 Slowed。
    // 范围内目标列表先 collect 出来，避免持有 query borrow 时 send_event。
    let center = world
        .get::<Position>(caster)
        .map(|p| p.get())
        .unwrap_or(DVec3::ZERO);
    let radius_sq = f64::from(ctx.definition.range).powi(2);
    let mut targets: Vec<Entity> = Vec::new();
    let mut query = world.query::<(Entity, &Position, &StatusEffects)>();
    for (entity, position, _) in query.iter(world) {
        if entity == caster {
            continue;
        }
        if position.get().distance_squared(center) <= radius_sq {
            targets.push(entity);
        }
    }
    for target in targets {
        world.send_event(ApplyStatusEffectIntent {
            target,
            kind: StatusEffectKind::Slowed,
            magnitude: 0.5,
            duration_ticks: (effects::RESONANCE_SLOW_MIN_SECS * 20.0) as u64,
            issued_at_tick: ctx.now_tick,
        });
    }

    emit_skill_av(
        world,
        caster,
        SwordPathSkillId::Resonance,
        ctx.now_tick,
        None,
    );

    CastResult::Started {
        cooldown_ticks: u64::from(ctx.definition.cooldown_ticks),
        anim_duration_ticks: ctx.definition.cast_ticks,
    }
}

// ─── 剑意化形 ────────────────────────────────────────────────────────────────

fn cast_manifest(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    target: Option<Entity>,
) -> CastResult {
    let ctx = match build_cast_context(world, caster, slot, SWORD_PATH_MANIFEST_ID) {
        Ok(ctx) => ctx,
        Err(reason) => return CastResult::Rejected { reason },
    };
    // 去掉"目标无效"门禁（Option B）：化形是方向招，准星没对准也照常释放，target 透传
    // Option —— 有目标命中、无目标 resolver 跳过即空放（不命中、无误伤）。
    let qi_cost = ctx.definition.qi_cost;
    if !drain_qi(world, caster, qi_cost) {
        return CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient,
        };
    }
    apply_cast_costs(world, caster, slot, ctx.now_tick, &ctx.definition);
    let injected = inject_bond_qi(world, caster, qi_cost);
    credit_skill_qi_to_zone(world, caster, qi_cost, injected);

    // 化形完整版（plan-sword-path-complete §C）：spawn SwordIntentEntity 追踪实体，
    // 5s 内追击目标 5 次，每次发 AttackIntent。
    // qi_invest = MANIFEST_ATTACK_MULT，与 condense_edge/qi_slash 同族（伤害倍率走 qi_invest 字段）；
    // sword 源在 resolver 免扣 qi，实际单次伤害由 resolver 以 attacker attack_power × 倍率缩放，
    // 不在此处手乘 base（否则与同族招式不一致）。
    let origin = world
        .get::<Position>(caster)
        .map(|p| p.get())
        .unwrap_or(DVec3::ZERO);
    let damage_per_hit = f64::from(effects::MANIFEST_ATTACK_MULT);
    spawn_sword_intent_in_world(
        world,
        caster,
        origin,
        target,
        damage_per_hit,
        AttackSource::SwordPathManifest,
    );

    // 化形结束后 bond_strength -= 0.1 (plan §techniques::effects::MANIFEST_BOND_PENALTY)
    if let Some(mut bond) = world.get_mut::<SwordBondComponent>(caster) {
        bond.bond_strength = (bond.bond_strength - effects::MANIFEST_BOND_PENALTY).max(0.0);
    }

    emit_skill_av(
        world,
        caster,
        SwordPathSkillId::Manifest,
        ctx.now_tick,
        None,
    );

    CastResult::Started {
        cooldown_ticks: u64::from(ctx.definition.cooldown_ticks),
        anim_duration_ticks: ctx.definition.cast_ticks,
    }
}

// ─── 化虚·一剑开天门 ─────────────────────────────────────────────────────────

fn cast_heaven_gate(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    _target: Option<Entity>,
) -> CastResult {
    let ctx = match build_cast_context(world, caster, slot, SWORD_PATH_HEAVEN_GATE_ID) {
        Ok(ctx) => ctx,
        Err(reason) => return CastResult::Rejected { reason },
    };

    let Some(cultivation) = world.get::<Cultivation>(caster).cloned() else {
        return CastResult::Rejected {
            reason: CastRejectReason::RealmTooLow,
        };
    };
    let position = world
        .get::<Position>(caster)
        .map(|p| p.get())
        .unwrap_or(DVec3::ZERO);

    // 化虚四阶段（plan-sword-path-complete §D）：
    // cast 时插入 HeavenGateChanneling 组件，由 heaven_gate_phase_system 分四阶段推进，
    // 不再立即发 HeavenGateCastEvent（legacy 路径保留，但 cast 不触发它）。
    apply_cast_costs(world, caster, slot, ctx.now_tick, &ctx.definition);
    let stored_qi = world
        .get::<SwordBondComponent>(caster)
        .map(|b| b.stored_qi)
        .unwrap_or(0.0);
    world.entity_mut(caster).insert(HeavenGateChanneling {
        caster,
        start_tick: ctx.now_tick,
        position,
        range: f64::from(ctx.definition.range),
        qi_max: cultivation.qi_max,
        stored_qi,
        aoe_done: false,
    });

    // 蓄力阶段 AV：天门 charge 动画 + 蓄力粒子（elapsed==0 对应蓄力开始）。
    // phase system 在 elapsed==60 emit charge_1s/flash，elapsed>=140 emit release。
    emit_skill_av(
        world,
        caster,
        SwordPathSkillId::HeavenGateCharge,
        ctx.now_tick,
        None,
    );

    CastResult::Started {
        cooldown_ticks: u64::from(ctx.definition.cooldown_ticks),
        anim_duration_ticks: ctx.definition.cast_ticks,
    }
}

/// P2.1 — `HeavenGateCastEvent` → 化虚结算：
/// 1. 计算 `staging_buffer = qi_max + stored_qi`
/// 2. 100 格范围 AoE：按 `compute_heaven_gate_damage(staging, dist)` 发 AttackIntent
/// 3. `Cultivation.qi_max *= HEAVEN_GATE_QI_MAX_RETAIN`（10% 保留），`qi_current = 0`
/// 4. 境界跌至固元（plan §techniques::effects + worldview §三:128）
/// 5. 注册 `TiandaoBlindZone`（5 min TTL）
/// 6. 发 `SwordShatterEvent`（灵剑碎裂，反噬走 sword_shatter_system）
/// 7. 把 staging_buffer 通过 QiTransfer ledger 释放回所在 zone，守 worldview §二
#[allow(clippy::too_many_arguments)]
pub fn heaven_gate_cast_system(
    clock: Res<CombatClock>,
    mut events: EventReader<HeavenGateCastEvent>,
    mut players: Query<(&mut Cultivation, Option<&mut SwordBondComponent>)>,
    targets: Query<(Entity, &Position)>,
    mut combat_intents: Option<ResMut<Events<AttackIntent>>>,
    mut shatter_events: Option<ResMut<Events<SwordShatterEvent>>>,
    mut qi_transfers: Option<ResMut<Events<QiTransfer>>>,
    technique_registry: Res<TechniqueRegistry>,
    mut blind_registry: ResMut<TiandaoBlindZoneRegistry>,
    mut zone_registry: Option<ResMut<crate::world::zone::ZoneRegistry>>,
    mut av_events: EventWriter<SwordPathSkillCastEvent>,
) {
    let heaven_gate_range = f64::from(
        technique_registry
            .get(SWORD_PATH_HEAVEN_GATE_ID)
            .expect("validated TechniqueRegistry must contain sword_path.heaven_gate")
            .range,
    );
    // 多 caster 同 tick 触发的情况罕见，但为了保证 deterministic 排序，按 Entity bits 排序。
    let mut pending: Vec<HeavenGateCastEvent> = events.read().cloned().collect();
    pending.sort_by_key(|e| e.caster.to_bits());
    for event in pending {
        let staging_buffer = event.qi_max + event.stored_qi;

        // 释放阶段 AV：一剑开天的 flash / shockwave / release 动画（纯 cosmetic，
        // 在 caster 当前位置触发）。蓄力阶段 AV 已在 cast_heaven_gate emit。
        av_events.send(SwordPathSkillCastEvent {
            skill: SwordPathSkillId::HeavenGateRelease,
            caster: event.caster,
            center: event.position,
            direction: None,
            tick: clock.tick,
        });

        // 100 格 AoE：每个范围内目标按距离衰减伤害。已死 / 无 Position 的略过。
        let center = event.position;
        let radius_sq = heaven_gate_range.powi(2);
        // 避免对自己造成伤害（caster 本身处理 shatter）。
        let mut emitted_targets: HashSet<Entity> = HashSet::new();
        for (entity, position) in targets.iter() {
            if entity == event.caster {
                continue;
            }
            let dist_sq = position.get().distance_squared(center);
            if dist_sq > radius_sq {
                continue;
            }
            if !emitted_targets.insert(entity) {
                continue;
            }
            let damage =
                super::heaven_gate::compute_heaven_gate_damage(staging_buffer, dist_sq.sqrt());
            if let Some(intents) = combat_intents.as_deref_mut() {
                intents.send(AttackIntent {
                    attacker: event.caster,
                    target: Some(entity),
                    issued_at_tick: clock.tick,
                    reach: AttackReach::new(heaven_gate_range as f32, 0.0),
                    qi_invest: damage as f32,
                    wound_kind: WoundKind::Cut,
                    source: AttackSource::SwordPathHeavenGate,
                    debug_command: None,
                });
            }
        }

        // Caster 修为 / 灵剑 aftermath（化虚自带完整结算：qi 归零、qi_max ×0.1、
        // 境界跌固元、灵剑 stored_qi 清零）。
        //
        // **不**再额外 emit `SwordShatterEvent`——下面 `staging_buffer` 通过
        // `QiTransfer::ReleaseToZone` 已经把全部真元（qi_max + bond.stored_qi）
        // 走 ledger 回灌 zone。如果再让 `sword_shatter_system` 按 stored_qi 走
        // 一次反噬，就会重复扣 qi_max / 重复写 ledger，破坏 worldview §二 守恒。
        //
        // 化虚走单向门 - cast 即结算 - 视觉碎剑 / 开天 release AV 由本 system
        // 上方 emit 的 SwordPathSkillCastEvent(HeavenGateRelease) 独立触发（见
        // network::vfx_animation_trigger / audio_trigger），逻辑上不走通用 shatter pipeline。
        let _shatter_events_unused = shatter_events.as_deref_mut(); // 保留 ResMut 借出以维持系统签名兼容性
                                                                    // 守恒修复（#qi-sweep-heaven-gate-drain）：在归零前先快照 qi_current，
                                                                    // 随后直写 zone.spirit_qi。QiTransfer 是 audit-only（无 EventReader），
                                                                    // 不能代替 zone.spirit_qi 直写，见 sword_basics.rs:433 注释。
        let qi_drained = if let Ok((mut cultivation, bond_opt)) = players.get_mut(event.caster) {
            let qi_drained = cultivation.qi_current;
            cultivation.qi_max = (cultivation.qi_max * effects::HEAVEN_GATE_QI_MAX_RETAIN).max(0.0);
            cultivation.qi_current = 0.0;
            cultivation.realm = Realm::Solidify;
            if let Some(mut bond) = bond_opt {
                bond.stored_qi = 0.0;
            }
            qi_drained
        } else {
            0.0
        };
        // players borrow 已释放——现在可以安全地写 zone_registry。
        credit_qi_current_to_zone(
            event.caster,
            event.position,
            qi_drained,
            zone_registry.as_deref_mut(),
            qi_transfers.as_deref_mut(),
        );

        // 盲区注册：把 caster 藏 5 min，agent world_state 不再推送其 snapshot。
        let zone = create_blind_zone_from_cast(&event, clock.tick, heaven_gate_range);
        blind_registry.add(zone);

        // 守恒：staging_buffer 进 caster 当前所在 zone（worldview §二 真元守恒、
        // zone 级储量必须按位置归账）。compute 函数算出 qi_max_lost / new_qi_max
        // 数值已在上面写入 cultivation，这里仅保留 ledger entry 并解析目标 zone。
        let _outcome = compute_heaven_gate_shatter(event.qi_max, event.stored_qi);
        let target_zone = zone_registry
            .as_deref()
            .and_then(|r| {
                r.find_zone(
                    crate::world::dimension::DimensionKind::Overworld,
                    event.position,
                )
            })
            .map(|z| z.name.clone())
            .unwrap_or_else(|| DEFAULT_SPAWN_ZONE_NAME.to_string());
        if let Some(transfers) = qi_transfers.as_deref_mut() {
            if let Ok(transfer) = QiTransfer::new(
                QiAccountId::player(format!("entity:{:?}", event.caster)),
                QiAccountId::zone(target_zone),
                staging_buffer,
                QiTransferReason::ReleaseToZone,
            ) {
                transfers.send(transfer);
            }
        }
    }
}

// ─── 天门四阶段 phase system（plan-sword-path-complete §D）────────────────────

/// 粒子 / 阶段 VFX event id（对齐 §1.2 常数表）。
const VFX_HEAVEN_GATE_CHARGE_0S: &str = "bong:heaven_gate_charge_0s";
const VFX_HEAVEN_GATE_CHARGE_1S: &str = "bong:heaven_gate_charge_1s";
const VFX_HEAVEN_GATE_FLASH: &str = "bong:heaven_gate_flash";
const VFX_HEAVEN_GATE_SHOCKWAVE: &str = "bong:heaven_gate_shockwave";
const VFX_HEAVEN_GATE_RELEASE_PARTICLE: &str = "bong:heaven_gate_release";

/// 天门四阶段 system。替代旧的"cast 即结算"模式，按 elapsed 推进：
///
/// - elapsed == 0:  蓄力粒子 `bong:heaven_gate_charge_0s`
/// - elapsed == 60: 临界粒子 `bong:heaven_gate_charge_1s` + `bong:heaven_gate_flash`
/// - elapsed == 120 && !aoe_done: AoE 结算 + `bong:heaven_gate_shockwave`
/// - elapsed >= 140: aftermath（qi 归零 / realm 跌落 / 盲区 / QiTransfer）+ `bong:heaven_gate_release`
///
/// **守恒不变量**：aftermath 的账目逐字搬运自 `heaven_gate_cast_system`（net 不变）。
#[allow(clippy::too_many_arguments)]
pub fn heaven_gate_phase_system(
    clock: Res<CombatClock>,
    mut channeling_q: Query<(Entity, &mut HeavenGateChanneling)>,
    mut players: Query<(
        &mut crate::cultivation::components::Cultivation,
        Option<&mut SwordBondComponent>,
    )>,
    targets: Query<(Entity, &Position)>,
    mut combat_intents: Option<ResMut<Events<AttackIntent>>>,
    mut qi_transfers: Option<ResMut<Events<QiTransfer>>>,
    mut blind_registry: ResMut<TiandaoBlindZoneRegistry>,
    mut zone_registry: Option<ResMut<crate::world::zone::ZoneRegistry>>,
    mut av_events: EventWriter<SwordPathSkillCastEvent>,
    mut vfx_events: EventWriter<crate::network::vfx_event_emit::VfxEventRequest>,
    mut commands: Commands,
) {
    use crate::cultivation::components::Realm;
    use crate::schema::vfx_event::VfxEventPayloadV1;

    let now = clock.tick;

    for (channeling_entity, mut channeling) in &mut channeling_q {
        let elapsed = (now.saturating_sub(channeling.start_tick)) as u32;
        let origin = channeling.position;

        // ── 阶段 0: 蓄力粒子（elapsed == 0 / cast 开始那帧）──────────────────
        if elapsed == 0 {
            vfx_events.send(crate::network::vfx_event_emit::VfxEventRequest::new(
                origin,
                VfxEventPayloadV1::SpawnParticle {
                    event_id: VFX_HEAVEN_GATE_CHARGE_0S.to_string(),
                    origin: [origin.x, origin.y, origin.z],
                    direction: None,
                    color: Some("#E8F0FF".to_string()),
                    strength: Some(0.8),
                    count: Some(12),
                    duration_ticks: Some(40),
                },
            ));
        }

        // ── 阶段 1: 临界（elapsed == CHARGE_END = 60）───────────────────────────
        if elapsed == HEAVEN_GATE_CHARGE_END {
            vfx_events.send(crate::network::vfx_event_emit::VfxEventRequest::new(
                origin,
                VfxEventPayloadV1::SpawnParticle {
                    event_id: VFX_HEAVEN_GATE_CHARGE_1S.to_string(),
                    origin: [origin.x, origin.y, origin.z],
                    direction: None,
                    color: Some("#E8F0FF".to_string()),
                    strength: Some(0.9),
                    count: Some(16),
                    duration_ticks: Some(30),
                },
            ));
            vfx_events.send(crate::network::vfx_event_emit::VfxEventRequest::new(
                origin,
                VfxEventPayloadV1::SpawnParticle {
                    event_id: VFX_HEAVEN_GATE_FLASH.to_string(),
                    origin: [origin.x, origin.y, origin.z],
                    direction: None,
                    color: Some("#FFFFFF".to_string()),
                    strength: Some(1.0),
                    count: Some(4),
                    duration_ticks: Some(8),
                },
            ));
        }

        // ── 阶段 2: AoE 结算（elapsed >= CRITICAL_END = 120，aoe_done guard 保证仅一次）──
        // 用 >= 而非 ==：若服务器跳过第 120 帧，仍在首个 >=120 帧补发 AoE，不被跳帧吞掉。
        if elapsed >= HEAVEN_GATE_CRITICAL_END && !channeling.aoe_done {
            let staging_buffer = channeling.qi_max + channeling.stored_qi;
            let radius_sq = channeling.range.powi(2);
            let mut emitted_targets = std::collections::HashSet::new();
            for (entity, position) in targets.iter() {
                if entity == channeling.caster {
                    continue;
                }
                let dist_sq = position.get().distance_squared(origin);
                if dist_sq > radius_sq {
                    continue;
                }
                if !emitted_targets.insert(entity) {
                    continue;
                }
                let damage = compute_heaven_gate_damage(staging_buffer, dist_sq.sqrt());
                if let Some(intents) = combat_intents.as_deref_mut() {
                    intents.send(AttackIntent {
                        attacker: channeling.caster,
                        target: Some(entity),
                        issued_at_tick: now,
                        reach: AttackReach::new(channeling.range as f32, 0.0),
                        qi_invest: damage as f32,
                        wound_kind: WoundKind::Cut,
                        source: AttackSource::SwordPathHeavenGate,
                        debug_command: None,
                    });
                }
            }
            vfx_events.send(crate::network::vfx_event_emit::VfxEventRequest::new(
                origin,
                VfxEventPayloadV1::SpawnParticle {
                    event_id: VFX_HEAVEN_GATE_SHOCKWAVE.to_string(),
                    origin: [origin.x, origin.y, origin.z],
                    direction: None,
                    color: Some("#E8F0FF".to_string()),
                    strength: Some(1.0),
                    count: Some(32),
                    duration_ticks: Some(20),
                },
            ));
            channeling.aoe_done = true;
        }

        // ── 阶段 3: aftermath（elapsed >= AOE_END = 140）────────────────────────
        if elapsed >= HEAVEN_GATE_AOE_END {
            let staging_buffer = channeling.qi_max + channeling.stored_qi;
            let caster = channeling.caster;

            // Caster 修为 / 灵剑 aftermath（与 heaven_gate_cast_system 完全一致的账目）
            // 守恒修复（#qi-sweep-heaven-gate-drain）：先快照 qi_current，归零后直写 zone。
            let qi_drained = if let Ok((mut cultivation, bond_opt)) = players.get_mut(caster) {
                // 用 cast 时快照 channeling.qi_max（非 aftermath 时刻的 live 值），
                // 与 ledger 释放的 staging_buffer 快照一致；蓄力期间若有 buff 改 qi_max 不致账目漂移。
                let qi_drained = cultivation.qi_current;
                cultivation.qi_max = (channeling.qi_max
                    * super::techniques::effects::HEAVEN_GATE_QI_MAX_RETAIN)
                    .max(0.0);
                cultivation.qi_current = 0.0;
                cultivation.realm = Realm::Solidify;
                if let Some(mut bond) = bond_opt {
                    bond.stored_qi = 0.0;
                }
                qi_drained
            } else {
                0.0
            };
            // players borrow 已释放——现在可以安全地写 zone_registry。
            credit_qi_current_to_zone(
                caster,
                origin,
                qi_drained,
                zone_registry.as_deref_mut(),
                qi_transfers.as_deref_mut(),
            );

            // 盲区注册
            let blind_zone_event = HeavenGateCastEvent {
                caster,
                position: origin,
                qi_max: channeling.qi_max,
                stored_qi: channeling.stored_qi,
            };
            let zone = create_blind_zone_from_cast(&blind_zone_event, now, channeling.range);
            blind_registry.add(zone);

            // 守恒：staging_buffer 通过 QiTransfer ledger 释放回 zone。
            let target_zone = zone_registry
                .as_deref()
                .and_then(|r| {
                    r.find_zone(crate::world::dimension::DimensionKind::Overworld, origin)
                })
                .map(|z| z.name.clone())
                .unwrap_or_else(|| DEFAULT_SPAWN_ZONE_NAME.to_string());
            if let Some(transfers) = qi_transfers.as_deref_mut() {
                if let Ok(transfer) = QiTransfer::new(
                    QiAccountId::player(format!("entity:{caster:?}")),
                    QiAccountId::zone(target_zone),
                    staging_buffer,
                    QiTransferReason::ReleaseToZone,
                ) {
                    transfers.send(transfer);
                }
            }

            // Release AV（一剑开天 flash / release 动画）
            av_events.send(SwordPathSkillCastEvent {
                skill: SwordPathSkillId::HeavenGateRelease,
                caster,
                center: origin,
                direction: None,
                tick: now,
            });
            vfx_events.send(crate::network::vfx_event_emit::VfxEventRequest::new(
                origin,
                VfxEventPayloadV1::SpawnParticle {
                    event_id: VFX_HEAVEN_GATE_RELEASE_PARTICLE.to_string(),
                    origin: [origin.x, origin.y, origin.z],
                    direction: None,
                    color: Some("#E8F0FF".to_string()),
                    strength: Some(1.0),
                    count: Some(24),
                    duration_ticks: Some(30),
                },
            ));

            // 移除 channeling 组件（channel 结束）
            commands
                .entity(channeling_entity)
                .remove::<HeavenGateChanneling>();
        }
    }
}

// ─── 共用工具 ────────────────────────────────────────────────────────────────

struct CastContext {
    now_tick: u64,
    definition: TechniqueDefinition,
}

fn build_cast_context(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    _slot: u8,
    skill_id: &str,
) -> Result<CastContext, CastRejectReason> {
    let definition = world
        .get_resource::<TechniqueRegistry>()
        .expect("cultivation::register must insert TechniqueRegistry before skill resolution")
        .get(skill_id)
        .unwrap_or_else(|| panic!("validated TechniqueRegistry must contain {skill_id}"))
        .clone();
    let now_tick = world
        .get_resource::<CombatClock>()
        .map(|c| c.tick)
        .unwrap_or_default();

    // 冷却（plan §SkillBarBindings）——按 skill_id 记账，与槽位无关。
    if world
        .get::<SkillBarBindings>(caster)
        .is_some_and(|b| b.is_on_cooldown(skill_id, now_tick))
    {
        return Err(CastRejectReason::OnCooldown);
    }

    // 持剑（必须 WeaponKind::Sword）——报 NoWeapon 而非 InvalidTarget：
    // 剑道招式全是方向技/自身 AoE，从不读锁定目标，"目标无效"是误导文案
    // （一剑开天门贴脸锁妖兽连刷"目标无效"实证；sword_basics 已是此做法）。
    let Some(weapon) = world.get::<Weapon>(caster) else {
        return Err(CastRejectReason::NoWeapon);
    };
    if weapon.weapon_kind != WeaponKind::Sword {
        return Err(CastRejectReason::NoWeapon);
    }

    // 体力前置校验（review fix）：除了 Exhausted/≤0 之外，还要保证当前体力够支付
    // definition.stamina_cost——否则 cast 走到 drain_qi 扣完真元再失败就是脏状态。
    if let Some(stamina) = world.get::<Stamina>(caster) {
        if stamina.state == StaminaState::Exhausted || stamina.current <= 0.0 {
            return Err(CastRejectReason::InRecovery);
        }
        if definition.stamina_cost > 0.0 && stamina.current < definition.stamina_cost {
            return Err(CastRejectReason::InRecovery);
        }
    }

    // 招式拥有 + active——同理拆出专用原因，client 显示"招式未激活"。
    let Some(known) = world.get::<KnownTechniques>(caster) else {
        return Err(CastRejectReason::TechniqueInactive);
    };
    if !known.entries.iter().any(|e| e.id == skill_id && e.active) {
        return Err(CastRejectReason::TechniqueInactive);
    }

    // plan-race-system-v1 P3a（决议 §8.1 #5/#6）—— race gate：拥有门后、境界门前。
    // 剑道五招（sword_path.*）全数据表标 RaceGate::Humanoid（依赖人体专属经脉拓扑 +
    // 双臂持械机能），本体非人形（`intrinsic_is_humanoid_from_world` 判 false）一律拒绝。
    let cultivation_race = world
        .get::<Cultivation>(caster)
        .map(|cultivation| cultivation.race.clone())
        .unwrap_or_else(|| crate::body_plan::RaceId::new(crate::body_plan::HUMAN_RACE_ID));
    let intrinsic_is_humanoid = intrinsic_is_humanoid_from_world(world, caster);
    if !definition
        .required_race
        .allows(&cultivation_race, intrinsic_is_humanoid)
    {
        return Err(CastRejectReason::RaceMismatch);
    }

    // 境界（plan §techniques::required_realm）直接消费同一条 TOML metadata。
    let cultivation = world
        .get::<Cultivation>(caster)
        .cloned()
        .ok_or(CastRejectReason::RealmTooLow)?;
    if realm_rank(cultivation.realm) < realm_rank(definition.required_realm_value()) {
        return Err(CastRejectReason::RealmTooLow);
    }

    // 经脉依赖（plan §P1.5）。SkillMeridianDependencies 是 Resource，缺则视为
    // 不限制（与 sword_basics 现有行为一致）。
    if let Some(deps_resource) = world.get_resource::<SkillMeridianDependencies>() {
        let deps = deps_resource.lookup(skill_id).to_vec();
        if !deps.is_empty() {
            let severed = world.get::<MeridianSeveredPermanent>(caster);
            if let Err(channel) = check_meridian_dependencies(&deps, severed) {
                return Err(CastRejectReason::MeridianSevered(Some(channel)));
            }
            // 同时校验当前 integrity（worldview §四:286）：完全 SEVERED 已被
            // check_meridian_dependencies 拦截；这里再防 integrity = 0 的临时损伤。
            // 语义：剑道五招要求**所有**依赖经脉都通畅，任一断裂就拒绝（与 check_meridian
            // _dependencies 的 ANY 拒绝语义对齐）。
            if let Some(meridians) = world.get::<MeridianSystem>(caster) {
                if let Some(broken) = deps
                    .iter()
                    .find(|m| meridians.get(**m).integrity <= f64::EPSILON)
                {
                    return Err(CastRejectReason::MeridianSevered(Some(*broken)));
                }
            }
        }
    }

    Ok(CastContext {
        now_tick,
        definition,
    })
}

fn apply_cast_costs(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    now_tick: u64,
    definition: &TechniqueDefinition,
) {
    if let Some(mut stamina) = world.get_mut::<Stamina>(caster) {
        stamina.current =
            (stamina.current - definition.stamina_cost.max(0.0)).clamp(0.0, stamina.max);
        stamina.state = if stamina.current <= 0.0 {
            StaminaState::Exhausted
        } else {
            StaminaState::Combat
        };
        stamina.last_drain_tick = Some(now_tick);
    }
    if let Some(mut bindings) = world.get_mut::<SkillBarBindings>(caster) {
        bindings.set_cooldown(
            &definition.id,
            now_tick.saturating_add(u64::from(definition.cooldown_ticks)),
        );
    }
    insert_casting(world, caster, slot, definition, now_tick);
}

fn insert_casting(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    definition: &TechniqueDefinition,
    now_tick: u64,
) {
    let start_position = world
        .get::<Position>(caster)
        .map(|p| p.get())
        .unwrap_or(DVec3::ZERO);
    world.entity_mut(caster).insert(Casting {
        source: crate::combat::components::CastSource::SkillBar,
        slot,
        started_at_tick: now_tick,
        duration_ticks: u64::from(definition.cast_ticks),
        started_at_ms: current_unix_millis(),
        duration_ms: definition
            .cast_ticks
            .saturating_mul(crate::time::MILLIS_PER_TICK as u32),
        bound_instance_id: None,
        start_position,
        complete_cooldown_ticks: u64::from(definition.cooldown_ticks),
        skill_id: Some(definition.id.clone()),
        skill_config: None,
    });
}

fn drain_qi(world: &mut bevy_ecs::world::World, caster: Entity, cost: f64) -> bool {
    if cost <= 0.0 {
        return true;
    }
    let Some(mut cultivation) = world.get_mut::<Cultivation>(caster) else {
        return false;
    };
    if cultivation.qi_current + f64::EPSILON < cost {
        return false;
    }
    cultivation.qi_current = (cultivation.qi_current - cost).clamp(0.0, cultivation.qi_max);
    true
}

/// P4 — emit 剑道一招的 AV 呈现事件（粒子 / 音效 / 动画走 network 侧双系统）。
///
/// **纯 cosmetic**：只发 `SwordPathSkillCastEvent`，不触碰任何战斗 / 真元状态。
/// `direction` 用于剑气斩的朝向 line trail；其余招式传 `None`。
/// caster 无 `Position`（测试 / 异常态）时落到 `DVec3::ZERO`，AV 系统会再次按
/// Position 查询渲染目标，缺失即静默 skip。
fn emit_skill_av(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    skill: SwordPathSkillId,
    now_tick: u64,
    direction: Option<DVec3>,
) {
    let center = world
        .get::<Position>(caster)
        .map(|p| p.get())
        .unwrap_or(DVec3::ZERO);
    world.send_event(SwordPathSkillCastEvent {
        skill,
        caster,
        center,
        direction,
        tick: now_tick,
    });
}

/// 化虚 aftermath：`qi_current` 归零后直写 `zone.spirit_qi`（worldview §二 守恒）。
///
/// 与 `credit_skill_qi_to_zone`（ExclusiveSystem 版）逻辑一致，但接受已分解的
/// Bevy 资源引用而非 `&mut World`，故可在普通 Schedule system 内调用。
///
/// `QiTransfer` 是 audit-only（无 `EventReader`），不能代替直写，见 sword_basics.rs:433。
fn credit_qi_current_to_zone(
    caster: Entity,
    position: DVec3,
    qi_drained: f64,
    zone_registry: Option<&mut crate::world::zone::ZoneRegistry>,
    qi_transfers: Option<&mut Events<QiTransfer>>,
) {
    if qi_drained <= f64::EPSILON {
        return;
    }
    // 先读 zone 名（不可变路径），释放不可变借用后再写（可变路径）。
    let zone_name: String = match &zone_registry {
        Some(r) => r
            .find_zone(crate::world::dimension::DimensionKind::Overworld, position)
            .map(|z| z.name.clone())
            .unwrap_or_else(|| DEFAULT_SPAWN_ZONE_NAME.to_string()),
        None => DEFAULT_SPAWN_ZONE_NAME.to_string(),
    };
    // 直写 zone.spirit_qi（QiTransfer 是 audit-only，不驱动这条写路径）。
    if let Some(reg) = zone_registry {
        if let Some(zone) = reg.find_zone_mut(&zone_name) {
            zone.spirit_qi = (zone.spirit_qi
                + qi_drained / crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY)
                .clamp(-1.0, 1.0);
        }
    }
    // Audit event（供 summarize_world_qi 统计，不代替上面直写）。
    if let Some(transfers) = qi_transfers {
        if let Ok(transfer) = QiTransfer::new(
            QiAccountId::player(format!("entity:{caster:?}")),
            QiAccountId::zone(zone_name),
            qi_drained,
            QiTransferReason::ReleaseToZone,
        ) {
            transfers.send(transfer);
        }
    }
}

/// 向灵剑注入真元，返回实际注入量（bond 不存在或品阶 < 凝脉时返回 0）。
/// 调用方**必须**随后对 `(qi_cost - injected)` 调用 `credit_skill_qi_to_zone`
/// 以保证守恒律（worldview §二）。
fn inject_bond_qi(world: &mut bevy_ecs::world::World, caster: Entity, qi_cost: f64) -> f64 {
    // 灵剑必须 ≥ 凝脉品阶才有存储能力。注入按 plan §bond::QI_INJECT_RATIO = 0.1。
    let injected = match world.get_mut::<SwordBondComponent>(caster) {
        Some(mut bond) if bond.grade.can_store_qi() => bond.try_inject_qi(qi_cost),
        _ => 0.0,
    };
    if injected > f64::EPSILON {
        if let Some(mut events) = world.get_resource_mut::<Events<QiTransfer>>() {
            if let Ok(transfer) = QiTransfer::new(
                QiAccountId::player(format!("entity:{caster:?}")),
                QiAccountId::container(format!("sword_bond:{caster:?}")),
                injected,
                QiTransferReason::Channeling,
            ) {
                events.send(transfer);
            }
        }
    }
    injected
}

/// 将剑道招式消耗中未注入灵剑的余量（`cost - injected`）归还给 caster 所在 zone，
/// 并发 `QiTransfer(player → zone, remainder)` 守恒审计事件。
///
/// - 若 ZoneRegistry 不存在（测试 / 早期启动期）则静默 skip。
/// - 若 remainder ≤ ε 则无需操作。
fn credit_skill_qi_to_zone(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    qi_cost: f64,
    injected: f64,
) {
    let remainder = qi_cost - injected;
    if remainder <= f64::EPSILON {
        return;
    }
    let pos = world
        .get::<Position>(caster)
        .map(|p| p.get())
        .unwrap_or(DVec3::ZERO);
    // 找到 caster 所在 zone 名（只读），再通过名字拿可变引用。
    let zone_name: String = {
        let Some(registry) = world.get_resource::<crate::world::zone::ZoneRegistry>() else {
            return;
        };
        registry
            .find_zone(crate::world::dimension::DimensionKind::Overworld, pos)
            .map(|z| z.name.clone())
            .unwrap_or_else(|| DEFAULT_SPAWN_ZONE_NAME.to_string())
    };
    if let Some(mut registry) = world.get_resource_mut::<crate::world::zone::ZoneRegistry>() {
        if let Some(zone) = registry.find_zone_mut(&zone_name) {
            zone.spirit_qi = (zone.spirit_qi
                + remainder / crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY)
                .clamp(-1.0, 1.0);
        }
    }
    if let Some(mut events) = world.get_resource_mut::<Events<QiTransfer>>() {
        if let Ok(transfer) = QiTransfer::new(
            QiAccountId::player(format!("entity:{caster:?}")),
            QiAccountId::zone(zone_name),
            remainder,
            QiTransferReason::ReleaseToZone,
        ) {
            events.send(transfer);
        }
    }
}

#[cfg(test)]
#[path = "skill_register_tests.rs"]
mod tests;
