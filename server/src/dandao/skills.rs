//! 丹道三基础招式 resolver。
//!
//! 遵循 SkillFn 签名: fn(&mut World, Entity, u8, Option<Entity>) -> CastResult。
//! 每个招式在 cast 时检查：
//! 1. 境界 gate（服丹急行=醒灵, 投丹=引气, 丹雾=凝脉）
//! 2. 经脉依赖（check_meridian_dependencies）
//! 3. 真元是否足够（§8.1 #4: capacity_for_tier(realm) × 3%，不硬编绝对数值）
//! 4. 冷却

use valence::prelude::{bevy_ecs, Entity, Events};

use crate::cultivation::components::{Cultivation, Meridian, MeridianId, Realm};
use crate::cultivation::meridian::severed::{
    check_meridian_dependencies, MeridianSeveredPermanent,
};
use crate::cultivation::skill_registry::{CastRejectReason, CastResult};
use crate::qi_physics::ledger::{QiAccountId, QiTransfer, QiTransferReason};

pub const DANDAO_PILL_RUSH_SKILL_ID: &str = "dandao.pill_rush";
pub const DANDAO_PILL_BOMB_SKILL_ID: &str = "dandao.pill_bomb";
pub const DANDAO_PILL_MIST_SKILL_ID: &str = "dandao.pill_mist";

/// 所有丹道招式通用 qi 消耗比例（§8.1 #4: capacity_for_tier(realm) × 此比例）。
const DANDAO_QI_RATIO: f64 = 0.03;
/// 投丹额外消耗系数（在 DANDAO_QI_RATIO 基础上乘此值）。
const PILL_BOMB_QI_MULTIPLIER: f64 = 1.5;
/// 投丹基础冷却 (ticks, 20tps × 8s = 160)
const PILL_BOMB_COOLDOWN_TICKS: u64 = 160;
/// 丹雾基础消耗（固定 10 qi，不走 capacity_for_tier 比例）。
const PILL_MIST_QI_COST: f64 = 10.0;
/// 丹雾冷却 (20tps × 30s = 600)
const PILL_MIST_COOLDOWN_TICKS: u64 = 600;
/// 服丹急行冷却 (20tps × 15s = 300)
const PILL_RUSH_COOLDOWN_TICKS: u64 = 300;

// 经脉依赖（plan §1.4）
const PILL_RUSH_MERIDIANS: &[MeridianId] = &[MeridianId::Spleen, MeridianId::Kidney];
const PILL_BOMB_MERIDIANS: &[MeridianId] = &[MeridianId::Lung, MeridianId::Spleen];
const PILL_MIST_MERIDIANS: &[MeridianId] = &[MeridianId::Spleen, MeridianId::Liver];

/// §8.1 #4: 用 capacity_for_tier(realm) 算 qi 消耗基数，不用 qi_max。
/// realm 对应 tier 映射（醒灵=0, 引气=1, ... 化虚=5）。
fn realm_to_tier(realm: Realm) -> u8 {
    match realm {
        Realm::Awaken => 0,
        Realm::Induce => 1,
        Realm::Condense => 2,
        Realm::Solidify => 3,
        Realm::Spirit => 4,
        Realm::Void => 5,
    }
}

/// 计算丹道招式 qi 消耗基数。
pub fn dandao_qi_cost_base(realm: Realm) -> f64 {
    let tier = realm_to_tier(realm);
    Meridian::capacity_for_tier(tier) * DANDAO_QI_RATIO
}

/// 实际扣除 qi 并 emit QiTransfer（player → zone 守恒）。
/// 返回 true 表示扣除成功。
fn drain_dandao_qi(world: &mut bevy_ecs::world::World, caster: Entity, cost: f64) -> bool {
    if cost <= 0.0 {
        return true;
    }
    let Some(mut cultivation) = world.get_mut::<Cultivation>(caster) else {
        return false;
    };
    if cultivation.qi_current + f64::EPSILON < cost {
        return false;
    }
    cultivation.qi_current = (cultivation.qi_current - cost).clamp(0.0, cultivation.qi_max);

    // Emit QiTransfer for ledger audit (player → zone, ReleaseToZone reason).
    if let Some(mut events) = world.get_resource_mut::<Events<QiTransfer>>() {
        if let Ok(transfer) = QiTransfer::new(
            QiAccountId::player(format!("entity:{caster:?}")),
            QiAccountId::zone("current_zone".to_string()),
            cost,
            QiTransferReason::ReleaseToZone,
        ) {
            events.send(transfer);
        }
    }
    true
}

