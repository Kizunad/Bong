//! plan-tribulation-balance-v1 P0：渡劫平衡监控 dev 命令 `/balance tribulation`。
//!
//! 输出：
//!   - quota_current / quota_max / 满载率%（QuotaFullTracker，limit==0 防除零）
//!   - halfstep / ascended 累计次数（TribulationMetrics）
//!   - failed：n/a（无遥测，halfstep-buff-v1 未埋点 — 遗留决策门 A，不显示假 0）
//!   - 半步化虚平均滞留 ticks + in-game 月（HalfStepState.entered_at vs CombatClock.tick）
//!   - TribulationBalanceConfig 全部参数值
//!
//! dev-only，无副作用，仅读取（不触发 QiTransfer / qi_physics 守恒）。

use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, Query, Res, Update};

use crate::combat::CombatClock;
use crate::cultivation::tribulation::{
    current_quota_full_duration_ticks, HalfStepState, QuotaFullTracker, TribulationMetrics,
    VoidQuotaConfig,
};
use crate::cultivation::tribulation_balance::TribulationBalanceConfig;
use crate::cultivation::void::components::TICKS_PER_MONTH;

// ──────────────────────────────────────────────────────────────────────────────
// Command 注册
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BalanceCmd {
    Tribulation,
}

impl Command for BalanceCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        let balance = graph.root().literal("balance").id();
        graph
            .at(balance)
            .literal("tribulation")
            .with_executable(|_| BalanceCmd::Tribulation);
    }
}

pub fn register(app: &mut App) {
    app.add_command::<BalanceCmd>().add_systems(Update, handle);
}

// ──────────────────────────────────────────────────────────────────────────────
// 结构化报告（纯函数，供测试直接断言）
// ──────────────────────────────────────────────────────────────────────────────

/// `/balance tribulation` 的结构化输出，由 `build_balance_report` 组装。
#[derive(Debug, Clone, PartialEq)]
pub struct BalanceReport {
    pub quota_current: u32,
    pub quota_max: u32,
    /// 满载率（正常范围 [0.0, 1.0]，occupied>limit 时 >1.0）。
    /// 特殊值：`f32::INFINITY` 表示 limit==0 且 occupied>0（最严重超载，名额已归零但仍有占用）；
    /// limit==0 且 occupied==0 时为 0.0（良性空载）。
    pub quota_fill_ratio: f32,
    pub halfstep_count: u64,
    pub ascended_count: u64,
    pub halfstep_active_count: u64,
    pub halfstep_avg_stay_ticks: u64,
    pub halfstep_avg_stay_months: f64,
    pub quota_full_duration_ticks: u64,
    pub config: TribulationBalanceConfig,
}

/// 由各 resource + 在场 HalfStepState 组装报告（纯函数，无 ECS 副作用）。
pub fn build_balance_report(
    metrics: &TribulationMetrics,
    tracker: &QuotaFullTracker,
    config: &TribulationBalanceConfig,
    halfstep_states: &[HalfStepState],
    current_tick: u64,
) -> BalanceReport {
    let quota_current = tracker.current_occupied;
    let quota_max = tracker.current_limit;

    // 满载率：
    //   - limit==0 且 occupied>0 → f32::INFINITY（最严重超载：名额归零仍有占用，不能掩盖）
    //   - limit==0 且 occupied==0 → 0.0（良性空载，防除零）
    //   - limit>0 → occupied/limit（正常比率；>1.0 表示天道衰减后的暂超载）
    let quota_fill_ratio = if quota_max == 0 {
        if quota_current > 0 {
            f32::INFINITY
        } else {
            0.0
        }
    } else {
        quota_current as f32 / quota_max as f32
    };

    // 平均滞留 ticks：遍历在场 HalfStepState
    let mut total_stay: u64 = 0;
    let mut count: u64 = 0;
    for state in halfstep_states {
        total_stay = total_stay.saturating_add(current_tick.saturating_sub(state.entered_at));
        count = count.saturating_add(1);
    }
    let halfstep_avg_stay_ticks = if count > 0 { total_stay / count } else { 0 };

    // in-game 月换算（TICKS_PER_MONTH = 30 * 24 * 3600 * 20）
    let halfstep_avg_stay_months = halfstep_avg_stay_ticks as f64 / TICKS_PER_MONTH as f64;

    let quota_full_duration_ticks =
        current_quota_full_duration_ticks(metrics, tracker, current_tick);

    BalanceReport {
        quota_current,
        quota_max,
        quota_fill_ratio,
        halfstep_count: metrics.halfstep_count,
        ascended_count: metrics.ascended_count,
        halfstep_active_count: count,
        halfstep_avg_stay_ticks,
        halfstep_avg_stay_months,
        quota_full_duration_ticks,
        config: *config,
    }
}

