use crossbeam_channel::{Receiver, Sender};
use std::{thread, time::Duration};
use valence::prelude::Resource;

use crate::schema::server_data::{ServerDataBuildError, ServerDataType, ServerDataV1};

pub const SERVER_DATA_CHANNEL: &str = "bong:server_data";

pub type PayloadBuildError = ServerDataBuildError;

#[derive(Debug, Clone)]
pub enum AgentCommand {
    Heartbeat,
}

#[derive(Debug, Clone)]
pub enum GameEvent {
    Placeholder,
    /// plan-mineral-v1 §3 / §M6 — 极品矿脉触发的天道劫气标记。
    /// agent 侧（未实装）订阅此 GameEvent 给 LLM 输入"玩家挖到品阶 N 矿，
    /// 概率 P 触发劫气"语义信号；当前阶段仅由 server 侧 emit。
    /// 字段 `#[allow(dead_code)]`：消费侧（agent bridge daemon）尚未读取，
    /// 但 server 侧已 emit；保留以确保 wire 接口稳定。
    #[allow(dead_code)]
    MineralKarmaFlag {
        player_username: String,
        mineral_id: String,
        position: [i32; 3],
        probability: f32,
    },
}

pub struct NetworkBridgeResource {
    pub tx_to_agent: Sender<GameEvent>,
    pub rx_from_agent: Receiver<AgentCommand>,
}

impl Resource for NetworkBridgeResource {}

pub fn serialize_server_data_payload(payload: &ServerDataV1) -> Result<Vec<u8>, PayloadBuildError> {
    payload.to_json_bytes_checked()
}

