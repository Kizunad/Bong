//! MeridianOpenTick（plan §2）— 玩家选定下一条经脉后，按 zone 浓度 +
//! qi 比例累积 `open_progress`，到 1.0 时打通、扩容 qi_max。
//!
//! P1 约束：
//!   * 目标必须与已打通经脉相邻（通过 `MeridianTopology`）
//!   * Awaken 期首脉特许（无已开经脉时允许任一正经）
//!   * zone.spirit_qi >= 0.3 才推进（阈值内不能打通）
//!   * 打通本身消耗 qi（cost = progress_delta × COST_FACTOR）

use valence::prelude::{bevy_ecs, Component, Entity, Event, Events, Position, Query, Res, ResMut};

use crate::body_plan::{
    resolve_meridian_topology_for_target, BodyPlanPurpose, BodyPlanRegistry, BodyPlanResolveInputs,
    RaceRegistry,
};
use crate::combat::components::StatusEffects;
use crate::combat::events::StatusEffectKind;
use crate::cultivation::tick::cultivation_acceleration_multiplier;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::events::EVENT_REALM_COLLAPSE;
use crate::world::zone::ZoneRegistry;

use super::components::{
    Cultivation, MeridianChannelId, MeridianFamily, MeridianId, MeridianSystem,
};
use super::life_record::{BiographyEntry, LifeRecord};
use super::tick::CultivationClock;
use super::topology::MeridianTopology;
use crate::network::{gameplay_vfx, vfx_event_emit::VfxEventRequest};
use crate::npc::spawn::NpcMarker;
use crate::qi_physics::{QiAccountId, QiTransferReason, WorldQiAccount};
use crate::skill::components::SkillId;
use crate::skill::events::{SkillXpGain, XpGainSource};

/// 玩家客户端发起的"选择下一条经脉"目标。未选目标时此 component 不存在。
///
/// plan-race-system-v1 P1b：从闭合枚举 `MeridianId` 换轨为 string
/// [`MeridianChannelId`]（非人形构型的经脉不在 20 个 TCM 名字之列）。内部推进逻辑
/// （`advance_open_progress_at`/`is_target_adjacent`）仍以 `MeridianId` 为参数类型
/// （humanoid-only boundary，本轮不迁移——与 `combat::baomai_v4` 同一惯例），
/// `meridian_open_tick` 在消费边界处经 [`MeridianChannelId::to_meridian_id`] 转换，
/// 非 humanoid channel（尚无非人形玩法数据）显式跳过而非 panic。
#[derive(Debug, Clone, Component)]
pub struct MeridianTarget(pub MeridianChannelId);

#[derive(Debug, Clone, Copy, Event)]
pub struct MeridianOpenedEvent {
    pub entity: Entity,
    pub origin: valence::prelude::DVec3,
}

pub const MIN_ZONE_QI_TO_OPEN: f64 = 0.3;
pub const BASE_OPEN_RATE: f64 = 0.00003;
pub const OPEN_COST_FACTOR: f64 = 5.0;
pub const MERIDIAN_CAPACITY_ON_OPEN: f64 = 10.0;

/// 逐脉难度递增：已开经脉越多，后续经脉打通越慢。
/// `progression` 对所有经脉适用（1/(1 + count*0.15)），
/// 奇经额外乘以 0.4 系数（天赋稀缺资源）。
pub fn meridian_difficulty_factor(opened_count: usize, family: MeridianFamily) -> f64 {
    let progression = 1.0 / (1.0 + opened_count as f64 * 0.15);
    let family_mult = match family {
        MeridianFamily::Regular => 1.0,
        MeridianFamily::Extraordinary => 0.4,
    };
    progression * family_mult
}

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
    Option<&'a NpcMarker>,
    // plan-cultivation-pacing-v1 P1.3：StatusEffects 用于 CultivationAcceleration
    // / ExtraordinaryMeridianAcceleration 聚合。
    Option<&'a StatusEffects>,
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
    advance_open_progress_at(cultivation, meridians, target, zone_qi, adjacent_ok, 0, 1.0)
        .map(|(delta, _just_opened)| delta)
}

