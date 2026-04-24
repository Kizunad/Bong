use serde::{de::Error as _, Deserialize, Deserializer, Serialize};

const JS_SAFE_INTEGER_MAX: u64 = 9_007_199_254_740_991;
const HOTBAR_SLOT_COUNT: usize = 9;
const INVENTORY_CONTAINER_COUNT: usize = 3;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ContainerIdV1 {
    MainPack,
    SmallPouch,
    FrontSatchel,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum EquipSlotV1 {
    Head,
    Chest,
    Legs,
    Feet,
    MainHand,
    OffHand,
    TwoHand,
    #[serde(rename = "treasure_belt_0")]
    TreasureBelt0,
    #[serde(rename = "treasure_belt_1")]
    TreasureBelt1,
    #[serde(rename = "treasure_belt_2")]
    TreasureBelt2,
    #[serde(rename = "treasure_belt_3")]
    TreasureBelt3,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ItemRarityV1 {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct InventoryItemViewV1 {
    #[serde(deserialize_with = "deserialize_js_safe_integer")]
    pub instance_id: u64,
    #[serde(deserialize_with = "deserialize_non_empty_string_up_to_128")]
    pub item_id: String,
    #[serde(deserialize_with = "deserialize_non_empty_string_up_to_256")]
    pub display_name: String,
    #[serde(deserialize_with = "deserialize_grid_span")]
    pub grid_width: u8,
    #[serde(deserialize_with = "deserialize_grid_span")]
    pub grid_height: u8,
    #[serde(deserialize_with = "deserialize_non_negative_f64")]
    pub weight: f64,
    pub rarity: ItemRarityV1,
    #[serde(deserialize_with = "deserialize_string_up_to_4096")]
    pub description: String,
    #[serde(deserialize_with = "deserialize_positive_u64")]
    pub stack_count: u64,
    #[serde(deserialize_with = "deserialize_unit_interval_f64")]
    pub spirit_quality: f64,
    #[serde(deserialize_with = "deserialize_unit_interval_f64")]
    pub durability: f64,
    /// plan-shelflife-v1 §0.4 — 物品保质期 NBT；缺省视作"无时间敏感"。
    /// 为旧 client snapshot / 未挂 freshness 的物品兼容。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness: Option<crate::shelflife::Freshness>,
    /// plan-shelflife-v1 M3a — 衍生数据（snapshot emit 时由 server 预算，供 client tooltip
    /// 直接渲染；client 不需要内置 compute_* 逻辑 + DecayProfileRegistry）。
    /// `None` = freshness 字段缺失 / profile 未在 registry / 无法衍生。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness_current: Option<FreshnessDerivedV1>,
    /// plan-mineral-v1 §2.2 — 矿物来源 item NBT 携带正典 mineral_id 字符串
    /// （如 "fan_tie" / "ling_shi_zhong"），非矿物来源 item 留 None。
    /// alchemy / forge 配方校验 `material` 时按此字段比对（不依赖 vanilla item id）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mineral_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scroll_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scroll_skill_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scroll_xp_grant: Option<u32>,
}

/// plan-shelflife-v1 M3a — 衍生 freshness 数据（current_qi + track_state）。
/// 由 server snapshot emit 时调 `compute_current_qi` + `compute_track_state` 算出，
/// 塞 InventoryItemViewV1 携带到 client。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FreshnessDerivedV1 {
    /// 当下灵气 / 真元 / 药力含量。
    pub current_qi: f32,
    /// 当下路径机态 — 7 档（Fresh / Declining / Dead / Spoiled / Peaking / PastPeak /
    /// AgePostPeakSpoiled）。client M3b 由此 + current_qi/initial_qi 比率衍生 5 档显示位。
    pub track_state: crate::shelflife::TrackState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct InventoryWeightV1 {
    #[serde(deserialize_with = "deserialize_non_negative_f64")]
    pub current: f64,
    #[serde(deserialize_with = "deserialize_non_negative_f64")]
    pub max: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EquippedInventorySnapshotV1 {
    pub head: Option<InventoryItemViewV1>,
    pub chest: Option<InventoryItemViewV1>,
    pub legs: Option<InventoryItemViewV1>,
    pub feet: Option<InventoryItemViewV1>,
    pub main_hand: Option<InventoryItemViewV1>,
    pub off_hand: Option<InventoryItemViewV1>,
    pub two_hand: Option<InventoryItemViewV1>,
    pub treasure_belt_0: Option<InventoryItemViewV1>,
    pub treasure_belt_1: Option<InventoryItemViewV1>,
    pub treasure_belt_2: Option<InventoryItemViewV1>,
    pub treasure_belt_3: Option<InventoryItemViewV1>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InventoryLocationV1 {
    Container {
        container_id: ContainerIdV1,
        row: u64,
        col: u64,
    },
    Equip {
        slot: EquipSlotV1,
    },
    Hotbar {
        index: u8,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlacedInventoryItemV1 {
    pub container_id: ContainerIdV1,
    #[serde(deserialize_with = "deserialize_grid_coordinate")]
    pub row: u64,
    #[serde(deserialize_with = "deserialize_grid_coordinate")]
    pub col: u64,
    pub item: InventoryItemViewV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ContainerSnapshotV1 {
    pub id: ContainerIdV1,
    #[serde(deserialize_with = "deserialize_non_empty_string_up_to_64")]
    pub name: String,
    #[serde(deserialize_with = "deserialize_container_extent")]
    pub rows: u8,
    #[serde(deserialize_with = "deserialize_container_extent")]
    pub cols: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct InventorySnapshotV1 {
    pub revision: u64,
    #[serde(deserialize_with = "deserialize_inventory_containers")]
    pub containers: Vec<ContainerSnapshotV1>,
    pub placed_items: Vec<PlacedInventoryItemV1>,
    pub equipped: EquippedInventorySnapshotV1,
    #[serde(deserialize_with = "deserialize_hotbar")]
    pub hotbar: Vec<Option<InventoryItemViewV1>>,
    #[serde(deserialize_with = "deserialize_js_safe_integer")]
    pub bone_coins: u64,
    pub weight: InventoryWeightV1,
    #[serde(deserialize_with = "deserialize_non_empty_string_up_to_64")]
    pub realm: String,
    #[serde(deserialize_with = "deserialize_non_negative_f64")]
    pub qi_current: f64,
    #[serde(deserialize_with = "deserialize_non_negative_f64")]
    pub qi_max: f64,
    #[serde(deserialize_with = "deserialize_non_negative_f64")]
    pub body_level: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InventoryEventV1 {
    Moved {
        revision: u64,
        instance_id: u64,
        from: InventoryLocationV1,
        to: InventoryLocationV1,
    },
    Dropped {
        revision: u64,
        instance_id: u64,
        from: InventoryLocationV1,
        world_pos: [f64; 3],
        item: InventoryItemViewV1,
    },
    StackChanged {
        revision: u64,
        instance_id: u64,
        stack_count: u64,
    },
    DurabilityChanged {
        revision: u64,
        instance_id: u64,
        durability: f64,
    },
}

impl<'de> Deserialize<'de> for InventoryLocationV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawInventoryLocationV1::deserialize(deserializer)?;
        raw.try_into().map_err(D::Error::custom)
    }
}

impl<'de> Deserialize<'de> for InventoryEventV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawInventoryEventV1::deserialize(deserializer)?;
        raw.try_into().map_err(D::Error::custom)
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawInventoryLocationV1 {
    Container(RawInventoryContainerLocationV1),
    Equip(RawInventoryEquipLocationV1),
    Hotbar(RawInventoryHotbarLocationV1),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawInventoryContainerLocationV1 {
    kind: String,
    container_id: ContainerIdV1,
    #[serde(deserialize_with = "deserialize_grid_coordinate")]
    row: u64,
    #[serde(deserialize_with = "deserialize_grid_coordinate")]
    col: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawInventoryEquipLocationV1 {
    kind: String,
    slot: EquipSlotV1,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawInventoryHotbarLocationV1 {
    kind: String,
    #[serde(deserialize_with = "deserialize_hotbar_index")]
    index: u8,
}

impl TryFrom<RawInventoryLocationV1> for InventoryLocationV1 {
    type Error = String;

    fn try_from(value: RawInventoryLocationV1) -> Result<Self, Self::Error> {
        match value {
            RawInventoryLocationV1::Container(location) => {
                if location.kind != "container" {
                    return Err(format!(
                        "InventoryLocationV1.kind must be 'container', got '{}'",
                        location.kind
                    ));
                }

                Ok(Self::Container {
                    container_id: location.container_id,
                    row: location.row,
                    col: location.col,
                })
            }
            RawInventoryLocationV1::Equip(location) => {
                if location.kind != "equip" {
                    return Err(format!(
                        "InventoryLocationV1.kind must be 'equip', got '{}'",
                        location.kind
                    ));
                }

                Ok(Self::Equip {
                    slot: location.slot,
                })
            }
            RawInventoryLocationV1::Hotbar(location) => {
                if location.kind != "hotbar" {
                    return Err(format!(
                        "InventoryLocationV1.kind must be 'hotbar', got '{}'",
                        location.kind
                    ));
                }

                Ok(Self::Hotbar {
                    index: location.index,
                })
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawInventoryEventV1 {
    Moved(RawInventoryEventMovedV1),
    Dropped(RawInventoryEventDroppedV1),
    StackChanged(RawInventoryEventStackChangedV1),
    DurabilityChanged(RawInventoryEventDurabilityChangedV1),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawInventoryEventMovedV1 {
    kind: String,
    pub revision: u64,
    #[serde(deserialize_with = "deserialize_js_safe_integer")]
    pub instance_id: u64,
    pub from: InventoryLocationV1,
    pub to: InventoryLocationV1,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawInventoryEventStackChangedV1 {
    kind: String,
    pub revision: u64,
    #[serde(deserialize_with = "deserialize_js_safe_integer")]
    pub instance_id: u64,
    #[serde(deserialize_with = "deserialize_non_negative_u64")]
    pub stack_count: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawInventoryEventDroppedV1 {
    kind: String,
    pub revision: u64,
    #[serde(deserialize_with = "deserialize_js_safe_integer")]
    pub instance_id: u64,
    pub from: InventoryLocationV1,
    pub world_pos: [f64; 3],
    pub item: InventoryItemViewV1,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawInventoryEventDurabilityChangedV1 {
    kind: String,
    pub revision: u64,
    #[serde(deserialize_with = "deserialize_js_safe_integer")]
    pub instance_id: u64,
    #[serde(deserialize_with = "deserialize_unit_interval_f64")]
    pub durability: f64,
}

impl TryFrom<RawInventoryEventV1> for InventoryEventV1 {
    type Error = String;

    fn try_from(value: RawInventoryEventV1) -> Result<Self, Self::Error> {
        match value {
            RawInventoryEventV1::Moved(event) => {
                if event.kind != "moved" {
                    return Err(format!(
                        "InventoryEventV1.kind must be 'moved', got '{}'",
                        event.kind
                    ));
                }

                Ok(Self::Moved {
                    revision: event.revision,
                    instance_id: event.instance_id,
                    from: event.from,
                    to: event.to,
                })
            }
            RawInventoryEventV1::Dropped(event) => {
                if event.kind != "dropped" {
                    return Err(format!(
                        "InventoryEventV1.kind must be 'dropped', got '{}'",
                        event.kind
                    ));
                }

                Ok(Self::Dropped {
                    revision: event.revision,
                    instance_id: event.instance_id,
                    from: event.from,
                    world_pos: event.world_pos,
                    item: event.item,
                })
            }
            RawInventoryEventV1::StackChanged(event) => {
                if event.kind != "stack_changed" {
                    return Err(format!(
                        "InventoryEventV1.kind must be 'stack_changed', got '{}'",
                        event.kind
                    ));
                }

                Ok(Self::StackChanged {
                    revision: event.revision,
                    instance_id: event.instance_id,
                    stack_count: event.stack_count,
                })
            }
            RawInventoryEventV1::DurabilityChanged(event) => {
                if event.kind != "durability_changed" {
                    return Err(format!(
                        "InventoryEventV1.kind must be 'durability_changed', got '{}'",
                        event.kind
                    ));
                }

                Ok(Self::DurabilityChanged {
                    revision: event.revision,
                    instance_id: event.instance_id,
                    durability: event.durability,
                })
            }
        }
    }
}

fn deserialize_fixed_len_vec<'de, D, T, const EXPECTED: usize>(
    deserializer: D,
) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let values = Vec::<T>::deserialize(deserializer)?;
    if values.len() == EXPECTED {
        Ok(values)
    } else {
        Err(D::Error::custom(format!(
            "expected array length {EXPECTED}, got {}",
            values.len()
        )))
    }
}

fn deserialize_inventory_containers<'de, D>(
    deserializer: D,
) -> Result<Vec<ContainerSnapshotV1>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_fixed_len_vec::<D, ContainerSnapshotV1, INVENTORY_CONTAINER_COUNT>(deserializer)
}

fn deserialize_hotbar<'de, D>(deserializer: D) -> Result<Vec<Option<InventoryItemViewV1>>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_fixed_len_vec::<D, Option<InventoryItemViewV1>, HOTBAR_SLOT_COUNT>(deserializer)
}

fn deserialize_js_safe_integer<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = u64::deserialize(deserializer)?;
    if value <= JS_SAFE_INTEGER_MAX {
        Ok(value)
    } else {
        Err(D::Error::custom(format!(
            "integer must be <= {JS_SAFE_INTEGER_MAX}, got {value}"
        )))
    }
}

fn deserialize_grid_coordinate<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    u64::deserialize(deserializer)
}

fn deserialize_grid_span<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    let value = u8::deserialize(deserializer)?;
    if (1..=4).contains(&value) {
        Ok(value)
    } else {
        Err(D::Error::custom(format!(
            "grid span must be in 1..=4, got {value}"
        )))
    }
}

fn deserialize_container_extent<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    let value = u8::deserialize(deserializer)?;
    if (1..=16).contains(&value) {
        Ok(value)
    } else {
        Err(D::Error::custom(format!(
            "container extent must be in 1..=16, got {value}"
        )))
    }
}

fn deserialize_hotbar_index<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    let value = u8::deserialize(deserializer)?;
    if (0..HOTBAR_SLOT_COUNT as u8).contains(&value) {
        Ok(value)
    } else {
        Err(D::Error::custom(format!(
            "hotbar index must be in 0..={}, got {value}",
            HOTBAR_SLOT_COUNT - 1
        )))
    }
}

fn deserialize_positive_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = u64::deserialize(deserializer)?;
    if value >= 1 {
        Ok(value)
    } else {
        Err(D::Error::custom("value must be >= 1"))
    }
}

fn deserialize_non_negative_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    u64::deserialize(deserializer)
}

fn deserialize_non_negative_f64<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = f64::deserialize(deserializer)?;
    if value >= 0.0 {
        Ok(value)
    } else {
        Err(D::Error::custom(format!(
            "number must be >= 0, got {value}"
        )))
    }
}

fn deserialize_unit_interval_f64<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = f64::deserialize(deserializer)?;
    if (0.0..=1.0).contains(&value) {
        Ok(value)
    } else {
        Err(D::Error::custom(format!(
            "number must be in 0..=1, got {value}"
        )))
    }
}

fn deserialize_string_up_to_4096<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_string::<D, 0, 4096>(deserializer)
}

fn deserialize_non_empty_string_up_to_64<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_string::<D, 1, 64>(deserializer)
}

fn deserialize_non_empty_string_up_to_128<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_string::<D, 1, 128>(deserializer)
}

fn deserialize_non_empty_string_up_to_256<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_string::<D, 1, 256>(deserializer)
}

fn deserialize_bounded_string<'de, D, const MIN: usize, const MAX: usize>(
    deserializer: D,
) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    let len = value.chars().count();
    if (MIN..=MAX).contains(&len) {
        Ok(value)
    } else {
        Err(D::Error::custom(format!(
            "string length must be in {MIN}..={MAX}, got {len}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
    use serde_json::{json, Value};

    const INVENTORY_SNAPSHOT_SAMPLE: &str =
        include_str!("../../../agent/packages/schema/samples/inventory-snapshot.sample.json");
    const INVENTORY_EVENT_SAMPLE: &str =
        include_str!("../../../agent/packages/schema/samples/inventory-event.sample.json");
    const INVENTORY_EVENT_INVALID_UNKNOWN_KIND_SAMPLE: &str = include_str!(
        "../../../agent/packages/schema/samples/inventory-event.invalid-unknown-kind.sample.json"
    );
    const SERVER_DATA_INVENTORY_SNAPSHOT_SAMPLE: &str = include_str!(
        "../../../agent/packages/schema/samples/server-data.inventory-snapshot.sample.json"
    );
    const SERVER_DATA_INVENTORY_EVENT_SAMPLE: &str = include_str!(
        "../../../agent/packages/schema/samples/server-data.inventory-event.sample.json"
    );

    fn sample_value(json_text: &str) -> Value {
        serde_json::from_str(json_text).expect("sample should parse into serde_json::Value")
    }

    #[test]
    fn deserialize_inventory_snapshot_sample() {
        let snapshot: InventorySnapshotV1 = serde_json::from_str(INVENTORY_SNAPSHOT_SAMPLE)
            .expect("inventory-snapshot.sample.json should deserialize into InventorySnapshotV1");

        assert_eq!(snapshot.revision, 12);
        assert_eq!(snapshot.containers.len(), 3);
        assert_eq!(snapshot.placed_items.len(), 2);
        assert_eq!(snapshot.placed_items[0].item.item_id, "starter_talisman");
        assert_eq!(snapshot.hotbar.len(), HOTBAR_SLOT_COUNT);
        assert_eq!(snapshot.bone_coins, 57);
        assert_eq!(snapshot.realm, "qi_refining_1");
    }

    #[test]
    fn inventory_snapshot_roundtrip_preserves_content() {
        let snapshot: InventorySnapshotV1 = serde_json::from_str(INVENTORY_SNAPSHOT_SAMPLE)
            .expect("inventory snapshot sample should deserialize");
        let reserialized =
            serde_json::to_string(&snapshot).expect("inventory snapshot should serialize back");
        let roundtrip: InventorySnapshotV1 = serde_json::from_str(&reserialized)
            .expect("serialized inventory snapshot should deserialize again");

        assert_eq!(
            serde_json::to_value(&snapshot).expect("snapshot should convert to value"),
            serde_json::to_value(&roundtrip).expect("roundtrip should convert to value"),
        );
    }

    #[test]
    fn inventory_snapshot_rejects_unknown_top_level_field() {
        let mut value = sample_value(INVENTORY_SNAPSHOT_SAMPLE);
        value["realm_clock"] = json!(99);

        assert!(serde_json::from_value::<InventorySnapshotV1>(value).is_err());
    }

    #[test]
    fn inventory_snapshot_rejects_unknown_nested_item_field() {
        let mut value = sample_value(INVENTORY_SNAPSHOT_SAMPLE);
        value["placed_items"][0]["item"]["attunement"] = json!(0.8);

        assert!(serde_json::from_value::<InventorySnapshotV1>(value).is_err());
    }

    #[test]
    fn deserialize_inventory_event_sample() {
        let event: InventoryEventV1 = serde_json::from_str(INVENTORY_EVENT_SAMPLE)
            .expect("inventory-event.sample.json should deserialize into InventoryEventV1");

        match event {
            InventoryEventV1::Moved {
                revision,
                instance_id,
                from,
                to,
            } => {
                assert_eq!(revision, 13);
                assert_eq!(instance_id, 1001);
                assert_eq!(
                    from,
                    InventoryLocationV1::Container {
                        container_id: ContainerIdV1::MainPack,
                        row: 0,
                        col: 0,
                    }
                );
                assert_eq!(to, InventoryLocationV1::Hotbar { index: 1 });
            }
            other => panic!("expected moved inventory event, got {other:?}"),
        }
    }

    #[test]
    fn inventory_event_roundtrip_preserves_content() {
        let event: InventoryEventV1 = serde_json::from_str(INVENTORY_EVENT_SAMPLE)
            .expect("inventory event sample should deserialize");
        let reserialized =
            serde_json::to_string(&event).expect("inventory event should serialize back");
        let roundtrip: InventoryEventV1 = serde_json::from_str(&reserialized)
            .expect("serialized inventory event should deserialize again");

        assert_eq!(
            serde_json::to_value(&event).expect("event should convert to value"),
            serde_json::to_value(&roundtrip).expect("roundtrip should convert to value"),
        );
    }

    #[test]
    fn inventory_event_rejects_unknown_field() {
        let mut value = sample_value(INVENTORY_EVENT_SAMPLE);
        value["cooldown_ticks"] = json!(40);

        assert!(serde_json::from_value::<InventoryEventV1>(value).is_err());
    }

    #[test]
    fn inventory_event_rejects_unsupported_kind_sample() {
        assert!(serde_json::from_str::<InventoryEventV1>(
            INVENTORY_EVENT_INVALID_UNKNOWN_KIND_SAMPLE
        )
        .is_err());
    }

    #[test]
    fn inventory_event_dropped_roundtrip_preserves_content() {
        let event = InventoryEventV1::Dropped {
            revision: 21,
            instance_id: 2002,
            from: InventoryLocationV1::Container {
                container_id: ContainerIdV1::MainPack,
                row: 0,
                col: 0,
            },
            world_pos: [8.0, 66.0, 8.0],
            item: InventoryItemViewV1 {
                instance_id: 2002,
                item_id: "starter_talisman".to_string(),
                display_name: "启程护符".to_string(),
                grid_width: 1,
                grid_height: 1,
                weight: 0.2,
                rarity: ItemRarityV1::Common,
                description: String::new(),
                stack_count: 1,
                spirit_quality: 1.0,
                durability: 1.0,
                freshness: None,
                freshness_current: None,
                mineral_id: None,
                scroll_kind: None,
                scroll_skill_id: None,
                scroll_xp_grant: None,
            },
        };
        let reserialized = serde_json::to_string(&event).expect("dropped event should serialize");
        let roundtrip: InventoryEventV1 =
            serde_json::from_str(&reserialized).expect("dropped event should deserialize");

        assert_eq!(event, roundtrip);
    }

    #[test]
    fn deserialize_server_data_inventory_snapshot_sample() {
        let payload: ServerDataV1 = serde_json::from_str(SERVER_DATA_INVENTORY_SNAPSHOT_SAMPLE)
            .expect("server-data inventory snapshot sample should deserialize into ServerDataV1");

        match payload.payload {
            ServerDataPayloadV1::InventorySnapshot(snapshot) => {
                assert_eq!(snapshot.revision, 12);
                assert_eq!(snapshot.placed_items[0].item.item_id, "starter_talisman");
            }
            other => panic!("expected inventory_snapshot payload, got {other:?}"),
        }
    }

    #[test]
    fn deserialize_server_data_inventory_event_sample() {
        let payload: ServerDataV1 = serde_json::from_str(SERVER_DATA_INVENTORY_EVENT_SAMPLE)
            .expect("server-data inventory event sample should deserialize into ServerDataV1");

        match payload.payload {
            ServerDataPayloadV1::InventoryEvent(event) => match event {
                InventoryEventV1::Dropped {
                    revision,
                    instance_id,
                    from,
                    world_pos,
                    item,
                } => {
                    assert_eq!(revision, 13);
                    assert_eq!(instance_id, 1004);
                    assert_eq!(world_pos, [8.0, 66.0, 8.0]);
                    assert_eq!(item.item_id, "starter_talisman");
                    match from {
                        InventoryLocationV1::Container {
                            container_id,
                            row,
                            col,
                        } => {
                            assert_eq!(container_id, ContainerIdV1::MainPack);
                            assert_eq!(row, 0);
                            assert_eq!(col, 0);
                        }
                        other => panic!("expected container source, got {other:?}"),
                    }
                }
                other => panic!("expected dropped inventory event, got {other:?}"),
            },
            other => panic!("expected inventory_event payload, got {other:?}"),
        }
    }

    #[test]
    fn server_data_inventory_samples_roundtrip_preserves_content() {
        for sample in [
            SERVER_DATA_INVENTORY_SNAPSHOT_SAMPLE,
            SERVER_DATA_INVENTORY_EVENT_SAMPLE,
        ] {
            let payload: ServerDataV1 = serde_json::from_str(sample)
                .expect("server data inventory sample should deserialize");
            let reserialized = serde_json::to_string(&payload)
                .expect("server data inventory sample should serialize");
            let roundtrip: ServerDataV1 = serde_json::from_str(&reserialized)
                .expect("serialized server data inventory sample should deserialize again");

            assert_eq!(
                serde_json::to_value(&payload).expect("payload should convert to value"),
                serde_json::to_value(&roundtrip).expect("roundtrip should convert to value"),
            );
        }
    }

    #[test]
    fn server_data_inventory_samples_reject_wrong_version() {
        let mut snapshot = sample_value(SERVER_DATA_INVENTORY_SNAPSHOT_SAMPLE);
        snapshot["v"] = json!(2);
        assert!(serde_json::from_value::<ServerDataV1>(snapshot).is_err());

        let mut event = sample_value(SERVER_DATA_INVENTORY_EVENT_SAMPLE);
        event["v"] = json!(9);
        assert!(serde_json::from_value::<ServerDataV1>(event).is_err());
    }

    #[test]
    fn server_data_inventory_samples_reject_unknown_field() {
        let mut snapshot = sample_value(SERVER_DATA_INVENTORY_SNAPSHOT_SAMPLE);
        snapshot["realm_clock"] = json!(99);
        assert!(serde_json::from_value::<ServerDataV1>(snapshot).is_err());

        let mut event = sample_value(SERVER_DATA_INVENTORY_EVENT_SAMPLE);
        event["extra_delta"] = json!(true);
        assert!(serde_json::from_value::<ServerDataV1>(event).is_err());
    }

    // =========== plan-shelflife-v1 M1 — InventoryItemViewV1.freshness ===========

    #[test]
    fn item_view_freshness_legacy_json_without_field_defaults_to_none() {
        // 旧 client snapshot 不带 freshness 字段 → 应能正确反序列化为 None。
        let legacy = json!({
            "instance_id": 42,
            "item_id": "ling_shi_fan",
            "display_name": "凡品灵石",
            "grid_width": 1,
            "grid_height": 1,
            "weight": 0.5,
            "rarity": "common",
            "description": "末法残石",
            "stack_count": 1,
            "spirit_quality": 0.7,
            "durability": 1.0,
        });

        let view: InventoryItemViewV1 =
            serde_json::from_value(legacy).expect("legacy snapshot must deserialize");
        assert!(view.freshness.is_none());
    }

    #[test]
    fn item_view_freshness_some_roundtrip_preserves_all_fields() {
        use crate::shelflife::{DecayProfileId, DecayTrack, Freshness};

        let view = InventoryItemViewV1 {
            instance_id: 42,
            item_id: "ling_shi_fan".to_string(),
            display_name: "凡品灵石".to_string(),
            grid_width: 1,
            grid_height: 1,
            weight: 0.5,
            rarity: ItemRarityV1::Common,
            description: "末法残石".to_string(),
            stack_count: 1,
            spirit_quality: 0.7,
            durability: 1.0,
            freshness: Some(Freshness {
                created_at_tick: 12345,
                initial_qi: 8.0,
                track: DecayTrack::Decay,
                profile: DecayProfileId::new("ling_shi_fan_v1"),
                frozen_accumulated: 200,
                frozen_since_tick: Some(1000),
            }),
            freshness_current: None,
            mineral_id: Some("ling_shi_fan".to_string()),
            scroll_kind: None,
            scroll_skill_id: None,
            scroll_xp_grant: None,
        };

        let json = serde_json::to_string(&view).expect("serialize");
        let back: InventoryItemViewV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, view);
    }

    #[test]
    fn item_view_freshness_none_omitted_from_serialization() {
        // skip_serializing_if = "Option::is_none" — None 时不写入 JSON，wire 字节减少。
        let view = InventoryItemViewV1 {
            instance_id: 1,
            item_id: "iron_axe".to_string(),
            display_name: "凡铁斧".to_string(),
            grid_width: 1,
            grid_height: 2,
            weight: 1.5,
            rarity: ItemRarityV1::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 0.0,
            durability: 1.0,
            freshness: None,
            freshness_current: None,
            mineral_id: None,
            scroll_kind: None,
            scroll_skill_id: None,
            scroll_xp_grant: None,
        };

        let json = serde_json::to_string(&view).expect("serialize");
        assert!(
            !json.contains("freshness"),
            "freshness=None should be skipped, got: {json}"
        );
    }

    #[test]
    fn item_view_freshness_legacy_json_missing_frozen_fields_defaults() {
        // Freshness 字段缺 frozen_accumulated / frozen_since_tick（v0 NBT）
        // 应经 #[serde(default)] 默认回 0 / None。
        let json = json!({
            "instance_id": 42,
            "item_id": "ling_shi_fan",
            "display_name": "凡品灵石",
            "grid_width": 1,
            "grid_height": 1,
            "weight": 0.5,
            "rarity": "common",
            "description": "",
            "stack_count": 1,
            "spirit_quality": 0.7,
            "durability": 1.0,
            "freshness": {
                "created_at_tick": 0,
                "initial_qi": 8.0,
                "track": "Decay",
                "profile": "ling_shi_fan_v1",
            },
        });

        let view: InventoryItemViewV1 = serde_json::from_value(json)
            .expect("legacy Freshness (without frozen_*) must deserialize");
        let f = view.freshness.expect("freshness should be Some");
        assert_eq!(f.frozen_accumulated, 0);
        assert!(f.frozen_since_tick.is_none());
    }

    // =========== plan-mineral-v1 §2.2 — InventoryItemViewV1.mineral_id ===========

    #[test]
    fn item_view_mineral_id_legacy_json_without_field_defaults_to_none() {
        let legacy = json!({
            "instance_id": 7,
            "item_id": "starter_talisman",
            "display_name": "启程护符",
            "grid_width": 1,
            "grid_height": 1,
            "weight": 0.2,
            "rarity": "common",
            "description": "",
            "stack_count": 1,
            "spirit_quality": 0.0,
            "durability": 1.0,
        });
        let view: InventoryItemViewV1 =
            serde_json::from_value(legacy).expect("legacy snapshot must deserialize");
        assert!(view.mineral_id.is_none());
    }

    #[test]
    fn item_view_mineral_id_some_roundtrip_preserves_value() {
        let view = InventoryItemViewV1 {
            instance_id: 100,
            item_id: "ore_drop_fan_tie".to_string(),
            display_name: "凡铁".to_string(),
            grid_width: 1,
            grid_height: 1,
            weight: 0.4,
            rarity: ItemRarityV1::Common,
            description: String::new(),
            stack_count: 3,
            spirit_quality: 0.0,
            durability: 1.0,
            freshness: None,
            freshness_current: None,
            mineral_id: Some("fan_tie".to_string()),
            scroll_kind: None,
            scroll_skill_id: None,
            scroll_xp_grant: None,
        };
        let json = serde_json::to_string(&view).expect("serialize");
        assert!(json.contains("\"mineral_id\":\"fan_tie\""));
        let back: InventoryItemViewV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, view);
    }

    #[test]
    fn item_view_mineral_id_none_omitted_from_serialization() {
        let view = InventoryItemViewV1 {
            instance_id: 1,
            item_id: "iron_axe".to_string(),
            display_name: "凡铁斧".to_string(),
            grid_width: 1,
            grid_height: 2,
            weight: 1.5,
            rarity: ItemRarityV1::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 0.0,
            durability: 1.0,
            freshness: None,
            freshness_current: None,
            mineral_id: None,
            scroll_kind: None,
            scroll_skill_id: None,
            scroll_xp_grant: None,
        };
        let json = serde_json::to_string(&view).expect("serialize");
        assert!(
            !json.contains("mineral_id"),
            "mineral_id=None should be skipped, got: {json}"
        );
    }

    #[test]
    fn item_view_freshness_invalid_track_rejected() {
        // 枚举字段必须严格匹配 DecayTrack 三值之一
        let bad = json!({
            "instance_id": 1,
            "item_id": "x",
            "display_name": "x",
            "grid_width": 1,
            "grid_height": 1,
            "weight": 0.0,
            "rarity": "common",
            "description": "",
            "stack_count": 1,
            "spirit_quality": 0.0,
            "durability": 1.0,
            "freshness": {
                "created_at_tick": 0,
                "initial_qi": 1.0,
                "track": "BogusTrack",
                "profile": "x",
            },
        });
        assert!(serde_json::from_value::<InventoryItemViewV1>(bad).is_err());
    }
}
