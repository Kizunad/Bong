//! plan-sword-path-v2 P1.4 / P1.5 / P1.6 / P2.1 — 剑道五招的 SkillRegistry 入口、
//! 经脉依赖声明、命中效果接入、化虚 runtime。
//!
//! 五招遵循同一骨架：
//! 1. 校验持剑（`WeaponKind::Sword`）+ 冷却 + 体力 + 真元 + 经脉依赖。
//! 2. 校验是否拥有该招式（`KnownTechniques` active + proficiency）。
//! 3. 走 worldview §二 守恒律：真元消耗写 `Cultivation.qi_current`；下注真元到
//!    灵剑容器走 `QiTransfer { reason: Channeling }`；化虚释放走 `ReleaseToZone`。
//! 4. 应用效果（直接 AttackIntent / 状态效果 / 化形实体），招式自身的 vfx /
//!    audio 暂留 P4，本 P 只保证 ECS 链路对得齐。

use std::collections::HashSet;

use valence::prelude::{
    bevy_ecs, DVec3, Entity, EventReader, Events, Position, Query, Res, ResMut,
};

use crate::combat::components::{
    Casting, SkillBarBindings, Stamina, StaminaState, StatusEffects, WoundKind,
};
use crate::combat::events::{
    ApplyStatusEffectIntent, AttackIntent, AttackReach, AttackSource, StatusEffectKind,
};
use crate::combat::weapon::{Weapon, WeaponKind};
use crate::combat::CombatClock;
use crate::cultivation::components::{Cultivation, MeridianId, MeridianSystem, Realm};
use crate::cultivation::known_techniques::KnownTechniques;
use crate::cultivation::meridian::severed::{
    check_meridian_dependencies, MeridianSeveredPermanent, SkillMeridianDependencies,
};
use crate::cultivation::skill_registry::{CastRejectReason, CastResult, SkillRegistry};
use crate::cultivation::technique_scroll::realm_rank;
use crate::network::cast_emit::current_unix_millis;
use crate::qi_physics::{QiAccountId, QiTransfer, QiTransferReason};
use crate::world::zone::DEFAULT_SPAWN_ZONE_NAME;

