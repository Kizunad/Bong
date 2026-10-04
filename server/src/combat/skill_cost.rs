//! 技能栏通用施放的资源门与起手结算。
//!
//! 没有专属 resolver 的功法仍然必须遵守 metadata 中的境界、真元和体力成本。
//! 真元结算统一调用 `release_qi_amount_to_zone`，由 `WorldQiAccount` 记录转移，
//! 区域装满或定位失败时进入稳定 overflow 账户，不能直接改余额后静默丢失。

use valence::prelude::{bevy_ecs, Entity, Events, Mut, Position};

use crate::combat::components::{Stamina, StaminaState};
use crate::combat::CombatClock;
use crate::cultivation::components::Cultivation;
use crate::cultivation::death_hooks::release_qi_amount_to_zone;
use crate::cultivation::life_record::LifeRecord;
use crate::qi_physics::constants::QI_EPSILON;
use crate::qi_physics::ledger::{QiTransfer, WorldQiAccount};
use crate::world::dimension::CurrentDimension;
use crate::world::zone::ZoneRegistry;

/// 境界门。缺少 Cultivation 的实体按不满足处理。
pub fn realm_sufficient(
    world: &bevy_ecs::world::World,
    caster: Entity,
    required: crate::cultivation::components::Realm,
) -> bool {
    world.get::<Cultivation>(caster).is_some_and(|cultivation| {
        crate::cultivation::technique_scroll::realm_rank(cultivation.realm)
            >= crate::cultivation::technique_scroll::realm_rank(required)
    })
}

/// 真元门。零成本 metadata 直接放行；缺少 Cultivation 按不满足处理。
pub fn qi_sufficient(world: &bevy_ecs::world::World, caster: Entity, amount: f64) -> bool {
    if amount <= QI_EPSILON {
        return true;
    }
    world
        .get::<Cultivation>(caster)
        .is_some_and(|cultivation| cultivation.qi_current + QI_EPSILON >= amount)
}

/// 体力门。没有 Stamina 组件的最小化实体沿用 resolver 的宽容语义；有组件时严格检查。
pub fn stamina_sufficient(world: &bevy_ecs::world::World, caster: Entity, amount: f32) -> bool {
    if amount <= f32::EPSILON {
        return true;
    }
    world.get::<Stamina>(caster).is_none_or(|stamina| {
        stamina.state != StaminaState::Exhausted
            && stamina.current > 0.0
            && stamina.current + f32::EPSILON >= amount
    })
}