/// 招式一：服丹急行 — 自服战斗丹（零距离，buff 自身）
/// 境界要求：醒灵+
/// 消耗：capacity_for_tier(realm) × 3%
pub fn resolve_pill_rush(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    _slot: u8,
    _target: Option<Entity>,
) -> CastResult {
    let Some(cultivation) = world.get::<Cultivation>(caster) else {
        return CastResult::Rejected {
            reason: CastRejectReason::RealmTooLow,
        };
    };

    if (cultivation.realm as u8) < (Realm::Awaken as u8) {
        return CastResult::Rejected {
            reason: CastRejectReason::RealmTooLow,
        };
    }

    let severed = world.get::<MeridianSeveredPermanent>(caster);
    if let Err(mid) = check_meridian_dependencies(PILL_RUSH_MERIDIANS, severed) {
        return CastResult::Rejected {
            reason: CastRejectReason::MeridianSevered(Some(mid)),
        };
    }

    let qi_cost = dandao_qi_cost_base(cultivation.realm);
    if cultivation.qi_current < qi_cost {
        return CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient,
        };
    }

    // Actual qi deduction (守恒: player → zone via QiTransfer)
    drain_dandao_qi(world, caster, qi_cost);

    CastResult::Started {
        cooldown_ticks: PILL_RUSH_COOLDOWN_TICKS,
        anim_duration_ticks: 10,
    }
}

/// 招式二：投丹 — 投掷丹药弹（5-15 格中距离）
/// 境界要求：引气+
/// 消耗：capacity_for_tier(realm) × 3% × 1.5
pub fn resolve_pill_bomb(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    _slot: u8,
    _target: Option<Entity>,
) -> CastResult {
    let Some(cultivation) = world.get::<Cultivation>(caster) else {
        return CastResult::Rejected {
            reason: CastRejectReason::RealmTooLow,
        };
    };

    if (cultivation.realm as u8) < (Realm::Induce as u8) {
        return CastResult::Rejected {
            reason: CastRejectReason::RealmTooLow,
        };
    }

    let severed = world.get::<MeridianSeveredPermanent>(caster);
    if let Err(mid) = check_meridian_dependencies(PILL_BOMB_MERIDIANS, severed) {
        return CastResult::Rejected {
            reason: CastRejectReason::MeridianSevered(Some(mid)),
        };
    }

    let qi_cost = dandao_qi_cost_base(cultivation.realm) * PILL_BOMB_QI_MULTIPLIER;
    if cultivation.qi_current < qi_cost {
        return CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient,
        };
    }

    // Actual qi deduction (守恒: player → zone via QiTransfer)
    drain_dandao_qi(world, caster, qi_cost);

    CastResult::Started {
        cooldown_ticks: PILL_BOMB_COOLDOWN_TICKS,
        anim_duration_ticks: 12,
    }
}

/// 招式三：丹雾 — 丹药蒸发（0-5 格近距离，持续 AoE）
/// 境界要求：凝脉+
/// 消耗：10 qi + 0.5 qi/s 持续
pub fn resolve_pill_mist(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    _slot: u8,
    _target: Option<Entity>,
) -> CastResult {
    let Some(cultivation) = world.get::<Cultivation>(caster) else {
        return CastResult::Rejected {
            reason: CastRejectReason::RealmTooLow,
        };
    };

    if (cultivation.realm as u8) < (Realm::Condense as u8) {
        return CastResult::Rejected {
            reason: CastRejectReason::RealmTooLow,
        };
    }

    let severed = world.get::<MeridianSeveredPermanent>(caster);
    if let Err(mid) = check_meridian_dependencies(PILL_MIST_MERIDIANS, severed) {
        return CastResult::Rejected {
            reason: CastRejectReason::MeridianSevered(Some(mid)),
        };
    }

    if cultivation.qi_current < PILL_MIST_QI_COST {
        return CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient,
        };
    }

    // Actual qi deduction (守恒: player → zone via QiTransfer)
    drain_dandao_qi(world, caster, PILL_MIST_QI_COST);

    CastResult::Started {
        cooldown_ticks: PILL_MIST_COOLDOWN_TICKS,
        anim_duration_ticks: 16,
    }
}

