use valence::prelude::{EventReader, Position, Query, Res};

use crate::combat::components::{
    ActiveStatusEffect, BodyPart, BodyRefiningMarker, DerivedAttrs, Stamina, StaminaState,
    StatusEffects, STATUS_EFFECT_TICK_INTERVAL_TICKS,
};
use crate::combat::events::{ApplyStatusEffectIntent, StatusEffectKind};
use crate::combat::CombatClock;
use crate::cultivation::components::Cultivation;
use crate::cultivation::death_hooks::release_qi_amount_to_zone;
use crate::cultivation::life_record::LifeRecord;
use crate::qi_physics::constants::{QI_EPSILON, QI_ZHENMAI_PARRY_RECOVERY_MOVE_SPEED_MULTIPLIER};
use crate::qi_physics::{QiTransfer, WorldQiAccount};
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
    )>,
) {
    for (status_effects, mut attrs, body_refining) in &mut q {
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

        // 虚脱 debuff：防御 ×magnitude（旧 Exhausted.defense_modifier，恒为 0.5）。
        // 全力一击释放后由 ApplyStatusEffectIntent 施加，标准 status 生命周期管理。
        let exhausted_defense_modifier = status_effects
            .active
            .iter()
            .filter(|effect| {
                effect.kind == StatusEffectKind::Exhausted && effect.remaining_ticks > 0
            })
            .fold(1.0_f32, |acc, effect| {
                acc * effect.magnitude.clamp(0.0, 1.0)
            });
        if exhausted_defense_modifier < 1.0 {
            attrs.defense_power =
                (attrs.defense_power * exhausted_defense_modifier).clamp(0.05, 1.0);
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
    mut ledger: valence::prelude::ResMut<WorldQiAccount>,
    mut qi_transfers: Option<valence::prelude::ResMut<valence::prelude::Events<QiTransfer>>>,
) {
    if !clock.tick.is_multiple_of(STATUS_EFFECT_TICK_INTERVAL_TICKS) {
        return;
    }

    let dt = STATUS_EFFECT_TICK_INTERVAL_TICKS as f32
        / crate::combat::components::TICKS_PER_SECOND as f32;
    for (
        _entity,
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
            continue;
        }
        let outcome = release_qi_amount_to_zone(
            &mut cultivation,
            drained,
            position,
            current_dimension,
            life_record,
            zones.as_deref_mut(),
            &mut ledger,
            qi_transfers.as_deref_mut(),
            "combat_pill_stamina_status",
        );
        if let Err(error) = outcome {
            tracing::warn!(
                ?error,
                "[bong][combat] stamina status qi release failed closed"
            );
        }
    }
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
