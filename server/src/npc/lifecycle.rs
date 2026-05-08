use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::json;
use valence::prelude::{
    bevy_ecs, Added, App, Bundle, Commands, Component, DVec3, Despawned, Entity, Event,
    EventReader, EventWriter, FixedUpdate, IntoSystemConfigs, Position, Query, Res, ResMut,
    Resource, Update, With, Without,
};

use crate::combat::components::{
    CombatState, DerivedAttrs, Lifecycle, LifecycleState, Stamina, StatusEffects, Wounds,
};
use crate::cultivation::components::{Contamination, Cultivation, MeridianSystem};
use crate::cultivation::death_hooks::{
    CultivationDeathCause, CultivationDeathTrigger, PlayerTerminated,
};
use crate::cultivation::life_record::LifeRecord;
use crate::cultivation::lifespan::{
    DeathRegistry, LifespanCapTable, LifespanComponent, LifespanExtensionLedger,
};
use crate::cultivation::possession::PossessedVictim;
use crate::npc::brain::canonical_npc_id;
use crate::npc::faction::{FactionId, FactionMembership};
use crate::npc::patrol::NpcPatrol;
use crate::npc::spawn::NpcMarker;

type RegistryNpcQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static NpcArchetype,
        Option<&'static Lifecycle>,
        Option<&'static NpcPatrol>,
    ),
    (With<NpcMarker>, Without<Despawned>),
>;

type ActiveNpcFilter = (
    With<NpcMarker>,
    Without<Despawned>,
    Without<PendingRetirement>,
);
type SharedAgingNpcQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static NpcArchetype,
        &'static mut NpcLifespan,
        Option<&'static LifespanComponent>,
    ),
    ActiveNpcFilter,
>;
type TerminatedNpcQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static NpcArchetype,
        &'static NpcLifespan,
        Option<&'static PendingRetirement>,
        Option<&'static LifespanComponent>,
        Option<&'static FactionMembership>,
        Option<&'static LifeRecord>,
    ),
    With<NpcMarker>,
>;

type DespawnedNpcNoticeQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static NpcArchetype,
        &'static NpcLifespan,
        Option<&'static LifespanComponent>,
        Option<&'static FactionMembership>,
        Option<&'static LifeRecord>,
        Option<&'static PossessedVictim>,
    ),
    (
        With<NpcMarker>,
        With<Despawned>,
        Added<Despawned>,
        Without<NpcDeathNoticeEmitted>,
    ),
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
    /// plan-tsy-lifecycle-v1 §4 — 道伥（干尸激活后的诡异 NPC，伪装平凡 / 突袭玩家）。
    Daoxiang,
    /// plan-tsy-hostile-v1 §1 — 执念（精英敌对 NPC，伪装道伥后伏击）。
    Zhinian,
    /// plan-tsy-hostile-v1 §1 — 负压畸变体（环境威胁，带耗真元光环）。
    Fuya,
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
            Self::Daoxiang => "daoxiang",
            Self::Zhinian => "zhinian",
            Self::Fuya => "fuya",
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
            // 道伥承接 plan-tsy-lifecycle-v1 §4 的"不老"语义；hostile 分层不改该 balance。
            Self::Daoxiang => 1_000_000.0,
            Self::Zhinian => 180_000.0,
            Self::Fuya => 240_000.0,
        }
    }

    pub const fn uses_lifespan_aging(self) -> bool {
        !matches!(self, Self::Zombie | Self::GuardianRelic | Self::Daoxiang)
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
    pub per_zone_caps: HashMap<String, usize>,
    pub counts_by_zone: HashMap<String, usize>,
}

impl Default for NpcRegistry {
    fn default() -> Self {
        Self {
            live_npc_count: 0,
            max_npc_count: 512,
            resume_npc_count: 460,
            spawn_paused: false,
            counts_by_archetype: HashMap::new(),
            per_zone_caps: HashMap::new(),
            counts_by_zone: HashMap::new(),
        }
    }
}

