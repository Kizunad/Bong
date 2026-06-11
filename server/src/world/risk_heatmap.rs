//! plan-sou-da-che-v1 P0 — 区域风险热图（RiskHeatmap）。
//!
//! 四轴叠加模型：
//! - **灵气轴**：读 `ZoneRegistry.zone.spirit_qi`（`[-1.0, 1.0]`）。
//!   死域（≤ 0.1）= 安全低分；馈赠区（0.3-0.5）= 中险；灵眼灵泉（≥ 0.6）= 高险。
//! - **野兽密度轴**：ECS Query `FaunaTag + Position + NpcMarker`，
//!   以 `Zone::contains` 分配至 zone，按 `realm_tier` 加权。
//! - **NPC 活跃度轴**：ECS Query `NpcArchetype + Position + NpcMarker`，
//!   散修/守墓人/骨煞按权重累计。
//! - **玩家密度轴**：ECS Query `Position + ClientMarker`，可见低境玩家数量。
//!
//! **境界相关**：`risk_score` 纯函数接受 `player_realm` 参数，通灵在馈赠区 -20（食物链顶），
//! 引气同位置 +30（底端），使用 `realm_ordinal` 差值修正。
//!
//! ## 系统接线
//! `update_risk_heatmap` 挂 `Update`，节流 `RISK_HEATMAP_EVAL_INTERVAL_TICKS`（约 5s）。
//! `register` 暴露给 `world/mod.rs` 调用。

use std::collections::HashMap;

use valence::client::ClientMarker;
use valence::prelude::{
    bevy_ecs, App, DVec3, Despawned, Position, Query, Res, ResMut, Resource, Update, With, Without,
};

use crate::cultivation::components::{Cultivation, Realm};
use crate::fauna::components::FaunaTag;
use crate::npc::combat_power::realm_ordinal;
use crate::npc::lifecycle::NpcArchetype;
use crate::npc::spawn::common::NpcMarker;
use crate::world::zone::ZoneRegistry;

// ─── 常量 ─────────────────────────────────────────────────────────────────────

/// 每次 eval 间隔 ticks（约 5s，1s = 20 ticks）。
pub const RISK_HEATMAP_EVAL_INTERVAL_TICKS: u64 = 5 * 20;

/// 灵气轴高险阈值（spirit_qi ≥ 此值为高险）。
pub const QI_HIGH_DANGER_THRESHOLD: f64 = 0.6;

/// 灵气轴中险下限。
pub const QI_MID_DANGER_LOW: f64 = 0.3;

/// 灵气轴中险上限。
pub const QI_MID_DANGER_HIGH: f64 = 0.5;

/// 灵气轴死域上限（spirit_qi ≤ 此值为死域，低危）。
pub const QI_DEAD_ZONE_THRESHOLD: f64 = 0.1;

/// 灵气轴高险时的基础得分贡献。
pub const QI_SCORE_HIGH: f32 = 30.0;

/// 灵气轴中险时的基础得分贡献。
pub const QI_SCORE_MID: f32 = 15.0;

/// 灵气轴死域时的基础得分贡献（接近安全）。
pub const QI_SCORE_DEAD: f32 = 5.0;

/// 灵气轴普通正值（0.1 < qi < 0.3）时的得分贡献。
pub const QI_SCORE_NORMAL: f32 = 10.0;

/// 野兽密度轴：每只野兽的基础得分（realm_tier 加权：tier * multiplier）。
pub const FAUNA_SCORE_PER_TIER: f32 = 5.0;

/// 野兽密度轴：每个 zone 野兽分的上限贡献。
pub const FAUNA_SCORE_CAP: f32 = 30.0;

/// NPC 活跃度轴：高危 NPC（GuardianRelic / SkullFiend）每只得分。
pub const NPC_SCORE_HIGH_DANGER: f32 = 8.0;

/// NPC 活跃度轴：散修类 NPC（Rogue / Commoner / Disciple）每只得分。
pub const NPC_SCORE_HUMANOID: f32 = 3.0;

/// NPC 活跃度轴：该轴上限贡献。
pub const NPC_SCORE_CAP: f32 = 25.0;

/// 玩家密度轴：每个可见低境玩家的得分。
pub const PLAYER_SCORE_PER_PLAYER: f32 = 4.0;

