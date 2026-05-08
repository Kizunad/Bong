//! ContaminationTick（plan §2.1）— 异种真元排异。
//!
//! 每 tick：
//!   * 对每条污染记录 `ContamSource`，按排异效率扣减 `amount`
//!   * 自身真元按 `排异量 × DRAIN_RATIO`（10:15 亏损）扣
//!   * qi_current 不够时，对随机经脉施加裂痕（P1: 施加到首条已打通经脉）
//!   * `amount <= 0` 的条目移除
//!   * 所有条目都清空 + qi/经络全毁 → emit `CultivationDeathTrigger::ContaminationOverflow`

use valence::prelude::{Entity, EventWriter, Events, Position, Query, ResMut};

use crate::alchemy::skill_hook::purge_rate_bonus;
use crate::skill::components::{SkillId, SkillSet};
use crate::skill::curve::effective_lv;

use super::breakthrough::skill_cap_for_realm;
use super::components::{Contamination, CrackCause, Cultivation, MeridianCrack, MeridianSystem};
use super::death_hooks::{CultivationDeathCause, CultivationDeathTrigger};
use super::tick::CultivationClock;
use crate::qi_physics::constants::{QI_EPSILON, QI_ZONE_UNIT_CAPACITY};
use crate::qi_physics::{qi_release_to_zone, QiAccountId, QiTransfer};
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::zone::ZoneRegistry;
use valence::prelude::Res;

/// plan §0-3 10:15 排异亏损比。
pub const DRAIN_RATIO: f64 = 1.5;
/// 每 tick 基础排异速率。
pub const BASE_PURGE_RATE: f64 = 0.1;

/// 纯函数：推进一条 contam 的排异。返回 (排异量, 真元消耗, 是否清空)。
pub fn purge_step(
    contam: &mut super::components::ContamSource,
    qi_budget: f64,
    purge_rate: f64,
) -> (f64, f64, bool) {
    let want_purge = purge_rate.min(contam.amount);
    let want_cost = want_purge * DRAIN_RATIO;
    let actual_cost = want_cost.min(qi_budget);
    let actual_purge = if want_cost > 0.0 {
        actual_cost / DRAIN_RATIO
    } else {
        0.0
    };
    contam.amount = (contam.amount - actual_purge).max(0.0);
    let cleared = contam.amount <= 1e-9;
    (actual_purge, actual_cost, cleared)
}

