use crate::cultivation::components::Realm;
use crate::schema::realm_vision::{FogShapeV1, RealmVisionParamsV1};

pub const FLOOR_CLAMP_M: f64 = 15.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RealmVisionStatusModifier {
    Meditation,
    Demonic,
    Enlightenment,
    TribulationNear,
    Poison,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RealmVisionEnvModifier {
    Night,
    RainOrSnow,
    FogZone,
    NegativeZone,
    LeylineRich,
}

pub fn compute_base_params(realm: Realm) -> RealmVisionParamsV1 {
    let spec = base_spec(realm);
    RealmVisionParamsV1 {
        fog_start: spec.clear_m,
        fog_end: fog_end_for(spec.clear_m, spec.fog_end_m, spec.view_distance_chunks),
        fog_color_rgb: spec.fog_color_rgb,
        fog_shape: spec.fog_shape,
        vignette_alpha: spec.vignette_alpha,
        tint_color_argb: spec.tint_color_argb,
        particle_density: spec.particle_density,
        transition_ticks: 100,
        server_view_distance_chunks: spec.view_distance_chunks,
        post_fx_sharpen: spec.post_fx_sharpen,
    }
}

pub fn compute_vision_params(
    realm: Realm,
    status_modifiers: &[RealmVisionStatusModifier],
    env_modifiers: &[RealmVisionEnvModifier],
) -> RealmVisionParamsV1 {
    let mut params = compute_base_params(realm);
    apply_env_modifiers(&mut params, realm, env_modifiers);
    apply_status_modifiers(&mut params, status_modifiers);
    params
}

pub fn realm_to_chunks(realm: Realm) -> u8 {
    base_spec(realm).view_distance_chunks
}

pub fn realm_rank(realm: Realm) -> u8 {
    match realm {
        Realm::Awaken => 0,
        Realm::Induce => 1,
        Realm::Condense => 2,
        Realm::Solidify => 3,
        Realm::Spirit => 4,
        Realm::Void => 5,
    }
}

pub fn view_distance_limit_m(chunks: u8) -> f64 {
    f64::from(chunks) * 16.0 - 4.0
}

pub fn clamp_clear_distance(clear_m: f64, chunks: u8) -> f64 {
    clear_m.clamp(FLOOR_CLAMP_M, view_distance_limit_m(chunks))
}

pub fn fog_end_for(clear_m: f64, requested_end_m: f64, chunks: u8) -> f64 {
    requested_end_m
        .max(clear_m)
        .min(view_distance_limit_m(chunks))
}

pub fn apply_status_modifiers(
    params: &mut RealmVisionParamsV1,
    modifiers: &[RealmVisionStatusModifier],
) {
    let mut multiplier = 1.0;
    for modifier in modifiers {
        multiplier *= match modifier {
            RealmVisionStatusModifier::Meditation => 1.5,
            RealmVisionStatusModifier::Demonic => 0.5,
            RealmVisionStatusModifier::Enlightenment => 3.0,
            RealmVisionStatusModifier::TribulationNear => 0.7,
            RealmVisionStatusModifier::Poison => 0.8,
        };
    }
    scale_clear_distance(params, multiplier);
}

pub fn apply_env_modifiers(
    params: &mut RealmVisionParamsV1,
    realm: Realm,
    modifiers: &[RealmVisionEnvModifier],
) {
    let mut multiplier = 1.0;
    for modifier in modifiers {
        multiplier *= match modifier {
            RealmVisionEnvModifier::Night => 0.3,
            RealmVisionEnvModifier::RainOrSnow => 0.5,
            RealmVisionEnvModifier::FogZone => 0.4,
            RealmVisionEnvModifier::NegativeZone => {
                0.5 * (1.0 - 0.15 * f64::from(realm_rank(realm))).max(0.25)
            }
            RealmVisionEnvModifier::LeylineRich => 1.2,
        };
    }
    scale_clear_distance(params, multiplier);
}

fn scale_clear_distance(params: &mut RealmVisionParamsV1, multiplier: f64) {
    let ratio = if params.fog_start > 0.0 {
        params.fog_end / params.fog_start
    } else {
        2.0
    };
    params.fog_start = clamp_clear_distance(
        params.fog_start * multiplier,
        params.server_view_distance_chunks,
    );
    params.fog_end = fog_end_for(
        params.fog_start,
        params.fog_start * ratio,
        params.server_view_distance_chunks,
    );
}

#[derive(Debug, Clone, Copy)]
struct RealmVisionBaseSpec {
    clear_m: f64,
    fog_end_m: f64,
    view_distance_chunks: u8,
    fog_shape: FogShapeV1,
    fog_color_rgb: u32,
    vignette_alpha: f64,
    tint_color_argb: u32,
    particle_density: f64,
    post_fx_sharpen: f64,
}

fn base_spec(realm: Realm) -> RealmVisionBaseSpec {
    match realm {
        Realm::Awaken => RealmVisionBaseSpec {
            clear_m: 30.0,
            fog_end_m: 60.0,
            view_distance_chunks: 4,
            fog_shape: FogShapeV1::Cylinder,
            fog_color_rgb: 0xB8B0A8,
            vignette_alpha: 0.55,
            tint_color_argb: 0x0FF0EDE8,
            particle_density: 0.0,
            post_fx_sharpen: 0.0,
        },
        Realm::Induce => RealmVisionBaseSpec {
            clear_m: 50.0,
            fog_end_m: 96.0,
            view_distance_chunks: 6,
            fog_shape: FogShapeV1::Cylinder,
            fog_color_rgb: 0xB0B5B8,
            vignette_alpha: 0.45,
            tint_color_argb: 0,
            particle_density: 0.05,
            post_fx_sharpen: 0.0,
        },
        Realm::Condense => RealmVisionBaseSpec {
            clear_m: 80.0,
            fog_end_m: 128.0,
            view_distance_chunks: 8,
            fog_shape: FogShapeV1::Sphere,
            fog_color_rgb: 0xA8B0BC,
            vignette_alpha: 0.35,
            tint_color_argb: 0x0AE8F0FA,
            particle_density: 0.20,
            post_fx_sharpen: 0.0,
        },
        Realm::Solidify => RealmVisionBaseSpec {
            clear_m: 120.0,
            fog_end_m: 192.0,
            view_distance_chunks: 12,
            fog_shape: FogShapeV1::Sphere,
            fog_color_rgb: 0x9CA8B8,
            vignette_alpha: 0.22,
            tint_color_argb: 0,
            particle_density: 0.45,
            post_fx_sharpen: 0.0,
        },
        Realm::Spirit => RealmVisionBaseSpec {
            clear_m: 180.0,
            fog_end_m: 256.0,
            view_distance_chunks: 16,
            fog_shape: FogShapeV1::Sphere,
            fog_color_rgb: 0x8898AA,
            vignette_alpha: 0.10,
            tint_color_argb: 0,
            particle_density: 0.65,
            post_fx_sharpen: 0.2,
        },
        Realm::Void => RealmVisionBaseSpec {
            clear_m: 240.0,
            fog_end_m: 320.0,
            view_distance_chunks: 20,
            fog_shape: FogShapeV1::Sphere,
            fog_color_rgb: 0x7888A0,
            vignette_alpha: 0.0,
            tint_color_argb: 0x05FFF8E8,
            particle_density: 0.85,
            post_fx_sharpen: 0.65,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REALMS: [Realm; 6] = [
        Realm::Awaken,
        Realm::Induce,
        Realm::Condense,
        Realm::Solidify,
        Realm::Spirit,
        Realm::Void,
    ];

    #[test]
    fn base_clear_distance_per_realm() {
        let actual: Vec<f64> = REALMS
            .into_iter()
            .map(|realm| compute_base_params(realm).fog_start)
            .collect();
        assert_eq!(actual, vec![30.0, 50.0, 80.0, 120.0, 180.0, 240.0]);
    }

    #[test]
    fn view_distance_per_realm() {
        let actual: Vec<u8> = REALMS.into_iter().map(realm_to_chunks).collect();
        assert_eq!(actual, vec![4, 6, 8, 12, 16, 20]);
    }

    #[test]
    fn final_clear_clamp_to_view_distance() {
        let mut params = compute_base_params(Realm::Void);
        apply_status_modifiers(&mut params, &[RealmVisionStatusModifier::Enlightenment]);
        assert_eq!(params.fog_start, view_distance_limit_m(20));
        assert!(params.fog_end <= view_distance_limit_m(20));
    }

    #[test]
    fn negative_zone_realm_scaling() {
        let awaken =
            compute_vision_params(Realm::Awaken, &[], &[RealmVisionEnvModifier::NegativeZone]);
        let void = compute_vision_params(Realm::Void, &[], &[RealmVisionEnvModifier::NegativeZone]);
        assert_eq!(awaken.fog_start, 15.0);
        assert_eq!(void.fog_start, 30.0);
    }

    #[test]
    fn floor_clamp() {
        let params = compute_vision_params(
            Realm::Awaken,
            &[],
            &[
                RealmVisionEnvModifier::Night,
                RealmVisionEnvModifier::RainOrSnow,
                RealmVisionEnvModifier::FogZone,
            ],
        );
        assert_eq!(params.fog_start, FLOOR_CLAMP_M);
    }
}