#[cfg(test)]
mod skill_tests {
    use super::*;
    use crate::cultivation::components::Cultivation;

    fn make_world_with_caster(
        realm: Realm,
        qi_current: f64,
        qi_max: f64,
    ) -> (bevy_ecs::world::World, Entity) {
        let mut world = bevy_ecs::world::World::new();
        world.init_resource::<Events<QiTransfer>>();
        let entity = world
            .spawn(Cultivation {
                realm,
                qi_current,
                qi_max,
                ..Default::default()
            })
            .id();
        (world, entity)
    }

    // --- 服丹急行 ---

    #[test]
    fn pill_rush_rejects_without_cultivation() {
        let mut world = bevy_ecs::world::World::new();
        let caster = world.spawn_empty().id();
        let result = resolve_pill_rush(&mut world, caster, 0, None);
        assert_eq!(
            result,
            CastResult::Rejected {
                reason: CastRejectReason::RealmTooLow
            },
            "无 Cultivation 组件时拒绝（视为凡人）"
        );
    }

    #[test]
    fn pill_rush_succeeds_at_awaken() {
        let qi_cost = dandao_qi_cost_base(Realm::Awaken);
        let (mut world, caster) = make_world_with_caster(Realm::Awaken, qi_cost + 1.0, 100.0);
        let result = resolve_pill_rush(&mut world, caster, 0, None);
        assert!(
            matches!(result, CastResult::Started { .. }),
            "醒灵境有足够 qi 可以使用服丹急行"
        );
    }

    #[test]
    fn pill_rush_rejects_qi_insufficient() {
        // qi_current < capacity_for_tier(0) * 0.03 = 10 * 0.03 = 0.3
        let (mut world, caster) = make_world_with_caster(Realm::Awaken, 0.0, 100.0);
        let result = resolve_pill_rush(&mut world, caster, 0, None);
        assert_eq!(
            result,
            CastResult::Rejected {
                reason: CastRejectReason::QiInsufficient
            },
            "真元不足时拒绝: 需要 capacity_for_tier(0)*0.03={}, 现有 0.0",
            dandao_qi_cost_base(Realm::Awaken)
        );
    }

    #[test]
    fn pill_rush_qi_cost_scales_with_realm_via_capacity_for_tier() {
        // §8.1 #4: qi 消耗应随境界递增（通过 capacity_for_tier 而非 qi_max）
        let cost_awaken = dandao_qi_cost_base(Realm::Awaken);
        let cost_induce = dandao_qi_cost_base(Realm::Induce);
        let cost_void = dandao_qi_cost_base(Realm::Void);
        assert!(
            cost_awaken < cost_induce,
            "引气 qi 消耗({cost_induce})应大于醒灵({cost_awaken})"
        );
        assert!(
            cost_induce < cost_void,
            "化虚 qi 消耗({cost_void})应大于引气({cost_induce})"
        );
    }

    #[test]
    fn pill_rush_qi_cost_uses_capacity_not_qi_max() {
        // 验证 qi 消耗不依赖 qi_max：两个不同 qi_max 的醒灵修士消耗相同
        let cost = dandao_qi_cost_base(Realm::Awaken);
        // 玩家 A: qi_max=10, qi_current=cost-0.01 → rejected
        let (mut world_a, caster_a) = make_world_with_caster(Realm::Awaken, cost - 0.01, 10.0);
        assert_eq!(
            resolve_pill_rush(&mut world_a, caster_a, 0, None),
            CastResult::Rejected {
                reason: CastRejectReason::QiInsufficient
            },
            "qi_max=10, qi_current 刚好不够 → 拒绝"
        );
        // 玩家 B: qi_max=1000, qi_current=cost → accepted
        let (mut world_b, caster_b) = make_world_with_caster(Realm::Awaken, cost, 1000.0);
        assert!(
            matches!(
                resolve_pill_rush(&mut world_b, caster_b, 0, None),
                CastResult::Started { .. }
            ),
            "qi_max=1000, qi_current=cost → 应通过"
        );
    }

    // --- 投丹 ---

