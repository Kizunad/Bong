//! 噬灵藓（plan-neg-domain-fauna-v1 P1）——负灵域 tainted 地块危害。
//!
//! 噬灵藓不是药材：无 drop、无 harvest、无合成路径。
//! 它通过 `SpreadByCrawl` spawn 模式在负灵域地面蔓延（由 `moss_spread_system` 驱动）。
//!
//! 四个系统：
//! - [`moss_step_on_system`]：玩家踩入 ShiLingXian `Plant` 覆盖范围 → 挂 `ShiLingXianDrainTag`。
//! - [`moss_step_off_system`]：玩家离开 ShiLingXian 覆盖范围 → 移除 `ShiLingXianDrainTag`。
//! - [`moss_drain_system`]：每 tick 对有 `ShiLingXianDrainTag` 的玩家 emit `QiTransfer`。
//! - [`moss_spread_system`]：按 `|spirit_qi|` 扩散 ShiLingXian Plant 实体；
//!   `spirit_qi >= 0` 时停止蔓延并启动 60s（1200 tick）枯萎倒计时（通过 `Plant.wither_progress`）。
//!
//! 真元守恒红线：所有 drain 通过 `QiTransfer{reason: ShiLingXianDrain}` 守恒流向 zone 账户，
//! **严禁** `cultivation.qi_current -= X` 无对应 zone 增。

use std::collections::HashSet;

use valence::prelude::{
    bevy_ecs, Commands, Component, DVec3, Entity, Events, Position, Query, Res, ResMut, Without,
};

use super::components::Plant;
use super::registry::BotanyPlantId;
use crate::cultivation::components::Cultivation;
use crate::cultivation::life_record::LifeRecord;
use crate::npc::spawn::common::NpcMarker;
use crate::qi_physics::constants::{MOSS_DRAIN_FACTOR, QI_EPSILON};
use crate::qi_physics::ledger::{QiAccountId, QiTransfer, QiTransferReason};
use crate::world::dimension::CurrentDimension;
use crate::world::zone::ZoneRegistry;

// ── 模块常量（蔓延/几何参数，非真元物理率；留本模块不归 qi_physics） ──

/// 踩踏检测半径（水平方向，格）。玩家与 ShiLingXian Plant 的水平距离小于此值视为踩踏。
pub const MOSS_STEP_RADIUS: f64 = 1.5;
/// 踩踏检测纵向范围（格）。允许玩家在 Plant 上方 0~2 格内触发。
pub const MOSS_STEP_VERTICAL_RANGE: f64 = 2.0;
/// 蔓延最大半径（格）。超出此半径的已有 ShiLingXian 实体不参与再蔓延（停止无限扩张）。
pub const MOSS_MAX_SPREAD_RADIUS: f64 = 16.0;
/// 蔓延基础速率（每 tick 扩展概率分母，|spirit_qi| 为分子）。
/// spread_probability = |spirit_qi| × MOSS_SPREAD_BASE_RATE（每 tick 对每个活跃 moss 判断）。
/// 值为 0.005 → spirit_qi=-1.0 时每 tick 0.5% 的概率每格扩散。
pub const MOSS_SPREAD_BASE_RATE: f64 = 0.005;
/// 枯萎倒计时长度（ticks）。spirit_qi >= 0 后经过此 tick 数 moss 自然枯萎（max_age_ticks=1200）。
pub const MOSS_WITHER_TICKS: u32 = 1_200;
/// 多 tick wither_progress 增量（每 tick +1；达到 MOSS_WITHER_TICKS 后标记 harvested=true 将被 lifecycle 清理）。
pub const MOSS_WITHER_INCREMENT: u32 = 1;

// ── 数据类型 ──

/// 玩家当前踩踏噬灵藓的持续扣量标记。
/// 挂在玩家实体上，离开 ShiLingXian 覆盖范围后移除。
/// drain_per_tick = qi_max × MOSS_DRAIN_FACTOR（挂 tag 时计算一次，qi_max 变化不实时更新）。
#[derive(Debug, Clone, Component)]
pub struct ShiLingXianDrainTag {
    /// 每 tick 从玩家真元扣除量（挂 tag 时由 qi_max × MOSS_DRAIN_FACTOR 计算）。
    pub drain_per_tick: f64,
    /// 所在 zone 名称（守恒目标 zone 账户）。
    pub zone_name: String,
}

// ── 系统 ──

