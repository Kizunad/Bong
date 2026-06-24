use valence::prelude::{Commands, Entity, EventReader, EventWriter, Position, Query, Res, ResMut};

use crate::combat::components::TICKS_PER_SECOND;
use crate::combat::CombatClock;
use crate::cultivation::components::Cultivation;
use crate::qi_physics::constants::{QI_EPSILON, QI_ZONE_UNIT_CAPACITY};
use crate::qi_physics::ledger::{QiAccountId, QiTransfer, QiTransferReason, WorldQiAccount};
use crate::qi_physics::release::qi_release_to_zone;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::zone::ZoneRegistry;

use super::events::{
    DuguReverseVictimQiEvent, EclipseNeedleEvent, PermanentQiMaxDecayApplied,
    ReverseTriggeredEvent,
};
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
    mut actors: Query<(Entity, &mut Cultivation, &ShroudActive)>,
) {
    for (entity, mut cultivation, shroud) in &mut actors {
        if !shroud.permanent_until_cancelled && clock.tick >= shroud.expires_at_tick {
            commands.entity(entity).remove::<ShroudActive>();
            continue;
        }
        if cultivation.qi_current <= shroud.maintain_qi_per_tick {
            commands.entity(entity).remove::<ShroudActive>();
            continue;
        }
        cultivation.qi_current =
            (cultivation.qi_current - shroud.maintain_qi_per_tick).clamp(0.0, cultivation.qi_max);
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

/// bughunt r8 — Reverse（倒蚀）清零受害者 qi_current 时，被消灭的真元守恒归还受害者所在 zone。
///
/// DuguReverseVictimQiEvent 由 skills.rs apply_reverse() 在循环结束后发送，携带所有受害者
/// qi_current 累加总量。此系统与 reverse_zone_credit_tick（处理脏真元残留）正交：
///   - reverse_zone_credit_tick：returned_zone_qi = taint_intensity × ratio（小值）
///   - 本系统：victim_qi_total = 受害者实际 qi_current 清零量（可为大值，如灵境目标 200）
///
/// 维度取施法者 CurrentDimension（缺失时 fallback Overworld），zone 查找用 event.center 坐标。
pub fn reverse_victim_qi_zone_credit_tick(
    mut events: EventReader<DuguReverseVictimQiEvent>,
    caster_dims: Query<(Entity, Option<&CurrentDimension>)>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut qi_account: Option<ResMut<WorldQiAccount>>,
) {
    let Some(ref mut zones) = zones else {
        for _ in events.read() {}
        return;
    };
    for event in events.read() {
        let victim_qi = f64::from(event.victim_qi_total);
        if victim_qi <= 0.0 {
            continue;
        }
        let pos = event.center;
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
        let zone_current = zone.spirit_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY;
        match qi_release_to_zone(victim_qi, from.clone(), to, zone_current, QI_ZONE_UNIT_CAPACITY) {
            Ok(outcome) => {
                zone.spirit_qi = (outcome.zone_after / QI_ZONE_UNIT_CAPACITY).clamp(-1.0, 1.0);
                if let Some(ref mut account) = qi_account {
                    account.push_transfer_audit(QiTransfer {
                        from: from.clone(),
                        to: QiAccountId::zone(zone_name.clone()),
                        amount: outcome.accepted,
                        reason: QiTransferReason::DuguReverseVictimQi,
                    });
                    if outcome.overflow > QI_EPSILON {
                        let overflow_to = QiAccountId::overflow(format!(
                            "dugu_reverse_victim_overflow:entity:{:?}",
                            event.caster
                        ));
                        if let Ok(t) = QiTransfer::new(
                            from,
                            overflow_to,
                            outcome.overflow,
                            QiTransferReason::DuguReverseVictimQi,
                        ) {
                            account.push_transfer_audit(t);
                        }
                    }
                }
            }
            Err(err) => {
                tracing::warn!(
                    ?err,
                    "[bong][dugu_v2] reverse_victim_qi_zone_credit invalid qi release; routing to overflow"
                );
                if let Some(ref mut account) = qi_account {
                    let overflow_to = QiAccountId::overflow(format!(
                        "dugu_reverse_victim_err_overflow:entity:{:?}",
                        event.caster
                    ));
                    if let Ok(t) = QiTransfer::new(
                        from,
                        overflow_to,
                        victim_qi,
                        QiTransferReason::DuguReverseVictimQi,
                    ) {
                        account.push_transfer_audit(t);
                    }
                }
            }
        }
    }
}
