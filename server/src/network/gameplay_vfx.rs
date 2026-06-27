use valence::prelude::{DVec3, Events};

use crate::network::vfx_event_emit::VfxEventRequest;
use crate::schema::vfx_event::{
    VfxEventPayloadV1, VFX_PARTICLE_COUNT_MAX, VFX_PARTICLE_DURATION_TICKS_MAX,
};

pub const CULTIVATION_ABSORB: &str = "bong:cultivation_absorb";
pub const MERIDIAN_OPEN: &str = "bong:meridian_open";
pub const BREAKTHROUGH_PILLAR: &str = "bong:breakthrough_pillar";
pub const BREAKTHROUGH_FAIL: &str = "bong:breakthrough_fail";
pub const COMBAT_HIT: &str = "bong:combat_hit";
pub const COMBAT_PARRY: &str = "bong:combat_parry";
pub const FORGE_HAMMER_STRIKE: &str = "bong:forge_hammer_strike";
pub const FORGE_INSCRIPTION: &str = "bong:forge_inscription";
pub const FORGE_CONSECRATION: &str = "bong:forge_consecration";
pub const ALCHEMY_BREW_VAPOR: &str = "bong:alchemy_brew_vapor";
pub const ALCHEMY_OVERHEAT: &str = "bong:alchemy_overheat";
pub const ALCHEMY_COMPLETE: &str = "bong:alchemy_complete";
pub const ALCHEMY_EXPLODE: &str = "bong:alchemy_explode";
pub const LINGTIAN_TILL: &str = "bong:lingtian_till";
pub const LINGTIAN_PLANT: &str = "bong:lingtian_plant";
pub const LINGTIAN_REPLENISH: &str = "bong:lingtian_replenish";
pub const ZHENFA_TRAP: &str = "bong:zhenfa_trap";
pub const ZHENFA_WARD: &str = "bong:zhenfa_ward";
pub const ZHENFA_DEPLETE: &str = "bong:zhenfa_deplete";
pub const BEAST_TRAP_SNAP: &str = "bong:beast_trap_snap";
pub const TRIP_WIRE_TRIGGER: &str = "bong:trip_wire_trigger";
pub const DECOY_BREAK: &str = "bong:decoy_break";
pub const DECOY_TAUNT: &str = "bong:decoy_taunt";
pub const LINGJU_ACTIVATE: &str = "bong:lingju_activate";
pub const SCATTER_BURST: &str = "bong:scatter_burst";
pub const NETWORK_ARRAY_FORM: &str = "bong:network_array_form";
pub const NETWORK_ARRAY_BREAK: &str = "bong:network_array_break";
pub const SOCIAL_NICHE_ESTABLISH: &str = "bong:social_niche_establish";
pub const SOCIAL_NICHE_REPAIR: &str = "bong:social_niche_repair";
pub const SOCIAL_PACT_LINK: &str = "bong:social_pact_link";
pub const SOCIAL_FEUD_MARK: &str = "bong:social_feud_mark";
pub const POISON_MIST: &str = "bong:poison_mist";
pub const MOVEMENT_DASH: &str = "bong:movement_dash";
pub const DEAD_DROP_WARD_BREAK: &str = "bong:dead_drop_ward_break";

// plan-tarkov-backpack-v1 P5 — 套包操作差异化视听反馈。三类操作各自独立 event_id，
// client `PackOperationVfxPlayer` 按 event_id 派发到差异化粒子 + 内联 audio recipe
// （落地散落 / 布料窸窣 / 轻 thunk）。三者 event_id / color / count / duration 均不同，
// 由 `pack_move_request` 构造、`classify_pack_move` 判分支，pin 测试断言三类 payload 互不相同。
pub const INVENTORY_PACK_UNEQUIP: &str = "bong:inventory_pack_unequip";
pub const INVENTORY_PACK_EQUIP: &str = "bong:inventory_pack_equip";
pub const INVENTORY_PACK_STOW: &str = "bong:inventory_pack_stow";

pub fn block_center(pos: [i32; 3]) -> DVec3 {
    DVec3::new(
        f64::from(pos[0]) + 0.5,
        f64::from(pos[1]) + 0.5,
        f64::from(pos[2]) + 0.5,
    )
}

pub fn send_spawn(events: &mut Events<VfxEventRequest>, request: VfxEventRequest) {
    events.send(request);
}

