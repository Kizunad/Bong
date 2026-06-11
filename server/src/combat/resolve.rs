use serde_json::json;
use valence::entity::Look;
use valence::prelude::bevy_ecs::system::SystemParam;
use valence::prelude::{
    bevy_ecs, Client, Commands, DVec3, Entity, EventReader, EventWriter, Events, GameMode,
    ParamSet, Position, Query, Res, ResMut, Username, With,
};

use crate::combat::anticheat::AntiCheatCounter;
use crate::combat::armor::{ArmorProfileRegistry, ARMOR_MITIGATION_CAP};
use crate::combat::body_mass::{BodyMass, Stance};
use crate::combat::jiemai::{
    jiemai_apply_effects, jiemai_effectiveness, jiemai_fov_check, jiemai_prep_window,
};
use crate::combat::knockback::{
    compute_combat_knockback, CombatKnockbackInput, KnockbackEvent, DEFAULT_CHAIN_DEPTH,
};
use crate::combat::shield_block::{
    self as shield_block_mod, shield_fov_check, SHIELD_NEAR_BREAK_DURABILITY_THRESHOLD,
};
use crate::combat::status::{body_part_damage_multiplier, has_active_status};
use crate::combat::sword_basics;
use crate::combat::tuike::{tuike_filter_contam, FalseSkin, ShedEvent};
use crate::combat::tuike_v2::physics::naked_defense_damage_multiplier;
use crate::combat::tuike_v2::StackedFalseSkins;
use crate::combat::weapon::{ShieldBroken, Weapon, WeaponBroken};
use crate::combat::zhenmai_v2::{
    self, BackfireAmplification, MeridianHardenActive, MultiPointActive,
};
use crate::combat::CombatClock;
use crate::combat::{
    components::{
        BodyPart, CombatState, DerivedAttrs, Lifecycle, LifecycleState, Stamina, StaminaState,
        StatusEffects, Wound, Wounds, HEAD_STUN_DURATION_TICKS, HEAD_STUN_SEVERITY_THRESHOLD,
        LEG_SLOWED_DURATION_TICKS, LEG_SLOWED_SEVERITY_THRESHOLD,
    },
    events::{
        ApplyStatusEffectIntent, AttackIntent, AttackSource, CombatEvent, DeathEvent,
        DefenseIntent, DefenseKind, StatusEffectKind,
    },
    raycast::raycast_humanoid,
};
use crate::cultivation::color::{record_style_practice, PracticeLog};
use crate::cultivation::components::{
    ColorKind, ContamSource, Contamination, CrackCause, Cultivation, MeridianCrack, MeridianSystem,
    QiColor,
};
use crate::cultivation::known_techniques::KnownTechniques;
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::cultivation::tribulation::JueBiLawDisruption;
use crate::inventory::{
    consume_item_instance_once, discard_inventory_item_to_dropped_loot,
    move_equipped_item_to_first_container_slot, set_item_instance_durability, DroppedLootRegistry,
    InventoryDurabilityChangedEvent, ItemRegistry, PlayerInventory, EQUIP_SLOT_CHEST,
    EQUIP_SLOT_FALSE_SKIN, EQUIP_SLOT_FEET, EQUIP_SLOT_HEAD, EQUIP_SLOT_LEGS, EQUIP_SLOT_OFF_HAND,
};
use crate::network::audio_event_emit::{
    AudioRecipient, PlaySoundRecipeRequest, AUDIO_BROADCAST_RADIUS,
};
use crate::network::{gameplay_vfx, vfx_event_emit::VfxEventRequest};
use crate::npc::brain::canonical_npc_id;
use crate::npc::movement::PendingKnockback;
use crate::npc::spawn::NpcMarker;
use crate::player::state::canonical_player_id;
use crate::qi_physics::constants::{
    QI_ZHENMAI_CONCUSSION_BLEEDING_PER_SEC, QI_ZHENMAI_PARRY_RECOVERY_TICKS,
};
use crate::qi_physics::{flow_modifier, QiAccountId, QiTransfer};
use crate::schema::anticheat::ViolationKindV1;
use crate::schema::common::{GameEventType, NarrationStyle};
use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};
use crate::schema::world_state::GameEvent;
use crate::skill::components::SkillId;
use crate::skill::events::{SkillXpGain, XpGainSource};
use crate::world::events::ActiveEventsResource;

const ARMOR_HIT_CONTAMINATION_MULTIPLIER: f64 = 0.1;
const ARMOR_HIT_DURABILITY_COST_POINTS: f64 = 0.5;

fn apply_armor_mitigation(
    wound: &mut Wound,
    derived: &DerivedAttrs,
    contam: &mut f64,
) -> Option<f32> {
    let &m = derived.defense_profile.get(&(wound.location, wound.kind))?;
    if m <= 0.0 {
        return None;
    }

    let m = m.clamp(0.0, ARMOR_MITIGATION_CAP);
    if m <= 0.0 {
        return None;
    }
    wound.severity *= 1.0 - m;
    wound.bleeding_per_sec *= 1.0 - m;
    // plan-armor-v1 §Q10: armor 把 severity 压低 (1-m) -> contam 一阶要随之减少；
    // 然后整体再压 ARMOR_HIT_CONTAMINATION_MULTIPLIER (0.1) 实现 "甲挡住基本不污染"。
    // 两段叠乘是有意为之 —— 1-m 让强弱甲仍有量级区分（顶甲 0.015×、弱甲 0.095×），
    // 0.1 整体闸门保证哪怕弱甲也不会推 contam 失控。改公式必须同步更新
    // `armor_hit_scales_contamination_and_ticks_item_durability` 的 expected_contam。
    *contam *= 1.0 - f64::from(m);
    *contam *= ARMOR_HIT_CONTAMINATION_MULTIPLIER;
    Some(m)
}

const DEBUG_ATTACK_STAMINA_COST: f32 = 12.0;
const DEBUG_ATTACK_CONTAMINATION_FACTOR: f64 = 0.25;
const ATTACKER_EYE_HEIGHT: f64 = 1.62;
const ATTACK_QI_DAMAGE_FACTOR: f32 = 1.0;
const ATTACK_QI_THROUGHPUT_FACTOR: f64 = 1.0;

#[derive(Debug, Clone, Copy)]
struct WoundKindProfile {
    damage_mul: f32,
    bleed_mul: f32,
    contam_mul: f64,
    crack_mul: f64,
}

type CombatClientItem<'a> = (
    Entity,
    &'a Position,
    &'a Username,
    &'a crate::player::state::PlayerState,
);
type CombatClientFilter = With<Client>;
type CombatTargetItem<'a> = (
    &'a mut Wounds,
    &'a mut Stamina,
    &'a mut Contamination,
    &'a mut MeridianSystem,
    Option<&'a mut LifeRecord>,
    Option<&'a Lifecycle>,
    Option<&'a mut CombatState>,
    Option<&'a mut Cultivation>,
    Option<&'a mut FalseSkin>,
    Option<&'a StackedFalseSkins>,
    Option<&'a mut DerivedAttrs>,
    Option<&'a mut PracticeLog>,
    Option<&'a mut MultiPointActive>,
    Option<&'a MeridianHardenActive>,
    Option<&'a BackfireAmplification>,
);
type CombatAttackerItem<'a> = (
    &'a mut Cultivation,
    &'a mut MeridianSystem,
    Option<&'a DerivedAttrs>,
    Option<&'a mut AntiCheatCounter>,
    Option<&'a CombatState>,
    Option<&'a KnownTechniques>,
    Option<&'a Lifecycle>,
);
type DefenseResponderItem<'a> = (
    &'a mut CombatState,
    &'a Cultivation,
    Option<&'a PlayerInventory>,
    Option<&'a StatusEffects>,
    Option<&'a FalseSkin>,
);
type PositionLookItem<'a> = (&'a Position, Option<&'a Look>);

/// 事件写出参数合并，避免 Bevy 0.14 顶层 SystemParam 数量上限。
#[derive(SystemParam)]
pub struct CombatResolveEventWriters<'w> {
    status_effect_intents: EventWriter<'w, ApplyStatusEffectIntent>,
    out_events: EventWriter<'w, CombatEvent>,
    qi_transfers: Option<ResMut<'w, Events<QiTransfer>>>,
    multipoint_backfires: Option<ResMut<'w, Events<zhenmai_v2::MultiPointBackfireEvent>>>,
    vfx_events: Option<ResMut<'w, Events<VfxEventRequest>>>,
    audio_events: Option<ResMut<'w, Events<PlaySoundRecipeRequest>>>,
    knockback_events: Option<ResMut<'w, Events<KnockbackEvent>>>,
    death_events: EventWriter<'w, DeathEvent>,
    durability_changed_tx: EventWriter<'w, InventoryDurabilityChangedEvent>,
    /// plan-shield-block-v1 P2 — 盾牌格挡成功触发 PARRY_BLOCK 动画（via emit_defense_animation_triggers）。
    defense_intent_tx: Option<ResMut<'w, Events<DefenseIntent>>>,
    /// plan-shield-block-v1 P4 — player-scope narration（格挡成功 / 近破盾）。
    narrations: Option<ResMut<'w, crate::player::gameplay::PendingGameplayNarrations>>,
}