pub fn payload_type_label(payload_type: ServerDataType) -> &'static str {
    match payload_type {
        ServerDataType::Welcome => "welcome",
        ServerDataType::Heartbeat => "heartbeat",
        ServerDataType::Narration => "narration",
        ServerDataType::ZoneInfo => "zone_info",
        ServerDataType::EventAlert => "event_alert",
        ServerDataType::PlayerState => "player_state",
        ServerDataType::UiOpen => "ui_open",
        ServerDataType::CultivationDetail => "cultivation_detail",
        ServerDataType::QiColorObserved => "qi_color_observed",
        ServerDataType::InventorySnapshot => "inventory_snapshot",
        ServerDataType::InventoryEvent => "inventory_event",
        ServerDataType::DroppedLootSync => "dropped_loot_sync",
        ServerDataType::BotanyHarvestProgress => "botany_harvest_progress",
        ServerDataType::BotanyPlantV2RenderProfiles => "botany_plant_v2_render_profiles",
        ServerDataType::MiningProgress => "mining_progress",
        ServerDataType::LumberProgress => "lumber_progress",
        ServerDataType::BotanySkill => "botany_skill",
        ServerDataType::AlchemyFurnace => "alchemy_furnace",
        ServerDataType::AlchemySession => "alchemy_session",
        ServerDataType::AlchemyOutcomeForecast => "alchemy_outcome_forecast",
        ServerDataType::AlchemyOutcomeResolved => "alchemy_outcome_resolved",
        ServerDataType::AlchemyRecipeBook => "alchemy_recipe_book",
        ServerDataType::AlchemyContamination => "alchemy_contamination",
        ServerDataType::CombatHudState => "combat_hud_state",
        ServerDataType::WoundsSnapshot => "wounds_snapshot",
        ServerDataType::DefenseWindow => "defense_window",
        ServerDataType::CastSync => "cast_sync",
        ServerDataType::QuickSlotConfig => "quickslot_config",
        ServerDataType::SkillBarConfig => "skillbar_config",
        ServerDataType::TechniquesSnapshot => "techniques_snapshot",
        ServerDataType::SkillConfigSnapshot => "skill_config_snapshot",
        ServerDataType::UnlocksSync => "unlocks_sync",
        ServerDataType::DerivedAttrsSync => "derived_attrs_sync",
        ServerDataType::EventStreamPush => "event_stream_push",
        ServerDataType::WeaponEquipped => "weapon_equipped",
        ServerDataType::WeaponBroken => "weapon_broken",
        ServerDataType::TreasureEquipped => "treasure_equipped",
        ServerDataType::VortexState => "vortex_state",
        ServerDataType::DuguPoisonState => "dugu_poison_state",
        ServerDataType::CarrierState => "carrier_state",
        ServerDataType::FalseSkinState => "false_skin_state",
        ServerDataType::LingtianSession => "lingtian_session",
        ServerDataType::DeathScreen => "death_screen",
        ServerDataType::TerminateScreen => "terminate_screen",
        ServerDataType::RiftPortalState => "rift_portal_state",
        ServerDataType::RiftPortalRemoved => "rift_portal_removed",
        ServerDataType::ExtractStarted => "extract_started",
        ServerDataType::ExtractProgress => "extract_progress",
        ServerDataType::ExtractCompleted => "extract_completed",
        ServerDataType::ExtractAborted => "extract_aborted",
        ServerDataType::ExtractFailed => "extract_failed",
        ServerDataType::TsyCollapseStartedIpc => "tsy_collapse_started_ipc",
        ServerDataType::ContainerState => "container_state",
        ServerDataType::SearchStarted => "search_started",
        ServerDataType::SearchProgress => "search_progress",
        ServerDataType::SearchCompleted => "search_completed",
        ServerDataType::SearchAborted => "search_aborted",
        ServerDataType::SkillXpGain => "skill_xp_gain",
        ServerDataType::SkillLvUp => "skill_lv_up",
        ServerDataType::SkillCapChanged => "skill_cap_changed",
        ServerDataType::SkillScrollUsed => "skill_scroll_used",
        ServerDataType::SkillSnapshot => "skill_snapshot",
        ServerDataType::ForgeStation => "forge_station",
        ServerDataType::ForgeSession => "forge_session",
        ServerDataType::ForgeOutcome => "forge_outcome",
        ServerDataType::ForgeBlueprintBook => "forge_blueprint_book",
        ServerDataType::TribulationState => "tribulation_state",
        ServerDataType::TribulationBroadcast => "tribulation_broadcast",
        ServerDataType::AscensionQuota => "ascension_quota",
        ServerDataType::HeartDemonOffer => "heart_demon_offer",
        ServerDataType::BurstMeridianEvent => "burst_meridian_event",
        ServerDataType::SocialAnonymity => "social_anonymity",
        ServerDataType::SocialExposure => "social_exposure",
        ServerDataType::SocialPact => "social_pact",
        ServerDataType::SocialFeud => "social_feud",
        ServerDataType::SocialRenownDelta => "social_renown_delta",
        ServerDataType::NicheIntrusion => "niche_intrusion",
        ServerDataType::NicheGuardianFatigue => "niche_guardian_fatigue",
        ServerDataType::NicheGuardianBroken => "niche_guardian_broken",
        ServerDataType::SparringInvite => "sparring_invite",
        ServerDataType::TradeOffer => "trade_offer",
        ServerDataType::RealmVisionParams => "realm_vision_params",
        ServerDataType::SpiritualSenseTargets => "spiritual_sense_targets",
        // ─── plan-craft-v1 P2/P3：通用手搓 ──────────────────────
        ServerDataType::CraftRecipeList => "craft_recipe_list",
        ServerDataType::CraftSessionState => "craft_session_state",
        ServerDataType::CraftOutcome => "craft_outcome",
        ServerDataType::RecipeUnlocked => "recipe_unlocked",
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecipientSelector {
    Broadcast,
    Player(String),
    Zone(String),
}

#[allow(dead_code)]
impl RecipientSelector {
    pub fn player(username_or_alias: impl Into<String>) -> Self {
        Self::Player(username_or_alias.into())
    }

    pub fn zone(zone_name: impl Into<String>) -> Self {
        Self::Zone(zone_name.into())
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RecipientMetadata {
    pub username: Option<String>,
    pub char_id: Option<String>,
    pub zone: Option<String>,
}

#[allow(dead_code)]
pub type ZoneRouteHook<'a> = &'a dyn Fn(&str, &RecipientMetadata) -> bool;

#[allow(dead_code)]
pub fn normalize_player_target(target: &str) -> Option<String> {
    let trimmed = target.trim();
    if trimmed.is_empty() {
        return None;
    }

    let routed = trimmed.split('|').next().unwrap_or(trimmed).trim();
    if routed.is_empty() {
        return None;
    }

    let logical_name = strip_offline_alias_prefix(routed).trim();
    if logical_name.is_empty() {
        return None;
    }

    Some(logical_name.to_ascii_lowercase())
}

#[allow(dead_code)]
pub fn route_recipient_indices(
    selector: &RecipientSelector,
    recipients: &[RecipientMetadata],
    zone_hook: Option<ZoneRouteHook<'_>>,
) -> Vec<usize> {
    match selector {
        RecipientSelector::Broadcast => (0..recipients.len()).collect(),
        RecipientSelector::Player(username_or_alias) => {
            let Some(target_key) = normalize_player_target(username_or_alias) else {
                return Vec::new();
            };

            recipients
                .iter()
                .enumerate()
                .filter_map(|(index, recipient)| {
                    let username_key = recipient
                        .username
                        .as_deref()
                        .and_then(normalize_player_target);
                    let char_id_key = recipient
                        .char_id
                        .as_deref()
                        .and_then(normalize_player_target);

                    (username_key.as_deref() == Some(target_key.as_str())
                        || char_id_key.as_deref() == Some(target_key.as_str()))
                    .then_some(index)
                })
                .collect()
        }
        RecipientSelector::Zone(zone_name) => {
            let Some(matches_zone) = zone_hook else {
                return Vec::new();
            };

            recipients
                .iter()
                .enumerate()
                .filter_map(|(index, recipient)| {
                    matches_zone(zone_name, recipient).then_some(index)
                })
                .collect()
        }
    }
}

impl NetworkBridgeResource {
    pub fn new(tx_to_agent: Sender<GameEvent>, rx_from_agent: Receiver<AgentCommand>) -> Self {
        Self {
            tx_to_agent,
            rx_from_agent,
        }
    }
}

pub fn spawn_mock_bridge_daemon(
    tx_to_game: Sender<AgentCommand>,
    _rx_from_game: Receiver<GameEvent>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let runtime = match tokio::runtime::Runtime::new() {
            Ok(runtime) => runtime,
            Err(error) => {
                tracing::error!(
                    "[bong][bridge] failed to create tokio runtime for mock bridge daemon: {error}"
                );
                return;
            }
        };

        tracing::info!("[bong][bridge] tokio runtime started");

        runtime.block_on(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(10)).await;

                if tx_to_game.send(AgentCommand::Heartbeat).is_err() {
                    tracing::warn!("[bong][bridge] channel to game closed; stopping daemon");
                    break;
                }
            }
        });
    })
}

