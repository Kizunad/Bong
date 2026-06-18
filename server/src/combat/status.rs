use valence::prelude::{EventReader, Position, Query, Res};

use crate::combat::components::{
    ActiveStatusEffect, BodyPart, BodyRefiningMarker, DerivedAttrs, Stamina, StaminaState,
    StatusEffects, STATUS_EFFECT_TICK_INTERVAL_TICKS,
};
use crate::combat::events::{ApplyStatusEffectIntent, StatusEffectKind};
use crate::combat::CombatClock;
use crate::cultivation::components::Cultivation;
use crate::cultivation::death_hooks::release_qi_amount_to_zone;
use crate::cultivation::full_power_strike::Exhausted;
use crate::cultivation::life_record::LifeRecord;
use crate::qi_physics::constants::{QI_EPSILON, QI_ZHENMAI_PARRY_RECOVERY_MOVE_SPEED_MULTIPLIER};
use crate::qi_physics::QiTransfer;
use crate::world::dimension::CurrentDimension;
use crate::world::zone::ZoneRegistry;

pub fn status_effect_apply_tick(
    mut intents: EventReader<ApplyStatusEffectIntent>,
    mut statuses: Query<&mut StatusEffects>,
) {
    for intent in intents.read() {
        let Ok(mut status_effects) = statuses.get_mut(intent.target) else {
            continue;
        };

        if intent.duration_ticks == 0 {
            remove_status_effect(&mut status_effects, intent.kind.clone());
            continue;
        }

        if intent.magnitude <= 0.0 {
            continue;
        }

        upsert_status_effect(
            &mut status_effects,
            ActiveStatusEffect {
                kind: intent.kind.clone(),
                magnitude: intent.magnitude,
                remaining_ticks: intent.duration_ticks,
                source_pill: None,
            },
        );
    }
}

pub fn upsert_status_effect(status_effects: &mut StatusEffects, effect: ActiveStatusEffect) {
    if let Some(existing) = status_effects
        .active
        .iter_mut()
        .find(|active| active.kind == effect.kind)
    {
        existing.magnitude = existing.magnitude.max(effect.magnitude);
        existing.remaining_ticks = existing.remaining_ticks.max(effect.remaining_ticks);
        return;
    }

    status_effects.active.push(effect);
}

/// plan-cultivation-pacing-v1 §8.1 #7：CultivationAcceleration 专用堆叠入口。
/// 允许多条同 kind 共存（丹药堆叠），但同一 `source_pill` 最多 2 条有效。
/// 返回 true 表示成功入栈，false 表示被 per-pill cap 拦截。
///
/// 由修炼丹药 consume_cultivation_pill 和 dandao/alchemy 系统投喂。
pub fn push_status_effect(status_effects: &mut StatusEffects, effect: ActiveStatusEffect) -> bool {
    if let Some(ref pill) = effect.source_pill {
        let same_pill_count = status_effects
            .active
            .iter()
            .filter(|e| e.source_pill.as_deref() == Some(pill) && e.remaining_ticks > 0)
            .count();
        if same_pill_count >= 2 {
            return false; // 同种丹药最多 2 层
        }
    }
    status_effects.active.push(effect);
    true
}

pub fn remove_status_effect(status_effects: &mut StatusEffects, kind: StatusEffectKind) {
    status_effects.active.retain(|effect| effect.kind != kind);
}

pub fn has_active_status(status_effects: &StatusEffects, kind: StatusEffectKind) -> bool {
    status_effects
        .active
        .iter()
        .any(|effect| effect.kind == kind && effect.remaining_ticks > 0)
}

/// plan-cultivation-v1 §3.1：汇总 BreakthroughBoost buff magnitude。
/// 只统计 remaining_ticks > 0 的条目；返回未 clamp 的和，调用方负责封顶。
pub fn sum_breakthrough_boost(status_effects: &StatusEffects) -> f32 {
    status_effects
        .active
        .iter()
        .filter(|e| e.kind == StatusEffectKind::BreakthroughBoost && e.remaining_ticks > 0)
        .map(|e| e.magnitude.max(0.0))
        .sum()
}

/// 一次性消费：移除所有 BreakthroughBoost 条目。供 breakthrough_system 在成败后调用。
pub fn clear_breakthrough_boost(status_effects: &mut StatusEffects) {
    status_effects
        .active
        .retain(|e| e.kind != StatusEffectKind::BreakthroughBoost);
}

/// plan-worldgen-v4-activate bughunt r4-P2#7：渡劫结束时移除渡劫丹来源的 DamageReduction。
///
/// 渡劫丹（`du_jie_dan`）以 `source_pill = Some("du_jie_dan")` + `duration_ticks = u64::MAX`
/// 施加 `DamageReduction(0.30)`，不依赖 tick 自然到期——必须在渡劫 settle 收口主动清除。
///
/// 只清 `source_pill == "du_jie_dan"` 的 DamageReduction，精准识别来源，
/// **不触碰**装备/技能等其它来源的 DamageReduction。
pub fn clear_du_jie_dan_damage_reduction(status_effects: &mut StatusEffects) {
    status_effects.active.retain(|e| {
        !(e.kind == StatusEffectKind::DamageReduction
            && e.source_pill.as_deref() == Some("du_jie_dan"))
    });
}

pub fn status_effect_tick(clock: Res<CombatClock>, mut statuses: Query<&mut StatusEffects>) {
    if !clock.tick.is_multiple_of(STATUS_EFFECT_TICK_INTERVAL_TICKS) {
        return;
    }

    for mut status_effects in &mut statuses {
        for effect in &mut status_effects.active {
            effect.remaining_ticks = effect
                .remaining_ticks
                .saturating_sub(STATUS_EFFECT_TICK_INTERVAL_TICKS);
        }

        // plan-cultivation-pacing-v1 §8.1 #8：洗髓液到期回调。
        // 在清理过期 effect 之前检查——source_pill=="xi_sui_ye" 的
        // CultivationAcceleration 到期（remaining==0）时追加 QiRegenSlowed。
        crate::alchemy::pill::check_xi_sui_ye_expiry_and_push_debuff(&mut status_effects);

        status_effects
            .active
            .retain(|effect| effect.remaining_ticks > 0);
    }
}

const BODY_REFINING_DEFENSE_MULTIPLIER: f32 = 1.0 / 1.3;
const DEFAULT_STAMINA_MAX_FOR_STATUS: f32 = 100.0;
const DEFAULT_STAMINA_RECOVER_FOR_STATUS: f32 = 5.0;
const MAX_HEALTH_REGEN_BOOST_MULTIPLIER: f32 = 5.0;

pub fn health_regen_boost_multiplier(status_effects: &StatusEffects) -> f32 {
    status_effects
        .active
        .iter()
        .filter(|effect| {
            effect.kind == StatusEffectKind::HealthRegenBoost && effect.remaining_ticks > 0
        })
        .fold(1.0, |acc, effect| acc * (1.0 + effect.magnitude.max(0.0)))
        .clamp(1.0, MAX_HEALTH_REGEN_BOOST_MULTIPLIER)
}

