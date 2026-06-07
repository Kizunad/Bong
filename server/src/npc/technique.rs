//! NPC 功法系统（plan-npc-combat-gear-v1 §P1）。
//!
//! NPC 复用玩家 `KnownTechniques` component，spawn 时由 `assign_npc_techniques()`
//! 一次性分配功法（proficiency 固定，不随战斗增长——§8.1 #2 决议）。
//!
//! 战斗中由 `NpcTechniqueScorer` 评估是否有可用功法，
//! `NpcTechniqueAction`（exclusive system）通过 `SkillRegistry.lookup()` 调用
//! 与玩家相同的 `SkillFn` 路径。
//!
//! 经脉依赖由 `SkillMeridianDependencies::lookup()` + `check_meridian_dependencies()`
//! 联合校验——断了肺经的 NPC 无法释放依赖肺经的功法（worldview §四:286）。

use std::collections::HashMap;

use big_brain::prelude::{ActionBuilder, ActionState, Actor, Score, ScorerBuilder};
use valence::prelude::{bevy_ecs, Commands, Component, Entity, Query, Res, Resource, With};

use crate::cultivation::components::{Cultivation, MeridianSystem, Realm};
use crate::cultivation::known_techniques::{
    technique_definition, KnownTechnique, KnownTechniques, SkillCategory, TechniqueDefinition,
    TECHNIQUE_DEFINITIONS,
};
use crate::cultivation::meridian::severed::{
    check_meridian_dependencies, MeridianSeveredPermanent, SkillMeridianDependencies,
};
use crate::cultivation::technique_scroll::{parse_meridian_id, realm_rank};
use crate::npc::lifecycle::NpcArchetype;
use crate::npc::spawn::NpcBlackboard;

// ─── splitmix64 helpers (deterministic RNG) ──────────────────────────────────

fn splitmix64(seed: u64) -> u64 {
    let mut x = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

fn splitmix64_unit(seed: u64) -> f32 {
    let bits = ((splitmix64(seed) >> 40) & 0x00FF_FFFF) as u32;
    bits as f32 / (1u32 << 24) as f32
}

fn splitmix64_range(seed: u64, n: u32) -> u32 {
    if n == 0 {
        return 0;
    }
    (splitmix64_unit(seed) * n as f32) as u32 % n
}

// ─── Realm parsing helper ────────────────────────────────────────────────────

fn parse_realm(raw: &str) -> Option<Realm> {
    match raw {
        "Awaken" => Some(Realm::Awaken),
        "Induce" => Some(Realm::Induce),
        "Condense" => Some(Realm::Condense),
        "Solidify" => Some(Realm::Solidify),
        "Spirit" => Some(Realm::Spirit),
        "Void" => Some(Realm::Void),
        _ => None,
    }
}

// ─── NpcCooldownMap Resource ─────────────────────────────────────────────────

/// NPC 功法冷却注册表。key = (npc_entity, technique_id), value = cooldown_until_tick.
///
/// 功法释放后写入 cooldown；NPC 死亡/despawn 时移除对应 entries 避免 Entity 复用冲突。
/// key 用 `String` 而非 `&'static str`，因为 exclusive system 中需要 owned key
/// 避免 lifetime 耦合。
#[derive(Default)]
pub struct NpcCooldownMap {
    map: HashMap<(Entity, String), u64>,
}

impl Resource for NpcCooldownMap {}

impl NpcCooldownMap {
    pub fn set(&mut self, npc: Entity, technique_id: &str, cooldown_until: u64) {
        self.map
            .insert((npc, technique_id.to_string()), cooldown_until);
    }

    pub fn is_on_cooldown(&self, npc: Entity, technique_id: &str, current_tick: u64) -> bool {
        self.map
            .get(&(npc, technique_id.to_string()))
            .is_some_and(|&until| current_tick < until)
    }

    /// 移除指定 NPC 的所有冷却 entries（NPC 死亡/despawn 时调用）。
    pub fn remove_all_for(&mut self, npc: Entity) {
        self.map.retain(|(entity, _), _| *entity != npc);
    }

    /// 仅供测试：返回所有 entries 数量。
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.map.len()
    }
}

// ─── Meridian system builder ────────────────────────────────────────────────

/// 根据境界生成 NPC 用的 MeridianSystem（按 worldview §8.1 #5 规则开脉）。
///
/// Awaken=1, Induce=3, Condense=6, Solidify=12, Spirit=16, Void=20。
/// 打开顺序：先 12 正经（REGULAR），再 8 奇经（EXTRAORDINARY）。
pub fn npc_meridian_system_for_realm(realm: Realm) -> MeridianSystem {
    use crate::cultivation::components::MeridianId;

    let count = realm.required_meridians();
    let mut sys = MeridianSystem::default();
    for &id in MeridianId::ALL.iter().take(count) {
        let m = sys.get_mut(id);
        m.opened = true;
        m.integrity = 1.0;
        m.throughput_current = 1.0;
    }
    sys
}

// ─── assign_npc_techniques ───────────────────────────────────────────────────

/// 检查 technique definition 的经脉依赖在 NPC 的 MeridianSystem 中是否满足（已开）。
fn meridian_deps_satisfied(
    definition: &TechniqueDefinition,
    meridian_sys: &MeridianSystem,
    meridian_deps: &SkillMeridianDependencies,
) -> bool {
    // 1. 检查 SkillMeridianDependencies 表中的依赖
    let deps = meridian_deps.lookup(definition.id);
    for dep_id in deps {
        let m = meridian_sys.get(*dep_id);
        if !m.opened {
            return false;
        }
    }
    // 2. 检查 TechniqueDefinition.required_meridians 中的依赖
    for required in definition.required_meridians {
        let Some(channel) = parse_meridian_id(required.channel) else {
            return false;
        };
        let m = meridian_sys.get(channel);
        if !m.opened || m.integrity < f64::from(required.min_health) {
            return false;
        }
    }
    true
}

/// 根据 archetype / realm / 经脉拓扑分配 NPC 功法（spawn 时调用）。
///
/// 分配的功法必须同时满足：
/// 1. `realm_rank(parse_realm(def.required_realm)) <= realm_rank(npc_realm)`
/// 2. 经脉依赖满足（`meridian_deps.lookup` + `MeridianSystem` 已开）
///
/// NPC 功法 proficiency spawn 时固定，不随战斗增长（§8.1 #2 决议）。
/// 各 archetype 功法分配见 plan §P1.1 表格。
pub fn assign_npc_techniques(
    archetype: NpcArchetype,
    realm: Realm,
    meridian_sys: &MeridianSystem,
    meridian_deps: &SkillMeridianDependencies,
    _qi_color_hint: Option<&str>,
    entity_seed: u64,
) -> KnownTechniques {
    let npc_rank = realm_rank(realm);

    // 收集所有 realm + 经脉可用的功法
    let available: Vec<&TechniqueDefinition> = TECHNIQUE_DEFINITIONS
        .iter()
        .filter(|def| {
            let Some(required) = parse_realm(def.required_realm) else {
                return false;
            };
            if realm_rank(required) > npc_rank {
                return false;
            }
            meridian_deps_satisfied(def, meridian_sys, meridian_deps)
        })
        .collect();

    let (count_min, count_max, prof_min, prof_max) = match archetype {
        // Commoner / Beast / SkullFiend / Fuya / Zombie → 无功法
        // plan-dying-elder-v1：垂死大能技法由 P1 inject（含 offered_skill_id），此处占位无功法
        NpcArchetype::Commoner
        | NpcArchetype::Beast
        | NpcArchetype::SkullFiend
        | NpcArchetype::Fuya
        | NpcArchetype::Zombie
        | NpcArchetype::DyingElder => {
            return KnownTechniques {
                entries: Vec::new(),
            };
        }
        NpcArchetype::Rogue => (1, 3, 0.2_f32, 0.7),
        NpcArchetype::Disciple => (2, 4, 0.3, 0.8),
        NpcArchetype::GuardianRelic => (3, 5, 0.6, 0.9),
        NpcArchetype::Daoxiang => (1, 2, 0.1, 0.4),
        NpcArchetype::Zhinian => (2, 3, 0.3, 0.6),
    };

    if available.is_empty() {
        return KnownTechniques {
            entries: Vec::new(),
        };
    }

    // 决定功法数量
    let range = (count_max - count_min + 1) as u32;
    let count = count_min + splitmix64_range(entity_seed, range) as usize;
    let count = count.min(available.len());

    // Fisher-Yates shuffle 选前 count 个
    let mut indices: Vec<usize> = (0..available.len()).collect();
    for i in (1..indices.len()).rev() {
        let j = splitmix64_range(
            entity_seed.wrapping_add(i as u64 * 0x9E37_79B9),
            (i + 1) as u32,
        ) as usize;
        indices.swap(i, j);
    }

    let mut entries: Vec<KnownTechnique> = indices
        .iter()
        .take(count)
        .enumerate()
        .map(|(idx, &orig_idx)| {
            let def = available[orig_idx];
            let prof_seed = entity_seed.wrapping_add(idx as u64 * 0xBF58_476D);
            let proficiency = prof_min + splitmix64_unit(prof_seed) * (prof_max - prof_min);
            KnownTechnique {
                id: def.id.to_string(),
                proficiency: proficiency.clamp(prof_min, prof_max),
                active: true,
            }
        })
        .collect();

    inject_npc_utility_skills(
        &mut entries,
        realm,
        &available,
        prof_min,
        prof_max,
        entity_seed,
    );

    KnownTechniques { entries }
}

