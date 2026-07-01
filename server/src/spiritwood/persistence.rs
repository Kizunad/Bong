//! F27 — `SpiritWoodHarvestedLogs` 持久化。
//!
//! 现状（修复前）：`SpiritWoodHarvestedLogs` 是纯内存 `HashSet<(DimensionKind,
//! [i32; 3])>`，没有 Serialize / 落盘。重启后所有已砍伐的巨树 log 位置全部
//! "恢复"，`is_spiritwood_log_target` 重新判定为可采，等同灵木原木（
//! `ling_mu_gun` 的唯一来源）无限刷——参照 `mineral::persistence::ExhaustedMineralsLog`
//! 的 hydrate/dirty/节流 flush 模式补上落盘。
//!
//! 落盘格式（沿用 #797 `craft::unlock::RecipeUnlockState` 的最佳实践：const 版本号
//! 常量 + `.tmp` 临时文件 + `rename` 原子落盘）：
//! ```json
//! {
//!   "version": 1,
//!   "entries": [
//!     { "dimension": "overworld", "x": 128, "y": 80, "z": 256, "tick": 12345 }
//!   ]
//! }
//! ```
//!
//! 重启后由 `spiritwood::register` 路径调 `SpiritWoodHarvestedLogs::hydrated` 恢复，
//! 已砍伐位置继续判定为不可采。

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use valence::prelude::{BlockPos, ChunkPos, ResMut, Resource};

use crate::world::dimension::DimensionKind;

const DEFAULT_HARVESTED_PATH: &str = "data/spiritwood/harvested.json";

/// 落盘 schema 版本 — writer（[`SpiritWoodHarvestedLogs::flush`]）与 loader
/// （[`load_harvested_log`]）必须共用同一常量，避免两处字面量漂移
/// （#797 `RECIPE_UNLOCK_VERSION` 的教训）。
const SPIRITWOOD_HARVESTED_VERSION: u32 = 1;

/// 单条已采伐记录。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct HarvestedLogEntry {
    pub dimension: DimensionKind,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub tick: u64,
}

/// 落盘格式 wrapper — 留 `version` 字段方便后续 schema 演进。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HarvestedLogFile {
    pub version: u32,
    pub entries: Vec<HarvestedLogEntry>,
}

impl Default for HarvestedLogFile {
    fn default() -> Self {
        Self {
            version: SPIRITWOOD_HARVESTED_VERSION,
            entries: Vec::new(),
        }
    }
}

/// 已采伐灵木 log 位置 → 采伐 tick 的映射 + 节流刷盘 — 在 `spiritwood::register`
/// 时插入到 ECS resource。
///
/// 用单一 `HashMap` 而不是"位置 HashSet + tick HashMap"两份平行集合 —— 位置本身
/// 就是 key，tick 是 value，没有理由拆成两个容器各自维护一致性。
#[derive(Debug)]
pub struct SpiritWoodHarvestedLogs {
    entries: HashMap<(DimensionKind, [i32; 3]), u64>,
    /// 自上次 flush 以来是否有未落盘的变更。
    dirty: bool,
    /// 距上次 flush 累计 tick 数，用于节流。
    flush_clock: u32,
    /// 节流窗口（tick）。默认 600 = 30 秒 @ 20 tps。
    flush_interval_ticks: u32,
    /// 落盘路径；test override 用 `with_path`。
    file_path: PathBuf,
}

impl Resource for SpiritWoodHarvestedLogs {}

impl Default for SpiritWoodHarvestedLogs {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            dirty: false,
            flush_clock: 0,
            flush_interval_ticks: 600,
            file_path: PathBuf::from(DEFAULT_HARVESTED_PATH),
        }
    }
}

