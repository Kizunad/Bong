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
