//! plan-race-system-v1 P5 / PR-6a — `RaceChange` 两阶段事务。
//!
//! `/race set` dev 命令（`cmd::dev::race`）唯一的落地入口。**不允许**任何调用点绕过
//! 本模块直接改 `Cultivation.race` —— 换种族牵涉装备门（`ItemTemplate.wearer_race`）
//! /经脉迁移（`races.json.meridian_mappings`）/ qi_max 重算/守恒（`qi_physics`）四条
//! 相互耦合的战线，必须原子完成。
//!
//! 两阶段设计：
//! - [`precheck_race_change`]：只读，产出 [`RaceChangeCommitPlan`]（或
//!   [`RaceChangeRejection`]）。任一子检查不过 → 整体 `Err` → 调用方不得 mutate
//!   world 任何字段。
//! - [`commit_race_change`]：确定性 apply，消费 `precheck` 产出的 plan——plan 内
//!   携带的 `qi_transfer` 计划已经过 [`crate::qi_physics::prepare_transfer`] 双端
//!   预校验，保证不会因为余额/容量不足而失败。
//!
//! 两阶段必须在**同一 Bevy exclusive system 内、不跨 tick**调用（见
//! `cmd::dev::race::handle_race_cmd`），否则两次调用之间 world 状态可能漂移，
//! precheck 产出的 plan 就不再保证适用。

use std::collections::HashSet;

use valence::prelude::bevy_ecs::world::World;
use valence::prelude::{Entity, Events, Position};

use crate::body_plan::{
    BodyPlanId, BodyPlanRegistry, IntrinsicRace, MeridianProfile, RaceId, RaceRegistry,
};
use crate::cultivation::components::{Cultivation, Meridian, MeridianChannelId, MeridianSystem};
use crate::cultivation::meridian::severed::MeridianSeveredPermanent;
use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
use crate::qi_physics::{
    prepare_transfer, QiAccountId, QiPhysicsError, QiTransfer, QiTransferReason, TransferPlan,
};
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::zone::ZoneRegistry;

/// 阶段一预检失败原因——任一变体命中，`world` 保证零变更（调用方不得 apply）。
#[derive(Debug, Clone, PartialEq)]
pub enum RaceChangeRejection {
    /// 目标种族不存在于 `RaceRegistry`（正常路径下 `/race set` 已在派发前校验过，
    /// 这里是防御性兜底——调用方传入了 registry 校验失败前就该拒绝的 id）。
    UnknownRace(RaceId),
    /// `BodyPlanRegistry` 资源缺失（生产环境恒装载；只应在裸测试环境出现）。
    MissingBodyPlanRegistry,
    /// 目标种族的 `body_plan_id` 在 `BodyPlanRegistry` 中查无——registry 加载期已经
    /// 保证这不可能发生，防御性兜底。
    UnknownBodyPlan(BodyPlanId),
    /// 目标玩家实体没有 `Cultivation` 组件——RaceChange 的前置结构性要求。
    MissingCultivation,
    /// 目标玩家实体没有 `MeridianSystem` 组件——RaceChange 的前置结构性要求。
    MissingMeridianSystem,
    /// `races.json` 声明的 `meridian_mappings` entry 的 from-channel 在这个具体实体
    /// 当前的 `MeridianSystem` 里查无——data/runtime 状态不一致的防御性拒绝（正常
    /// 情况下 `MeridianSystem` 应恰好反映 `Cultivation.race` 的 body_plan profile，
    /// 二者不该分叉）。
    MeridianMappingSourceMissing(MeridianChannelId),
    /// qi 超额释放的双端预校验失败——只可能是实体 `qi_current` 已损坏为非有限值 /
    /// 负值这类不该出现的运行时状态（正常输入下 `prepare_transfer` 恒成功，见其
    /// 文档）。
    QiTransferPrepareFailed(QiPhysicsError),
}

/// qi_current 超过新 `qi_max` 时的释放计划——[`prepare_transfer`] 产出，保证
/// `commit_race_change` apply 不会失败。
#[derive(Debug, Clone, PartialEq)]
pub struct QiExcessReleasePlan {
    /// 需要从 `qi_current` 移除的总量（= `qi_current - new_qi_max`，已在 precheck
    /// 时钳为非负）。
    pub amount: f64,
    /// 命中的 zone 名——`Some` 时 `transfer.accepted` 部分归还该 zone；`None`（玩家
    /// 缺 `Position`/`CurrentDimension`/未知区域）时全部走 overflow 账户（`to_capacity`
    /// 传 `f64::MAX`，`overflow` 恒为 0，`accepted` 记账信息仍然完整可追溯，只是
    /// 落点是 overflow 而非某个具体 zone）。
    pub zone_name: Option<String>,
    /// 双端预校验产出的转账计划。
    pub transfer: TransferPlan,
}

