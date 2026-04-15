use valence::prelude::{EventReader, Query, Res};

use crate::combat::components::{DerivedAttrs, StatusEffects, STATUS_EFFECT_TICK_INTERVAL_TICKS};
use crate::combat::events::{ApplyStatusEffectIntent, StatusEffectKind};
use crate::combat::CombatClock;

pub fn status_effect_apply_tick(
    mut intents: EventReader<ApplyStatusEffectIntent>,
    mut statuses: Query<&mut StatusEffects>,
) {
    for intent in intents.read() {
        let Ok(mut status_effects) = statuses.get_mut(intent.target) else {
            continue;
        };

        if intent.magnitude <= 0.0 || intent.duration_ticks == 0 {
            continue;
        }

        if let Some(existing) = status_effects
            .active
            .iter_mut()
            .find(|effect| effect.kind == intent.kind)
        {
            existing.magnitude = existing.magnitude.max(intent.magnitude);
            existing.remaining_ticks = existing.remaining_ticks.max(intent.duration_ticks);
            continue;
        }

        status_effects
            .active
            .push(crate::combat::components::ActiveStatusEffect {
                kind: intent.kind,
                magnitude: intent.magnitude,
                remaining_ticks: intent.duration_ticks,
            });
    }
}

pub fn has_active_status(status_effects: &StatusEffects, kind: StatusEffectKind) -> bool {
    status_effects
        .active
        .iter()
        .any(|effect| effect.kind == kind && effect.remaining_ticks > 0)
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
        status_effects
            .active
            .retain(|effect| effect.remaining_ticks > 0);
    }
}

pub fn attribute_aggregate_tick(mut q: Query<(&StatusEffects, &mut DerivedAttrs)>) {
    for (status_effects, mut attrs) in &mut q {
        attrs.attack_power = 1.0;
        attrs.defense_power = 1.0;
        attrs.move_speed_multiplier = 1.0;

        let slow_multiplier = status_effects
            .active
            .iter()
            .filter(|effect| effect.kind == StatusEffectKind::Slowed)
            .fold(1.0, |acc, effect| {
                acc * (1.0 - effect.magnitude.clamp(0.0, 0.95))
            });

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

        attrs.move_speed_multiplier = slow_multiplier.clamp(0.05, 1.0);
        attrs.attack_power = damage_amp_multiplier.max(1.0);
        attrs.defense_power = damage_reduction_multiplier.clamp(0.05, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::combat::components::{
        DerivedAttrs, StatusEffects, STATUS_EFFECT_TICK_INTERVAL_TICKS,
    };
    use crate::combat::events::{ApplyStatusEffectIntent, StatusEffectKind};
    use crate::combat::CombatClock;
    use valence::prelude::{App, Entity, Update};

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
    fn has_active_status_respects_kind_and_remaining_ticks() {
        let status_effects = StatusEffects {
            active: vec![
                crate::combat::components::ActiveStatusEffect {
                    kind: StatusEffectKind::Stunned,
                    magnitude: 1.0,
                    remaining_ticks: 20,
                },
                crate::combat::components::ActiveStatusEffect {
                    kind: StatusEffectKind::Slowed,
                    magnitude: 0.4,
                    remaining_ticks: 0,
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
                }],
            })
            .id();

        app.update();

        let status_effects = app.world().entity(entity).get::<StatusEffects>().unwrap();
        assert!(status_effects.active.is_empty());
    }
}
