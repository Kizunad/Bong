//! plan-tsy-loot-v1 §8.2 — TSY loot 端到端集成测试。
//!
//! 与 `world/tsy_integration_test.rs` 同模式：因 crate 是 bin-only，没有 `tests/`
//! 目录，所有"几个 system 串起来跑一个 App"的集成测都放在 src 子模块里。
//!
//! 覆盖：
//! - 入场首次 spawn 上古遗物（spawn_on_enter 系统）
//! - 重复入场 idempotent（不再 spawn）
//! - 主世界死亡走原 50% 路径，不 spawn 干尸
//! - TSY 内死亡走分流：秘境所得 100% / 原带 50% + spawn 干尸
//! - 上古遗物 rarity = Ancient + spirit_quality = 0 → loot 入 inventory 后保持

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use valence::prelude::{App, DVec3, Events, Position, Update};

    use crate::combat::CombatClock;
    use crate::cultivation::death_hooks::PlayerRevived;
    use crate::inventory::{
        ancient_relics::AncientRelicPool,
        apply_death_drop_on_revive,
        corpse::CorpseEmbalmed,
        tsy_loot_spawn::{tsy_loot_spawn_on_enter, TsySpawnedFamilies},
        ContainerState, DroppedItemEvent, DroppedLootRegistry, InventoryInstanceIdAllocator,
        InventoryRevision, ItemInstance, ItemRarity, ItemRegistry, PlacedItemState,
        PlayerInventory,
    };
    use crate::world::dimension::DimensionKind;
    use crate::world::tsy::{DimensionAnchor, TsyPresence};
    use crate::world::tsy_portal::TsyEnterEmit;
    use crate::world::zone::{Zone, ZoneRegistry};

    fn item(id: u64, name: &str) -> ItemInstance {
        ItemInstance {
            instance_id: id,
            template_id: format!("test_{name}"),
            display_name: name.into(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.5,
            rarity: ItemRarity::Common,
            description: "test".into(),
            stack_count: 1,
            spirit_quality: 0.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
        }
    }

    fn make_inventory(items: Vec<ItemInstance>) -> PlayerInventory {
        let placed = items
            .into_iter()
            .enumerate()
            .map(|(i, instance)| PlacedItemState {
                row: 0,
                col: i as u8,
                instance,
            })
            .collect();
        PlayerInventory {
            revision: InventoryRevision(1),
            containers: vec![ContainerState {
                id: "main_pack".into(),
                name: "main".into(),
                rows: 1,
                cols: 16,
                items: placed,
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 45.0,
        }
    }

    fn register_lingxu_subzones(app: &mut App) {
        let mut zones = app.world_mut().resource_mut::<ZoneRegistry>();
        for (name, ymin, ymax) in [
            ("tsy_lingxu_01_shallow", 40.0, 120.0),
            ("tsy_lingxu_01_mid", 0.0, 40.0),
            ("tsy_lingxu_01_deep", -40.0, 0.0),
        ] {
            zones
                .register_runtime_zone(Zone {
                    name: name.into(),
                    dimension: DimensionKind::Tsy,
                    bounds: (DVec3::new(0.0, ymin, 0.0), DVec3::new(100.0, ymax, 100.0)),
                    spirit_qi: -0.5,
                    danger_level: 5,
                    active_events: if name.ends_with("_shallow") {
                        vec!["tsy_entry".into()]
                    } else {
                        Vec::new()
                    },
                    patrol_anchors: vec![DVec3::new(50.0, (ymin + ymax) * 0.5, 50.0)],
                    blocked_tiles: Vec::new(),
                })
                .expect("register zone");
        }
    }

    fn make_app_with_loot_spawn() -> App {
        let mut app = App::new();
        app.insert_resource(CombatClock::default());
        app.insert_resource(ZoneRegistry::fallback());
        app.insert_resource(AncientRelicPool::from_seed());
        app.insert_resource(TsySpawnedFamilies::default());
        app.insert_resource(InventoryInstanceIdAllocator::default());
        app.insert_resource(DroppedLootRegistry::default());
        app.add_event::<TsyEnterEmit>();
        app.add_systems(Update, tsy_loot_spawn_on_enter);
        register_lingxu_subzones(&mut app);
        app
    }

    fn make_app_with_revive() -> App {
        let mut app = App::new();
        app.insert_resource(CombatClock::default());
        app.insert_resource(ItemRegistry::default());
        app.insert_resource(DroppedLootRegistry::default());
        app.add_event::<PlayerRevived>();
        app.add_event::<DroppedItemEvent>();
        app.add_systems(Update, apply_death_drop_on_revive);
        app
    }

    // ----------------------------------------------------------------------
    // §8.2 测试场景
    // ----------------------------------------------------------------------

    #[test]
    fn first_entry_spawns_relics_into_loot_registry() {
        let mut app = make_app_with_loot_spawn();
        let player = app.world_mut().spawn_empty().id();

        // 模拟玩家踏进 TSY family
        app.world_mut().send_event(TsyEnterEmit {
            player_entity: player,
            family_id: "tsy_lingxu_01".into(),
            return_to: DimensionAnchor {
                dimension: DimensionKind::Overworld,
                pos: DVec3::new(0.0, 64.0, 0.0),
            },
            filtered: Vec::new(),
        });

        app.update();

        let registry = app.world().resource::<DroppedLootRegistry>();
        let ancient_count = registry
            .entries
            .values()
            .filter(|e| e.item.rarity == ItemRarity::Ancient)
            .count();
        assert!(
            ancient_count > 0,
            "首次进入应至少 spawn 1 件 Ancient relic（实际 {ancient_count}）"
        );

        let spawned = app.world().resource::<TsySpawnedFamilies>();
        assert!(
            spawned.families.contains("tsy_lingxu_01"),
            "TsySpawnedFamilies 应记下已 spawn 的 family"
        );
    }

    #[test]
    fn second_entry_does_not_spawn_more_relics() {
        let mut app = make_app_with_loot_spawn();
        let player = app.world_mut().spawn_empty().id();

        for _ in 0..2 {
            app.world_mut().send_event(TsyEnterEmit {
                player_entity: player,
                family_id: "tsy_lingxu_01".into(),
                return_to: DimensionAnchor {
                    dimension: DimensionKind::Overworld,
                    pos: DVec3::new(0.0, 64.0, 0.0),
                },
                filtered: Vec::new(),
            });
            app.update();
        }

        let count_after_first = app.world().resource::<DroppedLootRegistry>().entries.len();

        // 再次入场（第三次）→ 总数不变
        app.world_mut().send_event(TsyEnterEmit {
            player_entity: player,
            family_id: "tsy_lingxu_01".into(),
            return_to: DimensionAnchor {
                dimension: DimensionKind::Overworld,
                pos: DVec3::new(0.0, 64.0, 0.0),
            },
            filtered: Vec::new(),
        });
        app.update();
        let count_after_third = app.world().resource::<DroppedLootRegistry>().entries.len();
        assert_eq!(
            count_after_first, count_after_third,
            "重复入场应 idempotent，不再 spawn 遗物"
        );
    }

    #[test]
    fn entry_without_zones_does_not_mark_family_spawned() {
        // Codex review #2 回归：family 的 mid/deep zone 还没注册时，第一次入场不
        // 应把 family 标 spawned，否则后续入场永远 skip → family 永久缺 relics。
        let mut app = App::new();
        app.insert_resource(CombatClock::default());
        app.insert_resource(ZoneRegistry::fallback());
        app.insert_resource(AncientRelicPool::from_seed());
        app.insert_resource(TsySpawnedFamilies::default());
        app.insert_resource(InventoryInstanceIdAllocator::default());
        app.insert_resource(DroppedLootRegistry::default());
        app.add_event::<TsyEnterEmit>();
        app.add_systems(Update, tsy_loot_spawn_on_enter);
        // 故意不 register_lingxu_subzones —— mid/deep zone 不存在

        let player = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(TsyEnterEmit {
            player_entity: player,
            family_id: "tsy_lingxu_01".into(),
            return_to: DimensionAnchor {
                dimension: DimensionKind::Overworld,
                pos: DVec3::new(0.0, 64.0, 0.0),
            },
            filtered: Vec::new(),
        });
        app.update();

        assert_eq!(
            app.world().resource::<DroppedLootRegistry>().entries.len(),
            0,
            "无 mid/deep zone 时 spawn 必为 0"
        );
        assert!(
            !app.world()
                .resource::<TsySpawnedFamilies>()
                .families
                .contains("tsy_lingxu_01"),
            "无 zone 不应标记 family spawned，留给下次入场重试"
        );

        // 后续 zone 上线 → 再次入场应能 spawn
        register_lingxu_subzones(&mut app);
        app.world_mut().send_event(TsyEnterEmit {
            player_entity: player,
            family_id: "tsy_lingxu_01".into(),
            return_to: DimensionAnchor {
                dimension: DimensionKind::Overworld,
                pos: DVec3::new(0.0, 64.0, 0.0),
            },
            filtered: Vec::new(),
        });
        app.update();
        assert!(
            app.world().resource::<DroppedLootRegistry>().entries.len() > 0,
            "zone 就绪后再入场应成功 spawn"
        );
        assert!(
            app.world()
                .resource::<TsySpawnedFamilies>()
                .families
                .contains("tsy_lingxu_01"),
            "成功 spawn 后才标记"
        );
    }

    #[test]
    fn ancient_relic_keeps_rarity_and_zero_spirit_quality() {
        let mut app = make_app_with_loot_spawn();
        let player = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(TsyEnterEmit {
            player_entity: player,
            family_id: "tsy_lingxu_01".into(),
            return_to: DimensionAnchor {
                dimension: DimensionKind::Overworld,
                pos: DVec3::new(0.0, 64.0, 0.0),
            },
            filtered: Vec::new(),
        });
        app.update();

        let registry = app.world().resource::<DroppedLootRegistry>();
        for entry in registry.entries.values() {
            if entry.item.rarity == ItemRarity::Ancient {
                assert_eq!(
                    entry.item.spirit_quality, 0.0,
                    "上古遗物应无灵：spirit_quality 必须 0"
                );
                assert_eq!(
                    entry.item.durability, 1.0,
                    "ancient durability 恒为 1.0（schema 0..=1）"
                );
                assert!(
                    matches!(entry.item.charges, Some(1) | Some(3) | Some(5)),
                    "ancient charges 必须 1/3/5（tier 映射），实际 {:?}",
                    entry.item.charges
                );
                assert!(
                    entry.item.template_id.starts_with("ancient_relic_"),
                    "template_id 命名约定 ancient_relic_*"
                );
            }
        }
    }

    #[test]
    fn main_world_death_does_not_spawn_corpse() {
        let mut app = make_app_with_revive();
        // 玩家无 TsyPresence
        let player = app
            .world_mut()
            .spawn((
                make_inventory(vec![item(1, "a"), item(2, "b"), item(3, "c"), item(4, "d")]),
                Position::new([0.0, 64.0, 0.0]),
            ))
            .id();
        app.world_mut().send_event(PlayerRevived { entity: player });
        app.update();

        let world = app.world_mut();
        let mut corpses = world.query::<&CorpseEmbalmed>();
        assert_eq!(
            corpses.iter(world).count(),
            0,
            "主世界死亡不应 spawn CorpseEmbalmed"
        );
    }

    #[test]
    fn tsy_death_routes_through_split_logic_and_spawns_corpse() {
        let mut app = make_app_with_revive();
        // 玩家有 TsyPresence + 4 件原带 + 2 件秘境所得
        let entry_items = vec![
            item(10, "entry_a"),
            item(11, "entry_b"),
            item(12, "entry_c"),
            item(13, "entry_d"),
        ];
        let acquired_items = vec![item(20, "tsy_a"), item(21, "tsy_b")];
        let mut all = entry_items.clone();
        all.extend(acquired_items);
        let player = app
            .world_mut()
            .spawn((
                make_inventory(all),
                Position::new([100.0, 64.0, 100.0]),
                TsyPresence {
                    family_id: "tsy_lingxu_01".into(),
                    entered_at_tick: 0,
                    entry_inventory_snapshot: vec![10, 11, 12, 13],
                    return_to: DimensionAnchor {
                        dimension: DimensionKind::Overworld,
                        pos: DVec3::new(0.0, 64.0, 0.0),
                    },
                },
            ))
            .id();

        app.world_mut().send_event(PlayerRevived { entity: player });
        app.update();

        // 应 spawn CorpseEmbalmed
        let world = app.world_mut();
        let mut corpses = world.query::<&CorpseEmbalmed>();
        let corpse_list: Vec<_> = corpses.iter(world).collect();
        assert_eq!(corpse_list.len(), 1, "TSY 死亡应 spawn 1 具干尸");
        assert_eq!(corpse_list[0].family_id, "tsy_lingxu_01");
        assert!(!corpse_list[0].activated_to_daoxiang);
        assert!(
            corpse_list[0].drops.len() >= 2,
            "干尸应至少包含 2 件秘境所得，实际 {}",
            corpse_list[0].drops.len()
        );

        // DroppedLootRegistry 应有秘境所得 + 部分原带
        let dropped = app.world().resource::<DroppedLootRegistry>();
        let acquired_dropped: Vec<_> = dropped
            .entries
            .values()
            .filter(|e| e.item.template_id == "test_tsy_a" || e.item.template_id == "test_tsy_b")
            .collect();
        assert_eq!(acquired_dropped.len(), 2, "2 件秘境所得 100% 掉落");

        // 4 件原带 → 50% Roll = 2 件掉
        let entry_dropped: Vec<_> = dropped
            .entries
            .values()
            .filter(|e| e.item.template_id.starts_with("test_entry_"))
            .collect();
        assert_eq!(entry_dropped.len(), 2, "4 件原带 50% Roll = 2 件掉");

        // 玩家身上保留剩 2 件原带 + 0 件秘境所得
        let inv = app.world().entity(player).get::<PlayerInventory>().unwrap();
        let remain_entry = inv
            .containers
            .iter()
            .flat_map(|c| c.items.iter())
            .filter(|p| p.instance.template_id.starts_with("test_entry_"))
            .count();
        let remain_acquired = inv
            .containers
            .iter()
            .flat_map(|c| c.items.iter())
            .filter(|p| p.instance.template_id.starts_with("test_tsy_"))
            .count();
        assert_eq!(remain_entry, 2);
        assert_eq!(remain_acquired, 0);

        // DroppedItemEvent 应被 emit
        let events = app.world().resource::<Events<DroppedItemEvent>>();
        let mut reader = events.get_reader();
        let count = reader.read(events).count();
        assert_eq!(count, 1, "应 emit 1 次 DroppedItemEvent");
    }

    #[test]
    fn empty_tsy_inventory_skips_corpse_spawn() {
        // 边界：玩家身上空 → 没东西掉，不 spawn 干尸
        let mut app = make_app_with_revive();
        let player = app
            .world_mut()
            .spawn((
                make_inventory(vec![]),
                Position::new([100.0, 64.0, 100.0]),
                TsyPresence {
                    family_id: "tsy_lingxu_01".into(),
                    entered_at_tick: 0,
                    entry_inventory_snapshot: Vec::new(),
                    return_to: DimensionAnchor {
                        dimension: DimensionKind::Overworld,
                        pos: DVec3::new(0.0, 64.0, 0.0),
                    },
                },
            ))
            .id();
        app.world_mut().send_event(PlayerRevived { entity: player });
        app.update();
        let world = app.world_mut();
        let mut corpses = world.query::<&CorpseEmbalmed>();
        assert_eq!(
            corpses.iter(world).count(),
            0,
            "空 inventory 不应 spawn 干尸"
        );
    }

    #[test]
    fn second_player_can_pick_up_first_players_drops() {
        // 简化版"跨玩家 loot 拾取"：玩家 A 死了 → loot 进 registry → 用
        // pickup_dropped_loot_instance 模拟玩家 B 拾取（位置 < 2.5）
        let mut app = make_app_with_revive();
        let player_a = app
            .world_mut()
            .spawn((
                make_inventory(vec![item(1, "a"), item(2, "b")]),
                Position::new([0.0, 64.0, 0.0]),
                TsyPresence {
                    family_id: "tsy_lingxu_01".into(),
                    entered_at_tick: 0,
                    entry_inventory_snapshot: Vec::new(),
                    return_to: DimensionAnchor {
                        dimension: DimensionKind::Overworld,
                        pos: DVec3::new(0.0, 64.0, 0.0),
                    },
                },
            ))
            .id();
        app.world_mut()
            .send_event(PlayerRevived { entity: player_a });
        app.update();

        // 玩家 B：单独 inventory + 位置贴近死亡点
        let mut inv_b = make_inventory(vec![]);
        let mut registry = app.world_mut().resource_mut::<DroppedLootRegistry>();
        let drop_id = *registry
            .entries
            .keys()
            .next()
            .expect("registry 至少有一件 loot");
        let _ = crate::inventory::pickup_dropped_loot_instance(
            &mut inv_b,
            &mut registry,
            [0.5, 64.0, 0.5],
            drop_id,
        )
        .expect("玩家 B 拾取应成功");

        let total_in_b: usize = inv_b.containers.iter().map(|c| c.items.len()).sum();
        assert_eq!(total_in_b, 1, "玩家 B 背包应有 1 件 A 的 loot");
        assert!(
            !registry.entries.contains_key(&drop_id),
            "拾取后 registry 应移除该条目"
        );
    }
}
