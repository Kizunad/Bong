//! 库存布局迁移的纯函数。
//!
//! 本模块只把旧版 JSON 布局转换为当前布局，并把明确标记为溢出的实例分离出来；
//! 不连接 SQLite、不猜玩家位置，也不负责把溢出物写入地面。R3 的 hydration 与
//! 后续 R10 transaction 会在拥有真实上下文后消费这里的结果。

use std::collections::{BTreeMap, HashMap};

use serde_json::{json, Map, Value};

use super::{container_id_for_worn_pack, ItemInstance, TREASURE_TRIGGER_CAP};

/// 当前布局 helper 能理解的最高 schema 版本。
pub const CURRENT_INVENTORY_LAYOUT_SCHEMA_VERSION: i32 = 2;

/// 旧布局转换失败时的原因；失败结果始终保留原始 JSON，方便重试。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventoryLayoutMigrationError {
    /// 根值不是可迁移的 JSON object。
    RootNotObject,
    /// `equipped` 存在但不是 object，无法判断槽位归属。
    EquippedNotObject,
    /// 存档版本高于当前代码，禁止猜测字段含义。
    UnsupportedSchema { found: i32, supported: i32 },
    /// 顶层 overflow 不是数组。
    OverflowNotArray,
    /// overflow 数组中的某一项不是完整的 ItemInstance。
    InvalidOverflowItem { index: usize },
}

/// 纯布局迁移的结果。
///
/// `migrated_value` 是可安全交给下一层解码的 JSON；`overflow` 只包含旧存档明确
/// 放在顶层 `overflow` 数组中的完整实例。`error` 非空时，两个结果字段都保持输入
/// 的保守副本，调用方不得写入新 schema。
#[derive(Debug, Clone, PartialEq)]
pub struct MigrationOutcome {
    /// 迁移后的 JSON，失败时等于输入副本。
    pub migrated_value: Value,
    /// 不属于布局本体、需要真实容量上下文处理的实例。
    pub overflow: Vec<ItemInstance>,
    /// 失败原因；`None` 表示可以继续解码。
    pub error: Option<InventoryLayoutMigrationError>,
    /// 输入是否发生了可观察的布局变化。
    pub changed: bool,
}

impl MigrationOutcome {
    /// 报告结果是否可以交给下一层解码和事务处理。
    pub fn is_valid(&self) -> bool {
        self.error.is_none()
    }
}

/// 将旧版 inventory JSON 转成当前布局。
///
/// 函数是幂等的：对已迁移的 v2 JSON 再调用不会重复创建槽位、触发位或容器。
/// 失败不会吞掉 overflow，也不会修改传入的原值（值按 move 传入）。
pub fn migrate_legacy_inventory_layout(value: Value, schema_version: i32) -> MigrationOutcome {
    let original = value.clone();
    if schema_version > CURRENT_INVENTORY_LAYOUT_SCHEMA_VERSION {
        return failed(
            original,
            InventoryLayoutMigrationError::UnsupportedSchema {
                found: schema_version,
                supported: CURRENT_INVENTORY_LAYOUT_SCHEMA_VERSION,
            },
        );
    }

    let mut migrated_value = value;
    let Some(root) = migrated_value.as_object_mut() else {
        return failed(original, InventoryLayoutMigrationError::RootNotObject);
    };

    let had_overflow = root.contains_key("overflow");
    let overflow = match take_overflow(root) {
        Ok(items) => items,
        Err(error) => return failed(original, error),
    };

    let before_equipped = root.get("equipped").cloned();
    if let Some(equipped) = root.get("equipped") {
        if !equipped.is_object() {
            return failed(original, InventoryLayoutMigrationError::EquippedNotObject);
        }
    }
    migrate_legacy_equipped(root);
    let changed = before_equipped != root.get("equipped").cloned() || had_overflow;

    MigrationOutcome {
        migrated_value,
        overflow,
        error: None,
        changed,
    }
}

/// 兼容旧 player-state loader 的 v1→v2 equipped 迁移入口。
///
/// 旧 loader 仍拥有 schema 分流；它调用此纯函数时只传入 `Value`，因此不会改变
/// 原有默认 loadout 或持久化时序。
pub fn migrate_equipped_v1_to_v2(value: &mut Value) {
    let input = std::mem::replace(value, Value::Null);
    let outcome = migrate_legacy_inventory_layout(input, 1);
    *value = outcome.migrated_value;
}

fn failed(value: Value, error: InventoryLayoutMigrationError) -> MigrationOutcome {
    MigrationOutcome {
        migrated_value: value,
        overflow: Vec::new(),
        error: Some(error),
        changed: false,
    }
}

fn take_overflow(
    root: &mut Map<String, Value>,
) -> Result<Vec<ItemInstance>, InventoryLayoutMigrationError> {
    let Some(raw_overflow) = root.remove("overflow") else {
        return Ok(Vec::new());
    };
    let Some(items) = raw_overflow.as_array() else {
        return Err(InventoryLayoutMigrationError::OverflowNotArray);
    };
    items
        .iter()
        .enumerate()
        .map(|(index, value)| {
            serde_json::from_value(value.clone())
                .map_err(|_| InventoryLayoutMigrationError::InvalidOverflowItem { index })
        })
        .collect()
}

