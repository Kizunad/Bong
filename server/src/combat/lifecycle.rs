use std::time::{SystemTime, UNIX_EPOCH};

use valence::prelude::{
    Entity, EventReader, EventWriter, Events, Position, Query, Res, ResMut, Username,
};

use crate::alchemy::LearnedRecipes;
use crate::combat::CombatClock;
use crate::cultivation::components::{Contamination, Cultivation, MeridianSystem, Realm};
use crate::cultivation::death_hooks::{
    apply_revive_penalty, CultivationDeathCause, CultivationDeathTrigger, PlayerRevived,
    PlayerTerminated,
};
use crate::cultivation::known_techniques::KnownTechniques;
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::cultivation::lifespan::{
    calculate_rebirth_chance, lifespan_tick_rate_multiplier, tribulation_rebirth_chance,
    DeathRegistry, LifespanCapTable, LifespanComponent, LifespanEventEmitted, RebirthChanceInput,
    ZoneDeathKind,
};
use crate::cultivation::{
    color::PracticeLog,
    components::{Karma, QiColor},
};
use crate::inventory::{
    instantiate_inventory_from_loadout, DeathDropAnchor, DefaultLoadout,
    InventoryInstanceIdAllocator, PlayerInventory,
};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::send_server_data_payload;
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::npc::spawn::NpcMarker;
use crate::persistence::{
    persist_near_death_transition, persist_revival_transition, persist_termination_transition,
    persist_termination_transition_with_death_context, LifespanEventRecord, PersistenceSettings,
};
use crate::player::state::{
    player_character_id, rotate_current_character_id, save_player_shrine_anchor_slice,
    save_player_slices, PlayerState, PlayerStatePersistence,
};
use crate::schema::cultivation::realm_to_string;
use crate::schema::death_insight::{
    DeathInsightCategoryV1, DeathInsightPositionV1, DeathInsightRequestV1, DeathInsightZoneKindV1,
};
use crate::schema::server_data::{DeathScreenStageV1, DeathScreenZoneKindV1, LifespanPreviewV1};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::skill::components::SkillSet;
use crate::world::dimension::DimensionKind;
use crate::world::zone::ZoneRegistry;

use super::components::{
    CombatState, DerivedAttrs, Lifecycle, LifecycleState, QuickSlotBindings, RevivalDecision,
    SkillBarBindings, Stamina, StaminaState, StatusEffects, UnlockedStyles, Wounds,
    ATTACK_STAMINA_COST, BLEED_TICK_INTERVAL_TICKS, COMBAT_STATE_TICK_INTERVAL_TICKS,
    NEAR_DEATH_HEALTH_FRACTION, REVIVAL_CONFIRM_WINDOW_TICKS, REVIVE_HEALTH_FRACTION,
    STAMINA_TICK_INTERVAL_TICKS, TICKS_PER_SECOND,
};
use super::events::{
    CombatEvent, DeathEvent, DeathInsightRequested, RevivalActionIntent, RevivalActionKind,
};

const COMBAT_DRAIN_PER_SEC: f32 = 5.0;
const JOG_DRAIN_PER_SEC: f32 = 2.0;
const SPRINT_DRAIN_PER_SEC: f32 = 10.0;
const EXHAUSTED_RECOVER_RATIO: f32 = 0.5;
const EXHAUSTED_EXIT_FRACTION: f32 = 0.3;
const DEATH_INSIGHT_RECENT_BIO_N: usize = 16;

type NearDeathQueryItem<'a> = (
    Entity,
    &'a mut Lifecycle,
    Option<&'a mut Wounds>,
    Option<&'a mut Stamina>,
    Option<&'a mut CombatState>,
);

type DeathArbiterQueryItem<'a> = (
    &'a mut Lifecycle,
    Option<&'a mut Wounds>,
    Option<&'a mut LifeRecord>,
    Option<&'a Cultivation>,
    Option<&'a PlayerState>,
    Option<&'a mut DeathRegistry>,
    Option<&'a mut LifespanComponent>,
    Option<&'a Position>,
);

type NearDeathPersistenceQueryItem<'a> = (
    NearDeathQueryItem<'a>,
    Option<&'a mut Cultivation>,
    Option<&'a mut MeridianSystem>,
    Option<&'a mut Contamination>,
    Option<&'a mut LifeRecord>,
    Option<&'a mut DeathRegistry>,
    Option<&'a mut LifespanComponent>,
    Option<&'a mut PlayerState>,
    Option<&'a mut Position>,
    Option<&'a Username>,
    Option<&'a NpcMarker>,
    Option<&'a mut PlayerInventory>,
    Option<&'a mut SkillSet>,
);

struct DeathScreenContext<'a> {
    lifecycle: &'a Lifecycle,
    death_registry: Option<&'a DeathRegistry>,
    lifespan: Option<&'a LifespanComponent>,
    position: Option<&'a Position>,
    zones: Option<&'a ZoneRegistry>,
}

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
                attacker: None,
                attacker_player_id: None,
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