use super::bond::{SwordBondComponent, SwordShatterEvent};
#[cfg(test)]
use super::grade::SwordGrade;
use super::heaven_gate::{
    create_blind_zone_from_cast, HeavenGateCastEvent, TiandaoBlindZoneRegistry,
};
use super::shatter::compute_heaven_gate_shatter;
use super::techniques::{effects, CONDENSE_EDGE, HEAVEN_GATE, MANIFEST, QI_SLASH, RESONANCE};

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

    let Some(target) = target else {
        return CastResult::Rejected {
            reason: CastRejectReason::InvalidTarget,
        };
    };

    apply_cast_costs(world, caster, slot, ctx.now_tick, &CONDENSE_EDGE_PROFILE);
    inject_bond_qi(world, caster, CONDENSE_EDGE.qi_cost);

    world.send_event(AttackIntent {
        attacker: caster,
        target: Some(target),
        issued_at_tick: ctx.now_tick,
        reach: AttackReach::new(CONDENSE_EDGE.range, 0.5),
        qi_invest: effects::CONDENSE_EDGE_DAMAGE_MULT,
        wound_kind: WoundKind::Cut,
        source: AttackSource::SwordPathCondenseEdge,
        debug_command: None,
    });

    CastResult::Started {
        cooldown_ticks: u64::from(CONDENSE_EDGE.cooldown_ticks),
        anim_duration_ticks: CONDENSE_EDGE.cast_ticks,
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

    let Some(target) = target else {
        return CastResult::Rejected {
            reason: CastRejectReason::InvalidTarget,
        };
    };

    if !drain_qi(world, caster, QI_SLASH.qi_cost) {
        return CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient,
        };
    }
    apply_cast_costs(world, caster, slot, ctx.now_tick, &QI_SLASH_PROFILE);
    inject_bond_qi(world, caster, QI_SLASH.qi_cost);

    world.send_event(AttackIntent {
        attacker: caster,
        target: Some(target),
        issued_at_tick: ctx.now_tick,
        reach: AttackReach::new(QI_SLASH.range, 0.0),
        qi_invest: QI_SLASH.qi_cost as f32,
        wound_kind: WoundKind::Cut,
        source: AttackSource::SwordPathQiSlash,
        debug_command: None,
    });

    CastResult::Started {
        cooldown_ticks: u64::from(QI_SLASH.cooldown_ticks),
        anim_duration_ticks: QI_SLASH.cast_ticks,
    }
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

    if !drain_qi(world, caster, RESONANCE.qi_cost) {
        return CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient,
        };
    }
    apply_cast_costs(world, caster, slot, ctx.now_tick, &RESONANCE_PROFILE);
    inject_bond_qi(world, caster, RESONANCE.qi_cost);

    // 6 格 AoE：扫范围内有 StatusEffects 的实体打 Slowed。
    // 范围内目标列表先 collect 出来，避免持有 query borrow 时 send_event。
    let center = world
        .get::<Position>(caster)
        .map(|p| p.get())
        .unwrap_or(DVec3::ZERO);
    let radius_sq = (RESONANCE.range as f64).powi(2);
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

    CastResult::Started {
        cooldown_ticks: u64::from(RESONANCE.cooldown_ticks),
        anim_duration_ticks: RESONANCE.cast_ticks,
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
    let Some(target) = target else {
        return CastResult::Rejected {
            reason: CastRejectReason::InvalidTarget,
        };
    };

    if !drain_qi(world, caster, MANIFEST.qi_cost) {
        return CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient,
        };
    }
    apply_cast_costs(world, caster, slot, ctx.now_tick, &MANIFEST_PROFILE);
    inject_bond_qi(world, caster, MANIFEST.qi_cost);

    // 化形完整版（剑意实体追踪 5s）留待 v3 BOSS AI 之后做。本 phase 用单次高强度
    // AttackIntent 作占位，保证伤害与品阶乘数走 combat pipeline。
    world.send_event(AttackIntent {
        attacker: caster,
        target: Some(target),
        issued_at_tick: ctx.now_tick,
        reach: AttackReach::new(MANIFEST.range, 0.0),
        qi_invest: effects::MANIFEST_ATTACK_MULT,
        wound_kind: WoundKind::Cut,
        source: AttackSource::SwordPathManifest,
        debug_command: None,
    });

    // 化形结束后 bond_strength -= 0.1 (plan §techniques::effects::MANIFEST_BOND_PENALTY)
    if let Some(mut bond) = world.get_mut::<SwordBondComponent>(caster) {
        bond.bond_strength = (bond.bond_strength - effects::MANIFEST_BOND_PENALTY).max(0.0);
    }

    CastResult::Started {
        cooldown_ticks: u64::from(MANIFEST.cooldown_ticks),
        anim_duration_ticks: MANIFEST.cast_ticks,
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

    // 化虚一击是单向门：cast 后立即发 HeavenGateCastEvent，由 heaven_gate_cast_system
    // 统一处理境界跌落 + 真元归零 + 盲区注册 + shatter。这里只锁 cooldown + 发 event。
    apply_cast_costs(world, caster, slot, ctx.now_tick, &HEAVEN_GATE_PROFILE);
    world.send_event(HeavenGateCastEvent {
        caster,
        position,
        qi_max: cultivation.qi_max,
        stored_qi: world
            .get::<SwordBondComponent>(caster)
            .map(|b| b.stored_qi)
            .unwrap_or(0.0),
    });

    CastResult::Started {
        cooldown_ticks: u64::from(HEAVEN_GATE.cooldown_ticks),
        anim_duration_ticks: HEAVEN_GATE.cast_ticks,
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
    mut blind_registry: ResMut<TiandaoBlindZoneRegistry>,
) {
    // 多 caster 同 tick 触发的情况罕见，但为了保证 deterministic 排序，按 Entity bits 排序。
    let mut pending: Vec<HeavenGateCastEvent> = events.read().cloned().collect();
    pending.sort_by_key(|e| e.caster.to_bits());
    for event in pending {
        let staging_buffer = event.qi_max + event.stored_qi;

        // 100 格 AoE：每个范围内目标按距离衰减伤害。已死 / 无 Position 的略过。
        let center = event.position;
        let radius_sq = effects::HEAVEN_GATE_RADIUS.powi(2);
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
                    reach: AttackReach::new(effects::HEAVEN_GATE_RADIUS as f32, 0.0),
                    qi_invest: damage as f32,
                    wound_kind: WoundKind::Cut,
                    source: AttackSource::SwordPathManifest,
                    debug_command: None,
                });
            }
        }

        // Caster 修为 / 灵剑 aftermath
        if let Ok((mut cultivation, bond_opt)) = players.get_mut(event.caster) {
            cultivation.qi_max = (cultivation.qi_max * effects::HEAVEN_GATE_QI_MAX_RETAIN).max(0.0);
            cultivation.qi_current = 0.0;
            cultivation.realm = Realm::Solidify;

            if let Some(mut bond) = bond_opt {
                let stored = bond.stored_qi;
                bond.stored_qi = 0.0;
                if let Some(events) = shatter_events.as_deref_mut() {
                    events.send(SwordShatterEvent {
                        player: event.caster,
                        weapon: bond.bonded_weapon_entity,
                        stored_qi: stored,
                        grade: bond.grade,
                    });
                }
            }
        }

        // 盲区注册：把 caster 藏 5 min，agent world_state 不再推送其 snapshot。
        let zone = create_blind_zone_from_cast(&event, clock.tick);
        blind_registry.add(zone);

        // 守恒：staging_buffer 进 zone（worldview §二 真元守恒）。先用 compute
        // 函数算出 qi_max_lost / new_qi_max（不直接覆盖前面的 qi_max 写入）。
        let outcome = compute_heaven_gate_shatter(event.qi_max, event.stored_qi);
        let _ = outcome; // 数值已在上面写入 cultivation，这里仅保留 ledger entry。
        if let Some(transfers) = qi_transfers.as_deref_mut() {
            if let Ok(transfer) = QiTransfer::new(
                QiAccountId::player(format!("entity:{:?}", event.caster)),
                QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME),
                staging_buffer,
                QiTransferReason::ReleaseToZone,
            ) {
                transfers.send(transfer);
            }
        }
    }
}

