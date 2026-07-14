//! plan-race-system-v1 P0 — 唯一解析入口 `resolve_body_plan`。
//!
//! 所有需要"这个实体现在是什么身体构型"答案的消费点（命中几何 / 伤残后果 / 经脉 /
//! 功法门 / 伤口面板 / 装备槽）都必须走本函数，禁止绕过直查 `BodyPlanRegistry`——
//! plan §P0 决议原文："所有消费点只准走此入口并逐点标注 purpose"。本函数已接入真实
//! 生产消费点：`combat::resolve::body_part_multipliers`（部位倍率）、
//! `combat::resolve::resolve_attack_intents`（近战 `raycast_humanoid` 分支，经
//! [`resolve_body_plan_for_target`]）、`combat::carrier::projectile_tick_system`
//! （投射物 `classify_body_part` 分支，同经 [`resolve_body_plan_for_target`]）——
//! 三者均对目标实体走 `BodyPlanPurpose::Intrinsic` 解析，缺失 registry 资源时
//! （大量既有单测未插入）优雅退化到 [`super::registry::humanoid_plan_static`]。
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

use crate::cultivation::components::{Cultivation, MeridianChannelId};
use crate::cultivation::topology::MeridianTopology;
use crate::dandao::mutation::BodySlot;
use crate::fauna::components::BeastKind;

use super::race_registry::RaceRegistry;
use super::registry::BodyPlanRegistry;
use super::types::{BodyPartId, BodyPlan, MeridianProfile, RaceId};

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
    /// plan-race-system-v1 P4 —— 当前易形形态（`None` = 未易形）。**只在
    /// `BodyPlanPurpose::Form` 分支被读取**——`Intrinsic` 分支恒等忽略本字段（未易形时
    /// Form≡Intrinsic 的既有约定不受影响，既有调用点可安全传 `None`）。
    pub morph_state: Option<&'a super::morph::MorphState>,
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
    purpose: BodyPlanPurpose,
    inputs: BodyPlanResolveInputs<'_>,
    body_plans: &'a BodyPlanRegistry,
    races: &RaceRegistry,
) -> Result<&'a BodyPlan, ResolveBodyPlanError> {
    // plan-race-system-v1 P4 —— Form purpose + MorphState 在场：当前形态权威真源是
    // `MorphState.form`，不再走 Tier1/2/3 本体判定。未易形（`morph_state = None`）时
    // 无条件落进下方既有逻辑，保持"未易形 Form≡Intrinsic"这条 P0-P3 既有契约不变。
    if matches!(purpose, BodyPlanPurpose::Form) {
        if let Some(morph) = inputs.morph_state {
            return resolve_race_to_plan(&morph.form, body_plans, races)
                .ok_or_else(|| ResolveBodyPlanError::UnknownPlayerRace(morph.form.clone()));
        }
    }

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

/// plan-race-system-v1 P1b —— `combat::baomai_v4::dead_armor::meridian_to_body_part`
/// 私表退役后的查询入口：给定 channel id，返回其 `ChannelDef.body_part`（`None` = 无
/// 可命中体表映射的奇经，或 plan 未声明该 channel）。
pub fn channel_body_part(plan: &BodyPlan, channel: &MeridianChannelId) -> Option<BodyPartId> {
    plan.meridian_profile
        .as_ref()?
        .channels
        .iter()
        .find(|c| &c.id == channel)
        .and_then(|c| c.body_part.clone())
}

/// plan-race-system-v1 P1b —— `cultivation::dugu::body_part_to_meridian` 私表退役后的
/// 查询入口：给定体表部位，返回排异毒素累积到哪条 channel（`None` = plan 未声明该
/// body_part 的 dugu 注入映射，如非人形构型或未接入 dugu 玩法）。
pub fn dugu_injection_channel(
    plan: &BodyPlan,
    body_part: &BodyPartId,
) -> Option<MeridianChannelId> {
    plan.meridian_profile
        .as_ref()?
        .dugu_injection
        .iter()
        .find(|e| &e.body_part == body_part)
        .map(|e| e.channel.clone())
}

