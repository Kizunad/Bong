//! 经脉锻造（plan §3.2）。两条独立轴：
//!   * 流速 `flow_rate` — 每 tick 真元吞吐上限
//!   * 容量 `flow_capacity` — 经脉可同时承载的真元总量
//!
//! P1 只放开到 tier 3（计划约定），tier 跃升需要消耗 qi 并抽成 integrity
//! （代表经脉扩张的损伤 — 后续 contamination 切片再强化）。

use valence::prelude::{
    bevy_ecs, Entity, Event, EventReader, EventWriter, Position, Query, Res, ResMut,
};

use super::components::{Cultivation, Meridian, MeridianChannelId, MeridianSystem};
use super::life_record::{BiographyEntry, LifeRecord};
use super::tick::CultivationClock;
use crate::npc::spawn::NpcMarker;
use crate::qi_physics::{QiAccountId, QiTransfer, QiTransferReason, WorldQiAccount};
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::zone::ZoneRegistry;

pub const P1_MAX_TIER: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ForgeAxis {
    Rate,
    Capacity,
}

#[derive(Debug, Clone, Event)]
pub struct ForgeRequest {
    pub entity: Entity,
    pub meridian: MeridianChannelId,
    pub axis: ForgeAxis,
}

#[derive(Debug, Clone, Event)]
pub struct ForgeOutcome {
    pub entity: Entity,
    pub meridian: MeridianChannelId,
    pub axis: ForgeAxis,
    pub result: Result<u8, ForgeError>, // Ok(new_tier)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ForgeError {
    MeridianClosed,
    AtMaxTier,
    NotEnoughQi { need: f64, have: f64 },
    LedgerUnavailable,
    /// plan-race-system-v1 P1 对抗审查 B1：wire 上的 channel id 不属于该实体的
    /// `MeridianSystem`（未知构型 / 伪造串 / 旧 PascalCase `MeridianId` 字面量如
    /// `"Lung"`）——安全拒绝，绝不 panic。
    UnknownChannel,
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

fn forge_next_tier(
    cultivation: &Cultivation,
    meridian: &Meridian,
    axis: ForgeAxis,
) -> Result<(u8, f64), ForgeError> {
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
    Ok((next, cost))
}

/// 纯函数 — 在单条 `meridian` 上锻造一次指定轴。消耗从 `cultivation` 扣。
pub fn try_forge(
    cultivation: &mut Cultivation,
    meridian: &mut Meridian,
    axis: ForgeAxis,
) -> Result<u8, ForgeError> {
    let (next, cost) = forge_next_tier(cultivation, meridian, axis)?;

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

type ForgeQueryItem<'a> = (
    &'a Position,
    Option<&'a CurrentDimension>,
    &'a mut Cultivation,
    &'a mut MeridianSystem,
    &'a mut LifeRecord,
    Option<&'a NpcMarker>,
);

pub fn forging_system(
    clock: Res<CultivationClock>,
    zones: Option<Res<ZoneRegistry>>,
    mut qi_account: Option<ResMut<WorldQiAccount>>,
    mut requests: EventReader<ForgeRequest>,
    mut outcomes: EventWriter<ForgeOutcome>,
    mut players: Query<ForgeQueryItem<'_>>,
) {
    let now = clock.tick;
    for req in requests.read() {
        let Ok((position, current_dimension, mut cultivation, mut meridians, mut life, npc_marker)) =
            players.get_mut(req.entity)
        else {
            continue;
        };
        let result = forge_with_ledger(
            &mut cultivation,
            &mut meridians,
            &life,
            npc_marker.is_some(),
            position,
            current_dimension,
            zones.as_deref(),
            qi_account.as_deref_mut(),
            req.meridian.clone(),
            req.axis,
        );
        match &result {
            Ok(tier) => {
                // life_record::BiographyEntry 仍以闭合 MeridianId 为公共接口（plan §7
                // 撞车面声明范围外，非本轮 wire 开放化任务）——非 humanoid channel（P5
                // 飞鲸等）没有对应 MeridianId 时显式跳过传记条目（不伪造哨兵 id），只记
                // debug 日志，不影响锻造结算本身。
                match req.meridian.to_meridian_id() {
                    Some(legacy_id) => {
                        let entry = match req.axis {
                            ForgeAxis::Rate => BiographyEntry::ForgedRate {
                                id: legacy_id,
                                tier: *tier,
                                tick: now,
                            },
                            ForgeAxis::Capacity => BiographyEntry::ForgedCapacity {
                                id: legacy_id,
                                tier: *tier,
                                tick: now,
                            },
                        };
                        life.push(entry);
                    }
                    None => {
                        tracing::debug!(
                            "[bong][cultivation] forge biography skipped for non-humanoid \
                             channel {:?} (no legacy MeridianId mapping)",
                            req.meridian
                        );
                    }
                }
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
            meridian: req.meridian.clone(),
            axis: req.axis,
            result,
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn forge_with_ledger(
    cultivation: &mut Cultivation,
    meridians: &mut MeridianSystem,
    life: &LifeRecord,
    is_npc: bool,
    position: &Position,
    current_dimension: Option<&CurrentDimension>,
    zones: Option<&ZoneRegistry>,
    qi_account: Option<&mut WorldQiAccount>,
    meridian_id: MeridianChannelId,
    axis: ForgeAxis,
) -> Result<u8, ForgeError> {
    if !meridians.contains(meridian_id.clone()) {
        return Err(ForgeError::UnknownChannel);
    }
    let (_, cost) = forge_next_tier(cultivation, meridians.get(meridian_id.clone()), axis)?;
    let zone_name = resolve_forge_zone_name(zones, position, current_dimension)
        .ok_or(ForgeError::LedgerUnavailable)?;
    let actor_account = forge_actor_account_id(life, is_npc)?;
    let Some(account) = qi_account else {
        return Err(ForgeError::LedgerUnavailable);
    };

    let cultivation_before = cultivation.clone();
    let meridians_before = meridians.clone();
    let result = try_forge(cultivation, meridians.get_mut(meridian_id), axis);
    if result.is_ok()
        && credit_forge_cost(account, zone_name.as_str(), actor_account, cost).is_err()
    {
        *cultivation = cultivation_before;
        *meridians = meridians_before;
        return Err(ForgeError::LedgerUnavailable);
    }
    result
}

fn resolve_forge_zone_name(
    zones: Option<&ZoneRegistry>,
    position: &Position,
    current_dimension: Option<&CurrentDimension>,
) -> Option<String> {
    let dimension = current_dimension
        .map(|current| current.0)
        .unwrap_or(DimensionKind::Overworld);
    zones.and_then(|zones| {
        zones
            .find_zone(dimension, position.get())
            .map(|zone| zone.name.clone())
    })
}

fn forge_actor_account_id(
    life_record: &LifeRecord,
    is_npc: bool,
) -> Result<QiAccountId, ForgeError> {
    let id = life_record.character_id.trim();
    if id.is_empty() {
        return Err(ForgeError::LedgerUnavailable);
    }
    if is_npc {
        Ok(QiAccountId::npc(life_record.character_id.clone()))
    } else {
        Ok(QiAccountId::player(life_record.character_id.clone()))
    }
}

fn credit_forge_cost(
    account: &mut WorldQiAccount,
    zone_name: &str,
    from: QiAccountId,
    amount: f64,
) -> Result<(), ForgeError> {
    if amount <= 0.0 {
        return Ok(());
    }
    let to = QiAccountId::zone(zone_name.to_string());
    let transfer = QiTransfer::new(from, to.clone(), amount, QiTransferReason::MeridianForge)
        .map_err(|_| ForgeError::LedgerUnavailable)?;
    if !account.has_account(&to) {
        account
            .set_balance(to.clone(), 0.0)
            .map_err(|_| ForgeError::LedgerUnavailable)?;
    }
    let zone_balance = account.balance(&to);
    account
        .set_balance(to, zone_balance + amount)
        .map_err(|_| ForgeError::LedgerUnavailable)?;
    account.push_transfer_audit(transfer);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::MeridianId;
    use crate::qi_physics::QiAccountKind;
    use crate::world::zone::ZoneRegistry;
    use valence::prelude::{App, Events, Update};

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

    fn add_forging_system(app: &mut App, with_ledger: bool) {
        app.insert_resource(CultivationClock { tick: 7 });
        app.insert_resource(ZoneRegistry::fallback());
        if with_ledger {
            app.insert_resource(WorldQiAccount::default());
        }
        app.add_event::<ForgeRequest>();
        app.add_event::<ForgeOutcome>();
        app.add_systems(Update, forging_system);
    }

    fn spawn_open_lung_player(app: &mut App, qi: f64) -> Entity {
        let (mut cultivation, meridians) = open_lung();
        cultivation.qi_current = qi;
        app.world_mut()
            .spawn((
                Position::new([8.0, 66.0, 8.0]),
                cultivation,
                meridians,
                LifeRecord::new("player_a"),
            ))
            .id()
    }

    fn collect_forge_outcomes(app: &App) -> Vec<ForgeOutcome> {
        let events = app.world().resource::<Events<ForgeOutcome>>();
        let mut reader = events.get_reader();
        reader.read(events).cloned().collect()
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
    fn forge_system_requires_ledger_before_consuming_qi() {
        let mut app = App::new();
        add_forging_system(&mut app, false);
        let player = spawn_open_lung_player(&mut app, 100.0);

        app.world_mut().send_event(ForgeRequest {
            entity: player,
            meridian: MeridianId::Lung.channel_id(),
            axis: ForgeAxis::Rate,
        });
        app.update();

        let outcomes = collect_forge_outcomes(&app);
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].result, Err(ForgeError::LedgerUnavailable));
        let player_ref = app.world().entity(player);
        let cultivation = player_ref.get::<Cultivation>().unwrap();
        let meridians = player_ref.get::<MeridianSystem>().unwrap();
        assert_eq!(cultivation.qi_current, 100.0);
        assert_eq!(meridians.get(MeridianId::Lung).rate_tier, 0);
    }

    #[test]
    fn forge_system_credits_spent_qi_to_zone_ledger() {
        let mut app = App::new();
        add_forging_system(&mut app, true);
        let player = spawn_open_lung_player(&mut app, 100.0);

        app.world_mut().send_event(ForgeRequest {
            entity: player,
            meridian: MeridianId::Lung.channel_id(),
            axis: ForgeAxis::Rate,
        });
        app.update();

        let outcomes = collect_forge_outcomes(&app);
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].result, Ok(1));
        let cost = tier_cost(1);
        let player_ref = app.world().entity(player);
        let cultivation = player_ref.get::<Cultivation>().unwrap();
        let meridians = player_ref.get::<MeridianSystem>().unwrap();
        assert_eq!(cultivation.qi_current, 100.0 - cost);
        assert_eq!(meridians.get(MeridianId::Lung).rate_tier, 1);

        let account = app.world().resource::<WorldQiAccount>();
        let zone_account = QiAccountId::zone("spawn");
        assert_eq!(account.balance(&zone_account), cost);
        let transfer = account
            .transfers()
            .iter()
            .find(|transfer| transfer.reason == QiTransferReason::MeridianForge)
            .expect("forge should append a MeridianForge audit record");
        assert_eq!(transfer.from.kind, QiAccountKind::Player);
        assert_eq!(transfer.from.id, "player_a");
        assert_eq!(transfer.to, zone_account);
        assert_eq!(transfer.amount, cost);
    }

    #[test]
    fn tier_cost_monotonic() {
        assert!(tier_cost(1) < tier_cost(2));
        assert!(tier_cost(2) < tier_cost(3));
    }

    // ── plan-race-system-v1 P1 对抗审查 B1：forge_request 消费边界必须安全拒绝未知
    // channel id，绝不 panic（`MeridianSystem::get`/`get_mut` 对未知 id 是 panic 分支，
    // `forge_with_ledger` 现在必须在到达它们之前用 `MeridianSystem::contains` 拦截）。

    #[test]
    fn forge_with_ledger_rejects_unknown_channel_id_without_panicking() {
        let mut app = App::new();
        add_forging_system(&mut app, true);
        let player = spawn_open_lung_player(&mut app, 100.0);

        app.world_mut().send_event(ForgeRequest {
            entity: player,
            meridian: MeridianChannelId::new("totally_made_up_channel"),
            axis: ForgeAxis::Rate,
        });
        // 必须不 panic —— 这是本用例的核心断言。
        app.update();

        let outcomes = collect_forge_outcomes(&app);
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].result, Err(ForgeError::UnknownChannel));
        let player_ref = app.world().entity(player);
        let cultivation = player_ref.get::<Cultivation>().unwrap();
        assert_eq!(
            cultivation.qi_current, 100.0,
            "unknown channel 请求必须完全无副作用——qi 不应被扣除"
        );
    }

