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
    ResMut, Username,
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
    check_player_skill_meridian_gate, MeridianSeveredPermanent, SkillMeridianDependencies,
};
use crate::cultivation::skill_registry::{CastRejectReason, CastResult, SkillRegistry};
use crate::cultivation::technique_scroll::realm_rank;
use crate::network::cast_emit::current_unix_millis;
use crate::player::state::canonical_player_id;
use crate::qi_physics::constants::{QI_EPSILON, QI_ZONE_UNIT_CAPACITY};
use crate::qi_physics::ledger::{
    pending_inflow_account, transfer_external_qi_to_ledger, QiAccountId, QiTransfer,
    QiTransferReason, WorldQiAccount,
};
use crate::qi_physics::release::qi_release_to_zone;
use crate::world::dimension::DimensionKind;
use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};
use crate::qi_physics::QiPhysicsError;

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
    if !settle_skill_qi(world, caster, qi_cost) {
        return CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient,
        };
    }
    apply_cast_costs(world, caster, slot, ctx.now_tick, &ctx.definition);

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
    if !settle_skill_qi(world, caster, qi_cost) {
        return CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient,
        };
    }
    apply_cast_costs(world, caster, slot, ctx.now_tick, &ctx.definition);

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
    if !settle_skill_qi(world, caster, qi_cost) {
        return CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient,
        };
    }
    apply_cast_costs(world, caster, slot, ctx.now_tick, &ctx.definition);

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
    if !settle_skill_qi(world, caster, qi_cost) {
        return CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient,
        };
    }
    apply_cast_costs(world, caster, slot, ctx.now_tick, &ctx.definition);

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

    // SkillMeridianDependencies 保留为 resolver-only 依赖；TechniqueDefinition 的
    // required_meridians 是 metadata 真源，统一走同一套 opened/SEVERED/integrity 门控。
    if let Some(meridians) = world.get::<MeridianSystem>(caster) {
        let severed = world.get::<MeridianSeveredPermanent>(caster);
        let deps = world.get_resource::<SkillMeridianDependencies>();
        if let Err(blocked) = check_player_skill_meridian_gate(
            skill_id,
            &definition.required_meridians,
            meridians,
            severed,
            deps,
        ) {
            return Err(CastRejectReason::MeridianSevered(Some(blocked)));
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

#[derive(Debug, Clone)]
struct QiSettlementPlan {
    ledger: WorldQiAccount,
    transfers: Vec<QiTransfer>,
    zone_name: Option<String>,
    zone_after: Option<f64>,
}

#[derive(Debug, Clone)]
struct ZoneSettlementTarget {
    name: String,
    current: f64,
}

fn zone_settlement_target(
    registry: Option<&ZoneRegistry>,
    position: DVec3,
) -> Option<ZoneSettlementTarget> {
    registry
        .and_then(|registry| registry.find_zone(DimensionKind::Overworld, position))
        .filter(|zone| zone.spirit_qi.is_finite() && zone.spirit_qi >= 0.0)
        .map(|zone| ZoneSettlementTarget {
            name: zone.name.clone(),
            current: zone.spirit_qi.clamp(0.0, 1.0) * QI_ZONE_UNIT_CAPACITY,
        })
}

fn append_external_transfer(
    ledger: &mut WorldQiAccount,
    transfers: &mut Vec<QiTransfer>,
    from: QiAccountId,
    to: QiAccountId,
    amount: f64,
    reason: QiTransferReason,
) -> Result<(), QiPhysicsError> {
    if let Some(transfer) = transfer_external_qi_to_ledger(ledger, from, to, amount, reason)? {
        transfers.push(transfer);
    }
    Ok(())
}

fn append_zone_release(
    ledger: &mut WorldQiAccount,
    transfers: &mut Vec<QiTransfer>,
    from: QiAccountId,
    amount: f64,
    target: Option<&ZoneSettlementTarget>,
    zone_current: &mut Option<f64>,
) -> Result<(), QiPhysicsError> {
    if amount == 0.0 {
        return Ok(());
    }

    let Some(target) = target else {
        return append_external_transfer(
            ledger,
            transfers,
            from,
            pending_inflow_account(),
            amount,
            QiTransferReason::ReleaseToZone,
        );
    };

    let current = zone_current
        .as_mut()
        .expect("zone settlement target must have a mutable current balance");
    let zone_account = QiAccountId::zone(target.name.clone());
    let outcome = qi_release_to_zone(
        amount,
        from.clone(),
        zone_account.clone(),
        *current,
        QI_ZONE_UNIT_CAPACITY,
    )?;
    append_external_transfer(
        ledger,
        transfers,
        from.clone(),
        zone_account,
        outcome.accepted,
        QiTransferReason::ReleaseToZone,
    )?;
    append_external_transfer(
        ledger,
        transfers,
        from,
        pending_inflow_account(),
        outcome.overflow,
        QiTransferReason::ReleaseToZone,
    )?;
    *current = outcome.zone_after;
    Ok(())
}

fn apply_qi_settlement_plan(
    ledger: &mut WorldQiAccount,
    zone_registry: Option<&mut ZoneRegistry>,
    qi_transfers: Option<&mut Events<QiTransfer>>,
    plan: QiSettlementPlan,
) {
    *ledger = plan.ledger;
    if let (Some(zone_name), Some(zone_after), Some(registry)) =
        (plan.zone_name.as_deref(), plan.zone_after, zone_registry)
    {
        if let Some(zone) = registry.find_zone_mut(zone_name) {
            zone.spirit_qi = zone_after / QI_ZONE_UNIT_CAPACITY;
        }
    }
    if let Some(events) = qi_transfers {
        for transfer in plan.transfers {
            events.send(transfer);
        }
    }
}

fn settle_skill_qi(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    qi_cost: f64,
) -> bool {
    let Some(cultivation) = world.get::<Cultivation>(caster).cloned() else {
        return false;
    };
    let Some(username) = world.get::<Username>(caster) else {
        return false;
    };
    if !qi_cost.is_finite() || qi_cost < 0.0 {
        return false;
    }
    if qi_cost == 0.0 {
        return true;
    }
    if !cultivation.qi_current.is_finite()
        || cultivation.qi_current + QI_EPSILON < qi_cost
    {
        return false;
    }

    let player_id = canonical_player_id(username.0.as_str());
    let player_account = QiAccountId::player(player_id.clone());
    let bond_snapshot = world.get::<SwordBondComponent>().cloned();
    let position = world
        .get::<Position>(caster)
        .map(Position::get)
        .unwrap_or(DVec3::ZERO);
    let target = zone_settlement_target(
        world.get_resource::<ZoneRegistry>(),
        position,
    );
    let mut zone_current = target.as_ref().map(|target| target.current);
    let mut ledger = world
        .get_resource::<WorldQiAccount>()
        .cloned()
        .unwrap_or_default();
    if let Some(target) = target.as_ref() {
        ledger
            .set_balance(
                QiAccountId::zone(target.name.clone()),
                target.current,
            )
            .is_ok()
            .then_some(())
            .ok_or(())
            .unwrap_or(());
    }

    let settlement = (|| -> Result<(QiSettlementPlan, f64), QiPhysicsError> {
        let injected = bond_snapshot
            .as_ref()
            .filter(|bond| bond.grade.can_store_qi())
            .map(|bond| {
                if !bond.stored_qi.is_finite() {
                    return Err(QiPhysicsError::InvalidAmount {
                        field: "bond.stored_qi",
                        value: bond.stored_qi,
                    });
                }
                let headroom = (bond.grade.stored_qi_cap() - bond.stored_qi).max(0.0);
                Ok((qi_cost * super::bond::QI_INJECT_RATIO).min(headroom))
            })
            .transpose()?
            .unwrap_or(0.0);
        let mut transfers = Vec::new();
        if injected > 0.0 {
            append_external_transfer(
                &mut ledger,
                &mut transfers,
                player_account.clone(),
                QiAccountId::container(format!("sword_bond:{player_id}")),
                injected,
                QiTransferReason::Channeling,
            )?;
        }
        append_zone_release(
            &mut ledger,
            &mut transfers,
            player_account.clone(),
            qi_cost - injected,
            target.as_ref(),
            &mut zone_current,
        )?;
        Ok((
            QiSettlementPlan {
                ledger: ledger.clone(),
                transfers,
                zone_name: target.as_ref().map(|target| target.name.clone()),
                zone_after: zone_current,
            },
            injected,
        ))
    })();

    let Ok((plan, injected)) = settlement else {
        return false;
    };
    let zone_name = plan.zone_name.clone();
    let zone_after = plan.zone_after;
    world.insert_resource(plan.ledger.clone());
    if let (Some(zone_name), Some(zone_after)) = (zone_name.as_deref(), zone_after) {
        if let Some(mut registry) = world.get_resource_mut::<ZoneRegistry>() {
            if let Some(zone) = registry.find_zone_mut(zone_name) {
                zone.spirit_qi = zone_after / QI_ZONE_UNIT_CAPACITY;
            }
        }
    }
    if let Some(mut current) = world.get_mut::<Cultivation>(caster) {
        current.qi_current = (current.qi_current - qi_cost).clamp(0.0, current.qi_max);
    }
    if injected > 0.0 {
        if let Some(mut bond) = world.get_mut::<SwordBondComponent>(caster) {
            bond.stored_qi += injected;
        }
    }
    if let Some(mut events) = world.get_resource_mut::<Events<QiTransfer>>() {
        for transfer in plan.transfers {
            events.send(transfer);
        }
    }
    true
}

/// P4 — emit 剑道一招的 AV 呈现事件（粒子 / 音效 / 动画走 network 侧双系统）。
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

fn prepare_heaven_gate_settlement(
    ledger: &WorldQiAccount,
    position: DVec3,
    player_id: &str,
    qi_current: f64,
    staging_buffer: f64,
    zone_registry: Option<&ZoneRegistry>,
) -> Result<QiSettlementPlan, QiPhysicsError> {
    let target = zone_settlement_target(zone_registry, position);
    let mut zone_current = target.as_ref().map(|target| target.current);
    let mut ledger = ledger.clone();
    if let Some(target) = target.as_ref() {
        ledger.set_balance(QiAccountId::zone(target.name.clone()), target.current)?;
    }
    let mut transfers = Vec::new();
    let player_account = QiAccountId::player(player_id.to_string());
    append_zone_release(
        &mut ledger,
        &mut transfers,
        player_account.clone(),
        qi_current,
        target.as_ref(),
        &mut zone_current,
    )?;
    append_zone_release(
        &mut ledger,
        &mut transfers,
        player_account,
        staging_buffer,
        target.as_ref(),
        &mut zone_current,
    )?;
    Ok(QiSettlementPlan {
        ledger,
        transfers,
        zone_name: target.as_ref().map(|target| target.name.clone()),
        zone_after: zone_current,
    })
}

fn emit_settlement_events(
    qi_transfers: Option<&mut Events<QiTransfer>>,
    transfers: Vec<QiTransfer>,
) {
    if let Some(events) = qi_transfers {
        for transfer in transfers {
            events.send(transfer);
        }
    }
}

fn settle_heaven_gate_player(
    players: &mut Query<(
        &mut crate::cultivation::components::Cultivation,
        Option<&mut SwordBondComponent>,
    )>,
    caster: Entity,
    qi_max_snapshot: f64,
    staging_stored_qi: f64,
    plan: QiSettlementPlan,
    ledger: &mut WorldQiAccount,
    zone_registry: Option<&mut ZoneRegistry>,
    qi_transfers: Option<&mut Events<QiTransfer>>,
) -> bool {
    let Ok((mut cultivation, bond_opt)) = players.get_mut(caster) else {
        return false;
    };
    cultivation.qi_max = (qi_max_snapshot * effects::HEAVEN_GATE_QI_MAX_RETAIN).max(0.0);
    cultivation.qi_current = 0.0;
    cultivation.realm = Realm::Solidify;
    if let Some(mut bond) = bond_opt {
        bond.stored_qi = 0.0;
    }
    let transfers = plan.transfers.clone();
    apply_qi_settlement_plan(ledger, zone_registry, qi_transfers, plan);
    let _ = staging_stored_qi;
    let _ = transfers;
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::weapon::EquipSlot;
    use crate::cultivation::components::Realm;
    use crate::cultivation::known_techniques::{KnownTechnique, KnownTechniques};
    use valence::prelude::{App, Update};

    fn technique_definition(app: &App, id: &str) -> TechniqueDefinition {
        app.world()
            .resource::<TechniqueRegistry>()
            .get(id)
            .unwrap_or_else(|| panic!("test fixture missing technique {id}"))
            .clone()
    }

    fn setup_app() -> (App, Entity) {
        let mut app = App::new();
        app.insert_resource(TechniqueRegistry::load_for_tests());
        app.insert_resource(CombatClock { tick: 100 });
        app.insert_resource(SkillMeridianDependencies::default());
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<HeavenGateCastEvent>();
        app.add_event::<SwordShatterEvent>();
        app.add_event::<QiTransfer>();
        app.add_event::<SwordPathSkillCastEvent>();
        app.init_resource::<TiandaoBlindZoneRegistry>();

        let mut deps = app.world_mut().resource_mut::<SkillMeridianDependencies>();
        declare_meridian_dependencies(&mut deps);

        let mut meridians = MeridianSystem::default();
        for definition in app.world().resource::<TechniqueRegistry>().iter() {
            for required in &definition.required_meridians {
                let id = crate::cultivation::technique_scroll::parse_meridian_id(&required.channel)
                    .expect("checked-in sword technique meridian must parse");
                let meridian = meridians.get_mut(id);
                meridian.opened = true;
                meridian.integrity = 1.0;
                meridian.throughput_current = 1.0;
            }
        }

        let caster = app
            .world_mut()
            .spawn((
                Position::default(),
                Weapon {
                    slot: EquipSlot::MainHand,
                    instance_id: 1,
                    template_id: "sword_iron".into(),
                    weapon_kind: WeaponKind::Sword,
                    base_attack: 10.0,
                    quality_tier: 0,
                    durability: 100.0,
                    durability_max: 100.0,
                },
                Stamina {
                    current: 100.0,
                    max: 100.0,
                    state: StaminaState::Combat,
                    last_drain_tick: None,
                    recover_per_sec: 10.0,
                },
                Cultivation {
                    realm: Realm::Void,
                    qi_current: 5000.0,
                    qi_max: 5000.0,
                    ..Cultivation::default()
                },
                meridians,
                KnownTechniques {
                    entries: vec![
                        KnownTechnique {
                            id: SWORD_PATH_CONDENSE_EDGE_ID.into(),
                            proficiency: 0.5,
                            active: true,
                        },
                        KnownTechnique {
                            id: SWORD_PATH_QI_SLASH_ID.into(),
                            proficiency: 0.5,
                            active: true,
                        },
                        KnownTechnique {
                            id: SWORD_PATH_RESONANCE_ID.into(),
                            proficiency: 0.5,
                            active: true,
                        },
                        KnownTechnique {
                            id: SWORD_PATH_MANIFEST_ID.into(),
                            proficiency: 0.5,
                            active: true,
                        },
                        KnownTechnique {
                            id: SWORD_PATH_HEAVEN_GATE_ID.into(),
                            proficiency: 1.0,
                            active: true,
                        },
                    ],
                },
            ))
            .id();
        (app, caster)
    }

    /// P1.4 — SkillRegistry 注册 5 招后可 lookup 命中。
    #[test]
    fn registry_lookup_finds_all_five_techniques() {
        let mut registry = SkillRegistry::default();
        register_skills(&mut registry);
        for id in [
            SWORD_PATH_CONDENSE_EDGE_ID,
            SWORD_PATH_QI_SLASH_ID,
            SWORD_PATH_RESONANCE_ID,
            SWORD_PATH_MANIFEST_ID,
            SWORD_PATH_HEAVEN_GATE_ID,
        ] {
            assert!(
                registry.lookup(id).is_some(),
                "招式 {id} 必须可 lookup，否则 SkillBar cast 走不通"
            );
        }
    }

    /// P1.5 — 经脉依赖按 plan §P1.5 声明落地。
    #[test]
    fn meridian_dependencies_match_plan_table() {
        let mut deps = SkillMeridianDependencies::default();
        declare_meridian_dependencies(&mut deps);
        assert_eq!(
            deps.lookup(SWORD_PATH_CONDENSE_EDGE_ID),
            &[MeridianId::LargeIntestine, MeridianId::SmallIntestine][..]
        );
        assert_eq!(
            deps.lookup(SWORD_PATH_QI_SLASH_ID),
            &[
                MeridianId::LargeIntestine,
                MeridianId::SmallIntestine,
                MeridianId::TripleEnergizer,
            ][..]
        );
        assert_eq!(deps.lookup(SWORD_PATH_HEAVEN_GATE_ID).len(), 4);
        assert!(deps
            .lookup(SWORD_PATH_HEAVEN_GATE_ID)
            .contains(&MeridianId::Du));
    }

    fn drain_av(app: &App) -> Vec<SwordPathSkillId> {
        app.world()
            .resource::<Events<SwordPathSkillCastEvent>>()
            .iter_current_update_events()
            .map(|e| e.skill)
            .collect()
    }

    /// P4 — 凝锋 cast 成功 → emit SwordPathSkillCastEvent(CondenseEdge)，无方向。
    #[test]
    fn condense_edge_emits_av_event() {
        let (mut app, caster) = setup_app();
        let target = app.world_mut().spawn(Position::default()).id();
        let result = cast_condense_edge(app.world_mut(), caster, 0, Some(target));
        assert!(matches!(result, CastResult::Started { .. }));

        let avs = drain_av(&app);
        assert_eq!(
            avs,
            vec![SwordPathSkillId::CondenseEdge],
            "凝锋应 emit 恰好一个 CondenseEdge AV 事件，实际 {avs:?}"
        );
    }

    #[test]
    fn qi_slash_runtime_consumes_overridden_metadata_fields() {
        let (mut app, caster) = setup_app();
        let configured_qi_cost = 7.25_f64;
        let configured_stamina_cost = 4.5_f32;
        let configured_cast_ticks = 17;
        let configured_cooldown_ticks = 29;
        let configured_range = 2.75_f32;
        app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
            SWORD_PATH_QI_SLASH_ID,
            |definition| {
                definition.qi_cost = configured_qi_cost;
                definition.stamina_cost = configured_stamina_cost;
                definition.cast_ticks = configured_cast_ticks;
                definition.cooldown_ticks = configured_cooldown_ticks;
                definition.range = configured_range;
            },
        ));
        app.world_mut()
            .entity_mut(caster)
            .insert(SkillBarBindings::default());
        let qi_before = app.world().get::<Cultivation>(caster).unwrap().qi_current;
        let stamina_before = app.world().get::<Stamina>(caster).unwrap().current;
        let target = app.world_mut().spawn(Position::default()).id();

        let result = cast_qi_slash(app.world_mut(), caster, 3, Some(target));

        assert_eq!(
            result,
            CastResult::Started {
                cooldown_ticks: u64::from(configured_cooldown_ticks),
                anim_duration_ticks: configured_cast_ticks,
            },
            "resolver result timing must come from the injected TechniqueRegistry"
        );
        let qi_after = app.world().get::<Cultivation>(caster).unwrap().qi_current;
        assert!(
            (qi_before - qi_after - configured_qi_cost).abs() < 1e-6,
            "qi charge must use overridden metadata: {qi_before} -> {qi_after}"
        );
        let stamina_after = app.world().get::<Stamina>(caster).unwrap().current;
        assert!(
            (stamina_before - stamina_after - configured_stamina_cost).abs() < 1e-6,
            "stamina charge must use overridden metadata: {stamina_before} -> {stamina_after}"
        );
        let attack = app
            .world()
            .resource::<Events<AttackIntent>>()
            .iter_current_update_events()
            .next()
            .expect("successful qi slash must emit AttackIntent");
        assert_eq!(
            attack.reach.base, configured_range,
            "attack reach must use overridden metadata range"
        );
        assert_eq!(
            attack.qi_invest, configured_qi_cost as f32,
            "attack qi investment must use the same overridden qi cost"
        );
        let casting = app
            .world()
            .get::<Casting>(caster)
            .expect("Casting component");
        assert_eq!(casting.duration_ticks, u64::from(configured_cast_ticks));
        assert_eq!(
            casting.complete_cooldown_ticks,
            u64::from(configured_cooldown_ticks)
        );
        assert_eq!(casting.skill_id.as_deref(), Some(SWORD_PATH_QI_SLASH_ID));
        let bindings = app.world().get::<SkillBarBindings>(caster).unwrap();
        assert!(bindings.is_on_cooldown(
            SWORD_PATH_QI_SLASH_ID,
            100 + u64::from(configured_cooldown_ticks) - 1
        ));
        assert!(!bindings.is_on_cooldown(
            SWORD_PATH_QI_SLASH_ID,
            100 + u64::from(configured_cooldown_ticks)
        ));
    }

    #[test]
    fn condense_edge_realm_gate_consumes_overridden_metadata() {
        let (mut app, caster) = setup_app();
        app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
            SWORD_PATH_CONDENSE_EDGE_ID,
            |definition| definition.required_realm = "Void".to_string(),
        ));
        app.world_mut()
            .get_mut::<Cultivation>(caster)
            .unwrap()
            .realm = Realm::Spirit;
        let qi_before = app.world().get::<Cultivation>(caster).unwrap().qi_current;

        let result = cast_condense_edge(app.world_mut(), caster, 0, None);

        assert_eq!(
            result,
            CastResult::Rejected {
                reason: CastRejectReason::RealmTooLow,
            },
            "Spirit caster would pass the removed Induce hardcode but must fail overridden Void metadata"
        );
        assert_eq!(
            app.world().get::<Cultivation>(caster).unwrap().qi_current,
            qi_before,
            "realm rejection must precede all costs"
        );
    }

    /// M13：Condense Edge resolver 必须消费 definition 的每个实际字段——cost、cast/cooldown
    /// timing、range、race、realm、meridians，而不只是 realm。override 全部字段后断言
    /// 扣费、AV 参数与结果一致。
    #[test]
    fn condense_edge_runtime_consumes_overridden_cost_timing_and_range() {
        let (mut app, caster) = setup_app();
        app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
            SWORD_PATH_CONDENSE_EDGE_ID,
            |definition| {
                definition.qi_cost = 12.5;
                definition.stamina_cost = 3.0;
                definition.cast_ticks = 7;
                definition.cooldown_ticks = 11;
                definition.range = 2.5;
            },
        ));
        let qi_before = app.world().get::<Cultivation>(caster).unwrap().qi_current;

        let result = cast_condense_edge(app.world_mut(), caster, 0, None);

        assert_eq!(
            result,
            CastResult::Started {
                cooldown_ticks: 11,
                anim_duration_ticks: 7,
            },
            "overridden cast/cooldown must drive the CastResult payload"
        );
        let charged = qi_before - app.world().get::<Cultivation>(caster).unwrap().qi_current;
        assert!(
            (charged - 12.5).abs() < 1e-6,
            "condense edge must drain the overridden qi_cost, got {charged}"
        );
        let avs = drain_av(&app);
        assert!(
            avs.contains(&SwordPathSkillId::CondenseEdge),
            "overridden metadata cast must still emit the CondenseEdge AV event"
        );
    }

    #[test]
    fn qi_slash_runtime_rejects_overridden_meridian_health_requirement() {
        let (mut app, caster) = setup_app();
        app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
            SWORD_PATH_QI_SLASH_ID,
            |definition| {
                definition.required_meridians[0].min_health = 0.9;
            },
        ));
        app.world_mut()
            .get_mut::<MeridianSystem>(caster)
            .unwrap()
            .get_mut(MeridianId::LargeIntestine)
            .integrity = 0.5;
        let qi_before = app.world().get::<Cultivation>(caster).unwrap().qi_current;

        let result = cast_qi_slash(app.world_mut(), caster, 0, None);

        assert_eq!(
            result,
            CastResult::Rejected {
                reason: CastRejectReason::MeridianSevered(Some(MeridianId::LargeIntestine)),
            },
            "runtime must enforce the overridden metadata min_health instead of only the static dependency table"
        );
        assert_eq!(
            app.world().get::<Cultivation>(caster).unwrap().qi_current,
            qi_before,
            "metadata meridian rejection must happen before qi drain"
        );
    }

    // ── F1（P3 opus verify 发现）：施放门行为测试 —— build_cast_context race gate ──
    //
    // 修复前只有 known_techniques.rs 的 `.allows()` 真值表 pin，从不触达
    // `build_cast_context` 里真实的 race gate 判定（line ~863-877）——回归删掉那段
    // `if !definition.required_race.allows(...) { return Err(RaceMismatch) }` 整块
    // 不会撞红。本测试直接调用真实 resolver 入口 `cast_condense_edge`（经
    // `build_cast_context`）锁死该行为，并与上面的 `condense_edge_emits_av_event`
    // （人形本体 happy path）对照。

    /// 构造 is_humanoid=false 的合成种族 fixture 并插入 `app`（与
    /// `combat::sword_basics` 测试同款手法：单部位 `HeightBands` 几何，
    /// 只求 `resolve_body_plan` 校验通过）。
    fn insert_non_humanoid_race_fixture(app: &mut App, race_id: &str) {
        use crate::body_plan::race_registry::RaceEntry;
        use crate::body_plan::{
            BodyPartDef, BodyPlan, BodyPlanRegistry, HeightBand, HeightBandAssignment, HitGeometry,
            PartConsequence, RaceId, RaceRegistry, StandingAabbSpec,
        };
        use std::collections::HashMap;

        let plan = BodyPlan {
            id: format!("test_{race_id}_plan").into(),
            display_name: "测试非人形构型".to_string(),
            is_humanoid: false,
            parts: vec![BodyPartDef {
                id: "core".into(),
                damage_mul: 1.0,
                contam_mul: 1.0,
                bleed_mul: 1.0,
                consequence: PartConsequence::Core,
            }],
            hit_geometry: HitGeometry::HeightBands {
                aabb: StandingAabbSpec {
                    half_width: 0.3,
                    height: 1.8,
                },
                bands: vec![HeightBand {
                    min_rel_y: -1.0,
                    assignment: HeightBandAssignment::Single {
                        part: "core".into(),
                    },
                }],
                lateral_threshold: 0.19,
            },
            equip_slots: vec![],
            meridian_profile: None,
            mutation_slot_mapping: HashMap::new(),
        };
        let plan_id = plan.id.clone();
        let body_plans = BodyPlanRegistry::from_plans(vec![plan]).expect("plan must validate");
        // `RaceRegistry::from_file_contents` 要求表内必须有一条 id=HUMAN_RACE_ID 的
        // 默认条目——复用 `combat::resolve` 同款手法：让这条 "human" 条目指向本
        // fixture 的 is_humanoid=false 构型，caster 的 `Cultivation.race` 同样设为
        // HUMAN_RACE_ID 即可解析出非人形本体。
        let races = RaceRegistry::from_parts_for_test(
            vec![RaceEntry {
                id: RaceId::new(crate::body_plan::HUMAN_RACE_ID),
                display_name: format!("测试非人形种族({race_id})"),
                body_plan_id: plan_id,
                beast_kinds: vec![],
            }],
            vec![],
            &body_plans,
        )
        .expect("races fixture must validate");
        app.insert_resource(body_plans);
        app.insert_resource(races);
    }

    #[test]
    fn cast_condense_edge_rejects_race_mismatch_for_non_humanoid_caster_qi_untouched() {
        // sword_path.condense_edge（剑道凝锋）全数据表标 RaceGate::Humanoid（§8.1
        // #6，依赖人体专属经脉拓扑 + 双臂持械机能）。非人形本体（race="test_whale"
        // + BodyPlan.is_humanoid=false）必须在 `build_cast_context` 的 race gate
        // 被拒绝——早于境界门/经脉依赖门，且 `inject_bond_qi`/`credit_skill_qi_to_zone`
        // 从未执行，qi_current 分毫不动（真元守恒律）。
        let (mut app, caster) = setup_app();
        insert_non_humanoid_race_fixture(&mut app, "test_whale");
        app.world_mut().entity_mut(caster).insert(Cultivation {
            race: crate::body_plan::RaceId::new(crate::body_plan::HUMAN_RACE_ID),
            realm: Realm::Void,
            qi_current: 5000.0,
            qi_max: 5000.0,
            ..Cultivation::default()
        });
        let target = app.world_mut().spawn(Position::default()).id();

        let result = cast_condense_edge(app.world_mut(), caster, 0, Some(target));

        assert_eq!(
            result,
            CastResult::Rejected {
                reason: CastRejectReason::RaceMismatch
            },
            "非人形本体施放 sword_path.condense_edge 必须拒绝 RaceMismatch（realm=Void/\
             weapon/proficiency 均已满足，唯独 race gate 应该是拒因），实际 {result:?}"
        );
        let avs = drain_av(&app);
        assert!(
            avs.is_empty(),
            "race gate 拒绝必须发生在 AV 事件 emit 之前；期望零事件，实际 {avs:?}"
        );
        let qi_current = app.world().get::<Cultivation>(caster).unwrap().qi_current;
        assert!(
            (qi_current - 5000.0).abs() < f64::EPSILON,
            "race gate 拒绝不应扣真元（守恒律）；期望 5000.0 不变，实际 {qi_current}"
        );
    }

    /// P4 — 剑气斩 cast 成功 → emit AV(QiSlash) 且带 caster→target 方向。
    #[test]
    fn qi_slash_emits_av_event_with_direction() {
        let (mut app, caster) = setup_app();
        // caster 在原点；target 在 +Z 5 格
        let target = app.world_mut().spawn(Position::new([0.0, 0.0, 5.0])).id();
        let result = cast_qi_slash(app.world_mut(), caster, 0, Some(target));
        assert!(matches!(result, CastResult::Started { .. }));

        let events: Vec<_> = app
            .world()
            .resource::<Events<SwordPathSkillCastEvent>>()
            .iter_current_update_events()
            .cloned()
            .collect();
        assert_eq!(events.len(), 1, "剑气斩应 emit 恰好一个 AV 事件");
        assert_eq!(events[0].skill, SwordPathSkillId::QiSlash);
        let dir = events[0].direction.expect("剑气斩 AV 必须带方向");
        assert!(
            (dir - DVec3::new(0.0, 0.0, 1.0)).length() < 1e-6,
            "方向应为 caster→target 单位向量 (+Z)，实际 {dir:?}"
        );
    }

    /// P4 — 剑气斩 caster/target 重合时方向退化为 +X（不 panic / 不 NaN）。
    #[test]
    fn qi_slash_av_direction_degenerates_to_x_when_coincident() {
        let (mut app, caster) = setup_app();
        // target 与 caster 同位置（都在原点）
        let target = app.world_mut().spawn(Position::default()).id();
        let result = cast_qi_slash(app.world_mut(), caster, 0, Some(target));
        assert!(matches!(result, CastResult::Started { .. }));

        let dir = app
            .world()
            .resource::<Events<SwordPathSkillCastEvent>>()
            .iter_current_update_events()
            .next()
            .expect("AV event")
            .direction
            .expect("方向存在");
        assert!(
            (dir - DVec3::new(1.0, 0.0, 0.0)).length() < 1e-6,
            "caster/target 重合时方向应退化为 +X，实际 {dir:?}"
        );
    }

    /// P4 — 剑鸣 cast 成功 → emit AV(Resonance)。
    #[test]
    fn resonance_emits_av_event() {
        let (mut app, caster) = setup_app();
        let result = cast_resonance(app.world_mut(), caster, 0, None);
        assert!(matches!(result, CastResult::Started { .. }));
        assert_eq!(drain_av(&app), vec![SwordPathSkillId::Resonance]);
    }

    /// P4 — 化形 cast 成功 → emit AV(Manifest)。
    #[test]
    fn manifest_emits_av_event() {
        let (mut app, caster) = setup_app();
        let target = app.world_mut().spawn(Position::default()).id();
        let result = cast_manifest(app.world_mut(), caster, 0, Some(target));
        assert!(matches!(result, CastResult::Started { .. }));
        assert_eq!(drain_av(&app), vec![SwordPathSkillId::Manifest]);
    }

    /// M13：Resonance 此前完全没有 override 驱动测试。override cost/timing/range 后
    /// 断言扣费、CastResult 与 AoE 半径（范围内目标命中/范围外目标不受影响）都来自
    /// injected definition。
    #[test]
    fn resonance_runtime_consumes_overridden_cost_timing_and_range() {
        let (mut app, caster) = setup_app();
        app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
            SWORD_PATH_RESONANCE_ID,
            |definition| {
                definition.qi_cost = 9.0;
                definition.cast_ticks = 4;
                definition.cooldown_ticks = 13;
                definition.range = 2.0;
            },
        ));
        let qi_before = app.world().get::<Cultivation>(caster).unwrap().qi_current;
        // caster 在原点；范围内放一个 StatusEffects 实体（命中），范围外放一个（不命中）。
        let in_range = app
            .world_mut()
            .spawn((Position::new([1.0, 0.0, 0.0]), StatusEffects::default()))
            .id();
        let out_range = app
            .world_mut()
            .spawn((Position::new([3.0, 0.0, 0.0]), StatusEffects::default()))
            .id();

        let result = cast_resonance(app.world_mut(), caster, 0, None);

        assert_eq!(
            result,
            CastResult::Started {
                cooldown_ticks: 13,
                anim_duration_ticks: 4,
            },
            "overridden cast/cooldown must drive the CastResult payload"
        );
        let charged = qi_before - app.world().get::<Cultivation>(caster).unwrap().qi_current;
        assert!(
            (charged - 9.0).abs() < 1e-6,
            "resonance must drain the overridden qi_cost, got {charged}"
        );
        let intents: Vec<_> = app
            .world()
            .resource::<Events<ApplyStatusEffectIntent>>()
            .iter_current_update_events()
            .cloned()
            .collect();
        assert!(
            intents.iter().any(|i| i.target == in_range),
            "entity within overridden range must be slowed, intents={intents:?}"
        );
        assert!(
            !intents.iter().any(|i| i.target == out_range),
            "entity outside overridden range must NOT be slowed, intents={intents:?}"
        );
    }

    /// M13：Manifest 也没有 override 驱动测试。override cost/timing/cooldown 后断言
    /// 扣费与 CastResult 都来自 injected definition。
    #[test]
    fn manifest_runtime_consumes_overridden_cost_timing_and_cooldown() {
        let (mut app, caster) = setup_app();
        app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
            SWORD_PATH_MANIFEST_ID,
            |definition| {
                definition.qi_cost = 15.0;
                definition.stamina_cost = 2.0;
                definition.cast_ticks = 6;
                definition.cooldown_ticks = 17;
            },
        ));
        let qi_before = app.world().get::<Cultivation>(caster).unwrap().qi_current;
        let target = app.world_mut().spawn(Position::default()).id();

        let result = cast_manifest(app.world_mut(), caster, 0, Some(target));

        assert_eq!(
            result,
            CastResult::Started {
                cooldown_ticks: 17,
                anim_duration_ticks: 6,
            },
            "overridden cast/cooldown must drive the CastResult payload"
        );
        let charged = qi_before - app.world().get::<Cultivation>(caster).unwrap().qi_current;
        assert!(
            (charged - 15.0).abs() < 1e-6,
            "manifest must drain the overridden qi_cost, got {charged}"
        );
        assert_eq!(drain_av(&app), vec![SwordPathSkillId::Manifest]);
    }

    /// P4 / §D — 天门 cast → emit charge AV；phase_system elapsed≥140 → emit release AV。
    #[test]
    fn heaven_gate_emits_charge_then_release_av() {
        let (mut app, caster) = setup_app();
        // heaven_gate_phase_system 需要 VfxEventRequest 注册。
        app.add_event::<crate::network::vfx_event_emit::VfxEventRequest>();
        app.add_systems(Update, heaven_gate_phase_system);

        // cast 阶段：charge AV（cast_heaven_gate 直接 emit，不走 system）
        let result = cast_heaven_gate(app.world_mut(), caster, 0, None);
        assert!(matches!(result, CastResult::Started { .. }));
        assert_eq!(
            drain_av(&app),
            vec![SwordPathSkillId::HeavenGateCharge],
            "cast 阶段应 emit charge AV"
        );

        // 推进 CombatClock 到 start_tick(100) + AOE_END(140) = 240 → aftermath 触发。
        app.world_mut().resource_mut::<CombatClock>().tick = 240;

        // 结算阶段：heaven_gate_phase_system elapsed=140 ≥ HEAVEN_GATE_AOE_END → release AV。
        app.update();
        let release_avs: Vec<_> = app
            .world()
            .resource::<Events<SwordPathSkillCastEvent>>()
            .iter_current_update_events()
            .map(|e| e.skill)
            .filter(|s| *s == SwordPathSkillId::HeavenGateRelease)
            .collect();
        assert_eq!(
            release_avs,
            vec![SwordPathSkillId::HeavenGateRelease],
            "phase_system elapsed=140 应 emit 恰好一个 release AV"
        );
    }

    /// 缺武器 → NoWeapon（专用原因）而非 InvalidTarget——剑道招式全是方向技/
    /// 自身 AoE 从不读锁定目标，"目标无效"是误导文案（一剑开天门贴脸锁妖兽
    /// 连刷"目标无效"实证；对齐 sword_basics 既有做法）。
    #[test]
    fn bare_handed_cast_rejects_no_weapon_not_invalid_target() {
        let (mut app, caster) = setup_app();
        app.world_mut().entity_mut(caster).remove::<Weapon>();
        for cast in [
            cast_condense_edge,
            cast_qi_slash,
            cast_resonance,
            cast_manifest,
            cast_heaven_gate,
        ] {
            let result = cast(app.world_mut(), caster, 0, None);
            assert_eq!(
                result,
                CastResult::Rejected {
                    reason: CastRejectReason::NoWeapon
                },
                "空手施放剑道招式应报 NoWeapon（客户端显示「缺少武器」），\
                 不应再冒用 InvalidTarget 误导玩家去找目标"
            );
        }
    }

    /// 武器不是剑（weapon_kind 不合规）同样报 NoWeapon。
    #[test]
    fn non_sword_weapon_rejects_no_weapon() {
        let (mut app, caster) = setup_app();
        if let Some(mut weapon) = app.world_mut().get_mut::<Weapon>(caster) {
            weapon.weapon_kind = WeaponKind::Saber;
        }
        let result = cast_heaven_gate(app.world_mut(), caster, 0, None);
        assert_eq!(
            result,
            CastResult::Rejected {
                reason: CastRejectReason::NoWeapon
            },
            "持非剑武器施放天门应报 NoWeapon"
        );
    }

    /// 招式未激活 / 未习得 → TechniqueInactive（客户端「招式未激活」）——
    /// 同为此前冒用 InvalidTarget 的第二类拒绝原因。
    #[test]
    fn inactive_technique_rejects_technique_inactive() {
        let (mut app, caster) = setup_app();
        if let Some(mut known) = app.world_mut().get_mut::<KnownTechniques>(caster) {
            for entry in known.entries.iter_mut() {
                if entry.id == SWORD_PATH_HEAVEN_GATE_ID {
                    entry.active = false;
                }
            }
        }
        assert_eq!(
            cast_heaven_gate(app.world_mut(), caster, 0, None),
            CastResult::Rejected {
                reason: CastRejectReason::TechniqueInactive
            },
            "active=false 应报 TechniqueInactive 而非「目标无效」"
        );

        // 完全没有 KnownTechniques 组件（未习得任何招式）同理。
        app.world_mut()
            .entity_mut(caster)
            .remove::<KnownTechniques>();
        assert_eq!(
            cast_heaven_gate(app.world_mut(), caster, 0, None),
            CastResult::Rejected {
                reason: CastRejectReason::TechniqueInactive
            },
            "无 KnownTechniques 组件应报 TechniqueInactive"
        );
    }

    /// P4 — cast 被拒绝（真元不足）时**不**应 emit AV 事件（纯加法不污染拒绝路径）。
    #[test]
    fn rejected_cast_does_not_emit_av_event() {
        let (mut app, caster) = setup_app();
        if let Some(mut c) = app.world_mut().get_mut::<Cultivation>(caster) {
            c.qi_current = 1.0; // < checked-in sword_path.qi_slash qi_cost
        }
        let target = app.world_mut().spawn(Position::default()).id();
        let result = cast_qi_slash(app.world_mut(), caster, 0, Some(target));
        assert!(matches!(result, CastResult::Rejected { .. }));
        assert!(drain_av(&app).is_empty(), "被拒绝的 cast 不应 emit AV 事件");
    }

    /// Option B — 凝锋/剑气斩/化形去掉"目标无效"门禁：无目标照常挥出（Started），
    /// AttackIntent.target==None（resolver 跳过即空挥/空斩，不命中、无误伤）。
    #[test]
    fn condense_edge_without_target_air_swings_not_rejected() {
        let (mut app, caster) = setup_app();
        let result = cast_condense_edge(app.world_mut(), caster, 0, None);
        assert!(
            matches!(result, CastResult::Started { .. }),
            "无目标凝锋应空挥 Started（不再 InvalidTarget），实际 {result:?}"
        );
        let intent = app
            .world()
            .resource::<Events<AttackIntent>>()
            .iter_current_update_events()
            .next()
            .expect("空挥仍应发 AttackIntent");
        assert_eq!(
            intent.target, None,
            "空挥 AttackIntent.target 必须 None（不命中、无误伤）"
        );
    }

    #[test]
    fn qi_slash_without_target_air_swings_with_facing_direction() {
        let (mut app, caster) = setup_app();
        let result = cast_qi_slash(app.world_mut(), caster, 0, None);
        assert!(
            matches!(result, CastResult::Started { .. }),
            "无目标剑气斩应空斩 Started，实际 {result:?}"
        );
        let intent = app
            .world()
            .resource::<Events<AttackIntent>>()
            .iter_current_update_events()
            .next()
            .expect("空斩仍应发 AttackIntent");
        assert_eq!(intent.target, None, "空斩 AttackIntent.target 必须 None");
        // 朝向：无目标 + 无 Look → 退化 +X（有限、非 NaN）。
        let dir = app
            .world()
            .resource::<Events<SwordPathSkillCastEvent>>()
            .iter_current_update_events()
            .next()
            .expect("空斩应发 AV")
            .direction
            .expect("剑气斩 AV 必带方向");
        assert!(dir.is_finite(), "空斩朝向必须有限不 NaN，实际 {dir:?}");
        assert!(
            (dir - DVec3::new(1.0, 0.0, 0.0)).length() < 1e-6,
            "无目标无 Look 时朝向退化为 +X，实际 {dir:?}"
        );
    }

    #[test]
    fn manifest_without_target_air_swings_not_rejected() {
        use super::super::sword_intent_entity::SwordIntentEntity;
        let (mut app, caster) = setup_app();
        let result = cast_manifest(app.world_mut(), caster, 0, None);
        assert!(
            matches!(result, CastResult::Started { .. }),
            "无目标化形应空放 Started，实际 {result:?}"
        );
        // §C 改为 spawn SwordIntentEntity，不再直发 AttackIntent。
        // 无目标时应 spawn target==None 的追踪实体（空放）。
        assert_eq!(
            app.world()
                .resource::<Events<AttackIntent>>()
                .iter_current_update_events()
                .count(),
            0,
            "化形 §C 不再直发 AttackIntent"
        );
        let mut q = app.world_mut().query::<&SwordIntentEntity>();
        let intents: Vec<_> = q.iter(app.world()).collect();
        assert_eq!(
            intents.len(),
            1,
            "无目标空放应 spawn 1 个 SwordIntentEntity，实际 {}",
            intents.len()
        );
        assert_eq!(
            intents[0].target, None,
            "空放 SwordIntentEntity.target 必须 None"
        );
    }

    /// P1.6 — 凝锋发 AttackIntent 走 SwordPathCondenseEdge source。
    #[test]
    fn condense_edge_emits_attack_intent_with_correct_source() {
        let (mut app, caster) = setup_app();
        let target = app.world_mut().spawn(Position::default()).id();

        let result = cast_condense_edge(app.world_mut(), caster, 0, Some(target));
        assert!(matches!(result, CastResult::Started { .. }));

        let intents = app.world().resource::<Events<AttackIntent>>();
        let intent = intents
            .iter_current_update_events()
            .next()
            .expect("至少一条 AttackIntent");
        assert_eq!(intent.source, AttackSource::SwordPathCondenseEdge);
        assert_eq!(intent.target, Some(target));
    }

    /// P1.6 — 剑气斩耗真元 + 走 SwordPathQiSlash source。
    #[test]
    fn qi_slash_drains_qi_and_emits_attack_intent() {
        let (mut app, caster) = setup_app();
        let qi_slash = technique_definition(&app, SWORD_PATH_QI_SLASH_ID);
        let target = app.world_mut().spawn(Position::default()).id();
        let qi_before = app.world().get::<Cultivation>(caster).unwrap().qi_current;

        let result = cast_qi_slash(app.world_mut(), caster, 0, Some(target));
        assert!(matches!(result, CastResult::Started { .. }));

        let qi_after = app.world().get::<Cultivation>(caster).unwrap().qi_current;
        assert!(
            (qi_before - qi_after - qi_slash.qi_cost).abs() < 1e-6,
            "qi_current 应扣 {} (qi_slash.qi_cost)，实际差值 {}",
            qi_slash.qi_cost,
            qi_before - qi_after
        );
        let intent = app
            .world()
            .resource::<Events<AttackIntent>>()
            .iter_current_update_events()
            .next()
            .expect("剑气斩应发 AttackIntent");
        assert_eq!(intent.source, AttackSource::SwordPathQiSlash);
    }

    /// P1.6 — 真元不足时剑气斩拒绝 cast，不扣真元、不发 intent。
    #[test]
    fn qi_slash_rejects_when_qi_insufficient() {
        let (mut app, caster) = setup_app();
        // 真元降到 1 < checked-in qi_cost (3.0)
        if let Some(mut c) = app.world_mut().get_mut::<Cultivation>(caster) {
            c.qi_current = 1.0;
        }
        let target = app.world_mut().spawn(Position::default()).id();
        let result = cast_qi_slash(app.world_mut(), caster, 0, Some(target));
        assert!(matches!(
            result,
            CastResult::Rejected {
                reason: CastRejectReason::QiInsufficient
            }
        ));
        assert_eq!(
            app.world().get::<Cultivation>(caster).unwrap().qi_current,
            1.0,
            "拒绝 cast 不应扣真元"
        );
        assert_eq!(
            app.world()
                .resource::<Events<AttackIntent>>()
                .iter_current_update_events()
                .count(),
            0,
            "拒绝 cast 不应发 AttackIntent"
        );
    }

    /// Review 第 2 轮 #3 fix — 体力前置校验：cast_qi_slash 体力不足时**先**拒绝，
    /// **不**进入扣真元和写冷却的副作用。旧实现允许只剩 1 点体力的玩家先消耗真元
    /// 再因 stamina clamp 到 0 失败，留下脏状态。
    #[test]
    fn cast_rejected_when_stamina_insufficient_before_qi_drain() {
        let (mut app, caster) = setup_app();
        // checked-in sword_path.qi_slash stamina_cost = 12.0；把体力降到 5.0 < 12.0
        if let Some(mut stamina) = app.world_mut().get_mut::<Stamina>(caster) {
            stamina.current = 5.0;
        }
        let qi_before = app.world().get::<Cultivation>(caster).unwrap().qi_current;
        let target = app.world_mut().spawn(Position::default()).id();

        let result = cast_qi_slash(app.world_mut(), caster, 0, Some(target));
        assert!(
            matches!(
                result,
                CastResult::Rejected {
                    reason: CastRejectReason::InRecovery
                }
            ),
            "体力 5 < 12 应触发 InRecovery 拒绝，实际 result={result:?}"
        );
        // 守 review 关键：真元、冷却、AttackIntent **不**应被写动
        let qi_after = app.world().get::<Cultivation>(caster).unwrap().qi_current;
        assert!(
            (qi_before - qi_after).abs() < f64::EPSILON,
            "拒绝 cast 不应扣真元（旧实现会先 drain 再失败留脏状态），\
             实际 qi {qi_before} → {qi_after}"
        );
        assert_eq!(
            app.world()
                .resource::<Events<AttackIntent>>()
                .iter_current_update_events()
                .count(),
            0,
            "拒绝 cast 不应发 AttackIntent"
        );
    }

    /// P1.5 — 经脉 SEVERED → cast 拒绝 with MeridianSevered。
    #[test]
    fn cast_rejected_when_dependency_meridian_severed() {
        let (mut app, caster) = setup_app();
        let mut severed = MeridianSeveredPermanent::default();
        severed.insert(
            MeridianId::TripleEnergizer,
            crate::cultivation::meridian::severed::SeveredSource::CombatWound,
            10,
        );
        app.world_mut().entity_mut(caster).insert(severed);
        let target = app.world_mut().spawn(Position::default()).id();

        let result = cast_qi_slash(app.world_mut(), caster, 0, Some(target));
        assert!(
            matches!(
                result,
                CastResult::Rejected {
                    reason: CastRejectReason::MeridianSevered(Some(MeridianId::TripleEnergizer))
                }
            ),
            "三焦 SEVERED 时剑气斩必须被 check_meridian_dependencies 拦截，实际 result={result:?}"
        );
    }

    /// P1.5 review 修复 — 任一依赖经脉 integrity ≤ ε 即拒绝，不要求"全部依赖断"。
    /// 原 `all()` 实现会放过部分损伤状态，违反 worldview §四:286。
    #[test]
    fn cast_rejected_when_any_dependency_meridian_integrity_zero() {
        let (mut app, caster) = setup_app();
        // 只损坏一条依赖经脉（小肠 SmallIntestine），保留其他两条
        // (LargeIntestine + TripleEnergizer)
        if let Some(mut meridians) = app.world_mut().get_mut::<MeridianSystem>(caster) {
            meridians.get_mut(MeridianId::SmallIntestine).integrity = 0.0;
            // 显式保证其余两条仍 > ε
            meridians.get_mut(MeridianId::LargeIntestine).integrity = 1.0;
            meridians.get_mut(MeridianId::TripleEnergizer).integrity = 1.0;
        }
        let target = app.world_mut().spawn(Position::default()).id();

        let result = cast_qi_slash(app.world_mut(), caster, 0, Some(target));
        assert!(
            matches!(
                result,
                CastResult::Rejected {
                    reason: CastRejectReason::MeridianSevered(Some(MeridianId::SmallIntestine))
                }
            ),
            "小肠 integrity=0 时剑气斩必须立即拒绝（实际 result={result:?}）—— 旧 all() \
            实现要求全部依赖都坏才拒绝，会破 worldview §四:286 物理可见性"
        );
    }

    /// P1.6 — 剑鸣对范围内目标发 Slowed 状态效果（plan §techniques::effects::RESONANCE_SLOW）。
    #[test]
    fn resonance_applies_slowed_to_targets_in_radius() {
        let (mut app, caster) = setup_app();
        // 在范围内放 2 个目标，范围外放 1 个
        let near_a = app
            .world_mut()
            .spawn((Position::default(), StatusEffects::default()))
            .id();
        let near_b = app
            .world_mut()
            .spawn((Position::default(), StatusEffects::default()))
            .id();
        let far_target = app
            .world_mut()
            .spawn((Position::new([100.0, 0.0, 0.0]), StatusEffects::default()))
            .id();

        let result = cast_resonance(app.world_mut(), caster, 0, None);
        assert!(matches!(result, CastResult::Started { .. }));

        let applied: Vec<Entity> = app
            .world()
            .resource::<Events<ApplyStatusEffectIntent>>()
            .iter_current_update_events()
            .filter(|e| matches!(e.kind, StatusEffectKind::Slowed))
            .map(|e| e.target)
            .collect();
        assert!(applied.contains(&near_a), "范围内目标 a 应被 Slowed");
        assert!(applied.contains(&near_b), "范围内目标 b 应被 Slowed");
        assert!(
            !applied.contains(&far_target),
            "范围外目标不应被 Slowed (远在 100 格外)"
        );
    }

    /// P1.6 / §C — 剑意化形 spawn SwordIntentEntity + 扣 bond_strength 0.1。
    /// （计划 sword-path-complete §C：化形改为 spawn 追踪实体，不再直发 AttackIntent）
    #[test]
    fn manifest_spawns_sword_intent_entity_and_dings_bond_strength() {
        use super::super::sword_intent_entity::SwordIntentEntity;
        let (mut app, caster) = setup_app();
        // 给 caster 挂一个已绑定 bond，stored_qi 与 bond_strength 都 > 0
        app.world_mut()
            .entity_mut(caster)
            .insert(SwordBondComponent {
                bonded_weapon_entity: Entity::from_raw(1),
                bond_strength: 0.8,
                stored_qi: 50.0,
                grade: SwordGrade::Spirit,
            });
        let target = app.world_mut().spawn(Position::default()).id();

        let result = cast_manifest(app.world_mut(), caster, 0, Some(target));
        assert!(
            matches!(result, CastResult::Started { .. }),
            "化形 cast 应返回 Started（带 bond + 目标），实际 {result:?}"
        );

        // bond_strength 应从 0.8 扣 0.1 → 0.7
        let bond = app.world().get::<SwordBondComponent>(caster).unwrap();
        assert!(
            (bond.bond_strength - 0.7).abs() < 1e-5,
            "bond_strength 应从 0.8 → 0.7 (扣 0.1)，实际 {}",
            bond.bond_strength
        );

        // 不应直发 AttackIntent（改为 SwordIntentEntity 追踪）
        let intent_count = app
            .world()
            .resource::<Events<AttackIntent>>()
            .iter_current_update_events()
            .count();
        assert_eq!(
            intent_count, 0,
            "化形 §C 改为 spawn 追踪实体，不再直发 AttackIntent（count={intent_count}）"
        );

        // 应 spawn 出一个 SwordIntentEntity，source = SwordPathManifest，target = 目标
        let mut sword_intent_q = app.world_mut().query::<&SwordIntentEntity>();
        let intents: Vec<_> = sword_intent_q.iter(app.world()).collect();
        assert_eq!(
            intents.len(),
            1,
            "化形 §C 应 spawn 恰好 1 个 SwordIntentEntity，实际 {}",
            intents.len()
        );
        assert_eq!(
            intents[0].source,
            AttackSource::SwordPathManifest,
            "SwordIntentEntity.source 应为 SwordPathManifest"
        );
        assert_eq!(
            intents[0].target,
            Some(target),
            "SwordIntentEntity.target 应为施放目标"
        );
    }

    /// P2.1 / §D — 化虚一击 → HeavenGateChanneling + phase_system elapsed≥140 → 结算：
    /// 境界跌至固元，qi_max 衰减 90%，qi_current = 0，盲区注册。
    ///
    /// review 修复（双重结算）：化虚是单向门，**不**走通用 SwordShatterEvent
    /// 路径——否则 sword_shatter_system 会再扣一次 qi_max + 再发一笔 ledger。
    #[test]
    fn heaven_gate_cast_system_full_aftermath() {
        use crate::cultivation::components::Realm;
        let (mut app, caster) = setup_app();
        // 化虚需先有 bond 才能算 stored_qi；这里挂上 Void 灵剑 + 100 stored_qi
        app.world_mut()
            .entity_mut(caster)
            .insert(SwordBondComponent {
                bonded_weapon_entity: Entity::from_raw(7),
                bond_strength: 1.0,
                stored_qi: 100.0,
                grade: SwordGrade::Void,
            });
        // 范围内随便放个目标
        let _target = app.world_mut().spawn(Position::new([10.0, 0.0, 0.0])).id();

        // §D：phase_system 替代旧 heaven_gate_cast_system。
        app.add_event::<crate::network::vfx_event_emit::VfxEventRequest>();
        app.add_systems(Update, heaven_gate_phase_system);

        // cast 阶段：插入 HeavenGateChanneling（start_tick = 100 来自 CombatClock）。
        let cast_result = cast_heaven_gate(app.world_mut(), caster, 0, None);
        assert!(matches!(cast_result, CastResult::Started { .. }));

        // 推进时钟到 100 + 140 = 240（elapsed=140 ≥ HEAVEN_GATE_AOE_END）。
        app.world_mut().resource_mut::<CombatClock>().tick = 240;
        app.update();

        let cultivation = app.world().get::<Cultivation>(caster).unwrap();
        assert_eq!(cultivation.realm, Realm::Solidify, "化虚跌至固元");
        assert_eq!(cultivation.qi_current, 0.0, "qi_current 归零");
        assert!(
            (cultivation.qi_max - 500.0).abs() < 1e-6,
            "qi_max 5000 → 500（保留 10%），实际 {}",
            cultivation.qi_max
        );

        let bond = app.world().get::<SwordBondComponent>(caster).unwrap();
        assert_eq!(bond.stored_qi, 0.0, "stored_qi 清零（剑碎）");

        let shatter_events: Vec<_> = app
            .world()
            .resource::<Events<SwordShatterEvent>>()
            .iter_current_update_events()
            .collect();
        assert!(
            shatter_events.is_empty(),
            "化虚单向门**不**走通用 SwordShatterEvent，否则会和 inline aftermath \
             重复扣 qi_max 与重复写 ledger（review 第 2 轮 #1 critical bug）；\
             实际事件数 {}",
            shatter_events.len()
        );

        let registry = app.world().resource::<TiandaoBlindZoneRegistry>();
        assert_eq!(
            registry.active_count(),
            1,
            "化虚一击必须注册一个天道盲区，agent 才会屏蔽 caster"
        );

        // QiTransfer 守恒：化虚应有 2 笔 ReleaseToZone：
        //   1. qi_current 归零补归 zone（守恒修复 #qi-sweep-heaven-gate-drain）
        //   2. staging_buffer = qi_max_snapshot(5000) + stored_qi(100) = 5100（bond 账目 audit）
        // 两笔独立，不是双重结算——qi_current 与 staging_buffer 是不同来源。
        let release_events: Vec<_> = app
            .world()
            .resource::<Events<QiTransfer>>()
            .iter_current_update_events()
            .filter(|t| matches!(t.reason, QiTransferReason::ReleaseToZone))
            .collect();
        assert_eq!(
            release_events.len(),
            2,
            "化虚一击应有恰好 2 笔 ReleaseToZone（qi_current + staging_buffer），实际 {} 笔",
            release_events.len()
        );
        let mut amounts: Vec<f64> = release_events.iter().map(|t| t.amount).collect();
        amounts.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert!(
            (amounts[0] - 5000.0).abs() < 1e-6,
            "第 1 笔应为 qi_current(5000.0)，实际 {}",
            amounts[0]
        );
        assert!(
            (amounts[1] - 5100.0).abs() < 1e-6,
            "第 2 笔应为 staging_buffer(5100.0)，实际 {}",
            amounts[1]
        );
    }

    // ─── QP-002 守恒修复测试 ──────────────────────────────────────────────────────

    /// QP-002 happy path — 无灵剑时 QI_SLASH 全部消耗应归还 zone（100% = qi_cost）。
    /// 此前 drain_qi 把 qi 从 Cultivation 扣掉、没有任何 zone credit，守恒漏洞。
    #[test]
    fn qi_slash_without_bond_credits_full_cost_to_zone() {
        let (mut app, caster) = setup_app();
        let qi_slash = technique_definition(&app, SWORD_PATH_QI_SLASH_ID);
        // 插入含 spawn zone（spirit_qi=0.9）的 ZoneRegistry。
        let zone_before = 0.9_f64;
        let mut registry = crate::world::zone::ZoneRegistry::fallback();
        registry.find_zone_mut("spawn").unwrap().spirit_qi = zone_before;
        app.world_mut().insert_resource(registry);

        let qi_before = app.world().get::<Cultivation>(caster).unwrap().qi_current;
        let result = cast_qi_slash(app.world_mut(), caster, 0, None);
        assert!(matches!(result, CastResult::Started { .. }));

        // qi_current 应减少 qi_slash.qi_cost。
        let qi_after = app.world().get::<Cultivation>(caster).unwrap().qi_current;
        assert!(
            (qi_before - qi_after - qi_slash.qi_cost).abs() < 1e-9,
            "qi_current 应扣 {} (qi_slash.qi_cost)，实际差 {}",
            qi_slash.qi_cost,
            qi_before - qi_after,
        );

        // 无 bond → 全部 cost 归还 zone：ReleaseToZone event amount = qi_slash.qi_cost。
        let release: Vec<_> = app
            .world()
            .resource::<Events<QiTransfer>>()
            .iter_current_update_events()
            .filter(|t| matches!(t.reason, QiTransferReason::ReleaseToZone))
            .collect();
        assert_eq!(
            release.len(),
            1,
            "无 bond 剑气斩应有恰好 1 笔 ReleaseToZone，实际 {} 笔",
            release.len()
        );
        assert!(
            (release[0].amount - qi_slash.qi_cost).abs() < 1e-9,
            "ReleaseToZone.amount 应等于 qi_slash.qi_cost={:.1}（全额归 zone），实际 {}",
            qi_slash.qi_cost,
            release[0].amount,
        );

        // zone.spirit_qi 应随之增加（qi 守恒）。
        let zone_after = app
            .world_mut()
            .resource_mut::<crate::world::zone::ZoneRegistry>()
            .find_zone_mut("spawn")
            .map(|z| z.spirit_qi)
            .unwrap_or(0.0);
        assert!(
            zone_after > zone_before,
            "zone.spirit_qi 应在剑气斩后上升（守恒），实际 {zone_before} → {zone_after}"
        );
    }

    /// QP-002 — 有凝脉灵剑时 10% 注入 bond、90% 归还 zone（守恒不漏）。
    #[test]
    fn qi_slash_with_condensed_bond_credits_remainder_to_zone() {
        let (mut app, caster) = setup_app();
        let qi_slash = technique_definition(&app, SWORD_PATH_QI_SLASH_ID);
        let zone_before = 0.5_f64;
        let mut registry = crate::world::zone::ZoneRegistry::fallback();
        registry.find_zone_mut("spawn").unwrap().spirit_qi = zone_before;
        app.world_mut().insert_resource(registry);

        // 给 caster 挂凝脉灵剑（stored_qi cap 远大于注入量，保证不截断）。
        app.world_mut()
            .entity_mut(caster)
            .insert(SwordBondComponent {
                bonded_weapon_entity: Entity::from_raw(1),
                bond_strength: 1.0,
                stored_qi: 0.0,
                grade: SwordGrade::Condensed,
            });

        let result = cast_qi_slash(app.world_mut(), caster, 0, None);
        assert!(matches!(result, CastResult::Started { .. }));

        let transfers: Vec<_> = app
            .world()
            .resource::<Events<QiTransfer>>()
            .iter_current_update_events()
            .collect();

        // 应有 1 笔 Channeling（player→sword_bond）+ 1 笔 ReleaseToZone（player→zone）。
        let channeling: Vec<_> = transfers
            .iter()
            .filter(|t| matches!(t.reason, QiTransferReason::Channeling))
            .collect();
        let release: Vec<_> = transfers
            .iter()
            .filter(|t| matches!(t.reason, QiTransferReason::ReleaseToZone))
            .collect();

        assert_eq!(
            channeling.len(),
            1,
            "有 bond 时应有 1 笔 Channeling audit event，实际 {}",
            channeling.len()
        );
        let expected_injected = qi_slash.qi_cost * super::super::bond::QI_INJECT_RATIO;
        assert!(
            (channeling[0].amount - expected_injected).abs() < 1e-9,
            "Channeling amount = cost × QI_INJECT_RATIO = {expected_injected:.2}，实际 {}",
            channeling[0].amount,
        );

        assert_eq!(
            release.len(),
            1,
            "有 bond 时仍应有 1 笔 ReleaseToZone，实际 {}",
            release.len()
        );
        let expected_remainder = qi_slash.qi_cost - expected_injected;
        assert!(
            (release[0].amount - expected_remainder).abs() < 1e-9,
            "ReleaseToZone amount = cost - injected = {expected_remainder:.2}，实际 {}",
            release[0].amount,
        );

        // bond.stored_qi 应已注入 injected 量。
        let bond = app.world().get::<SwordBondComponent>(caster).unwrap();
        assert!(
            (bond.stored_qi - expected_injected).abs() < 1e-9,
            "bond.stored_qi 应等于注入量 {expected_injected:.2}，实际 {}",
            bond.stored_qi,
        );
    }

    /// QP-002 — 凝脉以下灵剑（Mortal）：bond 不存储，全部 cost 归还 zone（与无 bond 相同）。
    #[test]
    fn qi_slash_with_mortal_bond_credits_full_cost_to_zone() {
        let (mut app, caster) = setup_app();
        let qi_slash = technique_definition(&app, SWORD_PATH_QI_SLASH_ID);
        let mut registry = crate::world::zone::ZoneRegistry::fallback();
        registry.find_zone_mut("spawn").unwrap().spirit_qi = 0.3;
        app.world_mut().insert_resource(registry);

        app.world_mut()
            .entity_mut(caster)
            .insert(SwordBondComponent {
                bonded_weapon_entity: Entity::from_raw(2),
                bond_strength: 0.5,
                stored_qi: 0.0,
                grade: SwordGrade::Mortal, // can_store_qi() = false
            });

        let result = cast_qi_slash(app.world_mut(), caster, 0, None);
        assert!(matches!(result, CastResult::Started { .. }));

        let channeling_count = app
            .world()
            .resource::<Events<QiTransfer>>()
            .iter_current_update_events()
            .filter(|t| matches!(t.reason, QiTransferReason::Channeling))
            .count();
        assert_eq!(
            channeling_count, 0,
            "Mortal 灵剑不应产生 Channeling event，实际 {channeling_count}"
        );

        let release: Vec<_> = app
            .world()
            .resource::<Events<QiTransfer>>()
            .iter_current_update_events()
            .filter(|t| matches!(t.reason, QiTransferReason::ReleaseToZone))
            .collect();
        assert_eq!(release.len(), 1, "Mortal 灵剑全部 cost 归 zone");
        assert!(
            (release[0].amount - qi_slash.qi_cost).abs() < 1e-9,
            "全额归 zone = qi_slash.qi_cost={:.1}，实际 {}",
            qi_slash.qi_cost,
            release[0].amount,
        );
    }

    /// QP-002 — 无 ZoneRegistry 资源时 cast 不 panic，守恒 skip（优雅降级）。
    #[test]
    fn qi_slash_without_zone_registry_does_not_panic() {
        let (mut app, caster) = setup_app();
        // 故意不插入 ZoneRegistry（模拟早期启动期）。

        let result = cast_qi_slash(app.world_mut(), caster, 0, None);
        // 应正常完成，不 panic，不 reject。
        assert!(
            matches!(result, CastResult::Started { .. }),
            "无 ZoneRegistry 时 cast 不应 panic / reject，实际 {result:?}"
        );
    }

    /// QP-002 — RESONANCE（20.0 qi）无 bond：全额归 zone。
    #[test]
    fn resonance_without_bond_credits_full_cost_to_zone() {
        let (mut app, caster) = setup_app();
        let resonance = technique_definition(&app, SWORD_PATH_RESONANCE_ID);
        let mut registry = crate::world::zone::ZoneRegistry::fallback();
        registry.find_zone_mut("spawn").unwrap().spirit_qi = 0.1;
        app.world_mut().insert_resource(registry);

        let result = cast_resonance(app.world_mut(), caster, 0, None);
        assert!(matches!(result, CastResult::Started { .. }));

        let release: Vec<_> = app
            .world()
            .resource::<Events<QiTransfer>>()
            .iter_current_update_events()
            .filter(|t| matches!(t.reason, QiTransferReason::ReleaseToZone))
            .collect();
        assert_eq!(release.len(), 1, "剑鸣无 bond 应有 1 笔 ReleaseToZone");
        assert!(
            (release[0].amount - resonance.qi_cost).abs() < 1e-9,
            "ReleaseToZone = resonance.qi_cost={:.1}，实际 {}",
            resonance.qi_cost,
            release[0].amount,
        );
    }

    /// QP-002 — MANIFEST（40.0 qi）无 bond：全额归 zone。
    #[test]
    fn manifest_without_bond_credits_full_cost_to_zone() {
        let (mut app, caster) = setup_app();
        let manifest = technique_definition(&app, SWORD_PATH_MANIFEST_ID);
        let mut registry = crate::world::zone::ZoneRegistry::fallback();
        registry.find_zone_mut("spawn").unwrap().spirit_qi = 0.1;
        app.world_mut().insert_resource(registry);

        let result = cast_manifest(app.world_mut(), caster, 0, None);
        assert!(matches!(result, CastResult::Started { .. }));

        let release: Vec<_> = app
            .world()
            .resource::<Events<QiTransfer>>()
            .iter_current_update_events()
            .filter(|t| matches!(t.reason, QiTransferReason::ReleaseToZone))
            .collect();
        assert_eq!(release.len(), 1, "化形无 bond 应有 1 笔 ReleaseToZone");
        assert!(
            (release[0].amount - manifest.qi_cost).abs() < 1e-9,
            "ReleaseToZone = manifest.qi_cost={:.1}，实际 {}",
            manifest.qi_cost,
            release[0].amount,
        );
    }

    // ─── §D 四阶段 phase_system 专属测试 ────────────────────────────────────────

    /// §D phase system 测试用 app 工厂（额外注册 VfxEventRequest + phase system）。
    fn setup_phase_app() -> (App, Entity) {
        let (mut app, caster) = setup_app();
        app.add_event::<crate::network::vfx_event_emit::VfxEventRequest>();
        app.add_systems(Update, heaven_gate_phase_system);
        (app, caster)
    }

    /// 从 VfxEventRequest 事件中提取所有 SpawnParticle event_id。
    fn drain_vfx_ids(app: &App) -> Vec<String> {
        use crate::schema::vfx_event::VfxEventPayloadV1;
        app.world()
            .resource::<Events<crate::network::vfx_event_emit::VfxEventRequest>>()
            .iter_current_update_events()
            .filter_map(|req| {
                if let VfxEventPayloadV1::SpawnParticle { event_id, .. } = &req.payload {
                    Some(event_id.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// §D P0 — 蓄力帧（elapsed==0）：charge_0s 粒子。
    #[test]
    fn phase_system_elapsed_0_emits_charge_0s_particle() {
        let (mut app, caster) = setup_phase_app();
        // CombatClock.tick = 100；cast → start_tick = 100 → elapsed = 100-100 = 0
        let result = cast_heaven_gate(app.world_mut(), caster, 0, None);
        assert!(
            matches!(result, CastResult::Started { .. }),
            "天门 cast 应返回 Started，实际 {result:?}"
        );
        // tick 不动，直接跑系统
        app.update();
        let vfx = drain_vfx_ids(&app);
        assert!(
            vfx.contains(&"bong:heaven_gate_charge_0s".to_string()),
            "elapsed==0 时应 emit charge_0s 粒子，实际 {vfx:?}"
        );
    }

    /// §D P1 — 临界帧（elapsed==60 = CHARGE_END）：charge_1s + flash 粒子。
    #[test]
    fn phase_system_elapsed_60_emits_charge_1s_and_flash() {
        let (mut app, caster) = setup_phase_app();
        let result = cast_heaven_gate(app.world_mut(), caster, 0, None);
        assert!(
            matches!(result, CastResult::Started { .. }),
            "天门 cast 应返回 Started，实际 {result:?}"
        );
        // start_tick=100，推进到 160 → elapsed=60
        app.world_mut().resource_mut::<CombatClock>().tick = 160;
        app.update();
        let vfx = drain_vfx_ids(&app);
        assert!(
            vfx.contains(&"bong:heaven_gate_charge_1s".to_string()),
            "elapsed==60 时应 emit charge_1s 粒子，实际 {vfx:?}"
        );
        assert!(
            vfx.contains(&"bong:heaven_gate_flash".to_string()),
            "elapsed==60 时应 emit flash 粒子，实际 {vfx:?}"
        );
    }

    /// §D P2 — AoE 帧（elapsed==120 = CRITICAL_END）：对范围内目标发 AttackIntent。
    #[test]
    fn phase_system_elapsed_120_triggers_aoe_attack_intent() {
        let (mut app, caster) = setup_phase_app();
        // 范围内放一个目标（5 格内，HEAVEN_GATE_RADIUS=100，肯定命中）
        let target = app.world_mut().spawn(Position::new([5.0, 0.0, 0.0])).id();

        let result = cast_heaven_gate(app.world_mut(), caster, 0, None);
        assert!(
            matches!(result, CastResult::Started { .. }),
            "天门 cast 应返回 Started，实际 {result:?}"
        );
        // start_tick=100，推进到 220 → elapsed=120
        app.world_mut().resource_mut::<CombatClock>().tick = 220;
        app.update();

        let intents: Vec<_> = app
            .world()
            .resource::<Events<AttackIntent>>()
            .iter_current_update_events()
            .collect();
        assert!(
            intents
                .iter()
                .any(|i| i.target == Some(target) && i.source == AttackSource::SwordPathHeavenGate),
            "elapsed==120 时应对范围内目标发 SwordPathHeavenGate AttackIntent，实际 {intents:?}"
        );
        // shockwave 粒子
        let vfx = drain_vfx_ids(&app);
        assert!(
            vfx.contains(&"bong:heaven_gate_shockwave".to_string()),
            "AoE 帧应 emit shockwave 粒子，实际 {vfx:?}"
        );
    }

    #[test]
    fn heaven_gate_phase_uses_range_snapshotted_from_metadata() {
        let (mut app, caster) = setup_phase_app();
        let configured_range = 4.0_f32;
        app.insert_resource(TechniqueRegistry::load_for_tests_with_override(
            SWORD_PATH_HEAVEN_GATE_ID,
            |definition| definition.range = configured_range,
        ));
        let inside = app.world_mut().spawn(Position::new([3.5, 0.0, 0.0])).id();
        let outside = app.world_mut().spawn(Position::new([4.5, 0.0, 0.0])).id();

        let result = cast_heaven_gate(app.world_mut(), caster, 0, None);
        assert!(matches!(result, CastResult::Started { .. }));
        assert_eq!(
            app.world()
                .get::<HeavenGateChanneling>(caster)
                .expect("cast must insert channeling")
                .range,
            f64::from(configured_range),
            "cast must snapshot TechniqueRegistry range into channeling"
        );

        app.world_mut().resource_mut::<CombatClock>().tick = 220;
        app.update();
        let intents: Vec<_> = app
            .world()
            .resource::<Events<AttackIntent>>()
            .iter_current_update_events()
            .filter(|intent| intent.source == AttackSource::SwordPathHeavenGate)
            .collect();
        assert!(
            intents.iter().any(|intent| intent.target == Some(inside)),
            "target inside overridden range must be hit"
        );
        assert!(
            intents.iter().all(|intent| intent.target != Some(outside)),
            "target outside overridden range must not be admitted by the removed Rust radius constant"
        );
        assert!(
            intents
                .iter()
                .all(|intent| (intent.reach.base - configured_range).abs() < f32::EPSILON),
            "emitted AttackReach must carry overridden metadata range"
        );
    }

    /// §D — AoE 只结算一次（aoe_done guard）。
    /// 若 phase_system 连续运行两帧 elapsed==120（时钟不前进），AoE 只触发一次。
    #[test]
    fn phase_system_aoe_fires_only_once() {
        let (mut app, caster) = setup_phase_app();
        app.world_mut().spawn(Position::new([5.0, 0.0, 0.0]));

        let result = cast_heaven_gate(app.world_mut(), caster, 0, None);
        assert!(
            matches!(result, CastResult::Started { .. }),
            "天门 cast 应返回 Started，实际 {result:?}"
        );
        // start_tick=100，固定在 220 → elapsed=120 两帧
        app.world_mut().resource_mut::<CombatClock>().tick = 220;
        app.update();
        // 不推进时钟，再跑一帧
        app.update();

        // 只应有 1 笔 AoE AttackIntent（第 2 帧 aoe_done=true 不再结算）
        let aoe_intents: Vec<_> = app
            .world()
            .resource::<Events<AttackIntent>>()
            .iter_current_update_events()
            .filter(|i| i.source == AttackSource::SwordPathHeavenGate)
            .collect();
        // 第 2 帧发生在 iter_current_update_events（只含最近一帧），应为 0
        assert_eq!(
            aoe_intents.len(),
            0,
            "第 2 帧 aoe_done=true，不应再发 AoE AttackIntent，实际 {}",
            aoe_intents.len()
        );
    }

    /// §D P3 — aftermath 后 HeavenGateChanneling 组件被移除。
    #[test]
    fn phase_system_aftermath_removes_channeling_component() {
        let (mut app, caster) = setup_phase_app();
        let result = cast_heaven_gate(app.world_mut(), caster, 0, None);
        assert!(
            matches!(result, CastResult::Started { .. }),
            "天门 cast 应返回 Started，实际 {result:?}"
        );
        assert!(
            app.world().get::<HeavenGateChanneling>(caster).is_some(),
            "cast 后 caster 必须有 HeavenGateChanneling 组件"
        );

        // 推进到 aftermath
        app.world_mut().resource_mut::<CombatClock>().tick = 240;
        app.update();

        assert!(
            app.world().get::<HeavenGateChanneling>(caster).is_none(),
            "aftermath 后 HeavenGateChanneling 必须被移除（channel 结束）"
        );
    }

    /// §D — 蓄力帧之前（elapsed < 0，即时钟倒退或 u64 溢出）：无副作用，不 panic。
    /// 实际场景：时钟精度问题导致 now < start_tick；u64 saturating_sub 保证 elapsed=0。
    #[test]
    fn phase_system_clock_before_start_no_panic() {
        let (mut app, caster) = setup_phase_app();
        let result = cast_heaven_gate(app.world_mut(), caster, 0, None);
        assert!(
            matches!(result, CastResult::Started { .. }),
            "天门 cast 应返回 Started，实际 {result:?}"
        );
        // start_tick=100，把时钟设为 50（比 start 早）→ saturating_sub = 0 → charge_0s 粒子
        app.world_mut().resource_mut::<CombatClock>().tick = 50;
        app.update(); // 不应 panic
    }

    // ─── §C SwordIntentEntity 专属行为测试 ─────────────────────────────────────

    /// §C — 化形无 bond 时 SwordIntentEntity 仍可 spawn（不依赖 bond）。
    #[test]
    fn manifest_spawns_sword_intent_entity_even_without_bond() {
        use super::super::sword_intent_entity::SwordIntentEntity;
        let (mut app, caster) = setup_app();
        // 无 SwordBondComponent
        let target = app.world_mut().spawn(Position::default()).id();
        let result = cast_manifest(app.world_mut(), caster, 0, Some(target));
        assert!(
            matches!(result, CastResult::Started { .. }),
            "无 bond 化形 cast 应返回 Started，实际 {result:?}"
        );
        let mut q = app.world_mut().query::<&SwordIntentEntity>();
        let count = q.iter(app.world()).count();
        assert_eq!(
            count, 1,
            "无 bond 时化形仍应 spawn SwordIntentEntity，count={count}"
        );
    }

    /// §C — 化形 bond_strength 从 0.0 不能再扣（不能变负）。
    #[test]
    fn manifest_does_not_underflow_bond_strength_at_zero() {
        let (mut app, caster) = setup_app();
        app.world_mut()
            .entity_mut(caster)
            .insert(SwordBondComponent {
                bonded_weapon_entity: Entity::from_raw(2),
                bond_strength: 0.0,
                stored_qi: 0.0,
                grade: SwordGrade::Spirit,
            });
        let result = cast_manifest(app.world_mut(), caster, 0, None);
        assert!(
            matches!(result, CastResult::Started { .. }),
            "bond_strength=0 化形 cast 应返回 Started，实际 {result:?}"
        );
        let bond = app.world().get::<SwordBondComponent>(caster).unwrap();
        assert!(
            bond.bond_strength >= 0.0,
            "bond_strength 不应变负（clamp 下界 0）：实际 {}",
            bond.bond_strength
        );
    }

    // ─── #qi-sweep-heaven-gate-drain 守恒修复专属测试 ────────────────────────────

    /// 守恒修复 happy path：phase_system aftermath 将 qi_current 直写 zone.spirit_qi。
    ///
    /// 修复前：qi_current 归零但 zone.spirit_qi 不变（真元蒸发）。
    /// 修复后：zone.spirit_qi 上升，且有对应 ReleaseToZone QiTransfer（audit）。
    ///
    /// 坑 (a)：fallback zone spirit_qi 置 0 避免近满溢出造成断言偏差。
    /// 坑 (b)：credit_qi_current_to_zone 用 DimensionKind::Overworld，不需要 CurrentDimension。
    #[test]
    fn heaven_gate_phase_system_credits_qi_current_to_zone() {
        let (mut app, caster) = setup_phase_app();
        // 坑 (a): 置空 zone，保证全量 qi_drained 都能入 zone（不被近满截断）。
        let mut registry = crate::world::zone::ZoneRegistry::fallback();
        registry.find_zone_mut("spawn").unwrap().spirit_qi = 0.0;
        app.world_mut().insert_resource(registry);

        // 坑 (c): staging_buffer = qi_max + stored_qi = 5000 + 0 = 5000；若 qi_current 也为 5000，
        // 则两笔 ReleaseToZone amount 相同，filter 撞出 2 笔导致 assert_eq!(len, 1) 红。
        // 令 qi_current = 3000 < qi_max=5000（天门无 qi_cost，cast 不扣真元），
        // 使"qi_current 归还 zone"事件(amount=3000)与"staging_buffer 释放"事件(amount=5000)可区分。
        app.world_mut()
            .get_mut::<Cultivation>(caster)
            .unwrap()
            .qi_current = 3000.0;

        let qi_before = app.world().get::<Cultivation>(caster).unwrap().qi_current;
        let result = cast_heaven_gate(app.world_mut(), caster, 0, None);
        assert!(matches!(result, CastResult::Started { .. }));

        // 推进到 aftermath（elapsed >= HEAVEN_GATE_AOE_END = 140）
        app.world_mut().resource_mut::<CombatClock>().tick = 240;
        app.update();

        // qi_current 归零（既有行为不变）
        let qi_after = app.world().get::<Cultivation>(caster).unwrap().qi_current;
        assert_eq!(
            qi_after, 0.0,
            "aftermath qi_current 必须归零，实际 {qi_after}"
        );

        // 守恒：zone.spirit_qi 必须上升（qi_before / QI_ZONE_UNIT_CAPACITY 转 zone 单位）
        let zone_spirit_qi = app
            .world_mut()
            .resource_mut::<crate::world::zone::ZoneRegistry>()
            .find_zone_mut("spawn")
            .unwrap()
            .spirit_qi;
        assert!(
            zone_spirit_qi > 0.0,
            "heaven gate aftermath 必须将 qi_current 补归 zone.spirit_qi（\
             修复前此值为 0.0，代表真元蒸发），实际 {zone_spirit_qi}"
        );

        // qi_drained 的 ReleaseToZone QiTransfer（audit entry）必须发出。
        let qi_drained_events: Vec<_> = app
            .world()
            .resource::<Events<QiTransfer>>()
            .iter_current_update_events()
            .filter(|t| {
                matches!(t.reason, QiTransferReason::ReleaseToZone)
                    && (t.amount - qi_before).abs() < 1e-6
            })
            .collect();
        assert_eq!(
            qi_drained_events.len(),
            1,
            "应有 1 笔 amount=qi_current({qi_before}) 的 ReleaseToZone（qi_current 归还 zone），\
             实际 {} 笔",
            qi_drained_events.len()
        );
    }

    /// 守恒修复 happy path — 旧 heaven_gate_cast_system（legacy path，由 HeavenGateCastEvent 触发）
    /// 同样将 qi_current 直写 zone.spirit_qi。
    #[test]
    fn heaven_gate_cast_system_legacy_credits_qi_current_to_zone() {
        let (mut app, caster) = setup_app();
        // legacy system 注册到 Update schedule
        app.add_systems(Update, heaven_gate_cast_system);

        // 坑 (a): 置空 zone 避免溢出
        let mut registry = crate::world::zone::ZoneRegistry::fallback();
        registry.find_zone_mut("spawn").unwrap().spirit_qi = 0.0;
        app.world_mut().insert_resource(registry);

        let qi_before = app.world().get::<Cultivation>(caster).unwrap().qi_current;
        let qi_max_before = app.world().get::<Cultivation>(caster).unwrap().qi_max;

        // 手动 send HeavenGateCastEvent（模拟 legacy cast path）。
        // 注：stored_qi 用 137.0（非零）使 staging_buffer = qi_max + 137.0 = 5137.0，
        // 与 qi_current(5000.0) 区分，避免两笔 ReleaseToZone amount 碰撞导致 filter 撞出 2 笔。
        app.world_mut()
            .resource_mut::<Events<HeavenGateCastEvent>>()
            .send(HeavenGateCastEvent {
                caster,
                position: DVec3::ZERO,
                qi_max: qi_max_before,
                stored_qi: 137.0,
            });

        app.update();

        // qi_current 归零
        let qi_after = app.world().get::<Cultivation>(caster).unwrap().qi_current;
        assert_eq!(
            qi_after, 0.0,
            "legacy aftermath qi_current 必须归零，实际 {qi_after}"
        );

        // zone.spirit_qi 上升
        let zone_spirit_qi = app
            .world_mut()
            .resource_mut::<crate::world::zone::ZoneRegistry>()
            .find_zone_mut("spawn")
            .unwrap()
            .spirit_qi;
        assert!(
            zone_spirit_qi > 0.0,
            "legacy heaven_gate_cast_system 必须将 qi_current 补归 zone.spirit_qi，实际 {zone_spirit_qi}"
        );

        // qi_drained ReleaseToZone audit 事件
        let qi_drained_events: Vec<_> = app
            .world()
            .resource::<Events<QiTransfer>>()
            .iter_current_update_events()
            .filter(|t| {
                matches!(t.reason, QiTransferReason::ReleaseToZone)
                    && (t.amount - qi_before).abs() < 1e-6
            })
            .collect();
        assert_eq!(
            qi_drained_events.len(),
            1,
            "legacy path 应有 1 笔 amount=qi_current({qi_before}) 的 ReleaseToZone，实际 {} 笔",
            qi_drained_events.len()
        );
    }

    /// 守恒修复边界：无 ZoneRegistry 时不 panic，QiTransfer audit 仍然发出。
    ///
    /// 场景：测试/启动期 ZoneRegistry 尚未插入 → zone.spirit_qi 写跳过，
    /// 但 QiTransfer 事件仍应落到默认 spawn zone 账户供 summarize_world_qi 统计。
    #[test]
    fn heaven_gate_aftermath_no_zone_registry_no_panic_audit_still_emitted() {
        let (mut app, caster) = setup_phase_app();
        // 故意不插入 ZoneRegistry。

        // 坑 (c): staging_buffer = qi_max + stored_qi = 5000 + 0 = 5000；若 qi_current 也为 5000，
        // 则两笔 ReleaseToZone amount 相同，filter 撞出 2 笔。
        // 令 qi_current = 3000，使"qi_current 归还"事件(amount=3000)与"staging_buffer"事件(amount=5000)可区分。
        app.world_mut()
            .get_mut::<Cultivation>(caster)
            .unwrap()
            .qi_current = 3000.0;

        let qi_before = app.world().get::<Cultivation>(caster).unwrap().qi_current;
        let result = cast_heaven_gate(app.world_mut(), caster, 0, None);
        assert!(matches!(result, CastResult::Started { .. }));

        app.world_mut().resource_mut::<CombatClock>().tick = 240;
        app.update(); // 不应 panic

        // qi_current 归零（守恒不受影响）
        let qi_after = app.world().get::<Cultivation>(caster).unwrap().qi_current;
        assert_eq!(
            qi_after, 0.0,
            "无 ZoneRegistry 时 qi_current 仍必须归零，实际 {qi_after}"
        );

        // qi_drained 的 audit QiTransfer 仍应发出（zone_name = spawn 作为账户 fallback）
        let qi_drained_events: Vec<_> = app
            .world()
            .resource::<Events<QiTransfer>>()
            .iter_current_update_events()
            .filter(|t| {
                matches!(t.reason, QiTransferReason::ReleaseToZone)
                    && (t.amount - qi_before).abs() < 1e-6
            })
            .collect();
        assert_eq!(
            qi_drained_events.len(),
            1,
            "无 ZoneRegistry 时 qi_drained QiTransfer 仍应发出（audit），实际 {} 笔",
            qi_drained_events.len()
        );
    }
}
