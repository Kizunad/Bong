#![allow(dead_code, unused_imports)]

use bong_server::network::vfx_event_emit::*;
use bong_server::schema::common::MAX_PAYLOAD_BYTES;
use bong_server::schema::vfx_event::{VfxEventPayloadV1, VfxEventV1};
use bong_server::world::dimension::{DimensionKind, DimensionLayers, OverworldLayer};
use valence::prelude::*;
use valence::protocol::packets::play::{CustomPayloadS2c, ParticleS2c};
use valence::testing::{create_mock_client, MockClientHelper, ScenarioSingleClient};

const TEST_UUID: &str = "550e8400-e29b-41d4-a716-446655440000";

#[derive(Debug)]
struct DecodedParticlePacket {
    particle: Particle,
    offset: Vec3,
    count: i32,
}

fn count_particle_packets(helper: &mut MockClientHelper) -> Vec<DecodedParticlePacket> {
    let mut packets = Vec::new();
    for frame in helper.collect_received().0 {
        let Ok(packet) = frame.decode::<ParticleS2c>() else {
            continue;
        };
        packets.push(DecodedParticlePacket {
            particle: packet.particle.into_owned(),
            offset: packet.offset,
            count: packet.count,
        });
    }
    packets
}

fn setup_vfx_emit_app() -> App {
    let mut app = App::new();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, emit_vfx_event_payloads);
    app
}

fn spawn_mock_client_at(app: &mut App, name: &str, pos: [f64; 3]) -> MockClientHelper {
    let (mut bundle, helper) = create_mock_client(name);
    bundle.player.position = Position::new(pos);
    app.world_mut().spawn(bundle);
    helper
}

fn flush_all_client_packets(app: &mut App) {
    let world = app.world_mut();
    let mut query = world.query::<&mut Client>();
    for mut client in query.iter_mut(world) {
        client
            .flush_packets()
            .expect("mock client packets should flush");
    }
}

fn count_vfx_channel_packets(helper: &mut MockClientHelper) -> Vec<VfxEventV1> {
    let mut payloads = Vec::new();
    for frame in helper.collect_received().0 {
        let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
            continue;
        };
        if packet.channel.as_str() != VFX_EVENT_CHANNEL {
            continue;
        }
        let payload: VfxEventV1 = serde_json::from_slice(packet.data.0 .0)
            .expect("vfx custom payload should decode as VfxEventV1 JSON");
        payloads.push(payload);
    }
    payloads
}

fn setup_vanilla_particle_app() -> (App, Entity, Entity, MockClientHelper, MockClientHelper) {
    let scenario = ScenarioSingleClient::new();
    let layer = scenario.layer;
    let mut app = scenario.app;
    let mut near_helper = scenario.helper;

    app.world_mut()
        .entity_mut(layer)
        .insert(bong_server::world::dimension::OverworldLayer);
    app.insert_resource(DimensionLayers {
        overworld: layer,
        tsy: Entity::PLACEHOLDER,
    });
    app.add_event::<VanillaVfxParticleRequest>();
    app.add_systems(Update, emit_vanilla_vfx_particles);

    app.world_mut().entity_mut(scenario.client).insert((
        Position::new([0.0, 64.0, 0.0]),
        OldPosition::new([0.0, 64.0, 0.0]),
        ViewDistance::new(2),
    ));

    let (mut far_bundle, far_helper) = create_mock_client("Far");
    far_bundle.player.position = Position::new([256.0, 64.0, 256.0]);
    far_bundle.player.old_position = OldPosition::new([256.0, 64.0, 256.0]);
    far_bundle.player.layer.0 = layer;
    far_bundle.visible_chunk_layer.0 = layer;
    far_bundle.visible_entity_layers.0.insert(layer);
    far_bundle.view_distance = ViewDistance::new(2);
    let far_client = app.world_mut().spawn(far_bundle).id();

    app.update();
    near_helper.clear_received();
    let mut far_helper = far_helper;
    far_helper.clear_received();

    (app, scenario.client, far_client, near_helper, far_helper)
}

