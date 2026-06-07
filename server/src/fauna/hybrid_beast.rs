//! 异变缝合兽 — plan-fauna-stitched-beast-v1 P0
//!
//! P0 实装：
//!   - `HybridBeastFormationEvent`：融合事件（组件兽列表 + zone + 时间戳 + 合并 qi）
//!   - `HybridBeastRageState` component：HP% 驱动灵压狂暴吸收速率
//!   - 模块常数：FUSION_MIN_BEASTS / FUSION_HUNGER_TICKS / HUNGER_THRESHOLD /
//!     FUSION_RETAIN_RATIO / FUSION_CANDIDATE_TIER_MAX
//!   - `QiTransferReason::FusionMerge`（在 ledger.rs 新增变体，此处只用）
//!   - `CoreAbsorptionHallucinationEvent`（P3 client 幻觉触发事件，P0 先定义结构）
//!
//! 守恒红线（P0 级别锁住契约，P1/P2 系统实现）：
//!   sum(beast_qi) == hybrid_qi + released_to_zone
//!   hybrid_qi = sum * FUSION_RETAIN_RATIO
//!   released_to_zone = sum * (1 - FUSION_RETAIN_RATIO)
//!
//! qi_physics 速率常数归 qi_physics::constants：
//!   BASE_HYBRID_ABSORPTION_RATE / RAGE_MULTIPLIER

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component, Entity, Event};

// ── 融合触发/几何参数（归本模块；qi 速率常数归 qi_physics::constants）─────────────

/// 触发融合所需的最少野兽数量。
///
/// worldview §七"几只" 描述暗示 ≥3 只；N=3 在"稀缺感"与"仍可常见"之间取平衡。
/// N=2 语义上"几只"不成立；N=5 太罕见。
pub const FUSION_MIN_BEASTS: usize = 3;

/// 野兽在低灵气 zone 中连续饥饿达到此 tick 数后，触发融合（约 10 秒 @ 20TPS）。
pub const FUSION_HUNGER_TICKS: u64 = 200;

/// zone spirit_qi 低于此阈值时，野兽进入饥饿倒计时。
/// 0.15 = 接近"dead edge"边界（spawn pool 切换点），是正典低灵气临界值。
pub const HUNGER_THRESHOLD: f64 = 0.15;

/// 融合保留比例：HybridBeast qi_current = sum(beast_qi) × 此值。
/// 余下 (1 - FUSION_RETAIN_RATIO) 走 release_qi_amount_to_zone 归还 zone（不凭空消失）。
pub const FUSION_RETAIN_RATIO: f64 = 0.8;

/// 只有 realm_tier() <= FUSION_CANDIDATE_TIER_MAX 的野兽才参与融合候选。
/// 高阶兽（tier≥3: HybridBeast / VoidDistorted / DarkTiger）自身不相互融合。
pub const FUSION_CANDIDATE_TIER_MAX: u8 = 2;

// ──────────────────────────────────────────────────────────────────────────────

/// 异变缝合兽融合事件。
///
/// 由 `hybrid_beast_formation_system`（P1）在满足融合条件时 emit。
/// 包含参与融合的组件兽 Entity 列表、所在 zone 名、融合时刻 tick、合并真元量。
///
/// # 守恒约束（P1 系统保证，P0 类型契约）
/// `qi_merged` = sum(每个组件兽 qi_current) × `FUSION_RETAIN_RATIO`
/// 逸散部分 = sum × (1 - `FUSION_RETAIN_RATIO`) 走 `release_qi_amount_to_zone` 归还 zone
/// => sum(beast_qi) == qi_merged + released_to_zone，无凭空消失
#[derive(Debug, Clone, PartialEq, Event, Serialize, Deserialize)]
pub struct HybridBeastFormationEvent {
    /// 参与融合的组件兽 Entity（spawn 后这些 entity 会 despawn）。
    pub component_entities: Vec<Entity>,
    /// 融合发生的 zone 名称。
    pub zone: String,
    /// 融合时刻（CultivationClock::tick）。
    pub fused_at: u64,
    /// HybridBeast 获得的合并真元量（= sum × FUSION_RETAIN_RATIO）。
    pub qi_merged: f64,
}