#[allow(dead_code)]
fn strip_offline_alias_prefix(value: &str) -> &str {
    match value.split_once(':') {
        Some((prefix, logical_name)) if prefix.eq_ignore_ascii_case("offline") => logical_name,
        _ => value,
    }
}

#[cfg(test)]
mod server_data_tests {
    use super::*;
    use crate::schema::common::{EventKind, NarrationScope, NarrationStyle, MAX_PAYLOAD_BYTES};
    use crate::schema::inventory::{
        ContainerIdV1, ContainerSnapshotV1, EquippedInventorySnapshotV1, InventoryEventV1,
        InventoryItemViewV1, InventorySnapshotV1, InventoryWeightV1, ItemRarityV1,
        PlacedInventoryItemV1,
    };
    use crate::schema::narration::Narration;
    use crate::schema::server_data::{
        ServerDataPayloadV1, HEARTBEAT_MESSAGE, SERVER_DATA_VERSION, WELCOME_MESSAGE,
    };
    use crate::schema::world_state::{PlayerPowerBreakdown, ZoneStatusV1};
    use serde_json::json;
    use std::collections::BTreeSet;

    fn sample_player_breakdown() -> PlayerPowerBreakdown {
        PlayerPowerBreakdown {
            combat: 0.2,
            wealth: 0.4,
            social: 0.65,
            karma: 0.2,
            territory: 0.1,
        }
    }

