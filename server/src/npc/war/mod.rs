//! plan-offscreen-war-v1 P6：涌现区域冲突生命周期（reframe b）。
//!
//! 末法残土**无宣战 / 无具名宗门**。离屏 dormant 散修群体在某 zone 累积互殴越过阈值
//! → 自发升级成「战事」（Emerging → Skirmish → Settling → Aftermath）。
//!
//! 状态机纯函数，零真元账户操作——`pressure` 是无量纲阈值标量，仅用于状态机判定，
//! 不回写任何账户、不触发 transfer。真元流动仍唯一走 P2 `release_dormant_qi_to_zone`。
//!
//! 战事关联的对象是**匿名区域群体**（裸 `EmergentGroupId`，无「青云猎盟」式专名），
//! 叙事沿用「{zone}一带散修」区域描述符。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Event, EventReader, EventWriter, ResMut, Resource};

pub mod settle;

use crate::npc::faction::EmergentGroupId;

// ─────────────────────────── 阈值常量 ────────────────────────────────────────

/// 越过即从无到有创建 Emerging 战事（pressure 累积起步阈）。
pub const WAR_PRESSURE_THRESHOLD: f64 = 30.0;
/// 再越过升 Skirmish（白热化，玩家可介入）。
pub const WAR_SKIRMISH_PRESSURE: f64 = 60.0;
/// loser 累计 ≥ 此数进 Settling（胜负已分，结算倒计时）。
pub const WAR_SETTLE_CASUALTIES: u32 = 6;
/// Settling 后冷却进 Aftermath 并清 zone_active（tick 数）。
pub const WAR_AFTERMATH_COOLDOWN_TICKS: u64 = 200;
/// 关联群体下限——少于此数不创建战事。
pub const WAR_MIN_GROUPS: usize = 2;

// ─────────────────────────── WarPhase ────────────────────────────────────────

/// 涌现冲突阶段（reframe b：自发升级语义，非宣战）。
///
/// - `Emerging`：涌现，阈值刚越过，区域冲突起势
/// - `Skirmish`：野战，白热化，玩家可介入
/// - `Settling`：结算，胜负已分，余量清算中
/// - `Aftermath`：余波，终态，冷却等待回收
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WarPhase {
    Emerging,
    Skirmish,
    Settling,
    Aftermath,
}

impl WarPhase {
    #[allow(dead_code)]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Emerging => "emerging",
            Self::Skirmish => "skirmish",
            Self::Settling => "settling",
            Self::Aftermath => "aftermath",
        }
    }
}

// ─────────────────────────── WarRole ─────────────────────────────────────────

/// 玩家在涌现冲突中的立场（reframe b：投靠匿名区域群体，非加入宗门）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WarRole {
    /// 投靠：长期站某一群体
    Enlist,
    /// 佣兵：按战果计酬，临时
    Mercenary,
    /// 截胡：敌对双方都打，趁火打劫
    Intercept,
    /// 旁观：默认，不参与
    Spectate,
}

// ─────────────────────────── PlayerFactionRole ───────────────────────────────

/// 单个玩家在某 war 的角色记录。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerFactionRole {
    pub player_id: String,
    pub role: WarRole,
    /// Enlist/Mercenary 必填；Intercept/Spectate 强制 None。
    pub allied_group: Option<EmergentGroupId>,
    pub joined_tick: u64,
}

// ─────────────────────────── WarId ───────────────────────────────────────────

/// war_id：稳定标识一场涌现冲突（zone + 单调计数器派生，确定性）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WarId(pub u64);

// ─────────────────────────── FactionWarOutcome ───────────────────────────────

/// 冲突结算结果（reframe b：胜负群体均为裸 group_id）。
#[derive(Clone, Debug, PartialEq)]
pub struct FactionWarOutcome {
    /// 胜方群体（压力贡献最大；平票取最小 group_id，确定性）。
    pub winner_group: EmergentGroupId,
    /// 败方群体。
    pub loser_group: EmergentGroupId,
    /// 累积 outcome 条数（loser 一侧计数）。
    pub total_casualties: u32,
    pub settled_tick: u64,
}

