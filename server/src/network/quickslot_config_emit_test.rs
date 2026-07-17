//! plan-skill-av-relink-v1 P3 —— quickslot_config 发射契约测试（图标链）。
//!
//! 直测 `build_quickslot_config`（emit 系统与 bind ack 回推共用的唯一构造入口，
//! 免 mock client）。核心契约：quickslot 是纯 Item 槽，**icon_texture 恒空串**
//! （P0 决议：client 按 item_id 走 ItemIconRegistry 富解析——tools/ 映射、armor
//! tint、broken_artifact 兜底；server 回填路径会绕过富解析造成图标回归）。

use std::collections::HashMap;

use super::quickslot_config_emit::build_quickslot_config;
use crate::combat::components::QuickSlotBindings;
use crate::inventory::{
    ContainerState, InventoryRevision, ItemCategory, ItemInstance, ItemRarity, ItemRegistry,
    ItemTemplate, PlacedItemState, PlayerInventory, DEFAULT_CAST_DURATION_MS, DEFAULT_COOLDOWN_MS,
};

const NOW_TICK: u64 = 10;
const NOW_MS: u64 = 1_000;

fn empty_inventory() -> PlayerInventory {
    PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(0),
        containers: vec![ContainerState {
            quick_access: false,
            id: "main_pack".to_string(),
            name: "main_pack".to_string(),
            rows: 5,
            cols: 7,
            items: Vec::new(),
            owner_instance_id: None,
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
        placeable: None,
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
        readable_scroll_spec: None,
        recipe_fragment_spec: None,
        container_spec: None,
        shelflife_profile: None,
        shield_spec: None,
        shelflife_track: None,
        wearer_race: crate::body_plan::types::RaceGateOwned::default(),
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

fn tea_registry() -> ItemRegistry {
    ItemRegistry::from_map(HashMap::from([(
        "tea".to_string(),
        template("tea", "清茶"),
    )]))
}

fn empty_registry() -> ItemRegistry {
    ItemRegistry::from_map(HashMap::new())
}

/// happy path：绑定背包内物品的槽下发完整模板字段，icon_texture 恒空串 pin。
#[test]
fn quickslot_bound_item_slot_pins_empty_icon_texture_and_template_fields() {
    let mut inventory = empty_inventory();
    inventory.containers[0].items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance: item_instance(42, "tea"),
    });
    let mut bindings = QuickSlotBindings::default();
    assert!(bindings.set(0, Some(42)));

    let config = build_quickslot_config(
        Some(&bindings),
        Some(&inventory),
        &tea_registry(),
        NOW_TICK,
        NOW_MS,
        None,
        None,
    );

    assert_eq!(
        config.slots.len(),
        QuickSlotBindings::SLOT_COUNT,
        "quickslot_config.slots 长度必须恒为 {}（client 按 idx 取）",
        QuickSlotBindings::SLOT_COUNT
    );
    assert_eq!(
        config.cooldown_until_ms.len(),
        QuickSlotBindings::SLOT_COUNT
    );
    let entry = config.slots[0]
        .as_ref()
        .expect("绑定了背包内物品的槽 0 应下发条目");
    assert_eq!(entry.item_id, "tea");
    assert_eq!(entry.display_name, "清茶");
    assert_eq!(entry.cast_duration_ms, 250);
    assert_eq!(entry.cooldown_ms, 1250);
    assert!(
        entry.icon_texture.is_empty(),
        "quickslot Item 槽 icon_texture 必须恒为空串（P0 显式契约：client 按 item_id 走 \
         ItemIconRegistry 富解析，server 回填路径会绕过它），实际 `{}`",
        entry.icon_texture
    );
    for slot in 1..QuickSlotBindings::SLOT_COUNT {
        assert!(
            config.slots[slot].is_none(),
            "未绑定的槽 {slot} 应下发 None"
        );
    }
    assert_eq!(config.ack_request_id, None, "普通广播不携带 ack");
    assert_eq!(config.bind_accepted, None, "普通广播不携带 bind_accepted");
}

/// 全 9 槽满绑（背包 + hotbar 两条 lookup 路径混合）：每个下发条目的
/// icon_texture 都必须是空串——契约对槽位与物品来源不敏感。
#[test]
fn quickslot_every_bound_slot_emits_empty_icon_texture_across_container_and_hotbar() {
    let mut inventory = empty_inventory();
    for i in 0..5u64 {
        inventory.containers[0].items.push(PlacedItemState {
            row: 0,
            col: i as u8,
            instance: item_instance(100 + i, "tea"),
        });
    }
    for i in 5..9u64 {
        inventory.hotbar[i as usize] = Some(item_instance(100 + i, "tea"));
    }
    let mut bindings = QuickSlotBindings::default();
    for slot in 0..QuickSlotBindings::SLOT_COUNT {
        assert!(bindings.set(slot as u8, Some(100 + slot as u64)));
    }

    let config = build_quickslot_config(
        Some(&bindings),
        Some(&inventory),
        &tea_registry(),
        NOW_TICK,
        NOW_MS,
        None,
        None,
    );

    for slot in 0..QuickSlotBindings::SLOT_COUNT {
        let entry = config.slots[slot]
            .as_ref()
            .unwrap_or_else(|| panic!("满绑时槽 {slot} 应下发条目"));
        assert_eq!(
            entry.item_id, "tea",
            "槽 {slot} 应回发绑定物品的 template id"
        );
        assert!(
            entry.icon_texture.is_empty(),
            "槽 {slot}（来源 {}）icon_texture 必须恒为空串，实际 `{}`",
            if slot < 5 { "背包容器" } else { "hotbar" },
            entry.icon_texture
        );
    }
}

/// 绑定的 instance_id 在背包/装备/hotbar 全都解析不到 → 槽位下发 None。
#[test]
fn quickslot_unresolvable_instance_id_emits_none() {
    let mut bindings = QuickSlotBindings::default();
    assert!(bindings.set(0, Some(9999)));

    let config = build_quickslot_config(
        Some(&bindings),
        Some(&empty_inventory()),
        &tea_registry(),
        NOW_TICK,
        NOW_MS,
        None,
        None,
    );

    assert!(
        config.slots[0].is_none(),
        "解析不到的 instance_id 不应下发条目（悬空绑定静默降级），实际 {:?}",
        config.slots[0]
    );
}

/// 物品模板不在注册表 → display_name 回退 template_id、cast/cooldown 走模板默认
/// 常数，icon_texture 仍为空串（兜底分支不破坏契约）。
#[test]
fn quickslot_missing_template_falls_back_to_defaults_icon_still_empty() {
    let mut inventory = empty_inventory();
    inventory.containers[0].items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance: item_instance(7, "ghost_item"),
    });
    let mut bindings = QuickSlotBindings::default();
    assert!(bindings.set(0, Some(7)));

    let config = build_quickslot_config(
        Some(&bindings),
        Some(&inventory),
        &empty_registry(),
        NOW_TICK,
        NOW_MS,
        None,
        None,
    );

    let entry = config.slots[0]
        .as_ref()
        .expect("模板缺失不应吞掉条目，应走默认值兜底");
    assert_eq!(
        entry.display_name, "ghost_item",
        "模板缺失时 display_name 应回退 template_id"
    );
    assert_eq!(entry.cast_duration_ms, DEFAULT_CAST_DURATION_MS);
    assert_eq!(entry.cooldown_ms, DEFAULT_COOLDOWN_MS);
    assert!(
        entry.icon_texture.is_empty(),
        "模板缺失兜底分支的 icon_texture 也必须为空串，实际 `{}`",
        entry.icon_texture
    );
}

