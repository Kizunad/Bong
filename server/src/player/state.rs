//! 玩家持久化切片的读写 facade。
//!
//! 本模块把既有 SQLite 行聚合为玩家运行时快照，并保留每个切片的载入 provenance。
//! 载入失败时运行时仍可使用安全默认值，但对应写出口会从 [`WriteSet`] 中移除，避免
//! 默认值覆盖未知的 durable row；功法、身体部位等冻结域仍由各自旧路径负责。

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use valence::prelude::{bevy_ecs, Component, DVec3, Resource};

use crate::coffin::CoffinGrade;
use crate::combat::components::{QuickSlotBindings, SkillBarBindings, SkillSlot, StatusEffects};
use crate::craft::CraftSession;
use crate::cultivation::components::{Cultivation, Realm};
use crate::cultivation::known_techniques::{KnownTechniques, TechniqueRegistry};
use crate::cultivation::lifespan::{
    lifespan_delta_years_for_real_seconds, LifespanComponent, LIFESPAN_OFFLINE_MULTIPLIER,
};
use crate::inventory::{DroppedLootEntry, PlayerInventory, MAX_DURABLE_DROPPED_LOOT_ENTRIES};
use crate::persistence::{ZoneRuntimeRecord, DEFAULT_DATABASE_PATH, SQLITE_BUSY_TIMEOUT_MS};
use crate::player::spawn_selector::SpawnPurpose;
use crate::qi_physics::ledger::WorldQiAccount;
use crate::schema::cultivation::realm_to_string;
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};
use crate::schema::social::PlayerSocialSnapshotV1;
use crate::schema::world_state::PlayerPowerBreakdown;
use crate::skill::components::SkillSet;
use crate::skill::config::SkillConfig;
use crate::world::dimension::DimensionKind;

pub const DEFAULT_PLAYER_DATA_DIR: &str = "data/players";

// plan-layered-equip-v1 P0.6（决议 #4）— inventory schema 内容版本。
// v1 = equipped 每槽单件 ItemInstance；v2 = SlotContents{worn:Vec, held:Option}。
// PLAYER_ROW_SCHEMA_VERSION bump 到 2：load 时 schema_version < 2 触发 migrate_equipped_v1_to_v2。
pub(crate) const PLAYER_ROW_SCHEMA_VERSION: i32 = 2;
const INVENTORY_SCHEMA_VERSION: i32 = 2;
const DEFAULT_INVENTORY_JSON: &str = "null";
const DROPPED_LOOT_ID_QUERY_BATCH_SIZE: usize = 900;
const MIN_SAFE_PLAYER_Y: f64 = crate::world::terrain::MIN_Y as f64;
const MAX_SAFE_PLAYER_Y: f64 =
    (crate::world::terrain::MIN_Y + crate::world::terrain::WORLD_HEIGHT as i32 - 1) as f64;

/// 参与玩家聚合写屏障的持久化切片。
///
/// `SkillSet` 与 `Wounds` 按 RF-11 冻结，不在此枚举中；`KnownTechniques` 仍由既有
/// canonical adapter 独立接管，但保留枚举项让调用方能描述完整的写集合。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum PlayerSlice {
    /// 玩家核心数值与当前角色键。
    Core,
    /// 最后位置与维度。
    Position,
    /// 背包与装备布局。
    Inventory,
    /// 寿命与棺材状态。
    Lifespan,
    /// 可恢复的工位会话。
    Craft,
    /// HUD 快捷栏与 UI 偏好。
    UiPrefs,
    /// 跨重连保留的长期状态效果。
    LongTermBuff,
    /// 独立的身份档案键。
    Identity,
    /// 已知功法的 canonical adapter slice。
    KnownTechniques,
}

/// 玩家切片的载入 provenance；`Failed` 表示不能证明 durable row 可安全覆盖。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerSliceLoadStatus {
    /// 数据库中没有该玩家的行，可用新玩家默认值建立。
    Missing,
    /// 行存在且已成功解码、校验。
    Loaded,
    /// 读取、解码或校验失败，禁止默认值覆盖原行。
    Failed,
}

/// 聚合 writer 当前获准更新的 durable row 集合。
///
/// 某个切片载入失败只会移除该切片；运行时可以继续使用默认值，但未知的原始行保持
/// 不动。这就是 `WriteSet omit` 契约。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct WriteSet(u16);

impl WriteSet {
    const CORE: u16 = 1 << 0;
    const POSITION: u16 = 1 << 1;
    const INVENTORY: u16 = 1 << 2;
    const LIFESPAN: u16 = 1 << 3;
    const CRAFT: u16 = 1 << 4;
    const UI_PREFS: u16 = 1 << 5;
    const LONG_TERM_BUFF: u16 = 1 << 6;
    const IDENTITY: u16 = 1 << 7;
    const KNOWN_TECHNIQUES: u16 = 1 << 8;

    /// 创建不允许写回任何切片的集合。
    pub const fn empty() -> Self {
        Self(0)
    }

    /// 创建允许写回所有本卡管理切片的集合。
    pub const fn all() -> Self {
        Self(
            Self::CORE
                | Self::POSITION
                | Self::INVENTORY
                | Self::LIFESPAN
                | Self::CRAFT
                | Self::UI_PREFS
                | Self::LONG_TERM_BUFF
                | Self::IDENTITY
                | Self::KNOWN_TECHNIQUES,
        )
    }

    const fn bit(slice: PlayerSlice) -> u16 {
        match slice {
            PlayerSlice::Core => Self::CORE,
            PlayerSlice::Position => Self::POSITION,
            PlayerSlice::Inventory => Self::INVENTORY,
            PlayerSlice::Lifespan => Self::LIFESPAN,
            PlayerSlice::Craft => Self::CRAFT,
            PlayerSlice::UiPrefs => Self::UI_PREFS,
            PlayerSlice::LongTermBuff => Self::LONG_TERM_BUFF,
            PlayerSlice::Identity => Self::IDENTITY,
            PlayerSlice::KnownTechniques => Self::KNOWN_TECHNIQUES,
        }
    }

    /// 判断指定切片是否仍在写集合中。
    pub const fn contains(self, slice: PlayerSlice) -> bool {
        self.0 & Self::bit(slice) != 0
    }

    /// 从写集合中移除一个切片，保留其他切片的资格。
    pub const fn omit(self, slice: PlayerSlice) -> Self {
        Self(self.0 & !Self::bit(slice))
    }

    /// 将一个已经确认可写的切片加入集合。
    pub const fn include(self, slice: PlayerSlice) -> Self {
        Self(self.0 | Self::bit(slice))
    }
}

/// 与运行时 fallback 值一起挂在 ECS 实体上的切片载入 provenance。
#[derive(Debug, Clone, Component)]
pub struct PlayerSliceLoadGuard {
    statuses: BTreeMap<PlayerSlice, PlayerSliceLoadStatus>,
    write_set: WriteSet,
    failures: HashMap<PlayerSlice, String>,
}

impl PlayerSliceLoadGuard {
    /// 返回切片的载入 provenance；未知项按失败处理以保持 fail-closed。
    pub fn status(&self, slice: PlayerSlice) -> PlayerSliceLoadStatus {
        self.statuses
            .get(&slice)
            .copied()
            .unwrap_or(PlayerSliceLoadStatus::Failed)
    }

    /// 返回当前聚合 writer 可使用的写集合。
    pub const fn write_set(&self) -> WriteSet {
        self.write_set
    }

    /// 返回失败切片的诊断文本（若有）。
    pub fn failure(&self, slice: PlayerSlice) -> Option<&str> {
        self.failures.get(&slice).map(String::as_str)
    }

    fn new(
        statuses: BTreeMap<PlayerSlice, PlayerSliceLoadStatus>,
        failures: HashMap<PlayerSlice, String>,
    ) -> Self {
        let mut write_set = WriteSet::empty();
        for (slice, status) in &statuses {
            if !matches!(status, PlayerSliceLoadStatus::Failed) {
                write_set = write_set.include(*slice);
            }
        }
        Self {
            statuses,
            write_set,
            failures,
        }
    }
}

#[derive(Clone, Debug, Component, Serialize, Deserialize, PartialEq)]
pub struct PlayerState {
    pub karma: f64,
    pub inventory_score: f64,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            karma: 0.0,
            inventory_score: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub(crate) struct PlayerUiPrefs {
    #[serde(default)]
    pub dash_skill_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_quick_slot_instances")]
    pub quick_slots: [Option<u64>; QuickSlotBindings::SLOT_COUNT],
    #[serde(default)]
    pub skill_bar: [SkillSlotPersist; SkillBarBindings::SLOT_COUNT],
    #[serde(default)]
    pub skill_configs: BTreeMap<String, SkillConfig>,
}

fn deserialize_quick_slot_instances<'de, D>(
    deserializer: D,
) -> Result<[Option<u64>; QuickSlotBindings::SLOT_COUNT], D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum PersistedQuickSlot {
        Instance(u64),
        LegacyTemplate(String),
    }

    let entries: [Option<PersistedQuickSlot>; QuickSlotBindings::SLOT_COUNT] =
        Deserialize::deserialize(deserializer)?;
    Ok(entries.map(|entry| match entry {
        Some(PersistedQuickSlot::Instance(instance_id)) => Some(instance_id),
        Some(PersistedQuickSlot::LegacyTemplate(legacy_template)) => {
            let _ = legacy_template;
            None
        }
        None => None,
    }))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum SkillSlotPersist {
    #[default]
    Empty,
    Item {
        template_id: String,
    },
    Skill {
        skill_id: String,
    },
}

impl PlayerUiPrefs {
    pub(crate) fn quick_slot_bindings(
        &self,
        inventory: Option<&PlayerInventory>,
    ) -> QuickSlotBindings {
        let mut bindings = QuickSlotBindings::default();
        let Some(inventory) = inventory else {
            return bindings;
        };

        for (slot, instance_id) in self.quick_slots.iter().enumerate() {
            let Some(instance_id) = *instance_id else {
                continue;
            };
            if crate::inventory::inventory_item_by_instance_borrow(inventory, instance_id).is_some()
            {
                bindings.set(slot as u8, Some(instance_id));
            }
        }
        bindings
    }

    /// Remove persisted skill-bar entries that are no longer valid generic actions.
    /// Dedicated-input techniques have their own C2S path and must not be rebound through
    /// the skill bar after reconnect; unknown ids are cleared as well.
    pub(crate) fn sanitize_skill_bar_bindings(&mut self, registry: &TechniqueRegistry) -> bool {
        let mut changed = false;
        for persist in &mut self.skill_bar {
            let SkillSlotPersist::Skill { skill_id } = persist else {
                continue;
            };
            let invalid = registry
                .get(skill_id)
                .is_none_or(|definition| definition.input_kind() == "dedicated");
            if invalid {
                *persist = SkillSlotPersist::Empty;
                changed = true;
            }
        }
        changed
    }

    pub(crate) fn skill_bar_bindings(
        &self,
        inventory: Option<&PlayerInventory>,
        registry: Option<&TechniqueRegistry>,
    ) -> SkillBarBindings {
        let mut bindings = SkillBarBindings {
            dash_skill_id: self
                .dash_skill_id
                .as_ref()
                .filter(|id| {
                    registry.is_some_and(|registry| {
                        registry
                            .get(id)
                            .is_some_and(|definition| definition.input_kind() == "dash")
                    })
                })
                .cloned(),
            ..Default::default()
        };
        for (slot, persist) in self.skill_bar.iter().enumerate() {
            let slot_value = match persist {
                SkillSlotPersist::Empty => SkillSlot::Empty,
                SkillSlotPersist::Item { template_id } => inventory
                    .and_then(|inventory| {
                        first_inventory_instance_for_template(inventory, template_id)
                    })
                    .map(|instance_id| SkillSlot::Item { instance_id })
                    .unwrap_or_default(),
                SkillSlotPersist::Skill { skill_id } => {
                    let valid = registry.is_none_or(|registry| {
                        registry
                            .get(skill_id)
                            .is_some_and(|definition| definition.input_kind() != "dedicated")
                    });
                    if valid {
                        SkillSlot::Skill {
                            skill_id: skill_id.clone(),
                        }
                    } else {
                        SkillSlot::Empty
                    }
                }
            };
            bindings.set(slot as u8, slot_value);
        }
        bindings
    }
}

fn first_inventory_instance_for_template(
    inventory: &PlayerInventory,
    template_id: &str,
) -> Option<u64> {
    for container in &inventory.containers {
        if let Some(placed) = container
            .items
            .iter()
            .find(|placed| placed.instance.template_id == template_id)
        {
            return Some(placed.instance.instance_id);
        }
    }
    if let Some(item) = inventory
        .hotbar
        .iter()
        .flatten()
        .find(|item| item.template_id == template_id)
    {
        return Some(item.instance_id);
    }
    inventory
        .equipped
        .values()
        .flat_map(|s| s.iter_all())
        .find(|item| item.template_id == template_id)
        .map(|item| item.instance_id)
}

#[derive(Debug, Clone)]
pub struct LoadedPlayerSlices {
    pub state: PlayerState,
    pub position: [f64; 3],
    pub last_dimension: DimensionKind,
    pub inventory: Option<PlayerInventory>,
    pub craft_session: Option<CraftSession>,
    pub lifespan: Option<LifespanComponent>,
    /// 已成功载入的长期状态效果；`None` 表示没有 durable row 或载入失败。
    pub status_effects: Option<StatusEffects>,
    pub in_coffin: bool,
    /// 棺材档级：Some(grade) = 在棺内 + 档级；None = 不在棺内（与 in_coffin=false 语义对齐）
    pub coffin_grade: Option<CoffinGrade>,
    pub skill_set: SkillSet,
    pub known_techniques: LoadedKnownTechniques,
    pub(crate) ui_prefs: PlayerUiPrefs,
    /// 供断线和关服聚合 writer 使用的载入 provenance。
    pub load_guard: PlayerSliceLoadGuard,
}

