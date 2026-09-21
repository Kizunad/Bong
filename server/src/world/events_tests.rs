use std::collections::HashMap;

use serde_json::json;
use serde_json::Value;
use valence::entity::lightning::LightningEntity;
use valence::prelude::{
    bevy_ecs, App, BlockPos, DVec3, Despawned, Entity, EntityKind, Events, IntoSystemConfigs,
    Position, Update, With,
};
use valence::testing::{create_mock_client, ScenarioSingleClient};

use super::{
    average_zone_qi, beast_kind_from_command, daoxiang_count_for_intensity,
    maybe_nullify_targeted_zone_qi, persist_zone_collapsed_overlays,
    redistribute_zone_qi_before_collapse, tick_active_events, ActiveEventsResource,
    CalamityTargetRecord, RealmCollapseLowQiMonitor, ZoneCollapsedEvent, ZoneOccupantPosition,
    COLLAPSED_ZONE_DANGER_LEVEL, EVENT_BEAST_TIDE, EVENT_DAOXIANG_WAVE, EVENT_KARMA_BACKLASH,
    EVENT_POISON_MIASMA, EVENT_REALM_COLLAPSE, EVENT_THUNDER_TRIBULATION,
    LOCUST_SWARM_DISBAND_THRESHOLD, REALM_COLLAPSE_BOUNDARY_VFX_EVENT_ID,
    REALM_COLLAPSE_EVACUATION_REMINDER_INTERVAL_TICKS, REALM_COLLAPSE_EVACUATION_WINDOW_TICKS,
    REALM_COLLAPSE_LOW_QI_REQUIRED_TICKS, REALM_COLLAPSE_LOW_QI_THRESHOLD,
    TARGETED_LIGHTNING_VFX_EVENT_ID,
};
use crate::combat::events::DeathEvent;
use crate::combat::rat_bite::RatBiteEvent;
use crate::cultivation::components::{Cultivation, Realm};
use crate::fauna::components::{BeastKind, FaunaTag};
use crate::fauna::rat_phase::RatPhase;
use crate::inventory::{DroppedLootEntry, DroppedLootRegistry, ItemInstance, ItemRarity};
use crate::npc::lifecycle::{NpcArchetype, NpcRegistry};
use crate::npc::patrol::NpcPatrol;
use crate::npc::spawn::NpcMarker;
use crate::persistence::{
    bootstrap_sqlite, load_zone_overlays, PersistenceSettings, ZONE_OVERLAY_PAYLOAD_VERSION,
};
use crate::qi_physics::QiTransfer;
use crate::schema::agent_command::Command;
use crate::schema::common::CommandType;
use crate::schema::tribulation::{TribulationKindV1, TribulationPhaseV1};
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::world::calamity::{
    AttentionTier, CalamityArsenal, CalamityKind, TiandaoPower, EVENT_HEAVENLY_FIRE,
    EVENT_MERIDIAN_SEAL,
};
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::karma::{KarmaWeightStore, QiDensityHeatmap};
use crate::world::season::Season;
use crate::world::zone::Zone;
use crate::world::zone::ZoneRegistry;
use crate::world::zone::DEFAULT_SPAWN_ZONE_NAME;

fn test_zone(name: &str, dimension: DimensionKind, spirit_qi: f64, x: f64) -> Zone {
    Zone {
        name: name.to_string(),
        dimension,
        bounds: (DVec3::new(x, 64.0, 0.0), DVec3::new(x + 16.0, 80.0, 16.0)),
        spirit_qi,
        danger_level: 0,
        active_events: Vec::new(),
        patrol_anchors: Vec::new(),
        blocked_tiles: Vec::new(),
        qi_equilibrium: 0.0,
        qi_inflow_per_min: 0.0,
    }
}

fn spawn_event_command(target: &str, event: &str, duration_ticks: u64) -> Command {
    let mut params = HashMap::new();
    params.insert("event".to_string(), json!(event));
    params.insert("duration_ticks".to_string(), json!(duration_ticks));

    Command {
        command_type: CommandType::SpawnEvent,
        target: target.to_string(),
        params,
    }
}

#[test]
fn average_zone_qi_ignores_tsy_blueprint_zones() {
    let registry = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![
            test_zone("spawn", DimensionKind::Overworld, 0.8, 0.0),
            test_zone("blood_valley", DimensionKind::Overworld, 0.2, 32.0),
            test_zone("tsy_daneng_01_deep", DimensionKind::Tsy, -1.2, 64.0),
        ],
    };

    assert_eq!(
        average_zone_qi(&registry),
        0.5,
        "主世界天道灾厄 power 均值不能被 TSY blueprint 负压拉低"
    );
}

#[test]
fn average_zone_qi_uses_fallback_when_no_overworld_zones_exist() {
    let registry = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![test_zone(
            "tsy_daneng_01_deep",
            DimensionKind::Tsy,
            -1.2,
            64.0,
        )],
    };

    assert_eq!(average_zone_qi(&registry), 0.4);
}

/// 回归 PR-177 codex P1：beast_tide 路径不能 spawn whale，否则掉落 exploit。
/// `beast_kind_from_command` 接受 rat / spider / hybrid_beast / void_distorted；
/// **拒绝** whale（防止僵尸贴 FaunaTag::Whale 后掉 jing_gu/jing_sui/jing_hun_yu）。
#[test]
fn beast_kind_from_command_rejects_whale_to_prevent_loot_exploit() {
    let make_cmd = |kind: &str| {
        let mut params = HashMap::new();
        params.insert("beast_kind".to_string(), json!(kind));
        Command {
            command_type: CommandType::SpawnEvent,
            target: "any".to_string(),
            params,
        }
    };
    // 接受这些（合法 tide kind —— 含新增陆地物种）
    let legit_kinds: &[(&str, BeastKind)] = &[
        ("rat", BeastKind::Rat),
        ("spider", BeastKind::Spider),
        ("hybrid_beast", BeastKind::HybridBeast),
        ("void_distorted", BeastKind::VoidDistorted),
        ("green_spider", BeastKind::GreenSpider),
        ("jungle_scorpion", BeastKind::JungleScorpion),
        ("cockade_snake", BeastKind::CockadeSnake),
        ("blue_spider", BeastKind::BlueSpider),
        ("ice_scorpion", BeastKind::IceScorpion),
        ("mandrake_snake", BeastKind::MandrakeSnake),
        ("dark_tiger", BeastKind::DarkTiger),
    ];
    for (name, expected) in legit_kinds {
        assert_eq!(
            beast_kind_from_command(&make_cmd(name)),
            Some(*expected),
            "{name} 应被合法 beast_tide 接受并映射到 {expected:?}"
        );
    }
    // 拒绝 whale / dragon / pillar —— 需要专属 spawn 路径，防 legendary exploit
    for rejected in ["whale", "poison_dragon", "bone_dragon", "living_pillar"] {
        assert_eq!(
            beast_kind_from_command(&make_cmd(rejected)),
            None,
            "{rejected} 不能进 beast_tide 通道（exploit 风险）"
        );
    }
    // 兜底：未知 kind 也是 None
    assert_eq!(beast_kind_from_command(&make_cmd("unknown_xyz")), None);
}

fn spawn_event_command_with_params(
    target: &str,
    event: &str,
    duration_ticks: u64,
    intensity: f64,
    target_player: Option<&str>,
) -> Command {
    let mut params = HashMap::new();
    params.insert("event".to_string(), json!(event));
    params.insert("duration_ticks".to_string(), json!(duration_ticks));
    params.insert("intensity".to_string(), json!(intensity));
    if let Some(target_player) = target_player {
        params.insert("target_player".to_string(), json!(target_player));
    }

    Command {
        command_type: CommandType::SpawnEvent,
        target: target.to_string(),
        params,
    }
}

fn calamity_command(target: &str, event: &str, attention: AttentionTier) -> Command {
    let mut command = spawn_event_command_with_params(target, event, 100, 0.7, None);
    command.params.insert(
        "attention_level".to_string(),
        json!(attention_wire(attention)),
    );
    command
        .params
        .insert("reason".to_string(), json!("unit-test calamity"));
    command
}

fn attention_wire(attention: AttentionTier) -> &'static str {
    match attention {
        AttentionTier::Watch => "watch",
        AttentionTier::Pressure => "pressure",
        AttentionTier::Tribulation => "tribulation",
        AttentionTier::Annihilate => "annihilate",
    }
}

#[test]
fn calamity_arsenal_spends_power_and_emits_vfx_audio() {
    let mut events = ActiveEventsResource::default();
    let mut zones = ZoneRegistry::fallback();
    let arsenal = CalamityArsenal::default();
    let mut power = TiandaoPower::default();
    power.current = 20.0;

    let command = calamity_command("spawn", EVENT_THUNDER_TRIBULATION, AttentionTier::Watch);
    assert!(
        events.enqueue_from_spawn_command_with_karma_power_and_season_at_tick(
            &command,
            Some(&mut zones),
            None,
            None,
            Season::Summer,
            1,
            Some(&mut power),
            Some(&arsenal),
        )
    );
    assert!((power.current - 5.004).abs() < 1e-9);
    assert_eq!(power.spend_log.len(), 1);

    let vfx = events.drain_vfx_events();
    assert_eq!(vfx.len(), 1);
    match &vfx[0].payload {
        VfxEventPayloadV1::SpawnParticle { event_id, .. } => {
            assert_eq!(event_id, CalamityKind::Thunder.vfx_event_id());
        }
        other => panic!("unexpected calamity vfx payload: {other:?}"),
    }
    let audio = events.drain_audio_events();
    assert_eq!(audio.len(), 1);
    assert_eq!(audio[0].recipe_id, CalamityKind::Thunder.audio_recipe_id());

    let expensive = calamity_command("spawn", EVENT_POISON_MIASMA, AttentionTier::Pressure);
    assert!(
        !events.enqueue_from_spawn_command_with_karma_power_and_season_at_tick(
            &expensive,
            Some(&mut zones),
            None,
            None,
            Season::Summer,
            2,
            Some(&mut power),
            Some(&arsenal),
        )
    );
}

#[test]
fn calamity_arsenal_rejects_season_and_concurrency_overflow() {
    let mut events = ActiveEventsResource::default();
    let mut zones = ZoneRegistry::fallback();
    let arsenal = CalamityArsenal::default();
    let mut power = TiandaoPower::default();

    let heavenly_fire = calamity_command("spawn", EVENT_HEAVENLY_FIRE, AttentionTier::Tribulation);
    assert!(
        !events.enqueue_from_spawn_command_with_karma_power_and_season_at_tick(
            &heavenly_fire,
            Some(&mut zones),
            None,
            None,
            Season::Winter,
            1,
            Some(&mut power),
            Some(&arsenal),
        )
    );
    assert_eq!(power.current, 100.0);

    let thunder = calamity_command("spawn", EVENT_THUNDER_TRIBULATION, AttentionTier::Watch);
    let miasma = calamity_command("spawn", EVENT_POISON_MIASMA, AttentionTier::Pressure);
    let seal = calamity_command("spawn", EVENT_MERIDIAN_SEAL, AttentionTier::Pressure);
    assert!(
        events.enqueue_from_spawn_command_with_karma_power_and_season_at_tick(
            &thunder,
            Some(&mut zones),
            None,
            None,
            Season::Summer,
            2,
            Some(&mut power),
            Some(&arsenal),
        )
    );
    assert!(
        events.enqueue_from_spawn_command_with_karma_power_and_season_at_tick(
            &miasma,
            Some(&mut zones),
            None,
            None,
            Season::Summer,
            3,
            Some(&mut power),
            Some(&arsenal),
        )
    );
    assert!(
        !events.enqueue_from_spawn_command_with_karma_power_and_season_at_tick(
            &seal,
            Some(&mut zones),
            None,
            None,
            Season::Summer,
            4,
            Some(&mut power),
            Some(&arsenal),
        )
    );
    assert_eq!(events.calamity_count_for_zone("spawn"), 2);
}

