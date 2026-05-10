//! NPC dormant data plane.
//!
//! v1 keeps a deliberately small two-state model: live ECS entities stay
//! hydrated, far NPCs move into this resource and are advanced in batches.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, App, DVec3, Event, EventWriter, Res, ResMut, Resource, Startup, Update,
};

use crate::cultivation::breakthrough::{
    breakthrough_qi_cost, next_realm, try_breakthrough, BreakthroughError, BreakthroughSuccess,
    RollSource, XorshiftRoll, MIN_ZONE_QI_TO_BREAKTHROUGH, MIN_ZONE_QI_TO_GUYUAN,
};
use crate::cultivation::components::{
    Contamination, Cultivation, MeridianId, MeridianSystem, Realm,
};
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::cultivation::lifespan::{
    DeathRegistry, LifespanCapTable, LifespanComponent, LifespanExtensionLedger,
};
use crate::cultivation::meridian::severed::MeridianSeveredPermanent;
use crate::npc::faction::FactionMembership;
use crate::npc::lifecycle::{NpcArchetype, NpcDeathNotice, NpcDeathReason, NpcLifespan};
use crate::npc::loot::default_loot_for_archetype;
use crate::npc::loot::NpcLootTable;
use crate::npc::movement::GameTick;
use crate::npc::spawn::{classify_zones_by_qi, initial_age_for_index, seed_position_for_zone};
use crate::qi_physics::{
    constants::QI_ZONE_UNIT_CAPACITY, qi_release_to_zone, regen_from_zone, QiAccountId, QiTransfer,
    QiTransferReason, WorldQiAccount,
};
use crate::schema::cultivation::realm_to_string;
use crate::social::components::CharId;
use crate::world::dimension::DimensionKind;
use crate::world::zone::ZoneRegistry;

pub const NPC_DORMANT_REDIS_KEY: &str = "bong:npc/dormant";
const REDIS_URL_ENV_KEY: &str = "REDIS_URL";
const DEFAULT_REDIS_URL: &str = "redis://127.0.0.1:6379";
pub const HYDRATE_RADIUS_BLOCKS: f64 = 64.0;
pub const DEHYDRATE_RADIUS_BLOCKS: f64 = 256.0;
pub const DORMANT_ZONE_ABSORPTION_RADIUS_BLOCKS: f64 = 64.0;

#[derive(Clone, Debug, Resource)]
pub struct NpcVirtualizationConfig {
    pub hydrate_radius_blocks: f64,
    pub dehydrate_radius_blocks: f64,
    pub transition_interval_ticks: u32,
    pub dormant_tick_interval_ticks: u32,
    pub dormant_aging_rate_multiplier: f64,
    pub max_hydrated_count: usize,
    pub max_dormant_count: usize,
    /// Test and batch-run escape hatch. Runtime keeps no-player worlds hydrated
    /// until seed paths can create dormant NPCs directly.
    pub dehydrate_without_players: bool,
}

impl Default for NpcVirtualizationConfig {
    fn default() -> Self {
        Self {
            hydrate_radius_blocks: HYDRATE_RADIUS_BLOCKS,
            dehydrate_radius_blocks: DEHYDRATE_RADIUS_BLOCKS,
            transition_interval_ticks: 20,
            dormant_tick_interval_ticks: 20 * 60,
            dormant_aging_rate_multiplier: 0.3,
            max_hydrated_count: 200,
            max_dormant_count: 5000,
            dehydrate_without_players: false,
        }
    }
}

#[derive(Clone, Debug, Resource)]
pub struct DormantRoguePopulationSeedConfig {
    pub target_count: u32,
    pub resource_fraction: f32,
    pub resource_spirit_qi_threshold: f64,
    pub max_initial_age_ratio: f64,
}

