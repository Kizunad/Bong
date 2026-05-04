//! plan-mineral-v1 §2.2 — `MineralDropEvent` 消费者。
//!
//! break_handler 发 `MineralDropEvent`（player / mineral_id / position）后，本系统
//! 按 `MineralRegistry` 合成 `ItemInstance`（NBT 写 `mineral_id`）塞进玩家 main_pack。
//!
//! 设计要点：
//!  * **不依赖 ItemRegistry**：矿物 item 没有 TOML 模板（避免 18 条重复登记），
//!    display_name / rarity / 默认 grid 由 `MineralRegistry` 提供。
//!  * **template_id = "mineral_{canonical}"**：给 client tooltip / schema layer 留正典锚，
//!    不需要在 ItemRegistry 注册就能区分（mineral_id NBT 才是权威）。
//!  * 找不到 inventory / registry miss / allocator 耗尽 → warn + skip（不 panic，
//!    丢失一次 drop 不阻塞服务器）。

use valence::prelude::{BlockPos, Client, EventReader, Events, Query, Res, ResMut, Username};

use super::events::MineralDropEvent;
use super::persistence::MineralTickClock;
use super::registry::{MineralEntry, MineralRegistry};
use super::types::MineralRarity;
use crate::cultivation::components::Cultivation;
use crate::inventory::{
    find_free_slot, find_mergeable_stack, InventoryInstanceIdAllocator, ItemInstance, ItemRarity,
    PlacedItemState, PlayerInventory, MAIN_PACK_CONTAINER_ID,
};
use crate::network::inventory_snapshot_emit::send_inventory_snapshot_to_client;
use crate::player::state::PlayerState;
use crate::shelflife::DecayProfileRegistry;
use crate::shelflife::{DecayProfileId, Freshness};
use crate::skill::components::SkillId;
use crate::skill::events::{SkillXpGain, XpGainSource};

/// Mineral ore drop 的默认堆数 —— 一次挖方块产一枚（与 vanilla 一致）。
const DEFAULT_DROP_STACK_COUNT: u32 = 1;
const MINERAL_MAX_STACK_COUNT: u32 = 32;

/// plan-mineral-v1 §2.2 consumer —— 把 MineralDropEvent 写成 PlayerInventory 的 ItemInstance。
#[allow(clippy::too_many_arguments)] // Bevy system signature; drop, inventory, snapshot, and skill concerns stay explicit.
pub fn consume_mineral_drops_into_inventory(
    mut events: EventReader<MineralDropEvent>,
    registry: Res<MineralRegistry>,
    profile_registry: Option<Res<DecayProfileRegistry>>,
    clock: Res<MineralTickClock>,
    mut allocator: ResMut<InventoryInstanceIdAllocator>,
    mut inventories: Query<&mut PlayerInventory>,
    mut clients: Query<(&mut Client, &Username, &PlayerState, &Cultivation)>,
    mut skill_xp_events: Option<ResMut<Events<SkillXpGain>>>,
) {
    for event in events.read() {
        let Ok(mut inventory) = inventories.get_mut(event.player) else {
            tracing::warn!(
                target: "bong::mineral",
                "MineralDropEvent for entity {:?} but no PlayerInventory — drop skipped",
                event.player
            );
            continue;
        };

        let Some(entry) = registry.get(event.mineral_id) else {
            tracing::warn!(
                target: "bong::mineral",
                "MineralDropEvent carries unregistered mineral_id {} — drop skipped",
                event.mineral_id
            );
            continue;
        };

        let Some(main_pack) = inventory
            .containers
            .iter_mut()
            .find(|c| c.id == MAIN_PACK_CONTAINER_ID)
        else {
            tracing::warn!(
                target: "bong::mineral",
                "player {:?} missing main_pack container — mineral drop lost",
                event.player
            );
            continue;
        };

        let template_id = format!("mineral_{}", entry.canonical_name);
        if let Some(placed) =
            find_mergeable_stack(main_pack, template_id.as_str(), MINERAL_MAX_STACK_COUNT)
        {
            placed.instance.stack_count = placed
                .instance
                .stack_count
                .saturating_add(DEFAULT_DROP_STACK_COUNT);
        } else {
            let Some((row, col)) = find_free_slot(main_pack, 1, 1) else {
                tracing::warn!(
                    target: "bong::mineral",
                    "player {:?} main_pack is full — mineral drop {} lost",
                    event.player,
                    entry.canonical_name
                );
                continue;
            };
            let instance_id = match allocator.next_id() {
                Ok(id) => id,
                Err(err) => {
                    tracing::warn!(
                        target: "bong::mineral",
                        "inventory allocator exhausted on mineral drop: {err}"
                    );
                    continue;
                }
            };
            let instance = build_mineral_item_instance(
                instance_id,
                entry,
                DEFAULT_DROP_STACK_COUNT,
                clock.tick,
                event.position,
                profile_registry.as_deref(),
            );
            main_pack.items.push(PlacedItemState { row, col, instance });
        }

        inventory.revision.0 = inventory.revision.0.saturating_add(1);
        if let Some(skill_xp_events) = skill_xp_events.as_deref_mut() {
            skill_xp_events.send(SkillXpGain {
                char_entity: event.player,
                skill: SkillId::Mineral,
                amount: 1,
                source: XpGainSource::Action {
                    plan_id: "mineral",
                    action: "ore_drop",
                },
            });
        }
        if let Ok((mut client, username, player_state, cultivation)) = clients.get_mut(event.player)
        {
            send_inventory_snapshot_to_client(
                event.player,
                &mut client,
                username.0.as_str(),
                &inventory,
                player_state,
                cultivation,
                "mineral_drop",
            );
        }
    }
}