#[test]
fn calamity_target_window_caps_to_three_recent_hits() {
    let mut events = ActiveEventsResource::default();
    let mut zones = ZoneRegistry::fallback();
    let arsenal = CalamityArsenal::default();
    let mut power = TiandaoPower::default();

    events.calamity_target_log.push_back(CalamityTargetRecord {
        tick: 1,
        target_key: "zone:spawn".to_string(),
    });
    events.calamity_target_log.push_back(CalamityTargetRecord {
        tick: 2,
        target_key: "zone:spawn".to_string(),
    });
    events.calamity_target_log.push_back(CalamityTargetRecord {
        tick: 3,
        target_key: "zone:spawn".to_string(),
    });

    let command = calamity_command("spawn", EVENT_THUNDER_TRIBULATION, AttentionTier::Watch);
    assert!(
        !events.enqueue_from_spawn_command_with_karma_power_and_season_at_tick(
            &command,
            Some(&mut zones),
            None,
            None,
            Season::Summer,
            4,
            Some(&mut power),
            Some(&arsenal),
        )
    );
}

fn setup_events_app() -> (App, Entity) {
    let scenario = ScenarioSingleClient::new();
    let layer = scenario.layer;
    let mut app = scenario.app;
    app.world_mut()
        .entity_mut(layer)
        .insert(crate::world::dimension::OverworldLayer);
    app.insert_resource(ZoneRegistry::fallback());
    app.insert_resource(ActiveEventsResource::default());
    app.add_event::<RatBiteEvent>();
    app.add_event::<DeathEvent>();
    app.add_event::<ZoneCollapsedEvent>();
    app.add_systems(Update, tick_active_events);
    (app, layer)
}

fn spawn_mock_player(app: &mut App, layer: Entity, username: &str, position: [f64; 3]) -> Entity {
    let (mut client_bundle, _helper) = create_mock_client(username);
    client_bundle.player.position = Position::new(position);
    client_bundle.player.layer.0 = layer;
    client_bundle.visible_chunk_layer.0 = layer;
    client_bundle.visible_entity_layers.0.insert(layer);

    app.world_mut().spawn(client_bundle).id()
}

fn query_npc_entities(world: &mut bevy_ecs::world::World) -> Vec<Entity> {
    let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
    query.iter(world).collect::<Vec<_>>()
}

fn query_lightning_entities(world: &mut bevy_ecs::world::World) -> Vec<Entity> {
    let mut query = world.query_filtered::<Entity, With<LightningEntity>>();
    query.iter(world).collect::<Vec<_>>()
}

fn overworld_occupant(position: [f64; 3]) -> ZoneOccupantPosition {
    ZoneOccupantPosition {
        dimension: DimensionKind::Overworld,
        position: DVec3::new(position[0], position[1], position[2]),
    }
}

fn test_item(instance_id: u64, template_id: &str) -> ItemInstance {
    ItemInstance {
        instance_id,
        template_id: template_id.to_string(),
        display_name: template_id.to_string(),
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
        charges: None,
        forge_quality: None,
        forge_color: None,
        forge_side_effects: Vec::new(),
        forge_achieved_tier: None,
        alchemy: None,
        lingering_owner_qi: None,
    }
}

fn is_on_zone_edge(zone: &Zone, position: DVec3) -> bool {
    let (min, max) = zone.bounds;
    let epsilon = 1e-6;

    (position.x - min.x).abs() <= epsilon
        || (position.x - max.x).abs() <= epsilon
        || (position.z - min.z).abs() <= epsilon
        || (position.z - max.z).abs() <= epsilon
}

#[test]
fn thunder_event_ticks_until_expiry() {
    let (mut app, layer) = setup_events_app();
    let _target_player = spawn_mock_player(&mut app, layer, "Steve", [8.0, 66.0, 8.0]);

    {
        let world = app.world_mut();
        let command = spawn_event_command_with_params(
            "spawn",
            EVENT_THUNDER_TRIBULATION,
            82,
            0.8,
            Some("offline:Steve"),
        );
        world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
            let mut events = world.resource_mut::<ActiveEventsResource>();
            events.enqueue_from_spawn_command(&command, Some(&mut zones));
        });
    }

    {
        let world = app.world();
        let zone = world
            .resource::<ZoneRegistry>()
            .find_zone(
                crate::world::dimension::DimensionKind::Overworld,
                DVec3::new(8.0, 66.0, 8.0),
            )
            .expect("spawn zone should exist");
        assert!(zone
            .active_events
            .iter()
            .any(|event| event == EVENT_THUNDER_TRIBULATION));
        assert!(world
            .resource::<ActiveEventsResource>()
            .contains("spawn", EVENT_THUNDER_TRIBULATION));
        assert_eq!(
            world
                .resource::<ActiveEventsResource>()
                .thunder_target_for_zone("spawn")
                .as_deref(),
            Some("offline:Steve")
        );
    }

    for _ in 0..40 {
        app.update();
    }
    {
        let world = app.world_mut();
        let events = world.resource::<ActiveEventsResource>();
        assert_eq!(
            events.elapsed_for_first("spawn", EVENT_THUNDER_TRIBULATION),
            Some(40)
        );

        let strikes = events.thunder_strikes_for_zone("spawn");
        assert_eq!(
            strikes.len(),
            3,
            "intensity=0.8 should emit 3 strikes per 40-tick cadence"
        );

        assert!(
            strikes
                .iter()
                .all(|strike| strike.distance_squared(DVec3::new(8.0, 66.0, 8.0)) <= 64.0),
            "target_player bias should place strikes near target player"
        );

        let lightning_entities = query_lightning_entities(world);
        assert_eq!(
            lightning_entities.len(),
            3,
            "thunder runtime should spawn concrete lightning entities"
        );
    }

    for _ in 0..42 {
        app.update();
    }
    {
        let world = app.world();
        let zone = world
            .resource::<ZoneRegistry>()
            .find_zone(
                crate::world::dimension::DimensionKind::Overworld,
                DVec3::new(8.0, 66.0, 8.0),
            )
            .expect("spawn zone should exist");
        assert!(
            !zone
                .active_events
                .iter()
                .any(|event| event == EVENT_THUNDER_TRIBULATION),
            "thunder event should be removed from zone after expiry"
        );
        assert!(
            !world
                .resource::<ActiveEventsResource>()
                .contains("spawn", EVENT_THUNDER_TRIBULATION),
            "thunder event should be removed from scheduler after expiry"
        );
    }
}

#[test]
fn thunder_intensity_scales_runtime_strike_density() {
    let (mut low_app, _low_layer) = setup_events_app();
    let (mut high_app, _high_layer) = setup_events_app();

    {
        let world = low_app.world_mut();
        let low =
            spawn_event_command_with_params("spawn", EVENT_THUNDER_TRIBULATION, 45, 0.1, None);
        world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
            world
                .resource_mut::<ActiveEventsResource>()
                .enqueue_from_spawn_command(&low, Some(&mut zones));
        });
    }

    {
        let world = high_app.world_mut();
        let high =
            spawn_event_command_with_params("spawn", EVENT_THUNDER_TRIBULATION, 45, 0.95, None);
        world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
            world
                .resource_mut::<ActiveEventsResource>()
                .enqueue_from_spawn_command(&high, Some(&mut zones));
        });
    }

    for _ in 0..40 {
        low_app.update();
        high_app.update();
    }

    let low_count = low_app
        .world()
        .resource::<ActiveEventsResource>()
        .thunder_strikes_for_zone("spawn")
        .len();
    let high_count = high_app
        .world()
        .resource::<ActiveEventsResource>()
        .thunder_strikes_for_zone("spawn")
        .len();

    assert_eq!(
        low_count, 1,
        "low intensity should emit one strike per cadence"
    );
    assert_eq!(
        high_count, 3,
        "high intensity should emit denser strikes per cadence"
    );

    let low_lightning = {
        let world = low_app.world_mut();
        query_lightning_entities(world).len()
    };
    let high_lightning = {
        let world = high_app.world_mut();
        query_lightning_entities(world).len()
    };

    assert_eq!(low_lightning, 1);
    assert_eq!(high_lightning, 3);
}

#[test]
fn beast_tide_event_spawns_and_cleans_up() {
    let (mut app, _layer) = setup_events_app();

    {
        let world = app.world_mut();
        let command = spawn_event_command_with_params("spawn", EVENT_BEAST_TIDE, 3, 0.6, None);
        world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
            let mut events = world.resource_mut::<ActiveEventsResource>();
            events.enqueue_from_spawn_command(&command, Some(&mut zones));
        });
    }

    {
        let world = app.world();
        let zone = world
            .resource::<ZoneRegistry>()
            .find_zone(
                crate::world::dimension::DimensionKind::Overworld,
                DVec3::new(8.0, 66.0, 8.0),
            )
            .expect("spawn zone should exist");
        assert!(zone
            .active_events
            .iter()
            .any(|event| event == EVENT_BEAST_TIDE));
        assert!(world
            .resource::<ActiveEventsResource>()
            .contains("spawn", EVENT_BEAST_TIDE));
    }

    app.update();
    {
        let world = app.world_mut();
        let spawned_beasts = world
            .resource::<ActiveEventsResource>()
            .beast_spawned_entities_for_zone("spawn");
        let spawn_points = world
            .resource::<ActiveEventsResource>()
            .beast_spawn_points_for_zone("spawn");

        assert!(
            !spawned_beasts.is_empty(),
            "beast_tide should spawn runtime beasts"
        );
        assert_eq!(
            spawned_beasts.len(),
            spawn_points.len(),
            "tracked beast entities and spawn points should align"
        );

        let zone = world
            .resource::<ZoneRegistry>()
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .expect("spawn zone should exist")
            .clone();

        for entity in &spawned_beasts {
            assert!(
                world.get::<NpcMarker>(*entity).is_some(),
                "spawned beast should include NpcMarker"
            );
            assert_eq!(
                *world
                    .get::<EntityKind>(*entity)
                    .expect("spawned beast should have EntityKind"),
                EntityKind::ZOMBIE,
                "beast_tide runtime should spawn zombie entities"
            );
            assert!(
                world
                    .get::<crate::fauna::components::FaunaTag>(*entity)
                    .is_some(),
                "beast_tide runtime should tag fauna drops"
            );
            let patrol = world
                .get::<NpcPatrol>(*entity)
                .expect("spawned beast should include NpcPatrol");
            assert_eq!(patrol.home_zone, DEFAULT_SPAWN_ZONE_NAME);
            assert!(
                patrol.current_target.distance_squared(zone.center()) < 1e-9,
                "beast patrol target should be zone center"
            );
        }

        assert!(
            spawn_points.iter().all(|pos| zone.contains(*pos)),
            "beast spawns should stay inside authoritative zone bounds"
        );
        assert!(
            spawn_points.iter().all(|pos| is_on_zone_edge(&zone, *pos)),
            "beast_tide should spawn beasts on zone edge"
        );
        assert!(
            query_npc_entities(world).len() >= spawned_beasts.len(),
            "live world should contain spawned beasts"
        );
    }

    app.update();
    app.update();
    {
        let world = app.world_mut();
        let zone = world
            .resource::<ZoneRegistry>()
            .find_zone(
                crate::world::dimension::DimensionKind::Overworld,
                DVec3::new(8.0, 66.0, 8.0),
            )
            .expect("spawn zone should exist");
        assert!(
            !zone
                .active_events
                .iter()
                .any(|event| event == EVENT_BEAST_TIDE),
            "beast_tide should be removed from zone when duration elapses"
        );
        assert!(
            !world
                .resource::<ActiveEventsResource>()
                .contains("spawn", EVENT_BEAST_TIDE),
            "beast_tide should be removed from scheduler when duration elapses"
        );

        let lingering = query_npc_entities(world);
        assert!(
            lingering.is_empty(),
            "beast_tide-spawned NPCs should be despawned after expiry"
        );
    }
}