fn inject_npc_utility_skills(
    entries: &mut Vec<KnownTechnique>,
    realm: Realm,
    available: &[&TechniqueDefinition],
    prof_min: f32,
    prof_max: f32,
    entity_seed: u64,
) {
    let npc_rank = realm_rank(realm);

    if npc_rank >= realm_rank(Realm::Induce) {
        try_inject_skill(
            entries,
            "npc.heal_basic",
            available,
            prof_min,
            prof_max,
            entity_seed.wrapping_add(0xA1B2_C3D4),
        );
    }

    if npc_rank >= realm_rank(Realm::Condense) {
        let buff_id = if splitmix64_unit(entity_seed.wrapping_add(0xD4C3_B2A1)) < 0.5 {
            "npc.buff_speed"
        } else {
            "npc.buff_defense"
        };
        try_inject_skill(
            entries,
            buff_id,
            available,
            prof_min,
            prof_max,
            entity_seed.wrapping_add(0xE5F6_0718),
        );
    }
}

fn try_inject_skill(
    entries: &mut Vec<KnownTechnique>,
    skill_id: &str,
    available: &[&TechniqueDefinition],
    prof_min: f32,
    prof_max: f32,
    seed: u64,
) {
    if entries.iter().any(|e| e.id == skill_id) {
        return;
    }
    if !available.iter().any(|def| def.id == skill_id) {
        return;
    }
    let proficiency = prof_min + splitmix64_unit(seed) * (prof_max - prof_min);
    entries.push(KnownTechnique {
        id: skill_id.to_string(),
        proficiency: proficiency.clamp(prof_min, prof_max),
        active: true,
    });
}

// ─── NpcSkillScoringContext ──────────────────────────────────────────────────

pub struct NpcSkillScoringContext {
    pub hp_ratio: f32,
    pub qi_ratio: f32,
    pub target_distance: f32,
    pub target_hp_ratio: f32,
    pub has_active_buff: bool,
    pub in_combat: bool,
}

pub fn category_weight(category: SkillCategory, ctx: &NpcSkillScoringContext) -> f32 {
    match category {
        SkillCategory::Heal => (1.0 - ctx.hp_ratio).powf(2.0) * 0.9,
        SkillCategory::Buff => {
            if !ctx.has_active_buff && ctx.in_combat {
                0.6
            } else {
                0.05
            }
        }
        SkillCategory::Attack => 0.8,
        SkillCategory::Control => 0.4,
        SkillCategory::Defense => 0.0,
    }
}

// ─── SkillTarget + SelectedTechnique ────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillTarget {
    NearestEnemy,
    SelfCast,
}

pub fn skill_target_for_category(category: SkillCategory) -> SkillTarget {
    match category {
        SkillCategory::Heal | SkillCategory::Buff => SkillTarget::SelfCast,
        SkillCategory::Attack | SkillCategory::Control | SkillCategory::Defense => {
            SkillTarget::NearestEnemy
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectedTechnique {
    pub technique_id: String,
    pub target: SkillTarget,
}

// ─── select_technique ────────────────────────────────────────────────────────

/// NPC 战斗中选择功法。
///
/// 过滤逻辑：active → category_filter 限定 → 经脉 SEVERED 排除 → 冷却排除 → qi 不足排除 →
/// qi_ratio < 0.15 排除高 qi_cost 50% → Defense 排除（走独立 NpcDefenseAction）→
/// 按 `category_weight(ctx) * proficiency` 加权随机选一个。全部不可用返回 None。
///
/// `category_filter` 为 Some 时，只保留该类别的功法（NpcHealAction 用 Some(Heal)）。
#[allow(clippy::too_many_arguments)]
pub fn select_technique(
    known: &KnownTechniques,
    cultivation: &Cultivation,
    meridian_deps: &SkillMeridianDependencies,
    severed: Option<&MeridianSeveredPermanent>,
    cooldowns: &NpcCooldownMap,
    npc_entity: Entity,
    _target_distance: f32,
    current_tick: u64,
    ctx: &NpcSkillScoringContext,
    category_filter: Option<SkillCategory>,
) -> Option<SelectedTechnique> {
    let mut candidates: Vec<(&KnownTechnique, &TechniqueDefinition)> = Vec::new();

    for entry in &known.entries {
        if !entry.active {
            continue;
        }
        let Some(def) = technique_definition(&entry.id) else {
            continue;
        };
        if let Some(filter) = category_filter {
            if def.category != filter {
                continue;
            }
        }
        let deps = meridian_deps.lookup(&entry.id);
        if check_meridian_dependencies(deps, severed).is_err() {
            continue;
        }
        if cooldowns.is_on_cooldown(npc_entity, &entry.id, current_tick) {
            continue;
        }
        if f64::from(def.qi_cost) > cultivation.qi_current {
            continue;
        }
        if category_filter.is_none() && def.category == SkillCategory::Defense {
            continue;
        }

        candidates.push((entry, def));
    }

    if candidates.is_empty() {
        return None;
    }

    if ctx.qi_ratio < 0.15 {
        let mut qi_costs: Vec<f32> = candidates.iter().map(|(_, def)| def.qi_cost).collect();
        qi_costs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median_idx = qi_costs.len().saturating_sub(1) / 2;
        let median_cost = qi_costs[median_idx];
        candidates.retain(|(_, def)| def.qi_cost <= median_cost);
    }

    if candidates.is_empty() {
        return None;
    }

    let total_weight: f32 = candidates
        .iter()
        .map(|(entry, def)| {
            let cw = category_weight(def.category, ctx);
            (cw * entry.proficiency).max(0.001)
        })
        .sum();
    let roll = splitmix64_unit(
        current_tick
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add(npc_entity.index() as u64),
    ) * total_weight;

    let mut accum = 0.0_f32;
    for (entry, def) in &candidates {
        let cw = category_weight(def.category, ctx);
        accum += (cw * entry.proficiency).max(0.001);
        if roll < accum {
            return Some(SelectedTechnique {
                technique_id: entry.id.clone(),
                target: skill_target_for_category(def.category),
            });
        }
    }

    candidates.last().map(|(entry, def)| SelectedTechnique {
        technique_id: entry.id.clone(),
        target: skill_target_for_category(def.category),
    })
}

// ─── NpcTechniqueScorer ──────────────────────────────────────────────────────

/// NPC 功法评分器（big-brain Scorer）。
///
/// 有可用功法 + 目标在 range 内 + qi 足够 + 经脉依赖满足 → 0.85
/// 否则 → 0.0（fallthrough 到 MeleeRangeScorer）。
///
/// 最小间隔 = `60 + realm_rank * 10` ticks（避免连续释放功法过于频繁）。
#[derive(Clone, Copy, Debug, Component)]
pub struct NpcTechniqueScorer;

impl ScorerBuilder for NpcTechniqueScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("NpcTechniqueScorer")
    }
}

/// NPC 功法释放 Action（big-brain Action）。
///
/// 声明为普通 Action component；实际执行由 `npc_technique_action_system`（exclusive
/// system）驱动，因为 `SkillFn` 需要 `&mut World`。
#[derive(Clone, Copy, Debug, Component)]
pub struct NpcTechniqueAction;

impl ActionBuilder for NpcTechniqueAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("NpcTechniqueAction")
    }
}

/// 记录 NPC 上次功法释放的 tick（用于最小间隔控制）。
#[derive(Clone, Copy, Debug, Default, Component)]
pub struct NpcLastTechniqueTick(pub u64);

// ─── NpcHealScorer ──────────────────────────────────────────────────────────

/// hp_ratio < 0.3 + has Heal technique + not on heal cooldown -> 0.9.
///
/// 在 thinker 中注册于 NpcTechniqueScorer 之前（FirstToScore 优先），
/// 触发时 paired NpcHealAction 只选 Heal 类别功法。
#[derive(Clone, Copy, Debug, Component)]
pub struct NpcHealScorer;

impl ScorerBuilder for NpcHealScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("NpcHealScorer")
    }
}

/// NPC 治疗 Action（big-brain Action）。
///
/// 与 NpcTechniqueAction 共享 exclusive action system，
/// 但 select_technique 使用 category_filter = Some(Heal)。
#[derive(Clone, Copy, Debug, Component)]
pub struct NpcHealAction;

impl ActionBuilder for NpcHealAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("NpcHealAction")
    }
}

// ─── NpcHealScorer system ────────────────────────────────────────────────────

