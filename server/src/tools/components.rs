use valence::prelude::{bevy_ecs, Component, Entity};

use super::ToolKind;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolTag {
    pub kind: ToolKind,
    pub instance_id: u64,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButcherableCorpse {
    pub corpse_id: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButcherSession {
    pub player: Entity,
    pub corpse: Entity,
    pub tool: Option<ToolKind>,
}
