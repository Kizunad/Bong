//! TSY 裂缝 / 撤离点 Component 与静态 portal 配置。

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component, DVec3, Resource};

use crate::world::tsy::DimensionAnchor;

pub const PORTAL_INTERACT_RADIUS: f64 = 2.0;

/// 裂缝 POI 朝向。Entry = 主世界 → TSY；Exit = TSY → 主世界。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortalDirection {
    Entry,
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiftKind {
    MainRift,
    DeepRift,
    CollapseTear,
}

impl RiftKind {
    pub const fn base_extract_ticks(self) -> u32 {
        match self {
            Self::MainRift => 160,
            Self::DeepRift => 240,
            Self::CollapseTear => 60,
        }
    }

    pub const fn allows_entry(self) -> bool {
        matches!(self, Self::MainRift)
    }

    pub const fn allows_exit(self) -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TickWindow {
    pub start_at_tick: u64,
    pub end_at_tick: u64,
}

/// TSY 的裂缝 POI / 撤离点。Entity 位置即 portal 位置。
#[derive(Component, Debug, Clone)]
pub struct RiftPortal {
    pub family_id: String,
    /// Entry 仍用此锚点入场；Exit 撤离完成改读 `TsyPresence.return_to`。
    pub target: DimensionAnchor,
    pub trigger_radius: f64,
    pub direction: PortalDirection,
    pub kind: RiftKind,
    pub current_extract_ticks: u32,
    pub activation_window: Option<TickWindow>,
}

impl RiftPortal {
    pub fn new(
        family_id: String,
        target: DimensionAnchor,
        trigger_radius: f64,
        direction: PortalDirection,
        kind: RiftKind,
    ) -> Self {
        Self {
            family_id,
            target,
            trigger_radius,
            direction,
            kind,
            current_extract_ticks: kind.base_extract_ticks(),
            activation_window: None,
        }
    }

    pub fn entry(family_id: String, target: DimensionAnchor, trigger_radius: f64) -> Self {
        Self::new(
            family_id,
            target,
            trigger_radius,
            PortalDirection::Entry,
            RiftKind::MainRift,
        )
    }

    pub fn exit(
        family_id: String,
        target: DimensionAnchor,
        trigger_radius: f64,
        kind: RiftKind,
    ) -> Self {
        Self::new(
            family_id,
            target,
            trigger_radius,
            PortalDirection::Exit,
            kind,
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TsyPortalSpec {
    pub kind: RiftKind,
    pub pos: DVec3,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TsyPortalFamilySpec {
    pub shallow: Vec<TsyPortalSpec>,
    pub mid: Vec<TsyPortalSpec>,
    pub deep: Vec<TsyPortalSpec>,
}

#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct TsyPortalRegistry {
    pub by_family: HashMap<String, TsyPortalFamilySpec>,
}

pub fn load_tsy_portals() -> TsyPortalRegistry {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tsy_portals.json");
    load_tsy_portals_from_path(path)
}

pub fn load_tsy_portals_from_path(path: impl AsRef<Path>) -> TsyPortalRegistry {
    let path = path.as_ref();
    let contents = match std::fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            tracing::info!(
                "[bong][tsy-portal] no {} config, using empty portal registry",
                path.display()
            );
            return TsyPortalRegistry::default();
        }
        Err(error) => {
            tracing::warn!(
                "[bong][tsy-portal] failed to read {}: {error}",
                path.display()
            );
            return TsyPortalRegistry::default();
        }
    };

    match serde_json::from_str::<HashMap<String, PortalFamilyWire>>(&contents) {
        Ok(wire) => TsyPortalRegistry {
            by_family: wire
                .into_iter()
                .map(|(family, spec)| (family, spec.into_runtime()))
                .collect(),
        },
        Err(error) => {
            tracing::warn!(
                "[bong][tsy-portal] failed to parse {}: {error}",
                path.display()
            );
            TsyPortalRegistry::default()
        }
    }
}

#[derive(Debug, Deserialize)]
struct PortalFamilyWire {
    #[serde(default)]
    shallow: Vec<PortalSpecWire>,
    #[serde(default)]
    mid: Vec<PortalSpecWire>,
    #[serde(default)]
    deep: Vec<PortalSpecWire>,
}

impl PortalFamilyWire {
    fn into_runtime(self) -> TsyPortalFamilySpec {
        TsyPortalFamilySpec {
            shallow: self
                .shallow
                .into_iter()
                .map(PortalSpecWire::into_runtime)
                .collect(),
            mid: self
                .mid
                .into_iter()
                .map(PortalSpecWire::into_runtime)
                .collect(),
            deep: self
                .deep
                .into_iter()
                .map(PortalSpecWire::into_runtime)
                .collect(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct PortalSpecWire {
    kind: RiftKind,
    pos: [f64; 3],
}

impl PortalSpecWire {
    fn into_runtime(self) -> TsyPortalSpec {
        TsyPortalSpec {
            kind: self.kind,
            pos: DVec3::new(self.pos[0], self.pos[1], self.pos[2]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rift_kind_extract_table_matches_worldview() {
        assert_eq!(RiftKind::MainRift.base_extract_ticks(), 160);
        assert_eq!(RiftKind::DeepRift.base_extract_ticks(), 240);
        assert_eq!(RiftKind::CollapseTear.base_extract_ticks(), 60);
    }

    #[test]
    fn rift_kind_entry_exit_permissions() {
        assert!(RiftKind::MainRift.allows_entry());
        assert!(!RiftKind::DeepRift.allows_entry());
        assert!(!RiftKind::CollapseTear.allows_entry());
        assert!(RiftKind::MainRift.allows_exit());
        assert!(RiftKind::DeepRift.allows_exit());
        assert!(RiftKind::CollapseTear.allows_exit());
    }
}