#[test]
fn daoxiang_wave_retries_spawn_after_budget_recovers() {
    let (mut app, _layer) = setup_events_app();
    app.insert_resource(NpcRegistry {
        max_npc_count: 0,
        resume_npc_count: 0,
        ..NpcRegistry::default()
    });

    {
        let world = app.world_mut();
        let command = spawn_event_command_with_params("spawn", EVENT_DAOXIANG_WAVE, 6, 0.2, None);
        world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
            let accepted = world
                .resource_mut::<ActiveEventsResource>()
                .enqueue_from_spawn_command(&command, Some(&mut zones));
            assert!(accepted, "daoxiang_wave should enter the scheduler");
        });
    }

    app.update();
    assert_eq!(
        {
            let world = app.world_mut();
            query_npc_entities(world).len()
        },
        0,
        "first tick should not spawn daoxiang while NPC budget is exhausted"
    );
    assert_eq!(
        app.world()
            .resource::<ActiveEventsResource>()
            .elapsed_for_first("spawn", EVENT_DAOXIANG_WAVE),
        Some(1),
        "exhausted first tick should advance time but keep event retryable"
    );

    {
        let mut registry = app.world_mut().resource_mut::<NpcRegistry>();
        registry.max_npc_count = 10;
        registry.resume_npc_count = 8;
        registry.live_npc_count = 0;
        registry.spawn_paused = false;
    }

    app.update();
    let spawned_count = {
        let world = app.world_mut();
        query_npc_entities(world).len()
    };
    assert_eq!(
        spawned_count,
        daoxiang_count_for_intensity(0.2),
        "daoxiang_wave should retry and spawn after budget recovers"
    );
    assert_eq!(
        app.world().resource::<NpcRegistry>().live_npc_count,
        spawned_count,
        "retry spawn must be backed by NpcRegistry reservation"
    );
}

#[test]
fn beast_tide_event_spawns_rats_via_spawn_rat_when_kind_is_rat() {
    let (mut app, _layer) = setup_events_app();

    {
        let world = app.world_mut();
        let mut command = spawn_event_command_with_params("spawn", EVENT_BEAST_TIDE, 3, 0.2, None);
        command
            .params
            .insert("beast_kind".to_string(), json!("rat"));
        world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
            let mut events = world.resource_mut::<ActiveEventsResource>();
            assert!(events.enqueue_from_spawn_command(&command, Some(&mut zones)));
            assert_eq!(events.beast_tide_kind_for_zone("spawn"), Some("wandering"));
        });
    }

    app.update();

    let world = app.world_mut();
    let spawned_beasts = world
        .resource::<ActiveEventsResource>()
        .beast_spawned_entities_for_zone("spawn");
    assert!(
        !spawned_beasts.is_empty(),
        "rat beast_tide should spawn concrete rat entities"
    );
    for entity in spawned_beasts {
        assert_eq!(
            world.get::<EntityKind>(entity),
            Some(&crate::fauna::visual::DEVOUR_RAT_ENTITY_KIND)
        );
        assert_eq!(
            world.get::<FaunaTag>(entity).map(|tag| tag.beast_kind),
            Some(BeastKind::Rat)
        );
        assert_eq!(world.get::<RatPhase>(entity), Some(&RatPhase::Solitary));
    }
}

#[test]
fn beast_tide_default_tide_kind_is_wandering_for_backward_compat() {
    let mut zones = ZoneRegistry::fallback();
    let mut events = ActiveEventsResource::default();
    let command = spawn_event_command_with_params("spawn", EVENT_BEAST_TIDE, 3, 0.2, None);

    assert!(events.enqueue_from_spawn_command(&command, Some(&mut zones)));

    assert_eq!(events.beast_tide_kind_for_zone("spawn"), Some("wandering"));
}

#[test]
fn beast_tide_with_tide_kind_locust_swarm_uses_locust_state() {
    let (mut app, _layer) = setup_events_app();

    {
        let world = app.world_mut();
        let mut command = spawn_event_command_with_params("spawn", EVENT_BEAST_TIDE, 5, 0.2, None);
        command
            .params
            .insert("tide_kind".to_string(), json!("locust_swarm"));
        world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
            let mut events = world.resource_mut::<ActiveEventsResource>();
            assert!(events.enqueue_from_spawn_command(&command, Some(&mut zones)));
            assert_eq!(
                events.beast_tide_kind_for_zone("spawn"),
                Some("locust_swarm")
            );
            let alert = events
                .drain_major_event_alerts()
                .pop()
                .expect("locust swarm should emit a major alert");
            assert!(alert
                .message
                .as_deref()
                .is_some_and(|message| message.contains("灵蝗潮逼近")));
        });
    }

    app.update();

    let world = app.world_mut();
    let spawned_beasts = world
        .resource::<ActiveEventsResource>()
        .beast_spawned_entities_for_zone("spawn");
    assert!(
        !spawned_beasts.is_empty(),
        "locust swarm should maintain live rat entities in its active window"
    );
    for entity in spawned_beasts {
        assert_eq!(
            world.get::<EntityKind>(entity),
            Some(&crate::fauna::visual::DEVOUR_RAT_ENTITY_KIND)
        );
        assert_eq!(world.get::<RatPhase>(entity), Some(&RatPhase::Gregarious));
    }
}

#[test]
fn locust_swarm_advance_drains_qi_loot_and_cultivator_qi_pressure() {
    let (mut app, _layer) = setup_events_app();
    let zone_center = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
        .expect("spawn zone should exist")
        .center();
    let heat_block = BlockPos::new(
        zone_center.x.floor() as i32,
        zone_center.y.floor() as i32,
        zone_center.z.floor() as i32,
    );
    let mut heatmap = QiDensityHeatmap::default();
    heatmap.add_heat(DimensionKind::Overworld, heat_block, 0.9);
    app.insert_resource(heatmap);
    app.insert_resource(DroppedLootRegistry {
        entries: HashMap::from([
            (
                41,
                DroppedLootEntry {
                    instance_id: 41,
                    source_container_id: "test".to_string(),
                    source_row: 0,
                    source_col: 0,
                    world_pos: [zone_center.x, zone_center.y, zone_center.z],
                    dimension: DimensionKind::Overworld,
                    item: test_item(41, "bone_coin_5"),
                },
            ),
            (
                42,
                DroppedLootEntry {
                    instance_id: 42,
                    source_container_id: "test".to_string(),
                    source_row: 0,
                    source_col: 0,
                    world_pos: [zone_center.x, zone_center.y, zone_center.z],
                    dimension: DimensionKind::Tsy,
                    item: test_item(42, "bone_coin_5"),
                },
            ),
        ]),
    });
    let cultivator = app
        .world_mut()
        .spawn((
            Position::new([zone_center.x, zone_center.y, zone_center.z]),
            Cultivation {
                realm: Realm::Induce,
                qi_current: 5.0,
                qi_max: 10.0,
                ..Default::default()
            },
        ))
        .id();

    {
        let world = app.world_mut();
        let mut command = spawn_event_command_with_params("spawn", EVENT_BEAST_TIDE, 5, 0.2, None);
        command
            .params
            .insert("tide_kind".to_string(), json!("locust_swarm"));
        world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
            assert!(world
                .resource_mut::<ActiveEventsResource>()
                .enqueue_from_spawn_command(&command, Some(&mut zones)));
        });
    }

    app.update();

    let heat_after = app
        .world()
        .resource::<QiDensityHeatmap>()
        .heat_at(DimensionKind::Overworld, heat_block);
    assert!(
        heat_after < 0.9,
        "locust front should drain qi density heat from the traversed chunk"
    );
    assert_eq!(
            app.world()
                .resource::<DroppedLootRegistry>()
                .entries
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            vec![42],
            "locust front should consume same-dimension loot without deleting matching drops in another dimension"
        );
    let bites = app.world().resource::<Events<RatBiteEvent>>();
    assert!(
        bites
            .iter_current_update_events()
            .any(|event| event.target == cultivator && event.qi_steal == 1),
        "locust front should pressure nearby cultivators through RatBiteEvent"
    );
}

#[test]
fn locust_swarm_disperses_when_live_group_drops_below_threshold() {
    let (mut app, _layer) = setup_events_app();

    {
        let world = app.world_mut();
        let mut command = spawn_event_command_with_params("spawn", EVENT_BEAST_TIDE, 20, 0.2, None);
        command
            .params
            .insert("tide_kind".to_string(), json!("locust_swarm"));
        world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
            assert!(world
                .resource_mut::<ActiveEventsResource>()
                .enqueue_from_spawn_command(&command, Some(&mut zones)));
        });
    }

    app.update();
    let spawned = app
        .world()
        .resource::<ActiveEventsResource>()
        .beast_spawned_entities_for_zone("spawn");
    assert!(
        spawned.len() > LOCUST_SWARM_DISBAND_THRESHOLD as usize,
        "test setup should start above the locust disband threshold"
    );

    for entity in spawned
        .iter()
        .skip(LOCUST_SWARM_DISBAND_THRESHOLD.saturating_sub(1) as usize)
    {
        app.world_mut().entity_mut(*entity).insert(Despawned);
    }

    app.update();

    let deaths = app.world().resource::<Events<DeathEvent>>();
    assert_eq!(
        deaths
            .iter_current_update_events()
            .filter(|event| event.cause == "locust_swarm_dispersed")
            .count(),
        LOCUST_SWARM_DISBAND_THRESHOLD.saturating_sub(1) as usize,
        "remaining live rats below threshold should receive dispersal death events"
    );
    assert!(
        !app.world()
            .resource::<ActiveEventsResource>()
            .contains("spawn", EVENT_BEAST_TIDE),
        "depleted locust swarm should leave the scheduler"
    );
}