pub fn spawn_request(
    event_id: &'static str,
    origin: DVec3,
    direction: Option<[f64; 3]>,
    color: &'static str,
    strength: f32,
    count: u32,
    duration_ticks: u32,
) -> VfxEventRequest {
    VfxEventRequest::new(
        origin,
        VfxEventPayloadV1::SpawnParticle {
            event_id: event_id.to_string(),
            origin: [origin.x, origin.y, origin.z],
            direction,
            color: Some(color.to_string()),
            strength: Some(strength.clamp(0.0, 1.0)),
            count: Some(count.clamp(1, VFX_PARTICLE_COUNT_MAX.into()) as u16),
            duration_ticks: Some(
                duration_ticks.clamp(1, VFX_PARTICLE_DURATION_TICKS_MAX.into()) as u16,
            ),
        },
    )
}

/// plan-tarkov-backpack-v1 P5 — 套包操作三类差异化视听反馈的判别式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackMoveVfx {
    /// 卸下非空/穿戴背包件（worn → 非 worn）：落地音 + 物品散落粒子。
    Unequip,
    /// 装上背包件（非 worn → worn）：布料窸窣音 + 轻柔布料粒子。
    Equip,
    /// 拖入物品到穿戴中的 `pack_<id>` 容器：轻 thunk 音 + 小尘扑。
    Stow,
}

/// 由 `handle_inventory_move` 算出的移动语义布尔位判定 P5 视听反馈类别。
///
/// **接线契约**（pin 测试锁住，server emit 命中正确分支）：
/// - 卸背包：被移走 instance 是背包件 + `from` 在 worn 层 + `to` 不在 worn 层 → `Unequip`
/// - 穿背包：被移走 instance 是背包件 + `to` 在 worn 层 + `from` 不在 worn 层 → `Equip`
/// - 拖入：`to` 是 `pack_<id>` 容器（且非穿/卸背包件本身的 worn 转移）→ `Stow`
/// - 其余移动（格子↔hotbar、非 pack 容器互移等）→ `None`（无套包视听反馈）
///
/// worn 转移优先于 Stow 判定：避免「穿/卸背包件」被误判成「拖入容器」。
pub fn classify_pack_move(
    moved_item_is_pack: bool,
    from_worn: bool,
    to_worn: bool,
    to_is_pack_container: bool,
) -> Option<PackMoveVfx> {
    if moved_item_is_pack && from_worn && !to_worn {
        Some(PackMoveVfx::Unequip)
    } else if moved_item_is_pack && to_worn && !from_worn {
        Some(PackMoveVfx::Equip)
    } else if to_is_pack_container {
        Some(PackMoveVfx::Stow)
    } else {
        None
    }
}

