use serde::{Deserialize, Serialize};

use crate::cultivation::void::components::{BarrierGeometry, VoidActionCost, VoidActionKind};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "snake_case")]
pub enum VoidActionRequestV1 {
    SuppressTsy {
        zone_id: String,
    },
    ExplodeZone {
        zone_id: String,
    },
    Barrier {
        zone_id: String,
        geometry: BarrierGeometry,
    },
    LegacyAssign {
        inheritor_id: String,
        #[serde(default)]
        item_instance_ids: Vec<u64>,
        #[serde(default)]
        message: Option<String>,
    },
}

impl VoidActionRequestV1 {
    pub fn kind(&self) -> VoidActionKind {
        match self {
            Self::SuppressTsy { .. } => VoidActionKind::SuppressTsy,
            Self::ExplodeZone { .. } => VoidActionKind::ExplodeZone,
            Self::Barrier { .. } => VoidActionKind::Barrier,
            Self::LegacyAssign { .. } => VoidActionKind::LegacyAssign,
        }
    }

    pub fn target_label(&self) -> String {
        match self {
            Self::SuppressTsy { zone_id }
            | Self::ExplodeZone { zone_id }
            | Self::Barrier { zone_id, .. } => zone_id.clone(),
            Self::LegacyAssign { inheritor_id, .. } => inheritor_id.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VoidActionResponseV1 {
    pub v: u8,
    pub accepted: bool,
    pub kind: VoidActionKind,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cooldown_until_tick: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<VoidActionCost>,
}

impl VoidActionResponseV1 {
    pub fn accepted(kind: VoidActionKind, reason: impl Into<String>) -> Self {
        Self {
            v: 1,
            accepted: true,
            kind,
            reason: reason.into(),
            cooldown_until_tick: None,
            cost: Some(VoidActionCost::for_kind(kind)),
        }
    }

    pub fn rejected(kind: VoidActionKind, reason: impl Into<String>) -> Self {
        Self {
            v: 1,
            accepted: false,
            kind,
            reason: reason.into(),
            cooldown_until_tick: None,
            cost: Some(VoidActionCost::for_kind(kind)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VoidActionBroadcastV1 {
    pub v: u8,
    pub kind: VoidActionKind,
    pub actor_id: String,
    pub actor_name: String,
    pub target: String,
    pub at_tick: u64,
    pub qi_cost: f64,
    pub lifespan_cost_years: u32,
    pub scope: String,
    pub public_text: String,
}

impl VoidActionBroadcastV1 {
    pub fn new(
        kind: VoidActionKind,
        actor_id: impl Into<String>,
        actor_name: impl Into<String>,
        target: impl Into<String>,
        at_tick: u64,
        public_text: impl Into<String>,
    ) -> Self {
        Self {
            v: 1,
            kind,
            actor_id: actor_id.into(),
            actor_name: actor_name.into(),
            target: target.into(),
            at_tick,
            qi_cost: kind.qi_cost(),
            lifespan_cost_years: kind.lifespan_cost_years(),
            scope: "broadcast".to_string(),
            public_text: public_text.into(),
        }
    }

    pub fn channel_name(&self) -> &'static str {
        crate::schema::channels::void_action_channel(self.kind)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VoidActionCooldownV1 {
    pub kind: VoidActionKind,
    pub ready_at_tick: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VoidActionStateV1 {
    pub v: u8,
    pub actor_id: String,
    pub cooldowns: Vec<VoidActionCooldownV1>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_kind_maps_suppress_tsy() {
        assert_eq!(
            VoidActionRequestV1::SuppressTsy {
                zone_id: "tsy".to_string()
            }
            .kind(),
            VoidActionKind::SuppressTsy
        );
    }

    #[test]
    fn request_kind_maps_explode_zone() {
        assert_eq!(
            VoidActionRequestV1::ExplodeZone {
                zone_id: "spawn".to_string()
            }
            .kind(),
            VoidActionKind::ExplodeZone
        );
    }

    #[test]
    fn request_kind_maps_barrier() {
        assert_eq!(
            VoidActionRequestV1::Barrier {
                zone_id: "spawn".to_string(),
                geometry: BarrierGeometry::circle([0.0, 64.0, 0.0], 10.0),
            }
            .kind(),
            VoidActionKind::Barrier
        );
    }

    #[test]
    fn request_kind_maps_legacy_assign() {
        assert_eq!(
            VoidActionRequestV1::LegacyAssign {
                inheritor_id: "heir".to_string(),
                item_instance_ids: vec![],
                message: None,
            }
            .kind(),
            VoidActionKind::LegacyAssign
        );
    }

    #[test]
    fn target_label_uses_zone_for_zone_actions() {
        let request = VoidActionRequestV1::ExplodeZone {
            zone_id: "spawn".to_string(),
        };
        assert_eq!(request.target_label(), "spawn");
    }

    #[test]
    fn target_label_uses_heir_for_legacy() {
        let request = VoidActionRequestV1::LegacyAssign {
            inheritor_id: "heir".to_string(),
            item_instance_ids: Vec::new(),
            message: None,
        };
        assert_eq!(request.target_label(), "heir");
    }

    #[test]
    fn accepted_response_carries_cost() {
        let response = VoidActionResponseV1::accepted(VoidActionKind::Barrier, "ok");
        assert!(response.accepted);
        assert_eq!(response.cost.unwrap().qi, 150.0);
    }

    #[test]
    fn rejected_response_keeps_version() {
        let response = VoidActionResponseV1::rejected(VoidActionKind::Barrier, "no");
        assert_eq!(response.v, 1);
        assert!(!response.accepted);
    }

    #[test]
    fn broadcast_defaults_to_broadcast_scope() {
        let event = VoidActionBroadcastV1::new(
            VoidActionKind::SuppressTsy,
            "actor",
            "Actor",
            "tsy",
            1,
            "text",
        );
        assert_eq!(event.scope, "broadcast");
    }

    #[test]
    fn broadcast_channel_uses_kind() {
        let event = VoidActionBroadcastV1::new(
            VoidActionKind::LegacyAssign,
            "actor",
            "Actor",
            "heir",
            1,
            "text",
        );
        assert_eq!(event.channel_name(), "bong:void_action/legacy_assign");
    }
}
