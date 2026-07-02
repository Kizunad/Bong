//! `InventoryMoveRejectedV1` S2C 回执发送 —— 库存操作拒绝原因下发。
//!
//! plan-inventory-hint-panel-v1 P0：把 `apply_inventory_move` / `validate_move_semantics`
//! 及境界门控（伪皮胸槽）拒绝的 [`crate::inventory::InventoryMoveRejectReason`] 结构化
//! 后下发触发操作的玩家（仅发给触发者，不广播）。
//!
//! 模板：`mineral_probe_emit.rs`（`ServerDataV1::new(ServerDataPayloadV1::MineralProbeResult(v1))`
//! 只发给触发者 entity 的范式）。本模块不走 event queue（handle_inventory_move 已持有
//! `entity` + `clients` query，直接调用即可，无需额外事件间接层）。

use valence::prelude::{Client, Entity, Query, Username};

use crate::inventory::InventoryMoveRejectReason;
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::server_data::{InventoryMoveRejectedV1, ServerDataPayloadV1, ServerDataV1};

/// `InventoryMoveRejectReason` → `InventoryMoveRejectedV1` 映射（纯函数，独立可测）。
///
/// 全 match、无 catch-all 的枚举内部访问器（`to_wire_tag` / `required_realm` / `slot` / `cap`）
/// 已在 `inventory::InventoryMoveRejectReason` 里保证穷举；本函数只是把它们组装成 wire struct。
pub fn inventory_move_rejected_v1_from_reason(
    reason: &InventoryMoveRejectReason,
) -> InventoryMoveRejectedV1 {
    InventoryMoveRejectedV1 {
        reason: reason.to_wire_tag().to_string(),
        required_realm: reason.required_realm().map(|s| s.to_string()),
        slot: reason.slot().map(|s| s.to_string()),
        cap: reason.cap(),
    }
}