    fn sample_inventory_item(
        instance_id: u64,
        item_id: &str,
        display_name: &str,
        rarity: ItemRarityV1,
    ) -> InventoryItemViewV1 {
        InventoryItemViewV1 {
            instance_id,
            item_id: item_id.to_string(),
            display_name: display_name.to_string(),
            grid_width: 1,
            grid_height: 1,
            weight: 0.2,
            rarity,
            description: "inventory mirror test fixture".to_string(),
            stack_count: 1,
            spirit_quality: 0.5,
            durability: 0.9,
            freshness: None,
            freshness_current: None,
            mineral_id: None,
            scroll_kind: None,
            scroll_skill_id: None,
            scroll_xp_grant: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }
    }

    fn sample_inventory_snapshot() -> InventorySnapshotV1 {
        InventorySnapshotV1 {
            revision: 12,
            containers: vec![
                ContainerSnapshotV1 {
                    id: ContainerIdV1::MainPack,
                    name: "主背包".to_string(),
                    rows: 5,
                    cols: 7,
                },
                ContainerSnapshotV1 {
                    id: ContainerIdV1::SmallPouch,
                    name: "小口袋".to_string(),
                    rows: 3,
                    cols: 3,
                },
                ContainerSnapshotV1 {
                    id: ContainerIdV1::FrontSatchel,
                    name: "前挂包".to_string(),
                    rows: 3,
                    cols: 4,
                },
            ],
            placed_items: vec![PlacedInventoryItemV1 {
                container_id: ContainerIdV1::MainPack,
                row: 0,
                col: 0,
                item: sample_inventory_item(
                    1001,
                    "starter_talisman",
                    "启程护符",
                    ItemRarityV1::Uncommon,
                ),
            }],
            equipped: EquippedInventorySnapshotV1 {
                head: None,
                chest: None,
                legs: None,
                feet: None,
                false_skin: None,
                main_hand: Some(sample_inventory_item(
                    1003,
                    "training_blade",
                    "训练短刃",
                    ItemRarityV1::Common,
                )),
                off_hand: None,
                two_hand: None,
                treasure_belt_0: Some(sample_inventory_item(
                    1005,
                    "starter_talisman",
                    "启程护符",
                    ItemRarityV1::Uncommon,
                )),
                treasure_belt_1: None,
                treasure_belt_2: None,
                treasure_belt_3: None,
            },
            hotbar: vec![
                Some(sample_inventory_item(
                    1004,
                    "healing_draught",
                    "疗伤药剂",
                    ItemRarityV1::Uncommon,
                )),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ],
            bone_coins: 57,
            weight: InventoryWeightV1 {
                current: 3.5,
                max: 50.0,
            },
            realm: "Awaken".to_string(),
            qi_current: 24.0,
            qi_max: 100.0,
            body_level: 0.18,
        }
    }

