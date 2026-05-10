use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component, Entity};

use crate::cultivation::components::{ColorKind, Realm};
use crate::qi_physics::StyleDefense;

pub const FALSE_SKIN_FAN_ITEM_ID: &str = "tuike_false_skin_fan";
pub const FALSE_SKIN_LIGHT_ITEM_ID: &str = "tuike_false_skin_light";
pub const FALSE_SKIN_MID_ITEM_ID: &str = "tuike_false_skin_mid";
pub const FALSE_SKIN_HEAVY_ITEM_ID: &str = "tuike_false_skin_heavy";
pub const FALSE_SKIN_ANCIENT_ITEM_ID: &str = "tuike_false_skin_ancient";
pub const FALSE_SKIN_ANCIENT_RELIC_SHARD_ITEM_ID: &str = "ancient_false_skin_shard";
pub const FALSE_SKIN_ASH_ITEM_ID: &str = "tuike_false_skin_ash";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FalseSkinTier {
    Fan,
    Light,
    Mid,
    Heavy,
    Ancient,
}

impl FalseSkinTier {
    #[allow(dead_code)]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fan => "fan",
            Self::Light => "light",
            Self::Mid => "mid",
            Self::Heavy => "heavy",
            Self::Ancient => "ancient",
        }
    }

    pub const fn material_factor(self) -> f64 {
        match self {
            Self::Fan => 0.2,
            Self::Light => 0.5,
            Self::Mid => 1.5,
            Self::Heavy => 4.0,
            Self::Ancient => 10.0,
        }
    }

    #[allow(dead_code)]
    pub const fn baseline_damage_capacity(self) -> f64 {
        self.material_factor() * 100.0
    }

    pub const fn maintain_qi_per_sec(self) -> f64 {
        match self {
            Self::Fan => 0.1,
            Self::Light => 0.2,
            Self::Mid => 0.3,
            Self::Heavy => 0.5,
            Self::Ancient => 1.0,
        }
    }

    pub const fn min_realm(self) -> Realm {
        match self {
            Self::Fan => Realm::Awaken,
            Self::Light => Realm::Condense,
            Self::Mid => Realm::Solidify,
            Self::Heavy => Realm::Spirit,
            Self::Ancient => Realm::Void,
        }
    }

    #[allow(dead_code)]
    pub const fn item_id(self) -> &'static str {
        match self {
            Self::Fan => FALSE_SKIN_FAN_ITEM_ID,
            Self::Light => FALSE_SKIN_LIGHT_ITEM_ID,
            Self::Mid => FALSE_SKIN_MID_ITEM_ID,
            Self::Heavy => FALSE_SKIN_HEAVY_ITEM_ID,
            Self::Ancient => FALSE_SKIN_ANCIENT_ITEM_ID,
        }
    }

    pub const fn residue_output_item_id(self) -> &'static str {
        match self {
            Self::Ancient => FALSE_SKIN_ANCIENT_RELIC_SHARD_ITEM_ID,
            _ => FALSE_SKIN_ASH_ITEM_ID,
        }
    }
}

impl StyleDefense for FalseSkinTier {
    fn defense_color(&self) -> ColorKind {
        ColorKind::Solid
    }

    fn resistance(&self) -> f64 {
        (self.material_factor() / FalseSkinTier::Ancient.material_factor()).clamp(0.0, 1.0)
    }

    fn drain_affinity(&self) -> f64 {
        match self {
            Self::Fan => 0.35,
            Self::Light => 0.30,
            Self::Mid => 0.20,
            Self::Heavy => 0.12,
            Self::Ancient => 0.05,
        }
    }
}

pub fn false_skin_tier_for_item(template_id: &str) -> Option<FalseSkinTier> {
    match template_id {
        FALSE_SKIN_FAN_ITEM_ID | crate::combat::tuike::SPIDER_SILK_FALSE_SKIN_ITEM_ID => {
            Some(FalseSkinTier::Fan)
        }
        FALSE_SKIN_LIGHT_ITEM_ID => Some(FalseSkinTier::Light),
        FALSE_SKIN_MID_ITEM_ID | crate::combat::tuike::ROTTEN_WOOD_ARMOR_ITEM_ID => {
            Some(FalseSkinTier::Mid)
        }
        FALSE_SKIN_HEAVY_ITEM_ID => Some(FalseSkinTier::Heavy),
        FALSE_SKIN_ANCIENT_ITEM_ID => Some(FalseSkinTier::Ancient),
        _ => None,
    }
}

