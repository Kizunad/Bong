#![allow(dead_code, unused_imports)]

use super::*;
use valence::prelude::{App, OldPosition, Update, ViewDistance};
use valence::protocol::packets::play::{CustomPayloadS2c, ParticleS2c};
use valence::testing::{create_mock_client, MockClientHelper, ScenarioSingleClient};

const TEST_UUID: &str = "550e8400-e29b-41d4-a716-446655440000";

fn test_uuid() -> Uuid {
    Uuid::parse_str(TEST_UUID).unwrap()
}

fn make_particle_request(event_id: &str, origin: [f64; 3], count: u16) -> VfxEventRequest {
    VfxEventRequest::new(
        DVec3::new(origin[0], origin[1], origin[2]),
        VfxEventPayloadV1::SpawnParticle {
            event_id: event_id.to_string(),
            origin,
            direction: None,
            color: None,
            strength: None,
            count: Some(count),
            duration_ticks: None,
        },
    )
}

// ========== §2.5 优先级 / per-chunk 上限 ==========

#[test]
fn per_chunk_cap_drops_excess_normal_keeps_critical() {
    // 同 chunk 塞 20 个事件:8 个 critical + 12 个 normal → critical 全过 + 剩 0 个 normal
    let mut reqs: Vec<VfxEventRequest> = (0..8)
        .map(|i| make_particle_request("bong:breakthrough_pillar", [i as f64 * 0.1, 64.0, 0.0], 1))
        .collect();
    reqs.extend(
        (0..12).map(|i| make_particle_request("bong:combat_hit", [i as f64 * 0.1, 64.0, 0.1], 1)),
    );
    let out = enforce_per_chunk_cap(reqs);
    // 合批前 20 个,8 critical 全过,normal 全丢(cap=8 已被 critical 占满)
    // 但合批在 cap 前执行——这里我们没 coalesce,所以 8 critical 不合批(因为不同 bin 但 same chunk)
    // 实际上 chunk 是 16x16,8 个 [0.x, 0.x] 都在同 chunk (0,0)
    // 12 个 combat_hit 也在 chunk (0,0),但前面 8 个 critical 已占满 cap
    assert_eq!(out.len(), 8);
    for r in &out {
        if let VfxEventPayloadV1::SpawnParticle { event_id, .. } = &r.payload {
            assert_eq!(event_id, "bong:breakthrough_pillar");
        }
    }
}

#[test]
fn per_chunk_cap_skill_particles_survive_over_generic_hits() {
    // 混战场景:同 chunk 12 个通用命中 + 4 个技能粒子,cap=8
    // → 4 个技能粒子(Important)全存活,通用命中(Normal)只留 4 个
    let mut reqs: Vec<VfxEventRequest> = (0..12)
        .map(|i| make_particle_request("bong:combat_hit", [i as f64 * 0.1, 64.0, 0.0], 1))
        .collect();
    reqs.extend(
        (0..4).map(|i| make_particle_request("bong:vortex_spiral", [i as f64 * 0.1, 64.0, 0.1], 1)),
    );
    let out = enforce_per_chunk_cap(reqs);
    assert_eq!(out.len(), 8, "cap=8 应保留恰好 8 条,实际 {}", out.len());
    let skill_survivors = out
        .iter()
        .filter(|r| {
            matches!(
                &r.payload,
                VfxEventPayloadV1::SpawnParticle { event_id, .. }
                    if event_id == "bong:vortex_spiral"
            )
        })
        .count();
    assert_eq!(
        skill_survivors, 4,
        "技能粒子(Important)应在拥挤 chunk 全存活,因为排序先于 Normal 通用命中;实际存活 {skill_survivors}/4"
    );
}

