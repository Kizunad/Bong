use valence::prelude::{Entity, EventReader, EventWriter, Position, Query, Res};

use crate::combat::CombatClock;
use crate::cultivation::death_hooks::{CultivationDeathTrigger, PlayerRevived, PlayerTerminated};
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::schema::vfx_event::VfxEventPayloadV1;

use super::components::{
    CombatState, Lifecycle, LifecycleState, Stamina, StaminaState, Wounds, ATTACK_STAMINA_COST,
    BLEED_TICK_INTERVAL_TICKS, COMBAT_STATE_TICK_INTERVAL_TICKS, NEAR_DEATH_HEALTH_FRACTION,
    REVIVE_HEALTH_FRACTION, STAMINA_TICK_INTERVAL_TICKS, TICKS_PER_SECOND,
};
use super::events::{CombatEvent, DeathEvent};

const COMBAT_DRAIN_PER_SEC: f32 = 5.0;
const JOG_DRAIN_PER_SEC: f32 = 2.0;
const SPRINT_DRAIN_PER_SEC: f32 = 10.0;
const EXHAUSTED_RECOVER_RATIO: f32 = 0.5;
const EXHAUSTED_EXIT_FRACTION: f32 = 0.3;

type NearDeathQueryItem<'a> = (
    Entity,
    &'a mut Lifecycle,
    Option<&'a mut Wounds>,
    Option<&'a mut Stamina>,
    Option<&'a mut CombatState>,
);

pub fn sync_combat_state_from_events(
    mut events: EventReader<CombatEvent>,
    mut actors: Query<(&mut CombatState, &mut Stamina)>,
) {
    for event in events.read() {
        if let Ok((mut state, mut stamina)) = actors.get_mut(event.attacker) {
            state.refresh_combat_window(event.resolved_at_tick);
            state.last_attack_at_tick = Some(event.resolved_at_tick);
            stamina.current = (stamina.current - ATTACK_STAMINA_COST).clamp(0.0, stamina.max);
            stamina.last_drain_tick = Some(event.resolved_at_tick);
            stamina.state = if stamina.current <= 0.0 {
                StaminaState::Exhausted
            } else {
                StaminaState::Combat
            };
        }

        if let Ok((mut state, mut stamina)) = actors.get_mut(event.target) {
            state.refresh_combat_window(event.resolved_at_tick);
            if stamina.state != StaminaState::Exhausted {
                stamina.state = StaminaState::Combat;
            }
        }
    }
}

pub fn wound_bleed_tick(
    clock: Res<CombatClock>,
    mut deaths: EventWriter<DeathEvent>,
    mut wounded: Query<(Entity, &mut Wounds, Option<&Lifecycle>)>,
) {
    if !clock.tick.is_multiple_of(BLEED_TICK_INTERVAL_TICKS) {
        return;
    }

    for (entity, mut wounds, lifecycle) in &mut wounded {
        if wounds.health_current <= 0.0 {
            continue;
        }
        if lifecycle.is_some_and(|lifecycle| {
            matches!(
                lifecycle.state,
                LifecycleState::NearDeath | LifecycleState::Terminated
            )
        }) {
            continue;
        }

        let total_bleed: f32 = wounds
            .entries
            .iter()
            .map(|entry| entry.bleeding_per_sec.max(0.0))
            .sum();
        if total_bleed <= f32::EPSILON {
            continue;
        }

        let was_alive = wounds.health_current > 0.0;
        wounds.health_current = (wounds.health_current - total_bleed).clamp(0.0, wounds.health_max);
        if was_alive && wounds.health_current <= 0.0 {
            deaths.send(DeathEvent {
                target: entity,
                cause: "bleed_out".to_string(),
                at_tick: clock.tick,
            });
        }
    }
}