#[test]
fn spawn_event_only_enters_scheduler_once() {
    let (mut app, _layer) = setup_events_app();
    let command = spawn_event_command("spawn", EVENT_THUNDER_TRIBULATION, 3);

    {
        let world = app.world_mut();
        world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
            let mut events = world.resource_mut::<ActiveEventsResource>();
            events.enqueue_from_spawn_command(&command, Some(&mut zones));
            events.enqueue_from_spawn_command(&command, Some(&mut zones));
        });
    }

    let world = app.world();
    let events = world.resource::<ActiveEventsResource>();
    let zone = world
        .resource::<ZoneRegistry>()
        .find_zone(
            crate::world::dimension::DimensionKind::Overworld,
            DVec3::new(8.0, 66.0, 8.0),
        )
        .expect("spawn zone should exist");

    assert_eq!(
        events.count_by_zone_and_event("spawn", EVENT_THUNDER_TRIBULATION),
        1,
        "repeated spawn_event should only register one scheduled thunder event"
    );
    assert_eq!(
        zone.active_events
            .iter()
            .filter(|name| name.as_str() == EVENT_THUNDER_TRIBULATION)
            .count(),
        1,
        "spawn zone should expose thunder exactly once in stable active_events"
    );

    app.update();
    app.update();
    app.update();

    let world = app.world();
    let events = world.resource::<ActiveEventsResource>();
    let zone = world
        .resource::<ZoneRegistry>()
        .find_zone(
            crate::world::dimension::DimensionKind::Overworld,
            DVec3::new(8.0, 66.0, 8.0),
        )
        .expect("spawn zone should exist");

    assert!(
        !events.contains("spawn", EVENT_THUNDER_TRIBULATION),
        "expired thunder event should be removed from scheduler after cleanup"
    );
    assert!(
        !zone
            .active_events
            .iter()
            .any(|event| event == EVENT_THUNDER_TRIBULATION),
        "expired thunder event should be removed from zone after cleanup"
    );
}

#[test]
fn realm_collapse_collapses_zone_and_kills_occupants() {
    let (mut app, layer) = setup_events_app();
    let player = spawn_mock_player(&mut app, layer, "Azure", [8.0, 66.0, 8.0]);
    let npc = app
        .world_mut()
        .spawn((NpcMarker, Position::new([10.0, 66.0, 10.0])))
        .id();
    let other_dimension_npc = app
        .world_mut()
        .spawn((
            NpcMarker,
            Position::new([8.0, 66.0, 8.0]),
            CurrentDimension(DimensionKind::Tsy),
        ))
        .id();
    let outsider = spawn_mock_player(&mut app, layer, "Far", [300.0, 66.0, 300.0]);
    let command = spawn_event_command("spawn", EVENT_REALM_COLLAPSE, 2);

    {
        let world = app.world_mut();
        world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
            let mut events = world.resource_mut::<ActiveEventsResource>();
            let accepted = events.enqueue_from_spawn_command(&command, Some(&mut zones));
            assert!(accepted, "realm_collapse event should be accepted");
        });
    }

    assert!(app
        .world()
        .resource::<ActiveEventsResource>()
        .contains("spawn", EVENT_REALM_COLLAPSE));

    app.update();
    app.update();

    let world = app.world();
    let zone = world
        .resource::<ZoneRegistry>()
        .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
        .expect("spawn zone should exist");
    assert_eq!(zone.spirit_qi, 0.0);
    assert_eq!(zone.danger_level, COLLAPSED_ZONE_DANGER_LEVEL);
    assert!(zone
        .active_events
        .iter()
        .any(|event| event == EVENT_REALM_COLLAPSE));

    assert!(
        !world
            .resource::<ActiveEventsResource>()
            .contains("spawn", EVENT_REALM_COLLAPSE),
        "realm_collapse should leave scheduler after collapse while zone keeps collapsed marker"
    );

    let deaths = world.resource::<Events<DeathEvent>>();
    let collected: Vec<_> = deaths.get_reader().read(deaths).cloned().collect();
    assert!(collected
        .iter()
        .any(|event| event.target == player && event.cause == "realm_collapse"));
    assert!(collected
        .iter()
        .any(|event| event.target == npc && event.cause == "realm_collapse"));
    assert!(
        !collected
            .iter()
            .any(|event| event.target == other_dimension_npc),
        "realm_collapse must not kill entities at the same coordinates in another dimension"
    );
    assert!(
        !collected.iter().any(|event| event.target == outsider),
        "realm_collapse must not kill entities outside zone bounds"
    );
}

#[test]
fn collapse_redistribution_preserves_positive_zone_qi() {
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![
            Zone {
                name: "source".to_string(),
                dimension: DimensionKind::Overworld,
                bounds: (DVec3::ZERO, DVec3::new(10.0, 10.0, 10.0)),
                spirit_qi: 0.6,
                danger_level: 1,
                active_events: Vec::new(),
                patrol_anchors: Vec::new(),
                blocked_tiles: Vec::new(),
                qi_equilibrium: 0.0,
                qi_inflow_per_min: 0.0,
            },
            Zone {
                name: "low".to_string(),
                dimension: DimensionKind::Overworld,
                bounds: (DVec3::new(20.0, 0.0, 0.0), DVec3::new(30.0, 10.0, 10.0)),
                spirit_qi: 0.1,
                danger_level: 1,
                active_events: Vec::new(),
                patrol_anchors: Vec::new(),
                blocked_tiles: Vec::new(),
                qi_equilibrium: 0.0,
                qi_inflow_per_min: 0.0,
            },
            Zone {
                name: "high".to_string(),
                dimension: DimensionKind::Overworld,
                bounds: (DVec3::new(40.0, 0.0, 0.0), DVec3::new(50.0, 10.0, 10.0)),
                spirit_qi: 0.8,
                danger_level: 1,
                active_events: Vec::new(),
                patrol_anchors: Vec::new(),
                blocked_tiles: Vec::new(),
                qi_equilibrium: 0.0,
                qi_inflow_per_min: 0.0,
            },
        ],
    };

    let before_total: f64 = zones.zones.iter().map(|zone| zone.spirit_qi).sum();
    let mut transfers = Vec::new();
    let redistributed = redistribute_zone_qi_before_collapse(&mut zones, "source", &mut transfers);
    zones.find_zone_mut("source").unwrap().spirit_qi = 0.0;
    let after_total: f64 = zones.zones.iter().map(|zone| zone.spirit_qi).sum();

    assert!(
        (redistributed - 0.6).abs() < 1e-9,
        "期望 redistributed=0.6（source 全部灵气），实际={redistributed}"
    );
    // 邻接未满，无 overflow，守恒全靠 zone 实际吸收
    assert!(
        transfers.is_empty(),
        "期望无 overflow transfer（邻接均有余量），实际有 {} 个",
        transfers.len()
    );
    assert!(
        (after_total - before_total).abs() < 1e-9,
        "期望坍缩前后 zone 总 spirit_qi 守恒，before={before_total} after={after_total}"
    );
    let low_delta = zones.find_zone_by_name("low").unwrap().spirit_qi - 0.1;
    let high_delta = zones.find_zone_by_name("high").unwrap().spirit_qi - 0.8;
    assert!(
        low_delta > high_delta,
        "期望低灵气 zone 吸收更多（反压力权重），low_delta={low_delta} high_delta={high_delta}"
    );
}

#[test]
fn collapse_redistribute_overflow_when_neighbor_near_full() {
    // 核心场景：邻接近满（spirit_qi=0.99）+ 大坍缩 qi（source=0.5）→
    // 邻接 spirit_qi 顶到 1.0（接收 0.01），剩余 ~0.49 进 overflow，无蒸发。
    use crate::qi_physics::ledger::{QiAccountKind, QiTransferReason};

    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![
            Zone {
                name: "source".to_string(),
                dimension: DimensionKind::Overworld,
                bounds: (DVec3::ZERO, DVec3::new(10.0, 10.0, 10.0)),
                spirit_qi: 0.5,
                danger_level: 1,
                active_events: Vec::new(),
                patrol_anchors: Vec::new(),
                blocked_tiles: Vec::new(),
                qi_equilibrium: 0.0,
                qi_inflow_per_min: 0.0,
            },
            Zone {
                name: "near_full".to_string(),
                dimension: DimensionKind::Overworld,
                bounds: (DVec3::new(20.0, 0.0, 0.0), DVec3::new(30.0, 10.0, 10.0)),
                spirit_qi: 0.99,
                danger_level: 1,
                active_events: Vec::new(),
                patrol_anchors: Vec::new(),
                blocked_tiles: Vec::new(),
                qi_equilibrium: 0.0,
                qi_inflow_per_min: 0.0,
            },
        ],
    };

    let stored_qi = 0.5_f64;
    let mut transfers = Vec::new();
    let accepted = redistribute_zone_qi_before_collapse(&mut zones, "source", &mut transfers);

    let neighbor_after = zones.find_zone_by_name("near_full").unwrap().spirit_qi;
    let accepted_by_neighbor = neighbor_after - 0.99;

    // 邻接 spirit_qi 不超过 1.0（clamp 保证）
    assert!(
        neighbor_after <= 1.0 + 1e-9,
        "期望邻接 spirit_qi <= 1.0（clamp 上限），实际={neighbor_after}"
    );
    // 邻接确实吸收了一些（0.99 → 1.0）
    assert!(
        accepted_by_neighbor > 0.0,
        "期望邻接 spirit_qi 增加（从 0.99 到 1.0），实际 delta={accepted_by_neighbor}"
    );
    // 返回值等于邻接实际吸收量
    assert!(
        (accepted - accepted_by_neighbor).abs() < 1e-9,
        "期望 accepted={accepted_by_neighbor}（邻接实际增量），实际 fn 返回={accepted}"
    );
    // overflow transfer 存在
    assert!(
        !transfers.is_empty(),
        "期望有 overflow QiTransfer（邻接满容后余量进 overflow），实际无 transfer"
    );
    // overflow transfer 来源是 source zone
    let t = &transfers[0];
    assert_eq!(
        t.from.kind,
        QiAccountKind::Zone,
        "期望 overflow transfer from 是 Zone 账户（坍缩源 zone），实际 kind={:?}",
        t.from.kind
    );
    assert_eq!(
        t.from.id, "source",
        "期望 overflow transfer from.id='source'，实际='{}'",
        t.from.id
    );
    assert_eq!(
        t.to.kind,
        QiAccountKind::Overflow,
        "期望 overflow transfer to 是 Overflow 账户，实际 kind={:?}",
        t.to.kind
    );
    assert_eq!(
        t.reason,
        QiTransferReason::RiftCollapse,
        "期望 overflow reason=RiftCollapse，实际={:?}",
        t.reason
    );
    // 守恒：accepted + overflow_total == stored_qi
    let overflow_total: f64 = transfers.iter().map(|tr| tr.amount).sum();
    assert!(
        (accepted + overflow_total - stored_qi).abs() < 1e-9,
        "守恒违反：accepted({accepted}) + overflow({overflow_total}) != stored_qi({stored_qi})"
    );
}