#[test]
fn per_chunk_cap_keeps_different_chunks_separate() {
    // 16 个 sword_qi_slash,分在 2 个 chunk,每 chunk 8 个 → 全过
    let mut reqs: Vec<VfxEventRequest> = (0..8)
        .map(|i| make_particle_request("bong:sword_qi_slash", [i as f64 * 0.1, 64.0, 0.0], 1))
        .collect();
    reqs.extend((0..8).map(|i| {
        make_particle_request("bong:sword_qi_slash", [20.0 + i as f64 * 0.1, 64.0, 0.0], 1)
    }));
    let out = enforce_per_chunk_cap(reqs);
    assert_eq!(out.len(), 16, "16 events across 2 chunks within cap");
}

#[test]
fn per_chunk_cap_ignores_non_spawn_particle() {
    let mut reqs: Vec<VfxEventRequest> = (0..10)
        .map(|_| {
            VfxEventRequest::new(
                DVec3::ZERO,
                VfxEventPayloadV1::PlayAnim {
                    target_player: TEST_UUID.to_string(),
                    anim_id: "bong:swing".to_string(),
                    priority: 1000,
                    fade_in_ticks: Some(3),
                },
            )
        })
        .collect();
    reqs.extend(
        (0..20)
            .map(|i| make_particle_request("bong:sword_qi_slash", [i as f64 * 0.1, 64.0, 0.0], 1)),
    );
    let out = enforce_per_chunk_cap(reqs);
    // 10 PlayAnim 全过(不限流) + 8 SpawnParticle(chunk 限流) = 18
    assert_eq!(out.len(), 18);
}

#[test]
fn particle_stress_100_500_1000_stays_bounded_by_coalesce_and_chunk_cap() {
    for total in [100usize, 500, 1000] {
        let reqs: Vec<VfxEventRequest> = (0..total)
            .map(|i| {
                let bin = i % 12;
                make_particle_request("bong:sword_qi_slash", [bin as f64, 64.0, 0.25], 1)
            })
            .collect();

        let coalesced = coalesce_requests(reqs);
        assert_eq!(
            coalesced.len(),
            12,
            "{total} same-tick particle requests should coalesce to one per 1m bin"
        );

        for req in &coalesced {
            if let VfxEventPayloadV1::SpawnParticle { count, .. } = &req.payload {
                assert!(
                    count.unwrap_or(1) <= VFX_PARTICLE_COUNT_MAX,
                    "{total} stress requests should never exceed merged count cap"
                );
            }
        }

        let capped = enforce_per_chunk_cap(coalesced);
        assert_eq!(
            capped.len(),
            VFX_PER_CHUNK_PER_TICK_MAX as usize,
            "{total} coalesced requests in one chunk should stay under per-chunk cap"
        );
    }

    let saturated = coalesce_requests(
        (0..100)
            .map(|_| make_particle_request("bong:sword_qi_slash", [0.0, 64.0, 0.0], 1))
            .collect(),
    );
    assert_eq!(saturated.len(), 1);
    if let VfxEventPayloadV1::SpawnParticle { count, .. } = &saturated[0].payload {
        assert_eq!(*count, Some(VFX_PARTICLE_COUNT_MAX));
    }
}

// ========== §2.5 合批 ==========

#[test]
fn coalesce_merges_same_id_and_bin_particles() {
    let reqs = vec![
        make_particle_request("bong:sword_qi_slash", [10.2, 64.0, 10.5], 4),
        make_particle_request("bong:sword_qi_slash", [10.6, 64.4, 10.1], 4),
        make_particle_request("bong:sword_qi_slash", [10.0, 64.0, 10.9], 4),
    ];
    let out = coalesce_requests(reqs);
    assert_eq!(out.len(), 1, "three hits in same 1m bin should merge");
    if let VfxEventPayloadV1::SpawnParticle { count, .. } = &out[0].payload {
        assert_eq!(count.unwrap(), 12);
    } else {
        panic!("expected SpawnParticle");
    }
}

#[test]
fn coalesce_keeps_different_bins_separate() {
    let reqs = vec![
        make_particle_request("bong:sword_qi_slash", [10.0, 64.0, 10.0], 4),
        make_particle_request("bong:sword_qi_slash", [12.0, 64.0, 10.0], 4),
    ];
    let out = coalesce_requests(reqs);
    assert_eq!(out.len(), 2, "events in different 1m bins must not merge");
}

