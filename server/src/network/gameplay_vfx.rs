use valence::prelude::{DVec3, Events};

use crate::network::vfx_event_emit::VfxEventRequest;
use crate::schema::vfx_event::VfxEventPayloadV1;

pub const CULTIVATION_ABSORB: &str = "bong:cultivation_absorb";
pub const MERIDIAN_OPEN: &str = "bong:meridian_open";
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
pub const SOCIAL_NICHE_ESTABLISH: &str = "bong:social_niche_establish";
pub const POISON_MIST: &str = "bong:poison_mist";

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
            count: Some(count.clamp(1, 128) as u16),
            duration_ticks: Some(duration_ticks.clamp(1, 200) as u16),
        },
    )
}
