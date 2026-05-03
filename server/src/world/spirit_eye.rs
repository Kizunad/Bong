use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

use valence::prelude::{
    bevy_ecs, App, Client, DVec3, Event, EventReader, EventWriter, IntoSystemConfigs, Position,
    Query, Res, ResMut, Resource, Update, Username, With,
};

use crate::cultivation::components::{Cultivation, Realm};
use crate::cultivation::life_record::LifeRecord;
use crate::cultivation::tick::CultivationClock;
use crate::network::redis_bridge::RedisOutbound;
use crate::network::RedisBridgeResource;
use crate::player::gameplay::PendingGameplayNarrations;
use crate::schema::common::NarrationStyle;
use crate::schema::death_insight::DeathInsightPositionV1;
use crate::schema::realm_vision::{SenseEntryV1, SenseKindV1};
use crate::schema::social::{RenownTagV1, SocialRenownDeltaV1};
use crate::schema::spirit_eye::{
    DeathInsightSpiritEyeV1, SpiritEyeCoordinateNoteV1, SpiritEyeDiscoveredV1,
    SpiritEyeMigrateReasonV1, SpiritEyeMigrateV1, SpiritEyeUsedForBreakthroughV1,
};

use super::dimension::{CurrentDimension, DimensionKind};
use super::zone::{Zone, ZoneRegistry};

pub const DEFAULT_SPIRIT_EYE_RADIUS: f64 = 20.0;
pub const SPIRIT_EYE_DISCOVERY_RADIUS: f64 = 20.0;
pub const SPIRIT_EYE_PERCEPTION_RADIUS: f64 = 50.0;
pub const SPIRIT_EYE_PRIVATE_MARKER_RADIUS: f64 = 96.0;
pub const SPIRIT_EYE_PRESSURE_PER_GUYUAN: f64 = 0.10;
pub const SPIRIT_EYE_PRESSURE_MIGRATE_THRESHOLD: f64 = 1.0;
pub const SPIRIT_EYE_DAILY_PRESSURE_DECAY: f64 = 0.05;
pub const TICKS_PER_DAY: u64 = 24 * 60 * 60 * 20;
pub const SPIRIT_EYE_PERIODIC_MIGRATE_TICKS: u64 = 72 * 60 * 60 * 20;
const MIN_MIGRATION_DISTANCE: f64 = 500.0;
const PERIODIC_DRIFT_BLOCKS: f64 = 50.0;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SpiritEyeId(pub String);

#[derive(Debug, Clone, PartialEq)]
pub struct SpiritEye {
    pub id: SpiritEyeId,
    pub dimension: DimensionKind,
    pub pos: [f64; 3],
    pub radius: f64,
    pub qi_concentration: f64,
    pub discovered_by: Vec<String>,
    pub usage_pressure: f64,
    pub last_migrate_tick: u64,
    pub zone_name: Option<String>,
    pub blood_valley: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpiritEyeCandidate {
    pub dimension: DimensionKind,
    pub pos: [f64; 3],
    pub zone_name: Option<String>,
    pub qi_concentration: f64,
    pub blood_valley: bool,
    score: f64,
}

#[derive(Debug, Clone, Resource)]
pub struct SpiritEyeRegistry {
    pub eyes: Vec<SpiritEye>,
    pub candidates: Vec<SpiritEyeCandidate>,
}

#[derive(Debug, Clone, Event)]
pub struct SpiritEyeDiscoveredEvent {
    pub payload: SpiritEyeDiscoveredV1,
}

#[derive(Debug, Clone, Event)]
pub struct SpiritEyeMigrateEvent {
    pub payload: SpiritEyeMigrateV1,
}

#[derive(Debug, Clone, Event)]
pub struct SpiritEyeUsedForBreakthroughEvent {
    pub payload: SpiritEyeUsedForBreakthroughV1,
}

#[derive(Debug, Clone, Event)]
pub struct SpiritEyeCoordinateSharedEvent {
    pub character_id: String,
    pub eye_id: String,
    pub tick: u64,
}

type SpiritEyeDiscoveryItem<'a> = (
    &'a Username,
    &'a Position,
    &'a Cultivation,
    &'a LifeRecord,
    Option<&'a CurrentDimension>,
);
type SpiritEyeDiscoveryFilter = With<Client>;

impl Default for SpiritEyeRegistry {
    fn default() -> Self {
        Self::from_zones(&ZoneRegistry::fallback(), 0)
    }
}

