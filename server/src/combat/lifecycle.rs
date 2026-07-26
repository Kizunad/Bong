use std::time::{SystemTime, UNIX_EPOCH};

use valence::prelude::{
    Added, Client, Commands, Entity, EventReader, EventWriter, Events, GameMode, Position, Query,
    Res, ResMut, Username, Without,
};

use crate::alchemy::LearnedRecipes;
use crate::combat::anticheat::AntiCheatCounter;
use crate::combat::status::health_regen_boost_multiplier;
use crate::combat::CombatClock;
use crate::cultivation::components::{Contamination, Cultivation, MeridianSystem, Realm};
use crate::cultivation::death_hooks::{
    apply_revive_penalty, CultivationDeathCause, CultivationDeathTrigger, PlayerRevived,
    PlayerTerminated,
};
use crate::cultivation::insight::InsightQuota;
use crate::cultivation::insight_apply::{InsightModifiers, UnlockedPerceptions};
use crate::cultivation::known_techniques::KnownTechniques;
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::cultivation::lifespan::{
    calculate_rebirth_chance, lifespan_tick_rate_multiplier, tribulation_rebirth_chance,
    DeathRegistry, LifespanCapTable, LifespanComponent, LifespanEventEmitted, RebirthChanceInput,
    ZoneDeathKind,
};
use crate::cultivation::meridian::severed::MeridianSeveredPermanent;
use crate::cultivation::poison_trait::{DigestionLoad, PoisonToxicity};
use crate::cultivation::tribulation::AscensionQuotaOpened;
use crate::cultivation::{
    color::PracticeLog,
    components::{Karma, QiColor},
};
use crate::fauna::components::FaunaTag;
use crate::inventory::{
    instantiate_inventory_from_loadout, DeathDropAnchor, DefaultLoadout,
    InventoryInstanceIdAllocator, PlayerInventory,
};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::send_server_data_payload;
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::nourishment::tick::NourishmentActivityWindow;
use crate::nourishment::Nourishment;
use crate::npc::spawn::NpcMarker;
use crate::persistence::{
    persist_near_death_transition, persist_new_character_transition,
    persist_revival_transition_with_bundle, persist_termination_transition,
    persist_termination_transition_with_death_context, release_ascension_quota_slot,
    LifespanEventRecord, NewCharacterPersistenceBundle, PersistenceSettings,
    PlayerCultivationBundle,
};
use crate::player::state::{PlayerState, PlayerStatePersistence};
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
use crate::world::dimension::DimensionKind;
use crate::world::spawn_tutorial::TutorialState;
use crate::world::spirit_eye::SpiritEyeRegistry;
use crate::world::zone::ZoneRegistry;

use super::components::{
    CombatState, DerivedAttrs, Lifecycle, LifecycleState, QuickSlotBindings, RevivalDecision,
    ShieldDrainOverride, SkillBarBindings, Stamina, StaminaState, StatusEffects, UnlockedStyles,
    Wounds, ATTACK_STAMINA_COST, BLEED_TICK_INTERVAL_TICKS, COMBAT_STATE_TICK_INTERVAL_TICKS,
    HEALTH_REGEN_TICK_INTERVAL_TICKS, NEAR_DEATH_HEALTH_FRACTION, REVIVAL_CONFIRM_WINDOW_TICKS,
    REVIVE_HEALTH_FRACTION, STAMINA_TICK_INTERVAL_TICKS, TICKS_PER_SECOND,
};
use super::events::{
    CombatEvent, DeathCinematicPublished, DeathEvent, DeathInsightRequested, RevivalActionIntent,
    RevivalActionKind,
};

const COMBAT_DRAIN_PER_SEC: f32 = 5.0;
const JOG_DRAIN_PER_SEC: f32 = 2.0;
const SPRINT_DRAIN_PER_SEC: f32 = 10.0;
/// plan-shield-block-v1 P2 — 举盾持续每秒体力消耗（量级：COMBAT=5.0，JOG=2.0，盾=3.0）。
/// 不触 qi_physics ledger（体力非真元）。P4 按熟练度 1→2 不在本阶段。
pub const SHIELD_DRAIN_PER_SEC: f32 = 3.0;
const EXHAUSTED_RECOVER_RATIO: f32 = 0.5;
const EXHAUSTED_EXIT_FRACTION: f32 = 0.3;
const DEATH_INSIGHT_RECENT_BIO_N: usize = 16;
pub const BASE_HEALTH_REGEN_PER_SEC: f32 = 0.5;

