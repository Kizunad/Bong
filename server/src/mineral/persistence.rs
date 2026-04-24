//! plan-mineral-v1 §M6 — 矿脉耗尽持久化。
//!
//! `MineralExhaustedEvent` 触发后，把 (mineral_id, BlockPos, exhausted_at_tick) 写入
//! 内存 `ExhaustedMineralsLog`，并按节流（默认每 600 tick = 30 秒 @ 20 tps）刷盘到
//! `data/minerals/exhausted.json`。
//!
//! 落盘格式（plan §2.1 矿脉有限性 / §7 数据契约）：
//! ```json
//! {
//!   "version": 1,
//!   "entries": [
//!     { "mineral_id": "fan_tie", "x": 128, "y": 72, "z": 256, "tick": 12345 },
//!     ...
//!   ]
//! }
//! ```
//!
//! 重启后由 `world::register` 路径调 `load_exhausted_log` 把记录恢复，让
//! worldgen 跳过已耗尽 BlockPos（避免再生 ore 块）。

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, EventReader, Res, ResMut, Resource};

use super::events::MineralExhaustedEvent;

const DEFAULT_EXHAUSTED_PATH: &str = "data/minerals/exhausted.json";

/// 单条耗尽记录。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExhaustedEntry {
    pub mineral_id: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub tick: u64,
}

impl ExhaustedEntry {
    pub fn from_event(event: &MineralExhaustedEvent, tick: u64) -> Self {
        Self {
            mineral_id: event.mineral_id.as_str().to_string(),
            x: event.position.x,
            y: event.position.y,
            z: event.position.z,
            tick,
        }
    }
}

/// 落盘格式 wrapper — 留 `version` 字段方便后续 schema 演进。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExhaustedLogFile {
    pub version: u32,
    pub entries: Vec<ExhaustedEntry>,
}

impl Default for ExhaustedLogFile {
    fn default() -> Self {
        Self {
            version: 1,
            entries: Vec::new(),
        }
    }
}

/// 内存日志 + 节流刷盘 — 在 `register` 时插入到 ECS resource。
#[derive(Debug, Resource)]
pub struct ExhaustedMineralsLog {
    entries: Vec<ExhaustedEntry>,
    dirty: bool,
    /// 距上次 flush 累计 tick 数，用于节流。
    flush_clock: u32,
    /// 节流窗口（tick）。默认 600 = 30 秒 @ 20 tps。
    flush_interval_ticks: u32,
    /// 落盘路径；test override 用 `with_path`。
    file_path: PathBuf,
}

impl Default for ExhaustedMineralsLog {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            dirty: false,
            flush_clock: 0,
            flush_interval_ticks: 600,
            file_path: PathBuf::from(DEFAULT_EXHAUSTED_PATH),
        }
    }
}

impl ExhaustedMineralsLog {
    pub fn with_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.file_path = path.into();
        self
    }

    pub fn with_flush_interval(mut self, ticks: u32) -> Self {
        self.flush_interval_ticks = ticks;
        self
    }

    pub fn entries(&self) -> &[ExhaustedEntry] {
        &self.entries
    }

    pub fn record(&mut self, entry: ExhaustedEntry) {
        self.entries.push(entry);
        self.dirty = true;
    }

    /// 强制刷盘 — 测试 / 关服 hook 用。
    pub fn flush(&mut self) -> Result<(), String> {
        if !self.dirty {
            return Ok(());
        }
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("create dir {} failed: {e}", parent.display()))?;
        }
        let file = ExhaustedLogFile {
            version: 1,
            entries: self.entries.clone(),
        };
        let json = serde_json::to_string_pretty(&file)
            .map_err(|e| format!("serialize exhausted log failed: {e}"))?;
        fs::write(&self.file_path, json)
            .map_err(|e| format!("write {} failed: {e}", self.file_path.display()))?;
        self.dirty = false;
        self.flush_clock = 0;
        Ok(())
    }
}

/// `world::Tick` 资源 stub — 由 world 模块实际驱动；此处为 plan-mineral-v1 M6 自包含
/// 引入的轻量计数器，避免与 world 模块耦合。后续若 world 提供统一 tick resource，
/// 本资源可下沉。
#[derive(Debug, Default, Resource)]
pub struct MineralTickClock {
    pub tick: u64,
}

pub fn tick_mineral_clock(mut clock: ResMut<MineralTickClock>) {
    clock.tick = clock.tick.saturating_add(1);
}

/// system — 把 MineralExhaustedEvent 收入内存 log，按节流刷盘。
pub fn record_exhausted_minerals(
    mut events: EventReader<MineralExhaustedEvent>,
    mut log: ResMut<ExhaustedMineralsLog>,
    clock: Res<MineralTickClock>,
) {
    for event in events.read() {
        log.record(ExhaustedEntry::from_event(event, clock.tick));
    }

    log.flush_clock = log.flush_clock.saturating_add(1);
    if log.flush_clock >= log.flush_interval_ticks && log.dirty {
        if let Err(error) = log.flush() {
            tracing::warn!(
                target: "bong::mineral",
                "exhausted minerals flush failed: {error}"
            );
        }
    }
}

/// 启动期 / 测试用 — 读取磁盘 log 重建 in-memory state。
pub fn load_exhausted_log(path: impl AsRef<Path>) -> Result<ExhaustedLogFile, String> {
    let path = path.as_ref();
    let raw =
        fs::read_to_string(path).map_err(|e| format!("read {} failed: {e}", path.display()))?;
    serde_json::from_str(&raw).map_err(|e| format!("parse {} failed: {e}", path.display()))
}

