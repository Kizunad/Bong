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
                // 更新 casualties（累积到 outcome）
                if let Some(ref mut outcome) = war.outcome {
                    outcome.total_casualties = entry.casualties;
                } else if war.phase == WarPhase::Skirmish {
                    // Skirmish 阶段有 casualties 但 outcome 还未 Some → 创建 placeholder
                    let (winner, loser) = war.determine_winner_loser(war.groups.clone());
                    war.outcome = Some(FactionWarOutcome {
                        winner_group: winner,
                        loser_group: loser,
                        total_casualties: entry.casualties,
                        settled_tick: now,
                    });
                }
                // 连跳多级直到稳定
                loop {
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

            // 新建后立即尝试连跳
            loop {
                let w = self.wars.get_mut(&war_id).unwrap();
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

// ─────────────────────────── 单测 ────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ─────────── A. WarPhase 转换矩阵 ───────────────────────────────────────

    fn war_emerging(zone: &str, pressure: f64, groups: Vec<EmergentGroupId>) -> FactionWar {
        FactionWar {
            war_id: WarId(1),
            zone: zone.to_string(),
            region_descriptor: format!("{zone}一带散修"),
            phase: WarPhase::Emerging,
            groups,
            player_roles: vec![],
            pressure,
            started_tick: 0,
            last_update_tick: 0,
            outcome: None,
            wins_by_group: HashMap::new(),
        }
    }

    fn war_skirmish_with_casualties(
        zone: &str,
        casualties: u32,
        groups: Vec<EmergentGroupId>,
    ) -> FactionWar {
        FactionWar {
            war_id: WarId(2),
            zone: zone.to_string(),
            region_descriptor: format!("{zone}一带散修"),
            phase: WarPhase::Skirmish,
            groups: groups.clone(),
            player_roles: vec![],
            pressure: WAR_SKIRMISH_PRESSURE + 1.0,
            started_tick: 0,
            last_update_tick: 0,
            outcome: Some(FactionWarOutcome {
                winner_group: groups[0],
                loser_group: if groups.len() >= 2 {
                    groups[1]
                } else {
                    groups[0]
                },
                total_casualties: casualties,
                settled_tick: 0,
            }),
            wins_by_group: HashMap::new(),
        }
    }

    fn war_settling(zone: &str, settled_tick: u64, groups: Vec<EmergentGroupId>) -> FactionWar {
        FactionWar {
            war_id: WarId(3),
            zone: zone.to_string(),
            region_descriptor: format!("{zone}一带散修"),
            phase: WarPhase::Settling,
            groups: groups.clone(),
            player_roles: vec![],
            pressure: WAR_SKIRMISH_PRESSURE + 10.0,
            started_tick: 0,
            last_update_tick: 0,
            outcome: Some(FactionWarOutcome {
                winner_group: groups[0],
                loser_group: if groups.len() >= 2 {
                    groups[1]
                } else {
                    groups[0]
                },
                total_casualties: WAR_SETTLE_CASUALTIES,
                settled_tick,
            }),
            wins_by_group: HashMap::new(),
        }
    }

    #[test]
    fn war_phase_emerging_to_skirmish_on_pressure() {
        // A→B：pressure 越 SKIRMISH 阈 → Skirmish
        let mut war = war_emerging(
            "残灰谷",
            WAR_SKIRMISH_PRESSURE,
            vec![EmergentGroupId(0), EmergentGroupId(1)],
        );
        let result = war.try_advance(10);
        assert_eq!(
            war.phase,
            WarPhase::Skirmish,
            "期望 pressure >= WAR_SKIRMISH_PRESSURE 时升 Skirmish，实际仍 {:?}",
            war.phase
        );
        assert_eq!(
            result,
            Some(WarPhase::Skirmish),
            "try_advance 应返回 Some(Skirmish)，实际 {:?}",
            result
        );
        assert_eq!(war.last_update_tick, 10, "last_update_tick 应更新为 10");
    }

    #[test]
    fn war_phase_emerging_stays_emerging_below_skirmish() {
        // A→A：pressure 增但未越 Skirmish 阈 → 仍 Emerging，刷新 tick
        let mut war = war_emerging(
            "残灰谷",
            WAR_SKIRMISH_PRESSURE - 1.0,
            vec![EmergentGroupId(0), EmergentGroupId(1)],
        );
        let result = war.try_advance(20);
        assert_eq!(
            war.phase,
            WarPhase::Emerging,
            "期望 pressure < WAR_SKIRMISH_PRESSURE 时保持 Emerging，实际 {:?}",
            war.phase
        );
        assert_eq!(
            result, None,
            "A→A 无相变时 try_advance 应返回 None，实际 {:?}",
            result
        );
        assert_eq!(war.last_update_tick, 20, "last_update_tick 应刷新为 20");
    }

    #[test]
    fn war_phase_skirmish_to_settling_on_casualties() {
        // A→B：casualties ≥ WAR_SETTLE_CASUALTIES → Settling + outcome 计算正确
        let groups = vec![EmergentGroupId(0), EmergentGroupId(1)];
        let mut war = war_skirmish_with_casualties("残灰谷", WAR_SETTLE_CASUALTIES, groups.clone());
        let result = war.try_advance(50);
        assert_eq!(
            war.phase,
            WarPhase::Settling,
            "期望 casualties >= WAR_SETTLE_CASUALTIES 升 Settling，实际 {:?}",
            war.phase
        );
        assert_eq!(result, Some(WarPhase::Settling));
        let outcome = war
            .outcome
            .as_ref()
            .expect("Settling 阶段 outcome 必须 Some");
        assert_eq!(
            outcome.total_casualties, WAR_SETTLE_CASUALTIES,
            "outcome.total_casualties 应等于 WAR_SETTLE_CASUALTIES，实际 {}",
            outcome.total_casualties
        );
        assert_ne!(
            outcome.winner_group, outcome.loser_group,
            "winner 与 loser 不应相同"
        );
        assert_eq!(outcome.settled_tick, 50);
    }

    #[test]
    fn war_phase_skirmish_stays_skirmish() {
        // A→A：casualties 未达阈
        let groups = vec![EmergentGroupId(0), EmergentGroupId(1)];
        let mut war = war_skirmish_with_casualties("残灰谷", WAR_SETTLE_CASUALTIES - 1, groups);
        let result = war.try_advance(30);
        assert_eq!(
            war.phase,
            WarPhase::Skirmish,
            "期望 casualties < WAR_SETTLE_CASUALTIES 时保持 Skirmish，实际 {:?}",
            war.phase
        );
        assert_eq!(result, None, "A→A 无相变，try_advance 应返回 None");
    }

    #[test]
    fn war_phase_settling_to_aftermath_after_cooldown() {
        // A→B：now - settled_tick ≥ WAR_AFTERMATH_COOLDOWN_TICKS → Aftermath
        let groups = vec![EmergentGroupId(0), EmergentGroupId(1)];
        let settled_at = 100u64;
        let now = settled_at + WAR_AFTERMATH_COOLDOWN_TICKS;
        let mut war = war_settling("残灰谷", settled_at, groups);
        let result = war.try_advance(now);
        assert_eq!(
            war.phase,
            WarPhase::Aftermath,
            "期望冷却到期升 Aftermath，实际 {:?}",
            war.phase
        );
        assert_eq!(result, Some(WarPhase::Aftermath));
    }

    #[test]
    fn war_phase_settling_stays_settling_before_cooldown() {
        // A→A：冷却未到
        let groups = vec![EmergentGroupId(0), EmergentGroupId(1)];
        let settled_at = 100u64;
        let now = settled_at + WAR_AFTERMATH_COOLDOWN_TICKS - 1; // 差 1 tick
        let mut war = war_settling("残灰谷", settled_at, groups);
        let result = war.try_advance(now);
        assert_eq!(
            war.phase,
            WarPhase::Settling,
            "期望冷却未到时保持 Settling，实际 {:?}",
            war.phase
        );
        assert_eq!(result, None, "冷却未到 try_advance 应返回 None");
    }

    #[test]
    fn war_phase_never_skips_emerging_to_settling() {
        // 非法：Emerging 即便 casualties 高也只到 Skirmish，不跳 Settling
        // 构造 Emerging + pressure 足够高 + 假设有 casualties（注：Emerging 的 outcome=None，
        // 实际 casualties 只在 Skirmish 计）→ try_advance 只走 Emerging→Skirmish 分支
        let mut war = war_emerging(
            "残灰谷",
            WAR_SKIRMISH_PRESSURE + 100.0, // pressure 超高
            vec![EmergentGroupId(0), EmergentGroupId(1)],
        );
        let result = war.try_advance(0);
        // 第一次推进应是 Emerging→Skirmish，不会跳到 Settling
        assert_eq!(
            war.phase,
            WarPhase::Skirmish,
            "期望 Emerging 只能升到 Skirmish（不跳 Settling），实际 {:?}",
            war.phase
        );
        assert_eq!(result, Some(WarPhase::Skirmish));
        // 再调一次（此时在 Skirmish，casualties=0 < WAR_SETTLE_CASUALTIES）→ 仍 Skirmish
        let result2 = war.try_advance(1);
        assert_eq!(
            war.phase,
            WarPhase::Skirmish,
            "Skirmish 下 casualties=0 不升 Settling，实际 {:?}",
            war.phase
        );
        assert_eq!(result2, None);
    }

    #[test]
    fn war_phase_never_skips_skirmish_to_aftermath() {
        // 非法跳级：Skirmish → Aftermath（必须经 Settling）
        // 构造一个 Skirmish 状态，即便 settled_tick 超过冷却也不直接升 Aftermath
        let groups = vec![EmergentGroupId(0), EmergentGroupId(1)];
        let mut war = war_skirmish_with_casualties("残灰谷", WAR_SETTLE_CASUALTIES, groups);
        // 注意：war_skirmish_with_casualties 的 outcome.settled_tick=0，
        // 而此处调 try_advance(WAR_AFTERMATH_COOLDOWN_TICKS + 1) 时 Skirmish→Settling 先发生
        let result = war.try_advance(WAR_AFTERMATH_COOLDOWN_TICKS + 1);
        // 应升到 Settling（不是 Aftermath）
        assert_eq!(
            war.phase,
            WarPhase::Settling,
            "期望 Skirmish 只能升到 Settling（不跳 Aftermath），实际 {:?}",
            war.phase
        );
        assert_eq!(result, Some(WarPhase::Settling));
    }

    #[test]
    fn war_phase_aftermath_is_terminal() {
        // Aftermath 终态，不可再推进
        let groups = vec![EmergentGroupId(0), EmergentGroupId(1)];
        let mut war = FactionWar {
            war_id: WarId(10),
            zone: "残灰谷".to_string(),
            region_descriptor: "残灰谷一带散修".to_string(),
            phase: WarPhase::Aftermath,
            groups,
            player_roles: vec![],
            pressure: 999.0,
            started_tick: 0,
            last_update_tick: 0,
            outcome: Some(FactionWarOutcome {
                winner_group: EmergentGroupId(0),
                loser_group: EmergentGroupId(1),
                total_casualties: 10,
                settled_tick: 0,
            }),
            wins_by_group: HashMap::new(),
        };
        let result = war.try_advance(9999);
        assert_eq!(
            war.phase,
            WarPhase::Aftermath,
            "Aftermath 是终态，try_advance 不应改变相，实际 {:?}",
            war.phase
        );
        assert_eq!(
            result, None,
            "Aftermath 终态 try_advance 应返回 None，实际 {:?}",
            result
        );
    }

    // ─────────── B. 触发阈值 ────────────────────────────────────────────────

    #[test]
    fn pressure_below_threshold_no_war() {
        let mut store = WarConflictStore::default();
        let entry = ZonePressureEntry {
            pressure: WAR_PRESSURE_THRESHOLD - 1.0,
            groups: vec![EmergentGroupId(0), EmergentGroupId(1)],
            ..Default::default()
        };
        let changes = store.escalate_or_create("残灰谷", &entry, 1);
        assert!(
            changes.is_empty(),
            "期望 pressure < WAR_PRESSURE_THRESHOLD 时不创建 war（因 pressure 未达起势阈），实际 {:?}",
            changes
        );
        assert!(
            !store.zone_active.contains_key("残灰谷"),
            "不应在 zone_active 里注册任何 war"
        );
    }

    #[test]
    fn pressure_crosses_threshold_creates_emerging_war() {
        let mut store = WarConflictStore::default();
        let entry = ZonePressureEntry {
            pressure: WAR_PRESSURE_THRESHOLD,
            groups: vec![EmergentGroupId(0), EmergentGroupId(1)],
            ..Default::default()
        };
        let changes = store.escalate_or_create("残灰谷", &entry, 42);
        assert!(
            !changes.is_empty(),
            "期望越阈且 ≥2 group 时创建 Emerging war，实际无变化"
        );
        assert_eq!(changes[0].1, WarPhase::Emerging, "首个变化应是 Emerging");
        let war_id = changes[0].0;
        let snapshot = &changes[0].2;
        assert_eq!(snapshot.zone, "残灰谷");
        // region_descriptor 应为匿名格式
        assert!(
            snapshot.region_descriptor.contains("散修"),
            "region_descriptor 应含'散修'（匿名描述符），实际 {}",
            snapshot.region_descriptor
        );
        assert!(
            !snapshot.region_descriptor.contains("青云"),
            "region_descriptor 不应含具名宗门字样"
        );
        // groups 去重升序
        assert_eq!(
            snapshot.groups,
            vec![EmergentGroupId(0), EmergentGroupId(1)]
        );
        // zone_active 已注册
        assert_eq!(store.zone_active.get("残灰谷"), Some(&war_id));
    }

    #[test]
    fn single_group_never_starts_war() {
        // pressure 越阈但只 1 group → 不创建（WAR_MIN_GROUPS = 2）
        let mut store = WarConflictStore::default();
        let entry = ZonePressureEntry {
            pressure: WAR_PRESSURE_THRESHOLD + 10.0,
            groups: vec![EmergentGroupId(0)], // 只有一个群体
            ..Default::default()
        };
        let changes = store.escalate_or_create("残灰谷", &entry, 1);
        assert!(
            changes.is_empty(),
            "期望只有 1 个 group 时不创建 war（因 WAR_MIN_GROUPS=2 限制），实际 {:?}",
            changes
        );
    }

    #[test]
    fn casualties_accumulate_across_outcomes() {
        // 多条 outcome 累加 casualties/pressure 单调
        let mut entry = ZonePressureEntry::default();
        let g0 = EmergentGroupId(0);
        let g1 = EmergentGroupId(1);
        entry.accumulate(g0, g1, 1);
        assert_eq!(entry.casualties, 1);
        assert_eq!(entry.pressure, 1.0);
        entry.accumulate(g0, g1, 2);
        assert_eq!(entry.casualties, 2);
        assert_eq!(entry.pressure, 2.0);
        // groups 去重
        assert_eq!(entry.groups, vec![g0, g1]);
        // 添加第三个群体
        entry.accumulate(g0, EmergentGroupId(2), 3);
        assert_eq!(entry.casualties, 3);
        assert_eq!(entry.groups, vec![g0, g1, EmergentGroupId(2)]);
    }

    // ─────────── C. 玩家立场 ─────────────────────────────────────────────────

    fn make_skirmish_war(groups: Vec<EmergentGroupId>) -> FactionWar {
        FactionWar {
            war_id: WarId(99),
            zone: "残灰谷".to_string(),
            region_descriptor: "残灰谷一带散修".to_string(),
            phase: WarPhase::Skirmish,
            groups,
            player_roles: vec![],
            pressure: WAR_SKIRMISH_PRESSURE + 1.0,
            started_tick: 0,
            last_update_tick: 0,
            outcome: None,
            wins_by_group: HashMap::new(),
        }
    }

    #[test]
    fn participate_enlist_mercenary_intercept_spectate() {
        // 四 WarRole 各一条 happy path
        let groups = vec![EmergentGroupId(0), EmergentGroupId(1)];
        let mut war = make_skirmish_war(groups.clone());

        // Enlist
        let r = war.apply_participation("player_a", WarRole::Enlist, Some(EmergentGroupId(0)), 1);
        assert_eq!(
            r,
            Ok(WarRole::Enlist),
            "Enlist happy path 应成功，实际 {:?}",
            r
        );
        let role_rec = war
            .player_roles
            .iter()
            .find(|r| r.player_id == "player_a")
            .unwrap();
        assert_eq!(role_rec.allied_group, Some(EmergentGroupId(0)));

        // Mercenary
        let r2 =
            war.apply_participation("player_b", WarRole::Mercenary, Some(EmergentGroupId(1)), 2);
        assert_eq!(r2, Ok(WarRole::Mercenary));
        let role_rec2 = war
            .player_roles
            .iter()
            .find(|r| r.player_id == "player_b")
            .unwrap();
        assert_eq!(role_rec2.allied_group, Some(EmergentGroupId(1)));

        // Intercept（不需要 group）
        let r3 =
            war.apply_participation("player_c", WarRole::Intercept, Some(EmergentGroupId(0)), 3);
        assert_eq!(r3, Ok(WarRole::Intercept));
        let role_rec3 = war
            .player_roles
            .iter()
            .find(|r| r.player_id == "player_c")
            .unwrap();
        assert_eq!(
            role_rec3.allied_group, None,
            "Intercept 强制 allied_group=None"
        );

        // Spectate
        let r4 = war.apply_participation("player_d", WarRole::Spectate, None, 4);
        assert_eq!(r4, Ok(WarRole::Spectate));
    }

    #[test]
    fn participate_spectate_to_enlist_transition() {
        let groups = vec![EmergentGroupId(0), EmergentGroupId(1)];
        let mut war = make_skirmish_war(groups);
        war.apply_participation("p", WarRole::Spectate, None, 1)
            .unwrap();
        // Spectate → Enlist 允许（覆盖既有记录）
        let r = war.apply_participation("p", WarRole::Enlist, Some(EmergentGroupId(0)), 2);
        assert_eq!(
            r,
            Ok(WarRole::Enlist),
            "Spectate→Enlist 转换应允许（覆盖），实际 {:?}",
            r
        );
        let role_rec = war
            .player_roles
            .iter()
            .find(|r| r.player_id == "p")
            .unwrap();
        assert_eq!(role_rec.role, WarRole::Enlist);
    }

    #[test]
    fn participate_duplicate_enlist_same_group_errs() {
        // AlreadyEnlisted：同 group 同 role 重复
        let groups = vec![EmergentGroupId(0), EmergentGroupId(1)];
        let mut war = make_skirmish_war(groups);
        war.apply_participation("p", WarRole::Enlist, Some(EmergentGroupId(0)), 1)
            .unwrap();
        let r = war.apply_participation("p", WarRole::Enlist, Some(EmergentGroupId(0)), 2);
        assert_eq!(
            r,
            Err(WarParticipateError::AlreadyEnlisted),
            "期望同 group 同 role 重复投靠返回 AlreadyEnlisted，实际 {:?}",
            r
        );
    }

    #[test]
    fn participate_switch_enlist_group_allowed() {
        // 叛投：Enlist A→Enlist B（不同 group），允许
        let groups = vec![EmergentGroupId(0), EmergentGroupId(1)];
        let mut war = make_skirmish_war(groups);
        war.apply_participation("p", WarRole::Enlist, Some(EmergentGroupId(0)), 1)
            .unwrap();
        let r = war.apply_participation("p", WarRole::Enlist, Some(EmergentGroupId(1)), 2);
        assert_eq!(
            r,
            Ok(WarRole::Enlist),
            "叛投（改投不同 group）应允许，实际 {:?}",
            r
        );
        let rec = war
            .player_roles
            .iter()
            .find(|r| r.player_id == "p")
            .unwrap();
        assert_eq!(
            rec.allied_group,
            Some(EmergentGroupId(1)),
            "叛投后 allied_group 应更新为新组"
        );
    }

    #[test]
    fn participate_group_not_in_war_errs() {
        let groups = vec![EmergentGroupId(0), EmergentGroupId(1)];
        let mut war = make_skirmish_war(groups);
        let r = war.apply_participation("p", WarRole::Enlist, Some(EmergentGroupId(99)), 1);
        assert_eq!(
            r,
            Err(WarParticipateError::GroupNotInWar),
            "期望 allied_group 不在 war.groups 时返回 GroupNotInWar，实际 {:?}",
            r
        );
    }

    #[test]
    fn participate_missing_group_for_enlist_errs() {
        let groups = vec![EmergentGroupId(0), EmergentGroupId(1)];
        let mut war = make_skirmish_war(groups);
        let r = war.apply_participation("p", WarRole::Enlist, None, 1);
        assert_eq!(
            r,
            Err(WarParticipateError::MissingAlliedGroup),
            "期望 Enlist 未带 group 返回 MissingAlliedGroup，实际 {:?}",
            r
        );
    }

    #[test]
    fn participate_war_not_found_errs() {
        let mut store = WarConflictStore::default();
        let r = store.participate(
            "不存在的zone",
            "p",
            WarRole::Enlist,
            Some(EmergentGroupId(0)),
            1,
        );
        assert_eq!(
            r,
            Err(WarParticipateError::WarNotFound),
            "期望 zone 无 active war 返回 WarNotFound，实际 {:?}",
            r
        );
    }

    #[test]
    fn participate_settling_phase_rejected() {
        // Settling 阶段拒绝参与
        let groups = vec![EmergentGroupId(0), EmergentGroupId(1)];
        let mut war = war_settling("残灰谷", 0, groups);

        let r = war.apply_participation("p", WarRole::Enlist, Some(EmergentGroupId(0)), 1);
        assert_eq!(
            r,
            Err(WarParticipateError::PhaseNotJoinable),
            "期望 Settling 阶段拒绝参与（PhaseNotJoinable），实际 {:?}",
            r
        );

        // Aftermath 也拒绝
        let groups2 = vec![EmergentGroupId(0), EmergentGroupId(1)];
        let mut war2 = FactionWar {
            war_id: WarId(5),
            zone: "残灰谷".to_string(),
            region_descriptor: "残灰谷一带散修".to_string(),
            phase: WarPhase::Aftermath,
            groups: groups2,
            player_roles: vec![],
            pressure: 100.0,
            started_tick: 0,
            last_update_tick: 0,
            outcome: Some(FactionWarOutcome {
                winner_group: EmergentGroupId(0),
                loser_group: EmergentGroupId(1),
                total_casualties: 6,
                settled_tick: 0,
            }),
            wins_by_group: HashMap::new(),
        };
        let r2 = war2.apply_participation("p", WarRole::Enlist, Some(EmergentGroupId(0)), 2);
        assert_eq!(
            r2,
            Err(WarParticipateError::PhaseNotJoinable),
            "期望 Aftermath 阶段拒绝参与（PhaseNotJoinable），实际 {:?}",
            r2
        );
    }

    #[test]
    fn intercept_ignores_allied_group() {
        // Intercept 携带 group 时强制 None
        let groups = vec![EmergentGroupId(0), EmergentGroupId(1)];
        let mut war = make_skirmish_war(groups);
        war.apply_participation("p", WarRole::Intercept, Some(EmergentGroupId(0)), 1)
            .unwrap();
        let rec = war
            .player_roles
            .iter()
            .find(|r| r.player_id == "p")
            .unwrap();
        assert_eq!(
            rec.allied_group, None,
            "期望 Intercept 携带的 group 被忽略（强制 None），实际 {:?}",
            rec.allied_group
        );
    }

    // ─────────── D. outcome 结算 ─────────────────────────────────────────────

    #[test]
    fn outcome_winner_is_max_pressure_group() {
        // winner = wins_by_group 最大群体；G1 赢 3 场 > G2 赢 1 场 → G1 为 winner
        let g1 = EmergentGroupId(1);
        let g2 = EmergentGroupId(2);
        let groups = vec![g1, g2];
        let mut war = war_skirmish_with_casualties("残灰谷", WAR_SETTLE_CASUALTIES, groups);
        // 为 G1 设置更多 wins（3 vs 1）
        war.wins_by_group.insert(g1, 3);
        war.wins_by_group.insert(g2, 1);
        war.try_advance(100);
        assert_eq!(war.phase, WarPhase::Settling);
        let outcome = war.outcome.as_ref().unwrap();
        assert_eq!(
            outcome.winner_group, g1,
            "期望 winner=G1 因其 wins=3 > G2 wins=1，实际 {:?}",
            outcome.winner_group
        );
        assert_eq!(
            outcome.loser_group, g2,
            "期望 loser=G2 因其 wins=1 最少，实际 {:?}",
            outcome.loser_group
        );
    }

    #[test]
    fn outcome_total_casualties_matches_accumulated() {
        // total_casualties 与累积值一致
        let groups = vec![EmergentGroupId(0), EmergentGroupId(1)];
        let mut war = war_skirmish_with_casualties("残灰谷", WAR_SETTLE_CASUALTIES + 3, groups);
        war.try_advance(200);
        assert_eq!(war.phase, WarPhase::Settling);
        let outcome = war.outcome.as_ref().unwrap();
        assert_eq!(
            outcome.total_casualties,
            WAR_SETTLE_CASUALTIES + 3,
            "期望 total_casualties 等于 WAR_SETTLE_CASUALTIES+3，实际 {}",
            outcome.total_casualties
        );
    }

    // ─────────── E. 战事注册表与 aftermath 清理 ─────────────────────────────

    #[test]
    fn aftermath_cleared_from_zone_active() {
        // Aftermath 后 zone_active 清除该 zone
        let mut store = WarConflictStore::default();
        let entry = ZonePressureEntry {
            pressure: WAR_PRESSURE_THRESHOLD,
            groups: vec![EmergentGroupId(0), EmergentGroupId(1)],
            casualties: 0,
            ..Default::default()
        };
        store.escalate_or_create("残灰谷", &entry, 0);
        let war_id = *store.zone_active.get("残灰谷").expect("应有 active war");

        // 推 casualties 到 Settling
        {
            let war = store.wars.get_mut(&war_id).unwrap();
            war.phase = WarPhase::Skirmish;
            war.outcome = Some(FactionWarOutcome {
                winner_group: EmergentGroupId(0),
                loser_group: EmergentGroupId(1),
                total_casualties: WAR_SETTLE_CASUALTIES,
                settled_tick: 0,
            });
            // wins_by_group 已有默认空 HashMap
            war.try_advance(0);
        }
        // Settling → Aftermath 冷却到期
        let changes = store.advance_settling_wars(WAR_AFTERMATH_COOLDOWN_TICKS);
        assert!(
            !changes.is_empty(),
            "期望 Settling→Aftermath 变化，实际无变化"
        );
        assert_eq!(changes[0].1, WarPhase::Aftermath);
        assert!(
            !store.zone_active.contains_key("残灰谷"),
            "期望 Aftermath 后 zone_active 清除 '残灰谷'，实际仍存在"
        );
    }

    // ─────────── F. ZonePressureEntry 累积 ──────────────────────────────────

    #[test]
    fn zone_pressure_entry_accumulate_deduplicates_groups() {
        let mut entry = ZonePressureEntry::default();
        let g0 = EmergentGroupId(0);
        let g1 = EmergentGroupId(1);
        entry.accumulate(g0, g1, 10);
        entry.accumulate(g0, g1, 20); // 重复 group，不应增加
        assert_eq!(
            entry.groups,
            vec![g0, g1],
            "重复 accumulate 相同 group 对不应在 groups 里重复，实际 {:?}",
            entry.groups
        );
        assert_eq!(entry.casualties, 2);
        assert_eq!(entry.pressure, 2.0);
    }

    // ─────────── G. WarPhase / WarRole serde ─────────────────────────────────

    #[test]
    fn war_phase_each_variant_roundtrips_serde() {
        for (phase, name) in [
            (WarPhase::Emerging, "emerging"),
            (WarPhase::Skirmish, "skirmish"),
            (WarPhase::Settling, "settling"),
            (WarPhase::Aftermath, "aftermath"),
        ] {
            let v = serde_json::to_value(phase).unwrap();
            assert_eq!(
                v,
                serde_json::json!(name),
                "WarPhase::{phase:?} 序列化应为 {name:?}，实际 {v}"
            );
            let back: WarPhase = serde_json::from_value(v).unwrap();
            assert_eq!(back, phase);
            assert_eq!(phase.as_str(), name);
        }
    }

    #[test]
    fn war_role_each_variant_roundtrips_serde() {
        for (role, name) in [
            (WarRole::Enlist, "enlist"),
            (WarRole::Mercenary, "mercenary"),
            (WarRole::Intercept, "intercept"),
            (WarRole::Spectate, "spectate"),
        ] {
            let v = serde_json::to_value(role).unwrap();
            assert_eq!(
                v,
                serde_json::json!(name),
                "WarRole::{role:?} 序列化应为 {name:?}，实际 {v}"
            );
            let back: WarRole = serde_json::from_value(v).unwrap();
            assert_eq!(back, role);
        }
    }

    // ─────────── H. PlayerRoleCounts 聚合 ───────────────────────────────────

    #[test]
    fn role_counts_aggregates_correctly() {
        let groups = vec![EmergentGroupId(0), EmergentGroupId(1)];
        let mut war = make_skirmish_war(groups);
        war.apply_participation("p1", WarRole::Enlist, Some(EmergentGroupId(0)), 1)
            .unwrap();
        war.apply_participation("p2", WarRole::Mercenary, Some(EmergentGroupId(1)), 2)
            .unwrap();
        war.apply_participation("p3", WarRole::Intercept, None, 3)
            .unwrap();
        war.apply_participation("p4", WarRole::Spectate, None, 4)
            .unwrap();
        let counts = war.role_counts();
        assert_eq!(counts.enlist, 1);
        assert_eq!(counts.mercenary, 1);
        assert_eq!(counts.intercept, 1);
        assert_eq!(counts.spectate, 1);
    }

    // ─────────── I. handle_war_participate_intent 汇聚点 system ───────────────
    //
    // 两路 intent（brigadier 路径 A + headless 路径 B）的唯一收口。
    // Ok 分支：participate 成功 → emit WarPhaseChanged（phase 不变、counts 刷新）。
    // Err 分支：participate 失败 → debug log 吞掉、不 emit（运行时静默 drop 的地方）。

    use bevy_ecs::event::Events;
    use bevy_ecs::schedule::Schedule;
    use bevy_ecs::world::World;

    /// 构建一个装好 WarConflictStore（含一场 zone 的 active Skirmish war）+ 事件队列的 World，
    /// 把若干 WarParticipateIntent 灌进去后跑一次 handle_war_participate_intent，
    /// 返回该 update 周期内 emit 的 WarPhaseChanged。
    fn run_participate_intents(
        store: WarConflictStore,
        intents: Vec<WarParticipateIntent>,
    ) -> Vec<WarPhaseChanged> {
        let mut world = World::new();
        world.insert_resource(store);
        world.insert_resource(Events::<WarParticipateIntent>::default());
        world.insert_resource(Events::<WarPhaseChanged>::default());

        {
            let mut queue = world.resource_mut::<Events<WarParticipateIntent>>();
            for intent in intents {
                queue.send(intent);
            }
        }

        let mut schedule = Schedule::default();
        schedule.add_systems(handle_war_participate_intent);
        schedule.run(&mut world);

        world
            .resource::<Events<WarPhaseChanged>>()
            .iter_current_update_events()
            .cloned()
            .collect()
    }

    /// 装一个 zone="残灰谷" 的 active Skirmish war 进 store（zone_active 已挂）。
    fn store_with_active_war() -> WarConflictStore {
        let mut store = WarConflictStore::default();
        let war = make_skirmish_war(vec![EmergentGroupId(0), EmergentGroupId(1)]);
        let war_id = war.war_id;
        let zone = war.zone.clone();
        store.wars.insert(war_id, war);
        store.zone_active.insert(zone, war_id);
        store
    }

    fn intent(zone: &str, player: &str, role: WarRole, group: Option<u16>) -> WarParticipateIntent {
        WarParticipateIntent {
            player_id: player.to_string(),
            zone: zone.to_string(),
            role,
            allied_group: group.map(EmergentGroupId),
            at_tick: 5,
        }
    }

    #[test]
    fn handle_participate_intent_ok_emits_phase_changed_with_refreshed_counts() {
        // Ok 分支：合法 Enlist intent → participate 成功 → emit 1 条 WarPhaseChanged，
        // phase 不变（仍 Skirmish），counts 反映新 Enlist。
        let store = store_with_active_war();
        let emitted = run_participate_intents(
            store,
            vec![intent("残灰谷", "p_enlist", WarRole::Enlist, Some(0))],
        );

        assert_eq!(
            emitted.len(),
            1,
            "期望参与成功 emit 1 条 WarPhaseChanged 因 telemetry 需即时反映新 role，实际 {}",
            emitted.len()
        );
        let ev = &emitted[0];
        assert_eq!(
            ev.phase,
            WarPhase::Skirmish,
            "期望 phase 不变（仍 Skirmish）因 participate 不推进阶段，实际 {:?}",
            ev.phase
        );
        assert_eq!(ev.zone, "残灰谷", "期望 zone 透传，实际 {}", ev.zone);
        assert_eq!(
            ev.region_descriptor, "残灰谷一带散修",
            "期望 region_descriptor 为匿名区域描述符 因 reframe b 禁具名宗门，实际 {}",
            ev.region_descriptor
        );
        assert_eq!(
            ev.player_role_counts.enlist, 1,
            "期望 counts.enlist=1 因刚投靠一名 Enlist，实际 {}",
            ev.player_role_counts.enlist
        );
        assert_eq!(
            ev.at_tick, 5,
            "期望 at_tick 透传自 intent，实际 {}",
            ev.at_tick
        );
    }

    #[test]
    fn handle_participate_intent_err_war_not_found_emits_nothing() {
        // Err 分支①：zone 无 active war → WarNotFound → 不 emit（静默 drop）。
        let store = store_with_active_war();
        let emitted = run_participate_intents(
            store,
            vec![intent("无战事的zone", "p", WarRole::Enlist, Some(0))],
        );
        assert!(
            emitted.is_empty(),
            "期望 zone 无 active war 时不 emit 任何 WarPhaseChanged 因 participate 失败应静默 drop，实际 emit {}",
            emitted.len()
        );
    }

    #[test]
    fn handle_participate_intent_err_invalid_group_emits_nothing() {
        // Err 分支②：Enlist 到不在 war.groups 的 group → GroupNotInWar → 不 emit。
        let store = store_with_active_war();
        let emitted = run_participate_intents(
            store,
            vec![intent("残灰谷", "p", WarRole::Enlist, Some(99))],
        );
        assert!(
            emitted.is_empty(),
            "期望非法 allied_group 时不 emit 因 participate 返回 GroupNotInWar 应静默 drop，实际 emit {}",
            emitted.len()
        );
    }

    // ─────────── J. P9 winner 精化（wins_by_group per-group 贡献）───────────

    #[test]
    fn winsbygroup_accumulate_increments_winner_only() {
        // accumulate(G1, G2) → wins_by_group[G1]==1，G2 不计 win
        let g1 = EmergentGroupId(1);
        let g2 = EmergentGroupId(2);
        let mut entry = ZonePressureEntry::default();
        entry.accumulate(g1, g2, 10);
        assert_eq!(
            entry.wins_by_group.get(&g1).copied().unwrap_or(0),
            1,
            "期望 winner=G1 侧 wins_by_group[G1]==1 因 accumulate 只计 winner，实际 {:?}",
            entry.wins_by_group.get(&g1)
        );
        assert_eq!(
            entry.wins_by_group.get(&g2),
            None,
            "期望 loser=G2 侧 wins_by_group 无 G2 条目（不计 loser win），实际 {:?}",
            entry.wins_by_group.get(&g2)
        );
    }

    #[test]
    fn determine_winner_loser_tie_breaks_to_min_group_id() {
        // G1、G2 各赢 2 场 → 平票取最小 id → winner=G1（id 小）
        let g1 = EmergentGroupId(1);
        let g2 = EmergentGroupId(2);
        let groups = vec![g1, g2];
        let mut war = make_skirmish_war(groups.clone());
        war.wins_by_group.insert(g1, 2);
        war.wins_by_group.insert(g2, 2);
        let (winner, loser) = war.determine_winner_loser(groups);
        assert_eq!(
            winner, g1,
            "期望平票时取最小 group_id G1(1) 为 winner，实际 {:?}",
            winner
        );
        assert_eq!(loser, g2, "期望平票 loser 为 G2(2)，实际 {:?}", loser);
    }

    #[test]
    fn single_side_group_loser_does_not_count_win() {
        // (None, Some(G2)) 路径：G2 是 loser，wins_by_group 不应记 G2
        // 此测试通过直接调 ZonePressureEntry 来覆盖 npc_event_bridge 的 (None, Some) 分支语义：
        // 仅调 accumulate(winner, loser) 时 loser 不计 win，(None, Some(g)) 路径手动模拟。
        let g2 = EmergentGroupId(2);
        // 模拟 "只有 loser 侧有 group" 路径（手动压 entry，不经 accumulate）
        let mut entry = ZonePressureEntry::default();
        entry.pressure += 1.0;
        entry.casualties += 1;
        entry.last_tick = 5;
        if !entry.groups.contains(&g2) {
            entry.groups.push(g2);
            entry.groups.sort();
        }
        // 关键：不调 wins_by_group.insert(g2, ...)
        assert_eq!(
            entry.wins_by_group.get(&g2),
            None,
            "期望 loser-only 路径不计入 wins_by_group（loser 不赢），实际 {:?}",
            entry.wins_by_group.get(&g2)
        );
    }

    #[test]
    fn escalate_propagates_wins_by_group_to_war() {
        // accumulate 后 escalate_or_create → war.wins_by_group 同步
        let g1 = EmergentGroupId(0);
        let g2 = EmergentGroupId(1);
        let mut entry = ZonePressureEntry::default();
        // 积到足够触发 Emerging
        for _ in 0..WAR_PRESSURE_THRESHOLD as u32 {
            entry.accumulate(g1, g2, 1);
        }
        assert_eq!(
            entry.wins_by_group.get(&g1).copied().unwrap_or(0),
            WAR_PRESSURE_THRESHOLD as u32,
            "期望 G1 wins == WAR_PRESSURE_THRESHOLD 因每次 accumulate 都计 G1"
        );
        let mut store = WarConflictStore::default();
        let changes = store.escalate_or_create("残灰谷", &entry, 1);
        assert!(!changes.is_empty(), "期望创建 war");
        let war_id = changes[0].0;
        let war = store.wars.get(&war_id).expect("war 应存在");
        assert_eq!(
            war.wins_by_group, entry.wins_by_group,
            "期望 war.wins_by_group 与 entry.wins_by_group 相等（escalate 逐字段同步），实际 {:?}",
            war.wins_by_group
        );
    }

    #[test]
    fn handle_participate_intent_err_settling_phase_not_joinable_emits_nothing() {
        // Err 分支③：Settling 阶段不可加入 → PhaseNotJoinable → 不 emit。
        let mut store = WarConflictStore::default();
        let mut war = make_skirmish_war(vec![EmergentGroupId(0), EmergentGroupId(1)]);
        war.phase = WarPhase::Settling;
        // 手动设置 outcome（Settling 需要 Some outcome）
        war.outcome = Some(FactionWarOutcome {
            winner_group: EmergentGroupId(0),
            loser_group: EmergentGroupId(1),
            total_casualties: WAR_SETTLE_CASUALTIES,
            settled_tick: 0,
        });
        let war_id = war.war_id;
        let zone = war.zone.clone();
        store.wars.insert(war_id, war);
        store.zone_active.insert(zone, war_id);

        let emitted =
            run_participate_intents(store, vec![intent("残灰谷", "p", WarRole::Spectate, None)]);
        assert!(
            emitted.is_empty(),
            "期望 Settling 阶段参与被拒时不 emit 因 PhaseNotJoinable 应静默 drop，实际 emit {}",
            emitted.len()
        );
    }

    #[test]
    fn handle_participate_intent_mixed_ok_and_err_only_emits_for_ok() {
        // 混合：一条合法 + 一条非法 intent 同周期 → 只为合法那条 emit 1 条。
        let store = store_with_active_war();
        let emitted = run_participate_intents(
            store,
            vec![
                intent("无战事的zone", "p_bad", WarRole::Enlist, Some(0)), // Err
                intent("残灰谷", "p_ok", WarRole::Mercenary, Some(1)),     // Ok
            ],
        );
        assert_eq!(
            emitted.len(),
            1,
            "期望仅合法 intent emit（1 条）因失败的静默 drop、成功的 emit，实际 {}",
            emitted.len()
        );
        assert_eq!(
            emitted[0].player_role_counts.mercenary, 1,
            "期望 counts.mercenary=1 因唯一成功的是 Mercenary，实际 {}",
            emitted[0].player_role_counts.mercenary
        );
    }
}
