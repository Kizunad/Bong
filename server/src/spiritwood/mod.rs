pub mod session;

use std::collections::HashSet;

use session::{WoodSession, WoodSessionStore, MOVEMENT_BREAK_DISTANCE_SQ};
use valence::prelude::{
    bevy_ecs, App, BlockPos, BlockState, ChunkLayer, Client, DiggingEvent, DiggingState, Entity,
    Event, EventReader, EventWriter, GameMode, IntoSystemConfigs, Position, Query, Res, ResMut,
    Resource, Update, Username, With,
};

use crate::combat::events::CombatEvent;
use crate::cultivation::components::{Cultivation, Realm};
use crate::gathering::quality::{quality_hint, roll_quality};
use crate::gathering::session::{
    GatheringCompleteEvent, GatheringProgressFrame, PROGRESS_SYNC_INTERVAL_TICKS,
};
use crate::gathering::tools::{equipped_gathering_tool, GatheringTargetKind};
use crate::inventory::{
    bump_revision, InventoryInstanceIdAllocator, ItemInstance, ItemRegistry, PlacedItemState,
    PlayerInventory, EQUIP_SLOT_MAIN_HAND, EQUIP_SLOT_TWO_HAND, MAIN_PACK_CONTAINER_ID,
};
use crate::network::send_server_data_payload;
use crate::player::gameplay::GameplayTick;
use crate::player::state::canonical_player_id;
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use crate::shelflife::{DecayProfileId, DecayProfileRegistry, Freshness};
use crate::world::dimension::{CurrentDimension, DimensionKind, DimensionLayers};
use crate::world::terrain::TerrainProviders;

pub const LING_MU_GUN_ITEM_ID: &str = "ling_mu_gun";
pub const LING_MU_GUN_PROFILE_ID: &str = "ling_mu_gun_v1";
const REQUIRED_AXE_TIER: u8 = 3;
const LING_MU_INITIAL_QI: f32 = 100.0;

#[derive(Debug, Default)]
pub struct SpiritWoodHarvestedLogs {
    positions: HashSet<(DimensionKind, [i32; 3])>,
}

impl Resource for SpiritWoodHarvestedLogs {}

impl SpiritWoodHarvestedLogs {
    pub fn contains(&self, dimension: DimensionKind, pos: BlockPos) -> bool {
        self.positions.contains(&position_key(dimension, pos))
    }

    pub fn mark_harvested(&mut self, dimension: DimensionKind, pos: BlockPos) {
        self.positions.insert(position_key(dimension, pos));
    }

    pub fn positions_in_chunk(
        &self,
        dimension: DimensionKind,
        chunk: valence::prelude::ChunkPos,
    ) -> Vec<BlockPos> {
        self.positions
            .iter()
            .filter_map(|(stored_dimension, [x, y, z])| {
                (*stored_dimension == dimension
                    && x.div_euclid(16) == chunk.x
                    && z.div_euclid(16) == chunk.z)
                    .then_some(BlockPos::new(*x, *y, *z))
            })
            .collect()
    }
}

fn position_key(dimension: DimensionKind, pos: BlockPos) -> (DimensionKind, [i32; 3]) {
    (dimension, [pos.x, pos.y, pos.z])
}

#[derive(Debug, Clone, PartialEq, Event)]
struct LumberTerminalEvent {
    client_entity: Entity,
    session_id: String,
    log_pos: BlockPos,
    progress: f64,
    interrupted: bool,
    completed: bool,
    detail: String,
    duration_ticks: u64,
    gathering_quality: Option<crate::gathering::quality::GatheringQuality>,
    tool_used: Option<String>,
}