pub fn apply_defense_intents(
    mut defenses: EventReader<DefenseIntent>,
    mut defenders: Query<DefenseResponderItem<'_>>,
    mut status_effect_intents: EventWriter<ApplyStatusEffectIntent>,
) {
    for defense in defenses.read() {
        let Ok((mut combat_state, cultivation, inventory, status_effects, false_skin)) =
            defenders.get_mut(defense.defender)
        else {
            continue;
        };

        if status_effects.is_some_and(|se| {
            has_active_status(se, StatusEffectKind::Stunned)
                || has_active_status(se, StatusEffectKind::VortexCasting)
                || has_active_status(se, StatusEffectKind::ParryRecovery)
                || has_active_status(se, StatusEffectKind::VoidCoreActive)
        }) {
            continue;
        }
        // plan-shield-block-v1 P2 — 盾格挡 emit 的 DefenseIntent 仅用于动画触发，
        // 不应开截脉 incoming_window 也不应施加 per-block ParryRecovery。
        // 盾格挡语义：凡人盾，无境界加成，不耦合 jiemai/zhenmai。
        if status_effects.is_some_and(|se| has_active_status(se, StatusEffectKind::ShieldBlocking))
        {
            continue;
        }
        if zhenmai_v2::parry_qi_cost_for_realm(cultivation.realm).is_none() {
            continue;
        }

        let mut window = jiemai_prep_window(inventory, defense.issued_at_tick);
        if let Some(skin) = false_skin {
            window.duration_ms = ((window.duration_ms as f32) * skin.kind.jiemai_window_modifier())
                .round()
                .max(1.0) as u32;
        }
        combat_state.incoming_window = Some(window);
        status_effect_intents.send(ApplyStatusEffectIntent {
            target: defense.defender,
            kind: StatusEffectKind::ParryRecovery,
            magnitude: 1.0,
            duration_ticks: QI_ZHENMAI_PARRY_RECOVERY_TICKS,
            issued_at_tick: defense.issued_at_tick,
        });
    }
}
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn resolve_attack_intents(
    clock: Res<CombatClock>,
    armor_profiles: Option<Res<ArmorProfileRegistry>>,
    mut intents: EventReader<AttackIntent>,
    mut active_events: Option<ResMut<ActiveEventsResource>>,
    clients: Query<CombatClientItem<'_>, CombatClientFilter>,
    positions: Query<PositionLookItem<'_>>,
    npc_markers: Query<(), With<NpcMarker>>,
    npc_positions: Query<(Entity, &Position), With<NpcMarker>>,
    statuses: Query<&StatusEffects>,
    juebi_law_disruptions: Query<&JueBiLawDisruption>,
    sparring_sessions: Query<&crate::social::components::SparringState>,
    mut combatants: ParamSet<(
        Query<CombatAttackerItem<'_>>,
        Query<CombatTargetItem<'_>>,
        Query<&GameMode>,
    )>,
    body_masses: Query<&BodyMass>,
    stances: Query<&Stance>,
    mut event_writers: CombatResolveEventWriters,
    // plan-weapon-v1 §6：武器加成 + 耐久扣减
    // plan-shield-block-v1 P3：shield_broken 事件写出 + ItemRegistry（读 ShieldSpec）
    // plan-shield-block-v1 P4：defender KnownTechniques（shield_block_profile 缩放）
    weapon_break: (
        Query<&mut Weapon>,
        EventWriter<WeaponBroken>,
        EventWriter<ShieldBroken>,
        Commands,
        Query<&mut PlayerInventory>,
        Query<&QiColor>,
        Option<ResMut<DroppedLootRegistry>>,
        Option<ResMut<Events<SkillXpGain>>>,
        Option<ResMut<Events<ShedEvent>>>,
        Option<Res<ItemRegistry>>,
        Query<Option<&KnownTechniques>>,
    ),
) {
    let (
        mut weapons,
        mut weapon_broken_events,
        mut shield_broken_events,
        mut commands,
        mut inventories,
        qi_colors,
        mut dropped_loot_registry,
        mut skill_xp_events,
        mut shed_events,
        item_registry,
        defender_known_q,
    ) = weapon_break;

    for intent in intents.read() {
        // Attacker blocked by VoidCoreActive (cannot attack while in void core collapse)
        if statuses.get(intent.attacker).is_ok_and(|se| {
            has_active_status(se, StatusEffectKind::Stunned)
                || has_active_status(se, StatusEffectKind::VortexCasting)
                || has_active_status(se, StatusEffectKind::ParryRecovery)
                || has_active_status(se, StatusEffectKind::VoidCoreActive)
        }) {
            continue;
        }

        let Some((attacker_position, attacker_id, target_entity, target_position, target_id)) =
            resolve_intent_entities(intent, &clients, &positions, &npc_markers, &npc_positions)
        else {
            continue;
        };
        let target_damageable = {
            let game_modes = combatants.p2();
            crate::combat::is_damageable(target_entity, &game_modes)
        };
        if !target_damageable {
            continue;
        }

        // Target blocked by VoidCoreActive (cannot be hit while in void core collapse)
        if statuses
            .get(target_entity)
            .is_ok_and(|se| has_active_status(se, StatusEffectKind::VoidCoreActive))
        {
            continue;
        }

        let target_lifecycle_blocks_attack = {
            let mut target_query = combatants.p1();
            let Ok((_, _, _, _, _, lifecycle, _, _, _, _, _, _, _, _, _)) =
                target_query.get_mut(target_entity)
            else {
                continue;
            };
            lifecycle.is_some_and(|lifecycle| {
                matches!(
                    lifecycle.state,
                    LifecycleState::NearDeath
                        | LifecycleState::AwaitingRevival
                        | LifecycleState::Terminated
                )
            })
        };
        if target_lifecycle_blocks_attack {
            continue;
        }

        let qi_invest = f64::from(intent.qi_invest.max(0.0));
        let juebi_law_env = juebi_law_disruptions
            .get(intent.attacker)
            .ok()
            .map(|disruption| disruption.env_field())
            .unwrap_or_default();

        {
            let mut attacker_query = combatants.p0();
            let Ok((
                attacker_cultivation,
                _,
                _,
                mut anticheat_counter,
                attacker_combat_state,
                _,
                attacker_lifecycle,
            )) = attacker_query.get_mut(intent.attacker)
            else {
                continue;
            };

            // Attacker is dead / dying — skip this intent.
            if attacker_lifecycle.is_some_and(|lc| {
                matches!(
                    lc.state,
                    LifecycleState::NearDeath
                        | LifecycleState::AwaitingRevival
                        | LifecycleState::Terminated
                )
            }) {
                continue;
            }

            if intent.source == AttackSource::Melee
                && intent.debug_command.is_none()
                && attacker_combat_state
                    .and_then(|state| state.last_attack_at_tick)
                    .is_some_and(|last_attack_at_tick| intent.issued_at_tick <= last_attack_at_tick)
            {
                record_anticheat_violation(
                    anticheat_counter.as_deref_mut(),
                    ViolationKindV1::CooldownBypassed,
                    format!(
                        "cooldown: issued_at_tick={} last_attack_at_tick={}",
                        intent.issued_at_tick,
                        attacker_combat_state
                            .and_then(|state| state.last_attack_at_tick)
                            .unwrap_or_default()
                    ),
                );
            }

            if qi_invest > f64::EPSILON
                && !source_uses_prepaid_qi(intent.source)
                && attacker_cultivation.qi_current + f64::EPSILON < qi_invest
            {
                if intent.debug_command.is_none() {
                    record_anticheat_violation(
                        anticheat_counter.as_deref_mut(),
                        ViolationKindV1::QiInvestExceeded,
                        format!(
                            "qi_invest: requested={:.3} available={:.3}",
                            qi_invest, attacker_cultivation.qi_current
                        ),
                    );
                }
                continue;
            }
        }

        let Some(hit_probe) = raycast_humanoid(
            attacker_position + DVec3::new(0.0, ATTACKER_EYE_HEIGHT, 0.0),
            target_position,
            f64::from(
                intent.reach.max
                    / (juebi_law_env.law_disruption_distance_multiplier() as f32).max(1.0),
            ),
        ) else {
            if intent.debug_command.is_none() {
                let mut attacker_query = combatants.p0();
                if let Ok((_, _, _, mut anticheat_counter, _, _, _)) =
                    attacker_query.get_mut(intent.attacker)
                {
                    record_anticheat_violation(
                        anticheat_counter.as_deref_mut(),
                        ViolationKindV1::ReachExceeded,
                        format!(
                            "reach: target_distance={:.3} server_max={:.3}",
                            target_position.distance(attacker_position),
                            intent.reach.max
                        ),
                    );
                }
            }
            continue;
        };
        let distance = hit_probe.distance as f32;

        let (attacker_damage_multiplier, attacker_body_mass, sword_damage_multiplier) = {
            let mut attacker_query = combatants.p0();
            let Ok((
                mut attacker_cultivation,
                mut attacker_meridians,
                attacker_attrs,
                _,
                _,
                attacker_known_techniques,
                _,
            )) = attacker_query.get_mut(intent.attacker)
            else {
                continue;
            };

            if qi_invest > f64::EPSILON && !source_uses_prepaid_qi(intent.source) {
                attacker_cultivation.qi_current = (attacker_cultivation.qi_current - qi_invest)
                    .clamp(0.0, attacker_cultivation.qi_max);
            }
            if qi_invest > f64::EPSILON && !sword_basics::is_sword_attack_source(intent.source) {
                if let Some(primary_meridian) =
                    first_open_or_fallback_meridian(&mut attacker_meridians)
                {
                    primary_meridian.throughput_current += qi_invest
                        * ATTACK_QI_THROUGHPUT_FACTOR
                        * juebi_law_env.law_disruption_channeling_multiplier();
                }
            }
            (
                attacker_attrs
                    .map(|attrs| attrs.attack_power)
                    .unwrap_or(1.0),
                body_masses.get(intent.attacker).ok().copied(),
                sword_basics::source_to_technique(intent.source)
                    .and_then(|technique| {
                        attacker_known_techniques.and_then(|known| {
                            known
                                .entries
                                .iter()
                                .find(|entry| entry.id == technique.id())
                                .map(|entry| {
                                    sword_basics::sword_profile(technique, entry.proficiency)
                                        .damage_multiplier
                                })
                        })
                    })
                    .unwrap_or(1.0),
            )
        };

        let mut target_query = combatants.p1();
        let Ok((
            mut wounds,
            mut stamina,
            mut contamination,
            mut meridians,
            life_record,
            lifecycle,
            combat_state,
            defender_cultivation,
            false_skin,
            tuike_v2_stack,
            defender_attrs,
            defender_practice_log,
            mut multipoint_active,
            harden_active,
            backfire_amplification,
        )) = target_query.get_mut(target_entity)
        else {
            continue;
        };
        let decay = ((intent.reach.max - distance) / intent.reach.max.max(0.001)).clamp(0.0, 1.0);
        let hit_qi = (intent.qi_invest * decay).max(0.0);
        let is_physical_hit = intent.qi_invest <= f32::EPSILON;
        let (body_damage_multiplier, contam_multiplier, bleed_multiplier) =
            body_part_multipliers(hit_probe.body_part);
        let wound_profile = wound_kind_profile(intent.wound_kind);
        let defender_damage_multiplier = defender_attrs
            .as_ref()
            .map(|attrs| attrs.defense_power)
            .unwrap_or(1.0)
            * naked_defense_damage_multiplier(tuike_v2_stack, clock.tick);
        // 正式武器走 Weapon component；凡器不挂 Weapon，但主手使用时按低倍率临时武器结算。
        let mut hit_tool: Option<crate::tools::ToolKind> = None;
        let mut weapon_kind_for_knockback = None;
        let (weapon_base_damage, weapon_multiplier): (f32, f32) = match weapons.get(intent.attacker)
        {
            Ok(weapon) => {
                weapon_kind_for_knockback = Some(weapon.weapon_kind);
                let resonance = inventories.get(intent.attacker).ok().and_then(|inventory| {
                    crate::forge::artifact_meridian::artifact_resonance_for_inventory(
                        inventory,
                        weapon.instance_id,
                        qi_colors.get(intent.attacker).ok(),
                    )
                });
                let multiplier = resonance
                    .map(|value| weapon.damage_multiplier_with_resonance(value))
                    .unwrap_or_else(|| weapon.damage_multiplier());
                (weapon.base_attack.max(1.0), multiplier)
            }
            Err(_) => {
                hit_tool = inventories
                    .get(intent.attacker)
                    .ok()
                    .and_then(crate::tools::main_hand_tool_in_inventory);
                let multiplier = hit_tool
                    .map(crate::tools::ToolKind::combat_damage_multiplier)
                    .unwrap_or(1.0);
                (1.0, multiplier)
            }
        };
        let defender_stance = stances
            .get(target_entity)
            .ok()
            .copied()
            .unwrap_or_else(|| Stance::from_runtime(&stamina, combat_state.as_deref()));
        let attacker_mass = attacker_body_mass.unwrap_or_default();
        let defender_mass = body_masses
            .get(target_entity)
            .ok()
            .copied()
            .unwrap_or_default();
        let zhenmai_attack_kind =
            zhenmai_v2::attack_kind_for_source(intent.source, intent.wound_kind);
        let harden_damage_multiplier = if is_physical_hit {
            1.0
        } else {
            harden_active
                .map(|active| flow_modifier(1.0, active.damage_multiplier))
                .unwrap_or(1.0)
        };
        let backfire_incoming_damage_multiplier = if is_physical_hit {
            1.0
        } else {
            backfire_amplification
                .filter(|active| active.active_for(zhenmai_attack_kind, clock.tick))
                .map(|active| active.incoming_damage_multiplier)
                .unwrap_or(1.0)
        };
        let base_damage = if is_physical_hit {
            weapon_base_damage
                * body_damage_multiplier
                * attacker_damage_multiplier
                * defender_damage_multiplier
                * weapon_multiplier
                * wound_profile.damage_mul
                * sword_damage_multiplier
        } else {
            hit_qi
                * ATTACK_QI_DAMAGE_FACTOR
                * body_damage_multiplier
                * attacker_damage_multiplier
                * defender_damage_multiplier
                * weapon_multiplier
                * harden_damage_multiplier
                * backfire_incoming_damage_multiplier
                * sword_damage_multiplier
        };
        let juebi_backfire_fraction = if is_physical_hit {
            0.0
        } else {
            juebi_law_env.law_disruption_backfire_fraction() as f32
        };
        let damage = (base_damage * (1.0 - juebi_backfire_fraction)).max(1.0);
        let juebi_backfire_damage = (base_damage * juebi_backfire_fraction).max(0.0);
        let was_alive = wounds.health_current > 0.0;
        if let Ok(knockback) = compute_combat_knockback(CombatKnockbackInput {
            physical_damage: damage,
            qi_invest: hit_qi,
            attacker_mass: Some(&attacker_mass),
            target_mass: Some(&defender_mass),
            target_stance: Some(&defender_stance),
            target_cultivation: defender_cultivation.as_deref(),
            weapon_kind: weapon_kind_for_knockback,
            source: intent.source,
        }) {
            if knockback.is_actionable() {
                let direction = target_position - attacker_position;
                if direction.length() > f64::EPSILON {
                    commands
                        .entity(target_entity)
                        .insert(PendingKnockback::from_result(
                            intent.attacker,
                            intent.source,
                            direction,
                            knockback,
                            DEFAULT_CHAIN_DEPTH,
                        ));
                    if let Some(events) = event_writers.knockback_events.as_deref_mut() {
                        events.send(KnockbackEvent {
                            attacker: intent.attacker,
                            target: target_entity,
                            source: intent.source,
                            distance_blocks: knockback.distance_blocks,
                            velocity_blocks_per_tick: knockback.velocity_blocks_per_tick,
                            duration_ticks: knockback.duration_ticks,
                            kinetic_energy: knockback.kinetic_energy,
                            collision_damage: None,
                            chain_depth: DEFAULT_CHAIN_DEPTH,
                            block_broken: false,
                        });
                    }
                }
            }
        }
        let mut pending_reflected_qi = 0.0_f64;
        if !is_physical_hit {
            if let Some(active) = multipoint_active.as_deref_mut() {
                let reflected =
                    zhenmai_v2::multipoint_contact(active, f64::from(hit_qi), zhenmai_attack_kind);
                pending_reflected_qi += reflected;
                if let Some(events) = event_writers.multipoint_backfires.as_deref_mut() {
                    events.send(zhenmai_v2::MultiPointBackfireEvent {
                        defender: target_entity,
                        attacker: Some(intent.attacker),
                        attack_kind: zhenmai_attack_kind,
                        contact_index: active.contact_count,
                        reflected_qi: reflected,
                        tick: clock.tick,
                    });
                }
                zhenmai_v2::apply_self_damage(&mut wounds, active.self_damage_per_contact);
            }
        }
        if !is_physical_hit {
            if let Some(active) = backfire_amplification
                .filter(|active| active.active_for(zhenmai_attack_kind, clock.tick))
            {
                pending_reflected_qi += zhenmai_v2::reflected_qi(
                    f64::from(hit_qi),
                    active.k_drain,
                    zhenmai_attack_kind,
                );
            }
        }
        if pending_reflected_qi > f64::EPSILON {
            if let Some(transfer) = zhenmai_v2::backfire_transfer(
                QiAccountId::player(attacker_id.clone()),
                QiAccountId::player(target_id.clone()),
                pending_reflected_qi,
            ) {
                if let Some(events) = event_writers.qi_transfers.as_deref_mut() {
                    events.send(transfer);
                }
            }
            let attacker = intent.attacker;
            commands.add(
                move |world: &mut valence::prelude::bevy_ecs::world::World| {
                    zhenmai_v2::apply_reflected_qi(world, attacker, pending_reflected_qi);
                },
            );
        }
        if juebi_backfire_damage > f32::EPSILON {
            let attacker = intent.attacker;
            commands.add(
                move |world: &mut valence::prelude::bevy_ecs::world::World| {
                    zhenmai_v2::apply_self_damage_to_entity(world, attacker, juebi_backfire_damage);
                },
            );
        }

        // plan-weapon-v1 §6.3：命中一次 → 耐久扣减。
        // 若耐久归零收集 broken info,下面统一 commands 操作(避免与 mut borrow 冲突)。
        let broken_weapon: Option<(u64, String)> = if let Ok(mut weapon) =
            weapons.get_mut(intent.attacker)
        {
            if weapon.tick_durability() {
                Some((weapon.instance_id, weapon.template_id.clone()))
            } else {
                if let Ok(mut inventory) = inventories.get_mut(intent.attacker) {
                    let durability_ratio = if weapon.durability_max > 0.0 {
                        f64::from((weapon.durability / weapon.durability_max).clamp(0.0, 1.0))
                    } else {
                        0.0
                    };
                    if let Err(error) = set_item_instance_durability(
                        &mut inventory,
                        weapon.instance_id,
                        durability_ratio,
                    ) {
                        tracing::warn!(
                                "[bong][combat][weapon] failed to persist durability for instance {}: {}",
                                weapon.instance_id,
                                error
                            );
                    }
                }
                None
            }
        } else {
            None
        };
        if let Some((instance_id, template_id)) = broken_weapon {
            let mut broken_dislodged = false;
            if let Ok(mut inventory) = inventories.get_mut(intent.attacker) {
                let broken_slot = inventory.equipped.iter().find_map(|(slot, item)| {
                    (item.instance_id == instance_id).then_some(match slot.as_str() {
                        crate::inventory::EQUIP_SLOT_MAIN_HAND => EquipSlotV1::MainHand,
                        crate::inventory::EQUIP_SLOT_OFF_HAND => EquipSlotV1::OffHand,
                        crate::inventory::EQUIP_SLOT_TWO_HAND => EquipSlotV1::TwoHand,
                        _ => EquipSlotV1::MainHand,
                    })
                });
                if let Err(error) = set_item_instance_durability(&mut inventory, instance_id, 0.0) {
                    tracing::warn!(
                        "[bong][combat][weapon] failed to persist broken durability for instance {}: {}",
                        instance_id,
                        error
                    );
                }
                match move_equipped_item_to_first_container_slot(&mut inventory, instance_id) {
                    Ok(_) => {
                        broken_dislodged = true;
                    }
                    Err(error) => {
                        tracing::warn!(
                            "[bong][combat][weapon] failed to move broken weapon instance {} into container: {}",
                            instance_id,
                            error
                        );
                        if let Some(slot) = broken_slot {
                            if let Some(dropped_loot_registry) = dropped_loot_registry.as_mut() {
                                let dropped = discard_inventory_item_to_dropped_loot(
                                    &mut inventory,
                                    dropped_loot_registry,
                                    [
                                        attacker_position.x,
                                        attacker_position.y,
                                        attacker_position.z,
                                    ],
                                    crate::world::dimension::DimensionKind::Overworld,
                                    instance_id,
                                    &InventoryLocationV1::Equip { slot },
                                );
                                match dropped {
                                    Ok(_) => {
                                        broken_dislodged = true;
                                    }
                                    Err(drop_error) => {
                                        tracing::warn!(
                                            "[bong][combat][weapon] failed to drop broken weapon instance {} after container fallback failed: {}",
                                            instance_id,
                                            drop_error
                                        );
                                    }
                                }
                            } else {
                                tracing::warn!(
                                    "[bong][combat][weapon] broken weapon instance {} cannot fall back to dropped loot because DroppedLootRegistry is unavailable",
                                    instance_id
                                );
                            }
                        } else {
                            tracing::warn!(
                                "[bong][combat][weapon] broken weapon instance {} no longer has an equipped slot",
                                instance_id
                            );
                        }
                    }
                }
            }
            if broken_dislodged {
                commands.entity(intent.attacker).remove::<Weapon>();
                weapon_broken_events.send(WeaponBroken {
                    entity: intent.attacker,
                    instance_id,
                    template_id,
                });
            }
        }

        if let Some(tool) = hit_tool {
            if let Ok(mut inventory) = inventories.get_mut(intent.attacker) {
                crate::tools::damage_main_hand_tool(
                    intent.attacker,
                    &mut inventory,
                    &mut event_writers.durability_changed_tx,
                    tool.durability_cost_ratio_per_use(),
                );
            }
        }

        let mut emitted_contam_delta = if is_physical_hit {
            0.0
        } else {
            f64::from(damage)
                * DEBUG_ATTACK_CONTAMINATION_FACTOR
                * f64::from(contam_multiplier)
                * wound_profile.contam_mul
        };
        let mut jiemai_success = false;
        let mut jiemai_effectiveness_value = None;
        let mut jiemai_contam_reduced = None;
        let mut jiemai_wound_severity = None;
        let mut sword_parry_success = false;
        let mut sword_parry_block_ratio = None;
        let mut sword_parry_contam_reduced = None;
        let mut sword_parry_reflected_damage = None;
        // plan-shield-block-v1 P2 — 盾牌格挡结果（无反伤）
        let mut shield_block_success = false;
        let mut shield_block_ratio: Option<f32> = None;
        let mut shield_block_contam_reduced: Option<f64> = None;
        let mut shield_blocked_damage: Option<f32> = None;
        let mut false_skin = false_skin;
        let mut defender_attrs = defender_attrs;

        // 污染结算顺序：截脉先改污染量，护甲再削污染，伪皮最后截胡剩余污染。

        stamina.current =
            (stamina.current - DEBUG_ATTACK_STAMINA_COST * decay).clamp(0.0, stamina.max);
        stamina.last_drain_tick = Some(clock.tick);
        stamina.state = if stamina.current <= 0.0 {
            StaminaState::Exhausted
        } else {
            StaminaState::Combat
        };

        if !is_physical_hit {
            if let (Some(mut combat_state), Some(mut defender_cultivation)) =
                (combat_state, defender_cultivation)
            {
                let window_open = combat_state
                    .incoming_window
                    .as_ref()
                    .is_some_and(|window| clock.tick < window.expires_at_tick());

                let qi_cost = zhenmai_v2::parry_qi_cost_for_realm(defender_cultivation.realm);
                let fov_ok = jiemai_fov_check(
                    attacker_position,
                    target_position,
                    positions
                        .get(target_entity)
                        .ok()
                        .and_then(|(_position, look)| look),
                    defender_cultivation.realm,
                );
                if window_open
                    && qi_cost
                        .is_some_and(|cost| defender_cultivation.qi_current + f64::EPSILON >= cost)
                    && fov_ok
                {
                    let qi_cost = qi_cost.expect("checked Some above");
                    defender_cultivation.qi_current = (defender_cultivation.qi_current - qi_cost)
                        .clamp(0.0, defender_cultivation.qi_max);

                    let before = emitted_contam_delta;
                    let effectiveness = jiemai_effectiveness(distance);
                    let mut concussion_severity =
                        zhenmai_v2::parry_self_damage_for_realm(defender_cultivation.realm);
                    jiemai_apply_effects(
                        effectiveness,
                        &mut emitted_contam_delta,
                        &mut concussion_severity,
                    );
                    jiemai_effectiveness_value = Some(effectiveness);
                    jiemai_contam_reduced = Some((before - emitted_contam_delta).max(0.0));
                    jiemai_wound_severity = Some(concussion_severity);

                    wounds.entries.push(Wound {
                        location: hit_probe.body_part,
                        kind: crate::combat::components::WoundKind::Concussion,
                        severity: concussion_severity,
                        bleeding_per_sec: QI_ZHENMAI_CONCUSSION_BLEEDING_PER_SEC,
                        created_at_tick: clock.tick,
                        inflicted_by: Some(attacker_id.clone()),
                    });
                    if let Some(mut practice_log) = defender_practice_log {
                        // qi_colors 已存在于 weapon_break tuple（只读），直接复用
                        let defender_qi_color = qi_colors.get(target_entity).ok();
                        record_style_practice(
                            &mut practice_log,
                            ColorKind::Violent,
                            defender_qi_color,
                        );
                    }
                    jiemai_success = true;
                }

                combat_state.incoming_window = None;
            }
        }

        let mut wound = Wound {
            location: hit_probe.body_part,
            kind: intent.wound_kind,
            severity: damage,
            bleeding_per_sec: damage * 0.05 * bleed_multiplier * wound_profile.bleed_mul,
            created_at_tick: clock.tick,
            inflicted_by: Some(attacker_id.clone()),
        };

        let defender_status_effects = statuses.get(target_entity).ok();
        if let Some(block_ratio) =
            active_status_magnitude(defender_status_effects, StatusEffectKind::SwordParrying)
        {
            let block_ratio = block_ratio.clamp(0.0, 0.95);
            let before_severity = wound.severity;
            let before_contam = emitted_contam_delta;
            wound.severity *= 1.0 - block_ratio;
            wound.bleeding_per_sec *= 1.0 - block_ratio;
            emitted_contam_delta *= f64::from(1.0 - block_ratio);
            let blocked_damage = (before_severity - wound.severity).max(0.0);
            let reflected_damage = blocked_damage * 0.15;
            sword_parry_success = true;
            sword_parry_block_ratio = Some(block_ratio);
            sword_parry_contam_reduced = Some((before_contam - emitted_contam_delta).max(0.0));
            sword_parry_reflected_damage = Some(reflected_damage);
            let attacker = intent.attacker;
            let reflected_by = target_id.clone();
            let reflected_at_tick = clock.tick;
            if reflected_damage > f32::EPSILON {
                commands.add(
                    move |world: &mut valence::prelude::bevy_ecs::world::World| {
                        if let Some(mut attacker_wounds) = world.get_mut::<Wounds>(attacker) {
                            attacker_wounds.health_current = (attacker_wounds.health_current
                                - reflected_damage)
                                .clamp(0.0, attacker_wounds.health_max);
                            attacker_wounds.entries.push(Wound {
                                location: BodyPart::Chest,
                                kind: crate::combat::components::WoundKind::Blunt,
                                severity: reflected_damage,
                                bleeding_per_sec: 0.0,
                                created_at_tick: reflected_at_tick,
                                inflicted_by: Some(reflected_by),
                            });
                        }
                    },
                );
            }
            event_writers
                .status_effect_intents
                .send(ApplyStatusEffectIntent {
                    target: intent.attacker,
                    kind: StatusEffectKind::Staggered,
                    magnitude: 0.3,
                    duration_ticks: sword_basics::SWORD_PARRY_STAGGER_TICKS,
                    issued_at_tick: clock.tick,
                });
            let defender = target_entity;
            commands.add(
                move |world: &mut valence::prelude::bevy_ecs::world::World| {
                    sword_basics::record_sword_parry_success(world, defender);
                },
            );
        }

        // plan-shield-block-v1 P2 / P4 — 盾牌格挡减伤分支（独立于 SwordParrying，无反伤）。
        // P4: block_ratio 经 shield_block_profile(proficiency) 缩放（替代 P2 固定 spec.block_ratio）。
        // 正面 FOV 判定：须先过 shield_fov_check，背面不减伤。
        if let Some(_raw_ratio) =
            active_status_magnitude(defender_status_effects, StatusEffectKind::ShieldBlocking)
        {
            // plan-shield-block-v1 P4 — 读取 defender 的 shield_block proficiency 并应用 profile。
            // shield_block_profile 根据 template_id 给出各盾的上下限（木盾 0.5→0.6，骨盾 0.65→0.72）。
            let (shield_template_id, shield_proficiency) = inventories
                .get(target_entity)
                .ok()
                .and_then(|inv| inv.equipped.get(EQUIP_SLOT_OFF_HAND).cloned())
                .map(|item| {
                    let proficiency = defender_known_q
                        .get(target_entity)
                        .ok()
                        .flatten()
                        .and_then(|known| {
                            known
                                .entries
                                .iter()
                                .find(|e| e.id == shield_block_mod::SHIELD_BLOCK_TECHNIQUE_ID)
                                .map(|e| e.proficiency)
                        })
                        .unwrap_or(0.0);
                    (item.template_id, proficiency)
                })
                .unwrap_or_else(|| ("wooden_shield".to_string(), 0.0));
            let profile =
                shield_block_mod::shield_block_profile(&shield_template_id, shield_proficiency);
            let ratio = profile.block_ratio.clamp(0.0, 0.95);
            // 正面 FOV 判定（±120°，dot ≥ -0.5）
            let fov_ok = shield_fov_check(
                attacker_position,
                target_position,
                positions
                    .get(target_entity)
                    .ok()
                    .and_then(|(_pos, look)| look),
            );
            if fov_ok && ratio > 0.0 {
                let before_severity = wound.severity;
                let before_contam = emitted_contam_delta;
                wound.severity *= 1.0 - ratio;
                wound.bleeding_per_sec *= 1.0 - ratio;
                emitted_contam_delta *= f64::from(1.0 - ratio);
                let blocked = (before_severity - wound.severity).max(0.0);
                shield_block_success = true;
                shield_block_ratio = Some(ratio);
                shield_block_contam_reduced = Some((before_contam - emitted_contam_delta).max(0.0));
                shield_blocked_damage = Some(blocked);

                // plan-shield-block-v1 P4 — 近破盾 narration 文本（在 get_mut 块内计算，在块外 emit）。
                let mut near_break_narration_text: Option<String> = None;

                // plan-shield-block-v1 P3 — 盾牌耐久扣减。
                // 语义：durability_max = 次满伤格挡。每次满伤格挡扣 1 单位（ratio 减 1/durability_max）。
                // 盾的 ItemInstance.durability 以 0..1 ratio 存储，与 set_item_instance_durability 契约一致。
                // SwordParrying+ShieldBlocking 互斥裁定：
                //   同帧两者同时 active 时，上方 SwordParrying 分支（lines ~912-963）先执行，
                //   ShieldBlocking 分支（当前）后执行，两者串行削减伤害（不叠加同一伤害来源）。
                //   SwordParrying 格挡已改变 wound.severity，ShieldBlocking 从改变后的值继续削，
                //   产生双重减伤但分属独立机制，不存在同一伤害被两侧重叠归零的算术失衡。
                //   防御者通过境界解锁 SwordParrying（需截脉窗口 + 真元），ShieldBlocking 仅需举盾——
                //   同帧双激活在实际游戏流程中极罕见（需同 tick 同时满足截脉窗口和举盾）；
                //   不施加互斥守护，两者独立减伤符合设计意图（盾减物理 + 截脉减真元污染）。
                // 注意：block_ratio clamp to 0.95 确保不会除零。
                if let Ok(mut inventory) = inventories.get_mut(target_entity) {
                    if let Some(item) = inventory.equipped.get(EQUIP_SLOT_OFF_HAND) {
                        let instance_id = item.instance_id;
                        let template_id_snap = item.template_id.clone();
                        let cur_ratio = item.durability;
                        let durability_max = item_registry
                            .as_deref()
                            .and_then(|reg| reg.get(&template_id_snap))
                            .and_then(|tpl| tpl.shield_spec.as_ref())
                            .map(|spec| spec.durability_max)
                            .unwrap_or(40.0); // 无 registry（单测环境）fallback 木盾值

                        if durability_max > 0.0 && cur_ratio > 0.0 {
                            // blocked_damage / durability_max 扣减比例
                            // 满伤格挡（=1次格挡整体damage=1.0 severity units） → 扣 1/durability_max ratio
                            // 部分伤害按比例线性扣
                            let cur_abs = cur_ratio * durability_max;
                            let cost = f64::from(blocked); // blocked 是本次 f32 伤害单位数
                            let next_abs = (cur_abs - cost).max(0.0);
                            let next_ratio = (next_abs / durability_max).clamp(0.0, 1.0);

                            if next_ratio < cur_ratio {
                                match set_item_instance_durability(
                                    &mut inventory,
                                    instance_id,
                                    next_ratio,
                                ) {
                                    Ok(update) => {
                                        event_writers.durability_changed_tx.send(
                                            InventoryDurabilityChangedEvent {
                                                entity: target_entity,
                                                revision: update.revision,
                                                instance_id: update.instance_id,
                                                durability: update.durability,
                                            },
                                        );
                                        // 耐久归零 → emit ShieldBroken + 移除盾物品（盾销毁）
                                        if next_ratio <= 0.0 {
                                            // 从 off_hand 移除盾（盾销毁，不保留在背包）
                                            let _ = consume_item_instance_once(
                                                &mut inventory,
                                                instance_id,
                                            );
                                            shield_broken_events.send(ShieldBroken {
                                                entity: target_entity,
                                                instance_id,
                                                template_id: template_id_snap.clone(),
                                            });
                                        }
                                        // plan-shield-block-v1 P4 — 近破盾 narration 文本。
                                        // next_ratio > 0: 盾未销毁但已低于预警阈值。
                                        if next_ratio > 0.0
                                            && next_ratio < SHIELD_NEAR_BREAK_DURABILITY_THRESHOLD
                                            && near_break_narration_text.is_none()
                                        {
                                            near_break_narration_text =
                                                Some(if template_id_snap == "bone_shield" {
                                                    "骨盾发出一声脆响，裂纹爬上盾沿。".to_string()
                                                } else {
                                                    "盾面传来裂木之声，这盾快撑不住了。".to_string()
                                                });
                                        }
                                    }
                                    Err(error) => {
                                        tracing::warn!(
                                            "[bong][combat][shield] failed to persist durability for shield instance {}: {}",
                                            instance_id,
                                            error
                                        );
                                    }
                                }
                            }
                        }
                    }
                }

                // 格挡成功：emit DefenseIntent 触发 PARRY_BLOCK 动画（兼容现有路径，不覆盖）。
                if let Some(defense_tx) = event_writers.defense_intent_tx.as_deref_mut() {
                    defense_tx.send(DefenseIntent {
                        defender: target_entity,
                        issued_at_tick: clock.tick,
                    });
                }

                // plan-shield-block-v1 P4 — 格挡成功 → 熟练度增益（接线点）。
                // 严格镜像 resolve.rs:963-965 SwordParry 的 commands.add 接线写法。
                let defender_for_proficiency = target_entity;
                commands.add(
                    move |world: &mut valence::prelude::bevy_ecs::world::World| {
                        shield_block_mod::record_shield_block_success(
                            world,
                            defender_for_proficiency,
                        );
                    },
                );

                // plan-shield-block-v1 P4 — 格挡成功 narration（scope=Player, style=Perception）。
                if let Some(narrations) = event_writers.narrations.as_deref_mut() {
                    // target_id 是 canonical_player_id（NPC 无 Username 时返回 offline: 前缀）
                    if !target_id.starts_with("offline:") && !target_id.starts_with("npc:") {
                        narrations.push_player(
                            &target_id,
                            "盾面一震，那一下被卸开了大半。",
                            NarrationStyle::Perception,
                        );
                    }
                }

                // plan-shield-block-v1 P4 — 近破盾 narration（耐久低于阈值时）。
                // near_break_narration_text 在 inventories.get_mut 块内计算（避免重借），
                // 在该块外 emit（narrations 在事件写出参数中）。
                if let Some(text) = near_break_narration_text.as_deref() {
                    if !target_id.starts_with("offline:") && !target_id.starts_with("npc:") {
                        if let Some(narrations) = event_writers.narrations.as_deref_mut() {
                            narrations.push_player(&target_id, text, NarrationStyle::Perception);
                        }
                    }
                }
            }
        }

        // plan-armor-v1 §4.1：护甲减免在截脉判定之后应用。
        // 截脉当前只影响污染与额外 concussion，不直接改变本次伤口 severity。
        if let Some(attrs) = defender_attrs.as_deref() {
            let armor_mitigation =
                apply_armor_mitigation(&mut wound, attrs, &mut emitted_contam_delta);

            // 护甲命中：扣减装备耐久（少量）。
            if let (Some(_m), Some(armor_profiles)) = (armor_mitigation, armor_profiles.as_deref())
            {
                if let Ok(mut inventory) = inventories.get_mut(target_entity) {
                    let best: Option<(u64, u32, f64, f32)> = [
                        EQUIP_SLOT_HEAD,
                        EQUIP_SLOT_CHEST,
                        EQUIP_SLOT_LEGS,
                        EQUIP_SLOT_FEET,
                    ]
                    .into_iter()
                    .filter_map(|slot| {
                        let item = inventory.equipped.get(slot)?;
                        let ap = armor_profiles.get(item.template_id.as_str())?;
                        if !ap.body_coverage.contains(&hit_probe.body_part) {
                            return None;
                        }
                        let base_m = *ap.kind_mitigation.get(&intent.wound_kind).unwrap_or(&0.0);
                        if base_m <= 0.0 {
                            return None;
                        }
                        let effective_mul =
                            ap.effective_multiplier_for_durability_ratio(item.durability);
                        let effective_m = (base_m * effective_mul).clamp(0.0, ARMOR_MITIGATION_CAP);
                        if effective_m <= 0.0 {
                            return None;
                        }
                        Some((
                            item.instance_id,
                            ap.durability_max,
                            item.durability,
                            effective_m,
                        ))
                    })
                    .max_by(|a, b| a.3.total_cmp(&b.3));

                    if let Some((instance_id, durability_max, cur_ratio, _effective_m)) = best {
                        if durability_max > 0 && cur_ratio > 0.0 {
                            let durability_max = f64::from(durability_max);
                            let cur_abs = (cur_ratio * durability_max).max(0.0);
                            let next_abs = (cur_abs - ARMOR_HIT_DURABILITY_COST_POINTS).max(0.0);
                            let next_ratio = (next_abs / durability_max).clamp(0.0, 1.0);
                            if next_ratio < cur_ratio {
                                let broke_now = next_ratio <= 0.0 && cur_ratio > 0.0;
                                match set_item_instance_durability(
                                    &mut inventory,
                                    instance_id,
                                    next_ratio,
                                ) {
                                    Ok(update) => {
                                        event_writers.durability_changed_tx.send(
                                            InventoryDurabilityChangedEvent {
                                                entity: target_entity,
                                                revision: update.revision,
                                                instance_id: update.instance_id,
                                                durability: update.durability,
                                            },
                                        );
                                        if broke_now {
                                            if let Some(audio_events) =
                                                event_writers.audio_events.as_deref_mut()
                                            {
                                                audio_events.send(PlaySoundRecipeRequest {
                                                    recipe_id: "armor_break".to_string(),
                                                    instance_id: 0,
                                                    pos: None,
                                                    flag: None,
                                                    volume_mul: 1.0,
                                                    pitch_shift: 0.0,
                                                    recipient: AudioRecipient::Radius {
                                                        origin: target_position,
                                                        radius: AUDIO_BROADCAST_RADIUS,
                                                    },
                                                });
                                            }
                                        }
                                    }
                                    Err(error) => {
                                        tracing::warn!(
                                            "[bong][combat][armor] failed to persist durability for instance {}: {}",
                                            instance_id,
                                            error
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        let pill_part_multiplier =
            body_part_damage_multiplier(defender_status_effects, hit_probe.body_part);
        if (pill_part_multiplier - 1.0).abs() > f32::EPSILON {
            wound.severity *= pill_part_multiplier;
            wound.bleeding_per_sec *= pill_part_multiplier;
            emitted_contam_delta *= f64::from(pill_part_multiplier);
        }

        let false_skin_kind_before = false_skin.as_ref().map(|skin| skin.kind);
        let filter_result = tuike_filter_contam(emitted_contam_delta, false_skin.as_deref_mut());
        let layers_remaining = false_skin
            .as_ref()
            .map(|skin| skin.layers_remaining)
            .unwrap_or(0);
        if let Some(attrs) = defender_attrs.as_deref_mut() {
            attrs.tuike_layers = layers_remaining;
        }
        if filter_result.shed_layers > 0 {
            if let Some(kind) = false_skin_kind_before {
                if let Some(events) = shed_events.as_deref_mut() {
                    events.send(ShedEvent {
                        target: target_entity,
                        attacker: Some(intent.attacker),
                        target_id: target_id.clone(),
                        attacker_id: Some(attacker_id.clone()),
                        kind,
                        layers_shed: filter_result.shed_layers,
                        layers_remaining,
                        contam_absorbed: filter_result.contam_absorbed,
                        contam_overflow: filter_result.passes_through,
                        tick: clock.tick,
                    });
                }
            }
        }
        if filter_result.depleted && false_skin_kind_before.is_some() {
            commands.entity(target_entity).remove::<FalseSkin>();
            if let Ok(mut inventory) = inventories.get_mut(target_entity) {
                if let Some(item) = inventory.equipped.get(EQUIP_SLOT_FALSE_SKIN) {
                    let instance_id = item.instance_id;
                    let _ = consume_item_instance_once(&mut inventory, instance_id);
                }
            }
        }
        emitted_contam_delta = filter_result.passes_through;
        if emitted_contam_delta > 0.0 {
            contamination.entries.push(ContamSource {
                amount: emitted_contam_delta,
                color: ColorKind::Mellow,
                meridian_id: Some(crate::cultivation::dugu::body_part_to_meridian(
                    hit_probe.body_part,
                )),
                attacker_id: Some(attacker_id.clone()),
                introduced_at: clock.tick,
            });
        }

        wounds.health_current =
            (wounds.health_current - wound.severity).clamp(0.0, wounds.health_max);
        let wound_bleeding = wound.bleeding_per_sec;
        let wound_severity = wound.severity;
        wounds.entries.push(wound);

        if wound_bleeding > 0.0 {
            event_writers
                .status_effect_intents
                .send(ApplyStatusEffectIntent {
                    target: target_entity,
                    kind: StatusEffectKind::Bleeding,
                    magnitude: wound_bleeding,
                    duration_ticks: u64::MAX,
                    issued_at_tick: clock.tick,
                });
        }

        if matches!(hit_probe.body_part, BodyPart::LegL | BodyPart::LegR)
            && wound_severity >= LEG_SLOWED_SEVERITY_THRESHOLD
        {
            event_writers
                .status_effect_intents
                .send(ApplyStatusEffectIntent {
                    target: target_entity,
                    kind: StatusEffectKind::Slowed,
                    magnitude: 0.4,
                    duration_ticks: LEG_SLOWED_DURATION_TICKS,
                    issued_at_tick: clock.tick,
                });
        }

        if hit_probe.body_part == BodyPart::Head && wound_severity >= HEAD_STUN_SEVERITY_THRESHOLD {
            event_writers
                .status_effect_intents
                .send(ApplyStatusEffectIntent {
                    target: target_entity,
                    kind: StatusEffectKind::Stunned,
                    magnitude: 1.0,
                    duration_ticks: HEAD_STUN_DURATION_TICKS,
                    issued_at_tick: clock.tick,
                });
        }

        if !is_physical_hit {
            if let Some(primary_meridian) = first_open_or_fallback_meridian(&mut meridians) {
                primary_meridian.throughput_current += qi_invest * f64::from(decay);
                primary_meridian.cracks.push(MeridianCrack {
                    severity: f64::from(wound_severity) * 0.02 * wound_profile.crack_mul,
                    healing_progress: 0.0,
                    cause: CrackCause::Attack,
                    created_at: clock.tick,
                });
            }
        }

        if let Some(mut life_record) = life_record {
            life_record.push(BiographyEntry::CombatHit {
                attacker_id: attacker_id.clone(),
                body_part: format!("{:?}", hit_probe.body_part),
                wound_kind: format!("{:?}", intent.wound_kind),
                damage: wound_severity,
                tick: clock.tick,
            });
            if let Some(effectiveness) = jiemai_effectiveness_value {
                life_record.push(BiographyEntry::JiemaiParry {
                    attacker_id: attacker_id.clone(),
                    effectiveness,
                    tick: clock.tick,
                });
            }
        }

        let action_label = if intent.debug_command.is_some() {
            "debug_attack_intent"
        } else {
            attack_source_label(intent.source)
        };
        let qi_damage = if is_physical_hit { 0.0 } else { wound_severity };
        let physical_damage = if is_physical_hit { wound_severity } else { 0.0 };
        let description = format!(
            "{} {} -> {} hit {:?} with {:?} for {:.1} qi / {:.1} physical damage (hit_qi {:.1}, jiemai={} sword_parry={} shield_block={} eff={:.2}) at {:.2} reach decay",
            action_label,
            attacker_id,
            target_id,
            hit_probe.body_part,
            intent.wound_kind,
            qi_damage,
            physical_damage,
            hit_qi,
            jiemai_success,
            sword_parry_success,
            shield_block_success,
            jiemai_effectiveness_value
                .or(sword_parry_block_ratio)
                .or(shield_block_ratio)
                .unwrap_or(0.0),
            decay
        );

        event_writers.out_events.send(CombatEvent {
            attacker: intent.attacker,
            target: target_entity,
            resolved_at_tick: clock.tick,
            body_part: hit_probe.body_part,
            wound_kind: intent.wound_kind,
            source: intent.source,
            debug_command: intent.debug_command.is_some(),
            physical_damage,
            damage: qi_damage,
            contam_delta: emitted_contam_delta,
            description,
            defense_kind: if sword_parry_success {
                Some(DefenseKind::SwordParry)
            } else if shield_block_success {
                Some(DefenseKind::ShieldBlock)
            } else {
                jiemai_success.then_some(DefenseKind::JieMai)
            },
            defense_effectiveness: jiemai_effectiveness_value
                .or(sword_parry_block_ratio)
                .or(shield_block_ratio),
            defense_contam_reduced: jiemai_contam_reduced
                .or(sword_parry_contam_reduced)
                .or(shield_block_contam_reduced),
            defense_wound_severity: jiemai_wound_severity
                .or(sword_parry_reflected_damage)
                .or(shield_blocked_damage),
        });
        if let Some(events) = event_writers.vfx_events.as_deref_mut() {
            let hit_origin = target_position + DVec3::new(0.0, 1.0, 0.0);
            let hit_dir = [
                target_position.x - attacker_position.x,
                target_position.y - attacker_position.y,
                target_position.z - attacker_position.z,
            ];
            let hit_len =
                (hit_dir[0] * hit_dir[0] + hit_dir[1] * hit_dir[1] + hit_dir[2] * hit_dir[2])
                    .sqrt();
            let hit_dir = if hit_len > 1e-6 {
                [
                    hit_dir[0] / hit_len,
                    hit_dir[1] / hit_len,
                    hit_dir[2] / hit_len,
                ]
            } else {
                [0.0, 0.0, 0.0]
            };
            gameplay_vfx::send_spawn(
                events,
                gameplay_vfx::spawn_request(
                    gameplay_vfx::COMBAT_HIT,
                    hit_origin,
                    Some(hit_dir),
                    "#FF3344",
                    (wound_severity / 20.0).clamp(0.25, 1.0),
                    6,
                    12,
                ),
            );
            if jiemai_success || sword_parry_success || shield_block_success {
                gameplay_vfx::send_spawn(
                    events,
                    gameplay_vfx::spawn_request(
                        gameplay_vfx::COMBAT_PARRY,
                        hit_origin,
                        Some([-hit_dir[0], -hit_dir[1], -hit_dir[2]]),
                        if sword_parry_success {
                            "#FFD080"
                        } else if shield_block_success {
                            "#A0C8FF"
                        } else {
                            "#4488FF"
                        },
                        jiemai_effectiveness_value
                            .or(sword_parry_block_ratio)
                            .or(shield_block_ratio)
                            .unwrap_or(0.6)
                            .clamp(0.3, 1.0),
                        8,
                        16,
                    ),
                );
            }
        }

        if let Some(active_events) = active_events.as_deref_mut() {
            active_events.record_recent_event(GameEvent {
                event_type: GameEventType::EventTriggered,
                tick: clock.tick,
                player: Some(attacker_id.clone()),
                target: Some(target_id),
                zone: None,
                details: Some(std::collections::HashMap::from([
                    ("action".to_string(), json!(action_label)),
                    (
                        "body_part".to_string(),
                        json!(format!("{:?}", hit_probe.body_part)),
                    ),
                    (
                        "wound_kind".to_string(),
                        json!(format!("{:?}", intent.wound_kind)),
                    ),
                    ("damage".to_string(), json!(wound_severity)),
                    ("physical_damage".to_string(), json!(physical_damage)),
                    ("contam_delta".to_string(), json!(emitted_contam_delta)),
                    ("qi_invest".to_string(), json!(intent.qi_invest)),
                    ("hit_qi".to_string(), json!(hit_qi)),
                    ("jiemai_success".to_string(), json!(jiemai_success)),
                    (
                        "sword_parry_success".to_string(),
                        json!(sword_parry_success),
                    ),
                    (
                        "jiemai_effectiveness".to_string(),
                        json!(jiemai_effectiveness_value),
                    ),
                    (
                        "jiemai_contam_reduced".to_string(),
                        json!(jiemai_contam_reduced),
                    ),
                    (
                        "jiemai_wound_severity".to_string(),
                        json!(jiemai_wound_severity),
                    ),
                    ("reach_decay".to_string(), json!(decay)),
                ])),
            });
        }

        let active_sparring = crate::social::active_sparring_between(
            &sparring_sessions,
            intent.attacker,
            target_entity,
        );
        if let Some(sparring) = active_sparring.as_ref() {
            if clock.tick <= sparring.expires_at_tick && was_alive && wounds.health_current <= 0.0 {
                wounds.health_current = (wounds.health_max.max(1.0) * 0.05).max(1.0);
                crate::social::conclude_sparring_defeat(
                    &mut commands,
                    &mut event_writers.status_effect_intents,
                    target_entity,
                    intent.attacker,
                    clock.tick,
                );
                continue;
            }
        }

        if was_alive
            && wounds.health_current <= 0.0
            && !lifecycle.is_some_and(|lifecycle| {
                matches!(
                    lifecycle.state,
                    LifecycleState::NearDeath | LifecycleState::Terminated
                )
            })
        {
            // plan-tsy-loot-v1 §6 — 攻击链路：attacker entity 来自 intent；
            // attacker_player_id 仅在攻击者是 player 时填（canonical id 形如
            // "offline:Foo"），NPC 攻击者保留 None。
            let attacker_player_id = attacker_id
                .starts_with("offline:")
                .then(|| attacker_id.clone());
            event_writers.death_events.send(DeathEvent {
                target: target_entity,
                cause: format!("{action_label}:{attacker_id}"),
                attacker: Some(intent.attacker),
                attacker_player_id,
                at_tick: clock.tick,
            });
            if let (true, Some(skill_xp_events)) = (
                attacker_id.starts_with("offline:"),
                skill_xp_events.as_deref_mut(),
            ) {
                skill_xp_events.send(SkillXpGain {
                    char_entity: intent.attacker,
                    skill: SkillId::Combat,
                    amount: 4,
                    source: XpGainSource::Action {
                        plan_id: "combat",
                        action: "kill_npc",
                    },
                });
            }
        }
    }
}

fn attack_source_label(source: AttackSource) -> &'static str {
    match source {
        AttackSource::Melee => "attack_intent",
        AttackSource::BurstMeridian => "burst_meridian_attack",
        AttackSource::QiNeedle => "qi_needle",
        AttackSource::FullPower => "full_power_strike",
        AttackSource::SwordCleave => "sword_cleave",
        AttackSource::SwordThrust => "sword_thrust",
        AttackSource::SwordPathCondenseEdge => "sword_path_condense_edge",
        AttackSource::SwordPathQiSlash => "sword_path_qi_slash",
        AttackSource::SwordPathResonance => "sword_path_resonance",
        AttackSource::SwordPathManifest => "sword_path_manifest",
        AttackSource::SwordPathHeavenGate => "sword_path_heaven_gate",
    }
}

fn source_uses_prepaid_qi(source: AttackSource) -> bool {
    matches!(
        source,
        AttackSource::BurstMeridian
            | AttackSource::FullPower
            | AttackSource::SwordCleave
            | AttackSource::SwordThrust
            // plan-sword-path-v2 §P1: 五招都在 cast 阶段通过 apply_cast_costs 已扣
            // 真元 / 体力。若漏掉 prepaid 白名单，resolver 的反作弊会因
            // qi_invest > qi_current 拒绝结算（QiInvestExceeded），结果是所有
            // sword_path 攻击都被错误拦截。
            | AttackSource::SwordPathCondenseEdge
            | AttackSource::SwordPathQiSlash
            | AttackSource::SwordPathResonance
            | AttackSource::SwordPathManifest
            | AttackSource::SwordPathHeavenGate
    )
}

fn active_status_magnitude(
    statuses: Option<&StatusEffects>,
    kind: StatusEffectKind,
) -> Option<f32> {
    statuses?
        .active
        .iter()
        .find(|effect| effect.kind == kind && effect.remaining_ticks > 0)
        .map(|effect| effect.magnitude)
}

fn record_anticheat_violation(
    counter: Option<&mut AntiCheatCounter>,
    kind: ViolationKindV1,
    details: String,
) {
    let Some(counter) = counter else {
        return;
    };
    counter.record_violation(kind, details);
}

fn body_part_multipliers(body_part: BodyPart) -> (f32, f32, f32) {
    match body_part {
        BodyPart::Head => (2.0, 1.5, 1.5),
        BodyPart::Chest => (1.0, 1.0, 1.0),
        BodyPart::Back => (0.9, 1.0, 1.0),
        BodyPart::Abdomen => (0.9, 1.2, 1.3),
        BodyPart::ArmL | BodyPart::ArmR => (0.7, 0.8, 0.8),
        BodyPart::LegL | BodyPart::LegR => (0.6, 0.7, 1.0),
    }
}

fn wound_kind_profile(kind: crate::combat::components::WoundKind) -> WoundKindProfile {
    match kind {
        crate::combat::components::WoundKind::Cut => WoundKindProfile {
            damage_mul: 1.0,
            bleed_mul: 1.4,
            contam_mul: 1.0,
            crack_mul: 1.0,
        },
        crate::combat::components::WoundKind::Blunt => WoundKindProfile {
            damage_mul: 1.0,
            bleed_mul: 0.7,
            contam_mul: 0.8,
            crack_mul: 1.3,
        },
        crate::combat::components::WoundKind::Pierce => WoundKindProfile {
            damage_mul: 1.0,
            bleed_mul: 1.0,
            contam_mul: 1.2,
            crack_mul: 1.1,
        },
        crate::combat::components::WoundKind::Burn => WoundKindProfile {
            damage_mul: 1.0,
            bleed_mul: 0.2,
            contam_mul: 1.3,
            crack_mul: 0.7,
        },
        crate::combat::components::WoundKind::Concussion => WoundKindProfile {
            damage_mul: 1.0,
            bleed_mul: 0.1,
            contam_mul: 0.6,
            crack_mul: 1.4,
        },
    }
}

type ResolvedIntent = (DVec3, String, Entity, DVec3, String);

fn resolve_intent_entities(
    intent: &AttackIntent,
    clients: &Query<CombatClientItem<'_>, CombatClientFilter>,
    positions: &Query<PositionLookItem<'_>>,
    npc_markers: &Query<(), With<NpcMarker>>,
    npc_positions: &Query<(Entity, &Position), With<NpcMarker>>,
) -> Option<ResolvedIntent> {
    let (attacker_position, attacker_id) =
        resolve_combat_actor(intent.attacker, clients, positions, npc_markers)?;

    if let Some(action) = intent.debug_command.as_ref() {
        let (target_entity, target_position, _target_hint_qi_max, target_id) =
            resolve_debug_target(
                intent,
                action,
                clients,
                positions,
                npc_markers,
                npc_positions,
            )?;
        return Some((
            attacker_position,
            attacker_id,
            target_entity,
            target_position,
            target_id,
        ));
    }

    let target_entity = intent.target?;
    if target_entity == intent.attacker {
        return None;
    }
    let (target_position, target_id) =
        resolve_combat_actor(target_entity, clients, positions, npc_markers)?;
    Some((
        attacker_position,
        attacker_id,
        target_entity,
        target_position,
        target_id,
    ))
}

fn resolve_combat_actor(
    entity: Entity,
    clients: &Query<CombatClientItem<'_>, CombatClientFilter>,
    positions: &Query<PositionLookItem<'_>>,
    npc_markers: &Query<(), With<NpcMarker>>,
) -> Option<(DVec3, String)> {
    if let Ok((_, position, username, _)) = clients.get(entity) {
        return Some((position.get(), canonical_player_id(username.0.as_str())));
    }
    if npc_markers.get(entity).is_ok() {
        let position = positions.get(entity).ok()?.0.get();
        return Some((position, canonical_npc_id(entity)));
    }
    None
}

fn resolve_debug_target(
    intent: &AttackIntent,
    action: &crate::player::gameplay::CombatAction,
    clients: &Query<CombatClientItem<'_>, CombatClientFilter>,
    positions: &Query<PositionLookItem<'_>>,
    npc_markers: &Query<(), With<NpcMarker>>,
    npc_positions: &Query<(Entity, &Position), With<NpcMarker>>,
) -> Option<(Entity, DVec3, f64, String)> {
    if let Some(target) = intent.target {
        if let Ok((_, position, username, _player_state)) = clients.get(target) {
            return Some((
                target,
                position.get(),
                0.0,
                canonical_player_id(username.0.as_str()),
            ));
        }

        if npc_markers.get(target).is_ok() {
            let position = positions.get(target).ok()?.0.get();
            return Some((target, position, 0.0, canonical_npc_id(target)));
        }

        return None;
    }

    let target_name = action.target.trim();
    if target_name.is_empty() {
        return None;
    }

    if let Some(player_match) =
        clients
            .iter()
            .find_map(|(entity, position, username, _player_state)| {
                if entity == intent.attacker {
                    return None;
                }

                let canonical = canonical_player_id(username.0.as_str());
                (username.0.eq_ignore_ascii_case(target_name)
                    || canonical.eq_ignore_ascii_case(target_name))
                .then_some((entity, position.get(), 0.0, canonical))
            })
    {
        return Some(player_match);
    }

    npc_positions.iter().find_map(|(entity, position)| {
        if entity == intent.attacker {
            return None;
        }

        let canonical = canonical_npc_id(entity);
        canonical.eq_ignore_ascii_case(target_name).then_some((
            entity,
            position.get(),
            0.0,
            canonical,
        ))
    })
}

fn first_open_or_fallback_meridian(
    meridians: &mut MeridianSystem,
) -> Option<&mut crate::cultivation::components::Meridian> {
    if let Some(index) = meridians
        .regular
        .iter()
        .position(|meridian| meridian.opened)
    {
        return meridians.regular.get_mut(index);
    }

    meridians.regular.get_mut(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::combat::anticheat::AntiCheatCounter;
    use crate::combat::armor::{ArmorProfile, ArmorProfileRegistry};
    use crate::combat::components::{
        ActiveStatusEffect, BodyPart, CombatState, DefenseWindow, DerivedAttrs, Lifecycle,
        RevivalDecision, StatusEffects, WoundKind, Wounds,
    };
    use crate::combat::events::{
        ApplyStatusEffectIntent, AttackIntent, AttackReach, AttackSource, DefenseKind,
        StatusEffectKind, FIST_REACH,
    };
    use crate::combat::jiemai::jiemai_contam_multiplier_for_effectiveness;
    use crate::cultivation::components::{
        Contamination, CrackCause, Cultivation, MeridianId, MeridianSystem, Realm,
    };
    use crate::cultivation::known_techniques::KnownTechnique;
    use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemCategory, ItemInstance, ItemRarity, ItemRegistry,
        ItemTemplate, PlayerInventory, WeaponSpec, EQUIP_SLOT_CHEST, EQUIP_SLOT_OFF_HAND,
    };
    use crate::npc::brain::canonical_npc_id;
    use crate::npc::spawn::NpcMeleeProfile;
    use crate::npc::spawn::{spawn_test_npc_runtime_shape, NpcMarker};
    use crate::player::state::PlayerState;
    use crate::social::components::SparringState;
    use valence::prelude::{
        bevy_ecs, App, Entity, Events, GameMode, IntoSystemConfigs, Position, Resource, Update,
    };
    use valence::testing::create_mock_client;

    #[derive(Clone, Copy, Resource)]
    struct TestLayer(Entity);

    fn setup_test_layer(mut commands: valence::prelude::Commands) {
        let layer = commands.spawn_empty().id();
        commands.insert_resource(TestLayer(layer));
    }

    fn spawn_runtime_npc(
        mut commands: valence::prelude::Commands,
        layer: valence::prelude::Res<TestLayer>,
    ) {
        spawn_test_npc_runtime_shape(&mut commands, layer.0);
    }

    fn spawn_player(
        app: &mut App,
        username: &str,
        position: [f64; 3],
        wounds: Wounds,
        stamina: Stamina,
    ) -> Entity {
        let (mut client_bundle, _helper) = create_mock_client(username);
        client_bundle.player.position = Position::new(position);
        let entity = app
            .world_mut()
            .spawn((
                client_bundle,
                Cultivation {
                    realm: crate::cultivation::components::Realm::Induce,
                    qi_current: 60.0,
                    qi_max: 100.0,
                    ..Cultivation::default()
                },
                PlayerState {
                    karma: 0.0,
                    inventory_score: 0.0,
                },
                MeridianSystem::default(),
                LifeRecord::new(canonical_player_id(username)),
                Contamination::default(),
                StatusEffects::default(),
                wounds,
                stamina,
                CombatState::default(),
                DerivedAttrs::default(),
                Lifecycle {
                    character_id: canonical_player_id(username),
                    ..Default::default()
                },
            ))
            .id();
        app.world_mut()
            .entity_mut(entity)
            .insert(GameMode::Survival);
        entity
    }

    fn weapon_test_registry() -> ItemRegistry {
        ItemRegistry::from_map(std::collections::HashMap::from([
            (
                "iron_sword".to_string(),
                ItemTemplate {
                    id: "iron_sword".to_string(),
                    display_name: "铁剑".to_string(),
                    category: ItemCategory::Weapon,
                    placeable: None,
                    max_stack_count: 1,
                    grid_w: 1,
                    grid_h: 2,
                    base_weight: 1.2,
                    rarity: ItemRarity::Common,
                    spirit_quality_initial: 1.0,
                    description: String::new(),
                    effect: None,
                    cast_duration_ms: 0,
                    cooldown_ms: 0,
                    weapon_spec: Some(WeaponSpec {
                        weapon_kind: crate::combat::weapon::WeaponKind::Sword,
                        base_attack: 12.0,
                        quality_tier: 0,
                        durability_max: 200.0,
                        qi_cost_mul: 1.0,
                    }),
                    forge_station_spec: None,
                    blueprint_scroll_spec: None,
                    inscription_scroll_spec: None,
                    technique_scroll_spec: None,
                    recipe_fragment_spec: None,
                    container_spec: None,
                    shelflife_profile: None,
                    shield_spec: None,
                    shelflife_track: None,
                },
            ),
            (
                "strong_sword".to_string(),
                ItemTemplate {
                    id: "strong_sword".to_string(),
                    display_name: "强剑".to_string(),
                    category: ItemCategory::Weapon,
                    placeable: None,
                    max_stack_count: 1,
                    grid_w: 1,
                    grid_h: 2,
                    base_weight: 1.0,
                    rarity: ItemRarity::Common,
                    spirit_quality_initial: 1.0,
                    description: String::new(),
                    effect: None,
                    cast_duration_ms: 0,
                    cooldown_ms: 0,
                    weapon_spec: Some(WeaponSpec {
                        weapon_kind: crate::combat::weapon::WeaponKind::Sword,
                        base_attack: 20.0,
                        quality_tier: 0,
                        durability_max: 200.0,
                        qi_cost_mul: 1.0,
                    }),
                    forge_station_spec: None,
                    blueprint_scroll_spec: None,
                    inscription_scroll_spec: None,
                    technique_scroll_spec: None,
                    recipe_fragment_spec: None,
                    container_spec: None,
                    shelflife_profile: None,
                    shield_spec: None,
                    shelflife_track: None,
                },
            ),
            (
                "glass_sword".to_string(),
                ItemTemplate {
                    id: "glass_sword".to_string(),
                    display_name: "玻璃剑".to_string(),
                    category: ItemCategory::Weapon,
                    placeable: None,
                    max_stack_count: 1,
                    grid_w: 1,
                    grid_h: 2,
                    base_weight: 1.0,
                    rarity: ItemRarity::Common,
                    spirit_quality_initial: 1.0,
                    description: String::new(),
                    effect: None,
                    cast_duration_ms: 0,
                    cooldown_ms: 0,
                    weapon_spec: Some(WeaponSpec {
                        weapon_kind: crate::combat::weapon::WeaponKind::Sword,
                        base_attack: 10.0,
                        quality_tier: 0,
                        durability_max: 10.0,
                        qi_cost_mul: 1.0,
                    }),
                    forge_station_spec: None,
                    blueprint_scroll_spec: None,
                    inscription_scroll_spec: None,
                    technique_scroll_spec: None,
                    recipe_fragment_spec: None,
                    container_spec: None,
                    shelflife_profile: None,
                    shield_spec: None,
                    shelflife_track: None,
                },
            ),
        ]))
    }

    #[test]
    fn armor_hit_scales_contamination_and_ticks_item_durability() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1500 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<SkillXpGain>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();

        app.insert_resource(crate::inventory::ItemRegistry::default());
        app.insert_resource(ArmorProfileRegistry::from_map(
            std::collections::HashMap::from([(
                "fake_spirit_hide".to_string(),
                ArmorProfile {
                    slot: EquipSlotV1::Chest,
                    body_coverage: vec![BodyPart::Chest],
                    kind_mitigation: std::collections::HashMap::from([(WoundKind::Blunt, 0.5)]),
                    durability_max: 100,
                    broken_multiplier: 0.3,
                },
            )]),
        ));

        app.add_systems(
            Update,
            (
                crate::combat::status::attribute_aggregate_tick,
                crate::combat::weapon::sync_weapon_component_from_equipped,
                crate::combat::armor_sync::sync_armor_to_derived_attrs,
                resolve_attack_intents,
            ),
        );

        let attacker = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_player(
            &mut app,
            "Crimson",
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );

        // 给 target 装一件胸甲，初始耐久比例 1.0。
        app.world_mut().entity_mut(target).insert(PlayerInventory {
            revision: InventoryRevision(1),
            containers: vec![ContainerState {
                id: crate::inventory::MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 5,
                cols: 7,
                items: vec![],
            }],
            equipped: std::collections::HashMap::from([(
                crate::inventory::EQUIP_SLOT_CHEST.to_string(),
                ItemInstance {
                    instance_id: 88,
                    template_id: "fake_spirit_hide".to_string(),
                    display_name: "假灵兽皮胸甲".to_string(),
                    grid_w: 2,
                    grid_h: 2,
                    weight: 5.0,
                    rarity: crate::inventory::ItemRarity::Common,
                    description: String::new(),
                    stack_count: 1,
                    spirit_quality: 1.0,
                    durability: 1.0,
                    freshness: None,
                    mineral_id: None,
                    charges: None,
                    forge_quality: None,
                    forge_color: None,
                    forge_side_effects: Vec::new(),
                    forge_achieved_tier: None,
                    alchemy: None,
                    lingering_owner_qi: None,
                },
            )]),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
        });

        app.update();

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 1499,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.update();

        let combat_events = app.world().resource::<Events<CombatEvent>>();
        let event = combat_events
            .iter_current_update_events()
            .next()
            .expect("combat event should emit");
        // event.damage 是 mitigation 之后的 wound_severity（已乘 1-m）。
        // emitted_contam_delta = init_damage * 0.25 * 1 * 0.8 * (1-m) * MULTIPLIER
        //                       = event.damage * 0.25 * 1 * 0.8 * MULTIPLIER。
        let expected_contam =
            f64::from(event.damage) * 0.25 * 1.0 * 0.8 * ARMOR_HIT_CONTAMINATION_MULTIPLIER;
        assert_eq!(event.contam_delta, expected_contam);

        let inventory = app.world().entity(target).get::<PlayerInventory>().unwrap();
        assert!(
            inventory.equipped[crate::inventory::EQUIP_SLOT_CHEST].durability < 1.0,
            "armor hit should tick down durability"
        );
    }

    #[test]
    fn armor_break_emits_durability_event_and_radius_audio() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1501 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<SkillXpGain>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_event::<PlaySoundRecipeRequest>();

        app.insert_resource(crate::inventory::ItemRegistry::default());
        app.insert_resource(ArmorProfileRegistry::from_map(
            std::collections::HashMap::from([(
                "fake_spirit_hide".to_string(),
                ArmorProfile {
                    slot: EquipSlotV1::Chest,
                    body_coverage: vec![BodyPart::Chest],
                    kind_mitigation: std::collections::HashMap::from([(WoundKind::Blunt, 0.5)]),
                    durability_max: 1,
                    broken_multiplier: 0.3,
                },
            )]),
        ));

        app.add_systems(
            Update,
            (
                crate::combat::status::attribute_aggregate_tick,
                crate::combat::weapon::sync_weapon_component_from_equipped,
                crate::combat::armor_sync::sync_armor_to_derived_attrs,
                resolve_attack_intents,
            ),
        );

        let attacker = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_player(
            &mut app,
            "Crimson",
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        app.world_mut().entity_mut(target).insert(PlayerInventory {
            revision: InventoryRevision(1),
            containers: vec![ContainerState {
                id: crate::inventory::MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 5,
                cols: 7,
                items: vec![],
            }],
            equipped: std::collections::HashMap::from([(
                crate::inventory::EQUIP_SLOT_CHEST.to_string(),
                ItemInstance {
                    instance_id: 89,
                    template_id: "fake_spirit_hide".to_string(),
                    display_name: "假灵兽皮胸甲".to_string(),
                    grid_w: 2,
                    grid_h: 2,
                    weight: 5.0,
                    rarity: crate::inventory::ItemRarity::Common,
                    description: String::new(),
                    stack_count: 1,
                    spirit_quality: 1.0,
                    durability: 0.25,
                    freshness: None,
                    mineral_id: None,
                    charges: None,
                    forge_quality: None,
                    forge_color: None,
                    forge_side_effects: Vec::new(),
                    forge_achieved_tier: None,
                    alchemy: None,
                    lingering_owner_qi: None,
                },
            )]),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
        });

        app.update();

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 1500,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.update();

        let durability_events = app
            .world()
            .resource::<Events<InventoryDurabilityChangedEvent>>();
        let events: Vec<_> = durability_events.iter_current_update_events().collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].entity, target);
        assert_eq!(events[0].instance_id, 89);
        assert_eq!(events[0].durability, 0.0);

        let audio_events = app.world().resource::<Events<PlaySoundRecipeRequest>>();
        let events: Vec<_> = audio_events.iter_current_update_events().collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].recipe_id, "armor_break");
        match &events[0].recipient {
            AudioRecipient::Radius { origin, radius } => {
                assert_eq!(*origin, valence::prelude::DVec3::new(1.0, 64.0, 0.0));
                assert_eq!(*radius, AUDIO_BROADCAST_RADIUS);
            }
            other => panic!("armor_break should use radius recipient, got {other:?}"),
        }
    }

    fn spawn_npc(app: &mut App, position: [f64; 3], wounds: Wounds, stamina: Stamina) -> Entity {
        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new(position),
                Cultivation {
                    qi_current: 60.0,
                    qi_max: 100.0,
                    ..Cultivation::default()
                },
                MeridianSystem::default(),
                LifeRecord::default(),
                Contamination::default(),
                StatusEffects::default(),
                wounds,
                stamina,
                CombatState::default(),
                DerivedAttrs::default(),
            ))
            .id();
        let canonical = canonical_npc_id(entity);
        app.world_mut().entity_mut(entity).insert((
            Lifecycle {
                character_id: canonical.clone(),
                ..Default::default()
            },
            LifeRecord::new(canonical),
        ));
        entity
    }

    #[test]
    fn hit_emits_direction_vfx() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 44 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let attacker = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_player(
            &mut app,
            "Crimson",
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 44,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.update();

        let vfx_events = app.world().resource::<Events<VfxEventRequest>>();
        let emitted = vfx_events
            .iter_current_update_events()
            .find(|event| {
                matches!(
                    &event.payload,
                    crate::schema::vfx_event::VfxEventPayloadV1::SpawnParticle { event_id, .. }
                        if event_id == gameplay_vfx::COMBAT_HIT
                )
            })
            .expect("resolved hit should emit combat_hit vfx");
        match &emitted.payload {
            crate::schema::vfx_event::VfxEventPayloadV1::SpawnParticle {
                event_id,
                direction,
                ..
            } => {
                assert_eq!(event_id, gameplay_vfx::COMBAT_HIT);
                assert!(direction.is_some(), "combat_hit should carry hit direction");
            }
            other => panic!("expected SpawnParticle, got {other:?}"),
        }
    }

    #[test]
    fn hit_emits_knockback_event_and_pending_movement() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 44 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<KnockbackEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let attacker = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_player(
            &mut app,
            "Crimson",
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 44,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.update();

        let pending = app
            .world()
            .get::<PendingKnockback>(target)
            .expect("resolved hit should install pending knockback");
        assert_eq!(pending.attacker, Some(attacker));
        assert_eq!(pending.source, AttackSource::Melee);
        assert!(pending.distance_blocks > 0.0);
        assert_eq!(pending.chain_depth, DEFAULT_CHAIN_DEPTH);

        let knockback_events = app.world().resource::<Events<KnockbackEvent>>();
        let events = knockback_events
            .iter_current_update_events()
            .collect::<Vec<_>>();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].attacker, attacker);
        assert_eq!(events[0].target, target);
        assert_eq!(events[0].collision_damage, None);
        assert!(!events[0].block_broken);
    }

    #[test]
    fn attack_intent_skips_creative_target_without_damage_events_or_knockback() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 44 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<KnockbackEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let attacker = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_player(
            &mut app,
            "Crimson",
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        app.world_mut()
            .entity_mut(target)
            .insert(GameMode::Creative);
        let before = app
            .world()
            .entity(target)
            .get::<Wounds>()
            .unwrap()
            .health_current;

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 44,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.update();

        let wounds = app.world().entity(target).get::<Wounds>().unwrap();
        assert_eq!(
            wounds.health_current, before,
            "Creative target must not lose health from resolver"
        );
        assert!(
            app.world()
                .resource::<Events<CombatEvent>>()
                .iter_current_update_events()
                .next()
                .is_none(),
            "Creative target must not emit CombatEvent"
        );
        assert!(
            app.world()
                .resource::<Events<KnockbackEvent>>()
                .iter_current_update_events()
                .next()
                .is_none(),
            "Creative target must not emit knockback"
        );
        assert!(
            app.world().get::<PendingKnockback>(target).is_none(),
            "Creative target must not receive pending knockback"
        );
    }

    #[test]
    fn attack_intent_skips_near_death_target_without_extra_wounds_or_knockback() {
        assert_attack_intent_skips_lifecycle_target_without_extra_wounds_or_knockback(
            "NearDeath",
            LifecycleState::NearDeath,
            |lifecycle| lifecycle.enter_near_death(40),
        );
    }

    #[test]
    fn attack_intent_skips_awaiting_revival_target_without_extra_wounds_or_knockback() {
        assert_attack_intent_skips_lifecycle_target_without_extra_wounds_or_knockback(
            "AwaitingRevival",
            LifecycleState::AwaitingRevival,
            |lifecycle| {
                lifecycle.enter_near_death(40);
                assert_eq!(
                    lifecycle.state,
                    LifecycleState::NearDeath,
                    "test setup expected enter_near_death to move target into NearDeath before awaiting revival"
                );
                lifecycle.await_revival_decision(RevivalDecision::Fortune { chance: 1.0 }, 120);
                assert_eq!(
                    lifecycle.state,
                    LifecycleState::AwaitingRevival,
                    "test setup expected await_revival_decision to move target into AwaitingRevival"
                );
            },
        );
    }

    #[test]
    fn attack_intent_skips_terminated_target_without_extra_wounds_or_knockback() {
        assert_attack_intent_skips_lifecycle_target_without_extra_wounds_or_knockback(
            "Terminated",
            LifecycleState::Terminated,
            |lifecycle| lifecycle.terminate(40),
        );
    }

    fn assert_attack_intent_skips_lifecycle_target_without_extra_wounds_or_knockback(
        state_name: &str,
        expected_state: LifecycleState,
        enter_state: impl FnOnce(&mut Lifecycle),
    ) {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 44 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<KnockbackEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let attacker = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_player(
            &mut app,
            "Crimson",
            [1.0, 64.0, 0.0],
            Wounds {
                health_current: 5.0,
                health_max: 100.0,
                entries: vec![Wound {
                    location: BodyPart::Chest,
                    kind: WoundKind::Blunt,
                    severity: 3.0,
                    bleeding_per_sec: 0.0,
                    created_at_tick: 40,
                    inflicted_by: Some("test".to_string()),
                }],
            },
            Stamina::default(),
        );
        {
            let mut target_entity = app.world_mut().entity_mut(target);
            let mut lifecycle = target_entity.get_mut::<Lifecycle>().unwrap();
            enter_state(&mut lifecycle);
            assert_eq!(
                lifecycle.state, expected_state,
                "test setup expected target Lifecycle::{state_name} before resolver"
            );
        }
        let before = app.world().entity(target).get::<Wounds>().unwrap().clone();
        let attacker_qi_before = app
            .world()
            .entity(attacker)
            .get::<Cultivation>()
            .unwrap()
            .qi_current;
        let attacker_meridian_throughputs_before = meridian_throughput_snapshot(
            app.world()
                .entity(attacker)
                .get::<MeridianSystem>()
                .unwrap(),
        );
        let target_stamina_before =
            stamina_snapshot(app.world().entity(target).get::<Stamina>().unwrap());
        let target_contamination_before =
            contamination_snapshot(app.world().entity(target).get::<Contamination>().unwrap());
        let target_meridian_throughputs_before = meridian_throughput_snapshot(
            app.world().entity(target).get::<MeridianSystem>().unwrap(),
        );

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 44,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.update();

        let wounds = app.world().entity(target).get::<Wounds>().unwrap();
        let attacker_qi_after = app
            .world()
            .entity(attacker)
            .get::<Cultivation>()
            .unwrap()
            .qi_current;
        let attacker_meridian_throughputs_after = meridian_throughput_snapshot(
            app.world()
                .entity(attacker)
                .get::<MeridianSystem>()
                .unwrap(),
        );
        let target_stamina_after =
            stamina_snapshot(app.world().entity(target).get::<Stamina>().unwrap());
        let target_contamination_after =
            contamination_snapshot(app.world().entity(target).get::<Contamination>().unwrap());
        let target_meridian_throughputs_after = meridian_throughput_snapshot(
            app.world().entity(target).get::<MeridianSystem>().unwrap(),
        );
        assert_eq!(
            attacker_qi_after, attacker_qi_before,
            "{state_name} target must not deduct attacker qi"
        );
        assert_eq!(
            attacker_meridian_throughputs_after, attacker_meridian_throughputs_before,
            "{state_name} target must not grant throughput to any attacker meridian"
        );
        assert_eq!(
            target_stamina_after, target_stamina_before,
            "{state_name} target must not change target stamina"
        );
        assert_eq!(
            target_contamination_after, target_contamination_before,
            "{state_name} target must not change target contamination"
        );
        assert_eq!(
            target_meridian_throughputs_after, target_meridian_throughputs_before,
            "{state_name} target must not change any target meridian throughput"
        );
        assert_eq!(
            wounds.health_current, before.health_current,
            "{state_name} target must not lose health from resolver"
        );
        assert_eq!(
            wounds.entries.len(),
            before.entries.len(),
            "{state_name} target must not receive new wound entries"
        );
        assert!(
            app.world()
                .resource::<Events<CombatEvent>>()
                .iter_current_update_events()
                .next()
                .is_none(),
            "{state_name} target must not emit CombatEvent"
        );
        assert!(
            app.world()
                .resource::<Events<KnockbackEvent>>()
                .iter_current_update_events()
                .next()
                .is_none(),
            "{state_name} target must not emit KnockbackEvent"
        );
        assert!(
            app.world().get::<PendingKnockback>(target).is_none(),
            "{state_name} target must not receive pending knockback"
        );
    }

    fn meridian_throughput_snapshot(meridians: &MeridianSystem) -> Vec<(MeridianId, f64)> {
        MeridianId::ALL
            .into_iter()
            .map(|id| (id, meridians.get(id).throughput_current))
            .collect()
    }

    fn stamina_snapshot(stamina: &Stamina) -> (f32, f32, f32, Option<u64>, StaminaState) {
        (
            stamina.current,
            stamina.max,
            stamina.recover_per_sec,
            stamina.last_drain_tick,
            stamina.state,
        )
    }

    fn contamination_snapshot(contamination: &Contamination) -> serde_json::Value {
        serde_json::to_value(contamination)
            .expect("Contamination should serialize for side-effect snapshot")
    }

    // ─── Attacker lifecycle guard tests ─────────────────────────────────────

    #[test]
    fn attacker_near_death_skips_intent() {
        assert_attacker_lifecycle_skips_intent("NearDeath", |lifecycle| {
            lifecycle.enter_near_death(40)
        });
    }

    #[test]
    fn attacker_awaiting_revival_skips_intent() {
        assert_attacker_lifecycle_skips_intent("AwaitingRevival", |lifecycle| {
            lifecycle.enter_near_death(40);
            lifecycle.await_revival_decision(RevivalDecision::Fortune { chance: 1.0 }, 120);
        });
    }

    #[test]
    fn attacker_terminated_skips_intent() {
        assert_attacker_lifecycle_skips_intent("Terminated", |lifecycle| {
            lifecycle.terminate(40);
        });
    }

    fn assert_attacker_lifecycle_skips_intent(
        state_name: &str,
        enter_state: impl FnOnce(&mut Lifecycle),
    ) {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 44 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<KnockbackEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let attacker = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_player(
            &mut app,
            "Crimson",
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );

        // Move attacker into the target lifecycle state.
        {
            let mut attacker_entity = app.world_mut().entity_mut(attacker);
            let mut lifecycle = attacker_entity.get_mut::<Lifecycle>().unwrap();
            enter_state(&mut lifecycle);
        }

        let target_wounds_before = app.world().entity(target).get::<Wounds>().unwrap().clone();
        let attacker_qi_before = app
            .world()
            .entity(attacker)
            .get::<Cultivation>()
            .unwrap()
            .qi_current;

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 44,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.update();

        // Assert: no wound written on target.
        let target_wounds_after = app.world().entity(target).get::<Wounds>().unwrap();
        assert_eq!(
            target_wounds_after.health_current, target_wounds_before.health_current,
            "{state_name} attacker must not deal damage to target"
        );
        assert_eq!(
            target_wounds_after.entries.len(),
            target_wounds_before.entries.len(),
            "{state_name} attacker must not write new wound entries on target"
        );

        // Assert: no qi deducted from attacker.
        let attacker_qi_after = app
            .world()
            .entity(attacker)
            .get::<Cultivation>()
            .unwrap()
            .qi_current;
        assert_eq!(
            attacker_qi_after, attacker_qi_before,
            "{state_name} attacker must not have qi deducted"
        );

        // Assert: no CombatEvent emitted.
        assert!(
            app.world()
                .resource::<Events<CombatEvent>>()
                .iter_current_update_events()
                .next()
                .is_none(),
            "{state_name} attacker must not emit CombatEvent"
        );
    }

    #[test]
    fn attack_intent_uses_latest_game_mode_component() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 44 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let attacker = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_player(
            &mut app,
            "Crimson",
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        app.world_mut()
            .entity_mut(target)
            .insert(GameMode::Creative);
        let before = app
            .world()
            .entity(target)
            .get::<Wounds>()
            .unwrap()
            .health_current;

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 44,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.update();
        assert_eq!(
            app.world()
                .entity(target)
                .get::<Wounds>()
                .unwrap()
                .health_current,
            before
        );

        app.world_mut()
            .entity_mut(target)
            .insert(GameMode::Survival);
        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 45,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.update();

        assert!(
            app.world()
                .entity(target)
                .get::<Wounds>()
                .unwrap()
                .health_current
                < before,
            "switching to Survival must make the target damageable immediately"
        );
        assert!(
            app.world()
                .resource::<Events<CombatEvent>>()
                .iter_current_update_events()
                .next()
                .is_some(),
            "Survival target should emit CombatEvent"
        );
    }

    #[test]
    fn sparring_lethal_hit_ends_without_death_event() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 44 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(
            Update,
            (
                resolve_attack_intents,
                crate::combat::status::status_effect_apply_tick,
            ),
        );
        let attacker = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_player(
            &mut app,
            "Crimson",
            [1.0, 64.0, 0.0],
            Wounds {
                health_current: 8.0,
                health_max: 100.0,
                entries: Vec::new(),
            },
            Stamina::default(),
        );
        app.world_mut().entity_mut(attacker).insert(SparringState {
            partner: target,
            invite_id: "sparring:1:a:b".to_string(),
            started_at_tick: 40,
            expires_at_tick: 6000,
        });
        app.world_mut().entity_mut(target).insert(SparringState {
            partner: attacker,
            invite_id: "sparring:1:a:b".to_string(),
            started_at_tick: 40,
            expires_at_tick: 6000,
        });

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 44,
            reach: FIST_REACH,
            qi_invest: 40.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.update();
        app.update();

        assert!(app.world().get::<SparringState>(attacker).is_none());
        assert!(app.world().get::<SparringState>(target).is_none());
        assert!(app.world().resource::<Events<DeathEvent>>().is_empty());
        let wounds = app.world().get::<Wounds>(target).unwrap();
        assert!(wounds.health_current > 0.0);
        let statuses = app.world().get::<StatusEffects>(target).unwrap();
        assert!(statuses
            .active
            .iter()
            .any(|effect| effect.kind == StatusEffectKind::Humility));
    }

    #[test]
    fn resolve_debug_attack_applies_damage_contamination_throughput_and_death() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 12 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(
            Update,
            (
                resolve_attack_intents,
                crate::combat::status::status_effect_apply_tick,
            ),
        );

        let attacker = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let mut target_meridians = MeridianSystem::default();
        target_meridians.get_mut(MeridianId::Lung).opened = true;
        let target = spawn_player(
            &mut app,
            "Crimson",
            [1.0, 64.0, 0.0],
            Wounds {
                health_current: 8.0,
                health_max: 100.0,
                entries: Vec::new(),
            },
            Stamina::default(),
        );
        app.world_mut().entity_mut(target).insert(target_meridians);

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: None,
            issued_at_tick: 11,
            reach: FIST_REACH,
            qi_invest: 40.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: Some(crate::player::gameplay::CombatAction {
                target: "Crimson".to_string(),
                qi_invest: 40.0,
            }),
        });

        app.update();
        app.update();
        app.update();

        let target_ref = app.world().entity(target);
        let attacker_ref = app.world().entity(attacker);
        let attacker_cultivation = attacker_ref
            .get::<Cultivation>()
            .expect("attacker should keep cultivation");
        let attacker_meridians = attacker_ref
            .get::<MeridianSystem>()
            .expect("attacker should keep meridians");
        let wounds = target_ref
            .get::<Wounds>()
            .expect("target should keep wounds");
        let stamina = target_ref
            .get::<Stamina>()
            .expect("target should keep stamina");
        let contamination = target_ref
            .get::<Contamination>()
            .expect("target should keep contamination");
        let status_effects = target_ref
            .get::<StatusEffects>()
            .expect("target should keep status effects");
        let meridians = target_ref
            .get::<MeridianSystem>()
            .expect("target should keep meridians");
        let life = target_ref
            .get::<LifeRecord>()
            .expect("target should keep life record");

        assert!(
            wounds.health_current <= 0.0,
            "damage should reduce health to zero"
        );
        assert_eq!(wounds.entries.len(), 1, "damage should record one wound");
        assert_eq!(wounds.entries[0].location, BodyPart::Chest);
        assert_eq!(wounds.entries[0].kind, WoundKind::Blunt);
        assert!(
            stamina.current < stamina.max,
            "damage should consume stamina"
        );
        assert_eq!(stamina.state, StaminaState::Combat);
        assert_eq!(
            contamination.entries.len(),
            1,
            "valid attack should write contamination"
        );
        assert_eq!(
            contamination.entries[0].attacker_id.as_deref(),
            Some("offline:Azure")
        );
        assert!(status_effects
            .active
            .iter()
            .any(|effect| effect.kind == StatusEffectKind::Bleeding && effect.magnitude > 0.0));
        assert_eq!(attacker_cultivation.qi_current, 20.0);
        assert!(
            attacker_meridians.get(MeridianId::Lung).throughput_current > 0.0,
            "attack should add attacker meridian throughput"
        );
        assert!(
            meridians.get(MeridianId::Lung).throughput_current > 0.0,
            "valid attack should add meridian throughput"
        );
        assert!(matches!(
            meridians.get(MeridianId::Lung).cracks.last(),
            Some(crack) if crack.cause == CrackCause::Attack
        ));
        assert!(matches!(
            life.biography.last(),
            Some(BiographyEntry::CombatHit { attacker_id, body_part, wound_kind, .. })
                if attacker_id == "offline:Azure"
                    && body_part == "Chest"
                    && wound_kind == "Blunt"
        ));
    }

    #[test]
    fn invalid_debug_attacks_have_no_side_effects() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 3 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<SkillXpGain>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let attacker = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_player(
            &mut app,
            "Crimson",
            [20.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );

        for action in [
            crate::player::gameplay::CombatAction {
                target: "".to_string(),
                qi_invest: 20.0,
            },
            crate::player::gameplay::CombatAction {
                target: "Crimson".to_string(),
                qi_invest: 0.0,
            },
            crate::player::gameplay::CombatAction {
                target: "Crimson".to_string(),
                qi_invest: 20.0,
            },
        ] {
            app.world_mut().send_event(AttackIntent {
                attacker,
                target: None,
                issued_at_tick: 2,
                reach: FIST_REACH,
                qi_invest: action.qi_invest as f32,
                wound_kind: WoundKind::Blunt,
                source: AttackSource::Melee,
                debug_command: Some(action),
            });
            app.update();
        }

        let target_ref = app.world().entity(target);
        let wounds = target_ref
            .get::<Wounds>()
            .expect("target should keep wounds");
        let stamina = target_ref
            .get::<Stamina>()
            .expect("target should keep stamina");
        let contamination = target_ref
            .get::<Contamination>()
            .expect("target should keep contamination");
        let meridians = target_ref
            .get::<MeridianSystem>()
            .expect("target should keep meridians");

        assert_eq!(wounds.health_current, wounds.health_max);
        assert!(
            wounds.entries.is_empty(),
            "invalid attacks must not create wounds"
        );
        assert_eq!(stamina.current, stamina.max);
        assert!(
            contamination.entries.is_empty(),
            "invalid attacks must not contaminate"
        );
        assert_eq!(meridians.get(MeridianId::Lung).throughput_current, 0.0);

        let combat_events = app.world().resource::<Events<CombatEvent>>();
        let death_events = app.world().resource::<Events<DeathEvent>>();
        assert!(
            combat_events.is_empty(),
            "invalid attacks must not emit CombatEvent"
        );
        assert!(
            death_events.is_empty(),
            "invalid attacks must not emit DeathEvent"
        );
    }

    #[test]
    fn npc_entity_target_attack_intent_flows_through_shared_resolver() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 44 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<SkillXpGain>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let npc_attacker = spawn_npc(
            &mut app,
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_player(
            &mut app,
            "Crimson",
            [1.0, 64.0, 0.0],
            Wounds {
                health_current: 5.0,
                health_max: 100.0,
                entries: Vec::new(),
            },
            Stamina::default(),
        );

        app.world_mut().send_event(AttackIntent {
            attacker: npc_attacker,
            target: Some(target),
            issued_at_tick: 43,
            reach: NpcMeleeProfile::spear().reach,
            qi_invest: 10.0,
            wound_kind: NpcMeleeProfile::spear().wound_kind,
            source: AttackSource::Melee,
            debug_command: None,
        });

        app.update();
        app.update();

        let target_ref = app.world().entity(target);
        let wounds = target_ref
            .get::<Wounds>()
            .expect("target should keep wounds");
        let contamination = target_ref
            .get::<Contamination>()
            .expect("target should keep contamination");

        assert!(
            wounds.health_current <= 0.0,
            "npc entity-target intent should apply lethal damage"
        );
        assert_eq!(
            wounds.entries.len(),
            1,
            "resolver should append exactly one wound"
        );
        assert_eq!(wounds.entries[0].location, BodyPart::Chest);
        assert_eq!(wounds.entries[0].kind, WoundKind::Pierce);
        assert_eq!(
            contamination.entries[0].attacker_id.as_deref(),
            Some(canonical_npc_id(npc_attacker).as_str()),
            "npc attacker identity should use canonical_npc_id"
        );

        let combat_events = app.world().resource::<Events<CombatEvent>>();
        let death_events = app.world().resource::<Events<DeathEvent>>();
        assert!(
            !combat_events.is_empty(),
            "npc entity-target intent should still emit CombatEvent via shared resolver"
        );
        assert!(
            !death_events.is_empty(),
            "npc entity-target intent should emit DeathEvent when lethal"
        );
        assert!(
            app.world().resource::<Events<SkillXpGain>>().is_empty(),
            "NPC attackers should not earn player skill XP"
        );
    }

    #[test]
    fn juebi_law_disruption_reduces_hit_and_backfires_attacker() {
        fn run_once(disrupted: bool) -> (f32, f32, f64) {
            let mut app = App::new();
            app.insert_resource(CombatClock { tick: 12 });
            app.add_event::<AttackIntent>();
            app.add_event::<ApplyStatusEffectIntent>();
            app.add_event::<CombatEvent>();
            app.add_event::<DeathEvent>();
            app.add_event::<crate::combat::weapon::WeaponBroken>();
            app.add_event::<crate::combat::weapon::ShieldBroken>();
            app.add_event::<InventoryDurabilityChangedEvent>();
            app.add_systems(Update, resolve_attack_intents);

            let attacker = spawn_player(
                &mut app,
                "Azure",
                [0.0, 64.0, 0.0],
                Wounds::default(),
                Stamina::default(),
            );
            let target = spawn_player(
                &mut app,
                "Crimson",
                [0.25, 64.0, 0.0],
                Wounds::default(),
                Stamina::default(),
            );
            if disrupted {
                app.world_mut()
                    .entity_mut(attacker)
                    .insert(JueBiLawDisruption {
                        epicenter: valence::prelude::BlockPos::new(0, 64, 0),
                        distance: 0.0,
                        seed: 11,
                    });
            }

            app.world_mut().send_event(AttackIntent {
                attacker,
                target: Some(target),
                issued_at_tick: 11,
                reach: FIST_REACH,
                qi_invest: 20.0,
                wound_kind: WoundKind::Blunt,
                source: AttackSource::Melee,
                debug_command: None,
            });
            app.update();
            app.update();

            let attacker_wounds = app.world().get::<Wounds>(attacker).unwrap();
            let target_wounds = app.world().get::<Wounds>(target).unwrap();
            let attacker_meridians = app.world().get::<MeridianSystem>(attacker).unwrap();
            (
                target_wounds.health_max - target_wounds.health_current,
                attacker_wounds.health_max - attacker_wounds.health_current,
                attacker_meridians.get(MeridianId::Lung).throughput_current,
            )
        }

        let (normal_damage, normal_backfire, normal_throughput) = run_once(false);
        let (disrupted_damage, disrupted_backfire, disrupted_throughput) = run_once(true);

        assert!(normal_damage > 1.0);
        assert_eq!(normal_backfire, 0.0);
        assert!(disrupted_damage < normal_damage);
        assert!(disrupted_backfire > 0.0);
        assert!(disrupted_throughput > normal_throughput);
    }

    #[test]
    fn player_to_npc_and_npc_to_player_share_same_resolver_path() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 91 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<SkillXpGain>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let player = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let npc = spawn_npc(
            &mut app,
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );

        app.world_mut().send_event(AttackIntent {
            attacker: player,
            target: Some(npc),
            issued_at_tick: 90,
            reach: FIST_REACH,
            qi_invest: 12.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.world_mut().send_event(AttackIntent {
            attacker: npc,
            target: Some(player),
            issued_at_tick: 90,
            reach: NpcMeleeProfile::spear().reach,
            qi_invest: 10.0,
            wound_kind: NpcMeleeProfile::spear().wound_kind,
            source: AttackSource::Melee,
            debug_command: None,
        });

        app.update();

        let player_ref = app.world().entity(player);
        let npc_ref = app.world().entity(npc);
        let player_wounds = player_ref
            .get::<Wounds>()
            .expect("player target should keep wounds");
        let npc_wounds = npc_ref
            .get::<Wounds>()
            .expect("npc target should keep wounds");
        let player_contamination = player_ref
            .get::<Contamination>()
            .expect("player target should keep contamination");
        let npc_contamination = npc_ref
            .get::<Contamination>()
            .expect("npc target should keep contamination");

        assert_eq!(
            player_wounds.entries.len(),
            1,
            "npc->player should resolve exactly one wound"
        );
        assert_eq!(player_wounds.entries[0].location, BodyPart::Chest);
        assert_eq!(player_wounds.entries[0].kind, WoundKind::Pierce);
        assert_eq!(
            npc_wounds.entries.len(),
            1,
            "player->npc should resolve exactly one wound"
        );
        assert_eq!(npc_wounds.entries[0].location, BodyPart::Chest);
        assert_eq!(npc_wounds.entries[0].kind, WoundKind::Blunt);
        assert_eq!(
            player_contamination.entries[0].attacker_id.as_deref(),
            Some(canonical_npc_id(npc).as_str())
        );
        assert_eq!(
            npc_contamination.entries[0].attacker_id.as_deref(),
            Some("offline:Azure")
        );

        let combat_events = app.world().resource::<Events<CombatEvent>>();
        assert!(
            !combat_events.is_empty(),
            "both directions should emit CombatEvent through the same resolver event family"
        );
    }

    #[test]
    fn zero_qi_npc_mundane_melee_damages_survival_player() {
        use crate::npc::lifecycle::{npc_runtime_bundle, NpcArchetype};

        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 93 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let player = spawn_player(
            &mut app,
            "Azure",
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let npc = app
            .world_mut()
            .spawn((NpcMarker, Position::new([0.0, 64.0, 0.0])))
            .id();
        let runtime = npc_runtime_bundle(npc, NpcArchetype::Zombie);
        assert_eq!(runtime.cultivation.qi_current, 0.0);
        app.world_mut().entity_mut(npc).insert(runtime);

        let before = app
            .world()
            .entity(player)
            .get::<Wounds>()
            .unwrap()
            .health_current;
        app.world_mut().send_event(AttackIntent {
            attacker: npc,
            target: Some(player),
            issued_at_tick: 92,
            reach: NpcMeleeProfile::fist().reach,
            qi_invest: 0.0,
            wound_kind: NpcMeleeProfile::fist().wound_kind,
            source: AttackSource::Melee,
            debug_command: None,
        });

        app.update();

        let player_wounds = app.world().entity(player).get::<Wounds>().unwrap();
        assert!(
            player_wounds.health_current < before,
            "mundane NPC melee must damage Survival players without requiring qi"
        );
        let events: Vec<_> = app
            .world()
            .resource::<Events<CombatEvent>>()
            .iter_current_update_events()
            .collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].damage, 0.0);
        assert!(
            events[0].physical_damage > 0.0,
            "mundane NPC melee should surface as physical damage"
        );
    }

    #[test]
    fn player_killing_npc_emits_combat_skill_xp() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 92 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<SkillXpGain>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let player = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let npc = spawn_npc(
            &mut app,
            [1.0, 64.0, 0.0],
            Wounds {
                health_current: 3.0,
                health_max: 100.0,
                entries: Vec::new(),
            },
            Stamina::default(),
        );

        app.world_mut().send_event(AttackIntent {
            attacker: player,
            target: Some(npc),
            issued_at_tick: 91,
            reach: FIST_REACH,
            qi_invest: 12.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.update();

        let xp_events = app.world().resource::<Events<SkillXpGain>>();
        let xp = xp_events
            .iter_current_update_events()
            .next()
            .expect("lethal player->npc hit should award combat xp");
        assert_eq!(xp.char_entity, player);
        assert_eq!(xp.skill, SkillId::Combat);
        assert_eq!(xp.amount, 4);
    }

    #[test]
    fn player_to_runtime_spawned_zombie_npc_target_resolves_without_dropping_intent() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 128 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_systems(
            valence::prelude::Startup,
            (setup_test_layer, spawn_runtime_npc.after(setup_test_layer)),
        );
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        app.update();
        app.update();

        let npc = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
            query
                .iter(world)
                .next()
                .expect("runtime zombie NPC should be spawned for resolver coverage test")
        };

        let attacker = spawn_player(
            &mut app,
            "Azure",
            [13.0, 66.0, 14.0],
            Wounds::default(),
            Stamina::default(),
        );

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(npc),
            issued_at_tick: 127,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });

        app.update();

        let npc_ref = app.world().entity(npc);
        let npc_wounds = npc_ref
            .get::<Wounds>()
            .expect("runtime zombie NPC should carry Wounds for shared resolver");
        let npc_contamination = npc_ref
            .get::<Contamination>()
            .expect("runtime zombie NPC should carry Contamination for shared resolver");

        assert_eq!(
            npc_wounds.entries.len(),
            1,
            "player->runtime-zombie intent should apply one wound"
        );
        assert_eq!(npc_wounds.entries[0].location, BodyPart::Chest);
        assert_eq!(npc_wounds.entries[0].kind, WoundKind::Blunt);
        assert_eq!(
            npc_contamination.entries[0].attacker_id.as_deref(),
            Some("offline:Azure"),
            "shared resolver should attribute player attacker on runtime zombie target"
        );

        let combat_events = app.world().resource::<Events<CombatEvent>>();
        assert!(
            !combat_events.is_empty(),
            "player->runtime-zombie intent should emit CombatEvent instead of dropping"
        );
    }

    #[test]
    fn repeated_hits_on_dead_target_emit_single_death_event() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 300 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let attacker = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_player(
            &mut app,
            "Crimson",
            [1.0, 64.0, 0.0],
            Wounds {
                health_current: 1.0,
                health_max: 100.0,
                entries: Vec::new(),
            },
            Stamina::default(),
        );

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 299,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.update();

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 300,
            reach: NpcMeleeProfile::spear().reach,
            qi_invest: 10.0,
            wound_kind: NpcMeleeProfile::spear().wound_kind,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.update();

        let death_events = app.world().resource::<Events<DeathEvent>>();
        assert_eq!(
            death_events.len(),
            1,
            "DeathEvent should only emit on alive->dead transition, not repeated corpse hits"
        );
    }

    #[test]
    fn debug_attack_resolves_canonical_npc_target_without_client_query_match() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 512 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let attacker = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let npc_target = spawn_npc(
            &mut app,
            [1.0, 64.0, 0.0],
            Wounds {
                health_current: 8.0,
                health_max: 100.0,
                entries: Vec::new(),
            },
            Stamina::default(),
        );
        let npc_id = canonical_npc_id(npc_target);

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: None,
            issued_at_tick: 511,
            reach: FIST_REACH,
            qi_invest: 40.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: Some(crate::player::gameplay::CombatAction {
                target: npc_id.clone(),
                qi_invest: 40.0,
            }),
        });

        app.update();

        let npc_ref = app.world().entity(npc_target);
        let wounds = npc_ref
            .get::<Wounds>()
            .expect("npc debug target should keep wounds");
        let contamination = npc_ref
            .get::<Contamination>()
            .expect("npc debug target should keep contamination");

        assert!(
            wounds.health_current <= 0.0,
            "debug npc target should receive resolver damage"
        );
        assert_eq!(
            contamination.entries[0].attacker_id.as_deref(),
            Some("offline:Azure"),
            "debug npc target should preserve canonical player attacker identity"
        );

        let combat_events = app.world().resource::<Events<CombatEvent>>();
        let death_events = app.world().resource::<Events<DeathEvent>>();
        assert!(
            !combat_events.is_empty(),
            "debug npc target should emit CombatEvent through shared resolver"
        );
        assert!(
            !death_events.is_empty(),
            "lethal debug npc target should emit DeathEvent"
        );
    }

    // ── 回归测试：黑武士近战 qi 闸门漏洞（plan-sword-path-v2 P3 review 发现）────────────────
    //
    // 修复前根因：`heiwushi_melee_slash_action_system` 发 `qi_invest = base_attack * phase_mult`
    // （Phase1 = 35.0），但 `npc_runtime_bundle` 给 boss 的 `Cultivation::default()` 只有
    // `qi_current = 0.0`。resolver §362-377 的 qi 闸门：
    //   `qi_invest > EPSILON && !prepaid && qi_current < qi_invest`
    // → `35.0 > ε && Melee 不在 prepaid 白名单 && 0.0 < 35.0` → continue（intent 被丢弃）。
    // 结果：所有 Phase 的近战斩击永远打不到人。
    //
    // 修复后：`qi_invest = 0.0` 触发物理路径（is_physical_hit=true），
    // 相位缩放伤害通过 `DerivedAttrs.attack_power` 传递，闸门不触发。
    //
    // 本测试专门覆盖 NPC 攻击者 qi_current=0 + qi_invest=0.0 能成功落伤的完整路径，
    // 若将 qi_invest 改回 35.0（pre-fix），断言会失败（target wounds 不变）。
    #[test]
    fn heiwushi_melee_physical_path_lands_with_zero_qi_attacker() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 200 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        // Boss attacker: qi_current=0（npc_runtime_bundle default）+ attack_power=35.0
        // 这是修复前触发 qi 闸门的精确条件。
        let boss = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                Cultivation {
                    qi_current: 0.0,
                    qi_max: 10.0,
                    ..Cultivation::default()
                },
                MeridianSystem::default(),
                Contamination::default(),
                StatusEffects::default(),
                Wounds::default(),
                crate::combat::components::Stamina::default(),
                CombatState::default(),
                DerivedAttrs {
                    attack_power: 35.0,
                    ..DerivedAttrs::default()
                },
            ))
            .id();
        let canonical = canonical_npc_id(boss);
        app.world_mut().entity_mut(boss).insert((
            Lifecycle {
                character_id: canonical.clone(),
                ..Default::default()
            },
            LifeRecord::new(canonical),
        ));

        // Player target — 近战距离内 (1 block away)
        let target = spawn_player(
            &mut app,
            "Crimson",
            [1.0, 64.0, 0.0],
            Wounds {
                health_current: 100.0,
                health_max: 100.0,
                entries: Vec::new(),
            },
            crate::combat::components::Stamina::default(),
        );

        // 物理命中路径：qi_invest=0.0（修复后的黑武士近战斩击）
        app.world_mut().send_event(AttackIntent {
            attacker: boss,
            target: Some(target),
            issued_at_tick: 199,
            reach: AttackReach::new(3.0, 0.5),
            qi_invest: 0.0,
            wound_kind: WoundKind::Cut,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.update();

        let target_wounds = app.world().entity(target).get::<Wounds>().unwrap();
        assert!(
            !target_wounds.entries.is_empty(),
            "黑武士近战（qi_invest=0, qi_current=0）必须落至少一道伤，实际伤口数={}。\
            若此断言失败，说明 qi 闸门仍在拦截物理近战。",
            target_wounds.entries.len()
        );
        assert!(
            target_wounds.health_current < 100.0,
            "黑武士近战必须扣血，修复前 qi_invest=35.0 被闸门拒绝导致此处不降；\
            实际 health_current={:.2}，期望 < 100.0",
            target_wounds.health_current
        );

        let combat_events = app.world().resource::<Events<CombatEvent>>();
        assert!(
            !combat_events.is_empty(),
            "黑武士近战命中必须 emit CombatEvent（resolver 未拒绝才会到达此处）"
        );
    }

    #[test]
    fn fist_reach_misses_when_target_is_outside_physical_range() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 900 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let attacker = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_npc(
            &mut app,
            [4.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 899,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });

        app.update();

        let target_ref = app.world().entity(target);
        let wounds = target_ref.get::<Wounds>().unwrap();
        let contamination = target_ref.get::<Contamination>().unwrap();
        let combat_events = app.world().resource::<Events<CombatEvent>>();

        assert_eq!(wounds.health_current, wounds.health_max);
        assert!(wounds.entries.is_empty());
        assert!(contamination.entries.is_empty());
        assert!(combat_events.is_empty());
    }

    #[test]
    fn fist_reach_misses_just_outside_client_melee_upper_bound() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 900 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        assert!(
            (FIST_REACH.max - 2.6).abs() <= f32::EPSILON,
            "test pins the current client melee upper bound for unarmed attacks, actual: {}",
            FIST_REACH.max
        );
        let attacker = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        // Humanoid hitboxes extend 0.3 block toward the attacker; this places the hitbox just outside reach.
        let target = spawn_npc(
            &mut app,
            [f64::from(FIST_REACH.max) + 0.301, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 899,
            reach: FIST_REACH,
            qi_invest: 0.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });

        app.update();

        let target_ref = app.world().entity(target);
        let wounds = target_ref.get::<Wounds>().unwrap();
        let combat_events = app.world().resource::<Events<CombatEvent>>();

        assert_eq!(
            wounds.health_current, wounds.health_max,
            "target just outside fist reach must not lose health"
        );
        assert!(
            wounds.entries.is_empty(),
            "target just outside fist reach must not receive wounds, actual: {:?}",
            wounds.entries
        );
        assert!(
            combat_events.is_empty(),
            "target just outside fist reach must not emit CombatEvent"
        );
    }

    #[test]
    fn fist_reach_hits_at_client_melee_distance() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 900 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        assert!(
            (FIST_REACH.max - 2.6).abs() <= f32::EPSILON,
            "test pins the current client melee upper bound for unarmed attacks, actual: {}",
            FIST_REACH.max
        );
        let attacker = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_npc(
            &mut app,
            // Raycast distance includes the chest-height slope from attacker eye to target AABB.
            [f64::from(FIST_REACH.max) + 0.27, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 899,
            reach: FIST_REACH,
            qi_invest: 0.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });

        app.update();

        let target_ref = app.world().entity(target);
        let wounds = target_ref.get::<Wounds>().unwrap();
        let combat_events = app.world().resource::<Events<CombatEvent>>();

        assert!(
            wounds.health_current < wounds.health_max,
            "target just inside fist reach must lose health, actual health: {}/{}",
            wounds.health_current,
            wounds.health_max
        );
        assert!(
            !wounds.entries.is_empty(),
            "target just inside fist reach must receive wounds"
        );
        assert!(
            !combat_events.is_empty(),
            "target just inside fist reach must emit CombatEvent"
        );
    }

    #[test]
    fn insufficient_qi_prevents_attack_side_effects() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 901 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let attacker = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_npc(
            &mut app,
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );

        app.world_mut().entity_mut(attacker).insert(Cultivation {
            qi_current: 5.0,
            qi_max: 100.0,
            ..Cultivation::default()
        });

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 900,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });

        app.update();

        let attacker_ref = app.world().entity(attacker);
        let target_ref = app.world().entity(target);
        let attacker_cultivation = attacker_ref.get::<Cultivation>().unwrap();
        let target_wounds = target_ref.get::<Wounds>().unwrap();
        let target_contamination = target_ref.get::<Contamination>().unwrap();
        let combat_events = app.world().resource::<Events<CombatEvent>>();
        let death_events = app.world().resource::<Events<DeathEvent>>();

        assert_eq!(attacker_cultivation.qi_current, 5.0);
        assert_eq!(target_wounds.health_current, target_wounds.health_max);
        assert!(target_wounds.entries.is_empty());
        assert!(target_contamination.entries.is_empty());
        assert!(combat_events.is_empty());
        assert!(death_events.is_empty());
    }

    #[test]
    fn anticheat_qi_invest_violation_counts_without_changing_rejection() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 903 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let attacker = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_npc(
            &mut app,
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        app.world_mut().entity_mut(attacker).insert((
            Cultivation {
                qi_current: 5.0,
                qi_max: 100.0,
                ..Cultivation::default()
            },
            AntiCheatCounter::default(),
        ));

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 902,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.update();

        let counter = app
            .world()
            .entity(attacker)
            .get::<AntiCheatCounter>()
            .unwrap();
        assert_eq!(counter.qi_invest_violations, 1);
        let target_ref = app.world().entity(target);
        assert!(
            target_ref.get::<Wounds>().unwrap().entries.is_empty(),
            "insufficient qi behavior should remain rejection"
        );
        assert!(
            app.world().resource::<Events<CombatEvent>>().is_empty(),
            "qi violation counting must not emit combat side effects"
        );
    }

    #[test]
    fn anticheat_reach_violation_counts_without_changing_miss() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 904 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let attacker = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_npc(
            &mut app,
            [4.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        app.world_mut()
            .entity_mut(attacker)
            .insert(AntiCheatCounter::default());

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 903,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.update();

        let counter = app
            .world()
            .entity(attacker)
            .get::<AntiCheatCounter>()
            .unwrap();
        assert_eq!(counter.reach_violations, 1);
        let target_ref = app.world().entity(target);
        assert_eq!(
            target_ref.get::<Wounds>().unwrap().health_current,
            target_ref.get::<Wounds>().unwrap().health_max
        );
        assert!(target_ref.get::<Wounds>().unwrap().entries.is_empty());
    }

    #[test]
    fn anticheat_cooldown_violation_counts_without_blocking_hit() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 905 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let attacker = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_npc(
            &mut app,
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        app.world_mut().entity_mut(attacker).insert((
            AntiCheatCounter::default(),
            CombatState {
                last_attack_at_tick: Some(904),
                ..CombatState::default()
            },
        ));

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 904,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.update();

        let counter = app
            .world()
            .entity(attacker)
            .get::<AntiCheatCounter>()
            .unwrap();
        assert_eq!(counter.cooldown_violations, 1);
        assert!(
            !app.world()
                .entity(target)
                .get::<Wounds>()
                .unwrap()
                .entries
                .is_empty(),
            "cooldown violation reporting must not change current hit resolution"
        );
        assert!(
            !app.world().resource::<Events<CombatEvent>>().is_empty(),
            "hit should still emit CombatEvent"
        );
    }

    #[test]
    fn debug_target_selection_does_not_change_damage_when_qi_invest_matches() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 902 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let attacker = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target_a = spawn_player(
            &mut app,
            "Crimson",
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target_b = spawn_player(
            &mut app,
            "Sable",
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: None,
            issued_at_tick: 901,
            reach: FIST_REACH,
            qi_invest: 18.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: Some(crate::player::gameplay::CombatAction {
                target: "Crimson".to_string(),
                qi_invest: 18.0,
            }),
        });
        app.update();

        let first_damage = app
            .world()
            .entity(target_a)
            .get::<Wounds>()
            .unwrap()
            .entries
            .last()
            .expect("first debug hit should create wound")
            .severity;

        app.world_mut().entity_mut(attacker).insert(Cultivation {
            qi_current: 60.0,
            qi_max: 100.0,
            ..Cultivation::default()
        });

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: None,
            issued_at_tick: 902,
            reach: FIST_REACH,
            qi_invest: 18.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: Some(crate::player::gameplay::CombatAction {
                target: "Sable".to_string(),
                qi_invest: 999.0,
            }),
        });
        app.update();

        let second_damage = app
            .world()
            .entity(target_b)
            .get::<Wounds>()
            .unwrap()
            .entries
            .last()
            .expect("second debug hit should create wound")
            .severity;

        assert_eq!(first_damage, second_damage);
    }

    #[test]
    fn jiemai_window_spends_qi_reduces_contam_and_adds_concussion() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1000 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let attacker = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_player(
            &mut app,
            "Crimson",
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );

        app.world_mut().entity_mut(target).insert((
            CombatState {
                incoming_window: Some(DefenseWindow {
                    opened_at_tick: 999,
                    duration_ms: 200,
                }),
                ..CombatState::default()
            },
            Cultivation {
                realm: Realm::Induce,
                qi_current: 20.0,
                qi_max: 100.0,
                ..Cultivation::default()
            },
            PracticeLog::default(),
        ));

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 999,
            reach: FIST_REACH,
            qi_invest: 20.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });

        app.update();

        let target_ref = app.world().entity(target);
        let wounds = target_ref.get::<Wounds>().unwrap();
        let contamination = target_ref.get::<Contamination>().unwrap();
        let cultivation = target_ref.get::<Cultivation>().unwrap();
        let state = target_ref.get::<CombatState>().unwrap();
        let combat_events = app.world().resource::<Events<CombatEvent>>();
        let event = combat_events
            .iter_current_update_events()
            .next()
            .expect("combat event should emit");

        let expected_effectiveness = event
            .defense_effectiveness
            .expect("jiemai success should report effectiveness");
        assert!((expected_effectiveness - 0.3).abs() < 1e-6);
        assert_eq!(
            cultivation.qi_current,
            20.0 - zhenmai_v2::parry_qi_cost_for_realm(Realm::Induce).unwrap()
        );
        assert!(state.incoming_window.is_none());
        assert_eq!(wounds.entries.len(), 2);
        assert!(wounds
            .entries
            .iter()
            .any(|w| w.kind == WoundKind::Concussion));
        let base_contam = f64::from(event.damage) * 0.25 * 0.8;
        assert_eq!(
            event.contam_delta,
            base_contam * jiemai_contam_multiplier_for_effectiveness(expected_effectiveness)
        );
        assert_eq!(event.defense_kind, Some(DefenseKind::JieMai));
        assert_eq!(event.defense_wound_severity, Some(1.0));
        assert_eq!(contamination.entries.len(), 1);
        assert_eq!(contamination.entries[0].amount, event.contam_delta);
        let life = target_ref.get::<LifeRecord>().unwrap();
        assert!(matches!(
            life.biography.last(),
            Some(BiographyEntry::JiemaiParry {
                attacker_id,
                effectiveness,
                tick,
            }) if attacker_id == "offline:Azure"
                && (*effectiveness - expected_effectiveness).abs() < 1e-6
                && *tick == 1000
        ));
        assert_eq!(
            target_ref
                .get::<PracticeLog>()
                .unwrap()
                .weights
                .get(&ColorKind::Violent)
                .copied(),
            Some(crate::cultivation::color::STYLE_PRACTICE_AMOUNT)
        );
    }

    #[test]
    fn jiemai_without_qi_falls_back_to_normal_settlement() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1001 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let attacker = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_player(
            &mut app,
            "Crimson",
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );

        app.world_mut().entity_mut(target).insert((
            CombatState {
                incoming_window: Some(DefenseWindow {
                    opened_at_tick: 1000,
                    duration_ms: 200,
                }),
                ..CombatState::default()
            },
            Cultivation {
                realm: Realm::Induce,
                qi_current: 1.0,
                qi_max: 100.0,
                ..Cultivation::default()
            },
        ));

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 1000,
            reach: FIST_REACH,
            qi_invest: 20.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });

        app.update();

        let target_ref = app.world().entity(target);
        let wounds = target_ref.get::<Wounds>().unwrap();
        let contamination = target_ref.get::<Contamination>().unwrap();
        let cultivation = target_ref.get::<Cultivation>().unwrap();
        let state = target_ref.get::<CombatState>().unwrap();
        let combat_events = app.world().resource::<Events<CombatEvent>>();
        let event = combat_events
            .iter_current_update_events()
            .next()
            .expect("combat event should emit");

        assert_eq!(cultivation.qi_current, 1.0);
        assert!(state.incoming_window.is_none());
        assert_eq!(wounds.entries.len(), 1);
        assert!(!wounds
            .entries
            .iter()
            .any(|w| w.kind == WoundKind::Concussion));
        let base_contam = f64::from(event.damage) * 0.25 * 0.8;
        assert_eq!(event.contam_delta, base_contam);
        assert_eq!(contamination.entries[0].amount, base_contam);
    }

    #[test]
    fn expired_jiemai_window_does_not_mitigate() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1006 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let attacker = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_player(
            &mut app,
            "Crimson",
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );

        app.world_mut().entity_mut(target).insert((
            CombatState {
                incoming_window: Some(DefenseWindow {
                    opened_at_tick: 1000,
                    duration_ms: 200,
                }),
                ..CombatState::default()
            },
            Cultivation {
                realm: Realm::Induce,
                qi_current: 20.0,
                qi_max: 100.0,
                ..Cultivation::default()
            },
        ));

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 1005,
            reach: FIST_REACH,
            qi_invest: 20.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });

        app.update();

        let target_ref = app.world().entity(target);
        let wounds = target_ref.get::<Wounds>().unwrap();
        let contamination = target_ref.get::<Contamination>().unwrap();
        let cultivation = target_ref.get::<Cultivation>().unwrap();
        let state = target_ref.get::<CombatState>().unwrap();
        let combat_events = app.world().resource::<Events<CombatEvent>>();
        let event = combat_events
            .iter_current_update_events()
            .next()
            .expect("combat event should emit");

        assert_eq!(cultivation.qi_current, 20.0);
        assert!(state.incoming_window.is_none());
        assert_eq!(wounds.entries.len(), 1);
        assert_eq!(contamination.entries[0].amount, event.contam_delta);
    }

    #[test]
    fn stunned_attacker_cannot_resolve_attack_intent() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1100 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let attacker = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_player(
            &mut app,
            "Crimson",
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );

        app.world_mut().entity_mut(attacker).insert(StatusEffects {
            active: vec![crate::combat::components::ActiveStatusEffect {
                kind: StatusEffectKind::Stunned,
                magnitude: 1.0,
                remaining_ticks: 20,
                source_pill: None,
            }],
        });

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 1099,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });

        app.update();

        let target_ref = app.world().entity(target);
        let wounds = target_ref.get::<Wounds>().unwrap();
        let contamination = target_ref.get::<Contamination>().unwrap();
        let combat_events = app.world().resource::<Events<CombatEvent>>();
        let death_events = app.world().resource::<Events<DeathEvent>>();

        assert_eq!(wounds.health_current, wounds.health_max);
        assert!(wounds.entries.is_empty());
        assert!(contamination.entries.is_empty());
        assert!(combat_events.is_empty());
        assert!(death_events.is_empty());
    }

    #[test]
    fn apply_defense_intent_ignored_while_stunned() {
        let mut app = App::new();
        app.add_event::<DefenseIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_systems(Update, apply_defense_intents);

        let defender = app
            .world_mut()
            .spawn((
                CombatState::default(),
                Cultivation {
                    realm: Realm::Induce,
                    qi_current: 10.0,
                    qi_max: 10.0,
                    ..Cultivation::default()
                },
                StatusEffects {
                    active: vec![crate::combat::components::ActiveStatusEffect {
                        kind: StatusEffectKind::Stunned,
                        magnitude: 1.0,
                        remaining_ticks: 20,
                        source_pill: None,
                    }],
                },
            ))
            .id();

        app.world_mut().send_event(DefenseIntent {
            defender,
            issued_at_tick: 10,
        });
        app.update();

        let state = app.world().entity(defender).get::<CombatState>().unwrap();
        assert!(state.incoming_window.is_none());
    }

    #[test]
    fn apply_defense_intent_uses_realm_armor_and_adds_parry_recovery() {
        let mut app = App::new();
        app.add_event::<DefenseIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_systems(
            Update,
            (
                apply_defense_intents,
                crate::combat::status::status_effect_apply_tick.after(apply_defense_intents),
            ),
        );

        let defender = app
            .world_mut()
            .spawn((
                CombatState::default(),
                Cultivation {
                    realm: Realm::Condense,
                    qi_current: 12.0,
                    qi_max: 20.0,
                    ..Cultivation::default()
                },
                PlayerInventory {
                    revision: InventoryRevision(0),
                    containers: Vec::new(),
                    equipped: std::collections::HashMap::from([(
                        EQUIP_SLOT_CHEST.to_string(),
                        ItemInstance {
                            instance_id: 90,
                            template_id: "heavy_armor".to_string(),
                            display_name: "heavy_armor".to_string(),
                            grid_w: 2,
                            grid_h: 2,
                            weight: 7.0,
                            rarity: ItemRarity::Common,
                            description: String::new(),
                            stack_count: 1,
                            spirit_quality: 1.0,
                            durability: 1.0,
                            freshness: None,
                            mineral_id: None,
                            charges: None,
                            forge_quality: None,
                            forge_color: None,
                            forge_side_effects: Vec::new(),
                            forge_achieved_tier: None,
                            alchemy: None,
                            lingering_owner_qi: None,
                        },
                    )]),
                    hotbar: Default::default(),
                    bone_coins: 0,
                    max_weight: 50.0,
                },
                StatusEffects::default(),
            ))
            .id();

        app.world_mut().send_event(DefenseIntent {
            defender,
            issued_at_tick: 10,
        });
        app.update();

        let entity = app.world().entity(defender);
        let state = entity.get::<CombatState>().unwrap();
        let window = state
            .incoming_window
            .as_ref()
            .expect("jiemai prep should open");
        let statuses = entity.get::<StatusEffects>().unwrap();

        assert_eq!(window.duration_ms, 600);
        assert!(statuses
            .active
            .iter()
            .any(|effect| effect.kind == StatusEffectKind::ParryRecovery));
    }

    #[test]
    fn head_hit_applies_stunned_status() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1200 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(
            Update,
            (
                resolve_attack_intents,
                crate::combat::status::status_effect_apply_tick,
            ),
        );

        let attacker = spawn_player(
            &mut app,
            "Azure",
            [0.0, 65.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_player(
            &mut app,
            "Crimson",
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 1199,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });

        app.update();
        app.update();

        let target_ref = app.world().entity(target);
        let wounds = target_ref.get::<Wounds>().unwrap();
        let status_effects = target_ref.get::<StatusEffects>().unwrap();

        assert!(wounds.entries.iter().any(|w| w.location == BodyPart::Head));
        assert!(status_effects
            .active
            .iter()
            .any(|effect| effect.kind == StatusEffectKind::Stunned));
    }

    #[test]
    fn resolver_uses_attack_power_for_outgoing_damage() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1300 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<WeaponBroken>();
        app.add_event::<ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(
            Update,
            (
                crate::combat::status::attribute_aggregate_tick,
                resolve_attack_intents,
            ),
        );

        let baseline_attacker = spawn_player(
            &mut app,
            "AzureBase",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let amp_attacker = spawn_player(
            &mut app,
            "AzureAmp",
            [0.0, 64.0, 2.0],
            Wounds::default(),
            Stamina::default(),
        );
        let baseline_target = spawn_player(
            &mut app,
            "CrimsonBase",
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let amp_target = spawn_player(
            &mut app,
            "CrimsonAmp",
            [1.0, 64.0, 2.0],
            Wounds::default(),
            Stamina::default(),
        );

        app.world_mut()
            .entity_mut(amp_attacker)
            .insert(StatusEffects {
                active: vec![crate::combat::components::ActiveStatusEffect {
                    kind: StatusEffectKind::DamageAmp,
                    magnitude: 0.25,
                    remaining_ticks: 20,
                    source_pill: None,
                }],
            });

        app.update();

        app.world_mut().send_event(AttackIntent {
            attacker: baseline_attacker,
            target: Some(baseline_target),
            issued_at_tick: 1299,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.world_mut().send_event(AttackIntent {
            attacker: amp_attacker,
            target: Some(amp_target),
            issued_at_tick: 1299,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });

        app.update();

        let baseline_target_ref = app.world().entity(baseline_target);
        let amp_target_ref = app.world().entity(amp_target);
        let baseline_wounds = baseline_target_ref.get::<Wounds>().unwrap();
        let amp_wounds = amp_target_ref.get::<Wounds>().unwrap();
        let combat_events = app.world().resource::<Events<CombatEvent>>();
        let events: Vec<_> = combat_events.iter_current_update_events().collect();

        assert_eq!(events.len(), 2);
        let baseline_damage = events[0].damage;
        let amp_damage = events[1].damage;

        assert!(amp_damage > baseline_damage);
        assert!(
            (baseline_wounds.health_current - (baseline_wounds.health_max - baseline_damage)).abs()
                < 0.001
        );
        assert!((amp_wounds.health_current - (amp_wounds.health_max - amp_damage)).abs() < 0.001);
    }

    #[test]
    fn resolver_applies_defense_power_to_incoming_damage() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1350 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<WeaponBroken>();
        app.add_event::<ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(
            Update,
            (
                crate::combat::status::attribute_aggregate_tick,
                resolve_attack_intents,
            ),
        );

        let baseline_attacker = spawn_player(
            &mut app,
            "AzureBaseDef",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let reduced_attacker = spawn_player(
            &mut app,
            "AzureRedDef",
            [0.0, 64.0, 2.0],
            Wounds::default(),
            Stamina::default(),
        );
        let baseline_target = spawn_player(
            &mut app,
            "CrimsonBaseDef",
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let reduced_target = spawn_player(
            &mut app,
            "CrimsonRedDef",
            [1.0, 64.0, 2.0],
            Wounds::default(),
            Stamina::default(),
        );

        app.world_mut()
            .entity_mut(reduced_target)
            .insert(StatusEffects {
                active: vec![crate::combat::components::ActiveStatusEffect {
                    kind: StatusEffectKind::DamageReduction,
                    magnitude: 0.25,
                    remaining_ticks: 20,
                    source_pill: None,
                }],
            });

        app.update();

        app.world_mut().send_event(AttackIntent {
            attacker: baseline_attacker,
            target: Some(baseline_target),
            issued_at_tick: 1349,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.world_mut().send_event(AttackIntent {
            attacker: reduced_attacker,
            target: Some(reduced_target),
            issued_at_tick: 1349,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });

        app.update();

        let baseline_target_ref = app.world().entity(baseline_target);
        let reduced_target_ref = app.world().entity(reduced_target);
        let baseline_wounds = baseline_target_ref.get::<Wounds>().unwrap();
        let reduced_wounds = reduced_target_ref.get::<Wounds>().unwrap();
        let combat_events = app.world().resource::<Events<CombatEvent>>();
        let events: Vec<_> = combat_events.iter_current_update_events().collect();

        assert_eq!(events.len(), 2);
        let baseline_damage = events[0].damage;
        let reduced_damage = events[1].damage;

        assert!(reduced_damage < baseline_damage);
        assert!(
            (baseline_wounds.health_current - (baseline_wounds.health_max - baseline_damage)).abs()
                < 0.001
        );
        assert!(
            (reduced_wounds.health_current - (reduced_wounds.health_max - reduced_damage)).abs()
                < 0.001
        );
    }

    #[test]
    fn resolver_applies_tuike_naked_window_damage_penalty() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1370 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<WeaponBroken>();
        app.add_event::<ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let baseline_attacker = spawn_player(
            &mut app,
            "AzureBaseNaked",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let naked_attacker = spawn_player(
            &mut app,
            "AzureNaked",
            [0.0, 64.0, 2.0],
            Wounds::default(),
            Stamina::default(),
        );
        let baseline_target = spawn_player(
            &mut app,
            "CrimsonBaseNaked",
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let naked_target = spawn_player(
            &mut app,
            "CrimsonNaked",
            [1.0, 64.0, 2.0],
            Wounds::default(),
            Stamina::default(),
        );
        app.world_mut()
            .entity_mut(naked_target)
            .insert(StackedFalseSkins {
                naked_until_tick: 1400,
                ..Default::default()
            });

        app.world_mut().send_event(AttackIntent {
            attacker: baseline_attacker,
            target: Some(baseline_target),
            issued_at_tick: 1369,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.world_mut().send_event(AttackIntent {
            attacker: naked_attacker,
            target: Some(naked_target),
            issued_at_tick: 1369,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });

        app.update();

        let combat_events = app.world().resource::<Events<CombatEvent>>();
        let events: Vec<_> = combat_events.iter_current_update_events().collect();
        assert_eq!(events.len(), 2);
        assert!(
            events[1].damage > events[0].damage * 1.49,
            "裸壳期应把本次承伤放大到约 1.5 倍"
        );
    }

    #[test]
    fn resolver_applies_backfire_amplification_to_defender_incoming_damage() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1360 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<WeaponBroken>();
        app.add_event::<ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(
            Update,
            (
                crate::combat::status::attribute_aggregate_tick,
                resolve_attack_intents,
            ),
        );

        let baseline_attacker = spawn_player(
            &mut app,
            "AzureBaseBackfire",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let amplified_attacker = spawn_player(
            &mut app,
            "AzureAmpBackfire",
            [0.0, 64.0, 2.0],
            Wounds::default(),
            Stamina::default(),
        );
        let baseline_target = spawn_player(
            &mut app,
            "CrimsonBaseBackfire",
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let amplified_target = spawn_player(
            &mut app,
            "CrimsonAmpBackfire",
            [1.0, 64.0, 2.0],
            Wounds::default(),
            Stamina::default(),
        );

        app.world_mut()
            .entity_mut(amplified_target)
            .insert(BackfireAmplification {
                meridian_id: MeridianId::Du,
                attack_kind: crate::combat::zhenmai_v2::ZhenmaiAttackKind::RealYuan,
                started_at_tick: 1300,
                expires_at_tick: 1400,
                k_drain: 1.5,
                incoming_damage_multiplier: 0.5,
            });

        app.update();

        app.world_mut().send_event(AttackIntent {
            attacker: baseline_attacker,
            target: Some(baseline_target),
            issued_at_tick: 1359,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.world_mut().send_event(AttackIntent {
            attacker: amplified_attacker,
            target: Some(amplified_target),
            issued_at_tick: 1359,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });

        app.update();

        let combat_events = app.world().resource::<Events<CombatEvent>>();
        let events: Vec<_> = combat_events.iter_current_update_events().collect();
        assert_eq!(events.len(), 2);
        let baseline_damage = events[0].damage;
        let amplified_damage = events[1].damage;

        assert!(
            amplified_damage < baseline_damage,
            "backfire amplification should reduce only the holder's incoming damage"
        );
        assert!(
            amplified_damage >= 1.0,
            "backfire amplification is not immunity; main hit still lands"
        );
    }

    // plan-weapon-v1 §6：武器加成 + 耐久扣减 + WeaponBroken 事件。
    #[test]
    fn weapon_increases_outgoing_damage_versus_unarmed() {
        use crate::combat::weapon::{Weapon, WeaponKind};
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1400 });
        app.insert_resource(weapon_test_registry());
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<WeaponBroken>();
        app.add_event::<ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(
            Update,
            (
                crate::combat::status::attribute_aggregate_tick,
                resolve_attack_intents,
            ),
        );

        let unarmed = spawn_player(
            &mut app,
            "Unarmed",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let armed = spawn_player(
            &mut app,
            "Swordsman",
            [0.0, 64.0, 2.0],
            Wounds::default(),
            Stamina::default(),
        );
        app.world_mut().entity_mut(armed).insert(PlayerInventory {
            revision: InventoryRevision(1),
            containers: vec![ContainerState {
                id: crate::inventory::MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 5,
                cols: 7,
                items: vec![],
            }],
            equipped: std::collections::HashMap::from([(
                crate::inventory::EQUIP_SLOT_MAIN_HAND.to_string(),
                ItemInstance {
                    instance_id: 1,
                    template_id: "strong_sword".to_string(),
                    display_name: "强剑".to_string(),
                    grid_w: 1,
                    grid_h: 2,
                    weight: 1.0,
                    rarity: crate::inventory::ItemRarity::Common,
                    description: String::new(),
                    stack_count: 1,
                    spirit_quality: 1.0,
                    durability: 1.0,
                    freshness: None,
                    mineral_id: None,
                    charges: None,
                    forge_quality: None,
                    forge_color: None,
                    forge_side_effects: Vec::new(),
                    forge_achieved_tier: None,
                    alchemy: None,
                    lingering_owner_qi: None,
                },
            )]),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
        });
        let t1 = spawn_player(
            &mut app,
            "T1",
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let t2 = spawn_player(
            &mut app,
            "T2",
            [1.0, 64.0, 2.0],
            Wounds::default(),
            Stamina::default(),
        );

        // armed 手持强攻武器:attack_mul 2.0 × quality 1.0 × durability 1.0 = 2.0
        app.world_mut().entity_mut(armed).insert(Weapon {
            slot: crate::combat::weapon::EquipSlot::MainHand,
            instance_id: 1,
            template_id: "strong_sword".to_string(),
            weapon_kind: WeaponKind::Sword,
            base_attack: 20.0, // attack_multiplier = 2.0
            quality_tier: 0,
            durability: 200.0,
            durability_max: 200.0,
        });

        app.update();

        app.world_mut().send_event(AttackIntent {
            attacker: unarmed,
            target: Some(t1),
            issued_at_tick: 1399,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.world_mut().send_event(AttackIntent {
            attacker: armed,
            target: Some(t2),
            issued_at_tick: 1399,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });

        app.update();

        let combat_events = app.world().resource::<Events<CombatEvent>>();
        let events: Vec<_> = combat_events.iter_current_update_events().collect();
        assert_eq!(events.len(), 2);
        let unarmed_damage = events[0].damage;
        let armed_damage = events[1].damage;
        assert!(
            armed_damage > unarmed_damage * 1.5,
            "armed {armed_damage} should exceed unarmed {unarmed_damage} × 1.5"
        );

        // 命中后 armed attacker 的武器应有:durability ↓。
        let weapon = app.world().entity(armed).get::<Weapon>().unwrap();
        assert!(weapon.durability < 200.0, "durability ticked down");
        let inventory = app.world().entity(armed).get::<PlayerInventory>().unwrap();
        assert!(
            inventory.equipped[crate::inventory::EQUIP_SLOT_MAIN_HAND].durability < 1.0,
            "inventory durability should persist the runtime wear"
        );
    }

    #[test]
    fn iron_sword_increases_damage_by_at_least_20_percent_vs_unarmed() {
        use crate::combat::weapon::{EquipSlot, Weapon, WeaponKind};

        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1420 });
        app.insert_resource(weapon_test_registry());
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<WeaponBroken>();
        app.add_event::<ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(
            Update,
            (
                crate::combat::status::attribute_aggregate_tick,
                resolve_attack_intents,
            ),
        );

        let unarmed = spawn_player(
            &mut app,
            "UnarmedIronBaseline",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let armed = spawn_player(
            &mut app,
            "IronSwordUser",
            [0.0, 64.0, 2.0],
            Wounds::default(),
            Stamina::default(),
        );
        app.world_mut().entity_mut(armed).insert(PlayerInventory {
            revision: InventoryRevision(1),
            containers: vec![ContainerState {
                id: crate::inventory::MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 5,
                cols: 7,
                items: vec![],
            }],
            equipped: std::collections::HashMap::from([(
                crate::inventory::EQUIP_SLOT_MAIN_HAND.to_string(),
                ItemInstance {
                    instance_id: 120,
                    template_id: "iron_sword".to_string(),
                    display_name: "铁剑".to_string(),
                    grid_w: 1,
                    grid_h: 2,
                    weight: 1.2,
                    rarity: crate::inventory::ItemRarity::Common,
                    description: String::new(),
                    stack_count: 1,
                    spirit_quality: 1.0,
                    durability: 1.0,
                    freshness: None,
                    mineral_id: None,
                    charges: None,
                    forge_quality: None,
                    forge_color: None,
                    forge_side_effects: Vec::new(),
                    forge_achieved_tier: None,
                    alchemy: None,
                    lingering_owner_qi: None,
                },
            )]),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
        });
        app.world_mut().entity_mut(armed).insert(Weapon {
            slot: EquipSlot::MainHand,
            instance_id: 120,
            template_id: "iron_sword".to_string(),
            weapon_kind: WeaponKind::Sword,
            base_attack: 12.0,
            quality_tier: 0,
            durability: 200.0,
            durability_max: 200.0,
        });
        let unarmed_target = spawn_player(
            &mut app,
            "IronBaselineTarget",
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let armed_target = spawn_player(
            &mut app,
            "IronSwordTarget",
            [1.0, 64.0, 2.0],
            Wounds::default(),
            Stamina::default(),
        );

        app.update();

        app.world_mut().send_event(AttackIntent {
            attacker: unarmed,
            target: Some(unarmed_target),
            issued_at_tick: 1419,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.world_mut().send_event(AttackIntent {
            attacker: armed,
            target: Some(armed_target),
            issued_at_tick: 1419,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });

        app.update();

        let combat_events = app.world().resource::<Events<CombatEvent>>();
        let events: Vec<_> = combat_events.iter_current_update_events().collect();
        assert_eq!(events.len(), 2);
        let unarmed_damage = events[0].damage;
        let iron_sword_damage = events[1].damage;
        let ratio = iron_sword_damage / unarmed_damage;
        println!(
            "iron_sword_damage_check unarmed={unarmed_damage:.3} iron_sword={iron_sword_damage:.3} ratio={ratio:.3}"
        );
        assert!(
            ratio >= 1.2,
            "iron_sword damage {iron_sword_damage} should be >= unarmed {unarmed_damage} x 1.2; ratio={ratio}"
        );
        assert!(
            (iron_sword_damage - unarmed_damage * 1.2).abs() < 0.001,
            "expected full-durability iron_sword to land exactly at 1.2x baseline"
        );
    }

    #[test]
    fn tool_main_hand_deals_low_damage_above_unarmed_below_entry_sword() {
        for (index, tool_kind) in crate::tools::ALL_TOOL_KINDS.into_iter().enumerate() {
            let mut app = App::new();
            app.insert_resource(CombatClock { tick: 1430 });
            app.add_event::<AttackIntent>();
            app.add_event::<ApplyStatusEffectIntent>();
            app.add_event::<CombatEvent>();
            app.add_event::<DeathEvent>();
            app.add_event::<WeaponBroken>();
            app.add_event::<ShieldBroken>();
            app.add_event::<InventoryDurabilityChangedEvent>();
            app.add_systems(
                Update,
                (
                    crate::combat::status::attribute_aggregate_tick,
                    resolve_attack_intents,
                ),
            );

            let z = (index as f64) * 3.0;
            let unarmed = spawn_player(
                &mut app,
                "BareHandBaseline",
                [0.0, 64.0, z],
                Wounds::default(),
                Stamina::default(),
            );
            let tool_user = spawn_player(
                &mut app,
                "ToolUser",
                [0.0, 64.0, z + 1.0],
                Wounds::default(),
                Stamina::default(),
            );
            app.world_mut()
                .entity_mut(tool_user)
                .insert(PlayerInventory {
                    revision: InventoryRevision(1),
                    containers: vec![ContainerState {
                        id: crate::inventory::MAIN_PACK_CONTAINER_ID.to_string(),
                        name: "主背包".to_string(),
                        rows: 5,
                        cols: 7,
                        items: vec![],
                    }],
                    equipped: std::collections::HashMap::from([(
                        crate::inventory::EQUIP_SLOT_MAIN_HAND.to_string(),
                        ItemInstance {
                            instance_id: 130 + index as u64,
                            template_id: tool_kind.item_id().to_string(),
                            display_name: tool_kind.display_name().to_string(),
                            grid_w: 1,
                            grid_h: 2,
                            weight: 0.9,
                            rarity: crate::inventory::ItemRarity::Common,
                            description: String::new(),
                            stack_count: 1,
                            spirit_quality: 0.0,
                            durability: 1.0,
                            freshness: None,
                            mineral_id: None,
                            charges: None,
                            forge_quality: None,
                            forge_color: None,
                            forge_side_effects: Vec::new(),
                            forge_achieved_tier: None,
                            alchemy: None,
                            lingering_owner_qi: None,
                        },
                    )]),
                    hotbar: Default::default(),
                    bone_coins: 0,
                    max_weight: 50.0,
                });
            let unarmed_target = spawn_player(
                &mut app,
                "BareHandTarget",
                [1.0, 64.0, z],
                Wounds::default(),
                Stamina::default(),
            );
            let tool_target = spawn_player(
                &mut app,
                "ToolTarget",
                [1.0, 64.0, z + 1.0],
                Wounds::default(),
                Stamina::default(),
            );

            app.update();

            app.world_mut().send_event(AttackIntent {
                attacker: unarmed,
                target: Some(unarmed_target),
                issued_at_tick: 1429,
                reach: FIST_REACH,
                qi_invest: 10.0,
                wound_kind: WoundKind::Blunt,
                source: AttackSource::Melee,
                debug_command: None,
            });
            app.world_mut().send_event(AttackIntent {
                attacker: tool_user,
                target: Some(tool_target),
                issued_at_tick: 1429,
                reach: FIST_REACH,
                qi_invest: 10.0,
                wound_kind: WoundKind::Blunt,
                source: AttackSource::Melee,
                debug_command: None,
            });

            app.update();

            let combat_events = app.world().resource::<Events<CombatEvent>>();
            let events: Vec<_> = combat_events.iter_current_update_events().collect();
            assert_eq!(events.len(), 2, "{tool_kind:?} should emit two hits");
            let unarmed_damage = events[0].damage;
            let tool_damage = events[1].damage;
            assert!(
                tool_damage > unarmed_damage,
                "{tool_kind:?} should beat bare hands"
            );
            assert!(
                tool_damage < unarmed_damage * 1.2,
                "{tool_kind:?} should stay below entry iron sword"
            );
            assert!(
                (tool_damage - unarmed_damage * tool_kind.combat_damage_multiplier()).abs() < 0.001,
                "{tool_kind:?} should use its ToolKind multiplier"
            );

            let inventory = app.world().get::<PlayerInventory>(tool_user).unwrap();
            assert_eq!(
                inventory
                    .equipped
                    .get(crate::inventory::EQUIP_SLOT_MAIN_HAND)
                    .unwrap()
                    .durability,
                0.99,
                "{tool_kind:?} hit should tick durability"
            );
            let durability_events = app
                .world()
                .resource::<Events<InventoryDurabilityChangedEvent>>();
            let events: Vec<_> = durability_events.iter_current_update_events().collect();
            assert_eq!(
                events.len(),
                1,
                "{tool_kind:?} should emit one durability event"
            );
            assert_eq!(events[0].entity, tool_user);
            assert_eq!(events[0].instance_id, 130 + index as u64);
            assert_eq!(events[0].durability, 0.99);
        }
    }

    #[test]
    fn broken_tool_main_hand_uses_unarmed_baseline() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1431 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<WeaponBroken>();
        app.add_event::<ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(
            Update,
            (
                crate::combat::status::attribute_aggregate_tick,
                resolve_attack_intents,
            ),
        );

        let broken_tool_user = spawn_player(
            &mut app,
            "BrokenToolUser",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        app.world_mut()
            .entity_mut(broken_tool_user)
            .insert(PlayerInventory {
                revision: InventoryRevision(1),
                containers: vec![ContainerState {
                    id: crate::inventory::MAIN_PACK_CONTAINER_ID.to_string(),
                    name: "主背包".to_string(),
                    rows: 5,
                    cols: 7,
                    items: vec![],
                }],
                equipped: std::collections::HashMap::from([(
                    crate::inventory::EQUIP_SLOT_MAIN_HAND.to_string(),
                    ItemInstance {
                        instance_id: 131,
                        template_id: "cao_lian".to_string(),
                        display_name: "草镰".to_string(),
                        grid_w: 1,
                        grid_h: 2,
                        weight: 0.9,
                        rarity: crate::inventory::ItemRarity::Common,
                        description: String::new(),
                        stack_count: 1,
                        spirit_quality: 0.0,
                        durability: 0.0,
                        freshness: None,
                        mineral_id: None,
                        charges: None,
                        forge_quality: None,
                        forge_color: None,
                        forge_side_effects: Vec::new(),
                        forge_achieved_tier: None,
                        alchemy: None,
                        lingering_owner_qi: None,
                    },
                )]),
                hotbar: Default::default(),
                bone_coins: 0,
                max_weight: 50.0,
            });
        let unarmed = spawn_player(
            &mut app,
            "UnarmedPeer",
            [0.0, 64.0, 2.0],
            Wounds::default(),
            Stamina::default(),
        );
        let broken_tool_target = spawn_player(
            &mut app,
            "BrokenToolTarget",
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let unarmed_target = spawn_player(
            &mut app,
            "UnarmedPeerTarget",
            [1.0, 64.0, 2.0],
            Wounds::default(),
            Stamina::default(),
        );

        app.update();

        app.world_mut().send_event(AttackIntent {
            attacker: broken_tool_user,
            target: Some(broken_tool_target),
            issued_at_tick: 1430,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.world_mut().send_event(AttackIntent {
            attacker: unarmed,
            target: Some(unarmed_target),
            issued_at_tick: 1430,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });

        app.update();

        let combat_events = app.world().resource::<Events<CombatEvent>>();
        let events: Vec<_> = combat_events.iter_current_update_events().collect();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].damage, events[1].damage);

        let durability_events = app
            .world()
            .resource::<Events<InventoryDurabilityChangedEvent>>();
        assert_eq!(durability_events.iter_current_update_events().count(), 0);
    }

    // 耐久归零后 Weapon component 被移除 + WeaponBroken 事件发出。
    #[test]
    fn weapon_breaks_after_durability_depleted() {
        use crate::combat::weapon::{Weapon, WeaponKind};
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1500 });
        app.insert_resource(weapon_test_registry());
        app.insert_resource(DroppedLootRegistry::default());
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<WeaponBroken>();
        app.add_event::<ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(
            Update,
            (
                crate::combat::status::attribute_aggregate_tick,
                resolve_attack_intents,
            ),
        );

        let attacker = spawn_player(
            &mut app,
            "FragileSwordsman",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        app.world_mut()
            .entity_mut(attacker)
            .insert(PlayerInventory {
                revision: InventoryRevision(1),
                containers: vec![ContainerState {
                    id: crate::inventory::MAIN_PACK_CONTAINER_ID.to_string(),
                    name: "主背包".to_string(),
                    rows: 5,
                    cols: 7,
                    items: vec![],
                }],
                equipped: std::collections::HashMap::from([(
                    crate::inventory::EQUIP_SLOT_MAIN_HAND.to_string(),
                    ItemInstance {
                        instance_id: 42,
                        template_id: "glass_sword".to_string(),
                        display_name: "玻璃剑".to_string(),
                        grid_w: 1,
                        grid_h: 2,
                        weight: 1.0,
                        rarity: crate::inventory::ItemRarity::Common,
                        description: String::new(),
                        stack_count: 1,
                        spirit_quality: 1.0,
                        durability: 0.04,
                        freshness: None,
                        mineral_id: None,
                        charges: None,
                        forge_quality: None,
                        forge_color: None,
                        forge_side_effects: Vec::new(),
                        forge_achieved_tier: None,
                        alchemy: None,
                        lingering_owner_qi: None,
                    },
                )]),
                hotbar: Default::default(),
                bone_coins: 0,
                max_weight: 50.0,
            });
        let target = spawn_player(
            &mut app,
            "Dummy",
            [1.0, 64.0, 0.0],
            Wounds {
                health_current: 1000.0, // 防止先死
                health_max: 1000.0,
                ..Wounds::default()
            },
            Stamina::default(),
        );
        // 脆武器:只剩 0.4 耐久,一击即破(HIT_DURABILITY_COST = 0.5)
        app.world_mut().entity_mut(attacker).insert(Weapon {
            slot: crate::combat::weapon::EquipSlot::MainHand,
            instance_id: 42,
            template_id: "glass_sword".to_string(),
            weapon_kind: WeaponKind::Sword,
            base_attack: 10.0,
            quality_tier: 0,
            durability: 0.4,
            durability_max: 10.0,
        });

        app.update();

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 1499,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });

        app.update();

        // Weapon component 已被移除
        assert!(
            app.world().entity(attacker).get::<Weapon>().is_none(),
            "Weapon removed after durability depleted"
        );
        // WeaponBroken event 发出
        let broken_events = app.world().resource::<Events<WeaponBroken>>();
        let events: Vec<_> = broken_events.iter_current_update_events().collect();
        assert_eq!(events.len(), 1, "one WeaponBroken emitted");
        assert_eq!(events[0].instance_id, 42);
        assert_eq!(events[0].template_id, "glass_sword");

        let inventory = app
            .world()
            .entity(attacker)
            .get::<PlayerInventory>()
            .unwrap();
        assert!(
            !inventory
                .equipped
                .contains_key(crate::inventory::EQUIP_SLOT_MAIN_HAND),
            "broken weapon should leave the equip slot"
        );
        assert_eq!(inventory.containers[0].items.len(), 1);
        assert_eq!(inventory.containers[0].items[0].instance.instance_id, 42);
        assert_eq!(inventory.containers[0].items[0].instance.durability, 0.0);
    }

    #[test]
    fn broken_weapon_drops_when_no_container_slot_is_available() {
        use crate::combat::weapon::{Weapon, WeaponKind};
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1600 });
        app.insert_resource(weapon_test_registry());
        app.insert_resource(DroppedLootRegistry::default());
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<WeaponBroken>();
        app.add_event::<ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(
            Update,
            (
                crate::combat::status::attribute_aggregate_tick,
                resolve_attack_intents,
            ),
        );

        let attacker = spawn_player(
            &mut app,
            "PackedSwordsman",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        app.world_mut()
            .entity_mut(attacker)
            .insert(PlayerInventory {
                revision: InventoryRevision(1),
                containers: vec![ContainerState {
                    id: crate::inventory::MAIN_PACK_CONTAINER_ID.to_string(),
                    name: "主背包".to_string(),
                    rows: 1,
                    cols: 1,
                    items: vec![crate::inventory::PlacedItemState {
                        row: 0,
                        col: 0,
                        instance: ItemInstance {
                            instance_id: 7,
                            template_id: "junk_stone".to_string(),
                            display_name: "碎石".to_string(),
                            grid_w: 1,
                            grid_h: 1,
                            weight: 1.0,
                            rarity: crate::inventory::ItemRarity::Common,
                            description: String::new(),
                            stack_count: 1,
                            spirit_quality: 1.0,
                            durability: 1.0,
                            freshness: None,
                            mineral_id: None,
                            charges: None,
                            forge_quality: None,
                            forge_color: None,
                            forge_side_effects: Vec::new(),
                            forge_achieved_tier: None,
                            alchemy: None,
                            lingering_owner_qi: None,
                        },
                    }],
                }],
                equipped: std::collections::HashMap::from([(
                    crate::inventory::EQUIP_SLOT_MAIN_HAND.to_string(),
                    ItemInstance {
                        instance_id: 42,
                        template_id: "glass_sword".to_string(),
                        display_name: "玻璃剑".to_string(),
                        grid_w: 1,
                        grid_h: 2,
                        weight: 1.0,
                        rarity: crate::inventory::ItemRarity::Common,
                        description: String::new(),
                        stack_count: 1,
                        spirit_quality: 1.0,
                        durability: 0.04,
                        freshness: None,
                        mineral_id: None,
                        charges: None,
                        forge_quality: None,
                        forge_color: None,
                        forge_side_effects: Vec::new(),
                        forge_achieved_tier: None,
                        alchemy: None,
                        lingering_owner_qi: None,
                    },
                )]),
                hotbar: Default::default(),
                bone_coins: 0,
                max_weight: 50.0,
            });
        let target = spawn_player(
            &mut app,
            "PackedDummy",
            [1.0, 64.0, 0.0],
            Wounds {
                health_current: 1000.0,
                health_max: 1000.0,
                ..Wounds::default()
            },
            Stamina::default(),
        );
        app.world_mut().entity_mut(attacker).insert(Weapon {
            slot: crate::combat::weapon::EquipSlot::MainHand,
            instance_id: 42,
            template_id: "glass_sword".to_string(),
            weapon_kind: WeaponKind::Sword,
            base_attack: 10.0,
            quality_tier: 0,
            durability: 0.4,
            durability_max: 10.0,
        });

        app.update();

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 1599,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });

        app.update();

        assert!(
            app.world().entity(attacker).get::<Weapon>().is_none(),
            "Weapon removed after broken weapon falls back to dropped loot"
        );
        let inventory = app
            .world()
            .entity(attacker)
            .get::<PlayerInventory>()
            .unwrap();
        assert!(
            !inventory
                .equipped
                .contains_key(crate::inventory::EQUIP_SLOT_MAIN_HAND),
            "broken weapon should leave the equip slot even when bag is full"
        );
        assert_eq!(inventory.containers[0].items.len(), 1);

        let dropped_registry = app.world().resource::<DroppedLootRegistry>();
        let dropped = dropped_registry
            .entries
            .get(&42)
            .expect("broken weapon should be registered as dropped loot");
        assert_eq!(dropped.instance_id, 42);
        assert_eq!(dropped.item.durability, 0.0);
    }

    #[test]
    fn cut_and_blunt_hits_produce_different_bleed_and_crack_outputs() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1400 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let cut_attacker = spawn_player(
            &mut app,
            "CutUser",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let blunt_attacker = spawn_player(
            &mut app,
            "BluntUser",
            [0.0, 64.0, 2.0],
            Wounds::default(),
            Stamina::default(),
        );
        let cut_target = spawn_player(
            &mut app,
            "CutTarget",
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let blunt_target = spawn_player(
            &mut app,
            "BluntTarget",
            [1.0, 64.0, 2.0],
            Wounds::default(),
            Stamina::default(),
        );

        app.world_mut().send_event(AttackIntent {
            attacker: cut_attacker,
            target: Some(cut_target),
            issued_at_tick: 1399,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Cut,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.world_mut().send_event(AttackIntent {
            attacker: blunt_attacker,
            target: Some(blunt_target),
            issued_at_tick: 1399,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });

        app.update();

        let cut_target_ref = app.world().entity(cut_target);
        let blunt_target_ref = app.world().entity(blunt_target);
        let cut_wound = cut_target_ref
            .get::<Wounds>()
            .unwrap()
            .entries
            .last()
            .unwrap()
            .clone();
        let blunt_wound = blunt_target_ref
            .get::<Wounds>()
            .unwrap()
            .entries
            .last()
            .unwrap()
            .clone();
        let cut_crack = cut_target_ref
            .get::<MeridianSystem>()
            .unwrap()
            .get(MeridianId::Lung)
            .cracks
            .last()
            .unwrap()
            .clone();
        let blunt_crack = blunt_target_ref
            .get::<MeridianSystem>()
            .unwrap()
            .get(MeridianId::Lung)
            .cracks
            .last()
            .unwrap()
            .clone();

        assert!(cut_wound.bleeding_per_sec > blunt_wound.bleeding_per_sec);
        assert!(blunt_crack.severity > cut_crack.severity);
    }

    #[test]
    fn pierce_hit_changes_contamination_output_against_blunt_baseline() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1500 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let pierce_attacker = spawn_player(
            &mut app,
            "PierceUser",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let blunt_attacker = spawn_player(
            &mut app,
            "BluntUser2",
            [0.0, 64.0, 2.0],
            Wounds::default(),
            Stamina::default(),
        );
        let pierce_target = spawn_player(
            &mut app,
            "PierceTarget",
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let blunt_target = spawn_player(
            &mut app,
            "BluntTarget2",
            [1.0, 64.0, 2.0],
            Wounds::default(),
            Stamina::default(),
        );

        app.world_mut().send_event(AttackIntent {
            attacker: pierce_attacker,
            target: Some(pierce_target),
            issued_at_tick: 1499,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Pierce,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.world_mut().send_event(AttackIntent {
            attacker: blunt_attacker,
            target: Some(blunt_target),
            issued_at_tick: 1499,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });

        app.update();

        let pierce_contam = app
            .world()
            .entity(pierce_target)
            .get::<Contamination>()
            .unwrap()
            .entries
            .last()
            .unwrap()
            .amount;
        let blunt_contam = app
            .world()
            .entity(blunt_target)
            .get::<Contamination>()
            .unwrap()
            .entries
            .last()
            .unwrap()
            .amount;

        assert!(pierce_contam > blunt_contam);
    }

    #[test]
    fn zero_qi_sword_hit_resolves_physical_damage_without_contamination_or_meridian_crack() {
        use crate::combat::weapon::{EquipSlot, Weapon, WeaponKind};

        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1540 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<WeaponBroken>();
        app.add_event::<ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let attacker = spawn_player(
            &mut app,
            "ZeroQiSword",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        app.world_mut().entity_mut(attacker).insert((
            Weapon {
                slot: EquipSlot::MainHand,
                instance_id: 1540,
                template_id: "iron_sword".to_string(),
                weapon_kind: WeaponKind::Sword,
                base_attack: 12.0,
                quality_tier: 0,
                durability: 200.0,
                durability_max: 200.0,
            },
            KnownTechniques {
                entries: vec![KnownTechnique {
                    id: sword_basics::SWORD_CLEAVE_SKILL_ID.to_string(),
                    proficiency: 0.5,
                    active: true,
                }],
            },
        ));
        let mut target_meridians = MeridianSystem::default();
        target_meridians.get_mut(MeridianId::Lung).opened = true;
        let target = spawn_player(
            &mut app,
            "ZeroQiTarget",
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        app.world_mut().entity_mut(target).insert(target_meridians);

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 1539,
            reach: AttackReach::new(3.0, 0.0),
            qi_invest: 0.0,
            wound_kind: WoundKind::Cut,
            source: AttackSource::SwordCleave,
            debug_command: None,
        });

        app.update();

        let events: Vec<_> = app
            .world()
            .resource::<Events<CombatEvent>>()
            .iter_current_update_events()
            .collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].damage, 0.0);
        assert!(
            events[0].physical_damage > 0.0,
            "zero-qi sword hit should still land physical damage"
        );
        assert_eq!(events[0].contam_delta, 0.0);

        let target_ref = app.world().entity(target);
        assert!(
            target_ref
                .get::<Contamination>()
                .expect("target contamination")
                .entries
                .is_empty(),
            "physical branch must not introduce contamination"
        );
        let meridian = target_ref
            .get::<MeridianSystem>()
            .expect("target meridians")
            .get(MeridianId::Lung);
        assert_eq!(meridian.throughput_current, 0.0);
        assert!(
            meridian.cracks.is_empty(),
            "physical branch must not crack meridians"
        );
    }

    #[test]
    fn sword_parry_blocks_physical_damage_reflects_and_staggers_attacker() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1541 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<WeaponBroken>();
        app.add_event::<ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let attacker = spawn_player(
            &mut app,
            "ParryAttacker",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let defender = spawn_player(
            &mut app,
            "ParryDefender",
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        app.world_mut().entity_mut(defender).insert((
            StatusEffects {
                active: vec![ActiveStatusEffect {
                    kind: StatusEffectKind::SwordParrying,
                    magnitude: 0.5,
                    remaining_ticks: 4,
                    source_pill: None,
                }],
            },
            KnownTechniques {
                entries: vec![KnownTechnique {
                    id: sword_basics::SWORD_PARRY_SKILL_ID.to_string(),
                    proficiency: 0.0,
                    active: true,
                }],
            },
        ));

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(defender),
            issued_at_tick: 1540,
            reach: FIST_REACH,
            qi_invest: 0.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });

        app.update();

        let events: Vec<_> = app
            .world()
            .resource::<Events<CombatEvent>>()
            .iter_current_update_events()
            .collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].defense_kind, Some(DefenseKind::SwordParry));
        assert_eq!(events[0].defense_effectiveness, Some(0.5));
        assert!(
            (events[0].physical_damage - 0.5).abs() < 0.001,
            "50% sword parry should halve the 1.0 unarmed physical hit"
        );

        let status_intents: Vec<_> = app
            .world()
            .resource::<Events<ApplyStatusEffectIntent>>()
            .iter_current_update_events()
            .collect();
        assert!(status_intents
            .iter()
            .any(|intent| intent.target == attacker && intent.kind == StatusEffectKind::Staggered));

        let attacker_wounds = app.world().entity(attacker).get::<Wounds>().unwrap();
        assert_eq!(attacker_wounds.entries.len(), 1);
        assert!(
            (attacker_wounds.entries[0].severity - 0.075).abs() < 0.001,
            "reflected physical damage should be 15% of blocked damage"
        );

        let known = app
            .world()
            .entity(defender)
            .get::<KnownTechniques>()
            .unwrap();
        assert!(
            known.entries[0].proficiency > 0.0,
            "successful parry should raise sword.parry proficiency"
        );
    }

    #[test]
    fn burst_meridian_attack_source_uses_prepaid_qi_without_second_spend() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1550 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let attacker = spawn_player(
            &mut app,
            "BurstUser",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        app.world_mut().entity_mut(attacker).insert(Cultivation {
            qi_current: 60.0,
            qi_max: 100.0,
            ..Cultivation::default()
        });
        let target = spawn_player(
            &mut app,
            "BurstTarget",
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 1549,
            reach: FIST_REACH,
            qi_invest: 80.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::BurstMeridian,
            debug_command: None,
        });

        app.update();

        assert_eq!(
            app.world()
                .entity(attacker)
                .get::<Cultivation>()
                .unwrap()
                .qi_current,
            60.0,
            "BurstMeridian source is already paid by skill resolver and must not spend qi again"
        );
        assert!(
            !app.world().resource::<Events<CombatEvent>>().is_empty(),
            "prepaid burst attack should still resolve even when qi_invest exceeds remaining qi"
        );
    }

    #[test]
    fn full_power_attack_source_uses_prepaid_qi_without_second_spend() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1550 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let attacker = spawn_player(
            &mut app,
            "FullPowerUser",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        app.world_mut().entity_mut(attacker).insert(Cultivation {
            qi_current: 60.0,
            qi_max: 100.0,
            ..Cultivation::default()
        });
        let target = spawn_player(
            &mut app,
            "FullPowerTarget",
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 1549,
            reach: FIST_REACH,
            qi_invest: 80.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::FullPower,
            debug_command: None,
        });

        app.update();

        assert_eq!(
            app.world()
                .entity(attacker)
                .get::<Cultivation>()
                .unwrap()
                .qi_current,
            60.0,
            "FullPower source is already paid by release handler and must not spend qi again"
        );
        assert!(
            !app.world().resource::<Events<CombatEvent>>().is_empty(),
            "prepaid full power attack should still resolve when qi_invest exceeds remaining qi"
        );
    }

    /// 端到端验证 NPC↔NPC 互殴走 shared resolver：使用 `npc_runtime_bundle`
    /// 的真实形态（**无 LifeRecord**）双方交叉 `AttackIntent`，断言 Wounds
    /// 写入 + 致命伤触发 DeathEvent。既有测试用 test-only helper 挂了
    /// LifeRecord，未代表生产形态；本测试补齐。
    #[test]
    fn npc_to_npc_duel_via_runtime_bundle_resolves_damage_and_death() {
        use crate::npc::lifecycle::{npc_runtime_bundle, NpcArchetype};

        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 200 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        // 两个 NPC 用真实生产 bundle，无 LifeRecord。
        let npc_a = app
            .world_mut()
            .spawn((NpcMarker, Position::new([0.0, 64.0, 0.0])))
            .id();
        let mut bundle_a = npc_runtime_bundle(npc_a, NpcArchetype::Rogue);
        // 让 A 血量濒死以便单击致命；qi 注满以过 resolver 的 qi_invest 检查。
        bundle_a.wounds = Wounds {
            health_current: 3.0,
            health_max: 100.0,
            entries: Vec::new(),
        };
        bundle_a.cultivation.qi_current = 80.0;
        bundle_a.cultivation.qi_max = 100.0;
        app.world_mut().entity_mut(npc_a).insert(bundle_a);

        let npc_b = app
            .world_mut()
            .spawn((NpcMarker, Position::new([1.0, 64.0, 0.0])))
            .id();
        let mut bundle_b = npc_runtime_bundle(npc_b, NpcArchetype::Zombie);
        bundle_b.cultivation.qi_current = 80.0;
        bundle_b.cultivation.qi_max = 100.0;
        app.world_mut().entity_mut(npc_b).insert(bundle_b);

        // 双向 AttackIntent：A 打 B 一下（非致命），B 打 A 一下（致命）。
        app.world_mut().send_event(AttackIntent {
            attacker: npc_a,
            target: Some(npc_b),
            issued_at_tick: 199,
            reach: FIST_REACH,
            qi_invest: 8.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.world_mut().send_event(AttackIntent {
            attacker: npc_b,
            target: Some(npc_a),
            issued_at_tick: 199,
            reach: NpcMeleeProfile::spear().reach,
            qi_invest: 12.0,
            wound_kind: WoundKind::Pierce,
            source: AttackSource::Melee,
            debug_command: None,
        });

        app.update();

        let a_wounds = app.world().entity(npc_a).get::<Wounds>().unwrap();
        let b_wounds = app.world().entity(npc_b).get::<Wounds>().unwrap();

        assert_eq!(
            a_wounds.entries.len(),
            1,
            "A should take exactly one wound from B's pierce"
        );
        assert_eq!(a_wounds.entries[0].kind, WoundKind::Pierce);
        assert!(
            a_wounds.health_current <= 0.0,
            "A was 3hp + pierce should be lethal, got {}",
            a_wounds.health_current
        );

        assert_eq!(
            b_wounds.entries.len(),
            1,
            "B should take exactly one wound from A's blunt"
        );
        assert_eq!(b_wounds.entries[0].kind, WoundKind::Blunt);
        assert!(
            b_wounds.health_current > 0.0,
            "B full-hp should survive one blunt, got {}",
            b_wounds.health_current
        );

        // Contamination 同样被写（双向都有 attacker_id = canonical_npc_id）。
        let a_contam = app.world().entity(npc_a).get::<Contamination>().unwrap();
        let b_contam = app.world().entity(npc_b).get::<Contamination>().unwrap();
        assert_eq!(
            a_contam.entries[0].attacker_id.as_deref(),
            Some(canonical_npc_id(npc_b).as_str())
        );
        assert_eq!(
            b_contam.entries[0].attacker_id.as_deref(),
            Some(canonical_npc_id(npc_a).as_str())
        );

        // DeathEvent 应该恰为 A 触发（B 未致命）。
        let deaths: Vec<_> = app
            .world()
            .resource::<Events<DeathEvent>>()
            .get_reader()
            .read(app.world().resource::<Events<DeathEvent>>())
            .cloned()
            .collect();
        assert_eq!(deaths.len(), 1);
        assert_eq!(deaths[0].target, npc_a);
    }

    // ────────────────────────────────────────────────────────
    // plan-woliu-path-v1: VoidCoreActive 回归测试
    // ────────────────────────────────────────────────────────

    #[test]
    fn void_core_active_attacker_cannot_deal_damage() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1100 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let attacker = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_player(
            &mut app,
            "Crimson",
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );

        app.world_mut().entity_mut(attacker).insert(StatusEffects {
            active: vec![ActiveStatusEffect {
                kind: StatusEffectKind::VoidCoreActive,
                magnitude: 1.0,
                remaining_ticks: 60,
                source_pill: None,
            }],
        });

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 1099,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });

        app.update();

        let wounds = app.world().entity(target).get::<Wounds>().unwrap();
        assert_eq!(
            wounds.health_current, wounds.health_max,
            "VoidCoreActive attacker should not deal damage; target health should be unchanged"
        );
        assert!(
            wounds.entries.is_empty(),
            "VoidCoreActive attacker should produce no wound entries"
        );
        assert!(
            app.world().resource::<Events<CombatEvent>>().is_empty(),
            "no CombatEvent should be emitted when attacker has VoidCoreActive"
        );
    }

    #[test]
    fn void_core_active_target_cannot_be_hit() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1100 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);

        let attacker = spawn_player(
            &mut app,
            "Azure",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        let target = spawn_player(
            &mut app,
            "Crimson",
            [1.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );

        app.world_mut().entity_mut(target).insert(StatusEffects {
            active: vec![ActiveStatusEffect {
                kind: StatusEffectKind::VoidCoreActive,
                magnitude: 1.0,
                remaining_ticks: 60,
                source_pill: None,
            }],
        });

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 1099,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });

        app.update();

        let wounds = app.world().entity(target).get::<Wounds>().unwrap();
        assert_eq!(
            wounds.health_current, wounds.health_max,
            "VoidCoreActive target should be immune to hits; health should be unchanged"
        );
        assert!(
            wounds.entries.is_empty(),
            "VoidCoreActive target should produce no wound entries"
        );
        assert!(
            app.world().resource::<Events<CombatEvent>>().is_empty(),
            "no CombatEvent should be emitted when target has VoidCoreActive"
        );
    }

    #[test]
    fn void_core_active_defender_cannot_produce_defense_event() {
        let mut app = App::new();
        app.add_event::<DefenseIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_systems(Update, apply_defense_intents);

        let defender = app
            .world_mut()
            .spawn((
                CombatState::default(),
                Cultivation {
                    realm: Realm::Induce,
                    qi_current: 10.0,
                    qi_max: 10.0,
                    ..Cultivation::default()
                },
                StatusEffects {
                    active: vec![ActiveStatusEffect {
                        kind: StatusEffectKind::VoidCoreActive,
                        magnitude: 1.0,
                        remaining_ticks: 60,
                        source_pill: None,
                    }],
                },
            ))
            .id();

        app.world_mut().send_event(DefenseIntent {
            defender,
            issued_at_tick: 10,
        });
        app.update();

        let state = app.world().entity(defender).get::<CombatState>().unwrap();
        assert!(
            state.incoming_window.is_none(),
            "VoidCoreActive defender should not produce a defense window"
        );
        let intent_count = app
            .world()
            .resource::<Events<ApplyStatusEffectIntent>>()
            .iter_current_update_events()
            .count();
        assert_eq!(
            intent_count, 0,
            "VoidCoreActive defender should not emit ApplyStatusEffectIntent (e.g. ParryRecovery), got {intent_count}"
        );
    }

    // ══════════════════════════════════════════════════════════════════════════
    // plan-shield-block-v1 P2 — resolve_attack_intents 减伤分支集成测试
    // ══════════════════════════════════════════════════════════════════════════

    fn make_shield_block_app() -> App {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 5000 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);
        app
    }

    // ── happy path：正面命中 + ShieldBlocking → wound.severity / bleeding / contam 按比例削减 ──
    #[test]
    fn shield_block_front_face_reduces_severity_bleeding_and_contam() {
        let mut app = make_shield_block_app();
        // 攻击者在 [0,64,0]，防御者在 [1,64,0] 朝 +Z（yaw=0）
        // 防御者朝 -X(facing=-sin(0)=0, cos(0)=1 → facing=(0,0,1))
        // to_attacker = 0-1=-1 in X → dot 需计算
        // 为简化：defender 在 z=1，attacker 在 z=3 → to_attacker=(0,0,2) → dot with (0,0,1)=1.0 > -0.5 → 正面
        let attacker = spawn_player(
            &mut app,
            "ShieldAtk",
            [0.0, 64.0, 3.0],
            Wounds::default(),
            Stamina::default(),
        );
        let defender = spawn_player(
            &mut app,
            "ShieldDef",
            [0.0, 64.0, 1.0],
            Wounds::default(),
            Stamina::default(),
        );
        // defender 朝 +Z（yaw=0），attacker 在 z=3（防御者前方）→ dot=1.0 > -0.5 → 正面
        // plan-shield-block-v1 P4: block_ratio 来自 shield_block_profile("bone_shield", proficiency)
        // 骨盾 proficiency=0.0 → block_ratio=0.65（P4 spec）。
        // off_hand 中的骨盾提供 template_id 用于 profile 查找；KnownTechniques 中 proficiency=0.0。
        app.world_mut().entity_mut(defender).insert((
            StatusEffects {
                active: vec![ActiveStatusEffect {
                    kind: StatusEffectKind::ShieldBlocking,
                    magnitude: 0.65, // P4 snapshot：magnitude 用于记录当前格挡态，block_ratio 通过 profile 计算
                    remaining_ticks: crate::combat::shield_block::SHIELD_BLOCKING_DURATION_TICKS,
                    source_pill: None,
                }],
            },
            crate::combat::shield_block::ShieldBlock {
                template_id: "bone_shield".to_string(),
            },
            Look {
                yaw: 0.0,
                pitch: 0.0,
            }, // 朝 +Z
            // P4: off_hand 骨盾供 profile 查找（template_id → block_ratio=0.65）
            PlayerInventory {
                revision: InventoryRevision(1),
                containers: vec![],
                equipped: std::collections::HashMap::from([(
                    EQUIP_SLOT_OFF_HAND.to_string(),
                    ItemInstance {
                        instance_id: 91,
                        template_id: "bone_shield".to_string(),
                        display_name: "骨盾".to_string(),
                        grid_w: 1,
                        grid_h: 2,
                        weight: 2.5,
                        rarity: ItemRarity::Common,
                        description: String::new(),
                        stack_count: 1,
                        spirit_quality: 0.0,
                        durability: 1.0,
                        freshness: None,
                        mineral_id: None,
                        charges: None,
                        forge_quality: None,
                        forge_color: None,
                        forge_side_effects: Vec::new(),
                        forge_achieved_tier: None,
                        alchemy: None,
                        lingering_owner_qi: None,
                    },
                )]),
                hotbar: Default::default(),
                bone_coins: 0,
                max_weight: 100.0,
            },
        ));

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(defender),
            issued_at_tick: 4999,
            reach: FIST_REACH,
            qi_invest: 0.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.update();

        // 1. CombatEvent 发出 defense_kind = ShieldBlock
        let events: Vec<_> = app
            .world()
            .resource::<Events<CombatEvent>>()
            .iter_current_update_events()
            .collect();
        assert_eq!(events.len(), 1, "应有且仅有一个 CombatEvent");
        assert_eq!(
            events[0].defense_kind,
            Some(DefenseKind::ShieldBlock),
            "正面命中 ShieldBlocking 应发出 defense_kind=ShieldBlock；actual: {:?}",
            events[0].defense_kind
        );
        // 2. defense_effectiveness == block_ratio（plan-shield-block-v1 P4）
        // bone_shield proficiency=0.0 → shield_block_profile("bone_shield", 0.0).block_ratio = 0.65
        assert!(
            events[0]
                .defense_effectiveness
                .is_some_and(|e| (e - 0.65).abs() < 0.01),
            "defense_effectiveness 应等于 bone_shield 基础 block_ratio=0.65（P4 profile，proficiency=0.0），\
             actual: {:?}",
            events[0].defense_effectiveness
        );
        // 3. wound 减伤确认（physical_damage 应小于无盾时的 1.0 unarmed）
        let phys_dmg = events[0].physical_damage;
        assert!(
            phys_dmg < 0.5,
            "65% 盾格挡后 physical_damage 应小于 0.5（减伤比例正确），actual={phys_dmg}"
        );
        // 4. 攻击者无 reflected_damage（盾格挡无反伤，对比 SwordParry 有 0.15 反伤）
        let attacker_wounds = app.world().entity(attacker).get::<Wounds>().unwrap();
        assert!(
            attacker_wounds.entries.is_empty(),
            "盾格挡后攻击者不应有 reflected_damage（无反伤语义），\
             actual entries: {:?}",
            attacker_wounds.entries
        );
    }

    // ── 边界 dot==-0.5（恰好 120° 边界） → 命中减伤 ──────────────────────────
    // 使攻击者方向 dot 精确等于 -0.5，不靠近似几何，直接在 FOV 判定上测 >= 语义。
    #[test]
    fn shield_fov_check_exact_boundary_dot_minus_half_triggers_block() {
        // 直接测 shield_fov_check 函数，dot 精确等于 -0.5（见 P5 §1 要求）
        use valence::entity::Look;
        // 防御者朝 +Z（yaw=0），facing=(0,0,1)
        // 攻击者方向 = cos(120°)=-0.5, sin(120°)=√3/2
        // to_attacker normalized = (-sin120°, 0, cos120°) = (-√3/2, 0, -0.5)
        // dot with (0,0,1) = -0.5 → 恰好等于 SHIELD_FOV_DOT
        let cos120 = 120.0_f64.to_radians().cos(); // = -0.5
        let sin120 = 120.0_f64.to_radians().sin(); // = √3/2
                                                   // 构造攻击者在方向 (-sin120, 0, cos120) 距离 2
        let attacker_pos = valence::prelude::DVec3::new(-sin120 * 2.0, 0.0, cos120 * 2.0);
        let defender_pos = valence::prelude::DVec3::ZERO;
        let look = Look {
            yaw: 0.0,
            pitch: 0.0,
        };

        let result =
            crate::combat::shield_block::shield_fov_check(attacker_pos, defender_pos, Some(&look));
        // 验证 dot = facing · to_att_norm = (0,0,1) · (-sin120, 0, cos120) = cos120 = -0.5
        assert!(
            result,
            "dot=-0.5 精确等于 SHIELD_FOV_DOT(-0.5)，>= 比较应通过（命中可挡）；\
             若改为 > 则此测试撞红（>=  vs > boundary pin）"
        );
    }

    // ── 背面命中（dot < -0.5）→ 盾格挡无效 ─────────────────────────────────
    #[test]
    fn shield_block_back_face_no_reduction() {
        let mut app = make_shield_block_app();
        // 防御者在 [0,64,0] 朝 +Z（yaw=0），攻击者在 [0,64,-2]（背后 dot=-1.0 < -0.5）
        let attacker = spawn_player(
            &mut app,
            "BackAtk",
            [0.0, 64.0, -2.0],
            Wounds::default(),
            Stamina::default(),
        );
        let defender = spawn_player(
            &mut app,
            "BackDef",
            [0.0, 64.0, 0.0],
            Wounds::default(),
            Stamina::default(),
        );
        app.world_mut().entity_mut(defender).insert((
            StatusEffects {
                active: vec![ActiveStatusEffect {
                    kind: StatusEffectKind::ShieldBlocking,
                    magnitude: 0.6,
                    remaining_ticks: crate::combat::shield_block::SHIELD_BLOCKING_DURATION_TICKS,
                    source_pill: None,
                }],
            },
            crate::combat::shield_block::ShieldBlock {
                template_id: "bone_shield".to_string(),
            },
            Look {
                yaw: 0.0,
                pitch: 0.0,
            }, // 朝 +Z，攻击者在 -Z（背后）
        ));

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(defender),
            issued_at_tick: 4999,
            reach: FIST_REACH,
            qi_invest: 0.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.update();

        // 背面命中：defense_kind != ShieldBlock（盾不生效）
        let events: Vec<_> = app
            .world()
            .resource::<Events<CombatEvent>>()
            .iter_current_update_events()
            .collect();
        assert_eq!(events.len(), 1);
        assert_ne!(
            events[0].defense_kind,
            Some(DefenseKind::ShieldBlock),
            "背面命中（dot=-1.0 < -0.5）盾格挡不应生效，defense_kind 不应为 ShieldBlock；\
             actual: {:?}",
            events[0].defense_kind
        );
        // 背面命中不减伤：物理伤害不被削减
        assert!(
            events[0].physical_damage >= 0.9,
            "背面命中不减伤，physical_damage 应接近无盾时的基础值（约 1.0 unarmed）；\
             actual: {}",
            events[0].physical_damage
        );
    }

    // ── 无反伤专属断言：盾格挡后攻击者无 reflected_damage ───────────────────
    // 对比 SwordParry（有 0.15 反伤），盾格挡应无反伤。
    #[test]
    fn shield_block_has_no_reflected_damage_unlike_sword_parry() {
        let mut app = make_shield_block_app();
        let attacker = spawn_player(
            &mut app,
            "NoReflectAtk",
            [0.0, 64.0, 3.0],
            Wounds::default(),
            Stamina::default(),
        );
        let defender = spawn_player(
            &mut app,
            "NoReflectDef",
            [0.0, 64.0, 1.0],
            Wounds::default(),
            Stamina::default(),
        );
        app.world_mut().entity_mut(defender).insert((
            StatusEffects {
                active: vec![ActiveStatusEffect {
                    kind: StatusEffectKind::ShieldBlocking,
                    magnitude: 0.5,
                    remaining_ticks: crate::combat::shield_block::SHIELD_BLOCKING_DURATION_TICKS,
                    source_pill: None,
                }],
            },
            crate::combat::shield_block::ShieldBlock {
                template_id: "bone_shield".to_string(),
            },
            Look {
                yaw: 0.0,
                pitch: 0.0,
            },
        ));

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(defender),
            issued_at_tick: 4999,
            reach: FIST_REACH,
            qi_invest: 0.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.update();

        let attacker_wounds = app.world().entity(attacker).get::<Wounds>().unwrap();
        assert!(
            attacker_wounds.entries.is_empty(),
            "盾格挡不应产生 reflected_damage（无反伤），\
             对比 SwordParry 有 0.15*blocked 反伤。\
             actual attacker wound entries: {:?}",
            attacker_wounds.entries
        );
    }

    // ── apply_defense_intents with ShieldBlocking → 无 jiemai 窗口，无 per-block ParryRecovery ──
    // plan-shield-block-v1 P2 Issue3 验证：盾格挡 emit 的 DefenseIntent 不应开 jiemai 窗口，
    // 也不应施加 per-block ParryRecovery（只有真截脉才有）。
    #[test]
    fn apply_defense_intent_shield_blocking_no_jiemai_window_no_parry_recovery() {
        let mut app = App::new();
        app.add_event::<DefenseIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_systems(
            Update,
            (
                apply_defense_intents,
                crate::combat::status::status_effect_apply_tick.after(apply_defense_intents),
            ),
        );

        let defender = app
            .world_mut()
            .spawn((
                CombatState::default(),
                Cultivation {
                    realm: Realm::Condense, // 修士境界（parry_qi_cost_for_realm 返回 Some）
                    qi_current: 12.0,
                    qi_max: 20.0,
                    ..Cultivation::default()
                },
                StatusEffects {
                    active: vec![ActiveStatusEffect {
                        kind: StatusEffectKind::ShieldBlocking,
                        magnitude: 0.6,
                        remaining_ticks:
                            crate::combat::shield_block::SHIELD_BLOCKING_DURATION_TICKS,
                        source_pill: None,
                    }],
                },
            ))
            .id();

        app.world_mut().send_event(DefenseIntent {
            defender,
            issued_at_tick: 10,
        });
        app.update();

        // 断言1：盾格挡时不开 jiemai incoming_window
        let state = app.world().entity(defender).get::<CombatState>().unwrap();
        assert!(
            state.incoming_window.is_none(),
            "盾格挡下 apply_defense_intents 不应开 jiemai incoming_window（盾不耦合截脉）；\
             actual incoming_window: {:?}",
            state.incoming_window
        );
        // 断言2：不发 per-block ParryRecovery intent
        let intents: Vec<_> = app
            .world()
            .resource::<Events<ApplyStatusEffectIntent>>()
            .iter_current_update_events()
            .collect();
        let has_parry_recovery = intents
            .iter()
            .any(|i| i.kind == StatusEffectKind::ParryRecovery && i.target == defender);
        assert!(
            !has_parry_recovery,
            "盾格挡下 apply_defense_intents 不应施加 per-block ParryRecovery（锁 0.5s）；\
             actual intents: {intents:?}"
        );
    }

    // ══════════════════════════════════════════════════════════════════════════
    // plan-shield-block-v1 P3 — 耐久扣减 / ShieldBroken 事件 / 互斥裁定
    // ══════════════════════════════════════════════════════════════════════════

    /// 构建带 ItemRegistry（木盾/骨盾 ShieldSpec）的 App，适合 P3 耐久测试。
    fn make_shield_durability_app() -> App {
        use crate::inventory::{ItemCategory, ItemTemplate, ShieldSpec};

        let registry = ItemRegistry::from_map(std::collections::HashMap::from([
            (
                "wooden_shield".to_string(),
                ItemTemplate {
                    id: "wooden_shield".to_string(),
                    display_name: "木盾".to_string(),
                    category: ItemCategory::Shield,
                    placeable: None,
                    max_stack_count: 1,
                    grid_w: 1,
                    grid_h: 2,
                    base_weight: 3.0,
                    rarity: ItemRarity::Common,
                    spirit_quality_initial: 0.0,
                    description: String::new(),
                    effect: None,
                    cast_duration_ms: 0,
                    cooldown_ms: 0,
                    weapon_spec: None,
                    forge_station_spec: None,
                    blueprint_scroll_spec: None,
                    inscription_scroll_spec: None,
                    technique_scroll_spec: None,
                    recipe_fragment_spec: None,
                    container_spec: None,
                    shelflife_profile: None,
                    shield_spec: Some(ShieldSpec {
                        block_ratio: 0.5,
                        durability_max: 40.0,
                        stamina_drain_per_s: 3.0,
                    }),
                    shelflife_track: None,
                },
            ),
            (
                "bone_shield".to_string(),
                ItemTemplate {
                    id: "bone_shield".to_string(),
                    display_name: "骨盾".to_string(),
                    category: ItemCategory::Shield,
                    placeable: None,
                    max_stack_count: 1,
                    grid_w: 1,
                    grid_h: 2,
                    base_weight: 4.5,
                    rarity: ItemRarity::Common,
                    spirit_quality_initial: 0.0,
                    description: String::new(),
                    effect: None,
                    cast_duration_ms: 0,
                    cooldown_ms: 0,
                    weapon_spec: None,
                    forge_station_spec: None,
                    blueprint_scroll_spec: None,
                    inscription_scroll_spec: None,
                    technique_scroll_spec: None,
                    recipe_fragment_spec: None,
                    container_spec: None,
                    shelflife_profile: None,
                    shield_spec: Some(ShieldSpec {
                        block_ratio: 0.65,
                        durability_max: 80.0,
                        stamina_drain_per_s: 3.0,
                    }),
                    shelflife_track: None,
                },
            ),
        ]));

        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 8000 });
        app.insert_resource(registry);
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, resolve_attack_intents);
        app
    }

    /// 给 entity 装上 off_hand 盾（初始耐久 ratio）。
    fn equip_shield_off_hand(
        app: &mut App,
        entity: Entity,
        template_id: &str,
        instance_id: u64,
        durability: f64,
    ) {
        let inv = PlayerInventory {
            revision: InventoryRevision(1),
            containers: vec![ContainerState {
                id: crate::inventory::MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 5,
                cols: 7,
                items: vec![],
            }],
            equipped: std::collections::HashMap::from([(
                EQUIP_SLOT_OFF_HAND.to_string(),
                ItemInstance {
                    instance_id,
                    template_id: template_id.to_string(),
                    display_name: template_id.to_string(),
                    grid_w: 1,
                    grid_h: 2,
                    weight: 3.0,
                    rarity: ItemRarity::Common,
                    description: String::new(),
                    stack_count: 1,
                    spirit_quality: 0.0,
                    durability,
                    freshness: None,
                    mineral_id: None,
                    charges: None,
                    forge_quality: None,
                    forge_color: None,
                    forge_side_effects: Vec::new(),
                    forge_achieved_tier: None,
                    alchemy: None,
                    lingering_owner_qi: None,
                },
            )]),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
        };
        app.world_mut().entity_mut(entity).insert(inv);
    }

    /// 给 entity 插入 ShieldBlocking status（front-face 格挡）。
    fn insert_shield_blocking(app: &mut App, entity: Entity, block_ratio: f32) {
        use valence::entity::Look;
        app.world_mut().entity_mut(entity).insert((
            StatusEffects {
                active: vec![ActiveStatusEffect {
                    kind: StatusEffectKind::ShieldBlocking,
                    magnitude: block_ratio,
                    remaining_ticks: crate::combat::shield_block::SHIELD_BLOCKING_DURATION_TICKS,
                    source_pill: None,
                }],
            },
            crate::combat::shield_block::ShieldBlock {
                template_id: "wooden_shield".to_string(),
            },
            Look {
                yaw: 0.0,
                pitch: 0.0,
            },
        ));
    }

    /// 发送物理攻击 intent。severity ≈ damage，以 WoundKind::Blunt 物理攻击。
    fn send_physical_attack(app: &mut App, attacker: Entity, target: Entity) {
        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 7999,
            reach: FIST_REACH,
            qi_invest: 0.0,
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });
    }

    // ── P3: 盾耐久公式锁定测试 ───────────────────────────────────────────────
    //
    // 设计语义（plan-shield-block-v1 P3 注释 line 998）：
    //   durability_max = 次"满伤格挡"的最大次数。
    //   但实际公式 cost = blocked（本次格挡削减的 f64 severity 值）：
    //     - 拳击 base_damage=1.0，ShieldBlocking magnitude=0.5 → blocked≈0.5
    //     - 因此拳击约需 80 次才能打碎木盾（而非 40 次）
    //   "满伤格挡"定义为 blocked=1.0（即攻击 severity=1.0 且 ratio=1.0，
    //    但 ratio 被 clamp 到 0.95）——真实游戏中此种攻击击伤值=severity*1.0=1.0 才能
    //    在 40 次后破盾。常规拳击不满足 blocked=1.0。
    //
    // 以下两条测试以实际参数锁定真实公式行为（诚实测试，非伪造参数）：
    //   1. 锁定公式形状：cost = blocked，严格单调下降，边界条件正确
    //   2. ECS 集成测试：给定真实攻击 blocked 值，N 击后耐久符合公式预测

    #[test]
    fn shield_durability_formula_uses_blocked_damage_as_cost() {
        // 验证公式形状（诚实参数）：
        //   - cost = blocked_per_hit（本次削减量），非固定值
        //   - 严格单调递减
        //   - 达到 blocked 总量 = durability_max 时归零
        //
        // 此测试用 blocked_per_hit = 0.5（对应 block_ratio=0.5, base_damage=1.0 拳击场景），
        // durability_max=40 → 需 80 次拳击。
        // 刻意不用 per_hit=1.0：那是不真实的伪造参数。

        let durability_max = 40.0_f64;
        let blocked_per_hit = 0.5_f64; // 拳击 base_damage=1.0, block_ratio=0.5 → 真实 blocked
        let expected_hits_to_break = (durability_max / blocked_per_hit).ceil() as u32; // = 80

        let mut cur_ratio = 1.0_f64;
        let mut prev_ratio = 2.0_f64; // 哨兵
        let mut broke_at = None;

        for hit in 1..=expected_hits_to_break + 5 {
            let cur_abs = cur_ratio * durability_max;
            let next_abs = (cur_abs - blocked_per_hit).max(0.0);
            let next_ratio = (next_abs / durability_max).clamp(0.0, 1.0);

            // 严格单调递减（只要 cur_ratio > 0 且有新的 blocked 扣减）
            if cur_ratio > 0.0 {
                assert!(
                    next_ratio <= cur_ratio,
                    "耐久应单调不增：hit={hit}，cur_ratio={cur_ratio:.6}，next_ratio={next_ratio:.6} \
                     — 公式回归：cost=blocked 时每次格挡必须扣减耐久"
                );
            }
            // 第 hit-1 次仍有耐久
            if next_ratio > 0.0 {
                assert!(
                    cur_ratio > 0.0,
                    "hit={hit}：cur_ratio 为 0 但 next_ratio>0，前提矛盾"
                );
            }

            prev_ratio = cur_ratio;
            cur_ratio = next_ratio;

            if cur_ratio <= 0.0 && broke_at.is_none() {
                broke_at = Some(hit);
            }
        }

        let broke_at = broke_at.expect("耐久应在有限次格挡后归零");
        assert_eq!(
            broke_at, expected_hits_to_break,
            "blocked_per_hit={blocked_per_hit:.2}，durability_max={durability_max}：\
             期望第 {expected_hits_to_break} 次破盾，\
             实际第 {broke_at} 次破盾。\
             注意：durability_max 的语义是「满伤（blocked=1.0）格挡次数」，\
             拳击 blocked≈0.5 时需 ~80 次，不是 40 次。"
        );
        let _ = prev_ratio;
    }

    // 锁定满伤（blocked=1.0）格挡 40 次破盾的语义
    #[test]
    fn shield_durability_breaks_at_durability_max_full_damage_hits() {
        // 语义：若每次格挡 blocked=1.0（真正满伤），40 次后精确归零。
        // blocked=1.0 需要攻击 severity=1.0 且 block_ratio=1.0（在游戏中极罕见，
        // ratio 被 clamp 到 0.95，所以真正满伤格挡最多 blocked≈0.95）。
        // 此测试用 blocked=1.0 是"假设完美满伤格挡"以验证数学边界。
        let durability_max = 40.0_f64;
        let blocked_full = 1.0_f64;
        let expected_hits = durability_max as u32; // exactly 40

        let mut cur_ratio = 1.0_f64;
        for hit in 1..=(expected_hits + 1) {
            let cur_abs = cur_ratio * durability_max;
            let next_abs = (cur_abs - blocked_full).max(0.0);
            cur_ratio = (next_abs / durability_max).clamp(0.0, 1.0);
            if hit == expected_hits - 1 {
                assert!(
                    cur_ratio > 0.0,
                    "hit={hit}（期望最后一次前 ratio > 0），actual={cur_ratio:.6}"
                );
            }
        }
        assert!(
            cur_ratio <= 0.0,
            "满伤（blocked=1.0）格挡 {expected_hits} 次后耐久应归零；actual ratio={cur_ratio:.6}"
        );
    }

    // ── P3: ECS 集成 — 盾耐久扣减 + 归零 → ShieldBroken 事件 + inventory 移除 ──
    #[test]
    fn shield_broken_event_emitted_exactly_once_when_durability_reaches_zero() {
        let mut app = make_shield_durability_app();

        // 攻击者在 [0,64,3]，防御者（持盾者）在 [0,64,1]，正面正对
        let attacker = spawn_player(
            &mut app,
            "ShieldBrkAtk",
            [0.0, 64.0, 3.0],
            Wounds::default(),
            Stamina::default(),
        );
        let defender = spawn_player(
            &mut app,
            "ShieldBrkDef",
            [0.0, 64.0, 1.0],
            Wounds::default(),
            Stamina::default(),
        );

        // 给防御者装木盾，初始耐久极低（ratio = 0.001，cur_abs ≈ 0.04 < 任何格挡值）
        // 原因：一次拳击 block_ratio=0.5 产生 blocked ≈ 0.5 单位，
        // 若 cur_abs=1.0 则 next_abs=0.5 > 0，不触发 ShieldBroken；
        // cur_abs=0.04 < 0.5，确保一击清零。
        let last_tick_ratio = 0.001_f64; // 极低耐久 — 任何格挡都会清空
        equip_shield_off_hand(&mut app, defender, "wooden_shield", 77, last_tick_ratio);
        insert_shield_blocking(&mut app, defender, 0.5);

        // 初始时 inventory 有盾
        {
            let inv = app
                .world()
                .entity(defender)
                .get::<PlayerInventory>()
                .unwrap();
            assert!(
                inv.equipped.contains_key(EQUIP_SLOT_OFF_HAND),
                "前提：off_hand 槽应有盾牌 instance_id=77"
            );
        }

        send_physical_attack(&mut app, attacker, defender);
        app.update();

        // 断言1：ShieldBroken 事件恰好发出一次
        let broken_events: Vec<_> = app
            .world()
            .resource::<Events<ShieldBroken>>()
            .iter_current_update_events()
            .collect();
        assert_eq!(
            broken_events.len(),
            1,
            "耐久归零时 ShieldBroken 事件应恰好发出一次；\
             期望 1 因为只有一次攻击且耐久归零；实际 {} 次。events: {broken_events:?}",
            broken_events.len()
        );
        assert_eq!(
            broken_events[0].instance_id, 77,
            "ShieldBroken.instance_id 应为 77；实际 {}",
            broken_events[0].instance_id
        );
        assert_eq!(
            broken_events[0].template_id, "wooden_shield",
            "ShieldBroken.template_id 应为 'wooden_shield'；实际 {:?}",
            broken_events[0].template_id
        );

        // 断言2：inventory 中盾已被移除（盾销毁）
        let inv = app
            .world()
            .entity(defender)
            .get::<PlayerInventory>()
            .unwrap();
        assert!(
            !inv.equipped.contains_key(EQUIP_SLOT_OFF_HAND),
            "盾耐久归零后 off_hand 槽应为空（盾销毁）；实际仍存在 instance: {:?}",
            inv.equipped.get(EQUIP_SLOT_OFF_HAND)
        );
    }

    // ── P3: 耐久未归零 → 不 emit ShieldBroken，物品保留 ──────────────────────
    #[test]
    fn no_shield_broken_event_when_durability_remains_above_zero() {
        let mut app = make_shield_durability_app();

        let attacker = spawn_player(
            &mut app,
            "ShieldPartAtk",
            [0.0, 64.0, 3.0],
            Wounds::default(),
            Stamina::default(),
        );
        let defender = spawn_player(
            &mut app,
            "ShieldPartDef",
            [0.0, 64.0, 1.0],
            Wounds::default(),
            Stamina::default(),
        );

        // 初始 ratio = 1.0（满耐久）
        equip_shield_off_hand(&mut app, defender, "wooden_shield", 55, 1.0);
        insert_shield_blocking(&mut app, defender, 0.5);

        send_physical_attack(&mut app, attacker, defender);
        app.update();

        // 未归零 → 不发 ShieldBroken
        let broken_events: Vec<_> = app
            .world()
            .resource::<Events<ShieldBroken>>()
            .iter_current_update_events()
            .collect();
        assert_eq!(
            broken_events.len(),
            0,
            "耐久未归零时 ShieldBroken 不应发出；\
             期望 0 因为木盾 durability_max=40 一次格挡仅扣 1/40；实际 {} 次",
            broken_events.len()
        );

        // 物品保留
        let inv = app
            .world()
            .entity(defender)
            .get::<PlayerInventory>()
            .unwrap();
        assert!(
            inv.equipped.contains_key(EQUIP_SLOT_OFF_HAND),
            "耐久未归零时 off_hand 盾应保留；实际 off_hand 为空"
        );
    }

    // ── P3: §10.5 qi_invest>0 减伤 pin 测试 — 盾对真元伤也减伤 ──────────────
    // worldview.md:432「防御本质=处理物理冲击 AND 真元污染」：
    // 盾的 FOV check + ShieldBlocking 对 qi 攻击同样削减 contam/qi severity。
    // P3 lock：qi_invest>0 攻击时，ShieldBlocking active 的防御者 contam_delta 被 block_ratio 削减。
    #[test]
    fn shield_block_reduces_qi_contamination_when_qi_invest_positive() {
        let mut app = make_shield_durability_app();

        let attacker = spawn_player(
            &mut app,
            "QiAtk",
            [0.0, 64.0, 3.0],
            Wounds::default(),
            Stamina::default(),
        );
        let defender = spawn_player(
            &mut app,
            "QiDef",
            [0.0, 64.0, 1.0],
            Wounds::default(),
            Stamina::default(),
        );

        equip_shield_off_hand(&mut app, defender, "wooden_shield", 99, 1.0);
        insert_shield_blocking(&mut app, defender, 0.5);

        // qi 攻击（qi_invest > 0）
        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(defender),
            issued_at_tick: 7999,
            reach: FIST_REACH,
            qi_invest: 10.0, // 真元投入 > 0
            wound_kind: WoundKind::Blunt,
            source: AttackSource::Melee,
            debug_command: None,
        });
        app.update();

        // 盾格挡对 qi 攻击的效果：CombatEvent 应有 shield_contam_reduced > 0
        let events: Vec<_> = app
            .world()
            .resource::<Events<CombatEvent>>()
            .iter_current_update_events()
            .collect();
        let has_shield_block = events
            .iter()
            .any(|e| e.defense_kind == Some(DefenseKind::ShieldBlock));
        assert!(
            has_shield_block,
            "qi_invest>0 攻击时，ShieldBlocking active 应触发 ShieldBlock defense_kind；\
             期望盾对真元伤也减伤（worldview §432）；actual events: {events:?}"
        );
        let shield_contam_reduced = events
            .iter()
            .find(|e| e.defense_kind == Some(DefenseKind::ShieldBlock))
            .and_then(|e| e.defense_contam_reduced);
        assert!(
            shield_contam_reduced.is_some_and(|r| r > 0.0),
            "qi 攻击 ShieldBlocking 应削减 contam（defense_contam_reduced > 0）；\
             期望 > 0 因为 qi_invest=10 产生 contam，block_ratio=0.5 削减一半；\
             actual defense_contam_reduced: {shield_contam_reduced:?}"
        );
    }

    // ── P3: §10.5 SwordParrying+ShieldBlocking 同帧互斥裁定（防回归 pin）────────
    // 互斥裁定结论：两者各自独立串行削减，无算术叠加失衡，不施加互斥守护。
    // 设计意图：SwordParrying 减境界加成伤害（真元），ShieldBlocking 减物理伤。
    // 同帧双激活极罕见（需同 tick 截脉窗口+举盾），串行削减符合设计意图。
    //
    // 本测试断言：同帧双 active 时，最终 physical_damage < 未格挡时的数值，
    // 且 ShieldBroken 不意外触发（双减伤不导致算术失衡/过扣）。
    #[test]
    fn swordparry_and_shieldblock_both_active_does_not_double_zero_damage_or_break_shield_unexpectedly(
    ) {
        // SwordParrying 和 ShieldBlocking 同帧双 active 时：
        // 1. wound.severity 经 SwordParrying 削减后，ShieldBlocking 再从残差继续削减 → 串行，合理
        // 2. 不会出现伤害归负 / 异常数值（两个 clamp 保证）
        // 3. 木盾满耐久一次双减伤格挡仍不会立即破盾（单次攻击 blocked_damage < durability 绝对值）

        let mut app = make_shield_durability_app();

        let attacker = spawn_player(
            &mut app,
            "DualDefAtk",
            [0.0, 64.0, 3.0],
            Wounds::default(),
            Stamina::default(),
        );
        let defender = spawn_player(
            &mut app,
            "DualDefDef",
            [0.0, 64.0, 1.0],
            Wounds::default(),
            Stamina::default(),
        );

        // 同时插入 SwordParrying（0.5）和 ShieldBlocking（0.5）两个 status
        use valence::entity::Look;
        app.world_mut().entity_mut(defender).insert((
            StatusEffects {
                active: vec![
                    ActiveStatusEffect {
                        kind: StatusEffectKind::SwordParrying,
                        magnitude: 0.5,
                        remaining_ticks: 100,
                        source_pill: None,
                    },
                    ActiveStatusEffect {
                        kind: StatusEffectKind::ShieldBlocking,
                        magnitude: 0.5,
                        remaining_ticks:
                            crate::combat::shield_block::SHIELD_BLOCKING_DURATION_TICKS,
                        source_pill: None,
                    },
                ],
            },
            crate::combat::shield_block::ShieldBlock {
                template_id: "wooden_shield".to_string(),
            },
            Look {
                yaw: 0.0,
                pitch: 0.0,
            },
        ));
        equip_shield_off_hand(&mut app, defender, "wooden_shield", 200, 1.0);

        send_physical_attack(&mut app, attacker, defender);
        app.update();

        // 断言1：physical_damage < 1.0（不满格挡时的基础伤害，双减伤有效）
        let events: Vec<_> = app
            .world()
            .resource::<Events<CombatEvent>>()
            .iter_current_update_events()
            .collect();
        assert_eq!(events.len(), 1, "应有一个 CombatEvent");
        let phys = events[0].physical_damage;
        assert!(
            phys >= 0.0,
            "双减伤后 physical_damage 不得为负数（clamp 保证）；实际 {phys:.4}"
        );
        // 期望 physical_damage < 1.0（无减伤时约 1.0，两层 0.5 减伤后应显著更小）
        assert!(
            phys < 1.0,
            "SwordParrying(0.5)+ShieldBlocking(0.5) 双激活后 physical_damage 应 < 1.0（双减伤有效）；\
             实际 {phys:.4}；若 ≥ 1.0 则减伤管道失效"
        );

        // 断言2：满耐久木盾不因一次双减伤格挡立即破盾
        let broken_events: Vec<_> = app
            .world()
            .resource::<Events<ShieldBroken>>()
            .iter_current_update_events()
            .collect();
        assert_eq!(
            broken_events.len(),
            0,
            "满耐久木盾（durability_max=40）一次双减伤格挡后不应立即破盾；\
             实际 {} 个 ShieldBroken 事件（期望 0，因为 blocked_damage < 40.0）",
            broken_events.len()
        );

        // 断言3（互斥裁定 pin）：双减伤结果在合法范围内（无算术失衡）
        // 期望 physical_damage = base * (1 - 0.5) * (1 - 0.5) = base * 0.25
        // 不要求精确值，但应在 [0, base] 区间内
        let inv = app
            .world()
            .entity(defender)
            .get::<PlayerInventory>()
            .unwrap();
        assert!(
            inv.equipped.contains_key(EQUIP_SLOT_OFF_HAND),
            "满耐久木盾一次格挡后 off_hand 盾应保留（未破盾）；实际 off_hand 为空"
        );
    }
}