/// 按 `PackMoveVfx` 类别构造差异化 VFX 请求。三类的 event_id / 方向 / 颜色 / 强度 / 数量 /
/// lifetime 全部不同——pin 测试断言三者 payload 互不相同（禁单方向 stub）。
///
/// - `Unequip`：暗草褐 `#7A6A3A`，16 粒，22 tick，向下散落（背包砸地连货散开）。
/// - `Equip`：柔草绿 `#9CA87E`，8 粒，14 tick，向上轻飘（布料上身窸窣）。
/// - `Stow`：浅褐 `#B0A878`，5 粒，10 tick，小幅上扑（物品入包轻顿）。
pub fn pack_move_request(kind: PackMoveVfx, origin: DVec3) -> VfxEventRequest {
    let (event_id, direction, color, strength, count, duration): (
        &'static str,
        [f64; 3],
        &'static str,
        f32,
        u32,
        u32,
    ) = match kind {
        PackMoveVfx::Unequip => (
            INVENTORY_PACK_UNEQUIP,
            [0.0, -1.0, 0.0],
            "#7A6A3A",
            0.85,
            16,
            22,
        ),
        PackMoveVfx::Equip => (
            INVENTORY_PACK_EQUIP,
            [0.0, 1.0, 0.0],
            "#9CA87E",
            0.55,
            8,
            14,
        ),
        PackMoveVfx::Stow => (INVENTORY_PACK_STOW, [0.0, 1.0, 0.0], "#B0A878", 0.40, 5, 10),
    };
    spawn_request(
        event_id,
        origin,
        Some(direction),
        color,
        strength,
        count,
        duration,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::vfx_event::{VfxEventBuildError, VfxEventV1};

    #[derive(Debug, Clone, Copy)]
    enum ExpectedParticleRangeError {
        Count(u16),
        Duration(u16),
    }

    fn matches_expected_particle_range_error(
        actual: &VfxEventBuildError,
        expected: ExpectedParticleRangeError,
    ) -> bool {
        match (actual, expected) {
            (
                VfxEventBuildError::ParticleCountOutOfRange { count },
                ExpectedParticleRangeError::Count(expected_count),
            ) => *count == expected_count,
            (
                VfxEventBuildError::ParticleDurationOutOfRange { ticks },
                ExpectedParticleRangeError::Duration(expected_ticks),
            ) => *ticks == expected_ticks,
            _ => false,
        }
    }

    #[test]
    fn spawn_request_clamps_particle_ranges_to_schema_contract_boundaries() {
        let cases = [
            (0, 0, 1, 1, "zero input clamps to schema minimum"),
            (1, 1, 1, 1, "schema minimum passes through unchanged"),
            (
                VFX_PARTICLE_COUNT_MAX as u32,
                VFX_PARTICLE_DURATION_TICKS_MAX as u32,
                VFX_PARTICLE_COUNT_MAX,
                VFX_PARTICLE_DURATION_TICKS_MAX,
                "schema maximum passes through unchanged",
            ),
            (
                VFX_PARTICLE_COUNT_MAX as u32 + 1,
                VFX_PARTICLE_DURATION_TICKS_MAX as u32 + 1,
                VFX_PARTICLE_COUNT_MAX,
                VFX_PARTICLE_DURATION_TICKS_MAX,
                "max plus one clamps to schema maximum",
            ),
            (
                128,
                999,
                VFX_PARTICLE_COUNT_MAX,
                VFX_PARTICLE_DURATION_TICKS_MAX,
                "legacy high gameplay inputs clamp to schema maximum",
            ),
        ];

        for (input_count, input_duration, expected_count, expected_duration, reason) in cases {
            let request = spawn_request(
                BREAKTHROUGH_PILLAR,
                DVec3::new(1.0, 2.0, 3.0),
                Some([0.0, 1.0, 0.0]),
                "#FFE8A0",
                1.5,
                input_count,
                input_duration,
            );

            let VfxEventPayloadV1::SpawnParticle {
                strength,
                count,
                duration_ticks,
                ..
            } = &request.payload
            else {
                panic!("spawn_request must build SpawnParticle payload");
            };

            assert_eq!(
                *strength,
                Some(1.0),
                "expected strength 1.0 because spawn_request clamps gameplay intensity to schema range, actual {strength:?}"
            );
            assert_eq!(
                *count,
                Some(expected_count),
                "expected count {expected_count} because {reason}, actual {count:?}"
            );
            assert_eq!(
                *duration_ticks,
                Some(expected_duration),
                "expected duration_ticks {expected_duration} because {reason}, actual {duration_ticks:?}"
            );
            VfxEventV1::new(request.payload)
                .to_json_bytes_checked()
                .expect("gameplay VFX helper should produce schema-valid payloads");
        }
    }

    #[test]
    fn checked_serializer_rejects_invalid_particle_ranges() {
        let invalid_cases = [
            (
                Some(0),
                Some(1),
                "count zero is below schema minimum",
                ExpectedParticleRangeError::Count(0),
            ),
            (
                Some(VFX_PARTICLE_COUNT_MAX + 1),
                Some(1),
                "count max plus one exceeds schema maximum",
                ExpectedParticleRangeError::Count(VFX_PARTICLE_COUNT_MAX + 1),
            ),
            (
                Some(1),
                Some(0),
                "duration zero is below schema minimum",
                ExpectedParticleRangeError::Duration(0),
            ),
            (
                Some(1),
                Some(VFX_PARTICLE_DURATION_TICKS_MAX + 1),
                "duration max plus one exceeds schema maximum",
                ExpectedParticleRangeError::Duration(VFX_PARTICLE_DURATION_TICKS_MAX + 1),
            ),
        ];

        for (count, duration_ticks, reason, expected_error) in invalid_cases {
            let payload = VfxEventPayloadV1::SpawnParticle {
                event_id: BREAKTHROUGH_PILLAR.to_string(),
                origin: [1.0, 2.0, 3.0],
                direction: None,
                color: Some("#FFE8A0".to_string()),
                strength: Some(1.0),
                count,
                duration_ticks,
            };
            let err = VfxEventV1::new(payload)
                .to_json_bytes_checked()
                .expect_err("invalid particle range should be rejected by checked serializer");
            assert!(
                matches_expected_particle_range_error(&err, expected_error),
                "expected checked serializer error {expected_error:?} because {reason}, actual {err:?}"
            );
        }
    }

    // ── plan-tarkov-backpack-v1 P5 — 套包操作差异化视听反馈 pin 测试 ────────────

    fn spawn_particle_fields(
        request: &VfxEventRequest,
    ) -> (String, Option<String>, Option<u16>, Option<u16>) {
        let VfxEventPayloadV1::SpawnParticle {
            event_id,
            color,
            count,
            duration_ticks,
            ..
        } = &request.payload
        else {
            panic!("pack_move_request must build SpawnParticle payload");
        };
        (event_id.clone(), color.clone(), *count, *duration_ticks)
    }

    #[test]
    fn classify_pack_move_routes_each_branch_to_distinct_category() {
        // 卸背包：背包件 + worn → 非 worn。
        assert_eq!(
            classify_pack_move(true, true, false, false),
            Some(PackMoveVfx::Unequip),
            "背包件从 worn 层移到非 worn 应判定为卸下（落地散落反馈）"
        );
        // 卸背包即便落点恰是 pack_ 容器，worn 转移仍优先（不退化成 Stow）。
        assert_eq!(
            classify_pack_move(true, true, false, true),
            Some(PackMoveVfx::Unequip),
            "卸背包优先于拖入判定，避免穿/卸被误判成 Stow"
        );
        // 穿背包：背包件 + 非 worn → worn。
        assert_eq!(
            classify_pack_move(true, false, true, false),
            Some(PackMoveVfx::Equip),
            "背包件移入 worn 层应判定为穿上（布料窸窣反馈）"
        );
        // 拖入：普通物品落入 pack_ 容器（非背包件 worn 转移）。
        assert_eq!(
            classify_pack_move(false, false, false, true),
            Some(PackMoveVfx::Stow),
            "普通物品落入穿戴 pack_ 容器应判定为拖入（轻 thunk 反馈）"
        );
        // 非套包移动（格子↔hotbar、非 pack 容器）无反馈。
        assert_eq!(
            classify_pack_move(false, false, false, false),
            None,
            "非套包移动不应触发任何套包视听反馈"
        );
        // 背包件在两个非 worn 位置间挪（捡起再放回格子）也不算穿/卸。
        assert_eq!(
            classify_pack_move(true, false, false, false),
            None,
            "背包件在非 worn 位置间移动不触发穿/卸反馈"
        );
    }

    #[test]
    fn pack_move_request_payloads_are_mutually_distinct() {
        let origin = DVec3::new(4.0, 65.0, -2.0);
        let unequip = spawn_particle_fields(&pack_move_request(PackMoveVfx::Unequip, origin));
        let equip = spawn_particle_fields(&pack_move_request(PackMoveVfx::Equip, origin));
        let stow = spawn_particle_fields(&pack_move_request(PackMoveVfx::Stow, origin));

        // event_id 必须逐类不同——否则 client 无法把三类反馈派发到差异化 player。
        assert_eq!(unequip.0, INVENTORY_PACK_UNEQUIP);
        assert_eq!(equip.0, INVENTORY_PACK_EQUIP);
        assert_eq!(stow.0, INVENTORY_PACK_STOW);
        assert_ne!(unequip.0, equip.0, "卸/装 event_id 不能相同");
        assert_ne!(unequip.0, stow.0, "卸/拖入 event_id 不能相同");
        assert_ne!(equip.0, stow.0, "装/拖入 event_id 不能相同");

        // 三类 payload 整体（event_id+color+count+duration）必须互不相同——
        // 单方向 stub（三类发同款 payload）撞红。
        assert_ne!(
            unequip, equip,
            "卸/装 payload 必须差异化（color/count/duration）"
        );
        assert_ne!(unequip, stow, "卸/拖入 payload 必须差异化");
        assert_ne!(equip, stow, "装/拖入 payload 必须差异化");

        // 量级方向锁定：卸下散落最多最久、拖入最少最短（强度递减直觉）。
        assert!(
            unequip.2 > equip.2 && equip.2 > stow.2,
            "粒子数量应卸下>装上>拖入，实际 {:?}/{:?}/{:?}",
            unequip.2,
            equip.2,
            stow.2
        );
        assert!(
            unequip.3 > equip.3 && equip.3 > stow.3,
            "lifetime 应卸下>装上>拖入，实际 {:?}/{:?}/{:?}",
            unequip.3,
            equip.3,
            stow.3
        );
    }

    #[test]
    fn pack_move_request_payloads_serialize_within_schema_contract() {
        let origin = DVec3::new(1.0, 70.0, 1.0);
        for kind in [PackMoveVfx::Unequip, PackMoveVfx::Equip, PackMoveVfx::Stow] {
            let request = pack_move_request(kind, origin);
            VfxEventV1::new(request.payload)
                .to_json_bytes_checked()
                .unwrap_or_else(|err| {
                    panic!(
                        "pack_move_request({kind:?}) must serialize within schema range: {err:?}"
                    )
                });
        }
    }
}
