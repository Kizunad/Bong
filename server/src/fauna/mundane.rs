//! plan-mundane-fauna-v1 P0 — 凡兽底盘：9 种 MC 1.20.1 原版被动生物，走 Valence 原生
//! entity bundle（`npc/spawn/zombie.rs:52` 的 `ZombieEntityBundle { kind: EntityKind::ZOMBIE, .. }`
//! Rail A 范式，**不走** `beast.rs:64` 的 `MarkerEntityBundle` custom visual 路数）。
//! client 零改动，vanilla renderer 免费渲染原版模型/贴图/音效。
//!
//! **威胁谱系**（[[feedback_threat_spectrum]]）：凡兽最低档也能反抗，见
//! `npc/spawn/mundane.rs` 的 4-thinker 组合（`CorneredScorer` 排在 `FleeThreatScorer` 前）。
//!
//! **qi_physics 锚点**：凡兽无灵——不吸灵气、不放灵气、死亡无 qi 释放（本模块引入零个
//! qi 常数）。

use valence::prelude::{bevy_ecs, App, Commands, Component, DVec3, Entity, EntityKind, Update};

use crate::fauna::components::fauna_spawn_seed;
use crate::npc::spawn::ambient_scheduler::{
    ambient_scheduler_system, AmbientMarkerData, AmbientSchedulerConfig, AmbientSchedulerState,
    ThreatBudget,
};
use crate::npc::spawn::spawn_mundane_fauna_at;
use crate::world::zone::Zone;

// ---------------------------------------------------------------------------
// MundaneFaunaKind — 9 变体终表（§8.1 #1 锁定，不增删）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MundaneFaunaKind {
    Cow,
    Pig,
    Sheep,
    Chicken,
    Rabbit,
    Goat,
    Frog,
    Fox,
    Wolf,
}

impl MundaneFaunaKind {
    /// 所有变体，用于遍历和穷尽性测试。
    pub const ALL: [MundaneFaunaKind; 9] = [
        Self::Cow,
        Self::Pig,
        Self::Sheep,
        Self::Chicken,
        Self::Rabbit,
        Self::Goat,
        Self::Frog,
        Self::Fox,
        Self::Wolf,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cow => "cow",
            Self::Pig => "pig",
            Self::Sheep => "sheep",
            Self::Chicken => "chicken",
            Self::Rabbit => "rabbit",
            Self::Goat => "goat",
            Self::Frog => "frog",
            Self::Fox => "fox",
            Self::Wolf => "wolf",
        }
    }

    /// 威胁谱系 health_max（§8.1 决议数值）：T0 惊扰反抗档（鸡/兔/蛙）最低 4-6，
    /// T1 被击反抗档（牛/猪/羊）10-14，T1.5 主动冲撞（山羊）与 T1 同档，
    /// T2 掠食骚扰（狐）16，T2.5 群体掠食（狼）最高 26——覆盖"鸡 < 狼"威胁谱系差异化，
    /// 不共享全局 DEFAULT_HEALTH_MAX（照 `beast.rs:111` 覆盖 wounds.health_current/max 范式）。
    pub const fn health_max(self) -> f32 {
        match self {
            Self::Chicken => 4.0,
            Self::Rabbit => 5.0,
            Self::Frog => 6.0,
            Self::Sheep => 10.0,
            Self::Pig => 12.0,
            Self::Goat => 12.0,
            Self::Cow => 14.0,
            Self::Fox => 16.0,
            Self::Wolf => 26.0,
        }
    }
}