/// moss_step_on_system：检测玩家是否踩踏 ShiLingXian，若是则挂 `ShiLingXianDrainTag`。
///
/// 查询所有 id = ShiLingXian 的 Plant 实体，检测玩家位置与 Plant.position 的水平/纵向距离。
/// 若已有 tag 且仍在覆盖范围内则跳过（幂等）。
/// 仅对玩家生效（`Without<NpcMarker>`），NPC 不进负灵域。
#[allow(clippy::type_complexity)]
pub fn moss_step_on_system(
    moss_plants: Query<&Plant>,
    mut players: Query<
        (
            Entity,
            &Position,
            Option<&CurrentDimension>,
            Option<&LifeRecord>,
            &Cultivation,
            Option<&ShiLingXianDrainTag>,
        ),
        Without<NpcMarker>,
    >,
    zones: Option<Res<ZoneRegistry>>,
    mut commands: Commands,
) {
    // 收集所有活跃的 ShiLingXian Plant 位置（id = ShiLingXian 且未枯萎）
    let moss_positions: Vec<([f64; 3], String)> = moss_plants
        .iter()
        .filter(|plant| {
            plant.id == BotanyPlantId::ShiLingXian && !plant.harvested && !plant.trampled
        })
        .map(|plant| (plant.position, plant.zone_name.clone()))
        .collect();

    if moss_positions.is_empty() {
        return;
    }

    let Some(zones) = zones else {
        return;
    };

    for (entity, pos, current_dim, life_record, cultivation, existing_tag) in players.iter_mut() {
        // 已有 tag 且仍在覆盖范围：检查是否还踩着
        let player_pos = pos.0;

        let on_moss = moss_positions.iter().any(|(moss_pos, _zone)| {
            let dx = (player_pos.x - moss_pos[0]).abs();
            let dz = (player_pos.z - moss_pos[2]).abs();
            let dy = player_pos.y - moss_pos[1]; // 正数：玩家在 moss 上方
            dx < MOSS_STEP_RADIUS
                && dz < MOSS_STEP_RADIUS
                && (0.0..MOSS_STEP_VERTICAL_RANGE).contains(&dy)
        });

        if !on_moss {
            continue;
        }

        // 已有 tag，跳过（幂等）
        if existing_tag.is_some() {
            continue;
        }

        // 找到玩家所在 zone
        let zone_name = current_dim
            .and_then(|dim| zones.find_zone(dim.0, player_pos))
            .map(|z| z.name.clone());
        let Some(zone_name) = zone_name else {
            continue;
        };

        // 计算 drain_per_tick = qi_max × MOSS_DRAIN_FACTOR
        let drain_per_tick = cultivation.qi_max * MOSS_DRAIN_FACTOR;

        let _ = life_record; // life_record 在 drain 时使用
        commands.entity(entity).insert(ShiLingXianDrainTag {
            drain_per_tick,
            zone_name,
        });
    }
}

/// moss_step_off_system：玩家离开 ShiLingXian 覆盖范围 → 移除 `ShiLingXianDrainTag`。
pub fn moss_step_off_system(
    moss_plants: Query<&Plant>,
    players: Query<(Entity, &Position, &ShiLingXianDrainTag), Without<NpcMarker>>,
    mut commands: Commands,
) {
    // 收集所有活跃 ShiLingXian 位置
    let moss_positions: Vec<[f64; 3]> = moss_plants
        .iter()
        .filter(|plant| {
            plant.id == BotanyPlantId::ShiLingXian && !plant.harvested && !plant.trampled
        })
        .map(|plant| plant.position)
        .collect();

    for (entity, pos, _tag) in players.iter() {
        let player_pos = pos.0;

        let still_on_moss = moss_positions.iter().any(|moss_pos| {
            let dx = (player_pos.x - moss_pos[0]).abs();
            let dz = (player_pos.z - moss_pos[2]).abs();
            let dy = player_pos.y - moss_pos[1];
            dx < MOSS_STEP_RADIUS
                && dz < MOSS_STEP_RADIUS
                && (0.0..MOSS_STEP_VERTICAL_RANGE).contains(&dy)
        });

        if !still_on_moss {
            commands.entity(entity).remove::<ShiLingXianDrainTag>();
        }
    }
}

/// moss_drain_system：每 tick 对有 `ShiLingXianDrainTag` 的玩家 emit `QiTransfer`。
///
/// drain_per_tick = qi_max × MOSS_DRAIN_FACTOR（已在 tag 内预计算）。
/// 通过 `QiTransfer{reason: ShiLingXianDrain}` 守恒流向 zone 账户。
#[allow(clippy::type_complexity)]
pub fn moss_drain_system(
    mut players: Query<
        (
            Entity,
            Option<&LifeRecord>,
            &mut Cultivation,
            &ShiLingXianDrainTag,
        ),
        Without<NpcMarker>,
    >,
    mut qi_transfer_events: Option<ResMut<Events<QiTransfer>>>,
) {
    for (entity, life_record, mut cultivation, tag) in players.iter_mut() {
        let drain = tag.drain_per_tick;
        if drain <= QI_EPSILON {
            continue;
        }

        // 实际扣除量（不超过 qi_current）
        let actual_drain = drain.min(cultivation.qi_current.max(0.0));
        if actual_drain <= QI_EPSILON {
            continue;
        }

        // 扣真元（守恒：player 减 → zone 增）
        cultivation.qi_current = (cultivation.qi_current - actual_drain).max(0.0);

        // emit QiTransfer 事件（守恒流向 zone 账户）
        let player_account_id = life_record
            .map(|lr| QiAccountId::player(&lr.character_id))
            .unwrap_or_else(|| QiAccountId::player(format!("{entity:?}")));
        let zone_account_id = QiAccountId::zone(&tag.zone_name);

        if let Ok(transfer) = QiTransfer::new(
            player_account_id,
            zone_account_id,
            actual_drain,
            QiTransferReason::ShiLingXianDrain,
        ) {
            if let Some(events) = qi_transfer_events.as_deref_mut() {
                events.send(transfer);
            }
        }
    }
}

