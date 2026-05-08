//! NPC 性能探针。
//!
//! 只记录内部热点系统耗时；不改变 gameplay 语义，也不对外新增 schema。

use std::collections::HashMap;
use std::time::Instant;

use valence::prelude::{bevy_ecs, App, Resource};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NpcPerfSample {
    pub total_us: u64,
    pub count: u64,
    pub max_us: u64,
}

#[derive(Clone, Debug, Default, Resource)]
pub struct NpcPerfProbe {
    samples: HashMap<&'static str, NpcPerfSample>,
    last_log_tick: u32,
}

impl NpcPerfProbe {
    pub const LOG_INTERVAL_TICKS: u32 = 200;

    pub fn record(&mut self, system_name: &'static str, dur_us: u64) {
        let sample = self.samples.entry(system_name).or_default();
        sample.total_us = sample.total_us.saturating_add(dur_us);
        sample.count = sample.count.saturating_add(1);
        sample.max_us = sample.max_us.max(dur_us);
    }

    pub fn record_elapsed(&mut self, system_name: &'static str, started_at: Instant) {
        self.record(
            system_name,
            started_at.elapsed().as_micros().min(u128::from(u64::MAX)) as u64,
        );
    }

    #[allow(dead_code)]
    pub fn sample(&self, system_name: &'static str) -> Option<NpcPerfSample> {
        self.samples.get(system_name).copied()
    }

    pub fn flush_if_due(&mut self, current_tick: u32) {
        if self.samples.is_empty() {
            return;
        }
        if current_tick.wrapping_sub(self.last_log_tick) < Self::LOG_INTERVAL_TICKS {
            return;
        }

        let mut entries = self
            .samples
            .iter()
            .map(|(name, sample)| {
                let avg = if sample.count == 0 {
                    0
                } else {
                    sample.total_us / sample.count
                };
                format!(
                    "{}={}us_avg/{}us_max/{}calls",
                    name, avg, sample.max_us, sample.count
                )
            })
            .collect::<Vec<_>>();
        entries.sort();

        tracing::info!("[npc-perf] tick {}: {}", current_tick, entries.join(" "));
        self.samples.clear();
        self.last_log_tick = current_tick;
    }
}

pub fn register(app: &mut App) {
    app.insert_resource(NpcPerfProbe::default());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_accumulates_total_count_and_max() {
        let mut probe = NpcPerfProbe::default();

        probe.record("social", 10);
        probe.record("social", 4);
        probe.record("social", 20);

        assert_eq!(
            probe.sample("social"),
            Some(NpcPerfSample {
                total_us: 34,
                count: 3,
                max_us: 20
            }),
            "sample should retain enough data to detect avg and spike regressions"
        );
    }

    #[test]
    fn flush_waits_for_log_interval() {
        let mut probe = NpcPerfProbe::default();
        probe.record("navigator", 7);

        probe.flush_if_due(NpcPerfProbe::LOG_INTERVAL_TICKS - 1);

        assert_eq!(
            probe.sample("navigator"),
            Some(NpcPerfSample {
                total_us: 7,
                count: 1,
                max_us: 7
            }),
            "early flush must not discard samples"
        );
    }

    #[test]
    fn flush_clears_due_samples() {
        let mut probe = NpcPerfProbe::default();
        probe.record("navigator", 7);

        probe.flush_if_due(NpcPerfProbe::LOG_INTERVAL_TICKS);

        assert_eq!(
            probe.sample("navigator"),
            None,
            "due flush should publish and reset the current window"
        );
    }
}
