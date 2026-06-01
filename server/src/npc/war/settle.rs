//! plan-offscreen-war-v1 P9：战事结算 system 集合。
//!
//! # 守恒证明（`regen_from_zone` 倍率安全性）
//!
//! `regen_from_zone(zone_qi, rate * m, integrity, room)` 内部（`excretion.rs:43`）：
//!   - `raw_gain = zone_qi * rate * m * integrity * QI_CULTIVATION_REGEN_RATE`
//!   - `capped_gain = raw_gain.min(room)`
//!   - `drain = capped_gain / QI_ZONE_UNIT_CAPACITY`
//!   - 恒有 `gain == drain * QI_ZONE_UNIT_CAPACITY`（函数内生不变量）
//!
//! 调用侧：`cultivation.qi_current += gain; zone.spirit_qi -= drain`（tick.rs:211-212）
//! 或 dormant 路径 ledger `QiTransfer`（dormant:1454，`set_balance` 来自真实余额）。
//!
//! 乘以 `m`（WAR_WINNER_ZONE_REGEN_MULTIPLIER = 1.10 / WAR_LOSER_ZONE_REGEN_MULTIPLIER = 0.95）
//! 只改 `rate`，zone 减多少 player 加多少，**零和**。`total_observed` 全程不变。

use std::collections::HashMap;

use valence::prelude::{bevy_ecs, EventReader, EventWriter, Res, ResMut, Resource};

use crate::npc::movement::GameTick;
use crate::npc::war::{FactionWarOutcome, WarPhase, WarPhaseChanged, WarRole};
use crate::social::events::SocialRenownDeltaEvent;

// ──────────────────────────── ZoneSpiritBonusStore ───────────────────────────

/// plan-offscreen-war-v1 P9：战事 zone 的 regen 速率倍率表。
///
/// key = zone name（String），value = 倍率（f64，默认 1.0）。
/// `multiplier_for` 查不到时返回 1.0，保证未参战 zone 不受影响。
///
/// **守恒安全**：倍率仅作用于 `regen_from_zone` 的 `rate` 参数；不直接修改
/// 任何真元账户、不 emit QiTransfer、不新增 QiTransferReason 变体。
/// 参见模块文档的守恒证明。
#[derive(Resource, Default, Debug)]
pub struct ZoneSpiritBonusStore {
    /// zone name → regen 速率倍率（默认查不到即 1.0）
    pub multipliers: HashMap<String, f64>,
}

impl ZoneSpiritBonusStore {
    /// 查询 zone 的 regen 速率倍率；未参战 zone 返回 1.0（中性，不受影响）。
    pub fn multiplier_for(&self, zone: &str) -> f64 {
        self.multipliers.get(zone).copied().unwrap_or(1.0)
    }
}

// ──────────────────────────── helper ─────────────────────────────────────────

fn current_game_tick(game_tick: Option<&GameTick>) -> u64 {
    game_tick.map(|t| t.0 as u64).unwrap_or(0)
}

// ──────────────────────────── apply_war_zone_spirit_bonus ────────────────────

/// plan-offscreen-war-v1 P9：读 `WarPhaseChanged` 事件沿，写入/清除 `ZoneSpiritBonusStore`。
///
/// - Settling（有 outcome）：胜方 zone `multipliers[zone] = WAR_WINNER_ZONE_REGEN_MULTIPLIER(1.10)`。
/// - Aftermath：清除 zone 条目，恢复到默认 1.0（余波消散）。
/// - 其他阶段：不触碰 store。
///
/// 守恒安全：store 只存倍率整数，不触任何真元账户。
pub fn apply_war_zone_spirit_bonus(
    mut phase_events: EventReader<WarPhaseChanged>,
    mut bonus_store: ResMut<ZoneSpiritBonusStore>,
) {
    use crate::qi_physics::constants::WAR_WINNER_ZONE_REGEN_MULTIPLIER;

    for event in phase_events.read() {
        match event.phase {
            WarPhase::Settling => {
                if event.outcome.is_some() {
                    // 胜方主导该 zone → +10% regen 速率
                    bonus_store
                        .multipliers
                        .insert(event.zone.clone(), WAR_WINNER_ZONE_REGEN_MULTIPLIER);
                }
            }
            WarPhase::Aftermath => {
                // 余波消散 → 恢复到默认 1.0（移除条目即可）
                bonus_store.multipliers.remove(&event.zone);
            }
            _ => {}
        }
    }
}