// ─── 共用工具 ────────────────────────────────────────────────────────────────

struct CastContext {
    now_tick: u64,
}

struct CastProfile {
    cooldown_ticks: u64,
    cast_ticks: u32,
    stamina_cost: f32,
    skill_id: &'static str,
}

const CONDENSE_EDGE_PROFILE: CastProfile = CastProfile {
    cooldown_ticks: CONDENSE_EDGE.cooldown_ticks as u64,
    cast_ticks: CONDENSE_EDGE.cast_ticks,
    stamina_cost: CONDENSE_EDGE.stamina_cost,
    skill_id: SWORD_PATH_CONDENSE_EDGE_ID,
};

const QI_SLASH_PROFILE: CastProfile = CastProfile {
    cooldown_ticks: QI_SLASH.cooldown_ticks as u64,
    cast_ticks: QI_SLASH.cast_ticks,
    stamina_cost: QI_SLASH.stamina_cost,
    skill_id: SWORD_PATH_QI_SLASH_ID,
};

const RESONANCE_PROFILE: CastProfile = CastProfile {
    cooldown_ticks: RESONANCE.cooldown_ticks as u64,
    cast_ticks: RESONANCE.cast_ticks,
    stamina_cost: RESONANCE.stamina_cost,
    skill_id: SWORD_PATH_RESONANCE_ID,
};

const MANIFEST_PROFILE: CastProfile = CastProfile {
    cooldown_ticks: MANIFEST.cooldown_ticks as u64,
    cast_ticks: MANIFEST.cast_ticks,
    stamina_cost: MANIFEST.stamina_cost,
    skill_id: SWORD_PATH_MANIFEST_ID,
};

