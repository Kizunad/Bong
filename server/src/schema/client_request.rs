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
    /// plan-HUD-v1 §7.3 / §11.3 切换防御姿态。`stance` 一个：
    /// "JIEMAI" / "TISHI" / "JUELING" / "NONE"（与 client `Stance.name()` 对齐）。
    /// server 校验 UnlockedStyles 后写入 DefenseStance Component。
    SwitchDefenseStance {
        v: u8,
        stance: String,
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
    fn rejects_unknown_type() {
        let json = r#"{"type":"nuke_world","v":1}"#;
        assert!(serde_json::from_str::<ClientRequestV1>(json).is_err());
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
}