#[test]
fn collapse_redistribute_overflow_multi_neighbor_some_full() {
    // 多邻接：一个半满（spirit_qi=0.5）+ 一个近满（spirit_qi=0.99），
    // 大坍缩 qi（source=0.8）→ 半满 zone 正常吸收，近满 zone overflow，总量守恒。
    use crate::qi_physics::ledger::QiAccountKind;

    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![
            Zone {
                name: "source".to_string(),
                dimension: DimensionKind::Overworld,
                bounds: (DVec3::ZERO, DVec3::new(10.0, 10.0, 10.0)),
                spirit_qi: 0.8,
                danger_level: 1,
                active_events: Vec::new(),
                patrol_anchors: Vec::new(),
                blocked_tiles: Vec::new(),
                qi_equilibrium: 0.0,
                qi_inflow_per_min: 0.0,
            },
            Zone {
                name: "half".to_string(),
                dimension: DimensionKind::Overworld,
                bounds: (DVec3::new(20.0, 0.0, 0.0), DVec3::new(30.0, 10.0, 10.0)),
                spirit_qi: 0.5,
                danger_level: 1,
                active_events: Vec::new(),
                patrol_anchors: Vec::new(),
                blocked_tiles: Vec::new(),
                qi_equilibrium: 0.0,
                qi_inflow_per_min: 0.0,
            },
            Zone {
                name: "near_full".to_string(),
                dimension: DimensionKind::Overworld,
                bounds: (DVec3::new(40.0, 0.0, 0.0), DVec3::new(50.0, 10.0, 10.0)),
                spirit_qi: 0.99,
                danger_level: 1,
                active_events: Vec::new(),
                patrol_anchors: Vec::new(),
                blocked_tiles: Vec::new(),
                qi_equilibrium: 0.0,
                qi_inflow_per_min: 0.0,
            },
        ],
    };

    let stored_qi = 0.8_f64;
    let mut transfers = Vec::new();
    let accepted = redistribute_zone_qi_before_collapse(&mut zones, "source", &mut transfers);

    let half_after = zones.find_zone_by_name("half").unwrap().spirit_qi;
    let full_after = zones.find_zone_by_name("near_full").unwrap().spirit_qi;

    // 两个邻接均不超 1.0
    assert!(
        half_after <= 1.0 + 1e-9,
        "half zone spirit_qi 不得超 1.0，实际={half_after}"
    );
    assert!(
        full_after <= 1.0 + 1e-9,
        "near_full zone spirit_qi 不得超 1.0，实际={full_after}"
    );

    // 有 overflow transfer（near_full 溢出）
    let overflow_ids: Vec<_> = transfers
        .iter()
        .filter(|t| t.to.kind == QiAccountKind::Overflow)
        .collect();
    assert!(
        !overflow_ids.is_empty(),
        "期望 near_full 溢出产生 overflow transfer，实际无"
    );

    // 守恒：accepted + Σoverflow == stored_qi
    let overflow_total: f64 = transfers.iter().map(|t| t.amount).sum();
    assert!(
        (accepted + overflow_total - stored_qi).abs() < 1e-9,
        "守恒违反：accepted({accepted}) + overflow({overflow_total}) != stored_qi({stored_qi})"
    );
}

#[test]
fn collapse_redistribute_zero_qi_no_transfers() {
    // 边界：坍缩 zone qi=0，不产生任何 overflow 也不崩溃。
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![
            Zone {
                name: "source".to_string(),
                dimension: DimensionKind::Overworld,
                bounds: (DVec3::ZERO, DVec3::new(10.0, 10.0, 10.0)),
                spirit_qi: 0.0,
                danger_level: 1,
                active_events: Vec::new(),
                patrol_anchors: Vec::new(),
                blocked_tiles: Vec::new(),
                qi_equilibrium: 0.0,
                qi_inflow_per_min: 0.0,
            },
            Zone {
                name: "neighbor".to_string(),
                dimension: DimensionKind::Overworld,
                bounds: (DVec3::new(20.0, 0.0, 0.0), DVec3::new(30.0, 10.0, 10.0)),
                spirit_qi: 0.5,
                danger_level: 1,
                active_events: Vec::new(),
                patrol_anchors: Vec::new(),
                blocked_tiles: Vec::new(),
                qi_equilibrium: 0.0,
                qi_inflow_per_min: 0.0,
            },
        ],
    };

    let mut transfers = Vec::new();
    let accepted = redistribute_zone_qi_before_collapse(&mut zones, "source", &mut transfers);

    assert!(
        accepted.abs() < 1e-9,
        "期望 qi=0 坍缩返回 accepted=0，实际={accepted}"
    );
    assert!(
        transfers.is_empty(),
        "期望 qi=0 坍缩无 overflow transfer，实际有 {} 个",
        transfers.len()
    );
    let neighbor_after = zones.find_zone_by_name("neighbor").unwrap().spirit_qi;
    assert!(
        (neighbor_after - 0.5).abs() < 1e-9,
        "期望邻接 spirit_qi 不变（0.5），实际={neighbor_after}"
    );
}

#[test]
fn collapse_redistribute_no_neighbors_all_to_overflow() {
    // 边界：无邻接 zone，source qi 全部进 overflow，守恒不蒸发。
    use crate::qi_physics::ledger::{QiAccountKind, QiTransferReason};

    let stored_qi = 0.6_f64;
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![Zone {
            name: "lone".to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (DVec3::ZERO, DVec3::new(10.0, 10.0, 10.0)),
            spirit_qi: stored_qi,
            danger_level: 1,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
            qi_equilibrium: 0.0,
            qi_inflow_per_min: 0.0,
        }],
    };

    let mut transfers = Vec::new();
    let accepted = redistribute_zone_qi_before_collapse(&mut zones, "lone", &mut transfers);

    assert!(
        accepted.abs() < 1e-9,
        "期望无邻接时 accepted=0，实际={accepted}"
    );
    assert_eq!(
        transfers.len(),
        1,
        "期望无邻接时产生 1 个 overflow transfer，实际有 {} 个",
        transfers.len()
    );
    let t = &transfers[0];
    assert_eq!(
        t.to.kind,
        QiAccountKind::Overflow,
        "期望 transfer to 是 Overflow 账户，实际={:?}",
        t.to.kind
    );
    assert_eq!(
        t.reason,
        QiTransferReason::RiftCollapse,
        "期望 reason=RiftCollapse，实际={:?}",
        t.reason
    );
    assert!(
        (t.amount - stored_qi).abs() < 1e-9,
        "期望 overflow amount={stored_qi}（全量），实际={}",
        t.amount
    );
}