/// `MundaneFaunaKind` → Valence 原生 `EntityKind`（Rail A 头部，照
/// `fauna::visual::entity_kind_for_beast` 范式）。实际 spawn 时每个 kind 对应一个**不同的**
/// `<X>EntityBundle` 具体类型（`CowEntityBundle`/`PigEntityBundle`/...），Rust 类型系统不允许
/// 从单个函数返回异构 Bundle 类型，故真正的 bundle 构造 match 落在
/// `npc/spawn/mundane.rs::spawn_mundane_fauna_at`（同 `spawn/common.rs::spawn_rogue_commoner_base`
/// 按 `EntityKind` match 后各自 `insert` 具体 bundle 的先例）。本函数是该 match 的分支依据。
pub const fn entity_kind_for_mundane(kind: MundaneFaunaKind) -> EntityKind {
    match kind {
        MundaneFaunaKind::Cow => EntityKind::COW,
        MundaneFaunaKind::Pig => EntityKind::PIG,
        MundaneFaunaKind::Sheep => EntityKind::SHEEP,
        MundaneFaunaKind::Chicken => EntityKind::CHICKEN,
        MundaneFaunaKind::Rabbit => EntityKind::RABBIT,
        MundaneFaunaKind::Goat => EntityKind::GOAT,
        MundaneFaunaKind::Frog => EntityKind::FROG,
        MundaneFaunaKind::Fox => EntityKind::FOX,
        MundaneFaunaKind::Wolf => EntityKind::WOLF,
    }
}

/// 凡兽物种 tag component——记录一个已生成实体是哪个 `MundaneFaunaKind`。**不能**塞进
/// [`MundaneFaunaMarker`]（见该类型文档：`ambient_scheduler_system` 会在 pool_fn 产出实体后
/// 用 `M::new(now, zone.name.clone())` 覆盖式 insert marker，`AmbientMarkerData::new` 签名
/// 只有 `(spawned_at, home_zone)` 两个参数，装不下 `kind`），故独立成组件，`spawn_mundane_fauna_at`
/// 内直接 insert。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Component)]
pub struct MundaneFaunaSpecies(pub MundaneFaunaKind);

// ---------------------------------------------------------------------------
// MundaneFaunaMarker — AmbientMarkerData 实现（§8.1 #2 3 步接入第一步）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Component)]
pub struct MundaneFaunaMarker {
    pub spawned_at: u64,
    pub home_zone: String,
}

impl AmbientMarkerData for MundaneFaunaMarker {
    fn new(spawned_at: u64, home_zone: String) -> Self {
        Self {
            spawned_at,
            home_zone,
        }
    }

    fn home_zone(&self) -> &str {
        &self.home_zone
    }
}

// ---------------------------------------------------------------------------
// biome 分池（§8.1 #2 决议：AmbientPoolFn 签名只给 `&Zone`，没有 `Res<TerrainProvider>`
// 可用——「零改调度核」约束下不能给 AmbientPoolFn 类型加参数改造调用点。改用 `Zone::name`
// 判定，同 `Zone::botany_tags`（world/zone.rs:475）既有先例：zone 名字面量与地形 profile
// 一一对应，见 CLAUDE.md「Terrain profiles」——qingyun_peaks≈峰区/lingquan_marsh≈沼泽/
// north_wastes≈荒原，其余（含 spawn）≈平原，语义等价 TerrainProvider 的
// is_peaks_biome/is_marsh_biome/is_wastes_biome 谓词。
// ---------------------------------------------------------------------------

/// 平原/spawn 池：鸡/兔/猪/羊（默认兜底——覆盖 spawn、blood_valley/rift_valley、
/// youan_depths 等未特别标注的 zone）。
const PLAINS_POOL: &[MundaneFaunaKind] = &[
    MundaneFaunaKind::Cow,
    MundaneFaunaKind::Pig,
    MundaneFaunaKind::Sheep,
    MundaneFaunaKind::Chicken,
    MundaneFaunaKind::Rabbit,
];

/// 沼泽池：蛙/兔。
const MARSH_POOL: &[MundaneFaunaKind] = &[MundaneFaunaKind::Frog, MundaneFaunaKind::Rabbit];

/// 峰区池：山羊/羊。
const PEAKS_POOL: &[MundaneFaunaKind] = &[MundaneFaunaKind::Goat, MundaneFaunaKind::Sheep];

/// 荒原池：兔/狐/狼——plan §P0 原文列了兔/狐但漏收狼（9 变体终表里狼在任何 biome 池均未
/// 出现，无处可刷），本次实施把 T2.5 狼补进荒原池（贫瘠地带=狼群猎场，语义自洽，且
/// 补足"9 variant 全覆盖"测试要求——若后续人工判定狼该独立成另一 biome，归 P2 调整）。
const WASTES_POOL: &[MundaneFaunaKind] = &[
    MundaneFaunaKind::Rabbit,
    MundaneFaunaKind::Fox,
    MundaneFaunaKind::Wolf,
];

