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

use crate::body_plan::RaceId;
use crate::cultivation::components::{Cultivation, MeridianSystem, Realm};
use crate::cultivation::known_techniques::{
    parse_required_realm, KnownTechnique, KnownTechniques, SkillCategory, TechniqueDefinition,
    TechniqueDispatch, TechniqueRegistry, NPC_PASSIVE_TECHNIQUE_IDS,
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

/// 根据境界生成 NPC 用的 MeridianSystem（§8.1 #8 "公式即数据"决议：配额来自
/// `body_plan.meridian_profile.realm_requirements`，不是全局曲线）。
///
/// humanoid 曲线 Awaken=1, Induce=3, Condense=6, Solidify=12, Spirit=16, Void=20 由
/// `humanoid.json` 自身声明；打开顺序：按 `body_plan.meridian_profile.channels` 声明
/// 顺序（humanoid.json 与退役前 `MeridianId::ALL`——先 12 正经（REGULAR），再 8 奇经
/// （EXTRAORDINARY）——逐条 bit-for-bit 一致，见 `body_plan` 侧对拍测试）。
///
/// plan-race-system-v1 P1 对抗审查 M1：此前骨架恒用 `MeridianSystem::default()`
/// （humanoid 骨架）+ `realm.required_meridians()`（humanoid 全局曲线），传入非人
/// `body_plan` 时会假参数化——数值仍按 humanoid 走，与传入的 profile 无关。现改为
/// `MeridianSystem::for_profile(profile)` 建骨架 + 从**本 profile 自身**
/// `realm_requirements[realm.rank() - 1].total` 读取应开脉数，非人 profile（如 P1
/// 6-channel 合成构型）不再 panic 或误用 humanoid 数值。
pub fn npc_meridian_system_for_realm(
    realm: Realm,
    body_plan: &crate::body_plan::BodyPlan,
) -> MeridianSystem {
    let profile = body_plan.meridian_profile.as_ref().unwrap_or_else(|| {
        panic!(
            "[bong][npc][technique] npc_meridian_system_for_realm: body plan {} has no \
             meridian_profile — cannot generate NPC MeridianSystem",
            body_plan.id
        )
    });
    let count = profile.realm_requirements[realm.rank() as usize - 1].total as usize;
    let mut sys = MeridianSystem::for_profile(profile);
    for channel in profile.channels.iter().take(count) {
        let m = sys.get_mut(channel.id.clone());
        m.opened = true;
        m.integrity = 1.0;
        m.throughput_current = 1.0;
    }
    sys
}

// ─── assign_npc_techniques ───────────────────────────────────────────────────

/// 判断一条 technique definition 的 `required_realm` 是否被给定 `realm` 满足
/// （`required_realm` 解析失败视为不满足）。`assign_npc_techniques` 内部筛选与
/// 跨模块 audit 测试（验证任意 spawn 路径产出的 NPC 不持有超出自身
/// `Cultivation.realm` 的功法）共用同一比较口径，避免两处各写一份、日后跑偏。
pub(crate) fn technique_realm_satisfied(def: &TechniqueDefinition, realm: Realm) -> bool {
    let Some(required) = parse_required_realm(&def.required_realm) else {
        return false;
    };
    realm_rank(required) <= realm_rank(realm)
}

/// 检查 technique definition 的经脉依赖在 NPC 的 MeridianSystem 中是否满足（已开）。
fn meridian_deps_satisfied(
    definition: &TechniqueDefinition,
    meridian_sys: &MeridianSystem,
    meridian_deps: &SkillMeridianDependencies,
) -> bool {
    // 1. 检查 SkillMeridianDependencies 表中的依赖
    let deps = meridian_deps.lookup(&definition.id);
    for dep_id in deps {
        let m = meridian_sys.get(*dep_id);
        if !m.opened {
            return false;
        }
    }
    // 2. 检查 TechniqueDefinition.required_meridians 中的依赖
    for required in &definition.required_meridians {
        let Some(channel) = parse_meridian_id(&required.channel) else {
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
/// 1. `realm_rank(parse_required_realm(&def.required_realm)) <= realm_rank(npc_realm)`
/// 2. 经脉依赖满足（`meridian_deps.lookup` + `MeridianSystem` 已开）
///
/// NPC 功法 proficiency spawn 时固定，不随战斗增长（§8.1 #2 决议）。
/// 各 archetype 功法分配见 plan §P1.1 表格。
pub fn assign_npc_techniques(
    technique_registry: &TechniqueRegistry,
    archetype: NpcArchetype,
    realm: Realm,
    meridian_sys: &MeridianSystem,
    meridian_deps: &SkillMeridianDependencies,
    qi_color_hint: Option<&str>,
    entity_seed: u64,
) -> KnownTechniques {
    assign_npc_techniques_for_identity(
        technique_registry,
        archetype,
        realm,
        meridian_sys,
        meridian_deps,
        qi_color_hint,
        entity_seed,
        &RaceId::new("human"),
        true,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn assign_npc_techniques_for_identity(
    technique_registry: &TechniqueRegistry,
    archetype: NpcArchetype,
    realm: Realm,
    meridian_sys: &MeridianSystem,
    meridian_deps: &SkillMeridianDependencies,
    _qi_color_hint: Option<&str>,
    entity_seed: u64,
    race: &RaceId,
    is_humanoid: bool,
) -> KnownTechniques {
    // NPC 主动功法池只收 resolver-backed 条目。`direct_generic` 只有玩家 skill-bar
    // 生命周期，并不等于 NPC 有可调用的 SkillFn；若把它们混入候选，评分器会给出高分，
    // action 随后 lookup 失败且不更新冷却/last_tick，最终反复抢占 NPC 的 melee 回合。
    // `body.guangbo_ticao` 是有意保留的 direct-generic 被动，走下方独立 passive 注入，
    // 不进入主动池。
    let available: Vec<&TechniqueDefinition> = technique_registry
        .iter()
        .filter(|def| {
            def.dispatch == TechniqueDispatch::MetadataBacked
                && def.required_race.allows(race, is_humanoid)
                && technique_realm_satisfied(def, realm)
                && meridian_deps_satisfied(def, meridian_sys, meridian_deps)
        })
        .collect();
    let passive_available: Vec<&TechniqueDefinition> = technique_registry
        .iter()
        .filter(|def| {
            NPC_PASSIVE_TECHNIQUE_IDS.contains(&def.id.as_str())
                && def.required_race.allows(race, is_humanoid)
                && technique_realm_satisfied(def, realm)
                && meridian_deps_satisfied(def, meridian_sys, meridian_deps)
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
        | NpcArchetype::DyingElder
        // plan-mundane-fauna-v1 P0：凡兽无灵，不修炼功法（qi_physics 锚点：无灵生物不接触
        // 功法/qi 系统）。
        | NpcArchetype::Mundane => {
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

    if available.is_empty() && passive_available.is_empty() {
        return KnownTechniques {
            entries: Vec::new(),
        };
    }

    // 决定基础功法总数量。被动条目走独立注入路径，但占用同一基础配额；这样 NPC
    // 仍保持既有 1-3/2-4/3-5 等 KnownTechniques 数量，同时不会把 passive 当成可施法招式。
    let range = (count_max - count_min + 1) as u32;
    let total_count = count_min + splitmix64_range(entity_seed, range) as usize;
    let passive_count = passive_available.len().min(total_count);
    // 被动条目共享基础配额，但不能吞掉唯一主动条目；NPC 至少保留一个可施放功法，
    // 否则通用战斗 scorer 会在已知功法非空时失去 cast-ready 路径。
    let active_count = total_count
        .saturating_sub(passive_count)
        .max(1)
        .min(available.len());

    // Fisher-Yates shuffle 选前 active_count 个
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
        .take(active_count)
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

    inject_npc_passive_skills(
        &mut entries,
        &passive_available,
        prof_min,
        prof_max,
        entity_seed,
    );
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

fn inject_npc_passive_skills(
    entries: &mut Vec<KnownTechnique>,
    available: &[&TechniqueDefinition],
    prof_min: f32,
    prof_max: f32,
    seed: u64,
) {
    for (index, definition) in available.iter().enumerate() {
        if entries.iter().any(|entry| entry.id == definition.id) {
            continue;
        }
        let proficiency = prof_min
            + splitmix64_unit(seed.wrapping_add(index as u64 * 0xD1B5_4A32))
                * (prof_max - prof_min);
        entries.push(KnownTechnique {
            id: definition.id.clone(),
            proficiency: proficiency.clamp(prof_min, prof_max),
            active: true,
        });
    }
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
/// 过滤逻辑：active → category_filter 限定 → 经脉 SEVERED 排除 → 经脉 opened 实时检查 →
/// 冷却排除 → qi 不足排除 →
/// qi_ratio < 0.15 排除高 qi_cost 50% → Defense 排除（走独立 NpcDefenseAction）→
/// 按 `category_weight(ctx) * proficiency` 加权随机选一个。全部不可用返回 None。
///
/// `category_filter` 为 Some 时，只保留该类别的功法（NpcHealAction 用 Some(Heal)）。
///
/// `meridian_sys` 为 Some 时，额外检查依赖经脉的 `opened` 字段（dugu 毒关脉但不触发
/// MeridianSeveredPermanent 的场景）。
#[allow(clippy::too_many_arguments)]
pub fn select_technique(
    technique_registry: &TechniqueRegistry,
    known: &KnownTechniques,
    cultivation: &Cultivation,
    meridian_deps: &SkillMeridianDependencies,
    severed: Option<&MeridianSeveredPermanent>,
    meridian_sys: Option<&MeridianSystem>,
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
        let Some(def) = technique_registry.get(&entry.id) else {
            continue;
        };
        if def.dispatch != TechniqueDispatch::MetadataBacked
            || NPC_PASSIVE_TECHNIQUE_IDS.contains(&def.id.as_str())
        {
            continue;
        }
        if let Some(filter) = category_filter {
            if def.category != filter {
                continue;
            }
        }
        let deps = meridian_deps.lookup(&entry.id);
        if check_meridian_dependencies(deps, severed).is_err() {
            continue;
        }
        // 实时检查依赖经脉的 opened 状态：dugu 毒会关脉（opened=false）但不写入
        // MeridianSeveredPermanent，所以上面的 SEVERED 检查不足以拦截这类情况。
        if let Some(sys) = meridian_sys {
            if deps.iter().any(|dep_id| !sys.get(*dep_id).opened) {
                continue;
            }
        }
        if cooldowns.is_on_cooldown(npc_entity, &entry.id, current_tick) {
            continue;
        }
        if def.qi_cost > cultivation.qi_current {
            continue;
        }
        // 通用功法池(category_filter=None)排除有专属 scorer/action 通道的类别：
        // Defense 走 NpcDefenseScorer→jiemai；Heal 走 NpcHealScorer→NpcHealAction(filter=Some(Heal))。
        // 二者若漏出到通用池，会被下方加权随机选中——且 category_weight(Heal/Defense) 即便为 0
        // 也被 .max(0.001) 抬成非零权重，导致 NPC 满血时仍有概率用「通用功法回合」self-cast 治疗，
        // 抢占进攻。Heal 此前漏排除（只挡了 Defense），与其专属通道重复且行为劣化。
        if category_filter.is_none()
            && matches!(def.category, SkillCategory::Defense | SkillCategory::Heal)
        {
            continue;
        }

        candidates.push((entry, def));
    }

    if candidates.is_empty() {
        return None;
    }

    if ctx.qi_ratio < 0.15 {
        let mut qi_costs: Vec<f64> = candidates.iter().map(|(_, def)| def.qi_cost).collect();
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
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn npc_heal_scorer_system(
    npcs: Query<
        (
            &NpcBlackboard,
            &Cultivation,
            Option<&KnownTechniques>,
            Option<&MeridianSeveredPermanent>,
            Option<&MeridianSystem>,
            Option<&crate::combat::components::Wounds>,
            Option<&crate::npc::lod::NpcLodTier>,
        ),
        With<crate::npc::spawn::NpcMarker>,
    >,
    technique_registry: Res<TechniqueRegistry>,
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
        let Ok((_bb, cultivation, known_opt, severed_opt, meridian_sys_opt, wounds_opt, tier)) =
            npcs.get(*actor)
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
                let Some(def) = technique_registry.get(&entry.id) else {
                    return false;
                };
                if def.dispatch != TechniqueDispatch::MetadataBacked
                    || NPC_PASSIVE_TECHNIQUE_IDS.contains(&def.id.as_str())
                    || def.category != SkillCategory::Heal
                {
                    return false;
                }
                let entry_deps = deps.lookup(&entry.id);
                if check_meridian_dependencies(entry_deps, severed_opt).is_err() {
                    return false;
                }
                // 实时检查依赖经脉 opened（dugu 毒场景）
                if let Some(sys) = meridian_sys_opt {
                    if entry_deps.iter().any(|dep_id| !sys.get(*dep_id).opened) {
                        return false;
                    }
                }
                if cooldowns.is_on_cooldown(*actor, &entry.id, current_tick) {
                    return false;
                }
                if def.qi_cost > cultivation.qi_current {
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
///
/// `meridian_sys` 为 Some 时，额外检查依赖经脉的 `opened` 字段（镜像 select_technique
/// 的实时 opened 检查，防止 dugu 毒关脉后仍释放治疗功法）。
#[allow(clippy::too_many_arguments)]
pub fn has_usable_heal_technique(
    technique_registry: &TechniqueRegistry,
    known: &KnownTechniques,
    cultivation: &Cultivation,
    deps: &SkillMeridianDependencies,
    severed: Option<&MeridianSeveredPermanent>,
    meridian_sys: Option<&MeridianSystem>,
    cooldowns: &NpcCooldownMap,
    npc_entity: Entity,
    current_tick: u64,
) -> bool {
    known.entries.iter().any(|entry| {
        if !entry.active {
            return false;
        }
        let Some(def) = technique_registry.get(&entry.id) else {
            return false;
        };
        if def.dispatch != TechniqueDispatch::MetadataBacked
            || NPC_PASSIVE_TECHNIQUE_IDS.contains(&def.id.as_str())
            || def.category != SkillCategory::Heal
        {
            return false;
        }
        let entry_deps = deps.lookup(&entry.id);
        if check_meridian_dependencies(entry_deps, severed).is_err() {
            return false;
        }
        // 实时检查依赖经脉 opened（dugu 毒场景）
        if let Some(sys) = meridian_sys {
            if entry_deps.iter().any(|dep_id| !sys.get(*dep_id).opened) {
                return false;
            }
        }
        if cooldowns.is_on_cooldown(npc_entity, &entry.id, current_tick) {
            return false;
        }
        if def.qi_cost > cultivation.qi_current {
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

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn npc_technique_scorer_system(
    npcs: Query<
        (
            &NpcBlackboard,
            &Cultivation,
            Option<&KnownTechniques>,
            Option<&NpcLastTechniqueTick>,
            Option<&MeridianSeveredPermanent>,
            Option<&MeridianSystem>,
            Option<&crate::combat::components::Wounds>,
            Option<&crate::npc::lod::NpcLodTier>,
        ),
        With<crate::npc::spawn::NpcMarker>,
    >,
    technique_registry: Res<TechniqueRegistry>,
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
        let Ok((
            bb,
            cultivation,
            known_opt,
            last_tick_opt,
            severed_opt,
            meridian_sys_opt,
            wounds_opt,
            tier,
        )) = npcs.get(*actor)
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
                &technique_registry,
                known,
                cultivation,
                deps,
                severed_opt,
                meridian_sys_opt,
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
                    let meridian_sys = world.get::<MeridianSystem>(actor_entity);

                    let technique_registry = world
                        .get_resource::<TechniqueRegistry>()
                        .expect(
                            "cultivation::register must insert TechniqueRegistry before NPC skill selection",
                        );
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
                        technique_registry,
                        known,
                        cultivation,
                        deps,
                        severed,
                        meridian_sys,
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
#[path = "technique_tests.rs"]
mod tests;