/// bindings 缺失（None）→ 9 槽全 None、9 冷却全 0（空配置仍保持定长契约）。
#[test]
fn quickslot_missing_bindings_yield_empty_fixed_length_config() {
    let config = build_quickslot_config(
        None,
        Some(&empty_inventory()),
        &empty_registry(),
        NOW_TICK,
        NOW_MS,
        None,
        None,
    );

    assert_eq!(config.slots.len(), QuickSlotBindings::SLOT_COUNT);
    assert!(config.slots.iter().all(Option::is_none));
    assert_eq!(
        config.cooldown_until_ms,
        vec![0; QuickSlotBindings::SLOT_COUNT],
        "无 bindings 时冷却应全 0"
    );
}

/// inventory 缺失（None）但绑定存在 → 槽位下发 None（不 panic）。
#[test]
fn quickslot_missing_inventory_emits_none_for_bound_slot() {
    let mut bindings = QuickSlotBindings::default();
    assert!(bindings.set(0, Some(42)));

    let config = build_quickslot_config(
        Some(&bindings),
        None,
        &tea_registry(),
        NOW_TICK,
        NOW_MS,
        None,
        None,
    );

    assert!(
        config.slots[0].is_none(),
        "无 inventory 时绑定槽应下发 None，实际 {:?}",
        config.slots[0]
    );
}

