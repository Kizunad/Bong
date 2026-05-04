//! Rift-mouth local negative pressure tick.
//!
//! `zone.spirit_qi < 0` remains handled by `negative_zone`; this module consumes
//! raster-local `neg_pressure + portal_anchor_sdf` hot-spots so rift mouths and
//! cave/abyssal internal entrances can drain qi without turning the whole zone
//! into a negative zone.

use valence::prelude::{DVec3, EventWriter, Position, Query, Res};

use crate::network::vfx_event_emit::VfxEventRequest;
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::terrain::TerrainProviders;

use super::components::{Cultivation, Realm};

pub const HOTSPOT_RADIUS_BLOCKS: f32 = 30.0;
pub const FULL_PULL_NEG_PRESSURE: f32 = 0.8;
pub const TICKS_PER_SECOND: f64 = 20.0;
pub const FROST_BREATH_EVENT_ID: &str = "bong:frost_breath";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NegPressureField {
    pub center: [f64; 2],
    pub max_pull: f32,
    pub falloff: f32,
}

impl NegPressureField {
    pub fn strength_at(&self, x: f64, z: f64) -> f32 {
        let dx = x - self.center[0];
        let dz = z - self.center[1];
        let distance = (dx * dx + dz * dz).sqrt() as f32;
        if distance > self.falloff || self.falloff <= 0.0 {
            return 0.0;
        }
        let t = 1.0 - distance / self.falloff;
        (self.max_pull * t).clamp(0.0, self.max_pull)
    }
}

pub fn qi_drain_per_sec(realm: Realm) -> f64 {
    match realm {
        Realm::Awaken => 0.0,
        Realm::Induce => 2.0,
        Realm::Condense => 5.0,
        Realm::Solidify => 10.0,
        Realm::Spirit => 25.0,
        Realm::Void => 60.0,
    }
}

pub fn drain_per_tick(realm: Realm, neg_pressure: f32, portal_anchor_sdf: f32) -> f64 {
    if portal_anchor_sdf > HOTSPOT_RADIUS_BLOCKS || neg_pressure <= 0.0 {
        return 0.0;
    }
    let pressure_scale = (neg_pressure / FULL_PULL_NEG_PRESSURE).clamp(0.0, 1.0) as f64;
    qi_drain_per_sec(realm) * pressure_scale / TICKS_PER_SECOND
}

pub fn frost_breath_payload(origin: DVec3, strength: f32) -> VfxEventPayloadV1 {
    VfxEventPayloadV1::SpawnParticle {
        event_id: FROST_BREATH_EVENT_ID.to_string(),
        origin: [origin.x, origin.y + 1.6, origin.z],
        direction: Some([0.0, 1.0, 0.0]),
        color: Some("#CFEFFF".to_string()),
        strength: Some(strength.clamp(0.15, 1.0)),
        count: Some(4),
        duration_ticks: Some(20),
    }
}

pub fn tick_neg_pressure(
    providers: Option<Res<TerrainProviders>>,
    mut actors: Query<(&Position, Option<&CurrentDimension>, &mut Cultivation)>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    let Some(providers) = providers else {
        return;
    };

    for (pos, current_dimension, mut cultivation) in actors.iter_mut() {
        let dimension = current_dimension
            .map(|current| current.0)
            .unwrap_or(DimensionKind::Overworld);
        let Some(provider) = providers.for_dimension(dimension) else {
            continue;
        };

        let sample = provider.sample(pos.0.x.floor() as i32, pos.0.z.floor() as i32);
        let drain = drain_per_tick(
            cultivation.realm,
            sample.neg_pressure,
            sample.portal_anchor_sdf,
        );
        if drain <= 0.0 {
            continue;
        }

        cultivation.qi_current = (cultivation.qi_current - drain).max(0.0);
        let strength = (sample.neg_pressure / FULL_PULL_NEG_PRESSURE).clamp(0.15, 1.0);
        vfx_events.send(VfxEventRequest::new(
            pos.0,
            frost_breath_payload(pos.0, strength),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn realm_drain_table_matches_rift_mouth_plan() {
        assert_eq!(qi_drain_per_sec(Realm::Awaken), 0.0);
        assert_eq!(qi_drain_per_sec(Realm::Induce), 2.0);
        assert_eq!(qi_drain_per_sec(Realm::Condense), 5.0);
        assert_eq!(qi_drain_per_sec(Realm::Solidify), 10.0);
        assert_eq!(qi_drain_per_sec(Realm::Spirit), 25.0);
        assert_eq!(qi_drain_per_sec(Realm::Void), 60.0);
    }

    #[test]
    fn drain_per_tick_uses_full_pull_at_point_eight_pressure() {
        let drain = drain_per_tick(Realm::Solidify, 0.8, 0.0);
        assert!((drain - 0.5).abs() < 1e-9);
    }

    #[test]
    fn drain_per_tick_gates_outside_portal_anchor_radius() {
        assert_eq!(drain_per_tick(Realm::Void, 0.8, 30.1), 0.0);
        assert_eq!(drain_per_tick(Realm::Void, 0.0, 0.0), 0.0);
    }

    #[test]
    fn neg_pressure_field_falls_off_from_center() {
        let field = NegPressureField {
            center: [0.0, 0.0],
            max_pull: 0.8,
            falloff: 30.0,
        };
        assert_eq!(field.strength_at(0.0, 0.0), 0.8);
        assert_eq!(field.strength_at(30.1, 0.0), 0.0);
        assert!(field.strength_at(15.0, 0.0) > 0.0);
    }

    #[test]
    fn frost_breath_payload_uses_dedicated_event_id() {
        let payload = frost_breath_payload(DVec3::new(1.0, 64.0, 2.0), 0.8);
        match payload {
            VfxEventPayloadV1::SpawnParticle {
                event_id,
                origin,
                color,
                ..
            } => {
                assert_eq!(event_id, FROST_BREATH_EVENT_ID);
                assert_eq!(origin, [1.0, 65.6, 2.0]);
                assert_eq!(color.as_deref(), Some("#CFEFFF"));
            }
            other => panic!("expected SpawnParticle, got {other:?}"),
        }
    }
}
