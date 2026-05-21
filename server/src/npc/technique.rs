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
    technique_definition, KnownTechnique, KnownTechniques, TechniqueDefinition,
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
        NpcArchetype::Commoner
        | NpcArchetype::Beast
        | NpcArchetype::SkullFiend
        | NpcArchetype::Fuya
        | NpcArchetype::Zombie => {
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

    let entries: Vec<KnownTechnique> = indices
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

    KnownTechniques { entries }
}

// ─── select_technique ────────────────────────────────────────────────────────

/// NPC 战斗中选择功法。
///
/// 过滤逻辑：active → 经脉 SEVERED 排除 → 冷却排除 → qi 不足排除 →
/// range 匹配 → 按 proficiency 加权随机选一个。全部不可用返回 None。
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
) -> Option<String> {
    let mut candidates: Vec<(&KnownTechnique, &TechniqueDefinition)> = Vec::new();

    for entry in &known.entries {
        // 1. active
        if !entry.active {
            continue;
        }
        // 2. has definition
        let Some(def) = technique_definition(&entry.id) else {
            continue;
        };
        // 3. 经脉 SEVERED 排除
        let deps = meridian_deps.lookup(&entry.id);
        if check_meridian_dependencies(deps, severed).is_err() {
            continue;
        }
        // 4. 冷却排除
        if cooldowns.is_on_cooldown(npc_entity, &entry.id, current_tick) {
            continue;
        }
        // 5. qi 不足排除
        if f64::from(def.qi_cost) > cultivation.qi_current {
            continue;
        }
        // 6. range 过滤（range 0 = 自身 buff，始终可用）
        // NPC 简化：不做精确 range check，让 SkillFn 自行处理

        candidates.push((entry, def));
    }

    if candidates.is_empty() {
        return None;
    }

    // 按 proficiency 加权随机
    let total_weight: f32 = candidates
        .iter()
        .map(|(entry, _)| entry.proficiency.max(0.01))
        .sum();
    let roll = splitmix64_unit(
        current_tick
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add(npc_entity.index() as u64),
    ) * total_weight;

    let mut accum = 0.0_f32;
    for (entry, _) in &candidates {
        accum += entry.proficiency.max(0.01);
        if roll < accum {
            return Some(entry.id.clone());
        }
    }

    // fallback: 选最后一个
    candidates.last().map(|(entry, _)| entry.id.clone())
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

// ─── Scorer system ───────────────────────────────────────────────────────────

#[allow(clippy::type_complexity)]
pub fn npc_technique_scorer_system(
    npcs: Query<
        (
            &NpcBlackboard,
            &Cultivation,
            Option<&KnownTechniques>,
            Option<&NpcLastTechniqueTick>,
            Option<&MeridianSeveredPermanent>,
        ),
        With<crate::npc::spawn::NpcMarker>,
    >,
    cooldowns: Option<Res<NpcCooldownMap>>,
    meridian_deps: Option<Res<SkillMeridianDependencies>>,
    mut scorers: Query<(&Actor, &mut Score), With<NpcTechniqueScorer>>,
    clock: Option<Res<crate::cultivation::tick::CultivationClock>>,
) {
    let current_tick = clock.as_deref().map(|c| c.tick).unwrap_or(0);
    let empty_cooldowns = NpcCooldownMap::default();
    let cooldowns = cooldowns.as_deref().unwrap_or(&empty_cooldowns);
    let empty_deps = SkillMeridianDependencies::default();
    let deps = meridian_deps.as_deref().unwrap_or(&empty_deps);

    for (Actor(actor), mut score) in &mut scorers {
        let Ok((bb, cultivation, known_opt, last_tick_opt, severed_opt)) = npcs.get(*actor) else {
            score.set(0.0);
            continue;
        };

        let Some(known) = known_opt else {
            score.set(0.0);
            continue;
        };

        if known.entries.is_empty() {
            score.set(0.0);
            continue;
        }

        // 最小间隔检查
        let min_interval = 60 + realm_rank(cultivation.realm) as u64 * 10;
        let last_tick = last_tick_opt.map(|t| t.0).unwrap_or(0);
        if current_tick > 0 && current_tick.saturating_sub(last_tick) < min_interval {
            score.set(0.0);
            continue;
        }

        // 需要有目标
        if bb.nearest_player.is_none() {
            score.set(0.0);
            continue;
        }

        // 检查是否有可用功法（含经脉 SEVERED 过滤）
        let has_usable = select_technique(
            known,
            cultivation,
            deps,
            severed_opt,
            cooldowns,
            *actor,
            bb.player_distance,
            current_tick,
        )
        .is_some();

        score.set(if has_usable { 0.85 } else { 0.0 });
    }
}

// ─── Action system (exclusive) ───────────────────────────────────────────────

/// Exclusive system 驱动 NpcTechniqueAction。
///
/// 与玩家功法调用路径一致：`SkillRegistry.lookup(technique_id)` → `SkillFn(&mut World, ...)`。
/// Requested → select_technique → skill_fn → CastResult → Success/Failure。
pub fn npc_technique_action_system(world: &mut valence::prelude::bevy_ecs::world::World) {
    use crate::cultivation::skill_registry::{CastResult, SkillRegistry};

    // Step 1: Collect all Requested NPC entities
    let mut requested: Vec<(Entity, Entity)> = Vec::new(); // (action_entity, actor_entity)
    {
        let mut query = world.query::<(&Actor, &ActionState, &NpcTechniqueAction)>();
        for (actor, state, _action) in query.iter(world) {
            if *state == ActionState::Requested {
                requested.push((Entity::PLACEHOLDER, actor.0));
            }
        }
    }

    // We need a different approach: query action entities with their state
    let mut actions_to_process: Vec<(Entity, Entity, ActionState)> = Vec::new();
    {
        let mut query =
            world.query_filtered::<(Entity, &Actor, &ActionState), With<NpcTechniqueAction>>();
        for (action_entity, actor, state) in query.iter(world) {
            actions_to_process.push((action_entity, actor.0, state.clone()));
        }
    }

    for (action_entity, actor_entity, state) in actions_to_process {
        match state {
            ActionState::Requested => {
                // Read NPC data
                let (technique_id, _cooldown_ticks) = {
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

                    match select_technique(
                        known,
                        cultivation,
                        deps,
                        severed,
                        cooldowns,
                        actor_entity,
                        target_distance,
                        current_tick,
                    ) {
                        Some(tid) => {
                            // Look up the definition for cooldown info
                            let cd = technique_definition(&tid)
                                .map(|d| d.cooldown_ticks as u64)
                                .unwrap_or(60);
                            (tid, cd)
                        }
                        None => {
                            set_action_state(world, action_entity, ActionState::Failure);
                            continue;
                        }
                    }
                };

                // Lookup skill_fn
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

                // Get target
                let target = world
                    .get::<NpcBlackboard>(actor_entity)
                    .and_then(|bb| bb.nearest_player);

                // Call skill_fn
                let result = skill_fn(world, actor_entity, 0, target);

                match result {
                    CastResult::Started {
                        cooldown_ticks: cd, ..
                    } => {
                        // Write cooldown
                        let clock =
                            world.get_resource::<crate::cultivation::tick::CultivationClock>();
                        let current_tick = clock.map(|c| c.tick).unwrap_or(0);

                        if let Some(cooldowns) = world.get_resource_mut::<NpcCooldownMap>() {
                            let mut cooldowns = cooldowns;
                            cooldowns.set(actor_entity, &technique_id, current_tick + cd);
                        }

                        // Update last technique tick
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
    fn assign_disciple_returns_2_to_4() {
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
                kt.entries.len() >= 2 && kt.entries.len() <= 4,
                "disciple should have 2-4 techniques, got {} (seed={})",
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
    fn assign_guardian_relic_returns_3_to_5() {
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
                kt.entries.len() >= 3 && kt.entries.len() <= 5,
                "guardian relic should have 3-5 techniques, got {} (seed={})",
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
    fn assign_daoxiang_returns_1_to_2() {
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
                !kt.entries.is_empty() && kt.entries.len() <= 2,
                "daoxiang should have 1-2 techniques, got {} (seed={})",
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
    fn assign_zhinian_returns_2_to_3() {
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
                kt.entries.len() >= 2 && kt.entries.len() <= 3,
                "zhinian should have 2-3 techniques, got {} (seed={})",
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
        );
        assert!(result.is_some(), "should select a technique");
        assert_eq!(result.unwrap(), "sword.cleave");
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
                id: "zhenmai.parry".to_string(), // depends on Lung
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
        // Declare Lung dependency for zhenmai.parry
        let mut deps = SkillMeridianDependencies::default();
        deps.declare("zhenmai.parry", vec![MeridianId::Lung]);

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
            if let Some(id) = select_technique(
                &known,
                &cultivation,
                &deps,
                None,
                &cooldowns,
                entity,
                3.0,
                tick,
            ) {
                if id == "sword.thrust" {
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
}