// ─────────────────────────── FactionWar ──────────────────────────────────────

/// 一场涌现冲突的完整运行时状态。
#[derive(Clone, Debug, PartialEq)]
pub struct FactionWar {
    pub war_id: WarId,
    pub zone: String,
    /// `"{zone}一带散修"`——复用 P5 派生规则，禁具名宗门。
    pub region_descriptor: String,
    pub phase: WarPhase,
    /// ≥2，去重升序
    pub groups: Vec<EmergentGroupId>,
    pub player_roles: Vec<PlayerFactionRole>,
    /// 累积冲突压力（来自 outcome.qi_released 之和，仅作阈值标量，不是真元账户）。
    pub pressure: f64,
    pub started_tick: u64,
    pub last_update_tick: u64,
    /// Settling/Aftermath 才 Some
    pub outcome: Option<FactionWarOutcome>,
    /// plan-offscreen-war-v1 P9：从 ZonePressureEntry 同步过来的 per-group 胜场数，
    /// 供 determine_winner_loser 取 max 用。key=group, value=wins。
    pub wins_by_group: HashMap<EmergentGroupId, u32>,
}

impl FactionWar {
    /// 尝试推进阶段，返回新相（若发生转换）。
    ///
    /// 合法转换表：
    /// - Emerging → Skirmish：pressure ≥ WAR_SKIRMISH_PRESSURE
    /// - Emerging → Emerging：pressure 增长但未越阈
    /// - Skirmish → Settling：casualties ≥ WAR_SETTLE_CASUALTIES
    /// - Skirmish → Skirmish：未达结算
    /// - Settling → Aftermath：now - settled_tick ≥ WAR_AFTERMATH_COOLDOWN_TICKS
    /// - Settling → Settling：冷却未到
    /// - Aftermath：终态，不可再推进
    ///
    /// 非法转换（Emerging→Settling、Skirmish→Aftermath、任意倒退）均被 match 守卫拦截，
    /// 返回 None（无变化）。
    pub fn try_advance(&mut self, now: u64) -> Option<WarPhase> {
        self.last_update_tick = now;
        match self.phase {
            WarPhase::Emerging => {
                if self.pressure >= WAR_SKIRMISH_PRESSURE {
                    self.phase = WarPhase::Skirmish;
                    Some(WarPhase::Skirmish)
                } else {
                    None // A→A，仅刷新 tick
                }
            }
            WarPhase::Skirmish => {
                let casualties = self
                    .outcome
                    .as_ref()
                    .map(|o| o.total_casualties)
                    .unwrap_or(0);
                if casualties >= WAR_SETTLE_CASUALTIES {
                    // 结算：计算 winner/loser（winner = 压力贡献最大群体，平票取最小 group_id）
                    // 当前 outcome 已有 casualties，选取 winner_group
                    let (winner_group, loser_group) =
                        self.determine_winner_loser(self.groups.clone());
                    let total_casualties = casualties;
                    self.outcome = Some(FactionWarOutcome {
                        winner_group,
                        loser_group,
                        total_casualties,
                        settled_tick: now,
                    });
                    self.phase = WarPhase::Settling;
                    Some(WarPhase::Settling)
                } else {
                    None // A→A
                }
            }
            WarPhase::Settling => {
                if let Some(ref outcome) = self.outcome {
                    if now.saturating_sub(outcome.settled_tick) >= WAR_AFTERMATH_COOLDOWN_TICKS {
                        self.phase = WarPhase::Aftermath;
                        Some(WarPhase::Aftermath)
                    } else {
                        None // A→A
                    }
                } else {
                    None
                }
            }
            WarPhase::Aftermath => None, // 终态，不可再推进
        }
    }

