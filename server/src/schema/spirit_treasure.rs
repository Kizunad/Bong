use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpiritTreasureDialogueTriggerV1 {
    Player,
    Random,
    Event,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpiritTreasureDialogueToneV1 {
    Cold,
    Curious,
    Warning,
    Amused,
    Silent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpiritTreasureDialogueHistoryEntryV1 {
    pub speaker: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpiritTreasureDialogueContextV1 {
    pub realm: String,
    pub qi_percent: f64,
    pub zone: String,
    pub recent_events: Vec<String>,
    pub affinity: f64,
    pub dialogue_history: Vec<SpiritTreasureDialogueHistoryEntryV1>,
    pub equipped: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpiritTreasureDialogueRequestV1 {
    pub v: u8,
    pub request_id: String,
    pub character_id: String,
    pub treasure_id: String,
    pub trigger: SpiritTreasureDialogueTriggerV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_message: Option<String>,
    pub context: SpiritTreasureDialogueContextV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpiritTreasureDialogueV1 {
    pub v: u8,
    pub request_id: String,
    pub character_id: String,
    pub treasure_id: String,
    pub text: String,
    pub tone: SpiritTreasureDialogueToneV1,
    pub affinity_delta: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpiritTreasurePassiveV1 {
    pub kind: String,
    pub value: f64,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpiritTreasureClientStateV1 {
    pub template_id: String,
    pub display_name: String,
    pub instance_id: u64,
    pub equipped: bool,
    pub passive_active: bool,
    pub affinity: f64,
    pub sleeping: bool,
    pub source_sect: Option<String>,
    pub icon_texture: String,
    pub passive_effects: Vec<SpiritTreasurePassiveV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpiritTreasureStatePayloadV1 {
    pub treasures: Vec<SpiritTreasureClientStateV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpiritTreasureDialoguePayloadV1 {
    pub dialogue: SpiritTreasureDialogueV1,
    pub display_name: String,
    pub zone: String,
}
