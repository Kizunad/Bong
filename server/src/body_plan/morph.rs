//! plan-race-system-v1 P4 —— 易形（形态变换）机制。
//!
//! `MorphState` 是「实体当前正易形为哪个种族外观」的唯一真源——插入即代表已易形，
//! 移除即代表已恢复本体。所有需要"当前形态"答案的消费点（装备槽/穿戴门/护甲折算/
//! 客户端渲染 payload）都经由 [`super::resolve::resolve_body_plan`] 的
//! [`super::resolve::BodyPlanPurpose::Form`] 分支消费本组件，禁止绕过直查。
//!
//! 手动解除易形走**幂等 cast 切换**（决议）：`morph.yixing` 再次施放时，若已处于
//! `MorphState` 则移除（=解除），否则插入（=易形）——不新增 C2S 协议面。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component, Entity, Position};

use crate::cultivation::components::{Cultivation, MeridianSystem};
use crate::cultivation::meridian::severed::MeridianSeveredPermanent;
use crate::cultivation::skill_registry::{CastRejectReason, CastResult, SkillRegistry};
use crate::cultivation::tick::CultivationClock;
use crate::qi_physics::constants::{QI_EPSILON, QI_ZONE_UNIT_CAPACITY};
use crate::qi_physics::ledger::{QiAccountId, QiTransfer, QiTransferReason};
use crate::qi_physics::release::qi_release_to_zone;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::zone::ZoneRegistry;

use super::race_registry::RaceRegistry;
use super::registry::BodyPlanRegistry;
use super::resolve::resolve_race_to_plan;
use super::types::{ChannelRole, MeridianProfile, RaceId};

/// 实体当前易形形态——`form` 为目标种族 id（`races.json` 键），`model_kind` 供客户端
/// 渲染差异化模型变体（PR-5b 消费，本 PR 只保证 payload 携带该字段），`since_tick`
/// 记录易形发生的 `CultivationClock` tick（供死亡/下线/UI 显示"已易形 N tick"之类的
/// 未来消费点使用，P4 本身不消费）。
#[derive(Debug, Clone, PartialEq, Eq, Component, Serialize, Deserialize)]
pub struct MorphState {
    pub form: RaceId,
    pub model_kind: u16,
    pub since_tick: u64,
}

impl MorphState {
    pub fn new(form: RaceId, model_kind: u16, since_tick: u64) -> Self {
        Self {
            form,
            model_kind,
            since_tick,
        }
    }
}

/// 「易形」类技能的本体经脉门判据表——P4 只有 `morph.yixing` 一条，未来新增易形技能
/// 加进这里即可，不必改 `TechniqueDefinition` 结构体形状（该结构体是 48 条 `const`
/// 数组字面量，加一个字段要动全部既有条目，代价远大于这张小白名单）。
pub fn technique_requires_form_anchor(technique_id: &str) -> bool {
    technique_id == "morph.yixing"
}

/// 本体 [`MeridianProfile`] 内全部标 [`ChannelRole::FormAnchor`] 的经脉是否**全部**已
/// 打通且未永久断绝（决议：易形前置检查）。profile 未声明任何 `FormAnchor` 经脉时
/// 视为无门可挡，返回 `true`（vacuous truth——非人形构型 fixture 若未声明 FormAnchor，
/// 不应被一个它自己都没定义的门卡死）。
pub fn form_anchors_open(
    profile: &MeridianProfile,
    meridians: &MeridianSystem,
    severed: Option<&MeridianSeveredPermanent>,
) -> bool {
    profile
        .channels
        .iter()
        .filter(|channel| channel.roles.contains(&ChannelRole::FormAnchor))
        .all(|channel| {
            if !meridians.contains(channel.id.clone()) {
                return false;
            }
            if severed.is_some_and(|s| s.is_severed(channel.id.clone())) {
                return false;
            }
            meridians.get(channel.id.clone()).opened
        })
}

/// `morph.yixing` 技能 id——`known_techniques::TECHNIQUE_DEFINITIONS` 与
/// `SkillRegistry` 的共享真源，避免字符串字面量在两处漂移。
pub const YIXING_SKILL_ID: &str = "morph.yixing";