    /// plan-offscreen-war-v1 P9：确定 winner/loser（winner = wins_by_group 最多群体）。
    /// 精化规则：按各群体累积胜场数取 max；平票取最小 group_id（确定性）。
    /// wins_by_group 由 ZonePressureEntry.accumulate 维护，escalate_or_create 同步到 war。
    fn determine_winner_loser(
        &self,
        mut groups: Vec<EmergentGroupId>,
    ) -> (EmergentGroupId, EmergentGroupId) {
        groups.sort();
        groups.dedup();
        if groups.len() < 2 {
            // 理论上不应到达（WAR_MIN_GROUPS ≥ 2），兜底
            let g = groups.first().copied().unwrap_or(EmergentGroupId(0));
            return (g, g);
        }
        // winner = wins 最多；平票取最小 group_id
        // max_by_key：相等时返回最后一个 → 为确保取最小 id，平票用 Reverse(g.0)
        let winner = *groups
            .iter()
            .max_by_key(|g| {
                let wins = self.wins_by_group.get(g).copied().unwrap_or(0);
                (wins, std::cmp::Reverse(g.0))
            })
            .unwrap();
        // loser = 非 winner 中 wins 最少；平票取最小 group_id（正序）
        let loser = *groups
            .iter()
            .filter(|g| **g != winner)
            .min_by_key(|g| {
                let wins = self.wins_by_group.get(g).copied().unwrap_or(0);
                (wins, g.0)
            })
            .unwrap_or(&groups[1]);
        (winner, loser)
    }

    /// 应用玩家参与 intent。
    pub fn apply_participation(
        &mut self,
        player_id: &str,
        role: WarRole,
        allied_group: Option<EmergentGroupId>,
        at_tick: u64,
    ) -> Result<WarRole, WarParticipateError> {
        // 阶段检查：仅 Emerging/Skirmish 可 join
        match self.phase {
            WarPhase::Emerging | WarPhase::Skirmish => {}
            WarPhase::Settling | WarPhase::Aftermath => {
                return Err(WarParticipateError::PhaseNotJoinable);
            }
        }

        // 规范化 allied_group：Intercept/Spectate 强制 None
        let effective_group = match role {
            WarRole::Intercept | WarRole::Spectate => None,
            WarRole::Enlist | WarRole::Mercenary => {
                let Some(g) = allied_group else {
                    return Err(WarParticipateError::MissingAlliedGroup);
                };
                if !self.groups.contains(&g) {
                    return Err(WarParticipateError::GroupNotInWar);
                }
                Some(g)
            }
        };

        // 查找既有记录
        if let Some(existing) = self
            .player_roles
            .iter_mut()
            .find(|r| r.player_id == player_id)
        {
            // AlreadyEnlisted：同 group 同 role 重复
            if existing.role == role
                && role == WarRole::Enlist
                && existing.allied_group == effective_group
            {
                return Err(WarParticipateError::AlreadyEnlisted);
            }
            // 覆盖既有记录
            existing.role = role;
            existing.allied_group = effective_group;
            existing.joined_tick = at_tick;
        } else {
            self.player_roles.push(PlayerFactionRole {
                player_id: player_id.to_string(),
                role,
                allied_group: effective_group,
                joined_tick: at_tick,
            });
        }

        Ok(role)
    }

    /// 聚合玩家角色计数。
    pub fn role_counts(&self) -> PlayerRoleCounts {
        let mut counts = PlayerRoleCounts::default();
        for r in &self.player_roles {
            match r.role {
                WarRole::Enlist => counts.enlist += 1,
                WarRole::Mercenary => counts.mercenary += 1,
                WarRole::Intercept => counts.intercept += 1,
                WarRole::Spectate => counts.spectate += 1,
            }
        }
        counts
    }
}

// ─────────────────────────── PlayerRoleCounts ────────────────────────────────

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerRoleCounts {
    pub enlist: u32,
    pub mercenary: u32,
    pub intercept: u32,
    pub spectate: u32,
}

// ─────────────────────────── ZonePressureEntry ───────────────────────────────

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ZonePressureEntry {
    /// 累积无量纲压力（每条 outcome 各 +1，纯计数，切断真元语义关联）
    pub pressure: f64,
    /// 参与群体集合（去重升序）
    pub groups: Vec<EmergentGroupId>,
    /// 累积 loser 计数
    pub casualties: u32,
    pub last_tick: u64,
    /// plan-offscreen-war-v1 P9：每组累积「赢的场次」，determine_winner_loser 取 max。
    /// key = winner 群体，value = 赢场数；loser 侧不计入。
    pub wins_by_group: HashMap<EmergentGroupId, u32>,
}

