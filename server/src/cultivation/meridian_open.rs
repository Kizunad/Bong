//! MeridianOpenTick（plan §2）— 玩家选定下一条经脉后，按 zone 浓度 +
//! qi 比例累积 `open_progress`，到 1.0 时打通、扩容 qi_max。
//!
//! P1 约束：
//!   * 目标必须与已打通经脉相邻（通过 `MeridianTopology`）
//!   * Awaken 期首脉特许（无已开经脉时允许任一正经）
//!   * zone.spirit_qi >= 0.3 才推进（阈值内不能打通）
//!   * 打通本身消耗 qi（cost = progress_delta × COST_FACTOR）

use valence::prelude::{bevy_ecs, Component, Entity, Event, Events, Position, Query, Res, ResMut};

use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::events::EVENT_REALM_COLLAPSE;
use crate::world::zone::ZoneRegistry;

use super::components::{Cultivation, MeridianFamily, MeridianId, MeridianSystem};
use super::life_record::{BiographyEntry, LifeRecord};
use super::tick::CultivationClock;
use super::topology::MeridianTopology;
use crate::network::{gameplay_vfx, vfx_event_emit::VfxEventRequest};
use crate::skill::components::SkillId;
use crate::skill::events::{SkillXpGain, XpGainSource};

/// 玩家客户端发起的"选择下一条经脉"目标。未选目标时此 component 不存在。
#[derive(Debug, Clone, Copy, Component)]
pub struct MeridianTarget(pub MeridianId);

#[derive(Debug, Clone, Copy, Event)]
pub struct MeridianOpenedEvent {
    pub entity: Entity,
    pub origin: valence::prelude::DVec3,
}

pub const MIN_ZONE_QI_TO_OPEN: f64 = 0.3;
pub const BASE_OPEN_RATE: f64 = 0.01;
pub const OPEN_COST_FACTOR: f64 = 5.0;
pub const MERIDIAN_CAPACITY_ON_OPEN: f64 = 10.0;

type MeridianOpenItem<'a> = (
    Entity,
    &'a Position,
    Option<&'a CurrentDimension>,
    &'a MeridianTarget,
    &'a mut Cultivation,
    &'a mut MeridianSystem,
    // LifeRecord 可选：玩家有完整生平卷，NPC 无（plan §8 已决定）。
    // 推进经脉逻辑对 NPC / 玩家一视同仁，仅生平记录步骤按存在与否跳过。
    Option<&'a mut LifeRecord>,
);

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

/// 判定邻接：首脉特许（无已开经脉时任一正经合法），否则必须邻接至少一条已通。
pub fn is_target_adjacent(
    topo: &MeridianTopology,
    meridians: &MeridianSystem,
    target: MeridianId,
) -> bool {
    if meridians.opened_count() == 0 {
        return target.family() == MeridianFamily::Regular;
    }
    topo.neighbors(target)
        .iter()
        .any(|n| meridians.get(*n).opened)
}

#[allow(clippy::type_complexity)]
pub fn meridian_open_tick(
    topo: Res<MeridianTopology>,
    clock: Res<CultivationClock>,
    zones: Option<Res<ZoneRegistry>>,
    mut entities: Query<MeridianOpenItem<'_>>,
    mut skill_xp_events: Option<ResMut<Events<SkillXpGain>>>,
    mut vfx_events: Option<ResMut<Events<VfxEventRequest>>>,
    mut meridian_opened_events: Option<ResMut<Events<MeridianOpenedEvent>>>,
) {
    let Some(zones) = zones else {
        return;
    };
    let now = clock.tick;
    for (entity, pos, current_dimension, target, mut cultivation, mut meridians, life) in
        entities.iter_mut()
    {
        let dimension = current_dimension
            .map(|current| current.0)
            .unwrap_or(DimensionKind::Overworld);
        let zone_qi = zones
            .find_zone(dimension, pos.0)
            .filter(|zone| {
                !zone
                    .active_events
                    .iter()
                    .any(|event| event == EVENT_REALM_COLLAPSE)
            })
            .map(|z| z.spirit_qi)
            .unwrap_or(0.0);
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
                if let Some(meridian_opened_events) = meridian_opened_events.as_deref_mut() {
                    meridian_opened_events.send(MeridianOpenedEvent {
                        entity,
                        origin: pos.0,
                    });
                }
                if let Some(mut life) = life {
                    life.push(BiographyEntry::MeridianOpened {
                        id: target.0,
                        tick: now,
                    });
                    if life.spirit_root_first.is_none() {
                        life.spirit_root_first = Some(target.0);
                    }
                }
                if let Some(skill_xp_events) = skill_xp_events.as_deref_mut() {
                    skill_xp_events.send(SkillXpGain {
                        char_entity: entity,
                        skill: SkillId::Cultivation,
                        amount: 2,
                        source: XpGainSource::Action {
                            plan_id: "cultivation",
                            action: "meridian_open",
                        },
                    });
                }
                if let Some(events) = vfx_events.as_deref_mut() {
                    let p = pos.get() + valence::prelude::DVec3::new(0.0, 1.0, 0.0);
                    gameplay_vfx::send_spawn(
                        events,
                        gameplay_vfx::spawn_request(
                            gameplay_vfx::MERIDIAN_OPEN,
                            p,
                            Some(meridian_flash_direction(target.0)),
                            "#22FFAA",
                            1.0,
                            4,
                            20,
                        ),
                    );
                }
            }
        }
    }
}