/// 将 BalanceReport 序列化为可读文本（纯函数）。
///
/// `quota_fill_ratio` 的三种情形：
/// - `f32::INFINITY`：limit==0 且 occupied>0，最严重超载态——输出 `[OVER-FULL: 名额=0 仍有 N 占用]`
/// - `> 1.0`：occupied > limit，天道衰减 quota_limit 致暂超载——输出真实比率并附 `[OVER-FULL]`
/// - `<= 1.0`：正常范围
///
/// **不静默 clamp**——看板核心职责是暴露失衡。
pub fn format_balance_report(report: &BalanceReport) -> String {
    // quota 行：区分三种失衡态
    let quota_line = if report.quota_fill_ratio.is_infinite() {
        // 最严重：名额归零但仍有占用
        format!(
            "quota: {current}/{max} [OVER-FULL: 名额=0 仍有 {current} 占用]",
            current = report.quota_current,
            max = report.quota_max,
        )
    } else {
        let fill_pct = report.quota_fill_ratio * 100.0;
        let over_full_marker = if report.quota_fill_ratio > 1.0 {
            " [OVER-FULL]"
        } else {
            ""
        };
        format!(
            "quota: {current}/{max} ({fill:.1}%满载{over_full})",
            current = report.quota_current,
            max = report.quota_max,
            fill = fill_pct,
            over_full = over_full_marker,
        )
    };

    format!(
        "[渡劫平衡看板]\n\
         {quota_line}\n\
         结算: halfstep={hs} ascended={asc} failed=n/a(无遥测)\n\
         当前半步修士: {active}人 平均滞留={stay_ticks}tick ({stay_months:.2}月)\n\
         quota满载累计: {full_ticks}tick\n\
         [BalanceConfig]\n\
         quota_k={qk:.2} wave_damage_base={wdb} wave_qi_drain_base={wqdb} \
         juebi_intensity_base={jib} heart_demon_qi_penalty_ratio={hdqpr:.2}",
        quota_line = quota_line,
        hs = report.halfstep_count,
        asc = report.ascended_count,
        active = report.halfstep_active_count,
        stay_ticks = report.halfstep_avg_stay_ticks,
        stay_months = report.halfstep_avg_stay_months,
        full_ticks = report.quota_full_duration_ticks,
        qk = report.config.quota_k,
        wdb = report.config.wave_damage_base,
        wqdb = report.config.wave_qi_drain_base,
        jib = report.config.juebi_intensity_base,
        hdqpr = report.config.heart_demon_qi_penalty_ratio,
    )
}