// ──────────────────────────── award_war_winner_renown ─────────────────────────

/// plan-offscreen-war-v1 P9：战事胜方参战玩家获 Renown 奖励。
///
/// 仅在 `WarPhase::Settling`（有 outcome）时触发一次：
/// - Enlist + allied=winner_group → fame_delta=5，reason="war_winner_enlist"
/// - Mercenary + allied=winner_group → fame_delta=3，reason="war_winner_mercenary"
/// - Intercept/Spectate/loser 侧 → 不发
/// - notoriety_delta=0，tags_added=[]（reframe b：零具名 tag，零真元）
///
/// Aftermath / 其他阶段均不触发，防重复奖励。
pub fn award_war_winner_renown(
    mut phase_events: EventReader<WarPhaseChanged>,
    mut renown_deltas: EventWriter<SocialRenownDeltaEvent>,
    game_tick: Option<Res<GameTick>>,
) {
    let now = current_game_tick(game_tick.as_deref());

    for event in phase_events.read() {
        // 仅 Settling + 有 outcome
        let WarPhase::Settling = event.phase else {
            continue;
        };
        let Some(FactionWarOutcome { winner_group, .. }) = &event.outcome else {
            continue;
        };

        for role_rec in &event.war_snapshot_player_roles {
            let fame_delta = match role_rec.role {
                WarRole::Enlist if role_rec.allied_group == Some(*winner_group) => 5,
                WarRole::Mercenary if role_rec.allied_group == Some(*winner_group) => 3,
                _ => continue,
            };
            renown_deltas.send(SocialRenownDeltaEvent {
                char_id: role_rec.player_id.clone(),
                fame_delta,
                notoriety_delta: 0,
                tags_added: vec![],
                tick: now,
                reason: if fame_delta == 5 {
                    "war_winner_enlist".to_string()
                } else {
                    "war_winner_mercenary".to_string()
                },
            });
        }
    }
}