pub fn register(app: &mut App) {
    app.insert_resource(WoodSessionStore::default());
    app.insert_resource(SpiritWoodHarvestedLogs::default());
    app.add_event::<LumberTerminalEvent>();
    app.add_systems(
        Update,
        (
            start_spiritwood_sessions,
            enforce_spiritwood_session_constraints,
            complete_spiritwood_sessions.in_set(crate::gathering::GatheringSystemSet::Produce),
            emit_active_lumber_progress.in_set(crate::gathering::GatheringSystemSet::Produce),
            emit_terminal_lumber_progress.in_set(crate::gathering::GatheringSystemSet::Produce),
        )
            .chain(),
    );
}

#[allow(clippy::too_many_arguments)]
fn start_spiritwood_sessions(
    gameplay_tick: Option<Res<GameplayTick>>,
    mut digs: EventReader<DiggingEvent>,
    mut store: ResMut<WoodSessionStore>,
    providers: Option<Res<TerrainProviders>>,
    dimension_layers: Option<Res<DimensionLayers>>,
    layers: Query<&ChunkLayer>,
    dimensions: Query<&CurrentDimension>,
    positions: Query<&Position, With<Client>>,
    usernames: Query<&Username, With<Client>>,
    inventories: Query<&PlayerInventory, With<Client>>,
    game_modes: Query<&GameMode, With<Client>>,
    harvested_logs: Res<SpiritWoodHarvestedLogs>,
) {
    let now_tick = gameplay_tick.map(|tick| tick.current_tick()).unwrap_or(0);
    for event in digs.read() {
        if event.state != DiggingState::Start {
            continue;
        }
        // Creative 下默认 block_break 系统已立刻把 log 抹成 AIR，再开 MiningSession
        // 等于在虚空里跑——白白产 ling_mu_gun 给 dev。Creative 不应得到掉落，跳过。
        if matches!(
            game_modes.get(event.client).copied().unwrap_or_default(),
            GameMode::Creative
        ) {
            continue;
        }
        if store.session_for(event.client).is_some() {
            continue;
        }

        let dimension = dimensions
            .get(event.client)
            .map(|current| current.0)
            .unwrap_or(DimensionKind::Overworld);
        if store.has_session_at(dimension, event.position) {
            continue;
        }
        let block_state = block_state_at(
            event.position,
            dimension,
            dimension_layers.as_deref(),
            &layers,
        );
        if !is_spiritwood_log_target(
            event.position,
            dimension,
            block_state,
            providers.as_deref(),
            harvested_logs.as_ref(),
        ) {
            continue;
        }

        let Ok(inventory) = inventories.get(event.client) else {
            continue;
        };
        let Some((tool_instance_id, tier)) = equipped_axe_tier(inventory) else {
            continue;
        };
        if tier < REQUIRED_AXE_TIER {
            continue;
        }

        let origin_position = positions.get(event.client).map(position_xyz).unwrap_or([
            event.position.x as f64,
            event.position.y as f64,
            event.position.z as f64,
        ]);
        let player_id = usernames
            .get(event.client)
            .map(|username| canonical_player_id(username.0.as_str()))
            .unwrap_or_else(|_| format!("entity:{}", event.client.to_bits()));

        store.upsert(WoodSession::new(
            event.client,
            player_id,
            dimension,
            event.position,
            now_tick,
            origin_position,
            Some(tool_instance_id),
        ));
    }
}

