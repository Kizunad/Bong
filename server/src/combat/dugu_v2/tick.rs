use valence::prelude::{
    Commands, Entity, EventReader, EventWriter, Events, Position, Query, Res, ResMut,
};

use crate::combat::components::TICKS_PER_SECOND;
use crate::combat::CombatClock;
use crate::cultivation::components::Cultivation;
use crate::cultivation::death_hooks::release_qi_amount_to_zone;
use crate::cultivation::life_record::LifeRecord;
use crate::qi_physics::constants::{QI_EPSILON, QI_ZONE_UNIT_CAPACITY};
use crate::qi_physics::ledger::{QiAccountId, QiTransfer, QiTransferReason, WorldQiAccount};
use crate::qi_physics::release::qi_release_to_zone;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::zone::ZoneRegistry;

use super::events::{EclipseNeedleEvent, PermanentQiMaxDecayApplied, ReverseTriggeredEvent};
use super::state::{ReverseAftermathCloud, ShroudActive, TaintMark};

pub fn taint_decay_tick(
    mut commands: Commands,
    clock: Res<CombatClock>,
    mut targets: Query<(Entity, &mut Cultivation, &TaintMark)>,
) {
    for (entity, mut cultivation, mark) in &mut targets {
        if mark
            .expires_at_tick
            .is_some_and(|expires| clock.tick >= expires)
        {
            if mark.temporary_qi_max_loss > 0.0 {
                cultivation.qi_max += f64::from(mark.temporary_qi_max_loss);
            }
            commands.entity(entity).remove::<TaintMark>();
        }
    }
}

pub fn permanent_qi_max_decay_tick(
    clock: Res<CombatClock>,
    mut targets: Query<(Entity, &mut Cultivation, &TaintMark)>,
    mut events: EventWriter<PermanentQiMaxDecayApplied>,
) {
    for (entity, mut cultivation, mark) in &mut targets {
        if !mark.is_permanent() || mark.permanent_decay_rate_per_min <= 0.0 {
            continue;
        }
        let per_tick =
            f64::from(mark.permanent_decay_rate_per_min) / 60.0 / TICKS_PER_SECOND as f64;
        let loss = (cultivation.qi_max * per_tick).max(0.0);
        if loss <= f64::EPSILON {
            continue;
        }
        cultivation.qi_max = (cultivation.qi_max - loss).max(0.0);
        cultivation.qi_current = cultivation.qi_current.min(cultivation.qi_max);
        events.send(PermanentQiMaxDecayApplied {
            target: entity,
            caster: mark.caster,
            loss: loss as f32,
            qi_max_after: cultivation.qi_max as f32,
            tick: clock.tick,
        });
    }
}

pub fn shroud_maintain_tick(
    mut commands: Commands,
    clock: Res<CombatClock>,
    mut actors: Query<(
        Entity,
        &mut Cultivation,
        &ShroudActive,
        Option<&Position>,
        Option<&CurrentDimension>,
        Option<&LifeRecord>,
    )>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut qi_transfers: Option<ResMut<Events<QiTransfer>>>,
) {
    for (entity, mut cultivation, shroud, position, current_dimension, life_record) in &mut actors {
        if !shroud.permanent_until_cancelled && clock.tick >= shroud.expires_at_tick {
            commands.entity(entity).remove::<ShroudActive>();
            continue;
        }
        if cultivation.qi_current <= shroud.maintain_qi_per_tick {
            commands.entity(entity).remove::<ShroudActive>();
            continue;
        }
        let drained = shroud.maintain_qi_per_tick;
        cultivation.qi_current = (cultivation.qi_current - drained).clamp(0.0, cultivation.qi_max);
        release_qi_amount_to_zone(
            entity,
            drained,
            position,
            current_dimension,
            life_record,
            zones.as_deref_mut(),
            qi_transfers.as_deref_mut(),
            "dugu_v2:shroud_maintain",
        );
    }
}

pub fn reverse_aftermath_decay_tick(
    mut commands: Commands,
    clock: Res<CombatClock>,
    clouds: Query<(Entity, &ReverseAftermathCloud)>,
) {
    for (entity, cloud) in &clouds {
        if clock.tick >= cloud.expires_at_tick {
            commands.entity(entity).remove::<ReverseAftermathCloud>();
        }
    }
}