// ──────────────────────────── 单测 ────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use bevy_ecs::event::Events;
    use bevy_ecs::schedule::Schedule;
    use bevy_ecs::world::World;

    use crate::npc::faction::EmergentGroupId;
    use crate::npc::war::{
        FactionWarOutcome, PlayerFactionRole, PlayerRoleCounts, WarId, WarPhase, WarPhaseChanged,
        WarRole,
    };
    use crate::qi_physics::{
        constants::{
            QI_ZONE_UNIT_CAPACITY, WAR_LOSER_ZONE_REGEN_MULTIPLIER,
            WAR_WINNER_ZONE_REGEN_MULTIPLIER,
        },
        excretion::regen_from_zone,
    };
    use crate::social::events::SocialRenownDeltaEvent;

    use super::*;

    // ─────────── 辅助函数 ──────────────────────────────────────────────────────

    fn make_settling_event(zone: &str, winner: u16, loser: u16) -> WarPhaseChanged {
        WarPhaseChanged {
            war_id: WarId(1),
            zone: zone.to_string(),
            region_descriptor: format!("{zone}一带散修"),
            phase: WarPhase::Settling,
            groups: vec![EmergentGroupId(winner), EmergentGroupId(loser)],
            outcome: Some(FactionWarOutcome {
                winner_group: EmergentGroupId(winner),
                loser_group: EmergentGroupId(loser),
                total_casualties: 10,
                settled_tick: 100,
            }),
            player_role_counts: PlayerRoleCounts::default(),
            war_snapshot_player_roles: vec![],
            at_tick: 100,
        }
    }

    fn make_aftermath_event(zone: &str, winner: u16, loser: u16) -> WarPhaseChanged {
        WarPhaseChanged {
            war_id: WarId(1),
            zone: zone.to_string(),
            region_descriptor: format!("{zone}一带散修"),
            phase: WarPhase::Aftermath,
            groups: vec![EmergentGroupId(winner), EmergentGroupId(loser)],
            outcome: Some(FactionWarOutcome {
                winner_group: EmergentGroupId(winner),
                loser_group: EmergentGroupId(loser),
                total_casualties: 10,
                settled_tick: 100,
            }),
            player_role_counts: PlayerRoleCounts::default(),
            war_snapshot_player_roles: vec![],
            at_tick: 300,
        }
    }

    fn make_skirmish_event(zone: &str) -> WarPhaseChanged {
        WarPhaseChanged {
            war_id: WarId(2),
            zone: zone.to_string(),
            region_descriptor: format!("{zone}一带散修"),
            phase: WarPhase::Skirmish,
            groups: vec![EmergentGroupId(0), EmergentGroupId(1)],
            outcome: None,
            player_role_counts: PlayerRoleCounts::default(),
            war_snapshot_player_roles: vec![],
            at_tick: 50,
        }
    }

    fn run_zone_bonus_system(
        store: ZoneSpiritBonusStore,
        events: Vec<WarPhaseChanged>,
    ) -> ZoneSpiritBonusStore {
        let mut world = World::new();
        world.insert_resource(store);
        world.insert_resource(Events::<WarPhaseChanged>::default());

        {
            let mut queue = world.resource_mut::<Events<WarPhaseChanged>>();
            for ev in events {
                queue.send(ev);
            }
        }

        let mut schedule = Schedule::default();
        schedule.add_systems(apply_war_zone_spirit_bonus);
        schedule.run(&mut world);

        world.remove_resource::<ZoneSpiritBonusStore>().unwrap()
    }

    fn run_renown_system(
        player_roles: Vec<PlayerFactionRole>,
        outcome: Option<FactionWarOutcome>,
        phase: WarPhase,
        zone: &str,
    ) -> Vec<SocialRenownDeltaEvent> {
        let mut world = World::new();
        world.insert_resource(Events::<WarPhaseChanged>::default());
        world.insert_resource(Events::<SocialRenownDeltaEvent>::default());

        {
            let mut queue = world.resource_mut::<Events<WarPhaseChanged>>();
            queue.send(WarPhaseChanged {
                war_id: WarId(1),
                zone: zone.to_string(),
                region_descriptor: format!("{zone}一带散修"),
                phase,
                groups: vec![EmergentGroupId(0), EmergentGroupId(1)],
                outcome: outcome.clone(),
                player_role_counts: PlayerRoleCounts::default(),
                war_snapshot_player_roles: player_roles,
                at_tick: 100,
            });
        }

        let mut schedule = Schedule::default();
        schedule.add_systems(award_war_winner_renown);
        schedule.run(&mut world);

        world
            .resource::<Events<SocialRenownDeltaEvent>>()
            .iter_current_update_events()
            .cloned()
            .collect()
    }

    // ─────────── A. ZoneSpiritBonusStore 基础接口 ─────────────────────────────

    #[test]
    fn store_default_multiplier_is_one() {
        // 空 store 查任意 zone → 1.0（期望未参战 zone 不受影响）
        let store = ZoneSpiritBonusStore::default();
        assert_eq!(
            store.multiplier_for("残灰谷"),
            1.0,
            "期望空 store 返回 1.0（中性倍率），因未参战 zone 不应受影响，实际 {}",
            store.multiplier_for("残灰谷")
        );
        assert_eq!(
            store.multiplier_for("无此zone"),
            1.0,
            "期望任意未注册 zone 返回 1.0，实际 {}",
            store.multiplier_for("无此zone")
        );
    }

    #[test]
    fn store_winner_and_loser_constants() {
        // 接口完整性 + 常量 pin：写入 winner/loser 倍率，读回相等
        let mut store = ZoneSpiritBonusStore::default();
        store
            .multipliers
            .insert("zone_w".to_string(), WAR_WINNER_ZONE_REGEN_MULTIPLIER);
        store
            .multipliers
            .insert("zone_l".to_string(), WAR_LOSER_ZONE_REGEN_MULTIPLIER);
        assert_eq!(
            store.multiplier_for("zone_w"),
            WAR_WINNER_ZONE_REGEN_MULTIPLIER,
            "期望 winner 倍率 == WAR_WINNER_ZONE_REGEN_MULTIPLIER(1.10)，实际 {}",
            store.multiplier_for("zone_w")
        );
        assert_eq!(
            store.multiplier_for("zone_l"),
            WAR_LOSER_ZONE_REGEN_MULTIPLIER,
            "期望 loser 倍率 == WAR_LOSER_ZONE_REGEN_MULTIPLIER(0.95)，实际 {}",
            store.multiplier_for("zone_l")
        );
    }

    // ─────────── B. ZoneSpiritBonus qi_physics 接入守恒测试 ──────────────────

    #[test]
    fn bonus_multiplier_routes_through_regen_from_zone() {
        // bonus_multiplier 乘在 rate 上，调 regen_from_zone，10% 倍率下 gain ≈ 1.10x
        let zone_qi = 0.8_f64;
        let rate = 0.5_f64;
        let integrity = 0.9_f64;
        let room = 10.0_f64; // 足够大，不封顶

        let (gain_base, drain_base) = regen_from_zone(zone_qi, rate, integrity, room);
        let (gain_bonus, drain_bonus) = regen_from_zone(
            zone_qi,
            rate * WAR_WINNER_ZONE_REGEN_MULTIPLIER,
            integrity,
            room,
        );

        assert!(
            gain_bonus > gain_base,
            "期望 bonus 倍率下 gain 大于基准（因 rate 乘 1.10 放大 regen），实际 gain_base={} gain_bonus={}",
            gain_base, gain_bonus
        );
        // 比值约 1.10（数值精度内）
        let ratio = gain_bonus / gain_base;
        assert!(
            (ratio - WAR_WINNER_ZONE_REGEN_MULTIPLIER).abs() < 1e-9,
            "期望 gain_bonus/gain_base ≈ WAR_WINNER_ZONE_REGEN_MULTIPLIER(1.10) 因线性乘积，实际 ratio={}",
            ratio
        );
        // regen_from_zone 内生守恒不变量：gain == drain * QI_ZONE_UNIT_CAPACITY
        assert!(
            (gain_base - drain_base * QI_ZONE_UNIT_CAPACITY).abs() < 1e-12,
            "期望 regen_from_zone 守恒（gain==drain*UNIT_CAPACITY），实际 gain_base={} drain_base*CAP={}",
            gain_base, drain_base * QI_ZONE_UNIT_CAPACITY
        );
        assert!(
            (gain_bonus - drain_bonus * QI_ZONE_UNIT_CAPACITY).abs() < 1e-12,
            "期望 bonus 路径 regen_from_zone 守恒，实际 gain_bonus={} drain_bonus*CAP={}",
            gain_bonus,
            drain_bonus * QI_ZONE_UNIT_CAPACITY
        );
    }

    #[test]
    fn bonus_conserves_total_zone_plus_player() {
        // 手算一次 regen tick：zone + player 总量应守恒（倍率只改速率，不创造/销毁）
        let zone_qi_before = 0.6_f64;
        let player_qi_before = 5.0_f64;
        let rate = 0.4_f64;
        let integrity = 1.0_f64;
        let room = 20.0_f64;
        let multiplier = WAR_WINNER_ZONE_REGEN_MULTIPLIER;

        let (gain, drain) = regen_from_zone(zone_qi_before, rate * multiplier, integrity, room);
        let zone_qi_after = zone_qi_before - drain;
        let player_qi_after = player_qi_before + gain;

        // zone + player before = zone + player after（守恒）
        // 注意：zone 和 player 单位不同（zone 是浓度 × QI_ZONE_UNIT_CAPACITY = 真元点）
        let before_total = zone_qi_before * QI_ZONE_UNIT_CAPACITY + player_qi_before;
        let after_total = zone_qi_after * QI_ZONE_UNIT_CAPACITY + player_qi_after;
        assert!(
            (before_total - after_total).abs() < 1e-10,
            "期望 zone+player 总量守恒（倍率仅改速率不铸造/销毁真元），实际 before={} after={} diff={}",
            before_total, after_total, (before_total - after_total).abs()
        );
    }

    #[test]
    fn loser_multiplier_also_conserves() {
        // 败方 0.95 倍率同样守恒
        let zone_qi_before = 0.5_f64;
        let player_qi_before = 3.0_f64;
        let rate = 0.3_f64;
        let integrity = 0.8_f64;
        let room = 15.0_f64;
        let multiplier = WAR_LOSER_ZONE_REGEN_MULTIPLIER;

        let (gain, drain) = regen_from_zone(zone_qi_before, rate * multiplier, integrity, room);
        let before_total = zone_qi_before * QI_ZONE_UNIT_CAPACITY + player_qi_before;
        let after_total =
            (zone_qi_before - drain) * QI_ZONE_UNIT_CAPACITY + player_qi_before + gain;
        assert!(
            (before_total - after_total).abs() < 1e-10,
            "期望 loser 倍率(0.95) 下 zone+player 总量守恒，实际 before={} after={}",
            before_total,
            after_total
        );
    }

    // ─────────── C. apply_war_zone_spirit_bonus system ───────────────────────

    #[test]
    fn settle_writes_winner_zone_multiplier() {
        // Settling 事件 → store["残灰谷"] == WAR_WINNER_ZONE_REGEN_MULTIPLIER(1.10)
        let store = ZoneSpiritBonusStore::default();
        let events = vec![make_settling_event("残灰谷", 0, 1)];
        let result = run_zone_bonus_system(store, events);
        assert_eq!(
            result.multiplier_for("残灰谷"),
            WAR_WINNER_ZONE_REGEN_MULTIPLIER,
            "期望 Settling 后 store[\"残灰谷\"] == 1.10（胜方 zone 加速 regen），实际 {}",
            result.multiplier_for("残灰谷")
        );
    }

    #[test]
    fn aftermath_clears_zone_multiplier() {
        // Aftermath 事件 → store.multiplier_for("残灰谷") == 1.0（余波消散）
        let mut store = ZoneSpiritBonusStore::default();
        store
            .multipliers
            .insert("残灰谷".to_string(), WAR_WINNER_ZONE_REGEN_MULTIPLIER);
        let events = vec![make_aftermath_event("残灰谷", 0, 1)];
        let result = run_zone_bonus_system(store, events);
        assert_eq!(
            result.multiplier_for("残灰谷"),
            1.0,
            "期望 Aftermath 后 zone 倍率恢复到 1.0（余波消散后不再加成），实际 {}",
            result.multiplier_for("残灰谷")
        );
    }

    #[test]
    fn skirmish_does_not_change_store() {
        // Skirmish 事件 → store 不变
        let store = ZoneSpiritBonusStore::default();
        let events = vec![make_skirmish_event("残灰谷")];
        let result = run_zone_bonus_system(store, events);
        assert_eq!(
            result.multiplier_for("残灰谷"),
            1.0,
            "期望 Skirmish 阶段不改 store（仅 Settling/Aftermath 触发），实际 {}",
            result.multiplier_for("残灰谷")
        );
    }

    #[test]
    fn settling_without_outcome_does_not_write_store() {
        // Settling 但 outcome=None → 不写 store（无胜方可言）
        let store = ZoneSpiritBonusStore::default();
        let mut event = make_settling_event("残灰谷", 0, 1);
        event.outcome = None;
        let events = vec![event];
        let result = run_zone_bonus_system(store, events);
        assert_eq!(
            result.multiplier_for("残灰谷"),
            1.0,
            "期望 Settling 无 outcome 不写 store，实际 {}",
            result.multiplier_for("残灰谷")
        );
    }

    // ─────────── D. award_war_winner_renown system ────────────────────────────

    fn make_player_role(player_id: &str, role: WarRole, allied: Option<u16>) -> PlayerFactionRole {
        PlayerFactionRole {
            player_id: player_id.to_string(),
            role,
            allied_group: allied.map(EmergentGroupId),
            joined_tick: 0,
        }
    }

    fn make_outcome(winner: u16, loser: u16) -> FactionWarOutcome {
        FactionWarOutcome {
            winner_group: EmergentGroupId(winner),
            loser_group: EmergentGroupId(loser),
            total_casualties: 8,
            settled_tick: 100,
        }
    }

    #[test]
    fn settle_awards_enlist_winner_fame_5() {
        // P1 role=Enlist allied=winner(G0) → Settling 后 fame_delta=5, reason="war_winner_enlist"
        let roles = vec![make_player_role("P1", WarRole::Enlist, Some(0))];
        let events = run_renown_system(
            roles,
            Some(make_outcome(0, 1)),
            WarPhase::Settling,
            "残灰谷",
        );
        assert_eq!(
            events.len(),
            1,
            "期望 Enlist 胜方 emit 1 条 SocialRenownDeltaEvent，实际 {}",
            events.len()
        );
        let ev = &events[0];
        assert_eq!(ev.char_id, "P1", "期望 char_id=P1，实际 {}", ev.char_id);
        assert_eq!(
            ev.fame_delta, 5,
            "期望 Enlist 胜方 fame_delta=5，实际 {}",
            ev.fame_delta
        );
        assert_eq!(
            ev.notoriety_delta, 0,
            "期望 notoriety_delta=0（守恒：renown 纯叙事整数，零真元），实际 {}",
            ev.notoriety_delta
        );
        assert_eq!(
            ev.reason, "war_winner_enlist",
            "期望 reason='war_winner_enlist'，实际 {}",
            ev.reason
        );
    }

    #[test]
    fn settle_awards_mercenary_winner_fame_3() {
        // role=Mercenary allied=winner → fame_delta=3
        let roles = vec![make_player_role("P2", WarRole::Mercenary, Some(0))];
        let events = run_renown_system(
            roles,
            Some(make_outcome(0, 1)),
            WarPhase::Settling,
            "残灰谷",
        );
        assert_eq!(
            events.len(),
            1,
            "期望 Mercenary 胜方 emit 1 条，实际 {}",
            events.len()
        );
        let ev = &events[0];
        assert_eq!(
            ev.fame_delta, 3,
            "期望 Mercenary fame_delta=3，实际 {}",
            ev.fame_delta
        );
        assert_eq!(
            ev.reason, "war_winner_mercenary",
            "期望 reason='war_winner_mercenary'，实际 {}",
            ev.reason
        );
    }

    #[test]
    fn settle_skips_loser_and_spectate() {
        // P2 Enlist allied=loser(G1)、P3 Spectate → 均无 SocialRenownDeltaEvent
        let roles = vec![
            make_player_role("P2", WarRole::Enlist, Some(1)), // loser group
            make_player_role("P3", WarRole::Spectate, None),
        ];
        let events = run_renown_system(
            roles,
            Some(make_outcome(0, 1)),
            WarPhase::Settling,
            "残灰谷",
        );
        assert!(
            events.is_empty(),
            "期望 loser 侧和 Spectate 角色均无 SocialRenownDeltaEvent（非胜方），实际 {} 条",
            events.len()
        );
    }

    #[test]
    fn aftermath_does_not_reaward() {
        // Aftermath 阶段 → 零 SocialRenownDeltaEvent（仅 Settling 触发，防重复奖励）
        let roles = vec![make_player_role("P1", WarRole::Enlist, Some(0))];
        let events = run_renown_system(
            roles,
            Some(make_outcome(0, 1)),
            WarPhase::Aftermath,
            "残灰谷",
        );
        assert!(
            events.is_empty(),
            "期望 Aftermath 阶段零奖励（仅 Settling 触发），实际 {} 条",
            events.len()
        );
    }

    #[test]
    fn renown_award_is_zero_qi() {
        // Renown 事件 notoriety_delta==0（守恒：renown 纯叙事整数，不触任何 ledger）
        let roles = vec![make_player_role("P1", WarRole::Enlist, Some(0))];
        let events = run_renown_system(
            roles,
            Some(make_outcome(0, 1)),
            WarPhase::Settling,
            "残灰谷",
        );
        assert!(!events.is_empty(), "应有事件");
        for ev in &events {
            assert_eq!(
                ev.notoriety_delta, 0,
                "期望 notoriety_delta=0（renown 守恒：纯叙事整数，零真元），实际 {}",
                ev.notoriety_delta
            );
            assert!(
                ev.tags_added.is_empty(),
                "期望 tags_added 为空（reframe b：无具名 tag），实际 {:?}",
                ev.tags_added
            );
        }
    }

    #[test]
    fn intercept_does_not_get_renown() {
        // Intercept 角色不获 renown（无论哪侧）
        let roles = vec![make_player_role("P4", WarRole::Intercept, None)];
        let events = run_renown_system(
            roles,
            Some(make_outcome(0, 1)),
            WarPhase::Settling,
            "残灰谷",
        );
        assert!(
            events.is_empty(),
            "期望 Intercept 不获 renown（只有 Enlist/Mercenary 胜方才奖励），实际 {} 条",
            events.len()
        );
    }

    #[test]
    fn multiple_winners_all_get_renown() {
        // 两个 Enlist 胜方 → 各得 5
        let roles = vec![
            make_player_role("PA", WarRole::Enlist, Some(0)),
            make_player_role("PB", WarRole::Enlist, Some(0)),
            make_player_role("PC", WarRole::Enlist, Some(1)), // loser
        ];
        let events = run_renown_system(
            roles,
            Some(make_outcome(0, 1)),
            WarPhase::Settling,
            "残灰谷",
        );
        assert_eq!(
            events.len(),
            2,
            "期望两名胜方 Enlist 各获一条奖励（2 条），实际 {}",
            events.len()
        );
        assert!(events
            .iter()
            .all(|e| e.fame_delta == 5 && e.notoriety_delta == 0));
    }
}

