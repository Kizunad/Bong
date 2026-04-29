//! 死亡对外契约（plan §4）— 修炼侧只 emit 致死触发，生死判定由战斗 plan 收口。
//!
//! 另外提供 `PlayerRevived` 监听：战斗 plan 完成重生后发事件，本 plan
//! 应用境界-1、qi=0、composure=0.3、contam 清空、LIFO 关脉等惩罚。
//! `PlayerTerminated` 也有监听 hook，停止该实体的所有修炼 tick（通过
//! 移除 Cultivation Component 实现）。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Commands, Entity, Event, EventReader, EventWriter, Query};

use super::components::{Contamination, Cultivation, MeridianSystem, Realm};
use super::life_record::{BiographyEntry, LifeRecord};
use super::qi_zero_decay::{close_meridian, pick_closures};
use super::tick::CultivationClock;
use super::tribulation::AscensionQuotaOpened;
use crate::persistence::{release_ascension_quota_slot, PersistenceSettings};
use crate::skill::components::SkillId;
use crate::skill::events::SkillCapChanged;
use valence::prelude::Res;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CultivationDeathCause {
    BreakthroughBackfire,
    MeridianCollapse,
    NegativeZoneDrain,
    ContaminationOverflow,
    NaturalAging,
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

/// 重生响应：境界 -1、qi=0、composure=0.3、contam 清空、LIFO 关脉至对应境界。
pub fn apply_revive_penalty(
    cultivation: &mut Cultivation,
    meridians: &mut MeridianSystem,
    contam: &mut Contamination,
) {
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
}

pub fn on_player_revived(
    clock: Res<CultivationClock>,
    settings: Res<PersistenceSettings>,
    mut events: EventReader<PlayerRevived>,
    mut quota_opened: EventWriter<AscensionQuotaOpened>,
    mut skill_cap_events: EventWriter<SkillCapChanged>,
    mut players: Query<(
        &mut Cultivation,
        &mut MeridianSystem,
        &mut Contamination,
        &mut LifeRecord,
    )>,
) {
    let now = clock.tick;
    for ev in events.read() {
        if let Ok((mut c, mut ms, mut cn, mut life)) = players.get_mut(ev.entity) {
            if matches!(
                life.biography.last(),
                Some(BiographyEntry::Rebirth { tick, .. }) if *tick == now
            ) {
                continue;
            }
            let prior = c.realm;
            apply_revive_penalty(&mut c, &mut ms, &mut cn);
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
            for skill in [SkillId::Herbalism, SkillId::Alchemy, SkillId::Forging] {
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
    players: Query<&Cultivation>,
) {
    for ev in events.read() {
        let was_void = players
            .get(ev.entity)
            .map(|cultivation| cultivation.realm == Realm::Void)
            .unwrap_or(false);
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
            tracing::info!(
                "[bong][cultivation] terminated entity {:?} — removed cultivation components",
                ev.entity
            );
        }
    }
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
    use valence::prelude::App;

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
        // Awaken 要求 0 条，但保留至少 1（下限 special case）
        assert!(ms.opened_count() <= 3);
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
}