impl SpiritEyeRegistry {
    pub fn from_zones(zones: &ZoneRegistry, blood_valley_salt: u64) -> Self {
        let mut candidates = candidates_from_zones(zones);
        if candidates.is_empty() {
            candidates.push(fallback_candidate());
        }

        candidates.sort_by(|left, right| {
            right
                .score
                .total_cmp(&left.score)
                .then_with(|| candidate_sort_key(left).cmp(&candidate_sort_key(right)))
        });

        let non_negative_zone_count = zones
            .zones
            .iter()
            .filter(|zone| zone.spirit_qi >= 0.0)
            .count()
            .max(1);
        let active_count = non_negative_zone_count.clamp(3, 8).min(candidates.len());
        let mut eyes = Vec::with_capacity(active_count);
        let mut used_candidates = HashSet::new();
        for slot in 0..active_count {
            let mut candidate_idx =
                select_initial_candidate_index(&candidates, slot, blood_valley_salt);
            if used_candidates.contains(&candidate_idx) {
                candidate_idx = (0..candidates.len())
                    .find(|idx| !used_candidates.contains(idx))
                    .unwrap_or(candidate_idx);
            }
            used_candidates.insert(candidate_idx);
            let candidate = &candidates[candidate_idx];
            let zone_slug = candidate
                .zone_name
                .as_deref()
                .map(slug)
                .unwrap_or_else(|| "wild".to_string());
            eyes.push(SpiritEye {
                id: SpiritEyeId(format!("spirit_eye:{zone_slug}:{slot}")),
                dimension: candidate.dimension,
                pos: candidate.pos,
                radius: if candidate.blood_valley {
                    DEFAULT_SPIRIT_EYE_RADIUS + 4.0
                } else {
                    DEFAULT_SPIRIT_EYE_RADIUS
                },
                qi_concentration: candidate.qi_concentration,
                discovered_by: Vec::new(),
                usage_pressure: 0.0,
                last_migrate_tick: 0,
                zone_name: candidate.zone_name.clone(),
                blood_valley: candidate.blood_valley,
            });
        }

        Self { eyes, candidates }
    }

    pub fn eye_at(&self, dimension: DimensionKind, pos: DVec3) -> Option<&SpiritEye> {
        self.eyes.iter().find(|eye| {
            eye.dimension == dimension && distance(pos_to_array(pos), eye.pos) <= eye.radius
        })
    }

    pub fn spirit_eye_qi_at(&self, dimension: DimensionKind, pos: DVec3) -> Option<f64> {
        self.eye_at(dimension, pos).map(|eye| eye.qi_concentration)
    }

    pub fn discover(
        &mut self,
        character_id: &str,
        dimension: DimensionKind,
        pos: DVec3,
        realm: Realm,
        tick: u64,
    ) -> Option<SpiritEyeDiscoveredV1> {
        let eye = self.eyes.iter_mut().find(|eye| {
            if eye.dimension != dimension || eye.discovered_by.iter().any(|id| id == character_id) {
                return false;
            }
            let d = distance(pos_to_array(pos), eye.pos);
            d <= SPIRIT_EYE_DISCOVERY_RADIUS
                || (can_perceive_spirit_eye(realm) && d <= SPIRIT_EYE_PERCEPTION_RADIUS)
        })?;

        eye.discovered_by.push(character_id.to_string());
        Some(SpiritEyeDiscoveredV1 {
            v: 1,
            eye_id: eye.id.0.clone(),
            character_id: character_id.to_string(),
            pos: eye.pos.into(),
            zone: eye.zone_name.clone(),
            qi_concentration: eye.qi_concentration,
            discovered_at_tick: tick,
        })
    }

    pub fn record_breakthrough_use_by_id(
        &mut self,
        eye_id: &SpiritEyeId,
        character_id: &str,
        realm_from: Realm,
        realm_to: Realm,
        tick: u64,
    ) -> Option<SpiritEyeUsedForBreakthroughV1> {
        let eye = self.eyes.iter_mut().find(|eye| eye.id == *eye_id)?;
        if !eye.discovered_by.iter().any(|id| id == character_id) {
            eye.discovered_by.push(character_id.to_string());
        }
        eye.usage_pressure = (eye.usage_pressure + SPIRIT_EYE_PRESSURE_PER_GUYUAN).clamp(0.0, 2.0);

        Some(SpiritEyeUsedForBreakthroughV1 {
            v: 1,
            eye_id: eye.id.0.clone(),
            character_id: character_id.to_string(),
            realm_from: format!("{realm_from:?}"),
            realm_to: format!("{realm_to:?}"),
            usage_pressure: eye.usage_pressure,
            tick,
        })
    }

