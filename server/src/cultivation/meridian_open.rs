//! MeridianOpenTick（plan §2）— 玩家选定下一条经脉后，按 zone 浓度 +
//! qi 比例累积 `open_progress`，到 1.0 时打通、扩容 qi_max。
//!
//! P1 约束：
//!   * 目标必须与已打通经脉相邻（通过 `MeridianTopology`）
//!   * Awaken 期首脉特许（无已开经脉时允许任一）
//!   * zone.spirit_qi >= 0.3 才推进（阈值内不能打通）
//!   * 打通本身消耗 qi（cost = progress_delta × COST_FACTOR）

use valence::prelude::{bevy_ecs, Component, Position, Query, Res};

use crate::world::zone::ZoneRegistry;

use super::components::{Cultivation, MeridianId, MeridianSystem};
use super::life_record::{BiographyEntry, LifeRecord};
use super::tick::CultivationClock;
use super::topology::MeridianTopology;

/// 玩家客户端发起的"选择下一条经脉"目标。未选目标时此 component 不存在。
#[derive(Debug, Clone, Copy, Component)]
pub struct MeridianTarget(pub MeridianId);

pub const MIN_ZONE_QI_TO_OPEN: f64 = 0.3;
pub const BASE_OPEN_RATE: f64 = 0.01;
pub const OPEN_COST_FACTOR: f64 = 5.0;
pub const MERIDIAN_CAPACITY_ON_OPEN: f64 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OpenStepError {
    ZoneTooWeak,
    NotAdjacent,
    NotEnoughQi,
    AlreadyOpen,
}

/// 纯函数：返回 `progress_delta`（可能 0）或拒绝原因。`adjacent_ok` 由 topology 在
/// 外部判定；此处只执行数值推进与扣费。
pub fn advance_open_progress(
    cultivation: &mut Cultivation,
    meridians: &mut MeridianSystem,
    target: MeridianId,
    zone_qi: f64,
    adjacent_ok: bool,
) -> Result<f64, OpenStepError> {
    advance_open_progress_at(cultivation, meridians, target, zone_qi, adjacent_ok, 0)
        .map(|(delta, _just_opened)| delta)
}

/// 与 [`advance_open_progress`] 相同，但额外返回 "本次是否完成打通"，并在打通时写入
/// `opened_at = tick_now` 以支持 LIFO 排序。
pub fn advance_open_progress_at(
    cultivation: &mut Cultivation,
    meridians: &mut MeridianSystem,
    target: MeridianId,
    zone_qi: f64,
    adjacent_ok: bool,
    tick_now: u64,
) -> Result<(f64, bool), OpenStepError> {
    if meridians.get(target).opened {
        return Err(OpenStepError::AlreadyOpen);
    }
    if !adjacent_ok {
        return Err(OpenStepError::NotAdjacent);
    }
    if zone_qi < MIN_ZONE_QI_TO_OPEN {
        return Err(OpenStepError::ZoneTooWeak);
    }
    let qi_ratio = if cultivation.qi_max > 0.0 {
        cultivation.qi_current / cultivation.qi_max
    } else {
        0.0
    };
    let delta = BASE_OPEN_RATE * zone_qi * qi_ratio;
    let cost = delta * OPEN_COST_FACTOR;
    if cultivation.qi_current < cost {
        return Err(OpenStepError::NotEnoughQi);
    }

    cultivation.qi_current -= cost;
    let m = meridians.get_mut(target);
    let was_open = m.opened;
    m.open_progress = (m.open_progress + delta).min(1.0);
    let mut just_opened = false;
    if !was_open && m.open_progress >= 1.0 {
        m.opened = true;
        m.opened_at = tick_now;
        m.flow_capacity = m.flow_capacity.max(MERIDIAN_CAPACITY_ON_OPEN);
        cultivation.qi_max += MERIDIAN_CAPACITY_ON_OPEN;
        just_opened = true;
    }

    Ok((delta, just_opened))
}

/// 判定邻接：首脉特许（无已开经脉时任意目标合法），否则必须邻接至少一条已通。
pub fn is_target_adjacent(
    topo: &MeridianTopology,
    meridians: &MeridianSystem,
    target: MeridianId,
) -> bool {
    if meridians.opened_count() == 0 {
        return true;
    }
    topo.neighbors(target)
        .iter()
        .any(|n| meridians.get(*n).opened)
}

