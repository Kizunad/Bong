//! NegativeZoneSiphonTick（plan §2.1）— 负灵域反吸玩家真元/血肉。
//!
//! 当 zone.spirit_qi < 0（负灵域定义来自 worldview §二）：
//!   * `siphon = |zone| × qi_max × SIPHON_FACTOR`
//!   * 优先从 qi 扣；qi=0 后从 `Health` 扣（战斗 plan 管辖，本 plan 产出事件）
//!   * `Health <= 0` → emit `CultivationDeathTrigger::NegativeZoneDrain`

use valence::prelude::{Entity, EventWriter, Events, Position, Query, ResMut};

use crate::world::dimension::CurrentDimension;
use crate::world::zone::ZoneRegistry;

use super::components::{Cultivation, QiFlowError};
use super::death_hooks::{
    release_qi_amount_to_zone, CultivationDeathCause, CultivationDeathTrigger,
};
use crate::cultivation::life_record::LifeRecord;
use crate::qi_physics::{subtraction_makes_progress, QiTransfer, WorldQiAccount};

pub const SIPHON_FACTOR: f64 = 0.001;

/// 纯函数：根据 zone 浓度 + qi_max 计算本 tick siphon 量（负值 zone 才有值）。
pub fn siphon_amount(zone_qi: f64, qi_max: f64) -> f64 {
    if zone_qi >= 0.0 {
        return 0.0;
    }
    let pressure = -zone_qi;
    pressure * qi_max * SIPHON_FACTOR
}

