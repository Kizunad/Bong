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
use crate::cultivation::components::{Cultivation, Realm};
use crate::cultivation::known_techniques::KnownTechniques;
use crate::cultivation::lifespan::{
    lifespan_delta_years_for_real_seconds, LifespanComponent, LIFESPAN_OFFLINE_MULTIPLIER,
};
use crate::inventory::PlayerInventory;
use crate::persistence::{DEFAULT_DATABASE_PATH, SQLITE_BUSY_TIMEOUT_MS};
use crate::player::spawn_selector::SpawnPurpose;
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
const PLAYER_ROW_SCHEMA_VERSION: i32 = 2;
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
    pub quick_slots: [Option<String>; 9],
    #[serde(default)]
    pub skill_bar: [SkillSlotPersist; 9],
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

    pub(crate) fn skill_bar_bindings(
        &self,
        inventory: Option<&PlayerInventory>,
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
                SkillSlotPersist::Skill { skill_id } => SkillSlot::Skill {
                    skill_id: skill_id.clone(),
                },
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
    pub lifespan: Option<LifespanComponent>,
    pub in_coffin: bool,
    /// 棺材档级：Some(grade) = 在棺内 + 档级；None = 不在棺内（与 in_coffin=false 语义对齐）
    pub coffin_grade: Option<CoffinGrade>,
    pub skill_set: SkillSet,
    pub known_techniques: KnownTechniques,
    pub(crate) ui_prefs: PlayerUiPrefs,
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

    pub fn server_payload_with_social_and_local_pressure(
        &self,
        cultivation: &Cultivation,
        player: Option<String>,
        zone: impl Into<String>,
        social: Option<PlayerSocialSnapshotV1>,
        local_neg_pressure: Option<f32>,
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
                lifespan: None,
                in_coffin: false,
                coffin_grade: None,
                skill_set: SkillSet::default(),
                known_techniques: KnownTechniques::default(),
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
    let known_techniques = match load_player_known_techniques_from_sqlite(&connection, username) {
        Ok(known_techniques) => known_techniques,
        Err(error) => {
            tracing::warn!(
                "[bong][player] failed to load persisted known techniques for `{}` from sqlite {}: {error}; using default known techniques",
                username,
                persistence.db_path().display()
            );
            KnownTechniques::default()
        }
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
        known_techniques: loaded.known_techniques,
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

fn open_player_connection(persistence: &PlayerStatePersistence) -> io::Result<Connection> {
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
    // equipped 所有身体槽 worn 层里的件 instance_id 集合（held 件不派生容器，无需收集）。
    let worn_instance_ids: HashSet<u64> = inventory
        .equipped
        .values()
        .flat_map(|slot| slot.worn.iter())
        .map(|item| item.instance_id)
        .collect();

    inventory.containers.iter().any(|container| {
        match crate::inventory::worn_pack_instance_from_container_id(&container.id) {
            // `pack_<id>` 容器但 equipped 无对应 worn 背包件 ⇒ 孤儿派生容器 ⇒ #736 污染指纹。
            Some(instance_id) => !worn_instance_ids.contains(&instance_id),
            None => false,
        }
    })
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
        let inventory = serde_json::from_str::<PlayerInventory>(&inventory_json)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

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

fn load_player_known_techniques_from_sqlite(
    connection: &Connection,
    username: &str,
) -> io::Result<KnownTechniques> {
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
        return Ok(KnownTechniques::default());
    };

    serde_json::from_str::<KnownTechniques>(&known_techniques_json)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
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
    transaction.commit().map_err(io::Error::other)
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
mod player_state_tests {
    use super::*;
    use crate::combat::components::TICKS_PER_SECOND;
    use crate::cultivation::lifespan::LifespanCapTable;
    use crate::inventory::{
        move_equipped_item_to_first_container_slot, set_item_instance_durability, ContainerState,
        InventoryRevision, ItemInstance, ItemRarity, PlayerInventory, EQUIP_SLOT_MAIN_HAND,
        MAIN_PACK_CONTAINER_ID,
    };
    use crate::network::agent_bridge::serialize_server_data_payload;
    use crate::persistence::bootstrap_sqlite;
    use crate::schema::server_data::{ServerDataPayloadV1, SERVER_DATA_VERSION};
    use rusqlite::{params, Connection};
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::{Arc, Barrier};
    use std::time::{SystemTime, UNIX_EPOCH};
    use uuid::Uuid;

    fn unique_temp_dir(test_name: &str) -> PathBuf {
        let unique_suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "bong-player-state-{test_name}-{}-{unique_suffix}",
            std::process::id()
        ))
    }

    fn approx_eq(left: f64, right: f64) {
        assert!(
            (left - right).abs() < 1e-9,
            "expected {left} to be approximately equal to {right}"
        );
    }

    fn sqlite_persistence(test_name: &str) -> (PlayerStatePersistence, PathBuf) {
        let data_dir = unique_temp_dir(test_name);
        let db_path = data_dir.join("bong.db");
        bootstrap_sqlite(&db_path, &format!("player-state-{test_name}"))
            .expect("sqlite bootstrap should succeed");
        (
            PlayerStatePersistence::with_db_path(&data_dir, &db_path),
            data_dir,
        )
    }

    fn iron_sword_instance(instance_id: u64, durability: f64) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: "iron_sword".to_string(),
            display_name: "Iron Sword".to_string(),
            grid_w: 1,
            grid_h: 2,
            weight: 1.2,
            rarity: ItemRarity::Common,
            description: "weapon persistence fixture".to_string(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }
    }

    fn empty_weapon_inventory() -> PlayerInventory {
        PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: InventoryRevision(41),
            containers: vec![ContainerState {
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: "Main Pack".to_string(),
                rows: 5,
                cols: 7,
                items: Vec::new(),
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 17,
            max_weight: 45.0,
        }
    }

    fn equipped_iron_sword_inventory(durability: f64) -> PlayerInventory {
        let mut inventory = empty_weapon_inventory();
        inventory.equipped.insert(
            EQUIP_SLOT_MAIN_HAND.to_string(),
            crate::inventory::SlotContents::held_single(iron_sword_instance(9_001, durability)),
        );
        inventory
    }

    /// 构造一个 v1 形态的 inventory JSON（每装备槽单件 object），仅含 equipped 段供 migrate 测试。
    fn v1_inventory_json_with_equipped(equipped: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "revision": 1,
            "containers": [],
            "equipped": equipped,
            "hotbar": [null, null, null, null, null, null, null, null, null],
            "bone_coins": 0,
            "max_weight": 50.0
        })
    }

    fn v1_treasure_item(instance_id: u64, template: &str) -> serde_json::Value {
        serde_json::json!({
            "instance_id": instance_id,
            "template_id": template,
            "display_name": template,
            "grid_w": 1, "grid_h": 1, "weight": 0.2,
            "rarity": "Uncommon", "description": "", "stack_count": 1,
            "spirit_quality": 0.5, "durability": 1.0
        })
    }

    // plan-layered-equip-v1 P4（决议 #8）— 旧 treasure_belt_* 槽迁入触发位 triggered_treasures，
    // 按 belt 槽序排列；不进装备槽 worn。
    #[test]
    fn migrate_v1_treasure_belt_lands_in_trigger_slots_in_order() {
        let mut value = v1_inventory_json_with_equipped(serde_json::json!({
            "treasure_belt_0": v1_treasure_item(10, "talisman_a"),
            "treasure_belt_2": v1_treasure_item(12, "talisman_c"),
            "treasure_belt_1": v1_treasure_item(11, "talisman_b"),
        }));

        migrate_equipped_v1_to_v2(&mut value);

        // 触发位顺序应按 belt_0,belt_1,belt_2（BTreeMap 槽名升序）。
        let triggered = value
            .get("triggered_treasures")
            .and_then(|v| v.as_array())
            .expect("triggered_treasures array present after migration");
        let ids: Vec<u64> = triggered
            .iter()
            .map(|item| item.get("instance_id").and_then(|v| v.as_u64()).unwrap())
            .collect();
        assert_eq!(
            ids,
            vec![10, 11, 12],
            "treasure_belt_0/1/2 should map to trigger slots in belt order"
        );

        // 装备结构里不应留 treasure_belt 槽（也不应进 worn）。
        let equipped = value.get("equipped").and_then(|v| v.as_object()).unwrap();
        assert!(
            !equipped.contains_key("treasure_belt_0")
                && !equipped.contains_key("treasure_belt_1")
                && !equipped.contains_key("treasure_belt_2"),
            "no treasure_belt slot key should survive migration"
        );
    }

    // 反序列化迁移产物为 PlayerInventory，确认 triggered_treasures 真正落进结构。
    #[test]
    fn migrate_v1_treasure_belt_deserializes_into_triggered_treasures_field() {
        let mut value = v1_inventory_json_with_equipped(serde_json::json!({
            "treasure_belt_0": v1_treasure_item(20, "talisman_x"),
        }));
        migrate_equipped_v1_to_v2(&mut value);

        let inventory: PlayerInventory =
            serde_json::from_value(value).expect("migrated v2 json deserializes");
        assert_eq!(inventory.triggered_treasures.len(), 1);
        assert_eq!(inventory.triggered_treasures[0].instance_id, 20);
        assert_eq!(inventory.triggered_treasures[0].template_id, "talisman_x");
    }

    // 无 treasure_belt 的旧档迁移后不应凭空生出 triggered_treasures 字段（serde default 空）。
    #[test]
    fn migrate_v1_without_treasure_belt_leaves_trigger_slot_empty() {
        let mut value = v1_inventory_json_with_equipped(serde_json::json!({
            "main_hand": v1_treasure_item(30, "iron_sword"),
        }));
        migrate_equipped_v1_to_v2(&mut value);
        assert!(
            value.get("triggered_treasures").is_none(),
            "no treasure_belt → migration must not inject triggered_treasures"
        );
        let inventory: PlayerInventory =
            serde_json::from_value(value).expect("deserializes with serde default empty trigger");
        assert!(inventory.triggered_treasures.is_empty());
    }

    /// 构造一个 v1 单件装备 object（带 instance_id），供 equipped 槽 / 容器 item 迁移测试复用。
    fn v1_equip_item(instance_id: u64, template: &str) -> serde_json::Value {
        serde_json::json!({
            "instance_id": instance_id,
            "template_id": template,
            "display_name": template,
            "grid_w": 2, "grid_h": 2, "weight": 0.5,
            "rarity": "Common", "description": "", "stack_count": 1,
            "spirit_quality": 0.5, "durability": 0.5
        })
    }

    // Bug1（真机回归）— 旧 default.toml 形态：chest=fake_spirit_hide、main_hand=iron_sword、
    // back_pack=worn_grass_pouch。迁移后 equipped 必须非空且正确：
    // chest.worn == [worn_grass_pouch, fake_spirit_hide]（栈底背包件、栈顶伪皮，与 fresh 实例化一致），
    // main_hand.held == iron_sword。绝不允许迁空 / 错置 / 把 equipped 件丢进容器。
    #[test]
    fn migrate_v1_legacy_default_loadout_keeps_equipped_correct() {
        let mut value = v1_inventory_json_with_equipped(serde_json::json!({
            "chest": v1_equip_item(1, "fake_spirit_hide"),
            "main_hand": v1_equip_item(2, "iron_sword"),
            "back_pack": v1_equip_item(3, "worn_grass_pouch"),
        }));
        migrate_equipped_v1_to_v2(&mut value);
        let inventory: PlayerInventory =
            serde_json::from_value(value).expect("migrated v2 json deserializes");

        let chest = inventory
            .equipped
            .get(crate::inventory::EQUIP_SLOT_CHEST)
            .expect("chest slot must exist after migration (equipped 不得迁空)");
        let chest_worn: Vec<&str> = chest.worn.iter().map(|i| i.template_id.as_str()).collect();
        assert_eq!(
            chest_worn,
            vec!["worn_grass_pouch", "fake_spirit_hide"],
            "迁移后 chest.worn 应为 [背包件, 伪皮]（栈底→栈顶），与 default.toml fresh 实例化一致；实际 {chest_worn:?}"
        );
        assert!(
            chest.held.is_none(),
            "身体槽 chest 不应有 held 件；实际 {:?}",
            chest.held.as_ref().map(|i| &i.template_id)
        );

        let main_hand = inventory
            .equipped
            .get(crate::inventory::EQUIP_SLOT_MAIN_HAND)
            .expect("main_hand slot must exist after migration");
        assert_eq!(
            main_hand.held.as_ref().map(|i| i.template_id.as_str()),
            Some("iron_sword"),
            "武器应迁入 main_hand.held（而非 worn / 容器）"
        );
        assert!(
            main_hand.worn.is_empty(),
            "手槽 main_hand 不应有 worn 件；实际 {:?}",
            main_hand.worn
        );

        // 不得残留旧背包专属槽 key。
        assert!(
            !inventory.equipped.contains_key("back_pack"),
            "旧 back_pack 装备槽 key 不应在 v2 equipped 中存活"
        );
    }

    // Bug3（真机回归）— 旧档背包件在 back_pack 装备槽，且有同名 `back_pack` 容器装着物品。
    // 迁移后该容器必须改名到 pack_<背包件 instance_id>，否则 rebuild_containers_from_equipment
    // 会新建空 pack_*、把旧 back_pack 容器留成无主孤儿（物品取不出）。
    #[test]
    fn migrate_v1_renames_legacy_backpack_container_to_pack_instance_namespace() {
        let mut value = serde_json::json!({
            "revision": 1,
            "containers": [
                {
                    "id": "body_pocket", "name": "暗袋", "rows": 2, "cols": 3,
                    "items": [{
                        "row": 0, "col": 0,
                        "instance": v1_equip_item(50, "fengling_bone_coin")
                    }]
                },
                {
                    "id": "back_pack", "name": "破草包", "rows": 3, "cols": 3,
                    "items": [{
                        "row": 0, "col": 0,
                        "instance": v1_equip_item(51, "spirit_grass")
                    }]
                }
            ],
            "equipped": {
                "back_pack": v1_equip_item(42, "worn_grass_pouch"),
            },
            "hotbar": [null, null, null, null, null, null, null, null, null],
            "bone_coins": 7,
            "max_weight": 23.0
        });
        migrate_equipped_v1_to_v2(&mut value);
        let inventory: PlayerInventory =
            serde_json::from_value(value).expect("migrated v2 json deserializes");

        // 背包件迁到 chest.worn。
        let chest = inventory
            .equipped
            .get(crate::inventory::EQUIP_SLOT_CHEST)
            .expect("chest slot present");
        assert_eq!(
            chest
                .worn
                .iter()
                .map(|i| i.template_id.as_str())
                .collect::<Vec<_>>(),
            vec!["worn_grass_pouch"],
            "worn_grass_pouch 应迁到 chest.worn"
        );
        let pack_instance_id = chest.worn[0].instance_id;
        assert_eq!(pack_instance_id, 42, "迁移保留原 instance_id");

        // 旧 back_pack 容器应改名到 pack_42，且内含物品原样保留。
        let expected_id = crate::inventory::container_id_for_worn_pack(pack_instance_id);
        let renamed = inventory
            .containers
            .iter()
            .find(|c| c.id == expected_id)
            .unwrap_or_else(|| {
                panic!(
                    "应存在改名后的容器 `{expected_id}`；实际容器 ids = {:?}",
                    inventory
                        .containers
                        .iter()
                        .map(|c| &c.id)
                        .collect::<Vec<_>>()
                )
            });
        assert_eq!(
            renamed.items.len(),
            1,
            "改名后容器内物品必须保留（不丢数据）"
        );
        assert_eq!(renamed.items[0].instance.template_id, "spirit_grass");

        // 旧 back_pack id 不应再存在（已被改名，不留孤儿）。
        assert!(
            !inventory.containers.iter().any(|c| c.id == "back_pack"),
            "旧 back_pack 容器 id 应已改名消失，不留无主孤儿"
        );
        // body_pocket 不动。
        assert!(
            inventory.containers.iter().any(|c| c.id == "body_pocket"),
            "body_pocket 容器应原样保留"
        );
    }

    /// 把任意 inventory_json 以指定 schema_version 落进 sqlite，再走 load_player_inventory_from_sqlite。
    /// 复现真机 join → 加载链路（DEFAULT_INVENTORY_JSON / orphan-pack / 正常 v2 / v1 迁移分流全覆盖）。
    fn load_inventory_row(
        schema_version: i32,
        inventory_json: &str,
    ) -> (Option<PlayerInventory>, PathBuf) {
        let (persistence, data_dir) = sqlite_persistence("load-inventory-row");
        let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
        connection
            .execute(
                "INSERT INTO inventories (username, inventory_json, schema_version, last_updated_wall)
                 VALUES (?1, ?2, ?3, 0)",
                params!["LoadProbe", inventory_json, schema_version],
            )
            .expect("insert inventory row");
        let loaded = load_player_inventory_from_sqlite(&connection, "LoadProbe")
            .expect("load_player_inventory_from_sqlite should not error");
        (loaded, data_dir)
    }

    // Bug A（真机回归）— 真机污染存档：#736 旧迁移 bug 把伪皮冲进 body_pocket、清空 equipped、
    // 丢 iron_sword/worn_grass_pouch，只剩孤儿 pack_<id> 容器，且已落盘为 schema_version=2。
    // 这是 Kizun3Desu 实测行内 JSON（worn_grass_pouch instance_id=11 派生 pack_11，但 equipped 空）。
    // 加载时必须识别为污染、丢弃存档、回落默认 loadout（返回 None），否则玩家 join 后 equipped 永久空。
    #[test]
    fn corrupt_v2_with_orphan_pack_container_is_discarded_to_default_loadout() {
        // 真机 Kizun3Desu v2 污染行的最小忠实复刻：equipped 空 + pack_11 孤儿容器 + body_pocket。
        let corrupt_v2 = serde_json::json!({
            "revision": 8,
            "containers": [
                {
                    "id": "body_pocket", "name": "贴身口袋", "rows": 2, "cols": 3,
                    "items": [
                        { "row": 0, "col": 0, "instance": v1_equip_item(2, "ningmai_powder") },
                        // 伪皮被旧迁移 bug 冲进 body_pocket（真机症状）。
                        { "row": 0, "col": 1, "instance": v1_equip_item(12, "fake_spirit_hide") }
                    ]
                },
                {
                    // 孤儿 pack_11：派生自 worn_grass_pouch(instance_id=11)，但 equipped 里已无该件。
                    "id": "pack_11", "name": "破草包", "rows": 3, "cols": 3,
                    "items": [
                        { "row": 0, "col": 0, "instance": v1_equip_item(4, "spirit_grass") }
                    ]
                }
            ],
            "equipped": {},
            "hotbar": [null, null, null, null, null, null, null, null, null],
            "bone_coins": 7,
            "max_weight": 23.0,
            "triggered_treasures": []
        });
        let (loaded, data_dir) = load_inventory_row(2, &corrupt_v2.to_string());
        assert!(
            loaded.is_none(),
            "孤儿 pack_<id> 容器（equipped 无对应 worn 背包件）= #736 污染指纹，必须丢弃回落默认 loadout（返回 None），\
             否则 attach_player_state 会把空 equipped 存档插上、抑制默认 loadout，玩家 join 后 equipped 永久空；实际 loaded.is_some()={}",
            loaded.is_some()
        );
        let _ = fs::remove_dir_all(&data_dir);
    }

    // Bug A（防误伤）— 健康 v2 存档：equipped 有 chest.worn 背包件 + 与之自洽的 pack_<id> 容器。
    // 这不是污染（容器有 backing worn 件），必须原样保留，绝不能被自愈逻辑误丢。
    #[test]
    fn healthy_v2_with_backed_pack_container_is_preserved() {
        let pack_id = crate::inventory::container_id_for_worn_pack(11);
        let healthy_v2 = serde_json::json!({
            "revision": 3,
            "containers": [
                { "id": "body_pocket", "name": "贴身口袋", "rows": 2, "cols": 3, "items": [] },
                {
                    "id": pack_id, "name": "破草包", "rows": 3, "cols": 3,
                    "items": [ { "row": 0, "col": 0, "instance": v1_equip_item(4, "spirit_grass") } ]
                }
            ],
            // worn_grass_pouch instance_id=11，与 pack_11 自洽 ⇒ 非孤儿。
            "equipped": {
                "chest": { "worn": [ v1_equip_item(11, "worn_grass_pouch") ], "held": null }
            },
            "hotbar": [null, null, null, null, null, null, null, null, null],
            "bone_coins": 7,
            "max_weight": 23.0,
            "triggered_treasures": []
        });
        let (loaded, data_dir) = load_inventory_row(2, &healthy_v2.to_string());
        let inventory = loaded
            .expect("健康 v2 存档（pack_<id> 有 backing worn 件）必须原样保留，不得被自愈误丢");
        let chest = inventory
            .equipped
            .get(crate::inventory::EQUIP_SLOT_CHEST)
            .expect("chest 槽应保留");
        assert_eq!(
            chest.worn.iter().map(|i| i.instance_id).collect::<Vec<_>>(),
            vec![11],
            "chest.worn 背包件 instance_id 应原样保留"
        );
        assert!(
            inventory.containers.iter().any(|c| c.id == pack_id),
            "自洽 pack_<id> 容器应原样保留"
        );
        let _ = fs::remove_dir_all(&data_dir);
    }

    // Bug A（防误伤）— 合法裸装玩家：equipped 空且无任何 pack_<id> 容器（卸背包时容器随之清掉）。
    // 不是污染（没有孤儿容器），必须原样保留空 equipped，不能被误判为 #736 污染而重置。
    #[test]
    fn naked_v2_without_pack_container_is_preserved_not_reset() {
        let naked_v2 = serde_json::json!({
            "revision": 5,
            "containers": [
                { "id": "body_pocket", "name": "贴身口袋", "rows": 2, "cols": 3, "items": [] }
            ],
            "equipped": {},
            "hotbar": [null, null, null, null, null, null, null, null, null],
            "bone_coins": 0,
            "max_weight": 23.0,
            "triggered_treasures": []
        });
        let (loaded, data_dir) = load_inventory_row(2, &naked_v2.to_string());
        let inventory =
            loaded.expect("合法裸装存档（无 pack_<id> 容器）必须保留，不得误判为污染重置");
        assert!(
            inventory.equipped.is_empty(),
            "裸装玩家 equipped 应保持空（保留其存档原貌），实际 {:?}",
            inventory.equipped.keys().collect::<Vec<_>>()
        );
        let _ = fs::remove_dir_all(&data_dir);
    }

    // Bug A（真机回归核心）— 真机 v1 旧档（旧 default.toml 形态：chest=fake_spirit_hide、
    // main_hand=iron_sword、back_pack=worn_grass_pouch + 同名 back_pack 容器装 7 件），
    // 走完整 sqlite 加载链路（schema_version=1 → migrate → 反序列化）。
    // 必须：equipped 非空 + chest.worn==[worn_grass_pouch, fake_spirit_hide] + main_hand.held==iron_sword
    // + back_pack 容器改名到 pack_<worn_grass_pouch instance_id> 且 7 件原样保留 + body_pocket 不动。
    // 这把真机 join 加载路径整条锁死，任何回归（迁空 / 错置 / 丢件 / 孤儿容器）立即撞红。
    #[test]
    fn real_v1_legacy_loadout_loads_with_equipped_populated_via_full_path() {
        let v1_row = serde_json::json!({
            "revision": 1,
            "containers": [
                {
                    "id": "body_pocket", "name": "贴身口袋", "rows": 2, "cols": 3,
                    "items": [
                        { "row": 0, "col": 0, "instance": v1_equip_item(2, "ningmai_powder") },
                        { "row": 0, "col": 1, "instance": v1_equip_item(3, "fengling_bone_coin") }
                    ]
                },
                {
                    "id": "back_pack", "name": "破草包", "rows": 3, "cols": 3,
                    "items": [
                        { "row": 0, "col": 0, "instance": v1_equip_item(4, "spirit_grass") },
                        { "row": 0, "col": 1, "instance": v1_equip_item(5, "ningmai_powder") },
                        { "row": 0, "col": 2, "instance": v1_equip_item(6, "guyuan_pill") },
                        { "row": 1, "col": 0, "instance": v1_equip_item(7, "bone_spike") },
                        { "row": 1, "col": 1, "instance": v1_equip_item(8, "ash_spider_silk") },
                        { "row": 2, "col": 1, "instance": v1_equip_item(9, "ci_she_hao_seed") },
                        { "row": 2, "col": 2, "instance": v1_equip_item(10, "ning_mai_cao_seed") }
                    ]
                }
            ],
            "equipped": {
                "chest": v1_equip_item(11, "fake_spirit_hide"),
                "main_hand": v1_equip_item(12, "iron_sword"),
                "back_pack": v1_equip_item(13, "worn_grass_pouch")
            },
            "hotbar": [null, null, null, null, null, null, null, null, null],
            "bone_coins": 7,
            "max_weight": 23.0
        });
        let (loaded, data_dir) = load_inventory_row(1, &v1_row.to_string());
        let inventory = loaded.expect("v1 旧档加载后 inventory 必须存在（不得迁空、不得误判污染）");

        assert!(
            !inventory.equipped.is_empty(),
            "真机 join 加载后 equipped 绝不能为空（Bug A 核心症状）"
        );
        let chest = inventory
            .equipped
            .get(crate::inventory::EQUIP_SLOT_CHEST)
            .expect("chest 槽必须存在");
        assert_eq!(
            chest
                .worn
                .iter()
                .map(|i| i.template_id.as_str())
                .collect::<Vec<_>>(),
            vec!["worn_grass_pouch", "fake_spirit_hide"],
            "chest.worn 应为 [背包件, 伪皮]（栈底→栈顶），与 default.toml fresh 实例化一致"
        );
        let main_hand = inventory
            .equipped
            .get(crate::inventory::EQUIP_SLOT_MAIN_HAND)
            .expect("main_hand 槽必须存在（iron_sword 不得丢失）");
        assert_eq!(
            main_hand.held.as_ref().map(|i| i.template_id.as_str()),
            Some("iron_sword"),
            "iron_sword 必须迁入 main_hand.held（真机数据丢失尤其严重，必锁死）"
        );

        // back_pack 容器改名到 pack_<worn_grass_pouch instance_id=13>，7 件原样保留。
        let expected_pack_id = crate::inventory::container_id_for_worn_pack(13);
        let pack = inventory
            .containers
            .iter()
            .find(|c| c.id == expected_pack_id)
            .unwrap_or_else(|| {
                panic!(
                    "back_pack 容器应改名到 `{expected_pack_id}`；实际容器 ids = {:?}",
                    inventory
                        .containers
                        .iter()
                        .map(|c| &c.id)
                        .collect::<Vec<_>>()
                )
            });
        assert_eq!(
            pack.items.len(),
            7,
            "改名后 pack 容器内 7 件原样保留（不丢数据）"
        );
        assert!(
            !inventory.containers.iter().any(|c| c.id == "back_pack"),
            "旧 back_pack 容器 id 不应残留（已改名，否则成无主孤儿 = 取不出）"
        );
        assert!(
            inventory.containers.iter().any(|c| c.id == "body_pocket"),
            "body_pocket 容器应原样保留"
        );
        // 关键：加载产物自身不得触发 orphan 判定（自洽，pack_13 有 backing worn 件）。
        assert!(
            !inventory_has_orphan_pack_container(&inventory),
            "v1 迁移产物必须自洽：pack_<id> 容器与 chest.worn 背包件 instance_id 对齐，不得被误判孤儿"
        );
        let _ = fs::remove_dir_all(&data_dir);
    }

    // Bug A（fresh join 路径）— 全新玩家（无 sqlite 行）应回落默认 loadout（instantiate_inventory_from_loadout），
    // equipped 正确填充：chest.worn==[worn_grass_pouch, fake_spirit_hide]、main_hand.held==iron_sword。
    // 这把 default.toml → try_into_loadout → instantiate 的 fresh 实例化结构锁死。
    #[test]
    fn fresh_instantiate_from_default_loadout_populates_equipped() {
        use crate::inventory::{
            instantiate_inventory_from_loadout, load_default_loadout, load_item_registry,
            InventoryInstanceIdAllocator,
        };
        // 真机 fresh join 路径：真实 ItemRegistry（assets/items）+ 真实 default.toml → instantiate。
        let registry = load_item_registry().expect("load item registry from assets/items");
        let loadout = load_default_loadout(&registry).expect("default loadout should load");
        let mut allocator = InventoryInstanceIdAllocator::default();
        let inventory = instantiate_inventory_from_loadout(&loadout, &mut allocator, &registry)
            .expect("instantiate default loadout should succeed");

        assert!(
            !inventory.equipped.is_empty(),
            "fresh 实例化后 equipped 绝不能为空"
        );
        let chest = inventory
            .equipped
            .get(crate::inventory::EQUIP_SLOT_CHEST)
            .expect("chest 槽必须存在");
        assert_eq!(
            chest
                .worn
                .iter()
                .map(|i| i.template_id.as_str())
                .collect::<Vec<_>>(),
            vec!["worn_grass_pouch", "fake_spirit_hide"],
            "fresh chest.worn 应为 [破草包, 伪皮]（两条 [[equip]] slot=chest 聚合到 worn 栈）"
        );
        let main_hand = inventory
            .equipped
            .get(crate::inventory::EQUIP_SLOT_MAIN_HAND)
            .expect("main_hand 槽必须存在");
        assert_eq!(
            main_hand.held.as_ref().map(|i| i.template_id.as_str()),
            Some("iron_sword"),
            "fresh main_hand.held 应为 iron_sword（[[equip]] slot=main_hand）"
        );
        assert!(
            !inventory_has_orphan_pack_container(&inventory),
            "fresh 实例化产物自洽：pack_<id> 与 chest.worn 背包件对齐，不得被误判孤儿"
        );
    }

    fn persisted_inventory_snapshot(
        persistence: &PlayerStatePersistence,
        username: &str,
    ) -> serde_json::Value {
        let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
        let inventory_json: String = connection
            .query_row(
                "SELECT inventory_json FROM inventories WHERE username = ?1",
                params![username],
                |row| row.get(0),
            )
            .expect("persisted inventory row should exist");

        serde_json::from_str(&inventory_json).expect("persisted inventory JSON should decode")
    }

    fn persist_player_with_inventory(
        persistence: &PlayerStatePersistence,
        username: &str,
        inventory: &PlayerInventory,
    ) {
        save_player_slices(
            persistence,
            username,
            &PlayerState::default(),
            [11.0, 70.0, -2.0],
            DimensionKind::default(),
            Some(inventory),
            None,
            &SkillSet::default(),
        )
        .expect("player slices with inventory should persist");
    }

    fn only_container_item(inventory: &PlayerInventory) -> &ItemInstance {
        &inventory.containers[0].items[0].instance
    }

    #[test]
    fn loads_and_saves_player_state_in_sqlite() {
        let (persistence, data_dir) = sqlite_persistence("sqlite-load-save");
        let autosave_interval_ticks = 60 * TICKS_PER_SECOND;

        let persisted = PlayerState {
            karma: 0.2,
            inventory_score: 0.4,
        };

        let save_path = save_player_state(&persistence, "Azure", &persisted)
            .expect("saving PlayerState should succeed");
        let reloaded = load_player_state(&persistence, "Azure");
        let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
        let current_char_id: String = connection
            .query_row(
                "SELECT current_char_id FROM player_core WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .expect("player_core row should exist");
        let (pos_x, pos_y, pos_z): (f64, f64, f64) = connection
            .query_row(
                "SELECT pos_x, pos_y, pos_z FROM player_slow WHERE username = ?1",
                params!["Azure"],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("player_slow row should exist");
        let inventory_json: String = connection
            .query_row(
                "SELECT inventory_json FROM inventories WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .expect("inventories row should exist");
        let prefs_json: String = connection
            .query_row(
                "SELECT prefs_json FROM player_ui_prefs WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .expect("player_ui_prefs row should exist");
        let prefs: PlayerUiPrefs =
            serde_json::from_str(&prefs_json).expect("prefs_json should decode");
        let current_char_uuid =
            Uuid::parse_str(&current_char_id).expect("current_char_id should be a UUID");
        let [spawn_x, spawn_y, spawn_z] =
            crate::player::spawn_position_for_seed("Azure", SpawnPurpose::InitialLogin);

        assert_eq!(save_path, persistence.db_path().to_path_buf());
        assert_eq!(reloaded, persisted.normalized());
        assert_eq!(autosave_interval_ticks, 1_200);
        assert_eq!(current_char_uuid.get_version_num(), 7);
        assert_eq!((pos_x, pos_y, pos_z), (spawn_x, spawn_y, spawn_z));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&inventory_json)
                .expect("inventory_json should decode"),
            serde_json::Value::Null
        );
        assert_eq!(prefs, PlayerUiPrefs::default());

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn invalid_persisted_login_y_falls_back_to_spawn() {
        let (persistence, data_dir) = sqlite_persistence("invalid-login-y");
        save_player_state(&persistence, "Azure", &PlayerState::default())
            .expect("saving PlayerState should succeed");
        save_player_slow_slice(
            &persistence,
            "Azure",
            [42.0, -26_297.0, -3.5],
            DimensionKind::default(),
        )
        .expect("saving invalid slow slice should succeed");

        let loaded = load_player_slices(&persistence, "Azure");

        assert_eq!(
            loaded.position,
            crate::player::spawn_position_for_seed("Azure", SpawnPurpose::InitialLogin)
        );
        assert_eq!(loaded.last_dimension, DimensionKind::default());

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn persisted_login_y_above_runtime_world_falls_back_to_spawn() {
        let (persistence, data_dir) = sqlite_persistence("too-high-login-y");
        save_player_state(&persistence, "Azure", &PlayerState::default())
            .expect("saving PlayerState should succeed");
        save_player_slow_slice(
            &persistence,
            "Azure",
            [42.0, MAX_SAFE_PLAYER_Y + 1.0, -3.5],
            DimensionKind::default(),
        )
        .expect("saving too-high slow slice should succeed");

        let loaded = load_player_slices(&persistence, "Azure");

        assert_eq!(
            loaded.position,
            crate::player::spawn_position_for_seed("Azure", SpawnPurpose::InitialLogin)
        );
        assert_eq!(loaded.last_dimension, DimensionKind::default());

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn player_export_bundle_roundtrips_back_into_sqlite() {
        let (source_persistence, source_data_dir) = sqlite_persistence("export-bundle-source");
        let exported_state = PlayerState {
            karma: 0.25,
            inventory_score: 0.7,
        };
        save_player_slices(
            &source_persistence,
            "Azure",
            &exported_state,
            [64.0, 80.0, -12.0],
            DimensionKind::Tsy,
            None,
            None,
            &SkillSet::default(),
        )
        .expect("source player slices should persist");

        let bundle = export_player_bundle(&source_persistence, "Azure")
            .expect("player export bundle should load");

        let (target_persistence, target_data_dir) = sqlite_persistence("export-bundle-target");
        import_player_bundle(&target_persistence, &bundle)
            .expect("player export bundle should import");

        let connection =
            Connection::open(target_persistence.db_path()).expect("sqlite db should open");
        let current_char_id: String = connection
            .query_row(
                "SELECT current_char_id FROM player_core WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .expect("player_core row should exist after import");
        let (karma, inventory_score): (f64, f64) = connection
            .query_row(
                "
                SELECT karma, inventory_score
                FROM player_core
                WHERE username = ?1
                ",
                params!["Azure"],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("player_core payload should exist after import");
        let (pos_x, pos_y, pos_z, last_dimension_text): (f64, f64, f64, String) = connection
            .query_row(
                "SELECT pos_x, pos_y, pos_z, last_dimension FROM player_slow WHERE username = ?1",
                params!["Azure"],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("player_slow row should exist after import");
        let inventory_json: String = connection
            .query_row(
                "SELECT inventory_json FROM inventories WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .expect("inventories row should exist after import");
        let prefs_json: String = connection
            .query_row(
                "SELECT prefs_json FROM player_ui_prefs WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .expect("player_ui_prefs row should exist after import");

        assert_eq!(bundle.kind, "player_export_v1");
        assert_eq!(current_char_id, bundle.current_char_id);
        assert_eq!(karma, 0.25);
        assert_eq!(inventory_score, 0.7);
        assert_eq!((pos_x, pos_y, pos_z), (64.0, 80.0, -12.0));
        assert_eq!(last_dimension_text, "tsy");
        assert_eq!(bundle.last_dimension, DimensionKind::Tsy);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&inventory_json)
                .expect("inventory_json should decode"),
            serde_json::Value::Null
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&prefs_json)
                .expect("prefs_json should decode"),
            bundle.ui_prefs
        );

        let _ = fs::remove_dir_all(&source_data_dir);
        let _ = fs::remove_dir_all(&target_data_dir);
    }

    #[test]
    fn player_lifespan_slice_roundtrips_with_offline_pause_wall() {
        let (persistence, data_dir) = sqlite_persistence("lifespan-roundtrip");
        let player_state = PlayerState::default();
        let lifespan = LifespanComponent {
            born_at_tick: 144,
            years_lived: 12.5,
            cap_by_realm: LifespanCapTable::CONDENSE,
            offline_pause_tick: Some(120),
        };

        save_player_slices(
            &persistence,
            "Azure",
            &player_state,
            [11.0, 70.0, -2.0],
            DimensionKind::default(),
            None,
            Some(&lifespan),
            &SkillSet::default(),
        )
        .expect("lifespan slice should persist with player slices");

        let loaded = load_player_slices(&persistence, "Azure");
        let loaded_lifespan = loaded.lifespan.expect("lifespan should reload");
        let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
        let offline_pause_wall: i64 = connection
            .query_row(
                "SELECT offline_pause_wall FROM player_lifespan WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .expect("player_lifespan row should exist");

        assert_eq!(loaded_lifespan.born_at_tick, lifespan.born_at_tick);
        assert_eq!(loaded_lifespan.cap_by_realm, lifespan.cap_by_realm);
        assert!(loaded_lifespan.years_lived >= lifespan.years_lived);
        assert!(loaded_lifespan.years_lived < lifespan.years_lived + 0.01);
        assert!(offline_pause_wall > 0);

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn player_known_techniques_slice_roundtrips_dash_proficiency() {
        let (persistence, data_dir) = sqlite_persistence("known-techniques-roundtrip");
        let known_techniques = KnownTechniques {
            entries: vec![crate::cultivation::known_techniques::KnownTechnique {
                id: "movement.dash".to_string(),
                proficiency: 0.42,
                active: true,
            }],
        };

        save_player_state(&persistence, "Azure", &PlayerState::default())
            .expect("baseline player state should persist");
        save_player_known_techniques_slice(&persistence, "Azure", &known_techniques)
            .expect("known techniques slice should persist");

        let loaded = load_player_slices(&persistence, "Azure");
        let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
        let known_techniques_json: String = connection
            .query_row(
                "SELECT known_techniques_json FROM player_known_techniques WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .expect("player_known_techniques row should exist");
        let snapshot: serde_json::Value = serde_json::from_str(&known_techniques_json)
            .expect("known techniques JSON should decode");

        assert_eq!(loaded.known_techniques, known_techniques);
        assert_eq!(
            snapshot
                .pointer("/entries/0/id")
                .and_then(serde_json::Value::as_str),
            Some("movement.dash")
        );
        let proficiency = snapshot
            .pointer("/entries/0/proficiency")
            .and_then(serde_json::Value::as_f64)
            .expect("dash proficiency should persist");
        assert!((proficiency - 0.42).abs() < 1e-6);

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn player_lifespan_load_applies_offline_delta_from_pause_wall() {
        let (persistence, data_dir) = sqlite_persistence("lifespan-offline-delta");
        save_player_state(&persistence, "Azure", &PlayerState::default())
            .expect("baseline player state should persist");

        let offline_pause_wall = current_unix_seconds()
            - (crate::cultivation::lifespan::LIFESPAN_SECONDS_PER_YEAR as i64 * 10);
        let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
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
                    schema_version,
                    last_updated_wall
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                ",
                params![
                    "Azure",
                    0_u64,
                    6.0_f64,
                    LifespanCapTable::AWAKEN,
                    offline_pause_wall,
                    0_i64,
                    PLAYER_ROW_SCHEMA_VERSION,
                    offline_pause_wall,
                ],
            )
            .expect("lifespan fixture should insert");

        let loaded = load_player_slices(&persistence, "Azure");
        let loaded_lifespan = loaded.lifespan.expect("lifespan should reload");

        assert!(
            (6.99..=7.01).contains(&loaded_lifespan.years_lived),
            "expected ten offline real hours at x0.1 to add about one year, got {}",
            loaded_lifespan.years_lived
        );

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn player_lifespan_load_applies_coffin_offline_multiplier() {
        let (persistence, data_dir) = sqlite_persistence("lifespan-coffin-offline-delta");
        save_player_state(&persistence, "Azure", &PlayerState::default())
            .expect("baseline player state should persist");

        let offline_pause_wall = current_unix_seconds()
            - (crate::cultivation::lifespan::LIFESPAN_SECONDS_PER_YEAR as i64 * 10);
        let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
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
                    schema_version,
                    last_updated_wall
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                ",
                params![
                    "Azure",
                    0_u64,
                    6.0_f64,
                    LifespanCapTable::AWAKEN,
                    offline_pause_wall,
                    1_i64,
                    PLAYER_ROW_SCHEMA_VERSION,
                    offline_pause_wall,
                ],
            )
            .expect("lifespan fixture should insert");

        let loaded = load_player_slices(&persistence, "Azure");
        let loaded_lifespan = loaded.lifespan.expect("lifespan should reload");

        assert!(loaded.in_coffin);
        // in_coffin=true 且无 coffin_grade → 默认凡木档 0.09
        assert_eq!(
            loaded.coffin_grade,
            Some(CoffinGrade::Mundane),
            "in_coffin=true + no explicit grade should load as Some(Mundane)"
        );
        assert!(
            (offline_lifespan_multiplier(Some(CoffinGrade::Mundane)) - 0.09).abs() < 1e-9,
            "mundane offline multiplier should be 0.09, got {}",
            offline_lifespan_multiplier(Some(CoffinGrade::Mundane))
        );
        assert!(
            (6.89..=6.91).contains(&loaded_lifespan.years_lived),
            "expected ten offline real hours in coffin at x0.09 to add about 0.9 years, got {}",
            loaded_lifespan.years_lived
        );

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn offline_lifespan_multiplier_all_grades() {
        // 四档离线倍率 = OFFLINE(0.1) × lifespan_factor
        assert!(
            (offline_lifespan_multiplier(None) - 0.1).abs() < 1e-9,
            "None → 0.1"
        );
        assert!(
            (offline_lifespan_multiplier(Some(CoffinGrade::Mundane)) - 0.09).abs() < 1e-9,
            "Mundane → 0.09"
        );
        assert!(
            (offline_lifespan_multiplier(Some(CoffinGrade::Jade)) - 0.07).abs() < 1e-9,
            "Jade → 0.07"
        );
        assert!(
            (offline_lifespan_multiplier(Some(CoffinGrade::Stone)) - 0.05).abs() < 1e-9,
            "Stone → 0.05"
        );
        assert!(
            (offline_lifespan_multiplier(Some(CoffinGrade::Bronze)) - 0.03).abs() < 1e-9,
            "Bronze → 0.03"
        );
    }

    #[test]
    fn player_lifespan_load_treats_zero_pause_wall_as_no_offline_delta() {
        let (persistence, data_dir) = sqlite_persistence("lifespan-zero-pause-wall");
        save_player_state(&persistence, "Azure", &PlayerState::default())
            .expect("baseline player state should persist");

        let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
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
                    schema_version,
                    last_updated_wall
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                ",
                params![
                    "Azure",
                    0_u64,
                    12.0_f64,
                    LifespanCapTable::AWAKEN,
                    0_i64,
                    0_i64,
                    PLAYER_ROW_SCHEMA_VERSION,
                    0_i64,
                ],
            )
            .expect("legacy zero-pause lifespan fixture should insert");

        let loaded = load_player_slices(&persistence, "Azure");
        let loaded_lifespan = loaded.lifespan.expect("lifespan should reload");

        assert_eq!(loaded_lifespan.years_lived, 12.0);

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn equipped_weapon_persists_across_player_reload() {
        let (persistence, data_dir) = sqlite_persistence("equipped-weapon-reload");
        let inventory = equipped_iron_sword_inventory(0.87);

        persist_player_with_inventory(&persistence, "Azure", &inventory);

        let loaded = load_player_slices(&persistence, "Azure");
        let loaded_inventory = loaded.inventory.expect("inventory should reload");
        let main_hand_slot = loaded_inventory
            .equipped
            .get(EQUIP_SLOT_MAIN_HAND)
            .expect("main_hand iron_sword should reload from sqlite");
        let main_hand = main_hand_slot
            .held
            .as_ref()
            .expect("main_hand slot should have held iron_sword");
        let snapshot = persisted_inventory_snapshot(&persistence, "Azure");

        assert_eq!(main_hand.instance_id, 9_001);
        assert_eq!(main_hand.template_id, "iron_sword");
        approx_eq(main_hand.durability, 0.87);
        assert_eq!(
            snapshot
                .pointer("/equipped/main_hand/held/template_id")
                .and_then(serde_json::Value::as_str),
            Some("iron_sword")
        );
        println!(
            "weapon_persistence_snapshot equipped={}",
            serde_json::to_string(&snapshot).expect("snapshot should serialize")
        );

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn unequipped_weapon_persists_empty_main_hand_across_reload() {
        let (persistence, data_dir) = sqlite_persistence("unequipped-weapon-reload");
        let mut inventory = equipped_iron_sword_inventory(0.62);
        move_equipped_item_to_first_container_slot(&mut inventory, 9_001)
            .expect("equipped sword should move back into the main pack");

        persist_player_with_inventory(&persistence, "Azure", &inventory);

        let loaded = load_player_slices(&persistence, "Azure");
        let loaded_inventory = loaded.inventory.expect("inventory should reload");
        let packed_sword = only_container_item(&loaded_inventory);
        let snapshot = persisted_inventory_snapshot(&persistence, "Azure");

        assert!(!loaded_inventory.equipped.contains_key(EQUIP_SLOT_MAIN_HAND));
        assert_eq!(packed_sword.instance_id, 9_001);
        assert_eq!(packed_sword.template_id, "iron_sword");
        approx_eq(packed_sword.durability, 0.62);
        assert!(snapshot.pointer("/equipped/main_hand").is_none());
        assert_eq!(
            snapshot
                .pointer("/containers/0/items/0/instance/template_id")
                .and_then(serde_json::Value::as_str),
            Some("iron_sword")
        );
        println!(
            "weapon_persistence_snapshot unequipped={}",
            serde_json::to_string(&snapshot).expect("snapshot should serialize")
        );

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn broken_weapon_state_persists_after_inventory_slice_flush() {
        let (persistence, data_dir) = sqlite_persistence("broken-weapon-reload");
        let mut inventory = equipped_iron_sword_inventory(1.0);
        persist_player_with_inventory(&persistence, "Azure", &inventory);

        set_item_instance_durability(&mut inventory, 9_001, 0.0)
            .expect("weapon durability should update to broken");
        move_equipped_item_to_first_container_slot(&mut inventory, 9_001)
            .expect("broken weapon should move back into the main pack");
        save_player_inventory_slice(&persistence, "Azure", Some(&inventory))
            .expect("changed inventory slice should persist");

        let loaded = load_player_slices(&persistence, "Azure");
        let loaded_inventory = loaded.inventory.expect("inventory should reload");
        let broken_sword = only_container_item(&loaded_inventory);
        let snapshot = persisted_inventory_snapshot(&persistence, "Azure");

        assert!(!loaded_inventory.equipped.contains_key(EQUIP_SLOT_MAIN_HAND));
        assert_eq!(broken_sword.instance_id, 9_001);
        assert_eq!(broken_sword.template_id, "iron_sword");
        approx_eq(broken_sword.durability, 0.0);
        assert_eq!(
            snapshot
                .pointer("/containers/0/items/0/instance/durability")
                .and_then(serde_json::Value::as_f64),
            Some(0.0)
        );
        println!(
            "weapon_persistence_snapshot broken={}",
            serde_json::to_string(&snapshot).expect("snapshot should serialize")
        );

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn ui_prefs_accepts_legacy_payload_without_skill_bar() {
        let prefs: PlayerUiPrefs = serde_json::from_value(serde_json::json!({
            "quick_slots": ["tea", null, null, null, null, null, null, null, null]
        }))
        .expect("legacy prefs should decode with default skill_bar");

        assert_eq!(prefs.quick_slots[0], Some("tea".to_string()));
        assert!(prefs.skill_configs.is_empty());
        assert!(prefs
            .skill_bar
            .iter()
            .all(|slot| matches!(slot, SkillSlotPersist::Empty)));
    }

    #[test]
    fn ui_prefs_accepts_legacy_payload_without_skill_configs() {
        let prefs: PlayerUiPrefs = serde_json::from_value(serde_json::json!({
            "quick_slots": [null, null, null, null, null, null, null, null, null],
            "skill_bar": [
                {"kind":"empty"},
                {"kind":"empty"},
                {"kind":"empty"},
                {"kind":"empty"},
                {"kind":"empty"},
                {"kind":"empty"},
                {"kind":"empty"},
                {"kind":"empty"},
                {"kind":"empty"}
            ]
        }))
        .expect("legacy prefs should decode without skill_configs");

        assert!(prefs.skill_configs.is_empty());
    }

    #[test]
    fn ui_prefs_rehydrates_quick_and_skill_bindings_from_inventory() {
        let prefs: PlayerUiPrefs = serde_json::from_value(serde_json::json!({
            "quick_slots": ["tea", null, null, null, null, null, null, null, null],
            "skill_bar": [
                {"kind":"skill","skill_id":"burst_meridian.beng_quan"},
                {"kind":"item","template_id":"tea"},
                {"kind":"item","template_id":"missing"},
                {"kind":"empty"},
                {"kind":"empty"},
                {"kind":"empty"},
                {"kind":"empty"},
                {"kind":"empty"},
                {"kind":"empty"}
            ]
        }))
        .expect("prefs should decode");
        let inventory = PlayerInventory {
            triggered_treasures: Vec::new(),
            revision: crate::inventory::InventoryRevision(0),
            containers: vec![crate::inventory::ContainerState {
                id: "main".to_string(),
                name: "main".to_string(),
                rows: 5,
                cols: 7,
                items: vec![crate::inventory::PlacedItemState {
                    row: 0,
                    col: 0,
                    instance: crate::inventory::ItemInstance {
                        instance_id: 42,
                        template_id: "tea".to_string(),
                        display_name: "tea".to_string(),
                        grid_w: 1,
                        grid_h: 1,
                        weight: 0.1,
                        rarity: crate::inventory::ItemRarity::Common,
                        description: String::new(),
                        stack_count: 1,
                        spirit_quality: 1.0,
                        durability: 1.0,
                        freshness: None,
                        mineral_id: None,
                        charges: None,
                        forge_quality: None,
                        forge_color: None,
                        forge_side_effects: Vec::new(),
                        forge_achieved_tier: None,
                        alchemy: None,
                        lingering_owner_qi: None,
                    },
                }],
            }],
            equipped: Default::default(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
        };

        let quick = prefs.quick_slot_bindings(Some(&inventory));
        let skill_bar = prefs.skill_bar_bindings(Some(&inventory));

        assert_eq!(quick.slots[0], Some(42));
        assert!(matches!(
            &skill_bar.slots[0],
            SkillSlot::Skill { skill_id } if skill_id == "burst_meridian.beng_quan"
        ));
        assert_eq!(skill_bar.slots[1], SkillSlot::Item { instance_id: 42 });
        assert_eq!(skill_bar.slots[2], SkillSlot::Empty);
    }

    #[test]
    fn import_player_bundle_rejects_invalid_current_char_id() {
        let (persistence, data_dir) = sqlite_persistence("import-invalid-char-id");
        let bundle = PlayerExportBundle {
            kind: "player_export_v1".to_string(),
            username: "Azure".to_string(),
            current_char_id: "not-a-uuid".to_string(),
            state: PlayerState {
                karma: 0.25,
                inventory_score: 0.7,
            },
            position: [64.0, 80.0, -12.0],
            last_dimension: DimensionKind::default(),
            inventory: None,
            skill_set: SkillSet::default(),
            known_techniques: KnownTechniques::default(),
            ui_prefs: serde_json::json!({
                "quick_slots": [null, null, null, null, null, null, null, null, null]
            }),
        };

        let error = import_player_bundle(&persistence, &bundle)
            .expect_err("invalid current_char_id should be rejected");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);

        let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
        let player_core_exists: Option<String> = connection
            .query_row(
                "SELECT username FROM player_core WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .optional()
            .expect("player_core query should succeed");
        let player_slow_exists: Option<String> = connection
            .query_row(
                "SELECT username FROM player_slow WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .optional()
            .expect("player_slow query should succeed");
        let inventories_exists: Option<String> = connection
            .query_row(
                "SELECT username FROM inventories WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .optional()
            .expect("inventories query should succeed");
        let prefs_exists: Option<String> = connection
            .query_row(
                "SELECT username FROM player_ui_prefs WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .optional()
            .expect("player_ui_prefs query should succeed");

        assert!(player_core_exists.is_none());
        assert!(player_slow_exists.is_none());
        assert!(inventories_exists.is_none());
        assert!(prefs_exists.is_none());

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn import_player_bundle_rejects_invalid_ui_prefs() {
        let (persistence, data_dir) = sqlite_persistence("import-invalid-ui-prefs");
        let bundle = PlayerExportBundle {
            kind: "player_export_v1".to_string(),
            username: "Azure".to_string(),
            current_char_id: Uuid::now_v7().to_string(),
            state: PlayerState {
                karma: 0.25,
                inventory_score: 0.7,
            },
            position: [64.0, 80.0, -12.0],
            last_dimension: DimensionKind::default(),
            inventory: None,
            skill_set: SkillSet::default(),
            known_techniques: KnownTechniques::default(),
            ui_prefs: serde_json::json!({
                "quick_slots": [0, 1, 2]
            }),
        };

        let error = import_player_bundle(&persistence, &bundle)
            .expect_err("invalid ui_prefs should be rejected");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);

        let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
        let player_core_exists: Option<String> = connection
            .query_row(
                "SELECT username FROM player_core WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .optional()
            .expect("player_core query should succeed");
        let player_slow_exists: Option<String> = connection
            .query_row(
                "SELECT username FROM player_slow WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .optional()
            .expect("player_slow query should succeed");
        let inventories_exists: Option<String> = connection
            .query_row(
                "SELECT username FROM inventories WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .optional()
            .expect("inventories query should succeed");
        let prefs_exists: Option<String> = connection
            .query_row(
                "SELECT username FROM player_ui_prefs WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .optional()
            .expect("player_ui_prefs query should succeed");

        assert!(player_core_exists.is_none());
        assert!(player_slow_exists.is_none());
        assert!(inventories_exists.is_none());
        assert!(prefs_exists.is_none());

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn computes_composite_power() {
        let state = PlayerState {
            karma: 0.25,
            inventory_score: 0.4,
        };

        let cultivation = Cultivation {
            realm: Realm::Induce,
            qi_current: 60.0,
            qi_max: 100.0,
            ..Cultivation::default()
        };

        let breakdown = state.power_breakdown(&cultivation);
        approx_eq(breakdown.combat, 0.39);
        approx_eq(breakdown.wealth, 0.4);
        approx_eq(breakdown.social, 0.4);
        approx_eq(breakdown.karma, 0.25);
        approx_eq(breakdown.territory, 0.325);
        approx_eq(state.composite_power(&cultivation), 0.36225);
    }

    #[test]
    fn serializes_player_state_payload() {
        let state = PlayerState {
            karma: 0.2,
            inventory_score: 0.4,
        };

        let cultivation = Cultivation {
            realm: Realm::Induce,
            qi_current: 78.0,
            // qi_max≠qi_current 且≠100 fallback：锁住 HUD 真元条分母 = 真实 qi_max（非 current、非 100）。
            qi_max: 150.0,
            ..Cultivation::default()
        };

        let payload = state.server_payload_with_social_and_local_pressure(
            &cultivation,
            Some(canonical_player_id("Steve")),
            "blood_valley",
            None,
            None,
        );
        let bytes =
            serialize_server_data_payload(&payload).expect("PlayerState payload should serialize");
        let json: serde_json::Value =
            serde_json::from_slice(&bytes).expect("serialized payload should decode as JSON value");

        assert_eq!(json.get("v"), Some(&serde_json::json!(SERVER_DATA_VERSION)));
        assert_eq!(json.get("type"), Some(&serde_json::json!("player_state")));
        assert_eq!(
            json.get("player"),
            Some(&serde_json::json!("offline:Steve"))
        );
        assert_eq!(json.get("realm"), Some(&serde_json::json!("Induce")));
        assert_eq!(json.get("spirit_qi"), Some(&serde_json::json!(78.0)));
        // P0 HUD fix：下发真元上限，client 才能算正确分母（缺失则回退 max(100,current) 显示恒满）。
        assert_eq!(json.get("spirit_qi_max"), Some(&serde_json::json!(150.0)));
        assert_eq!(json.get("karma"), Some(&serde_json::json!(0.2)));
        assert_eq!(json.get("zone"), Some(&serde_json::json!("blood_valley")));

        match payload.payload {
            ServerDataPayloadV1::PlayerState {
                spirit_qi_max,
                composite_power,
                breakdown,
                ..
            } => {
                approx_eq(spirit_qi_max, cultivation.qi_max);
                approx_eq(composite_power, state.composite_power(&cultivation));
                approx_eq(breakdown.combat, state.power_breakdown(&cultivation).combat);
                approx_eq(breakdown.wealth, state.power_breakdown(&cultivation).wealth);
                approx_eq(breakdown.social, state.power_breakdown(&cultivation).social);
                approx_eq(breakdown.karma, state.power_breakdown(&cultivation).karma);
                approx_eq(
                    breakdown.territory,
                    state.power_breakdown(&cultivation).territory,
                );
            }
            other => panic!("expected PlayerState payload, got {other:?}"),
        }
    }

    #[test]
    fn migrate_legacy_player_json_to_sqlite_once() {
        let (persistence, data_dir) = sqlite_persistence("legacy-migrate");

        #[derive(serde::Serialize)]
        struct LegacyPlayerStateV0 {
            realm: String,
            spirit_qi: f64,
            spirit_qi_max: f64,
            karma: f64,
            experience: u64,
            inventory_score: f64,
        }

        let legacy_state = LegacyPlayerStateV0 {
            realm: "Induce".to_string(),
            spirit_qi: 78.0,
            spirit_qi_max: 100.0,
            karma: 0.2,
            experience: 1_200,
            inventory_score: 0.4,
        };
        let expected_state = PlayerState {
            karma: 0.2,
            inventory_score: 0.4,
        };
        let save_path = persistence.path_for_username("CorruptCultivator");
        let migrated_path = persistence.migrated_path_for_username("CorruptCultivator");

        fs::create_dir_all(persistence.data_dir()).expect("test data dir should be creatable");
        fs::write(
            &save_path,
            serde_json::to_vec_pretty(&legacy_state).expect("legacy state should serialize"),
        )
        .expect("legacy PlayerState fixture should be writable");

        let migrated = load_player_state(&persistence, "CorruptCultivator");
        let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
        let first_char_id: String = connection
            .query_row(
                "SELECT current_char_id FROM player_core WHERE username = ?1",
                params!["CorruptCultivator"],
                |row| row.get(0),
            )
            .expect("migrated player_core row should exist");
        let reloaded = load_player_state(&persistence, "CorruptCultivator");
        let second_char_id: String = connection
            .query_row(
                "SELECT current_char_id FROM player_core WHERE username = ?1",
                params!["CorruptCultivator"],
                |row| row.get(0),
            )
            .expect("reloaded player_core row should exist");

        assert_eq!(migrated, expected_state.normalized());
        assert_eq!(reloaded, expected_state.normalized());
        assert!(
            !save_path.exists(),
            "legacy json should be renamed after migration"
        );
        assert!(
            migrated_path.exists(),
            "migrated legacy json should be preserved"
        );
        assert_eq!(first_char_id, second_char_id);
        assert_eq!(
            Uuid::parse_str(&first_char_id)
                .expect("current_char_id should be a UUID")
                .get_version_num(),
            7
        );

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn corrupt_legacy_player_json_falls_back_without_affecting_other_players() {
        let (persistence, data_dir) = sqlite_persistence("corrupt-json-isolation");
        let corrupted_username = "CorruptCultivator";
        let healthy_username = "StableCultivator";
        let corrupted_path = persistence.path_for_username(corrupted_username);
        let corrupted_migrated_path = persistence.migrated_path_for_username(corrupted_username);
        let healthy_state = PlayerState {
            karma: -0.3,
            inventory_score: 0.55,
        };

        save_player_state(&persistence, healthy_username, &healthy_state)
            .expect("healthy player state should persist");

        fs::create_dir_all(persistence.data_dir()).expect("test data dir should be creatable");
        fs::write(&corrupted_path, br#"{"realm":"broken""#)
            .expect("corrupted legacy fixture should be writable");

        let corrupted_loaded = load_player_state(&persistence, corrupted_username);
        let healthy_loaded = load_player_state(&persistence, healthy_username);

        assert_eq!(corrupted_loaded, PlayerState::default());
        assert_eq!(healthy_loaded, healthy_state.normalized());
        assert!(
            corrupted_path.exists(),
            "corrupted legacy json should remain in place after failed migration"
        );
        assert!(
            !corrupted_migrated_path.exists(),
            "corrupted legacy json should not be marked as migrated"
        );

        let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
        let corrupted_row: Option<(f64, f64)> = connection
            .query_row(
                "
                SELECT karma, inventory_score
                FROM player_core
                WHERE username = ?1
                ",
                params![corrupted_username],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .expect("corrupted player_core row query should succeed");
        let healthy_row: (f64, f64) = connection
            .query_row(
                "
                SELECT karma, inventory_score
                FROM player_core
                WHERE username = ?1
                ",
                params![healthy_username],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("healthy player_core row should exist");

        assert_eq!(
            corrupted_row,
            Some((
                PlayerState::default().karma,
                PlayerState::default().inventory_score,
            ))
        );
        assert_eq!(
            healthy_row,
            (
                healthy_state.normalized().karma,
                healthy_state.normalized().inventory_score,
            )
        );

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn concurrent_player_core_slice_writers_serialize_under_sqlite_busy_timeout() {
        let (persistence, data_dir) = sqlite_persistence("core-slice-concurrency");
        let writer_count = 50usize;
        let baseline_state = PlayerState {
            karma: 0.1,
            inventory_score: 0.2,
        };

        for index in 0..writer_count {
            save_player_state(
                &persistence,
                format!("Player{index}").as_str(),
                &baseline_state,
            )
            .expect("baseline player state should persist");
        }

        let persistence = Arc::new(persistence);
        let barrier = Arc::new(Barrier::new(writer_count + 1));
        let handles = (0..writer_count)
            .map(|index| {
                let persistence = Arc::clone(&persistence);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    let username = format!("Player{index}");
                    let updated_state = PlayerState {
                        karma: ((index as f64 / 25.0) - 1.0).clamp(-1.0, 1.0),
                        inventory_score: (index as f64 / writer_count as f64).clamp(0.0, 1.0),
                    };

                    barrier.wait();
                    save_player_core_slice(persistence.as_ref(), username.as_str(), &updated_state)
                })
            })
            .collect::<Vec<_>>();

        barrier.wait();
        let errors = handles
            .into_iter()
            .map(|handle| handle.join().expect("writer thread should not panic"))
            .filter_map(Result::err)
            .map(|error| error.to_string())
            .collect::<Vec<_>>();
        assert!(
            errors.is_empty(),
            "all concurrent player core slice writers should succeed: {errors:?}"
        );

        let connection = Connection::open(persistence.db_path()).expect("sqlite db should open");
        let row_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM player_core", [], |row| row.get(0))
            .expect("player_core row count should be readable");
        assert_eq!(row_count, writer_count as i64);

        for index in 0..writer_count {
            let username = format!("Player{index}");
            let (karma, inventory_score): (f64, f64) = connection
                .query_row(
                    "
                    SELECT karma, inventory_score
                    FROM player_core
                    WHERE username = ?1
                    ",
                    params![username.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("updated player_core row should exist");

            assert_eq!(karma, ((index as f64 / 25.0) - 1.0).clamp(-1.0, 1.0));
            assert_eq!(
                inventory_score,
                (index as f64 / writer_count as f64).clamp(0.0, 1.0)
            );
        }

        let _ = fs::remove_dir_all(&data_dir);
    }

    // ─── plan-coffin-tiers-v1 P0 charge #2/#13 ──────────────────────────
    // save_player_lifespan_slice（无棺上下文）不能把已存的 Jade/Stone/Bronze grade 洗成 Mundane

    #[test]
    fn save_player_lifespan_slice_preserves_jade_grade_on_no_coffin_context_save() {
        let (persistence, data_dir) = sqlite_persistence("lifespan-jade-grade-preserve");
        save_player_state(&persistence, "Azure", &PlayerState::default())
            .expect("baseline player state should persist");

        // 先存一个 Jade 棺玩家
        let lifespan = crate::cultivation::lifespan::LifespanComponent {
            born_at_tick: 0,
            years_lived: 10.0,
            cap_by_realm: 100,
            offline_pause_tick: None,
        };
        save_player_lifespan_slice_with_coffin(
            &persistence,
            "Azure",
            &lifespan,
            Some(CoffinGrade::Jade),
        )
        .expect("save with jade coffin should succeed");

        // 验证 DB 里 grade=jade
        {
            let conn = Connection::open(persistence.db_path()).expect("db should open");
            let grade: String = conn
                .query_row(
                    "SELECT coffin_grade FROM player_lifespan WHERE username = ?1",
                    params!["Azure"],
                    |row| row.get(0),
                )
                .expect("grade row should exist");
            assert_eq!(grade, "jade", "grade should be jade after save_with_coffin");
        }

        // 触发无棺上下文保存（模拟悟道延寿路径）
        save_player_lifespan_slice(&persistence, "Azure", &lifespan)
            .expect("save_player_lifespan_slice should succeed");

        // 验证 grade 没有被洗成 mundane
        {
            let conn = Connection::open(persistence.db_path()).expect("db should open");
            let grade: String = conn
                .query_row(
                    "SELECT coffin_grade FROM player_lifespan WHERE username = ?1",
                    params!["Azure"],
                    |row| row.get(0),
                )
                .expect("grade row should exist after no-coffin save");
            assert_eq!(
                grade, "jade",
                "save_player_lifespan_slice (无棺上下文) 不应把 jade 洗成 mundane，\
                 期望 jade，实际 {grade}"
            );
        }

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn save_player_lifespan_slice_preserves_stone_grade() {
        let (persistence, data_dir) = sqlite_persistence("lifespan-stone-grade-preserve");
        save_player_state(&persistence, "Azure", &PlayerState::default())
            .expect("baseline player state should persist");

        let lifespan = crate::cultivation::lifespan::LifespanComponent {
            born_at_tick: 0,
            years_lived: 5.0,
            cap_by_realm: 100,
            offline_pause_tick: None,
        };
        save_player_lifespan_slice_with_coffin(
            &persistence,
            "Azure",
            &lifespan,
            Some(CoffinGrade::Stone),
        )
        .expect("save with stone coffin should succeed");
        save_player_lifespan_slice(&persistence, "Azure", &lifespan)
            .expect("save without coffin context should succeed");

        let conn = Connection::open(persistence.db_path()).expect("db should open");
        let grade: String = conn
            .query_row(
                "SELECT coffin_grade FROM player_lifespan WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .expect("grade row should exist");
        assert_eq!(
            grade, "stone",
            "save_player_lifespan_slice 不应洗掉 stone grade，期望 stone，实际 {grade}"
        );

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn save_player_lifespan_slice_preserves_bronze_grade() {
        let (persistence, data_dir) = sqlite_persistence("lifespan-bronze-grade-preserve");
        save_player_state(&persistence, "Azure", &PlayerState::default())
            .expect("baseline player state should persist");

        let lifespan = crate::cultivation::lifespan::LifespanComponent {
            born_at_tick: 0,
            years_lived: 5.0,
            cap_by_realm: 100,
            offline_pause_tick: None,
        };
        save_player_lifespan_slice_with_coffin(
            &persistence,
            "Azure",
            &lifespan,
            Some(CoffinGrade::Bronze),
        )
        .expect("save with bronze coffin should succeed");
        save_player_lifespan_slice(&persistence, "Azure", &lifespan)
            .expect("save without coffin context should succeed");

        let conn = Connection::open(persistence.db_path()).expect("db should open");
        let grade: String = conn
            .query_row(
                "SELECT coffin_grade FROM player_lifespan WHERE username = ?1",
                params!["Azure"],
                |row| row.get(0),
            )
            .expect("grade row should exist");
        assert_eq!(
            grade, "bronze",
            "save_player_lifespan_slice 不应洗掉 bronze grade，期望 bronze，实际 {grade}"
        );

        let _ = fs::remove_dir_all(&data_dir);
    }

    // ─── plan-coffin-tiers-v1 P0 charge #6 — 非 mundane DB 全链路 ─────────
    // save_player_lifespan_slice_with_coffin(Jade/Stone/Bronze) → DB → load → offline 回算正确

    #[test]
    fn db_full_chain_jade_coffin_offline_multiplier() {
        let (persistence, data_dir) = sqlite_persistence("db-full-chain-jade");
        save_player_state(&persistence, "Azure", &PlayerState::default())
            .expect("baseline player state should persist");

        // 玩家在 Jade 棺内，离线 10 年等效真实秒
        let offline_seconds = crate::cultivation::lifespan::LIFESPAN_SECONDS_PER_YEAR as i64 * 10;
        let offline_pause_wall = current_unix_seconds() - offline_seconds;

        let conn = Connection::open(persistence.db_path()).expect("db should open");
        conn.execute(
            "INSERT INTO player_lifespan (
                username, born_at_tick, years_lived, cap_by_realm,
                offline_pause_wall, in_coffin, coffin_grade, schema_version, last_updated_wall
             ) VALUES (?1, ?2, ?3, ?4, ?5, 1, 'jade', ?6, ?7)",
            params![
                "Azure",
                0_u64,
                6.0_f64,
                100_u32,
                offline_pause_wall,
                PLAYER_ROW_SCHEMA_VERSION,
                offline_pause_wall
            ],
        )
        .expect("jade lifespan fixture should insert");
        drop(conn);

        let loaded = load_player_slices(&persistence, "Azure");
        let loaded_lifespan = loaded.lifespan.expect("lifespan should reload");

        assert!(loaded.in_coffin, "should be in_coffin");
        assert_eq!(
            loaded.coffin_grade,
            Some(CoffinGrade::Jade),
            "loaded grade should be Some(Jade), got {:?}",
            loaded.coffin_grade
        );
        // jade 倍率 0.07 → 10 年 × 0.07 = 0.7 年
        assert!(
            (offline_lifespan_multiplier(Some(CoffinGrade::Jade)) - 0.07).abs() < 1e-9,
            "jade offline multiplier should be 0.07"
        );
        assert!(
            (6.69..=6.71).contains(&loaded_lifespan.years_lived),
            "expected 10 offline years in jade coffin at x0.07 to add ~0.7 years, \
             started at 6.0, got {}",
            loaded_lifespan.years_lived
        );

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn db_full_chain_stone_coffin_offline_multiplier() {
        let (persistence, data_dir) = sqlite_persistence("db-full-chain-stone");
        save_player_state(&persistence, "Azure", &PlayerState::default())
            .expect("baseline player state should persist");

        let offline_seconds = crate::cultivation::lifespan::LIFESPAN_SECONDS_PER_YEAR as i64 * 10;
        let offline_pause_wall = current_unix_seconds() - offline_seconds;

        let conn = Connection::open(persistence.db_path()).expect("db should open");
        conn.execute(
            "INSERT INTO player_lifespan (
                username, born_at_tick, years_lived, cap_by_realm,
                offline_pause_wall, in_coffin, coffin_grade, schema_version, last_updated_wall
             ) VALUES (?1, ?2, ?3, ?4, ?5, 1, 'stone', ?6, ?7)",
            params![
                "Azure",
                0_u64,
                6.0_f64,
                100_u32,
                offline_pause_wall,
                PLAYER_ROW_SCHEMA_VERSION,
                offline_pause_wall
            ],
        )
        .expect("stone lifespan fixture should insert");
        drop(conn);

        let loaded = load_player_slices(&persistence, "Azure");
        let loaded_lifespan = loaded.lifespan.expect("lifespan should reload");

        assert!(loaded.in_coffin, "should be in_coffin");
        assert_eq!(
            loaded.coffin_grade,
            Some(CoffinGrade::Stone),
            "loaded grade should be Some(Stone), got {:?}",
            loaded.coffin_grade
        );
        // stone 倍率 0.05 → 10 年 × 0.05 = 0.5 年
        assert!(
            (offline_lifespan_multiplier(Some(CoffinGrade::Stone)) - 0.05).abs() < 1e-9,
            "stone offline multiplier should be 0.05"
        );
        assert!(
            (6.49..=6.51).contains(&loaded_lifespan.years_lived),
            "expected 10 offline years in stone coffin at x0.05 to add ~0.5 years, \
             started at 6.0, got {}",
            loaded_lifespan.years_lived
        );

        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn db_full_chain_bronze_coffin_offline_multiplier() {
        let (persistence, data_dir) = sqlite_persistence("db-full-chain-bronze");
        save_player_state(&persistence, "Azure", &PlayerState::default())
            .expect("baseline player state should persist");

        let offline_seconds = crate::cultivation::lifespan::LIFESPAN_SECONDS_PER_YEAR as i64 * 10;
        let offline_pause_wall = current_unix_seconds() - offline_seconds;

        let conn = Connection::open(persistence.db_path()).expect("db should open");
        conn.execute(
            "INSERT INTO player_lifespan (
                username, born_at_tick, years_lived, cap_by_realm,
                offline_pause_wall, in_coffin, coffin_grade, schema_version, last_updated_wall
             ) VALUES (?1, ?2, ?3, ?4, ?5, 1, 'bronze', ?6, ?7)",
            params![
                "Azure",
                0_u64,
                6.0_f64,
                100_u32,
                offline_pause_wall,
                PLAYER_ROW_SCHEMA_VERSION,
                offline_pause_wall
            ],
        )
        .expect("bronze lifespan fixture should insert");
        drop(conn);

        let loaded = load_player_slices(&persistence, "Azure");
        let loaded_lifespan = loaded.lifespan.expect("lifespan should reload");

        assert!(loaded.in_coffin, "should be in_coffin");
        assert_eq!(
            loaded.coffin_grade,
            Some(CoffinGrade::Bronze),
            "loaded grade should be Some(Bronze), got {:?}",
            loaded.coffin_grade
        );
        // bronze 倍率 0.03 → 10 年 × 0.03 = 0.3 年
        assert!(
            (offline_lifespan_multiplier(Some(CoffinGrade::Bronze)) - 0.03).abs() < 1e-9,
            "bronze offline multiplier should be 0.03"
        );
        assert!(
            (6.29..=6.31).contains(&loaded_lifespan.years_lived),
            "expected 10 offline years in bronze coffin at x0.03 to add ~0.3 years, \
             started at 6.0, got {}",
            loaded_lifespan.years_lived
        );

        let _ = fs::remove_dir_all(&data_dir);
    }
}
