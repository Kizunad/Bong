//! 丹道三基础招式 resolver。
//!
//! 遵循 SkillFn 签名: fn(&mut World, Entity, u8, Option<Entity>) -> CastResult。
//! 每个招式在 cast 时检查：
//! 1. 境界 gate（服丹急行=醒灵, 投丹=引气, 丹雾=凝脉）
//! 2. 经脉依赖（check_meridian_dependencies）
//! 3. 真元是否足够（qi_max × 3% 或固定值）
//! 4. 冷却

use valence::prelude::{bevy_ecs, Entity};

use crate::cultivation::components::{Cultivation, MeridianId, Realm};
use crate::cultivation::meridian::severed::{
    check_meridian_dependencies, MeridianSeveredPermanent,
};
use crate::cultivation::skill_registry::{CastRejectReason, CastResult};

pub const DANDAO_PILL_RUSH_SKILL_ID: &str = "dandao.pill_rush";
pub const DANDAO_PILL_BOMB_SKILL_ID: &str = "dandao.pill_bomb";
pub const DANDAO_PILL_MIST_SKILL_ID: &str = "dandao.pill_mist";

/// 服丹急行消耗比例
const PILL_RUSH_QI_RATIO: f64 = 0.03;
/// 投丹基础冷却 (ticks, 20tps × 8s = 160)
const PILL_BOMB_COOLDOWN_TICKS: u64 = 160;
/// 丹雾基础消耗
const PILL_MIST_QI_COST: f64 = 10.0;
/// 丹雾冷却 (20tps × 30s = 600)
const PILL_MIST_COOLDOWN_TICKS: u64 = 600;
/// 服丹急行冷却 (20tps × 15s = 300)
const PILL_RUSH_COOLDOWN_TICKS: u64 = 300;

// 经脉依赖（plan §1.4）
const PILL_RUSH_MERIDIANS: &[MeridianId] = &[MeridianId::Spleen, MeridianId::Kidney];
const PILL_BOMB_MERIDIANS: &[MeridianId] = &[MeridianId::Lung, MeridianId::Spleen];
const PILL_MIST_MERIDIANS: &[MeridianId] = &[MeridianId::Spleen, MeridianId::Liver];

/// 招式一：服丹急行 — 自服战斗丹（零距离，buff 自身）
/// 境界要求：醒灵+
/// 消耗：qi_max × 3%
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

    let qi_cost = cultivation.qi_max * PILL_RUSH_QI_RATIO;
    if cultivation.qi_current < qi_cost {
        return CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient,
        };
    }

    CastResult::Started {
        cooldown_ticks: PILL_RUSH_COOLDOWN_TICKS,
        anim_duration_ticks: 10,
    }
}

/// 招式二：投丹 — 投掷丹药弹（5-15 格中距离）
/// 境界要求：引气+
/// 消耗：丹药本身 + 封存真元
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

    let qi_cost = cultivation.qi_max * PILL_RUSH_QI_RATIO * 1.5;
    if cultivation.qi_current < qi_cost {
        return CastResult::Rejected {
            reason: CastRejectReason::QiInsufficient,
        };
    }

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
        let (mut world, caster) = make_world_with_caster(Realm::Awaken, 10.0, 10.0);
        let result = resolve_pill_rush(&mut world, caster, 0, None);
        assert!(
            matches!(result, CastResult::Started { .. }),
            "醒灵境可以使用服丹急行"
        );
    }

    #[test]
    fn pill_rush_rejects_qi_insufficient() {
        let (mut world, caster) = make_world_with_caster(Realm::Awaken, 0.0, 100.0);
        let result = resolve_pill_rush(&mut world, caster, 0, None);
        assert_eq!(
            result,
            CastResult::Rejected {
                reason: CastRejectReason::QiInsufficient
            },
            "真元不足时拒绝"
        );
    }

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
            "真元不足 10 qi 时拒绝丹雾"
        );
    }

    #[test]
    fn pill_rush_rejects_severed_spleen() {
        let (mut world, caster) = make_world_with_caster(Realm::Awaken, 10.0, 10.0);
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
}