impl Default for DormantRoguePopulationSeedConfig {
    fn default() -> Self {
        let target_count = std::env::var("BONG_DORMANT_ROGUE_SEED_COUNT")
            .ok()
            .and_then(|raw| raw.parse::<u32>().ok())
            .unwrap_or(1000);
        Self {
            target_count,
            resource_fraction: 0.8,
            resource_spirit_qi_threshold: 0.4,
            max_initial_age_ratio: 0.8,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DormantPatrolSnapshot {
    pub home_zone: String,
    pub anchor_index: usize,
    pub current_target: [f64; 3],
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DormantGuardianRelicSnapshot {
    pub relic_id: String,
    pub alarm_center: [f64; 3],
    pub alarm_radius: f64,
    pub trial_template_id: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub last_offered_tick: Option<u32>,
    pub offer_cooldown_ticks: u32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DormantZhinianPhase {
    Masquerade,
    Aggressive,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DormantFuyaAuraSnapshot {
    pub radius_blocks: f32,
    pub drain_boost_multiplier: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DormantDaoxiangOriginSnapshot {
    pub from_family: String,
    pub from_corpse_death_cause: String,
    pub activated_at_tick: u64,
    pub inherited_drops: Vec<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DormantTsyHostileSnapshot {
    pub family_id: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub zhinian_phase: Option<DormantZhinianPhase>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub zhinian_phase_entered_at_tick: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub fuya_aura: Option<DormantFuyaAuraSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub daoxiang_origin: Option<DormantDaoxiangOriginSnapshot>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DormantBehaviorIntent {
    Wander { drift_radius: f64 },
    PatrolToward { target: [f64; 3] },
    FleeFrom { source: [f64; 3], until_tick: u64 },
    Cultivate { zone: String },
    Retire { destination: [f64; 3] },
}

impl DormantBehaviorIntent {
    pub fn for_archetype(archetype: NpcArchetype, patrol: Option<&DormantPatrolSnapshot>) -> Self {
        match archetype {
            NpcArchetype::Rogue | NpcArchetype::Disciple => patrol
                .map(|patrol| Self::Cultivate {
                    zone: patrol.home_zone.clone(),
                })
                .unwrap_or(Self::Wander {
                    drift_radius: 120.0,
                }),
            NpcArchetype::Beast | NpcArchetype::GuardianRelic => patrol
                .map(|patrol| Self::PatrolToward {
                    target: patrol.current_target,
                })
                .unwrap_or(Self::Wander { drift_radius: 80.0 }),
            _ => Self::Wander {
                drift_radius: 120.0,
            },
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NpcDormantSnapshot {
    pub char_id: CharId,
    pub archetype: NpcArchetype,
    pub dimension: DimensionKind,
    pub zone_name: String,
    pub position: [f64; 3],
    pub cultivation: Cultivation,
    pub meridian_system: MeridianSystem,
    pub meridian_severed: MeridianSeveredPermanent,
    pub contamination: Contamination,
    pub lifespan: NpcLifespan,
    pub shared_lifespan: LifespanComponent,
    pub lifespan_extension_ledger: LifespanExtensionLedger,
    pub death_registry: DeathRegistry,
    pub life_record: LifeRecord,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub faction: Option<FactionMembership>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub patrol: Option<DormantPatrolSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub loot_table: Option<NpcLootTable>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub guardian_relic: Option<DormantGuardianRelicSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tsy_hostile: Option<DormantTsyHostileSnapshot>,
    pub intent: DormantBehaviorIntent,
    pub dormant_since_tick: u64,
    pub last_dormant_tick_processed: u64,
    pub initial_qi: f64,
    pub qi_ledger_net: f64,
}

impl NpcDormantSnapshot {
    pub fn position_vec(&self) -> DVec3 {
        dvec3_from_array(self.position)
    }

    pub fn set_position_vec(&mut self, pos: DVec3) {
        self.position = vec3_to_array(pos);
    }

    pub fn realm_label(&self) -> String {
        realm_to_string(self.cultivation.realm).to_string()
    }

    pub fn faction_id_label(&self) -> Option<crate::npc::faction::FactionId> {
        self.faction
            .as_ref()
            .map(|membership| membership.faction_id)
    }
}

#[derive(Clone, Debug, Default, Resource, Serialize, Deserialize)]
pub struct NpcDormantStore {
    pub snapshots: HashMap<CharId, NpcDormantSnapshot>,
    pub by_archetype: HashMap<NpcArchetype, Vec<CharId>>,
    pub by_zone: HashMap<String, Vec<CharId>>,
    #[serde(skip, default)]
    restore_failed: bool,
}

impl NpcDormantStore {
    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    pub fn mark_restore_failed(&mut self) {
        self.restore_failed = true;
    }

    pub fn restore_failed(&self) -> bool {
        self.restore_failed
    }

    pub fn insert(&mut self, snapshot: NpcDormantSnapshot) -> Option<NpcDormantSnapshot> {
        let previous = self.snapshots.insert(snapshot.char_id.clone(), snapshot);
        self.rebuild_indexes();
        previous
    }

    pub fn remove(&mut self, char_id: &str) -> Option<NpcDormantSnapshot> {
        let removed = self.snapshots.remove(char_id);
        if removed.is_some() {
            self.rebuild_indexes();
        }
        removed
    }

    pub fn contains(&self, char_id: &str) -> bool {
        self.snapshots.contains_key(char_id)
    }

    #[cfg(test)]
    pub fn ids_by_archetype(&self, archetype: NpcArchetype) -> &[CharId] {
        self.by_archetype
            .get(&archetype)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    #[cfg(test)]
    pub fn ids_by_zone(&self, zone_name: &str) -> &[CharId] {
        self.by_zone
            .get(zone_name)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn sorted_snapshots(&self) -> Vec<&NpcDormantSnapshot> {
        let mut values = self.snapshots.values().collect::<Vec<_>>();
        values.sort_by(|left, right| left.char_id.cmp(&right.char_id));
        values
    }

    pub fn rebuild_indexes(&mut self) {
        self.by_archetype.clear();
        self.by_zone.clear();
        for snapshot in self.snapshots.values() {
            self.by_archetype
                .entry(snapshot.archetype)
                .or_default()
                .push(snapshot.char_id.clone());
            self.by_zone
                .entry(snapshot.zone_name.clone())
                .or_default()
                .push(snapshot.char_id.clone());
        }
        for ids in self.by_archetype.values_mut() {
            ids.sort();
        }
        for ids in self.by_zone.values_mut() {
            ids.sort();
        }
    }

    pub fn to_redis_hash_payloads(&self) -> Result<Vec<(String, String)>, serde_json::Error> {
        self.sorted_snapshots()
            .into_iter()
            .map(|snapshot| {
                serde_json::to_string(snapshot).map(|payload| (snapshot.char_id.clone(), payload))
            })
            .collect()
    }
}

#[derive(Clone, Debug, Event, PartialEq, Eq)]
pub struct DormantSeveredAt {
    pub char_id: CharId,
    pub meridian_id: crate::cultivation::components::MeridianId,
}

pub fn register(app: &mut App) {
    tracing::info!("[bong][npc] registering dormant NPC store and batch tick");
    app.init_resource::<NpcDormantStore>()
        .insert_resource(NpcVirtualizationConfig::default())
        .insert_resource(DormantRoguePopulationSeedConfig::default())
        .add_event::<DormantSeveredAt>()
        .add_systems(Startup, load_dormant_store_from_redis_system)
        .add_systems(
            Update,
            (
                seed_initial_dormant_population_on_startup,
                dormant_global_tick_system,
            ),
        );
}

fn load_dormant_store_from_redis_system(mut store: ResMut<NpcDormantStore>) {
    if !store.is_empty() {
        return;
    }
    match load_dormant_snapshots_from_redis(&mut store) {
        Ok(0) => {}
        Ok(count) => {
            tracing::info!("[bong][npc] loaded {count} dormant NPC snapshot(s) from Redis HASH")
        }
        Err(error) => {
            tracing::warn!("[bong][npc] failed dormant Redis HASH restore: {error}");
            store.mark_restore_failed();
        }
    }
}

fn load_dormant_snapshots_from_redis(store: &mut NpcDormantStore) -> Result<usize, String> {
    let client = redis::Client::open(dormant_redis_url_from_env()).map_err(|error| {
        format!("failed to open Redis client for {NPC_DORMANT_REDIS_KEY}: {error}")
    })?;
    let mut connection = client
        .get_connection()
        .map_err(|error| format!("failed to connect Redis for {NPC_DORMANT_REDIS_KEY}: {error}"))?;
    let entries: HashMap<String, String> = redis::cmd("HGETALL")
        .arg(NPC_DORMANT_REDIS_KEY)
        .query(&mut connection)
        .map_err(|error| format!("failed to HGETALL {NPC_DORMANT_REDIS_KEY}: {error}"))?;
    load_dormant_snapshots_from_hash_entries(store, entries)
}

fn load_dormant_snapshots_from_hash_entries(
    store: &mut NpcDormantStore,
    entries: HashMap<String, String>,
) -> Result<usize, String> {
    if entries.is_empty() {
        return Ok(0);
    }
    let mut skipped = 0usize;
    for (char_id, payload) in entries {
        match serde_json::from_str::<NpcDormantSnapshot>(&payload) {
            Ok(snapshot) => {
                store.snapshots.insert(snapshot.char_id.clone(), snapshot);
            }
            Err(error) => {
                skipped += 1;
                tracing::warn!("[bong][npc] skipped invalid dormant snapshot `{char_id}`: {error}");
            }
        }
    }
    store.rebuild_indexes();
    if skipped > 0 && store.is_empty() {
        return Err(format!(
            "all {skipped} dormant Redis snapshot entries were invalid"
        ));
    }
    Ok(store.len())
}

fn dormant_redis_url_from_env() -> String {
    std::env::var(REDIS_URL_ENV_KEY)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_REDIS_URL.to_string())
}

pub fn current_tick(game_tick: Option<&GameTick>) -> u64 {
    game_tick.map(|tick| u64::from(tick.0)).unwrap_or_default()
}

pub fn should_run_interval(tick: u64, interval: u32) -> bool {
    let interval = interval.max(1) as u64;
    tick == 0 || tick.is_multiple_of(interval)
}

pub fn vec3_to_array(pos: DVec3) -> [f64; 3] {
    [pos.x, pos.y, pos.z]
}

pub fn dvec3_from_array(pos: [f64; 3]) -> DVec3 {
    DVec3::new(pos[0], pos[1], pos[2])
}

pub fn planar_distance(left: DVec3, right: DVec3) -> f64 {
    let dx = left.x - right.x;
    let dz = left.z - right.z;
    (dx * dx + dz * dz).sqrt()
}

fn dormant_global_tick_system(
    game_tick: Option<Res<GameTick>>,
    config: Res<NpcVirtualizationConfig>,
    mut store: ResMut<NpcDormantStore>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut ledger: Option<ResMut<WorldQiAccount>>,
    mut death_notices: EventWriter<NpcDeathNotice>,
) {
    let tick = current_tick(game_tick.as_deref());
    if !should_run_interval(tick, config.dormant_tick_interval_ticks) {
        return;
    }
    let mut ids = store.snapshots.keys().cloned().collect::<Vec<_>>();
    ids.sort();

    let mut expired = Vec::new();
    let mut indexes_dirty = false;
    for char_id in ids {
        let Some(snapshot) = store.snapshots.get_mut(&char_id) else {
            continue;
        };
        let elapsed_ticks = tick.saturating_sub(snapshot.last_dormant_tick_processed);
        snapshot.last_dormant_tick_processed = tick;
        if elapsed_ticks == 0 {
            continue;
        }
        advance_dormant_position(snapshot, elapsed_ticks, tick);
        if let Some(zones) = zones.as_deref() {
            indexes_dirty |= refresh_snapshot_zone_name(snapshot, zones);
        }
        snapshot.lifespan.age_ticks +=
            elapsed_ticks as f64 * config.dormant_aging_rate_multiplier.max(0.0);

        if let (Some(zones), Some(ledger)) = (zones.as_deref_mut(), ledger.as_deref_mut()) {
            apply_dormant_regen(snapshot, zones, ledger);
        }
        let zone_qi = zones
            .as_deref()
            .and_then(|zones| dormant_zone_qi(snapshot, zones));
        let _ = advance_dormant_breakthrough(snapshot, zone_qi, tick);

        if snapshot.lifespan.is_expired() {
            if let (Some(zones), Some(ledger)) = (zones.as_deref_mut(), ledger.as_deref_mut()) {
                release_dormant_qi_to_zone(snapshot, zones, ledger);
            }
            death_notices.send(dormant_death_notice(snapshot));
            expired.push(char_id);
        }
    }

    let removed_expired = !expired.is_empty();
    for char_id in expired {
        store.snapshots.remove(&char_id);
    }
    if removed_expired || indexes_dirty {
        store.rebuild_indexes();
    }
}

fn seed_initial_dormant_population_on_startup(
    game_tick: Option<Res<GameTick>>,
    config: Res<NpcVirtualizationConfig>,
    seed_config: Res<DormantRoguePopulationSeedConfig>,
    mut store: ResMut<NpcDormantStore>,
    zone_registry: Option<Res<ZoneRegistry>>,
    mut seeded: valence::prelude::Local<bool>,
) {
    if *seeded || seed_config.target_count == 0 {
        return;
    }
    if store.restore_failed() {
        *seeded = true;
        tracing::warn!("[bong][npc] skipped dormant seed population because Redis restore failed");
        return;
    }
    if !store.is_empty() {
        *seeded = true;
        return;
    }
    let Some(zone_registry) = zone_registry.as_deref() else {
        return;
    };
    if zone_registry.zones.is_empty() {
        return;
    }

    let capacity = config.max_dormant_count.saturating_sub(store.len());
    let target_count = seed_config.target_count.min(capacity as u32);
    if target_count == 0 {
        *seeded = true;
        return;
    }

    let (resource_zones, background_zones) = classify_zones_by_qi(
        &zone_registry.zones,
        seed_config.resource_spirit_qi_threshold,
    );
    let resource_target =
        ((target_count as f32) * seed_config.resource_fraction.clamp(0.0, 1.0)).round() as u32;
    let tick = current_tick(game_tick.as_deref());

    for index in 0..target_count {
        let zone_candidates = if index < resource_target && !resource_zones.is_empty() {
            &resource_zones
        } else if !background_zones.is_empty() {
            &background_zones
        } else {
            &resource_zones
        };
        if zone_candidates.is_empty() {
            break;
        }

        let zone = zone_candidates[(index as usize) % zone_candidates.len()];
        let snapshot =
            dormant_rogue_seed_snapshot(zone, index, tick, seed_config.max_initial_age_ratio);
        store.snapshots.insert(snapshot.char_id.clone(), snapshot);
    }
    store.rebuild_indexes();
    *seeded = true;
    tracing::info!(
        "[bong][npc] seeded {} dormant rogue NPC snapshots",
        store.len()
    );
}

fn dormant_rogue_seed_snapshot(
    zone: &crate::world::zone::Zone,
    index: u32,
    tick: u64,
    max_initial_age_ratio: f64,
) -> NpcDormantSnapshot {
    let archetype = NpcArchetype::Rogue;
    let (position, patrol_target) = seed_position_for_zone(zone, index);
    let char_id = format!("dormant:rogue:{index}");
    let cultivation = Cultivation::default();
    let mut meridian_system = MeridianSystem::default();
    meridian_system.get_mut(MeridianId::Lung).opened = true;
    let lifespan = NpcLifespan::new(
        initial_age_for_index(
            index,
            archetype.default_max_age_ticks(),
            max_initial_age_ratio,
        ),
        archetype.default_max_age_ticks(),
    );
    let patrol = Some(DormantPatrolSnapshot {
        home_zone: zone.name.clone(),
        anchor_index: index as usize,
        current_target: vec3_to_array(patrol_target),
    });
    let intent = DormantBehaviorIntent::for_archetype(archetype, patrol.as_ref());

    NpcDormantSnapshot {
        char_id: char_id.clone(),
        archetype,
        dimension: zone.dimension,
        zone_name: zone.name.clone(),
        position: vec3_to_array(position),
        cultivation: cultivation.clone(),
        meridian_system,
        meridian_severed: MeridianSeveredPermanent::default(),
        contamination: Contamination::default(),
        lifespan,
        shared_lifespan: LifespanComponent::for_realm(cultivation.realm),
        lifespan_extension_ledger: LifespanExtensionLedger::default(),
        death_registry: DeathRegistry::new(char_id.clone()),
        life_record: LifeRecord::new(char_id),
        faction: None,
        patrol,
        loot_table: Some(default_loot_for_archetype(archetype)),
        guardian_relic: None,
        tsy_hostile: None,
        intent,
        dormant_since_tick: tick,
        last_dormant_tick_processed: tick,
        initial_qi: cultivation.qi_current,
        qi_ledger_net: 0.0,
    }
}

pub fn advance_dormant_position(
    snapshot: &mut NpcDormantSnapshot,
    elapsed_ticks: u64,
    salt_tick: u64,
) {
    let seconds = elapsed_ticks as f64 / 20.0;
    let current = snapshot.position_vec();
    let next = match &snapshot.intent {
        DormantBehaviorIntent::Wander { drift_radius } => {
            let seed = deterministic_unit(snapshot.char_id.as_str(), salt_tick);
            let angle = seed * std::f64::consts::TAU;
            let step = seconds.clamp(0.0, 60.0);
            let drift_cap = drift_radius.max(0.0);
            DVec3::new(
                current.x + angle.cos() * step.min(drift_cap),
                current.y,
                current.z + angle.sin() * step.min(drift_cap),
            )
        }
        DormantBehaviorIntent::PatrolToward { target }
        | DormantBehaviorIntent::Retire {
            destination: target,
        } => move_toward(current, dvec3_from_array(*target), seconds.max(0.0)),
        DormantBehaviorIntent::FleeFrom { source, .. } => {
            let source = dvec3_from_array(*source);
            let away = current - source;
            let length = (away.x * away.x + away.z * away.z).sqrt();
            if length <= f64::EPSILON {
                current
            } else {
                let step = seconds.max(0.0);
                DVec3::new(
                    current.x + away.x / length * step,
                    current.y,
                    current.z + away.z / length * step,
                )
            }
        }
        DormantBehaviorIntent::Cultivate { .. } => current,
    };
    snapshot.set_position_vec(next);
}

fn move_toward(current: DVec3, target: DVec3, max_step: f64) -> DVec3 {
    let dx = target.x - current.x;
    let dz = target.z - current.z;
    let distance = (dx * dx + dz * dz).sqrt();
    if distance <= f64::EPSILON || max_step >= distance {
        return DVec3::new(target.x, current.y, target.z);
    }
    DVec3::new(
        current.x + dx / distance * max_step,
        current.y,
        current.z + dz / distance * max_step,
    )
}

fn deterministic_unit(char_id: &str, salt: u64) -> f64 {
    let hash = deterministic_hash(char_id, salt);
    (hash & 0xffff) as f64 / 65_535.0
}

fn deterministic_hash(char_id: &str, salt: u64) -> u64 {
    let mut hash = salt ^ 0x9E37_79B9_7F4A_7C15;
    for byte in char_id.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    }
    hash
}

pub fn apply_dormant_regen(
    snapshot: &mut NpcDormantSnapshot,
    zones: &mut ZoneRegistry,
    ledger: &mut WorldQiAccount,
) -> Option<QiTransfer> {
    let pos = snapshot.position_vec();
    let zone_name = zones
        .find_zone(snapshot.dimension, pos)
        .filter(|zone| planar_distance(zone.center(), pos) <= DORMANT_ZONE_ABSORPTION_RADIUS_BLOCKS)
        .map(|zone| zone.name.clone())?;
    let zone = zones.find_zone_mut(zone_name.as_str())?;
    if zone.spirit_qi <= 0.0 {
        return None;
    }

    let rate = snapshot.meridian_system.sum_rate();
    if rate <= 0.0 {
        return None;
    }
    let integrity_count = snapshot.meridian_system.iter().count() as f64;
    let avg_integrity = if integrity_count > 0.0 {
        snapshot
            .meridian_system
            .iter()
            .map(|meridian| meridian.integrity)
            .sum::<f64>()
            / integrity_count
    } else {
        1.0
    };
    let room = (snapshot.cultivation.qi_max - snapshot.cultivation.qi_current).max(0.0);
    let (gain, drain) = regen_from_zone(zone.spirit_qi, rate, avg_integrity, room);
    if gain <= 0.0 || drain <= 0.0 {
        return None;
    }

    let zone_account = QiAccountId::zone(zone.name.clone());
    let npc_account = QiAccountId::npc(snapshot.char_id.clone());
    ledger
        .set_balance(
            zone_account.clone(),
            zone.spirit_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY,
        )
        .ok()?;
    ledger
        .set_balance(
            npc_account.clone(),
            snapshot.cultivation.qi_current.max(0.0),
        )
        .ok()?;
    let transfer = QiTransfer::new(
        zone_account,
        npc_account.clone(),
        gain,
        QiTransferReason::CultivationRegen,
    )
    .ok()?;
    ledger.transfer(transfer.clone()).ok()?;

    snapshot.cultivation.qi_current = ledger.balance(&npc_account);
    snapshot.qi_ledger_net += gain;
    zone.spirit_qi = (zone.spirit_qi - drain).max(0.0);
    Some(transfer)
}

fn dormant_zone_qi(snapshot: &NpcDormantSnapshot, zones: &ZoneRegistry) -> Option<f64> {
    zones
        .find_zone(snapshot.dimension, snapshot.position_vec())
        .map(|zone| zone.spirit_qi)
}

fn refresh_snapshot_zone_name(snapshot: &mut NpcDormantSnapshot, zones: &ZoneRegistry) -> bool {
    let Some(zone) = zones.find_zone(snapshot.dimension, snapshot.position_vec()) else {
        return false;
    };
    if snapshot.zone_name == zone.name {
        return false;
    }
    snapshot.zone_name = zone.name.clone();
    true
}

pub fn advance_dormant_breakthrough(
    snapshot: &mut NpcDormantSnapshot,
    zone_qi: Option<f64>,
    tick: u64,
) -> Option<Result<BreakthroughSuccess, BreakthroughError>> {
    let mut roll = XorshiftRoll(deterministic_hash(&snapshot.char_id, tick));
    advance_dormant_breakthrough_with_roll(snapshot, zone_qi, tick, &mut roll)
}

fn advance_dormant_breakthrough_with_roll<R: RollSource>(
    snapshot: &mut NpcDormantSnapshot,
    zone_qi: Option<f64>,
    tick: u64,
    roll: &mut R,
) -> Option<Result<BreakthroughSuccess, BreakthroughError>> {
    let next = next_realm(snapshot.cultivation.realm)?;
    if next == Realm::Void {
        return None;
    }
    if snapshot.cultivation.qi_current < breakthrough_qi_cost(next) {
        return None;
    }
    let required_zone_qi = if next == Realm::Solidify {
        MIN_ZONE_QI_TO_GUYUAN
    } else {
        MIN_ZONE_QI_TO_BREAKTHROUGH
    };
    if zone_qi? < required_zone_qi {
        return None;
    }

    let result = try_breakthrough(
        &mut snapshot.cultivation,
        &mut snapshot.meridian_system,
        0.0,
        roll,
    );
    match result {
        Ok(success) => {
            snapshot
                .shared_lifespan
                .apply_cap(LifespanCapTable::for_realm(success.to));
            snapshot
                .life_record
                .push(BiographyEntry::BreakthroughSucceeded {
                    realm: success.to,
                    tick,
                });
            Some(Ok(success))
        }
        Err(BreakthroughError::RolledFailure { severity }) => {
            snapshot
                .life_record
                .push(BiographyEntry::BreakthroughFailed {
                    realm_target: next,
                    severity,
                    tick,
                });
            Some(Err(BreakthroughError::RolledFailure { severity }))
        }
        Err(error) => Some(Err(error)),
    }
}

pub fn release_dormant_qi_to_zone(
    snapshot: &mut NpcDormantSnapshot,
    zones: &mut ZoneRegistry,
    ledger: &mut WorldQiAccount,
) -> Option<QiTransfer> {
    let pos = snapshot.position_vec();
    let zone_name = zones
        .find_zone(snapshot.dimension, pos)
        .map(|zone| zone.name.clone())
        .or_else(|| Some(snapshot.zone_name.clone()))?;
    let zone = zones.find_zone_mut(zone_name.as_str())?;
    let amount = snapshot.cultivation.qi_current.max(0.0);
    if amount <= 0.0 {
        return None;
    }

    let npc_account = QiAccountId::npc(snapshot.char_id.clone());
    let zone_account = QiAccountId::zone(zone.name.clone());
    ledger.set_balance(npc_account.clone(), amount).ok()?;
    let zone_current = zone.spirit_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY;
    ledger
        .set_balance(zone_account.clone(), zone_current)
        .ok()?;
    let outcome = qi_release_to_zone(
        amount,
        npc_account,
        zone_account,
        zone_current,
        QI_ZONE_UNIT_CAPACITY,
    )
    .ok()?;
    let transfer = outcome.transfer?;
    ledger.transfer(transfer.clone()).ok()?;
    zone.spirit_qi = (outcome.zone_after / QI_ZONE_UNIT_CAPACITY).clamp(-1.0, 1.0);
    snapshot.cultivation.qi_current = outcome.overflow;
    snapshot.qi_ledger_net -= outcome.accepted;
    Some(transfer)
}

fn dormant_death_notice(snapshot: &NpcDormantSnapshot) -> NpcDeathNotice {
    let life_record_snapshot = {
        let summary = snapshot.life_record.recent_summary_text(8);
        if summary.is_empty() {
            None
        } else {
            Some(summary)
        }
    };
    NpcDeathNotice {
        npc_id: snapshot.char_id.clone(),
        archetype: snapshot.archetype,
        reason: NpcDeathReason::NaturalAging,
        faction_id: snapshot
            .faction
            .as_ref()
            .map(|membership| membership.faction_id),
        life_record_snapshot,
        age_ticks: snapshot.lifespan.age_ticks,
        max_age_ticks: snapshot.lifespan.max_age_ticks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::{MeridianId, Realm};
    use crate::world::dimension::DimensionKind;
    use crate::world::zone::{Zone, DEFAULT_SPAWN_ZONE_NAME};

    fn zone() -> Zone {
        Zone {
            name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (DVec3::new(0.0, 0.0, 0.0), DVec3::new(100.0, 128.0, 100.0)),
            spirit_qi: 0.8,
            danger_level: 0,
            active_events: Vec::new(),
            patrol_anchors: vec![DVec3::new(10.0, 64.0, 10.0)],
            blocked_tiles: Vec::new(),
        }
    }

    fn snapshot(char_id: &str, pos: DVec3) -> NpcDormantSnapshot {
        let cultivation = Cultivation {
            qi_current: 0.1,
            qi_max: 1.0,
            ..Default::default()
        };
        NpcDormantSnapshot {
            char_id: char_id.to_string(),
            archetype: NpcArchetype::Rogue,
            dimension: DimensionKind::Overworld,
            zone_name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            position: vec3_to_array(pos),
            cultivation: cultivation.clone(),
            meridian_system: MeridianSystem::default(),
            meridian_severed: MeridianSeveredPermanent::default(),
            contamination: Contamination::default(),
            lifespan: NpcLifespan::new(0.0, 1_000.0),
            shared_lifespan: LifespanComponent::for_realm(cultivation.realm),
            lifespan_extension_ledger: LifespanExtensionLedger::default(),
            death_registry: DeathRegistry::new(char_id),
            life_record: LifeRecord::new(char_id),
            faction: None,
            patrol: None,
            loot_table: None,
            guardian_relic: None,
            tsy_hostile: None,
            intent: DormantBehaviorIntent::Cultivate {
                zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            },
            dormant_since_tick: 0,
            last_dormant_tick_processed: 0,
            initial_qi: 0.1,
            qi_ledger_net: 0.0,
        }
    }

    struct FixedRoll(f64);

    impl RollSource for FixedRoll {
        fn roll_unit(&mut self) -> f64 {
            self.0
        }
    }

    fn open_regular_meridians(snapshot: &mut NpcDormantSnapshot, count: usize) {
        for id in MeridianId::REGULAR.into_iter().take(count) {
            let meridian = snapshot.meridian_system.get_mut(id);
            meridian.opened = true;
        }
    }

    #[test]
    fn store_indexes_by_archetype_and_zone() {
        let mut store = NpcDormantStore::default();
        store.insert(snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0)));
        store.insert(snapshot("npc_b", DVec3::new(11.0, 64.0, 10.0)));

        assert_eq!(
            store.ids_by_archetype(NpcArchetype::Rogue),
            &["npc_a".to_string(), "npc_b".to_string()]
        );
        assert_eq!(
            store.ids_by_zone(DEFAULT_SPAWN_ZONE_NAME),
            &["npc_a".to_string(), "npc_b".to_string()]
        );
    }

    #[test]
    fn dormant_regen_moves_qi_through_ledger() {
        let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
        open_regular_meridians(&mut snapshot, 1);
        let mut zones = ZoneRegistry {
            zones: vec![zone()],
        };
        let mut ledger = WorldQiAccount::default();

        let transfer = apply_dormant_regen(&mut snapshot, &mut zones, &mut ledger)
            .expect("dormant regen should emit a transfer");

        assert_eq!(transfer.from, QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME));
        assert_eq!(transfer.to, QiAccountId::npc("npc_a"));
        assert!(snapshot.cultivation.qi_current > 0.1);
        assert!(
            zones
                .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
                .unwrap()
                .spirit_qi
                < 0.8
        );
        assert!(
            (snapshot.qi_ledger_net - transfer.amount).abs() < f64::EPSILON,
            "qi_ledger_net must audit the same amount as the ledger transfer"
        );
        assert!(
            (ledger.balance(&QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME))
                - zones
                    .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
                    .unwrap()
                    .spirit_qi
                    * QI_ZONE_UNIT_CAPACITY)
                .abs()
                < 1e-9,
            "ledger zone balance must use absolute qi units matching normalized zone drain"
        );
    }

    #[test]
    fn dormant_regen_requires_open_meridian_flow() {
        let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
        let mut zones = ZoneRegistry {
            zones: vec![zone()],
        };
        let mut ledger = WorldQiAccount::default();

        assert!(apply_dormant_regen(&mut snapshot, &mut zones, &mut ledger).is_none());
        assert_eq!(snapshot.cultivation.qi_current, 0.1);
        assert_eq!(
            ledger.balance(&QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME)),
            0.0
        );
    }

    #[test]
    fn dormant_realm_label_uses_shared_schema_serializer() {
        let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
        snapshot.cultivation.realm = Realm::Condense;

        assert_eq!(snapshot.realm_label(), "Condense");
    }

    #[test]
    fn expired_dormant_npc_releases_qi_to_zone() {
        let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
        snapshot.cultivation.qi_current = 0.4;
        let mut zones = ZoneRegistry {
            zones: vec![zone()],
        };
        let mut ledger = WorldQiAccount::default();

        let transfer = release_dormant_qi_to_zone(&mut snapshot, &mut zones, &mut ledger)
            .expect("death release should emit a transfer");

        assert_eq!(transfer.from, QiAccountId::npc("npc_a"));
        assert_eq!(transfer.to, QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME));
        assert_eq!(snapshot.cultivation.qi_current, 0.0);
        assert!(
            zones
                .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
                .unwrap()
                .spirit_qi
                > 0.8
        );
        assert!(
            (ledger.balance(&QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME))
                - zones
                    .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
                    .unwrap()
                    .spirit_qi
                    * QI_ZONE_UNIT_CAPACITY)
                .abs()
                < 1e-9,
            "ledger zone balance must use the same absolute qi unit as normalized zone state"
        );
    }

    #[test]
    fn death_qi_release_preserves_zone_overflow_on_npc_account() {
        let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
        snapshot.cultivation.qi_current = 2.0;
        let mut full_zone = zone();
        full_zone.spirit_qi = 0.99;
        let mut zones = ZoneRegistry {
            zones: vec![full_zone],
        };
        let mut ledger = WorldQiAccount::default();

        let transfer = release_dormant_qi_to_zone(&mut snapshot, &mut zones, &mut ledger)
            .expect("near-full zone should still accept the available room");

        assert!((transfer.amount - 0.5).abs() < 1e-9);
        assert!((snapshot.cultivation.qi_current - 1.5).abs() < 1e-9);
        assert!(
            (ledger.balance(&QiAccountId::npc("npc_a")) - snapshot.cultivation.qi_current).abs()
                < 1e-9,
            "overflow must stay in the NPC ledger account instead of disappearing"
        );
    }

    #[test]
    fn dormant_wander_uses_absolute_tick_salt() {
        let mut snapshot = snapshot("npc_a", DVec3::ZERO);
        snapshot.intent = DormantBehaviorIntent::Wander {
            drift_radius: 10_000.0,
        };
        let start = snapshot.position_vec();

        advance_dormant_position(&mut snapshot, 1200, 1200);
        let first = snapshot.position_vec();
        advance_dormant_position(&mut snapshot, 1200, 2400);
        let second = snapshot.position_vec();

        let straight_line_second = DVec3::new(
            start.x + (first.x - start.x) * 2.0,
            start.y,
            start.z + (first.z - start.z) * 2.0,
        );
        assert!(
            planar_distance(second, straight_line_second) > 1e-6,
            "wander angle must vary across absolute ticks instead of repeating one straight-line heading"
        );
    }

    #[test]
    fn dormant_global_tick_clears_indexes_when_all_snapshots_expire() {
        let mut app = App::new();
        app.add_event::<NpcDeathNotice>();
        app.insert_resource(NpcVirtualizationConfig {
            dormant_tick_interval_ticks: 1,
            ..Default::default()
        });
        app.insert_resource(GameTick(1));
        app.insert_resource(ZoneRegistry {
            zones: vec![zone()],
        });
        app.insert_resource(WorldQiAccount::default());
        let mut expired = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
        expired.lifespan.age_ticks = expired.lifespan.max_age_ticks + 1.0;
        let mut store = NpcDormantStore::default();
        store.insert(expired);
        app.insert_resource(store);
        app.add_systems(Update, dormant_global_tick_system);

        app.update();

        let store = app.world().resource::<NpcDormantStore>();
        assert!(store.is_empty());
        assert!(store.ids_by_archetype(NpcArchetype::Rogue).is_empty());
        assert!(store.ids_by_zone(DEFAULT_SPAWN_ZONE_NAME).is_empty());
    }

    #[test]
    fn dormant_global_tick_refreshes_zone_index_after_movement() {
        let mut app = App::new();
        app.add_event::<NpcDeathNotice>();
        app.insert_resource(NpcVirtualizationConfig {
            dormant_tick_interval_ticks: 1,
            ..Default::default()
        });
        app.insert_resource(GameTick(2400));
        let second_zone = Zone {
            name: "east".to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (DVec3::new(120.0, 0.0, 0.0), DVec3::new(200.0, 128.0, 80.0)),
            spirit_qi: 0.5,
            danger_level: 0,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
        };
        app.insert_resource(ZoneRegistry {
            zones: vec![zone(), second_zone],
        });
        app.insert_resource(WorldQiAccount::default());
        let mut mover = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
        mover.intent = DormantBehaviorIntent::PatrolToward {
            target: [130.0, 64.0, 10.0],
        };
        let mut store = NpcDormantStore::default();
        store.insert(mover);
        app.insert_resource(store);
        app.add_systems(Update, dormant_global_tick_system);

        app.update();

        let store = app.world().resource::<NpcDormantStore>();
        assert_eq!(store.snapshots["npc_a"].zone_name, "east");
        assert!(store.ids_by_zone(DEFAULT_SPAWN_ZONE_NAME).is_empty());
        assert_eq!(store.ids_by_zone("east"), &["npc_a"]);
    }

    #[test]
    fn dormant_breakthrough_uses_cultivation_rules_below_duxu() {
        let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
        snapshot.cultivation.realm = Realm::Awaken;
        snapshot.cultivation.qi_current = 20.0;
        snapshot.cultivation.qi_max = 100.0;
        open_regular_meridians(&mut snapshot, 3);
        let mut roll = FixedRoll(0.0);

        let result = advance_dormant_breakthrough_with_roll(
            &mut snapshot,
            Some(MIN_ZONE_QI_TO_BREAKTHROUGH),
            1200,
            &mut roll,
        )
        .expect("eligible dormant NPC should attempt breakthrough")
        .expect("fixed low roll should pass");

        assert_eq!(result.to, Realm::Induce);
        assert_eq!(snapshot.cultivation.realm, Realm::Induce);
        assert_eq!(snapshot.cultivation.qi_current, 12.0);
        assert_eq!(
            snapshot.shared_lifespan.cap_by_realm,
            LifespanCapTable::INDUCE
        );
        assert!(snapshot.life_record.biography.iter().any(|entry| {
            matches!(
                entry,
                BiographyEntry::BreakthroughSucceeded {
                    realm: Realm::Induce,
                    tick: 1200
                }
            )
        }));
    }

    #[test]
    fn redis_payload_roundtrips_snapshot() {
        let mut store = NpcDormantStore::default();
        store.insert(snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0)));

        let payloads = store.to_redis_hash_payloads().expect("serialize");
        assert_eq!(payloads[0].0, "npc_a");
        let decoded: NpcDormantSnapshot =
            serde_json::from_str(payloads[0].1.as_str()).expect("deserialize");
        assert_eq!(decoded.char_id, "npc_a");
        assert_eq!(decoded.position, [10.0, 64.0, 10.0]);
    }

    #[test]
    fn loads_dormant_snapshots_from_redis_hash_entries() {
        let source = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
        let payload = serde_json::to_string(&source).expect("serialize dormant snapshot");
        let entries = HashMap::from([(source.char_id.clone(), payload)]);
        let mut store = NpcDormantStore::default();

        let count = load_dormant_snapshots_from_hash_entries(&mut store, entries).expect("load");

        assert_eq!(count, 1);
        assert!(store.contains("npc_a"));
        assert_eq!(store.ids_by_zone(DEFAULT_SPAWN_ZONE_NAME), &["npc_a"]);
    }

    #[test]
    fn redis_hash_restore_skips_bad_entries_without_losing_good_snapshots() {
        let source = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
        let payload = serde_json::to_string(&source).expect("serialize dormant snapshot");
        let entries = HashMap::from([
            (source.char_id.clone(), payload),
            ("npc_bad".to_string(), "{not-json".to_string()),
        ]);
        let mut store = NpcDormantStore::default();

        let count = load_dormant_snapshots_from_hash_entries(&mut store, entries).expect("load");

        assert_eq!(count, 1);
        assert!(store.contains("npc_a"));
        assert!(!store.contains("npc_bad"));
    }

    #[test]
    fn redis_hash_restore_fails_when_every_entry_is_invalid() {
        let entries = HashMap::from([("npc_bad".to_string(), "{not-json".to_string())]);
        let mut store = NpcDormantStore::default();

        let error = load_dormant_snapshots_from_hash_entries(&mut store, entries)
            .expect_err("all invalid entries should fail restore");

        assert!(error.contains("all 1 dormant Redis snapshot"));
        assert!(store.is_empty());
    }
}