/// 阶段一预检产出的确定性提交计划——`commit_race_change` 只消费本结构体，不重新
/// 计算任何决策分支。
#[derive(Debug, Clone, PartialEq)]
pub struct RaceChangeCommitPlan {
    pub target_race: RaceId,
    pub target_is_humanoid: bool,
    /// 已按 `meridian_mapping`/休眠恢复/全新默认三条路径逐 channel 决议完毕的新
    /// `MeridianSystem`（可以直接整体替换玩家组件）。
    pub new_meridian_system: MeridianSystem,
    /// 旧 `MeridianSystem` 里未被 `meridian_mapping` 用作迁移来源的经脉——`commit`
    /// 阶段原样封存进 `MeridianSeveredPermanent::dormant_meridians`。
    pub newly_dormant: Vec<Meridian>,
    /// 新 `MeridianSystem` 里从休眠登记表原样恢复回来的 channel id 列表——`commit`
    /// 阶段据此从 `dormant_meridians` 里移除对应记录（避免重复占用）。
    pub restored_from_dormant: Vec<MeridianChannelId>,
    pub new_qi_max: f64,
    pub new_qi_current: f64,
    pub qi_excess_release: Option<QiExcessReleasePlan>,
}

/// 阶段一：只读预检，产出确定性提交计划。任一子检查失败 → `Err` → 调用方不得
/// 对 `world` 做任何写入。
pub fn precheck_race_change(
    world: &World,
    player: Entity,
    target: RaceId,
    registry: &RaceRegistry,
) -> Result<RaceChangeCommitPlan, RaceChangeRejection> {
    let Some(cultivation) = world.get::<Cultivation>(player) else {
        return Err(RaceChangeRejection::MissingCultivation);
    };
    let Some(old_meridians) = world.get::<MeridianSystem>(player) else {
        return Err(RaceChangeRejection::MissingMeridianSystem);
    };
    let Some(target_entry) = registry.get(&target) else {
        return Err(RaceChangeRejection::UnknownRace(target));
    };
    let Some(body_plans) = world.get_resource::<BodyPlanRegistry>() else {
        return Err(RaceChangeRejection::MissingBodyPlanRegistry);
    };
    let Some(target_plan) = body_plans.get(&target_entry.body_plan_id) else {
        return Err(RaceChangeRejection::UnknownBodyPlan(
            target_entry.body_plan_id.clone(),
        ));
    };
    let target_is_humanoid = target_plan.is_humanoid;

    // 损坏的 qi_current（非有限值：NaN/±inf）必须尽早拒绝——否则 `NaN.max(0.0)`
    // 在 Rust 里返回 `0.0`、`NaN.min(finite)` 返回 finite，会让"超额释放"分支被
    // 静默跳过、`prepare_transfer` 永不被调用，把损坏值伪装成合法输入带进 commit。
    if !cultivation.qi_current.is_finite() {
        return Err(RaceChangeRejection::QiTransferPrepareFailed(
            QiPhysicsError::InvalidAmount {
                field: "qi_current",
                value: cultivation.qi_current,
            },
        ));
    }

    // 非人形/无经脉构型的目标种族（`meridian_profile: None`）视为空 profile——
    // 迁出方全部经脉走休眠登记，新系统恒空。这是合法的运行时状态（不是失败），
    // 与 `MeridianMappingDef` 加载期校验的"无 profile 时 channel 集合视为空"口径
    // 一致（见 `race_registry::from_file_contents`）。
    let empty_profile = MeridianProfile {
        channels: Vec::new(),
        topology_edges: Vec::new(),
        realm_requirements: Default::default(),
        dugu_injection: Vec::new(),
    };
    let new_profile = target_plan
        .meridian_profile
        .as_ref()
        .unwrap_or(&empty_profile);

    let old_race = cultivation.race.clone();
    let mapping = registry.meridian_mapping(&old_race, &target);
    let severed = world.get::<MeridianSeveredPermanent>(player);

    let mut new_meridian_system = MeridianSystem::for_profile(new_profile);
    let mut restored_from_dormant: Vec<MeridianChannelId> = Vec::new();
    let mut consumed_old_channels: HashSet<MeridianChannelId> = HashSet::new();

    for m in new_meridian_system.iter_mut() {
        let new_id = m.id.clone();
        if let Some(mapping) = mapping {
            if let Some(from_id) = mapping.source_for(&new_id) {
                if !old_meridians.contains(from_id.clone()) {
                    return Err(RaceChangeRejection::MeridianMappingSourceMissing(
                        from_id.clone(),
                    ));
                }
                let mut migrated = old_meridians.get(from_id.clone()).clone();
                migrated.id = new_id.clone();
                *m = migrated;
                consumed_old_channels.insert(from_id.clone());
                continue;
            }
        }
        if let Some(severed) = severed {
            if let Some(dormant) = severed.dormant(new_id.clone()) {
                *m = dormant.clone();
                restored_from_dormant.push(new_id);
            }
        }
    }

    let mut newly_dormant: Vec<Meridian> = Vec::new();
    for old_m in old_meridians.iter() {
        if !consumed_old_channels.contains(&old_m.id) {
            newly_dormant.push(old_m.clone());
        }
    }

    let new_qi_max = 10.0 + new_meridian_system.sum_capacity();
    let excess = (cultivation.qi_current - new_qi_max).max(0.0);
    let new_qi_current = cultivation.qi_current.min(new_qi_max);

    let qi_excess_release = if excess > f64::EPSILON {
        let position = world.get::<Position>(player).map(|p| p.get());
        let dimension = world
            .get::<CurrentDimension>(player)
            .map(|d| d.0)
            .unwrap_or(DimensionKind::Overworld);
        let zones = world.get_resource::<ZoneRegistry>();
        let zone_lookup = position.zip(zones).and_then(|(pos, zones)| {
            zones
                .find_zone(dimension, pos)
                .map(|z| (z.name.clone(), z.spirit_qi))
        });
        let (to_capacity, zone_name) = match zone_lookup {
            Some((name, spirit_qi)) => {
                let zone_current = spirit_qi * QI_ZONE_UNIT_CAPACITY;
                let room = (QI_ZONE_UNIT_CAPACITY - zone_current).max(0.0);
                (room, Some(name))
            }
            // 无 zone 可归还——全部路由到无上限 overflow 账户（`f64::MAX` 容量，
            // `overflow` 恒为 0），与 `death_hooks::release_qi_amount_to_zone` 的
            // 既有 fallback 语义一致。
            None => (f64::MAX, None),
        };
        let transfer = prepare_transfer(cultivation.qi_current, to_capacity, excess)
            .map_err(RaceChangeRejection::QiTransferPrepareFailed)?;
        Some(QiExcessReleasePlan {
            amount: excess,
            zone_name,
            transfer,
        })
    } else {
        None
    };

    Ok(RaceChangeCommitPlan {
        target_race: target,
        target_is_humanoid,
        new_meridian_system,
        newly_dormant,
        restored_from_dormant,
        new_qi_max,
        new_qi_current,
        qi_excess_release,
    })
}