/// 一次易形 cast 的默认冷却/演出时长（tick，20 TPS）——与
/// `TechniqueDefinition{id:"morph.yixing"}` 的 `cast_ticks`/`cooldown_ticks` 字段
/// 数值同步，本处只是 `CastResult::Started` 回填给 client 的演出参数，非权威真源
/// （权威真源仍是 `TechniqueDefinition`，改一处需同步改另一处，两条 pin 测试互相锁定）。
const YIXING_CAST_TICKS: u32 = 60;
const YIXING_COOLDOWN_TICKS: u64 = 600;

pub fn register_skills(registry: &mut SkillRegistry) {
    registry.register(YIXING_SKILL_ID, cast_morph_yixing);
}

pub fn declare_meridian_dependencies(
    dependencies: &mut crate::cultivation::meridian::severed::SkillMeridianDependencies,
) {
    // 无经脉前置表条目——真正的前置门是本模块专属的 `form_anchors_open`
    // （`technique_requires_form_anchor` 白名单命中，由 cast/学习两处收拢点消费），
    // 不是这张通用表；显式声明空 deps 满足 `SkillRegistry` 审计完整性不变量。
    dependencies.declare(YIXING_SKILL_ID, Vec::new());
}

/// `morph.yixing` 的 `SkillFn` resolver——手动易形/解除的**唯一**落点（决议 §1
/// 幂等切换：已处于 `MorphState` 时再次施放=解除，否则=易形）。
///
/// 易形（插入）分支：本体 race 在 `races.json.morph_pairs` 里必须有正向配对（P4 生产
/// `races.json` 为空数组，本分支恒 `CastRejectReason::InvalidTarget`——"机制已通、
/// 缺数据"是预期状态，P5 补 whale 真数据后本函数无需任何改动即可工作）。
fn cast_morph_yixing(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    _slot: u8,
    _target: Option<Entity>,
) -> CastResult {
    if world.get::<MorphState>(caster).is_some() {
        release_morph_state(world, caster);
        return CastResult::Started {
            cooldown_ticks: YIXING_COOLDOWN_TICKS,
            anim_duration_ticks: YIXING_CAST_TICKS,
        };
    }

    let Some(cultivation) = world.get::<Cultivation>(caster) else {
        return CastResult::Rejected {
            reason: CastRejectReason::InvalidTarget,
        };
    };
    let intrinsic_race = cultivation.race.clone();

    let Some(races) = world.get_resource::<RaceRegistry>() else {
        return CastResult::Rejected {
            reason: CastRejectReason::InvalidTarget,
        };
    };
    let Some(target_race) = races
        .morph_targets_from(&intrinsic_race)
        .into_iter()
        .next()
        .cloned()
    else {
        // 无正向 morph_pairs 配对——生产 races.json 当前恒此分支（P4 无真实易形目标）。
        return CastResult::Rejected {
            reason: CastRejectReason::InvalidTarget,
        };
    };

    let qi_cost = f64::from(
        crate::cultivation::known_techniques::technique_definition(YIXING_SKILL_ID)
            .map(|def| def.qi_cost)
            .unwrap_or(40.0),
    );
    if !drain_qi_to_zone(world, caster, qi_cost) {
        return CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient,
        };
    }

    let tick = world
        .get_resource::<CultivationClock>()
        .map(|clock| clock.tick)
        .unwrap_or(0);
    world
        .entity_mut(caster)
        .insert(MorphState::new(target_race, 0, tick));

    CastResult::Started {
        cooldown_ticks: YIXING_COOLDOWN_TICKS,
        anim_duration_ticks: YIXING_CAST_TICKS,
    }
}