/// hp_ratio < 0.3 + has Heal-category technique + heal not on cooldown -> 0.9.
#[allow(clippy::type_complexity)]
pub fn npc_heal_scorer_system(
    npcs: Query<
        (
            &NpcBlackboard,
            &Cultivation,
            Option<&KnownTechniques>,
            Option<&MeridianSeveredPermanent>,
            Option<&crate::combat::components::Wounds>,
            Option<&crate::npc::lod::NpcLodTier>,
        ),
        With<crate::npc::spawn::NpcMarker>,
    >,
    cooldowns: Option<Res<NpcCooldownMap>>,
    meridian_deps: Option<Res<SkillMeridianDependencies>>,
    mut scorers: Query<(&Actor, &mut Score), With<NpcHealScorer>>,
    clock: Option<Res<crate::cultivation::tick::CultivationClock>>,
    lod_config: Option<Res<crate::npc::lod::NpcLodConfig>>,
    lod_tick: Option<Res<crate::npc::lod::NpcLodTick>>,
) {
    let current_tick = clock.as_deref().map(|c| c.tick).unwrap_or(0);
    let empty_cooldowns = NpcCooldownMap::default();
    let cooldowns = cooldowns.as_deref().unwrap_or(&empty_cooldowns);
    let empty_deps = SkillMeridianDependencies::default();
    let deps = meridian_deps.as_deref().unwrap_or(&empty_deps);
    let cfg = lod_config.as_deref().cloned().unwrap_or_default();
    let tick = lod_tick.as_deref().map(|t| t.0).unwrap_or(0);

    for (Actor(actor), mut score) in &mut scorers {
        let Ok((_bb, cultivation, known_opt, severed_opt, wounds_opt, tier)) = npcs.get(*actor)
        else {
            score.set(0.0);
            continue;
        };

        let value = match crate::npc::lod::lod_gated_score(tier, tick, &cfg, || {
            let hp_ratio = wounds_opt
                .map(|w| {
                    if w.health_max > 0.0 {
                        w.health_current / w.health_max
                    } else {
                        1.0
                    }
                })
                .unwrap_or(1.0);

            if hp_ratio >= 0.3 {
                return 0.0;
            }

            let Some(known) = known_opt else {
                return 0.0;
            };

            let has_usable_heal = known.entries.iter().any(|entry| {
                if !entry.active {
                    return false;
                }
                let Some(def) = technique_definition(&entry.id) else {
                    return false;
                };
                if def.category != SkillCategory::Heal {
                    return false;
                }
                if check_meridian_dependencies(deps.lookup(&entry.id), severed_opt).is_err() {
                    return false;
                }
                if cooldowns.is_on_cooldown(*actor, &entry.id, current_tick) {
                    return false;
                }
                if f64::from(def.qi_cost) > cultivation.qi_current {
                    return false;
                }
                true
            });

            if has_usable_heal {
                0.9
            } else {
                0.0
            }
        }) {
            Some(v) => v,
            None => continue,
        };

        score.set(value);
    }
}

/// 检查 NPC 是否有可用的 Heal 类别功法（供 unit test 使用）。
pub fn has_usable_heal_technique(
    known: &KnownTechniques,
    cultivation: &Cultivation,
    deps: &SkillMeridianDependencies,
    severed: Option<&MeridianSeveredPermanent>,
    cooldowns: &NpcCooldownMap,
    npc_entity: Entity,
    current_tick: u64,
) -> bool {
    known.entries.iter().any(|entry| {
        if !entry.active {
            return false;
        }
        let Some(def) = technique_definition(&entry.id) else {
            return false;
        };
        if def.category != SkillCategory::Heal {
            return false;
        }
        if check_meridian_dependencies(deps.lookup(&entry.id), severed).is_err() {
            return false;
        }
        if cooldowns.is_on_cooldown(npc_entity, &entry.id, current_tick) {
            return false;
        }
        if f64::from(def.qi_cost) > cultivation.qi_current {
            return false;
        }
        true
    })
}

// ─── Scorer system ───────────────────────────────────────────────────────────

pub fn build_npc_skill_scoring_context(
    cultivation: &Cultivation,
    wounds: Option<&crate::combat::components::Wounds>,
    bb: &NpcBlackboard,
) -> NpcSkillScoringContext {
    let hp_ratio = wounds
        .map(|w| {
            if w.health_max > 0.0 {
                w.health_current / w.health_max
            } else {
                1.0
            }
        })
        .unwrap_or(1.0);
    let qi_ratio = if cultivation.qi_max > 0.0 {
        (cultivation.qi_current / cultivation.qi_max) as f32
    } else {
        1.0
    };
    NpcSkillScoringContext {
        hp_ratio,
        qi_ratio,
        target_distance: bb.player_distance,
        target_hp_ratio: 1.0,
        has_active_buff: false,
        in_combat: bb.nearest_player.is_some(),
    }
}

#[allow(clippy::type_complexity)]
pub fn npc_technique_scorer_system(
    npcs: Query<
        (
            &NpcBlackboard,
            &Cultivation,
            Option<&KnownTechniques>,
            Option<&NpcLastTechniqueTick>,
            Option<&MeridianSeveredPermanent>,
            Option<&crate::combat::components::Wounds>,
            Option<&crate::npc::lod::NpcLodTier>,
        ),
        With<crate::npc::spawn::NpcMarker>,
    >,
    cooldowns: Option<Res<NpcCooldownMap>>,
    meridian_deps: Option<Res<SkillMeridianDependencies>>,
    mut scorers: Query<(&Actor, &mut Score), With<NpcTechniqueScorer>>,
    clock: Option<Res<crate::cultivation::tick::CultivationClock>>,
    lod_config: Option<Res<crate::npc::lod::NpcLodConfig>>,
    lod_tick: Option<Res<crate::npc::lod::NpcLodTick>>,
) {
    let current_tick = clock.as_deref().map(|c| c.tick).unwrap_or(0);
    let empty_cooldowns = NpcCooldownMap::default();
    let cooldowns = cooldowns.as_deref().unwrap_or(&empty_cooldowns);
    let empty_deps = SkillMeridianDependencies::default();
    let deps = meridian_deps.as_deref().unwrap_or(&empty_deps);
    let cfg = lod_config.as_deref().cloned().unwrap_or_default();
    let lod_t = lod_tick.as_deref().map(|t| t.0).unwrap_or(0);

    for (Actor(actor), mut score) in &mut scorers {
        let Ok((bb, cultivation, known_opt, last_tick_opt, severed_opt, wounds_opt, tier)) =
            npcs.get(*actor)
        else {
            score.set(0.0);
            continue;
        };

        let value = match crate::npc::lod::lod_gated_score(tier, lod_t, &cfg, || {
            let Some(known) = known_opt else {
                return 0.0;
            };

            if known.entries.is_empty() {
                return 0.0;
            }

            let min_interval = 60 + realm_rank(cultivation.realm) as u64 * 10;
            let last_tick = last_tick_opt.map(|t| t.0).unwrap_or(0);
            if current_tick > 0 && current_tick.saturating_sub(last_tick) < min_interval {
                return 0.0;
            }

            if bb.nearest_player.is_none() {
                return 0.0;
            }

            let ctx = build_npc_skill_scoring_context(cultivation, wounds_opt, bb);

            let has_usable = select_technique(
                known,
                cultivation,
                deps,
                severed_opt,
                cooldowns,
                *actor,
                bb.player_distance,
                current_tick,
                &ctx,
                None,
            )
            .is_some();

            if has_usable {
                0.85
            } else {
                0.0
            }
        }) {
            Some(v) => v,
            None => continue,
        };

        score.set(value);
    }
}

// ─── Action system (exclusive) ───────────────────────────────────────────────

/// Exclusive system 驱动 NpcTechniqueAction（通用功法释放，category_filter = None）。
pub fn npc_technique_action_system(world: &mut valence::prelude::bevy_ecs::world::World) {
    run_technique_action::<NpcTechniqueAction>(world, None);
}

/// Exclusive system 驱动 NpcHealAction（category_filter = Some(Heal)）。
pub fn npc_heal_action_system(world: &mut valence::prelude::bevy_ecs::world::World) {
    run_technique_action::<NpcHealAction>(world, Some(SkillCategory::Heal));
}