impl SpiritWoodHarvestedLogs {
    pub fn with_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.file_path = path.into();
        self
    }

    pub fn with_flush_interval(mut self, ticks: u32) -> Self {
        self.flush_interval_ticks = ticks;
        self
    }

    /// 是否有未落盘的变更（测试 / flush 系统用）。
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// 已记录的采伐位置数量（hydrate 启动日志 / 测试用）。
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn contains(&self, dimension: DimensionKind, pos: BlockPos) -> bool {
        self.entries.contains_key(&position_key(dimension, pos))
    }

    /// 标记该位置已采伐并记录发生的 tick。返回 true = 实际新增（首次采伐），
    /// false = 该位置已记录过（noop，tick 不会被覆盖，不重复标脏）。
    pub fn mark_harvested(&mut self, dimension: DimensionKind, pos: BlockPos, tick: u64) -> bool {
        let key = position_key(dimension, pos);
        if self.entries.contains_key(&key) {
            return false;
        }
        self.entries.insert(key, tick);
        self.dirty = true;
        true
    }

    pub fn positions_in_chunk(&self, dimension: DimensionKind, chunk: ChunkPos) -> Vec<BlockPos> {
        self.entries
            .keys()
            .filter_map(|(stored_dimension, [x, y, z])| {
                (*stored_dimension == dimension
                    && x.div_euclid(16) == chunk.x
                    && z.div_euclid(16) == chunk.z)
                    .then_some(BlockPos::new(*x, *y, *z))
            })
            .collect()
    }

    /// 强制刷盘 — 测试 / 关服 hook 用。
    ///
    /// 原子落盘：先写同目录 `.tmp` 临时文件，成功后 `rename` 到最终路径
    /// （照抄 #797 `RecipeUnlockState::flush` 的写法，避免写入中途失败把
    /// `file_path` 留成截断 JSON）。
    pub fn flush(&mut self) -> Result<(), String> {
        if !self.dirty {
            return Ok(());
        }
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("create dir {} failed: {e}", parent.display()))?;
        }
        let entries = self
            .entries
            .iter()
            .map(|((dimension, [x, y, z]), tick)| HarvestedLogEntry {
                dimension: *dimension,
                x: *x,
                y: *y,
                z: *z,
                tick: *tick,
            })
            .collect();
        let file = HarvestedLogFile {
            version: SPIRITWOOD_HARVESTED_VERSION,
            entries,
        };
        let json = serde_json::to_string_pretty(&file)
            .map_err(|e| format!("serialize spiritwood harvested log failed: {e}"))?;
        let tmp_path = self.file_path.with_extension("tmp");
        fs::write(&tmp_path, json)
            .map_err(|e| format!("write {} failed: {e}", tmp_path.display()))?;
        fs::rename(&tmp_path, &self.file_path).map_err(|e| {
            format!(
                "rename {} to {} failed: {e}",
                tmp_path.display(),
                self.file_path.display()
            )
        })?;
        self.dirty = false;
        self.flush_clock = 0;
        Ok(())
    }

    /// 启动期 hydrator — 从 `path` 读回磁盘 log，还原 in-memory state。
    ///
    /// - 文件不存在：等价 `default()`，静默（首次启动常态）。
    /// - 文件存在但解析失败：warn + 启动一份空 log（避免 corrupt 文件阻塞启动）。
    /// - 成功：`entries` 预填；`dirty=false` 防止启动立即重写文件。
    pub fn hydrated_from_path(path: impl Into<PathBuf>) -> Self {
        let path: PathBuf = path.into();
        let mut log = Self::default().with_path(path.clone());
        if !path.exists() {
            return log;
        }
        match load_harvested_log(&path) {
            Ok(file) => {
                for entry in file.entries {
                    let key =
                        position_key(entry.dimension, BlockPos::new(entry.x, entry.y, entry.z));
                    log.entries.insert(key, entry.tick);
                }
                log.dirty = false;
            }
            Err(err) => {
                tracing::warn!(
                    target: "bong::spiritwood",
                    "failed to load spiritwood harvested log at {}: {err} — starting fresh",
                    path.display()
                );
            }
        }
        log
    }

    /// 默认路径（`data/spiritwood/harvested.json`）hydrator — `register` 启动路径用。
    pub fn hydrated() -> Self {
        Self::hydrated_from_path(DEFAULT_HARVESTED_PATH)
    }
}

fn position_key(dimension: DimensionKind, pos: BlockPos) -> (DimensionKind, [i32; 3]) {
    (dimension, [pos.x, pos.y, pos.z])
}

/// 启动期 / 测试用 — 读取磁盘 log 重建 in-memory state。
///
/// 拒绝 `version` 与 [`SPIRITWOOD_HARVESTED_VERSION`] 不一致的文件，与
/// `mineral::persistence::load_exhausted_log` / `craft::unlock::load_recipe_unlock_log`
/// 的落盘约定一致——版本漂移当错误处理，不静默接受。
pub fn load_harvested_log(path: impl AsRef<Path>) -> Result<HarvestedLogFile, String> {
    let path = path.as_ref();
    let raw =
        fs::read_to_string(path).map_err(|e| format!("read {} failed: {e}", path.display()))?;
    let file: HarvestedLogFile =
        serde_json::from_str(&raw).map_err(|e| format!("parse {} failed: {e}", path.display()))?;
    if file.version != SPIRITWOOD_HARVESTED_VERSION {
        return Err(format!(
            "unsupported spiritwood harvested log version {} at {} (expected {SPIRITWOOD_HARVESTED_VERSION})",
            file.version,
            path.display()
        ));
    }
    Ok(file)
}

