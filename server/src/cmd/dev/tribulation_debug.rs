//! plan-halfstep-buff-v1 P0：渡虚劫遥测 dev 命令 `/tribulation_debug`。
//!
//! 输出当前 `TribulationMetrics` 累计计数（halfstep / ascended / quota_full_ticks）
//! 加 `QuotaFullTracker` 当前 occupied/limit + `HalfStepState` 在场半步修士数 / 平均滞留 ticks。
//!
//! dev-only，无副作用，仅读取。

use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, EventReader, Query, Res, Update};

use crate::combat::CombatClock;
use crate::cultivation::tribulation::{
    current_quota_full_duration_ticks, HalfStepState, QuotaFullTracker, TribulationMetrics,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TribulationDebugCmd {
    Dump,
}

impl Command for TribulationDebugCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("tribulation_debug")
            .with_executable(|_| TribulationDebugCmd::Dump);
    }
}

pub fn register(app: &mut App) {
    app.add_command::<TribulationDebugCmd>()
        .add_systems(Update, handle);
}

/// 结构化报告：用于 dev 命令输出 + 测试断言。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TribulationDebugReport {
    pub halfstep_count: u64,
    pub ascended_count: u64,
    pub quota_full_duration_ticks: u64,
    pub current_quota_occupied: u32,
    pub current_quota_limit: u32,
    pub halfstep_active_count: u64,
    pub halfstep_avg_stay_ticks: u64,
}

/// 由 metrics + tracker + 在场 HalfStepState entities 合成 report。
///
/// 提取为独立 fn 是因为：(a) 测试可直接断言，无需 mock client；(b) 未来若要扩到
/// Redis emit / agent payload 复用同一组装逻辑。
pub fn build_report(
    metrics: &TribulationMetrics,
    tracker: &QuotaFullTracker,
    halfstep_states: &[HalfStepState],
    current_tick: u64,
) -> TribulationDebugReport {
    let mut total_stay: u64 = 0;
    let mut count: u64 = 0;
    for state in halfstep_states {
        total_stay = total_stay.saturating_add(current_tick.saturating_sub(state.entered_at));
        count = count.saturating_add(1);
    }
    let avg = if count > 0 { total_stay / count } else { 0 };
    TribulationDebugReport {
        halfstep_count: metrics.halfstep_count,
        ascended_count: metrics.ascended_count,
        quota_full_duration_ticks: current_quota_full_duration_ticks(
            metrics,
            tracker,
            current_tick,
        ),
        current_quota_occupied: tracker.current_occupied,
        current_quota_limit: tracker.current_limit,
        halfstep_active_count: count,
        halfstep_avg_stay_ticks: avg,
    }
}

pub fn format_report(report: &TribulationDebugReport) -> String {
    format!(
        "tribulation_debug: halfstep={} ascended={} quota_full_ticks={} \
         quota={}/{} active_halfstep={} avg_stay_ticks={}",
        report.halfstep_count,
        report.ascended_count,
        report.quota_full_duration_ticks,
        report.current_quota_occupied,
        report.current_quota_limit,
        report.halfstep_active_count,
        report.halfstep_avg_stay_ticks
    )
}

pub fn handle(
    mut events: EventReader<CommandResultEvent<TribulationDebugCmd>>,
    metrics: Option<Res<TribulationMetrics>>,
    tracker: Option<Res<QuotaFullTracker>>,
    clock: Option<Res<CombatClock>>,
    halfstep_query: Query<&HalfStepState>,
    mut clients: Query<&mut Client>,
) {
    for event in events.read() {
        let Ok(mut client) = clients.get_mut(event.executor) else {
            continue;
        };
        let Some(metrics) = metrics.as_deref() else {
            client.send_chat_message("tribulation_debug: TribulationMetrics resource missing");
            continue;
        };
        let Some(tracker) = tracker.as_deref() else {
            client.send_chat_message("tribulation_debug: QuotaFullTracker resource missing");
            continue;
        };
        let Some(clock) = clock.as_deref() else {
            client.send_chat_message("tribulation_debug: CombatClock resource missing");
            continue;
        };
        let states: Vec<HalfStepState> = halfstep_query.iter().copied().collect();
        let report = build_report(metrics, tracker, &states, clock.tick);
        client.send_chat_message(format_report(&report));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_report_with_empty_state_reports_zeros() {
        let metrics = TribulationMetrics::default();
        let tracker = QuotaFullTracker::default();
        let report = build_report(&metrics, &tracker, &[], 1000);
        assert_eq!(
            report,
            TribulationDebugReport {
                halfstep_count: 0,
                ascended_count: 0,
                quota_full_duration_ticks: 0,
                current_quota_occupied: 0,
                current_quota_limit: 0,
                halfstep_active_count: 0,
                halfstep_avg_stay_ticks: 0,
            },
            "empty state should report all zeros; observed deltas indicate hidden ambient state"
        );
    }

    #[test]
    fn build_report_aggregates_halfstep_states_average() {
        let metrics = TribulationMetrics {
            halfstep_count: 3,
            ascended_count: 1,
            quota_full_duration_ticks: 0,
        };
        let tracker = QuotaFullTracker::default();
        // 三个 HalfStep：分别在 tick 100 / 200 / 300 进入，current_tick = 1000
        let states = [
            HalfStepState::new(100), // 滞留 900
            HalfStepState::new(200), // 滞留 800
            HalfStepState::new(300), // 滞留 700
        ];
        let report = build_report(&metrics, &tracker, &states, 1000);
        assert_eq!(report.halfstep_count, 3);
        assert_eq!(report.ascended_count, 1);
        assert_eq!(
            report.halfstep_active_count, 3,
            "in-scene halfstep count != states.len(), aggregation broken"
        );
        // 平均 (900 + 800 + 700) / 3 = 800
        assert_eq!(
            report.halfstep_avg_stay_ticks, 800,
            "expected avg 800 (= sum 2400 / 3); off-by-one or precision bug"
        );
    }

    #[test]
    fn build_report_includes_pending_quota_full_duration() {
        let metrics = TribulationMetrics {
            halfstep_count: 0,
            ascended_count: 0,
            quota_full_duration_ticks: 500, // 历史累计
        };
        let tracker = QuotaFullTracker {
            current_occupied: 3,
            current_limit: 3,
            full_since_tick: Some(2000),
        };
        let report = build_report(&metrics, &tracker, &[], 3000);
        // 500 历史累计 + (3000 - 2000) pending = 1500
        assert_eq!(
            report.quota_full_duration_ticks, 1500,
            "pending duration (current_tick - full_since_tick) must be added on top of historical accum"
        );
    }

    #[test]
    fn format_report_keeps_key_value_grammar() {
        let report = TribulationDebugReport {
            halfstep_count: 7,
            ascended_count: 2,
            quota_full_duration_ticks: 1234,
            current_quota_occupied: 1,
            current_quota_limit: 3,
            halfstep_active_count: 4,
            halfstep_avg_stay_ticks: 600,
        };
        let text = format_report(&report);
        // 断言关键字段都被输出（防止 format 字符串顺序漂移导致测试无感知）
        for needle in [
            "halfstep=7",
            "ascended=2",
            "quota_full_ticks=1234",
            "quota=1/3",
            "active_halfstep=4",
            "avg_stay_ticks=600",
        ] {
            assert!(
                text.contains(needle),
                "format output missing `{needle}`; got: {text}"
            );
        }
    }
}