pub fn attribute_aggregate_tick(
    mut q: Query<(
        &StatusEffects,
        &mut DerivedAttrs,
        Option<&BodyRefiningMarker>,
        Option<&Exhausted>,
    )>,
) {
    for (status_effects, mut attrs, body_refining, exhausted) in &mut q {
        attrs.attack_power = 1.0;
        attrs.defense_power = 1.0;
        attrs.move_speed_multiplier = 1.0;
        attrs.jump_height_multiplier = 1.0;
        attrs.qi_max_multiplier = 1.0;

        let slow_multiplier = status_effects
            .active
            .iter()
            .filter(|effect| effect.kind == StatusEffectKind::Slowed)
            .fold(1.0, |acc, effect| {
                acc * (1.0 - effect.magnitude.clamp(0.0, 0.95))
            });
        let vortex_multiplier =
            if has_active_status(status_effects, StatusEffectKind::VortexCasting) {
                0.2
            } else {
                1.0
            };
        let parry_recovery_multiplier =
            if has_active_status(status_effects, StatusEffectKind::ParryRecovery) {
                QI_ZHENMAI_PARRY_RECOVERY_MOVE_SPEED_MULTIPLIER
            } else {
                1.0
            };

        let damage_amp_multiplier = status_effects
            .active
            .iter()
            .filter(|effect| effect.kind == StatusEffectKind::DamageAmp)
            .fold(1.0, |acc, effect| acc * (1.0 + effect.magnitude.max(0.0)));

        let damage_reduction_multiplier = status_effects
            .active
            .iter()
            .filter(|effect| effect.kind == StatusEffectKind::DamageReduction)
            .fold(1.0, |acc, effect| {
                acc * (1.0 - effect.magnitude.clamp(0.0, 0.95))
            });
        let speed_boost_multiplier = status_effects
            .active
            .iter()
            .filter(|effect| effect.kind == StatusEffectKind::SpeedBoost)
            .fold(1.0, |acc, effect| acc * (1.0 + effect.magnitude.max(0.0)));
        let stamina_crash_slow = status_effects
            .active
            .iter()
            .filter(|effect| effect.kind == StatusEffectKind::StaminaCrash)
            .fold(1.0, |acc, effect| {
                acc * (1.0 - (effect.magnitude * 0.5).clamp(0.0, 0.75))
            });
        let leg_strain_slow = status_effects
            .active
            .iter()
            .filter(|effect| effect.kind == StatusEffectKind::LegStrain)
            .fold(1.0, |acc, effect| {
                acc * (1.0 - (effect.magnitude * 0.15).clamp(0.0, 1.0))
            });

        attrs.move_speed_multiplier = (slow_multiplier
            * vortex_multiplier
            * parry_recovery_multiplier
            * speed_boost_multiplier
            * stamina_crash_slow
            * leg_strain_slow)
            .clamp(0.05, 2.5);
        // plan-cultivation-pacing-v1 P1.1：DamageVulnerability —— 受击伤害 × (1+N)。
        // 在所有防御 reduction 之后乘入，允许 defense_power 超过 1.0（脆弱态）。
        let vulnerability_multiplier = status_effects
            .active
            .iter()
            .filter(|effect| {
                effect.kind == StatusEffectKind::DamageVulnerability && effect.remaining_ticks > 0
            })
            .map(|effect| effect.magnitude.max(0.0))
            .sum::<f32>();

        attrs.attack_power = damage_amp_multiplier.max(1.0);
        attrs.defense_power = damage_reduction_multiplier.clamp(0.05, 1.0);

        // plan-armor-v1 §4.2：体修 defense_power 基础加成。
        // 1.0 / 1.3 ≈ 0.77，约 23% 基础伤害减免，与护甲 kind_mitigation 独立相乘。
        if body_refining.is_some() {
            attrs.defense_power =
                (attrs.defense_power * BODY_REFINING_DEFENSE_MULTIPLIER).clamp(0.05, 1.0);
        }

        if let Some(exhausted) = exhausted {
            attrs.defense_power =
                (attrs.defense_power * exhausted.defense_modifier).clamp(0.05, 1.0);
        }

        // Vulnerability 在所有 reduction 之后乘入。不 clamp 上限——脆弱就是脆弱。
        if vulnerability_multiplier > f32::EPSILON {
            attrs.defense_power *= 1.0 + vulnerability_multiplier;
        }

        // bughunt r4-P2#5：QiCapPermMinus —— 永久真元上限折损 debuff。
        // 累加所有 remaining_ticks > 0 的 magnitude（fraction 单位），clamp [0, 0.99]
        // 使 qi_max_multiplier ∈ [0.01, 1.0]，由 qi_regen_and_zone_drain_tick 读取。
        let qi_cap_minus: f32 = status_effects
            .active
            .iter()
            .filter(|e| e.kind == StatusEffectKind::QiCapPermMinus && e.remaining_ticks > 0)
            .map(|e| e.magnitude.max(0.0))
            .sum();
        attrs.qi_max_multiplier = (1.0 - f64::from(qi_cap_minus.clamp(0.0, 0.99))).clamp(0.01, 1.0);
    }
}

pub fn body_part_damage_multiplier(status_effects: Option<&StatusEffects>, part: BodyPart) -> f32 {
    let Some(status_effects) = status_effects else {
        return 1.0;
    };
    status_effects
        .active
        .iter()
        .filter(|effect| effect.remaining_ticks > 0)
        .fold(1.0, |acc, effect| {
            let next = match effect.kind {
                StatusEffectKind::BodyPartResist(target) if target == part => {
                    1.0 - effect.magnitude.clamp(0.0, 0.95)
                }
                StatusEffectKind::BodyPartWeaken(target) if target == part => {
                    1.0 + effect.magnitude.max(0.0)
                }
                _ => 1.0,
            };
            acc * next
        })
}

type StaminaStatusActorItem<'a> = (
    valence::prelude::Entity,
    &'a StatusEffects,
    &'a mut Stamina,
    Option<&'a Position>,
    Option<&'a CurrentDimension>,
    Option<&'a LifeRecord>,
    Option<&'a mut Cultivation>,
);