type NearDeathQueryItem<'a> = (
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
    Option<&'a NpcVisualProfile>,
    Option<&'a mut PlayerInventory>,
    (
        (
            Option<&'a mut SkillSet>,
            (
                Option<&'a FaunaTag>,
                Option<&'a mut Nourishment>,
                Option<&'a mut NourishmentActivityWindow>,
            ),
        ),
        (
            Option<&'a QiColor>,
            Option<&'a Karma>,
            Option<&'a PracticeLog>,
            Option<&'a InsightQuota>,
            Option<&'a UnlockedPerceptions>,
            Option<&'a InsightModifiers>,
            Option<&'a MeridianSeveredPermanent>,
            Option<&'a TutorialState>,
            Option<&'a PoisonToxicity>,
            Option<&'a DigestionLoad>,
        ),
    ),
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
            LifecycleState::NearDeath
                | LifecycleState::AwaitingRevival
                | LifecycleState::Terminated
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
    spirit_eyes: Option<Res<SpiritEyeRegistry>>,
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
            status_effects,
            life_record,
            cultivation,
            player_state,
            mut death_registry,
            mut lifespan,
            position,
            npc_visual_profile,
            npc_marker,
        )) = lifecycle_q.get_mut(event.target)
        else {
            continue;
        };

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
        // 已经在死亡屏（AwaitingRevival）等待玩家决策的实体不接受新死亡事件重入——
        // 否则濒死窗口每 tick 被新死亡事件拍回 NearDeath，AwaitingRevival 窗口实际只活 1 tick，
        // 玩家永远点不中重生按钮（bughunt 实证：污染溢出持续触发死亡导致死循环）。
        if matches!(
            lifecycle.state,
            LifecycleState::NearDeath
                | LifecycleState::AwaitingRevival
                | LifecycleState::Terminated
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
            known_spirit_eyes: known_spirit_eyes_for_death_insight(
                life_record.as_deref(),
                &lifecycle,
                spirit_eyes.as_deref(),
            ),
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
        enter_near_death(&mut lifecycle, wounds, status_effects, now_tick);
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
        // 同上：AwaitingRevival 期间不接受新的 cultivation 死亡事件重入。
        if matches!(
            lifecycle.state,
            LifecycleState::NearDeath
                | LifecycleState::AwaitingRevival
                | LifecycleState::Terminated
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
            will_terminate: lifespan_exhausted,
            known_spirit_eyes: known_spirit_eyes_for_death_insight(
                life_record.as_deref(),
                &lifecycle,
                spirit_eyes.as_deref(),
            ),
        });

        if lifespan_exhausted {
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
        enter_near_death(&mut lifecycle, wounds, status_effects, clock.tick);
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
    mut commands: Commands,
    mut terminated: EventWriter<PlayerTerminated>,
    mut death_cinematics: ResMut<Events<DeathCinematicPublished>>,
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
        npc_visual_profile,
        _inventory,
        ((_skill_set, (fauna_tag, _nourishment, _nourishment_activity)), _bundle),
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

        let immediate_npc_termination =
            should_terminate_npc_without_near_death_wait(npc_marker, fauna_tag);
        if !immediate_npc_termination {
            let Some(deadline_tick) = lifecycle.near_death_deadline_tick else {
                continue;
            };
            if clock.tick < deadline_tick {
                continue;
            }
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
                npc_marker.is_some(),
                npc_visual_profile,
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
                npc_marker.is_some(),
                npc_visual_profile,
                &mut vfx_events,
                "natural_end",
            ) {
                hide_death_screen(&mut clients, entity);
            }
            continue;
        };

        let decision_deadline_tick = clock.tick.saturating_add(REVIVAL_CONFIRM_WINDOW_TICKS);
        lifecycle.await_revival_decision(decision, decision_deadline_tick);
        let cause = eventual_cause(life_record.as_deref());
        let death_zone =
            death_zone_from_context(cause.as_str(), position.as_deref(), zones.as_deref());
        let final_words = vec![default_final_words(cause.as_str(), death_zone)];
        let cinematic = crate::death_lifecycle::cinematic::build_death_cinematic(
            &lifecycle,
            death_registry.as_deref(),
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
        emit_death_screen(
            &mut clients,
            entity,
            cause.as_str(),
            decision,
            DeathScreenContext {
                lifecycle: &lifecycle,
                death_registry: death_registry.as_deref(),
                lifespan: lifespan.as_deref(),
                position: position.as_deref(),
                zones: zones.as_deref(),
                final_words,
                cinematic: Some(cinematic_payload),
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

fn should_terminate_npc_without_near_death_wait(
    npc_marker: Option<&NpcMarker>,
    _fauna_tag: Option<&FaunaTag>,
) -> bool {
    // All NPCs skip the NearDeath wait window and go straight to Terminated.
    // NearDeath is only meaningful for players who need a revival decision window.
    npc_marker.is_some()
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

/// bughunt player-lifecycle-relog-death-consequence-wipe（OPUS 返工要求 2）：断线时正处于
/// `AwaitingRevival`（濒死已判定出渡劫/大限决策、等待玩家确认）的角色，重连后必须重新
/// 收到死亡屏与 `DeathCinematic`——不能让玩家在满血、无任何 UI 解释的情况下静默"裸奔"
/// 在这个会阻断攻防（见 `resolve.rs` 对 `LifecycleState::AwaitingRevival` 的双向 gate）、
/// 又会在 deadline 到期后被 `auto_confirm_revival_decisions` 强制结算（可能永久终结角色，
/// 见 `RevivalDecision::Tribulation`）的状态里。
///
/// 只处理 `AwaitingRevival`：`NearDeath` 本身没有独立的"死亡屏"（濒死靠 `Wounds.
/// health_current` 走低血量 HUD 呈现），重连时 `Wounds::default()` 会让血量满血复位，
/// `near_death_tick` 下一 tick 就会判定"已稳定"并静默清回 `Alive`——这属于
/// `Wounds`/`NearDeath` 秒退漏洞（另案跟踪，不在本次返工范围内），这里不重复处理。
///
/// 直接在查询数据元组里拿 `&mut Client`（而不是像 `emit_death_screen` 那样另开一个
/// `Query<&mut Client>`），是为了避免同一系统里 `Added<Client>` 过滤器（要求对 `Client`
/// 的读访问）与另一个 `Query<&mut Client>`（要求写访问）产生 Bevy 查询访问冲突 panic。
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
            // panic，交由 near_death_tick/auto_confirm 之类的常规 tick 逻辑去纠偏。
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

#[allow(clippy::too_many_arguments)]
pub fn handle_revival_action_intents(
    clock: Res<CombatClock>,
    persistence: Res<PersistenceSettings>,
    player_persistence: Option<Res<PlayerStatePersistence>>,
    default_loadout: Option<Res<DefaultLoadout>>,
    item_registry: Option<Res<crate::inventory::ItemRegistry>>,
    mut inventory_allocator: Option<ResMut<InventoryInstanceIdAllocator>>,
    mut intents: EventReader<RevivalActionIntent>,
    mut revived: EventWriter<PlayerRevived>,
    mut terminated: EventWriter<PlayerTerminated>,
    mut quota_opened: EventWriter<AscensionQuotaOpened>,
    mut commands: valence::prelude::Commands,
    mut lifecycle_q: Query<NearDeathPersistenceQueryItem<'_>>,
    mut clients: Query<&mut valence::prelude::Client>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    // P0 fix: coffin 清除参数（复活/新建时彻底清除 coffin 状态）
    mut coffin_registry: Option<ResMut<crate::coffin::CoffinRegistry>>,
    mut coffin_state_events: EventWriter<crate::coffin::CoffinStateChanged>,
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
            npc_marker,
            npc_visual_profile,
            inventory,
            (
                (skill_set, (_fauna_tag, nourishment, nourishment_activity)),
                (
                    qi_color,
                    karma,
                    practice_log,
                    insight_quota,
                    unlocked_perceptions,
                    insight_modifiers,
                    meridian_severed,
                    tutorial_state,
                    poison_toxicity,
                    digestion_load,
                ),
            ),
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
                    let (
                        Some(qi_color),
                        Some(karma),
                        Some(practice_log),
                        Some(insight_quota),
                        Some(unlocked_perceptions),
                        Some(insight_modifiers),
                        Some(meridian_severed),
                    ) = (
                        qi_color,
                        karma,
                        practice_log,
                        insight_quota,
                        unlocked_perceptions,
                        insight_modifiers,
                        meridian_severed,
                    )
                    else {
                        tracing::warn!(
                            "[bong][combat] refusing revival for {entity:?}: cultivation bundle is incomplete"
                        );
                        continue;
                    };
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
                        nourishment,
                        nourishment_activity,
                        qi_color,
                        karma,
                        practice_log,
                        insight_quota,
                        unlocked_perceptions,
                        insight_modifiers,
                        meridian_severed,
                        tutorial_state,
                        poison_toxicity,
                        digestion_load,
                        &mut revived,
                        &mut quota_opened,
                        &mut commands,
                        coffin_registry.as_deref_mut(),
                        &mut coffin_state_events,
                        username,
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
                    &mut terminated,
                    position.as_deref(),
                    npc_marker.is_some(),
                    npc_visual_profile,
                    &mut vfx_events,
                    "tribulation_failed",
                ) {
                    // 劫数不过 → 形神俱散：清 coffin 状态（四件套），防止 Registry/ECS/SQLite 残留导致重启复钉。
                    clear_coffin_on_exit(
                        entity,
                        &mut commands,
                        coffin_registry.as_deref_mut(),
                        &mut coffin_state_events,
                        player_persistence.as_deref(),
                        username,
                        lifespan.as_deref(),
                    );
                    commands
                        .entity(entity)
                        .remove::<crate::death_lifecycle::cinematic::DeathCinematic>();
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
                    npc_marker.is_some(),
                    npc_visual_profile,
                    &mut vfx_events,
                    "voluntary_retire",
                ) {
                    // 主动归隐终结：清 coffin 状态（四件套），防止 Registry/ECS/SQLite 残留导致重启复钉。
                    clear_coffin_on_exit(
                        entity,
                        &mut commands,
                        coffin_registry.as_deref_mut(),
                        &mut coffin_state_events,
                        player_persistence.as_deref(),
                        username,
                        lifespan.as_deref(),
                    );
                    commands
                        .entity(entity)
                        .remove::<crate::death_lifecycle::cinematic::DeathCinematic>();
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
                if reset_for_new_character(
                    entity,
                    &mut commands,
                    clock.tick,
                    &persistence,
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
                    nourishment,
                    nourishment_activity,
                    tutorial_state,
                    meridian_severed,
                    poison_toxicity,
                    digestion_load,
                    player_persistence.as_deref(),
                    default_loadout.as_deref(),
                    item_registry.as_deref(),
                    inventory_allocator.as_deref_mut(),
                    coffin_registry.as_deref_mut(),
                    &mut coffin_state_events,
                ) {
                    commands
                        .entity(entity)
                        .remove::<crate::death_lifecycle::cinematic::DeathCinematic>();
                    hide_death_screen(&mut clients, entity);
                    hide_terminate_screen(&mut clients, entity);
                }
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
    nourishment: Option<valence::prelude::Mut<'_, Nourishment>>,
    nourishment_activity: Option<valence::prelude::Mut<'_, NourishmentActivityWindow>>,
    qi_color: &QiColor,
    karma: &Karma,
    practice_log: &PracticeLog,
    insight_quota: &InsightQuota,
    unlocked_perceptions: &UnlockedPerceptions,
    insight_modifiers: &InsightModifiers,
    meridian_severed: &MeridianSeveredPermanent,
    tutorial_state: Option<&TutorialState>,
    poison_toxicity: Option<&PoisonToxicity>,
    digestion_load: Option<&DigestionLoad>,
    revived: &mut EventWriter<PlayerRevived>,
    quota_opened: &mut EventWriter<AscensionQuotaOpened>,
    // P0 fix: coffin 清除参数（复活后不应继续锁棺）
    commands: &mut valence::prelude::Commands,
    coffin_registry: Option<&mut crate::coffin::CoffinRegistry>,
    coffin_state_events: &mut EventWriter<crate::coffin::CoffinStateChanged>,
    coffin_username: Option<&Username>,
    coffin_player_persistence: Option<&PlayerStatePersistence>,
) -> bool {
    let (Some(username), Some(player_persistence), Some(mut position)) =
        (coffin_username, coffin_player_persistence, position)
    else {
        tracing::warn!(
            "[bong][combat] refusing revival for {entity:?}: Username, PlayerStatePersistence, or Position is missing"
        );
        return false;
    };
    let revived_position = lifecycle
        .spawn_anchor
        .unwrap_or_else(crate::player::spawn_position);

    let mut staged_lifecycle = lifecycle.clone();
    if matches!(
        lifecycle.awaiting_decision,
        Some(RevivalDecision::Fortune { .. })
    ) {
        staged_lifecycle.fortune_remaining = staged_lifecycle.fortune_remaining.saturating_sub(1);
    }
    let weakened_multiplier = damaged_spawn_anchor_weakened_multiplier(lifecycle);
    staged_lifecycle.revive_with_weakened_multiplier(now_tick, weakened_multiplier);

    let (mut staged_cultivation, mut staged_meridians, mut staged_contam, mut staged_life_record) =
        match (
            cultivation.as_ref().map(|value| (**value).clone()),
            meridians.as_ref().map(|value| (**value).clone()),
            contam.as_ref().map(|value| (**value).clone()),
            life_record.as_ref().map(|value| (**value).clone()),
        ) {
            (Some(cultivation), Some(meridians), Some(contamination), Some(life_record)) => {
                (cultivation, meridians, contamination, life_record)
            }
            _ => {
                tracing::warn!("[bong][combat] refusing revival for {entity:?}: required lifecycle bundle component is missing");
                return false;
            }
        };
    let staged_nourishment = Nourishment::spawn_default();
    let prior_realm = staged_cultivation.realm;
    apply_revive_penalty(
        &mut staged_cultivation,
        &mut staged_meridians,
        &mut staged_contam,
    );
    staged_life_record.push(BiographyEntry::Rebirth {
        prior_realm,
        new_realm: staged_cultivation.realm,
        tick: now_tick,
    });
    if let Err(error) = persist_revival_transition_with_bundle(
        persistence,
        player_persistence,
        username.0.as_str(),
        revived_position,
        crate::persistence::PlayerCultivationBundle {
            cultivation: &staged_cultivation,
            meridians: &staged_meridians,
            qi_color,
            karma,
            contamination: &staged_contam,
            life_record: &staged_life_record,
            practice_log,
            insight_quota,
            unlocked_perceptions,
            insight_modifiers,
            tutorial_state,
            meridian_severed,
            poison_toxicity,
            digestion_load,
            nourishment: &staged_nourishment,
        },
    ) {
        tracing::warn!(
            "[bong][persistence] failed to persist revival transition for {}: {error}",
            staged_life_record.character_id
        );
        return false;
    }
    if prior_realm == Realm::Void && staged_cultivation.realm != Realm::Void {
        match release_ascension_quota_slot(persistence) {
            Ok(release) if release.opened_slot => {
                quota_opened.send(AscensionQuotaOpened {
                    occupied_slots: release.quota.occupied_slots,
                });
            }
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(
                    "[bong][combat] failed to release ascension quota after revive for {:?}: {error}",
                    entity,
                );
            }
        }
    }

    lifecycle.fortune_remaining = staged_lifecycle.fortune_remaining;
    lifecycle.revive_with_weakened_multiplier(now_tick, weakened_multiplier);
    if let Some(mut cultivation) = cultivation {
        *cultivation = staged_cultivation;
    }
    if let Some(mut meridians) = meridians {
        *meridians = staged_meridians;
    }
    if let Some(mut contam) = contam {
        *contam = staged_contam;
    }
    if let Some(mut life_record) = life_record {
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
    position.set(revived_position);

    if let Some(mut nourishment) = nourishment {
        nourishment.reset_to_spawn();
    } else {
        commands.entity(entity).insert(Nourishment::spawn_default());
    }
    if let Some(mut nourishment_activity) = nourishment_activity {
        nourishment_activity.reset();
    } else {
        commands
            .entity(entity)
            .insert(NourishmentActivityWindow::default());
    }

    // SQLite 已在 revival transaction 内清除 in_coffin；此处只提交运行时副作用。
    clear_coffin_runtime(entity, commands, coffin_registry, coffin_state_events);
    revived.send(PlayerRevived { entity });
    true
}

///
/// 用于 revive / terminate / new_char 三条退出路径，确保任何离棺场景都不遗漏。
/// 仅当玩家确实在棺内（registry.clear_player 返回 Some）时才落持久化和事件，避免噪音。
fn clear_coffin_runtime(
    entity: Entity,
    commands: &mut valence::prelude::Commands,
    coffin_registry: Option<&mut crate::coffin::CoffinRegistry>,
    coffin_state_events: &mut EventWriter<crate::coffin::CoffinStateChanged>,
) -> bool {
    let was_in_coffin = coffin_registry
        .and_then(|registry| registry.clear_player(entity))
        .is_some();
    commands
        .entity(entity)
        .remove::<crate::coffin::CoffinComponent>();
    if was_in_coffin {
        coffin_state_events.send(crate::coffin::CoffinStateChanged {
            player: entity,
            grade: None,
        });
    }
    was_in_coffin
}

/// Runtime + standalone persistence wrapper for exit paths that do not own a larger transaction.
fn clear_coffin_on_exit(
    entity: Entity,
    commands: &mut valence::prelude::Commands,
    coffin_registry: Option<&mut crate::coffin::CoffinRegistry>,
    coffin_state_events: &mut EventWriter<crate::coffin::CoffinStateChanged>,
    player_persistence: Option<&PlayerStatePersistence>,
    username: Option<&Username>,
    lifespan: Option<&crate::cultivation::lifespan::LifespanComponent>,
) {
    if clear_coffin_runtime(entity, commands, coffin_registry, coffin_state_events) {
        crate::coffin::persist_in_coffin(player_persistence, username, lifespan, None);
    }
}

fn damaged_spawn_anchor_weakened_multiplier(lifecycle: &Lifecycle) -> u64 {
    if lifecycle.spawn_anchor.is_some() && lifecycle.spawn_anchor_damaged {
        2
    } else {
        1
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
        if is_npc {
            vfx_events.send(crate::skin::faction_tint::npc_death_smoke_request(p));
            if let Some(request) =
                crate::skin::faction_tint::npc_death_qi_burst_request(p, npc_visual_profile)
            {
                vfx_events.send(request);
            }
        }
    }
    true
}

#[allow(clippy::too_many_arguments)]
fn reset_for_new_character(
    entity: Entity,
    commands: &mut valence::prelude::Commands,
    now_tick: u64,
    persistence: &PersistenceSettings,
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
    nourishment: Option<valence::prelude::Mut<'_, Nourishment>>,
    nourishment_activity: Option<valence::prelude::Mut<'_, NourishmentActivityWindow>>,
    tutorial_state: Option<&TutorialState>,
    meridian_severed: Option<&MeridianSeveredPermanent>,
    poison_toxicity: Option<&PoisonToxicity>,
    digestion_load: Option<&DigestionLoad>,
    player_persistence: Option<&PlayerStatePersistence>,
    default_loadout: Option<&DefaultLoadout>,
    item_registry: Option<&crate::inventory::ItemRegistry>,
    inventory_allocator: Option<&mut InventoryInstanceIdAllocator>,
    // P0 fix: coffin 清除参数（新建角色不应继承死亡前的棺状态）
    coffin_registry: Option<&mut crate::coffin::CoffinRegistry>,
    coffin_state_events: &mut EventWriter<crate::coffin::CoffinStateChanged>,
) -> bool {
    let (
        Some(username),
        Some(player_persistence),
        Some(default_loadout),
        Some(item_registry),
        Some(inventory_allocator),
    ) = (
        username,
        player_persistence,
        default_loadout,
        item_registry,
        inventory_allocator,
    )
    else {
        tracing::warn!(
            "[bong][combat] refusing CreateNewCharacter for {entity:?}: persistence or default inventory resources are incomplete"
        );
        return false;
    };

    let reincarnation =
        crate::cultivation::character_select::prepare_new_character(username.0.as_str());
    let mut staged_lifecycle = lifecycle.clone();
    staged_lifecycle.character_id = reincarnation.next_character_id.clone();
    crate::cultivation::luck_pool::reset_for_new_life(&mut staged_lifecycle);
    staged_lifecycle.last_death_tick = None;
    staged_lifecycle.last_revive_tick = Some(now_tick);
    staged_lifecycle.spawn_anchor = None;
    staged_lifecycle.near_death_deadline_tick = None;
    staged_lifecycle.awaiting_decision = None;
    staged_lifecycle.revival_decision_deadline_tick = None;
    staged_lifecycle.weakened_until_tick = None;
    staged_lifecycle.state = LifecycleState::Alive;

    let fresh_life_record = LifeRecord::new(staged_lifecycle.character_id.clone());
    let fresh_death_registry = DeathRegistry::new(staged_lifecycle.character_id.clone());
    let fresh_player_state = PlayerState::default();
    let spawn_position = reincarnation.spec.spawn_pos;
    let fresh_lifespan = LifespanComponent::new(reincarnation.spec.lifespan_cap);

    let mut staged_inventory_allocator = inventory_allocator.clone();
    let fresh_inventory = match instantiate_inventory_from_loadout(
        &default_loadout.0,
        &mut staged_inventory_allocator,
        item_registry,
    ) {
        Ok(inventory) => inventory,
        Err(error) => {
            tracing::warn!(
                "[bong][combat] refusing CreateNewCharacter for {entity:?}: default loadout failed: {error}"
            );
            return false;
        }
    };
    let fresh_skill_set = SkillSet::default();
    let fresh_nourishment = Nourishment::spawn_default();
    let fresh_cultivation = Cultivation::default();
    let fresh_meridians = MeridianSystem::default();
    let fresh_qi_color = QiColor::default();
    let fresh_karma = Karma::default();
    let fresh_contamination = Contamination::default();
    let fresh_practice_log = PracticeLog::default();
    let fresh_insight_quota = InsightQuota::default();
    let fresh_unlocked_perceptions = UnlockedPerceptions::default();
    let fresh_insight_modifiers = InsightModifiers::new();
    let persisted_meridian_severed = meridian_severed.cloned().unwrap_or_default();

    if let Err(error) = persist_new_character_transition(
        persistence,
        player_persistence,
        username.0.as_str(),
        NewCharacterPersistenceBundle {
            current_char_id: reincarnation.current_char_id.as_str(),
            state: &fresh_player_state,
            position: spawn_position,
            inventory: Some(&fresh_inventory),
            lifespan: &fresh_lifespan,
            skill_set: &fresh_skill_set,
            cultivation: PlayerCultivationBundle {
                cultivation: &fresh_cultivation,
                meridians: &fresh_meridians,
                qi_color: &fresh_qi_color,
                karma: &fresh_karma,
                contamination: &fresh_contamination,
                life_record: &fresh_life_record,
                practice_log: &fresh_practice_log,
                insight_quota: &fresh_insight_quota,
                unlocked_perceptions: &fresh_unlocked_perceptions,
                insight_modifiers: &fresh_insight_modifiers,
                tutorial_state,
                meridian_severed: &persisted_meridian_severed,
                poison_toxicity,
                digestion_load,
                nourishment: &fresh_nourishment,
            },
        },
    ) {
        tracing::warn!(
            "[bong][persistence] failed to persist fresh character transaction for `{}`: {error}",
            username.0
        );
        return false;
    }

    *lifecycle = staged_lifecycle;
    *inventory_allocator = staged_inventory_allocator;
    if let Some(mut life_record) = life_record {
        *life_record = fresh_life_record;
    }
    if let Some(mut death_registry) = death_registry {
        *death_registry = fresh_death_registry;
    }
    if let Some(mut lifespan) = lifespan {
        *lifespan = fresh_lifespan.clone();
    } else {
        commands.entity(entity).insert(fresh_lifespan.clone());
    }
    if let Some(mut player_state) = player_state {
        *player_state = fresh_player_state;
    }
    if let Some(mut position) = position {
        position.set(spawn_position);
    }
    if let Some(mut inventory) = inventory {
        *inventory = fresh_inventory.clone();
    } else {
        commands.entity(entity).insert(fresh_inventory);
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
        *skill_set = fresh_skill_set;
    } else {
        commands.entity(entity).insert(fresh_skill_set);
    }
    if let Some(mut nourishment) = nourishment {
        *nourishment = fresh_nourishment;
    } else {
        commands.entity(entity).insert(fresh_nourishment);
    }
    if let Some(mut nourishment_activity) = nourishment_activity {
        nourishment_activity.reset();
    } else {
        commands
            .entity(entity)
            .insert(NourishmentActivityWindow::default());
    }

    let mut learned_recipes = LearnedRecipes::default();
    learned_recipes.learn("kai_mai_pill_v0".into());
    let mut entity_commands = commands.entity(entity);
    entity_commands.insert((
        fresh_cultivation,
        fresh_meridians,
        fresh_qi_color,
        fresh_karma,
        fresh_practice_log,
        fresh_contamination,
        fresh_insight_quota,
        fresh_unlocked_perceptions,
        fresh_insight_modifiers,
        StatusEffects::default(),
        DerivedAttrs::default(),
        AntiCheatCounter::default(),
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

    clear_coffin_runtime(entity, commands, coffin_registry, coffin_state_events);

    tracing::info!(
        "[bong][combat] rotated current_char_id for `{}` to {}",
        username.0,
        reincarnation.next_character_id
    );
    true
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
    ServerDataV1::new(ServerDataPayloadV1::DeathScreen {
        visible: true,
        cause: cause.to_string(),
        luck_remaining: decision.chance_shown(),
        final_words: context.final_words,
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
        cinematic: context.cinematic,
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
    status_effects: Option<valence::prelude::Mut<'_, StatusEffects>>,
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
    if let Some(mut status_effects) = status_effects {
        if !status_effects.active.is_empty() {
            status_effects.active.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::combat::anticheat::AntiCheatCounter;
    use crate::combat::components::{
        ActiveStatusEffect, BodyPart, DefenseWindow, StatusEffects, Wound, WoundKind,
        IN_COMBAT_WINDOW_TICKS, REVIVE_WEAKENED_TICKS,
    };
    use crate::combat::events::{
        ApplyStatusEffectIntent, DefenseIntent, RevivalActionIntent, RevivalActionKind,
        StatusEffectKind,
    };
    use crate::cultivation::components::Cultivation;
    use crate::cultivation::death_hooks::CultivationDeathCause;
    use crate::cultivation::life_record::LifeRecord;
    use crate::cultivation::tick::CultivationClock;
    use crate::death_lifecycle::cinematic::DeathCinematicInit;
    use crate::movement::{MovementAction, MovementState};
    use crate::network::agent_bridge::SERVER_DATA_CHANNEL;
    use crate::persistence::{
        bootstrap_sqlite, complete_tribulation_ascension, load_ascension_quota,
        persist_active_tribulation, ActiveTribulationRecord, DeceasedIndexEntry, DeceasedSnapshot,
        PersistenceSettings,
    };
    use crate::player::state::{
        canonical_player_id, load_player_slices, player_character_id, save_player_slices,
    };
    use crate::qi_physics::constants::QI_ZHENMAI_PREP_WINDOW_MS;
    use crate::schema::anticheat::ViolationKindV1;
    use crate::schema::death_cinematic::{
        DeathCinematicRollV1, DeathCinematicZoneKindV1, DeathRollResultV1,
    };
    use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
    use rusqlite::{params, Connection};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use valence::prelude::{App, Events, GameMode, IntoSystemConfigs, Update};
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

    fn load_persisted_nourishment(settings: &PersistenceSettings, username: &str) -> Nourishment {
        let bundle = crate::persistence::load_player_cultivation_bundle(settings, username)
            .expect("cultivation bundle reload should succeed")
            .expect("cultivation bundle should exist");
        assert!(
            bundle.get("nourishment_activity_window").is_none(),
            "lifecycle persistence must not write session-only activity"
        );
        serde_json::from_value(
            bundle
                .get("nourishment")
                .cloned()
                .expect("cultivation bundle should persist nourishment"),
        )
        .expect("persisted nourishment should decode")
    }

    fn seed_revival_nourishment_bundle(
        settings: &PersistenceSettings,
        username: &str,
        cultivation: &Cultivation,
        meridians: &MeridianSystem,
        contamination: &Contamination,
        life_record: &LifeRecord,
        nourishment: Nourishment,
    ) {
        crate::persistence::persist_player_cultivation_bundle_with_nourishment(
            settings,
            username,
            cultivation,
            meridians,
            &crate::cultivation::components::QiColor::default(),
            &crate::cultivation::components::Karma::default(),
            contamination,
            life_record,
            &crate::cultivation::color::PracticeLog::default(),
            &crate::cultivation::insight::InsightQuota::default(),
            &crate::cultivation::insight_apply::UnlockedPerceptions::default(),
            &crate::cultivation::insight_apply::InsightModifiers::new(),
            None,
            &crate::cultivation::meridian::severed::MeridianSeveredPermanent::default(),
            None,
            None,
            Some(&nourishment),
        )
        .expect("test setup must persist the pre-revival nourishment axes");
    }

    fn revival_action_test_app(settings: PersistenceSettings, tick: u64) -> App {
        let mut app = App::new();
        let player_persistence = PlayerStatePersistence::with_db_path(
            settings
                .db_path()
                .parent()
                .unwrap_or_else(|| std::path::Path::new(".")),
            settings.db_path(),
        );
        app.insert_resource(settings);
        app.insert_resource(player_persistence);
        app.insert_resource(CombatClock { tick });
        app.add_event::<valence::movement::MovementEvent>();
        app.add_event::<RevivalActionIntent>();
        app.add_event::<PlayerRevived>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<crate::coffin::CoffinStateChanged>();
        app.add_systems(
            Update,
            (
                crate::nourishment::tick::sample_activity,
                handle_revival_action_intents,
            ),
        );
        app
    }

    struct RevivalActionActorState {
        lifecycle: Lifecycle,
        cultivation: Cultivation,
        meridians: MeridianSystem,
        contamination: Contamination,
        life_record: LifeRecord,
        nourishment: Nourishment,
    }

    fn spawn_revival_action_actor(
        app: &mut App,
        username: &str,
        state: RevivalActionActorState,
    ) -> (Entity, MockClientHelper) {
        let RevivalActionActorState {
            lifecycle,
            cultivation,
            meridians,
            contamination,
            life_record,
            nourishment,
        } = state;
        let (client_bundle, helper) = create_mock_client(username);
        let entity = app.world_mut().spawn_empty().id();
        app.world_mut().entity_mut(entity).insert(client_bundle);
        app.world_mut().entity_mut(entity).insert((
            lifecycle,
            cultivation,
            meridians,
            contamination,
            life_record,
            Wounds {
                health_current: 3.0,
                health_max: 30.0,
                entries: vec![Wound {
                    location: crate::body_plan::legacy_body_part_to_id(BodyPart::Chest),
                    kind: WoundKind::Cut,
                    severity: 0.5,
                    bleeding_per_sec: 1.0,
                    created_at_tick: 1,
                    inflicted_by: None,
                }],
            },
            Stamina {
                current: 2.0,
                max: 10.0,
                state: StaminaState::Combat,
                ..Default::default()
            },
            CombatState {
                incoming_window: Some(DefenseWindow {
                    opened_at_tick: 1,
                    duration_ms: 950,
                }),
                in_combat_until_tick: Some(20),
                last_attack_at_tick: Some(1),
            },
            Position::new([12.0, 70.0, -4.0]),
            Username(username.to_string()),
        ));
        app.world_mut().entity_mut(entity).insert((
            crate::cultivation::components::QiColor::default(),
            crate::cultivation::components::Karma::default(),
            crate::cultivation::color::PracticeLog::default(),
            crate::cultivation::insight::InsightQuota::default(),
            crate::cultivation::insight_apply::UnlockedPerceptions::default(),
            crate::cultivation::insight_apply::InsightModifiers::new(),
            crate::cultivation::meridian::severed::MeridianSeveredPermanent::default(),
            NourishmentActivityWindow::default(),
            nourishment,
            MovementState {
                action: MovementAction::Dashing,
                ..Default::default()
            },
        ));
        (entity, helper)
    }

    #[test]
    fn reincarnate_action_gate_rejection_preserves_ecs_and_sqlite_nourishment() {
        let (settings, root) = persistence_settings("reincarnate-action-gate-rejection");
        let username = "RejectedReincarnate";
        let cultivation = Cultivation {
            realm: Realm::Induce,
            qi_current: 7.0,
            qi_max: 24.0,
            ..Default::default()
        };
        let meridians = MeridianSystem::default();
        let contamination = Contamination::default();
        let life_record = LifeRecord::new(crate::player::state::canonical_player_id(username));
        let persisted_nourishment = Nourishment {
            satiety: 37.0,
            hydration: 46.0,
        };
        seed_revival_nourishment_bundle(
            &settings,
            username,
            &cultivation,
            &meridians,
            &contamination,
            &life_record,
            persisted_nourishment,
        );

        let mut app = revival_action_test_app(settings.clone(), 700);
        let (entity, _helper) = spawn_revival_action_actor(
            &mut app,
            username,
            RevivalActionActorState {
                lifecycle: Lifecycle::default(),
                cultivation,
                meridians,
                contamination,
                life_record,
                nourishment: persisted_nourishment,
            },
        );
        app.update();
        let activity_before = *app
            .world()
            .get::<NourishmentActivityWindow>(entity)
            .expect("sampling update must retain the activity window");
        assert_eq!(
            activity_before.observed_flags(),
            (false, true),
            "test setup must provide non-empty session activity before the rejected intent"
        );
        let lifecycle_before = serde_json::to_value(app.world().get::<Lifecycle>(entity).unwrap())
            .expect("lifecycle snapshot should serialize");
        let nourishment_before = *app.world().get::<Nourishment>(entity).unwrap();
        let life_record_before =
            serde_json::to_value(app.world().get::<LifeRecord>(entity).unwrap())
                .expect("life record snapshot should serialize");

        app.world_mut().send_event(RevivalActionIntent {
            entity,
            action: RevivalActionKind::Reincarnate,
            issued_at_tick: 700,
        });
        app.update();

        assert_eq!(
            serde_json::to_value(app.world().get::<Lifecycle>(entity).unwrap())
                .expect("lifecycle snapshot should serialize"),
            lifecycle_before,
            "Reincarnate outside AwaitingRevival must not create a revival transition"
        );
        assert_eq!(
            *app.world().get::<Nourishment>(entity).unwrap(),
            nourishment_before,
            "a rejected Reincarnate must not reset live satiety or hydration"
        );
        assert_eq!(
            *app.world()
                .get::<NourishmentActivityWindow>(entity)
                .unwrap(),
            activity_before,
            "a rejected Reincarnate must not clear session movement or dash flags"
        );
        assert_eq!(
            serde_json::to_value(app.world().get::<LifeRecord>(entity).unwrap())
                .expect("life record snapshot should serialize"),
            life_record_before,
            "the action gate must not append a rebirth biography entry"
        );
        assert_eq!(
            app.world().resource::<Events<PlayerRevived>>().len(),
            0,
            "a rejected Reincarnate must not emit PlayerRevived"
        );
        assert_eq!(
            load_persisted_nourishment(&settings, username),
            persisted_nourishment,
            "a rejected Reincarnate must leave the stored nourishment axes untouched"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reincarnate_precommit_failure_rolls_back_sqlite_and_ecs() {
        let (settings, root) = persistence_settings("reincarnate-persistence-atomicity");
        let username = "AtomicReincarnate";
        let cultivation = Cultivation {
            realm: Realm::Induce,
            qi_current: 7.0,
            qi_max: 24.0,
            ..Default::default()
        };
        let meridians = MeridianSystem::default();
        let contamination = Contamination::default();
        let life_record = LifeRecord::new(crate::player::state::canonical_player_id(username));
        let persisted_nourishment = Nourishment {
            satiety: 21.0,
            hydration: 34.0,
        };
        seed_revival_nourishment_bundle(
            &settings,
            username,
            &cultivation,
            &meridians,
            &contamination,
            &life_record,
            persisted_nourishment,
        );

        let old_position = [12.0, 70.0, -4.0];
        let old_lifespan = LifespanComponent::new(LifespanCapTable::AWAKEN);
        let player_persistence =
            PlayerStatePersistence::with_db_path(root.join("data"), settings.db_path());
        crate::player::state::save_player_slices_with_coffin(
            &player_persistence,
            username,
            &PlayerState::default(),
            old_position,
            DimensionKind::Tsy,
            None,
            Some(&old_lifespan),
            &SkillSet::default(),
            Some(crate::coffin::CoffinGrade::Jade),
            None,
        )
        .expect("test setup must persist pre-revival player slices and coffin state");

        let mut app = revival_action_test_app(settings.clone(), 701);
        let (entity, mut helper) = spawn_revival_action_actor(
            &mut app,
            username,
            RevivalActionActorState {
                lifecycle: Lifecycle {
                    state: LifecycleState::AwaitingRevival,
                    awaiting_decision: Some(RevivalDecision::Fortune { chance: 1.0 }),
                    fortune_remaining: 1,
                    ..Default::default()
                },
                cultivation,
                meridians,
                contamination,
                life_record,
                nourishment: persisted_nourishment,
            },
        );
        let coffin_lower = valence::prelude::BlockPos::new(10, 64, 10);
        let coffin_before = crate::coffin::CoffinComponent {
            entered_at_tick: 400,
            coffin_lower,
            grade: crate::coffin::CoffinGrade::Jade,
        };
        app.world_mut().entity_mut(entity).insert(coffin_before);
        let mut coffin_registry = crate::coffin::CoffinRegistry::default();
        assert!(coffin_registry.insert(coffin_lower, 300, crate::coffin::CoffinGrade::Jade));
        assert!(coffin_registry.set_occupied(coffin_lower, entity));
        app.insert_resource(coffin_registry);
        app.update();
        let lifecycle_before = serde_json::to_value(app.world().get::<Lifecycle>(entity).unwrap())
            .expect("lifecycle snapshot should serialize");
        let cultivation_before = app.world().get::<Cultivation>(entity).unwrap().clone();
        let meridians_before = app.world().get::<MeridianSystem>(entity).unwrap().clone();
        let contamination_before =
            serde_json::to_value(app.world().get::<Contamination>(entity).unwrap())
                .expect("contamination snapshot should serialize");
        let life_record_before =
            serde_json::to_value(app.world().get::<LifeRecord>(entity).unwrap())
                .expect("life record snapshot should serialize");
        let wounds_before = serde_json::to_value(app.world().get::<Wounds>(entity).unwrap())
            .expect("wounds snapshot should serialize");
        let stamina_before = serde_json::to_value(app.world().get::<Stamina>(entity).unwrap())
            .expect("stamina snapshot should serialize");
        let combat_before = serde_json::to_value(app.world().get::<CombatState>(entity).unwrap())
            .expect("combat snapshot should serialize");
        let nourishment_before = *app.world().get::<Nourishment>(entity).unwrap();
        let activity_before = *app
            .world()
            .get::<NourishmentActivityWindow>(entity)
            .unwrap();
        assert_eq!(
            activity_before.observed_flags(),
            (false, true),
            "test setup must provide non-empty session activity before the persistence failure"
        );

        let _failpoint = crate::persistence::arm_fail_before_commit(settings.db_path());

        app.world_mut().send_event(RevivalActionIntent {
            entity,
            action: RevivalActionKind::Reincarnate,
            issued_at_tick: 701,
        });
        app.update();

        let revived_event_count = app.world().resource::<Events<PlayerRevived>>().len();
        let persisted_after = load_persisted_nourishment(&settings, username);
        let persisted_player_after = load_player_slices(&player_persistence, username);
        let connection = Connection::open(settings.db_path()).expect("sqlite should reopen");
        let life_record_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM life_records WHERE char_id = ?1",
                [crate::player::state::canonical_player_id(username)],
                |row| row.get(0),
            )
            .expect("life record count should query");
        let life_event_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM life_events WHERE char_id = ?1",
                [crate::player::state::canonical_player_id(username)],
                |row| row.get(0),
            )
            .expect("life event count should query");

        assert_eq!(
            serde_json::to_value(app.world().get::<Lifecycle>(entity).unwrap())
                .expect("lifecycle snapshot should serialize"),
            lifecycle_before,
            "failed revival persistence must leave lifecycle AwaitingRevival"
        );
        assert_eq!(
            *app.world().get::<Cultivation>(entity).unwrap(),
            cultivation_before,
            "failed revival persistence must not apply the cultivation penalty"
        );
        assert_eq!(
            *app.world().get::<MeridianSystem>(entity).unwrap(),
            meridians_before,
            "failed revival persistence must not mutate meridians"
        );
        assert_eq!(
            serde_json::to_value(app.world().get::<Contamination>(entity).unwrap())
                .expect("contamination snapshot should serialize"),
            contamination_before,
            "failed revival persistence must not mutate contamination"
        );
        assert_eq!(
            serde_json::to_value(app.world().get::<LifeRecord>(entity).unwrap())
                .expect("life record snapshot should serialize"),
            life_record_before,
            "failed revival persistence must not append rebirth biography"
        );
        assert_eq!(
            serde_json::to_value(app.world().get::<Wounds>(entity).unwrap())
                .expect("wounds snapshot should serialize"),
            wounds_before,
            "failed revival persistence must not clear wounds or restore health"
        );
        assert_eq!(
            serde_json::to_value(app.world().get::<Stamina>(entity).unwrap())
                .expect("stamina snapshot should serialize"),
            stamina_before,
            "failed revival persistence must not restore stamina"
        );
        assert_eq!(
            serde_json::to_value(app.world().get::<CombatState>(entity).unwrap())
                .expect("combat snapshot should serialize"),
            combat_before,
            "failed revival persistence must not clear combat state"
        );
        assert_eq!(
            app.world().get::<Position>(entity).unwrap().get(),
            old_position.into(),
            "failed revival persistence must not move the live entity"
        );
        assert_eq!(
            *app.world().get::<Nourishment>(entity).unwrap(),
            nourishment_before,
            "failed revival persistence must not reset satiety or hydration"
        );
        assert_eq!(
            *app.world()
                .get::<NourishmentActivityWindow>(entity)
                .unwrap(),
            activity_before,
            "failed revival persistence must not clear session movement or dash flags"
        );
        assert_eq!(
            revived_event_count, 0,
            "failed revival persistence must not emit PlayerRevived"
        );
        assert_eq!(
            persisted_after, persisted_nourishment,
            "precommit failure must roll back the staged nourishment reset"
        );
        assert_eq!(
            persisted_player_after.position, old_position,
            "precommit failure must roll back the staged shrine/world-spawn position"
        );
        flush_client_packets(&mut app);
        let server_payloads = collect_server_data_payloads(&mut helper);
        assert!(
            server_payloads.iter().all(|payload| !matches!(
                payload.payload,
                ServerDataPayloadV1::DeathScreen { visible: false, .. }
                    | ServerDataPayloadV1::TerminateScreen { visible: false, .. }
            )),
            "failed revival persistence must not hide death or terminate UI"
        );
        assert_eq!(
            app.world().get::<crate::coffin::CoffinComponent>(entity),
            Some(&coffin_before),
            "failed revival persistence must not remove the live CoffinComponent"
        );
        let coffin_registry_after = app.world().resource::<crate::coffin::CoffinRegistry>();
        assert_eq!(
            coffin_registry_after.player_in_coffin.get(&entity),
            Some(&coffin_lower),
            "failed revival persistence must retain the player-to-coffin registry index"
        );
        assert_eq!(
            coffin_registry_after
                .lookup(coffin_lower)
                .expect("registered coffin should remain")
                .occupied_by,
            Some(entity),
            "failed revival persistence must retain coffin occupancy"
        );
        assert_eq!(
            app.world_mut()
                .resource_mut::<Events<crate::coffin::CoffinStateChanged>>()
                .drain()
                .count(),
            0,
            "failed revival persistence must not emit CoffinStateChanged"
        );
        assert!(
            persisted_player_after.in_coffin,
            "precommit failure must retain the persisted coffin flag"
        );
        assert_eq!(
            persisted_player_after.coffin_grade,
            Some(crate::coffin::CoffinGrade::Jade),
            "precommit failure must retain the persisted coffin grade"
        );
        assert_eq!(
            persisted_player_after.last_dimension,
            DimensionKind::Tsy,
            "precommit failure must retain the persisted pre-revival dimension"
        );
        assert_eq!(
            life_record_count, 0,
            "precommit failure must roll back the staged life_records upsert"
        );
        assert_eq!(
            life_event_count, 0,
            "precommit failure must roll back the staged rebirth life_event"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn realm_collapse_death_causes_count_as_death_zone() {
        assert_eq!(
            death_zone_from_context("realm_collapse", None, None),
            ZoneDeathKind::Death
        );
        assert_eq!(
            death_zone_from_context("realm_collapse_entry_lock", None, None),
            ZoneDeathKind::Death
        );
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
                    location: crate::body_plan::legacy_body_part_to_id(BodyPart::Chest),
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
    fn wound_bleed_tick_skips_creative_players() {
        let mut app = App::new();
        app.insert_resource(CombatClock {
            tick: BLEED_TICK_INTERVAL_TICKS,
        });
        app.add_event::<DeathEvent>();
        app.add_systems(Update, wound_bleed_tick);

        let entity = spawn_actor(
            &mut app,
            Wounds {
                health_current: 12.0,
                health_max: 30.0,
                entries: vec![Wound {
                    location: crate::body_plan::legacy_body_part_to_id(BodyPart::Chest),
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
        app.world_mut()
            .entity_mut(entity)
            .insert(GameMode::Creative);

        app.update();

        let wounds = app.world().entity(entity).get::<Wounds>().unwrap();
        assert_eq!(wounds.health_current, 12.0);
        assert_eq!(app.world().resource::<Events<DeathEvent>>().len(), 0);
    }

    #[test]
    fn wound_bleed_tick_uses_latest_game_mode_component() {
        let mut app = App::new();
        app.insert_resource(CombatClock {
            tick: BLEED_TICK_INTERVAL_TICKS,
        });
        app.add_event::<DeathEvent>();
        app.add_systems(Update, wound_bleed_tick);

        let entity = spawn_actor(
            &mut app,
            Wounds {
                health_current: 12.0,
                health_max: 30.0,
                entries: vec![Wound {
                    location: crate::body_plan::legacy_body_part_to_id(BodyPart::Chest),
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

        app.world_mut()
            .entity_mut(entity)
            .insert(GameMode::Survival);
        app.update();
        let after_survival = app
            .world()
            .entity(entity)
            .get::<Wounds>()
            .unwrap()
            .health_current;
        assert_eq!(after_survival, 9.0);

        app.world_mut().resource_mut::<CombatClock>().tick += BLEED_TICK_INTERVAL_TICKS;
        app.world_mut()
            .entity_mut(entity)
            .insert(GameMode::Creative);
        app.update();

        let wounds = app.world().entity(entity).get::<Wounds>().unwrap();
        assert_eq!(
            wounds.health_current, after_survival,
            "switching to Creative must stop residual wound bleed damage"
        );
    }

    #[test]
    fn health_regen_tick_recovers_base_rate() {
        let mut app = App::new();
        app.insert_resource(CombatClock {
            tick: HEALTH_REGEN_TICK_INTERVAL_TICKS,
        });
        app.add_systems(Update, health_regen_tick);

        let entity = spawn_actor(
            &mut app,
            Wounds {
                health_current: 10.0,
                health_max: 30.0,
                entries: Vec::new(),
            },
            Stamina::default(),
            Lifecycle::default(),
        );

        app.update();

        let wounds = app.world().entity(entity).get::<Wounds>().unwrap();
        assert!((wounds.health_current - 10.5).abs() < 1e-6);
    }

    #[test]
    fn health_regen_tick_clamps_at_health_max() {
        let mut app = App::new();
        app.insert_resource(CombatClock {
            tick: HEALTH_REGEN_TICK_INTERVAL_TICKS,
        });
        app.add_systems(Update, health_regen_tick);

        let entity = spawn_actor(
            &mut app,
            Wounds {
                health_current: 29.8,
                health_max: 30.0,
                entries: Vec::new(),
            },
            Stamina::default(),
            Lifecycle::default(),
        );

        app.update();

        let wounds = app.world().entity(entity).get::<Wounds>().unwrap();
        assert_eq!(wounds.health_current, 30.0);
    }

    #[test]
    fn health_regen_tick_skips_zero_full_and_active_bleeding() {
        let mut app = App::new();
        app.insert_resource(CombatClock {
            tick: HEALTH_REGEN_TICK_INTERVAL_TICKS,
        });
        app.add_systems(Update, health_regen_tick);

        let zero_health = spawn_actor(
            &mut app,
            Wounds {
                health_current: 0.0,
                health_max: 30.0,
                entries: Vec::new(),
            },
            Stamina::default(),
            Lifecycle::default(),
        );
        let full_health = spawn_actor(
            &mut app,
            Wounds {
                health_current: 30.0,
                health_max: 30.0,
                entries: Vec::new(),
            },
            Stamina::default(),
            Lifecycle::default(),
        );
        let bleeding = spawn_actor(
            &mut app,
            Wounds {
                health_current: 12.0,
                health_max: 30.0,
                entries: vec![Wound {
                    location: crate::body_plan::legacy_body_part_to_id(BodyPart::Chest),
                    kind: WoundKind::Cut,
                    severity: 0.3,
                    bleeding_per_sec: 0.1,
                    created_at_tick: 0,
                    inflicted_by: None,
                }],
            },
            Stamina::default(),
            Lifecycle::default(),
        );

        app.update();

        assert_eq!(
            app.world()
                .entity(zero_health)
                .get::<Wounds>()
                .unwrap()
                .health_current,
            0.0
        );
        assert_eq!(
            app.world()
                .entity(full_health)
                .get::<Wounds>()
                .unwrap()
                .health_current,
            30.0
        );
        assert_eq!(
            app.world()
                .entity(bleeding)
                .get::<Wounds>()
                .unwrap()
                .health_current,
            12.0
        );
    }

    #[test]
    fn health_regen_tick_skips_pending_revival_lifecycles() {
        let mut app = App::new();
        app.insert_resource(CombatClock {
            tick: HEALTH_REGEN_TICK_INTERVAL_TICKS,
        });
        app.add_systems(Update, health_regen_tick);

        let near_death = spawn_actor(
            &mut app,
            Wounds {
                health_current: 1.0,
                health_max: 30.0,
                entries: Vec::new(),
            },
            Stamina::default(),
            Lifecycle {
                state: LifecycleState::NearDeath,
                ..Lifecycle::default()
            },
        );
        let awaiting_revival = spawn_actor(
            &mut app,
            Wounds {
                health_current: 1.0,
                health_max: 30.0,
                entries: Vec::new(),
            },
            Stamina::default(),
            Lifecycle {
                state: LifecycleState::AwaitingRevival,
                ..Lifecycle::default()
            },
        );
        let terminated = spawn_actor(
            &mut app,
            Wounds {
                health_current: 1.0,
                health_max: 30.0,
                entries: Vec::new(),
            },
            Stamina::default(),
            Lifecycle {
                state: LifecycleState::Terminated,
                ..Lifecycle::default()
            },
        );

        app.update();

        assert_eq!(
            app.world()
                .entity(near_death)
                .get::<Wounds>()
                .unwrap()
                .health_current,
            1.0
        );
        assert_eq!(
            app.world()
                .entity(awaiting_revival)
                .get::<Wounds>()
                .unwrap()
                .health_current,
            1.0
        );
        assert_eq!(
            app.world()
                .entity(terminated)
                .get::<Wounds>()
                .unwrap()
                .health_current,
            1.0
        );
    }

    #[test]
    fn health_regen_tick_multiplies_derived_attrs_and_status_boost() {
        let mut app = App::new();
        app.insert_resource(CombatClock {
            tick: HEALTH_REGEN_TICK_INTERVAL_TICKS,
        });
        app.add_systems(Update, health_regen_tick);

        let entity = spawn_actor(
            &mut app,
            Wounds {
                health_current: 10.0,
                health_max: 30.0,
                entries: Vec::new(),
            },
            Stamina::default(),
            Lifecycle::default(),
        );
        app.world_mut().entity_mut(entity).insert((
            DerivedAttrs {
                healing_rate_multiplier: 1.5,
                ..DerivedAttrs::default()
            },
            StatusEffects {
                active: vec![ActiveStatusEffect {
                    kind: StatusEffectKind::HealthRegenBoost,
                    magnitude: 0.5,
                    remaining_ticks: 100,
                    source_pill: None,
                }],
            },
        ));

        app.update();

        let wounds = app.world().entity(entity).get::<Wounds>().unwrap();
        assert!((wounds.health_current - 11.125).abs() < 1e-6);
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
            source: crate::combat::events::AttackSource::Melee,
            debug_command: false,
            physical_damage: 0.0,
            damage: 3.0,
            contam_delta: 0.75,
            description: "hit".to_string(),
            defense_kind: None,
            defense_effectiveness: None,
            defense_contam_reduced: None,
            defense_wound_severity: None,
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
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_systems(Update, crate::combat::resolve::apply_defense_intents);

        let entity = spawn_actor(
            &mut app,
            Wounds::default(),
            Stamina::default(),
            Lifecycle::default(),
        );
        app.world_mut().entity_mut(entity).insert((
            Cultivation {
                realm: crate::cultivation::components::Realm::Induce,
                qi_current: 10.0,
                qi_max: 10.0,
                ..Cultivation::default()
            },
            StatusEffects::default(),
        ));

        app.world_mut().send_event(DefenseIntent {
            defender: entity,
            issued_at_tick: 42,
        });
        app.update();

        let state = app.world().entity(entity).get::<CombatState>().unwrap();
        let window = state.incoming_window.as_ref().expect("window should open");
        assert_eq!(window.opened_at_tick, 42);
        assert_eq!(window.duration_ms, QI_ZHENMAI_PREP_WINDOW_MS);
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
        app.add_event::<DeathCinematicPublished>();
        app.add_event::<PlayerRevived>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<RevivalActionIntent>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<crate::coffin::CoffinStateChanged>();
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
    fn tsy_collapsed_death_keeps_standard_fortune_revival_decision() {
        let mut lifecycle = Lifecycle {
            fortune_remaining: 1,
            ..Default::default()
        };
        lifecycle.enter_near_death(100);

        let decision = determine_revival_decision(
            &lifecycle,
            None,
            "tsy_collapsed",
            None,
            None,
            None,
            None,
            701,
        );

        assert!(matches!(
            decision,
            Some(RevivalDecision::Fortune { chance }) if (chance - 1.0).abs() < f64::EPSILON
        ));
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
        app.add_event::<DeathCinematicPublished>();
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
    fn death_arbiter_skips_death_event_reentry_while_awaiting_revival() {
        // bughunt 实证：污染溢出持续触发 DeathEvent，AwaitingRevival（死亡屏，60s 确认窗口）
        // 期间如果被新死亡事件拍回 NearDeath，窗口实际只活 1 tick，玩家永远点不中重生。
        // pin 住：死亡屏等待决策期间的死亡事件必须被 continue 跳过，不触碰任何状态。
        let mut app = App::new();
        let (settings, root) = persistence_settings("awaiting-revival-skip-death-event");
        app.insert_resource(settings);
        app.insert_resource(CombatClock { tick: 900 });
        app.add_event::<DeathEvent>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<DeathInsightRequested>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, death_arbiter_tick);

        let mut life_record = LifeRecord::default();
        life_record.push(BiographyEntry::NearDeath {
            cause: "prior".to_string(),
            tick: 100,
        });
        let biography_len_before = life_record.biography.len();

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
                life_record,
                Lifecycle {
                    state: LifecycleState::AwaitingRevival,
                    awaiting_decision: Some(RevivalDecision::Fortune { chance: 1.0 }),
                    near_death_deadline_tick: None,
                    revival_decision_deadline_tick: Some(999),
                    death_count: 1,
                    ..Default::default()
                },
            ))
            .id();

        app.world_mut().send_event(DeathEvent {
            target: entity,
            cause: "contamination_overflow".to_string(),
            attacker: None,
            attacker_player_id: None,
            at_tick: 900,
        });
        app.update();

        let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
        assert_eq!(
            lifecycle.state,
            LifecycleState::AwaitingRevival,
            "期望仍是 AwaitingRevival 因为死亡屏等待决策期间不应接受新死亡事件重入把状态拍回 NearDeath；实际 {:?}",
            lifecycle.state
        );
        assert_eq!(
            lifecycle.near_death_deadline_tick, None,
            "期望 near_death_deadline_tick 保持 None 因为守卫应在触碰任何字段前 continue，不应被 enter_near_death 重新设置；实际 {:?}",
            lifecycle.near_death_deadline_tick
        );
        assert_eq!(
            lifecycle.revival_decision_deadline_tick,
            Some(999),
            "期望死亡屏 60s 确认窗口 deadline 保持不变（不被新死亡事件打断/重置）；实际 {:?}",
            lifecycle.revival_decision_deadline_tick
        );
        assert_eq!(
            lifecycle.death_count, 1,
            "期望 death_count 不因重入死亡事件而递增；实际 {}",
            lifecycle.death_count
        );

        let life_record = app.world().entity(entity).get::<LifeRecord>().unwrap();
        assert_eq!(
            life_record.biography.len(),
            biography_len_before,
            "期望 biography 不新增 NearDeath 条目因为守卫应在 push 之前 continue；实际长度 {}",
            life_record.biography.len()
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn death_arbiter_tick_auto_releases_morph_state_on_death() {
        // plan-race-system-v1 P4 opus verifier MAJOR — 死亡三条易形自动解除触发路径
        // 之一（见 death_arbiter_tick 内 release_morph_state deferred command）此前
        // 零测试断言真被 remove。走真实事件 → 真实 system → 真实 Commands flush
        // （单次 app.update() 后 Bevy 自动 apply_deferred，见 `combat::lifecycle`
        // 模块内其余测试同款依赖 —— DeathDropAnchor 断言同一 tick 内可见的既有惯例）。
        let mut app = App::new();
        let (settings, root) = persistence_settings("morph-auto-release-death");
        app.insert_resource(settings);
        app.insert_resource(CombatClock { tick: 950 });
        app.add_event::<DeathEvent>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<DeathInsightRequested>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, death_arbiter_tick);

        let entity = app
            .world_mut()
            .spawn((
                crate::body_plan::MorphState::new(crate::body_plan::RaceId::new("whale"), 0, 900),
                Wounds {
                    health_current: 0.0,
                    health_max: 30.0,
                    entries: Vec::new(),
                },
                Stamina::default(),
                CombatState::default(),
                Lifecycle::default(),
            ))
            .id();

        assert!(
            app.world()
                .entity(entity)
                .get::<crate::body_plan::MorphState>()
                .is_some(),
            "前置条件：死亡前应处于易形态"
        );

        app.world_mut().send_event(DeathEvent {
            target: entity,
            cause: "test_death".to_string(),
            attacker: None,
            attacker_player_id: None,
            at_tick: 950,
        });
        app.update();

        assert!(
            app.world()
                .entity(entity)
                .get::<crate::body_plan::MorphState>()
                .is_none(),
            "死亡应通过 release_morph_state 的 deferred command 移除 MorphState \
             （单次 app.update() 后 Commands 已 flush），实测组件仍在场"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn death_arbiter_skips_cultivation_death_trigger_reentry_while_awaiting_revival() {
        // 同上，覆盖 cultivation_deaths 事件循环的守卫（第二处跳过点，独立于 DeathEvent 路径）。
        let mut app = App::new();
        let (settings, root) = persistence_settings("awaiting-revival-skip-cultivation-trigger");
        app.insert_resource(settings);
        app.insert_resource(CombatClock { tick: 900 });
        app.add_event::<DeathEvent>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<DeathInsightRequested>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, death_arbiter_tick);

        let mut life_record = LifeRecord::default();
        life_record.push(BiographyEntry::NearDeath {
            cause: "prior".to_string(),
            tick: 100,
        });
        let biography_len_before = life_record.biography.len();

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
                life_record,
                Lifecycle {
                    state: LifecycleState::AwaitingRevival,
                    awaiting_decision: Some(RevivalDecision::Tribulation { chance: 0.5 }),
                    near_death_deadline_tick: None,
                    revival_decision_deadline_tick: Some(1500),
                    death_count: 2,
                    ..Default::default()
                },
            ))
            .id();

        app.world_mut().send_event(CultivationDeathTrigger {
            entity,
            cause: CultivationDeathCause::NegativeZoneDrain,
            context: serde_json::json!({"zone": "rift_valley"}),
        });
        app.update();

        let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
        assert_eq!(
            lifecycle.state,
            LifecycleState::AwaitingRevival,
            "期望仍是 AwaitingRevival 因为死亡屏等待决策期间不应接受新 cultivation 死亡事件重入；实际 {:?}",
            lifecycle.state
        );
        assert_eq!(
            lifecycle.near_death_deadline_tick, None,
            "期望 near_death_deadline_tick 保持 None，守卫应在 enter_near_death 之前 continue；实际 {:?}",
            lifecycle.near_death_deadline_tick
        );
        assert_eq!(
            lifecycle.revival_decision_deadline_tick,
            Some(1500),
            "期望死亡屏确认窗口 deadline 不被新 cultivation 死亡事件重置；实际 {:?}",
            lifecycle.revival_decision_deadline_tick
        );
        assert_eq!(
            lifecycle.death_count, 2,
            "期望 death_count 不因重入 cultivation 死亡事件而递增；实际 {}",
            lifecycle.death_count
        );

        let life_record = app.world().entity(entity).get::<LifeRecord>().unwrap();
        assert_eq!(
            life_record.biography.len(),
            biography_len_before,
            "期望 biography 不新增 NearDeath 条目因为守卫应在 push 之前 continue；实际长度 {}",
            life_record.biography.len()
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn near_death_stabilization_preserves_nourishment_and_activity_window() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("near-death-nourishment-preserve");
        app.insert_resource(settings);
        app.insert_resource(CombatClock { tick: 100 });
        app.add_event::<DeathEvent>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<DeathCinematicPublished>();
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

        let expected_nourishment = Nourishment {
            satiety: 37.0,
            hydration: 46.0,
        };
        let expected_activity = NourishmentActivityWindow::default();
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
                LifeRecord::new("offline:Stable"),
                Lifecycle::default(),
                expected_nourishment,
                expected_activity,
            ))
            .id();

        app.world_mut().send_event(DeathEvent {
            target: entity,
            cause: "test_stabilization".to_string(),
            attacker: None,
            attacker_player_id: None,
            at_tick: 100,
        });
        app.update();
        assert_eq!(
            app.world().get::<Lifecycle>(entity).unwrap().state,
            LifecycleState::NearDeath,
            "the production death arbiter should enter NearDeath before healing"
        );

        app.world_mut()
            .get_mut::<Wounds>(entity)
            .expect("near-death actor should retain wounds")
            .health_current = 2.0;
        app.world_mut().resource_mut::<CombatClock>().tick = 101;
        app.update();

        let lifecycle = app.world().get::<Lifecycle>(entity).unwrap();
        assert_eq!(lifecycle.state, LifecycleState::Alive);
        assert_eq!(lifecycle.near_death_deadline_tick, None);
        assert_eq!(
            *app.world().get::<Nourishment>(entity).unwrap(),
            expected_nourishment,
            "stabilizing above the strict five-percent threshold is not a formal revival and must preserve both axes"
        );
        assert_eq!(
            *app.world()
                .get::<NourishmentActivityWindow>(entity)
                .unwrap(),
            expected_activity,
            "stabilization must preserve every accumulated activity tick"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn nourishment_sweep_runs_before_revival_reset_on_the_same_boundary_tick() {
        // plan-satiety-hydration-v1 §5：全局 CombatClock 扫荡必须排在
        // handle_revival_action_intents 之前，否则一个恰好落在 200-tick 边界上的
        // 正式复活会先被 revive_lifecycle 重置到 80/80，再被同一 Update 内的扫荡
        // 顺手扣掉一份闲置损耗——玩家复活的瞬间饱食/水分就低于满值。
        // 用一个远离 80/80 的复活前脏值验证:若顺序正确，扫荡只会处理复活前的旧值，
        // 随后 revive_lifecycle 无条件覆写为精确的 spawn_default；若顺序颠倒，最终值
        // 会是 80 减去一份闲置 sweep 损耗，与 spawn_default 精确相等的断言就会撞红。
        let mut app = App::new();
        let (settings, root) = persistence_settings("nourishment-sweep-before-revival");
        let username = "SweepBeforeRevival";
        let cultivation = Cultivation::default();
        let meridians = MeridianSystem::default();
        let contamination = Contamination::default();
        let life_record = LifeRecord::new(canonical_player_id(username));
        seed_revival_nourishment_bundle(
            &settings,
            username,
            &cultivation,
            &meridians,
            &contamination,
            &life_record,
            Nourishment {
                satiety: 12.0,
                hydration: 8.0,
            },
        );
        let player_persistence = PlayerStatePersistence::with_db_path(
            settings
                .db_path()
                .parent()
                .unwrap_or_else(|| std::path::Path::new(".")),
            settings.db_path(),
        );
        app.insert_resource(settings);
        app.insert_resource(player_persistence);
        app.insert_resource(CombatClock {
            tick: u64::from(crate::nourishment::NOURISH_SWEEP_INTERVAL_TICKS),
        });
        app.add_event::<RevivalActionIntent>();
        app.add_event::<PlayerRevived>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<crate::coffin::CoffinStateChanged>();
        crate::nourishment::register(&mut app);
        app.add_systems(Update, handle_revival_action_intents);

        let (entity, _helper) = spawn_revival_action_actor(
            &mut app,
            username,
            RevivalActionActorState {
                lifecycle: Lifecycle {
                    state: LifecycleState::AwaitingRevival,
                    awaiting_decision: Some(RevivalDecision::Fortune { chance: 1.0 }),
                    fortune_remaining: 1,
                    ..Default::default()
                },
                cultivation,
                meridians,
                contamination,
                life_record,
                nourishment: Nourishment {
                    satiety: 12.0,
                    hydration: 8.0,
                },
            },
        );

        app.world_mut().send_event(RevivalActionIntent {
            entity,
            action: RevivalActionKind::Reincarnate,
            issued_at_tick: u64::from(crate::nourishment::NOURISH_SWEEP_INTERVAL_TICKS),
        });
        app.update();

        assert_eq!(
            app.world().get::<Lifecycle>(entity).unwrap().state,
            LifecycleState::Alive,
            "the Fortune decision must have revived the actor on this Update"
        );
        assert_eq!(
            *app.world().get::<Nourishment>(entity).unwrap(),
            Nourishment::spawn_default(),
            "the sweep must settle the pre-revival window first so revive_lifecycle's \
             unconditional reset lands exactly on spawn_default with no trailing sweep \
             deduction from this same Update"
        );
        assert_eq!(
            *app.world()
                .get::<NourishmentActivityWindow>(entity)
                .unwrap(),
            NourishmentActivityWindow::default(),
            "revival must leave a fresh, empty session activity window"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn death_loop_full_cycle_reentrant_death_event_does_not_block_reincarnate() {
        // 回归场景：进入 NearDeath → 快进过 deadline 让 near_death_tick 判定出 AwaitingRevival →
        // 再灌一条同 cause 死亡事件（模拟污染溢出持续触发）→ 状态不应被拍回 NearDeath →
        // 玩家送 Reincarnate 决策 → 必须能正常复活（state == Alive）。
        // 这是 Bug 1 的整链路回归：修复前，重入死亡事件会把状态踢回 NearDeath，
        // Reincarnate intent 因 `lifecycle.state != AwaitingRevival` 被静默丢弃，玩家永远点不中重生。
        let mut app = App::new();
        let (settings, root) = persistence_settings("death-loop-full-cycle");
        let player_persistence = PlayerStatePersistence::with_db_path(
            settings
                .db_path()
                .parent()
                .unwrap_or_else(|| std::path::Path::new(".")),
            settings.db_path(),
        );
        app.insert_resource(settings);
        app.insert_resource(player_persistence);
        app.insert_resource(CombatClock { tick: 100 });
        app.add_event::<DeathEvent>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<DeathInsightRequested>();
        app.add_event::<DeathCinematicPublished>();
        app.add_event::<PlayerRevived>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<RevivalActionIntent>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<crate::coffin::CoffinStateChanged>();
        app.add_systems(
            Update,
            (
                death_arbiter_tick,
                near_death_tick.after(death_arbiter_tick),
                handle_revival_action_intents.after(near_death_tick),
            ),
        );

        let (entity, _helper) = spawn_client_actor(
            &mut app,
            "Loopy",
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
        app.world_mut().entity_mut(entity).insert((
            Cultivation::default(),
            MeridianSystem::default(),
            Contamination::default(),
            crate::cultivation::components::QiColor::default(),
            crate::cultivation::components::Karma::default(),
            crate::cultivation::color::PracticeLog::default(),
            crate::cultivation::insight::InsightQuota::default(),
            crate::cultivation::insight_apply::UnlockedPerceptions::default(),
            crate::cultivation::insight_apply::InsightModifiers::new(),
            crate::cultivation::meridian::severed::MeridianSeveredPermanent::default(),
        ));

        // 首次死亡事件：Alive → NearDeath。
        app.world_mut().send_event(DeathEvent {
            target: entity,
            cause: "contamination_overflow".to_string(),
            attacker: None,
            attacker_player_id: None,
            at_tick: 100,
        });
        app.update();
        {
            let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
            assert_eq!(
                lifecycle.state,
                LifecycleState::NearDeath,
                "期望首次死亡事件后进入 NearDeath；实际 {:?}",
                lifecycle.state
            );
        }

        // 快进过 NEAR_DEATH_WINDOW（600 ticks）→ near_death_tick 应判定出 AwaitingRevival。
        app.world_mut().resource_mut::<CombatClock>().tick = 701;
        app.update();
        {
            let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
            assert_eq!(
                lifecycle.state,
                LifecycleState::AwaitingRevival,
                "期望濒死窗口期满后进入 AwaitingRevival（死亡屏）；实际 {:?}",
                lifecycle.state
            );
        }

        // 实证场景：污染溢出在死亡屏挂起期间又触发一条同 cause 死亡事件（下一 tick，601 之后每 601 tick 重入）。
        app.world_mut().send_event(DeathEvent {
            target: entity,
            cause: "contamination_overflow".to_string(),
            attacker: None,
            attacker_player_id: None,
            at_tick: 702,
        });
        app.update();
        {
            let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
            assert_eq!(
                lifecycle.state,
                LifecycleState::AwaitingRevival,
                "期望重入死亡事件后状态仍是 AwaitingRevival（未被拍回 NearDeath）——这是 Bug 1 的核心断言；实际 {:?}",
                lifecycle.state
            );
        }

        // 玩家送出 Reincarnate 决策：必须成功复活，而不是因状态已被重入死亡事件破坏而被
        // `lifecycle.state != AwaitingRevival` 静默丢弃。
        app.world_mut().send_event(RevivalActionIntent {
            entity,
            action: RevivalActionKind::Reincarnate,
            issued_at_tick: 703,
        });
        app.update();

        let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
        assert_eq!(
            lifecycle.state,
            LifecycleState::Alive,
            "期望 Reincarnate 决策后成功复活为 Alive 因为死亡屏窗口本应完整存活直到玩家决策；实际 {:?}",
            lifecycle.state
        );
        let revived_events = app.world().resource::<Events<PlayerRevived>>();
        assert_eq!(
            revived_events.len(),
            1,
            "期望恰好一次 PlayerRevived 事件；实际 {}",
            revived_events.len()
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn near_death_npc_termination_keeps_high_realm_qi_burst_profile() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("npc-near-death-vfx");
        app.insert_resource(settings);
        app.insert_resource(CombatClock { tick: 200 });
        app.add_event::<PlayerRevived>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<DeathCinematicPublished>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, near_death_tick);

        let profile = crate::skin::select_npc_visual_profile(
            crate::npc::lifecycle::NpcArchetype::Rogue,
            Realm::Spirit,
            None,
            None,
            0.5,
        );
        let entity = app
            .world_mut()
            .spawn((
                Lifecycle {
                    character_id: "npc_high_realm".to_string(),
                    state: LifecycleState::NearDeath,
                    near_death_deadline_tick: Some(199),
                    ..Default::default()
                },
                LifeRecord::new("npc_high_realm"),
                Position::new([0.0, 66.0, 0.0]),
                NpcMarker,
                profile,
            ))
            .id();

        app.update();

        let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
        assert_eq!(lifecycle.state, LifecycleState::Terminated);
        assert_eq!(app.world().resource::<Events<PlayerTerminated>>().len(), 1);

        let vfx_events = app.world().resource::<Events<VfxEventRequest>>();
        let mut reader = vfx_events.get_reader();
        let event_ids = reader
            .read(vfx_events)
            .filter_map(|request| match &request.payload {
                VfxEventPayloadV1::SpawnParticle { event_id, .. } => Some(event_id.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(
            event_ids.contains(&"bong:npc_death_smoke"),
            "terminating an NPC through near-death should emit death smoke"
        );
        assert!(
            event_ids.contains(&"bong:npc_death_qi_burst"),
            "high-realm NPC profile should survive the near-death wrapper and emit qi burst"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn near_death_rat_terminates_without_waiting_for_player_revival_window() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("rat-near-death-immediate");
        app.insert_resource(settings);
        app.insert_resource(CombatClock { tick: 200 });
        app.add_event::<PlayerRevived>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<DeathCinematicPublished>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, near_death_tick);

        let entity = app
            .world_mut()
            .spawn((
                Lifecycle {
                    character_id: "rat-immediate".to_string(),
                    state: LifecycleState::NearDeath,
                    near_death_deadline_tick: Some(
                        200 + crate::combat::components::NEAR_DEATH_WINDOW_TICKS,
                    ),
                    ..Default::default()
                },
                Wounds {
                    health_current: 0.0,
                    health_max: 100.0,
                    entries: Vec::new(),
                },
                Position::new([0.0, 66.0, 0.0]),
                NpcMarker,
                crate::fauna::components::FaunaTag::new(crate::fauna::components::BeastKind::Rat),
            ))
            .id();

        app.update();

        let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
        assert_eq!(lifecycle.state, LifecycleState::Terminated);
        assert_eq!(
            app.world().resource::<Events<PlayerTerminated>>().len(),
            1,
            "rat should not wait out the player revival near-death window"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn near_death_non_rat_npc_terminates_immediately() {
        // All NPCs skip the NearDeath wait window — only players use it.
        let mut app = App::new();
        let (settings, root) = persistence_settings("spider-near-death-immediate");
        app.insert_resource(settings);
        app.insert_resource(CombatClock { tick: 200 });
        app.add_event::<PlayerRevived>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<DeathCinematicPublished>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, near_death_tick);

        let entity = app
            .world_mut()
            .spawn((
                Lifecycle {
                    character_id: "spider-immediate".to_string(),
                    state: LifecycleState::NearDeath,
                    near_death_deadline_tick: Some(
                        200 + crate::combat::components::NEAR_DEATH_WINDOW_TICKS,
                    ),
                    ..Default::default()
                },
                Wounds {
                    health_current: 0.0,
                    health_max: 100.0,
                    entries: Vec::new(),
                },
                Position::new([0.0, 66.0, 0.0]),
                NpcMarker,
                crate::fauna::components::FaunaTag::new(
                    crate::fauna::components::BeastKind::Spider,
                ),
            ))
            .id();

        app.update();

        let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
        assert_eq!(lifecycle.state, LifecycleState::Terminated);
        assert_eq!(
            app.world().resource::<Events<PlayerTerminated>>().len(),
            1,
            "all NPCs should terminate immediately without near-death wait"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn near_death_rat_without_npc_marker_waits_for_deadline() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("rat-without-npc-marker-waits");
        app.insert_resource(settings);
        app.insert_resource(CombatClock { tick: 200 });
        app.add_event::<PlayerRevived>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<DeathCinematicPublished>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, near_death_tick);

        let entity = app
            .world_mut()
            .spawn((
                Lifecycle {
                    character_id: "rat-without-npc-marker".to_string(),
                    state: LifecycleState::NearDeath,
                    near_death_deadline_tick: Some(
                        200 + crate::combat::components::NEAR_DEATH_WINDOW_TICKS,
                    ),
                    ..Default::default()
                },
                Wounds {
                    health_current: 0.0,
                    health_max: 100.0,
                    entries: Vec::new(),
                },
                Position::new([0.0, 66.0, 0.0]),
                crate::fauna::components::FaunaTag::new(crate::fauna::components::BeastKind::Rat),
            ))
            .id();

        app.update();

        let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
        assert_eq!(lifecycle.state, LifecycleState::NearDeath);
        assert_eq!(
            app.world().resource::<Events<PlayerTerminated>>().len(),
            0,
            "rat tag without NpcMarker should not use NPC immediate termination path"
        );

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
        app.add_event::<DeathCinematicPublished>();
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
    fn death_arbiter_clears_status_effects_on_near_death() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 10 });
        let (settings, root) = persistence_settings("death-clears-status-effects");
        app.insert_resource(settings);
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
        app.world_mut().entity_mut(entity).insert(StatusEffects {
            active: vec![ActiveStatusEffect {
                kind: StatusEffectKind::Bleeding,
                magnitude: 1.0,
                remaining_ticks: 120,
                source_pill: None,
            }],
        });

        app.world_mut().send_event(DeathEvent {
            target: entity,
            cause: "bleed_out".to_string(),
            attacker: None,
            attacker_player_id: None,
            at_tick: 10,
        });
        app.update();

        let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
        assert_eq!(lifecycle.state, LifecycleState::NearDeath);
        let statuses = app.world().entity(entity).get::<StatusEffects>().unwrap();
        assert!(statuses.active.is_empty());

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
        app.add_event::<DeathCinematicPublished>();
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
        app.add_event::<DeathCinematicPublished>();
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
    fn void_quota_exceeded_cultivation_death_terminates_without_lifespan_penalty() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("void-quota-exceeded");
        app.insert_resource(settings.clone());
        app.insert_resource(CombatClock { tick: 300 });
        app.add_event::<DeathEvent>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<DeathInsightRequested>();
        app.add_event::<DeathCinematicPublished>();
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
                    character_id: "offline:Azure".to_string(),
                    ..Default::default()
                },
                Cultivation {
                    realm: Realm::Spirit,
                    ..Default::default()
                },
                LifeRecord::new("offline:Azure"),
                DeathRegistry::new("offline:Azure"),
                LifespanComponent {
                    born_at_tick: 0,
                    years_lived: 80.0,
                    cap_by_realm: LifespanCapTable::SPIRIT,
                    offline_pause_tick: None,
                },
                Position::new([0.0, 66.0, 0.0]),
            ))
            .id();

        app.world_mut().send_event(CultivationDeathTrigger {
            entity,
            cause: CultivationDeathCause::VoidQuotaExceeded,
            context: serde_json::json!({"reason": "void_quota_exceeded"}),
        });
        app.update();

        let lifecycle = app.world().entity(entity).get::<Lifecycle>().unwrap();
        assert_eq!(lifecycle.state, LifecycleState::Terminated);
        assert_eq!(lifecycle.death_count, 1);
        assert_eq!(lifecycle.last_death_tick, Some(300));
        let life_record = app.world().entity(entity).get::<LifeRecord>().unwrap();
        assert!(matches!(
            life_record.biography.last(),
            Some(BiographyEntry::Terminated { cause, tick })
                if cause == crate::cultivation::tribulation::VOID_QUOTA_EXCEEDED_REASON
                    && *tick == 300
        ));
        let lifespan = app
            .world()
            .entity(entity)
            .get::<LifespanComponent>()
            .expect("lifespan should remain attached");
        assert_eq!(lifespan.years_lived, 80.0);
        assert_eq!(app.world().resource::<Events<PlayerTerminated>>().len(), 1);

        let insight_events = app.world().resource::<Events<DeathInsightRequested>>();
        let mut insight_reader = insight_events.get_reader();
        let insights: Vec<_> = insight_reader.read(insight_events).cloned().collect();
        assert_eq!(insights.len(), 1);
        let payload = &insights[0].payload;
        assert_eq!(payload.character_id, "offline:Azure");
        assert_eq!(payload.cause, "cultivation:VoidQuotaExceeded");
        assert_eq!(payload.category, DeathInsightCategoryV1::Cultivation);
        assert_eq!(payload.context["will_terminate"], true);

        let connection = Connection::open(settings.db_path()).expect("db should open");
        let death_registry: (i64, i64, String) = connection
            .query_row(
                "SELECT death_count, last_death_tick, last_death_cause FROM death_registry WHERE char_id = ?1",
                params!["offline:Azure"],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("void-quota death should persist death registry");
        let lifespan_events: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM lifespan_events WHERE char_id = ?1 AND event_type = 'death_penalty'",
                params!["offline:Azure"],
                |row| row.get(0),
            )
            .expect("lifespan event count should be readable");
        assert_eq!(
            death_registry,
            (1, 300, "cultivation:VoidQuotaExceeded".to_string())
        );
        assert_eq!(lifespan_events, 0);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn void_action_backlash_records_dedicated_termination_cause() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("void-action-backlash");
        app.insert_resource(settings);
        app.insert_resource(CombatClock { tick: 320 });
        app.add_event::<DeathEvent>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<DeathInsightRequested>();
        app.add_event::<DeathCinematicPublished>();
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
                    character_id: "offline:Void".to_string(),
                    ..Default::default()
                },
                Cultivation {
                    realm: Realm::Void,
                    ..Default::default()
                },
                LifeRecord::new("offline:Void"),
                DeathRegistry::new("offline:Void"),
                LifespanComponent {
                    born_at_tick: 0,
                    years_lived: LifespanCapTable::VOID as f64,
                    cap_by_realm: LifespanCapTable::VOID,
                    offline_pause_tick: None,
                },
                Position::new([0.0, 66.0, 0.0]),
            ))
            .id();

        app.world_mut().send_event(CultivationDeathTrigger {
            entity,
            cause: CultivationDeathCause::VoidActionBacklash,
            context: serde_json::json!({"kind": "barrier"}),
        });
        app.update();

        let life_record = app.world().entity(entity).get::<LifeRecord>().unwrap();
        assert!(matches!(
            life_record.biography.last(),
            Some(BiographyEntry::Terminated { cause, tick })
                if cause == "void_action_backlash" && *tick == 320
        ));

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
        app.add_event::<DeathCinematicPublished>();
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
        app.add_event::<DeathCinematicPublished>();
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
        let player_persistence = PlayerStatePersistence::with_db_path(
            settings
                .db_path()
                .parent()
                .unwrap_or_else(|| std::path::Path::new(".")),
            settings.db_path(),
        );
        app.insert_resource(settings.clone());
        app.insert_resource(player_persistence);
        app.insert_resource(CombatClock { tick: 90 });
        app.insert_resource(CultivationClock { tick: 691 });
        app.add_event::<DeathEvent>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<DeathInsightRequested>();
        app.add_event::<DeathCinematicPublished>();
        app.add_event::<PlayerRevived>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<RevivalActionIntent>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<crate::skill::events::SkillCapChanged>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<crate::coffin::CoffinStateChanged>();
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
                crate::cultivation::components::QiColor::default(),
                crate::cultivation::components::Karma::default(),
                crate::cultivation::color::PracticeLog::default(),
                crate::cultivation::components::Contamination::default(),
                LifeRecord::new("offline:Ancestor"),
                crate::cultivation::insight::InsightQuota::default(),
                crate::cultivation::insight_apply::UnlockedPerceptions::default(),
                crate::cultivation::insight_apply::InsightModifiers::new(),
                Username("Ancestor".to_string()),
            ))
            .id();
        app.world_mut().entity_mut(entity).insert((
            Nourishment {
                satiety: 7.0,
                hydration: 8.0,
            },
            NourishmentActivityWindow::default(),
            crate::cultivation::meridian::severed::MeridianSeveredPermanent::default(),
            Position::new([8.0, 66.0, 8.0]),
        ));

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
        assert_eq!(
            *app.world()
                .entity(entity)
                .get::<Nourishment>()
                .expect("reincarnated player should retain nourishment component"),
            Nourishment::spawn_default(),
            "formal Reincarnate must reset both nourishment axes to 80/80"
        );
        assert_eq!(
            *app.world()
                .entity(entity)
                .get::<NourishmentActivityWindow>()
                .expect("reincarnated player should retain activity window"),
            NourishmentActivityWindow::default(),
            "formal Reincarnate must clear every accumulated activity tick"
        );
        assert_eq!(
            load_persisted_nourishment(&settings, "Ancestor"),
            Nourishment::spawn_default(),
            "formal Reincarnate must persist reset axes without serializing session activity"
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
    fn void_revival_releases_ascension_quota() {
        let (settings, root) = persistence_settings("void-revival-release-quota");
        persist_active_tribulation(
            &settings,
            &ActiveTribulationRecord {
                char_id: "offline:VoidWalker".to_string(),
                kind: "du_xu".to_string(),
                source: String::new(),
                origin_dimension: Some("minecraft:overworld".to_string()),
                wave_current: 3,
                waves_total: 3,
                started_tick: 10,
                epicenter: [0.0, 64.0, 0.0],
                intensity: 0.0,
            },
        )
        .expect("active DuXu should persist before quota setup");
        complete_tribulation_ascension(&settings, "offline:VoidWalker")
            .expect("quota setup should succeed");
        let mut app = revival_action_test_app(settings.clone(), 700);
        let (entity, _helper) = spawn_revival_action_actor(
            &mut app,
            "VoidWalker",
            RevivalActionActorState {
                lifecycle: Lifecycle {
                    character_id: "offline:VoidWalker".to_string(),
                    state: LifecycleState::AwaitingRevival,
                    awaiting_decision: Some(RevivalDecision::Fortune { chance: 1.0 }),
                    revival_decision_deadline_tick: Some(800),
                    fortune_remaining: 1,
                    ..Default::default()
                },
                cultivation: Cultivation {
                    realm: Realm::Void,
                    qi_current: 12.0,
                    qi_max: 240.0,
                    ..Default::default()
                },
                meridians: MeridianSystem::default(),
                contamination: Contamination::default(),
                life_record: LifeRecord::new("offline:VoidWalker"),
                nourishment: Nourishment::spawn_default(),
            },
        );

        app.world_mut().send_event(RevivalActionIntent {
            entity,
            action: RevivalActionKind::Reincarnate,
            issued_at_tick: 700,
        });
        app.update();

        let cultivation = app
            .world()
            .get::<Cultivation>(entity)
            .expect("cultivation should remain attached");
        assert_eq!(cultivation.realm, Realm::Spirit);
        let quota = load_ascension_quota(&settings).expect("quota load should succeed");
        assert_eq!(quota.occupied_slots, 0);
        let quota_events: Vec<_> = app
            .world_mut()
            .resource_mut::<Events<AscensionQuotaOpened>>()
            .drain()
            .collect();
        assert_eq!(quota_events.len(), 1);
        assert_eq!(quota_events[0].occupied_slots, 0);

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
        app.add_event::<DeathCinematicPublished>();
        app.add_event::<PlayerRevived>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<RevivalActionIntent>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<crate::coffin::CoffinStateChanged>();
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
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<crate::coffin::CoffinStateChanged>();
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
        app.add_event::<DeathCinematicPublished>();
        app.add_event::<PlayerRevived>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<RevivalActionIntent>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<crate::coffin::CoffinStateChanged>();
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

    // ── bughunt player-lifecycle-relog-death-consequence-wipe（OPUS 返工要求 2）──
    //
    // 断线时正处于 AwaitingRevival 的角色重连后必须重新收到死亡屏 + DeathCinematic，不能
    // 让玩家满血、无 UI 地"裸奔"在这个阻断攻防、又会被 auto_confirm_revival_decisions
    // 强制结算（可能永久终结角色）的状态里。下面的用例覆盖：两个 RevivalDecision 变体各一条
    // 专属 case（happy path）、NearDeath/Alive 两个不该触发的状态（负分支）、
    // awaiting_decision=None 的内部不一致状态（错误分支，不panic）、以及
    // Without<DeathCinematic> 过滤器的防重复触发保护。

    fn spawn_reconnected_client_actor(
        app: &mut App,
        username: &str,
        lifecycle: Lifecycle,
    ) -> (Entity, MockClientHelper) {
        spawn_client_actor(
            app,
            username,
            Wounds {
                health_current: 30.0,
                health_max: 30.0,
                entries: Vec::new(),
            },
            Stamina::default(),
            lifecycle,
        )
    }

    #[test]
    fn reconnect_while_awaiting_revival_tribulation_reemits_death_screen_and_cinematic() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 500 });
        app.add_event::<DeathCinematicPublished>();
        app.add_systems(
            Update,
            reemit_death_screen_for_reconnected_awaiting_revival_clients,
        );

        let (entity, mut helper) = spawn_reconnected_client_actor(
            &mut app,
            "ReconnectTribulation",
            Lifecycle {
                character_id: "offline:ReconnectTribulation".to_string(),
                death_count: 2,
                fortune_remaining: 0,
                state: LifecycleState::AwaitingRevival,
                awaiting_decision: Some(RevivalDecision::Tribulation { chance: 0.2 }),
                revival_decision_deadline_tick: Some(560),
                ..Default::default()
            },
        );

        app.update();
        flush_client_packets(&mut app);

        let payloads = collect_server_data_payloads(&mut helper);
        assert!(
            payloads.iter().any(|payload| matches!(
                payload.payload,
                ServerDataPayloadV1::DeathScreen {
                    visible: true,
                    can_reincarnate: true,
                    can_terminate: true,
                    stage: Some(DeathScreenStageV1::Tribulation),
                    ..
                }
            )),
            "重连时处于 AwaitingRevival + Tribulation 待决策的角色必须重新收到死亡屏\
             （can_terminate=true 因为 Tribulation 携带永久终结风险）；实际 payloads={payloads:?}"
        );

        let cinematic = app
            .world()
            .entity(entity)
            .get::<crate::death_lifecycle::cinematic::DeathCinematic>();
        assert!(
            cinematic.is_some(),
            "重连必须重新插入 DeathCinematic 组件，不能让玩家停在无 UI 的裸奔状态"
        );
    }

    #[test]
    fn reconnect_while_awaiting_revival_fortune_reemits_death_screen_without_terminate_button() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 500 });
        app.add_event::<DeathCinematicPublished>();
        app.add_systems(
            Update,
            reemit_death_screen_for_reconnected_awaiting_revival_clients,
        );

        let (entity, mut helper) = spawn_reconnected_client_actor(
            &mut app,
            "ReconnectFortune",
            Lifecycle {
                character_id: "offline:ReconnectFortune".to_string(),
                fortune_remaining: 1,
                state: LifecycleState::AwaitingRevival,
                awaiting_decision: Some(RevivalDecision::Fortune { chance: 1.0 }),
                revival_decision_deadline_tick: Some(560),
                ..Default::default()
            },
        );

        app.update();
        flush_client_packets(&mut app);

        let payloads = collect_server_data_payloads(&mut helper);
        assert!(
            payloads.iter().any(|payload| matches!(
                payload.payload,
                ServerDataPayloadV1::DeathScreen {
                    visible: true,
                    can_reincarnate: true,
                    can_terminate: false,
                    stage: Some(DeathScreenStageV1::Fortune),
                    ..
                }
            )),
            "Fortune 分支重连必须重新收到死亡屏，且 can_terminate=false（Fortune 不携带\
             永久终结风险，voluntary termination 按钮不应出现）；实际 payloads={payloads:?}"
        );

        assert!(
            app.world()
                .entity(entity)
                .get::<crate::death_lifecycle::cinematic::DeathCinematic>()
                .is_some(),
            "Fortune 分支重连同样必须重新插入 DeathCinematic"
        );
    }

    #[test]
    fn reconnect_while_near_death_does_not_reemit_death_screen() {
        // NearDeath 没有独立的死亡屏（濒死靠 Wounds.health_current 走低血量 HUD 呈现）；
        // 重连时 Wounds::default() 满血复位属于另案跟踪的秒退漏洞（out of scope），这里只
        // 锁住"NearDeath 不会触发本系统发送 DeathScreen/DeathCinematic"这个边界。
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 500 });
        app.add_event::<DeathCinematicPublished>();
        app.add_systems(
            Update,
            reemit_death_screen_for_reconnected_awaiting_revival_clients,
        );

        let (entity, mut helper) = spawn_reconnected_client_actor(
            &mut app,
            "ReconnectNearDeath",
            Lifecycle {
                state: LifecycleState::NearDeath,
                near_death_deadline_tick: Some(560),
                ..Default::default()
            },
        );

        app.update();
        flush_client_packets(&mut app);

        let payloads = collect_server_data_payloads(&mut helper);
        assert!(
            payloads.is_empty(),
            "NearDeath 状态不应该触发死亡屏重发（本系统只处理 AwaitingRevival）；\
             实际收到 {} 个 payload：{payloads:?}",
            payloads.len()
        );
        assert!(
            app.world()
                .entity(entity)
                .get::<crate::death_lifecycle::cinematic::DeathCinematic>()
                .is_none(),
            "NearDeath 状态不应该被插入 DeathCinematic"
        );
    }

    #[test]
    fn reconnect_while_alive_does_not_reemit_death_screen() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 500 });
        app.add_event::<DeathCinematicPublished>();
        app.add_systems(
            Update,
            reemit_death_screen_for_reconnected_awaiting_revival_clients,
        );

        let (_entity, mut helper) =
            spawn_reconnected_client_actor(&mut app, "ReconnectAlive", Lifecycle::default());

        app.update();
        flush_client_packets(&mut app);

        let payloads = collect_server_data_payloads(&mut helper);
        assert!(
            payloads.is_empty(),
            "Alive 状态（最常见的健康在线玩家重连路径）绝不应该触发死亡屏；\
             实际收到 {} 个 payload：{payloads:?}",
            payloads.len()
        );
    }

    #[test]
    fn reconnect_while_awaiting_revival_without_pending_decision_skips_without_panicking() {
        // 状态机内部不一致：state=AwaitingRevival 却没有 awaiting_decision（正常流程不会
        // 产生这种组合，但组件是外部可写的，防御性地要求不 panic、不发送残缺 payload）。
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 500 });
        app.add_event::<DeathCinematicPublished>();
        app.add_systems(
            Update,
            reemit_death_screen_for_reconnected_awaiting_revival_clients,
        );

        let (_entity, mut helper) = spawn_reconnected_client_actor(
            &mut app,
            "ReconnectInconsistent",
            Lifecycle {
                state: LifecycleState::AwaitingRevival,
                awaiting_decision: None,
                ..Default::default()
            },
        );

        app.update();
        flush_client_packets(&mut app);

        let payloads = collect_server_data_payloads(&mut helper);
        assert!(
            payloads.is_empty(),
            "awaiting_decision=None 时没有决策可展示，不应该发送残缺的死亡屏 payload；\
             实际收到 {} 个 payload：{payloads:?}",
            payloads.len()
        );
    }

    #[test]
    fn reconnect_skips_entities_that_already_carry_a_death_cinematic() {
        // Without<DeathCinematic> 过滤器防重复触发：如果实体在 Added<Client> 这一 tick
        // 就已经带着 DeathCinematic（例如某种未来的预取/迁移路径），本系统不应该覆盖或
        // 重复发送。
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 500 });
        app.add_event::<DeathCinematicPublished>();
        app.add_systems(
            Update,
            reemit_death_screen_for_reconnected_awaiting_revival_clients,
        );

        let (entity, mut helper) = spawn_reconnected_client_actor(
            &mut app,
            "ReconnectAlreadyCinematic",
            Lifecycle {
                state: LifecycleState::AwaitingRevival,
                awaiting_decision: Some(RevivalDecision::Fortune { chance: 1.0 }),
                revival_decision_deadline_tick: Some(560),
                ..Default::default()
            },
        );
        let pre_existing_cinematic =
            crate::death_lifecycle::cinematic::DeathCinematic::new(DeathCinematicInit {
                character_id: "offline:ReconnectAlreadyCinematic".to_string(),
                started_at_tick: 400,
                roll: DeathCinematicRollV1 {
                    probability: 1.0,
                    threshold: 1.0,
                    luck_value: 1.0,
                    result: DeathRollResultV1::Survive,
                },
                insight_text: vec!["既有插曲".to_string()],
                is_final: false,
                death_number: 1,
                zone_kind: DeathCinematicZoneKindV1::Ordinary,
                tsy_death: false,
            });
        app.world_mut()
            .entity_mut(entity)
            .insert(pre_existing_cinematic.clone());

        app.update();
        flush_client_packets(&mut app);

        let payloads = collect_server_data_payloads(&mut helper);
        assert!(
            payloads.is_empty(),
            "已经携带 DeathCinematic 的实体必须被 Without<DeathCinematic> 过滤掉，不应该\
             再收到一份重复的死亡屏；实际收到 {} 个 payload：{payloads:?}",
            payloads.len()
        );
        assert_eq!(
            app.world()
                .entity(entity)
                .get::<crate::death_lifecycle::cinematic::DeathCinematic>(),
            Some(&pre_existing_cinematic),
            "既有 DeathCinematic 不应该被覆盖"
        );
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
        // plan-layered-equip-v1 P0.6 — reset_for_new_character 现需 ItemRegistry 重建 inventory。
        app.insert_resource(item_registry);
        app.insert_resource(InventoryInstanceIdAllocator::default());

        app.add_event::<RevivalActionIntent>();
        app.add_event::<PlayerRevived>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<crate::coffin::CoffinStateChanged>();
        app.add_systems(Update, handle_revival_action_intents);

        let username = Username("Azure".to_string());
        let mut anticheat_counter = AntiCheatCounter::default();
        anticheat_counter
            .record_violation(ViolationKindV1::ReachExceeded, "reach: previous character");
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
                        location: crate::body_plan::legacy_body_part_to_id(BodyPart::Chest),
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
                anticheat_counter,
                Position::new([99.0, 64.0, 99.0]),
                username.clone(),
                SkillSet::default(),
                Nourishment {
                    satiety: 11.0,
                    hydration: 12.0,
                },
                NourishmentActivityWindow::default(),
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
        let anticheat_counter = entity_ref
            .get::<AntiCheatCounter>()
            .expect("anticheat counter should remain attached");

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
        // plan-multi-life-v1 §2：新角色 = Awaken 境界，寿元 = 醒灵 cap (AWAKEN=120)
        // 与 attach_cultivation_to_joined_clients 路径保持一致；旧值 MORTAL=80 是 bug。
        assert_eq!(lifespan.cap_by_realm, LifespanCapTable::AWAKEN);
        assert_eq!(lifespan.years_lived, 0.0);
        assert_eq!(player_state, &PlayerState::default());
        let expected_spawn = crate::cultivation::character_select::next_character_spec_for_seed(
            &lifecycle.character_id,
        )
        .spawn_pos;
        assert_eq!(position.get(), Position::new(expected_spawn).get());
        assert_eq!(cultivation.realm, Realm::Awaken);
        assert_eq!(cultivation.qi_current, 0.0);
        assert_eq!(cultivation.qi_max, 10.0);
        assert_eq!(meridians.opened_count(), 0);
        assert_eq!(learned.ids, vec!["kai_mai_pill_v0".to_string()]);
        assert!(inventory.revision.0 >= 1);
        assert_eq!(anticheat_counter.reach_violations, 0);
        assert_eq!(anticheat_counter.cooldown_violations, 0);
        assert_eq!(anticheat_counter.qi_invest_violations, 0);
        assert!(anticheat_counter.last_reach_details.is_empty());
        assert_eq!(
            *entity_ref
                .get::<Nourishment>()
                .expect("fresh character should retain nourishment component"),
            Nourishment::spawn_default(),
            "CreateNewCharacter must reset both nourishment axes to 80/80"
        );
        assert_eq!(
            *entity_ref
                .get::<NourishmentActivityWindow>()
                .expect("fresh character should retain activity window"),
            NourishmentActivityWindow::default(),
            "CreateNewCharacter must clear every accumulated activity tick"
        );

        let persisted = crate::player::state::load_player_slices(
            &PlayerStatePersistence::with_db_path(&data_dir, settings.db_path()),
            username.0.as_str(),
        );
        assert_eq!(persisted.state, PlayerState::default());
        assert_eq!(persisted.position, expected_spawn);
        assert!(persisted.inventory.is_some());
        let persisted_lifespan = persisted.lifespan.expect("fresh lifespan should persist");
        assert_eq!(persisted_lifespan.born_at_tick, 0);
        // plan-multi-life-v1 §2：持久化的 lifespan 同样为 AWAKEN cap
        assert_eq!(persisted_lifespan.cap_by_realm, LifespanCapTable::AWAKEN);
        assert!(persisted_lifespan.years_lived >= 0.0);
        assert!(persisted_lifespan.years_lived < 0.01);
        assert_eq!(persisted_lifespan.offline_pause_tick, None);
        assert_eq!(
            load_persisted_nourishment(&settings, username.0.as_str()),
            Nourishment::spawn_default(),
            "CreateNewCharacter must persist reset nourishment without serializing session activity"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn create_new_character_precommit_failure_rolls_back_sqlite_and_ecs() {
        let (settings, root) = persistence_settings("create-new-character-precommit-rollback");
        let data_dir = root.join("data");
        let player_persistence =
            PlayerStatePersistence::with_db_path(&data_dir, settings.db_path());
        let username = "AtomicNewCharacter";
        let old_state = PlayerState {
            karma: 0.4,
            inventory_score: 0.8,
        };
        let old_position = [99.0, 64.0, 99.0];
        let old_lifespan = LifespanComponent::new(LifespanCapTable::MORTAL);
        let mut old_skill_set = SkillSet::default();
        old_skill_set.skills.insert(
            crate::skill::components::SkillId::Combat,
            crate::skill::components::SkillEntry {
                lv: 3,
                xp: 70,
                total_xp: 470,
                last_action_at: 600,
                recent_repeat_count: 1,
            },
        );
        crate::player::state::save_player_slices_with_coffin(
            &player_persistence,
            username,
            &old_state,
            old_position,
            DimensionKind::Tsy,
            None,
            Some(&old_lifespan),
            &old_skill_set,
            Some(crate::coffin::CoffinGrade::Jade),
            None,
        )
        .expect("test setup must persist prior player slices and coffin state");
        let current_char_id_before =
            crate::player::state::load_current_character_id(&player_persistence, username)
                .expect("current character id should load")
                .expect("current character id should exist");

        let old_character_id = player_character_id(username, current_char_id_before.as_str());
        let old_cultivation = Cultivation {
            realm: Realm::Induce,
            qi_current: 7.0,
            qi_max: 24.0,
            ..Default::default()
        };
        let old_meridians = MeridianSystem::default();
        let old_contamination = Contamination::default();
        let old_life_record = LifeRecord::new(old_character_id.clone());
        let old_nourishment = Nourishment {
            satiety: 23.0,
            hydration: 31.0,
        };
        seed_revival_nourishment_bundle(
            &settings,
            username,
            &old_cultivation,
            &old_meridians,
            &old_contamination,
            &old_life_record,
            old_nourishment,
        );
        let persisted_bundle_before =
            crate::persistence::load_player_cultivation_bundle(&settings, username)
                .expect("prior cultivation bundle should load")
                .expect("prior cultivation bundle should exist");

        let mut app = revival_action_test_app(settings.clone(), 800);
        app.insert_resource(player_persistence.clone());
        let item_registry =
            crate::inventory::load_item_registry().expect("item registry should load");
        let default_loadout = crate::inventory::load_default_loadout(&item_registry)
            .expect("default loadout should load");
        app.insert_resource(DefaultLoadout(default_loadout));
        app.insert_resource(item_registry);
        app.insert_resource(InventoryInstanceIdAllocator::default());
        let (entity, mut helper) = spawn_revival_action_actor(
            &mut app,
            username,
            RevivalActionActorState {
                lifecycle: Lifecycle {
                    character_id: old_character_id.clone(),
                    state: LifecycleState::Terminated,
                    death_count: 9,
                    fortune_remaining: 0,
                    last_death_tick: Some(799),
                    ..Default::default()
                },
                cultivation: old_cultivation,
                meridians: old_meridians,
                contamination: old_contamination,
                life_record: old_life_record,
                nourishment: old_nourishment,
            },
        );
        app.world_mut().entity_mut(entity).insert((
            DeathRegistry {
                char_id: old_character_id,
                death_count: 9,
                last_death_tick: Some(799),
                prev_death_tick: None,
                last_death_zone: Some(ZoneDeathKind::Death),
            },
            old_lifespan,
            old_state.clone(),
        ));
        let coffin_lower = valence::prelude::BlockPos::new(10, 64, 10);
        let coffin_before = crate::coffin::CoffinComponent {
            entered_at_tick: 500,
            coffin_lower,
            grade: crate::coffin::CoffinGrade::Jade,
        };
        app.world_mut().entity_mut(entity).insert(coffin_before);
        let mut coffin_registry = crate::coffin::CoffinRegistry::default();
        assert!(coffin_registry.insert(coffin_lower, 300, crate::coffin::CoffinGrade::Jade));
        assert!(coffin_registry.set_occupied(coffin_lower, entity));
        app.insert_resource(coffin_registry);
        app.update();
        let lifecycle_before = serde_json::to_value(app.world().get::<Lifecycle>(entity).unwrap())
            .expect("lifecycle snapshot should serialize");
        let life_record_before =
            serde_json::to_value(app.world().get::<LifeRecord>(entity).unwrap())
                .expect("life record snapshot should serialize");
        let activity_before = *app
            .world()
            .get::<NourishmentActivityWindow>(entity)
            .unwrap();
        let allocator_before = app
            .world()
            .resource::<InventoryInstanceIdAllocator>()
            .clone();
        let persisted_before = load_player_slices(&player_persistence, username);
        assert!(persisted_before.inventory.is_none());
        assert_eq!(
            serde_json::to_value(&persisted_before.skill_set)
                .expect("prior skill set should serialize"),
            serde_json::to_value(&old_skill_set).expect("skill set fixture should serialize")
        );
        let _failpoint = crate::persistence::arm_fail_before_commit(settings.db_path());

        app.world_mut().send_event(RevivalActionIntent {
            entity,
            action: RevivalActionKind::CreateNewCharacter,
            issued_at_tick: 800,
        });
        app.update();

        assert_eq!(
            serde_json::to_value(app.world().get::<Lifecycle>(entity).unwrap())
                .expect("lifecycle snapshot should serialize"),
            lifecycle_before,
            "precommit failure must leave the terminated lifecycle unchanged"
        );
        assert_eq!(
            serde_json::to_value(app.world().get::<LifeRecord>(entity).unwrap())
                .expect("life record snapshot should serialize"),
            life_record_before,
            "precommit failure must not replace the prior life record"
        );
        assert_eq!(
            *app.world().get::<PlayerState>(entity).unwrap(),
            old_state,
            "precommit failure must not reset live player state"
        );
        assert_eq!(
            *app.world().get::<Nourishment>(entity).unwrap(),
            old_nourishment,
            "precommit failure must not reset live nourishment"
        );
        assert_eq!(
            *app.world()
                .get::<NourishmentActivityWindow>(entity)
                .unwrap(),
            activity_before,
            "precommit failure must not clear session activity"
        );
        assert!(
            app.world().get::<PlayerInventory>(entity).is_none(),
            "precommit failure must not attach a fresh inventory"
        );
        assert_eq!(
            format!(
                "{:?}",
                app.world().resource::<InventoryInstanceIdAllocator>()
            ),
            format!("{allocator_before:?}"),
            "precommit failure must not consume inventory instance ids"
        );
        flush_client_packets(&mut app);
        let server_payloads = collect_server_data_payloads(&mut helper);
        assert!(
            server_payloads.iter().all(|payload| !matches!(
                payload.payload,
                ServerDataPayloadV1::DeathScreen { visible: false, .. }
                    | ServerDataPayloadV1::TerminateScreen { visible: false, .. }
            )),
            "failed new-character persistence must not hide death or terminate UI"
        );
        assert_eq!(
            app.world().get::<crate::coffin::CoffinComponent>(entity),
            Some(&coffin_before),
            "failed new-character persistence must not remove the live CoffinComponent"
        );
        let coffin_registry_after = app.world().resource::<crate::coffin::CoffinRegistry>();
        assert_eq!(
            coffin_registry_after.player_in_coffin.get(&entity),
            Some(&coffin_lower),
            "failed new-character persistence must retain the player-to-coffin registry index"
        );
        assert_eq!(
            coffin_registry_after
                .lookup(coffin_lower)
                .expect("registered coffin should remain")
                .occupied_by,
            Some(entity),
            "failed new-character persistence must retain coffin occupancy"
        );
        assert_eq!(
            app.world_mut()
                .resource_mut::<Events<crate::coffin::CoffinStateChanged>>()
                .drain()
                .count(),
            0,
            "failed new-character persistence must not emit CoffinStateChanged"
        );

        let current_char_id_after =
            crate::player::state::load_current_character_id(&player_persistence, username)
                .expect("current character id should reload")
                .expect("current character id should remain");
        assert_eq!(
            current_char_id_after, current_char_id_before,
            "precommit failure must roll back player_core.current_char_id"
        );
        let persisted_after =
            crate::player::state::load_player_slices(&player_persistence, username);
        assert_eq!(persisted_after.state, old_state);
        assert_eq!(persisted_after.position, old_position);
        assert_eq!(persisted_after.last_dimension, DimensionKind::Tsy);
        assert!(
            persisted_after.inventory.is_none(),
            "precommit failure must roll back the staged fresh inventory slice"
        );
        assert_eq!(
            serde_json::to_value(&persisted_after.skill_set)
                .expect("rolled-back skill set should serialize"),
            serde_json::to_value(&old_skill_set).expect("skill set fixture should serialize"),
            "precommit failure must roll back the staged fresh skill slice"
        );
        assert!(persisted_after.in_coffin);
        assert_eq!(
            persisted_after.coffin_grade,
            Some(crate::coffin::CoffinGrade::Jade)
        );
        assert_eq!(
            persisted_after
                .lifespan
                .expect("prior lifespan should remain")
                .cap_by_realm,
            LifespanCapTable::MORTAL
        );
        assert_eq!(
            crate::persistence::load_player_cultivation_bundle(&settings, username)
                .expect("cultivation bundle should reload")
                .expect("cultivation bundle should remain"),
            persisted_bundle_before,
            "precommit failure must roll back the staged cultivation bundle"
        );

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
        // plan-layered-equip-v1 P0.6 — reset_for_new_character 现需 ItemRegistry 重建 inventory。
        app.insert_resource(item_registry);
        app.insert_resource(InventoryInstanceIdAllocator::default());

        app.add_event::<RevivalActionIntent>();
        app.add_event::<PlayerRevived>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<crate::coffin::CoffinStateChanged>();
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
        app.add_event::<DeathCinematicPublished>();
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
        let (settings, root) = persistence_settings("revive-spawn-anchor");
        let mut app = revival_action_test_app(settings.clone(), 42);
        let shrine_anchor = [123.0, 45.0, -67.0];

        let spawn_actor = |app: &mut App, username: &str, spawn_anchor: Option<[f64; 3]>| {
            let cultivation = Cultivation::default();
            let meridians = MeridianSystem::default();
            let contamination = Contamination::default();
            let life_record = LifeRecord::new(canonical_player_id(username));
            seed_revival_nourishment_bundle(
                &settings,
                username,
                &cultivation,
                &meridians,
                &contamination,
                &life_record,
                Nourishment::spawn_default(),
            );
            spawn_revival_action_actor(
                app,
                username,
                RevivalActionActorState {
                    lifecycle: Lifecycle {
                        state: LifecycleState::AwaitingRevival,
                        awaiting_decision: Some(RevivalDecision::Fortune { chance: 1.0 }),
                        spawn_anchor,
                        fortune_remaining: 1,
                        ..Default::default()
                    },
                    cultivation,
                    meridians,
                    contamination,
                    life_record,
                    nourishment: Nourishment::spawn_default(),
                },
            )
        };

        let (with_shrine, _with_shrine_helper) =
            spawn_actor(&mut app, "ReviveAtShrine", Some(shrine_anchor));
        let (without_shrine, _without_shrine_helper) =
            spawn_actor(&mut app, "ReviveAtWorldSpawn", None);

        for entity in [with_shrine, without_shrine] {
            app.world_mut().send_event(RevivalActionIntent {
                entity,
                action: RevivalActionKind::Reincarnate,
                issued_at_tick: 42,
            });
        }
        app.update();

        assert_eq!(
            app.world()
                .entity(with_shrine)
                .get::<Position>()
                .expect("position should exist")
                .get(),
            Position::new(shrine_anchor).get()
        );
        assert_eq!(
            app.world()
                .entity(without_shrine)
                .get::<Position>()
                .expect("position should exist")
                .get(),
            Position::new(crate::player::spawn_position()).get()
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn damaged_spawn_anchor_doubles_revive_weakened_duration() {
        let (settings, root) = persistence_settings("revive-damaged-spawn-anchor");
        let mut app = revival_action_test_app(settings, 42);

        let (damaged, _damaged_helper) = spawn_revival_action_actor(
            &mut app,
            "DamagedAnchor",
            RevivalActionActorState {
                lifecycle: Lifecycle {
                    character_id: canonical_player_id("DamagedAnchor"),
                    state: LifecycleState::AwaitingRevival,
                    awaiting_decision: Some(RevivalDecision::Fortune { chance: 1.0 }),
                    spawn_anchor: Some([11.0, 65.0, 10.0]),
                    spawn_anchor_damaged: true,
                    fortune_remaining: 1,
                    ..Default::default()
                },
                cultivation: Cultivation::default(),
                meridians: MeridianSystem::default(),
                contamination: Contamination::default(),
                life_record: LifeRecord::new(canonical_player_id("DamagedAnchor")),
                nourishment: Nourishment::spawn_default(),
            },
        );
        let (intact, _intact_helper) = spawn_revival_action_actor(
            &mut app,
            "IntactAnchor",
            RevivalActionActorState {
                lifecycle: Lifecycle {
                    character_id: canonical_player_id("IntactAnchor"),
                    state: LifecycleState::AwaitingRevival,
                    awaiting_decision: Some(RevivalDecision::Fortune { chance: 1.0 }),
                    spawn_anchor: Some([12.0, 65.0, 10.0]),
                    spawn_anchor_damaged: false,
                    fortune_remaining: 1,
                    ..Default::default()
                },
                cultivation: Cultivation::default(),
                meridians: MeridianSystem::default(),
                contamination: Contamination::default(),
                life_record: LifeRecord::new(canonical_player_id("IntactAnchor")),
                nourishment: Nourishment::spawn_default(),
            },
        );

        for entity in [damaged, intact] {
            app.world_mut().send_event(RevivalActionIntent {
                entity,
                action: RevivalActionKind::Reincarnate,
                issued_at_tick: 42,
            });
        }
        app.update();

        let damaged_lifecycle = app.world().entity(damaged).get::<Lifecycle>().unwrap();
        let intact_lifecycle = app.world().entity(intact).get::<Lifecycle>().unwrap();
        assert_eq!(
            damaged_lifecycle.weakened_until_tick.unwrap() - 42,
            REVIVE_WEAKENED_TICKS * 2,
            "damaged spirit niche spawn anchor should double revive weakened duration"
        );
        assert_eq!(
            intact_lifecycle.weakened_until_tick.unwrap() - 42,
            REVIVE_WEAKENED_TICKS,
            "intact spirit niche spawn anchor should keep baseline revive weakened duration"
        );

        let _ = fs::remove_dir_all(root);
    }

    // ── plan-shield-block-v1 P2 §Issue5.2 — sync_combat_state ShieldBlocking 保留 ──
    // 被命中时若受击方处于 ShieldBlocking 状态，sync_combat_state_from_events 不应将其
    // stamina.state 翻成 Combat（应保留 ShieldBlocking，由 stamina_tick 维护 drain 逻辑）。
    #[test]
    fn sync_combat_state_preserves_shield_blocking_state_on_target() {
        let mut app = App::new();
        app.add_event::<CombatEvent>();
        app.add_systems(Update, sync_combat_state_from_events);

        let attacker = app
            .world_mut()
            .spawn((
                Wounds::default(),
                Stamina {
                    current: 100.0,
                    max: 100.0,
                    recover_per_sec: 5.0,
                    state: StaminaState::Combat,
                    last_drain_tick: None,
                },
                CombatState::default(),
                Lifecycle::default(),
            ))
            .id();
        let target = app
            .world_mut()
            .spawn((
                Wounds::default(),
                Stamina {
                    current: 60.0,
                    max: 100.0,
                    recover_per_sec: 5.0,
                    state: StaminaState::ShieldBlocking,
                    last_drain_tick: None,
                },
                CombatState::default(),
                Lifecycle::default(),
            ))
            .id();

        app.world_mut().send_event(CombatEvent {
            attacker,
            target,
            resolved_at_tick: 100,
            body_part: BodyPart::Chest,
            wound_kind: WoundKind::Blunt,
            source: crate::combat::events::AttackSource::Melee,
            debug_command: false,
            physical_damage: 0.5,
            damage: 0.0,
            contam_delta: 0.0,
            description: "test_hit".to_string(),
            defense_kind: Some(crate::combat::events::DefenseKind::ShieldBlock),
            defense_effectiveness: Some(0.6),
            defense_contam_reduced: None,
            defense_wound_severity: None,
        });
        app.update();

        let target_stamina = app.world().entity(target).get::<Stamina>().unwrap();
        assert_eq!(
            target_stamina.state,
            StaminaState::ShieldBlocking,
            "sync_combat_state_from_events 被命中时不应将 ShieldBlocking 状态覆写为 Combat；\
             举盾状态由 stamina_tick 维护（drain/exhausted 逻辑）；\
             actual: {:?}",
            target_stamina.state
        );
    }

    // ─────────────── P0 fix: revive/new_char 清 coffin 状态 ───────────────
    //
    // 覆盖 r5-P0 修复：入棺玩家复活/新建角色后 coffin 状态必须彻底清除。
    // 三件套：CoffinComponent（ECS）+ CoffinRegistry + CoffinStateChanged 事件。
    // 持久化层（SQLite persist_in_coffin）在无 PlayerStatePersistence 时静默跳过，
    // 单测靠 CoffinRegistry + ECS 断言可观察行为。

    fn make_coffin_registry_with_player(player: Entity) -> crate::coffin::CoffinRegistry {
        let lower = valence::prelude::BlockPos::new(10, 64, 10);
        let mut registry = crate::coffin::CoffinRegistry::default();
        registry.insert(lower, 0, crate::coffin::CoffinGrade::Mundane);
        registry.set_occupied(lower, player);
        registry
    }

    fn coffin_setup_base(app: &mut App, tick: u64) {
        let (settings, _root) = persistence_settings("coffin-clear-revive");
        app.insert_resource(settings);
        app.insert_resource(CombatClock { tick });
        app.add_event::<RevivalActionIntent>();
        app.add_event::<PlayerRevived>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<crate::coffin::CoffinStateChanged>();
        app.add_systems(Update, handle_revival_action_intents);
    }

    /// 入棺玩家复活后：
    ///   - CoffinComponent 从 entity 移除（期望：None，因为复活后不应继续锁棺）
    ///   - CoffinRegistry.player_in_coffin 不含该 entity（期望：None，因为 clear_player 清双索引）
    ///   - CoffinStateChanged 事件被发出 grade=None（期望：收到 1 条，因为玩家确实在棺内）
    #[test]
    fn revive_clears_coffin_component_and_registry_and_emits_state_changed() {
        let (settings, root) = persistence_settings("coffin-clear-revive");
        let mut app = revival_action_test_app(settings, 500);

        let (entity, _helper) = spawn_revival_action_actor(
            &mut app,
            "CoffinRevive",
            RevivalActionActorState {
                lifecycle: Lifecycle {
                    character_id: canonical_player_id("CoffinRevive"),
                    state: LifecycleState::AwaitingRevival,
                    awaiting_decision: Some(RevivalDecision::Fortune { chance: 1.0 }),
                    revival_decision_deadline_tick: Some(600),
                    fortune_remaining: 1,
                    ..Default::default()
                },
                cultivation: Cultivation {
                    realm: Realm::Awaken,
                    qi_current: 10.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                meridians: MeridianSystem::default(),
                contamination: Contamination::default(),
                life_record: LifeRecord::new(canonical_player_id("CoffinRevive")),
                nourishment: Nourishment::spawn_default(),
            },
        );
        app.world_mut()
            .entity_mut(entity)
            .insert(crate::coffin::CoffinComponent {
                entered_at_tick: 400,
                coffin_lower: valence::prelude::BlockPos::new(10, 64, 10),
                grade: crate::coffin::CoffinGrade::Mundane,
            });

        let registry = make_coffin_registry_with_player(entity);
        app.insert_resource(registry);

        app.world_mut().send_event(RevivalActionIntent {
            entity,
            action: RevivalActionKind::Reincarnate,
            issued_at_tick: 500,
        });
        app.update();

        // CoffinComponent 应已从 entity 移除（复活后不继续锁棺）
        assert!(
            app.world()
                .entity(entity)
                .get::<crate::coffin::CoffinComponent>()
                .is_none(),
            "期望 CoffinComponent=None（复活后不锁棺），实际仍有 CoffinComponent"
        );

        // CoffinRegistry.player_in_coffin 应清空
        let reg = app.world().resource::<crate::coffin::CoffinRegistry>();
        assert!(
            !reg.player_in_coffin.contains_key(&entity),
            "期望 player_in_coffin 不含 entity（clear_player 应清双索引），实际仍含该 entity"
        );

        // CoffinStateChanged(grade=None) 应被发送
        let state_events = app
            .world_mut()
            .resource_mut::<Events<crate::coffin::CoffinStateChanged>>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(
            state_events.len(),
            1,
            "期望发出 1 条 CoffinStateChanged（玩家在棺内复活），实际发出 {} 条",
            state_events.len()
        );
        assert!(
            state_events[0].grade.is_none(),
            "期望 CoffinStateChanged.grade=None（离棺），实际 {:?}",
            state_events[0].grade
        );

        let _ = fs::remove_dir_all(root);
    }

    /// 非入棺玩家复活：不误清、不误发 CoffinStateChanged。
    ///   - CoffinComponent 不存在（期望：无副作用，remove 幂等）
    ///   - CoffinStateChanged 事件不发（期望：0 条，因为玩家本来不在棺内）
    #[test]
    fn revive_without_coffin_does_not_emit_coffin_state_changed() {
        let (settings, root) = persistence_settings("revive-without-coffin");
        let mut app = revival_action_test_app(settings, 500);
        app.insert_resource(crate::coffin::CoffinRegistry::default());

        let (entity, _helper) = spawn_revival_action_actor(
            &mut app,
            "NoCoffin",
            RevivalActionActorState {
                lifecycle: Lifecycle {
                    character_id: canonical_player_id("NoCoffin"),
                    state: LifecycleState::AwaitingRevival,
                    awaiting_decision: Some(RevivalDecision::Fortune { chance: 1.0 }),
                    revival_decision_deadline_tick: Some(600),
                    fortune_remaining: 1,
                    ..Default::default()
                },
                cultivation: Cultivation {
                    realm: Realm::Awaken,
                    qi_current: 10.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                meridians: MeridianSystem::default(),
                contamination: Contamination::default(),
                life_record: LifeRecord::new(canonical_player_id("NoCoffin")),
                nourishment: Nourishment::spawn_default(),
            },
        );

        app.world_mut().send_event(RevivalActionIntent {
            entity,
            action: RevivalActionKind::Reincarnate,
            issued_at_tick: 500,
        });
        app.update();

        assert_eq!(
            app.world().get::<Lifecycle>(entity).unwrap().state,
            LifecycleState::Alive,
            "test must observe a completed revival before checking coffin side effects"
        );
        // CoffinComponent 本来就没有，remove 幂等，entity 无异常
        assert!(
            app.world()
                .entity(entity)
                .get::<crate::coffin::CoffinComponent>()
                .is_none(),
            "非棺内玩家复活后 CoffinComponent 应为 None（remove 幂等）"
        );

        // 不应发出 CoffinStateChanged（clear_player 返回 None → 条件不满足 → 不发事件）
        let state_events = app
            .world_mut()
            .resource_mut::<Events<crate::coffin::CoffinStateChanged>>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(
            state_events.len(),
            0,
            "期望非棺内玩家复活不发 CoffinStateChanged（避免噪音推送），实际发出 {} 条",
            state_events.len()
        );

        let _ = fs::remove_dir_all(root);
    }

    /// 新建角色：coffin 状态同样清除（即便理论上新角色无 coffin，防止旧 entity 残留）。
    ///   - CoffinComponent 从 entity 移除（期望：None，因为新角色不继承死亡前棺状态）
    ///   - CoffinRegistry.player_in_coffin 不含该 entity（期望：None）
    ///   - CoffinStateChanged 事件被发出 grade=None（期望：1 条，因为玩家在棺内）
    #[test]
    fn create_new_character_clears_coffin_state_and_emits_state_changed() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("coffin-clear-new-char");
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
        // plan-layered-equip-v1 P0.6 — reset_for_new_character 现需 ItemRegistry 重建 inventory。
        app.insert_resource(item_registry);
        app.insert_resource(InventoryInstanceIdAllocator::default());
        app.add_event::<RevivalActionIntent>();
        app.add_event::<PlayerRevived>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<crate::coffin::CoffinStateChanged>();
        app.add_systems(Update, handle_revival_action_intents);

        let entity = app
            .world_mut()
            .spawn((
                Wounds::default(),
                Stamina::default(),
                CombatState::default(),
                Lifecycle {
                    character_id: "offline:CoffinNewChar".to_string(),
                    state: LifecycleState::Terminated,
                    ..Default::default()
                },
                LifeRecord::new("offline:CoffinNewChar"),
                Username("CoffinNewChar".to_string()),
                DeathRegistry::new("offline:CoffinNewChar"),
                LifespanComponent {
                    born_at_tick: 0,
                    years_lived: 50.0,
                    cap_by_realm: crate::cultivation::lifespan::LifespanCapTable::AWAKEN,
                    offline_pause_tick: None,
                },
                Cultivation::default(),
                MeridianSystem::default(),
                crate::coffin::CoffinComponent {
                    entered_at_tick: 700,
                    coffin_lower: valence::prelude::BlockPos::new(20, 64, 20),
                    grade: crate::coffin::CoffinGrade::Jade,
                },
            ))
            .id();

        let registry = make_coffin_registry_with_player(entity);
        app.insert_resource(registry);

        app.world_mut().send_event(RevivalActionIntent {
            entity,
            action: RevivalActionKind::CreateNewCharacter,
            issued_at_tick: 800,
        });
        app.update();

        // CoffinComponent 应已从 entity 移除（新建角色不继承旧棺状态）
        assert!(
            app.world()
                .entity(entity)
                .get::<crate::coffin::CoffinComponent>()
                .is_none(),
            "期望 CoffinComponent=None（新建角色后不锁棺），实际仍有 CoffinComponent"
        );

        // CoffinRegistry.player_in_coffin 应清空
        let reg = app.world().resource::<crate::coffin::CoffinRegistry>();
        assert!(
            !reg.player_in_coffin.contains_key(&entity),
            "期望新建角色后 player_in_coffin 不含 entity，实际仍含"
        );

        // CoffinStateChanged(grade=None) 应被发送（玩家确实在棺内 → clear_player 返回 Some）
        let state_events = app
            .world_mut()
            .resource_mut::<Events<crate::coffin::CoffinStateChanged>>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(
            state_events.len(),
            1,
            "期望新建角色发出 1 条 CoffinStateChanged（玩家在棺内），实际 {} 条",
            state_events.len()
        );
        assert!(
            state_events[0].grade.is_none(),
            "期望 CoffinStateChanged.grade=None（离棺），实际 {:?}",
            state_events[0].grade
        );

        let _ = fs::remove_dir_all(root);
    }

    /// 边界：入棺 → 复活 → 再入棺 → 再复活，两次循环均正常清除 coffin 状态。
    #[test]
    fn revive_enter_coffin_revive_cycle_clears_correctly() {
        let (settings, root) = persistence_settings("coffin-revive-cycle");
        let mut app = revival_action_test_app(settings, 100);
        app.insert_resource(crate::coffin::CoffinRegistry::default());

        // 第一轮：带 CoffinComponent 的玩家复活
        let lower = valence::prelude::BlockPos::new(5, 64, 5);
        let (entity, _helper) = spawn_revival_action_actor(
            &mut app,
            "CycleTest",
            RevivalActionActorState {
                lifecycle: Lifecycle {
                    character_id: canonical_player_id("CycleTest"),
                    state: LifecycleState::AwaitingRevival,
                    awaiting_decision: Some(RevivalDecision::Fortune { chance: 1.0 }),
                    revival_decision_deadline_tick: Some(200),
                    fortune_remaining: 3,
                    ..Default::default()
                },
                cultivation: Cultivation {
                    realm: Realm::Awaken,
                    qi_current: 10.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                meridians: MeridianSystem::default(),
                contamination: Contamination::default(),
                life_record: LifeRecord::new(canonical_player_id("CycleTest")),
                nourishment: Nourishment::spawn_default(),
            },
        );
        app.world_mut()
            .entity_mut(entity)
            .insert(crate::coffin::CoffinComponent {
                entered_at_tick: 50,
                coffin_lower: lower,
                grade: crate::coffin::CoffinGrade::Mundane,
            });

        {
            let mut reg = app
                .world_mut()
                .resource_mut::<crate::coffin::CoffinRegistry>();
            reg.insert(lower, 0, crate::coffin::CoffinGrade::Mundane);
            reg.set_occupied(lower, entity);
        }

        // 第一次复活
        app.world_mut().send_event(RevivalActionIntent {
            entity,
            action: RevivalActionKind::Reincarnate,
            issued_at_tick: 100,
        });
        app.update();

        // 第一次复活后：棺状态已清除
        assert!(
            app.world()
                .entity(entity)
                .get::<crate::coffin::CoffinComponent>()
                .is_none(),
            "第一次复活后 CoffinComponent 应为 None"
        );
        {
            let reg = app.world().resource::<crate::coffin::CoffinRegistry>();
            assert!(
                !reg.player_in_coffin.contains_key(&entity),
                "第一次复活后 player_in_coffin 应为空"
            );
        }

        // 模拟第二次入棺（ECS 加回 CoffinComponent，registry 重新 set_occupied）
        let lower2 = valence::prelude::BlockPos::new(30, 64, 30);
        app.world_mut()
            .entity_mut(entity)
            .insert(crate::coffin::CoffinComponent {
                entered_at_tick: 150,
                coffin_lower: lower2,
                grade: crate::coffin::CoffinGrade::Mundane,
            });
        {
            let world = app.world_mut();
            let mut entity_ref = world.entity_mut(entity);
            let mut lifecycle = entity_ref.get_mut::<Lifecycle>().unwrap();
            lifecycle.state = LifecycleState::AwaitingRevival;
            lifecycle.awaiting_decision = Some(RevivalDecision::Fortune { chance: 1.0 });
            lifecycle.revival_decision_deadline_tick = Some(300);
        }
        {
            let mut reg = app
                .world_mut()
                .resource_mut::<crate::coffin::CoffinRegistry>();
            reg.insert(lower2, 100, crate::coffin::CoffinGrade::Mundane);
            reg.set_occupied(lower2, entity);
        }

        // 第二次复活
        app.world_mut().resource_mut::<CombatClock>().tick = 200;
        app.world_mut().send_event(RevivalActionIntent {
            entity,
            action: RevivalActionKind::Reincarnate,
            issued_at_tick: 200,
        });
        app.update();

        // 第二次复活后：棺状态同样清除
        assert!(
            app.world()
                .entity(entity)
                .get::<crate::coffin::CoffinComponent>()
                .is_none(),
            "第二次复活后 CoffinComponent 应为 None（循环应正常清除）"
        );
        {
            let reg = app.world().resource::<crate::coffin::CoffinRegistry>();
            assert!(
                !reg.player_in_coffin.contains_key(&entity),
                "第二次复活后 player_in_coffin 应为空（循环应正常清除）"
            );
        }

        let _ = fs::remove_dir_all(root);
    }

    // ─────────── must_fix #1&#2: terminate 路径清 coffin（ECS + Registry + CoffinStateChanged）───────────

    /// 劫数不过（tribulation_failed）→ terminate_lifecycle 后 coffin 状态必须全部清除：
    ///   - CoffinComponent 从 entity 移除（期望：None）
    ///   - CoffinRegistry.player_in_coffin 不含该 entity（期望：None）
    ///   - CoffinStateChanged(grade=None) 被发出（期望：1 条）
    #[test]
    fn tribulation_failed_terminate_clears_coffin_state() {
        let mut app = App::new();
        coffin_setup_base(&mut app, 600);
        // 注：coffin_setup_base 不预插 CoffinRegistry，需手动 insert 后再 set_occupied
        app.insert_resource(crate::coffin::CoffinRegistry::default());

        let lower = valence::prelude::BlockPos::new(15, 64, 15);
        let entity = app
            .world_mut()
            .spawn((
                Wounds {
                    health_current: 1.0,
                    health_max: 30.0,
                    entries: Vec::new(),
                },
                Stamina::default(),
                CombatState::default(),
                Lifecycle {
                    character_id: "offline:TribFail".to_string(),
                    state: LifecycleState::AwaitingRevival,
                    // 劫数决策，chance=0 → roll_rebirth 必然返回 false → 走 terminate 分支
                    awaiting_decision: Some(RevivalDecision::Tribulation { chance: 0.0 }),
                    revival_decision_deadline_tick: Some(700),
                    fortune_remaining: 0,
                    ..Default::default()
                },
                Cultivation {
                    realm: Realm::Awaken,
                    qi_current: 10.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                MeridianSystem::default(),
                Contamination::default(),
                LifeRecord::new("offline:TribFail"),
                crate::coffin::CoffinComponent {
                    entered_at_tick: 550,
                    coffin_lower: lower,
                    grade: crate::coffin::CoffinGrade::Mundane,
                },
            ))
            .id();

        {
            let mut reg = app
                .world_mut()
                .resource_mut::<crate::coffin::CoffinRegistry>();
            reg.insert(lower, 0, crate::coffin::CoffinGrade::Mundane);
            reg.set_occupied(lower, entity);
        }

        // 发 Reincarnate；因 chance=0 roll 必失 → 走 terminate 分支
        app.world_mut().send_event(RevivalActionIntent {
            entity,
            action: RevivalActionKind::Reincarnate,
            issued_at_tick: 600,
        });
        app.update();

        // 验证：CoffinComponent 已移除
        assert!(
            app.world()
                .entity(entity)
                .get::<crate::coffin::CoffinComponent>()
                .is_none(),
            "期望 tribulation_failed 后 CoffinComponent=None，实际仍存在"
        );

        // 验证：Registry 已清空
        let reg = app.world().resource::<crate::coffin::CoffinRegistry>();
        assert!(
            !reg.player_in_coffin.contains_key(&entity),
            "期望 tribulation_failed 后 player_in_coffin 不含 entity，实际仍含"
        );

        // 验证：CoffinStateChanged(grade=None) 被发出
        let state_events = app
            .world_mut()
            .resource_mut::<Events<crate::coffin::CoffinStateChanged>>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(
            state_events.len(),
            1,
            "期望 tribulation_failed 发出 1 条 CoffinStateChanged，实际 {} 条",
            state_events.len()
        );
        assert!(
            state_events[0].grade.is_none(),
            "期望 CoffinStateChanged.grade=None（离棺），实际 {:?}",
            state_events[0].grade
        );
    }

    /// 主动归隐（voluntary_retire / Terminate 决策）后 coffin 状态必须全部清除：
    ///   - CoffinComponent 从 entity 移除（期望：None）
    ///   - CoffinRegistry.player_in_coffin 不含该 entity（期望：None）
    ///   - CoffinStateChanged(grade=None) 被发出（期望：1 条）
    #[test]
    fn voluntary_retire_terminate_clears_coffin_state() {
        let mut app = App::new();
        coffin_setup_base(&mut app, 700);
        app.insert_resource(crate::coffin::CoffinRegistry::default());

        let lower = valence::prelude::BlockPos::new(25, 64, 25);
        let entity = app
            .world_mut()
            .spawn((
                Wounds {
                    health_current: 1.0,
                    health_max: 30.0,
                    entries: Vec::new(),
                },
                Stamina::default(),
                CombatState::default(),
                Lifecycle {
                    character_id: "offline:VolRetire".to_string(),
                    state: LifecycleState::AwaitingRevival,
                    // Tribulation 决策 + fortune_remaining=0 → can_terminate()=true
                    awaiting_decision: Some(RevivalDecision::Tribulation { chance: 0.5 }),
                    revival_decision_deadline_tick: Some(800),
                    fortune_remaining: 0,
                    ..Default::default()
                },
                Cultivation {
                    realm: Realm::Awaken,
                    qi_current: 10.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                MeridianSystem::default(),
                Contamination::default(),
                LifeRecord::new("offline:VolRetire"),
                crate::coffin::CoffinComponent {
                    entered_at_tick: 650,
                    coffin_lower: lower,
                    grade: crate::coffin::CoffinGrade::Mundane,
                },
            ))
            .id();

        {
            let mut reg = app
                .world_mut()
                .resource_mut::<crate::coffin::CoffinRegistry>();
            reg.insert(lower, 0, crate::coffin::CoffinGrade::Mundane);
            reg.set_occupied(lower, entity);
        }

        // 发 Terminate（主动归隐）
        app.world_mut().send_event(RevivalActionIntent {
            entity,
            action: RevivalActionKind::Terminate,
            issued_at_tick: 700,
        });
        app.update();

        // 验证：CoffinComponent 已移除
        assert!(
            app.world()
                .entity(entity)
                .get::<crate::coffin::CoffinComponent>()
                .is_none(),
            "期望 voluntary_retire 后 CoffinComponent=None，实际仍存在"
        );

        // 验证：Registry 已清空
        let reg = app.world().resource::<crate::coffin::CoffinRegistry>();
        assert!(
            !reg.player_in_coffin.contains_key(&entity),
            "期望 voluntary_retire 后 player_in_coffin 不含 entity，实际仍含"
        );

        // 验证：CoffinStateChanged(grade=None) 被发出
        let state_events = app
            .world_mut()
            .resource_mut::<Events<crate::coffin::CoffinStateChanged>>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(
            state_events.len(),
            1,
            "期望 voluntary_retire 发出 1 条 CoffinStateChanged，实际 {} 条",
            state_events.len()
        );
        assert!(
            state_events[0].grade.is_none(),
            "期望 CoffinStateChanged.grade=None（离棺），实际 {:?}",
            state_events[0].grade
        );
    }

    // ─────────── must_fix #3a: SQLite in_coffin 持久化契约锁住（带 Username + PlayerStatePersistence）─────────

    /// 棺内玩家复活后，SQLite in_coffin 列必须被写为 false（0）。
    /// 这是重启后唯一的权威来源（player/mod.rs:258 读 in_coffin=true 即重新复钉）。
    ///
    /// 断言：load_player_slices(...).in_coffin == false（回读 SQLite，不依赖内存状态）
    #[test]
    fn revive_with_username_clears_sqlite_in_coffin() {
        let (settings, root) = persistence_settings("sqlite-coffin-revive");
        let data_dir = root.join("data");
        let mut app = revival_action_test_app(settings.clone(), 500);

        let username = Username("SQLiteCoffinRevive".to_string());
        let lifespan = crate::cultivation::lifespan::LifespanComponent {
            born_at_tick: 0,
            years_lived: 20.0,
            cap_by_realm: crate::cultivation::lifespan::LifespanCapTable::AWAKEN,
            offline_pause_tick: None,
        };
        let lower = valence::prelude::BlockPos::new(40, 64, 40);

        // 先写入 in_coffin=true 到 SQLite（模拟玩家断线前已入棺状态）
        crate::player::state::save_player_lifespan_slice_with_coffin(
            &PlayerStatePersistence::with_db_path(&data_dir, settings.db_path()),
            username.0.as_str(),
            &lifespan,
            Some(crate::coffin::CoffinGrade::Mundane),
        )
        .expect("pre-populate in_coffin=true 应成功");

        // 验证前置条件：SQLite 已有 in_coffin=true
        let before = crate::player::state::load_player_slices(
            &PlayerStatePersistence::with_db_path(&data_dir, settings.db_path()),
            username.0.as_str(),
        );
        assert!(
            before.in_coffin,
            "前置条件：SQLite in_coffin 应为 true（已写入），实际 false"
        );

        // 构造完整原子复活 bundle，并附加棺材持久化切片
        let (entity, _helper) = spawn_revival_action_actor(
            &mut app,
            username.0.as_str(),
            RevivalActionActorState {
                lifecycle: Lifecycle {
                    character_id: "offline:SQLiteCoffinRevive".to_string(),
                    state: LifecycleState::AwaitingRevival,
                    awaiting_decision: Some(RevivalDecision::Fortune { chance: 1.0 }),
                    revival_decision_deadline_tick: Some(600),
                    fortune_remaining: 1,
                    ..Default::default()
                },
                cultivation: Cultivation {
                    realm: Realm::Awaken,
                    qi_current: 10.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                meridians: MeridianSystem::default(),
                contamination: Contamination::default(),
                life_record: LifeRecord::new("offline:SQLiteCoffinRevive"),
                nourishment: Nourishment::spawn_default(),
            },
        );
        app.world_mut().entity_mut(entity).insert((
            lifespan.clone(),
            crate::coffin::CoffinComponent {
                entered_at_tick: 400,
                coffin_lower: lower,
                grade: crate::coffin::CoffinGrade::Mundane,
            },
        ));

        {
            let mut reg = crate::coffin::CoffinRegistry::default();
            reg.insert(lower, 0, crate::coffin::CoffinGrade::Mundane);
            reg.set_occupied(lower, entity);
            app.insert_resource(reg);
        }

        // 触发复活
        app.world_mut().send_event(RevivalActionIntent {
            entity,
            action: RevivalActionKind::Reincarnate,
            issued_at_tick: 500,
        });
        app.update();

        // 核心断言：SQLite in_coffin 必须为 false（回读验证，不依赖 ECS 内存）
        let after = crate::player::state::load_player_slices(
            &PlayerStatePersistence::with_db_path(&data_dir, settings.db_path()),
            username.0.as_str(),
        );
        assert!(
            !after.in_coffin,
            "期望复活后 SQLite in_coffin=false（重启不应再复钉），实际 in_coffin=true"
        );
        assert!(
            after.coffin_grade.is_none(),
            "期望复活后 SQLite coffin_grade=None（清棺），实际 {:?}",
            after.coffin_grade
        );

        let _ = fs::remove_dir_all(root);
    }

    /// 劫数不过 terminate 后，SQLite in_coffin 列必须被写为 false（0）。
    /// 同 revive_with_username_clears_sqlite_in_coffin，但走 terminate 路径。
    #[test]
    fn terminate_tribulation_failed_with_username_clears_sqlite_in_coffin() {
        let mut app = App::new();
        let (settings, root) = persistence_settings("sqlite-coffin-term");
        let data_dir = root.join("data");

        app.insert_resource(settings.clone());
        app.insert_resource(PlayerStatePersistence::with_db_path(
            &data_dir,
            settings.db_path(),
        ));
        app.insert_resource(CombatClock { tick: 600 });
        app.add_event::<RevivalActionIntent>();
        app.add_event::<PlayerRevived>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<crate::coffin::CoffinStateChanged>();
        app.add_systems(Update, handle_revival_action_intents);

        let username = Username("SQLiteCoffinTerm".to_string());
        let lifespan = crate::cultivation::lifespan::LifespanComponent {
            born_at_tick: 0,
            years_lived: 80.0,
            cap_by_realm: crate::cultivation::lifespan::LifespanCapTable::AWAKEN,
            offline_pause_tick: None,
        };
        let lower = valence::prelude::BlockPos::new(50, 64, 50);

        // 先写入 in_coffin=true 到 SQLite
        crate::player::state::save_player_lifespan_slice_with_coffin(
            &PlayerStatePersistence::with_db_path(&data_dir, settings.db_path()),
            username.0.as_str(),
            &lifespan,
            Some(crate::coffin::CoffinGrade::Jade),
        )
        .expect("pre-populate in_coffin=true 应成功");

        // 前置验证
        let before = crate::player::state::load_player_slices(
            &PlayerStatePersistence::with_db_path(&data_dir, settings.db_path()),
            username.0.as_str(),
        );
        assert!(
            before.in_coffin,
            "前置条件：SQLite in_coffin 应为 true，实际 false"
        );

        let entity = app
            .world_mut()
            .spawn((
                Wounds {
                    health_current: 1.0,
                    health_max: 30.0,
                    entries: Vec::new(),
                },
                Stamina::default(),
                CombatState::default(),
                Lifecycle {
                    character_id: "offline:SQLiteCoffinTerm".to_string(),
                    state: LifecycleState::AwaitingRevival,
                    // chance=0 → roll 必失 → terminate 分支
                    awaiting_decision: Some(RevivalDecision::Tribulation { chance: 0.0 }),
                    revival_decision_deadline_tick: Some(700),
                    fortune_remaining: 0,
                    ..Default::default()
                },
                Cultivation {
                    realm: Realm::Awaken,
                    qi_current: 10.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                MeridianSystem::default(),
                Contamination::default(),
                LifeRecord::new("offline:SQLiteCoffinTerm"),
                lifespan.clone(),
                username.clone(),
                crate::coffin::CoffinComponent {
                    entered_at_tick: 550,
                    coffin_lower: lower,
                    grade: crate::coffin::CoffinGrade::Jade,
                },
            ))
            .id();

        {
            let mut reg = crate::coffin::CoffinRegistry::default();
            reg.insert(lower, 0, crate::coffin::CoffinGrade::Jade);
            reg.set_occupied(lower, entity);
            app.insert_resource(reg);
        }

        // 触发 Reincarnate（chance=0 → 必走 terminate 分支）
        app.world_mut().send_event(RevivalActionIntent {
            entity,
            action: RevivalActionKind::Reincarnate,
            issued_at_tick: 600,
        });
        app.update();

        // 核心断言：SQLite in_coffin 必须为 false
        let after = crate::player::state::load_player_slices(
            &PlayerStatePersistence::with_db_path(&data_dir, settings.db_path()),
            username.0.as_str(),
        );
        assert!(
            !after.in_coffin,
            "期望 tribulation_failed terminate 后 SQLite in_coffin=false（重启不应复钉），实际 true"
        );
        assert!(
            after.coffin_grade.is_none(),
            "期望 terminate 后 SQLite coffin_grade=None，实际 {:?}",
            after.coffin_grade
        );

        let _ = fs::remove_dir_all(root);
    }
}