/// 玩家密度轴：该轴上限贡献。
pub const PLAYER_SCORE_CAP: f32 = 15.0;

/// 境界修正：通灵（Spirit，ordinal=4）在中险区 -20（食物链顶）。
pub const REALM_SPIRIT_MID_BONUS: i32 = -20;

/// 境界修正：引气（Induce，ordinal=1）在中险区 +30（底端）。
pub const REALM_INDUCE_MID_PENALTY: i32 = 30;

// ─── 数据类型 ──────────────────────────────────────────────────────────────────

/// 四轴风险分量（各轴归一化到 0.0–1.0 之前的原始计算值，最终合并为 RiskScore）。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RiskAxes {
    /// 灵气轴原始得分（0.0 – QI_SCORE_HIGH）。
    pub qi: f32,
    /// 野兽密度轴原始得分（0.0 – FAUNA_SCORE_CAP）。
    pub fauna: f32,
    /// NPC 活跃度轴原始得分（0.0 – NPC_SCORE_CAP）。
    pub npc: f32,
    /// 玩家密度轴原始得分（0.0 – PLAYER_SCORE_CAP）。
    pub player: f32,
}

/// 综合风险评分（0–100）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct RiskScore(pub u8);

/// 区域风险热图 Bevy Resource。
/// 每次 `update_risk_heatmap` 系统 tick 时，按 ZoneRegistry 所有 zone 更新。
#[derive(Debug, Default, Resource)]
pub struct RiskHeatmap {
    pub by_zone: HashMap<String, RiskAxes>,
    /// 上次 eval 时的 tick 计数（用于节流）。
    last_eval_tick: u64,
}

impl RiskHeatmap {
    /// 获取指定 zone 的四轴数据（若 zone 未被计算过则返回全零默认值）。
    pub fn axes_for_zone(&self, zone_name: &str) -> RiskAxes {
        self.by_zone.get(zone_name).copied().unwrap_or_default()
    }
}

// ─── 核心纯函数 ────────────────────────────────────────────────────────────────

/// 将四轴原始得分 + 玩家境界信息合并为 RiskScore(0-100)。
///
/// **境界修正逻辑**：
/// - 以参考境界 Condense(凝脉，ordinal=2) 为基准：
///   - 玩家境界 > 基准（如 Spirit=4）：每级扣 `(ordinal_diff) * 4` 分（食物链顶，危险低）。
///   - 玩家境界 < 基准（如 Induce=1）：每级加 `(ordinal_diff) * 6` 分（处于弱势，危险高）。
/// - 灵气处于中险区（spirit_qi 0.3-0.5）时额外境界修正：通灵 -20，引气 +30。
/// - 最终 clamp 到 `[0, 100]`。
pub fn risk_score(axes: RiskAxes, player_realm: Realm) -> RiskScore {
    let base_score = axes.qi + axes.fauna + axes.npc + axes.player;

    // 境界修正：以凝脉 (ordinal=2) 为基准
    let ordinal = realm_ordinal(player_realm) as i32;
    let base_ordinal = realm_ordinal(Realm::Condense) as i32;
    let realm_diff = ordinal - base_ordinal;
    let realm_adjustment = if realm_diff > 0 {
        // 高于凝脉 → 安全性提升
        -(realm_diff * 4)
    } else {
        // 低于凝脉 → 危险性提升
        (-realm_diff) * 6
    };

    // 灵气中险区额外境界修正
    let mid_zone_adjustment = if axes.qi >= QI_SCORE_MID - 0.1 && axes.qi <= QI_SCORE_MID + 0.1 {
        // 灵气分接近中险基准值时触发
        match player_realm {
            Realm::Spirit | Realm::Void => REALM_SPIRIT_MID_BONUS,
            Realm::Induce | Realm::Awaken => REALM_INDUCE_MID_PENALTY,
            _ => 0,
        }
    } else {
        0
    };

    let final_score = (base_score as i32 + realm_adjustment + mid_zone_adjustment).clamp(0, 100);

    RiskScore(final_score as u8)
}