fn build_mineral_item_instance(
    instance_id: u64,
    entry: &MineralEntry,
    stack_count: u32,
    created_at_tick: u64,
    position: BlockPos,
    profile_registry: Option<&DecayProfileRegistry>,
) -> ItemInstance {
    let freshness = build_mineral_freshness(
        entry,
        created_at_tick,
        position,
        instance_id,
        profile_registry,
    );
    ItemInstance {
        instance_id,
        template_id: format!("mineral_{}", entry.canonical_name),
        display_name: entry.display_name_zh.to_string(),
        grid_w: 1,
        grid_h: 1,
        weight: 0.5,
        rarity: rarity_from_mineral(entry.rarity),
        description: String::new(),
        stack_count,
        spirit_quality: 0.0,
        durability: 1.0,
        freshness,
        mineral_id: Some(entry.canonical_name.to_string()),
        charges: None,
        forge_quality: None,
        forge_color: None,
        forge_side_effects: Vec::new(),
        forge_achieved_tier: None,
        alchemy: None,
        lingering_owner_qi: None,
    }
}

fn build_mineral_freshness(
    entry: &MineralEntry,
    created_at_tick: u64,
    position: BlockPos,
    instance_id: u64,
    profile_registry: Option<&DecayProfileRegistry>,
) -> Option<Freshness> {
    let profile = entry.decay_profile?;
    let qi_range = entry.ling_shi_qi_range?;
    let Some(profile_registry) = profile_registry else {
        tracing::warn!(
            target: "bong::mineral",
            "shelflife DecayProfileRegistry missing while building freshness for {}",
            entry.canonical_name
        );
        return None;
    };
    let profile_id = DecayProfileId::new(profile);
    let Some(profile) = profile_registry.get(&profile_id) else {
        tracing::warn!(
            target: "bong::mineral",
            "missing shelflife profile {} for mineral {}",
            profile_id.as_str(),
            entry.canonical_name
        );
        return None;
    };
    let initial_qi =
        qi_range.min + (qi_range.max - qi_range.min) * mineral_qi_roll(position, instance_id);
    Some(Freshness::new(created_at_tick, initial_qi, profile))
}