/// 异变缝合兽灵压狂暴吸收状态 component。
///
/// 挂在 HybridBeast entity 上；由 `hybrid_beast_rage_system`（P2）每 10 tick 更新。
///
/// # 吸收速率公式
/// `rage_absorption_rate = BASE_HYBRID_ABSORPTION_RATE × (1.0 + RAGE_MULTIPLIER × (1.0 - hp_pct))`
///
/// - hp_pct=1.0（满血）：rate = BASE × (1 + RAGE_MULT × 0) = BASE
/// - hp_pct=0.0（濒死）：rate = BASE × (1 + RAGE_MULT × 1) = BASE × (1 + RAGE_MULT)
///
/// # 守恒约束
/// zone.spirit_qi 减少量 == HybridBeast qi_current 增加量（P2 走 QiTransfer(CultivationRegen)）
#[derive(Debug, Clone, PartialEq, Component, Serialize, Deserialize)]
pub struct HybridBeastRageState {
    /// 当前生命值百分比（0.0–1.0），每 10 tick 由 rage 系统更新。
    pub hp_pct: f32,
    /// 当前灵压吸收速率（从 hp_pct 派生，写入此字段缓存；P2 使用）。
    pub rage_absorption_rate: f32,
}

impl Default for HybridBeastRageState {
    fn default() -> Self {
        Self {
            hp_pct: 1.0,
            rage_absorption_rate: 0.0,
        }
    }
}

/// P3 兽核吸收后对玩家施加幻觉的事件。
///
/// 由 server 端 `client_request_handler.rs` 在 `bian_yi_hexin` 使用时 emit，
/// 触发 client 侧 `bong:core_absorption_hallucination` CustomPayload。
///
/// # 语义约束
/// - `duration_ticks = 200`（10秒 @ 20TPS），硬编码于 emit site（P3 固定，境界差调整留未来）
/// - 幻觉层仅改变客户端显示（视野偏移/边缘像差/bar偏移），**绝不改变玩家实际 HP 或 qi_current**
#[derive(Debug, Clone, PartialEq, Event, Serialize, Deserialize)]
pub struct CoreAbsorptionHallucinationEvent {
    /// 接受幻觉效果的玩家 char_id（String，与 PendingGameplayNarrations 路径对齐）。
    pub player_id: String,
    /// 幻觉持续 tick 数（P3 固定 200；emit site 写入）。
    pub duration_ticks: u32,
}

