//! 余烬死域 tick：零回复之外的自然真元挥发。

use valence::prelude::{bevy_ecs, Entity, Events, Position, Query, ResMut, Resource, Without};

use crate::npc::spawn::NpcMarker;
use crate::qi_physics::QiTransfer;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::zone::{Zone, ZoneRegistry};

use super::components::Cultivation;
use super::death_hooks::release_qi_amount_to_zone;
use super::life_record::LifeRecord;

pub const DEAD_ZONE_QI_THRESHOLD: f64 = 0.01;
pub const TICKS_PER_MINUTE: f64 = 20.0 * 60.0;

#[derive(Debug, Clone, Copy, Resource)]
pub struct DeadZoneTickHandler {
    pub qi_drain_per_minute: f64,
    pub shelflife_zone_multiplier: f32,
}

impl Default for DeadZoneTickHandler {
    fn default() -> Self {
        Self {
            qi_drain_per_minute: 1.0,
            shelflife_zone_multiplier: 3.0,
        }
    }
}

impl DeadZoneTickHandler {
    pub fn qi_drain_per_tick(self) -> f64 {
        (self.qi_drain_per_minute / TICKS_PER_MINUTE).max(0.0)
    }
}

pub fn is_dead_zone(zone: &Zone) -> bool {
    zone.spirit_qi >= 0.0 && zone.spirit_qi < DEAD_ZONE_QI_THRESHOLD
}

pub fn drain_for_ticks(handler: DeadZoneTickHandler, ticks: u64) -> f64 {
    handler.qi_drain_per_tick() * ticks as f64
}

pub fn apply_dead_zone_drain(cultivation: &mut Cultivation, handler: DeadZoneTickHandler) -> f64 {
    let drain = handler
        .qi_drain_per_tick()
        .min(cultivation.qi_current.max(0.0));
    cultivation.qi_current = (cultivation.qi_current - drain).max(0.0);
    drain
}

