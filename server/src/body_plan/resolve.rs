//! plan-race-system-v1 P0 — 唯一解析入口 `resolve_body_plan`。
//!
//! 所有需要"这个实体现在是什么身体构型"答案的消费点（命中几何 / 伤残后果 / 经脉 /
//! 功法门 / 伤口面板 / 装备槽）都必须走本函数，禁止绕过直查 `BodyPlanRegistry`——
//! plan §P0 决议原文："所有消费点只准走此入口并逐点标注 purpose"。P0a 阶段尚无实际
//! 生产消费点接入调用（`combat::raycast` 等消费点改造是后续阶段的工作），本文件只锁定
//! 入口签名 + 优先级语义 + 全解析矩阵测试。
//!
//! 优先级（plan §P0 原文）：
//! 1. 玩家（无 `BeastKind` 组件但携带 `Cultivation`）走 `Cultivation.race`——未知
//!    `RaceId` 是拒载入错误态，**不**静默兜底 humanoid 白得权限。
//! 2. NPC / fauna（携带 `BeastKind` 组件，即使意外同时挂着 `Cultivation` 也优先判定
//!    为兽类——一只带着残留 `Cultivation` 组件的蜘蛛仍必须按蜘蛛的命中判定,不能被误判
//!    成玩家）走 `BeastKind → RaceId` 派生（`RaceRegistry::race_id_for_beast_kind`）。
//! 3. 其余不携带任何身份标记的可受击实体，兜底 humanoid。
//!
//! `BodyPlanPurpose::{Intrinsic, Form}` 区分本体 / 当前形态两套语义；P4 `MorphState`
//! 落地前二者恒等（"未易形时 Form ≡ Intrinsic"），本函数当前对两个 purpose 返回相同
//! 结果——签名已经就位，P4 只需在 Form 分支插入 MorphState 查询即可，不用改调用点。

use valence::prelude::Entity;

use crate::cultivation::components::Cultivation;
use crate::dandao::mutation::BodySlot;
use crate::fauna::components::BeastKind;

use super::race_registry::RaceRegistry;
use super::registry::BodyPlanRegistry;
use super::types::{BodyPartId, BodyPlan, RaceId};

/// `resolve_body_plan` 的语义参数——P0 无实际差异（见模块文档），签名先行锁定供 P4 使用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyPlanPurpose {
    /// 本体：命中几何 / 伤残后果 / 经脉 / 功法门 / 伤口面板。
    Intrinsic,
    /// 当前形态：装备槽集合 / 穿戴 RaceGate / coverage 折算起点。
    Form,
}

/// 调用方预取的组件快照——解耦 `resolve_body_plan` 与具体 Bevy `Query` 类型，方便
/// 单元测试直接构造，同时不妨碍生产 system 用真实 `Query` 结果组装本结构体。
#[derive(Debug, Clone, Copy, Default)]
pub struct BodyPlanResolveInputs<'a> {
    pub cultivation: Option<&'a Cultivation>,
    pub beast_kind: Option<&'a BeastKind>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveBodyPlanError {
    /// 玩家身份携带的 `Cultivation.race` 在 `RaceRegistry` 中找不到对应条目——
    /// 拒绝解析（不静默兜底 humanoid），调用方应把该实体的相关操作判定为失败态。
    UnknownPlayerRace(RaceId),
}

impl std::fmt::Display for ResolveBodyPlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolveBodyPlanError::UnknownPlayerRace(race_id) => {
                write!(
                    f,
                    "unknown player race id {race_id} — refusing to resolve a body plan"
                )
            }
        }
    }
}

impl std::error::Error for ResolveBodyPlanError {}

pub fn resolve_body_plan<'a>(
    entity: Entity,
    _purpose: BodyPlanPurpose,
    inputs: BodyPlanResolveInputs<'_>,
    body_plans: &'a BodyPlanRegistry,
    races: &RaceRegistry,
) -> Result<&'a BodyPlan, ResolveBodyPlanError> {
    // Tier 1：BeastKind 组件在场即视为 NPC/fauna，优先于 Cultivation（防止「带着残留
    // Cultivation 组件的兽类」被误判为玩家身份）。
    if let Some(kind) = inputs.beast_kind {
        let race_id = races.race_id_for_beast_kind(*kind);
        if let Some(race_entry) = races.get(&race_id) {
            if let Some(plan) = body_plans.get(&race_entry.body_plan_id) {
                return Ok(plan);
            }
        }
        // race_id_for_beast_kind 返回的 race_id 理论上必定已在 RaceRegistry 登记且
        // body_plan_id 必定存在（RaceRegistry::load_file 的跨 registry 校验保证），
        // 这个分支只在校验被绕过（如测试直接手搓不一致的 registry）时触发；生产环境
        // 绝不应该让一只野怪的攻击结算直接 panic，防御性兜底到 humanoid。
        tracing::error!(
            "[bong][body_plan] entity {entity:?} beast_kind resolved to race {race_id} but its \
             body_plan_id is missing from BodyPlanRegistry — falling back to humanoid_default()"
        );
        return Ok(body_plans.humanoid_default());
    }

    // Tier 2：玩家身份——Cultivation.race 是权威真源，未知 race 拒绝解析。
    if let Some(cultivation) = inputs.cultivation {
        return match races.get(&cultivation.race) {
            Some(race_entry) => match body_plans.get(&race_entry.body_plan_id) {
                Some(plan) => Ok(plan),
                None => Err(ResolveBodyPlanError::UnknownPlayerRace(
                    cultivation.race.clone(),
                )),
            },
            None => Err(ResolveBodyPlanError::UnknownPlayerRace(
                cultivation.race.clone(),
            )),
        };
    }

    // Tier 3：既非兽类也非玩家身份（无 Cultivation/BeastKind 组件）——兜底 humanoid。
    Ok(body_plans.humanoid_default())
}