/// moss_spread_system：按 `|spirit_qi|` 扩散 ShiLingXian Plant 实体。
///
/// - spirit_qi < 0 且当前蔓延半径 < MOSS_MAX_SPREAD_RADIUS：按 `|spirit_qi| × MOSS_SPREAD_BASE_RATE` 概率
///   向相邻地面位置扩散（生成新 Plant entity）。
/// - spirit_qi >= 0：所有 ShiLingXian Plant 开始 wither（wither_progress 递增），
///   达到 MOSS_WITHER_TICKS 后标记 `harvested=true`（由 botany lifecycle 清理）。
pub fn moss_spread_system(
    zones: Option<Res<ZoneRegistry>>,
    mut moss_plants: Query<&mut Plant>,
    mut commands: Commands,
    tick: Option<Res<crate::cultivation::tick::CultivationClock>>,
) {
    let Some(zones) = zones else {
        return;
    };
    let current_tick = tick.as_deref().map(|t| t.tick).unwrap_or(0);

    // 收集当前所有 ShiLingXian 植物快照（为蔓延计算）
    let existing_moss: Vec<([f64; 3], String)> = moss_plants
        .iter()
        .filter(|p| p.id == BotanyPlantId::ShiLingXian && !p.harvested)
        .map(|p| (p.position, p.zone_name.clone()))
        .collect();

    // 记录本轮已占用位置（防止同 tick 重复 spawn）
    let occupied: HashSet<(i64, i64, i64)> = existing_moss
        .iter()
        .map(|(pos, _)| {
            (
                pos[0].floor() as i64,
                pos[1].floor() as i64,
                pos[2].floor() as i64,
            )
        })
        .collect();

    // 本轮要新增的 Plant 数据：(position, zone_name)
    let mut new_plants: Vec<([f64; 3], String)> = Vec::new();

    for (moss_pos, zone_name) in &existing_moss {
        let Some(zone) = zones.find_zone_by_name(zone_name) else {
            continue;
        };
        let zone_qi = zone.spirit_qi;

        // spirit_qi >= 0：进入枯萎模式，由 wither 分支处理（见下方 wither loop）
        if zone_qi >= 0.0 {
            continue;
        }

        // 判断是否超出最大蔓延半径（以 zone 中心为基准）
        let zone_center = zone.center();
        let dist = DVec3::new(moss_pos[0], moss_pos[1], moss_pos[2]).distance(DVec3::new(
            zone_center.x,
            moss_pos[1],
            zone_center.z,
        ));
        if dist >= MOSS_MAX_SPREAD_RADIUS {
            continue;
        }

        // spread_probability = |spirit_qi| × MOSS_SPREAD_BASE_RATE
        let spread_prob = (-zone_qi) * MOSS_SPREAD_BASE_RATE;

        // 简单确定性散布：用 tick + moss 坐标哈希作为伪随机种子
        let seed = current_tick
            .wrapping_add((moss_pos[0] * 1000.0) as u64)
            .wrapping_add((moss_pos[2] * 7777.0) as u64)
            .wrapping_mul(0x9E3779B97F4A7C15);

        // 每个 moss entity 每 tick 以 spread_prob 概率尝试扩散到一个相邻位置
        let roll = (seed as f64) / (u64::MAX as f64);
        if roll >= spread_prob {
            continue;
        }

        // 选择一个相邻方向（四个水平方向）
        let dir_idx = (seed >> 32) % 4;
        let (dx, dz): (f64, f64) = match dir_idx {
            0 => (1.0, 0.0),
            1 => (-1.0, 0.0),
            2 => (0.0, 1.0),
            _ => (0.0, -1.0),
        };
        let new_x = moss_pos[0] + dx;
        let new_z = moss_pos[2] + dz;
        let new_y = moss_pos[1];

        // 检查 zone bounds（不出 zone）
        let new_pos = DVec3::new(new_x, new_y, new_z);
        if !zone.contains(new_pos) {
            continue;
        }

        // 检查是否已占用
        let cell = (
            new_x.floor() as i64,
            new_y.floor() as i64,
            new_z.floor() as i64,
        );
        if occupied.contains(&cell) {
            continue;
        }

        new_plants.push(([new_x, new_y, new_z], zone_name.clone()));
    }

    // Spawn 新的 ShiLingXian Plant 实体
    for (pos, zone_name) in new_plants {
        commands.spawn(Plant {
            id: BotanyPlantId::ShiLingXian,
            zone_name,
            position: pos,
            planted_at_tick: current_tick,
            wither_progress: 0,
            source_point: None,
            harvested: false,
            trampled: false,
            variant: super::registry::PlantVariant::Tainted,
        });
    }

    // Wither loop：spirit_qi >= 0 时递增 wither_progress，达到上限后标记枯萎
    for mut plant in moss_plants.iter_mut() {
        if plant.id != BotanyPlantId::ShiLingXian || plant.harvested {
            continue;
        }

        let zone_qi = zones
            .find_zone_by_name(&plant.zone_name)
            .map(|z| z.spirit_qi)
            .unwrap_or(0.0);

        if zone_qi < 0.0 {
            // 负灵域中：重置枯萎进度（spirit_qi 回负时恢复）
            if plant.wither_progress > 0 {
                plant.wither_progress = 0;
            }
            continue;
        }

        // spirit_qi >= 0：递增枯萎计数
        plant.wither_progress = plant.wither_progress.saturating_add(MOSS_WITHER_INCREMENT);
        if plant.wither_progress >= MOSS_WITHER_TICKS {
            plant.harvested = true; // 标记已消亡，lifecycle 清理
        }
    }
}