#[allow(clippy::too_many_arguments)]
pub fn death_arbiter_tick(
    clock: Res<CombatClock>,
    persistence: Res<PersistenceSettings>,
    zones: Option<Res<ZoneRegistry>>,
    mut commands: valence::prelude::Commands,
    mut death_events: EventReader<DeathEvent>,
    mut cultivation_deaths: EventReader<CultivationDeathTrigger>,
    mut death_insights: Option<ResMut<Events<DeathInsightRequested>>>,
    mut terminated: EventWriter<PlayerTerminated>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    mut lifespan_events: Option<ResMut<Events<LifespanEventEmitted>>>,
    mut lifecycle_q: Query<DeathArbiterQueryItem<'_>>,
) {
    for event in death_events.read() {
        let Ok((
            mut lifecycle,
            wounds,
            life_record,
            cultivation,
            player_state,
            mut death_registry,
            mut lifespan,
            position,
        )) = lifecycle_q.get_mut(event.target)
        else {
            continue;
        };

        // Worldview §十二：死亡掉落应落在死亡点。
        if let Some(position) = position {
            let p = position.get();
            commands.entity(event.target).insert(DeathDropAnchor {
                pos: [p.x, p.y, p.z],
            });
        }
        if matches!(
            lifecycle.state,
            LifecycleState::NearDeath | LifecycleState::Terminated
        ) {
            continue;
        }
        let now_tick = event.at_tick.max(clock.tick);
        let death_zone = death_zone_from_context(event.cause.as_str(), position, zones.as_deref());
        if let Some(registry) = death_registry.as_deref_mut() {
            registry.record_death(now_tick, death_zone);
        }
        let lifespan_exhausted =
            apply_death_lifespan_penalty(cultivation, lifespan.as_deref_mut(), player_state);
        let revival_decision = if lifespan_exhausted {
            None
        } else {
            determine_revival_decision(
                &lifecycle,
                death_registry.as_deref(),
                event.cause.as_str(),
                lifespan.as_deref(),
                player_state,
                position,
                zones.as_deref(),
                now_tick,
            )
        };
        let rebirth_chance = revival_decision.map(|decision| decision.chance_shown());
        let category = death_insight_category_from_revival_decision(
            DeathInsightCategoryV1::Combat,
            revival_decision,
        );
        let insight_payload = build_death_insight_request(DeathInsightBuildInput {
            lifecycle: &lifecycle,
            life_record: life_record.as_deref(),
            cultivation,
            death_registry: death_registry.as_deref(),
            lifespan: lifespan.as_deref(),
            position,
            at_tick: now_tick,
            cause: event.cause.as_str(),
            category,
            zone_kind: death_zone,
            rebirth_chance,
            will_terminate: lifespan_exhausted,
        });

        if lifespan_exhausted {
            let lifespan_event =
                death_penalty_lifespan_event(cultivation, now_tick, event.cause.as_str());
            let lifespan_event_char_id = lifespan_event
                .as_ref()
                .map(|_| lifespan_event_character_id(life_record.as_deref(), &lifecycle));
            let terminated_now = terminate_lifecycle_with_death_context(
                event.target,
                &mut lifecycle,
                life_record,
                &persistence,
                now_tick,
                &mut terminated,
                position,
                &mut vfx_events,
                "natural_end",
                Some(event.cause.as_str()),
                lifespan_event.clone(),
            );
            if terminated_now {
                emit_death_lifespan_event(
                    lifespan_events.as_deref_mut(),
                    lifespan_event_char_id,
                    lifespan_event.as_ref(),
                );
                if let Some(death_insights) = death_insights.as_deref_mut() {
                    death_insights.send(DeathInsightRequested {
                        payload: insight_payload,
                    });
                }
            }
            continue;
        }

        let lifespan_event =
            death_penalty_lifespan_event(cultivation, now_tick, event.cause.as_str());
        let lifespan_event_char_id = lifespan_event
            .as_ref()
            .map(|_| lifespan_event_character_id(life_record.as_deref(), &lifecycle));
        if let Some(mut life_record) = life_record {
            life_record.push(BiographyEntry::NearDeath {
                cause: event.cause.clone(),
                tick: now_tick,
            });
            let mut staged_lifecycle = lifecycle.clone();
            staged_lifecycle.enter_near_death(now_tick);
            if let Err(error) = persist_near_death_transition(
                &persistence,
                &staged_lifecycle,
                &life_record,
                event.cause.as_str(),
                lifespan_event.as_ref(),
            ) {
                tracing::warn!(
                    "[bong][persistence] failed to persist near-death transition for {}: {error}",
                    life_record.character_id
                );
                let _ = life_record.biography.pop();
                continue;
            }
        }
        emit_death_lifespan_event(
            lifespan_events.as_deref_mut(),
            lifespan_event_char_id,
            lifespan_event.as_ref(),
        );
        enter_near_death(&mut lifecycle, wounds, now_tick);
        if let Some(death_insights) = death_insights.as_deref_mut() {
            death_insights.send(DeathInsightRequested {
                payload: insight_payload,
            });
        }
    }

    for event in cultivation_deaths.read() {
        let Ok((
            mut lifecycle,
            wounds,
            life_record,
            cultivation,
            player_state,
            mut death_registry,
            mut lifespan,
            position,
        )) = lifecycle_q.get_mut(event.entity)
        else {
            continue;
        };

        // Worldview §十二：死亡掉落应落在死亡点。
        if let Some(position) = position {
            let p = position.get();
            commands.entity(event.entity).insert(DeathDropAnchor {
                pos: [p.x, p.y, p.z],
            });
        }
        if matches!(
            lifecycle.state,
            LifecycleState::NearDeath | LifecycleState::Terminated
        ) {
            continue;
        }
        let cause = format!("cultivation:{:?}", event.cause);
        let death_zone = match event.cause {
            CultivationDeathCause::NegativeZoneDrain => ZoneDeathKind::Negative,
            _ => death_zone_from_context(cause.as_str(), position, zones.as_deref()),
        };
        if let Some(registry) = death_registry.as_deref_mut() {
            registry.record_death(clock.tick, death_zone);
        }
        let lifespan_exhausted = if event.cause == CultivationDeathCause::NaturalAging {
            apply_natural_aging_lifespan_exhaustion(
                cultivation,
                lifespan.as_deref_mut(),
                player_state,
            );
            true
        } else {
            apply_death_lifespan_penalty(cultivation, lifespan.as_deref_mut(), player_state)
        };
        let revival_decision = if lifespan_exhausted {
            None
        } else {
            determine_revival_decision(
                &lifecycle,
                death_registry.as_deref(),
                cause.as_str(),
                lifespan.as_deref(),
                player_state,
                position,
                zones.as_deref(),
                clock.tick,
            )
        };
        let rebirth_chance = revival_decision.map(|decision| decision.chance_shown());
        let category = death_insight_category_from_revival_decision(
            death_insight_category_from_cultivation_cause(event.cause),
            revival_decision,
        );
        let insight_payload = build_death_insight_request(DeathInsightBuildInput {
            lifecycle: &lifecycle,
            life_record: life_record.as_deref(),
            cultivation,
            death_registry: death_registry.as_deref(),
            lifespan: lifespan.as_deref(),
            position,
            at_tick: clock.tick,
            cause: cause.as_str(),
            category,
            zone_kind: death_zone,
            rebirth_chance,
            will_terminate: lifespan_exhausted,
        });

        if lifespan_exhausted {
            let lifespan_event = if event.cause == CultivationDeathCause::NaturalAging {
                None
            } else {
                death_penalty_lifespan_event(cultivation, clock.tick, cause.as_str())
            };
            let lifespan_event_char_id = lifespan_event
                .as_ref()
                .map(|_| lifespan_event_character_id(life_record.as_deref(), &lifecycle));
            let terminated_now = terminate_lifecycle_with_death_context(
                event.entity,
                &mut lifecycle,
                life_record,
                &persistence,
                clock.tick,
                &mut terminated,
                position,
                &mut vfx_events,
                "natural_end",
                Some(cause.as_str()),
                lifespan_event.clone(),
            );
            if terminated_now {
                emit_death_lifespan_event(
                    lifespan_events.as_deref_mut(),
                    lifespan_event_char_id,
                    lifespan_event.as_ref(),
                );
                if let Some(death_insights) = death_insights.as_deref_mut() {
                    death_insights.send(DeathInsightRequested {
                        payload: insight_payload,
                    });
                }
            }
            continue;
        }

        let lifespan_event = death_penalty_lifespan_event(cultivation, clock.tick, cause.as_str());
        let lifespan_event_char_id = lifespan_event
            .as_ref()
            .map(|_| lifespan_event_character_id(life_record.as_deref(), &lifecycle));
        if let Some(mut life_record) = life_record {
            life_record.push(BiographyEntry::NearDeath {
                cause: cause.clone(),
                tick: clock.tick,
            });
            let mut staged_lifecycle = lifecycle.clone();
            staged_lifecycle.enter_near_death(clock.tick);
            if let Err(error) = persist_near_death_transition(
                &persistence,
                &staged_lifecycle,
                &life_record,
                cause.as_str(),
                lifespan_event.as_ref(),
            ) {
                tracing::warn!(
                    "[bong][persistence] failed to persist cultivation near-death transition for {}: {error}",
                    life_record.character_id
                );
                let _ = life_record.biography.pop();
                continue;
            }
        }
        emit_death_lifespan_event(
            lifespan_events.as_deref_mut(),
            lifespan_event_char_id,
            lifespan_event.as_ref(),
        );
        enter_near_death(&mut lifecycle, wounds, clock.tick);
        if let Some(death_insights) = death_insights.as_deref_mut() {
            death_insights.send(DeathInsightRequested {
                payload: insight_payload,
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn near_death_tick(
    clock: Res<CombatClock>,
    persistence: Res<PersistenceSettings>,
    zones: Option<Res<ZoneRegistry>>,
    _revived: EventWriter<PlayerRevived>,
    mut terminated: EventWriter<PlayerTerminated>,
    mut lifecycle_q: Query<NearDeathPersistenceQueryItem<'_>>,
    mut clients: Query<&mut valence::prelude::Client>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for (
        (entity, mut lifecycle, wounds, stamina, combat_state),
        cultivation,
        meridians,
        contam,
        life_record,
        death_registry,
        lifespan,
        player_state,
        position,
        _username,
        npc_marker,
        _inventory,
        _skill_set,
    ) in &mut lifecycle_q
    {
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

        if npc_marker.is_some() {
            if terminate_lifecycle(
                entity,
                &mut lifecycle,
                life_record,
                &persistence,
                clock.tick,
                &mut terminated,
                position.as_deref(),
                &mut vfx_events,
                "npc_death",
            ) {
                hide_death_screen(&mut clients, entity);
            }
            continue;
        }

        let Some(decision) = determine_revival_decision(
            &lifecycle,
            death_registry.as_deref(),
            eventual_cause(life_record.as_deref()).as_str(),
            lifespan.as_deref(),
            player_state.as_deref(),
            position.as_deref(),
            zones.as_deref(),
            clock.tick,
        ) else {
            if terminate_lifecycle(
                entity,
                &mut lifecycle,
                life_record,
                &persistence,
                clock.tick,
                &mut terminated,
                position.as_deref(),
                &mut vfx_events,
                "natural_end",
            ) {
                hide_death_screen(&mut clients, entity);
            }
            continue;
        };

        let decision_deadline_tick = clock.tick.saturating_add(REVIVAL_CONFIRM_WINDOW_TICKS);
        lifecycle.await_revival_decision(decision, decision_deadline_tick);
        emit_death_screen(
            &mut clients,
            entity,
            &eventual_cause(life_record.as_deref()),
            decision,
            DeathScreenContext {
                lifecycle: &lifecycle,
                death_registry: death_registry.as_deref(),
                lifespan: lifespan.as_deref(),
                position: position.as_deref(),
                zones: zones.as_deref(),
            },
            clock.tick,
            decision_deadline_tick,
        );
        hide_terminate_screen(&mut clients, entity);

        let _ = (
            cultivation,
            meridians,
            contam,
            death_registry,
            stamina,
            combat_state,
            lifespan,
            wounds,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub fn handle_revival_action_intents(
    clock: Res<CombatClock>,
    persistence: Res<PersistenceSettings>,
    player_persistence: Option<Res<PlayerStatePersistence>>,
    default_loadout: Option<Res<DefaultLoadout>>,
    mut inventory_allocator: Option<ResMut<InventoryInstanceIdAllocator>>,
    mut intents: EventReader<RevivalActionIntent>,
    mut revived: EventWriter<PlayerRevived>,
    mut terminated: EventWriter<PlayerTerminated>,
    mut commands: valence::prelude::Commands,
    mut lifecycle_q: Query<NearDeathPersistenceQueryItem<'_>>,
    mut clients: Query<&mut valence::prelude::Client>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for intent in intents.read() {
        let Ok((
            (entity, mut lifecycle, wounds, stamina, combat_state),
            cultivation,
            meridians,
            contam,
            life_record,
            death_registry,
            lifespan,
            player_state,
            position,
            username,
            _npc_marker,
            inventory,
            skill_set,
        )) = lifecycle_q.get_mut(intent.entity)
        else {
            continue;
        };

        match intent.action {
            RevivalActionKind::Reincarnate => {
                if lifecycle.state != LifecycleState::AwaitingRevival {
                    continue;
                }
                let Some(decision) = lifecycle.awaiting_decision else {
                    continue;
                };

                let survived = matches!(decision, RevivalDecision::Fortune { .. })
                    || matches!(decision, RevivalDecision::Tribulation { chance } if roll_rebirth(clock.tick, entity, chance));

                if survived {
                    if revive_lifecycle(
                        entity,
                        clock.tick,
                        &persistence,
                        &mut lifecycle,
                        cultivation,
                        meridians,
                        contam,
                        life_record,
                        wounds,
                        stamina,
                        combat_state,
                        player_state,
                        position,
                        &mut revived,
                    ) {
                        hide_death_screen(&mut clients, entity);
                        hide_terminate_screen(&mut clients, entity);
                    }
                } else if terminate_lifecycle(
                    entity,
                    &mut lifecycle,
                    life_record,
                    &persistence,
                    clock.tick,
                    &mut terminated,
                    position.as_deref(),
                    &mut vfx_events,
                    "tribulation_failed",
                ) {
                    emit_terminate_screen(
                        &mut clients,
                        entity,
                        "终焉之言未竟。",
                        "劫数已定，形神俱散。",
                        "凡人",
                    );
                    hide_death_screen(&mut clients, entity);
                }
            }
            RevivalActionKind::Terminate => {
                if lifecycle.state != LifecycleState::AwaitingRevival {
                    continue;
                }
                let Some(decision) = lifecycle.awaiting_decision else {
                    continue;
                };
                if !decision.can_terminate() {
                    continue;
                }

                if terminate_lifecycle(
                    entity,
                    &mut lifecycle,
                    life_record,
                    &persistence,
                    intent.issued_at_tick,
                    &mut terminated,
                    position.as_deref(),
                    &mut vfx_events,
                    "voluntary_retire",
                ) {
                    emit_terminate_screen(
                        &mut clients,
                        entity,
                        "此身止于此。",
                        "你选择了归隐与终结。",
                        "凡人",
                    );
                    hide_death_screen(&mut clients, entity);
                }
            }
            RevivalActionKind::CreateNewCharacter => {
                if lifecycle.state != LifecycleState::Terminated {
                    continue;
                }
                reset_for_new_character(
                    entity,
                    &mut commands,
                    clock.tick,
                    &mut lifecycle,
                    life_record,
                    death_registry,
                    lifespan,
                    player_state,
                    position,
                    wounds,
                    stamina,
                    combat_state,
                    username,
                    inventory,
                    skill_set,
                    player_persistence.as_deref(),
                    default_loadout.as_deref(),
                    inventory_allocator.as_deref_mut(),
                );
                hide_death_screen(&mut clients, entity);
                hide_terminate_screen(&mut clients, entity);
            }
        }
    }
}

pub fn auto_confirm_revival_decisions(
    clock: Res<CombatClock>,
    mut revival_tx: EventWriter<RevivalActionIntent>,
    lifecycle_q: Query<(Entity, &Lifecycle)>,
) {
    for (entity, lifecycle) in &lifecycle_q {
        if lifecycle.state != LifecycleState::AwaitingRevival {
            continue;
        }
        let Some(deadline_tick) = lifecycle.revival_decision_deadline_tick else {
            continue;
        };
        if clock.tick < deadline_tick {
            continue;
        }
        revival_tx.send(RevivalActionIntent {
            entity,
            action: RevivalActionKind::Reincarnate,
            issued_at_tick: clock.tick,
        });
    }
}

fn death_penalty_lifespan_event(
    cultivation: Option<&Cultivation>,
    at_tick: u64,
    source: &str,
) -> Option<LifespanEventRecord> {
    let delta_years = -i64::from(match cultivation {
        Some(cultivation) => death_penalty_years(cultivation.realm),
        None => 4,
    });
    Some(LifespanEventRecord {
        at_tick,
        kind: "death_penalty".to_string(),
        delta_years,
        source: source.to_string(),
    })
}

fn lifespan_event_character_id(life_record: Option<&LifeRecord>, lifecycle: &Lifecycle) -> String {
    life_record
        .map(|record| record.character_id.clone())
        .unwrap_or_else(|| lifecycle.character_id.clone())
}

fn emit_death_lifespan_event(
    events: Option<&mut Events<LifespanEventEmitted>>,
    char_id: Option<String>,
    event: Option<&LifespanEventRecord>,
) {
    let (Some(events), Some(char_id), Some(event)) = (events, char_id, event) else {
        return;
    };
    events.send(LifespanEventEmitted {
        payload: crate::cultivation::lifespan::lifespan_event_payload_from_record(char_id, event),
    });
}

struct DeathInsightBuildInput<'a> {
    lifecycle: &'a Lifecycle,
    life_record: Option<&'a LifeRecord>,
    cultivation: Option<&'a Cultivation>,
    death_registry: Option<&'a DeathRegistry>,
    lifespan: Option<&'a LifespanComponent>,
    position: Option<&'a Position>,
    at_tick: u64,
    cause: &'a str,
    category: DeathInsightCategoryV1,
    zone_kind: ZoneDeathKind,
    rebirth_chance: Option<f64>,
    will_terminate: bool,
}

fn build_death_insight_request(input: DeathInsightBuildInput<'_>) -> DeathInsightRequestV1 {
    let death_count = death_count_for_current_insight(input.lifecycle, input.death_registry);
    let character_id = input
        .life_record
        .map(|record| record.character_id.clone())
        .unwrap_or_else(|| input.lifecycle.character_id.clone());
    let recent_biography = input
        .life_record
        .map(|record| {
            record
                .recent_summary(DEATH_INSIGHT_RECENT_BIO_N)
                .iter()
                .map(|entry| format!("{entry:?}"))
                .collect()
        })
        .unwrap_or_default();
    let position = input.position.map(|position| {
        let p = position.get();
        DeathInsightPositionV1 {
            x: p.x,
            y: p.y,
            z: p.z,
        }
    });

    DeathInsightRequestV1 {
        v: 1,
        request_id: format!(
            "death_insight:{}:{}:{}",
            character_id, input.at_tick, death_count
        ),
        character_id,
        at_tick: input.at_tick,
        cause: input.cause.to_string(),
        category: input.category,
        realm: input
            .cultivation
            .map(|cultivation| realm_to_string(cultivation.realm).to_string()),
        player_realm: input
            .cultivation
            .map(|cultivation| realm_to_string(cultivation.realm).to_string()),
        zone_kind: map_death_insight_zone_kind(input.zone_kind),
        death_count,
        rebirth_chance: input.rebirth_chance,
        lifespan_remaining_years: input.lifespan.map(LifespanComponent::remaining_years),
        recent_biography,
        position,
        context: serde_json::json!({
            "will_terminate": input.will_terminate,
            "fortune_remaining": input.lifecycle.fortune_remaining,
            "lifecycle_state": format!("{:?}", input.lifecycle.state),
        }),
    }
}

fn death_insight_category_from_cultivation_cause(
    cause: CultivationDeathCause,
) -> DeathInsightCategoryV1 {
    match cause {
        CultivationDeathCause::NaturalAging => DeathInsightCategoryV1::Natural,
        CultivationDeathCause::BreakthroughBackfire
        | CultivationDeathCause::MeridianCollapse
        | CultivationDeathCause::NegativeZoneDrain
        | CultivationDeathCause::ContaminationOverflow => DeathInsightCategoryV1::Cultivation,
    }
}

fn death_insight_category_from_revival_decision(
    base_category: DeathInsightCategoryV1,
    decision: Option<RevivalDecision>,
) -> DeathInsightCategoryV1 {
    if matches!(decision, Some(RevivalDecision::Tribulation { .. })) {
        DeathInsightCategoryV1::Tribulation
    } else {
        base_category
    }
}

fn death_count_for_current_insight(
    lifecycle: &Lifecycle,
    death_registry: Option<&DeathRegistry>,
) -> u32 {
    death_registry
        .map_or_else(
            || {
                if lifecycle_includes_current_death(lifecycle) {
                    lifecycle.death_count
                } else {
                    lifecycle.death_count.saturating_add(1)
                }
            },
            |registry| registry.death_count,
        )
        .max(1)
}

fn map_death_insight_zone_kind(zone_kind: ZoneDeathKind) -> DeathInsightZoneKindV1 {
    match zone_kind {
        ZoneDeathKind::Ordinary => DeathInsightZoneKindV1::Ordinary,
        ZoneDeathKind::Death => DeathInsightZoneKindV1::Death,
        ZoneDeathKind::Negative => DeathInsightZoneKindV1::Negative,
    }
}

fn apply_death_lifespan_penalty(
    cultivation: Option<&Cultivation>,
    lifespan: Option<&mut LifespanComponent>,
    _player_state: Option<&PlayerState>,
) -> bool {
    let Some(lifespan) = lifespan else {
        return false;
    };
    let cap = cultivation.map_or(LifespanCapTable::MORTAL, |cultivation| {
        LifespanCapTable::for_realm(cultivation.realm)
    });
    lifespan.apply_cap(cap);
    lifespan.years_lived += LifespanCapTable::death_penalty_years_for_cap(cap) as f64;
    lifespan.remaining_years() <= f64::EPSILON
}

fn apply_natural_aging_lifespan_exhaustion(
    cultivation: Option<&Cultivation>,
    lifespan: Option<&mut LifespanComponent>,
    _player_state: Option<&PlayerState>,
) {
    let Some(lifespan) = lifespan else {
        return;
    };
    let cap = cultivation.map_or(LifespanCapTable::MORTAL, |cultivation| {
        LifespanCapTable::for_realm(cultivation.realm)
    });
    lifespan.apply_cap(cap);
    lifespan.years_lived = lifespan.years_lived.max(cap as f64);
}

#[allow(clippy::too_many_arguments)]
fn determine_revival_decision(
    lifecycle: &Lifecycle,
    death_registry: Option<&DeathRegistry>,
    cause: &str,
    lifespan: Option<&LifespanComponent>,
    player_state: Option<&PlayerState>,
    position: Option<&Position>,
    zones: Option<&ZoneRegistry>,
    now_tick: u64,
) -> Option<RevivalDecision> {
    if lifespan.is_some_and(|lifespan| lifespan.remaining_years() <= f64::EPSILON) {
        return None;
    }

    let current_death_zone = death_zone_from_context(cause, position, zones);
    let (registry, includes_current_death, death_zone) = match death_registry {
        Some(registry) => (
            registry.clone(),
            true,
            registry.last_death_zone.unwrap_or(current_death_zone),
        ),
        None => {
            let includes_current_death = lifecycle_includes_current_death(lifecycle);
            let mut registry = DeathRegistry::new(lifecycle.character_id.clone());
            registry.death_count = lifecycle.death_count;
            registry.last_death_tick = lifecycle.last_death_tick;
            if includes_current_death {
                registry.last_death_zone = Some(current_death_zone);
            }
            (registry, includes_current_death, current_death_zone)
        }
    };
    let result = calculate_rebirth_chance(&RebirthChanceInput {
        registry,
        at_tick: now_tick,
        death_zone,
        karma: player_state.map_or(0.0, |state| state.karma),
        // plan-death-lifecycle-v1 §2：拥有"灵龛归属"可满足运数期保底条件。
        // MVP：以 Lifecycle.spawn_anchor 是否存在作为归属判定（社交侧揭露/失效规则后续接入）。
        has_shrine: lifecycle.spawn_anchor.is_some(),
        includes_current_death,
    });

    if lifecycle.fortune_remaining == 0 && result.guaranteed {
        return Some(RevivalDecision::Tribulation {
            chance: tribulation_rebirth_chance(result.death_number),
        });
    }

    if result.guaranteed {
        return Some(RevivalDecision::Fortune {
            chance: result.chance,
        });
    }

    if result.chance <= 0.0 {
        None
    } else {
        Some(RevivalDecision::Tribulation {
            chance: result.chance,
        })
    }
}

fn lifecycle_includes_current_death(lifecycle: &Lifecycle) -> bool {
    matches!(
        lifecycle.state,
        LifecycleState::NearDeath | LifecycleState::AwaitingRevival | LifecycleState::Terminated
    )
}

#[allow(clippy::too_many_arguments)]
fn revive_lifecycle(
    entity: Entity,
    now_tick: u64,
    persistence: &PersistenceSettings,
    lifecycle: &mut Lifecycle,
    cultivation: Option<valence::prelude::Mut<'_, Cultivation>>,
    meridians: Option<valence::prelude::Mut<'_, MeridianSystem>>,
    contam: Option<valence::prelude::Mut<'_, Contamination>>,
    life_record: Option<valence::prelude::Mut<'_, LifeRecord>>,
    wounds: Option<valence::prelude::Mut<'_, Wounds>>,
    stamina: Option<valence::prelude::Mut<'_, Stamina>>,
    combat_state: Option<valence::prelude::Mut<'_, CombatState>>,
    player_state: Option<valence::prelude::Mut<'_, PlayerState>>,
    position: Option<valence::prelude::Mut<'_, Position>>,
    revived: &mut EventWriter<PlayerRevived>,
) -> bool {
    let mut staged_lifecycle = lifecycle.clone();
    if matches!(
        lifecycle.awaiting_decision,
        Some(RevivalDecision::Fortune { .. })
    ) {
        staged_lifecycle.fortune_remaining = staged_lifecycle.fortune_remaining.saturating_sub(1);
    }
    staged_lifecycle.revive(now_tick);

    let mut staged_cultivation = cultivation.as_ref().map(|value| (**value).clone());
    let mut staged_meridians = meridians.as_ref().map(|value| (**value).clone());
    let mut staged_contam = contam.as_ref().map(|value| (**value).clone());
    let mut staged_life_record = life_record.as_ref().map(|value| (**value).clone());

    if let (
        Some(staged_cultivation),
        Some(staged_meridians),
        Some(staged_contam),
        Some(staged_life_record),
    ) = (
        staged_cultivation.as_mut(),
        staged_meridians.as_mut(),
        staged_contam.as_mut(),
        staged_life_record.as_mut(),
    ) {
        let prior_realm = staged_cultivation.realm;
        apply_revive_penalty(staged_cultivation, staged_meridians, staged_contam);
        staged_life_record.push(BiographyEntry::Rebirth {
            prior_realm,
            new_realm: staged_cultivation.realm,
            tick: now_tick,
        });
        if let Err(error) = persist_revival_transition(persistence, staged_life_record) {
            tracing::warn!(
                "[bong][persistence] failed to persist revival transition for {}: {error}",
                staged_life_record.character_id
            );
            return false;
        }
    }

    lifecycle.fortune_remaining = staged_lifecycle.fortune_remaining;
    lifecycle.revive(now_tick);
    if let (Some(mut cultivation), Some(staged_cultivation)) = (cultivation, staged_cultivation) {
        *cultivation = staged_cultivation;
    }
    if let (Some(mut meridians), Some(staged_meridians)) = (meridians, staged_meridians) {
        *meridians = staged_meridians;
    }
    if let (Some(mut contam), Some(staged_contam)) = (contam, staged_contam) {
        *contam = staged_contam;
    }
    if let (Some(mut life_record), Some(staged_life_record)) = (life_record, staged_life_record) {
        *life_record = staged_life_record;
    }

    if let Some(mut wounds) = wounds {
        wounds.entries.clear();
        wounds.health_current = (wounds.health_max * REVIVE_HEALTH_FRACTION).max(1.0);
    }
    if let Some(mut stamina) = stamina {
        stamina.current = stamina.max;
        stamina.state = StaminaState::Idle;
    }
    if let Some(mut combat_state) = combat_state {
        combat_state.incoming_window = None;
        combat_state.in_combat_until_tick = None;
        combat_state.last_attack_at_tick = None;
    }
    let _ = player_state;
    if let Some(mut position) = position {
        // worldview §十二：重生位置优先灵龛（如有）> 世界出生点。
        position.set(
            lifecycle
                .spawn_anchor
                .unwrap_or_else(crate::player::spawn_position),
        );
    }

    revived.send(PlayerRevived { entity });
    true
}

#[allow(clippy::too_many_arguments)]
fn terminate_lifecycle(
    entity: Entity,
    lifecycle: &mut Lifecycle,
    life_record: Option<valence::prelude::Mut<'_, LifeRecord>>,
    persistence: &PersistenceSettings,
    now_tick: u64,
    terminated: &mut EventWriter<PlayerTerminated>,
    position: Option<&Position>,
    vfx_events: &mut EventWriter<VfxEventRequest>,
    cause: &str,
) -> bool {
    terminate_lifecycle_with_death_context(
        entity,
        lifecycle,
        life_record,
        persistence,
        now_tick,
        terminated,
        position,
        vfx_events,
        cause,
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn terminate_lifecycle_with_death_context(
    entity: Entity,
    lifecycle: &mut Lifecycle,
    life_record: Option<valence::prelude::Mut<'_, LifeRecord>>,
    persistence: &PersistenceSettings,
    now_tick: u64,
    terminated: &mut EventWriter<PlayerTerminated>,
    position: Option<&Position>,
    vfx_events: &mut EventWriter<VfxEventRequest>,
    cause: &str,
    death_registry_cause: Option<&str>,
    lifespan_event: Option<LifespanEventRecord>,
) -> bool {
    let Some(mut life_record) = life_record else {
        if death_registry_cause.is_some()
            && !matches!(
                lifecycle.state,
                LifecycleState::NearDeath | LifecycleState::AwaitingRevival
            )
        {
            lifecycle.death_count = lifecycle.death_count.saturating_add(1);
        }
        lifecycle.terminate(now_tick);
        terminated.send(PlayerTerminated { entity });
        return true;
    };
    life_record.push(BiographyEntry::Terminated {
        cause: cause.to_string(),
        tick: now_tick,
    });
    let mut staged_lifecycle = lifecycle.clone();
    let should_record_direct_death = death_registry_cause.is_some()
        && !matches!(
            lifecycle.state,
            LifecycleState::NearDeath | LifecycleState::AwaitingRevival
        );
    if should_record_direct_death {
        staged_lifecycle.death_count = staged_lifecycle.death_count.saturating_add(1);
    }
    staged_lifecycle.terminate(now_tick);
    let persist_result = if death_registry_cause.is_some() || lifespan_event.is_some() {
        persist_termination_transition_with_death_context(
            persistence,
            &staged_lifecycle,
            &life_record,
            death_registry_cause,
            lifespan_event.as_ref(),
        )
    } else {
        persist_termination_transition(persistence, &staged_lifecycle, &life_record)
    };
    if let Err(error) = persist_result {
        tracing::warn!(
            "[bong][persistence] failed to persist terminated snapshot for {}: {error}",
            life_record.character_id
        );
        let _ = life_record.biography.pop();
        return false;
    }
    if should_record_direct_death {
        lifecycle.death_count = lifecycle.death_count.saturating_add(1);
    }
    lifecycle.terminate(now_tick);
    terminated.send(PlayerTerminated { entity });

    if let Some(pos) = position {
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
    true
}

#[allow(clippy::too_many_arguments)]
fn reset_for_new_character(
    entity: Entity,
    commands: &mut valence::prelude::Commands,
    now_tick: u64,
    lifecycle: &mut Lifecycle,
    life_record: Option<valence::prelude::Mut<'_, LifeRecord>>,
    death_registry: Option<valence::prelude::Mut<'_, DeathRegistry>>,
    lifespan: Option<valence::prelude::Mut<'_, LifespanComponent>>,
    player_state: Option<valence::prelude::Mut<'_, PlayerState>>,
    position: Option<valence::prelude::Mut<'_, Position>>,
    wounds: Option<valence::prelude::Mut<'_, Wounds>>,
    stamina: Option<valence::prelude::Mut<'_, Stamina>>,
    combat_state: Option<valence::prelude::Mut<'_, CombatState>>,
    username: Option<&Username>,
    inventory: Option<valence::prelude::Mut<'_, PlayerInventory>>,
    skill_set: Option<valence::prelude::Mut<'_, SkillSet>>,
    player_persistence: Option<&PlayerStatePersistence>,
    default_loadout: Option<&DefaultLoadout>,
    inventory_allocator: Option<&mut InventoryInstanceIdAllocator>,
) {
    let mut next_character_id = None;
    if let (Some(username), Some(player_persistence)) = (username, player_persistence) {
        if let Ok(next_char_id) =
            rotate_current_character_id(player_persistence, username.0.as_str())
        {
            tracing::info!(
                "[bong][combat] rotated current_char_id for `{}` to {next_char_id}",
                username.0
            );
            next_character_id = Some(player_character_id(username.0.as_str(), &next_char_id));
        }

        // 新角色与前角色无机制关联；灵龛归属同样不继承。
        if let Err(error) =
            save_player_shrine_anchor_slice(player_persistence, username.0.as_str(), None)
        {
            tracing::warn!(
                "[bong][combat] failed to clear persisted shrine anchor for `{}`: {error}",
                username.0
            );
        }
    }

    if let Some(next_character_id) = next_character_id {
        lifecycle.character_id = next_character_id;
    }

    lifecycle.death_count = 0;
    lifecycle.fortune_remaining = 3;
    lifecycle.last_death_tick = None;
    lifecycle.last_revive_tick = Some(now_tick);
    // 新角色与前角色无机制关联；灵龛归属同样不继承。
    lifecycle.spawn_anchor = None;
    lifecycle.near_death_deadline_tick = None;
    lifecycle.awaiting_decision = None;
    lifecycle.revival_decision_deadline_tick = None;
    lifecycle.weakened_until_tick = None;
    lifecycle.state = LifecycleState::Alive;

    if let Some(mut life_record) = life_record {
        *life_record = LifeRecord::new(lifecycle.character_id.clone());
    }

    let default_player_state = PlayerState::default();
    let spawn_position = crate::player::spawn_position();
    let fresh_lifespan = LifespanComponent::new(LifespanCapTable::MORTAL);

    if let Some(mut death_registry) = death_registry {
        *death_registry = DeathRegistry::new(lifecycle.character_id.clone());
    }
    if let Some(mut lifespan) = lifespan {
        *lifespan = fresh_lifespan.clone();
    } else {
        commands.entity(entity).insert(fresh_lifespan.clone());
    }
    if let Some(mut player_state) = player_state {
        *player_state = default_player_state.clone();
    }
    if let Some(mut position) = position {
        position.set(spawn_position);
    }
    let mut persisted_inventory = None;
    if let (Some(default_loadout), Some(inventory_allocator)) =
        (default_loadout, inventory_allocator)
    {
        let new_inventory =
            instantiate_inventory_from_loadout(&default_loadout.0, inventory_allocator)
                .expect("default loadout should instantiate");
        if let Some(mut inventory) = inventory {
            *inventory = new_inventory.clone();
        } else {
            commands.entity(entity).insert(new_inventory.clone());
        }
        persisted_inventory = Some(new_inventory);
    }
    if let Some(mut wounds) = wounds {
        *wounds = Wounds::default();
    }
    if let Some(mut stamina) = stamina {
        *stamina = Stamina::default();
    }
    if let Some(mut combat_state) = combat_state {
        *combat_state = CombatState::default();
    }
    if let Some(mut skill_set) = skill_set {
        *skill_set = SkillSet::default();
    } else {
        commands.entity(entity).insert(SkillSet::default());
    }

    let mut learned_recipes = LearnedRecipes::default();
    learned_recipes.learn("kai_mai_pill_v0".into());
    let mut entity_commands = commands.entity(entity);
    entity_commands.insert((
        Cultivation::default(),
        MeridianSystem::default(),
        QiColor::default(),
        Karma::default(),
        PracticeLog::default(),
        Contamination::default(),
        crate::cultivation::insight::InsightQuota::default(),
        crate::cultivation::insight_apply::UnlockedPerceptions::default(),
        crate::cultivation::insight_apply::InsightModifiers::new(),
        StatusEffects::default(),
        DerivedAttrs::default(),
        QuickSlotBindings::default(),
    ));
    entity_commands.insert((
        SkillBarBindings::default(),
        UnlockedStyles::default(),
        KnownTechniques::default(),
        learned_recipes,
    ));
    commands
        .entity(entity)
        .remove::<crate::combat::components::Casting>()
        .remove::<crate::cultivation::insight_flow::PendingInsightOffer>()
        .remove::<crate::cultivation::tribulation::TribulationState>()
        .remove::<crate::inventory::OverloadedMarker>();

    if let (Some(username), Some(player_persistence)) = (username, player_persistence) {
        if let Err(error) = save_player_slices(
            player_persistence,
            username.0.as_str(),
            &default_player_state,
            spawn_position,
            DimensionKind::default(),
            persisted_inventory.as_ref(),
            Some(&fresh_lifespan),
            &SkillSet::default(),
        ) {
            tracing::warn!(
                "[bong][combat] failed to persist fresh character slices for `{}`: {error}",
                username.0
            );
        }
    }
}

fn detect_zone_kind(
    position: Option<&Position>,
    zones: Option<&ZoneRegistry>,
) -> Option<ZoneDeathKind> {
    let position = position?;
    let zone = zones?.find_zone(DimensionKind::Overworld, position.get())?;
    if zone.spirit_qi < -0.2 {
        Some(ZoneDeathKind::Negative)
    } else {
        Some(ZoneDeathKind::Ordinary)
    }
}

fn death_zone_from_context(
    cause: &str,
    position: Option<&Position>,
    zones: Option<&ZoneRegistry>,
) -> ZoneDeathKind {
    let cause_lower = cause.to_ascii_lowercase();
    if cause_lower.contains("negative") {
        return ZoneDeathKind::Negative;
    }
    if cause_lower.contains("death") {
        return ZoneDeathKind::Death;
    }
    detect_zone_kind(position, zones).unwrap_or(ZoneDeathKind::Ordinary)
}

fn roll_rebirth(now_tick: u64, entity: Entity, chance: f64) -> bool {
    if chance >= 1.0 {
        return true;
    }
    let seed = now_tick ^ ((entity.index() as u64) << 32) ^ entity.generation() as u64;
    let mixed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    let sample = ((mixed >> 11) as f64) / ((1u64 << 53) as f64);
    sample < chance
}

fn eventual_cause(life_record: Option<&LifeRecord>) -> String {
    match life_record.and_then(|record| record.biography.last()) {
        Some(BiographyEntry::NearDeath { cause, .. }) => cause.clone(),
        _ => "unknown".to_string(),
    }
}

fn current_unix_millis() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_millis().min(u128::from(u64::MAX)) as u64,
        Err(_) => 0,
    }
}

fn decision_deadline_ms(decision_deadline_tick: u64, now_tick: u64) -> u64 {
    let remaining_ticks = decision_deadline_tick.saturating_sub(now_tick);
    current_unix_millis().saturating_add(remaining_ticks.saturating_mul(50))
}

fn emit_death_screen(
    clients: &mut Query<&mut valence::prelude::Client>,
    entity: Entity,
    cause: &str,
    decision: RevivalDecision,
    context: DeathScreenContext<'_>,
    now_tick: u64,
    decision_deadline_tick: u64,
) {
    let zone_kind = death_zone_from_context(cause, context.position, context.zones);
    send_payload(
        clients,
        entity,
        ServerDataV1::new(ServerDataPayloadV1::DeathScreen {
            visible: true,
            cause: cause.to_string(),
            luck_remaining: decision.chance_shown(),
            final_words: vec!["尘归尘，劫未尽。".to_string()],
            countdown_until_ms: decision_deadline_ms(decision_deadline_tick, now_tick),
            can_reincarnate: decision.can_reincarnate(),
            can_terminate: decision.can_terminate(),
            stage: Some(death_screen_stage(decision)),
            death_number: Some(
                context
                    .death_registry
                    .map_or(context.lifecycle.death_count, |registry| {
                        registry.death_count.max(context.lifecycle.death_count)
                    }),
            ),
            zone_kind: Some(death_screen_zone_kind(zone_kind)),
            lifespan: context.lifespan.map(|lifespan| {
                death_screen_lifespan_preview(lifespan, context.position, context.zones)
            }),
        }),
    );
}

fn emit_terminate_screen(
    clients: &mut Query<&mut valence::prelude::Client>,
    entity: Entity,
    final_words: &str,
    epilogue: &str,
    archetype_suggestion: &str,
) {
    send_payload(
        clients,
        entity,
        ServerDataV1::new(ServerDataPayloadV1::TerminateScreen {
            visible: true,
            final_words: final_words.to_string(),
            epilogue: epilogue.to_string(),
            archetype_suggestion: archetype_suggestion.to_string(),
        }),
    );
}

fn hide_death_screen(clients: &mut Query<&mut valence::prelude::Client>, entity: Entity) {
    send_payload(
        clients,
        entity,
        ServerDataV1::new(ServerDataPayloadV1::DeathScreen {
            visible: false,
            cause: String::new(),
            luck_remaining: 0.0,
            final_words: Vec::new(),
            countdown_until_ms: 0,
            can_reincarnate: false,
            can_terminate: false,
            stage: None,
            death_number: None,
            zone_kind: None,
            lifespan: None,
        }),
    );
}

fn death_screen_stage(decision: RevivalDecision) -> DeathScreenStageV1 {
    match decision {
        RevivalDecision::Fortune { .. } => DeathScreenStageV1::Fortune,
        RevivalDecision::Tribulation { .. } => DeathScreenStageV1::Tribulation,
    }
}

fn death_screen_zone_kind(kind: ZoneDeathKind) -> DeathScreenZoneKindV1 {
    match kind {
        ZoneDeathKind::Ordinary => DeathScreenZoneKindV1::Ordinary,
        ZoneDeathKind::Death => DeathScreenZoneKindV1::Death,
        ZoneDeathKind::Negative => DeathScreenZoneKindV1::Negative,
    }
}

fn death_screen_lifespan_preview(
    lifespan: &LifespanComponent,
    position: Option<&Position>,
    zones: Option<&ZoneRegistry>,
) -> LifespanPreviewV1 {
    LifespanPreviewV1 {
        years_lived: lifespan.years_lived,
        cap_by_realm: lifespan.cap_by_realm,
        remaining_years: lifespan.remaining_years(),
        death_penalty_years: LifespanCapTable::death_penalty_years_for_cap(lifespan.cap_by_realm),
        tick_rate_multiplier: lifespan_tick_rate_multiplier(position, zones),
        is_wind_candle: lifespan.is_wind_candle(),
    }
}

fn hide_terminate_screen(clients: &mut Query<&mut valence::prelude::Client>, entity: Entity) {
    send_payload(
        clients,
        entity,
        ServerDataV1::new(ServerDataPayloadV1::TerminateScreen {
            visible: false,
            final_words: String::new(),
            epilogue: String::new(),
            archetype_suggestion: String::new(),
        }),
    );
}

fn send_payload(
    clients: &mut Query<&mut valence::prelude::Client>,
    entity: Entity,
    payload: ServerDataV1,
) {
    let payload_type = payload_type_label(payload.payload_type());
    let Ok(payload_bytes) = serialize_server_data_payload(&payload) else {
        return;
    };
    if let Ok(mut client) = clients.get_mut(entity) {
        send_server_data_payload(&mut client, payload_bytes.as_slice());
        tracing::info!(
            "[bong][network] sent {} {} payload to client entity {entity:?}",
            SERVER_DATA_CHANNEL,
            payload_type
        );
    }
}

fn death_penalty_years(realm: Realm) -> i32 {
    match realm {
        Realm::Awaken => 6,
        Realm::Induce => 10,
        Realm::Condense => 17,
        Realm::Solidify => 30,
        Realm::Spirit => 50,
        Realm::Void => 100,
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
        BodyPart, DefenseWindow, Wound, WoundKind, IN_COMBAT_WINDOW_TICKS, JIEMAI_DEFENSE_WINDOW_MS,
    };
    use crate::combat::events::{DefenseIntent, RevivalActionIntent, RevivalActionKind};
    use crate::cultivation::death_hooks::CultivationDeathCause;
    use crate::cultivation::life_record::LifeRecord;
    use crate::cultivation::tick::CultivationClock;
    use crate::network::agent_bridge::SERVER_DATA_CHANNEL;
    use crate::persistence::{
        bootstrap_sqlite, DeceasedIndexEntry, DeceasedSnapshot, PersistenceSettings,
    };
    use crate::player::state::player_character_id;
    use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
    use rusqlite::{params, Connection};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use valence::prelude::{App, Events, IntoSystemConfigs, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

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

    fn spawn_client_actor(
        app: &mut App,
        username: &str,
        wounds: Wounds,
        stamina: Stamina,
        lifecycle: Lifecycle,
    ) -> (Entity, MockClientHelper) {
        let (mut client_bundle, helper) = create_mock_client(username);
        client_bundle.player.position = Position::new([8.0, 66.0, 8.0]);
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                wounds,
                stamina,
                CombatState::default(),
                LifeRecord::new(crate::player::state::canonical_player_id(username)),
                lifecycle,
            ))
            .id();
        (entity, helper)
    }

    fn flush_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut valence::prelude::Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush successfully");
        }
    }

    fn collect_server_data_payloads(helper: &mut MockClientHelper) -> Vec<ServerDataV1> {
        let mut payloads = Vec::new();
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }
            payloads.push(
                serde_json::from_slice(packet.data.0 .0)
                    .expect("server_data payload should decode"),
            );
        }
        payloads
    }

    fn unique_temp_dir(test_name: &str) -> PathBuf {
        let unique_suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "bong-combat-lifecycle-{test_name}-{}-{unique_suffix}",
            std::process::id()
        ))
    }

    fn persistence_settings(test_name: &str) -> (PersistenceSettings, PathBuf) {
        let root = unique_temp_dir(test_name);
        let db_path = root.join("data").join("bong.db");
        let deceased_dir = root.join("library-web").join("public").join("deceased");
        bootstrap_sqlite(&db_path, &format!("combat-lifecycle-{test_name}"))
            .expect("sqlite bootstrap should succeed");
        (
            PersistenceSettings::with_paths(
                &db_path,
                &deceased_dir,
                format!("combat-lifecycle-{test_name}"),
            ),
            root,
        )
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
    fn death_arbiter_timeout_enters_awaiting_revival_when_fortune_remains() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("revive-existing");
        app.insert_resource(settings);
        app.insert_resource(CombatClock { tick: 100 });
        app.add_event::<DeathEvent>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<DeathInsightRequested>();
        app.add_event::<PlayerRevived>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<RevivalActionIntent>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(
            Update,
            (
                death_arbiter_tick,
                near_death_tick.after(death_arbiter_tick),
                handle_revival_action_intents.after(near_death_tick),
            ),
        );

        let (entity, mut helper) = spawn_client_actor(
            &mut app,
            "Azure",
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
            attacker: None,
            attacker_player_id: None,
            at_tick: 100,
        });
        app.update();

        {
            let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
            assert_eq!(lifecycle.state, LifecycleState::NearDeath);
            assert_eq!(lifecycle.death_count, 1);
            let insight_events = app.world().resource::<Events<DeathInsightRequested>>();
            let mut insight_reader = insight_events.get_reader();
            let insights: Vec<_> = insight_reader.read(insight_events).cloned().collect();
            assert_eq!(insights.len(), 1);
            assert_eq!(insights[0].payload.character_id, "offline:Azure");
            assert_eq!(insights[0].payload.cause, "test");
            assert_eq!(insights[0].payload.category, DeathInsightCategoryV1::Combat);
        }

        app.world_mut().resource_mut::<CombatClock>().tick = 701;
        app.update();
        flush_client_packets(&mut app);

        let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
        let revived_events = app.world().resource::<Events<PlayerRevived>>();
        assert_eq!(lifecycle.state, LifecycleState::AwaitingRevival);
        assert!(matches!(
            lifecycle.awaiting_decision,
            Some(RevivalDecision::Fortune { chance }) if (chance - 1.0).abs() < 1e-9
        ));
        assert_eq!(revived_events.len(), 0);

        let payloads = collect_server_data_payloads(&mut helper);
        assert!(payloads.iter().any(|payload| matches!(
            payload.payload,
            ServerDataPayloadV1::DeathScreen {
                visible: true,
                can_reincarnate: true,
                can_terminate: false,
                stage: Some(DeathScreenStageV1::Fortune),
                ..
            }
        )));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cultivation_death_without_fortune_enters_awaiting_revival_after_deadline() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("terminate-existing");
        app.insert_resource(settings);
        app.insert_resource(CombatClock { tick: 40 });
        app.add_event::<DeathEvent>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<DeathInsightRequested>();
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
        assert_eq!(lifecycle.state, LifecycleState::AwaitingRevival);
        assert!(matches!(
            lifecycle.awaiting_decision,
            Some(RevivalDecision::Tribulation { chance }) if (chance - 0.80).abs() < 1e-9
        ));
        assert_eq!(terminated_events.len(), 0);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn repeated_death_events_do_not_extend_near_death_deadline() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 10 });
        let (settings, root) = persistence_settings("repeated-death");
        app.insert_resource(settings.clone());
        app.add_event::<DeathEvent>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<DeathInsightRequested>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<VfxEventRequest>();
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
            attacker: None,
            attacker_player_id: None,
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
            attacker: None,
            attacker_player_id: None,
            at_tick: 200,
        });
        app.update();

        let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
        assert_eq!(lifecycle.state, LifecycleState::NearDeath);
        assert_eq!(lifecycle.near_death_deadline_tick, first_deadline);
        assert_eq!(lifecycle.death_count, 1);
        let insight_events = app.world().resource::<Events<DeathInsightRequested>>();
        let mut insight_reader = insight_events.get_reader();
        let insights: Vec<_> = insight_reader.read(insight_events).cloned().collect();
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].payload.character_id, "unassigned:life_record");
        assert_eq!(insights[0].payload.cause, "first");
        assert_eq!(insights[0].payload.category, DeathInsightCategoryV1::Combat);

        let connection = Connection::open(settings.db_path()).expect("db should open");
        let life_event_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM life_events WHERE char_id = ?1",
                params!["unassigned:life_record"],
                |row| row.get(0),
            )
            .expect("life_events query should succeed");
        assert_eq!(life_event_count, 1);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn missing_death_registry_uses_lifecycle_death_count_for_tribulation_stage() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("lifecycle-count-without-registry");
        app.insert_resource(settings);
        app.insert_resource(CombatClock { tick: 200 });
        app.add_event::<DeathEvent>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<DeathInsightRequested>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, death_arbiter_tick);

        let entity = app
            .world_mut()
            .spawn((
                Wounds::default(),
                Stamina::default(),
                CombatState::default(),
                Lifecycle {
                    character_id: "offline:FourthDeath".to_string(),
                    death_count: 3,
                    fortune_remaining: 3,
                    last_death_tick: Some(1),
                    ..Default::default()
                },
                LifeRecord::new("offline:FourthDeath"),
            ))
            .id();

        app.world_mut().send_event(DeathEvent {
            target: entity,
            cause: "bleed_out".to_string(),
            attacker: None,
            attacker_player_id: None,
            at_tick: 200,
        });
        app.update();

        let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
        assert_eq!(lifecycle.state, LifecycleState::NearDeath);
        assert_eq!(lifecycle.death_count, 4);

        let insight_events = app.world().resource::<Events<DeathInsightRequested>>();
        let mut insight_reader = insight_events.get_reader();
        let insights: Vec<_> = insight_reader.read(insight_events).cloned().collect();
        assert_eq!(insights.len(), 1);
        let payload = &insights[0].payload;
        assert_eq!(payload.character_id, "offline:FourthDeath");
        assert_eq!(payload.death_count, 4);
        assert_eq!(payload.category, DeathInsightCategoryV1::Tribulation);
        assert_eq!(payload.rebirth_chance, Some(0.65));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn natural_aging_death_emits_natural_death_insight_request() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("natural-aging-insight");
        app.insert_resource(settings.clone());
        app.insert_resource(CombatClock { tick: 440 });
        app.add_event::<DeathEvent>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<DeathInsightRequested>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, death_arbiter_tick);

        let entity = app
            .world_mut()
            .spawn((
                Wounds::default(),
                Stamina::default(),
                CombatState::default(),
                Lifecycle {
                    character_id: "offline:Ancestor".to_string(),
                    death_count: 4,
                    fortune_remaining: 0,
                    last_death_tick: Some(300),
                    ..Default::default()
                },
                Cultivation {
                    realm: Realm::Condense,
                    ..Default::default()
                },
                LifeRecord::new("offline:Ancestor"),
                DeathRegistry {
                    char_id: "offline:Ancestor".to_string(),
                    death_count: 4,
                    last_death_tick: Some(300),
                    prev_death_tick: None,
                    last_death_zone: Some(ZoneDeathKind::Ordinary),
                },
                LifespanComponent {
                    born_at_tick: 0,
                    years_lived: 349.0,
                    cap_by_realm: LifespanCapTable::CONDENSE,
                    offline_pause_tick: None,
                },
                Position::new([9.0, 80.0, -3.0]),
            ))
            .id();

        app.world_mut().send_event(CultivationDeathTrigger {
            entity,
            cause: CultivationDeathCause::NaturalAging,
            context: serde_json::json!({"source": "lifespan_tick"}),
        });
        app.update();

        let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
        assert_eq!(lifecycle.state, LifecycleState::Terminated);
        assert_eq!(lifecycle.death_count, 5);
        assert_eq!(lifecycle.last_death_tick, Some(440));
        let lifespan = app
            .world()
            .entity(entity)
            .get::<LifespanComponent>()
            .expect("lifespan should remain attached");
        assert_eq!(lifespan.years_lived, LifespanCapTable::CONDENSE as f64);
        assert_eq!(lifespan.remaining_years(), 0.0);
        let insight_events = app.world().resource::<Events<DeathInsightRequested>>();
        let mut insight_reader = insight_events.get_reader();
        let insights: Vec<_> = insight_reader.read(insight_events).cloned().collect();
        assert_eq!(insights.len(), 1);
        let payload = &insights[0].payload;
        assert_eq!(payload.v, 1);
        assert_eq!(payload.character_id, "offline:Ancestor");
        assert_eq!(payload.cause, "cultivation:NaturalAging");
        assert_eq!(payload.category, DeathInsightCategoryV1::Natural);
        assert_eq!(payload.realm.as_deref(), Some("Condense"));
        assert_eq!(payload.death_count, 5);
        assert_eq!(payload.lifespan_remaining_years, Some(0.0));
        assert_eq!(payload.zone_kind, DeathInsightZoneKindV1::Ordinary);
        assert_eq!(payload.context["will_terminate"], true);

        let connection = Connection::open(settings.db_path()).expect("db should open");
        let death_registry: (i64, i64, String) = connection
            .query_row(
                "SELECT death_count, last_death_tick, last_death_cause FROM death_registry WHERE char_id = ?1",
                params!["offline:Ancestor"],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("natural end should persist death registry");
        let lifespan_events: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM lifespan_events WHERE char_id = ?1 AND event_type = 'death_penalty'",
                params!["offline:Ancestor"],
                |row| row.get(0),
            )
            .expect("lifespan event count should be readable");
        assert_eq!(
            death_registry,
            (5, 440, "cultivation:NaturalAging".to_string())
        );
        assert_eq!(lifespan_events, 0);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn negative_zone_death_insight_is_classified_as_tribulation_before_fourth_death() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("negative-zone-tribulation-insight");
        app.insert_resource(settings);
        app.insert_resource(CombatClock { tick: 120 });
        app.add_event::<DeathEvent>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<DeathInsightRequested>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, death_arbiter_tick);

        let entity = app
            .world_mut()
            .spawn((
                Wounds::default(),
                Stamina::default(),
                CombatState::default(),
                Lifecycle {
                    character_id: "offline:DepthWalker".to_string(),
                    fortune_remaining: 3,
                    ..Default::default()
                },
                LifeRecord::new("offline:DepthWalker"),
                DeathRegistry::new("offline:DepthWalker"),
                Position::new([3.0, 55.0, -7.0]),
            ))
            .id();

        app.world_mut().send_event(DeathEvent {
            target: entity,
            cause: "negative_zone_drain".to_string(),
            attacker: None,
            attacker_player_id: None,
            at_tick: 120,
        });
        app.update();

        let insight_events = app.world().resource::<Events<DeathInsightRequested>>();
        let mut insight_reader = insight_events.get_reader();
        let insights: Vec<_> = insight_reader.read(insight_events).cloned().collect();
        assert_eq!(insights.len(), 1);
        let payload = &insights[0].payload;
        assert_eq!(payload.character_id, "offline:DepthWalker");
        assert_eq!(payload.death_count, 1);
        assert_eq!(payload.category, DeathInsightCategoryV1::Tribulation);
        assert_eq!(payload.zone_kind, DeathInsightZoneKindV1::Negative);
        assert_eq!(payload.rebirth_chance, Some(0.80));

        let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
        assert_eq!(lifecycle.state, LifecycleState::NearDeath);
        assert_eq!(lifecycle.death_count, 1);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn death_penalty_exhaustion_persists_registry_and_lifespan_event_before_termination() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("death-penalty-exhaustion");
        app.insert_resource(settings.clone());
        app.insert_resource(CombatClock { tick: 240 });
        app.add_event::<DeathEvent>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<DeathInsightRequested>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, death_arbiter_tick);

        let entity = app
            .world_mut()
            .spawn((
                Wounds::default(),
                Stamina::default(),
                CombatState::default(),
                Lifecycle {
                    character_id: "offline:ShortLived".to_string(),
                    ..Default::default()
                },
                Cultivation {
                    realm: Realm::Awaken,
                    ..Default::default()
                },
                LifeRecord::new("offline:ShortLived"),
                DeathRegistry::new("offline:ShortLived"),
                LifespanComponent {
                    born_at_tick: 0,
                    years_lived: LifespanCapTable::AWAKEN as f64 - 1.0,
                    cap_by_realm: LifespanCapTable::AWAKEN,
                    offline_pause_tick: None,
                },
                Position::new([2.0, 70.0, 2.0]),
            ))
            .id();

        app.world_mut().send_event(DeathEvent {
            target: entity,
            cause: "bleed_out".to_string(),
            attacker: None,
            attacker_player_id: None,
            at_tick: 240,
        });
        app.update();

        let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
        assert_eq!(lifecycle.state, LifecycleState::Terminated);
        assert_eq!(lifecycle.death_count, 1);
        let lifespan = app
            .world()
            .entity(entity)
            .get::<LifespanComponent>()
            .expect("lifespan should remain attached");
        assert_eq!(lifespan.remaining_years(), 0.0);

        let connection = Connection::open(settings.db_path()).expect("db should open");
        let death_registry: (i64, i64, String) = connection
            .query_row(
                "SELECT death_count, last_death_tick, last_death_cause FROM death_registry WHERE char_id = ?1",
                params!["offline:ShortLived"],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("death penalty exhaustion should persist death registry");
        let lifespan_payload_json: String = connection
            .query_row(
                "SELECT payload_json FROM lifespan_events WHERE char_id = ?1 AND event_type = 'death_penalty'",
                params!["offline:ShortLived"],
                |row| row.get(0),
            )
            .expect("death penalty lifespan event should persist");
        let lifespan_payload: LifespanEventRecord =
            serde_json::from_str(&lifespan_payload_json).expect("lifespan payload should decode");
        let snapshot: DeceasedSnapshot = serde_json::from_str(
            &fs::read_to_string(
                settings
                    .deceased_public_dir()
                    .join("offline_ShortLived.json"),
            )
            .expect("deceased snapshot should exist"),
        )
        .expect("deceased snapshot should decode");

        assert_eq!(death_registry, (1, 240, "bleed_out".to_string()));
        assert_eq!(lifespan_payload.kind, "death_penalty");
        assert_eq!(lifespan_payload.delta_years, -6);
        assert_eq!(lifespan_payload.source, "bleed_out");
        assert_eq!(snapshot.lifecycle.death_count, 1);
        assert_eq!(snapshot.termination_category, "善终");
        assert!(matches!(
            snapshot.life_record.biography.last(),
            Some(BiographyEntry::Terminated { cause, tick })
                if cause == "natural_end" && *tick == 240
        ));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn life_events_are_append_only_and_atomic_with_state_updates() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("append-only-atomic");
        app.insert_resource(settings.clone());
        app.insert_resource(CombatClock { tick: 90 });
        app.insert_resource(CultivationClock { tick: 691 });
        app.add_event::<DeathEvent>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<DeathInsightRequested>();
        app.add_event::<PlayerRevived>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<RevivalActionIntent>();
        app.add_event::<crate::skill::events::SkillCapChanged>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(
            Update,
            (
                death_arbiter_tick,
                near_death_tick.after(death_arbiter_tick),
                handle_revival_action_intents.after(near_death_tick),
                crate::cultivation::death_hooks::on_player_revived.after(near_death_tick),
                crate::cultivation::death_hooks::on_player_terminated.after(near_death_tick),
            ),
        );

        let entity = app
            .world_mut()
            .spawn((
                Wounds {
                    health_current: 0.0,
                    health_max: 30.0,
                    entries: Vec::new(),
                },
                Stamina::default(),
                CombatState::default(),
                Lifecycle {
                    character_id: "offline:Ancestor".to_string(),
                    fortune_remaining: 1,
                    ..Default::default()
                },
                crate::cultivation::components::Cultivation {
                    realm: Realm::Induce,
                    qi_current: 12.0,
                    qi_max: 24.0,
                    ..Default::default()
                },
                crate::cultivation::components::MeridianSystem::default(),
                crate::cultivation::components::Contamination::default(),
                LifeRecord::new("offline:Ancestor"),
            ))
            .id();

        app.world_mut().send_event(DeathEvent {
            target: entity,
            cause: "bleed_out".to_string(),
            attacker: None,
            attacker_player_id: None,
            at_tick: 90,
        });
        app.update();

        let connection = Connection::open(settings.db_path()).expect("db should open");
        let near_death_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM life_events WHERE char_id = ?1 AND event_type = 'near_death'",
                params!["offline:Ancestor"],
                |row| row.get(0),
            )
            .expect("near death count query should succeed");
        let lifespan_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM lifespan_events WHERE char_id = ?1 AND event_type = 'death_penalty'",
                params!["offline:Ancestor"],
                |row| row.get(0),
            )
            .expect("lifespan count query should succeed");
        let death_registry: (i64, i64, String) = connection
            .query_row(
                "SELECT death_count, last_death_tick, last_death_cause FROM death_registry WHERE char_id = ?1",
                params!["offline:Ancestor"],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("death registry should exist");

        assert_eq!(near_death_count, 1);
        assert_eq!(lifespan_count, 1);
        assert_eq!(death_registry, (1, 90, "bleed_out".to_string()));
        assert_eq!(
            app.world().entity(entity).get::<Lifecycle>().unwrap().state,
            LifecycleState::NearDeath
        );

        app.world_mut().resource_mut::<CombatClock>().tick = 691;
        app.update();
        app.world_mut().send_event(RevivalActionIntent {
            entity,
            action: RevivalActionKind::Reincarnate,
            issued_at_tick: 691,
        });
        app.update();

        let life_event_types: Vec<String> = connection
            .prepare(
                "SELECT event_type FROM life_events WHERE char_id = ?1 ORDER BY game_tick, event_id",
            )
            .expect("statement should prepare")
            .query_map(params!["offline:Ancestor"], |row| row.get(0))
            .expect("life_events query should succeed")
            .map(|row| row.expect("row should decode"))
            .collect();
        let lifespan_payload_json: String = connection
            .query_row(
                "SELECT payload_json FROM lifespan_events WHERE char_id = ?1 LIMIT 1",
                params!["offline:Ancestor"],
                |row| row.get(0),
            )
            .expect("lifespan payload should exist");
        let lifespan_payload: crate::persistence::LifespanEventRecord =
            serde_json::from_str(&lifespan_payload_json).expect("lifespan payload should decode");

        assert_eq!(
            life_event_types,
            vec!["near_death".to_string(), "rebirth".to_string()]
        );
        assert_eq!(lifespan_payload.delta_years, -10);
        assert_eq!(lifespan_payload.kind, "death_penalty");
        assert_eq!(
            app.world().entity(entity).get::<Lifecycle>().unwrap().state,
            LifecycleState::Alive
        );
        assert!(matches!(
            app.world()
                .entity(entity)
                .get::<LifeRecord>()
                .unwrap()
                .biography
                .last(),
            Some(BiographyEntry::Rebirth { tick: 691, .. })
        ));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn deceased_snapshot_export_writes_public_json_after_termination_confirmation() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("deceased-public-json");
        app.insert_resource(settings.clone());
        app.insert_resource(CombatClock { tick: 40 });
        app.add_event::<DeathEvent>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<DeathInsightRequested>();
        app.add_event::<PlayerRevived>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<RevivalActionIntent>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(
            Update,
            (
                death_arbiter_tick,
                near_death_tick.after(death_arbiter_tick),
                handle_revival_action_intents.after(near_death_tick),
                crate::cultivation::death_hooks::on_player_terminated.after(near_death_tick),
            ),
        );

        let entity = app
            .world_mut()
            .spawn((
                Wounds::default(),
                Stamina::default(),
                CombatState::default(),
                Lifecycle {
                    character_id: "offline:Ancestor".to_string(),
                    fortune_remaining: 0,
                    ..Default::default()
                },
                LifeRecord::new("offline:Ancestor"),
            ))
            .id();

        app.world_mut().send_event(CultivationDeathTrigger {
            entity,
            cause: CultivationDeathCause::NegativeZoneDrain,
            context: serde_json::json!({"zone": "rift_valley"}),
        });
        app.update();
        app.world_mut().resource_mut::<CombatClock>().tick = 641;
        app.update();
        app.world_mut().send_event(RevivalActionIntent {
            entity,
            action: RevivalActionKind::Terminate,
            issued_at_tick: 641,
        });
        app.update();

        let snapshot_path = settings.deceased_public_dir().join("offline_Ancestor.json");
        let index_path = settings.deceased_public_dir().join("_index.json");
        let snapshot: DeceasedSnapshot = serde_json::from_str(
            &fs::read_to_string(&snapshot_path).expect("snapshot file should exist"),
        )
        .expect("snapshot file should decode");
        let index: Vec<DeceasedIndexEntry> = serde_json::from_str(
            &fs::read_to_string(&index_path).expect("index file should exist"),
        )
        .expect("index file should decode");

        assert_eq!(snapshot.char_id, "offline:Ancestor");
        assert_eq!(snapshot.died_at_tick, 641);
        assert_eq!(snapshot.termination_category, "自主归隐");
        assert_eq!(snapshot.lifecycle.state, LifecycleState::Terminated);
        assert!(matches!(
            snapshot.life_record.biography.last(),
            Some(BiographyEntry::Terminated { tick: 641, .. })
        ));
        assert_eq!(index.len(), 1);
        assert_eq!(index[0].char_id, "offline:Ancestor");
        assert_eq!(index[0].path, "deceased/offline_Ancestor.json");
        assert_eq!(index[0].termination_category, "自主归隐");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn terminate_action_is_ignored_for_alive_and_fortune_stage_characters() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("terminate-gated");
        app.insert_resource(settings);
        app.insert_resource(CombatClock { tick: 120 });
        app.add_event::<RevivalActionIntent>();
        app.add_event::<PlayerRevived>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, handle_revival_action_intents);

        let alive = app
            .world_mut()
            .spawn((
                Lifecycle {
                    character_id: "offline:Alive".to_string(),
                    state: LifecycleState::Alive,
                    ..Default::default()
                },
                LifeRecord::new("offline:Alive"),
            ))
            .id();
        let fortune_stage = app
            .world_mut()
            .spawn((
                Lifecycle {
                    character_id: "offline:Fortune".to_string(),
                    state: LifecycleState::AwaitingRevival,
                    awaiting_decision: Some(RevivalDecision::Fortune { chance: 1.0 }),
                    revival_decision_deadline_tick: Some(200),
                    ..Default::default()
                },
                LifeRecord::new("offline:Fortune"),
            ))
            .id();

        app.world_mut().send_event(RevivalActionIntent {
            entity: alive,
            action: RevivalActionKind::Terminate,
            issued_at_tick: 120,
        });
        app.world_mut().send_event(RevivalActionIntent {
            entity: fortune_stage,
            action: RevivalActionKind::Terminate,
            issued_at_tick: 120,
        });
        app.update();

        assert_eq!(
            app.world().entity(alive).get::<Lifecycle>().unwrap().state,
            LifecycleState::Alive
        );
        assert_eq!(
            app.world()
                .entity(fortune_stage)
                .get::<Lifecycle>()
                .unwrap()
                .state,
            LifecycleState::AwaitingRevival
        );
        assert_eq!(app.world().resource::<Events<PlayerTerminated>>().len(), 0);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn fortune_stage_death_screen_disables_voluntary_termination() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("fortune-no-terminate-button");
        app.insert_resource(settings);
        app.insert_resource(CombatClock { tick: 100 });
        app.add_event::<DeathEvent>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<DeathInsightRequested>();
        app.add_event::<PlayerRevived>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<RevivalActionIntent>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(
            Update,
            (
                death_arbiter_tick,
                near_death_tick.after(death_arbiter_tick),
                handle_revival_action_intents.after(near_death_tick),
            ),
        );

        let (entity, mut helper) = spawn_client_actor(
            &mut app,
            "FortuneOnly",
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
            attacker: None,
            attacker_player_id: None,
            at_tick: 100,
        });
        app.update();
        app.world_mut().resource_mut::<CombatClock>().tick = 701;
        app.update();
        flush_client_packets(&mut app);

        let payloads = collect_server_data_payloads(&mut helper);
        assert!(payloads.iter().any(|payload| matches!(
            payload.payload,
            ServerDataPayloadV1::DeathScreen {
                visible: true,
                can_reincarnate: true,
                can_terminate: false,
                stage: Some(DeathScreenStageV1::Fortune),
                ..
            }
        )));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn create_new_character_rehydrates_default_character_state_and_persists_slices() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("create-new-character");
        let data_dir = root.join("data");
        app.insert_resource(settings.clone());
        app.insert_resource(PlayerStatePersistence::with_db_path(
            &data_dir,
            settings.db_path(),
        ));
        app.insert_resource(CombatClock { tick: 800 });

        let item_registry =
            crate::inventory::load_item_registry().expect("item registry should load");
        let default_loadout = crate::inventory::load_default_loadout(&item_registry)
            .expect("default loadout should load");
        app.insert_resource(DefaultLoadout(default_loadout));
        app.insert_resource(InventoryInstanceIdAllocator::default());

        app.add_event::<RevivalActionIntent>();
        app.add_event::<PlayerRevived>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, handle_revival_action_intents);

        let username = Username("Azure".to_string());
        let _ = save_player_slices(
            &PlayerStatePersistence::with_db_path(&data_dir, settings.db_path()),
            username.0.as_str(),
            &PlayerState {
                karma: 0.4,
                inventory_score: 0.8,
            },
            [99.0, 64.0, 99.0],
            DimensionKind::default(),
            None,
            None,
            &SkillSet::default(),
        );

        let entity = app
            .world_mut()
            .spawn((
                Wounds {
                    health_current: 0.0,
                    health_max: 30.0,
                    entries: vec![Wound {
                        location: BodyPart::Chest,
                        kind: WoundKind::Cut,
                        severity: 0.9,
                        bleeding_per_sec: 2.0,
                        created_at_tick: 1,
                        inflicted_by: Some("offline:Enemy".to_string()),
                    }],
                },
                Stamina {
                    current: 1.0,
                    max: 100.0,
                    recover_per_sec: 5.0,
                    last_drain_tick: Some(12),
                    state: StaminaState::Exhausted,
                },
                CombatState {
                    in_combat_until_tick: Some(900),
                    last_attack_at_tick: Some(700),
                    incoming_window: Some(DefenseWindow {
                        opened_at_tick: 700,
                        duration_ms: 100,
                    }),
                },
                Lifecycle {
                    character_id: "offline:Ancestor".to_string(),
                    state: LifecycleState::Terminated,
                    death_count: 9,
                    fortune_remaining: 0,
                    last_death_tick: Some(799),
                    ..Default::default()
                },
                LifeRecord::new("offline:Ancestor"),
                DeathRegistry {
                    char_id: "offline:Ancestor".to_string(),
                    death_count: 9,
                    last_death_tick: Some(799),
                    prev_death_tick: None,
                    last_death_zone: Some(ZoneDeathKind::Death),
                },
                LifespanComponent {
                    born_at_tick: 10,
                    years_lived: 79.0,
                    cap_by_realm: 80,
                    offline_pause_tick: Some(700),
                },
                PlayerState {
                    karma: 0.4,
                    inventory_score: 0.8,
                },
                Position::new([99.0, 64.0, 99.0]),
                username.clone(),
                SkillSet::default(),
            ))
            .id();

        app.world_mut().send_event(RevivalActionIntent {
            entity,
            action: RevivalActionKind::CreateNewCharacter,
            issued_at_tick: 800,
        });
        app.update();

        let entity_ref = app.world().entity(entity);
        let lifecycle = entity_ref
            .get::<Lifecycle>()
            .expect("lifecycle should remain attached");
        let death_registry = entity_ref
            .get::<DeathRegistry>()
            .expect("death registry should be reset for new character");
        let lifespan = entity_ref
            .get::<LifespanComponent>()
            .expect("lifespan should be reset for new character");
        let player_state = entity_ref
            .get::<PlayerState>()
            .expect("player state should remain attached");
        let position = entity_ref
            .get::<Position>()
            .expect("position should remain attached");
        let cultivation = entity_ref
            .get::<Cultivation>()
            .expect("cultivation should be reattached for new character");
        let meridians = entity_ref
            .get::<MeridianSystem>()
            .expect("meridians should be reattached for new character");
        let learned = entity_ref
            .get::<LearnedRecipes>()
            .expect("learned recipes should be reattached for new character");
        let inventory = entity_ref
            .get::<PlayerInventory>()
            .expect("inventory should be reinitialized for new character");

        assert_eq!(lifecycle.state, LifecycleState::Alive);
        let connection = Connection::open(settings.db_path()).expect("db should open");
        let current_char_id: String = connection
            .query_row(
                "SELECT current_char_id FROM player_core WHERE username = ?1",
                params![username.0.as_str()],
                |row| row.get(0),
            )
            .expect("current_char_id should persist");
        assert_eq!(
            lifecycle.character_id,
            player_character_id(username.0.as_str(), &current_char_id)
        );
        assert_eq!(lifecycle.death_count, 0);
        assert_eq!(lifecycle.fortune_remaining, 3);
        assert_eq!(death_registry.death_count, 0);
        assert_eq!(death_registry.char_id, lifecycle.character_id);
        assert_eq!(lifespan.cap_by_realm, LifespanCapTable::MORTAL);
        assert_eq!(lifespan.years_lived, 0.0);
        assert_eq!(player_state, &PlayerState::default());
        assert_eq!(
            position.get(),
            Position::new(crate::player::spawn_position()).get()
        );
        assert_eq!(cultivation.realm, Realm::Awaken);
        assert_eq!(cultivation.qi_current, 0.0);
        assert_eq!(cultivation.qi_max, 10.0);
        assert_eq!(meridians.opened_count(), 0);
        assert_eq!(learned.ids, vec!["kai_mai_pill_v0".to_string()]);
        assert!(inventory.revision.0 >= 1);

        let persisted = crate::player::state::load_player_slices(
            &PlayerStatePersistence::with_db_path(&data_dir, settings.db_path()),
            username.0.as_str(),
        );
        assert_eq!(persisted.state, PlayerState::default());
        assert_eq!(persisted.position, crate::player::spawn_position());
        assert!(persisted.inventory.is_some());
        let persisted_lifespan = persisted.lifespan.expect("fresh lifespan should persist");
        assert_eq!(persisted_lifespan.born_at_tick, 0);
        assert_eq!(persisted_lifespan.cap_by_realm, LifespanCapTable::MORTAL);
        assert!(persisted_lifespan.years_lived >= 0.0);
        assert!(persisted_lifespan.years_lived < 0.01);
        assert_eq!(persisted_lifespan.offline_pause_tick, None);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn create_new_character_uses_distinct_character_ids_for_deceased_exports() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("new-character-deceased-unique");
        let data_dir = root.join("data");
        app.insert_resource(settings.clone());
        app.insert_resource(PlayerStatePersistence::with_db_path(
            &data_dir,
            settings.db_path(),
        ));
        app.insert_resource(CombatClock { tick: 800 });

        let item_registry =
            crate::inventory::load_item_registry().expect("item registry should load");
        let default_loadout = crate::inventory::load_default_loadout(&item_registry)
            .expect("default loadout should load");
        app.insert_resource(DefaultLoadout(default_loadout));
        app.insert_resource(InventoryInstanceIdAllocator::default());

        app.add_event::<RevivalActionIntent>();
        app.add_event::<PlayerRevived>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, handle_revival_action_intents);

        let username = Username("Azure".to_string());
        save_player_slices(
            &PlayerStatePersistence::with_db_path(&data_dir, settings.db_path()),
            username.0.as_str(),
            &PlayerState::default(),
            crate::player::spawn_position(),
            DimensionKind::default(),
            None,
            None,
            &SkillSet::default(),
        )
        .expect("initial player slices should persist");

        let entity = app
            .world_mut()
            .spawn((
                Wounds::default(),
                Stamina::default(),
                CombatState::default(),
                Lifecycle {
                    character_id: "offline:Ancestor".to_string(),
                    state: LifecycleState::Terminated,
                    ..Default::default()
                },
                LifeRecord::new("offline:Ancestor"),
                DeathRegistry::new("offline:Ancestor"),
                LifespanComponent::new(LifespanCapTable::MORTAL),
                PlayerState::default(),
                Position::new(crate::player::spawn_position()),
                username.clone(),
                SkillSet::default(),
            ))
            .id();

        app.world_mut().send_event(RevivalActionIntent {
            entity,
            action: RevivalActionKind::CreateNewCharacter,
            issued_at_tick: 800,
        });
        app.update();
        let first_character_id = app
            .world()
            .entity(entity)
            .get::<Lifecycle>()
            .unwrap()
            .character_id
            .clone();

        {
            let mut lifecycle = app.world_mut().entity_mut(entity);
            *lifecycle.get_mut::<Lifecycle>().unwrap() = Lifecycle {
                character_id: first_character_id.clone(),
                state: LifecycleState::Terminated,
                ..Default::default()
            };
            *lifecycle.get_mut::<LifeRecord>().unwrap() =
                LifeRecord::new(first_character_id.clone());
        }
        app.world_mut().send_event(RevivalActionIntent {
            entity,
            action: RevivalActionKind::CreateNewCharacter,
            issued_at_tick: 801,
        });
        app.update();
        let second_character_id = app
            .world()
            .entity(entity)
            .get::<Lifecycle>()
            .unwrap()
            .character_id
            .clone();

        assert_ne!(first_character_id, second_character_id);

        let mut first_lifecycle = Lifecycle {
            character_id: first_character_id.clone(),
            state: LifecycleState::Terminated,
            ..Default::default()
        };
        let mut first_life_record = LifeRecord::new(first_character_id.clone());
        first_life_record.push(BiographyEntry::Terminated {
            cause: "voluntary_retire".to_string(),
            tick: 900,
        });
        first_lifecycle.terminate(900);
        persist_termination_transition(&settings, &first_lifecycle, &first_life_record)
            .expect("first terminated character should export");

        let mut second_lifecycle = Lifecycle {
            character_id: second_character_id.clone(),
            state: LifecycleState::Terminated,
            ..Default::default()
        };
        let mut second_life_record = LifeRecord::new(second_character_id.clone());
        second_life_record.push(BiographyEntry::Terminated {
            cause: "voluntary_retire".to_string(),
            tick: 901,
        });
        second_lifecycle.terminate(901);
        persist_termination_transition(&settings, &second_lifecycle, &second_life_record)
            .expect("second terminated character should export");

        let index_path = settings.deceased_public_dir().join("_index.json");
        let index: Vec<DeceasedIndexEntry> = serde_json::from_str(
            &fs::read_to_string(&index_path).expect("index file should exist"),
        )
        .expect("index file should decode");

        assert_eq!(index.len(), 2);
        assert!(index
            .iter()
            .any(|entry| entry.char_id == first_character_id));
        assert!(index
            .iter()
            .any(|entry| entry.char_id == second_character_id));
        assert_ne!(index[0].path, index[1].path);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn shrine_anchor_allows_fortune_stage_under_recent_death_and_high_karma() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("shrine-fortune-stage");
        app.insert_resource(settings);
        app.insert_resource(CombatClock { tick: 100 });
        app.add_event::<DeathEvent>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<DeathInsightRequested>();
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

        let player_state = PlayerState {
            karma: 0.9,
            inventory_score: 0.0,
        };

        let wounds = Wounds {
            health_current: 0.0,
            health_max: 30.0,
            entries: Vec::new(),
        };

        let without_shrine = app
            .world_mut()
            .spawn((
                wounds.clone(),
                Stamina::default(),
                CombatState::default(),
                Position::new([8.0, 66.0, 8.0]),
                Lifecycle {
                    fortune_remaining: 1,
                    spawn_anchor: None,
                    ..Default::default()
                },
                DeathRegistry {
                    char_id: "offline:NoShrine".to_string(),
                    death_count: 1,
                    // 当前死亡会在 death_arbiter_tick 内 record_death；这里模拟“上一次死亡”发生在 24h 内，
                    // 使 without_shrine 不满足运数期保底条件。
                    last_death_tick: Some(1),
                    prev_death_tick: None,
                    last_death_zone: Some(ZoneDeathKind::Ordinary),
                },
                player_state.clone(),
            ))
            .id();

        let with_shrine = app
            .world_mut()
            .spawn((
                wounds,
                Stamina::default(),
                CombatState::default(),
                Position::new([8.0, 66.0, 8.0]),
                Lifecycle {
                    fortune_remaining: 1,
                    spawn_anchor: Some([11.0, 22.0, 33.0]),
                    ..Default::default()
                },
                DeathRegistry {
                    char_id: "offline:WithShrine".to_string(),
                    death_count: 1,
                    last_death_tick: Some(1),
                    prev_death_tick: None,
                    last_death_zone: Some(ZoneDeathKind::Ordinary),
                },
                player_state,
            ))
            .id();

        app.world_mut().send_event(DeathEvent {
            target: without_shrine,
            cause: "test".to_string(),
            attacker: None,
            attacker_player_id: None,
            at_tick: 100,
        });
        app.world_mut().send_event(DeathEvent {
            target: with_shrine,
            cause: "test".to_string(),
            attacker: None,
            attacker_player_id: None,
            at_tick: 100,
        });
        app.update();

        app.world_mut().resource_mut::<CombatClock>().tick = 701;
        app.update();

        let lifecycle_without_shrine = app
            .world()
            .entity(without_shrine)
            .get::<Lifecycle>()
            .expect("lifecycle should exist");
        assert!(matches!(
            lifecycle_without_shrine.awaiting_decision,
            Some(RevivalDecision::Tribulation { chance }) if (chance - 0.80).abs() < 1e-9
        ));

        let lifecycle_with_shrine = app
            .world()
            .entity(with_shrine)
            .get::<Lifecycle>()
            .expect("lifecycle should exist");
        assert!(matches!(
            lifecycle_with_shrine.awaiting_decision,
            Some(RevivalDecision::Fortune { chance }) if (chance - 1.0).abs() < 1e-9
        ));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reincarnate_places_player_at_shrine_anchor_or_world_spawn() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("revive-spawn-anchor");
        app.insert_resource(settings);
        app.insert_resource(CombatClock { tick: 42 });
        app.add_event::<RevivalActionIntent>();
        app.add_event::<PlayerRevived>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, handle_revival_action_intents);

        let shrine_anchor = [123.0, 45.0, -67.0];

        let with_shrine = app
            .world_mut()
            .spawn((
                Position::new([99.0, 64.0, 99.0]),
                Lifecycle {
                    state: LifecycleState::AwaitingRevival,
                    awaiting_decision: Some(RevivalDecision::Fortune { chance: 1.0 }),
                    spawn_anchor: Some(shrine_anchor),
                    fortune_remaining: 1,
                    ..Default::default()
                },
            ))
            .id();

        let without_shrine = app
            .world_mut()
            .spawn((
                Position::new([99.0, 64.0, 99.0]),
                Lifecycle {
                    state: LifecycleState::AwaitingRevival,
                    awaiting_decision: Some(RevivalDecision::Fortune { chance: 1.0 }),
                    spawn_anchor: None,
                    fortune_remaining: 1,
                    ..Default::default()
                },
            ))
            .id();

        app.world_mut().send_event(RevivalActionIntent {
            entity: with_shrine,
            action: RevivalActionKind::Reincarnate,
            issued_at_tick: 42,
        });
        app.world_mut().send_event(RevivalActionIntent {
            entity: without_shrine,
            action: RevivalActionKind::Reincarnate,
            issued_at_tick: 42,
        });
        app.update();

        let with_shrine_pos = app
            .world()
            .entity(with_shrine)
            .get::<Position>()
            .expect("position should exist")
            .get();
        assert_eq!(with_shrine_pos, Position::new(shrine_anchor).get());

        let without_shrine_pos = app
            .world()
            .entity(without_shrine)
            .get::<Position>()
            .expect("position should exist")
            .get();
        assert_eq!(
            without_shrine_pos,
            Position::new(crate::player::spawn_position()).get()
        );

        let _ = fs::remove_dir_all(root);
    }
}