    #[test]
    fn serializes_known_payloads() {
        let payloads: Vec<(ServerDataV1, serde_json::Value)> = vec![
            (
                ServerDataV1::welcome(WELCOME_MESSAGE),
                json!({
                    "v": SERVER_DATA_VERSION,
                    "type": "welcome",
                    "message": WELCOME_MESSAGE,
                }),
            ),
            (
                ServerDataV1::heartbeat(HEARTBEAT_MESSAGE),
                json!({
                    "v": SERVER_DATA_VERSION,
                    "type": "heartbeat",
                    "message": HEARTBEAT_MESSAGE,
                }),
            ),
            (
                ServerDataV1::new(ServerDataPayloadV1::Narration {
                    narrations: vec![Narration {
                        scope: NarrationScope::Broadcast,
                        target: None,
                        text: "血谷上空乌云翻涌，一道紫雷正在酝酿。".to_string(),
                        style: NarrationStyle::SystemWarning,
                        kind: None,
                    }],
                }),
                json!({
                    "v": SERVER_DATA_VERSION,
                    "type": "narration",
                    "narrations": [{
                        "scope": "broadcast",
                        "text": "血谷上空乌云翻涌，一道紫雷正在酝酿。",
                        "style": "system_warning",
                    }],
                }),
            ),
            (
                ServerDataV1::new(ServerDataPayloadV1::ZoneInfo {
                    zone: "blood_valley".to_string(),
                    spirit_qi: 0.42,
                    danger_level: 3,
                    status: ZoneStatusV1::Normal,
                    active_events: Some(vec!["beast_tide".to_string()]),
                    perception_text: Some("灵气稀薄，引气如吸沙".to_string()),
                }),
                json!({
                    "v": SERVER_DATA_VERSION,
                    "type": "zone_info",
                    "zone": "blood_valley",
                    "spirit_qi": 0.42,
                    "danger_level": 3,
                    "status": "normal",
                    "active_events": ["beast_tide"],
                    "perception_text": "灵气稀薄，引气如吸沙",
                }),
            ),
            (
                ServerDataV1::new(ServerDataPayloadV1::EventAlert {
                    event: EventKind::ThunderTribulation,
                    message: "天劫将至，请于三十息内离开血谷中央。".to_string(),
                    zone: Some("blood_valley".to_string()),
                    duration_ticks: Some(600),
                }),
                json!({
                    "v": SERVER_DATA_VERSION,
                    "type": "event_alert",
                    "event": "thunder_tribulation",
                    "message": "天劫将至，请于三十息内离开血谷中央。",
                    "zone": "blood_valley",
                    "duration_ticks": 600,
                }),
            ),
            (
                ServerDataV1::new(ServerDataPayloadV1::PlayerState {
                    player: Some("offline:Steve".to_string()),
                    realm: "Induce".to_string(),
                    spirit_qi: 78.0,
                    karma: 0.2,
                    composite_power: 0.35,
                    breakdown: sample_player_breakdown(),
                    zone: "blood_valley".to_string(),
                    local_neg_pressure: None,
                    season_state: None,
                    social: None,
                }),
                json!({
                    "v": SERVER_DATA_VERSION,
                    "type": "player_state",
                    "player": "offline:Steve",
                    "realm": "Induce",
                    "spirit_qi": 78.0,
                    "karma": 0.2,
                    "composite_power": 0.35,
                    "breakdown": {
                        "combat": 0.2,
                        "wealth": 0.4,
                        "social": 0.65,
                        "karma": 0.2,
                        "territory": 0.1,
                    },
                    "zone": "blood_valley",
                }),
            ),
            (
                ServerDataV1::new(ServerDataPayloadV1::UiOpen {
                    ui: Some("cultivation_panel".to_string()),
                    xml: "<flow-layout><label text=\"修仙面板\"/></flow-layout>".to_string(),
                }),
                json!({
                    "v": SERVER_DATA_VERSION,
                    "type": "ui_open",
                    "ui": "cultivation_panel",
                    "xml": "<flow-layout><label text=\"修仙面板\"/></flow-layout>",
                }),
            ),
        ];

        let mut observed_payload_types = BTreeSet::new();

        for (payload, expected_json) in payloads {
            let payload_type = payload.payload_type();
            let bytes =
                serialize_server_data_payload(&payload).expect("known payload should serialize");
            let value: serde_json::Value =
                serde_json::from_slice(&bytes).expect("serialized payload should be JSON");

            assert_eq!(value, expected_json);
            assert_eq!(
                value.get("type"),
                Some(&json!(payload_type_label(payload_type.clone())))
            );

            observed_payload_types.insert(payload_type_label(payload_type));
        }

        assert_eq!(observed_payload_types.len(), 7);
        assert_eq!(
            observed_payload_types,
            BTreeSet::from([
                "welcome",
                "heartbeat",
                "narration",
                "zone_info",
                "event_alert",
                "player_state",
                "ui_open",
            ])
        );
    }