/// 功法聚合加载结果。`LoadFailed` 表示持久化状态无法可靠读取：行存在但读取/解析失败
/// （JSON 损坏、SELECT 报错），或连接都打不开导致**行状态完全不可知**——两种情况都
/// 绝不允许用 `KnownTechniques::default()` 覆盖写回（会把玩家全部功法+熟练度
/// 永久清零）。production join 由 canonical persistence adapter 保留 failed provenance、挂
/// `KnownTechniquesLoadFailed` 并统一阻断 Changed/disconnect/shutdown 写出口；仍消费本聚合
/// API 的调用方也必须保留同一写保护语义。唯一能确认「无数据」的是连接成功且查到无行
/// （真新玩家），归入 `Loaded(default)`，可正常写回。
#[derive(Debug, Clone, PartialEq)]
pub enum LoadedKnownTechniques {
    Loaded(KnownTechniques),
    LoadFailed,
    /// 本次聚合加载主动跳过功法；canonical persistence slice 负责独立加载。
    /// 该状态不携带可写回的数据，调用方不得将其解释为空功法表。
    NotLoaded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerExportBundle {
    pub kind: String,
    pub username: String,
    pub current_char_id: String,
    pub state: PlayerState,
    pub position: [f64; 3],
    #[serde(default)]
    pub last_dimension: DimensionKind,
    pub inventory: Option<PlayerInventory>,
    pub skill_set: SkillSet,
    #[serde(default)]
    pub known_techniques: KnownTechniques,
    pub ui_prefs: serde_json::Value,
}

impl PlayerState {
    pub fn normalized(&self) -> Self {
        Self {
            karma: self.karma.clamp(-1.0, 1.0),
            inventory_score: clamp_unit(self.inventory_score),
        }
    }

    pub fn power_breakdown(&self, cultivation: &Cultivation) -> PlayerPowerBreakdown {
        let normalized = self.normalized();
        let realm_score = realm_progress_score(cultivation.realm);
        let qi_ratio = ratio_score(cultivation.qi_current, cultivation.qi_max);
        let wealth = clamp_unit(normalized.inventory_score);
        let karma_alignment = ((normalized.karma + 1.0) * 0.5).clamp(0.0, 1.0);
        let karma_influence = normalized.karma.abs().clamp(0.0, 1.0);

        PlayerPowerBreakdown {
            combat: clamp_unit(realm_score * 0.6 + qi_ratio * 0.4),
            wealth,
            social: clamp_unit(realm_score * 0.6 + karma_alignment * 0.4),
            karma: karma_influence,
            territory: clamp_unit(realm_score * 0.5 + wealth * 0.5),
        }
    }

    pub fn composite_power(&self, cultivation: &Cultivation) -> f64 {
        let breakdown = self.power_breakdown(cultivation);

        clamp_unit(
            breakdown.combat * 0.4
                + breakdown.wealth * 0.15
                + breakdown.social * 0.15
                + breakdown.karma * 0.15
                + breakdown.territory * 0.15,
        )
    }

    /// `zone_spirit_qi`：plan-wire-format-bridge-v1 P3/RC6 —— 调用方从 `ZoneRegistry` 查得的
    /// 当前 zone 灵气浓度（`Zone::spirit_qi`）。此前该字段在 proto 里根本不存在，client
    /// `PlayerStateViewModel.zoneSpiritQiNormalized()` 恒为 NaN 归一化默认值。
    /// 无法解析 zone（stale/未知 zone 名）时传 `None`，client 端已有默认值 clamp。
    pub fn server_payload_with_social_and_local_pressure(
        &self,
        cultivation: &Cultivation,
        player: Option<String>,
        zone: impl Into<String>,
        social: Option<PlayerSocialSnapshotV1>,
        local_neg_pressure: Option<f32>,
        zone_spirit_qi: Option<f64>,
    ) -> ServerDataV1 {
        let normalized = self.normalized();
        let breakdown = normalized.power_breakdown(cultivation);
        let composite_power = clamp_unit(
            breakdown.combat * 0.4
                + breakdown.wealth * 0.15
                + breakdown.social * 0.15
                + breakdown.karma * 0.15
                + breakdown.territory * 0.15,
        );

        ServerDataV1::new(ServerDataPayloadV1::PlayerState {
            player,
            realm: realm_to_string(cultivation.realm).to_string(),
            spirit_qi: cultivation.qi_current,
            spirit_qi_max: cultivation.qi_max,
            karma: normalized.karma,
            composite_power,
            breakdown,
            zone: zone.into(),
            local_neg_pressure,
            season_state: None,
            social,
            zone_spirit_qi,
        })
    }
}

#[derive(Debug, Clone)]
pub struct PlayerStatePersistence {
    data_dir: PathBuf,
    db_path: PathBuf,
}

impl Default for PlayerStatePersistence {
    fn default() -> Self {
        Self::new(DEFAULT_PLAYER_DATA_DIR)
    }
}

impl Resource for PlayerStatePersistence {}

impl PlayerStatePersistence {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self::with_db_path(data_dir, DEFAULT_DATABASE_PATH)
    }

    pub fn with_db_path(data_dir: impl Into<PathBuf>, db_path: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
            db_path: db_path.into(),
        }
    }

    pub fn db_path(&self) -> &std::path::Path {
        self.db_path.as_path()
    }

    #[cfg(test)]
    pub fn data_dir(&self) -> &std::path::Path {
        self.data_dir.as_path()
    }

    pub fn path_for_username(&self, username: &str) -> PathBuf {
        let player_key = canonical_player_id(username);
        self.data_dir.join(format!("{player_key}.json"))
    }

    fn migrated_path_for_username(&self, username: &str) -> PathBuf {
        let player_key = canonical_player_id(username);
        self.data_dir.join(format!("{player_key}.json.migrated"))
    }
}

#[derive(Debug, Default)]
pub struct PlayerStateAutosaveTimer {
    pub ticks: u64,
}

impl Resource for PlayerStateAutosaveTimer {}

pub fn canonical_player_id(username: &str) -> String {
    format!("offline:{username}")
}

pub fn player_character_id(username: &str, current_char_id: &str) -> String {
    if current_char_id.trim().is_empty() {
        canonical_player_id(username)
    } else {
        format!("{}:{current_char_id}", canonical_player_id(username))
    }
}

pub fn player_username_from_character_id(character_id: &str) -> Option<&str> {
    let rest = character_id.strip_prefix("offline:")?;
    let username = rest.split_once(':').map_or(rest, |(username, _)| username);
    if username.is_empty() {
        None
    } else {
        Some(username)
    }
}

pub fn position_array_from_dvec3(position: DVec3) -> [f64; 3] {
    [position.x, position.y, position.z]
}

pub fn load_current_character_id(
    persistence: &PlayerStatePersistence,
    username: &str,
) -> io::Result<Option<String>> {
    let connection = open_player_connection(persistence)?;
    ensure_player_schema(&connection)?;
    connection
        .query_row(
            "SELECT current_char_id FROM player_core WHERE username = ?1",
            params![username],
            |row| row.get(0),
        )
        .optional()
        .map_err(io::Error::other)
}

pub fn load_player_state(persistence: &PlayerStatePersistence, username: &str) -> PlayerState {
    let mut connection = match open_player_connection(persistence) {
        Ok(connection) => connection,
        Err(error) => {
            tracing::warn!(
                "[bong][player] failed to open sqlite PlayerState store for `{}` at {}: {error}; using default state",
                username,
                persistence.db_path().display()
            );
            return PlayerState::default();
        }
    };

    match load_player_state_from_sqlite(&connection, username) {
        Ok(Some(state)) => {
            if let Err(error) = ensure_player_auxiliary_rows(&mut connection, username) {
                tracing::warn!(
                    "[bong][player] failed to ensure auxiliary sqlite rows for `{}`: {error}",
                    username
                );
            }
            return state;
        }
        Ok(None) => {}
        Err(error) => {
            tracing::warn!(
                "[bong][player] failed to load PlayerState for `{}` from sqlite {}: {error}; using default state",
                username,
                persistence.db_path().display()
            );
            return PlayerState::default();
        }
    }

    match migrate_legacy_player_json_to_sqlite(persistence, &mut connection, username) {
        Ok(Some(state)) => return state,
        Ok(None) => {}
        Err(error) => tracing::warn!(
            "[bong][player] failed to migrate legacy PlayerState for `{}` from {}: {error}; using default state",
            username,
            persistence.path_for_username(username).display()
        ),
    }

    let default_state = PlayerState::default();
    if let Err(error) = save_player_state(persistence, username, &default_state) {
        tracing::warn!(
            "[bong][player] failed to initialize default sqlite PlayerState for `{}`: {error}",
            username
        );
    } else {
        tracing::warn!(
            "[bong][player] no sqlite PlayerState for `{}`; initialized default state in {}",
            username,
            persistence.db_path().display()
        );
    }

    default_state
}

pub fn load_player_slices(
    persistence: &PlayerStatePersistence,
    username: &str,
) -> LoadedPlayerSlices {
    load_player_slices_inner(persistence, username, true)
}

/// 加载玩家其它切片，但跳过功法读取。
///
/// 功法由 canonical persistence slice 独立加载并管理写保护，因此该路径返回
/// [`LoadedKnownTechniques::NotLoaded`]，调用方不得据此写回功法。
pub(crate) fn load_player_slices_for_canonical_techniques(
    persistence: &PlayerStatePersistence,
    username: &str,
) -> LoadedPlayerSlices {
    load_player_slices_inner(persistence, username, false)
}