#[test]
fn coalesce_keeps_different_event_ids_separate() {
    let reqs = vec![
        make_particle_request("bong:sword_qi_slash", [10.0, 64.0, 10.0], 4),
        make_particle_request("bong:breakthrough_pillar", [10.3, 64.0, 10.3], 12),
    ];
    let out = coalesce_requests(reqs);
    assert_eq!(
        out.len(),
        2,
        "different event ids share bin but stay separate"
    );
}

#[test]
fn coalesce_clamps_to_count_max() {
    let big = VFX_PARTICLE_COUNT_MAX / 2 + 5;
    let reqs = vec![
        make_particle_request("bong:sword_qi_slash", [10.0, 64.0, 10.0], big),
        make_particle_request("bong:sword_qi_slash", [10.2, 64.0, 10.1], big),
    ];
    let out = coalesce_requests(reqs);
    assert_eq!(out.len(), 1);
    if let VfxEventPayloadV1::SpawnParticle { count, .. } = &out[0].payload {
        assert_eq!(count.unwrap(), VFX_PARTICLE_COUNT_MAX, "saturate at max");
    } else {
        panic!("expected SpawnParticle");
    }
}

#[test]
fn coalesce_preserves_non_particle_payloads() {
    let reqs = vec![
        VfxEventRequest::new(
            DVec3::ZERO,
            VfxEventPayloadV1::PlayAnim {
                target_player: TEST_UUID.to_string(),
                anim_id: "bong:sword_swing".to_string(),
                priority: 1000,
                fade_in_ticks: Some(3),
            },
        ),
        make_particle_request("bong:sword_qi_slash", [10.0, 64.0, 10.0], 4),
        make_particle_request("bong:sword_qi_slash", [10.1, 64.0, 10.2], 4),
    ];
    let out = coalesce_requests(reqs);
    assert_eq!(out.len(), 2, "particles merge, anim passes through");
    // 顺序:particles 先,others 后
    assert!(matches!(
        out[0].payload,
        VfxEventPayloadV1::SpawnParticle { .. }
    ));
    assert!(matches!(out[1].payload, VfxEventPayloadV1::PlayAnim { .. }));
}

// ========== is_within_vfx_broadcast_radius ==========

#[test]
fn within_radius_at_zero_distance() {
    let origin = DVec3::new(0.0, 64.0, 0.0);
    assert!(is_within_vfx_broadcast_radius(origin, origin));
}

#[test]
fn within_radius_just_under_limit() {
    let origin = DVec3::new(0.0, 64.0, 0.0);
    let target = DVec3::new(63.9, 64.0, 0.0);
    assert!(is_within_vfx_broadcast_radius(origin, target));
}

#[test]
fn within_radius_at_exact_boundary() {
    // distance_squared == 4096 == 64*64；<= 判定把正好 64 的人也纳入，避免边界抖动
    let origin = DVec3::new(0.0, 64.0, 0.0);
    let target = DVec3::new(64.0, 64.0, 0.0);
    assert!(is_within_vfx_broadcast_radius(origin, target));
}

#[test]
fn out_of_radius_beyond_limit() {
    let origin = DVec3::new(0.0, 64.0, 0.0);
    let target = DVec3::new(64.5, 64.0, 0.0);
    assert!(!is_within_vfx_broadcast_radius(origin, target));
}

#[test]
fn out_of_radius_via_vertical_only() {
    let origin = DVec3::new(0.0, 0.0, 0.0);
    let target = DVec3::new(0.0, 100.0, 0.0);
    assert!(!is_within_vfx_broadcast_radius(origin, target));
}

#[test]
fn within_radius_via_diagonal() {
    let origin = DVec3::new(0.0, 0.0, 0.0);
    // sqrt(30^2 + 30^2 + 30^2) ≈ 51.96，仍 ≤ 64
    let target = DVec3::new(30.0, 30.0, 30.0);
    assert!(is_within_vfx_broadcast_radius(origin, target));
}

