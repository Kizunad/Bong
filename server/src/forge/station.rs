//! plan-forge-v1 §1.2 锻炉系统。
//!
//! MVP：Component 挂到 BlockEntity（或 placeholder Entity）上，
//! tier 限制能使用的图谱（凡铁砧最高锻法器）。
//!
//! TODO(plan-persistence-v1): 将 `WeaponForgeStation` 接到方块实体存档；预留字段形如
//! `block_entity: Option<BlockEntityRef>`，与 alchemy furnace / lingtian plot 一并落地。

use std::collections::HashSet;

use valence::prelude::{
    bevy_ecs, BlockPos, BlockState, ChunkLayer, Client, Commands, Component, Entity, Event,
    EventReader, Query, Res, Username, With,
};

use super::session::ForgeSessionId;
use crate::cultivation::components::Cultivation;
use crate::inventory::{
    consume_item_instance_once, inventory_item_by_instance, ItemRegistry, PlayerInventory,
};
use crate::network::inventory_snapshot_emit::send_inventory_snapshot_to_client;
use crate::player::state::PlayerState;

/// 砧 tier：1 凡铁 / 2 灵铁 / 3 玄铁 / 4 道砧。
pub type StationTier = u8;

#[derive(Debug, Clone, Component)]
pub struct WeaponForgeStation {
    pub tier: StationTier,
    pub owner: Option<Entity>,
    pub session: Option<ForgeSessionId>,
    pub integrity: f32,
    // TODO(plan-persistence-v1): block_entity: Option<BlockEntityRef>
    pub pos: Option<(i32, i32, i32)>,
}

impl Default for WeaponForgeStation {
    fn default() -> Self {
        Self {
            tier: 1,
            owner: None,
            session: None,
            integrity: 1.0,
            pos: None,
        }
    }
}

impl WeaponForgeStation {
    pub fn with_tier(tier: StationTier) -> Self {
        Self {
            tier,
            ..Default::default()
        }
    }

    pub fn placed(pos: BlockPos, tier: StationTier, owner: Entity) -> Self {
        Self {
            tier,
            owner: Some(owner),
            pos: Some((pos.x, pos.y, pos.z)),
            ..Default::default()
        }
    }

    pub fn block_pos(&self) -> Option<BlockPos> {
        self.pos.map(|(x, y, z)| BlockPos { x, y, z })
    }

    /// 图谱是否可在此砧上使用（本砧 tier ≥ station_tier_min）。
    pub fn can_craft(&self, station_tier_min: StationTier) -> bool {
        self.tier >= station_tier_min && self.integrity > 0.0
    }

    /// 爆炉损耗（clamp 到 0）。
    pub fn apply_wear(&mut self, wear: f32) {
        self.integrity = (self.integrity - wear).max(0.0);
    }

    pub fn is_broken(&self) -> bool {
        self.integrity <= 0.0
    }
}

#[derive(Debug, Clone, Event)]
pub struct PlaceForgeStationRequest {
    pub player: Entity,
    pub pos: BlockPos,
    pub item_instance_id: u64,
    pub station_tier: StationTier,
}

