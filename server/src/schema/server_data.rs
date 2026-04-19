use serde::{de::Error as _, Deserialize, Deserializer, Serialize};

use super::alchemy::{
    AlchemyContaminationDataV1, AlchemyFurnaceDataV1, AlchemyOutcomeForecastDataV1,
    AlchemyOutcomeResolvedDataV1, AlchemyRecipeBookDataV1, AlchemySessionDataV1,
};
use super::combat_hud::{
    CastSyncV1, CombatHudStateV1, DefenseSyncV1, DefenseWindowV1, EventStreamPushV1,
    QuickSlotConfigV1, UnlocksSyncV1, WeaponBrokenV1, WeaponEquippedV1, WoundsSnapshotV1,
};
use super::common::{EventKind, MAX_PAYLOAD_BYTES};
use super::inventory::{InventoryEventV1, InventorySnapshotV1};
use super::lingtian::LingtianSessionDataV1;
use super::narration::Narration;
use super::world_state::PlayerPowerBreakdown;

pub const SERVER_DATA_VERSION: u8 = 1;
pub const WELCOME_MESSAGE: &str = "Bong server connected";
pub const HEARTBEAT_MESSAGE: &str = "mock agent tick";

#[derive(Debug)]
pub enum ServerDataBuildError {
    Json(serde_json::Error),
    Oversize { size: usize, max: usize },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ServerDataType {
    Welcome,
    Heartbeat,
    Narration,
    ZoneInfo,
    EventAlert,
    PlayerState,
    UiOpen,
    CultivationDetail,
    InventorySnapshot,
    InventoryEvent,
    AlchemyFurnace,
    AlchemySession,
    AlchemyOutcomeForecast,
    AlchemyOutcomeResolved,
    AlchemyRecipeBook,
    AlchemyContamination,
    CombatHudState,
    WoundsSnapshot,
    DefenseWindow,
    CastSync,
    QuickSlotConfig,
    UnlocksSync,
    EventStreamPush,
    DefenseSync,
    WeaponEquipped,
    WeaponBroken,
    LingtianSession,
}

#[derive(Debug, Clone)]
pub enum ServerDataPayloadV1 {
    Welcome {
        message: String,
    },
    Heartbeat {
        message: String,
    },
    Narration {
        narrations: Vec<Narration>,
    },
    ZoneInfo {
        zone: String,
        spirit_qi: f64,
        danger_level: u8,
        active_events: Option<Vec<String>>,
    },
    EventAlert {
        event: EventKind,
        message: String,
        zone: Option<String>,
        duration_ticks: Option<u64>,
    },
    PlayerState {
        player: Option<String>,
        realm: String,
        spirit_qi: f64,
        karma: f64,
        composite_power: f64,
        breakdown: PlayerPowerBreakdown,
        zone: String,
    },
    UiOpen {
        ui: Option<String>,
        xml: String,
    },
    /// 经脉详细快照。20 条经脉以 SoA(parallel arrays) 布局，顺序与 `MeridianId` 判别式一致
    /// (Lung=0..Liver=11, Ren=12..YangWei=19)。保持 ≤ MAX_PAYLOAD_BYTES 预算。
    CultivationDetail {
        /// 境界字面量（Awaken/Induce/Condense/Solidify/Spirit/Void，与 `Realm` 判别式对齐）。
        realm: String,
        opened: Vec<bool>,
        flow_rate: Vec<f64>,
        flow_capacity: Vec<f64>,
        integrity: Vec<f64>,
        /// 每条经脉未打通时的累积进度 0..=1（已打通恒为 1.0）。
        open_progress: Vec<f64>,
        /// 每条经脉当前裂痕条目数（0..=255，饱和）。UI 用于渲染裂痕图标密度。
        cracks_count: Vec<u8>,
        /// 整个实体的污染总量（所有 `Contamination.entries.amount` 求和）。
        contamination_total: f64,
    },
    InventorySnapshot(Box<InventorySnapshotV1>),
    InventoryEvent(InventoryEventV1),
    AlchemyFurnace(Box<AlchemyFurnaceDataV1>),
    AlchemySession(Box<AlchemySessionDataV1>),
    AlchemyOutcomeForecast(Box<AlchemyOutcomeForecastDataV1>),
    AlchemyOutcomeResolved(Box<AlchemyOutcomeResolvedDataV1>),
    AlchemyRecipeBook(Box<AlchemyRecipeBookDataV1>),
    AlchemyContamination(Box<AlchemyContaminationDataV1>),
    CombatHudState(CombatHudStateV1),
    WoundsSnapshot(WoundsSnapshotV1),
    DefenseWindow(DefenseWindowV1),
    CastSync(CastSyncV1),
    QuickSlotConfig(QuickSlotConfigV1),
    UnlocksSync(UnlocksSyncV1),
    EventStreamPush(EventStreamPushV1),
    DefenseSync(DefenseSyncV1),
    WeaponEquipped(WeaponEquippedV1),
    WeaponBroken(WeaponBrokenV1),
    LingtianSession(Box<LingtianSessionDataV1>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, tag = "type", rename_all = "snake_case")]
enum ServerDataPayloadWireV1 {
    Welcome {
        message: String,
    },
    Heartbeat {
        message: String,
    },
    Narration {
        narrations: Vec<Narration>,
    },
    ZoneInfo {
        zone: String,
        spirit_qi: f64,
        danger_level: u8,
        #[serde(skip_serializing_if = "Option::is_none")]
        active_events: Option<Vec<String>>,
    },
    EventAlert {
        event: EventKind,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        zone: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        duration_ticks: Option<u64>,
    },
    PlayerState {
        #[serde(skip_serializing_if = "Option::is_none")]
        player: Option<String>,
        realm: String,
        spirit_qi: f64,
        karma: f64,
        composite_power: f64,
        breakdown: PlayerPowerBreakdown,
        zone: String,
    },
    UiOpen {
        #[serde(skip_serializing_if = "Option::is_none")]
        ui: Option<String>,
        xml: String,
    },
    CultivationDetail {
        realm: String,
        opened: Vec<bool>,
        flow_rate: Vec<f64>,
        flow_capacity: Vec<f64>,
        integrity: Vec<f64>,
        open_progress: Vec<f64>,
        cracks_count: Vec<u8>,
        contamination_total: f64,
    },
    InventorySnapshot {
        #[serde(flatten)]
        snapshot: Box<InventorySnapshotV1>,
    },
    InventoryEvent {
        #[serde(flatten)]
        event: ServerDataInventoryEventWireV1,
    },
    AlchemyFurnace {
        #[serde(flatten)]
        data: Box<AlchemyFurnaceDataV1>,
    },
    AlchemySession {
        #[serde(flatten)]
        data: Box<AlchemySessionDataV1>,
    },
    AlchemyOutcomeForecast {
        #[serde(flatten)]
        data: Box<AlchemyOutcomeForecastDataV1>,
    },
    AlchemyOutcomeResolved {
        #[serde(flatten)]
        data: Box<AlchemyOutcomeResolvedDataV1>,
    },
    AlchemyRecipeBook {
        #[serde(flatten)]
        data: Box<AlchemyRecipeBookDataV1>,
    },
    AlchemyContamination {
        #[serde(flatten)]
        data: Box<AlchemyContaminationDataV1>,
    },
    CombatHudState {
        #[serde(flatten)]
        state: CombatHudStateV1,
    },
    WoundsSnapshot {
        #[serde(flatten)]
        snapshot: WoundsSnapshotV1,
    },
    DefenseWindow {
        #[serde(flatten)]
        window: DefenseWindowV1,
    },
    CastSync {
        #[serde(flatten)]
        state: CastSyncV1,
    },
    // 显式 rename 因为默认 snake_case 会得到 "quick_slot_config"，
    // 但 plan §11.4 / client handler 注册的是无下划线 "quickslot_config"。
    #[serde(rename = "quickslot_config")]
    QuickSlotConfig {
        #[serde(flatten)]
        config: QuickSlotConfigV1,
    },
    UnlocksSync {
        #[serde(flatten)]
        unlocks: UnlocksSyncV1,
    },
    EventStreamPush {
        #[serde(flatten)]
        event: EventStreamPushV1,
    },
    DefenseSync {
        #[serde(flatten)]
        state: DefenseSyncV1,
    },
    WeaponEquipped {
        #[serde(flatten)]
        weapon_equipped: WeaponEquippedV1,
    },
    WeaponBroken {
        #[serde(flatten)]
        weapon_broken: WeaponBrokenV1,
    },
    LingtianSession {
        #[serde(flatten)]
        lingtian_session: LingtianSessionDataV1,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum InventoryEventKindWireV1 {
    Moved,
    StackChanged,
    DurabilityChanged,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ServerDataInventoryEventWireV1 {
    kind: InventoryEventKindWireV1,
    revision: u64,
    instance_id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    from: Option<super::inventory::InventoryLocationV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    to: Option<super::inventory::InventoryLocationV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stack_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    durability: Option<f64>,
}

impl TryFrom<ServerDataInventoryEventWireV1> for InventoryEventV1 {
    type Error = String;

    fn try_from(value: ServerDataInventoryEventWireV1) -> Result<Self, Self::Error> {
        let raw = serde_json::to_value(value).map_err(|err| err.to_string())?;
        serde_json::from_value(raw).map_err(|err| err.to_string())
    }
}

impl From<&InventoryEventV1> for ServerDataInventoryEventWireV1 {
    fn from(value: &InventoryEventV1) -> Self {
        match value {
            InventoryEventV1::Moved {
                revision,
                instance_id,
                from,
                to,
            } => Self {
                kind: InventoryEventKindWireV1::Moved,
                revision: *revision,
                instance_id: *instance_id,
                from: Some(from.clone()),
                to: Some(to.clone()),
                stack_count: None,
                durability: None,
            },
            InventoryEventV1::StackChanged {
                revision,
                instance_id,
                stack_count,
            } => Self {
                kind: InventoryEventKindWireV1::StackChanged,
                revision: *revision,
                instance_id: *instance_id,
                from: None,
                to: None,
                stack_count: Some(*stack_count),
                durability: None,
            },
            InventoryEventV1::DurabilityChanged {
                revision,
                instance_id,
                durability,
            } => Self {
                kind: InventoryEventKindWireV1::DurabilityChanged,
                revision: *revision,
                instance_id: *instance_id,
                from: None,
                to: None,
                stack_count: None,
                durability: Some(*durability),
            },
        }
    }
}

impl TryFrom<ServerDataPayloadWireV1> for ServerDataPayloadV1 {
    type Error = String;

    fn try_from(value: ServerDataPayloadWireV1) -> Result<Self, Self::Error> {
        match value {
            ServerDataPayloadWireV1::Welcome { message } => Ok(Self::Welcome { message }),
            ServerDataPayloadWireV1::Heartbeat { message } => Ok(Self::Heartbeat { message }),
            ServerDataPayloadWireV1::Narration { narrations } => Ok(Self::Narration { narrations }),
            ServerDataPayloadWireV1::ZoneInfo {
                zone,
                spirit_qi,
                danger_level,
                active_events,
            } => Ok(Self::ZoneInfo {
                zone,
                spirit_qi,
                danger_level,
                active_events,
            }),
            ServerDataPayloadWireV1::EventAlert {
                event,
                message,
                zone,
                duration_ticks,
            } => Ok(Self::EventAlert {
                event,
                message,
                zone,
                duration_ticks,
            }),
            ServerDataPayloadWireV1::PlayerState {
                player,
                realm,
                spirit_qi,
                karma,
                composite_power,
                breakdown,
                zone,
            } => Ok(Self::PlayerState {
                player,
                realm,
                spirit_qi,
                karma,
                composite_power,
                breakdown,
                zone,
            }),
            ServerDataPayloadWireV1::UiOpen { ui, xml } => Ok(Self::UiOpen { ui, xml }),
            ServerDataPayloadWireV1::CultivationDetail {
                realm,
                opened,
                flow_rate,
                flow_capacity,
                integrity,
                open_progress,
                cracks_count,
                contamination_total,
            } => Ok(Self::CultivationDetail {
                realm,
                opened,
                flow_rate,
                flow_capacity,
                integrity,
                open_progress,
                cracks_count,
                contamination_total,
            }),
            ServerDataPayloadWireV1::InventorySnapshot { snapshot } => {
                Ok(Self::InventorySnapshot(snapshot))
            }
            ServerDataPayloadWireV1::InventoryEvent { event } => {
                Ok(Self::InventoryEvent(event.try_into()?))
            }
            ServerDataPayloadWireV1::AlchemyFurnace { data } => Ok(Self::AlchemyFurnace(data)),
            ServerDataPayloadWireV1::AlchemySession { data } => Ok(Self::AlchemySession(data)),
            ServerDataPayloadWireV1::AlchemyOutcomeForecast { data } => {
                Ok(Self::AlchemyOutcomeForecast(data))
            }
            ServerDataPayloadWireV1::AlchemyOutcomeResolved { data } => {
                Ok(Self::AlchemyOutcomeResolved(data))
            }
            ServerDataPayloadWireV1::AlchemyRecipeBook { data } => {
                Ok(Self::AlchemyRecipeBook(data))
            }
            ServerDataPayloadWireV1::AlchemyContamination { data } => {
                Ok(Self::AlchemyContamination(data))
            }
            ServerDataPayloadWireV1::CombatHudState { state } => Ok(Self::CombatHudState(state)),
            ServerDataPayloadWireV1::WoundsSnapshot { snapshot } => {
                Ok(Self::WoundsSnapshot(snapshot))
            }
            ServerDataPayloadWireV1::DefenseWindow { window } => Ok(Self::DefenseWindow(window)),
            ServerDataPayloadWireV1::CastSync { state } => Ok(Self::CastSync(state)),
            ServerDataPayloadWireV1::QuickSlotConfig { config } => {
                Ok(Self::QuickSlotConfig(config))
            }
            ServerDataPayloadWireV1::UnlocksSync { unlocks } => Ok(Self::UnlocksSync(unlocks)),
            ServerDataPayloadWireV1::EventStreamPush { event } => Ok(Self::EventStreamPush(event)),
            ServerDataPayloadWireV1::DefenseSync { state } => Ok(Self::DefenseSync(state)),
            ServerDataPayloadWireV1::WeaponEquipped { weapon_equipped } => {
                Ok(Self::WeaponEquipped(weapon_equipped))
            }
            ServerDataPayloadWireV1::WeaponBroken { weapon_broken } => {
                Ok(Self::WeaponBroken(weapon_broken))
            }
            ServerDataPayloadWireV1::LingtianSession { lingtian_session } => {
                Ok(Self::LingtianSession(Box::new(lingtian_session)))
            }
        }
    }
}

impl From<&ServerDataPayloadV1> for ServerDataPayloadWireV1 {
    fn from(value: &ServerDataPayloadV1) -> Self {
        match value {
            ServerDataPayloadV1::Welcome { message } => Self::Welcome {
                message: message.clone(),
            },
            ServerDataPayloadV1::Heartbeat { message } => Self::Heartbeat {
                message: message.clone(),
            },
            ServerDataPayloadV1::Narration { narrations } => Self::Narration {
                narrations: narrations.clone(),
            },
            ServerDataPayloadV1::ZoneInfo {
                zone,
                spirit_qi,
                danger_level,
                active_events,
            } => Self::ZoneInfo {
                zone: zone.clone(),
                spirit_qi: *spirit_qi,
                danger_level: *danger_level,
                active_events: active_events.clone(),
            },
            ServerDataPayloadV1::EventAlert {
                event,
                message,
                zone,
                duration_ticks,
            } => Self::EventAlert {
                event: event.clone(),
                message: message.clone(),
                zone: zone.clone(),
                duration_ticks: *duration_ticks,
            },
            ServerDataPayloadV1::PlayerState {
                player,
                realm,
                spirit_qi,
                karma,
                composite_power,
                breakdown,
                zone,
            } => Self::PlayerState {
                player: player.clone(),
                realm: realm.clone(),
                spirit_qi: *spirit_qi,
                karma: *karma,
                composite_power: *composite_power,
                breakdown: breakdown.clone(),
                zone: zone.clone(),
            },
            ServerDataPayloadV1::UiOpen { ui, xml } => Self::UiOpen {
                ui: ui.clone(),
                xml: xml.clone(),
            },
            ServerDataPayloadV1::CultivationDetail {
                realm,
                opened,
                flow_rate,
                flow_capacity,
                integrity,
                open_progress,
                cracks_count,
                contamination_total,
            } => Self::CultivationDetail {
                realm: realm.clone(),
                opened: opened.clone(),
                flow_rate: flow_rate.clone(),
                flow_capacity: flow_capacity.clone(),
                integrity: integrity.clone(),
                open_progress: open_progress.clone(),
                cracks_count: cracks_count.clone(),
                contamination_total: *contamination_total,
            },
            ServerDataPayloadV1::InventorySnapshot(snapshot) => Self::InventorySnapshot {
                snapshot: snapshot.clone(),
            },
            ServerDataPayloadV1::InventoryEvent(event) => Self::InventoryEvent {
                event: event.into(),
            },
            ServerDataPayloadV1::AlchemyFurnace(data) => {
                Self::AlchemyFurnace { data: data.clone() }
            }
            ServerDataPayloadV1::AlchemySession(data) => {
                Self::AlchemySession { data: data.clone() }
            }
            ServerDataPayloadV1::AlchemyOutcomeForecast(data) => {
                Self::AlchemyOutcomeForecast { data: data.clone() }
            }
            ServerDataPayloadV1::AlchemyOutcomeResolved(data) => {
                Self::AlchemyOutcomeResolved { data: data.clone() }
            }
            ServerDataPayloadV1::AlchemyRecipeBook(data) => {
                Self::AlchemyRecipeBook { data: data.clone() }
            }
            ServerDataPayloadV1::AlchemyContamination(data) => {
                Self::AlchemyContamination { data: data.clone() }
            }
            ServerDataPayloadV1::CombatHudState(state) => Self::CombatHudState { state: *state },
            ServerDataPayloadV1::WoundsSnapshot(snapshot) => Self::WoundsSnapshot {
                snapshot: snapshot.clone(),
            },
            ServerDataPayloadV1::DefenseWindow(window) => Self::DefenseWindow { window: *window },
            ServerDataPayloadV1::CastSync(state) => Self::CastSync { state: *state },
            ServerDataPayloadV1::QuickSlotConfig(config) => Self::QuickSlotConfig {
                config: config.clone(),
            },
            ServerDataPayloadV1::UnlocksSync(unlocks) => Self::UnlocksSync { unlocks: *unlocks },
            ServerDataPayloadV1::EventStreamPush(event) => Self::EventStreamPush {
                event: event.clone(),
            },
            ServerDataPayloadV1::DefenseSync(state) => Self::DefenseSync { state: *state },
            ServerDataPayloadV1::WeaponEquipped(w) => Self::WeaponEquipped {
                weapon_equipped: w.clone(),
            },
            ServerDataPayloadV1::WeaponBroken(b) => Self::WeaponBroken {
                weapon_broken: b.clone(),
            },
            ServerDataPayloadV1::LingtianSession(s) => Self::LingtianSession {
                lingtian_session: (**s).clone(),
            },
        }
    }
}

impl Serialize for ServerDataPayloadV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        ServerDataPayloadWireV1::from(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ServerDataPayloadV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = ServerDataPayloadWireV1::deserialize(deserializer)?;
        wire.try_into().map_err(D::Error::custom)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerDataV1 {
    #[serde(deserialize_with = "deserialize_server_data_version")]
    pub v: u8,
    #[serde(flatten)]
    pub payload: ServerDataPayloadV1,
}

fn deserialize_server_data_version<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    let version = u8::deserialize(deserializer)?;
    if version == SERVER_DATA_VERSION {
        Ok(version)
    } else {
        Err(D::Error::custom(format!(
            "ServerDataV1.v must be {SERVER_DATA_VERSION}, got {version}"
        )))
    }
}

impl ServerDataV1 {
    pub fn new(payload: ServerDataPayloadV1) -> Self {
        Self {
            v: SERVER_DATA_VERSION,
            payload,
        }
    }

    pub fn welcome(message: impl Into<String>) -> Self {
        Self::new(ServerDataPayloadV1::Welcome {
            message: message.into(),
        })
    }

    pub fn heartbeat(message: impl Into<String>) -> Self {
        Self::new(ServerDataPayloadV1::Heartbeat {
            message: message.into(),
        })
    }

    pub fn payload_type(&self) -> ServerDataType {
        self.payload.payload_type()
    }

    pub fn to_json_bytes_checked(&self) -> Result<Vec<u8>, ServerDataBuildError> {
        let bytes = serde_json::to_vec(self).map_err(ServerDataBuildError::Json)?;
        if bytes.len() > MAX_PAYLOAD_BYTES {
            return Err(ServerDataBuildError::Oversize {
                size: bytes.len(),
                max: MAX_PAYLOAD_BYTES,
            });
        }

        Ok(bytes)
    }
}

impl ServerDataPayloadV1 {
    pub fn payload_type(&self) -> ServerDataType {
        match self {
            Self::Welcome { .. } => ServerDataType::Welcome,
            Self::Heartbeat { .. } => ServerDataType::Heartbeat,
            Self::Narration { .. } => ServerDataType::Narration,
            Self::ZoneInfo { .. } => ServerDataType::ZoneInfo,
            Self::EventAlert { .. } => ServerDataType::EventAlert,
            Self::PlayerState { .. } => ServerDataType::PlayerState,
            Self::UiOpen { .. } => ServerDataType::UiOpen,
            Self::CultivationDetail { .. } => ServerDataType::CultivationDetail,
            Self::InventorySnapshot(..) => ServerDataType::InventorySnapshot,
            Self::InventoryEvent(..) => ServerDataType::InventoryEvent,
            Self::AlchemyFurnace(..) => ServerDataType::AlchemyFurnace,
            Self::AlchemySession(..) => ServerDataType::AlchemySession,
            Self::AlchemyOutcomeForecast(..) => ServerDataType::AlchemyOutcomeForecast,
            Self::AlchemyOutcomeResolved(..) => ServerDataType::AlchemyOutcomeResolved,
            Self::AlchemyRecipeBook(..) => ServerDataType::AlchemyRecipeBook,
            Self::AlchemyContamination(..) => ServerDataType::AlchemyContamination,
            Self::CombatHudState(..) => ServerDataType::CombatHudState,
            Self::WoundsSnapshot(..) => ServerDataType::WoundsSnapshot,
            Self::DefenseWindow(..) => ServerDataType::DefenseWindow,
            Self::CastSync(..) => ServerDataType::CastSync,
            Self::QuickSlotConfig(..) => ServerDataType::QuickSlotConfig,
            Self::UnlocksSync(..) => ServerDataType::UnlocksSync,
            Self::EventStreamPush(..) => ServerDataType::EventStreamPush,
            Self::DefenseSync(..) => ServerDataType::DefenseSync,
            Self::WeaponEquipped(..) => ServerDataType::WeaponEquipped,
            Self::WeaponBroken(..) => ServerDataType::WeaponBroken,
            Self::LingtianSession(..) => ServerDataType::LingtianSession,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::agent_bridge::payload_type_label;

    /// Catches wire-vs-label drift like the QuickSlotConfig "snake_case" bug
    /// (would have routed `quick_slot_config` while client expected `quickslot_config`).
    #[test]
    fn hud_payload_wire_type_matches_label() {
        use crate::schema::combat_hud::*;
        let cases: Vec<ServerDataPayloadV1> = vec![
            ServerDataPayloadV1::CombatHudState(CombatHudStateV1 {
                hp_percent: 1.0,
                qi_percent: 1.0,
                stamina_percent: 1.0,
                derived: DerivedAttrFlagsV1::default(),
            }),
            ServerDataPayloadV1::WoundsSnapshot(WoundsSnapshotV1 { wounds: vec![] }),
            ServerDataPayloadV1::DefenseWindow(DefenseWindowV1 {
                duration_ms: 200,
                started_at_ms: 0,
                expires_at_ms: 200,
            }),
            ServerDataPayloadV1::CastSync(CastSyncV1 {
                phase: CastPhaseV1::Idle,
                slot: 0,
                duration_ms: 0,
                started_at_ms: 0,
                outcome: CastOutcomeV1::None,
            }),
            ServerDataPayloadV1::QuickSlotConfig(QuickSlotConfigV1 {
                slots: vec![None; 9],
                cooldown_until_ms: vec![0; 9],
            }),
            ServerDataPayloadV1::UnlocksSync(UnlocksSyncV1::default()),
            ServerDataPayloadV1::EventStreamPush(EventStreamPushV1 {
                channel: EventChannelV1::Combat,
                priority: EventPriorityV1::P1Important,
                source_tag: String::new(),
                text: "x".to_string(),
                color: 0,
                created_at_ms: 0,
            }),
            ServerDataPayloadV1::DefenseSync(DefenseSyncV1 {
                stance: DefenseStanceV1::None,
                fake_skin_layers: 0,
                vortex_active: false,
                vortex_ready_at_ms: 0,
            }),
        ];

        for payload in cases {
            let label = payload_type_label(payload.payload_type());
            let envelope = ServerDataV1::new(payload);
            let bytes = serde_json::to_vec(&envelope).expect("serialize");
            let value: serde_json::Value = serde_json::from_slice(&bytes).expect("decode");
            let wire_type = value
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            assert_eq!(
                wire_type, label,
                "wire type {wire_type} does not match payload_type_label {label}"
            );
        }
    }

    #[test]
    fn cultivation_detail_roundtrip_and_size_budget() {
        let payload = ServerDataV1::new(ServerDataPayloadV1::CultivationDetail {
            realm: "Induce".to_string(),
            opened: vec![true; 20],
            flow_rate: vec![1.5; 20],
            flow_capacity: vec![10.25; 20],
            integrity: vec![0.87; 20],
            open_progress: vec![1.0; 20],
            cracks_count: vec![0; 20],
            contamination_total: 0.0,
        });
        let bytes = payload
            .to_json_bytes_checked()
            .expect("cultivation_detail must fit MAX_PAYLOAD_BYTES");
        assert!(
            bytes.len() <= super::super::common::MAX_PAYLOAD_BYTES,
            "over budget: {} bytes",
            bytes.len()
        );
        let back: ServerDataV1 = serde_json::from_slice(&bytes).expect("roundtrip");
        match back.payload {
            ServerDataPayloadV1::CultivationDetail {
                opened, flow_rate, ..
            } => {
                assert_eq!(opened.len(), 20);
                assert_eq!(flow_rate.len(), 20);
                assert_eq!(flow_rate[0], 1.5);
            }
            other => panic!("expected CultivationDetail, got {other:?}"),
        }
    }

    #[test]
    fn deserialize_server_data_samples() {
        let samples = [
            include_str!("../../../agent/packages/schema/samples/server-data.welcome.sample.json"),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.heartbeat.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.narration.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.zone-info.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.event-alert.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.player-state.sample.json"
            ),
            include_str!("../../../agent/packages/schema/samples/server-data.ui-open.sample.json"),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.inventory-snapshot.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.inventory-event.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.alchemy-furnace.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.alchemy-session.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.alchemy-outcome-forecast.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.alchemy-outcome-resolved.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.alchemy-recipe-book.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.alchemy-contamination.sample.json"
            ),
        ];

        for json in samples {
            let payload: ServerDataV1 =
                serde_json::from_str(json).expect("sample should deserialize into ServerDataV1");

            let reserialized = serde_json::to_string(&payload)
                .expect("deserialized ServerDataV1 should serialize back to JSON");
            let roundtrip: ServerDataV1 = serde_json::from_str(&reserialized)
                .expect("serialized ServerDataV1 should deserialize again");

            let payload_value =
                serde_json::to_value(&payload).expect("payload should convert to JSON value");
            let roundtrip_value =
                serde_json::to_value(&roundtrip).expect("roundtrip should convert to JSON value");

            assert_eq!(
                payload_value, roundtrip_value,
                "roundtrip must preserve typed payload content"
            );
        }
    }
}