#[test]
fn out_of_radius_via_diagonal() {
    let origin = DVec3::new(0.0, 0.0, 0.0);
    // sqrt(40^2 + 40^2 + 40^2) ≈ 69.28 > 64
    let target = DVec3::new(40.0, 40.0, 40.0);
    assert!(!is_within_vfx_broadcast_radius(origin, target));
}

#[test]
fn tribulation_boundary_uses_extended_vfx_radius() {
    let origin = DVec3::new(0.0, 64.0, 0.0);
    let target = DVec3::new(100.0, 64.0, 0.0);
    assert!(is_within_vfx_broadcast_radius_for_event(
        Some("bong:tribulation_boundary"),
        origin,
        target,
    ));
    assert!(!is_within_vfx_broadcast_radius_for_event(
        Some("bong:tribulation_lightning"),
        origin,
        target,
    ));
}

#[test]
fn realm_collapse_boundary_uses_zone_scale_vfx_radius() {
    let origin = DVec3::new(128.0, 65.0, 128.0);
    let target = DVec3::new(384.0, 65.0, 128.0);
    assert!(is_within_vfx_broadcast_radius_for_event(
        Some("bong:realm_collapse_boundary"),
        origin,
        target,
    ));
    assert!(!is_within_vfx_broadcast_radius_for_event(
        Some("bong:tribulation_lightning"),
        origin,
        target,
    ));
}

// ========== parse_vfx_debug_command ==========

#[test]
fn parse_play_with_defaults() {
    match parse_vfx_debug_command("/bong-vfx play bong:sword_swing_horiz", test_uuid()) {
        VfxDebugCommand::Play {
            payload:
                VfxEventPayloadV1::PlayAnim {
                    target_player,
                    anim_id,
                    priority,
                    fade_in_ticks,
                },
        } => {
            assert_eq!(target_player, TEST_UUID);
            assert_eq!(anim_id, "bong:sword_swing_horiz");
            assert_eq!(priority, DEFAULT_DEBUG_PRIORITY);
            assert_eq!(fade_in_ticks, Some(DEFAULT_DEBUG_FADE_IN_TICKS));
        }
        other => panic!("expected Play, got {other:?}"),
    }
}

#[test]
fn parse_play_with_explicit_priority_and_fade() {
    match parse_vfx_debug_command("/bong-vfx play bong:meditate_sit 500 10", test_uuid()) {
        VfxDebugCommand::Play {
            payload:
                VfxEventPayloadV1::PlayAnim {
                    priority,
                    fade_in_ticks,
                    ..
                },
        } => {
            assert_eq!(priority, 500);
            assert_eq!(fade_in_ticks, Some(10));
        }
        other => panic!("expected Play, got {other:?}"),
    }
}

#[test]
fn parse_play_clamps_priority_above_max() {
    match parse_vfx_debug_command("/bong-vfx play bong:foo 9999", test_uuid()) {
        VfxDebugCommand::Play {
            payload: VfxEventPayloadV1::PlayAnim { priority, .. },
        } => assert_eq!(priority, VFX_ANIM_PRIORITY_MAX),
        other => panic!("expected Play, got {other:?}"),
    }
}

#[test]
fn parse_play_clamps_priority_below_min() {
    match parse_vfx_debug_command("/bong-vfx play bong:foo 10", test_uuid()) {
        VfxDebugCommand::Play {
            payload: VfxEventPayloadV1::PlayAnim { priority, .. },
        } => assert_eq!(priority, VFX_ANIM_PRIORITY_MIN),
        other => panic!("expected Play, got {other:?}"),
    }
}