impl ZonePressureEntry {
    /// 从一条 outcome 累加 pressure（纯计数，+1/条）+ 更新 groups + casualties。
    /// winner 侧 wins_by_group +1；loser 侧不计 win。
    pub fn accumulate(&mut self, winner: EmergentGroupId, loser: EmergentGroupId, tick: u64) {
        self.pressure += 1.0; // 纯计数，切断真元语义
        self.casualties += 1;
        self.last_tick = tick;
        // winner 侧 wins +1
        *self.wins_by_group.entry(winner).or_insert(0) += 1;
        // 合并 groups，去重升序（winner + loser 均计入）
        for g in [winner, loser] {
            if !self.groups.contains(&g) {
                self.groups.push(g);
            }
        }
        self.groups.sort();
    }
}

// ─────────────────────────── ZoneConflictPressure ────────────────────────────

/// 区域冲突压力累加器（订阅 DormantCombatOutcome）。
#[derive(Resource, Default)]
pub struct ZoneConflictPressure {
    pub zones: HashMap<String, ZonePressureEntry>,
}

impl ZoneConflictPressure {
    pub fn entry(&mut self, zone: &str) -> &mut ZonePressureEntry {
        self.zones.entry(zone.to_string()).or_default()
    }

    pub fn get(&self, zone: &str) -> Option<&ZonePressureEntry> {
        self.zones.get(zone)
    }
}

// ─────────────────────────── WarConflictStore ────────────────────────────────

/// war 生命周期 Resource。
#[derive(Resource, Default)]
pub struct WarConflictStore {
    pub wars: HashMap<WarId, FactionWar>,
    pub next_id: u64,
    /// zone → 当前 active war（同 zone 一次只一场，Aftermath 后清）
    pub zone_active: HashMap<String, WarId>,
}

impl WarConflictStore {
    /// 派生 region_descriptor："{zone}一带散修"（禁具名宗门）。
    fn region_descriptor(zone: &str) -> String {
        format!("{zone}一带散修")
    }