    pub fn tick_migration(&mut self, tick: u64) -> Vec<SpiritEyeMigrateV1> {
        if tick > 0 && tick % TICKS_PER_DAY == 0 {
            for eye in &mut self.eyes {
                eye.usage_pressure =
                    (eye.usage_pressure - SPIRIT_EYE_DAILY_PRESSURE_DECAY).max(0.0);
            }
        }

        let candidates = self.candidates.clone();
        let mut events = Vec::new();
        for eye in &mut self.eyes {
            let overused = eye.usage_pressure >= SPIRIT_EYE_PRESSURE_MIGRATE_THRESHOLD;
            let periodic =
                tick.saturating_sub(eye.last_migrate_tick) >= SPIRIT_EYE_PERIODIC_MIGRATE_TICKS;
            if !overused && !periodic {
                continue;
            }

            let reason = if overused {
                SpiritEyeMigrateReasonV1::UsagePressure
            } else {
                SpiritEyeMigrateReasonV1::PeriodicDrift
            };
            let from = eye.pos;
            let next = if overused {
                select_far_candidate(eye, &candidates).unwrap_or_else(|| {
                    offset_position(from, MIN_MIGRATION_DISTANCE + PERIODIC_DRIFT_BLOCKS)
                })
            } else {
                offset_position(from, PERIODIC_DRIFT_BLOCKS)
            };
            eye.pos = next;
            eye.discovered_by.clear();
            eye.usage_pressure = 0.0;
            eye.last_migrate_tick = tick;

            events.push(SpiritEyeMigrateV1 {
                v: 1,
                eye_id: eye.id.0.clone(),
                from: from.into(),
                to: next.into(),
                reason,
                usage_pressure: 0.0,
                tick,
            });
        }
        events
    }

    pub fn known_spirit_eyes_for(&self, character_id: &str) -> Vec<DeathInsightSpiritEyeV1> {
        self.eyes
            .iter()
            .filter(|eye| eye.discovered_by.iter().any(|id| id == character_id))
            .map(|eye| DeathInsightSpiritEyeV1 {
                eye_id: eye.id.0.clone(),
                zone: eye.zone_name.clone(),
                pos: DeathInsightPositionV1 {
                    x: eye.pos[0],
                    y: eye.pos[1],
                    z: eye.pos[2],
                },
                qi_concentration: eye.qi_concentration,
            })
            .collect()
    }

    #[allow(dead_code)] // plan-spirit-eye-v1 P4 locks the trade-note DTO before UI consumption.
    pub fn coordinate_note_for(
        &self,
        owner_character_id: &str,
        eye_id: &str,
        discovered_at_tick: u64,
    ) -> Option<SpiritEyeCoordinateNoteV1> {
        let eye = self.eyes.iter().find(|eye| {
            eye.id.0 == eye_id && eye.discovered_by.iter().any(|id| id == owner_character_id)
        })?;
        Some(SpiritEyeCoordinateNoteV1 {
            v: 1,
            eye_id: eye.id.0.clone(),
            owner_character_id: owner_character_id.to_string(),
            pos: eye.pos.into(),
            zone: eye.zone_name.clone(),
            qi_concentration: eye.qi_concentration,
            discovered_at_tick,
        })
    }

    pub fn private_marker_entries(
        &self,
        character_id: &str,
        observer_dimension: DimensionKind,
        observer_pos: [f64; 3],
    ) -> Vec<SenseEntryV1> {
        self.eyes
            .iter()
            .filter(|eye| eye.dimension == observer_dimension)
            .filter(|eye| eye.discovered_by.iter().any(|id| id == character_id))
            .filter_map(|eye| {
                let d = distance(observer_pos, eye.pos);
                (d <= SPIRIT_EYE_PRIVATE_MARKER_RADIUS).then(|| SenseEntryV1 {
                    kind: SenseKindV1::SpiritEye,
                    x: eye.pos[0],
                    y: eye.pos[1],
                    z: eye.pos[2],
                    intensity: (1.0 - d / SPIRIT_EYE_PRIVATE_MARKER_RADIUS).clamp(0.35, 1.0),
                })
            })
            .collect()
    }
}