/// plan-qi-conservation-leaks-v1 P4 — EclipseNeedleEvent `returned_zone_qi` 入账到受害者所在 zone。
///
/// 脏真元过渡态（排斥部分）立即散回受害者脚下的 zone。
/// worldview §424-426 正典：脏真元注入『敌人体内』，被排斥后从受害者体内飞出，
/// 落到**受害者**所在 zone（而非施法者所在 zone）。
///
/// **守恒约束**：
///   - 通过 `qi_release_to_zone` 做 absolute→normalized 换算，overflow 入账到 overflow account；
///   - push audit-only QiTransfer(from=player:<caster>, to=zone:<name>, DuguReturnToZone)；
///   - 不调 WorldQiAccount::transfer（player qi 活在 ECS，不在 ledger balances）；
///   - returned_zone_qi == 0 时静默跳过，不产生噪音审计记录。
///   - ZoneRegistry 缺失（单测场景）时静默跳过。
pub fn eclipse_zone_credit_tick(
    mut events: EventReader<EclipseNeedleEvent>,
    entity_positions: Query<(Entity, &Position, Option<&CurrentDimension>)>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut qi_account: Option<ResMut<WorldQiAccount>>,
) {
    let Some(ref mut zones) = zones else {
        // 测试环境未 insert ZoneRegistry 时静默消费事件，不 panic
        for _ in events.read() {}
        return;
    };
    for event in events.read() {
        let returned = f64::from(event.returned_zone_qi);
        if returned <= 0.0 {
            continue;
        }
        // 脏真元从受害者体内散逸 → 入账受害者所在 zone（对齐 worldview §424-426）
        let Some((pos, dim)) = entity_positions.get(event.target).ok().map(|(_, p, d)| {
            (
                p.get(),
                d.map(|cd| cd.0).unwrap_or(DimensionKind::Overworld),
            )
        }) else {
            continue;
        };
        let Some(zone) = zones.find_zone_mut_by_pos(dim, pos) else {
            continue;
        };
        let zone_name = zone.name.clone();
        let from = QiAccountId::player(format!("entity:{:?}", event.caster));
        let to = QiAccountId::zone(zone_name.clone());
        // MF3 fix: convert absolute→normalized via qi_release_to_zone (overflow never dropped)
        let zone_current = zone.spirit_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY;
        match qi_release_to_zone(
            returned,
            from.clone(),
            to,
            zone_current,
            QI_ZONE_UNIT_CAPACITY,
        ) {
            Ok(outcome) => {
                zone.spirit_qi = (outcome.zone_after / QI_ZONE_UNIT_CAPACITY).clamp(-1.0, 1.0);
                // audit trail: DuguReturnToZone（区别于 qi_release_to_zone 产出的 ReleaseToZone）
                if let Some(ref mut account) = qi_account {
                    account.push_transfer_audit(QiTransfer {
                        from: from.clone(),
                        to: QiAccountId::zone(zone_name.clone()),
                        amount: outcome.accepted,
                        reason: QiTransferReason::DuguReturnToZone,
                    });
                    // overflow sink: qi 绝不蒸发
                    if outcome.overflow > QI_EPSILON {
                        let overflow_to = QiAccountId::overflow(format!(
                            "dugu_eclipse_overflow:entity:{:?}",
                            event.caster
                        ));
                        if let Ok(t) = QiTransfer::new(
                            from,
                            overflow_to,
                            outcome.overflow,
                            QiTransferReason::DuguReturnToZone,
                        ) {
                            account.push_transfer_audit(t);
                        }
                    }
                }
            }
            Err(err) => {
                tracing::warn!(
                    ?err,
                    "[bong][dugu_v2] eclipse_zone_credit invalid qi release; routing to overflow"
                );
                if let Some(ref mut account) = qi_account {
                    let overflow_to = QiAccountId::overflow(format!(
                        "dugu_eclipse_err_overflow:entity:{:?}",
                        event.caster
                    ));
                    if let Ok(t) = QiTransfer::new(
                        from,
                        overflow_to,
                        returned,
                        QiTransferReason::DuguReturnToZone,
                    ) {
                        account.push_transfer_audit(t);
                    }
                }
            }
        }
    }
}