// ──────────────────────────── e2e 集成测试 ───────────────────────────────────
//
// 用 Bevy World + Schedule 验证完整系统链路：
// apply_war_zone_spirit_bonus + award_war_winner_renown 在 WarPhaseChanged 事件驱动下的行为。
// 守恒验证走 regen_from_zone 纯函数（不需要真实 tick loop 即可验证 zone+player 零和不变）。

#[cfg(test)]
mod e2e_tests {
    use bevy_ecs::event::Events;
    use bevy_ecs::schedule::Schedule;
    use bevy_ecs::world::World;

    use crate::npc::faction::EmergentGroupId;
    use crate::npc::war::{
        FactionWarOutcome, PlayerFactionRole, PlayerRoleCounts, WarId, WarPhase, WarPhaseChanged,
        WarRole,
    };
    use crate::qi_physics::{
        constants::{QI_ZONE_UNIT_CAPACITY, WAR_WINNER_ZONE_REGEN_MULTIPLIER},
        excretion::regen_from_zone,
    };
    use crate::social::events::SocialRenownDeltaEvent;

    use super::*;

    const ZONE: &str = "残灰谷";

    fn make_e2e_world() -> World {
        let mut world = World::new();
        world.insert_resource(ZoneSpiritBonusStore::default());
        world.insert_resource(Events::<WarPhaseChanged>::default());
        world.insert_resource(Events::<SocialRenownDeltaEvent>::default());
        world
    }

