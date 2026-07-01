use valence::prelude::{bevy_ecs, Component};

use super::ToolKind;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolTag {
    pub kind: ToolKind,
    pub instance_id: u64,
}
