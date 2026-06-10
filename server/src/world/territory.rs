//! plan-territory-v1 P0 — 区域影响力系统（ZoneInfluence）。
//!
//! 按玩家在 Zone 内「停留 / 修炼 / 战斗」三源累积影响力，支持持久化（SQLite），
//! 计算区域霸主（ZoneDominance），并 emit InfluenceChangedEvent。
//!
//! ## 系统接线
//! - `territory_tick` 挂 `Update`，节流 `TERRITORY_EVAL_INTERVAL_TICKS`（约 60s，可调）。
//! - 进料全部复用已落地 API：
//!   - `ZoneRegistry::find_zone`（zone.rs:265）
//!   - `CultivationClock`（tick.rs:34）
//!   - `CultivationSessionPracticeAccumulator::is_recently_practicing`（tick.rs:50）
//!   - `CombatState.in_combat_until_tick`（components.rs:148）
//!   - `TiandaoAttention.level`（tiandao_hunt.rs:51）
//!   - `Renown.fame`（social/components.rs:151）
//!   - `canonical_player_id`（player/state.rs:304）

use std::collections::HashMap;

use valence::prelude::{
    bevy_ecs, App, Client, Entity, Event, EventReader, EventWriter, Position, Query, Res, ResMut,
    Resource, Update, Username, With,
};

use crate::combat::components::CombatState;
use crate::combat::events::DeathEvent;
use crate::cultivation::components::Cultivation;
use crate::cultivation::tick::{CultivationClock, CultivationSessionPracticeAccumulator};
use crate::player::state::canonical_player_id;
use crate::social::components::Renown;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::territory_narration::{DominanceChangedEvent, DominanceEventKind};
use crate::world::territory_rumor::RealmBand;
use crate::world::tiandao_hunt::{realm_rank, TiandaoAttention};
use crate::world::zone::ZoneRegistry;

// ─── 常量（可调）─────────────────────────────────────────────────────────────

/// 每次 eval 间隔 ticks。plan 文档标 60s，1s = 20 ticks。可调。
pub const TERRITORY_EVAL_INTERVAL_TICKS: u64 = 60 * 20;

/// 24 小时对应的 ticks（用于快速衰减判断）。
const TICKS_PER_DAY: u64 = 24 * 60 * 60 * 20;

/// 每 eval 周期基础停留增量（可调）。plan §「打坐中 +0.3/分钟」，折算 per-eval-60s。
pub const BASE_DWELL_PER_EVAL: f64 = 0.3;

/// 修炼乘子（can调）。
pub const PRACTICE_MULT: f64 = 1.5;

/// 战斗乘子（可调）。
pub const COMBAT_MULT: f64 = 1.2;

/// 离开 zone 每 eval 衰减量（per-60s）（可调）。
pub const DECAY_PER_EVAL: f64 = 0.5;

/// 超过 24h 未活动，衰减乘子×3（可调）。
pub const DECAY_LONG_ABSENCE_MULT: f64 = 3.0;

/// 影响力上限（可调）。
pub const INFLUENCE_MAX: f64 = 100.0;

/// 霸主候选门槛（可调）。
pub const DOMINANCE_THRESHOLD: f64 = 30.0;

/// 霸主领先第二名须达到的优势（可调）。
pub const DOMINANCE_LEAD: f64 = 10.0;

/// 现任霸主 influence 低于此值则失去地位（可调）。
pub const DOMINANCE_DROP: f64 = 20.0;

/// plan-territory-v1 P2 — PvP 击杀后胜者获得的影响力增量（可调）。
pub const KILL_INFLUENCE_GAIN: f64 = 5.0;

/// TiandaoAttention.level 最大值（与 tiandao_hunt.rs ATTENTION_MAX 一致）。
const ATTENTION_MAX: f64 = 100.0;

// ─── 类型定义 ─────────────────────────────────────────────────────────────────

/// 影响力来源枚举。用于 InfluenceChangedEvent.source 契约级断言。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfluenceSource {
    Dwell,
    Practice,
    Combat,
    Decay,
}

/// 影响力变动事件。
///
/// **consumer: P1** NPC 态度反应（npc::brain 读 InfluenceChangedEvent 调整对霸主态度）。
/// **consumer: P3** narration 广播（network::redis_bridge → bong:agent_narrate
/// "青云残峰换主了"）。P0 仅 emit，下游 reader 在 P1/P3 接入。
///
/// Bevy Event 双缓冲机制：无 reader 时事件每帧自动 drain 丢弃，不 panic、不泄漏。
/// 既有先例：era.rs EraChangedEvent 等 add_event 后跨阶段才接 reader。
// P0 无 consumer，字段由 P1/P3 读取，暂时 allow dead_code。
#[allow(dead_code)]
#[derive(Debug, Clone, Event)]
pub struct InfluenceChangedEvent {
    pub player_entity: Entity,
    /// canonical_player_id（跨 session 稳定字符串键）。
    pub player_id: String,
    pub zone_name: String,
    /// 正=增益，负=衰减。
    pub delta: f64,
    pub new_total: f64,
    pub source: InfluenceSource,
}

/// 区域霸主状态。
#[derive(Debug, Clone, PartialEq)]
pub struct ZoneDominance {
    pub char_id: String,
    pub influence: f64,
    pub established_tick: u64,
    /// 是否已向其他玩家公开。public_known 传播延迟属 P3。P0 初始为 false。
    pub public_known: bool,
    /// plan-territory-v1 P3 — 匿名境界段（Low/Mid/High）。
    /// 确立时从霸主 Cultivation.realm 映射，None 表示尚未记录（P0 存量）。
    pub realm_band: Option<RealmBand>,
}

/// 按玩家累计影响力来源细项。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InfluenceSources {
    pub meditation_ticks: u64,
    /// P0 保留=0；接口先于实现锁定。事件级胜负/击杀归属判定标 P1。
    pub combat_wins: u32,
    /// P0 保留=0；接口先于实现锁定。DeathEvent 归属判定属博弈层，标 P1。
    pub player_kills: u32,
    pub gather_count: u32,
    pub continuous_sessions: u32,
}

/// 单玩家在某区域的累计影响力记录。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlayerInfluence {
    pub value: f64,
    pub last_activity_tick: u64,
    pub source_breakdown: InfluenceSources,
}

/// 单区域的影响力条目（含所有玩家和当前霸主）。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ZoneInfluenceEntry {
    /// key = canonical_player_id。跨 session 稳定。
    pub players: HashMap<String, PlayerInfluence>,
    pub dominant: Option<ZoneDominance>,
}

/// 全局区域影响力 Resource。key = zone.name（与 ZoneRegistry 对齐）。
#[derive(Debug, Default)]
pub struct ZoneInfluenceMap {
    pub zones: HashMap<String, ZoneInfluenceEntry>,
}

impl Resource for ZoneInfluenceMap {}

// ─── 纯函数 ───────────────────────────────────────────────────────────────────

/// 计算本次 eval 的影响力 delta（纯函数，可单测）。
///
/// # 参数
/// - `realm_rank_val` — 0..=5（醒灵=0 … 化虚=5），由 `tiandao_hunt::realm_rank` 提供。
/// - `is_practicing` — `CultivationSessionPracticeAccumulator::is_recently_practicing`。
/// - `in_combat` — `CombatState.in_combat_until_tick > now`。
/// - `zone_spirit_qi` — `Zone.spirit_qi`，clamp [0,1]（worldview §十：废地无领地价值）。
/// - `tiandao_level` — `TiandaoAttention.level`，0..=100（驻守越被天道盯越显眼）。
/// - `renown_fame` — `Renown.fame`。
pub fn compute_influence_delta(
    realm_rank_val: u8,
    is_practicing: bool,
    in_combat: bool,
    zone_spirit_qi: f64,
    tiandao_level: f64,
    renown_fame: i32,
) -> f64 {
    // 基础停留量
    let mut delta = BASE_DWELL_PER_EVAL;

    // 修炼乘子（乘全体 delta，非叠加）
    if is_practicing {
        delta *= PRACTICE_MULT;
    }

    // 战斗乘子
    if in_combat {
        delta *= COMBAT_MULT;
    }

    // 境界乘子：(rank+1) 倍，醒灵×1 … 化虚×6
    let realm_multiplier = f64::from(realm_rank_val + 1);
    delta *= realm_multiplier;

    // TiandaoAttention 驻守加速：最多 +50%。被天道盯得越紧，领地争夺越显眼（可调）。
    let attention_ratio = (tiandao_level / ATTENTION_MAX).clamp(0.0, 1.0);
    delta *= 1.0 + 0.5 * attention_ratio;

    // Renown 声望分档（可调）。
    let renown_mult = if renown_fame > 500 {
        1.3
    } else if renown_fame > 100 {
        1.1
    } else {
        1.0
    };
    delta *= renown_mult;

    // worldview §十 灵气零和：废地/负灵域 spirit_qi≤0 → 累积归零；
    // 正常地 spirit_qi clamp 到 [0,1]。
    let qi_factor = zone_spirit_qi.clamp(0.0, 1.0);
    delta *= qi_factor;

    delta
}

/// 重算区域霸主（纯函数，可单测）。
///
/// 判定规则：
/// - 候选门槛 ≥ DOMINANCE_THRESHOLD(30.0)。
/// - 候选中取 influence 最高；领先第二名 ≥ DOMINANCE_LEAD(10.0) 才确认。
/// - 现任霸主 influence < DOMINANCE_DROP(20.0) → 失去地位。
/// - 平局（差 < LEAD）→ 维持现任 / 无霸主（不切换，防抖动）。
///
/// `new_top_realm_band` — 新霸主候选的境界段（P3），在新确立时写入 ZoneDominance.realm_band。
/// 若为 None，则新霸主 realm_band 也为 None（P0 存量兼容）。
pub fn recompute_dominance(
    entry: &ZoneInfluenceEntry,
    now: u64,
    new_top_realm_band: Option<RealmBand>,
) -> Option<ZoneDominance> {
    // 找候选（influence ≥ 门槛）
    let mut candidates: Vec<(&str, f64)> = entry
        .players
        .iter()
        .filter(|(_, p)| p.value >= DOMINANCE_THRESHOLD)
        .map(|(id, p)| (id.as_str(), p.value))
        .collect();

    if candidates.is_empty() {
        return None;
    }

    // 降序排序
    candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let (top_id, top_val) = candidates[0];
    let second_val = candidates.get(1).map(|(_, v)| *v).unwrap_or(0.0);

    // 滞后防抖：现任霸主 influence 低于 DROP 阈值 → 清除
    if let Some(current) = &entry.dominant {
        if current.char_id == top_id {
            if top_val < DOMINANCE_DROP {
                return None;
            }
            // 现任霸主仍满足条件，维持（即使无领先优势也不切换）
            return Some(ZoneDominance {
                char_id: current.char_id.clone(),
                influence: top_val,
                established_tick: current.established_tick,
                public_known: current.public_known,
                realm_band: current.realm_band,
            });
        }
        // 现任霸主被另一人超越
        let current_val = entry
            .players
            .get(&current.char_id)
            .map(|p| p.value)
            .unwrap_or(0.0);

        if current_val < DOMINANCE_DROP {
            // 现任跌破 DROP → 可以换主，检查新候选是否满足领先要求
        } else {
            // 现任仍有一定势力，平局/未达领先优势 → 维持现任（防抖）
            if top_val - current_val < DOMINANCE_LEAD {
                return Some(ZoneDominance {
                    char_id: current.char_id.clone(),
                    influence: current_val,
                    established_tick: current.established_tick,
                    public_known: current.public_known,
                    realm_band: current.realm_band,
                });
            }
        }
    }

    // 新霸主判定：需领先第二名 ≥ LEAD
    if top_val - second_val >= DOMINANCE_LEAD {
        Some(ZoneDominance {
            char_id: top_id.to_string(),
            influence: top_val,
            established_tick: now,
            public_known: false,
            realm_band: new_top_realm_band,
        })
    } else {
        // 平局 → 无明确霸主
        None
    }
}

// ─── 查询类型别名 ──────────────────────────────────────────────────────────────

