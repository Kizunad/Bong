use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Entity, Event};

use super::state::FalseSkinTier;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TuikeSkillId {
    Don,
    Shed,
    TransferTaint,
}

impl TuikeSkillId {
    #[allow(dead_code)]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Don => "tuike.don",
            Self::Shed => "tuike.shed",
            Self::TransferTaint => "tuike.transfer_taint",
        }
    }

    pub const fn payload_kind(self) -> &'static str {
        match self {
            Self::Don => "don",
            Self::Shed => "shed",
            Self::TransferTaint => "transfer_taint",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TuikeSkillVisual {
    pub animation_id: &'static str,
    pub particle_id: &'static str,
    pub sound_recipe_id: &'static str,
    pub icon_texture: &'static str,
}

impl TuikeSkillVisual {
    pub const fn for_skill(skill: TuikeSkillId, ancient: bool) -> Self {
        match skill {
            TuikeSkillId::Don => Self {
                animation_id: "bong:tuike_don_skin",
                particle_id: "bong:false_skin_don_dust",
                sound_recipe_id: "don_skin_low_thud",
                icon_texture: "bong-client:textures/gui/skill/tuike_don.png",
            },
            TuikeSkillId::Shed => Self {
                animation_id: "bong:tuike_shed_burst",
                particle_id: if ancient {
                    "bong:ancient_skin_glow"
                } else {
                    "bong:false_skin_shed_burst"
                },
                sound_recipe_id: "shed_skin_burst",
                icon_texture: "bong-client:textures/gui/skill/tuike_shed.png",
            },
            TuikeSkillId::TransferTaint => Self {
                animation_id: "bong:tuike_taint_transfer",
                particle_id: if ancient {
                    "bong:ancient_skin_glow"
                } else {
                    "bong:false_skin_don_dust"
                },
                sound_recipe_id: "contam_transfer_hum",
                icon_texture: "bong-client:textures/gui/skill/tuike_transfer_taint.png",
            },
        }
    }
}

#[derive(Debug, Clone, Event, PartialEq, Serialize, Deserialize)]
pub struct DonFalseSkinEvent {
    pub caster: Entity,
    pub tier: FalseSkinTier,
    pub layers_after: u8,
    pub tick: u64,
    pub visual: TuikeSkillVisualPayload,
}

#[derive(Debug, Clone, Event, PartialEq, Serialize, Deserialize)]
pub struct FalseSkinSheddedEvent {
    pub owner: Entity,
    pub attacker: Option<Entity>,
    pub tier: FalseSkinTier,
    pub damage_absorbed: f64,
    pub damage_overflow: f64,
    pub contam_load: f64,
    pub permanent_taint_load: f64,
    pub layers_after: u8,
    pub active: bool,
    pub tick: u64,
    pub visual: TuikeSkillVisualPayload,
}

#[derive(Debug, Clone, Event, PartialEq, Serialize, Deserialize)]
pub struct ContamTransferredEvent {
    pub caster: Entity,
    pub tier: FalseSkinTier,
    pub contam_moved_percent: f64,
    pub backflow_percent: f64,
    pub permanent_absorbed: f64,
    pub qi_cost: f64,
    pub tick: u64,
    pub visual: TuikeSkillVisualPayload,
}

#[derive(Debug, Clone, Event, PartialEq, Serialize, Deserialize)]
pub struct FalseSkinDecayedToAshEvent {
    pub owner: Entity,
    pub tier: FalseSkinTier,
    pub output_item_id: String,
    pub tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq, Serialize, Deserialize)]
pub struct PermanentTaintAbsorbedEvent {
    pub caster: Entity,
    pub amount: f64,
    pub tier: FalseSkinTier,
    pub tick: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TuikeSkillVisualPayload {
    pub animation_id: String,
    pub particle_id: String,
    pub sound_recipe_id: String,
    pub icon_texture: String,
}

impl From<TuikeSkillVisual> for TuikeSkillVisualPayload {
    fn from(value: TuikeSkillVisual) -> Self {
        Self {
            animation_id: value.animation_id.to_string(),
            particle_id: value.particle_id.to_string(),
            sound_recipe_id: value.sound_recipe_id.to_string(),
            icon_texture: value.icon_texture.to_string(),
        }
    }
}
