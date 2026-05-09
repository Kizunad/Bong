use std::collections::HashSet;
use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};
use uuid::Uuid;
use valence::prelude::{
    bevy_ecs, App, Client, Position, Query, Res, ResMut, Resource, Update, With,
};

use crate::combat::components::{Lifecycle, LifecycleState};
use crate::combat::CombatClock;
use crate::identity::{IdentityId, IdentityProfile, PlayerIdentities};
use crate::network::redis_bridge::RedisOutbound;
use crate::network::RedisBridgeResource;
use crate::persistence::{open_persistence_connection, PersistenceSettings};
use crate::schema::social::{HighRenownMilestoneEventTag, HighRenownMilestoneEventV1};
use crate::social::components::{Anonymity, ExposureLog};
use crate::world::dimension::DimensionKind;
use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};

pub const MILESTONE_THRESHOLDS: [u32; 3] = [100, 500, 1000];
const ROW_SCHEMA_VERSION: i32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HighRenownMilestoneKey {
    pub player_uuid: Uuid,
    pub identity_id: IdentityId,
    pub milestone: u32,
}

#[derive(Debug, Default, Resource)]
pub struct HighRenownMilestoneTracker {
    pub already_emitted: HashSet<HighRenownMilestoneKey>,
    hydrated: bool,
}

pub fn register(app: &mut App) {
    app.init_resource::<HighRenownMilestoneTracker>();
    app.add_systems(Update, emit_high_renown_milestone_system);
}

#[allow(clippy::type_complexity)]
pub fn emit_high_renown_milestone_system(
    mut tracker: ResMut<HighRenownMilestoneTracker>,
    clock: Option<Res<CombatClock>>,
    persistence: Option<Res<PersistenceSettings>>,
    redis: Option<Res<RedisBridgeResource>>,
    zone_registry: Option<Res<ZoneRegistry>>,
    players: Query<
        (
            &PlayerIdentities,
            &Lifecycle,
            Option<&Anonymity>,
            Option<&ExposureLog>,
            Option<&Position>,
        ),
        With<Client>,
    >,
) {
    hydrate_tracker(&mut tracker, persistence.as_deref());
    let Some(redis) = redis else {
        return;
    };
    let tick = clock.map(|clock| clock.tick).unwrap_or_default();

    for (identities, lifecycle, anonymity, exposure_log, position) in players.iter() {
        if lifecycle.state == LifecycleState::Terminated {
            continue;
        }
        let Some(active) = identities.active() else {
            continue;
        };
        let player_uuid = Uuid::new_v5(&Uuid::NAMESPACE_OID, lifecycle.character_id.as_bytes());
        let zone = resolve_zone(position, zone_registry.as_deref());
        let identity_exposed = is_identity_exposed(anonymity, exposure_log);

        for milestone in MILESTONE_THRESHOLDS {
            if active.renown.fame < milestone as i32 {
                continue;
            }
            let key = HighRenownMilestoneKey {
                player_uuid,
                identity_id: active.id,
                milestone,
            };
            if tracker.already_emitted.contains(&key) {
                continue;
            }

            if let Some(settings) = persistence.as_deref() {
                if let Err(error) =
                    persist_emitted_milestone(settings, key, lifecycle.character_id.as_str(), tick)
                {
                    tracing::warn!(
                        ?error,
                        char_id = lifecycle.character_id.as_str(),
                        identity_id = active.id.0,
                        milestone,
                        "[bong][social] failed to persist high renown milestone"
                    );
                    continue;
                }
            }
            tracker.already_emitted.insert(key);

            let payload = build_high_renown_milestone_event(
                lifecycle.character_id.as_str(),
                active,
                milestone,
                tick,
                Some(zone.clone()),
                identity_exposed,
            );
            if let Err(error) = redis
                .tx_outbound
                .send(RedisOutbound::HighRenownMilestone(payload))
            {
                tracing::warn!(
                    ?error,
                    char_id = lifecycle.character_id.as_str(),
                    identity_id = active.id.0,
                    milestone,
                    "[bong][social] failed to enqueue high renown milestone"
                );
            }
        }
    }
}

pub fn build_high_renown_milestone_event(
    char_id: &str,
    active: &IdentityProfile,
    milestone: u32,
    tick: u64,
    zone: Option<String>,
    identity_exposed: bool,
) -> HighRenownMilestoneEventV1 {
    HighRenownMilestoneEventV1 {
        v: 1,
        event: HighRenownMilestoneEventTag::HighRenownMilestone,
        player_uuid: Uuid::new_v5(&Uuid::NAMESPACE_OID, char_id.as_bytes()).to_string(),
        char_id: char_id.to_string(),
        identity_id: active.id.0,
        identity_display_name: active.display_name.clone(),
        fame: active.renown.fame,
        milestone,
        identity_exposed,
        tick,
        zone,
    }
}

pub(crate) fn load_emitted_milestones(
    settings: &PersistenceSettings,
) -> io::Result<HashSet<HighRenownMilestoneKey>> {
    let connection = open_persistence_connection(settings)?;
    load_emitted_milestones_from_connection(&connection)
}