/// 通用功法 Action 驱动逻辑。
///
/// `T` 为 Action marker component（NpcTechniqueAction 或 NpcHealAction）。
/// `category_filter` 为 Some 时只选该类别功法。
fn run_technique_action<T: Component>(
    world: &mut valence::prelude::bevy_ecs::world::World,
    category_filter: Option<SkillCategory>,
) {
    use crate::cultivation::skill_registry::{CastResult, SkillRegistry};

    let mut actions_to_process: Vec<(Entity, Entity, ActionState)> = Vec::new();
    {
        let mut query = world.query_filtered::<(Entity, &Actor, &ActionState), With<T>>();
        for (action_entity, actor, state) in query.iter(world) {
            actions_to_process.push((action_entity, actor.0, state.clone()));
        }
    }

    for (action_entity, actor_entity, state) in actions_to_process {
        match state {
            ActionState::Requested => {
                let selected = {
                    let Some(known) = world.get::<KnownTechniques>(actor_entity) else {
                        set_action_state(world, action_entity, ActionState::Failure);
                        continue;
                    };
                    let Some(cultivation) = world.get::<Cultivation>(actor_entity) else {
                        set_action_state(world, action_entity, ActionState::Failure);
                        continue;
                    };
                    let severed = world.get::<MeridianSeveredPermanent>(actor_entity);

                    let empty_deps = SkillMeridianDependencies::default();
                    let deps = world
                        .get_resource::<SkillMeridianDependencies>()
                        .unwrap_or(&empty_deps);
                    let empty_cooldowns = NpcCooldownMap::default();
                    let cooldowns = world
                        .get_resource::<NpcCooldownMap>()
                        .unwrap_or(&empty_cooldowns);

                    let bb = world.get::<NpcBlackboard>(actor_entity);
                    let target_distance = bb.map(|b| b.player_distance).unwrap_or(f32::INFINITY);
                    let clock = world.get_resource::<crate::cultivation::tick::CultivationClock>();
                    let current_tick = clock.map(|c| c.tick).unwrap_or(0);

                    let default_bb = NpcBlackboard::default();
                    let bb_ref = bb.unwrap_or(&default_bb);
                    let wounds = world.get::<crate::combat::components::Wounds>(actor_entity);
                    let ctx = build_npc_skill_scoring_context(cultivation, wounds, bb_ref);

                    match select_technique(
                        known,
                        cultivation,
                        deps,
                        severed,
                        cooldowns,
                        actor_entity,
                        target_distance,
                        current_tick,
                        &ctx,
                        category_filter,
                    ) {
                        Some(sel) => sel,
                        None => {
                            set_action_state(world, action_entity, ActionState::Failure);
                            continue;
                        }
                    }
                };

                let technique_id = selected.technique_id.clone();

                let skill_fn = {
                    let Some(registry) = world.get_resource::<SkillRegistry>() else {
                        set_action_state(world, action_entity, ActionState::Failure);
                        continue;
                    };
                    registry.lookup(&technique_id)
                };

                let Some(skill_fn) = skill_fn else {
                    set_action_state(world, action_entity, ActionState::Failure);
                    continue;
                };

                let target = match selected.target {
                    SkillTarget::SelfCast => Some(actor_entity),
                    SkillTarget::NearestEnemy => world
                        .get::<NpcBlackboard>(actor_entity)
                        .and_then(|bb| bb.nearest_player),
                };

                let result = skill_fn(world, actor_entity, 0, target);

                match result {
                    CastResult::Started {
                        cooldown_ticks: cd, ..
                    } => {
                        let clock =
                            world.get_resource::<crate::cultivation::tick::CultivationClock>();
                        let current_tick = clock.map(|c| c.tick).unwrap_or(0);

                        if let Some(cooldowns) = world.get_resource_mut::<NpcCooldownMap>() {
                            let mut cooldowns = cooldowns;
                            cooldowns.set(actor_entity, &technique_id, current_tick + cd);
                        }

                        if let Some(mut last_tick) =
                            world.get_mut::<NpcLastTechniqueTick>(actor_entity)
                        {
                            last_tick.0 = current_tick;
                        }

                        set_action_state(world, action_entity, ActionState::Success);
                    }
                    CastResult::Rejected { .. } | CastResult::Interrupted => {
                        set_action_state(world, action_entity, ActionState::Failure);
                    }
                }
            }
            ActionState::Cancelled => {
                set_action_state(world, action_entity, ActionState::Failure);
            }
            ActionState::Init
            | ActionState::Executing
            | ActionState::Success
            | ActionState::Failure => {}
        }
    }
}

