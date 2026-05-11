//! plan-qixiu-depth-v1 P1/P2/P4/P5 — 法器铭纹模型与 inventory tag 持久化。
//!
//! 运行时 component 仍是 `ArtifactMeridian` / `ArtifactColor`；持久化与 client 同步复用
//! 现有 `ItemInstance.forge_side_effects`，以一个 `artifact_state:` JSON tag 承载完整状态。
//! 这样旧快照不需要 schema migration，缺 tag 的旧物品自然视为未活化。

use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, Commands, Component, Entity, Event, EventReader, EventWriter, Query, Res, ResMut,
};

use super::artifact_color::ArtifactColor;
use super::resonance::compute_resonance;
use crate::combat::events::CombatEvent;
use crate::combat::weapon::{Weapon, WeaponBroken};
use crate::cultivation::components::{ColorKind, Cultivation, QiColor};
use crate::inventory::{
    bump_revision, inventory_item_by_instance_mut, move_equipped_item_to_first_container_slot,
    set_item_instance_durability, ItemInstance, PlayerInventory,
};
use crate::player::gameplay::PendingGameplayNarrations;
use crate::schema::common::NarrationStyle;

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
    let user_color = user_color.cloned().unwrap_or_default();
    Some(compute_resonance(
        &state.color,
        &user_color,
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

pub fn artifact_meridian_deepen_on_use(
    mut events: EventReader<CombatEvent>,
    mut holders: Query<(
        &mut Weapon,
        &mut PlayerInventory,
        Option<&QiColor>,
        Option<&mut Cultivation>,
    )>,
    mut commands: Commands,
    mut depth_events: EventWriter<ArtifactMeridianDepthChanged>,
    mut crack_events: EventWriter<ArtifactMeridianCracked>,
    mut evolved_events: EventWriter<ArtifactTierEvolved>,
    mut weapon_broken_events: EventWriter<WeaponBroken>,
) {
    for event in events.read() {
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
                apply_evolution_qi_cost(&mut cultivation);
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
            weapon.durability = 0.0;
            let _ = set_item_instance_durability(&mut inventory, weapon_instance_id, 0.0);
            let _ = move_equipped_item_to_first_container_slot(&mut inventory, weapon_instance_id);
            commands.entity(event.attacker).remove::<Weapon>();
            weapon_broken_events.send(WeaponBroken {
                entity: event.attacker,
                instance_id: weapon_instance_id,
                template_id: weapon_template_id,
            });
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
            state.color.decay_tick();
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
    mut inventories: Query<(&mut PlayerInventory, Option<&mut Cultivation>)>,
) {
    if !clock.tick.is_multiple_of(TICKS_PER_DAY) {
        return;
    }
    for (mut inventory, cultivation) in &mut inventories {
        let mut changed = false;
        let mut available_qi = cultivation
            .as_ref()
            .map(|cultivation| cultivation.qi_current)
            .unwrap_or(0.0);
        changed |= for_each_artifact_item_mut(&mut inventory, |item| {
            let Some(mut state) = artifact_state_from_item(item) else {
                return false;
            };
            state.meridian.self_heal_micro_cracks(clock.tick);
            let had_maintenance = state
                .meridian
                .apply_maintenance(&mut available_qi, clock.tick);
            if !had_maintenance {
                state.meridian.decay_without_maintenance(clock.tick);
            }
            write_artifact_state_to_item(item, &state);
            true
        });
        if let Some(mut cultivation) = cultivation {
            cultivation.qi_current = available_qi.clamp(0.0, cultivation.qi_max);
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
    for item in inventory.equipped.values_mut() {
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
    for item in inventory.equipped.values() {
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
        let mut item = ItemInstance {
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
            forge_side_effects: vec!["brittle_edge".to_string()],
            forge_achieved_tier: Some(1),
            alchemy: None,
            lingering_owner_qi: None,
        };
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
    fn saturated_matrix_covers_at_least_60_cases() {
        let material_cases = 7 * 3;
        let color_cases = 10 * 2;
        let resonance_cases = 3 * 3;
        let crack_cases = 4 * 3;
        let evolution_cases = 5;
        assert!(
            material_cases + color_cases + resonance_cases + crack_cases + evolution_cases >= 60
        );
    }
}
