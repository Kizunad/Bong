//! plan-lingtian-v1 §1.3 / §1.4 — 灵气账本（区域级）+ lingtian-tick 计数器。
//!
//! `ZoneQiAccount` 是 lingtian 系统的区域灵气 facade；本地 f32 账面继续服务
//! plan-lingtian-v1 的"补灵 +0.5 / 抽吸 -0.5"绝对量语义，同时通过
//! `sync_world_qi_account` 把 zone 余额镜像到底盘 `WorldQiAccount`。
//!
//! `LingtianTickAccumulator` 把 Bevy tick（1/20s）累计到 lingtian-tick（60s）。

use std::collections::HashMap;

use valence::prelude::{bevy_ecs, Res, ResMut, Resource};

use crate::qi_physics::{QiAccountId, QiPhysicsError, WorldQiAccount};

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

    pub fn sync_world_qi_account(
        &self,
        account: &mut WorldQiAccount,
    ) -> Result<(), QiPhysicsError> {
        for (zone, value) in &self.qi {
            account.set_balance(QiAccountId::zone(zone.clone()), f64::from(value.max(0.0)))?;
        }
        Ok(())
    }
}

pub fn sync_zone_qi_account_to_world_qi_account(
    zone_qi: Option<Res<ZoneQiAccount>>,
    world_qi: Option<ResMut<WorldQiAccount>>,
) {
    let (Some(zone_qi), Some(mut world_qi)) = (zone_qi, world_qi) else {
        return;
    };
    if let Err(error) = zone_qi.sync_world_qi_account(&mut world_qi) {
        tracing::warn!("[bong][lingtian] failed to sync ZoneQiAccount to WorldQiAccount: {error}");
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
    fn zone_qi_account_syncs_to_world_qi_account_facade() {
        let mut acct = ZoneQiAccount::new();
        acct.set("field", 3.5);
        let mut world = WorldQiAccount::default();

        acct.sync_world_qi_account(&mut world).unwrap();

        assert_eq!(world.balance(&QiAccountId::zone("field")), 3.5);
    }

    #[test]
    fn sync_system_mirrors_zone_qi_account_into_world_qi_account() {
        let mut app = valence::prelude::App::new();
        let mut acct = ZoneQiAccount::new();
        acct.set("field", 3.5);
        app.insert_resource(acct);
        app.insert_resource(WorldQiAccount::default());
        app.add_systems(
            valence::prelude::Update,
            sync_zone_qi_account_to_world_qi_account,
        );

        app.update();

        let world = app.world().resource::<WorldQiAccount>();
        assert_eq!(world.balance(&QiAccountId::zone("field")), 3.5);
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