    fn make_e2e_schedule() -> Schedule {
        let mut schedule = Schedule::default();
        schedule.add_systems(apply_war_zone_spirit_bonus);
        schedule.add_systems(award_war_winner_renown);
        schedule
    }

    fn settling_ev(
        winner: u16,
        loser: u16,
        player_roles: Vec<PlayerFactionRole>,
    ) -> WarPhaseChanged {
        WarPhaseChanged {
            war_id: WarId(10),
            zone: ZONE.to_string(),
            region_descriptor: "残灰谷一带散修".to_string(),
            phase: WarPhase::Settling,
            groups: vec![EmergentGroupId(winner), EmergentGroupId(loser)],
            outcome: Some(FactionWarOutcome {
                winner_group: EmergentGroupId(winner),
                loser_group: EmergentGroupId(loser),
                total_casualties: 8,
                settled_tick: 100,
            }),
            player_role_counts: PlayerRoleCounts::default(),
            war_snapshot_player_roles: player_roles,
            at_tick: 100,
        }
    }

    fn aftermath_ev(winner: u16, loser: u16) -> WarPhaseChanged {
        WarPhaseChanged {
            war_id: WarId(10),
            zone: ZONE.to_string(),
            region_descriptor: "残灰谷一带散修".to_string(),
            phase: WarPhase::Aftermath,
            groups: vec![EmergentGroupId(winner), EmergentGroupId(loser)],
            outcome: Some(FactionWarOutcome {
                winner_group: EmergentGroupId(winner),
                loser_group: EmergentGroupId(loser),
                total_casualties: 8,
                settled_tick: 100,
            }),
            player_role_counts: PlayerRoleCounts::default(),
            war_snapshot_player_roles: vec![],
            at_tick: 300,
        }
    }