fn load_player_slices_inner(
    persistence: &PlayerStatePersistence,
    username: &str,
    load_known_techniques: bool,
) -> LoadedPlayerSlices {
    let spawn_position =
        crate::player::spawn_position_for_seed(username, SpawnPurpose::InitialLogin);
    let mut statuses = BTreeMap::new();
    let mut failures = HashMap::new();
    let mut connection = match open_player_connection(persistence) {
        Ok(connection) => connection,
        Err(error) => {
            tracing::warn!(
                "[bong][player] failed to reopen sqlite player slice store for `{}` at {}: {error}; using default slow/inventory slices",
                username,
                persistence.db_path().display()
            );
            for slice in [
                PlayerSlice::Core,
                PlayerSlice::Position,
                PlayerSlice::Inventory,
                PlayerSlice::Lifespan,
                PlayerSlice::Craft,
                PlayerSlice::UiPrefs,
                PlayerSlice::LongTermBuff,
                PlayerSlice::KnownTechniques,
                PlayerSlice::Identity,
            ] {
                statuses.insert(slice, PlayerSliceLoadStatus::Failed);
                failures.insert(slice, error.to_string());
            }
            return LoadedPlayerSlices {
                state: PlayerState::default(),
                position: spawn_position,
                last_dimension: DimensionKind::default(),
                inventory: None,
                craft_session: None,
                lifespan: None,
                status_effects: None,
                in_coffin: false,
                coffin_grade: None,
                skill_set: SkillSet::default(),
                // 连接都打不开 = 行状态不可知（DB busy/文件不可达），
                // 必须按 LoadFailed 写保护，绝不能当「新玩家」用 default 覆盖写回
                known_techniques: LoadedKnownTechniques::LoadFailed,
                ui_prefs: PlayerUiPrefs::default(),
                load_guard: PlayerSliceLoadGuard::new(statuses, failures),
            };
        }
    };

    let state = match load_player_state_from_sqlite(&connection, username) {
        Ok(Some(state)) => {
            statuses.insert(PlayerSlice::Core, PlayerSliceLoadStatus::Loaded);
            state
        }
        Ok(None) => {
            match migrate_legacy_player_json_to_sqlite(persistence, &mut connection, username) {
                Ok(Some(state)) => {
                    statuses.insert(PlayerSlice::Core, PlayerSliceLoadStatus::Loaded);
                    state
                }
                Ok(None) => {
                    statuses.insert(PlayerSlice::Core, PlayerSliceLoadStatus::Missing);
                    PlayerState::default()
                }
                Err(error) => {
                    statuses.insert(PlayerSlice::Core, PlayerSliceLoadStatus::Failed);
                    failures.insert(PlayerSlice::Core, error.to_string());
                    PlayerState::default()
                }
            }
        }
        Err(error) => {
            tracing::warn!(
                "[bong][player] failed to load persisted core state for `{}` from sqlite {}: {error}; using runtime default without write permission",
                username,
                persistence.db_path().display()
            );
            statuses.insert(PlayerSlice::Core, PlayerSliceLoadStatus::Failed);
            failures.insert(PlayerSlice::Core, error.to_string());
            PlayerState::default()
        }
    };

    let (position, last_dimension) = match load_player_slow_from_sqlite(&connection, username) {
        Ok(Some((pos, dim))) => match sanitize_loaded_position(username, pos, dim) {
            Ok(position) => {
                statuses.insert(PlayerSlice::Position, PlayerSliceLoadStatus::Loaded);
                position
            }
            Err(error) => {
                statuses.insert(PlayerSlice::Position, PlayerSliceLoadStatus::Failed);
                failures.insert(PlayerSlice::Position, error.to_string());
                (spawn_position, DimensionKind::default())
            }
        },
        Ok(None) => {
            statuses.insert(PlayerSlice::Position, PlayerSliceLoadStatus::Missing);
            (spawn_position, DimensionKind::default())
        }
        Err(error) => {
            tracing::warn!(
                "[bong][player] failed to load persisted position/dimension for `{}` from sqlite {}: {error}; using runtime default without write permission",
                username,
                persistence.db_path().display()
            );
            statuses.insert(PlayerSlice::Position, PlayerSliceLoadStatus::Failed);
            failures.insert(PlayerSlice::Position, error.to_string());
            (spawn_position, DimensionKind::default())
        }
    };
    let inventory = match load_player_inventory_from_sqlite(
        &mut connection,
        username,
        Some(LegacyInventorySpillContext {
            world_pos: position,
            dimension: last_dimension,
        }),
    ) {
        Ok(inventory) => {
            let status = match inventory {
                Some(_) => PlayerSliceLoadStatus::Loaded,
                None => match inventory_row_has_payload(&connection, username) {
                    Ok(true) => PlayerSliceLoadStatus::Failed,
                    Ok(false) => PlayerSliceLoadStatus::Missing,
                    Err(error) => {
                        failures.insert(PlayerSlice::Inventory, error.to_string());
                        PlayerSliceLoadStatus::Failed
                    }
                },
            };
            statuses.insert(PlayerSlice::Inventory, status);
            inventory
        }
        Err(error) => {
            tracing::warn!(
                "[bong][player] failed to load persisted inventory for `{}` from sqlite {}: {error}; using runtime default without write permission",
                username,
                persistence.db_path().display()
            );
            statuses.insert(PlayerSlice::Inventory, PlayerSliceLoadStatus::Failed);
            failures.insert(PlayerSlice::Inventory, error.to_string());
            None
        }
    };
    let craft_session = match load_player_craft_session_from_sqlite(&connection, username) {
        Ok(session) => {
            statuses.insert(
                PlayerSlice::Craft,
                if session.is_some() {
                    PlayerSliceLoadStatus::Loaded
                } else {
                    PlayerSliceLoadStatus::Missing
                },
            );
            session
        }
        Err(error) => {
            tracing::error!(
                "[bong][player] failed to load persisted craft session for `{}` from sqlite {}: {error}; refusing to invent a replacement session",
                username,
                persistence.db_path().display()
            );
            statuses.insert(PlayerSlice::Craft, PlayerSliceLoadStatus::Failed);
            failures.insert(PlayerSlice::Craft, error.to_string());
            None
        }
    };
    let (lifespan, in_coffin, coffin_grade) = match load_player_lifespan_from_sqlite(
        &connection,
        username,
    ) {
        Ok(Some((lifespan, in_coffin, grade))) => {
            statuses.insert(PlayerSlice::Lifespan, PlayerSliceLoadStatus::Loaded);
            // coffin_grade = Some(grade) 当 in_coffin=true，None 当 in_coffin=false
            let coffin_grade = if in_coffin { Some(grade) } else { None };
            (Some(lifespan), in_coffin, coffin_grade)
        }
        Ok(None) => {
            statuses.insert(PlayerSlice::Lifespan, PlayerSliceLoadStatus::Missing);
            (None, false, None)
        }
        Err(error) => {
            tracing::warn!(
                    "[bong][player] failed to load persisted lifespan for `{}` from sqlite {}: {error}; using runtime default without write permission",
                    username,
                    persistence.db_path().display()
                );
            statuses.insert(PlayerSlice::Lifespan, PlayerSliceLoadStatus::Failed);
            failures.insert(PlayerSlice::Lifespan, error.to_string());
            (None, false, None)
        }
    };
    let skill_set = match load_player_skill_set_from_sqlite(&connection, username) {
        Ok(skill_set) => skill_set,
        Err(error) => {
            tracing::warn!(
                "[bong][player] failed to load persisted skill set for `{}` from sqlite {}: {error}; using default skill set",
                username,
                persistence.db_path().display()
            );
            SkillSet::default()
        }
    };
    let known_techniques = if load_known_techniques {
        match load_player_known_techniques_from_sqlite(&connection, username) {
            Ok(known_techniques) => {
                statuses.insert(PlayerSlice::KnownTechniques, PlayerSliceLoadStatus::Loaded);
                LoadedKnownTechniques::Loaded(known_techniques.unwrap_or_default())
            }
            Err(error) => {
                tracing::error!(
                    "[bong][player] failed to load persisted known techniques for `{}` from sqlite {}: {error}; blocking known techniques persistence for this session to protect the stored row",
                    username,
                    persistence.db_path().display()
                );
                statuses.insert(PlayerSlice::KnownTechniques, PlayerSliceLoadStatus::Failed);
                failures.insert(PlayerSlice::KnownTechniques, error.to_string());
                LoadedKnownTechniques::LoadFailed
            }
        }
    } else {
        statuses.insert(PlayerSlice::KnownTechniques, PlayerSliceLoadStatus::Failed);
        failures.insert(
            PlayerSlice::KnownTechniques,
            "canonical persistence adapter owns this slice".to_string(),
        );
        LoadedKnownTechniques::NotLoaded
    };
    let ui_prefs = match load_player_ui_prefs_optional_from_sqlite(&connection, username) {
        Ok(Some(ui_prefs)) => {
            statuses.insert(PlayerSlice::UiPrefs, PlayerSliceLoadStatus::Loaded);
            ui_prefs
        }
        Ok(None) => {
            statuses.insert(PlayerSlice::UiPrefs, PlayerSliceLoadStatus::Missing);
            PlayerUiPrefs::default()
        }
        Err(error) => {
            tracing::warn!(
                "[bong][player] failed to load persisted UI prefs for `{}` from sqlite {}: {error}; using runtime default without write permission",
                username,
                persistence.db_path().display()
            );
            statuses.insert(PlayerSlice::UiPrefs, PlayerSliceLoadStatus::Failed);
            failures.insert(PlayerSlice::UiPrefs, error.to_string());
            PlayerUiPrefs::default()
        }
    };
    let status_effects = match load_player_status_effects_from_sqlite(&connection, username) {
        Ok(Some(status_effects)) => {
            statuses.insert(PlayerSlice::LongTermBuff, PlayerSliceLoadStatus::Loaded);
            Some(status_effects)
        }
        Ok(None) => {
            statuses.insert(PlayerSlice::LongTermBuff, PlayerSliceLoadStatus::Missing);
            None
        }
        Err(error) => {
            tracing::warn!(
                "[bong][player] failed to load persisted long-term buffs for `{}` from sqlite {}: {error}; using runtime default without write permission",
                username,
                persistence.db_path().display()
            );
            statuses.insert(PlayerSlice::LongTermBuff, PlayerSliceLoadStatus::Failed);
            failures.insert(PlayerSlice::LongTermBuff, error.to_string());
            None
        }
    };
    statuses.insert(PlayerSlice::Identity, PlayerSliceLoadStatus::Failed);
    failures.insert(
        PlayerSlice::Identity,
        "identity adapter loads independently from canonical player key".to_string(),
    );

    LoadedPlayerSlices {
        state,
        position,
        last_dimension,
        inventory,
        craft_session,
        lifespan,
        status_effects,
        in_coffin,
        coffin_grade,
        skill_set,
        known_techniques,
        ui_prefs,
        load_guard: PlayerSliceLoadGuard::new(statuses, failures),
    }
}

pub fn load_player_shrine_anchor_slice(
    persistence: &PlayerStatePersistence,
    username: &str,
) -> io::Result<Option<[f64; 3]>> {
    let connection = open_player_connection(persistence)?;
    load_player_shrine_anchor_from_sqlite(&connection, username)
}

pub fn save_player_shrine_anchor_slice(
    persistence: &PlayerStatePersistence,
    username: &str,
    anchor: Option<[f64; 3]>,
) -> io::Result<PathBuf> {
    let mut connection = open_player_connection(persistence)?;
    persist_player_shrine_anchor_slice_in_sqlite(&mut connection, username, anchor)?;
    Ok(persistence.db_path().to_path_buf())
}

/// bughunt player-lifecycle-relog-death-consequence-wipe：读回断线前持久化的死亡/复活
/// 状态机（`state`/`fortune_remaining`/`awaiting_decision`/各 deadline tick）。
/// `None` = 该用户名从未落过盘（首次登录，或 pre-v39 老档），调用方应回退到
/// `Lifecycle::default()` 而非当作"读取失败"处理。
///
/// `current_combat_clock_tick` 是读档当刻（重连那一瞬）的 `CombatClock.tick`——用于把
/// 落盘时刻记录的"绝对 tick" deadline（
/// `revival_decision_deadline_tick`/`weakened_until_tick`）折算到当前 tick 空间，
/// 详见 `translate_lifecycle_deadline_tick_across_restart` 的文档注释。
pub fn load_player_lifecycle_slice(
    persistence: &PlayerStatePersistence,
    username: &str,
    current_combat_clock_tick: u64,
) -> io::Result<Option<crate::combat::components::Lifecycle>> {
    let connection = open_player_connection(persistence)?;
    load_player_lifecycle_from_sqlite(&connection, username, current_combat_clock_tick)
}

/// 读取玩家长期状态效果；损坏或不可读的行返回 `Err`，由调用方挂载写保护标记。
pub fn load_player_status_effects_slice(
    persistence: &PlayerStatePersistence,
    username: &str,
) -> io::Result<Option<StatusEffects>> {
    let connection = open_player_connection(persistence)?;
    load_player_status_effects_from_sqlite(&connection, username)
}

/// bughunt player-lifecycle-relog-death-consequence-wipe：断线/关服 flush 时把当前
/// `Lifecycle` 组件整份落盘，让重连不再盲插 `Lifecycle::default()`（否则待复活玩家
/// 会被静默重置成满运气次数的"新角色"，绕过渡劫概率判定与永久终结风险）。
///
/// `combat_clock_tick` 是落盘那一刻的 `CombatClock.tick`，作为跨重启折算 deadline 的锚点
/// 存进 `combat_clock_tick_at_save` 列（见 `load_player_lifecycle_slice`）。
pub fn save_player_lifecycle_slice(
    persistence: &PlayerStatePersistence,
    username: &str,
    lifecycle: &crate::combat::components::Lifecycle,
    combat_clock_tick: u64,
) -> io::Result<PathBuf> {
    let mut connection = open_player_connection(persistence)?;
    persist_player_lifecycle_slice_in_sqlite(
        &mut connection,
        username,
        lifecycle,
        combat_clock_tick,
    )?;
    Ok(persistence.db_path().to_path_buf())
}