pub fn xueguai_eye_unstable_init(candidate_count: usize, salt: u64) -> usize {
    if candidate_count == 0 {
        return 0;
    }
    (mix64(salt ^ 0x5800_6a11) as usize) % candidate_count
}

pub fn register(app: &mut App) {
    let registry = SpiritEyeRegistry::from_zones(&ZoneRegistry::load(), startup_salt());
    app.insert_resource(registry);
    app.add_event::<SpiritEyeDiscoveredEvent>();
    app.add_event::<SpiritEyeMigrateEvent>();
    app.add_event::<SpiritEyeUsedForBreakthroughEvent>();
    app.add_event::<SpiritEyeCoordinateSharedEvent>();
    app.add_systems(
        Update,
        (
            spirit_eye_discovery_tick,
            spirit_eye_migration_tick,
            publish_spirit_eye_events.after(spirit_eye_migration_tick),
            spirit_eye_coordinate_share_renown_stub,
        ),
    );
}

pub fn spirit_eye_discovery_tick(
    clock: Res<CultivationClock>,
    mut registry: ResMut<SpiritEyeRegistry>,
    mut pending_narrations: Option<ResMut<PendingGameplayNarrations>>,
    mut discovered_events: EventWriter<SpiritEyeDiscoveredEvent>,
    players: Query<SpiritEyeDiscoveryItem<'_>, SpiritEyeDiscoveryFilter>,
) {
    for (username, position, cultivation, life_record, current_dimension) in &players {
        let dimension = current_dimension
            .map(|dimension| dimension.0)
            .unwrap_or_default();
        let Some(payload) = registry.discover(
            life_record.character_id.as_str(),
            dimension,
            position.get(),
            cultivation.realm,
            clock.tick,
        ) else {
            continue;
        };
        if let Some(narrations) = pending_narrations.as_deref_mut() {
            narrations.push_player(
                username.0.as_str(),
                "此间有什么凝聚着，说不清的稠。",
                NarrationStyle::Perception,
            );
        }
        discovered_events.send(SpiritEyeDiscoveredEvent { payload });
    }
}

pub fn spirit_eye_migration_tick(
    clock: Res<CultivationClock>,
    mut registry: ResMut<SpiritEyeRegistry>,
    mut pending_narrations: Option<ResMut<PendingGameplayNarrations>>,
    mut migrate_events: EventWriter<SpiritEyeMigrateEvent>,
) {
    for payload in registry.tick_migration(clock.tick) {
        if let Some(narrations) = pending_narrations.as_deref_mut() {
            narrations.push_broadcast("东方某处天地凝气散了几分。", NarrationStyle::Narration);
        }
        migrate_events.send(SpiritEyeMigrateEvent { payload });
    }
}

pub fn publish_spirit_eye_events(
    redis: Option<Res<RedisBridgeResource>>,
    mut discovered: EventReader<SpiritEyeDiscoveredEvent>,
    mut migrated: EventReader<SpiritEyeMigrateEvent>,
    mut used: EventReader<SpiritEyeUsedForBreakthroughEvent>,
) {
    let Some(redis) = redis else {
        return;
    };

    for event in discovered.read() {
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::SpiritEyeDiscovered(event.payload.clone()));
    }
    for event in migrated.read() {
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::SpiritEyeMigrate(event.payload.clone()));
    }
    for event in used.read() {
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::SpiritEyeUsedForBreakthrough(
                event.payload.clone(),
            ));
    }
}

pub fn spirit_eye_coordinate_share_renown_stub(
    redis: Option<Res<RedisBridgeResource>>,
    mut shares: EventReader<SpiritEyeCoordinateSharedEvent>,
) {
    let Some(redis) = redis else {
        return;
    };

    for event in shares.read() {
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::SocialRenownDelta(SocialRenownDeltaV1 {
                v: 1,
                char_id: event.character_id.clone(),
                fame_delta: 1,
                notoriety_delta: 0,
                tags_added: vec![RenownTagV1 {
                    tag: "信使/向导".to_string(),
                    weight: 1.0,
                    last_seen_tick: event.tick,
                    permanent: false,
                }],
                tick: event.tick,
                reason: format!("spirit_eye_coordinate_shared:{}", event.eye_id),
            }));
    }
}

fn candidates_from_zones(zones: &ZoneRegistry) -> Vec<SpiritEyeCandidate> {
    zones
        .zones
        .iter()
        .filter(|zone| zone.spirit_qi >= 0.0)
        .flat_map(candidates_from_zone)
        .collect()
}