/// 阶段二：确定性提交——消费 `precheck_race_change` 产出的 plan，不重新决策、
/// 无失败注入点。
pub fn commit_race_change(world: &mut World, player: Entity, plan: RaceChangeCommitPlan) {
    let RaceChangeCommitPlan {
        target_race,
        target_is_humanoid,
        new_meridian_system,
        newly_dormant,
        restored_from_dormant,
        new_qi_max,
        new_qi_current,
        qi_excess_release,
    } = plan;

    if let Some(mut cultivation) = world.get_mut::<Cultivation>(player) {
        cultivation.race = target_race.clone();
        cultivation.qi_max = new_qi_max;
        cultivation.qi_current = new_qi_current;
    }

    if let Some(mut meridians) = world.get_mut::<MeridianSystem>(player) {
        *meridians = new_meridian_system;
    }

    if world.get::<MeridianSeveredPermanent>(player).is_none() {
        world
            .entity_mut(player)
            .insert(MeridianSeveredPermanent::default());
    }
    if let Some(mut severed) = world.get_mut::<MeridianSeveredPermanent>(player) {
        for id in restored_from_dormant {
            severed.take_dormant(id);
        }
        for m in newly_dormant {
            severed.register_dormant(m);
        }
    }

    // IntrinsicRace 是本体种族的真源（join 首帧 insert 归 PR-6c）——RaceChange 改
    // `Cultivation.race` 时必须同步 insert/replace，二者不得分叉。
    world
        .entity_mut(player)
        .insert(IntrinsicRace(target_race.clone()));

    if let Some(release) = qi_excess_release {
        apply_qi_excess_release(world, player, release);
    }

    if world.contains_resource::<crate::inventory::ItemRegistry>()
        && world.contains_resource::<crate::inventory::DroppedLootRegistry>()
    {
        relocate_equipment_for_race(world, player, &target_race, target_is_humanoid);
    }

    // 强制下一 tick 立即全量重发 cultivation_detail（而不是等最多 20 tick 的既有
    // 周期节流）——RaceChange 后 client 必须尽快拿到新 race/经脉/qi 快照，不能靠
    // 1s 内的自愈周期兜底（第四时机：真实 RaceChange，见 PR-6a 任务文档）。
    if let Some(mut state) = world
        .get_resource_mut::<crate::network::cultivation_detail_emit::CultivationDetailEmitState>(
    ) {
        state.force_resend_on_next_tick();
    }
}

/// 按 [`QiExcessReleasePlan`] apply——`transfer.accepted` 归还目标 zone（若有），
/// `transfer.overflow` 走无上限 overflow 账户。两段都只 emit `QiTransfer` 审计
/// event（模式照抄 `body_plan::morph::drain_qi_to_zone` /
/// `cultivation::death_hooks::release_qi_amount_to_zone`——玩家真元活在 ECS
/// `Cultivation.qi_current`，此处不镜像到 `WorldQiAccount` balance，纯审计轨迹）。
fn apply_qi_excess_release(world: &mut World, player: Entity, release: QiExcessReleasePlan) {
    let from = QiAccountId::player(format!("entity:{}", player.to_bits()));

    if let Some(zone_name) = release.zone_name.as_deref() {
        if let Some(mut zones) = world.get_resource_mut::<ZoneRegistry>() {
            if let Some(zone) = zones.find_zone_mut(zone_name) {
                let to = QiAccountId::zone(zone.name.clone());
                let zone_current = zone.spirit_qi * QI_ZONE_UNIT_CAPACITY;
                zone.spirit_qi = ((zone_current + release.transfer.accepted)
                    / QI_ZONE_UNIT_CAPACITY)
                    .clamp(-1.0, 1.0);
                if release.transfer.accepted > f64::EPSILON {
                    if let Ok(transfer) = QiTransfer::new(
                        from.clone(),
                        to,
                        release.transfer.accepted,
                        QiTransferReason::ReleaseToZone,
                    ) {
                        if let Some(mut events) = world.get_resource_mut::<Events<QiTransfer>>() {
                            events.send(transfer);
                        }
                    }
                }
            }
        }
    }

    if release.transfer.overflow > f64::EPSILON {
        let overflow_to =
            QiAccountId::overflow(format!("race_change_overflow:{}", player.to_bits()));
        if let Ok(transfer) = QiTransfer::new(
            from,
            overflow_to,
            release.transfer.overflow,
            QiTransferReason::ReleaseToZone,
        ) {
            if let Some(mut events) = world.get_resource_mut::<Events<QiTransfer>>() {
                events.send(transfer);
            }
        }
    }
}

