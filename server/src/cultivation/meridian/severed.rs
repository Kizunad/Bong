//! plan-meridian-severed-v1 主体 — `MeridianSeveredPermanent` component +
//! `MeridianSeveredEvent` + `SeveredSource` 7 类来源 + cast 强约束检查 +
//! `try_acupoint_repair` 接经术接口（plan-yidao-v1 占位）。
//!
//! 决策门拍板（user, 2026-05-08）：
//! - #1 跨周目处理：B —— 写入生平卷（由 plan-life-record 负责），但新角色 SEVERED
//!   重置 INTACT。本模块通过 `on_player_terminated` 移除 component 实现 reset。
//! - #2 接经术失败升级：A —— SEVERED 升级为「死脉」，`Failed` outcome 把对应
//!   meridian 标记 `dead`（无法再尝试），不连带额外伤损。
//! - #3 招式依赖经脉粒度：C —— 混合，本模块提供 `check_meridian_dependencies`
//!   通用工具，每招/每流派自行决定声明粒度。
//! - #4 docs/CLAUDE.md §四 红旗加一条：A —— 但 docs/CLAUDE.md 严禁自动写入，
//!   待用户手动加红旗（Finish Evidence 内备注）。
//! - #5 SEVERED 状态表达：② —— 独立 component 与 `Meridian.cracks/integrity`
//!   共存。本 component 只负责"永久断绝"长期记忆持久化，瞬时损伤仍走 cracks。

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component, Entity, Event, EventReader, Query, Resource};

use crate::cultivation::components::{Meridian, MeridianId, MeridianSystem};

/// 永久断脉登记：玩家 SEVERED 经脉集合 + 断脉时戳与来源。
///
/// 跨 server restart 由 serde 序列化保留；跨周目（新角色）由 `on_player_terminated`
/// 移除 component 实现重置。死脉（接经术失败升级）记录在 `dead_meridians` 子集。
#[derive(Debug, Clone, Default, Component, Serialize, Deserialize, PartialEq, Eq)]
pub struct MeridianSeveredPermanent {
    pub severed_meridians: HashSet<MeridianId>,
    pub severed_at: HashMap<MeridianId, SeveredRecord>,
    /// 接经术失败后升级的死脉 — 永远在 severed_meridians 中且无法再尝试 repair。
    pub dead_meridians: HashSet<MeridianId>,
}

/// 单条 SEVERED 经脉的"出事时戳 + 来源"快照。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeveredRecord {
    pub at_tick: u64,
    pub source: SeveredSource,
}

/// SEVERED 来源 7 类（plan §4）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SeveredSource {
    /// zhenmai-v2 ⑤ 绝脉断链主动 cast（plan-zhenmai-v2 实装）。
    VoluntarySever,
    /// woliu / dugu / zhenmai / baomai 反噬累积超阈值（各流派 v2 实装）。
    BackfireOverload,
    /// worldview §四:354 强行调动超流量真元（plan-baomai-v3 实装）。
    OverloadTear,
    /// 战场被打经脉损伤累积 INTACT→MICRO_TEAR→TORN→SEVERED（plan-combat-no_ui 已部分实装）。
    CombatWound,
    /// 渡劫失败爆脉降境（worldview §三:124-131 + §十二:316，plan-tribulation-v1 接入）。
    TribulationFail,
    /// dugu 阴诡色 90%+ 形貌异化 → 自身经脉慢性侵蚀（worldview §六:621，plan-dugu-v2 实装）。
    DuguDistortion,
    /// 扩展（未来未预见来源）。
    Other(String),
}

/// SEVERED 事件 — 7 类来源都通过本 event 统一写入 `MeridianSeveredPermanent`。
#[derive(Debug, Clone, Event, PartialEq, Eq)]
pub struct MeridianSeveredEvent {
    pub entity: Entity,
    pub meridian_id: MeridianId,
    pub source: SeveredSource,
    pub at_tick: u64,
}

impl MeridianSeveredPermanent {
    pub fn is_severed(&self, id: MeridianId) -> bool {
        self.severed_meridians.contains(&id)
    }

    pub fn is_dead(&self, id: MeridianId) -> bool {
        self.dead_meridians.contains(&id)
    }

    pub fn record_for(&self, id: MeridianId) -> Option<&SeveredRecord> {
        self.severed_at.get(&id)
    }