/// 根据 zone 的 spirit_qi 计算灵气轴原始得分。
///
/// - `spirit_qi` ≤ 0.1（死域）→ QI_SCORE_DEAD（低危）
/// - `spirit_qi` 0.1–0.3（普通）→ QI_SCORE_NORMAL
/// - `spirit_qi` 0.3–0.5（馈赠区）→ QI_SCORE_MID（中险）
/// - `spirit_qi` ≥ 0.6（灵眼灵泉）→ QI_SCORE_HIGH（高险）
/// - 负值（浊气/末法区域）→ QI_SCORE_DEAD（低危，无灵气意味无竞争）
pub fn qi_axis_score(spirit_qi: f64) -> f32 {
    if spirit_qi <= QI_DEAD_ZONE_THRESHOLD {
        QI_SCORE_DEAD
    } else if spirit_qi < QI_MID_DANGER_LOW {
        QI_SCORE_NORMAL
    } else if spirit_qi <= QI_MID_DANGER_HIGH {
        QI_SCORE_MID
    } else if spirit_qi >= QI_HIGH_DANGER_THRESHOLD {
        QI_SCORE_HIGH
    } else {
        // 0.5 < qi < 0.6 — 插值
        let t = (spirit_qi - QI_MID_DANGER_HIGH) / (QI_HIGH_DANGER_THRESHOLD - QI_MID_DANGER_HIGH);
        QI_SCORE_MID + (QI_SCORE_HIGH - QI_SCORE_MID) * t as f32
    }
}

/// NPC 是否为高危类型（GuardianRelic / SkullFiend / DyingElder）。
pub fn is_high_danger_npc(archetype: NpcArchetype) -> bool {
    matches!(
        archetype,
        NpcArchetype::GuardianRelic | NpcArchetype::SkullFiend | NpcArchetype::DyingElder
    )
}

/// NPC 是否为散修类（Rogue / Commoner / Disciple / Daoxiang / Zhinian）。
pub fn is_humanoid_npc(archetype: NpcArchetype) -> bool {
    matches!(
        archetype,
        NpcArchetype::Rogue
            | NpcArchetype::Commoner
            | NpcArchetype::Disciple
            | NpcArchetype::Daoxiang
            | NpcArchetype::Zhinian
    )
}

// ─── Bevy System ──────────────────────────────────────────────────────────────

type ActiveNpcFilter = (With<NpcMarker>, Without<Despawned>);
type FaunaQuery<'w, 's> = Query<'w, 's, (&'static Position, &'static FaunaTag), ActiveNpcFilter>;
type NpcArchetypeQuery<'w, 's> =
    Query<'w, 's, (&'static Position, &'static NpcArchetype), ActiveNpcFilter>;
type PlayerCultivationQuery<'w, 's> =
    Query<'w, 's, (&'static Position, &'static Cultivation), With<ClientMarker>>;

