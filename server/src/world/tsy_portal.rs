//! plan-tsy-zone-v1 §3.3 / §3.4 — TSY 裂缝 entry / exit 传送 system。
//!
//! 架构反转后（`plan-tsy-dimension-v1` 落地），传送不再自己 `insert Position`，
//! 改为发 `DimensionTransferRequest` 让 `apply_dimension_transfers` 系统统一处理
//! layer 切换 + Position 更新 + Respawn packet（`Changed<VisibleChunkLayer>`
//! 由 valence 自动转 PlayerRespawnS2c）。

use valence::prelude::{
    bevy_ecs, App, Commands, DVec3, Entity, Event, EventWriter, IntoSystemConfigs, Position, Query,
    Res, Update, Without,
};

use crate::combat::CombatClock;
use crate::inventory::PlayerInventory;
use crate::npc::spawn::NpcMarker;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::dimension_transfer::{DimensionTransferRequest, DimensionTransferSet};
use crate::world::tsy::{DimensionAnchor, PortalDirection, RiftPortal, TsyPresence};
use crate::world::tsy_filter::{apply_entry_filter, FilteredItem};

/// 出关时把玩家落点推到 entry portal trigger_radius **外** 的安全裕度（格）。
/// 偏移量 = `trigger_radius + RETURN_ESCAPE_MARGIN`；保证欧氏距离严格大于 radius
/// 即可——避免出关那一瞬间被同 portal 吸回去。
const RETURN_ESCAPE_MARGIN: f64 = 1.0;

/// 玩家踏进 Entry 裂缝时由 portal system emit。
///
/// 由 `plan-tsy-zone-v1 §1.4` schema 接成 IPC `tsy_enter` event 推到 agent 层；
/// P0 仅在 server 内部产 emit 信号，schema bridge 由后续 commit / agent plan 接通。
#[allow(dead_code)] // 字段由 schema bridge / agent IPC 后续接通时消费
#[derive(Event, Debug, Clone)]
pub struct TsyEnterEmit {
    pub player_entity: Entity,
    pub family_id: String,
    /// 出关锚点（=入场时 `TsyPresence.return_to`，方便下游写日志）。
    pub return_to: DimensionAnchor,
    pub filtered: Vec<FilteredItem>,
}

/// 玩家踏进 Exit 裂缝时由 portal system emit。
#[allow(dead_code)] // 字段由 schema bridge / agent IPC 后续接通时消费
#[derive(Event, Debug, Clone)]
pub struct TsyExitEmit {
    pub player_entity: Entity,
    pub family_id: String,
    pub duration_ticks: u64,
}