#[test]
fn default_priority_covers_all_known_events() {
    assert_eq!(
        vfx_default_priority("bong:tribulation_lightning"),
        VfxPriority::Critical
    );
    assert_eq!(
        vfx_default_priority("bong:tribulation_boundary"),
        VfxPriority::Critical
    );
    assert_eq!(
        vfx_default_priority("bong:realm_collapse_boundary"),
        VfxPriority::Critical
    );
    assert_eq!(
        vfx_default_priority("bong:breakthrough_pillar"),
        VfxPriority::Critical
    );
    assert_eq!(
        vfx_default_priority("bong:death_soul_dissipate"),
        VfxPriority::Critical
    );
    assert_eq!(
        vfx_default_priority("bong:npc_death_smoke"),
        VfxPriority::Critical
    );
    assert_eq!(
        vfx_default_priority("bong:npc_death_qi_burst"),
        VfxPriority::Critical
    );
    assert_eq!(
        vfx_default_priority("bong:enlightenment_aura"),
        VfxPriority::Important
    );
    assert_eq!(
        vfx_default_priority("bong:npc_rank_aura_elder"),
        VfxPriority::Important
    );
    assert_eq!(
        vfx_default_priority("bong:npc_rank_aura_master"),
        VfxPriority::Important
    );
    assert_eq!(
        vfx_default_priority("bong:npc_qi_aura_ripple"),
        VfxPriority::Important
    );
    assert_eq!(
        vfx_default_priority("bong:unknown_event"),
        VfxPriority::Normal
    );
}

#[test]
fn player_skill_vfx_families_promote_to_important() {
    // 每个功法家族至少一个代表 id:前缀命中 → Important
    for id in [
        "bong:sword_qi_slash",
        "bong:sword_qi_slash_path",
        "bong:heaven_gate_release",
        "bong:woliu_vacuum_palm_spiral",
        "bong:vortex_spiral",
        "bong:anqi_snipe_bolt",
        "bong:dugu_taint_pulse",
        "bong:tuike_shed_burst",
        "bong:baomai_blood_burn",
        "bong:yidao_meridian_repair",
        "bong:zhenmai_sever_chain",
        // P5 去复用后的真脉 5 招粒子（前缀 `bong:zhenmai_` 原地继承 Important）
        "bong:zhenmai_parry_flash",
        "bong:zhenmai_sever_snap",
        // 被动断脉叙事仍发 jiemai_sever_flash，家族前缀保留
        "bong:jiemai_sever_flash",
        // P5 新登记的爆脉家族前缀（含既存漏网的 beng_quan 本尊）
        "bong:burst_meridian_beng_quan",
        "bong:burst_meridian_tie_shan_kao",
        "bong:burst_meridian_xue_beng_bu",
        "bong:burst_meridian_ni_mai_hu_ti",
        "bong:palm_strike",
    ] {
        assert_eq!(
            vfx_default_priority(id),
            VfxPriority::Important,
            "技能家族粒子 {id} 应为 Important(拥挤 chunk 后于普通命中被丢),实际 {:?}",
            vfx_default_priority(id)
        );
    }
    // 无前缀散号也要命中
    for id in [
        "bong:beng_quan",
        "bong:release_burst",
        "bong:false_skin_shed_burst",
        "bong:poison_mist",
        "bong:shield_raise",
    ] {
        assert_eq!(
            vfx_default_priority(id),
            VfxPriority::Important,
            "技能散号粒子 {id} 应为 Important,实际 {:?}",
            vfx_default_priority(id)
        );
    }
    // 通用命中/环境粒子留 Normal,仍是拥挤时的牺牲品
    for id in [
        "bong:combat_hit",
        "bong:combat_parry",
        "bong:movement_dash",
        "bong:lingtian_till",
        "bong:cultivation_absorb",
        // plan-skill-anim-fidelity-v1 P5：NPC 施法粒子有意留在 Normal 档。
        // 去借用前它们借的 `bong:yidao_meridian_repair` / `bong:jiemai_neutralize_dust`
        // 命中玩家技能家族前缀、误吃 Important,在拥挤 chunk 里挤掉玩家自己的技能
        // 反馈;NPC 背景 cosmetic 不该与玩家主动施放争配额。
        "bong:npc_heal_basic",
        "bong:npc_buff_speed",
        "bong:npc_buff_defense",
    ] {
        assert_eq!(
            vfx_default_priority(id),
            VfxPriority::Normal,
            "非技能粒子 {id} 应保持 Normal,实际 {:?}",
            vfx_default_priority(id)
        );
    }
    // 技能档不越级:仍低于渡劫/死亡 Critical
    assert!(VfxPriority::Critical > vfx_default_priority("bong:sword_qi_slash"));
}