fn enforce_spiritwood_session_constraints(
    gameplay_tick: Option<Res<GameplayTick>>,
    mut store: ResMut<WoodSessionStore>,
    positions: Query<&Position, With<Client>>,
    inventories: Query<&PlayerInventory, With<Client>>,
    mut combat_events: EventReader<CombatEvent>,
    mut terminal_events: EventWriter<LumberTerminalEvent>,
) {
    let now_tick = gameplay_tick.map(|tick| tick.current_tick()).unwrap_or(0);
    let hit_entities = combat_events
        .read()
        .map(|event| event.target)
        .collect::<HashSet<_>>();
    let mut to_cancel = Vec::new();

    for session in store.iter() {
        let hit = hit_entities.contains(&session.player);
        let moved = positions
            .get(session.player)
            .map(|position| {
                let current = position_xyz(position);
                distance_sq(current, session.origin_position) > MOVEMENT_BREAK_DISTANCE_SQ
            })
            .unwrap_or(false);
        let tool_switched = inventories
            .get(session.player)
            .ok()
            .and_then(equipped_harvest_tool_instance_id)
            != session.tool_instance_id;

        if hit || moved || tool_switched {
            let detail = if hit {
                "受击打断"
            } else if moved {
                "移动打断"
            } else {
                "切换工具打断"
            };
            to_cancel.push((
                session.player,
                session.player_id.clone(),
                session.log_pos,
                session.progress_at(now_tick),
                session.ticks_total,
                detail.to_string(),
            ));
        }
    }

    for (player, session_id, log_pos, progress, duration_ticks, detail) in to_cancel {
        store.remove(player);
        terminal_events.send(LumberTerminalEvent {
            client_entity: player,
            session_id,
            log_pos,
            progress,
            interrupted: true,
            completed: false,
            detail,
            duration_ticks,
            gathering_quality: None,
            tool_used: None,
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn complete_spiritwood_sessions(
    gameplay_tick: Option<Res<GameplayTick>>,
    mut store: ResMut<WoodSessionStore>,
    dimension_layers: Option<Res<DimensionLayers>>,
    mut layers: Query<&mut ChunkLayer>,
    mut harvested_logs: ResMut<SpiritWoodHarvestedLogs>,
    item_registry: Res<ItemRegistry>,
    profile_registry: Option<Res<DecayProfileRegistry>>,
    mut allocator: ResMut<InventoryInstanceIdAllocator>,
    mut inventories: Query<&mut PlayerInventory, With<Client>>,
    cultivations: Query<&Cultivation, With<Client>>,
    mut gathering_completions: EventWriter<GatheringCompleteEvent>,
    mut terminal_events: EventWriter<LumberTerminalEvent>,
) {
    let now_tick = gameplay_tick.map(|tick| tick.current_tick()).unwrap_or(0);
    let completed = store
        .iter()
        .filter(|session| session.completed_at(now_tick))
        .map(|session| session.player)
        .collect::<Vec<_>>();

    for player in completed {
        let Some(session) = store.remove(player) else {
            continue;
        };
        harvested_logs.mark_harvested(session.dimension, session.log_pos);
        if let Some(dimension_layers) = dimension_layers.as_deref() {
            if let Ok(mut layer) = layers.get_mut(dimension_layers.entity_for(session.dimension)) {
                layer.set_block(session.log_pos, BlockState::AIR);
            }
        }

        let drop_count = ling_mu_drop_count(session.log_pos, session.player, now_tick);
        let mut gathering_tool = None;
        let mut gathering_quality = None;
        if let Ok(mut inventory) = inventories.get_mut(session.player) {
            gathering_tool = equipped_gathering_tool(&inventory)
                .filter(|tool| tool.matches_target(GatheringTargetKind::Wood));
            let realm = cultivations
                .get(session.player)
                .map(|cultivation| cultivation.realm)
                .unwrap_or(Realm::Awaken);
            let gathering_quality_seed = now_tick
                ^ session.player.to_bits().wrapping_mul(0x9E37_79B9_7F4A_7C15)
                ^ session.ticks_total.wrapping_mul(0xBF58_476D_1CE4_E5B9);
            gathering_quality = Some(roll_quality(
                gathering_quality_seed,
                gathering_tool.map(|tool| tool.material),
                realm,
            ));
            if let Err(error) = grant_ling_mu_gun_to_inventory(
                &mut inventory,
                item_registry.as_ref(),
                profile_registry.as_deref(),
                &mut allocator,
                drop_count,
                now_tick,
            ) {
                tracing::warn!(
                    target: "bong::spiritwood",
                    "failed to grant ling_mu_gun drop to {:?}: {error}",
                    session.player
                );
            }
        }
        if let Some(quality) = gathering_quality {
            gathering_completions.send(GatheringCompleteEvent {
                player: session.player,
                session_id: session.player_id.clone(),
                origin_position: block_origin(session.log_pos),
                target_name: "灵木".to_string(),
                target_type: GatheringTargetKind::Wood,
                quality,
                tool_used: gathering_tool.map(|tool| tool.item_id.to_string()),
            });
        }

        terminal_events.send(LumberTerminalEvent {
            client_entity: session.player,
            session_id: session.player_id,
            log_pos: session.log_pos,
            progress: 1.0,
            interrupted: false,
            completed: true,
            detail: format!("采得灵木原木 ×{drop_count}"),
            duration_ticks: session.ticks_total,
            gathering_quality,
            tool_used: gathering_tool.map(|tool| tool.item_id.to_string()),
        });
    }
}

fn emit_active_lumber_progress(
    gameplay_tick: Option<Res<GameplayTick>>,
    store: Res<WoodSessionStore>,
    mut gathering_frames: EventWriter<GatheringProgressFrame>,
    mut clients: Query<&mut Client, With<Client>>,
    inventories: Query<&PlayerInventory, With<Client>>,
    cultivations: Query<&Cultivation, With<Client>>,
) {
    let now_tick = gameplay_tick.map(|tick| tick.current_tick()).unwrap_or(0);
    for session in store.iter() {
        let Ok(mut client) = clients.get_mut(session.player) else {
            continue;
        };
        let progress = session.progress_at(now_tick);
        if now_tick % PROGRESS_SYNC_INTERVAL_TICKS == 0 {
            let active_tool = inventories
                .get(session.player)
                .ok()
                .and_then(equipped_gathering_tool)
                .filter(|tool| tool.matches_target(GatheringTargetKind::Wood));
            let active_realm = cultivations
                .get(session.player)
                .map(|cultivation| cultivation.realm)
                .unwrap_or(Realm::Awaken);
            gathering_frames.send(GatheringProgressFrame {
                player: session.player,
                session_id: session.player_id.clone(),
                origin_position: block_origin(session.log_pos),
                progress_ticks: (progress * session.ticks_total as f64).round() as u64,
                total_ticks: session.ticks_total,
                target_name: "灵木".to_string(),
                target_type: GatheringTargetKind::Wood,
                quality_hint: quality_hint(active_tool.map(|tool| tool.material), active_realm)
                    .to_string(),
                tool_used: active_tool.map(|tool| tool.item_id.to_string()),
                interrupted: false,
                completed: false,
            });
        }
        send_lumber_progress_to_client(
            &mut client,
            session.player_id.clone(),
            session.log_pos,
            progress,
            false,
            false,
            String::new(),
        );
    }
}

fn emit_terminal_lumber_progress(
    mut events: EventReader<LumberTerminalEvent>,
    mut gathering_frames: EventWriter<GatheringProgressFrame>,
    mut clients: Query<&mut Client, With<Client>>,
) {
    for event in events.read() {
        let Ok(mut client) = clients.get_mut(event.client_entity) else {
            continue;
        };
        gathering_frames.send(GatheringProgressFrame {
            player: event.client_entity,
            session_id: event.session_id.clone(),
            origin_position: block_origin(event.log_pos),
            progress_ticks: if event.completed {
                event.duration_ticks.max(1)
            } else {
                0
            },
            total_ticks: event.duration_ticks.max(1),
            target_name: "灵木".to_string(),
            target_type: GatheringTargetKind::Wood,
            quality_hint: event
                .gathering_quality
                .map(|quality| quality.as_wire().to_string())
                .unwrap_or_else(|| "normal".to_string()),
            tool_used: event.tool_used.clone(),
            interrupted: event.interrupted,
            completed: event.completed,
        });
        send_lumber_progress_to_client(
            &mut client,
            event.session_id.clone(),
            event.log_pos,
            event.progress,
            event.interrupted,
            event.completed,
            event.detail.clone(),
        );
    }
}

fn send_lumber_progress_to_client(
    client: &mut Client,
    session_id: String,
    log_pos: BlockPos,
    progress: f64,
    interrupted: bool,
    completed: bool,
    detail: String,
) {
    let payload = ServerDataV1::new(ServerDataPayloadV1::LumberProgress {
        session_id,
        log_pos: [log_pos.x, log_pos.y, log_pos.z],
        progress: progress.clamp(0.0, 1.0),
        interrupted,
        completed,
        detail,
    });
    let Ok(bytes) = crate::network::agent_bridge::serialize_server_data_payload(&payload) else {
        return;
    };
    send_server_data_payload(client, bytes.as_slice());
}

fn block_origin(pos: BlockPos) -> [f64; 3] {
    [pos.x as f64 + 0.5, pos.y as f64 + 0.5, pos.z as f64 + 0.5]
}

fn is_spiritwood_log_target(
    pos: BlockPos,
    dimension: DimensionKind,
    block_state: Option<BlockState>,
    providers: Option<&TerrainProviders>,
    harvested_logs: &SpiritWoodHarvestedLogs,
) -> bool {
    if dimension != DimensionKind::Overworld || harvested_logs.contains(dimension, pos) {
        return false;
    }
    if block_state != Some(BlockState::OAK_LOG) {
        return false;
    }
    providers.is_some_and(|providers| {
        crate::world::terrain::mega_tree::is_spiritwood_log_at(pos, &providers.overworld)
    })
}

fn block_state_at(
    pos: BlockPos,
    dimension: DimensionKind,
    dimension_layers: Option<&DimensionLayers>,
    layers: &Query<&ChunkLayer>,
) -> Option<BlockState> {
    let layer_entity = dimension_layers?.entity_for(dimension);
    layers
        .get(layer_entity)
        .ok()
        .and_then(|layer| layer.block(pos).map(|block| block.state))
}

fn equipped_axe_tier(inventory: &PlayerInventory) -> Option<(u64, u8)> {
    inventory
        .equipped
        .get(EQUIP_SLOT_MAIN_HAND)
        .or_else(|| inventory.equipped.get(EQUIP_SLOT_TWO_HAND))
        .and_then(|item| axe_tier_from_item(item).map(|tier| (item.instance_id, tier)))
}

fn axe_tier_from_item(item: &ItemInstance) -> Option<u8> {
    let id = item.template_id.as_str();
    if id.contains("wooden_axe") || id.contains("golden_axe") || id == "axe_bone" {
        Some(1)
    } else if id == "axe_copper" || id.contains("stone_axe") || id.contains("fan_iron_axe") {
        Some(2)
    } else if id == "axe_iron" || id.contains("iron_axe") || id.contains("ling_iron_axe") {
        Some(3)
    } else if id.contains("diamond_axe") || id.contains("netherite_axe") || id.contains("yi_axe") {
        Some(4)
    } else {
        None
    }
}

fn equipped_harvest_tool_instance_id(inventory: &PlayerInventory) -> Option<u64> {
    equipped_axe_tier(inventory).map(|(instance_id, _tier)| instance_id)
}

fn position_xyz(position: &Position) -> [f64; 3] {
    let pos = position.get();
    [pos.x, pos.y, pos.z]
}

fn distance_sq(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    dx * dx + dy * dy + dz * dz
}

fn ling_mu_drop_count(pos: BlockPos, player: Entity, completed_at_tick: u64) -> u32 {
    let mut hash = completed_at_tick
        ^ player.to_bits().wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (pos.x as i64 as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
        ^ (pos.y as i64 as u64).rotate_left(17)
        ^ (pos.z as i64 as u64).wrapping_mul(0x94D0_49BB_1331_11EB);
    hash ^= hash >> 33;
    hash = hash.wrapping_mul(0xff51_afd7_ed55_8ccd);
    2 + (hash % 3) as u32
}

fn grant_ling_mu_gun_to_inventory(
    inventory: &mut PlayerInventory,
    item_registry: &ItemRegistry,
    profile_registry: Option<&DecayProfileRegistry>,
    allocator: &mut InventoryInstanceIdAllocator,
    stack_count: u32,
    created_at_tick: u64,
) -> Result<u64, String> {
    let template = item_registry
        .get(LING_MU_GUN_ITEM_ID)
        .ok_or_else(|| format!("unknown item template `{LING_MU_GUN_ITEM_ID}`"))?;
    let instance_id = allocator.next_id()?;
    let freshness = profile_registry
        .and_then(|registry| registry.get(&DecayProfileId::new(LING_MU_GUN_PROFILE_ID)))
        .map(|profile| Freshness::new(created_at_tick, LING_MU_INITIAL_QI, profile));
    let instance = ItemInstance {
        instance_id,
        template_id: template.id.clone(),
        display_name: template.display_name.clone(),
        grid_w: template.grid_w,
        grid_h: template.grid_h,
        weight: template.base_weight,
        rarity: template.rarity,
        description: template.description.clone(),
        stack_count,
        spirit_quality: template.spirit_quality_initial,
        durability: 1.0,
        freshness,
        mineral_id: None,
        charges: None,
        forge_quality: None,
        forge_color: None,
        forge_side_effects: Vec::new(),
        forge_achieved_tier: None,
        alchemy: None,
        lingering_owner_qi: None,
    };
    let Some(main_pack) = inventory
        .containers
        .iter_mut()
        .find(|container| container.id == MAIN_PACK_CONTAINER_ID)
    else {
        return Err(format!("missing `{MAIN_PACK_CONTAINER_ID}` container"));
    };
    main_pack.items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance,
    });
    bump_revision(inventory);
    Ok(instance_id)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn ling_xia_container_behavior(
    item: &ItemInstance,
) -> Option<crate::shelflife::ContainerFreshnessBehavior> {
    (item.template_id == "ling_xia").then_some(crate::shelflife::ContainerFreshnessBehavior::Freeze)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn durability_tick_allowed_in_ling_xia(container: Option<&ItemInstance>) -> bool {
    container.is_none_or(|item| item.template_id != "ling_xia")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{ContainerState, InventoryRevision, ItemRarity};
    use std::collections::HashMap;

    fn item(template_id: &str, instance_id: u64) -> ItemInstance {
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
            spirit_quality: 0.0,
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

    fn empty_inventory() -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: "main".to_string(),
                rows: 4,
                cols: 4,
                items: Vec::new(),
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 10.0,
        }
    }

    #[test]
    fn axe_tier_requires_ling_iron_equivalent() {
        assert_eq!(
            axe_tier_from_item(&item("minecraft:wooden_axe", 1)),
            Some(1)
        );
        assert_eq!(axe_tier_from_item(&item("fan_iron_axe", 1)), Some(2));
        assert_eq!(axe_tier_from_item(&item("axe_copper", 1)), Some(2));
        assert_eq!(axe_tier_from_item(&item("ling_iron_axe", 1)), Some(3));
        assert_eq!(axe_tier_from_item(&item("axe_iron", 1)), Some(3));
        assert_eq!(axe_tier_from_item(&item("minecraft:iron_axe", 1)), Some(3));
        assert_eq!(axe_tier_from_item(&item("iron_sword", 1)), None);
    }

    #[test]
    fn ling_mu_drop_count_stays_in_plan_range() {
        for tick in 0..100 {
            let count = ling_mu_drop_count(BlockPos::new(1, 80, 2), Entity::from_raw(9), tick);
            assert!((2..=4).contains(&count));
        }
    }

    #[test]
    fn harvest_tool_identity_accepts_two_hand_axe() {
        let mut inventory = empty_inventory();
        inventory
            .equipped
            .insert(EQUIP_SLOT_TWO_HAND.to_string(), item("ling_iron_axe", 42));

        assert_eq!(equipped_harvest_tool_instance_id(&inventory), Some(42));
    }

    #[test]
    fn block_origin_offsets_to_block_center() {
        assert_eq!(block_origin(BlockPos::new(3, 64, 5)), [3.5, 64.5, 5.5]);
        assert_eq!(block_origin(BlockPos::new(-1, 64, -1)), [-0.5, 64.5, -0.5]);
    }

    #[test]
    fn completed_drop_grants_fresh_ling_mu_log() {
        let registry = crate::inventory::load_item_registry().expect("item registry should load");
        let profiles = crate::shelflife::build_default_registry();
        let mut inventory = empty_inventory();
        let mut allocator = InventoryInstanceIdAllocator::new(7);

        let id = grant_ling_mu_gun_to_inventory(
            &mut inventory,
            &registry,
            Some(&profiles),
            &mut allocator,
            3,
            120,
        )
        .expect("drop grant should succeed");

        let item = &inventory.containers[0].items[0].instance;
        assert_eq!(id, 7);
        assert_eq!(item.template_id, LING_MU_GUN_ITEM_ID);
        assert_eq!(item.stack_count, 3);
        let freshness = item
            .freshness
            .as_ref()
            .expect("ling_mu_gun should be fresh");
        assert_eq!(freshness.profile.as_str(), LING_MU_GUN_PROFILE_ID);
        assert_eq!(freshness.created_at_tick, 120);
    }

    #[test]
    fn harvested_logs_are_chunk_addressable() {
        let mut logs = SpiritWoodHarvestedLogs::default();
        logs.mark_harvested(DimensionKind::Overworld, BlockPos::new(17, 80, -1));

        assert!(logs.contains(DimensionKind::Overworld, BlockPos::new(17, 80, -1)));
        assert_eq!(
            logs.positions_in_chunk(
                DimensionKind::Overworld,
                valence::prelude::ChunkPos::new(1, -1)
            ),
            vec![BlockPos::new(17, 80, -1)]
        );
    }

    #[test]
    fn ling_xia_freezes_container_shelflife_and_pauses_durability_ticks() {
        let box_item = item("ling_xia", 99);
        assert!(matches!(
            ling_xia_container_behavior(&box_item),
            Some(crate::shelflife::ContainerFreshnessBehavior::Freeze)
        ));
        assert!(!durability_tick_allowed_in_ling_xia(Some(&box_item)));
        assert!(durability_tick_allowed_in_ling_xia(None));
    }

    /// Creative 玩家发 Start 时 spiritwood 必须跳过 session 创建——默认 block_break
    /// 已立刻把 log 抹成 AIR，再开 MiningSession 等于在虚空里跑出 ling_mu_gun 给 dev。
    /// Vanilla Creative 不掉物，本系统也不该破例。
    #[test]
    fn creative_player_does_not_start_wood_session() {
        use valence::prelude::{App, BlockPos, GameMode, Update};
        use valence::testing::create_mock_client;

        let mut app = App::new();
        app.add_event::<DiggingEvent>();
        app.insert_resource(WoodSessionStore::default());
        app.insert_resource(SpiritWoodHarvestedLogs::default());
        app.add_systems(Update, start_spiritwood_sessions);

        let (client_bundle, _helper) = create_mock_client("Creative");
        let player = app.world_mut().spawn(client_bundle).id();
        app.world_mut()
            .entity_mut(player)
            .insert(GameMode::Creative);

        app.world_mut().send_event(DiggingEvent {
            client: player,
            position: BlockPos::new(8, 80, 8),
            direction: valence::protocol::Direction::Up,
            state: DiggingState::Start,
        });

        app.update();

        let store = app.world().resource::<WoodSessionStore>();
        assert!(
            store.session_for(player).is_none(),
            "Creative Start must not create a WoodSession (would yield ling_mu_gun for free)"
        );
    }
}
