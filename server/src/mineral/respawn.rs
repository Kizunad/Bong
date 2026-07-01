//! plan-cultivation-pacing-v1 P1.9 — 矿脉再生接线。
//!
//! `ExhaustedMineralsLog::remove_respawned` 已经能算出哪些 exhausted 条目到期，
//! 但此前没有任何 Update 系统调用它 —— 矿脉写入 `respawn_at_tick` 后从此
//! 永远停留在耗尽状态，永不真正物化回 `MineralOreNode`。
//!
//! 本模块每 tick 查一次到期条目，按 `anchors::spawn_mineral_anchor_nodes` 的
//! 物化模式（`commands.spawn(MineralOreNode + Gatherable)` + `index.insert`）
//! 重新造回矿脉实体。

use valence::prelude::{BlockPos, Commands, Res, ResMut};

use super::anchors::mineral_gatherable;
use super::components::MineralOreIndex;
use super::components::MineralOreNode;
use super::persistence::{ExhaustedMineralsLog, MineralTickClock};
use super::registry::MineralRegistry;
use super::types::MineralId;
use crate::world::dimension::DimensionKind;

/// system — 按 `MineralTickClock` 当前 tick 查询到期的耗尽记录，重新物化矿脉。
///
/// 挂进 `Update`，与 `record_exhausted_minerals` 同级（都读写 `ExhaustedMineralsLog`）。
///
/// 当前所有矿脉锚点/化石矿脉均只在 `DimensionKind::Overworld` 物化
/// （`anchors::spawn_mineral_anchor_nodes` 硬编码 Overworld；`ExhaustedEntry` 本身
/// 也不携带 dimension 字段），因此重生同样固定 Overworld —— 与耗尽记账的隐含
/// 维度假设保持一致，不额外发明新语义。
pub fn respawn_exhausted_minerals(
    mut commands: Commands,
    mut exhausted: ResMut<ExhaustedMineralsLog>,
    clock: Res<MineralTickClock>,
    registry: Res<MineralRegistry>,
    mut index: ResMut<MineralOreIndex>,
) {
    let respawned = exhausted.remove_respawned(clock.tick);
    if respawned.is_empty() {
        return;
    }

    let mut spawned = 0usize;
    for entry in respawned {
        let Some(mineral_id) = MineralId::from_str(&entry.mineral_id) else {
            tracing::warn!(
                target: "bong::mineral",
                "skipping respawn for unknown mineral_id `{}` at ({}, {}, {})",
                entry.mineral_id,
                entry.x,
                entry.y,
                entry.z
            );
            continue;
        };
        let pos = BlockPos::new(entry.x, entry.y, entry.z);
        if index.lookup(DimensionKind::Overworld, pos).is_some() {
            // 防御性去重：该位置已经有活的 OreNode（理论上不该发生 —— exhausted
            // 记录期间该位置不会被重新物化），跳过避免同一 BlockPos 挂两个实体。
            continue;
        }

        let entity = commands
            .spawn((
                MineralOreNode::new(mineral_id, pos),
                mineral_gatherable(mineral_id, registry.as_ref()),
            ))
            .id();
        index.insert(DimensionKind::Overworld, pos, entity);
        spawned += 1;
    }

    if spawned > 0 {
        tracing::info!(
            target: "bong::mineral",
            "respawned {spawned} mineral ore node(s) from expired exhaustion entries"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::super::persistence::ExhaustedEntry;
    use super::super::registry::build_default_registry;
    use super::*;
    use crate::gathering::session::Gatherable;
    use crate::gathering::tools::{base_time_ticks, GatheringTargetKind};
    use valence::prelude::{App, Update};

    fn app_with(exhausted: ExhaustedMineralsLog, tick: u64) -> App {
        let mut app = App::new();
        app.insert_resource(exhausted);
        app.insert_resource(MineralTickClock { tick });
        app.insert_resource(build_default_registry());
        app.insert_resource(MineralOreIndex::default());
        app.add_systems(Update, respawn_exhausted_minerals);
        app
    }

    #[test]
    fn happy_path_respawns_expired_entry_and_updates_index() {
        let mut log = ExhaustedMineralsLog::default();
        log.record(ExhaustedEntry {
            mineral_id: "fan_tie".into(),
            x: 10,
            y: 64,
            z: -5,
            tick: 100,
            respawn_at_tick: Some(200),
        });

        let mut app = app_with(log, 200);
        app.update();

        let index = app.world().resource::<MineralOreIndex>();
        let pos = BlockPos::new(10, 64, -5);
        let entity = index.lookup(DimensionKind::Overworld, pos);
        assert!(
            entity.is_some(),
            "expected an OreNode entity to be indexed at the respawned position, got None"
        );

        let mut query = app.world_mut().query::<&MineralOreNode>();
        let nodes = query.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(
            nodes.len(),
            1,
            "expected exactly one respawned MineralOreNode entity, got {}",
            nodes.len()
        );
        assert_eq!(nodes[0].mineral_id, MineralId::FanTie);
        assert_eq!(nodes[0].position, pos);
        assert_eq!(
            nodes[0].remaining_units, 1,
            "respawned OreNode should start with 1 remaining unit like a freshly-materialized anchor node"
        );

        let mut gatherable_query = app.world_mut().query::<&Gatherable>();
        let gatherables = gatherable_query.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(
            gatherables.len(),
            1,
            "respawned OreNode must carry Gatherable metadata like anchor-spawned nodes"
        );
        assert_eq!(gatherables[0].target, GatheringTargetKind::Ore);
        assert_eq!(
            gatherables[0].base_time_ticks,
            base_time_ticks(GatheringTargetKind::Ore)
        );
        assert_eq!(gatherables[0].loot_table, "mineral:fan_tie");

        let exhausted = app.world().resource::<ExhaustedMineralsLog>();
        assert!(
            exhausted.entries().is_empty(),
            "expired entry must be removed from the exhausted log after respawn"
        );
    }

    #[test]
    fn not_yet_due_entry_is_not_respawned() {
        let mut log = ExhaustedMineralsLog::default();
        log.record(ExhaustedEntry {
            mineral_id: "fan_tie".into(),
            x: 0,
            y: 64,
            z: 0,
            tick: 100,
            respawn_at_tick: Some(500),
        });

        let mut app = app_with(log, 200);
        app.update();

        let index = app.world().resource::<MineralOreIndex>();
        assert_eq!(
            index.lookup(DimensionKind::Overworld, BlockPos::new(0, 64, 0)),
            None,
            "entry not yet due (tick 200 < respawn_at_tick 500) must not be respawned"
        );
        let mut query = app.world_mut().query::<&MineralOreNode>();
        assert_eq!(query.iter(app.world()).count(), 0);

        let exhausted = app.world().resource::<ExhaustedMineralsLog>();
        assert_eq!(
            exhausted.entries().len(),
            1,
            "not-yet-due entry must remain in the exhausted log"
        );
    }

    #[test]
    fn empty_log_is_a_no_op() {
        let mut app = app_with(ExhaustedMineralsLog::default(), 999_999);
        // Should not panic on an empty log.
        app.update();

        let index = app.world().resource::<MineralOreIndex>();
        assert!(index.is_empty(), "empty log must spawn nothing");
        let mut query = app.world_mut().query::<&MineralOreNode>();
        assert_eq!(query.iter(app.world()).count(), 0);
    }

    #[test]
    fn permanent_entry_never_respawns() {
        let mut log = ExhaustedMineralsLog::default();
        log.record(ExhaustedEntry {
            mineral_id: "ku_jin".into(),
            x: 1,
            y: 64,
            z: 1,
            tick: 100,
            respawn_at_tick: None,
        });

        let mut app = app_with(log, 999_999_999);
        app.update();

        let index = app.world().resource::<MineralOreIndex>();
        assert!(
            index.is_empty(),
            "永不再生矿物(respawn_at_tick=None)在任何 tick 都不应被重新物化"
        );
        let exhausted = app.world().resource::<ExhaustedMineralsLog>();
        assert_eq!(exhausted.entries().len(), 1, "permanent entry must remain");
    }

    #[test]
    fn multiple_due_entries_all_respawn_in_one_tick() {
        let mut log = ExhaustedMineralsLog::default();
        for i in 0..5 {
            log.record(ExhaustedEntry {
                mineral_id: "fan_tie".into(),
                x: i,
                y: 64,
                z: 0,
                tick: 100,
                respawn_at_tick: Some(300),
            });
        }
        // one not-yet-due entry mixed in
        log.record(ExhaustedEntry {
            mineral_id: "ling_tie".into(),
            x: 10,
            y: 64,
            z: 10,
            tick: 100,
            respawn_at_tick: Some(1000),
        });

        let mut app = app_with(log, 300);
        app.update();

        let mut query = app.world_mut().query::<&MineralOreNode>();
        assert_eq!(
            query.iter(app.world()).count(),
            5,
            "all 5 due entries should respawn in the same tick"
        );
        let index = app.world().resource::<MineralOreIndex>();
        for i in 0..5 {
            assert!(
                index
                    .lookup(DimensionKind::Overworld, BlockPos::new(i, 64, 0))
                    .is_some(),
                "expected respawned OreNode indexed at x={i}"
            );
        }
        let exhausted = app.world().resource::<ExhaustedMineralsLog>();
        assert_eq!(
            exhausted.entries().len(),
            1,
            "only the not-yet-due ling_tie entry should remain"
        );
        assert_eq!(exhausted.entries()[0].mineral_id, "ling_tie");
    }

    #[test]
    fn unknown_mineral_id_is_skipped_without_panicking() {
        let mut log = ExhaustedMineralsLog::default();
        log.record(ExhaustedEntry {
            mineral_id: "xuan_tie_does_not_exist".into(),
            x: 0,
            y: 64,
            z: 0,
            tick: 100,
            respawn_at_tick: Some(200),
        });

        let mut app = app_with(log, 200);
        app.update();

        let index = app.world().resource::<MineralOreIndex>();
        assert!(
            index.is_empty(),
            "unknown mineral_id must not produce a spawned OreNode"
        );
        let mut query = app.world_mut().query::<&MineralOreNode>();
        assert_eq!(query.iter(app.world()).count(), 0);
    }

    #[test]
    fn already_indexed_position_is_not_double_spawned() {
        let mut log = ExhaustedMineralsLog::default();
        log.record(ExhaustedEntry {
            mineral_id: "fan_tie".into(),
            x: 0,
            y: 64,
            z: 0,
            tick: 100,
            respawn_at_tick: Some(200),
        });

        let mut app = app_with(log, 200);
        // Pre-seed the index at the same position with a placeholder entity to
        // simulate the defensive "already occupied" branch.
        let placeholder = app.world_mut().spawn_empty().id();
        app.world_mut().resource_mut::<MineralOreIndex>().insert(
            DimensionKind::Overworld,
            BlockPos::new(0, 64, 0),
            placeholder,
        );

        app.update();

        let mut query = app.world_mut().query::<&MineralOreNode>();
        assert_eq!(
            query.iter(app.world()).count(),
            0,
            "already-occupied position must not receive a second MineralOreNode entity"
        );
        // Expired entry is still consumed out of the log (remove_respawned already popped it
        // before the occupancy check runs), matching anchors.rs's dedup-then-skip pattern.
        let exhausted = app.world().resource::<ExhaustedMineralsLog>();
        assert!(exhausted.entries().is_empty());
    }
}
