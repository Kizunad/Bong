//! plan-lingtian-v1 §1.3 / §1.4 — 灵气账本（区域级）+ lingtian-tick 计数器。
//!
//! `ZoneQiAccount` 是 lingtian 系统的区域灵气 facade，与 `world::zone::Zone.spirit_qi`
//! （归一化 -1..=1 用于 NPC AI / heal）保持边界清晰。所有分账比例由
//! `qi_physics::constants` 主张，避免灵田模块继续自带物理常量。设计原因：
//!   * plan §1.4 把"补灵 +0.5 / 抽吸 -0.5"作为绝对量记，与 -1..=1 归一化不兼容
//!   * lingtian 仍需要自有 plot/zone 视图；跨系统总账由 WorldQiAccount 统一汇总
//!
//! `LingtianTickAccumulator` 把 Bevy tick（1/20s）累计到 lingtian-tick（60s）。

use std::collections::HashMap;

use valence::prelude::{bevy_ecs, Resource};

/// plan §4 — LingtianTick 周期 = 1 min = 1200 Bevy tick @ 20tps。
pub const BEVY_TICKS_PER_LINGTIAN_TICK: u32 = 1200;

/// lingtian 自有灵气账本。Key 为 zone 名（与 `world::zone::Zone.name` 同集合）。
#[derive(Debug, Default, Resource)]
pub struct ZoneQiAccount {
    qi: HashMap<String, f32>,
}

/// 默认 zone 名（plan §1.3 zone 解析未实装时的 fallback）。
pub const DEFAULT_ZONE: &str = "default";

impl ZoneQiAccount {
    pub fn new() -> Self {
        Self::default()
    }

    /// 用某个 baseline 在指定 zone 设初值（loader / 测试用）。
    pub fn set(&mut self, zone: impl Into<String>, value: f32) {
        self.qi.insert(zone.into(), value.max(0.0));
    }

    pub fn get(&self, zone: &str) -> f32 {
        self.qi.get(zone).copied().unwrap_or(0.0)
    }

    /// 拿可变引用；不存在则插入 0 后返回。
    pub fn get_mut(&mut self, zone: &str) -> &mut f32 {
        self.qi.entry(zone.to_string()).or_insert(0.0)
    }

    pub fn zones(&self) -> impl Iterator<Item = &String> {
        self.qi.keys()
    }
}

/// Bevy tick → lingtian-tick 累计器。
#[derive(Debug, Default, Resource)]
pub struct LingtianTickAccumulator {
    bevy_ticks: u32,
}

impl LingtianTickAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    /// 累一个 Bevy tick，若达到周期则返回 true 并归零（让上层跑一次 lingtian-tick）。
    pub fn step(&mut self) -> bool {
        self.bevy_ticks = self.bevy_ticks.saturating_add(1);
        if self.bevy_ticks >= BEVY_TICKS_PER_LINGTIAN_TICK {
            self.bevy_ticks = 0;
            true
        } else {
            false
        }
    }

    pub fn raw(&self) -> u32 {
        self.bevy_ticks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_get_round_trip() {
        let mut acct = ZoneQiAccount::new();
        acct.set("default", 5.0);
        assert_eq!(acct.get("default"), 5.0);
        assert_eq!(acct.get("missing"), 0.0);
    }

    #[test]
    fn negative_set_clamped_to_zero() {
        let mut acct = ZoneQiAccount::new();
        acct.set("a", -3.0);
        assert_eq!(acct.get("a"), 0.0);
    }

    #[test]
    fn get_mut_inserts_zero() {
        let mut acct = ZoneQiAccount::new();
        let r = acct.get_mut("new");
        assert_eq!(*r, 0.0);
        *r = 7.0;
        assert_eq!(acct.get("new"), 7.0);
    }

    #[test]
    fn accumulator_fires_after_full_cycle() {
        let mut acc = LingtianTickAccumulator::new();
        for _ in 0..BEVY_TICKS_PER_LINGTIAN_TICK - 1 {
            assert!(!acc.step());
        }
        assert!(acc.step(), "第 {} 步应触发", BEVY_TICKS_PER_LINGTIAN_TICK);
        // 触发后归零，下一周期重新数
        assert_eq!(acc.raw(), 0);
        for _ in 0..BEVY_TICKS_PER_LINGTIAN_TICK - 1 {
            assert!(!acc.step());
        }
        assert!(acc.step());
    }
}
