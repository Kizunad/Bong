//! 客户端 → 服务端请求 schema（plan-cultivation-v1 §P1 剩余）。
//!
//! 与 TypeScript `agent/packages/schema/src/client-request.ts` 1:1。
//! 由 Fabric 客户端通过 Minecraft CustomPayload 发送，服务端反序列化为对应
//! Bevy Event（MeridianTarget Component 更新 / BreakthroughRequest / ForgeRequest）。

use serde::{Deserialize, Serialize};

use super::alchemy::AlchemyInterventionV1;
use super::inventory::InventoryLocationV1;
use crate::cultivation::components::MeridianId;
use crate::cultivation::forging::ForgeAxis;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApplyPillTargetV1 {
    #[serde(rename = "self")]
    SelfTarget,
    Meridian {
        meridian_id: MeridianId,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientRequestV1 {
    SetMeridianTarget {
        v: u8,
        meridian: MeridianId,
    },
    BreakthroughRequest {
        v: u8,
    },
    ForgeRequest {
        v: u8,
        meridian: MeridianId,
        axis: ForgeAxis,
    },
    /// 顿悟邀约回执：玩家选择 / 拒绝 / 超时。
    /// `choice_idx = Some(n)` → 选中第 n 个候选；`None` → 拒绝或超时（服务端等价处理）。
    InsightDecision {
        v: u8,
        trigger_id: String,
        choice_idx: Option<u32>,
    },
    BotanyHarvestRequest {
        v: u8,
        session_id: String,
        mode: crate::schema::botany::BotanyHarvestModeV1,
    },
    // ─── 炼丹（plan-alchemy-v1 §4） ─────────────────────────
    AlchemyOpenFurnace {
        v: u8,
        furnace_id: String,
    },
    AlchemyFeedSlot {
        v: u8,
        slot_idx: u8,
        material: String,
        count: u32,
    },
    AlchemyTakeBack {
        v: u8,
        slot_idx: u8,
    },
    AlchemyIgnite {
        v: u8,
        recipe_id: String,
    },
    AlchemyIntervention {
        v: u8,
        intervention: AlchemyInterventionV1,
    },
    AlchemyTurnPage {
        v: u8,
        delta: i32,
    },
    AlchemyLearnRecipe {
        v: u8,
        recipe_id: String,
    },
    AlchemyTakePill {
        v: u8,
        pill_item_id: String,
    },
    /// plan-alchemy-v1 §1.2 — 玩家手持炉类物品，客户端拦截右键地面并发此请求。
    /// server 校验 `item_instance_id` 为合法炉类物品 → 消耗一个 → 在 `pos`
    /// spawn `AlchemyFurnace` ECS entity，并把对应方块刷成 `FURNACE`。
    AlchemyFurnacePlace {
        v: u8,
        x: i32,
        y: i32,
        z: i32,
        item_instance_id: u64,
    },
    LearnSkillScroll {
        v: u8,
        instance_id: u64,
    },
    /// 客户端拖拽完成后通知 server 把 instance_id 从 from 移动到 to。
    /// server 校验后改 PlayerInventory，回推 inventory_event::moved。
    InventoryMoveIntent {
        v: u8,
        instance_id: u64,
        from: InventoryLocationV1,
        to: InventoryLocationV1,
    },
    InventoryDiscardItem {
        v: u8,
        instance_id: u64,
        from: InventoryLocationV1,
    },
    DropWeaponIntent {
        v: u8,
        instance_id: u64,
        from: InventoryLocationV1,
    },
    RepairWeaponIntent {
        v: u8,
        instance_id: u64,
        station_pos: [i32; 3],
    },
    PickupDroppedItem {
        v: u8,
        instance_id: u64,
    },
    ApplyPill {
        v: u8,
        instance_id: u64,
        target: ApplyPillTargetV1,
    },
    /// plan-HUD-v1 §3.2 截脉弹反反应键。无 payload。
    /// server 翻译为 `DefenseIntent` Bevy event，立即开 200ms `incoming_window`，
    /// 并回推 `defense_window` payload 让 client 渲染红环。
    Jiemai {
        v: u8,
    },
    /// plan-HUD-v1 §4 / §11.3 触发 F1-F9 快捷使用槽。
    /// server 校验后插入 `Casting` Component，回推 `cast_sync(Casting)`；
    /// `tick_casts` 系统在 duration 到期时移除 Component 并推 `cast_sync(Complete)`。
    UseQuickSlot {
        v: u8,
        slot: u8,
    },
    /// plan-HUD-v1 §10 / §11.3 InspectScreen 内拖拽配置 F1-F9 槽。
    /// `item_id` 为 None 表示清空槽位。
    QuickSlotBind {
        v: u8,
        slot: u8,
        item_id: Option<String>,
    },
    StartExtractRequest {
        v: u8,
        portal_entity_id: u64,
    },
    CancelExtractRequest {
        v: u8,
    },
    // ─── 灵田（plan-lingtian-v1 §1.2 / §1.4 / §1.5 / §1.6 / §1.7） ────
    /// plan §1.2.2 — 起开垦 session。terrain / environment 由 server 从
    /// chunk_layer 读 BlockKind 自动派生（避免客户端伪造）。
    LingtianStartTill {
        v: u8,
        x: i32,
        y: i32,
        z: i32,
        hoe_instance_id: u64,
        /// "manual" / "auto"（auto 需 herbalism Lv.3+，server 暂不校验）。
        mode: String,
    },
    /// plan §1.6 — 起翻新 session。
    LingtianStartRenew {
        v: u8,
        x: i32,
        y: i32,
        z: i32,
        hoe_instance_id: u64,
    },
    /// plan §1.2.3 — 起种植 session（背包内须有该 plant 的种子）。
    LingtianStartPlanting {
        v: u8,
        x: i32,
        y: i32,
        z: i32,
        plant_id: String,
    },
    /// plan §1.5 — 起收获 session（plot.crop 须 ripe）。
    LingtianStartHarvest {
        v: u8,
        x: i32,
        y: i32,
        z: i32,
        mode: String,
    },
    /// plan §1.4 — 起补灵 session。
    LingtianStartReplenish {
        v: u8,
        x: i32,
        y: i32,
        z: i32,
        /// "zone" / "bone_coin" / "beast_core" / "ling_shui"。
        source: String,
    },
    /// plan §1.7 — 起偷灵 session。
    LingtianStartDrainQi {
        v: u8,
        x: i32,
        y: i32,
        z: i32,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_meridian_target_roundtrip() {
        let json = r#"{"type":"set_meridian_target","v":1,"meridian":"Lung"}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::SetMeridianTarget { v, meridian } => {
                assert_eq!(v, 1);
                assert_eq!(meridian, MeridianId::Lung);
            }
            other => panic!("expected SetMeridianTarget, got {other:?}"),
        }
    }

    #[test]
    fn breakthrough_request_roundtrip() {
        let json = r#"{"type":"breakthrough_request","v":1}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        assert!(matches!(req, ClientRequestV1::BreakthroughRequest { v: 1 }));
    }

    #[test]
    fn forge_request_roundtrip() {
        let json = r#"{"type":"forge_request","v":1,"meridian":"Ren","axis":"Rate"}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::ForgeRequest { meridian, axis, .. } => {
                assert_eq!(meridian, MeridianId::Ren);
                assert!(matches!(axis, ForgeAxis::Rate));
            }
            other => panic!("expected ForgeRequest, got {other:?}"),
        }
    }

    #[test]
    fn forge_request_capacity_axis_roundtrip() {
        let v = ClientRequestV1::ForgeRequest {
            v: 1,
            meridian: MeridianId::Du,
            axis: ForgeAxis::Capacity,
        };
        let s = serde_json::to_string(&v).unwrap();
        assert!(s.contains("\"axis\":\"Capacity\""));
        let back: ClientRequestV1 = serde_json::from_str(&s).unwrap();
        assert!(matches!(
            back,
            ClientRequestV1::ForgeRequest {
                axis: ForgeAxis::Capacity,
                ..
            }
        ));
    }

    #[test]
    fn insight_decision_chosen_roundtrip() {
        let json =
            r#"{"type":"insight_decision","v":1,"trigger_id":"awaken_first","choice_idx":2}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::InsightDecision {
                v,
                trigger_id,
                choice_idx,
            } => {
                assert_eq!(v, 1);
                assert_eq!(trigger_id, "awaken_first");
                assert_eq!(choice_idx, Some(2));
            }
            other => panic!("expected InsightDecision, got {other:?}"),
        }
    }

    #[test]
    fn insight_decision_declined_roundtrip() {
        let json =
            r#"{"type":"insight_decision","v":1,"trigger_id":"awaken_first","choice_idx":null}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        assert!(matches!(
            req,
            ClientRequestV1::InsightDecision {
                choice_idx: None,
                ..
            }
        ));
    }

    #[test]
    fn apply_pill_self_roundtrip() {
        let json = r#"{"type":"apply_pill","v":1,"instance_id":1001,"target":{"kind":"self"}}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::ApplyPill {
                v,
                instance_id,
                target,
            } => {
                assert_eq!(v, 1);
                assert_eq!(instance_id, 1001);
                assert_eq!(target, ApplyPillTargetV1::SelfTarget);
            }
            other => panic!("expected ApplyPill, got {other:?}"),
        }
    }

    #[test]
    fn apply_pill_meridian_roundtrip() {
        let json = r#"{"type":"apply_pill","v":1,"instance_id":2002,"target":{"kind":"meridian","meridian_id":"Ren"}}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::ApplyPill {
                instance_id,
                target,
                ..
            } => {
                assert_eq!(instance_id, 2002);
                assert_eq!(
                    target,
                    ApplyPillTargetV1::Meridian {
                        meridian_id: MeridianId::Ren,
                    }
                );
            }
            other => panic!("expected ApplyPill, got {other:?}"),
        }
    }

    #[test]
    fn pickup_dropped_item_roundtrip() {
        let json = r#"{"type":"pickup_dropped_item","v":1,"instance_id":3003}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::PickupDroppedItem { v, instance_id } => {
                assert_eq!(v, 1);
                assert_eq!(instance_id, 3003);
            }
            other => panic!("expected PickupDroppedItem, got {other:?}"),
        }
    }

    #[test]
    fn inventory_discard_item_roundtrip() {
        let json = r#"{"type":"inventory_discard_item","v":1,"instance_id":1001,"from":{"kind":"container","container_id":"main_pack","row":0,"col":0}}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::InventoryDiscardItem {
                v,
                instance_id,
                from,
            } => {
                assert_eq!(v, 1);
                assert_eq!(instance_id, 1001);
                assert_eq!(
                    from,
                    InventoryLocationV1::Container {
                        container_id: crate::schema::inventory::ContainerIdV1::MainPack,
                        row: 0,
                        col: 0,
                    }
                );
            }
            other => panic!("expected InventoryDiscardItem, got {other:?}"),
        }
    }

    #[test]
    fn drop_weapon_intent_roundtrip() {
        let json = r#"{"type":"drop_weapon_intent","v":1,"instance_id":1001,"from":{"kind":"equip","slot":"main_hand"}}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::DropWeaponIntent {
                v,
                instance_id,
                from,
            } => {
                assert_eq!(v, 1);
                assert_eq!(instance_id, 1001);
                assert_eq!(
                    from,
                    InventoryLocationV1::Equip {
                        slot: crate::schema::inventory::EquipSlotV1::MainHand,
                    }
                );
            }
            other => panic!("expected DropWeaponIntent, got {other:?}"),
        }
    }

    #[test]
    fn repair_weapon_intent_roundtrip() {
        let json =
            r#"{"type":"repair_weapon_intent","v":1,"instance_id":4242,"station_pos":[1,64,2]}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::RepairWeaponIntent {
                v,
                instance_id,
                station_pos,
            } => {
                assert_eq!(v, 1);
                assert_eq!(instance_id, 4242);
                assert_eq!(station_pos, [1, 64, 2]);
            }
            other => panic!("expected RepairWeaponIntent, got {other:?}"),
        }
    }

    #[test]
    fn rejects_unknown_type() {
        let json = r#"{"type":"nuke_world","v":1}"#;
        assert!(serde_json::from_str::<ClientRequestV1>(json).is_err());
    }

    #[test]
    fn botany_harvest_request_roundtrip() {
        let json = r#"{"type":"botany_harvest_request","v":1,"session_id":"session-botany-01","mode":"manual"}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::BotanyHarvestRequest {
                v,
                session_id,
                mode,
            } => {
                assert_eq!(v, 1);
                assert_eq!(session_id, "session-botany-01");
                assert!(matches!(
                    mode,
                    crate::schema::botany::BotanyHarvestModeV1::Manual
                ));
            }
            other => panic!("expected BotanyHarvestRequest, got {other:?}"),
        }
    }

    #[test]
    fn alchemy_open_furnace_roundtrip() {
        let json = r#"{"type":"alchemy_open_furnace","v":1,"furnace_id":"block_-12_64_38"}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::AlchemyOpenFurnace { v, furnace_id } => {
                assert_eq!(v, 1);
                assert_eq!(furnace_id, "block_-12_64_38");
            }
            other => panic!("expected AlchemyOpenFurnace, got {other:?}"),
        }
    }

    #[test]
    fn alchemy_feed_slot_roundtrip() {
        let json =
            r#"{"type":"alchemy_feed_slot","v":1,"slot_idx":0,"material":"kai_mai_cao","count":3}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::AlchemyFeedSlot {
                slot_idx,
                material,
                count,
                ..
            } => {
                assert_eq!(slot_idx, 0);
                assert_eq!(material, "kai_mai_cao");
                assert_eq!(count, 3);
            }
            other => panic!("expected AlchemyFeedSlot, got {other:?}"),
        }
    }

    #[test]
    fn alchemy_intervention_inject_qi_roundtrip() {
        let json =
            r#"{"type":"alchemy_intervention","v":1,"intervention":{"kind":"inject_qi","qi":1.0}}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::AlchemyIntervention { intervention, .. } => match intervention {
                super::AlchemyInterventionV1::InjectQi { qi } => assert!((qi - 1.0).abs() < 1e-9),
                other => panic!("expected InjectQi, got {other:?}"),
            },
            other => panic!("expected AlchemyIntervention, got {other:?}"),
        }
    }

    #[test]
    fn alchemy_turn_page_roundtrip() {
        let json = r#"{"type":"alchemy_turn_page","v":1,"delta":-1}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::AlchemyTurnPage { delta, .. } => assert_eq!(delta, -1),
            other => panic!("expected AlchemyTurnPage, got {other:?}"),
        }
    }

    #[test]
    fn alchemy_furnace_place_roundtrip() {
        let json = r#"{"type":"alchemy_furnace_place","v":1,"x":-12,"y":64,"z":38,"item_instance_id":4242}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::AlchemyFurnacePlace {
                v,
                x,
                y,
                z,
                item_instance_id,
            } => {
                assert_eq!(v, 1);
                assert_eq!((x, y, z), (-12, 64, 38));
                assert_eq!(item_instance_id, 4242);
            }
            other => panic!("expected AlchemyFurnacePlace, got {other:?}"),
        }
    }

    #[test]
    fn alchemy_ignite_roundtrip() {
        let json = r#"{"type":"alchemy_ignite","v":1,"recipe_id":"kai_mai_pill_v0"}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::AlchemyIgnite { recipe_id, .. } => {
                assert_eq!(recipe_id, "kai_mai_pill_v0");
            }
            other => panic!("expected AlchemyIgnite, got {other:?}"),
        }
    }

    #[test]
    fn extract_requests_roundtrip() {
        let start = r#"{"type":"start_extract_request","v":1,"portal_entity_id":42}"#;
        let req: ClientRequestV1 = serde_json::from_str(start).unwrap();
        match req {
            ClientRequestV1::StartExtractRequest {
                v,
                portal_entity_id,
            } => {
                assert_eq!(v, 1);
                assert_eq!(portal_entity_id, 42);
            }
            other => panic!("expected StartExtractRequest, got {other:?}"),
        }

        let cancel = r#"{"type":"cancel_extract_request","v":1}"#;
        let req: ClientRequestV1 = serde_json::from_str(cancel).unwrap();
        assert!(matches!(
            req,
            ClientRequestV1::CancelExtractRequest { v: 1 }
        ));
    }
}