pub fn save_player_state(
    persistence: &PlayerStatePersistence,
    username: &str,
    state: &PlayerState,
) -> io::Result<PathBuf> {
    save_player_slices(
        persistence,
        username,
        state,
        crate::player::spawn_position_for_seed(username, SpawnPurpose::InitialLogin),
        DimensionKind::default(),
        None,
        None,
        &SkillSet::default(),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn save_player_slices(
    persistence: &PlayerStatePersistence,
    username: &str,
    state: &PlayerState,
    position: [f64; 3],
    last_dimension: DimensionKind,
    inventory: Option<&PlayerInventory>,
    lifespan: Option<&LifespanComponent>,
    skill_set: &SkillSet,
) -> io::Result<PathBuf> {
    let mut connection = open_player_connection(persistence)?;
    // 该入口只用于显式的新角色/重生重置；长期状态效果也必须随同一事务清空，
    // 否则下一次 hydrate 会把旧角色的 buff 带回新角色。
    let reset_status_effects = StatusEffects::default();
    // grade=None → resolve_coffin_grade_for_persist 回读 DB 既有 grade
    persist_player_slices_in_sqlite_with_write_set(
        &mut connection,
        username,
        state,
        position,
        last_dimension,
        inventory,
        lifespan,
        skill_set,
        None,
        None,
        None,
        Some(&reset_status_effects),
        WriteSet::all(),
    )?;
    Ok(persistence.db_path().to_path_buf())
}

#[allow(clippy::too_many_arguments)]
pub fn save_player_slices_with_coffin(
    persistence: &PlayerStatePersistence,
    username: &str,
    state: &PlayerState,
    position: [f64; 3],
    last_dimension: DimensionKind,
    inventory: Option<&PlayerInventory>,
    lifespan: Option<&LifespanComponent>,
    skill_set: &SkillSet,
    grade: Option<CoffinGrade>,
    craft_session: Option<&CraftSession>,
) -> io::Result<PathBuf> {
    save_player_slices_with_coffin_and_write_set(
        persistence,
        username,
        state,
        position,
        last_dimension,
        inventory,
        lifespan,
        skill_set,
        grade,
        craft_session,
        None,
        WriteSet::all(),
    )
}

/// 按 `write_set` 原子保存玩家聚合切片。
///
/// 被省略的切片完全不写对应 durable row；调用方应把载入失败产生的集合原样传入，
/// 这样运行时 fallback 不会覆盖未知的持久化数据。
#[allow(clippy::too_many_arguments)]
pub fn save_player_slices_with_coffin_and_write_set(
    persistence: &PlayerStatePersistence,
    username: &str,
    state: &PlayerState,
    position: [f64; 3],
    last_dimension: DimensionKind,
    inventory: Option<&PlayerInventory>,
    lifespan: Option<&LifespanComponent>,
    skill_set: &SkillSet,
    grade: Option<CoffinGrade>,
    craft_session: Option<&CraftSession>,
    status_effects: Option<&StatusEffects>,
    write_set: WriteSet,
) -> io::Result<PathBuf> {
    let mut connection = open_player_connection(persistence)?;
    persist_player_slices_in_sqlite_with_write_set(
        &mut connection,
        username,
        state,
        position,
        last_dimension,
        inventory,
        lifespan,
        skill_set,
        Some(grade.is_some()),
        grade,
        Some(craft_session),
        status_effects,
        write_set,
    )?;
    Ok(persistence.db_path().to_path_buf())
}

pub fn save_player_lifespan_slice(
    persistence: &PlayerStatePersistence,
    username: &str,
    lifespan: &LifespanComponent,
) -> io::Result<PathBuf> {
    let mut connection = open_player_connection(persistence)?;
    // grade=None → resolve_coffin_grade_for_persist 回读 DB 既有 grade，
    // 避免悟道延寿路径把 Jade/Stone/Bronze 洗成 Mundane。
    persist_player_lifespan_slice_in_sqlite(&mut connection, username, lifespan, None, None, None)?;
    Ok(persistence.db_path().to_path_buf())
}

pub fn save_player_lifespan_slice_with_coffin(
    persistence: &PlayerStatePersistence,
    username: &str,
    lifespan: &LifespanComponent,
    grade: Option<CoffinGrade>,
) -> io::Result<PathBuf> {
    let mut connection = open_player_connection(persistence)?;
    persist_player_lifespan_slice_in_sqlite(
        &mut connection,
        username,
        lifespan,
        None,
        Some(grade.is_some()),
        grade,
    )?;
    Ok(persistence.db_path().to_path_buf())
}

pub fn save_player_core_slice(
    persistence: &PlayerStatePersistence,
    username: &str,
    state: &PlayerState,
) -> io::Result<PathBuf> {
    let mut connection = open_player_connection(persistence)?;
    persist_player_core_slice_in_sqlite(&mut connection, username, state)?;
    Ok(persistence.db_path().to_path_buf())
}

/// F21 — 断连/组件缺失兜底：不依赖 [`LifespanComponent`]，直接把某用户名的
/// `in_coffin` 清 0。用户名在 `player_lifespan` 里没有对应行时天然 no-op
/// （`UPDATE ... WHERE username = ?1` 命中 0 行，既不报错也不误插入残缺行）。
///
/// 修 F21"重启复钉"风险：当 [`LifespanComponent`] 缺失（例如断连时组件已被
/// ECS 移除）导致 [`crate::coffin::persist_in_coffin`] 早退只 `warn` 不写库，
/// SQLite 会保留旧 `in_coffin=true`，玩家下次连接会被错误重钉到可能已不存在
/// 的棺材坐标（见 `player::attach_player_state_to_joined_clients` 的
/// `persisted.in_coffin` 读取路径）。
///
/// `coffin_grade` 一并写回默认档（`CoffinGrade::Mundane` / db 值 `"mundane"`），
/// 而不是计划文本里写的 `NULL`——`player_lifespan.coffin_grade` 列是
/// `NOT NULL DEFAULT 'mundane'`（见 `persistence::apply_migrations`
/// user_version 27 迁移），写 NULL 会直接违反列约束、让整条 UPDATE 报错、
/// 反而连 `in_coffin` 都清不掉。`in_coffin=0` 时读路径本就不读 `coffin_grade`
/// （`load_player_lifespan_from_sqlite`：`if in_coffin { Some(grade) } else {
/// None }`），所以写回默认档在语义上等价于"清空"。
pub fn clear_coffin_flag_for_username(
    persistence: &PlayerStatePersistence,
    username: &str,
) -> io::Result<PathBuf> {
    let connection = open_player_connection(persistence)?;
    connection
        .execute(
            "UPDATE player_lifespan SET in_coffin = 0, coffin_grade = ?2 WHERE username = ?1",
            params![username, CoffinGrade::default().as_db_str()],
        )
        .map_err(io::Error::other)?;
    Ok(persistence.db_path().to_path_buf())
}

pub fn save_player_slow_slice(
    persistence: &PlayerStatePersistence,
    username: &str,
    position: [f64; 3],
    last_dimension: DimensionKind,
) -> io::Result<PathBuf> {
    let mut connection = open_player_connection(persistence)?;
    persist_player_slow_slice_in_sqlite(&mut connection, username, position, last_dimension)?;
    Ok(persistence.db_path().to_path_buf())
}

pub fn save_player_inventory_slice(
    persistence: &PlayerStatePersistence,
    username: &str,
    inventory: Option<&PlayerInventory>,
) -> io::Result<PathBuf> {
    let mut connection = open_player_connection(persistence)?;
    persist_player_inventory_slice_in_sqlite(&mut connection, username, inventory)?;
    Ok(persistence.db_path().to_path_buf())
}

/// 保存玩家长期状态效果切片；该行与其他玩家 slice 使用同一 SQLite schema 版本。
pub fn save_player_status_effects_slice(
    persistence: &PlayerStatePersistence,
    username: &str,
    status_effects: &StatusEffects,
) -> io::Result<PathBuf> {
    let mut connection = open_player_connection(persistence)?;
    persist_player_status_effects_slice_in_sqlite(&mut connection, username, status_effects)?;
    Ok(persistence.db_path().to_path_buf())
}

pub fn save_player_inventory_and_craft_session_slices(
    persistence: &PlayerStatePersistence,
    username: &str,
    inventory: Option<&PlayerInventory>,
    craft_session: Option<&CraftSession>,
) -> io::Result<PathBuf> {
    save_player_craft_checkpoint(
        persistence,
        username,
        inventory,
        craft_session,
        None,
        None,
        &[],
    )
}

#[allow(clippy::too_many_arguments)]
pub fn save_player_craft_checkpoint(
    persistence: &PlayerStatePersistence,
    username: &str,
    inventory: Option<&PlayerInventory>,
    craft_session: Option<&CraftSession>,
    cultivation: Option<&Cultivation>,
    qi_ledger: Option<&WorldQiAccount>,
    durable_drops: &[DroppedLootEntry],
) -> io::Result<PathBuf> {
    let mut connection = open_player_connection(persistence)?;
    let inventory_json = serialize_inventory_json(inventory)?;
    let craft_session_json = craft_session
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let last_updated_wall = current_unix_seconds();
    let transaction = connection.transaction().map_err(io::Error::other)?;
    persist_player_inventory_json_in_transaction(
        &transaction,
        username,
        &inventory_json,
        last_updated_wall,
    )?;
    persist_player_craft_session_in_transaction(
        &transaction,
        username,
        craft_session_json.as_deref(),
        last_updated_wall,
    )?;
    if let Some(cultivation) = cultivation {
        crate::persistence::upsert_player_cultivation_slice(
            &transaction,
            username,
            cultivation,
            last_updated_wall,
        )?;
    }
    if let Some(qi_ledger) = qi_ledger {
        crate::persistence::upsert_runtime_qi_account_balances(
            &transaction,
            qi_ledger,
            last_updated_wall,
        )?;
    }
    crate::persistence::upsert_dropped_loot_entries(
        &transaction,
        durable_drops,
        last_updated_wall,
    )?;
    transaction.commit().map_err(io::Error::other)?;
    Ok(persistence.db_path().to_path_buf())
}

/// 容器缩容时，库存移除与溢出物落地必须一起提交，不能覆盖正在进行的制作会话。
pub fn save_player_inventory_with_drops(
    persistence: &PlayerStatePersistence,
    username: &str,
    inventory: &PlayerInventory,
    drops: &[DroppedLootEntry],
) -> io::Result<()> {
    let mut connection = open_player_connection(persistence)?;
    let inventory_json = serialize_inventory_json(Some(inventory))?;
    let wall = current_unix_seconds();
    let transaction = connection.transaction().map_err(io::Error::other)?;
    persist_player_inventory_json_in_transaction(&transaction, username, &inventory_json, wall)?;
    crate::persistence::upsert_dropped_loot_entries(&transaction, drops, wall)?;
    transaction.commit().map_err(io::Error::other)
}

pub fn save_player_inventory_and_delete_dropped_loot(
    persistence: &PlayerStatePersistence,
    username: &str,
    inventory: &PlayerInventory,
    dropped_instance_id: u64,
    zone_runtime: Option<&ZoneRuntimeRecord>,
) -> io::Result<PathBuf> {
    let mut connection = open_player_connection(persistence)?;
    let inventory_json = serialize_inventory_json(Some(inventory))?;
    let last_updated_wall = current_unix_seconds();
    let transaction = connection.transaction().map_err(io::Error::other)?;
    persist_player_inventory_json_in_transaction(
        &transaction,
        username,
        &inventory_json,
        last_updated_wall,
    )?;
    crate::persistence::delete_dropped_loot_entry(&transaction, dropped_instance_id)?;
    if let Some(zone_runtime) = zone_runtime {
        crate::persistence::upsert_zone_runtime(&transaction, zone_runtime, last_updated_wall)?;
    }
    transaction.commit().map_err(io::Error::other)?;
    Ok(persistence.db_path().to_path_buf())
}

/// Persist world drops without requiring a live player inventory.
///
/// Forge outcomes can be finalized after the caster entity has been despawned. The
/// dropped-loot row still needs to survive a restart, so this narrow checkpoint writes
/// only the durable ground entries and leaves player slices untouched.
pub fn persist_dropped_loot_entries(
    persistence: &PlayerStatePersistence,
    entries: &[DroppedLootEntry],
) -> io::Result<PathBuf> {
    let mut connection = open_player_connection(persistence)?;
    let last_updated_wall = current_unix_seconds();
    let transaction = connection.transaction().map_err(io::Error::other)?;
    crate::persistence::upsert_dropped_loot_entries(&transaction, entries, last_updated_wall)?;
    transaction.commit().map_err(io::Error::other)?;
    Ok(persistence.db_path().to_path_buf())
}

pub fn rotate_current_character_id(
    persistence: &PlayerStatePersistence,
    username: &str,
) -> io::Result<String> {
    let connection = open_player_connection(persistence)?;
    ensure_player_schema(&connection)?;
    let next_char_id = Uuid::now_v7().to_string();
    let last_updated_wall = current_unix_seconds();

    connection
        .execute(
            "
            INSERT INTO player_core (
                username,
                current_char_id,
                karma,
                inventory_score,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, 0.0, 0.0, ?3, ?4)
            ON CONFLICT(username) DO UPDATE SET
                current_char_id = excluded.current_char_id,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                username,
                next_char_id,
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;

    Ok(next_char_id)
}

fn ensure_player_schema(connection: &Connection) -> io::Result<()> {
    let has_player_core: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'player_core'",
            [],
            |row| row.get(0),
        )
        .map_err(io::Error::other)?;
    if has_player_core == 0 {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "player_core table is missing; bootstrap sqlite before loading character ids",
        ));
    }
    Ok(())
}

pub fn save_player_skill_slice(
    persistence: &PlayerStatePersistence,
    username: &str,
    skill_set: &SkillSet,
) -> io::Result<PathBuf> {
    let mut connection = open_player_connection(persistence)?;
    persist_player_skill_slice_in_sqlite(&mut connection, username, skill_set)?;
    Ok(persistence.db_path().to_path_buf())
}

pub fn save_player_known_techniques_slice(
    persistence: &PlayerStatePersistence,
    username: &str,
    known_techniques: &KnownTechniques,
) -> io::Result<PathBuf> {
    let mut connection = open_player_connection(persistence)?;
    persist_player_known_techniques_slice_in_sqlite(&mut connection, username, known_techniques)?;
    Ok(persistence.db_path().to_path_buf())
}

pub(crate) fn update_player_ui_prefs<F>(
    persistence: &PlayerStatePersistence,
    username: &str,
    update: F,
) -> io::Result<PathBuf>
where
    F: FnOnce(&mut PlayerUiPrefs),
{
    let mut connection = open_player_connection(persistence)?;
    let mut ui_prefs = load_player_ui_prefs_from_sqlite(&connection, username)?;
    update(&mut ui_prefs);
    persist_player_ui_prefs_slice_in_sqlite(&mut connection, username, &ui_prefs)?;
    Ok(persistence.db_path().to_path_buf())
}

/// 尝试在不等待 SQLite 写锁的情况下更新 UI 偏好。
///
/// 网络请求处理系统位于 ECS 主线程；这里的快速路径只能做一次零等待尝试，
/// 否则另一个合法的持久化写事务就能把 `quick_slot_bind` 的 ACK 挡在几十秒之外。
/// `SQLITE_BUSY` 原样保留在返回的 `io::Error` 中，调用方据此把更新排到后续帧。
pub(crate) fn try_update_player_ui_prefs<F>(
    persistence: &PlayerStatePersistence,
    username: &str,
    update: F,
) -> io::Result<PathBuf>
where
    F: FnOnce(&mut PlayerUiPrefs),
{
    let mut connection = open_player_connection_with_timeout(persistence, Duration::ZERO)?;
    let mut ui_prefs = load_player_ui_prefs_from_sqlite(&connection, username)?;
    update(&mut ui_prefs);
    persist_player_ui_prefs_slice_in_sqlite(&mut connection, username, &ui_prefs)?;
    Ok(persistence.db_path().to_path_buf())
}

/// `io::Error::other(rusqlite::Error)` 保留底层错误作为 source；网络层只需要知道
/// 这次失败是否可通过下一帧重试，不应依赖 rusqlite 具体错误文本。
pub(crate) fn is_sqlite_busy_error(error: &io::Error) -> bool {
    let Some(source) = error.get_ref() else {
        return false;
    };
    let Some(sqlite_error) = source.downcast_ref::<rusqlite::Error>() else {
        return false;
    };
    matches!(
        sqlite_error,
        rusqlite::Error::SqliteFailure(code, _)
            if matches!(
                code.code,
                rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
            )
    )
}

pub fn export_player_bundle(
    persistence: &PlayerStatePersistence,
    username: &str,
) -> io::Result<PlayerExportBundle> {
    let loaded = load_player_slices(persistence, username);
    let LoadedKnownTechniques::Loaded(known_techniques) = loaded.known_techniques else {
        return Err(io::Error::other(format!(
            "known techniques for `{username}` could not be reliably loaded; refusing to export a default table in its place"
        )));
    };
    let connection = open_player_connection(persistence)?;
    let current_char_id: String = connection
        .query_row(
            "SELECT current_char_id FROM player_core WHERE username = ?1",
            params![username],
            |row| row.get(0),
        )
        .map_err(io::Error::other)?;
    let ui_prefs_json: String = connection
        .query_row(
            "SELECT prefs_json FROM player_ui_prefs WHERE username = ?1",
            params![username],
            |row| row.get(0),
        )
        .map_err(io::Error::other)?;
    let ui_prefs = serde_json::from_str(&ui_prefs_json)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    Ok(PlayerExportBundle {
        kind: "player_export_v1".to_string(),
        username: username.to_string(),
        current_char_id,
        state: loaded.state,
        position: loaded.position,
        last_dimension: loaded.last_dimension,
        inventory: loaded.inventory,
        skill_set: loaded.skill_set,
        known_techniques,
        ui_prefs,
    })
}