#[test]
fn vanilla_particle_map_keeps_bong_custom_events_off_vanilla_channel() {
    assert_eq!(
        vanilla_particle_from_id("minecraft:smoke"),
        Some(Particle::Smoke)
    );
    assert_eq!(vanilla_particle_from_id("smoke"), Some(Particle::Smoke));
    assert_eq!(
        vanilla_particle_from_id("minecraft:sweep_attack"),
        Some(Particle::SweepAttack)
    );
    assert_eq!(vanilla_particle_from_id("bong:sword_qi_slash"), None);
}

#[test]
fn priority_ordering_is_correct() {
    // 确保 Ord 派生按数值升序:Critical > Important > Normal > Verbose
    assert!(VfxPriority::Critical > VfxPriority::Important);
    assert!(VfxPriority::Important > VfxPriority::Normal);
    assert!(VfxPriority::Normal > VfxPriority::Verbose);
}

#[test]
fn emit_only_delivers_within_64_blocks() {
    let mut app = setup_vfx_emit_app();
    let mut near_helper = spawn_mock_client_at(&mut app, "Near", [10.0, 64.0, 10.0]);
    let mut far_helper = spawn_mock_client_at(&mut app, "Far", [1000.0, 64.0, 1000.0]);

    app.world_mut().send_event(VfxEventRequest::new(
        DVec3::new(10.0, 64.0, 10.0),
        VfxEventPayloadV1::PlayAnim {
            target_player: TEST_UUID.to_string(),
            anim_id: "bong:sword_swing_horiz".to_string(),
            priority: 1000,
            fade_in_ticks: Some(3),
        },
    ));

    app.update();
    flush_all_client_packets(&mut app);

    let near_payloads = count_vfx_channel_packets(&mut near_helper);
    let far_payloads = count_vfx_channel_packets(&mut far_helper);

    assert_eq!(
        near_payloads.len(),
        1,
        "near client should receive exactly one vfx payload"
    );
    assert!(
        far_payloads.is_empty(),
        "far client should not receive vfx payload (filtered at 64-block radius)"
    );

    match &near_payloads[0].payload {
        VfxEventPayloadV1::PlayAnim {
            anim_id, priority, ..
        } => {
            assert_eq!(anim_id, "bong:sword_swing_horiz");
            assert_eq!(*priority, 1000);
        }
        other => panic!("expected PlayAnim, got {other:?}"),
    }
}

#[test]
fn emit_boundary_vfx_reaches_tribulation_edge_clients() {
    let mut app = setup_vfx_emit_app();
    let mut edge_helper = spawn_mock_client_at(&mut app, "Edge", [100.0, 64.0, 0.0]);
    let mut far_helper = spawn_mock_client_at(&mut app, "Far", [160.0, 64.0, 0.0]);

    app.world_mut().send_event(VfxEventRequest::new(
        DVec3::new(0.0, 64.0, 0.0),
        VfxEventPayloadV1::SpawnParticle {
            event_id: "bong:tribulation_boundary".to_string(),
            origin: [0.0, 64.0, 0.0],
            direction: None,
            color: Some("#D0C8FF".to_string()),
            strength: Some(1.0),
            count: Some(1),
            duration_ticks: Some(200),
        },
    ));

    app.update();
    flush_all_client_packets(&mut app);

    let edge_payloads = count_vfx_channel_packets(&mut edge_helper);
    let far_payloads = count_vfx_channel_packets(&mut far_helper);
    assert_eq!(edge_payloads.len(), 1);
    assert!(far_payloads.is_empty());
}

