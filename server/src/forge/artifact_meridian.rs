//! plan-qixiu-depth-v1 P1/P2/P4/P5 — 法器铭纹模型与 inventory tag 持久化。
//!
//! 运行时 component 仍是 `ArtifactMeridian` / `ArtifactColor`；持久化与 client 同步复用
//! 现有 `ItemInstance.forge_side_effects`，以一个 `artifact_state:` JSON tag 承载完整状态。
//! 这样旧快照不需要 schema migration，缺 tag 的旧物品自然视为未活化。

use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, Commands, Component, Entity, Event, EventReader, EventWriter, Position, Query, Res,
    ResMut,
};

use super::artifact_color::ArtifactColor;
use super::resonance::compute_resonance;
use crate::combat::events::CombatEvent;
use crate::combat::weapon::{Weapon, WeaponBroken};
use crate::cultivation::components::{ColorKind, Cultivation, QiColor};
use crate::inventory::{
    bump_revision, discard_inventory_item_to_dropped_loot, inventory_item_by_instance_mut,
    move_equipped_item_to_first_container_slot, set_item_instance_durability, DroppedLootRegistry,
    ItemInstance, PlayerInventory,
};
use crate::player::gameplay::PendingGameplayNarrations;
use crate::qi_physics::ledger::{QiAccountId, QiTransfer, QiTransferReason, WorldQiAccount};
use crate::schema::common::NarrationStyle;
use crate::schema::inventory::{EquipSlotV1, EquipStateV1, InventoryLocationV1};
use crate::world::dimension::DimensionKind;
use crate::world::zone::ZoneRegistry;

pub const ARTIFACT_STATE_PREFIX: &str = "artifact_state:";
const MICRO_CRACK_LIMIT: f64 = 0.15;
const CRACK_WARNING_LIMIT: f64 = 0.16;
const DEEP_CRACK_LIMIT: f64 = 0.41;
const SEVERED_LIMIT: f64 = 0.71;
const TICKS_PER_HOUR: u64 = 20 * 60 * 60;
const TICKS_PER_DAY: u64 = 24 * TICKS_PER_HOUR;

pub type GrooveId = String;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaterialSpec {
    pub grooves: usize,
    pub depth_cap: f64,
    pub lock_coeff: f64,
    pub max_quality_tier: u8,
}

