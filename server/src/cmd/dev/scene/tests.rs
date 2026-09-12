use super::*;

use crate::cmd::completions::{answer_command_completions, mark_ask_server_arguments};
use crate::combat::components::DerivedAttrs;
use crate::combat::events::ApplyStatusEffectIntent;
use crate::combat::CombatClock;
use crate::cultivation::known_techniques::TechniqueRegistry;
use crate::inventory::ItemRegistry;
use valence::prelude::{Entity, Events, IntoSystemSetConfigs, PostStartup};
use valence::protocol::packets::play::{
    CommandExecutionC2s, CommandSuggestionsS2c, CustomPayloadS2c, RequestCommandCompletionsC2s,
};
use valence::protocol::{Bounded, FixedBitSet, VarInt};
use valence::testing::{create_mock_client, MockClientHelper};

fn setup(test_env: bool) -> (App, Entity, MockClientHelper) {
    let mut app = App::new();
    app.add_plugins((
        valence::event_loop::EventLoopPlugin,
        valence::command::manager::CommandPlugin,
    ));
    register(&mut app, test_env);
    super::super::register_operator_gate(&mut app);
    app.insert_resource(DevCommandPermissions::allow_user("Builder"));
    app.insert_resource(CombatClock { tick: 1 });
    app.insert_resource(ItemRegistry::from_map(Default::default()));
    app.insert_resource(TechniqueRegistry::load_for_tests());
    app.add_event::<ApplyStatusEffectIntent>();
    app.configure_sets(
        Update,
        (CombatSystemSet::Intent, CombatSystemSet::Physics).chain(),
    );
    app.add_systems(
        Update,
        status::status_effect_apply_tick.in_set(CombatSystemSet::Intent),
    );
    app.add_systems(
        Update,
        (
            status::status_effect_tick,
            status::attribute_aggregate_tick,
            emit_status_snapshot_payloads,
        )
            .chain()
            .in_set(CombatSystemSet::Physics),
    );
    app.add_systems(PostStartup, mark_ask_server_arguments);
    app.add_systems(
        valence::prelude::EventLoopPreUpdate,
        answer_command_completions,
    );
    let (bundle, helper) = create_mock_client("Builder");
    let entity = app
        .world_mut()
        .spawn((
            bundle,
            StatusEffects::default(),
            Lifecycle::default(),
            DerivedAttrs::default(),
        ))
        .id();
    app.finish();
    app.cleanup();
    app.update();
    (app, entity, helper)
}

fn execute(app: &mut App, helper: &mut MockClientHelper, command: &str) {
    helper.send(&CommandExecutionC2s {
        command: Bounded(command),
        timestamp: 0,
        salt: 0,
        argument_signatures: Vec::new(),
        message_count: VarInt(0),
        acknowledgement: FixedBitSet::default(),
    });
    app.update();
}

fn flush(app: &mut App) {
    let world = app.world_mut();
    let mut clients = world.query::<&mut Client>();
    for mut client in clients.iter_mut(world) {
        client.flush_packets().unwrap();
    }
}

fn snapshot(app: &mut App, helper: &mut MockClientHelper) -> serde_json::Value {
    flush(app);
    helper
        .collect_received()
        .0
        .into_iter()
        .filter_map(|frame| {
            let packet = frame.decode::<CustomPayloadS2c>().ok()?;
            if packet.channel.as_str() != crate::network::agent_bridge::SERVER_DATA_CHANNEL {
                return None;
            }
            let payload: serde_json::Value = serde_json::from_slice(packet.data.0 .0).ok()?;
            (payload["type"] == "status_snapshot").then_some(payload)
        })
        .last()
        .expect("场景切换或清除必须下发真实 status_snapshot")
}