    /// 写入 SEVERED。若已 SEVERED：保留首次记录（首次时戳 + 来源不被覆盖），
    /// 返回 false。新写入返回 true。
    pub fn insert(&mut self, id: MeridianId, source: SeveredSource, at_tick: u64) -> bool {
        if self.severed_meridians.contains(&id) {
            return false;
        }
        self.severed_meridians.insert(id);
        self.severed_at
            .insert(id, SeveredRecord { at_tick, source });
        true
    }

    /// 跨周目重置：清空所有 SEVERED 与 dead 记录（决策门 #1 = B）。
    /// 由 `on_player_terminated` 在角色彻底死亡时调用；新角色重生后 component 默认空。
    pub fn reset(&mut self) {
        self.severed_meridians.clear();
        self.severed_at.clear();
        self.dead_meridians.clear();
    }

    pub fn severed_count(&self) -> usize {
        self.severed_meridians.len()
    }
}

/// SEVERED 时同步把 `Meridian.integrity` 钳到 0、`opened` 标 false（避免下游
/// 仍把这条经脉算作可用）。返回经脉是否被该次调用真的下推。
///
/// 不直接处理 cracks 列表 —— cracks 表达瞬时损伤，由 overload / heal / combat
/// 各自负责；SEVERED 只是把"不可逆终态"挂到 component 上。
pub fn enforce_severed_state(meridians: &mut MeridianSystem, id: MeridianId) -> bool {
    let m = meridians.get_mut(id);
    let already = m.integrity <= f64::EPSILON && !m.opened;
    set_meridian_severed(m);
    !already
}

fn set_meridian_severed(meridian: &mut Meridian) {
    meridian.integrity = 0.0;
    meridian.opened = false;
    meridian.throughput_current = 0.0;
}

/// 招式依赖经脉强约束检查（plan §3）。
///
/// 返回 `Err(meridian_id)` 时调用方应把 cast 拒绝原因映射为
/// `CastRejectReason::MeridianSevered(meridian_id)`。
pub fn check_meridian_dependencies(
    deps: &[MeridianId],
    severed: Option<&MeridianSeveredPermanent>,
) -> Result<(), MeridianId> {
    let Some(severed) = severed else {
        return Ok(());
    };
    for dep in deps {
        if severed.is_severed(*dep) {
            return Err(*dep);
        }
    }
    Ok(())
}

/// 同时检查永久 SEVERED + 当前 `Meridian.integrity` 是否可用 —— 给未引入永久 component
/// 的旧调用点（如 `burst_meridian` "右臂任一经脉 integrity > ε"）提供平滑迁移。
pub fn check_meridian_runtime_integrity(
    deps: &[MeridianId],
    meridians: &MeridianSystem,
    severed: Option<&MeridianSeveredPermanent>,
) -> Result<(), MeridianId> {
    check_meridian_dependencies(deps, severed)?;
    for dep in deps {
        if meridians.get(*dep).integrity > f64::EPSILON {
            return Ok(());
        }
    }
    // 全部依赖经脉 integrity ≤ ε —— 退化为 "选第一条声明" 作为拒绝原因。
    Err(*deps
        .first()
        .expect("dependencies must be non-empty when all are unusable"))
}

/// 接经术结果（plan §5 + 决策门 #2 = A）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcupointRepairOutcome {
    /// 接经成功，经脉从 SEVERED 列表移除（INTACT 恢复，但 cracks 数值仍由调用方决定）。
    Restored,
    /// 接经失败 —— 该经脉升级为「死脉」无法再尝试（决策门 #2 = A，简洁路径）。
    Failed,
    /// 经脉不在 SEVERED 列表 —— 不该 cast 接经术，调用方上层应已过滤。
    NotSevered,
    /// 已是死脉，拒绝再次尝试。
    AlreadyDead,
}