impl ExhaustedMineralsLog {
    /// 启动期 hydrator — 从 `path` 读回磁盘 log，还原 in-memory entries。
    ///
    /// - 文件不存在：等价 `default()`，静默（首次启动常态）。
    /// - 文件存在但解析失败：warn + 启动一份空 log（避免 corrupt 文件阻塞启动）。
    /// - 成功：entries 预填；`dirty=false` 防止启动立即重写文件。
    pub fn hydrated_from_path(path: impl Into<PathBuf>) -> Self {
        let path: PathBuf = path.into();
        let mut log = Self::default().with_path(path.clone());
        if !path.exists() {
            return log;
        }
        match load_exhausted_log(&path) {
            Ok(file) => {
                log.entries = file.entries;
                log.dirty = false;
            }
            Err(err) => {
                tracing::warn!(
                    target: "bong::mineral",
                    "failed to load exhausted log at {}: {err} — starting fresh",
                    path.display()
                );
            }
        }
        log
    }

    /// 默认路径（`data/minerals/exhausted.json`）hydrator — `register` 启动路径用。
    pub fn hydrated() -> Self {
        Self::hydrated_from_path(DEFAULT_EXHAUSTED_PATH)
    }
}

#[cfg(test)]
mod tests {
    use super::super::types::MineralId;
    use super::*;
    use std::env;
    use valence::prelude::BlockPos;

    fn unique_tmp_path(name: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        env::temp_dir().join(format!("bong-mineral-exhausted-{stamp}-{name}.json"))
    }

    #[test]
    fn entry_from_event_matches_position_and_id() {
        let ev = MineralExhaustedEvent {
            mineral_id: MineralId::SuiTie,
            position: BlockPos::new(10, 64, -5),
        };
        let entry = ExhaustedEntry::from_event(&ev, 999);
        assert_eq!(entry.mineral_id, "sui_tie");
        assert_eq!(entry.x, 10);
        assert_eq!(entry.y, 64);
        assert_eq!(entry.z, -5);
        assert_eq!(entry.tick, 999);
    }

    #[test]
    fn flush_writes_json_and_roundtrips() {
        let path = unique_tmp_path("flush_writes");
        let mut log = ExhaustedMineralsLog::default().with_path(&path);
        log.record(ExhaustedEntry {
            mineral_id: "fan_tie".into(),
            x: 0,
            y: 64,
            z: 0,
            tick: 100,
        });
        log.flush().expect("flush should succeed");

        let loaded = load_exhausted_log(&path).expect("load should parse");
        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(loaded.entries[0].mineral_id, "fan_tie");

        // cleanup
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn flush_no_op_when_clean() {
        let path = unique_tmp_path("flush_clean");
        let mut log = ExhaustedMineralsLog::default().with_path(&path);
        // 没 record 过 → 不应写文件
        log.flush().expect("clean flush ok");
        assert!(!path.exists(), "clean flush should not create file");
    }

    #[test]
    fn record_marks_dirty_and_appends() {
        let mut log = ExhaustedMineralsLog::default();
        assert!(!log.dirty);
        log.record(ExhaustedEntry {
            mineral_id: "ling_shi_yi".into(),
            x: 1,
            y: 2,
            z: 3,
            tick: 5,
        });
        assert!(log.dirty);
        assert_eq!(log.entries().len(), 1);
    }

    #[test]
    fn load_exhausted_log_rejects_invalid_json() {
        let path = unique_tmp_path("invalid_json");
        fs::write(&path, "not valid json").unwrap();
        assert!(load_exhausted_log(&path).is_err());
        let _ = fs::remove_file(&path);
    }

    // plan-mineral-v1 §M6 — startup hydrate path
    #[test]
    fn hydrated_from_missing_path_returns_empty_log() {
        let path = unique_tmp_path("hydrate_missing");
        assert!(!path.exists());
        let log = ExhaustedMineralsLog::hydrated_from_path(&path);
        assert!(log.entries().is_empty());
        assert!(!log.dirty, "fresh startup log must not be dirty");
        assert_eq!(log.file_path, path);
    }

    #[test]
    fn hydrated_restores_entries_from_prior_flush() {
        let path = unique_tmp_path("hydrate_restore");
        // 先 flush 一份到磁盘（模拟上次关服）
        let mut prior = ExhaustedMineralsLog::default().with_path(&path);
        prior.record(ExhaustedEntry {
            mineral_id: MineralId::SuiTie.as_str().to_string(),
            x: 10,
            y: 64,
            z: -5,
            tick: 12345,
        });
        prior.flush().expect("flush should succeed");

        // hydrate 回来
        let restored = ExhaustedMineralsLog::hydrated_from_path(&path);
        assert_eq!(restored.entries().len(), 1);
        assert_eq!(restored.entries()[0].mineral_id, "sui_tie");
        assert_eq!(restored.entries()[0].x, 10);
        assert_eq!(restored.entries()[0].tick, 12345);
        assert!(!restored.dirty, "hydrated log must not be dirty on startup");

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn hydrated_falls_back_when_file_corrupt() {
        let path = unique_tmp_path("hydrate_corrupt");
        fs::write(&path, "corrupted json {{{").unwrap();
        let log = ExhaustedMineralsLog::hydrated_from_path(&path);
        // 坏文件 → 空 log（warn log 已发，启动不阻塞）
        assert!(log.entries().is_empty());
        assert!(!log.dirty);
        let _ = fs::remove_file(&path);
    }
}