    // ── e2e 1: Settling → ZoneSpiritBonus 写入 ──

    #[test]
    fn e2e_settling_writes_zone_spirit_bonus() {
        let mut world = make_e2e_world();
        let mut schedule = make_e2e_schedule();

        {
            let mut events = world.resource_mut::<Events<WarPhaseChanged>>();
            events.send(settling_ev(1, 2, vec![]));
        }
        schedule.run(&mut world);

        let bonus = world.resource::<ZoneSpiritBonusStore>();
        assert_eq!(
            bonus.multiplier_for(ZONE),
            WAR_WINNER_ZONE_REGEN_MULTIPLIER,
            "期望 Settling 后 ZoneSpiritBonusStore[\"{}\"] == 1.10（胜方 zone regen 加速），实际 {}",
            ZONE, bonus.multiplier_for(ZONE)
        );
    }

    // ── e2e 2: Aftermath → ZoneSpiritBonus 清除 ──

    #[test]
    fn e2e_aftermath_clears_zone_spirit_bonus() {
        let mut world = make_e2e_world();
        let mut schedule = make_e2e_schedule();

        // 预先写入倍率
        world
            .resource_mut::<ZoneSpiritBonusStore>()
            .multipliers
            .insert(ZONE.to_string(), WAR_WINNER_ZONE_REGEN_MULTIPLIER);

        {
            let mut events = world.resource_mut::<Events<WarPhaseChanged>>();
            events.send(aftermath_ev(1, 2));
        }
        schedule.run(&mut world);

        let bonus = world.resource::<ZoneSpiritBonusStore>();
        assert_eq!(
            bonus.multiplier_for(ZONE),
            1.0,
            "期望 Aftermath 后倍率恢复 1.0（余波消散后 store 条目被移除），实际 {}",
            bonus.multiplier_for(ZONE)
        );
    }