/// 按 zone 名判定 biome 分池（见模块顶部关于 `AmbientPoolFn` 签名约束的说明）。取
/// `zone_name: &str`（而非 `&Zone`）——本函数只读 zone 名字这一个字段，接口收窄成纯字符串
/// 判定后，`npc/hydrate/mod.rs` 复活快照（无 `Zone` 对象、只有持久化的 `zone_name`）也能
/// 调用同一份口径重新派生物种，不需要新增持久化字段（同 `spawn_beast_npc_at` 用
/// `fauna_tag_for_beast_spawn(home_zone, seed)` 从 home_zone+位置重新派生 `BeastKind`
/// 而不持久化它的先例）。
pub fn mundane_biome_pool(zone_name: &str) -> &'static [MundaneFaunaKind] {
    if zone_name.eq_ignore_ascii_case("lingquan_marsh") {
        MARSH_POOL
    } else if zone_name.eq_ignore_ascii_case("qingyun_peaks") {
        PEAKS_POOL
    } else if zone_name.eq_ignore_ascii_case("north_wastes") {
        WASTES_POOL
    } else {
        PLAINS_POOL
    }
}

/// 从 pool 按权重相等（均权 1）抽样，`seed % len` 确定性选取。空池返回 `None`
/// （理论上四张池均非空，双重防御同 `select_threat_species` 范式）。
pub fn select_mundane_species(pool: &[MundaneFaunaKind], seed: u64) -> Option<MundaneFaunaKind> {
    if pool.is_empty() {
        return None;
    }
    let idx = (seed % pool.len() as u64) as usize;
    Some(pool[idx])
}

/// 从 `(zone_name, position)` 确定性派生物种——biome 池 + [`fauna_spawn_seed`] 组合，
/// 供 [`mundane_pool_fn`]（新 spawn）与 `npc::hydrate`（dormant 复活重新派生，不持久化
/// `MundaneFaunaKind` 字段）共用同一口径。四张 biome 池恒非空，`unwrap_or` 兜底仅为
/// 防御（理论不可达）。
pub fn mundane_species_for_position(zone_name: &str, position: DVec3) -> MundaneFaunaKind {
    let pool = mundane_biome_pool(zone_name);
    let seed = fauna_spawn_seed(zone_name, position.x, position.z);
    select_mundane_species(pool, seed).unwrap_or(MundaneFaunaKind::Cow)
}

// ---------------------------------------------------------------------------
// mundane_passive_budget_fn / mundane_pool_fn（§8.1 #2/#4 决议数值）
// ---------------------------------------------------------------------------

/// 凡兽 passive 预算：v1 保守拍小，不看 `danger`（凡兽不分 danger 分级，恒同一档）。
/// `max_alive=3`（§8.1 #4），`pack_size_range=(1,1)`（调度核每次巡检产 1 个，多产未消费），
/// `spawn_interval_ticks=400`（复用 `threat_budget(3)` 同档 stride，§8.1 #4）。
pub fn mundane_passive_budget_fn(_danger: u8) -> ThreatBudget {
    ThreatBudget {
        max_alive: 3,
        spawn_interval_ticks: 400,
        pack_size_range: (1, 1),
    }
}

/// 真实 `AmbientPoolFn`：按 biome 分池选物种 → `spawn_mundane_fauna_at`。**P0 范围限定**：
/// 死域/负灵域栖息门槛过滤（按 `zone.spirit_qi`）留给 P2 落地（§8.1 决议），P0 本函数恒不
/// 过滤，四张 biome 池均非空、`mundane_species_for_position` 恒有确定性结果，故 P0 阶段
/// 本函数恒返回 `Some`（调用方 `ambient_scheduler_system` 已经处理 `None` 分支，留给 P2
/// 门槛落地时复用）。
pub fn mundane_pool_fn(
    commands: &mut Commands,
    layer: Entity,
    zone: &Zone,
    spawn_position: DVec3,
    patrol_target: DVec3,
) -> Option<Entity> {
    let kind = mundane_species_for_position(&zone.name, spawn_position);
    Some(spawn_mundane_fauna_at(
        commands,
        layer,
        &zone.name,
        spawn_position,
        patrol_target,
        kind,
    ))
}

