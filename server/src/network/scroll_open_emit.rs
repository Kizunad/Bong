//! `ScrollOpen` S2C 回执发送 —— 可阅读残卷阅读屏。
//!
//! plan-scroll-reading-v1 P0：把 C2S `ScrollReadRequest { instance_id }` 解析为
//! `ServerDataPayloadV1::ScrollOpen` 并只发给发起请求的玩家（不广播）。读取不消耗物品。
//!
//! 模板：`inventory_move_rejected_emit.rs`（targeted-only 单玩家回执范式）。
//! 校验分支（`ScrollReadRejectReason`）覆盖 §9 描述的三类静默拒绝：
//! 实例不属于该玩家背包 / 模板未注册 / 模板无 `readable_scroll_spec`。

use valence::prelude::{Client, Entity, Query, Username};

use crate::inventory::{inventory_item_by_instance_borrow, ItemRegistry, PlayerInventory};
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

/// `ScrollReadRequest` 静默拒绝原因。三者都只 warn + 不回执，不向 client 暴露区分信息
/// （避免给作弊 client 提供 instance_id 探测信号）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScrollReadRejectReason {
    /// `instance_id` 未在该玩家 `PlayerInventory` 的 containers/equipped/hotbar 任一处找到——
    /// 覆盖"物品不存在"与"物品属于别的玩家"两种情况（后者天然不会出现在此玩家的 inventory 里）。
    InstanceNotInInventory,
    /// 该 instance 的 `template_id` 未注册进 `ItemRegistry`（数据损坏 / 测试伪造 id）。
    UnknownTemplate { template_id: String },
    /// 模板存在但 `readable_scroll_spec` 为 `None`（该物品不是可阅读残卷）。
    NoReadableScrollSpec { template_id: String },
}

/// `resolve_scroll_read_request` 成功结果：S2C payload + 可选阅读循环动画 id。
///
/// `anim_id` 与 `payload` 共享同一次模板查找（避免调用方对同一 instance_id 二次查 registry）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScrollReadResolution {
    pub payload_scroll_id: String,
    pub payload_title: String,
    pub payload_body_pages: Vec<String>,
    /// server 在 `ScrollOpen` 同帧 emit 的循环姿态动画 id（`readable_scroll_spec.anim_id`
    /// 若为 `None`，该残卷无阅读动画，调用方不得 emit `play_anim`）。
    pub anim_id: Option<String>,
}

impl ScrollReadResolution {
    /// 组装成 `ServerDataPayloadV1::ScrollOpen`（供 `emit_scroll_open` 直接消费）。
    pub fn into_payload(self) -> ServerDataPayloadV1 {
        ServerDataPayloadV1::ScrollOpen {
            scroll_id: self.payload_scroll_id,
            title: self.payload_title,
            body_pages: self.payload_body_pages,
        }
    }
}

/// 解析 `ScrollReadRequest`：在玩家背包中定位 instance → 查模板 → 取
/// `readable_scroll_spec` → 组装 `ScrollReadResolution`（S2C payload 字段 + anim_id）。
///
/// 只读，不消耗物品、不改动 inventory/state（区别于 `consume_item_instance_once` 消耗式）。
pub fn resolve_scroll_read_request(
    inventory: &PlayerInventory,
    registry: &ItemRegistry,
    instance_id: u64,
) -> Result<ScrollReadResolution, ScrollReadRejectReason> {
    let item = inventory_item_by_instance_borrow(inventory, instance_id)
        .ok_or(ScrollReadRejectReason::InstanceNotInInventory)?;
    let template_id = item.template_id.clone();
    let template = registry.get(template_id.as_str()).ok_or_else(|| {
        ScrollReadRejectReason::UnknownTemplate {
            template_id: template_id.clone(),
        }
    })?;
    let spec = template.readable_scroll_spec.as_ref().ok_or_else(|| {
        ScrollReadRejectReason::NoReadableScrollSpec {
            template_id: template_id.clone(),
        }
    })?;
    Ok(ScrollReadResolution {
        payload_scroll_id: template_id,
        payload_title: spec.title.clone(),
        payload_body_pages: spec.body_pages.clone(),
        anim_id: spec.anim_id.clone(),
    })
}

