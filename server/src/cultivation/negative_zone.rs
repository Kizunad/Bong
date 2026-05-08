//! NegativeZoneSiphonTick（plan §2.1）— 负灵域反吸玩家真元/血肉。
//!
//! 当 zone.spirit_qi < 0（负灵域定义来自 worldview §二）：
//!   * `siphon = |zone| × qi_max × SIPHON_FACTOR`
//!   * 优先从 qi 扣；qi=0 后从 `Health` 扣（战斗 plan 管辖，本 plan 产出事件）
//!   * `Health <= 0` → emit `CultivationDeathTrigger::NegativeZoneDrain`

use valence::prelude::{Entity, EventWriter, Events, Position, Query, ResMut};

use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::zone::ZoneRegistry;

use super::components::Cultivation;
use super::death_hooks::{
    release_qi_amount_to_zone, CultivationDeathCause, CultivationDeathTrigger,
};
use crate::cultivation::life_record::LifeRecord;
use crate::qi_physics::QiTransfer;

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
        let dimension = current_dimension
            .map(|current| current.0)
            .unwrap_or(DimensionKind::Overworld);
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
        if cultivation.qi_current >= siphon {
            cultivation.qi_current -= siphon;
            release_qi_amount_to_zone(
                entity,
                siphon,
                Some(pos),
                current_dimension,
                life_record,
                Some(&mut *zones),
                qi_transfers.as_deref_mut(),
                "negative_zone_siphon",
            );
            continue;
        }
        // qi 吸干，转抽血肉：本 plan 不持 Health Component，发事件由战斗 plan 消费。
        // 作为最低保障：qi_current 归零，并若尚无命脉收口，直接报死亡触发。
        let drained = cultivation.qi_current.max(0.0);
        cultivation.qi_current = 0.0;
        release_qi_amount_to_zone(
            entity,
            drained,
            Some(pos),
            current_dimension,
            life_record,
            Some(&mut *zones),
            qi_transfers.as_deref_mut(),
            "negative_zone_siphon",
        );
        deaths.send(CultivationDeathTrigger {
            entity,
            cause: CultivationDeathCause::NegativeZoneDrain,
            context: serde_json::json!({
                "zone": zone_name,
                "siphon": siphon,
            }),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::state::canonical_player_id;
    use crate::qi_physics::QiAccountId;
    use crate::world::dimension::{CurrentDimension, DimensionKind};
    use valence::prelude::{App, Events, Update};

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
    fn negative_zone_siphon_records_qi_transfer_to_zone() {
        let mut app = App::new();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<QiTransfer>();
        let mut zones = ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = -0.5;
        app.insert_resource(zones);
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
    }
}