pub fn stamina_tick(clock: Res<CombatClock>, mut stamina_q: Query<&mut Stamina>) {
    if !clock.tick.is_multiple_of(STAMINA_TICK_INTERVAL_TICKS) {
        return;
    }

    let dt = STAMINA_TICK_INTERVAL_TICKS as f32 / TICKS_PER_SECOND as f32;
    for mut stamina in &mut stamina_q {
        stamina.max = stamina.max.max(1.0);
        stamina.recover_per_sec = stamina.recover_per_sec.max(0.0);

        let delta_per_sec = match stamina.state {
            StaminaState::Idle | StaminaState::Walking => stamina.recover_per_sec,
            StaminaState::Jogging => stamina.recover_per_sec - JOG_DRAIN_PER_SEC,
            StaminaState::Sprinting => -SPRINT_DRAIN_PER_SEC,
            StaminaState::Combat => -COMBAT_DRAIN_PER_SEC,
            StaminaState::Exhausted => stamina.recover_per_sec * EXHAUSTED_RECOVER_RATIO,
        };

        stamina.current = (stamina.current + delta_per_sec * dt).clamp(0.0, stamina.max);

        if stamina.current <= 0.0
            && matches!(
                stamina.state,
                StaminaState::Sprinting | StaminaState::Combat
            )
        {
            stamina.state = StaminaState::Exhausted;
            continue;
        }

        if stamina.state == StaminaState::Exhausted
            && stamina.current >= stamina.max * EXHAUSTED_EXIT_FRACTION
        {
            stamina.state = StaminaState::Idle;
        }
    }
}

pub fn combat_state_tick(
    clock: Res<CombatClock>,
    mut state_q: Query<(&mut CombatState, Option<&mut Stamina>)>,
) {
    if !clock.tick.is_multiple_of(COMBAT_STATE_TICK_INTERVAL_TICKS) {
        return;
    }

    for (mut state, stamina) in &mut state_q {
        if let Some(window) = state.incoming_window.as_ref() {
            if clock.tick >= window.expires_at_tick() {
                state.incoming_window = None;
            }
        }

        if let Some(until_tick) = state.in_combat_until_tick {
            if clock.tick >= until_tick {
                state.in_combat_until_tick = None;
                if let Some(mut stamina) = stamina {
                    if stamina.state == StaminaState::Combat {
                        stamina.state = if stamina.current <= 0.0 {
                            StaminaState::Exhausted
                        } else {
                            StaminaState::Idle
                        };
                    }
                }
            }
        }
    }
}

pub fn death_arbiter_tick(
    clock: Res<CombatClock>,
    mut death_events: EventReader<DeathEvent>,
    mut cultivation_deaths: EventReader<CultivationDeathTrigger>,
    mut lifecycle_q: Query<(&mut Lifecycle, Option<&mut Wounds>, Option<&mut LifeRecord>)>,
) {
    for event in death_events.read() {
        let Ok((mut lifecycle, wounds, life_record)) = lifecycle_q.get_mut(event.target) else {
            continue;
        };
        if let Some(mut life_record) = life_record {
            life_record.push(BiographyEntry::NearDeath {
                cause: event.cause.clone(),
                tick: event.at_tick.max(clock.tick),
            });
        }
        enter_near_death(&mut lifecycle, wounds, event.at_tick.max(clock.tick));
    }

    for event in cultivation_deaths.read() {
        let Ok((mut lifecycle, wounds, life_record)) = lifecycle_q.get_mut(event.entity) else {
            continue;
        };
        if let Some(mut life_record) = life_record {
            life_record.push(BiographyEntry::NearDeath {
                cause: format!("cultivation:{:?}", event.cause),
                tick: clock.tick,
            });
        }
        enter_near_death(&mut lifecycle, wounds, clock.tick);
    }
}

