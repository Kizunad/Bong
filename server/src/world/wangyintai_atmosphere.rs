//! P2 忘音台 zone atmosphere configuration.
//!
//! Defines the fog, particle, and sky color constants for the wangyintai zone
//! per plan-woliu-path-v1 §7.6.
//!
//! The zone features:
//! - FogVeil: extreme dark purple fog (fogStart 40, fogEnd 120, density 0.01)
//! - Particle: `void_shimmer` BongSpriteParticle, density 0.2/s, tint #4A2D6E
//! - Sound echo: all player sounds delayed 2s and replayed (volume * 0.6, pitch * 0.7)
//! - Sky RGB shift: (-10, -8, +5) -- dark cold purple
//!
//! Zone entry narration templates are also defined here.

/// Fog start distance (blocks).
pub const WANGYINTAI_FOG_START: f32 = 40.0;
/// Fog end distance (blocks).
pub const WANGYINTAI_FOG_END: f32 = 120.0;
/// Fog density.
pub const WANGYINTAI_FOG_DENSITY: f32 = 0.01;
/// Fog tint RGB: #1A1025 (extreme dark purple -- almost black).
pub const WANGYINTAI_FOG_TINT: [u8; 3] = [0x1A, 0x10, 0x25];

/// Particle type identifier.
pub const WANGYINTAI_PARTICLE_TYPE: &str = "void_shimmer";
/// Particle spawn rate (per second).
pub const WANGYINTAI_PARTICLE_RATE: f32 = 0.2;
/// Particle tint hex: #4A2D6E (dark purple).
pub const WANGYINTAI_PARTICLE_TINT: [u8; 3] = [0x4A, 0x2D, 0x6E];
/// Particle lifetime (ticks).
pub const WANGYINTAI_PARTICLE_LIFETIME_TICKS: u32 = 30;

/// Echo delay for sound replay (ticks, = 2 seconds).
pub const WANGYINTAI_ECHO_DELAY_TICKS: u32 = 40;
/// Echo replay volume multiplier.
pub const WANGYINTAI_ECHO_VOLUME_MULT: f32 = 0.6;
/// Echo replay pitch multiplier.
pub const WANGYINTAI_ECHO_PITCH_MULT: f32 = 0.7;

/// Sky color temperature shift (RGB delta).
pub const WANGYINTAI_SKY_RGB_SHIFT: [i8; 3] = [-10, -8, 5];

/// Zone entry narration templates.
pub mod narration {
    /// First entry narration (scope: player, style: perception).
    pub const ENTRY_FOOTSTEP: &str = "你踩在碎石上的声音——晚了两秒才传到你耳朵里。";

    /// Second entry narration (scope: player, style: perception).
    pub const ENTRY_THIN_AIR: &str =
        "空气很薄。不是缺氧，是缺灵。你能感觉到真元在向某个方向流——但你看不到那个方向。";

    /// All entry narration templates.
    pub const ALL: &[&str] = &[ENTRY_FOOTSTEP, ENTRY_THIN_AIR];
}

/// Builds a FogVeil EnvironmentEffect for the wangyintai zone.
pub fn build_fog_veil(
    zone_min: [f64; 3],
    zone_max: [f64; 3],
) -> crate::world::environment::EnvironmentEffect {
    crate::world::environment::EnvironmentEffect::FogVeil {
        aabb_min: zone_min,
        aabb_max: zone_max,
        tint_rgb: WANGYINTAI_FOG_TINT,
        density: WANGYINTAI_FOG_DENSITY,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fog_tint_matches_plan_spec() {
        assert_eq!(
            WANGYINTAI_FOG_TINT,
            [0x1A, 0x10, 0x25],
            "fog tint should be #1A1025 (extreme dark purple)"
        );
    }

    #[test]
    fn fog_density_is_low() {
        assert!(
            (WANGYINTAI_FOG_DENSITY - 0.01).abs() < f32::EPSILON,
            "fog density should be 0.01 per plan spec"
        );
    }

    #[test]
    fn particle_tint_matches_plan_spec() {
        assert_eq!(
            WANGYINTAI_PARTICLE_TINT,
            [0x4A, 0x2D, 0x6E],
            "particle tint should be #4A2D6E (dark purple)"
        );
    }

    #[test]
    fn particle_lifetime_is_30_ticks() {
        assert_eq!(WANGYINTAI_PARTICLE_LIFETIME_TICKS, 30);
    }

    #[test]
    fn echo_delay_is_2_seconds() {
        assert_eq!(
            WANGYINTAI_ECHO_DELAY_TICKS, 40,
            "echo delay should be 40 ticks (2 seconds at 20 tps)"
        );
    }

    #[test]
    fn echo_volume_and_pitch_multipliers() {
        assert!((WANGYINTAI_ECHO_VOLUME_MULT - 0.6).abs() < f32::EPSILON);
        assert!((WANGYINTAI_ECHO_PITCH_MULT - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn sky_rgb_shift_matches_plan_spec() {
        assert_eq!(
            WANGYINTAI_SKY_RGB_SHIFT,
            [-10, -8, 5],
            "sky RGB shift should be (-10, -8, +5) per plan spec"
        );
    }

    #[test]
    fn entry_narration_count() {
        assert_eq!(narration::ALL.len(), 2);
    }

    #[test]
    fn entry_narrations_match_plan() {
        assert!(narration::ENTRY_FOOTSTEP.contains("晚了两秒"));
        assert!(narration::ENTRY_THIN_AIR.contains("缺灵"));
    }

    #[test]
    fn build_fog_veil_produces_correct_variant() {
        let effect = build_fog_veil([3200.0, 40.0, -2800.0], [4200.0, 200.0, -1800.0]);
        match effect {
            crate::world::environment::EnvironmentEffect::FogVeil {
                tint_rgb, density, ..
            } => {
                assert_eq!(tint_rgb, WANGYINTAI_FOG_TINT);
                assert!((density - WANGYINTAI_FOG_DENSITY).abs() < f32::EPSILON);
            }
            _ => panic!("expected FogVeil variant"),
        }
    }
}
