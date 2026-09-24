use std::time::{SystemTime, UNIX_EPOCH};

use valence::prelude::{
    bevy_ecs, bevy_ecs::system::SystemParam, Added, Client, Commands, Entity, EventReader,
    EventWriter, Events, GameMode, Position, Query, Res, ResMut, Username, With, Without,
};

use crate::alchemy::LearnedRecipes;
use crate::combat::anticheat::AntiCheatCounter;
use crate::combat::status::health_regen_boost_multiplier;
use crate::combat::CombatClock;
use crate::cultivation::components::{
    ActorQiIdentity, ActorQiKind, Contamination, Cultivation, CultivationQiInit, MeridianSystem,
    Realm,
};
use crate::cultivation::death_hooks::{
    apply_revive_penalty, CultivationDeathCause, CultivationDeathTrigger, PlayerRevived,
    PlayerTerminated,
};
use crate::cultivation::known_techniques::{KnownTechniques, TechniqueRegistry};
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::cultivation::lifespan::{
    calculate_rebirth_chance, lifespan_tick_rate_multiplier, tribulation_rebirth_chance,
    DeathRegistry, LifespanCapTable, LifespanComponent, LifespanEventEmitted, RebirthChanceInput,
    ZoneDeathKind,
};
use crate::cultivation::tribulation::AscensionQuotaOpened;
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
    persist_death_transition, persist_revival_qi_transaction, persist_termination_transition,
    persist_termination_transition_with_death_context, LifespanEventRecord, PersistenceSettings,
};
use crate::player::state::{save_player_slices, PlayerState, PlayerStatePersistence};
use crate::qi_physics::{QiTransfer, QiTransferReason, WorldQiAccount};
use crate::schema::cultivation::realm_to_string;
use crate::schema::death_cinematic::DeathCinematicS2cV1;
use crate::schema::death_insight::{
    DeathInsightCategoryV1, DeathInsightPositionV1, DeathInsightRequestV1, DeathInsightZoneKindV1,
};
use crate::schema::server_data::{DeathScreenStageV1, DeathScreenZoneKindV1, LifespanPreviewV1};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use crate::schema::spirit_eye::DeathInsightSpiritEyeV1;
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::skill::components::SkillSet;
use crate::skin::NpcVisualProfile;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::spirit_eye::SpiritEyeRegistry;
use crate::world::zone::ZoneRegistry;

use super::components::{
    ActiveCombatWindow, CombatState, DerivedAttrs, Lifecycle, LifecycleState, QuickSlotBindings,
    RevivalDecision, ShieldDrainOverride, SkillBarBindings, Stamina, StaminaState, StatusEffects,
    UnlockedStyles, Wounds, ATTACK_STAMINA_COST, BLEED_TICK_INTERVAL_TICKS,
    COMBAT_STATE_TICK_INTERVAL_TICKS, HEALTH_REGEN_TICK_INTERVAL_TICKS, REVIVE_HEALTH_FRACTION,
    STAMINA_TICK_INTERVAL_TICKS, TICKS_PER_SECOND,
};
use super::events::{
    CombatEvent, DeathCinematicPublished, DeathEvent, DeathInsightRequested, RevivalActionIntent,
    RevivalActionKind,
};

const COMBAT_DRAIN_PER_SEC: f32 = 5.0;
pub const REVIVAL_ROLL_TICKS: u64 = 64;
const JOG_DRAIN_PER_SEC: f32 = 2.0;
const SPRINT_DRAIN_PER_SEC: f32 = 10.0;
/// plan-shield-block-v1 P2 — 举盾持续每秒体力消耗（量级：COMBAT=5.0，JOG=2.0，盾=3.0）。
/// 不触 qi_physics ledger（体力非真元）。P4 按熟练度 1→2 不在本阶段。
pub const SHIELD_DRAIN_PER_SEC: f32 = 3.0;
const EXHAUSTED_RECOVER_RATIO: f32 = 0.5;
const EXHAUSTED_EXIT_FRACTION: f32 = 0.3;
const DEATH_INSIGHT_RECENT_BIO_N: usize = 16;
pub const BASE_HEALTH_REGEN_PER_SEC: f32 = 0.5;

type RevivalQueryItem<'a> = (
    Entity,
    &'a mut Lifecycle,
    Option<&'a mut Wounds>,
    Option<&'a mut Stamina>,
    Option<&'a mut CombatState>,
);

type HealthRegenQueryItem<'a> = (
    &'a mut Wounds,
    Option<&'a Lifecycle>,
    Option<&'a DerivedAttrs>,
    Option<&'a StatusEffects>,
);

type DeathArbiterQueryItem<'a> = (
    &'a mut Lifecycle,
    Option<&'a mut Wounds>,
    Option<&'a mut StatusEffects>,
    Option<&'a mut LifeRecord>,
    Option<&'a Cultivation>,
    Option<&'a PlayerState>,
    Option<&'a mut DeathRegistry>,
    Option<&'a mut LifespanComponent>,
    Option<&'a Position>,
    Option<&'a NpcVisualProfile>,
    Option<&'a NpcMarker>,
    Option<&'a crate::npc::lifecycle::NpcArchetype>,
    Option<&'a crate::npc::patrol::NpcPatrol>,
);

type RevivalPersistenceQueryItem<'a> = (
    RevivalQueryItem<'a>,
    Option<&'a mut Cultivation>,
    Option<&'a mut MeridianSystem>,
    Option<&'a mut Contamination>,
    Option<&'a mut LifeRecord>,
    Option<&'a mut DeathRegistry>,
    Option<&'a mut LifespanComponent>,
    Option<&'a mut PlayerState>,
    Option<&'a mut Position>,
    Option<&'a CurrentDimension>,
    Option<&'a Username>,
    Option<&'a NpcMarker>,
    Option<&'a NpcVisualProfile>,
    Option<&'a mut PlayerInventory>,
    Option<&'a mut SkillSet>,
);

struct DeathScreenContext<'a> {
    lifecycle: &'a Lifecycle,
    death_registry: Option<&'a DeathRegistry>,
    lifespan: Option<&'a LifespanComponent>,
    position: Option<&'a Position>,
    zones: Option<&'a ZoneRegistry>,
    final_words: Vec<String>,
    cinematic: Option<DeathCinematicS2cV1>,
}

pub fn sync_combat_state_from_events(
    mut events: EventReader<CombatEvent>,
    mut commands: Commands,
    mut actors: Query<(&mut CombatState, &mut Stamina)>,
) {
    for event in events.read() {
        if let Ok((mut state, mut stamina)) = actors.get_mut(event.attacker) {
            state.refresh_combat_window(event.resolved_at_tick);
            commands.entity(event.attacker).insert(ActiveCombatWindow);
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
            commands.entity(event.target).insert(ActiveCombatWindow);
            // 举盾态与精疲状态不被战斗事件覆盖（让 stamina_tick 维护其 drain/drain-零 逻辑）。
            if !matches!(
                stamina.state,
                StaminaState::Exhausted | StaminaState::ShieldBlocking
            ) {
                stamina.state = StaminaState::Combat;
            }
        }
    }
}