/// `Update` 系统：按 RISK_HEATMAP_EVAL_INTERVAL_TICKS 节流，
/// 读取 ZoneRegistry + ECS 查询，写入 RiskHeatmap。
pub fn update_risk_heatmap(
    mut heatmap: ResMut<RiskHeatmap>,
    zones: Option<Res<ZoneRegistry>>,
    fauna_query: FaunaQuery,
    npc_query: NpcArchetypeQuery,
    player_query: PlayerCultivationQuery,
) {
    // 简单 tick 计数节流（用 by_zone.len() 作为代理指示器不够准确，
    // 此处使用 last_eval_tick 字段做 20-tick 粒度节流）。
    // 由于 Bevy 的 Res<Time> 未在此系统参数列表中（避免与现有 territory.rs 约束冲突），
    // 此处以 update 调用次数作为粗粒度时钟。
    heatmap.last_eval_tick = heatmap.last_eval_tick.wrapping_add(1);
    if heatmap.last_eval_tick % RISK_HEATMAP_EVAL_INTERVAL_TICKS != 0 {
        return;
    }

    let Some(zones) = zones else {
        return;
    };

    // 按 zone 名称积累各轴数据
    let mut fauna_score: HashMap<String, f32> = HashMap::new();
    let mut npc_score: HashMap<String, f32> = HashMap::new();
    let mut player_score: HashMap<String, f32> = HashMap::new();

    // 野兽密度轴
    for (pos, fauna_tag) in &fauna_query {
        let p: DVec3 = pos.0;
        for zone in &zones.zones {
            if zone.contains(p) {
                let tier_weight =
                    (fauna_tag.beast_kind.realm_tier() as f32 + 1.0) * FAUNA_SCORE_PER_TIER;
                let entry = fauna_score.entry(zone.name.clone()).or_insert(0.0);
                *entry = (*entry + tier_weight).min(FAUNA_SCORE_CAP);
                break; // 只归属最小 zone（zone.rs find_zone 语义）
            }
        }
    }

    // NPC 活跃度轴
    for (pos, archetype) in &npc_query {
        let p: DVec3 = pos.0;
        for zone in &zones.zones {
            if zone.contains(p) {
                let score = if is_high_danger_npc(*archetype) {
                    NPC_SCORE_HIGH_DANGER
                } else if is_humanoid_npc(*archetype) {
                    NPC_SCORE_HUMANOID
                } else {
                    0.0
                };
                if score > 0.0 {
                    let entry = npc_score.entry(zone.name.clone()).or_insert(0.0);
                    *entry = (*entry + score).min(NPC_SCORE_CAP);
                }
                break;
            }
        }
    }

    // 玩家密度轴（所有在线玩家）
    for (pos, _cultivation) in &player_query {
        let p: DVec3 = pos.0;
        for zone in &zones.zones {
            if zone.contains(p) {
                let entry = player_score.entry(zone.name.clone()).or_insert(0.0);
                *entry = (*entry + PLAYER_SCORE_PER_PLAYER).min(PLAYER_SCORE_CAP);
                break;
            }
        }
    }

    // 更新 heatmap
    heatmap.by_zone.clear();
    for zone in &zones.zones {
        let axes = RiskAxes {
            qi: qi_axis_score(zone.spirit_qi),
            fauna: fauna_score.get(&zone.name).copied().unwrap_or(0.0),
            npc: npc_score.get(&zone.name).copied().unwrap_or(0.0),
            player: player_score.get(&zone.name).copied().unwrap_or(0.0),
        };
        heatmap.by_zone.insert(zone.name.clone(), axes);
    }
}

/// 注册 RiskHeatmap resource 和 update system。
pub fn register(app: &mut App) {
    app.insert_resource(RiskHeatmap::default())
        .add_systems(Update, update_risk_heatmap);
}

