//! 死亡对外契约（plan §4）— 修炼侧只 emit 致死触发，生死判定由战斗 plan 收口。
//!
//! 另外提供 `PlayerRevived` 监听：战斗 plan 完成重生后发事件，本 plan
//! 应用境界-1、qi=0、composure=0.3、contam 清空、LIFO 关脉等惩罚。
//! `PlayerTerminated` 也有监听 hook，停止该实体的所有修炼 tick（通过
//! 移除 Cultivation Component 实现）。

use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, Commands, Entity, Event, EventReader, EventWriter, Events, Position, Query, Res,
    ResMut,
};

use super::color::PracticeLog;
use super::components::{Contamination, Cultivation, MeridianSystem, QiColor, Realm};
use super::life_record::{BiographyEntry, LifeRecord};
use super::qi_zero_decay::{close_meridian, pick_closures};
use super::tick::CultivationClock;
use super::tribulation::AscensionQuotaOpened;
use crate::persistence::{release_ascension_quota_slot, PersistenceSettings};
use crate::qi_physics::constants::{QI_EPSILON, QI_ZONE_UNIT_CAPACITY};
use crate::qi_physics::{qi_release_to_zone, QiAccountId, QiTransfer};
use crate::skill::components::SkillId;
use crate::skill::events::SkillCapChanged;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::zone::ZoneRegistry;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CultivationDeathCause {
    BreakthroughBackfire,
    MeridianCollapse,
    NegativeZoneDrain,
    ContaminationOverflow,
    NaturalAging,
    SwarmQiDrain,
    VoidQuotaExceeded,
}

#[derive(Debug, Clone, Event)]
pub struct CultivationDeathTrigger {
    pub entity: Entity,
    pub cause: CultivationDeathCause,
    pub context: serde_json::Value,
}

#[derive(Debug, Clone, Event)]
pub struct PlayerRevived {
    pub entity: Entity,
}

#[derive(Debug, Clone, Event)]
pub struct PlayerTerminated {
    pub entity: Entity,
}

type TerminatedPlayerQueryItem<'a> = (
    &'a Cultivation,
    Option<&'a Position>,
    Option<&'a CurrentDimension>,
    Option<&'a LifeRecord>,
);

/// 重生响应：境界 -1、qi=0、composure=0.3、contam 清空、LIFO 关脉至对应境界。
pub fn apply_revive_penalty(
    cultivation: &mut Cultivation,
    meridians: &mut MeridianSystem,
    contam: &mut Contamination,
) -> f64 {
    let released_qi = cultivation.qi_current.max(0.0);
    if let Some(prev) = cultivation.realm.previous() {
        cultivation.realm = prev;
    }
    cultivation.qi_current = 0.0;
    cultivation.composure = 0.3;
    cultivation.last_qi_zero_at = None;
    contam.entries.clear();

    let keep = cultivation
        .realm
        .required_meridians()
        .max(if cultivation.realm == Realm::Awaken {
            0
        } else {
            1
        });
    let closures = pick_closures(meridians, keep);
    for (is_regular, idx) in closures {
        if is_regular {
            close_meridian(&mut meridians.regular[idx]);
        } else {
            close_meridian(&mut meridians.extraordinary[idx]);
        }
    }
    cultivation.qi_max = 10.0 + meridians.sum_capacity();
    released_qi
}

