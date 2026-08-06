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
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use valence::prelude::{
        App, Biome, BiomeRegistry, DVec3, EntityLayerId, Events, Ident, IntoSystemConfigs,
        Position, Startup, Update, VisibleChunkLayer, VisibleEntityLayers,
    };

    use crate::combat::events::DeathEvent;
    use crate::combat::CombatClock;
    use crate::cultivation::components::Cultivation;
    use crate::inventory::{InventoryRevision, ItemInstance, ItemRarity, PlayerInventory};
    use crate::qi_physics::WorldQiAccount;
    use crate::world::dimension::{
        CurrentDimension, DimensionKind, DimensionLayers, OverworldLayer, TsyLayer,
    };
    use crate::world::dimension_transfer::{
        apply_dimension_transfers, DimensionTransferRequest, DimensionTransferSet,
    };
    use crate::world::terrain::{TerrainProvider, TerrainProviders};
    use crate::world::tsy::{DimensionAnchor, PortalDirection, RiftKind, RiftPortal, TsyPresence};
    use crate::world::tsy_drain::tsy_drain_tick;
    use crate::world::tsy_poi_consumer::spawn_rift_portals;
    use crate::world::tsy_portal::{
        tsy_entry_portal_tick, tsy_exit_portal_tick, TsyEnterEmit, TsyExitEmit,
    };
    use crate::world::zone::{Zone, ZoneRegistry};

    fn fresh_app() -> App {
        let mut app = App::new();
        app.insert_resource(CombatClock::default());
        app.insert_resource(WorldQiAccount::default());
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
                    qi_equilibrium: 0.0,
                    qi_inflow_per_min: 0.0,
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
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        });
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(1),
            containers: Vec::new(),
            equipped: HashMap::new(),
            hotbar: hb,
            bone_coins: 0,
            max_weight: 100.0,
        }
    }

    fn empty_player_inventory() -> PlayerInventory {
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(1),
            containers: Vec::new(),
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 100.0,
        }
    }

    struct NorthRiftManifestFixture {
        root: PathBuf,
    }

    impl NorthRiftManifestFixture {
        fn from_default_blueprint() -> Self {
            let blueprint_path =
                Path::new(env!("CARGO_MANIFEST_DIR")).join("zones.worldview.example.json");
            let blueprint: serde_json::Value = serde_json::from_str(
                &fs::read_to_string(&blueprint_path).expect("default blueprint should be readable"),
            )
            .expect("default blueprint should be valid JSON");
            let zones = blueprint["zones"]
                .as_array()
                .expect("default blueprint must contain zones array");
            let north_rift = zones
                .iter()
                .find(|zone| zone["name"] == "rift_mouth_north_002")
                .expect("default blueprint must contain north rift");
            let all_pois = north_rift["pois"]
                .as_array()
                .expect("north rift must contain POIs");
            let portals = all_pois
                .iter()
                .filter(|poi| poi["kind"] == "rift_portal" && poi["name"] == "塌缩裂缝·北荒东陲")
                .cloned()
                .collect::<Vec<_>>();
            assert_eq!(
                portals.len(),
                1,
                "default blueprint must expose exactly one north rift portal by kind/name"
            );
            assert_eq!(
                portals[0]["pos_xyz"],
                serde_json::json!([2000.0, 74.0, -7300.0])
            );
            assert_ne!(
                portals[0]["pos_xyz"],
                serde_json::json!([2000.0, 74.0, -7800.0])
            );
            let manifest_pois = all_pois
                .iter()
                .cloned()
                .map(|mut poi| {
                    poi.as_object_mut()
                        .expect("blueprint POI must be a JSON object")
                        .insert(
                            "zone".to_string(),
                            serde_json::Value::String("rift_mouth_north_002".to_string()),
                        );
                    poi
                })
                .collect::<Vec<_>>();

            let tsy_blueprint_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("zones.tsy.json");
            let tsy_blueprint: serde_json::Value = serde_json::from_str(
                &fs::read_to_string(&tsy_blueprint_path)
                    .expect("default TSY blueprint should be readable"),
            )
            .expect("default TSY blueprint should be valid JSON");
            let tsy_manifest_pois =
                tsy_blueprint["zones"]
                    .as_array()
                    .expect("default TSY blueprint must contain zones array")
                    .iter()
                    .flat_map(|zone| {
                        let zone_name = zone["name"]
                            .as_str()
                            .expect("TSY blueprint zone must have a name")
                            .to_string();
                        zone["pois"].as_array().into_iter().flatten().cloned().map(
                            move |mut poi| {
                                let object = poi
                                    .as_object_mut()
                                    .expect("TSY blueprint POI must be a JSON object");
                                object.insert(
                                    "zone".to_string(),
                                    serde_json::Value::String(zone_name.clone()),
                                );
                                for (key, default) in [
                                    ("name", serde_json::Value::String(String::new())),
                                    ("tags", serde_json::Value::Array(Vec::new())),
                                    ("unlock", serde_json::Value::String(String::new())),
                                    ("qi_affinity", serde_json::json!(0.0)),
                                    ("danger_bias", serde_json::json!(0)),
                                ] {
                                    object.entry(key.to_string()).or_insert(default);
                                }
                                poi
                            },
                        )
                    })
                    .collect::<Vec<_>>();

            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock after epoch")
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "bong-north-rift-portal-loader-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&root).expect("create north rift manifest fixture dir");
            let manifest = serde_json::json!({
                "version": 2,
                "tile_size": 1,
                "world_bounds": {
                    "min_x": 1850,
                    "max_x": 2150,
                    "min_z": -7450,
                    "max_z": -7150
                },
                "surface_palette": ["stone"],
                "biome_palette": ["plains"],
                "tiles": [],
                "pois": manifest_pois
            });
            fs::write(
                root.join("manifest.json"),
                serde_json::to_vec_pretty(&manifest).expect("manifest fixture should serialize"),
            )
            .expect("write north rift manifest fixture");
            let tsy_manifest = serde_json::json!({
                "version": 2,
                "tile_size": 1,
                "world_bounds": {
                    "min_x": -2000,
                    "max_x": 2000,
                    "min_z": -2000,
                    "max_z": 2000
                },
                "surface_palette": ["stone"],
                "biome_palette": ["plains"],
                "tiles": [],
                "pois": tsy_manifest_pois
            });
            fs::write(
                root.join("manifest.tsy.json"),
                serde_json::to_vec_pretty(&tsy_manifest)
                    .expect("TSY manifest fixture should serialize"),
            )
            .expect("write TSY manifest fixture");
            Self { root }
        }

        fn manifest_path(&self) -> PathBuf {
            self.root.join("manifest.json")
        }

        fn tsy_manifest_path(&self) -> PathBuf {
            self.root.join("manifest.tsy.json")
        }
    }

    impl Drop for NorthRiftManifestFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn make_cultivation(qi_current: f64, qi_max: f64) -> Cultivation {
        Cultivation {
            qi_current,
            qi_max,
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
            RiftPortal::entry(
                "tsy_lingxu_01".to_string(),
                DimensionAnchor {
                    dimension: DimensionKind::Tsy,
                    pos: shallow_center,
                },
                1.5,
            ),
        ));
        let player = app
            .world_mut()
            .spawn((
                Position::new([0.5, 64.0, 0.0]),
                make_player_inventory_with_qi_item(),
                make_cultivation(50.0, 50.0),
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
    /// 玩家持 TsyPresence + 在 TSY shallow zone 内 + Tsy dim → qi_current 按 §2.1 公式衰减。
    #[test]
    fn b_drain_after_entry_decreases_spirit_qi_per_tick() {
        let mut app = fresh_app();
        let shallow_center = register_lingxu_subzones(&mut app);

        // 玩家直接 spawn 在 TSY 内（跳过 entry，专注 drain 验证）
        let player = app
            .world_mut()
            .spawn((
                Position::new([shallow_center.x, shallow_center.y, shallow_center.z]),
                make_cultivation(50.0, 50.0),
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
            .get::<Cultivation>()
            .unwrap()
            .qi_current;

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

        // qi_current 故意调小 + 池子大让 drain rate 高 → 1 tick 即可归零
        let _player = app
            .world_mut()
            .spawn((
                Position::new([shallow_center.x, 80.0, shallow_center.z]),
                Cultivation {
                    qi_current: 0.001,
                    qi_max: 500.0, // 化虚，rate 巨大
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
            RiftPortal::exit(
                "tsy_lingxu_01".to_string(),
                DimensionAnchor {
                    dimension: DimensionKind::Overworld,
                    pos: return_to.pos,
                },
                1.5,
                crate::world::tsy::RiftKind::MainRift,
            ),
        ));

        let player = app
            .world_mut()
            .spawn((
                Position::new([exit_pos.x + 0.5, exit_pos.y, exit_pos.z]),
                make_cultivation(50.0, 50.0),
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
        // （drain 把 qi_current 减一点，之后 exit 触发 + 移除 TsyPresence）
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

    #[test]
    fn e_relocated_north_rift_manifest_spawns_and_transfers_to_target() {
        let fixture = NorthRiftManifestFixture::from_default_blueprint();
        let mut biomes = BiomeRegistry::default();
        biomes.insert(
            Ident::new("plains").expect("valid plains biome ident"),
            Biome::default(),
        );
        let provider = TerrainProvider::load(&fixture.manifest_path(), &fixture.root, &biomes)
            .expect("production TerrainProvider should load north rift manifest fixture");
        let loaded_portals = provider
            .pois()
            .iter()
            .filter(|poi| {
                poi.zone == "rift_mouth_north_002"
                    && poi.kind == "rift_portal"
                    && poi.name == "塌缩裂缝·北荒东陲"
            })
            .collect::<Vec<_>>();
        assert_eq!(loaded_portals.len(), 1);
        assert_eq!(loaded_portals[0].pos_xyz, [2000.0, 74.0, -7300.0]);
        assert!(provider
            .pois()
            .iter()
            .filter(|poi| poi.kind == "rift_portal")
            .all(|poi| poi.pos_xyz != [2000.0, 74.0, -7800.0]));
        let tsy_provider =
            TerrainProvider::load(&fixture.tsy_manifest_path(), &fixture.root, &biomes)
                .expect("production TerrainProvider should load zones.tsy.json fixture");
        let loaded_exit_portals = tsy_provider
            .pois()
            .iter()
            .filter(|poi| {
                poi.zone == "tsy_zongmen_01_shallow"
                    && poi.kind == "rift_portal"
                    && poi.tags.iter().any(|tag| tag == "family_id:zongmen_01")
                    && poi.tags.iter().any(|tag| tag == "direction:exit")
            })
            .collect::<Vec<_>>();
        assert_eq!(
            loaded_exit_portals.len(),
            1,
            "zones.tsy.json must expose exactly one zongmen_01 exit portal"
        );
        assert_eq!(loaded_exit_portals[0].pos_xyz, [250.0, 100.0, 250.0]);

        let mut app = App::new();
        let overworld = app.world_mut().spawn(OverworldLayer).id();
        let tsy = app.world_mut().spawn(TsyLayer).id();
        app.insert_resource(DimensionLayers { overworld, tsy });
        app.insert_resource(TerrainProviders {
            overworld: provider,
            tsy: Some(tsy_provider),
        });
        app.insert_resource(CombatClock::default());
        app.add_event::<DimensionTransferRequest>();
        app.add_event::<TsyEnterEmit>();
        app.add_event::<TsyExitEmit>();
        app.add_systems(Startup, spawn_rift_portals);
        app.add_systems(
            Update,
            (
                (tsy_entry_portal_tick, tsy_exit_portal_tick).before(DimensionTransferSet),
                apply_dimension_transfers.in_set(DimensionTransferSet),
            ),
        );

        let mut new_visible = VisibleEntityLayers::default();
        new_visible.0.insert(overworld);
        let new_anchor_player = app
            .world_mut()
            .spawn((
                EntityLayerId(overworld),
                VisibleChunkLayer(overworld),
                new_visible,
                Position::new([2000.0, 74.0, -7300.0]),
                empty_player_inventory(),
                CurrentDimension(DimensionKind::Overworld),
            ))
            .id();
        let mut old_visible = VisibleEntityLayers::default();
        old_visible.0.insert(overworld);
        let old_anchor_player = app
            .world_mut()
            .spawn((
                EntityLayerId(overworld),
                VisibleChunkLayer(overworld),
                old_visible,
                Position::new([2000.0, 74.0, -7800.0]),
                empty_player_inventory(),
                CurrentDimension(DimensionKind::Overworld),
            ))
            .id();

        app.update();

        let mut portal_query = app
            .world_mut()
            .query::<(&RiftPortal, &Position, &EntityLayerId)>();
        let portals = portal_query
            .iter(app.world())
            .filter(|(portal, _, _)| portal.family_id == "zongmen_01")
            .collect::<Vec<_>>();
        assert_eq!(
            portals.len(),
            2,
            "production providers should spawn one entry and one exit marker for zongmen_01"
        );
        let (portal, portal_pos, portal_layer) = portals
            .iter()
            .copied()
            .find(|(portal, _, _)| portal.direction == PortalDirection::Entry)
            .expect("zongmen_01 entry portal must spawn from overworld provider");
        assert_eq!(portal_pos.get(), DVec3::new(2000.0, 74.0, -7300.0));
        assert_eq!(portal_layer.0, overworld);
        assert_eq!(portal.direction, PortalDirection::Entry);
        assert_eq!(portal.kind, RiftKind::MainRift);
        assert_eq!(portal.trigger_radius, 2.0);
        assert_eq!(portal.target.dimension, DimensionKind::Tsy);
        assert_eq!(portal.target.pos, DVec3::new(253.0, 100.0, 250.0));
        let (exit_portal, exit_pos, exit_layer) = portals
            .iter()
            .copied()
            .find(|(portal, _, _)| portal.direction == PortalDirection::Exit)
            .expect("zongmen_01 exit portal must spawn from TSY provider");
        assert_eq!(exit_portal.kind, RiftKind::MainRift);
        assert_eq!(exit_portal.trigger_radius, 1.5);
        assert_eq!(exit_pos.get(), DVec3::new(250.0, 100.0, 250.0));
        assert_eq!(exit_layer.0, tsy);
        assert!(
            portal.target.pos.distance(exit_pos.get()) > exit_portal.trigger_radius,
            "the production entry target must lie outside the matching exit radius"
        );

        let new_player = app.world().entity(new_anchor_player);
        assert_eq!(
            new_player.get::<CurrentDimension>().copied(),
            Some(CurrentDimension(DimensionKind::Tsy))
        );
        assert_eq!(
            new_player.get::<EntityLayerId>().map(|layer| layer.0),
            Some(tsy)
        );
        assert_eq!(
            new_player.get::<VisibleChunkLayer>().map(|layer| layer.0),
            Some(tsy)
        );
        assert_eq!(
            new_player.get::<Position>().map(|position| position.get()),
            Some(DVec3::new(253.0, 100.0, 250.0))
        );
        let new_visible = new_player
            .get::<VisibleEntityLayers>()
            .expect("transferred player must keep visible entity layers");
        assert!(new_visible.0.contains(&tsy));
        assert!(!new_visible.0.contains(&overworld));
        let presence = new_player
            .get::<TsyPresence>()
            .expect("entry portal must attach TSY presence");
        assert_eq!(presence.family_id, "zongmen_01");
        assert_eq!(presence.return_to.dimension, DimensionKind::Overworld);
        assert_eq!(
            presence.return_to.pos,
            DVec3::new(2003.0, 75.0, -7300.0),
            "return anchor must derive from the relocated portal and lie outside radius"
        );

        let old_player = app.world().entity(old_anchor_player);
        assert_eq!(
            old_player.get::<CurrentDimension>().copied(),
            Some(CurrentDimension(DimensionKind::Overworld))
        );
        assert_eq!(
            old_player.get::<Position>().map(|position| position.get()),
            Some(DVec3::new(2000.0, 74.0, -7800.0))
        );
        assert_eq!(
            old_player.get::<EntityLayerId>().map(|layer| layer.0),
            Some(overworld)
        );
        assert_eq!(
            old_player.get::<VisibleChunkLayer>().map(|layer| layer.0),
            Some(overworld)
        );
        let old_visible = old_player
            .get::<VisibleEntityLayers>()
            .expect("non-triggered player must keep visible entity layers");
        assert!(old_visible.0.contains(&overworld));
        assert!(!old_visible.0.contains(&tsy));
        assert!(old_player.get::<TsyPresence>().is_none());

        let enter_events = app.world().resource::<Events<TsyEnterEmit>>();
        let entered = enter_events
            .get_reader()
            .read(enter_events)
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(entered.len(), 1);
        assert_eq!(entered[0].player_entity, new_anchor_player);
        assert_eq!(entered[0].family_id, "zongmen_01");
        assert_eq!(entered[0].return_to, presence.return_to);
        assert!(entered[0].filtered.is_empty());

        app.update();

        let new_player = app.world().entity(new_anchor_player);
        assert_eq!(
            new_player.get::<CurrentDimension>().copied(),
            Some(CurrentDimension(DimensionKind::Tsy)),
            "the production exit portal must not bounce the player back on the next tick"
        );
        assert_eq!(
            new_player.get::<Position>().map(|position| position.get()),
            Some(DVec3::new(253.0, 100.0, 250.0)),
            "the next production tick must keep the player at the exit-radius-safe TSY target"
        );
        assert!(
            new_player.get::<TsyPresence>().is_some(),
            "TsyPresence must survive the tick after entry when the target is outside exit radius"
        );
        let exit_events = app.world().resource::<Events<TsyExitEmit>>();
        assert_eq!(
            exit_events.get_reader().read(exit_events).count(),
            0,
            "no TsyExitEmit may fire on the tick after production entry"
        );

        let registry = ZoneRegistry::load();
        assert_eq!(
            registry
                .find_zone(DimensionKind::Tsy, DVec3::new(253.0, 100.0, 250.0))
                .map(|zone| zone.name.as_str()),
            Some("tsy_zongmen_01_shallow"),
            "portal target must remain inside the production-merged TSY entry zone"
        );
    }
}