pub fn meridian_open_tick(
    topo: Res<MeridianTopology>,
    clock: Res<CultivationClock>,
    zones: Option<Res<ZoneRegistry>>,
    mut entities: Query<(
        &Position,
        &MeridianTarget,
        &mut Cultivation,
        &mut MeridianSystem,
        // LifeRecord 可选：玩家有完整生平卷，NPC 无（plan §8 已决定）。
        // 推进经脉逻辑对 NPC / 玩家一视同仁，仅生平记录步骤按存在与否跳过。
        Option<&mut LifeRecord>,
    )>,
) {
    let Some(zones) = zones else {
        return;
    };
    let now = clock.tick;
    for (pos, target, mut cultivation, mut meridians, life) in entities.iter_mut() {
        let zone_qi = zones.find_zone(pos.0).map(|z| z.spirit_qi).unwrap_or(0.0);
        let adj = is_target_adjacent(&topo, &meridians, target.0);
        if let Ok((_delta, just_opened)) = advance_open_progress_at(
            &mut cultivation,
            &mut meridians,
            target.0,
            zone_qi,
            adj,
            now,
        ) {
            if just_opened {
                if let Some(mut life) = life {
                    life.push(BiographyEntry::MeridianOpened {
                        id: target.0,
                        tick: now,
                    });
                    if life.spirit_root_first.is_none() {
                        life.spirit_root_first = Some(target.0);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::Cultivation;

    fn player_with_qi(qi: f64) -> Cultivation {
        Cultivation {
            qi_current: qi,
            qi_max: 10.0,
            ..Default::default()
        }
    }

    #[test]
    fn first_meridian_always_adjacent() {
        let t = MeridianTopology::standard();
        let ms = MeridianSystem::default();
        assert!(is_target_adjacent(&t, &ms, MeridianId::Lung));
        assert!(is_target_adjacent(&t, &ms, MeridianId::YangWei));
    }

    #[test]
    fn second_meridian_requires_real_adjacency() {
        let t = MeridianTopology::standard();
        let mut ms = MeridianSystem::default();
        ms.get_mut(MeridianId::Lung).opened = true;
        assert!(is_target_adjacent(&t, &ms, MeridianId::LargeIntestine));
        assert!(!is_target_adjacent(&t, &ms, MeridianId::Stomach));
    }

    #[test]
    fn zone_too_weak_rejected_without_side_effects() {
        let mut c = player_with_qi(10.0);
        let mut ms = MeridianSystem::default();
        let err = advance_open_progress(&mut c, &mut ms, MeridianId::Lung, 0.1, true).unwrap_err();
        assert_eq!(err, OpenStepError::ZoneTooWeak);
        assert_eq!(c.qi_current, 10.0);
        assert_eq!(ms.get(MeridianId::Lung).open_progress, 0.0);
    }

    #[test]
    fn non_adjacent_rejected() {
        let mut c = player_with_qi(10.0);
        let mut ms = MeridianSystem::default();
        ms.get_mut(MeridianId::Lung).opened = true;
        let err =
            advance_open_progress(&mut c, &mut ms, MeridianId::Heart, 0.9, false).unwrap_err();
        assert_eq!(err, OpenStepError::NotAdjacent);
    }

    #[test]
    fn already_open_rejected() {
        let mut c = player_with_qi(10.0);
        let mut ms = MeridianSystem::default();
        ms.get_mut(MeridianId::Lung).opened = true;
        let err = advance_open_progress(&mut c, &mut ms, MeridianId::Lung, 0.9, true).unwrap_err();
        assert_eq!(err, OpenStepError::AlreadyOpen);
    }

    #[test]
    fn progress_accumulates_and_opens() {
        let mut c = player_with_qi(1000.0);
        c.qi_max = 1000.0;
        let mut ms = MeridianSystem::default();
        for _ in 0..200 {
            let _ = advance_open_progress(&mut c, &mut ms, MeridianId::Lung, 1.0, true);
            if ms.get(MeridianId::Lung).opened {
                break;
            }
        }
        assert!(ms.get(MeridianId::Lung).opened);
        assert!(c.qi_max >= 1000.0 + MERIDIAN_CAPACITY_ON_OPEN);
    }
}