    #[test]
    fn pill_bomb_rejects_below_yinqi() {
        let (mut world, caster) = make_world_with_caster(Realm::Awaken, 40.0, 40.0);
        let result = resolve_pill_bomb(&mut world, caster, 0, None);
        assert_eq!(
            result,
            CastResult::Rejected {
                reason: CastRejectReason::RealmTooLow
            },
            "醒灵境不能使用投丹（需引气）"
        );
    }

    #[test]
    fn pill_bomb_succeeds_at_yinqi() {
        let (mut world, caster) = make_world_with_caster(Realm::Induce, 40.0, 40.0);
        let result = resolve_pill_bomb(&mut world, caster, 0, None);
        assert!(
            matches!(result, CastResult::Started { .. }),
            "引气境可以使用投丹"
        );
    }

    #[test]
    fn pill_bomb_rejects_qi_insufficient() {
        let qi_cost = dandao_qi_cost_base(Realm::Induce) * PILL_BOMB_QI_MULTIPLIER;
        let (mut world, caster) = make_world_with_caster(Realm::Induce, qi_cost - 0.01, 100.0);
        let result = resolve_pill_bomb(&mut world, caster, 0, None);
        assert_eq!(
            result,
            CastResult::Rejected {
                reason: CastRejectReason::QiInsufficient
            },
            "投丹 qi 不足时拒绝: 需要 {qi_cost}, 现有 {}",
            qi_cost - 0.01
        );
    }

    #[test]
    fn pill_bomb_qi_cost_is_1_5x_pill_rush_at_same_realm() {
        let rush_cost = dandao_qi_cost_base(Realm::Induce);
        let bomb_cost = dandao_qi_cost_base(Realm::Induce) * PILL_BOMB_QI_MULTIPLIER;
        assert!(
            (bomb_cost - rush_cost * 1.5).abs() < f64::EPSILON,
            "投丹消耗应为服丹急行的 1.5 倍: rush={rush_cost}, bomb={bomb_cost}"
        );
    }

    // --- 丹雾 ---

    #[test]
    fn pill_mist_rejects_below_ningmai() {
        let (mut world, caster) = make_world_with_caster(Realm::Induce, 150.0, 150.0);
        let result = resolve_pill_mist(&mut world, caster, 0, None);
        assert_eq!(
            result,
            CastResult::Rejected {
                reason: CastRejectReason::RealmTooLow
            },
            "引气境不能使用丹雾（需凝脉）"
        );
    }

    #[test]
    fn pill_mist_succeeds_at_ningmai() {
        let (mut world, caster) = make_world_with_caster(Realm::Condense, 150.0, 150.0);
        let result = resolve_pill_mist(&mut world, caster, 0, None);
        assert!(
            matches!(result, CastResult::Started { .. }),
            "凝脉境可以使用丹雾"
        );
    }

    #[test]
    fn pill_mist_rejects_qi_insufficient() {
        let (mut world, caster) = make_world_with_caster(Realm::Condense, 5.0, 150.0);
        let result = resolve_pill_mist(&mut world, caster, 0, None);
        assert_eq!(
            result,
            CastResult::Rejected {
                reason: CastRejectReason::QiInsufficient
            },
            "真元不足 {PILL_MIST_QI_COST} qi 时拒绝丹雾"
        );
    }

    #[test]
    fn pill_mist_qi_boundary_exact_cost() {
        // 恰好等于 10.0 qi 应通过
        let (mut world, caster) = make_world_with_caster(Realm::Condense, PILL_MIST_QI_COST, 150.0);
        assert!(
            matches!(
                resolve_pill_mist(&mut world, caster, 0, None),
                CastResult::Started { .. }
            ),
            "恰好 {PILL_MIST_QI_COST} qi 应通过"
        );
    }

    // --- 经脉 SEVERED 拒绝 ---

    #[test]
    fn pill_rush_rejects_severed_spleen() {
        let qi_cost = dandao_qi_cost_base(Realm::Awaken);
        let (mut world, caster) = make_world_with_caster(Realm::Awaken, qi_cost + 1.0, 100.0);
        let mut severed = MeridianSeveredPermanent::default();
        severed.severed_meridians.insert(MeridianId::Spleen);
        world.entity_mut(caster).insert(severed);

        let result = resolve_pill_rush(&mut world, caster, 0, None);
        assert_eq!(
            result,
            CastResult::Rejected {
                reason: CastRejectReason::MeridianSevered(Some(MeridianId::Spleen))
            },
            "脾经断裂时服丹急行不可用"
        );
    }

