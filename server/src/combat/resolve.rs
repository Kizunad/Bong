use serde_json::json;
use valence::prelude::{
    Client, Commands, DVec3, Entity, EventReader, EventWriter, ParamSet, Position, Query, Res,
    ResMut, Username, With,
};

use crate::combat::armor::{ArmorProfileRegistry, ARMOR_MITIGATION_CAP};
use crate::combat::status::has_active_status;
use crate::combat::weapon::{Weapon, WeaponBroken};
use crate::combat::CombatClock;
use crate::combat::{
    components::{
        BodyPart, CombatState, DefenseWindow, DerivedAttrs, Lifecycle, LifecycleState, Stamina,
        StaminaState, StatusEffects, Wound, Wounds, HEAD_STUN_DURATION_TICKS,
        HEAD_STUN_SEVERITY_THRESHOLD, JIEMAI_CONCUSSION_BLEEDING_PER_SEC,
        JIEMAI_CONCUSSION_SEVERITY, JIEMAI_CONTAM_MULTIPLIER, JIEMAI_DEFENSE_QI_COST,
        JIEMAI_DEFENSE_WINDOW_MS, LEG_SLOWED_DURATION_TICKS, LEG_SLOWED_SEVERITY_THRESHOLD,
    },
    events::{
        ApplyStatusEffectIntent, AttackIntent, CombatEvent, DeathEvent, DefenseIntent,
        StatusEffectKind,
    },
    raycast::raycast_humanoid,
};
use crate::cultivation::components::{
    ColorKind, ContamSource, Contamination, CrackCause, Cultivation, MeridianCrack, MeridianSystem,
};
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::inventory::{
    discard_inventory_item_to_dropped_loot, move_equipped_item_to_first_container_slot,
    set_item_instance_durability, DroppedLootRegistry, InventoryDurabilityChangedEvent,
    PlayerInventory, EQUIP_SLOT_CHEST, EQUIP_SLOT_FEET, EQUIP_SLOT_HEAD, EQUIP_SLOT_LEGS,
};
use crate::npc::brain::canonical_npc_id;
use crate::npc::spawn::NpcMarker;
use crate::player::state::canonical_player_id;
use crate::schema::common::GameEventType;
use crate::schema::inventory::{EquipSlotV1, InventoryLocationV1};
use crate::schema::world_state::GameEvent;
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
    Option<&'a DerivedAttrs>,
);
type CombatAttackerItem<'a> = (
    &'a mut Cultivation,
    &'a mut MeridianSystem,
    Option<&'a DerivedAttrs>,
);

