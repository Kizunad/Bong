//! NegativeZoneSiphonTick（plan §2.1）— 负灵域反吸玩家真元/血肉。
//!
//! 当 zone.spirit_qi < 0（负灵域定义来自 worldview §二）：
//!   * `siphon = |zone| × qi_max × SIPHON_FACTOR`
//!   * 优先从 qi 扣；qi=0 后从 `Health` 扣（战斗 plan 管辖，本 plan 产出事件）
//!   * `Health <= 0` → emit `CultivationDeathTrigger::NegativeZoneDrain`

use valence::prelude::{Entity, EventWriter, Position, Query, Res};

use crate::world::zone::ZoneRegistry;

use super::components::Cultivation;
use super::death_hooks::{CultivationDeathCause, CultivationDeathTrigger};

pub const SIPHON_FACTOR: f64 = 0.001;

/// 纯函数：根据 zone 浓度 + qi_max 计算本 tick siphon 量（负值 zone 才有值）。
pub fn siphon_amount(zone_qi: f64, qi_max: f64) -> f64 {
    if zone_qi >= 0.0 {
        return 0.0;
    }
    let pressure = -zone_qi;
    pressure * qi_max * SIPHON_FACTOR
}

pub fn negative_zone_siphon_tick(
    zones: Option<Res<ZoneRegistry>>,
    mut deaths: EventWriter<CultivationDeathTrigger>,
    mut players: Query<(Entity, &Position, &mut Cultivation)>,
) {
    let Some(zones) = zones else {
        return;
    };
    for (entity, pos, mut cultivation) in players.iter_mut() {
        let zone_name = zones
            .find_zone(crate::world::dimension::DimensionKind::Overworld, pos.0)
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
            continue;
        }
        // qi 吸干，转抽血肉：本 plan 不持 Health Component，发事件由战斗 plan 消费。
        // 作为最低保障：qi_current 归零，并若尚无命脉收口，直接报死亡触发。
        cultivation.qi_current = 0.0;
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
}
