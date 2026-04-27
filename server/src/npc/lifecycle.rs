use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::json;
use valence::prelude::{
    bevy_ecs, App, Bundle, Commands, Component, Despawned, Entity, Event, EventReader, EventWriter,
    IntoSystemConfigs, PreUpdate, Query, Res, ResMut, Resource, Update, With, Without,
};

use crate::combat::components::{
    CombatState, DerivedAttrs, Lifecycle, LifecycleState, Stamina, StatusEffects, Wounds,
};
use crate::cultivation::components::{Contamination, Cultivation, MeridianSystem};
use crate::cultivation::death_hooks::{
    CultivationDeathCause, CultivationDeathTrigger, PlayerTerminated,
};
use crate::cultivation::life_record::LifeRecord;
use crate::cultivation::lifespan::{DeathRegistry, LifespanComponent, LifespanExtensionLedger};
use crate::npc::brain::canonical_npc_id;
use crate::npc::spawn::NpcMarker;

type RegistryNpcQuery<'w, 's> = Query<
    'w,
    's,
    (&'static NpcArchetype, Option<&'static Lifecycle>),
    (With<NpcMarker>, Without<Despawned>),
>;

type ActiveNpcFilter = (
    With<NpcMarker>,
    Without<Despawned>,
    Without<PendingRetirement>,
);
type SharedAgingNpcQuery<'w, 's> =
    Query<'w, 's, (&'static mut NpcLifespan, Option<&'static LifespanComponent>), ActiveNpcFilter>;
type TerminatedNpcQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static NpcArchetype,
        &'static NpcLifespan,
        Option<&'static PendingRetirement>,
        Option<&'static LifespanComponent>,
    ),
    With<NpcMarker>,
>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, Component)]
#[serde(rename_all = "snake_case")]
pub enum NpcArchetype {
    #[default]
    Zombie,
    Commoner,
    Rogue,
    Beast,
    Disciple,
    GuardianRelic,
}

impl NpcArchetype {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Zombie => "zombie",
            Self::Commoner => "commoner",
            Self::Rogue => "rogue",
            Self::Beast => "beast",
            Self::Disciple => "disciple",
            Self::GuardianRelic => "guardian_relic",
        }
    }

    pub const fn default_max_age_ticks(self) -> f64 {
        match self {
            Self::Zombie => 120_000.0,
            Self::Commoner => 90_000.0,
            Self::Rogue => 110_000.0,
            Self::Beast => 80_000.0,
            Self::Disciple => 140_000.0,
            Self::GuardianRelic => 1_000_000.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Component, Serialize, Deserialize)]
pub struct NpcLifespan {
    pub age_ticks: f64,
    pub max_age_ticks: f64,
}

impl NpcLifespan {
    pub const fn new(age_ticks: f64, max_age_ticks: f64) -> Self {
        Self {
            age_ticks,
            max_age_ticks,
        }
    }

    pub fn for_archetype(archetype: NpcArchetype) -> Self {
        Self::new(0.0, archetype.default_max_age_ticks())
    }

    pub fn age_ratio(&self) -> f64 {
        if self.max_age_ticks <= f64::EPSILON {
            1.0
        } else {
            (self.age_ticks / self.max_age_ticks).clamp(0.0, 16.0)
        }
    }

    pub fn is_expired(&self) -> bool {
        self.age_ticks >= self.max_age_ticks
    }
}

#[derive(Clone, Copy, Debug, Resource)]
pub struct NpcAgingConfig {
    pub enabled: bool,
    pub rate_multiplier: f64,
}

impl Default for NpcAgingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            rate_multiplier: 0.3,
        }
    }
}

#[derive(Clone, Debug, Resource)]
pub struct NpcRegistry {
    pub live_npc_count: usize,
    pub max_npc_count: usize,
    pub resume_npc_count: usize,
    pub spawn_paused: bool,
    pub counts_by_archetype: HashMap<NpcArchetype, usize>,
}

impl Default for NpcRegistry {
    fn default() -> Self {
        Self {
            live_npc_count: 0,
            max_npc_count: 512,
            resume_npc_count: 460,
            spawn_paused: false,
            counts_by_archetype: HashMap::new(),
        }
    }
}

impl NpcRegistry {
    pub fn refresh_from_counts(
        &mut self,
        live_npc_count: usize,
        counts_by_archetype: HashMap<NpcArchetype, usize>,
    ) {
        self.live_npc_count = live_npc_count;
        self.counts_by_archetype = counts_by_archetype;

        if self.live_npc_count >= self.max_npc_count {
            self.spawn_paused = true;
        } else if self.live_npc_count < self.resume_npc_count {
            self.spawn_paused = false;
        }
    }

    pub fn reserve_spawn_batch(&mut self, desired: usize) -> usize {
        if desired == 0 {
            return 0;
        }

        if self.spawn_paused && self.live_npc_count >= self.resume_npc_count {
            return 0;
        }

        let remaining = self.max_npc_count.saturating_sub(self.live_npc_count);
        let granted = desired.min(remaining);
        self.live_npc_count = self.live_npc_count.saturating_add(granted);
        if self.live_npc_count >= self.max_npc_count {
            self.spawn_paused = true;
        }
        granted
    }