// ---------------------------------------------------------------------------
// register — 3 步纯复用 ambient_scheduler（§8.1 #2 决议，零改调度核）
// ---------------------------------------------------------------------------

pub fn register(app: &mut App) {
    app.insert_resource(AmbientSchedulerState::<MundaneFaunaMarker>::default())
        .insert_resource(AmbientSchedulerConfig::<MundaneFaunaMarker>::new(
            mundane_passive_budget_fn,
            mundane_pool_fn,
            // counts_against_threat_budget=false（§8.1 #3）：凡兽全档独立 passive 预算，
            // 不进 plan-ambient-threat-v1 的 zone 威胁密度统计。
            false,
        ))
        .add_systems(Update, ambient_scheduler_system::<MundaneFaunaMarker>);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::dimension::DimensionKind;

    fn zone_with_name(name: &str) -> Zone {
        Zone {
            name: name.to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (
                DVec3::new(-500.0, 0.0, -500.0),
                DVec3::new(500.0, 200.0, 500.0),
            ),
            spirit_qi: 0.5,
            danger_level: 1,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
            qi_equilibrium: 0.0,
            qi_inflow_per_min: 0.0,
        }
    }

    // -----------------------------------------------------------------
    // MundaneFaunaKind — 9 variant pin
    // -----------------------------------------------------------------

    #[test]
    fn all_nine_variants_present_and_distinct() {
        assert_eq!(MundaneFaunaKind::ALL.len(), 9, "§8.1 #1 锁定 9 变体终表");
        let mut as_strs: Vec<&str> = MundaneFaunaKind::ALL.iter().map(|k| k.as_str()).collect();
        as_strs.sort_unstable();
        as_strs.dedup();
        assert_eq!(
            as_strs.len(),
            9,
            "9 个变体的 as_str() 必须两两不同，否则 narration/loot 文案会撞名"
        );
    }

    #[test]
    fn entity_kind_for_mundane_pins_all_nine_to_native_valence_kind() {
        let expected = [
            (MundaneFaunaKind::Cow, EntityKind::COW),
            (MundaneFaunaKind::Pig, EntityKind::PIG),
            (MundaneFaunaKind::Sheep, EntityKind::SHEEP),
            (MundaneFaunaKind::Chicken, EntityKind::CHICKEN),
            (MundaneFaunaKind::Rabbit, EntityKind::RABBIT),
            (MundaneFaunaKind::Goat, EntityKind::GOAT),
            (MundaneFaunaKind::Frog, EntityKind::FROG),
            (MundaneFaunaKind::Fox, EntityKind::FOX),
            (MundaneFaunaKind::Wolf, EntityKind::WOLF),
        ];
        for (kind, expected_entity_kind) in expected {
            assert_eq!(
                entity_kind_for_mundane(kind),
                expected_entity_kind,
                "{kind:?} 必须映射到原生 {expected_entity_kind:?}（Rail A，client 零改渲染）"
            );
        }
    }

    #[test]
    fn health_max_is_strictly_lower_for_chicken_than_wolf() {
        // 威胁谱系硬约束：鸡（T0）< 狼（T2.5），不能共享全局默认值。
        assert!(
            MundaneFaunaKind::Chicken.health_max() < MundaneFaunaKind::Wolf.health_max(),
            "鸡 health_max={} 必须严格低于狼 health_max={}",
            MundaneFaunaKind::Chicken.health_max(),
            MundaneFaunaKind::Wolf.health_max()
        );
    }

    #[test]
    fn health_max_is_differentiated_across_all_nine_variants() {
        // 每个变体至少要有自己的档位（不要求两两不同，但要覆盖 T0<T1<T2<T2.5 整体单调）。
        let chicken = MundaneFaunaKind::Chicken.health_max();
        let rabbit = MundaneFaunaKind::Rabbit.health_max();
        let frog = MundaneFaunaKind::Frog.health_max();
        let sheep = MundaneFaunaKind::Sheep.health_max();
        let pig = MundaneFaunaKind::Pig.health_max();
        let goat = MundaneFaunaKind::Goat.health_max();
        let cow = MundaneFaunaKind::Cow.health_max();
        let fox = MundaneFaunaKind::Fox.health_max();
        let wolf = MundaneFaunaKind::Wolf.health_max();

        let t0_max = chicken.max(rabbit).max(frog);
        let t1_min = sheep.min(pig).min(goat).min(cow);
        let t1_max = sheep.max(pig).max(goat).max(cow);
        assert!(
            t0_max < t1_min,
            "T0(鸡/兔/蛙, max={t0_max}) 应严格低于 T1(牛/猪/羊/山羊, min={t1_min})"
        );
        assert!(t1_max < fox, "T1(max={t1_max}) 应低于 T2(狐, {fox})");
        assert!(
            fox < wolf,
            "T2(狐, {fox})应低于 T2.5(狼, {wolf})——狼是凡兽里的真威胁"
        );
    }

    #[test]
    fn health_max_all_positive() {
        for kind in MundaneFaunaKind::ALL {
            assert!(
                kind.health_max() > 0.0,
                "{kind:?} health_max 必须为正数，实际 {}",
                kind.health_max()
            );
        }
    }

    // -----------------------------------------------------------------
    // biome 分池 — 4 biome × 命中物种集
    // -----------------------------------------------------------------

    #[test]
    fn plains_biome_pool_matches_expected_species_set() {
        let pool = mundane_biome_pool("spawn");
        let mut got: Vec<&str> = pool.iter().map(|k| k.as_str()).collect();
        got.sort_unstable();
        assert_eq!(got, vec!["chicken", "cow", "pig", "rabbit", "sheep"]);
    }

    #[test]
    fn unknown_zone_name_falls_back_to_plains_pool() {
        assert_eq!(
            mundane_biome_pool("some_unmapped_zone_name"),
            PLAINS_POOL,
            "未映射的 zone 名应兜底走平原池，不能返回空池"
        );
    }

    #[test]
    fn marsh_biome_pool_matches_expected_species_set() {
        let mut got: Vec<&str> = mundane_biome_pool("lingquan_marsh")
            .iter()
            .map(|k| k.as_str())
            .collect();
        got.sort_unstable();
        assert_eq!(got, vec!["frog", "rabbit"]);
    }

    #[test]
    fn peaks_biome_pool_matches_expected_species_set() {
        let mut got: Vec<&str> = mundane_biome_pool("qingyun_peaks")
            .iter()
            .map(|k| k.as_str())
            .collect();
        got.sort_unstable();
        assert_eq!(got, vec!["goat", "sheep"]);
    }

    #[test]
    fn wastes_biome_pool_matches_expected_species_set() {
        let mut got: Vec<&str> = mundane_biome_pool("north_wastes")
            .iter()
            .map(|k| k.as_str())
            .collect();
        got.sort_unstable();
        assert_eq!(got, vec!["fox", "rabbit", "wolf"]);
    }

    #[test]
    fn biome_pool_name_match_is_case_insensitive() {
        assert_eq!(mundane_biome_pool("QINGYUN_PEAKS"), PEAKS_POOL);
    }

    #[test]
    fn wolf_only_appears_in_wastes_pool() {
        // 补齐狼的落点决策（模块文档已记录）：其余三池不应含狼。
        for name in ["spawn", "lingquan_marsh", "qingyun_peaks"] {
            assert!(
                !mundane_biome_pool(name).contains(&MundaneFaunaKind::Wolf),
                "zone={name} 不应含狼（狼只在 north_wastes 池）"
            );
        }
    }

    // -----------------------------------------------------------------
    // select_mundane_species — 权重抽样
    // -----------------------------------------------------------------

    #[test]
    fn select_mundane_species_returns_none_for_empty_pool() {
        assert_eq!(select_mundane_species(&[], 42), None);
    }

    #[test]
    fn select_mundane_species_deterministic_for_same_seed() {
        let pool = PLAINS_POOL;
        assert_eq!(
            select_mundane_species(pool, 777),
            select_mundane_species(pool, 777),
            "同一 seed 必须产出同一物种（可复现）"
        );
    }

    #[test]
    fn select_mundane_species_exhaustive_over_plains_pool() {
        // PLAINS_POOL 5 条目，seed 0..5 应恰好遍历全部各一次（roll = seed % 5）。
        let pool = PLAINS_POOL;
        let mut hit: Vec<MundaneFaunaKind> = (0u64..5)
            .map(|seed| select_mundane_species(pool, seed).expect("非空池必命中"))
            .collect();
        hit.sort_by_key(|k| k.as_str());
        let mut expected: Vec<MundaneFaunaKind> = pool.to_vec();
        expected.sort_by_key(|k| k.as_str());
        assert_eq!(hit, expected);
    }

    // -----------------------------------------------------------------
    // mundane_species_for_position — hydrate 复活口径（不持久化 kind 字段）
    // -----------------------------------------------------------------

    #[test]
    fn species_for_position_is_deterministic_for_same_zone_and_position() {
        let pos = DVec3::new(12.0, 64.0, -8.0);
        assert_eq!(
            mundane_species_for_position("north_wastes", pos),
            mundane_species_for_position("north_wastes", pos),
            "同一 (zone_name, position) 必须复现同一物种——dormant 复活依赖此确定性，\
             不持久化 MundaneFaunaKind 字段"
        );
    }

    #[test]
    fn species_for_position_respects_biome_pool() {
        // north_wastes 池只含 兔/狐/狼，任意坐标结果都必须落在该集合内。
        for x in [0.0, 17.0, -42.5, 999.0] {
            let kind = mundane_species_for_position("north_wastes", DVec3::new(x, 64.0, 0.0));
            assert!(
                WASTES_POOL.contains(&kind),
                "north_wastes 池派生出的物种 {kind:?} 必须属于 {WASTES_POOL:?}"
            );
        }
    }

    // -----------------------------------------------------------------
    // mundane_pool_fn — 真实 AmbientPoolFn 端到端
    // -----------------------------------------------------------------

    #[test]
    fn mundane_pool_fn_spawns_entity_with_species_matching_biome_pool() {
        let mut app = valence::prelude::App::new();
        let layer = app.world_mut().spawn_empty().id();
        let zone = zone_with_name("qingyun_peaks");
        let spawned = {
            let mut commands = app.world_mut().commands();
            mundane_pool_fn(
                &mut commands,
                layer,
                &zone,
                DVec3::new(10.0, 64.0, 10.0),
                DVec3::new(10.0, 64.0, 10.0),
            )
        };
        app.world_mut().flush();
        let entity = spawned.expect("mundane_pool_fn 在 P0 阶段应恒返回 Some");
        let species = app
            .world()
            .get::<crate::fauna::mundane::MundaneFaunaSpecies>(entity)
            .expect("mundane_pool_fn 产出的实体必须带 MundaneFaunaSpecies");
        assert!(
            PEAKS_POOL.contains(&species.0),
            "qingyun_peaks zone 产出的物种 {:?} 必须属于峰区池 {PEAKS_POOL:?}",
            species.0
        );
    }

    // -----------------------------------------------------------------
    // mundane_passive_budget_fn — §8.1 #4 数值 pin
    // -----------------------------------------------------------------

    #[test]
    fn mundane_passive_budget_matches_8_1_4_decision() {
        for danger in [0u8, 1, 4, 7, 255] {
            let budget = mundane_passive_budget_fn(danger);
            assert_eq!(
                budget,
                ThreatBudget {
                    max_alive: 3,
                    spawn_interval_ticks: 400,
                    pack_size_range: (1, 1),
                },
                "danger={danger} 不应改变凡兽 passive 预算（凡兽不分 danger 分级）"
            );
        }
    }

    // -----------------------------------------------------------------
    // MundaneFaunaMarker — AmbientMarkerData 两函数契约
    // -----------------------------------------------------------------

    #[test]
    fn marker_new_and_home_zone_round_trip() {
        let marker = MundaneFaunaMarker::new(1234, "test_zone".to_string());
        assert_eq!(marker.spawned_at, 1234);
        assert_eq!(marker.home_zone(), "test_zone");
    }
}
