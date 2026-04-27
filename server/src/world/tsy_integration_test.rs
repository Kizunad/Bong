//! plan-tsy-zone-followup-v1 §1 — TSY zone 端到端集成测试。
//!
//! **路径决策**：plan §1.1 原写 `server/tests/tsy_zone_integration.rs`，但本 crate 是 `bin`
//! only（无 `lib.rs`），cargo `tests/*.rs` 集成测无 lib 可链。同等价方案：在 `src/world/`
//! 下放一个 `#[cfg(test)] mod` 把多个 tsy_* system 串到同一个 `App` 里跑——比 unit test
//! 大、比真 valence harness 小，正好覆盖 plan §5.2 想验证的"几个 system 协作"维度。
//!
//! 不依赖 `valence::testing::ScenarioSingleClient`、不起真网络，纯 ECS。

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use valence::prelude::{App, DVec3, Events, IntoSystemConfigs, Position, Update};

    use crate::combat::events::DeathEvent;
    use crate::combat::CombatClock;
    use crate::inventory::{InventoryRevision, ItemInstance, ItemRarity, PlayerInventory};
    use crate::player::state::PlayerState;
    use crate::world::dimension::{CurrentDimension, DimensionKind};
    use crate::world::dimension_transfer::DimensionTransferRequest;
    use crate::world::tsy::{DimensionAnchor, PortalDirection, RiftPortal, TsyPresence};
    use crate::world::tsy_drain::tsy_drain_tick;
    use crate::world::tsy_portal::{
        tsy_entry_portal_tick, tsy_exit_portal_tick, TsyEnterEmit, TsyExitEmit,
    };
    use crate::world::zone::{Zone, ZoneRegistry};

    fn fresh_app() -> App {
        let mut app = App::new();
        app.insert_resource(CombatClock::default());
        app.insert_resource(ZoneRegistry::fallback());
        app.add_event::<DimensionTransferRequest>();
        app.add_event::<TsyEnterEmit>();
        app.add_event::<TsyExitEmit>();
        app.add_event::<DeathEvent>();
        // 全部 tsy_* system 接到 Update；DimensionTransferRequest 我们在 assert
        // 时直接读 Events，不需要真 apply_dimension_transfers
        app.add_systems(
            Update,
            (tsy_entry_portal_tick, tsy_exit_portal_tick, tsy_drain_tick).chain(),
        );
        app
    }

    fn register_lingxu_subzones(app: &mut App) -> DVec3 {
        // 装一个 family：tsy_lingxu_01_{shallow, mid, deep}。
        // shallow Y∈[40,120]、center=(50,80,50)
        let shallow_center = DVec3::new(50.0, 80.0, 50.0);
        let mut zones = app.world_mut().resource_mut::<ZoneRegistry>();
        for (name, ymin, ymax, qi) in [
            ("tsy_lingxu_01_shallow", 40.0, 120.0, -0.4),
            ("tsy_lingxu_01_mid", 0.0, 40.0, -0.7),
            ("tsy_lingxu_01_deep", -40.0, 0.0, -1.1),
        ] {
            zones
                .register_runtime_zone(Zone {
                    name: name.to_string(),
                    dimension: DimensionKind::Tsy,
                    bounds: (DVec3::new(0.0, ymin, 0.0), DVec3::new(100.0, ymax, 100.0)),
                    spirit_qi: qi,
                    danger_level: 5,
                    active_events: if name.ends_with("_shallow") {
                        vec!["tsy_entry".to_string()]
                    } else {
                        Vec::new()
                    },
                    patrol_anchors: Vec::new(),
                    blocked_tiles: Vec::new(),
                })
                .expect("register_runtime_zone ok");
        }
        shallow_center
    }

    fn make_player_inventory_with_qi_item() -> PlayerInventory {
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
            spirit_quality: 0.7,
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

    fn make_player_state(spirit_qi: f64, spirit_qi_max: f64) -> PlayerState {
        PlayerState {
            spirit_qi,
            spirit_qi_max,
            ..Default::default()
        }
    }

    /// A. plan §1.2 entry_full_path:
    /// 玩家踏进 Entry portal → emit TsyEnterEmit + 入场过滤 + attach TsyPresence
    /// + DimensionTransferRequest(Tsy, target.pos)。
    #[test]
    fn a_entry_full_path_attaches_presence_and_strips_qi_item() {
        let mut app = fresh_app();
        let shallow_center = register_lingxu_subzones(&mut app);

        // Entry portal 在主世界 (0,64,0)，target = TSY shallow center
        app.world_mut().spawn((
            Position::new([0.0, 64.0, 0.0]),
            RiftPortal {
                family_id: "tsy_lingxu_01".to_string(),
                target: DimensionAnchor {
                    dimension: DimensionKind::Tsy,
                    pos: shallow_center,
                },
                trigger_radius: 1.5,
                direction: PortalDirection::Entry,
            },
        ));
        let player = app
            .world_mut()
            .spawn((
                Position::new([0.5, 64.0, 0.0]),
                make_player_inventory_with_qi_item(),
                make_player_state(50.0, 50.0),
                CurrentDimension(DimensionKind::Overworld),
            ))
            .id();

        app.update();

        // 1) TsyEnterEmit fired
        let enter_events = app.world().resource::<Events<TsyEnterEmit>>();
        let enter_collected: Vec<_> = enter_events
            .get_reader()
            .read(enter_events)
            .cloned()
            .collect();
        assert_eq!(enter_collected.len(), 1, "expect exactly one TsyEnterEmit");
        let ev = &enter_collected[0];
        assert_eq!(ev.family_id, "tsy_lingxu_01");
        assert_eq!(ev.return_to.dimension, DimensionKind::Overworld);
        assert_eq!(
            ev.filtered.len(),
            1,
            "spirit_quality=0.7 item should be filtered"
        );
        assert_eq!(ev.filtered[0].instance_id, 7);

        // 2) DimensionTransferRequest sent for player → Tsy / shallow center
        let dim_events = app.world().resource::<Events<DimensionTransferRequest>>();
        let dim_collected: Vec<_> = dim_events.get_reader().read(dim_events).cloned().collect();
        assert_eq!(dim_collected.len(), 1);
        assert_eq!(dim_collected[0].entity, player);
        assert_eq!(dim_collected[0].target, DimensionKind::Tsy);
        assert_eq!(dim_collected[0].target_pos, shallow_center);

        // 3) TsyPresence attached
        let presence = app
            .world()
            .entity(player)
            .get::<TsyPresence>()
            .expect("TsyPresence attached after entry");
        assert_eq!(presence.family_id, "tsy_lingxu_01");

        // 4) Inventory item stripped
        let inv = app.world().entity(player).get::<PlayerInventory>().unwrap();
        let stripped = inv.hotbar[0].as_ref().expect("hotbar slot retained");
        assert_eq!(stripped.spirit_quality, 0.0);
        assert_eq!(stripped.display_name, "枯骨残片");
    }

    /// B. plan §1.2 drain_after_entry:
    /// 玩家持 TsyPresence + 在 TSY shallow zone 内 + Tsy dim → spirit_qi 按 §2.1 公式衰减。
    #[test]
    fn b_drain_after_entry_decreases_spirit_qi_per_tick() {
        let mut app = fresh_app();
        let shallow_center = register_lingxu_subzones(&mut app);

        // 玩家直接 spawn 在 TSY 内（跳过 entry，专注 drain 验证）
        let player = app
            .world_mut()
            .spawn((
                Position::new([shallow_center.x, shallow_center.y, shallow_center.z]),
                make_player_state(50.0, 50.0),
                CurrentDimension(DimensionKind::Tsy),
                TsyPresence {
                    family_id: "tsy_lingxu_01".to_string(),
                    entered_at_tick: 0,
                    entry_inventory_snapshot: Vec::new(),
                    return_to: DimensionAnchor {
                        dimension: DimensionKind::Overworld,
                        pos: DVec3::new(0.0, 65.0, 0.0),
                    },
                },
            ))
            .id();

        let n = 10_u32;
        for _ in 0..n {
            app.update();
        }

        let qi_after = app
            .world()
            .entity(player)
            .get::<PlayerState>()
            .unwrap()
            .spirit_qi;

        // §2.1 公式：rate = |qi=-0.4| * (50/100)^1.5 * 0.5 = 0.4 * 0.354 * 0.5 ≈ 0.0707/tick
        // n=10 ticks → 期望降幅 ≈ 0.707，下界给 0.5 留容差
        let expected_min = 0.5_f64;
        let drained = 50.0 - qi_after;
        assert!(
            drained >= expected_min,
            "expected drain ≥ {expected_min} after {n} ticks @ shallow, got {drained:.3}"
        );
    }

    /// C. plan §1.2 drain_to_zero_emits_death_event:
    /// 真元小到 1 tick 内被抽干 → DeathEvent(cause="tsy_drain") 发出。
    #[test]
    fn c_drain_to_zero_emits_death_event() {
        let mut app = fresh_app();
        let shallow_center = register_lingxu_subzones(&mut app);

        // spirit_qi 故意调小 + 池子大让 drain rate 高 → 1 tick 即可归零
        let _player = app
            .world_mut()
            .spawn((
                Position::new([shallow_center.x, 80.0, shallow_center.z]),
                PlayerState {
                    spirit_qi: 0.001,
                    spirit_qi_max: 500.0, // 化虚，rate 巨大
                    ..Default::default()
                },
                CurrentDimension(DimensionKind::Tsy),
                TsyPresence {
                    family_id: "tsy_lingxu_01".to_string(),
                    entered_at_tick: 0,
                    entry_inventory_snapshot: Vec::new(),
                    return_to: DimensionAnchor {
                        dimension: DimensionKind::Overworld,
                        pos: DVec3::new(0.0, 65.0, 0.0),
                    },
                },
            ))
            .id();

        app.update();

        let death_events = app.world().resource::<Events<DeathEvent>>();
        let collected: Vec<_> = death_events
            .get_reader()
            .read(death_events)
            .cloned()
            .collect();
        assert_eq!(collected.len(), 1, "expect one DeathEvent on qi=0");
        assert_eq!(collected[0].cause, "tsy_drain");
    }

    /// D. plan §1.2 exit_round_trip:
    /// 玩家在 TSY 内 + 持 TsyPresence + 走到对应 family Exit portal trigger 内
    /// → 发 DimensionTransferRequest(Overworld, return_to.pos) + remove TsyPresence
    /// + emit TsyExitEmit。
    #[test]
    fn d_exit_round_trip_removes_presence_and_routes_back() {
        let mut app = fresh_app();
        register_lingxu_subzones(&mut app);

        // Exit portal 在 TSY shallow center
        let exit_pos = DVec3::new(50.0, 80.0, 50.0);
        let return_to = DimensionAnchor {
            dimension: DimensionKind::Overworld,
            pos: DVec3::new(2.5, 65.0, 0.0), // codex P1 修复后形态：escape margin
        };
        app.world_mut().spawn((
            Position::new([exit_pos.x, exit_pos.y, exit_pos.z]),
            RiftPortal {
                family_id: "tsy_lingxu_01".to_string(),
                target: DimensionAnchor {
                    dimension: DimensionKind::Overworld,
                    pos: return_to.pos,
                },
                trigger_radius: 1.5,
                direction: PortalDirection::Exit,
            },
        ));

        let player = app
            .world_mut()
            .spawn((
                Position::new([exit_pos.x + 0.5, exit_pos.y, exit_pos.z]),
                make_player_state(50.0, 50.0),
                CurrentDimension(DimensionKind::Tsy),
                TsyPresence {
                    family_id: "tsy_lingxu_01".to_string(),
                    entered_at_tick: 100,
                    entry_inventory_snapshot: Vec::new(),
                    return_to,
                },
            ))
            .id();

        // 跑一 tick：tsy_exit_portal_tick 命中 + 同 tick tsy_drain_tick 还能再抽一次
        // （drain 把 spirit_qi 减一点，之后 exit 触发 + 移除 TsyPresence）
        app.world_mut().resource_mut::<CombatClock>().tick = 200;
        app.update();

        // 1) TsyExitEmit fired
        let exit_events = app.world().resource::<Events<TsyExitEmit>>();
        let exit_collected: Vec<_> = exit_events
            .get_reader()
            .read(exit_events)
            .cloned()
            .collect();
        assert_eq!(exit_collected.len(), 1);
        assert_eq!(exit_collected[0].family_id, "tsy_lingxu_01");
        assert_eq!(exit_collected[0].duration_ticks, 100);

        // 2) DimensionTransferRequest sent → Overworld + return_to.pos
        let dim_events = app.world().resource::<Events<DimensionTransferRequest>>();
        let dim_collected: Vec<_> = dim_events.get_reader().read(dim_events).cloned().collect();
        assert_eq!(dim_collected.len(), 1);
        assert_eq!(dim_collected[0].target, DimensionKind::Overworld);
        assert_eq!(dim_collected[0].target_pos, return_to.pos);

        // 3) TsyPresence removed
        assert!(
            app.world().entity(player).get::<TsyPresence>().is_none(),
            "TsyPresence should be removed after exit"
        );
    }
}
