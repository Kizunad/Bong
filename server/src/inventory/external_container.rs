//! 通用"外部容器"框架——服务端权威的非玩家容器（物资棺 loot / 未来箱子 / 商人）。
//!
//! 生命周期：interact → 创建 session → 玩家搜刮（双向拖拽）→ 超时/关闭/走远 → 销毁。

use std::collections::HashMap;

use valence::prelude::{bevy_ecs, Component, Entity, Resource};

use super::{
    find_free_slot, ContainerState, InventoryInstanceIdAllocator, ItemInstance, ItemRegistry,
    ItemTemplate, PlacedItemState,
};
use crate::schema::server_data::LootContainerSourceKindV1;
use crate::supply_coffin::SupplyCoffinGrade;

/// 外部容器的来源类型。
#[derive(Debug, Clone, PartialEq)]
pub enum ExternalContainerKind {
    SupplyCoffin { grade: SupplyCoffinGrade },
    StorageCrate { is_herb: bool },
    DeadDrop,
}

pub fn external_kind_to_source_kind(kind: &ExternalContainerKind) -> LootContainerSourceKindV1 {
    match kind {
        ExternalContainerKind::SupplyCoffin { grade } => LootContainerSourceKindV1::SupplyCoffin {
            grade: grade.as_str().to_string(),
        },
        ExternalContainerKind::StorageCrate { is_herb } => {
            LootContainerSourceKindV1::StorageCrate { is_herb: *is_herb }
        }
        ExternalContainerKind::DeadDrop => LootContainerSourceKindV1::DeadDrop,
    }
}

/// 附加到 world entity 上的外部容器 ECS 组件。
#[derive(Debug, Clone, Component)]
pub struct ExternalContainer {
    pub session_id: u64,
    pub container: ContainerState,
    pub opened_by: Option<Entity>,
    /// 绝对 wall-clock 截止时间（秒）。
    pub timeout_wall_secs: u64,
    pub source_kind: ExternalContainerKind,
}

impl ExternalContainer {
    pub fn container_id(session_id: u64) -> String {
        format!("ext_{session_id}")
    }
}

/// 全局 session 注册表。
#[derive(Debug, Resource, Default)]
pub struct ExternalContainerRegistry {
    pub next_session_id: u64,
    pub sessions: HashMap<u64, Entity>,
}

impl ExternalContainerRegistry {
    pub fn allocate_session(&mut self, entity: Entity) -> u64 {
        let id = self.next_session_id;
        self.next_session_id += 1;
        self.sessions.insert(id, entity);
        id
    }

    pub fn remove_session(&mut self, session_id: u64) {
        self.sessions.remove(&session_id);
    }
}

/// 把 loot roll 结果装入 grid 容器。返回成功放入的物品数。
///
/// 物品按 roll 顺序 row-major first-fit 放置。放不下的 warn 跳过。
pub fn pack_loot_into_grid(
    container: &mut ContainerState,
    rolled: &[(String, u8)],
    item_registry: &ItemRegistry,
    allocator: &mut InventoryInstanceIdAllocator,
) -> usize {
    let mut placed = 0;
    for (template_id, count) in rolled {
        let Some(template) = item_registry.get(template_id) else {
            tracing::warn!(
                "[bong][ext_container] unknown template `{template_id}` in loot roll, skipping"
            );
            continue;
        };
        let instance_id = match allocator.next_id() {
            Ok(id) => id,
            Err(e) => {
                tracing::warn!(
                    "[bong][ext_container] instance_id exhausted: {e}, skipping `{template_id}`"
                );
                continue;
            }
        };
        let Some((row, col)) = find_free_slot(container, template.grid_w, template.grid_h) else {
            tracing::warn!(
                "[bong][ext_container] grid full, dropping `{template_id}` ({}x{} won't fit in {}x{})",
                template.grid_w,
                template.grid_h,
                container.cols,
                container.rows,
            );
            continue;
        };
        let instance = item_instance_from_template(template, instance_id, u32::from(*count));
        container.items.push(PlacedItemState { row, col, instance });
        placed += 1;
    }
    placed
}