// ── 单测 ──

#[cfg(test)]
mod tests {
    use super::*;
    use crate::botany::components::Plant;
    use crate::botany::registry::{BotanyPlantId, PlantVariant};
    use crate::cultivation::components::Cultivation;
    use crate::player::state::canonical_player_id;
    use crate::world::dimension::{CurrentDimension, DimensionKind};
    use valence::prelude::{App, Events, IntoSystemConfigs, Update};

    // ── helpers ──

    fn make_moss_plant(pos: [f64; 3], zone_name: &str) -> Plant {
        Plant {
            id: BotanyPlantId::ShiLingXian,
            zone_name: zone_name.to_string(),
            position: pos,
            planted_at_tick: 0,
            wither_progress: 0,
            source_point: None,
            harvested: false,
            trampled: false,
            variant: PlantVariant::Tainted,
        }
    }

    fn make_player_cultivation(qi_current: f64, qi_max: f64) -> Cultivation {
        Cultivation {
            qi_current,
            qi_max,
            ..Default::default()
        }
    }

    fn make_app_with_neg_zone(spirit_qi: f64) -> App {
        let mut app = App::new();
        app.add_event::<QiTransfer>();
        let mut zones = crate::world::zone::ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = spirit_qi;
        app.insert_resource(zones);
        app
    }

    // ── T1: 踩踏挂 tag（happy path）──

    #[test]
    fn moss_step_on_attaches_drain_tag_when_player_over_moss() {
        let mut app = make_app_with_neg_zone(-0.5);
        app.add_systems(Update, (moss_step_on_system, moss_drain_system).chain());

        // Spawn 一朵 ShiLingXian at (8, 64, 8)
        app.world_mut()
            .spawn(make_moss_plant([8.0, 64.0, 8.0], "spawn"));

        // Spawn 玩家站在 moss 上方（y=65，水平居中）
        let player = app
            .world_mut()
            .spawn((
                Position::new([8.0, 65.0, 8.0]),
                CurrentDimension(DimensionKind::Overworld),
                make_player_cultivation(100.0, 100.0),
                LifeRecord::new(canonical_player_id("Azure")),
            ))
            .id();

        app.update();

        assert!(
            app.world()
                .entity(player)
                .get::<ShiLingXianDrainTag>()
                .is_some(),
            "踩踏 ShiLingXian 后玩家应挂 ShiLingXianDrainTag（期望：tag 存在）"
        );
    }

    // ── T2: 离开移除 tag ──

    #[test]
    fn moss_step_off_removes_drain_tag_when_player_leaves() {
        let mut app = make_app_with_neg_zone(-0.5);
        app.add_systems(Update, moss_step_off_system);

        // 没有任何 ShiLingXian Plant（玩家已离开区域）
        let player = app
            .world_mut()
            .spawn((
                Position::new([100.0, 65.0, 100.0]),
                ShiLingXianDrainTag {
                    drain_per_tick: 0.2,
                    zone_name: "spawn".to_string(),
                },
            ))
            .id();

        app.update();

        assert!(
            app.world()
                .entity(player)
                .get::<ShiLingXianDrainTag>()
                .is_none(),
            "离开 ShiLingXian 范围后应移除 ShiLingXianDrainTag（期望：tag 已移除）"
        );
    }

    // ── T3: drain 守恒（qi_current 减少 + QiTransfer 事件）──

