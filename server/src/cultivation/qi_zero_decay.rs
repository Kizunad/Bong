//! QiZeroDecayTick — 爆脉降境（plan §2）。
//!
//! 真元长期低于 1% qi_max 后：
//!   * 境界跌落一阶
//!   * 按 `(rate_tier+capacity_tier DESC, opened_at DESC)` LIFO 关闭多余经脉
//!   * open_progress 清零（但 tier 保留 — 作为"温柔"惩罚）
//!   * qi_max 按剩余经脉 capacity 重算
//!
//! P1 简化：`Meridian::opened_at` 已在结构上记录；当前 demo 仍用数组索引
//! 代理关闭顺序，等打通流程统一写真实 tick 后再切换到时间戳排序。

use valence::prelude::{bevy_ecs, Entity, Event, EventWriter, Query, Res};

use super::components::{Cultivation, Meridian, MeridianId, MeridianSystem, Realm};
use super::life_record::{BiographyEntry, LifeRecord};
use super::tick::CultivationClock;

pub const ZERO_THRESHOLD_RATIO: f64 = 0.01;
/// tick 中多少次"几近耗尽"后触发降境。默认 600（约 30s@20TPS）。
/// 与 plan 的 10min 相比这里是 demo 友好值；上线时按境界分层调节。
pub const DECAY_TRIGGER_TICKS: u64 = 600;

#[derive(Debug, Clone, Event)]
pub struct RealmRegressed {
    pub entity: Entity,
    pub from: Realm,
    pub to: Realm,
    pub closed_meridians: usize,
}

/// 纯函数：给定 meridians，决定要封闭的索引集（局部坐标：regular[0..12], extraordinary[0..8]）。
/// 返回 (is_regular, idx) 对的 Vec，按封闭优先级排序（最前的最先封）。
pub fn pick_closures(meridians: &MeridianSystem, keep_count: usize) -> Vec<(bool, usize)> {
    let mut all: Vec<(bool, usize, u8, usize)> = Vec::new(); // (is_regular, idx, tier_sum, stable_ord)
    for (i, m) in meridians.regular.iter().enumerate() {
        if m.opened {
            all.push((true, i, m.rate_tier + m.capacity_tier, i));
        }
    }
    for (i, m) in meridians.extraordinary.iter().enumerate() {
        if m.opened {
            // 奇经在 stable_ord 上偏后（代表"更晚"的加入）
            all.push((false, i, m.rate_tier + m.capacity_tier, 100 + i));
        }
    }
    if all.len() <= keep_count {
        return Vec::new();
    }
    // 排序：tier_sum DESC, stable_ord DESC（越大越先关）
    all.sort_by(|a, b| b.2.cmp(&a.2).then(b.3.cmp(&a.3)));
    let excess = all.len() - keep_count;
    all.into_iter()
        .take(excess)
        .map(|(reg, i, _, _)| (reg, i))
        .collect()
}

/// 对单条经脉执行关闭 — tier 保留，progress/throughput 清零。
pub fn close_meridian(m: &mut Meridian) {
    m.opened = false;
    m.open_progress = 0.0;
    m.throughput_current = 0.0;
    // flow_rate/flow_capacity 与 tier 一起保留——重开后无需重锻
}