    #[test]
    fn forge_with_ledger_rejects_legacy_pascal_case_lung_string_as_unknown_channel() {
        // `MeridianChannelId` 的规范形态是 snake_case（如 "lung"）；旧 `MeridianId::Lung`
        // 的 `Debug`/字面量拼写 "Lung" 不是合法 channel id —— 消费边界必须把它当未知
        // channel 安全拒绝，而不是误认成合法经脉或 panic。
        let mut app = App::new();
        add_forging_system(&mut app, true);
        let player = spawn_open_lung_player(&mut app, 100.0);

        app.world_mut().send_event(ForgeRequest {
            entity: player,
            meridian: MeridianChannelId::new("Lung"),
            axis: ForgeAxis::Rate,
        });
        app.update();

        let outcomes = collect_forge_outcomes(&app);
        assert_eq!(outcomes.len(), 1);
        assert_eq!(
            outcomes[0].result,
            Err(ForgeError::UnknownChannel),
            "旧 PascalCase 字面量 'Lung' 不是合法 wire channel id，必须被拒绝而非误认作 \
             regular Lung meridian"
        );
    }

    #[test]
    fn meridian_system_contains_true_for_known_false_for_unknown() {
        let ms = MeridianSystem::default();
        assert!(ms.contains(MeridianId::Lung.channel_id()));
        assert!(!ms.contains(MeridianChannelId::new("Lung"))); // legacy PascalCase 串
        assert!(!ms.contains(MeridianChannelId::new("nonexistent_channel")));
    }
}