    #[test]
    fn moss_drain_reduces_qi_and_emits_qi_transfer_event() {
        let mut app = make_app_with_neg_zone(-0.5);
        app.add_systems(Update, moss_drain_system);

        let drain_per_tick = 0.2_f64;
        let initial_qi = 100.0_f64;

        let player = app
            .world_mut()
            .spawn((
                make_player_cultivation(initial_qi, 100.0),
                LifeRecord::new(canonical_player_id("Azure")),
                ShiLingXianDrainTag {
                    drain_per_tick,
                    zone_name: "spawn".to_string(),
                },
            ))
            .id();

        app.update();

        let cultivation = app.world().entity(player).get::<Cultivation>().unwrap();
        let expected_qi = initial_qi - drain_per_tick;
        assert!(
            (cultivation.qi_current - expected_qi).abs() < 1e-9,
            "期望 qi_current == {expected_qi}（初始 {initial_qi} - drain {drain_per_tick}），\
             实际 {}（moss drain 应守恒减少真元）",
            cultivation.qi_current
        );

        let transfers: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<QiTransfer>>()
            .drain()
            .collect();
        assert_eq!(
            transfers.len(),
            1,
            "每 tick 应 emit 1 条 QiTransfer（期望 1，实际 {}）",
            transfers.len()
        );
        assert_eq!(
            transfers[0].reason,
            QiTransferReason::ShiLingXianDrain,
            "QiTransfer reason 应为 ShiLingXianDrain（期望），实际 {:?}",
            transfers[0].reason
        );
        assert!(
            (transfers[0].amount - drain_per_tick).abs() < 1e-9,
            "QiTransfer amount 应为 {drain_per_tick}，实际 {}（守恒：player 减 == zone 增）",
            transfers[0].amount
        );
    }

    // ── T4: drain 不超过 qi_current（边界：qi_current < drain_per_tick）──

    #[test]
    fn moss_drain_clamps_to_remaining_qi_current() {
        let mut app = make_app_with_neg_zone(-0.5);
        app.add_systems(Update, moss_drain_system);

        let drain_per_tick = 5.0_f64;
        let initial_qi = 2.0_f64; // 少于 drain_per_tick

        let player = app
            .world_mut()
            .spawn((
                make_player_cultivation(initial_qi, 100.0),
                LifeRecord::new(canonical_player_id("Bao")),
                ShiLingXianDrainTag {
                    drain_per_tick,
                    zone_name: "spawn".to_string(),
                },
            ))
            .id();

        app.update();

        let cultivation = app.world().entity(player).get::<Cultivation>().unwrap();
        assert!(
            cultivation.qi_current >= 0.0,
            "qi_current 不应低于 0（期望 >= 0，实际 {}；drain 应 clamp 到剩余量）",
            cultivation.qi_current
        );
        assert!(
            (cultivation.qi_current).abs() < 1e-9,
            "qi_current 不足 drain 时应归零（期望 0.0，实际 {}）",
            cultivation.qi_current
        );

        let transfers: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<QiTransfer>>()
            .drain()
            .collect();
        assert_eq!(
            transfers.len(),
            1,
            "qi_current 不足时仍应 emit QiTransfer（实际扣除量），实际 {} 条",
            transfers.len()
        );
        assert!(
            (transfers[0].amount - initial_qi).abs() < 1e-9,
            "守恒：transfer amount 应为 {initial_qi}（实际扣量），实际 {}",
            transfers[0].amount
        );
    }

    // ── T5: qi_current == 0 时不 emit（边界：零真元）──

    #[test]
    fn moss_drain_skips_when_qi_current_is_zero() {
        let mut app = make_app_with_neg_zone(-0.5);
        app.add_systems(Update, moss_drain_system);

        app.world_mut().spawn((
            make_player_cultivation(0.0, 100.0),
            LifeRecord::new(canonical_player_id("Empty")),
            ShiLingXianDrainTag {
                drain_per_tick: 0.2,
                zone_name: "spawn".to_string(),
            },
        ));

        app.update();

        let transfers: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<QiTransfer>>()
            .drain()
            .collect();
        assert!(
            transfers.is_empty(),
            "qi_current == 0 时不应 emit QiTransfer（期望空，实际 {} 条）",
            transfers.len()
        );
    }

    // ── T6: 多 tick 累计扣量 ──

    #[test]
    fn moss_drain_accumulates_over_multiple_ticks() {
        let mut app = make_app_with_neg_zone(-0.5);
        app.add_systems(Update, moss_drain_system);

        let drain_per_tick = 0.2_f64;
        let initial_qi = 100.0_f64;
        let ticks = 5_usize;

        let player = app
            .world_mut()
            .spawn((
                make_player_cultivation(initial_qi, 100.0),
                LifeRecord::new(canonical_player_id("Multi")),
                ShiLingXianDrainTag {
                    drain_per_tick,
                    zone_name: "spawn".to_string(),
                },
            ))
            .id();

        for _ in 0..ticks {
            app.update();
        }

        let cultivation = app.world().entity(player).get::<Cultivation>().unwrap();
        let expected_qi = initial_qi - drain_per_tick * ticks as f64;
        assert!(
            (cultivation.qi_current - expected_qi).abs() < 1e-6,
            "期望 {ticks} tick 后 qi_current == {expected_qi}（每 tick drain {drain_per_tick}），\
             实际 {}",
            cultivation.qi_current
        );
    }