#[test]
fn realm_collapse_emits_evacuation_warning_before_collapse() {
    let mut zones = ZoneRegistry::fallback();
    let mut events = ActiveEventsResource::default();
    let duration_ticks = REALM_COLLAPSE_EVACUATION_WINDOW_TICKS + 2;
    let command = spawn_event_command(
        DEFAULT_SPAWN_ZONE_NAME,
        EVENT_REALM_COLLAPSE,
        duration_ticks,
    );

    assert!(events.enqueue_from_spawn_command(&command, Some(&mut zones)));
    let initial_alerts = events.drain_major_event_alerts();
    assert_eq!(initial_alerts.len(), 1);
    assert_eq!(initial_alerts[0].duration_ticks, duration_ticks);
    assert!(initial_alerts[0].message.is_none());
    let initial_events = events.drain_tribulation_events();
    assert_eq!(initial_events.len(), 1);
    assert_eq!(initial_events[0].kind, TribulationKindV1::ZoneCollapse);
    assert_eq!(initial_events[0].phase, TribulationPhaseV1::Omen);
    let initial_vfx = events.drain_vfx_events();
    assert_eq!(initial_vfx.len(), 1);
    match &initial_vfx[0].payload {
        VfxEventPayloadV1::SpawnParticle {
            event_id,
            origin,
            direction,
            color,
            strength,
            count,
            duration_ticks,
        } => {
            assert_eq!(event_id, REALM_COLLAPSE_BOUNDARY_VFX_EVENT_ID);
            assert_eq!(*origin, [0.0, 65.0, 0.0]);
            assert_eq!(*direction, Some([128.0, 0.0, 128.0]));
            assert_eq!(color.as_deref(), Some("#2B2B31"));
            assert_eq!(*strength, Some(0.35));
            assert_eq!(*count, Some(64));
            assert_eq!(*duration_ticks, Some(160));
        }
        other => panic!("unexpected realm collapse vfx payload: {other:?}"),
    }

    let _ = events.tick(
        Some(&mut zones),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    assert!(events.drain_major_event_alerts().is_empty());
    assert!(events.drain_tribulation_events().is_empty());
    assert!(events.drain_vfx_events().is_empty());

    let _ = events.tick(
        Some(&mut zones),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let evacuation_alerts = events.drain_major_event_alerts();
    assert_eq!(evacuation_alerts.len(), 1);
    assert_eq!(
        evacuation_alerts[0].duration_ticks,
        REALM_COLLAPSE_EVACUATION_WINDOW_TICKS
    );
    assert!(evacuation_alerts[0]
        .message
        .as_deref()
        .is_some_and(|message| message.contains("撤离窗口") && message.contains("横死")));
    let lock_events = events.drain_tribulation_events();
    assert_eq!(lock_events.len(), 1);
    assert_eq!(lock_events[0].kind, TribulationKindV1::ZoneCollapse);
    assert_eq!(lock_events[0].phase, TribulationPhaseV1::Lock);
    let lock_vfx = events.drain_vfx_events();
    assert_eq!(lock_vfx.len(), 1);
    match &lock_vfx[0].payload {
        VfxEventPayloadV1::SpawnParticle {
            event_id, strength, ..
        } => {
            assert_eq!(event_id, REALM_COLLAPSE_BOUNDARY_VFX_EVENT_ID);
            assert_eq!(*strength, Some(0.70));
        }
        other => panic!("unexpected realm collapse lock vfx payload: {other:?}"),
    }

    let _ = events.tick(
        Some(&mut zones),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    assert!(events.drain_major_event_alerts().is_empty());
    assert!(events.drain_tribulation_events().is_empty());
    assert!(events.drain_vfx_events().is_empty());
}

#[test]
fn realm_collapse_emits_minute_evacuation_reminders() {
    let mut zones = ZoneRegistry::fallback();
    let mut events = ActiveEventsResource::default();
    let duration_ticks = REALM_COLLAPSE_EVACUATION_WINDOW_TICKS + 1;
    let command = spawn_event_command(
        DEFAULT_SPAWN_ZONE_NAME,
        EVENT_REALM_COLLAPSE,
        duration_ticks,
    );

    assert!(events.enqueue_from_spawn_command(&command, Some(&mut zones)));
    let _ = events.drain_major_event_alerts();
    let _ = events.drain_tribulation_events();
    let _ = events.drain_vfx_events();

    let _ = events.tick(
        Some(&mut zones),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let evacuation_alerts = events.drain_major_event_alerts();
    assert_eq!(evacuation_alerts.len(), 1);
    assert!(evacuation_alerts[0]
        .message
        .as_deref()
        .is_some_and(|message| message.contains("撤离窗口") && message.contains("横死")));
    assert_eq!(
        evacuation_alerts[0].duration_ticks,
        REALM_COLLAPSE_EVACUATION_WINDOW_TICKS
    );
    let _ = events.drain_tribulation_events();
    let _ = events.drain_vfx_events();

    for _ in 0..REALM_COLLAPSE_EVACUATION_REMINDER_INTERVAL_TICKS - 1 {
        let _ = events.tick(
            Some(&mut zones),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert!(
            events.drain_major_event_alerts().is_empty(),
            "same minute bucket should not spam evacuation reminders"
        );
    }

    let _ = events.tick(
        Some(&mut zones),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let reminders = events.drain_major_event_alerts();
    assert_eq!(reminders.len(), 1);
    assert_eq!(
        reminders[0].duration_ticks,
        REALM_COLLAPSE_EVACUATION_WINDOW_TICKS - REALM_COLLAPSE_EVACUATION_REMINDER_INTERVAL_TICKS
    );
    assert!(reminders[0]
        .message
        .as_deref()
        .is_some_and(|message| message.contains("倒计时") && message.contains("约 9 分钟")));
    assert!(
        events.drain_tribulation_events().is_empty(),
        "periodic reminders should not re-emit tribulation lock events"
    );
    assert!(
        events.drain_vfx_events().is_empty(),
        "periodic reminders should not replay boundary VFX"
    );
}

#[test]
fn realm_collapse_lock_rejects_new_entrants_after_evacuation_window_opens() {
    let (mut app, layer) = setup_events_app();
    let evacuee = spawn_mock_player(&mut app, layer, "Azure", [8.0, 66.0, 8.0]);
    let command = spawn_event_command(
        DEFAULT_SPAWN_ZONE_NAME,
        EVENT_REALM_COLLAPSE,
        REALM_COLLAPSE_EVACUATION_WINDOW_TICKS + 2,
    );

    {
        let world = app.world_mut();
        world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
            world
                .resource_mut::<ActiveEventsResource>()
                .enqueue_from_spawn_command(&command, Some(&mut zones));
        });
    }

    app.update();
    app.update();

    let intruder = spawn_mock_player(&mut app, layer, "Intruder", [9.0, 66.0, 9.0]);
    let other_dimension_intruder =
        spawn_mock_player(&mut app, layer, "TsyIntruder", [9.0, 66.0, 9.0]);
    app.world_mut()
        .entity_mut(other_dimension_intruder)
        .insert(CurrentDimension(DimensionKind::Tsy));
    app.update();

    let world = app.world();
    let deaths = world.resource::<Events<DeathEvent>>();
    let collected: Vec<_> = deaths.get_reader().read(deaths).cloned().collect();
    assert!(collected
        .iter()
        .any(|event| { event.target == intruder && event.cause == "realm_collapse_entry_lock" }));
    assert!(
        !collected
            .iter()
            .any(|event| event.target == other_dimension_intruder),
        "realm_collapse entry lock must not kill same-coordinate players in another dimension"
    );
    assert!(
        !collected
            .iter()
            .any(|event| { event.target == evacuee && event.cause == "realm_collapse_entry_lock" }),
        "players already in the zone when evacuation opens are allowed to evacuate"
    );

    assert!(
        world
            .resource::<ActiveEventsResource>()
            .recent_events_snapshot()
            .iter()
            .any(|event| {
                event.target.as_deref() == Some("realm_collapse_entry_lock")
                    && event.zone.as_deref() == Some(DEFAULT_SPAWN_ZONE_NAME)
            }),
        "entry lock kills should be visible in internal recent events"
    );
}

#[test]
fn realm_collapse_persists_collapsed_overlay() {
    let (mut app, layer) = setup_events_app();
    let db_path = unique_test_db("realm-collapse-persists-overlay");
    std::fs::create_dir_all(db_path.parent().expect("test db should have parent"))
        .expect("test db parent should be creatable");
    let settings = PersistenceSettings::with_db_path(db_path.clone(), "realm-collapse-test");
    bootstrap_sqlite(settings.db_path(), settings.server_run_id())
        .expect("test sqlite should bootstrap");
    app.insert_resource(settings.clone());
    app.add_systems(
        Update,
        persist_zone_collapsed_overlays.after(tick_active_events),
    );
    let _player = spawn_mock_player(&mut app, layer, "Azure", [8.0, 66.0, 8.0]);

    {
        let world = app.world_mut();
        let command = spawn_event_command("spawn", EVENT_REALM_COLLAPSE, 1);
        world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
            world
                .resource_mut::<ActiveEventsResource>()
                .enqueue_from_spawn_command(&command, Some(&mut zones));
        });
    }

    app.update();

    let overlays = load_zone_overlays(&settings).expect("collapsed overlay should load");
    assert!(overlays.iter().any(|overlay| {
        overlay.zone_id == DEFAULT_SPAWN_ZONE_NAME
            && overlay.overlay_kind == "collapsed"
            && overlay.payload_version == ZONE_OVERLAY_PAYLOAD_VERSION
            && overlay
                .payload_json
                .contains("\"zone_status\":\"collapsed\"")
            && overlay.payload_json.contains(EVENT_REALM_COLLAPSE)
    }));
}

#[test]
fn low_qi_monitor_waits_until_threshold_and_occupant_before_realm_collapse() {
    let mut zones = ZoneRegistry::fallback();
    zones
        .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
        .expect("spawn zone should exist")
        .spirit_qi = REALM_COLLAPSE_LOW_QI_THRESHOLD - 0.01;
    let mut events = ActiveEventsResource::default();
    let mut monitor = RealmCollapseLowQiMonitor::default();
    let occupants = [overworld_occupant([8.0, 66.0, 8.0])];

    for _ in 0..REALM_COLLAPSE_LOW_QI_REQUIRED_TICKS - 1 {
        monitor.tick(&mut zones, &mut events, occupants.as_slice());
    }

    assert!(!events.contains(DEFAULT_SPAWN_ZONE_NAME, EVENT_REALM_COLLAPSE));
    assert_eq!(
        monitor.low_qi_ticks_for_zone(DEFAULT_SPAWN_ZONE_NAME),
        Some(REALM_COLLAPSE_LOW_QI_REQUIRED_TICKS - 1)
    );

    monitor.tick(&mut zones, &mut events, occupants.as_slice());

    assert!(events.contains(DEFAULT_SPAWN_ZONE_NAME, EVENT_REALM_COLLAPSE));
    assert_eq!(
        events.elapsed_for_first(DEFAULT_SPAWN_ZONE_NAME, EVENT_REALM_COLLAPSE),
        Some(0)
    );
    assert_eq!(
        monitor.low_qi_ticks_for_zone(DEFAULT_SPAWN_ZONE_NAME),
        None,
        "successful schedule should clear low-qi accumulation"
    );
    assert_eq!(
        zones
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .expect("spawn zone should exist")
            .active_events
            .iter()
            .filter(|event| event.as_str() == EVENT_REALM_COLLAPSE)
            .count(),
        1
    );

    let alerts = events.drain_major_event_alerts();
    assert_eq!(alerts.len(), 1);
    assert_eq!(
            alerts[0].duration_ticks,
            REALM_COLLAPSE_EVACUATION_WINDOW_TICKS,
            "low-qi monitor should schedule the 10-minute evacuation window after the one-hour threshold"
        );
    let tribulation_events = events.drain_tribulation_events();
    assert_eq!(tribulation_events.len(), 1);
    assert_eq!(tribulation_events[0].kind, TribulationKindV1::ZoneCollapse);
    assert_eq!(tribulation_events[0].phase, TribulationPhaseV1::Omen);
    let vfx = events.drain_vfx_events();
    assert_eq!(vfx.len(), 1);
    match &vfx[0].payload {
        VfxEventPayloadV1::SpawnParticle {
            event_id, strength, ..
        } => {
            assert_eq!(event_id, REALM_COLLAPSE_BOUNDARY_VFX_EVENT_ID);
            assert_eq!(*strength, Some(0.35));
        }
        other => panic!("unexpected realm collapse monitor vfx payload: {other:?}"),
    }
}

#[test]
fn low_qi_monitor_requires_occupant_at_threshold() {
    let mut zones = ZoneRegistry::fallback();
    zones
        .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
        .expect("spawn zone should exist")
        .spirit_qi = REALM_COLLAPSE_LOW_QI_THRESHOLD - 0.01;
    let mut events = ActiveEventsResource::default();
    let mut monitor = RealmCollapseLowQiMonitor::default();

    for _ in 0..REALM_COLLAPSE_LOW_QI_REQUIRED_TICKS {
        monitor.tick(&mut zones, &mut events, &[]);
    }

    assert!(!events.contains(DEFAULT_SPAWN_ZONE_NAME, EVENT_REALM_COLLAPSE));
    assert_eq!(
        monitor.low_qi_ticks_for_zone(DEFAULT_SPAWN_ZONE_NAME),
        Some(REALM_COLLAPSE_LOW_QI_REQUIRED_TICKS)
    );

    monitor.tick(
        &mut zones,
        &mut events,
        &[ZoneOccupantPosition {
            dimension: DimensionKind::Tsy,
            position: DVec3::new(8.0, 66.0, 8.0),
        }],
    );

    assert!(
        !events.contains(DEFAULT_SPAWN_ZONE_NAME, EVENT_REALM_COLLAPSE),
        "occupants in another dimension must not trigger overworld zone collapse"
    );

    monitor.tick(
        &mut zones,
        &mut events,
        &[overworld_occupant([300.0, 66.0, 300.0])],
    );

    assert!(
        !events.contains(DEFAULT_SPAWN_ZONE_NAME, EVENT_REALM_COLLAPSE),
        "occupants outside the zone must not trigger collapse"
    );

    monitor.tick(
        &mut zones,
        &mut events,
        &[overworld_occupant([8.0, 66.0, 8.0])],
    );

    assert!(events.contains(DEFAULT_SPAWN_ZONE_NAME, EVENT_REALM_COLLAPSE));
}

#[test]
fn low_qi_monitor_ignores_tsy_blueprint_zones() {
    let zone_name = "tsy_daneng_01_shallow";
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![test_zone(zone_name, DimensionKind::Tsy, -0.45, 0.0)],
    };
    let mut events = ActiveEventsResource::default();
    let mut monitor = RealmCollapseLowQiMonitor::default();
    let occupants = [ZoneOccupantPosition {
        dimension: DimensionKind::Tsy,
        position: DVec3::new(8.0, 66.0, 8.0),
    }];

    for _ in 0..REALM_COLLAPSE_LOW_QI_REQUIRED_TICKS + 1 {
        monitor.tick(&mut zones, &mut events, occupants.as_slice());
    }

    assert!(
        !events.contains(zone_name, EVENT_REALM_COLLAPSE),
        "TSY blueprint 常态负灵气不能触发主世界 low-qi realm_collapse monitor"
    );
    assert_eq!(
        monitor.low_qi_ticks_for_zone(zone_name),
        None,
        "monitor 应清理 TSY zone 计数，避免默认补载后残留状态误触发"
    );
}

#[test]
fn low_qi_monitor_resets_when_qi_recovers() {
    let mut zones = ZoneRegistry::fallback();
    zones
        .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
        .expect("spawn zone should exist")
        .spirit_qi = REALM_COLLAPSE_LOW_QI_THRESHOLD - 0.01;
    let mut events = ActiveEventsResource::default();
    let mut monitor = RealmCollapseLowQiMonitor::default();
    let occupants = [overworld_occupant([8.0, 66.0, 8.0])];

    monitor.tick(&mut zones, &mut events, occupants.as_slice());
    assert_eq!(
        monitor.low_qi_ticks_for_zone(DEFAULT_SPAWN_ZONE_NAME),
        Some(1)
    );

    zones
        .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
        .expect("spawn zone should exist")
        .spirit_qi = REALM_COLLAPSE_LOW_QI_THRESHOLD;
    monitor.tick(&mut zones, &mut events, occupants.as_slice());

    assert_eq!(monitor.low_qi_ticks_for_zone(DEFAULT_SPAWN_ZONE_NAME), None);
    assert!(!events.contains(DEFAULT_SPAWN_ZONE_NAME, EVENT_REALM_COLLAPSE));
}

#[test]
fn low_qi_monitor_does_not_duplicate_active_realm_collapse() {
    let mut zones = ZoneRegistry::fallback();
    zones
        .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
        .expect("spawn zone should exist")
        .spirit_qi = REALM_COLLAPSE_LOW_QI_THRESHOLD - 0.01;
    let mut events = ActiveEventsResource::default();
    let mut monitor = RealmCollapseLowQiMonitor::default();
    let occupants = [overworld_occupant([8.0, 66.0, 8.0])];

    for _ in 0..REALM_COLLAPSE_LOW_QI_REQUIRED_TICKS {
        monitor.tick(&mut zones, &mut events, occupants.as_slice());
    }
    monitor.tick(&mut zones, &mut events, occupants.as_slice());

    assert_eq!(
        events.count_by_zone_and_event(DEFAULT_SPAWN_ZONE_NAME, EVENT_REALM_COLLAPSE),
        1
    );
    assert_eq!(monitor.low_qi_ticks_for_zone(DEFAULT_SPAWN_ZONE_NAME), None);
    assert_eq!(
        zones
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .expect("spawn zone should exist")
            .active_events
            .iter()
            .filter(|event| event.as_str() == EVENT_REALM_COLLAPSE)
            .count(),
        1
    );
}

#[test]
fn concurrent_beast_tides_share_npc_registry_budget() {
    let (mut app, _layer) = setup_events_app();
    app.insert_resource(NpcRegistry {
        max_npc_count: 10,
        resume_npc_count: 8,
        ..NpcRegistry::default()
    });
    app.world_mut()
        .resource_mut::<ZoneRegistry>()
        .zones
        .push(Zone {
            name: "forest".to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            bounds: (
                DVec3::new(100.0, 60.0, 100.0),
                DVec3::new(200.0, 80.0, 200.0),
            ),
            spirit_qi: 0.5,
            danger_level: 0,
            active_events: Vec::new(),
            patrol_anchors: vec![DVec3::new(150.0, 70.0, 150.0)],
            blocked_tiles: Vec::new(),
            qi_equilibrium: 0.0,
            qi_inflow_per_min: 0.0,
        });

    {
        let world = app.world_mut();
        let cmd_spawn = spawn_event_command_with_params("spawn", EVENT_BEAST_TIDE, 6, 0.7, None);
        let cmd_forest = spawn_event_command_with_params("forest", EVENT_BEAST_TIDE, 6, 0.7, None);
        world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
            let mut events = world.resource_mut::<ActiveEventsResource>();
            assert!(events.enqueue_from_spawn_command(&cmd_spawn, Some(&mut zones)));
            assert!(events.enqueue_from_spawn_command(&cmd_forest, Some(&mut zones)));
        });
    }

    app.update();

    let live = query_npc_entities(app.world_mut()).len();
    let cap = app.world().resource::<NpcRegistry>().max_npc_count;
    assert!(
        live <= cap,
        "concurrent beast_tides must share the reserved npc budget: live={live} cap={cap}"
    );
}