fn candidates_from_zone(zone: &Zone) -> Vec<SpiritEyeCandidate> {
    let center = zone.center();
    let patrol = zone.patrol_target(0);
    let offset = zone.clamp_position(center + DVec3::new(73.0, 0.0, -41.0));
    [patrol, center, offset]
        .into_iter()
        .enumerate()
        .map(|(idx, pos)| {
            let blood_valley = is_blood_valley_zone(zone);
            let altitude_score = ((pos.y - 80.0) / 120.0).clamp(0.0, 1.0);
            let zone_qi = zone.spirit_qi.clamp(0.0, 1.0);
            let score = zone_qi * 0.60 + altitude_score * 0.25 + (idx as f64) * 0.01;
            SpiritEyeCandidate {
                dimension: zone.dimension,
                pos: pos_to_array(pos),
                zone_name: Some(zone.name.clone()),
                qi_concentration: if blood_valley {
                    1.20
                } else {
                    (zone_qi + 0.20).clamp(0.85, 1.10)
                },
                blood_valley,
                score: if blood_valley { score + 0.20 } else { score },
            }
        })
        .collect()
}

fn fallback_candidate() -> SpiritEyeCandidate {
    SpiritEyeCandidate {
        dimension: DimensionKind::Overworld,
        pos: [14.0, 66.0, 14.0],
        zone_name: Some("spawn".to_string()),
        qi_concentration: 1.0,
        blood_valley: false,
        score: 1.0,
    }
}

fn select_initial_candidate_index(
    candidates: &[SpiritEyeCandidate],
    slot: usize,
    blood_valley_salt: u64,
) -> usize {
    let blood_indices: Vec<_> = candidates
        .iter()
        .enumerate()
        .filter_map(|(idx, candidate)| candidate.blood_valley.then_some(idx))
        .collect();
    if slot == 0 && !blood_indices.is_empty() {
        return blood_indices[xueguai_eye_unstable_init(blood_indices.len(), blood_valley_salt)];
    }
    slot % candidates.len()
}

fn select_far_candidate(eye: &SpiritEye, candidates: &[SpiritEyeCandidate]) -> Option<[f64; 3]> {
    candidates
        .iter()
        .filter(|candidate| candidate.dimension == eye.dimension)
        .filter(|candidate| distance(eye.pos, candidate.pos) >= MIN_MIGRATION_DISTANCE)
        .max_by(|left, right| left.score.total_cmp(&right.score))
        .map(|candidate| candidate.pos)
}

fn can_perceive_spirit_eye(realm: Realm) -> bool {
    matches!(
        realm,
        Realm::Induce | Realm::Condense | Realm::Solidify | Realm::Spirit | Realm::Void
    )
}

fn is_blood_valley_zone(zone: &Zone) -> bool {
    let name = zone.name.to_ascii_lowercase();
    name.contains("blood") || name.contains("xue") || name.contains("rift")
}

fn offset_position(pos: [f64; 3], blocks: f64) -> [f64; 3] {
    [pos[0] + blocks, pos[1], pos[2] - blocks * 0.5]
}

fn candidate_sort_key(candidate: &SpiritEyeCandidate) -> String {
    format!(
        "{}:{}:{}",
        candidate.zone_name.as_deref().unwrap_or(""),
        candidate.pos[0],
        candidate.pos[2]
    )
}