/// system — 按节流窗口把 dirty 的 `SpiritWoodHarvestedLogs` 刷盘。
/// 挂进 `Update`；`mark_harvested` 是直接函数调用（非事件驱动），
/// 所以本系统只负责节流计时 + flush，不读取任何 EventReader
/// （照抄 `craft::unlock::tick_recipe_unlock_flush`）。
pub fn tick_spiritwood_harvested_flush(mut logs: ResMut<SpiritWoodHarvestedLogs>) {
    logs.flush_clock = logs.flush_clock.saturating_add(1);
    if logs.flush_clock >= logs.flush_interval_ticks && logs.dirty {
        // 无论 flush 成功与否都先清零计时器，避免磁盘/权限故障时每帧重试 + 刷爆日志。
        logs.flush_clock = 0;
        if let Err(error) = logs.flush() {
            tracing::warn!(
                target: "bong::spiritwood",
                "spiritwood harvested log flush failed: {error}"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use valence::prelude::{App, Update};

    fn unique_tmp_path(name: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        env::temp_dir().join(format!("bong-spiritwood-harvested-{stamp}-{name}.json"))
    }

    // ── happy path ──────────────────────────────────────────────

    #[test]
    fn mark_flush_hydrate_roundtrip_is_consistent() {
        let path = unique_tmp_path("roundtrip");
        let mut log = SpiritWoodHarvestedLogs::default().with_path(&path);
        assert!(log.mark_harvested(DimensionKind::Overworld, BlockPos::new(17, 80, -1), 555));
        log.flush().expect("flush should succeed");

        let restored = SpiritWoodHarvestedLogs::hydrated_from_path(&path);
        assert!(
            restored.contains(DimensionKind::Overworld, BlockPos::new(17, 80, -1)),
            "expected the flushed position to survive the flush/hydrate roundtrip"
        );
        assert_eq!(restored.len(), 1);
        assert!(
            !restored.is_dirty(),
            "hydrated state must not be dirty on startup"
        );

        let _ = fs::remove_file(&path);
    }

    // ── empty state ─────────────────────────────────────────────

    #[test]
    fn new_state_is_not_dirty_and_empty() {
        let log = SpiritWoodHarvestedLogs::default();
        assert!(!log.is_dirty());
        assert!(log.is_empty());
        assert_eq!(log.len(), 0);
    }

    #[test]
    fn flush_no_op_when_clean() {
        let path = unique_tmp_path("flush_clean");
        let mut log = SpiritWoodHarvestedLogs::default().with_path(&path);
        log.flush().expect("clean flush ok");
        assert!(!path.exists(), "clean flush should not create a file");
    }

    #[test]
    fn hydrated_from_missing_path_returns_empty_log() {
        let path = unique_tmp_path("hydrate_missing");
        assert!(!path.exists());
        let log = SpiritWoodHarvestedLogs::hydrated_from_path(&path);
        assert!(log.is_empty());
        assert!(!log.is_dirty(), "fresh startup log must not be dirty");
    }

    // ── multiple entries ────────────────────────────────────────

    #[test]
    fn multiple_entries_across_dimensions_round_trip_independently() {
        let path = unique_tmp_path("multi_entries");
        let mut log = SpiritWoodHarvestedLogs::default().with_path(&path);
        log.mark_harvested(DimensionKind::Overworld, BlockPos::new(0, 80, 0), 1);
        log.mark_harvested(DimensionKind::Overworld, BlockPos::new(16, 80, 0), 2);
        log.mark_harvested(DimensionKind::Tsy, BlockPos::new(0, 80, 0), 3);
        log.flush().expect("flush should succeed");

        let restored = SpiritWoodHarvestedLogs::hydrated_from_path(&path);
        assert_eq!(
            restored.len(),
            3,
            "expected all 3 entries to survive roundtrip"
        );
        assert!(restored.contains(DimensionKind::Overworld, BlockPos::new(0, 80, 0)));
        assert!(restored.contains(DimensionKind::Overworld, BlockPos::new(16, 80, 0)));
        assert!(restored.contains(DimensionKind::Tsy, BlockPos::new(0, 80, 0)));
        // Cross-dimension isolation: same XZ at Overworld vs Tsy must not collide.
        assert!(
            !restored.contains(DimensionKind::Tsy, BlockPos::new(16, 80, 0)),
            "Tsy dimension must not inherit Overworld-only harvested positions"
        );

        let _ = fs::remove_file(&path);
    }

    // ── version field / schema pin ──────────────────────────────

    #[test]
    fn flush_writes_expected_version_field() {
        let path = unique_tmp_path("version_field");
        let mut log = SpiritWoodHarvestedLogs::default().with_path(&path);
        log.mark_harvested(DimensionKind::Overworld, BlockPos::new(1, 2, 3), 9);
        log.flush().expect("flush should succeed");

        let loaded = load_harvested_log(&path).expect("load should parse");
        assert_eq!(loaded.version, SPIRITWOOD_HARVESTED_VERSION);
        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(loaded.entries[0].dimension, DimensionKind::Overworld);
        assert_eq!(loaded.entries[0].x, 1);
        assert_eq!(loaded.entries[0].y, 2);
        assert_eq!(loaded.entries[0].z, 3);
        assert_eq!(loaded.entries[0].tick, 9);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn load_harvested_log_rejects_unsupported_version() {
        let path = unique_tmp_path("version_unsupported");
        fs::write(
            &path,
            r#"{"version":999,"entries":[{"dimension":"overworld","x":0,"y":0,"z":0,"tick":0}]}"#,
        )
        .unwrap();
        let result = load_harvested_log(&path);
        assert!(
            result.is_err(),
            "expected Err for version=999 (!= {SPIRITWOOD_HARVESTED_VERSION}), actual={result:?}"
        );
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn load_harvested_log_rejects_missing_version_field() {
        let path = unique_tmp_path("version_missing_field");
        fs::write(&path, r#"{"entries":[]}"#).unwrap();
        let result = load_harvested_log(&path);
        assert!(
            result.is_err(),
            "expected Err when `version` field is absent, actual={result:?}"
        );
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn load_harvested_log_rejects_invalid_json() {
        let path = unique_tmp_path("invalid_json");
        fs::write(&path, "not valid json").unwrap();
        assert!(load_harvested_log(&path).is_err());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn hydrated_falls_back_when_file_corrupt() {
        let path = unique_tmp_path("hydrate_corrupt");
        fs::write(&path, "corrupted json {{{").unwrap();
        let log = SpiritWoodHarvestedLogs::hydrated_from_path(&path);
        assert!(log.is_empty());
        assert!(!log.is_dirty());
        let _ = fs::remove_file(&path);
    }

    // ── dirty tracking / state transitions ──────────────────────

    #[test]
    fn mark_harvested_marks_dirty_only_when_actually_new() {
        let mut log = SpiritWoodHarvestedLogs::default();
        assert!(!log.is_dirty());
        assert!(
            log.mark_harvested(DimensionKind::Overworld, BlockPos::new(0, 64, 0), 10),
            "first mark of a new position must report true (actually inserted)"
        );
        assert!(
            log.is_dirty(),
            "first mark of a new position must dirty the log"
        );

        log.dirty = false; // simulate "already flushed"
        assert!(
            !log.mark_harvested(DimensionKind::Overworld, BlockPos::new(0, 64, 0), 99),
            "re-marking an already-harvested position must report false (noop)"
        );
        assert!(
            !log.is_dirty(),
            "re-marking an already-harvested position must not spuriously re-dirty the log"
        );
    }

    #[test]
    fn re_marking_does_not_overwrite_original_tick() {
        let mut log = SpiritWoodHarvestedLogs::default();
        log.mark_harvested(DimensionKind::Overworld, BlockPos::new(0, 64, 0), 10);
        log.mark_harvested(DimensionKind::Overworld, BlockPos::new(0, 64, 0), 999);
        assert_eq!(
            log.entries.get(&(DimensionKind::Overworld, [0, 64, 0])),
            Some(&10),
            "the first-recorded tick must win; re-marking must not overwrite it"
        );
    }

    #[test]
    fn flush_clears_dirty_flag_on_success() {
        let path = unique_tmp_path("flush_clears_dirty");
        let mut log = SpiritWoodHarvestedLogs::default().with_path(&path);
        log.mark_harvested(DimensionKind::Overworld, BlockPos::new(0, 64, 0), 1);
        assert!(log.is_dirty());
        log.flush().expect("flush should succeed");
        assert!(!log.is_dirty(), "successful flush must clear dirty flag");
        let _ = fs::remove_file(&path);
    }

    // ── atomic write safety ──────────────────────────────────────

    #[test]
    fn flush_does_not_corrupt_existing_file_when_tmp_write_fails() {
        let path = unique_tmp_path("flush_fail_atomic");
        let mut log = SpiritWoodHarvestedLogs::default().with_path(&path);
        log.mark_harvested(DimensionKind::Overworld, BlockPos::new(0, 64, 0), 1);
        log.flush().expect("first flush should succeed");
        let original_bytes = fs::read_to_string(&path).expect("file must exist after first flush");

        let tmp_path = path.with_extension("tmp");
        fs::create_dir_all(&tmp_path).expect("setup: create blocking dir at tmp path");

        log.mark_harvested(DimensionKind::Overworld, BlockPos::new(1, 64, 1), 2);
        let result = log.flush();
        assert!(
            result.is_err(),
            "expected flush to fail: tmp path occupied by a directory"
        );

        let bytes_after_failed_flush =
            fs::read_to_string(&path).expect("original file must still exist after failed flush");
        assert_eq!(
            bytes_after_failed_flush, original_bytes,
            "a failed flush must never touch the final path — only a successful \
             write+rename pair may replace it"
        );
        assert!(log.is_dirty(), "a failed flush must leave dirty=true");

        let _ = fs::remove_dir_all(&tmp_path);
        let _ = fs::remove_file(&path);
    }

    // ── throttled flush system ───────────────────────────────────

    #[test]
    fn tick_flush_system_only_writes_after_interval_when_dirty() {
        let path = unique_tmp_path("tick_flush_interval");
        let mut app = App::new();
        let log = SpiritWoodHarvestedLogs::default()
            .with_path(&path)
            .with_flush_interval(3);
        app.insert_resource(log);
        app.add_systems(Update, tick_spiritwood_harvested_flush);

        app.update();
        app.update();
        assert!(
            !path.exists(),
            "flush system must not write while state is clean"
        );

        app.world_mut()
            .resource_mut::<SpiritWoodHarvestedLogs>()
            .mark_harvested(DimensionKind::Overworld, BlockPos::new(0, 64, 0), 5);

        for _ in 0..5 {
            app.update();
            if path.exists() {
                break;
            }
        }
        assert!(
            path.exists(),
            "flush system must eventually persist dirty state once flush_interval_ticks elapses"
        );

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn tick_flush_system_off_by_one_boundary() {
        let path = unique_tmp_path("tick_flush_off_by_one");
        let mut app = App::new();
        let log = SpiritWoodHarvestedLogs::default()
            .with_path(&path)
            .with_flush_interval(5);
        app.insert_resource(log);
        app.add_systems(Update, tick_spiritwood_harvested_flush);

        app.world_mut()
            .resource_mut::<SpiritWoodHarvestedLogs>()
            .mark_harvested(DimensionKind::Overworld, BlockPos::new(0, 64, 0), 5);

        for i in 1..=4 {
            app.update();
            assert!(
                !path.exists(),
                "expected no flush at tick {i} of 5 (< flush_interval_ticks)"
            );
        }
        app.update();
        assert!(
            path.exists(),
            "expected a flush exactly at tick 5 (== flush_interval_ticks)"
        );

        let _ = fs::remove_file(&path);
    }

    // ── positions_in_chunk / contains parity (unchanged public API) ──

    #[test]
    fn positions_in_chunk_and_contains_are_unaffected_by_persistence_fields() {
        let mut log = SpiritWoodHarvestedLogs::default();
        log.mark_harvested(DimensionKind::Overworld, BlockPos::new(17, 80, -1), 0);
        assert!(log.contains(DimensionKind::Overworld, BlockPos::new(17, 80, -1)));
        assert_eq!(
            log.positions_in_chunk(DimensionKind::Overworld, ChunkPos::new(1, -1)),
            vec![BlockPos::new(17, 80, -1)]
        );
        assert!(!log.contains(DimensionKind::Tsy, BlockPos::new(17, 80, -1)));
    }
}
