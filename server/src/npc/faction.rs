#![allow(dead_code)]

use std::collections::HashMap;
use std::time::Instant;

use big_brain::prelude::{ActionBuilder, ActionState, Actor, BigBrainSet, Score, ScorerBuilder};
use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, App, Client, Commands, Component, DVec3, Entity, Event, EventReader, EventWriter,
    IntoSystemConfigs, Position, PreUpdate, Query, Res, ResMut, Resource, Update, With,
};

use crate::cultivation::components::Realm;
use crate::npc::lod::{lod_gated_score, NpcLodConfig, NpcLodTick, NpcLodTier};
use crate::npc::navigator::Navigator;
use crate::npc::perf::NpcPerfProbe;
use crate::npc::spatial::NpcSpatialIndex;
use crate::npc::spawn::{spawn_disciple_npc_at, DuelTarget, NpcMarker, NpcSkinSpawnContext};
use crate::skin::NpcSkinFallbackPolicy;
use crate::world::dimension::OverworldLayer;
use crate::world::zone::ZoneRegistry;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactionId {
    Attack,
    Defend,
    Neutral,
}

impl FactionId {
    pub const fn all() -> [Self; 3] {
        [Self::Attack, Self::Defend, Self::Neutral]
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Attack => "attack",
            Self::Defend => "defend",
            Self::Neutral => "neutral",
        }
    }

    pub fn from_str_name(value: &str) -> Option<Self> {
        match value {
            "attack" => Some(Self::Attack),
            "defend" => Some(Self::Defend),
            "neutral" => Some(Self::Neutral),
            _ => None,
        }
    }
}

/// 离屏散修「群体」的匿名稳定 id（plan-offscreen-war-v1 P5 reframe b）。
///
/// 末法残土**无具名宗门**：散修在某 zone 自发聚成涌现集体，集体身份由「区域涌现 + 描述符」
/// 标识（如「{zone}一带散修」），而非「青云猎盟」式专名。同 id ⇒ 同一涌现群体（同进退、
/// 不内斗）；不同 id ⇒ 争夺有限灵气的敌对群体（§七散修利己 + §十灵气零和）。
///
/// `#[serde(transparent)]`：序列化成裸数字（`3` 而非 `{"0":3}`），与 Redis 快照紧凑对齐。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EmergentGroupId(pub u16);

/// 散修群体消长三态（plan-offscreen-war-v1 P5 reframe b）。
///
/// 涌现集体不是静态势力，会随灵气争夺胜负此消彼长：`Rising`（新涌现、正壮大）→ `Stable`
/// （稳固，人口/势力均衡）→ `Waning`（式微，被压制 / 人口流失）。本 commit 仅定义 enum 与
/// 序列化契约，消长状态的 telemetry / census 推进留 commit 2。
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GroupStatus {
    Rising,
    Stable,
    Waning,
}

impl GroupStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rising => "rising",
            Self::Stable => "stable",
            Self::Waning => "waning",
        }
    }

    pub fn from_str_name(value: &str) -> Option<Self> {
        match value {
            "rising" => Some(Self::Rising),
            "stable" => Some(Self::Stable),
            "waning" => Some(Self::Waning),
            _ => None,
        }
    }
}

/// seed 阶段把散修分布到的涌现群体数（plan-offscreen-war-v1 P5 reframe b）。
///
/// `> 2` 解锁多群体互殴——取代 P1 `is_hostile_pair` 的 Attack↔Defend 2-faction 硬上限。
pub const EMERGENT_GROUP_COUNT: u16 = 4;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactionRank {
    Leader,
    #[default]
    Disciple,
    Ally,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lineage {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub master_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub disciple_ids: Vec<String>,
}

impl Lineage {
    pub fn disciple_count(&self) -> u32 {
        self.disciple_ids.len() as u32
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Reputation {
    pub loyalty: f64,
}

impl Default for Reputation {
    fn default() -> Self {
        Self { loyalty: 0.5 }
    }
}

impl Reputation {
    pub fn loyalty(self) -> f64 {
        self.loyalty.clamp(0.0, 1.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MissionId(pub String);

impl MissionId {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionQueue {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub pending: Vec<MissionId>,
}

impl MissionQueue {
    pub fn pending_count(&self) -> u32 {
        self.pending.len() as u32
    }

    pub fn top_mission_id(&self) -> Option<&str> {
        self.pending.first().map(MissionId::as_str)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FactionState {
    pub id: FactionId,
    pub loyalty_bias: f64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub leader_lineage: Option<Lineage>,
    #[serde(skip_serializing_if = "MissionQueue::is_empty", default)]
    pub mission_queue: MissionQueue,
}

impl FactionState {
    pub fn new(id: FactionId) -> Self {
        Self {
            id,
            loyalty_bias: 0.5,
            leader_lineage: None,
            mission_queue: MissionQueue::default(),
        }
    }
}

impl MissionQueue {
    fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Resource)]
pub struct FactionStore {
    pub factions: Vec<FactionState>,
}

impl Default for FactionStore {
    fn default() -> Self {
        Self {
            factions: FactionId::all()
                .into_iter()
                .map(FactionState::new)
                .collect(),
        }
    }
}

impl FactionStore {
    pub fn iter(&self) -> impl Iterator<Item = &FactionState> {
        self.factions.iter()
    }

    pub fn faction_mut(&mut self, faction_id: FactionId) -> Option<&mut FactionState> {
        self.factions
            .iter_mut()
            .find(|faction| faction.id == faction_id)
    }

    pub fn apply_event(
        &mut self,
        event: FactionEventCommand,
    ) -> Result<FactionEventApplied, FactionEventError> {
        let faction = self
            .faction_mut(event.faction_id)
            .ok_or(FactionEventError::UnknownFaction(event.faction_id))?;

        match event.kind {
            FactionEventKind::SetLeader => {
                faction.leader_lineage = Some(Lineage {
                    master_id: None,
                    disciple_ids: Vec::new(),
                });
                Ok(FactionEventApplied {
                    faction_id: faction.id,
                    kind: event.kind,
                    leader_id: None,
                    loyalty_bias: faction.loyalty_bias,
                    mission_queue_size: faction.mission_queue.pending_count(),
                })
            }
            FactionEventKind::ClearLeader => {
                faction.leader_lineage = None;
                Ok(FactionEventApplied {
                    faction_id: faction.id,
                    kind: event.kind,
                    leader_id: None,
                    loyalty_bias: faction.loyalty_bias,
                    mission_queue_size: faction.mission_queue.pending_count(),
                })
            }
            FactionEventKind::SetLeaderLineage => {
                let Some(leader_id) = event.subject_id else {
                    return Err(FactionEventError::MissingSubjectId);
                };
                faction.leader_lineage = Some(Lineage {
                    master_id: Some(leader_id.clone()),
                    disciple_ids: Vec::new(),
                });
                Ok(FactionEventApplied {
                    faction_id: faction.id,
                    kind: event.kind,
                    leader_id: Some(leader_id),
                    loyalty_bias: faction.loyalty_bias,
                    mission_queue_size: faction.mission_queue.pending_count(),
                })
            }
            FactionEventKind::AdjustLoyaltyBias => {
                let Some(delta) = event.loyalty_delta else {
                    return Err(FactionEventError::MissingLoyaltyDelta);
                };
                faction.loyalty_bias = (faction.loyalty_bias + delta).clamp(0.0, 1.0);
                Ok(FactionEventApplied {
                    faction_id: faction.id,
                    kind: event.kind,
                    leader_id: faction
                        .leader_lineage
                        .as_ref()
                        .and_then(|lineage| lineage.master_id.clone()),
                    loyalty_bias: faction.loyalty_bias,
                    mission_queue_size: faction.mission_queue.pending_count(),
                })
            }
            FactionEventKind::EnqueueMission => {
                let Some(mission_id) = event.mission_id else {
                    return Err(FactionEventError::MissingMissionId);
                };
                faction.mission_queue.pending.push(MissionId(mission_id));
                Ok(FactionEventApplied {
                    faction_id: faction.id,
                    kind: event.kind,
                    leader_id: faction
                        .leader_lineage
                        .as_ref()
                        .and_then(|lineage| lineage.master_id.clone()),
                    loyalty_bias: faction.loyalty_bias,
                    mission_queue_size: faction.mission_queue.pending_count(),
                })
            }
            FactionEventKind::PopMission => {
                if !faction.mission_queue.pending.is_empty() {
                    faction.mission_queue.pending.remove(0);
                }
                Ok(FactionEventApplied {
                    faction_id: faction.id,
                    kind: event.kind,
                    leader_id: faction
                        .leader_lineage
                        .as_ref()
                        .and_then(|lineage| lineage.master_id.clone()),
                    loyalty_bias: faction.loyalty_bias,
                    mission_queue_size: faction.mission_queue.pending_count(),
                })
            }
        }
    }

    pub fn is_hostile_pair(&self, left: FactionId, right: FactionId) -> bool {
        matches!(
            (left, right),
            (FactionId::Attack, FactionId::Defend) | (FactionId::Defend, FactionId::Attack)
        )
    }

    /// 离屏散修群体敌对判定（plan-offscreen-war-v1 P5 reframe b §十灵气零和）。
    ///
    /// 同 zone 内**不同涌现群体**争夺有限灵气即敌对（§七散修利己）；同群体散修不内斗。
    /// 取代离屏路径 `is_hostile_pair` 的 Attack↔Defend 2-faction 硬上限（§10.1 #1），
    /// 因为「不同 id 即敌对」天然支持 `> 2` 个群体两两互殴。
    ///
    /// 注：hydrated AI 的 `is_hostile_pair(FactionId)`（big-brain `DuelTarget`，本文件
    /// `assign_hostile_encounters` / `abstract_combat_system`）是**另一层身份模型，保持不变**——
    /// 本方法只服务离屏 dormant 战斗的群体身份。
    pub fn are_hostile(&self, a: EmergentGroupId, b: EmergentGroupId) -> bool {
        a != b
    }

    /// plan-faction-expansion-v1 P0：两具名势力交战→映射回现有 FactionId 二元 war 模型。
    ///
    /// 约定 a=发起/进攻方→Attack，b=防守方→Defend；返回 (FactionId, FactionId)
    /// 直接喂现有 `is_hostile_pair`（无需改 war 逻辑），Attack/Defend 语义不变。
    ///
    /// P1 faction-wars 真接战时仍走此映射，NamedFactionId 经此兼容层桥接现有 hydrated AI
    /// 战斗二元模型（防孤岛 #2）。
    ///
    /// # 前置断言
    /// `a != b`——同势力不交战（调试模式断言，生产不 panic）。
    pub fn faction_id_for_war(a: NamedFactionId, b: NamedFactionId) -> (FactionId, FactionId) {
        debug_assert!(a != b, "同势力不交战：faction_id_for_war 要求 a != b");
        (FactionId::Attack, FactionId::Defend)
    }

    /// 从旧 `FactionId` 派生涌现群体 id（plan-offscreen-war-v1 P5 reframe b 非破坏迁移）。
    ///
    /// 旧持久化快照只有 `faction` 没有显式 `emergent_group`，反序列化后用本方法回退派生：
    /// `Attack → 群体 0`、`Defend → 群体 1`、`Neutral → None`。**Neutral → None 关键**：保
    /// 中立散修离屏仍非战斗（既有 `no_hostile_pair_yields_no_combat` 测试依赖——None 群体
    /// 进不了 `are_hostile` 配对）。
    pub fn emergent_group_from_faction(&self, faction_id: FactionId) -> Option<EmergentGroupId> {
        match faction_id {
            FactionId::Attack => Some(EmergentGroupId(0)),
            FactionId::Defend => Some(EmergentGroupId(1)),
            FactionId::Neutral => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactionEventKind {
    SetLeader,
    ClearLeader,
    SetLeaderLineage,
    AdjustLoyaltyBias,
    EnqueueMission,
    PopMission,
}

impl FactionEventKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SetLeader => "set_leader",
            Self::ClearLeader => "clear_leader",
            Self::SetLeaderLineage => "set_leader_lineage",
            Self::AdjustLoyaltyBias => "adjust_loyalty_bias",
            Self::EnqueueMission => "enqueue_mission",
            Self::PopMission => "pop_mission",
        }
    }

    pub fn from_str_name(value: &str) -> Option<Self> {
        match value {
            "set_leader" => Some(Self::SetLeader),
            "clear_leader" => Some(Self::ClearLeader),
            "set_leader_lineage" => Some(Self::SetLeaderLineage),
            "adjust_loyalty_bias" => Some(Self::AdjustLoyaltyBias),
            "enqueue_mission" => Some(Self::EnqueueMission),
            "pop_mission" => Some(Self::PopMission),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FactionEventCommand {
    pub faction_id: FactionId,
    pub kind: FactionEventKind,
    pub subject_id: Option<String>,
    pub mission_id: Option<String>,
    pub loyalty_delta: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FactionEventApplied {
    pub faction_id: FactionId,
    pub kind: FactionEventKind,
    pub leader_id: Option<String>,
    pub loyalty_bias: f64,
    pub mission_queue_size: u32,
}

#[derive(Clone, Debug, Event)]
pub struct FactionEventNotice {
    pub applied: FactionEventApplied,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FactionEventError {
    UnknownFaction(FactionId),
    MissingSubjectId,
    MissingMissionId,
    MissingLoyaltyDelta,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Component)]
pub struct FactionMembership {
    pub faction_id: FactionId,
    pub rank: FactionRank,
    #[serde(default)]
    pub reputation: Reputation,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub lineage: Option<Lineage>,
    #[serde(skip_serializing_if = "MissionQueue::is_empty", default)]
    pub mission_queue: MissionQueue,
}

/// Disciple 执行任务的停留位置（挂在 actor 上，由 MissionExecuteAction 使用）。
/// MissionExecuteAction 本身依赖 plan-quest-v1 落实剧本，本 plan 只维护最小状态机。
#[derive(Clone, Copy, Debug, Default, Component)]
pub struct MissionExecuteState {
    pub elapsed_ticks: u32,
}

/// 派系忠诚度评分：读 entity 的 FactionMembership.reputation.loyalty，
/// 加上所在 faction 的 loyalty_bias，给 0..=1。
/// Disciple thinker 用此决定是否服从派系任务。
#[derive(Clone, Copy, Debug, Component)]
pub struct LoyaltyScorer;

/// 待办任务数量评分：FactionMembership.mission_queue.pending 越多分越高，
/// 上限 1.0 在 pending >= `MISSION_QUEUE_SCORER_CAP` 时达到。
#[derive(Clone, Copy, Debug, Component)]
pub struct MissionQueueScorer;

/// MissionExecuteAction 占位 Action：由 plan-quest-v1 承接，本 plan 仅给
/// "弟子抽任务 → 原地走流程 → 超时 Success → 弹出一个 mission" 的最小骨架，
/// 避免 disciple thinker 没有下游出口。
#[derive(Clone, Copy, Debug, Component)]
pub struct MissionExecuteAction;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Component)]
pub struct NamedFactionLeader {
    pub faction: NamedFactionId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Component)]
pub struct FactionZoneClaim {
    pub faction: NamedFactionId,
    pub zone: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Resource)]
pub struct FactionZoneClaims {
    pub claims: Vec<FactionZoneClaim>,
}

#[derive(Clone, Copy, Debug, Component)]
pub struct FactionLeaderTerritoryScorer;

#[derive(Clone, Copy, Debug, Component)]
pub struct FactionLeaderPatrolAction;

#[derive(Clone, Copy, Debug, Default, Component)]
pub struct FactionLeaderPatrolState {
    pub elapsed_ticks: u32,
    pub toll_notices_emitted: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FactionLeaderTollTargetKind {
    Npc,
    Player,
}

#[derive(Clone, Debug, PartialEq, Eq, Event)]
pub struct FactionLeaderTollNotice {
    pub faction: NamedFactionId,
    pub zone: String,
    pub target: Entity,
    pub target_kind: FactionLeaderTollTargetKind,
}

/// MissionQueueScorer 饱和阈值：pending ≥ 此值时分数封顶 1.0。
pub const MISSION_QUEUE_SCORER_CAP: u32 = 3;
/// Disciple 执行单个任务的最大 tick 数（超时 Success，避免卡死）。
pub const MISSION_EXECUTE_MAX_TICKS: u32 = 600;
pub const FACTION_LEADER_PATROL_SCORE: f32 = 0.72;
pub const FACTION_LEADER_PATROL_MAX_TICKS: u32 = 100;

impl FactionZoneClaims {
    pub fn from_registry(registry: &NamedFactionRegistry) -> Self {
        let claims = registry
            .iter()
            .map(|faction| FactionZoneClaim {
                faction: faction.id,
                zone: faction.zone_anchor.clone(),
            })
            .collect();
        Self { claims }
    }

    pub fn get(&self, faction: NamedFactionId) -> Option<&FactionZoneClaim> {
        self.claims.iter().find(|claim| claim.faction == faction)
    }
}

impl ScorerBuilder for LoyaltyScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }
    fn label(&self) -> Option<&str> {
        Some("LoyaltyScorer")
    }
}

impl ScorerBuilder for MissionQueueScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }
    fn label(&self) -> Option<&str> {
        Some("MissionQueueScorer")
    }
}

impl ScorerBuilder for FactionLeaderTerritoryScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }
    fn label(&self) -> Option<&str> {
        Some("FactionLeaderTerritoryScorer")
    }
}

impl ActionBuilder for MissionExecuteAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }
    fn label(&self) -> Option<&str> {
        Some("MissionExecuteAction")
    }
}

impl ActionBuilder for FactionLeaderPatrolAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }
    fn label(&self) -> Option<&str> {
        Some("FactionLeaderPatrolAction")
    }
}