impl NpcRegistry {
    pub fn refresh_from_counts(
        &mut self,
        live_npc_count: usize,
        counts_by_archetype: HashMap<NpcArchetype, usize>,
        counts_by_zone: HashMap<String, usize>,
    ) {
        self.live_npc_count = live_npc_count;
        self.counts_by_archetype = counts_by_archetype;
        self.counts_by_zone = counts_by_zone;

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

    /// 回滚已 reserve 但未实际落盘的配额。用于"先 reserve 再决定能否 spawn"
    /// 路径在早退分支未回退导致的 1-tick 暂态泄漏 —— 这一 tick 里
    /// `live_npc_count >= resume_npc_count` 会误触发 `spawn_paused=true`，
    /// 同 tick 后续 spawn 分支被误杀。
    pub fn release_spawn_batch(&mut self, count: usize) {
        if count == 0 {
            return;
        }
        self.live_npc_count = self.live_npc_count.saturating_sub(count);
        if self.live_npc_count < self.resume_npc_count {
            self.spawn_paused = false;
        }
    }

    pub fn reserve_zone_batch(&mut self, zone: &str, desired: usize) -> usize {
        if desired == 0 {
            return 0;
        }
        let zone_remaining = self
            .per_zone_caps
            .get(zone)
            .map(|cap| cap.saturating_sub(*self.counts_by_zone.get(zone).unwrap_or(&0)))
            .unwrap_or(desired);
        let desired = desired.min(zone_remaining);
        let granted = self.reserve_spawn_batch(desired);
        if granted > 0 {
            *self.counts_by_zone.entry(zone.to_string()).or_default() += granted;
        }
        granted
    }

    #[allow(dead_code)]
    pub fn release_zone_slot(&mut self, zone: &str) {
        self.release_zone_batch(zone, 1);
    }

    pub fn release_zone_batch(&mut self, zone: &str, count: usize) {
        if count == 0 {
            return;
        }
        self.release_spawn_batch(count);
        let remove = if let Some(current) = self.counts_by_zone.get_mut(zone) {
            *current = current.saturating_sub(count);
            *current == 0
        } else {
            false
        };
        if remove {
            self.counts_by_zone.remove(zone);
        }
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

/// 邻居生子（plan §3.3）：Commoner 老死后由 spawn 侧消费，在死者附近
/// 生一个年龄 0–5% max_age 的新生儿。受 `NpcRegistry` 预留预算约束。
///
/// Beast 领地繁衍（§8）复用同一通道：`archetype = Beast` + 必填
/// `territory_center` / `territory_radius`（新生幼崽要挂 Territory 组件，
/// spawn 侧据此重建）。避免 lifecycle.rs 反向依赖 territory.rs。
#[derive(Clone, Debug, Event)]
pub struct NpcReproductionRequest {
    pub archetype: NpcArchetype,
    pub position: DVec3,
    pub home_zone: String,
    pub initial_age_ticks: f64,
    /// Beast 必填；Commoner 忽略。
    pub territory_center: Option<DVec3>,
    /// Beast 必填；Commoner 忽略。
    pub territory_radius: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NpcSpawnSource {
    Startup,
    Seed,
    Reproduction,
    AgentCommand,
}

impl NpcSpawnSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Startup => "startup",
            Self::Seed => "seed",
            Self::Reproduction => "reproduction",
            Self::AgentCommand => "agent_command",
        }
    }
}

#[derive(Clone, Debug, Event)]
pub struct NpcSpawnNotice {
    pub npc_id: String,
    pub archetype: NpcArchetype,
    pub source: NpcSpawnSource,
    pub home_zone: String,
    pub position: DVec3,
    pub initial_age_ticks: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NpcDeathReason {
    NaturalAging,
    Combat,
    Despawned,
    DuoShe,
}

impl NpcDeathReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NaturalAging => "natural_aging",
            Self::Combat => "combat",
            Self::Despawned => "despawned",
            Self::DuoShe => "duo_she",
        }
    }
}

#[derive(Clone, Copy, Debug, Component)]
pub struct NpcDeathNoticeEmitted;

#[derive(Clone, Debug, Event)]
#[allow(dead_code)]
pub struct NpcDeathNotice {
    pub npc_id: String,
    pub archetype: NpcArchetype,
    pub reason: NpcDeathReason,
    pub faction_id: Option<FactionId>,
    pub life_record_snapshot: Option<String>,
    pub age_ticks: f64,
    pub max_age_ticks: f64,
}

pub fn register(app: &mut App) {
    app.insert_resource(NpcAgingConfig::default())
        .insert_resource(NpcRegistry::default())
        .add_event::<CultivationDeathTrigger>()
        .add_event::<PlayerTerminated>()
        .add_event::<NpcRetireRequest>()
        .add_event::<NpcReproductionRequest>()
        .add_event::<NpcSpawnNotice>()
        .add_event::<NpcDeathNotice>()
        .add_systems(FixedUpdate, (update_npc_registry, age_npcs))
        .add_systems(
            Update,
            (
                process_npc_retire_requests,
                handle_npc_terminated,
                emit_npc_despawn_notices.after(handle_npc_terminated),
            ),
        );
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
    npc_runtime_bundle_with_age(entity, archetype, 0.0)
}