fn pos_to_array(pos: DVec3) -> [f64; 3] {
    [pos.x, pos.y, pos.z]
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn slug(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn startup_salt() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn mix64(mut x: u64) -> u64 {
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51afd7ed558ccd);
    x ^= x >> 33;
    x = x.wrapping_mul(0xc4ceb9fe1a85ec53);
    x ^ (x >> 33)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zone(name: &str, spirit_qi: f64, min_x: f64) -> Zone {
        Zone {
            name: name.to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (
                DVec3::new(min_x, 64.0, 0.0),
                DVec3::new(min_x + 128.0, 220.0, 128.0),
            ),
            spirit_qi,
            danger_level: 1,
            active_events: Vec::new(),
            patrol_anchors: vec![DVec3::new(min_x + 16.0, 90.0, 16.0)],
            blocked_tiles: Vec::new(),
        }
    }

    #[test]
    fn registry_initializes_active_eyes_from_non_negative_zone_count() {
        let zones = ZoneRegistry {
            zones: vec![
                zone("qingyun_peaks", 0.9, 0.0),
                zone("spring_marsh", 0.8, 700.0),
                zone("north_wastes", -0.2, 1400.0),
                zone("blood_valley", 0.4, 2100.0),
            ],
        };

        let registry = SpiritEyeRegistry::from_zones(&zones, 1);

        assert_eq!(registry.eyes.len(), 3);
        assert!(registry.candidates.len() >= 9);
        assert!(registry.eyes.iter().any(|eye| eye.blood_valley));
    }

    #[test]
    fn discovery_is_private_and_idempotent() {
        let zones = ZoneRegistry {
            zones: vec![zone("spawn", 0.9, 0.0)],
        };
        let mut registry = SpiritEyeRegistry::from_zones(&zones, 0);
        let eye_pos = registry.eyes[0].pos;

        let first = registry.discover(
            "char:alice",
            DimensionKind::Overworld,
            DVec3::new(eye_pos[0], eye_pos[1], eye_pos[2]),
            Realm::Awaken,
            7,
        );
        let second = registry.discover(
            "char:alice",
            DimensionKind::Overworld,
            DVec3::new(eye_pos[0], eye_pos[1], eye_pos[2]),
            Realm::Awaken,
            8,
        );

        assert!(first.is_some());
        assert!(second.is_none());
        assert_eq!(registry.eyes[0].discovered_by, vec!["char:alice"]);
    }

    #[test]
    fn perception_can_discover_beyond_touch_radius() {
        let zones = ZoneRegistry {
            zones: vec![zone("spawn", 0.9, 0.0)],
        };
        let mut registry = SpiritEyeRegistry::from_zones(&zones, 0);
        let eye_pos = registry.eyes[0].pos;

        let payload = registry.discover(
            "char:alice",
            DimensionKind::Overworld,
            DVec3::new(eye_pos[0] + 45.0, eye_pos[1], eye_pos[2]),
            Realm::Induce,
            10,
        );

        assert!(payload.is_some());
    }

    #[test]
    fn breakthrough_use_adds_pressure_and_known_death_insight_entry() {
        let zones = ZoneRegistry {
            zones: vec![zone("spawn", 0.9, 0.0)],
        };
        let mut registry = SpiritEyeRegistry::from_zones(&zones, 0);
        let eye_id = registry.eyes[0].id.clone();

        let used = registry
            .record_breakthrough_use_by_id(
                &eye_id,
                "char:alice",
                Realm::Condense,
                Realm::Solidify,
                11,
            )
            .expect("spirit eye use should be recorded");

        assert_eq!(used.usage_pressure, SPIRIT_EYE_PRESSURE_PER_GUYUAN);
        assert_eq!(registry.known_spirit_eyes_for("char:alice").len(), 1);
    }

    #[test]
    fn pressure_threshold_migrates_and_clears_discovered_by() {
        let zones = ZoneRegistry {
            zones: vec![zone("spawn", 0.9, 0.0), zone("spring_marsh", 0.8, 800.0)],
        };
        let mut registry = SpiritEyeRegistry::from_zones(&zones, 0);
        registry.eyes[0]
            .discovered_by
            .push("char:alice".to_string());
        registry.eyes[0].usage_pressure = SPIRIT_EYE_PRESSURE_MIGRATE_THRESHOLD;
        let old_pos = registry.eyes[0].pos;

        let events = registry.tick_migration(99);

        assert_eq!(events.len(), 1);
        assert_ne!(registry.eyes[0].pos, old_pos);
        assert!(registry.eyes[0].discovered_by.is_empty());
        assert_eq!(events[0].reason, SpiritEyeMigrateReasonV1::UsagePressure);
    }

    #[test]
    fn private_markers_do_not_cross_dimensions() {
        let zones = ZoneRegistry {
            zones: vec![zone("spawn", 0.9, 0.0)],
        };
        let mut registry = SpiritEyeRegistry::from_zones(&zones, 0);
        let eye_pos = registry.eyes[0].pos;
        registry.eyes[0]
            .discovered_by
            .push("char:alice".to_string());

        assert_eq!(
            registry
                .private_marker_entries("char:alice", DimensionKind::Overworld, eye_pos)
                .len(),
            1
        );
        assert!(registry
            .private_marker_entries("char:alice", DimensionKind::Tsy, eye_pos)
            .is_empty());
    }
}
