//! P3: `bong:void_erosion_visual` S2C CustomPayload emitter.
//!
//! Emits [`VoidErosionVisualSyncPayloadV1`] to nearby clients when:
//! - VoidErosion stage changes (VoidErosionAdvanceEvent)
//! - Ambient vortex toggles on/off
//!
//! Also schedules echo replay VfxEventRequest when ScheduledEcho fires.

use std::collections::HashMap;

use valence::prelude::{
    ident, Client, Entity, EventReader, EventWriter, Local, Position, Query, Res, UniqueId, With,
};

use crate::combat::components::TICKS_PER_SECOND;
use crate::combat::woliu_v2::erosion::{
    erosion_model_alpha, should_show_sound_distortion_overlay, VoidErosion,
    VoidErosionAdvanceEvent, VoidErosionStage, ECHO_REPLAY_VFX_SCALE,
};
use crate::combat::CombatClock;
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::schema::common::MAX_PAYLOAD_BYTES;
use crate::schema::server_data::ServerDataBuildError;
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::schema::woliu_erosion::VoidErosionVisualSyncPayloadV1;

#[allow(dead_code)]
pub const VOID_EROSION_VISUAL_CHANNEL: &str = "bong:void_erosion_visual";
const VISUAL_SYNC_BROADCAST_RADIUS: f64 = 64.0;
/// Periodic sync interval (every 5 seconds) for ambient_active state drift.
const PERIODIC_SYNC_INTERVAL_TICKS: u64 = 5 * TICKS_PER_SECOND;

#[derive(Default)]
pub struct VoidErosionVisualEmitCache {
    last_stage: HashMap<Entity, VoidErosionStage>,
    last_ambient: HashMap<Entity, bool>,
}

/// Emits `bong:void_erosion_visual` payloads on stage change or ambient toggle.
pub fn emit_void_erosion_visual_sync(
    clock: Res<CombatClock>,
    mut cache: Local<VoidErosionVisualEmitCache>,
    mut advance_events: EventReader<VoidErosionAdvanceEvent>,
    erosion_query: Query<(Entity, &VoidErosion, &Position, Option<&UniqueId>)>,
    mut clients: Query<(Entity, &mut Client, &Position), With<Client>>,
) {
    let periodic = clock.tick.is_multiple_of(PERIODIC_SYNC_INTERVAL_TICKS);
    // Collect entities that need a sync this tick
    let mut entities_to_sync: Vec<Entity> = Vec::new();

    // Stage advance events always trigger sync
    for event in advance_events.read() {
        entities_to_sync.push(event.entity);
    }

    // Check for ambient toggle changes and periodic sync
    for (entity, erosion, _pos, _uid) in &erosion_query {
        let prev_ambient = cache.last_ambient.get(&entity).copied().unwrap_or(false);
        let prev_stage = cache
            .last_stage
            .get(&entity)
            .copied()
            .unwrap_or(VoidErosionStage::None);
        let stage_changed = erosion.stage != prev_stage;
        let ambient_changed = erosion.ambient_active != prev_ambient;
        if (stage_changed || ambient_changed || periodic) && !entities_to_sync.contains(&entity) {
            entities_to_sync.push(entity);
        }
        cache.last_stage.insert(entity, erosion.stage);
        cache.last_ambient.insert(entity, erosion.ambient_active);
    }

    // Prune cache entries for entities no longer in the query
    let active: std::collections::HashSet<Entity> = erosion_query.iter().map(|(e, ..)| e).collect();
    cache.last_stage.retain(|e, _| active.contains(e));
    cache.last_ambient.retain(|e, _| active.contains(e));

    // Emit payloads
    for sync_entity in &entities_to_sync {
        let Ok((_, erosion, erosion_pos, unique_id)) = erosion_query.get(*sync_entity) else {
            continue;
        };
        let entity_id = unique_id
            .map(|uid| uid.0.to_string())
            .unwrap_or_else(|| format!("entity:{}", sync_entity.to_bits()));
        let payload = VoidErosionVisualSyncPayloadV1 {
            entity_id,
            stage: erosion.stage.as_index() as u8,
            cumulative_erosion: erosion.cumulative_erosion,
            ambient_active: erosion.ambient_active,
            model_alpha: erosion_model_alpha(erosion.stage),
            sound_distortion_active: should_show_sound_distortion_overlay(erosion.stage),
            server_tick: clock.tick,
        };
        let Ok(bytes) = serialize_visual_payload(&payload) else {
            tracing::warn!(
                "[void-erosion-visual] serialize failed for {}",
                payload.entity_id
            );
            continue;
        };
        let erosion_origin = erosion_pos.get();
        for (_client_entity, mut client, client_pos) in &mut clients {
            if client_pos.get().distance(erosion_origin)
                <= VISUAL_SYNC_BROADCAST_RADIUS + f64::EPSILON
            {
                client.send_custom_payload(ident!("bong:void_erosion_visual"), &bytes);
            }
        }
    }
}