#[allow(clippy::type_complexity)]
pub fn negative_zone_siphon_tick(
    zones: Option<ResMut<ZoneRegistry>>,
    mut ledger: ResMut<WorldQiAccount>,
    mut deaths: EventWriter<CultivationDeathTrigger>,
    mut qi_transfers: Option<ResMut<Events<QiTransfer>>>,
    mut players: Query<(
        Entity,
        &Position,
        Option<&CurrentDimension>,
        Option<&LifeRecord>,
        &mut Cultivation,
    )>,
) {
    let Some(mut zones) = zones else {
        return;
    };
    for (entity, pos, current_dimension, life_record, mut cultivation) in players.iter_mut() {
        let Some(dimension) = current_dimension.map(|current| current.0) else {
            continue;
        };
        let zone_name = zones
            .find_zone(dimension, pos.0)
            .map(|z| (z.name.clone(), z.spirit_qi));
        let Some((zone_name, zone_qi)) = zone_name else {
            continue;
        };
        let siphon = siphon_amount(zone_qi, cultivation.qi_max);
        if siphon <= 0.0 {
            continue;
        }
        match subtraction_makes_progress(cultivation.qi_current, siphon) {
            Ok(true) => {}
            Ok(false) => {
                // 该笔 siphon 无法改变来源字段的 f64 表示；跳过才是守恒中性的 no-op。
                continue;
            }
            Err(error) => {
                tracing::warn!(
                    ?error,
                    "[bong][cultivation] negative-zone qi siphon rejected before release"
                );
                continue;
            }
        }
        if cultivation.qi_current >= siphon {
            let outcome = release_qi_amount_to_zone(
                &mut cultivation,
                siphon,
                Some(pos),
                current_dimension,
                life_record,
                Some(&mut *zones),
                &mut ledger,
                qi_transfers.as_deref_mut(),
                "negative_zone_siphon",
            );
            if let Err(error) = outcome {
                tracing::warn!(
                    ?error,
                    "[bong][cultivation] negative-zone qi release failed closed"
                );
            }
            continue;
        }
        // qi 吸干，转抽血肉：本 plan 不持 Health Component，发事件由战斗 plan 消费。
        // 作为最低保障：qi_current 归零，并若尚无命脉收口，直接报死亡触发。
        let drained = cultivation.qi_current.max(0.0);
        match release_qi_amount_to_zone(
            &mut cultivation,
            drained,
            Some(pos),
            current_dimension,
            life_record,
            Some(&mut *zones),
            &mut ledger,
            qi_transfers.as_deref_mut(),
            "negative_zone_siphon",
        ) {
            Ok(_) => {
                deaths.send(CultivationDeathTrigger {
                    entity,
                    cause: CultivationDeathCause::NegativeZoneDrain,
                    context: serde_json::json!({
                        "zone": zone_name,
                        "siphon": siphon,
                    }),
                });
            }
            Err(error @ QiFlowError::UnrepresentableFlow {
                field: "zone.spirit_qi",
                ..
            }) => {
                // signed zone 的增量若小于其 f64 ULP，原 release 会原子失败；把同一笔
                // drained 真元转入稳定 overflow，完成真实入账后才允许发死亡触发。
                tracing::warn!(
                    ?error,
                    "[bong][cultivation] negative-zone zone release unrepresentable; retrying overflow"
                );
                match release_qi_amount_to_zone(
                    &mut cultivation,
                    drained,
                    None,
                    None,
                    life_record,
                    None,
                    &mut ledger,
                    qi_transfers.as_deref_mut(),
                    "negative_zone_siphon_overflow_fallback",
                ) {
                    Ok(_) => {
                        deaths.send(CultivationDeathTrigger {
                            entity,
                            cause: CultivationDeathCause::NegativeZoneDrain,
                            context: serde_json::json!({
                                "zone": zone_name,
                                "siphon": siphon,
                            }),
                        });
                    }
                    Err(fallback_error) => {
                        tracing::warn!(
                            ?fallback_error,
                            "[bong][cultivation] negative-zone overflow fallback failed closed"
                        );
                    }
                }
            }
            Err(error) => {
                tracing::warn!(
                    ?error,
                    "[bong][cultivation] negative-zone qi release failed closed"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::state::canonical_player_id;
    use crate::qi_physics::{
        assert_conservation, qi_flow_overflow_account, summarize_world_qi, QiAccountId,
        WorldQiBudget,
    };
    use crate::qi_physics::constants::{DEFAULT_SPIRIT_QI_TOTAL, QI_ZONE_UNIT_CAPACITY};
    use crate::world::dimension::{CurrentDimension, DimensionKind};
    use valence::prelude::{App, Events, Update};

    fn app_with_player(zone_qi: f64, qi_current: f64, qi_max: f64) -> (App, Entity) {
        let mut app = App::new();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<QiTransfer>();
        let mut zones = ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = zone_qi;
        app.insert_resource(zones);
        app.insert_resource(WorldQiAccount::default());
        app.insert_resource(WorldQiBudget::from_total(DEFAULT_SPIRIT_QI_TOTAL));
        app.add_systems(Update, negative_zone_siphon_tick);

        let player = app
            .world_mut()
            .spawn((
                Position::new([8.0, 66.0, 8.0]),
                CurrentDimension(DimensionKind::Overworld),
                Cultivation {
                    qi_current,
                    qi_max,
                    ..Default::default()
                },
                LifeRecord::new(canonical_player_id("Azure")),
            ))
            .id();
        (app, player)
    }

    #[test]
    fn positive_zone_no_siphon() {
        assert_eq!(siphon_amount(0.5, 100.0), 0.0);
        assert_eq!(siphon_amount(0.0, 100.0), 0.0);
    }

    #[test]
    fn negative_zone_siphon_scales_with_qi_max() {
        let a = siphon_amount(-0.5, 100.0);
        let b = siphon_amount(-0.5, 200.0);
        assert!(b > a);
        assert!((b - 2.0 * a).abs() < 1e-9);
    }

    #[test]
    fn negative_zone_siphon_uses_current_dimension_for_zone_lookup() {
        let mut app = App::new();
        app.add_event::<CultivationDeathTrigger>();
        let mut zones = ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = -0.5;
        app.insert_resource(zones);
        app.insert_resource(WorldQiAccount::default());
        app.add_systems(Update, negative_zone_siphon_tick);

        let player = app
            .world_mut()
            .spawn((
                Position::new([8.0, 66.0, 8.0]),
                CurrentDimension(DimensionKind::Tsy),
                Cultivation {
                    qi_current: 1.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                LifeRecord::new(canonical_player_id("Azure")),
            ))
            .id();

        app.update();

        let cultivation = app.world().entity(player).get::<Cultivation>().unwrap();
        assert_eq!(cultivation.qi_current, 1.0);
        let deaths: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<CultivationDeathTrigger>>()
            .drain()
            .collect();
        assert!(deaths.is_empty());
    }

    #[test]
    fn negative_zone_siphon_skips_entity_without_current_dimension() {
        let mut app = App::new();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<QiTransfer>();
        let mut zones = ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = -0.5;
        app.insert_resource(zones);
        app.insert_resource(WorldQiAccount::default());
        app.add_systems(Update, negative_zone_siphon_tick);

        let player = app
            .world_mut()
            .spawn((
                Position::new([8.0, 66.0, 8.0]),
                Cultivation {
                    qi_current: 1.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                LifeRecord::new(canonical_player_id("Azure")),
            ))
            .id();

        app.update();

        let cultivation = app.world().entity(player).get::<Cultivation>().unwrap();
        assert_eq!(cultivation.qi_current, 1.0);
        assert_eq!(
            app.world()
                .resource::<Events<CultivationDeathTrigger>>()
                .len(),
            0
        );
        assert_eq!(app.world().resource::<Events<QiTransfer>>().len(), 0);
    }

    #[test]
    fn negative_zone_siphon_records_qi_transfer_to_zone() {
        let mut app = App::new();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<QiTransfer>();
        let mut zones = ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = -0.5;
        app.insert_resource(zones);
        app.insert_resource(WorldQiAccount::default());
        app.insert_resource(WorldQiBudget::from_total(DEFAULT_SPIRIT_QI_TOTAL));
        app.add_systems(Update, negative_zone_siphon_tick);
        let player = app
            .world_mut()
            .spawn((
                Position::new([8.0, 66.0, 8.0]),
                CurrentDimension(DimensionKind::Overworld),
                Cultivation {
                    qi_current: 1.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                LifeRecord::new(canonical_player_id("Azure")),
            ))
            .id();

        let before = summarize_world_qi(app.world_mut());
        app.update();
        let after = summarize_world_qi(app.world_mut());

        let cultivation = app.world().entity(player).get::<Cultivation>().unwrap();
        assert!(cultivation.qi_current < 1.0);
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
        assert_conservation(&before, &after, 0.0)
            .expect("normal negative-zone siphon must conserve player and signed-zone qi");
    }

    #[test]
    fn micro_negative_zone_siphon_is_a_repeated_noop_when_source_cannot_progress() {
        let (mut app, player) = app_with_player(-1e-16, 1.0, 100.0);
        let before = summarize_world_qi(app.world_mut());

        for _ in 0..8 {
            app.update();
        }

        let after = summarize_world_qi(app.world_mut());
        let cultivation = app.world().entity(player).get::<Cultivation>().unwrap();
        assert_eq!(cultivation.qi_current, 1.0);
        assert_eq!(
            app.world()
                .resource::<ZoneRegistry>()
                .find_zone_by_name("spawn")
                .unwrap()
                .spirit_qi,
            -1e-16
        );
        assert!(app
            .world()
            .resource::<WorldQiAccount>()
            .transfers()
            .is_empty());
        assert_eq!(
            app.world()
                .resource::<Events<QiTransfer>>()
                .len(),
            0,
            "unrepresentable epsilon siphon must not emit retry transfers"
        );
        assert_eq!(
            app.world()
                .resource::<Events<CultivationDeathTrigger>>()
                .len(),
            0
        );
        assert_conservation(&before, &after, 0.0)
            .expect("an unrepresentable epsilon siphon must be conservation-neutral");
    }

    #[test]
    fn non_negative_zone_siphon_is_a_noop() {
        for zone_qi in [0.0, 0.5] {
            let (mut app, player) = app_with_player(zone_qi, 1.0, 100.0);
            app.update();

            assert_eq!(
                app.world().entity(player).get::<Cultivation>().unwrap().qi_current,
                1.0
            );
            assert!(app
                .world()
                .resource::<WorldQiAccount>()
                .transfers()
                .is_empty());
            assert_eq!(
                app.world()
                    .resource::<Events<QiTransfer>>()
                    .len(),
                0
            );
            assert_eq!(
                app.world()
                    .resource::<Events<CultivationDeathTrigger>>()
                    .len(),
                0
            );
        }
    }

    #[test]
    fn qi_current_equal_to_siphon_releases_and_does_not_emit_death() {
        let siphon = siphon_amount(-0.5, 100.0);
        let (mut app, player) = app_with_player(-0.5, siphon, 100.0);

        app.update();

        assert_eq!(
            app.world().entity(player).get::<Cultivation>().unwrap().qi_current,
            0.0
        );
        let zone_after = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name("spawn")
            .unwrap()
            .spirit_qi;
        assert!(
            (zone_after - (-0.5 + siphon / QI_ZONE_UNIT_CAPACITY)).abs() < f64::EPSILON,
            "equal-boundary siphon must be credited to the signed zone, actual {zone_after}"
        );
        assert_eq!(
            app.world()
                .resource::<Events<CultivationDeathTrigger>>()
                .len(),
            0,
            "exactly-equal qi must take the normal release branch, not the drain branch"
        );
        assert_eq!(app.world().resource::<Events<QiTransfer>>().len(), 1);
    }

    #[test]
    fn qi_drain_release_emits_death_after_successful_settlement() {
        let (mut app, player) = app_with_player(-1.0, 0.05, 100.0);
        let before = summarize_world_qi(app.world_mut());

        app.update();

        let after = summarize_world_qi(app.world_mut());
        assert_eq!(
            app.world().entity(player).get::<Cultivation>().unwrap().qi_current,
            0.0
        );
        let deaths: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<CultivationDeathTrigger>>()
            .drain()
            .collect();
        assert_eq!(deaths.len(), 1);
        assert_eq!(deaths[0].entity, player);
        assert_eq!(deaths[0].cause, CultivationDeathCause::NegativeZoneDrain);
        assert_eq!(app.world().resource::<Events<QiTransfer>>().len(), 1);
        assert_conservation(&before, &after, 0.0)
            .expect("successful qi drain release must conserve player and signed-zone qi");
    }

    #[test]
    fn qi_drain_zone_precision_failure_falls_back_to_overflow_before_death() {
        let tiny = f64::MIN_POSITIVE;
        let (mut app, player) = app_with_player(-1.0, tiny, 100.0);
        let before = summarize_world_qi(app.world_mut());

        app.update();

        let after = summarize_world_qi(app.world_mut());
        assert_eq!(
            app.world().entity(player).get::<Cultivation>().unwrap().qi_current,
            0.0
        );
        assert_eq!(
            app.world()
                .resource::<ZoneRegistry>()
                .find_zone_by_name("spawn")
                .unwrap()
                .spirit_qi,
            -1.0
        );
        let overflow = app
            .world()
            .resource::<WorldQiAccount>()
            .balance(&qi_flow_overflow_account());
        assert_eq!(overflow, tiny);
        assert_eq!(app.world().resource::<Events<QiTransfer>>().len(), 1);
        let deaths: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<CultivationDeathTrigger>>()
            .drain()
            .collect();
        assert_eq!(deaths.len(), 1);
        assert_eq!(deaths[0].entity, player);
        assert_eq!(deaths[0].cause, CultivationDeathCause::NegativeZoneDrain);
        assert_conservation(&before, &after, 0.0)
            .expect("overflow fallback must conserve the unrepresentable drained qi");
    }

    #[test]
    fn non_numeric_drain_release_failure_remains_fail_closed_without_death() {
        let mut app = App::new();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<QiTransfer>();
        let mut zones = ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = -1.0;
        app.insert_resource(zones);
        app.insert_resource(WorldQiAccount::default());
        app.add_systems(Update, negative_zone_siphon_tick);
        let player = app
            .world_mut()
            .spawn((
                Position::new([8.0, 66.0, 8.0]),
                CurrentDimension(DimensionKind::Overworld),
                Cultivation {
                    qi_current: 0.05,
                    qi_max: 100.0,
                    ..Default::default()
                },
            ))
            .id();

        app.update();

        assert_eq!(
            app.world().entity(player).get::<Cultivation>().unwrap().qi_current,
            0.05
        );
        assert_eq!(
            app.world()
                .resource::<Events<CultivationDeathTrigger>>()
                .len(),
            0,
            "missing canonical identity must not be converted into a death trigger"
        );
        assert!(app
            .world()
            .resource::<WorldQiAccount>()
            .transfers()
            .is_empty());
    }
}
