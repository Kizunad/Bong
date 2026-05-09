use valence::prelude::{
    bevy_ecs, Component, EntityInteraction, EventReader, EventWriter, InteractEntityEvent,
    Position, Query, Res, With,
};

use crate::combat::components::{
    Lifecycle, LifecycleState, Stamina, WoundKind, ATTACK_STAMINA_COST,
};
use crate::combat::events::{AttackIntent, AttackSource, FIST_REACH, SWORD_REACH};
use crate::combat::weapon::Weapon;
use crate::combat::CombatClock;
use crate::npc::spawn::NpcMarker;

const ATTACK_COOLDOWN_TICKS: u64 = 10;
const REACH_TOLERANCE: f64 = 0.5;

#[derive(Debug, Clone, Component, Default)]
pub struct PlayerAttackCooldown {
    pub last_attack_tick: u64,
}

pub fn handle_player_attack(
    mut interactions: EventReader<InteractEntityEvent>,
    mut intents: EventWriter<AttackIntent>,
    clock: Res<CombatClock>,
    mut clients: Query<
        (
            &Position,
            &Stamina,
            Option<&Weapon>,
            &mut PlayerAttackCooldown,
        ),
        With<valence::prelude::Client>,
    >,
    targets: Query<(&Position, Option<&Lifecycle>), With<NpcMarker>>,
) {
    for ev in interactions.read() {
        if ev.interact != EntityInteraction::Attack {
            continue;
        }

        if ev.client == ev.entity {
            continue;
        }

        let Ok((attacker_pos, stamina, weapon, mut cooldown)) = clients.get_mut(ev.client) else {
            continue;
        };

        let Ok((target_pos, target_lifecycle)) = targets.get(ev.entity) else {
            continue;
        };

        if let Some(lc) = target_lifecycle {
            if matches!(
                lc.state,
                LifecycleState::NearDeath | LifecycleState::AwaitingRevival
            ) {
                continue;
            }
        }

        if clock.tick.saturating_sub(cooldown.last_attack_tick) < ATTACK_COOLDOWN_TICKS {
            continue;
        }

        if stamina.current < ATTACK_STAMINA_COST {
            continue;
        }

        let reach = weapon.map(weapon_reach).unwrap_or(FIST_REACH);
        let dist = attacker_pos.0.distance(target_pos.0);
        if dist > reach.max as f64 + REACH_TOLERANCE {
            tracing::warn!(
                "[bong][combat] player attack rejected: distance {dist:.1} > reach {} + {REACH_TOLERANCE}",
                reach.max
            );
            continue;
        }

        let wound_kind = weapon.map(|_| WoundKind::Cut).unwrap_or(WoundKind::Blunt);

        cooldown.last_attack_tick = clock.tick;

        intents.send(AttackIntent {
            attacker: ev.client,
            target: Some(ev.entity),
            issued_at_tick: clock.tick,
            reach,
            qi_invest: 0.0,
            wound_kind,
            source: AttackSource::Melee,
            debug_command: None,
        });
    }
}