pub fn npc_runtime_bundle_with_age(
    entity: Entity,
    archetype: NpcArchetype,
    initial_age_ticks: f64,
) -> NpcRuntimeBundle {
    let char_id = canonical_npc_id(entity);
    let mut bundle = NpcRuntimeBundle {
        archetype,
        lifespan: NpcLifespan::new(
            initial_age_ticks.max(0.0),
            archetype.default_max_age_ticks(),
        ),
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
    };
    if archetype == NpcArchetype::Commoner {
        bundle.shared_lifespan.apply_cap(LifespanCapTable::MORTAL);
    }
    sync_shared_lifespan_from_npc_age(&mut bundle.shared_lifespan, &bundle.lifespan);
    bundle
}

fn sync_shared_lifespan_from_npc_age(
    shared_lifespan: &mut LifespanComponent,
    npc_lifespan: &NpcLifespan,
) {
    let ratio = npc_lifespan.age_ratio().clamp(0.0, 1.0);
    shared_lifespan.years_lived = shared_lifespan.cap_by_realm as f64 * ratio;
}

fn update_npc_registry(mut registry: ResMut<NpcRegistry>, npcs: RegistryNpcQuery<'_, '_>) {
    let mut counts_by_archetype = HashMap::new();
    let mut counts_by_zone = HashMap::new();
    let mut live_npc_count = 0usize;

    for (archetype, lifecycle, patrol) in &npcs {
        if lifecycle.is_some_and(|lifecycle| lifecycle.state == LifecycleState::Terminated) {
            continue;
        }

        live_npc_count += 1;
        *counts_by_archetype.entry(*archetype).or_default() += 1;
        if let Some(patrol) = patrol {
            *counts_by_zone.entry(patrol.home_zone.clone()).or_default() += 1;
        }
    }

    registry.refresh_from_counts(live_npc_count, counts_by_archetype, counts_by_zone);
}

