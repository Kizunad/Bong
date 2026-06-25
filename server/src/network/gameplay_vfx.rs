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
}