pub fn qi_zero_decay_tick(
    clock: Res<CultivationClock>,
    mut outcomes: EventWriter<RealmRegressed>,
    mut entities: Query<(
        Entity,
        &mut Cultivation,
        &mut MeridianSystem,
        // LifeRecord 可选：plan §0 规则平等，NPC 无生平卷但降境数值/事件照常生效。
        Option<&mut LifeRecord>,
    )>,
) {
    let now = clock.tick;
    for (entity, mut cultivation, mut meridians, life) in entities.iter_mut() {
        let mut life = life;
        let threshold = cultivation.qi_max * ZERO_THRESHOLD_RATIO;
        if cultivation.qi_current <= threshold {
            if cultivation.last_qi_zero_at.is_none() {
                cultivation.last_qi_zero_at = Some(now);
                continue;
            }
            let since = now - cultivation.last_qi_zero_at.unwrap();
            if since < DECAY_TRIGGER_TICKS {
                continue;
            }

            // 触发降境
            let from = cultivation.realm;
            let Some(to) = from.previous() else {
                // 已在最底层，不再降
                cultivation.last_qi_zero_at = None;
                continue;
            };
            cultivation.realm = to;
            let keep = to.required_meridians();
            let closures = pick_closures(&meridians, keep);
            let closed_count = closures.len();
            for (is_regular, idx) in closures {
                let id: MeridianId = if is_regular {
                    let m = &mut meridians.regular[idx];
                    let id = m.id;
                    close_meridian(m);
                    id
                } else {
                    let m = &mut meridians.extraordinary[idx];
                    let id = m.id;
                    close_meridian(m);
                    id
                };
                if let Some(life) = life.as_deref_mut() {
                    life.push(BiographyEntry::MeridianClosed {
                        id,
                        tick: now,
                        reason: "qi_zero_decay".into(),
                    });
                }
            }
            // 重算 qi_max = 10 (基础) + 剩余经脉 capacity 之和（对齐 MeridianOpenTick 打通时 +10）
            cultivation.qi_max = 10.0 + meridians.sum_capacity();
            cultivation.qi_current = cultivation.qi_current.min(cultivation.qi_max);
            cultivation.last_qi_zero_at = None;

            tracing::warn!(
                "[bong][cultivation] {entity:?} realm regressed {from:?} -> {to:?}, closed {closed_count} meridians"
            );
            outcomes.send(RealmRegressed {
                entity,
                from,
                to,
                closed_meridians: closed_count,
            });
        } else {
            cultivation.last_qi_zero_at = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::MeridianId;

    #[test]
    fn pick_closures_returns_empty_when_within_keep() {
        let mut ms = MeridianSystem::default();
        ms.get_mut(MeridianId::Lung).opened = true;
        assert!(pick_closures(&ms, 1).is_empty());
        assert!(pick_closures(&ms, 5).is_empty());
    }

    #[test]
    fn pick_closures_prefers_high_tier_first() {
        let mut ms = MeridianSystem::default();
        ms.get_mut(MeridianId::Lung).opened = true;
        ms.get_mut(MeridianId::LargeIntestine).opened = true;
        ms.get_mut(MeridianId::LargeIntestine).rate_tier = 3;
        ms.get_mut(MeridianId::LargeIntestine).capacity_tier = 3;

        // 要保留 1 条，应砍 LI（tier_sum=6）
        let to_close = pick_closures(&ms, 1);
        assert_eq!(to_close.len(), 1);
        let (is_reg, idx) = to_close[0];
        assert!(is_reg);
        let li_idx = MeridianId::REGULAR
            .iter()
            .position(|x| *x == MeridianId::LargeIntestine)
            .unwrap();
        assert_eq!(idx, li_idx);
    }

    #[test]
    fn close_meridian_preserves_tier() {
        let mut m = Meridian::new(MeridianId::Lung);
        m.opened = true;
        m.open_progress = 1.0;
        m.rate_tier = 2;
        m.flow_rate = 3.0;
        close_meridian(&mut m);
        assert!(!m.opened);
        assert_eq!(m.open_progress, 0.0);
        // tier / flow_rate 保留
        assert_eq!(m.rate_tier, 2);
        assert_eq!(m.flow_rate, 3.0);
    }

    /// plan §0 "NPC 与玩家规则平等"：NPC 无 LifeRecord 也必须走降境。
    #[test]
    fn qi_zero_decay_tick_regresses_npc_without_life_record() {
        use valence::prelude::{App, Update};

        let mut app = App::new();
        app.insert_resource(CultivationClock {
            tick: DECAY_TRIGGER_TICKS + 10,
        });
        app.add_event::<RealmRegressed>();
        app.add_systems(Update, qi_zero_decay_tick);

        let mut meridians = MeridianSystem::default();
        meridians.get_mut(MeridianId::Lung).opened = true;
        meridians.get_mut(MeridianId::LargeIntestine).opened = true;
        let cultivation = Cultivation {
            realm: Realm::Condense, // 需降到 Induce
            qi_current: 0.0,
            qi_max: 100.0,
            last_qi_zero_at: Some(1),
            ..Cultivation::default()
        };

        let npc = app.world_mut().spawn((cultivation, meridians)).id();

        app.update();

        let cult = app.world().get::<Cultivation>(npc).unwrap();
        assert_eq!(
            cult.realm,
            Realm::Induce,
            "NPC should regress one realm even without LifeRecord"
        );
        let events: Vec<_> = app
            .world()
            .resource::<bevy_ecs::event::Events<RealmRegressed>>()
            .iter_current_update_events()
            .cloned()
            .collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].from, Realm::Condense);
        assert_eq!(events[0].to, Realm::Induce);
    }
}
