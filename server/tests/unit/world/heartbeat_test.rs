#![allow(dead_code, unused_imports)]

use std::collections::HashMap;

use bong_server::npc::lifecycle::NpcRegistry;
use bong_server::schema::agent_command::Command;
use bong_server::schema::common::CommandType;
use bong_server::world::dimension::DimensionKind;
use bong_server::world::events::{ActiveEventsResource, EVENT_BEAST_TIDE};
use bong_server::world::heartbeat::{
    apply_heartbeat_override_command, chain_reaction_tick, season_event_modifiers,
    EventChainTrigger, HeartbeatEventKind, HeartbeatOverrideAction, HeartbeatOverrideError,
    WorldHeartbeat,
};
use bong_server::world::season::Season;
use bong_server::world::zone::{Zone, ZoneRegistry};
use serde_json::json;
use valence::prelude::{App, DVec3, Update};

fn zone(name: &str, x: f64, z: f64, spirit_qi: f64) -> Zone {
    Zone {
        name: name.to_string(),
        dimension: DimensionKind::Overworld,
        bounds: (
            DVec3::new(x - 50.0, 60.0, z - 50.0),
            DVec3::new(x + 50.0, 90.0, z + 50.0),
        ),
        spirit_qi,
        danger_level: 0,
        active_events: Vec::new(),
        patrol_anchors: vec![DVec3::new(x, 65.0, z)],
        blocked_tiles: Vec::new(),
        qi_equilibrium: 0.0,
        qi_inflow_per_min: 0.0,
    }
}

fn zone_with_danger(name: &str, x: f64, z: f64, spirit_qi: f64, danger_level: u8) -> Zone {
    Zone {
        danger_level,
        ..zone(name, x, z, spirit_qi)
    }
}

fn tsy_zone(name: &str, x: f64, z: f64, spirit_qi: f64) -> Zone {
    Zone {
        dimension: DimensionKind::Tsy,
        ..zone(name, x, z, spirit_qi)
    }
}

#[test]
fn season_modifiers_pin_world_heartbeat_table() {
    let summer = season_event_modifiers(Season::Summer);
    assert_eq!(summer.pseudo_vein_frequency, 1.0);
    assert_eq!(summer.beast_tide_frequency, 1.5);
    assert_eq!(summer.realm_collapse_frequency, 1.2);

    let winter = season_event_modifiers(Season::Winter);
    assert_eq!(winter.pseudo_vein_frequency, 0.5);
    assert_eq!(winter.pseudo_vein_strength_min, 0.7);
    assert_eq!(winter.beast_tide_scale, 0.6);

    let tide = season_event_modifiers(Season::SummerToWinter);
    assert_eq!(tide.pseudo_vein_frequency, 2.0);
    assert_eq!(tide.karma_backlash_frequency, 2.0);
    assert_eq!(tide.pseudo_vein_strength_min, 0.4);
    assert_eq!(tide.pseudo_vein_strength_max, 0.8);
}

#[test]
fn override_command_rejects_invalid_contract_branches() {
    let valid = Command {
        command_type: CommandType::HeartbeatOverride,
        target: "waste".to_string(),
        params: HashMap::from([
            ("action".to_string(), json!("accelerate")),
            ("event_type".to_string(), json!("beast_tide")),
            ("duration_ticks".to_string(), json!(6000)),
        ]),
    };

    let mut heartbeat = WorldHeartbeat::default();
    assert_eq!(
        apply_heartbeat_override_command(None, &valid, 100),
        Err(HeartbeatOverrideError::MissingHeartbeat),
        "missing WorldHeartbeat resource should reject heartbeat_override instead of succeeding"
    );

    let mut invalid_action = valid.clone();
    invalid_action
        .params
        .insert("action".to_string(), json!("unknown"));
    assert_eq!(
        apply_heartbeat_override_command(Some(&mut heartbeat), &invalid_action, 100),
        Err(HeartbeatOverrideError::InvalidAction),
        "unsupported heartbeat_override action should be rejected"
    );

    let mut invalid_event = valid.clone();
    invalid_event
        .params
        .insert("event_type".to_string(), json!("not_real"));
    assert_eq!(
        apply_heartbeat_override_command(Some(&mut heartbeat), &invalid_event, 100),
        Err(HeartbeatOverrideError::InvalidEventType),
        "unsupported heartbeat_override event_type should be rejected"
    );

    for value in [json!(0), json!(-1), json!("bad")] {
        let mut invalid_duration = valid.clone();
        invalid_duration
            .params
            .insert("duration_ticks".to_string(), value);
        assert_eq!(
            apply_heartbeat_override_command(Some(&mut heartbeat), &invalid_duration, 100),
            Err(HeartbeatOverrideError::InvalidDuration),
            "explicit invalid heartbeat_override duration_ticks should be rejected"
        );
    }
}