/// 组装 `InventoryMoveRejectedV1` payload 并只发给触发者 `entity`（不广播）。
///
/// 找不到该 entity 对应的 `Client`（已断线 / 测试里未 spawn）时静默跳过——拒绝已经通过
/// `apply_inventory_move` 的 `Err` 分支阻止了状态变更，本函数只是补一条 UX 提示，
/// 找不到发送目标不应影响调用方（`resync_snapshot` 等）继续执行。
pub fn emit_inventory_move_rejected(
    entity: Entity,
    reason: &InventoryMoveRejectReason,
    clients: &mut Query<(&Username, &mut Client)>,
) {
    let v1 = inventory_move_rejected_v1_from_reason(reason);
    let payload = ServerDataV1::new(ServerDataPayloadV1::InventoryMoveRejected(v1));
    let payload_type = payload_type_label(payload.payload_type());
    let payload_bytes = match serialize_server_data_payload(&payload) {
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

    // ─────────────────────────────────────────────────────────────────
    // inventory_move_rejected_v1_from_reason：每个 InventoryMoveRejectReason
    // 变体一条专属 case（饱和覆盖 wire 字段映射）。
    // ─────────────────────────────────────────────────────────────────

    #[test]
    fn maps_realm_too_low_with_required_realm() {
        let v1 = inventory_move_rejected_v1_from_reason(&InventoryMoveRejectReason::RealmTooLow {
            required_realm: "Condense".to_string(),
        });
        assert_eq!(v1.reason, "realm_too_low");
        assert_eq!(v1.required_realm.as_deref(), Some("Condense"));
        assert!(v1.slot.is_none(), "realm_too_low 不带 slot/cap");
        assert!(v1.cap.is_none());
    }

    #[test]
    fn maps_worn_cap_full_with_slot_and_cap() {
        let v1 = inventory_move_rejected_v1_from_reason(&InventoryMoveRejectReason::WornCapFull {
            slot: "chest".to_string(),
            cap: 3,
        });
        assert_eq!(v1.reason, "worn_cap_full");
        assert_eq!(v1.slot.as_deref(), Some("chest"));
        assert_eq!(v1.cap, Some(3));
        assert!(v1.required_realm.is_none());
    }

    #[test]
    fn maps_armor_slot_mismatch_with_slot_no_cap() {
        let v1 =
            inventory_move_rejected_v1_from_reason(&InventoryMoveRejectReason::ArmorSlotMismatch {
                expected_slot: "chest".to_string(),
            });
        assert_eq!(v1.reason, "armor_slot_mismatch");
        assert_eq!(v1.slot.as_deref(), Some("chest"));
        assert!(v1.cap.is_none());
        assert!(v1.required_realm.is_none());
    }

    #[test]
    fn maps_pack_equip_slot_mismatch_with_slot() {
        let v1 = inventory_move_rejected_v1_from_reason(
            &InventoryMoveRejectReason::PackEquipSlotMismatch {
                expected_slot: "legs".to_string(),
            },
        );
        assert_eq!(v1.reason, "pack_equip_slot_mismatch");
        assert_eq!(v1.slot.as_deref(), Some("legs"));
    }

    #[test]
    fn maps_forbidden_in_hotbar_tag_no_extra_fields() {
        let v1 =
            inventory_move_rejected_v1_from_reason(&InventoryMoveRejectReason::ForbiddenInHotbar {
                category: crate::inventory::ItemCategory::Weapon,
            });
        assert_eq!(v1.reason, "forbidden_in_hotbar");
        assert!(v1.slot.is_none());
        assert!(v1.cap.is_none());
        assert!(v1.required_realm.is_none());
    }

    #[test]
    fn maps_pack_detached_tag_no_extra_fields() {
        // owner_instance_id 只用于 server 内部日志/精确断言（PackDetached matches! 测试），
        // 不下发到 wire（wire 只带 reason/required_realm/slot/cap 四个通用字段，见 plan §P0）。
        let v1 = inventory_move_rejected_v1_from_reason(&InventoryMoveRejectReason::PackDetached {
            owner_instance_id: 3001,
        });
        assert_eq!(v1.reason, "pack_detached");
        assert!(v1.slot.is_none());
    }

    #[test]
    fn maps_target_occupied_tag_no_extra_fields() {
        let v1 =
            inventory_move_rejected_v1_from_reason(&InventoryMoveRejectReason::TargetOccupied {
                instance_id: 42,
            });
        assert_eq!(v1.reason, "target_occupied");
    }

    #[test]
    fn maps_all_unit_variants_to_distinct_tags() {
        // 穷举所有无额外字段的变体 → tag 各不相同（防止 to_wire_tag 复制粘贴撞车）。
        use InventoryMoveRejectReason::*;
        let unit_variants = [
            FromLocationMismatch,
            InstanceNotFound,
            UnknownContainerId,
            TargetOutOfBounds,
            MultiOverlapNotSupported,
            HotbarIndexOutOfRange,
            HotbarOccupied,
            SwapFootprintMismatch,
            UnknownItemTemplate,
            WornStackNotTop,
            HeldWornMismatch,
            EquipCategoryMismatch,
            OffHandTypeMismatch,
            HandOccupied,
            TwoHandedLocksOther,
            ArmorDurabilityZero,
        ];
        let mut tags: Vec<String> = unit_variants
            .iter()
            .map(|r| inventory_move_rejected_v1_from_reason(r).reason)
            .collect();
        let unique_count = {
            tags.sort_unstable();
            tags.dedup();
            tags.len()
        };
        assert_eq!(
            unique_count,
            unit_variants.len(),
            "每个 unit variant 的 to_wire_tag 必须互不相同"
        );
    }

    #[test]
    fn emit_targets_only_triggering_entity_not_broadcast() {
        use valence::prelude::{bevy_ecs, App, Res, Resource};
        use valence::protocol::packets::play::CustomPayloadS2c;
        use valence::testing::{create_mock_client, MockClientHelper};

        use crate::network::agent_bridge::SERVER_DATA_CHANNEL;

        fn spawn_mock_client(app: &mut App, name: &str) -> (Entity, MockClientHelper) {
            let (bundle, helper) = create_mock_client(name);
            let entity = app.world_mut().spawn(bundle).id();
            (entity, helper)
        }

        #[derive(Resource)]
        struct PendingRejectEmit(Entity, InventoryMoveRejectReason);

        fn drive_emit_once(
            pending: Res<PendingRejectEmit>,
            mut clients: Query<(&Username, &mut Client)>,
        ) {
            emit_inventory_move_rejected(pending.0, &pending.1, &mut clients);
        }

        let mut app = App::new();
        let (player, mut player_helper) = spawn_mock_client(&mut app, "Kiz");
        let (_spectator, mut spectator_helper) = spawn_mock_client(&mut app, "Spectator");

        app.insert_resource(PendingRejectEmit(
            player,
            InventoryMoveRejectReason::RealmTooLow {
                required_realm: "Condense".to_string(),
            },
        ));

        let mut schedule = bevy_ecs::schedule::Schedule::default();
        schedule.add_systems(drive_emit_once);
        schedule.run(app.world_mut());

        for mut client in app
            .world_mut()
            .query::<&mut Client>()
            .iter_mut(app.world_mut())
        {
            let _ = client.flush_packets();
        }

        let collect = |helper: &mut MockClientHelper| -> usize {
            let mut count = 0;
            for frame in helper.collect_received().0 {
                let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                    continue;
                };
                if packet.channel.as_str() == SERVER_DATA_CHANNEL {
                    count += 1;
                }
            }
            count
        };
        assert_eq!(collect(&mut player_helper), 1, "触发者应收到 1 包");
        assert_eq!(collect(&mut spectator_helper), 0, "旁观者不应收到任何包");
    }
}