    /// 升级或创建 war，返回变化后的（war_id, 新 phase, war 快照）。
    ///
    /// - 若 zone 已有 active war：对其追加 pressure/casualties/groups，调 `try_advance`（可连跳多级）
    /// - 若无 active war 且 pressure ≥ WAR_PRESSURE_THRESHOLD 且 groups ≥ 2：新建 Emerging
    pub fn escalate_or_create(
        &mut self,
        zone: &str,
        entry: &ZonePressureEntry,
        now: u64,
    ) -> Vec<(WarId, WarPhase, FactionWar)> {
        if entry.groups.len() < WAR_MIN_GROUPS {
            return vec![];
        }
        let mut changes = vec![];

        if let Some(&war_id) = self.zone_active.get(zone) {
            // 已有 active war：更新 pressure/casualties/groups/wins_by_group
            if let Some(war) = self.wars.get_mut(&war_id) {
                war.pressure = entry.pressure;
                war.last_update_tick = now;
                // 合并 groups
                for &g in &entry.groups {
                    if !war.groups.contains(&g) {
                        war.groups.push(g);
                    }
                }
                war.groups.sort();
                // 同步 wins_by_group（逐字段拷贝，plan-offscreen-war-v1 P9）
                war.wins_by_group = entry.wins_by_group.clone();
                // casualties 先同步到既有 outcome（若已有）
                if let Some(ref mut outcome) = war.outcome {
                    outcome.total_casualties = entry.casualties;
                }
                // 连跳多级直到稳定
                // 注意：在每次 try_advance 之前确保 Skirmish 阶段有 outcome，
                // 这样从 Emerging 一跳到 Skirmish 后能继续升 Settling（修复 CR#405 bug）。
                loop {
                    // 每次循环前：若已在 Skirmish 且 outcome 还未 Some，补上 placeholder
                    if war.phase == WarPhase::Skirmish
                        && war.outcome.is_none()
                        && entry.casualties > 0
                    {
                        let (winner, loser) = war.determine_winner_loser(war.groups.clone());
                        war.outcome = Some(FactionWarOutcome {
                            winner_group: winner,
                            loser_group: loser,
                            total_casualties: entry.casualties,
                            settled_tick: now,
                        });
                    } else if let Some(ref mut outcome) = war.outcome {
                        // 保持 casualties 最新
                        outcome.total_casualties = entry.casualties;
                    }
                    let prev_phase = war.phase;
                    if let Some(new_phase) = war.try_advance(now) {
                        let snapshot = war.clone();
                        changes.push((war_id, new_phase, snapshot));
                        if new_phase == WarPhase::Aftermath {
                            break;
                        }
                    } else {
                        // 无变化，检查 A→A 刷新（按 spec 已在 try_advance 里更新 last_update_tick）
                        let _ = prev_phase;
                        break;
                    }
                }
            }
        } else if entry.pressure >= WAR_PRESSURE_THRESHOLD {
            // 新建 Emerging war
            let war_id = WarId(self.next_id);
            self.next_id += 1;
            let mut groups = entry.groups.clone();
            groups.sort();
            groups.dedup();
            let war = FactionWar {
                war_id,
                zone: zone.to_string(),
                region_descriptor: Self::region_descriptor(zone),
                phase: WarPhase::Emerging,
                groups,
                player_roles: vec![],
                pressure: entry.pressure,
                started_tick: now,
                last_update_tick: now,
                outcome: None,
                wins_by_group: entry.wins_by_group.clone(),
            };
            let snapshot = war.clone();
            self.wars.insert(war_id, war);
            self.zone_active.insert(zone.to_string(), war_id);
            changes.push((war_id, WarPhase::Emerging, snapshot));

            // 新建后立即尝试连跳（同样需要在循环内补 placeholder，CR#405 fix）
            loop {
                let w = self.wars.get_mut(&war_id).unwrap();
                // 每次循环前：若已在 Skirmish 且 outcome 还未 Some，补上 placeholder
                if w.phase == WarPhase::Skirmish && w.outcome.is_none() && entry.casualties > 0 {
                    let (winner, loser) = w.determine_winner_loser(w.groups.clone());
                    w.outcome = Some(FactionWarOutcome {
                        winner_group: winner,
                        loser_group: loser,
                        total_casualties: entry.casualties,
                        settled_tick: now,
                    });
                } else if let Some(ref mut outcome) = w.outcome {
                    outcome.total_casualties = entry.casualties;
                }
                if let Some(new_phase) = w.try_advance(now) {
                    let snapshot = w.clone();
                    changes.push((war_id, new_phase, snapshot));
                    if new_phase == WarPhase::Aftermath {
                        self.zone_active.remove(zone);
                        break;
                    }
                } else {
                    break;
                }
            }
        }

        changes
    }

    /// 推进所有 Settling war 的冷却（Settling → Aftermath）。
    /// 返回发生变化的 (war_id, new_phase, war_snapshot)。
    pub fn advance_settling_wars(&mut self, now: u64) -> Vec<(WarId, WarPhase, FactionWar)> {
        let mut changes = vec![];
        let settling_ids: Vec<WarId> = self
            .wars
            .iter()
            .filter(|(_, w)| w.phase == WarPhase::Settling)
            .map(|(id, _)| *id)
            .collect();
        for war_id in settling_ids {
            if let Some(war) = self.wars.get_mut(&war_id) {
                if let Some(new_phase) = war.try_advance(now) {
                    if new_phase == WarPhase::Aftermath {
                        let zone = war.zone.clone();
                        let snapshot = war.clone();
                        changes.push((war_id, new_phase, snapshot));
                        self.zone_active.remove(&zone);
                    }
                }
            }
        }
        changes
    }