/// plan-race-system-v1 P0c —— 消费点通用封装：`combat::resolve::body_part_multipliers`
/// 首创的"resource missing / unknown race → humanoid 兜底"约定在这里被抽成公共入口，
/// 供 `combat::raycast`（`classify_body_part`/`standing_humanoid_aabb`/`raycast_humanoid`
/// 的目标实体分派）与 `combat::carrier`（投射物命中路径）等新增消费点复用，避免同一条
/// 退化逻辑散落三份。
///
/// `body_plans`/`races` 均存在时走 [`resolve_body_plan`]；解析失败（未知玩家 race，
/// 理论上不会发生——`cultivation::attach_cultivation_to_joined_clients` 早已在持久化
/// 加载路径拒绝未知 race 落地为组件）或任一资源缺失（大量既有单测未插入这两个资源）
/// 时退化到 [`super::registry::humanoid_plan_static`]（与 registry 加载同一份
/// `humanoid.json`，数值 bit-for-bit 相同，不是第二份硬编码表）——生产环境
/// `body_plan::register()` 恒装载两资源，这条退化分支不会在真实部署触发。
pub fn resolve_body_plan_for_target<'a>(
    entity: Entity,
    purpose: BodyPlanPurpose,
    inputs: BodyPlanResolveInputs<'_>,
    body_plans: Option<&'a BodyPlanRegistry>,
    races: Option<&RaceRegistry>,
) -> &'a BodyPlan {
    match (body_plans, races) {
        (Some(body_plans), Some(races)) => {
            match resolve_body_plan(entity, purpose, inputs, body_plans, races) {
                Ok(plan) => plan,
                Err(error) => {
                    tracing::error!(
                        "[bong][body_plan] resolve_body_plan_for_target: {error} — falling \
                         back to humanoid"
                    );
                    super::registry::humanoid_plan_static()
                }
            }
        }
        _ => super::registry::humanoid_plan_static(),
    }
}

/// plan-race-system-v1 P1b —— `meridian_open`/NPC 选招消费点的经脉拓扑解析入口，
/// 语义与 [`resolve_body_plan_for_target`] 完全对齐（同一套退化规则：解析失败或资源
/// 缺失退化到 humanoid 单例）。目标 plan 未声明 `meridian_profile`（P0 遗留 fixture /
/// 尚未接入经脉的非人形构型）时同样退化到 humanoid 拓扑——`meridian_open_tick` 等
/// 消费点始终需要"某种拓扑数据"才能判定邻接，不能对着 `None` 停摆。
pub fn resolve_meridian_topology_for_target<'a>(
    entity: Entity,
    purpose: BodyPlanPurpose,
    inputs: BodyPlanResolveInputs<'_>,
    body_plans: Option<&'a BodyPlanRegistry>,
    races: Option<&RaceRegistry>,
) -> &'a MeridianTopology {
    match (body_plans, races) {
        (Some(body_plans), Some(races)) => {
            match resolve_body_plan(entity, purpose, inputs, body_plans, races) {
                Ok(plan) => body_plans
                    .topology_for(&plan.id)
                    .unwrap_or_else(|| super::registry::humanoid_topology_static()),
                Err(error) => {
                    tracing::error!(
                        "[bong][body_plan] resolve_meridian_topology_for_target: {error} — \
                         falling back to humanoid"
                    );
                    super::registry::humanoid_topology_static()
                }
            }
        }
        _ => super::registry::humanoid_topology_static(),
    }
}

/// plan-race-system-v1 P5 —— `cultivation::breakthrough` 突破配额消费点用，语义与
/// [`resolve_meridian_topology_for_target`] 完全对齐（同一套解析 + 退化规则）。目标
/// plan 缺 `meridian_profile`（非人形构型尚未接入经脉，如 P0 遗留 fixture）时同样退化
/// 到 humanoid 曲线——`breakthrough_precondition_error_for_profile` 等消费点始终需要
/// "某种配额曲线"才能判定，不能对着 `None` 停摆。humanoid 目标本身经此函数解析出的
/// 结果与旧的零参 `humanoid_plan_static().meridian_profile` 直读 bit-for-bit 相同
/// （同一份 `humanoid.json`），换轨不改变现有人族突破行为。
pub fn meridian_profile_for_target<'a>(
    entity: Entity,
    purpose: BodyPlanPurpose,
    inputs: BodyPlanResolveInputs<'_>,
    body_plans: Option<&'a BodyPlanRegistry>,
    races: Option<&RaceRegistry>,
) -> &'a MeridianProfile {
    let plan = resolve_body_plan_for_target(entity, purpose, inputs, body_plans, races);
    plan.meridian_profile.as_ref().unwrap_or_else(|| {
        super::registry::humanoid_plan_static()
            .meridian_profile
            .as_ref()
            .expect(
                "humanoid body plan must declare meridian_profile from plan-race-system-v1 P1 \
                 onward — validate_body_plan should have rejected a humanoid plan missing it",
            )
    })
}

