//! 经脉锻造（plan §3.2）。两条独立轴：
//!   * 流速 `flow_rate` — 每 tick 真元吞吐上限
//!   * 容量 `flow_capacity` — 经脉可同时承载的真元总量
//!
//! P1 只放开到 tier 3（计划约定），tier 跃升需要消耗 qi 并抽成 integrity
//! （代表经脉扩张的损伤 — 后续 contamination 切片再强化）。

use valence::prelude::{bevy_ecs, Entity, Event, EventReader, EventWriter, Query, Res};

use super::components::{Cultivation, Meridian, MeridianId, MeridianSystem};
use super::life_record::{BiographyEntry, LifeRecord};
use super::tick::CultivationClock;

pub const P1_MAX_TIER: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ForgeAxis {
    Rate,
    Capacity,
}

#[derive(Debug, Clone, Event)]
pub struct ForgeRequest {
    pub entity: Entity,
    pub meridian: MeridianId,
    pub axis: ForgeAxis,
}

#[derive(Debug, Clone, Event)]
pub struct ForgeOutcome {
    pub entity: Entity,
    pub meridian: MeridianId,
    pub axis: ForgeAxis,
    pub result: Result<u8, ForgeError>, // Ok(new_tier)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ForgeError {
    MeridianClosed,
    AtMaxTier,
    NotEnoughQi { need: f64, have: f64 },
}

/// tier n→n+1 的 qi 消耗（递增）。
pub fn tier_cost(next_tier: u8) -> f64 {
    // 1:4, 2:9, 3:16, ... 二次曲线，P1 上限 tier 3
    4.0 * (next_tier as f64).powi(2)
}

/// tier 对应的 flow_rate / flow_capacity 数值。
pub fn rate_for_tier(tier: u8) -> f64 {
    1.0 + tier as f64 * 0.8
}
pub fn capacity_for_tier(tier: u8) -> f64 {
    10.0 + tier as f64 * 12.0
}

/// 纯函数 — 在单条 `meridian` 上锻造一次指定轴。消耗从 `cultivation` 扣。
pub fn try_forge(
    cultivation: &mut Cultivation,
    meridian: &mut Meridian,
    axis: ForgeAxis,
) -> Result<u8, ForgeError> {
    if !meridian.opened {
        return Err(ForgeError::MeridianClosed);
    }
    let current = match axis {
        ForgeAxis::Rate => meridian.rate_tier,
        ForgeAxis::Capacity => meridian.capacity_tier,
    };
    if current >= P1_MAX_TIER {
        return Err(ForgeError::AtMaxTier);
    }
    let next = current + 1;
    let cost = tier_cost(next);
    if cultivation.qi_current < cost {
        return Err(ForgeError::NotEnoughQi {
            need: cost,
            have: cultivation.qi_current,
        });
    }

    cultivation.qi_current -= cost;
    match axis {
        ForgeAxis::Rate => {
            meridian.rate_tier = next;
            meridian.flow_rate = rate_for_tier(next);
        }
        ForgeAxis::Capacity => {
            meridian.capacity_tier = next;
            meridian.flow_capacity = capacity_for_tier(next);
        }
    }
    // 锻造轻微损耗完整度，后续 contamination 可叠加
    meridian.integrity = (meridian.integrity - 0.02).max(0.0);

    Ok(next)
}

pub fn forging_system(
    clock: Res<CultivationClock>,
    mut requests: EventReader<ForgeRequest>,
    mut outcomes: EventWriter<ForgeOutcome>,
    mut players: Query<(&mut Cultivation, &mut MeridianSystem, &mut LifeRecord)>,
) {
    let now = clock.tick;
    for req in requests.read() {
        let Ok((mut cultivation, mut meridians, mut life)) = players.get_mut(req.entity) else {
            continue;
        };
        let meridian = meridians.get_mut(req.meridian);
        let result = try_forge(&mut cultivation, meridian, req.axis);
        match &result {
            Ok(tier) => {
                let entry = match req.axis {
                    ForgeAxis::Rate => BiographyEntry::ForgedRate {
                        id: req.meridian,
                        tier: *tier,
                        tick: now,
                    },
                    ForgeAxis::Capacity => BiographyEntry::ForgedCapacity {
                        id: req.meridian,
                        tier: *tier,
                        tick: now,
                    },
                };
                life.push(entry);
                tracing::info!(
                    "[bong][cultivation] {:?} forged {:?}.{:?} -> tier {tier}",
                    req.entity,
                    req.meridian,
                    req.axis,
                );
            }
            Err(err) => tracing::debug!(
                "[bong][cultivation] {:?} forge {:?}.{:?} denied: {err:?}",
                req.entity,
                req.meridian,
                req.axis
            ),
        }
        outcomes.send(ForgeOutcome {
            entity: req.entity,
            meridian: req.meridian,
            axis: req.axis,
            result,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_lung() -> (Cultivation, MeridianSystem) {
        let c = Cultivation {
            qi_current: 100.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let mut m = MeridianSystem::default();
        m.get_mut(MeridianId::Lung).opened = true;
        (c, m)
    }

    #[test]
    fn forge_rate_happy_path_chain_to_p1_cap() {
        let (mut c, mut ms) = open_lung();
        for tier in 1..=P1_MAX_TIER {
            let m = ms.get_mut(MeridianId::Lung);
            let new_tier = try_forge(&mut c, m, ForgeAxis::Rate).unwrap();
            assert_eq!(new_tier, tier);
            assert_eq!(m.rate_tier, tier);
            assert_eq!(m.flow_rate, rate_for_tier(tier));
        }
        // 再锻就该被 P1 上限卡住
        let m = ms.get_mut(MeridianId::Lung);
        let err = try_forge(&mut c, m, ForgeAxis::Rate).unwrap_err();
        assert_eq!(err, ForgeError::AtMaxTier);
    }

    #[test]
    fn rate_and_capacity_are_independent_axes() {
        let (mut c, mut ms) = open_lung();
        let m = ms.get_mut(MeridianId::Lung);
        try_forge(&mut c, m, ForgeAxis::Rate).unwrap();
        try_forge(&mut c, m, ForgeAxis::Capacity).unwrap();
        assert_eq!(m.rate_tier, 1);
        assert_eq!(m.capacity_tier, 1);
        assert_eq!(m.flow_capacity, capacity_for_tier(1));
    }

    #[test]
    fn closed_meridian_cannot_be_forged() {
        let mut c = Cultivation {
            qi_current: 100.0,
            ..Default::default()
        };
        let mut ms = MeridianSystem::default();
        let err = try_forge(&mut c, ms.get_mut(MeridianId::Heart), ForgeAxis::Rate).unwrap_err();
        assert_eq!(err, ForgeError::MeridianClosed);
        assert_eq!(c.qi_current, 100.0);
    }

    #[test]
    fn not_enough_qi_blocks_without_side_effects() {
        let (mut c, mut ms) = open_lung();
        c.qi_current = 1.0;
        let before_integrity = ms.get(MeridianId::Lung).integrity;
        let err = try_forge(&mut c, ms.get_mut(MeridianId::Lung), ForgeAxis::Rate).unwrap_err();
        assert!(matches!(err, ForgeError::NotEnoughQi { .. }));
        assert_eq!(c.qi_current, 1.0);
        assert_eq!(ms.get(MeridianId::Lung).integrity, before_integrity);
    }

    #[test]
    fn tier_cost_monotonic() {
        assert!(tier_cost(1) < tier_cost(2));
        assert!(tier_cost(2) < tier_cost(3));
    }
}