    /// 玩家参与：返回 Ok(new_role) 或 Err。
    pub fn participate(
        &mut self,
        zone: &str,
        player_id: &str,
        role: WarRole,
        allied_group: Option<EmergentGroupId>,
        at_tick: u64,
    ) -> Result<(WarId, WarRole), WarParticipateError> {
        let war_id = *self
            .zone_active
            .get(zone)
            .ok_or(WarParticipateError::WarNotFound)?;
        let war = self
            .wars
            .get_mut(&war_id)
            .ok_or(WarParticipateError::WarNotFound)?;
        let new_role = war.apply_participation(player_id, role, allied_group, at_tick)?;
        Ok((war_id, new_role))
    }
}

// ─────────────────────────── WarParticipateError ─────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WarParticipateError {
    /// zone 无 active war
    WarNotFound,
    /// 仅 Emerging/Skirmish 可 join；Settling/Aftermath 拒绝
    PhaseNotJoinable,
    /// allied_group 不属于该 war 的 groups
    GroupNotInWar,
    /// 同 group 同 role 重复投靠
    AlreadyEnlisted,
    /// Enlist/Mercenary 未带 group
    MissingAlliedGroup,
}

// ─────────────────────────── Events ──────────────────────────────────────────

/// 玩家参与意图（brigadier 与 headless 注入两路的共同出口）。
#[derive(Clone, Debug, Event, PartialEq)]
pub struct WarParticipateIntent {
    pub player_id: String,
    /// 目标区域（war 按 zone 查 active）
    pub zone: String,
    pub role: WarRole,
    pub allied_group: Option<EmergentGroupId>,
    pub at_tick: u64,
}

/// war 阶段推进事件（供 publish system 转 telemetry）。
#[derive(Clone, Debug, Event, PartialEq)]
pub struct WarPhaseChanged {
    pub war_id: WarId,
    pub zone: String,
    pub region_descriptor: String,
    pub phase: WarPhase,
    pub groups: Vec<EmergentGroupId>,
    pub outcome: Option<FactionWarOutcome>,
    pub player_role_counts: PlayerRoleCounts,
    /// plan-offscreen-war-v1 P9：战事 settle 时的玩家角色快照，供 Renown 授予 system 用。
    /// 非 Settling/Aftermath 阶段为空 vec。
    pub war_snapshot_player_roles: Vec<PlayerFactionRole>,
    pub at_tick: u64,
}

// ─────────────────────────── Bevy systems ─────────────────────────────────────

/// plan-offscreen-war-v1 P6：处理玩家参与意图（brigadier 路径 A + headless 路径 B 汇聚点）。
///
/// 从 `WarParticipateIntent` 队列读取，对每条 intent 调 `WarConflictStore::participate`，
/// 成功时追加 emit 一条 `WarPhaseChanged`（phase 不变，仅 counts 刷新）让 telemetry 反映新 role；
/// 失败时打 debug log（期望X因Y实际Z 格式）。
pub fn handle_war_participate_intent(
    mut intents: EventReader<WarParticipateIntent>,
    mut war_store: ResMut<WarConflictStore>,
    mut phase_changed: EventWriter<WarPhaseChanged>,
) {
    for intent in intents.read() {
        match war_store.participate(
            &intent.zone,
            &intent.player_id,
            intent.role,
            intent.allied_group,
            intent.at_tick,
        ) {
            Ok((war_id, _new_role)) => {
                // 参与成功：emit phase-changed（phase 不变，counts 刷新）让 telemetry 即时反映
                if let Some(war) = war_store.wars.get(&war_id) {
                    let counts = war.role_counts();
                    phase_changed.send(WarPhaseChanged {
                        war_id,
                        zone: war.zone.clone(),
                        region_descriptor: war.region_descriptor.clone(),
                        phase: war.phase,
                        groups: war.groups.clone(),
                        outcome: war.outcome.clone(),
                        player_role_counts: counts,
                        war_snapshot_player_roles: war.player_roles.clone(),
                        at_tick: intent.at_tick,
                    });
                }
            }
            Err(err) => {
                tracing::debug!(
                    "[bong][war] participate rejected: 期望 zone={} 有可加入 war 因 {:?}，player_id={} role={:?} 实际拒绝",
                    intent.zone, err, intent.player_id, intent.role
                );
            }
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