#[test]
fn parse_play_clamps_fade_ticks_above_max() {
    match parse_vfx_debug_command("/bong-vfx play bong:foo 1000 99", test_uuid()) {
        VfxDebugCommand::Play {
            payload: VfxEventPayloadV1::PlayAnim { fade_in_ticks, .. },
        } => assert_eq!(fade_in_ticks, Some(VFX_FADE_TICKS_MAX)),
        other => panic!("expected Play, got {other:?}"),
    }
}

#[test]
fn parse_play_rejects_anim_id_without_colon() {
    match parse_vfx_debug_command("/bong-vfx play sword_swing", test_uuid()) {
        VfxDebugCommand::Usage(hint) => assert!(hint.contains("namespace:path")),
        other => panic!("expected Usage, got {other:?}"),
    }
}

#[test]
fn parse_play_rejects_anim_id_empty_parts() {
    match parse_vfx_debug_command("/bong-vfx play :path", test_uuid()) {
        VfxDebugCommand::Usage(_) => {}
        other => panic!("expected Usage, got {other:?}"),
    }
    match parse_vfx_debug_command("/bong-vfx play bong:", test_uuid()) {
        VfxDebugCommand::Usage(_) => {}
        other => panic!("expected Usage, got {other:?}"),
    }
}

#[test]
fn parse_missing_subcommand_returns_usage() {
    match parse_vfx_debug_command("/bong-vfx", test_uuid()) {
        VfxDebugCommand::Usage(_) => {}
        other => panic!("expected Usage, got {other:?}"),
    }
}

#[test]
fn parse_unknown_subcommand_returns_usage() {
    match parse_vfx_debug_command("/bong-vfx foobar", test_uuid()) {
        VfxDebugCommand::Usage(_) => {}
        other => panic!("expected Usage, got {other:?}"),
    }
}

#[test]
fn parse_play_missing_anim_id_returns_usage() {
    match parse_vfx_debug_command("/bong-vfx play", test_uuid()) {
        VfxDebugCommand::Usage(_) => {}
        other => panic!("expected Usage, got {other:?}"),
    }
}

// ========== /bong-vfx particle ==========

fn test_origin() -> [f64; 3] {
    [42.0, 64.0, -7.0]
}

#[test]
fn parse_particle_with_defaults() {
    match parse_vfx_debug_command_with_origin(
        "/bong-vfx particle bong:sword_qi_slash",
        test_uuid(),
        test_origin(),
    ) {
        VfxDebugCommand::Play {
            payload:
                VfxEventPayloadV1::SpawnParticle {
                    event_id,
                    origin,
                    color,
                    strength,
                    count,
                    duration_ticks,
                    direction,
                },
        } => {
            assert_eq!(event_id, "bong:sword_qi_slash");
            assert_eq!(origin, test_origin());
            assert!(color.is_none(), "color not provided -> None");
            assert_eq!(strength, Some(DEFAULT_PARTICLE_STRENGTH));
            assert_eq!(count, Some(DEFAULT_PARTICLE_COUNT));
            assert_eq!(duration_ticks, Some(DEFAULT_PARTICLE_DURATION_TICKS));
            assert_eq!(direction, Some([1.0, 0.0, 0.0]));
        }
        other => panic!("expected SpawnParticle, got {other:?}"),
    }
}

#[test]
fn parse_particle_with_color_strength_count() {
    match parse_vfx_debug_command_with_origin(
        "/bong-vfx particle bong:sword_qi_slash #ffaa00 0.5 3",
        test_uuid(),
        test_origin(),
    ) {
        VfxDebugCommand::Play {
            payload:
                VfxEventPayloadV1::SpawnParticle {
                    color,
                    strength,
                    count,
                    ..
                },
        } => {
            assert_eq!(color.as_deref(), Some("#ffaa00"));
            assert_eq!(strength, Some(0.5));
            assert_eq!(count, Some(3));
        }
        other => panic!("expected SpawnParticle, got {other:?}"),
    }
}

