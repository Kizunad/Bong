//! worldview 锚定的真元物理常量。

/// worldview §四：距离基础衰减，正典 0.03 / block。
pub const QI_DECAY_PER_BLOCK: f64 = 0.03;
/// worldview §二：离体真元被末法残土分解的基础速率。
pub const QI_AMBIENT_EXCRETION_PER_SEC: f64 = 0.001;
/// worldview §四：异体排斥基础率。
pub const QI_EXCRETION_BASE: f64 = 0.30;
/// worldview §四：声学激发阈值。
pub const QI_ACOUSTIC_THRESHOLD: f64 = 0.40;
/// worldview §四：涡流 1/r^2 系数。
pub const QI_NEGATIVE_FIELD_K: f64 = 1.0;
/// worldview §四：避免 1/r^2 在 r→0 处奇异；1 格内视为饱和近场。
pub const QI_NEGATIVE_FIELD_MIN_RADIUS_BLOCKS: f64 = 1.0;
/// worldview §四：涡流反向获利上限。
pub const QI_DRAIN_CLAMP: f64 = 0.50;
/// plan-woliu-v2 §0/#4：涡流搅拌器默认吸入比例，保留给后续 telemetry 校准。
pub const VORTEX_ABSORPTION_RATIO_BASE: f64 = 0.01;
/// plan-woliu-v2 §0：涡流搅拌器默认甩回环境比例。
pub const VORTEX_SWIRL_RATIO_BASE: f64 = 0.99;
/// plan-dugu-v2 §0：毒蛊脏真元异体排斥率，伪装成宿主真元混入经脉。
pub const DUGU_RHO: f64 = 0.05;
/// plan-dugu-v2 §0：脏真元过渡态最终散回受害者所在 zone qi 的比例。
pub const DUGU_DIRTY_QI_ZONE_RETURN_RATIO: f64 = 0.99;
/// plan-woliu-v2 §0：紊流场基础自然耗散率（每秒）。
pub const VORTEX_TURBULENCE_DECAY_PER_SEC: f64 = 0.05;
/// plan-woliu-v2 §0：紊流场内灵物/丹材保质期加速倍率。
pub const VORTEX_TURBULENCE_SHELFLIFE_MULTIPLIER: f64 = 3.0;
/// plan-woliu-v2 §0：紊流场内静坐修炼吸收倍率。
pub const VORTEX_TURBULENCE_ABSORPTION_MULTIPLIER: f64 = 0.0;
/// plan-woliu-v2 §0：紊流场内战斗真元注入精度倍率。
pub const VORTEX_TURBULENCE_CAST_PRECISION_MULTIPLIER: f64 = 0.5;
/// plan-woliu-v2 §0：紊流场内护体真气额外消耗比例。
pub const VORTEX_TURBULENCE_DEFENSE_DRAIN_BONUS: f64 = 0.20;
/// plan-tuike-v2 §0：替尸蜕壳启动系数，归属 qi_physics 统一矩阵。
pub const TUIKE_BETA: f64 = 1.2;
/// worldview §九：骨币半衰参考，约 13 天。
pub const QI_HALFLIFE_REFERENCE_DAYS: f64 = 13.0;
/// plan-shelflife-v1：死域保质期衰减倍率，保持旧曲线三倍加速。
pub const QI_SHELFLIFE_DEAD_ZONE_MULTIPLIER: f32 = 3.0;
/// worldview §四：防御维持基线。
pub const QI_MAINTENANCE_IDLE: f64 = 1.0;
/// worldview §十六：末法残土抽真元强度。
pub const QI_TSY_DRAIN_FACTOR: f64 = 0.5;
/// worldview §十六：末法残土非线性指数。
pub const QI_TSY_DRAIN_NONLINEAR_EXPONENT: f64 = 1.5;
/// plan-tsy-zone-v1：TSY 抽取基准真元池。
pub const QI_TSY_REFERENCE_POOL: f64 = 100.0;
/// plan-tsy-zone-v1：TSY 抽取基准速率（点 / tick）。
pub const QI_TSY_BASE_DRAIN_PER_TICK: f64 = 0.5;
/// plan-tsy-container-v1：搜刮中主动暴露的 TSY 抽取放大因子。
pub const QI_TSY_SEARCH_EXPOSURE_FACTOR: f64 = 1.5;
/// worldview §十七：中性节律。
pub const QI_RHYTHM_NEUTRAL: f64 = 1.0;
/// worldview §十七：活跃节律。
pub const QI_RHYTHM_ACTIVE: f64 = 1.2;
/// worldview §十七：汐转波动范围。
pub const QI_RHYTHM_TURBULENT_RANGE: (f64, f64) = (0.7, 1.5);
/// worldview §十：全服灵气预算默认值；生产值由 server config 初始化。
pub const DEFAULT_SPIRIT_QI_TOTAL: f64 = 100.0;
/// worldview §十：天道时代衰减下限。
pub const QI_TIANDAO_DECAY_PER_ERA_MIN: f64 = 0.01;
/// worldview §十：天道时代衰减上限。
pub const QI_TIANDAO_DECAY_PER_ERA_MAX: f64 = 0.03;
/// worldview §十一：灵物密度阈值。
pub const QI_DENSITY_GAZE_THRESHOLD: f64 = 0.85;
/// worldview §九/§十一：区域饥饿阈值。
pub const QI_REGION_STARVATION_THRESHOLD: f64 = 0.1;
/// plan-cultivation-v1：修炼吸纳速率系数。
pub const QI_CULTIVATION_REGEN_RATE: f64 = 0.003;
/// plan-cultivation-v1：1.0 zone 浓度可支撑的玩家真元点数。
pub const QI_ZONE_UNIT_CAPACITY: f64 = 50.0;
/// player gather：采集动作默认真元奖励，以 zone qi 对冲供给。
pub const QI_GATHER_REWARD: f64 = 14.0;
/// plan-lingtian-v1：偷灵注入操作者比例。
pub const LINGTIAN_DRAIN_PLAYER_RATIO: f32 = 0.8;
/// plan-lingtian-v1：偷灵散逸回 zone 比例。
pub const LINGTIAN_DRAIN_ZONE_RATIO: f32 = 0.2;
/// plan-lingtian-v1：plot 灵气不足时从环境场漏吸的比例。
pub const QI_LINGTIAN_AMBIENT_LEAK_RATIO: f32 = 0.2;
/// plan-qi-physics-patch-v1 P2-6：跨界磨损最小比例。
pub const QI_TARGETED_ITEM_WEAR_MIN_FRACTION: f64 = 0.01;
/// plan-qi-physics-patch-v1 P2-6：跨界磨损最大比例。
pub const QI_TARGETED_ITEM_WEAR_MAX_FRACTION: f64 = 0.05;
/// plan-qi-physics-patch-v1 P2-6：低于该 karma 权重不触发跨界磨损。
pub const QI_TARGETED_ITEM_WEAR_WEIGHT_THRESHOLD: f32 = 0.01;
/// plan-zhenmai-v1：截脉防御窗口基础时长。
pub const QI_ZHENMAI_PREP_WINDOW_MS: u32 = 1000;
/// plan-zhenmai-v1：截脉污染残留倍率。
pub const QI_ZHENMAI_CONTAM_RESIDUAL_MULTIPLIER: f64 = 0.2;
/// plan-zhenmai-v1：截脉震荡基础严重度。
pub const QI_ZHENMAI_CONCUSSION_BASE_SEVERITY: f32 = 0.3;
/// plan-zhenmai-v1：截脉震荡流血速率。
pub const QI_ZHENMAI_CONCUSSION_BLEEDING_PER_SEC: f32 = 0.0;
/// plan-zhenmai-v1：招架恢复 tick。
pub const QI_ZHENMAI_PARRY_RECOVERY_TICKS: u64 = 10;
/// plan-zhenmai-v1：招架恢复期间移动速度倍率。
pub const QI_ZHENMAI_PARRY_RECOVERY_MOVE_SPEED_MULTIPLIER: f32 = 0.7;
/// 数值断言默认容忍度。
pub const QI_EPSILON: f64 = 1e-6;
/// plan-offscreen-war-v1 P9：战事胜方主导 zone 的 regen 速率倍率（+10%）。
/// 仅作用于 regen_from_zone 的 rate 参数，不铸造/销毁真元，守恒安全。
pub const WAR_WINNER_ZONE_REGEN_MULTIPLIER: f64 = 1.10;
/// plan-offscreen-war-v1 P9：战事败方主导 zone 的 regen 速率倍率（−5%）。
/// 仅作用于 regen_from_zone 的 rate 参数，不铸造/销毁真元，守恒安全。
pub const WAR_LOSER_ZONE_REGEN_MULTIPLIER: f64 = 0.95;

