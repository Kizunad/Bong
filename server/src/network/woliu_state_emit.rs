use std::collections::HashMap;

use valence::prelude::{Client, Entity, EventReader, Local, Query, Res, UniqueId, Username, With};

use crate::combat::components::TICKS_PER_SECOND;
use crate::combat::woliu::{
    entity_wire_id, vortex_field_state_payload, ProjectileQiDrainedEvent, VortexField,
};
use crate::combat::woliu_v2::state::{TurbulenceField, VortexV2State};
use crate::combat::CombatClock;
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::cast_emit::current_unix_millis;
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

#[derive(Default)]
pub struct VortexStateEmitCache {
    active: HashMap<Entity, bool>,
    intercepted_count: HashMap<Entity, u32>,
}

type VortexStateClientItem<'a> = (
    Entity,
    &'a mut Client,
    &'a Username,
    Option<&'a UniqueId>,
    Option<&'a VortexField>,
    Option<&'a VortexV2State>,
    Option<&'a TurbulenceField>,
);

pub fn emit_vortex_state_payloads(
    clock: Res<CombatClock>,
    mut cache: Local<VortexStateEmitCache>,
    mut drained_events: EventReader<ProjectileQiDrainedEvent>,
    mut clients: Query<VortexStateClientItem<'_>, With<Client>>,
) {
    for event in drained_events.read() {
        let count = cache
            .intercepted_count
            .entry(event.field_caster)
            .or_default();
        *count = count.saturating_add(1);
    }

    let periodic = clock.tick.is_multiple_of(TICKS_PER_SECOND);
    for (entity, mut client, username, unique_id, field, v2_state, turbulence) in &mut clients {
        let active = field.is_some() || v2_state.is_some() || turbulence.is_some();
        let previously_active = cache.active.get(&entity).copied().unwrap_or(false);
        if !periodic && active == previously_active {
            continue;
        }

        cache.active.insert(entity, active);
        let intercepted_count = cache
            .intercepted_count
            .get(&entity)
            .copied()
            .unwrap_or_default();
        let mut state = vortex_field_state_payload(
            entity_wire_id(unique_id, entity),
            field,
            clock.tick,
            intercepted_count,
        );
        apply_woliu_v2_state_overlay(&mut state, v2_state, turbulence, clock.tick);
        let payload = ServerDataV1::new(ServerDataPayloadV1::VortexState(state));
        let payload_type = payload_type_label(payload.payload_type());
        let payload_bytes = match serialize_server_data_payload(&payload) {
            Ok(bytes) => bytes,
            Err(error) => {
                log_payload_build_error(payload_type, &error);
                continue;
            }
        };
        send_server_data_payload(&mut client, payload_bytes.as_slice());
        tracing::debug!(
            "[bong][network] sent {} {} payload to entity {entity:?} for `{}` active={active}",
            SERVER_DATA_CHANNEL,
            payload_type,
            username.0
        );
    }
}

fn apply_woliu_v2_state_overlay(
    state: &mut crate::schema::woliu::VortexFieldStateV1,
    v2_state: Option<&VortexV2State>,
    turbulence: Option<&TurbulenceField>,
    now_tick: u64,
) {
    let now_ms = current_unix_millis();
    if let Some(v2_state) = v2_state {
        state.active = true;
        state.active_skill_id = v2_state.active_skill_kind.as_str().to_string();
        state.radius = v2_state.lethal_radius;
        state.charge_progress = 1.0;
        state.cooldown_until_ms =
            ticks_from_now_to_ms(now_ms, now_tick, v2_state.cooldown_until_tick);
        state.backfire_level = v2_state
            .backfire_level
            .map(backfire_level_label)
            .unwrap_or_default()
            .to_string();
        state.turbulence_radius = v2_state.turbulence_radius;
        state.turbulence_intensity = v2_state.turbulence_intensity;
    }
    if let Some(turbulence) = turbulence {
        state.active = true;
        state.center = [
            turbulence.center.x,
            turbulence.center.y,
            turbulence.center.z,
        ];
        state.turbulence_radius = turbulence.radius;
        state.turbulence_intensity = turbulence.intensity;
        state.turbulence_until_ms = turbulence_until_ms(now_ms, turbulence);
    }
}

fn ticks_from_now_to_ms(now_ms: u64, now_tick: u64, until_tick: u64) -> u64 {
    let millis_per_tick = 1_000_u64 / TICKS_PER_SECOND.max(1);
    now_ms.saturating_add(
        until_tick
            .saturating_sub(now_tick)
            .saturating_mul(millis_per_tick),
    )
}

fn turbulence_until_ms(now_ms: u64, turbulence: &TurbulenceField) -> u64 {
    if turbulence.remaining_swirl_qi <= f32::EPSILON {
        return 0;
    }
    let rate = f64::from(turbulence.decay_rate_per_second).max(0.001);
    let seconds = ((f64::from(turbulence.remaining_swirl_qi) / f64::from(f32::EPSILON))
        .ln()
        .max(0.0)
        / rate)
        .clamp(0.0, 300.0);
    now_ms.saturating_add((seconds * 1000.0) as u64)
}

fn backfire_level_label(level: crate::combat::woliu_v2::BackfireLevel) -> &'static str {
    match level {
        crate::combat::woliu_v2::BackfireLevel::Sensation => "sensation",
        crate::combat::woliu_v2::BackfireLevel::MicroTear => "micro_tear",
        crate::combat::woliu_v2::BackfireLevel::Torn => "torn",
        crate::combat::woliu_v2::BackfireLevel::Severed => "severed",
    }
}

#[cfg(test)]
mod tests {
    use valence::prelude::{DVec3, Entity};

    use crate::combat::woliu::vortex_field_state_payload;
    use crate::combat::woliu_v2::state::{TurbulenceField, VortexV2State};
    use crate::combat::woliu_v2::{BackfireLevel, WoliuSkillId};

    use super::apply_woliu_v2_state_overlay;

    #[test]
    fn woliu_v2_overlay_populates_vortex_state_hud_fields() {
        let mut state = vortex_field_state_payload("entity:1".to_string(), None, 10, 0);
        let v2_state = VortexV2State {
            active_skill_kind: WoliuSkillId::Heart,
            heart_passive_enabled: false,
            lethal_radius: 5.0,
            influence_radius: 300.0,
            turbulence_radius: 75.0,
            turbulence_intensity: 0.8,
            backfire_level: Some(BackfireLevel::MicroTear),
            started_at_tick: 10,
            active_until_tick: 40,
            cooldown_until_tick: 110,
        };
        let turbulence = TurbulenceField::new(
            Entity::from_raw(1),
            DVec3::new(3.0, 64.0, 4.0),
            12.0,
            0.75,
            100.0,
            10,
        );

        apply_woliu_v2_state_overlay(&mut state, Some(&v2_state), Some(&turbulence), 10);

        assert!(state.active);
        assert_eq!(state.active_skill_id, "woliu.heart");
        assert_eq!(state.charge_progress, 1.0);
        assert!(state.cooldown_until_ms > 0);
        assert_eq!(state.backfire_level, "micro_tear");
        assert_eq!(state.center, [3.0, 64.0, 4.0]);
        assert_eq!(state.radius, 5.0);
        assert_eq!(state.turbulence_radius, 12.0);
        assert_eq!(state.turbulence_intensity, 0.75);
        assert!(state.turbulence_until_ms > 0);
    }
}