    // ── e2e 3: regen_from_zone 守恒（带 war 倍率）──

    #[test]
    fn e2e_regen_with_war_multiplier_conserves_total() {
        let zone_qi_before = 0.7_f64;
        let player_qi_before = 8.0_f64;
        let rate = 0.4_f64;
        let integrity = 0.9_f64;
        let room = 20.0_f64;

        let (gain, drain) = regen_from_zone(
            zone_qi_before,
            rate * WAR_WINNER_ZONE_REGEN_MULTIPLIER,
            integrity,
            room,
        );

        // zone + player 总量守恒
        let before_total = zone_qi_before * QI_ZONE_UNIT_CAPACITY + player_qi_before;
        let after_total =
            (zone_qi_before - drain) * QI_ZONE_UNIT_CAPACITY + player_qi_before + gain;
        assert!(
            (before_total - after_total).abs() < 1e-10,
            "期望 war 倍率下 zone+player 总量守恒（倍率仅改 regen 速率不铸造真元），\
             before={:.8} after={:.8} diff={:.2e}",
            before_total,
            after_total,
            (before_total - after_total).abs()
        );
        // 内生不变量
        assert!(
            (gain - drain * QI_ZONE_UNIT_CAPACITY).abs() < 1e-12,
            "期望 regen_from_zone 内生守恒 gain==drain*UNIT_CAPACITY，gain={} drain*CAP={}",
            gain,
            drain * QI_ZONE_UNIT_CAPACITY
        );
        // 倍率线性提升
        let (gain_base, _) = regen_from_zone(zone_qi_before, rate, integrity, room);
        let ratio = gain / gain_base;
        assert!(
            (ratio - WAR_WINNER_ZONE_REGEN_MULTIPLIER).abs() < 1e-9,
            "期望 war 倍率(1.10) 线性，ratio={ratio:.8}",
        );
    }