/// 解除易形的公共落点——手动再 cast（本文件）/ 死亡（`combat::lifecycle`）/ 下线
/// （`player::mod`）三条触发路径共用本函数：移除 `MorphState` 组件后，本体身份恢复
/// 权威真源，立即重扫装备（`inventory::enforce_intrinsic_gate_on_morph_release`）
/// 卸下本体身份不再允许穿戴的物件（塞进背包，背包满则掉落，绝不静默销毁）。
pub fn release_morph_state(world: &mut bevy_ecs::world::World, entity: Entity) -> bool {
    if world.get::<MorphState>(entity).is_none() {
        return false;
    }
    world.entity_mut(entity).remove::<MorphState>();

    let intrinsic_race = world
        .get::<Cultivation>(entity)
        .map(|c| c.race.clone())
        .unwrap_or_else(|| RaceId::new(super::race_registry::HUMAN_RACE_ID));
    let body_plans = world.get_resource::<BodyPlanRegistry>();
    let races = world.get_resource::<RaceRegistry>();
    let intrinsic_is_humanoid = match (body_plans, races) {
        (Some(body_plans), Some(races)) => resolve_race_to_plan(&intrinsic_race, body_plans, races)
            .map(|plan| plan.is_humanoid)
            .unwrap_or(true),
        _ => true,
    };

    let player_pos = world
        .get::<Position>(entity)
        .map(|p| p.get())
        .unwrap_or([0.0, 0.0, 0.0].into());
    let player_dimension = world
        .get::<CurrentDimension>(entity)
        .map(|d| d.0)
        .unwrap_or(DimensionKind::Overworld);

    if !world.contains_resource::<crate::inventory::ItemRegistry>()
        || !world.contains_resource::<crate::inventory::DroppedLootRegistry>()
    {
        // 既有大量单测未插入这两个资源——优雅跳过装备重扫（与仓库其余 P0b 消费点
        // 同款退化规则），不 panic。
        return true;
    }

    let (stashed, dropped) = world.resource_scope(
        |world, item_registry: bevy_ecs::world::Mut<crate::inventory::ItemRegistry>| {
            world.resource_scope(
                |world,
                 mut dropped_registry: bevy_ecs::world::Mut<
                    crate::inventory::DroppedLootRegistry,
                >| {
                    let Some(mut inventory) =
                        world.get_mut::<crate::inventory::PlayerInventory>(entity)
                    else {
                        return (Vec::new(), Vec::new());
                    };
                    crate::inventory::enforce_intrinsic_gate_on_morph_release(
                        &mut inventory,
                        &item_registry,
                        &mut dropped_registry,
                        &intrinsic_race,
                        intrinsic_is_humanoid,
                        [player_pos.x, player_pos.y, player_pos.z],
                        player_dimension,
                    )
                },
            )
        },
    );
    if !stashed.is_empty() || !dropped.is_empty() {
        tracing::info!(
            "[bong][body_plan][morph] release_morph_state entity={entity:?} stashed={stashed:?} dropped={dropped:?}"
        );
    }
    true
}