// ── plan-faction-expansion-v1 P0：具名势力注册表 ────────────────────────────────
//
// 三具名势力（QingyunHunters / CangyuanMerchants / NorthWasteDrifters）是末法残土已知的
// 有组织散修集体，与离屏涌现群体（EmergentGroupId）**正交**——后者是匿名动态集体，前者是
// 固定势力档案。NamedFactionId 不替换 FactionId；FactionId::Attack/Defend/Neutral
// 及 FactionMembership 一律不变；通过兼容层 faction_id_for_war 映射到现有 war 二元模型。
//
// zone_anchor 用 zone.rs 字符串体系（如 "qingyun_peaks"），对齐 world/zone.rs；
// plan 文案写 ZoneId 此处实现落 &'static str/String，注释说明决议。
/// zone 锚点字符串别名（对齐 world/zone.rs 字符串 zone 体系，非 enum）。
pub type ZoneAnchor = &'static str;

/// plan-faction-expansion-v1 P0：三具名散修势力 id。
///
/// 与 FactionId（attack/defend/neutral 战斗二元模型）正交；通过 `FactionStore::faction_id_for_war`
/// 兼容层桥接。变体命名对齐 plan；序列化为 snake_case（TypeBox 双端 `NamedFactionEntryV1.id`）。
///
/// # 正典依据
/// - QingyunHunters：docs/library/peoples/宗门残息.json（青云宗外门三脉、第一脉「锋锐三十二人」）
/// - NorthWasteDrifters：docs/library/geography/北荒坍缩渊记.json（入者十归者不过百）
/// - CangyuanMerchants：plan-faction-expansion-v1 创作设定，正典无「沧渊/盐商」直接记载，
///   推演自 worldview §13 血谷矿脉经济生态（见注释）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NamedFactionId {
    /// 青云猎盟——青云宗外门残脉，以猎兽护矿为生。正典：宗门残息.json 外门第一/三脉。
    QingyunHunters,
    /// 沧渊商会——盘踞血谷矿道的骨币商会。
    /// 正典无「沧渊/盐商」直接记载，推演自 worldview §13 血谷矿脉经济生态；名取自 plan-faction-expansion-v1。
    CangyuanMerchants,
    /// 北荒漂流者——入北荒寻坍缩渊遗物的游荡散修松散结盟。
    /// 正典依据：北荒坍缩渊记.json（入者十归者不过百），初始 Headless 有正典支撑。
    NorthWasteDrifters,
}

impl NamedFactionId {
    pub const fn all() -> [Self; 3] {
        [
            Self::QingyunHunters,
            Self::CangyuanMerchants,
            Self::NorthWasteDrifters,
        ]
    }

    /// 正典显示名，对应 docs/worldview.md + docs/library/ 正典依据。
    pub const fn display_name(self) -> &'static str {
        match self {
            // 正典：宗门残息.json 外门残脉，猎盟非正典专名，对应外门第一/三脉散修残余。
            Self::QingyunHunters => "青云猎盟",
            // 正典无「沧渊商会」直接记载，推演自 worldview §13 血谷矿脉经济生态；非正典专名。
            Self::CangyuanMerchants => "沧渊商会",
            // 正典：北荒坍缩渊记.json 游荡散修；无固定领袖，成员替换率极高。
            Self::NorthWasteDrifters => "北荒漂流者",
        }
    }

    /// zone 锚点字符串（对齐 world/zone.rs 字符串 zone 体系；plan 文案写 ZoneId，实现落 &'static str）。
    pub const fn zone_anchor(self) -> ZoneAnchor {
        match self {
            // 青云残峰——worldview §13 + zone.rs terrain profile "qingyun_peaks"。
            Self::QingyunHunters => "qingyun_peaks",
            // 裂谷·血谷——worldview §13 坐标(3000,-2500)，zone.rs:447 blood_valley。
            // plan 文案「裂谷·血谷」对应 blood_valley（rift_valley 同区另名，P1 可扩）。
            Self::CangyuanMerchants => "blood_valley",
            // 北荒·灵泉沼主锚——worldview §13 坐标(0,-7000)，zone.rs:453 north_wastes。
            Self::NorthWasteDrifters => "north_wastes",
        }
    }

    /// lore 标签，供叙事 agent 参考（非正典字段已注释标注来源）。
    pub const fn lore_tag(self) -> &'static str {
        match self {
            // 正典：宗门残息.json 外门第一脉「锋锐三十二人」，掌脉「沉舟」凝脉中期；
            // 「猎盟/护山堂」非正典专名，此处描述对应外门残脉守矿生态。
            Self::QingyunHunters => "青云宗外门残脉,以猎兽护矿为生,收保护费维系松散盟约",
            // 正典无「沧渊/盐商」直接记载；推演自 worldview §13 血谷矿脉经济生态；plan 创作设定。
            Self::CangyuanMerchants => "盘踞血谷矿道的骨币商会,以货仓为据点垄断矿脉供给,见利翻脸",
            // 正典：北荒坍缩渊记.json（入者十归者不过百）+ 散修百态.json（游荡者原型）。
            Self::NorthWasteDrifters => {
                "入北荒寻坍缩渊遗物的游荡散修松散结盟,成员替换率极高,无固定领袖"
            }
        }
    }

    /// snake_case 字符串（与 serde 序列化一致，供 schema/migration 串行）。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::QingyunHunters => "qingyun_hunters",
            Self::CangyuanMerchants => "cangyuan_merchants",
            Self::NorthWasteDrifters => "north_waste_drifters",
        }
    }

    /// 从 snake_case 字符串反序列化（与 FactionId::from_str_name 风格一致）。
    pub fn from_str_name(value: &str) -> Option<Self> {
        match value {
            "qingyun_hunters" => Some(Self::QingyunHunters),
            "cangyuan_merchants" => Some(Self::CangyuanMerchants),
            "north_waste_drifters" => Some(Self::NorthWasteDrifters),
            _ => None,
        }
    }
}

/// plan-faction-expansion-v1 P0：领袖存活态（具名势力用，不影响 FactionId 现有战斗状态）。
///
/// Active=有确定领袖 / Headless=无领袖（北荒漂流者初始态；正典：坍缩渊记无法组织化）/
/// Decayed=势力已式微（领袖陨落后无人接续）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactionStatus {
    /// 有确定领袖，势力正常运转。
    Active,
    /// 无领袖——初始态（北荒漂流者）或领袖死亡后尚未补位。
    /// 正典：北荒坍缩渊记.json「入者十归者不过百」，松散结盟无法组织化。
    Headless,
    /// 势力式微——领袖陨落后无人接续，P2 census 填充后可迁入此态。
    Decayed,
}

impl FactionStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Headless => "headless",
            Self::Decayed => "decayed",
        }
    }

    pub fn from_str_name(value: &str) -> Option<Self> {
        match value {
            "active" => Some(Self::Active),
            "headless" => Some(Self::Headless),
            "decayed" => Some(Self::Decayed),
            _ => None,
        }
    }
}

/// plan-faction-expansion-v1 P0：具名势力注册表条目。
///
/// display_name/zone_anchor 从 `NamedFactionId` 派生填入（避免手抄）；
/// current_npc_count P0 初始 0，P2 census 填充。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NamedFaction {
    pub id: NamedFactionId,
    pub display_name: String,
    /// zone 锚点字符串（对齐 world/zone.rs 字符串 zone 体系；plan 文案写 ZoneId，实现落 String）。
    pub zone_anchor: String,
    /// P0 初始 0；P2 census 按 zone_anchor 计入 hydrated NPC 填充。
    pub current_npc_count: u32,
    pub status: FactionStatus,
    pub is_active: bool,
}

impl NamedFaction {
    /// 从 NamedFactionId + FactionStatus 构造（派生 display_name/zone_anchor，避免手抄）。
    pub fn from_id(id: NamedFactionId, status: FactionStatus) -> Self {
        Self {
            id,
            display_name: id.display_name().to_string(),
            zone_anchor: id.zone_anchor().to_string(),
            current_npc_count: 0,
            status,
            is_active: status != FactionStatus::Decayed,
        }
    }

    pub fn set_status(&mut self, status: FactionStatus) {
        self.status = status;
        self.is_active = status != FactionStatus::Decayed;
    }
}

/// plan-faction-expansion-v1 P0：具名势力注册表 Bevy Resource。
///
/// 启动时在 `register()` 中 `insert_resource(NamedFactionRegistry::startup_default())`，
/// 与 `FactionStore` 同处注册，确保启动即三条可查（防孤岛 #1）。
///
/// 下游消费契约（P0 标注，P1 实现）：
/// - social-v2 WarReputation 按 (NamedFactionId, NamedFactionId) 累积
/// - faction-wars FactionWarEventV1 携 NamedFactionId 发起/防守方
///
/// P1: faction-wars consumes NamedFactionId via faction_id_for_war
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Resource)]
pub struct NamedFactionRegistry {
    pub factions: Vec<NamedFaction>,
}

impl NamedFactionRegistry {
    /// 启动默认注册表：三条；北荒漂流者初始 Headless（正典支撑），其余 Active。
    pub fn startup_default() -> Self {
        Self {
            factions: vec![
                NamedFaction::from_id(NamedFactionId::QingyunHunters, FactionStatus::Active),
                NamedFaction::from_id(NamedFactionId::CangyuanMerchants, FactionStatus::Active),
                NamedFaction::from_id(NamedFactionId::NorthWasteDrifters, FactionStatus::Headless),
            ],
        }
    }

    pub fn get(&self, id: NamedFactionId) -> Option<&NamedFaction> {
        self.factions.iter().find(|f| f.id == id)
    }

    pub fn get_mut(&mut self, id: NamedFactionId) -> Option<&mut NamedFaction> {
        self.factions.iter_mut().find(|f| f.id == id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &NamedFaction> {
        self.factions.iter()
    }
}

// ── plan-faction-expansion-v1 P1：三值关系枚举 + 关系矩阵 Resource ───────────────

/// plan-faction-expansion-v1 P1：具名势力间关系的三值枚举。
///
/// - `Hostile`：两势力互为敌对，NPC 相遇会触发 DuelTarget 并进行战斗。
/// - `Neutral`：两势力互不干涉，不主动开战；NPC 相遇默认无 DuelTarget。
/// - `Pact`：两势力盟约，NPC 绝不互打（scorer 施加 -0.3 反偏置）。
///
/// serde 序列化为 snake_case，便于 Redis 快照/schema 层互通。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactionRelation {
    Hostile,
    Neutral,
    Pact,
}

impl FactionRelation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Hostile => "hostile",
            Self::Neutral => "neutral",
            Self::Pact => "pact",
        }
    }

    pub fn from_str_name(value: &str) -> Option<Self> {
        match value {
            "hostile" => Some(Self::Hostile),
            "neutral" => Some(Self::Neutral),
            "pact" => Some(Self::Pact),
            _ => None,
        }
    }
}

