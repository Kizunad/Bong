//! World-space NPC dialogue bubble bridge (`bong:npc_bubble`).

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use valence::entity::EntityId;
use valence::prelude::{
    bevy_ecs, ident, Client, Entity, EventReader, Position, Query, ResMut, Resource, With, Without,
};

use crate::combat::components::{Lifecycle, LifecycleState};
use crate::combat::events::CombatEvent;
use crate::npc::interaction_memory::{
    memory_bubble_text, should_emit_memory_bubble, NpcMemoryComponent,
};
use crate::npc::lifecycle::NpcArchetype;
use crate::npc::spawn::NpcMarker;
use crate::schema::common::MAX_PAYLOAD_BYTES;
use crate::schema::server_data::ServerDataBuildError;

pub const NPC_BUBBLE_SYNC_RADIUS: f64 = 25.0;
pub const NPC_BUBBLE_SYNC_INTERVAL_TICKS: u64 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NpcBubbleType {
    Greeting,
    Reaction,
    Warning,
    Memory,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NpcBubbleS2c {
    pub v: u8,
    #[serde(rename = "type")]
    pub ty: String,
    pub entity_id: i32,
    pub text: String,
    pub duration_ticks: u32,
    pub bubble_type: NpcBubbleType,
}

impl NpcBubbleS2c {
    pub fn to_json_bytes_checked(&self) -> Result<Vec<u8>, ServerDataBuildError> {
        let bytes = serde_json::to_vec(self).map_err(ServerDataBuildError::Json)?;
        if bytes.len() > MAX_PAYLOAD_BYTES {
            return Err(ServerDataBuildError::Oversize {
                size: bytes.len(),
                max: MAX_PAYLOAD_BYTES,
            });
        }
        Ok(bytes)
    }
}

#[derive(Debug, Default, Resource)]
pub struct NpcBubbleSyncState {
    tick: u64,
    greeted_pairs: HashSet<(Entity, Entity)>,
}

type ClientBubbleItem<'a> = (Entity, &'a mut Client, &'a Position);
type NpcBubbleItem<'a> = (
    Entity,
    &'a EntityId,
    &'a Position,
    &'a NpcArchetype,
    Option<&'a NpcMemoryComponent>,
    Option<&'a Lifecycle>,
);
type CombatNpcBubbleItem<'a> = (
    &'a EntityId,
    &'a Position,
    &'a NpcArchetype,
    Option<&'a Lifecycle>,
);

pub fn emit_npc_bubble_payloads(
    mut state: ResMut<NpcBubbleSyncState>,
    mut clients: Query<ClientBubbleItem<'_>, With<Client>>,
    npcs: Query<NpcBubbleItem<'_>, (With<NpcMarker>, Without<Client>)>,
) {
    state.tick = state.tick.saturating_add(1);
    if !state.tick.is_multiple_of(NPC_BUBBLE_SYNC_INTERVAL_TICKS) {
        return;
    }

    let mut active_pairs = HashSet::new();
    let radius_sq = NPC_BUBBLE_SYNC_RADIUS * NPC_BUBBLE_SYNC_RADIUS;
    for (client_entity, mut client, client_position) in &mut clients {
        for (npc_entity, entity_id, npc_position, archetype, memory, lifecycle) in &npcs {
            if lifecycle.is_some_and(|lifecycle| lifecycle.state == LifecycleState::Terminated) {
                continue;
            }
            if client_position.get().distance_squared(npc_position.get()) > radius_sq {
                continue;
            }

            let pair = (client_entity, npc_entity);
            active_pairs.insert(pair);
            if !state.greeted_pairs.insert(pair) {
                continue;
            }

            let (text, bubble_type) = bubble_content_for_pair(
                entity_id.get(),
                format!("{client_entity:?}").as_str(),
                *archetype,
                memory,
            );
            let payload = build_npc_bubble(entity_id.get(), text, bubble_type);
            let bytes = match payload.to_json_bytes_checked() {
                Ok(bytes) => bytes,
                Err(error) => {
                    tracing::warn!(
                        "[bong][npc_bubble] dropping npc bubble entity_id={}: {error:?}",
                        entity_id.get()
                    );
                    continue;
                }
            };
            client.send_custom_payload(ident!("bong:npc_bubble"), &bytes);
        }
    }

    state
        .greeted_pairs
        .retain(|pair| active_pairs.contains(pair));
}

pub fn emit_npc_reaction_bubbles(
    mut events: EventReader<CombatEvent>,
    mut clients: Query<ClientBubbleItem<'_>, With<Client>>,
    npcs: Query<CombatNpcBubbleItem<'_>, (With<NpcMarker>, Without<Client>)>,
) {
    let radius_sq = NPC_BUBBLE_SYNC_RADIUS * NPC_BUBBLE_SYNC_RADIUS;
    for event in events.read() {
        if event.damage <= 0.0 {
            continue;
        }
        let Ok((entity_id, npc_position, archetype, lifecycle)) = npcs.get(event.target) else {
            continue;
        };
        if lifecycle.is_some_and(|lifecycle| lifecycle.state == LifecycleState::Terminated) {
            continue;
        }

        let payload = build_npc_bubble(
            entity_id.get(),
            bubble_text_by_archetype(*archetype, NpcBubbleType::Reaction),
            NpcBubbleType::Reaction,
        );
        let bytes = match payload.to_json_bytes_checked() {
            Ok(bytes) => bytes,
            Err(error) => {
                tracing::warn!(
                    "[bong][npc_bubble] dropping reaction bubble entity_id={}: {error:?}",
                    entity_id.get()
                );
                continue;
            }
        };
        let origin = npc_position.get();
        for (_, mut client, client_position) in &mut clients {
            if client_position.get().distance_squared(origin) <= radius_sq {
                client.send_custom_payload(ident!("bong:npc_bubble"), &bytes);
            }
        }
    }
}