    // ── T7: moss_spread_system — spirit_qi >= 0 时 wither 递增 ──

    #[test]
    fn moss_wither_increments_when_zone_spirit_qi_nonneg() {
        let mut app = make_app_with_neg_zone(0.5); // spirit_qi 正值
        app.add_systems(Update, moss_spread_system);

        let moss = app
            .world_mut()
            .spawn(make_moss_plant([8.0, 64.0, 8.0], "spawn"))
            .id();

        app.update();

        let plant = app.world().entity(moss).get::<Plant>().unwrap();
        assert!(
            plant.wither_progress > 0,
            "spirit_qi >= 0 时 wither_progress 应 > 0（期望递增，实际 {}）",
            plant.wither_progress
        );
    }

    // ── T8: moss_spread_system — wither 达到上限后标记 harvested ──

    #[test]
    fn moss_marked_harvested_when_wither_ticks_exceeded() {
        let mut app = make_app_with_neg_zone(0.5);
        app.add_systems(Update, moss_spread_system);

        // 预设 wither_progress 接近上限
        let mut plant = make_moss_plant([8.0, 64.0, 8.0], "spawn");
        plant.wither_progress = MOSS_WITHER_TICKS - 1;
        let moss = app.world_mut().spawn(plant).id();

        app.update();

        let p = app.world().entity(moss).get::<Plant>().unwrap();
        assert!(
            p.harvested,
            "wither_progress 达到 MOSS_WITHER_TICKS 后应标记 harvested=true（期望 true，实际 {}）",
            p.harvested
        );
    }

    // ── T9: 玩家距离超出范围时不挂 tag ──

    #[test]
    fn moss_step_on_does_not_attach_tag_when_player_is_far() {
        let mut app = make_app_with_neg_zone(-0.5);
        app.add_systems(Update, moss_step_on_system);

        app.world_mut()
            .spawn(make_moss_plant([8.0, 64.0, 8.0], "spawn"));

        // 玩家在很远处
        let player = app
            .world_mut()
            .spawn((
                Position::new([100.0, 65.0, 100.0]),
                CurrentDimension(DimensionKind::Overworld),
                make_player_cultivation(100.0, 100.0),
                LifeRecord::new(canonical_player_id("Far")),
            ))
            .id();

        app.update();

        assert!(
            app.world()
                .entity(player)
                .get::<ShiLingXianDrainTag>()
                .is_none(),
            "玩家远离 ShiLingXian 时不应挂 tag（期望：无 tag）"
        );
    }

    // ── T10: 踩踏幂等（已有 tag 时不重复挂）──

    #[test]
    fn moss_step_on_is_idempotent_when_tag_already_present() {
        let mut app = make_app_with_neg_zone(-0.5);
        app.add_systems(Update, moss_step_on_system);

        app.world_mut()
            .spawn(make_moss_plant([8.0, 64.0, 8.0], "spawn"));

        // 玩家已有 tag
        let player = app
            .world_mut()
            .spawn((
                Position::new([8.0, 65.0, 8.0]),
                CurrentDimension(DimensionKind::Overworld),
                make_player_cultivation(100.0, 100.0),
                LifeRecord::new(canonical_player_id("Repeat")),
                ShiLingXianDrainTag {
                    drain_per_tick: 0.2,
                    zone_name: "spawn".to_string(),
                },
            ))
            .id();

        app.update();

        // 仍然只有一个 tag（系统跳过了重复插入）
        assert!(
            app.world()
                .entity(player)
                .get::<ShiLingXianDrainTag>()
                .is_some(),
            "幂等：tag 应保持存在（期望：tag 存在）"
        );
        // 验证 drain_per_tick 未被覆盖（幂等不更新）
        let tag = app
            .world()
            .entity(player)
            .get::<ShiLingXianDrainTag>()
            .unwrap();
        assert!(
            (tag.drain_per_tick - 0.2).abs() < 1e-9,
            "幂等：已有 tag 的 drain_per_tick 不应被覆盖（期望 0.2，实际 {}）",
            tag.drain_per_tick
        );
    }

    // ── T11: 已枯萎（harvested=true）的 moss 不触发踩踏 ──