/// plan-faction-expansion-v1 P1：具名势力关系矩阵 Bevy Resource。
///
/// 以 `(NamedFactionId, NamedFactionId)` 有序对（key 规范化：小者在前）存储关系；
/// `are_hostile` 对称处理——`(a,b)` 与 `(b,a)` 查同一条记录。
///
/// v1 初值（startup_default）：
/// - (QingyunHunters, CangyuanMerchants)  = Neutral
/// - (QingyunHunters, NorthWasteDrifters) = Hostile
/// - (CangyuanMerchants, NorthWasteDrifters) = Neutral
///
/// 未注册对组默认 Neutral（不敌对），防止遗漏配置触发意外战斗。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Resource)]
pub struct FactionRelationMatrix {
    /// 规范化 key（variant ordinal 小者先）→ 关系三值。
    relations: HashMap<(NamedFactionId, NamedFactionId), FactionRelation>,
}

/// 规范化一对 NamedFactionId key，小 ordinal 先，保证 (a,b)==(b,a) 查同一条。
fn canonical_pair(a: NamedFactionId, b: NamedFactionId) -> (NamedFactionId, NamedFactionId) {
    // NamedFactionId::all() 顺序：QingyunHunters < CangyuanMerchants < NorthWasteDrifters。
    // 用 all().iter().position() 算序列号作为比较键，保证稳定。
    let ord = |id: NamedFactionId| {
        NamedFactionId::all()
            .iter()
            .position(|x| *x == id)
            .unwrap_or(usize::MAX)
    };
    if ord(a) <= ord(b) {
        (a, b)
    } else {
        (b, a)
    }
}

impl FactionRelationMatrix {
    /// v1 启动初值：三对关系（设计收口）。
    pub fn startup_default() -> Self {
        let mut relations = HashMap::new();
        // (QingyunHunters, CangyuanMerchants) = Neutral — 猎盟与商会各守一方，互不开战。
        relations.insert(
            canonical_pair(
                NamedFactionId::QingyunHunters,
                NamedFactionId::CangyuanMerchants,
            ),
            FactionRelation::Neutral,
        );
        // (QingyunHunters, NorthWasteDrifters) = Hostile — 猎盟排斥闯入北荒的游荡者。
        relations.insert(
            canonical_pair(
                NamedFactionId::QingyunHunters,
                NamedFactionId::NorthWasteDrifters,
            ),
            FactionRelation::Hostile,
        );
        // (CangyuanMerchants, NorthWasteDrifters) = Neutral — 商会有时雇佣漂流者，暂中立。
        relations.insert(
            canonical_pair(
                NamedFactionId::CangyuanMerchants,
                NamedFactionId::NorthWasteDrifters,
            ),
            FactionRelation::Neutral,
        );
        Self { relations }
    }

    /// 查询两具名势力的关系。对称：`(a,b)` 与 `(b,a)` 返回相同结果。
    /// 未注册对组返回 `Neutral`（保守默认，防止意外战斗）。
    pub fn get(&self, a: NamedFactionId, b: NamedFactionId) -> FactionRelation {
        let key = canonical_pair(a, b);
        self.relations
            .get(&key)
            .copied()
            .unwrap_or(FactionRelation::Neutral)
    }

    /// 两具名势力是否互为敌对（`FactionRelation::Hostile`）。对称。
    pub fn are_hostile(&self, a: NamedFactionId, b: NamedFactionId) -> bool {
        self.get(a, b) == FactionRelation::Hostile
    }

    /// 设置两势力关系（测试/运行时动态调整用）。对称写入规范化 key。
    pub fn set(&mut self, a: NamedFactionId, b: NamedFactionId, relation: FactionRelation) {
        let key = canonical_pair(a, b);
        self.relations.insert(key, relation);
    }
}

/// plan-faction-expansion-v1 P1：NPC 具名势力归属 Component。
///
/// 挂在 hydrated NPC entity 上，表明该 NPC 隶属某具名势力。
/// `assign_hostile_encounters` 优先用此 component + `FactionRelationMatrix` 判断敌对，
/// fallback 到旧 `FactionMembership.faction_id` + `FactionStore::is_hostile_pair`。
///
/// P1 只添加 component 定义；NPC spawn 时由具体 archetype plugin 按 zone_anchor 附加。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Component)]
pub struct NamedFactionMembership {
    pub faction_id: NamedFactionId,
}

#[derive(Clone, Debug, PartialEq, Eq, Event, Serialize, Deserialize)]
pub struct NamedFactionLeaderDownEvent {
    pub faction: NamedFactionId,
    pub zone: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Event, Serialize, Deserialize)]
pub struct NamedFactionDecayEvent {
    pub faction: NamedFactionId,
    pub final_zone: String,
    pub last_npc_count: u32,
}

pub fn register(app: &mut App) {
    let named_registry = NamedFactionRegistry::startup_default();
    let claims = FactionZoneClaims::from_registry(&named_registry);
    app.insert_resource(FactionStore::default())
        // plan-faction-expansion-v1 P0：具名势力注册表，启动即 3 条可查（防孤岛 #1）。
        .insert_resource(named_registry)
        // plan-faction-expansion-v1 P2：具名势力地盘 claim，按 registry.zone_anchor 派生。
        .insert_resource(claims)
        // plan-faction-expansion-v1 P1：三势力关系矩阵，startup_default 写入三对关系。
        .insert_resource(FactionRelationMatrix::startup_default())
        .add_event::<FactionEventNotice>()
        .add_event::<FactionLeaderTollNotice>()
        .add_event::<NamedFactionLeaderDownEvent>()
        .add_event::<NamedFactionDecayEvent>();
    app.add_systems(Update, assign_hostile_encounters)
        .add_systems(
            Update,
            (
                spawn_named_faction_leaders_on_startup,
                sync_named_faction_census_system,
                handle_faction_leader_toll_notices_system,
            )
                .chain(),
        )
        .add_systems(
            PreUpdate,
            (
                loyalty_scorer_system,
                mission_queue_scorer_system,
                faction_leader_territory_scorer_system,
            )
                .in_set(BigBrainSet::Scorers),
        )
        .add_systems(
            PreUpdate,
            (
                mission_execute_action_system,
                faction_leader_patrol_action_system,
            )
                .in_set(BigBrainSet::Actions),
        );
}

fn spawn_named_faction_leaders_on_startup(
    mut commands: Commands,
    registry: Option<Res<NamedFactionRegistry>>,
    claims: Option<Res<FactionZoneClaims>>,
    zones: Option<Res<ZoneRegistry>>,
    layers: Query<Entity, With<OverworldLayer>>,
    leaders: Query<&NamedFactionLeader>,
    mut done: valence::prelude::Local<bool>,
) {
    if *done {
        return;
    }
    let Some(registry) = registry.as_deref() else {
        return;
    };
    let Some(claims) = claims.as_deref() else {
        return;
    };
    let Some(zones) = zones.as_deref() else {
        return;
    };
    let Some(layer) = layers.iter().next() else {
        return;
    };

    let existing = leaders
        .iter()
        .map(|leader| leader.faction)
        .collect::<Vec<_>>();
    let skin_policy = NpcSkinFallbackPolicy::AllowFallback;
    for faction in registry
        .iter()
        .filter(|faction| faction.status == FactionStatus::Active)
    {
        if existing.contains(&faction.id) {
            continue;
        }
        let Some(claim) = claims.get(faction.id) else {
            tracing::warn!(
                "[bong][faction] leader spawn skipped for {:?}: missing FactionZoneClaim",
                faction.id
            );
            continue;
        };
        let Some(zone) = zones.find_zone_by_name(claim.zone.as_str()) else {
            tracing::warn!(
                "[bong][faction] leader spawn skipped for {:?}: zone `{}` missing",
                faction.id,
                claim.zone
            );
            continue;
        };
        let spawn_position = zone.patrol_target(0);
        let entity = spawn_disciple_npc_at(
            &mut commands,
            NpcSkinSpawnContext::new(None, skin_policy),
            layer,
            zone.name.as_str(),
            spawn_position,
            zone.center(),
            legacy_faction_id_for_named_faction(faction.id),
            FactionRank::Leader,
            leader_realm_for(faction.id),
            None,
            0.0,
        );
        commands.entity(entity).insert((
            NamedFactionLeader {
                faction: faction.id,
            },
            NamedFactionMembership {
                faction_id: faction.id,
            },
            claim.clone(),
            FactionLeaderPatrolState::default(),
        ));
    }
    *done = true;
}

fn sync_named_faction_census_system(
    mut registry: Option<ResMut<NamedFactionRegistry>>,
    members: Query<&NamedFactionMembership, With<NpcMarker>>,
    leaders: Query<&NamedFactionLeader, With<NpcMarker>>,
    mut memberships: Query<&mut crate::social::components::FactionMembership, With<Client>>,
    mut leader_down_events: EventWriter<NamedFactionLeaderDownEvent>,
    mut decay_events: EventWriter<NamedFactionDecayEvent>,
) {
    let Some(registry) = registry.as_deref_mut() else {
        return;
    };
    let mut counts: HashMap<NamedFactionId, u32> = HashMap::new();
    for membership in &members {
        *counts.entry(membership.faction_id).or_insert(0) += 1;
    }
    let live_leaders = leaders
        .iter()
        .map(|leader| leader.faction)
        .collect::<Vec<_>>();
    for faction in registry.factions.iter_mut() {
        let previous_count = faction.current_npc_count;
        let next_count = counts.get(&faction.id).copied().unwrap_or(0);
        faction.current_npc_count = next_count;

        if faction.status == FactionStatus::Active
            && next_count > 0
            && !live_leaders.contains(&faction.id)
        {
            faction.set_status(FactionStatus::Headless);
            leader_down_events.send(NamedFactionLeaderDownEvent {
                faction: faction.id,
                zone: faction.zone_anchor.clone(),
            });
        }

        if faction.status != FactionStatus::Decayed && previous_count > 0 && next_count == 0 {
            let last_npc_count = previous_count;
            faction.set_status(FactionStatus::Decayed);
            for mut membership in &mut memberships {
                if membership.named_faction == Some(faction.id) {
                    membership.named_faction = None;
                }
            }
            decay_events.send(NamedFactionDecayEvent {
                faction: faction.id,
                final_zone: faction.zone_anchor.clone(),
                last_npc_count,
            });
        } else {
            faction.is_active = faction.status != FactionStatus::Decayed;
        }
    }
}

fn leader_realm_for(faction: NamedFactionId) -> Realm {
    match faction {
        NamedFactionId::QingyunHunters => Realm::Solidify,
        NamedFactionId::CangyuanMerchants => Realm::Spirit,
        NamedFactionId::NorthWasteDrifters => Realm::Awaken,
    }
}

fn legacy_faction_id_for_named_faction(faction: NamedFactionId) -> FactionId {
    match faction {
        NamedFactionId::QingyunHunters => FactionId::Attack,
        NamedFactionId::CangyuanMerchants => FactionId::Defend,
        NamedFactionId::NorthWasteDrifters => FactionId::Neutral,
    }
}

fn loyalty_scorer_system(
    store: Res<FactionStore>,
    members: Query<(&FactionMembership, Option<&NpcLodTier>), With<NpcMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<LoyaltyScorer>>,
    lod_config: Option<Res<NpcLodConfig>>,
    lod_tick: Option<Res<NpcLodTick>>,
) {
    let cfg = lod_config.as_deref().cloned().unwrap_or_default();
    let tick = lod_tick.as_deref().map(|t| t.0).unwrap_or(0);
    for (Actor(actor), mut score) in &mut scorers {
        let value = match members.get(*actor) {
            Ok((membership, tier)) => match lod_gated_score(tier, tick, &cfg, || {
                let bias = store
                    .iter()
                    .find(|f| f.id == membership.faction_id)
                    .map(|f| f.loyalty_bias)
                    .unwrap_or(0.5);
                ((membership.reputation.loyalty() + bias) * 0.5).clamp(0.0, 1.0) as f32
            }) {
                Some(value) => value,
                None => continue,
            },
            Err(_) => 0.0,
        };
        score.set(value);
    }
}

fn mission_queue_scorer_system(
    members: Query<(&FactionMembership, Option<&NpcLodTier>), With<NpcMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<MissionQueueScorer>>,
    lod_config: Option<Res<NpcLodConfig>>,
    lod_tick: Option<Res<NpcLodTick>>,
) {
    let cfg = lod_config.as_deref().cloned().unwrap_or_default();
    let tick = lod_tick.as_deref().map(|t| t.0).unwrap_or(0);
    for (Actor(actor), mut score) in &mut scorers {
        let value = match members.get(*actor) {
            Ok((m, tier)) => match lod_gated_score(tier, tick, &cfg, || {
                let pending = m
                    .mission_queue
                    .pending_count()
                    .min(MISSION_QUEUE_SCORER_CAP);
                (pending as f32 / MISSION_QUEUE_SCORER_CAP as f32).clamp(0.0, 1.0)
            }) {
                Some(value) => value,
                None => continue,
            },
            Err(_) => 0.0,
        };
        score.set(value);
    }
}