/// plan §3.3 — Entry portal system。
///
/// gate：玩家在 `Overworld` + 没有 `TsyPresence` + 没有 `NpcMarker`。
/// 触发：玩家中心距某个 `Entry` 标记的 `RiftPortal` ≤ trigger_radius。
/// 结果：跑入场过滤 → attach `TsyPresence` → 发 `DimensionTransferRequest` → emit。
#[allow(clippy::type_complexity)]
pub fn tsy_entry_portal_tick(
    mut commands: Commands,
    clock: Res<CombatClock>,
    mut players: Query<
        (Entity, &Position, &mut PlayerInventory, &CurrentDimension),
        (Without<TsyPresence>, Without<NpcMarker>),
    >,
    portals: Query<(&Position, &RiftPortal)>,
    mut dim_transfer: EventWriter<DimensionTransferRequest>,
    mut emit: EventWriter<TsyEnterEmit>,
) {
    for (player_entity, player_pos, mut inv, cur_dim) in players.iter_mut() {
        if cur_dim.0 != DimensionKind::Overworld {
            continue;
        }

        for (portal_pos, portal) in &portals {
            if !matches!(portal.direction, PortalDirection::Entry) {
                continue;
            }
            if player_pos.0.distance(portal_pos.0) > portal.trigger_radius {
                continue;
            }

            // Step 1: 入场过滤（剥离高灵质物品）
            let filtered = apply_entry_filter(&mut inv);

            // Step 2: 出关锚点 — 必须落在 entry portal 的 trigger_radius **外**，
            // 否则出关后下一 tick 又会被同一个 entry portal 吸进去（plan §3.3 文档
            // 写 `+ (0,1,0)` 是漏洞，距离只 1.0，而默认 trigger_radius=1.5）。
            // 沿 +X 方向偏移 (radius + RETURN_ESCAPE_MARGIN)，保证欧氏距离 > radius。
            let escape_offset = portal.trigger_radius + RETURN_ESCAPE_MARGIN;
            let return_to = DimensionAnchor {
                dimension: DimensionKind::Overworld,
                pos: portal_pos.0 + DVec3::new(escape_offset, 1.0, 0.0),
            };

            // Step 3: attach TsyPresence
            commands.entity(player_entity).insert(TsyPresence {
                family_id: portal.family_id.clone(),
                entered_at_tick: clock.tick,
                entry_inventory_snapshot: collect_instance_ids(&inv),
                return_to,
            });

            // Step 4: 发跨位面传送请求
            dim_transfer.send(DimensionTransferRequest {
                entity: player_entity,
                target: portal.target.dimension,
                target_pos: portal.target.pos,
            });

            // Step 5: emit Tsy enter event
            emit.send(TsyEnterEmit {
                player_entity,
                family_id: portal.family_id.clone(),
                return_to,
                filtered,
            });

            break; // 一玩家一 tick 只能进一个 portal
        }
    }
}

/// plan §3.4 — Exit portal system。
///
/// gate：玩家在 `Tsy` + 持有 `TsyPresence`。
/// 触发：踏进 family_id 匹配的 Exit `RiftPortal` 半径内。
/// 结果：发跨位面传回 `presence.return_to` → 移除 `TsyPresence` → emit。
#[allow(clippy::type_complexity)]
pub fn tsy_exit_portal_tick(
    mut commands: Commands,
    clock: Res<CombatClock>,
    players: Query<(Entity, &Position, &TsyPresence, &CurrentDimension), Without<NpcMarker>>,
    portals: Query<(&Position, &RiftPortal)>,
    mut dim_transfer: EventWriter<DimensionTransferRequest>,
    mut emit: EventWriter<TsyExitEmit>,
) {
    for (entity, pos, presence, cur_dim) in &players {
        if cur_dim.0 != DimensionKind::Tsy {
            continue;
        }

        for (portal_pos, portal) in &portals {
            if !matches!(portal.direction, PortalDirection::Exit) {
                continue;
            }
            if portal.family_id != presence.family_id {
                continue;
            }
            if pos.0.distance(portal_pos.0) > portal.trigger_radius {
                continue;
            }

            dim_transfer.send(DimensionTransferRequest {
                entity,
                target: presence.return_to.dimension,
                target_pos: presence.return_to.pos,
            });
            commands.entity(entity).remove::<TsyPresence>();
            emit.send(TsyExitEmit {
                player_entity: entity,
                family_id: presence.family_id.clone(),
                duration_ticks: clock.tick.saturating_sub(presence.entered_at_tick),
            });

            break;
        }
    }
}

fn collect_instance_ids(inv: &PlayerInventory) -> Vec<u64> {
    let mut ids = Vec::new();
    for c in &inv.containers {
        for placed in &c.items {
            ids.push(placed.instance.instance_id);
        }
    }
    for item in inv.equipped.values() {
        ids.push(item.instance_id);
    }
    for slot in inv.hotbar.iter() {
        if let Some(item) = slot {
            ids.push(item.instance_id);
        }
    }
    ids
}

