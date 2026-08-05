use bevy_transform::components::{GlobalTransform, Transform};
use big_brain::prelude::ThinkerBuilder;
use valence::entity::player::PlayerEntityBundle;
use valence::entity::villager::VillagerEntityBundle;
use valence::entity::witch::WitchEntityBundle;
use valence::prelude::{
    bevy_ecs, Commands, Component, DVec3, Entity, EntityKind, EntityLayerId, Position, UniqueId,
};

use crate::combat::components::WoundKind;
use crate::combat::events::{AttackReach, FIST_REACH, SPEAR_REACH, SWORD_REACH};
use crate::npc::brain::threat::{SelfInterestDecision, ThreatAssessment};
use crate::npc::brain::{canonical_npc_id, GoToPoiState, RestState, StallState};
use crate::npc::lifecycle::{NpcArchetype, NpcSpawnNotice, NpcSpawnSource};
use crate::npc::lod::NpcLodTier;
use crate::npc::movement::{MovementCapabilities, MovementController, MovementCooldowns};
use crate::npc::navigator::Navigator;
use crate::npc::patrol::NpcPatrol;
use crate::npc::schedule::{home_base_for_archetype, schedule_seed_from_char_id, NpcDailySchedule};
use crate::skin::faction_tint::visual_equipment;
use crate::skin::{
    npc_uuid, NpcPlayerSkin, NpcSkinFallbackPolicy, NpcVisualProfile, SignedSkin, SkinPool,
};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, Component)]
pub struct NpcMarker;

pub struct NpcSkinSpawnContext<'a> {
    pub pool: Option<&'a mut SkinPool>,
    #[allow(dead_code)]
    pub policy: NpcSkinFallbackPolicy,
}

impl NpcSkinSpawnContext<'_> {
    pub const fn new(
        pool: Option<&mut SkinPool>,
        policy: NpcSkinFallbackPolicy,
    ) -> NpcSkinSpawnContext<'_> {
        NpcSkinSpawnContext { pool, policy }
    }
}

#[derive(Clone, Copy, Debug, Component, PartialEq, Eq)]
pub(crate) enum DeferredNpcBrain {
    ScatteredCultivator,
}