pub fn near_death_tick(
    clock: Res<CombatClock>,
    mut revived: EventWriter<PlayerRevived>,
    mut terminated: EventWriter<PlayerTerminated>,
    mut lifecycle_q: Query<NearDeathQueryItem<'_>>,
    positions: Query<&Position>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for (entity, mut lifecycle, wounds, stamina, combat_state) in &mut lifecycle_q {
        if lifecycle
            .weakened_until_tick
            .is_some_and(|until_tick| clock.tick >= until_tick)
        {
            lifecycle.weakened_until_tick = None;
        }

        if lifecycle.state != LifecycleState::NearDeath {
            continue;
        }

        let stabilized = wounds.as_ref().is_some_and(|wounds| {
            wounds.health_current > wounds.health_max.max(1.0) * NEAR_DEATH_HEALTH_FRACTION
        });
        if stabilized {
            lifecycle.near_death_deadline_tick = None;
            lifecycle.state = LifecycleState::Alive;
            continue;
        }

        let Some(deadline_tick) = lifecycle.near_death_deadline_tick else {
            continue;
        };
        if clock.tick < deadline_tick {
            continue;
        }

        if lifecycle.fortune_remaining > 0 {
            lifecycle.fortune_remaining = lifecycle.fortune_remaining.saturating_sub(1);
            lifecycle.revive(clock.tick);

            if let Some(mut wounds) = wounds {
                let recovered = (wounds.health_max * REVIVE_HEALTH_FRACTION).max(1.0);
                wounds.health_current = wounds.health_current.max(recovered);
            }
            if let Some(mut stamina) = stamina {
                stamina.current = stamina.current.max(stamina.max * EXHAUSTED_EXIT_FRACTION);
                if matches!(
                    stamina.state,
                    StaminaState::Combat | StaminaState::Exhausted
                ) {
                    stamina.state = StaminaState::Idle;
                }
            }
            if let Some(mut combat_state) = combat_state {
                combat_state.incoming_window = None;
                combat_state.refresh_combat_window(clock.tick);
            }

            revived.send(PlayerRevived { entity });
            continue;
        }

        lifecycle.terminate(clock.tick);
        terminated.send(PlayerTerminated { entity });

        // plan-particle-system-v1 §4.4：终结时发 `death_soul_dissipate` 魂散。
        if let Ok(pos) = positions.get(entity) {
            let p = pos.get();
            vfx_events.send(VfxEventRequest::new(
                p,
                VfxEventPayloadV1::SpawnParticle {
                    event_id: "bong:death_soul_dissipate".to_string(),
                    origin: [p.x, p.y, p.z],
                    direction: None,
                    color: Some("#CFEFFF".to_string()),
                    strength: Some(0.9),
                    count: Some(20),
                    duration_ticks: Some(40),
                },
            ));
        }
    }
}