#[test]
fn chain_reaction_from_pseudo_vein_dissipation_enqueues_low_qi_beast_tide() {
    let mut app = App::new();
    app.insert_resource(WorldHeartbeat::default());
    app.insert_resource(ActiveEventsResource::default());
    app.insert_resource(ZoneRegistry {
        spatial_revision: 0,
        zones: vec![
            zone("pseudo_vein_done", 0.0, 0.0, 0.0),
            zone("hungry", 300.0, 0.0, 0.1),
        ],
    });
    app.insert_resource(NpcRegistry {
        counts_by_zone: HashMap::from([("hungry".to_string(), 4)]),
        ..Default::default()
    });
    app.add_event::<EventChainTrigger>();
    app.add_systems(Update, chain_reaction_tick);
    app.world_mut()
        .send_event(EventChainTrigger::PseudoVeinDissipated {
            zone_name: "pseudo_vein_done".to_string(),
            redistributed_qi: 0.7,
        });
    app.update();

    let active = app.world().resource::<ActiveEventsResource>();
    assert!(active.contains("hungry", EVENT_BEAST_TIDE));
}

#[test]
fn chain_reaction_from_tsy_pseudo_vein_does_not_enqueue_beast_tide() {
    let mut app = App::new();
    app.insert_resource(WorldHeartbeat::default());
    app.insert_resource(ActiveEventsResource::default());
    app.insert_resource(ZoneRegistry {
        spatial_revision: 0,
        zones: vec![
            tsy_zone("pseudo_vein_tsy_done", 0.0, 0.0, 0.0),
            tsy_zone("tsy_hungry", 300.0, 0.0, -0.30),
        ],
    });
    app.insert_resource(NpcRegistry {
        counts_by_zone: HashMap::from([("tsy_hungry".to_string(), 8)]),
        ..Default::default()
    });
    app.add_event::<EventChainTrigger>();
    app.add_systems(Update, chain_reaction_tick);
    app.world_mut()
        .send_event(EventChainTrigger::PseudoVeinDissipated {
            zone_name: "pseudo_vein_tsy_done".to_string(),
            redistributed_qi: 0.7,
        });
    app.update();

    let active = app.world().resource::<ActiveEventsResource>();
    assert!(
        !active.contains("tsy_hungry", EVENT_BEAST_TIDE),
        "TSY 遗留伪灵脉不应通过主世界 chain_reaction 触发兽潮"
    );
    let zones = app.world().resource::<ZoneRegistry>();
    assert!(
        zones.find_zone_by_name("pseudo_vein_tsy_done").is_none(),
        "即使拒绝 TSY chain reaction，也应清理已完成的 runtime pseudo-vein zone"
    );
}

#[test]
fn chain_reaction_suppression_removes_runtime_zone_without_enqueuing() {
    let mut heartbeat = WorldHeartbeat::default();
    heartbeat.apply_override(
        HeartbeatOverrideAction::Suppress,
        HeartbeatEventKind::BeastTide,
        "hungry".to_string(),
        1_000,
        None,
        0,
    );

    let mut app = App::new();
    app.insert_resource(heartbeat);
    app.insert_resource(ActiveEventsResource::default());
    app.insert_resource(ZoneRegistry {
        spatial_revision: 0,
        zones: vec![
            zone("pseudo_vein_done", 0.0, 0.0, 0.0),
            zone("hungry", 300.0, 0.0, 0.1),
        ],
    });
    app.insert_resource(NpcRegistry {
        counts_by_zone: HashMap::from([("hungry".to_string(), 4)]),
        ..Default::default()
    });
    app.add_event::<EventChainTrigger>();
    app.add_systems(Update, chain_reaction_tick);
    app.world_mut()
        .send_event(EventChainTrigger::PseudoVeinDissipated {
            zone_name: "pseudo_vein_done".to_string(),
            redistributed_qi: 0.7,
        });
    app.update();

    let active = app.world().resource::<ActiveEventsResource>();
    assert!(
        !active.contains("hungry", EVENT_BEAST_TIDE),
        "suppressed beast tide chain reaction should not enqueue an event"
    );
    let zones = app.world().resource::<ZoneRegistry>();
    assert!(
        zones.find_zone_by_name("pseudo_vein_done").is_none(),
        "dissipated runtime pseudo-vein zone should be unregistered"
    );
}