/// plan-qi-conservation-leaks-v1 P4 — ReverseTriggeredEvent `returned_zone_qi` 入账到受害者所在 zone。
///
/// Reverse（倒蚀）爆发后寄生残留从受害者体内散逸，落到**受害者（event.center）**所在 zone。
/// worldview §424-426 正典：脏真元从受害者体内飞出，应记到受害者脚下 zone。
/// event.center 已是 target 坐标（skills.rs apply_reverse），可直接用于 zone 查找。
/// 维度取施法者 CurrentDimension（双方应在同一维度才能施法，center 坐标与施法者维度一致）。
/// ZoneRegistry 缺失（单测场景）时静默跳过。
pub fn reverse_zone_credit_tick(
    mut events: EventReader<ReverseTriggeredEvent>,
    caster_dims: Query<(Entity, Option<&CurrentDimension>)>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut qi_account: Option<ResMut<WorldQiAccount>>,
) {
    let Some(ref mut zones) = zones else {
        for _ in events.read() {}
        return;
    };
    for event in events.read() {
        let returned = f64::from(event.returned_zone_qi);
        if returned <= 0.0 {
            continue;
        }
        // event.center 是受害者位置（apply_reverse 设为 target 坐标）
        let pos = event.center;
        // 取施法者维度（施法者与受害者必须在同一维度才能施法）
        let dim = caster_dims
            .get(event.caster)
            .ok()
            .and_then(|(_, d)| d)
            .map(|cd| cd.0)
            .unwrap_or(DimensionKind::Overworld);
        let Some(zone) = zones.find_zone_mut_by_pos(dim, pos) else {
            continue;
        };
        let zone_name = zone.name.clone();
        let from = QiAccountId::player(format!("entity:{:?}", event.caster));
        let to = QiAccountId::zone(zone_name.clone());
        // MF3 fix: convert absolute→normalized via qi_release_to_zone (overflow never dropped)
        let zone_current = zone.spirit_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY;
        match qi_release_to_zone(
            returned,
            from.clone(),
            to,
            zone_current,
            QI_ZONE_UNIT_CAPACITY,
        ) {
            Ok(outcome) => {
                zone.spirit_qi = (outcome.zone_after / QI_ZONE_UNIT_CAPACITY).clamp(-1.0, 1.0);
                // audit trail: DuguReturnToZone（区别于 qi_release_to_zone 产出的 ReleaseToZone）
                if let Some(ref mut account) = qi_account {
                    account.push_transfer_audit(QiTransfer {
                        from: from.clone(),
                        to: QiAccountId::zone(zone_name.clone()),
                        amount: outcome.accepted,
                        reason: QiTransferReason::DuguReturnToZone,
                    });
                    // overflow sink: qi 绝不蒸发
                    if outcome.overflow > QI_EPSILON {
                        let overflow_to = QiAccountId::overflow(format!(
                            "dugu_reverse_overflow:entity:{:?}",
                            event.caster
                        ));
                        if let Ok(t) = QiTransfer::new(
                            from,
                            overflow_to,
                            outcome.overflow,
                            QiTransferReason::DuguReturnToZone,
                        ) {
                            account.push_transfer_audit(t);
                        }
                    }
                }
            }
            Err(err) => {
                tracing::warn!(
                    ?err,
                    "[bong][dugu_v2] reverse_zone_credit invalid qi release; routing to overflow"
                );
                if let Some(ref mut account) = qi_account {
                    let overflow_to = QiAccountId::overflow(format!(
                        "dugu_reverse_err_overflow:entity:{:?}",
                        event.caster
                    ));
                    if let Ok(t) = QiTransfer::new(
                        from,
                        overflow_to,
                        returned,
                        QiTransferReason::DuguReturnToZone,
                    ) {
                        account.push_transfer_audit(t);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::combat::dugu_v2::events::DuguSkillId;
    use crate::combat::CombatClock;
    use crate::cultivation::components::{Cultivation, QiColor, Realm};
    use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
    use crate::world::dimension::{CurrentDimension, DimensionKind};
    use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};
    use valence::prelude::{App, Position, Update};

    fn make_shroud(expires_at_tick: u64, maintain_qi_per_tick: f64) -> ShroudActive {
        ShroudActive {
            skill: DuguSkillId::Shroud,
            strength: 0.5,
            fake_qi_color: QiColor::default(),
            started_at_tick: 0,
            expires_at_tick,
            permanent_until_cancelled: false,
            maintain_qi_per_tick,
        }
    }

    fn setup_app_with_zone() -> App {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1 });
        app.insert_resource(ZoneRegistry::fallback());
        app.add_event::<QiTransfer>();
        // Empty the spawn zone so the full drain amount fits without overflow split
        app.world_mut()
            .resource_mut::<ZoneRegistry>()
            .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
            .unwrap()
            .spirit_qi = 0.0;
        app.add_systems(Update, shroud_maintain_tick);
        app
    }

    /// Happy path: Shroud active, entity has Position + CurrentDimension → qi drains from player
    /// and the same amount is credited into the spawn zone. Conservation holds.
    #[test]
    fn shroud_maintain_tick_credits_drained_qi_to_zone() {
        let mut app = setup_app_with_zone();

        let maintain = 0.5 / TICKS_PER_SECOND as f64; // 0.025
        let initial_qi = 10.0_f64;
        let entity = app
            .world_mut()
            .spawn((
                Cultivation {
                    qi_current: initial_qi,
                    qi_max: 100.0,
                    realm: Realm::Awaken,
                    ..Default::default()
                },
                make_shroud(1000, maintain),
                Position::new([0.0, 64.0, 0.0]),
                CurrentDimension(DimensionKind::Overworld),
            ))
            .id();

        let zone_before = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .unwrap()
            .spirit_qi;

        app.update();

        // Player qi should have decreased by maintain_qi_per_tick
        let cultivation = app.world().entity(entity).get::<Cultivation>().unwrap();
        assert!(
            (cultivation.qi_current - (initial_qi - maintain)).abs() < 1e-10,
            "qi_current should decrease by maintain_qi_per_tick={maintain}; got {}",
            cultivation.qi_current
        );

        // Zone should have received the drained amount (zone started empty so no overflow split)
        let zone_after = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .unwrap()
            .spirit_qi;
        let zone_delta_abs = (zone_after - zone_before) * QI_ZONE_UNIT_CAPACITY;
        assert!(
            (zone_delta_abs - maintain).abs() < 1e-10,
            "zone must receive exactly maintain_qi_per_tick={maintain} qi; got delta={zone_delta_abs}"
        );

        // A QiTransfer event must be emitted
        let transfers: Vec<_> = app
            .world()
            .resource::<Events<QiTransfer>>()
            .iter_current_update_events()
            .cloned()
            .collect();
        assert_eq!(
            transfers.len(),
            1,
            "exactly one QiTransfer event should be emitted; got {}",
            transfers.len()
        );
        assert!(
            (transfers[0].amount - maintain).abs() < 1e-10,
            "QiTransfer amount should equal maintain_qi_per_tick; got {}",
            transfers[0].amount
        );
    }

    /// Boundary: when Shroud expires (clock.tick >= expires_at_tick), ShroudActive is removed
    /// and no qi is drained.
    #[test]
    fn shroud_maintain_tick_removes_shroud_on_expiry() {
        let mut app = setup_app_with_zone();

        let initial_qi = 10.0_f64;
        let maintain = 0.5 / TICKS_PER_SECOND as f64;
        let entity = app
            .world_mut()
            .spawn((
                Cultivation {
                    qi_current: initial_qi,
                    qi_max: 100.0,
                    realm: Realm::Awaken,
                    ..Default::default()
                },
                // expires_at_tick=1, CombatClock.tick=1 → should expire this tick
                make_shroud(1, maintain),
                Position::new([0.0, 64.0, 0.0]),
                CurrentDimension(DimensionKind::Overworld),
            ))
            .id();

        app.update();

        // ShroudActive should be removed
        assert!(
            app.world().entity(entity).get::<ShroudActive>().is_none(),
            "ShroudActive should be removed when clock.tick >= expires_at_tick"
        );
        // No qi should be drained when expired
        let cultivation = app.world().entity(entity).get::<Cultivation>().unwrap();
        assert!(
            (cultivation.qi_current - initial_qi).abs() < 1e-10,
            "qi_current should be unchanged when Shroud expires; got {}",
            cultivation.qi_current
        );
        // No QiTransfer event
        let transfers: Vec<_> = app
            .world()
            .resource::<Events<QiTransfer>>()
            .iter_current_update_events()
            .cloned()
            .collect();
        assert_eq!(
            transfers.len(),
            0,
            "no QiTransfer should be emitted on expiry; got {}",
            transfers.len()
        );
    }

    /// Boundary: when qi_current <= maintain_qi_per_tick, Shroud collapses (removed) and
    /// no qi drain happens (conserved; we don't drain the last sliver).
    #[test]
    fn shroud_maintain_tick_collapses_when_insufficient_qi() {
        let mut app = setup_app_with_zone();

        let maintain = 0.5 / TICKS_PER_SECOND as f64; // 0.025
        let too_low_qi = maintain; // exactly at the boundary → should collapse
        let entity = app
            .world_mut()
            .spawn((
                Cultivation {
                    qi_current: too_low_qi,
                    qi_max: 100.0,
                    realm: Realm::Awaken,
                    ..Default::default()
                },
                make_shroud(1000, maintain),
                Position::new([0.0, 64.0, 0.0]),
                CurrentDimension(DimensionKind::Overworld),
            ))
            .id();

        app.update();

        // ShroudActive should collapse when qi_current <= maintain_qi_per_tick
        assert!(
            app.world().entity(entity).get::<ShroudActive>().is_none(),
            "ShroudActive should collapse when qi_current <= maintain_qi_per_tick"
        );
        // qi is NOT drained in this branch (collapse, not drain)
        let cultivation = app.world().entity(entity).get::<Cultivation>().unwrap();
        assert!(
            (cultivation.qi_current - too_low_qi).abs() < 1e-10,
            "qi_current should be unchanged on collapse; got {}",
            cultivation.qi_current
        );
    }

    /// Error branch: no ZoneRegistry inserted → drain still happens (qi leaves player),
    /// but qi routes to overflow (no zone credit). The function must not panic.
    #[test]
    fn shroud_maintain_tick_drains_qi_without_zone_registry() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1 });
        // Intentionally do NOT insert ZoneRegistry
        app.add_event::<QiTransfer>();
        app.add_systems(Update, shroud_maintain_tick);

        let maintain = 0.5 / TICKS_PER_SECOND as f64;
        let initial_qi = 10.0_f64;
        let entity = app
            .world_mut()
            .spawn((
                Cultivation {
                    qi_current: initial_qi,
                    qi_max: 100.0,
                    realm: Realm::Awaken,
                    ..Default::default()
                },
                make_shroud(1000, maintain),
                Position::new([0.0, 64.0, 0.0]),
                CurrentDimension(DimensionKind::Overworld),
            ))
            .id();

        app.update(); // must not panic

        // Player qi is still drained (the drain itself is unconditional; only zone credit differs)
        let cultivation = app.world().entity(entity).get::<Cultivation>().unwrap();
        assert!(
            (cultivation.qi_current - (initial_qi - maintain)).abs() < 1e-10,
            "qi_current should decrease even without ZoneRegistry; got {}",
            cultivation.qi_current
        );
    }

    /// Error branch: no Position on entity → qi drains but routes to overflow (no zone credit).
    /// Must not panic.
    #[test]
    fn shroud_maintain_tick_drains_qi_without_position() {
        let mut app = setup_app_with_zone();

        let maintain = 0.5 / TICKS_PER_SECOND as f64;
        let initial_qi = 10.0_f64;
        let entity = app
            .world_mut()
            .spawn((
                Cultivation {
                    qi_current: initial_qi,
                    qi_max: 100.0,
                    realm: Realm::Awaken,
                    ..Default::default()
                },
                make_shroud(1000, maintain),
                // No Position, no CurrentDimension
            ))
            .id();

        app.update(); // must not panic

        let cultivation = app.world().entity(entity).get::<Cultivation>().unwrap();
        assert!(
            (cultivation.qi_current - (initial_qi - maintain)).abs() < 1e-10,
            "qi_current should decrease even without Position; got {}",
            cultivation.qi_current
        );
        // Zone should NOT have received any credit (overflow route taken instead)
        let zone_after = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .unwrap()
            .spirit_qi;
        assert!(
            zone_after.abs() < 1e-10,
            "zone should not receive qi when entity has no Position; spirit_qi={}",
            zone_after
        );
    }
}
