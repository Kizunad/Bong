//! plan-tsy-lifecycle-v1 §7.2 — TSY 生命周期端到端集成测试。
//!
//! crate 是 bin-only（无 `tests/` 目录），与 `tsy_integration_test.rs` /
//! `tsy_loot_integration_test.rs` 同模式：把"几个 system 串起来跑一个 App"
//! 的集成测放在 src 子模块。
//!
//! 覆盖场景：
//! - happy path：Active → 骨架取一半 → Declining → 全取走 → Collapsing → 30 秒 → Dead
//! - cleanup：Dead 时 zone 移除 + 玩家弹回
//! - 干尸自然转化（6000 tick 后 spawn 道伥）
//! - 塌缩加速干尸激活

#[cfg(test)]
mod tests {
    use valence::prelude::{
        App, DVec3, EntityLayerId, Events, Position, Update, VisibleChunkLayer, VisibleEntityLayers,
    };

    use crate::combat::events::DeathEvent;
    use crate::combat::CombatClock;
    use crate::inventory::ancient_relics::AncientRelicSource;
    use crate::inventory::corpse::CorpseEmbalmed;
    use crate::inventory::{DroppedLootEntry, DroppedLootRegistry, ItemInstance, ItemRarity};
    use crate::world::dimension::{
        CurrentDimension, DimensionKind, DimensionLayers, OverworldLayer, TsyLayer,
    };
    use valence::prelude::IntoSystemConfigs;

    use crate::world::dimension_transfer::{apply_dimension_transfers, DimensionTransferRequest};
    use crate::world::tsy::{DimensionAnchor, TsyPresence};
    use crate::world::tsy_lifecycle::{
        tsy_collapse_completed_cleanup, tsy_corpse_to_daoxiang_tick, tsy_lifecycle_apply_spirit_qi,
        tsy_lifecycle_tick, TsyCollapseCompleted, TsyCollapseStarted, TsyLifecycle,
        TsyZoneStateRegistry, COLLAPSE_DURATION_TICKS, DAOXIANG_NATURAL_TICKS,
    };
    use crate::world::zone::{Zone, ZoneRegistry};

    fn make_tsy_zone(name: &str) -> Zone {
        Zone {
            name: name.to_string(),
            dimension: DimensionKind::Tsy,
            bounds: (DVec3::new(0.0, 0.0, 0.0), DVec3::new(100.0, 100.0, 100.0)),
            spirit_qi: -0.3,
            danger_level: 5,
            active_events: Vec::new(),
            patrol_anchors: vec![DVec3::new(50.0, 50.0, 50.0)],
            blocked_tiles: Vec::new(),
        }
    }

    fn ancient_item(id: u64) -> ItemInstance {
        ItemInstance {
            instance_id: id,
            template_id: format!("ancient_relic_test_{id}"),
            display_name: format!("relic_{id}"),
            grid_w: 1,
            grid_h: 1,
            weight: 0.5,
            rarity: ItemRarity::Ancient,
            description: "test ancient".into(),
            stack_count: 1,
            spirit_quality: 0.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: Some(3),
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
        }
    }

    fn drop_entry(id: u64, family: &str, pos: DVec3) -> DroppedLootEntry {
        DroppedLootEntry {
            instance_id: id,
            source_container_id: format!("tsy_spawn:{family}"),
            source_row: 0,
            source_col: 0,
            world_pos: [pos.x, pos.y, pos.z],
            item: ancient_item(id),
        }
    }

