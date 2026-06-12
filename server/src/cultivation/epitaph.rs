//! 生平记录·碑刻系统 — plan-life-record-epitaph-v1 P0
//!
//! 交付物：
//!   * 数据模型：EpitaphId / LifeRecordSummary / FinalThought / EpitaphEntry
//!   * WorldEpitaphRegistry（Bevy Resource，IndexMap 保插入序，超 1000 淘汰最旧）
//!   * EpitaphGenerationSystem：监听 PlayerTerminated → 提取 LifeRecordSummary →
//!     创建 EpitaphEntry → 写 Registry → 持久化 SQLite epitaphs 表
//!
//! §8#1 决策门结论：监听 PlayerTerminated（cultivation/death_hooks.rs:53），
//! 过滤含 LifeRecord 且不含 NpcMarker 的 entity，只为玩家生碑。

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use valence::prelude::{
    bevy_ecs, EventReader, IntoSystemConfigs, Position, Query, Res, ResMut, Resource, Without,
};

use super::components::Realm;
use super::death_hooks::PlayerTerminated;
use super::life_record::{BiographyEntry, LifeRecord, SkillMilestone};
use crate::combat::components::Lifecycle;
use crate::npc::spawn::NpcMarker;
use crate::persistence::PersistenceSettings;
use crate::skill::components::SkillId;

// ─── 数据模型 ────────────────────────────────────────────────────────────────

/// 碑刻唯一 ID（UUID v7 字符串，便于时序排序）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EpitaphId(pub String);

impl EpitaphId {
    /// 用 UUID v7 生成一个新 EpitaphId（仿 persistence::Uuid::now_v7().to_string() 先例）。
    pub fn new() -> Self {
        Self(Uuid::now_v7().to_string())
    }
}

impl Default for EpitaphId {
    fn default() -> Self {
        Self::new()
    }
}

/// 从 LifeRecord + Lifecycle 提取的五字段生平摘要。
///
/// 用于 EpitaphEntry 的核心描述层，不保存完整 biography 链（biography 已在 SQLite
/// 其他表永久保留，碑刻只要「一眼可读的峰值快照」）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifeRecordSummary {
    /// 本角色有生以来达到的最高境界（无 BreakthroughSucceeded 时 fallback Awaken）。
    pub peak_realm: Realm,
    /// 合计 PvP 击杀数（PvpEncounter outcome = "death_fight" + TribulationIntercepted）。
    pub total_kills: u32,
    /// 合计死亡次数（取 Lifecycle.death_count）。
    pub total_deaths: u32,
    /// 最高熟练度的前 N 个 skill（按 SkillMilestone.new_lv 降序取前 3）。
    pub signature_skill_ids: Vec<SkillId>,
    /// 最后一条带 zone 的 biography entry 的 zone 字符串（无则空）。
    pub final_zone: String,
}

/// 遗念（P1 由 PendingFinalThoughtStore 填充，P0 全取 None 占位）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FinalThought {
    /// 无遗念（P0 默认占位）。
    None,
    /// 位置线索（「某处有宝」）。
    LocationHint(String),
    /// 复仇线索（「此人杀我」）。
    RevengeHint(String),
    /// 顿悟残影（「我悟到了……」）。
    InsightHint(String),
}

/// 单条碑刻记录。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpitaphEntry {
    /// 碑刻唯一 ID（UUID v7）。
    pub id: EpitaphId,
    /// 对应角色 ID（与 LifeRecord.character_id 对齐）。
    pub character_id: String,
    /// 玩家用户名（游戏内显示名）。
    pub player_name: String,
    /// 终死亡时的境界。
    pub final_realm: Realm,
    /// 终死亡的 tick（来自 Lifecycle.last_death_tick 或 biography Terminated.tick）。
    pub death_tick: u64,
    /// 碑刻预期放置坐标（P1 EpitaphPlacementSystem 填充；P0 存死亡时的 Position 或 None）。
    pub niche_pos: Option<[i32; 3]>,
    /// 生平摘要。
    pub record_summary: LifeRecordSummary,
    /// 遗念（P0 占位 FinalThought::None）。
    pub final_thought: FinalThought,
}

// ─── WorldEpitaphRegistry ────────────────────────────────────────────────────

/// 全局碑刻注册表（Bevy Resource）。
///
/// 按插入顺序保存最多 `max_cap` 条 EpitaphEntry；超 cap 时 shift_remove_index(0)
/// 淘汰最旧 entry（内存 LRU）。SQLite epitaphs 表永久保留全量，不随内存淘汰删除。
#[derive(Debug, Clone, Resource)]
pub struct WorldEpitaphRegistry {
    /// 按插入序的碑刻条目（IndexMap 保证插入序，便于 O(1) 淘汰最旧）。
    pub entries: IndexMap<EpitaphId, EpitaphEntry>,
    /// 最大内存 cap（超出则淘汰最旧 entry）。
    pub max_cap: usize,
}

impl Default for WorldEpitaphRegistry {
    fn default() -> Self {
        Self {
            entries: IndexMap::new(),
            max_cap: 1000,
        }
    }
}

impl WorldEpitaphRegistry {
    /// 插入一条碑刻；超 max_cap 时淘汰最旧 entry（shift_remove_index(0) O(n)）。
    ///
    /// 注意：淘汰只影响内存 IndexMap，SQLite epitaphs 表**永久保留**该条目。
    pub fn insert(&mut self, entry: EpitaphEntry) {
        // 先插入
        self.entries.insert(entry.id.clone(), entry);
        // 超 cap 时淘汰最旧（插入顺序 index 0）
        while self.entries.len() > self.max_cap {
            self.entries.shift_remove_index(0);
        }
    }
}

// ─── P1 软依赖桩（PendingFinalThoughtStore）────────────────────────────────

/// P0 占位接口——P1 实装时替换为真实 Resource。
///
/// EpitaphGenerationSystem 通过 `Option<ResMut<PendingFinalThoughtStore>>` 软依赖，
/// P0 不注册此 Resource，全部遗念取 FinalThought::None。P1 接入时只需 insert_resource
/// 并填充 HashMap，不改 EpitaphGenerationSystem 签名。
#[derive(Debug, Default, Resource)]
pub struct PendingFinalThoughtStore {
    pending: std::collections::HashMap<String, FinalThought>,
}