/// 接经术接口（plan-yidao-v1 占位）。`success_roll ∈ [0,1)` 由调用方按医者境界 +
/// 玩家气运 + 经脉位置 + 已 SEVERED 时长综合派生。`success_threshold ∈ [0,1]` 由
/// 调用方决定（worldview §六:617 医者境界决定成功率）。
pub fn try_acupoint_repair(
    severed: &mut MeridianSeveredPermanent,
    id: MeridianId,
    success_roll: f64,
    success_threshold: f64,
) -> AcupointRepairOutcome {
    if severed.is_dead(id) {
        return AcupointRepairOutcome::AlreadyDead;
    }
    if !severed.is_severed(id) {
        return AcupointRepairOutcome::NotSevered;
    }
    if success_roll < success_threshold {
        // 成功 —— 移除 SEVERED 标记 + 时戳。Meridian.integrity 由调用方在外重置。
        severed.severed_meridians.remove(&id);
        severed.severed_at.remove(&id);
        AcupointRepairOutcome::Restored
    } else {
        // 失败 —— 升级为死脉。SEVERED 集合保留（worldview §四:286 已废 + 死脉永久不可逆）。
        severed.dead_meridians.insert(id);
        AcupointRepairOutcome::Failed
    }
}

/// `MeridianSeveredEvent` 写入 component 的运行时系统。读取 event → 写
/// `MeridianSeveredPermanent` + 把 `Meridian.integrity / opened` 钳到 SEVERED。
pub fn apply_severed_event_system(
    mut events: EventReader<MeridianSeveredEvent>,
    mut targets: Query<(&mut MeridianSeveredPermanent, Option<&mut MeridianSystem>)>,
) {
    for ev in events.read() {
        let Ok((mut severed, meridians)) = targets.get_mut(ev.entity) else {
            tracing::warn!(
                "[bong][cultivation][severed] dropped event {:?} for {:?}: missing MeridianSeveredPermanent",
                ev.source,
                ev.entity,
            );
            continue;
        };
        let inserted = severed.insert(ev.meridian_id, ev.source.clone(), ev.at_tick);
        if let Some(mut meridians) = meridians {
            enforce_severed_state(&mut meridians, ev.meridian_id);
        }
        if inserted {
            tracing::info!(
                "[bong][cultivation][severed] entity={:?} meridian={:?} source={:?} tick={}",
                ev.entity,
                ev.meridian_id,
                ev.source,
                ev.at_tick,
            );
        }
    }
}

/// SkillRegistry 招式依赖经脉表 — Resource，复用现有 `SkillRegistry` 的 fn-pointer
/// 注册风格（plan §3 强约束接口）。
///
/// 注册时写：
/// ```ignore
/// dependencies.declare("zhenmai.parry", vec![MeridianId::Lung, MeridianId::LargeIntestine]);
/// ```
/// cast 前检查：
/// ```ignore
/// let deps = dependencies.lookup("zhenmai.parry");
/// match check_meridian_dependencies(deps, severed) { ... }
/// ```
#[derive(Debug, Default, Resource)]
pub struct SkillMeridianDependencies {
    table: HashMap<&'static str, Vec<MeridianId>>,
}

impl SkillMeridianDependencies {
    pub fn declare(&mut self, skill_id: &'static str, deps: Vec<MeridianId>) {
        self.table.insert(skill_id, deps);
    }