#[test]
fn scene_commands_replace_only_executor_statuses_and_sync_clear_to_hud() {
    let (mut app, player, mut helper) = setup(true);
    let other = app.world_mut().spawn(StatusEffects::default()).id();
    execute(&mut app, &mut helper, "scene test_debuff_effect_combo_1");
    let payload = snapshot(&mut app, &mut helper);
    assert!(payload["effects"]
        .as_array()
        .unwrap()
        .iter()
        .any(|effect| effect["id"] == "slowed"));
    assert!(
        app.world()
            .get::<DerivedAttrs>(player)
            .unwrap()
            .move_speed_multiplier
            < 1.0,
        "场景应驱动真实减速属性"
    );
    let count = app
        .world()
        .get::<StatusEffects>(player)
        .unwrap()
        .active
        .len();
    execute(&mut app, &mut helper, "scene test_debuff_effect_combo_1");
    assert_eq!(
        app.world()
            .get::<StatusEffects>(player)
            .unwrap()
            .active
            .len(),
        count,
        "重复加载不能累加效果"
    );

    execute(&mut app, &mut helper, "scene test_buff_effect_combo_1");
    let payload = snapshot(&mut app, &mut helper);
    assert!(
        payload["effects"]
            .as_array()
            .unwrap()
            .iter()
            .all(|effect| effect["kind"] == "buff"),
        "切换场景应移除上一场景的负面状态"
    );
    assert!(
        app.world()
            .get::<DerivedAttrs>(player)
            .unwrap()
            .move_speed_multiplier
            > 1.0
    );
    assert!(
        app.world()
            .get::<StatusEffects>(other)
            .unwrap()
            .active
            .is_empty(),
        "不能改写其他玩家的状态"
    );

    execute(&mut app, &mut helper, "scene clear");
    let payload = snapshot(&mut app, &mut helper);
    assert_eq!(payload["effects"], serde_json::json!([]));
    assert_eq!(payload["cultivation_acceleration"], 1.0);
    assert_eq!(
        app.world()
            .get::<DerivedAttrs>(player)
            .unwrap()
            .move_speed_multiplier,
        1.0
    );
}

#[test]
fn pending_revival_and_non_operators_cannot_load_scenes() {
    let (mut app, player, mut helper) = setup(true);
    execute(&mut app, &mut helper, "scene test_debuff_effect_combo_1");
    let before = serde_json::to_value(app.world().get::<StatusEffects>(player).unwrap()).unwrap();
    app.world_mut().get_mut::<Lifecycle>(player).unwrap().state = LifecycleState::AwaitingRevival;
    execute(&mut app, &mut helper, "scene test_buff_effect_combo_1");
    assert_eq!(
        serde_json::to_value(app.world().get::<StatusEffects>(player).unwrap()).unwrap(),
        before,
        "复活裁决中不得加载场景"
    );

    let (bundle, mut unauthorized) = create_mock_client("Visitor");
    let visitor = app
        .world_mut()
        .spawn((bundle, StatusEffects::default(), Lifecycle::default()))
        .id();
    execute(
        &mut app,
        &mut unauthorized,
        "scene test_debuff_effect_combo_1",
    );
    assert!(
        app.world()
            .get::<StatusEffects>(visitor)
            .unwrap()
            .active
            .is_empty(),
        "非管理员不能执行场景命令"
    );
}