pub fn apply_defense_intents(
    mut defenses: EventReader<DefenseIntent>,
    mut defenders: Query<(&mut CombatState, Option<&StatusEffects>)>,
) {
    for defense in defenses.read() {
        let Ok((mut combat_state, status_effects)) = defenders.get_mut(defense.defender) else {
            continue;
        };

        if status_effects.is_some_and(|se| has_active_status(se, StatusEffectKind::Stunned)) {
            continue;
        }

        combat_state.incoming_window = Some(DefenseWindow {
            opened_at_tick: defense.issued_at_tick,
            duration_ms: JIEMAI_DEFENSE_WINDOW_MS,
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
    positions: Query<&Position>,
    npc_markers: Query<(), With<NpcMarker>>,
    npc_positions: Query<(Entity, &Position), With<NpcMarker>>,
    statuses: Query<&StatusEffects>,
    mut combatants: ParamSet<(Query<CombatAttackerItem<'_>>, Query<CombatTargetItem<'_>>)>,
    mut status_effect_intents: EventWriter<ApplyStatusEffectIntent>,
    mut out_events: EventWriter<CombatEvent>,
    mut death_events: EventWriter<DeathEvent>,
    mut durability_changed_tx: EventWriter<InventoryDurabilityChangedEvent>,
    // plan-weapon-v1 §6：武器加成 + 耐久扣减
    weapon_break: (
        Query<&mut Weapon>,
        EventWriter<WeaponBroken>,
        Commands,
        Query<&mut PlayerInventory>,
        Option<ResMut<DroppedLootRegistry>>,
    ),
) {
    let (
        mut weapons,
        mut weapon_broken_events,
        mut commands,
        mut inventories,
        mut dropped_loot_registry,
    ) = weapon_break;

    for intent in intents.read() {
        if statuses
            .get(intent.attacker)
            .is_ok_and(|se| has_active_status(se, StatusEffectKind::Stunned))
        {
            continue;
        }

        let Some((attacker_position, attacker_id, target_entity, target_position, target_id)) =
            resolve_intent_entities(intent, &clients, &positions, &npc_markers, &npc_positions)
        else {
            continue;
        };

        if intent.qi_invest <= 0.0 {
            continue;
        }

        let qi_invest = f64::from(intent.qi_invest);

        {
            let mut attacker_query = combatants.p0();
            let Ok((attacker_cultivation, _, _)) = attacker_query.get_mut(intent.attacker) else {
                continue;
            };

            if attacker_cultivation.qi_current + f64::EPSILON < qi_invest {
                continue;
            }
        }

        let Some(hit_probe) = raycast_humanoid(
            attacker_position + DVec3::new(0.0, ATTACKER_EYE_HEIGHT, 0.0),
            target_position,
            f64::from(intent.reach.max),
        ) else {
            continue;
        };
        let distance = hit_probe.distance as f32;

        let attacker_damage_multiplier = {
            let mut attacker_query = combatants.p0();
            let Ok((mut attacker_cultivation, mut attacker_meridians, attacker_attrs)) =
                attacker_query.get_mut(intent.attacker)
            else {
                continue;
            };

            attacker_cultivation.qi_current = (attacker_cultivation.qi_current - qi_invest)
                .clamp(0.0, attacker_cultivation.qi_max);
            if let Some(primary_meridian) = first_open_or_fallback_meridian(&mut attacker_meridians)
            {
                primary_meridian.throughput_current += qi_invest * ATTACK_QI_THROUGHPUT_FACTOR;
            }
            attacker_attrs
                .map(|attrs| attrs.attack_power)
                .unwrap_or(1.0)
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
            defender_attrs,
        )) = target_query.get_mut(target_entity)
        else {
            continue;
        };

        let decay = ((intent.reach.max - distance) / intent.reach.max.max(0.001)).clamp(0.0, 1.0);
        let hit_qi = (intent.qi_invest * decay).max(0.0);
        if hit_qi <= 0.0 {
            continue;
        }
        let (damage_multiplier, contam_multiplier, bleed_multiplier) =
            body_part_multipliers(hit_probe.body_part);
        let wound_profile = wound_kind_profile(intent.wound_kind);
        let defender_damage_multiplier = defender_attrs
            .map(|attrs| attrs.defense_power)
            .unwrap_or(1.0);
        // plan-weapon-v1 §6.1：查 attacker 的 Weapon component 得伤害倍率。
        // 无武器(赤手) → 1.0 基线;有武器 → attack × quality × durability。
        let weapon_multiplier: f32 = weapons
            .get(intent.attacker)
            .map(|w| w.damage_multiplier())
            .unwrap_or(1.0);
        let damage = (hit_qi
            * ATTACK_QI_DAMAGE_FACTOR
            * damage_multiplier
            * attacker_damage_multiplier
            * defender_damage_multiplier
            * weapon_multiplier)
            .max(1.0);
        let was_alive = wounds.health_current > 0.0;

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

        let mut emitted_contam_delta = f64::from(damage)
            * DEBUG_ATTACK_CONTAMINATION_FACTOR
            * f64::from(contam_multiplier)
            * wound_profile.contam_mul;
        let mut jiemai_success = false;

        // 先写入 stamina 与污染，再做截脉与护甲减免。

        stamina.current =
            (stamina.current - DEBUG_ATTACK_STAMINA_COST * decay).clamp(0.0, stamina.max);
        stamina.last_drain_tick = Some(clock.tick);
        stamina.state = if stamina.current <= 0.0 {
            StaminaState::Exhausted
        } else {
            StaminaState::Combat
        };

        contamination.entries.push(ContamSource {
            amount: emitted_contam_delta,
            color: ColorKind::Mellow,
            attacker_id: Some(attacker_id.clone()),
            introduced_at: clock.tick,
        });

        if let (Some(mut combat_state), Some(mut defender_cultivation)) =
            (combat_state, defender_cultivation)
        {
            let window_open = combat_state
                .incoming_window
                .as_ref()
                .is_some_and(|window| clock.tick < window.expires_at_tick());

            if window_open
                && defender_cultivation.qi_current + f64::EPSILON >= JIEMAI_DEFENSE_QI_COST
            {
                defender_cultivation.qi_current = (defender_cultivation.qi_current
                    - JIEMAI_DEFENSE_QI_COST)
                    .clamp(0.0, defender_cultivation.qi_max);

                if let Some(last_contam) = contamination.entries.last_mut() {
                    last_contam.amount *= JIEMAI_CONTAM_MULTIPLIER;
                    emitted_contam_delta = last_contam.amount;
                }

                wounds.entries.push(Wound {
                    location: hit_probe.body_part,
                    kind: crate::combat::components::WoundKind::Concussion,
                    severity: JIEMAI_CONCUSSION_SEVERITY,
                    bleeding_per_sec: JIEMAI_CONCUSSION_BLEEDING_PER_SEC,
                    created_at_tick: clock.tick,
                    inflicted_by: Some(attacker_id.clone()),
                });
                jiemai_success = true;
            }

            combat_state.incoming_window = None;
        }

        let mut wound = Wound {
            location: hit_probe.body_part,
            kind: intent.wound_kind,
            severity: damage,
            bleeding_per_sec: damage * 0.05 * bleed_multiplier * wound_profile.bleed_mul,
            created_at_tick: clock.tick,
            inflicted_by: Some(attacker_id.clone()),
        };

        // plan-armor-v1 §4.1：护甲减免在截脉判定之后应用。
        // 截脉当前只影响污染与额外 concussion，不直接改变本次伤口 severity。
        if let Some(attrs) = defender_attrs {
            let armor_mitigation =
                apply_armor_mitigation(&mut wound, attrs, &mut emitted_contam_delta);
            // 同步污染 source 的最后一条（本次命中刚 push）。
            if let Some(last_contam) = contamination.entries.last_mut() {
                last_contam.amount = emitted_contam_delta;
            }

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
                                match set_item_instance_durability(
                                    &mut inventory,
                                    instance_id,
                                    next_ratio,
                                ) {
                                    Ok(update) => {
                                        durability_changed_tx.send(
                                            InventoryDurabilityChangedEvent {
                                                entity: target_entity,
                                                revision: update.revision,
                                                instance_id: update.instance_id,
                                                durability: update.durability,
                                            },
                                        );
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

        wounds.health_current =
            (wounds.health_current - wound.severity).clamp(0.0, wounds.health_max);
        let wound_bleeding = wound.bleeding_per_sec;
        let wound_severity = wound.severity;
        wounds.entries.push(wound);

        if wound_bleeding > 0.0 {
            status_effect_intents.send(ApplyStatusEffectIntent {
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
            status_effect_intents.send(ApplyStatusEffectIntent {
                target: target_entity,
                kind: StatusEffectKind::Slowed,
                magnitude: 0.4,
                duration_ticks: LEG_SLOWED_DURATION_TICKS,
                issued_at_tick: clock.tick,
            });
        }

        if hit_probe.body_part == BodyPart::Head && wound_severity >= HEAD_STUN_SEVERITY_THRESHOLD {
            status_effect_intents.send(ApplyStatusEffectIntent {
                target: target_entity,
                kind: StatusEffectKind::Stunned,
                magnitude: 1.0,
                duration_ticks: HEAD_STUN_DURATION_TICKS,
                issued_at_tick: clock.tick,
            });
        }

        if let Some(primary_meridian) = first_open_or_fallback_meridian(&mut meridians) {
            primary_meridian.throughput_current += qi_invest * f64::from(decay);
            primary_meridian.cracks.push(MeridianCrack {
                severity: f64::from(wound_severity) * 0.02 * wound_profile.crack_mul,
                healing_progress: 0.0,
                cause: CrackCause::Attack,
                created_at: clock.tick,
            });
        }

        if let Some(mut life_record) = life_record {
            life_record.push(BiographyEntry::CombatHit {
                attacker_id: attacker_id.clone(),
                body_part: format!("{:?}", hit_probe.body_part),
                wound_kind: format!("{:?}", intent.wound_kind),
                damage: wound_severity,
                tick: clock.tick,
            });
        }

        let action_label = if intent.debug_command.is_some() {
            "debug_attack_intent"
        } else {
            "attack_intent"
        };
        let description = format!(
            "{} {} -> {} hit {:?} with {:?} for {:.1} damage (hit_qi {:.1}, jiemai={}) at {:.2} reach decay",
            action_label,
            attacker_id,
            target_id,
            hit_probe.body_part,
            intent.wound_kind,
            wound_severity,
            hit_qi,
            jiemai_success,
            decay
        );

        out_events.send(CombatEvent {
            attacker: intent.attacker,
            target: target_entity,
            resolved_at_tick: clock.tick,
            body_part: hit_probe.body_part,
            wound_kind: intent.wound_kind,
            damage: wound_severity,
            contam_delta: emitted_contam_delta,
            description,
        });

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
                    ("contam_delta".to_string(), json!(emitted_contam_delta)),
                    ("qi_invest".to_string(), json!(intent.qi_invest)),
                    ("hit_qi".to_string(), json!(hit_qi)),
                    ("jiemai_success".to_string(), json!(jiemai_success)),
                    ("reach_decay".to_string(), json!(decay)),
                ])),
            });
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
            death_events.send(DeathEvent {
                target: target_entity,
                cause: format!("{action_label}:{attacker_id}"),
                attacker: Some(intent.attacker),
                attacker_player_id,
                at_tick: clock.tick,
            });
        }
    }
}

fn body_part_multipliers(body_part: BodyPart) -> (f32, f32, f32) {
    match body_part {
        BodyPart::Head => (2.0, 1.5, 1.5),
        BodyPart::Chest => (1.0, 1.0, 1.0),
        BodyPart::Abdomen => (0.9, 1.2, 1.3),
        BodyPart::ArmL | BodyPart::ArmR => (0.7, 0.8, 0.8),
        BodyPart::LegL | BodyPart::LegR => (0.6, 0.7, 1.0),
    }
}

fn wound_kind_profile(kind: crate::combat::components::WoundKind) -> WoundKindProfile {
    match kind {
        crate::combat::components::WoundKind::Cut => WoundKindProfile {
            bleed_mul: 1.4,
            contam_mul: 1.0,
            crack_mul: 1.0,
        },
        crate::combat::components::WoundKind::Blunt => WoundKindProfile {
            bleed_mul: 0.7,
            contam_mul: 0.8,
            crack_mul: 1.3,
        },
        crate::combat::components::WoundKind::Pierce => WoundKindProfile {
            bleed_mul: 1.0,
            contam_mul: 1.2,
            crack_mul: 1.1,
        },
        crate::combat::components::WoundKind::Burn => WoundKindProfile {
            bleed_mul: 0.2,
            contam_mul: 1.3,
            crack_mul: 0.7,
        },
        crate::combat::components::WoundKind::Concussion => WoundKindProfile {
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
    positions: &Query<&Position>,
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
    positions: &Query<&Position>,
    npc_markers: &Query<(), With<NpcMarker>>,
) -> Option<(DVec3, String)> {
    if let Ok((_, position, username, _)) = clients.get(entity) {
        return Some((position.get(), canonical_player_id(username.0.as_str())));
    }
    if npc_markers.get(entity).is_ok() {
        let position = positions.get(entity).ok()?.get();
        return Some((position, canonical_npc_id(entity)));
    }
    None
}

fn resolve_debug_target(
    intent: &AttackIntent,
    action: &crate::player::gameplay::CombatAction,
    clients: &Query<CombatClientItem<'_>, CombatClientFilter>,
    positions: &Query<&Position>,
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
            let position = positions.get(target).ok()?.get();
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

    use crate::combat::armor::{ArmorProfile, ArmorProfileRegistry};
    use crate::combat::components::{
        BodyPart, CombatState, DefenseWindow, DerivedAttrs, Lifecycle, StatusEffects, WoundKind,
        Wounds, JIEMAI_CONTAM_MULTIPLIER, JIEMAI_DEFENSE_QI_COST,
    };
    use crate::combat::events::{
        ApplyStatusEffectIntent, AttackIntent, StatusEffectKind, FIST_REACH,
    };
    use crate::cultivation::components::{
        Contamination, CrackCause, Cultivation, MeridianId, MeridianSystem,
    };
    use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemCategory, ItemInstance, ItemRarity, ItemRegistry,
        ItemTemplate, PlayerInventory, WeaponSpec,
    };
    use crate::npc::brain::canonical_npc_id;
    use crate::npc::spawn::NpcMeleeProfile;
    use crate::npc::spawn::{spawn_test_npc_runtime_shape, NpcMarker};
    use crate::player::state::PlayerState;
    use valence::prelude::{
        bevy_ecs, App, Entity, Events, IntoSystemConfigs, Position, Resource, Update,
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
        app.world_mut()
            .spawn((
                client_bundle,
                Cultivation {
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
            .id()
    }

    fn weapon_test_registry() -> ItemRegistry {
        ItemRegistry::from_map(std::collections::HashMap::from([
            (
                "iron_sword".to_string(),
                ItemTemplate {
                    id: "iron_sword".to_string(),
                    display_name: "铁剑".to_string(),
                    category: ItemCategory::Weapon,
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
                },
            ),
            (
                "strong_sword".to_string(),
                ItemTemplate {
                    id: "strong_sword".to_string(),
                    display_name: "强剑".to_string(),
                    category: ItemCategory::Weapon,
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
                },
            ),
            (
                "glass_sword".to_string(),
                ItemTemplate {
                    id: "glass_sword".to_string(),
                    display_name: "玻璃剑".to_string(),
                    category: ItemCategory::Weapon,
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
        app.add_event::<crate::combat::weapon::WeaponBroken>();
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
    fn resolve_debug_attack_applies_damage_contamination_throughput_and_death() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 12 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
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
        app.add_event::<crate::combat::weapon::WeaponBroken>();
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
        app.add_event::<crate::combat::weapon::WeaponBroken>();
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
    }

    #[test]
    fn player_to_npc_and_npc_to_player_share_same_resolver_path() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 91 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
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
            debug_command: None,
        });
        app.world_mut().send_event(AttackIntent {
            attacker: npc,
            target: Some(player),
            issued_at_tick: 90,
            reach: NpcMeleeProfile::spear().reach,
            qi_invest: 10.0,
            wound_kind: NpcMeleeProfile::spear().wound_kind,
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

    #[test]
    fn fist_reach_misses_when_target_is_outside_physical_range() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 900 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
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
            [2.0, 64.0, 0.0],
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
    fn insufficient_qi_prevents_attack_side_effects() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 901 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
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
    fn debug_target_selection_does_not_change_damage_when_qi_invest_matches() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 902 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
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
                qi_current: 20.0,
                qi_max: 100.0,
                ..Cultivation::default()
            },
        ));

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 999,
            reach: FIST_REACH,
            qi_invest: 20.0,
            wound_kind: WoundKind::Blunt,
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

        assert_eq!(cultivation.qi_current, 20.0 - JIEMAI_DEFENSE_QI_COST);
        assert!(state.incoming_window.is_none());
        assert_eq!(wounds.entries.len(), 2);
        assert!(wounds
            .entries
            .iter()
            .any(|w| w.kind == WoundKind::Concussion));
        let base_contam = f64::from(event.damage) * 0.25 * 0.8;
        assert_eq!(event.contam_delta, base_contam * JIEMAI_CONTAM_MULTIPLIER);
        assert_eq!(contamination.entries.len(), 1);
        assert_eq!(contamination.entries[0].amount, event.contam_delta);
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
            }],
        });

        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(target),
            issued_at_tick: 1099,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
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
        app.add_systems(Update, apply_defense_intents);

        let defender = app
            .world_mut()
            .spawn((
                CombatState::default(),
                StatusEffects {
                    active: vec![crate::combat::components::ActiveStatusEffect {
                        kind: StatusEffectKind::Stunned,
                        magnitude: 1.0,
                        remaining_ticks: 20,
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
    fn head_hit_applies_stunned_status() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 1200 });
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
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
            debug_command: None,
        });
        app.world_mut().send_event(AttackIntent {
            attacker: amp_attacker,
            target: Some(amp_target),
            issued_at_tick: 1299,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
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
            debug_command: None,
        });
        app.world_mut().send_event(AttackIntent {
            attacker: reduced_attacker,
            target: Some(reduced_target),
            issued_at_tick: 1349,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
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
            debug_command: None,
        });
        app.world_mut().send_event(AttackIntent {
            attacker: armed,
            target: Some(t2),
            issued_at_tick: 1399,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
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
            debug_command: None,
        });
        app.world_mut().send_event(AttackIntent {
            attacker: armed,
            target: Some(armed_target),
            issued_at_tick: 1419,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
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
            debug_command: None,
        });
        app.world_mut().send_event(AttackIntent {
            attacker: blunt_attacker,
            target: Some(blunt_target),
            issued_at_tick: 1399,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
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
            debug_command: None,
        });
        app.world_mut().send_event(AttackIntent {
            attacker: blunt_attacker,
            target: Some(blunt_target),
            issued_at_tick: 1499,
            reach: FIST_REACH,
            qi_invest: 10.0,
            wound_kind: WoundKind::Blunt,
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
            debug_command: None,
        });
        app.world_mut().send_event(AttackIntent {
            attacker: npc_b,
            target: Some(npc_a),
            issued_at_tick: 199,
            reach: NpcMeleeProfile::spear().reach,
            qi_invest: 12.0,
            wound_kind: WoundKind::Pierce,
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
}