impl PendingFinalThoughtStore {
    /// 取出并移除指定 character_id 的遗念（P1 填充路径）。
    pub fn take(&mut self, character_id: &str) -> Option<FinalThought> {
        self.pending.remove(character_id)
    }

    /// P1 测试辅助：插入遗念。
    pub fn insert(&mut self, character_id: impl Into<String>, thought: FinalThought) {
        self.pending.insert(character_id.into(), thought);
    }
}

// ─── LifeRecordSummary 提取函数 ───────────────────────────────────────────────

/// 最多取前 N 个最高级别的技能。
const SIGNATURE_SKILL_TOP_N: usize = 3;

/// PvpEncounter.outcome 中被认定为「本玩家是击杀方」的精确字面值。
///
/// "death_fight" 是 EncounterOutcome::DeathFight 的 wire name（见 pvp_encounter.rs），
/// 表示致死格斗；参与者一方确认为击杀方。其余 outcome（bypass/peaceful_separation/
/// probe_fight/temporary_cooperation/betrayal）均不构成"击杀对手"语义。
const KILL_OUTCOME_EXACT: &str = "death_fight";

/// 从 LifeRecord + Lifecycle.death_count 提取 LifeRecordSummary。
///
/// # 字段语义
/// * `peak_realm`：biography 中 BreakthroughSucceeded{realm} 取最高（按 Realm::rank()），无则 Awaken
/// * `total_kills`：PvpEncounter{outcome} == "death_fight"（致死格斗） + TribulationIntercepted（截劫）
/// * `total_deaths`：传入 lifecycle.death_count（Lifecycle component 最精确）
/// * `signature_skill_ids`：skill_milestones 按 new_lv 降序取前 3
/// * `final_zone`：最后一条带 zone 的 biography entry（SpiritEyeBreakthrough/PvpEncounter）
pub fn summarize(life_record: &LifeRecord, death_count: u32) -> LifeRecordSummary {
    // peak_realm
    let peak_realm = life_record
        .biography
        .iter()
        .filter_map(|entry| {
            if let BiographyEntry::BreakthroughSucceeded { realm, .. } = entry {
                Some(*realm)
            } else {
                None
            }
        })
        .max_by_key(|r| r.rank())
        .unwrap_or(Realm::Awaken);

    // total_kills：PvpEncounter.outcome == "death_fight"（致死格斗）+ TribulationIntercepted（截劫）
    //
    // JueBiKilled 表示本玩家自己在绝壁劫中殒命，属死亡语义而非击杀对手，不计入此处。
    // "death_fight" 是 EncounterOutcome::DeathFight 唯一的 wire name，精确匹配排除伪阳性。
    let total_kills = life_record
        .biography
        .iter()
        .filter(|entry| match entry {
            BiographyEntry::PvpEncounter { outcome, .. } => outcome == KILL_OUTCOME_EXACT,
            BiographyEntry::TribulationIntercepted { .. } => true,
            _ => false,
        })
        .count() as u32;

    // total_deaths：直接取 Lifecycle.death_count
    let total_deaths = death_count;

    // signature_skill_ids：按 new_lv 降序取前 3
    let mut sorted_milestones: Vec<&SkillMilestone> = life_record.skill_milestones.iter().collect();
    sorted_milestones.sort_by(|a, b| b.new_lv.cmp(&a.new_lv));
    // 去重（同一 skill 取最高 level 那条），然后取前 N
    let mut seen_skills: std::collections::HashSet<&SkillId> = std::collections::HashSet::new();
    let signature_skill_ids: Vec<SkillId> = sorted_milestones
        .iter()
        .filter(|m| seen_skills.insert(&m.skill))
        .take(SIGNATURE_SKILL_TOP_N)
        .map(|m| m.skill)
        .collect();

    // final_zone：最后一条带 zone 的 biography entry
    let final_zone = life_record
        .biography
        .iter()
        .rev()
        .find_map(|entry| match entry {
            BiographyEntry::PvpEncounter { zone, .. } if !zone.is_empty() => Some(zone.clone()),
            BiographyEntry::SpiritEyeBreakthrough { zone: Some(z), .. } if !z.is_empty() => {
                Some(z.clone())
            }
            _ => None,
        })
        .unwrap_or_default();

    LifeRecordSummary {
        peak_realm,
        total_kills,
        total_deaths,
        signature_skill_ids,
        final_zone,
    }
}

// ─── EpitaphGenerationSystem ─────────────────────────────────────────────────

/// EpitaphGenerationSystem：监听 PlayerTerminated → 只处理玩家（有 LifeRecord、无 NpcMarker）
/// → 提取 LifeRecordSummary → 从可选 PendingFinalThoughtStore 取遗念（P0 无则 None）
/// → 构建 EpitaphEntry → 写 WorldEpitaphRegistry → 持久化 SQLite。
#[allow(clippy::type_complexity)]
pub fn epitaph_generation_system(
    settings: Res<PersistenceSettings>,
    mut events: EventReader<PlayerTerminated>,
    mut registry: ResMut<WorldEpitaphRegistry>,
    mut pending_store: Option<ResMut<PendingFinalThoughtStore>>,
    players: Query<(&LifeRecord, &Lifecycle, Option<&Position>), Without<NpcMarker>>,
) {
    for ev in events.read() {
        let Ok((life_record, lifecycle, position)) = players.get(ev.entity) else {
            // 实体不含 LifeRecord / 或含 NpcMarker（被 Without<NpcMarker> 过滤）
            continue;
        };

        // 提取生平摘要
        let record_summary = summarize(life_record, lifecycle.death_count);

        // 取遗念（P0: PendingFinalThoughtStore 不实装，全取 None）
        let final_thought = pending_store
            .as_mut()
            .and_then(|store| store.take(&life_record.character_id))
            .unwrap_or(FinalThought::None);

        // 碑刻放置坐标（P0: 死亡时的 Position，取整数格坐标）
        let niche_pos = position.map(|pos| {
            [
                pos.0.x.floor() as i32,
                pos.0.y.floor() as i32,
                pos.0.z.floor() as i32,
            ]
        });

        // 终死亡 tick：优先取 Lifecycle.last_death_tick，fallback 取 biography 最后 Terminated tick
        let death_tick = lifecycle
            .last_death_tick
            .or_else(|| {
                life_record.biography.iter().rev().find_map(|e| {
                    if let BiographyEntry::Terminated { tick, .. } = e {
                        Some(*tick)
                    } else {
                        None
                    }
                })
            })
            .unwrap_or(0);

        let entry = EpitaphEntry {
            id: EpitaphId::new(),
            character_id: life_record.character_id.clone(),
            player_name: lifecycle.character_id.clone(),
            final_realm: record_summary.peak_realm,
            death_tick,
            niche_pos,
            record_summary,
            final_thought,
        };

        // 持久化到 SQLite（先落盘，再写内存 registry）
        if let Err(error) = crate::persistence::persist_epitaph(&settings, &entry) {
            tracing::warn!(
                "[bong][epitaph] failed to persist epitaph for `{}`: {error}",
                entry.character_id,
            );
        }

        // 写内存 Registry（超 cap 时内部淘汰最旧）
        registry.insert(entry);
    }
}