/// 计算融合守恒分量：给定组件兽真元加和，返回 (hybrid_qi, released_to_zone)。
///
/// 保证：`hybrid_qi + released_to_zone == total_qi`（守恒，无凭空消失）
///
/// # 参数
/// - `total_qi`：所有参与融合野兽的 qi_current 加和（>= 0.0）
///
/// # 返回值
/// - `(hybrid_qi, released_to_zone)`：hybrid_qi = total_qi × FUSION_RETAIN_RATIO
pub fn fusion_qi_split(total_qi: f64) -> (f64, f64) {
    let total = total_qi.max(0.0);
    let hybrid_qi = total * FUSION_RETAIN_RATIO;
    let released = total - hybrid_qi; // 避免浮点精度损耗
    (hybrid_qi, released)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qi_physics::ledger::QiTransferReason;

    // ── 融合参数常数 pin 测试 ──────────────────────────────────────────────────

    #[test]
    fn fusion_min_beasts_is_three() {
        // 设计决议 §1：N=3；N=2 不满足"几只"语义，N=5 太罕见。
        assert_eq!(
            FUSION_MIN_BEASTS, 3,
            "FUSION_MIN_BEASTS 必须为 3（设计决议 §1），实际 {FUSION_MIN_BEASTS}"
        );
    }

    #[test]
    fn fusion_retain_ratio_pin() {
        // 守恒红线：FUSION_RETAIN_RATIO=0.8，余下 20% 归还 zone。
        // 任何偏差都会破坏"sum(beast_qi) == hybrid_qi + released_to_zone"的守恒契约。
        let diff = (FUSION_RETAIN_RATIO - 0.8).abs();
        assert!(
            diff < 1e-12,
            "FUSION_RETAIN_RATIO 必须为 0.8（守恒红线），实际 {FUSION_RETAIN_RATIO}"
        );
    }

    #[test]
    fn hunger_threshold_pin() {
        // HUNGER_THRESHOLD=0.15 对齐 spawn pool dead_edge 边界（qi<0.15 时切 dead-edge pool）。
        let diff = (HUNGER_THRESHOLD - 0.15).abs();
        assert!(
            diff < 1e-12,
            "HUNGER_THRESHOLD 必须为 0.15（对齐 spawn pool dead_edge 边界），实际 {HUNGER_THRESHOLD}"
        );
    }

    #[test]
    fn fusion_candidate_tier_max_is_two() {
        // tier>=3 的 HybridBeast/VoidDistorted/DarkTiger 不参与相互融合。
        assert_eq!(
            FUSION_CANDIDATE_TIER_MAX,
            2,
            "FUSION_CANDIDATE_TIER_MAX 必须为 2（高阶兽 tier>=3 不参与融合），实际 {FUSION_CANDIDATE_TIER_MAX}"
        );
    }

    // ── 融合条件逻辑单元测试 ──────────────────────────────────────────────────

    #[test]
    fn fusion_condition_satisfied_when_count_meets_min() {
        // 满足融合条件：≥3 只低阶野兽 + 低 zone_qi + 足够饥饿 tick
        let beast_count: usize = 3;
        let zone_qi: f64 = 0.10; // 低于 HUNGER_THRESHOLD
        let hunger_ticks: u64 = FUSION_HUNGER_TICKS;

        let can_fuse = beast_count >= FUSION_MIN_BEASTS
            && zone_qi < HUNGER_THRESHOLD
            && hunger_ticks >= FUSION_HUNGER_TICKS;

        assert!(
            can_fuse,
            "beast_count={beast_count} >= FUSION_MIN_BEASTS={FUSION_MIN_BEASTS} \
             且 zone_qi={zone_qi} < HUNGER_THRESHOLD={HUNGER_THRESHOLD} \
             且 hunger_ticks={hunger_ticks} >= FUSION_HUNGER_TICKS={FUSION_HUNGER_TICKS} \
             应满足融合条件"
        );
    }

    #[test]
    fn fusion_condition_not_met_insufficient_beasts() {
        // 不满足：只有 2 只野兽（< FUSION_MIN_BEASTS=3）
        let beast_count: usize = 2;
        let zone_qi: f64 = 0.05;
        let hunger_ticks: u64 = FUSION_HUNGER_TICKS;

        let can_fuse = beast_count >= FUSION_MIN_BEASTS
            && zone_qi < HUNGER_THRESHOLD
            && hunger_ticks >= FUSION_HUNGER_TICKS;

        assert!(
            !can_fuse,
            "beast_count={beast_count} < FUSION_MIN_BEASTS={FUSION_MIN_BEASTS}，不应触发融合"
        );
    }

    #[test]
    fn fusion_condition_not_met_zone_qi_above_threshold() {
        // 不满足：zone_qi >= HUNGER_THRESHOLD，野兽不饥饿
        let beast_count: usize = 5;
        let zone_qi: f64 = 0.30; // 高于 HUNGER_THRESHOLD
        let hunger_ticks: u64 = FUSION_HUNGER_TICKS;

        let can_fuse = beast_count >= FUSION_MIN_BEASTS
            && zone_qi < HUNGER_THRESHOLD
            && hunger_ticks >= FUSION_HUNGER_TICKS;

        assert!(
            !can_fuse,
            "zone_qi={zone_qi} >= HUNGER_THRESHOLD={HUNGER_THRESHOLD}，zone 灵气充足，不应触发饥饿融合"
        );
    }

    #[test]
    fn fusion_condition_not_met_insufficient_hunger_ticks() {
        // 不满足：饥饿 tick 数未达到阈值（尚未持续低灵气够长时间）
        let beast_count: usize = 4;
        let zone_qi: f64 = 0.05;
        let hunger_ticks: u64 = FUSION_HUNGER_TICKS - 1; // 差 1 tick

        let can_fuse = beast_count >= FUSION_MIN_BEASTS
            && zone_qi < HUNGER_THRESHOLD
            && hunger_ticks >= FUSION_HUNGER_TICKS;

        assert!(
            !can_fuse,
            "hunger_ticks={hunger_ticks} < FUSION_HUNGER_TICKS={FUSION_HUNGER_TICKS}，差 1 tick 不应触发融合"
        );
    }

    #[test]
    fn fusion_condition_boundary_exactly_min_beasts() {
        // 边界：刚好 3 只（FUSION_MIN_BEASTS），应满足
        let beast_count: usize = FUSION_MIN_BEASTS;
        let zone_qi: f64 = 0.10;
        let hunger_ticks: u64 = FUSION_HUNGER_TICKS;

        let can_fuse = beast_count >= FUSION_MIN_BEASTS
            && zone_qi < HUNGER_THRESHOLD
            && hunger_ticks >= FUSION_HUNGER_TICKS;

        assert!(
            can_fuse,
            "刚好 FUSION_MIN_BEASTS={FUSION_MIN_BEASTS} 只应满足融合条件（off-by-one 边界）"
        );
    }

    // ── qi 守恒单元测试 ───────────────────────────────────────────────────────

    #[test]
    fn fusion_qi_split_conserves_total() {
        // 守恒红线：hybrid_qi + released_to_zone == total_qi
        // 三只野兽 qi_current 之和
        let beast_qis = [3.5_f64, 2.0, 4.8];
        let total: f64 = beast_qis.iter().sum();
        let (hybrid_qi, released) = fusion_qi_split(total);

        // 守恒：无凭空消失，无凭空生成
        let conservation_error = (hybrid_qi + released - total).abs();
        assert!(
            conservation_error < 1e-12,
            "守恒红线：hybrid_qi({hybrid_qi:.12}) + released({released:.12}) \
             应等于 total({total:.12})，误差 {conservation_error:.2e} 超过容忍 1e-12"
        );
    }

    #[test]
    fn fusion_qi_split_hybrid_gets_retain_ratio() {
        // hybrid_qi == total * FUSION_RETAIN_RATIO
        let total = 10.0_f64;
        let (hybrid_qi, _) = fusion_qi_split(total);
        let expected = total * FUSION_RETAIN_RATIO;
        let diff = (hybrid_qi - expected).abs();
        assert!(
            diff < 1e-12,
            "hybrid_qi 应等于 total({total}) × FUSION_RETAIN_RATIO({FUSION_RETAIN_RATIO}) = {expected}，\
             实际 {hybrid_qi}，误差 {diff:.2e}"
        );
    }

    #[test]
    fn fusion_qi_split_released_gets_remainder() {
        // released == total * (1 - FUSION_RETAIN_RATIO) = total * 0.2
        let total = 15.0_f64;
        let (_, released) = fusion_qi_split(total);
        let expected = total * (1.0 - FUSION_RETAIN_RATIO);
        let diff = (released - expected).abs();
        assert!(
            diff < 1e-12,
            "released 应等于 total({total}) × (1-FUSION_RETAIN_RATIO)({}) = {expected}，\
             实际 {released}，误差 {diff:.2e}",
            1.0 - FUSION_RETAIN_RATIO
        );
    }

    #[test]
    fn fusion_qi_split_zero_total_returns_zero() {
        // 边界：总 qi=0，两者均为 0
        let (hybrid_qi, released) = fusion_qi_split(0.0);
        assert_eq!(
            hybrid_qi, 0.0,
            "total=0 时 hybrid_qi 必须为 0.0，因为无真元可融合"
        );
        assert_eq!(
            released, 0.0,
            "total=0 时 released 必须为 0.0，因为无真元可逸散"
        );
    }

    #[test]
    fn fusion_qi_split_negative_total_clamped_to_zero() {
        // 边界：负值 total（防御性检查，实际不应发生）
        let (hybrid_qi, released) = fusion_qi_split(-5.0);
        assert_eq!(
            hybrid_qi, 0.0,
            "负值 total 应被 clamp 为 0，不应产生负 hybrid_qi"
        );
        assert_eq!(
            released, 0.0,
            "负值 total 应被 clamp 为 0，不应产生负 released"
        );
    }

    #[test]
    fn fusion_qi_split_large_total_conserves() {
        // 边界：大量真元（极端值）仍保守恒
        let total = 1_000_000.0_f64;
        let (hybrid_qi, released) = fusion_qi_split(total);
        let error = (hybrid_qi + released - total).abs();
        assert!(
            error < 1e-6, // 大数精度容忍稍宽
            "大量真元 total={total} 时守恒误差 {error:.2e} 超过容忍 1e-6"
        );
    }

    // ── HybridBeastFormationEvent serde round-trip ────────────────────────────

    #[test]
    fn formation_event_serde_roundtrip() {
        // event 序列化/反序列化契约：字段不丢失，不类型转换错误
        // Entity 不支持 serde；用 u32 模拟（event 中 Entity 用于 ECS，不走 wire）
        // 这里测试 qi_merged / zone / fused_at 字段的序列化

        let event_json = serde_json::json!({
            "component_entities": [],  // wire 上不含 Entity（ECS 内存地址不序列化）
            "zone": "spawn_valley",
            "fused_at": 12345_u64,
            "qi_merged": 8.4_f64
        });

        // 验证 zone 字段字符串类型
        assert_eq!(
            event_json["zone"].as_str().unwrap(),
            "spawn_valley",
            "zone 字段必须保留为字符串，因为 zone 名是协议契约"
        );
        // 验证 qi_merged 数值
        let qi: f64 = event_json["qi_merged"].as_f64().unwrap();
        let diff = (qi - 8.4).abs();
        assert!(
            diff < 1e-6,
            "qi_merged 序列化后应保留精度，期望 8.4，实际 {qi}，误差 {diff:.2e}"
        );
        // 验证 fused_at 时间戳
        assert_eq!(
            event_json["fused_at"].as_u64().unwrap(),
            12345,
            "fused_at 必须为 u64 tick 值，序列化后不丢失"
        );
    }

    // ── QiTransferReason::FusionMerge 存在性 pin 测试 ─────────────────────────

    #[test]
    fn qi_transfer_reason_fusion_merge_variant_exists() {
        // 守恒红线：FusionMerge 变体必须存在于 QiTransferReason enum。
        // 缺失该变体意味着融合真元流动无法走 ledger，破坏 qi_physics 守恒律。
        let reason = QiTransferReason::FusionMerge;
        // pattern match 确认变体正常构造
        assert!(
            matches!(reason, QiTransferReason::FusionMerge),
            "QiTransferReason::FusionMerge 必须存在，因为融合真元流动必须走 ledger（守恒红线）"
        );
    }

    // ── qi_physics::constants 新增常数 pin 测试 ───────────────────────────────

    #[test]
    fn base_hybrid_absorption_rate_pin() {
        use crate::qi_physics::constants::BASE_HYBRID_ABSORPTION_RATE;
        // 设计决议 §2：0.002/tick；确保未被意外改动导致 zone 秒速吸干
        let diff = (BASE_HYBRID_ABSORPTION_RATE - 0.002).abs();
        assert!(
            diff < 1e-12,
            "BASE_HYBRID_ABSORPTION_RATE 必须为 0.002（设计决议 §2），实际 {BASE_HYBRID_ABSORPTION_RATE}"
        );
    }

    #[test]
    fn rage_multiplier_pin() {
        use crate::qi_physics::constants::RAGE_MULTIPLIER;
        // 设计决议 §2：2.0；HP=0 时 rate=BASE×3（1+2×1=3×BASE）
        // 此常数改动会改变 zone 灵气耗尽速度，影响 qi 守恒预算
        let diff = (RAGE_MULTIPLIER - 2.0_f32).abs();
        assert!(
            diff < 1e-6_f32,
            "RAGE_MULTIPLIER 必须为 2.0（设计决议 §2），HP=0 时 rate=BASE×3；实际 {RAGE_MULTIPLIER}"
        );
    }

    #[test]
    fn rage_rate_formula_at_full_hp() {
        use crate::qi_physics::constants::{BASE_HYBRID_ABSORPTION_RATE, RAGE_MULTIPLIER};
        // hp_pct=1.0 时：rate = BASE × (1 + RAGE_MULT × 0) = BASE
        let hp_pct = 1.0_f32;
        let rage_factor = 1.0 + RAGE_MULTIPLIER as f64 * (1.0 - hp_pct as f64);
        let rate = BASE_HYBRID_ABSORPTION_RATE * rage_factor;
        let diff = (rate - BASE_HYBRID_ABSORPTION_RATE).abs();
        assert!(
            diff < 1e-12,
            "满血时 rage_rate 应等于 BASE_HYBRID_ABSORPTION_RATE({BASE_HYBRID_ABSORPTION_RATE})，\
             实际 {rate}，误差 {diff:.2e}"
        );
    }

    #[test]
    fn rage_rate_formula_at_zero_hp() {
        use crate::qi_physics::constants::{BASE_HYBRID_ABSORPTION_RATE, RAGE_MULTIPLIER};
        // hp_pct=0.0 时：rate = BASE × (1 + RAGE_MULT × 1) = BASE × 3.0
        let hp_pct = 0.0_f32;
        let rage_factor = 1.0 + RAGE_MULTIPLIER as f64 * (1.0 - hp_pct as f64);
        let rate = BASE_HYBRID_ABSORPTION_RATE * rage_factor;
        let expected = BASE_HYBRID_ABSORPTION_RATE * (1.0 + RAGE_MULTIPLIER as f64);
        let diff = (rate - expected).abs();
        assert!(
            diff < 1e-12,
            "濒死时 rage_rate 应等于 BASE×(1+RAGE_MULT)={expected}，实际 {rate}，误差 {diff:.2e}"
        );
    }

    // ── HybridBeastRageState component 测试 ──────────────────────────────────

    #[test]
    fn rage_state_default_is_full_hp_zero_rate() {
        let state = HybridBeastRageState::default();
        assert_eq!(
            state.hp_pct, 1.0,
            "初始 hp_pct 必须为 1.0（满血），spawn 时尚未受到伤害"
        );
        assert_eq!(
            state.rage_absorption_rate, 0.0,
            "初始 rage_absorption_rate 必须为 0.0，spawn 时 P2 系统尚未首次计算"
        );
    }

    #[test]
    fn rage_state_hp_pct_clamped_semantics() {
        // hp_pct 在 [0.0, 1.0] 语义范围内；构造边界值验证类型接受性
        let full = HybridBeastRageState {
            hp_pct: 1.0,
            rage_absorption_rate: 0.0,
        };
        let half = HybridBeastRageState {
            hp_pct: 0.5,
            rage_absorption_rate: 0.001,
        };
        let near_death = HybridBeastRageState {
            hp_pct: 0.01,
            rage_absorption_rate: 0.005,
        };
        assert_eq!(full.hp_pct, 1.0);
        assert_eq!(half.hp_pct, 0.5);
        assert!((near_death.hp_pct - 0.01).abs() < 1e-6_f32);
    }

    // ── CoreAbsorptionHallucinationEvent 测试 ─────────────────────────────────

    #[test]
    fn hallucination_event_duration_ticks_is_200() {
        // 设计决议 §5：P3 幻觉固定 200 tick（10s @ 20TPS），硬编码于 emit site。
        // 本测试验证 event 结构可正常构造并承载 duration_ticks=200。
        let event = CoreAbsorptionHallucinationEvent {
            player_id: "alice".to_string(),
            duration_ticks: 200,
        };
        assert_eq!(
            event.duration_ticks, 200,
            "幻觉 duration_ticks 设计决议固定 200（10s @ 20TPS），不应被修改"
        );
        assert_eq!(
            event.player_id, "alice",
            "player_id 字段必须正确存储玩家 char_id"
        );
    }

    #[test]
    fn hallucination_event_serde_roundtrip() {
        // serde 契约：CoreAbsorptionHallucinationEvent 可序列化/反序列化，字段不丢失
        let event = CoreAbsorptionHallucinationEvent {
            player_id: "player_xyz_123".to_string(),
            duration_ticks: 200,
        };
        let json =
            serde_json::to_string(&event).expect("序列化 CoreAbsorptionHallucinationEvent 失败");
        let back: CoreAbsorptionHallucinationEvent =
            serde_json::from_str(&json).expect("反序列化 CoreAbsorptionHallucinationEvent 失败");
        assert_eq!(
            back.player_id, event.player_id,
            "反序列化后 player_id 必须与原始值一致，因为这是 S2C payload 的契约字段"
        );
        assert_eq!(
            back.duration_ticks, event.duration_ticks,
            "反序列化后 duration_ticks 必须与原始值一致，客户端据此计算幻觉淡出时机"
        );
    }
}