/// P2 PvP system 查 victim/killer 位置+dimension+用户名。
type PvpParticipantQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        Option<&'static CurrentDimension>,
        Option<&'static Username>,
    ),
    With<Client>,
>;

type TerritoryPlayerQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Cultivation,
        &'static Position,
        &'static Username,
        Option<&'static CurrentDimension>,
        Option<&'static CombatState>,
        Option<&'static TiandaoAttention>,
        Option<&'static Renown>,
    ),
    With<Client>,
>;

// ─── System ───────────────────────────────────────────────────────────────────

/// 区域影响力 tick system。挂 Update，节流 `TERRITORY_EVAL_INTERVAL_TICKS`（60s 可调）。
///
/// 三源累积：停留(Dwell) / 修炼(Practice) / 战斗(Combat)。
/// 离开 zone → 旧 zone 衰减。>24h 未回 → 衰减 ×3。
pub fn territory_tick(
    clock: Option<Res<CultivationClock>>,
    zones: Option<Res<ZoneRegistry>>,
    practice_accumulator: Option<Res<CultivationSessionPracticeAccumulator>>,
    mut influence_map: ResMut<ZoneInfluenceMap>,
    mut ev_writer: EventWriter<InfluenceChangedEvent>,
    mut dominance_ev_writer: EventWriter<DominanceChangedEvent>,
    players: TerritoryPlayerQuery<'_, '_>,
) {
    let Some(clock) = clock else {
        return;
    };
    let now_tick = clock.tick;
    let Some(zone_registry) = zones else {
        return;
    };

    // 收集本 tick 变动的 zone 名，用于末尾重算 dominance
    let mut dirty_zones: std::collections::HashSet<String> = Default::default();

    // 先处理在线玩家（可能累积，也可能切 zone 需要对旧 zone 衰减）
    for (entity, cultivation, position, username, dimension, combat, attention, renown) in
        players.iter()
    {
        let player_id = canonical_player_id(username.0.as_str());

        // 使用 tiandao_hunt 的节流函数，但节流间隔改用 TERRITORY_EVAL_INTERVAL_TICKS
        // 通过检查玩家在各 zone 的 last_activity_tick 来节流
        // 简化：直接用 now_tick 与 TERRITORY_EVAL_INTERVAL_TICKS 检查
        // 先看此玩家是否已到 eval 间隔（取任一已记录 zone 的 last_activity_tick）
        let last_eval = influence_map
            .zones
            .values()
            .filter_map(|entry| entry.players.get(&player_id))
            .map(|p| p.last_activity_tick)
            .max()
            .unwrap_or(0);

        // 对于全新玩家（没有任何记录）在 now_tick=0 时仍要处理
        if last_eval > 0 && !should_eval_territory(now_tick, last_eval) {
            continue;
        }

        let dim = dimension.map(|d| d.0).unwrap_or(DimensionKind::Overworld);
        let current_zone = zone_registry
            .find_zone(dim, position.0)
            .map(|z| z.name.clone());

        let is_practicing = practice_accumulator
            .as_deref()
            .map(|acc| acc.is_recently_practicing(entity, now_tick))
            .unwrap_or(false);

        let in_combat = combat
            .and_then(|c| c.in_combat_until_tick)
            .is_some_and(|until| until > now_tick);

        let attention_level = attention.map(|a| a.level).unwrap_or(0.0);
        let fame = renown.map(|r| r.fame).unwrap_or(0);

        // 处理旧 zone 衰减：找出该玩家有记录但当前不在的 zone
        let zones_with_player: Vec<String> = influence_map
            .zones
            .iter()
            .filter(|(_, entry)| entry.players.contains_key(&player_id))
            .map(|(name, _)| name.clone())
            .collect();

        for zone_name in &zones_with_player {
            let is_current = current_zone.as_deref() == Some(zone_name.as_str());
            if is_current {
                continue;
            }
            // 玩家不在此 zone → 衰减
            let entry = influence_map.zones.entry(zone_name.clone()).or_default();
            let player_inf = entry.players.entry(player_id.clone()).or_default();

            let absence_ticks = now_tick.saturating_sub(player_inf.last_activity_tick);
            let old_val = player_inf.value;
            player_inf.value = compute_decay(player_inf.value, absence_ticks);
            let delta = player_inf.value - old_val;
            let new_total = player_inf.value;

            if delta.abs() > 1e-12 {
                dirty_zones.insert(zone_name.clone());
                ev_writer.send(InfluenceChangedEvent {
                    player_entity: entity,
                    player_id: player_id.clone(),
                    zone_name: zone_name.clone(),
                    delta,
                    new_total,
                    source: InfluenceSource::Decay,
                });
            }
        }

        // 累积当前 zone
        let Some(zone_name) = current_zone else {
            continue;
        };

        // 读取 zone 的 spirit_qi
        let zone_spirit_qi = zone_registry
            .find_zone(dim, position.0)
            .map(|z| z.spirit_qi)
            .unwrap_or(0.0);

        let realm_rank_val = realm_rank(cultivation.realm);
        let delta = compute_influence_delta(
            realm_rank_val,
            is_practicing,
            in_combat,
            zone_spirit_qi,
            attention_level,
            fame,
        );

        let source = if in_combat {
            InfluenceSource::Combat
        } else if is_practicing {
            InfluenceSource::Practice
        } else {
            InfluenceSource::Dwell
        };

        let entry = influence_map.zones.entry(zone_name.clone()).or_default();
        let player_inf = entry.players.entry(player_id.clone()).or_default();
        let old_val = player_inf.value;
        player_inf.value = (player_inf.value + delta).clamp(0.0, INFLUENCE_MAX);
        player_inf.last_activity_tick = now_tick;

        // 更新 source_breakdown（meditation_ticks 对应修炼或停留）
        player_inf.source_breakdown.meditation_ticks = player_inf
            .source_breakdown
            .meditation_ticks
            .saturating_add(1);

        let actual_delta = player_inf.value - old_val;
        let new_total = player_inf.value;

        if actual_delta.abs() > 1e-12 {
            dirty_zones.insert(zone_name.clone());
            ev_writer.send(InfluenceChangedEvent {
                player_entity: entity,
                player_id: player_id.clone(),
                zone_name: zone_name.clone(),
                delta: actual_delta,
                new_total,
                source,
            });
        }
    }

    // 对本 tick 有变动的 zone 重算 dominance，并 emit DominanceChangedEvent
    for zone_name in &dirty_zones {
        {
            // 用 players 中任意玩家作为 realm_band 取 token（实际取 top player）
            let entry = influence_map.zones.entry(zone_name.clone()).or_default();

            // 找候选中 influence 最高的 player id，查其境界段
            let top_id = entry
                .players
                .iter()
                .max_by(|a, b| {
                    a.1.value
                        .partial_cmp(&b.1.value)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(id, _)| id.clone());

            let new_realm_band = top_id.as_deref().and_then(|tid| {
                players
                    .iter()
                    .find_map(|(_, cultivation, _, username, ..)| {
                        let pid = canonical_player_id(username.0.as_str());
                        if pid == tid {
                            Some(RealmBand::from_realm(cultivation.realm))
                        } else {
                            None
                        }
                    })
            });

            let old_dominant = entry.dominant.clone();
            let new_dominant = recompute_dominance(entry, now_tick, new_realm_band);

            // emit DominanceChangedEvent
            let realm_band = new_realm_band
                .or_else(|| old_dominant.as_ref().and_then(|d| d.realm_band))
                .unwrap_or(RealmBand::Low);

            match (&old_dominant, &new_dominant) {
                (None, Some(nd)) | (Some(_), Some(nd))
                    if old_dominant.as_ref().map(|d| &d.char_id) != Some(&nd.char_id) =>
                {
                    dominance_ev_writer.send(DominanceChangedEvent {
                        zone_name: zone_name.clone(),
                        kind: DominanceEventKind::Established,
                        realm_band: nd.realm_band.unwrap_or(realm_band),
                        observed_at_tick: now_tick,
                    });
                }
                (Some(_), None) => {
                    dominance_ev_writer.send(DominanceChangedEvent {
                        zone_name: zone_name.clone(),
                        kind: DominanceEventKind::Ousted,
                        realm_band,
                        observed_at_tick: now_tick,
                    });
                }
                _ => {}
            }

            entry.dominant = new_dominant;
        }
    }
}

/// 节流判断：距上次 eval 是否已达 `TERRITORY_EVAL_INTERVAL_TICKS`（约 60s）。
///
/// 只用 territory 自己的 60s 间隔——不复用 `should_evaluate_attention`（10s 间隔），
/// 否则 10s 项作为 `||` 首项会令 60s 常量永远无法生效，实际 eval 节奏退化为 10s，
/// BASE_DWELL_PER_EVAL 按 60s 标定，结果影响力累积约 6×设计值。
fn should_eval_territory(now_tick: u64, last_eval_tick: u64) -> bool {
    now_tick >= last_eval_tick
        && now_tick.saturating_sub(last_eval_tick) >= TERRITORY_EVAL_INTERVAL_TICKS
}

/// 计算一次 eval 的衰减量（纯函数，可单测）。
///
/// - 短暂离开（absence_ticks ≤ TICKS_PER_DAY）→ 每 eval 减 `DECAY_PER_EVAL`。
/// - 超过 24h（absence_ticks > TICKS_PER_DAY）→ 乘以 `DECAY_LONG_ABSENCE_MULT`。
/// - 结果 clamp ≥ 0，不返回负数（调用方应再与当前值做 saturating 处理）。
pub fn compute_decay(current_value: f64, absence_ticks: u64) -> f64 {
    let decay_mult = if absence_ticks > TICKS_PER_DAY {
        DECAY_LONG_ABSENCE_MULT
    } else {
        1.0
    };
    let decay = DECAY_PER_EVAL * decay_mult;
    (current_value - decay).max(0.0)
}

/// plan-territory-v1 P2 — PvP 击杀 → 影响力争夺。
///
/// 真 EventReader<DeathEvent>，真 mutate ZoneInfluenceMap。
///
/// 触发条件：
/// - `death.attacker_player_id.is_some()`（攻击者是玩家）
/// - victim(`death.target`)有 `Username`（victim 也是玩家，纯 PvP 双向校验）
/// - victim 在某 zone 内（zone 外击杀不调 influence）
///
/// 效果：
/// - 击杀者 influence +KILL_INFLUENCE_GAIN(5.0)，clamp ≤ INFLUENCE_MAX
/// - 被杀者 influence 归零（= 0.0，彻底失去此地话语权）
/// - 写完后主动调 recompute_dominance（即时生效，不等 territory_tick 60s 批次）
/// - 各 emit 一次 InfluenceChangedEvent{source: Combat}
///
/// 注意：击败不杀（+2.0/-5.0）DEFER P2.5（需 Lifecycle.last_pvp_attacker +
/// NearDeathSurvivedEvent 前置），P2 不实现此路径。
pub fn territory_pvp_influence_system(
    mut deaths: EventReader<DeathEvent>,
    players: PvpParticipantQuery<'_, '_>,
    zones: Option<Res<ZoneRegistry>>,
    clock: Option<Res<CultivationClock>>,
    mut influence_map: ResMut<ZoneInfluenceMap>,
    mut ev_writer: EventWriter<InfluenceChangedEvent>,
    mut dominance_ev_writer: EventWriter<DominanceChangedEvent>,
) {
    let Some(zone_registry) = zones else {
        return;
    };
    let now_tick = clock.as_ref().map(|c| c.tick).unwrap_or(0);

    for death in deaths.read() {
        // PvP 条件 1：攻击者必须有 player_id（即攻击者是玩家）
        let Some(killer_player_id) = death.attacker_player_id.as_ref() else {
            continue;
        };
        // PvP 条件 2：攻击者 entity 存在
        let Some(killer_entity) = death.attacker else {
            continue;
        };

        // PvP 条件 3：victim 必须有 Username（victim 也是玩家，纯 PvP 双向校验）
        let Ok((victim_pos, victim_dim, Some(victim_username))) = players.get(death.target) else {
            continue;
        };

        // zone 定位：用 victim 位置（被击杀地点代表话语权易手的地方）
        let dim = victim_dim.map(|d| d.0).unwrap_or(DimensionKind::Overworld);
        let Some(zone) = zone_registry.find_zone(dim, victim_pos.0) else {
            // zone 外击杀不调 influence
            continue;
        };
        let zone_name = zone.name.clone();

        let victim_player_id = canonical_player_id(victim_username.0.as_str());

        // 自杀防护（killer 和 victim 是同一玩家 id）
        if killer_player_id == &victim_player_id {
            continue;
        }

        // ── 击杀者 +KILL_INFLUENCE_GAIN ──────────────────────────────────────
        {
            let entry = influence_map.zones.entry(zone_name.clone()).or_default();
            let killer_inf = entry.players.entry(killer_player_id.clone()).or_default();
            let old_val = killer_inf.value;
            killer_inf.value = (killer_inf.value + KILL_INFLUENCE_GAIN).clamp(0.0, INFLUENCE_MAX);
            killer_inf.last_activity_tick = now_tick;
            killer_inf.source_breakdown.player_kills += 1;
            let actual_delta = killer_inf.value - old_val;
            let new_total = killer_inf.value;
            ev_writer.send(InfluenceChangedEvent {
                player_entity: killer_entity,
                player_id: killer_player_id.clone(),
                zone_name: zone_name.clone(),
                delta: actual_delta,
                new_total,
                source: InfluenceSource::Combat,
            });
        }

        // ── 被杀者 influence 归零 ─────────────────────────────────────────────
        {
            let entry = influence_map.zones.entry(zone_name.clone()).or_default();
            let victim_inf = entry.players.entry(victim_player_id.clone()).or_default();
            let old_val = victim_inf.value;
            victim_inf.value = 0.0;
            victim_inf.last_activity_tick = now_tick;
            let actual_delta = victim_inf.value - old_val; // 负数或 0
            let new_total = victim_inf.value;
            ev_writer.send(InfluenceChangedEvent {
                player_entity: death.target,
                player_id: victim_player_id.clone(),
                zone_name: zone_name.clone(),
                delta: actual_delta,
                new_total,
                source: InfluenceSource::Combat,
            });
        }

        // ── 主动重算霸主（不等 territory_tick 60s 批次，即时体验）────────────
        {
            let entry = influence_map.zones.entry(zone_name.clone()).or_default();
            let old_dominant = entry.dominant.clone();
            // pvp 场景下 realm_band 从 killer 取（杀者夺主）
            // PvpParticipantQuery 无 Cultivation，realm_band 暂为 None
            let new_dominant = recompute_dominance(entry, now_tick, None);

            // emit DominanceChangedEvent
            let realm_band = old_dominant
                .as_ref()
                .and_then(|d| d.realm_band)
                .unwrap_or(RealmBand::Low);
            match (&old_dominant, &new_dominant) {
                (None, Some(nd)) | (Some(_), Some(nd))
                    if old_dominant.as_ref().map(|d| &d.char_id) != Some(&nd.char_id) =>
                {
                    dominance_ev_writer.send(DominanceChangedEvent {
                        zone_name: zone_name.clone(),
                        kind: DominanceEventKind::Established,
                        realm_band: nd.realm_band.unwrap_or(realm_band),
                        observed_at_tick: now_tick,
                    });
                }
                (Some(_), None) => {
                    dominance_ev_writer.send(DominanceChangedEvent {
                        zone_name: zone_name.clone(),
                        kind: DominanceEventKind::Ousted,
                        realm_band,
                        observed_at_tick: now_tick,
                    });
                }
                _ => {}
            }

            entry.dominant = new_dominant;
        }
    }
}

/// plan-territory-v1 P3 — 灵气耗尽边沿检测系统。
///
/// 独立于 dominance 切换。每次 eval 读 ZoneRegistry spirit_qi 与上一 tick 水位比较，
/// 若从 >0 跌至 ≤0 且该 zone 有霸主 → emit DominanceChangedEvent(QiDepleted)。
/// 使用 ZoneQiWatermark 缓存上一 tick 值，避免每 tick 重复 emit。
pub fn territory_qi_depleted_system(
    clock: Option<Res<CultivationClock>>,
    zones: Option<Res<ZoneRegistry>>,
    influence_map: Res<ZoneInfluenceMap>,
    mut watermark: ResMut<crate::world::territory_narration::ZoneQiWatermark>,
    mut dominance_ev_writer: EventWriter<DominanceChangedEvent>,
) {
    let Some(clock) = clock else {
        return;
    };
    let Some(zone_registry) = zones else {
        return;
    };
    let now_tick = clock.tick;

    for zone in &zone_registry.zones {
        let current_qi = zone.spirit_qi;
        let last_qi = watermark
            .last_qi
            .get(&zone.name)
            .copied()
            .unwrap_or(current_qi);

        let has_dominant = influence_map
            .zones
            .get(&zone.name)
            .and_then(|e| e.dominant.as_ref())
            .is_some();

        if crate::world::territory_narration::detect_qi_depleted_edge(last_qi, current_qi)
            && has_dominant
        {
            let realm_band = influence_map
                .zones
                .get(&zone.name)
                .and_then(|e| e.dominant.as_ref())
                .and_then(|d| d.realm_band)
                .unwrap_or(RealmBand::Low);

            dominance_ev_writer.send(DominanceChangedEvent {
                zone_name: zone.name.clone(),
                kind: DominanceEventKind::QiDepleted,
                realm_band,
                observed_at_tick: now_tick,
            });
        }

        // 更新水位
        watermark.last_qi.insert(zone.name.clone(), current_qi);
    }
}

/// 注册 territory 系统和资源到 Bevy App。
pub fn register(app: &mut App) {
    app.init_resource::<ZoneInfluenceMap>();
    app.add_event::<InfluenceChangedEvent>();
    app.add_systems(Update, territory_tick);
    // plan-territory-v1 P3 — 接入 narration + rumor + public_known 系统
    crate::world::territory_narration::register(app);
    crate::world::territory_rumor::register(app);
    app.add_systems(Update, territory_qi_depleted_system);
}

// ─── 单测 ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod territory_tests {
    use super::*;
    use crate::cultivation::components::Realm;

    // ── 辅助 ──────────────────────────────────────────────────────────────────

    fn assert_close(actual: f64, expected: f64, msg: &str) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "{msg}: 期望 {expected:.12}, 实际 {actual:.12}"
        );
    }

    // ── 1. 停留累积 ──────────────────────────────────────────────────────────

    #[test]
    fn dwell_accumulates_influence() {
        // Awaken(rank=0), spirit_qi=1.0, 无修炼/战斗/注意力/声望
        let delta = compute_influence_delta(0, false, false, 1.0, 0.0, 0);
        // 期望 = BASE_DWELL_PER_EVAL * (0+1) * spirit_qi_factor(1.0) = 0.3
        assert_close(
            delta,
            BASE_DWELL_PER_EVAL,
            "醒灵停留基础 delta 应为 BASE_DWELL_PER_EVAL",
        );
    }

    // ── 2. 修炼乘子 ─────────────────────────────────────────────────────────

    #[test]
    fn practice_multiplier_applied() {
        let without = compute_influence_delta(0, false, false, 1.0, 0.0, 0);
        let with_practice = compute_influence_delta(0, true, false, 1.0, 0.0, 0);
        // 修炼时 = 停留时 × PRACTICE_MULT(1.5)
        assert_close(
            with_practice,
            without * PRACTICE_MULT,
            "修炼乘子应为 PRACTICE_MULT(1.5)",
        );
    }

    // ── 3. 战斗乘子 ─────────────────────────────────────────────────────────

    #[test]
    fn combat_multiplier_applied() {
        let without = compute_influence_delta(0, false, false, 1.0, 0.0, 0);
        let in_combat = compute_influence_delta(0, false, true, 1.0, 0.0, 0);
        assert_close(
            in_combat,
            without * COMBAT_MULT,
            "战斗乘子应为 COMBAT_MULT(1.2)",
        );
    }

    // ── 4. 境界缩放 ─────────────────────────────────────────────────────────

    #[test]
    fn realm_rank_scales_influence() {
        let awaken = compute_influence_delta(realm_rank(Realm::Awaken), false, false, 1.0, 0.0, 0);
        let void_realm =
            compute_influence_delta(realm_rank(Realm::Void), false, false, 1.0, 0.0, 0);
        // Void(rank=5) vs Awaken(rank=0) → (5+1)/(0+1) = 6 倍
        assert_close(void_realm, awaken * 6.0, "化虚应比醒灵影响力高 6 倍");
    }

    // ── 5. TiandaoAttention 驻守加速 ────────────────────────────────────────

    #[test]
    fn tiandao_attention_accelerates() {
        let no_attention = compute_influence_delta(0, false, false, 1.0, 0.0, 0);
        let max_attention = compute_influence_delta(0, false, false, 1.0, ATTENTION_MAX, 0);
        // attention_level=100 → 乘 (1 + 0.5*1.0) = 1.5
        assert_close(max_attention, no_attention * 1.5, "满天道注意力应加速 1.5x");
    }

    // ── 6. 声望分档 ─────────────────────────────────────────────────────────

    #[test]
    fn renown_fame_tiers() {
        let tier0 = compute_influence_delta(0, false, false, 1.0, 0.0, 50);
        let tier1 = compute_influence_delta(0, false, false, 1.0, 0.0, 150);
        let tier2 = compute_influence_delta(0, false, false, 1.0, 0.0, 600);
        // tier0 fame=50 ×1.0, tier1 fame=150 ×1.1, tier2 fame=600 ×1.3
        assert_close(tier0, BASE_DWELL_PER_EVAL, "fame=50 不加成");
        assert_close(tier1, BASE_DWELL_PER_EVAL * 1.1, "fame=150 应 ×1.1");
        assert_close(tier2, BASE_DWELL_PER_EVAL * 1.3, "fame=600 应 ×1.3");
    }

    // ── 7. worldview §十 灵气零和衰减因子 ──────────────────────────────────

    #[test]
    fn spirit_qi_decay_factor() {
        // 废地 spirit_qi=0 → 累积归零
        let waste = compute_influence_delta(0, false, false, 0.0, 0.0, 0);
        assert_close(waste, 0.0, "废地(spirit_qi=0) 累积应归零");

        // 负灵域 spirit_qi=-0.5 → clamp 到 0 → 同归零
        let neg = compute_influence_delta(0, false, false, -0.5, 0.0, 0);
        assert_close(neg, 0.0, "负灵域(spirit_qi<0) 累积应归零");

        // 满浓度 spirit_qi=1 → 满系数
        let full = compute_influence_delta(0, false, false, 1.0, 0.0, 0);
        assert_close(full, BASE_DWELL_PER_EVAL, "满灵域 spirit_qi=1 应满系数");
    }

    // ── 8. 节流判断 ──────────────────────────────────────────────────────────

    #[test]
    fn throttle_first_eval_at_tick_zero() {
        // last_eval=0, now_tick=0 → 差=0 < TERRITORY_EVAL_INTERVAL_TICKS → 不 eval
        // 注意：last_eval=0 对「全新玩家」在 territory_tick 里走 last_eval=0 路径时
        // 用 `last_eval > 0` 守卫，所以此处仅测节流函数本身行为：
        // 同 tick 不满足间隔，返回 false。
        assert!(
            !should_eval_territory(0, 0),
            "now=0 last=0 差=0 < TERRITORY_EVAL_INTERVAL_TICKS({TERRITORY_EVAL_INTERVAL_TICKS}) 应返回 false"
        );
    }

    #[test]
    fn throttle_before_interval_returns_false() {
        // 距上次 eval 未满 60s（1199 ticks < 1200）→ 不 eval
        let last = 1000u64;
        let now = last + TERRITORY_EVAL_INTERVAL_TICKS - 1;
        assert!(
            !should_eval_territory(now, last),
            "距上次 eval {TERRITORY_EVAL_INTERVAL_TICKS}-1 ticks 不应触发 eval"
        );
    }

    #[test]
    fn throttle_exactly_at_interval_returns_true() {
        // 距上次 eval 恰好 60s（1200 ticks）→ 应 eval
        let last = 1000u64;
        let now = last + TERRITORY_EVAL_INTERVAL_TICKS;
        assert!(
            should_eval_territory(now, last),
            "距上次 eval 恰好 TERRITORY_EVAL_INTERVAL_TICKS({TERRITORY_EVAL_INTERVAL_TICKS}) ticks 应触发 eval"
        );
    }

    #[test]
    fn throttle_over_interval_returns_true() {
        // 距上次 eval 超过 60s → 应 eval
        let last = 0u64;
        let now = TERRITORY_EVAL_INTERVAL_TICKS * 3;
        assert!(
            should_eval_territory(now, last),
            "超过 TERRITORY_EVAL_INTERVAL_TICKS 应触发 eval"
        );
    }

    // ── 9. compute_decay 纯函数 ──────────────────────────────────────────────

    #[test]
    fn leaving_zone_decays() {
        // 短暂离开（absence_ticks < TICKS_PER_DAY）→ 减 DECAY_PER_EVAL
        let result = compute_decay(50.0, 100);
        assert_close(result, 50.0 - DECAY_PER_EVAL, "短暂离开应减 DECAY_PER_EVAL");
    }

    #[test]
    fn long_absence_decays_faster() {
        // 超过 24h（absence_ticks > TICKS_PER_DAY）→ 减 DECAY_PER_EVAL × DECAY_LONG_ABSENCE_MULT
        let result = compute_decay(50.0, TICKS_PER_DAY + 1);
        assert_close(
            result,
            50.0 - DECAY_PER_EVAL * DECAY_LONG_ABSENCE_MULT,
            ">24h 未回应快速衰减 ×DECAY_LONG_ABSENCE_MULT({DECAY_LONG_ABSENCE_MULT})",
        );
    }

    #[test]
    fn decay_clamps_at_zero() {
        // 当前值 < DECAY_PER_EVAL → clamp 到 0，不出负数
        let result = compute_decay(0.1, 100);
        assert_close(result, 0.0, "衰减结果应 clamp ≥ 0，不出负数");
    }

    #[test]
    fn decay_boundary_exactly_ticks_per_day() {
        // absence_ticks == TICKS_PER_DAY（恰好 24h，非超过）→ 用普通衰减
        let result = compute_decay(50.0, TICKS_PER_DAY);
        assert_close(
            result,
            50.0 - DECAY_PER_EVAL,
            "absence_ticks=TICKS_PER_DAY 恰好不超过应用普通衰减",
        );
    }

    // ── 10. dominance 判定 ───────────────────────────────────────────────────

    #[test]
    fn dominance_promotes_highest() {
        // 两玩家 35 / 45，45 者领先 ≥ 10 → 成为 dominant
        let mut entry = ZoneInfluenceEntry::default();
        entry.players.insert(
            "p1".to_string(),
            PlayerInfluence {
                value: 35.0,
                ..Default::default()
            },
        );
        entry.players.insert(
            "p2".to_string(),
            PlayerInfluence {
                value: 45.0,
                ..Default::default()
            },
        );
        let dom = recompute_dominance(&entry, 1000, None);
        assert!(dom.is_some(), "应有霸主");
        let dom = dom.unwrap();
        assert_eq!(dom.char_id, "p2", "p2(45)应成为霸主");
        assert_close(dom.influence, 45.0, "霸主 influence 应为 45");
        assert_eq!(dom.established_tick, 1000, "霸主确立 tick 应为 1000");
        assert!(!dom.public_known, "P0 initial public_known=false");
    }

    // ── 11. dominance 平局/边界 ─────────────────────────────────────────────

    #[test]
    fn dominance_tie_no_switch() {
        // 两玩家 40 / 40 → 无霸主（差 < LEAD）
        let mut entry = ZoneInfluenceEntry::default();
        entry.players.insert(
            "p1".to_string(),
            PlayerInfluence {
                value: 40.0,
                ..Default::default()
            },
        );
        entry.players.insert(
            "p2".to_string(),
            PlayerInfluence {
                value: 40.0,
                ..Default::default()
            },
        );
        let dom = recompute_dominance(&entry, 100, None);
        assert!(dom.is_none(), "两人并列应无霸主（防抖）");
    }

    #[test]
    fn dominance_empty_zone_none() {
        let entry = ZoneInfluenceEntry::default();
        let dom = recompute_dominance(&entry, 0, None);
        assert!(dom.is_none(), "空 zone 应无霸主");
    }

    // ── 12. dominance 状态转换 A→B（换主）────────────────────────────────────

    #[test]
    fn dominance_switch_a_to_b_when_incumbent_drops_below_drop_threshold() {
        // A→B 换主：现任霸主 influence 跌破 DOMINANCE_DROP(20)，挑战者领先 ≥ LEAD(10)
        // → 切换为挑战者（对应生产代码 line: current_val < DOMINANCE_DROP → 检查新候选）
        let mut entry = ZoneInfluenceEntry::default();
        // 现任霸主 influence 跌到 15（< DROP=20）
        entry.players.insert(
            "incumbent".to_string(),
            PlayerInfluence {
                value: 15.0,
                ..Default::default()
            },
        );
        // 挑战者 influence 45（≥ THRESHOLD=30，领先 second=15 ≥ LEAD=10）
        entry.players.insert(
            "challenger".to_string(),
            PlayerInfluence {
                value: 45.0,
                ..Default::default()
            },
        );
        entry.dominant = Some(ZoneDominance {
            char_id: "incumbent".to_string(),
            influence: 35.0,
            established_tick: 0,
            public_known: true,
            realm_band: None,
        });

        let dom = recompute_dominance(&entry, 2000, None);
        assert!(dom.is_some(), "应有新霸主");
        let dom = dom.unwrap();
        assert_eq!(
            dom.char_id, "challenger",
            "现任跌破 DROP 且挑战者领先 ≥ LEAD，应切换为挑战者；实际霸主={}",
            dom.char_id
        );
        assert_eq!(
            dom.established_tick, 2000,
            "新霸主 established_tick 应为当前 tick=2000，实际={}",
            dom.established_tick
        );
        assert!(
            !dom.public_known,
            "新霸主初始 public_known 应为 false（P3 传播）"
        );
    }

    // ── 13. dominance 状态转换 A→A（维持）──────────────────────────────────

    #[test]
    fn dominance_maintain_a_to_a_when_incumbent_healthy_and_challenger_not_leading_enough() {
        // A→A 维持：现任霸主 influence 健康(≥DROP=20)，挑战者接近但差 < LEAD(10)
        // → 维持现任（对应生产代码：current_val ≥ DROP → top_val - current_val < LEAD → return incumbent）
        let mut entry = ZoneInfluenceEntry::default();
        // 现任霸主 influence=40（≥ DROP=20，≥ THRESHOLD=30）
        entry.players.insert(
            "incumbent".to_string(),
            PlayerInfluence {
                value: 40.0,
                ..Default::default()
            },
        );
        // 挑战者 influence=45（领先 45-40=5 < LEAD=10）→ 防抖，维持现任
        entry.players.insert(
            "challenger".to_string(),
            PlayerInfluence {
                value: 45.0,
                ..Default::default()
            },
        );
        entry.dominant = Some(ZoneDominance {
            char_id: "incumbent".to_string(),
            influence: 40.0,
            established_tick: 500,
            public_known: true,
            realm_band: None,
        });

        let dom = recompute_dominance(&entry, 3000, None);
        assert!(dom.is_some(), "应维持有霸主");
        let dom = dom.unwrap();
        assert_eq!(
            dom.char_id, "incumbent",
            "挑战者领先 < LEAD({DOMINANCE_LEAD}) 应维持现任 incumbent，实际={}",
            dom.char_id
        );
        assert_eq!(
            dom.established_tick, 500,
            "维持现任时 established_tick 应保持原值 500，实际={}",
            dom.established_tick
        );
        assert!(dom.public_known, "维持现任时 public_known 应保持原值 true");
    }

    // ── 14. dominance 滞后防抖 drop ─────────────────────────────────────────

    #[test]
    fn dominance_hysteresis_drop() {
        // 现任霸主 influence 跌破 DOMINANCE_DROP(20) 且无够格挑战者 → 失去地位
        let mut entry = ZoneInfluenceEntry::default();
        let current = ZoneDominance {
            char_id: "incumbent".to_string(),
            influence: 35.0,
            established_tick: 0,
            public_known: false,
            realm_band: None,
        };
        entry.dominant = Some(current);
        // 当前记录 incumbent 跌到 15（< DROP），且无候选超过 THRESHOLD
        entry.players.insert(
            "incumbent".to_string(),
            PlayerInfluence {
                value: 15.0,
                ..Default::default()
            },
        );

        let dom = recompute_dominance(&entry, 500, None);
        assert!(
            dom.is_none(),
            "现任霸主 influence(15) < DOMINANCE_DROP({DOMINANCE_DROP}) 且无够格挑战者应失去地位"
        );
    }

    // ── 15. influence 上限 clamp ─────────────────────────────────────────────

    #[test]
    fn clamp_upper_bound() {
        // 累积超过 INFLUENCE_MAX(100) 应 clamp 到 100
        let current = 99.9;
        let delta = compute_influence_delta(5, true, true, 1.0, 100.0, 600);
        let new_val = (current + delta).min(INFLUENCE_MAX);
        assert!(
            new_val <= INFLUENCE_MAX,
            "clamp 后应 ≤ {INFLUENCE_MAX}，实际 {new_val}"
        );
        assert_close(new_val, INFLUENCE_MAX, "超出 INFLUENCE_MAX 应 clamp 至 100");
    }

    // ── 16. ZoneInfluenceEntry 中 dominant 字段写入 ──────────────────────────

    #[test]
    fn dominant_player_written_to_zone_entry() {
        // 验证 recompute_dominance 的结果写回 entry.dominant（出料：写 Zone.dominant_player）
        let mut entry = ZoneInfluenceEntry::default();
        entry.players.insert(
            "hero".to_string(),
            PlayerInfluence {
                value: 50.0,
                ..Default::default()
            },
        );
        entry.players.insert(
            "rival".to_string(),
            PlayerInfluence {
                value: 30.0,
                ..Default::default()
            },
        );
        entry.dominant = recompute_dominance(&entry, 999, None);
        let dom = entry.dominant.as_ref().expect("应有霸主");
        assert_eq!(dom.char_id, "hero", "霸主应为 hero");
        assert_close(dom.influence, 50.0, "霸主 influence 应为 50");
    }

    // ── 17. territory_tick 系统集成测试 ─────────────────────────────────────
    //
    // 使用 valence::testing::create_mock_client 构造真实 Bevy App，
    // 驱动 territory_tick system 跑完整 Update 周期，断言：
    //   - ZoneInfluenceMap 中玩家 influence > 0（值写入）
    //   - source 分类正确（Dwell/Practice/Combat）
    //   - InfluenceChangedEvent 被 emit（source/delta/new_total 契约）
    //   - 离开 zone 后衰减（Decay 事件 emit）
    //   - 节流：< 60s 不重复 eval

    #[test]
    fn territory_tick_dwell_accumulates_influence_and_emits_event() {
        use crate::cultivation::tick::CultivationClock;
        use crate::world::zone::{Zone, ZoneRegistry};
        use valence::prelude::{bevy_ecs, DVec3, Update};
        use valence::testing::create_mock_client;

        let mut app = App::new();
        app.add_event::<InfluenceChangedEvent>();
        app.add_event::<DominanceChangedEvent>();
        app.init_resource::<ZoneInfluenceMap>();

        // 构造一个包含 spawn zone 的 ZoneRegistry
        // spawn zone: bounds = [(-128,64,-128) .. (128,80,128)], spirit_qi=0.9
        let zone = Zone {
            name: "spawn".to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            bounds: (
                DVec3::new(-128.0, 64.0, -128.0),
                DVec3::new(128.0, 80.0, 128.0),
            ),
            spirit_qi: 0.9,
            danger_level: 0,
            active_events: vec![],
            patrol_anchors: vec![],
            blocked_tiles: vec![],
        };
        app.insert_resource(ZoneRegistry { zones: vec![zone] });
        app.insert_resource(CultivationClock {
            tick: TERRITORY_EVAL_INTERVAL_TICKS,
        });
        app.add_systems(Update, territory_tick);

        // 构造带真实 Client 组件的玩家 entity
        // ClientBundle 已包含 Position，需通过 bundle.player.position 设置，不能重复 spawn
        let (mut client_bundle, _helper) = create_mock_client("Hero");
        client_bundle.player.position = valence::prelude::Position::new([0.0, 66.0, 0.0]);
        let player = app
            .world_mut()
            .spawn((
                client_bundle,
                crate::cultivation::components::Cultivation {
                    realm: crate::cultivation::components::Realm::Awaken,
                    qi_current: 50.0,
                    qi_max: 100.0,
                    ..crate::cultivation::components::Cultivation::default()
                },
            ))
            .id();

        app.update();

        // 断言 ZoneInfluenceMap 已写入影响力（先读出值，避免与后续 world_mut 的借用冲突）
        let pid = canonical_player_id("Hero");
        let (recorded_value, recorded_last_tick) = {
            let map = app.world().resource::<ZoneInfluenceMap>();
            let entry = map
                .zones
                .get("spawn")
                .expect("territory_tick 应在 ZoneInfluenceMap 写入 spawn zone 条目");
            let player_inf = entry
                .players
                .get(&pid)
                .expect("territory_tick 应在 spawn zone 记录玩家影响力");
            (player_inf.value, player_inf.last_activity_tick)
        };
        assert!(
            recorded_value > 0.0,
            "停留应累积正 influence，实际={:.6}",
            recorded_value
        );
        assert_eq!(
            recorded_last_tick, TERRITORY_EVAL_INTERVAL_TICKS,
            "last_activity_tick 应更新为当前 clock.tick"
        );

        // 断言 InfluenceChangedEvent 被 emit，source=Dwell，delta>0，new_total 与记录一致
        let emitted: Vec<InfluenceChangedEvent> = app
            .world_mut()
            .resource_mut::<bevy_ecs::event::Events<InfluenceChangedEvent>>()
            .drain()
            .collect();
        assert!(
            !emitted.is_empty(),
            "territory_tick 应 emit InfluenceChangedEvent"
        );
        let ev = emitted
            .iter()
            .find(|e| e.player_entity == player && e.zone_name == "spawn")
            .expect("应找到对应玩家+zone 的事件");
        assert_eq!(
            ev.source,
            InfluenceSource::Dwell,
            "纯停留（未修炼/战斗）source 应为 Dwell，实际={:?}",
            ev.source
        );
        assert!(
            ev.delta > 0.0,
            "delta 应为正数（停留累积），实际={}",
            ev.delta
        );
        assert!(
            (ev.new_total - recorded_value).abs() < 1e-9,
            "事件 new_total({}) 应与 ZoneInfluenceMap 记录值({}) 一致",
            ev.new_total,
            recorded_value
        );
        let _ = player; // suppress unused warning
    }

    #[test]
    fn territory_tick_practice_source_emitted_when_recently_practicing() {
        use crate::cultivation::tick::{CultivationClock, CultivationSessionPracticeAccumulator};
        use crate::world::zone::{Zone, ZoneRegistry};
        use valence::prelude::{bevy_ecs, DVec3, Update};
        use valence::testing::create_mock_client;

        let mut app = App::new();
        app.add_event::<InfluenceChangedEvent>();
        app.add_event::<DominanceChangedEvent>();
        app.init_resource::<ZoneInfluenceMap>();

        let zone = Zone {
            name: "qingyun".to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            bounds: (DVec3::new(-64.0, 60.0, -64.0), DVec3::new(64.0, 80.0, 64.0)),
            spirit_qi: 1.0,
            danger_level: 0,
            active_events: vec![],
            patrol_anchors: vec![],
            blocked_tiles: vec![],
        };
        app.insert_resource(ZoneRegistry { zones: vec![zone] });

        // 构造修炼累积器，记录玩家最近修炼
        let mut accumulator = CultivationSessionPracticeAccumulator::default();
        let eval_tick = TERRITORY_EVAL_INTERVAL_TICKS;
        // 预分配 entity id 需先 spawn 再插入，用占位 id 填写 accumulator
        // 通过 note_practice_tick_for_tests 在 eval_tick 记录
        app.insert_resource(CultivationClock { tick: eval_tick });
        app.add_systems(Update, territory_tick);

        let (mut client_bundle, _helper) = create_mock_client("Cultivator");
        client_bundle.player.position = valence::prelude::Position::new([0.0, 66.0, 0.0]);
        let player = app
            .world_mut()
            .spawn((
                client_bundle,
                crate::cultivation::components::Cultivation {
                    realm: crate::cultivation::components::Realm::Induce,
                    qi_current: 80.0,
                    qi_max: 100.0,
                    ..crate::cultivation::components::Cultivation::default()
                },
            ))
            .id();

        // 记录修炼 tick（在 eval_tick 时刻修炼）
        accumulator.note_practice_tick_for_tests(player, eval_tick);
        app.insert_resource(accumulator);

        app.update();

        // 断言事件 source=Practice
        let emitted: Vec<InfluenceChangedEvent> = app
            .world_mut()
            .resource_mut::<bevy_ecs::event::Events<InfluenceChangedEvent>>()
            .drain()
            .collect();
        let ev = emitted
            .iter()
            .find(|e| e.player_entity == player && e.zone_name == "qingyun")
            .expect("应有 qingyun zone 的 InfluenceChangedEvent");
        assert_eq!(
            ev.source,
            InfluenceSource::Practice,
            "正在修炼时 source 应为 Practice，实际={:?}",
            ev.source
        );
        assert!(
            ev.delta > 0.0,
            "修炼时 delta 应 > 0（修炼乘子 × 停留量），实际={}",
            ev.delta
        );
        let _ = player;
    }

    #[test]
    fn territory_tick_combat_source_emitted_when_in_combat() {
        use crate::combat::components::CombatState;
        use crate::cultivation::tick::CultivationClock;
        use crate::world::zone::{Zone, ZoneRegistry};
        use valence::prelude::{bevy_ecs, DVec3, Update};
        use valence::testing::create_mock_client;

        let mut app = App::new();
        app.add_event::<InfluenceChangedEvent>();
        app.add_event::<DominanceChangedEvent>();
        app.init_resource::<ZoneInfluenceMap>();

        let eval_tick = TERRITORY_EVAL_INTERVAL_TICKS;
        let zone = Zone {
            name: "bloodzone".to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            bounds: (DVec3::new(-32.0, 60.0, -32.0), DVec3::new(32.0, 80.0, 32.0)),
            spirit_qi: 0.8,
            danger_level: 3,
            active_events: vec![],
            patrol_anchors: vec![],
            blocked_tiles: vec![],
        };
        app.insert_resource(ZoneRegistry { zones: vec![zone] });
        app.insert_resource(CultivationClock { tick: eval_tick });
        app.add_systems(Update, territory_tick);

        // in_combat_until_tick > eval_tick → 当前 tick 处于战斗状态
        let combat_state = CombatState {
            in_combat_until_tick: Some(eval_tick + 100),
            ..CombatState::default()
        };

        let (mut client_bundle, _helper) = create_mock_client("Warrior");
        client_bundle.player.position = valence::prelude::Position::new([0.0, 66.0, 0.0]);
        let player = app
            .world_mut()
            .spawn((
                client_bundle,
                crate::cultivation::components::Cultivation {
                    realm: crate::cultivation::components::Realm::Condense,
                    qi_current: 60.0,
                    qi_max: 100.0,
                    ..crate::cultivation::components::Cultivation::default()
                },
                combat_state,
            ))
            .id();

        app.update();

        let emitted: Vec<InfluenceChangedEvent> = app
            .world_mut()
            .resource_mut::<bevy_ecs::event::Events<InfluenceChangedEvent>>()
            .drain()
            .collect();
        let ev = emitted
            .iter()
            .find(|e| e.player_entity == player && e.zone_name == "bloodzone")
            .expect("应有 bloodzone 的 InfluenceChangedEvent");
        assert_eq!(
            ev.source,
            InfluenceSource::Combat,
            "战斗中 source 应为 Combat，实际={:?}",
            ev.source
        );
        assert!(
            ev.delta > 0.0,
            "战斗时 delta 应 > 0（战斗乘子 × 停留量），实际={}",
            ev.delta
        );
        let _ = player;
    }

    #[test]
    fn territory_tick_decay_event_emitted_when_player_leaves_zone() {
        use crate::cultivation::tick::CultivationClock;
        use crate::world::zone::{Zone, ZoneRegistry};
        use valence::prelude::{bevy_ecs, DVec3, Update};
        use valence::testing::create_mock_client;

        let mut app = App::new();
        app.add_event::<InfluenceChangedEvent>();
        app.add_event::<DominanceChangedEvent>();
        app.init_resource::<ZoneInfluenceMap>();

        let zone = Zone {
            name: "mountain".to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            // zone 覆盖 x=[200..300] → 玩家站在 x=0 → 不在 zone 内 → 衰减
            bounds: (
                DVec3::new(200.0, 60.0, 200.0),
                DVec3::new(300.0, 80.0, 300.0),
            ),
            spirit_qi: 0.7,
            danger_level: 0,
            active_events: vec![],
            patrol_anchors: vec![],
            blocked_tiles: vec![],
        };
        app.insert_resource(ZoneRegistry { zones: vec![zone] });
        let eval_tick = TERRITORY_EVAL_INTERVAL_TICKS;
        app.insert_resource(CultivationClock { tick: eval_tick });
        app.add_systems(Update, territory_tick);

        // 玩家站在 x=0，不在 mountain zone（zone 在 x=[200..300]）
        let (mut client_bundle, _helper) = create_mock_client("Wanderer");
        client_bundle.player.position = valence::prelude::Position::new([0.0, 66.0, 0.0]);
        let player = app
            .world_mut()
            .spawn((
                client_bundle,
                crate::cultivation::components::Cultivation {
                    realm: crate::cultivation::components::Realm::Awaken,
                    qi_current: 30.0,
                    qi_max: 100.0,
                    ..crate::cultivation::components::Cultivation::default()
                },
            ))
            .id();

        // 预填玩家在 mountain zone 有历史影响力
        {
            let mut map = app.world_mut().resource_mut::<ZoneInfluenceMap>();
            let entry = map.zones.entry("mountain".to_string()).or_default();
            let pid = canonical_player_id("Wanderer");
            entry.players.insert(
                pid,
                PlayerInfluence {
                    value: 20.0,
                    last_activity_tick: 0, // 长时间未活动（absence=eval_tick）
                    source_breakdown: Default::default(),
                },
            );
        }

        app.update();

        // 断言 ZoneInfluenceMap 中影响力已衰减
        let pid = canonical_player_id("Wanderer");
        let map = app.world().resource::<ZoneInfluenceMap>();
        let entry = map
            .zones
            .get("mountain")
            .expect("mountain zone 应仍存在 ZoneInfluenceMap 中");
        let player_inf = entry.players.get(&pid).expect("玩家应仍有记录");
        assert!(
            player_inf.value < 20.0,
            "离开 zone 后影响力应衰减，期望 < 20.0，实际={}",
            player_inf.value
        );

        // 断言 Decay 事件被 emit
        let emitted: Vec<InfluenceChangedEvent> = app
            .world_mut()
            .resource_mut::<bevy_ecs::event::Events<InfluenceChangedEvent>>()
            .drain()
            .collect();
        let decay_ev = emitted
            .iter()
            .find(|e| e.player_entity == player && e.zone_name == "mountain" && e.delta < 0.0);
        assert!(
            decay_ev.is_some(),
            "离开 zone 应 emit InfluenceChangedEvent(source=Decay, delta<0)，实际事件={:?}",
            emitted
                .iter()
                .map(|e| (&e.zone_name, e.delta, &e.source))
                .collect::<Vec<_>>()
        );
        let decay_ev = decay_ev.unwrap();
        assert_eq!(
            decay_ev.source,
            InfluenceSource::Decay,
            "离开 zone 的事件 source 应为 Decay，实际={:?}",
            decay_ev.source
        );
        let _ = player;
    }

    #[test]
    fn territory_tick_throttle_no_double_eval_within_interval() {
        use crate::cultivation::tick::CultivationClock;
        use crate::world::zone::{Zone, ZoneRegistry};
        use valence::prelude::{DVec3, Update};
        use valence::testing::create_mock_client;

        let mut app = App::new();
        app.add_event::<InfluenceChangedEvent>();
        app.add_event::<DominanceChangedEvent>();
        app.init_resource::<ZoneInfluenceMap>();

        let zone = Zone {
            name: "spawn".to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            bounds: (
                DVec3::new(-128.0, 64.0, -128.0),
                DVec3::new(128.0, 80.0, 128.0),
            ),
            spirit_qi: 0.9,
            danger_level: 0,
            active_events: vec![],
            patrol_anchors: vec![],
            blocked_tiles: vec![],
        };
        app.insert_resource(ZoneRegistry { zones: vec![zone] });
        app.insert_resource(CultivationClock {
            tick: TERRITORY_EVAL_INTERVAL_TICKS,
        });
        app.add_systems(Update, territory_tick);

        let (mut client_bundle, _helper) = create_mock_client("Throttled");
        client_bundle.player.position = valence::prelude::Position::new([0.0, 66.0, 0.0]);
        let player = app
            .world_mut()
            .spawn((
                client_bundle,
                crate::cultivation::components::Cultivation {
                    realm: crate::cultivation::components::Realm::Awaken,
                    qi_current: 50.0,
                    qi_max: 100.0,
                    ..crate::cultivation::components::Cultivation::default()
                },
            ))
            .id();

        // 第一次 update → 累积
        app.update();
        let influence_after_first = {
            let map = app.world().resource::<ZoneInfluenceMap>();
            let pid = canonical_player_id("Throttled");
            map.zones
                .get("spawn")
                .and_then(|e| e.players.get(&pid))
                .map(|p| p.value)
                .unwrap_or(0.0)
        };
        assert!(
            influence_after_first > 0.0,
            "第一次 update 应累积影响力，实际={}",
            influence_after_first
        );

        // 不推进 clock（仍为 TERRITORY_EVAL_INTERVAL_TICKS），第二次 update 应被节流
        app.update();
        let influence_after_second = {
            let map = app.world().resource::<ZoneInfluenceMap>();
            let pid = canonical_player_id("Throttled");
            map.zones
                .get("spawn")
                .and_then(|e| e.players.get(&pid))
                .map(|p| p.value)
                .unwrap_or(0.0)
        };
        assert_eq!(
            influence_after_first, influence_after_second,
            "clock 未推进（距上次 eval < TERRITORY_EVAL_INTERVAL_TICKS），第二次 update 应被节流不再累积；\
             第一次={}, 第二次={}",
            influence_after_first, influence_after_second
        );
        let _ = player;
    }

    // ── P2 PvP 影响力系统集成测试 ─────────────────────────────────────────────
    //
    // 用真实 Bevy App + add_systems(territory_pvp_influence_system) + 手动 send
    // DeathEvent + app.update() 断言 ZoneInfluenceMap 的副作用（非纯函数测试）。

    /// 辅助：构造 P2 测试用 App（注册必要资源 + 系统）。
    fn make_pvp_app() -> App {
        use crate::cultivation::tick::CultivationClock;
        use crate::world::zone::{Zone, ZoneRegistry};
        use valence::prelude::DVec3;

        let mut app = App::new();
        app.add_event::<DeathEvent>();
        app.add_event::<InfluenceChangedEvent>();
        app.add_event::<DominanceChangedEvent>();
        app.init_resource::<ZoneInfluenceMap>();

        let zone = Zone {
            name: "pvp_zone".to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            bounds: (DVec3::new(-64.0, 60.0, -64.0), DVec3::new(64.0, 80.0, 64.0)),
            spirit_qi: 1.0,
            danger_level: 1,
            active_events: vec![],
            patrol_anchors: vec![],
            blocked_tiles: vec![],
        };
        app.insert_resource(ZoneRegistry { zones: vec![zone] });
        app.insert_resource(CultivationClock { tick: 100 });
        app.add_systems(valence::prelude::Update, territory_pvp_influence_system);
        app
    }

    /// 辅助：读 ZoneInfluenceMap 中某 zone 某玩家的 influence 值（若不存在返回 None）。
    fn read_influence(app: &App, zone: &str, player_id: &str) -> Option<f64> {
        app.world()
            .resource::<ZoneInfluenceMap>()
            .zones
            .get(zone)
            .and_then(|e| e.players.get(player_id))
            .map(|p| p.value)
    }

    /// 辅助：读 ZoneInfluenceMap 中某 zone 某玩家的 last_activity_tick。
    fn read_last_tick(app: &App, zone: &str, player_id: &str) -> Option<u64> {
        app.world()
            .resource::<ZoneInfluenceMap>()
            .zones
            .get(zone)
            .and_then(|e| e.players.get(player_id))
            .map(|p| p.last_activity_tick)
    }

    /// 辅助：读 ZoneInfluenceMap 中某 zone 某玩家的 player_kills 计数。
    fn read_player_kills(app: &App, zone: &str, player_id: &str) -> u32 {
        app.world()
            .resource::<ZoneInfluenceMap>()
            .zones
            .get(zone)
            .and_then(|e| e.players.get(player_id))
            .map(|p| p.source_breakdown.player_kills)
            .unwrap_or(0)
    }

    /// 辅助：drain InfluenceChangedEvent。
    fn drain_influence_events(app: &mut App) -> Vec<InfluenceChangedEvent> {
        app.world_mut()
            .resource_mut::<bevy_ecs::event::Events<InfluenceChangedEvent>>()
            .drain()
            .collect()
    }

    fn drain_dominance_events(app: &mut App) -> Vec<DominanceChangedEvent> {
        app.world_mut()
            .resource_mut::<bevy_ecs::event::Events<DominanceChangedEvent>>()
            .drain()
            .collect()
    }

    fn make_qi_depleted_app(initial_qi: f64) -> App {
        use crate::world::territory_narration::ZoneQiWatermark;
        use crate::world::zone::Zone;
        use valence::prelude::DVec3;

        let mut app = App::new();
        app.add_event::<DominanceChangedEvent>();
        app.init_resource::<ZoneInfluenceMap>();
        app.init_resource::<ZoneQiWatermark>();
        app.insert_resource(CultivationClock { tick: 42 });
        app.insert_resource(ZoneRegistry {
            zones: vec![Zone {
                name: "dead_zone".to_string(),
                dimension: DimensionKind::Overworld,
                bounds: (DVec3::new(0.0, 60.0, 0.0), DVec3::new(64.0, 80.0, 64.0)),
                spirit_qi: initial_qi,
                danger_level: 0,
                active_events: vec![],
                patrol_anchors: vec![],
                blocked_tiles: vec![],
            }],
        });
        app.add_systems(Update, territory_qi_depleted_system);

        {
            let mut map = app.world_mut().resource_mut::<ZoneInfluenceMap>();
            map.zones.insert(
                "dead_zone".to_string(),
                ZoneInfluenceEntry {
                    dominant: Some(ZoneDominance {
                        char_id: "hero".to_string(),
                        influence: 60.0,
                        established_tick: 1,
                        public_known: true,
                        realm_band: Some(RealmBand::Mid),
                    }),
                    ..Default::default()
                },
            );
        }

        app
    }

    #[test]
    fn qi_depleted_system_does_not_emit_for_initial_dead_zone() {
        let mut app = make_qi_depleted_app(0.0);

        app.update();

        assert!(
            drain_dominance_events(&mut app).is_empty(),
            "初始已为废地的 zone 不应在服务启动首 tick 误触发 QiDepleted"
        );
    }

    #[test]
    fn qi_depleted_system_emits_when_qi_crosses_to_zero_after_watermark() {
        let mut app = make_qi_depleted_app(0.5);

        app.update();
        assert!(
            drain_dominance_events(&mut app).is_empty(),
            "首次建立正水位 watermark 不应触发 QiDepleted"
        );

        {
            let mut zones = app.world_mut().resource_mut::<ZoneRegistry>();
            zones.zones[0].spirit_qi = 0.0;
        }
        app.world_mut().resource_mut::<CultivationClock>().tick = 43;
        app.update();

        let events = drain_dominance_events(&mut app);
        assert_eq!(events.len(), 1, "正水位跌至 0 应触发 1 条 QiDepleted");
        assert_eq!(events[0].zone_name, "dead_zone");
        assert_eq!(events[0].kind, DominanceEventKind::QiDepleted);
        assert_eq!(events[0].realm_band, RealmBand::Mid);
        assert_eq!(events[0].observed_at_tick, 43);
    }

    // ── P2 Case 1: zone内PvP击杀 killer +5.0, victim 归零 ────────────────────

    #[test]
    fn pvp_kill_in_zone_killer_gains_5_victim_zeroed() {
        use valence::testing::create_mock_client;

        let mut app = make_pvp_app();

        let (mut killer_bundle, _) = create_mock_client("Killer");
        killer_bundle.player.position = valence::prelude::Position::new([0.0, 66.0, 0.0]);
        let killer_entity = app.world_mut().spawn(killer_bundle).id();

        let (mut victim_bundle, _) = create_mock_client("Victim");
        victim_bundle.player.position = valence::prelude::Position::new([5.0, 66.0, 5.0]);
        let victim_entity = app.world_mut().spawn(victim_bundle).id();

        let killer_id = canonical_player_id("Killer");
        let victim_id = canonical_player_id("Victim");

        app.world_mut()
            .resource_mut::<bevy_ecs::event::Events<DeathEvent>>()
            .send(DeathEvent {
                target: victim_entity,
                cause: "pvp".to_string(),
                attacker: Some(killer_entity),
                attacker_player_id: Some(killer_id.clone()),
                at_tick: 100,
            });

        app.update();

        let killer_val = read_influence(&app, "pvp_zone", &killer_id);
        let victim_val = read_influence(&app, "pvp_zone", &victim_id);

        assert!(
            killer_val.is_some(),
            "击杀者 influence 应写入 pvp_zone，期望 +{KILL_INFLUENCE_GAIN}"
        );
        assert!(
            (killer_val.unwrap() - KILL_INFLUENCE_GAIN).abs() < 1e-9,
            "击杀者 influence 期望 +{KILL_INFLUENCE_GAIN}，实际={:?}",
            killer_val
        );
        assert!(
            victim_val.is_some(),
            "被杀者 influence 应写入 pvp_zone，期望归零"
        );
        assert!(
            victim_val.unwrap().abs() < 1e-9,
            "被杀者 influence 期望归零(0.0)，实际={:?}",
            victim_val
        );

        // last_activity_tick 更新为 clock.tick=100
        let killer_tick = read_last_tick(&app, "pvp_zone", &killer_id);
        assert_eq!(
            killer_tick,
            Some(100),
            "击杀者 last_activity_tick 期望=100，实际={:?}",
            killer_tick
        );
        let victim_tick = read_last_tick(&app, "pvp_zone", &victim_id);
        assert_eq!(
            victim_tick,
            Some(100),
            "被杀者 last_activity_tick 期望=100，实际={:?}",
            victim_tick
        );
    }

    // ── P2 Case 2: victim 非0初值击杀后归零 ───────────────────────────────────

    #[test]
    fn pvp_kill_zeroes_nonzero_victim_influence() {
        use valence::testing::create_mock_client;

        let mut app = make_pvp_app();

        let (mut killer_bundle, _) = create_mock_client("Killer2");
        killer_bundle.player.position = valence::prelude::Position::new([0.0, 66.0, 0.0]);
        let killer_entity = app.world_mut().spawn(killer_bundle).id();

        let (mut victim_bundle, _) = create_mock_client("Victim2");
        victim_bundle.player.position = valence::prelude::Position::new([3.0, 66.0, 3.0]);
        let victim_entity = app.world_mut().spawn(victim_bundle).id();

        let killer_id = canonical_player_id("Killer2");
        let victim_id = canonical_player_id("Victim2");

        // 预置 victim influence = 30.0
        {
            let mut map = app.world_mut().resource_mut::<ZoneInfluenceMap>();
            let entry = map.zones.entry("pvp_zone".to_string()).or_default();
            entry.players.insert(
                victim_id.clone(),
                PlayerInfluence {
                    value: 30.0,
                    last_activity_tick: 10,
                    ..Default::default()
                },
            );
        }

        app.world_mut()
            .resource_mut::<bevy_ecs::event::Events<DeathEvent>>()
            .send(DeathEvent {
                target: victim_entity,
                cause: "pvp".to_string(),
                attacker: Some(killer_entity),
                attacker_player_id: Some(killer_id.clone()),
                at_tick: 100,
            });

        app.update();

        let victim_val = read_influence(&app, "pvp_zone", &victim_id);
        assert!(
            victim_val.is_some_and(|v| v.abs() < 1e-9),
            "victim 初始 30.0，击杀后应归零(0.0)，实际={:?}",
            victim_val
        );

        let killer_val = read_influence(&app, "pvp_zone", &killer_id);
        assert!(
            killer_val.is_some_and(|v| (v - KILL_INFLUENCE_GAIN).abs() < 1e-9),
            "killer 初始为0，击杀后 influence 期望={KILL_INFLUENCE_GAIN}，实际={:?}",
            killer_val
        );
        let _ = (killer_entity, victim_entity);
    }

    // ── P2 Case 3: killer 初值+击杀后精确+5.0 ────────────────────────────────

    #[test]
    fn pvp_kill_adds_exact_5_to_killer() {
        use valence::testing::create_mock_client;

        let mut app = make_pvp_app();

        let (mut killer_bundle, _) = create_mock_client("Killer3");
        killer_bundle.player.position = valence::prelude::Position::new([0.0, 66.0, 0.0]);
        let killer_entity = app.world_mut().spawn(killer_bundle).id();

        let (mut victim_bundle, _) = create_mock_client("Victim3");
        victim_bundle.player.position = valence::prelude::Position::new([2.0, 66.0, 2.0]);
        let victim_entity = app.world_mut().spawn(victim_bundle).id();

        let killer_id = canonical_player_id("Killer3");
        let victim_id = canonical_player_id("Victim3");

        // 预置 killer influence = 20.0
        {
            let mut map = app.world_mut().resource_mut::<ZoneInfluenceMap>();
            let entry = map.zones.entry("pvp_zone".to_string()).or_default();
            entry.players.insert(
                killer_id.clone(),
                PlayerInfluence {
                    value: 20.0,
                    ..Default::default()
                },
            );
        }

        app.world_mut()
            .resource_mut::<bevy_ecs::event::Events<DeathEvent>>()
            .send(DeathEvent {
                target: victim_entity,
                cause: "pvp".to_string(),
                attacker: Some(killer_entity),
                attacker_player_id: Some(killer_id.clone()),
                at_tick: 100,
            });

        app.update();

        let killer_val = read_influence(&app, "pvp_zone", &killer_id);
        assert!(
            killer_val.is_some_and(|v| (v - 25.0).abs() < 1e-9),
            "killer 初始20.0，击杀后 influence 期望=25.0(+5)，实际={:?}",
            killer_val
        );
        let _ = (victim_id, killer_entity, victim_entity);
    }

    // ── P2 Case 4: zone外击杀不调 influence ──────────────────────────────────

    #[test]
    fn pvp_kill_outside_zone_no_influence_change() {
        use valence::testing::create_mock_client;

        let mut app = make_pvp_app();

        // 位置在 zone 外（pvp_zone bounds: [-64,60,-64] to [64,80,64]）
        let (mut killer_bundle, _) = create_mock_client("KillerOut");
        killer_bundle.player.position = valence::prelude::Position::new([200.0, 66.0, 200.0]); // 在 zone 外
        let killer_entity = app.world_mut().spawn(killer_bundle).id();

        let (mut victim_bundle, _) = create_mock_client("VictimOut");
        victim_bundle.player.position = valence::prelude::Position::new([200.0, 66.0, 200.0]); // 也在 zone 外
        let victim_entity = app.world_mut().spawn(victim_bundle).id();

        let killer_id = canonical_player_id("KillerOut");
        let victim_id = canonical_player_id("VictimOut");

        app.world_mut()
            .resource_mut::<bevy_ecs::event::Events<DeathEvent>>()
            .send(DeathEvent {
                target: victim_entity,
                cause: "pvp".to_string(),
                attacker: Some(killer_entity),
                attacker_player_id: Some(killer_id.clone()),
                at_tick: 100,
            });

        app.update();

        let map = app.world().resource::<ZoneInfluenceMap>();
        assert!(
            !map.zones
                .get("pvp_zone")
                .is_some_and(|e| e.players.contains_key(&killer_id)),
            "zone 外击杀，killer influence 不应写入 pvp_zone"
        );
        assert!(
            !map.zones
                .get("pvp_zone")
                .is_some_and(|e| e.players.contains_key(&victim_id)),
            "zone 外击杀，victim influence 不应写入 pvp_zone"
        );
        let _ = (killer_entity, victim_entity);
    }

    // ── P2 Case 5: 环境死亡（attacker=None, attacker_player_id=None）不调 ──────

    #[test]
    fn environmental_death_no_influence_change() {
        use valence::testing::create_mock_client;

        let mut app = make_pvp_app();

        let (mut victim_bundle, _) = create_mock_client("VictimEnv");
        victim_bundle.player.position = valence::prelude::Position::new([0.0, 66.0, 0.0]);
        let victim_entity = app.world_mut().spawn(victim_bundle).id();

        let victim_id = canonical_player_id("VictimEnv");

        app.world_mut()
            .resource_mut::<bevy_ecs::event::Events<DeathEvent>>()
            .send(DeathEvent {
                target: victim_entity,
                cause: "bleed_out".to_string(),
                attacker: None,
                attacker_player_id: None,
                at_tick: 100,
            });

        app.update();

        let val = read_influence(&app, "pvp_zone", &victim_id);
        assert!(val.is_none(), "环境死亡不应写入 influence，实际={:?}", val);
        let _ = victim_entity;
    }

    // ── P2 Case 6: NPC杀玩家（attacker_player_id=None）不调 ──────────────────

    #[test]
    fn npc_kills_player_no_influence_change() {
        use valence::testing::create_mock_client;

        let mut app = make_pvp_app();

        // NPC entity（无 Client 组件，但有 position）
        let npc_entity = app.world_mut().spawn_empty().id();

        let (mut victim_bundle, _) = create_mock_client("VictimNPC");
        victim_bundle.player.position = valence::prelude::Position::new([0.0, 66.0, 0.0]);
        let victim_entity = app.world_mut().spawn(victim_bundle).id();

        let victim_id = canonical_player_id("VictimNPC");

        app.world_mut()
            .resource_mut::<bevy_ecs::event::Events<DeathEvent>>()
            .send(DeathEvent {
                target: victim_entity,
                cause: "npc_bite".to_string(),
                attacker: Some(npc_entity),
                attacker_player_id: None, // NPC 无 player_id
                at_tick: 100,
            });

        app.update();

        let val = read_influence(&app, "pvp_zone", &victim_id);
        assert!(
            val.is_none(),
            "NPC杀玩家（attacker_player_id=None）不应写入 influence，实际={:?}",
            val
        );
        let _ = (npc_entity, victim_entity);
    }

    // ── P2 Case 7: victim 是 NPC（无 Username）不调 ───────────────────────────

    #[test]
    fn player_kills_npc_no_influence_change() {
        use valence::testing::create_mock_client;

        let mut app = make_pvp_app();

        let (mut killer_bundle, _) = create_mock_client("KillerP");
        killer_bundle.player.position = valence::prelude::Position::new([0.0, 66.0, 0.0]);
        let killer_entity = app.world_mut().spawn(killer_bundle).id();
        let killer_id = canonical_player_id("KillerP");

        // NPC victim: 在 zone 内但无 Username
        let npc_victim = app
            .world_mut()
            .spawn(valence::prelude::Position::new([1.0, 66.0, 1.0]))
            .id();

        app.world_mut()
            .resource_mut::<bevy_ecs::event::Events<DeathEvent>>()
            .send(DeathEvent {
                target: npc_victim,
                cause: "kill".to_string(),
                attacker: Some(killer_entity),
                attacker_player_id: Some(killer_id.clone()),
                at_tick: 100,
            });

        app.update();

        let val = read_influence(&app, "pvp_zone", &killer_id);
        assert!(
            val.is_none(),
            "victim 是 NPC（无 Username），不应写入 killer influence，实际={:?}",
            val
        );
        let _ = (killer_entity, npc_victim);
    }

    // ── P2 Case 8: influence clamp 上限 ──────────────────────────────────────

    #[test]
    fn pvp_kill_influence_clamps_at_max() {
        use valence::testing::create_mock_client;

        let mut app = make_pvp_app();

        let (mut killer_bundle, _) = create_mock_client("KillerMax");
        killer_bundle.player.position = valence::prelude::Position::new([0.0, 66.0, 0.0]);
        let killer_entity = app.world_mut().spawn(killer_bundle).id();

        let (mut victim_bundle, _) = create_mock_client("VictimMax");
        victim_bundle.player.position = valence::prelude::Position::new([2.0, 66.0, 2.0]);
        let victim_entity = app.world_mut().spawn(victim_bundle).id();

        let killer_id = canonical_player_id("KillerMax");
        let victim_id = canonical_player_id("VictimMax");

        // 预置 killer influence = 98（+5 后理论值=103，应 clamp 到 100）
        {
            let mut map = app.world_mut().resource_mut::<ZoneInfluenceMap>();
            let entry = map.zones.entry("pvp_zone".to_string()).or_default();
            entry.players.insert(
                killer_id.clone(),
                PlayerInfluence {
                    value: 98.0,
                    ..Default::default()
                },
            );
        }

        app.world_mut()
            .resource_mut::<bevy_ecs::event::Events<DeathEvent>>()
            .send(DeathEvent {
                target: victim_entity,
                cause: "pvp".to_string(),
                attacker: Some(killer_entity),
                attacker_player_id: Some(killer_id.clone()),
                at_tick: 100,
            });

        app.update();

        let killer_val = read_influence(&app, "pvp_zone", &killer_id);
        assert!(
            killer_val.is_some_and(|v| (v - INFLUENCE_MAX).abs() < 1e-9),
            "killer 初始 98 +5 应 clamp 到 INFLUENCE_MAX({INFLUENCE_MAX})，实际={:?}",
            killer_val
        );
        let _ = (victim_id, killer_entity, victim_entity);
    }

    // ── P2 Case 9: 击杀致霸主易主 ────────────────────────────────────────────

    #[test]
    fn pvp_kill_triggers_dominance_switch() {
        use valence::testing::create_mock_client;

        let mut app = make_pvp_app();

        let (mut killer_bundle, _) = create_mock_client("NewDom");
        killer_bundle.player.position = valence::prelude::Position::new([0.0, 66.0, 0.0]);
        let killer_entity = app.world_mut().spawn(killer_bundle).id();

        let (mut victim_bundle, _) = create_mock_client("OldDom");
        victim_bundle.player.position = valence::prelude::Position::new([5.0, 66.0, 5.0]);
        let victim_entity = app.world_mut().spawn(victim_bundle).id();

        let killer_id = canonical_player_id("NewDom");
        let victim_id = canonical_player_id("OldDom");

        // 预置：OldDom 是霸主（influence=50），NewDom influence=40（领先不够）
        {
            let mut map = app.world_mut().resource_mut::<ZoneInfluenceMap>();
            let entry = map.zones.entry("pvp_zone".to_string()).or_default();
            entry.players.insert(
                victim_id.clone(),
                PlayerInfluence {
                    value: 50.0,
                    ..Default::default()
                },
            );
            entry.players.insert(
                killer_id.clone(),
                PlayerInfluence {
                    value: 40.0,
                    ..Default::default()
                },
            );
            entry.dominant = Some(ZoneDominance {
                char_id: victim_id.clone(),
                influence: 50.0,
                established_tick: 0,
                public_known: false,
                realm_band: None,
            });
        }

        // killer 击杀 victim → victim 归零，killer +5 → 45；
        // recompute_dominance: victim=0(< THRESHOLD), killer=45(≥ THRESHOLD=30)，
        // victim 现任跌破 DOMINANCE_DROP(20) → 允许换主，killer=45 > second=0 → 领先≥LEAD → 换主
        app.world_mut()
            .resource_mut::<bevy_ecs::event::Events<DeathEvent>>()
            .send(DeathEvent {
                target: victim_entity,
                cause: "pvp".to_string(),
                attacker: Some(killer_entity),
                attacker_player_id: Some(killer_id.clone()),
                at_tick: 100,
            });

        app.update();

        let dom = app
            .world()
            .resource::<ZoneInfluenceMap>()
            .zones
            .get("pvp_zone")
            .and_then(|e| e.dominant.as_ref())
            .cloned();

        assert!(
            dom.is_some(),
            "击杀后应有新霸主（NewDom +5=45 > OldDom 归零=0）"
        );
        assert_eq!(
            dom.as_ref().unwrap().char_id,
            killer_id,
            "新霸主应为击杀者 NewDom（influence=45），实际={:?}",
            dom.as_ref().map(|d| &d.char_id)
        );
        let _ = (killer_entity, victim_entity);
    }

    // ── P2 Case 10: victim归零后失霸主 ───────────────────────────────────────

    #[test]
    fn pvp_kill_dominant_victim_loses_dominance_when_no_challenger() {
        use valence::testing::create_mock_client;

        let mut app = make_pvp_app();

        let (mut killer_bundle, _) = create_mock_client("WeakKiller");
        killer_bundle.player.position = valence::prelude::Position::new([0.0, 66.0, 0.0]);
        let killer_entity = app.world_mut().spawn(killer_bundle).id();

        let (mut victim_bundle, _) = create_mock_client("StrongVictim");
        victim_bundle.player.position = valence::prelude::Position::new([5.0, 66.0, 5.0]);
        let victim_entity = app.world_mut().spawn(victim_bundle).id();

        let killer_id = canonical_player_id("WeakKiller");
        let victim_id = canonical_player_id("StrongVictim");

        // victim 是霸主 influence=40，killer influence=0（击杀后 killer=5，victim=0）
        // recompute: killer=5 < THRESHOLD(30) → 无霸主
        {
            let mut map = app.world_mut().resource_mut::<ZoneInfluenceMap>();
            let entry = map.zones.entry("pvp_zone".to_string()).or_default();
            entry.players.insert(
                victim_id.clone(),
                PlayerInfluence {
                    value: 40.0,
                    ..Default::default()
                },
            );
            entry.dominant = Some(ZoneDominance {
                char_id: victim_id.clone(),
                influence: 40.0,
                established_tick: 0,
                public_known: false,
                realm_band: None,
            });
        }

        app.world_mut()
            .resource_mut::<bevy_ecs::event::Events<DeathEvent>>()
            .send(DeathEvent {
                target: victim_entity,
                cause: "pvp".to_string(),
                attacker: Some(killer_entity),
                attacker_player_id: Some(killer_id.clone()),
                at_tick: 100,
            });

        app.update();

        let dom = app
            .world()
            .resource::<ZoneInfluenceMap>()
            .zones
            .get("pvp_zone")
            .and_then(|e| e.dominant.as_ref())
            .cloned();

        assert!(
            dom.is_none(),
            "victim 归零失去地位，killer 影响力(5) < DOMINANCE_THRESHOLD(30)，应无霸主；实际={:?}",
            dom.as_ref().map(|d| &d.char_id)
        );
        let _ = (killer_entity, victim_entity);
    }

    // ── P2 Case 11: emit InfluenceChangedEvent(killer + victim各一条) ─────────

    #[test]
    fn pvp_kill_emits_influence_changed_events() {
        use valence::testing::create_mock_client;

        let mut app = make_pvp_app();

        let (mut killer_bundle, _) = create_mock_client("KillerEv");
        killer_bundle.player.position = valence::prelude::Position::new([0.0, 66.0, 0.0]);
        let killer_entity = app.world_mut().spawn(killer_bundle).id();

        let (mut victim_bundle, _) = create_mock_client("VictimEv");
        victim_bundle.player.position = valence::prelude::Position::new([2.0, 66.0, 2.0]);
        let victim_entity = app.world_mut().spawn(victim_bundle).id();

        let killer_id = canonical_player_id("KillerEv");
        let victim_id = canonical_player_id("VictimEv");

        app.world_mut()
            .resource_mut::<bevy_ecs::event::Events<DeathEvent>>()
            .send(DeathEvent {
                target: victim_entity,
                cause: "pvp".to_string(),
                attacker: Some(killer_entity),
                attacker_player_id: Some(killer_id.clone()),
                at_tick: 100,
            });

        app.update();

        let events = drain_influence_events(&mut app);

        let killer_ev = events
            .iter()
            .find(|e| e.player_id == killer_id && e.zone_name == "pvp_zone");
        let victim_ev = events
            .iter()
            .find(|e| e.player_id == victim_id && e.zone_name == "pvp_zone");

        assert!(
            killer_ev.is_some(),
            "击杀者应 emit InfluenceChangedEvent，实际 events={:?}",
            events.iter().map(|e| &e.player_id).collect::<Vec<_>>()
        );
        let ke = killer_ev.unwrap();
        assert_eq!(
            ke.source,
            InfluenceSource::Combat,
            "击杀事件 source 应为 Combat，实际={:?}",
            ke.source
        );
        assert!(
            (ke.delta - KILL_INFLUENCE_GAIN).abs() < 1e-9,
            "killer delta 期望={KILL_INFLUENCE_GAIN}，实际={}",
            ke.delta
        );
        assert!(
            (ke.new_total - KILL_INFLUENCE_GAIN).abs() < 1e-9,
            "killer new_total 期望={KILL_INFLUENCE_GAIN}，实际={}",
            ke.new_total
        );

        assert!(victim_ev.is_some(), "被杀者应 emit InfluenceChangedEvent");
        let ve = victim_ev.unwrap();
        assert_eq!(
            ve.source,
            InfluenceSource::Combat,
            "被杀者事件 source 应为 Combat，实际={:?}",
            ve.source
        );
        assert!(
            ve.delta <= 0.0,
            "victim delta 应 <= 0（归零），实际={}",
            ve.delta
        );
        assert!(
            ve.new_total.abs() < 1e-9,
            "victim new_total 应为 0.0，实际={}",
            ve.new_total
        );
        let _ = (killer_entity, victim_entity);
    }

    // ── P2 Case 12: player_kills 计数 +1 ────────────────────────────────────

    #[test]
    fn pvp_kill_increments_player_kills_counter() {
        use valence::testing::create_mock_client;

        let mut app = make_pvp_app();

        let (mut killer_bundle, _) = create_mock_client("KillerKC");
        killer_bundle.player.position = valence::prelude::Position::new([0.0, 66.0, 0.0]);
        let killer_entity = app.world_mut().spawn(killer_bundle).id();

        let (mut victim_bundle, _) = create_mock_client("VictimKC");
        victim_bundle.player.position = valence::prelude::Position::new([2.0, 66.0, 2.0]);
        let victim_entity = app.world_mut().spawn(victim_bundle).id();

        let killer_id = canonical_player_id("KillerKC");

        app.world_mut()
            .resource_mut::<bevy_ecs::event::Events<DeathEvent>>()
            .send(DeathEvent {
                target: victim_entity,
                cause: "pvp".to_string(),
                attacker: Some(killer_entity),
                attacker_player_id: Some(killer_id.clone()),
                at_tick: 100,
            });

        app.update();

        let kills = read_player_kills(&app, "pvp_zone", &killer_id);
        assert_eq!(
            kills, 1,
            "首次 PvP 击杀后 player_kills 应为 1，实际={}",
            kills
        );
        let _ = (killer_entity, victim_entity);
    }

    // ── P2 Case 13: 差距<10共治（P0 emerge）——两人击杀后差 <10 → 无霸主 ─────

    #[test]
    fn pvp_kill_close_influence_no_dominance() {
        use valence::testing::create_mock_client;

        let mut app = make_pvp_app();

        // 两玩家初始各 40，A 杀 B → A=45，B=0。A 领先 45，满足霸主；
        // 换另一场景：A=35，B=40 → A 杀 B → A=40，B=0。A=40 vs next=0 → 领先≥LEAD → 有霸主。
        // 测试「差距<10无霸主」需要：两玩家击杀后 value 差<10。
        // 构造：A=35，C=32（都在 zone 内），A 杀一个无 Username 的 NPC（无效），然后验。
        // 更直接：不走击杀，直接手写 influence，验 recompute_dominance emerge 结果。
        // 但该测试应测 P2 调用后整体 emerge，不重测 recompute 内部。
        //
        // 场景：A 初始 38，B 初始 35，A 杀 C（C=0初始）→ A=43，B 仍 35；差=8 < LEAD(10) → 无霸主。
        // 但需要 B 也在同 zone 中（B entity 不发 DeathEvent）。

        let (mut a_bundle, _) = create_mock_client("PlayerA");
        a_bundle.player.position = valence::prelude::Position::new([0.0, 66.0, 0.0]);
        let a_entity = app.world_mut().spawn(a_bundle).id();

        let (mut b_bundle, _) = create_mock_client("PlayerB");
        b_bundle.player.position = valence::prelude::Position::new([1.0, 66.0, 1.0]);
        let _b_entity = app.world_mut().spawn(b_bundle).id();

        let (mut c_bundle, _) = create_mock_client("PlayerC");
        c_bundle.player.position = valence::prelude::Position::new([2.0, 66.0, 2.0]);
        let c_entity = app.world_mut().spawn(c_bundle).id();

        let a_id = canonical_player_id("PlayerA");
        let b_id = canonical_player_id("PlayerB");
        let c_id = canonical_player_id("PlayerC");

        // 预置 A=38，B=35，C=0
        {
            let mut map = app.world_mut().resource_mut::<ZoneInfluenceMap>();
            let entry = map.zones.entry("pvp_zone".to_string()).or_default();
            entry.players.insert(
                a_id.clone(),
                PlayerInfluence {
                    value: 38.0,
                    ..Default::default()
                },
            );
            entry.players.insert(
                b_id.clone(),
                PlayerInfluence {
                    value: 35.0,
                    ..Default::default()
                },
            );
        }

        // A 击杀 C → A=43，C=0；A 与 B 差=43-35=8 < LEAD(10) → 无霸主
        app.world_mut()
            .resource_mut::<bevy_ecs::event::Events<DeathEvent>>()
            .send(DeathEvent {
                target: c_entity,
                cause: "pvp".to_string(),
                attacker: Some(a_entity),
                attacker_player_id: Some(a_id.clone()),
                at_tick: 100,
            });

        app.update();

        let a_val = read_influence(&app, "pvp_zone", &a_id);
        let b_val = read_influence(&app, "pvp_zone", &b_id);

        assert!(
            a_val.is_some_and(|v| (v - 43.0).abs() < 1e-9),
            "A 初始38+5应=43，实际={:?}",
            a_val
        );
        assert!(
            b_val.is_some_and(|v| (v - 35.0).abs() < 1e-9),
            "B 不参与本次击杀应维持35，实际={:?}",
            b_val
        );

        let dom = app
            .world()
            .resource::<ZoneInfluenceMap>()
            .zones
            .get("pvp_zone")
            .and_then(|e| e.dominant.as_ref())
            .cloned();

        // A=43，B=35，差=8 < DOMINANCE_LEAD(10) → 无霸主（平局共治 emerge）
        assert!(
            dom.is_none(),
            "A(43) vs B(35) 差={}<DOMINANCE_LEAD({DOMINANCE_LEAD})，应无霸主（共治）；实际={:?}",
            43.0_f64 - 35.0,
            dom.as_ref().map(|d| &d.char_id)
        );
        let _ = (a_entity, _b_entity, c_entity, c_id);
    }
}
