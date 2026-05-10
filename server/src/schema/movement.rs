use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MovementActionRequestV1 {
    Dash,
    Slide,
    DoubleJump,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MovementActionV1 {
    None,
    Dashing,
    Sliding,
    DoubleJumping,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MovementZoneKindV1 {
    Normal,
    Dead,
    Negative,
    ResidueAsh,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MovementStateV1 {
    pub current_speed_multiplier: f32,
    pub stamina_cost_active: bool,
    pub movement_action: MovementActionV1,
    pub zone_kind: MovementZoneKindV1,
    pub dash_cooldown_remaining_ticks: u64,
    pub slide_cooldown_remaining_ticks: u64,
    pub double_jump_charges_remaining: u8,
    pub double_jump_charges_max: u8,
    pub hitbox_height_blocks: f32,
    pub stamina_current: f32,
    pub stamina_max: f32,
    pub low_stamina: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_action_tick: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejected_action: Option<MovementActionRequestV1>,
}