fn mineral_qi_roll(position: BlockPos, instance_id: u64) -> f32 {
    let mut hash = instance_id;
    hash ^= (position.x as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    hash ^= (position.y as i64 as u64).rotate_left(21);
    hash ^= (position.z as i64 as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    hash ^= hash >> 33;
    hash = hash.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    hash ^= hash >> 33;
    (hash as f64 / u64::MAX as f64) as f32
}

fn rarity_from_mineral(r: MineralRarity) -> ItemRarity {
    match r {
        MineralRarity::Fan => ItemRarity::Common,
        MineralRarity::Ling => ItemRarity::Uncommon,
        MineralRarity::Xi => ItemRarity::Rare,
        MineralRarity::Yi => ItemRarity::Legendary,
    }
}

#[cfg(test)]
mod tests {
    use super::super::registry::build_default_registry;
    use super::super::types::MineralId;
    use super::*;
    use crate::inventory::{
        ContainerState, InventoryRevision, PlayerInventory as Inv, MAIN_PACK_CONTAINER_ID as MAIN,
    };
    use crate::shelflife::DecayTrack;
    use std::collections::HashMap;
    use valence::prelude::{App, BlockPos, Events, Update};

    fn empty_inventory() -> Inv {
        Inv {
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                id: MAIN.to_string(),
                name: MAIN.to_string(),
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
    fn rarity_mapping_matches_mineral_tiers() {
        assert_eq!(rarity_from_mineral(MineralRarity::Fan), ItemRarity::Common);
        assert_eq!(
            rarity_from_mineral(MineralRarity::Ling),
            ItemRarity::Uncommon
        );
        assert_eq!(rarity_from_mineral(MineralRarity::Xi), ItemRarity::Rare);
        assert_eq!(
            rarity_from_mineral(MineralRarity::Yi),
            ItemRarity::Legendary
        );
    }

    #[test]
    fn build_mineral_item_instance_carries_canonical_mineral_id() {
        let reg = build_default_registry();
        let entry = reg.get(MineralId::SuiTie).unwrap();
        let item = build_mineral_item_instance(99, entry, 1, 123, BlockPos::new(1, 2, 3), None);
        assert_eq!(item.mineral_id.as_deref(), Some("sui_tie"));
        assert_eq!(item.template_id, "mineral_sui_tie");
        assert_eq!(item.display_name, "髓铁");
        assert_eq!(item.rarity, ItemRarity::Rare);
        assert!(item.freshness.is_none());
    }

    #[test]
    fn build_ling_shi_item_instance_carries_freshness_profile() {
        let reg = build_default_registry();
        let profile_reg = crate::shelflife::build_default_registry();
        let entry = reg.get(MineralId::LingShiZhong).unwrap();
        let item = build_mineral_item_instance(
            100,
            entry,
            1,
            123,
            BlockPos::new(1, 2, 3),
            Some(&profile_reg),
        );
        let freshness = item.freshness.expect("ling_shi should carry freshness");
        assert_eq!(freshness.profile.as_str(), "ling_shi_zhong_v1");
        assert_eq!(freshness.track, DecayTrack::Decay);
        assert!(
            (30.0..=60.0).contains(&freshness.initial_qi),
            "initial_qi should stay inside registry range: {}",
            freshness.initial_qi
        );
        assert_eq!(freshness.created_at_tick, 123);
        assert_eq!(item.mineral_id.as_deref(), Some("ling_shi_zhong"));
    }

    #[test]
    fn mineral_qi_roll_is_position_sensitive() {
        let left = mineral_qi_roll(BlockPos::new(1, 2, 3), 100);
        let right = mineral_qi_roll(BlockPos::new(1, 2, 4), 100);
        assert!((0.0..=1.0).contains(&left));
        assert!((0.0..=1.0).contains(&right));
        assert_ne!(left, right);
    }

    #[test]
    fn drop_event_appends_instance_to_main_pack() {
        let mut app = App::new();
        app.add_event::<MineralDropEvent>();
        app.add_event::<SkillXpGain>();
        app.insert_resource(build_default_registry());
        app.insert_resource(crate::shelflife::build_default_registry());
        app.insert_resource(MineralTickClock::default());
        app.insert_resource(InventoryInstanceIdAllocator::default());

        let player = app.world_mut().spawn(empty_inventory()).id();
        app.add_systems(Update, consume_mineral_drops_into_inventory);

        app.world_mut()
            .resource_mut::<Events<MineralDropEvent>>()
            .send(MineralDropEvent {
                player,
                mineral_id: MineralId::FanTie,
                position: BlockPos::new(1, 64, 2),
            });

        app.update();

        let inv = app.world().get::<Inv>(player).expect("player inventory");
        let main = inv
            .containers
            .iter()
            .find(|c| c.id == MAIN)
            .expect("main_pack present");
        assert_eq!(main.items.len(), 1);
        let item = &main.items[0].instance;
        assert_eq!(item.mineral_id.as_deref(), Some("fan_tie"));
        assert_eq!(item.template_id, "mineral_fan_tie");
        assert_eq!(inv.revision.0, 1);

        let xp_events = app.world().resource::<Events<SkillXpGain>>();
        let xp = xp_events
            .iter_current_update_events()
            .next()
            .expect("mineral drop should emit skill xp");
        assert_eq!(xp.char_entity, player);
        assert_eq!(xp.skill, SkillId::Mineral);
        assert_eq!(xp.amount, 1);
    }

    #[test]
    fn repeated_drop_events_merge_same_mineral_stack() {
        let mut app = App::new();
        app.add_event::<MineralDropEvent>();
        app.add_event::<SkillXpGain>();
        app.insert_resource(build_default_registry());
        app.insert_resource(crate::shelflife::build_default_registry());
        app.insert_resource(MineralTickClock::default());
        app.insert_resource(InventoryInstanceIdAllocator::default());

        let player = app.world_mut().spawn(empty_inventory()).id();
        app.add_systems(Update, consume_mineral_drops_into_inventory);

        {
            let mut events = app.world_mut().resource_mut::<Events<MineralDropEvent>>();
            events.send(MineralDropEvent {
                player,
                mineral_id: MineralId::FanTie,
                position: BlockPos::new(1, 64, 2),
            });
            events.send(MineralDropEvent {
                player,
                mineral_id: MineralId::FanTie,
                position: BlockPos::new(2, 64, 2),
            });
        }

        app.update();

        let inv = app.world().get::<Inv>(player).expect("player inventory");
        let main = inv
            .containers
            .iter()
            .find(|c| c.id == MAIN)
            .expect("main_pack present");
        assert_eq!(main.items.len(), 1);
        assert_eq!(main.items[0].row, 0);
        assert_eq!(main.items[0].col, 0);
        assert_eq!(main.items[0].instance.template_id, "mineral_fan_tie");
        assert_eq!(main.items[0].instance.stack_count, 2);
        assert_eq!(inv.revision.0, 2);
    }

    #[test]
    fn different_mineral_drops_allocate_non_overlapping_slots() {
        let mut app = App::new();
        app.add_event::<MineralDropEvent>();
        app.add_event::<SkillXpGain>();
        app.insert_resource(build_default_registry());
        app.insert_resource(crate::shelflife::build_default_registry());
        app.insert_resource(MineralTickClock::default());
        app.insert_resource(InventoryInstanceIdAllocator::default());

        let player = app.world_mut().spawn(empty_inventory()).id();
        app.add_systems(Update, consume_mineral_drops_into_inventory);

        {
            let mut events = app.world_mut().resource_mut::<Events<MineralDropEvent>>();
            events.send(MineralDropEvent {
                player,
                mineral_id: MineralId::FanTie,
                position: BlockPos::new(1, 64, 2),
            });
            events.send(MineralDropEvent {
                player,
                mineral_id: MineralId::LingTie,
                position: BlockPos::new(2, 64, 2),
            });
        }

        app.update();

        let inv = app.world().get::<Inv>(player).expect("player inventory");
        let main = inv
            .containers
            .iter()
            .find(|c| c.id == MAIN)
            .expect("main_pack present");
        let positions: Vec<_> = main
            .items
            .iter()
            .map(|item| (item.instance.template_id.as_str(), item.row, item.col))
            .collect();
        assert_eq!(
            positions,
            vec![("mineral_fan_tie", 0, 0), ("mineral_ling_tie", 0, 1)]
        );
    }

    #[test]
    fn ling_shi_freshness_requires_registered_profile_lookup() {
        let reg = build_default_registry();
        let entry = reg.get(MineralId::LingShiFan).unwrap();
        let item = build_mineral_item_instance(1, entry, 1, 0, BlockPos::new(0, 0, 0), None);
        assert!(item.freshness.is_none());
    }

    #[test]
    fn drop_event_without_inventory_is_silently_skipped() {
        let mut app = App::new();
        app.add_event::<MineralDropEvent>();
        app.add_event::<SkillXpGain>();
        app.insert_resource(build_default_registry());
        app.insert_resource(crate::shelflife::build_default_registry());
        app.insert_resource(MineralTickClock::default());
        app.insert_resource(InventoryInstanceIdAllocator::default());
        app.add_systems(Update, consume_mineral_drops_into_inventory);

        // 随便 spawn 一个没 inventory 的 entity
        let ghost = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<Events<MineralDropEvent>>()
            .send(MineralDropEvent {
                player: ghost,
                mineral_id: MineralId::LingTie,
                position: BlockPos::new(0, 0, 0),
            });

        // 不应 panic
        app.update();
    }
}