pub fn combat_pill_stamina_status_tick(
    clock: Res<CombatClock>,
    mut actors: Query<StaminaStatusActorItem<'_>>,
    mut zones: Option<valence::prelude::ResMut<ZoneRegistry>>,
    mut qi_transfers: Option<valence::prelude::ResMut<valence::prelude::Events<QiTransfer>>>,
) {
    if !clock.tick.is_multiple_of(STATUS_EFFECT_TICK_INTERVAL_TICKS) {
        return;
    }

    let dt = STATUS_EFFECT_TICK_INTERVAL_TICKS as f32
        / crate::combat::components::TICKS_PER_SECOND as f32;
    for (
        entity,
        status_effects,
        mut stamina,
        position,
        current_dimension,
        life_record,
        cultivation,
    ) in &mut actors
    {
        let has_relevant_status = status_effects.active.iter().any(|effect| {
            matches!(
                effect.kind,
                StatusEffectKind::StaminaRecovBoost
                    | StatusEffectKind::StaminaCrash
                    | StatusEffectKind::QiDrainForStamina
            ) && effect.remaining_ticks > 0
        });
        if !has_relevant_status {
            if (stamina.max - DEFAULT_STAMINA_MAX_FOR_STATUS).abs() > f32::EPSILON {
                stamina.max = DEFAULT_STAMINA_MAX_FOR_STATUS;
                stamina.current = stamina.current.clamp(0.0, stamina.max);
            }
            if (stamina.recover_per_sec - DEFAULT_STAMINA_RECOVER_FOR_STATUS).abs() > f32::EPSILON {
                stamina.recover_per_sec = DEFAULT_STAMINA_RECOVER_FOR_STATUS;
            }
            continue;
        }

        let max_bonus = status_effects
            .active
            .iter()
            .filter(|effect| {
                effect.remaining_ticks > 0
                    && effect.kind == StatusEffectKind::StaminaRecovBoost
                    && effect.magnitude < 1.0
            })
            .fold(0.0_f32, |acc, effect| acc.max(effect.magnitude.max(0.0)));
        let crash_penalty = status_effects
            .active
            .iter()
            .filter(|effect| {
                effect.remaining_ticks > 0 && effect.kind == StatusEffectKind::StaminaCrash
            })
            .fold(0.0_f32, |acc, effect| {
                acc.max(effect.magnitude.clamp(0.0, 0.95))
            });
        let effective_max =
            (DEFAULT_STAMINA_MAX_FOR_STATUS * (1.0 + max_bonus) * (1.0 - crash_penalty)).max(1.0);
        stamina.max = effective_max;
        stamina.current = stamina.current.clamp(0.0, stamina.max);

        let recov_multiplier = status_effects
            .active
            .iter()
            .filter(|effect| {
                effect.remaining_ticks > 0
                    && effect.kind == StatusEffectKind::StaminaRecovBoost
                    && effect.magnitude >= 1.0
            })
            .fold(1.0, |acc, effect| acc * effect.magnitude.max(1.0));
        let crash_recov_multiplier = status_effects
            .active
            .iter()
            .filter(|effect| {
                effect.remaining_ticks > 0 && effect.kind == StatusEffectKind::StaminaCrash
            })
            .fold(1.0, |acc, effect| {
                acc * (1.0 - (effect.magnitude * 2.0).clamp(0.0, 0.9))
            });
        stamina.recover_per_sec =
            (DEFAULT_STAMINA_RECOVER_FOR_STATUS * recov_multiplier * crash_recov_multiplier)
                .max(0.0);

        if has_active_status(status_effects, StatusEffectKind::StaminaCrash)
            && stamina.state != StaminaState::Exhausted
            && stamina.current <= stamina.max * 0.05
        {
            stamina.state = StaminaState::Exhausted;
        }

        let drain_per_sec = status_effects
            .active
            .iter()
            .filter(|effect| {
                effect.remaining_ticks > 0 && effect.kind == StatusEffectKind::QiDrainForStamina
            })
            .map(|effect| effect.magnitude.max(0.0))
            .sum::<f32>();
        if drain_per_sec <= f32::EPSILON {
            continue;
        }
        let amount = f64::from(drain_per_sec * dt);
        let Some(mut cultivation) = cultivation else {
            continue;
        };
        let drained = cultivation.qi_current.min(amount);
        if drained <= QI_EPSILON {
            cultivation.qi_current = 0.0;
            continue;
        }
        cultivation.qi_current = (cultivation.qi_current - drained).max(0.0);
        release_qi_amount_to_zone(
            entity,
            drained,
            position,
            current_dimension,
            life_record,
            zones.as_deref_mut(),
            qi_transfers.as_deref_mut(),
            "combat_pill_stamina_status",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::combat::components::{
        BodyRefiningMarker, DerivedAttrs, StatusEffects, STATUS_EFFECT_TICK_INTERVAL_TICKS,
    };
    use crate::combat::events::{ApplyStatusEffectIntent, StatusEffectKind};
    use crate::combat::CombatClock;
    use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
    use crate::world::dimension::{CurrentDimension, DimensionKind};
    use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};
    use valence::prelude::{App, Entity, Position, Update};

    fn spawn_status_actor(app: &mut App) -> Entity {
        app.world_mut()
            .spawn((StatusEffects::default(), DerivedAttrs::default()))
            .id()
    }

    #[test]
    fn status_effect_apply_refreshes_existing_effect_instead_of_stacking_duplicate() {
        let mut app = App::new();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_systems(Update, status_effect_apply_tick);

        let entity = spawn_status_actor(&mut app);
        app.world_mut().send_event(ApplyStatusEffectIntent {
            target: entity,
            kind: StatusEffectKind::Bleeding,
            magnitude: 0.4,
            duration_ticks: 20,
            issued_at_tick: 1,
        });
        app.world_mut().send_event(ApplyStatusEffectIntent {
            target: entity,
            kind: StatusEffectKind::Bleeding,
            magnitude: 0.6,
            duration_ticks: 40,
            issued_at_tick: 2,
        });

        app.update();

        let status_effects = app.world().entity(entity).get::<StatusEffects>().unwrap();
        assert_eq!(status_effects.active.len(), 1);
        assert_eq!(status_effects.active[0].kind, StatusEffectKind::Bleeding);
        assert_eq!(status_effects.active[0].magnitude, 0.6);
        assert_eq!(status_effects.active[0].remaining_ticks, 40);
    }

    #[test]
    fn zero_duration_status_intent_dispels_existing_effect() {
        let mut app = App::new();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_systems(Update, status_effect_apply_tick);

        let entity = app
            .world_mut()
            .spawn(StatusEffects {
                active: vec![crate::combat::components::ActiveStatusEffect {
                    kind: StatusEffectKind::VortexCasting,
                    magnitude: 1.0,
                    remaining_ticks: u64::MAX,
                    source_pill: None,
                }],
            })
            .id();
        app.world_mut().send_event(ApplyStatusEffectIntent {
            target: entity,
            kind: StatusEffectKind::VortexCasting,
            magnitude: 0.0,
            duration_ticks: 0,
            issued_at_tick: 10,
        });

        app.update();

        assert!(app
            .world()
            .entity(entity)
            .get::<StatusEffects>()
            .unwrap()
            .active
            .is_empty());
    }

    #[test]
    fn immobilized_status_intent_applies_and_has_active_status_reports_it() {
        let mut app = App::new();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_systems(Update, status_effect_apply_tick);

        let entity = spawn_status_actor(&mut app);
        app.world_mut().send_event(ApplyStatusEffectIntent {
            target: entity,
            kind: StatusEffectKind::Immobilized,
            magnitude: 1.0,
            duration_ticks: 40,
            issued_at_tick: 10,
        });
        app.update();

        let status_effects = app.world().entity(entity).get::<StatusEffects>().unwrap();
        assert!(
            has_active_status(status_effects, StatusEffectKind::Immobilized),
            "Immobilized must be visible to explicit consumers such as NPC navigator"
        );
    }

    #[test]
    fn immobilized_status_intent_expires_through_shared_lifecycle() {
        let mut app = App::new();
        app.insert_resource(CombatClock {
            tick: STATUS_EFFECT_TICK_INTERVAL_TICKS,
        });
        app.add_systems(Update, status_effect_tick);

        let entity = app
            .world_mut()
            .spawn(StatusEffects {
                active: vec![crate::combat::components::ActiveStatusEffect {
                    kind: StatusEffectKind::Immobilized,
                    magnitude: 1.0,
                    remaining_ticks: STATUS_EFFECT_TICK_INTERVAL_TICKS,
                    source_pill: None,
                }],
            })
            .id();
        app.update();

        let status_effects = app.world().entity(entity).get::<StatusEffects>().unwrap();
        assert!(
            !has_active_status(status_effects, StatusEffectKind::Immobilized),
            "Immobilized should expire through the same status tick lifecycle as other statuses"
        );
    }

    #[test]
    fn immobilized_zero_duration_dispels_and_non_positive_magnitude_is_ignored() {
        let mut app = App::new();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_systems(Update, status_effect_apply_tick);

        let entity = app
            .world_mut()
            .spawn(StatusEffects {
                active: vec![crate::combat::components::ActiveStatusEffect {
                    kind: StatusEffectKind::Immobilized,
                    magnitude: 1.0,
                    remaining_ticks: 40,
                    source_pill: None,
                }],
            })
            .id();
        app.world_mut().send_event(ApplyStatusEffectIntent {
            target: entity,
            kind: StatusEffectKind::Immobilized,
            magnitude: 0.0,
            duration_ticks: 0,
            issued_at_tick: 11,
        });
        app.update();
        assert!(
            app.world()
                .entity(entity)
                .get::<StatusEffects>()
                .unwrap()
                .active
                .is_empty(),
            "zero-duration Immobilized intent must remove an existing immobilize control"
        );

        app.world_mut().send_event(ApplyStatusEffectIntent {
            target: entity,
            kind: StatusEffectKind::Immobilized,
            magnitude: 0.0,
            duration_ticks: 40,
            issued_at_tick: 12,
        });
        app.update();
        assert!(
            app.world()
                .entity(entity)
                .get::<StatusEffects>()
                .unwrap()
                .active
                .is_empty(),
            "non-positive magnitude Immobilized intent must not create an inert active status"
        );
    }

    #[test]
    fn status_effect_tick_expires_effect_after_duration() {
        let mut app = App::new();
        app.insert_resource(CombatClock {
            tick: STATUS_EFFECT_TICK_INTERVAL_TICKS,
        });
        app.add_systems(Update, status_effect_tick);

        let entity = app
            .world_mut()
            .spawn(StatusEffects {
                active: vec![crate::combat::components::ActiveStatusEffect {
                    kind: StatusEffectKind::Bleeding,
                    magnitude: 0.5,
                    remaining_ticks: STATUS_EFFECT_TICK_INTERVAL_TICKS,
                    source_pill: None,
                }],
            })
            .id();

        app.update();

        let status_effects = app.world().entity(entity).get::<StatusEffects>().unwrap();
        assert!(status_effects.active.is_empty());
    }

    #[test]
    fn slowed_effect_aggregates_into_move_speed_multiplier() {
        let mut app = App::new();
        app.add_systems(Update, attribute_aggregate_tick);

        let entity = app
            .world_mut()
            .spawn((
                StatusEffects {
                    active: vec![crate::combat::components::ActiveStatusEffect {
                        kind: StatusEffectKind::Slowed,
                        magnitude: 0.4,
                        remaining_ticks: 20,
                        source_pill: None,
                    }],
                },
                DerivedAttrs::default(),
            ))
            .id();

        app.update();

        let attrs = app.world().entity(entity).get::<DerivedAttrs>().unwrap();
        assert_eq!(attrs.move_speed_multiplier, 0.6);
    }

    #[test]
    fn vortex_casting_clamps_move_speed_to_twenty_percent() {
        let mut app = App::new();
        app.add_systems(Update, attribute_aggregate_tick);

        let entity = app
            .world_mut()
            .spawn((
                StatusEffects {
                    active: vec![crate::combat::components::ActiveStatusEffect {
                        kind: StatusEffectKind::VortexCasting,
                        magnitude: 1.0,
                        remaining_ticks: 20,
                        source_pill: None,
                    }],
                },
                DerivedAttrs::default(),
            ))
            .id();

        app.update();

        let attrs = app.world().entity(entity).get::<DerivedAttrs>().unwrap();
        assert_eq!(attrs.move_speed_multiplier, 0.2);
    }

    #[test]
    fn parry_recovery_stacks_with_slowed_move_speed_multiplier() {
        let mut app = App::new();
        app.add_systems(Update, attribute_aggregate_tick);

        let entity = app
            .world_mut()
            .spawn((
                StatusEffects {
                    active: vec![
                        crate::combat::components::ActiveStatusEffect {
                            kind: StatusEffectKind::Slowed,
                            magnitude: 0.4,
                            remaining_ticks: 20,
                            source_pill: None,
                        },
                        crate::combat::components::ActiveStatusEffect {
                            kind: StatusEffectKind::ParryRecovery,
                            magnitude: 1.0,
                            remaining_ticks: 10,
                            source_pill: None,
                        },
                    ],
                },
                DerivedAttrs::default(),
            ))
            .id();

        app.update();

        let attrs = app.world().entity(entity).get::<DerivedAttrs>().unwrap();
        assert!((attrs.move_speed_multiplier - 0.42).abs() < 1e-6);
    }

    #[test]
    fn damage_amp_aggregates_into_attack_power() {
        let mut app = App::new();
        app.add_systems(Update, attribute_aggregate_tick);

        let entity = app
            .world_mut()
            .spawn((
                StatusEffects {
                    active: vec![crate::combat::components::ActiveStatusEffect {
                        kind: StatusEffectKind::DamageAmp,
                        magnitude: 0.25,
                        remaining_ticks: 20,
                        source_pill: None,
                    }],
                },
                DerivedAttrs::default(),
            ))
            .id();

        app.update();

        let attrs = app.world().entity(entity).get::<DerivedAttrs>().unwrap();
        assert_eq!(attrs.attack_power, 1.25);
    }

    #[test]
    fn damage_reduction_aggregates_into_defense_power() {
        let mut app = App::new();
        app.add_systems(Update, attribute_aggregate_tick);

        let entity = app
            .world_mut()
            .spawn((
                StatusEffects {
                    active: vec![crate::combat::components::ActiveStatusEffect {
                        kind: StatusEffectKind::DamageReduction,
                        magnitude: 0.25,
                        remaining_ticks: 20,
                        source_pill: None,
                    }],
                },
                DerivedAttrs::default(),
            ))
            .id();

        app.update();

        let attrs = app.world().entity(entity).get::<DerivedAttrs>().unwrap();
        assert_eq!(attrs.defense_power, 0.75);
    }

    #[test]
    fn body_refining_reduces_damage_via_defense_power() {
        let mut app = App::new();
        app.add_systems(Update, attribute_aggregate_tick);

        let entity = app
            .world_mut()
            .spawn((
                StatusEffects::default(),
                DerivedAttrs::default(),
                BodyRefiningMarker,
            ))
            .id();
        app.update();

        let attrs = app.world().entity(entity).get::<DerivedAttrs>().unwrap();
        // 1.0 / 1.3 ≈ 0.769
        assert!((attrs.defense_power - 0.769).abs() < 0.01);
    }

    #[test]
    fn exhausted_defense_modifier_is_halved_once() {
        let mut app = App::new();
        app.add_systems(Update, attribute_aggregate_tick);

        let entity = app
            .world_mut()
            .spawn((
                StatusEffects {
                    active: vec![crate::combat::components::ActiveStatusEffect {
                        kind: StatusEffectKind::DamageReduction,
                        magnitude: 0.25,
                        remaining_ticks: 20,
                        source_pill: None,
                    }],
                },
                DerivedAttrs::default(),
                Exhausted::from_committed_qi(10, 100.0),
            ))
            .id();

        app.update();

        let attrs = app.world().entity(entity).get::<DerivedAttrs>().unwrap();
        assert_eq!(attrs.defense_power, 0.375);
    }

    #[test]
    fn sum_breakthrough_boost_accumulates_and_ignores_other_kinds() {
        let status_effects = StatusEffects {
            active: vec![
                crate::combat::components::ActiveStatusEffect {
                    kind: StatusEffectKind::BreakthroughBoost,
                    magnitude: 0.12,
                    remaining_ticks: 100,
                    source_pill: None,
                },
                crate::combat::components::ActiveStatusEffect {
                    kind: StatusEffectKind::BreakthroughBoost,
                    magnitude: 0.05,
                    remaining_ticks: 50,
                    source_pill: None,
                },
                crate::combat::components::ActiveStatusEffect {
                    kind: StatusEffectKind::DamageAmp,
                    magnitude: 0.25,
                    remaining_ticks: 100,
                    source_pill: None,
                },
                crate::combat::components::ActiveStatusEffect {
                    kind: StatusEffectKind::BreakthroughBoost,
                    magnitude: 0.20,
                    remaining_ticks: 0,
                    source_pill: None, // 过期，不计入
                },
            ],
        };
        assert!((sum_breakthrough_boost(&status_effects) - 0.17).abs() < 1e-6);
    }

    #[test]
    fn clear_breakthrough_boost_removes_only_target_kind() {
        let mut status_effects = StatusEffects {
            active: vec![
                crate::combat::components::ActiveStatusEffect {
                    kind: StatusEffectKind::BreakthroughBoost,
                    magnitude: 0.1,
                    remaining_ticks: 100,
                    source_pill: None,
                },
                crate::combat::components::ActiveStatusEffect {
                    kind: StatusEffectKind::Bleeding,
                    magnitude: 0.4,
                    remaining_ticks: 50,
                    source_pill: None,
                },
            ],
        };
        clear_breakthrough_boost(&mut status_effects);
        assert_eq!(status_effects.active.len(), 1);
        assert_eq!(status_effects.active[0].kind, StatusEffectKind::Bleeding);
    }

    #[test]
    fn health_regen_boost_multiplier_defaults_to_one_without_active_buff() {
        let status_effects = StatusEffects {
            active: vec![
                crate::combat::components::ActiveStatusEffect {
                    kind: StatusEffectKind::HealthRegenBoost,
                    magnitude: 3.0,
                    remaining_ticks: 0,
                    source_pill: None,
                },
                crate::combat::components::ActiveStatusEffect {
                    kind: StatusEffectKind::StaminaRecovBoost,
                    magnitude: 2.0,
                    remaining_ticks: 20,
                    source_pill: None,
                },
            ],
        };

        assert_eq!(health_regen_boost_multiplier(&status_effects), 1.0);
    }

    #[test]
    fn health_regen_boost_multiplier_stacks_positive_magnitude_and_ignores_negative() {
        let status_effects = StatusEffects {
            active: vec![
                crate::combat::components::ActiveStatusEffect {
                    kind: StatusEffectKind::HealthRegenBoost,
                    magnitude: 0.5,
                    remaining_ticks: 20,
                    source_pill: None,
                },
                crate::combat::components::ActiveStatusEffect {
                    kind: StatusEffectKind::HealthRegenBoost,
                    magnitude: 1.0,
                    remaining_ticks: 20,
                    source_pill: None,
                },
                crate::combat::components::ActiveStatusEffect {
                    kind: StatusEffectKind::HealthRegenBoost,
                    magnitude: -0.75,
                    remaining_ticks: 20,
                    source_pill: None,
                },
            ],
        };

        assert!((health_regen_boost_multiplier(&status_effects) - 3.0).abs() < 1e-6);
    }

    #[test]
    fn health_regen_boost_multiplier_caps_runaway_stack() {
        let status_effects = StatusEffects {
            active: vec![
                crate::combat::components::ActiveStatusEffect {
                    kind: StatusEffectKind::HealthRegenBoost,
                    magnitude: 9.0,
                    remaining_ticks: 20,
                    source_pill: None,
                },
                crate::combat::components::ActiveStatusEffect {
                    kind: StatusEffectKind::HealthRegenBoost,
                    magnitude: 9.0,
                    remaining_ticks: 20,
                    source_pill: None,
                },
            ],
        };

        assert_eq!(
            health_regen_boost_multiplier(&status_effects),
            MAX_HEALTH_REGEN_BOOST_MULTIPLIER
        );
    }

    #[test]
    fn health_regen_boost_multiplier_returns_to_one_after_expiry() {
        let mut app = App::new();
        app.insert_resource(CombatClock {
            tick: STATUS_EFFECT_TICK_INTERVAL_TICKS,
        });
        app.add_systems(Update, status_effect_tick);

        let entity = app
            .world_mut()
            .spawn(StatusEffects {
                active: vec![crate::combat::components::ActiveStatusEffect {
                    kind: StatusEffectKind::HealthRegenBoost,
                    magnitude: 0.5,
                    remaining_ticks: STATUS_EFFECT_TICK_INTERVAL_TICKS,
                    source_pill: None,
                }],
            })
            .id();

        app.update();

        let status_effects = app.world().entity(entity).get::<StatusEffects>().unwrap();
        assert!(status_effects.active.is_empty());
        assert_eq!(health_regen_boost_multiplier(status_effects), 1.0);
    }

    #[test]
    fn has_active_status_respects_kind_and_remaining_ticks() {
        let status_effects = StatusEffects {
            active: vec![
                crate::combat::components::ActiveStatusEffect {
                    kind: StatusEffectKind::Stunned,
                    magnitude: 1.0,
                    remaining_ticks: 20,
                    source_pill: None,
                },
                crate::combat::components::ActiveStatusEffect {
                    kind: StatusEffectKind::Slowed,
                    magnitude: 0.4,
                    remaining_ticks: 0,
                    source_pill: None,
                },
            ],
        };

        assert!(has_active_status(
            &status_effects,
            StatusEffectKind::Stunned
        ));
        assert!(!has_active_status(
            &status_effects,
            StatusEffectKind::Slowed
        ));
    }

    #[test]
    fn stunned_effect_expires_after_duration() {
        let mut app = App::new();
        app.insert_resource(CombatClock {
            tick: STATUS_EFFECT_TICK_INTERVAL_TICKS,
        });
        app.add_systems(Update, status_effect_tick);

        let entity = app
            .world_mut()
            .spawn(StatusEffects {
                active: vec![crate::combat::components::ActiveStatusEffect {
                    kind: StatusEffectKind::Stunned,
                    magnitude: 1.0,
                    remaining_ticks: STATUS_EFFECT_TICK_INTERVAL_TICKS,
                    source_pill: None,
                }],
            })
            .id();

        app.update();

        let status_effects = app.world().entity(entity).get::<StatusEffects>().unwrap();
        assert!(status_effects.active.is_empty());
    }

    #[test]
    fn body_part_damage_multiplier_combines_active_resist_and_weaken() {
        let status_effects = StatusEffects {
            active: vec![
                crate::combat::components::ActiveStatusEffect {
                    kind: StatusEffectKind::BodyPartResist(BodyPart::Chest),
                    magnitude: 0.40,
                    remaining_ticks: 20,
                    source_pill: None,
                },
                crate::combat::components::ActiveStatusEffect {
                    kind: StatusEffectKind::BodyPartWeaken(BodyPart::Chest),
                    magnitude: 0.25,
                    remaining_ticks: 20,
                    source_pill: None,
                },
                crate::combat::components::ActiveStatusEffect {
                    kind: StatusEffectKind::BodyPartWeaken(BodyPart::Chest),
                    magnitude: 0.50,
                    remaining_ticks: 0,
                    source_pill: None,
                },
            ],
        };

        assert!(
            (body_part_damage_multiplier(Some(&status_effects), BodyPart::Chest) - 0.75).abs()
                < 1e-6
        );
        assert_eq!(
            body_part_damage_multiplier(Some(&status_effects), BodyPart::ArmL),
            1.0
        );
    }

    #[test]
    fn combat_pill_stamina_status_tick_applies_recovery_and_qi_drain() {
        let mut app = App::new();
        app.insert_resource(CombatClock {
            tick: STATUS_EFFECT_TICK_INTERVAL_TICKS,
        });
        app.insert_resource(ZoneRegistry::fallback());
        app.add_event::<crate::qi_physics::QiTransfer>();
        app.add_systems(Update, combat_pill_stamina_status_tick);
        let zone_before = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .expect("spawn zone should exist")
            .spirit_qi;

        let entity = app
            .world_mut()
            .spawn((
                StatusEffects {
                    active: vec![
                        crate::combat::components::ActiveStatusEffect {
                            kind: StatusEffectKind::StaminaRecovBoost,
                            magnitude: 3.0,
                            remaining_ticks: 20,
                            source_pill: None,
                        },
                        crate::combat::components::ActiveStatusEffect {
                            kind: StatusEffectKind::QiDrainForStamina,
                            magnitude: 2.0,
                            remaining_ticks: 20,
                            source_pill: None,
                        },
                    ],
                },
                Stamina {
                    current: 40.0,
                    max: 100.0,
                    recover_per_sec: 5.0,
                    last_drain_tick: None,
                    state: StaminaState::Idle,
                },
                Cultivation {
                    qi_current: 10.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                Position::new([0.0, 64.0, 0.0]),
                CurrentDimension(DimensionKind::Overworld),
            ))
            .id();

        app.update();

        let stamina = app.world().entity(entity).get::<Stamina>().unwrap();
        assert_eq!(stamina.max, 100.0);
        assert_eq!(stamina.recover_per_sec, 15.0);
        let cultivation = app.world().entity(entity).get::<Cultivation>().unwrap();
        assert!((cultivation.qi_current - 9.6).abs() < 1e-6);
        let transfers: Vec<_> = app
            .world()
            .resource::<valence::prelude::Events<crate::qi_physics::QiTransfer>>()
            .iter_current_update_events()
            .cloned()
            .collect();
        assert_eq!(transfers.len(), 1);
        assert!((transfers[0].amount - 0.4).abs() < 1e-6);
        let zone_after = app
            .world()
            .resource::<ZoneRegistry>()
            .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
            .expect("spawn zone should exist")
            .spirit_qi;
        let zone_delta = (zone_after - zone_before) * QI_ZONE_UNIT_CAPACITY;
        assert!(
            (zone_delta - 0.4).abs() < 1e-6,
            "QiDrainForStamina must credit drained qi into the current zone; delta={zone_delta}"
        );
    }

    #[test]
    fn combat_pill_stamina_status_tick_resets_expired_pill_adjustments() {
        let mut app = App::new();
        app.insert_resource(CombatClock {
            tick: STATUS_EFFECT_TICK_INTERVAL_TICKS,
        });
        app.add_systems(Update, combat_pill_stamina_status_tick);

        let entity = app
            .world_mut()
            .spawn((
                StatusEffects {
                    active: vec![crate::combat::components::ActiveStatusEffect {
                        kind: StatusEffectKind::StaminaRecovBoost,
                        magnitude: 0.5,
                        remaining_ticks: 0,
                        source_pill: None,
                    }],
                },
                Stamina {
                    current: 140.0,
                    max: 150.0,
                    recover_per_sec: 15.0,
                    last_drain_tick: None,
                    state: StaminaState::Idle,
                },
            ))
            .id();

        app.update();

        let stamina = app.world().entity(entity).get::<Stamina>().unwrap();
        assert_eq!(stamina.max, 100.0);
        assert_eq!(stamina.current, 100.0);
        assert_eq!(stamina.recover_per_sec, 5.0);
    }

    // ── plan-cultivation-pacing-v1 push_status_effect 测试 ──

    fn pill_effect(pill: &str, magnitude: f32) -> ActiveStatusEffect {
        ActiveStatusEffect {
            kind: StatusEffectKind::CultivationAcceleration,
            magnitude,
            remaining_ticks: 100,
            source_pill: Some(pill.to_string()),
        }
    }

    #[test]
    fn push_status_effect_same_pill_third_rejected() {
        let mut se = StatusEffects::default();
        assert!(push_status_effect(&mut se, pill_effect("pill_a", 0.5)));
        assert!(push_status_effect(&mut se, pill_effect("pill_a", 0.5)));
        assert!(
            !push_status_effect(&mut se, pill_effect("pill_a", 0.5)),
            "同种丹药第 3 颗应被拒绝"
        );
        assert_eq!(se.active.len(), 2);
    }

    #[test]
    fn push_status_effect_different_pill_not_blocked() {
        let mut se = StatusEffects::default();
        assert!(push_status_effect(&mut se, pill_effect("pill_a", 0.5)));
        assert!(push_status_effect(&mut se, pill_effect("pill_a", 0.5)));
        assert!(
            push_status_effect(&mut se, pill_effect("pill_b", 0.5)),
            "不同丹药不应被 per-pill cap 拦截"
        );
        assert_eq!(se.active.len(), 3);
    }

    #[test]
    fn push_status_effect_none_source_pill_unlimited() {
        let mut se = StatusEffects::default();
        for _ in 0..5 {
            let effect = ActiveStatusEffect {
                kind: StatusEffectKind::CultivationAcceleration,
                magnitude: 0.5,
                remaining_ticks: 100,
                source_pill: None,
            };
            assert!(
                push_status_effect(&mut se, effect),
                "source_pill=None 不应受 per-pill cap 限制"
            );
        }
        assert_eq!(se.active.len(), 5);
    }

    #[test]
    fn push_status_effect_expired_same_pill_not_counted() {
        let mut se = StatusEffects {
            active: vec![
                ActiveStatusEffect {
                    kind: StatusEffectKind::CultivationAcceleration,
                    magnitude: 0.5,
                    remaining_ticks: 0, // 已过期
                    source_pill: Some("ling_xi_wan".to_string()),
                },
                ActiveStatusEffect {
                    kind: StatusEffectKind::CultivationAcceleration,
                    magnitude: 0.5,
                    remaining_ticks: 0, // 已过期
                    source_pill: Some("ling_xi_wan".to_string()),
                },
            ],
        };
        // 即使有 2 条同 pill 但都过期，新的应该能 push 成功
        let ok = push_status_effect(
            &mut se,
            ActiveStatusEffect {
                kind: StatusEffectKind::CultivationAcceleration,
                magnitude: 0.5,
                remaining_ticks: 100,
                source_pill: Some("ling_xi_wan".to_string()),
            },
        );
        assert!(ok, "过期的同种丹药不应计入 per-pill cap");
    }

    #[test]
    fn push_status_effect_boundary_exactly_two_same_pill() {
        let mut se = StatusEffects::default();
        assert!(push_status_effect(&mut se, pill_effect("x", 1.0)));
        assert!(push_status_effect(&mut se, pill_effect("x", 2.0)));
        // 第三颗被拒
        assert!(!push_status_effect(&mut se, pill_effect("x", 3.0)));
        // 确认已有的两颗 magnitude 正确（push 不合并，直接入栈）
        assert_eq!(se.active[0].magnitude, 1.0);
        assert_eq!(se.active[1].magnitude, 2.0);
    }

    // ── plan-cultivation-pacing-v1 DamageVulnerability 消费侧测试 ──

    #[test]
    fn damage_vulnerability_doubles_defense_power() {
        let mut app = App::new();
        app.add_systems(Update, attribute_aggregate_tick);

        let entity = app
            .world_mut()
            .spawn((
                StatusEffects {
                    active: vec![crate::combat::components::ActiveStatusEffect {
                        kind: StatusEffectKind::DamageVulnerability,
                        magnitude: 1.0,
                        remaining_ticks: 20,
                        source_pill: None,
                    }],
                },
                DerivedAttrs::default(),
            ))
            .id();

        app.update();

        let attrs = app.world().entity(entity).get::<DerivedAttrs>().unwrap();
        // 基线 defense_power=1.0，乘以 (1+1.0)=2.0
        assert!(
            (attrs.defense_power - 2.0).abs() < 1e-6,
            "DamageVulnerability(mag=1.0) 应使 defense_power=2.0（受伤翻倍）；\
             实际 {:.6}",
            attrs.defense_power
        );
    }

    #[test]
    fn damage_vulnerability_stacks_with_damage_reduction() {
        let mut app = App::new();
        app.add_systems(Update, attribute_aggregate_tick);

        let entity = app
            .world_mut()
            .spawn((
                StatusEffects {
                    active: vec![
                        crate::combat::components::ActiveStatusEffect {
                            kind: StatusEffectKind::DamageReduction,
                            magnitude: 0.5,
                            remaining_ticks: 20,
                            source_pill: None,
                        },
                        crate::combat::components::ActiveStatusEffect {
                            kind: StatusEffectKind::DamageVulnerability,
                            magnitude: 1.0,
                            remaining_ticks: 20,
                            source_pill: None,
                        },
                    ],
                },
                DerivedAttrs::default(),
            ))
            .id();

        app.update();

        let attrs = app.world().entity(entity).get::<DerivedAttrs>().unwrap();
        // DamageReduction(0.5) → defense_power=0.5
        // DamageVulnerability(1.0) → defense_power *= 2.0 → 1.0
        assert!(
            (attrs.defense_power - 1.0).abs() < 1e-6,
            "DamageReduction(0.5) + DamageVulnerability(1.0) 应回到 1.0；\
             实际 {:.6}",
            attrs.defense_power
        );
    }

    #[test]
    fn damage_vulnerability_expired_not_applied() {
        let mut app = App::new();
        app.add_systems(Update, attribute_aggregate_tick);

        let entity = app
            .world_mut()
            .spawn((
                StatusEffects {
                    active: vec![crate::combat::components::ActiveStatusEffect {
                        kind: StatusEffectKind::DamageVulnerability,
                        magnitude: 1.0,
                        remaining_ticks: 0,
                        source_pill: None,
                    }],
                },
                DerivedAttrs::default(),
            ))
            .id();

        app.update();

        let attrs = app.world().entity(entity).get::<DerivedAttrs>().unwrap();
        assert!(
            (attrs.defense_power - 1.0).abs() < 1e-6,
            "过期的 DamageVulnerability 不应影响 defense_power；实际 {:.6}",
            attrs.defense_power
        );
    }

    #[test]
    fn no_damage_vulnerability_preserves_baseline() {
        let mut app = App::new();
        app.add_systems(Update, attribute_aggregate_tick);

        let entity = app
            .world_mut()
            .spawn((StatusEffects::default(), DerivedAttrs::default()))
            .id();

        app.update();

        let attrs = app.world().entity(entity).get::<DerivedAttrs>().unwrap();
        assert!(
            (attrs.defense_power - 1.0).abs() < 1e-6,
            "无 DamageVulnerability 时 defense_power 应为 1.0；实际 {:.6}",
            attrs.defense_power
        );
    }

    // ── bughunt r4-P2#7：clear_du_jie_dan_damage_reduction 测试 ──

    /// 渡劫结束后，source_pill="du_jie_dan" 的 DamageReduction 必须被清除。
    #[test]
    fn clear_du_jie_dan_damage_reduction_removes_du_jie_dan_source() {
        let mut se = StatusEffects {
            active: vec![
                ActiveStatusEffect {
                    kind: StatusEffectKind::BreakthroughBoost,
                    magnitude: 0.25,
                    remaining_ticks: u64::MAX,
                    source_pill: Some("du_jie_dan".to_string()),
                },
                ActiveStatusEffect {
                    kind: StatusEffectKind::DamageReduction,
                    magnitude: 0.30,
                    remaining_ticks: u64::MAX,
                    source_pill: Some("du_jie_dan".to_string()),
                },
            ],
        };
        super::clear_du_jie_dan_damage_reduction(&mut se);
        assert_eq!(
            se.active.len(),
            1,
            "期望 DamageReduction(du_jie_dan) 被清除后只剩 1 条；实际 {}",
            se.active.len()
        );
        assert_eq!(
            se.active[0].kind,
            StatusEffectKind::BreakthroughBoost,
            "期望保留 BreakthroughBoost，实际 {:?}",
            se.active[0].kind
        );
    }

    /// 其它来源的 DamageReduction（装备/技能）不应被渡劫丹清除函数误清。
    #[test]
    fn clear_du_jie_dan_damage_reduction_preserves_other_source_damage_reduction() {
        let mut se = StatusEffects {
            active: vec![
                // 渡劫丹 DamageReduction（应被清）
                ActiveStatusEffect {
                    kind: StatusEffectKind::DamageReduction,
                    magnitude: 0.30,
                    remaining_ticks: u64::MAX,
                    source_pill: Some("du_jie_dan".to_string()),
                },
                // 其他丹药来源的 DamageReduction（应保留）
                ActiveStatusEffect {
                    kind: StatusEffectKind::DamageReduction,
                    magnitude: 0.15,
                    remaining_ticks: 200,
                    source_pill: Some("some_other_pill".to_string()),
                },
                // 无 source_pill 的 DamageReduction（如技能 buff，应保留）
                ActiveStatusEffect {
                    kind: StatusEffectKind::DamageReduction,
                    magnitude: 0.10,
                    remaining_ticks: 100,
                    source_pill: None,
                },
            ],
        };
        super::clear_du_jie_dan_damage_reduction(&mut se);
        assert_eq!(
            se.active.len(),
            2,
            "期望只清除 du_jie_dan 来源的 DamageReduction，保留另外 2 条；实际 {} 条",
            se.active.len()
        );
        for effect in &se.active {
            assert_ne!(
                effect.source_pill.as_deref(),
                Some("du_jie_dan"),
                "不应保留 source_pill=du_jie_dan 的条目，实际保留了 {:?}",
                effect
            );
        }
        // 验证保留了正确数量的 DamageReduction
        let remaining_dr: Vec<_> = se
            .active
            .iter()
            .filter(|e| e.kind == StatusEffectKind::DamageReduction)
            .collect();
        assert_eq!(
            remaining_dr.len(),
            2,
            "期望保留 2 条非渡劫丹来源的 DamageReduction；实际 {}",
            remaining_dr.len()
        );
    }

    /// 渡劫前（buff 还在时），DamageReduction 正常生效产生减伤。
    #[test]
    fn du_jie_dan_damage_reduction_active_before_clear_reduces_defense_power() {
        let mut app = App::new();
        app.add_systems(Update, attribute_aggregate_tick);

        let entity = app
            .world_mut()
            .spawn((
                StatusEffects {
                    active: vec![ActiveStatusEffect {
                        kind: StatusEffectKind::DamageReduction,
                        magnitude: 0.30,
                        remaining_ticks: u64::MAX,
                        source_pill: Some("du_jie_dan".to_string()),
                    }],
                },
                DerivedAttrs::default(),
            ))
            .id();

        app.update();

        let attrs = app.world().entity(entity).get::<DerivedAttrs>().unwrap();
        assert!(
            (attrs.defense_power - 0.70).abs() < 1e-6,
            "渡劫中 DamageReduction(0.30) 应使 defense_power=0.70；实际 {:.6}",
            attrs.defense_power
        );
    }

    /// 渡劫结束清除后，defense_power 恢复到基线 1.0，确认无永久减伤泄漏。
    #[test]
    fn du_jie_dan_damage_reduction_cleared_after_breakthrough_restores_defense_power() {
        let mut app = App::new();
        app.add_systems(Update, attribute_aggregate_tick);

        let entity = app
            .world_mut()
            .spawn((
                StatusEffects {
                    active: vec![ActiveStatusEffect {
                        kind: StatusEffectKind::DamageReduction,
                        magnitude: 0.30,
                        remaining_ticks: u64::MAX,
                        source_pill: Some("du_jie_dan".to_string()),
                    }],
                },
                DerivedAttrs::default(),
            ))
            .id();

        // 模拟 breakthrough settle：调用 clear_du_jie_dan_damage_reduction
        {
            let world = app.world_mut();
            let mut entity_mut = world.entity_mut(entity);
            let mut se = entity_mut.get_mut::<StatusEffects>().unwrap();
            super::clear_du_jie_dan_damage_reduction(&mut se);
        }

        app.update();

        let attrs = app.world().entity(entity).get::<DerivedAttrs>().unwrap();
        assert!(
            (attrs.defense_power - 1.0).abs() < 1e-6,
            "渡劫结束后 clear_du_jie_dan_damage_reduction 应使 defense_power 回到 1.0（无永久减伤泄漏）；\
             实际 {:.6}",
            attrs.defense_power
        );
    }

    /// clear_du_jie_dan_damage_reduction 在空列表上不 panic，且不影响其他 kind。
    #[test]
    fn clear_du_jie_dan_damage_reduction_empty_and_no_du_jie_dan_entry_is_noop() {
        // 空列表
        let mut se_empty = StatusEffects::default();
        super::clear_du_jie_dan_damage_reduction(&mut se_empty);
        assert!(se_empty.active.is_empty(), "空列表清除后应仍为空");

        // 无渡劫丹条目，但有其他 kind
        let mut se_other = StatusEffects {
            active: vec![
                ActiveStatusEffect {
                    kind: StatusEffectKind::Bleeding,
                    magnitude: 0.5,
                    remaining_ticks: 50,
                    source_pill: None,
                },
                ActiveStatusEffect {
                    kind: StatusEffectKind::BreakthroughBoost,
                    magnitude: 0.20,
                    remaining_ticks: u64::MAX,
                    source_pill: Some("po_jing_dan".to_string()),
                },
            ],
        };
        super::clear_du_jie_dan_damage_reduction(&mut se_other);
        assert_eq!(
            se_other.active.len(),
            2,
            "无渡劫丹 DamageReduction 时 clear 应为 noop，保留 2 条；实际 {}",
            se_other.active.len()
        );
    }

    // ── bughunt r4-P2#5：QiCapPermMinus 消费侧 pin 测试 ──

    fn qi_cap_minus_effect(magnitude: f32, remaining_ticks: u64) -> ActiveStatusEffect {
        ActiveStatusEffect {
            kind: StatusEffectKind::QiCapPermMinus,
            magnitude,
            remaining_ticks,
            source_pill: None,
        }
    }

    /// QiCapPermMinus(mag=0.01) 应使 qi_max_multiplier = 0.99（折损 1%）。
    #[test]
    fn qi_cap_perm_minus_mag_one_percent_sets_multiplier_to_0_99() {
        let mut app = App::new();
        app.add_systems(Update, attribute_aggregate_tick);

        let entity = app
            .world_mut()
            .spawn((
                StatusEffects {
                    active: vec![qi_cap_minus_effect(0.01, u64::MAX)],
                },
                DerivedAttrs::default(),
            ))
            .id();

        app.update();

        let attrs = app.world().entity(entity).get::<DerivedAttrs>().unwrap();
        assert!(
            (attrs.qi_max_multiplier - 0.99).abs() < 1e-6,
            "QiCapPermMinus(mag=0.01) 应使 qi_max_multiplier=0.99（折损 1%）；\
             期望 0.99，实际 {:.6}",
            attrs.qi_max_multiplier
        );
    }

    /// QiCapPermMinus 两层叠加（各 0.01）→ 总折损 0.02 → multiplier=0.98。
    #[test]
    fn qi_cap_perm_minus_stacks_two_entries() {
        let mut app = App::new();
        app.add_systems(Update, attribute_aggregate_tick);

        let entity = app
            .world_mut()
            .spawn((
                StatusEffects {
                    active: vec![
                        qi_cap_minus_effect(0.01, u64::MAX),
                        qi_cap_minus_effect(0.01, u64::MAX),
                    ],
                },
                DerivedAttrs::default(),
            ))
            .id();

        app.update();

        let attrs = app.world().entity(entity).get::<DerivedAttrs>().unwrap();
        assert!(
            (attrs.qi_max_multiplier - 0.98).abs() < 1e-6,
            "QiCapPermMinus×2(mag=0.01 each) 应使 qi_max_multiplier=0.98（折损 2%）；\
             期望 0.98，实际 {:.6}",
            attrs.qi_max_multiplier
        );
    }

    /// remaining_ticks=0（过期）的 QiCapPermMinus 不应纳入计算 → multiplier 保持 1.0。
    #[test]
    fn qi_cap_perm_minus_expired_not_applied() {
        let mut app = App::new();
        app.add_systems(Update, attribute_aggregate_tick);

        let entity = app
            .world_mut()
            .spawn((
                StatusEffects {
                    active: vec![qi_cap_minus_effect(0.01, 0)],
                },
                DerivedAttrs::default(),
            ))
            .id();

        app.update();

        let attrs = app.world().entity(entity).get::<DerivedAttrs>().unwrap();
        assert!(
            (attrs.qi_max_multiplier - 1.0).abs() < 1e-6,
            "remaining_ticks=0 的 QiCapPermMinus 不应计入；期望 multiplier=1.0，实际 {:.6}",
            attrs.qi_max_multiplier
        );
    }

    /// 无 QiCapPermMinus 时 qi_max_multiplier 应为中性值 1.0。
    #[test]
    fn qi_cap_perm_minus_absent_multiplier_is_one() {
        let mut app = App::new();
        app.add_systems(Update, attribute_aggregate_tick);

        let entity = app
            .world_mut()
            .spawn((StatusEffects::default(), DerivedAttrs::default()))
            .id();

        app.update();

        let attrs = app.world().entity(entity).get::<DerivedAttrs>().unwrap();
        assert!(
            (attrs.qi_max_multiplier - 1.0).abs() < 1e-6,
            "无 QiCapPermMinus 时 qi_max_multiplier 应为 1.0；实际 {:.6}",
            attrs.qi_max_multiplier
        );
    }

    /// 极端叠加（sum ≥ 0.99）时 qi_max_multiplier clamp 到最低 0.01（不能清零）。
    #[test]
    fn qi_cap_perm_minus_clamps_to_min_0_01() {
        let mut app = App::new();
        app.add_systems(Update, attribute_aggregate_tick);

        let entity = app
            .world_mut()
            .spawn((
                StatusEffects {
                    active: vec![qi_cap_minus_effect(0.99, u64::MAX)],
                },
                DerivedAttrs::default(),
            ))
            .id();

        app.update();

        let attrs = app.world().entity(entity).get::<DerivedAttrs>().unwrap();
        assert!(
            attrs.qi_max_multiplier >= 0.01 - 1e-9,
            "QiCapPermMinus 极端叠加时 qi_max_multiplier 不得低于 0.01；实际 {:.6}",
            attrs.qi_max_multiplier
        );
        assert!(
            attrs.qi_max_multiplier <= 0.01 + 1e-6,
            "mag=0.99 应使 qi_max_multiplier ≈ 0.01；实际 {:.6}",
            attrs.qi_max_multiplier
        );
    }

    /// DerivedAttrs 初始 qi_max_multiplier 是中性 1.0（Default 约束）。
    #[test]
    fn derived_attrs_default_qi_max_multiplier_is_one() {
        let attrs = DerivedAttrs::default();
        assert!(
            (attrs.qi_max_multiplier - 1.0).abs() < 1e-9,
            "DerivedAttrs::default() qi_max_multiplier 应为 1.0（无效果）；实际 {:.9}",
            attrs.qi_max_multiplier
        );
    }
}