    pub fn lookup(&self, skill_id: &str) -> &[MeridianId] {
        self.table
            .get(skill_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn declared_skills(&self) -> impl Iterator<Item = &&'static str> {
        self.table.keys()
    }

    pub fn is_declared(&self, skill_id: &str) -> bool {
        self.table.contains_key(skill_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{from_str, to_string};
    use valence::prelude::App;

    // --- MeridianSeveredPermanent: 写入 / 重复 / 持久化 / 跨周目重置 (8 tests) ---

    #[test]
    fn permanent_default_is_empty() {
        let p = MeridianSeveredPermanent::default();
        assert_eq!(p.severed_count(), 0);
        assert!(!p.is_severed(MeridianId::Lung));
        assert!(p.record_for(MeridianId::Lung).is_none());
    }

    #[test]
    fn permanent_insert_records_tick_and_source() {
        let mut p = MeridianSeveredPermanent::default();
        let inserted = p.insert(MeridianId::Lung, SeveredSource::CombatWound, 100);
        assert!(inserted, "首次写入应返回 true");
        assert!(p.is_severed(MeridianId::Lung));
        let r = p.record_for(MeridianId::Lung).expect("record should exist");
        assert_eq!(r.at_tick, 100);
        assert_eq!(r.source, SeveredSource::CombatWound);
    }

    #[test]
    fn permanent_insert_duplicate_keeps_first_record() {
        let mut p = MeridianSeveredPermanent::default();
        p.insert(MeridianId::Heart, SeveredSource::CombatWound, 50);
        let again = p.insert(MeridianId::Heart, SeveredSource::TribulationFail, 200);
        assert!(!again, "重复写入应返回 false");
        let r = p
            .record_for(MeridianId::Heart)
            .expect("first record retained");
        assert_eq!(r.at_tick, 50, "首次时戳保留");
        assert_eq!(r.source, SeveredSource::CombatWound, "首次来源保留");
    }

    #[test]
    fn permanent_serde_round_trip_preserves_all_fields() {
        // 跨 server restart 持久化：serde JSON 完整往返
        let mut p = MeridianSeveredPermanent::default();
        p.insert(MeridianId::Lung, SeveredSource::CombatWound, 100);
        p.insert(MeridianId::Du, SeveredSource::TribulationFail, 200);
        p.dead_meridians.insert(MeridianId::Heart);
        let s = to_string(&p).expect("serialize");
        let back: MeridianSeveredPermanent = from_str(&s).expect("deserialize");
        assert_eq!(back, p);
        assert!(back.is_severed(MeridianId::Lung));
        assert!(back.is_severed(MeridianId::Du));
        assert!(back.is_dead(MeridianId::Heart));
    }

    #[test]
    fn permanent_reset_clears_all_state_for_cross_lifecycle() {
        // 决策门 #1 = B：跨周目新角色 SEVERED 重置
        let mut p = MeridianSeveredPermanent::default();
        p.insert(MeridianId::Lung, SeveredSource::CombatWound, 100);
        p.insert(MeridianId::Heart, SeveredSource::TribulationFail, 200);
        p.dead_meridians.insert(MeridianId::Heart);
        p.reset();
        assert_eq!(p.severed_count(), 0);
        assert!(p.dead_meridians.is_empty());
        assert!(p.severed_at.is_empty());
    }

    #[test]
    fn permanent_insert_independent_meridians_accumulates() {
        let mut p = MeridianSeveredPermanent::default();
        p.insert(MeridianId::Lung, SeveredSource::CombatWound, 100);
        p.insert(MeridianId::LargeIntestine, SeveredSource::OverloadTear, 110);
        p.insert(MeridianId::Du, SeveredSource::VoluntarySever, 120);
        assert_eq!(p.severed_count(), 3);
    }

    #[test]
    fn permanent_handles_other_source_payload() {
        let mut p = MeridianSeveredPermanent::default();
        let src = SeveredSource::Other("unforeseen-cause".to_string());
        p.insert(MeridianId::Chong, src.clone(), 999);
        assert_eq!(p.record_for(MeridianId::Chong).unwrap().source, src);
    }

    #[test]
    fn permanent_dead_state_queryable() {
        let mut p = MeridianSeveredPermanent::default();
        p.insert(MeridianId::Liver, SeveredSource::DuguDistortion, 500);
        assert!(!p.is_dead(MeridianId::Liver), "SEVERED ≠ dead 默认");
        p.dead_meridians.insert(MeridianId::Liver);
        assert!(p.is_dead(MeridianId::Liver));
    }

    // --- check_meridian_dependencies: deps INTACT / SEVERED / 多依赖 / 无依赖 (8 tests) ---

    #[test]
    fn check_deps_no_severed_component_passes() {
        // 无 component 表示玩家从未受过 SEVERED 损伤 —— 检查通过。
        let deps = vec![MeridianId::Lung, MeridianId::Heart];
        assert!(check_meridian_dependencies(&deps, None).is_ok());
    }

    #[test]
    fn check_deps_intact_passes() {
        let p = MeridianSeveredPermanent::default();
        let deps = vec![MeridianId::Lung];
        assert!(check_meridian_dependencies(&deps, Some(&p)).is_ok());
    }

    #[test]
    fn check_deps_severed_rejects_with_offending_id() {
        let mut p = MeridianSeveredPermanent::default();
        p.insert(MeridianId::Lung, SeveredSource::CombatWound, 100);
        let deps = vec![MeridianId::Lung];
        assert_eq!(
            check_meridian_dependencies(&deps, Some(&p)),
            Err(MeridianId::Lung)
        );
    }

    #[test]
    fn check_deps_multi_any_severed_rejects() {
        let mut p = MeridianSeveredPermanent::default();
        p.insert(MeridianId::Heart, SeveredSource::TribulationFail, 100);
        let deps = vec![MeridianId::Lung, MeridianId::Heart, MeridianId::Pericardium];
        assert_eq!(
            check_meridian_dependencies(&deps, Some(&p)),
            Err(MeridianId::Heart)
        );
    }

    #[test]
    fn check_deps_returns_first_severed_in_declaration_order() {
        // 强约束：返回声明顺序中首条 SEVERED，方便招式 cast 站给玩家精确反馈
        let mut p = MeridianSeveredPermanent::default();
        p.insert(MeridianId::Heart, SeveredSource::CombatWound, 100);
        p.insert(MeridianId::Lung, SeveredSource::CombatWound, 100);
        let deps_a = vec![MeridianId::Lung, MeridianId::Heart];
        let deps_b = vec![MeridianId::Heart, MeridianId::Lung];
        assert_eq!(
            check_meridian_dependencies(&deps_a, Some(&p)),
            Err(MeridianId::Lung)
        );
        assert_eq!(
            check_meridian_dependencies(&deps_b, Some(&p)),
            Err(MeridianId::Heart)
        );
    }

    #[test]
    fn check_deps_empty_dependencies_passes() {
        let mut p = MeridianSeveredPermanent::default();
        p.insert(MeridianId::Lung, SeveredSource::CombatWound, 100);
        // 无依赖招式：永远不应被 SEVERED 拦截
        assert!(check_meridian_dependencies(&[], Some(&p)).is_ok());
    }

    #[test]
    fn check_runtime_integrity_passes_when_one_dep_intact() {
        // 退化路径：burst_meridian "任一右臂经脉 integrity > ε" 风格
        let mut meridians = MeridianSystem::default();
        meridians.get_mut(MeridianId::Lung).integrity = 0.0;
        meridians.get_mut(MeridianId::LargeIntestine).integrity = 0.5;
        let deps = vec![MeridianId::Lung, MeridianId::LargeIntestine];
        assert!(check_meridian_runtime_integrity(&deps, &meridians, None).is_ok());
    }

    #[test]
    fn check_runtime_integrity_rejects_when_all_deps_at_zero() {
        let mut meridians = MeridianSystem::default();
        for id in [MeridianId::Lung, MeridianId::LargeIntestine] {
            meridians.get_mut(id).integrity = 0.0;
        }
        let deps = vec![MeridianId::Lung, MeridianId::LargeIntestine];
        assert_eq!(
            check_meridian_runtime_integrity(&deps, &meridians, None),
            Err(MeridianId::Lung)
        );
    }

    // --- MeridianSeveredEvent: 7 类来源 + 写入 component (10 tests) ---

    fn run_event_through_system(events: Vec<MeridianSeveredEvent>) -> MeridianSeveredPermanent {
        let mut app = App::new();
        app.add_event::<MeridianSeveredEvent>();
        let entity = app
            .world_mut()
            .spawn((
                MeridianSeveredPermanent::default(),
                MeridianSystem::default(),
            ))
            .id();
        let events = events
            .into_iter()
            .map(|e| MeridianSeveredEvent { entity, ..e })
            .collect::<Vec<_>>();
        for ev in events {
            app.world_mut().send_event(ev);
        }
        app.add_systems(valence::prelude::Update, apply_severed_event_system);
        app.update();
        app.world()
            .entity(entity)
            .get::<MeridianSeveredPermanent>()
            .expect("component remains")
            .clone()
    }

    fn make_event(meridian: MeridianId, source: SeveredSource, tick: u64) -> MeridianSeveredEvent {
        MeridianSeveredEvent {
            entity: Entity::PLACEHOLDER,
            meridian_id: meridian,
            source,
            at_tick: tick,
        }
    }

    #[test]
    fn event_voluntary_sever_writes_component() {
        let p = run_event_through_system(vec![make_event(
            MeridianId::Du,
            SeveredSource::VoluntarySever,
            10,
        )]);
        assert!(p.is_severed(MeridianId::Du));
        assert_eq!(
            p.record_for(MeridianId::Du).unwrap().source,
            SeveredSource::VoluntarySever
        );
    }

    #[test]
    fn event_backfire_overload_writes_component() {
        let p = run_event_through_system(vec![make_event(
            MeridianId::Heart,
            SeveredSource::BackfireOverload,
            20,
        )]);
        assert!(p.is_severed(MeridianId::Heart));
    }

    #[test]
    fn event_overload_tear_writes_component() {
        let p = run_event_through_system(vec![make_event(
            MeridianId::LargeIntestine,
            SeveredSource::OverloadTear,
            30,
        )]);
        assert!(p.is_severed(MeridianId::LargeIntestine));
    }

    #[test]
    fn event_combat_wound_writes_component() {
        let p = run_event_through_system(vec![make_event(
            MeridianId::Bladder,
            SeveredSource::CombatWound,
            40,
        )]);
        assert!(p.is_severed(MeridianId::Bladder));
    }

    #[test]
    fn event_tribulation_fail_writes_component() {
        let p = run_event_through_system(vec![make_event(
            MeridianId::Ren,
            SeveredSource::TribulationFail,
            50,
        )]);
        assert!(p.is_severed(MeridianId::Ren));
    }

    #[test]
    fn event_dugu_distortion_writes_component() {
        let p = run_event_through_system(vec![make_event(
            MeridianId::Liver,
            SeveredSource::DuguDistortion,
            60,
        )]);
        assert!(p.is_severed(MeridianId::Liver));
    }

    #[test]
    fn event_other_writes_component() {
        let p = run_event_through_system(vec![make_event(
            MeridianId::Chong,
            SeveredSource::Other("test-source".to_string()),
            70,
        )]);
        assert!(p.is_severed(MeridianId::Chong));
    }

    #[test]
    fn event_clamps_meridian_integrity_to_zero() {
        // SEVERED event 要把 Meridian.integrity 钳到 0 + opened 标 false
        let mut app = App::new();
        app.add_event::<MeridianSeveredEvent>();
        let mut ms = MeridianSystem::default();
        ms.get_mut(MeridianId::Lung).integrity = 1.0;
        ms.get_mut(MeridianId::Lung).opened = true;
        let entity = app
            .world_mut()
            .spawn((MeridianSeveredPermanent::default(), ms))
            .id();
        app.world_mut().send_event(MeridianSeveredEvent {
            entity,
            meridian_id: MeridianId::Lung,
            source: SeveredSource::CombatWound,
            at_tick: 1,
        });
        app.add_systems(valence::prelude::Update, apply_severed_event_system);
        app.update();
        let ms = app.world().entity(entity).get::<MeridianSystem>().unwrap();
        assert_eq!(ms.get(MeridianId::Lung).integrity, 0.0);
        assert!(!ms.get(MeridianId::Lung).opened);
    }

    #[test]
    fn event_drops_when_component_missing() {
        // 无 MeridianSeveredPermanent 的 entity 不应 panic，event 静默丢弃
        let mut app = App::new();
        app.add_event::<MeridianSeveredEvent>();
        let entity = app.world_mut().spawn(()).id();
        app.world_mut().send_event(MeridianSeveredEvent {
            entity,
            meridian_id: MeridianId::Lung,
            source: SeveredSource::CombatWound,
            at_tick: 1,
        });
        app.add_systems(valence::prelude::Update, apply_severed_event_system);
        app.update();
        // 没 panic 即通过
    }

    #[test]
    fn event_multiple_in_one_tick_writes_all_unique() {
        let p = run_event_through_system(vec![
            make_event(MeridianId::Lung, SeveredSource::CombatWound, 100),
            make_event(MeridianId::Heart, SeveredSource::CombatWound, 100),
            make_event(MeridianId::Du, SeveredSource::TribulationFail, 100),
            // 重复同条经脉应保留首次
            make_event(MeridianId::Lung, SeveredSource::TribulationFail, 999),
        ]);
        assert_eq!(p.severed_count(), 3);
        assert_eq!(
            p.record_for(MeridianId::Lung).unwrap().source,
            SeveredSource::CombatWound,
            "首次 CombatWound 来源被保留"
        );
        assert_eq!(p.record_for(MeridianId::Lung).unwrap().at_tick, 100);
    }

    // --- 持久化 (跨 restart serde 完整 + 组合状态) (6 tests) ---

    #[test]
    fn serde_round_trip_with_seven_source_variants() {
        let mut p = MeridianSeveredPermanent::default();
        let pairs: &[(MeridianId, SeveredSource)] = &[
            (MeridianId::Lung, SeveredSource::VoluntarySever),
            (MeridianId::LargeIntestine, SeveredSource::BackfireOverload),
            (MeridianId::Heart, SeveredSource::OverloadTear),
            (MeridianId::SmallIntestine, SeveredSource::CombatWound),
            (MeridianId::Du, SeveredSource::TribulationFail),
            (MeridianId::Liver, SeveredSource::DuguDistortion),
            (
                MeridianId::Chong,
                SeveredSource::Other("ancient-curse".to_string()),
            ),
        ];
        for (i, (m, s)) in pairs.iter().enumerate() {
            p.insert(*m, s.clone(), i as u64 * 100);
        }
        let s = to_string(&p).expect("serialize");
        let back: MeridianSeveredPermanent = from_str(&s).expect("deserialize");
        assert_eq!(back, p);
        for (m, s) in pairs {
            assert_eq!(back.record_for(*m).unwrap().source, *s);
        }
    }

    #[test]
    fn serde_preserves_dead_meridians_subset() {
        let mut p = MeridianSeveredPermanent::default();
        p.insert(MeridianId::Lung, SeveredSource::CombatWound, 1);
        p.dead_meridians.insert(MeridianId::Lung);
        let back: MeridianSeveredPermanent = from_str(&to_string(&p).unwrap()).unwrap();
        assert!(back.is_dead(MeridianId::Lung));
        assert!(back.is_severed(MeridianId::Lung));
    }

    #[test]
    fn serde_default_round_trip_is_empty() {
        let p = MeridianSeveredPermanent::default();
        let back: MeridianSeveredPermanent = from_str(&to_string(&p).unwrap()).unwrap();
        assert_eq!(back.severed_count(), 0);
    }

    #[test]
    fn cross_lifecycle_reset_via_terminate_then_default() {
        // 模拟跨周目：终结时 reset，下一角色 component default 空
        let mut p = MeridianSeveredPermanent::default();
        p.insert(MeridianId::Lung, SeveredSource::TribulationFail, 100);
        p.dead_meridians.insert(MeridianId::Lung);
        p.reset();
        assert!(!p.is_severed(MeridianId::Lung));
        let new_char = MeridianSeveredPermanent::default();
        assert_eq!(new_char.severed_count(), 0);
    }

    #[test]
    fn enforce_severed_state_clamps_integrity_and_opened() {
        let mut ms = MeridianSystem::default();
        let m = ms.get_mut(MeridianId::Du);
        m.integrity = 0.7;
        m.opened = true;
        m.throughput_current = 5.0;
        let did = enforce_severed_state(&mut ms, MeridianId::Du);
        assert!(did);
        let m = ms.get(MeridianId::Du);
        assert_eq!(m.integrity, 0.0);
        assert!(!m.opened);
        assert_eq!(m.throughput_current, 0.0);
    }

    #[test]
    fn enforce_severed_state_idempotent() {
        let mut ms = MeridianSystem::default();
        let m = ms.get_mut(MeridianId::Du);
        m.integrity = 0.0;
        m.opened = false;
        let did = enforce_severed_state(&mut ms, MeridianId::Du);
        assert!(!did, "已 SEVERED 状态再调用返回 false");
    }

    // --- AcupointRepair: 成功 / 失败升级死脉 / 边界 (8 tests) ---

    #[test]
    fn repair_not_severed_returns_not_severed() {
        let mut p = MeridianSeveredPermanent::default();
        let outcome = try_acupoint_repair(&mut p, MeridianId::Lung, 0.0, 0.5);
        assert_eq!(outcome, AcupointRepairOutcome::NotSevered);
    }

    #[test]
    fn repair_success_removes_severed() {
        let mut p = MeridianSeveredPermanent::default();
        p.insert(MeridianId::Lung, SeveredSource::CombatWound, 100);
        // success_roll < success_threshold → 成功
        let outcome = try_acupoint_repair(&mut p, MeridianId::Lung, 0.1, 0.7);
        assert_eq!(outcome, AcupointRepairOutcome::Restored);
        assert!(!p.is_severed(MeridianId::Lung));
        assert!(p.record_for(MeridianId::Lung).is_none());
    }

    #[test]
    fn repair_failure_marks_dead() {
        let mut p = MeridianSeveredPermanent::default();
        p.insert(MeridianId::Heart, SeveredSource::TribulationFail, 50);
        let outcome = try_acupoint_repair(&mut p, MeridianId::Heart, 0.9, 0.3);
        assert_eq!(outcome, AcupointRepairOutcome::Failed);
        assert!(p.is_dead(MeridianId::Heart));
        assert!(p.is_severed(MeridianId::Heart), "死脉仍在 SEVERED 集合");
    }

    #[test]
    fn repair_already_dead_rejects() {
        let mut p = MeridianSeveredPermanent::default();
        p.insert(MeridianId::Lung, SeveredSource::CombatWound, 100);
        p.dead_meridians.insert(MeridianId::Lung);
        let outcome = try_acupoint_repair(&mut p, MeridianId::Lung, 0.0, 1.0);
        assert_eq!(outcome, AcupointRepairOutcome::AlreadyDead);
    }

    #[test]
    fn repair_threshold_zero_always_fails() {
        let mut p = MeridianSeveredPermanent::default();
        p.insert(MeridianId::Lung, SeveredSource::CombatWound, 1);
        let outcome = try_acupoint_repair(&mut p, MeridianId::Lung, 0.0, 0.0);
        assert_eq!(outcome, AcupointRepairOutcome::Failed);
    }

    #[test]
    fn repair_threshold_one_always_succeeds_for_zero_roll() {
        let mut p = MeridianSeveredPermanent::default();
        p.insert(MeridianId::Lung, SeveredSource::CombatWound, 1);
        let outcome = try_acupoint_repair(&mut p, MeridianId::Lung, 0.0, 1.0);
        assert_eq!(outcome, AcupointRepairOutcome::Restored);
    }

    #[test]
    fn repair_failure_does_not_remove_other_severed() {
        let mut p = MeridianSeveredPermanent::default();
        p.insert(MeridianId::Lung, SeveredSource::CombatWound, 100);
        p.insert(MeridianId::Heart, SeveredSource::CombatWound, 110);
        let _ = try_acupoint_repair(&mut p, MeridianId::Lung, 0.99, 0.5);
        assert!(
            p.is_severed(MeridianId::Heart),
            "Heart 不应受 Lung repair 影响"
        );
    }

    #[test]
    fn repair_success_then_re_sever_starts_clean() {
        // 成功修复后，再次 SEVERED 应记录新时戳与新来源
        let mut p = MeridianSeveredPermanent::default();
        p.insert(MeridianId::Lung, SeveredSource::CombatWound, 100);
        let _ = try_acupoint_repair(&mut p, MeridianId::Lung, 0.1, 0.9);
        let _ = p.insert(MeridianId::Lung, SeveredSource::TribulationFail, 500);
        let r = p.record_for(MeridianId::Lung).unwrap();
        assert_eq!(r.at_tick, 500);
        assert_eq!(r.source, SeveredSource::TribulationFail);
    }

    // --- SkillMeridianDependencies (declared 表) (4 tests) ---

    #[test]
    fn dependencies_default_empty_lookup_returns_empty_slice() {
        let table = SkillMeridianDependencies::default();
        assert!(table.lookup("zhenmai.parry").is_empty());
        assert!(!table.is_declared("zhenmai.parry"));
    }

    #[test]
    fn dependencies_declare_and_lookup() {
        let mut table = SkillMeridianDependencies::default();
        table.declare(
            "zhenmai.parry",
            vec![MeridianId::Lung, MeridianId::LargeIntestine],
        );
        assert_eq!(
            table.lookup("zhenmai.parry"),
            &[MeridianId::Lung, MeridianId::LargeIntestine]
        );
        assert!(table.is_declared("zhenmai.parry"));
    }

    #[test]
    fn dependencies_declare_overwrites_previous() {
        let mut table = SkillMeridianDependencies::default();
        table.declare("baomai.beng_quan", vec![MeridianId::LargeIntestine]);
        table.declare(
            "baomai.beng_quan",
            vec![
                MeridianId::LargeIntestine,
                MeridianId::SmallIntestine,
                MeridianId::TripleEnergizer,
            ],
        );
        assert_eq!(table.lookup("baomai.beng_quan").len(), 3);
    }

    #[test]
    fn dependencies_check_via_check_meridian_dependencies() {
        // 端到端：声明 + check_meridian_dependencies 联合用法
        let mut table = SkillMeridianDependencies::default();
        table.declare(
            "zhenmai.parry",
            vec![MeridianId::Lung, MeridianId::LargeIntestine],
        );
        let mut p = MeridianSeveredPermanent::default();
        p.insert(MeridianId::Lung, SeveredSource::CombatWound, 1);
        let deps = table.lookup("zhenmai.parry").to_vec();
        assert_eq!(
            check_meridian_dependencies(&deps, Some(&p)),
            Err(MeridianId::Lung)
        );
    }
}