/// 在起手一次结算真元，并把相同数量经 qi ledger 释放回环境。
///
/// 成功返回 `true`；缺少玩家身份、`WorldQiAccount` 或真元不足时返回 `false`，且不
/// 改动玩家状态。区域资源可以缺失，此时规范 helper 会把全部金额记入 overflow。
pub fn spend_qi_conserved(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    amount: f64,
    source: &'static str,
) -> bool {
    if amount <= QI_EPSILON {
        return true;
    }
    if world.get::<Cultivation>(caster).is_none()
        || world.get::<LifeRecord>(caster).is_none()
        || world.get_resource::<WorldQiAccount>().is_none()
    {
        return false;
    }

    let position = world.get::<Position>(caster).copied();
    let dimension = world.get::<CurrentDimension>(caster).copied();
    let life_record = world
        .get::<LifeRecord>(caster)
        .expect("life record existence checked above")
        .clone();

    if world.contains_resource::<ZoneRegistry>() {
        world.resource_scope(|world, mut zones: Mut<'_, ZoneRegistry>| {
            settle_qi(
                world,
                caster,
                amount,
                position.as_ref(),
                dimension.as_ref(),
                &life_record,
                Some(&mut zones),
                source,
            )
        })
    } else {
        settle_qi(
            world,
            caster,
            amount,
            position.as_ref(),
            dimension.as_ref(),
            &life_record,
            None,
            source,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn settle_qi(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    amount: f64,
    position: Option<&Position>,
    dimension: Option<&CurrentDimension>,
    life_record: &LifeRecord,
    zones: Option<&mut ZoneRegistry>,
    source: &'static str,
) -> bool {
    if world.contains_resource::<Events<QiTransfer>>() {
        world.resource_scope(|world, mut transfers: Mut<'_, Events<QiTransfer>>| {
            settle_qi_with_ledger(
                world,
                caster,
                amount,
                position,
                dimension,
                life_record,
                zones,
                Some(&mut transfers),
                source,
            )
        })
    } else {
        settle_qi_with_ledger(
            world,
            caster,
            amount,
            position,
            dimension,
            life_record,
            zones,
            None,
            source,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn settle_qi_with_ledger(
    world: &mut bevy_ecs::world::World,
    caster: Entity,
    amount: f64,
    position: Option<&Position>,
    dimension: Option<&CurrentDimension>,
    life_record: &LifeRecord,
    zones: Option<&mut ZoneRegistry>,
    transfers: Option<&mut Events<QiTransfer>>,
    source: &'static str,
) -> bool {
    world.resource_scope(|world, mut ledger: Mut<'_, WorldQiAccount>| {
        let Some(mut cultivation) = world.get_mut::<Cultivation>(caster) else {
            return false;
        };
        release_qi_amount_to_zone(
            &mut cultivation,
            amount,
            position,
            dimension,
            Some(life_record),
            zones,
            &mut ledger,
            transfers,
            source,
        )
        .is_ok()
    })
}

/// 扣除已通过门禁的体力。
pub fn spend_stamina(world: &mut bevy_ecs::world::World, caster: Entity, amount: f32) {
    if amount <= f32::EPSILON {
        return;
    }
    let now_tick = world
        .get_resource::<CombatClock>()
        .map_or(0, |clock| clock.tick);
    let Some(mut stamina) = world.get_mut::<Stamina>(caster) else {
        return;
    };
    stamina.current = (stamina.current - amount).clamp(0.0, stamina.max);
    stamina.state = if stamina.current <= 0.0 {
        StaminaState::Exhausted
    } else {
        StaminaState::Combat
    };
    stamina.last_drain_tick = Some(now_tick);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qi_physics::ledger::{assert_conservation, summarize_world_qi};
    use crate::qi_physics::WorldQiBudget;
    use crate::schema::common::TEST_QI_FIXTURE_TOTAL;
    use crate::world::dimension::DimensionKind;
    use valence::prelude::App;

    fn test_app() -> App {
        let mut app = App::new();
        app.insert_resource(WorldQiAccount::default());
        app.insert_resource(WorldQiBudget::from_total(TEST_QI_FIXTURE_TOTAL));
        app.insert_resource(ZoneRegistry::fallback());
        app.add_event::<QiTransfer>();
        app
    }

    fn caster(app: &mut App, qi: f64) -> Entity {
        app.world_mut()
            .spawn((
                Cultivation {
                    qi_current: qi,
                    qi_max: TEST_QI_FIXTURE_TOTAL,
                    ..Default::default()
                },
                LifeRecord::new(crate::player::state::canonical_player_id("GenericCost")),
                Position::new([0.0, 64.0, 0.0]),
                CurrentDimension(DimensionKind::Overworld),
            ))
            .id()
    }

    #[test]
    fn conserved_spend_preserves_world_snapshot_and_emits_ledger_transfer() {
        let mut app = test_app();
        let entity = caster(&mut app, 10.0);
        let before = summarize_world_qi(app.world_mut());
        assert_eq!(before.budget_initial_total, TEST_QI_FIXTURE_TOTAL);

        assert!(spend_qi_conserved(
            app.world_mut(),
            entity,
            1.0,
            "generic_skillbar_cast"
        ));

        let after = summarize_world_qi(app.world_mut());
        assert_eq!(after.budget_initial_total, TEST_QI_FIXTURE_TOTAL);
        assert_conservation(&before, &after, 0.0)
            .expect("generic cast qi must move through the ledger without world loss");
        let transfers = app
            .world()
            .resource::<Events<QiTransfer>>()
            .iter_current_update_events()
            .collect::<Vec<_>>();
        assert_eq!(transfers.len(), 1);
        assert_eq!(transfers[0].amount, 1.0);
    }

    #[test]
    fn conserved_spend_without_identity_or_qi_is_a_noop() {
        let mut app = test_app();
        let entity = caster(&mut app, 0.5);
        let before = summarize_world_qi(app.world_mut());
        assert!(!spend_qi_conserved(
            app.world_mut(),
            entity,
            1.0,
            "generic_skillbar_cast"
        ));
        let after = summarize_world_qi(app.world_mut());
        assert_conservation(&before, &after, 0.0)
            .expect("rejected generic cast must not mutate qi ownership");
        assert_eq!(
            app.world().get::<Cultivation>(entity).unwrap().qi_current,
            0.5
        );

        let bare = app.world_mut().spawn_empty().id();
        assert!(!spend_qi_conserved(
            app.world_mut(),
            bare,
            1.0,
            "generic_skillbar_cast"
        ));
    }
}