    #[test]
    fn pill_rush_rejects_severed_kidney() {
        let qi_cost = dandao_qi_cost_base(Realm::Awaken);
        let (mut world, caster) = make_world_with_caster(Realm::Awaken, qi_cost + 1.0, 100.0);
        let mut severed = MeridianSeveredPermanent::default();
        severed.severed_meridians.insert(MeridianId::Kidney);
        world.entity_mut(caster).insert(severed);

        let result = resolve_pill_rush(&mut world, caster, 0, None);
        assert_eq!(
            result,
            CastResult::Rejected {
                reason: CastRejectReason::MeridianSevered(Some(MeridianId::Kidney))
            },
            "肾经断裂时服丹急行不可用"
        );
    }

    #[test]
    fn pill_bomb_rejects_severed_lung() {
        let (mut world, caster) = make_world_with_caster(Realm::Induce, 40.0, 100.0);
        let mut severed = MeridianSeveredPermanent::default();
        severed.severed_meridians.insert(MeridianId::Lung);
        world.entity_mut(caster).insert(severed);

        let result = resolve_pill_bomb(&mut world, caster, 0, None);
        assert_eq!(
            result,
            CastResult::Rejected {
                reason: CastRejectReason::MeridianSevered(Some(MeridianId::Lung))
            },
            "肺经断裂时投丹不可用"
        );
    }

    #[test]
    fn pill_mist_rejects_severed_liver() {
        let (mut world, caster) = make_world_with_caster(Realm::Condense, 150.0, 150.0);
        let mut severed = MeridianSeveredPermanent::default();
        severed.severed_meridians.insert(MeridianId::Liver);
        world.entity_mut(caster).insert(severed);

        let result = resolve_pill_mist(&mut world, caster, 0, None);
        assert_eq!(
            result,
            CastResult::Rejected {
                reason: CastRejectReason::MeridianSevered(Some(MeridianId::Liver))
            },
            "肝经断裂时丹雾不可用"
        );
    }

    // --- 冷却值正确性 ---

    #[test]
    fn pill_rush_cooldown_is_15s() {
        let qi_cost = dandao_qi_cost_base(Realm::Awaken);
        let (mut world, caster) = make_world_with_caster(Realm::Awaken, qi_cost + 1.0, 100.0);
        let result = resolve_pill_rush(&mut world, caster, 0, None);
        match result {
            CastResult::Started { cooldown_ticks, .. } => {
                assert_eq!(cooldown_ticks, 300, "服丹急行 CD = 15s = 300 ticks");
            }
            _ => panic!("应为 Started"),
        }
    }

    #[test]
    fn pill_bomb_cooldown_is_8s() {
        let (mut world, caster) = make_world_with_caster(Realm::Induce, 40.0, 40.0);
        let result = resolve_pill_bomb(&mut world, caster, 0, None);
        match result {
            CastResult::Started { cooldown_ticks, .. } => {
                assert_eq!(cooldown_ticks, 160, "投丹 CD = 8s = 160 ticks");
            }
            _ => panic!("应为 Started"),
        }
    }

    #[test]
    fn pill_mist_cooldown_is_30s() {
        let (mut world, caster) = make_world_with_caster(Realm::Condense, 150.0, 150.0);
        let result = resolve_pill_mist(&mut world, caster, 0, None);
        match result {
            CastResult::Started { cooldown_ticks, .. } => {
                assert_eq!(cooldown_ticks, 600, "丹雾 CD = 30s = 600 ticks");
            }
            _ => panic!("应为 Started"),
        }
    }

    // --- qi 实际扣除验证 ---

    #[test]
    fn pill_rush_actually_deducts_qi() {
        let qi_cost = dandao_qi_cost_base(Realm::Awaken);
        let initial_qi = qi_cost + 10.0;
        let (mut world, caster) = make_world_with_caster(Realm::Awaken, initial_qi, 100.0);
        let result = resolve_pill_rush(&mut world, caster, 0, None);
        assert!(matches!(result, CastResult::Started { .. }));
        let cultivation = world.get::<Cultivation>(caster).unwrap();
        let expected = initial_qi - qi_cost;
        assert!(
            (cultivation.qi_current - expected).abs() < f64::EPSILON,
            "服丹急行应扣除 {qi_cost} qi: 期望 {expected}, 实际 {}",
            cultivation.qi_current
        );
    }