/// 向 App 注册 WorldEpitaphRegistry resource 与 EpitaphGenerationSystem。
///
/// 在 cultivation::register 中调用（在 on_player_terminated 之后以确保 Lifecycle 已更新）。
pub fn register(app: &mut valence::prelude::App) {
    app.init_resource::<WorldEpitaphRegistry>();
    // EpitaphGenerationSystem 需要在 on_player_terminated 之后执行，
    // 确保 Lifecycle/LifeRecord 处于终死亡后状态。
    app.add_systems(
        valence::prelude::Update,
        epitaph_generation_system.after(super::death_hooks::on_player_terminated),
    );
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::life_record::{BiographyEntry, LifeRecord, SkillMilestone};
    use crate::persistence::{bootstrap_sqlite, PersistenceSettings};
    use crate::player::state::canonical_player_id;
    use std::path::PathBuf;
    use valence::prelude::App;

    // ─── 测试辅助 ───────────────────────────────────────────────────────────

    fn temp_persistence(test_name: &str) -> (PersistenceSettings, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "bong-epitaph-{test_name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos(),
        ));
        let db_path = root.join("data").join("bong.db");
        let deceased_dir = root.join("library-web").join("public").join("deceased");
        let settings = PersistenceSettings::with_paths(
            &db_path,
            &deceased_dir,
            format!("epitaph-{test_name}"),
        );
        bootstrap_sqlite(settings.db_path(), settings.server_run_id())
            .expect("bootstrap should succeed");
        (settings, root)
    }

    fn make_life_record(char_id: &str) -> LifeRecord {
        LifeRecord::new(canonical_player_id(char_id))
    }

    // ─── LifeRecordSummary 字段提取 ─────────────────────────────────────────

    #[test]
    fn summarize_empty_biography_returns_awaken_zero_kills_no_skills_empty_zone() {
        let lr = make_life_record("Alice");
        let summary = summarize(&lr, 0);
        assert_eq!(
            summary.peak_realm,
            Realm::Awaken,
            "期望无突破记录时 peak_realm=Awaken，实际={:?}",
            summary.peak_realm
        );
        assert_eq!(
            summary.total_kills, 0,
            "期望空 biography 击杀数=0，实际={}",
            summary.total_kills
        );
        assert_eq!(
            summary.total_deaths, 0,
            "期望 death_count=0 → total_deaths=0，实际={}",
            summary.total_deaths
        );
        assert!(
            summary.signature_skill_ids.is_empty(),
            "期望无 skill_milestones 时 signature_skill_ids 为空，实际={:?}",
            summary.signature_skill_ids
        );
        assert!(
            summary.final_zone.is_empty(),
            "期望无 zone 条目时 final_zone 为空，实际={}",
            summary.final_zone
        );
    }

    #[test]
    fn summarize_single_breakthrough_returns_correct_peak_realm() {
        let mut lr = make_life_record("Alice");
        lr.push(BiographyEntry::BreakthroughSucceeded {
            realm: Realm::Induce,
            tick: 100,
        });
        let summary = summarize(&lr, 0);
        assert_eq!(
            summary.peak_realm,
            Realm::Induce,
            "期望单次突破到引气时 peak_realm=Induce，实际={:?}",
            summary.peak_realm
        );
    }

    #[test]
    fn summarize_multiple_breakthroughs_picks_highest_realm() {
        let mut lr = make_life_record("Alice");
        lr.push(BiographyEntry::BreakthroughSucceeded {
            realm: Realm::Induce,
            tick: 100,
        });
        lr.push(BiographyEntry::BreakthroughSucceeded {
            realm: Realm::Condense,
            tick: 500,
        });
        lr.push(BiographyEntry::BreakthroughSucceeded {
            realm: Realm::Spirit,
            tick: 2000,
        });
        let summary = summarize(&lr, 5);
        assert_eq!(
            summary.peak_realm,
            Realm::Spirit,
            "期望多次突破取最高境界 Spirit，实际={:?}",
            summary.peak_realm
        );
        assert_eq!(
            summary.total_deaths, 5,
            "期望 death_count=5 如实传入，实际={}",
            summary.total_deaths
        );
    }

    #[test]
    fn summarize_pvp_encounter_with_kill_outcome_counts_kills() {
        let mut lr = make_life_record("Alice");
        // "death_fight" — EncounterOutcome::DeathFight 唯一 wire name，计为击杀
        lr.push(BiographyEntry::PvpEncounter {
            counterparty_id: "offline:Bob".to_string(),
            outcome: "death_fight".to_string(),
            zone: "spawn".to_string(),
            context: "wilderness".to_string(),
            observed_style: None,
            appearance_hint: None,
            qi_color_hint: None,
            tick: 200,
        });
        // 第二条 death_fight
        lr.push(BiographyEntry::PvpEncounter {
            counterparty_id: "offline:Charlie".to_string(),
            outcome: "death_fight".to_string(),
            zone: "qingyun_peaks".to_string(),
            context: "resource_point".to_string(),
            observed_style: None,
            appearance_hint: None,
            qi_color_hint: None,
            tick: 400,
        });
        // 非击杀 outcome（不计）："betrayal" / "probe_fight" / "bypass" 均不算击杀对手
        lr.push(BiographyEntry::PvpEncounter {
            counterparty_id: "offline:Dave".to_string(),
            outcome: "betrayal".to_string(),
            zone: "blood_valley".to_string(),
            context: "tsy_extract".to_string(),
            observed_style: None,
            appearance_hint: None,
            qi_color_hint: None,
            tick: 600,
        });
        lr.push(BiographyEntry::PvpEncounter {
            counterparty_id: "offline:Eve".to_string(),
            outcome: "probe_fight".to_string(),
            zone: "spawn".to_string(),
            context: "wilderness".to_string(),
            observed_style: None,
            appearance_hint: None,
            qi_color_hint: None,
            tick: 800,
        });
        lr.push(BiographyEntry::PvpEncounter {
            counterparty_id: "offline:Frank".to_string(),
            outcome: "bypass".to_string(),
            zone: "spawn".to_string(),
            context: "wilderness".to_string(),
            observed_style: None,
            appearance_hint: None,
            qi_color_hint: None,
            tick: 1000,
        });
        let summary = summarize(&lr, 0);
        assert_eq!(
            summary.total_kills, 2,
            "期望只有 outcome=death_fight 的 2 条 PvpEncounter 被计入击杀，实际={}（非death_fight的outcome如betrayal/probe_fight/bypass不应计入）",
            summary.total_kills
        );
    }

    #[test]
    fn summarize_juebi_killed_does_not_count_as_kill() {
        // JueBiKilled = 玩家自己在绝壁劫中殒命（"绝壁劫殁亡"），
        // 是死亡语义而非击杀对手——不应计入 total_kills。
        let mut lr = make_life_record("Alice");
        lr.push(BiographyEntry::JueBiKilled {
            source: "void_quota_exceeded".to_string(),
            tick: 100,
        });
        let summary = summarize(&lr, 0);
        assert_eq!(
            summary.total_kills, 0,
            "期望 JueBiKilled（绝壁劫殁亡=自身死亡）不计入 total_kills，实际={}",
            summary.total_kills
        );
    }

    #[test]
    fn summarize_juebi_killed_does_not_inflate_kills_mixed_with_real_kills() {
        // 混入真实击杀（death_fight + TribulationIntercepted）后，
        // JueBiKilled 仍不应虚增 kills 计数。
        let mut lr = make_life_record("Alice");
        lr.push(BiographyEntry::JueBiKilled {
            source: "void_quota_exceeded".to_string(),
            tick: 50,
        });
        lr.push(BiographyEntry::PvpEncounter {
            counterparty_id: "offline:Bob".to_string(),
            outcome: "death_fight".to_string(),
            zone: "spawn".to_string(),
            context: "wilderness".to_string(),
            observed_style: None,
            appearance_hint: None,
            qi_color_hint: None,
            tick: 200,
        });
        lr.push(BiographyEntry::TribulationIntercepted {
            victim_id: "offline:Charlie".to_string(),
            tag: "戮道者 · 截劫".to_string(),
            tick: 400,
        });
        let summary = summarize(&lr, 0);
        assert_eq!(
            summary.total_kills, 2,
            "期望 JueBiKilled 不虚增 kills：1 death_fight + 1 TribulationIntercepted = 2，实际={}",
            summary.total_kills
        );
    }

    #[test]
    fn summarize_tribulation_intercepted_counts_as_kill() {
        let mut lr = make_life_record("Alice");
        lr.push(BiographyEntry::TribulationIntercepted {
            victim_id: "offline:Victim".to_string(),
            tag: "戮道者 · 截劫".to_string(),
            tick: 300,
        });
        let summary = summarize(&lr, 0);
        assert_eq!(
            summary.total_kills, 1,
            "期望 TribulationIntercepted 被计入击杀数，实际={}",
            summary.total_kills
        );
    }

    #[test]
    fn summarize_skill_milestones_picks_top3_by_level() {
        let mut lr = make_life_record("Alice");
        lr.push_skill_milestone(SkillMilestone {
            skill: SkillId::Herbalism,
            new_lv: 2,
            achieved_at: 100,
            narration: "草木渐熟。".to_string(),
            total_xp_at: 120,
        });
        lr.push_skill_milestone(SkillMilestone {
            skill: SkillId::Alchemy,
            new_lv: 5,
            achieved_at: 300,
            narration: "炉火识性稍深。".to_string(),
            total_xp_at: 420,
        });
        lr.push_skill_milestone(SkillMilestone {
            skill: SkillId::Combat,
            new_lv: 4,
            achieved_at: 200,
            narration: "招式熟练。".to_string(),
            total_xp_at: 300,
        });
        lr.push_skill_milestone(SkillMilestone {
            skill: SkillId::Mineral,
            new_lv: 1,
            achieved_at: 50,
            narration: "识石初步。".to_string(),
            total_xp_at: 30,
        });
        let summary = summarize(&lr, 0);
        // 期望按 new_lv 降序：Alchemy(5) > Combat(4) > Herbalism(2) > Mineral(1)
        // 取前 3
        assert_eq!(
            summary.signature_skill_ids.len(),
            3,
            "期望取前 3 个最高 level 技能，实际={:?}",
            summary.signature_skill_ids
        );
        assert_eq!(
            summary.signature_skill_ids[0],
            SkillId::Alchemy,
            "期望第一位为最高 lv=5 的 Alchemy，实际={:?}",
            summary.signature_skill_ids[0]
        );
        assert_eq!(
            summary.signature_skill_ids[1],
            SkillId::Combat,
            "期望第二位为 lv=4 的 Combat，实际={:?}",
            summary.signature_skill_ids[1]
        );
        assert_eq!(
            summary.signature_skill_ids[2],
            SkillId::Herbalism,
            "期望第三位为 lv=2 的 Herbalism，实际={:?}",
            summary.signature_skill_ids[2]
        );
    }

    #[test]
    fn summarize_final_zone_takes_last_zone_entry() {
        let mut lr = make_life_record("Alice");
        lr.push(BiographyEntry::PvpEncounter {
            counterparty_id: "offline:Bob".to_string(),
            outcome: "fled".to_string(),
            zone: "spawn".to_string(),
            context: "first_encounter".to_string(),
            observed_style: None,
            appearance_hint: None,
            qi_color_hint: None,
            tick: 100,
        });
        lr.push(BiographyEntry::PvpEncounter {
            counterparty_id: "offline:Charlie".to_string(),
            outcome: "killed".to_string(),
            zone: "blood_valley".to_string(),
            context: "final_battle".to_string(),
            observed_style: None,
            appearance_hint: None,
            qi_color_hint: None,
            tick: 500,
        });
        let summary = summarize(&lr, 0);
        assert_eq!(
            summary.final_zone, "blood_valley",
            "期望取最后一条 PvpEncounter 的 zone=blood_valley，实际={}",
            summary.final_zone
        );
    }

    #[test]
    fn summarize_spirit_eye_breakthrough_zone_contributes_to_final_zone() {
        let mut lr = make_life_record("Alice");
        lr.push(BiographyEntry::SpiritEyeBreakthrough {
            eye_id: "eye_01".to_string(),
            zone: Some("qingyun_peaks".to_string()),
            tick: 200,
        });
        let summary = summarize(&lr, 0);
        assert_eq!(
            summary.final_zone, "qingyun_peaks",
            "期望 SpiritEyeBreakthrough 的 zone 被计入 final_zone，实际={}",
            summary.final_zone
        );
    }

    // ─── WorldEpitaphRegistry cap 测试 ─────────────────────────────────────

    #[test]
    fn registry_insert_under_cap_retains_all() {
        let mut registry = WorldEpitaphRegistry::default();
        for i in 0..5u32 {
            let entry = make_test_entry(&format!("offline:player_{i}"), i as u64);
            registry.insert(entry);
        }
        assert_eq!(
            registry.entries.len(),
            5,
            "期望插入 5 条后 registry 含 5 条，实际={}",
            registry.entries.len()
        );
    }

    #[test]
    fn registry_insert_1001_evicts_oldest_from_memory() {
        let mut registry = WorldEpitaphRegistry::default();
        let mut first_id: Option<EpitaphId> = None;
        for i in 0..=1000u32 {
            let entry = make_test_entry(&format!("offline:player_{i}"), i as u64);
            if i == 0 {
                first_id = Some(entry.id.clone());
            }
            registry.insert(entry);
        }
        // 1001 条插入后 cap=1000，第 0 条被淘汰
        assert_eq!(
            registry.entries.len(),
            1000,
            "期望超 cap 1000 后 registry 只保留 1000 条，实际={}",
            registry.entries.len()
        );
        let first_id = first_id.expect("first_id should be set");
        assert!(
            !registry.entries.contains_key(&first_id),
            "期望第 0 条（最旧）被淘汰出内存 registry，但仍存在"
        );
    }

    #[test]
    fn registry_cap_exactly_1000_does_not_evict() {
        let mut registry = WorldEpitaphRegistry::default();
        let mut ids = Vec::new();
        for i in 0..1000u32 {
            let entry = make_test_entry(&format!("offline:player_{i}"), i as u64);
            ids.push(entry.id.clone());
            registry.insert(entry);
        }
        assert_eq!(
            registry.entries.len(),
            1000,
            "期望插入恰好 1000 条后不触发淘汰，实际={}",
            registry.entries.len()
        );
        // 第 0 条仍在
        assert!(
            registry.entries.contains_key(&ids[0]),
            "期望 cap=1000 刚好不淘汰第 0 条，但它不在 registry 中"
        );
    }

    // ─── SQLite persist_epitaph round-trip ─────────────────────────────────

    #[test]
    fn persist_epitaph_round_trip_reads_back_identical() {
        let (settings, root) = temp_persistence("round-trip");
        let entry = make_test_entry("offline:RoundTrip", 999);
        crate::persistence::persist_epitaph(&settings, &entry)
            .expect("persist_epitaph should succeed");
        // 重新开连接读回
        let loaded = crate::persistence::load_epitaph(&settings, entry.id.0.as_str())
            .expect("load_epitaph should succeed")
            .expect("epitaph should be present in db");
        assert_eq!(
            loaded, entry,
            "期望 SQLite round-trip 读回与写入 EpitaphEntry 完全相同，实际不同"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn persist_epitaph_idempotent_upsert() {
        let (settings, root) = temp_persistence("upsert");
        let entry = make_test_entry("offline:Upsert", 42);
        crate::persistence::persist_epitaph(&settings, &entry)
            .expect("first persist should succeed");
        // 再写一次（ON CONFLICT DO UPDATE）
        crate::persistence::persist_epitaph(&settings, &entry)
            .expect("second persist (upsert) should succeed");
        let loaded = crate::persistence::load_epitaph(&settings, entry.id.0.as_str())
            .expect("load_epitaph should succeed")
            .expect("epitaph should be present");
        assert_eq!(loaded, entry, "期望幂等写入后读回内容不变，实际不同");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn persist_epitaph_1001_all_present_in_sqlite_after_registry_eviction() {
        let (settings, root) = temp_persistence("cap-sqlite-retain");
        let mut registry = WorldEpitaphRegistry::default();
        let mut first_entry: Option<EpitaphEntry> = None;
        for i in 0..=1000u32 {
            let entry = make_test_entry(&format!("offline:p_{i}"), i as u64);
            if i == 0 {
                first_entry = Some(entry.clone());
            }
            crate::persistence::persist_epitaph(&settings, &entry)
                .expect("persist_epitaph should succeed");
            registry.insert(entry);
        }
        // 内存 registry 已淘汰第 0 条
        let first = first_entry.expect("first_entry should be set");
        assert!(
            !registry.entries.contains_key(&first.id),
            "期望第 0 条已被淘汰出内存 registry"
        );
        // 但 SQLite 仍可读回（永久保留语义）
        let in_db = crate::persistence::load_epitaph(&settings, first.id.0.as_str())
            .expect("load_epitaph should succeed")
            .expect("期望第 0 条在 SQLite 中永久保留，但未找到");
        assert_eq!(
            in_db.character_id, first.character_id,
            "期望 SQLite 中读回的 character_id 与原始条目一致，实际不同"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    // ─── FinalThought 变体 pin 测试 ─────────────────────────────────────────

    #[test]
    fn final_thought_none_serde_roundtrip() {
        let ft = FinalThought::None;
        let json = serde_json::to_string(&ft).expect("FinalThought::None 应可序列化");
        let back: FinalThought =
            serde_json::from_str(&json).expect("FinalThought::None 应可反序列化");
        assert_eq!(ft, back, "期望 FinalThought::None serde round-trip 一致");
    }

    #[test]
    fn final_thought_location_hint_serde_roundtrip() {
        let ft = FinalThought::LocationHint("血谷东侧有灵石矿脉".to_string());
        let json = serde_json::to_string(&ft).unwrap();
        let back: FinalThought = serde_json::from_str(&json).unwrap();
        assert_eq!(
            ft, back,
            "期望 FinalThought::LocationHint serde round-trip 一致"
        );
    }

    #[test]
    fn final_thought_revenge_hint_serde_roundtrip() {
        let ft = FinalThought::RevengeHint("offline:Betrayer 从背后偷袭了我".to_string());
        let json = serde_json::to_string(&ft).unwrap();
        let back: FinalThought = serde_json::from_str(&json).unwrap();
        assert_eq!(
            ft, back,
            "期望 FinalThought::RevengeHint serde round-trip 一致"
        );
    }

    #[test]
    fn final_thought_insight_hint_serde_roundtrip() {
        let ft = FinalThought::InsightHint("合虚归真，非逆也".to_string());
        let json = serde_json::to_string(&ft).unwrap();
        let back: FinalThought = serde_json::from_str(&json).unwrap();
        assert_eq!(
            ft, back,
            "期望 FinalThought::InsightHint serde round-trip 一致"
        );
    }

    // ─── PendingFinalThoughtStore 软依赖（无 store → FinalThought::None）────

    #[test]
    fn no_pending_store_yields_final_thought_none() {
        // P0：PendingFinalThoughtStore 不注册时，系统应生成 FinalThought::None
        // 此处直接测 summarize + entry 构建逻辑，不启动完整 Bevy App
        let lr = make_life_record("Wanderer");
        let summary = summarize(&lr, 0);
        let final_thought: FinalThought = Option::<&mut PendingFinalThoughtStore>::None
            .and_then(|store| store.take(&lr.character_id))
            .unwrap_or(FinalThought::None);
        assert_eq!(
            final_thought,
            FinalThought::None,
            "期望无 PendingFinalThoughtStore 时 final_thought=None，实际={:?}",
            final_thought
        );
        // summary 也正常
        assert_eq!(summary.peak_realm, Realm::Awaken);
    }

    #[test]
    fn pending_store_with_entry_returns_correct_thought() {
        let mut store = PendingFinalThoughtStore::default();
        store.insert(
            canonical_player_id("Alice"),
            FinalThought::RevengeHint("offline:Bob 杀了我".to_string()),
        );
        let thought = store
            .take(&canonical_player_id("Alice"))
            .unwrap_or(FinalThought::None);
        assert_eq!(
            thought,
            FinalThought::RevengeHint("offline:Bob 杀了我".to_string()),
            "期望从 store 取出正确的 RevengeHint"
        );
        // 取后应移除
        let second = store
            .take(&canonical_player_id("Alice"))
            .unwrap_or(FinalThought::None);
        assert_eq!(
            second,
            FinalThought::None,
            "期望 take 后再次 take 返回 None（条目已移除）"
        );
    }

    // ─── EpitaphGenerationSystem Bevy 集成测试 ──────────────────────────────

    #[test]
    fn epitaph_generation_system_creates_entry_for_player_terminated() {
        let (settings, root) = temp_persistence("system-player");
        let mut app = App::new();
        app.insert_resource(settings.clone());
        app.init_resource::<WorldEpitaphRegistry>();
        app.add_event::<PlayerTerminated>();
        app.add_systems(valence::prelude::Update, epitaph_generation_system);

        let entity = app
            .world_mut()
            .spawn((
                LifeRecord::new(canonical_player_id("Alice")),
                crate::combat::components::Lifecycle {
                    character_id: canonical_player_id("Alice"),
                    death_count: 3,
                    ..Default::default()
                },
            ))
            .id();
        app.world_mut().send_event(PlayerTerminated { entity });
        app.update();

        let registry = app.world().resource::<WorldEpitaphRegistry>();
        assert_eq!(
            registry.entries.len(),
            1,
            "期望 PlayerTerminated 触发后 registry 含 1 条碑刻，实际={}",
            registry.entries.len()
        );
        let (_id, entry) = registry
            .entries
            .iter()
            .next()
            .expect("should have one entry");
        assert_eq!(
            entry.character_id,
            canonical_player_id("Alice"),
            "期望碑刻 character_id = offline:Alice，实际={}",
            entry.character_id
        );
        assert_eq!(
            entry.record_summary.total_deaths, 3,
            "期望 death_count=3 被提取到 total_deaths，实际={}",
            entry.record_summary.total_deaths
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn epitaph_generation_system_skips_npc_terminated() {
        let (settings, root) = temp_persistence("system-npc-skip");
        let mut app = App::new();
        app.insert_resource(settings.clone());
        app.init_resource::<WorldEpitaphRegistry>();
        app.add_event::<PlayerTerminated>();
        app.add_systems(valence::prelude::Update, epitaph_generation_system);

        // NPC 实体：带 NpcMarker，Without<NpcMarker> 过滤器应排除它
        let npc_entity = app
            .world_mut()
            .spawn((
                LifeRecord::new("npc:zombie_01".to_string()),
                crate::combat::components::Lifecycle {
                    character_id: "npc:zombie_01".to_string(),
                    ..Default::default()
                },
                NpcMarker,
            ))
            .id();
        app.world_mut()
            .send_event(PlayerTerminated { entity: npc_entity });
        app.update();

        let registry = app.world().resource::<WorldEpitaphRegistry>();
        assert_eq!(
            registry.entries.len(),
            0,
            "期望 NPC 终死亡不生成碑刻（Without<NpcMarker> 过滤），实际 registry 含 {} 条",
            registry.entries.len()
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn epitaph_generation_system_entity_without_life_record_is_skipped() {
        let (settings, root) = temp_persistence("system-no-life-record");
        let mut app = App::new();
        app.insert_resource(settings.clone());
        app.init_resource::<WorldEpitaphRegistry>();
        app.add_event::<PlayerTerminated>();
        app.add_systems(valence::prelude::Update, epitaph_generation_system);

        // 实体无 LifeRecord 也无 Lifecycle
        let entity = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(PlayerTerminated { entity });
        app.update();

        let registry = app.world().resource::<WorldEpitaphRegistry>();
        assert_eq!(
            registry.entries.len(),
            0,
            "期望无 LifeRecord 的实体不生成碑刻，实际 registry 含 {} 条",
            registry.entries.len()
        );
        let _ = std::fs::remove_dir_all(root);
    }

    // ─── death_tick 三分支专属测试 ──────────────────────────────────────────

    #[test]
    fn death_tick_branch_lifecycle_last_death_tick_takes_priority() {
        // 分支 1：Lifecycle.last_death_tick 有值时，优先取它
        let (settings, root) = temp_persistence("death-tick-lifecycle");
        let mut app = App::new();
        app.insert_resource(settings.clone());
        app.init_resource::<WorldEpitaphRegistry>();
        app.add_event::<PlayerTerminated>();
        app.add_systems(valence::prelude::Update, epitaph_generation_system);

        let mut lr = LifeRecord::new(canonical_player_id("Alice"));
        // biography 里也有 Terminated，但 Lifecycle.last_death_tick 应优先
        lr.push(BiographyEntry::Terminated {
            cause: "pvp".to_string(),
            tick: 9999,
        });

        let entity = app
            .world_mut()
            .spawn((
                lr,
                crate::combat::components::Lifecycle {
                    character_id: canonical_player_id("Alice"),
                    last_death_tick: Some(42),
                    death_count: 1,
                    ..Default::default()
                },
            ))
            .id();
        app.world_mut().send_event(PlayerTerminated { entity });
        app.update();

        let registry = app.world().resource::<WorldEpitaphRegistry>();
        let entry = registry.entries.values().next().expect("应生成一条碑刻");
        assert_eq!(
            entry.death_tick, 42,
            "期望 Lifecycle.last_death_tick=42 优先于 biography Terminated.tick=9999，实际={}",
            entry.death_tick
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn death_tick_branch_biography_terminated_fallback_when_no_lifecycle_tick() {
        // 分支 2：Lifecycle.last_death_tick=None，fallback 取 biography 末尾 Terminated.tick
        let (settings, root) = temp_persistence("death-tick-biography");
        let mut app = App::new();
        app.insert_resource(settings.clone());
        app.init_resource::<WorldEpitaphRegistry>();
        app.add_event::<PlayerTerminated>();
        app.add_systems(valence::prelude::Update, epitaph_generation_system);

        let mut lr = LifeRecord::new(canonical_player_id("Bob"));
        lr.push(BiographyEntry::Terminated {
            cause: "tribulation".to_string(),
            tick: 777,
        });

        let entity = app
            .world_mut()
            .spawn((
                lr,
                crate::combat::components::Lifecycle {
                    character_id: canonical_player_id("Bob"),
                    last_death_tick: None, // 无 lifecycle tick
                    death_count: 1,
                    ..Default::default()
                },
            ))
            .id();
        app.world_mut().send_event(PlayerTerminated { entity });
        app.update();

        let registry = app.world().resource::<WorldEpitaphRegistry>();
        let entry = registry.entries.values().next().expect("应生成一条碑刻");
        assert_eq!(
            entry.death_tick, 777,
            "期望 Lifecycle.last_death_tick=None 时回退到 biography Terminated.tick=777，实际={}",
            entry.death_tick
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn death_tick_branch_fallback_zero_when_neither_source_present() {
        // 分支 3：Lifecycle.last_death_tick=None 且 biography 无 Terminated 条目 → unwrap_or(0)
        let (settings, root) = temp_persistence("death-tick-zero");
        let mut app = App::new();
        app.insert_resource(settings.clone());
        app.init_resource::<WorldEpitaphRegistry>();
        app.add_event::<PlayerTerminated>();
        app.add_systems(valence::prelude::Update, epitaph_generation_system);

        // biography 空，lifecycle.last_death_tick=None
        let lr = LifeRecord::new(canonical_player_id("Charlie"));

        let entity = app
            .world_mut()
            .spawn((
                lr,
                crate::combat::components::Lifecycle {
                    character_id: canonical_player_id("Charlie"),
                    last_death_tick: None,
                    death_count: 0,
                    ..Default::default()
                },
            ))
            .id();
        app.world_mut().send_event(PlayerTerminated { entity });
        app.update();

        let registry = app.world().resource::<WorldEpitaphRegistry>();
        let entry = registry.entries.values().next().expect("应生成一条碑刻");
        assert_eq!(
            entry.death_tick, 0,
            "期望两个 tick 来源均无时 death_tick=0（unwrap_or(0)），实际={}",
            entry.death_tick
        );
        let _ = std::fs::remove_dir_all(root);
    }

    // ─── niche_pos 两路径专属测试 ────────────────────────────────────────────

    #[test]
    fn niche_pos_set_from_position_including_floor_of_negative_coords() {
        // 有 Position 时，niche_pos 应取整数 floor 坐标（含负坐标：-5.7 → -6）
        let (settings, root) = temp_persistence("niche-pos-with-pos");
        let mut app = App::new();
        app.insert_resource(settings.clone());
        app.init_resource::<WorldEpitaphRegistry>();
        app.add_event::<PlayerTerminated>();
        app.add_systems(valence::prelude::Update, epitaph_generation_system);

        let entity = app
            .world_mut()
            .spawn((
                LifeRecord::new(canonical_player_id("Diana")),
                crate::combat::components::Lifecycle {
                    character_id: canonical_player_id("Diana"),
                    death_count: 1,
                    ..Default::default()
                },
                // 正值和负小数坐标：x=12.9 → 12, y=64.1 → 64, z=-5.7 → -6
                valence::prelude::Position(valence::prelude::DVec3::new(12.9, 64.1, -5.7)),
            ))
            .id();
        app.world_mut().send_event(PlayerTerminated { entity });
        app.update();

        let registry = app.world().resource::<WorldEpitaphRegistry>();
        let entry = registry.entries.values().next().expect("应生成一条碑刻");
        assert_eq!(
            entry.niche_pos,
            Some([12, 64, -6]),
            "期望 Position(12.9, 64.1, -5.7) floor → [12, 64, -6]，实际={:?}",
            entry.niche_pos
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn niche_pos_none_when_entity_has_no_position_component() {
        // 实体无 Position component 时 niche_pos=None
        let (settings, root) = temp_persistence("niche-pos-none");
        let mut app = App::new();
        app.insert_resource(settings.clone());
        app.init_resource::<WorldEpitaphRegistry>();
        app.add_event::<PlayerTerminated>();
        app.add_systems(valence::prelude::Update, epitaph_generation_system);

        let entity = app
            .world_mut()
            .spawn((
                LifeRecord::new(canonical_player_id("Eric")),
                crate::combat::components::Lifecycle {
                    character_id: canonical_player_id("Eric"),
                    death_count: 1,
                    ..Default::default()
                },
                // 故意不附加 Position component
            ))
            .id();
        app.world_mut().send_event(PlayerTerminated { entity });
        app.update();

        let registry = app.world().resource::<WorldEpitaphRegistry>();
        let entry = registry.entries.values().next().expect("应生成一条碑刻");
        assert_eq!(
            entry.niche_pos, None,
            "期望实体无 Position component 时 niche_pos=None，实际={:?}",
            entry.niche_pos
        );
        let _ = std::fs::remove_dir_all(root);
    }

    // ─── final_thought 系统层端到端装配 ─────────────────────────────────────

    #[test]
    fn final_thought_taken_from_pending_store_and_removed_on_player_terminated() {
        // 插入 PendingFinalThoughtStore，触发 PlayerTerminated，
        // 断言 EpitaphEntry.final_thought 取自 store 且 store 已 remove。
        let (settings, root) = temp_persistence("final-thought-store");
        let mut app = App::new();
        app.insert_resource(settings.clone());
        app.init_resource::<WorldEpitaphRegistry>();
        // 注册 PendingFinalThoughtStore（P1 路径）
        let mut store = PendingFinalThoughtStore::default();
        store.insert(
            canonical_player_id("Fang"),
            FinalThought::RevengeHint("offline:Killer 偷袭了我".to_string()),
        );
        app.insert_resource(store);
        app.add_event::<PlayerTerminated>();
        app.add_systems(valence::prelude::Update, epitaph_generation_system);

        let entity = app
            .world_mut()
            .spawn((
                LifeRecord::new(canonical_player_id("Fang")),
                crate::combat::components::Lifecycle {
                    character_id: canonical_player_id("Fang"),
                    death_count: 2,
                    ..Default::default()
                },
            ))
            .id();
        app.world_mut().send_event(PlayerTerminated { entity });
        app.update();

        let registry = app.world().resource::<WorldEpitaphRegistry>();
        let entry = registry.entries.values().next().expect("应生成一条碑刻");
        assert_eq!(
            entry.final_thought,
            FinalThought::RevengeHint("offline:Killer 偷袭了我".to_string()),
            "期望 final_thought 取自 PendingFinalThoughtStore 的 RevengeHint，实际={:?}",
            entry.final_thought
        );

        // store 已 remove（take 后条目消失，再次 take 返回 None）
        let mut store = app.world_mut().resource_mut::<PendingFinalThoughtStore>();
        let second_take = store.take(&canonical_player_id("Fang"));
        assert!(
            second_take.is_none(),
            "期望 PlayerTerminated 触发后 PendingFinalThoughtStore 中对应条目已移除（二次 take 返回 None）"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn final_thought_none_when_no_pending_store_resource_registered() {
        // 无 PendingFinalThoughtStore resource（P0 默认场景）→ final_thought=None
        let (settings, root) = temp_persistence("final-thought-no-store");
        let mut app = App::new();
        app.insert_resource(settings.clone());
        app.init_resource::<WorldEpitaphRegistry>();
        // 故意不 insert PendingFinalThoughtStore
        app.add_event::<PlayerTerminated>();
        app.add_systems(valence::prelude::Update, epitaph_generation_system);

        let entity = app
            .world_mut()
            .spawn((
                LifeRecord::new(canonical_player_id("Ghost")),
                crate::combat::components::Lifecycle {
                    character_id: canonical_player_id("Ghost"),
                    death_count: 0,
                    ..Default::default()
                },
            ))
            .id();
        app.world_mut().send_event(PlayerTerminated { entity });
        app.update();

        let registry = app.world().resource::<WorldEpitaphRegistry>();
        let entry = registry.entries.values().next().expect("应生成一条碑刻");
        assert_eq!(
            entry.final_thought,
            FinalThought::None,
            "期望无 PendingFinalThoughtStore 时 final_thought=FinalThought::None，实际={:?}",
            entry.final_thought
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn final_thought_none_when_store_has_no_entry_for_character() {
        // PendingFinalThoughtStore 存在但不含本角色的条目 → final_thought=None
        let (settings, root) = temp_persistence("final-thought-store-miss");
        let mut app = App::new();
        app.insert_resource(settings.clone());
        app.init_resource::<WorldEpitaphRegistry>();
        // store 里只有别的角色
        let mut store = PendingFinalThoughtStore::default();
        store.insert(
            canonical_player_id("OtherPlayer"),
            FinalThought::LocationHint("血谷有矿".to_string()),
        );
        app.insert_resource(store);
        app.add_event::<PlayerTerminated>();
        app.add_systems(valence::prelude::Update, epitaph_generation_system);

        let entity = app
            .world_mut()
            .spawn((
                LifeRecord::new(canonical_player_id("Heron")),
                crate::combat::components::Lifecycle {
                    character_id: canonical_player_id("Heron"),
                    death_count: 1,
                    ..Default::default()
                },
            ))
            .id();
        app.world_mut().send_event(PlayerTerminated { entity });
        app.update();

        let registry = app.world().resource::<WorldEpitaphRegistry>();
        let entry = registry.entries.values().next().expect("应生成一条碑刻");
        assert_eq!(
            entry.final_thought,
            FinalThought::None,
            "期望 store 中无本角色条目时 final_thought=None，实际={:?}",
            entry.final_thought
        );
        let _ = std::fs::remove_dir_all(root);
    }

    // ─── 辅助：构造测试用 EpitaphEntry ─────────────────────────────────────

    fn make_test_entry(character_id: &str, death_tick: u64) -> EpitaphEntry {
        EpitaphEntry {
            id: EpitaphId::new(),
            character_id: character_id.to_string(),
            player_name: character_id.to_string(),
            final_realm: Realm::Induce,
            death_tick,
            niche_pos: Some([10, 64, -5]),
            record_summary: LifeRecordSummary {
                peak_realm: Realm::Induce,
                total_kills: 2,
                total_deaths: 1,
                signature_skill_ids: vec![SkillId::Combat],
                final_zone: "spawn".to_string(),
            },
            final_thought: FinalThought::None,
        }
    }
}