#[test]
fn beast_tide_releases_leftover_budget_when_no_layer_available() {
    // P2-5: 当 beast_tide 因 missing layer 提前 continue 时，
    // 事先 reserve 的 npc 配额必须回流到 NpcRegistry —— 否则
    // 同 tick 内 `live_npc_count >= resume_npc_count` 可能误触
    // `spawn_paused=true`，击杀后续 spawn。
    let mut registry = NpcRegistry::default();
    let reserved = registry.reserve_zone_batch(DEFAULT_SPAWN_ZONE_NAME, 5);
    assert_eq!(reserved, 5);
    assert_eq!(registry.live_npc_count, 5);

    let mut zones = ZoneRegistry::fallback();
    let mut events = ActiveEventsResource::default();
    let cmd =
        spawn_event_command_with_params(DEFAULT_SPAWN_ZONE_NAME, EVENT_BEAST_TIDE, 6, 0.7, None);
    assert!(events.enqueue_from_spawn_command(&cmd, Some(&mut zones)));

    // 不传 layer / commands，模拟"事件已 enqueue 但 chunk layer 尚未就位"。
    let leftover = events.tick(
        Some(&mut zones),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(HashMap::from([(
            DEFAULT_SPAWN_ZONE_NAME.to_string(),
            reserved,
        )])),
    );
    assert_eq!(
        leftover
            .as_ref()
            .and_then(|budgets| budgets.get(DEFAULT_SPAWN_ZONE_NAME))
            .copied(),
        Some(reserved),
        "tick must return the full reserved budget when spawn could not occur"
    );

    for (zone, remaining) in leftover.unwrap_or_default() {
        registry.release_zone_batch(zone.as_str(), remaining);
    }
    assert_eq!(
        registry.live_npc_count, 0,
        "leftover budget must be released back to NpcRegistry"
    );
    assert!(
        !registry.spawn_paused,
        "release must un-pause registry if live_npc_count drops below resume threshold"
    );
}

#[test]
fn hidden_karma_backlash_records_internal_marker() {
    let (mut app, _layer) = setup_events_app();
    let command = spawn_event_command("spawn", EVENT_KARMA_BACKLASH, 3);

    {
        let world = app.world_mut();
        world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
            let mut events = world.resource_mut::<ActiveEventsResource>();
            let accepted = events.enqueue_from_spawn_command(&command, Some(&mut zones));
            assert!(accepted, "hidden event should be accepted");
        });
    }

    let world = app.world();
    let events = world.resource::<ActiveEventsResource>();

    assert!(
        !events.contains("spawn", EVENT_KARMA_BACKLASH),
        "hidden event should not remain in active scheduler queue"
    );

    let recent = events.recent_events_snapshot();
    assert!(
        recent.iter().any(
            |event| event.target.as_deref() == Some(EVENT_KARMA_BACKLASH)
                && event.zone.as_deref() == Some("spawn")
                && event
                    .details
                    .as_ref()
                    .and_then(|details| details.get("hidden"))
                    .is_some_and(|flag| flag == &Value::Bool(true))
        ),
        "karma_backlash should append an internal hidden marker"
    );
}

#[test]
fn hidden_karma_backlash_misses_negative_event_at_base_probability() {
    let (mut app, _layer) = setup_events_app();
    let command = spawn_event_command("spawn", EVENT_KARMA_BACKLASH, 3);

    {
        let world = app.world_mut();
        world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
            let mut events = world.resource_mut::<ActiveEventsResource>();
            assert!(events.enqueue_from_spawn_command_with_karma(
                &command,
                Some(&mut zones),
                None,
                None,
            ));

            let recent = events.recent_events_snapshot();
            let marker = recent
                .iter()
                .find(|event| event.target.as_deref() == Some(EVENT_KARMA_BACKLASH))
                .expect("hidden marker should be recorded");
            let details = marker.details.as_ref().expect("marker details");
            assert_eq!(
                details
                    .get("negative_event_triggered")
                    .and_then(Value::as_bool),
                Some(false)
            );
            assert!(recent
                .iter()
                .all(|event| event.target.as_deref() != Some("targeted_negative_event")));
            assert!(events.drain_tribulation_events().is_empty());
        });
    }
}

#[test]
fn hidden_karma_backlash_emits_targeted_hint_when_weighted_roll_hits() {
    let (mut app, _layer) = setup_events_app();
    let command = spawn_event_command("spawn", EVENT_KARMA_BACKLASH, 3);
    let mut karma = KarmaWeightStore::default();
    karma.mark_player(
        "Azure",
        Some("spawn".to_string()),
        valence::prelude::BlockPos::new(8, 66, 8),
        1.0,
        1,
    );

    {
        let world = app.world_mut();
        world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
            let mut events = world.resource_mut::<ActiveEventsResource>();
            assert!(events.enqueue_from_spawn_command_with_karma(
                &command,
                Some(&mut zones),
                Some(&karma),
                None,
            ));

            let recent = events.recent_events_snapshot();
            let marker = recent
                .iter()
                .find(|event| event.target.as_deref() == Some(EVENT_KARMA_BACKLASH))
                .expect("hidden marker should be recorded");
            let details = marker.details.as_ref().expect("marker details");
            assert_eq!(
                details
                    .get("negative_event_triggered")
                    .and_then(Value::as_bool),
                Some(true)
            );
            assert!(recent.iter().any(|event| {
                event.target.as_deref() == Some("targeted_negative_event")
                    && event.zone.as_deref() == Some("spawn")
            }));
            let lightning_marker = recent
                .iter()
                .find(|event| event.target.as_deref() == Some("targeted_local_lightning"))
                .expect("weighted negative event should record local lightning");
            let lightning_position = lightning_marker
                .details
                .as_ref()
                .and_then(|details| details.get("position"))
                .and_then(Value::as_array)
                .expect("local lightning marker should record position");
            assert_eq!(lightning_position[0].as_f64(), Some(8.0));
            assert_eq!(lightning_position[1].as_f64(), Some(66.0));
            assert_eq!(lightning_position[2].as_f64(), Some(8.0));

            let vfx = events.drain_vfx_events();
            assert_eq!(vfx.len(), 1);
            match &vfx[0].payload {
                VfxEventPayloadV1::SpawnParticle {
                    event_id,
                    origin,
                    color,
                    strength,
                    count,
                    duration_ticks,
                    ..
                } => {
                    assert_eq!(event_id, TARGETED_LIGHTNING_VFX_EVENT_ID);
                    assert_eq!(*origin, [8.0, 66.0, 8.0]);
                    assert_eq!(color.as_deref(), Some("#D0C8FF"));
                    assert_eq!(*strength, Some(1.0));
                    assert_eq!(*count, Some(3));
                    assert_eq!(*duration_ticks, Some(14));
                }
                other => panic!("unexpected targeted lightning vfx payload: {other:?}"),
            }

            let tribulation_events = events.drain_tribulation_events();
            assert_eq!(tribulation_events.len(), 1);
            assert_eq!(tribulation_events[0].kind, TribulationKindV1::Targeted);
            assert_eq!(tribulation_events[0].phase, TribulationPhaseV1::Omen);
            assert_eq!(tribulation_events[0].zone.as_deref(), Some("spawn"));
            assert_eq!(tribulation_events[0].epicenter, Some([8.0, 66.0, 8.0]));
        });
    }

    app.update();
    assert_eq!(
        query_lightning_entities(app.world_mut()).len(),
        1,
        "targeted local lightning should spawn a concrete lightning entity on the next world tick"
    );
}

#[test]
fn hidden_karma_backlash_nullifies_zone_qi_when_density_heat_is_high() {
    let (mut app, _layer) = setup_events_app();
    let command = spawn_event_command("spawn", EVENT_KARMA_BACKLASH, 3);
    let mut heatmap = QiDensityHeatmap::default();
    heatmap.add_heat(
        DimensionKind::Overworld,
        valence::prelude::BlockPos::new(8, 66, 8),
        1.0,
    );

    {
        let world = app.world_mut();
        world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
            zones.zones.push(Zone {
                name: "karma_neighbor".to_string(),
                dimension: DimensionKind::Overworld,
                bounds: (DVec3::new(200.0, 0.0, 0.0), DVec3::new(220.0, 80.0, 20.0)),
                spirit_qi: 0.1,
                danger_level: 1,
                active_events: Vec::new(),
                patrol_anchors: Vec::new(),
                blocked_tiles: Vec::new(),
                qi_equilibrium: 0.0,
                qi_inflow_per_min: 0.0,
            });
            let before_total: f64 = zones.zones.iter().map(|zone| zone.spirit_qi).sum();

            let mut events = world.resource_mut::<ActiveEventsResource>();
            assert!(events.enqueue_from_spawn_command_with_karma(
                &command,
                Some(&mut zones),
                None,
                Some(&heatmap),
            ));

            let zone = zones
                .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
                .expect("spawn zone should exist");
            assert_eq!(zone.spirit_qi, 0.0);
            let after_total: f64 = zones.zones.iter().map(|zone| zone.spirit_qi).sum();
            assert!((after_total - before_total).abs() < 1e-9);

            let recent = events.recent_events_snapshot();
            let marker = recent
                .iter()
                .find(|event| event.target.as_deref() == Some(EVENT_KARMA_BACKLASH))
                .expect("hidden marker should be recorded");
            let marker_details = marker.details.as_ref().expect("marker details");
            assert_eq!(
                marker_details
                    .get("qi_density_heat")
                    .and_then(Value::as_f64),
                Some(1.0)
            );
            assert_eq!(
                marker_details
                    .get("negative_event_triggered")
                    .and_then(Value::as_bool),
                Some(true)
            );

            let nullified = recent
                .iter()
                .find(|event| event.target.as_deref() == Some("targeted_qi_nullified"))
                .expect("high heat hit should record qi nullification");
            let details = nullified.details.as_ref().expect("nullification details");
            assert_eq!(
                details.get("previous_spirit_qi").and_then(Value::as_f64),
                Some(0.9)
            );
            assert_eq!(details.get("spirit_qi").and_then(Value::as_f64), Some(0.0));
            assert_eq!(
                details
                    .get("redistributed_spirit_qi")
                    .and_then(Value::as_f64),
                Some(0.9)
            );
            assert_eq!(
                details.get("tribulation_cause").and_then(Value::as_str),
                Some("density_gaze")
            );
            assert_eq!(
                details.get("qi_density_heat").and_then(Value::as_f64),
                Some(1.0)
            );
        });
    }
}