    // ── e2e 4: Settling + Enlist 胜方 → fame_delta=5 + notoriety=0 ──

    #[test]
    fn e2e_settling_enlist_winner_gets_fame_5_zero_qi() {
        let mut world = make_e2e_world();
        let mut schedule = make_e2e_schedule();

        let roles = vec![
            PlayerFactionRole {
                player_id: "P_winner".to_string(),
                role: WarRole::Enlist,
                allied_group: Some(EmergentGroupId(1)), // winner
                joined_tick: 0,
            },
            PlayerFactionRole {
                player_id: "P_loser".to_string(),
                role: WarRole::Enlist,
                allied_group: Some(EmergentGroupId(2)), // loser
                joined_tick: 0,
            },
        ];

        {
            let mut events = world.resource_mut::<Events<WarPhaseChanged>>();
            events.send(settling_ev(1, 2, roles));
        }
        schedule.run(&mut world);

        let renown: Vec<_> = world
            .resource::<Events<SocialRenownDeltaEvent>>()
            .iter_current_update_events()
            .cloned()
            .collect();

        assert_eq!(
            renown.len(),
            1,
            "期望仅胜方 Enlist emit 1 条 SocialRenownDeltaEvent（败方不奖励），实际 {}",
            renown.len()
        );
        let ev = &renown[0];
        assert_eq!(ev.char_id, "P_winner");
        assert_eq!(
            ev.fame_delta, 5,
            "期望 Enlist 胜方 fame_delta=5，实际 {}",
            ev.fame_delta
        );
        assert_eq!(
            ev.notoriety_delta, 0,
            "期望 notoriety_delta=0（守恒：renown 纯叙事整数，零真元），实际 {}",
            ev.notoriety_delta
        );
    }

    // ── e2e 5: Aftermath 不重复奖励 ──

    #[test]
    fn e2e_aftermath_does_not_reaward_renown() {
        let mut world = make_e2e_world();
        let mut schedule = make_e2e_schedule();

        {
            let mut events = world.resource_mut::<Events<WarPhaseChanged>>();
            events.send(aftermath_ev(1, 2));
        }
        schedule.run(&mut world);

        let renown: Vec<_> = world
            .resource::<Events<SocialRenownDeltaEvent>>()
            .iter_current_update_events()
            .cloned()
            .collect();

        assert!(
            renown.is_empty(),
            "期望 Aftermath 不奖励（仅 Settling 触发，防重复），实际 {} 条",
            renown.len()
        );
    }
}
