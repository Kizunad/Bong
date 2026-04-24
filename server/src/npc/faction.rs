use big_brain::prelude::{ActionBuilder, ActionState, Actor, BigBrainSet, Score, ScorerBuilder};
use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, App, Commands, Component, DVec3, Entity, IntoSystemConfigs, Position, PreUpdate,
    Query, Res, Resource, Update, With,
};

use crate::npc::navigator::Navigator;
use crate::npc::spawn::{DuelTarget, NpcMarker};

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

/// MissionQueueScorer 饱和阈值：pending ≥ 此值时分数封顶 1.0。
pub const MISSION_QUEUE_SCORER_CAP: u32 = 3;
/// Disciple 执行单个任务的最大 tick 数（超时 Success，避免卡死）。
pub const MISSION_EXECUTE_MAX_TICKS: u32 = 600;

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

impl ActionBuilder for MissionExecuteAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }
    fn label(&self) -> Option<&str> {
        Some("MissionExecuteAction")
    }
}

pub fn register(app: &mut App) {
    app.insert_resource(FactionStore::default());
    app.add_systems(Update, assign_hostile_encounters);
    app.add_systems(
        PreUpdate,
        (loyalty_scorer_system, mission_queue_scorer_system).in_set(BigBrainSet::Scorers),
    );
    app.add_systems(
        PreUpdate,
        mission_execute_action_system.in_set(BigBrainSet::Actions),
    );
}

fn loyalty_scorer_system(
    store: Res<FactionStore>,
    members: Query<&FactionMembership, With<NpcMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<LoyaltyScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let value = match members.get(*actor) {
            Ok(membership) => {
                let bias = store
                    .iter()
                    .find(|f| f.id == membership.faction_id)
                    .map(|f| f.loyalty_bias)
                    .unwrap_or(0.5);
                ((membership.reputation.loyalty() + bias) * 0.5).clamp(0.0, 1.0) as f32
            }
            Err(_) => 0.0,
        };
        score.set(value);
    }
}

fn mission_queue_scorer_system(
    members: Query<&FactionMembership, With<NpcMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<MissionQueueScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let value = match members.get(*actor) {
            Ok(m) => {
                let pending = m.mission_queue.pending_count().min(MISSION_QUEUE_SCORER_CAP);
                (pending as f32 / MISSION_QUEUE_SCORER_CAP as f32).clamp(0.0, 1.0)
            }
            Err(_) => 0.0,
        };
        score.set(value);
    }
}

/// 最小 MissionExecuteAction：停 Navigator → 计时 → 达到上限 pop 掉
/// 队首任务 → Success。真实剧本由 plan-quest-v1 替换。
fn mission_execute_action_system(
    mut members: Query<
        (&mut FactionMembership, &mut Navigator, &mut MissionExecuteState),
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

type EncounterNpcQueryItem<'a> = (
    Entity,
    &'a Position,
    Option<&'a FactionMembership>,
    Option<&'a DuelTarget>,
);

#[allow(clippy::type_complexity)]
fn assign_hostile_encounters(
    faction_store: Res<FactionStore>,
    npc_positions: Query<EncounterNpcQueryItem<'_>, With<NpcMarker>>,
    mut commands: valence::prelude::Commands,
) {
    let npcs = npc_positions
        .iter()
        .map(|(entity, position, membership, duel_target)| {
            (
                entity,
                position.get(),
                membership.map(|membership| membership.faction_id),
                duel_target.map(|target| target.0),
            )
        })
        .collect::<Vec<_>>();

    for (entity, position, faction_id, duel_target) in &npcs {
        let Some(faction_id) = faction_id else {
            if duel_target.is_some() {
                commands.entity(*entity).remove::<DuelTarget>();
            }
            continue;
        };

        let nearest_hostile = npcs
            .iter()
            .filter_map(|(other_entity, other_position, other_faction_id, _)| {
                let other_faction_id = (*other_faction_id)?;
                if *other_entity == *entity
                    || !faction_store.is_hostile_pair(*faction_id, other_faction_id)
                {
                    return None;
                }

                let distance_sq = planar_distance_sq(*position, *other_position);
                (distance_sq <= 16.0 * 16.0).then_some((*other_entity, distance_sq))
            })
            .min_by(|left, right| left.1.total_cmp(&right.1))
            .map(|(target, _)| target);

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
        assert!(got >= 0.0 && got <= 1.0);
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
        app.add_systems(
            PreUpdate,
            mission_execute_action_system.in_set(BigBrainSet::Actions),
        );
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
}