fn set_action_state(
    world: &mut valence::prelude::bevy_ecs::world::World,
    entity: Entity,
    state: ActionState,
) {
    if let Some(mut current) = world.get_mut::<ActionState>(entity) {
        *current = state;
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::{Cultivation, MeridianId, MeridianSystem};
    use crate::cultivation::known_techniques::{KnownTechnique, KnownTechniques};
    use crate::cultivation::meridian::severed::{
        MeridianSeveredPermanent, SeveredSource, SkillMeridianDependencies,
    };
    use crate::npc::lifecycle::NpcArchetype;

    /// 创建一个有 N 条经脉已开的 MeridianSystem（方便测试）。
    fn meridian_sys_with_opened(ids: &[MeridianId]) -> MeridianSystem {
        let mut sys = MeridianSystem::default();
        for id in ids {
            let m = sys.get_mut(*id);
            m.opened = true;
            m.integrity = 1.0;
            m.throughput_current = 1.0;
        }
        sys
    }

    /// 全 12 正经已开的 MeridianSystem。
    fn full_regular_meridians() -> MeridianSystem {
        use MeridianId::*;
        meridian_sys_with_opened(&[
            Lung,
            LargeIntestine,
            Stomach,
            Spleen,
            Heart,
            SmallIntestine,
            Bladder,
            Kidney,
            Pericardium,
            TripleEnergizer,
            Gallbladder,
            Liver,
        ])
    }

    /// 全 20 条经脉已开的 MeridianSystem。
    fn full_all_meridians() -> MeridianSystem {
        use MeridianId::*;
        meridian_sys_with_opened(&[
            Lung,
            LargeIntestine,
            Stomach,
            Spleen,
            Heart,
            SmallIntestine,
            Bladder,
            Kidney,
            Pericardium,
            TripleEnergizer,
            Gallbladder,
            Liver,
            Ren,
            Du,
            Chong,
            Dai,
            YinQiao,
            YangQiao,
            YinWei,
            YangWei,
        ])
    }

    fn empty_deps() -> SkillMeridianDependencies {
        SkillMeridianDependencies::default()
    }

    fn default_ctx() -> NpcSkillScoringContext {
        NpcSkillScoringContext {
            hp_ratio: 1.0,
            qi_ratio: 1.0,
            target_distance: 3.0,
            target_hp_ratio: 1.0,
            has_active_buff: false,
            in_combat: true,
        }
    }

    // === assign_npc_techniques: archetype coverage ===

    #[test]
    fn assign_commoner_returns_empty() {
        let sys = MeridianSystem::default();
        let deps = empty_deps();
        let kt =
            assign_npc_techniques(NpcArchetype::Commoner, Realm::Awaken, &sys, &deps, None, 42);
        assert!(kt.entries.is_empty(), "commoner should have no techniques");
    }

    #[test]
    fn assign_beast_returns_empty() {
        let sys = full_regular_meridians();
        let deps = empty_deps();
        let kt = assign_npc_techniques(NpcArchetype::Beast, Realm::Condense, &sys, &deps, None, 42);
        assert!(kt.entries.is_empty(), "beast should have no techniques");
    }

    #[test]
    fn assign_skull_fiend_returns_empty() {
        let sys = full_regular_meridians();
        let deps = empty_deps();
        let kt =
            assign_npc_techniques(NpcArchetype::SkullFiend, Realm::Void, &sys, &deps, None, 42);
        assert!(
            kt.entries.is_empty(),
            "skull fiend should have no techniques"
        );
    }

    #[test]
    fn assign_fuya_returns_empty() {
        let sys = full_regular_meridians();
        let deps = empty_deps();
        let kt = assign_npc_techniques(NpcArchetype::Fuya, Realm::Spirit, &sys, &deps, None, 42);
        assert!(kt.entries.is_empty(), "fuya should have no techniques");
    }

    #[test]
    fn assign_zombie_returns_empty() {
        let sys = full_regular_meridians();
        let deps = empty_deps();
        let kt = assign_npc_techniques(NpcArchetype::Zombie, Realm::Awaken, &sys, &deps, None, 42);
        assert!(kt.entries.is_empty(), "zombie should have no techniques");
    }

    #[test]
    fn assign_rogue_awaken_returns_1_to_3() {
        let sys = full_regular_meridians();
        let deps = empty_deps();
        // Run multiple seeds to hit different counts
        for seed in 0..50u64 {
            let kt =
                assign_npc_techniques(NpcArchetype::Rogue, Realm::Awaken, &sys, &deps, None, seed);
            assert!(
                !kt.entries.is_empty() && kt.entries.len() <= 3,
                "rogue awaken should have 1-3 techniques, got {} (seed={})",
                kt.entries.len(),
                seed
            );
            for entry in &kt.entries {
                assert!(
                    entry.proficiency >= 0.2 && entry.proficiency <= 0.7,
                    "rogue proficiency should be 0.2-0.7, got {} for {} (seed={})",
                    entry.proficiency,
                    entry.id,
                    seed
                );
                assert!(entry.active, "assigned techniques should be active");
            }
        }
    }

    #[test]
    fn assign_disciple_returns_2_to_6() {
        let sys = full_regular_meridians();
        let deps = empty_deps();
        for seed in 0..50u64 {
            let kt = assign_npc_techniques(
                NpcArchetype::Disciple,
                Realm::Condense,
                &sys,
                &deps,
                None,
                seed * 7,
            );
            assert!(
                kt.entries.len() >= 2 && kt.entries.len() <= 6,
                "disciple Condense should have 2-6 techniques (base 2-4 + heal + buff), got {} (seed={})",
                kt.entries.len(),
                seed
            );
            for entry in &kt.entries {
                assert!(
                    entry.proficiency >= 0.3 && entry.proficiency <= 0.8,
                    "disciple proficiency should be 0.3-0.8, got {}",
                    entry.proficiency
                );
            }
        }
    }

    #[test]
    fn assign_guardian_relic_returns_3_to_7() {
        let sys = full_all_meridians();
        let deps = empty_deps();
        for seed in 0..50u64 {
            let kt = assign_npc_techniques(
                NpcArchetype::GuardianRelic,
                Realm::Spirit,
                &sys,
                &deps,
                None,
                seed * 13,
            );
            assert!(
                kt.entries.len() >= 3 && kt.entries.len() <= 7,
                "guardian relic Spirit should have 3-7 techniques (base 3-5 + heal + buff), got {} (seed={})",
                kt.entries.len(),
                seed
            );
            for entry in &kt.entries {
                assert!(
                    entry.proficiency >= 0.6 && entry.proficiency <= 0.9,
                    "guardian relic proficiency should be 0.6-0.9, got {}",
                    entry.proficiency
                );
            }
        }
    }

    #[test]
    fn assign_daoxiang_returns_1_to_3() {
        let sys = full_regular_meridians();
        let deps = empty_deps();
        for seed in 0..50u64 {
            let kt = assign_npc_techniques(
                NpcArchetype::Daoxiang,
                Realm::Induce,
                &sys,
                &deps,
                None,
                seed * 17,
            );
            assert!(
                !kt.entries.is_empty() && kt.entries.len() <= 3,
                "daoxiang Induce should have 1-3 techniques (base 1-2 + heal), got {} (seed={})",
                kt.entries.len(),
                seed
            );
            for entry in &kt.entries {
                assert!(
                    entry.proficiency >= 0.1 && entry.proficiency <= 0.4,
                    "daoxiang proficiency should be 0.1-0.4, got {}",
                    entry.proficiency
                );
            }
        }
    }

    #[test]
    fn assign_zhinian_returns_2_to_5() {
        let sys = full_regular_meridians();
        let deps = empty_deps();
        for seed in 0..50u64 {
            let kt = assign_npc_techniques(
                NpcArchetype::Zhinian,
                Realm::Condense,
                &sys,
                &deps,
                None,
                seed * 19,
            );
            assert!(
                kt.entries.len() >= 2 && kt.entries.len() <= 5,
                "zhinian Condense should have 2-5 techniques (base 2-3 + heal + buff), got {} (seed={})",
                kt.entries.len(),
                seed
            );
            for entry in &kt.entries {
                assert!(
                    entry.proficiency >= 0.3 && entry.proficiency <= 0.6,
                    "zhinian proficiency should be 0.3-0.6, got {}",
                    entry.proficiency
                );
            }
        }
    }

    // === assign_npc_techniques: realm gating ===

    #[test]
    fn realm_too_low_excludes_techniques() {
        // Awaken NPC should not get Induce+ techniques
        let sys = full_regular_meridians();
        let deps = empty_deps();
        for seed in 0..100u64 {
            let kt =
                assign_npc_techniques(NpcArchetype::Rogue, Realm::Awaken, &sys, &deps, None, seed);
            for entry in &kt.entries {
                let def = technique_definition(&entry.id).expect("valid technique");
                let required = parse_realm(def.required_realm).unwrap();
                assert!(
                    realm_rank(required) <= realm_rank(Realm::Awaken),
                    "Awaken NPC should not have technique {} requiring {:?}",
                    entry.id,
                    def.required_realm
                );
            }
        }
    }

    #[test]
    fn no_meridians_opened_returns_empty_for_meridian_gated_techniques() {
        // MeridianSystem default = all closed
        let sys = MeridianSystem::default();
        let deps = empty_deps();
        // Even Induce rogue — techniques requiring meridians should be excluded
        let kt = assign_npc_techniques(NpcArchetype::Rogue, Realm::Induce, &sys, &deps, None, 42);
        for entry in &kt.entries {
            let def = technique_definition(&entry.id).expect("valid technique");
            assert!(
                def.required_meridians.is_empty(),
                "technique {} requires meridians but NPC has none opened — should have been filtered",
                entry.id
            );
        }
    }

    // === assign_npc_techniques: P1.4 NPC utility skill injection ===

    #[test]
    fn assign_induce_rogue_always_has_heal_basic() {
        let sys = full_regular_meridians();
        let deps = empty_deps();
        for seed in 0..100u64 {
            let kt =
                assign_npc_techniques(NpcArchetype::Rogue, Realm::Induce, &sys, &deps, None, seed);
            assert!(
                kt.entries.iter().any(|e| e.id == "npc.heal_basic"),
                "Induce+ Rogue should always have npc.heal_basic (seed={})",
                seed
            );
        }
    }

    #[test]
    fn assign_condense_rogue_always_has_heal_and_buff() {
        let sys = full_regular_meridians();
        let deps = empty_deps();
        for seed in 0..100u64 {
            let kt = assign_npc_techniques(
                NpcArchetype::Rogue,
                Realm::Condense,
                &sys,
                &deps,
                None,
                seed,
            );
            assert!(
                kt.entries.iter().any(|e| e.id == "npc.heal_basic"),
                "Condense+ Rogue should always have npc.heal_basic (seed={})",
                seed
            );
            let has_buff = kt
                .entries
                .iter()
                .any(|e| e.id == "npc.buff_speed" || e.id == "npc.buff_defense");
            assert!(
                has_buff,
                "Condense+ Rogue should always have npc.buff_speed or npc.buff_defense (seed={})",
                seed
            );
        }
    }

    #[test]
    fn assign_awaken_rogue_never_has_npc_utility_skills() {
        let sys = full_regular_meridians();
        let deps = empty_deps();
        for seed in 0..100u64 {
            let kt =
                assign_npc_techniques(NpcArchetype::Rogue, Realm::Awaken, &sys, &deps, None, seed);
            for entry in &kt.entries {
                assert!(
                    !entry.id.starts_with("npc."),
                    "Awaken NPC should not have NPC utility skill {}, got it at seed={}",
                    entry.id,
                    seed
                );
            }
        }
    }

    #[test]
    fn assign_npc_utility_skills_no_duplicates() {
        let sys = full_regular_meridians();
        let deps = empty_deps();
        for seed in 0..200u64 {
            let kt = assign_npc_techniques(
                NpcArchetype::Disciple,
                Realm::Condense,
                &sys,
                &deps,
                None,
                seed,
            );
            let ids: Vec<&str> = kt.entries.iter().map(|e| e.id.as_str()).collect();
            let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
            assert_eq!(
                ids.len(),
                unique.len(),
                "no duplicate technique IDs should exist, got {:?} (seed={})",
                ids,
                seed
            );
        }
    }

    #[test]
    fn assign_npc_heal_blocked_when_meridians_closed() {
        let sys = MeridianSystem::default();
        let deps = empty_deps();
        for seed in 0..50u64 {
            let kt =
                assign_npc_techniques(NpcArchetype::Rogue, Realm::Induce, &sys, &deps, None, seed);
            assert!(
                !kt.entries.iter().any(|e| e.id == "npc.heal_basic"),
                "npc.heal_basic requires Spleen+Kidney meridians — should not appear with all closed (seed={})",
                seed
            );
        }
    }

    #[test]
    fn assign_buff_is_speed_or_defense_deterministic_per_seed() {
        let sys = full_regular_meridians();
        let deps = empty_deps();
        for seed in 0..100u64 {
            let a = assign_npc_techniques(
                NpcArchetype::Rogue,
                Realm::Condense,
                &sys,
                &deps,
                None,
                seed,
            );
            let b = assign_npc_techniques(
                NpcArchetype::Rogue,
                Realm::Condense,
                &sys,
                &deps,
                None,
                seed,
            );
            let buff_a: Vec<&str> = a
                .entries
                .iter()
                .filter(|e| e.id == "npc.buff_speed" || e.id == "npc.buff_defense")
                .map(|e| e.id.as_str())
                .collect();
            let buff_b: Vec<&str> = b
                .entries
                .iter()
                .filter(|e| e.id == "npc.buff_speed" || e.id == "npc.buff_defense")
                .map(|e| e.id.as_str())
                .collect();
            assert_eq!(
                buff_a, buff_b,
                "same seed should pick same buff variant (seed={})",
                seed
            );
        }
    }

    #[test]
    fn assign_all_combat_archetypes_get_heal_at_induce() {
        let sys = full_regular_meridians();
        let deps = empty_deps();
        for archetype in [
            NpcArchetype::Rogue,
            NpcArchetype::Disciple,
            NpcArchetype::GuardianRelic,
            NpcArchetype::Daoxiang,
            NpcArchetype::Zhinian,
        ] {
            let kt = assign_npc_techniques(archetype, Realm::Induce, &sys, &deps, None, 42);
            assert!(
                kt.entries.iter().any(|e| e.id == "npc.heal_basic"),
                "{:?} at Induce should have npc.heal_basic",
                archetype
            );
        }
    }

    #[test]
    fn assign_non_combat_archetypes_never_get_npc_skills() {
        let sys = full_regular_meridians();
        let deps = empty_deps();
        for archetype in [
            NpcArchetype::Commoner,
            NpcArchetype::Beast,
            NpcArchetype::SkullFiend,
            NpcArchetype::Fuya,
            NpcArchetype::Zombie,
        ] {
            let kt = assign_npc_techniques(archetype, Realm::Void, &sys, &deps, None, 42);
            assert!(
                kt.entries.is_empty(),
                "{:?} should have no techniques even at Void",
                archetype
            );
        }
    }

    // === assign_npc_techniques: determinism ===

    #[test]
    fn assign_techniques_deterministic() {
        let sys = full_regular_meridians();
        let deps = empty_deps();
        for archetype in [
            NpcArchetype::Rogue,
            NpcArchetype::Disciple,
            NpcArchetype::GuardianRelic,
            NpcArchetype::Daoxiang,
            NpcArchetype::Zhinian,
        ] {
            let a = assign_npc_techniques(archetype, Realm::Condense, &sys, &deps, None, 12345);
            let b = assign_npc_techniques(archetype, Realm::Condense, &sys, &deps, None, 12345);
            assert_eq!(
                a, b,
                "same seed should produce identical techniques for {:?}",
                archetype
            );
        }
    }

    // === select_technique: basic ===

    #[test]
    fn select_technique_with_available_returns_some() {
        let known = KnownTechniques {
            entries: vec![KnownTechnique {
                id: "sword.cleave".to_string(),
                proficiency: 0.5,
                active: true,
            }],
        };
        let cultivation = Cultivation {
            realm: Realm::Awaken,
            qi_current: 100.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let deps = empty_deps();
        let cooldowns = NpcCooldownMap::default();
        let entity = Entity::from_raw(1);

        let result = select_technique(
            &known,
            &cultivation,
            &deps,
            None,
            &cooldowns,
            entity,
            3.0,
            100,
            &default_ctx(),
            None,
        );
        assert!(result.is_some(), "should select a technique");
        let sel = result.unwrap();
        assert_eq!(sel.technique_id, "sword.cleave");
        assert_eq!(sel.target, SkillTarget::NearestEnemy);
    }

    #[test]
    fn select_technique_inactive_excluded() {
        let known = KnownTechniques {
            entries: vec![KnownTechnique {
                id: "sword.cleave".to_string(),
                proficiency: 0.5,
                active: false,
            }],
        };
        let cultivation = Cultivation {
            realm: Realm::Awaken,
            qi_current: 100.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let deps = empty_deps();
        let cooldowns = NpcCooldownMap::default();
        let entity = Entity::from_raw(1);

        let result = select_technique(
            &known,
            &cultivation,
            &deps,
            None,
            &cooldowns,
            entity,
            3.0,
            100,
            &default_ctx(),
            None,
        );
        assert!(result.is_none(), "inactive technique should be excluded");
    }

    #[test]
    fn select_technique_on_cooldown_excluded() {
        let known = KnownTechniques {
            entries: vec![KnownTechnique {
                id: "sword.cleave".to_string(),
                proficiency: 0.5,
                active: true,
            }],
        };
        let cultivation = Cultivation {
            realm: Realm::Awaken,
            qi_current: 100.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let deps = empty_deps();
        let mut cooldowns = NpcCooldownMap::default();
        let entity = Entity::from_raw(1);
        cooldowns.set(entity, "sword.cleave", 200); // on CD until tick 200

        let result = select_technique(
            &known,
            &cultivation,
            &deps,
            None,
            &cooldowns,
            entity,
            3.0,
            100,
            &default_ctx(),
            None,
        );
        assert!(result.is_none(), "technique on cooldown should be excluded");
    }

    #[test]
    fn select_technique_cooldown_expired_available() {
        let known = KnownTechniques {
            entries: vec![KnownTechnique {
                id: "sword.cleave".to_string(),
                proficiency: 0.5,
                active: true,
            }],
        };
        let cultivation = Cultivation {
            realm: Realm::Awaken,
            qi_current: 100.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let deps = empty_deps();
        let mut cooldowns = NpcCooldownMap::default();
        let entity = Entity::from_raw(1);
        cooldowns.set(entity, "sword.cleave", 100); // expired at tick 100

        let result = select_technique(
            &known,
            &cultivation,
            &deps,
            None,
            &cooldowns,
            entity,
            3.0,
            100,
            &default_ctx(),
            None,
        );
        assert!(result.is_some(), "expired cooldown should allow technique");
    }

    #[test]
    fn select_technique_qi_insufficient_excluded() {
        let known = KnownTechniques {
            entries: vec![KnownTechnique {
                id: "burst_meridian.beng_quan".to_string(), // qi_cost = 0.4
                proficiency: 0.5,
                active: true,
            }],
        };
        let cultivation = Cultivation {
            realm: Realm::Induce,
            qi_current: 0.1, // insufficient
            qi_max: 100.0,
            ..Default::default()
        };
        let deps = empty_deps();
        let cooldowns = NpcCooldownMap::default();
        let entity = Entity::from_raw(1);

        let result = select_technique(
            &known,
            &cultivation,
            &deps,
            None,
            &cooldowns,
            entity,
            3.0,
            100,
            &default_ctx(),
            None,
        );
        assert!(
            result.is_none(),
            "technique with qi_cost > qi_current should be excluded"
        );
    }

    #[test]
    fn select_technique_severed_meridian_excluded() {
        let known = KnownTechniques {
            entries: vec![KnownTechnique {
                id: "woliu.burst".to_string(),
                proficiency: 0.5,
                active: true,
            }],
        };
        let cultivation = Cultivation {
            realm: Realm::Condense,
            qi_current: 100.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let mut deps = SkillMeridianDependencies::default();
        deps.declare("woliu.burst", vec![MeridianId::Lung]);

        let mut severed = MeridianSeveredPermanent::default();
        severed.insert(MeridianId::Lung, SeveredSource::CombatWound, 50);

        let cooldowns = NpcCooldownMap::default();
        let entity = Entity::from_raw(1);

        let result = select_technique(
            &known,
            &cultivation,
            &deps,
            Some(&severed),
            &cooldowns,
            entity,
            3.0,
            100,
            &default_ctx(),
            None,
        );
        assert!(
            result.is_none(),
            "technique with SEVERED dependent meridian should be excluded"
        );
    }

    #[test]
    fn select_technique_all_on_cooldown_returns_none() {
        let known = KnownTechniques {
            entries: vec![
                KnownTechnique {
                    id: "sword.cleave".to_string(),
                    proficiency: 0.5,
                    active: true,
                },
                KnownTechnique {
                    id: "sword.thrust".to_string(),
                    proficiency: 0.5,
                    active: true,
                },
            ],
        };
        let cultivation = Cultivation {
            realm: Realm::Awaken,
            qi_current: 100.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let deps = empty_deps();
        let mut cooldowns = NpcCooldownMap::default();
        let entity = Entity::from_raw(1);
        cooldowns.set(entity, "sword.cleave", 200);
        cooldowns.set(entity, "sword.thrust", 200);

        let result = select_technique(
            &known,
            &cultivation,
            &deps,
            None,
            &cooldowns,
            entity,
            3.0,
            100,
            &default_ctx(),
            None,
        );
        assert!(
            result.is_none(),
            "all techniques on cooldown should return None"
        );
    }

    #[test]
    fn select_technique_empty_known_returns_none() {
        let known = KnownTechniques {
            entries: Vec::new(),
        };
        let cultivation = Cultivation::default();
        let deps = empty_deps();
        let cooldowns = NpcCooldownMap::default();
        let entity = Entity::from_raw(1);

        let result = select_technique(
            &known,
            &cultivation,
            &deps,
            None,
            &cooldowns,
            entity,
            3.0,
            100,
            &default_ctx(),
            None,
        );
        assert!(
            result.is_none(),
            "empty known techniques should return None"
        );
    }

    // === NpcCooldownMap ===

    #[test]
    fn cooldown_map_set_and_check() {
        let mut map = NpcCooldownMap::default();
        let entity = Entity::from_raw(1);
        map.set(entity, "sword.cleave", 100);

        assert!(map.is_on_cooldown(entity, "sword.cleave", 50));
        assert!(map.is_on_cooldown(entity, "sword.cleave", 99));
        assert!(!map.is_on_cooldown(entity, "sword.cleave", 100));
        assert!(!map.is_on_cooldown(entity, "sword.cleave", 101));
    }

    #[test]
    fn cooldown_map_different_entities_independent() {
        let mut map = NpcCooldownMap::default();
        let e1 = Entity::from_raw(1);
        let e2 = Entity::from_raw(2);
        map.set(e1, "sword.cleave", 100);

        assert!(map.is_on_cooldown(e1, "sword.cleave", 50));
        assert!(
            !map.is_on_cooldown(e2, "sword.cleave", 50),
            "cooldown for e1 should not affect e2"
        );
    }

    #[test]
    fn cooldown_map_remove_all_for_entity() {
        let mut map = NpcCooldownMap::default();
        let e1 = Entity::from_raw(1);
        let e2 = Entity::from_raw(2);
        map.set(e1, "sword.cleave", 100);
        map.set(e1, "sword.thrust", 200);
        map.set(e2, "sword.cleave", 150);

        map.remove_all_for(e1);
        assert!(!map.is_on_cooldown(e1, "sword.cleave", 50));
        assert!(!map.is_on_cooldown(e1, "sword.thrust", 50));
        assert!(
            map.is_on_cooldown(e2, "sword.cleave", 50),
            "removing e1 entries should not affect e2"
        );
    }

    #[test]
    fn cooldown_map_overwrite() {
        let mut map = NpcCooldownMap::default();
        let entity = Entity::from_raw(1);
        map.set(entity, "sword.cleave", 100);
        map.set(entity, "sword.cleave", 200); // overwrite

        assert!(map.is_on_cooldown(entity, "sword.cleave", 150));
        assert!(!map.is_on_cooldown(entity, "sword.cleave", 200));
    }

    #[test]
    fn cooldown_map_empty_check() {
        let map = NpcCooldownMap::default();
        let entity = Entity::from_raw(1);
        assert!(
            !map.is_on_cooldown(entity, "sword.cleave", 0),
            "empty map should not report cooldown"
        );
    }

    // === select_technique: weighted random ===

    #[test]
    fn select_technique_higher_proficiency_more_likely() {
        let known = KnownTechniques {
            entries: vec![
                KnownTechnique {
                    id: "sword.cleave".to_string(),
                    proficiency: 0.01, // very low
                    active: true,
                },
                KnownTechnique {
                    id: "sword.thrust".to_string(),
                    proficiency: 0.99, // very high
                    active: true,
                },
            ],
        };
        let cultivation = Cultivation {
            realm: Realm::Awaken,
            qi_current: 100.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let deps = empty_deps();
        let cooldowns = NpcCooldownMap::default();
        let entity = Entity::from_raw(1);

        let mut thrust_count = 0;
        for tick in 0..1000u64 {
            if let Some(sel) = select_technique(
                &known,
                &cultivation,
                &deps,
                None,
                &cooldowns,
                entity,
                3.0,
                tick,
                &default_ctx(),
                None,
            ) {
                if sel.technique_id == "sword.thrust" {
                    thrust_count += 1;
                }
            }
        }
        // sword.thrust with 0.99 should be selected much more than sword.cleave with 0.01
        assert!(
            thrust_count > 800,
            "high proficiency technique should be selected >80% of the time, got {}/1000",
            thrust_count
        );
    }

    // === meridian_deps_satisfied ===

    #[test]
    fn meridian_deps_satisfied_no_deps() {
        let def = technique_definition("sword.cleave").unwrap();
        let sys = MeridianSystem::default();
        let deps = empty_deps();
        assert!(
            meridian_deps_satisfied(def, &sys, &deps),
            "technique with no meridian deps should pass"
        );
    }

    #[test]
    fn meridian_deps_satisfied_with_opened() {
        let def = technique_definition("zhenmai.parry").unwrap(); // requires Lung
        let sys = meridian_sys_with_opened(&[MeridianId::Lung]);
        let deps = empty_deps();
        assert!(
            meridian_deps_satisfied(def, &sys, &deps),
            "technique with opened required meridian should pass"
        );
    }

    #[test]
    fn meridian_deps_not_satisfied_when_closed() {
        let def = technique_definition("zhenmai.parry").unwrap(); // requires Lung
        let sys = MeridianSystem::default(); // all closed
        let deps = empty_deps();
        assert!(
            !meridian_deps_satisfied(def, &sys, &deps),
            "technique with closed required meridian should fail"
        );
    }

    // === assign_npc_techniques: all archetypes x all realms ===

    #[test]
    fn assign_all_archetypes_all_realms_valid() {
        let all_archetypes = [
            NpcArchetype::Zombie,
            NpcArchetype::Commoner,
            NpcArchetype::Rogue,
            NpcArchetype::Beast,
            NpcArchetype::Disciple,
            NpcArchetype::GuardianRelic,
            NpcArchetype::Daoxiang,
            NpcArchetype::Zhinian,
            NpcArchetype::Fuya,
            NpcArchetype::SkullFiend,
        ];
        let all_realms = [
            Realm::Awaken,
            Realm::Induce,
            Realm::Condense,
            Realm::Solidify,
            Realm::Spirit,
            Realm::Void,
        ];
        let sys = full_all_meridians();
        let deps = empty_deps();

        for archetype in all_archetypes {
            for realm in all_realms {
                let kt = assign_npc_techniques(archetype, realm, &sys, &deps, None, 42);
                for entry in &kt.entries {
                    assert!(
                        entry.proficiency >= 0.0 && entry.proficiency <= 1.0,
                        "{:?} x {:?}: proficiency {} out of range",
                        archetype,
                        realm,
                        entry.proficiency
                    );
                    assert!(entry.active);
                    // Verify technique exists
                    assert!(
                        technique_definition(&entry.id).is_some(),
                        "{:?} x {:?}: technique {} not found in definitions",
                        archetype,
                        realm,
                        entry.id
                    );
                    // Verify realm requirement met
                    let def = technique_definition(&entry.id).unwrap();
                    if let Some(required) = parse_realm(def.required_realm) {
                        assert!(
                            realm_rank(required) <= realm_rank(realm),
                            "{:?} x {:?}: technique {} requires {:?} but NPC is {:?}",
                            archetype,
                            realm,
                            entry.id,
                            def.required_realm,
                            realm
                        );
                    }
                }
            }
        }
    }

    // === parse_realm ===

    #[test]
    fn parse_realm_all_variants() {
        assert_eq!(parse_realm("Awaken"), Some(Realm::Awaken));
        assert_eq!(parse_realm("Induce"), Some(Realm::Induce));
        assert_eq!(parse_realm("Condense"), Some(Realm::Condense));
        assert_eq!(parse_realm("Solidify"), Some(Realm::Solidify));
        assert_eq!(parse_realm("Spirit"), Some(Realm::Spirit));
        assert_eq!(parse_realm("Void"), Some(Realm::Void));
        assert_eq!(parse_realm("invalid"), None);
        assert_eq!(parse_realm(""), None);
    }

    // === category_weight ===

    #[test]
    fn category_weight_heal_scales_with_missing_hp() {
        let ctx_full = NpcSkillScoringContext {
            hp_ratio: 1.0,
            ..default_ctx()
        };
        let ctx_half = NpcSkillScoringContext {
            hp_ratio: 0.5,
            ..default_ctx()
        };
        let ctx_low = NpcSkillScoringContext {
            hp_ratio: 0.3,
            ..default_ctx()
        };
        let ctx_zero = NpcSkillScoringContext {
            hp_ratio: 0.0,
            ..default_ctx()
        };

        let w_full = category_weight(SkillCategory::Heal, &ctx_full);
        let w_half = category_weight(SkillCategory::Heal, &ctx_half);
        let w_low = category_weight(SkillCategory::Heal, &ctx_low);
        let w_zero = category_weight(SkillCategory::Heal, &ctx_zero);

        assert!(
            w_full < f32::EPSILON,
            "full HP should yield ~0 heal weight, got {w_full}"
        );
        assert!(
            w_half > w_full,
            "half HP should yield higher heal weight than full"
        );
        assert!(
            w_low > w_half,
            "low HP should yield higher heal weight than half"
        );
        assert!(
            (w_zero - 0.9).abs() < f32::EPSILON,
            "zero HP should yield 0.9 heal weight, got {w_zero}"
        );
    }

    #[test]
    fn category_weight_buff_in_combat_no_active() {
        let ctx = NpcSkillScoringContext {
            has_active_buff: false,
            in_combat: true,
            ..default_ctx()
        };
        assert!(
            (category_weight(SkillCategory::Buff, &ctx) - 0.6).abs() < f32::EPSILON,
            "buff in combat without active buff should be 0.6"
        );
    }

    #[test]
    fn category_weight_buff_already_buffed() {
        let ctx = NpcSkillScoringContext {
            has_active_buff: true,
            in_combat: true,
            ..default_ctx()
        };
        assert!(
            (category_weight(SkillCategory::Buff, &ctx) - 0.05).abs() < f32::EPSILON,
            "buff with active buff should be 0.05"
        );
    }

    #[test]
    fn category_weight_buff_out_of_combat() {
        let ctx = NpcSkillScoringContext {
            has_active_buff: false,
            in_combat: false,
            ..default_ctx()
        };
        assert!(
            (category_weight(SkillCategory::Buff, &ctx) - 0.05).abs() < f32::EPSILON,
            "buff out of combat should be 0.05"
        );
    }

    #[test]
    fn category_weight_attack_constant() {
        let ctx = default_ctx();
        assert!(
            (category_weight(SkillCategory::Attack, &ctx) - 0.8).abs() < f32::EPSILON,
            "attack weight should be 0.8"
        );
    }

    #[test]
    fn category_weight_control_constant() {
        let ctx = default_ctx();
        assert!(
            (category_weight(SkillCategory::Control, &ctx) - 0.4).abs() < f32::EPSILON,
            "control weight should be 0.4"
        );
    }

    #[test]
    fn category_weight_defense_zero() {
        let ctx = default_ctx();
        assert!(
            category_weight(SkillCategory::Defense, &ctx) < f32::EPSILON,
            "defense weight should be 0.0"
        );
    }

    #[test]
    fn select_technique_defense_excluded() {
        let known = KnownTechniques {
            entries: vec![KnownTechnique {
                id: "sword.parry".to_string(),
                proficiency: 0.9,
                active: true,
            }],
        };
        let cultivation = Cultivation {
            realm: Realm::Awaken,
            qi_current: 100.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let deps = empty_deps();
        let cooldowns = NpcCooldownMap::default();
        let entity = Entity::from_raw(1);

        let result = select_technique(
            &known,
            &cultivation,
            &deps,
            None,
            &cooldowns,
            entity,
            3.0,
            100,
            &default_ctx(),
            None,
        );
        assert!(
            result.is_none(),
            "Defense category techniques should be excluded from select_technique"
        );
    }

    #[test]
    fn select_technique_low_qi_filters_high_cost() {
        let known = KnownTechniques {
            entries: vec![
                KnownTechnique {
                    id: "sword.cleave".to_string(), // qi_cost = 0.0
                    proficiency: 0.5,
                    active: true,
                },
                KnownTechnique {
                    id: "woliu.heart".to_string(), // qi_cost = 50.0
                    proficiency: 0.5,
                    active: true,
                },
            ],
        };
        let cultivation = Cultivation {
            realm: Realm::Condense,
            qi_current: 100.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let deps = empty_deps();
        let cooldowns = NpcCooldownMap::default();
        let entity = Entity::from_raw(1);

        let low_qi_ctx = NpcSkillScoringContext {
            qi_ratio: 0.1,
            ..default_ctx()
        };

        let mut cleave_selected = false;
        let mut heart_selected = false;
        for tick in 0..500u64 {
            if let Some(sel) = select_technique(
                &known,
                &cultivation,
                &deps,
                None,
                &cooldowns,
                entity,
                3.0,
                tick,
                &low_qi_ctx,
                None,
            ) {
                match sel.technique_id.as_str() {
                    "sword.cleave" => cleave_selected = true,
                    "woliu.heart" => heart_selected = true,
                    _ => {}
                }
            }
        }

        assert!(
            cleave_selected,
            "low qi_cost technique should be selectable at low qi_ratio"
        );
        assert!(
            !heart_selected,
            "high qi_cost technique should be filtered at qi_ratio < 0.15"
        );
    }

    // === category_filter: select_technique with filter ===

    #[test]
    fn select_technique_category_filter_heal_only() {
        let known = KnownTechniques {
            entries: vec![
                KnownTechnique {
                    id: "sword.cleave".to_string(),
                    proficiency: 0.5,
                    active: true,
                },
                KnownTechnique {
                    id: "zhenmai.neutralize".to_string(),
                    proficiency: 0.5,
                    active: true,
                },
            ],
        };
        let cultivation = Cultivation {
            realm: Realm::Condense,
            qi_current: 100.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let deps = empty_deps();
        let cooldowns = NpcCooldownMap::default();
        let entity = Entity::from_raw(1);

        let result = select_technique(
            &known,
            &cultivation,
            &deps,
            None,
            &cooldowns,
            entity,
            3.0,
            100,
            &default_ctx(),
            Some(SkillCategory::Heal),
        );
        let sel = result.expect("with Heal filter, zhenmai.neutralize should be selectable");
        assert_eq!(sel.technique_id, "zhenmai.neutralize");
        assert_eq!(
            sel.target,
            SkillTarget::SelfCast,
            "Heal should route to SelfCast"
        );
    }

    #[test]
    fn select_technique_category_filter_returns_none_when_no_match() {
        let known = KnownTechniques {
            entries: vec![KnownTechnique {
                id: "sword.cleave".to_string(),
                proficiency: 0.5,
                active: true,
            }],
        };
        let cultivation = Cultivation {
            realm: Realm::Condense,
            qi_current: 100.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let deps = empty_deps();
        let cooldowns = NpcCooldownMap::default();
        let entity = Entity::from_raw(1);

        let result = select_technique(
            &known,
            &cultivation,
            &deps,
            None,
            &cooldowns,
            entity,
            3.0,
            100,
            &default_ctx(),
            Some(SkillCategory::Heal),
        );
        assert!(
            result.is_none(),
            "no Heal techniques available, should return None"
        );
    }

    #[test]
    fn select_technique_category_filter_defense_selectable_when_explicit() {
        let known = KnownTechniques {
            entries: vec![KnownTechnique {
                id: "sword.parry".to_string(),
                proficiency: 0.5,
                active: true,
            }],
        };
        let cultivation = Cultivation {
            realm: Realm::Condense,
            qi_current: 100.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let deps = empty_deps();
        let cooldowns = NpcCooldownMap::default();
        let entity = Entity::from_raw(1);

        let result = select_technique(
            &known,
            &cultivation,
            &deps,
            None,
            &cooldowns,
            entity,
            3.0,
            100,
            &default_ctx(),
            Some(SkillCategory::Defense),
        );
        let sel = result.expect("Defense filter should override the default Defense exclusion");
        assert_eq!(sel.technique_id, "sword.parry");
        assert_eq!(
            sel.target,
            SkillTarget::NearestEnemy,
            "Defense should route to NearestEnemy"
        );
    }

    // === has_usable_heal_technique ===

    #[test]
    fn has_usable_heal_with_heal_technique() {
        let known = KnownTechniques {
            entries: vec![KnownTechnique {
                id: "zhenmai.neutralize".to_string(),
                proficiency: 0.5,
                active: true,
            }],
        };
        let cultivation = Cultivation {
            realm: Realm::Condense,
            qi_current: 100.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let deps = empty_deps();
        let cooldowns = NpcCooldownMap::default();
        let entity = Entity::from_raw(1);
        assert!(
            has_usable_heal_technique(&known, &cultivation, &deps, None, &cooldowns, entity, 100),
            "NPC with active heal technique should have usable heal"
        );
    }

    #[test]
    fn has_usable_heal_without_heal_technique() {
        let known = KnownTechniques {
            entries: vec![KnownTechnique {
                id: "sword.cleave".to_string(),
                proficiency: 0.5,
                active: true,
            }],
        };
        let cultivation = Cultivation {
            realm: Realm::Condense,
            qi_current: 100.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let deps = empty_deps();
        let cooldowns = NpcCooldownMap::default();
        let entity = Entity::from_raw(1);
        assert!(
            !has_usable_heal_technique(&known, &cultivation, &deps, None, &cooldowns, entity, 100),
            "NPC with only Attack techniques should not have usable heal"
        );
    }

    #[test]
    fn has_usable_heal_on_cooldown() {
        let known = KnownTechniques {
            entries: vec![KnownTechnique {
                id: "zhenmai.neutralize".to_string(),
                proficiency: 0.5,
                active: true,
            }],
        };
        let cultivation = Cultivation {
            realm: Realm::Condense,
            qi_current: 100.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let deps = empty_deps();
        let mut cooldowns = NpcCooldownMap::default();
        let entity = Entity::from_raw(1);
        cooldowns.set(entity, "zhenmai.neutralize", 200);
        assert!(
            !has_usable_heal_technique(&known, &cultivation, &deps, None, &cooldowns, entity, 100),
            "heal technique on cooldown should not be usable"
        );
    }

    #[test]
    fn has_usable_heal_inactive_technique() {
        let known = KnownTechniques {
            entries: vec![KnownTechnique {
                id: "zhenmai.neutralize".to_string(),
                proficiency: 0.5,
                active: false,
            }],
        };
        let cultivation = Cultivation {
            realm: Realm::Condense,
            qi_current: 100.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let deps = empty_deps();
        let cooldowns = NpcCooldownMap::default();
        let entity = Entity::from_raw(1);
        assert!(
            !has_usable_heal_technique(&known, &cultivation, &deps, None, &cooldowns, entity, 100),
            "inactive heal technique should not be usable"
        );
    }

    #[test]
    fn has_usable_heal_insufficient_qi() {
        let known = KnownTechniques {
            entries: vec![KnownTechnique {
                id: "zhenmai.neutralize".to_string(),
                proficiency: 0.5,
                active: true,
            }],
        };
        let cultivation = Cultivation {
            realm: Realm::Condense,
            qi_current: 0.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let deps = empty_deps();
        let cooldowns = NpcCooldownMap::default();
        let entity = Entity::from_raw(1);
        let def = technique_definition("zhenmai.neutralize").unwrap();
        if def.qi_cost > 0.0 {
            assert!(
                !has_usable_heal_technique(
                    &known,
                    &cultivation,
                    &deps,
                    None,
                    &cooldowns,
                    entity,
                    100
                ),
                "heal technique should not be usable with 0 qi when qi_cost > 0"
            );
        }
    }

    // === NpcCooldownMap cleanup on death ===

    #[test]
    fn cooldown_map_remove_all_for_clears_entity_entries() {
        let mut map = NpcCooldownMap::default();
        let npc_a = Entity::from_raw(10);
        let npc_b = Entity::from_raw(20);
        map.set(npc_a, "sword.cleave", 200);
        map.set(npc_a, "sword.thrust", 300);
        map.set(npc_b, "woliu.burst", 200);

        assert_eq!(map.len(), 3, "should have 3 entries before cleanup");

        map.remove_all_for(npc_a);

        assert_eq!(
            map.len(),
            1,
            "only npc_b's entry should remain after removing npc_a"
        );
        assert!(
            !map.is_on_cooldown(npc_a, "sword.cleave", 100),
            "npc_a cleave cooldown should be removed"
        );
        assert!(
            !map.is_on_cooldown(npc_a, "sword.thrust", 100),
            "npc_a thrust cooldown should be removed"
        );
        assert!(
            map.is_on_cooldown(npc_b, "woliu.burst", 100),
            "npc_b burst cooldown should remain intact"
        );
    }

    #[test]
    fn cooldown_map_remove_all_for_noop_on_unknown_entity() {
        let mut map = NpcCooldownMap::default();
        let npc_a = Entity::from_raw(10);
        let npc_b = Entity::from_raw(20);
        map.set(npc_a, "sword.cleave", 200);

        map.remove_all_for(npc_b);

        assert_eq!(
            map.len(),
            1,
            "removing unknown entity should not affect existing entries"
        );
    }
}
