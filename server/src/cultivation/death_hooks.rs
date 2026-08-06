//! 死亡对外契约（plan §4）— 修炼侧只 emit 致死触发，生死判定由战斗 plan 收口。
//!
//! 另提供 `CultivationReviveRequested` 监听：dev command 恢复战斗生命周期后发请求，
//! 修炼层应用境界-1、qi=0、composure=0.3、contam 清空、LIFO 关脉等惩罚。
//! `PlayerRevived` 仅表示复活全链已完成，供下游刷新快照、音频与 HUD；
//! `PlayerTerminated` 监听 hook 则停止该实体的所有修炼 tick（通过
//! 移除 Cultivation Component 实现）。

use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, Commands, Entity, Event, EventReader, EventWriter, Events, Position, Query, Res,
    ResMut, Without,
};

use super::color::PracticeLog;
use super::components::{
    ActorQiIdentity, ActorQiKind, Contamination, Cultivation, MeridianSystem, QiColor, QiFlowError,
    QiFlowOutcome, Realm,
};
use super::life_record::{BiographyEntry, LifeRecord};
use super::qi_zero_decay::{close_meridian, pick_closures};
use super::tick::CultivationClock;
use super::tribulation::AscensionQuotaOpened;
use crate::npc::spawn::NpcMarker;
use crate::persistence::{release_ascension_quota_slot, PersistenceSettings};
use crate::qi_physics::{QiTransfer, QiTransferReason, WorldQiAccount};
use crate::skill::components::SkillId;
use crate::skill::events::SkillCapChanged;
use crate::world::dimension::CurrentDimension;
use crate::world::zone::ZoneRegistry;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CultivationDeathCause {
    BreakthroughBackfire,
    MeridianCollapse,
    NegativeZoneDrain,
    ContaminationOverflow,
    NaturalAging,
    DevCommand,
    SwarmQiDrain,
    VoidQuotaExceeded,
    VoidActionBacklash,
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

/// 请求修仙层执行 dev-only 复活惩罚与真元结算。
///
/// 生产战斗复活会先在 `revive_lifecycle` 的持久化事务内完成同一结算，再只发
/// [`PlayerRevived`] 通知；将请求与通知拆开后，不再依赖 CombatClock/CultivationClock
/// tick 恰好相等来防双罚。
#[derive(Debug, Clone, Event)]
pub struct CultivationReviveRequested {
    pub entity: Entity,
}

#[derive(Debug, Clone, Event)]
pub struct PlayerTerminated {
    pub entity: Entity,
}

type TerminatedPlayerQueryItem<'a> = (
    &'a mut Cultivation,
    Option<&'a Position>,
    Option<&'a CurrentDimension>,
    Option<&'a LifeRecord>,
);