/// 与 [`advance_open_progress`] 相同，但额外返回 "本次是否完成打通"，并在打通时写入
/// `opened_at = tick_now` 以支持 LIFO 排序。
///
/// `cultivation_boost`：外部（StatusEffects）聚合后的加速倍率，1.0 = 无加速。
pub fn advance_open_progress_at(
    cultivation: &mut Cultivation,
    meridians: &mut MeridianSystem,
    target: MeridianId,
    zone_qi: f64,
    adjacent_ok: bool,
    tick_now: u64,
    cultivation_boost: f64,
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
    let cultivation_boost = if cultivation_boost.is_finite() && cultivation_boost > 0.0 {
        cultivation_boost
    } else {
        1.0
    };
    let difficulty = meridian_difficulty_factor(meridians.opened_count(), target.family());
    let delta = BASE_OPEN_RATE * zone_qi * qi_ratio * difficulty * cultivation_boost;
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
        .any(|n| meridians.get(n.clone()).opened)
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub fn meridian_open_tick(
    body_plans: Option<Res<BodyPlanRegistry>>,
    races: Option<Res<RaceRegistry>>,
    clock: Res<CultivationClock>,
    zones: Option<Res<ZoneRegistry>>,
    mut qi_account: Option<ResMut<WorldQiAccount>>,
    mut entities: Query<MeridianOpenItem<'_>>,
    mut skill_xp_events: Option<ResMut<Events<SkillXpGain>>>,
    mut vfx_events: Option<ResMut<Events<VfxEventRequest>>>,
    mut meridian_opened_events: Option<ResMut<Events<MeridianOpenedEvent>>>,
) {
    let Some(zones) = zones else {
        return;
    };
    let now = clock.tick;
    for (
        entity,
        pos,
        current_dimension,
        target,
        mut cultivation,
        mut meridians,
        life,
        npc_marker,
        statuses,
    ) in entities.iter_mut()
    {
        // plan-race-system-v1 P1b：`MeridianTarget` 现为 `MeridianChannelId`——
        // `advance_open_progress_at`/`is_target_adjacent`/`meridian_difficulty_factor`
        // 仍是 humanoid-only boundary（本轮不迁移，legacy `MeridianId` 参数类型不变，
        // 见 `combat::baomai_v4` 同一惯例）。非 humanoid channel（尚无非人形玩法数据）
        // 显式跳过整个实体，而非 panic 或静默误判为某个哨兵经脉。
        let Some(target_meridian_id) = target.0.to_meridian_id() else {
            tracing::warn!(
                "[bong][cultivation][meridian_open] entity={:?} MeridianTarget channel {} has no \
                 legacy MeridianId mapping — meridian_open_tick cannot advance non-humanoid \
                 channels yet, skipping",
                entity,
                target.0,
            );
            continue;
        };
        let dimension = current_dimension
            .map(|current| current.0)
            .unwrap_or(DimensionKind::Overworld);
        let Some((zone_name, zone_qi)) = zones
            .find_zone(dimension, pos.0)
            .filter(|zone| {
                !zone
                    .active_events
                    .iter()
                    .any(|event| event == EVENT_REALM_COLLAPSE)
            })
            .map(|z| (z.name.clone(), z.spirit_qi))
        else {
            continue;
        };
        let Some(account) = qi_account.as_deref_mut() else {
            continue;
        };
        let topo = resolve_meridian_topology_for_target(
            entity,
            BodyPlanPurpose::Intrinsic,
            BodyPlanResolveInputs {
                cultivation: Some(&*cultivation),
                // `BeastKind` 不是 `Component`（既有 `combat::resolve`/`combat::carrier`
                // 消费点同款简化，见其文档）——races.json 现阶段全部 BeastKind 派生种族
                // 均落 humanoid body plan，`None` 与"真的查了 BeastKind"结果 bit-for-bit
                // 一致，P5 引入差异化非人形 NPC 经脉时需要回来补上。
                beast_kind: None,
            },
            body_plans.as_deref(),
            races.as_deref(),
        );
        let adj = is_target_adjacent(topo, &meridians, target_meridian_id);
        let cultivation_boost = {
            let accel = statuses
                .map(cultivation_acceleration_multiplier)
                .unwrap_or(1.0);
            let extra = if target_meridian_id.family() == MeridianFamily::Extraordinary {
                statuses
                    .map(extraordinary_meridian_acceleration_multiplier)
                    .unwrap_or(1.0)
            } else {
                1.0
            };
            (accel * extra).min(5.0)
        };
        let actor_account =
            meridian_open_actor_account_id(entity, life.as_deref(), npc_marker.is_some());
        if let Ok((delta, just_opened)) = advance_open_progress_at(
            &mut cultivation,
            &mut meridians,
            target_meridian_id,
            zone_qi,
            adj,
            now,
            cultivation_boost,
        ) {
            credit_meridian_open_cost(account, &zone_name, actor_account, delta * OPEN_COST_FACTOR);
            if just_opened {
                if let Some(meridian_opened_events) = meridian_opened_events.as_deref_mut() {
                    meridian_opened_events.send(MeridianOpenedEvent {
                        entity,
                        origin: pos.0,
                    });
                }
                if let Some(mut life) = life {
                    life.push(BiographyEntry::MeridianOpened {
                        id: target_meridian_id,
                        tick: now,
                    });
                    if life.spirit_root_first.is_none() {
                        life.spirit_root_first = Some(target_meridian_id);
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
                            Some(meridian_flash_direction(target.0.clone())),
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

fn meridian_open_actor_account_id(
    entity: Entity,
    life_record: Option<&LifeRecord>,
    is_npc: bool,
) -> QiAccountId {
    let id = life_record
        .and_then(|life_record| {
            let id = life_record.character_id.trim();
            (!id.is_empty()).then(|| life_record.character_id.clone())
        })
        .unwrap_or_else(|| format!("entity:{entity:?}"));
    if is_npc {
        QiAccountId::npc(id)
    } else {
        QiAccountId::player(id)
    }
}

/// plan-zone-qi-economy-v1 P0 §8.1 决议 #1：开脉消耗回充**独立待分配池**
/// （`qi_physics::ledger::credit_pending_inflow`），不再注水 audit-only 的
/// `zone:<name>` 账户（旧写法会被 `apply_dormant_regen_with_multiplier` 整体覆写、
/// 且从不写回 `zone.spirit_qi`——记账蒸发 bug 本身）。`meridian_open_tick` 沿用旧
/// "best-effort、失败静默丢弃"行为：进度/qi 扣减已经生效，ledger credit 失败不回滚
/// 玩家侧状态（与 `credit_active_breakthrough_cost` 的显式回滚分支不同——那里失败
/// 会撤销整次突破尝试）。
fn credit_meridian_open_cost(
    account: &mut WorldQiAccount,
    zone_name: &str,
    from: QiAccountId,
    amount: f64,
) {
    if let Err(error) = crate::qi_physics::credit_pending_inflow(
        account,
        zone_name,
        from,
        amount,
        QiTransferReason::MeridianOpen,
    ) {
        tracing::warn!(
            "[bong][cultivation] meridian_open pending inflow credit failed zone={} amount={} error={:?}",
            zone_name,
            amount,
            error
        );
    }
}

/// plan-cultivation-pacing-v1 P1.5：仅加速奇经打通，magnitude N → (1+N)× 奇经开脉速度。
/// 无上限——丹药系统控制投入量。
fn extraordinary_meridian_acceleration_multiplier(se: &StatusEffects) -> f64 {
    let sum: f32 = se
        .active
        .iter()
        .filter(|e| {
            e.kind == StatusEffectKind::ExtraordinaryMeridianAcceleration && e.remaining_ticks > 0
        })
        .map(|e| e.magnitude.max(0.0))
        .sum();
    1.0 + sum as f64
}

/// plan-race-system-v1 P1b：签名换轨为 [`MeridianChannelId`]（`MeridianTarget` 的新
/// 字段类型），内部经 [`MeridianChannelId::to_meridian_id`] 桥回 legacy 枚举复用既有
/// 8 方向分组（纯视觉 VFX 方向，非 humanoid channel 尚无对应美术语义）。非 humanoid /
/// 未知 channel id 落一个中性正上方向（`[0.0, 0.8, 0.0]`，与既有奇经分组同向）——
/// 仅影响粒子朝向的美术细节，不阻塞打通逻辑本身，故显式默认而非 panic。
fn meridian_flash_direction(target: MeridianChannelId) -> [f64; 3] {
    match target.to_meridian_id() {
        Some(MeridianId::Lung | MeridianId::LargeIntestine | MeridianId::Heart) => [0.5, -0.2, 0.0],
        Some(MeridianId::Stomach | MeridianId::Spleen | MeridianId::SmallIntestine) => {
            [-0.5, -0.2, 0.0]
        }
        Some(MeridianId::Bladder | MeridianId::Kidney | MeridianId::Gallbladder) => {
            [0.0, -0.3, 0.5]
        }
        Some(MeridianId::Liver | MeridianId::Pericardium | MeridianId::TripleEnergizer) => {
            [0.0, -0.3, -0.5]
        }
        Some(
            MeridianId::Du
            | MeridianId::Ren
            | MeridianId::Chong
            | MeridianId::Dai
            | MeridianId::YinQiao
            | MeridianId::YangQiao
            | MeridianId::YinWei
            | MeridianId::YangWei,
        )
        | None => [0.0, 0.8, 0.0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::Cultivation;
    use crate::world::dimension::{CurrentDimension, DimensionKind};
    use crate::world::zone::ZoneRegistry;
    use valence::prelude::{App, Update};

    fn add_meridian_open_system(app: &mut App) {
        app.insert_resource(WorldQiAccount::default());
        app.add_systems(Update, meridian_open_tick);
    }

    fn player_with_qi(qi: f64) -> Cultivation {
        Cultivation {
            qi_current: qi,
            qi_max: 10.0,
            ..Default::default()
        }
    }

    #[test]
    fn first_meridian_allows_regular_only() {
        let t = crate::body_plan::humanoid_topology_static();
        let ms = MeridianSystem::default();
        assert!(is_target_adjacent(t, &ms, MeridianId::Lung));
        assert!(!is_target_adjacent(t, &ms, MeridianId::YangWei));
    }

    #[test]
    fn second_meridian_requires_real_adjacency() {
        let t = crate::body_plan::humanoid_topology_static();
        let mut ms = MeridianSystem::default();
        ms.get_mut(MeridianId::Lung).opened = true;
        assert!(is_target_adjacent(t, &ms, MeridianId::LargeIntestine));
        assert!(!is_target_adjacent(t, &ms, MeridianId::Stomach));
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
        for _ in 0..35_000 {
            let _ = advance_open_progress(&mut c, &mut ms, MeridianId::Lung, 1.0, true);
            if ms.get(MeridianId::Lung).opened {
                break;
            }
        }
        assert!(ms.get(MeridianId::Lung).opened);
        assert!(c.qi_max >= 1000.0 + MERIDIAN_CAPACITY_ON_OPEN);
    }

    #[test]
    fn meridian_open_requires_qi_account_before_consuming_qi() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 42 });
        let mut zones = ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = 1.0;
        app.insert_resource(zones);
        app.add_systems(Update, meridian_open_tick);

        let mut cultivation = player_with_qi(1000.0);
        cultivation.qi_max = 1000.0;
        let player = app
            .world_mut()
            .spawn((
                Position::new([8.0, 66.0, 8.0]),
                MeridianTarget(MeridianId::Lung.channel_id()),
                cultivation,
                MeridianSystem::default(),
            ))
            .id();

        app.update();

        let cultivation = app.world().entity(player).get::<Cultivation>().unwrap();
        let meridians = app.world().entity(player).get::<MeridianSystem>().unwrap();
        assert_eq!(cultivation.qi_current, 1000.0);
        assert_eq!(meridians.get(MeridianId::Lung).open_progress, 0.0);
    }

    #[test]
    fn meridian_open_cost_is_credited_to_pending_inflow_pool_not_zone_ledger() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 42 });
        let mut zones = ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = 1.0;
        app.insert_resource(zones);
        add_meridian_open_system(&mut app);

        let mut cultivation = player_with_qi(1000.0);
        cultivation.qi_max = 1000.0;
        let player = app
            .world_mut()
            .spawn((
                Position::new([8.0, 66.0, 8.0]),
                MeridianTarget(MeridianId::Lung.channel_id()),
                cultivation,
                MeridianSystem::default(),
            ))
            .id();

        app.update();

        let expected_delta = BASE_OPEN_RATE;
        let expected_cost = expected_delta * OPEN_COST_FACTOR;
        let cultivation = app.world().entity(player).get::<Cultivation>().unwrap();
        assert!((cultivation.qi_current - (1000.0 - expected_cost)).abs() < 1e-12);

        let account = app.world().resource::<WorldQiAccount>();
        // plan-zone-qi-economy-v1 P0 §8.1 决议 #1：目标账户是独立待分配池，不是
        // zone:<name>（该 key 会被 dormant regen 整体覆写，credit 进去等于蒸发）。
        let pending_pool = crate::qi_physics::pending_inflow_account();
        let zone_account = QiAccountId::zone("spawn");
        assert!(
            (account.balance(&pending_pool) - expected_cost).abs() < 1e-12,
            "meridian open cost must credit the independent pending inflow pool, got {}",
            account.balance(&pending_pool)
        );
        assert_eq!(
            account.balance(&zone_account),
            0.0,
            "meridian open must never touch the zone:<name> ledger account (dormant regen \
             overwrites it wholesale from zone.spirit_qi, clobbering any credit stuffed there)"
        );
        let transfer = account
            .transfers()
            .iter()
            .find(|transfer| transfer.reason == QiTransferReason::MeridianOpen)
            .expect("meridian open should append a MeridianOpen audit record");
        assert_eq!(transfer.from.kind, crate::qi_physics::QiAccountKind::Player);
        assert_eq!(transfer.to, pending_pool);
        assert!((transfer.amount - expected_cost).abs() < 1e-12);
    }

    #[test]
    fn meridian_open_preserves_total_observed_qi_conservation() {
        // plan-zone-qi-economy-v1 P0 §10.3 — 开脉消耗真元回充守恒对拍：total_observed()
        // = player_qi + zone_qi + container_qi + ledger_qi（含待分配池），必须严格不变。
        use crate::qi_physics::{assert_conservation, summarize_world_qi};

        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 42 });
        let mut zones = ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = 1.0;
        app.insert_resource(zones);
        add_meridian_open_system(&mut app);

        let mut cultivation = player_with_qi(1000.0);
        cultivation.qi_max = 1000.0;
        let player = app
            .world_mut()
            .spawn((
                Position::new([8.0, 66.0, 8.0]),
                MeridianTarget(MeridianId::Lung.channel_id()),
                cultivation,
                MeridianSystem::default(),
            ))
            .id();

        let before = summarize_world_qi(app.world_mut());
        app.update();
        let cultivation = app.world().entity(player).get::<Cultivation>().unwrap();
        assert!(
            cultivation.qi_current < 1000.0,
            "sanity: meridian open must actually spend qi for this conservation test to be \
             meaningful"
        );
        let after = summarize_world_qi(app.world_mut());

        assert_conservation(&before, &after, 0.0).unwrap_or_else(|error| {
            panic!(
                "meridian open must conserve total_observed qi with zero era decay — got \
                 drift: {error} (before={before:?}, after={after:?}); a mismatch means spent \
                 qi vanished instead of reaching the pending inflow pool"
            )
        });
    }

    #[test]
    fn collapsed_zone_blocks_meridian_open_progress_even_with_stale_qi() {
        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 42 });
        let mut zones = ZoneRegistry::fallback();
        let zone = zones.find_zone_mut("spawn").unwrap();
        zone.spirit_qi = 0.9;
        zone.active_events.push(EVENT_REALM_COLLAPSE.to_string());
        app.insert_resource(zones);
        add_meridian_open_system(&mut app);

        let player = app
            .world_mut()
            .spawn((
                Position::new([8.0, 66.0, 8.0]),
                MeridianTarget(MeridianId::Lung.channel_id()),
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
        let mut zones = ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = 0.9;
        app.insert_resource(zones);
        add_meridian_open_system(&mut app);

        let player = app
            .world_mut()
            .spawn((
                Position::new([8.0, 66.0, 8.0]),
                CurrentDimension(DimensionKind::Tsy),
                MeridianTarget(MeridianId::Lung.channel_id()),
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
        let mut zones = ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = 1.0;
        app.insert_resource(zones);
        app.add_event::<VfxEventRequest>();
        add_meridian_open_system(&mut app);

        let mut cultivation = player_with_qi(1000.0);
        cultivation.qi_max = 1000.0;
        let mut meridians = MeridianSystem::default();
        // open_progress 必须离 1.0 足够近，使得一次 delta（BASE_OPEN_RATE * zone_qi * qi_ratio
        // = 0.00003 * 1.0 * 1.0 = 0.00003）能跨过 1.0 门槛。
        meridians.get_mut(MeridianId::Lung).open_progress = 1.0 - 1e-6;
        app.world_mut().spawn((
            Position::new([8.0, 66.0, 8.0]),
            MeridianTarget(MeridianId::Lung.channel_id()),
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
        let mut zones = ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = 1.0;
        app.insert_resource(zones);
        app.add_event::<MeridianOpenedEvent>();
        add_meridian_open_system(&mut app);

        let mut cultivation = player_with_qi(1000.0);
        cultivation.qi_max = 1000.0;
        let mut meridians = MeridianSystem::default();
        // open_progress 必须离 1.0 足够近（同 emits_vfx 测试理由）。
        meridians.get_mut(MeridianId::Lung).open_progress = 1.0 - 1e-6;
        let player = app
            .world_mut()
            .spawn((
                Position::new([8.0, 66.0, 8.0]),
                MeridianTarget(MeridianId::Lung.channel_id()),
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

    // ── P0.3 meridian_difficulty_factor pin 测试 ──

    fn assert_close(actual: f64, expected: f64, label: &str) {
        assert!(
            (actual - expected).abs() < 0.001,
            "{label}: 期望 {expected:.4} 因为公式 1/(1+count*0.15)*family_mult，实际 {actual:.4}"
        );
    }

    #[test]
    fn difficulty_factor_opened_0_regular() {
        assert_close(
            meridian_difficulty_factor(0, MeridianFamily::Regular),
            1.0,
            "opened=0 Regular",
        );
    }

    #[test]
    fn difficulty_factor_opened_3_regular() {
        // 1/(1+3*0.15) = 1/1.45 ≈ 0.6897
        assert_close(
            meridian_difficulty_factor(3, MeridianFamily::Regular),
            0.6897,
            "opened=3 Regular",
        );
    }

    #[test]
    fn difficulty_factor_opened_6_regular() {
        // 1/(1+6*0.15) = 1/1.9 ≈ 0.5263
        assert_close(
            meridian_difficulty_factor(6, MeridianFamily::Regular),
            0.5263,
            "opened=6 Regular",
        );
    }

    #[test]
    fn difficulty_factor_opened_12_regular() {
        // 1/(1+12*0.15) = 1/2.8 ≈ 0.3571
        assert_close(
            meridian_difficulty_factor(12, MeridianFamily::Regular),
            0.3571,
            "opened=12 Regular",
        );
    }

    #[test]
    fn difficulty_factor_opened_0_extraordinary() {
        // 1.0 * 0.4 = 0.4
        assert_close(
            meridian_difficulty_factor(0, MeridianFamily::Extraordinary),
            0.4,
            "opened=0 Extraordinary",
        );
    }

    #[test]
    fn difficulty_factor_opened_3_extraordinary() {
        // 0.6897 * 0.4 ≈ 0.2759
        assert_close(
            meridian_difficulty_factor(3, MeridianFamily::Extraordinary),
            0.2759,
            "opened=3 Extraordinary",
        );
    }

    #[test]
    fn difficulty_factor_opened_6_extraordinary() {
        // 0.5263 * 0.4 ≈ 0.2105
        assert_close(
            meridian_difficulty_factor(6, MeridianFamily::Extraordinary),
            0.2105,
            "opened=6 Extraordinary",
        );
    }

    #[test]
    fn difficulty_factor_opened_12_extraordinary() {
        // 0.3571 * 0.4 ≈ 0.1429
        assert_close(
            meridian_difficulty_factor(12, MeridianFamily::Extraordinary),
            0.1429,
            "opened=12 Extraordinary",
        );
    }

    // ── P0.3 首条正经开脉耗时测试 ──

    #[test]
    fn first_regular_meridian_tick_count_matches_expected() {
        // zone_qi=0.6, qi_ratio≈0.8 (8000/10000), opened_count=0, Regular
        // delta = 0.00003 * 0.6 * 0.8 * 1.0 = 0.0000144
        // ticks = ceil(1.0/0.0000144) = 69445
        // qi 充足（qi_current=8000）使 qi_ratio 在循环内微降可忽略。
        let mut c = Cultivation {
            qi_current: 8000.0,
            qi_max: 10000.0,
            ..Default::default()
        };
        let mut ms = MeridianSystem::default();
        let target = MeridianId::Lung;

        // 69400 次后不应 opened
        for _ in 0..69_400 {
            let _ = advance_open_progress(&mut c, &mut ms, target, 0.6, true);
        }
        assert!(
            !ms.get(target).opened,
            "首条正经在 69400 tick 后不应打通（delta≈0.0000144，需 ~69445 tick）；\
             实际 open_progress={:.6}",
            ms.get(target).open_progress
        );

        // 再跑到 69600 次后应已 opened
        for _ in 0..200 {
            let _ = advance_open_progress(&mut c, &mut ms, target, 0.6, true);
            if ms.get(target).opened {
                break;
            }
        }
        assert!(
            ms.get(target).opened,
            "首条正经在 69600 tick 后应已打通；实际 open_progress={:.6}",
            ms.get(target).open_progress
        );
    }

    // ── P0.3 首条奇经开脉 delta 测试（不循环，直接验 delta 值）──

    #[test]
    fn first_extraordinary_meridian_delta_matches_expected() {
        let mut c = Cultivation {
            qi_current: 10000.0,
            qi_max: 10000.0,
            ..Default::default()
        };
        let mut ms = MeridianSystem::default();
        // 模拟已开 12 条正经
        for &id in MeridianId::REGULAR.iter() {
            ms.get_mut(id).opened = true;
        }
        let target = MeridianId::Ren; // 奇经，首脉特许不适用但 adjacent_ok=true 跳过检查

        let (delta, _just_opened) =
            advance_open_progress_at(&mut c, &mut ms, target, 0.6, true, 0, 1.0)
                .expect("应成功推进奇经进度");

        // expected: 0.00003 * 0.6 * (10000/10000) * meridian_difficulty_factor(12, Extraordinary)
        //         = 0.00003 * 0.6 * 1.0 * (0.4 / 2.8)
        //         = 0.00003 * 0.6 * 0.14286
        //         = 0.000002571
        let ticks_to_open = (1.0 / delta).ceil() as u64;
        assert!(
            ticks_to_open >= 380_000,
            "奇经开脉应极慢（opened=12, Extraordinary），\
             期望 ≥380000 tick 因为 delta≈2.57e-6，实际 {ticks_to_open} tick (delta={delta:.9})"
        );
        assert!(
            ticks_to_open <= 500_000,
            "奇经开脉不应超出合理范围，\
             期望 ≤500000 tick，实际 {ticks_to_open} tick (delta={delta:.9})"
        );
    }

    // ── plan-cultivation-pacing-v1 P1.3 meridian delta 在 CultivationAcceleration 下的测试 ──

    #[test]
    fn meridian_delta_tripled_under_cultivation_acceleration_mag_two() {
        let mut c = Cultivation {
            qi_current: 1000.0,
            qi_max: 1000.0,
            ..Default::default()
        };
        let mut ms = MeridianSystem::default();
        let target = MeridianId::Lung;

        // 基线：boost=1.0
        let (delta_base, _) =
            advance_open_progress_at(&mut c, &mut ms, target, 0.6, true, 0, 1.0).unwrap();

        // 重置 progress
        ms.get_mut(target).open_progress = 0.0;
        c.qi_current = 1000.0;

        // boost=3.0 (CultivationAcceleration mag=2.0)
        let (delta_boosted, _) =
            advance_open_progress_at(&mut c, &mut ms, target, 0.6, true, 0, 3.0).unwrap();

        assert!(
            (delta_boosted - delta_base * 3.0).abs() < 1e-12,
            "cultivation_boost=3.0 应使 delta 3×；\
             期望 {:.12}，实际 {:.12}",
            delta_base * 3.0,
            delta_boosted
        );
    }

    // ── plan-cultivation-pacing-v1 P1.5 ExtraordinaryMeridianAcceleration 测试 ──

    #[test]
    fn extraordinary_accel_only_affects_extraordinary_meridian() {
        use crate::combat::components::{ActiveStatusEffect, StatusEffects};

        let extra_accel_effects = StatusEffects {
            active: vec![ActiveStatusEffect {
                kind: crate::combat::events::StatusEffectKind::ExtraordinaryMeridianAcceleration,
                magnitude: 4.0,
                remaining_ticks: 100,
                source_pill: None,
            }],
        };

        // 对正经无效
        let regular_boost = {
            let accel = cultivation_acceleration_multiplier(&extra_accel_effects);
            // ExtraordinaryMeridianAcceleration is NOT CultivationAcceleration, so accel=1.0
            let extra = 1.0; // Regular meridian → no extra boost
            (accel * extra).min(5.0)
        };
        assert!(
            (regular_boost - 1.0).abs() < 1e-9,
            "ExtraordinaryMeridianAcceleration 对正经应无效；实际 boost={regular_boost}"
        );

        // 对奇经有效：boost = 1.0 * (1 + 4.0) = 5.0
        let extraordinary_boost = {
            let accel = cultivation_acceleration_multiplier(&extra_accel_effects);
            let extra = extraordinary_meridian_acceleration_multiplier(&extra_accel_effects);
            (accel * extra).min(5.0)
        };
        assert!(
            (extraordinary_boost - 5.0).abs() < 1e-9,
            "ExtraordinaryMeridianAcceleration(mag=4.0) 对奇经应 (1+4)=5.0× 但被 min(5.0) cap；\
             实际 boost={extraordinary_boost}"
        );
    }

    #[test]
    fn extraordinary_accel_multiplier_basic() {
        use crate::combat::components::{ActiveStatusEffect, StatusEffects};

        let se = StatusEffects {
            active: vec![ActiveStatusEffect {
                kind: crate::combat::events::StatusEffectKind::ExtraordinaryMeridianAcceleration,
                magnitude: 4.0,
                remaining_ticks: 100,
                source_pill: None,
            }],
        };
        let result = extraordinary_meridian_acceleration_multiplier(&se);
        assert!(
            (result - 5.0).abs() < 1e-9,
            "mag=4.0 应返回 5.0×；实际 {result}"
        );
    }

    #[test]
    fn extraordinary_accel_expired_not_counted() {
        use crate::combat::components::{ActiveStatusEffect, StatusEffects};

        let se = StatusEffects {
            active: vec![ActiveStatusEffect {
                kind: crate::combat::events::StatusEffectKind::ExtraordinaryMeridianAcceleration,
                magnitude: 4.0,
                remaining_ticks: 0,
                source_pill: None,
            }],
        };
        let result = extraordinary_meridian_acceleration_multiplier(&se);
        assert!(
            (result - 1.0).abs() < 1e-9,
            "remaining_ticks=0 不应计入；实际 {result}"
        );
    }

    #[test]
    fn cultivation_boost_zero_falls_back_to_one() {
        let mut c = player_with_qi(10000.0);
        c.qi_max = 10000.0;
        let mut ms = MeridianSystem::default();
        let (delta_default, _) =
            advance_open_progress_at(&mut c, &mut ms, MeridianId::Lung, 0.6, true, 0, 1.0).unwrap();

        let mut c2 = player_with_qi(10000.0);
        c2.qi_max = 10000.0;
        let mut ms2 = MeridianSystem::default();
        let (delta_zero, _) =
            advance_open_progress_at(&mut c2, &mut ms2, MeridianId::Lung, 0.6, true, 0, 0.0)
                .unwrap();

        assert!(
            (delta_default - delta_zero).abs() < 1e-15,
            "cultivation_boost=0.0 应 fallback 到 1.0，delta_default={delta_default} delta_zero={delta_zero}"
        );
    }

    #[test]
    fn cultivation_boost_negative_falls_back_to_one() {
        let mut c = player_with_qi(10000.0);
        c.qi_max = 10000.0;
        let mut ms = MeridianSystem::default();
        let (delta_default, _) =
            advance_open_progress_at(&mut c, &mut ms, MeridianId::Lung, 0.6, true, 0, 1.0).unwrap();

        let mut c2 = player_with_qi(10000.0);
        c2.qi_max = 10000.0;
        let mut ms2 = MeridianSystem::default();
        let (delta_neg, _) =
            advance_open_progress_at(&mut c2, &mut ms2, MeridianId::Lung, 0.6, true, 0, -5.0)
                .unwrap();

        assert!(
            (delta_default - delta_neg).abs() < 1e-15,
            "cultivation_boost=-5.0 应 fallback 到 1.0"
        );
    }

    #[test]
    fn cultivation_boost_nan_falls_back_to_one() {
        let mut c = player_with_qi(10000.0);
        c.qi_max = 10000.0;
        let mut ms = MeridianSystem::default();
        let (delta_default, _) =
            advance_open_progress_at(&mut c, &mut ms, MeridianId::Lung, 0.6, true, 0, 1.0).unwrap();

        let mut c2 = player_with_qi(10000.0);
        c2.qi_max = 10000.0;
        let mut ms2 = MeridianSystem::default();
        let (delta_nan, _) =
            advance_open_progress_at(&mut c2, &mut ms2, MeridianId::Lung, 0.6, true, 0, f64::NAN)
                .unwrap();

        assert!(
            (delta_default - delta_nan).abs() < 1e-15,
            "cultivation_boost=NaN 应 fallback 到 1.0"
        );
    }

    #[test]
    fn extraordinary_accel_no_cap() {
        use crate::combat::components::{ActiveStatusEffect, StatusEffects};

        // 单独函数不 cap（cap 在 meridian_open_tick 聚合时做）
        let se = StatusEffects {
            active: vec![ActiveStatusEffect {
                kind: crate::combat::events::StatusEffectKind::ExtraordinaryMeridianAcceleration,
                magnitude: 10.0,
                remaining_ticks: 100,
                source_pill: None,
            }],
        };
        let result = extraordinary_meridian_acceleration_multiplier(&se);
        assert!(
            (result - 11.0).abs() < 1e-9,
            "extraordinary_meridian_acceleration_multiplier 本身无 cap；mag=10 应返回 11.0×，实际 {result}"
        );
    }
}