/// 把 Entry / Exit portal tick 接到 valence Update schedule，并约束在
/// `DimensionTransferSet` 之前 —— 这样同 tick 内发的 `DimensionTransferRequest`
/// 会在本 tick 末被 `apply_dimension_transfers` 立即消费，玩家不需要等 1 tick。
pub fn register(app: &mut App) {
    app.add_event::<TsyEnterEmit>()
        .add_event::<TsyExitEmit>()
        .add_systems(
            Update,
            (tsy_entry_portal_tick, tsy_exit_portal_tick).before(DimensionTransferSet),
        );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::CombatClock;
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemInstance, ItemRarity, PlacedItemState,
        PlayerInventory,
    };
    use std::collections::HashMap;
    use valence::prelude::{
        App, DVec3, EntityLayerId, Events, VisibleChunkLayer, VisibleEntityLayers,
    };

    fn make_inv_with_qi_item() -> PlayerInventory {
        let mut hb: [Option<ItemInstance>; 9] = Default::default();
        hb[0] = Some(ItemInstance {
            instance_id: 7,
            template_id: "bone_coin".to_string(),
            display_name: "满灵骨币".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.1,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 0.8,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
        });
        PlayerInventory {
            revision: InventoryRevision(1),
            containers: Vec::new(),
            equipped: HashMap::new(),
            hotbar: hb,
            bone_coins: 0,
            max_weight: 100.0,
        }
    }

    fn empty_inv() -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(1),
            containers: vec![ContainerState {
                id: "bag".to_string(),
                name: "Bag".to_string(),
                rows: 4,
                cols: 4,
                items: vec![PlacedItemState {
                    row: 0,
                    col: 0,
                    instance: ItemInstance {
                        instance_id: 1,
                        template_id: "stone".to_string(),
                        display_name: "石头".to_string(),
                        grid_w: 1,
                        grid_h: 1,
                        weight: 1.0,
                        rarity: ItemRarity::Common,
                        description: String::new(),
                        stack_count: 1,
                        spirit_quality: 0.0,
                        durability: 1.0,
                        freshness: None,
                        mineral_id: None,
                    },
                }],
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 100.0,
        }
    }

    #[test]
    fn entry_portal_skips_when_player_already_in_tsy() {
        // 玩家已在 TSY dim → cur_dim filter 拒绝触发。
        let mut app = App::new();
        app.add_event::<DimensionTransferRequest>();
        app.add_event::<TsyEnterEmit>();
        app.insert_resource(CombatClock::default());
        app.add_systems(Update, tsy_entry_portal_tick);

        // Spawn a portal at (0,64,0)
        app.world_mut().spawn((
            Position::new([0.0, 64.0, 0.0]),
            RiftPortal {
                family_id: "tsy_lingxu_01".to_string(),
                target: DimensionAnchor {
                    dimension: DimensionKind::Tsy,
                    pos: DVec3::new(50.0, 80.0, 50.0),
                },
                trigger_radius: 1.5,
                direction: PortalDirection::Entry,
            },
        ));

        // Player already in TSY (CurrentDimension::Tsy) at (0.5, 64, 0)
        app.world_mut().spawn((
            Position::new([0.5, 64.0, 0.0]),
            empty_inv(),
            CurrentDimension(DimensionKind::Tsy),
        ));

        app.update();

        let events = app.world().resource::<Events<TsyEnterEmit>>();
        let mut reader = events.get_reader();
        assert_eq!(
            reader.read(events).count(),
            0,
            "should not emit when player not in overworld"
        );
    }

    #[test]
    fn entry_portal_outside_radius_does_not_trigger() {
        let mut app = App::new();
        app.add_event::<DimensionTransferRequest>();
        app.add_event::<TsyEnterEmit>();
        app.insert_resource(CombatClock::default());
        app.add_systems(Update, tsy_entry_portal_tick);

        app.world_mut().spawn((
            Position::new([0.0, 64.0, 0.0]),
            RiftPortal {
                family_id: "tsy_lingxu_01".to_string(),
                target: DimensionAnchor {
                    dimension: DimensionKind::Tsy,
                    pos: DVec3::new(50.0, 80.0, 50.0),
                },
                trigger_radius: 1.5,
                direction: PortalDirection::Entry,
            },
        ));

        // Player far away (10 blocks)
        app.world_mut().spawn((
            Position::new([10.0, 64.0, 0.0]),
            empty_inv(),
            CurrentDimension(DimensionKind::Overworld),
        ));

        app.update();
        let events = app.world().resource::<Events<TsyEnterEmit>>();
        assert_eq!(events.get_reader().read(events).count(), 0);
    }

    #[test]
    fn entry_portal_within_radius_triggers_filter_attach_and_transfer() {
        let mut app = App::new();
        app.add_event::<DimensionTransferRequest>();
        app.add_event::<TsyEnterEmit>();
        app.insert_resource(CombatClock { tick: 100 });
        app.add_systems(Update, tsy_entry_portal_tick);

        app.world_mut().spawn((
            Position::new([0.0, 64.0, 0.0]),
            RiftPortal {
                family_id: "tsy_lingxu_01".to_string(),
                target: DimensionAnchor {
                    dimension: DimensionKind::Tsy,
                    pos: DVec3::new(50.0, 80.0, 50.0),
                },
                trigger_radius: 1.5,
                direction: PortalDirection::Entry,
            },
        ));

        let player = app
            .world_mut()
            .spawn((
                Position::new([0.5, 64.0, 0.0]),
                make_inv_with_qi_item(),
                CurrentDimension(DimensionKind::Overworld),
            ))
            .id();

        app.update();

        // 1) TsyEnterEmit was fired
        {
            let events = app.world().resource::<Events<TsyEnterEmit>>();
            let mut reader = events.get_reader();
            let collected: Vec<_> = reader.read(events).cloned().collect();
            assert_eq!(collected.len(), 1);
            let ev = &collected[0];
            assert_eq!(ev.family_id, "tsy_lingxu_01");
            assert_eq!(ev.return_to.dimension, DimensionKind::Overworld);
            // 出关锚点必须落在 entry portal 的 trigger_radius 外（escape margin），
            // 这里 portal_pos = (0,64,0) + (radius=1.5 + margin=1.0, 1.0, 0.0) = (2.5, 65.0, 0.0)
            assert_eq!(
                ev.return_to.pos,
                DVec3::new(0.0 + 1.5 + RETURN_ESCAPE_MARGIN, 65.0, 0.0)
            );
            assert!(
                ev.return_to.pos.distance(DVec3::new(0.0, 64.0, 0.0)) > 1.5,
                "return_to must be outside entry trigger_radius (1.5) to avoid re-entry loop"
            );
            assert_eq!(ev.filtered.len(), 1, "高灵质骨币应当被过滤一次");
            assert_eq!(ev.filtered[0].instance_id, 7);
        }

        // 2) DimensionTransferRequest sent
        {
            let events = app.world().resource::<Events<DimensionTransferRequest>>();
            let collected: Vec<_> = events.get_reader().read(events).cloned().collect();
            assert_eq!(collected.len(), 1);
            assert_eq!(collected[0].target, DimensionKind::Tsy);
            assert_eq!(collected[0].target_pos, DVec3::new(50.0, 80.0, 50.0));
            assert_eq!(collected[0].entity, player);
        }

        // 3) TsyPresence attached
        let presence = app
            .world()
            .entity(player)
            .get::<TsyPresence>()
            .expect("TsyPresence should be attached");
        assert_eq!(presence.family_id, "tsy_lingxu_01");
        assert_eq!(presence.entered_at_tick, 100);
        assert!(
            !presence.entry_inventory_snapshot.is_empty(),
            "snapshot should include the original instance_ids"
        );

        // 4) 物品被剥离（spirit_quality = 0 + 改名）
        let inv = app.world().entity(player).get::<PlayerInventory>().unwrap();
        let bone = inv.hotbar[0].as_ref().unwrap();
        assert_eq!(bone.spirit_quality, 0.0);
        assert_eq!(bone.display_name, "枯骨残片");
    }

    #[test]
    fn exit_portal_round_trip_removes_presence_and_sends_transfer() {
        let mut app = App::new();
        app.add_event::<DimensionTransferRequest>();
        app.add_event::<TsyExitEmit>();
        app.insert_resource(CombatClock { tick: 200 });
        app.add_systems(Update, tsy_exit_portal_tick);

        // Exit portal at (50, 80, 50) — TSY dim
        app.world_mut().spawn((
            Position::new([50.0, 80.0, 50.0]),
            RiftPortal {
                family_id: "tsy_lingxu_01".to_string(),
                target: DimensionAnchor {
                    dimension: DimensionKind::Overworld,
                    pos: DVec3::new(0.0, 65.0, 0.0),
                },
                trigger_radius: 1.5,
                direction: PortalDirection::Exit,
            },
        ));

        let return_to = DimensionAnchor {
            dimension: DimensionKind::Overworld,
            pos: DVec3::new(0.0, 65.0, 0.0),
        };
        let player = app
            .world_mut()
            .spawn((
                Position::new([50.5, 80.0, 50.0]),
                CurrentDimension(DimensionKind::Tsy),
                TsyPresence {
                    family_id: "tsy_lingxu_01".to_string(),
                    entered_at_tick: 100,
                    entry_inventory_snapshot: vec![1, 2, 3],
                    return_to,
                },
            ))
            .id();

        app.update();

        // 1) TsyExitEmit fired with duration = 100
        let events = app.world().resource::<Events<TsyExitEmit>>();
        let collected: Vec<_> = events.get_reader().read(events).cloned().collect();
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].duration_ticks, 100);
        assert_eq!(collected[0].family_id, "tsy_lingxu_01");

        // 2) DimensionTransferRequest sent toward Overworld return_to
        let dim_events = app.world().resource::<Events<DimensionTransferRequest>>();
        let dim_collected: Vec<_> = dim_events.get_reader().read(dim_events).cloned().collect();
        assert_eq!(dim_collected.len(), 1);
        assert_eq!(dim_collected[0].target, DimensionKind::Overworld);
        assert_eq!(dim_collected[0].target_pos, DVec3::new(0.0, 65.0, 0.0));

        // 3) TsyPresence removed
        assert!(
            app.world().entity(player).get::<TsyPresence>().is_none(),
            "presence should be removed after exit"
        );
    }

    #[test]
    fn exit_portal_rejects_other_family_id() {
        // 走到非自己 family 的 Exit portal 时应该被忽略。
        let mut app = App::new();
        app.add_event::<DimensionTransferRequest>();
        app.add_event::<TsyExitEmit>();
        app.insert_resource(CombatClock { tick: 200 });
        app.add_systems(Update, tsy_exit_portal_tick);

        // Exit portal for "tsy_other_99"
        app.world_mut().spawn((
            Position::new([50.0, 80.0, 50.0]),
            RiftPortal {
                family_id: "tsy_other_99".to_string(),
                target: DimensionAnchor {
                    dimension: DimensionKind::Overworld,
                    pos: DVec3::new(0.0, 65.0, 0.0),
                },
                trigger_radius: 1.5,
                direction: PortalDirection::Exit,
            },
        ));

        // Player presence is tsy_lingxu_01
        let return_to = DimensionAnchor {
            dimension: DimensionKind::Overworld,
            pos: DVec3::new(0.0, 65.0, 0.0),
        };
        app.world_mut().spawn((
            Position::new([50.5, 80.0, 50.0]),
            CurrentDimension(DimensionKind::Tsy),
            TsyPresence {
                family_id: "tsy_lingxu_01".to_string(),
                entered_at_tick: 100,
                entry_inventory_snapshot: Vec::new(),
                return_to,
            },
        ));

        app.update();
        let events = app.world().resource::<Events<TsyExitEmit>>();
        assert_eq!(
            events.get_reader().read(events).count(),
            0,
            "wrong family_id should not trigger exit"
        );
    }

    /// Regression test for codex review P1：出关后玩家被传到 return_to，必须
    /// 落在 entry portal trigger_radius 外，否则下一 tick 又被吸进去。
    ///
    /// 这里只验证"几何不变量"：从 entry tick 计算出的 return_to.pos 严格在
    /// entry portal trigger_radius 之外（不需要再跑 second-tick 模拟入场判定）。
    #[test]
    fn return_to_pos_lies_strictly_outside_entry_trigger_radius() {
        let mut app = App::new();
        app.add_event::<DimensionTransferRequest>();
        app.add_event::<TsyEnterEmit>();
        app.insert_resource(CombatClock { tick: 0 });
        app.add_systems(Update, tsy_entry_portal_tick);

        let portal_pos = DVec3::new(10.0, 64.0, 7.0);
        let trigger_radius = 1.5_f64;

        app.world_mut().spawn((
            Position::new([portal_pos.x, portal_pos.y, portal_pos.z]),
            RiftPortal {
                family_id: "tsy_test".to_string(),
                target: DimensionAnchor {
                    dimension: DimensionKind::Tsy,
                    pos: DVec3::new(50.0, 80.0, 50.0),
                },
                trigger_radius,
                direction: PortalDirection::Entry,
            },
        ));
        app.world_mut().spawn((
            Position::new([portal_pos.x + 0.2, portal_pos.y, portal_pos.z]),
            empty_inv(),
            CurrentDimension(DimensionKind::Overworld),
        ));

        app.update();

        let events = app.world().resource::<Events<TsyEnterEmit>>();
        let collected: Vec<_> = events.get_reader().read(events).cloned().collect();
        assert_eq!(collected.len(), 1, "should have entered exactly once");
        let return_to_pos = collected[0].return_to.pos;
        let dist = return_to_pos.distance(portal_pos);
        assert!(
            dist > trigger_radius,
            "return_to.pos {return_to_pos:?} is at distance {dist} from portal {portal_pos:?}; \
             must be > trigger_radius={trigger_radius} to prevent re-entry loop"
        );
    }

    /// Regression test for codex review P1：portal 自定义 trigger_radius 时
    /// escape offset 也要按 radius 自适应——不是写死 1 格。
    #[test]
    fn return_to_escape_offset_adapts_to_custom_trigger_radius() {
        let mut app = App::new();
        app.add_event::<DimensionTransferRequest>();
        app.add_event::<TsyEnterEmit>();
        app.insert_resource(CombatClock { tick: 0 });
        app.add_systems(Update, tsy_entry_portal_tick);

        // 故意调成大半径
        let trigger_radius = 5.0_f64;
        app.world_mut().spawn((
            Position::new([0.0, 64.0, 0.0]),
            RiftPortal {
                family_id: "tsy_big".to_string(),
                target: DimensionAnchor {
                    dimension: DimensionKind::Tsy,
                    pos: DVec3::new(50.0, 80.0, 50.0),
                },
                trigger_radius,
                direction: PortalDirection::Entry,
            },
        ));
        app.world_mut().spawn((
            Position::new([2.0, 64.0, 0.0]),
            empty_inv(),
            CurrentDimension(DimensionKind::Overworld),
        ));

        app.update();
        let events = app.world().resource::<Events<TsyEnterEmit>>();
        let collected: Vec<_> = events.get_reader().read(events).cloned().collect();
        assert_eq!(collected.len(), 1);
        let dist = collected[0]
            .return_to
            .pos
            .distance(DVec3::new(0.0, 64.0, 0.0));
        assert!(
            dist > trigger_radius,
            "with trigger_radius=5.0, return_to must escape > 5.0; got dist={dist}"
        );
    }

    // unused imports suppressor
    #[allow(dead_code)]
    fn _unused_imports_suppressor() {
        let _ = (EntityLayerId, VisibleChunkLayer, VisibleEntityLayers);
    }
}
