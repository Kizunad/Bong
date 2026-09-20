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
use valence::prelude::{
    bevy_ecs, Component, Entity, Event, EventReader, EventWriter, Query, Res, Resource,
};

use crate::cultivation::components::{
    CrackCause, Meridian, MeridianChannelId, MeridianId, MeridianSystem,
};
use crate::cultivation::tick::CultivationClock;

/// 永久断脉登记：玩家 SEVERED 经脉集合 + 断脉时戳与来源。
///
/// 跨 server restart 由 serde 序列化保留；跨周目（新角色）由 `on_player_terminated`
/// 移除 component 实现重置。死脉（接经术失败升级）记录在 `dead_meridians` 子集。
///
/// plan-race-system-v1 P1a：key 类型从闭合枚举 [`MeridianId`] 换轨为 string
/// [`MeridianChannelId`]（旧存档迁移见 `persistence`/`cultivation::mod` 的
/// bundle 迁移函数）。全部公共方法接受 `impl Into<MeridianChannelId>`，既有传
/// `MeridianId::X` 字面量的调用点（各流派 `check_meridian_dependencies` 等）无需改写。
#[derive(Debug, Clone, Default, Component, Serialize, Deserialize, PartialEq)]
pub struct MeridianSeveredPermanent {
    pub severed_meridians: HashSet<MeridianChannelId>,
    pub severed_at: HashMap<MeridianChannelId, SeveredRecord>,
    /// 接经术失败后升级的死脉 — 永远在 severed_meridians 中且无法再尝试 repair。
    pub dead_meridians: HashSet<MeridianChannelId>,
    /// plan-race-system-v1 P5/PR-6a —— RaceChange 换种族时，目标种族
    /// `meridian_profile` 不含的经脉**不会被摧毁**（不是 SEVERED 永久断绝）：完整
    /// `Meridian` 状态原样封存于此，key = 迁出时该经脉的 channel id。换回一个
    /// profile 里恰好含有该 channel id 的种族时（见 `race_change::precheck_race_change`
    /// 消费点），按 id 精确恢复——不新建平行类型，复用本 component 承载"永久性经脉
    /// 状态旁路"的既有语义（决议 #5：本 component 只负责经脉的长期记忆持久化）。
    #[serde(default)]
    pub dormant_meridians: HashMap<MeridianChannelId, Meridian>,
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
    pub fn is_severed(&self, id: impl Into<MeridianChannelId>) -> bool {
        self.severed_meridians.contains(&id.into())
    }

    pub fn is_dead(&self, id: impl Into<MeridianChannelId>) -> bool {
        self.dead_meridians.contains(&id.into())
    }

    pub fn record_for(&self, id: impl Into<MeridianChannelId>) -> Option<&SeveredRecord> {
        self.severed_at.get(&id.into())
    }

    /// 写入 SEVERED。若已 SEVERED：保留首次记录（首次时戳 + 来源不被覆盖），
    /// 返回 false。新写入返回 true。
    pub fn insert(
        &mut self,
        id: impl Into<MeridianChannelId>,
        source: SeveredSource,
        at_tick: u64,
    ) -> bool {
        let id = id.into();
        if self.severed_meridians.contains(&id) {
            return false;
        }
        self.severed_meridians.insert(id.clone());
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
        self.dormant_meridians.clear();
    }

    pub fn severed_count(&self) -> usize {
        self.severed_meridians.len()
    }

    // ─── plan-race-system-v1 P5/PR-6a —— 休眠登记（RaceChange 经脉迁移旁路） ───

    /// 登记一条休眠经脉——`RaceChange` 迁出、目标种族 `meridian_profile` 不含该
    /// channel id 时调用。若该 id 已有休眠记录（理论不可达：同一 channel 不会在
    /// 未恢复前重复迁出），覆盖为最新状态并返回被替换的旧值。
    pub fn register_dormant(&mut self, meridian: Meridian) -> Option<Meridian> {
        self.dormant_meridians.insert(meridian.id.clone(), meridian)
    }

    pub fn is_dormant(&self, id: impl Into<MeridianChannelId>) -> bool {
        self.dormant_meridians.contains_key(&id.into())
    }

    pub fn dormant(&self, id: impl Into<MeridianChannelId>) -> Option<&Meridian> {
        self.dormant_meridians.get(&id.into())
    }