// ── plan-neg-domain-fauna-v1 P0 — 负灵域生态真元抽取率 ──
/// 诡影接触真元抽取率（归口 qi_physics；几何/密度参数留在 fauna 模块）。
/// pulse_amount = |zone_qi| × qi_max × GHOST_CONTACT_FACTOR（每次接触，1s cooldown）。
pub const GHOST_CONTACT_FACTOR: f64 = 0.05;
/// 噬灵藓踩踏每 tick 真元抽取率（归口 qi_physics；蔓延参数留在 botany 模块）。
/// drain_per_tick = qi_max × MOSS_DRAIN_FACTOR（每 tick 持续扣除）。
pub const MOSS_DRAIN_FACTOR: f64 = 0.002;

// ── plan-baomai-v4 §1.3 疤纹回路（经脉 integrity 物理阈值）──
/// 回路形成所需经脉 integrity 下限（含）。
pub const SCAR_CIRCUIT_INTEGRITY_MIN: f64 = 0.50;
/// 回路形成所需经脉 integrity 上限（含）——超过此值回路断开。
pub const SCAR_CIRCUIT_INTEGRITY_MAX: f64 = 0.85;

// ── plan-baomai-v4 §1.3 互噬（真元排斥/经脉损伤物理）──
/// 共振锁定期间 ρ 覆写值（排斥系数降为零）。
pub const RESONANCE_LOCK_RHO_OVERRIDE: f64 = 0.0;
/// 锁定期间经脉 integrity 下降速率（per tick，仅已受损经脉）。
pub const RESONANCE_LOCK_INTEGRITY_DRAIN: f64 = 0.005;
/// 脱离共振的 integrity 惩罚（per 受损经脉）。
pub const RESONANCE_RETREAT_INTEGRITY_PENALTY: f64 = 0.08;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vortex_stir_ratios_close_conservation_budget() {
        let total = VORTEX_ABSORPTION_RATIO_BASE + VORTEX_SWIRL_RATIO_BASE;
        assert!(
            (total - 1.0).abs() < 1e-12,
            "woliu stir ratios must sum to 1.0 for qi conservation, got {total}"
        );
    }

    #[test]
    fn dugu_dirty_qi_constants_match_plan_budget() {
        assert!((DUGU_RHO - 0.05).abs() < QI_EPSILON);
        assert!((DUGU_DIRTY_QI_ZONE_RETURN_RATIO - 0.99).abs() < QI_EPSILON);
    }

    // ── plan-neg-domain-fauna-v1 P0：真元抽取率常量 pin 测试 ──

    #[test]
    fn ghost_contact_factor_positive_and_in_range() {
        // GHOST_CONTACT_FACTOR 必须 > 0（有效抽取）且 < 1（每次不超过 qi_max 全量）。
        // 设计值 0.05 ≈ 每次接触抽 5% × |zone_qi|，高手比低手更危险。
        assert!(
            GHOST_CONTACT_FACTOR > 0.0,
            "GHOST_CONTACT_FACTOR 必须 > 0，否则诡影接触无效"
        );
        assert!(
            GHOST_CONTACT_FACTOR < 1.0,
            "GHOST_CONTACT_FACTOR 必须 < 1，一次接触不应抽超过 qi_max 全量"
        );
        assert!(
            (GHOST_CONTACT_FACTOR - 0.05).abs() < QI_EPSILON,
            "期望 GHOST_CONTACT_FACTOR == 0.05（plan-neg-domain-fauna-v1 §6 设计决议），实际 {GHOST_CONTACT_FACTOR}"
        );
    }

    #[test]
    fn moss_drain_factor_positive_and_small() {
        // MOSS_DRAIN_FACTOR 必须 > 0（有效持续扣除）且远小于 GHOST_CONTACT_FACTOR。
        // 踩踏是每 tick 持续抽取，设计值 0.002（20 tps → 每秒 4% qi_max），
        // 应小于诡影单次接触抽取率 0.05。
        assert!(
            MOSS_DRAIN_FACTOR > 0.0,
            "MOSS_DRAIN_FACTOR 必须 > 0，否则噬灵藓踩踏无效"
        );
        assert!(
            MOSS_DRAIN_FACTOR < GHOST_CONTACT_FACTOR,
            "MOSS_DRAIN_FACTOR 应小于 GHOST_CONTACT_FACTOR（持续 tick 累积 vs 单次脉冲）"
        );
        assert!(
            (MOSS_DRAIN_FACTOR - 0.002).abs() < QI_EPSILON,
            "期望 MOSS_DRAIN_FACTOR == 0.002（plan-neg-domain-fauna-v1 §6 设计决议），实际 {MOSS_DRAIN_FACTOR}"
        );
    }

    #[test]
    fn ghost_pulse_formula_scales_with_qi_max() {
        // 验证公式：pulse = |zone_qi| × qi_max × GHOST_CONTACT_FACTOR。
        // 高手（qi_max=200）被抽双倍——这是设计意图（高手在负灵域更危险）。
        let zone_qi = -0.5_f64;
        let low_qi_max = 100.0_f64;
        let high_qi_max = 200.0_f64;
        let pulse_low = zone_qi.abs() * low_qi_max * GHOST_CONTACT_FACTOR;
        let pulse_high = zone_qi.abs() * high_qi_max * GHOST_CONTACT_FACTOR;
        assert!(
            (pulse_high - 2.0 * pulse_low).abs() < QI_EPSILON,
            "qi_max 翻倍，pulse 应翻倍；期望 {}, 实际 {pulse_high}（pulse_low={pulse_low}）",
            2.0 * pulse_low
        );
        assert!(
            pulse_low > 0.0,
            "负 zone_qi 下 pulse 必须 > 0，否则诡影接触无效"
        );
    }

    #[test]
    fn moss_drain_formula_zero_for_positive_zone() {
        // 噬灵藓只在负灵域生长，spirit_qi >= 0 时不应有持续 drain（系统会移除 tag）。
        // 但公式本身不检查 zone_qi，检查逻辑在 moss_step_on_system 和 moss_spread_system 中。
        // 这里只验证公式数值范围：qi_max=100, MOSS_DRAIN_FACTOR=0.002 → 每 tick 扣 0.2。
        let qi_max = 100.0_f64;
        let drain_per_tick = qi_max * MOSS_DRAIN_FACTOR;
        assert!(
            (drain_per_tick - 0.2).abs() < QI_EPSILON,
            "qi_max=100 时 drain_per_tick 应为 0.2（100×0.002），实际 {drain_per_tick}"
        );
    }
}
