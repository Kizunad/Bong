use bevy_transform::components::{GlobalTransform, Transform};
use valence::entity::marker::MarkerEntityBundle;
use valence::prelude::{
    bevy_ecs, App, Commands, Component, DVec3, Entity, EntityLayerId, IntoSystemConfigs, Position,
    Update,
};

use crate::body_plan::{BodyPlanRegistry, RaceId, RaceRegistry};
use crate::combat::body_mass::{BodyMass, Stance};
use crate::cultivation::known_techniques::{KnownTechnique, KnownTechniques};
use crate::cultivation::meridian::severed::MeridianSeveredPermanent;
use crate::fauna::components::{fauna_spawn_seed, BeastKind, FaunaTag};
use crate::fauna::visual::{entity_kind_for_beast, visual_kind_for_beast};
use crate::npc::hunger::Hunger;
use crate::npc::lifecycle::{npc_runtime_bundle, NpcArchetype};
use crate::npc::lod::NpcLodTier;
use crate::npc::movement::MovementController;
use crate::npc::navigator::Navigator;
use crate::npc::patrol::NpcPatrol;
use crate::npc::spawn::ambient_scheduler::{
    ambient_scheduler_system, AmbientMarkerData, AmbientSchedulerConfig, AmbientSchedulerState,
    ThreatBudget,
};
use crate::npc::spawn::{NpcBlackboard, NpcMarker};
use crate::npc::technique::npc_meridian_system_for_realm;
use crate::world::dimension::DimensionKind;
use crate::world::season::Season;
use crate::world::zone::Zone;

use super::brain::{wildlife_thinker, WildlifeBrain};
use super::config::{WildlifeAttributes, WildlifeCatalog};

#[derive(Clone, Debug, Component)]
pub struct WildlifeMarker {
    pub home_zone: String,
}

impl AmbientMarkerData for WildlifeMarker {
    fn new(_spawned_at: u64, home_zone: String) -> Self {
        Self { home_zone }
    }

    fn home_zone(&self) -> &str {
        &self.home_zone
    }

    fn requires_qi_settlement() -> bool {
        true
    }
}

/// 出生与后续群体协作使用相同的地理群落 ID，跨区/跨层不会互相召集。
pub fn herd_id(zone: &str, position: DVec3) -> u64 {
    wildlife_seed(
        zone,
        (position.x / 48.0).floor(),
        (position.z / 48.0).floor(),
    )
}

/// 浮点坐标的位模式低位稀疏，直接取模会让部分区域永远抽不到狮。
/// 使用 SplitMix64 的终混合步骤，让坐标高位也参与物种与境界抽样。
fn wildlife_seed(zone: &str, x: f64, z: f64) -> u64 {
    let mut value = fauna_spawn_seed(zone, x, z);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

pub fn choose_kind(zone: &Zone, position: DVec3) -> Option<BeastKind> {
    if zone.dimension != DimensionKind::Overworld || zone.name == "spawn" || zone.spirit_qi <= 0.0 {
        return None;
    }
    let roll = herd_id(&zone.name, position) % 10;
    Some(if zone.danger_level >= 3 && roll == 0 {
        BeastKind::DainuLion
    } else if zone.name.contains("wastes") || zone.name.contains("谷") || roll < 4 {
        BeastKind::FuyuVulture
    } else {
        BeastKind::Horse
    })
}

pub fn pool(
    commands: &mut Commands,
    layer: Entity,
    zone: &Zone,
    position: DVec3,
    origin: DVec3,
    _season: Season,
) -> Option<Entity> {
    let kind = choose_kind(zone, origin)?;
    Some(spawn_at(commands, layer, zone, position, kind, origin))
}

pub fn spawn_at(
    commands: &mut Commands,
    layer: Entity,
    zone: &Zone,
    position: DVec3,
    kind: BeastKind,
    origin: DVec3,
) -> Entity {
    let entity = commands.spawn_empty().id();
    let danger = zone.danger_level;
    let zone = zone.name.clone();
    commands.add(move |world: &mut bevy_ecs::world::World| {
        let definition = world.resource::<WildlifeCatalog>().get(kind).clone();
        let seed = wildlife_seed(&zone, position.x, position.z);
        let higher = seed % 5 == 0 && (kind != BeastKind::DainuLion || danger >= 4);
        let stats = &definition.realms[usize::from(higher)];
        let mut runtime = npc_runtime_bundle(entity, NpcArchetype::Beast, stats.realm);
        runtime.cultivation.race = RaceId::new(kind.as_str());
        let races = world.resource::<RaceRegistry>();
        let plans = world.resource::<BodyPlanRegistry>();
        let plan = crate::body_plan::resolve_race_to_plan(&runtime.cultivation.race, plans, races)
            .expect("野生生物身体构型必须已注册");
        runtime.meridian_system = npc_meridian_system_for_realm(stats.realm, plan);
        runtime.wounds.health_current = stats.health;
        runtime.wounds.health_max = stats.health;
        runtime.stamina.current = stats.stamina;
        runtime.stamina.max = stats.stamina;
        let mut skill_ids = vec![
            definition.engage_skill.clone(),
            definition.followup_skill.clone(),
        ];
        skill_ids.sort();
        skill_ids.dedup();
        let known = KnownTechniques {
            entries: skill_ids
                .into_iter()
                .map(|id| KnownTechnique {
                    id,
                    proficiency: 0.65,
                    active: true,
                })
                .collect(),
        };
        let group = herd_id(&zone, origin);
        world.entity_mut(entity).insert((
            MarkerEntityBundle {
                kind: entity_kind_for_beast(kind),
                layer: EntityLayerId(layer),
                position: Position(position),
                ..Default::default()
            },
            runtime,
            Transform::from_xyz(position.x as f32, position.y as f32, position.z as f32),
            GlobalTransform::default(),
            FaunaTag::new(kind),
            visual_kind_for_beast(kind).expect("野生生物渲染类型"),
            NpcMarker,
            NpcBlackboard::default(),
            NpcLodTier::Near,
            NpcPatrol::new(&zone, position),
            Hunger::default(),
            MeridianSeveredPermanent::default(),
            known,
        ));
        world.entity_mut(entity).insert((
            BodyMass::npc(definition.mass),
            Stance::Standing,
            Navigator::new(),
            MovementController::new(),
            crate::npc::movement::MovementCooldowns::default(),
            WildlifeMarker {
                home_zone: zone.clone(),
            },
            WildlifeAttributes {
                attack: stats.attack,
                damage_taken: stats.damage_taken,
                stamina: stats.stamina,
            },
            WildlifeBrain::new(kind, position, group, definition),
            wildlife_thinker(),
        ));
    });
    entity
}

fn budget(danger: u8) -> ThreatBudget {
    ThreatBudget {
        max_alive: if danger >= 4 { 9 } else { 6 },
        spawn_interval_ticks: 600,
        pack_size_range: (3, 3),
    }
}

pub fn register(app: &mut App) {
    let mut config = AmbientSchedulerConfig::<WildlifeMarker>::new(budget, pool, true);
    config.pack_size = Some(|zone, position| match choose_kind(zone, position) {
        Some(BeastKind::DainuLion) => 1,
        Some(_) => 3,
        None => 0,
    });
    app.init_resource::<AmbientSchedulerState<WildlifeMarker>>()
        .insert_resource(config)
        .add_systems(
            Update,
            ambient_scheduler_system::<WildlifeMarker>
                .in_set(crate::npc::spawn::ambient_scheduler::AmbientTerminalSystemSet::Recycle),
        );
}
