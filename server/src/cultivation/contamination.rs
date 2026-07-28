//! ContaminationTick（plan §2.1）— 异种真元排异。
//!
//! 每 tick：
//!   * 对每条污染记录 `ContamSource`，按排异效率扣减 `amount`
//!   * 自身真元按 `排异量 × DRAIN_RATIO`（10:15 亏损）扣
//!   * qi_current 不够时，按 `ContamSource.meridian_id` 定向施加裂痕：
//!     - `Some(id)` → 打指定经脉（已开时精确命中，未开时 fallback 首开经脉）
//!     - `None` → 首条已打通经脉（原行为）
//!   * `amount <= 0` 的条目移除
//!   * 所有条目都清空 + qi/经络全毁 → emit `CultivationDeathTrigger::ContaminationOverflow`

use valence::prelude::{Despawned, Entity, EventWriter, Events, Position, Query, ResMut, Without};

use crate::alchemy::skill_hook::purge_rate_bonus;
use crate::combat::components::DerivedAttrs;
use crate::skill::components::{SkillId, SkillSet};
use crate::skill::curve::effective_lv;

use super::breakthrough::skill_cap_for_realm;
use super::components::{Contamination, CrackCause, Cultivation, MeridianCrack, MeridianSystem};
use super::death_hooks::{CultivationDeathCause, CultivationDeathTrigger};
use super::tick::CultivationClock;
use crate::qi_physics::constants::QI_EPSILON;
use crate::qi_physics::{QiTransfer, WorldQiAccount};
use crate::world::dimension::CurrentDimension;
use crate::world::zone::ZoneRegistry;
use valence::prelude::Res;

use super::death_hooks::release_qi_amount_to_zone;
use super::life_record::LifeRecord;

/// plan §0-3 10:15 排异亏损比。
pub const DRAIN_RATIO: f64 = 1.5;
/// 每 tick 基础排异速率。
pub const BASE_PURGE_RATE: f64 = 0.1;

/// 定向裂痕路由：决定裂痕应施加到哪条经脉。
///
/// - `meridian_id = Some(id)` 且该经脉存在且已开 → 返回 `Some(id)`（精确命中）
/// - `meridian_id = Some(id)` 但未开/该实体的经脉档案里没有这条 channel → fallback
///   到首条已开经脉
/// - `meridian_id = None` → 首条已开经脉（原行为）
/// - 无已开经脉 → `None`（不施加裂痕）
///
/// plan-race-system-v1 P6b review BLOCKER 收口：入参/返回值都已换轨为通用
/// `MeridianChannelId`（不再是 legacy `MeridianId` 闭合枚举），非 humanoid 构型的
/// 专属 channel（如 P5 飞鲸的 `tail_core`）现在能被真实命中，不再受限于"必须能逆
/// 映射回 20 条 TCM 经脉之一"——换轨前的实现在 fallback 分支对无 legacy 对应物的
/// channel 直接 panic，本函数现在用 `MeridianSystem::contains` 判断该实体是否真的
/// 拥有这条 channel，未知/不属于该实体的 channel 一律安全 fallback，不会 panic。
pub fn resolve_crack_target(
    meridian_id: Option<super::components::MeridianChannelId>,
    meridians: &MeridianSystem,
) -> Option<super::components::MeridianChannelId> {
    match meridian_id {
        Some(id) if meridians.contains(id.clone()) && meridians.get(id.clone()).opened => Some(id),
        _ => meridians.iter().find(|m| m.opened).map(|m| m.id.clone()),
    }
}

/// 纯函数：推进一条 contam 的排异。返回 (排异量, 真元消耗, 是否清空)。
pub fn purge_step(
    contam: &mut super::components::ContamSource,
    qi_budget: f64,
    purge_rate: f64,
) -> (f64, f64, bool) {
    let (actual_purge, actual_cost, _) = preview_purge_step(contam.amount, qi_budget, purge_rate);
    apply_purge_cost(contam, actual_cost);
    let cleared = contam.amount <= 1e-9;
    (actual_purge, actual_cost, cleared)
}

