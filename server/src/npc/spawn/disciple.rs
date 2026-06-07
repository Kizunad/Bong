use bevy_transform::components::{GlobalTransform, Transform};
use big_brain::prelude::{FirstToScore, Thinker, ThinkerBuilder};
use valence::entity::villager::VillagerEntityBundle;
use valence::prelude::{Commands, DVec3, Entity, EntityKind, EntityLayerId, Position};

use crate::cultivation::components::Realm;
use crate::npc::brain::{
    AgeingScorer, ChaseAction, CultivateAction, CultivateState, CultivationDriveHistory,
    CultivationDriveScorer, CuriosityScorer, FleeAction, GoToPoiAction, MeleeAttackAction,
    MeleeRangeScorer, PlayerProximityScorer, RetireAction, ReturnHomeAction, ReturnHomeScorer,
    SeclusionAction, SeclusionScorer, StallAction, StartDuXuAction, TradeStallScorer,
    TribulationReadyScorer, WanderAction, WanderScorer, WanderState,
};
use crate::npc::faction::{
    FactionId, FactionMembership, FactionRank, Lineage, MissionExecuteAction, MissionExecuteState,
    MissionQueue, MissionQueueScorer, Reputation,
};
use crate::npc::lifecycle::{npc_runtime_bundle, npc_runtime_bundle_with_age, NpcArchetype};
use crate::npc::lod::NpcLodTier;
use crate::npc::movement::{MovementController, MovementCooldowns};
use crate::npc::navigator::Navigator;
use crate::npc::patrol::NpcPatrol;
use crate::npc::relic::{
    GuardAction, GuardState, GuardianDuty, GuardianDutyScorer, GuardianRelicTag, TrialAction,
    TrialEval, TrialEvalScorer, TrialState,
};
use crate::npc::social::{FactionDuelScorer, SocializeAction, SocializeScorer, SocializeState};
use crate::npc::technique::{
    assign_npc_techniques, NpcLastTechniqueTick, NpcTechniqueAction, NpcTechniqueScorer,
};
use crate::npc::trade::assign_npc_trade_inventory;
use crate::skin::{initial_age_ratio, select_npc_visual_profile};

use super::common::{
    attach_player_skin, draw_npc_skin, spawn_rogue_commoner_base, NpcBlackboard, NpcCombatLoadout,
    NpcMarker, NpcSkinSpawnContext,
};

// ---------------------------------------------------------------------------
// Thinkers
// ---------------------------------------------------------------------------

#[allow(dead_code)]
pub(crate) fn disciple_npc_thinker() -> ThinkerBuilder {
    Thinker::build()
        .picker(FirstToScore { threshold: 0.05 })
        .when(AgeingScorer, RetireAction)
        .when(SeclusionScorer, SeclusionAction)
        .when(TribulationReadyScorer, StartDuXuAction)
        .when(NpcTechniqueScorer, NpcTechniqueAction)
        .when(MeleeRangeScorer, MeleeAttackAction)
        .when(FactionDuelScorer, ChaseAction)
        .when(PlayerProximityScorer, FleeAction)
        .when(MissionQueueScorer, MissionExecuteAction)
        .when(ReturnHomeScorer, ReturnHomeAction)
        .when(TradeStallScorer, StallAction)
        .when(CultivationDriveScorer, CultivateAction)
        .when(SocializeScorer, SocializeAction)
        .when(CuriosityScorer, GoToPoiAction::default())
        .when(WanderScorer, GoToPoiAction::default())
}

#[allow(dead_code)]
pub(crate) fn relic_guard_thinker() -> ThinkerBuilder {
    Thinker::build()
        .picker(FirstToScore { threshold: 0.05 })
        .when(GuardianDutyScorer, GuardAction)
        .when(NpcTechniqueScorer, NpcTechniqueAction)
        .when(MeleeRangeScorer, MeleeAttackAction)
        .when(TrialEvalScorer, TrialAction)
        .when(WanderScorer, WanderAction)
}

// ---------------------------------------------------------------------------
// Spawn
// ---------------------------------------------------------------------------