// ─── 测试 ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── risk_score 境界差异测试 ────────────────────────────────────────────────

    #[test]
    fn risk_score_by_realm_spirit_lower_than_induce_in_mid_zone() {
        // 同一中险区：通灵应比引气的 RiskScore 低
        let mid_axes = RiskAxes {
            qi: QI_SCORE_MID,
            fauna: 0.0,
            npc: 0.0,
            player: 0.0,
        };
        let score_spirit = risk_score(mid_axes, Realm::Spirit).0 as i32;
        let score_induce = risk_score(mid_axes, Realm::Induce).0 as i32;

        assert!(
            score_spirit < score_induce,
            "通灵在中险区应比引气风险低，spirit={score_spirit} induce={score_induce}"
        );
    }

    #[test]
    fn risk_score_by_realm_spirit_bonus_in_mid_zone() {
        // 通灵在中险区：境界 bonus 修正 -20（Spirit=-20, Condense=0）
        let mid_axes = RiskAxes {
            qi: QI_SCORE_MID,
            fauna: 0.0,
            npc: 0.0,
            player: 0.0,
        };
        let score_spirit = risk_score(mid_axes, Realm::Spirit).0 as i32;
        let score_condense = risk_score(mid_axes, Realm::Condense).0 as i32;

        // Spirit: ordinal=4, diff=+2 vs Condense → realm_adjustment = -(2*4)=-8
        // mid_zone_adjustment = REALM_SPIRIT_MID_BONUS = -20
        // 总调整 = -28
        assert!(
            score_spirit < score_condense,
            "通灵在中险区应低于凝脉，spirit={score_spirit} condense={score_condense}"
        );
    }

    #[test]
    fn risk_score_by_realm_induce_penalty_in_mid_zone() {
        // 引气在中险区：境界 penalty +30（Induce vs Condense）
        let mid_axes = RiskAxes {
            qi: QI_SCORE_MID,
            fauna: 0.0,
            npc: 0.0,
            player: 0.0,
        };
        let score_induce = risk_score(mid_axes, Realm::Induce).0 as i32;
        let score_condense = risk_score(mid_axes, Realm::Condense).0 as i32;

        // Induce: ordinal=1, diff=-1 vs Condense → realm_adjustment = 1*6=+6
        // mid_zone_adjustment = REALM_INDUCE_MID_PENALTY = +30
        // 总调整 = +36
        assert!(
            score_induce > score_condense,
            "引气在中险区应高于凝脉，induce={score_induce} condense={score_condense}"
        );
    }

    #[test]
    fn risk_score_by_realm_all_variants_produce_valid_score() {
        // 每个境界变体都产生合法 RiskScore（0-100）
        let axes = RiskAxes {
            qi: 15.0,
            fauna: 10.0,
            npc: 8.0,
            player: 4.0,
        };
        let realms = [
            Realm::Awaken,
            Realm::Induce,
            Realm::Condense,
            Realm::Solidify,
            Realm::Spirit,
            Realm::Void,
        ];
        for realm in realms {
            let score = risk_score(axes, realm).0;
            assert!(
                score <= 100,
                "RiskScore 必须 ≤ 100，realm={realm:?} score={score}"
            );
        }
    }

    #[test]
    fn risk_score_by_realm_void_lowest_risk() {
        // 化虚应是所有境界中风险最低的
        let axes = RiskAxes {
            qi: 20.0,
            fauna: 10.0,
            npc: 5.0,
            player: 0.0,
        };
        let score_void = risk_score(axes, Realm::Void).0;
        let score_awaken = risk_score(axes, Realm::Awaken).0;

        assert!(
            score_void < score_awaken,
            "化虚应比醒灵风险更低，void={score_void} awaken={score_awaken}"
        );
    }

    // ── risk_heatmap_four_axis_overlay 测试 ───────────────────────────────────

    #[test]
    fn risk_heatmap_four_axis_overlay_all_zero_yields_low_score() {
        // 四轴全零（空 zone，无野兽无 NPC 无玩家，但灵气 = 死域）：低分
        let axes = RiskAxes {
            qi: QI_SCORE_DEAD,
            fauna: 0.0,
            npc: 0.0,
            player: 0.0,
        };
        let score = risk_score(axes, Realm::Condense).0;
        assert!(score <= 20, "四轴极低时 RiskScore 应 ≤ 20，实际={score}");
    }

    #[test]
    fn risk_heatmap_four_axis_overlay_all_max_clamps_to_100() {
        // 四轴全满时，RiskScore 应 clamp 到 100
        let axes = RiskAxes {
            qi: QI_SCORE_HIGH,
            fauna: FAUNA_SCORE_CAP,
            npc: NPC_SCORE_CAP,
            player: PLAYER_SCORE_CAP,
        };
        let score = risk_score(axes, Realm::Awaken).0;
        assert_eq!(
            score, 100,
            "四轴全满 RiskScore 应 clamp 到 100，实际={score}"
        );
    }

    #[test]
    fn risk_heatmap_four_axis_overlay_single_qi_axis() {
        // 只有灵气轴有值，其他轴为零
        let dead_zone_axes = RiskAxes {
            qi: QI_SCORE_DEAD,
            fauna: 0.0,
            npc: 0.0,
            player: 0.0,
        };
        let high_qi_axes = RiskAxes {
            qi: QI_SCORE_HIGH,
            fauna: 0.0,
            npc: 0.0,
            player: 0.0,
        };
        let score_dead = risk_score(dead_zone_axes, Realm::Condense).0;
        let score_high = risk_score(high_qi_axes, Realm::Condense).0;

        assert!(
            score_high > score_dead,
            "高灵气区应比死域风险高，high={score_high} dead={score_dead}"
        );
    }

    #[test]
    fn risk_heatmap_four_axis_overlay_dead_zone_is_low_danger() {
        // 死域（spirit_qi ≤ 0.1）应得低分
        let qi_score = qi_axis_score(-0.5); // 负灵气
        assert_eq!(
            qi_score, QI_SCORE_DEAD,
            "负灵气应等于死域得分 {QI_SCORE_DEAD}，实际={qi_score}"
        );

        let qi_score_low = qi_axis_score(0.05);
        assert_eq!(
            qi_score_low, QI_SCORE_DEAD,
            "spirit_qi=0.05 应等于死域得分，实际={qi_score_low}"
        );
    }

    #[test]
    fn risk_heatmap_four_axis_overlay_single_fauna_axis() {
        // 只有野兽轴满值，其他轴为零
        let fauna_axes = RiskAxes {
            qi: 0.0,
            fauna: FAUNA_SCORE_CAP,
            npc: 0.0,
            player: 0.0,
        };
        let score = risk_score(fauna_axes, Realm::Condense).0;
        assert!(score > 0, "野兽轴满值时 RiskScore 应 > 0，实际={score}");
    }

    #[test]
    fn risk_heatmap_four_axis_overlay_empty_zone_defaults_zero() {
        // 空 zone 无数据时，axes_for_zone 返回全零默认值
        let heatmap = RiskHeatmap::default();
        let axes = heatmap.axes_for_zone("nonexistent_zone");
        assert_eq!(axes, RiskAxes::default(), "未知 zone 应返回全零 RiskAxes");
    }

    // ── qi_axis_score 边界测试 ─────────────────────────────────────────────────

    #[test]
    fn qi_axis_score_thresholds() {
        // 死域边界
        assert_eq!(qi_axis_score(0.1), QI_SCORE_DEAD);
        assert_eq!(qi_axis_score(0.0), QI_SCORE_DEAD);
        assert_eq!(qi_axis_score(-1.0), QI_SCORE_DEAD);

        // 普通区
        assert_eq!(qi_axis_score(0.2), QI_SCORE_NORMAL);

        // 中险区
        assert_eq!(qi_axis_score(0.3), QI_SCORE_MID);
        assert_eq!(qi_axis_score(0.5), QI_SCORE_MID);

        // 高险区
        assert_eq!(qi_axis_score(0.6), QI_SCORE_HIGH);
        assert_eq!(qi_axis_score(1.0), QI_SCORE_HIGH);
    }

    #[test]
    fn qi_axis_score_interpolates_between_mid_and_high() {
        // 0.5 < qi < 0.6 时插值
        let score = qi_axis_score(0.55);
        assert!(
            score > QI_SCORE_MID && score < QI_SCORE_HIGH,
            "0.55 应在中险与高险之间插值，实际={score}"
        );
    }

    // ── NPC 分类测试 ───────────────────────────────────────────────────────────

    #[test]
    fn npc_archetype_high_danger_classification() {
        assert!(is_high_danger_npc(NpcArchetype::GuardianRelic));
        assert!(is_high_danger_npc(NpcArchetype::SkullFiend));
        assert!(is_high_danger_npc(NpcArchetype::DyingElder));
        assert!(!is_high_danger_npc(NpcArchetype::Rogue));
        assert!(!is_high_danger_npc(NpcArchetype::Zombie));
    }

    #[test]
    fn npc_archetype_humanoid_classification() {
        assert!(is_humanoid_npc(NpcArchetype::Rogue));
        assert!(is_humanoid_npc(NpcArchetype::Commoner));
        assert!(is_humanoid_npc(NpcArchetype::Disciple));
        assert!(is_humanoid_npc(NpcArchetype::Daoxiang));
        assert!(is_humanoid_npc(NpcArchetype::Zhinian));
        assert!(!is_humanoid_npc(NpcArchetype::GuardianRelic));
        assert!(!is_humanoid_npc(NpcArchetype::SkullFiend));
    }

    // ── RiskScore 单轴 0 边界 ─────────────────────────────────────────────────

    #[test]
    fn risk_score_single_axis_zero_boundary() {
        // 所有轴为零时，凝脉应得分接近 0
        let axes = RiskAxes::default();
        let score = risk_score(axes, Realm::Condense).0;
        assert_eq!(score, 0, "全零轴凝脉 RiskScore 应为 0，实际={score}");
    }

    // ── 境界 ordinal 顺序验证 ─────────────────────────────────────────────────

    #[test]
    fn realm_ordinal_order_consistent() {
        assert!(realm_ordinal(Realm::Awaken) < realm_ordinal(Realm::Induce));
        assert!(realm_ordinal(Realm::Induce) < realm_ordinal(Realm::Condense));
        assert!(realm_ordinal(Realm::Condense) < realm_ordinal(Realm::Solidify));
        assert!(realm_ordinal(Realm::Solidify) < realm_ordinal(Realm::Spirit));
        assert!(realm_ordinal(Realm::Spirit) < realm_ordinal(Realm::Void));
    }
}