    #[test]
    fn routes_player_and_zone_targets() {
        let recipients = vec![
            RecipientMetadata {
                username: Some("Steve".to_string()),
                char_id: Some("char:101".to_string()),
                zone: Some("blood_valley".to_string()),
            },
            RecipientMetadata {
                username: Some("offline:Alex".to_string()),
                char_id: Some("char:202".to_string()),
                zone: Some("spawn".to_string()),
            },
            RecipientMetadata {
                username: None,
                char_id: Some("char:303".to_string()),
                zone: Some("blood_valley".to_string()),
            },
        ];

        let broadcast_matches =
            route_recipient_indices(&RecipientSelector::Broadcast, &recipients, None);
        assert_eq!(broadcast_matches, vec![0, 1, 2]);

        let steve_plain =
            route_recipient_indices(&RecipientSelector::player("Steve"), &recipients, None);
        let steve_alias = route_recipient_indices(
            &RecipientSelector::player("offline:Steve"),
            &recipients,
            None,
        );
        assert_eq!(steve_plain, vec![0]);
        assert_eq!(steve_alias, vec![0]);

        let alex_plain =
            route_recipient_indices(&RecipientSelector::player("Alex"), &recipients, None);
        let alex_alias = route_recipient_indices(
            &RecipientSelector::player("offline:Alex"),
            &recipients,
            None,
        );
        assert_eq!(alex_plain, vec![1]);
        assert_eq!(alex_alias, vec![1]);

        let steve_char =
            route_recipient_indices(&RecipientSelector::player("char:101"), &recipients, None);
        let hidden_char =
            route_recipient_indices(&RecipientSelector::player("char:303"), &recipients, None);
        assert_eq!(steve_char, vec![0]);
        assert_eq!(hidden_char, vec![2]);

        let zone_matches = route_recipient_indices(
            &RecipientSelector::zone("blood_valley"),
            &recipients,
            Some(&|zone_name, recipient| {
                recipient
                    .zone
                    .as_deref()
                    .is_some_and(|zone| zone.eq_ignore_ascii_case(zone_name))
            }),
        );
        assert_eq!(zone_matches, vec![0, 2]);

        let zone_without_hook =
            route_recipient_indices(&RecipientSelector::zone("blood_valley"), &recipients, None);
        assert!(zone_without_hook.is_empty());
    }

    #[test]
    fn rejects_oversize_payloads() {
        let payload = ServerDataV1::welcome("x".repeat(MAX_PAYLOAD_BYTES * 2));

        let error =
            serialize_server_data_payload(&payload).expect_err("oversized payload should fail");

        match error {
            PayloadBuildError::Oversize { size, max } => {
                assert!(size > max);
                assert_eq!(max, MAX_PAYLOAD_BYTES);
            }
            PayloadBuildError::Json(err) => {
                panic!("unexpected json error while testing oversize rejection: {err}");
            }
        }
    }

    #[test]
    fn labels_inventory_payload_types() {
        assert_eq!(
            payload_type_label(ServerDataType::InventorySnapshot),
            "inventory_snapshot"
        );
        assert_eq!(
            payload_type_label(ServerDataType::InventoryEvent),
            "inventory_event"
        );
        assert_eq!(
            payload_type_label(ServerDataType::BotanyHarvestProgress),
            "botany_harvest_progress"
        );
        assert_eq!(
            payload_type_label(ServerDataType::MiningProgress),
            "mining_progress"
        );
        assert_eq!(
            payload_type_label(ServerDataType::LumberProgress),
            "lumber_progress"
        );
        assert_eq!(
            payload_type_label(ServerDataType::BotanySkill),
            "botany_skill"
        );
        assert_eq!(
            payload_type_label(ServerDataType::RealmVisionParams),
            "realm_vision_params"
        );
        assert_eq!(
            payload_type_label(ServerDataType::SpiritualSenseTargets),
            "spiritual_sense_targets"
        );
    }