/// 冷却 tick→ms 折算边界：恰等于 now_tick 不算冷却（严格大于才折算）、
/// now_tick+1 折算一个 tick、过期与未设置均为 0。
#[test]
fn quickslot_cooldown_tick_to_ms_conversion_boundaries() {
    let mut bindings = QuickSlotBindings::default();
    bindings.set_cooldown(0, NOW_TICK); // 恰等于 now → 0（off-by-one 边界）
    bindings.set_cooldown(1, NOW_TICK + 1); // 剩 1 tick → now_ms + 50
    bindings.set_cooldown(2, NOW_TICK - 1); // 已过期 → 0
    bindings.set_cooldown(3, NOW_TICK + 100); // 剩 100 tick → now_ms + 5000
                                              // 槽 4..9 未设置 → 0

    let config = build_quickslot_config(
        Some(&bindings),
        Some(&empty_inventory()),
        &empty_registry(),
        NOW_TICK,
        NOW_MS,
        None,
        None,
    );

    assert_eq!(
        config.cooldown_until_ms[0], 0,
        "cd_tick == now_tick 不算冷却（严格大于才折算）"
    );
    assert_eq!(
        config.cooldown_until_ms[1],
        NOW_MS + 50,
        "剩 1 tick 应折算 now_ms + 50ms"
    );
    assert_eq!(config.cooldown_until_ms[2], 0, "已过期的冷却应为 0");
    assert_eq!(
        config.cooldown_until_ms[3],
        NOW_MS + 100 * 50,
        "剩 100 tick 应折算 now_ms + 5000ms"
    );
    for slot in 4..QuickSlotBindings::SLOT_COUNT {
        assert_eq!(
            config.cooldown_until_ms[slot], 0,
            "未设置冷却的槽 {slot} 应为 0"
        );
    }
}

/// bind ack 回推参数透传：ack_request_id / bind_accepted 原样出现在 payload。
#[test]
fn quickslot_ack_request_id_and_bind_accepted_pass_through() {
    let accepted = build_quickslot_config(
        None,
        None,
        &empty_registry(),
        NOW_TICK,
        NOW_MS,
        Some("req-7".to_string()),
        Some(true),
    );
    assert_eq!(accepted.ack_request_id.as_deref(), Some("req-7"));
    assert_eq!(accepted.bind_accepted, Some(true));

    let rejected = build_quickslot_config(
        None,
        None,
        &empty_registry(),
        NOW_TICK,
        NOW_MS,
        Some("req-8".to_string()),
        Some(false),
    );
    assert_eq!(rejected.ack_request_id.as_deref(), Some("req-8"));
    assert_eq!(
        rejected.bind_accepted,
        Some(false),
        "拒绝分支的 false 必须原样透传（client 靠它回滚本地乐观状态）"
    );
}