    /// 取出并移除休眠登记——`RaceChange` 迁回一个 `meridian_profile` 恰好含有该
    /// channel id 的种族时调用，原样恢复迁出前的完整状态（`opened`/`flow_rate`/
    /// `integrity`/`cracks` 等全部字段）。
    pub fn take_dormant(&mut self, id: impl Into<MeridianChannelId>) -> Option<Meridian> {
        self.dormant_meridians.remove(&id.into())
    }

    pub fn dormant_count(&self) -> usize {
        self.dormant_meridians.len()
    }
}

/// 玩家 skill-bar cast 前的经脉门控检查（plan-bug-qc-p1 §skill-cast P0）。
///
/// 覆盖两条来源：
/// 1. `SkillMeridianDependencies` 表（按 skill_id 声明）——检查 `opened` + SEVERED（永久断脉）。
/// 2. `TechniqueDefinition.required_meridians` ——检查 `opened` + SEVERED + `integrity ≥ min_health`。
///
/// **「未打通」（`opened = false`）属拒绝范围**（对齐 NPC 侧 `npc/technique.rs:meridian_deps_satisfied`
/// 与 worldview 正典"经脉没通就放不出招"）。`Meridian::new()` 默认 `opened = false`，纯 integrity
/// 阈值（1.0）永远放行未打通经脉，因此两条路径都必须先判 `opened`。
///
/// 任一条件不满足 → 返回 `Err(blocked_meridian_id)`，调用方应拒绝 cast。
/// 无依赖（两条来源均空）→ 返回 `Ok(())` 放行。
/// `meridians` 未能获取（pre-init 玩家、entity 无 MeridianSystem）→ 调用方应直接放行
/// （在 `handle_skill_bar_cast` 的 `if let Some(meridians) = ...` 分支控制）。
pub(crate) fn check_player_skill_meridian_gate(
    skill_id: &str,
    required_meridians: &[crate::cultivation::known_techniques::TechniqueRequiredMeridian],
    meridians: &MeridianSystem,
    severed: Option<&MeridianSeveredPermanent>,
    deps_table: Option<&SkillMeridianDependencies>,
) -> Result<(), MeridianChannelId> {
    check_skill_channels(required_meridians, meridians, severed)?;
    // 静态依赖也使用开放 channel，避免配置经脉表与技能声明出现门控分歧。
    if let Some(table) = deps_table {
        for dep in table.channel_dependencies(skill_id) {
            if severed
                .is_some_and(|state| state.is_severed(dep.clone()) || state.is_dead(dep.clone()))
                || !meridians
                    .iter()
                    .any(|state| state.id == dep && state.opened)
            {
                return Err(dep);
            }
        }
    }
    Ok(())
}

/// 玩家/NPC 共用的开放经脉门：缺脉、闭脉、伤脉与永久断脉均不可施放。
pub fn check_skill_channels(
    required: &[crate::cultivation::known_techniques::TechniqueRequiredMeridian],
    meridians: &MeridianSystem,
    severed: Option<&MeridianSeveredPermanent>,
) -> Result<(), MeridianChannelId> {
    for requirement in required {
        let channel = crate::cultivation::technique_scroll::technique_channel(&requirement.channel);
        let valid = meridians
            .iter()
            .find(|meridian| meridian.id == channel)
            .is_some_and(|meridian| {
                meridian.opened
                    && meridian.integrity.is_finite()
                    && meridian.integrity >= f64::from(requirement.min_health)
            });
        if !valid
            || severed.is_some_and(|state| {
                state.is_severed(channel.clone()) || state.is_dead(channel.clone())
            })
        {
            return Err(channel);
        }
    }
    Ok(())
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
    let channel_id = id.channel_id();
    if severed.is_dead(channel_id.clone()) {
        return AcupointRepairOutcome::AlreadyDead;
    }
    if !severed.is_severed(channel_id.clone()) {
        return AcupointRepairOutcome::NotSevered;
    }
    if success_roll < success_threshold {
        // 成功 —— 移除 SEVERED 标记 + 时戳。Meridian.integrity 由调用方在外重置。
        severed.severed_meridians.remove(&channel_id);
        severed.severed_at.remove(&channel_id);
        AcupointRepairOutcome::Restored
    } else {
        // 失败 —— 升级为死脉。SEVERED 集合保留（worldview §四:286 已废 + 死脉永久不可逆）。
        severed.dead_meridians.insert(channel_id);
        AcupointRepairOutcome::Failed
    }
}