    #[test]
    fn moss_step_on_ignores_harvested_plants() {
        let mut app = make_app_with_neg_zone(-0.5);
        app.add_systems(Update, moss_step_on_system);

        // 已枯萎的 moss
        let mut dead_moss = make_moss_plant([8.0, 64.0, 8.0], "spawn");
        dead_moss.harvested = true;
        app.world_mut().spawn(dead_moss);

        let player = app
            .world_mut()
            .spawn((
                Position::new([8.0, 65.0, 8.0]),
                CurrentDimension(DimensionKind::Overworld),
                make_player_cultivation(100.0, 100.0),
                LifeRecord::new(canonical_player_id("Dead")),
            ))
            .id();

        app.update();

        assert!(
            app.world()
                .entity(player)
                .get::<ShiLingXianDrainTag>()
                .is_none(),
            "已枯萎的 ShiLingXian 不应触发踩踏（期望：无 tag）"
        );
    }

    // ── T12: drain_per_tick 公式验证 ──

    #[test]
    fn drain_per_tick_formula_matches_moss_drain_factor() {
        // drain_per_tick = qi_max × MOSS_DRAIN_FACTOR
        // qi_max=100 → drain=0.2；qi_max=200 → drain=0.4（高手更危险）
        let low = 100.0_f64 * MOSS_DRAIN_FACTOR;
        let high = 200.0_f64 * MOSS_DRAIN_FACTOR;
        assert!(
            (low - 0.2).abs() < 1e-9,
            "qi_max=100 时 drain_per_tick 应为 0.2（100×0.002），实际 {low}"
        );
        assert!(
            (high - 2.0 * low).abs() < 1e-9,
            "qi_max 翻倍，drain 应翻倍（期望 {}, 实际 {high}）",
            2.0 * low
        );
    }

    // ── P2 生态联动测试 ──

    /// P2-T1: moss spread_probability 正比于 |spirit_qi|
    /// spirit_qi=-0.5 的蔓延概率是 spirit_qi=-1.0 的一半（生态联动）。
    #[test]
    fn p2_moss_spread_probability_proportional_to_spirit_qi_magnitude() {
        // spread_prob = |spirit_qi| × MOSS_SPREAD_BASE_RATE
        // spirit_qi=-0.5 → prob=0.5×0.005=0.0025
        // spirit_qi=-1.0 → prob=1.0×0.005=0.005（2× faster）
        let prob_low = 0.5_f64 * MOSS_SPREAD_BASE_RATE;
        let prob_high = 1.0_f64 * MOSS_SPREAD_BASE_RATE;

        assert!(
            (prob_low - 0.0025).abs() < 1e-9,
            "spirit_qi=-0.5 时 spread_prob 应为 0.0025（0.5×0.005），实际 {prob_low}（P2 生态联动）"
        );
        assert!(
            (prob_high - 0.005).abs() < 1e-9,
            "spirit_qi=-1.0 时 spread_prob 应为 0.005（1.0×0.005），实际 {prob_high}（P2 生态联动）"
        );
        assert!(
            (prob_high - 2.0 * prob_low).abs() < 1e-9,
            "spirit_qi=-1.0 蔓延速率应是 spirit_qi=-0.5 的 2 倍（期望 {}，实际 {}）（P2 生态联动）",
            2.0 * prob_low,
            prob_high
        );
    }

    /// P2-T2: spirit_qi 趋近 0（= -0.0）时 moss 停止蔓延
    /// spirit_qi=-0.0 → spread_prob 极小（趋近 0），实际系统中 spirit_qi>=0 停蔓延。
    #[test]
    fn p2_moss_spread_stops_when_spirit_qi_approaches_zero() {
        // 当 spirit_qi 非常接近 0（负）时，spread_probability 趋于 0
        // spirit_qi = -0.001 → spread_prob = 0.001 × 0.005 = 0.000005（极低）
        // spirit_qi = 0.0 （= wither 条件）→ spread_prob 公式不再触发（走 wither 分支）
        let tiny_pressure = 0.001_f64;
        let prob_tiny = tiny_pressure * MOSS_SPREAD_BASE_RATE;
        assert!(
            prob_tiny < 0.0001,
            "spirit_qi 趋于 0 时 spread_prob 应极小（期望 < 0.0001，实际 {}）（P2 生态联动）",
            prob_tiny
        );

        // spirit_qi=0 时 moss_spread_system 进入 wither 分支（不再蔓延），
        // 验证：zone spirit_qi=0 的 zone 中，已有 moss 不再新增，而是开始 wither。
        let mut app = make_app_with_neg_zone(0.0);
        app.add_systems(Update, moss_spread_system);

        let moss = app
            .world_mut()
            .spawn(make_moss_plant([8.0, 64.0, 8.0], "spawn"))
            .id();
        app.update();

        // spirit_qi=0 → 走 wither 分支，wither_progress 应增加
        let plant = app.world().entity(moss).get::<Plant>().unwrap();
        assert!(
            plant.wither_progress > 0,
            "spirit_qi=0 时 moss 应进入 wither 模式（期望 wither_progress > 0，实际 {}）（P2 生态联动）",
            plant.wither_progress
        );
    }