#[allow(clippy::too_many_arguments)]
pub fn handle_place_station_request(
    mut events: EventReader<PlaceForgeStationRequest>,
    mut commands: Commands,
    mut inventories: Query<&mut PlayerInventory>,
    registry: Res<ItemRegistry>,
    mut layers: Query<&mut ChunkLayer, With<crate::world::dimension::OverworldLayer>>,
    existing: Query<&WeaponForgeStation>,
    mut clients: Query<(&Username, &mut Client, &PlayerState, &Cultivation)>,
) {
    let mut placed_this_tick: HashSet<(i32, i32, i32)> = HashSet::new();

    for req in events.read() {
        let pos_key = (req.pos.x, req.pos.y, req.pos.z);
        if placed_this_tick.contains(&pos_key)
            || existing
                .iter()
                .any(|station| station.block_pos() == Some(req.pos))
        {
            tracing::warn!(
                "[bong][forge] place_station rejected: pos={:?} already occupied by another forge station",
                req.pos
            );
            continue;
        }

        let Ok(mut inv) = inventories.get_mut(req.player) else {
            tracing::warn!(
                "[bong][forge] place_station rejected: player={:?} has no PlayerInventory",
                req.player
            );
            continue;
        };
        let Some(instance) = inventory_item_by_instance(&inv, req.item_instance_id) else {
            tracing::warn!(
                "[bong][forge] place_station rejected: instance_id={} not in inventory of {:?}",
                req.item_instance_id,
                req.player
            );
            continue;
        };
        let Some(station_spec) = registry
            .get(instance.template_id.as_str())
            .and_then(|template| template.forge_station_spec.as_ref())
        else {
            tracing::warn!(
                "[bong][forge] place_station rejected: item `{}` is not a forge station",
                instance.template_id
            );
            continue;
        };
        if station_spec.tier != req.station_tier {
            tracing::warn!(
                "[bong][forge] place_station rejected: client tier {} disagrees with item `{}` tier {}",
                req.station_tier,
                instance.template_id,
                station_spec.tier
            );
            continue;
        }
        if let Err(err) = consume_item_instance_once(&mut inv, req.item_instance_id) {
            tracing::warn!(
                "[bong][forge] place_station rejected: consume instance_id={} failed: {err}",
                req.item_instance_id
            );
            continue;
        }

        let station = WeaponForgeStation::placed(req.pos, station_spec.tier, req.player);
        commands.spawn(station);
        placed_this_tick.insert(pos_key);
        if let Ok(mut layer) = layers.get_single_mut() {
            layer.set_block(req.pos, BlockState::ANVIL);
        }
        if let Ok((username, mut client, player_state, cultivation)) = clients.get_mut(req.player) {
            send_inventory_snapshot_to_client(
                req.player,
                &mut client,
                username.0.as_str(),
                &inv,
                player_state,
                cultivation,
                "forge_station_place_consumed",
            );
        }
        tracing::info!(
            "[bong][forge] place_station ok: player={:?} pos={:?} tier={} from item=`{}`",
            req.player,
            req.pos,
            station_spec.tier,
            instance.template_id
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{
        ContainerState, ForgeStationSpec, InventoryRevision, ItemCategory, ItemInstance,
        ItemRarity, ItemTemplate, PlacedItemState,
    };
    use crate::network::agent_bridge::SERVER_DATA_CHANNEL;
    use std::collections::HashMap;
    use valence::prelude::{App, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    #[test]
    fn default_tier_1_can_craft_tier_1() {
        let s = WeaponForgeStation::default();
        assert!(s.can_craft(1));
        assert!(!s.can_craft(2));
    }

    #[test]
    fn wear_clamped_and_breaks() {
        let mut s = WeaponForgeStation::with_tier(2);
        s.apply_wear(0.3);
        assert!((s.integrity - 0.7).abs() < 1e-6);
        s.apply_wear(5.0);
        assert_eq!(s.integrity, 0.0);
        assert!(s.is_broken());
        assert!(!s.can_craft(1));
    }

    fn anvil_template(id: &str, tier: u8) -> ItemTemplate {
        ItemTemplate {
            id: id.to_string(),
            display_name: id.to_string(),
            category: ItemCategory::Misc,
            max_stack_count: 1,
            grid_w: 2,
            grid_h: 2,
            base_weight: 10.0,
            rarity: ItemRarity::Common,
            spirit_quality_initial: 1.0,
            description: String::new(),
            effect: None,
            cast_duration_ms: crate::inventory::DEFAULT_CAST_DURATION_MS,
            cooldown_ms: crate::inventory::DEFAULT_COOLDOWN_MS,
            weapon_spec: None,
            forge_station_spec: Some(ForgeStationSpec { tier }),
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
            technique_scroll_spec: None,
        }
    }

    fn misc_template(id: &str) -> ItemTemplate {
        ItemTemplate {
            forge_station_spec: None,
            ..anvil_template(id, 1)
        }
    }

    fn item_instance(instance_id: u64, template_id: &str, stack_count: u32) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: template_id.to_string(),
            display_name: template_id.to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 1.0,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count,
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

    fn inventory_with(item: ItemInstance) -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                id: crate::inventory::MAIN_PACK_CONTAINER_ID.to_string(),
                name: "main_pack".to_string(),
                rows: 5,
                cols: 7,
                items: vec![PlacedItemState {
                    row: 0,
                    col: 0,
                    instance: item,
                }],
            }],
            equipped: Default::default(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
        }
    }

    fn place_app(templates: HashMap<String, ItemTemplate>) -> App {
        let mut app = App::new();
        app.insert_resource(ItemRegistry::from_map(templates));
        app.add_event::<PlaceForgeStationRequest>();
        app.add_systems(Update, handle_place_station_request);
        app
    }

    fn flush_all_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush successfully");
        }
    }

    fn collect_inventory_snapshot_body_levels(helper: &mut MockClientHelper) -> Vec<f64> {
        let mut body_levels = Vec::new();
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }
            let payload: serde_json::Value = serde_json::from_slice(packet.data.0 .0)
                .expect("server_data payload should decode as JSON");
            if payload.get("type").and_then(|ty| ty.as_str()) == Some("inventory_snapshot") {
                body_levels.push(
                    payload
                        .get("body_level")
                        .and_then(|value| value.as_f64())
                        .expect("inventory_snapshot should carry body_level"),
                );
            }
        }
        body_levels
    }

    #[test]
    fn place_station_consumes_item() {
        let mut templates = HashMap::new();
        templates.insert(
            "fan_iron_anvil".to_string(),
            anvil_template("fan_iron_anvil", 1),
        );
        let mut app = place_app(templates);
        let player = app
            .world_mut()
            .spawn(inventory_with(item_instance(42, "fan_iron_anvil", 1)))
            .id();
        let pos = BlockPos::new(-12, 64, 38);

        app.world_mut().send_event(PlaceForgeStationRequest {
            player,
            pos,
            item_instance_id: 42,
            station_tier: 1,
        });
        app.update();

        let stations: Vec<_> = app
            .world_mut()
            .query::<&WeaponForgeStation>()
            .iter(app.world())
            .cloned()
            .collect();
        assert_eq!(stations.len(), 1);
        assert_eq!(stations[0].tier, 1);
        assert_eq!(stations[0].block_pos(), Some(pos));
        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        assert!(inv.containers[0].items.is_empty());
    }

    #[test]
    fn place_station_snapshot_uses_current_cultivation() {
        let mut templates = HashMap::new();
        templates.insert(
            "fan_iron_anvil".to_string(),
            anvil_template("fan_iron_anvil", 1),
        );
        let mut app = place_app(templates);
        let (client_bundle, mut helper) = create_mock_client("Azure");
        let cultivation = Cultivation {
            qi_current: 6.0,
            qi_max: 10.0,
            ..Default::default()
        };
        let player = app
            .world_mut()
            .spawn((
                client_bundle,
                inventory_with(item_instance(42, "fan_iron_anvil", 1)),
                PlayerState::default(),
                cultivation,
            ))
            .id();

        app.world_mut().send_event(PlaceForgeStationRequest {
            player,
            pos: BlockPos::new(-12, 64, 38),
            item_instance_id: 42,
            station_tier: 1,
        });
        app.update();
        flush_all_client_packets(&mut app);

        let body_levels = collect_inventory_snapshot_body_levels(&mut helper);
        assert_eq!(body_levels.len(), 1);
        assert!((body_levels[0] - 0.6).abs() < f64::EPSILON);
    }

    #[test]
    fn place_station_rejects_non_anvil_item() {
        let mut templates = HashMap::new();
        templates.insert("spirit_wood".to_string(), misc_template("spirit_wood"));
        let mut app = place_app(templates);
        let player = app
            .world_mut()
            .spawn(inventory_with(item_instance(43, "spirit_wood", 1)))
            .id();

        app.world_mut().send_event(PlaceForgeStationRequest {
            player,
            pos: BlockPos::new(0, 64, 0),
            item_instance_id: 43,
            station_tier: 1,
        });
        app.update();

        assert_eq!(
            app.world_mut()
                .query::<&WeaponForgeStation>()
                .iter(app.world())
                .count(),
            0
        );
        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(inv.containers[0].items[0].instance.stack_count, 1);
    }

    #[test]
    fn place_station_tier_matches_item_template() {
        let mut templates = HashMap::new();
        templates.insert(
            "ling_iron_anvil".to_string(),
            anvil_template("ling_iron_anvil", 2),
        );
        let mut app = place_app(templates);
        let player = app
            .world_mut()
            .spawn(inventory_with(item_instance(44, "ling_iron_anvil", 2)))
            .id();

        app.world_mut().send_event(PlaceForgeStationRequest {
            player,
            pos: BlockPos::new(1, 64, 1),
            item_instance_id: 44,
            station_tier: 1,
        });
        app.update();
        assert_eq!(
            app.world_mut()
                .query::<&WeaponForgeStation>()
                .iter(app.world())
                .count(),
            0
        );

        app.world_mut().send_event(PlaceForgeStationRequest {
            player,
            pos: BlockPos::new(2, 64, 2),
            item_instance_id: 44,
            station_tier: 2,
        });
        app.update();

        let stations: Vec<_> = app
            .world_mut()
            .query::<&WeaponForgeStation>()
            .iter(app.world())
            .cloned()
            .collect();
        assert_eq!(stations.len(), 1);
        assert_eq!(stations[0].tier, 2);
        let inv = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(inv.containers[0].items[0].instance.stack_count, 1);
    }
}