fn load_emitted_milestones_from_connection(
    connection: &Connection,
) -> io::Result<HashSet<HighRenownMilestoneKey>> {
    let mut statement = connection
        .prepare(
            "
            SELECT player_uuid, identity_id, milestone
            FROM high_renown_milestones
            ",
        )
        .map_err(io::Error::other)?;
    let rows = statement
        .query_map([], |row| {
            let player_uuid_raw: String = row.get(0)?;
            let identity_id_raw: i64 = row.get(1)?;
            let milestone_raw: i64 = row.get(2)?;
            Ok((player_uuid_raw, identity_id_raw, milestone_raw))
        })
        .map_err(io::Error::other)?;

    let mut keys = HashSet::new();
    for row in rows {
        let (player_uuid_raw, identity_id_raw, milestone_raw) = row.map_err(io::Error::other)?;
        let player_uuid = Uuid::parse_str(player_uuid_raw.as_str())
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let identity_id = IdentityId(
            u32::try_from(identity_id_raw)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
        );
        let milestone = u32::try_from(milestone_raw)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        keys.insert(HighRenownMilestoneKey {
            player_uuid,
            identity_id,
            milestone,
        });
    }
    Ok(keys)
}

pub(crate) fn persist_emitted_milestone(
    settings: &PersistenceSettings,
    key: HighRenownMilestoneKey,
    char_id: &str,
    emitted_at_tick: u64,
) -> io::Result<()> {
    let mut connection = open_persistence_connection(settings)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    transaction
        .execute(
            "
            INSERT OR IGNORE INTO high_renown_milestones (
                player_uuid,
                char_id,
                identity_id,
                milestone,
                emitted_at_tick,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ",
            params![
                key.player_uuid.to_string(),
                char_id,
                key.identity_id.0,
                key.milestone,
                emitted_at_tick,
                ROW_SCHEMA_VERSION,
                current_unix_seconds(),
            ],
        )
        .map_err(io::Error::other)?;
    transaction.commit().map_err(io::Error::other)?;
    Ok(())
}

fn hydrate_tracker(
    tracker: &mut HighRenownMilestoneTracker,
    persistence: Option<&PersistenceSettings>,
) {
    if tracker.hydrated {
        return;
    }
    if let Some(settings) = persistence {
        match load_emitted_milestones(settings) {
            Ok(keys) => tracker.already_emitted.extend(keys),
            Err(error) => tracing::warn!(
                ?error,
                "[bong][social] failed to hydrate high renown milestone tracker"
            ),
        }
    }
    tracker.hydrated = true;
}

fn is_identity_exposed(anonymity: Option<&Anonymity>, exposure_log: Option<&ExposureLog>) -> bool {
    anonymity
        .map(|entry| entry.displayed_name.is_some() || !entry.exposed_to.is_empty())
        .unwrap_or(false)
        || exposure_log
            .map(|entry| !entry.0.is_empty())
            .unwrap_or(false)
}

fn resolve_zone(position: Option<&Position>, zone_registry: Option<&ZoneRegistry>) -> String {
    let Some(position) = position else {
        return DEFAULT_SPAWN_ZONE_NAME.to_string();
    };
    zone_registry
        .and_then(|registry| {
            registry
                .find_zone(DimensionKind::Overworld, position.get())
                .map(|zone| zone.name.clone())
        })
        .unwrap_or_else(|| DEFAULT_SPAWN_ZONE_NAME.to_string())
}

fn current_unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::{bootstrap_sqlite, PersistenceSettings};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(test_name: &str) -> PathBuf {
        let unique_suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "bong-high-renown-{test_name}-{}-{unique_suffix}",
            std::process::id()
        ))
    }

    fn test_settings(test_name: &str) -> (PersistenceSettings, PathBuf) {
        let data_dir = unique_temp_dir(test_name);
        let db_path = data_dir.join("bong.db");
        bootstrap_sqlite(&db_path, &format!("high-renown-{test_name}"))
            .expect("sqlite bootstrap should succeed");
        (
            PersistenceSettings::with_paths(
                db_path,
                data_dir.join("deceased"),
                format!("high-renown-{test_name}"),
            ),
            data_dir,
        )
    }

    #[test]
    fn build_event_uses_active_identity_display_name_and_deterministic_uuid() {
        let mut identities = PlayerIdentities::with_default("玄锋", 0);
        let active = identities.active_mut().expect("default identity");
        active.renown.fame = 1000;

        let payload = build_high_renown_milestone_event(
            "offline:kiz",
            active,
            1000,
            96_000,
            Some("blood_valley".to_string()),
            true,
        );

        assert_eq!(
            payload.event,
            HighRenownMilestoneEventTag::HighRenownMilestone
        );
        assert_eq!(payload.identity_display_name, "玄锋");
        assert_eq!(payload.fame, 1000);
        assert_eq!(payload.milestone, 1000);
        assert!(payload.identity_exposed);
        assert_eq!(payload.zone.as_deref(), Some("blood_valley"));
        assert_eq!(
            payload.player_uuid,
            Uuid::new_v5(&Uuid::NAMESPACE_OID, b"offline:kiz").to_string()
        );
    }

    #[test]
    fn persistence_round_trips_emitted_milestone_keys() {
        let (settings, data_dir) = test_settings("roundtrip");
        let key = HighRenownMilestoneKey {
            player_uuid: Uuid::new_v5(&Uuid::NAMESPACE_OID, b"offline:kiz"),
            identity_id: IdentityId(7),
            milestone: 500,
        };

        persist_emitted_milestone(&settings, key, "offline:kiz", 48_000)
            .expect("persist should succeed");
        let loaded = load_emitted_milestones(&settings).expect("load should succeed");

        assert!(loaded.contains(&key));
        let _ = std::fs::remove_dir_all(data_dir);
    }
}