fn weapon_reach(w: &Weapon) -> crate::combat::events::AttackReach {
    use crate::combat::weapon::WeaponKind;
    match w.weapon_kind {
        WeaponKind::Sword | WeaponKind::Saber => SWORD_REACH,
        WeaponKind::Spear => crate::combat::events::SPEAR_REACH,
        WeaponKind::Staff => crate::combat::events::STAFF_REACH,
        WeaponKind::Dagger => crate::combat::events::DAGGER_REACH,
        WeaponKind::Fist => FIST_REACH,
        WeaponKind::Bow => SWORD_REACH,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{App, Entity, Events, Update};
    use valence::testing::create_mock_client;

    fn stamina_full() -> Stamina {
        Stamina {
            current: 100.0,
            max: 100.0,
            recover_per_sec: 1.0,
            last_drain_tick: None,
            state: crate::combat::components::StaminaState::Idle,
        }
    }

    fn stamina_empty() -> Stamina {
        Stamina {
            current: 0.0,
            max: 100.0,
            recover_per_sec: 1.0,
            last_drain_tick: None,
            state: crate::combat::components::StaminaState::Idle,
        }
    }

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_event::<InteractEntityEvent>();
        app.add_event::<AttackIntent>();
        app.insert_resource(CombatClock { tick: 100 });
        app.add_systems(Update, handle_player_attack);
        app
    }

    fn spawn_attacker(app: &mut App, stamina: Stamina, cooldown: PlayerAttackCooldown) -> Entity {
        let (client_bundle, _helper) = create_mock_client("TestPlayer");
        let entity = app.world_mut().spawn((client_bundle, stamina, cooldown)).id();
        *app.world_mut().get_mut::<Position>(entity).unwrap() = Position::new([0.0, 0.0, 0.0]);
        entity
    }

    #[test]
    fn attack_generates_intent() {
        let mut app = setup_app();
        let attacker = spawn_attacker(&mut app, stamina_full(), PlayerAttackCooldown::default());
        let target = app
            .world_mut()
            .spawn((NpcMarker, Position::new([1.0, 0.0, 0.0])))
            .id();

        app.world_mut().send_event(InteractEntityEvent {
            client: attacker,
            entity: target,
            sneaking: false,
            interact: EntityInteraction::Attack,
        });
        app.update();

        let events = app.world().resource::<Events<AttackIntent>>();
        let intent = events
            .iter_current_update_events()
            .next()
            .expect("attack should generate AttackIntent");
        assert_eq!(intent.attacker, attacker);
        assert_eq!(intent.target, Some(target));
        assert_eq!(intent.wound_kind, WoundKind::Blunt);
    }

    #[test]
    fn non_attack_interaction_ignored() {
        let mut app = setup_app();
        let attacker = spawn_attacker(&mut app, stamina_full(), PlayerAttackCooldown::default());
        let target = app
            .world_mut()
            .spawn((NpcMarker, Position::new([1.0, 0.0, 0.0])))
            .id();

        app.world_mut().send_event(InteractEntityEvent {
            client: attacker,
            entity: target,
            sneaking: false,
            interact: EntityInteraction::Interact(valence::prelude::Hand::Main),
        });
        app.update();

        let events = app.world().resource::<Events<AttackIntent>>();
        assert!(
            events.iter_current_update_events().next().is_none(),
            "Interact(Main) must not generate AttackIntent"
        );
    }

    #[test]
    fn stamina_insufficient_ignored() {
        let mut app = setup_app();
        let attacker = spawn_attacker(&mut app, stamina_empty(), PlayerAttackCooldown::default());
        let target = app
            .world_mut()
            .spawn((NpcMarker, Position::new([1.0, 0.0, 0.0])))
            .id();

        app.world_mut().send_event(InteractEntityEvent {
            client: attacker,
            entity: target,
            sneaking: false,
            interact: EntityInteraction::Attack,
        });
        app.update();

        let events = app.world().resource::<Events<AttackIntent>>();
        assert!(
            events.iter_current_update_events().next().is_none(),
            "no stamina should block attack"
        );
    }

    #[test]
    fn cooldown_prevents_spam() {
        let mut app = setup_app();
        let attacker = spawn_attacker(
            &mut app,
            stamina_full(),
            PlayerAttackCooldown {
                last_attack_tick: 95,
            },
        );
        let target = app
            .world_mut()
            .spawn((NpcMarker, Position::new([1.0, 0.0, 0.0])))
            .id();

        app.world_mut().send_event(InteractEntityEvent {
            client: attacker,
            entity: target,
            sneaking: false,
            interact: EntityInteraction::Attack,
        });
        app.update();

        let events = app.world().resource::<Events<AttackIntent>>();
        assert!(
            events.iter_current_update_events().next().is_none(),
            "cooldown (5 ticks < 10) should block attack"
        );
    }

    #[test]
    fn out_of_range_ignored() {
        let mut app = setup_app();
        let attacker = spawn_attacker(&mut app, stamina_full(), PlayerAttackCooldown::default());
        let target = app
            .world_mut()
            .spawn((NpcMarker, Position::new([10.0, 0.0, 0.0])))
            .id();

        app.world_mut().send_event(InteractEntityEvent {
            client: attacker,
            entity: target,
            sneaking: false,
            interact: EntityInteraction::Attack,
        });
        app.update();

        let events = app.world().resource::<Events<AttackIntent>>();
        assert!(
            events.iter_current_update_events().next().is_none(),
            "target at distance 10 should be out of fist reach"
        );
    }
}