fn migrate_legacy_equipped(root: &mut Map<String, Value>) {
    let Some(equipped) = root.get_mut("equipped").and_then(Value::as_object_mut) else {
        return;
    };
    let old = std::mem::take(equipped);
    let mut triggered: BTreeMap<String, Value> = BTreeMap::new();
    let mut new_slots: HashMap<String, (Vec<Value>, Option<Value>)> = HashMap::new();
    let mut legacy_pack_container_renames: HashMap<String, u64> = HashMap::new();

    for (old_slot, item) in old {
        if item.get("worn").is_some() || item.get("held").is_some() {
            let entry = new_slots.entry(old_slot).or_default();
            if let Some(worn) = item.get("worn").and_then(Value::as_array) {
                entry.0.extend(worn.iter().cloned());
            }
            if let Some(held) = item.get("held") {
                if !held.is_null() {
                    entry.1 = Some(held.clone());
                }
            }
            continue;
        }

        match old_slot.as_str() {
            "false_skin" => push_worn(&mut new_slots, "chest", item),
            "two_hand" => set_held(&mut new_slots, "main_hand", item),
            "treasure_belt_0" | "treasure_belt_1" | "treasure_belt_2" | "treasure_belt_3" => {
                triggered.insert(old_slot, item);
            }
            "back_pack" | "waist_pouch" | "chest_satchel" => {
                if let Some(instance_id) = item.get("instance_id").and_then(Value::as_u64) {
                    legacy_pack_container_renames.insert(old_slot, instance_id);
                }
                push_worn(&mut new_slots, "chest", item);
            }
            "head" | "chest" | "legs" | "feet" => push_worn(&mut new_slots, &old_slot, item),
            "main_hand" | "off_hand" | "extra_hand_0" | "extra_hand_1" => {
                set_held(&mut new_slots, &old_slot, item);
            }
            other => push_worn(&mut new_slots, other, item),
        }
    }

    let rebuilt = root
        .get_mut("equipped")
        .and_then(Value::as_object_mut)
        .expect("equipped object was present before migration");
    for (slot, (worn, held)) in new_slots {
        rebuilt.insert(
            slot,
            json!({ "worn": worn, "held": held.unwrap_or(Value::Null) }),
        );
    }

    if !legacy_pack_container_renames.is_empty() {
        if let Some(containers) = root.get_mut("containers").and_then(Value::as_array_mut) {
            for container in containers {
                let Some(container_id) = container.get("id").and_then(Value::as_str) else {
                    continue;
                };
                let Some(instance_id) = legacy_pack_container_renames.get(container_id) else {
                    continue;
                };
                if let Some(object) = container.as_object_mut() {
                    object.insert(
                        "id".to_string(),
                        Value::String(container_id_for_worn_pack(*instance_id)),
                    );
                }
            }
        }
    }

    if !triggered.is_empty() {
        let trigger_items = triggered
            .into_values()
            .take(TREASURE_TRIGGER_CAP)
            .collect::<Vec<_>>();
        root.insert(
            "triggered_treasures".to_string(),
            Value::Array(trigger_items),
        );
    }
}

fn push_worn(slots: &mut HashMap<String, (Vec<Value>, Option<Value>)>, slot: &str, item: Value) {
    slots.entry(slot.to_string()).or_default().0.push(item);
}

fn set_held(slots: &mut HashMap<String, (Vec<Value>, Option<Value>)>, slot: &str, item: Value) {
    slots.entry(slot.to_string()).or_default().1 = Some(item);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: u64, template_id: &str) -> Value {
        json!({
            "instance_id": id,
            "template_id": template_id,
            "display_name": template_id,
            "grid_w": 1,
            "grid_h": 1,
            "weight": 1.0,
            "rarity": "Common",
            "description": "",
            "stack_count": 1,
            "spirit_quality": 0.25,
            "durability": 1.0,
            "freshness": null,
            "mineral_id": null,
            "charges": null,
            "forge_quality": null,
            "forge_color": null,
            "forge_side_effects": [],
            "forge_achieved_tier": null,
            "alchemy": null,
            "lingering_owner_qi": null
        })
    }

    #[test]
    fn legacy_layout_migration_is_idempotent_and_preserves_dynamic_instance_fields() {
        let input = json!({
            "revision": 4,
            "containers": [{"id": "back_pack", "rows": 1, "cols": 1, "items": []}],
            "equipped": {"back_pack": item(7, "worn_grass_pouch")},
            "hotbar": [null, null],
            "bone_coins": 0,
            "max_weight": 15.0
        });

        let first = migrate_legacy_inventory_layout(input, 1);
        assert!(first.is_valid());
        let second = migrate_legacy_inventory_layout(first.migrated_value.clone(), 2);
        assert!(second.is_valid());
        assert_eq!(second.migrated_value, first.migrated_value);
        let item = second.migrated_value["equipped"]["chest"]["worn"][0].clone();
        assert_eq!(item["instance_id"], 7);
        assert_eq!(item["template_id"], "worn_grass_pouch");
    }

    #[test]
    fn explicit_overflow_is_returned_without_being_dropped_or_rewritten() {
        let input = json!({"overflow": [item(11, "ore")], "equipped": {}});
        let outcome = migrate_legacy_inventory_layout(input, 1);
        assert!(outcome.is_valid());
        assert_eq!(outcome.overflow.len(), 1);
        assert_eq!(outcome.overflow[0].instance_id, 11);
        assert!(outcome.migrated_value.get("overflow").is_none());
    }

    #[test]
    fn invalid_layout_keeps_original_value_for_retry() {
        let input = json!({"equipped": "corrupt"});
        let outcome = migrate_legacy_inventory_layout(input.clone(), 1);
        assert_eq!(
            outcome.error,
            Some(InventoryLayoutMigrationError::EquippedNotObject)
        );
        assert_eq!(outcome.migrated_value, input);
        assert!(outcome.overflow.is_empty());
    }
}