#[test]
fn emit_drops_oversize_payload_without_crashing() {
    // 单独伪造一个超过 MAX_PAYLOAD_BYTES 的 anim_id，触发 to_json_bytes_checked 里的 Oversize 分支。
    let mut app = setup_vfx_emit_app();
    let mut helper = spawn_mock_client_at(&mut app, "Near", [10.0, 64.0, 10.0]);

    app.world_mut().send_event(VfxEventRequest::new(
        DVec3::new(10.0, 64.0, 10.0),
        VfxEventPayloadV1::PlayAnim {
            target_player: TEST_UUID.to_string(),
            anim_id: format!(
                "bong:{}",
                "a".repeat(bong_server::schema::common::MAX_PAYLOAD_BYTES * 2)
            ),
            priority: 1000,
            fade_in_ticks: Some(3),
        },
    ));

    app.update();
    flush_all_client_packets(&mut app);

    let payloads = count_vfx_channel_packets(&mut helper);
    assert!(
        payloads.is_empty(),
        "oversize payload must be dropped rather than sent"
    );
}

#[test]
fn emit_drops_out_of_range_priority_without_crashing() {
    let mut app = setup_vfx_emit_app();
    let mut helper = spawn_mock_client_at(&mut app, "Near", [10.0, 64.0, 10.0]);

    app.world_mut().send_event(VfxEventRequest::new(
        DVec3::new(10.0, 64.0, 10.0),
        VfxEventPayloadV1::PlayAnim {
            target_player: TEST_UUID.to_string(),
            anim_id: "bong:foo".to_string(),
            priority: 9999, // > VFX_ANIM_PRIORITY_MAX, validate_ranges 应拦截
            fade_in_ticks: Some(3),
        },
    ));

    app.update();
    flush_all_client_packets(&mut app);

    let payloads = count_vfx_channel_packets(&mut helper);
    assert!(
        payloads.is_empty(),
        "priority out of range should fail validation before send"
    );
}

#[test]
fn vanilla_particles_use_chunk_layer_view_filtering() {
    let (mut app, _near, _far, mut near_helper, mut far_helper) = setup_vanilla_particle_app();

    app.world_mut().send_event(VanillaVfxParticleRequest {
        dimension: DimensionKind::Overworld,
        particle: Particle::Smoke,
        origin: DVec3::new(0.0, 64.0, 0.0),
        offset: Vec3::new(0.25, 0.5, 0.25),
        max_speed: 0.1,
        count: 6,
        long_distance: false,
    });

    app.update();
    flush_all_client_packets(&mut app);

    let near_packets = count_particle_packets(&mut near_helper);
    let far_packets = count_particle_packets(&mut far_helper);

    assert_eq!(
        near_packets.len(),
        1,
        "near viewer should receive ParticleS2c"
    );
    assert!(
        far_packets.is_empty(),
        "far viewer outside chunk view should receive nothing"
    );
    assert_eq!(near_packets[0].particle, Particle::Smoke);
    assert_eq!(near_packets[0].count, 6);
    assert_eq!(near_packets[0].offset, Vec3::new(0.25, 0.5, 0.25));
}

#[test]
fn vanilla_particles_do_not_use_bong_custom_payload_channel() {
    let (mut app, _near, _far, mut near_helper, _far_helper) = setup_vanilla_particle_app();

    app.world_mut().send_event(VanillaVfxParticleRequest::new(
        DimensionKind::Overworld,
        Particle::SweepAttack,
        DVec3::new(0.0, 64.0, 0.0),
    ));

    app.update();
    flush_all_client_packets(&mut app);

    let custom_payloads = count_vfx_channel_packets(&mut near_helper);
    assert!(
        custom_payloads.is_empty(),
        "vanilla particles must bypass bong:vfx_event JSON channel"
    );
}

#[test]
fn vanilla_particle_drop_when_dimension_layers_missing() {
    let mut app = App::new();
    app.add_event::<VanillaVfxParticleRequest>();
    app.add_systems(Update, emit_vanilla_vfx_particles);
    app.world_mut().send_event(VanillaVfxParticleRequest::new(
        DimensionKind::Overworld,
        Particle::Smoke,
        DVec3::ZERO,
    ));

    app.update();
}