fn age_npcs(config: Res<NpcAgingConfig>, mut npcs: SharedAgingNpcQuery<'_, '_>) {
    if !config.enabled {
        return;
    }

    for (archetype, mut npc_lifespan, shared_lifespan) in &mut npcs {
        if !archetype.uses_lifespan_aging() {
            continue;
        }
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

#[allow(clippy::type_complexity)]
pub(crate) fn process_npc_retire_requests(
    mut retire_requests: EventReader<NpcRetireRequest>,
    npcs: Query<
        (
            &NpcArchetype,
            &NpcLifespan,
            Option<&Position>,
            Option<&crate::npc::patrol::NpcPatrol>,
        ),
        With<NpcMarker>,
    >,
    mut cultivation_deaths: EventWriter<CultivationDeathTrigger>,
    mut reproduction_requests: EventWriter<NpcReproductionRequest>,
) {
    for request in retire_requests.read() {
        let Ok((archetype, lifespan, position, patrol)) = npcs.get(request.entity) else {
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

        // plan §3.3 — 凡人老死即邻居生子。由 spawn 侧消费事件并通过
        // `NpcRegistry::reserve_spawn_batch` 统一占配额，避免击穿上限。
        if *archetype == NpcArchetype::Commoner {
            if let (Some(pos), Some(patrol)) = (position, patrol) {
                reproduction_requests.send(NpcReproductionRequest {
                    archetype: NpcArchetype::Commoner,
                    position: pos.get(),
                    home_zone: patrol.home_zone.clone(),
                    initial_age_ticks: 0.0,
                    territory_center: None,
                    territory_radius: None,
                });
            }
        }
    }
}

fn handle_npc_terminated(
    mut commands: Commands,
    mut terminated: EventReader<PlayerTerminated>,
    npcs: TerminatedNpcQuery<'_, '_>,
    mut notices: EventWriter<NpcDeathNotice>,
) {
    for event in terminated.read() {
        let Ok((archetype, lifespan, pending_retirement, shared_lifespan, faction, life_record)) =
            npcs.get(event.entity)
        else {
            continue;
        };

        let reason = if pending_retirement.is_some()
            || lifespan.is_expired()
            || shared_lifespan.is_some_and(|lifespan| lifespan.remaining_years() <= f64::EPSILON)
        {
            NpcDeathReason::NaturalAging
        } else {
            NpcDeathReason::Combat
        };

        notices.send(build_npc_death_notice(
            event.entity,
            *archetype,
            lifespan,
            faction,
            life_record,
            reason,
        ));

        if let Some(mut entity_commands) = commands.get_entity(event.entity) {
            entity_commands.insert((Despawned, NpcDeathNoticeEmitted));
            entity_commands.remove::<PendingRetirement>();
        }
    }
}

fn emit_npc_despawn_notices(
    mut commands: Commands,
    despawned: DespawnedNpcNoticeQuery<'_, '_>,
    mut notices: EventWriter<NpcDeathNotice>,
) {
    for (entity, archetype, lifespan, _shared_lifespan, faction, life_record, possessed) in
        &despawned
    {
        let reason = if possessed.is_some() {
            NpcDeathReason::DuoShe
        } else {
            NpcDeathReason::Despawned
        };
        notices.send(build_npc_death_notice(
            entity,
            *archetype,
            lifespan,
            faction,
            life_record,
            reason,
        ));
        if let Some(mut entity_commands) = commands.get_entity(entity) {
            entity_commands.insert(NpcDeathNoticeEmitted);
        }
    }
}

fn build_npc_death_notice(
    entity: Entity,
    archetype: NpcArchetype,
    lifespan: &NpcLifespan,
    faction: Option<&FactionMembership>,
    life_record: Option<&LifeRecord>,
    reason: NpcDeathReason,
) -> NpcDeathNotice {
    NpcDeathNotice {
        npc_id: canonical_npc_id(entity),
        archetype,
        reason,
        faction_id: faction.map(|membership| membership.faction_id),
        life_record_snapshot: life_record
            .map(|record| record.recent_summary_text(8))
            .filter(|summary| !summary.is_empty()),
        age_ticks: lifespan.age_ticks,
        max_age_ticks: lifespan.max_age_ticks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use valence::prelude::{App, Update};

    #[test]
    fn registry_hysteresis_pauses_at_cap_and_resumes_below_low_watermark() {
        let mut registry = NpcRegistry::default();

        registry.refresh_from_counts(512, HashMap::new(), HashMap::new());
        assert!(registry.spawn_paused);

        registry.refresh_from_counts(500, HashMap::new(), HashMap::new());
        assert!(
            registry.spawn_paused,
            "should remain paused until low watermark"
        );

        registry.refresh_from_counts(459, HashMap::new(), HashMap::new());
        assert!(!registry.spawn_paused, "should resume below low watermark");
    }

    #[test]
    fn reserve_spawn_batch_clamps_to_remaining_capacity() {
        let mut registry = NpcRegistry::default();
        registry.refresh_from_counts(510, HashMap::new(), HashMap::new());

        let granted = registry.reserve_spawn_batch(8);
        assert_eq!(granted, 2);
        assert_eq!(registry.live_npc_count, 512);
        assert!(registry.spawn_paused);
    }

    #[test]
    fn reserve_zone_batch_clamps_to_zone_cap_and_global_cap() {
        let mut registry = NpcRegistry {
            max_npc_count: 10,
            resume_npc_count: 8,
            ..Default::default()
        };
        registry.per_zone_caps.insert("forest".to_string(), 2);
        registry.counts_by_zone.insert("forest".to_string(), 1);
        registry.live_npc_count = 9;

        let granted = registry.reserve_zone_batch("forest", 5);

        assert_eq!(granted, 1);
        assert_eq!(registry.live_npc_count, 10);
        assert_eq!(registry.counts_by_zone["forest"], 2);
        assert!(registry.spawn_paused);
    }

    #[test]
    fn update_npc_registry_counts_home_zones() {
        let mut app = App::new();
        app.insert_resource(NpcRegistry::default());
        app.add_systems(Update, update_npc_registry);

        app.world_mut().spawn((
            NpcMarker,
            NpcArchetype::Rogue,
            crate::npc::patrol::NpcPatrol::new("forest", DVec3::ZERO),
        ));
        app.world_mut().spawn((
            NpcMarker,
            NpcArchetype::Beast,
            crate::npc::patrol::NpcPatrol::new("marsh", DVec3::ZERO),
        ));
        app.update();

        let registry = app.world().resource::<NpcRegistry>();
        assert_eq!(registry.live_npc_count, 2);
        assert_eq!(registry.counts_by_zone["forest"], 1);
        assert_eq!(registry.counts_by_zone["marsh"], 1);
    }

    #[test]
    fn process_retire_requests_emits_natural_aging_trigger() {
        let mut app = App::new();
        app.add_event::<NpcRetireRequest>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<NpcReproductionRequest>();
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

        let births = app
            .world()
            .resource::<bevy_ecs::event::Events<NpcReproductionRequest>>();
        assert_eq!(
            births.len(),
            0,
            "zombie retirement must not trigger reproduction"
        );
    }

    #[test]
    fn process_retire_requests_triggers_commoner_reproduction() {
        let mut app = App::new();
        app.add_event::<NpcRetireRequest>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<NpcReproductionRequest>();
        app.add_systems(Update, process_npc_retire_requests);

        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::Commoner,
                NpcLifespan::new(89_999.0, 90_000.0),
                Position::new([42.0, 66.0, 17.5]),
                crate::npc::patrol::NpcPatrol::new("forest", DVec3::new(42.0, 66.0, 17.5)),
            ))
            .id();

        app.world_mut().send_event(NpcRetireRequest { entity });
        app.update();

        let births = app
            .world()
            .resource::<bevy_ecs::event::Events<NpcReproductionRequest>>();
        let all: Vec<_> = births.iter_current_update_events().collect();
        assert_eq!(all.len(), 1);
        let req = all[0];
        assert_eq!(req.archetype, NpcArchetype::Commoner);
        assert_eq!(req.home_zone, "forest");
        assert_eq!(req.position, DVec3::new(42.0, 66.0, 17.5));
        assert_eq!(req.initial_age_ticks, 0.0);
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
            faction_id: None,
            life_record_snapshot: Some("少时醒灵".to_string()),
            age_ticks: 120.0,
            max_age_ticks: 100.0,
        };

        assert_eq!(notice.npc_id, "npc_1v1");
        assert_eq!(notice.archetype, NpcArchetype::Zombie);
        assert_eq!(notice.reason, NpcDeathReason::NaturalAging);
        assert_eq!(notice.reason.as_str(), "natural_aging");
        assert_eq!(notice.faction_id, None);
        assert_eq!(notice.life_record_snapshot.as_deref(), Some("少时醒灵"));
        assert_eq!(notice.age_ticks, 120.0);
        assert_eq!(notice.max_age_ticks, 100.0);
    }

    #[test]
    fn npc_runtime_bundle_with_age_syncs_shared_lifespan_source_of_truth() {
        let mut app = App::new();
        let entity = app.world_mut().spawn_empty().id();
        let bundle = npc_runtime_bundle_with_age(entity, NpcArchetype::Commoner, 45_000.0);
        assert_eq!(bundle.lifespan.age_ticks, 45_000.0);
        assert_eq!(
            bundle.shared_lifespan.cap_by_realm,
            LifespanCapTable::MORTAL
        );
        assert!((bundle.shared_lifespan.years_lived - 40.0).abs() < 1e-9);
    }

    #[test]
    fn ageing_sync_skips_non_aging_archetypes() {
        let mut app = App::new();
        app.insert_resource(NpcAgingConfig::default());
        app.add_systems(Update, age_npcs);

        let mut shared_lifespan = LifespanComponent::new(100);
        shared_lifespan.years_lived = 90.0;
        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::GuardianRelic,
                NpcLifespan::new(0.0, 200.0),
                shared_lifespan,
            ))
            .id();

        app.update();

        let lifespan = app.world().get::<NpcLifespan>(entity).unwrap();
        assert_eq!(lifespan.age_ticks, 0.0);
    }

    /// 端到端：致命 AttackIntent → resolve → DeathEvent → death_arbiter
    /// → NearDeath → near_death_tick 过 deadline → PlayerTerminated
    /// → handle_npc_terminated → `Despawned`.
    #[test]
    fn npc_full_death_chain_from_attack_to_despawned() {
        use crate::combat::components::NEAR_DEATH_WINDOW_TICKS;
        use crate::combat::events::{
            ApplyStatusEffectIntent, AttackIntent, CombatEvent, DeathEvent, FIST_REACH,
        };
        use crate::combat::lifecycle::{death_arbiter_tick, near_death_tick};
        use crate::combat::resolve::resolve_attack_intents;
        use crate::combat::CombatClock;
        use crate::cultivation::death_hooks::{
            CultivationDeathTrigger, PlayerRevived, PlayerTerminated,
        };
        use crate::persistence::{bootstrap_sqlite, PersistenceSettings};
        use std::time::{SystemTime, UNIX_EPOCH};
        use valence::prelude::{App, IntoSystemConfigs, Position, Update};

        let unique_suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "bong-npc-full-death-chain-{}-{unique_suffix}",
            std::process::id()
        ));
        let db_path = root.join("data").join("bong.db");
        let deceased_dir = root.join("library-web").join("public").join("deceased");
        bootstrap_sqlite(&db_path, "npc-full-death-chain")
            .expect("sqlite bootstrap should succeed");

        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 100 });
        app.insert_resource(PersistenceSettings::with_paths(
            &db_path,
            &deceased_dir,
            "npc-full-death-chain",
        ));
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::inventory::InventoryDurabilityChangedEvent>();
        app.add_event::<crate::network::vfx_event_emit::VfxEventRequest>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<PlayerRevived>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<NpcDeathNotice>();
        app.add_systems(
            Update,
            (
                resolve_attack_intents,
                death_arbiter_tick.after(resolve_attack_intents),
                near_death_tick.after(death_arbiter_tick),
                handle_npc_terminated.after(near_death_tick),
            ),
        );

        // 两个 NPC：attacker（满 qi）+ victim（濒死）
        let attacker = app
            .world_mut()
            .spawn((NpcMarker, Position::new([0.0, 64.0, 0.0])))
            .id();
        let mut attacker_bundle = npc_runtime_bundle(attacker, NpcArchetype::Zombie);
        attacker_bundle.cultivation.qi_current = 80.0;
        attacker_bundle.cultivation.qi_max = 100.0;
        app.world_mut().entity_mut(attacker).insert(attacker_bundle);

        let victim = app
            .world_mut()
            .spawn((NpcMarker, Position::new([1.0, 64.0, 0.0])))
            .id();
        let mut victim_bundle = npc_runtime_bundle(victim, NpcArchetype::Commoner);
        victim_bundle.wounds.health_current = 3.0;
        victim_bundle.wounds.health_max = 100.0;
        victim_bundle.cultivation.qi_current = 80.0;
        victim_bundle.cultivation.qi_max = 100.0;
        app.world_mut().entity_mut(victim).insert(victim_bundle);

        let victim_record = app
            .world()
            .get::<crate::cultivation::life_record::LifeRecord>(victim)
            .expect("death lifecycle NPC bundle should carry LifeRecord");
        assert_eq!(victim_record.character_id, canonical_npc_id(victim));

        // 一击致命。
        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(victim),
            issued_at_tick: 99,
            reach: FIST_REACH,
            qi_invest: 30.0,
            wound_kind: crate::combat::components::WoundKind::Blunt,
            source: crate::combat::events::AttackSource::Melee,
            debug_command: None,
        });

        // Tick 1: resolve 写 Wounds + DeathEvent；Tick 2: death_arbiter 消费
        // DeathEvent，转 NearDeath + 设 deadline = clock.tick + 600。
        app.update();
        app.update();

        let victim_lifecycle = app
            .world()
            .entity(victim)
            .get::<crate::combat::components::Lifecycle>()
            .expect("victim keeps Lifecycle");
        assert_eq!(
            victim_lifecycle.state,
            crate::combat::components::LifecycleState::NearDeath,
            "after first tick victim should be NearDeath"
        );
        let deadline = victim_lifecycle
            .near_death_deadline_tick
            .expect("deadline should be set on NearDeath entry");
        assert_eq!(deadline, 100 + NEAR_DEATH_WINDOW_TICKS);
        assert!(app
            .world()
            .get::<valence::prelude::Despawned>(victim)
            .is_none());

        // 推进 CombatClock 过 deadline：NPC fortune_remaining=0 → 直接 Terminated。
        app.world_mut().resource_mut::<CombatClock>().tick = deadline + 1;

        // near_death_tick 发 PlayerTerminated；下一帧 handle_npc_terminated
        // 插 Despawned + 发 NpcDeathNotice；普通战斗死亡标记为 combat。
        app.update();
        app.update();

        assert!(
            app.world()
                .get::<valence::prelude::Despawned>(victim)
                .is_some(),
            "victim should be marked Despawned after termination chain"
        );

        // attacker 存活。
        assert!(app
            .world()
            .get::<valence::prelude::Despawned>(attacker)
            .is_none());
        let attacker_life = app
            .world()
            .entity(attacker)
            .get::<crate::combat::components::Lifecycle>()
            .unwrap();
        assert_eq!(
            attacker_life.state,
            crate::combat::components::LifecycleState::Alive
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