pub fn on_player_revived(
    clock: Res<CultivationClock>,
    settings: Res<PersistenceSettings>,
    mut events: EventReader<PlayerRevived>,
    mut quota_opened: EventWriter<AscensionQuotaOpened>,
    mut skill_cap_events: EventWriter<SkillCapChanged>,
    mut qi_transfers: Option<ResMut<Events<QiTransfer>>>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut players: Query<(
        &mut Cultivation,
        &mut MeridianSystem,
        &mut Contamination,
        &mut LifeRecord,
        Option<&Position>,
        Option<&CurrentDimension>,
    )>,
) {
    let now = clock.tick;
    for ev in events.read() {
        if let Ok((mut c, mut ms, mut cn, mut life, position, current_dimension)) =
            players.get_mut(ev.entity)
        {
            if matches!(
                life.biography.last(),
                Some(BiographyEntry::Rebirth { tick, .. }) if *tick == now
            ) {
                continue;
            }
            let prior = c.realm;
            let released_qi = apply_revive_penalty(&mut c, &mut ms, &mut cn);
            release_qi_amount_to_zone(
                ev.entity,
                released_qi,
                position,
                current_dimension,
                Some(&life),
                zones.as_deref_mut(),
                qi_transfers.as_deref_mut(),
                "revive_penalty",
            );
            if prior == Realm::Void && c.realm != Realm::Void {
                match release_ascension_quota_slot(&settings) {
                    Ok(release) if release.opened_slot => {
                        quota_opened.send(AscensionQuotaOpened {
                            occupied_slots: release.quota.occupied_slots,
                        });
                    }
                    Ok(_) => {}
                    Err(error) => {
                        tracing::warn!(
                            "[bong][cultivation] failed to release ascension quota after revive for {:?}: {error}",
                            ev.entity,
                        );
                    }
                }
            }
            life.push(BiographyEntry::Rebirth {
                prior_realm: prior,
                new_realm: c.realm,
                tick: now,
            });
            let new_cap = super::breakthrough::skill_cap_for_realm(c.realm);
            for skill in SkillId::ALL {
                skill_cap_events.send(SkillCapChanged {
                    char_entity: ev.entity,
                    skill,
                    new_cap,
                });
            }
            tracing::info!(
                "[bong][cultivation] applied revive penalty to {:?}: realm {:?} -> {:?}",
                ev.entity,
                prior,
                c.realm
            );
        }
    }
}

pub fn on_player_terminated(
    settings: Res<PersistenceSettings>,
    mut commands: Commands,
    mut events: EventReader<PlayerTerminated>,
    mut quota_opened: EventWriter<AscensionQuotaOpened>,
    mut qi_transfers: Option<ResMut<Events<QiTransfer>>>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    players: Query<TerminatedPlayerQueryItem<'_>>,
) {
    let mut processed_entities = std::collections::HashSet::new();
    for ev in events.read() {
        if !processed_entities.insert(ev.entity) {
            tracing::warn!(
                "[bong][cultivation] skip duplicate PlayerTerminated for {:?} in same update",
                ev.entity,
            );
            continue;
        }
        let was_void;
        if let Ok((cultivation, position, current_dimension, life_record)) = players.get(ev.entity)
        {
            was_void = cultivation.realm == Realm::Void;
            release_terminated_qi_to_zone(
                ev.entity,
                cultivation,
                position,
                current_dimension,
                life_record,
                zones.as_deref_mut(),
                qi_transfers.as_deref_mut(),
            );
        } else {
            was_void = false;
        }
        if was_void {
            match release_ascension_quota_slot(&settings) {
                Ok(release) if release.opened_slot => {
                    quota_opened.send(AscensionQuotaOpened {
                        occupied_slots: release.quota.occupied_slots,
                    });
                }
                Ok(_) => {}
                Err(error) => {
                    tracing::warn!(
                        "[bong][cultivation] failed to release ascension quota after termination for {:?}: {error}",
                        ev.entity,
                    );
                }
            }
        }
        if let Some(mut e) = commands.get_entity(ev.entity) {
            e.remove::<Cultivation>();
            e.remove::<MeridianSystem>();
            e.remove::<Contamination>();
            e.remove::<PracticeLog>();
            e.remove::<QiColor>();
            // 决策门 #1 = B：跨周目（新角色）SEVERED 全重置 INTACT
            e.remove::<crate::cultivation::meridian::severed::MeridianSeveredPermanent>();
            tracing::info!(
                "[bong][cultivation] terminated entity {:?} — removed cultivation components",
                ev.entity
            );
        }
    }
}

fn release_terminated_qi_to_zone(
    entity: Entity,
    cultivation: &Cultivation,
    position: Option<&Position>,
    current_dimension: Option<&CurrentDimension>,
    life_record: Option<&LifeRecord>,
    zones: Option<&mut ZoneRegistry>,
    qi_transfers: Option<&mut Events<QiTransfer>>,
) {
    let amount = cultivation.qi_current.max(0.0);
    release_qi_amount_to_zone(
        entity,
        amount,
        position,
        current_dimension,
        life_record,
        zones,
        qi_transfers,
        "terminated",
    );
}

