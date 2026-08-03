use std::time::{SystemTime, UNIX_EPOCH};

use valence::prelude::{Changed, Client, Entity, Query, Res, Username, With};

use crate::combat::components::{SkillBarBindings, SkillSlot};
use crate::combat::CombatClock;
use crate::cultivation::known_techniques::TechniqueRegistry;
use crate::inventory::{
    ItemRegistry, PlayerInventory, DEFAULT_CAST_DURATION_MS as TEMPLATE_DEFAULT_CAST_MS,
    DEFAULT_COOLDOWN_MS as TEMPLATE_DEFAULT_COOLDOWN_MS,
};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::combat_hud::{SkillBarConfigV1, SkillBarEntryV1};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

const TICK_MS: u64 = 50;

type SkillBarEmitFilter = (With<Client>, Changed<SkillBarBindings>);

pub fn emit_skillbar_config_payloads(
    clock: Res<CombatClock>,
    item_registry: Res<ItemRegistry>,
    technique_registry: Res<TechniqueRegistry>,
    mut clients: Query<
        (
            Entity,
            &mut Client,
            &Username,
            &SkillBarBindings,
            Option<&PlayerInventory>,
        ),
        SkillBarEmitFilter,
    >,
) {
    let now_ms = current_unix_millis();
    let now_tick = clock.tick;

    for (entity, mut client, username, bindings, inventory) in &mut clients {
        let config = build_skillbar_config(
            bindings,
            inventory,
            &item_registry,
            &technique_registry,
            now_tick,
            now_ms,
        );
        let payload = ServerDataV1::new(ServerDataPayloadV1::SkillBarConfig(config));
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
            "[bong][network] sent {} {} payload to entity {entity:?} for `{}`",
            SERVER_DATA_CHANNEL,
            payload_type,
            username.0
        );
    }
}

fn build_skillbar_config(
    bindings: &SkillBarBindings,
    inventory: Option<&PlayerInventory>,
    item_registry: &ItemRegistry,
    technique_registry: &TechniqueRegistry,
    now_tick: u64,
    now_ms: u64,
) -> SkillBarConfigV1 {
    let mut slots = Vec::with_capacity(SkillBarBindings::SLOT_COUNT);
    let mut cooldown_until_ms = Vec::with_capacity(SkillBarBindings::SLOT_COUNT);
    for i in 0..SkillBarBindings::SLOT_COUNT {
        let entry = match &bindings.slots[i] {
            SkillSlot::Empty => None,
            SkillSlot::Item { instance_id } => inventory.and_then(|inventory| {
                let template_id = lookup_template_id(inventory, *instance_id)?;
                let template = item_registry.get(&template_id);
                let display_name = template
                    .map(|template| template.display_name.clone())
                    .unwrap_or_else(|| template_id.clone());
                let cast_duration_ms = template
                    .map(|template| template.cast_duration_ms)
                    .unwrap_or(TEMPLATE_DEFAULT_CAST_MS);
                let cooldown_ms = template
                    .map(|template| template.cooldown_ms)
                    .unwrap_or(TEMPLATE_DEFAULT_COOLDOWN_MS);
                Some(SkillBarEntryV1::Item {
                    template_id,
                    display_name,
                    cast_duration_ms,
                    cooldown_ms,
                    icon_texture: String::new(),
                })
            }),
            SkillSlot::Skill { skill_id } => {
                technique_registry
                    .get(skill_id)
                    .map(|definition| SkillBarEntryV1::Skill {
                        skill_id: skill_id.clone(),
                        display_name: definition.display_name.to_string(),
                        cast_duration_ms: definition.cast_ticks.saturating_mul(TICK_MS as u32),
                        cooldown_ms: definition.cooldown_ticks.saturating_mul(TICK_MS as u32),
                        icon_texture: definition.icon_texture.to_string(),
                    })
            }
        };
        slots.push(entry);
        let cd_tick = match &bindings.slots[i] {
            SkillSlot::Skill { skill_id } => bindings.cooldowns.get(skill_id).copied().unwrap_or(0),
            SkillSlot::Item { .. } | SkillSlot::Empty => 0,
        };
        let cd_until_ms = if cd_tick > now_tick {
            now_ms.saturating_add((cd_tick - now_tick).saturating_mul(TICK_MS))
        } else {
            0
        };
        cooldown_until_ms.push(cd_until_ms);
    }

    SkillBarConfigV1 {
        slots,
        cooldown_until_ms,
    }
}

fn lookup_template_id(inventory: &PlayerInventory, instance_id: u64) -> Option<String> {
    for container in &inventory.containers {
        if let Some(placed) = container
            .items
            .iter()
            .find(|placed| placed.instance.instance_id == instance_id)
        {
            return Some(placed.instance.template_id.clone());
        }
    }
    if let Some(item) = inventory
        .equipped
        .values()
        .flat_map(|s| s.iter_all())
        .find(|item| item.instance_id == instance_id)
    {
        return Some(item.template_id.clone());
    }
    inventory
        .hotbar
        .iter()
        .flatten()
        .find(|item| item.instance_id == instance_id)
        .map(|item| item.template_id.clone())
}

fn current_unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_uses_injected_technique_metadata_and_skill_cooldown() {
        let registry =
            TechniqueRegistry::load_for_tests_with_override("sword.cleave", |definition| {
                definition.display_name = "运行时覆写劈".to_string();
                definition.cast_ticks = 17;
                definition.cooldown_ticks = 83;
                definition.icon_texture =
                    "bong:textures/gui/skill/runtime_override.png".to_string();
            });
        let mut bindings = SkillBarBindings::default();
        assert!(bindings.set(
            2,
            SkillSlot::Skill {
                skill_id: "sword.cleave".to_string(),
            },
        ));
        bindings.set_cooldown("sword.cleave", 120);

        let config = build_skillbar_config(
            &bindings,
            None,
            &ItemRegistry::default(),
            &registry,
            100,
            10_000,
        );

        assert_eq!(config.slots.len(), SkillBarBindings::SLOT_COUNT);
        assert!(config.slots[..2].iter().all(Option::is_none));
        assert!(config.slots[3..].iter().all(Option::is_none));
        assert_eq!(config.cooldown_until_ms[2], 11_000);
        assert!(config
            .cooldown_until_ms
            .iter()
            .enumerate()
            .all(|(index, value)| index == 2 || *value == 0));
        match config.slots[2]
            .as_ref()
            .expect("skill slot must be emitted")
        {
            SkillBarEntryV1::Skill {
                skill_id,
                display_name,
                cast_duration_ms,
                cooldown_ms,
                icon_texture,
            } => {
                assert_eq!(skill_id, "sword.cleave");
                assert_eq!(display_name, "运行时覆写劈");
                assert_eq!(*cast_duration_ms, 850);
                assert_eq!(*cooldown_ms, 4_150);
                assert_eq!(icon_texture, "bong:textures/gui/skill/runtime_override.png");
            }
            SkillBarEntryV1::Item { .. } => panic!("skill binding emitted as item"),
        }
    }
}