/// plan-race-system-v1 P3a —— 施放门 race gate 消费点用（`sword_path::skill_register`
/// / `combat::sword_basics` 的技能 resolver 均以 `world: &bevy_ecs::world::World` 原始
/// World 访问操作，而非 Bevy `Query`/`Res` system param），封装
/// `resolve_body_plan_for_target(..., BodyPlanPurpose::Intrinsic, ...)` 的原始 World
/// 版本，避免每个 resolver 各自手搓一遍 `world.get::<Cultivation>` /
/// `world.get_resource::<BodyPlanRegistry>` 拼装。
pub fn intrinsic_is_humanoid_from_world(
    world: &valence::prelude::bevy_ecs::world::World,
    entity: Entity,
) -> bool {
    let cultivation = world.get::<Cultivation>(entity);
    let body_plans = world.get_resource::<super::registry::BodyPlanRegistry>();
    let races = world.get_resource::<RaceRegistry>();
    resolve_body_plan_for_target(
        entity,
        BodyPlanPurpose::Intrinsic,
        BodyPlanResolveInputs {
            cultivation,
            // `BeastKind` 不是 Bevy `Component`（既有 `combat::resolve` / `combat::carrier` /
            // `cultivation::meridian_open` 消费点同款简化，见 `resolve_body_plan` 模块文档）——
            // sword_path / sword_basics 施放门只对玩家实体生效（NPC 不走这两条 cast 路径），
            // `None` 与"真的查了 BeastKind"结果 bit-for-bit 一致。
            beast_kind: None,
            morph_state: None,
        },
        body_plans,
        races,
    )
    .is_humanoid
}

/// plan-race-system-v1 P4 —— 给定种族 id，直接查表返回其 [`BodyPlan`]（`races.json`
/// 的 `RaceEntry.body_plan_id` 一跳）。`resolve_body_plan` 的 Form 分支、
/// `combat::resolve` 的护甲折算逆查等消费点复用本函数，避免各自手写
/// `races.get(...).and_then(|e| body_plans.get(&e.body_plan_id))`。
pub fn resolve_race_to_plan<'a>(
    race: &RaceId,
    body_plans: &'a BodyPlanRegistry,
    races: &RaceRegistry,
) -> Option<&'a BodyPlan> {
    let entry = races.get(race)?;
    body_plans.get(&entry.body_plan_id)
}