const HEAVEN_GATE_PROFILE: CastProfile = CastProfile {
    // 一次性招式：CD = u32::MAX 哨兵（plan §techniques::HEAVEN_GATE）。
    cooldown_ticks: u32::MAX as u64,
    cast_ticks: HEAVEN_GATE.cast_ticks,
    stamina_cost: HEAVEN_GATE.stamina_cost,
    skill_id: SWORD_PATH_HEAVEN_GATE_ID,
};

fn build_cast_context(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    skill_id: &'static str,
) -> Result<CastContext, CastRejectReason> {
    let now_tick = world
        .get_resource::<CombatClock>()
        .map(|c| c.tick)
        .unwrap_or_default();

    // 冷却（plan §SkillBarBindings）
    if world
        .get::<SkillBarBindings>(caster)
        .is_some_and(|b| b.is_on_cooldown(slot, now_tick))
    {
        return Err(CastRejectReason::OnCooldown);
    }

    // 持剑（必须 WeaponKind::Sword）
    let Some(weapon) = world.get::<Weapon>(caster) else {
        return Err(CastRejectReason::InvalidTarget);
    };
    if weapon.weapon_kind != WeaponKind::Sword {
        return Err(CastRejectReason::InvalidTarget);
    }

    // 体力 / Stunned
    if world
        .get::<Stamina>(caster)
        .is_some_and(|s| s.state == StaminaState::Exhausted || s.current <= 0.0)
    {
        return Err(CastRejectReason::InRecovery);
    }

    // 招式拥有 + active
    let Some(known) = world.get::<KnownTechniques>(caster) else {
        return Err(CastRejectReason::InvalidTarget);
    };
    if !known.entries.iter().any(|e| e.id == skill_id && e.active) {
        return Err(CastRejectReason::InvalidTarget);
    }

    // 境界（plan §techniques::required_realm）
    let cultivation = world
        .get::<Cultivation>(caster)
        .cloned()
        .ok_or(CastRejectReason::RealmTooLow)?;
    let required_realm = match skill_id {
        SWORD_PATH_CONDENSE_EDGE_ID => Realm::Induce,
        SWORD_PATH_QI_SLASH_ID => Realm::Condense,
        SWORD_PATH_RESONANCE_ID => Realm::Solidify,
        SWORD_PATH_MANIFEST_ID => Realm::Spirit,
        SWORD_PATH_HEAVEN_GATE_ID => Realm::Void,
        _ => Realm::Awaken,
    };
    if realm_rank(cultivation.realm) < realm_rank(required_realm) {
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
            if let Some(meridians) = world.get::<MeridianSystem>(caster) {
                if deps
                    .iter()
                    .all(|m| meridians.get(*m).integrity <= f64::EPSILON)
                {
                    return Err(CastRejectReason::MeridianSevered(deps.first().copied()));
                }
            }
        }
    }

    Ok(CastContext { now_tick })
}

fn apply_cast_costs(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    now_tick: u64,
    profile: &CastProfile,
) {
    if let Some(mut stamina) = world.get_mut::<Stamina>(caster) {
        stamina.current = (stamina.current - profile.stamina_cost.max(0.0)).clamp(0.0, stamina.max);
        stamina.state = if stamina.current <= 0.0 {
            StaminaState::Exhausted
        } else {
            StaminaState::Combat
        };
        stamina.last_drain_tick = Some(now_tick);
    }
    if let Some(mut bindings) = world.get_mut::<SkillBarBindings>(caster) {
        bindings.set_cooldown(slot, now_tick.saturating_add(profile.cooldown_ticks));
    }
    insert_casting(world, caster, slot, profile, now_tick);
}

