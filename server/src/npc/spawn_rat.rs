use bevy_transform::components::{GlobalTransform, Transform};
use big_brain::prelude::{FirstToScore, Thinker, ThinkerBuilder};
use valence::entity::entity::{CustomName, NameVisible};
use valence::entity::silverfish::SilverfishEntityBundle;
use valence::prelude::{
    bevy_ecs, ChunkPos, Commands, Component, DVec3, Entity, EntityKind, EntityLayerId, Position,
};

use crate::fauna::components::{BeastKind, FaunaTag};
use crate::fauna::rat_phase::{
    chunk_pos_from_world, rat_phase_display_name, PressureSensor, RatGroupId, RatPhase,
};
use crate::npc::brain::{WanderAction, WanderScorer, WanderState};
use crate::npc::brain_rat::{
    DrainedChunkAvoidScorer, GroupCohesionScorer, QiSourceProximityScorer, RegroupAction,
    SeekQiSourceAction,
};
use crate::npc::hunger::Hunger;
use crate::npc::lifecycle::{npc_runtime_bundle, NpcArchetype};
use crate::npc::movement::{MovementCapabilities, MovementController, MovementCooldowns};
use crate::npc::navigator::Navigator;
use crate::npc::patrol::NpcPatrol;
use crate::npc::spawn::{NpcBlackboard, NpcCombatLoadout, NpcMarker, NpcMeleeArchetype};

const RAT_THINKER_THRESHOLD: f32 = 0.05;

#[derive(Debug, Clone, PartialEq, Component)]
pub struct RatBlackboard {
    pub home_chunk: ChunkPos,
    pub home_zone: String,
    pub group_id: RatGroupId,
    pub last_pressure_target: Option<DVec3>,
    pub recently_drained: Vec<ChunkPos>,
    pub drained_qi: f64,
}

impl RatBlackboard {
    pub fn new(home_zone: &str, home_chunk: ChunkPos) -> Self {
        Self {
            home_chunk,
            home_zone: home_zone.to_string(),
            group_id: RatGroupId::for_zone_chunk(home_zone, home_chunk),
            last_pressure_target: None,
            recently_drained: Vec::new(),
            drained_qi: 0.0,
        }
    }
}

pub fn rat_npc_thinker() -> ThinkerBuilder {
    Thinker::build()
        .picker(FirstToScore {
            threshold: RAT_THINKER_THRESHOLD,
        })
        .when(DrainedChunkAvoidScorer, WanderAction)
        .when(GroupCohesionScorer, RegroupAction)
        .when(QiSourceProximityScorer, SeekQiSourceAction)
        .when(WanderScorer, WanderAction)
}

pub fn spawn_rat_npc_at(
    commands: &mut Commands,
    layer: Entity,
    home_zone: &str,
    spawn_position: DVec3,
    patrol_target: DVec3,
) -> Entity {
    let loadout = NpcCombatLoadout::new(
        NpcMeleeArchetype::Brawler,
        MovementCapabilities {
            can_sprint: true,
            can_dash: false,
        },
    );
    let home_chunk = chunk_pos_from_world(spawn_position);
    let rat_blackboard = RatBlackboard::new(home_zone, home_chunk);
    let group_id = rat_blackboard.group_id;
    let initial_phase = RatPhase::Solitary;
    let entity = commands
        .spawn(SilverfishEntityBundle {
            kind: EntityKind::SILVERFISH,
            layer: EntityLayerId(layer),
            position: Position::new([spawn_position.x, spawn_position.y, spawn_position.z]),
            entity_custom_name: CustomName(Some(rat_phase_display_name(&initial_phase))),
            entity_name_visible: NameVisible(true),
            ..Default::default()
        })
        .insert((
            Transform::from_xyz(
                spawn_position.x as f32,
                spawn_position.y as f32,
                spawn_position.z as f32,
            ),
            GlobalTransform::default(),
            NpcMarker,
            NpcBlackboard::default(),
            loadout.clone(),
            loadout.melee_archetype,
            loadout.melee_profile(),
            NpcArchetype::Beast,
            FaunaTag::new(BeastKind::Rat),
            initial_phase,
            group_id,
            PressureSensor::default(),
            rat_blackboard,
        ))
        .id();

    commands.entity(entity).insert((
        Navigator::new(),
        MovementController::new(),
        loadout.movement_capabilities,
        MovementCooldowns::default(),
        NpcPatrol::new(home_zone, patrol_target),
    ));

    commands.entity(entity).insert((
        Hunger::default(),
        WanderState::default(),
        rat_npc_thinker(),
        npc_runtime_bundle(entity, NpcArchetype::Beast),
    ));

    entity
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::testing::ScenarioSingleClient;

    #[test]
    fn spawn_rat_npc_at_attaches_fauna_tag_and_blackboard() {
        let scenario = ScenarioSingleClient::new();
        let layer = scenario.layer;
        let mut app = scenario.app;
        let rat = spawn_rat_npc_at(
            &mut app.world_mut().commands(),
            layer,
            "spawn",
            DVec3::new(1.0, 64.0, 1.0),
            DVec3::new(8.0, 64.0, 8.0),
        );
        app.world_mut().flush();

        assert_eq!(
            app.world().get::<FaunaTag>(rat).map(|tag| tag.beast_kind),
            Some(BeastKind::Rat)
        );
        let blackboard = app
            .world()
            .get::<RatBlackboard>(rat)
            .expect("rat spawn should attach RatBlackboard");
        assert_eq!(blackboard.home_zone, "spawn");
        assert_eq!(blackboard.home_chunk, ChunkPos::new(0, 0));
    }

    #[test]
    fn spawn_rat_npc_at_uses_silverfish_entity_kind() {
        let scenario = ScenarioSingleClient::new();
        let layer = scenario.layer;
        let mut app = scenario.app;
        let rat = spawn_rat_npc_at(
            &mut app.world_mut().commands(),
            layer,
            "spawn",
            DVec3::new(1.0, 64.0, 1.0),
            DVec3::new(8.0, 64.0, 8.0),
        );
        app.world_mut().flush();

        assert_eq!(
            app.world().get::<EntityKind>(rat),
            Some(&EntityKind::SILVERFISH)
        );
        assert_eq!(
            app.world()
                .get::<CustomName>(rat)
                .and_then(|name| name.0.clone()),
            Some(rat_phase_display_name(&RatPhase::Solitary))
        );
        assert_eq!(
            app.world().get::<NameVisible>(rat),
            Some(&NameVisible(true))
        );
    }
}
