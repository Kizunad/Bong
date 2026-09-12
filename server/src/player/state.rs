use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use valence::prelude::{bevy_ecs, Component, DVec3, Resource};

use crate::coffin::CoffinGrade;
use crate::combat::components::{QuickSlotBindings, SkillBarBindings, SkillSlot};
use crate::craft::CraftSession;
use crate::cultivation::components::{Cultivation, Realm};
use crate::cultivation::known_techniques::{
    has_dedicated_input_consumer, KnownTechniques, TechniqueDispatch, TechniqueRegistry,
};
use crate::cultivation::lifespan::{
    lifespan_delta_years_for_real_seconds, LifespanComponent, LIFESPAN_OFFLINE_MULTIPLIER,
};
use crate::inventory::{DroppedLootEntry, PlayerInventory};
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
const MIN_SAFE_PLAYER_Y: f64 = crate::world::terrain::MIN_Y as f64;
const MAX_SAFE_PLAYER_Y: f64 =
    (crate::world::terrain::MIN_Y + crate::world::terrain::WORLD_HEIGHT as i32 - 1) as f64;

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
    pub quick_slots: [Option<String>; QuickSlotBindings::SLOT_COUNT],
    #[serde(default)]
    pub skill_bar: [SkillSlotPersist; SkillBarBindings::SLOT_COUNT],
    #[serde(default)]
    pub skill_configs: BTreeMap<String, SkillConfig>,
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

        for (slot, template_id) in self.quick_slots.iter().enumerate() {
            let Some(template_id) = template_id.as_deref() else {
                continue;
            };
            if let Some(instance_id) = first_inventory_instance_for_template(inventory, template_id)
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
            let invalid = has_dedicated_input_consumer(skill_id)
                || registry.get(skill_id).is_none_or(|definition| {
                    definition.dispatch == TechniqueDispatch::DedicatedInput
                });
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
        let mut bindings = SkillBarBindings::default();
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
                    let valid = !has_dedicated_input_consumer(skill_id)
                        && registry.is_none_or(|registry| {
                            registry.get(skill_id).is_some_and(|definition| {
                                definition.dispatch != TechniqueDispatch::DedicatedInput
                            })
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
    pub in_coffin: bool,
    /// 棺材档级：Some(grade) = 在棺内 + 档级；None = 不在棺内（与 in_coffin=false 语义对齐）
    pub coffin_grade: Option<CoffinGrade>,
    pub skill_set: SkillSet,
    pub known_techniques: LoadedKnownTechniques,
    pub(crate) ui_prefs: PlayerUiPrefs,
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
    let state = load_player_state(persistence, username);
    let connection = match open_player_connection(persistence) {
        Ok(connection) => connection,
        Err(error) => {
            tracing::warn!(
                "[bong][player] failed to reopen sqlite player slice store for `{}` at {}: {error}; using default slow/inventory slices",
                username,
                persistence.db_path().display()
            );
            return LoadedPlayerSlices {
                state,
                position: crate::player::spawn_position_for_seed(
                    username,
                    SpawnPurpose::InitialLogin,
                ),
                last_dimension: DimensionKind::default(),
                inventory: None,
                craft_session: None,
                lifespan: None,
                in_coffin: false,
                coffin_grade: None,
                skill_set: SkillSet::default(),
                // 连接都打不开 = 行状态不可知（DB busy/文件不可达），
                // 必须按 LoadFailed 写保护，绝不能当「新玩家」用 default 覆盖写回
                known_techniques: LoadedKnownTechniques::LoadFailed,
                ui_prefs: PlayerUiPrefs::default(),
            };
        }
    };

    let (position, last_dimension) = match load_player_slow_from_sqlite(&connection, username) {
        Ok(Some((pos, dim))) => sanitize_loaded_position(username, pos, dim),
        Ok(None) => (
            crate::player::spawn_position_for_seed(username, SpawnPurpose::InitialLogin),
            DimensionKind::default(),
        ),
        Err(error) => {
            tracing::warn!(
                "[bong][player] failed to load persisted position/dimension for `{}` from sqlite {}: {error}; using spawn defaults",
                username,
                persistence.db_path().display()
            );
            (
                crate::player::spawn_position_for_seed(username, SpawnPurpose::InitialLogin),
                DimensionKind::default(),
            )
        }
    };
    let inventory = match load_player_inventory_from_sqlite(&connection, username) {
        Ok(inventory) => inventory,
        Err(error) => {
            tracing::warn!(
                "[bong][player] failed to load persisted inventory for `{}` from sqlite {}: {error}; using default inventory fallback",
                username,
                persistence.db_path().display()
            );
            None
        }
    };
    let craft_session = match load_player_craft_session_from_sqlite(&connection, username) {
        Ok(session) => session,
        Err(error) => {
            tracing::error!(
                "[bong][player] failed to load persisted craft session for `{}` from sqlite {}: {error}; refusing to invent a replacement session",
                username,
                persistence.db_path().display()
            );
            None
        }
    };
    let (lifespan, in_coffin, coffin_grade) = match load_player_lifespan_from_sqlite(
        &connection,
        username,
    ) {
        Ok(Some((lifespan, in_coffin, grade))) => {
            // coffin_grade = Some(grade) 当 in_coffin=true，None 当 in_coffin=false
            let coffin_grade = if in_coffin { Some(grade) } else { None };
            (Some(lifespan), in_coffin, coffin_grade)
        }
        Ok(None) => (None, false, None),
        Err(error) => {
            tracing::warn!(
                    "[bong][player] failed to load persisted lifespan for `{}` from sqlite {}: {error}; using runtime default",
                    username,
                    persistence.db_path().display()
                );
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
                LoadedKnownTechniques::Loaded(known_techniques.unwrap_or_default())
            }
            Err(error) => {
                tracing::error!(
                    "[bong][player] failed to load persisted known techniques for `{}` from sqlite {}: {error}; blocking known techniques persistence for this session to protect the stored row",
                    username,
                    persistence.db_path().display()
                );
                LoadedKnownTechniques::LoadFailed
            }
        }
    } else {
        LoadedKnownTechniques::NotLoaded
    };
    let ui_prefs = match load_player_ui_prefs_from_sqlite(&connection, username) {
        Ok(ui_prefs) => ui_prefs,
        Err(error) => {
            tracing::warn!(
                "[bong][player] failed to load persisted UI prefs for `{}` from sqlite {}: {error}; using default UI prefs",
                username,
                persistence.db_path().display()
            );
            PlayerUiPrefs::default()
        }
    };

    LoadedPlayerSlices {
        state,
        position,
        last_dimension,
        inventory,
        craft_session,
        lifespan,
        in_coffin,
        coffin_grade,
        skill_set,
        known_techniques,
        ui_prefs,
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
    // grade=None → resolve_coffin_grade_for_persist 回读 DB 既有 grade
    persist_player_slices_in_sqlite(
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
    let mut connection = open_player_connection(persistence)?;
    persist_player_slices_in_sqlite(
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
    if let Some(parent) = persistence.db_path().parent() {
        fs::create_dir_all(parent)?;
    }

    let connection = Connection::open(persistence.db_path()).map_err(io::Error::other)?;
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(io::Error::other)?;
    connection
        .busy_timeout(Duration::from_millis(SQLITE_BUSY_TIMEOUT_MS))
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

    let last_dimension = dimension_kind_from_sql(&dimension_text).unwrap_or_else(|error| {
        tracing::warn!(
            "[bong][player] unknown last_dimension `{dimension_text}` for `{username}`: {error}; defaulting to overworld"
        );
        DimensionKind::default()
    });

    Ok(Some(([pos_x, pos_y, pos_z], last_dimension)))
}

fn sanitize_loaded_position(
    username: &str,
    position: [f64; 3],
    last_dimension: DimensionKind,
) -> ([f64; 3], DimensionKind) {
    let [x, y, z] = position;
    if x.is_finite()
        && y.is_finite()
        && z.is_finite()
        && (MIN_SAFE_PLAYER_Y..=MAX_SAFE_PLAYER_Y).contains(&y)
    {
        return (position, last_dimension);
    }

    tracing::warn!(
        "[bong][player] persisted position for `{username}` is outside safe login bounds \
         ({x:.2}, {y:.2}, {z:.2}, {last_dimension:?}); using spawn defaults"
    );
    (
        crate::player::spawn_position_for_seed(username, SpawnPurpose::InitialLogin),
        DimensionKind::default(),
    )
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

fn load_player_inventory_from_sqlite(
    connection: &Connection,
    username: &str,
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

    // 旧版本（v1）：解析为 Value → 迁移 equipped 形态 → 反序列化。
    let mut value: serde_json::Value = serde_json::from_str(&inventory_json)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    migrate_equipped_v1_to_v2(&mut value);
    serde_json::from_value::<PlayerInventory>(value)
        .map(Some)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
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
fn migrate_equipped_v1_to_v2(value: &mut serde_json::Value) {
    use serde_json::{json, Value};

    let Some(equipped) = value.get_mut("equipped").and_then(Value::as_object_mut) else {
        return;
    };
    let old = std::mem::take(equipped);

    // plan-layered-equip-v1 P4（决议 #8）：旧 treasure_belt_* 件迁入触发位，按 belt 槽序排列。
    let mut triggered: std::collections::BTreeMap<String, Value> =
        std::collections::BTreeMap::new();

    // 累积每个目标身体/手槽的 worn 列表与 held 件。
    let mut new_slots: std::collections::HashMap<String, (Vec<Value>, Option<Value>)> =
        std::collections::HashMap::new();
    // plan-layered-equip-v1 P0.6（决议 #17 / Bug3）— 旧背包专属装备槽（back_pack/waist_pouch/
    // chest_satchel）的背包件迁去 chest.worn 后，其同名静态容器必须随之改名到运行时
    // `pack_<instance_id>` 命名空间，否则 rebuild_containers_from_equipment 会新建空 pack_*、
    // 把装着东西的旧 back_pack 容器留成无主孤儿（伪皮/物品全卡在里面取不出 = 真机症状）。
    // legacy_slot_name → 该槽背包件的 instance_id。
    let mut legacy_pack_container_renames: std::collections::HashMap<String, u64> =
        std::collections::HashMap::new();
    let push_worn = |slots: &mut std::collections::HashMap<String, (Vec<Value>, Option<Value>)>,
                     slot: &str,
                     item: Value| {
        slots.entry(slot.to_string()).or_default().0.push(item);
    };
    let set_held = |slots: &mut std::collections::HashMap<String, (Vec<Value>, Option<Value>)>,
                    slot: &str,
                    item: Value| {
        slots.entry(slot.to_string()).or_default().1 = Some(item);
    };

    for (old_slot, item) in old {
        // 已是 v2 形态（含 worn/held）的件：原样保留（容错幂等）。
        if item.get("worn").is_some() || item.get("held").is_some() {
            let entry = new_slots.entry(old_slot.clone()).or_default();
            if let Some(worn) = item.get("worn").and_then(Value::as_array) {
                entry.0.extend(worn.iter().cloned());
            }
            if let Some(held) = item.get("held") {
                if !held.is_null() {
                    entry.1 = Some(held.clone());
                }
            }
            continue;
        }
        match old_slot.as_str() {
            "false_skin" => push_worn(&mut new_slots, "chest", item),
            "two_hand" => set_held(&mut new_slots, "main_hand", item),
            "treasure_belt_0" | "treasure_belt_1" | "treasure_belt_2" | "treasure_belt_3" => {
                // 法宝激活态归触发位（决议 #8）——不进装备槽 worn，按 belt 槽序收集到 triggered。
                triggered.insert(old_slot.clone(), item);
            }
            "back_pack" | "waist_pouch" | "chest_satchel" => {
                // 记下背包件 instance_id，下面把同名旧容器改名到 pack_<instance_id>。
                if let Some(instance_id) = item.get("instance_id").and_then(Value::as_u64) {
                    legacy_pack_container_renames.insert(old_slot.clone(), instance_id);
                }
                push_worn(&mut new_slots, "chest", item)
            }
            "head" | "chest" | "legs" | "feet" => push_worn(&mut new_slots, &old_slot, item),
            "main_hand" | "off_hand" | "extra_hand_0" | "extra_hand_1" => {
                set_held(&mut new_slots, &old_slot, item)
            }
            // 未知旧槽：默认按 worn 落到原槽名（容错）。
            other => push_worn(&mut new_slots, other, item),
        }
    }

    let rebuilt = value
        .get_mut("equipped")
        .and_then(Value::as_object_mut)
        .expect("equipped object present");
    for (slot, (worn, held)) in new_slots {
        rebuilt.insert(
            slot,
            json!({ "worn": worn, "held": held.unwrap_or(Value::Null) }),
        );
    }

    // 旧背包专属容器改名到 pack_<instance_id>（决议 #17 / Bug3）。
    // 旧档静态容器 id 与旧装备槽同名（back_pack/waist_pouch/chest_satchel，见 #736 前 default.toml）；
    // 改名后容器随穿戴背包件进入 pack_<id> 命名空间，与 rebuild_containers_from_equipment 一致，
    // 装在里面的物品不再丢失。
    if !legacy_pack_container_renames.is_empty() {
        if let Some(containers) = value.get_mut("containers").and_then(Value::as_array_mut) {
            for container in containers.iter_mut() {
                let Some(container_id) = container
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                else {
                    continue;
                };
                if let Some(&instance_id) = legacy_pack_container_renames.get(&container_id) {
                    if let Some(obj) = container.as_object_mut() {
                        obj.insert(
                            "id".to_string(),
                            Value::String(crate::inventory::container_id_for_worn_pack(
                                instance_id,
                            )),
                        );
                    }
                }
            }
        }
    }

    // plan-layered-equip-v1 P4（决议 #8）：旧 treasure_belt_* 件迁入顶层 triggered_treasures。
    // BTreeMap 按 belt_0..3 槽名升序迭代，保持原 belt 顺序；超出触发位容量的多余件丢弃。
    if !triggered.is_empty() {
        let trigger_items: Vec<Value> = triggered
            .into_values()
            .take(crate::inventory::TREASURE_TRIGGER_CAP)
            .collect();
        if let Some(obj) = value.as_object_mut() {
            obj.insert(
                "triggered_treasures".to_string(),
                Value::Array(trigger_items),
            );
        }
    }
}

fn load_player_ui_prefs_from_sqlite(
    connection: &Connection,
    username: &str,
) -> io::Result<PlayerUiPrefs> {
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
        return Ok(PlayerUiPrefs::default());
    };

    serde_json::from_str::<PlayerUiPrefs>(&prefs_json)
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
        persist_player_slices_in_sqlite(
            connection,
            username,
            state,
            crate::player::spawn_position_for_seed(username, SpawnPurpose::InitialLogin),
            DimensionKind::default(),
            None,
            None,
            &SkillSet::default(),
            None,
            None,
            None,
        )?;
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
    // None = 回读 DB 既有 grade（无棺上下文保存路径，防止洗掉 Jade/Stone/Bronze）
    coffin_grade: Option<CoffinGrade>,
    // None = 调用方无 craft 上下文，保留数据库原值；Some(None) = 删除 session。
    craft_session: Option<Option<&CraftSession>>,
) -> io::Result<()> {
    let normalized = state.normalized();
    let karma = normalized.karma;
    let inventory_score = normalized.inventory_score;
    let [pos_x, pos_y, pos_z] = position;
    let inventory_json = serialize_inventory_json(inventory)?;
    let skill_set_json = serialize_skill_set_json(skill_set)?;
    let known_techniques_json = serialize_known_techniques_json(&KnownTechniques::default())?;
    let last_updated_wall = current_unix_seconds();
    let prefs_json = default_ui_prefs_json()?;
    let craft_session_json = craft_session
        .flatten()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let in_coffin_value = resolve_in_coffin_for_persist(connection, username, in_coffin)?;
    let coffin_grade_value = resolve_coffin_grade_for_persist(connection, username, coffin_grade)?;

    let transaction = connection.transaction().map_err(io::Error::other)?;
    let current_char_id: Option<String> = transaction
        .query_row(
            "SELECT current_char_id FROM player_core WHERE username = ?1",
            params![username],
            |row| row.get(0),
        )
        .optional()
        .map_err(io::Error::other)?;
    let current_char_id = current_char_id.unwrap_or_else(|| Uuid::now_v7().to_string());

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
                username,
                skill_set_json,
                PLAYER_ROW_SCHEMA_VERSION,
                last_updated_wall
            ],
        )
        .map_err(io::Error::other)?;
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
    if let Some(lifespan) = lifespan {
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
    if craft_session.is_some() {
        persist_player_craft_session_in_transaction(
            &transaction,
            username,
            craft_session_json.as_deref(),
            last_updated_wall,
        )?;
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