    #[test]
    fn serializes_inventory_payloads() {
        let snapshot_payload = ServerDataV1::new(ServerDataPayloadV1::InventorySnapshot(Box::new(
            sample_inventory_snapshot(),
        )));
        let snapshot_bytes = serialize_server_data_payload(&snapshot_payload)
            .expect("inventory snapshot payload should serialize");
        let snapshot_json: serde_json::Value = serde_json::from_slice(&snapshot_bytes)
            .expect("serialized inventory snapshot should decode as JSON");

        assert_eq!(
            snapshot_json.get("type"),
            Some(&json!(payload_type_label(snapshot_payload.payload_type())))
        );
        assert_eq!(snapshot_json.get("revision"), Some(&json!(12)));
        assert_eq!(snapshot_json.get("bone_coins"), Some(&json!(57)));

        let event_payload = ServerDataV1::new(ServerDataPayloadV1::InventoryEvent(Box::new(
            InventoryEventV1::Dropped {
                revision: 13,
                instance_id: 1004,
                from: crate::schema::inventory::InventoryLocationV1::Container {
                    container_id: crate::schema::inventory::ContainerIdV1::MainPack,
                    row: 0,
                    col: 0,
                },
                world_pos: [8.0, 66.0, 8.0],
                item: crate::schema::inventory::InventoryItemViewV1 {
                    instance_id: 1004,
                    item_id: "starter_talisman".to_string(),
                    display_name: "启程护符".to_string(),
                    grid_width: 1,
                    grid_height: 1,
                    weight: 0.2,
                    rarity: crate::schema::inventory::ItemRarityV1::Common,
                    description: "fixture".to_string(),
                    stack_count: 1,
                    spirit_quality: 0.5,
                    durability: 1.0,
                    freshness: None,
                    freshness_current: None,
                    mineral_id: None,
                    scroll_kind: None,
                    scroll_skill_id: None,
                    scroll_xp_grant: None,
                    charges: None,
                    forge_quality: None,
                    forge_color: None,
                    forge_side_effects: Vec::new(),
                    forge_achieved_tier: None,
                    alchemy: None,
                    lingering_owner_qi: None,
                },
            },
        )));
        let event_bytes = serialize_server_data_payload(&event_payload)
            .expect("inventory event payload should serialize");
        let event_json: serde_json::Value = serde_json::from_slice(&event_bytes)
            .expect("serialized inventory event should decode as JSON");

        assert_eq!(
            event_json.get("type"),
            Some(&json!(payload_type_label(event_payload.payload_type())))
        );
        assert_eq!(event_json.get("kind"), Some(&json!("dropped")));
        assert_eq!(event_json.get("instance_id"), Some(&json!(1004)));

        let botany_payload = ServerDataV1::new(ServerDataPayloadV1::BotanyHarvestProgress {
            session_id: "session-botany-01".to_string(),
            target_id: "plant-1".to_string(),
            target_name: "开脉草".to_string(),
            plant_kind: "ning_mai_cao".to_string(),
            mode: "manual".to_string(),
            progress: 0.5,
            auto_selectable: true,
            request_pending: false,
            interrupted: false,
            completed: false,
            detail: "晨露未散".to_string(),
            hazard_hints: Vec::new(),
            target_pos: Some([10.5, 64.0, 10.5]),
        });
        let botany_bytes = serialize_server_data_payload(&botany_payload)
            .expect("botany payload should serialize");
        let botany_json: serde_json::Value = serde_json::from_slice(&botany_bytes)
            .expect("serialized botany state should decode as JSON");

        assert_eq!(
            botany_json.get("type"),
            Some(&json!(payload_type_label(botany_payload.payload_type())))
        );
        assert_eq!(
            botany_json.get("session_id"),
            Some(&json!("session-botany-01"))
        );

        let skill_payload = ServerDataV1::new(ServerDataPayloadV1::BotanySkill {
            level: 3,
            xp: 250,
            xp_to_next_level: 400,
            auto_unlock_level: 3,
        });
        let skill_bytes = serialize_server_data_payload(&skill_payload)
            .expect("botany skill payload should serialize");
        let skill_json: serde_json::Value = serde_json::from_slice(&skill_bytes)
            .expect("serialized botany skill should decode as JSON");

        assert_eq!(
            skill_json.get("type"),
            Some(&json!(payload_type_label(skill_payload.payload_type())))
        );
        assert_eq!(skill_json.get("xp"), Some(&json!(250)));
    }
}