#[derive(Debug, Clone, Component, Default, PartialEq, Serialize, Deserialize)]
pub struct StackedFalseSkins {
    pub layers: Vec<FalseSkinLayer>,
    pub naked_until_tick: u64,
    pub transfer_permanent_cooldown_until_tick: u64,
}

impl StackedFalseSkins {
    pub fn with_layer(layer: FalseSkinLayer) -> Self {
        Self {
            layers: vec![layer],
            ..Default::default()
        }
    }

    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    pub fn outer(&self) -> Option<&FalseSkinLayer> {
        self.layers.last()
    }

    pub fn outer_mut(&mut self) -> Option<&mut FalseSkinLayer> {
        self.layers.last_mut()
    }

    pub fn push_outer(&mut self, layer: FalseSkinLayer, max_layers: usize) -> bool {
        if self.layers.len() >= max_layers.max(1) {
            return false;
        }
        self.layers.push(layer);
        true
    }

    pub fn shed_outer(&mut self, now_tick: u64) -> Option<FalseSkinLayer> {
        let layer = self.layers.pop()?;
        if self.layers.is_empty() {
            self.naked_until_tick =
                now_tick.saturating_add(5 * crate::combat::components::TICKS_PER_SECOND);
        }
        Some(layer)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FalseSkinLayer {
    pub instance_id: u64,
    pub tier: FalseSkinTier,
    pub spirit_quality: f64,
    pub damage_taken: f64,
    pub contam_load: f64,
    pub permanent_taint_load: f64,
    pub equipped_at_tick: u64,
}

impl FalseSkinLayer {
    pub fn new(instance_id: u64, tier: FalseSkinTier, spirit_quality: f64, tick: u64) -> Self {
        Self {
            instance_id,
            tier,
            spirit_quality: spirit_quality.clamp(0.1, 10.0),
            damage_taken: 0.0,
            contam_load: 0.0,
            permanent_taint_load: 0.0,
            equipped_at_tick: tick,
        }
    }

    #[allow(dead_code)]
    pub fn damage_capacity(&self) -> f64 {
        self.tier.baseline_damage_capacity() * self.spirit_quality
    }

    pub fn contam_capacity_percent(&self) -> f64 {
        100.0
    }

    #[allow(dead_code)]
    pub fn remaining_damage_capacity(&self) -> f64 {
        (self.damage_capacity() - self.damage_taken).max(0.0)
    }

    pub fn remaining_contam_capacity_percent(&self) -> f64 {
        (self.contam_capacity_percent() - self.contam_load).max(0.0)
    }
}

#[derive(Debug, Clone, Copy, Component, PartialEq, Serialize, Deserialize)]
pub struct WornFalseSkin {
    pub instance_id: u64,
    pub tier: FalseSkinTier,
    pub spirit_quality: f64,
    pub contam_load: f64,
    pub permanent_taint_load: f64,
}

impl From<&FalseSkinLayer> for WornFalseSkin {
    fn from(layer: &FalseSkinLayer) -> Self {
        Self {
            instance_id: layer.instance_id,
            tier: layer.tier,
            spirit_quality: layer.spirit_quality,
            contam_load: layer.contam_load,
            permanent_taint_load: layer.permanent_taint_load,
        }
    }
}

#[derive(Debug, Clone, Component, PartialEq, Serialize, Deserialize)]
pub struct FalseSkinResidue {
    pub owner: Entity,
    pub tier: FalseSkinTier,
    pub contam_load: f64,
    pub permanent_taint_load: f64,
    pub dropped_at_tick: u64,
    pub decay_at_tick: u64,
    pub picked_up: bool,
}

#[derive(Debug, Clone, Component, PartialEq, Serialize, Deserialize)]
pub struct PermanentQiMaxDecay {
    pub source: Entity,
    pub amount: f64,
    pub applied_at_tick: u64,
}