pub fn import_player_bundle(
    persistence: &PlayerStatePersistence,
    bundle: &PlayerExportBundle,
) -> io::Result<()> {
    if bundle.kind != "player_export_v1" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unexpected player export kind: {}", bundle.kind),
        ));
    }

    let _ = Uuid::parse_str(&bundle.current_char_id)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let ui_prefs = serde_json::from_value::<PlayerUiPrefs>(bundle.ui_prefs.clone())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let ui_prefs_json = serde_json::to_string(&ui_prefs)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let inventory_json = serialize_inventory_json(bundle.inventory.as_ref())?;
    let skill_set_json = serialize_skill_set_json(&bundle.skill_set)?;
    let known_techniques_json = serialize_known_techniques_json(&bundle.known_techniques)?;
    let normalized = bundle.state.normalized();
    let [pos_x, pos_y, pos_z] = bundle.position;
    let last_updated_wall = current_unix_seconds();
    let mut connection = open_player_connection(persistence)?;
    let transaction = connection.transaction().map_err(io::Error::other)?;

    transaction
        .execute(
            "
            INSERT INTO player_core (
                username,
                current_char_id,
                karma,
                inventory_score,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(username) DO UPDATE SET
                current_char_id = excluded.current_char_id,
                karma = excluded.karma,
                inventory_score = excluded.inventory_score,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                bundle.username,
                bundle.current_char_id,
                normalized.karma,
                normalized.inventory_score,
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;
    transaction
        .execute(
            "
            INSERT INTO player_slow (
                username,
                pos_x,
                pos_y,
                pos_z,
                last_dimension,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(username) DO UPDATE SET
                pos_x = excluded.pos_x,
                pos_y = excluded.pos_y,
                pos_z = excluded.pos_z,
                last_dimension = excluded.last_dimension,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                bundle.username,
                pos_x,
                pos_y,
                pos_z,
                dimension_kind_to_sql(bundle.last_dimension),
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;
    transaction
        .execute(
            "
            INSERT INTO inventories (
                username,
                inventory_json,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(username) DO UPDATE SET
                inventory_json = excluded.inventory_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                bundle.username,
                inventory_json,
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;
    transaction
        .execute(
            "
            INSERT INTO player_skills (
                username,
                skill_set_json,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(username) DO UPDATE SET
                skill_set_json = excluded.skill_set_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                bundle.username,
                skill_set_json,
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;
    transaction
        .execute(
            "
            INSERT INTO player_known_techniques (
                username,
                known_techniques_json,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(username) DO UPDATE SET
                known_techniques_json = excluded.known_techniques_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                bundle.username,
                known_techniques_json,
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;
    transaction
        .execute(
            "
            INSERT INTO player_ui_prefs (
                username,
                prefs_json,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(username) DO UPDATE SET
                prefs_json = excluded.prefs_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                bundle.username,
                ui_prefs_json,
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;

    transaction.commit().map_err(io::Error::other)
}

pub(crate) fn open_player_connection(
    persistence: &PlayerStatePersistence,
) -> io::Result<Connection> {
    open_player_connection_with_timeout(persistence, Duration::from_millis(SQLITE_BUSY_TIMEOUT_MS))
}

fn open_player_connection_with_timeout(
    persistence: &PlayerStatePersistence,
    busy_timeout: Duration,
) -> io::Result<Connection> {
    if let Some(parent) = persistence.db_path().parent() {
        fs::create_dir_all(parent)?;
    }

    let connection = Connection::open(persistence.db_path()).map_err(io::Error::other)?;
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(io::Error::other)?;
    connection
        .busy_timeout(busy_timeout)
        .map_err(io::Error::other)?;
    Ok(connection)
}

fn load_player_state_from_sqlite(
    connection: &Connection,
    username: &str,
) -> io::Result<Option<PlayerState>> {
    let row: Option<(f64, f64)> = connection
        .query_row(
            "
            SELECT karma, inventory_score
            FROM player_core
            WHERE username = ?1
            ",
            params![username],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(io::Error::other)?;

    let Some((karma, inventory_score)) = row else {
        return Ok(None);
    };

    Ok(Some(
        PlayerState {
            karma,
            inventory_score,
        }
        .normalized(),
    ))
}

fn load_player_slow_from_sqlite(
    connection: &Connection,
    username: &str,
) -> io::Result<Option<([f64; 3], DimensionKind)>> {
    let row: Option<(f64, f64, f64, String)> = connection
        .query_row(
            "
            SELECT pos_x, pos_y, pos_z, last_dimension
            FROM player_slow
            WHERE username = ?1
            ",
            params![username],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(io::Error::other)?;

    let Some((pos_x, pos_y, pos_z, dimension_text)) = row else {
        return Ok(None);
    };

    let last_dimension = dimension_kind_from_sql(&dimension_text).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unknown last_dimension `{dimension_text}` for `{username}`: {error}"),
        )
    })?;

    Ok(Some(([pos_x, pos_y, pos_z], last_dimension)))
}

fn sanitize_loaded_position(
    username: &str,
    position: [f64; 3],
    last_dimension: DimensionKind,
) -> io::Result<([f64; 3], DimensionKind)> {
    let [x, y, z] = position;
    if x.is_finite()
        && y.is_finite()
        && z.is_finite()
        && (MIN_SAFE_PLAYER_Y..=MAX_SAFE_PLAYER_Y).contains(&y)
    {
        return Ok((position, last_dimension));
    }

    tracing::warn!(
        "[bong][player] persisted position for `{username}` is outside safe login bounds \
         ({x:.2}, {y:.2}, {z:.2}, {last_dimension:?}); using spawn defaults"
    );
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        format!("persisted position for `{username}` is outside safe login bounds"),
    ))
}

/// Bug A（真机回归）— 检测 #736 旧迁移 bug 污染并已落盘为 v2 的存档指纹：
/// 存在 `pack_<instance_id>` 容器，却在 equipped 任何身体槽的 worn 层里找不到该 instance_id
/// 的穿戴背包件（孤儿派生容器）。`pack_<id>` 容器只可能由穿戴背包件运行时派生，因此孤儿即污染。
///
/// 合法裸装玩家：equipped 空但**没有任何** `pack_<id>` 容器（背包卸下时容器会随之清掉），
/// 故此判定不会误伤裸装玩家。返回 true 表示存档已污染、应丢弃回落默认 loadout。
fn inventory_has_orphan_pack_container(inventory: &PlayerInventory) -> bool {
    use std::collections::HashSet;
    // plan-tarkov-backpack-v1 套包修复 §6：live 集合扩到「玩家身上直接携带面」
    // （worn + held + hotbar + body_pocket），与 inventory/mod.rs 的 find_pack_instances_anywhere
    // （rebuild live_ids）镜像一致——背包件合法移入 body_pocket / hotbar / held 后其 pack_<id>
    // 容器仍 live，不得误判孤儿（防 #736 误删存档）。
    //
    // **不扫 pack_* / 其它 grid 容器内含物**：P5「2 层封顶」下 grid 内背包件是货物、不派生可访问
    // 容器，故 rebuild 也不会为其建 pack_<id>；与 rebuild 镜像保证孤儿检测既不误删合法暗袋背包、
    // 又仍能识别真孤儿（pack_<id> 容器但 owner 不在任何携带面）。此处只比 instance_id。
    let live_instance_ids: HashSet<u64> = inventory
        .equipped
        .values()
        .flat_map(|slot| slot.worn.iter())
        .chain(inventory.equipped.values().filter_map(|s| s.held.as_ref()))
        .chain(
            inventory
                .containers
                .iter()
                .filter(|c| c.id == crate::inventory::BODY_POCKET_CONTAINER_ID)
                .flat_map(|c| c.items.iter().map(|p| &p.instance)),
        )
        .chain(inventory.hotbar.iter().filter_map(|o| o.as_ref()))
        .map(|item| item.instance_id)
        .collect();

    inventory.containers.iter().any(|container| {
        match crate::inventory::worn_pack_instance_from_container_id(&container.id) {
            // `pack_<id>` 容器但 owner 不在任何携带面 ⇒ 真孤儿 ⇒ #736 污染指纹。
            Some(instance_id) => !live_instance_ids.contains(&instance_id),
            None => false,
        }
    })
}

/// plan-tarkov-backpack-v1 P0（交付物 #6，决议 #1 衔接）— 旧存档 `owner_instance_id` 回填。
///
/// 旧存档（`ContainerState` 无 `owner_instance_id` 字段，`serde(default)` 读为 `None`）加载后，
/// 遍历 containers：对 `pack_<id>` 前缀且 `owner_instance_id == None` 者，用
/// `worn_pack_instance_from_container_id` 解析前缀回填 `Some(instance_id)`。
///
/// 语义：**纯内存层每次加载重算、不写回 DB**（不 bump `INVENTORY_SCHEMA_VERSION`、无 SQL migration）；
/// 下次加载从前缀重新解析，幂等。回填**必须先于** `inventory_has_orphan_pack_container`——
/// 回填后前缀路径与 owner 路径结果一致，避免误判合法新格式容器（防 #736 污染误删存档）。
///
/// 注意：回填只对 `owner_instance_id` 为 `None` 的 `pack_<id>` 容器生效；非 pack 容器
/// （`body_pocket` 等）与已带 owner 的新格式容器不动（幂等）。
fn backfill_owner_instance_ids(inventory: &mut PlayerInventory) {
    for container in inventory.containers.iter_mut() {
        if container.owner_instance_id.is_some() {
            continue;
        }
        if let Some(instance_id) =
            crate::inventory::worn_pack_instance_from_container_id(&container.id)
        {
            container.owner_instance_id = Some(instance_id);
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct LegacyInventorySpillContext {
    world_pos: [f64; 3],
    dimension: DimensionKind,
}

fn load_player_inventory_from_sqlite(
    connection: &mut Connection,
    username: &str,
    spill_context: Option<LegacyInventorySpillContext>,
) -> io::Result<Option<PlayerInventory>> {
    // plan-layered-equip-v1 P0.6（决议 #4）— 先读 schema_version 分流；旧版本走 v1→v2 迁移。
    let row: Option<(String, i32)> = connection
        .query_row(
            "
            SELECT inventory_json, schema_version
            FROM inventories
            WHERE username = ?1
            ",
            params![username],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(io::Error::other)?;

    let Some((inventory_json, schema_version)) = row else {
        return Ok(None);
    };

    if inventory_json.trim() == DEFAULT_INVENTORY_JSON {
        return Ok(None);
    }

    if schema_version >= INVENTORY_SCHEMA_VERSION {
        // 新版本：直接反序列化。
        let mut inventory = serde_json::from_str::<PlayerInventory>(&inventory_json)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

        // plan-tarkov-backpack-v1 P0 — 回填 owner_instance_id（None→Some(前缀解析)），供
        // 依赖该字段的消费者用；纯内存层重算、不写回 DB（幂等）。
        // 注：下方孤儿检测改为「前缀解析 + 携带面镜像」判 live（套包修复后位置无关），不再
        // 依赖本回填的先后顺序——保留此调用是为字段完整性，非孤儿检测前置条件。
        backfill_owner_instance_ids(&mut inventory);

        // Bug A（真机回归）— #736 旧版迁移 bug 已被 #751 修，但**被那次 bug 污染并已落盘为
        // v2** 的存档不会再走迁移分支自愈：它带「孤儿 `pack_<id>` 容器（派生自某穿戴背包件）却
        // 在 equipped 里找不到对应背包件」的指纹（实测 Kizun3Desu：equipped 空、伪皮被冲进
        // body_pocket、worn_grass_pouch 连同 iron_sword 全丢，只剩 pack_11 孤儿容器）。
        // 这类存档已无法恢复丢失件（iron_sword 真丢了），最干净的恢复是丢弃污染存档、回落
        // 默认 loadout（与 fresh join 一致）。判定指纹极窄：`pack_<id>` 容器只可能由穿戴背包件
        // 派生，故「有 pack_<id> 容器但 equipped 无对应 instance」⇒ 必是 #736 污染，绝不会误伤
        // 合法裸装玩家（裸装 equipped 空但也无任何 pack_<id> 容器）。
        if inventory_has_orphan_pack_container(&inventory) {
            tracing::warn!(
                "[bong][player] detected #736-corrupted v2 inventory for `{username}` (orphan pack_* container without backing worn item); discarding corrupt save and falling back to default loadout"
            );
            return Ok(None);
        }

        return Ok(Some(inventory));
    }

    // 旧版本（v1）：解析为 Value → 迁移完整 inventory layout → 反序列化。
    let value: serde_json::Value = serde_json::from_str(&inventory_json)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let outcome = crate::inventory::migrate_legacy_inventory_layout(value, schema_version);
    if let Some(error) = outcome.error {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("legacy inventory layout migration failed: {error:?}"),
        ));
    }
    let mut inventory = serde_json::from_value::<PlayerInventory>(outcome.migrated_value)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    backfill_owner_instance_ids(&mut inventory);

    if !outcome.overflow.is_empty() {
        let Some(spill_context) = spill_context else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "legacy inventory overflow requires a position and dimension spill context",
            ));
        };
        persist_legacy_inventory_migration(
            connection,
            username,
            &inventory,
            &outcome.overflow,
            spill_context,
        )?;
    }

    Ok(Some(inventory))
}

/// 识别 `load_player_inventory_from_sqlite` 返回 `None` 时究竟是缺行/显式 `null`，还是
/// 旧存档校验后主动丢弃了一个损坏载荷。后者必须进入 `Failed`，否则聚合 writer 会把
/// 默认库存写回并覆盖待人工恢复的原始 JSON。
fn inventory_row_has_payload(connection: &Connection, username: &str) -> io::Result<bool> {
    let inventory_json: Option<String> = connection
        .query_row(
            "SELECT inventory_json FROM inventories WHERE username = ?1",
            params![username],
            |row| row.get(0),
        )
        .optional()
        .map_err(io::Error::other)?;
    Ok(inventory_json.is_some_and(|json| json.trim() != DEFAULT_INVENTORY_JSON))
}

/// Commit a legacy layout migration and its overflow handoff together.
///
/// 旧库存行保持不变，直到所有 overflow 实例都写入 durable dropped-loot 表；写入失败时
/// 原始 JSON 仍可重试，hydration 不会静默删除物品。
fn persist_legacy_inventory_migration(
    connection: &mut Connection,
    username: &str,
    inventory: &PlayerInventory,
    overflow: &[crate::inventory::ItemInstance],
    spill_context: LegacyInventorySpillContext,
) -> io::Result<()> {
    let entries = overflow
        .iter()
        .map(|item| crate::inventory::DroppedLootEntry {
            instance_id: item.instance_id,
            source_container_id: format!("legacy_inventory_overflow:{username}"),
            source_row: 0,
            source_col: 0,
            world_pos: spill_context.world_pos,
            dimension: spill_context.dimension,
            item: item.clone(),
        })
        .collect::<Vec<_>>();
    let inventory_json = serialize_inventory_json(Some(inventory))?;
    let last_updated_wall = current_unix_seconds();
    let transaction = connection.transaction().map_err(io::Error::other)?;

    let current_count: i64 = transaction
        .query_row("SELECT COUNT(*) FROM dropped_loot", [], |row| row.get(0))
        .map_err(io::Error::other)?;
    let current_count = usize::try_from(current_count).map_err(io::Error::other)?;
    if entries.len() > MAX_DURABLE_DROPPED_LOOT_ENTRIES.saturating_sub(current_count) {
        return Err(io::Error::other(format!(
            "legacy inventory overflow exceeds dropped-loot capacity: current={current_count}, required={}, limit={MAX_DURABLE_DROPPED_LOOT_ENTRIES}",
            entries.len()
        )));
    }

    let mut instance_ids = HashSet::with_capacity(entries.len());
    let mut persisted_ids = Vec::with_capacity(entries.len());
    for entry in &entries {
        if !instance_ids.insert(entry.instance_id) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "legacy inventory overflow contains duplicate instance {}",
                    entry.instance_id
                ),
            ));
        }
        persisted_ids.push(i64::try_from(entry.instance_id).map_err(io::Error::other)?);
    }
    let existing_ids = load_existing_dropped_loot_ids_in_batches(&transaction, &persisted_ids)?;
    for entry in &entries {
        if existing_ids.contains(&entry.instance_id) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "legacy inventory overflow instance {} already exists in dropped_loot",
                    entry.instance_id
                ),
            ));
        }
    }

    crate::persistence::upsert_dropped_loot_entries(&transaction, &entries, last_updated_wall)?;
    persist_player_inventory_json_in_transaction(
        &transaction,
        username,
        &inventory_json,
        last_updated_wall,
    )?;
    transaction.commit().map_err(io::Error::other)
}