/// 组装 `ScrollOpen` payload 并只发给触发者 `entity`（不广播）。
///
/// 找不到该 entity 对应的 `Client`（已断线 / 测试里未 spawn）时静默跳过，模式照抄
/// `emit_inventory_move_rejected`。
pub fn emit_scroll_open(
    entity: Entity,
    payload: ServerDataPayloadV1,
    clients: &mut Query<(&Username, &mut Client)>,
) {
    let envelope = ServerDataV1::new(payload);
    let payload_type = payload_type_label(envelope.payload_type());
    let payload_bytes = match serialize_server_data_payload(&envelope) {
        Ok(bytes) => bytes,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };
    if let Ok((_, mut client)) = clients.get_mut(entity) {
        send_server_data_payload(&mut client, payload_bytes.as_slice());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemCategory, ItemInstance, ItemRarity, ItemTemplate,
        PlacedItemState, ReadableScrollSpec, MAIN_PACK_CONTAINER_ID,
    };
    use std::collections::HashMap;

    fn test_item(instance_id: u64, template_id: &str) -> ItemInstance {
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

    fn empty_inventory() -> PlayerInventory {
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(1),
            containers: vec![ContainerState {
                quick_access: false,
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 3,
                cols: 3,
                items: Vec::new(),
                owner_instance_id: None,
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 45.0,
        }
    }

    fn inventory_with_items(items: Vec<(&str, u64)>) -> PlayerInventory {
        let mut inv = empty_inventory();
        for (idx, (template_id, instance_id)) in items.iter().enumerate() {
            inv.containers[0].items.push(PlacedItemState {
                row: idx as u8,
                col: 0,
                instance: test_item(*instance_id, template_id),
            });
        }
        inv
    }

    fn base_template(id: &str, category: ItemCategory) -> ItemTemplate {
        ItemTemplate {
            id: id.to_string(),
            display_name: id.to_string(),
            category,
            placeable: None,
            max_stack_count: 1,
            grid_w: 1,
            grid_h: 2,
            base_weight: 0.05,
            rarity: ItemRarity::Common,
            spirit_quality_initial: 0.3,
            description: "test".to_string(),
            effect: None,
            cast_duration_ms: 1500,
            cooldown_ms: 1500,
            weapon_spec: None,
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
            technique_scroll_spec: None,
            readable_scroll_spec: None,
            recipe_fragment_spec: None,
            container_spec: None,
            shield_spec: None,
            shelflife_profile: None,
            shelflife_track: None,
        }
    }

    fn readable_scroll_template(id: &str) -> ItemTemplate {
        ItemTemplate {
            readable_scroll_spec: Some(ReadableScrollSpec {
                title: "《经脉浅述·残卷》".to_string(),
                body_pages: vec!["第一页".to_string(), "第二页".to_string()],
                anim_id: Some("bong:read_scroll".to_string()),
            }),
            ..base_template(id, ItemCategory::Scroll)
        }
    }

    fn registry_with(templates: Vec<ItemTemplate>) -> ItemRegistry {
        let mut map = HashMap::new();
        for t in templates {
            map.insert(t.id.clone(), t);
        }
        ItemRegistry::from_map(map)
    }

    // ── happy path ──────────────────────────────────────────────────

    #[test]
    fn resolves_scroll_open_for_readable_scroll_in_inventory() {
        let registry = registry_with(vec![readable_scroll_template("scroll_meridian_primer")]);
        let inventory = inventory_with_items(vec![("scroll_meridian_primer", 42)]);

        let result = resolve_scroll_read_request(&inventory, &registry, 42);

        match result {
            Ok(resolution) => {
                assert_eq!(resolution.payload_scroll_id, "scroll_meridian_primer");
                assert_eq!(resolution.payload_title, "《经脉浅述·残卷》");
                assert_eq!(
                    resolution.payload_body_pages,
                    vec!["第一页".to_string(), "第二页".to_string()],
                    "body_pages 必须原样透传 spec 内容，供 client 阅读屏渲染"
                );
                assert_eq!(
                    resolution.anim_id.as_deref(),
                    Some("bong:read_scroll"),
                    "spec 挂了 anim_id 时必须原样透传"
                );
            }
            other => panic!("expected Ok(ScrollReadResolution), got {other:?}"),
        }
    }

    #[test]
    fn resolves_scroll_open_without_anim_id_when_spec_has_none() {
        // §8.1 #1：anim_id 是 Option——残卷不强制有阅读动画，None 时调用方不应 emit play_anim。
        let mut template = readable_scroll_template("scroll_meridian_primer");
        template.readable_scroll_spec.as_mut().unwrap().anim_id = None;
        let registry = registry_with(vec![template]);
        let inventory = inventory_with_items(vec![("scroll_meridian_primer", 42)]);

        let result = resolve_scroll_read_request(&inventory, &registry, 42)
            .expect("readable scroll without anim_id should still resolve");

        assert_eq!(
            result.anim_id, None,
            "spec.anim_id=None 必须原样透传为 None，不得凭空发明动画 id"
        );
    }

    #[test]
    fn resolve_does_not_mutate_inventory() {
        // 读取不消耗（区别于 technique_scroll 消耗式）：instance 在解析前后必须原样保留。
        let registry = registry_with(vec![readable_scroll_template("scroll_meridian_primer")]);
        let mut inventory = inventory_with_items(vec![("scroll_meridian_primer", 42)]);
        let before = inventory.clone();

        let _ = resolve_scroll_read_request(&inventory, &registry, 42);

        assert_eq!(
            inventory.containers[0].items.len(),
            before.containers[0].items.len(),
            "resolve_scroll_read_request 必须是只读操作，不得移除/修改物品实例"
        );
        let _ = &mut inventory; // 显式标注：即便签名允许 mut，本函数也不应触碰
    }

    // ── 边界 / 错误分支 ──────────────────────────────────────────────

    #[test]
    fn rejects_when_instance_not_in_inventory() {
        let registry = registry_with(vec![readable_scroll_template("scroll_meridian_primer")]);
        let inventory = empty_inventory();

        let result = resolve_scroll_read_request(&inventory, &registry, 999);

        assert_eq!(
            result,
            Err(ScrollReadRejectReason::InstanceNotInInventory),
            "空背包里查任意 instance_id 都应判定为 InstanceNotInInventory"
        );
    }

    #[test]
    fn rejects_forged_instance_id_not_belonging_to_player() {
        // 伪造 instance_id：该 id 不在这个玩家的背包（哪怕全局别处存在同 id 物品，
        // 本函数只看传入的这一份 inventory，天然覆盖"非本人物品"场景）。
        let registry = registry_with(vec![readable_scroll_template("scroll_meridian_primer")]);
        let inventory = inventory_with_items(vec![("scroll_meridian_primer", 42)]);

        let result = resolve_scroll_read_request(&inventory, &registry, 1337);

        assert_eq!(result, Err(ScrollReadRejectReason::InstanceNotInInventory));
    }

    #[test]
    fn rejects_unknown_template_id() {
        let registry = registry_with(vec![]); // registry 为空，template 未注册
        let inventory = inventory_with_items(vec![("ghost_template", 1)]);

        let result = resolve_scroll_read_request(&inventory, &registry, 1);

        assert_eq!(
            result,
            Err(ScrollReadRejectReason::UnknownTemplate {
                template_id: "ghost_template".to_string()
            })
        );
    }

    #[test]
    fn rejects_template_without_readable_scroll_spec() {
        // 普通消耗式招式残卷：有 technique_scroll_spec，但没有 readable_scroll_spec。
        let mut template = base_template("scroll_technique_sword_cleave", ItemCategory::Scroll);
        template.technique_scroll_spec = Some(crate::inventory::TechniqueScrollSpec {
            kind: "combat_technique".to_string(),
            skill_id: "sword.cleave".to_string(),
        });
        let registry = registry_with(vec![template]);
        let inventory = inventory_with_items(vec![("scroll_technique_sword_cleave", 7)]);

        let result = resolve_scroll_read_request(&inventory, &registry, 7);

        assert_eq!(
            result,
            Err(ScrollReadRejectReason::NoReadableScrollSpec {
                template_id: "scroll_technique_sword_cleave".to_string()
            }),
            "无 readable_scroll_spec 的物品（如消耗式招式残卷）必须被拒绝，不可误开阅读屏"
        );
    }

    #[test]
    fn repeated_requests_for_same_instance_resolve_identically() {
        // 重复请求（同一 instance_id 连续两次）：读取不消耗，两次都应成功且内容一致。
        let registry = registry_with(vec![readable_scroll_template("scroll_meridian_primer")]);
        let inventory = inventory_with_items(vec![("scroll_meridian_primer", 42)]);

        let first = resolve_scroll_read_request(&inventory, &registry, 42)
            .expect("first read should succeed");
        let second = resolve_scroll_read_request(&inventory, &registry, 42)
            .expect("second (repeated) read should also succeed — reading never consumes");

        assert_eq!(
            first, second,
            "重复请求同一 instance_id 必须解析出完全一致的 ScrollReadResolution"
        );
    }

    #[test]
    fn resolves_scroll_from_equipped_slot() {
        // instance 可能挂在 equipped 而非 containers（inventory_item_by_instance_borrow 三处都扫）。
        use crate::inventory::SlotContents;
        let registry = registry_with(vec![readable_scroll_template("scroll_meridian_primer")]);
        let mut inventory = empty_inventory();
        inventory.equipped.insert(
            "main_hand".to_string(),
            SlotContents {
                worn: vec![test_item(55, "scroll_meridian_primer")],
                held: None,
            },
        );

        let result = resolve_scroll_read_request(&inventory, &registry, 55);

        assert!(
            result.is_ok(),
            "挂在 equipped 槽的可阅读残卷也应能被解析到，得到 {result:?}"
        );
    }
}