pub fn dead_zone_silent_qi_loss_tick(
    handler: Option<ResMut<DeadZoneTickHandler>>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut qi_transfers: Option<ResMut<Events<QiTransfer>>>,
    mut players: Query<
        (
            Entity,
            &Position,
            Option<&CurrentDimension>,
            Option<&LifeRecord>,
            &mut Cultivation,
        ),
        Without<NpcMarker>,
    >,
) {
    let handler = handler.as_deref().copied().unwrap_or_default();
    if handler.qi_drain_per_tick() <= 0.0 {
        return;
    }

    // Collect entities in dead zones first (immutable borrow), then apply drain.
    let mut to_drain: Vec<(Entity, f64)> = Vec::new();
    {
        let zones_ref = match zones.as_ref() {
            Some(z) => z,
            None => return,
        };
        for (entity, position, current_dim, _life_record, cultivation) in players.iter() {
            let dim = current_dim.map(|c| c.0).unwrap_or(DimensionKind::Overworld);
            let Some(zone) = zones_ref.find_zone(dim, position.0) else {
                continue;
            };
            if is_dead_zone(zone) {
                let drain = handler
                    .qi_drain_per_tick()
                    .min(cultivation.qi_current.max(0.0));
                if drain > 0.0 {
                    to_drain.push((entity, drain));
                }
            }
        }
    }

    for (entity, drain) in to_drain {
        let Ok((_, position, current_dim, life_record, mut cultivation)) = players.get_mut(entity)
        else {
            continue;
        };
        // Deduct from player qi first, then credit the zone for ledger balance.
        let actual_drain = drain.min(cultivation.qi_current.max(0.0));
        cultivation.qi_current = (cultivation.qi_current - actual_drain).max(0.0);
        release_qi_amount_to_zone(
            entity,
            actual_drain,
            Some(position),
            current_dim,
            life_record,
            zones.as_deref_mut(),
            qi_transfers.as_deref_mut(),
            "dead_zone_drain",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::Realm;
    use crate::player::state::canonical_player_id;
    use crate::qi_physics::{QiAccountId, QiTransfer, QiTransferReason};
    use crate::world::zone::ZoneRegistry;
    use valence::prelude::{App, DVec3, Events, Update};

    fn dead_zone(name: &str) -> Zone {
        Zone {
            name: name.to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (DVec3::new(0.0, 0.0, 0.0), DVec3::new(100.0, 100.0, 100.0)),
            spirit_qi: 0.0,
            danger_level: 5,
            active_events: vec!["no_cadence".to_string()],
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
        }
    }

    fn normal_zone(name: &str) -> Zone {
        Zone {
            name: name.to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (DVec3::new(200.0, 0.0, 0.0), DVec3::new(300.0, 100.0, 100.0)),
            spirit_qi: 0.4,
            danger_level: 1,
            active_events: vec![],
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
        }
    }

    #[test]
    fn dead_zone_predicate_excludes_negative_fields() {
        let zone_at = |spirit_qi: f64| Zone {
            name: "test".to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (DVec3::ZERO, DVec3::ONE),
            spirit_qi,
            danger_level: 0,
            active_events: vec![],
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
        };
        assert!(
            is_dead_zone(&zone_at(0.0)),
            "spirit_qi=0.0 should be dead zone"
        );
        assert!(
            is_dead_zone(&zone_at(0.009)),
            "spirit_qi=0.009 (just below threshold) should be dead zone"
        );
        assert!(
            !is_dead_zone(&zone_at(0.05)),
            "spirit_qi=0.05 (above threshold) should not be dead zone"
        );
        assert!(
            !is_dead_zone(&zone_at(-0.1)),
            "spirit_qi=-0.1 (negative field) should not be dead zone"
        );
        assert!(
            !is_dead_zone(&zone_at(DEAD_ZONE_QI_THRESHOLD)),
            "spirit_qi=DEAD_ZONE_QI_THRESHOLD (exact boundary, exclusive) should not be dead zone"
        );
    }

    #[test]
    fn sixty_seconds_drains_one_qi_for_all_realms() {
        let handler = DeadZoneTickHandler::default();
        for realm in [
            Realm::Awaken,
            Realm::Induce,
            Realm::Condense,
            Realm::Solidify,
            Realm::Spirit,
            Realm::Void,
        ] {
            let mut cultivation = Cultivation {
                realm,
                qi_current: 10.0,
                qi_max: 100.0,
                ..Cultivation::default()
            };
            let drain = drain_for_ticks(handler, TICKS_PER_MINUTE as u64);
            cultivation.qi_current = (cultivation.qi_current - drain).max(0.0);
            assert!(
                (cultivation.qi_current - 9.0).abs() < 1e-9,
                "realm={realm:?}: expected qi_current=9.0, got {}",
                cultivation.qi_current
            );
        }
    }

    #[test]
    fn system_drains_players_inside_dead_zone_only() {
        let mut app = App::new();
        app.insert_resource(DeadZoneTickHandler::default());
        app.insert_resource(ZoneRegistry {
            zones: vec![dead_zone("ash"), normal_zone("normal")],
        });
        app.add_event::<QiTransfer>();
        app.add_systems(Update, dead_zone_silent_qi_loss_tick);

        let inside = app
            .world_mut()
            .spawn((
                Position::new([10.0, 66.0, 10.0]),
                CurrentDimension(DimensionKind::Overworld),
                Cultivation {
                    qi_current: 10.0,
                    qi_max: 100.0,
                    ..Cultivation::default()
                },
            ))
            .id();
        let outside = app
            .world_mut()
            .spawn((
                Position::new([210.0, 66.0, 10.0]),
                CurrentDimension(DimensionKind::Overworld),
                Cultivation {
                    qi_current: 10.0,
                    qi_max: 100.0,
                    ..Cultivation::default()
                },
            ))
            .id();

        app.update();

        let inside_qi = app.world().get::<Cultivation>(inside).unwrap().qi_current;
        let outside_qi = app.world().get::<Cultivation>(outside).unwrap().qi_current;
        let expected_drain = 1.0 / TICKS_PER_MINUTE;
        assert!(
            (inside_qi - (10.0 - expected_drain)).abs() < 1e-9,
            "player inside dead zone: expected qi={}, got {}",
            10.0 - expected_drain,
            inside_qi
        );
        assert_eq!(
            outside_qi, 10.0,
            "player outside dead zone must not lose qi"
        );
    }

    /// 核心守恒测试：drain 从玩家扣除的 qi 必须出现在 QiTransfer 事件中（进 zone 或 overflow）。
    /// 此测试确保 dead_zone_drain 不再是"蒸发"路径——每次扣 qi 都有对应事件。
    /// NOTE: ash zone 的 spirit_qi=0.0，这是正好 dead zone 阈值，所以接受 qi 后会
    /// 变成 0.0 + drain/QI_ZONE_UNIT_CAPACITY；zone 仍然接近 cap=0（实际上 ash zone
    /// spirit_qi 会稍微增加但可能立刻又是 dead zone 下次 tick 再 drain）。
    /// 重点是 QiTransfer 事件一定被发出，说明 ledger 被记账了。
    #[test]
    fn drain_emits_qi_transfer_event_for_ledger_accounting() {
        let mut app = App::new();
        app.insert_resource(DeadZoneTickHandler::default());
        // ash zone: spirit_qi=0.0 (dead zone), empty so has room to receive qi
        app.insert_resource(ZoneRegistry {
            zones: vec![dead_zone("ash")],
        });
        app.add_event::<QiTransfer>();
        app.add_systems(Update, dead_zone_silent_qi_loss_tick);

        let player = app
            .world_mut()
            .spawn((
                Position::new([10.0, 66.0, 10.0]),
                CurrentDimension(DimensionKind::Overworld),
                LifeRecord::new(canonical_player_id("Kiz")),
                Cultivation {
                    qi_current: 10.0,
                    qi_max: 100.0,
                    ..Cultivation::default()
                },
            ))
            .id();

        app.update();

        let qi_after = app.world().get::<Cultivation>(player).unwrap().qi_current;
        let expected_drain = 1.0 / TICKS_PER_MINUTE;
        assert!(
            (qi_after - (10.0 - expected_drain)).abs() < 1e-9,
            "expected player qi={} after drain, got {}",
            10.0 - expected_drain,
            qi_after
        );

        let transfers: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<QiTransfer>>()
            .drain()
            .collect();
        assert_eq!(
            transfers.len(),
            1,
            "expected exactly one QiTransfer event for ledger accounting (dead_zone_drain), got {}",
            transfers.len()
        );
        assert_eq!(
            transfers[0].from,
            QiAccountId::player(canonical_player_id("Kiz")),
            "QiTransfer.from must identify the draining player"
        );
        assert_eq!(
            transfers[0].reason,
            QiTransferReason::ReleaseToZone,
            "QiTransfer reason must be ReleaseToZone"
        );
        assert!(
            (transfers[0].amount - expected_drain).abs() < 1e-9,
            "QiTransfer.amount must equal the drained amount {}, got {}",
            expected_drain,
            transfers[0].amount
        );
    }

    /// 边界：玩家 qi_current=0 时不 drain，也不发 QiTransfer 事件（drain=0 < QI_EPSILON）。
    #[test]
    fn zero_qi_player_in_dead_zone_emits_no_transfer() {
        let mut app = App::new();
        app.insert_resource(DeadZoneTickHandler::default());
        app.insert_resource(ZoneRegistry {
            zones: vec![dead_zone("ash")],
        });
        app.add_event::<QiTransfer>();
        app.add_systems(Update, dead_zone_silent_qi_loss_tick);

        let player = app
            .world_mut()
            .spawn((
                Position::new([10.0, 66.0, 10.0]),
                CurrentDimension(DimensionKind::Overworld),
                Cultivation {
                    qi_current: 0.0,
                    qi_max: 100.0,
                    ..Cultivation::default()
                },
            ))
            .id();

        app.update();

        let qi_after = app.world().get::<Cultivation>(player).unwrap().qi_current;
        assert_eq!(qi_after, 0.0, "player with qi=0 must not go below 0");
        let transfers: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<QiTransfer>>()
            .drain()
            .collect();
        assert!(
            transfers.is_empty(),
            "no QiTransfer expected when drain amount is zero, got {} events",
            transfers.len()
        );
    }

    /// 边界：zone spirit_qi 接近满容时（0.0 + tiny room），drain 进 zone；
    /// 若 zone 是 dead zone（spirit_qi≈0.0，刚空出来），有大量 headroom 接受 drain。
    /// 此测试确认 zone spirit_qi 在 drain 后增加了（qi 进入 zone，ledger 平衡）。
    ///
    /// PITFALL (a): 使用自定义零 spirit_qi 的 ash zone，不用 ZoneRegistry::fallback()
    /// 的默认 spawn zone（fallback spawn zone spirit_qi=0.9 接近满，room 只有 ~5 raw qi）。
    /// PITFALL (b): 玩家必须有 CurrentDimension 组件，否则 find_zone 返回 None，
    /// qi 路由到 Overflow 账户而非 zone 账户。
    #[test]
    fn drain_credits_zone_spirit_qi_for_conservation() {
        let mut app = App::new();
        app.insert_resource(DeadZoneTickHandler::default());
        // ash zone starts at spirit_qi=0.0: dead zone with ample headroom to receive drain
        app.insert_resource(ZoneRegistry {
            zones: vec![dead_zone("ash")],
        });
        app.add_event::<QiTransfer>();
        app.add_systems(Update, dead_zone_silent_qi_loss_tick);

        app.world_mut().spawn((
            Position::new([10.0, 66.0, 10.0]),
            CurrentDimension(DimensionKind::Overworld), // PITFALL (b): must include CurrentDimension
            Cultivation {
                qi_current: 10.0,
                qi_max: 100.0,
                ..Cultivation::default()
            },
        ));

        let zone_before = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name("ash")
            .unwrap()
            .spirit_qi;

        app.update();

        let zone_after = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name("ash")
            .unwrap()
            .spirit_qi;

        assert!(
            zone_after > zone_before,
            "dead zone spirit_qi must increase after drain (conservation: player qi -> zone qi), was {zone_before}, got {zone_after}"
        );
    }

    /// 无 ZoneRegistry 资源时，系统提前退出，不 drain 任何玩家。
    #[test]
    fn missing_zone_registry_skips_all_drains() {
        let mut app = App::new();
        app.insert_resource(DeadZoneTickHandler::default());
        // No ZoneRegistry inserted
        app.add_event::<QiTransfer>();
        app.add_systems(Update, dead_zone_silent_qi_loss_tick);

        let player = app
            .world_mut()
            .spawn((
                Position::new([10.0, 66.0, 10.0]),
                CurrentDimension(DimensionKind::Overworld),
                Cultivation {
                    qi_current: 10.0,
                    qi_max: 100.0,
                    ..Cultivation::default()
                },
            ))
            .id();

        app.update();

        let qi_after = app.world().get::<Cultivation>(player).unwrap().qi_current;
        assert_eq!(
            qi_after, 10.0,
            "player must not lose qi when ZoneRegistry is missing"
        );
    }

    /// qi_drain_per_minute=0 配置时，不做任何 drain，不发事件。
    #[test]
    fn zero_drain_rate_skips_all_drains() {
        let mut app = App::new();
        app.insert_resource(DeadZoneTickHandler {
            qi_drain_per_minute: 0.0,
            shelflife_zone_multiplier: 3.0,
        });
        app.insert_resource(ZoneRegistry {
            zones: vec![dead_zone("ash")],
        });
        app.add_event::<QiTransfer>();
        app.add_systems(Update, dead_zone_silent_qi_loss_tick);

        let player = app
            .world_mut()
            .spawn((
                Position::new([10.0, 66.0, 10.0]),
                CurrentDimension(DimensionKind::Overworld),
                Cultivation {
                    qi_current: 10.0,
                    qi_max: 100.0,
                    ..Cultivation::default()
                },
            ))
            .id();

        app.update();

        let qi_after = app.world().get::<Cultivation>(player).unwrap().qi_current;
        assert_eq!(qi_after, 10.0, "zero drain rate must not reduce player qi");
        let transfers: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<QiTransfer>>()
            .drain()
            .collect();
        assert!(
            transfers.is_empty(),
            "zero drain rate must emit no QiTransfer events, got {}",
            transfers.len()
        );
    }
}