/// 最小 MissionExecuteAction：停 Navigator → 计时 → 达到上限 pop 掉
/// 队首任务 → Success。真实剧本由 plan-quest-v1 替换。
fn mission_execute_action_system(
    mut members: Query<
        (
            &mut FactionMembership,
            &mut Navigator,
            &mut MissionExecuteState,
        ),
        With<NpcMarker>,
    >,
    mut actions: Query<(&Actor, &mut ActionState), With<MissionExecuteAction>>,
) {
    for (Actor(actor), mut state) in &mut actions {
        let Ok((mut membership, mut navigator, mut exec_state)) = members.get_mut(*actor) else {
            *state = ActionState::Failure;
            continue;
        };
        match *state {
            ActionState::Requested => {
                if membership.mission_queue.pending.is_empty() {
                    *state = ActionState::Success;
                    continue;
                }
                navigator.stop();
                exec_state.elapsed_ticks = 0;
                *state = ActionState::Executing;
            }
            ActionState::Executing => {
                exec_state.elapsed_ticks = exec_state.elapsed_ticks.saturating_add(1);
                if exec_state.elapsed_ticks >= MISSION_EXECUTE_MAX_TICKS {
                    if !membership.mission_queue.pending.is_empty() {
                        membership.mission_queue.pending.remove(0);
                    }
                    *state = ActionState::Success;
                }
            }
            ActionState::Cancelled => {
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

fn faction_leader_territory_scorer_system(
    zones: Option<Res<ZoneRegistry>>,
    leaders: Query<(&NamedFactionLeader, &FactionZoneClaim, &Position), With<NpcMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<FactionLeaderTerritoryScorer>>,
) {
    let Some(zones) = zones.as_deref() else {
        for (_, mut score) in &mut scorers {
            score.set(0.0);
        }
        return;
    };

    for (Actor(actor), mut score) in &mut scorers {
        let Ok((leader, claim, position)) = leaders.get(*actor) else {
            score.set(0.0);
            continue;
        };
        if leader.faction != claim.faction {
            score.set(0.0);
            continue;
        }
        let active = zones
            .find_zone_by_name(claim.zone.as_str())
            .is_some_and(|zone| zone.contains(position.get()));
        score.set(if active {
            FACTION_LEADER_PATROL_SCORE
        } else {
            0.0
        });
    }
}

type FactionLeaderNpcIntruderQueryItem<'a> = (
    Entity,
    &'a Position,
    Option<&'a NamedFactionMembership>,
    Option<&'a FactionMembership>,
);

type FactionLeaderNpcIntruderQuery<'w, 's> =
    Query<'w, 's, FactionLeaderNpcIntruderQueryItem<'static>, With<NpcMarker>>;

fn faction_leader_patrol_action_system(
    zones: Option<Res<ZoneRegistry>>,
    mut leaders: Query<
        (
            &NamedFactionLeader,
            &FactionZoneClaim,
            &mut Navigator,
            &mut FactionLeaderPatrolState,
        ),
        With<NamedFactionLeader>,
    >,
    npc_intruders: FactionLeaderNpcIntruderQuery<'_, '_>,
    player_intruders: Query<(Entity, &Position), With<Client>>,
    mut actions: Query<(&Actor, &mut ActionState), With<FactionLeaderPatrolAction>>,
    mut toll_notices: EventWriter<FactionLeaderTollNotice>,
) {
    let Some(zones) = zones.as_deref() else {
        for (_, mut state) in &mut actions {
            *state = ActionState::Failure;
        }
        return;
    };

    for (Actor(actor), mut action_state) in &mut actions {
        let Ok((leader, claim, mut navigator, mut patrol_state)) = leaders.get_mut(*actor) else {
            *action_state = ActionState::Failure;
            continue;
        };
        let Some(zone) = zones.find_zone_by_name(claim.zone.as_str()) else {
            *action_state = ActionState::Failure;
            continue;
        };
        match *action_state {
            ActionState::Requested => {
                patrol_state.elapsed_ticks = 0;
                navigator.set_goal(zone.patrol_target(0), 1.0);
                let npc_target = npc_intruders.iter().find_map(
                    |(target, position, named_membership, legacy_membership)| {
                        if target == *actor || !zone.contains(position.get()) {
                            return None;
                        }
                        let same_named_faction = named_membership
                            .is_some_and(|membership| membership.faction_id == leader.faction);
                        let same_legacy_faction = legacy_membership.is_some_and(|membership| {
                            membership.faction_id
                                == legacy_faction_id_for_named_faction(leader.faction)
                        });
                        let same_faction = same_named_faction || same_legacy_faction;
                        (!same_faction).then_some((target, FactionLeaderTollTargetKind::Npc))
                    },
                );
                let target = npc_target.or_else(|| {
                    player_intruders
                        .iter()
                        .find(|(target, position)| target != actor && zone.contains(position.get()))
                        .map(|(target, _)| (target, FactionLeaderTollTargetKind::Player))
                });
                if let Some((target, target_kind)) = target {
                    patrol_state.toll_notices_emitted =
                        patrol_state.toll_notices_emitted.saturating_add(1);
                    toll_notices.send(FactionLeaderTollNotice {
                        faction: leader.faction,
                        zone: claim.zone.clone(),
                        target,
                        target_kind,
                    });
                }
                *action_state = ActionState::Executing;
            }
            ActionState::Executing => {
                patrol_state.elapsed_ticks = patrol_state.elapsed_ticks.saturating_add(1);
                if patrol_state.elapsed_ticks >= FACTION_LEADER_PATROL_MAX_TICKS {
                    navigator.set_goal(zone.patrol_target(1), 1.0);
                    *action_state = ActionState::Success;
                }
            }
            ActionState::Cancelled => {
                navigator.stop();
                *action_state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

fn handle_faction_leader_toll_notices_system(mut notices: EventReader<FactionLeaderTollNotice>) {
    for notice in notices.read() {
        tracing::info!(
            "[bong][faction] leader toll notice faction={:?} zone={} target={:?} kind={:?}",
            notice.faction,
            notice.zone,
            notice.target,
            notice.target_kind
        );
    }
}

type EncounterNpcQueryItem<'a> = (
    Entity,
    &'a Position,
    Option<&'a FactionMembership>,
    Option<&'a NamedFactionMembership>,
    Option<&'a DuelTarget>,
);

const HOSTILE_ENCOUNTER_RADIUS: f64 = 16.0;

/// plan-faction-expansion-v1 P1：Hostile 关系偏置（big-brain 0-1 范围内叠加）。
/// 当两 NPC 都携带 NamedFactionMembership 且关系为 Hostile，score += NAMED_HOSTILE_BIAS。
pub const NAMED_HOSTILE_BIAS: f32 = 0.20;

/// plan-faction-expansion-v1 P1：Pact 关系反偏置（big-brain 0-1 范围内叠加，值为负）。
/// 当两 NPC 都携带 NamedFactionMembership 且关系为 Pact，score += NAMED_PACT_BIAS（约 -0.30）。
pub const NAMED_PACT_BIAS: f32 = -0.30;

#[allow(clippy::type_complexity)]
fn assign_hostile_encounters(
    faction_store: Res<FactionStore>,
    relation_matrix: Option<Res<FactionRelationMatrix>>,
    npc_positions: Query<EncounterNpcQueryItem<'_>, With<NpcMarker>>,
    spatial_index: Option<Res<NpcSpatialIndex>>,
    mut perf_probe: Option<ResMut<NpcPerfProbe>>,
    lod_tick: Option<Res<NpcLodTick>>,
    mut commands: valence::prelude::Commands,
) {
    let started_at = Instant::now();
    let npcs = npc_positions
        .iter()
        .map(
            |(entity, position, membership, named_membership, duel_target)| {
                (
                    entity,
                    position.get(),
                    membership.map(|membership| membership.faction_id),
                    named_membership.map(|m| m.faction_id),
                    duel_target.map(|target| target.0),
                )
            },
        )
        .collect::<Vec<_>>();
    let by_entity = npcs
        .iter()
        .map(
            |(entity, position, faction_id, named_faction_id, duel_target)| {
                (
                    *entity,
                    (*position, *faction_id, *named_faction_id, *duel_target),
                )
            },
        )
        .collect::<HashMap<_, _>>();
    let spatial_index = spatial_index.as_deref();
    let relation_matrix = relation_matrix.as_deref();

    for (entity, position, faction_id, named_faction_id, duel_target) in &npcs {
        // NPC 必须有某种派系身份（旧 FactionId 或新 NamedFactionId）才参与敌对判定。
        if faction_id.is_none() && named_faction_id.is_none() {
            if duel_target.is_some() {
                commands.entity(*entity).remove::<DuelTarget>();
            }
            continue;
        }

        let mut nearest_hostile: Option<(Entity, f64)> = None;
        let mut consider =
            |other_entity: Entity,
             other_position: DVec3,
             other_faction_id: Option<FactionId>,
             other_named_faction_id: Option<NamedFactionId>| {
                if other_entity == *entity {
                    return;
                }

                // plan-faction-expansion-v1 P1 反孤岛硬要求：
                // 双方都携带 NamedFactionMembership → 优先走 FactionRelationMatrix::are_hostile。
                // 否则 fallback 到旧 FactionStore::is_hostile_pair（Attack↔Defend 二元硬编码）。
                let is_hostile = match (*named_faction_id, other_named_faction_id) {
                    (Some(self_nf), Some(other_nf)) => {
                        // 双方都有具名势力身份：走关系矩阵判断。
                        relation_matrix
                            .map(|m| m.are_hostile(self_nf, other_nf))
                            .unwrap_or(false)
                    }
                    _ => {
                        // 任一方无具名势力身份：fallback 旧二元模型。
                        let Some(self_fid) = faction_id else { return };
                        let Some(other_fid) = other_faction_id else {
                            return;
                        };
                        faction_store.is_hostile_pair(*self_fid, other_fid)
                    }
                };

                if !is_hostile {
                    return;
                }

                let distance_sq = planar_distance_sq(*position, other_position);
                if distance_sq > HOSTILE_ENCOUNTER_RADIUS * HOSTILE_ENCOUNTER_RADIUS {
                    return;
                }
                if nearest_hostile
                    .as_ref()
                    .is_none_or(|(_, best_sq)| distance_sq < *best_sq)
                {
                    nearest_hostile = Some((other_entity, distance_sq));
                }
            };

        if let Some(index) = spatial_index {
            for other_entity in index.neighbors_within(*position, HOSTILE_ENCOUNTER_RADIUS) {
                if let Some((other_position, other_faction_id, other_named_faction_id, _)) =
                    by_entity.get(&other_entity)
                {
                    consider(
                        other_entity,
                        *other_position,
                        *other_faction_id,
                        *other_named_faction_id,
                    );
                }
            }
        } else {
            for (other_entity, other_position, other_faction_id, other_named_faction_id, _) in &npcs
            {
                consider(
                    *other_entity,
                    *other_position,
                    *other_faction_id,
                    *other_named_faction_id,
                );
            }
        }

        let nearest_hostile = nearest_hostile.map(|(target, _)| target);

        match (duel_target, nearest_hostile) {
            (Some(current), Some(next)) if *current == next => {}
            (_, Some(next)) => {
                commands.entity(*entity).insert(DuelTarget(next));
            }
            (Some(_), None) => {
                commands.entity(*entity).remove::<DuelTarget>();
            }
            (None, None) => {}
        }
    }

    if let Some(probe) = perf_probe.as_deref_mut() {
        probe.record_elapsed("faction_hostile", started_at);
        probe.flush_if_due(lod_tick.as_deref().map(|tick| tick.0).unwrap_or(0));
    }
}

fn planar_distance_sq(left: DVec3, right: DVec3) -> f64 {
    let dx = left.x - right.x;
    let dz = left.z - right.z;
    dx * dx + dz * dz
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use valence::prelude::Events;
    use valence::testing::create_mock_client;

    #[test]
    fn default_store_bootstraps_exactly_three_stable_factions() {
        let store = FactionStore::default();

        assert_eq!(store.factions.len(), 3);
        assert_eq!(store.factions[0].id, FactionId::Attack);
        assert_eq!(store.factions[1].id, FactionId::Defend);
        assert_eq!(store.factions[2].id, FactionId::Neutral);
        assert!(store
            .factions
            .iter()
            .all(|state| state.leader_lineage.is_none()));
        assert!(store
            .factions
            .iter()
            .all(|state| (state.loyalty_bias - 0.5).abs() < f64::EPSILON));
        assert!(store
            .factions
            .iter()
            .all(|state| state.mission_queue.pending.is_empty()));
    }

    #[test]
    fn disciple_membership_roundtrips_with_passive_runtime_fields() {
        let membership = FactionMembership {
            faction_id: FactionId::Attack,
            rank: FactionRank::Disciple,
            reputation: Reputation { loyalty: 0.72 },
            lineage: Some(Lineage {
                master_id: Some("npc_master_001".to_string()),
                disciple_ids: vec!["npc_peer_001".to_string(), "npc_peer_002".to_string()],
            }),
            mission_queue: MissionQueue {
                pending: vec![MissionId("mission:sweep_blood_valley".to_string())],
            },
        };

        let value = serde_json::to_value(&membership).expect("membership should serialize");
        assert_eq!(value["faction_id"], json!("attack"));
        assert_eq!(value["rank"], json!("disciple"));
        assert_eq!(value["reputation"]["loyalty"], json!(0.72));
        assert_eq!(value["lineage"]["master_id"], json!("npc_master_001"));
        assert_eq!(
            value["mission_queue"]["pending"][0],
            json!("mission:sweep_blood_valley")
        );

        let roundtrip: FactionMembership =
            serde_json::from_value(value).expect("membership should deserialize");
        assert_eq!(roundtrip, membership);
    }

    #[test]
    fn faction_store_applies_minimal_events_without_external_runtime() {
        let mut store = FactionStore::default();

        let leader = store
            .apply_event(FactionEventCommand {
                faction_id: FactionId::Neutral,
                kind: FactionEventKind::SetLeaderLineage,
                subject_id: Some("npc_master_001".to_string()),
                mission_id: None,
                loyalty_delta: None,
            })
            .expect("set leader lineage should succeed");
        assert_eq!(leader.leader_id.as_deref(), Some("npc_master_001"));

        let enqueue = store
            .apply_event(FactionEventCommand {
                faction_id: FactionId::Neutral,
                kind: FactionEventKind::EnqueueMission,
                subject_id: None,
                mission_id: Some("mission:hold_spawn_gate".to_string()),
                loyalty_delta: None,
            })
            .expect("enqueue mission should succeed");
        assert_eq!(enqueue.mission_queue_size, 1);

        let adjust = store
            .apply_event(FactionEventCommand {
                faction_id: FactionId::Neutral,
                kind: FactionEventKind::AdjustLoyaltyBias,
                subject_id: None,
                mission_id: None,
                loyalty_delta: Some(0.2),
            })
            .expect("adjust loyalty should succeed");
        assert!((adjust.loyalty_bias - 0.7).abs() < 1e-9);
    }

    #[test]
    fn pop_mission_removes_head_not_tail() {
        let mut store = FactionStore::default();

        for mission in ["mission_a", "mission_b"] {
            store
                .apply_event(FactionEventCommand {
                    faction_id: FactionId::Neutral,
                    kind: FactionEventKind::EnqueueMission,
                    subject_id: None,
                    mission_id: Some(mission.to_string()),
                    loyalty_delta: None,
                })
                .expect("enqueue should succeed");
        }

        let popped = store
            .apply_event(FactionEventCommand {
                faction_id: FactionId::Neutral,
                kind: FactionEventKind::PopMission,
                subject_id: None,
                mission_id: None,
                loyalty_delta: None,
            })
            .expect("pop should succeed");
        assert_eq!(popped.mission_queue_size, 1);

        let queue = &store.faction_mut(FactionId::Neutral).unwrap().mission_queue;
        assert_eq!(queue.top_mission_id(), Some("mission_b"));

        store
            .apply_event(FactionEventCommand {
                faction_id: FactionId::Neutral,
                kind: FactionEventKind::PopMission,
                subject_id: None,
                mission_id: None,
                loyalty_delta: None,
            })
            .expect("pop on single-entry queue should succeed");
        store
            .apply_event(FactionEventCommand {
                faction_id: FactionId::Neutral,
                kind: FactionEventKind::PopMission,
                subject_id: None,
                mission_id: None,
                loyalty_delta: None,
            })
            .expect("pop on empty queue should be a no-op, not a panic");
    }

    #[test]
    fn hostility_matrix_is_only_attack_vs_defend() {
        let store = FactionStore::default();
        assert!(store.is_hostile_pair(FactionId::Attack, FactionId::Defend));
        assert!(store.is_hostile_pair(FactionId::Defend, FactionId::Attack));
        assert!(!store.is_hostile_pair(FactionId::Neutral, FactionId::Attack));
        assert!(!store.is_hostile_pair(FactionId::Neutral, FactionId::Defend));
        assert!(!store.is_hostile_pair(FactionId::Attack, FactionId::Attack));
    }

    // ── plan-offscreen-war-v1 P5 reframe b：涌现群体身份 + 群体敌对 ──────────────

    #[test]
    fn are_hostile_distinct_groups_are_hostile() {
        // §十灵气零和：不同涌现群体争同 zone 灵气 ⇒ 敌对。
        let store = FactionStore::default();
        assert!(
            store.are_hostile(EmergentGroupId(0), EmergentGroupId(1)),
            "distinct emergent groups must be hostile (different groups compete for the same finite qi); 0 vs 1 returned false"
        );
        // 验证 > 2 群体：2 与 3 也敌对（解锁多群体互殴，不止 Attack↔Defend 两家）。
        assert!(
            store.are_hostile(EmergentGroupId(2), EmergentGroupId(3)),
            "any two distinct group ids must be hostile to support >2-group melee; 2 vs 3 returned false"
        );
    }

    #[test]
    fn are_hostile_same_group_is_not_hostile() {
        // §七散修利己仅指向异群体——同一涌现群体的散修同进退，不内斗。
        let store = FactionStore::default();
        assert!(
            !store.are_hostile(EmergentGroupId(2), EmergentGroupId(2)),
            "same emergent group must NOT be hostile to itself (members of one group do not fight each other); 2 vs 2 returned true"
        );
    }

    #[test]
    fn are_hostile_is_symmetric() {
        // 敌对是对称关系：are_hostile(a,b) == are_hostile(b,a)，不区分先后。
        let store = FactionStore::default();
        let a = EmergentGroupId(0);
        let b = EmergentGroupId(3);
        assert_eq!(
            store.are_hostile(a, b),
            store.are_hostile(b, a),
            "are_hostile must be symmetric: hostility does not depend on argument order"
        );
    }

    #[test]
    fn emergent_group_from_faction_attack_maps_to_group_zero() {
        let store = FactionStore::default();
        assert_eq!(
            store.emergent_group_from_faction(FactionId::Attack),
            Some(EmergentGroupId(0)),
            "non-breaking migration: legacy Attack faction must derive to emergent group 0"
        );
    }

    #[test]
    fn emergent_group_from_faction_defend_maps_to_group_one() {
        let store = FactionStore::default();
        assert_eq!(
            store.emergent_group_from_faction(FactionId::Defend),
            Some(EmergentGroupId(1)),
            "non-breaking migration: legacy Defend faction must derive to emergent group 1"
        );
    }

    #[test]
    fn emergent_group_from_faction_neutral_is_none() {
        // 关键非破坏点：Neutral → None，保中立散修离屏仍非战斗（None 进不了 are_hostile）。
        let store = FactionStore::default();
        assert_eq!(
            store.emergent_group_from_faction(FactionId::Neutral),
            None,
            "Neutral must derive to None (not a group) so neutral rogues stay non-combatant offscreen, as no_hostile_pair_yields_no_combat depends on"
        );
    }

    #[test]
    fn group_status_each_variant_roundtrips_via_serde() {
        // 每个 GroupStatus 变体一条专属 serde 往返 + as_str / from_str_name 双向对拍。
        for (status, name) in [
            (GroupStatus::Rising, "rising"),
            (GroupStatus::Stable, "stable"),
            (GroupStatus::Waning, "waning"),
        ] {
            assert_eq!(
                status.as_str(),
                name,
                "GroupStatus::{status:?}.as_str() must equal {name:?}"
            );
            assert_eq!(
                GroupStatus::from_str_name(name),
                Some(status),
                "GroupStatus::from_str_name({name:?}) must roundtrip back to {status:?}"
            );
            let value = serde_json::to_value(status).expect("GroupStatus should serialize");
            assert_eq!(
                value,
                json!(name),
                "GroupStatus::{status:?} must serialize to snake_case string {name:?}, got {value}"
            );
            let back: GroupStatus =
                serde_json::from_value(value).expect("GroupStatus should deserialize");
            assert_eq!(
                back, status,
                "GroupStatus serde roundtrip must return the same variant {status:?}, got {back:?}"
            );
        }
    }

    #[test]
    fn group_status_from_str_name_rejects_unknown() {
        // 错误分支：非法字符串 → None（不 panic、不静默归到某变体）。
        assert_eq!(
            GroupStatus::from_str_name("ascending"),
            None,
            "an unknown status string must yield None, not silently map to a variant"
        );
        assert_eq!(
            GroupStatus::from_str_name(""),
            None,
            "an empty status string must yield None"
        );
    }

    #[test]
    fn emergent_group_id_serializes_as_transparent_bare_number() {
        // serde(transparent)：序列化成裸数字 `3`（不是 `{"0":3}`），与紧凑 Redis 快照对齐。
        let id = EmergentGroupId(3);
        let value = serde_json::to_value(id).expect("EmergentGroupId should serialize");
        assert_eq!(
            value,
            json!(3),
            "EmergentGroupId must serialize transparently as a bare number `3`, not a wrapper object like {{\"0\":3}}; got {value}"
        );
        let back: EmergentGroupId = serde_json::from_value(value)
            .expect("EmergentGroupId should deserialize from bare number");
        assert_eq!(
            back, id,
            "EmergentGroupId must roundtrip through transparent serde back to the same id, got {back:?}"
        );
    }

    #[test]
    fn assign_hostile_encounters_binds_only_near_hostile_pairs() {
        let mut app = App::new();
        app.insert_resource(FactionStore::default());
        app.add_systems(Update, assign_hostile_encounters);

        let attack = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                FactionMembership {
                    faction_id: FactionId::Attack,
                    rank: FactionRank::Disciple,
                    reputation: Reputation::default(),
                    lineage: None,
                    mission_queue: MissionQueue::default(),
                },
            ))
            .id();
        let defend = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([4.0, 64.0, 0.0]),
                FactionMembership {
                    faction_id: FactionId::Defend,
                    rank: FactionRank::Disciple,
                    reputation: Reputation::default(),
                    lineage: None,
                    mission_queue: MissionQueue::default(),
                },
            ))
            .id();
        let neutral = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([3.0, 64.0, 3.0]),
                FactionMembership {
                    faction_id: FactionId::Neutral,
                    rank: FactionRank::Disciple,
                    reputation: Reputation::default(),
                    lineage: None,
                    mission_queue: MissionQueue::default(),
                },
            ))
            .id();

        app.update();

        assert_eq!(
            app.world().get::<DuelTarget>(attack).map(|target| target.0),
            Some(defend)
        );
        assert_eq!(
            app.world().get::<DuelTarget>(defend).map(|target| target.0),
            Some(attack)
        );
        assert!(app.world().get::<DuelTarget>(neutral).is_none());
    }

    // ---------------------------------------------------------------------
    // Phase 5 Disciple Scorer / Action 饱和测试
    // ---------------------------------------------------------------------

    use valence::prelude::{App, PreUpdate};

    fn base_membership(faction: FactionId, loyalty: f64, pending: u32) -> FactionMembership {
        let pending_ids: Vec<MissionId> = (0..pending)
            .map(|i| MissionId(format!("mission_{i}")))
            .collect();
        FactionMembership {
            faction_id: faction,
            rank: FactionRank::Disciple,
            reputation: Reputation { loyalty },
            lineage: None,
            mission_queue: MissionQueue {
                pending: pending_ids,
            },
        }
    }

    fn build_loyalty_app() -> App {
        let mut app = App::new();
        app.insert_resource(FactionStore::default());
        app.add_systems(PreUpdate, loyalty_scorer_system);
        app
    }

    #[test]
    fn loyalty_scorer_zero_when_entity_has_no_membership() {
        let mut app = build_loyalty_app();
        let npc = app.world_mut().spawn(NpcMarker).id();
        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), LoyaltyScorer))
            .id();
        app.update();
        assert_eq!(app.world().get::<Score>(scorer).unwrap().get(), 0.0);
    }

    #[test]
    fn loyalty_scorer_averages_reputation_and_faction_bias() {
        let mut app = build_loyalty_app();
        app.world_mut()
            .resource_mut::<FactionStore>()
            .faction_mut(FactionId::Attack)
            .unwrap()
            .loyalty_bias = 0.8;
        let npc = app
            .world_mut()
            .spawn((NpcMarker, base_membership(FactionId::Attack, 0.4, 0)))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), LoyaltyScorer))
            .id();
        app.update();
        // (0.4 + 0.8) * 0.5 = 0.6
        let got = app.world().get::<Score>(scorer).unwrap().get();
        assert!((got - 0.6).abs() < 1e-6, "expected 0.6, got {got}");
    }

    #[test]
    fn loyalty_scorer_clamps_out_of_range_inputs() {
        let mut app = build_loyalty_app();
        app.world_mut()
            .resource_mut::<FactionStore>()
            .faction_mut(FactionId::Defend)
            .unwrap()
            .loyalty_bias = 2.0; // 超标，Scorer 不信任 store 状态时自保
        let npc = app
            .world_mut()
            .spawn((NpcMarker, base_membership(FactionId::Defend, 5.0, 0)))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), LoyaltyScorer))
            .id();
        app.update();
        let got = app.world().get::<Score>(scorer).unwrap().get();
        assert!((0.0..=1.0).contains(&got));
    }

    fn build_mq_app() -> App {
        let mut app = App::new();
        app.add_systems(PreUpdate, mission_queue_scorer_system);
        app
    }

    #[test]
    fn mission_queue_scorer_zero_when_empty() {
        let mut app = build_mq_app();
        let npc = app
            .world_mut()
            .spawn((NpcMarker, base_membership(FactionId::Attack, 0.5, 0)))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), MissionQueueScorer))
            .id();
        app.update();
        assert_eq!(app.world().get::<Score>(scorer).unwrap().get(), 0.0);
    }

    #[test]
    fn mission_queue_scorer_scales_with_pending_count() {
        let mut app = build_mq_app();
        let npc = app
            .world_mut()
            .spawn((NpcMarker, base_membership(FactionId::Attack, 0.5, 2)))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), MissionQueueScorer))
            .id();
        app.update();
        // 2 / 3 ≈ 0.667
        let got = app.world().get::<Score>(scorer).unwrap().get();
        assert!((got - 2.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn mission_queue_scorer_saturates_at_cap() {
        let mut app = build_mq_app();
        let npc = app
            .world_mut()
            .spawn((NpcMarker, base_membership(FactionId::Attack, 0.5, 10)))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), MissionQueueScorer))
            .id();
        app.update();
        assert_eq!(app.world().get::<Score>(scorer).unwrap().get(), 1.0);
    }

    fn build_exec_app() -> App {
        let mut app = App::new();
        app.add_systems(PreUpdate, mission_execute_action_system);
        app
    }

    #[test]
    fn mission_execute_success_when_queue_empty() {
        let mut app = build_exec_app();
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                base_membership(FactionId::Attack, 0.5, 0),
                Navigator::new(),
                MissionExecuteState::default(),
            ))
            .id();
        let action = app
            .world_mut()
            .spawn((Actor(npc), MissionExecuteAction, ActionState::Requested))
            .id();
        app.update();
        assert_eq!(
            *app.world().get::<ActionState>(action).unwrap(),
            ActionState::Success
        );
    }

    #[test]
    fn mission_execute_pops_mission_on_timeout() {
        let mut app = build_exec_app();
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                base_membership(FactionId::Attack, 0.5, 2),
                Navigator::new(),
                MissionExecuteState::default(),
            ))
            .id();
        let action = app
            .world_mut()
            .spawn((Actor(npc), MissionExecuteAction, ActionState::Requested))
            .id();
        app.update(); // Requested → Executing
        {
            let mut exec = app.world_mut().get_mut::<MissionExecuteState>(npc).unwrap();
            exec.elapsed_ticks = MISSION_EXECUTE_MAX_TICKS - 1;
        }
        app.update(); // Executing → +1 elapsed → 到上限 → Success
        assert_eq!(
            *app.world().get::<ActionState>(action).unwrap(),
            ActionState::Success
        );
        let m = app.world().get::<FactionMembership>(npc).unwrap();
        assert_eq!(m.mission_queue.pending_count(), 1, "应弹出 1 个任务");
    }

    #[test]
    fn mission_execute_stops_navigator_on_requested() {
        let mut app = build_exec_app();
        let mut nav = Navigator::new();
        nav.set_goal(DVec3::new(10.0, 64.0, 10.0), 1.0);
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                base_membership(FactionId::Attack, 0.5, 1),
                nav,
                MissionExecuteState::default(),
            ))
            .id();
        let _action = app
            .world_mut()
            .spawn((Actor(npc), MissionExecuteAction, ActionState::Requested))
            .id();
        app.update();
        assert!(app.world().get::<Navigator>(npc).unwrap().is_idle());
    }

    #[test]
    fn mission_execute_cancelled_transitions_to_failure() {
        let mut app = build_exec_app();
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                base_membership(FactionId::Attack, 0.5, 1),
                Navigator::new(),
                MissionExecuteState::default(),
            ))
            .id();
        let action = app
            .world_mut()
            .spawn((Actor(npc), MissionExecuteAction, ActionState::Cancelled))
            .id();
        app.update();
        assert_eq!(
            *app.world().get::<ActionState>(action).unwrap(),
            ActionState::Failure
        );
    }

    // ── plan-faction-expansion-v1 P0：具名势力注册表 & 兼容层 ────────────────────

    #[test]
    fn test_named_faction_id_display_name() {
        // 三变体 display_name 各==正典串。
        // 正典依据：宗门残息.json（青云外门），北荒坍缩渊记.json（北荒漂流者）。
        // 沧渊商会为 plan 创作设定，正典无「沧渊/盐商」直接记载。
        assert_eq!(
            NamedFactionId::QingyunHunters.display_name(),
            "青云猎盟",
            "QingyunHunters display_name 必须=「青云猎盟」(正典：宗门残息.json 外门残脉)"
        );
        assert_eq!(
            NamedFactionId::CangyuanMerchants.display_name(),
            "沧渊商会",
            "CangyuanMerchants display_name 必须=「沧渊商会」(plan 创作设定，非正典直载)"
        );
        assert_eq!(
            NamedFactionId::NorthWasteDrifters.display_name(),
            "北荒漂流者",
            "NorthWasteDrifters display_name 必须=「北荒漂流者」(正典：北荒坍缩渊记.json)"
        );
    }

    #[test]
    fn test_named_faction_id_zone_anchor() {
        // 三变体 zone_anchor 各对应 zone.rs 字符串（防止与 zone 体系漂移）。
        assert_eq!(
            NamedFactionId::QingyunHunters.zone_anchor(),
            "qingyun_peaks",
            "QingyunHunters zone_anchor 必须对齐 zone.rs 「qingyun_peaks」terrain profile"
        );
        assert_eq!(
            NamedFactionId::CangyuanMerchants.zone_anchor(),
            "blood_valley",
            "CangyuanMerchants zone_anchor 必须对齐 zone.rs 「blood_valley」（裂谷·血谷）"
        );
        assert_eq!(
            NamedFactionId::NorthWasteDrifters.zone_anchor(),
            "north_wastes",
            "NorthWasteDrifters zone_anchor 必须对齐 zone.rs 「north_wastes」（北荒主锚）"
        );
    }

    #[test]
    fn test_named_faction_id_lore_tag() {
        // 三变体 lore_tag 非空且含锚区关键词；CangyuanMerchants 注释标注非正典。
        let q = NamedFactionId::QingyunHunters.lore_tag();
        assert!(!q.is_empty(), "QingyunHunters lore_tag 不能空");
        assert!(
            q.contains("青云") || q.contains("外门") || q.contains("猎"),
            "QingyunHunters lore_tag 应含「青云/外门/猎」相关词，实际：{q}"
        );

        let c = NamedFactionId::CangyuanMerchants.lore_tag();
        assert!(!c.is_empty(), "CangyuanMerchants lore_tag 不能空");
        // 沧渊商会 plan 创作设定，验证 lore_tag 含「血谷」（地理锚点）。
        assert!(
            c.contains("血谷") || c.contains("矿"),
            "CangyuanMerchants lore_tag 应含「血谷/矿」（地理依据），实际：{c}"
        );

        let n = NamedFactionId::NorthWasteDrifters.lore_tag();
        assert!(!n.is_empty(), "NorthWasteDrifters lore_tag 不能空");
        assert!(
            n.contains("北荒") || n.contains("坍缩") || n.contains("漂流"),
            "NorthWasteDrifters lore_tag 应含「北荒/坍缩/漂流」，实际：{n}"
        );
    }

    #[test]
    fn test_faction_status_three_variants() {
        // Active/Headless/Decayed 各 as_str↔from_str_name 往返 + 非法串返 None（三 state 专属 case）。
        for (status, wire) in [
            (FactionStatus::Active, "active"),
            (FactionStatus::Headless, "headless"),
            (FactionStatus::Decayed, "decayed"),
        ] {
            assert_eq!(
                status.as_str(),
                wire,
                "FactionStatus::{status:?}.as_str() 必须返回 {wire:?}"
            );
            assert_eq!(
                FactionStatus::from_str_name(wire),
                Some(status),
                "FactionStatus::from_str_name({wire:?}) 必须返回 {status:?}"
            );
            let serialized = serde_json::to_value(status).expect("FactionStatus should serialize");
            assert_eq!(
                serialized,
                json!(wire),
                "FactionStatus::{status:?} 序列化必须是 snake_case {wire:?}，实际 {serialized}"
            );
            let back: FactionStatus =
                serde_json::from_value(serialized).expect("FactionStatus should deserialize");
            assert_eq!(
                back, status,
                "FactionStatus serde roundtrip 必须还原 {status:?}，实际 {back:?}"
            );
        }
        // 非法串返 None，不 panic。
        assert_eq!(
            FactionStatus::from_str_name("alive"),
            None,
            "非法 status 字符串 \"alive\" 必须返回 None"
        );
        assert_eq!(FactionStatus::from_str_name(""), None, "空串必须返回 None");
    }

    #[test]
    fn test_named_faction_registry_registers_three() {
        // startup_default 返回 3 条；北荒漂流者初始 Headless，其余 Active；
        // display_name/zone_anchor 经 from_id 派生正确。
        let registry = NamedFactionRegistry::startup_default();
        assert_eq!(
            registry.iter().count(),
            3,
            "startup_default 必须注册 3 条具名势力，实际 {}",
            registry.iter().count()
        );
        let north = registry
            .get(NamedFactionId::NorthWasteDrifters)
            .expect("NorthWasteDrifters 必须存在于注册表");
        assert_eq!(
            north.status,
            FactionStatus::Headless,
            "NorthWasteDrifters 初始 status 必须是 Headless（正典：坍缩渊记无法组织化），实际 {:?}",
            north.status
        );
        for faction_id in [
            NamedFactionId::QingyunHunters,
            NamedFactionId::CangyuanMerchants,
        ] {
            let f = registry
                .get(faction_id)
                .unwrap_or_else(|| panic!("{faction_id:?} 必须存在"));
            assert_eq!(
                f.status,
                FactionStatus::Active,
                "{faction_id:?} 初始 status 必须是 Active，实际 {:?}",
                f.status
            );
            assert_eq!(
                f.display_name,
                faction_id.display_name(),
                "{faction_id:?} display_name 必须从 id 派生"
            );
            assert_eq!(
                f.zone_anchor,
                faction_id.zone_anchor(),
                "{faction_id:?} zone_anchor 必须从 id 派生"
            );
        }
    }

    #[test]
    fn test_registry_inserted_at_app_boot() {
        // 防孤岛 #1：build 最小 App→register()→断言 world.get_resource::<NamedFactionRegistry>()
        // Some 且 3 条（非仅类型存在，必须可查内容）。
        let mut app = App::new();
        register(&mut app);
        let registry = app
            .world()
            .get_resource::<NamedFactionRegistry>()
            .expect("NamedFactionRegistry 必须在 register() 后存在于 World（防孤岛 #1）");
        assert_eq!(
            registry.iter().count(),
            3,
            "注册表 App boot 后必须有 3 条，实际 {}",
            registry.iter().count()
        );
    }

    #[test]
    fn test_faction_id_for_war_maps_to_hostile() {
        // 防孤岛 #2：faction_id_for_war 返回喂现有 FactionStore::is_hostile_pair，
        // 单测真打通到现有 war 逻辑（assert 理由：兼容层必须打通到 is_hostile_pair 否则孤岛）。
        let store = FactionStore::default();
        let (attack, defend) = FactionStore::faction_id_for_war(
            NamedFactionId::QingyunHunters,
            NamedFactionId::CangyuanMerchants,
        );
        assert!(
            store.is_hostile_pair(attack, defend),
            "faction_id_for_war(Qingyun, Cangyuan) 返回的 ({attack:?},{defend:?}) 必须满足 \
             is_hostile_pair==true（兼容层必须打通到 is_hostile_pair，否则为孤岛桩）"
        );
        // 对称：进攻/防守互换方向也要正确。
        let (attack2, defend2) = FactionStore::faction_id_for_war(
            NamedFactionId::CangyuanMerchants,
            NamedFactionId::NorthWasteDrifters,
        );
        assert!(
            store.is_hostile_pair(attack2, defend2),
            "faction_id_for_war(Cangyuan, North) 也必须打通到 is_hostile_pair"
        );
    }

    // ── plan-faction-expansion-v1 P1：FactionRelationMatrix 饱和测试 ───────────────

    #[test]
    fn relation_matrix_startup_default_three_pairs_correct() {
        // v1 初值三对关系：(Qingyun,Cangyuan)=Neutral / (Qingyun,North)=Hostile /
        // (Cangyuan,North)=Neutral。正典依据见 FactionRelationMatrix::startup_default。
        let matrix = FactionRelationMatrix::startup_default();
        assert_eq!(
            matrix.get(NamedFactionId::QingyunHunters, NamedFactionId::CangyuanMerchants),
            FactionRelation::Neutral,
            "v1 初值：(QingyunHunters, CangyuanMerchants) 必须 = Neutral；猎盟与商会各守一方互不开战"
        );
        assert_eq!(
            matrix.get(
                NamedFactionId::QingyunHunters,
                NamedFactionId::NorthWasteDrifters
            ),
            FactionRelation::Hostile,
            "v1 初值：(QingyunHunters, NorthWasteDrifters) 必须 = Hostile；猎盟排斥闯入的游荡者"
        );
        assert_eq!(
            matrix.get(
                NamedFactionId::CangyuanMerchants,
                NamedFactionId::NorthWasteDrifters
            ),
            FactionRelation::Neutral,
            "v1 初值：(CangyuanMerchants, NorthWasteDrifters) 必须 = Neutral；商会有时雇佣漂流者"
        );
    }

    #[test]
    fn relation_matrix_are_hostile_symmetric() {
        // are_hostile 对称性：(a,b) 与 (b,a) 返回相同结果。
        let matrix = FactionRelationMatrix::startup_default();
        let a = NamedFactionId::QingyunHunters;
        let b = NamedFactionId::NorthWasteDrifters;
        assert_eq!(
            matrix.are_hostile(a, b),
            matrix.are_hostile(b, a),
            "are_hostile 必须对称：(Qingyun, North) 与 (North, Qingyun) 应相等，不区分参数顺序"
        );
        // 中立对组同样对称。
        let c = NamedFactionId::CangyuanMerchants;
        assert_eq!(
            matrix.are_hostile(a, c),
            matrix.are_hostile(c, a),
            "are_hostile 对称性：(Qingyun, Cangyuan) 与 (Cangyuan, Qingyun) 应相等"
        );
    }

    #[test]
    fn relation_matrix_unregistered_pair_defaults_neutral() {
        // 未注册对组默认 Neutral（不敌对，防止遗漏配置引发意外战斗）。
        // 构造一个空矩阵，任意对组都应返回 Neutral。
        let matrix = FactionRelationMatrix {
            relations: HashMap::new(),
        };
        assert_eq!(
            matrix.get(
                NamedFactionId::QingyunHunters,
                NamedFactionId::CangyuanMerchants
            ),
            FactionRelation::Neutral,
            "未注册对组必须默认 Neutral，防止遗漏配置触发意外战斗"
        );
        assert!(
            !matrix.are_hostile(
                NamedFactionId::QingyunHunters,
                NamedFactionId::NorthWasteDrifters
            ),
            "未注册对组 are_hostile 必须返回 false（默认 Neutral，边界防御）"
        );
    }

    #[test]
    fn relation_matrix_set_updates_relation() {
        // set() 动态更新关系（运行时/测试用）。
        let mut matrix = FactionRelationMatrix::startup_default();
        // 初始 Neutral，改为 Pact。
        matrix.set(
            NamedFactionId::QingyunHunters,
            NamedFactionId::CangyuanMerchants,
            FactionRelation::Pact,
        );
        assert_eq!(
            matrix.get(
                NamedFactionId::QingyunHunters,
                NamedFactionId::CangyuanMerchants
            ),
            FactionRelation::Pact,
            "set() 后关系应变为 Pact，初始 Neutral 被覆盖"
        );
        // 对称性不变。
        assert_eq!(
            matrix.get(
                NamedFactionId::CangyuanMerchants,
                NamedFactionId::QingyunHunters
            ),
            FactionRelation::Pact,
            "set() 写入规范化 key 后 (b,a) 方向也应反映更新（对称写入）"
        );
    }

    #[test]
    fn faction_relation_all_variants_serde_roundtrip() {
        // 每个 FactionRelation 变体 serde 往返 + as_str/from_str_name 对拍。
        for (relation, wire) in [
            (FactionRelation::Hostile, "hostile"),
            (FactionRelation::Neutral, "neutral"),
            (FactionRelation::Pact, "pact"),
        ] {
            assert_eq!(
                relation.as_str(),
                wire,
                "FactionRelation::{relation:?}.as_str() 必须=「{wire}」"
            );
            assert_eq!(
                FactionRelation::from_str_name(wire),
                Some(relation),
                "FactionRelation::from_str_name({wire:?}) 必须=Some({relation:?})"
            );
            let value = serde_json::to_value(relation).expect("FactionRelation should serialize");
            assert_eq!(
                value,
                json!(wire),
                "FactionRelation::{relation:?} 序列化应为 {wire:?}，实际 {value}"
            );
            let back: FactionRelation =
                serde_json::from_value(value).expect("FactionRelation should deserialize");
            assert_eq!(
                back, relation,
                "FactionRelation serde roundtrip 应还原 {relation:?}，实际 {back:?}"
            );
        }
        assert_eq!(
            FactionRelation::from_str_name("unknown"),
            None,
            "非法 relation 字符串必须返回 None"
        );
    }

    #[test]
    fn relation_matrix_inserted_at_app_boot() {
        // register() 后 FactionRelationMatrix 必须存在于 World 且三对初值正确（防孤岛）。
        let mut app = App::new();
        register(&mut app);
        let matrix = app
            .world()
            .get_resource::<FactionRelationMatrix>()
            .expect("FactionRelationMatrix 必须在 register() 后存在于 World（防孤岛）");
        assert!(
            matrix.are_hostile(
                NamedFactionId::QingyunHunters,
                NamedFactionId::NorthWasteDrifters
            ),
            "App boot 后矩阵的 (Qingyun, North) 必须 = Hostile"
        );
        assert!(
            !matrix.are_hostile(
                NamedFactionId::QingyunHunters,
                NamedFactionId::CangyuanMerchants
            ),
            "App boot 后矩阵的 (Qingyun, Cangyuan) 必须 = Neutral（不敌对）"
        );
    }

    #[test]
    fn assign_hostile_encounters_named_faction_hostile_pair_triggers_duel() {
        // 反孤岛硬要求：两 NPC 都携带 NamedFactionMembership，关系 Hostile ⇒ assign_hostile_encounters
        // 用 FactionRelationMatrix 判断 ⇒ 写入 DuelTarget（非孤岛：真实 combat 路径消费）。
        let mut app = App::new();
        app.insert_resource(FactionStore::default());
        app.insert_resource(FactionRelationMatrix::startup_default());
        app.add_systems(Update, assign_hostile_encounters);

        // Qingyun vs NorthWaste = Hostile in startup_default。
        let qingyun = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                NamedFactionMembership {
                    faction_id: NamedFactionId::QingyunHunters,
                },
            ))
            .id();
        let north = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([4.0, 64.0, 0.0]),
                NamedFactionMembership {
                    faction_id: NamedFactionId::NorthWasteDrifters,
                },
            ))
            .id();
        app.update();

        assert_eq!(
            app.world().get::<DuelTarget>(qingyun).map(|t| t.0),
            Some(north),
            "QingyunHunters NPC 必须把 NorthWasteDrifters NPC 设为 DuelTarget（关系 Hostile，反孤岛硬验证）"
        );
        assert_eq!(
            app.world().get::<DuelTarget>(north).map(|t| t.0),
            Some(qingyun),
            "NorthWasteDrifters NPC 必须把 QingyunHunters NPC 设为 DuelTarget（关系 Hostile，反孤岛硬验证）"
        );
    }

    #[test]
    fn assign_hostile_encounters_named_faction_neutral_pair_no_duel() {
        // Neutral 关系不触发 DuelTarget：(Qingyun, Cangyuan) = Neutral ⇒ 不成对。
        let mut app = App::new();
        app.insert_resource(FactionStore::default());
        app.insert_resource(FactionRelationMatrix::startup_default());
        app.add_systems(Update, assign_hostile_encounters);

        let qingyun = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                NamedFactionMembership {
                    faction_id: NamedFactionId::QingyunHunters,
                },
            ))
            .id();
        let cangyuan = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([4.0, 64.0, 0.0]),
                NamedFactionMembership {
                    faction_id: NamedFactionId::CangyuanMerchants,
                },
            ))
            .id();
        app.update();

        assert!(
            app.world().get::<DuelTarget>(qingyun).is_none(),
            "Neutral 关系的 QingyunHunters NPC 不应有 DuelTarget（不主动与 CangyuanMerchants 开战）"
        );
        assert!(
            app.world().get::<DuelTarget>(cangyuan).is_none(),
            "Neutral 关系的 CangyuanMerchants NPC 不应有 DuelTarget"
        );
    }

    #[test]
    fn assign_hostile_encounters_headless_faction_still_uses_matrix() {
        // Headless 势力（NorthWasteDrifters）仍按关系矩阵参战：与 QingyunHunters = Hostile。
        // 即便 FactionStatus::Headless 也不影响 NamedFactionMembership 的判定路径。
        let mut app = App::new();
        app.insert_resource(FactionStore::default());
        app.insert_resource(FactionRelationMatrix::startup_default());
        app.add_systems(Update, assign_hostile_encounters);

        let north = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                NamedFactionMembership {
                    faction_id: NamedFactionId::NorthWasteDrifters,
                },
            ))
            .id();
        let qingyun = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([5.0, 64.0, 0.0]),
                NamedFactionMembership {
                    faction_id: NamedFactionId::QingyunHunters,
                },
            ))
            .id();
        app.update();

        assert_eq!(
            app.world().get::<DuelTarget>(north).map(|t| t.0),
            Some(qingyun),
            "Headless 势力 NorthWasteDrifters 仍按关系矩阵参战（与 QingyunHunters = Hostile）；\
             FactionStatus::Headless 不影响敌对判定路径"
        );
    }

    #[test]
    fn assign_hostile_encounters_pact_relation_no_duel() {
        // Pact 关系：动态改关系矩阵为 Pact，两 NPC 不应生成 DuelTarget。
        let mut app = App::new();
        app.insert_resource(FactionStore::default());
        let mut matrix = FactionRelationMatrix::startup_default();
        // 改 (Qingyun, North) → Pact
        matrix.set(
            NamedFactionId::QingyunHunters,
            NamedFactionId::NorthWasteDrifters,
            FactionRelation::Pact,
        );
        app.insert_resource(matrix);
        app.add_systems(Update, assign_hostile_encounters);

        let qingyun = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                NamedFactionMembership {
                    faction_id: NamedFactionId::QingyunHunters,
                },
            ))
            .id();
        let north = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([4.0, 64.0, 0.0]),
                NamedFactionMembership {
                    faction_id: NamedFactionId::NorthWasteDrifters,
                },
            ))
            .id();
        app.update();

        assert!(
            app.world().get::<DuelTarget>(qingyun).is_none(),
            "Pact 关系下 QingyunHunters NPC 不应有 DuelTarget（盟友不互打）"
        );
        assert!(
            app.world().get::<DuelTarget>(north).is_none(),
            "Pact 关系下 NorthWasteDrifters NPC 不应有 DuelTarget"
        );
    }

    #[test]
    fn assign_hostile_encounters_fallback_to_faction_id_when_no_named_membership() {
        // fallback 路径验证：无 NamedFactionMembership 时仍走旧 is_hostile_pair（非孤岛）。
        // Attack↔Defend = Hostile，不依赖 FactionRelationMatrix。
        let mut app = App::new();
        app.insert_resource(FactionStore::default());
        // 故意不插入 FactionRelationMatrix，确认 fallback 路径不依赖它。
        app.add_systems(Update, assign_hostile_encounters);

        let attack = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                FactionMembership {
                    faction_id: FactionId::Attack,
                    rank: FactionRank::Disciple,
                    reputation: Reputation::default(),
                    lineage: None,
                    mission_queue: MissionQueue::default(),
                },
            ))
            .id();
        let defend = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([4.0, 64.0, 0.0]),
                FactionMembership {
                    faction_id: FactionId::Defend,
                    rank: FactionRank::Disciple,
                    reputation: Reputation::default(),
                    lineage: None,
                    mission_queue: MissionQueue::default(),
                },
            ))
            .id();
        app.update();

        assert_eq!(
            app.world().get::<DuelTarget>(attack).map(|t| t.0),
            Some(defend),
            "旧 FactionId::Attack NPC 应在 fallback 路径（无 NamedFactionMembership）把 \
             Defend NPC 设为 DuelTarget（is_hostile_pair 仍生效）"
        );
    }

    fn test_zone(
        name: &str,
        min_x: f64,
        min_z: f64,
        max_x: f64,
        max_z: f64,
    ) -> crate::world::zone::Zone {
        crate::world::zone::Zone {
            name: name.to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            bounds: (
                DVec3::new(min_x, 60.0, min_z),
                DVec3::new(max_x, 90.0, max_z),
            ),
            spirit_qi: 0.5,
            danger_level: 1,
            active_events: Vec::new(),
            patrol_anchors: vec![
                DVec3::new((min_x + max_x) * 0.5, 66.0, (min_z + max_z) * 0.5),
                DVec3::new(max_x - 4.0, 66.0, max_z - 4.0),
            ],
            blocked_tiles: Vec::new(),
        }
    }

    fn p2_zone_registry() -> ZoneRegistry {
        ZoneRegistry {
            zones: vec![
                test_zone("qingyun_peaks", 0.0, 0.0, 100.0, 100.0),
                test_zone("blood_valley", 200.0, 0.0, 320.0, 120.0),
                test_zone("north_wastes", 400.0, 0.0, 560.0, 160.0),
            ],
        }
    }

    fn build_leader_patrol_app() -> App {
        let mut app = App::new();
        app.insert_resource(p2_zone_registry());
        app.add_event::<FactionLeaderTollNotice>();
        app.add_systems(PreUpdate, faction_leader_patrol_action_system);
        app
    }

    #[test]
    fn faction_zone_claims_match_registry_zone_anchors() {
        let registry = NamedFactionRegistry::startup_default();
        let claims = FactionZoneClaims::from_registry(&registry);
        for faction in registry.iter() {
            let claim = claims
                .get(faction.id)
                .unwrap_or_else(|| panic!("{:?} 必须有 FactionZoneClaim", faction.id));
            assert_eq!(
                claim.zone, faction.zone_anchor,
                "{:?} 的 FactionZoneClaim 必须与 NamedFactionRegistry.zone_anchor 一致",
                faction.id
            );
        }
    }

    #[test]
    fn leader_spawn_creates_active_faction_leaders_and_skips_headless() {
        let mut app = App::new();
        app.insert_resource(NamedFactionRegistry::startup_default());
        let claims =
            FactionZoneClaims::from_registry(app.world().resource::<NamedFactionRegistry>());
        app.insert_resource(claims);
        app.insert_resource(p2_zone_registry());
        app.world_mut().spawn(OverworldLayer);
        app.add_systems(Update, spawn_named_faction_leaders_on_startup);
        app.update();

        let mut leaders = app.world_mut().query::<(
            &NamedFactionLeader,
            &NamedFactionMembership,
            &FactionMembership,
            &FactionZoneClaim,
            &Position,
        )>();
        let rows = leaders
            .iter(app.world())
            .map(|(leader, membership, legacy_membership, claim, position)| {
                (
                    leader.faction,
                    membership.faction_id,
                    legacy_membership.faction_id,
                    claim.zone.clone(),
                    position.get(),
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            rows.len(),
            2,
            "P2 只应为 Active 势力刷新 2 名领袖；Headless 北荒漂流者不刷新，实际 {:?}",
            rows
        );
        assert!(
            rows.iter().any(|(leader, membership, legacy, zone, pos)| {
                *leader == NamedFactionId::QingyunHunters
                    && *membership == NamedFactionId::QingyunHunters
                    && *legacy == FactionId::Attack
                    && zone == "qingyun_peaks"
                    && p2_zone_registry()
                        .find_zone_by_name("qingyun_peaks")
                        .unwrap()
                        .contains(*pos)
            }),
            "青云猎盟领袖必须带 NamedFactionLeader + NamedFactionMembership 并刷新在 qingyun_peaks 内"
        );
        assert!(
            rows.iter().any(|(leader, membership, legacy, zone, pos)| {
                *leader == NamedFactionId::CangyuanMerchants
                    && *membership == NamedFactionId::CangyuanMerchants
                    && *legacy == FactionId::Defend
                    && zone == "blood_valley"
                    && p2_zone_registry()
                        .find_zone_by_name("blood_valley")
                        .unwrap()
                        .contains(*pos)
            }),
            "沧渊商会领袖必须带 NamedFactionLeader + NamedFactionMembership 并刷新在 blood_valley 内"
        );
        assert!(
            rows.iter()
                .all(|(leader, _, _, _, _)| *leader != NamedFactionId::NorthWasteDrifters),
            "NorthWasteDrifters 初始 Headless，P2 不应刷新领袖"
        );
    }

    #[test]
    fn leader_spawn_is_idempotent_after_startup_runs_twice() {
        let mut app = App::new();
        app.insert_resource(NamedFactionRegistry::startup_default());
        let claims =
            FactionZoneClaims::from_registry(app.world().resource::<NamedFactionRegistry>());
        app.insert_resource(claims);
        app.insert_resource(p2_zone_registry());
        app.world_mut().spawn(OverworldLayer);
        app.add_systems(Update, spawn_named_faction_leaders_on_startup);

        app.update();
        app.update();

        let mut leaders = app.world_mut().query::<&NamedFactionLeader>();
        assert_eq!(
            leaders.iter(app.world()).count(),
            2,
            "startup system 重跑不应为 Active 势力重复刷新领袖"
        );
    }

    #[test]
    fn leader_spawn_waits_for_zone_registry_before_marking_done() {
        let mut app = App::new();
        app.insert_resource(NamedFactionRegistry::startup_default());
        let claims =
            FactionZoneClaims::from_registry(app.world().resource::<NamedFactionRegistry>());
        app.insert_resource(claims);
        app.world_mut().spawn(OverworldLayer);
        app.add_systems(Update, spawn_named_faction_leaders_on_startup);

        app.update();
        let mut leaders = app.world_mut().query::<&NamedFactionLeader>();
        assert_eq!(
            leaders.iter(app.world()).count(),
            0,
            "缺 ZoneRegistry 时不应刷新领袖，也不应把 startup Local done 置真"
        );

        app.insert_resource(p2_zone_registry());
        app.update();
        let mut leaders = app.world_mut().query::<&NamedFactionLeader>();
        assert_eq!(
            leaders.iter(app.world()).count(),
            2,
            "ZoneRegistry 后置注入后，startup system 仍必须刷新两个 Active 领袖"
        );
    }

    #[test]
    fn leader_realm_matches_plan_table() {
        assert_eq!(
            leader_realm_for(NamedFactionId::QingyunHunters),
            Realm::Solidify,
            "青云猎盟盟主按 P2 表应为固元"
        );
        assert_eq!(
            leader_realm_for(NamedFactionId::CangyuanMerchants),
            Realm::Spirit,
            "沧渊商会会首按 P2 表应为通灵"
        );
    }

    #[test]
    fn leader_territory_scorer_only_activates_inside_claimed_zone() {
        let mut app = App::new();
        app.insert_resource(p2_zone_registry());
        app.add_systems(PreUpdate, faction_leader_territory_scorer_system);
        let leader = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([50.0, 66.0, 50.0]),
                NamedFactionLeader {
                    faction: NamedFactionId::QingyunHunters,
                },
                FactionZoneClaim {
                    faction: NamedFactionId::QingyunHunters,
                    zone: "qingyun_peaks".to_string(),
                },
            ))
            .id();
        let scorer = app
            .world_mut()
            .spawn((
                Actor(leader),
                Score::default(),
                FactionLeaderTerritoryScorer,
            ))
            .id();
        app.update();
        let active_score = app.world().get::<Score>(scorer).unwrap().get();
        assert!(
            (active_score - FACTION_LEADER_PATROL_SCORE).abs() < f32::EPSILON,
            "领袖在自己 FactionZoneClaim 内时 scorer 必须激活，实际 {active_score}"
        );

        app.world_mut()
            .get_mut::<Position>(leader)
            .unwrap()
            .set([500.0, 66.0, 50.0]);
        app.update();
        let outside_score = app.world().get::<Score>(scorer).unwrap().get();
        assert_eq!(
            outside_score, 0.0,
            "领袖离开自己 claim zone 后 scorer 必须失活，实际 {outside_score}"
        );
    }

    #[test]
    fn leader_territory_scorer_zero_when_claim_zone_missing() {
        let mut app = App::new();
        app.insert_resource(p2_zone_registry());
        app.add_systems(PreUpdate, faction_leader_territory_scorer_system);
        let leader = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([50.0, 66.0, 50.0]),
                NamedFactionLeader {
                    faction: NamedFactionId::QingyunHunters,
                },
                FactionZoneClaim {
                    faction: NamedFactionId::QingyunHunters,
                    zone: "missing_zone".to_string(),
                },
            ))
            .id();
        let scorer = app
            .world_mut()
            .spawn((
                Actor(leader),
                Score::default(),
                FactionLeaderTerritoryScorer,
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Score>(scorer).unwrap().get(),
            0.0,
            "claim zone 缺失时 scorer 必须失活，避免领袖在未知区域触发地盘行为"
        );
    }

    #[test]
    fn leader_territory_scorer_zero_when_claim_faction_mismatch() {
        let mut app = App::new();
        app.insert_resource(p2_zone_registry());
        app.add_systems(PreUpdate, faction_leader_territory_scorer_system);
        let leader = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([50.0, 66.0, 50.0]),
                NamedFactionLeader {
                    faction: NamedFactionId::QingyunHunters,
                },
                FactionZoneClaim {
                    faction: NamedFactionId::CangyuanMerchants,
                    zone: "qingyun_peaks".to_string(),
                },
            ))
            .id();
        let scorer = app
            .world_mut()
            .spawn((
                Actor(leader),
                Score::default(),
                FactionLeaderTerritoryScorer,
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Score>(scorer).unwrap().get(),
            0.0,
            "NamedFactionLeader 与 FactionZoneClaim faction 不一致时 scorer 必须失活"
        );
    }

    #[test]
    fn leader_patrol_action_sets_zone_patrol_goal_and_finishes() {
        let mut app = build_leader_patrol_app();
        let leader = app
            .world_mut()
            .spawn((
                NpcMarker,
                NamedFactionLeader {
                    faction: NamedFactionId::QingyunHunters,
                },
                FactionZoneClaim {
                    faction: NamedFactionId::QingyunHunters,
                    zone: "qingyun_peaks".to_string(),
                },
                Navigator::new(),
                FactionLeaderPatrolState::default(),
            ))
            .id();
        let action = app
            .world_mut()
            .spawn((
                Actor(leader),
                FactionLeaderPatrolAction,
                ActionState::Requested,
            ))
            .id();
        app.update();
        assert_eq!(
            *app.world().get::<ActionState>(action).unwrap(),
            ActionState::Executing,
            "Requested 后领袖巡逻 action 应进入 Executing"
        );
        assert!(
            !app.world().get::<Navigator>(leader).unwrap().is_idle(),
            "Requested 后 Navigator 必须收到 claim zone 的 patrol goal"
        );

        app.world_mut()
            .get_mut::<FactionLeaderPatrolState>(leader)
            .unwrap()
            .elapsed_ticks = FACTION_LEADER_PATROL_MAX_TICKS - 1;
        app.update();
        assert_eq!(
            *app.world().get::<ActionState>(action).unwrap(),
            ActionState::Success,
            "达到 FACTION_LEADER_PATROL_MAX_TICKS 后 action 必须 Success，避免领袖卡死"
        );
    }

    #[test]
    fn leader_patrol_action_emits_toll_notice_for_foreign_npc_in_claim() {
        let mut app = build_leader_patrol_app();
        let leader = app
            .world_mut()
            .spawn((
                NpcMarker,
                NamedFactionLeader {
                    faction: NamedFactionId::QingyunHunters,
                },
                FactionZoneClaim {
                    faction: NamedFactionId::QingyunHunters,
                    zone: "qingyun_peaks".to_string(),
                },
                Navigator::new(),
                FactionLeaderPatrolState::default(),
            ))
            .id();
        let intruder = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([52.0, 66.0, 52.0]),
                NamedFactionMembership {
                    faction_id: NamedFactionId::CangyuanMerchants,
                },
            ))
            .id();
        app.world_mut().spawn((
            Actor(leader),
            FactionLeaderPatrolAction,
            ActionState::Requested,
        ));

        app.update();

        let events = app.world().resource::<Events<FactionLeaderTollNotice>>();
        let notices = events
            .get_reader()
            .read(events)
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(
            notices.len(),
            1,
            "外派 NPC 进入领袖地盘时必须产生收费 notice"
        );
        assert_eq!(
            notices[0].faction,
            NamedFactionId::QingyunHunters,
            "expected QingyunHunters because the patrol leader owns qingyun_peaks, actual {:?}",
            notices[0].faction
        );
        assert_eq!(
            notices[0].zone, "qingyun_peaks",
            "expected qingyun_peaks because the leader claim is qingyun_peaks, actual {}",
            notices[0].zone
        );
        assert_eq!(
            notices[0].target, intruder,
            "expected intruder entity because it is the first foreign NPC in the claim, actual {:?}",
            notices[0].target
        );
        assert_eq!(
            notices[0].target_kind,
            FactionLeaderTollTargetKind::Npc,
            "expected Npc because the toll target is a foreign NPC, actual {:?}",
            notices[0].target_kind
        );
        assert_eq!(
            app.world()
                .get::<FactionLeaderPatrolState>(leader)
                .unwrap()
                .toll_notices_emitted,
            1,
            "收费 notice 计数用于防止行为树收费分支变成不可观测副作用"
        );
    }

    #[test]
    fn leader_patrol_action_does_not_toll_same_faction_npc() {
        let mut app = build_leader_patrol_app();
        let leader = app
            .world_mut()
            .spawn((
                NpcMarker,
                NamedFactionLeader {
                    faction: NamedFactionId::QingyunHunters,
                },
                FactionZoneClaim {
                    faction: NamedFactionId::QingyunHunters,
                    zone: "qingyun_peaks".to_string(),
                },
                Navigator::new(),
                FactionLeaderPatrolState::default(),
            ))
            .id();
        app.world_mut().spawn((
            NpcMarker,
            Position::new([52.0, 66.0, 52.0]),
            NamedFactionMembership {
                faction_id: NamedFactionId::QingyunHunters,
            },
        ));
        app.world_mut().spawn((
            Actor(leader),
            FactionLeaderPatrolAction,
            ActionState::Requested,
        ));

        app.update();

        let events = app.world().resource::<Events<FactionLeaderTollNotice>>();
        assert_eq!(
            events.get_reader().read(events).count(),
            0,
            "同派 NPC 在自家 claim 内不应触发收费 notice"
        );
    }

    #[test]
    fn leader_patrol_action_does_not_toll_legacy_same_faction_npc() {
        let cases = [
            (
                NamedFactionId::QingyunHunters,
                FactionId::Attack,
                "qingyun_peaks",
                [52.0, 66.0, 52.0],
            ),
            (
                NamedFactionId::CangyuanMerchants,
                FactionId::Defend,
                "blood_valley",
                [220.0, 66.0, 52.0],
            ),
            (
                NamedFactionId::NorthWasteDrifters,
                FactionId::Neutral,
                "north_wastes",
                [430.0, 66.0, 52.0],
            ),
        ];

        for (named_faction, legacy_faction, claim_zone, npc_position) in cases {
            let mut app = build_leader_patrol_app();
            let leader = app
                .world_mut()
                .spawn((
                    NpcMarker,
                    NamedFactionLeader {
                        faction: named_faction,
                    },
                    FactionZoneClaim {
                        faction: named_faction,
                        zone: claim_zone.to_string(),
                    },
                    Navigator::new(),
                    FactionLeaderPatrolState::default(),
                ))
                .id();
            app.world_mut().spawn((
                NpcMarker,
                Position::new(npc_position),
                base_membership(legacy_faction, 0.0, 0),
            ));
            app.world_mut().spawn((
                Actor(leader),
                FactionLeaderPatrolAction,
                ActionState::Requested,
            ));

            app.update();

            let events = app.world().resource::<Events<FactionLeaderTollNotice>>();
            assert_eq!(
                events.get_reader().read(events).count(),
                0,
                "只带旧 {legacy_faction:?} 的同派普通弟子不应被 {named_faction:?} 领袖误收过路费"
            );
        }
    }

    #[test]
    fn leader_patrol_action_cancelled_stops_navigator_and_fails() {
        let mut app = build_leader_patrol_app();
        let mut navigator = Navigator::new();
        navigator.set_goal(DVec3::new(80.0, 66.0, 80.0), 1.0);
        let leader = app
            .world_mut()
            .spawn((
                NpcMarker,
                NamedFactionLeader {
                    faction: NamedFactionId::QingyunHunters,
                },
                FactionZoneClaim {
                    faction: NamedFactionId::QingyunHunters,
                    zone: "qingyun_peaks".to_string(),
                },
                navigator,
                FactionLeaderPatrolState::default(),
            ))
            .id();
        let action = app
            .world_mut()
            .spawn((
                Actor(leader),
                FactionLeaderPatrolAction,
                ActionState::Cancelled,
            ))
            .id();

        app.update();

        assert_eq!(
            *app.world().get::<ActionState>(action).unwrap(),
            ActionState::Failure,
            "Cancelled 后巡逻 action 必须 Failure"
        );
        assert!(
            app.world().get::<Navigator>(leader).unwrap().is_idle(),
            "Cancelled 后 Navigator 必须 stop，避免残留巡逻 goal"
        );
    }

    #[test]
    fn named_faction_census_counts_named_memberships() {
        let mut app = App::new();
        app.insert_resource(NamedFactionRegistry::startup_default());
        app.add_event::<NamedFactionLeaderDownEvent>();
        app.add_event::<NamedFactionDecayEvent>();
        app.add_systems(Update, sync_named_faction_census_system);
        app.world_mut().spawn((
            NpcMarker,
            NamedFactionMembership {
                faction_id: NamedFactionId::QingyunHunters,
            },
        ));
        app.world_mut().spawn((
            NpcMarker,
            NamedFactionMembership {
                faction_id: NamedFactionId::QingyunHunters,
            },
        ));
        app.world_mut().spawn((
            NpcMarker,
            NamedFactionMembership {
                faction_id: NamedFactionId::CangyuanMerchants,
            },
        ));
        app.update();
        let registry = app.world().resource::<NamedFactionRegistry>();
        assert_eq!(
            registry
                .get(NamedFactionId::QingyunHunters)
                .unwrap()
                .current_npc_count,
            2,
            "census 必须把青云猎盟两名 NamedFactionMembership 计入 current_npc_count"
        );
        assert_eq!(
            registry
                .get(NamedFactionId::CangyuanMerchants)
                .unwrap()
                .current_npc_count,
            1,
            "census 必须把沧渊商会一名 NamedFactionMembership 计入 current_npc_count"
        );
        assert_eq!(
            registry
                .get(NamedFactionId::NorthWasteDrifters)
                .unwrap()
                .current_npc_count,
            0,
            "没有实体归属北荒漂流者时 census 应保持 0"
        );
    }

    #[test]
    fn named_faction_census_emits_leader_down_for_active_faction_without_leader() {
        let mut app = App::new();
        app.insert_resource(NamedFactionRegistry::startup_default());
        app.add_event::<NamedFactionLeaderDownEvent>();
        app.add_event::<NamedFactionDecayEvent>();
        app.add_systems(Update, sync_named_faction_census_system);
        app.world_mut().spawn((
            NpcMarker,
            NamedFactionMembership {
                faction_id: NamedFactionId::QingyunHunters,
            },
        ));

        app.update();

        let registry = app.world().resource::<NamedFactionRegistry>();
        assert_eq!(
            registry.get(NamedFactionId::QingyunHunters).unwrap().status,
            FactionStatus::Headless
        );
        let events = app
            .world()
            .resource::<Events<NamedFactionLeaderDownEvent>>();
        let emitted = events
            .iter_current_update_events()
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].faction, NamedFactionId::QingyunHunters);
        assert_eq!(emitted[0].zone, "qingyun_peaks");
    }

    #[test]
    fn named_faction_census_decays_zero_count_and_clears_player_membership() {
        let mut registry = NamedFactionRegistry::startup_default();
        registry
            .get_mut(NamedFactionId::QingyunHunters)
            .unwrap()
            .current_npc_count = 1;

        let mut app = App::new();
        app.insert_resource(registry);
        app.add_event::<NamedFactionLeaderDownEvent>();
        app.add_event::<NamedFactionDecayEvent>();
        app.add_systems(Update, sync_named_faction_census_system);
        let (client_bundle, _helper) = create_mock_client("Azure");
        let player = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .entity_mut(player)
            .insert(crate::social::components::FactionMembership {
                faction: FactionId::Attack,
                named_faction: Some(NamedFactionId::QingyunHunters),
                rank: 0,
                loyalty: 10,
                betrayal_count: 0,
                invite_block_until_tick: None,
                permanently_refused: false,
            });

        app.update();

        let registry = app.world().resource::<NamedFactionRegistry>();
        let qingyun = registry.get(NamedFactionId::QingyunHunters).unwrap();
        assert_eq!(qingyun.status, FactionStatus::Decayed);
        assert!(!qingyun.is_active);
        let membership = app
            .world()
            .get::<crate::social::components::FactionMembership>(player)
            .unwrap();
        assert_eq!(
            membership.named_faction, None,
            "势力消亡时在线玩家挂靠必须清空"
        );
        let events = app.world().resource::<Events<NamedFactionDecayEvent>>();
        let emitted = events
            .iter_current_update_events()
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].faction, NamedFactionId::QingyunHunters);
        assert_eq!(emitted[0].last_npc_count, 1);
    }
}