/// 回归测试：no-neighbors / all-neighbors-full 时 maybe_nullify 必须归零 source zone。
///
/// 复现场景：无邻接 zone 时 redistribute 返回 accepted=0（overflow 入账守恒），
/// 旧代码 `if redistributed <= 0 { return None }` 导致 source.spirit_qi 永不归零。
/// 修复后：accepted=0 不再阻止归零，overflow transfer 已入账，source 必须为 0。
#[test]
fn maybe_nullify_zeros_source_zone_when_no_neighbors_exist() {
    let (mut app, _layer) = setup_events_app();
    let initial_qi = 0.7_f64;

    // 移除所有 zone 后仅插入一个孤立 zone（无邻接），确保触发 no-neighbors 路径。
    {
        let world = app.world_mut();
        let mut zones = world.resource_mut::<ZoneRegistry>();
        zones.zones.clear();
        zones.zones.push(Zone {
            name: "lone_nullify".to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (DVec3::ZERO, DVec3::new(16.0, 80.0, 16.0)),
            spirit_qi: initial_qi,
            danger_level: 1,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
            qi_equilibrium: 0.0,
            qi_inflow_per_min: 0.0,
        });
    }

    // qi_density_heat = 1.0 > TARGETED_QI_NULLIFICATION_HEAT_THRESHOLD → 触发 nullify。
    let mut qi_transfers: Vec<QiTransfer> = Vec::new();
    {
        let world = app.world_mut();
        world.resource_scope(|_world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
                let result = maybe_nullify_targeted_zone_qi(
                    &mut zones,
                    "lone_nullify",
                    1.0_f32,
                    &mut qi_transfers,
                );

                // 核心契约：source zone 必须被归零（即使无邻接 / 无 accepted 量）。
                assert!(
                    result.is_some(),
                    "期望 maybe_nullify 在 heat>=阈值、qi>0 时返回 Some，实际 None（source 未归零）"
                );
                let zone = zones
                    .find_zone_by_name("lone_nullify")
                    .expect("lone_nullify zone should exist");
                assert_eq!(
                    zone.spirit_qi,
                    0.0,
                    "期望无邻接时 source zone.spirit_qi 归零，实际={}（守恒背离：overflow 已入账而 source 仍保有全量）",
                    zone.spirit_qi
                );
            });
    }

    // overflow 守恒：qi_transfers 必须含 1 个 overflow transfer，amount = initial_qi。
    assert_eq!(
        qi_transfers.len(),
        1,
        "期望无邻接时有 1 个 overflow QiTransfer（守恒入账），实际 {} 个",
        qi_transfers.len()
    );
    use crate::qi_physics::ledger::QiAccountKind;
    assert_eq!(
        qi_transfers[0].to.kind,
        QiAccountKind::Overflow,
        "期望 transfer.to 是 Overflow 账户，实际={:?}",
        qi_transfers[0].to.kind
    );
    assert!(
        (qi_transfers[0].amount - initial_qi).abs() < 1e-9,
        "期望 overflow amount={initial_qi}（全量），实际={}",
        qi_transfers[0].amount
    );

    // redistributed_spirit_qi = 0 时仍应产生 TargetedQiNullification 返回值。
    let result_data = {
        let world = app.world_mut();
        let mut dummy_transfers: Vec<QiTransfer> = Vec::new();
        // 重置 zone 再测 TargetedQiNullification 字段内容。
        {
            let mut zones = world.resource_mut::<ZoneRegistry>();
            if let Some(z) = zones.find_zone_mut("lone_nullify") {
                z.spirit_qi = initial_qi;
            }
        }
        world.resource_scope(|_world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
            maybe_nullify_targeted_zone_qi(
                &mut zones,
                "lone_nullify",
                1.0_f32,
                &mut dummy_transfers,
            )
        })
    };
    let nullification = result_data.expect("TargetedQiNullification should be Some");
    assert!(
        (nullification.previous_spirit_qi - initial_qi).abs() < 1e-9,
        "期望 previous_spirit_qi={initial_qi}，实际={}",
        nullification.previous_spirit_qi
    );
    // redistributed_spirit_qi=0 合法（无邻接 accepted=0，overflow 守恒），字段如实上报。
    assert!(
        (nullification.redistributed_spirit_qi - 0.0).abs() < 1e-9,
        "期望无邻接时 redistributed_spirit_qi=0（全量进 overflow），实际={}",
        nullification.redistributed_spirit_qi
    );
}

/// 回归测试：all-neighbors-full 时 maybe_nullify 必须归零 source zone。
///
/// 邻接 spirit_qi=1.0（满容），redistribute accepted=0（全部 overflow），
/// source zone 仍必须归零。
#[test]
fn maybe_nullify_zeros_source_zone_when_all_neighbors_full() {
    let initial_qi = 0.5_f64;
    let mut zones = ZoneRegistry {
        spatial_revision: 0,
        zones: vec![
            Zone {
                name: "source_full_neighbors".to_string(),
                dimension: DimensionKind::Overworld,
                bounds: (DVec3::ZERO, DVec3::new(16.0, 80.0, 16.0)),
                spirit_qi: initial_qi,
                danger_level: 1,
                active_events: Vec::new(),
                patrol_anchors: Vec::new(),
                blocked_tiles: Vec::new(),
                qi_equilibrium: 0.0,
                qi_inflow_per_min: 0.0,
            },
            Zone {
                name: "full_neighbor".to_string(),
                dimension: DimensionKind::Overworld,
                bounds: (DVec3::new(20.0, 0.0, 0.0), DVec3::new(36.0, 80.0, 16.0)),
                spirit_qi: 1.0, // 满容，accepted=0
                danger_level: 1,
                active_events: Vec::new(),
                patrol_anchors: Vec::new(),
                blocked_tiles: Vec::new(),
                qi_equilibrium: 0.0,
                qi_inflow_per_min: 0.0,
            },
        ],
    };

    let mut qi_transfers: Vec<QiTransfer> = Vec::new();
    let result = maybe_nullify_targeted_zone_qi(
        &mut zones,
        "source_full_neighbors",
        1.0_f32,
        &mut qi_transfers,
    );

    // 核心契约：邻接全满时，source zone 必须被归零。
    assert!(
        result.is_some(),
        "期望 maybe_nullify 在邻接全满时返回 Some，实际 None（source 未归零）"
    );
    let source = zones
        .find_zone_by_name("source_full_neighbors")
        .expect("source zone should exist");
    assert_eq!(
        source.spirit_qi, 0.0,
        "期望邻接全满时 source.spirit_qi=0，实际={}（overflow 已入账而 source 仍保有全量）",
        source.spirit_qi
    );

    // 邻接满容后全量进 overflow，邻接本身不变。
    let neighbor = zones
        .find_zone_by_name("full_neighbor")
        .expect("full_neighbor zone should exist");
    assert_eq!(
        neighbor.spirit_qi, 1.0,
        "期望满容邻接 spirit_qi 不变=1.0，实际={}",
        neighbor.spirit_qi
    );

    // overflow transfer 必须存在，amount >= initial_qi（允许微浮点差）。
    use crate::qi_physics::ledger::QiAccountKind;
    let overflow_transfers: Vec<_> = qi_transfers
        .iter()
        .filter(|t| t.to.kind == QiAccountKind::Overflow)
        .collect();
    assert!(
        !overflow_transfers.is_empty(),
        "期望邻接全满时存在 overflow QiTransfer，实际无（守恒断裂）"
    );
    let total_overflow: f64 = overflow_transfers.iter().map(|t| t.amount).sum();
    assert!(
        (total_overflow - initial_qi).abs() < 1e-9,
        "期望 overflow 总量={initial_qi}，实际={total_overflow}（守恒不满足）"
    );
}

#[test]
fn hidden_karma_backlash_spawns_daoxiang_after_qi_was_already_nullified() {
    let (mut app, _layer) = setup_events_app();
    app.world_mut()
        .resource_mut::<ZoneRegistry>()
        .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
        .expect("spawn zone should exist")
        .spirit_qi = 0.0;

    let command = spawn_event_command("spawn", EVENT_KARMA_BACKLASH, 3);
    let mut heatmap = QiDensityHeatmap::default();
    heatmap.add_heat(
        DimensionKind::Overworld,
        valence::prelude::BlockPos::new(8, 66, 8),
        1.0,
    );

    {
        let world = app.world_mut();
        world.resource_scope(|world, mut zones: valence::prelude::Mut<ZoneRegistry>| {
            let mut events = world.resource_mut::<ActiveEventsResource>();
            assert!(events.enqueue_from_spawn_command_with_karma(
                &command,
                Some(&mut zones),
                None,
                Some(&heatmap),
            ));

            let recent = events.recent_events_snapshot();
            let negative = recent
                .iter()
                .find(|event| event.target.as_deref() == Some("targeted_negative_event"))
                .expect("weighted hit should record negative event");
            assert_eq!(
                negative
                    .details
                    .as_ref()
                    .and_then(|details| details.get("qi_nullified"))
                    .and_then(Value::as_bool),
                Some(false)
            );
            assert_eq!(
                negative
                    .details
                    .as_ref()
                    .and_then(|details| details.get("daoxiang_spawn_queued"))
                    .and_then(Value::as_bool),
                Some(true)
            );
        });
    }

    app.update();

    let world = app.world_mut();
    let mut archetypes = world.query_filtered::<&NpcArchetype, With<NpcMarker>>();
    assert!(
        archetypes
            .iter(world)
            .any(|archetype| *archetype == NpcArchetype::Daoxiang),
        "high heat targeted backlash should spawn one daoxiang when qi was already nullified"
    );
    let events = world.resource::<ActiveEventsResource>();
    assert!(events.recent_events_snapshot().iter().any(|event| {
        event.target.as_deref() == Some("targeted_daoxiang_spawned")
            && event.zone.as_deref() == Some(DEFAULT_SPAWN_ZONE_NAME)
    }));
}

fn unique_test_db(test_name: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();
    std::env::temp_dir()
        .join(format!("bong-world-events-{test_name}-{nanos}"))
        .join("bong.db")
}