    /// 构建一个最小测试 App：lifecycle 三个 tick + cleanup + dim transfer。
    fn make_app(initial_tick: u64) -> App {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: initial_tick });
        app.insert_resource(ZoneRegistry::fallback());
        app.insert_resource(DroppedLootRegistry::default());
        app.insert_resource(TsyZoneStateRegistry::default());

        let overworld = app.world_mut().spawn(OverworldLayer).id();
        let tsy = app.world_mut().spawn(TsyLayer).id();
        app.insert_resource(DimensionLayers { overworld, tsy });

        app.add_event::<DimensionTransferRequest>();
        app.add_event::<DeathEvent>();
        app.add_event::<TsyCollapseStarted>();
        app.add_event::<TsyCollapseCompleted>();
        app.add_systems(
            Update,
            (
                tsy_lifecycle_tick,
                crate::world::extract_system::on_tsy_collapse_completed.after(tsy_lifecycle_tick),
                tsy_lifecycle_apply_spirit_qi.after(tsy_lifecycle_tick),
                tsy_corpse_to_daoxiang_tick.after(tsy_lifecycle_apply_spirit_qi),
                tsy_collapse_completed_cleanup
                    .after(tsy_lifecycle_tick)
                    .after(crate::world::extract_system::on_tsy_collapse_completed),
                apply_dimension_transfers.after(tsy_collapse_completed_cleanup),
            ),
        );
        app
    }

    fn register_lingxu(app: &mut App) {
        let mut zones = app.world_mut().resource_mut::<ZoneRegistry>();
        for name in [
            "tsy_lingxu_01_shallow",
            "tsy_lingxu_01_mid",
            "tsy_lingxu_01_deep",
        ] {
            zones
                .register_runtime_zone(make_tsy_zone(name))
                .expect("register");
        }
    }

    #[test]
    fn happy_path_active_to_dead_cycles_state_machine() {
        let mut app = make_app(0);
        register_lingxu(&mut app);

        // 注册 family + 4 件骨架 → Active 全骨架 ratio=1.0
        {
            let mut reg = app.world_mut().resource_mut::<TsyZoneStateRegistry>();
            reg.ensure_active(
                "tsy_lingxu_01",
                AncientRelicSource::DaoLord,
                DimensionAnchor {
                    dimension: DimensionKind::Overworld,
                    pos: DVec3::new(0.0, 64.0, 0.0),
                },
                0,
            );
            reg.mark_initial_skeleton("tsy_lingxu_01", vec![1, 2, 3, 4]);
        }
        // 4 件 entries
        {
            let mut loot = app.world_mut().resource_mut::<DroppedLootRegistry>();
            for id in [1u64, 2, 3, 4] {
                loot.entries.insert(
                    id,
                    drop_entry(id, "tsy_lingxu_01", DVec3::new(50.0, 50.0, 50.0)),
                );
            }
        }

        // Tick 1：还在 Active（4/4 = 100%）
        app.update();
        {
            let reg = app.world().resource::<TsyZoneStateRegistry>();
            let s = reg.by_family.get("tsy_lingxu_01").unwrap();
            assert_eq!(s.lifecycle, TsyLifecycle::Active);
        }

        // 取走 1, 2 → 2/4 = 50%（< 50% 才 Declining；本步刚好 = 50% 仍 Active）
        {
            let mut loot = app.world_mut().resource_mut::<DroppedLootRegistry>();
            loot.entries.remove(&1);
            loot.entries.remove(&2);
        }
        app.update();
        {
            let reg = app.world().resource::<TsyZoneStateRegistry>();
            let s = reg.by_family.get("tsy_lingxu_01").unwrap();
            assert_eq!(
                s.lifecycle,
                TsyLifecycle::Active,
                "刚到 50% 不算 Declining；要严格 < 50% 才推进"
            );
        }

        // 再取走一件（剩 1/4 = 25% < 50%）→ Declining
        {
            let mut loot = app.world_mut().resource_mut::<DroppedLootRegistry>();
            loot.entries.remove(&3);
        }
        app.update();
        {
            let reg = app.world().resource::<TsyZoneStateRegistry>();
            let s = reg.by_family.get("tsy_lingxu_01").unwrap();
            assert_eq!(s.lifecycle, TsyLifecycle::Declining);
        }
        // Declining 阶段 spirit_qi 应被加深（base shallow=-0.3，ratio=0.25 → -0.3 + (-0.3)*0.75 = -0.525）
        {
            let zones = app.world().resource::<ZoneRegistry>();
            let z = zones
                .find_zone_by_name("tsy_lingxu_01_shallow")
                .expect("shallow zone");
            assert!(
                (z.spirit_qi - (-0.525)).abs() < 1e-6,
                "Declining shallow expected -0.525, got {}",
                z.spirit_qi
            );
        }

        // 最后一件 → Collapsing + 倒计时
        {
            let mut loot = app.world_mut().resource_mut::<DroppedLootRegistry>();
            loot.entries.remove(&4);
        }
        let collapse_started_tick = {
            app.world_mut().resource_mut::<CombatClock>().tick = 100;
            app.update();
            let reg = app.world().resource::<TsyZoneStateRegistry>();
            let s = reg.by_family.get("tsy_lingxu_01").unwrap();
            assert_eq!(s.lifecycle, TsyLifecycle::Collapsing);
            s.collapsing_started_at_tick.expect("set on Collapsing")
        };

        // TsyCollapseStarted 已 emit
        let started_events: Vec<_> = app
            .world()
            .resource::<Events<TsyCollapseStarted>>()
            .get_reader()
            .read(app.world().resource::<Events<TsyCollapseStarted>>())
            .cloned()
            .collect();
        assert_eq!(started_events.len(), 1);
        assert_eq!(started_events[0].family_id, "tsy_lingxu_01");

        // Collapsing 阶段 spirit_qi 翻倍（ratio=0 → -0.6 ×2 = -1.2 → clamp -1.0）
        {
            let zones = app.world().resource::<ZoneRegistry>();
            let z = zones
                .find_zone_by_name("tsy_lingxu_01_shallow")
                .expect("shallow zone");
            assert!((z.spirit_qi - (-1.0)).abs() < 1e-6);
        }

        // 推 30 秒 → Dead + cleanup
        app.world_mut().resource_mut::<CombatClock>().tick =
            collapse_started_tick + COLLAPSE_DURATION_TICKS + 1;
        app.update();
        // cleanup 是 Update 阶段 system；下一 update 才被消费 event 后发生 cleanup。
        // 已经在同 tick：tsy_lifecycle_tick 转 Dead → emit Completed → cleanup 同 update。
        {
            let reg = app.world().resource::<TsyZoneStateRegistry>();
            let s = reg.by_family.get("tsy_lingxu_01").unwrap();
            assert_eq!(s.lifecycle, TsyLifecycle::Dead);
        }
        // 三层 subzone 应被移除
        let zones = app.world().resource::<ZoneRegistry>();
        for suffix in ["_shallow", "_mid", "_deep"] {
            assert!(
                zones
                    .find_zone_by_name(&format!("tsy_lingxu_01{suffix}"))
                    .is_none(),
                "Dead 后 {suffix} subzone 应被移除"
            );
        }
    }

    #[test]
    fn collapse_completed_kills_player_in_tsy() {
        let mut app = make_app(0);
        register_lingxu(&mut app);

        // 玩家：在 TSY 里持有 TsyPresence
        let return_anchor = DimensionAnchor {
            dimension: DimensionKind::Overworld,
            pos: DVec3::new(7.0, 70.0, 9.0),
        };
        let layers = *app.world().resource::<DimensionLayers>();
        let mut visible = VisibleEntityLayers::default();
        visible.0.insert(layers.tsy);
        let player = app
            .world_mut()
            .spawn((
                EntityLayerId(layers.tsy),
                VisibleChunkLayer(layers.tsy),
                visible,
                Position::new([50.0, 50.0, 50.0]),
                CurrentDimension(DimensionKind::Tsy),
                TsyPresence {
                    family_id: "tsy_lingxu_01".into(),
                    entered_at_tick: 0,
                    entry_inventory_snapshot: Vec::new(),
                    return_to: return_anchor,
                },
            ))
            .id();

        // 注册 family，直接 force complete event
        {
            let mut reg = app.world_mut().resource_mut::<TsyZoneStateRegistry>();
            reg.ensure_active(
                "tsy_lingxu_01",
                AncientRelicSource::DaoLord,
                return_anchor,
                0,
            );
            reg.by_family.get_mut("tsy_lingxu_01").unwrap().lifecycle = TsyLifecycle::Collapsing;
        }
        app.world_mut()
            .resource_mut::<Events<TsyCollapseCompleted>>()
            .send(TsyCollapseCompleted {
                family_id: "tsy_lingxu_01".into(),
                at_tick: 1,
            });

        app.update();

        // TsyPresence 留给 P1 死亡掉落路径读取，P5 handler 发 DeathEvent 化灰。
        assert!(
            app.world().entity(player).get::<TsyPresence>().is_some(),
            "Collapse death drop path still needs TsyPresence"
        );
        let deaths = app.world().resource::<Events<DeathEvent>>();
        let collected: Vec<_> = deaths.get_reader().read(deaths).cloned().collect();
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].target, player);
        assert_eq!(collected[0].cause, "tsy_collapsed");
        let cur = app
            .world()
            .entity(player)
            .get::<CurrentDimension>()
            .expect("CurrentDimension");
        assert_eq!(cur.0, DimensionKind::Tsy);
    }

    #[test]
    fn collapse_cleanup_evaporates_loot_inside_aabbs_only() {
        let mut app = make_app(0);
        register_lingxu(&mut app);

        // 1 件 loot 在 zone 内（pos 命中 mid 层 AABB）+ 1 件在 zone 外
        {
            let mut loot = app.world_mut().resource_mut::<DroppedLootRegistry>();
            loot.entries.insert(
                100,
                drop_entry(100, "tsy_lingxu_01", DVec3::new(50.0, 50.0, 50.0)),
            );
            loot.entries.insert(
                999,
                drop_entry(999, "tsy_lingxu_01", DVec3::new(500.0, 500.0, 500.0)),
            );
        }
        {
            let mut reg = app.world_mut().resource_mut::<TsyZoneStateRegistry>();
            reg.ensure_active(
                "tsy_lingxu_01",
                AncientRelicSource::DaoLord,
                DimensionAnchor {
                    dimension: DimensionKind::Overworld,
                    pos: DVec3::ZERO,
                },
                0,
            );
            reg.by_family.get_mut("tsy_lingxu_01").unwrap().lifecycle = TsyLifecycle::Collapsing;
        }
        app.world_mut()
            .resource_mut::<Events<TsyCollapseCompleted>>()
            .send(TsyCollapseCompleted {
                family_id: "tsy_lingxu_01".into(),
                at_tick: 1,
            });
        app.update();

        let loot = app.world().resource::<DroppedLootRegistry>();
        assert!(!loot.entries.contains_key(&100), "zone 内 loot 应被蒸发");
        assert!(loot.entries.contains_key(&999), "zone 外 loot 不应受影响");
    }

    /// Regression test for Codex review P1 #1：cleanup 不应删除主世界同 XYZ 的 entries。
    /// 主世界的 drop（player container source_container_id）即使坐标命中 TSY zone AABB
    /// 也必须保留 —— 仅 `tsy_spawn:` / `tsy_corpse:{family}/...` 前缀的 entries 才参与蒸发。
    #[test]
    fn collapse_cleanup_preserves_overworld_drops_at_same_xyz() {
        let mut app = make_app(0);
        register_lingxu(&mut app);

        // entry 100：主世界玩家死亡掉的"main_pack"物品，坐标 (50,50,50) 落在 TSY mid AABB 内
        {
            let mut loot = app.world_mut().resource_mut::<DroppedLootRegistry>();
            let mut overworld_drop = drop_entry(100, "tsy_lingxu_01", DVec3::new(50.0, 50.0, 50.0));
            overworld_drop.source_container_id = "main_pack".into();
            loot.entries.insert(100, overworld_drop);

            // entry 200：合法的 TSY family 残留 ancient relic（应被蒸发）
            let mut tsy_drop = drop_entry(200, "tsy_lingxu_01", DVec3::new(50.0, 50.0, 50.0));
            tsy_drop.source_container_id = "tsy_spawn:tsy_lingxu_01".into();
            loot.entries.insert(200, tsy_drop);
        }
        {
            let mut reg = app.world_mut().resource_mut::<TsyZoneStateRegistry>();
            reg.ensure_active(
                "tsy_lingxu_01",
                AncientRelicSource::DaoLord,
                DimensionAnchor {
                    dimension: DimensionKind::Overworld,
                    pos: DVec3::ZERO,
                },
                0,
            );
            reg.by_family.get_mut("tsy_lingxu_01").unwrap().lifecycle = TsyLifecycle::Collapsing;
        }
        app.world_mut()
            .resource_mut::<Events<TsyCollapseCompleted>>()
            .send(TsyCollapseCompleted {
                family_id: "tsy_lingxu_01".into(),
                at_tick: 1,
            });
        app.update();

        let loot = app.world().resource::<DroppedLootRegistry>();
        assert!(
            loot.entries.contains_key(&100),
            "主世界 main_pack drop 在同 XYZ 不应被误删（Codex P1 #1 regression）"
        );
        assert!(
            !loot.entries.contains_key(&200),
            "tsy_spawn 前缀 entry 在 zone aabb 内应被蒸发"
        );
    }

    /// Regression test for Codex review P1 #2：玩家捡走 ancient relic 后再 discard
    /// 回主世界，instance_id 仍在 registry 但 source 不再是 tsy_spawn，状态机应正确
    /// 视其为"已取走"而非"还在 zone 里"，能推进 Collapsing。
    #[test]
    fn lifecycle_tick_excludes_discarded_relics_from_remaining_count() {
        let mut app = make_app(0);
        register_lingxu(&mut app);

        // family + 2 件 ancient relic
        {
            let mut reg = app.world_mut().resource_mut::<TsyZoneStateRegistry>();
            reg.ensure_active(
                "tsy_lingxu_01",
                AncientRelicSource::DaoLord,
                DimensionAnchor {
                    dimension: DimensionKind::Overworld,
                    pos: DVec3::ZERO,
                },
                0,
            );
            reg.mark_initial_skeleton("tsy_lingxu_01", vec![10, 20]);
        }

        // entry 10：玩家捡了又 discard 回主世界，source 改成 "main_pack"
        // entry 20：捡了，instance 离开 registry（彻底取走）
        // 期望：remaining = 0 → 推进到 Collapsing
        {
            let mut loot = app.world_mut().resource_mut::<DroppedLootRegistry>();
            let mut discarded = drop_entry(10, "tsy_lingxu_01", DVec3::new(0.0, 64.0, 0.0));
            discarded.source_container_id = "main_pack".into();
            loot.entries.insert(10, discarded);
            // entry 20 不在 registry —— 已被永远捡走
        }

        app.update();

        let reg = app.world().resource::<TsyZoneStateRegistry>();
        let s = reg.by_family.get("tsy_lingxu_01").unwrap();
        assert_eq!(
            s.lifecycle,
            TsyLifecycle::Collapsing,
            "discarded relic 应不算 remaining，状态机应推进到 Collapsing（Codex P1 #2 regression）"
        );
    }

    #[test]
    fn corpse_natural_activation_after_threshold_spawns_daoxiang() {
        let mut app = make_app(DAOXIANG_NATURAL_TICKS + 1);
        register_lingxu(&mut app);

        let layers = *app.world().resource::<DimensionLayers>();

        // 注册 family Active（保证 corpse_to_daoxiang 不被 Collapsing skip）
        {
            let mut reg = app.world_mut().resource_mut::<TsyZoneStateRegistry>();
            reg.ensure_active(
                "tsy_lingxu_01",
                AncientRelicSource::DaoLord,
                DimensionAnchor {
                    dimension: DimensionKind::Overworld,
                    pos: DVec3::ZERO,
                },
                0,
            );
        }

        let corpse_entity = app
            .world_mut()
            .spawn((
                Position::new([50.0, 50.0, 50.0]),
                EntityLayerId(layers.tsy),
                CorpseEmbalmed {
                    family_id: "tsy_lingxu_01".into(),
                    died_at_tick: 0,
                    death_cause: "tsy_drain".into(),
                    drops: vec![10, 11],
                    activated_to_daoxiang: false,
                },
            ))
            .id();

        app.update();

        // Corpse entity 应被 Despawned 标记
        assert!(
            app.world()
                .entity(corpse_entity)
                .get::<valence::prelude::Despawned>()
                .is_some(),
            "corpse entity 应在自然激活后被 despawn"
        );
        // 至少 spawn 1 个 Daoxiang archetype NPC
        let mut count = 0;
        let world = app.world_mut();
        let mut q = world.query::<&crate::npc::lifecycle::NpcArchetype>();
        for arch in q.iter(world) {
            if matches!(arch, crate::npc::lifecycle::NpcArchetype::Daoxiang) {
                count += 1;
            }
        }
        assert_eq!(count, 1, "应 spawn 1 个 Daoxiang NPC");
    }

    #[test]
    fn corpse_below_threshold_does_not_spawn() {
        let mut app = make_app(100);
        register_lingxu(&mut app);

        let layers = *app.world().resource::<DimensionLayers>();
        {
            let mut reg = app.world_mut().resource_mut::<TsyZoneStateRegistry>();
            reg.ensure_active(
                "tsy_lingxu_01",
                AncientRelicSource::DaoLord,
                DimensionAnchor {
                    dimension: DimensionKind::Overworld,
                    pos: DVec3::ZERO,
                },
                0,
            );
        }
        let corpse = app
            .world_mut()
            .spawn((
                Position::new([50.0, 50.0, 50.0]),
                EntityLayerId(layers.tsy),
                CorpseEmbalmed {
                    family_id: "tsy_lingxu_01".into(),
                    died_at_tick: 0,
                    death_cause: "tsy_drain".into(),
                    drops: vec![10],
                    activated_to_daoxiang: false,
                },
            ))
            .id();
        app.update();

        // 100 tick < DAOXIANG_NATURAL_TICKS = 6000 → 不激活
        assert!(
            app.world()
                .entity(corpse)
                .get::<valence::prelude::Despawned>()
                .is_none(),
            "未到自然阈值不应 despawn corpse"
        );
        let world = app.world_mut();
        let mut q = world.query::<&crate::npc::lifecycle::NpcArchetype>();
        let any_daoxiang = q
            .iter(world)
            .any(|a| matches!(a, crate::npc::lifecycle::NpcArchetype::Daoxiang));
        assert!(!any_daoxiang, "未达阈值不应 spawn Daoxiang");
    }

    #[test]
    fn collapse_accelerates_corpse_into_daoxiang_immediately() {
        // 干尸刚 100 tick（远小于 6000），但 family 进 Collapsing → cleanup 立刻激活
        let mut app = make_app(100);
        register_lingxu(&mut app);

        let layers = *app.world().resource::<DimensionLayers>();
        {
            let mut reg = app.world_mut().resource_mut::<TsyZoneStateRegistry>();
            reg.ensure_active(
                "tsy_lingxu_01",
                AncientRelicSource::DaoLord,
                DimensionAnchor {
                    dimension: DimensionKind::Overworld,
                    pos: DVec3::ZERO,
                },
                0,
            );
            reg.by_family.get_mut("tsy_lingxu_01").unwrap().lifecycle = TsyLifecycle::Collapsing;
        }
        let corpse = app
            .world_mut()
            .spawn((
                Position::new([50.0, 50.0, 50.0]),
                EntityLayerId(layers.tsy),
                CorpseEmbalmed {
                    family_id: "tsy_lingxu_01".into(),
                    died_at_tick: 0,
                    death_cause: "tsy_drain".into(),
                    drops: vec![10],
                    activated_to_daoxiang: false,
                },
            ))
            .id();

        // 直接发 CompleteEvent（绕过 30 秒计时）
        app.world_mut()
            .resource_mut::<Events<TsyCollapseCompleted>>()
            .send(TsyCollapseCompleted {
                family_id: "tsy_lingxu_01".into(),
                at_tick: 100,
            });
        app.update();

        // Corpse 应被 despawn（cleanup 路径加速激活）
        assert!(
            app.world()
                .entity(corpse)
                .get::<valence::prelude::Despawned>()
                .is_some(),
            "塌缩 cleanup 必须立刻 despawn corpse 不等 6000 tick"
        );
    }
}