    pub fn should_reduce_population(&self) -> bool {
        self.live_npc_count >= self.max_npc_count
    }
}

#[derive(Clone, Copy, Debug, Component)]
pub struct PendingRetirement;

#[derive(Clone, Debug, Event)]
pub struct NpcRetireRequest {
    pub entity: Entity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NpcDeathReason {
    NaturalAging,
}

#[derive(Clone, Debug, Event)]
#[allow(dead_code)]
pub struct NpcDeathNotice {
    pub npc_id: String,
    pub archetype: NpcArchetype,
    pub reason: NpcDeathReason,
    pub age_ticks: f64,
    pub max_age_ticks: f64,
}

pub fn register(app: &mut App) {
    app.insert_resource(NpcAgingConfig::default())
        .insert_resource(NpcRegistry::default())
        .add_event::<CultivationDeathTrigger>()
        .add_event::<PlayerTerminated>()
        .add_event::<NpcRetireRequest>()
        .add_event::<NpcDeathNotice>()
        .add_systems(
            PreUpdate,
            (update_npc_registry, age_npcs).before(big_brain::prelude::BigBrainSet::Scorers),
        )
        .add_systems(Update, (process_npc_retire_requests, handle_npc_terminated));
}

#[derive(Bundle)]
pub struct NpcRuntimeBundle {
    pub archetype: NpcArchetype,
    pub lifespan: NpcLifespan,
    pub shared_lifespan: LifespanComponent,
    pub death_registry: DeathRegistry,
    pub life_record: LifeRecord,
    pub lifespan_extension_ledger: LifespanExtensionLedger,
    pub cultivation: Cultivation,
    pub meridian_system: MeridianSystem,
    pub contamination: Contamination,
    pub wounds: Wounds,
    pub stamina: Stamina,
    pub combat_state: CombatState,
    pub status_effects: StatusEffects,
    pub derived_attrs: DerivedAttrs,
    pub lifecycle: Lifecycle,
}

pub fn npc_runtime_bundle(entity: Entity, archetype: NpcArchetype) -> NpcRuntimeBundle {
    let char_id = canonical_npc_id(entity);
    NpcRuntimeBundle {
        archetype,
        lifespan: NpcLifespan::for_archetype(archetype),
        shared_lifespan: LifespanComponent::for_realm(Cultivation::default().realm),
        death_registry: DeathRegistry::new(char_id.clone()),
        life_record: LifeRecord::new(char_id.clone()),
        lifespan_extension_ledger: LifespanExtensionLedger::default(),
        cultivation: Cultivation::default(),
        meridian_system: MeridianSystem::default(),
        contamination: Contamination::default(),
        wounds: Wounds::default(),
        stamina: Stamina::default(),
        combat_state: CombatState::default(),
        status_effects: StatusEffects::default(),
        derived_attrs: DerivedAttrs::default(),
        lifecycle: Lifecycle {
            character_id: char_id,
            fortune_remaining: 0,
            ..Default::default()
        },
    }
}

fn update_npc_registry(mut registry: ResMut<NpcRegistry>, npcs: RegistryNpcQuery<'_, '_>) {
    let mut counts_by_archetype = HashMap::new();
    let mut live_npc_count = 0usize;

    for (archetype, lifecycle) in &npcs {
        if lifecycle.is_some_and(|lifecycle| lifecycle.state == LifecycleState::Terminated) {
            continue;
        }

        live_npc_count += 1;
        *counts_by_archetype.entry(*archetype).or_default() += 1;
    }

    registry.refresh_from_counts(live_npc_count, counts_by_archetype);
}

fn age_npcs(config: Res<NpcAgingConfig>, mut npcs: SharedAgingNpcQuery<'_, '_>) {
    if !config.enabled {
        return;
    }

    for (mut npc_lifespan, shared_lifespan) in &mut npcs {
        if let Some(shared_lifespan) = shared_lifespan {
            let ratio = if shared_lifespan.cap_by_realm == 0 {
                1.0
            } else {
                (shared_lifespan.years_lived / shared_lifespan.cap_by_realm as f64).clamp(0.0, 1.0)
            };
            npc_lifespan.age_ticks = npc_lifespan.max_age_ticks * ratio;
        } else {
            npc_lifespan.age_ticks += config.rate_multiplier.max(0.0);
        }
    }
}

fn process_npc_retire_requests(
    mut retire_requests: EventReader<NpcRetireRequest>,
    npcs: Query<(&NpcArchetype, &NpcLifespan), With<NpcMarker>>,
    mut cultivation_deaths: EventWriter<CultivationDeathTrigger>,
) {
    for request in retire_requests.read() {
        let Ok((archetype, lifespan)) = npcs.get(request.entity) else {
            continue;
        };

        cultivation_deaths.send(CultivationDeathTrigger {
            entity: request.entity,
            cause: CultivationDeathCause::NaturalAging,
            context: json!({
                "npc_id": canonical_npc_id(request.entity),
                "archetype": archetype.as_str(),
                "age_ticks": lifespan.age_ticks,
                "max_age_ticks": lifespan.max_age_ticks,
                "age_ratio": lifespan.age_ratio(),
                "reason": "retire_action",
            }),
        });
    }
}

fn handle_npc_terminated(
    mut commands: Commands,
    mut terminated: EventReader<PlayerTerminated>,
    npcs: TerminatedNpcQuery<'_, '_>,
    mut notices: EventWriter<NpcDeathNotice>,
) {
    for event in terminated.read() {
        let Ok((archetype, lifespan, pending_retirement, shared_lifespan)) = npcs.get(event.entity)
        else {
            continue;
        };

        if pending_retirement.is_some()
            || lifespan.is_expired()
            || shared_lifespan.is_some_and(|lifespan| lifespan.remaining_years() <= f64::EPSILON)
        {
            notices.send(NpcDeathNotice {
                npc_id: canonical_npc_id(event.entity),
                archetype: *archetype,
                reason: NpcDeathReason::NaturalAging,
                age_ticks: lifespan.age_ticks,
                max_age_ticks: lifespan.max_age_ticks,
            });
        }

        if let Some(mut entity_commands) = commands.get_entity(event.entity) {
            entity_commands.insert(Despawned);
            entity_commands.remove::<PendingRetirement>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use valence::prelude::{App, Update};

    #[test]
    fn registry_hysteresis_pauses_at_cap_and_resumes_below_low_watermark() {
        let mut registry = NpcRegistry::default();

        registry.refresh_from_counts(512, HashMap::new());
        assert!(registry.spawn_paused);

        registry.refresh_from_counts(500, HashMap::new());
        assert!(
            registry.spawn_paused,
            "should remain paused until low watermark"
        );

        registry.refresh_from_counts(459, HashMap::new());
        assert!(!registry.spawn_paused, "should resume below low watermark");
    }

    #[test]
    fn reserve_spawn_batch_clamps_to_remaining_capacity() {
        let mut registry = NpcRegistry::default();
        registry.refresh_from_counts(510, HashMap::new());

        let granted = registry.reserve_spawn_batch(8);
        assert_eq!(granted, 2);
        assert_eq!(registry.live_npc_count, 512);
        assert!(registry.spawn_paused);
    }

    #[test]
    fn process_retire_requests_emits_natural_aging_trigger() {
        let mut app = App::new();
        app.add_event::<NpcRetireRequest>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_systems(Update, process_npc_retire_requests);

        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::Zombie,
                NpcLifespan::new(99.0, 100.0),
            ))
            .id();

        app.world_mut().send_event(NpcRetireRequest { entity });
        app.update();

        let events = app
            .world()
            .resource::<bevy_ecs::event::Events<CultivationDeathTrigger>>();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn handle_npc_terminated_emits_notice_and_marks_despawned() {
        let mut app = App::new();
        app.add_event::<PlayerTerminated>();
        app.add_event::<NpcDeathNotice>();
        app.add_systems(Update, handle_npc_terminated);

        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::Zombie,
                NpcLifespan::new(120.0, 100.0),
                PendingRetirement,
            ))
            .id();

        app.world_mut().send_event(PlayerTerminated { entity });
        app.update();

        let events = app
            .world()
            .resource::<bevy_ecs::event::Events<NpcDeathNotice>>();
        assert_eq!(events.len(), 1);
        assert!(app.world().get::<Despawned>(entity).is_some());
    }