/// 装备门重校验——RaceChange 提交后本体身份已经切到新种族，复用 P4 易形解除的
/// 既有 infallible 逻辑（`enforce_intrinsic_gate_on_morph_release`：遍历装备槽，
/// `wearer_race` 不再放行的物件优先塞进背包，背包放不下才转地面掉落，从不静默
/// 销毁）。资源缺失（大量既有单测未插入 `ItemRegistry`/`DroppedLootRegistry`）时
/// 调用方已在外层 `contains_resource` 短路跳过，本函数内不再重复判断。
fn relocate_equipment_for_race(
    world: &mut World,
    player: Entity,
    target_race: &RaceId,
    target_is_humanoid: bool,
) {
    let player_pos = world
        .get::<Position>(player)
        .map(|p| p.get())
        .unwrap_or([0.0, 0.0, 0.0].into());
    let player_dimension = world
        .get::<CurrentDimension>(player)
        .map(|d| d.0)
        .unwrap_or(DimensionKind::Overworld);

    let (stashed, dropped) =
        world.resource_scope(
            |world,
             item_registry: valence::prelude::bevy_ecs::world::Mut<
                crate::inventory::ItemRegistry,
            >| {
                world.resource_scope(
                    |world,
                     mut dropped_registry: valence::prelude::bevy_ecs::world::Mut<
                        crate::inventory::DroppedLootRegistry,
                    >| {
                        let Some(mut inventory) =
                            world.get_mut::<crate::inventory::PlayerInventory>(player)
                        else {
                            return (Vec::new(), Vec::new());
                        };
                        crate::inventory::enforce_intrinsic_gate_on_morph_release(
                            &mut inventory,
                            &item_registry,
                            &mut dropped_registry,
                            target_race,
                            target_is_humanoid,
                            [player_pos.x, player_pos.y, player_pos.z],
                            player_dimension,
                        )
                    },
                )
            },
        );
    if !stashed.is_empty() || !dropped.is_empty() {
        tracing::info!(
            "[bong][cultivation][race_change] relocate_equipment_for_race entity={player:?} \
             target_race={target_race} stashed={stashed:?} dropped={dropped:?}"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body_plan::race_registry::{MeridianMappingDef, RaceEntry};
    use crate::body_plan::registry::BodyPlanRegistry;
    use crate::body_plan::types::{
        BodyPartDef, ChannelDef, HeightBand, HeightBandAssignment, HitGeometry, MeridianFamily,
        PartConsequence, RealmMeridianReq, StandingAabbSpec,
    };
    use crate::body_plan::HUMAN_RACE_ID;
    use crate::world::dimension::{CurrentDimension, DimensionKind};
    use valence::prelude::App;

    fn trivial_plan_with_channels(id: &str, channel_ids: &[&str]) -> crate::body_plan::BodyPlan {
        crate::body_plan::BodyPlan {
            id: BodyPlanId::new(id),
            display_name: id.to_string(),
            is_humanoid: false,
            parts: vec![BodyPartDef {
                id: "core".into(),
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
                        part: "core".into(),
                    },
                }],
                lateral_threshold: 0.19,
            },
            equip_slots: vec![],
            meridian_profile: Some(MeridianProfile {
                channels: channel_ids
                    .iter()
                    .map(|cid| ChannelDef {
                        id: MeridianChannelId::new(*cid),
                        family: MeridianFamily::Regular,
                        body_part: None,
                        roles: vec![],
                    })
                    .collect(),
                topology_edges: vec![],
                realm_requirements: [RealmMeridianReq::default(); 6],
                dugu_injection: vec![],
            }),
            mutation_slot_mapping: Default::default(),
        }
    }

    fn trivial_plan_without_meridian_profile(id: &str) -> crate::body_plan::BodyPlan {
        crate::body_plan::BodyPlan {
            id: BodyPlanId::new(id),
            display_name: id.to_string(),
            is_humanoid: false,
            parts: vec![BodyPartDef {
                id: "core".into(),
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
                        part: "core".into(),
                    },
                }],
                lateral_threshold: 0.19,
            },
            equip_slots: vec![],
            meridian_profile: None,
            mutation_slot_mapping: Default::default(),
        }
    }

    /// human(lung, heart) --[lung->fin]--> whale(fin, tail) 注册表 fixture。
    /// `heart` 未被映射覆盖 → 期望进休眠登记；`tail` 无来源 → 期望全新默认状态。
    fn human_whale_registry_with_partial_mapping() -> (BodyPlanRegistry, RaceRegistry) {
        let body_plans = BodyPlanRegistry::from_plans(vec![
            trivial_plan_with_channels("humanoid", &["lung", "heart"]),
            trivial_plan_with_channels("whale", &["fin", "tail"]),
        ])
        .expect("fixture plans should validate");
        let races = RaceRegistry::from_parts_for_test_with_meridian_mappings(
            vec![
                RaceEntry {
                    id: RaceId::new(HUMAN_RACE_ID),
                    display_name: "人族".to_string(),
                    body_plan_id: BodyPlanId::new("humanoid"),
                    beast_kinds: vec![],
                },
                RaceEntry {
                    id: RaceId::new("whale"),
                    display_name: "飞鲸".to_string(),
                    body_plan_id: BodyPlanId::new("whale"),
                    beast_kinds: vec![],
                },
            ],
            vec![],
            vec![MeridianMappingDef {
                from: RaceId::new(HUMAN_RACE_ID),
                to: RaceId::new("whale"),
                entries: vec![(
                    MeridianChannelId::new("lung"),
                    MeridianChannelId::new("fin"),
                )],
            }],
            &body_plans,
        )
        .expect("fixture registry should load");
        (body_plans, races)
    }

    fn spawn_human_player(app: &mut App, qi_current: f64, qi_max: f64) -> Entity {
        let cultivation = Cultivation {
            qi_current,
            qi_max,
            race: RaceId::new(HUMAN_RACE_ID),
            ..Default::default()
        };
        let mut meridians = MeridianSystem {
            regular: vec![
                Meridian::new(MeridianChannelId::new("lung")),
                Meridian::new(MeridianChannelId::new("heart")),
            ],
            extraordinary: vec![],
        };
        meridians.get_mut(MeridianChannelId::new("lung")).opened = true;
        meridians.get_mut(MeridianChannelId::new("lung")).integrity = 0.8;
        app.world_mut()
            .spawn((cultivation, meridians, MeridianSeveredPermanent::default()))
            .id()
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 阶段一预检：happy path — 映射迁移 + 休眠 + 新鲜默认。
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn precheck_happy_path_migrates_mapped_dormants_unmapped_and_defaults_fresh() {
        let (body_plans, races) = human_whale_registry_with_partial_mapping();
        let mut app = App::new();
        app.insert_resource(body_plans);
        let player = spawn_human_player(&mut app, 5.0, 100.0);

        let plan = precheck_race_change(app.world(), player, RaceId::new("whale"), &races)
            .expect("happy path precheck must succeed");

        assert_eq!(plan.target_race, RaceId::new("whale"));
        assert!(!plan.target_is_humanoid);

        let fin = plan.new_meridian_system.get(MeridianChannelId::new("fin"));
        assert!(fin.opened, "lung(opened) 映射到 fin，状态应原样迁移");
        assert_eq!(fin.integrity, 0.8);

        let tail = plan.new_meridian_system.get(MeridianChannelId::new("tail"));
        assert!(!tail.opened, "tail 无映射来源，应是全新默认（未打通）");

        assert_eq!(plan.newly_dormant.len(), 1);
        assert_eq!(plan.newly_dormant[0].id, MeridianChannelId::new("heart"));
        assert!(plan.restored_from_dormant.is_empty());
    }

    #[test]
    fn precheck_restores_previously_dormant_channel_matching_new_profile() {
        let (body_plans, races) = human_whale_registry_with_partial_mapping();
        let mut app = App::new();
        app.insert_resource(body_plans);
        let player = spawn_human_player(&mut app, 5.0, 100.0);

        // 预先在休眠登记里放一条 "tail"（模拟这个玩家很早以前是 whale、又换回
        // human 时把 tail 挂了休眠——现在换回 whale，tail 应该被原样恢复）。
        let mut dormant_tail = Meridian::new(MeridianChannelId::new("tail"));
        dormant_tail.opened = true;
        dormant_tail.integrity = 0.55;
        app.world_mut()
            .get_mut::<MeridianSeveredPermanent>(player)
            .unwrap()
            .register_dormant(dormant_tail.clone());

        let plan = precheck_race_change(app.world(), player, RaceId::new("whale"), &races)
            .expect("precheck should succeed");

        let restored_tail = plan.new_meridian_system.get(MeridianChannelId::new("tail"));
        assert_eq!(restored_tail, &dormant_tail);
        assert_eq!(
            plan.restored_from_dormant,
            vec![MeridianChannelId::new("tail")]
        );
    }

    #[test]
    fn precheck_no_qi_excess_when_new_qi_max_covers_current() {
        let (body_plans, races) = human_whale_registry_with_partial_mapping();
        let mut app = App::new();
        app.insert_resource(body_plans);
        let player = spawn_human_player(&mut app, 5.0, 1000.0);

        let plan = precheck_race_change(app.world(), player, RaceId::new("whale"), &races)
            .expect("precheck should succeed");
        assert!(plan.qi_excess_release.is_none());
        assert_eq!(plan.new_qi_current, 5.0);
    }

    #[test]
    fn precheck_computes_qi_excess_release_plan_when_new_qi_max_shrinks() {
        let (body_plans, races) = human_whale_registry_with_partial_mapping();
        let mut app = App::new();
        app.insert_resource(body_plans);
        // qi_current 远超新构型 qi_max（新构型只有 1 条打通经脉 fin，容量小）。
        let player = spawn_human_player(&mut app, 500.0, 500.0);

        let plan = precheck_race_change(app.world(), player, RaceId::new("whale"), &races)
            .expect("precheck should succeed");
        let release = plan
            .qi_excess_release
            .as_ref()
            .expect("qi_current far exceeds new qi_max, excess release plan expected");
        assert_eq!(
            release.zone_name, None,
            "无 Position/Zone，应路由到 overflow"
        );
        assert_eq!(release.transfer.overflow, 0.0, "overflow 账户容量无上限");
        assert!(plan.new_qi_current <= plan.new_qi_max + f64::EPSILON);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 阶段一预检：注入失败 —— 各断言 world 零变更。
    // ─────────────────────────────────────────────────────────────────────────

    fn snapshot(
        app: &App,
        player: Entity,
    ) -> (Cultivation, MeridianSystem, MeridianSeveredPermanent) {
        (
            app.world().get::<Cultivation>(player).unwrap().clone(),
            app.world().get::<MeridianSystem>(player).unwrap().clone(),
            app.world()
                .get::<MeridianSeveredPermanent>(player)
                .unwrap()
                .clone(),
        )
    }

    #[test]
    fn precheck_rejects_unknown_target_race_and_world_is_unchanged() {
        let (body_plans, races) = human_whale_registry_with_partial_mapping();
        let mut app = App::new();
        app.insert_resource(body_plans);
        let player = spawn_human_player(&mut app, 5.0, 100.0);
        let before = snapshot(&app, player);

        let err = precheck_race_change(app.world(), player, RaceId::new("no_such_race"), &races)
            .expect_err("unknown race must reject");
        assert_eq!(
            err,
            RaceChangeRejection::UnknownRace(RaceId::new("no_such_race"))
        );
        assert_eq!(
            snapshot(&app, player),
            before,
            "world must be byte-for-byte unchanged"
        );
    }

    #[test]
    fn precheck_rejects_missing_cultivation_and_world_is_unchanged() {
        let (body_plans, races) = human_whale_registry_with_partial_mapping();
        let mut app = App::new();
        app.insert_resource(body_plans);
        let player = app.world_mut().spawn(()).id();

        let err = precheck_race_change(app.world(), player, RaceId::new("whale"), &races)
            .expect_err("missing Cultivation must reject");
        assert_eq!(err, RaceChangeRejection::MissingCultivation);
        assert!(app.world().get::<Cultivation>(player).is_none());
        assert!(app.world().get::<MeridianSystem>(player).is_none());
    }

    #[test]
    fn precheck_rejects_missing_meridian_system_and_world_is_unchanged() {
        let (body_plans, races) = human_whale_registry_with_partial_mapping();
        let mut app = App::new();
        app.insert_resource(body_plans);
        let cultivation = Cultivation {
            race: RaceId::new(HUMAN_RACE_ID),
            ..Default::default()
        };
        let player = app.world_mut().spawn(cultivation.clone()).id();

        let err = precheck_race_change(app.world(), player, RaceId::new("whale"), &races)
            .expect_err("missing MeridianSystem must reject");
        assert_eq!(err, RaceChangeRejection::MissingMeridianSystem);
        assert_eq!(
            app.world().get::<Cultivation>(player).unwrap().race,
            cultivation.race,
            "world must be unchanged"
        );
    }

    #[test]
    fn precheck_rejects_meridian_mapping_source_missing_from_entity_system_and_world_unchanged() {
        // 数据/运行时状态分叉的防御场景：mapping 声明 from=lung，但这个具体实体的
        // MeridianSystem 里根本没有 lung（构造一个跟 registry 声明不一致的实体）。
        let (body_plans, races) = human_whale_registry_with_partial_mapping();
        let mut app = App::new();
        app.insert_resource(body_plans);
        let cultivation = Cultivation {
            race: RaceId::new(HUMAN_RACE_ID),
            ..Default::default()
        };
        let meridians = MeridianSystem {
            regular: vec![Meridian::new(MeridianChannelId::new("heart"))],
            extraordinary: vec![],
        };
        let player = app
            .world_mut()
            .spawn((cultivation, meridians, MeridianSeveredPermanent::default()))
            .id();
        let before = snapshot(&app, player);

        let err = precheck_race_change(app.world(), player, RaceId::new("whale"), &races)
            .expect_err("dangling mapping source in entity's own MeridianSystem must reject");
        assert_eq!(
            err,
            RaceChangeRejection::MeridianMappingSourceMissing(MeridianChannelId::new("lung"))
        );
        assert_eq!(snapshot(&app, player), before);
    }

    #[test]
    fn precheck_rejects_corrupted_qi_current_nan_and_world_unchanged() {
        let (body_plans, races) = human_whale_registry_with_partial_mapping();
        let mut app = App::new();
        app.insert_resource(body_plans);
        let player = spawn_human_player(&mut app, f64::NAN, 100.0);

        let err = precheck_race_change(app.world(), player, RaceId::new("whale"), &races)
            .expect_err("NaN qi_current must fail prepare_transfer and reject overall");
        assert!(matches!(
            err,
            RaceChangeRejection::QiTransferPrepareFailed(_)
        ));
        // 不能用 snapshot 整体 PartialEq 对拍——qi_current 是 NaN，NaN != NaN 会让
        // "零变更"断言恒假。改为逐字段断言：precheck 只读，race/qi_max/经脉必须原值，
        // qi_current 仍是那颗 NaN（未被 commit 触碰）。
        let cultivation = app.world().get::<Cultivation>(player).unwrap();
        assert_eq!(cultivation.race, RaceId::new(HUMAN_RACE_ID));
        assert_eq!(cultivation.qi_max, 100.0);
        assert!(cultivation.qi_current.is_nan(), "qi_current 应仍是未被触碰的 NaN");
        assert!(app
            .world()
            .get::<MeridianSystem>(player)
            .unwrap()
            .contains(MeridianChannelId::new("lung")));
        // precheck 不得写 IntrinsicRace（那是 commit 的职责）。
        assert!(app.world().get::<IntrinsicRace>(player).is_none());
    }

    #[test]
    fn precheck_rejects_missing_body_plan_registry_and_world_unchanged() {
        let (_body_plans, races) = human_whale_registry_with_partial_mapping();
        let mut app = App::new(); // 故意不插入 BodyPlanRegistry
        let player = spawn_human_player(&mut app, 5.0, 100.0);
        let before = snapshot(&app, player);

        let err = precheck_race_change(app.world(), player, RaceId::new("whale"), &races)
            .expect_err("missing BodyPlanRegistry must reject");
        assert_eq!(err, RaceChangeRejection::MissingBodyPlanRegistry);
        assert_eq!(snapshot(&app, player), before);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 阶段二提交：全量应用 + 守恒不变量。
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn commit_applies_race_meridian_and_intrinsic_race() {
        let (body_plans, races) = human_whale_registry_with_partial_mapping();
        let mut app = App::new();
        app.insert_resource(body_plans);
        app.add_event::<QiTransfer>();
        let player = spawn_human_player(&mut app, 5.0, 100.0);

        let plan = precheck_race_change(app.world(), player, RaceId::new("whale"), &races)
            .expect("precheck should succeed");
        commit_race_change(app.world_mut(), player, plan);

        let cultivation = app.world().get::<Cultivation>(player).unwrap();
        assert_eq!(cultivation.race, RaceId::new("whale"));
        let meridians = app.world().get::<MeridianSystem>(player).unwrap();
        assert!(meridians.contains(MeridianChannelId::new("fin")));
        assert!(meridians.get(MeridianChannelId::new("fin")).opened);
        let intrinsic = app.world().get::<IntrinsicRace>(player).unwrap();
        assert_eq!(intrinsic.0, RaceId::new("whale"));
        let severed = app.world().get::<MeridianSeveredPermanent>(player).unwrap();
        assert!(severed.is_dormant(MeridianChannelId::new("heart")));
    }

    #[test]
    fn commit_restores_dormant_channel_and_clears_its_registry_entry() {
        let (body_plans, races) = human_whale_registry_with_partial_mapping();
        let mut app = App::new();
        app.insert_resource(body_plans);
        app.add_event::<QiTransfer>();
        let player = spawn_human_player(&mut app, 5.0, 100.0);
        let mut dormant_tail = Meridian::new(MeridianChannelId::new("tail"));
        dormant_tail.opened = true;
        app.world_mut()
            .get_mut::<MeridianSeveredPermanent>(player)
            .unwrap()
            .register_dormant(dormant_tail);

        let plan = precheck_race_change(app.world(), player, RaceId::new("whale"), &races)
            .expect("precheck should succeed");
        commit_race_change(app.world_mut(), player, plan);

        let severed = app.world().get::<MeridianSeveredPermanent>(player).unwrap();
        assert!(
            !severed.is_dormant(MeridianChannelId::new("tail")),
            "恢复后应从休眠登记移除，不留残留"
        );
        let meridians = app.world().get::<MeridianSystem>(player).unwrap();
        assert!(meridians.get(MeridianChannelId::new("tail")).opened);
    }

    #[test]
    fn commit_conserves_qi_current_plus_qi_max_delta_against_zone_when_excess_released() {
        // 守恒不变量：qi_current 减少量必须精确等于 zone.spirit_qi 增加量
        // （换算 QI_ZONE_UNIT_CAPACITY），overflow 为 0 时全额落 zone。
        let (body_plans, races) = human_whale_registry_with_partial_mapping();
        let mut app = App::new();
        app.insert_resource(body_plans);
        app.add_event::<QiTransfer>();
        let player = spawn_human_player(&mut app, 30.0, 1000.0);
        app.world_mut().entity_mut(player).insert((
            Position::new([0.0, 64.0, 0.0]),
            CurrentDimension(DimensionKind::Overworld),
        ));
        // `ZoneRegistry::default()` = `fallback()`：单个 spawn zone，bounds 覆盖
        // [0,64,0]（见 `world::zone::default_spawn_bounds`）。把 spirit_qi 压到 0.0，
        // 保证 zone 剩余容量（50）足以完整吸收本次 excess（无 overflow 分流），这样
        // "qi_current 减少量 == zone 增加量"的守恒断言不被 overflow 拆分干扰。
        let mut zones = ZoneRegistry::default();
        zones.zones[0].spirit_qi = 0.0;
        app.insert_resource(zones);

        let plan = precheck_race_change(app.world(), player, RaceId::new("whale"), &races)
            .expect("precheck should succeed");
        let excess = plan
            .qi_excess_release
            .as_ref()
            .expect("qi_current(30) vastly exceeds new qi_max (only fin channel opened)")
            .amount;
        let qi_current_before = app.world().get::<Cultivation>(player).unwrap().qi_current;
        let zone_qi_before = app
            .world()
            .get_resource::<ZoneRegistry>()
            .unwrap()
            .zones
            .first()
            .unwrap()
            .spirit_qi;

        commit_race_change(app.world_mut(), player, plan);

        let qi_current_after = app.world().get::<Cultivation>(player).unwrap().qi_current;
        let zone_qi_after = app
            .world()
            .get_resource::<ZoneRegistry>()
            .unwrap()
            .zones
            .first()
            .unwrap()
            .spirit_qi;
        let qi_current_delta = qi_current_before - qi_current_after;
        let zone_qi_delta = (zone_qi_after - zone_qi_before) * QI_ZONE_UNIT_CAPACITY;
        assert!(
            (qi_current_delta - excess).abs() < 1e-9,
            "qi_current 减少量必须等于 precheck 计算的 excess"
        );
        assert!(
            (zone_qi_delta - excess).abs() < 1e-6,
            "zone 增加量必须等于 qi_current 减少量（守恒），实际 delta={zone_qi_delta} excess={excess}"
        );
    }

    #[test]
    fn commit_inserts_meridian_severed_permanent_when_missing() {
        let (body_plans, races) = human_whale_registry_with_partial_mapping();
        let mut app = App::new();
        app.insert_resource(body_plans);
        app.add_event::<QiTransfer>();
        let cultivation = Cultivation {
            qi_current: 5.0,
            qi_max: 100.0,
            race: RaceId::new(HUMAN_RACE_ID),
            ..Default::default()
        };
        // lung 映射到 whale 的 fin；heart 无映射 → 期望进休眠登记（证明 commit 在
        // MeridianSeveredPermanent 缺失时会自己补插入并写入休眠记录）。
        let mut meridians = MeridianSystem {
            regular: vec![
                Meridian::new(MeridianChannelId::new("lung")),
                Meridian::new(MeridianChannelId::new("heart")),
            ],
            extraordinary: vec![],
        };
        meridians.get_mut(MeridianChannelId::new("lung")).opened = true;
        // 故意不插入 MeridianSeveredPermanent —— commit 必须自己补插入。
        let player = app.world_mut().spawn((cultivation, meridians)).id();

        let plan = precheck_race_change(app.world(), player, RaceId::new("whale"), &races)
            .expect("precheck should succeed even without MeridianSeveredPermanent present");
        commit_race_change(app.world_mut(), player, plan);

        let severed = app
            .world()
            .get::<MeridianSeveredPermanent>(player)
            .expect("commit must insert MeridianSeveredPermanent if missing");
        assert!(severed.is_dormant(MeridianChannelId::new("heart")));
    }

    #[test]
    fn precheck_treats_target_body_plan_without_meridian_profile_as_all_dormant() {
        let mut body_plans_vec = vec![
            trivial_plan_with_channels("humanoid", &["lung", "heart"]),
            trivial_plan_without_meridian_profile("mindless"),
        ];
        let body_plans = BodyPlanRegistry::from_plans(std::mem::take(&mut body_plans_vec)).unwrap();
        let races = RaceRegistry::from_parts_for_test_with_meridian_mappings(
            vec![
                RaceEntry {
                    id: RaceId::new(HUMAN_RACE_ID),
                    display_name: "人族".to_string(),
                    body_plan_id: BodyPlanId::new("humanoid"),
                    beast_kinds: vec![],
                },
                RaceEntry {
                    id: RaceId::new("mindless"),
                    display_name: "无脉兽".to_string(),
                    body_plan_id: BodyPlanId::new("mindless"),
                    beast_kinds: vec![],
                },
            ],
            vec![],
            vec![],
            &body_plans,
        )
        .expect("fixture registry should load");

        let mut app = App::new();
        app.insert_resource(body_plans);
        let player = spawn_human_player(&mut app, 5.0, 100.0);

        let plan = precheck_race_change(app.world(), player, RaceId::new("mindless"), &races)
            .expect("target without meridian_profile must still succeed (all-dormant path)");
        assert_eq!(
            plan.new_meridian_system.regular.len() + plan.new_meridian_system.extraordinary.len(),
            0
        );
        assert_eq!(plan.newly_dormant.len(), 2, "两条经脉都应全部进休眠登记");
    }
}