/// dandao `BodySlot`（变异挂载部位）→ 本 plan 部位 id 查询 API。非 humanoid /
/// 未配置映射的 plan 返回 `None`（不 panic，见 `types::BodyPlan::mutation_slot_mapping`
/// 文档——空映射对非人形构型是合法状态）。
pub fn body_part_for_mutation_slot(plan: &BodyPlan, slot: BodySlot) -> Option<&BodyPartId> {
    plan.mutation_slot_mapping.get(&slot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body_plan::race_registry::{RaceEntry, HUMAN_RACE_ID};
    use crate::body_plan::types::{
        BodyPartDef, HeightBand, HeightBandAssignment, HitGeometry, PartConsequence,
        StandingAabbSpec,
    };
    use std::collections::HashMap;
    use valence::prelude::World;

    fn humanoid_plan() -> BodyPlan {
        BodyPlan {
            id: "humanoid".into(),
            display_name: "人形".to_string(),
            is_humanoid: true,
            parts: vec![BodyPartDef {
                id: "head".into(),
                damage_mul: 2.0,
                contam_mul: 1.5,
                bleed_mul: 1.5,
                consequence: PartConsequence::Sensory,
            }],
            hit_geometry: HitGeometry::HeightBands {
                aabb: StandingAabbSpec {
                    half_width: 0.3,
                    height: 1.8,
                },
                bands: vec![HeightBand {
                    min_rel_y: -1.0,
                    assignment: HeightBandAssignment::Single {
                        part: "head".into(),
                    },
                }],
                lateral_threshold: 0.19,
            },
            equip_slots: vec![],
            meridian_profile: None,
            mutation_slot_mapping: {
                let mut map = HashMap::new();
                map.insert(BodySlot::Head, BodyPartId::new("head"));
                map
            },
        }
    }

    fn registries() -> (BodyPlanRegistry, RaceRegistry) {
        let body_plans =
            BodyPlanRegistry::from_plans(vec![humanoid_plan()]).expect("humanoid plan valid");
        let races = RaceRegistry::from_parts_for_test(
            vec![
                RaceEntry {
                    id: RaceId::new(HUMAN_RACE_ID),
                    display_name: "人族".to_string(),
                    body_plan_id: "humanoid".into(),
                    beast_kinds: vec![],
                },
                RaceEntry {
                    id: RaceId::new("beast_common"),
                    display_name: "凡兽通用".to_string(),
                    body_plan_id: "humanoid".into(),
                    beast_kinds: vec!["rat".to_string(), "whale".to_string()],
                },
            ],
            vec![],
            &body_plans,
        )
        .expect("races.json fixture valid");
        (body_plans, races)
    }

    /// 只为了在测试里取到一个真实 `Entity` 值（本函数不查 ECS 组件，仅用于
    /// resolve_body_plan 的错误消息/日志上下文）。
    fn dummy_entity() -> Entity {
        let mut world = World::new();
        world.spawn_empty().id()
    }

    #[test]
    fn player_with_known_race_resolves_humanoid_for_both_purposes() {
        let (body_plans, races) = registries();
        let cultivation = Cultivation::default();
        for purpose in [BodyPlanPurpose::Intrinsic, BodyPlanPurpose::Form] {
            let plan = resolve_body_plan(
                dummy_entity(),
                purpose,
                BodyPlanResolveInputs {
                    cultivation: Some(&cultivation),
                    beast_kind: None,
                },
                &body_plans,
                &races,
            )
            .expect("known human race must resolve");
            assert_eq!(plan.id.as_str(), "humanoid");
        }
    }

    #[test]
    fn player_with_unknown_race_is_rejected_not_defaulted() {
        let (body_plans, races) = registries();
        let mut cultivation = Cultivation::default();
        cultivation.race = RaceId::new("does_not_exist");
        let err = resolve_body_plan(
            dummy_entity(),
            BodyPlanPurpose::Intrinsic,
            BodyPlanResolveInputs {
                cultivation: Some(&cultivation),
                beast_kind: None,
            },
            &body_plans,
            &races,
        )
        .expect_err("unknown race must not silently fall back to humanoid");
        assert_eq!(
            err,
            ResolveBodyPlanError::UnknownPlayerRace(RaceId::new("does_not_exist"))
        );
    }

    #[test]
    fn player_with_unknown_race_rejected_for_form_purpose_too() {
        let (body_plans, races) = registries();
        let mut cultivation = Cultivation::default();
        cultivation.race = RaceId::new("does_not_exist");
        let err = resolve_body_plan(
            dummy_entity(),
            BodyPlanPurpose::Form,
            BodyPlanResolveInputs {
                cultivation: Some(&cultivation),
                beast_kind: None,
            },
            &body_plans,
            &races,
        )
        .expect_err(
            "Form purpose must reject unknown race exactly like Intrinsic (未易形 Form≡Intrinsic)",
        );
        assert_eq!(
            err,
            ResolveBodyPlanError::UnknownPlayerRace(RaceId::new("does_not_exist"))
        );
    }

    #[test]
    fn npc_beast_kind_resolves_via_race_registry_derivation() {
        let (body_plans, races) = registries();
        let plan = resolve_body_plan(
            dummy_entity(),
            BodyPlanPurpose::Intrinsic,
            BodyPlanResolveInputs {
                cultivation: None,
                beast_kind: Some(&BeastKind::Rat),
            },
            &body_plans,
            &races,
        )
        .expect("beast kind must resolve via races.json derivation");
        assert_eq!(plan.id.as_str(), "humanoid");
    }

    #[test]
    fn fauna_at_enum_boundary_whale_resolves_too() {
        let (body_plans, races) = registries();
        let plan = resolve_body_plan(
            dummy_entity(),
            BodyPlanPurpose::Intrinsic,
            BodyPlanResolveInputs {
                cultivation: None,
                beast_kind: Some(&BeastKind::Whale),
            },
            &body_plans,
            &races,
        )
        .expect("whale (last BeastKind variant) must resolve");
        assert_eq!(plan.id.as_str(), "humanoid");
    }

    #[test]
    fn beast_kind_takes_priority_over_stray_cultivation_component() {
        // 一只带着残留 Cultivation 组件（例如曾被某个系统误插入）的 spider 必须仍按
        // BeastKind 判定几何，不能被误判为「玩家」（否则会消费 Cultivation.race，
        // 这不是它的身份来源）。
        let (body_plans, races) = registries();
        let mut cultivation = Cultivation::default();
        cultivation.race = RaceId::new("does_not_exist"); // 若被误当玩家会直接报错
        let plan = resolve_body_plan(
            dummy_entity(),
            BodyPlanPurpose::Intrinsic,
            BodyPlanResolveInputs {
                cultivation: Some(&cultivation),
                beast_kind: Some(&BeastKind::Spider),
            },
            &body_plans,
            &races,
        )
        .expect("BeastKind tier must win over a stray Cultivation component");
        assert_eq!(plan.id.as_str(), "humanoid");
    }

    #[test]
    fn entity_with_neither_marker_falls_back_to_humanoid() {
        let (body_plans, races) = registries();
        let plan = resolve_body_plan(
            dummy_entity(),
            BodyPlanPurpose::Intrinsic,
            BodyPlanResolveInputs {
                cultivation: None,
                beast_kind: None,
            },
            &body_plans,
            &races,
        )
        .expect("entity with no identity markers must fall back to humanoid, not error");
        assert_eq!(plan.id.as_str(), "humanoid");
    }

    #[test]
    fn body_part_for_mutation_slot_returns_mapped_part() {
        let plan = humanoid_plan();
        let part = body_part_for_mutation_slot(&plan, BodySlot::Head)
            .expect("Head slot is mapped in the fixture plan");
        assert_eq!(part, &BodyPartId::new("head"));
    }

    #[test]
    fn body_part_for_mutation_slot_returns_none_for_unmapped_slot() {
        let plan = humanoid_plan();
        assert_eq!(body_part_for_mutation_slot(&plan, BodySlot::Torso), None);
    }

    #[test]
    fn body_part_for_mutation_slot_returns_none_for_empty_mapping() {
        let mut plan = humanoid_plan();
        plan.mutation_slot_mapping.clear();
        assert_eq!(body_part_for_mutation_slot(&plan, BodySlot::Head), None);
    }

    #[test]
    fn resolve_body_plan_error_display_mentions_race_id() {
        let err = ResolveBodyPlanError::UnknownPlayerRace(RaceId::new("phantom"));
        assert!(err.to_string().contains("phantom"));
    }
}