impl DeferredNpcBrain {
    pub(crate) fn build(self) -> ThinkerBuilder {
        match self {
            Self::ScatteredCultivator => super::rogue::scattered_cultivator_thinker(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Component)]
pub enum NpcMeleeArchetype {
    #[default]
    Brawler,
    Sword,
    Spear,
}

impl NpcMeleeArchetype {
    pub const fn profile(self) -> NpcMeleeProfile {
        match self {
            Self::Brawler => NpcMeleeProfile::fist(),
            Self::Sword => NpcMeleeProfile::sword(),
            Self::Spear => NpcMeleeProfile::spear(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Component)]
pub struct NpcMeleeProfile {
    pub reach: AttackReach,
    pub wound_kind: WoundKind,
    pub preferred_distance: f32,
    pub disengage_distance: f32,
}

impl NpcMeleeProfile {
    pub const fn from_reach(reach: AttackReach, wound_kind: WoundKind) -> Self {
        Self {
            reach,
            wound_kind,
            preferred_distance: reach.base,
            disengage_distance: reach.max * 1.5,
        }
    }

    pub const fn fist() -> Self {
        Self::from_reach(FIST_REACH, WoundKind::Blunt)
    }

    pub const fn sword() -> Self {
        Self::from_reach(SWORD_REACH, WoundKind::Cut)
    }

    pub const fn spear() -> Self {
        Self::from_reach(SPEAR_REACH, WoundKind::Pierce)
    }
}

impl Default for NpcMeleeProfile {
    fn default() -> Self {
        NpcMeleeArchetype::default().profile()
    }
}

#[derive(Clone, Debug, Component)]
pub struct NpcCombatLoadout {
    pub melee_archetype: NpcMeleeArchetype,
    pub movement_capabilities: MovementCapabilities,
}

impl NpcCombatLoadout {
    pub const fn new(
        melee_archetype: NpcMeleeArchetype,
        movement_capabilities: MovementCapabilities,
    ) -> Self {
        Self {
            melee_archetype,
            movement_capabilities,
        }
    }

    pub const fn civilian() -> Self {
        Self::new(
            NpcMeleeArchetype::Brawler,
            MovementCapabilities {
                can_sprint: true,
                can_dash: false,
            },
        )
    }

    pub const fn fighter(melee_archetype: NpcMeleeArchetype) -> Self {
        Self::new(
            melee_archetype,
            MovementCapabilities {
                can_sprint: true,
                can_dash: true,
            },
        )
    }

    pub const fn melee_profile(&self) -> NpcMeleeProfile {
        self.melee_archetype.profile()
    }
}

impl Default for NpcCombatLoadout {
    fn default() -> Self {
        Self::civilian()
    }
}

/// Override target for NPC-vs-NPC scenarios (e.g. duel).
/// When present, the NPC targets this entity instead of the nearest player.
#[derive(Clone, Copy, Debug, Component)]
pub struct DuelTarget(pub Entity);

/// Passive bait target for NPC aggression redirection.
/// The inner entity is the trap owner; combat targeting uses the component entity itself.
#[derive(Clone, Copy, Debug, Component)]
pub struct DecoyTarget(pub Entity);

impl DecoyTarget {
    pub fn owner(&self) -> Entity {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Component)]
#[allow(dead_code, unfulfilled_lint_expectations)]
pub struct NpcBlackboard {
    pub nearest_player: Option<Entity>,
    pub player_distance: f32,
    /// Cached world position of the current target (player or duel opponent).
    pub target_position: Option<DVec3>,
    /// GameTick of the last melee attack (for cooldown tracking).
    pub last_melee_tick: u32,
    /// Composite threat assessment for the nearest player (P2).
    pub threat_assessment: Option<ThreatAssessment>,
    /// Self-interest decision derived from threat assessment + memory (P2).
    pub self_interest_decision: Option<SelfInterestDecision>,
    /// Immediate retaliation: (attacker entity, expire tick).
    /// Set when the NPC takes damage; overrides chase/melee scorers until it expires.
    pub retaliation_target: Option<(Entity, u64)>,
    /// Passive bait target: (decoy entity, expire tick).
    /// Recomputed by `update_npc_blackboard`; never overrides duel or retaliation targets.
    pub decoy_target: Option<(Entity, u64)>,
}

impl Default for NpcBlackboard {
    fn default() -> Self {
        Self {
            nearest_player: None,
            player_distance: f32::INFINITY,
            target_position: None,
            last_melee_tick: 0,
            threat_assessment: None,
            self_interest_decision: None,
            retaliation_target: None,
            decoy_target: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

pub fn snap_spawn_y_to_surface(
    pos: DVec3,
    terrain: Option<&impl crate::world::terrain::SurfaceProvider>,
) -> DVec3 {
    if let Some(terrain) = terrain {
        let info = terrain.query_surface(pos.x.floor() as i32, pos.z.floor() as i32);
        if info.passable {
            return DVec3::new(pos.x, f64::from(info.y + 1), pos.z);
        }
    }
    pos
}

pub(crate) fn draw_npc_skin(
    skin_context: NpcSkinSpawnContext<'_>,
    profile: NpcVisualProfile,
    spawn_position: DVec3,
) -> Option<SignedSkin> {
    let pool = skin_context.pool?;
    if !pool.ready_for_spawn() {
        return None;
    }

    let salt = skin_salt(spawn_position);
    Some(pool.next_for_profile(profile, salt))
}

pub(crate) fn skin_salt(spawn_position: DVec3) -> u64 {
    spawn_position.x.to_bits()
        ^ spawn_position.y.to_bits().rotate_left(17)
        ^ spawn_position.z.to_bits().rotate_left(31)
}

pub(crate) fn schedule_seed_for_entity(entity: Entity) -> u64 {
    schedule_seed_from_char_id(canonical_npc_id(entity).as_str())
}

pub(crate) fn npc_skin_name(entity: Entity, archetype: NpcArchetype) -> String {
    let mut name = format!("bong_{}_{}", archetype.as_str(), entity.index());
    name.truncate(16);
    name
}

pub(crate) fn attach_player_skin(
    commands: &mut Commands,
    entity: Entity,
    archetype: NpcArchetype,
    skin: SignedSkin,
) {
    let uuid = npc_uuid(entity);
    commands.entity(entity).insert((
        UniqueId(uuid),
        NpcPlayerSkin {
            uuid,
            name: npc_skin_name(entity, archetype),
            skin,
        },
    ));
}

pub fn fallback_rogue_commoner_kind(skin: &Option<SignedSkin>) -> EntityKind {
    if skin.is_some() {
        EntityKind::PLAYER
    } else {
        EntityKind::VILLAGER
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_rogue_commoner_base(
    commands: &mut Commands,
    layer: Entity,
    spawn_position: DVec3,
    skin: &Option<SignedSkin>,
    profile: NpcVisualProfile,
    loadout: NpcCombatLoadout,
    archetype: NpcArchetype,
    home_zone: &str,
    patrol_target: DVec3,
) -> Entity {
    let mut entity_commands = commands.spawn_empty();
    match fallback_rogue_commoner_kind(skin) {
        EntityKind::PLAYER => {
            entity_commands.insert(PlayerEntityBundle {
                kind: EntityKind::PLAYER,
                layer: EntityLayerId(layer),
                position: Position::new([spawn_position.x, spawn_position.y, spawn_position.z]),
                ..Default::default()
            });
        }
        EntityKind::WITCH => {
            entity_commands.insert(WitchEntityBundle {
                kind: EntityKind::WITCH,
                layer: EntityLayerId(layer),
                position: Position::new([spawn_position.x, spawn_position.y, spawn_position.z]),
                ..Default::default()
            });
        }
        _ => {
            entity_commands.insert(VillagerEntityBundle {
                kind: EntityKind::VILLAGER,
                layer: EntityLayerId(layer),
                position: Position::new([spawn_position.x, spawn_position.y, spawn_position.z]),
                ..Default::default()
            });
        }
    }

    let entity = entity_commands
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
            archetype,
            NpcLodTier::Dormant,
            Navigator::new(),
            MovementController::new(),
            loadout.movement_capabilities,
            MovementCooldowns::default(),
            NpcPatrol::new(home_zone, patrol_target),
        ))
        .id();
    commands.entity(entity).insert((
        NpcDailySchedule::for_archetype(archetype, schedule_seed_for_entity(entity)),
        home_base_for_archetype(archetype, patrol_target),
        GoToPoiState::default(),
        StallState::default(),
        RestState::default(),
    ));
    commands
        .entity(entity)
        .insert((profile, visual_equipment(&profile)));
    entity
}

pub fn spawn_notice(
    entity: Entity,
    archetype: NpcArchetype,
    source: NpcSpawnSource,
    home_zone: &str,
    position: DVec3,
    initial_age_ticks: f64,
) -> NpcSpawnNotice {
    NpcSpawnNotice {
        npc_id: crate::npc::brain::canonical_npc_id(entity),
        archetype,
        source,
        home_zone: home_zone.to_string(),
        position,
        initial_age_ticks,
    }
}