pub fn wound_bleed_tick(
    clock: Res<CombatClock>,
    mut deaths: EventWriter<DeathEvent>,
    game_modes: Query<&GameMode>,
    mut wounded: Query<(Entity, &mut Wounds, Option<&Lifecycle>)>,
) {
    if !clock.tick.is_multiple_of(BLEED_TICK_INTERVAL_TICKS) {
        return;
    }

    for (entity, mut wounds, lifecycle) in &mut wounded {
        if wounds.health_current <= 0.0 {
            continue;
        }
        if !super::is_damageable(entity, &game_modes) {
            continue;
        }
        if lifecycle.is_some_and(|lifecycle| {
            matches!(
                lifecycle.state,
                LifecycleState::AwaitingRevival | LifecycleState::Terminated
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

pub fn health_regen_tick(clock: Res<CombatClock>, mut wounded: Query<HealthRegenQueryItem<'_>>) {
    if !clock.tick.is_multiple_of(HEALTH_REGEN_TICK_INTERVAL_TICKS) {
        return;
    }

    let dt = HEALTH_REGEN_TICK_INTERVAL_TICKS as f32 / TICKS_PER_SECOND as f32;
    for (mut wounds, lifecycle, derived_attrs, status_effects) in &mut wounded {
        if !can_health_regen(lifecycle, &wounds) {
            continue;
        }

        let derived_multiplier = derived_attrs
            .map(|attrs| attrs.healing_rate_multiplier.max(0.0) as f32)
            .unwrap_or(1.0);
        let status_multiplier = status_effects
            .map(health_regen_boost_multiplier)
            .unwrap_or(1.0);
        let regen = BASE_HEALTH_REGEN_PER_SEC * derived_multiplier * status_multiplier * dt;
        if regen <= f32::EPSILON {
            continue;
        }

        wounds.health_current = (wounds.health_current + regen).clamp(0.0, wounds.health_max);
    }
}

fn can_health_regen(lifecycle: Option<&Lifecycle>, wounds: &Wounds) -> bool {
    if wounds.health_max <= 0.0
        || wounds.health_current <= 0.0
        || wounds.health_current >= wounds.health_max
        || has_active_bleeding(wounds)
    {
        return false;
    }

    !lifecycle.is_some_and(|lifecycle| {
        matches!(
            lifecycle.state,
            LifecycleState::AwaitingRevival | LifecycleState::Terminated
        )
    })
}

fn has_active_bleeding(wounds: &Wounds) -> bool {
    wounds
        .entries
        .iter()
        .any(|entry| entry.bleeding_per_sec > 0.0)
}

pub fn stamina_tick(
    clock: Res<CombatClock>,
    mut stamina_q: Query<(&mut Stamina, Option<&ShieldDrainOverride>)>,
) {
    if !clock.tick.is_multiple_of(STAMINA_TICK_INTERVAL_TICKS) {
        return;
    }

    let dt = STAMINA_TICK_INTERVAL_TICKS as f32 / TICKS_PER_SECOND as f32;
    for (mut stamina, shield_drain_override) in &mut stamina_q {
        stamina.max = stamina.max.max(1.0);
        stamina.recover_per_sec = stamina.recover_per_sec.max(0.0);

        let delta_per_sec = match stamina.state {
            StaminaState::Idle | StaminaState::Walking => stamina.recover_per_sec,
            StaminaState::Jogging => stamina.recover_per_sec - JOG_DRAIN_PER_SEC,
            StaminaState::Sprinting => -SPRINT_DRAIN_PER_SEC,
            StaminaState::Combat => -COMBAT_DRAIN_PER_SEC,
            StaminaState::Exhausted => stamina.recover_per_sec * EXHAUSTED_RECOVER_RATIO,
            // plan-shield-block-v1 P2/P4 — 举盾持续 drain。
            // P4: ShieldDrainOverride component 携带 shield_block_profile 按熟练度缩放的 drain_per_s
            //   （2.0..3.0），覆写常量 SHIELD_DRAIN_PER_SEC（P2 fallback）。
            // 体力归零时由 `force_lower_shield_on_stamina_exhausted` 负责强制放盾 + 施加 ParryRecovery。
            StaminaState::ShieldBlocking => {
                let drain = shield_drain_override
                    .map(|o| o.drain_per_s)
                    .unwrap_or(SHIELD_DRAIN_PER_SEC);
                -drain
            }
        };

        stamina.current = (stamina.current + delta_per_sec * dt).clamp(0.0, stamina.max);

        if stamina.current <= 0.0
            && matches!(
                stamina.state,
                StaminaState::Sprinting | StaminaState::Combat | StaminaState::ShieldBlocking
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

pub fn combat_state_tick(clock: Res<CombatClock>, mut state_q: Query<&mut CombatState>) {
    if !clock.tick.is_multiple_of(COMBAT_STATE_TICK_INTERVAL_TICKS) {
        return;
    }

    for mut state in &mut state_q {
        // 防御窗口不要求截止 tick 的 HUD 同步，继续按低频节拍清理。
        if let Some(window) = state.incoming_window.as_ref() {
            if clock.tick >= window.expires_at_tick() {
                state.incoming_window = None;
            }
        }
    }
}

/// 精确清理活跃战斗窗口，触发 `Changed<CombatState>` 让 HUD 在截止 tick 发送脱战态。
pub fn combat_window_expiry_tick(
    clock: Res<CombatClock>,
    mut commands: Commands,
    mut state_q: Query<(Entity, &mut CombatState, Option<&mut Stamina>), With<ActiveCombatWindow>>,
) {
    for (entity, mut state, stamina) in &mut state_q {
        let Some(until_tick) = state.in_combat_until_tick else {
            // 复活等其它生命周期路径可能先清除窗口；同步移除标记避免无效轮询。
            commands.entity(entity).remove::<ActiveCombatWindow>();
            continue;
        };
        if clock.tick < until_tick {
            continue;
        }

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
        commands.entity(entity).remove::<ActiveCombatWindow>();
    }
}

#[allow(clippy::too_many_arguments)]
pub fn death_arbiter_tick(
    clock: Res<CombatClock>,
    persistence: Res<PersistenceSettings>,
    zones: Option<Res<ZoneRegistry>>,
    spirit_eyes: Option<Res<SpiritEyeRegistry>>,
    mut commands: valence::prelude::Commands,
    mut death_events: EventReader<DeathEvent>,
    mut cultivation_deaths: EventReader<CultivationDeathTrigger>,
    mut death_insights: Option<ResMut<Events<DeathInsightRequested>>>,
    mut terminated: EventWriter<PlayerTerminated>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    mut lifespan_events: Option<ResMut<Events<LifespanEventEmitted>>>,
    mut lifecycle_q: Query<DeathArbiterQueryItem<'_>>,
    mut clients: Query<&mut Client>,
    mut death_cinematics: Option<ResMut<Events<DeathCinematicPublished>>>,
) {
    for event in death_events.read() {
        let Ok((
            mut lifecycle,
            wounds,
            status_effects,
            life_record,
            cultivation,
            player_state,
            mut death_registry,
            mut lifespan,
            position,
            npc_visual_profile,
            npc_marker,
            _npc_archetype,
            _npc_patrol,
        )) = lifecycle_q.get_mut(event.target)
        else {
            continue;
        };

        if npc_marker.is_some() {
            let now_tick = event.at_tick.max(clock.tick);
            let death_zone =
                death_zone_from_context(event.cause.as_str(), position, zones.as_deref());
            let Some(life_record) = life_record.as_deref() else {
                tracing::warn!(
                    target = ?event.target,
                    "[bong][combat] retained NPC death because canonical LifeRecord is missing"
                );
                continue;
            };
            let (Some(death_registry), Some(lifespan)) =
                (death_registry.as_deref(), lifespan.as_deref())
            else {
                tracing::warn!(
                    target = ?event.target,
                    "[bong][combat] retained NPC death because terminal owner components are missing"
                );
                continue;
            };
            let Ok(actor_qi_identity) =
                ActorQiIdentity::from_life_record(life_record, ActorQiKind::Npc)
            else {
                tracing::warn!(
                    target = ?event.target,
                    "[bong][combat] retained NPC death because terminal identity diverged"
                );
                continue;
            };
            if lifecycle.character_id != life_record.character_id
                || death_registry.char_id != life_record.character_id
            {
                tracing::warn!(
                    target = ?event.target,
                    "[bong][combat] retained NPC death because terminal identity diverged"
                );
                continue;
            }
            let mut staged_registry = death_registry.clone();
            staged_registry.record_death(now_tick, death_zone);
            let insight_payload = build_death_insight_request(DeathInsightBuildInput {
                lifecycle: &lifecycle,
                life_record: Some(life_record),
                cultivation,
                death_registry: Some(&staged_registry),
                lifespan: Some(lifespan),
                position,
                at_tick: now_tick,
                cause: event.cause.as_str(),
                category: DeathInsightCategoryV1::Combat,
                zone_kind: death_zone,
                rebirth_chance: None,
                will_terminate: true,
                known_spirit_eyes: known_spirit_eyes_for_death_insight(
                    Some(life_record),
                    &lifecycle,
                    spirit_eyes.as_deref(),
                ),
            });
            commands
                .entity(event.target)
                .insert(crate::npc::lifecycle::PendingNpcTermination {
                    cause: event.cause.clone(),
                    at_tick: now_tick,
                    death_zone,
                    lifespan_event: death_penalty_lifespan_event(
                        cultivation,
                        now_tick,
                        event.cause.as_str(),
                    ),
                    death_insight: Some(insight_payload),
                    reason: crate::npc::lifecycle::NpcDeathReason::Combat,
                    attacker: event.attacker,
                    attacker_player_id: event.attacker_player_id.clone(),
                    authorize_loot: true,
                    actor_qi_identity,
                    reproduction: None,
                });
            continue;
        }

        // plan-race-system-v1 P4（决议 §6）—— 死亡三条解除易形触发路径之一：死亡即刻
        // 解除易形（移除 `MorphState` + 重扫装备门，见 `body_plan::morph::
        // release_morph_state`）。本系统是 `Query` 而非原始 `World`，无法直接调用
        // 需要 `&mut World` 的 `release_morph_state`，故走 `commands.add` 排入
        // deferred command——**不是**立即生效，而是在本次 `Update` 调度末尾
        // `apply_deferred` 时才真正执行；下游掉落/复活链路只要排在这次调度的
        // command flush 之后（同一 tick 内），就能看到"已恢复本体"的状态。
        {
            let target = event.target;
            commands.add(
                move |world: &mut valence::prelude::bevy_ecs::world::World| {
                    crate::body_plan::morph::release_morph_state(world, target);
                },
            );
        }

        // Worldview §十二：死亡掉落应落在死亡点。
        if let Some(position) = position {
            let p = position.get();
            commands.entity(event.target).insert(DeathDropAnchor {
                pos: [p.x, p.y, p.z],
            });
        }
        // 等待裁决或已终结时拒绝重入，避免重复计数、扣寿或重置选择窗口。
        if matches!(
            lifecycle.state,
            LifecycleState::AwaitingRevival | LifecycleState::Terminated
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
            will_terminate: revival_decision.is_none(),
            known_spirit_eyes: known_spirit_eyes_for_death_insight(
                life_record.as_deref(),
                &lifecycle,
                spirit_eyes.as_deref(),
            ),
        });

        if revival_decision.is_none() {
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
                npc_marker.is_some(),
                npc_visual_profile,
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
        let decision = revival_decision.expect("terminal deaths handled above");
        let mut staged_lifecycle = lifecycle.clone();
        staged_lifecycle.await_revival_decision(decision, now_tick);
        if let Some(mut life_record) = life_record {
            life_record.push(BiographyEntry::Death {
                cause: event.cause.clone(),
                tick: now_tick,
            });
            if let Err(error) = persist_death_transition(
                &persistence,
                &staged_lifecycle,
                &life_record,
                event.cause.as_str(),
                lifespan_event.as_ref(),
            ) {
                tracing::warn!(
                    "[bong][persistence] failed to persist death transition for {}: {error}",
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
        *lifecycle = staged_lifecycle;
        clear_death_combat_state(wounds, status_effects);
        publish_revival_decision(
            &mut commands,
            &mut clients,
            death_cinematics.as_deref_mut(),
            event.target,
            event.cause.as_str(),
            decision,
            DeathScreenContext {
                lifecycle: &lifecycle,
                death_registry: death_registry.as_deref(),
                lifespan: lifespan.as_deref(),
                position,
                zones: zones.as_deref(),
                final_words: Vec::new(),
                cinematic: None,
            },
            now_tick,
        );
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
            status_effects,
            life_record,
            cultivation,
            player_state,
            mut death_registry,
            mut lifespan,
            position,
            npc_visual_profile,
            npc_marker,
            npc_archetype,
            npc_patrol,
        )) = lifecycle_q.get_mut(event.entity)
        else {
            continue;
        };

        if npc_marker.is_some() {
            let cause = format!("cultivation:{:?}", event.cause);
            let death_zone = match event.cause {
                CultivationDeathCause::NegativeZoneDrain => ZoneDeathKind::Negative,
                CultivationDeathCause::SwarmQiDrain => ZoneDeathKind::Ordinary,
                _ => death_zone_from_context(cause.as_str(), position, zones.as_deref()),
            };
            let Some(life_record) = life_record.as_deref() else {
                tracing::warn!(
                    target = ?event.entity,
                    "[bong][combat] retained NPC cultivation death without canonical LifeRecord"
                );
                continue;
            };
            let Some(death_registry) = death_registry.as_deref() else {
                tracing::warn!(
                    target = ?event.entity,
                    "[bong][combat] retained NPC cultivation death without DeathRegistry"
                );
                continue;
            };
            let Ok(actor_qi_identity) =
                ActorQiIdentity::from_life_record(life_record, ActorQiKind::Npc)
            else {
                tracing::warn!(
                    target = ?event.entity,
                    "[bong][combat] retained NPC cultivation death after identity mismatch"
                );
                continue;
            };
            if lifecycle.character_id != life_record.character_id
                || death_registry.char_id != life_record.character_id
            {
                tracing::warn!(
                    target = ?event.entity,
                    "[bong][combat] retained NPC cultivation death after identity mismatch"
                );
                continue;
            }
            let mut staged_registry = death_registry.clone();
            staged_registry.record_death(clock.tick, death_zone);
            let insight_payload = build_death_insight_request(DeathInsightBuildInput {
                lifecycle: &lifecycle,
                life_record: Some(life_record),
                cultivation,
                death_registry: Some(&staged_registry),
                lifespan: lifespan.as_deref(),
                position,
                at_tick: clock.tick,
                cause: cause.as_str(),
                category: death_insight_category_from_cultivation_cause(event.cause),
                zone_kind: death_zone,
                rebirth_chance: None,
                will_terminate: true,
                known_spirit_eyes: known_spirit_eyes_for_death_insight(
                    Some(life_record),
                    &lifecycle,
                    spirit_eyes.as_deref(),
                ),
            });
            commands
                .entity(event.entity)
                .insert(crate::npc::lifecycle::PendingNpcTermination {
                    cause,
                    at_tick: clock.tick,
                    death_zone,
                    lifespan_event: if event.cause == CultivationDeathCause::NaturalAging
                        || event.cause == CultivationDeathCause::VoidQuotaExceeded
                        || event.cause == CultivationDeathCause::VoidActionBacklash
                    {
                        None
                    } else {
                        death_penalty_lifespan_event(cultivation, clock.tick, "cultivation_death")
                    },
                    death_insight: Some(insight_payload),
                    reason: if event.cause == CultivationDeathCause::NaturalAging {
                        crate::npc::lifecycle::NpcDeathReason::NaturalAging
                    } else {
                        crate::npc::lifecycle::NpcDeathReason::Combat
                    },
                    attacker: None,
                    attacker_player_id: None,
                    authorize_loot: event.cause != CultivationDeathCause::NaturalAging,
                    actor_qi_identity,
                    reproduction: if event.cause == CultivationDeathCause::NaturalAging {
                        crate::npc::lifecycle::natural_aging_reproduction_request(
                            npc_archetype,
                            position,
                            npc_patrol,
                        )
                    } else {
                        None
                    },
                });
            continue;
        }

        // Worldview §十二：死亡掉落应落在死亡点。
        if let Some(position) = position {
            let p = position.get();
            commands.entity(event.entity).insert(DeathDropAnchor {
                pos: [p.x, p.y, p.z],
            });
        }
        // 同上：AwaitingRevival 期间不接受新的 cultivation 死亡事件重入。
        if matches!(
            lifecycle.state,
            LifecycleState::AwaitingRevival | LifecycleState::Terminated
        ) {
            continue;
        }
        let cause = format!("cultivation:{:?}", event.cause);
        let death_zone = match event.cause {
            CultivationDeathCause::NegativeZoneDrain => ZoneDeathKind::Negative,
            CultivationDeathCause::SwarmQiDrain => ZoneDeathKind::Ordinary,
            _ => death_zone_from_context(cause.as_str(), position, zones.as_deref()),
        };
        if let Some(registry) = death_registry.as_deref_mut() {
            registry.record_death(clock.tick, death_zone);
        }
        let void_quota_exceeded = event.cause == CultivationDeathCause::VoidQuotaExceeded;
        let void_action_backlash = event.cause == CultivationDeathCause::VoidActionBacklash;
        let lifespan_exhausted = if event.cause == CultivationDeathCause::NaturalAging {
            apply_natural_aging_lifespan_exhaustion(
                cultivation,
                lifespan.as_deref_mut(),
                player_state,
            );
            true
        } else if void_quota_exceeded || void_action_backlash {
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
            will_terminate: revival_decision.is_none(),
            known_spirit_eyes: known_spirit_eyes_for_death_insight(
                life_record.as_deref(),
                &lifecycle,
                spirit_eyes.as_deref(),
            ),
        });

        if revival_decision.is_none() {
            let lifespan_event = if event.cause == CultivationDeathCause::NaturalAging
                || void_quota_exceeded
                || void_action_backlash
            {
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
                npc_marker.is_some(),
                npc_visual_profile,
                &mut vfx_events,
                if void_quota_exceeded {
                    crate::cultivation::tribulation::VOID_QUOTA_EXCEEDED_REASON
                } else if void_action_backlash {
                    "void_action_backlash"
                } else {
                    "natural_end"
                },
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
        let decision = revival_decision.expect("terminal deaths handled above");
        let mut staged_lifecycle = lifecycle.clone();
        staged_lifecycle.await_revival_decision(decision, clock.tick);
        if let Some(mut life_record) = life_record {
            life_record.push(BiographyEntry::Death {
                cause: cause.clone(),
                tick: clock.tick,
            });
            if let Err(error) = persist_death_transition(
                &persistence,
                &staged_lifecycle,
                &life_record,
                cause.as_str(),
                lifespan_event.as_ref(),
            ) {
                tracing::warn!(
                    "[bong][persistence] failed to persist cultivation death transition for {}: {error}",
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
        *lifecycle = staged_lifecycle;
        clear_death_combat_state(wounds, status_effects);
        publish_revival_decision(
            &mut commands,
            &mut clients,
            death_cinematics.as_deref_mut(),
            event.entity,
            cause.as_str(),
            decision,
            DeathScreenContext {
                lifecycle: &lifecycle,
                death_registry: death_registry.as_deref(),
                lifespan: lifespan.as_deref(),
                position,
                zones: zones.as_deref(),
                final_words: Vec::new(),
                cinematic: None,
            },
            clock.tick,
        );
        if let Some(death_insights) = death_insights.as_deref_mut() {
            death_insights.send(DeathInsightRequested {
                payload: insight_payload,
            });
        }
    }
}

pub fn clear_expired_revival_weakness(clock: Res<CombatClock>, mut actors: Query<&mut Lifecycle>) {
    for mut lifecycle in &mut actors {
        if lifecycle
            .weakened_until_tick
            .is_some_and(|until| clock.tick >= until)
        {
            lifecycle.weakened_until_tick = None;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn publish_revival_decision(
    commands: &mut Commands,
    clients: &mut Query<&mut Client>,
    death_cinematics: Option<&mut Events<DeathCinematicPublished>>,
    entity: Entity,
    cause: &str,
    decision: RevivalDecision,
    mut context: DeathScreenContext<'_>,
    now_tick: u64,
) {
    let death_zone = death_zone_from_context(cause, context.position, context.zones);
    context.final_words = vec![default_final_words(cause, death_zone)];
    let cinematic = crate::death_lifecycle::cinematic::build_death_cinematic(
        context.lifecycle,
        context.death_registry,
        Some(decision),
        death_zone,
        cause,
        context.final_words.clone(),
        now_tick,
    );
    let payload = cinematic.snapshot(now_tick);
    commands.entity(entity).insert(cinematic);
    if let Some(events) = death_cinematics {
        events.send(DeathCinematicPublished {
            payload: payload.clone(),
        });
    }
    context.cinematic = Some(payload);
    let deadline = context
        .lifecycle
        .revival_decision_deadline_tick
        .expect("revival decision must have a deadline");
    emit_death_screen(
        clients, entity, cause, decision, context, now_tick, deadline,
    );
    hide_terminate_screen(clients, entity);
}

type ReconnectedAwaitingRevivalQueryItem<'a> = (
    Entity,
    &'a Lifecycle,
    &'a mut Client,
    Option<&'a LifeRecord>,
    Option<&'a DeathRegistry>,
    Option<&'a LifespanComponent>,
    Option<&'a Position>,
);

/// 重连时补发尚未完成的复活裁决及演出，保留原有选择窗口。
/// Client 访问放在同一查询内，避免 Added<Client> 与另一个可变查询冲突。
#[allow(clippy::too_many_arguments)]
pub fn reemit_death_screen_for_reconnected_awaiting_revival_clients(
    clock: Res<CombatClock>,
    zones: Option<Res<ZoneRegistry>>,
    mut commands: Commands,
    mut death_cinematics: ResMut<Events<DeathCinematicPublished>>,
    mut reconnected: Query<
        ReconnectedAwaitingRevivalQueryItem<'_>,
        (
            Added<Client>,
            Without<crate::death_lifecycle::cinematic::DeathCinematic>,
        ),
    >,
) {
    for (entity, lifecycle, mut client, life_record, death_registry, lifespan, position) in
        &mut reconnected
    {
        if lifecycle.state != LifecycleState::AwaitingRevival {
            continue;
        }
        let Some(decision) = lifecycle.awaiting_decision else {
            // 状态机内部不一致（AwaitingRevival 却没有待决策项）——没有决策可展示，跳过而不
            // panic；缺少裁决的异常存档不在此处猜测恢复。
            continue;
        };

        let decision_deadline_tick = lifecycle
            .revival_decision_deadline_tick
            .unwrap_or(clock.tick);
        let cause = eventual_cause(life_record);
        let death_zone = death_zone_from_context(cause.as_str(), position, zones.as_deref());
        let final_words = vec![default_final_words(cause.as_str(), death_zone)];
        let cinematic = crate::death_lifecycle::cinematic::build_death_cinematic(
            lifecycle,
            death_registry,
            Some(decision),
            death_zone,
            cause.as_str(),
            final_words.clone(),
            clock.tick,
        );
        commands.entity(entity).insert(cinematic.clone());
        let cinematic_payload = cinematic.snapshot(clock.tick);
        death_cinematics.send(DeathCinematicPublished {
            payload: cinematic_payload.clone(),
        });

        let payload = build_death_screen_payload(
            cause.as_str(),
            decision,
            DeathScreenContext {
                lifecycle,
                death_registry,
                lifespan,
                position,
                zones: zones.as_deref(),
                final_words,
                cinematic: Some(cinematic_payload),
            },
            clock.tick,
            decision_deadline_tick,
        );
        let Ok(payload_bytes) = serialize_server_data_payload(&payload) else {
            continue;
        };
        send_server_data_payload(&mut client, payload_bytes.as_slice());
        tracing::info!(
            "[bong][network] sent {} {} payload to reconnected client entity {entity:?} \
             (re-emitted AwaitingRevival death screen)",
            SERVER_DATA_CHANNEL,
            payload_type_label(payload.payload_type()),
        );
    }
}

#[derive(SystemParam)]
pub struct RevivalQiResources<'w> {
    ledger: ResMut<'w, WorldQiAccount>,
    zones: Option<ResMut<'w, ZoneRegistry>>,
}

#[derive(SystemParam)]
pub struct RevivalEventWriters<'w> {
    revived: EventWriter<'w, PlayerRevived>,
    terminated: EventWriter<'w, PlayerTerminated>,
    quota_opened: EventWriter<'w, AscensionQuotaOpened>,
    qi_transfers: EventWriter<'w, QiTransfer>,
    vfx_events: EventWriter<'w, VfxEventRequest>,
    coffin_state_events: EventWriter<'w, crate::coffin::CoffinStateChanged>,
}

#[allow(clippy::too_many_arguments)]
pub fn handle_revival_action_intents(
    clock: Res<CombatClock>,
    persistence: Res<PersistenceSettings>,
    player_persistence: Option<Res<PlayerStatePersistence>>,
    default_loadout: Option<Res<DefaultLoadout>>,
    item_registry: Option<Res<crate::inventory::ItemRegistry>>,
    mut inventory_allocator: Option<ResMut<InventoryInstanceIdAllocator>>,
    technique_registry: Option<Res<TechniqueRegistry>>,
    mut intents: EventReader<RevivalActionIntent>,
    mut qi: RevivalQiResources,
    mut events: RevivalEventWriters,
    mut commands: valence::prelude::Commands,
    mut lifecycle_q: Query<RevivalPersistenceQueryItem<'_>>,
    mut clients: Query<&mut valence::prelude::Client>,
    // P0 fix: coffin 清除参数（复活/新建时彻底清除 coffin 状态）
    mut coffin_registry: Option<ResMut<crate::coffin::CoffinRegistry>>,
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
            current_dimension,
            username,
            npc_marker,
            npc_visual_profile,
            inventory,
            skill_set,
        )) = lifecycle_q.get_mut(intent.entity)
        else {
            continue;
        };

        match intent.action {
            RevivalActionKind::RollRebirth => {
                if npc_marker.is_some()
                    || lifecycle.state != LifecycleState::AwaitingRevival
                    || lifecycle.revival_roll_survived.is_some()
                {
                    continue;
                }
                let Some(decision) = lifecycle.awaiting_decision else {
                    continue;
                };
                let survived = matches!(decision, RevivalDecision::Fortune { .. })
                    || matches!(decision, RevivalDecision::Tribulation { chance } if roll_rebirth(clock.tick, entity, chance));
                let mut staged = lifecycle.clone();
                staged.revival_roll_survived = Some(survived);
                staged.revival_decision_deadline_tick =
                    Some(clock.tick.saturating_add(REVIVAL_ROLL_TICKS));
                if let (Some(storage), Some(username)) = (player_persistence.as_deref(), username) {
                    if let Err(error) = crate::player::state::save_player_lifecycle_slice(
                        storage,
                        &username.0,
                        &staged,
                        clock.tick,
                    ) {
                        tracing::warn!("[bong][combat] cannot persist revival roll: {error}");
                        continue;
                    }
                }
                *lifecycle = staged;
                let context = DeathScreenContext {
                    lifecycle: &lifecycle,
                    death_registry: death_registry.as_deref(),
                    lifespan: lifespan.as_deref(),
                    position: position.as_deref(),
                    zones: qi.zones.as_deref(),
                    final_words: Vec::new(),
                    cinematic: None,
                };
                let payload = build_death_screen_payload(
                    &eventual_cause(life_record.as_deref()),
                    decision,
                    context,
                    clock.tick,
                    lifecycle.revival_decision_deadline_tick.unwrap(),
                );
                send_payload(&mut clients, entity, payload);
            }
            RevivalActionKind::Reincarnate => {
                if npc_marker.is_some() {
                    tracing::warn!(
                        "[bong][combat] reject player reincarnation intent for NPC {:?}",
                        entity,
                    );
                    continue;
                }
                if lifecycle.state != LifecycleState::AwaitingRevival {
                    continue;
                }
                if lifecycle.revival_roll_survived.is_some()
                    && lifecycle
                        .revival_decision_deadline_tick
                        .is_some_and(|deadline| clock.tick < deadline)
                {
                    continue;
                }
                let Some(decision) = lifecycle.awaiting_decision else {
                    continue;
                };

                let survived = lifecycle.revival_roll_survived.unwrap_or_else(||
                    matches!(decision, RevivalDecision::Fortune { .. })
                    || matches!(decision, RevivalDecision::Tribulation { chance } if roll_rebirth(clock.tick, entity, chance)));

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
                        current_dimension,
                        &mut qi.ledger,
                        qi.zones.as_deref_mut(),
                        &mut events.revived,
                        &mut events.quota_opened,
                        &mut events.qi_transfers,
                        &mut commands,
                        coffin_registry.as_deref_mut(),
                        &mut events.coffin_state_events,
                        username,
                        lifespan.as_deref(),
                        player_persistence.as_deref(),
                    ) {
                        commands
                            .entity(entity)
                            .remove::<crate::death_lifecycle::cinematic::DeathCinematic>();
                        hide_death_screen(&mut clients, entity);
                        hide_terminate_screen(&mut clients, entity);
                    }
                } else if terminate_lifecycle(
                    entity,
                    &mut lifecycle,
                    life_record,
                    &persistence,
                    clock.tick,
                    &mut events.terminated,
                    position.as_deref(),
                    npc_marker.is_some(),
                    npc_visual_profile,
                    &mut events.vfx_events,
                    "tribulation_failed",
                ) {
                    // 劫数不过 → 形神俱散：清 coffin 状态（四件套），防止 Registry/ECS/SQLite 残留导致重启复钉。
                    clear_coffin_on_exit(
                        entity,
                        &mut commands,
                        coffin_registry.as_deref_mut(),
                        &mut events.coffin_state_events,
                        player_persistence.as_deref(),
                        username,
                        lifespan.as_deref(),
                    );
                    commands
                        .entity(entity)
                        .remove::<crate::death_lifecycle::cinematic::DeathCinematic>();
                    hide_death_screen(&mut clients, entity);
                }
            }
            RevivalActionKind::Terminate => {
                if lifecycle.state != LifecycleState::AwaitingRevival
                    || lifecycle.revival_roll_survived.is_some()
                {
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
                    &mut events.terminated,
                    position.as_deref(),
                    npc_marker.is_some(),
                    npc_visual_profile,
                    &mut events.vfx_events,
                    "voluntary_retire",
                ) {
                    // 主动归隐终结：清 coffin 状态（四件套），防止 Registry/ECS/SQLite 残留导致重启复钉。
                    clear_coffin_on_exit(
                        entity,
                        &mut commands,
                        coffin_registry.as_deref_mut(),
                        &mut events.coffin_state_events,
                        player_persistence.as_deref(),
                        username,
                        lifespan.as_deref(),
                    );
                    commands
                        .entity(entity)
                        .remove::<crate::death_lifecycle::cinematic::DeathCinematic>();
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
                    item_registry.as_deref(),
                    inventory_allocator.as_deref_mut(),
                    technique_registry.as_deref(),
                    coffin_registry.as_deref_mut(),
                    &mut events.coffin_state_events,
                );
                commands
                    .entity(entity)
                    .remove::<crate::death_lifecycle::cinematic::DeathCinematic>();
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
            action: if lifecycle.revival_roll_survived.is_some() {
                RevivalActionKind::Reincarnate
            } else {
                RevivalActionKind::RollRebirth
            },
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
    known_spirit_eyes: Vec<DeathInsightSpiritEyeV1>,
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
        known_spirit_eyes: input.known_spirit_eyes,
        context: serde_json::json!({
            "will_terminate": input.will_terminate,
            "fortune_remaining": input.lifecycle.fortune_remaining,
            "lifecycle_state": format!("{:?}", input.lifecycle.state),
        }),
    }
}

fn known_spirit_eyes_for_death_insight(
    life_record: Option<&LifeRecord>,
    lifecycle: &Lifecycle,
    registry: Option<&SpiritEyeRegistry>,
) -> Vec<DeathInsightSpiritEyeV1> {
    let Some(registry) = registry else {
        return Vec::new();
    };
    let character_id = life_record
        .map(|record| record.character_id.as_str())
        .unwrap_or(lifecycle.character_id.as_str());
    registry.known_spirit_eyes_for(character_id)
}

fn death_insight_category_from_cultivation_cause(
    cause: CultivationDeathCause,
) -> DeathInsightCategoryV1 {
    match cause {
        CultivationDeathCause::NaturalAging => DeathInsightCategoryV1::Natural,
        CultivationDeathCause::BreakthroughBackfire
        | CultivationDeathCause::MeridianCollapse
        | CultivationDeathCause::NegativeZoneDrain
        | CultivationDeathCause::ContaminationOverflow
        | CultivationDeathCause::DevCommand
        | CultivationDeathCause::SwarmQiDrain
        | CultivationDeathCause::VoidQuotaExceeded
        | CultivationDeathCause::VoidActionBacklash => DeathInsightCategoryV1::Cultivation,
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
        LifecycleState::AwaitingRevival | LifecycleState::Terminated
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
    current_dimension: Option<&CurrentDimension>,
    ledger: &mut WorldQiAccount,
    zones: Option<&mut ZoneRegistry>,
    revived: &mut EventWriter<PlayerRevived>,
    quota_opened: &mut EventWriter<AscensionQuotaOpened>,
    qi_transfers: &mut EventWriter<QiTransfer>,
    // P0 fix: coffin 清除参数（复活后不应继续锁棺）
    commands: &mut valence::prelude::Commands,
    coffin_registry: Option<&mut crate::coffin::CoffinRegistry>,
    coffin_state_events: &mut EventWriter<crate::coffin::CoffinStateChanged>,
    username: Option<&Username>,
    coffin_lifespan: Option<&crate::cultivation::lifespan::LifespanComponent>,
    coffin_player_persistence: Option<&PlayerStatePersistence>,
) -> bool {
    let revival_username = match username {
        Some(username) => username.0.as_str(),
        None => {
            tracing::warn!(
                "[bong][combat] revive player identity missing Username; fail closed for {:?}",
                entity,
            );
            return false;
        }
    };
    let mut staged_lifecycle = lifecycle.clone();
    if matches!(
        lifecycle.awaiting_decision,
        Some(RevivalDecision::Fortune { .. })
    ) {
        staged_lifecycle.fortune_remaining = staged_lifecycle.fortune_remaining.saturating_sub(1);
    }
    let weakened_multiplier = damaged_spawn_anchor_weakened_multiplier(lifecycle);
    staged_lifecycle.revive_with_weakened_multiplier(now_tick, weakened_multiplier);

    let mut staged_cultivation = cultivation.as_ref().map(|value| (**value).clone());
    let mut staged_meridians = meridians.as_ref().map(|value| (**value).clone());
    let mut staged_contam = contam.as_ref().map(|value| (**value).clone());
    let mut staged_life_record = life_record.as_ref().map(|value| (**value).clone());
    let mut staged_ledger = ledger.clone();
    let mut staged_zones = zones.as_deref().cloned();
    let staged_owner_state = (
        staged_cultivation.as_mut(),
        staged_meridians.as_mut(),
        staged_contam.as_mut(),
        staged_life_record.as_mut(),
    );
    let (
        committed_transfers,
        release_void_quota,
        persisted_staged_cultivation,
        persisted_staged_meridians,
        persisted_staged_contam,
    ) = match staged_owner_state {
        (
            Some(staged_cultivation),
            Some(staged_meridians),
            Some(staged_contam),
            Some(staged_life_record),
        ) => {
            let prior_realm = staged_cultivation.realm;
            let actor =
                match ActorQiIdentity::from_life_record(staged_life_record, ActorQiKind::Player) {
                    Ok(actor) => actor,
                    Err(error) => {
                        tracing::warn!(
                            ?error,
                            "[bong][combat] revive qi identity failed closed for {:?}",
                            entity,
                        );
                        return false;
                    }
                };
            let zone_name = match (
                position.as_deref(),
                current_dimension,
                staged_zones.as_ref(),
            ) {
                (Some(position), Some(current_dimension), Some(zones)) => zones
                    .find_zone(current_dimension.0, position.0)
                    .map(|zone| zone.name.clone()),
                _ => None,
            };
            let zone = zone_name.as_deref().and_then(|zone_name| {
                staged_zones
                    .as_mut()
                    .and_then(|zones| zones.find_zone_mut(zone_name))
            });
            let release_amount = staged_cultivation.qi_snapshot().current;
            let committed_transfers = match staged_cultivation.release_to_zone(
                zone,
                &mut staged_ledger,
                &actor,
                release_amount,
                QiTransferReason::ReleaseToZone,
            ) {
                Ok(outcome) => outcome.transfers,
                Err(error) => {
                    tracing::warn!(
                        ?error,
                        "[bong][combat] revive qi release failed closed for {:?}",
                        entity,
                    );
                    return false;
                }
            };

            apply_revive_penalty(staged_cultivation, staged_meridians, staged_contam);
            let qi_state = staged_cultivation.qi_snapshot();
            staged_cultivation
                .set_for_init(CultivationQiInit {
                    current: 0.0,
                    max: qi_state.max,
                    frozen: qi_state.frozen,
                })
                .expect("revive penalty staged qi capacity must remain valid");
            staged_life_record.push(BiographyEntry::Rebirth {
                prior_realm,
                new_realm: staged_cultivation.realm,
                tick: now_tick,
            });
            let release_void_quota =
                prior_realm == Realm::Void && staged_cultivation.realm != Realm::Void;
            (
                committed_transfers,
                release_void_quota,
                staged_cultivation.clone(),
                staged_meridians.clone(),
                staged_contam.clone(),
            )
        }
        _ => {
            tracing::warn!(
                "[bong][combat] revive owner bundle incomplete; fail closed for {:?}",
                entity,
            );
            return false;
        }
    };
    let persisted_staged_life_record = staged_life_record
        .as_ref()
        .expect("revival owner-bundle match must leave a staged LifeRecord");
    let quota_release = match persist_revival_qi_transaction(
        persistence,
        revival_username,
        &persisted_staged_cultivation,
        &persisted_staged_meridians,
        &persisted_staged_contam,
        persisted_staged_life_record,
        staged_zones.as_ref(),
        &staged_ledger,
        release_void_quota,
    ) {
        Ok(quota_release) => quota_release,
        Err(error) => {
            tracing::warn!(
                "[bong][persistence] failed to persist atomic revival qi transaction for {:?}: {error}",
                entity,
            );
            return false;
        }
    };
    *ledger = staged_ledger;
    if let (Some(zones), Some(staged_zones)) = (zones, staged_zones) {
        *zones = staged_zones;
    }
    lifecycle.fortune_remaining = staged_lifecycle.fortune_remaining;
    lifecycle.revive_with_weakened_multiplier(now_tick, weakened_multiplier);
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

    // P0 fix: 清 coffin 状态（复活后不应继续锁棺）。统一走 clear_coffin_on_exit 四件套。
    clear_coffin_on_exit(
        entity,
        commands,
        coffin_registry,
        coffin_state_events,
        coffin_player_persistence,
        username,
        coffin_lifespan,
    );

    if let Some(release) = quota_release {
        if release.opened_slot {
            quota_opened.send(AscensionQuotaOpened {
                occupied_slots: release.quota.occupied_slots,
            });
        }
    }
    for transfer in committed_transfers {
        qi_transfers.send(transfer);
    }
    revived.send(PlayerRevived { entity });
    true
}

/// 棺材状态四件套清除：CoffinRegistry + CoffinComponent(ECS) + SQLite(persist_in_coffin) + CoffinStateChanged。
///
/// 用于 revive / terminate / new_char 三条退出路径，确保任何离棺场景都不遗漏。
/// 仅当玩家确实在棺内（registry.clear_player 返回 Some）时才落持久化和事件，避免噪音。
fn clear_coffin_on_exit(
    entity: Entity,
    commands: &mut valence::prelude::Commands,
    coffin_registry: Option<&mut crate::coffin::CoffinRegistry>,
    coffin_state_events: &mut EventWriter<crate::coffin::CoffinStateChanged>,
    player_persistence: Option<&PlayerStatePersistence>,
    username: Option<&Username>,
    lifespan: Option<&crate::cultivation::lifespan::LifespanComponent>,
) {
    let was_in_coffin = if let Some(registry) = coffin_registry {
        registry.clear_player(entity).is_some()
    } else {
        false
    };
    commands
        .entity(entity)
        .remove::<crate::coffin::CoffinComponent>();
    if was_in_coffin {
        crate::coffin::persist_in_coffin(player_persistence, username, lifespan, None);
        coffin_state_events.send(crate::coffin::CoffinStateChanged {
            player: entity,
            grade: None,
        });
    }
}

fn damaged_spawn_anchor_weakened_multiplier(lifecycle: &Lifecycle) -> u64 {
    if lifecycle.spawn_anchor.is_some() && lifecycle.spawn_anchor_damaged {
        2
    } else {
        1
    }
}

pub(crate) fn emit_terminal_vfx(
    position: Option<&Position>,
    is_npc: bool,
    npc_visual_profile: Option<&NpcVisualProfile>,
    vfx_events: &mut EventWriter<VfxEventRequest>,
) {
    let Some(pos) = position else {
        return;
    };
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
    if is_npc {
        vfx_events.send(crate::skin::faction_tint::npc_death_smoke_request(p));
        if let Some(request) =
            crate::skin::faction_tint::npc_death_qi_burst_request(p, npc_visual_profile)
        {
            vfx_events.send(request);
        }
    }
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
    is_npc: bool,
    npc_visual_profile: Option<&NpcVisualProfile>,
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
        is_npc,
        npc_visual_profile,
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
    is_npc: bool,
    npc_visual_profile: Option<&NpcVisualProfile>,
    vfx_events: &mut EventWriter<VfxEventRequest>,
    cause: &str,
    death_registry_cause: Option<&str>,
    lifespan_event: Option<LifespanEventRecord>,
) -> bool {
    let Some(mut life_record) = life_record else {
        if death_registry_cause.is_some()
            && !matches!(lifecycle.state, LifecycleState::AwaitingRevival)
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
        && !matches!(lifecycle.state, LifecycleState::AwaitingRevival);
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

    emit_terminal_vfx(position, is_npc, npc_visual_profile, vfx_events);
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
    item_registry: Option<&crate::inventory::ItemRegistry>,
    inventory_allocator: Option<&mut InventoryInstanceIdAllocator>,
    technique_registry: Option<&TechniqueRegistry>,
    // P0 fix: coffin 清除参数（新建角色不应继承死亡前的棺状态）
    coffin_registry: Option<&mut crate::coffin::CoffinRegistry>,
    coffin_state_events: &mut EventWriter<crate::coffin::CoffinStateChanged>,
) {
    // plan-remains-suite：轮换 char_id + 派生新角色 spec 的逻辑抽到
    // `cultivation::character_select::rotate_to_new_character`，与 join 转世门
    // （`cultivation::attach_cultivation_to_joined_clients`）共用同一份取值，
    // 避免"新角色长什么样"在两处漂移。
    let mut rotated_spec = None;
    if let (Some(username), Some(player_persistence)) = (username, player_persistence) {
        match crate::cultivation::character_select::rotate_to_new_character(
            player_persistence,
            username.0.as_str(),
        ) {
            Ok(bundle) => {
                tracing::info!(
                    "[bong][combat] rotated current_char_id for `{}` to {}",
                    username.0,
                    bundle.next_character_id
                );
                lifecycle.character_id = bundle.next_character_id;
                rotated_spec = Some(bundle.spec);
            }
            Err(error) => {
                tracing::warn!(
                    "[bong][combat] failed to rotate current_char_id for `{}`: {error}",
                    username.0
                );
            }
        }
    }

    // plan-multi-life-v1 §0 O.4: per-life 运数。luck_pool::reset_for_new_life 同时
    // 把 fortune 重置到 INITIAL_FORTUNE_PER_LIFE 并清零 death_count，
    // 与 cultivation::luck_pool 单一数据源保持一致。
    crate::cultivation::luck_pool::reset_for_new_life(lifecycle);
    lifecycle.last_death_tick = None;
    lifecycle.last_revive_tick = Some(now_tick);
    // 新角色与前角色无机制关联；灵龛归属同样不继承。
    lifecycle.spawn_anchor = None;
    lifecycle.awaiting_decision = None;
    lifecycle.revival_decision_deadline_tick = None;
    lifecycle.weakened_until_tick = None;
    lifecycle.state = LifecycleState::Alive;

    if let Some(mut life_record) = life_record {
        *life_record = LifeRecord::new(lifecycle.character_id.clone());
    }

    let default_player_state = PlayerState::default();
    // plan-multi-life-v1 §2 / §3：新角色 spec 由 cultivation::character_select 唯一管理
    // （spawn_pos = spawn_plain，realm = Awaken，lifespan = AWAKEN cap）。这里曾硬编
    // MORTAL=80，与 attach_cultivation_to_joined_clients 路径用的 AWAKEN=120 数值漂移；
    // 现统一从 spec 读，单一数据源。rotate_to_new_character 已经算过一份（用轮换后的新
    // char_id 做 seed）；轮换失败（无 username/persistence）时才退回旧的直接派生。
    let new_char_spec = rotated_spec.unwrap_or_else(|| {
        crate::cultivation::character_select::next_character_spec_for_seed(&lifecycle.character_id)
    });
    let spawn_position = new_char_spec.spawn_pos;
    let fresh_lifespan = LifespanComponent::new(new_char_spec.lifespan_cap);

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
    if let (Some(default_loadout), Some(item_registry), Some(inventory_allocator)) =
        (default_loadout, item_registry, inventory_allocator)
    {
        let new_inventory = instantiate_inventory_from_loadout(
            &default_loadout.0,
            inventory_allocator,
            item_registry,
        )
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
        AntiCheatCounter::default(),
        QuickSlotBindings::default(),
    ));
    let fresh_known_techniques = technique_registry
        .map(KnownTechniques::progression_reset)
        .unwrap_or_default();
    entity_commands.insert((
        SkillBarBindings::default(),
        UnlockedStyles::default(),
        fresh_known_techniques,
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

    // P0 fix: 清 coffin 状态（新建角色不应继承死亡前的棺状态）。
    // 仅当玩家确实在棺内（clear_player 返回 Some）时才发 CoffinStateChanged，避免噪音推送。
    let was_in_coffin = if let Some(coffin_registry) = coffin_registry {
        coffin_registry.clear_player(entity).is_some()
    } else {
        false
    };
    commands
        .entity(entity)
        .remove::<crate::coffin::CoffinComponent>();
    if was_in_coffin {
        // fresh_lifespan 已覆写 ECS 值（新角色寿元），以 None grade 落盘清棺+新角色寿元同帧落盘
        crate::coffin::persist_in_coffin(player_persistence, username, Some(&fresh_lifespan), None);
        coffin_state_events.send(crate::coffin::CoffinStateChanged {
            player: entity,
            grade: None,
        });
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
    if cause_lower.contains("realm_collapse") {
        return ZoneDeathKind::Death;
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
        Some(BiographyEntry::Death { cause, .. }) => cause.clone(),
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
    current_unix_millis()
        .saturating_add(remaining_ticks.saturating_mul(crate::time::MILLIS_PER_TICK))
}

/// 构造 DeathScreen payload（不负责发送）。抽出来是为了让
/// `reemit_death_screen_for_reconnected_awaiting_revival_clients`（bughunt
/// player-lifecycle-relog-death-consequence-wipe OPUS 返工要求 2）可以复用同一份
/// payload 构造逻辑：它直接持有 `&mut Client`（避免与 `Added<Client>` 过滤器在同一系统里
/// 对 `Client` 组件产生读写冲突），而不是像 `emit_death_screen` 那样通过
/// `Query<&mut Client>` 二次查找。
fn build_death_screen_payload(
    cause: &str,
    decision: RevivalDecision,
    context: DeathScreenContext<'_>,
    now_tick: u64,
    decision_deadline_tick: u64,
) -> ServerDataV1 {
    let zone_kind = death_zone_from_context(cause, context.position, context.zones);
    let rolling = context.lifecycle.revival_roll_survived;
    let cinematic = rolling
        .map(|survived| {
            use crate::schema::death_cinematic::{
                DeathCinematicPhaseV1, DeathCinematicRollV1, DeathRollResultV1,
            };
            let remaining = decision_deadline_tick
                .saturating_sub(now_tick)
                .min(REVIVAL_ROLL_TICKS);
            DeathCinematicS2cV1 {
                v: 1,
                character_id: context.lifecycle.character_id.clone(),
                phase: DeathCinematicPhaseV1::Roll,
                phase_tick: REVIVAL_ROLL_TICKS - remaining,
                phase_duration_ticks: REVIVAL_ROLL_TICKS,
                total_elapsed_ticks: REVIVAL_ROLL_TICKS - remaining,
                total_duration_ticks: REVIVAL_ROLL_TICKS,
                roll: DeathCinematicRollV1 {
                    probability: decision.chance_shown(),
                    threshold: decision.chance_shown(),
                    luck_value: 0.0,
                    result: if survived {
                        DeathRollResultV1::Survive
                    } else {
                        DeathRollResultV1::Fall
                    },
                },
                insight_text: Vec::new(),
                is_final: false,
                death_number: context.lifecycle.death_count,
                zone_kind: crate::death_lifecycle::cinematic::map_zone_kind(zone_kind),
                tsy_death: false,
                rebirth_weakened_ticks: super::components::REVIVE_WEAKENED_TICKS,
            }
        })
        .or(context.cinematic);
    ServerDataV1::new(ServerDataPayloadV1::DeathScreen {
        visible: true,
        cause: cause.to_string(),
        luck_remaining: decision.chance_shown(),
        final_words: context.final_words,
        countdown_until_ms: decision_deadline_ms(decision_deadline_tick, now_tick),
        can_reincarnate: rolling.is_none() && decision.can_reincarnate(),
        can_terminate: rolling.is_none() && decision.can_terminate(),
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
        cinematic,
    })
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
    let payload =
        build_death_screen_payload(cause, decision, context, now_tick, decision_deadline_tick);
    send_payload(clients, entity, payload);
}

pub(crate) fn hide_death_screen(
    clients: &mut Query<&mut valence::prelude::Client>,
    entity: Entity,
) {
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
            cinematic: None,
        }),
    );
}

fn default_final_words(cause: &str, zone_kind: ZoneDeathKind) -> String {
    match zone_kind {
        ZoneDeathKind::Death | ZoneDeathKind::Negative => "秘境所得悉数散落。".to_string(),
        ZoneDeathKind::Ordinary if cause.contains("tribulation") => {
            "此次劫数已记入天道。".to_string()
        }
        ZoneDeathKind::Ordinary => "尘归尘，劫未尽。".to_string(),
    }
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
            summary: None,
        }),
    );
}

pub(crate) fn send_payload(
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

fn clear_death_combat_state(
    wounds: Option<valence::prelude::Mut<'_, Wounds>>,
    status_effects: Option<valence::prelude::Mut<'_, StatusEffects>>,
) {
    if let Some(mut wounds) = wounds {
        wounds.health_current = 0.0;
    }
    if let Some(mut effects) = status_effects {
        effects.active.clear();
    }
}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod tests;