fn item_instance_from_template(
    template: &ItemTemplate,
    instance_id: u64,
    stack_count: u32,
) -> ItemInstance {
    ItemInstance {
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

/// 从外部容器移除指定 instance_id 的物品。返回被移除的条目。
#[allow(dead_code)]
pub fn remove_item_from_container(
    container: &mut ContainerState,
    instance_id: u64,
) -> Option<PlacedItemState> {
    let idx = container
        .items
        .iter()
        .position(|p| p.instance.instance_id == instance_id)?;
    Some(container.items.remove(idx))
}

/// 尝试把物品放入外部容器的指定位置。成功返回 Ok。
#[allow(dead_code)]
pub fn place_item_into_container(
    container: &mut ContainerState,
    row: u8,
    col: u8,
    instance: ItemInstance,
) -> Result<(), String> {
    if !super::item_fits_in_container_bounds(container, row, col, instance.grid_w, instance.grid_h)
    {
        return Err("out of bounds".to_string());
    }
    let probe = super::footprint_probe(row, col, instance.grid_w, instance.grid_h);
    if container
        .items
        .iter()
        .any(|existing| super::placed_item_footprints_overlap(&probe, existing))
    {
        return Err("overlaps existing item".to_string());
    }
    container.items.push(PlacedItemState { row, col, instance });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{ItemCategory, ItemRarity};

    fn make_registry(templates: Vec<(&str, u8, u8)>) -> ItemRegistry {
        let mut map = HashMap::new();
        for (id, gw, gh) in templates {
            map.insert(
                id.to_string(),
                ItemTemplate {
                    id: id.to_string(),
                    display_name: id.to_string(),
                    category: ItemCategory::Misc,
                    placeable: None,
                    max_stack_count: 64,
                    grid_w: gw,
                    grid_h: gh,
                    base_weight: 1.0,
                    rarity: ItemRarity::Common,
                    spirit_quality_initial: 0.0,
                    description: String::new(),
                    effect: None,
                    cast_duration_ms: 0,
                    cooldown_ms: 0,
                    weapon_spec: None,
                    forge_station_spec: None,
                    blueprint_scroll_spec: None,
                    inscription_scroll_spec: None,
                    technique_scroll_spec: None,
                    recipe_fragment_spec: None,
                    container_spec: None,
                    shelflife_profile: None,
                    shield_spec: None,
                    shelflife_track: None,
                },
            );
        }
        ItemRegistry { templates: map }
    }

    fn make_container(cols: u8, rows: u8) -> ContainerState {
        ContainerState {
            id: "test".to_string(),
            name: "Test".to_string(),
            rows,
            cols,
            items: Vec::new(),
            owner_instance_id: None,
        }
    }

    fn make_allocator() -> InventoryInstanceIdAllocator {
        InventoryInstanceIdAllocator::new(1000)
    }

    #[test]
    fn pack_single_item_into_empty_grid() {
        let reg = make_registry(vec![("ore_a", 1, 1)]);
        let mut alloc = make_allocator();
        let mut container = make_container(4, 3);
        let rolled = vec![("ore_a".to_string(), 5u8)];

        let placed = pack_loot_into_grid(&mut container, &rolled, &reg, &mut alloc);

        assert_eq!(placed, 1, "should place 1 item type");
        assert_eq!(
            container.items.len(),
            1,
            "container should hold exactly 1 item"
        );
        assert_eq!(
            container.items[0].instance.stack_count, 5,
            "stack_count should match rolled quantity"
        );
        assert_eq!(container.items[0].row, 0, "first item should be at row 0");
        assert_eq!(container.items[0].col, 0, "first item should be at col 0");
    }

    #[test]
    fn pack_multiple_items_row_major() {
        let reg = make_registry(vec![("ore_a", 1, 1), ("ore_b", 2, 1)]);
        let mut alloc = make_allocator();
        let mut container = make_container(4, 3);
        let rolled = vec![
            ("ore_a".to_string(), 1),
            ("ore_b".to_string(), 1),
            ("ore_a".to_string(), 1),
        ];

        let placed = pack_loot_into_grid(&mut container, &rolled, &reg, &mut alloc);

        assert_eq!(placed, 3, "all 3 rolled items should fit in 4x3 grid");
        assert_eq!(container.items[0].col, 0, "ore_a should be at col 0");
        assert_eq!(
            container.items[1].col, 1,
            "ore_b (2-wide) should be at col 1"
        );
        assert_eq!(container.items[2].col, 3, "second ore_a should be at col 3");
    }

    #[test]
    fn pack_drops_items_that_dont_fit() {
        let reg = make_registry(vec![("big", 3, 3)]);
        let mut alloc = make_allocator();
        let mut container = make_container(4, 3);
        let rolled = vec![("big".to_string(), 1), ("big".to_string(), 1)];

        let placed = pack_loot_into_grid(&mut container, &rolled, &reg, &mut alloc);

        assert_eq!(placed, 1, "only 1 of 2 3x3 items should fit in 4x3 grid");
        assert_eq!(
            container.items.len(),
            1,
            "second 3x3 item should be dropped"
        );
    }

    #[test]
    fn pack_skips_unknown_template() {
        let reg = make_registry(vec![("ore_a", 1, 1)]);
        let mut alloc = make_allocator();
        let mut container = make_container(4, 3);
        let rolled = vec![("nonexistent".to_string(), 1), ("ore_a".to_string(), 1)];

        let placed = pack_loot_into_grid(&mut container, &rolled, &reg, &mut alloc);

        assert_eq!(
            placed, 1,
            "unknown template 'nonexistent' should be skipped"
        );
        assert_eq!(
            container.items[0].instance.template_id, "ore_a",
            "surviving item should be ore_a, not the unknown template"
        );
    }

    #[test]
    fn pack_fills_grid_completely() {
        let reg = make_registry(vec![("1x1", 1, 1)]);
        let mut alloc = make_allocator();
        let mut container = make_container(4, 3); // 12 slots
        let rolled: Vec<_> = (0..15).map(|_| ("1x1".to_string(), 1u8)).collect();

        let placed = pack_loot_into_grid(&mut container, &rolled, &reg, &mut alloc);

        assert_eq!(
            placed, 12,
            "4x3 grid fits exactly 12 1x1 items; 3 overflow should be dropped"
        );
        assert_eq!(
            container.items.len(),
            12,
            "container should hold all 12 placed items"
        );
    }

    #[test]
    fn pack_multi_cell_items() {
        let reg = make_registry(vec![("wide", 2, 2)]);
        let mut alloc = make_allocator();
        let mut container = make_container(4, 4);
        let rolled: Vec<_> = (0..5).map(|_| ("wide".to_string(), 1u8)).collect();

        let placed = pack_loot_into_grid(&mut container, &rolled, &reg, &mut alloc);

        assert_eq!(placed, 4, "4x4 grid fits 4 items of 2x2");
    }

    #[test]
    fn remove_item_by_instance_id() {
        let mut container = make_container(4, 3);
        container.items.push(PlacedItemState {
            row: 0,
            col: 0,
            instance: ItemInstance {
                instance_id: 42,
                template_id: "ore".to_string(),
                display_name: "Ore".to_string(),
                grid_w: 1,
                grid_h: 1,
                weight: 1.0,
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
            },
        });

        let removed = remove_item_from_container(&mut container, 42);
        assert!(
            removed.is_some(),
            "should find and remove item with instance_id 42"
        );
        assert_eq!(
            container.items.len(),
            0,
            "container should be empty after removing the only item"
        );

        let missing = remove_item_from_container(&mut container, 999);
        assert!(
            missing.is_none(),
            "removing non-existent instance_id 999 should return None"
        );
    }

    #[test]
    fn place_item_validates_bounds_and_overlap() {
        let mut container = make_container(4, 3);
        let instance = ItemInstance {
            instance_id: 1,
            template_id: "t".to_string(),
            display_name: "T".to_string(),
            grid_w: 2,
            grid_h: 2,
            weight: 1.0,
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
        };

        // Out of bounds: 2x2 at (2,3) → row+h=4 > 3
        assert!(
            place_item_into_container(&mut container, 2, 3, instance.clone()).is_err(),
            "2x2 item at (2,3) in 4x3 container should fail: row+grid_h=4 > rows=3"
        );

        // Valid placement
        assert!(
            place_item_into_container(&mut container, 0, 0, instance.clone()).is_ok(),
            "2x2 item at (0,0) in 4x3 container should succeed"
        );
        assert_eq!(
            container.items.len(),
            1,
            "container should hold 1 item after valid placement"
        );

        // Overlap
        let mut instance2 = instance.clone();
        instance2.instance_id = 2;
        assert!(
            place_item_into_container(&mut container, 0, 1, instance2).is_err(),
            "2x2 item at (0,1) should fail: overlaps existing 2x2 item at (0,0)"
        );
    }

    #[test]
    fn pack_empty_rolled_array() {
        let reg = make_registry(vec![("ore_a", 1, 1)]);
        let mut alloc = make_allocator();
        let mut container = make_container(4, 3);
        let rolled: Vec<(String, u8)> = vec![];

        let placed = pack_loot_into_grid(&mut container, &rolled, &reg, &mut alloc);

        assert_eq!(placed, 0, "no items should be placed from empty roll");
        assert_eq!(container.items.len(), 0, "container should remain empty");
    }

    #[test]
    fn pack_skips_items_when_allocator_exhausted() {
        use crate::inventory::JS_SAFE_INTEGER_MAX;

        let reg = make_registry(vec![("ore_a", 1, 1)]);
        let mut alloc = InventoryInstanceIdAllocator::new(JS_SAFE_INTEGER_MAX);
        let mut container = make_container(4, 3);
        let rolled = vec![
            ("ore_a".to_string(), 1),
            ("ore_a".to_string(), 1),
            ("ore_a".to_string(), 1),
        ];

        let placed = pack_loot_into_grid(&mut container, &rolled, &reg, &mut alloc);

        assert_eq!(
            placed, 1,
            "only the first item should be placed before allocator hits JS_SAFE_INTEGER_MAX"
        );
    }

    #[test]
    fn place_item_at_exact_boundary() {
        let mut container = make_container(4, 3);
        let instance = ItemInstance {
            instance_id: 1,
            template_id: "t".to_string(),
            display_name: "T".to_string(),
            grid_w: 2,
            grid_h: 2,
            weight: 1.0,
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
        };

        // 2x2 at (1,2): row+h=3==rows, col+w=4==cols → exact boundary, should succeed
        assert!(
            place_item_into_container(&mut container, 1, 2, instance.clone()).is_ok(),
            "2x2 item at (1,2) in 4x3 grid should fit exactly at boundary"
        );
        assert_eq!(
            container.items.len(),
            1,
            "item should be placed at boundary"
        );

        // Off-by-one: 2x2 at (2,2): row+h=4 > rows=3
        let mut i2 = instance.clone();
        i2.instance_id = 2;
        assert!(
            place_item_into_container(&mut container, 2, 2, i2).is_err(),
            "2x2 item at (2,2) should fail: row+grid_h=4 > rows=3"
        );

        // Off-by-one: 2x2 at (1,3): col+w=5 > cols=4
        let mut i3 = instance;
        i3.instance_id = 3;
        assert!(
            place_item_into_container(&mut container, 1, 3, i3).is_err(),
            "2x2 item at (1,3) should fail: col+grid_w=5 > cols=4"
        );
    }

    #[test]
    fn place_item_rejects_overflowing_coordinates() {
        let mut container = make_container(4, 3);
        let instance = ItemInstance {
            instance_id: 1,
            template_id: "t".to_string(),
            display_name: "T".to_string(),
            grid_w: 2,
            grid_h: 2,
            weight: 1.0,
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
        };

        assert!(
            place_item_into_container(&mut container, u8::MAX, 0, instance.clone()).is_err(),
            "overflowing row + grid_h must be rejected instead of panicking or wrapping"
        );
        assert!(
            place_item_into_container(&mut container, 0, u8::MAX, instance).is_err(),
            "overflowing col + grid_w must be rejected instead of panicking or wrapping"
        );
        assert!(
            container.items.is_empty(),
            "rejected overflow coordinates must not mutate the container"
        );
    }

    #[test]
    fn registry_allocate_and_remove_session() {
        let mut reg = ExternalContainerRegistry::default();
        let entity = Entity::from_raw(42);

        let s1 = reg.allocate_session(entity);
        let s2 = reg.allocate_session(entity);
        assert_ne!(s1, s2);
        assert_eq!(reg.sessions.len(), 2);

        reg.remove_session(s1);
        assert_eq!(reg.sessions.len(), 1);
        assert!(reg.sessions.contains_key(&s2));
    }

    #[test]
    fn container_id_format() {
        assert_eq!(ExternalContainer::container_id(42), "ext_42");
        assert_eq!(ExternalContainer::container_id(0), "ext_0");
    }

    #[test]
    fn external_kind_to_source_kind_maps_all_variants() {
        assert_eq!(
            external_kind_to_source_kind(&ExternalContainerKind::SupplyCoffin {
                grade: SupplyCoffinGrade::Common
            }),
            LootContainerSourceKindV1::SupplyCoffin {
                grade: "common".to_string()
            },
            "supply coffin source kind must preserve common grade"
        );
        assert_eq!(
            external_kind_to_source_kind(&ExternalContainerKind::SupplyCoffin {
                grade: SupplyCoffinGrade::Rare
            }),
            LootContainerSourceKindV1::SupplyCoffin {
                grade: "rare".to_string()
            },
            "supply coffin source kind must preserve rare grade"
        );
        assert_eq!(
            external_kind_to_source_kind(&ExternalContainerKind::SupplyCoffin {
                grade: SupplyCoffinGrade::Precious
            }),
            LootContainerSourceKindV1::SupplyCoffin {
                grade: "precious".to_string()
            },
            "supply coffin source kind must preserve precious grade"
        );
        assert_eq!(
            external_kind_to_source_kind(&ExternalContainerKind::StorageCrate { is_herb: false }),
            LootContainerSourceKindV1::StorageCrate { is_herb: false },
            "trade crate source kind must preserve is_herb=false"
        );
        assert_eq!(
            external_kind_to_source_kind(&ExternalContainerKind::StorageCrate { is_herb: true }),
            LootContainerSourceKindV1::StorageCrate { is_herb: true },
            "herb crate source kind must preserve is_herb=true"
        );
        assert_eq!(
            external_kind_to_source_kind(&ExternalContainerKind::DeadDrop),
            LootContainerSourceKindV1::DeadDrop,
            "dead drop source kind must be distinct"
        );
    }
}
