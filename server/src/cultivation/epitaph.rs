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
    bevy_ecs, EventReader, IntoSystemConfigs, Position, Query, Res, ResMut, Resource, Username,
    Without,
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
    /// 合计定向击杀数（仅 TribulationIntercepted——截劫者专属，记录在截劫者一方）。
    ///
    /// PvpEncounter{outcome="death_fight"} **不**计入：该条目由双方（winner+loser）对称写入，
    /// 无法区分击杀方与被击杀方，直接计数会给被杀的输家凭空+1击杀（幻影战绩）。
    /// PvpEncounter 的方向性击杀语义留 P1 在 PvpEncounter 模型加 winner/survived 字段后补全。
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
    /// 玩家游戏内显示名（取自 Username component）。
    ///
    /// EpitaphGenerationSystem 优先从 Username component 读取；若 terminate 时
    /// Username component 已不存在（如非 Client entity），则回退到 lifecycle.character_id。
    pub player_name: String,
    /// 有生以来最高境界（= LifeRecordSummary.peak_realm）。
    ///
    /// P0 无法从死亡事件快照死时境界（on_player_terminated 时 Cultivation 组件已 remove，
    /// biography 只有 BreakthroughSucceeded 峰值，无回退记录）。故 P0 诚实记录峰值境界。
    /// 死亡时实际境界（若与峰值不同）留 P1 在 PlayerTerminated 前快照进 LifeRecord 后补全。
    pub peak_realm: Realm,
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

// NOTE: PvpEncounter{outcome="death_fight"} 不用于计算 total_kills。
// "death_fight" 是对称记录（pvp_encounter.rs record_life_entries 给格斗双方各写一条
// outcome="death_fight"），PvpEncounter 模型无 killer/victim 方向区分。
// 直接计数会给被击杀的输家凭空 +1 击杀（幻影战绩）。
// PvpEncounter 方向性字段（winner/survived）由 P1 补全后再重新接入 total_kills。

/// 从 LifeRecord + Lifecycle.death_count 提取 LifeRecordSummary。
///
/// # 字段语义
/// * `peak_realm`：biography 中 BreakthroughSucceeded{realm} 取最高（按 Realm::rank()），无则 Awaken
/// * `total_kills`：仅 TribulationIntercepted（截劫，方向性——只写入截劫方）。
///   PvpEncounter{outcome="death_fight"} 是对称记录（双方均收到），不计入此处以避免幻影战绩。
/// * `total_deaths`：传入 lifecycle.death_count（Lifecycle component 最精确）
/// * `signature_skill_ids`：skill_milestones 按 new_lv 降序取前 3（同 skill 去重取最高 level）
/// * `final_zone`：最后一条带非空 zone 的 biography entry（倒序扫，PvpEncounter/SpiritEyeBreakthrough）
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

    // total_kills：仅计 TribulationIntercepted（截劫，方向性记录，只写入截劫者一方）。
    //
    // PvpEncounter{outcome="death_fight"} **不**计入：pvp_encounter.rs record_life_entries
    // 给格斗双方各写一条 outcome="death_fight"，无法区分击杀方与被击杀方，
    // 直接计数会给被击杀的输家凭空 +1 击杀（幻影战绩）。
    // PvpEncounter 方向性 kill 由 P1 加 winner/survived 字段后接入。
    let total_kills = life_record
        .biography
        .iter()
        .filter(|entry| matches!(entry, BiographyEntry::TribulationIntercepted { .. }))
        .count() as u32;

    // total_deaths：直接取 Lifecycle.death_count
    let total_deaths = death_count;

    // signature_skill_ids：按 new_lv 降序取前 3
    let mut sorted_milestones: Vec<&SkillMilestone> = life_record.skill_milestones.iter().collect();
    sorted_milestones.sort_by_key(|milestone| std::cmp::Reverse(milestone.new_lv));
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
    players: Query<
        (
            &LifeRecord,
            &Lifecycle,
            Option<&Position>,
            Option<&Username>,
        ),
        Without<NpcMarker>,
    >,
) {
    for ev in events.read() {
        let Ok((life_record, lifecycle, position, username)) = players.get(ev.entity) else {
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

        // 游戏内显示名：优先 Username component，回退 lifecycle.character_id
        let player_name = username
            .map(|u| u.0.as_str().to_string())
            .unwrap_or_else(|| lifecycle.character_id.clone());

        let entry = EpitaphEntry {
            id: EpitaphId::new(),
            character_id: life_record.character_id.clone(),
            player_name,
            peak_realm: record_summary.peak_realm,
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