#[test]
fn test_env_gates_command_registration_and_scene_completions() {
    for enabled in [false, true] {
        let (mut app, player, mut helper) = setup(enabled);
        assert_eq!(
            app.world()
                .contains_resource::<Events<CommandResultEvent<SceneCmd>>>(),
            enabled,
            "关闭测试环境时不能注册场景命令事件"
        );
        execute(&mut app, &mut helper, "scene test_debuff_effect_combo_1");
        assert_eq!(
            !app.world()
                .get::<StatusEffects>(player)
                .unwrap()
                .active
                .is_empty(),
            enabled
        );
        helper.send(&RequestCommandCompletionsC2s {
            transaction_id: VarInt(42),
            text: Bounded("/scene test_debuff"),
        });
        app.update();
        flush(&mut app);
        let matches: Vec<String> = helper
            .collect_received()
            .0
            .into_iter()
            .flat_map(|frame| {
                frame
                    .decode::<CommandSuggestionsS2c>()
                    .map(|packet| {
                        packet
                            .matches
                            .iter()
                            .map(|item| item.suggested_match.to_string())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            })
            .collect();
        assert_eq!(
            matches.contains(&"test_debuff_effect_combo_1".to_string()),
            enabled,
            "只有测试环境能够补全固定场景名"
        );
    }
}

fn setup_gathering(enabled: bool) -> (App, Entity, MockClientHelper) {
    let (mut app, player, helper) = setup(enabled);
    crate::gathering::register(&mut app);
    crate::player::gameplay::register(&mut app);
    app.add_event::<crate::combat::events::CombatEvent>();
    app.add_event::<crate::combat::events::AttackIntent>();
    app.add_event::<crate::cultivation::breakthrough::BreakthroughRequest>();
    app.add_event::<crate::inventory::InventoryDurabilityChangedEvent>();
    app.add_event::<crate::network::vfx_event_emit::VfxEventRequest>();
    app.add_event::<crate::network::audio_event_emit::PlaySoundRecipeRequest>();
    (app, player, helper)
}

fn gathering_payloads(app: &mut App, helper: &mut MockClientHelper) -> Vec<serde_json::Value> {
    scene_payloads(app, helper)
        .into_iter()
        .filter(|payload| payload["type"] == "gathering_session")
        .collect()
}

fn scene_payloads(app: &mut App, helper: &mut MockClientHelper) -> Vec<serde_json::Value> {
    flush(app);
    helper
        .collect_received()
        .0
        .into_iter()
        .filter_map(|frame| {
            let packet = frame.decode::<CustomPayloadS2c>().ok()?;
            if packet.channel.as_str() != crate::network::agent_bridge::SERVER_DATA_CHANNEL {
                return None;
            }
            serde_json::from_slice(packet.data.0 .0).ok()
        })
        .collect()
}

#[test]
fn gathering_tools_are_temporary_views_and_restore_current_equipment() {
    use crate::inventory::{
        instantiate_inventory_from_loadout, load_default_loadout, load_item_registry,
        InventoryInstanceIdAllocator, PlayerInventory,
    };
    let (mut app, player, mut helper) = setup_gathering(true);
    let registry = load_item_registry().unwrap();
    let loadout = load_default_loadout(&registry).unwrap();
    let inventory = instantiate_inventory_from_loadout(
        &loadout,
        &mut InventoryInstanceIdAllocator::default(),
        &registry,
    )
    .unwrap();
    let original = serde_json::to_value(&inventory).unwrap();
    let original_hand = inventory.equipped["main_hand"]
        .held
        .as_ref()
        .unwrap()
        .template_id
        .clone();
    app.world_mut().insert_resource(registry);
    app.world_mut().entity_mut(player).insert(inventory);
    app.add_systems(
        Update,
        crate::network::weapon_equipped_emit::emit_weapon_equipped_payloads,
    );
    let hand = |payloads: Vec<serde_json::Value>| {
        payloads
            .into_iter()
            .rfind(|payload| payload["type"] == "weapon_equipped" && payload["slot"] == "main_hand")
            .unwrap()["weapon"]
            .clone()
    };
    for (scene, tool) in [
        ("scene test_gathering_ore_1", "pickaxe_iron"),
        ("scene test_gathering_wood_1", "axe_iron"),
        ("scene test_gathering_herb_1", "hoe_iron"),
    ] {
        execute(&mut app, &mut helper, scene);
        assert_eq!(
            hand(scene_payloads(&mut app, &mut helper))["template_id"],
            tool,
            "场景必须通过真实手持通道显示对应工具"
        );
        assert_eq!(
            serde_json::to_value(app.world().get::<PlayerInventory>(player).unwrap()).unwrap(),
            original,
            "测试工具不能进入真实背包、扣耐久或替换持久装备"
        );
    }
    execute(&mut app, &mut helper, "scene clear");
    assert_eq!(
        hand(scene_payloads(&mut app, &mut helper))["template_id"],
        original_hand
    );

    execute(&mut app, &mut helper, "scene test_gathering_ore_1");
    scene_payloads(&mut app, &mut helper);
    app.world_mut()
        .get_mut::<PlayerInventory>(player)
        .unwrap()
        .equipped
        .get_mut("main_hand")
        .unwrap()
        .held = None;
    app.update();
    assert_eq!(
        hand(scene_payloads(&mut app, &mut helper))["template_id"],
        "pickaxe_iron",
        "真实装备同步不能提前覆盖仍在演出的测试工具"
    );
    execute(&mut app, &mut helper, "scene clear");
    assert!(
        hand(scene_payloads(&mut app, &mut helper)).is_null(),
        "退出场景应恢复当前空手，不能复活场景开始时的旧装备"
    );
}

#[test]
fn gathering_scenes_sync_target_switch_progress_and_completion() {
    let (mut app, _player, mut helper) = setup_gathering(true);
    let mut previous_id = None;
    for (command, target) in [
        ("scene test_gathering_herb_1", "herb"),
        ("scene test_gathering_ore_1", "ore"),
        ("scene test_gathering_wood_1", "wood"),
    ] {
        execute(&mut app, &mut helper, command);
        let payloads = gathering_payloads(&mut app, &mut helper);
        if let Some(id) = previous_id {
            assert!(
                payloads
                    .iter()
                    .any(|p| p["session_id"] == id && p["interrupted"] == true),
                "切换目标前必须下发旧会话的终态，停止旧采集动画"
            );
        }
        let started = payloads
            .last()
            .expect("场景必须通过正式 gathering_session 通道同步客户端");
        assert_eq!(started["target_type"], target);
        assert_eq!(started["completed"], false);
        assert_eq!(started["interrupted"], false);
        previous_id = Some(started["session_id"].clone());
    }
    for _ in 0..60 {
        app.update();
    }
    let payloads = gathering_payloads(&mut app, &mut helper);
    assert!(
        payloads.iter().any(|p| {
            let progress = p["progress_ticks"].as_u64().unwrap();
            progress > 0 && progress < p["total_ticks"].as_u64().unwrap()
        }),
        "正式采集时钟必须持续驱动 HUD 进度"
    );
    for _ in 0..120 {
        app.update();
    }
    let payloads = gathering_payloads(&mut app, &mut helper);
    let complete = payloads
        .iter()
        .find(|p| p["completed"] == true)
        .expect("站定应自然完成");
    assert_eq!(complete["progress_ticks"], complete["total_ticks"]);
    assert_eq!(complete["interrupted"], false);
}

#[test]
fn gathering_scenes_interrupt_and_do_not_restart_after_cleanup() {
    use crate::gathering::session::{GatheringProgressFrame, GatheringSessionStore};
    for trigger in ["clear", "status_scene", "move", "death", "disconnect"] {
        let (mut app, player, mut helper) = setup_gathering(true);
        execute(&mut app, &mut helper, "scene test_gathering_herb_1");
        let session_id = app
            .world()
            .resource::<GatheringSessionStore>()
            .session_for(player)
            .unwrap()
            .session_id
            .clone();
        let mut frames = app
            .world()
            .resource::<Events<GatheringProgressFrame>>()
            .get_reader_current();
        match trigger {
            "clear" => execute(&mut app, &mut helper, "scene clear"),
            "status_scene" => execute(&mut app, &mut helper, "scene test_buff_effect_combo_1"),
            "move" => {
                app.world_mut()
                    .get_mut::<Position>(player)
                    .unwrap()
                    .set([10.0, 0.0, 0.0]);
                app.update();
            }
            "death" => {
                app.world_mut().get_mut::<Lifecycle>(player).unwrap().state =
                    LifecycleState::AwaitingRevival;
                app.update();
            }
            "disconnect" => {
                app.world_mut().entity_mut(player).remove::<Client>();
                app.update();
            }
            _ => unreachable!(),
        }
        assert!(
            frames
                .read(app.world().resource::<Events<GatheringProgressFrame>>())
                .any(|frame| frame.session_id == session_id
                    && frame.interrupted
                    && !frame.completed),
            "{trigger} 必须产生终态，供 HUD 和循环动画收尾"
        );
        for _ in 0..180 {
            app.update();
        }
        assert!(
            app.world()
                .resource::<GatheringSessionStore>()
                .session_for(player)
                .is_none(),
            "{trigger} 后测试会话不得重新开始"
        );
    }
}

#[test]
fn gathering_scenes_respect_access_and_normal_session_ownership() {
    use crate::botany::components::{BotanyHarvestMode, HarvestSessionStore};
    use crate::botany::harvest::start_or_resume_harvest;
    use crate::botany::registry::BotanyPlantId;
    use crate::gathering::session::GatheringSessionStore;
    use crate::spiritwood::session::{WoodSession, WoodSessionStore};
    use crate::world::dimension::DimensionKind;
    use valence::prelude::BlockPos;
    for (enabled, allowed, alive) in [
        (false, true, true),
        (true, false, true),
        (true, true, false),
    ] {
        let (mut app, player, mut helper) = setup_gathering(enabled);
        if !allowed {
            app.insert_resource(DevCommandPermissions::allow_user("AnotherOperator"));
        }
        if !alive {
            app.world_mut().get_mut::<Lifecycle>(player).unwrap().state =
                LifecycleState::AwaitingRevival;
        }
        execute(&mut app, &mut helper, "scene test_gathering_herb_1");
        assert!(
            app.world().resource::<GatheringSessionStore>().is_empty(),
            "环境关闭、未授权或复活裁决中都不得启动采集夹具"
        );
    }

    let (mut app, player, mut helper) = setup_gathering(true);
    execute(&mut app, &mut helper, "scene test_gathering_herb_1");
    let mut normal = app
        .world_mut()
        .resource_mut::<GatheringSessionStore>()
        .remove(player)
        .unwrap();
    normal.session_id = "normal-gathering".to_string();
    app.world_mut()
        .resource_mut::<GatheringSessionStore>()
        .upsert(normal.clone());
    let other = app.world_mut().spawn_empty().id();
    let mut other_session = normal.clone();
    other_session.player = other;
    app.world_mut()
        .resource_mut::<GatheringSessionStore>()
        .upsert(other_session.clone());
    for command in ["scene test_gathering_ore_1", "scene clear"] {
        execute(&mut app, &mut helper, command);
        let store = app.world().resource::<GatheringSessionStore>();
        assert_eq!(
            store.session_for(player),
            Some(&normal),
            "不得覆盖或清理正常玩法接管的会话"
        );
        assert_eq!(
            store.session_for(other),
            Some(&other_session),
            "不得修改其他玩家的会话"
        );
    }

    for herb in [true, false] {
        let (mut app, player, mut helper) = setup_gathering(true);
        execute(&mut app, &mut helper, "scene test_gathering_ore_1");
        let mut herbs = HarvestSessionStore::default();
        let mut wood = WoodSessionStore::default();
        if herb {
            assert!(start_or_resume_harvest(
                &mut herbs,
                "Builder",
                player,
                None,
                BotanyPlantId::NingMaiCao,
                BotanyHarvestMode::Manual,
                [0.0; 3],
                0,
            ));
        } else {
            wood.upsert(WoodSession::new(
                player,
                "offline:Builder".to_string(),
                DimensionKind::Overworld,
                BlockPos::new(1, 0, 0),
                0,
                [0.0; 3],
                None,
            ));
        }
        let expected_herbs: Vec<_> = herbs.iter().cloned().collect();
        let expected_wood = wood.session_for(player).cloned();
        app.insert_resource(herbs);
        app.insert_resource(wood);
        app.update();
        for command in ["scene test_gathering_ore_1", "scene clear"] {
            assert!(
                app.world().resource::<GatheringSessionStore>().is_empty(),
                "正式草药或伐木会话应终止测试采集，并阻止再次加载"
            );
            execute(&mut app, &mut helper, command);
        }
        assert_eq!(
            app.world()
                .resource::<HarvestSessionStore>()
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            expected_herbs,
            "场景操作不得修改正式草药会话"
        );
        assert_eq!(
            app.world()
                .resource::<WoodSessionStore>()
                .session_for(player),
            expected_wood.as_ref(),
            "场景操作不得修改正式伐木会话"
        );
    }
}