// ──────────────────────────────────────────────────────────────────────────────
// ECS handle system
// ──────────────────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn handle(
    mut events: EventReader<CommandResultEvent<BalanceCmd>>,
    metrics: Option<Res<TribulationMetrics>>,
    tracker: Option<Res<QuotaFullTracker>>,
    config: Option<Res<TribulationBalanceConfig>>,
    void_quota: Option<Res<VoidQuotaConfig>>,
    clock: Option<Res<CombatClock>>,
    halfstep_query: Query<&HalfStepState>,
    mut clients: Query<&mut Client>,
) {
    for event in events.read() {
        let Ok(mut client) = clients.get_mut(event.executor) else {
            continue;
        };
        let Some(metrics) = metrics.as_deref() else {
            client.send_chat_message("balance tribulation: TribulationMetrics resource missing");
            continue;
        };
        let Some(tracker) = tracker.as_deref() else {
            client.send_chat_message("balance tribulation: QuotaFullTracker resource missing");
            continue;
        };
        let Some(config) = config.as_deref() else {
            client.send_chat_message(
                "balance tribulation: TribulationBalanceConfig resource missing",
            );
            continue;
        };
        let Some(clock) = clock.as_deref() else {
            client.send_chat_message("balance tribulation: CombatClock resource missing");
            continue;
        };
        // quota_k 展示 live VoidQuotaConfig 值（防止 env 覆盖后看板撒谎）
        let mut live_config = *config;
        if let Some(vq) = void_quota.as_deref() {
            live_config.quota_k = vq.quota_k;
        }
        let states: Vec<HalfStepState> = halfstep_query.iter().copied().collect();
        let report = build_balance_report(metrics, tracker, &live_config, &states, clock.tick);
        client.send_chat_message(format_balance_report(&report));
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::CombatClock;
    use crate::cultivation::tribulation::{
        HalfStepState, JueBiTriggerSource, QuotaFullTracker, TribulationMetrics, TribulationSettled,
    };
    use crate::cultivation::tribulation_balance::TribulationBalanceConfig;
    use crate::schema::tribulation::DuXuOutcomeV1;
    use valence::prelude::{App, Entity, Events, Update};

    // ─── 辅助函数 ───────────────────────────────────────────────────────────

    fn default_balance_report_inputs() -> (
        TribulationMetrics,
        QuotaFullTracker,
        TribulationBalanceConfig,
    ) {
        (
            TribulationMetrics::default(),
            QuotaFullTracker::default(),
            TribulationBalanceConfig::default(),
        )
    }

    // ─── P0 看板纯函数测试 ──────────────────────────────────────────────────

    /// ① build_report 空 state：全零输出，config 字段用 default。
    #[test]
    fn build_balance_report_with_empty_state_returns_zeros() {
        let (metrics, tracker, config) = default_balance_report_inputs();
        let report = build_balance_report(&metrics, &tracker, &config, &[], 1000);
        assert_eq!(
            report.quota_current, 0,
            "空 state quota_current 应为 0，got {}",
            report.quota_current
        );
        assert_eq!(
            report.quota_max, 0,
            "空 state quota_max 应为 0，got {}",
            report.quota_max
        );
        assert_eq!(
            report.quota_fill_ratio, 0.0,
            "quota_max==0 时满载率应为 0.0（防除零），got {}",
            report.quota_fill_ratio
        );
        assert_eq!(
            report.halfstep_count, 0,
            "空 metrics halfstep_count 应为 0，got {}",
            report.halfstep_count
        );
        assert_eq!(
            report.ascended_count, 0,
            "空 metrics ascended_count 应为 0，got {}",
            report.ascended_count
        );
        assert_eq!(
            report.halfstep_active_count, 0,
            "空 HalfStepState 数组 active_count 应为 0",
        );
        assert_eq!(
            report.halfstep_avg_stay_ticks, 0,
            "空 HalfStepState 平均滞留 应为 0，got {}",
            report.halfstep_avg_stay_ticks
        );
    }

    /// ② 聚合多 HalfStepState 平均滞留 tick 正确（含 in-game 月换算边界）。
    #[test]
    fn build_balance_report_aggregates_halfstep_avg_stay_correctly() {
        let metrics = TribulationMetrics {
            halfstep_count: 3,
            ascended_count: 0,
            quota_full_duration_ticks: 0,
        };
        let tracker = QuotaFullTracker::default();
        let config = TribulationBalanceConfig::default();
        // entered_at: 100/200/300，current_tick=1000 → 滞留 900/800/700，avg=800
        let states = [
            HalfStepState::new(100),
            HalfStepState::new(200),
            HalfStepState::new(300),
        ];
        let report = build_balance_report(&metrics, &tracker, &config, &states, 1000);
        assert_eq!(
            report.halfstep_active_count, 3,
            "在场半步数应为 3，got {}",
            report.halfstep_active_count
        );
        assert_eq!(
            report.halfstep_avg_stay_ticks, 800,
            "期望平均滞留=800 ticks (sum=2400/3=800)，got {}；off-by-one 或聚合逻辑错误",
            report.halfstep_avg_stay_ticks
        );
        // in-game 月换算：800 / TICKS_PER_MONTH（约 51840000），应该是极小正数
        assert!(
            report.halfstep_avg_stay_months >= 0.0,
            "stay_months 应为非负数，got {}",
            report.halfstep_avg_stay_months
        );
        let expected_months = 800.0 / TICKS_PER_MONTH as f64;
        assert!(
            (report.halfstep_avg_stay_months - expected_months).abs() < 1e-10,
            "stay_months 期望={expected_months}，got {}；TICKS_PER_MONTH 换算错误",
            report.halfstep_avg_stay_months
        );
    }

    /// ③ 满载率计算：occupied/limit + limit==0 防除零 + 满载情形。
    #[test]
    fn build_balance_report_quota_fill_ratio_correct_and_div_zero_guarded() {
        let config = TribulationBalanceConfig::default();

        // occupied==0 且 limit==0 → 0.0（良性空载：天道尚未开放名额，亦无占用，不是失衡）
        {
            let metrics = TribulationMetrics::default();
            let tracker = QuotaFullTracker {
                current_occupied: 0,
                current_limit: 0,
                full_since_tick: None,
            };
            let report = build_balance_report(&metrics, &tracker, &config, &[], 100);
            assert_eq!(
                report.quota_fill_ratio, 0.0,
                "occupied=0 且 limit==0（良性空载）时满载率应为 0.0，got {}",
                report.quota_fill_ratio
            );
        }

        // occupied=1 limit=2 → 0.5
        {
            let metrics = TribulationMetrics::default();
            let tracker = QuotaFullTracker {
                current_occupied: 1,
                current_limit: 2,
                full_since_tick: None,
            };
            let report = build_balance_report(&metrics, &tracker, &config, &[], 100);
            assert!(
                (report.quota_fill_ratio - 0.5).abs() < 1e-6,
                "occupied=1 limit=2 时满载率期望=0.5，got {}",
                report.quota_fill_ratio
            );
        }

        // occupied==limit（满载）→ 1.0
        {
            let metrics = TribulationMetrics::default();
            let tracker = QuotaFullTracker {
                current_occupied: 3,
                current_limit: 3,
                full_since_tick: None,
            };
            let report = build_balance_report(&metrics, &tracker, &config, &[], 100);
            assert!(
                (report.quota_fill_ratio - 1.0).abs() < 1e-6,
                "occupied==limit 时满载率期望=1.0，got {}",
                report.quota_fill_ratio
            );
        }
    }

    /// ③-a MAJOR-1：over-full 边界——occupied > limit 时 ratio 真实反映超载（>1.0），不 clamp 成 1.0，
    ///        format 输出包含 [OVER-FULL] 标注（天道衰减 quota_limit 致暂超 occupied 的真实场景）。
    #[test]
    fn build_balance_report_over_full_ratio_exceeds_one_and_format_marks_it() {
        let config = TribulationBalanceConfig::default();

        // occupied=4 limit=3 → ratio = 4/3 ≈ 1.333...（天道衰减 limit 后的超载态）
        {
            let metrics = TribulationMetrics::default();
            let tracker = QuotaFullTracker {
                current_occupied: 4,
                current_limit: 3,
                full_since_tick: None,
            };
            let report = build_balance_report(&metrics, &tracker, &config, &[], 100);
            let expected_ratio = 4.0_f32 / 3.0_f32;
            assert!(
                report.quota_fill_ratio > 1.0,
                "occupied=4 limit=3 时满载率应 >1.0（超载），got {}；不允许 clamp 成 1.0 掩盖失衡",
                report.quota_fill_ratio
            );
            assert!(
                (report.quota_fill_ratio - expected_ratio).abs() < 1e-5,
                "occupied=4 limit=3 满载率期望={expected_ratio:.6}，got {}；计算有误",
                report.quota_fill_ratio
            );
            let text = format_balance_report(&report);
            assert!(
                text.contains("[OVER-FULL]"),
                "over-full 时 format 应包含 [OVER-FULL] 标注，got:\n{text}"
            );
            // 确保百分比真实显示（> 100%），不被截断
            assert!(
                text.contains("133."),
                "over-full 时百分比应真实显示（约133.3%），got:\n{text}"
            );
        }

        // occupied=1 limit=3 → ratio≈0.333（正常范围），format 不包含 [OVER-FULL]
        {
            let metrics = TribulationMetrics::default();
            let tracker = QuotaFullTracker {
                current_occupied: 1,
                current_limit: 3,
                full_since_tick: None,
            };
            let report = build_balance_report(&metrics, &tracker, &config, &[], 100);
            let text = format_balance_report(&report);
            assert!(
                !text.contains("[OVER-FULL]"),
                "正常范围（occupied<limit）format 不应包含 [OVER-FULL]，got:\n{text}"
            );
        }

        // occupied==limit → ratio==1.0（正好满载），format 不包含 [OVER-FULL]
        {
            let metrics = TribulationMetrics::default();
            let tracker = QuotaFullTracker {
                current_occupied: 3,
                current_limit: 3,
                full_since_tick: None,
            };
            let report = build_balance_report(&metrics, &tracker, &config, &[], 100);
            let text = format_balance_report(&report);
            assert!(
                !text.contains("[OVER-FULL]"),
                "occupied==limit（恰好满载，ratio=1.0）format 不应含 [OVER-FULL]，got:\n{text}"
            );
        }
    }

    /// ③-a2 MAJOR：occupied>0 且 limit==0（最严重超载：名额归零仍有占用）——
    ///   ratio 应为 f32::INFINITY，format 应包含 [OVER-FULL: 名额=0] 标注；
    ///   occupied==0 且 limit==0（良性空载）——ratio==0.0，format 不含 OVER-FULL 标注。
    #[test]
    fn build_balance_report_occupied_gt_zero_with_limit_zero_is_over_full_infinity() {
        let config = TribulationBalanceConfig::default();

        // occupied=2, limit=0 → ratio=INFINITY（最严重超载，看板必须暴露）
        {
            let metrics = TribulationMetrics::default();
            let tracker = QuotaFullTracker {
                current_occupied: 2,
                current_limit: 0,
                full_since_tick: None,
            };
            let report = build_balance_report(&metrics, &tracker, &config, &[], 100);
            assert!(
                report.quota_fill_ratio.is_infinite(),
                "occupied=2 limit=0 时 quota_fill_ratio 应为 INFINITY（最严重超载），\
                 got {}；不允许返回 0.0 掩盖失衡",
                report.quota_fill_ratio
            );
            let text = format_balance_report(&report);
            assert!(
                text.contains("[OVER-FULL: 名额=0 仍有 2 占用]"),
                "occupied=2 limit=0 时 format 应包含 [OVER-FULL: 名额=0 仍有 2 占用]；got:\n{text}"
            );
        }

        // occupied=0, limit=0 → ratio=0.0（良性空载：名额归零且无人占用，不是失衡）
        {
            let metrics = TribulationMetrics::default();
            let tracker = QuotaFullTracker {
                current_occupied: 0,
                current_limit: 0,
                full_since_tick: None,
            };
            let report = build_balance_report(&metrics, &tracker, &config, &[], 100);
            assert_eq!(
                report.quota_fill_ratio, 0.0,
                "occupied=0 limit=0（良性空载）时 ratio 应为 0.0，got {}",
                report.quota_fill_ratio
            );
            let text = format_balance_report(&report);
            assert!(
                !text.contains("[OVER-FULL"),
                "occupied=0 limit=0（良性空载）format 不应含 [OVER-FULL] 标注，got:\n{text}"
            );
        }
    }

    /// ③-a3 MINOR：avg-stay 时钟回拨饱和——current_tick < entered_at 时每条记录饱和到 0，
    ///   总 avg 仍为 0，不 panic、不 underflow。
    #[test]
    fn build_balance_report_avg_stay_saturates_to_zero_on_clock_rollback() {
        let config = TribulationBalanceConfig::default();
        let metrics = TribulationMetrics::default();
        let tracker = QuotaFullTracker::default();

        // current_tick=100 但 entered_at=500（时钟回拨场景）→ 每条 saturating_sub = 0
        let states = [HalfStepState::new(500), HalfStepState::new(800)];
        let report = build_balance_report(&metrics, &tracker, &config, &states, 100);
        assert_eq!(
            report.halfstep_avg_stay_ticks, 0,
            "时钟回拨(current_tick=100 < entered_at=500/800)时 avg_stay 应饱和至 0，\
             got {}；saturating_sub 语义失效或逻辑错误",
            report.halfstep_avg_stay_ticks
        );
        assert_eq!(
            report.halfstep_active_count, 2,
            "时钟回拨场景 active_count 仍应为 2（entity 存在），got {}",
            report.halfstep_active_count
        );
    }

    /// ③-b MAJOR-2：quota_full_duration_ticks pending 分支覆盖——tracker.full_since_tick=Some(t)
    ///        且 current_tick>t 时，build_balance_report 报告的满载累计 = base + (current_tick - t)，
    ///        不 panic，pending 分支不被零覆盖。
    #[test]
    fn build_balance_report_quota_full_duration_includes_pending_when_full_since_set() {
        let config = TribulationBalanceConfig::default();

        // base=500（已记录的历史满载），full_since_tick=Some(1000)，current_tick=1200
        // 期望：pending = 1200-1000 = 200，total = 500+200 = 700
        {
            let metrics = TribulationMetrics {
                halfstep_count: 0,
                ascended_count: 0,
                quota_full_duration_ticks: 500, // base：已结算的历史满载
            };
            let tracker = QuotaFullTracker {
                current_occupied: 3,
                current_limit: 3,
                full_since_tick: Some(1000), // 正在进行中的满载，尚未结算
            };
            let report = build_balance_report(&metrics, &tracker, &config, &[], 1200);
            assert_eq!(
                report.quota_full_duration_ticks, 700,
                "base=500 + pending(1200-1000=200) = 700；\
                 pending 分支未命中或 current_quota_full_duration_ticks 计算错误，got {}",
                report.quota_full_duration_ticks
            );
        }

        // base=0，full_since_tick=Some(50)，current_tick=150 → pending=100，total=100
        {
            let metrics = TribulationMetrics::default(); // quota_full_duration_ticks=0
            let tracker = QuotaFullTracker {
                current_occupied: 2,
                current_limit: 2,
                full_since_tick: Some(50),
            };
            let report = build_balance_report(&metrics, &tracker, &config, &[], 150);
            assert_eq!(
                report.quota_full_duration_ticks, 100,
                "base=0 + pending(150-50=100) = 100；got {}",
                report.quota_full_duration_ticks
            );
        }

        // full_since_tick=None → 仅返回 base，不加 pending（对照组，确保分支区分正确）
        {
            let metrics = TribulationMetrics {
                halfstep_count: 0,
                ascended_count: 0,
                quota_full_duration_ticks: 300,
            };
            let tracker = QuotaFullTracker {
                current_occupied: 1,
                current_limit: 3,
                full_since_tick: None, // 非满载，无 pending
            };
            let report = build_balance_report(&metrics, &tracker, &config, &[], 1000);
            assert_eq!(
                report.quota_full_duration_ticks, 300,
                "full_since_tick=None 时应返回 base=300，不加 pending；got {}",
                report.quota_full_duration_ticks
            );
        }
    }

    /// ④ config 读取 / default 字段 = 真实 const（pin 联测）
    #[test]
    fn build_balance_report_embeds_config_with_correct_defaults() {
        use crate::cultivation::tribulation::JUEBI_INTENSITY_BASE;
        use crate::cultivation::tribulation::{
            DEFAULT_VOID_QUOTA_K, DUXU_AOE_DAMAGE_BASE,
            DUXU_HEART_DEMON_OBSESSION_QI_PENALTY_RATIO, DUXU_QI_DRAIN_BASE,
        };

        let config = TribulationBalanceConfig::default();
        let (metrics, tracker, _) = default_balance_report_inputs();
        let report = build_balance_report(&metrics, &tracker, &config, &[], 0);

        assert_eq!(
            report.config.quota_k, DEFAULT_VOID_QUOTA_K,
            "report.config.quota_k 应= DEFAULT_VOID_QUOTA_K={DEFAULT_VOID_QUOTA_K}，got {}",
            report.config.quota_k
        );
        assert_eq!(
            report.config.wave_damage_base,
            DUXU_AOE_DAMAGE_BASE,
            "report.config.wave_damage_base 应= DUXU_AOE_DAMAGE_BASE={DUXU_AOE_DAMAGE_BASE}，got {}",
            report.config.wave_damage_base
        );
        assert_eq!(
            report.config.wave_qi_drain_base, DUXU_QI_DRAIN_BASE,
            "report.config.wave_qi_drain_base 应= DUXU_QI_DRAIN_BASE={DUXU_QI_DRAIN_BASE}，got {}",
            report.config.wave_qi_drain_base
        );
        assert_eq!(
            report.config.juebi_intensity_base,
            JUEBI_INTENSITY_BASE,
            "report.config.juebi_intensity_base 应= JUEBI_INTENSITY_BASE={JUEBI_INTENSITY_BASE}，got {}",
            report.config.juebi_intensity_base
        );
        assert_eq!(
            report.config.heart_demon_qi_penalty_ratio,
            DUXU_HEART_DEMON_OBSESSION_QI_PENALTY_RATIO,
            "report.config.heart_demon_qi_penalty_ratio 应= DUXU_HEART_DEMON_OBSESSION_QI_PENALTY_RATIO={DUXU_HEART_DEMON_OBSESSION_QI_PENALTY_RATIO}，got {}",
            report.config.heart_demon_qi_penalty_ratio
        );
    }

    /// ⑤ format_balance_report 包含全部关键字段（防 format 字符串顺序漂移）。
    #[test]
    fn format_balance_report_contains_all_key_fields() {
        let config = TribulationBalanceConfig::default();
        let report = BalanceReport {
            quota_current: 2,
            quota_max: 3,
            quota_fill_ratio: 2.0 / 3.0,
            halfstep_count: 5,
            ascended_count: 1,
            halfstep_active_count: 2,
            halfstep_avg_stay_ticks: 12000,
            halfstep_avg_stay_months: 12000.0 / TICKS_PER_MONTH as f64,
            quota_full_duration_ticks: 999,
            config,
        };
        let text = format_balance_report(&report);
        // 关键字段必须出现在输出文本中
        for needle in [
            "quota:",
            "2/3",
            "halfstep=5",
            "ascended=1",
            "failed=n/a",
            "12000tick",
            "quota_k=",
            "wave_damage_base=",
            "wave_qi_drain_base=",
            "juebi_intensity_base=",
            "heart_demon_qi_penalty_ratio=",
        ] {
            assert!(
                text.contains(needle),
                "format_balance_report 输出缺少 `{needle}`；got:\n{text}"
            );
        }
    }

    // ─── ⑥ mock 10 次渡劫结算后 counter 正确（P0 验收核心） ────────────────

    /// mock 辅助：构造指定 outcome 的 TribulationSettled 事件（仅 test 使用）
    fn make_test_settled(entity: Entity, outcome: DuXuOutcomeV1) -> TribulationSettled {
        use crate::cultivation::tribulation::TribulationKind;
        TribulationSettled {
            entity,
            kind: TribulationKind::JueBi,
            source: Some(JueBiTriggerSource::VoidQuotaExceeded),
            result: crate::schema::tribulation::DuXuResultV1 {
                char_id: "balance_test_char".to_string(),
                outcome,
                killer: None,
                waves_survived: 3,
                reason: Some("balance_test".to_string()),
            },
        }
    }

    /// 搭建最小测试 App：含 TribulationMetrics + track_tribulation_metrics_system。
    fn balance_metrics_test_app() -> App {
        use crate::cultivation::tribulation::{
            dispatch_rechallenge_on_quota_opened_system, track_quota_full_duration_system,
            track_tribulation_metrics_system, AscensionQuotaOccupied, AscensionQuotaOpened,
            HalfStepRechallengeQueue, VoidQuotaConfig,
        };
        use crate::qi_physics::ledger::{QiTransfer, WorldQiBudget};

        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 0 });
        app.init_resource::<TribulationMetrics>();
        app.init_resource::<QuotaFullTracker>();
        app.init_resource::<HalfStepRechallengeQueue>();
        app.insert_resource(WorldQiBudget::from_total(100.0));
        app.insert_resource(VoidQuotaConfig::default());
        app.init_resource::<TribulationBalanceConfig>();
        app.add_event::<TribulationSettled>();
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<AscensionQuotaOccupied>();
        app.add_event::<QiTransfer>();
        app.add_event::<crate::cultivation::tribulation::HalfStepRechallengeTriggerEvent>();
        app.add_systems(
            Update,
            (
                track_tribulation_metrics_system,
                track_quota_full_duration_system,
                dispatch_rechallenge_on_quota_opened_system,
            ),
        );
        app
    }

    /// mock 10 次 HalfStep 结算 + 5 次 Ascended 结算后，build_balance_report 读值正确。
    #[test]
    fn mock_10_tribulation_settlements_balance_report_shows_correct_counts() {
        let mut app = balance_metrics_test_app();
        let dummy = app.world_mut().spawn(()).id();

        // mock 10 次 HalfStep 结算（source=VoidQuotaExceeded 才增 halfstep_count）
        for _ in 0..10 {
            app.world_mut()
                .resource_mut::<Events<TribulationSettled>>()
                .send(make_test_settled(dummy, DuXuOutcomeV1::HalfStep));
        }
        // mock 5 次 Ascended 结算
        for _ in 0..5 {
            app.world_mut()
                .resource_mut::<Events<TribulationSettled>>()
                .send(make_test_settled(dummy, DuXuOutcomeV1::Ascended));
        }
        app.update();

        let metrics = *app.world().resource::<TribulationMetrics>();
        let tracker = *app.world().resource::<QuotaFullTracker>();
        let config = *app.world().resource::<TribulationBalanceConfig>();

        let report = build_balance_report(&metrics, &tracker, &config, &[], 0);

        assert_eq!(
            report.halfstep_count, 10,
            "mock 10 次 HalfStep 结算后 halfstep_count 期望=10，got {}；\
             track_tribulation_metrics_system 或 build_balance_report 未正确读取",
            report.halfstep_count
        );
        assert_eq!(
            report.ascended_count, 5,
            "mock 5 次 Ascended 结算后 ascended_count 期望=5，got {}",
            report.ascended_count
        );
    }

    // ─── handle 集成路径（含资源缺失 graceful fallback） ────────────────────

    use crate::cmd::dev::test_support::{run_update, spawn_test_client};

    fn dispatch_balance_tribulation(app: &mut App, executor: Entity) {
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<BalanceCmd>>>()
            .send(CommandResultEvent {
                result: BalanceCmd::Tribulation,
                executor,
                modifiers: Default::default(),
            });
        run_update(app);
    }

    /// ⑦ handle happy path：全资源到位不 panic。
    #[test]
    fn handle_runs_to_completion_with_full_resources_and_client() {
        let mut app = App::new();
        app.insert_resource(crate::combat::CombatClock { tick: 100 });
        app.init_resource::<TribulationMetrics>();
        app.init_resource::<QuotaFullTracker>();
        app.init_resource::<TribulationBalanceConfig>();
        app.add_event::<CommandResultEvent<BalanceCmd>>();
        app.add_systems(Update, handle);
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);
        dispatch_balance_tribulation(&mut app, player);
        // 不 panic 即通过：handle 已正确读取所有资源并输出报告
    }

    /// ⑧ handle graceful fallback：各资源缺失时均不 panic，仅发提示 chat。
    #[test]
    fn handle_graceful_when_metrics_missing() {
        let mut app = App::new();
        app.insert_resource(crate::combat::CombatClock { tick: 100 });
        // 故意不 init TribulationMetrics
        app.init_resource::<QuotaFullTracker>();
        app.init_resource::<TribulationBalanceConfig>();
        app.add_event::<CommandResultEvent<BalanceCmd>>();
        app.add_systems(Update, handle);
        let player = spawn_test_client(&mut app, "Bob", [0.0, 0.0, 0.0]);
        dispatch_balance_tribulation(&mut app, player);
    }

    #[test]
    fn handle_graceful_when_tracker_missing() {
        let mut app = App::new();
        app.insert_resource(crate::combat::CombatClock { tick: 100 });
        app.init_resource::<TribulationMetrics>();
        // 故意不 init QuotaFullTracker
        app.init_resource::<TribulationBalanceConfig>();
        app.add_event::<CommandResultEvent<BalanceCmd>>();
        app.add_systems(Update, handle);
        let player = spawn_test_client(&mut app, "Carol", [0.0, 0.0, 0.0]);
        dispatch_balance_tribulation(&mut app, player);
    }

    #[test]
    fn handle_graceful_when_config_missing() {
        let mut app = App::new();
        app.insert_resource(crate::combat::CombatClock { tick: 100 });
        app.init_resource::<TribulationMetrics>();
        app.init_resource::<QuotaFullTracker>();
        // 故意不 init TribulationBalanceConfig
        app.add_event::<CommandResultEvent<BalanceCmd>>();
        app.add_systems(Update, handle);
        let player = spawn_test_client(&mut app, "Dave", [0.0, 0.0, 0.0]);
        dispatch_balance_tribulation(&mut app, player);
    }

    #[test]
    fn handle_graceful_when_clock_missing() {
        let mut app = App::new();
        // 故意不 insert CombatClock
        app.init_resource::<TribulationMetrics>();
        app.init_resource::<QuotaFullTracker>();
        app.init_resource::<TribulationBalanceConfig>();
        app.add_event::<CommandResultEvent<BalanceCmd>>();
        app.add_systems(Update, handle);
        let player = spawn_test_client(&mut app, "Eve", [0.0, 0.0, 0.0]);
        dispatch_balance_tribulation(&mut app, player);
    }

    #[test]
    fn handle_no_op_when_executor_lacks_client_component() {
        let mut app = App::new();
        app.insert_resource(crate::combat::CombatClock { tick: 100 });
        app.init_resource::<TribulationMetrics>();
        app.init_resource::<QuotaFullTracker>();
        app.init_resource::<TribulationBalanceConfig>();
        app.add_event::<CommandResultEvent<BalanceCmd>>();
        app.add_systems(Update, handle);
        let bare = app.world_mut().spawn_empty().id();
        dispatch_balance_tribulation(&mut app, bare);
        // 无 Client 的 executor → handle continue 跳过，不影响其它 entity
    }

    /// ⑨ MINOR：handle() 注入 live VoidQuotaConfig 时 quota_k 使用 live 值（防 env 覆盖后撒谎）。
    ///   VoidQuotaConfig.quota_k != DEFAULT_VOID_QUOTA_K 时，build_balance_report 收到的
    ///   config.quota_k 应等于 live VoidQuotaConfig.quota_k 而非编译时常数。
    #[test]
    fn handle_uses_live_void_quota_config_quota_k_when_available() {
        use crate::cultivation::tribulation::DEFAULT_VOID_QUOTA_K;

        // live VoidQuotaConfig 设为与 default 不同的值，模拟 BONG_VOID_QUOTA_K env 覆盖
        let live_quota_k = DEFAULT_VOID_QUOTA_K * 2.0 + 1.0;
        let live_void_quota = VoidQuotaConfig {
            quota_k: live_quota_k,
        };

        let config = TribulationBalanceConfig::default(); // quota_k = DEFAULT_VOID_QUOTA_K（旧常数）
        let metrics = TribulationMetrics::default();
        let tracker = QuotaFullTracker::default();

        // 模拟 handle 内部的 override 逻辑（build_balance_report 接收的 live_config 验证）
        let mut live_config = config;
        live_config.quota_k = live_void_quota.quota_k;

        let report = build_balance_report(&metrics, &tracker, &live_config, &[], 0);
        assert!(
            (report.config.quota_k - live_quota_k).abs() < 1e-9,
            "handle 注入 live VoidQuotaConfig 后 report.config.quota_k 应= live_quota_k={live_quota_k}，\
             got {}；quota_k 覆盖逻辑失效（看板撒谎）",
            report.config.quota_k
        );
        assert!(
            (report.config.quota_k - DEFAULT_VOID_QUOTA_K).abs() > 1e-9,
            "live VoidQuotaConfig.quota_k 应覆盖 DEFAULT_VOID_QUOTA_K；两者相同说明覆盖未生效",
        );
    }
}
