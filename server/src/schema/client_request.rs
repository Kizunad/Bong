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
#[serde(deny_unknown_fields, tag = "type", rename_all = "snake_case")]
pub enum ClientRequestV1 {
    SetMeridianTarget {
        v: u8,
        meridian: MeridianId,
    },
    BreakthroughRequest {
        v: u8,
    },
    StartDuXu {
        v: u8,
    },
    AbortTribulation {
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
    /// plan-mineral-v1 §3 — 凝脉+ 右键矿块，server 反查 MineralOreIndex。
    MineralProbe {
        v: u8,
        x: i32,
        y: i32,
        z: i32,
    },
    ApplyPill {
        v: u8,
        instance_id: u64,
        target: ApplyPillTargetV1,
    },
    DuoSheRequest {
        v: u8,
        target_id: String,
    },
    UseLifeCore {
        v: u8,
        instance_id: u64,
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
        #[serde(deserialize_with = "deserialize_slot_index")]
        slot: u8,
    },
    /// plan-HUD-v1 §10 / §11.3 InspectScreen 内拖拽配置 F1-F9 槽。
    /// `item_id` 为 None 表示清空槽位。
    QuickSlotBind {
        v: u8,
        #[serde(deserialize_with = "deserialize_slot_index")]
        slot: u8,
        item_id: Option<String>,
    },
    /// plan-hotbar-modify-v1 §3.2：触发 1-9 技能栏槽位。
    SkillBarCast {
        v: u8,
        #[serde(deserialize_with = "deserialize_slot_index")]
        slot: u8,
        #[serde(skip_serializing_if = "Option::is_none")]
        target: Option<String>,
    },
    /// plan-hotbar-modify-v1 §3.2：配置 1-9 技能栏；None 表示清空槽位。
    SkillBarBind {
        v: u8,
        #[serde(deserialize_with = "deserialize_slot_index")]
        slot: u8,
        binding: Option<SkillBarBindingV1>,
    },
    CombatReincarnate {
        v: u8,
    },
    CombatTerminate {
        v: u8,
    },
    CombatCreateNewCharacter {
        v: u8,
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
    // ─── 炼器（武器）（plan-forge-v1 §4） ────────────────────────
    /// plan §1.3.1 — 起炉请求。client 拖齐坯料 + 选图谱后发起。
    ForgeStartSession {
        v: u8,
        station_id: String,
        blueprint_id: String,
        materials: Vec<(String, u32)>,
    },
    /// plan §1.3.2 — 淬炼击键上报。
    ForgeTemperingHit {
        v: u8,
        session_id: u64,
        beat: String,
        ticks_remaining: u32,
    },
    /// plan §1.3.3 — 铭文残卷投入。
    ForgeInscriptionScroll {
        v: u8,
        session_id: u64,
        inscription_id: String,
    },
    /// plan §1.3.4 — 开光真元注入。
    ForgeConsecrationInject {
        v: u8,
        session_id: u64,
        qi_amount: f64,
    },
    /// plan §1.3 — 步骤推进（当前步骤完成，进下一步）。
    ForgeStepAdvance {
        v: u8,
        session_id: u64,
    },
    /// plan §1.4 — 图谱书翻页。
    ForgeBlueprintTurnPage {
        v: u8,
        delta: i32,
    },
    /// plan §1.4 — 学习图谱（客户端拖残卷到图谱区）。
    ForgeLearnBlueprint {
        v: u8,
        blueprint_id: String,
    },
    /// plan §1.2 — 玩家手持砧类物品，客户端拦截右键放砧方块。
    ForgeStationPlace {
        v: u8,
        x: i32,
        y: i32,
        z: i32,
        item_instance_id: u64,
        station_tier: u8,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "snake_case")]
pub enum SkillBarBindingV1 {
    Item { template_id: String },
    Skill { skill_id: String },
}

fn deserialize_slot_index<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let slot = u8::deserialize(deserializer)?;
    if slot < 9 {
        Ok(slot)
    } else {
        Err(serde::de::Error::custom("slot must be between 0 and 8"))
    }
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
    fn use_quick_slot_roundtrip() {
        let json = r#"{"type":"use_quick_slot","v":1,"slot":3}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        assert!(matches!(
            req,
            ClientRequestV1::UseQuickSlot { v: 1, slot: 3 }
        ));
    }

    #[test]
    fn quick_slot_bind_roundtrip_and_clear() {
        let bind_json = r#"{"type":"quick_slot_bind","v":1,"slot":1,"item_id":"kai_mai_pill"}"#;
        let req: ClientRequestV1 = serde_json::from_str(bind_json).unwrap();
        assert!(matches!(
            req,
            ClientRequestV1::QuickSlotBind {
                v: 1,
                slot: 1,
                item_id: Some(ref item_id),
            } if item_id == "kai_mai_pill"
        ));

        let clear_json = r#"{"type":"quick_slot_bind","v":1,"slot":1,"item_id":null}"#;
        let req: ClientRequestV1 = serde_json::from_str(clear_json).unwrap();
        assert!(matches!(
            req,
            ClientRequestV1::QuickSlotBind {
                v: 1,
                slot: 1,
                item_id: None,
            }
        ));
    }

    #[test]
    fn skill_bar_cast_roundtrip_with_optional_target() {
        let json = r#"{"type":"skill_bar_cast","v":1,"slot":0,"target":"entity:42"}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::SkillBarCast { v, slot, target } => {
                assert_eq!(v, 1);
                assert_eq!(slot, 0);
                assert_eq!(target.as_deref(), Some("entity:42"));
            }
            other => panic!("expected SkillBarCast, got {other:?}"),
        }

        let no_target = ClientRequestV1::SkillBarCast {
            v: 1,
            slot: 2,
            target: None,
        };
        let serialized = serde_json::to_string(&no_target).unwrap();
        assert!(
            !serialized.contains("target"),
            "target None should be omitted: {serialized}"
        );
    }

    #[test]
    fn skill_bar_bind_roundtrip_for_null_item_and_skill() {
        let clear_json = r#"{"type":"skill_bar_bind","v":1,"slot":0,"binding":null}"#;
        let req: ClientRequestV1 = serde_json::from_str(clear_json).unwrap();
        assert!(matches!(
            req,
            ClientRequestV1::SkillBarBind {
                v: 1,
                slot: 0,
                binding: None,
            }
        ));

        let item_json = r#"{"type":"skill_bar_bind","v":1,"slot":1,"binding":{"kind":"item","template_id":"iron_sword"}}"#;
        let req: ClientRequestV1 = serde_json::from_str(item_json).unwrap();
        assert!(matches!(
            req,
            ClientRequestV1::SkillBarBind {
                v: 1,
                slot: 1,
                binding: Some(SkillBarBindingV1::Item { ref template_id }),
            } if template_id == "iron_sword"
        ));

        let skill_json = r#"{"type":"skill_bar_bind","v":1,"slot":2,"binding":{"kind":"skill","skill_id":"burst_meridian.beng_quan"}}"#;
        let req: ClientRequestV1 = serde_json::from_str(skill_json).unwrap();
        assert!(matches!(
            req,
            ClientRequestV1::SkillBarBind {
                v: 1,
                slot: 2,
                binding: Some(SkillBarBindingV1::Skill { ref skill_id }),
            } if skill_id == "burst_meridian.beng_quan"
        ));
    }

    #[test]
    fn skill_bar_binding_rejects_unknown_kind_and_extra_fields() {
        let wrong_kind = r#"{"type":"skill_bar_bind","v":1,"slot":0,"binding":{"kind":"unknown","skill_id":"x"}}"#;
        assert!(serde_json::from_str::<ClientRequestV1>(wrong_kind).is_err());

        let extra_field = r#"{"type":"skill_bar_cast","v":1,"slot":0,"extra":1}"#;
        assert!(serde_json::from_str::<ClientRequestV1>(extra_field).is_err());
    }

    #[test]
    fn hotbar_slot_indices_reject_out_of_range_values() {
        for json in [
            r#"{"type":"use_quick_slot","v":1,"slot":9}"#,
            r#"{"type":"quick_slot_bind","v":1,"slot":9,"item_id":null}"#,
            r#"{"type":"skill_bar_cast","v":1,"slot":9}"#,
            r#"{"type":"skill_bar_bind","v":1,"slot":9,"binding":null}"#,
        ] {
            let error = serde_json::from_str::<ClientRequestV1>(json)
                .expect_err("slot 9 should be rejected by schema");
            assert!(error.to_string().contains("slot must be between 0 and 8"));
        }
    }

    #[test]
    fn duo_she_request_roundtrip() {
        let json = r#"{"type":"duo_she_request","v":1,"target_id":"npc_12v0"}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::DuoSheRequest { v, target_id } => {
                assert_eq!(v, 1);
                assert_eq!(target_id, "npc_12v0");
            }
            other => panic!("expected DuoSheRequest, got {other:?}"),
        }
    }

    #[test]
    fn use_life_core_roundtrip() {
        let json = r#"{"type":"use_life_core","v":1,"instance_id":4242}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::UseLifeCore { v, instance_id } => {
                assert_eq!(v, 1);
                assert_eq!(instance_id, 4242);
            }
            other => panic!("expected UseLifeCore, got {other:?}"),
        }
    }

    #[test]
    fn combat_reincarnate_roundtrip() {
        let json = r#"{"type":"combat_reincarnate","v":1}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        assert!(matches!(req, ClientRequestV1::CombatReincarnate { v: 1 }));
    }

    #[test]
    fn combat_terminate_roundtrip() {
        let json = r#"{"type":"combat_terminate","v":1}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        assert!(matches!(req, ClientRequestV1::CombatTerminate { v: 1 }));
    }

    #[test]
    fn combat_create_new_character_roundtrip() {
        let json = r#"{"type":"combat_create_new_character","v":1}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        assert!(matches!(
            req,
            ClientRequestV1::CombatCreateNewCharacter { v: 1 }
        ));
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
    fn mineral_probe_roundtrip() {
        let json = r#"{"type":"mineral_probe","v":1,"x":8,"y":32,"z":8}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::MineralProbe { v, x, y, z } => {
                assert_eq!(v, 1);
                assert_eq!((x, y, z), (8, 32, 8));
            }
            other => panic!("expected MineralProbe, got {other:?}"),
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
    fn forge_station_place_roundtrip() {
        let json = r#"{"type":"forge_station_place","v":1,"x":-12,"y":64,"z":38,"item_instance_id":4242,"station_tier":2}"#;
        let req: ClientRequestV1 = serde_json::from_str(json).unwrap();
        match req {
            ClientRequestV1::ForgeStationPlace {
                v,
                x,
                y,
                z,
                item_instance_id,
                station_tier,
            } => {
                assert_eq!(v, 1);
                assert_eq!((x, y, z), (-12, 64, 38));
                assert_eq!(item_instance_id, 4242);
                assert_eq!(station_tier, 2);
            }
            other => panic!("expected ForgeStationPlace, got {other:?}"),
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