fn preview_purge_step(contam_amount: f64, qi_budget: f64, purge_rate: f64) -> (f64, f64, bool) {
    let want_purge = purge_rate.min(contam_amount);
    let want_cost = want_purge * DRAIN_RATIO;
    let actual_cost = want_cost.min(qi_budget);
    let actual_purge = if want_cost > 0.0 {
        actual_cost / DRAIN_RATIO
    } else {
        0.0
    };
    let cleared = (contam_amount - actual_purge).max(0.0) <= 1e-9;
    (actual_purge, actual_cost, cleared)
}

fn apply_purge_cost(contam: &mut super::components::ContamSource, accepted_cost: f64) -> f64 {
    let actual_purge = if accepted_cost > 0.0 {
        (accepted_cost / DRAIN_RATIO).min(contam.amount)
    } else {
        0.0
    };
    contam.amount = (contam.amount - actual_purge).max(0.0);
    actual_purge
}

#[allow(clippy::type_complexity)]
pub fn contamination_tick(
    clock: Res<CultivationClock>,
    mut ledger: ResMut<WorldQiAccount>,
    mut deaths: EventWriter<CultivationDeathTrigger>,
    mut qi_transfers: Option<ResMut<Events<QiTransfer>>>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut players: Query<
        (
            Entity,
            Option<&Position>,
            Option<&CurrentDimension>,
            Option<&LifeRecord>,
            &mut Cultivation,
            &mut Contamination,
            &mut MeridianSystem,
            Option<&SkillSet>,
            Option<&DerivedAttrs>,
        ),
        Without<Despawned>,
    >,
) {
    let now = clock.tick;
    for (
        entity,
        position,
        current_dimension,
        life_record,
        mut cultivation,
        mut contam,
        mut meridians,
        skill_set,
        derived_attrs,
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
        let purge_rate = BASE_PURGE_RATE
            * (1.0 + purge_rate_bonus(alchemy_effective_lv) as f64)
            * baomai_scar_contam_purge_multiplier(derived_attrs);
        let mut any_qi_deficit = false;
        // 按 amount 从大到小处理
        contam.entries.sort_by(|a, b| {
            b.amount
                .partial_cmp(&a.amount)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for entry in contam.entries.iter_mut() {
            let budget = cultivation.qi_current.max(0.0);
            let want_cost = purge_rate.min(entry.amount) * DRAIN_RATIO;
            let (_purge, planned_cost, _cleared) =
                preview_purge_step(entry.amount, budget, purge_rate);
            let accepted_cost = match release_qi_amount_to_zone(
                &mut cultivation,
                planned_cost,
                position,
                current_dimension,
                life_record,
                zones.as_deref_mut(),
                &mut ledger,
                qi_transfers.as_deref_mut(),
                "contamination_purge",
            ) {
                Ok(outcome) => outcome.source_debited,
                Err(error) => {
                    tracing::warn!(
                        ?error,
                        "[bong][cultivation] contamination purge qi release failed closed"
                    );
                    0.0
                }
            };
            if accepted_cost + QI_EPSILON < want_cost {
                any_qi_deficit = true;
                if let Some(target_id) = resolve_crack_target(entry.meridian_id.clone(), &meridians)
                {
                    let m = meridians.get_mut(target_id);
                    m.cracks.push(MeridianCrack {
                        severity: 0.1,
                        healing_progress: 0.0,
                        cause: CrackCause::Backfire,
                        created_at: now,
                    });
                    m.integrity = (m.integrity - 0.05).max(0.0);
                }
            }
            if accepted_cost <= QI_EPSILON {
                continue;
            }
            apply_purge_cost(entry, accepted_cost);
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

fn baomai_scar_contam_purge_multiplier(derived_attrs: Option<&DerivedAttrs>) -> f64 {
    derived_attrs
        .map(|attrs| attrs.contam_purge_multiplier.max(0.0))
        .unwrap_or(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::{ColorKind, ContamSource};
    use crate::cultivation::components::{Cultivation, MeridianSystem, Realm};
    use crate::cultivation::death_hooks::CultivationDeathTrigger;
    use crate::qi_physics::{QiAccountId, QiTransferReason};
    use crate::skill::components::{SkillEntry, SkillSet};
    use crate::world::dimension::DimensionKind;
    use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};
    use valence::prelude::{App, Events, Position, Update};

    #[test]
    fn purge_consumes_qi_at_10_to_15_ratio() {
        let mut c = ContamSource {
            amount: 1.0,
            color: ColorKind::Sharp,
            meridian_id: None,
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
            meridian_id: None,
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
            meridian_id: None,
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
        app.insert_resource(ZoneRegistry::fallback());
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<QiTransfer>();
        app.insert_resource(WorldQiAccount::default());
        app.add_systems(Update, contamination_tick);

        let baseline = app
            .world_mut()
            .spawn((
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
                        meridian_id: None,
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
                Position::new([9.0, 66.0, 9.0]),
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
                        meridian_id: None,
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
        app.insert_resource(WorldQiAccount::default());
        app.add_systems(Update, contamination_tick);
        let before = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .unwrap()
            .spirit_qi;
        app.world_mut().spawn((
            Position::new([8.0, 66.0, 8.0]),
            CurrentDimension(DimensionKind::Overworld),
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
                    meridian_id: None,
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

    #[test]
    fn contamination_purge_without_zone_release_does_not_consume_qi_or_contam() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 42 });
        app.add_event::<CultivationDeathTrigger>();
        app.insert_resource(WorldQiAccount::default());
        app.add_systems(Update, contamination_tick);
        let entity = app
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
                        meridian_id: None,
                        attacker_id: None,
                        introduced_at: 1,
                    }],
                },
                MeridianSystem::default(),
            ))
            .id();

        app.update();

        let cultivation = app.world().get::<Cultivation>(entity).unwrap();
        let contamination = app.world().get::<Contamination>(entity).unwrap();
        assert_eq!(cultivation.qi_current, 10.0);
        assert_eq!(contamination.entries[0].amount, 1.0);
    }

    #[test]
    fn contamination_purge_without_zone_routes_to_overflow_when_event_available() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 42 });
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<QiTransfer>();
        app.insert_resource(WorldQiAccount::default());
        app.add_systems(Update, contamination_tick);
        let entity = app
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
                        meridian_id: None,
                        attacker_id: None,
                        introduced_at: 1,
                    }],
                },
                MeridianSystem::default(),
            ))
            .id();

        app.update();

        let cultivation = app.world().get::<Cultivation>(entity).unwrap();
        let contamination = app.world().get::<Contamination>(entity).unwrap();
        assert!(cultivation.qi_current < 10.0);
        assert!(contamination.entries[0].amount < 1.0);
        let transfers: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<QiTransfer>>()
            .drain()
            .collect();
        assert_eq!(transfers.len(), 1);
        assert_eq!(
            transfers[0].to,
            QiAccountId::overflow(format!("contamination_purge:{entity:?}"))
        );
        assert_eq!(transfers[0].reason, QiTransferReason::ReleaseToZone);
    }

    fn spawn_contaminated_player(
        app: &mut App,
        attrs: Option<DerivedAttrs>,
        despawned: bool,
    ) -> Entity {
        let mut entity = app.world_mut().spawn((
            Position::new([8.0, 66.0, 8.0]),
            CurrentDimension(DimensionKind::Overworld),
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
                    meridian_id: None,
                    attacker_id: None,
                    introduced_at: 1,
                }],
            },
            MeridianSystem::default(),
        ));
        if let Some(attrs) = attrs {
            entity.insert(attrs);
        }
        if despawned {
            entity.insert(Despawned);
        }
        entity.id()
    }

    #[test]
    fn scar_contam_purge_multiplier_increases_purge_without_changing_release_ratio() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 42 });
        app.insert_resource(ZoneRegistry::fallback());
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<QiTransfer>();
        app.insert_resource(WorldQiAccount::default());
        app.add_systems(Update, contamination_tick);

        let before = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .unwrap()
            .spirit_qi;
        let baseline = spawn_contaminated_player(&mut app, None, false);
        let boosted = spawn_contaminated_player(
            &mut app,
            Some(DerivedAttrs {
                contam_purge_multiplier: 2.0,
                ..DerivedAttrs::default()
            }),
            false,
        );

        app.update();

        let baseline_contam = app.world().get::<Contamination>(baseline).unwrap();
        let boosted_contam = app.world().get::<Contamination>(boosted).unwrap();
        let baseline_qi = app.world().get::<Cultivation>(baseline).unwrap().qi_current;
        let boosted_qi = app.world().get::<Cultivation>(boosted).unwrap().qi_current;
        assert!(
            (baseline_contam.entries[0].amount - 0.9).abs() < 1e-9,
            "baseline purge should remove BASE_PURGE_RATE contamination"
        );
        assert!(
            (boosted_contam.entries[0].amount - 0.8).abs() < 1e-9,
            "contam_purge_multiplier=2 should remove double BASE_PURGE_RATE contamination"
        );
        assert!(
            (baseline_qi - (10.0 - BASE_PURGE_RATE * DRAIN_RATIO)).abs() < 1e-9,
            "baseline qi cost must keep 10:15 ratio"
        );
        assert!(
            (boosted_qi - (10.0 - BASE_PURGE_RATE * 2.0 * DRAIN_RATIO)).abs() < 1e-9,
            "boosted qi cost must scale only because purge amount scaled"
        );

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
        let total_released: f64 = transfers.iter().map(|transfer| transfer.amount).sum();
        assert_eq!(transfers.len(), 2);
        assert!(
            (total_released - BASE_PURGE_RATE * DRAIN_RATIO * 3.0).abs() < 1e-9,
            "baseline + boosted release amount should equal their accepted purge costs"
        );
        assert!(
            ((after - before) * crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY
                - total_released)
                .abs()
                < 1e-9,
            "zone.spirit_qi increase must match QiTransfer release amount"
        );
    }

    #[test]
    fn scar_contam_purge_multiplier_clamps_negative_to_noop_rate() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 42 });
        app.insert_resource(ZoneRegistry::fallback());
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<QiTransfer>();
        app.insert_resource(WorldQiAccount::default());
        app.add_systems(Update, contamination_tick);
        let entity = spawn_contaminated_player(
            &mut app,
            Some(DerivedAttrs {
                contam_purge_multiplier: -1.0,
                ..DerivedAttrs::default()
            }),
            false,
        );

        app.update();

        let qi_current = app.world().get::<Cultivation>(entity).unwrap().qi_current;
        let contamination_amount =
            app.world().get::<Contamination>(entity).unwrap().entries[0].amount;
        let transfers: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<QiTransfer>>()
            .drain()
            .collect();
        assert_eq!(qi_current, 10.0);
        assert_eq!(contamination_amount, 1.0);
        assert!(
            transfers.is_empty(),
            "negative contam_purge_multiplier clamps to zero purge rate and emits no release"
        );
    }

    #[test]
    fn contamination_tick_skips_despawned_offline_players() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 42 });
        app.insert_resource(ZoneRegistry::fallback());
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<QiTransfer>();
        app.insert_resource(WorldQiAccount::default());
        app.add_systems(Update, contamination_tick);
        let before = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .unwrap()
            .spirit_qi;
        let entity = spawn_contaminated_player(
            &mut app,
            Some(DerivedAttrs {
                contam_purge_multiplier: 2.0,
                ..DerivedAttrs::default()
            }),
            true,
        );

        app.update();

        let qi_current = app.world().get::<Cultivation>(entity).unwrap().qi_current;
        let contamination_amount =
            app.world().get::<Contamination>(entity).unwrap().entries[0].amount;
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
        assert_eq!(qi_current, 10.0);
        assert_eq!(contamination_amount, 1.0);
        assert_eq!(after, before);
        assert!(
            transfers.is_empty(),
            "离线 Despawned 玩家不应继续排异或释放真元"
        );
    }

    // ── 定向裂痕路由测试（PR-1: contamination meridian_id routing）──
    //
    // 测试 `resolve_crack_target` 纯函数：决定裂痕施加到哪条经脉。

    use crate::cultivation::components::MeridianId;

    /// 辅助：构造带指定已开经脉的 MeridianSystem。
    fn meridian_system_with_opened(opened_ids: &[MeridianId]) -> MeridianSystem {
        let mut ms = MeridianSystem::default();
        for &id in opened_ids {
            let m = ms.get_mut(id);
            m.opened = true;
            m.opened_at = 1;
        }
        ms
    }

    #[test]
    fn crack_route_none_hits_first_opened_meridian() {
        // meridian_id: None → 应该命中首条已开经脉（Lung，因 iter 顺序 Lung 在前）
        let ms = meridian_system_with_opened(&[MeridianId::Lung, MeridianId::Heart]);
        let target = resolve_crack_target(None, &ms);
        assert_eq!(
            target,
            Some(MeridianId::Lung.channel_id()),
            "meridian_id=None 时应 fallback 到首开经脉 Lung（iter 序第一），\
             实际返回 {:?}",
            target
        );
    }

    #[test]
    fn crack_route_some_lung_hits_lung() {
        // meridian_id: Some(Lung) → 精确打肺经
        let ms = meridian_system_with_opened(&[MeridianId::Lung, MeridianId::Heart]);
        let target = resolve_crack_target(Some(MeridianId::Lung.channel_id()), &ms);
        assert_eq!(
            target,
            Some(MeridianId::Lung.channel_id()),
            "meridian_id=Some(Lung) 且 Lung 已开时应精确命中 Lung，\
             实际返回 {:?}",
            target
        );
    }

    #[test]
    fn crack_route_some_heart_hits_heart() {
        // meridian_id: Some(Heart) → 精确打心经
        let ms = meridian_system_with_opened(&[MeridianId::Lung, MeridianId::Heart]);
        let target = resolve_crack_target(Some(MeridianId::Heart.channel_id()), &ms);
        assert_eq!(
            target,
            Some(MeridianId::Heart.channel_id()),
            "meridian_id=Some(Heart) 且 Heart 已开时应精确命中 Heart，\
             实际返回 {:?}",
            target
        );
    }

    #[test]
    fn crack_route_target_not_opened_falls_back_to_first_opened() {
        // meridian_id: Some(Kidney) 但 Kidney 未开 → fallback 到首开经脉 Lung
        let ms = meridian_system_with_opened(&[MeridianId::Lung]); // 只开了 Lung
        let target = resolve_crack_target(Some(MeridianId::Kidney.channel_id()), &ms);
        assert_eq!(
            target,
            Some(MeridianId::Lung.channel_id()),
            "目标经脉 Kidney 未开时应 fallback 到首开经脉 Lung，\
             实际返回 {:?}",
            target
        );
    }

    #[test]
    fn crack_route_target_opened_but_not_first_still_hits_target() {
        // meridian_id: Some(Heart) 且 Heart 已开但非首开（Lung 在 iter 序更前）
        // → 应精确打 Heart 而非 Lung
        let ms = meridian_system_with_opened(&[MeridianId::Lung, MeridianId::Heart]);
        let target = resolve_crack_target(Some(MeridianId::Heart.channel_id()), &ms);
        assert_eq!(
            target,
            Some(MeridianId::Heart.channel_id()),
            "meridian_id=Some(Heart) 且 Heart 已开时应精确命中 Heart（即使非首开），\
             实际返回 {:?}",
            target
        );
    }

    #[test]
    fn crack_route_no_meridians_opened_returns_none() {
        // 所有经脉都未开 → 返回 None（无合法目标，不 panic）
        let ms = meridian_system_with_opened(&[]); // 全部未开
        let target = resolve_crack_target(Some(MeridianId::Lung.channel_id()), &ms);
        assert_eq!(
            target, None,
            "所有经脉都未开时应返回 None（无合法目标），\
             实际返回 {:?}",
            target
        );
        // None meridian_id 同样
        let target2 = resolve_crack_target(None, &ms);
        assert_eq!(
            target2, None,
            "meridian_id=None 且所有经脉都未开时应返回 None，\
             实际返回 {:?}",
            target2
        );
    }

    /// review BLOCKER 收口专属 pin：目标 channel 是一个该实体经脉档案里根本不存在的
    /// 非 humanoid channel id（如误挂靠到另一种构型的 channel）——换轨前的实现会在
    /// fallback 分支对"逆映射不到 legacy MeridianId"的 channel panic；换轨后必须
    /// 安全 fallback 到首开经脉，不 panic（`MeridianSystem::contains` 短路判断）。
    #[test]
    fn crack_route_target_channel_not_in_profile_falls_back_without_panic() {
        let ms = meridian_system_with_opened(&[MeridianId::Lung]);
        let target = resolve_crack_target(
            Some(crate::cultivation::components::MeridianChannelId::new(
                "tail_core",
            )),
            &ms,
        );
        assert_eq!(
            target,
            Some(MeridianId::Lung.channel_id()),
            "目标 channel 不在该实体经脉档案里时应安全 fallback 到首开经脉而不 panic，\
             实际返回 {:?}",
            target
        );
    }
}