/// 实际扣除 qi 并将散出真元归还玩家所在 zone（player → zone 守恒，镜像
/// `dandao::skills::drain_dandao_qi` 的既有模式——见该函数文档）。返回 `true` 表示扣除
/// 成功；真元不足 / 无 `Cultivation` 组件时返回 `false` 且不做任何修改。
fn drain_qi_to_zone(world: &mut bevy_ecs::world::World, caster: Entity, cost: f64) -> bool {
    if cost <= 0.0 {
        return true;
    }
    {
        let Some(mut cultivation) = world.get_mut::<Cultivation>(caster) else {
            return false;
        };
        if cultivation.qi_current + f64::EPSILON < cost {
            return false;
        }
        cultivation.qi_current = (cultivation.qi_current - cost).clamp(0.0, cultivation.qi_max);
    }

    let from = QiAccountId::player(format!("entity:{}", caster.to_bits()));
    let position = world.get::<Position>(caster).map(|p| p.get());
    let dimension = world
        .get::<CurrentDimension>(caster)
        .map(|d| d.0)
        .unwrap_or(DimensionKind::Overworld);

    let mut pending_transfers: Vec<QiTransfer> = Vec::new();

    if let (Some(pos), Some(mut zones)) = (position, world.get_resource_mut::<ZoneRegistry>()) {
        let zone_name = zones.find_zone(dimension, pos).map(|z| z.name.clone());
        if let Some(zone_name) = zone_name {
            if let Some(zone) = zones.find_zone_mut(&zone_name) {
                let zone_current = zone.spirit_qi * QI_ZONE_UNIT_CAPACITY;
                let to = QiAccountId::zone(zone.name.clone());
                match qi_release_to_zone(
                    cost,
                    from.clone(),
                    to,
                    zone_current,
                    QI_ZONE_UNIT_CAPACITY,
                ) {
                    Ok(outcome) => {
                        zone.spirit_qi =
                            (outcome.zone_after / QI_ZONE_UNIT_CAPACITY).clamp(-1.0, 1.0);
                        if let Some(t) = outcome.transfer {
                            pending_transfers.push(t);
                        }
                        if outcome.overflow > QI_EPSILON {
                            let overflow_to = QiAccountId::overflow(format!(
                                "morph_yixing_overflow:{}",
                                caster.to_bits()
                            ));
                            if let Ok(t) = QiTransfer::new(
                                from.clone(),
                                overflow_to,
                                outcome.overflow,
                                QiTransferReason::ReleaseToZone,
                            ) {
                                pending_transfers.push(t);
                            }
                        }
                    }
                    Err(err) => {
                        tracing::warn!(
                            ?err,
                            "[bong][body_plan][morph] invalid qi release for {:?}; routing to overflow",
                            caster
                        );
                        let overflow_to = QiAccountId::overflow(format!(
                            "morph_yixing_overflow:{}",
                            caster.to_bits()
                        ));
                        if let Ok(t) = QiTransfer::new(
                            from.clone(),
                            overflow_to,
                            cost,
                            QiTransferReason::ReleaseToZone,
                        ) {
                            pending_transfers.push(t);
                        }
                    }
                }
            } else {
                let overflow_to =
                    QiAccountId::overflow(format!("morph_yixing_overflow:{}", caster.to_bits()));
                if let Ok(t) = QiTransfer::new(
                    from.clone(),
                    overflow_to,
                    cost,
                    QiTransferReason::ReleaseToZone,
                ) {
                    pending_transfers.push(t);
                }
            }
        } else {
            let overflow_to =
                QiAccountId::overflow(format!("morph_yixing_overflow:{}", caster.to_bits()));
            if let Ok(t) = QiTransfer::new(
                from.clone(),
                overflow_to,
                cost,
                QiTransferReason::ReleaseToZone,
            ) {
                pending_transfers.push(t);
            }
        }
    } else {
        let overflow_to =
            QiAccountId::overflow(format!("morph_yixing_overflow:{}", caster.to_bits()));
        if let Ok(t) = QiTransfer::new(
            from.clone(),
            overflow_to,
            cost,
            QiTransferReason::ReleaseToZone,
        ) {
            pending_transfers.push(t);
        }
    }

    if let Some(mut events) = world.get_resource_mut::<valence::prelude::Events<QiTransfer>>() {
        for t in pending_transfers {
            events.send(t);
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body_plan::types::{ChannelDef, MeridianFamily, RealmMeridianReq};
    use crate::cultivation::components::Meridian;

    fn profile_with_anchors(anchor_ids: &[&str], plain_ids: &[&str]) -> MeridianProfile {
        let mut channels = Vec::new();
        for id in anchor_ids {
            channels.push(ChannelDef {
                id: (*id).into(),
                family: MeridianFamily::Extraordinary,
                body_part: None,
                roles: vec![ChannelRole::FormAnchor],
            });
        }
        for id in plain_ids {
            channels.push(ChannelDef {
                id: (*id).into(),
                family: MeridianFamily::Regular,
                body_part: None,
                roles: vec![],
            });
        }
        MeridianProfile {
            channels,
            topology_edges: vec![],
            dugu_injection: vec![],
            realm_requirements: [RealmMeridianReq::default(); 6],
        }
    }

    fn meridians_with(opened_ids: &[&str], all_ids: &[&str]) -> MeridianSystem {
        let mut system = MeridianSystem {
            regular: Vec::new(),
            extraordinary: Vec::new(),
        };
        for id in all_ids {
            let mut m = Meridian::new((*id).into());
            if opened_ids.contains(id) {
                m.opened = true;
            }
            system.extraordinary.push(m);
        }
        system
    }

    #[test]
    fn technique_requires_form_anchor_only_matches_morph_yixing() {
        assert!(technique_requires_form_anchor("morph.yixing"));
        assert!(!technique_requires_form_anchor("sword.cleave"));
        assert!(!technique_requires_form_anchor(""));
    }

    #[test]
    fn form_anchors_open_true_when_no_anchor_channels_declared() {
        let profile = profile_with_anchors(&[], &["lung"]);
        let meridians = meridians_with(&[], &["lung"]);
        assert!(
            form_anchors_open(&profile, &meridians, None),
            "无 FormAnchor 声明必须视为无门可挡（vacuous truth）"
        );
    }

    #[test]
    fn form_anchors_open_true_when_all_anchors_opened_and_not_severed() {
        let profile = profile_with_anchors(&["ren", "du"], &[]);
        let meridians = meridians_with(&["ren", "du"], &["ren", "du"]);
        assert!(form_anchors_open(&profile, &meridians, None));
    }

    #[test]
    fn form_anchors_open_false_when_one_anchor_not_opened() {
        let profile = profile_with_anchors(&["ren", "du"], &[]);
        let meridians = meridians_with(&["ren"], &["ren", "du"]);
        assert!(
            !form_anchors_open(&profile, &meridians, None),
            "du 未打通必须拒绝，即便 ren 已通"
        );
    }

    #[test]
    fn form_anchors_open_false_when_anchor_channel_missing_from_meridian_system() {
        let profile = profile_with_anchors(&["ren", "du"], &[]);
        // meridians 系统压根没有 du（构型不一致的防御性场景）。
        let meridians = meridians_with(&["ren"], &["ren"]);
        assert!(!form_anchors_open(&profile, &meridians, None));
    }

    #[test]
    fn form_anchors_open_false_when_anchor_severed_even_if_opened_flag_stale_true() {
        let profile = profile_with_anchors(&["ren"], &[]);
        let meridians = meridians_with(&["ren"], &["ren"]);
        let mut severed = MeridianSeveredPermanent::default();
        severed.insert(
            crate::cultivation::components::MeridianChannelId::new("ren"),
            crate::cultivation::meridian::severed::SeveredSource::CombatWound,
            0,
        );
        assert!(
            !form_anchors_open(&profile, &meridians, Some(&severed)),
            "SEVERED 优先于 opened 标志——断了就是断了"
        );
    }

    // ══════════════════════════════════════════════════════════════════════════
    // plan-race-system-v1 P4 opus verifier MAJOR/MEDIUM — cast_morph_yixing /
    // drain_qi_to_zone / release_morph_state 此前零测试，走真实 bevy_ecs::World
    // 驱动（镜像 `dandao::skills::drain_dandao_qi` 既有测试模式）。
    mod cast_and_conservation_tests {
        use super::*;
        use crate::body_plan::race_registry::RaceEntry;
        use crate::body_plan::registry::BodyPlanRegistry;
        use crate::body_plan::types::{
            BodyPartDef, HeightBand, HeightBandAssignment, HitGeometry, PartConsequence,
            StandingAabbSpec,
        };
        use crate::body_plan::HUMAN_RACE_ID;
        use crate::world::dimension::{CurrentDimension, DimensionKind};
        use crate::world::zone::ZoneRegistry;
        use std::collections::HashMap as StdHashMap;
        use valence::prelude::Events;

        fn trivial_plan(id: &str, part_id: &str) -> crate::body_plan::BodyPlan {
            crate::body_plan::BodyPlan {
                id: id.into(),
                display_name: id.to_string(),
                is_humanoid: false,
                parts: vec![BodyPartDef {
                    id: part_id.into(),
                    damage_mul: 1.0,
                    contam_mul: 1.0,
                    bleed_mul: 1.0,
                    consequence: PartConsequence::Core,
                }],
                hit_geometry: HitGeometry::HeightBands {
                    aabb: StandingAabbSpec {
                        half_width: 0.3,
                        height: 1.8,
                    },
                    bands: vec![HeightBand {
                        min_rel_y: -1.0,
                        assignment: HeightBandAssignment::Single {
                            part: part_id.into(),
                        },
                    }],
                    lateral_threshold: 0.19,
                },
                equip_slots: vec![],
                meridian_profile: None,
                mutation_slot_mapping: StdHashMap::new(),
            }
        }

        /// human(from) → whale(to) 正向 morph_pair 的 RaceRegistry——`cast_morph_yixing`
        /// 的 `morph_targets_from(&intrinsic_race)` 消费点用它解析出 target_race。
        fn human_to_whale_registry() -> RaceRegistry {
            let body_plans = BodyPlanRegistry::from_plans(vec![
                trivial_plan("test_human_plan", "chest"),
                trivial_plan("test_whale_plan", "tail_fin"),
            ])
            .expect("test plans must validate");
            RaceRegistry::from_parts_for_test(
                vec![
                    RaceEntry {
                        id: RaceId::new(HUMAN_RACE_ID),
                        display_name: "人族".to_string(),
                        body_plan_id: "test_human_plan".into(),
                        beast_kinds: vec![],
                    },
                    RaceEntry {
                        id: RaceId::new("whale"),
                        display_name: "飞鲸".to_string(),
                        body_plan_id: "test_whale_plan".into(),
                        beast_kinds: vec![],
                    },
                ],
                vec![crate::body_plan::race_registry::MorphPairDef {
                    from: RaceId::new(HUMAN_RACE_ID),
                    to: RaceId::new("whale"),
                    part_mapping: StdHashMap::new(),
                }],
                &body_plans,
            )
            .expect("human->whale morph pair fixture must validate")
        }

        /// 无 morph_pairs 的空 RaceRegistry——`morph_targets_from` 恒返回空。
        fn no_morph_pairs_registry() -> RaceRegistry {
            let body_plans =
                BodyPlanRegistry::from_plans(vec![trivial_plan("test_human_only_plan", "chest")])
                    .expect("test plan must validate");
            RaceRegistry::from_parts_for_test(
                vec![RaceEntry {
                    id: RaceId::new(HUMAN_RACE_ID),
                    display_name: "人族".to_string(),
                    body_plan_id: "test_human_only_plan".into(),
                    beast_kinds: vec![],
                }],
                vec![],
                &body_plans,
            )
            .expect("human-only fixture must validate")
        }

        /// 带 ZoneRegistry + Position（spawn zone 内，[14,66,14]）+ Cultivation 的
        /// world——镜像 `dandao::skills::make_world_with_zone`。
        fn make_world_with_caster_and_zone(
            qi_current: f64,
            qi_max: f64,
            races: RaceRegistry,
        ) -> (bevy_ecs::world::World, Entity) {
            let mut world = bevy_ecs::world::World::new();
            world.init_resource::<Events<QiTransfer>>();
            world.insert_resource(ZoneRegistry::default());
            world.insert_resource(CultivationClock { tick: 1234 });
            world.insert_resource(races);
            let entity = world
                .spawn((
                    Cultivation {
                        qi_current,
                        qi_max,
                        ..Default::default()
                    },
                    Position::new([14.0, 66.0, 14.0]),
                    CurrentDimension(DimensionKind::Overworld),
                ))
                .id();
            (world, entity)
        }

        const YIXING_QI_COST: f64 = 40.0; // 与 known_techniques::TECHNIQUE_DEFINITIONS["morph.yixing"].qi_cost 同步的镜像常量。

        #[test]
        fn cast_succeeds_inserts_morph_state_deducts_qi_and_credits_zone() {
            let races = human_to_whale_registry();
            let (mut world, caster) =
                make_world_with_caster_and_zone(YIXING_QI_COST + 10.0, 100.0, races);
            let initial_zone_spirit_qi = world
                .resource::<ZoneRegistry>()
                .find_zone_by_name("spawn")
                .expect("spawn zone must exist")
                .spirit_qi;

            let result = cast_morph_yixing(&mut world, caster, 0, None);
            assert!(
                matches!(result, CastResult::Started { .. }),
                "余额充足 + 有正向 morph_pair 时应 Started，实际 {result:?}"
            );

            let morph_state = world
                .get::<MorphState>(caster)
                .expect("cast 成功后应插入 MorphState");
            assert_eq!(
                morph_state.form,
                RaceId::new("whale"),
                "form 应为 morph_targets_from 解析出的 whale"
            );

            let cultivation = world.get::<Cultivation>(caster).unwrap();
            assert!(
                (cultivation.qi_current - 10.0).abs() < 1e-9,
                "qi_current 应恰好扣去 qi_cost={YIXING_QI_COST}（10.0+40.0-40.0=10.0），实际 {}",
                cultivation.qi_current
            );

            let final_zone_spirit_qi = world
                .resource::<ZoneRegistry>()
                .find_zone_by_name("spawn")
                .unwrap()
                .spirit_qi;
            assert!(
                final_zone_spirit_qi > initial_zone_spirit_qi,
                "扣除的 qi_cost 必须归还 zone（守恒方向），实测之前={initial_zone_spirit_qi} 之后={final_zone_spirit_qi}"
            );

            let events = world.resource::<Events<QiTransfer>>();
            let mut reader = events.get_reader();
            let transfers: Vec<_> = reader.read(events).collect();
            assert!(!transfers.is_empty(), "应 emit QiTransfer 事件（守恒审计）");
            let total_amount: f64 = transfers.iter().map(|t| t.amount).sum();
            assert!(
                (total_amount - YIXING_QI_COST).abs() < f64::EPSILON,
                "QiTransfer 总金额应恰好等于 qi_cost={YIXING_QI_COST}（不多不少，守恒\
                 金额一次性记账），实际 {total_amount}"
            );
        }

        #[test]
        fn cast_rejects_qi_insufficient_no_state_change() {
            let races = human_to_whale_registry();
            // qi_current < qi_cost=40.0。
            let (mut world, caster) = make_world_with_caster_and_zone(10.0, 100.0, races);
            let initial_zone_spirit_qi = world
                .resource::<ZoneRegistry>()
                .find_zone_by_name("spawn")
                .unwrap()
                .spirit_qi;

            let result = cast_morph_yixing(&mut world, caster, 0, None);
            assert_eq!(
                result,
                CastResult::Rejected {
                    reason: CastRejectReason::QiInsufficient
                },
                "余额不足应 Rejected(QiInsufficient)，实际 {result:?}"
            );

            assert!(
                world.get::<MorphState>(caster).is_none(),
                "余额不足不应插入 MorphState"
            );
            let cultivation = world.get::<Cultivation>(caster).unwrap();
            assert!(
                (cultivation.qi_current - 10.0).abs() < 1e-9,
                "余额不足不应扣减 qi_current（不转账），实际 {}",
                cultivation.qi_current
            );
            let final_zone_spirit_qi = world
                .resource::<ZoneRegistry>()
                .find_zone_by_name("spawn")
                .unwrap()
                .spirit_qi;
            assert_eq!(
                final_zone_spirit_qi, initial_zone_spirit_qi,
                "余额不足不应转账，zone.spirit_qi 应保持不变"
            );
        }

        #[test]
        fn cast_rejects_invalid_target_when_no_morph_pair_no_state_change() {
            let races = no_morph_pairs_registry();
            let (mut world, caster) =
                make_world_with_caster_and_zone(YIXING_QI_COST + 10.0, 100.0, races);

            let result = cast_morph_yixing(&mut world, caster, 0, None);
            assert_eq!(
                result,
                CastResult::Rejected {
                    reason: CastRejectReason::InvalidTarget
                },
                "无正向 morph_pair（生产 races.json 当前恒此分支）应 Rejected(InvalidTarget)，\
                 实际 {result:?}"
            );
            assert!(
                world.get::<MorphState>(caster).is_none(),
                "InvalidTarget 不应插入 MorphState"
            );
            let cultivation = world.get::<Cultivation>(caster).unwrap();
            assert!(
                (cultivation.qi_current - (YIXING_QI_COST + 10.0)).abs() < 1e-9,
                "InvalidTarget 不应扣减 qi_current（在 drain_qi_to_zone 之前就已提前返回），\
                 实际 {}",
                cultivation.qi_current
            );
        }

        #[test]
        fn cast_rejects_invalid_target_when_no_cultivation_component() {
            let mut world = bevy_ecs::world::World::new();
            world.insert_resource(human_to_whale_registry());
            let caster = world.spawn_empty().id();

            let result = cast_morph_yixing(&mut world, caster, 0, None);
            assert_eq!(
                result,
                CastResult::Rejected {
                    reason: CastRejectReason::InvalidTarget
                },
                "无 Cultivation 组件应 Rejected(InvalidTarget)，实际 {result:?}"
            );
            assert!(world.get::<MorphState>(caster).is_none());
        }

        #[test]
        fn cast_toggle_is_idempotent_second_cast_releases() {
            // 幂等切换（决议 §1）：已处于 MorphState 时再次施放 = 解除，不叠加/不 panic。
            // 余额留够两次真实易形消耗（第三次 A→B→A→B 里的第二个 B 分支也要扣费），
            // 只有"解除"分支不扣费。
            let races = human_to_whale_registry();
            let (mut world, caster) =
                make_world_with_caster_and_zone(YIXING_QI_COST * 2.0 + 10.0, 100.0, races);

            let first = cast_morph_yixing(&mut world, caster, 0, None);
            assert!(matches!(first, CastResult::Started { .. }));
            assert!(
                world.get::<MorphState>(caster).is_some(),
                "第一次 cast 后应处于易形态"
            );

            let second = cast_morph_yixing(&mut world, caster, 0, None);
            assert!(
                matches!(second, CastResult::Started { .. }),
                "第二次 cast（解除分支）也应 Started（幂等切换，不是拒绝），实际 {second:?}"
            );
            assert!(
                world.get::<MorphState>(caster).is_none(),
                "第二次 cast 应移除 MorphState（解除），不叠加/不 panic"
            );

            // A→B（未易形→易形）→A（易形→未易形）之后再 A→B 应能重新易形，
            // 证明幂等切换不会把状态锁死在某个分支。
            let third = cast_morph_yixing(&mut world, caster, 0, None);
            assert!(
                matches!(third, CastResult::Started { .. }),
                "解除后应能重新易形（状态机 A→B→A→B 全通），实际 {third:?}"
            );
            assert!(
                world.get::<MorphState>(caster).is_some(),
                "第三次 cast 应重新插入 MorphState"
            );
        }

        #[test]
        fn release_morph_state_is_noop_and_returns_false_when_not_morphed() {
            // 状态转换 A→A（未易形态调用 release）：不 panic，返回 false，不产生副作用。
            let mut world = bevy_ecs::world::World::new();
            let caster = world.spawn(Cultivation::default()).id();

            let released = release_morph_state(&mut world, caster);
            assert!(
                !released,
                "未处于 MorphState 时 release_morph_state 应返回 false（无操作）"
            );
            assert!(world.get::<MorphState>(caster).is_none());
        }

        #[test]
        fn release_morph_state_removes_component_and_returns_true_when_morphed() {
            let mut world = bevy_ecs::world::World::new();
            let caster = world
                .spawn((
                    Cultivation::default(),
                    MorphState::new(RaceId::new("whale"), 0, 0),
                ))
                .id();

            let released = release_morph_state(&mut world, caster);
            assert!(
                released,
                "已处于 MorphState 时 release_morph_state 应返回 true"
            );
            assert!(
                world.get::<MorphState>(caster).is_none(),
                "release 后 MorphState 组件应被 remove"
            );
        }
    }
}