    #[test]
    fn npc_shared_lifespan_syncs_to_ai_age_view() {
        let mut app = App::new();
        app.insert_resource(NpcAgingConfig::default());
        app.add_systems(Update, age_npcs);

        let mut shared_lifespan = LifespanComponent::new(100);
        shared_lifespan.years_lived = 75.0;
        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::Commoner,
                NpcLifespan::new(0.0, 200.0),
                shared_lifespan,
            ))
            .id();

        app.update();

        let lifespan = app.world().get::<NpcLifespan>(entity).unwrap();
        assert_eq!(lifespan.age_ticks, 150.0);
    }

    #[test]
    fn npc_death_notice_fields_are_readable_for_bridge_consumers() {
        let notice = NpcDeathNotice {
            npc_id: "npc_1v1".to_string(),
            archetype: NpcArchetype::Zombie,
            reason: NpcDeathReason::NaturalAging,
            age_ticks: 120.0,
            max_age_ticks: 100.0,
        };

        assert_eq!(notice.npc_id, "npc_1v1");
        assert_eq!(notice.archetype, NpcArchetype::Zombie);
        assert_eq!(notice.reason, NpcDeathReason::NaturalAging);
        assert_eq!(notice.age_ticks, 120.0);
        assert_eq!(notice.max_age_ticks, 100.0);
    }
}