pub fn release_qi_amount_to_zone(
    entity: Entity,
    amount: f64,
    position: Option<&Position>,
    current_dimension: Option<&CurrentDimension>,
    life_record: Option<&LifeRecord>,
    zones: Option<&mut ZoneRegistry>,
    qi_transfers: Option<&mut Events<QiTransfer>>,
    source: &'static str,
) -> f64 {
    if amount <= QI_EPSILON {
        return 0.0;
    }
    let Some(position) = position else {
        tracing::warn!(
            "[bong][cultivation] {source} {:?} with qi={} but no Position; skip qi release",
            entity,
            amount,
        );
        return 0.0;
    };
    let Some(zones) = zones else {
        tracing::warn!(
            "[bong][cultivation] {source} {:?} with qi={} but no ZoneRegistry; skip qi release",
            entity,
            amount,
        );
        return 0.0;
    };
    let dimension = current_dimension
        .map(|current| current.0)
        .unwrap_or(DimensionKind::Overworld);
    let Some(zone_name) = zones
        .find_zone(dimension, position.0)
        .map(|zone| zone.name.clone())
    else {
        tracing::warn!(
            "[bong][cultivation] {source} {:?} with qi={} outside known zone; skip qi release",
            entity,
            amount,
        );
        return 0.0;
    };
    let Some(zone) = zones.find_zone_mut(zone_name.as_str()) else {
        return 0.0;
    };

    let from = terminated_qi_account_id(entity, life_record);
    let to = QiAccountId::zone(zone.name.clone());
    let zone_current = zone.spirit_qi * QI_ZONE_UNIT_CAPACITY;
    let outcome = match qi_release_to_zone(amount, from, to, zone_current, QI_ZONE_UNIT_CAPACITY) {
        Ok(outcome) => outcome,
        Err(error) => {
            tracing::warn!(
                ?error,
                "[bong][cultivation] invalid terminated qi release for {:?}",
                entity,
            );
            return 0.0;
        }
    };

    zone.spirit_qi = outcome.zone_after / QI_ZONE_UNIT_CAPACITY;
    if let Some(transfer) = outcome.transfer {
        if let Some(qi_transfers) = qi_transfers {
            qi_transfers.send(transfer);
        } else {
            tracing::warn!(
                "[bong][cultivation] terminated qi release for {:?} has no QiTransfer event resource",
                entity,
            );
        }
    }
    if outcome.overflow > QI_EPSILON {
        tracing::warn!(
            "[bong][cultivation] terminated qi release for {:?} overflowed zone cap by {}",
            entity,
            outcome.overflow,
        );
    }
    outcome.accepted
}

fn terminated_qi_account_id(entity: Entity, life_record: Option<&LifeRecord>) -> QiAccountId {
    if let Some(life_record) = life_record {
        if !life_record.character_id.trim().is_empty() {
            return QiAccountId::player(life_record.character_id.clone());
        }
    }
    QiAccountId::player(format!("entity:{entity:?}"))
}