#[test]
fn parse_particle_rejects_bad_color() {
    match parse_vfx_debug_command_with_origin(
        "/bong-vfx particle bong:x nothex",
        test_uuid(),
        test_origin(),
    ) {
        VfxDebugCommand::Usage(hint) => assert!(hint.contains("#RRGGBB")),
        other => panic!("expected Usage, got {other:?}"),
    }
}

#[test]
fn parse_particle_rejects_strength_out_of_range() {
    match parse_vfx_debug_command_with_origin(
        "/bong-vfx particle bong:x #ffffff 1.5",
        test_uuid(),
        test_origin(),
    ) {
        VfxDebugCommand::Usage(hint) => assert!(hint.contains("strength")),
        other => panic!("expected Usage, got {other:?}"),
    }
}

#[test]
fn parse_particle_rejects_bad_event_id() {
    match parse_vfx_debug_command_with_origin(
        "/bong-vfx particle sword_qi",
        test_uuid(),
        test_origin(),
    ) {
        VfxDebugCommand::Usage(hint) => assert!(hint.contains("namespace:path")),
        other => panic!("expected Usage, got {other:?}"),
    }
}

#[test]
fn parse_particle_clamps_count_above_max() {
    match parse_vfx_debug_command_with_origin(
        "/bong-vfx particle bong:x #ffffff 0.5 9999",
        test_uuid(),
        test_origin(),
    ) {
        VfxDebugCommand::Play {
            payload: VfxEventPayloadV1::SpawnParticle { count, .. },
        } => assert_eq!(count, Some(VFX_PARTICLE_COUNT_MAX)),
        other => panic!("expected SpawnParticle, got {other:?}"),
    }
}

#[test]
fn particle_command_builds_serializable_payload() {
    let cmd = parse_vfx_debug_command_with_origin(
        "/bong-vfx particle bong:sword_qi_slash #88ccff 0.8 2",
        test_uuid(),
        test_origin(),
    );
    let VfxDebugCommand::Play { payload } = cmd else {
        panic!("expected Play, got {cmd:?}");
    };
    let event = VfxEventV1::new(payload);
    let bytes = event
        .to_json_bytes_checked()
        .expect("particle debug payload should serialize");
    let back: VfxEventV1 = serde_json::from_slice(&bytes).unwrap();
    match back.payload {
        VfxEventPayloadV1::SpawnParticle {
            event_id,
            origin,
            color,
            strength,
            count,
            ..
        } => {
            assert_eq!(event_id, "bong:sword_qi_slash");
            assert_eq!(origin, test_origin());
            assert_eq!(color.as_deref(), Some("#88ccff"));
            assert_eq!(strength, Some(0.8));
            assert_eq!(count, Some(2));
        }
        other => panic!("expected SpawnParticle, got {other:?}"),
    }
}

// ========== payload 构造端到端（不入 ECS）==========

#[test]
fn play_command_builds_serializable_payload() {
    let cmd = parse_vfx_debug_command("/bong-vfx play bong:meditate_sit 800 5", test_uuid());
    let VfxDebugCommand::Play { payload } = cmd else {
        panic!("expected Play, got {cmd:?}");
    };
    let event = VfxEventV1::new(payload);
    let bytes = event
        .to_json_bytes_checked()
        .expect("debug-built payload should serialize");
    // 反序列化回来应当 roundtrip（同一 UUID、anim_id、priority、fade_in_ticks）
    let back: VfxEventV1 = serde_json::from_slice(&bytes).expect("json should be valid");
    match back.payload {
        VfxEventPayloadV1::PlayAnim {
            target_player,
            anim_id,
            priority,
            fade_in_ticks,
        } => {
            assert_eq!(target_player, TEST_UUID);
            assert_eq!(anim_id, "bong:meditate_sit");
            assert_eq!(priority, 800);
            assert_eq!(fade_in_ticks, Some(5));
        }
        other => panic!("expected PlayAnim, got {other:?}"),
    }
}

// ========== emit_vfx_event_payloads ECS 集成 ==========
//
// 两个 mock client 分别放在半径内外；系统应当只把 CustomPayloadS2c 发给近的那个。