/// Look up legacy overflow ids with bounded `IN` lists instead of one SQLite query per item.
fn load_existing_dropped_loot_ids_in_batches(
    transaction: &rusqlite::Transaction<'_>,
    persisted_ids: &[i64],
) -> io::Result<HashSet<u64>> {
    let mut existing_ids = HashSet::new();
    for batch in persisted_ids.chunks(DROPPED_LOOT_ID_QUERY_BATCH_SIZE) {
        let placeholders = std::iter::repeat_n("?", batch.len())
            .collect::<Vec<_>>()
            .join(", ");
        let query =
            format!("SELECT instance_id FROM dropped_loot WHERE instance_id IN ({placeholders})");
        let mut statement = transaction.prepare(&query).map_err(io::Error::other)?;
        let rows = statement
            .query_map(params_from_iter(batch.iter()), |row| row.get::<_, i64>(0))
            .map_err(io::Error::other)?;
        for row in rows {
            let id = row.map_err(io::Error::other)?;
            existing_ids.insert(u64::try_from(id).map_err(io::Error::other)?);
        }
    }
    Ok(existing_ids)
}

fn load_player_craft_session_from_sqlite(
    connection: &Connection,
    username: &str,
) -> io::Result<Option<CraftSession>> {
    let session_json: Option<String> = connection
        .query_row(
            "SELECT session_json FROM player_craft_sessions WHERE username = ?1",
            params![username],
            |row| row.get(0),
        )
        .optional()
        .map_err(io::Error::other)?;
    session_json
        .map(|json| {
            serde_json::from_str::<CraftSession>(&json)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
        })
        .transpose()
}

fn load_player_status_effects_from_sqlite(
    connection: &Connection,
    username: &str,
) -> io::Result<Option<StatusEffects>> {
    let status_json: Option<String> = connection
        .query_row(
            "SELECT status_effects_json FROM player_status_effects WHERE username = ?1",
            params![username],
            |row| row.get(0),
        )
        .optional()
        .map_err(io::Error::other)?;
    status_json
        .map(|json| {
            serde_json::from_str::<StatusEffects>(&json)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
        })
        .transpose()
}

/// plan-layered-equip-v1 P0.6（决议 #4 / #17）— inventory v1→v2 存档迁移（原地改写 Value）。
///
/// v1 `equipped` 形态：`{ "<slot>": <ItemInstance object>, ... }`（每槽单件）。
/// v2 形态：`{ "<slot>": { "worn": [<ItemInstance>...], "held": <ItemInstance|null> }, ... }`。
///
/// 旧专槽映射去向（决议 #4 定死）：
/// - `false_skin` → `chest.worn` 追加一件（伪皮归胸槽 worn 层，决议 #9）。
/// - `two_hand` → `main_hand.held`（对侧 off_hand lock 由 P1 状态机 load 后重算，迁移只落 held，决议 #7）。
/// - `treasure_belt_0..3` → **迁入触发位**（`triggered_treasures`，法宝激活态改由灵宝 UI 触发位承载，
///   决议 #8）。按 belt 槽序追加，超出 `TREASURE_TRIGGER_CAP` 的多余件丢弃（旧 belt 只有 4 槽，正常不会超）。
/// - `back_pack/waist_pouch/chest_satchel` → 归 `chest.worn`（旧背包件按身体槽 worn 落位，决议 #17；
///   现存档背包均 back_pack，默认落 chest worn 栈尾，与 default.toml worn_grass_pouch→chest 一致）。
///   **同时**把同名静态容器（旧档容器 id == 旧装备槽名）改名到运行时 `pack_<instance_id>`
///   命名空间（Bug3），否则装在旧 back_pack 容器里的物品会被 rebuild_containers_from_equipment
///   留成无主孤儿。
/// - `extra_hand_0/1` → `<slot>.held`（武器落 held，不误塞多件）。
/// - `head/chest/legs/feet` → `<slot>.worn`（盔甲穿戴层）。
/// - `main_hand/off_hand` → `<slot>.held`（手持武器/工具）。
#[cfg(test)]
fn migrate_equipped_v1_to_v2(value: &mut serde_json::Value) {
    crate::inventory::layout::migrate_equipped_v1_to_v2(value);
}

fn load_player_ui_prefs_from_sqlite(
    connection: &Connection,
    username: &str,
) -> io::Result<PlayerUiPrefs> {
    Ok(load_player_ui_prefs_optional_from_sqlite(connection, username)?.unwrap_or_default())
}