    #[test]
    fn pill_bomb_actually_deducts_qi() {
        let qi_cost = dandao_qi_cost_base(Realm::Induce) * PILL_BOMB_QI_MULTIPLIER;
        let initial_qi = qi_cost + 10.0;
        let (mut world, caster) = make_world_with_caster(Realm::Induce, initial_qi, 200.0);
        let result = resolve_pill_bomb(&mut world, caster, 0, None);
        assert!(matches!(result, CastResult::Started { .. }));
        let cultivation = world.get::<Cultivation>(caster).unwrap();
        let expected = initial_qi - qi_cost;
        assert!(
            (cultivation.qi_current - expected).abs() < f64::EPSILON,
            "投丹应扣除 {qi_cost} qi: 期望 {expected}, 实际 {}",
            cultivation.qi_current
        );
    }

    #[test]
    fn pill_mist_actually_deducts_qi() {
        let initial_qi = 50.0;
        let (mut world, caster) = make_world_with_caster(Realm::Condense, initial_qi, 200.0);
        let result = resolve_pill_mist(&mut world, caster, 0, None);
        assert!(matches!(result, CastResult::Started { .. }));
        let cultivation = world.get::<Cultivation>(caster).unwrap();
        let expected = initial_qi - PILL_MIST_QI_COST;
        assert!(
            (cultivation.qi_current - expected).abs() < f64::EPSILON,
            "丹雾应扣除 {PILL_MIST_QI_COST} qi: 期望 {expected}, 实际 {}",
            cultivation.qi_current
        );
    }

    #[test]
    fn pill_rush_emits_qi_transfer_event() {
        let qi_cost = dandao_qi_cost_base(Realm::Awaken);
        let (mut world, caster) = make_world_with_caster(Realm::Awaken, qi_cost + 1.0, 100.0);
        let result = resolve_pill_rush(&mut world, caster, 0, None);
        assert!(matches!(result, CastResult::Started { .. }));
        let events = world.resource::<Events<QiTransfer>>();
        let mut reader = events.get_reader();
        let transfers: Vec<_> = reader.read(events).collect();
        assert_eq!(transfers.len(), 1, "服丹急行应 emit 1 条 QiTransfer 事件");
        assert_eq!(
            transfers[0].reason,
            QiTransferReason::ReleaseToZone,
            "QiTransfer reason 应为 ReleaseToZone"
        );
        assert!(
            (transfers[0].amount - qi_cost).abs() < f64::EPSILON,
            "QiTransfer 金额应等于 qi_cost={qi_cost}, 实际 {}",
            transfers[0].amount
        );
    }

    #[test]
    fn pill_mist_exact_boundary_deducts_to_zero() {
        let (mut world, caster) = make_world_with_caster(Realm::Condense, PILL_MIST_QI_COST, 200.0);
        let result = resolve_pill_mist(&mut world, caster, 0, None);
        assert!(matches!(result, CastResult::Started { .. }));
        let cultivation = world.get::<Cultivation>(caster).unwrap();
        assert!(
            cultivation.qi_current.abs() < f64::EPSILON,
            "恰好 {PILL_MIST_QI_COST} qi 后应扣至 0, 实际 {}",
            cultivation.qi_current
        );
    }

    #[test]
    fn rejected_cast_does_not_deduct_qi() {
        // Realm too low for pill_bomb (need Induce, have Awaken)
        let (mut world, caster) = make_world_with_caster(Realm::Awaken, 100.0, 100.0);
        let result = resolve_pill_bomb(&mut world, caster, 0, None);
        assert!(matches!(result, CastResult::Rejected { .. }));
        let cultivation = world.get::<Cultivation>(caster).unwrap();
        assert!(
            (cultivation.qi_current - 100.0).abs() < f64::EPSILON,
            "拒绝时不应扣除 qi: 期望 100.0, 实际 {}",
            cultivation.qi_current
        );
    }
}