#[allow(clippy::type_complexity)]
pub fn contamination_tick(
    clock: Res<CultivationClock>,
    mut deaths: EventWriter<CultivationDeathTrigger>,
    mut qi_transfers: Option<ResMut<Events<QiTransfer>>>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut players: Query<(
        Entity,
        Option<&Position>,
        Option<&CurrentDimension>,
        &mut Cultivation,
        &mut Contamination,
        &mut MeridianSystem,
        Option<&SkillSet>,
    )>,
) {
    let now = clock.tick;
    for (
        entity,
        position,
        current_dimension,
        mut cultivation,
        mut contam,
        mut meridians,
        skill_set,
    ) in players.iter_mut()
    {
        if contam.entries.is_empty() {
            continue;
        }
        let alchemy_real_lv = skill_set
            .and_then(|skill_set| {
                skill_set
                    .skills
                    .get(&SkillId::Alchemy)
                    .map(|entry| entry.lv)
            })
            .unwrap_or(0);
        let alchemy_effective_lv =
            effective_lv(alchemy_real_lv, skill_cap_for_realm(cultivation.realm));
        let purge_rate = BASE_PURGE_RATE * (1.0 + purge_rate_bonus(alchemy_effective_lv) as f64);
        let mut any_qi_deficit = false;
        // 按 amount 从大到小处理
        contam.entries.sort_by(|a, b| {
            b.amount
                .partial_cmp(&a.amount)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for entry in contam.entries.iter_mut() {
            let budget = cultivation.qi_current.max(0.0);
            let (_purge, cost, _cleared) = purge_step(entry, budget, purge_rate);
            cultivation.qi_current -= cost;
            release_contamination_cost_to_zone(
                entity,
                cost,
                position,
                current_dimension,
                zones.as_deref_mut(),
                qi_transfers.as_deref_mut(),
            );
            if cultivation.qi_current < 0.0 {
                any_qi_deficit = true;
                // 对首条已打通经脉添加裂痕
                if let Some(m) = meridians.iter_mut().find(|m| m.opened) {
                    m.cracks.push(MeridianCrack {
                        severity: 0.1,
                        healing_progress: 0.0,
                        cause: CrackCause::Backfire,
                        created_at: now,
                    });
                    m.integrity = (m.integrity - 0.05).max(0.0);
                }
                cultivation.qi_current = 0.0;
            }
        }

        contam.entries.retain(|e| e.amount > 1e-9);

        // 致死检查：经络全毁 + qi=0 + 仍残留污染（暂用简单判据）
        let all_broken = meridians.iter().all(|m| m.integrity <= 0.0 || !m.opened);
        if any_qi_deficit && all_broken && !contam.entries.is_empty() {
            deaths.send(CultivationDeathTrigger {
                entity,
                cause: CultivationDeathCause::ContaminationOverflow,
                context: serde_json::json!({
                    "remaining": contam.entries.len(),
                    "tick": now,
                }),
            });
        }
    }
}

fn release_contamination_cost_to_zone(
    entity: Entity,
    amount: f64,
    position: Option<&Position>,
    current_dimension: Option<&CurrentDimension>,
    zones: Option<&mut ZoneRegistry>,
    qi_transfers: Option<&mut Events<QiTransfer>>,
) {
    if amount <= QI_EPSILON {
        return;
    }
    let (Some(position), Some(zones)) = (position, zones) else {
        return;
    };
    let dimension = current_dimension
        .map(|current| current.0)
        .unwrap_or(DimensionKind::Overworld);
    let Some(zone_name) = zones
        .find_zone(dimension, position.0)
        .map(|zone| zone.name.clone())
    else {
        return;
    };
    let Some(zone) = zones.find_zone_mut(zone_name.as_str()) else {
        return;
    };
    let from = QiAccountId::player(format!("entity:{entity:?}:contamination"));
    let to = QiAccountId::zone(zone.name.clone());
    let zone_current = zone.spirit_qi * QI_ZONE_UNIT_CAPACITY;
    let Ok(outcome) = qi_release_to_zone(amount, from, to, zone_current, QI_ZONE_UNIT_CAPACITY)
    else {
        return;
    };
    zone.spirit_qi = outcome.zone_after / QI_ZONE_UNIT_CAPACITY;
    if let (Some(transfer), Some(qi_transfers)) = (outcome.transfer, qi_transfers) {
        qi_transfers.send(transfer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::{ColorKind, ContamSource};
    use crate::cultivation::components::{Cultivation, MeridianSystem, Realm};
    use crate::cultivation::death_hooks::CultivationDeathTrigger;
    use crate::skill::components::{SkillEntry, SkillSet};
    use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};
    use valence::prelude::{App, Events, Position, Update};

    #[test]
    fn purge_consumes_qi_at_10_to_15_ratio() {
        let mut c = ContamSource {
            amount: 1.0,
            color: ColorKind::Sharp,
            attacker_id: None,
            introduced_at: 0,
        };
        let (purge, cost, _) = purge_step(&mut c, 100.0, BASE_PURGE_RATE);
        assert!((cost / purge - DRAIN_RATIO).abs() < 1e-9);
    }

    #[test]
    fn purge_clamped_by_qi_budget() {
        let mut c = ContamSource {
            amount: 1.0,
            color: ColorKind::Sharp,
            attacker_id: None,
            introduced_at: 0,
        };
        let (_purge, cost, _) = purge_step(&mut c, 0.05, BASE_PURGE_RATE);
        assert!(cost <= 0.05 + 1e-9);
    }

    #[test]
    fn purge_clears_when_amount_reaches_zero() {
        let mut c = ContamSource {
            amount: 0.05,
            color: ColorKind::Sharp,
            attacker_id: None,
            introduced_at: 0,
        };
        let (_, _, cleared) = purge_step(&mut c, 100.0, BASE_PURGE_RATE);
        assert!(cleared);
    }

    #[test]
    fn alchemy_skill_increases_contamination_purge_rate() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 42 });
        app.add_event::<CultivationDeathTrigger>();
        app.add_systems(Update, contamination_tick);

        let baseline = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 10.0,
                    qi_max: 10.0,
                    ..Default::default()
                },
                Contamination {
                    entries: vec![ContamSource {
                        amount: 1.0,
                        color: ColorKind::Mellow,
                        attacker_id: None,
                        introduced_at: 1,
                    }],
                },
                MeridianSystem::default(),
            ))
            .id();

        let mut skilled_set = SkillSet::default();
        skilled_set.skills.insert(
            SkillId::Alchemy,
            SkillEntry {
                lv: 10,
                ..Default::default()
            },
        );
        let skilled = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 10.0,
                    qi_max: 10.0,
                    ..Default::default()
                },
                Contamination {
                    entries: vec![ContamSource {
                        amount: 1.0,
                        color: ColorKind::Mellow,
                        attacker_id: None,
                        introduced_at: 1,
                    }],
                },
                MeridianSystem::default(),
                skilled_set,
            ))
            .id();

        app.update();

        let baseline_contam = app
            .world()
            .get::<Contamination>(baseline)
            .expect("baseline player should still exist");
        let skilled_contam = app
            .world()
            .get::<Contamination>(skilled)
            .expect("skilled player should still exist");

        assert!(
            skilled_contam.entries[0].amount < baseline_contam.entries[0].amount,
            "alchemy skill should purge more contamination per tick"
        );
    }

    #[test]
    fn contamination_purge_releases_spent_qi_to_current_zone() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 42 });
        app.insert_resource(ZoneRegistry::fallback());
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<QiTransfer>();
        app.add_systems(Update, contamination_tick);
        let before = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .unwrap()
            .spirit_qi;
        app.world_mut().spawn((
            Position::new([8.0, 66.0, 8.0]),
            Cultivation {
                realm: Realm::Spirit,
                qi_current: 10.0,
                qi_max: 10.0,
                ..Default::default()
            },
            Contamination {
                entries: vec![ContamSource {
                    amount: 1.0,
                    color: ColorKind::Mellow,
                    attacker_id: None,
                    introduced_at: 1,
                }],
            },
            MeridianSystem::default(),
        ));

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
    }
}
