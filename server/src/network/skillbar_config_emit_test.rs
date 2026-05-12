use std::collections::HashMap;

use valence::prelude::{App, Update};
use valence::protocol::packets::play::CustomPayloadS2c;
use valence::testing::create_mock_client;

use super::skillbar_config_emit::emit_skillbar_config_payloads;
use crate::combat::components::{SkillBarBindings, SkillSlot};
use crate::combat::CombatClock;
use crate::inventory::{
    ContainerState, InventoryRevision, ItemCategory, ItemInstance, ItemRarity, ItemRegistry,
    ItemTemplate, PlacedItemState, PlayerInventory,
};
use crate::network::agent_bridge::SERVER_DATA_CHANNEL;
use crate::schema::combat_hud::SkillBarEntryV1;
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

fn empty_inventory() -> PlayerInventory {
    PlayerInventory {
        revision: InventoryRevision(0),
        containers: vec![ContainerState {
            id: "main_pack".to_string(),
            name: "main_pack".to_string(),
            rows: 5,
            cols: 7,
            items: Vec::new(),
        }],
        equipped: Default::default(),
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 50.0,
    }
}

fn template(id: &str, display_name: &str) -> ItemTemplate {
    ItemTemplate {
        id: id.to_string(),
        display_name: display_name.to_string(),
        category: ItemCategory::Pill,
        max_stack_count: 1,
        grid_w: 1,
        grid_h: 1,
        base_weight: 0.1,
        rarity: ItemRarity::Common,
        spirit_quality_initial: 1.0,
        description: String::new(),
        effect: None,
        cast_duration_ms: 250,
        cooldown_ms: 1250,
        weapon_spec: None,
        forge_station_spec: None,
        blueprint_scroll_spec: None,
        inscription_scroll_spec: None,
        technique_scroll_spec: None,
    }
}

fn item_instance(instance_id: u64, template_id: &str) -> ItemInstance {
    ItemInstance {
        instance_id,
        template_id: template_id.to_string(),
        display_name: template_id.to_string(),
        grid_w: 1,
        grid_h: 1,
        weight: 0.1,
        rarity: ItemRarity::Common,
        description: String::new(),
        stack_count: 1,
        spirit_quality: 1.0,
        durability: 1.0,
        freshness: None,
        mineral_id: None,
        charges: None,
        forge_quality: None,
        forge_color: None,
        forge_side_effects: Vec::new(),
        forge_achieved_tier: None,
        alchemy: None,
        lingering_owner_qi: None,
    }
}

#[test]
fn skillbar_config_emit_serializes_skill_item_and_cooldown() {
    let mut app = App::new();
    app.insert_resource(CombatClock { tick: 10 });
    app.insert_resource(ItemRegistry::from_map(HashMap::from([(
        "tea".to_string(),
        template("tea", "清茶"),
    )])));
    app.add_systems(Update, emit_skillbar_config_payloads);

    let (client_bundle, mut helper) = create_mock_client("Azure");
    let mut inventory = empty_inventory();
    inventory.containers[0].items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance: item_instance(42, "tea"),
    });
    let mut bindings = SkillBarBindings::default();
    assert!(bindings.set(
        0,
        SkillSlot::Skill {
            skill_id: "burst_meridian.beng_quan".to_string(),
        },
    ));
    assert!(bindings.set(1, SkillSlot::Item { instance_id: 42 }));
    bindings.set_cooldown(0, 70);
    app.world_mut().spawn((client_bundle, bindings, inventory));

    app.update();
    let mut client_query = app.world_mut().query::<&mut valence::prelude::Client>();
    for mut client in client_query.iter_mut(app.world_mut()) {
        client
            .flush_packets()
            .expect("mock client packets should flush");
    }

    let mut received = Vec::new();
    for frame in helper.collect_received().0 {
        let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
            continue;
        };
        if packet.channel.as_str() != SERVER_DATA_CHANNEL {
            continue;
        }
        let payload: ServerDataV1 =
            serde_json::from_slice(packet.data.0 .0).expect("server_data payload should decode");
        received.push(payload);
    }

    let skillbar = received
        .into_iter()
        .find_map(|payload| match payload.payload {
            ServerDataPayloadV1::SkillBarConfig(config) => Some(config),
            _ => None,
        })
        .expect("skillbar_config should be emitted");
    assert_eq!(skillbar.slots.len(), 9);
    assert!(matches!(
        &skillbar.slots[0],
        Some(SkillBarEntryV1::Skill { skill_id, display_name, cast_duration_ms, cooldown_ms, .. })
            if skill_id == "burst_meridian.beng_quan"
                && display_name == "崩拳"
                && *cast_duration_ms == 400
                && *cooldown_ms == 3000
    ));
    assert!(matches!(
        &skillbar.slots[1],
        Some(SkillBarEntryV1::Item { template_id, display_name, cast_duration_ms, cooldown_ms, .. })
            if template_id == "tea"
                && display_name == "清茶"
                && *cast_duration_ms == 250
                && *cooldown_ms == 1250
    ));
    assert!(skillbar.cooldown_until_ms[0] > 0);
}