fn enter_near_death(
    lifecycle: &mut Lifecycle,
    mut wounds: Option<valence::prelude::Mut<'_, Wounds>>,
    now_tick: u64,
) {
    if lifecycle.state == LifecycleState::Terminated {
        return;
    }

    lifecycle.enter_near_death(now_tick);
    if let Some(wounds) = wounds.as_mut() {
        let floor = wounds.health_max.max(1.0) * NEAR_DEATH_HEALTH_FRACTION;
        wounds.health_current = wounds.health_current.min(floor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::combat::components::{
        BodyPart, DefenseWindow, Wound, WoundKind, IN_COMBAT_WINDOW_TICKS,
        JIEMAI_DEFENSE_WINDOW_MS, REVIVE_WEAKENED_TICKS,
    };
    use crate::combat::events::DefenseIntent;
    use crate::cultivation::death_hooks::CultivationDeathCause;
    use crate::cultivation::life_record::LifeRecord;
    use valence::prelude::{App, Events, IntoSystemConfigs, Update};

    fn spawn_actor(
        app: &mut App,
        wounds: Wounds,
        stamina: Stamina,
        lifecycle: Lifecycle,
    ) -> Entity {
        app.world_mut()
            .spawn((
                wounds,
                stamina,
                CombatState::default(),
                LifeRecord::default(),
                lifecycle,
            ))
            .id()
    }

    #[test]
    fn wound_bleed_tick_emits_single_death_event_on_alive_to_dead_transition() {
        let mut app = App::new();
        app.insert_resource(CombatClock {
            tick: BLEED_TICK_INTERVAL_TICKS,
        });
        app.add_event::<DeathEvent>();
        app.add_systems(Update, wound_bleed_tick);

        let entity = spawn_actor(
            &mut app,
            Wounds {
                health_current: 2.0,
                health_max: 30.0,
                entries: vec![Wound {
                    location: BodyPart::Chest,
                    kind: WoundKind::Cut,
                    severity: 0.3,
                    bleeding_per_sec: 3.0,
                    created_at_tick: 0,
                    inflicted_by: None,
                }],
            },
            Stamina::default(),
            Lifecycle::default(),
        );

        app.update();
        app.world_mut().resource_mut::<CombatClock>().tick += BLEED_TICK_INTERVAL_TICKS;
        app.update();

        let wounds = app.world().entity(entity).get::<Wounds>().unwrap();
        let death_events = app.world().resource::<Events<DeathEvent>>();
        assert_eq!(wounds.health_current, 0.0);
        assert_eq!(death_events.len(), 1);
    }

    #[test]
    fn stamina_tick_recovers_exhausted_back_to_idle_after_threshold() {
        let mut app = App::new();
        app.insert_resource(CombatClock {
            tick: STAMINA_TICK_INTERVAL_TICKS,
        });
        app.add_systems(Update, stamina_tick);

        let entity = spawn_actor(
            &mut app,
            Wounds::default(),
            Stamina {
                current: 30.0,
                max: 100.0,
                recover_per_sec: 5.0,
                last_drain_tick: None,
                state: StaminaState::Exhausted,
            },
            Lifecycle::default(),
        );

        app.update();

        let stamina = app.world().entity(entity).get::<Stamina>().unwrap();
        assert!(stamina.current > 30.0);
        assert_eq!(stamina.state, StaminaState::Idle);
    }

    #[test]
    fn sync_combat_state_marks_both_sides_and_charges_attacker_stamina() {
        let mut app = App::new();
        app.add_event::<CombatEvent>();
        app.add_systems(Update, sync_combat_state_from_events);

        let attacker = spawn_actor(
            &mut app,
            Wounds::default(),
            Stamina::default(),
            Lifecycle::default(),
        );
        let target = spawn_actor(
            &mut app,
            Wounds::default(),
            Stamina::default(),
            Lifecycle::default(),
        );

        app.world_mut().send_event(CombatEvent {
            attacker,
            target,
            resolved_at_tick: 15,
            body_part: BodyPart::Chest,
            wound_kind: WoundKind::Blunt,
            damage: 3.0,
            contam_delta: 0.75,
            description: "hit".to_string(),
        });
        app.update();

        let attacker_ref = app.world().entity(attacker);
        let target_ref = app.world().entity(target);
        let attacker_state = attacker_ref.get::<CombatState>().unwrap();
        let target_state = target_ref.get::<CombatState>().unwrap();
        let attacker_stamina = attacker_ref.get::<Stamina>().unwrap();
        let target_stamina = target_ref.get::<Stamina>().unwrap();

        assert_eq!(attacker_state.last_attack_at_tick, Some(15));
        assert_eq!(
            attacker_state.in_combat_until_tick,
            Some(15 + IN_COMBAT_WINDOW_TICKS)
        );
        assert_eq!(
            target_state.in_combat_until_tick,
            Some(15 + IN_COMBAT_WINDOW_TICKS)
        );
        assert!(attacker_stamina.current <= 97.0);
        assert!(attacker_stamina.current >= 94.0);
        assert_eq!(attacker_stamina.state, StaminaState::Combat);
        assert_eq!(target_stamina.state, StaminaState::Combat);
    }

    #[test]
    fn combat_state_tick_clears_expired_windows_and_combat_stamina_state() {
        let mut app = App::new();
        app.insert_resource(CombatClock {
            tick: COMBAT_STATE_TICK_INTERVAL_TICKS,
        });
        app.add_systems(Update, combat_state_tick);

        let entity = app
            .world_mut()
            .spawn((
                Wounds::default(),
                Stamina {
                    current: 40.0,
                    max: 100.0,
                    recover_per_sec: 5.0,
                    last_drain_tick: None,
                    state: StaminaState::Combat,
                },
                CombatState {
                    in_combat_until_tick: Some(10),
                    last_attack_at_tick: Some(1),
                    incoming_window: Some(DefenseWindow {
                        opened_at_tick: 0,
                        duration_ms: 100,
                    }),
                },
                Lifecycle::default(),
            ))
            .id();

        app.update();

        let state = app.world().entity(entity).get::<CombatState>().unwrap();
        let stamina = app.world().entity(entity).get::<Stamina>().unwrap();
        assert!(state.in_combat_until_tick.is_none());
        assert!(state.incoming_window.is_none());
        assert_eq!(stamina.state, StaminaState::Idle);
    }

    #[test]
    fn defense_intent_opens_incoming_window() {
        let mut app = App::new();
        app.add_event::<DefenseIntent>();
        app.add_systems(Update, crate::combat::resolve::apply_defense_intents);

        let entity = spawn_actor(
            &mut app,
            Wounds::default(),
            Stamina::default(),
            Lifecycle::default(),
        );

        app.world_mut().send_event(DefenseIntent {
            defender: entity,
            issued_at_tick: 42,
        });
        app.update();

        let state = app.world().entity(entity).get::<CombatState>().unwrap();
        let window = state.incoming_window.as_ref().expect("window should open");
        assert_eq!(window.opened_at_tick, 42);
        assert_eq!(window.duration_ms, JIEMAI_DEFENSE_WINDOW_MS);
    }

    #[test]
    fn death_arbiter_timeout_revives_when_fortune_remains() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 100 });
        app.add_event::<DeathEvent>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<PlayerRevived>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(
            Update,
            (
                death_arbiter_tick,
                near_death_tick.after(death_arbiter_tick),
            ),
        );

        let entity = spawn_actor(
            &mut app,
            Wounds {
                health_current: 0.0,
                health_max: 30.0,
                entries: Vec::new(),
            },
            Stamina::default(),
            Lifecycle {
                fortune_remaining: 1,
                ..Default::default()
            },
        );

        app.world_mut().send_event(DeathEvent {
            target: entity,
            cause: "test".to_string(),
            at_tick: 100,
        });
        app.update();

        {
            let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
            assert_eq!(lifecycle.state, LifecycleState::NearDeath);
            assert_eq!(lifecycle.death_count, 1);
        }

        app.world_mut().resource_mut::<CombatClock>().tick = 701;
        app.update();

        let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
        let wounds = app.world().entity(entity).get::<Wounds>().unwrap();
        let revived_events = app.world().resource::<Events<PlayerRevived>>();
        assert_eq!(lifecycle.state, LifecycleState::Alive);
        assert_eq!(lifecycle.fortune_remaining, 0);
        assert_eq!(lifecycle.last_revive_tick, Some(701));
        assert_eq!(
            lifecycle.weakened_until_tick,
            Some(701 + REVIVE_WEAKENED_TICKS)
        );
        assert!(wounds.health_current >= 6.0);
        assert_eq!(revived_events.len(), 1);
    }

    #[test]
    fn cultivation_death_without_fortune_terminates_after_deadline() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 40 });
        app.add_event::<DeathEvent>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<PlayerRevived>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(
            Update,
            (
                death_arbiter_tick,
                near_death_tick.after(death_arbiter_tick),
            ),
        );

        let entity = spawn_actor(
            &mut app,
            Wounds::default(),
            Stamina::default(),
            Lifecycle {
                fortune_remaining: 0,
                ..Default::default()
            },
        );

        app.world_mut().send_event(CultivationDeathTrigger {
            entity,
            cause: CultivationDeathCause::NegativeZoneDrain,
            context: serde_json::json!({"zone": "rift_valley"}),
        });
        app.update();

        app.world_mut().resource_mut::<CombatClock>().tick = 641;
        app.update();

        let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
        let terminated_events = app.world().resource::<Events<PlayerTerminated>>();
        assert_eq!(lifecycle.state, LifecycleState::Terminated);
        assert_eq!(terminated_events.len(), 1);
    }

    #[test]
    fn repeated_death_events_do_not_extend_near_death_deadline() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 10 });
        app.add_event::<DeathEvent>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_systems(Update, death_arbiter_tick);

        let entity = spawn_actor(
            &mut app,
            Wounds::default(),
            Stamina::default(),
            Lifecycle::default(),
        );

        app.world_mut().send_event(DeathEvent {
            target: entity,
            cause: "first".to_string(),
            at_tick: 10,
        });
        app.update();

        let first_deadline = app
            .world()
            .entity(entity)
            .get::<Lifecycle>()
            .unwrap()
            .near_death_deadline_tick;

        app.world_mut().resource_mut::<CombatClock>().tick = 200;
        app.world_mut().send_event(DeathEvent {
            target: entity,
            cause: "second".to_string(),
            at_tick: 200,
        });
        app.update();

        let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
        assert_eq!(lifecycle.state, LifecycleState::NearDeath);
        assert_eq!(lifecycle.near_death_deadline_tick, first_deadline);
        assert_eq!(lifecycle.death_count, 1);
    }
}
