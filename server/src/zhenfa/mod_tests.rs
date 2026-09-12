#![allow(dead_code)]
    use super::*;
    use crate::combat::events::{CombatEvent, DeathEvent};
    use crate::cultivation::components::{QiColor, Realm};
    use crate::inventory::{
        inventory_item_by_instance_borrow, ContainerState, InventoryRevision, ItemCategory,
        ItemInstance, ItemRarity, ItemTemplate, PlayerInventory, SlotContents, EQUIP_SLOT_CHEST,
        EQUIP_SLOT_MAIN_HAND, EQUIP_SLOT_OFF_HAND,
    };
    use crate::lingtian::PLOT_QI_CAP_BASE;
    use valence::prelude::{App, ChunkLayer, DVec3, Entity, Events, UnloadedChunk};
    use valence::testing::ScenarioSingleClient;

    fn app_with_zhenfa() -> App {
        let mut app = App::new();
        install_zhenfa_test_systems(&mut app);
        app
    }

    fn app_with_zhenfa_layer() -> (App, Entity) {
        let scenario = ScenarioSingleClient::new();
        let mut app = scenario.app;
        crate::world::dimension::mark_test_layer_as_overworld(&mut app);
        app.world_mut()
            .get_mut::<ChunkLayer>(scenario.layer)
            .expect("test layer should carry ChunkLayer")
            .insert_chunk([0, 0], UnloadedChunk::new());
        install_zhenfa_test_systems(&mut app);
        (app, scenario.layer)
    }

    fn app_with_loaded_zhenfa() -> App {
        let (app, _) = app_with_zhenfa_layer();
        app
    }

    fn app_with_zhenfa_unloaded_layer() -> (App, Entity) {
        let scenario = ScenarioSingleClient::new();
        let mut app = scenario.app;
        crate::world::dimension::mark_test_layer_as_overworld(&mut app);
        install_zhenfa_test_systems(&mut app);
        (app, scenario.layer)
    }

    fn zhenfa_eye_state(charged: bool) -> BlockState {
        BlockState::BONG_ZHENFA_EYE.set(
            PropName::Charged,
            if charged {
                PropValue::True
            } else {
                PropValue::False
            },
        )
    }

    fn layer_block_state(app: &App, layer_entity: Entity, pos: [i32; 3]) -> Option<BlockState> {
        app.world()
            .get::<ChunkLayer>(layer_entity)
            .and_then(|layer| {
                layer
                    .block(block_pos_from_array(pos))
                    .map(|block| block.state)
            })
    }

    fn install_zhenfa_test_systems(app: &mut App) {
        app.insert_resource(CombatClock::default());
        app.insert_resource(PendingGameplayNarrations::default());
        app.insert_resource(WorldQiAccount::default());
        app.insert_resource(ScatterBeadBurials::default());
        app.insert_resource(ScatterDisturbanceZones::default());
        app.add_event::<ZhenfaPlaceRequest>();
        app.add_event::<ZhenfaTriggerRequest>();
        app.add_event::<ZhenfaDisarmRequest>();
        app.add_event::<ScatterBeadUseRequest>();
        app.add_event::<ScatterBeadTriggerRequest>();
        app.add_event::<ZhenfaSensePulse>();
        app.add_event::<WardArrayDeployEvent>();
        app.add_event::<LingArrayDeployEvent>();
        app.add_event::<DeceiveHeavenEvent>();
        app.add_event::<DeceiveHeavenExposedEvent>();
        app.add_event::<IllusionArrayDeployEvent>();
        app.add_event::<NetworkArrayDeployEvent>();
        app.add_event::<ArrayDecayEvent>();
        app.add_event::<ArrayBreakthroughEvent>();
        app.add_event::<QiTransfer>();
        app.add_event::<JueBiTriggerEvent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.insert_resource(ZhenfaRegistry::default());
        app.add_systems(
            Update,
            (
                handle_zhenfa_place_requests,
                handle_scatter_bead_use,
                handle_scatter_bead_trigger_requests,
                handle_zhenfa_trigger_requests,
                handle_zhenfa_disarm_requests,
                tick_scatter_bead_excretion,
                tick_scatter_disturbance_zones,
                handle_decoy_stake_attack_intents,
                tick_zhenfa_registry,
            )
                .chain(),
        );
    }

    fn spawn_player(app: &mut App, name: &str, pos: [f64; 3]) -> Entity {
        spawn_player_with_inventory(app, name, pos, zhenfa_flag_inventory())
    }

    fn spawn_player_with_inventory(
        app: &mut App,
        name: &str,
        pos: [f64; 3],
        inventory: PlayerInventory,
    ) -> Entity {
        app.world_mut()
            .spawn((
                Username(name.to_string()),
                UniqueId::default(),
                Position::new(pos),
                Cultivation {
                    realm: Realm::Induce,
                    qi_current: 100.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                QiColor::default(),
                PracticeLog::default(),
                Wounds::default(),
                Contamination::default(),
                MeridianSystem::default(),
                inventory,
            ))
            .id()
    }

    fn spawn_plot(app: &mut App, pos: [i32; 3], cap: f32) -> Entity {
        let mut plot = LingtianPlot::new(block_pos_from_array(pos), None);
        plot.plot_qi_cap = cap;
        app.world_mut().spawn(plot).id()
    }

    fn plot_cap(app: &mut App, pos: [i32; 3]) -> f32 {
        app.world_mut()
            .query::<&LingtianPlot>()
            .iter(app.world())
            .find(|plot| plot.pos == block_pos_from_array(pos))
            .map(|plot| plot.plot_qi_cap)
            .expect("test plot should exist")
    }

    fn send_lingju_place(app: &mut App, player: Entity, pos: [i32; 3], tick: u64) {
        app.world_mut().send_event(ZhenfaPlaceRequest {
            player,
            pos,
            kind: ZhenfaKind::Lingju,
            carrier: ZhenfaCarrierKind::BeastCoreInlaid,
            qi_invest_ratio: 0.30,
            trigger: None,
            item_instance_id: None,
            target_face: None,
            requested_at_tick: tick,
        });
    }






    #[test]
    fn network_array_three_flags_and_eye_form_active_cap_feedback_and_consume_items() {
        let mut app = app_with_loaded_zhenfa();
        app.insert_resource(ZoneRegistry::fallback());
        app.add_event::<VfxEventRequest>();
        let owner = spawn_player_with_inventory(
            &mut app,
            "Alice",
            [0.5, 64.0, 0.5],
            network_array_test_inventory(),
        );
        spawn_plot(&mut app, [2, 64, 2], PLOT_QI_CAP_BASE);

        place_basic_network_array(&mut app, owner, 1);

        let registry = app.world().resource::<ZhenfaRegistry>();
        let networks = registry.active_network_arrays().collect::<Vec<_>>();
        assert_eq!(networks.len(), 1, "三旗 + 圈内阵眼必须激活一个组网阵");
        assert_eq!(
            networks[0].flag_instance_ids.len(),
            3,
            "成阵后必须记录 3 面阵旗，供破阵和 HUD 文案使用"
        );
        assert!(
            (plot_cap(&mut app, [2, 64, 2])
                - (PLOT_QI_CAP_BASE + QI_NETWORK_ARRAY_LINGJU_CAP_BONUS))
                .abs()
                < 1e-6,
            "圈内 plot 应获得凡阶组网阵 +QI_NETWORK_ARRAY_LINGJU_CAP_BONUS cap"
        );

        let network_events = app.world().resource::<Events<NetworkArrayDeployEvent>>();
        let deploy = network_events
            .iter_current_update_events()
            .last()
            .expect("阵眼激活应发 NetworkArrayDeployEvent");
        assert_eq!(deploy.pos, [1, 64, 1]);
        assert_eq!(deploy.owner, owner);

        let vfx_events = app.world().resource::<Events<VfxEventRequest>>();
        assert!(
            vfx_events
                .iter_current_update_events()
                .any(|event| matches!(
                    &event.payload,
                    crate::schema::vfx_event::VfxEventPayloadV1::SpawnParticle { event_id, .. }
                        if event_id == gameplay_vfx::NETWORK_ARRAY_FORM
                )),
            "成阵必须发 bong:network_array_form VFX"
        );
        assert!(
            app.world_mut()
                .resource_mut::<PendingGameplayNarrations>()
                .drain()
                .iter()
                .any(|entry| entry.text.contains("组网阵已成")),
            "成阵必须给 owner 推 HUD/narration 文案"
        );
        for instance_id in [8101, 8102, 8103, 8201] {
            assert!(
                !inventory_still_has_item(&app, owner, instance_id),
                "network array item instance {instance_id} 应在放置成功后被消耗"
            );
        }
    }

    #[test]
    fn network_array_two_flags_and_eye_do_not_activate_or_boost_plot() {
        let mut app = app_with_loaded_zhenfa();
        let owner = spawn_player_with_inventory(
            &mut app,
            "Alice",
            [0.5, 64.0, 0.5],
            network_array_inventory(&[
                (8101, NETWORK_ARRAY_FLAG_ITEM_ID),
                (8102, NETWORK_ARRAY_FLAG_ITEM_ID),
                (8201, NETWORK_ARRAY_EYE_ITEM_ID),
            ]),
        );
        spawn_plot(&mut app, [2, 64, 2], PLOT_QI_CAP_BASE);

        place_network_array_node(&mut app, owner, [0, 64, 0], 8101, 1);
        place_network_array_node(&mut app, owner, [6, 64, 0], 8102, 2);
        place_network_array_node(&mut app, owner, [1, 64, 1], 8201, 3);

        assert_eq!(
            app.world()
                .resource::<ZhenfaRegistry>()
                .active_network_arrays()
                .count(),
            0,
            "两旗低于凸多边形下限，阵眼不应激活组网阵"
        );
        assert!(
            (plot_cap(&mut app, [2, 64, 2]) - PLOT_QI_CAP_BASE).abs() < 1e-6,
            "未成阵时 plot cap 必须保持基线"
        );
        assert!(
            app.world()
                .resource::<Events<NetworkArrayDeployEvent>>()
                .iter_current_update_events()
                .next()
                .is_none(),
            "未成阵不得广播 network_array deploy，避免 agent 误报"
        );
    }

    #[test]
    fn network_array_breaking_flag_dissolves_network_and_restores_cap() {
        let mut app = app_with_loaded_zhenfa();
        app.add_event::<VfxEventRequest>();
        let owner = spawn_player_with_inventory(
            &mut app,
            "Alice",
            [0.5, 64.0, 0.5],
            network_array_test_inventory(),
        );
        spawn_plot(&mut app, [2, 64, 2], PLOT_QI_CAP_BASE);
        place_basic_network_array(&mut app, owner, 1);
        assert!(
            (plot_cap(&mut app, [2, 64, 2])
                - (PLOT_QI_CAP_BASE + QI_NETWORK_ARRAY_LINGJU_CAP_BONUS))
                .abs()
                < 1e-6
        );

        app.world_mut().send_event(ZhenfaDisarmRequest {
            player: owner,
            pos: [0, 64, 0],
            mode: ZhenfaDisarmMode::ForceBreak,
            requested_at_tick: 10,
        });
        app.update();

        assert_eq!(
            app.world()
                .resource::<ZhenfaRegistry>()
                .active_network_arrays()
                .count(),
            0,
            "任一阵旗被拆后 active network 必须失效"
        );
        assert!(
            (plot_cap(&mut app, [2, 64, 2]) - PLOT_QI_CAP_BASE).abs() < 1e-6,
            "组网阵破后 plot cap 必须恢复基线"
        );
        assert!(
            app.world_mut()
                .resource_mut::<PendingGameplayNarrations>()
                .drain()
                .iter()
                .any(|entry| entry.text.contains("阵破")),
            "破阵必须给 owner 推阵破提示"
        );
    }


    #[test]
    fn network_array_alerts_on_entry_and_respects_ward_throttle() {
        let mut app = app_with_loaded_zhenfa();
        let owner = spawn_player_with_inventory(
            &mut app,
            "Alice",
            [0.5, 64.0, 0.5],
            network_array_test_inventory(),
        );
        place_basic_network_array(&mut app, owner, 1);
        app.world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();

        let intruder = app.world_mut().spawn(Position::new([2.5, 64.0, 2.5])).id();
        app.world_mut().resource_mut::<CombatClock>().tick = 10;
        app.update();
        assert_eq!(
            app.world_mut()
                .resource_mut::<PendingGameplayNarrations>()
                .drain()
                .iter()
                .filter(|entry| entry.text.contains("阵内有动静"))
                .count(),
            1,
            "首次进入组网阵应给 owner 发警戒提示"
        );

        app.world_mut()
            .entity_mut(intruder)
            .insert(Position::new([20.0, 64.0, 20.0]));
        app.world_mut().resource_mut::<CombatClock>().tick = 11;
        app.update();
        app.world_mut()
            .entity_mut(intruder)
            .insert(Position::new([2.5, 64.0, 2.5]));
        app.world_mut().resource_mut::<CombatClock>().tick = 12;
        app.update();
        assert!(
            app.world_mut()
                .resource_mut::<PendingGameplayNarrations>()
                .drain()
                .is_empty(),
            "WARD_ALERT_THROTTLE_TICKS 内重复进入不得刷屏"
        );

        app.world_mut()
            .entity_mut(intruder)
            .insert(Position::new([20.0, 64.0, 20.0]));
        app.world_mut().resource_mut::<CombatClock>().tick = 13;
        app.update();
        app.world_mut()
            .entity_mut(intruder)
            .insert(Position::new([2.5, 64.0, 2.5]));
        app.world_mut().resource_mut::<CombatClock>().tick =
            10_u64.saturating_add(WARD_ALERT_THROTTLE_TICKS);
        app.update();
        assert_eq!(
            app.world_mut()
                .resource_mut::<PendingGameplayNarrations>()
                .drain()
                .iter()
                .filter(|entry| entry.text.contains("阵内有动静"))
                .count(),
            1,
            "超过 WARD_ALERT_THROTTLE_TICKS 后再次进入应重新警戒"
        );
    }

    #[test]
    fn network_array_rejects_non_matching_item_without_consuming_it() {
        let mut app = app_with_loaded_zhenfa();
        let owner = spawn_player_with_inventory(
            &mut app,
            "Alice",
            [0.5, 64.0, 0.5],
            network_array_inventory(&[(8301, ZHENFA_FLAG_ITEM_ID)]),
        );

        send_network_array_place(&mut app, owner, [0, 64, 0], 8301, 1);
        app.update();

        assert_eq!(
            app.world().resource::<ZhenfaRegistry>().len(),
            0,
            "旧 array_flag 不能冒充 array_flag_basic 参与 NetworkArray 放置"
        );
        assert!(
            inventory_still_has_item(&app, owner, 8301),
            "NetworkArray 拒绝非匹配 item 时不得消耗玩家物品"
        );
    }



    #[test]
    fn scatter_bead_active_use_consumes_item_and_applies_ledger_transfer() {
        let mut app = app_with_loaded_zhenfa();
        let mut zones = ZoneRegistry::fallback();
        zones.zones[0].spirit_qi = 0.2;
        app.insert_resource(zones);
        app.add_event::<VfxEventRequest>();
        let owner = spawn_player_with_inventory(
            &mut app,
            "Alice",
            [0.5, 64.0, 0.5],
            scatter_bead_inventory(7001),
        );

        app.world_mut().send_event(ScatterBeadUseRequest {
            player: owner,
            item_instance_id: 7001,
            bury_pos: None,
            requested_at_tick: 1,
        });
        app.update();

        let inventory = app
            .world()
            .get::<PlayerInventory>(owner)
            .expect("player inventory should exist");
        assert!(
            inventory_item_by_instance_borrow(inventory, 7001).is_none(),
            "主动使用散灵珠必须消费对应 inventory instance"
        );

        let zone = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .expect("spawn zone should exist");
        assert!(
            (zone.spirit_qi - (0.2 + QI_SCATTER_BEAD_CAPACITY / QI_ZONE_UNIT_CAPACITY)).abs()
                < 1e-9,
            "zone 浓度增量必须等于散灵珠实际注入量 / QI_ZONE_UNIT_CAPACITY"
        );
        assert!(zone
            .active_events
            .iter()
            .any(|event| event == SCATTER_DISTURBANCE_EVENT));

        let source = QiAccountId::container("qi_scatter:offline:Alice:7001");
        let zone_account = QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME);
        let ledger = app.world().resource::<WorldQiAccount>();
        assert!(
            ledger.balance(&source) <= QI_EPSILON,
            "散灵珠 source account 应在主动使用后归零"
        );
        assert!(
            (ledger.balance(&zone_account) - QI_SCATTER_BEAD_CAPACITY).abs() < 1e-9,
            "WorldQiAccount zone balance 必须真实接收散灵珠真元，不能只 emit event"
        );
        assert!(ledger.transfers().iter().any(|transfer| {
            transfer.from == source
                && transfer.to == zone_account
                && (transfer.amount - QI_SCATTER_BEAD_CAPACITY).abs() < 1e-9
                && transfer.reason == QiTransferReason::ReleaseToZone
        }));

        let vfx_events = app.world().resource::<Events<VfxEventRequest>>();
        assert!(vfx_events
            .iter_current_update_events()
            .any(|event| matches!(
                &event.payload,
                crate::schema::vfx_event::VfxEventPayloadV1::SpawnParticle { event_id, .. }
                    if event_id == gameplay_vfx::SCATTER_BURST
            )));
        let narrations = app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();
        assert_eq!(narrations.len(), 2, "主动破裂应入队两条感知旁白");
        assert!(narrations.iter().all(|narration| matches!(
            narration.scope,
            crate::schema::common::NarrationScope::Zone
        )));
    }



    #[test]
    fn scatter_disturbance_tag_expires() {
        let mut app = app_with_loaded_zhenfa();
        app.insert_resource(ZoneRegistry::fallback());
        let owner = spawn_player_with_inventory(
            &mut app,
            "Alice",
            [0.5, 64.0, 0.5],
            scatter_bead_inventory(7004),
        );

        app.world_mut().send_event(ScatterBeadUseRequest {
            player: owner,
            item_instance_id: 7004,
            bury_pos: None,
            requested_at_tick: 1,
        });
        app.update();
        assert!(app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .expect("spawn zone should exist")
            .active_events
            .iter()
            .any(|event| event == SCATTER_DISTURBANCE_EVENT));

        app.world_mut().resource_mut::<CombatClock>().tick = 1 + SCATTER_DISTURBANCE_DURATION_TICKS;
        app.update();
        assert!(!app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .expect("spawn zone should exist")
            .active_events
            .iter()
            .any(|event| event == SCATTER_DISTURBANCE_EVENT));
    }

    #[test]
    fn buried_scatter_bead_excretes_conservatively_and_elapsed_zero_is_stable() {
        let mut app = app_with_loaded_zhenfa();
        let mut zones = ZoneRegistry::fallback();
        zones.zones[0].spirit_qi = 0.0;
        app.insert_resource(zones);
        let owner = spawn_player_with_inventory(
            &mut app,
            "Alice",
            [0.5, 64.0, 0.5],
            scatter_bead_inventory(7005),
        );

        app.world_mut().send_event(ScatterBeadUseRequest {
            player: owner,
            item_instance_id: 7005,
            bury_pos: Some([0, 64, 0]),
            requested_at_tick: 0,
        });

        app.update();
        let bead_id = 1;
        assert_eq!(
            app.world()
                .resource::<ScatterBeadBurials>()
                .beads
                .get(&bead_id)
                .expect("buried bead should still exist")
                .remaining_qi,
            QI_SCATTER_BEAD_CAPACITY,
            "elapsed=0 时预埋散灵珠 remaining 不应变化"
        );

        app.world_mut().resource_mut::<CombatClock>().tick = 60 * TICKS_PER_SECOND;
        app.update();

        let remaining = app
            .world()
            .resource::<ScatterBeadBurials>()
            .beads
            .get(&bead_id)
            .expect("60 秒后不应立即归零")
            .remaining_qi;
        assert!(
            remaining < QI_SCATTER_BEAD_CAPACITY && remaining > 0.0,
            "预埋散灵珠应随 EmbeddedTrap 逸散曲线单调递减"
        );
        let source = QiAccountId::container(format!("qi_scatter_buried:offline:Alice:{bead_id}"));
        let zone_account = QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME);
        let ledger = app.world().resource::<WorldQiAccount>();
        assert!(
            (ledger.balance(&source) - remaining).abs() < 1e-9,
            "buried source account balance 必须等于剩余真元"
        );
        assert!(
            (remaining + ledger.balance(&zone_account) - QI_SCATTER_BEAD_CAPACITY).abs() < 1e-9,
            "bead_remaining + 已注入 zone 必须闭合为 QI_SCATTER_BEAD_CAPACITY"
        );
    }

    #[test]
    fn buried_scatter_bead_owner_trigger_releases_remaining_qi() {
        let mut app = app_with_loaded_zhenfa();
        let mut zones = ZoneRegistry::fallback();
        zones.zones[0].spirit_qi = 0.0;
        app.insert_resource(zones);
        app.add_event::<VfxEventRequest>();
        let owner = spawn_player_with_inventory(
            &mut app,
            "Alice",
            [0.5, 64.0, 0.5],
            scatter_bead_inventory(7006),
        );
        let intruder = spawn_player(&mut app, "Bob", [0.5, 64.0, 0.5]);

        app.world_mut().send_event(ScatterBeadUseRequest {
            player: owner,
            item_instance_id: 7006,
            bury_pos: Some([0, 64, 0]),
            requested_at_tick: 0,
        });
        app.update();
        let bead_id = 1;

        app.world_mut().send_event(ScatterBeadTriggerRequest {
            player: intruder,
            bead_id,
            requested_at_tick: 1,
        });
        app.update();
        assert!(
            app.world()
                .resource::<ScatterBeadBurials>()
                .beads
                .contains_key(&bead_id),
            "非 owner 触发预埋散灵珠必须被拒绝并保留埋设记录"
        );

        app.world_mut().send_event(ScatterBeadTriggerRequest {
            player: owner,
            bead_id,
            requested_at_tick: 2,
        });
        app.update();
        assert!(
            !app.world()
                .resource::<ScatterBeadBurials>()
                .beads
                .contains_key(&bead_id),
            "owner 触发后预埋散灵珠必须移除"
        );
        let zone_account = QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME);
        let ledger = app.world().resource::<WorldQiAccount>();
        assert!(
            (ledger.balance(&zone_account) - QI_SCATTER_BEAD_CAPACITY).abs() < 1e-9,
            "owner 触发预埋散灵珠应将剩余真元守恒释放到 zone"
        );
    }

    #[test]
    fn buried_scatter_bead_trigger_requires_owner() {
        let mut burials = ScatterBeadBurials::default();
        let mut app = app_with_zhenfa();
        let owner = app.world_mut().spawn_empty().id();
        let intruder = app.world_mut().spawn_empty().id();
        let bead_id = burials.insert(
            owner,
            "offline:Alice",
            [0, 64, 0],
            QI_SCATTER_BEAD_CAPACITY,
            0,
        );

        assert!(
            burials.trigger_buried(bead_id, intruder, 1).is_none(),
            "非 owner 触发预埋散灵珠必须被拒绝"
        );
        assert!(burials.beads.contains_key(&bead_id));
        assert!(
            burials.trigger_buried(bead_id, owner, 2).is_some(),
            "owner 可以触发自己的预埋散灵珠"
        );
        assert!(!burials.beads.contains_key(&bead_id));
    }

    #[test]
    fn clear_lingju_effect_ignores_removed_or_unknown_instance() {
        let mut app = app_with_zhenfa();
        spawn_plot(&mut app, [0, 64, 0], PLOT_QI_CAP_BASE);
        let owner = app.world_mut().spawn_empty().id();
        let anchor_entity = app.world_mut().spawn_empty().id();
        let instance = ZhenfaInstance {
            id: 404,
            kind: ZhenfaKind::Lingju,
            owner,
            owner_player_id: "offline:Alice".to_string(),
            pos: [0, 64, 0],
            carrier: ZhenfaCarrierKind::BeastCoreInlaid,
            qi_invest_ratio: 0.30,
            qi_invest_amount: 30.0,
            realm_at_cast: Realm::Induce,
            mastery_at_cast: 0.0,
            effect_radius: 20,
            ward_radius: 20,
            placed_at_tick: 1,
            expires_at_tick: 100,
            triggered_at: None,
            trigger: None,
            color_main: ColorKind::Intricate,
            color_secondary: None,
            anchor_entity,
        };

        app.world_mut()
            .resource_scope(|world, mut registry: Mut<ZhenfaRegistry>| {
                let mut plots = world.query::<&mut LingtianPlot>();
                clear_lingju_effect_for_plots(&instance, &mut registry, plots.iter_mut(world));
            });
        app.update();

        assert!(
            (plot_cap(&mut app, [0, 64, 0]) - PLOT_QI_CAP_BASE).abs() < 1e-6,
            "未知/已移除 Lingju 清理不应 panic，也不应改动未覆盖 plot"
        );
    }


    fn array_flag_item(instance_id: u64) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: ZHENFA_FLAG_ITEM_ID.to_string(),
            display_name: "阵旗".to_string(),
            grid_w: 1,
            grid_h: 2,
            weight: 0.6,
            rarity: ItemRarity::Uncommon,
            description: "地师用来牵引阵眼气机的短旗。".to_string(),
            stack_count: 1,
            spirit_quality: 0.8,
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
        }
    }

    fn trap_item(instance_id: u64, template_id: &str, display_name: &str) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: template_id.to_string(),
            display_name: display_name.to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.1,
            rarity: ItemRarity::Common,
            description: display_name.to_string(),
            stack_count: 1,
            spirit_quality: 1.0,
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
        }
    }

    fn material_item(instance_id: u64, template_id: &str, stack_count: u32) -> ItemInstance {
        let mut item = trap_item(instance_id, template_id, template_id);
        item.stack_count = stack_count;
        item
    }

    fn scatter_bead_item(instance_id: u64) -> ItemInstance {
        trap_item(instance_id, QI_SCATTER_BEAD_ITEM_ID, "散灵珠")
    }

    fn network_array_item(instance_id: u64, template_id: &str) -> ItemInstance {
        let mut item = trap_item(instance_id, template_id, template_id);
        item.spirit_quality = match template_id {
            NETWORK_ARRAY_EYE_ITEM_ID => 0.5,
            NETWORK_ARRAY_FLAG_ITEM_ID => 0.0,
            _ => item.spirit_quality,
        };
        item
    }

    fn network_array_inventory(items: &[(u64, &str)]) -> PlayerInventory {
        let mut inventory = zhenfa_flag_inventory();
        for (slot, (instance_id, template_id)) in items.iter().enumerate() {
            inventory.containers[0]
                .items
                .push(crate::inventory::PlacedItemState {
                    row: 0,
                    col: slot as u8,
                    instance: network_array_item(*instance_id, template_id),
                });
        }
        inventory
    }

    fn network_array_test_inventory() -> PlayerInventory {
        network_array_inventory(&[
            (8101, NETWORK_ARRAY_FLAG_ITEM_ID),
            (8102, NETWORK_ARRAY_FLAG_ITEM_ID),
            (8103, NETWORK_ARRAY_FLAG_ITEM_ID),
            (8201, NETWORK_ARRAY_EYE_ITEM_ID),
        ])
    }

    fn send_network_array_place(
        app: &mut App,
        player: Entity,
        pos: [i32; 3],
        item_instance_id: u64,
        tick: u64,
    ) {
        app.world_mut().send_event(ZhenfaPlaceRequest {
            player,
            pos,
            kind: ZhenfaKind::NetworkArray,
            carrier: ZhenfaCarrierKind::CommonStone,
            qi_invest_ratio: 0.0,
            trigger: None,
            item_instance_id: Some(item_instance_id),
            target_face: None,
            requested_at_tick: tick,
        });
    }

    fn place_network_array_node(
        app: &mut App,
        player: Entity,
        pos: [i32; 3],
        item_instance_id: u64,
        tick: u64,
    ) {
        send_network_array_place(app, player, pos, item_instance_id, tick);
        app.update();
    }

    fn place_basic_network_array(app: &mut App, owner: Entity, start_tick: u64) {
        place_network_array_node(app, owner, [0, 64, 0], 8101, start_tick);
        place_network_array_node(app, owner, [6, 64, 0], 8102, start_tick + 1);
        place_network_array_node(app, owner, [0, 64, 6], 8103, start_tick + 2);
        place_network_array_node(app, owner, [1, 64, 1], 8201, start_tick + 3);
    }

    fn inventory_still_has_item(app: &App, player: Entity, instance_id: u64) -> bool {
        app.world()
            .get::<PlayerInventory>(player)
            .and_then(|inventory| inventory_item_by_instance_borrow(inventory, instance_id))
            .is_some()
    }

    fn released_zhenfa_qi_total(transfers: &Events<QiTransfer>) -> f64 {
        transfers
            .iter_current_update_events()
            .filter(|transfer| {
                transfer.reason == QiTransferReason::ReleaseToZone
                    && transfer.from.kind == crate::qi_physics::QiAccountKind::Container
                    && transfer.from.id.starts_with("zhenfa_trap:")
            })
            .map(|transfer| transfer.amount)
            .sum()
    }

    fn zhenfa_flag_inventory() -> PlayerInventory {
        let mut inventory = empty_inventory();
        inventory.equipped.insert(
            EQUIP_SLOT_MAIN_HAND.to_string(),
            SlotContents::held_single(array_flag_item(9001)),
        );
        inventory
    }

    fn deceive_heaven_material_inventory() -> PlayerInventory {
        let mut inventory = zhenfa_flag_inventory();
        inventory.bone_coins = DECEIVE_HEAVEN_BONE_COIN_COST;
        inventory.containers[0]
            .items
            .push(crate::inventory::PlacedItemState {
                row: 0,
                col: 0,
                instance: material_item(
                    9101,
                    DECEIVE_HEAVEN_SPIRITWOOD_ITEM_ID,
                    DECEIVE_HEAVEN_SPIRITWOOD_COST,
                ),
            });
        inventory.containers[0]
            .items
            .push(crate::inventory::PlacedItemState {
                row: 0,
                col: 1,
                instance: material_item(
                    9102,
                    DECEIVE_HEAVEN_BEAST_BONE_ITEM_ID,
                    DECEIVE_HEAVEN_BEAST_BONE_COST,
                ),
            });
        inventory
    }

    fn ordinary_trap_inventory(item: ItemInstance) -> PlayerInventory {
        let mut inventory = empty_inventory();
        inventory.equipped.insert(
            EQUIP_SLOT_MAIN_HAND.to_string(),
            SlotContents::held_single(item),
        );
        inventory
    }

    fn scatter_bead_inventory(instance_id: u64) -> PlayerInventory {
        ordinary_trap_inventory(scatter_bead_item(instance_id))
    }

    fn pearl_registry() -> ItemRegistry {
        let template = ItemTemplate {
            id: ZHENFA_PEARL_ITEM_ID.to_string(),
            display_name: "散逸真元珠".to_string(),
            category: ItemCategory::Misc,
            placeable: None,
            max_stack_count: 1,
            grid_w: 1,
            grid_h: 1,
            base_weight: 0.05,
            rarity: ItemRarity::Uncommon,
            spirit_quality_initial: 0.6,
            description: "破阵后凝住的一小粒散逸真元。".to_string(),
            effect: None,
            cast_duration_ms: 1500,
            cooldown_ms: 1500,
            weapon_spec: None,
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
            technique_scroll_spec: None,
            readable_scroll_spec: None,
            recipe_fragment_spec: None,
            container_spec: None,
            shelflife_profile: None,
            shield_spec: None,
            shelflife_track: None,
            wearer_race: crate::body_plan::types::RaceGateOwned::default(),
        };
        ItemRegistry::from_map(HashMap::from([(
            ZHENFA_PEARL_ITEM_ID.to_string(),
            template,
        )]))
    }

    fn empty_inventory() -> PlayerInventory {
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                quick_access: false,
                id: crate::inventory::MAIN_PACK_CONTAINER_ID.to_string(),
                name: "main".to_string(),
                rows: 4,
                cols: 6,
                items: Vec::new(),
                owner_instance_id: None,
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 45.0,
        }
    }


    // ── plan-skill-av-relink-v1 P3 —— rune_draw 内联 emit pin ────────────────────

    fn drain_rune_draw_anims(app: &mut App) -> Vec<(String, u16)> {
        app.world_mut()
            .resource_mut::<Events<VfxEventRequest>>()
            .drain()
            .filter_map(|request| match request.payload {
                crate::schema::vfx_event::VfxEventPayloadV1::PlayAnim {
                    target_player,
                    anim_id,
                    priority,
                    ..
                } if anim_id == crate::network::vfx_animation_trigger::ANIM_RUNE_DRAW => {
                    Some((target_player, priority))
                }
                _ => None,
            })
            .collect()
    }

    /// happy path：普通陷阱（无 deploy 事件覆盖的 kind——内联点存在的理由）落阵成功
    /// 恰发一条 rune_draw 画符动画，target = 落阵者本人 uuid。


    /// 重复触发语义：每次成功落阵各配一次画符动画（两阵两动画，1:1 无去重）。


    /// 错误分支：材料门拒绝（DeceiveHeaven 缺料）→ 落阵失败不发 rune_draw。


    /// 错误分支：目标 chunk 未加载 → 自定义方块写入失败、落阵回滚，不发 rune_draw。



    #[test]
    fn deceive_heaven_consumes_spiritwood_beast_bone_bone_coin_and_qi() {
        let mut app = app_with_loaded_zhenfa();
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        app.world_mut().get_mut::<Cultivation>(owner).unwrap().realm = Realm::Solidify;
        app.world_mut()
            .entity_mut(owner)
            .insert(deceive_heaven_material_inventory());

        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos: [1, 64, 1],
            kind: ZhenfaKind::DeceiveHeaven,
            carrier: ZhenfaCarrierKind::BeastCoreInlaid,
            qi_invest_ratio: 0.80,
            trigger: None,
            item_instance_id: None,
            target_face: None,
            requested_at_tick: 10,
        });
        app.update();

        assert_eq!(app.world().resource::<ZhenfaRegistry>().len(), 1);
        let cultivation = app.world().get::<Cultivation>(owner).unwrap();
        assert_eq!(cultivation.qi_current, 20.0);
        let inventory = app.world().get::<PlayerInventory>(owner).unwrap();
        assert_eq!(inventory.bone_coins, 0);
        assert_eq!(
            inventory_template_count(inventory, DECEIVE_HEAVEN_SPIRITWOOD_ITEM_ID),
            0
        );
        assert_eq!(
            inventory_template_count(inventory, DECEIVE_HEAVEN_BEAST_BONE_ITEM_ID),
            0
        );
    }







    #[test]
    fn placement_registry_failure_cleans_world_block_and_anchor_entity() {
        let (mut app, layer_entity) = app_with_zhenfa_layer();
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        let pos = [1, 64, 1];
        app.world_mut()
            .resource_mut::<ZhenfaRegistry>()
            .by_pos
            .insert(pos, 999);

        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos,
            kind: ZhenfaKind::Trap,
            carrier: ZhenfaCarrierKind::LingqiBlock,
            qi_invest_ratio: 0.20,
            trigger: None,
            item_instance_id: None,
            target_face: None,
            requested_at_tick: 10,
        });
        app.update();

        assert_eq!(
            app.world().get::<Cultivation>(owner).unwrap().qi_current,
            100.0
        );
        assert_eq!(app.world().resource::<ZhenfaRegistry>().len(), 0);
        assert_eq!(
            layer_block_state(&app, layer_entity, pos),
            Some(BlockState::AIR)
        );
        let anchor_count = {
            let world = app.world_mut();
            let mut query = world.query::<&ZhenfaAnchor>();
            query.iter(world).count()
        };
        assert_eq!(anchor_count, 0);
    }




    #[test]
    fn place_warning_deducts_qi_and_consumes_trap_item_without_array_flag() {
        let mut app = app_with_loaded_zhenfa();
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        app.world_mut()
            .entity_mut(owner)
            .insert(ordinary_trap_inventory(trap_item(
                9101,
                trap_content::WARNING_TRAP_ITEM_ID,
                "警示符",
            )));

        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos: [1, 64, 1],
            kind: ZhenfaKind::WarningTrap,
            carrier: ZhenfaCarrierKind::CommonStone,
            qi_invest_ratio: 0.0,
            trigger: None,
            item_instance_id: Some(9101),
            target_face: Some(trap_content::TrapTargetFace::Top),
            requested_at_tick: 10,
        });
        app.update();

        assert_eq!(
            app.world().get::<Cultivation>(owner).unwrap().qi_current,
            98.0
        );
        let registry = app.world().resource::<ZhenfaRegistry>();
        let instance = registry
            .find_at([1, 64, 1])
            .expect("warning trap should be placed");
        assert_eq!(instance.kind, ZhenfaKind::WarningTrap);
        assert_eq!(instance.qi_invest_amount, 2.0);
        let transfers = app.world().resource::<Events<QiTransfer>>();
        let seal_transfer = transfers
            .iter_current_update_events()
            .find(|transfer| {
                transfer.reason == QiTransferReason::Channeling
                    && transfer.to
                        == zhenfa_sealed_qi_account(&instance.owner_player_id, instance.id)
            })
            .expect("warning trap placement must channel sealed qi into the zhenfa container");
        assert_eq!(
            seal_transfer.from,
            QiAccountId::player(instance.owner_player_id.clone())
        );
        assert!((seal_transfer.amount - 2.0).abs() < f64::EPSILON);
        let inventory = app.world().get::<PlayerInventory>(owner).unwrap();
        assert!(inventory_item_by_instance_borrow(inventory, 9101).is_none());
    }



    fn assert_runtime_trap_place_rejected(
        kind: ZhenfaKind,
        item_id: &str,
        request_item_instance_id: Option<u64>,
        request_face: Option<trap_content::TrapTargetFace>,
        inventory_item_instance_id: u64,
        pos: [i32; 3],
        case_label: &str,
    ) {
        let mut app = app_with_loaded_zhenfa();
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        app.world_mut()
            .entity_mut(owner)
            .insert(ordinary_trap_inventory(trap_item(
                inventory_item_instance_id,
                item_id,
                item_id,
            )));

        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos,
            kind,
            carrier: ZhenfaCarrierKind::CommonStone,
            qi_invest_ratio: 1.0,
            trigger: None,
            item_instance_id: request_item_instance_id,
            target_face: request_face,
            requested_at_tick: 10,
        });
        app.update();

        assert!(
            app.world()
                .resource::<ZhenfaRegistry>()
                .find_at(pos)
                .is_none(),
            "{kind:?} placement must reject {case_label} before creating a registry entry"
        );
        assert_eq!(
            app.world().get::<Cultivation>(owner).unwrap().qi_current,
            100.0,
            "{kind:?} rejected placement for {case_label} must not debit qi"
        );
        let inventory = app.world().get::<PlayerInventory>(owner).unwrap();
        assert!(
            inventory_item_by_instance_borrow(inventory, inventory_item_instance_id).is_some(),
            "{kind:?} rejected placement for {case_label} must keep the trap item in inventory"
        );
    }

    #[test]
    fn beast_trap_snaps_low_tier_beast_once_and_emits_status_damage_vfx() {
        let mut app = app_with_loaded_zhenfa();
        app.add_event::<VfxEventRequest>();
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        app.world_mut()
            .entity_mut(owner)
            .insert(ordinary_trap_inventory(trap_item(
                9401,
                trap_content::BEAST_TRAP_ITEM_ID,
                trap_content::BEAST_TRAP_ITEM_ID,
            )));
        let beast = app
            .world_mut()
            .spawn((
                Position::new([1.5, 64.0, 1.5]),
                Wounds::default(),
                FaunaTag::new(BeastKind::Spider),
            ))
            .id();

        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos: [1, 64, 1],
            kind: ZhenfaKind::BeastTrap,
            carrier: ZhenfaCarrierKind::CommonStone,
            qi_invest_ratio: 1.0,
            trigger: None,
            item_instance_id: Some(9401),
            target_face: Some(trap_content::TrapTargetFace::North),
            requested_at_tick: 10,
        });
        app.update();
        app.world_mut().resource_mut::<CombatClock>().tick = 11;
        app.update();

        assert!(
            app.world()
                .resource::<ZhenfaRegistry>()
                .find_at([1, 64, 1])
                .is_none(),
            "P1 BeastTrap must consume itself after the first valid low-tier beast snap"
        );
        let status_events = app
            .world()
            .resource::<Events<ApplyStatusEffectIntent>>()
            .iter_current_update_events()
            .collect::<Vec<_>>();
        assert_eq!(status_events.len(), 1);
        assert_eq!(status_events[0].target, beast);
        assert_eq!(status_events[0].kind, StatusEffectKind::Immobilized);
        assert_eq!(
            status_events[0].duration_ticks, BEAST_TRAP_IMMOBILIZED_TICKS,
            "BeastTrap immobilize duration must stay pinned to the P1 8s contract"
        );
        let wounds = app.world().get::<Wounds>(beast).unwrap();
        assert_eq!(
            wounds.health_current,
            wounds.health_max - BEAST_TRAP_SNAP_DAMAGE
        );
        assert_eq!(wounds.entries.len(), 1);
        assert_eq!(wounds.entries[0].kind, WoundKind::Pierce);
        assert_eq!(wounds.entries[0].severity, BEAST_TRAP_SNAP_DAMAGE);
        let has_snap_vfx = app
            .world()
            .resource::<Events<VfxEventRequest>>()
            .iter_current_update_events()
            .any(|event| {
                matches!(
                    &event.payload,
                    crate::schema::vfx_event::VfxEventPayloadV1::SpawnParticle { event_id, .. }
                        if event_id == gameplay_vfx::BEAST_TRAP_SNAP
                )
            });
        assert!(
            has_snap_vfx,
            "BeastTrap snap must emit its dedicated VFX id"
        );

        app.world_mut().resource_mut::<CombatClock>().tick = 12;
        app.update();
        let wounds_after_second_tick = app.world().get::<Wounds>(beast).unwrap();
        assert_eq!(
            wounds_after_second_tick.entries.len(),
            1,
            "consumed BeastTrap must not reapply wound entries on later ticks"
        );
    }


    #[test]
    fn trip_wire_alerts_owner_with_throttle_vfx_and_no_damage() {
        let mut app = app_with_loaded_zhenfa();
        app.add_event::<VfxEventRequest>();
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        let intruder = spawn_player(&mut app, "Bob", [1.5, 64.0, 1.5]);
        let health_before = app.world().get::<Wounds>(intruder).unwrap().health_current;
        app.world_mut()
            .entity_mut(owner)
            .insert(ordinary_trap_inventory(trap_item(
                9403,
                trap_content::TRIP_WIRE_ITEM_ID,
                trap_content::TRIP_WIRE_ITEM_ID,
            )));

        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos: [1, 64, 1],
            kind: ZhenfaKind::TripWire,
            carrier: ZhenfaCarrierKind::CommonStone,
            qi_invest_ratio: 1.0,
            trigger: None,
            item_instance_id: Some(9403),
            target_face: Some(trap_content::TrapTargetFace::North),
            requested_at_tick: 10,
        });
        app.update();
        app.world_mut().resource_mut::<CombatClock>().tick = 11;
        app.update();

        assert!(
            app.world()
                .resource::<ZhenfaRegistry>()
                .find_at([1, 64, 1])
                .is_some(),
            "TripWire is an alarm and must not consume itself on trigger"
        );
        let narrations = app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();
        assert_eq!(narrations.len(), 1);
        assert!(
            narrations[0].text.contains("绊线"),
            "TripWire owner narration should explicitly identify the line alarm"
        );
        assert!(
            app.world()
                .resource::<Events<ZhenfaSensePulse>>()
                .iter_current_update_events()
                .any(|pulse| pulse.owner == owner && pulse.kind == SenseKindV1::ZhenfaWardAlert),
            "TripWire must reuse ward-alert sense pulse semantics"
        );
        assert!(
            app.world()
                .resource::<Events<VfxEventRequest>>()
                .iter_current_update_events()
                .any(|event| {
                    matches!(
                        &event.payload,
                        crate::schema::vfx_event::VfxEventPayloadV1::SpawnParticle { event_id, .. }
                            if event_id == gameplay_vfx::TRIP_WIRE_TRIGGER
                    )
                }),
            "TripWire must emit a distinct VFX id so client can play the trip-wire SFX"
        );
        assert_eq!(
            app.world().get::<Wounds>(intruder).unwrap().health_current,
            health_before,
            "TripWire alarm must not damage intruders"
        );
        assert!(
            app.world()
                .resource::<Events<ApplyStatusEffectIntent>>()
                .iter_current_update_events()
                .next()
                .is_none(),
            "TripWire alarm must not apply combat status effects"
        );

        app.world_mut().resource_mut::<CombatClock>().tick = 12;
        app.update();
        assert!(
            app.world_mut()
                .resource_mut::<PendingGameplayNarrations>()
                .drain()
                .is_empty(),
            "TripWire must throttle repeat alerts while the same intruder remains inside"
        );

        app.world_mut().resource_mut::<CombatClock>().tick = 11 + WARD_ALERT_THROTTLE_TICKS;
        app.update();
        assert_eq!(
            app.world_mut()
                .resource_mut::<PendingGameplayNarrations>()
                .drain()
                .len(),
            1,
            "TripWire should alert again after the ward throttle window elapses"
        );
    }


    fn send_bait_attack(app: &mut App, attacker: Entity, target: Entity, tick: u64) {
        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: tick,
            reach: crate::combat::events::FIST_REACH,
            qi_invest: 0.0,
            wound_kind: WoundKind::Blunt,
            source: crate::combat::events::AttackSource::Melee,
            debug_command: None,
        });
    }

    #[test]
    fn bait_stake_places_decoy_component_and_taunt_pulse() {
        let mut app = app_with_loaded_zhenfa();
        app.add_event::<VfxEventRequest>();
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        app.world_mut()
            .entity_mut(owner)
            .insert(ordinary_trap_inventory(trap_item(
                9405,
                trap_content::BAIT_STAKE_ITEM_ID,
                trap_content::BAIT_STAKE_ITEM_ID,
            )));

        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos: [1, 64, 1],
            kind: ZhenfaKind::DecoyStake,
            carrier: ZhenfaCarrierKind::CommonStone,
            qi_invest_ratio: 1.0,
            trigger: None,
            item_instance_id: Some(9405),
            target_face: Some(trap_content::TrapTargetFace::Top),
            requested_at_tick: 10,
        });
        app.update();

        let instance = app
            .world()
            .resource::<ZhenfaRegistry>()
            .find_at([1, 64, 1])
            .expect("bait_stake should create a DecoyStake zhenfa instance");
        let anchor = instance.anchor_entity;
        assert_eq!(
            app.world()
                .get::<DecoyTarget>(anchor)
                .map(|target| target.0),
            Some(owner),
            "bait_stake anchor must expose DecoyTarget(owner) for NPC blackboard scans"
        );
        assert_eq!(
            app.world()
                .get::<BaitDurability>(anchor)
                .map(|durability| durability.remaining_hits),
            Some(BAIT_STAKE_DURABILITY_HITS),
            "bait_stake must start with the P2 four-hit durability contract"
        );

        app.world_mut().resource_mut::<CombatClock>().tick = 20;
        app.update();
        assert!(
            app.world()
                .resource::<Events<VfxEventRequest>>()
                .iter_current_update_events()
                .any(|event| {
                    matches!(
                        &event.payload,
                        crate::schema::vfx_event::VfxEventPayloadV1::SpawnParticle { event_id, .. }
                            if event_id == gameplay_vfx::DECOY_TAUNT
                    )
                }),
            "active bait_stake must emit the decoy_taunt pulse for client top-marker VFX"
        );
    }



    #[test]
    fn beast_target_filter_uses_fauna_tag_low_tier_set_and_excludes_owner() {
        let mut app = app_with_zhenfa();
        let owner = app.world_mut().spawn_empty().id();
        let non_owner = app.world_mut().spawn_empty().id();

        for kind in [
            BeastKind::Rat,
            BeastKind::Spider,
            BeastKind::GreenSpider,
            BeastKind::JungleScorpion,
            BeastKind::CockadeSnake,
        ] {
            let tag = FaunaTag::new(kind);
            assert!(
                is_beast_target(&tag),
                "{kind:?} is in the P0 mundane beast-trap target set"
            );
            assert!(
                is_non_owner_beast_target(non_owner, owner, Some(&tag)),
                "{kind:?} non-owner with FaunaTag should pass beast-trap targeting"
            );
            assert!(
                !is_non_owner_beast_target(owner, owner, Some(&tag)),
                "{kind:?} owner must not trigger their own beast trap"
            );
        }

        for kind in [
            BeastKind::BlueSpider,
            BeastKind::IceScorpion,
            BeastKind::MandrakeSnake,
            BeastKind::HybridBeast,
            BeastKind::LivingPillar,
            BeastKind::Whale,
        ] {
            assert!(
                !is_beast_target(&FaunaTag::new(kind)),
                "{kind:?} is outside the P0 mundane beast-trap target set"
            );
        }
        assert!(
            !is_non_owner_beast_target(non_owner, owner, None),
            "entities without FaunaTag, including players and mundane mobs, must not be beast targets"
        );
    }














    #[test]
    fn ward_alert_fires_on_entry_for_position_only_entities() {
        let mut app = app_with_loaded_zhenfa();
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        let intruder = app.world_mut().spawn(Position::new([4.5, 64.0, 0.5])).id();
        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos: [0, 64, 0],
            kind: ZhenfaKind::Ward,
            carrier: ZhenfaCarrierKind::LingqiBlock,
            qi_invest_ratio: 0.20,
            trigger: None,
            item_instance_id: None,
            target_face: None,
            requested_at_tick: 1,
        });
        app.update();
        app.world_mut().resource_mut::<CombatClock>().tick = 2;
        app.update();
        app.world_mut().resource_mut::<CombatClock>().tick = 3;
        app.update();

        let narrations = app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();
        assert_eq!(narrations.len(), 1);
        assert_eq!(narrations[0].target.as_deref(), Some("offline:Alice"));

        app.world_mut().resource_mut::<CombatClock>().tick =
            WARD_ALERT_THROTTLE_TICKS.saturating_add(5);
        app.update();
        assert!(app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain()
            .is_empty());

        app.world_mut()
            .entity_mut(intruder)
            .insert(Position::new([30.0, 64.0, 0.0]));
        app.world_mut().resource_mut::<CombatClock>().tick =
            WARD_ALERT_THROTTLE_TICKS.saturating_add(6);
        app.update();
        assert!(app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain()
            .is_empty());

        app.world_mut()
            .entity_mut(intruder)
            .insert(Position::new([4.5, 64.0, 0.5]));
        app.world_mut().resource_mut::<CombatClock>().tick =
            WARD_ALERT_THROTTLE_TICKS.saturating_add(7);
        app.update();
        assert_eq!(
            app.world_mut()
                .resource_mut::<PendingGameplayNarrations>()
                .drain()
                .len(),
            1
        );
    }


    #[test]
    fn expert_disarm_grants_scattered_qi_pearl() {
        let mut app = app_with_loaded_zhenfa();
        app.insert_resource(pearl_registry());
        app.insert_resource(InventoryInstanceIdAllocator::default());
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        let breaker = spawn_player(&mut app, "Bob", [1.5, 64.0, 1.5]);
        app.world_mut().entity_mut(breaker).insert((
            InsightModifiers {
                zhenfa_disenchant: 5.0,
                ..InsightModifiers::new()
            },
            empty_inventory(),
        ));
        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos: [1, 64, 1],
            kind: ZhenfaKind::Trap,
            carrier: ZhenfaCarrierKind::LingqiBlock,
            qi_invest_ratio: 0.10,
            trigger: None,
            item_instance_id: None,
            target_face: None,
            requested_at_tick: 1,
        });
        app.update();

        let anchor_entity = app
            .world()
            .resource::<ZhenfaRegistry>()
            .find_at([1, 64, 1])
            .unwrap()
            .anchor_entity;
        app.world_mut().send_event(ZhenfaDisarmRequest {
            player: breaker,
            pos: [1, 64, 1],
            mode: ZhenfaDisarmMode::Disarm,
            requested_at_tick: 2,
        });
        app.update();

        let inventory = app.world().get::<PlayerInventory>(breaker).unwrap();
        let main_pack = inventory
            .containers
            .iter()
            .find(|container| container.id == crate::inventory::MAIN_PACK_CONTAINER_ID)
            .unwrap();
        assert_eq!(main_pack.items.len(), 1);
        assert_eq!(
            main_pack.items[0].instance.template_id,
            ZHENFA_PEARL_ITEM_ID
        );
        assert!(app.world().get_entity(anchor_entity).is_none());
    }

    #[test]
    fn helper_ranges_follow_plan_thresholds() {
        assert_eq!(trap_effect_radius(0.10), 0);
        assert_eq!(trap_effect_radius(0.20), 1);
        assert_eq!(trap_effect_radius(0.50), 2);
        assert_eq!(
            ward_radius(ZhenfaKind::Ward, 0.20, 8, ZhenfaSpecialistLevel::None),
            4
        );
        assert_eq!(
            ward_radius(ZhenfaKind::Ward, 0.20, 8, ZhenfaSpecialistLevel::Expert),
            8
        );
        assert!(in_horizontal_radius(
            DVec3::new(1.5, 64.0, 1.5),
            [1, 64, 1],
            0
        ));
    }





    #[test]
    fn deceive_heaven_exposure_emits_dedicated_event() {
        let mut app = app_with_loaded_zhenfa();
        let owner = spawn_player(&mut app, "Alice", [0.0, 64.0, 0.0]);
        app.world_mut().entity_mut(owner).insert(Cultivation {
            realm: Realm::Solidify,
            qi_current: 100.0,
            qi_max: 100.0,
            ..Default::default()
        });
        app.world_mut()
            .entity_mut(owner)
            .insert(deceive_heaven_material_inventory());
        let exposed_id = (1..=1_000)
            .find(|id| {
                deceive_heaven_reveal_tick(
                    *id,
                    1,
                    1 + DECEIVE_HEAVEN_DURATION_TICKS,
                    Realm::Solidify,
                )
                .is_some()
            })
            .expect("test id window should contain at least one exposed array");
        app.world_mut().resource_mut::<ZhenfaRegistry>().next_id = exposed_id - 1;
        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos: [0, 64, 0],
            kind: ZhenfaKind::DeceiveHeaven,
            carrier: ZhenfaCarrierKind::BeastCoreInlaid,
            qi_invest_ratio: 0.90,
            trigger: None,
            item_instance_id: None,
            target_face: None,
            requested_at_tick: 1,
        });
        app.update();

        let instance = app
            .world()
            .resource::<ZhenfaRegistry>()
            .find_at([0, 64, 0])
            .unwrap()
            .clone();
        let exposure_tick = deceive_heaven_reveal_tick(
            instance.id,
            instance.placed_at_tick,
            instance.expires_at_tick,
            instance.realm_at_cast,
        )
        .expect("selected array id should expose during its lifecycle");
        app.world_mut().resource_mut::<CombatClock>().tick = exposure_tick;
        app.update();

        assert!(app
            .world()
            .resource::<ZhenfaRegistry>()
            .find_at([0, 64, 0])
            .is_none());
        assert!(!app
            .world()
            .resource::<Events<DeceiveHeavenExposedEvent>>()
            .is_empty());
        assert!(app
            .world()
            .resource::<Events<JueBiTriggerEvent>>()
            .iter_current_update_events()
            .any(|event| event.source == JueBiTriggerSource::ZhenfaDeceptionExposed));
        assert!(
            (released_zhenfa_qi_total(app.world().resource::<Events<QiTransfer>>()) - 90.0).abs()
                < f64::EPSILON
        );
    }

    #[test]
    fn deceive_heaven_detection_is_lifecycle_ten_percent_not_per_tick() {
        assert_eq!(
            deceive_heaven_reveal_chance(Realm::Solidify),
            DECEIVE_HEAVEN_REVEAL_CHANCE
        );
        assert_eq!(
            deceive_heaven_reveal_chance(Realm::Void),
            DECEIVE_HEAVEN_REVEAL_CHANCE
        );

        let exposed_count = (1..=1_000)
            .filter(|id| {
                deceive_heaven_reveal_tick(
                    *id,
                    1,
                    1 + DECEIVE_HEAVEN_DURATION_TICKS,
                    Realm::Solidify,
                )
                .is_some()
            })
            .count();

        assert!(
            (80..=120).contains(&exposed_count),
            "deterministic lifecycle exposure should approximate 10%, actual={exposed_count}/1000"
        );
    }



    #[test]
    fn zhenfa_v2_profiles_encode_plan_thresholds() {
        let lingju = zhenfa_kind_profile(
            ZhenfaKind::Lingju,
            Realm::Void,
            100.0,
            ZhenfaCarrierKind::BeastCoreInlaid,
        );
        assert_eq!(lingju.radius, 60);
        assert_eq!(lingju.density_multiplier, 3.5);
        assert!(lingju.duration_ticks > 6 * 60 * 60 * TICKS_PER_SECOND);

        let deceive = zhenfa_kind_profile(
            ZhenfaKind::DeceiveHeaven,
            Realm::Void,
            0.0,
            ZhenfaCarrierKind::BeastCoreInlaid,
        );
        assert_eq!(deceive.min_invest_ratio, 0.80);
        assert_eq!(deceive.duration_ticks, DECEIVE_HEAVEN_DURATION_TICKS);
        assert_eq!(deceive.reveal_chance, DECEIVE_HEAVEN_REVEAL_CHANCE);
    }


    #[test]
    fn zhenfa_anchor_visual_state_reflects_registry_lifecycle() {
        let mut registry = ZhenfaRegistry::default();
        assert_eq!(
            registry.anchor_visual_state(&ZhenfaAnchor { id: 0 }),
            ZHENFA_VISUAL_STATE_INACTIVE
        );

        let id = registry
            .insert(ZhenfaInstance {
                id: 0,
                kind: ZhenfaKind::Trap,
                owner: Entity::from_raw(1),
                owner_player_id: "offline:Azure".to_string(),
                pos: [1, 64, 1],
                carrier: ZhenfaCarrierKind::LingqiBlock,
                qi_invest_ratio: 0.5,
                qi_invest_amount: 25.0,
                realm_at_cast: Realm::Induce,
                mastery_at_cast: 0.0,
                effect_radius: 2,
                ward_radius: 8,
                placed_at_tick: 1,
                expires_at_tick: 100,
                triggered_at: None,
                trigger: None,
                color_main: ColorKind::Intricate,
                color_secondary: Some(ColorKind::Solid),
                anchor_entity: Entity::from_raw(2),
            })
            .unwrap();
        assert_eq!(
            registry.anchor_visual_state(&ZhenfaAnchor { id }),
            ZHENFA_VISUAL_STATE_ACTIVE
        );

        registry
            .pending_chain
            .push_back(PendingChainTrigger { id, due_tick: 8 });
        assert_eq!(
            registry.anchor_visual_state(&ZhenfaAnchor { id }),
            ZHENFA_VISUAL_STATE_EXHAUSTED
        );
        assert_eq!(
            registry.anchor_visual_state(&ZhenfaAnchor { id: 999 }),
            ZHENFA_VISUAL_STATE_EXHAUSTED
        );
    }

    // P1: 杂色 guard — effective_duration_ticks & active_trigger_range --------

    fn cultivation_for_realm(realm: Realm) -> Cultivation {
        Cultivation {
            realm,
            qi_current: 100.0,
            qi_max: 100.0,
            ..Default::default()
        }
    }

    #[test]
    fn effective_duration_no_color_bonus_when_chaotic() {
        // 期望: 杂色时 Solid 主色不提供 2x 延时加成（worldview §六.2「只剩基础真元属性」）
        let base_ticks: u64 = 200;
        let chaotic = QiColor {
            main: ColorKind::Solid,
            is_chaotic: true,
            ..Default::default()
        };
        let ticks_chaotic =
            effective_duration_ticks(base_ticks, &chaotic, ZhenfaSpecialistLevel::Expert);
        assert_eq!(
            ticks_chaotic, base_ticks,
            "期望杂色时 effective_duration_ticks={base_ticks}（无 2x 加成），实际={ticks_chaotic}"
        );
    }

    #[test]
    fn effective_duration_solid_color_bonus_when_non_chaotic() {
        // 期望: 正常 Solid 主色时 Expert 级别延时翻倍 (base * 2.0)
        let base_ticks: u64 = 200;
        let solid = QiColor {
            main: ColorKind::Solid,
            ..Default::default() // is_chaotic=false by default
        };
        let ticks = effective_duration_ticks(base_ticks, &solid, ZhenfaSpecialistLevel::Expert);
        assert_eq!(
            ticks,
            base_ticks * 2,
            "期望正常 Solid 色 Expert 级别延时={expected}（2x 加成），实际={ticks}",
            expected = base_ticks * 2
        );
    }

    #[test]
    fn active_trigger_range_no_bonus_when_chaotic() {
        // 期望: 杂色时 Intricate 主色不提供 1.5x 范围加成（worldview §六.2「只剩基础真元属性」）
        let cultivation = cultivation_for_realm(Realm::Condense);
        let chaotic = QiColor {
            main: ColorKind::Intricate,
            is_chaotic: true,
            ..Default::default()
        };
        let normal = QiColor {
            main: ColorKind::Intricate,
            ..Default::default() // is_chaotic=false
        };
        let range_chaotic = active_trigger_range(&cultivation, &chaotic);
        let range_normal = active_trigger_range(&cultivation, &normal);
        assert!(
            range_chaotic < range_normal,
            "期望杂色时 active_trigger_range={range_chaotic} < 正常 Intricate 色 range={range_normal}（杂色无 1.5x 加成）"
        );
    }

    #[test]
    fn active_trigger_range_intricate_bonus_when_non_chaotic() {
        // 期望: 正常 Intricate 主色提供 1.5x 范围加成
        let cultivation = cultivation_for_realm(Realm::Condense);
        let intricate = QiColor {
            main: ColorKind::Intricate,
            ..Default::default() // is_chaotic=false
        };
        let mellow = QiColor::default(); // default non-Intricate
        let base_range = active_trigger_range(&cultivation, &mellow);
        let bonus_range = active_trigger_range(&cultivation, &intricate);
        assert!(
            (bonus_range - base_range * 1.5).abs() < 0.1,
            "期望正常 Intricate 色 active_trigger_range={bonus_range} ≈ base*1.5={expected}",
            expected = base_range * 1.5
        );
    }

    // ── QS-02 fix: Ward/ShrineWard/Lingju/Illusion/Trap deploy & expiry must emit QiTransfer ─

    /// Ward 放置必须发 Channeling QiTransfer，将真元从玩家账户密封进阵法容器。
    /// 修复前 should_release_sealed_qi_to_zone 把 Ward 排除在外，
    /// 导致 qi_current 扣减但守恒账本不挂账，真元凭空消失。
    #[test]
    fn ward_deploy_seals_qi_into_zhenfa_container() {
        let mut app = app_with_loaded_zhenfa();
        // 不需要 ZoneRegistry：deploy 只做 seal（player→container），不涉及 zone
        let owner = spawn_player(&mut app, "Alice", [0.5, 64.0, 0.5]);

        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos: [0, 64, 0],
            kind: ZhenfaKind::Ward,
            carrier: ZhenfaCarrierKind::LingqiBlock,
            qi_invest_ratio: 0.20,
            trigger: None,
            item_instance_id: None,
            target_face: None,
            requested_at_tick: 1,
        });
        app.update();

        let registry = app.world().resource::<ZhenfaRegistry>();
        let instance = registry
            .find_at([0, 64, 0])
            .expect("Ward placement must succeed so the qi-seal transfer can be verified");
        // qi_cost = max(min_invest_ratio, requested_ratio) * qi_max = 0.20 * 100.0 = 20.0
        let expected_cost = instance.qi_invest_amount;
        assert!(
            expected_cost > f64::EPSILON,
            "Ward qi_invest_amount must be nonzero; got {expected_cost}"
        );
        let expected_container = zhenfa_sealed_qi_account(&instance.owner_player_id, instance.id);
        let transfers = app.world().resource::<Events<QiTransfer>>();
        let seal = transfers
            .iter_current_update_events()
            .find(|t| {
                t.reason == QiTransferReason::Channeling && t.to == expected_container
            })
            .expect(
                "Ward deploy must emit a Channeling QiTransfer sealing qi into the zhenfa container; \
                 before fix, should_release_sealed_qi_to_zone excluded Ward so no transfer was emitted"
            );
        assert_eq!(
            seal.from,
            QiAccountId::player(instance.owner_player_id.clone()),
            "seal transfer 'from' must be the caster's player account"
        );
        assert!(
            (seal.amount - expected_cost).abs() < f64::EPSILON,
            "seal transfer amount must equal qi_invest_amount={expected_cost}; got {}",
            seal.amount
        );
    }

    /// Illusion 放置必须发 Channeling QiTransfer。Illusion min_invest_ratio=0.10，
    /// 修复前与 Ward 相同缺陷：qi_current 扣减无账本挂账。
    #[test]
    fn illusion_deploy_seals_qi_into_zhenfa_container() {
        let mut app = app_with_loaded_zhenfa();
        let owner = spawn_player(&mut app, "Alice", [0.5, 64.0, 0.5]);

        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos: [0, 64, 0],
            kind: ZhenfaKind::Illusion,
            carrier: ZhenfaCarrierKind::LingqiBlock,
            qi_invest_ratio: 0.10,
            trigger: None,
            item_instance_id: None,
            target_face: None,
            requested_at_tick: 1,
        });
        app.update();

        let registry = app.world().resource::<ZhenfaRegistry>();
        let instance = registry
            .find_at([0, 64, 0])
            .expect("Illusion placement must succeed");
        let expected_cost = instance.qi_invest_amount;
        assert!(
            expected_cost > f64::EPSILON,
            "Illusion qi_invest_amount must be nonzero; got {expected_cost}"
        );
        let expected_container = zhenfa_sealed_qi_account(&instance.owner_player_id, instance.id);
        let transfers = app.world().resource::<Events<QiTransfer>>();
        let seal = transfers
            .iter_current_update_events()
            .find(|t| t.reason == QiTransferReason::Channeling && t.to == expected_container)
            .expect(
                "Illusion deploy must emit Channeling QiTransfer; \
                 before fix, should_release_sealed_qi_to_zone excluded Illusion",
            );
        assert!(
            (seal.amount - expected_cost).abs() < f64::EPSILON,
            "seal amount must equal qi_invest_amount={expected_cost}; got {}",
            seal.amount
        );
    }

    /// Ward 到期后必须将密封真元释放回区域，守恒账本完整。
    /// zone spirit_qi 先清零，避免 fallback 默认 0.9 导致 overflow 分叉使金额断言失败。
    #[test]
    fn ward_expiry_releases_sealed_qi_to_zone() {
        let mut app = app_with_loaded_zhenfa();
        // spirit_qi=0.0 → zone 有足够容量接收 Ward 密封真元，断言 total==cost 成立
        let mut zones = ZoneRegistry::fallback();
        zones.zones[0].spirit_qi = 0.0;
        app.insert_resource(zones);

        let owner = spawn_player(&mut app, "Alice", [0.5, 64.0, 0.5]);
        let qi_invest_ratio = 0.20_f64;

        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos: [0, 64, 0],
            kind: ZhenfaKind::Ward,
            carrier: ZhenfaCarrierKind::LingqiBlock,
            qi_invest_ratio,
            trigger: None,
            item_instance_id: None,
            target_face: None,
            requested_at_tick: 1,
        });
        app.update();

        let (expires_at_tick, qi_invest_amount, instance_id, owner_player_id) = {
            let reg = app.world().resource::<ZhenfaRegistry>();
            let inst = reg
                .find_at([0, 64, 0])
                .expect("Ward should be placed before expiry test");
            (
                inst.expires_at_tick,
                inst.qi_invest_amount,
                inst.id,
                inst.owner_player_id.clone(),
            )
        };
        assert!(
            qi_invest_amount > f64::EPSILON,
            "pre-condition: Ward qi_invest_amount must be nonzero for expiry release test"
        );

        // 推进到到期 tick，触发 tick_zhenfa_registry 清理
        app.world_mut().resource_mut::<CombatClock>().tick = expires_at_tick;
        app.update();

        // Ward 已从 registry 移除
        assert!(
            app.world()
                .resource::<ZhenfaRegistry>()
                .find_at([0, 64, 0])
                .is_none(),
            "Ward must be removed from registry after expiry"
        );

        // 必须能看到 ReleaseToZone 转账从密封容器流向区域
        let container = zhenfa_sealed_qi_account(&owner_player_id, instance_id);
        let transfers = app.world().resource::<Events<QiTransfer>>();
        let released: f64 = transfers
            .iter_current_update_events()
            .filter(|t| t.from == container)
            .map(|t| t.amount)
            .sum();
        assert!(
            (released - qi_invest_amount).abs() < f64::EPSILON,
            "Ward expiry must release full qi_invest_amount={qi_invest_amount} from container to zone; \
             before fix, should_release_sealed_qi_to_zone excluded Ward so no release was emitted; \
             actual total released={released}"
        );
    }

    /// Lingju 到期后必须释放密封真元。Lingju min_invest_ratio=0.30，是区域控制核心机制。
    /// spirit_qi=0.0 以避免 fallback 容量不足导致 overflow 分叉。
    #[test]
    fn lingju_expiry_releases_sealed_qi_to_zone() {
        let mut app = app_with_loaded_zhenfa();
        let mut zones = ZoneRegistry::fallback();
        zones.zones[0].spirit_qi = 0.0;
        app.insert_resource(zones);
        spawn_plot(&mut app, [0, 64, 0], PLOT_QI_CAP_BASE);

        let owner = spawn_player(&mut app, "Alice", [0.5, 64.0, 0.5]);

        send_lingju_place(&mut app, owner, [0, 64, 0], 1);
        app.update();

        let (expires_at_tick, qi_invest_amount, instance_id, owner_player_id) = {
            let reg = app.world().resource::<ZhenfaRegistry>();
            let inst = reg
                .find_at([0, 64, 0])
                .expect("Lingju should be placed before expiry test");
            (
                inst.expires_at_tick,
                inst.qi_invest_amount,
                inst.id,
                inst.owner_player_id.clone(),
            )
        };
        assert!(
            qi_invest_amount > f64::EPSILON,
            "pre-condition: Lingju qi_invest_amount must be nonzero (min_invest_ratio=0.30)"
        );

        app.world_mut().resource_mut::<CombatClock>().tick = expires_at_tick;
        app.update();

        assert!(
            app.world()
                .resource::<ZhenfaRegistry>()
                .find_at([0, 64, 0])
                .is_none(),
            "Lingju must be removed from registry after expiry"
        );

        let container = zhenfa_sealed_qi_account(&owner_player_id, instance_id);
        let transfers = app.world().resource::<Events<QiTransfer>>();
        let released: f64 = transfers
            .iter_current_update_events()
            .filter(|t| t.from == container)
            .map(|t| t.amount)
            .sum();
        assert!(
            (released - qi_invest_amount).abs() < f64::EPSILON,
            "Lingju expiry must release full qi_invest_amount={qi_invest_amount} to zone; \
             before fix, should_release_sealed_qi_to_zone excluded Lingju; \
             actual total released={released}"
        );
    }

    /// 当区域满载时，Ward 到期释放真元必须路由到 overflow 账户（非凭空消失）。
    /// 这是边界分支：zone 容量 = 0（spirit_qi = 1.0），qi_cost 全进 overflow。
    #[test]
    fn ward_expiry_qi_routes_to_overflow_when_zone_full() {
        let mut app = app_with_loaded_zhenfa();
        // spirit_qi=1.0 → zone 已满，release 全部路由到 overflow
        let mut zones = ZoneRegistry::fallback();
        zones.zones[0].spirit_qi = 1.0;
        app.insert_resource(zones);

        let owner = spawn_player(&mut app, "Alice", [0.5, 64.0, 0.5]);

        app.world_mut().send_event(ZhenfaPlaceRequest {
            player: owner,
            pos: [0, 64, 0],
            kind: ZhenfaKind::Ward,
            carrier: ZhenfaCarrierKind::LingqiBlock,
            qi_invest_ratio: 0.20,
            trigger: None,
            item_instance_id: None,
            target_face: None,
            requested_at_tick: 1,
        });
        app.update();

        let (expires_at_tick, qi_invest_amount, instance_id, owner_player_id) = {
            let reg = app.world().resource::<ZhenfaRegistry>();
            let inst = reg
                .find_at([0, 64, 0])
                .expect("Ward should be placed before overflow expiry test");
            (
                inst.expires_at_tick,
                inst.qi_invest_amount,
                inst.id,
                inst.owner_player_id.clone(),
            )
        };

        app.world_mut().resource_mut::<CombatClock>().tick = expires_at_tick;
        app.update();

        // 区域 spirit_qi 不变（已满不能再增）
        let zone_qi = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name(crate::world::zone::DEFAULT_SPAWN_ZONE_NAME)
            .expect("spawn zone must exist")
            .spirit_qi;
        assert!(
            (zone_qi - 1.0).abs() < 1e-9,
            "full zone spirit_qi must remain 1.0 after Ward expiry; got {zone_qi}"
        );

        // overflow 账户应接收全部真元（守恒不丢失）
        let container = zhenfa_sealed_qi_account(&owner_player_id, instance_id);
        let transfers = app.world().resource::<Events<QiTransfer>>();
        let overflow_total: f64 = transfers
            .iter_current_update_events()
            .filter(|t| {
                t.from == container && t.to.kind == crate::qi_physics::QiAccountKind::Overflow
            })
            .map(|t| t.amount)
            .sum();
        assert!(
            (overflow_total - qi_invest_amount).abs() < f64::EPSILON,
            "when zone is full, Ward expiry must route full qi_invest_amount={qi_invest_amount} \
             to overflow (not disappear); actual overflow={overflow_total}"
        );
    }