/// Emit echo replay VfxEventRequest with the ECHO_REPLAY_VFX_SCALE parameter.
///
/// Called from tick system when a ScheduledEcho fires. The VfxEventRequest
/// carries scale information via `count` and `strength` fields (scale=0.4).
#[allow(dead_code)]
pub fn emit_echo_replay_vfx(
    events: &mut EventWriter<VfxEventRequest>,
    center: valence::prelude::DVec3,
    original_event_id: &str,
    color: &str,
) {
    let scaled_count = ((24.0 * ECHO_REPLAY_VFX_SCALE) as u16).max(4);
    events.send(VfxEventRequest::new(
        center,
        VfxEventPayloadV1::SpawnParticle {
            event_id: format!("{original_event_id}_echo"),
            origin: [center.x, center.y + 1.0, center.z],
            direction: None,
            color: Some(color.to_string()),
            strength: Some(ECHO_REPLAY_VFX_SCALE),
            count: Some(scaled_count),
            duration_ticks: Some(15),
        },
    ));
}

fn serialize_visual_payload(
    payload: &VoidErosionVisualSyncPayloadV1,
) -> Result<Vec<u8>, ServerDataBuildError> {
    let bytes = serde_json::to_vec(payload).map_err(ServerDataBuildError::Json)?;
    if bytes.len() > MAX_PAYLOAD_BYTES {
        return Err(ServerDataBuildError::Oversize {
            size: bytes.len(),
            max: MAX_PAYLOAD_BYTES,
        });
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visual_sync_payload_serializes() {
        let payload = VoidErosionVisualSyncPayloadV1 {
            entity_id: "test_player".to_string(),
            stage: 2,
            cumulative_erosion: 95.5,
            ambient_active: false,
            model_alpha: 0.7,
            sound_distortion_active: false,
            server_tick: 1000,
        };
        let bytes = serialize_visual_payload(&payload).expect("should serialize");
        assert!(
            !bytes.is_empty(),
            "serialized visual payload should be non-empty"
        );
        let back: VoidErosionVisualSyncPayloadV1 =
            serde_json::from_slice(&bytes).expect("should deserialize");
        assert_eq!(
            payload, back,
            "visual payload round-trip through serialize_visual_payload"
        );
    }

    #[test]
    fn echo_replay_vfx_scale_is_correct() {
        assert!(
            (ECHO_REPLAY_VFX_SCALE - 0.4).abs() < f32::EPSILON,
            "echo replay scale should be 0.4, got {}",
            ECHO_REPLAY_VFX_SCALE
        );
    }

    #[test]
    fn visual_sync_channel_name() {
        assert_eq!(
            VOID_EROSION_VISUAL_CHANNEL, "bong:void_erosion_visual",
            "channel name must match plan spec"
        );
    }
}
