use valence::prelude::{EventReader, EventWriter, Query, Res, ResMut};

use crate::combat::components::{Lifecycle, Stamina, Wound, Wounds};
use crate::combat::events::{
    AttackIntent, CombatEvent, DebugCombatCommand, DebugCombatCommandKind,
};
use crate::combat::CombatClock;

/// 调试命令默认的 bleeding 系数（per-second）— 约为 severity × BLEEDING_COEF。
const DEBUG_WOUND_BLEEDING_COEF: f32 = 0.5;

pub fn tick_combat_clock(mut clock: ResMut<CombatClock>) {
    clock.tick = clock.tick.saturating_add(1);
}

pub fn enqueue_debug_attack_intent(intents: &mut EventWriter<AttackIntent>, intent: AttackIntent) {
    intents.send(intent);
}

pub fn drain_combat_events_for_debug(mut events: EventReader<CombatEvent>) {
    for event in events.read() {
        tracing::debug!(
            "[bong][combat][debug] event attacker={:?} target={:?} tick={} desc={}",
            event.attacker,
            event.target,
            event.resolved_at_tick,
            event.description
        );
    }
}

/// plan §13 C1 — 消费 `!wound add` / `!health set` / `!stamina set` 调试命令，直接改写组件。
pub fn apply_debug_combat_commands(
    mut events: EventReader<DebugCombatCommand>,
    clock: Res<CombatClock>,
    mut wounds_q: Query<&mut Wounds>,
    mut stamina_q: Query<&mut Stamina>,
    mut lifecycle_q: Query<&mut Lifecycle>,
) {
    for cmd in events.read() {
        match cmd.kind {
            DebugCombatCommandKind::AddWound {
                location,
                kind,
                severity,
            } => {
                let Ok(mut wounds) = wounds_q.get_mut(cmd.target) else {
                    tracing::warn!(
                        "[bong][combat][debug] AddWound target {:?} has no Wounds component",
                        cmd.target
                    );
                    continue;
                };
                let sev = severity.clamp(0.0, 1.0);
                wounds.entries.push(Wound {
                    location,
                    kind,
                    severity: sev,
                    bleeding_per_sec: sev * DEBUG_WOUND_BLEEDING_COEF,
                    created_at_tick: clock.tick,
                    inflicted_by: None,
                });
                tracing::info!(
                    "[bong][combat][debug] added {:?} wound at {:?} severity={:.2} on entity={:?}",
                    kind,
                    location,
                    sev,
                    cmd.target
                );
            }
            DebugCombatCommandKind::SetHealth(n) => {
                let Ok(mut wounds) = wounds_q.get_mut(cmd.target) else {
                    tracing::warn!(
                        "[bong][combat][debug] SetHealth target {:?} has no Wounds component",
                        cmd.target
                    );
                    continue;
                };
                wounds.health_current = n.clamp(0.0, wounds.health_max);
                tracing::info!(
                    "[bong][combat][debug] health={:.1}/{:.1} on entity={:?}",
                    wounds.health_current,
                    wounds.health_max,
                    cmd.target
                );
            }
            DebugCombatCommandKind::SetStamina(n) => {
                let Ok(mut stamina) = stamina_q.get_mut(cmd.target) else {
                    tracing::warn!(
                        "[bong][combat][debug] SetStamina target {:?} has no Stamina component",
                        cmd.target
                    );
                    continue;
                };
                stamina.current = n.clamp(0.0, stamina.max);
                tracing::info!(
                    "[bong][combat][debug] stamina={:.1}/{:.1} on entity={:?}",
                    stamina.current,
                    stamina.max,
                    cmd.target
                );
            }
            DebugCombatCommandKind::SetSpawnAnchor(anchor) => {
                let Ok(mut lifecycle) = lifecycle_q.get_mut(cmd.target) else {
                    tracing::warn!(
                        "[bong][combat][debug] SetSpawnAnchor target {:?} has no Lifecycle component",
                        cmd.target
                    );
                    continue;
                };
                lifecycle.spawn_anchor = anchor;
                tracing::info!(
                    "[bong][combat][debug] spawn_anchor={:?} on entity={:?}",
                    lifecycle.spawn_anchor,
                    cmd.target
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::components::{BodyPart, WoundKind};
    use valence::prelude::{App, Update};

    fn setup_app() -> App {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 42 });
        app.add_event::<DebugCombatCommand>();
        app.add_systems(Update, apply_debug_combat_commands);
        app
    }

    #[test]
    fn add_wound_appends_entry_with_clock_tick() {
        let mut app = setup_app();
        let target = app.world_mut().spawn(Wounds::default()).id();
        app.world_mut().send_event(DebugCombatCommand {
            target,
            kind: DebugCombatCommandKind::AddWound {
                location: BodyPart::Head,
                kind: WoundKind::Cut,
                severity: 0.5,
            },
        });

        app.update();

        let wounds = app.world().entity(target).get::<Wounds>().unwrap();
        assert_eq!(wounds.entries.len(), 1);
        let w = &wounds.entries[0];
        assert_eq!(w.location, BodyPart::Head);
        assert_eq!(w.kind, WoundKind::Cut);
        assert!((w.severity - 0.5).abs() < 1e-6);
        assert!((w.bleeding_per_sec - 0.25).abs() < 1e-6);
        assert_eq!(w.created_at_tick, 42);
        assert!(w.inflicted_by.is_none());
    }

    #[test]
    fn add_wound_clamps_severity() {
        let mut app = setup_app();
        let target = app.world_mut().spawn(Wounds::default()).id();
        app.world_mut().send_event(DebugCombatCommand {
            target,
            kind: DebugCombatCommandKind::AddWound {
                location: BodyPart::Chest,
                kind: WoundKind::Blunt,
                severity: 2.5,
            },
        });

        app.update();

        let wounds = app.world().entity(target).get::<Wounds>().unwrap();
        assert!((wounds.entries[0].severity - 1.0).abs() < 1e-6);
    }

    #[test]
    fn set_health_clamps_to_health_max() {
        let mut app = setup_app();
        let target = app.world_mut().spawn(Wounds::default()).id();
        let health_max = app
            .world()
            .entity(target)
            .get::<Wounds>()
            .unwrap()
            .health_max;

        app.world_mut().send_event(DebugCombatCommand {
            target,
            kind: DebugCombatCommandKind::SetHealth(health_max * 10.0),
        });
        app.update();

        let wounds = app.world().entity(target).get::<Wounds>().unwrap();
        assert!((wounds.health_current - health_max).abs() < 1e-6);

        app.world_mut().send_event(DebugCombatCommand {
            target,
            kind: DebugCombatCommandKind::SetHealth(-10.0),
        });
        app.update();

        let wounds = app.world().entity(target).get::<Wounds>().unwrap();
        assert!((wounds.health_current - 0.0).abs() < 1e-6);
    }

    #[test]
    fn set_stamina_clamps_to_max() {
        let mut app = setup_app();
        let target = app.world_mut().spawn(Stamina::default()).id();
        let stamina_max = app.world().entity(target).get::<Stamina>().unwrap().max;

        app.world_mut().send_event(DebugCombatCommand {
            target,
            kind: DebugCombatCommandKind::SetStamina(stamina_max * 2.0),
        });
        app.update();

        let stamina = app.world().entity(target).get::<Stamina>().unwrap();
        assert!((stamina.current - stamina_max).abs() < 1e-6);

        app.world_mut().send_event(DebugCombatCommand {
            target,
            kind: DebugCombatCommandKind::SetStamina(-5.0),
        });
        app.update();

        let stamina = app.world().entity(target).get::<Stamina>().unwrap();
        assert!((stamina.current - 0.0).abs() < 1e-6);
    }

    #[test]
    fn add_wound_warns_when_component_missing() {
        let mut app = setup_app();
        let target = app.world_mut().spawn_empty().id();

        app.world_mut().send_event(DebugCombatCommand {
            target,
            kind: DebugCombatCommandKind::AddWound {
                location: BodyPart::Head,
                kind: WoundKind::Blunt,
                severity: 0.2,
            },
        });

        app.update();
        assert!(app.world().entity(target).get::<Wounds>().is_none());
    }
}