pub fn build_npc_bubble(
    entity_id: i32,
    text: impl Into<String>,
    bubble_type: NpcBubbleType,
) -> NpcBubbleS2c {
    let text = clamp_bubble_text(text.into());
    NpcBubbleS2c {
        v: 1,
        ty: "npc_bubble".to_string(),
        entity_id,
        duration_ticks: duration_ticks_for_text(&text),
        text,
        bubble_type,
    }
}

pub fn duration_ticks_for_text(text: &str) -> u32 {
    let char_count = text.chars().count() as f64;
    let seconds = (char_count * 0.15).clamp(3.0, 6.0);
    (seconds * 20.0).round() as u32
}

pub fn bubble_text_by_archetype(
    archetype: NpcArchetype,
    bubble_type: NpcBubbleType,
) -> &'static str {
    match bubble_type {
        NpcBubbleType::Warning => match archetype {
            NpcArchetype::Daoxiang | NpcArchetype::Zhinian | NpcArchetype::Fuya => "...",
            NpcArchetype::Beast => "它压低身子，喉间发出低吼。",
            _ => "...别再近了。",
        },
        NpcBubbleType::Reaction => match archetype {
            NpcArchetype::Rogue | NpcArchetype::Disciple => "你找死！",
            NpcArchetype::Commoner => "大仙饶命！",
            NpcArchetype::GuardianRelic => "退。",
            _ => "...",
        },
        NpcBubbleType::Memory => match archetype {
            NpcArchetype::Rogue => "...你。上次的骨币，成色不对。",
            NpcArchetype::Commoner => "大仙饶命！小人再不敢了...",
            NpcArchetype::GuardianRelic => "...又来？",
            _ => "...",
        },
        NpcBubbleType::Greeting => match archetype {
            NpcArchetype::Rogue | NpcArchetype::Disciple => "道友...",
            NpcArchetype::Commoner => "大仙，小人不敢...",
            NpcArchetype::GuardianRelic => "墓前止步。",
            NpcArchetype::Daoxiang => crate::npc::tsy_hostile::dao_chang_fake_friendly_bubble(),
            NpcArchetype::Zhinian => "...这是...",
            NpcArchetype::Fuya => "气息陷下去了。",
            NpcArchetype::SkullFiend => "骨声贴地。",
            NpcArchetype::Beast => "它警惕地盯着你。",
            NpcArchetype::Zombie => "...",
        },
    }
}

fn bubble_content_for_pair<'a>(
    entity_id: i32,
    player_key: &str,
    archetype: NpcArchetype,
    memory: Option<&'a NpcMemoryComponent>,
) -> (&'a str, NpcBubbleType) {
    if let Some(entry) = memory.and_then(|memory| memory.interactions.last()) {
        if should_emit_memory_bubble(
            &format!("npc:{entity_id}"),
            player_key,
            memory
                .map(|memory| memory.interactions.len())
                .unwrap_or_default(),
        ) {
            return (memory_bubble_text(archetype, entry), NpcBubbleType::Memory);
        }
    }
    (
        bubble_text_by_archetype(archetype, NpcBubbleType::Greeting),
        NpcBubbleType::Greeting,
    )
}

fn clamp_bubble_text(text: String) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= 96 {
        return trimmed.to_string();
    }
    let mut out: String = trimmed.chars().take(93).collect();
    out.push_str("...");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bubble_text_by_archetype() {
        assert_eq!(
            super::bubble_text_by_archetype(NpcArchetype::Rogue, NpcBubbleType::Greeting),
            "道友..."
        );
        assert_eq!(
            super::bubble_text_by_archetype(NpcArchetype::Daoxiang, NpcBubbleType::Greeting),
            "..."
        );
        assert_eq!(
            super::bubble_text_by_archetype(NpcArchetype::Commoner, NpcBubbleType::Memory),
            "大仙饶命！小人再不敢了..."
        );
    }

    #[test]
    fn bubble_duration_is_text_scaled_and_clamped() {
        assert_eq!(duration_ticks_for_text("短句"), 60);
        assert_eq!(duration_ticks_for_text(&"长".repeat(80)), 120);
    }

    #[test]
    fn bubble_payload_serializes_contract_fields() {
        let payload = build_npc_bubble(42, "道友...", NpcBubbleType::Greeting);
        let json = String::from_utf8(payload.to_json_bytes_checked().expect("serialize"))
            .expect("npc bubble payload should be utf8 json");
        assert!(json.contains(r#""type":"npc_bubble""#));
        assert!(json.contains(r#""entity_id":42"#));
        assert!(json.contains(r#""bubble_type":"greeting""#));
    }
}