/// crack cause 派生 SEVERED 来源：用于 detection system 把已落 cracks 的经脉 SEVERED
/// 时正确归因。`Backfire`/`Overload` 都映射到 BackfireOverload —— overload 在
/// worldview §四:354 与反噬同源（强行调动超流量真元）。
pub fn severed_source_from_crack(cause: CrackCause) -> SeveredSource {
    match cause {
        CrackCause::Attack => SeveredSource::CombatWound,
        CrackCause::Overload => SeveredSource::BackfireOverload,
        CrackCause::Backfire => SeveredSource::BackfireOverload,
        CrackCause::ForgeFailure => SeveredSource::Other("forge_failure".to_string()),
        CrackCause::VoluntarySever => SeveredSource::VoluntarySever,
        CrackCause::TribulationFail => SeveredSource::TribulationFail,
        CrackCause::DuguDistortion => SeveredSource::DuguDistortion,
    }
}

/// SEVERED 检测系统：watch `Meridian.integrity ≤ ε` 的过渡，对**未在
/// `MeridianSeveredPermanent.severed_meridians` 里**且**有 cracks 历史**的经脉发
/// `MeridianSeveredEvent`。来源由最新 crack 的 `CrackCause` 派生。
///
/// 这是 7 类来源里 CombatWound / OverloadTear / BackfireOverload 的统一捕获点（cracks
/// 已落）；VoluntarySever / TribulationFail / DuguDistortion 由各自路径**显式 send
/// event**（不需要先落 crack），detection system 看到 SEVERED component 已 set 就跳过。
pub fn meridian_severed_detection_tick(
    clock: Res<CultivationClock>,
    mut targets: Query<(Entity, &mut MeridianSystem, &mut MeridianSeveredPermanent)>,
    mut severed_events: EventWriter<MeridianSeveredEvent>,
) {
    let now = clock.tick;
    for (entity, mut meridians, mut permanent) in &mut targets {
        for m in meridians.iter_mut() {
            if m.integrity > f64::EPSILON {
                continue;
            }
            if permanent.is_severed(m.id.clone()) {
                continue;
            }
            let Some(latest_crack) = m.cracks.iter().max_by_key(|c| c.created_at) else {
                // 经脉 integrity ≤ ε 但无 crack 历史 —— 不写 SEVERED（可能是出生 default
                // 或被显式 close_meridian 调用过；那种情况由调用方决定是否 emit）
                continue;
            };
            // wire 事件暂保留人形枚举；非人形经脉在此完成同样的永久登记。
            let Some(meridian_id) = m.id.to_meridian_id() else {
                // 旧 wire 枚举暂只广播人形经脉；兽脉仍必须完成权威登记与闭脉，
                // 不能仅跳过事件而让技能在之后的修复 tick 中重新可用。
                permanent.insert(
                    m.id.clone(),
                    severed_source_from_crack(latest_crack.cause),
                    now,
                );
                m.opened = false;
                m.integrity = 0.0;
                continue;
            };
            severed_events.send(MeridianSeveredEvent {
                entity,
                meridian_id,
                source: severed_source_from_crack(latest_crack.cause),
                at_tick: now,
            });
        }
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
    channels: HashMap<&'static str, Vec<MeridianChannelId>>,
}

impl SkillMeridianDependencies {
    /// 非人形技能使用开放的 channel ID，旧人形 resolver 保留原签名。
    pub fn declare_channels(&mut self, skill_id: &'static str, deps: Vec<MeridianChannelId>) {
        assert!(
            !self.is_declared(skill_id),
            "duplicate dependency: {skill_id}"
        );
        self.channels.insert(skill_id, deps);
    }

    pub fn channel_dependencies(&self, skill_id: &str) -> Vec<MeridianChannelId> {
        self.channels.get(skill_id).cloned().unwrap_or_else(|| {
            self.lookup(skill_id)
                .iter()
                .map(|id| id.channel_id())
                .collect()
        })
    }
    /// 声明某个 resolver 对经脉的依赖。重复声明意味着两个初始化路径在争夺同一
    /// 技能的门控真源，必须在启动期直接失败，不能悄悄覆盖先前声明。
    pub fn declare(&mut self, skill_id: &'static str, deps: Vec<MeridianId>) {
        assert!(
            !self.is_declared(skill_id),
            "duplicate meridian dependency declaration for skill: {skill_id}"
        );
        self.table.insert(skill_id, deps);
    }

    pub fn lookup(&self, skill_id: &str) -> &[MeridianId] {
        self.table
            .get(skill_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn declared_skills(&self) -> impl Iterator<Item = &&'static str> {
        self.table.keys().chain(self.channels.keys())
    }

    pub fn is_declared(&self, skill_id: &str) -> bool {
        self.table.contains_key(skill_id) || self.channels.contains_key(skill_id)
    }
}