#[test]
fn beast_tide_secondary_entry_danger_weight_widens_effective_qi_threshold() {
    // 三态矩阵「collapse/邻域塌缩扩散事件单独触发」态：只走次入口
    // `PseudoVeinDissipated`，全程 `low_qi_ticks_by_zone` 为空（主入口/qi 骤降因子
    // 完全缺席，heartbeat 用全新默认值）。spirit_qi=0.2 位于 base 阈值(0.15)之上、
    // danger=7 加权阈值(0.15*1.6=0.24)之下——只有 danger 权重放宽窗口后才会触发，
    // 验证次入口确实吃到了 danger 加权。
    let mut app = App::new();
    app.insert_resource(WorldHeartbeat::default());
    app.insert_resource(ActiveEventsResource::default());
    app.insert_resource(ZoneRegistry {
        spatial_revision: 0,
        zones: vec![
            zone("pseudo_vein_done", 0.0, 0.0, 0.0),
            zone_with_danger("scorch_neighbor", 300.0, 0.0, 0.2, 7),
        ],
    });
    app.insert_resource(NpcRegistry {
        counts_by_zone: HashMap::from([("scorch_neighbor".to_string(), 4)]),
        ..Default::default()
    });
    app.add_event::<EventChainTrigger>();
    app.add_systems(Update, chain_reaction_tick);
    app.world_mut()
        .send_event(EventChainTrigger::PseudoVeinDissipated {
            zone_name: "pseudo_vein_done".to_string(),
            redistributed_qi: 0.7,
        });
    app.update();

    let active = app.world().resource::<ActiveEventsResource>();
    assert!(
        active.contains("scorch_neighbor", EVENT_BEAST_TIDE),
        "danger=7 邻域 spirit_qi=0.2 高于 base 阈值 0.15 但低于加权阈值 0.24，\
         danger 加权应放宽次入口的有效窗口使其仍触发兽潮——若未触发说明加权没接次入口"
    );
}

#[test]
fn beast_tide_secondary_entry_low_danger_zone_at_same_qi_does_not_trigger() {
    // 同样 spirit_qi=0.2，邻域改成 danger=1（权重=1.0，有效阈值仍是原始 0.15）——
    // 不该触发，证明"加权放宽"只在真的高危 zone 生效，不是无脑放行所有邻域。
    let mut app = App::new();
    app.insert_resource(WorldHeartbeat::default());
    app.insert_resource(ActiveEventsResource::default());
    app.insert_resource(ZoneRegistry {
        spatial_revision: 0,
        zones: vec![
            zone("pseudo_vein_done", 0.0, 0.0, 0.0),
            zone_with_danger("calm_neighbor", 300.0, 0.0, 0.2, 1),
        ],
    });
    app.insert_resource(NpcRegistry {
        counts_by_zone: HashMap::from([("calm_neighbor".to_string(), 4)]),
        ..Default::default()
    });
    app.add_event::<EventChainTrigger>();
    app.add_systems(Update, chain_reaction_tick);
    app.world_mut()
        .send_event(EventChainTrigger::PseudoVeinDissipated {
            zone_name: "pseudo_vein_done".to_string(),
            redistributed_qi: 0.7,
        });
    app.update();

    let active = app.world().resource::<ActiveEventsResource>();
    assert!(
        !active.contains("calm_neighbor", EVENT_BEAST_TIDE),
        "danger=1 权重=1.0，effective_threshold 仍是原始 0.15；spirit_qi=0.2 >= 0.15 \
         应被判定为灵气已回升而跳过，不应触发"
    );
}