/// plan-race-system-v1 P4 —— Form 身份（当前形态 race_id + is_humanoid）的原始 World
/// 版本，镜像 [`intrinsic_is_humanoid_from_world`] 的用法（无 Bevy `Query` 访问权限的
/// resolver / 原始 `World` 消费点用）。修复此前"未易形时 Form≡Intrinsic 掩盖了
/// `form_race_id` 恒等于本体 `Cultivation.race`"的问题——`MorphState` 落地后本函数
/// 优先读取它，而不是无条件回落本体身份。
pub fn form_identity_from_world(
    world: &valence::prelude::bevy_ecs::world::World,
    entity: Entity,
) -> (RaceId, bool) {
    let cultivation = world.get::<Cultivation>(entity);
    let morph_state = world.get::<super::morph::MorphState>(entity);
    let body_plans = world.get_resource::<super::registry::BodyPlanRegistry>();
    let races = world.get_resource::<RaceRegistry>();

    let intrinsic_race = cultivation
        .map(|c| c.race.clone())
        .unwrap_or_else(|| RaceId::new(super::race_registry::HUMAN_RACE_ID));
    let form_race_id = morph_state
        .map(|m| m.form.clone())
        .unwrap_or(intrinsic_race);

    let plan = resolve_body_plan_for_target(
        entity,
        BodyPlanPurpose::Form,
        BodyPlanResolveInputs {
            cultivation,
            beast_kind: None,
            morph_state,
        },
        body_plans,
        races,
    );
    (form_race_id, plan.is_humanoid)
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
            // plan-race-system-v1 P1a：validate_body_plan 现在要求 is_humanoid==true
            // 必须提供 meridian_profile；本 fixture 明确代表"the humanoid plan"，给一条
            // 最小合法 channel + 六境界配额，保持语义忠实而非改 is_humanoid=false。
            meridian_profile: Some(crate::body_plan::types::MeridianProfile {
                channels: vec![crate::body_plan::types::ChannelDef {
                    id: "lung".into(),
                    family: crate::body_plan::types::MeridianFamily::Regular,
                    body_part: Some(BodyPartId::new("head")),
                    roles: vec![],
                }],
                topology_edges: vec![],
                dugu_injection: vec![],
                realm_requirements: [crate::body_plan::types::RealmMeridianReq {
                    total: 1,
                    regular_min: 1,
                    extraordinary_min: 0,
                }; 6],
            }),
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
                    morph_state: None,
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
        let cultivation = Cultivation {
            race: RaceId::new("does_not_exist"),
            ..Default::default()
        };
        let err = resolve_body_plan(
            dummy_entity(),
            BodyPlanPurpose::Intrinsic,
            BodyPlanResolveInputs {
                cultivation: Some(&cultivation),
                beast_kind: None,
                morph_state: None,
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
        let cultivation = Cultivation {
            race: RaceId::new("does_not_exist"),
            ..Default::default()
        };
        let err = resolve_body_plan(
            dummy_entity(),
            BodyPlanPurpose::Form,
            BodyPlanResolveInputs {
                cultivation: Some(&cultivation),
                beast_kind: None,
                morph_state: None,
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
    fn npc_with_cultivation_and_known_race_resolves_like_player_for_both_purposes() {
        // "NPC" 轴的另一半：非兽类、携带 Cultivation 的战斗 NPC（如道伥一类人形
        // 敌对生物，`npc/technique.rs` 同样挂 Cultivation 供功法/经脉判定）与玩家共用
        // 同一条 Tier2 分支——本测试锁定 Tier2 判定不依赖"是不是玩家"这个身份，
        // 只认 Cultivation.race，任何携带该组件的实体（NPC 亦然）都走这条路径。
        let (body_plans, races) = registries();
        let cultivation = Cultivation {
            race: RaceId::new("beast_common"),
            ..Default::default()
        };
        for purpose in [BodyPlanPurpose::Intrinsic, BodyPlanPurpose::Form] {
            let plan = resolve_body_plan(
                dummy_entity(),
                purpose,
                BodyPlanResolveInputs {
                    cultivation: Some(&cultivation),
                    beast_kind: None,
                    morph_state: None,
                },
                &body_plans,
                &races,
            )
            .expect("NPC with a known Cultivation.race must resolve exactly like a player");
            assert_eq!(plan.id.as_str(), "humanoid", "purpose={purpose:?}");
        }
    }

    #[test]
    fn npc_with_cultivation_and_unknown_race_rejected_for_both_purposes() {
        let (body_plans, races) = registries();
        let cultivation = Cultivation {
            race: RaceId::new("npc_ghost_race"),
            ..Default::default()
        };
        for purpose in [BodyPlanPurpose::Intrinsic, BodyPlanPurpose::Form] {
            let err = resolve_body_plan(
                dummy_entity(),
                purpose,
                BodyPlanResolveInputs {
                    cultivation: Some(&cultivation),
                    beast_kind: None,
                    morph_state: None,
                },
                &body_plans,
                &races,
            )
            .expect_err(
                "NPC with an unknown Cultivation.race must be rejected exactly like a player \
                 — no separate lenient path for non-player humanoid entities",
            );
            assert_eq!(
                err,
                ResolveBodyPlanError::UnknownPlayerRace(RaceId::new("npc_ghost_race")),
                "purpose={purpose:?}"
            );
        }
    }

    #[test]
    fn npc_beast_kind_resolves_via_race_registry_derivation_for_both_purposes() {
        // "NPC" 轴（携带 BeastKind 的兽形战斗实体）——Tier1 分支对 Intrinsic/Form 两种
        // purpose 必须给出相同解析结果（P0 未落地 MorphState 前 Form≡Intrinsic）。
        let (body_plans, races) = registries();
        for purpose in [BodyPlanPurpose::Intrinsic, BodyPlanPurpose::Form] {
            let plan = resolve_body_plan(
                dummy_entity(),
                purpose,
                BodyPlanResolveInputs {
                    cultivation: None,
                    beast_kind: Some(&BeastKind::Rat),
                    morph_state: None,
                },
                &body_plans,
                &races,
            )
            .expect("beast kind must resolve via races.json derivation");
            assert_eq!(plan.id.as_str(), "humanoid", "purpose={purpose:?}");
        }
    }

    #[test]
    fn fauna_at_enum_boundary_whale_resolves_for_both_purposes() {
        // "fauna" 轴的枚举边界样本（`BeastKind` 最后一个变体）——同样对两种 purpose
        // 都要覆盖，不能只验证 Intrinsic 就假设 Form 分支"顺带也对"。
        let (body_plans, races) = registries();
        for purpose in [BodyPlanPurpose::Intrinsic, BodyPlanPurpose::Form] {
            let plan = resolve_body_plan(
                dummy_entity(),
                purpose,
                BodyPlanResolveInputs {
                    cultivation: None,
                    beast_kind: Some(&BeastKind::Whale),
                    morph_state: None,
                },
                &body_plans,
                &races,
            )
            .expect("whale (last BeastKind variant) must resolve");
            assert_eq!(plan.id.as_str(), "humanoid", "purpose={purpose:?}");
        }
    }

    #[test]
    fn beast_kind_takes_priority_over_stray_cultivation_component_for_both_purposes() {
        // 一只带着残留 Cultivation 组件（例如曾被某个系统误插入）的 spider 必须仍按
        // BeastKind 判定几何，不能被误判为「玩家」（否则会消费 Cultivation.race，
        // 这不是它的身份来源）——两种 purpose 下都要保持这一优先级。
        let (body_plans, races) = registries();
        let cultivation = Cultivation {
            race: RaceId::new("does_not_exist"), // 若被误当玩家会直接报错
            ..Default::default()
        };
        for purpose in [BodyPlanPurpose::Intrinsic, BodyPlanPurpose::Form] {
            let plan = resolve_body_plan(
                dummy_entity(),
                purpose,
                BodyPlanResolveInputs {
                    cultivation: Some(&cultivation),
                    beast_kind: Some(&BeastKind::Spider),
                    morph_state: None,
                },
                &body_plans,
                &races,
            )
            .expect("BeastKind tier must win over a stray Cultivation component");
            assert_eq!(plan.id.as_str(), "humanoid", "purpose={purpose:?}");
        }
    }

    #[test]
    fn entity_with_neither_marker_falls_back_to_humanoid_for_both_purposes() {
        // "其他实体" 轴（既非玩家也非兽类）——Tier3 兜底同样必须对两种 purpose 一致。
        let (body_plans, races) = registries();
        for purpose in [BodyPlanPurpose::Intrinsic, BodyPlanPurpose::Form] {
            let plan = resolve_body_plan(
                dummy_entity(),
                purpose,
                BodyPlanResolveInputs {
                    cultivation: None,
                    beast_kind: None,
                    morph_state: None,
                },
                &body_plans,
                &races,
            )
            .expect("entity with no identity markers must fall back to humanoid, not error");
            assert_eq!(plan.id.as_str(), "humanoid", "purpose={purpose:?}");
        }
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
    fn body_part_for_mutation_slot_covers_every_body_slot_variant_on_real_humanoid_plan() {
        // 真实 `assets/body_plans/plans/humanoid.json` 的全变体饱和 pin（§7 声明落点，
        // 与 `registry.rs` 的同名断言互为独立验证路径——此处直接走
        // `super::registry::humanoid_plan_static()` 单例，不经 fixture）。
        use crate::body_plan::registry::humanoid_plan_static;

        let plan = humanoid_plan_static();
        let expected: [(BodySlot, &str); 5] = [
            (BodySlot::Head, "head"),
            (BodySlot::Forearm, "arm_r"),
            (BodySlot::Back, "back"),
            (BodySlot::Torso, "chest"),
            (BodySlot::Lower, "abdomen"),
        ];
        for (slot, expected_part_id) in expected {
            let part = body_part_for_mutation_slot(plan, slot)
                .unwrap_or_else(|| panic!("real humanoid.json must map {slot:?}"));
            assert_eq!(
                part.as_str(),
                expected_part_id,
                "slot={slot:?}: expected part {expected_part_id}, got {part}"
            );
        }
    }

    #[test]
    fn resolve_body_plan_error_display_mentions_race_id() {
        let err = ResolveBodyPlanError::UnknownPlayerRace(RaceId::new("phantom"));
        assert!(err.to_string().contains("phantom"));
    }

    // ───────────────────────── channel_body_part / dugu_injection_channel ──────
    // plan-race-system-v1 P1b —— `combat::baomai_v4::dead_armor::meridian_to_body_part`
    // / `cultivation::dugu::body_part_to_meridian` 两张私表退役后的查询 API。

    #[test]
    fn channel_body_part_returns_mapped_part_on_fixture_plan() {
        let plan = humanoid_plan();
        let part = channel_body_part(&plan, &MeridianChannelId::new("lung"))
            .expect("fixture plan maps lung -> head");
        assert_eq!(part, BodyPartId::new("head"));
    }

    #[test]
    fn channel_body_part_returns_none_for_unknown_channel() {
        let plan = humanoid_plan();
        assert_eq!(
            channel_body_part(&plan, &MeridianChannelId::new("does_not_exist")),
            None
        );
    }

    #[test]
    fn channel_body_part_returns_none_when_plan_has_no_meridian_profile() {
        let mut plan = humanoid_plan();
        plan.meridian_profile = None;
        assert_eq!(
            channel_body_part(&plan, &MeridianChannelId::new("lung")),
            None
        );
    }

    #[test]
    fn channel_body_part_returns_none_for_channel_without_body_part_mapping() {
        // 奇经排除表：`body_part: None` 的 channel（fixture 未声明该 channel 时同样
        // 视为"无映射"——本用例用真实 humanoid.json 覆盖 6 条无体部映射的奇经）。
        let plan = crate::body_plan::registry::humanoid_plan_static();
        for id in [
            "chong",
            "dai",
            "yin_qiao",
            "yang_qiao",
            "yin_wei",
            "yang_wei",
        ] {
            assert_eq!(
                channel_body_part(plan, &MeridianChannelId::new(id)),
                None,
                "channel {id} 在 humanoid.json 中应无体部映射"
            );
        }
    }

    #[test]
    fn channel_body_part_matches_retired_dead_armor_table_bit_for_bit_on_real_humanoid_plan() {
        use crate::cultivation::components::MeridianId;

        let plan = crate::body_plan::registry::humanoid_plan_static();
        let expected: [(MeridianId, &str); 14] = [
            (MeridianId::Lung, "arm_l"),
            (MeridianId::Heart, "arm_l"),
            (MeridianId::Pericardium, "arm_l"),
            (MeridianId::LargeIntestine, "arm_r"),
            (MeridianId::SmallIntestine, "arm_r"),
            (MeridianId::TripleEnergizer, "arm_r"),
            (MeridianId::Spleen, "leg_l"),
            (MeridianId::Kidney, "leg_l"),
            (MeridianId::Liver, "leg_l"),
            (MeridianId::Stomach, "leg_r"),
            (MeridianId::Bladder, "leg_r"),
            (MeridianId::Gallbladder, "leg_r"),
            (MeridianId::Ren, "chest"),
            (MeridianId::Du, "chest"),
        ];
        for (id, expected_part) in expected {
            let part = channel_body_part(plan, &id.channel_id())
                .unwrap_or_else(|| panic!("{id:?} must map to a body part"));
            assert_eq!(part.as_str(), expected_part, "{id:?}");
        }
    }

    #[test]
    fn dugu_injection_channel_returns_mapped_channel_for_unknown_free_fixture() {
        let mut plan = humanoid_plan();
        plan.meridian_profile.as_mut().unwrap().dugu_injection =
            vec![crate::body_plan::types::DuguInjectionEntry {
                body_part: BodyPartId::new("head"),
                channel: MeridianChannelId::new("du"),
            }];
        assert_eq!(
            dugu_injection_channel(&plan, &BodyPartId::new("head")),
            Some(MeridianChannelId::new("du"))
        );
    }

    #[test]
    fn dugu_injection_channel_returns_none_for_unmapped_body_part() {
        let plan = humanoid_plan();
        assert_eq!(
            dugu_injection_channel(&plan, &BodyPartId::new("does_not_exist")),
            None
        );
    }

    #[test]
    fn dugu_injection_channel_returns_none_when_plan_has_no_meridian_profile() {
        let mut plan = humanoid_plan();
        plan.meridian_profile = None;
        assert_eq!(
            dugu_injection_channel(&plan, &BodyPartId::new("head")),
            None
        );
    }

    #[test]
    fn dugu_injection_channel_matches_retired_dugu_table_bit_for_bit_on_real_humanoid_plan() {
        let plan = crate::body_plan::registry::humanoid_plan_static();
        let expected: [(&str, &str); 8] = [
            ("head", "du"),
            ("chest", "heart"),
            ("back", "du"),
            ("abdomen", "spleen"),
            ("arm_l", "large_intestine"),
            ("arm_r", "large_intestine"),
            ("leg_l", "bladder"),
            ("leg_r", "bladder"),
        ];
        for (body_part, expected_channel) in expected {
            let channel = dugu_injection_channel(plan, &BodyPartId::new(body_part))
                .unwrap_or_else(|| panic!("body_part {body_part} must map to a dugu channel"));
            assert_eq!(channel.as_str(), expected_channel, "body_part={body_part}");
        }
    }
}