fn load_player_ui_prefs_optional_from_sqlite(
    connection: &Connection,
    username: &str,
) -> io::Result<Option<PlayerUiPrefs>> {
    let prefs_json: Option<String> = connection
        .query_row(
            "
            SELECT prefs_json
            FROM player_ui_prefs
            WHERE username = ?1
            ",
            params![username],
            |row| row.get(0),
        )
        .optional()
        .map_err(io::Error::other)?;

    let Some(prefs_json) = prefs_json else {
        return Ok(None);
    };

    serde_json::from_str::<PlayerUiPrefs>(&prefs_json)
        .map(Some)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn persist_player_ui_prefs_slice_in_sqlite(
    connection: &mut Connection,
    username: &str,
    prefs: &PlayerUiPrefs,
) -> io::Result<()> {
    let prefs_json = serde_json::to_string(prefs)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let last_updated_wall = current_unix_seconds();

    connection
        .execute(
            "
            INSERT INTO player_ui_prefs (
                username,
                prefs_json,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(username) DO UPDATE SET
                prefs_json = excluded.prefs_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                username,
                prefs_json,
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;

    Ok(())
}

fn load_player_lifespan_from_sqlite(
    connection: &Connection,
    username: &str,
) -> io::Result<Option<(LifespanComponent, bool, CoffinGrade)>> {
    let row: Option<(u64, f64, u32, i64, i64, Option<String>)> = connection
        .query_row(
            "
            SELECT born_at_tick, years_lived, cap_by_realm, offline_pause_wall, in_coffin,
                   coffin_grade
            FROM player_lifespan
            WHERE username = ?1
            ",
            params![username],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .optional()
        .map_err(io::Error::other)?;

    let Some((born_at_tick, years_lived, cap_by_realm, offline_pause_wall, in_coffin, grade_str)) =
        row
    else {
        return Ok(None);
    };
    let in_coffin = in_coffin != 0;
    // coffin_grade 列可能在旧库中缺失（legacy 行 grade_str = None），默认 Mundane
    let coffin_grade = grade_str
        .as_deref()
        .map(CoffinGrade::from_db_str)
        .unwrap_or_default();
    let grade_for_multiplier = if in_coffin { Some(coffin_grade) } else { None };
    let now_wall = current_unix_seconds();
    let offline_seconds = if offline_pause_wall > 0 {
        u64::try_from(now_wall.saturating_sub(offline_pause_wall)).unwrap_or(0)
    } else {
        0
    };
    let years_lived = years_lived
        + lifespan_delta_years_for_real_seconds(
            offline_seconds,
            offline_lifespan_multiplier(grade_for_multiplier),
        );
    let mut lifespan = LifespanComponent {
        born_at_tick,
        years_lived: years_lived.min(cap_by_realm as f64),
        cap_by_realm,
        offline_pause_tick: None,
    };
    lifespan.apply_cap(cap_by_realm.max(1));
    Ok(Some((lifespan, in_coffin, coffin_grade)))
}

/// 离线寿元倍率：Some(grade) = 在棺内按档折减；None = 不在棺内，按基础 OFFLINE 速率衰减
pub(crate) fn offline_lifespan_multiplier(grade: Option<CoffinGrade>) -> f64 {
    match grade {
        Some(g) => LIFESPAN_OFFLINE_MULTIPLIER * g.lifespan_factor(),
        None => LIFESPAN_OFFLINE_MULTIPLIER,
    }
}

fn load_player_shrine_anchor_from_sqlite(
    connection: &Connection,
    username: &str,
) -> io::Result<Option<[f64; 3]>> {
    let row: Option<(f64, f64, f64)> = connection
        .query_row(
            "
            SELECT anchor_x, anchor_y, anchor_z
            FROM player_shrine
            WHERE username = ?1
            ",
            params![username],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(io::Error::other)?;
    Ok(row.map(|(x, y, z)| [x, y, z]))
}

fn persist_player_shrine_anchor_slice_in_sqlite(
    connection: &mut Connection,
    username: &str,
    anchor: Option<[f64; 3]>,
) -> io::Result<()> {
    let last_updated_wall = current_unix_seconds();

    match anchor {
        Some([x, y, z]) => {
            connection
                .execute(
                    "
                    INSERT INTO player_shrine (
                        username,
                        anchor_x,
                        anchor_y,
                        anchor_z,
                        schema_version,
                        last_updated_wall
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                    ON CONFLICT(username) DO UPDATE SET
                        anchor_x = excluded.anchor_x,
                        anchor_y = excluded.anchor_y,
                        anchor_z = excluded.anchor_z,
                        schema_version = excluded.schema_version,
                        last_updated_wall = excluded.last_updated_wall
                    ",
                    params![
                        username,
                        x,
                        y,
                        z,
                        PLAYER_ROW_SCHEMA_VERSION,
                        last_updated_wall
                    ],
                )
                .map_err(io::Error::other)?;
        }
        None => {
            connection
                .execute(
                    "DELETE FROM player_shrine WHERE username = ?1",
                    params![username],
                )
                .map_err(io::Error::other)?;
        }
    }

    Ok(())
}

fn persist_player_lifespan_slice_in_sqlite(
    connection: &mut Connection,
    username: &str,
    lifespan: &LifespanComponent,
    offline_pause_wall: Option<i64>,
    in_coffin: Option<bool>,
    // None = 回读 DB 既有 grade（无棺上下文保存路径，防止洗掉 Jade/Stone/Bronze）
    coffin_grade: Option<CoffinGrade>,
) -> io::Result<()> {
    let last_updated_wall = current_unix_seconds();
    let offline_pause_wall = offline_pause_wall.unwrap_or(last_updated_wall).max(0);
    let in_coffin = resolve_in_coffin_for_persist(connection, username, in_coffin)?;
    let coffin_grade = resolve_coffin_grade_for_persist(connection, username, coffin_grade)?;
    connection
        .execute(
            "
            INSERT INTO player_lifespan (
                username,
                born_at_tick,
                years_lived,
                cap_by_realm,
                offline_pause_wall,
                in_coffin,
                coffin_grade,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(username) DO UPDATE SET
                born_at_tick = excluded.born_at_tick,
                years_lived = excluded.years_lived,
                cap_by_realm = excluded.cap_by_realm,
                offline_pause_wall = excluded.offline_pause_wall,
                in_coffin = excluded.in_coffin,
                coffin_grade = excluded.coffin_grade,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                username,
                lifespan.born_at_tick,
                lifespan.years_lived.min(lifespan.cap_by_realm as f64),
                lifespan.cap_by_realm,
                offline_pause_wall,
                i64::from(in_coffin),
                coffin_grade.as_db_str(),
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

fn resolve_in_coffin_for_persist(
    connection: &Connection,
    username: &str,
    explicit: Option<bool>,
) -> io::Result<bool> {
    if let Some(value) = explicit {
        return Ok(value);
    }
    let stored: Option<i64> = connection
        .query_row(
            "SELECT in_coffin FROM player_lifespan WHERE username = ?1",
            params![username],
            |row| row.get(0),
        )
        .optional()
        .map_err(io::Error::other)?;
    Ok(stored.unwrap_or(0) != 0)
}

/// 保护 coffin_grade 不被无棺上下文的保存路径洗掉。
/// explicit=Some(g) 时直接返回 g；
/// explicit=None 时回读 DB 既有 grade（例如悟道延寿保存路径），避免 ON CONFLICT 无条件覆成 mundane。
fn resolve_coffin_grade_for_persist(
    connection: &Connection,
    username: &str,
    explicit: Option<CoffinGrade>,
) -> io::Result<CoffinGrade> {
    if let Some(grade) = explicit {
        return Ok(grade);
    }
    let stored: Option<String> = connection
        .query_row(
            "SELECT coffin_grade FROM player_lifespan WHERE username = ?1",
            params![username],
            |row| row.get(0),
        )
        .optional()
        .map_err(io::Error::other)?;
    Ok(stored
        .as_deref()
        .map(CoffinGrade::from_db_str)
        .unwrap_or_default())
}

fn load_player_skill_set_from_sqlite(
    connection: &Connection,
    username: &str,
) -> io::Result<SkillSet> {
    let skill_set_json: Option<String> = connection
        .query_row(
            "
            SELECT skill_set_json
            FROM player_skills
            WHERE username = ?1
            ",
            params![username],
            |row| row.get(0),
        )
        .optional()
        .map_err(io::Error::other)?;

    let Some(skill_set_json) = skill_set_json else {
        return Ok(SkillSet::default());
    };

    serde_json::from_str::<SkillSet>(&skill_set_json)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub(crate) fn load_player_known_techniques_slice(
    persistence: &PlayerStatePersistence,
    username: &str,
) -> io::Result<Option<KnownTechniques>> {
    let connection = open_player_connection(persistence)?;
    load_player_known_techniques_from_sqlite(&connection, username)
}

fn load_player_known_techniques_from_sqlite(
    connection: &Connection,
    username: &str,
) -> io::Result<Option<KnownTechniques>> {
    let known_techniques_json: Option<String> = connection
        .query_row(
            "
            SELECT known_techniques_json
            FROM player_known_techniques
            WHERE username = ?1
            ",
            params![username],
            |row| row.get(0),
        )
        .optional()
        .map_err(io::Error::other)?;

    let Some(known_techniques_json) = known_techniques_json else {
        return Ok(None);
    };

    serde_json::from_str::<KnownTechniques>(&known_techniques_json)
        .map(Some)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

/// bughunt player-lifecycle-relog-death-consequence-wipe：`None` = 未落过盘（vs. 老档 /
/// 首次登录），调用方要能区分"从未持久化"和"反序列化失败"（后者仍走 `Err` fail-loud，
/// 不静默吞成 `None` 掩盖坏数据）。
///
/// 读回后会把两个"绝对 tick" deadline 字段（
/// `revival_decision_deadline_tick`/`weakened_until_tick`）折算到 `current_combat_clock_tick`
/// 所在的 tick 空间——见 `translate_lifecycle_deadline_tick_across_restart`。
fn load_player_lifecycle_from_sqlite(
    connection: &Connection,
    username: &str,
    current_combat_clock_tick: u64,
) -> io::Result<Option<crate::combat::components::Lifecycle>> {
    let row: Option<(String, i64, u64)> = connection
        .query_row(
            "
            SELECT lifecycle_json, last_updated_wall, combat_clock_tick_at_save
            FROM player_lifecycle
            WHERE username = ?1
            ",
            params![username],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(io::Error::other)?;

    let Some((lifecycle_json, last_updated_wall, combat_clock_tick_at_save)) = row else {
        return Ok(None);
    };

    let mut lifecycle =
        serde_json::from_str::<crate::combat::components::Lifecycle>(&lifecycle_json)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    let now_wall = current_unix_seconds();
    lifecycle.revival_decision_deadline_tick = translate_lifecycle_deadline_tick_across_restart(
        lifecycle.revival_decision_deadline_tick,
        combat_clock_tick_at_save,
        last_updated_wall,
        now_wall,
        current_combat_clock_tick,
    );
    lifecycle.weakened_until_tick = translate_lifecycle_deadline_tick_across_restart(
        lifecycle.weakened_until_tick,
        combat_clock_tick_at_save,
        last_updated_wall,
        now_wall,
        current_combat_clock_tick,
    );

    Ok(Some(lifecycle))
}

/// bughunt player-lifecycle-relog-death-consequence-wipe（OPUS 返工要求 1）：把落盘时刻记录
/// 的"绝对 tick" deadline 折算到读档当刻的 `CombatClock` tick 空间。
///
/// `CombatClock` 每次进程重启都从 0 重新计数（`combat::mod::register` 里
/// `insert_resource(CombatClock::default())`），全仓没有任何从持久化恢复 tick 的代码。
/// `revival_decision_deadline_tick`/`weakened_until_tick` 都是
/// 落盘那一刻算出的"绝对 tick"值——若跨重启直接复用，新进程 tick=0 时，旧 deadline 动辄
/// 百万级，等价于几十小时后才会被 `auto_confirm_revival_decisions`
/// 结算，期间玩家会卡在 AwaitingRevival（`resolve.rs` 同时禁止攻击与被攻击）却没有任何
/// UI 解释为什么，然后在数小时后的随机时刻被强制渡劫、可能永久终结角色。
///
/// 用 `combat_clock_tick_at_save`（落盘时刻 CombatClock.tick）+ `last_updated_wall`
/// （落盘时刻墙钟）两个锚点，按真实流逝的墙钟秒数折算出"距 deadline 还剩多少 tick"，
/// 再叠加到 `current_combat_clock_tick` 上，重建一个在当前 tick 空间里有意义的新
/// deadline——镜像 `player_lifespan.offline_pause_wall` 按墙钟折算离线寿元的模式
/// （`load_player_lifespan_from_sqlite`）。同一进程内断线重连（未重启）时，这个折算
/// 结果应与直接复用旧绝对值几乎一致（wall 流逝与 tick 流逝理论上同步，±1 tick 抖动可忽略）；
/// 跨重启时则会正确地把"早已过期"的 deadline 立即结算（`remaining_now` 饱和到 0），而不是
/// 把它错当成几十小时后的未来事件。
fn translate_lifecycle_deadline_tick_across_restart(
    deadline_tick: Option<u64>,
    combat_clock_tick_at_save: u64,
    last_updated_wall: i64,
    now_wall: i64,
    current_combat_clock_tick: u64,
) -> Option<u64> {
    let deadline_tick = deadline_tick?;
    let remaining_at_save = deadline_tick.saturating_sub(combat_clock_tick_at_save);
    // now_wall < last_updated_wall（系统时钟回拨）时按 0 流逝处理，不倒推出负数流逝时间。
    let elapsed_wall_seconds = now_wall.saturating_sub(last_updated_wall).max(0) as u64;
    let elapsed_ticks =
        elapsed_wall_seconds.saturating_mul(crate::combat::components::TICKS_PER_SECOND);
    let remaining_now = remaining_at_save.saturating_sub(elapsed_ticks);
    Some(current_combat_clock_tick.saturating_add(remaining_now))
}

fn persist_player_core_slice_in_sqlite(
    connection: &mut Connection,
    username: &str,
    state: &PlayerState,
) -> io::Result<()> {
    let normalized = state.normalized();
    let last_updated_wall = current_unix_seconds();
    let updated = connection
        .execute(
            "
            UPDATE player_core
            SET karma = ?2,
                inventory_score = ?3,
                schema_version = ?4,
                last_updated_wall = ?5
            WHERE username = ?1
            ",
            params![
                username,
                normalized.karma,
                normalized.inventory_score,
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;

    if updated == 0 {
        // 核心切片首次落盘只建立角色锚点。其它切片的载入 provenance 可能仍是
        // Failed，不能借这个 fallback 把默认值写进未知的 durable row。
        let current_char_id = Uuid::now_v7().to_string();
        connection
            .execute(
                "
                INSERT INTO player_core (
                    username,
                    current_char_id,
                    karma,
                    inventory_score,
                    schema_version,
                    last_updated_wall
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ",
                params![
                    username,
                    current_char_id,
                    normalized.karma,
                    normalized.inventory_score,
                    PLAYER_ROW_SCHEMA_VERSION,
                    last_updated_wall
                ],
            )
            .map_err(io::Error::other)?;
    }

    Ok(())
}

fn persist_player_slow_slice_in_sqlite(
    connection: &mut Connection,
    username: &str,
    position: [f64; 3],
    last_dimension: DimensionKind,
) -> io::Result<()> {
    let [pos_x, pos_y, pos_z] = position;
    let last_updated_wall = current_unix_seconds();
    let prefs_json = default_ui_prefs_json()?;

    connection
        .execute(
            "
            INSERT INTO player_slow (
                username,
                pos_x,
                pos_y,
                pos_z,
                last_dimension,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(username) DO UPDATE SET
                pos_x = excluded.pos_x,
                pos_y = excluded.pos_y,
                pos_z = excluded.pos_z,
                last_dimension = excluded.last_dimension,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                username,
                pos_x,
                pos_y,
                pos_z,
                dimension_kind_to_sql(last_dimension),
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;
    connection
        .execute(
            "
            INSERT OR IGNORE INTO player_ui_prefs (
                username,
                prefs_json,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ",
            params![
                username,
                prefs_json,
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;
    connection
        .execute(
            "
            UPDATE player_ui_prefs
            SET schema_version = ?2,
                last_updated_wall = ?3
            WHERE username = ?1
            ",
            params![username, PLAYER_ROW_SCHEMA_VERSION, last_updated_wall],
        )
        .map_err(io::Error::other)?;

    Ok(())
}

fn persist_player_inventory_slice_in_sqlite(
    connection: &mut Connection,
    username: &str,
    inventory: Option<&PlayerInventory>,
) -> io::Result<()> {
    let inventory_json = serialize_inventory_json(inventory)?;
    let last_updated_wall = current_unix_seconds();

    connection
        .execute(
            "
            INSERT INTO inventories (
                username,
                inventory_json,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(username) DO UPDATE SET
                inventory_json = excluded.inventory_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                username,
                inventory_json,
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;

    Ok(())
}

fn persist_player_skill_slice_in_sqlite(
    connection: &mut Connection,
    username: &str,
    skill_set: &SkillSet,
) -> io::Result<()> {
    let skill_set_json = serialize_skill_set_json(skill_set)?;
    let last_updated_wall = current_unix_seconds();

    connection
        .execute(
            "
            INSERT INTO player_skills (
                username,
                skill_set_json,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(username) DO UPDATE SET
                skill_set_json = excluded.skill_set_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                username,
                skill_set_json,
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;

    Ok(())
}

fn persist_player_known_techniques_slice_in_sqlite(
    connection: &mut Connection,
    username: &str,
    known_techniques: &KnownTechniques,
) -> io::Result<()> {
    let known_techniques_json = serialize_known_techniques_json(known_techniques)?;
    let last_updated_wall = current_unix_seconds();

    connection
        .execute(
            "
            INSERT INTO player_known_techniques (
                username,
                known_techniques_json,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(username) DO UPDATE SET
                known_techniques_json = excluded.known_techniques_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                username,
                known_techniques_json,
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;

    Ok(())
}

fn persist_player_status_effects_slice_in_sqlite(
    connection: &mut Connection,
    username: &str,
    status_effects: &StatusEffects,
) -> io::Result<()> {
    let status_effects_json = serde_json::to_string(status_effects)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let last_updated_wall = current_unix_seconds();
    connection
        .execute(
            "
            INSERT INTO player_status_effects (
                username,
                status_effects_json,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(username) DO UPDATE SET
                status_effects_json = excluded.status_effects_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                username,
                status_effects_json,
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

/// bughunt player-lifecycle-relog-death-consequence-wipe：整份 `Lifecycle` 组件镜像进
/// `player_lifecycle.lifecycle_json`（同 known_techniques 的单 JSON 列模式），覆盖
/// state/fortune_remaining/awaiting_decision/各 deadline tick —— 调用方（断线清理 /
/// 关服 flush）必须传入断连那一刻的真实组件值，不得传入尚未跑过状态转换的陈旧快照。
///
/// `combat_clock_tick` 是落盘那一刻的 `CombatClock.tick`，写入 `combat_clock_tick_at_save`
/// 列，作为读档时折算"绝对 tick" deadline 的锚点（见 `load_player_lifecycle_from_sqlite`）。
fn persist_player_lifecycle_slice_in_sqlite(
    connection: &mut Connection,
    username: &str,
    lifecycle: &crate::combat::components::Lifecycle,
    combat_clock_tick: u64,
) -> io::Result<()> {
    let lifecycle_json = serde_json::to_string(lifecycle)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let last_updated_wall = current_unix_seconds();

    connection
        .execute(
            "
            INSERT INTO player_lifecycle (
                username,
                lifecycle_json,
                schema_version,
                last_updated_wall,
                combat_clock_tick_at_save
            ) VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(username) DO UPDATE SET
                lifecycle_json = excluded.lifecycle_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall,
                combat_clock_tick_at_save = excluded.combat_clock_tick_at_save
            ",
            params![
                username,
                lifecycle_json,
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall,
                combat_clock_tick
            ],
        )
        .map_err(io::Error::other)?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn persist_player_slices_in_sqlite(
    connection: &mut Connection,
    username: &str,
    state: &PlayerState,
    position: [f64; 3],
    last_dimension: DimensionKind,
    inventory: Option<&PlayerInventory>,
    lifespan: Option<&LifespanComponent>,
    skill_set: &SkillSet,
    in_coffin: Option<bool>,
    coffin_grade: Option<CoffinGrade>,
    craft_session: Option<Option<&CraftSession>>,
) -> io::Result<()> {
    persist_player_slices_in_sqlite_with_write_set(
        connection,
        username,
        state,
        position,
        last_dimension,
        inventory,
        lifespan,
        skill_set,
        in_coffin,
        coffin_grade,
        craft_session,
        None,
        WriteSet::all(),
    )
}

#[allow(clippy::too_many_arguments)]
fn persist_player_slices_in_sqlite_with_write_set(
    connection: &mut Connection,
    username: &str,
    state: &PlayerState,
    position: [f64; 3],
    last_dimension: DimensionKind,
    inventory: Option<&PlayerInventory>,
    lifespan: Option<&LifespanComponent>,
    _skill_set: &SkillSet,
    in_coffin: Option<bool>,
    // None = 回读 DB 既有 grade（无棺上下文保存路径，防止洗掉 Jade/Stone/Bronze）
    coffin_grade: Option<CoffinGrade>,
    // None = 调用方无 craft 上下文，保留数据库原值；Some(None) = 删除 session。
    craft_session: Option<Option<&CraftSession>>,
    status_effects: Option<&StatusEffects>,
    write_set: WriteSet,
) -> io::Result<()> {
    let normalized = state.normalized();
    let karma = normalized.karma;
    let inventory_score = normalized.inventory_score;
    let [pos_x, pos_y, pos_z] = position;
    let inventory_json = write_set
        .contains(PlayerSlice::Inventory)
        .then(|| serialize_inventory_json(inventory))
        .transpose()?;
    let known_techniques_json = write_set
        .contains(PlayerSlice::KnownTechniques)
        .then(|| serialize_known_techniques_json(&KnownTechniques::default()))
        .transpose()?;
    let last_updated_wall = current_unix_seconds();
    let prefs_json = write_set
        .contains(PlayerSlice::UiPrefs)
        .then(default_ui_prefs_json)
        .transpose()?;
    let craft_session_json = write_set
        .contains(PlayerSlice::Craft)
        .then(|| craft_session.flatten().map(serde_json::to_string))
        .flatten()
        .transpose()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let in_coffin_value = write_set
        .contains(PlayerSlice::Lifespan)
        .then(|| resolve_in_coffin_for_persist(connection, username, in_coffin))
        .transpose()?;
    let coffin_grade_value = write_set
        .contains(PlayerSlice::Lifespan)
        .then(|| resolve_coffin_grade_for_persist(connection, username, coffin_grade))
        .transpose()?;

    let transaction = connection.transaction().map_err(io::Error::other)?;
    let current_char_id = if write_set.contains(PlayerSlice::Core) {
        let current_char_id: Option<String> = transaction
            .query_row(
                "SELECT current_char_id FROM player_core WHERE username = ?1",
                params![username],
                |row| row.get(0),
            )
            .optional()
            .map_err(io::Error::other)?;
        Some(current_char_id.unwrap_or_else(|| Uuid::now_v7().to_string()))
    } else {
        None
    };

    if let Some(current_char_id) = current_char_id {
        transaction
            .execute(
                "
            INSERT INTO player_core (
                username,
                current_char_id,
                karma,
                inventory_score,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(username) DO UPDATE SET
                current_char_id = excluded.current_char_id,
                karma = excluded.karma,
                inventory_score = excluded.inventory_score,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
                params![
                    username,
                    current_char_id,
                    karma,
                    inventory_score,
                    PLAYER_ROW_SCHEMA_VERSION,
                    last_updated_wall
                ],
            )
            .map_err(io::Error::other)?;
    }

    if write_set.contains(PlayerSlice::Position) {
        transaction
            .execute(
                "
            INSERT INTO player_slow (
                username,
                pos_x,
                pos_y,
                pos_z,
                last_dimension,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(username) DO UPDATE SET
                pos_x = excluded.pos_x,
                pos_y = excluded.pos_y,
                pos_z = excluded.pos_z,
                last_dimension = excluded.last_dimension,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
                params![
                    username,
                    pos_x,
                    pos_y,
                    pos_z,
                    dimension_kind_to_sql(last_dimension),
                    PLAYER_ROW_SCHEMA_VERSION,
                    last_updated_wall
                ],
            )
            .map_err(io::Error::other)?;
    }
    if let Some(inventory_json) = inventory_json {
        transaction
            .execute(
                "
            INSERT INTO inventories (
                username,
                inventory_json,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(username) DO UPDATE SET
                inventory_json = excluded.inventory_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
                params![
                    username,
                    inventory_json,
                    PLAYER_ROW_SCHEMA_VERSION,
                    last_updated_wall
                ],
            )
            .map_err(io::Error::other)?;
    }
    if let Some(known_techniques_json) = known_techniques_json {
        transaction
            .execute(
                "
            INSERT OR IGNORE INTO player_known_techniques (
                username,
                known_techniques_json,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ",
                params![
                    username,
                    known_techniques_json,
                    PLAYER_ROW_SCHEMA_VERSION,
                    last_updated_wall
                ],
            )
            .map_err(io::Error::other)?;
    }
    if let Some(prefs_json) = prefs_json {
        transaction
            .execute(
                "
            INSERT OR IGNORE INTO player_ui_prefs (
                username,
                prefs_json,
                schema_version,
                last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4)
            ",
                params![
                    username,
                    prefs_json,
                    PLAYER_ROW_SCHEMA_VERSION,
                    last_updated_wall
                ],
            )
            .map_err(io::Error::other)?;
    }
    if write_set.contains(PlayerSlice::Lifespan) {
        if let (Some(lifespan), Some(in_coffin_value), Some(coffin_grade_value)) =
            (lifespan, in_coffin_value, coffin_grade_value)
        {
            let offline_pause_wall = last_updated_wall;
            transaction
                .execute(
                    "
                INSERT INTO player_lifespan (
                    username,
                    born_at_tick,
                    years_lived,
                    cap_by_realm,
                    offline_pause_wall,
                    in_coffin,
                    coffin_grade,
                    schema_version,
                    last_updated_wall
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                ON CONFLICT(username) DO UPDATE SET
                    born_at_tick = excluded.born_at_tick,
                    years_lived = excluded.years_lived,
                    cap_by_realm = excluded.cap_by_realm,
                    offline_pause_wall = excluded.offline_pause_wall,
                    in_coffin = excluded.in_coffin,
                    coffin_grade = excluded.coffin_grade,
                    schema_version = excluded.schema_version,
                    last_updated_wall = excluded.last_updated_wall
                ",
                    params![
                        username,
                        lifespan.born_at_tick,
                        lifespan.years_lived.min(lifespan.cap_by_realm as f64),
                        lifespan.cap_by_realm,
                        offline_pause_wall,
                        i64::from(in_coffin_value),
                        coffin_grade_value.as_db_str(),
                        PLAYER_ROW_SCHEMA_VERSION,
                        last_updated_wall
                    ],
                )
                .map_err(io::Error::other)?;
        }
    }
    if write_set.contains(PlayerSlice::Craft) && craft_session.is_some() {
        persist_player_craft_session_in_transaction(
            &transaction,
            username,
            craft_session_json.as_deref(),
            last_updated_wall,
        )?;
    }
    if write_set.contains(PlayerSlice::LongTermBuff) {
        if let Some(status_effects) = status_effects {
            let status_effects_json = serde_json::to_string(status_effects)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            transaction
                .execute(
                    "
                    INSERT INTO player_status_effects (
                        username, status_effects_json, schema_version, last_updated_wall
                    ) VALUES (?1, ?2, ?3, ?4)
                    ON CONFLICT(username) DO UPDATE SET
                        status_effects_json = excluded.status_effects_json,
                        schema_version = excluded.schema_version,
                        last_updated_wall = excluded.last_updated_wall
                    ",
                    params![
                        username,
                        status_effects_json,
                        PLAYER_ROW_SCHEMA_VERSION,
                        last_updated_wall
                    ],
                )
                .map_err(io::Error::other)?;
        }
    }
    transaction.commit().map_err(io::Error::other)
}

fn persist_player_inventory_json_in_transaction(
    transaction: &rusqlite::Transaction<'_>,
    username: &str,
    inventory_json: &str,
    last_updated_wall: i64,
) -> io::Result<()> {
    transaction
        .execute(
            "
            INSERT INTO inventories (username, inventory_json, schema_version, last_updated_wall)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(username) DO UPDATE SET
                inventory_json = excluded.inventory_json,
                schema_version = excluded.schema_version,
                last_updated_wall = excluded.last_updated_wall
            ",
            params![
                username,
                inventory_json,
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;
    Ok(())
}

fn persist_player_craft_session_in_transaction(
    transaction: &rusqlite::Transaction<'_>,
    username: &str,
    session_json: Option<&str>,
    last_updated_wall: i64,
) -> io::Result<()> {
    if let Some(session_json) = session_json {
        transaction
            .execute(
                "
                INSERT INTO player_craft_sessions (
                    username, session_json, schema_version, last_updated_wall
                ) VALUES (?1, ?2, ?3, ?4)
                ON CONFLICT(username) DO UPDATE SET
                    session_json = excluded.session_json,
                    schema_version = excluded.schema_version,
                    last_updated_wall = excluded.last_updated_wall
                ",
                params![
                    username,
                    session_json,
                    PLAYER_ROW_SCHEMA_VERSION,
                    last_updated_wall
                ],
            )
            .map_err(io::Error::other)?;
    } else {
        transaction
            .execute(
                "DELETE FROM player_craft_sessions WHERE username = ?1",
                params![username],
            )
            .map_err(io::Error::other)?;
    }
    Ok(())
}

fn ensure_player_auxiliary_rows(connection: &mut Connection, username: &str) -> io::Result<()> {
    let last_updated_wall = current_unix_seconds();
    let prefs_json = default_ui_prefs_json()?;
    let transaction = connection.transaction().map_err(io::Error::other)?;
    insert_default_player_slice_rows(&transaction, username, last_updated_wall, &prefs_json)
        .map_err(io::Error::other)?;
    transaction.commit().map_err(io::Error::other)
}

fn insert_default_player_slice_rows(
    transaction: &rusqlite::Transaction<'_>,
    username: &str,
    last_updated_wall: i64,
    prefs_json: &str,
) -> rusqlite::Result<()> {
    let [pos_x, pos_y, pos_z] =
        crate::player::spawn_position_for_seed(username, SpawnPurpose::InitialLogin);
    let skill_set_json = serialize_skill_set_json(&SkillSet::default())
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    let known_techniques_json = serialize_known_techniques_json(&KnownTechniques::default())
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

    transaction.execute(
        "
        INSERT OR IGNORE INTO player_slow (
            username,
            pos_x,
            pos_y,
            pos_z,
            last_dimension,
            schema_version,
            last_updated_wall
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ",
        params![
            username,
            pos_x,
            pos_y,
            pos_z,
            dimension_kind_to_sql(DimensionKind::default()),
            PLAYER_ROW_SCHEMA_VERSION,
            last_updated_wall
        ],
    )?;
    transaction.execute(
        "
        INSERT OR IGNORE INTO inventories (
            username,
            inventory_json,
            schema_version,
            last_updated_wall
        ) VALUES (?1, ?2, ?3, ?4)
        ",
        params![
            username,
            DEFAULT_INVENTORY_JSON,
            PLAYER_ROW_SCHEMA_VERSION,
            last_updated_wall
        ],
    )?;
    transaction.execute(
        "
        INSERT OR IGNORE INTO player_skills (
            username,
            skill_set_json,
            schema_version,
            last_updated_wall
        ) VALUES (?1, ?2, ?3, ?4)
        ",
        params![
            username,
            skill_set_json,
            PLAYER_ROW_SCHEMA_VERSION,
            last_updated_wall
        ],
    )?;
    transaction.execute(
        "
        INSERT OR IGNORE INTO player_known_techniques (
            username,
            known_techniques_json,
            schema_version,
            last_updated_wall
        ) VALUES (?1, ?2, ?3, ?4)
        ",
        params![
            username,
            known_techniques_json,
            PLAYER_ROW_SCHEMA_VERSION,
            last_updated_wall
        ],
    )?;
    transaction.execute(
        "
        INSERT OR IGNORE INTO player_ui_prefs (
            username,
            prefs_json,
            schema_version,
            last_updated_wall
        ) VALUES (?1, ?2, ?3, ?4)
        ",
        params![
            username,
            prefs_json,
            PLAYER_ROW_SCHEMA_VERSION,
            last_updated_wall
        ],
    )?;

    Ok(())
}

fn migrate_legacy_player_json_to_sqlite(
    persistence: &PlayerStatePersistence,
    connection: &mut Connection,
    username: &str,
) -> io::Result<Option<PlayerState>> {
    let path = persistence.path_for_username(username);
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };

    let state = serde_json::from_str::<PlayerState>(&contents)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
        .normalized();
    persist_player_slices_in_sqlite(
        connection,
        username,
        &state,
        crate::player::spawn_position_for_seed(username, SpawnPurpose::InitialLogin),
        DimensionKind::default(),
        None,
        None,
        &SkillSet::default(),
        None,
        None,
        None,
    )?;
    fs::rename(&path, persistence.migrated_path_for_username(username))?;
    Ok(Some(state))
}

fn default_ui_prefs_json() -> io::Result<String> {
    serde_json::to_string(&PlayerUiPrefs::default())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn serialize_inventory_json(inventory: Option<&PlayerInventory>) -> io::Result<String> {
    match inventory {
        Some(inventory) => serde_json::to_string(inventory)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error)),
        None => Ok(DEFAULT_INVENTORY_JSON.to_string()),
    }
}

fn serialize_skill_set_json(skill_set: &SkillSet) -> io::Result<String> {
    serde_json::to_string(skill_set)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn serialize_known_techniques_json(known_techniques: &KnownTechniques) -> io::Result<String> {
    serde_json::to_string(known_techniques)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn dimension_kind_to_sql(kind: DimensionKind) -> &'static str {
    match kind {
        DimensionKind::Overworld => "overworld",
        DimensionKind::Tsy => "tsy",
    }
}

fn dimension_kind_from_sql(value: &str) -> io::Result<DimensionKind> {
    match value {
        "overworld" => Ok(DimensionKind::Overworld),
        "tsy" => Ok(DimensionKind::Tsy),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unknown dimension kind `{other}`"),
        )),
    }
}
fn current_unix_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_secs() as i64
}

fn ratio_score(value: f64, max: f64) -> f64 {
    if max <= 0.0 {
        0.0
    } else {
        (value / max).clamp(0.0, 1.0)
    }
}

fn clamp_unit(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

fn realm_progress_score(realm: Realm) -> f64 {
    // Realm score is only used for coarse power estimation in player/world snapshots.
    // Keep the mapping stable and monotonic across the six realms.
    match realm {
        Realm::Awaken => 0.05,
        Realm::Induce => 0.25,
        Realm::Condense => 0.4,
        Realm::Solidify => 0.55,
        Realm::Spirit => 0.75,
        Realm::Void => 1.0,
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