/// Spawn a Disciple（残宗余孽）NPC. 基于 Rogue 外观 + 挂 FactionMembership。
/// `faction_id` 决定所属派系（Attack / Defend / Neutral）。`master_id` 可选
/// 挂师承；缺省表示无师父。
#[allow(clippy::too_many_arguments)]
#[allow(dead_code)]
pub fn spawn_disciple_npc_at(
    commands: &mut Commands,
    skin_context: NpcSkinSpawnContext<'_>,
    layer: Entity,
    home_zone: &str,
    spawn_position: DVec3,
    patrol_target: DVec3,
    faction_id: FactionId,
    rank: FactionRank,
    realm: Realm,
    master_id: Option<String>,
    initial_age_ticks: f64,
) -> Entity {
    let loadout = NpcCombatLoadout::civilian();
    let profile = select_npc_visual_profile(
        NpcArchetype::Disciple,
        realm,
        Some(faction_id),
        Some(rank),
        initial_age_ratio(NpcArchetype::Disciple, initial_age_ticks),
    );
    let skin = draw_npc_skin(skin_context, profile, spawn_position);
    let entity = spawn_rogue_commoner_base(
        commands,
        layer,
        spawn_position,
        &skin,
        profile,
        loadout.clone(),
        NpcArchetype::Disciple,
        home_zone,
        patrol_target,
    );

    if let Some(skin) = skin {
        attach_player_skin(commands, entity, NpcArchetype::Disciple, skin);
    }

    commands.entity(entity).insert((
        WanderState::default(),
        CultivateState::default(),
        CultivationDriveHistory::default(),
        SocializeState::default(),
        MissionExecuteState::default(),
        FactionMembership {
            faction_id,
            rank,
            reputation: Reputation::default(),
            lineage: master_id.map(|id| Lineage {
                master_id: Some(id),
                disciple_ids: Vec::new(),
            }),
            mission_queue: MissionQueue::default(),
        },
        disciple_npc_thinker(),
    ));

    // P1: NPC 功法 + 交易库存
    let meridian_sys = crate::npc::technique::npc_meridian_system_for_realm(realm);
    let empty_deps = crate::cultivation::meridian::severed::SkillMeridianDependencies::default();
    let known_techniques = assign_npc_techniques(
        NpcArchetype::Disciple,
        realm,
        &meridian_sys,
        &empty_deps,
        None,
        entity.index() as u64,
    );
    let trade_inv =
        assign_npc_trade_inventory(NpcArchetype::Disciple, realm, entity.index() as u64);
    commands
        .entity(entity)
        .insert((known_techniques, NpcLastTechniqueTick::default(), trade_inv));

    let runtime = npc_runtime_bundle_with_age(entity, NpcArchetype::Disciple, initial_age_ticks);
    commands.entity(entity).insert(runtime);

    entity
}

/// Spawn 仙家遗种守护者（绑定遗迹 ID + 警戒范围）。外观复用 Villager。
#[allow(dead_code)]
pub fn spawn_relic_guard_npc_at(
    commands: &mut Commands,
    layer: Entity,
    home_zone: &str,
    relic_center: DVec3,
    alarm_radius: f64,
    relic_id: impl Into<String>,
    trial_template_id: impl Into<String>,
) -> Entity {
    let loadout = NpcCombatLoadout::fighter(NpcMeleeArchetype::Sword);
    let entity = commands
        .spawn((
            VillagerEntityBundle {
                kind: EntityKind::VILLAGER,
                layer: EntityLayerId(layer),
                position: Position::new([relic_center.x, relic_center.y, relic_center.z]),
                ..Default::default()
            },
            Transform::from_xyz(
                relic_center.x as f32,
                relic_center.y as f32,
                relic_center.z as f32,
            ),
            GlobalTransform::default(),
            NpcMarker,
            NpcBlackboard::default(),
            loadout.clone(),
            loadout.melee_archetype,
            loadout.melee_profile(),
            NpcArchetype::GuardianRelic,
            NpcLodTier::Dormant,
            Navigator::new(),
            MovementController::new(),
            loadout.movement_capabilities,
            MovementCooldowns::default(),
            NpcPatrol::new(home_zone, relic_center),
        ))
        .id();

    commands.entity(entity).insert((
        WanderState::default(),
        GuardState::default(),
        TrialState::default(),
        GuardianRelicTag,
        GuardianDuty::new(relic_id, relic_center).with_radius(alarm_radius),
        TrialEval::new(trial_template_id),
        relic_guard_thinker(),
    ));

    // P1: NPC 功法（GuardianRelic 默认 Spirit 境界）
    let guard_realm = Realm::Spirit;
    let meridian_sys = crate::npc::technique::npc_meridian_system_for_realm(guard_realm);
    let empty_deps = crate::cultivation::meridian::severed::SkillMeridianDependencies::default();
    let known_techniques = assign_npc_techniques(
        NpcArchetype::GuardianRelic,
        guard_realm,
        &meridian_sys,
        &empty_deps,
        None,
        entity.index() as u64,
    );
    commands
        .entity(entity)
        .insert((known_techniques, NpcLastTechniqueTick::default()));

    // GuardianRelic 不老：max_age 极大 + rate_multiplier 不会撞上限
    let runtime = npc_runtime_bundle(entity, NpcArchetype::GuardianRelic);
    commands.entity(entity).insert(runtime);

    entity
}

use super::common::NpcMeleeArchetype;