fn release_cultivation_qi_to_zone(
    cultivation: &mut Cultivation,
    position: Option<&Position>,
    current_dimension: Option<&CurrentDimension>,
    life_record: &LifeRecord,
    zones: Option<&mut ZoneRegistry>,
    ledger: &mut WorldQiAccount,
) -> Result<Vec<QiTransfer>, QiFlowError> {
    let actor = ActorQiIdentity::from_life_record(life_record, ActorQiKind::Player)?;
    let zone = match (position, current_dimension, zones) {
        (Some(position), Some(current_dimension), Some(zones)) => {
            let zone_name = zones
                .find_zone(current_dimension.0, position.0)
                .map(|zone| zone.name.clone());
            zone_name.and_then(|zone_name| zones.find_zone_mut(zone_name.as_str()))
        }
        _ => None,
    };
    let amount = cultivation.qi_snapshot().current;
    cultivation
        .release_to_zone(
            zone,
            ledger,
            &actor,
            amount,
            QiTransferReason::ReleaseToZone,
        )
        .map(|outcome| outcome.transfers)
}

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
    // 同 qi_zero_decay / tribulation：缩小 qi_max 后收敛 qi_max_frozen，
    // 避免 effective_max 变负锁死真元回复（Pi review 补漏的第三处 shrink 路径）。
    if let Some(frozen) = cultivation.qi_max_frozen {
        cultivation.qi_max_frozen = Some(
            frozen
                .min(cultivation.qi_max * super::breakthrough::BREAKTHROUGH_FAIL_FROZEN_CAP_RATIO),
        );
    }
    released_qi
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub fn on_cultivation_revive_requested(
    clock: Res<CultivationClock>,
    settings: Res<PersistenceSettings>,
    mut events: EventReader<CultivationReviveRequested>,
    mut completed: EventWriter<PlayerRevived>,
    mut quota_opened: EventWriter<AscensionQuotaOpened>,
    mut skill_cap_events: EventWriter<SkillCapChanged>,
    mut qi_transfers: EventWriter<QiTransfer>,
    mut ledger: ResMut<WorldQiAccount>,
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
            let prior = c.realm;
            let mut staged_cultivation = c.clone();
            let mut staged_meridians = ms.clone();
            let mut staged_contam = cn.clone();
            apply_revive_penalty(
                &mut staged_cultivation,
                &mut staged_meridians,
                &mut staged_contam,
            );
            let transfers = match release_cultivation_qi_to_zone(
                &mut c,
                position,
                current_dimension,
                &life,
                zones.as_deref_mut(),
                &mut ledger,
            ) {
                Ok(transfers) => transfers,
                Err(error) => {
                    tracing::warn!(
                        ?error,
                        "[bong][cultivation] revive qi release failed closed for {:?}",
                        ev.entity,
                    );
                    continue;
                }
            };
            let qi_state = c.qi_snapshot();
            let staged_qi = staged_cultivation.qi_snapshot();
            staged_cultivation
                .set_for_init(super::components::CultivationQiInit {
                    current: qi_state.current,
                    max: staged_qi.max,
                    frozen: staged_qi.frozen,
                })
                .expect("revive penalty staged qi capacity must remain valid");
            *c = staged_cultivation;
            *ms = staged_meridians;
            *cn = staged_contam;
            for transfer in transfers {
                qi_transfers.send(transfer);
            }
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
            completed.send(PlayerRevived { entity: ev.entity });
            tracing::info!(
                "[bong][cultivation] applied revive penalty to {:?}: realm {:?} -> {:?}",
                ev.entity,
                prior,
                c.realm
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn on_player_terminated(
    settings: Res<PersistenceSettings>,
    mut commands: Commands,
    mut events: EventReader<PlayerTerminated>,
    mut quota_opened: EventWriter<AscensionQuotaOpened>,
    mut qi_transfers: EventWriter<QiTransfer>,
    mut ledger: ResMut<WorldQiAccount>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut players: Query<TerminatedPlayerQueryItem<'_>, Without<NpcMarker>>,
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
        if let Ok((mut cultivation, position, current_dimension, life_record)) =
            players.get_mut(ev.entity)
        {
            was_void = cultivation.realm == Realm::Void;
            let Some(life_record) = life_record else {
                tracing::warn!(
                    "[bong][cultivation] termination qi release failed closed for {:?}: missing LifeRecord",
                    ev.entity,
                );
                continue;
            };
            let transfers = match release_cultivation_qi_to_zone(
                &mut cultivation,
                position,
                current_dimension,
                life_record,
                zones.as_deref_mut(),
                &mut ledger,
            ) {
                Ok(transfers) => transfers,
                Err(error) => {
                    tracing::warn!(
                        ?error,
                        "[bong][cultivation] termination qi release failed closed for {:?}",
                        ev.entity,
                    );
                    continue;
                }
            };
            for transfer in transfers {
                qi_transfers.send(transfer);
            }
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

#[allow(clippy::too_many_arguments)]
pub fn release_qi_amount_to_zone(
    cultivation: &mut Cultivation,
    amount: f64,
    position: Option<&Position>,
    current_dimension: Option<&CurrentDimension>,
    life_record: Option<&LifeRecord>,
    zones: Option<&mut ZoneRegistry>,
    ledger: &mut WorldQiAccount,
    mut qi_transfers: Option<&mut Events<QiTransfer>>,
    source: &'static str,
) -> Result<QiFlowOutcome, QiFlowError> {
    let Some(life_record) = life_record else {
        tracing::warn!(
            "[bong][cultivation] reject {source} qi release without canonical LifeRecord",
        );
        return Err(QiFlowError::InvalidActorIdentity);
    };
    let actor = ActorQiIdentity::from_life_record(life_record, ActorQiKind::Player)?;
    let zone = match (position, current_dimension, zones) {
        (Some(position), Some(current_dimension), Some(zones)) => {
            let zone_name = zones
                .find_zone(current_dimension.0, position.0)
                .map(|zone| zone.name.clone());
            zone_name.and_then(|zone_name| zones.find_zone_mut(zone_name.as_str()))
        }
        _ => None,
    };
    let outcome = cultivation.release_to_zone(
        zone,
        ledger,
        &actor,
        amount,
        QiTransferReason::ReleaseToZone,
    )?;
    if let Some(qi_transfers) = qi_transfers.as_mut() {
        for transfer in outcome.transfers.iter().cloned() {
            qi_transfers.send(transfer);
        }
    }
    Ok(outcome)
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
    use crate::cultivation::color::PracticeLog;
    use crate::cultivation::components::MeridianId;
    use crate::cultivation::tick::CultivationClock;
    use crate::persistence::{
        complete_tribulation_ascension, load_ascension_quota, persist_active_tribulation,
        ActiveTribulationRecord,
    };
    use crate::player::state::canonical_player_id;
    use crate::qi_physics::{qi_flow_overflow_account, QiAccountId, QiTransferReason};
    use crate::world::dimension::DimensionKind;
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
        app.add_event::<CultivationReviveRequested>();
        app.add_event::<PlayerRevived>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<SkillCapChanged>();
        app.add_event::<QiTransfer>();
        app.insert_resource(WorldQiAccount::default());
        app.add_systems(valence::prelude::Update, on_cultivation_revive_requested);

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

        app.world_mut()
            .send_event(CultivationReviveRequested { entity });
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
        let mut zones = ZoneRegistry::fallback();
        zones
            .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
            .unwrap()
            .spirit_qi = 0.2;
        app.insert_resource(zones);
        app.add_event::<CultivationReviveRequested>();
        app.add_event::<PlayerRevived>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<SkillCapChanged>();
        app.add_event::<QiTransfer>();
        app.insert_resource(WorldQiAccount::default());
        app.add_systems(valence::prelude::Update, on_cultivation_revive_requested);
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
                CurrentDimension(DimensionKind::Overworld),
                LifeRecord::new(canonical_player_id("Alice")),
            ))
            .id();

        app.world_mut()
            .send_event(CultivationReviveRequested { entity });
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
    fn completed_revival_notification_does_not_apply_cultivation_penalty() {
        let mut app = App::new();
        app.insert_resource(PersistenceSettings::default());
        app.insert_resource(CultivationClock { tick: 42 });
        app.add_event::<CultivationReviveRequested>();
        app.add_event::<PlayerRevived>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<SkillCapChanged>();
        app.add_event::<QiTransfer>();
        app.insert_resource(WorldQiAccount::default());
        app.add_systems(valence::prelude::Update, on_cultivation_revive_requested);

        let mut meridians = MeridianSystem::default();
        meridians.get_mut(MeridianId::Lung).opened = true;
        let expected_meridians = meridians.clone();
        let entity = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Induce,
                    qi_current: 8.0,
                    composure: 0.9,
                    ..Default::default()
                },
                meridians,
                Contamination::default(),
                LifeRecord::new(canonical_player_id("Alice")),
            ))
            .id();

        app.world_mut().send_event(PlayerRevived { entity });
        app.update();

        let cultivation = app
            .world()
            .get::<Cultivation>(entity)
            .expect("cultivation should remain attached");
        let meridians = app
            .world()
            .get::<MeridianSystem>(entity)
            .expect("meridians should remain attached");
        let life = app
            .world()
            .get::<LifeRecord>(entity)
            .expect("life record should remain attached");

        assert_eq!(cultivation.realm, Realm::Induce);
        assert!((cultivation.qi_current - 8.0).abs() < 1e-9);
        assert!((cultivation.composure - 0.9).abs() < 1e-9);
        assert_eq!(meridians, &expected_meridians);
        assert!(life.biography.is_empty());
        assert_eq!(app.world().resource::<Events<QiTransfer>>().len(), 0);
        assert_eq!(app.world().resource::<Events<SkillCapChanged>>().len(), 0);
    }

    #[test]
    fn revive_request_is_not_suppressed_by_rebirth_at_same_cultivation_tick() {
        let mut app = App::new();
        app.insert_resource(PersistenceSettings::default());
        app.insert_resource(CultivationClock { tick: 42 });
        app.add_event::<CultivationReviveRequested>();
        app.add_event::<PlayerRevived>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<SkillCapChanged>();
        app.add_event::<QiTransfer>();
        app.insert_resource(WorldQiAccount::default());
        app.add_systems(valence::prelude::Update, on_cultivation_revive_requested);

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
                    ..LifeRecord::default()
                },
            ))
            .id();

        app.world_mut()
            .send_event(CultivationReviveRequested { entity });
        app.update();

        let cultivation = app
            .world()
            .get::<Cultivation>(entity)
            .expect("cultivation should remain attached");
        let life = app
            .world()
            .get::<LifeRecord>(entity)
            .expect("life record should remain attached");

        assert_eq!(cultivation.realm, Realm::Awaken);
        assert_eq!(life.biography.len(), 2);
        assert!(matches!(
            life.biography.last(),
            Some(BiographyEntry::Rebirth {
                prior_realm: Realm::Induce,
                new_realm: Realm::Awaken,
                tick: 42,
            })
        ));
        assert_eq!(app.world().resource::<Events<PlayerRevived>>().len(), 1);
    }

    #[test]
    fn terminated_void_player_releases_ascension_quota() {
        let (settings, root) = temp_persistence_settings("terminated-void-release-quota");
        persist_active_tribulation(
            &settings,
            &ActiveTribulationRecord {
                char_id: canonical_player_id("Azure"),
                kind: "du_xu".to_string(),
                source: String::new(),
                origin_dimension: Some("minecraft:overworld".to_string()),
                wave_current: 3,
                waves_total: 3,
                started_tick: 10,
                epicenter: [0.0, 64.0, 0.0],
                intensity: 0.0,
            },
        )
        .expect("active DuXu should persist before quota setup");
        complete_tribulation_ascension(&settings, canonical_player_id("Azure").as_str())
            .expect("quota setup should succeed");

        let mut app = App::new();
        app.insert_resource(settings.clone());
        app.add_event::<PlayerTerminated>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<QiTransfer>();
        app.insert_resource(WorldQiAccount::default());
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
                LifeRecord::new(canonical_player_id("Azure")),
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
        app.insert_resource(WorldQiAccount::default());
        app.add_systems(valence::prelude::Update, on_player_terminated);

        let entity = app
            .world_mut()
            .spawn((
                Cultivation::default(),
                MeridianSystem::default(),
                Contamination::default(),
                LifeRecord::new(canonical_player_id("Azure")),
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
        app.insert_resource(WorldQiAccount::default());
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
                LifeRecord::new(canonical_player_id("Azure")),
                Position::new([8.0, 66.0, 8.0]),
                CurrentDimension(DimensionKind::Overworld),
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
        app.insert_resource(WorldQiAccount::default());
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
                LifeRecord::new(canonical_player_id("Azure")),
                Position::new([8.0, 66.0, 8.0]),
                CurrentDimension(DimensionKind::Overworld),
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
        app.insert_resource(WorldQiAccount::default());
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
                LifeRecord::new(canonical_player_id("Azure")),
                Position::new([8.0, 66.0, 8.0]),
                CurrentDimension(DimensionKind::Overworld),
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
        assert_eq!(transfers.len(), 2);
        assert_eq!(transfers[0].to, QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME));
        assert!((transfers[0].amount - 2.5).abs() < 1e-9);
        assert_eq!(transfers[0].reason, QiTransferReason::ReleaseToZone);
        assert_eq!(transfers[1].to, qi_flow_overflow_account());
        assert!((transfers[1].amount - 7.5).abs() < 1e-9);
        assert_eq!(transfers[1].reason, QiTransferReason::ReleaseToZone);
    }

    #[test]
    fn terminated_qi_release_without_zone_routes_to_overflow_transfer() {
        let mut app = App::new();
        app.insert_resource(PersistenceSettings::default());
        app.add_event::<PlayerTerminated>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<QiTransfer>();
        app.insert_resource(WorldQiAccount::default());
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
                LifeRecord::new(canonical_player_id("Azure")),
            ))
            .id();
        app.world_mut().send_event(PlayerTerminated { entity });

        app.update();

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
        assert_eq!(transfers[0].to, qi_flow_overflow_account());
        assert!((transfers[0].amount - 10.0).abs() < 1e-9);
        assert_eq!(transfers[0].reason, QiTransferReason::ReleaseToZone);
    }

    /// 直接面向 `release_qi_amount_to_zone` 的饱和单测组。这里锁定 facade 的外部契约：
    /// canonical actor identity、signed zone、固定稳定 overflow、可选广播投影与失败原子性。
    mod release_qi_amount_to_zone_unit_tests {
        use super::*;
        use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
        use crate::qi_physics::{qi_flow_overflow_account, QiAccountId};

        fn cultivation_with_qi(current: f64) -> Cultivation {
            Cultivation {
                qi_current: current,
                qi_max: current.max(100.0),
                ..Default::default()
            }
        }

        fn life_record(name: &str) -> LifeRecord {
            LifeRecord::new(canonical_player_id(name))
        }

        fn fresh_zones_with_qi(spirit_qi: f64) -> ZoneRegistry {
            let mut zones = ZoneRegistry::fallback();
            zones
                .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
                .expect("spawn zone should exist")
                .spirit_qi = spirit_qi;
            zones
        }

        fn overworld_dim() -> CurrentDimension {
            CurrentDimension(DimensionKind::Overworld)
        }

        fn pos_in_spawn_zone() -> Position {
            Position::new([8.0, 66.0, 8.0])
        }

        #[test]
        fn zero_amount_is_a_valid_noop() {
            let mut cultivation = cultivation_with_qi(5.0);
            let life_record = life_record("Mira");
            let mut zones = fresh_zones_with_qi(0.2);
            let mut ledger = WorldQiAccount::default();
            let mut events = Events::<QiTransfer>::default();
            let position = pos_in_spawn_zone();
            let dimension = overworld_dim();

            let outcome = release_qi_amount_to_zone(
                &mut cultivation,
                0.0,
                Some(&position),
                Some(&dimension),
                Some(&life_record),
                Some(&mut zones),
                &mut ledger,
                Some(&mut events),
                "unit-zero",
            )
            .expect("zero release should be a valid no-op");

            assert_eq!(outcome.source_debited, 0.0);
            assert_eq!(cultivation.qi_current, 5.0);
            assert_eq!(ledger.total(), 0.0);
            assert!(ledger.transfers().is_empty());
            assert!(events.drain().next().is_none());
        }

        #[test]
        fn invalid_amount_fails_without_mutating_any_owner_or_audit() {
            let mut cultivation = cultivation_with_qi(5.0);
            let life_record = life_record("Mira");
            let mut zones = fresh_zones_with_qi(0.2);
            let mut ledger = WorldQiAccount::default();
            let mut events = Events::<QiTransfer>::default();
            let position = pos_in_spawn_zone();
            let dimension = overworld_dim();

            let error = release_qi_amount_to_zone(
                &mut cultivation,
                f64::NAN,
                Some(&position),
                Some(&dimension),
                Some(&life_record),
                Some(&mut zones),
                &mut ledger,
                Some(&mut events),
                "unit-nan",
            )
            .expect_err("NaN release must fail closed");

            assert!(matches!(error, QiFlowError::Physics(_)));
            assert_eq!(cultivation.qi_current, 5.0);
            assert_eq!(
                zones
                    .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
                    .unwrap()
                    .spirit_qi,
                0.2
            );
            assert_eq!(ledger.total(), 0.0);
            assert!(ledger.transfers().is_empty());
            assert!(events.drain().next().is_none());
        }

        #[test]
        fn missing_or_blank_actor_identity_fails_closed() {
            for life_record in [None, Some(LifeRecord::new("   "))] {
                let mut cultivation = cultivation_with_qi(5.0);
                let mut ledger = WorldQiAccount::default();
                let error = release_qi_amount_to_zone(
                    &mut cultivation,
                    3.0,
                    None,
                    None,
                    life_record.as_ref(),
                    None,
                    &mut ledger,
                    None,
                    "unit-invalid-identity",
                )
                .expect_err("durable releases require canonical identity");

                assert!(matches!(error, QiFlowError::InvalidActorIdentity));
                assert_eq!(cultivation.qi_current, 5.0);
                assert_eq!(ledger.total(), 0.0);
                assert!(ledger.transfers().is_empty());
            }
        }

        #[test]
        fn missing_location_credits_fixed_persistent_overflow() {
            let mut cultivation = cultivation_with_qi(5.0);
            let life_record = life_record("Mira");
            let mut ledger = WorldQiAccount::default();
            let mut events = Events::<QiTransfer>::default();

            let outcome = release_qi_amount_to_zone(
                &mut cultivation,
                5.0,
                None,
                None,
                Some(&life_record),
                None,
                &mut ledger,
                Some(&mut events),
                "arbitrary-source-label",
            )
            .expect("missing location should route to stable overflow");

            assert_eq!(cultivation.qi_current, 0.0);
            assert_eq!(outcome.overflow_credited, 5.0);
            assert_eq!(ledger.balance(&qi_flow_overflow_account()), 5.0);
            assert_eq!(ledger.transfers().len(), 1);
            assert_eq!(ledger.transfers()[0].to, qi_flow_overflow_account());
            assert_eq!(
                ledger.transfers()[0].from,
                QiAccountId::player(canonical_player_id("Mira"))
            );
            let broadcast: Vec<_> = events.drain().collect();
            assert_eq!(broadcast, ledger.transfers());
        }

        #[test]
        fn happy_path_debits_actor_and_credits_signed_zone() {
            let mut cultivation = cultivation_with_qi(5.0);
            let life_record = life_record("Mira");
            let mut zones = fresh_zones_with_qi(-0.4);
            let mut ledger = WorldQiAccount::default();
            let mut events = Events::<QiTransfer>::default();
            let position = pos_in_spawn_zone();
            let dimension = overworld_dim();

            let outcome = release_qi_amount_to_zone(
                &mut cultivation,
                5.0,
                Some(&position),
                Some(&dimension),
                Some(&life_record),
                Some(&mut zones),
                &mut ledger,
                Some(&mut events),
                "unit-signed-zone",
            )
            .expect("negative zone should accept release toward zero");

            assert_eq!(cultivation.qi_current, 0.0);
            assert_eq!(outcome.zone_accepted, 5.0);
            assert_eq!(outcome.overflow_credited, 0.0);
            let zone_after = zones
                .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
                .unwrap()
                .spirit_qi;
            assert!((zone_after - (-0.4 + 5.0 / QI_ZONE_UNIT_CAPACITY)).abs() < 1e-9);
            assert_eq!(ledger.total(), 0.0, "external zone must not be mirrored");
            assert_eq!(ledger.transfers().len(), 1);
            assert_eq!(
                ledger.transfers()[0].to,
                QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME)
            );
            assert_eq!(events.drain().count(), 1);
        }

        #[test]
        fn partial_zone_release_commits_overflow_then_zone_with_full_accounting() {
            let mut cultivation = cultivation_with_qi(10.0);
            let life_record = life_record("Mira");
            let mut zones = fresh_zones_with_qi(0.95);
            let mut ledger = WorldQiAccount::default();
            let mut events = Events::<QiTransfer>::default();
            let position = pos_in_spawn_zone();
            let dimension = overworld_dim();

            let outcome = release_qi_amount_to_zone(
                &mut cultivation,
                10.0,
                Some(&position),
                Some(&dimension),
                Some(&life_record),
                Some(&mut zones),
                &mut ledger,
                Some(&mut events),
                "unit-partial",
            )
            .expect("partial release should account for every unit");

            assert_eq!(cultivation.qi_current, 0.0);
            assert!((outcome.zone_accepted - 2.5).abs() < 1e-9);
            assert!((outcome.overflow_credited - 7.5).abs() < 1e-9);
            assert!((outcome.source_debited - 10.0).abs() < 1e-9);
            assert_eq!(ledger.balance(&qi_flow_overflow_account()), 7.5);
            assert_eq!(outcome.transfers[0].to, qi_flow_overflow_account());
            assert_eq!(
                outcome.transfers[1].to,
                QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME)
            );
            assert_eq!(events.drain().collect::<Vec<_>>(), outcome.transfers);
        }

        #[test]
        fn missing_event_resource_does_not_change_physical_settlement() {
            let mut cultivation = cultivation_with_qi(10.0);
            let life_record = life_record("Mira");
            let mut zones = fresh_zones_with_qi(0.95);
            let mut ledger = WorldQiAccount::default();
            let position = pos_in_spawn_zone();
            let dimension = overworld_dim();

            let outcome = release_qi_amount_to_zone(
                &mut cultivation,
                10.0,
                Some(&position),
                Some(&dimension),
                Some(&life_record),
                Some(&mut zones),
                &mut ledger,
                None,
                "unit-no-events",
            )
            .expect("broadcast projection must be optional");

            assert_eq!(cultivation.qi_current, 0.0);
            assert!((outcome.zone_accepted - 2.5).abs() < 1e-9);
            assert!((ledger.balance(&qi_flow_overflow_account()) - 7.5).abs() < 1e-9);
            assert_eq!(ledger.transfers().len(), 2);
        }

        #[test]
        fn insufficient_actor_qi_fails_before_mutation() {
            let mut cultivation = cultivation_with_qi(2.0);
            let life_record = life_record("Mira");
            let mut ledger = WorldQiAccount::default();

            let error = release_qi_amount_to_zone(
                &mut cultivation,
                3.0,
                None,
                None,
                Some(&life_record),
                None,
                &mut ledger,
                None,
                "unit-insufficient",
            )
            .expect_err("release cannot debit more than the actor owns");

            assert!(matches!(error, QiFlowError::InsufficientCurrent { .. }));
            assert_eq!(cultivation.qi_current, 2.0);
            assert_eq!(ledger.total(), 0.0);
            assert!(ledger.transfers().is_empty());
        }

        #[test]
        fn stable_ledger_failure_keeps_actor_zone_and_audit_unchanged() {
            let mut cultivation = cultivation_with_qi(5.0);
            let life_record = life_record("Mira");
            let mut zones = fresh_zones_with_qi(1.0);
            let mut ledger = WorldQiAccount::default();
            ledger
                .set_balance(qi_flow_overflow_account(), f64::MAX)
                .expect("finite sink fixture");
            let mut events = Events::<QiTransfer>::default();
            let position = pos_in_spawn_zone();
            let dimension = overworld_dim();

            let error = release_qi_amount_to_zone(
                &mut cultivation,
                5.0,
                Some(&position),
                Some(&dimension),
                Some(&life_record),
                Some(&mut zones),
                &mut ledger,
                Some(&mut events),
                "unit-ledger-failure",
            )
            .expect_err("overflow destination that cannot progress must fail closed");

            assert!(matches!(error, QiFlowError::Physics(_)));
            assert_eq!(cultivation.qi_current, 5.0);
            assert_eq!(
                zones
                    .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
                    .unwrap()
                    .spirit_qi,
                1.0
            );
            assert_eq!(ledger.balance(&qi_flow_overflow_account()), f64::MAX);
            assert!(ledger.transfers().is_empty());
            assert!(events.drain().next().is_none());
        }
    }
}