    /// P2-T3: moss wither 60s（1200 tick）倒计时——MOSS_WITHER_TICKS 常量 pin 测试
    #[test]
    fn p2_moss_wither_ticks_constant_is_60s() {
        // MC 20TPS × 60s = 1200 ticks（60s 枯萎倒计时是 plan 设计决议）
        assert_eq!(
            MOSS_WITHER_TICKS, 1200,
            "MOSS_WITHER_TICKS 应为 1200（60s × 20TPS），实际 {MOSS_WITHER_TICKS}（P2 生态联动 60s 倒计时 pin）"
        );
    }

    /// P2-T4: moss spirit_qi 由负变 0 后 wither 倒计时精确 1200 tick 后完成
    #[test]
    fn p2_moss_wither_completes_after_exactly_wither_ticks() {
        let mut app = make_app_with_neg_zone(0.5); // spirit_qi >= 0 → wither 模式
        app.add_systems(Update, moss_spread_system);

        // 预设 wither_progress = MOSS_WITHER_TICKS - 1（差 1 tick 完成）
        let mut plant = make_moss_plant([8.0, 64.0, 8.0], "spawn");
        plant.wither_progress = MOSS_WITHER_TICKS - 1;
        let moss = app.world_mut().spawn(plant).id();

        app.update(); // 最后 1 tick，应触发 harvested=true

        let p = app.world().entity(moss).get::<Plant>().unwrap();
        assert!(
            p.harvested,
            "wither_progress 达到 MOSS_WITHER_TICKS（{MOSS_WITHER_TICKS}）时应标记 harvested=true（期望 true，实际 {}）（P2 60s 倒计时精确）",
            p.harvested
        );
    }

    /// P2-T5: moss 在负灵域（spirit_qi < 0）时 wither_progress 被重置为 0（恢复）
    #[test]
    fn p2_moss_wither_progress_resets_when_zone_returns_negative() {
        // 当 zone spirit_qi 回到负值，已开始 wither 的 moss 重置进度（负灵域恢复）
        let mut app = make_app_with_neg_zone(-0.5); // spirit_qi < 0 → 恢复
        app.add_systems(Update, moss_spread_system);

        // 预设有部分 wither_progress
        let mut plant = make_moss_plant([8.0, 64.0, 8.0], "spawn");
        plant.wither_progress = 100; // 已经枯萎了一些
        let moss = app.world_mut().spawn(plant).id();

        app.update();

        let p = app.world().entity(moss).get::<Plant>().unwrap();
        assert_eq!(
            p.wither_progress, 0,
            "负灵域（spirit_qi=-0.5）中 moss wither_progress 应被重置为 0（期望 0，实际 {}）（P2 生态联动恢复）",
            p.wither_progress
        );
    }

    /// P2-T6: moss 超出 MOSS_MAX_SPREAD_RADIUS 时不蔓延——常量 pin + 蔓延条件代码路径验证
    #[test]
    fn p2_moss_spread_stops_beyond_max_radius() {
        // MOSS_MAX_SPREAD_RADIUS = 16.0（plan 设计决议 pin）
        assert_eq!(
            MOSS_MAX_SPREAD_RADIUS, 16.0,
            "MOSS_MAX_SPREAD_RADIUS 应为 16.0（plan 设计决议），实际 {MOSS_MAX_SPREAD_RADIUS}（P2 最大半径 pin）"
        );

        // 验证：放在 zone center 偏移 >= 16.0 的 moss 不触发蔓延。
        // 实现：20 tick 全程对单个超界 moss 运行 moss_spread_system，
        // plant 总数保持 1（无新增），因为 dist >= MOSS_MAX_SPREAD_RADIUS 分支跳过。
        let mut app = make_app_with_neg_zone(-1.0);
        app.add_systems(Update, moss_spread_system);

        let zones = {
            let r = app.world().resource::<crate::world::zone::ZoneRegistry>();
            r.clone()
        };
        let zone = zones.find_zone_by_name("spawn").expect("spawn zone exists");
        let center = zone.center();

        // 放在超出最大蔓延半径的位置（center + MOSS_MAX_SPREAD_RADIUS + 1.0）
        let far_x = center.x + MOSS_MAX_SPREAD_RADIUS + 1.0;
        app.world_mut()
            .spawn(make_moss_plant([far_x, center.y, center.z], "spawn"));

        for _ in 0..20 {
            app.update();
        }

        // 超界 moss 不蔓延——plant 总数仍为 1（只有起始那一个）
        let count = app.world_mut().query::<&Plant>().iter(app.world()).count();
        assert_eq!(
            count, 1,
            "超出 MOSS_MAX_SPREAD_RADIUS({MOSS_MAX_SPREAD_RADIUS}) 的 moss 不应蔓延（期望 1，实际 {count}）（P2 最大半径边界）"
        );
    }
}
