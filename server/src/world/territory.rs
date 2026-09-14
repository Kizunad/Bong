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

#[cfg(test)]
#[path = "territory_tests.rs"]
mod territory_tests;