impl MaterialSpec {
    pub const fn mundane() -> Self {
        Self {
            grooves: 1,
            depth_cap: 10.0,
            lock_coeff: 0.01,
            max_quality_tier: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Groove {
    pub id: GrooveId,
    pub depth: f64,
    pub depth_cap: f64,
    pub flow_capacity: f64,
    pub crack_severity: f64,
}

impl Groove {
    pub fn is_severed(&self) -> bool {
        self.crack_severity >= SEVERED_LIMIT
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Component)]
pub struct ArtifactMeridian {
    pub grooves: Vec<Groove>,
    pub total_depth: f64,
    pub depth_cap: f64,
    pub quality_tier: u8,
    pub material_lock_coefficient: f64,
    pub material_max_quality_tier: u8,
    pub overload_cracks: u8,
    pub created_at_tick: u64,
    pub last_flow_tick: u64,
    pub last_maintenance_tick: u64,
    pub usage_hours: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactState {
    pub meridian: ArtifactMeridian,
    pub color: ArtifactColor,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct ArtifactMeridianDepthChanged {
    pub owner: Entity,
    pub instance_id: u64,
    pub groove_id: GrooveId,
    pub old_depth: f64,
    pub new_depth: f64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct ArtifactMeridianCracked {
    pub owner: Entity,
    pub instance_id: u64,
    pub groove_id: GrooveId,
    pub old_severity: f64,
    pub new_severity: f64,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct ArtifactTierEvolved {
    pub owner: Entity,
    pub instance_id: u64,
    pub old_tier: u8,
    pub new_tier: u8,
}

impl ArtifactMeridian {
    pub fn new_from_spec(
        spec: MaterialSpec,
        consecration_qi: f64,
        tempering_quality_factor: f64,
        created_at_tick: u64,
        quality_tier: u8,
    ) -> Self {
        let groove_count = spec.grooves.max(1);
        let initial_depth = (consecration_qi.max(0.0) * 0.01).min(spec.depth_cap);
        let flow_capacity = (spec.depth_cap * tempering_quality_factor.clamp(0.3, 0.8)).max(0.1);
        let grooves = (0..groove_count)
            .map(|idx| Groove {
                id: groove_id_for_index(idx),
                depth: initial_depth,
                depth_cap: spec.depth_cap,
                flow_capacity,
                crack_severity: 0.0,
            })
            .collect::<Vec<_>>();
        let mut meridian = Self {
            grooves,
            total_depth: 0.0,
            depth_cap: spec.depth_cap * groove_count as f64,
            quality_tier: quality_tier.min(spec.max_quality_tier),
            material_lock_coefficient: spec.lock_coeff,
            material_max_quality_tier: spec.max_quality_tier,
            overload_cracks: 0,
            created_at_tick,
            last_flow_tick: created_at_tick,
            last_maintenance_tick: created_at_tick,
            usage_hours: 0.0,
        };
        meridian.recompute_total_depth();
        meridian
    }

    pub fn average_depth(&self) -> f64 {
        if self.grooves.is_empty() {
            0.0
        } else {
            self.total_depth / self.grooves.len() as f64
        }
    }

    pub fn maturity(&self) -> f64 {
        if self.depth_cap > 0.0 {
            (self.total_depth / self.depth_cap).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    pub fn effective_flow_capacity(&self) -> f64 {
        self.grooves
            .iter()
            .filter(|groove| !groove.is_severed())
            .map(|groove| groove.flow_capacity * (1.0 - groove.crack_severity).clamp(0.0, 1.0))
            .sum::<f64>()
            .max(0.0)
    }

    pub fn deepen_on_flow(&mut self, qi_flow_amount: f64, tick: u64) -> Vec<DepthChange> {
        let flow = qi_flow_amount.max(0.0);
        if flow <= f64::EPSILON {
            return Vec::new();
        }
        let active_indices = self
            .grooves
            .iter()
            .enumerate()
            .filter_map(|(idx, groove)| (!groove.is_severed()).then_some(idx))
            .collect::<Vec<_>>();
        if active_indices.is_empty() {
            return Vec::new();
        }
        let per_groove_flow = flow / active_indices.len() as f64;
        let mut changes = Vec::new();
        for idx in active_indices {
            let groove = &mut self.grooves[idx];
            let old = groove.depth;
            let diminishing =
                (1.0 - groove.depth / groove.depth_cap.max(f64::EPSILON)).clamp(0.0, 1.0);
            let increment = per_groove_flow * self.material_lock_coefficient * diminishing;
            groove.depth = (groove.depth + increment).clamp(0.0, groove.depth_cap);
            if (groove.depth - old).abs() > f64::EPSILON {
                changes.push(DepthChange {
                    groove_id: groove.id.clone(),
                    old_depth: old,
                    new_depth: groove.depth,
                });
            }
        }
        self.last_flow_tick = tick;
        self.usage_hours += flow / 20.0;
        self.recompute_total_depth();
        changes
    }

    pub fn apply_overload(&mut self, qi_flow_amount: f64) -> Option<CrackChange> {
        let capacity = self.effective_flow_capacity();
        if qi_flow_amount <= capacity || capacity <= f64::EPSILON {
            return None;
        }
        let target_idx = self
            .grooves
            .iter()
            .enumerate()
            .filter(|(_, groove)| !groove.is_severed())
            .max_by(|(_, a), (_, b)| {
                a.flow_capacity
                    .partial_cmp(&b.flow_capacity)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(idx, _)| idx)?;

        let groove = &mut self.grooves[target_idx];
        let old = groove.crack_severity;
        let extra = ((qi_flow_amount - capacity) / capacity).max(0.0) * 0.3;
        groove.crack_severity = (groove.crack_severity + extra).clamp(0.0, 1.0);
        if groove.crack_severity > old {
            self.overload_cracks = self.overload_cracks.saturating_add(1);
            if old < DEEP_CRACK_LIMIT && groove.crack_severity >= DEEP_CRACK_LIMIT {
                self.quality_tier = self.quality_tier.saturating_sub(1);
            }
            Some(CrackChange {
                groove_id: groove.id.clone(),
                old_severity: old,
                new_severity: groove.crack_severity,
            })
        } else {
            None
        }
    }

    pub fn self_heal_micro_cracks(&mut self, now_tick: u64) -> usize {
        if now_tick.saturating_sub(self.last_flow_tick) < 2 * TICKS_PER_HOUR {
            return 0;
        }
        let mut healed = 0;
        for groove in &mut self.grooves {
            if groove.crack_severity > 0.0 && groove.crack_severity <= MICRO_CRACK_LIMIT {
                groove.crack_severity = 0.0;
                healed += 1;
            }
        }
        healed
    }

    pub fn repair_at_station(&mut self, repair_power: f64) -> usize {
        let mut repaired = 0;
        for groove in &mut self.grooves {
            if groove.is_severed() || groove.crack_severity <= 0.0 {
                continue;
            }
            groove.crack_severity = (groove.crack_severity - repair_power.max(0.0)).max(0.0);
            repaired += 1;
        }
        repaired
    }

    pub fn maintenance_cost_per_day(&self) -> f64 {
        f64::from(self.quality_tier) * 2.0 * self.grooves.len() as f64
    }

    pub fn apply_maintenance(&mut self, available_qi: &mut f64, now_tick: u64) -> bool {
        let cost = self.maintenance_cost_per_day();
        if cost <= f64::EPSILON || *available_qi + f64::EPSILON < cost {
            return false;
        }
        *available_qi = (*available_qi - cost).max(0.0);
        self.last_maintenance_tick = now_tick;
        true
    }

    pub fn decay_without_maintenance(&mut self, now_tick: u64) -> bool {
        if now_tick.saturating_sub(self.last_maintenance_tick) < 3 * TICKS_PER_DAY {
            return false;
        }
        let mut changed = false;
        for groove in &mut self.grooves {
            if groove.is_severed() {
                continue;
            }
            let old = groove.depth;
            groove.depth = (groove.depth - 0.1).max(0.0);
            changed |= (groove.depth - old).abs() > f64::EPSILON;
        }
        if self.quality_tier > 0 {
            self.quality_tier = self.quality_tier.saturating_sub(1);
            changed = true;
        }
        if changed {
            self.recompute_total_depth();
        }
        changed
    }

    pub fn try_evolve(&mut self, resonance: f64) -> Option<(u8, u8)> {
        let old = self.quality_tier;
        let avg = self.average_depth();
        let next = match self.quality_tier {
            0 if avg >= 15.0 && resonance >= 0.4 && self.usage_hours >= 3.0 => 1,
            1 if avg >= 50.0
                && resonance >= 0.6
                && self.usage_hours >= 15.0
                && !self.has_cracks() =>
            {
                2
            }
            2 if avg >= 120.0
                && resonance >= 0.8
                && self.usage_hours >= 40.0
                && self.material_max_quality_tier >= 3 =>
            {
                3
            }
            _ => old,
        }
        .min(self.material_max_quality_tier);
        if next > old {
            self.quality_tier = next;
            Some((old, next))
        } else {
            None
        }
    }

    pub fn has_cracks(&self) -> bool {
        self.grooves
            .iter()
            .any(|groove| groove.crack_severity > 0.0)
    }

    pub fn all_grooves_severed(&self) -> bool {
        !self.grooves.is_empty() && self.grooves.iter().all(Groove::is_severed)
    }

    fn recompute_total_depth(&mut self) {
        self.total_depth = self.grooves.iter().map(|groove| groove.depth).sum();
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DepthChange {
    pub groove_id: GrooveId,
    pub old_depth: f64,
    pub new_depth: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CrackChange {
    pub groove_id: GrooveId,
    pub old_severity: f64,
    pub new_severity: f64,
}

pub fn material_spec_for_carrier(kind: crate::combat::carrier::CarrierKind) -> MaterialSpec {
    use crate::combat::carrier::CarrierKind;
    match kind {
        CarrierKind::BoneChip => MaterialSpec {
            grooves: 2,
            depth_cap: 30.0,
            lock_coeff: 0.03,
            max_quality_tier: 1,
        },
        CarrierKind::YibianShougu => MaterialSpec {
            grooves: 3,
            depth_cap: 60.0,
            lock_coeff: 0.06,
            max_quality_tier: 1,
        },
        CarrierKind::LingmuArrow => MaterialSpec {
            grooves: 4,
            depth_cap: 100.0,
            lock_coeff: 0.08,
            max_quality_tier: 2,
        },
        CarrierKind::DyedBone => MaterialSpec {
            grooves: 5,
            depth_cap: 150.0,
            lock_coeff: 0.10,
            max_quality_tier: 2,
        },
        CarrierKind::FenglingheBone => MaterialSpec {
            grooves: 6,
            depth_cap: 200.0,
            lock_coeff: 0.12,
            max_quality_tier: 3,
        },
        CarrierKind::ShangguBone => MaterialSpec {
            grooves: 8,
            depth_cap: 300.0,
            lock_coeff: 0.15,
            max_quality_tier: 3,
        },
    }
}

pub fn material_spec_for_template_id(template_id: &str, achieved_tier: u8) -> MaterialSpec {
    match template_id {
        "bone_sword" => MaterialSpec {
            grooves: 3,
            depth_cap: 60.0,
            lock_coeff: 0.06,
            max_quality_tier: 1,
        },
        "lingmu_sword" => MaterialSpec {
            grooves: 4,
            depth_cap: 100.0,
            lock_coeff: 0.08,
            max_quality_tier: 2,
        },
        id if id.contains("lingmu") || id.contains("spirit_wood") => MaterialSpec {
            grooves: 4,
            depth_cap: 100.0,
            lock_coeff: 0.08,
            max_quality_tier: 2,
        },
        id if id.contains("bone") => MaterialSpec {
            grooves: 3,
            depth_cap: 60.0,
            lock_coeff: 0.06,
            max_quality_tier: 1,
        },
        id if id.contains("fengling") => MaterialSpec {
            grooves: 6,
            depth_cap: 200.0,
            lock_coeff: 0.12,
            max_quality_tier: 3,
        },
        id if id.contains("shanggu") => MaterialSpec {
            grooves: 8,
            depth_cap: 300.0,
            lock_coeff: 0.15,
            max_quality_tier: 3,
        },
        _ if achieved_tier >= 3 => MaterialSpec {
            grooves: 6,
            depth_cap: 200.0,
            lock_coeff: 0.12,
            max_quality_tier: 3,
        },
        _ if achieved_tier == 2 => MaterialSpec {
            grooves: 4,
            depth_cap: 100.0,
            lock_coeff: 0.08,
            max_quality_tier: 2,
        },
        _ if achieved_tier == 1 => MaterialSpec {
            grooves: 2,
            depth_cap: 30.0,
            lock_coeff: 0.03,
            max_quality_tier: 1,
        },
        _ => MaterialSpec::mundane(),
    }
}

pub fn artifact_state_for_outcome(
    template_id: &str,
    achieved_tier: u8,
    forge_quality: f32,
    color: Option<ColorKind>,
    consecration_qi: f64,
    created_at_tick: u64,
) -> ArtifactState {
    let spec = crate::combat::carrier::CarrierKind::from_template_id(template_id)
        .map(material_spec_for_carrier)
        .unwrap_or_else(|| material_spec_for_template_id(template_id, achieved_tier));
    let tempering_factor = if forge_quality >= 0.9 {
        0.8
    } else if forge_quality >= 0.65 {
        0.5
    } else {
        0.3
    };
    ArtifactState {
        meridian: ArtifactMeridian::new_from_spec(
            spec,
            consecration_qi,
            tempering_factor,
            created_at_tick,
            achieved_tier.saturating_sub(1),
        ),
        color: color
            .map(|color| ArtifactColor::from_initial_color(color, consecration_qi.max(1.0) * 0.1))
            .unwrap_or_default(),
    }
}

pub fn artifact_state_from_item(item: &ItemInstance) -> Option<ArtifactState> {
    item.forge_side_effects.iter().find_map(|tag| {
        let payload = tag.strip_prefix(ARTIFACT_STATE_PREFIX)?;
        serde_json::from_str::<ArtifactState>(payload).ok()
    })
}

pub fn write_artifact_state_to_item(item: &mut ItemInstance, state: &ArtifactState) {
    item.forge_side_effects
        .retain(|tag| !tag.starts_with(ARTIFACT_STATE_PREFIX));
    if let Ok(encoded) = serde_json::to_string(state) {
        item.forge_side_effects
            .push(format!("{ARTIFACT_STATE_PREFIX}{encoded}"));
    }
}

pub fn artifact_resonance_for_item(
    item: &ItemInstance,
    user_color: Option<&QiColor>,
) -> Option<f64> {
    let state = artifact_state_from_item(item)?;
    let user_color = user_color?;
    Some(compute_resonance(
        &state.color,
        user_color,
        state.meridian.total_depth,
        state.meridian.depth_cap,
    ))
}

pub fn artifact_resonance_for_inventory(
    inventory: &PlayerInventory,
    instance_id: u64,
    user_color: Option<&QiColor>,
) -> Option<f64> {
    find_item_by_instance(inventory, instance_id)
        .and_then(|item| artifact_resonance_for_item(item, user_color))
}

pub fn apply_evolution_qi_cost(cultivation: &mut Cultivation) -> f64 {
    let cost = (cultivation.qi_current * 0.3).max(0.0);
    cultivation.qi_current = (cultivation.qi_current - cost).clamp(0.0, cultivation.qi_max);
    cost
}

#[allow(clippy::too_many_arguments)] // Bevy system signature; each resource owns a separate side effect.
pub fn artifact_meridian_deepen_on_use(
    mut events: EventReader<CombatEvent>,
    positions: Query<&Position>,
    zone_registry: Option<Res<ZoneRegistry>>,
    mut qi_account: Option<ResMut<WorldQiAccount>>,
    mut holders: Query<(
        &mut Weapon,
        &mut PlayerInventory,
        Option<&QiColor>,
        Option<&mut Cultivation>,
    )>,
    mut dropped_loot_registry: Option<ResMut<DroppedLootRegistry>>,
    mut commands: Commands,
    mut depth_events: EventWriter<ArtifactMeridianDepthChanged>,
    mut crack_events: EventWriter<ArtifactMeridianCracked>,
    mut evolved_events: EventWriter<ArtifactTierEvolved>,
    mut weapon_broken_events: EventWriter<WeaponBroken>,
) {
    for event in events.read() {
        if event.debug_command {
            continue;
        }
        let Ok((mut weapon, mut inventory, user_color, cultivation)) =
            holders.get_mut(event.attacker)
        else {
            continue;
        };

        let weapon_instance_id = weapon.instance_id;
        let weapon_template_id = weapon.template_id.clone();
        let mut should_broken = false;

        let Some(item) = inventory_item_by_instance_mut(&mut inventory, weapon_instance_id) else {
            continue;
        };
        let Some(mut state) = artifact_state_from_item(item) else {
            continue;
        };
        let flow = f64::from(event.damage).max(0.0);
        if let Some(user_color) = user_color {
            state.color.record_use(user_color.main, flow);
        }
        for change in state.meridian.deepen_on_flow(flow, event.resolved_at_tick) {
            depth_events.send(ArtifactMeridianDepthChanged {
                owner: event.attacker,
                instance_id: weapon_instance_id,
                groove_id: change.groove_id,
                old_depth: change.old_depth,
                new_depth: change.new_depth,
            });
        }
        if let Some(change) = state.meridian.apply_overload(flow) {
            if change.old_severity < CRACK_WARNING_LIMIT
                && change.new_severity >= CRACK_WARNING_LIMIT
            {
                crack_events.send(ArtifactMeridianCracked {
                    owner: event.attacker,
                    instance_id: weapon_instance_id,
                    groove_id: change.groove_id,
                    old_severity: change.old_severity,
                    new_severity: change.new_severity,
                });
            }
        }
        if state.meridian.all_grooves_severed() {
            should_broken = true;
        }
        let user_color = user_color.cloned().unwrap_or_default();
        let resonance = compute_resonance(
            &state.color,
            &user_color,
            state.meridian.total_depth,
            state.meridian.depth_cap,
        );
        if let Some((old_tier, new_tier)) = state.meridian.try_evolve(resonance) {
            if let Some(mut cultivation) = cultivation {
                // 守恒：apply_evolution_qi_cost 返回实际消耗量，必须转入玩家所在 zone ledger。
                // WorldQiAccount 缺失时跳过整段扣减（cost=0），绝不蒸发。
                if let Some(ref mut account) = qi_account {
                    let cost = apply_evolution_qi_cost(&mut cultivation);
                    if cost > 0.0 {
                        let zone_id = positions
                            .get(event.attacker)
                            .ok()
                            .and_then(|pos| {
                                zone_registry.as_deref().and_then(|zr| {
                                    zr.find_zone(DimensionKind::Overworld, pos.0)
                                        .map(|zone| QiAccountId::zone(zone.name.clone()))
                                })
                            })
                            .unwrap_or_else(|| {
                                QiAccountId::overflow(format!(
                                    "artifact_evolution_no_zone:{}",
                                    event.attacker.to_bits()
                                ))
                            });
                        if !account.has_account(&zone_id) {
                            let _ = account.set_balance(zone_id.clone(), 0.0);
                        }
                        let zone_balance = account.balance(&zone_id);
                        let _ = account.set_balance(zone_id.clone(), zone_balance + cost);
                        let player_id =
                            QiAccountId::player(format!("entity:{}", event.attacker.to_bits()));
                        account.push_transfer_audit(QiTransfer {
                            from: player_id,
                            to: zone_id,
                            amount: cost,
                            reason: QiTransferReason::ArtifactEvolution,
                        });
                    }
                }
                // 若 WorldQiAccount 缺失，跳过 qi_current 扣减（守恒保卫：无轨迹就无扣减）。
            }
            evolved_events.send(ArtifactTierEvolved {
                owner: event.attacker,
                instance_id: weapon_instance_id,
                old_tier,
                new_tier,
            });
        }
        write_artifact_state_to_item(item, &state);

        if should_broken {
            let broken_slot = inventory.equipped.iter().find_map(|(slot, contents)| {
                contents
                    .iter_all()
                    .find(|item| item.instance_id == weapon_instance_id)
                    .map(|_| match slot.as_str() {
                        crate::inventory::EQUIP_SLOT_MAIN_HAND => EquipSlotV1::MainHand,
                        crate::inventory::EQUIP_SLOT_OFF_HAND => EquipSlotV1::OffHand,
                        _ => EquipSlotV1::MainHand,
                    })
            });
            weapon.durability = 0.0;
            if let Err(error) =
                set_item_instance_durability(&mut inventory, weapon_instance_id, 0.0)
            {
                tracing::warn!(
                    "[bong][forge] failed to persist broken durability for instance {}: {}",
                    weapon_instance_id,
                    error
                );
            }
            let moved = match move_equipped_item_to_first_container_slot(
                &mut inventory,
                weapon_instance_id,
            ) {
                Ok(_) => true,
                Err(error) => {
                    tracing::warn!(
                        "[bong][forge] failed to move broken weapon instance {} into container: {}",
                        weapon_instance_id,
                        error
                    );
                    if let Some(slot) = broken_slot {
                        if let Some(dropped_loot_registry) = dropped_loot_registry.as_mut() {
                            if let Ok(position) = positions.get(event.attacker) {
                                match discard_inventory_item_to_dropped_loot(
                                    &mut inventory,
                                    dropped_loot_registry,
                                    [position.0.x, position.0.y, position.0.z],
                                    DimensionKind::Overworld,
                                    weapon_instance_id,
                                    &InventoryLocationV1::Equip {
                                        slot,
                                        state: EquipStateV1::Held,
                                    },
                                ) {
                                    Ok(_) => true,
                                    Err(drop_error) => {
                                        tracing::warn!(
                                            "[bong][forge] failed to drop broken weapon instance {} after container fallback failed: {}",
                                            weapon_instance_id,
                                            drop_error
                                        );
                                        false
                                    }
                                }
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }
            };
            if moved {
                commands.entity(event.attacker).remove::<Weapon>();
                weapon_broken_events.send(WeaponBroken {
                    entity: event.attacker,
                    instance_id: weapon_instance_id,
                    template_id: weapon_template_id,
                });
            }
        }
        bump_revision(&mut inventory);
    }
}

pub fn artifact_color_evolve_tick(
    clock: Res<crate::combat::CombatClock>,
    mut inventories: Query<&mut PlayerInventory>,
) {
    if !clock.tick.is_multiple_of(20) {
        return;
    }
    for mut inventory in &mut inventories {
        let mut changed = false;
        changed |= for_each_artifact_item_mut(&mut inventory, |item| {
            let Some(mut state) = artifact_state_from_item(item) else {
                return false;
            };
            let old_weights = state.color.practice_log.weights.clone();
            let old_main = state.color.main;
            let old_secondary = state.color.secondary;
            let old_chaotic = state.color.is_chaotic;
            state.color.decay_tick();
            let color_changed = state.color.practice_log.weights != old_weights
                || state.color.main != old_main
                || state.color.secondary != old_secondary
                || state.color.is_chaotic != old_chaotic;
            if !color_changed {
                return false;
            }
            write_artifact_state_to_item(item, &state);
            true
        });
        if changed {
            bump_revision(&mut inventory);
        }
    }
}

pub fn artifact_meridian_maintenance_tick(
    clock: Res<crate::combat::CombatClock>,
    zone_registry: Option<Res<ZoneRegistry>>,
    mut qi_account: Option<ResMut<WorldQiAccount>>,
    mut inventories: Query<(
        Entity,
        &mut PlayerInventory,
        Option<&mut Cultivation>,
        Option<&Position>,
    )>,
) {
    if !clock.tick.is_multiple_of(TICKS_PER_DAY) {
        return;
    }
    for (entity, mut inventory, cultivation, position) in &mut inventories {
        // 守恒门卫：WorldQiAccount 缺失时跳过整段养护（不扣 qi_current），
        // 绝不出现「qi 已扣但 ledger 未记账」的孤立路径。
        let Some(ref mut account) = qi_account else {
            continue;
        };

        let mut changed = false;
        let mut available_qi = cultivation
            .as_ref()
            .map(|cultivation| cultivation.qi_current)
            .unwrap_or(0.0);
        let qi_before = available_qi;

        // 解析玩家所在 zone（用于守恒 ledger）
        let zone_id = position
            .and_then(|pos| {
                zone_registry.as_deref().and_then(|zr| {
                    zr.find_zone(DimensionKind::Overworld, pos.0)
                        .map(|zone| QiAccountId::zone(zone.name.clone()))
                })
            })
            .unwrap_or_else(|| {
                QiAccountId::overflow(format!("artifact_maintenance_no_zone:{}", entity.to_bits()))
            });

        changed |= for_each_artifact_item_mut(&mut inventory, |item| {
            let Some(mut state) = artifact_state_from_item(item) else {
                return false;
            };
            let old_meridian = state.meridian.clone();
            state.meridian.self_heal_micro_cracks(clock.tick);
            let qi_snapshot_before = available_qi;
            let had_maintenance = state
                .meridian
                .apply_maintenance(&mut available_qi, clock.tick);
            if !had_maintenance {
                state.meridian.decay_without_maintenance(clock.tick);
            }

            // 守恒记账：每件法器单独 transfer，可溯源到具体法器。
            let cost = (qi_snapshot_before - available_qi).max(0.0);
            if cost > 0.0 {
                if !account.has_account(&zone_id) {
                    let _ = account.set_balance(zone_id.clone(), 0.0);
                }
                let zone_balance = account.balance(&zone_id);
                let _ = account.set_balance(zone_id.clone(), zone_balance + cost);
                let player_id = QiAccountId::player(format!("entity:{}", entity.to_bits()));
                account.push_transfer_audit(QiTransfer {
                    from: player_id,
                    to: zone_id.clone(),
                    amount: cost,
                    reason: QiTransferReason::ArtifactMaintenance,
                });
            }

            if state.meridian == old_meridian {
                return false;
            }
            write_artifact_state_to_item(item, &state);
            true
        });
        if let Some(mut cultivation) = cultivation {
            cultivation.qi_current = available_qi.clamp(0.0, cultivation.qi_max);
            // 守恒不变式验证（debug build）：已在 loop 内每法器粒度更新 zone ledger，
            // 总转移量 == qi_before - qi_after（扣减量全进 zone，无蒸发）。
            let _ = qi_before; // 已在 loop 内按法器粒度 credit，此处只用于 clamp。
        }
        if changed {
            bump_revision(&mut inventory);
        }
    }
}

pub fn artifact_tier_evolved_narration(
    mut events: EventReader<ArtifactTierEvolved>,
    mut pending_narrations: Option<ResMut<PendingGameplayNarrations>>,
) {
    let Some(pending_narrations) = pending_narrations.as_deref_mut() else {
        return;
    };
    for event in events.read() {
        if event.new_tier >= 2 {
            pending_narrations.push_broadcast(
                artifact_tier_narration(event.new_tier),
                NarrationStyle::Narration,
            );
        }
    }
}

fn artifact_tier_narration(new_tier: u8) -> &'static str {
    if new_tier >= 3 {
        "某处有法器道纹重开，灵光一闪而没。"
    } else {
        "某处有法器通灵之兆，寒光在天地间微微一亮。"
    }
}

fn for_each_artifact_item_mut(
    inventory: &mut PlayerInventory,
    mut f: impl FnMut(&mut ItemInstance) -> bool,
) -> bool {
    let mut changed = false;
    for container in &mut inventory.containers {
        for placed in &mut container.items {
            changed |= f(&mut placed.instance);
        }
    }
    for item in inventory
        .equipped
        .values_mut()
        .flat_map(|s| s.iter_all_mut())
    {
        changed |= f(item);
    }
    for item in inventory.hotbar.iter_mut().flatten() {
        changed |= f(item);
    }
    changed
}

fn find_item_by_instance(inventory: &PlayerInventory, instance_id: u64) -> Option<&ItemInstance> {
    for container in &inventory.containers {
        if let Some(placed) = container
            .items
            .iter()
            .find(|placed| placed.instance.instance_id == instance_id)
        {
            return Some(&placed.instance);
        }
    }
    for item in inventory.equipped.values().flat_map(|s| s.iter_all()) {
        if item.instance_id == instance_id {
            return Some(item);
        }
    }
    inventory
        .hotbar
        .iter()
        .flatten()
        .find(|item| item.instance_id == instance_id)
}

fn groove_id_for_index(idx: usize) -> String {
    match idx {
        0 => "primary".to_string(),
        1 => "secondary_1".to_string(),
        2 => "tertiary_2".to_string(),
        n => format!("auxiliary_{n}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::carrier::CarrierKind;
    use crate::inventory::ItemRarity;
    use valence::prelude::{App, Update};

    fn meridian(spec: MaterialSpec) -> ArtifactMeridian {
        ArtifactMeridian::new_from_spec(spec, 100.0, 0.8, 0, 0)
    }

    fn sample_weapon_item(side_effects: Vec<String>) -> ItemInstance {
        ItemInstance {
            instance_id: 7,
            template_id: "bone_sword".to_string(),
            display_name: "骨剑".to_string(),
            grid_w: 1,
            grid_h: 2,
            weight: 0.9,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: Some(0.9),
            forge_color: Some(ColorKind::Solid),
            forge_side_effects: side_effects,
            forge_achieved_tier: Some(1),
            alchemy: None,
            lingering_owner_qi: None,
        }
    }

    #[test]
    fn material_spec_all_6_carriers() {
        let specs = CarrierKind::ALL.map(material_spec_for_carrier);
        assert_eq!(specs.map(|spec| spec.grooves), [2, 3, 4, 5, 6, 8]);
        assert_eq!(
            specs.map(|spec| spec.depth_cap),
            [30.0, 60.0, 100.0, 150.0, 200.0, 300.0]
        );
    }

    #[test]
    fn mundane_weapon_single_shallow_groove() {
        let artifact = meridian(MaterialSpec::mundane());
        assert_eq!(artifact.grooves.len(), 1);
        assert_eq!(artifact.depth_cap, 10.0);
        assert_eq!(artifact.material_lock_coefficient, 0.01);
    }

    #[test]
    fn initial_depth_from_consecration() {
        let artifact = meridian(material_spec_for_carrier(CarrierKind::YibianShougu));
        assert!((artifact.grooves[0].depth - 1.0).abs() < 1e-9);
    }

    #[test]
    fn flow_capacity_by_tempering_quality() {
        let spec = material_spec_for_carrier(CarrierKind::LingmuArrow);
        let perfect = ArtifactMeridian::new_from_spec(spec, 0.0, 0.8, 0, 0);
        let good = ArtifactMeridian::new_from_spec(spec, 0.0, 0.5, 0, 0);
        let flawed = ArtifactMeridian::new_from_spec(spec, 0.0, 0.3, 0, 0);

        assert_eq!(perfect.grooves[0].flow_capacity, 80.0);
        assert_eq!(good.grooves[0].flow_capacity, 50.0);
        assert_eq!(flawed.grooves[0].flow_capacity, 30.0);
    }

    #[test]
    fn deepen_on_attack() {
        let mut artifact = ArtifactMeridian::new_from_spec(MaterialSpec::mundane(), 0.0, 0.8, 0, 0);
        let changes = artifact.deepen_on_flow(10.0, 20);

        assert_eq!(changes.len(), 1);
        assert!(artifact.total_depth > 0.0);
        assert_eq!(artifact.last_flow_tick, 20);
    }

    #[test]
    fn deepen_diminishing_returns() {
        let spec = MaterialSpec {
            grooves: 1,
            depth_cap: 100.0,
            lock_coeff: 0.1,
            max_quality_tier: 3,
        };
        let mut shallow = ArtifactMeridian::new_from_spec(spec, 0.0, 0.8, 0, 0);
        let mut deep = ArtifactMeridian::new_from_spec(spec, 0.0, 0.8, 0, 0);
        deep.grooves[0].depth = 90.0;
        deep.recompute_total_depth();

        shallow.deepen_on_flow(10.0, 1);
        deep.deepen_on_flow(10.0, 1);

        assert!(shallow.total_depth > deep.total_depth - 90.0);
    }

    #[test]
    fn mundane_deepen_extremely_slow() {
        let mut mundane = ArtifactMeridian::new_from_spec(MaterialSpec::mundane(), 0.0, 0.8, 0, 0);
        mundane.deepen_on_flow(100.0, 1);

        assert!(mundane.total_depth <= 1.0);
    }

    #[test]
    fn cracked_groove_excluded_from_flow() {
        let mut artifact = ArtifactMeridian::new_from_spec(
            material_spec_for_carrier(CarrierKind::YibianShougu),
            0.0,
            0.8,
            0,
            0,
        );
        artifact.grooves[0].crack_severity = 0.8;

        let changes = artifact.deepen_on_flow(30.0, 1);

        assert!(changes
            .iter()
            .all(|change| change.groove_id != artifact.grooves[0].id));
        assert_eq!(artifact.grooves[0].depth, 0.0);
    }

    #[test]
    fn overload_causes_crack() {
        let mut artifact = ArtifactMeridian::new_from_spec(MaterialSpec::mundane(), 0.0, 0.3, 0, 0);
        let change = artifact
            .apply_overload(100.0)
            .expect("overload should crack");

        assert!(change.new_severity > change.old_severity);
        assert!(artifact.overload_cracks > 0);
    }

    #[test]
    fn micro_crack_self_heals() {
        let mut artifact = ArtifactMeridian::new_from_spec(MaterialSpec::mundane(), 0.0, 0.8, 0, 0);
        artifact.grooves[0].crack_severity = 0.10;
        let healed = artifact.self_heal_micro_cracks(2 * TICKS_PER_HOUR);

        assert_eq!(healed, 1);
        assert_eq!(artifact.grooves[0].crack_severity, 0.0);
    }

    #[test]
    fn deep_crack_drops_quality_tier() {
        let mut artifact = ArtifactMeridian::new_from_spec(
            material_spec_for_carrier(CarrierKind::LingmuArrow),
            0.0,
            0.3,
            0,
            2,
        );
        artifact.grooves[0].crack_severity = 0.40;
        let change = artifact
            .apply_overload(1_000.0)
            .expect("deep overload should crack");

        assert!(change.new_severity >= DEEP_CRACK_LIMIT);
        assert_eq!(artifact.quality_tier, 1);
    }

    #[test]
    fn severed_groove_permanent() {
        let mut artifact = ArtifactMeridian::new_from_spec(MaterialSpec::mundane(), 0.0, 0.8, 0, 0);
        artifact.grooves[0].crack_severity = 0.8;
        assert_eq!(artifact.repair_at_station(1.0), 0);
        assert!(artifact.grooves[0].is_severed());
    }

    #[test]
    fn all_grooves_severed_triggers_weapon_broken_condition() {
        let mut artifact = ArtifactMeridian::new_from_spec(MaterialSpec::mundane(), 0.0, 0.8, 0, 0);
        artifact.grooves[0].crack_severity = 0.9;
        assert!(artifact.all_grooves_severed());
    }

    #[test]
    fn repair_at_station_reduces_crack() {
        let mut artifact = ArtifactMeridian::new_from_spec(MaterialSpec::mundane(), 0.0, 0.8, 0, 0);
        artifact.grooves[0].crack_severity = 0.4;
        assert_eq!(artifact.repair_at_station(0.15), 1);
        assert!((artifact.grooves[0].crack_severity - 0.25).abs() < 1e-9);
    }

    #[test]
    fn maintenance_drains_qi() {
        let mut artifact = ArtifactMeridian::new_from_spec(MaterialSpec::mundane(), 0.0, 0.8, 0, 1);
        let mut qi = 10.0;
        assert!(artifact.apply_maintenance(&mut qi, 99));
        assert_eq!(qi, 8.0);
        assert_eq!(artifact.last_maintenance_tick, 99);
    }

    #[test]
    fn no_maintenance_3days_drops_tier() {
        let mut artifact = ArtifactMeridian::new_from_spec(MaterialSpec::mundane(), 0.0, 0.8, 0, 1);
        assert!(artifact.decay_without_maintenance(3 * TICKS_PER_DAY));
        assert_eq!(artifact.quality_tier, 0);
    }

    #[test]
    fn no_maintenance_causes_shallowing() {
        let mut artifact =
            ArtifactMeridian::new_from_spec(MaterialSpec::mundane(), 100.0, 0.8, 0, 1);
        let old_depth = artifact.total_depth;

        assert!(artifact.decay_without_maintenance(3 * TICKS_PER_DAY));

        assert!(artifact.total_depth < old_depth);
    }

    #[test]
    fn evolution_mundane_to_magic() {
        let mut artifact = ArtifactMeridian::new_from_spec(
            material_spec_for_carrier(CarrierKind::YibianShougu),
            0.0,
            0.8,
            0,
            0,
        );
        for groove in &mut artifact.grooves {
            groove.depth = 20.0;
        }
        artifact.recompute_total_depth();
        artifact.usage_hours = 3.0;
        assert_eq!(artifact.try_evolve(0.5), Some((0, 1)));
    }

    #[test]
    fn evolution_costs_30pct_qi() {
        let mut cultivation = Cultivation {
            qi_current: 100.0,
            qi_max: 120.0,
            ..Default::default()
        };

        let cost = apply_evolution_qi_cost(&mut cultivation);

        assert_eq!(cost, 30.0);
        assert_eq!(cultivation.qi_current, 70.0);
    }

    #[test]
    fn evolution_to_lingqi_emits_broadcast_narration() {
        let mut app = App::new();
        app.add_event::<ArtifactTierEvolved>();
        app.insert_resource(PendingGameplayNarrations::default());
        app.add_systems(Update, artifact_tier_evolved_narration);
        let owner = app.world_mut().spawn_empty().id();

        app.world_mut().send_event(ArtifactTierEvolved {
            owner,
            instance_id: 7,
            old_tier: 1,
            new_tier: 2,
        });
        app.update();

        let mut narrations = app.world_mut().resource_mut::<PendingGameplayNarrations>();
        let drained = narrations.drain();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].style, NarrationStyle::Narration);
        assert!(drained[0].text.contains("法器通灵"));
    }

    #[test]
    fn evolution_blocked_by_material_cap() {
        let mut artifact = ArtifactMeridian::new_from_spec(MaterialSpec::mundane(), 0.0, 0.8, 0, 1);
        artifact.grooves[0].depth = 120.0;
        artifact.recompute_total_depth();
        artifact.usage_hours = 40.0;
        assert_eq!(artifact.try_evolve(1.0), None);
        assert_eq!(artifact.quality_tier, 1);
    }

    #[test]
    fn state_tag_roundtrip_preserves_artifact_state() {
        let mut item = sample_weapon_item(vec!["brittle_edge".to_string()]);
        let state =
            artifact_state_for_outcome("bone_sword", 1, 0.9, Some(ColorKind::Solid), 100.0, 0);
        write_artifact_state_to_item(&mut item, &state);

        let decoded = artifact_state_from_item(&item).expect("artifact state tag should decode");
        assert_eq!(decoded.meridian.grooves.len(), 3);
        assert!(item
            .forge_side_effects
            .iter()
            .any(|tag| tag == "brittle_edge"));
    }

    #[test]
    fn invalid_state_tag_is_ignored() {
        let item = sample_weapon_item(vec![format!("{ARTIFACT_STATE_PREFIX}not-json")]);

        assert!(
            artifact_state_from_item(&item).is_none(),
            "malformed artifact_state tag should not deserialize into an artifact"
        );
    }

    #[test]
    fn artifact_resonance_requires_holder_color() {
        let mut item = sample_weapon_item(Vec::new());
        let state =
            artifact_state_for_outcome("bone_sword", 1, 0.9, Some(ColorKind::Solid), 100.0, 0);
        write_artifact_state_to_item(&mut item, &state);

        assert_eq!(
            artifact_resonance_for_item(&item, None),
            None,
            "artifact resonance should not apply when holder QiColor is unavailable"
        );
        let solid_holder = QiColor {
            main: ColorKind::Solid,
            ..Default::default()
        };
        let heavy_holder = QiColor {
            main: ColorKind::Heavy,
            ..Default::default()
        };
        let matching = artifact_resonance_for_item(&item, Some(&solid_holder))
            .expect("matching holder color should compute resonance");
        let mismatched = artifact_resonance_for_item(&item, Some(&heavy_holder))
            .expect("mismatched holder color still computes a lower resonance");

        assert!(
            matching > mismatched,
            "matching holder color should resonate more than mismatched color: matching={matching}, mismatched={mismatched}"
        );
    }

    // ── plan-qi-conservation-leaks-v1 P2 — 养护/进化守恒系统测试 ─────────────────

    /// 构造一个含单件法器（quality_tier=1, grooves=1）的 PlayerInventory，放入单 container。
    fn inventory_with_artifact(artifact_quality_tier: u8) -> PlayerInventory {
        use crate::inventory::{ContainerState, InventoryRevision, ItemRarity, PlacedItemState};
        let mut item = ItemInstance {
            instance_id: 99,
            template_id: "bone_sword".to_string(),
            display_name: "骨剑".to_string(),
            grid_w: 1,
            grid_h: 2,
            weight: 0.9,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: Some(0.9),
            forge_color: Some(ColorKind::Solid),
            forge_side_effects: Vec::new(),
            forge_achieved_tier: Some(artifact_quality_tier),
            alchemy: None,
            lingering_owner_qi: None,
        };
        // quality_tier=1, mundane spec → 1 groove, cost = 1*2*1 = 2.0 per day
        let state = ArtifactState {
            meridian: ArtifactMeridian::new_from_spec(
                MaterialSpec::mundane(),
                100.0,
                0.8,
                0,
                artifact_quality_tier,
            ),
            color: ArtifactColor::default(),
        };
        write_artifact_state_to_item(&mut item, &state);
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(1),
            containers: vec![ContainerState {
                quick_access: false,
                id: "main_pack".to_string(),
                name: "main".to_string(),
                rows: 1,
                cols: 1,
                items: vec![PlacedItemState {
                    row: 0,
                    col: 0,
                    instance: item,
                }],

                owner_instance_id: None,
            }],
            equipped: std::collections::HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 45.0,
        }
    }

    #[test]
    fn maintenance_tick_conserves_qi_to_zone() {
        // 期望：养护扣减 cultivation.qi_current 量 == 进 zone ledger 量（守恒）。
        // 守恒断言：player 减少量 == zone 增加量。
        // artifact: quality_tier=1, mundane(1 groove) → cost = 1*2*1 = 2.0
        use crate::combat::CombatClock;
        use crate::qi_physics::ledger::{QiAccountId, QiTransferReason, WorldQiAccount};
        use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};
        use valence::math::DVec3;
        use valence::prelude::{App, Update};

        let mut app = App::new();
        let clock_tick = TICKS_PER_DAY; // 正好触发一次养护
        app.insert_resource(CombatClock { tick: clock_tick });
        app.insert_resource(WorldQiAccount::default());
        app.insert_resource(ZoneRegistry::fallback());
        app.add_systems(Update, artifact_meridian_maintenance_tick);

        // 玩家在 spawn zone 内（fallback zone bounds 约为 [-128,128]）
        let pos = Position(DVec3::new(0.0, 66.0, 0.0));
        let qi_before = 50.0_f64;
        let player = app
            .world_mut()
            .spawn((
                inventory_with_artifact(1),
                Cultivation {
                    qi_current: qi_before,
                    qi_max: 100.0,
                    ..Default::default()
                },
                pos,
            ))
            .id();

        app.update();

        let qi_after = app
            .world()
            .entity(player)
            .get::<Cultivation>()
            .unwrap()
            .qi_current;

        // 养护 cost = quality_tier=1 * 2 * grooves=1 = 2.0
        let expected_cost = 2.0_f64;
        assert!(
            (qi_after - (qi_before - expected_cost)).abs() < 1e-9,
            "期望养护扣 {expected_cost} 后 qi_current={}, 实际={qi_after}（守恒：扣减量应进 zone）",
            qi_before - expected_cost
        );

        let account = app.world().resource::<WorldQiAccount>();
        let zone_id = QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME.to_string());
        let zone_balance = account.balance(&zone_id);
        let player_delta = qi_before - qi_after;

        assert!(
            (player_delta - zone_balance).abs() < 1e-9,
            "守恒失败：player 减少 {player_delta}，zone 增加 {zone_balance}（二者应相等，真元不蒸发）"
        );
        assert!(
            zone_balance > 0.0,
            "期望 zone balance > 0（养护已归入 zone），实际={zone_balance}"
        );

        let transfers = account.transfers();
        assert!(
            !transfers.is_empty(),
            "期望至少一条 ArtifactMaintenance 审计记录，实际 transfers 为空"
        );
        let maintenance_transfer = transfers
            .iter()
            .find(|t| t.reason == QiTransferReason::ArtifactMaintenance);
        assert!(
            maintenance_transfer.is_some(),
            "期望审计记录中包含 ArtifactMaintenance reason，实际={:?}",
            transfers.iter().map(|t| t.reason).collect::<Vec<_>>()
        );
        let mt = maintenance_transfer.unwrap();
        assert!(
            (mt.amount - expected_cost).abs() < 1e-9,
            "期望 ArtifactMaintenance transfer.amount={expected_cost}，实际={}",
            mt.amount
        );
    }

    #[test]
    fn maintenance_tick_no_qi_account_skips_deduction() {
        // 期望：WorldQiAccount 缺失时，qi_current 不变（守恒保卫：无 ledger 就不扣减）。
        use crate::combat::CombatClock;
        use valence::math::DVec3;
        use valence::prelude::{App, Update};

        let mut app = App::new();
        app.insert_resource(CombatClock {
            tick: TICKS_PER_DAY,
        });
        // 故意不插入 WorldQiAccount
        app.insert_resource(ZoneRegistry::fallback());
        app.add_systems(Update, artifact_meridian_maintenance_tick);

        let pos = Position(DVec3::new(0.0, 66.0, 0.0));
        let qi_before = 50.0_f64;
        let player = app
            .world_mut()
            .spawn((
                inventory_with_artifact(1),
                Cultivation {
                    qi_current: qi_before,
                    qi_max: 100.0,
                    ..Default::default()
                },
                pos,
            ))
            .id();

        app.update();

        let qi_after = app
            .world()
            .entity(player)
            .get::<Cultivation>()
            .unwrap()
            .qi_current;

        assert!(
            (qi_after - qi_before).abs() < 1e-9,
            "期望 WorldQiAccount 缺失时 qi_current 不变={qi_before}（守恒保卫），实际={qi_after}"
        );
    }

    #[test]
    fn maintenance_tick_no_zone_falls_back_to_overflow() {
        // 期望：玩家不在任何 zone（超出边界）→ 守恒记入 overflow，真元绝不蒸发。
        use crate::combat::CombatClock;
        use crate::qi_physics::ledger::{QiAccountKind, WorldQiAccount};
        use valence::math::DVec3;
        use valence::prelude::{App, Update};

        let mut app = App::new();
        app.insert_resource(CombatClock {
            tick: TICKS_PER_DAY,
        });
        app.insert_resource(WorldQiAccount::default());
        // fallback zone bounds ~[-128,128]；玩家放到超出范围的位置
        app.insert_resource(ZoneRegistry::fallback());
        app.add_systems(Update, artifact_meridian_maintenance_tick);

        let pos = Position(DVec3::new(1_000_000.0, 66.0, 1_000_000.0));
        let qi_before = 50.0_f64;
        let player = app
            .world_mut()
            .spawn((
                inventory_with_artifact(1),
                Cultivation {
                    qi_current: qi_before,
                    qi_max: 100.0,
                    ..Default::default()
                },
                pos,
            ))
            .id();

        app.update();

        let qi_after = app
            .world()
            .entity(player)
            .get::<Cultivation>()
            .unwrap()
            .qi_current;

        let player_delta = qi_before - qi_after;
        let account = app.world().resource::<WorldQiAccount>();
        let transfers = account.transfers();

        assert!(
            player_delta > 0.0,
            "期望 qi 被扣减（zone 不可解析不影响扣减），实际={qi_after}"
        );
        assert!(
            !transfers.is_empty(),
            "期望 zone 不可解析时仍有 overflow 审计记录，实际 transfers 为空"
        );
        let overflow_transfer = transfers
            .iter()
            .find(|t| t.to.kind == QiAccountKind::Overflow);
        assert!(
            overflow_transfer.is_some(),
            "期望 to.kind=Overflow（true 不蒸发），实际={:?}",
            transfers.iter().map(|t| &t.to).collect::<Vec<_>>()
        );
        // 守恒：overflow 账户余额 == 玩家减少量
        let overflow_balance = account.balance(&overflow_transfer.unwrap().to);
        assert!(
            (overflow_balance - player_delta).abs() < 1e-9,
            "期望 overflow_balance={player_delta}（== 玩家减少量，守恒），实际={overflow_balance}"
        );
    }

    /// 构造用于进化系统测试的 CombatEvent（走法器铭纹路径），damage=50.0 触发 flow。
    fn sample_combat_event(
        attacker: valence::prelude::Entity,
    ) -> crate::combat::events::CombatEvent {
        use crate::combat::components::{BodyPart, WoundKind};
        use crate::combat::events::{AttackSource, CombatEvent};
        CombatEvent {
            attacker,
            target: attacker,
            resolved_at_tick: 1,
            body_part: BodyPart::Chest,
            wound_kind: WoundKind::Cut,
            source: AttackSource::Melee,
            debug_command: false,
            physical_damage: 0.0,
            damage: 50.0,
            contam_delta: 0.0,
            description: String::new(),
            defense_kind: None,
            defense_effectiveness: None,
            defense_contam_reduced: None,
            defense_wound_severity: None,
        }
    }

    /// 构造一件处于进化临界的法器 inventory（2槽 depth_cap=30, depth=20/20, usage_hours=3.5, Solid）。
    ///
    /// 为什么不用 mundane()：mundane() 单槽 depth_cap=10，把手动设的 depth=20 经由
    /// deepen_on_flow 的 clamp(0, depth_cap) 钳回 10 → avg=10 < 15 → try_evolve 不触发。
    /// 改用 grooves=2/depth_cap=30：20<=30 不被钳，avg=40/2=20>=15；maturity=40/60≈0.667；
    /// color_match(Solid/Solid)=1.0 → resonance≈0.667>=0.4；usage_hours>=3.0；
    /// max_quality_tier=1>=1 → 进化 0→1 必然触发。
    fn inventory_with_pre_evolve_artifact(instance_id: u64) -> PlayerInventory {
        use crate::inventory::{ContainerState, InventoryRevision, PlacedItemState};
        let mut item = sample_weapon_item(Vec::new());
        item.instance_id = instance_id;
        let spec = MaterialSpec {
            grooves: 2,
            depth_cap: 30.0,
            lock_coeff: 0.03,
            max_quality_tier: 1,
        };
        let mut meridian = ArtifactMeridian::new_from_spec(spec, 0.0, 0.8, 0, 0);
        // 两槽各设 20.0，均在 cap=30 内，avg=20>=15 满足进化阈值。
        meridian.grooves[0].depth = 20.0;
        meridian.grooves[1].depth = 20.0;
        meridian.recompute_total_depth();
        meridian.usage_hours = 3.5;
        let state = ArtifactState {
            meridian,
            color: ArtifactColor::from_initial_color(ColorKind::Solid, 10.0),
        };
        write_artifact_state_to_item(&mut item, &state);
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(1),
            containers: vec![ContainerState {
                quick_access: false,
                id: "main_pack".to_string(),
                name: "main".to_string(),
                rows: 1,
                cols: 1,
                items: vec![PlacedItemState {
                    row: 0,
                    col: 0,
                    instance: item,
                }],

                owner_instance_id: None,
            }],
            equipped: std::collections::HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 45.0,
        }
    }

    /// 构造 Weapon 组件（满血）。
    fn sample_weapon(instance_id: u64) -> crate::combat::weapon::Weapon {
        use crate::combat::weapon::{EquipSlot, Weapon, WeaponKind};
        Weapon {
            slot: EquipSlot::MainHand,
            instance_id,
            template_id: "bone_sword".to_string(),
            weapon_kind: WeaponKind::Sword,
            base_attack: 10.0,
            quality_tier: 0,
            durability: 100.0,
            durability_max: 100.0,
        }
    }

    #[test]
    fn evolution_conserves_qi_to_zone() {
        // 期望：进化触发时 qi_current*0.3 扣减量 == 进 zone ledger 量（守恒）。
        // 用 artifact_meridian_deepen_on_use 系统 + CombatEvent 触发进化路径。
        use crate::combat::events::CombatEvent;
        use crate::combat::CombatClock;
        use crate::qi_physics::ledger::{QiAccountId, QiTransferReason, WorldQiAccount};
        use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};
        use valence::math::DVec3;
        use valence::prelude::{App, Update};

        let mut app = App::new();
        app.add_event::<CombatEvent>();
        app.add_event::<ArtifactMeridianDepthChanged>();
        app.add_event::<ArtifactMeridianCracked>();
        app.add_event::<ArtifactTierEvolved>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.insert_resource(CombatClock { tick: 1 });
        app.insert_resource(WorldQiAccount::default());
        app.insert_resource(ZoneRegistry::fallback());
        app.add_systems(Update, artifact_meridian_deepen_on_use);

        let instance_id = 55u64;
        let qi_before = 100.0_f64;
        let attacker = app
            .world_mut()
            .spawn((
                sample_weapon(instance_id),
                inventory_with_pre_evolve_artifact(instance_id),
                Cultivation {
                    qi_current: qi_before,
                    qi_max: 120.0,
                    ..Default::default()
                },
                QiColor {
                    main: ColorKind::Solid,
                    ..Default::default()
                },
                Position(DVec3::new(0.0, 66.0, 0.0)),
            ))
            .id();

        // CombatEvent 触发进化路径（damage 足够触发 try_evolve 条件）
        app.world_mut().send_event(sample_combat_event(attacker));
        app.update();

        let qi_after = app
            .world()
            .entity(attacker)
            .get::<Cultivation>()
            .unwrap()
            .qi_current;

        let evolved = (qi_before - qi_after) > 0.0;
        // 进化必须触发：fixture 已保证 avg=20>=15 / resonance>=0.4 / usage_hours>=3.0。
        // 若此断言失败，说明 fixture 条件又被破坏，守恒断言会变成空跑。
        assert!(
            evolved,
            "进化必须触发（avg>=15, resonance>=0.4, usage_hours>=3.0），\
             但 qi_before={qi_before} qi_after={qi_after}；检查 fixture depth/resonance/usage_hours"
        );
        if evolved {
            let expected_cost = qi_before * 0.3;
            assert!(
                (qi_after - (qi_before - expected_cost)).abs() < 1e-9,
                "期望进化扣 {expected_cost}（qi*0.3），qi_after={}，实际={qi_after}",
                qi_before - expected_cost
            );

            let account = app.world().resource::<WorldQiAccount>();
            let zone_id = QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME.to_string());
            let zone_balance = account.balance(&zone_id);
            let player_delta = qi_before - qi_after;

            assert!(
                (player_delta - zone_balance).abs() < 1e-9,
                "守恒失败：player 减少 {player_delta}，zone 增加 {zone_balance}（二者应相等，进化真元不蒸发）"
            );

            let transfers = account.transfers();
            let evo_transfer = transfers
                .iter()
                .find(|t| t.reason == QiTransferReason::ArtifactEvolution);
            assert!(
                evo_transfer.is_some(),
                "期望审计记录含 ArtifactEvolution reason，实际={:?}",
                transfers.iter().map(|t| t.reason).collect::<Vec<_>>()
            );
        }
        // 无论进化是否触发，守恒不变式：player 减少量 == zone 增加量（零也是守恒）
        let account = app.world().resource::<WorldQiAccount>();
        let zone_id = QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME.to_string());
        let zone_balance = account.balance(&zone_id);
        let player_delta = qi_before - qi_after;
        assert!(
            (player_delta - zone_balance).abs() < 1e-9,
            "守恒不变式失败：player_delta={player_delta}，zone_balance={zone_balance}"
        );
    }

    #[test]
    fn evolution_no_qi_account_skips_deduction() {
        // 期望：WorldQiAccount 缺失时，进化路径不扣 qi_current（守恒保卫）。
        use crate::combat::events::CombatEvent;
        use crate::combat::CombatClock;
        use valence::math::DVec3;
        use valence::prelude::{App, Update};

        let mut app = App::new();
        app.add_event::<CombatEvent>();
        app.add_event::<ArtifactMeridianDepthChanged>();
        app.add_event::<ArtifactMeridianCracked>();
        app.add_event::<ArtifactTierEvolved>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.insert_resource(CombatClock { tick: 1 });
        // 故意不插入 WorldQiAccount
        app.insert_resource(ZoneRegistry::fallback());
        app.add_systems(Update, artifact_meridian_deepen_on_use);

        let instance_id = 77u64;
        let qi_before = 100.0_f64;
        let attacker = app
            .world_mut()
            .spawn((
                sample_weapon(instance_id),
                inventory_with_pre_evolve_artifact(instance_id),
                Cultivation {
                    qi_current: qi_before,
                    qi_max: 120.0,
                    ..Default::default()
                },
                QiColor {
                    main: ColorKind::Solid,
                    ..Default::default()
                },
                Position(DVec3::new(0.0, 66.0, 0.0)),
            ))
            .id();

        app.world_mut().send_event(sample_combat_event(attacker));
        app.update();

        let qi_after = app
            .world()
            .entity(attacker)
            .get::<Cultivation>()
            .unwrap()
            .qi_current;

        // 进化必须已触发（通过 ArtifactTierEvolved 事件核实），否则下面的"qi不变"断言等价空跑。
        let evolved_events = app
            .world()
            .resource::<bevy_ecs::event::Events<ArtifactTierEvolved>>();
        let mut reader = evolved_events.get_reader();
        let evolved_fired = reader.read(evolved_events).count() > 0;
        assert!(
            evolved_fired,
            "进化必须触发（avg>=15, resonance>=0.4, usage_hours>=3.0），\
             否则「qi不变」断言与进化路径无关；检查 fixture depth/resonance/usage_hours"
        );

        // 核心断言：WorldQiAccount 缺失时进化不扣 qi（守恒保卫：无账本就无扣减）。
        assert!(
            (qi_after - qi_before).abs() < 1e-9,
            "期望 WorldQiAccount 缺失时 qi_current 不变={qi_before}（守恒保卫），实际={qi_after}"
        );
    }

    #[test]
    fn evolution_no_zone_falls_back_to_overflow() {
        // 期望：entity 在 zone 外触发进化时，qi 守恒路径 fallback 到 overflow 账本而非蒸发。
        // 覆盖 artifact_meridian_deepen_on_use 648-653 的 overflow 分支。
        use crate::combat::events::CombatEvent;
        use crate::combat::CombatClock;
        use crate::qi_physics::ledger::{QiTransferReason, WorldQiAccount};
        use crate::world::zone::ZoneRegistry;
        use valence::math::DVec3;
        use valence::prelude::{App, Update};

        let mut app = App::new();
        app.add_event::<CombatEvent>();
        app.add_event::<ArtifactMeridianDepthChanged>();
        app.add_event::<ArtifactMeridianCracked>();
        app.add_event::<ArtifactTierEvolved>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.insert_resource(CombatClock { tick: 1 });
        app.insert_resource(WorldQiAccount::default());
        // ZoneRegistry::fallback() 只包含 spawn zone [-128..128, 64..80, -128..128]。
        // entity 放在 (200, 66, 200) 使 find_zone 返回 None → overflow 分支触发。
        app.insert_resource(ZoneRegistry::fallback());
        app.add_systems(Update, artifact_meridian_deepen_on_use);

        let instance_id = 99u64;
        let qi_before = 100.0_f64;
        let attacker = app
            .world_mut()
            .spawn((
                sample_weapon(instance_id),
                inventory_with_pre_evolve_artifact(instance_id),
                Cultivation {
                    qi_current: qi_before,
                    qi_max: 120.0,
                    ..Default::default()
                },
                QiColor {
                    main: ColorKind::Solid,
                    ..Default::default()
                },
                // 故意放在 spawn zone 之外（x=200 > 128）
                Position(DVec3::new(200.0, 66.0, 200.0)),
            ))
            .id();

        app.world_mut().send_event(sample_combat_event(attacker));
        app.update();

        let qi_after = app
            .world()
            .entity(attacker)
            .get::<Cultivation>()
            .unwrap()
            .qi_current;

        // 进化必须触发
        let evolved_events = app
            .world()
            .resource::<bevy_ecs::event::Events<ArtifactTierEvolved>>();
        let mut evo_reader = evolved_events.get_reader();
        let evolved_fired = evo_reader.read(evolved_events).count() > 0;
        assert!(
            evolved_fired,
            "进化必须触发（avg>=15, resonance>=0.4, usage_hours>=3.0），\
             否则 overflow 分支断言无意义；检查 fixture depth/resonance/usage_hours"
        );

        // 守恒不变式：player 减少量必须进入某个 overflow 账本，总量守恒
        let player_delta = qi_before - qi_after;
        assert!(
            player_delta > 0.0,
            "进化触发后 qi_current 应减少（cost=qi*0.3），实际 qi_before={qi_before} qi_after={qi_after}"
        );

        let account = app.world().resource::<WorldQiAccount>();

        // overflow 账本键以 "artifact_evolution_no_zone:" 开头
        let overflow_balance: f64 = account
            .iter_balances()
            .filter(|(id, _)| {
                id.kind == crate::qi_physics::ledger::QiAccountKind::Overflow
                    && id.id.starts_with("artifact_evolution_no_zone:")
            })
            .map(|(_, b)| b)
            .sum();
        assert!(
            (player_delta - overflow_balance).abs() < 1e-9,
            "守恒失败：player 减少 {player_delta}，overflow 账本合计 {overflow_balance}（应相等，进化真元不蒸发）"
        );

        // 审计记录应包含 ArtifactEvolution reason
        let evo_transfer = account
            .transfers()
            .iter()
            .find(|t| t.reason == QiTransferReason::ArtifactEvolution);
        assert!(
            evo_transfer.is_some(),
            "期望审计记录含 ArtifactEvolution reason，实际={:?}",
            account
                .transfers()
                .iter()
                .map(|t| t.reason)
                .collect::<Vec<_>>()
        );
    }
}