/// 将致死触发转发到生平卷（by caller）与 Redis 外发通道（留给 network 模块接入）。
pub fn log_death_trigger(
    mut events: EventReader<CultivationDeathTrigger>,
    mut out: EventWriter<CultivationDeathTrigger>,
) {
    // 简单 pass-through + tracing，真实接入时把 context 推到 network::agent_bridge
    for ev in events.read() {
        tracing::warn!(
            "[bong][cultivation] DEATH TRIGGER entity={:?} cause={:?} context={}",
            ev.entity,
            ev.cause,
            ev.context
        );
        out.send(ev.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::MeridianId;
    use crate::cultivation::tick::CultivationClock;
    use crate::persistence::{complete_tribulation_ascension, load_ascension_quota};
    use crate::player::state::canonical_player_id;
    use crate::qi_physics::{QiAccountId, QiTransferReason};
    use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};
    use valence::prelude::{App, Events, Position};

    fn temp_persistence_settings(test_name: &str) -> (PersistenceSettings, std::path::PathBuf) {
        let temp_root = std::env::temp_dir().join(format!(
            "bong-death-hooks-{test_name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos(),
        ));
        let db_path = temp_root.join("data").join("bong.db");
        let deceased_dir = temp_root
            .join("library-web")
            .join("public")
            .join("deceased");
        let settings = PersistenceSettings::with_paths(&db_path, &deceased_dir, "death-hooks-test");
        crate::persistence::bootstrap_sqlite(settings.db_path(), settings.server_run_id())
            .expect("bootstrap should succeed");
        (settings, temp_root)
    }

    #[test]
    fn revive_penalty_drops_one_realm_and_closes_excess() {
        let mut c = Cultivation {
            realm: Realm::Induce,
            qi_max: 100.0,
            qi_current: 50.0,
            composure: 1.0,
            ..Default::default()
        };
        let mut ms = MeridianSystem::default();
        ms.get_mut(MeridianId::Lung).opened = true;
        ms.get_mut(MeridianId::LargeIntestine).opened = true;
        ms.get_mut(MeridianId::Stomach).opened = true;
        let mut cn = Contamination::default();

        apply_revive_penalty(&mut c, &mut ms, &mut cn);
        assert_eq!(c.realm, Realm::Awaken);
        assert_eq!(c.qi_current, 0.0);
        assert!((c.composure - 0.3).abs() < 1e-9);
        // 醒灵正典门槛为 1 条，重生降境后只保留最低门槛。
        assert_eq!(ms.opened_count(), Realm::Awaken.required_meridians());
    }

    #[test]
    fn revive_at_awaken_stays_awaken() {
        let mut c = Cultivation {
            realm: Realm::Awaken,
            ..Default::default()
        };
        let mut ms = MeridianSystem::default();
        let mut cn = Contamination::default();
        apply_revive_penalty(&mut c, &mut ms, &mut cn);
        assert_eq!(c.realm, Realm::Awaken);
    }

    #[test]
    fn revive_penalty_does_not_mutate_character_anchor() {
        let mut app = App::new();
        app.insert_resource(PersistenceSettings::default());
        app.insert_resource(CultivationClock { tick: 42 });
        app.add_event::<PlayerRevived>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<SkillCapChanged>();
        app.add_systems(valence::prelude::Update, on_player_revived);

        let entity = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Induce,
                    qi_current: 8.0,
                    composure: 0.9,
                    ..Default::default()
                },
                MeridianSystem::default(),
                Contamination::default(),
                LifeRecord::new(canonical_player_id("Alice")),
            ))
            .id();

        app.world_mut().send_event(PlayerRevived { entity });
        app.update();

        let life = app
            .world()
            .get::<LifeRecord>(entity)
            .expect("life record should remain attached after revive");

        assert_eq!(life.character_id, canonical_player_id("Alice"));
        assert!(matches!(
            life.biography.last(),
            Some(BiographyEntry::Rebirth { tick: 42, .. })
        ));
    }

    #[test]
    fn revived_hook_releases_previous_qi_to_current_zone() {
        let mut app = App::new();
        app.insert_resource(PersistenceSettings::default());
        app.insert_resource(CultivationClock { tick: 42 });
        app.insert_resource(ZoneRegistry::fallback());
        app.add_event::<PlayerRevived>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<SkillCapChanged>();
        app.add_event::<QiTransfer>();
        app.add_systems(valence::prelude::Update, on_player_revived);
        let before = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .unwrap()
            .spirit_qi;
        let entity = app
            .world_mut()
            .spawn((
                Position::new([8.0, 66.0, 8.0]),
                Cultivation {
                    realm: Realm::Induce,
                    qi_current: 8.0,
                    composure: 0.9,
                    ..Default::default()
                },
                MeridianSystem::default(),
                Contamination::default(),
                LifeRecord::new(canonical_player_id("Alice")),
            ))
            .id();

        app.world_mut().send_event(PlayerRevived { entity });
        app.update();

        let after = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .unwrap()
            .spirit_qi;
        let transfers: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<QiTransfer>>()
            .drain()
            .collect();
        assert!(after > before);
        assert_eq!(transfers.len(), 1);
        assert_eq!(transfers[0].reason, QiTransferReason::ReleaseToZone);
    }

    #[test]
    fn revived_hook_skips_when_rebirth_already_recorded_for_tick() {
        let mut app = App::new();
        app.insert_resource(PersistenceSettings::default());
        app.insert_resource(CultivationClock { tick: 42 });
        app.add_event::<PlayerRevived>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<SkillCapChanged>();
        app.add_systems(valence::prelude::Update, on_player_revived);

        let entity = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Induce,
                    qi_current: 8.0,
                    composure: 0.9,
                    ..Default::default()
                },
                MeridianSystem::default(),
                Contamination::default(),
                LifeRecord {
                    character_id: canonical_player_id("Alice"),
                    created_at: 0,
                    biography: vec![BiographyEntry::Rebirth {
                        prior_realm: Realm::Induce,
                        new_realm: Realm::Awaken,
                        tick: 42,
                    }],
                    insights_taken: Vec::new(),
                    death_insights: Vec::new(),
                    skill_milestones: Vec::new(),
                    spirit_root_first: None,
                },
            ))
            .id();

        app.world_mut().send_event(PlayerRevived { entity });
        app.update();

        let cultivation = app
            .world()
            .get::<Cultivation>(entity)
            .expect("cultivation should remain attached");
        let life = app
            .world()
            .get::<LifeRecord>(entity)
            .expect("life record should remain attached");

        assert_eq!(cultivation.realm, Realm::Induce);
        assert_eq!(life.biography.len(), 1);
    }

    #[test]
    fn terminated_void_player_releases_ascension_quota() {
        let (settings, root) = temp_persistence_settings("terminated-void-release-quota");
        complete_tribulation_ascension(&settings, canonical_player_id("Azure").as_str())
            .expect("quota setup should succeed");

        let mut app = App::new();
        app.insert_resource(settings.clone());
        app.add_event::<PlayerTerminated>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<QiTransfer>();
        app.add_systems(valence::prelude::Update, on_player_terminated);

        let entity = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Void,
                    ..Default::default()
                },
                MeridianSystem::default(),
                Contamination::default(),
            ))
            .id();
        app.world_mut().send_event(PlayerTerminated { entity });

        app.update();

        let quota = load_ascension_quota(&settings).expect("quota load should succeed");
        assert_eq!(quota.occupied_slots, 0);
        let quota_events: Vec<_> = app
            .world_mut()
            .resource_mut::<valence::prelude::Events<AscensionQuotaOpened>>()
            .drain()
            .collect();
        assert_eq!(quota_events.len(), 1);
        assert_eq!(quota_events[0].occupied_slots, 0);
        assert!(app.world().get::<Cultivation>(entity).is_none());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn terminated_player_removes_practice_log_and_qi_color() {
        let mut app = App::new();
        app.insert_resource(PersistenceSettings::default());
        app.add_event::<PlayerTerminated>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<QiTransfer>();
        app.add_systems(valence::prelude::Update, on_player_terminated);

        let entity = app
            .world_mut()
            .spawn((
                Cultivation::default(),
                MeridianSystem::default(),
                Contamination::default(),
                PracticeLog::default(),
                QiColor::default(),
            ))
            .id();
        app.world_mut().send_event(PlayerTerminated { entity });

        app.update();

        assert!(app.world().get::<Cultivation>(entity).is_none());
        assert!(app.world().get::<MeridianSystem>(entity).is_none());
        assert!(app.world().get::<Contamination>(entity).is_none());
        assert!(app.world().get::<PracticeLog>(entity).is_none());
        assert!(app.world().get::<QiColor>(entity).is_none());
    }

    #[test]
    fn terminated_player_releases_qi_to_current_zone() {
        let mut app = App::new();
        app.insert_resource(PersistenceSettings::default());
        let mut zones = ZoneRegistry::fallback();
        zones
            .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
            .unwrap()
            .spirit_qi = 0.2;
        app.insert_resource(zones);
        app.add_event::<PlayerTerminated>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<QiTransfer>();
        app.add_systems(valence::prelude::Update, on_player_terminated);

        let entity = app
            .world_mut()
            .spawn((
                Cultivation {
                    qi_current: 10.0,
                    ..Default::default()
                },
                MeridianSystem::default(),
                Contamination::default(),
                Position::new([8.0, 66.0, 8.0]),
                LifeRecord::new(canonical_player_id("Azure")),
            ))
            .id();
        app.world_mut().send_event(PlayerTerminated { entity });

        app.update();

        let zone_after = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .unwrap()
            .spirit_qi;
        assert!(
            (zone_after - 0.4).abs() < 1e-9,
            "10 qi should add 0.2 normalized zone qi, got {zone_after}",
        );
        let transfers: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<QiTransfer>>()
            .drain()
            .collect();
        assert_eq!(transfers.len(), 1);
        assert_eq!(
            transfers[0].from,
            QiAccountId::player(canonical_player_id("Azure"))
        );
        assert_eq!(transfers[0].to, QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME));
        assert!((transfers[0].amount - 10.0).abs() < 1e-9);
        assert_eq!(transfers[0].reason, QiTransferReason::ReleaseToZone);
        assert!(app.world().get::<Cultivation>(entity).is_none());
    }

    #[test]
    fn duplicate_terminated_events_release_qi_once() {
        let mut app = App::new();
        app.insert_resource(PersistenceSettings::default());
        let mut zones = ZoneRegistry::fallback();
        zones
            .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
            .unwrap()
            .spirit_qi = 0.2;
        app.insert_resource(zones);
        app.add_event::<PlayerTerminated>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<QiTransfer>();
        app.add_systems(valence::prelude::Update, on_player_terminated);

        let entity = app
            .world_mut()
            .spawn((
                Cultivation {
                    qi_current: 10.0,
                    ..Default::default()
                },
                MeridianSystem::default(),
                Contamination::default(),
                Position::new([8.0, 66.0, 8.0]),
                LifeRecord::new(canonical_player_id("Azure")),
            ))
            .id();
        app.world_mut().send_event(PlayerTerminated { entity });
        app.world_mut().send_event(PlayerTerminated { entity });

        app.update();

        let zone_after = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .unwrap()
            .spirit_qi;
        assert!(
            (zone_after - 0.4).abs() < 1e-9,
            "duplicate termination events must not double release qi, got {zone_after}",
        );
        let transfers: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<QiTransfer>>()
            .drain()
            .collect();
        assert_eq!(transfers.len(), 1);
        assert!((transfers[0].amount - 10.0).abs() < 1e-9);
        assert!(app.world().get::<Cultivation>(entity).is_none());
    }

    #[test]
    fn terminated_qi_release_caps_at_zone_capacity() {
        let mut app = App::new();
        app.insert_resource(PersistenceSettings::default());
        let mut zones = ZoneRegistry::fallback();
        zones
            .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
            .unwrap()
            .spirit_qi = 0.95;
        app.insert_resource(zones);
        app.add_event::<PlayerTerminated>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<QiTransfer>();
        app.add_systems(valence::prelude::Update, on_player_terminated);

        let entity = app
            .world_mut()
            .spawn((
                Cultivation {
                    qi_current: 10.0,
                    ..Default::default()
                },
                MeridianSystem::default(),
                Contamination::default(),
                Position::new([8.0, 66.0, 8.0]),
            ))
            .id();
        app.world_mut().send_event(PlayerTerminated { entity });

        app.update();

        let zone_after = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .unwrap()
            .spirit_qi;
        assert!((zone_after - 1.0).abs() < 1e-9);
        let transfers: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<QiTransfer>>()
            .drain()
            .collect();
        assert_eq!(transfers.len(), 1);
        assert_eq!(transfers[0].to, QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME));
        assert!((transfers[0].amount - 2.5).abs() < 1e-9);
        assert_eq!(transfers[0].reason, QiTransferReason::ReleaseToZone);
    }
}