fn insert_casting(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    slot: u8,
    profile: &CastProfile,
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
        duration_ticks: u64::from(profile.cast_ticks),
        started_at_ms: current_unix_millis(),
        duration_ms: profile.cast_ticks.saturating_mul(50),
        bound_instance_id: None,
        start_position,
        complete_cooldown_ticks: profile.cooldown_ticks,
        skill_id: Some(profile.skill_id.to_string()),
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

fn inject_bond_qi(world: &mut bevy_ecs::world::World, caster: Entity, qi_cost: f64) {
    // 灵剑必须 ≥ 凝脉品阶才有存储能力。注入按 plan §bond::QI_INJECT_RATIO = 0.1。
    let injected = match world.get_mut::<SwordBondComponent>(caster) {
        Some(mut bond) if bond.grade.can_store_qi() => bond.try_inject_qi(qi_cost),
        _ => return,
    };
    if injected <= f64::EPSILON {
        return;
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::weapon::EquipSlot;
    use crate::cultivation::components::Realm;
    use crate::cultivation::known_techniques::{KnownTechnique, KnownTechniques};
    use valence::prelude::{App, Update};

    fn setup_app() -> (App, Entity) {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 100 });
        app.insert_resource(SkillMeridianDependencies::default());
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<HeavenGateCastEvent>();
        app.add_event::<SwordShatterEvent>();
        app.add_event::<QiTransfer>();
        app.init_resource::<TiandaoBlindZoneRegistry>();

        let mut deps = app.world_mut().resource_mut::<SkillMeridianDependencies>();
        declare_meridian_dependencies(&mut deps);

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
                MeridianSystem::default(),
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
        let target = app.world_mut().spawn(Position::default()).id();
        let qi_before = app.world().get::<Cultivation>(caster).unwrap().qi_current;

        let result = cast_qi_slash(app.world_mut(), caster, 0, Some(target));
        assert!(matches!(result, CastResult::Started { .. }));

        let qi_after = app.world().get::<Cultivation>(caster).unwrap().qi_current;
        assert!(
            (qi_before - qi_after - QI_SLASH.qi_cost).abs() < 1e-6,
            "qi_current 应扣 {} (QI_SLASH.qi_cost)，实际差值 {}",
            QI_SLASH.qi_cost,
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
        // 真元降到 1 < QI_SLASH.qi_cost (3.0)
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

    /// P1.6 — 剑意化形发 AttackIntent + 扣 bond_strength 0.1（plan §effects::MANIFEST_BOND_PENALTY）。
    #[test]
    fn manifest_emits_intent_and_dings_bond_strength() {
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
        assert!(matches!(result, CastResult::Started { .. }));

        let bond = app.world().get::<SwordBondComponent>(caster).unwrap();
        assert!(
            (bond.bond_strength - 0.7).abs() < 1e-5,
            "bond_strength 应从 0.8 → 0.7 (扣 0.1)，实际 {}",
            bond.bond_strength
        );

        let intent = app
            .world()
            .resource::<Events<AttackIntent>>()
            .iter_current_update_events()
            .next()
            .expect("化形应发 AttackIntent");
        assert_eq!(intent.source, AttackSource::SwordPathManifest);
    }

    /// P2.1 — 化虚一击 → HeavenGateCastEvent → 系统结算：境界跌至固元，
    /// qi_max 衰减 90%，qi_current = 0，发 SwordShatterEvent，注册盲区。
    #[test]
    fn heaven_gate_cast_system_full_aftermath() {
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
        let target = app.world_mut().spawn(Position::new([10.0, 0.0, 0.0])).id();
        let _ = target;

        app.add_systems(Update, heaven_gate_cast_system);
        let cast_result = cast_heaven_gate(app.world_mut(), caster, 0, None);
        assert!(matches!(cast_result, CastResult::Started { .. }));
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
        assert_eq!(shatter_events.len(), 1, "必须发一条 SwordShatterEvent");
        assert_eq!(shatter_events[0].stored_qi, 100.0);

        let registry = app.world().resource::<TiandaoBlindZoneRegistry>();
        assert_eq!(
            registry.active_count(),
            1,
            "化虚一击必须注册一个天道盲区，agent 才会屏蔽 caster"
        );
    }
}