fn meridian_flash_direction(target: MeridianId) -> [f64; 3] {
    match target {
        MeridianId::Lung | MeridianId::LargeIntestine | MeridianId::Heart => [0.5, -0.2, 0.0],
        MeridianId::Stomach | MeridianId::Spleen | MeridianId::SmallIntestine => [-0.5, -0.2, 0.0],
        MeridianId::Bladder | MeridianId::Kidney | MeridianId::Gallbladder => [0.0, -0.3, 0.5],
        MeridianId::Liver | MeridianId::Pericardium | MeridianId::TripleEnergizer => {
            [0.0, -0.3, -0.5]
        }
        MeridianId::Du
        | MeridianId::Ren
        | MeridianId::Chong
        | MeridianId::Dai
        | MeridianId::YinQiao
        | MeridianId::YangQiao
        | MeridianId::YinWei
        | MeridianId::YangWei => [0.0, 0.8, 0.0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::Cultivation;
    use crate::world::dimension::{CurrentDimension, DimensionKind};
    use crate::world::zone::ZoneRegistry;
    use valence::prelude::{App, Update};

    fn player_with_qi(qi: f64) -> Cultivation {
        Cultivation {
            qi_current: qi,
            qi_max: 10.0,
            ..Default::default()
        }
    }

    #[test]
    fn first_meridian_allows_regular_only() {
        let t = MeridianTopology::standard();
        let ms = MeridianSystem::default();
        assert!(is_target_adjacent(&t, &ms, MeridianId::Lung));
        assert!(!is_target_adjacent(&t, &ms, MeridianId::YangWei));
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

    #[test]
    fn collapsed_zone_blocks_meridian_open_progress_even_with_stale_qi() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 42 });
        app.insert_resource(MeridianTopology::standard());
        let mut zones = ZoneRegistry::fallback();
        let zone = zones.find_zone_mut("spawn").unwrap();
        zone.spirit_qi = 0.9;
        zone.active_events.push(EVENT_REALM_COLLAPSE.to_string());
        app.insert_resource(zones);
        app.add_systems(Update, meridian_open_tick);

        let player = app
            .world_mut()
            .spawn((
                Position::new([8.0, 66.0, 8.0]),
                MeridianTarget(MeridianId::Lung),
                player_with_qi(10.0),
                MeridianSystem::default(),
            ))
            .id();

        app.update();

        let cultivation = app.world().entity(player).get::<Cultivation>().unwrap();
        let meridians = app.world().entity(player).get::<MeridianSystem>().unwrap();
        assert_eq!(cultivation.qi_current, 10.0);
        assert_eq!(meridians.get(MeridianId::Lung).open_progress, 0.0);
        assert!(!meridians.get(MeridianId::Lung).opened);
    }

    #[test]
    fn meridian_open_uses_current_dimension_for_zone_lookup() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 42 });
        app.insert_resource(MeridianTopology::standard());
        let mut zones = ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = 0.9;
        app.insert_resource(zones);
        app.add_systems(Update, meridian_open_tick);

        let player = app
            .world_mut()
            .spawn((
                Position::new([8.0, 66.0, 8.0]),
                CurrentDimension(DimensionKind::Tsy),
                MeridianTarget(MeridianId::Lung),
                player_with_qi(10.0),
                MeridianSystem::default(),
            ))
            .id();

        app.update();

        let cultivation = app.world().entity(player).get::<Cultivation>().unwrap();
        let meridians = app.world().entity(player).get::<MeridianSystem>().unwrap();
        assert_eq!(cultivation.qi_current, 10.0);
        assert_eq!(meridians.get(MeridianId::Lung).open_progress, 0.0);
    }

    #[test]
    fn meridian_open_emits_vfx() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 42 });
        app.insert_resource(MeridianTopology::standard());
        let mut zones = ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = 1.0;
        app.insert_resource(zones);
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, meridian_open_tick);

        let mut cultivation = player_with_qi(1000.0);
        cultivation.qi_max = 1000.0;
        let mut meridians = MeridianSystem::default();
        meridians.get_mut(MeridianId::Lung).open_progress = 0.999;
        app.world_mut().spawn((
            Position::new([8.0, 66.0, 8.0]),
            MeridianTarget(MeridianId::Lung),
            cultivation,
            meridians,
        ));

        app.update();

        let events = app.world().resource::<Events<VfxEventRequest>>();
        let emitted = events
            .iter_current_update_events()
            .next()
            .expect("meridian open should emit vfx");
        match &emitted.payload {
            crate::schema::vfx_event::VfxEventPayloadV1::SpawnParticle { event_id, .. } => {
                assert_eq!(event_id, gameplay_vfx::MERIDIAN_OPEN);
            }
            other => panic!("expected SpawnParticle, got {other:?}"),
        }
    }

    #[test]
    fn meridian_open_emits_opened_event() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 42 });
        app.insert_resource(MeridianTopology::standard());
        let mut zones = ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = 1.0;
        app.insert_resource(zones);
        app.add_event::<MeridianOpenedEvent>();
        app.add_systems(Update, meridian_open_tick);

        let mut cultivation = player_with_qi(1000.0);
        cultivation.qi_max = 1000.0;
        let mut meridians = MeridianSystem::default();
        meridians.get_mut(MeridianId::Lung).open_progress = 0.999;
        let player = app
            .world_mut()
            .spawn((
                Position::new([8.0, 66.0, 8.0]),
                MeridianTarget(MeridianId::Lung),
                cultivation,
                meridians,
            ))
            .id();

        app.update();

        let events = app.world().resource::<Events<MeridianOpenedEvent>>();
        let emitted = events
            .iter_current_update_events()
            .next()
            .expect("meridian open should emit MeridianOpenedEvent");
        assert_eq!(emitted.entity, player);
        assert_eq!(emitted.origin, valence::prelude::DVec3::new(8.0, 66.0, 8.0));
    }
}
