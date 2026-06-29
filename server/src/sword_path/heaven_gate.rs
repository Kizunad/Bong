//! plan-sword-path-v1 P4 — 化虚·一剑开天门运行时逻辑。
//!
//! 包含：
//! - HeavenGateCastEvent → 触发结算（legacy，保留供内部引用）
//! - HeavenGateChanneling → 四阶段时序组件（plan-sword-path-complete §D）
//! - TiandaoBlindZoneRegistry resource → 管理活跃盲区
//! - tick system → 过期清理
//! - player_in_blind_zone() → 供 agent bridge 过滤

use valence::prelude::{bevy_ecs, Component, DVec3, Entity, Event, Resource};

// ── 四阶段时序常数（plan-sword-path-complete §D）────────────────────────────

/// 蓄力阶段结束 tick（[0, 60) 蓄力）。
pub const HEAVEN_GATE_CHARGE_END: u32 = 60;
/// 临界阶段结束 tick（[60, 120) 临界 / 区域警告）。
pub const HEAVEN_GATE_CRITICAL_END: u32 = 120;
/// AoE 结算阶段结束 tick（[120, 140) AoE 结算）。
pub const HEAVEN_GATE_AOE_END: u32 = 140;
// [140, ∞) → aftermath（qi 归零 / 境界跌落 / 盲区 / QiTransfer）

/// 四阶段 channeling 组件。
///
/// cast_heaven_gate 不再立即结算，改 insert 此组件；`heaven_gate_phase_system`
/// 按 elapsed 推进各阶段；aftermath 完成后 remove 组件（并 despawn 实体若需要）。
///
/// **守恒不变量**：`qi_max`/`stored_qi` 在 cast 时快照；charge 阶段不动 qi，
/// 结算仅在 aftermath（elapsed >= AOE_END），与旧 `heaven_gate_cast_system` net 账目完全一致。
#[derive(Debug, Clone, Component)]
pub struct HeavenGateChanneling {
    /// 施法者实体（通常 == 组件所在 Entity，但保存以便 system 使用）。
    pub caster: Entity,
    /// 开始 tick（用于计算 elapsed = now - start_tick）。
    pub start_tick: u64,
    /// 施法位置（用于 AoE 范围 / 盲区 / QiTransfer zone 查找）。
    pub position: DVec3,
    /// cast 时快照的 qi_max（aftermath 结算基准，不受 charge 阶段影响）。
    pub qi_max: f64,
    /// cast 时快照的 bond.stored_qi（aftermath 结算基准）。
    pub stored_qi: f64,
    /// AoE 只结算一次的 guard。
    pub aoe_done: bool,
}

use super::techniques::effects;
use super::tiandao_blind::TiandaoBlindZone;

#[derive(Debug, Clone, Event)]
pub struct HeavenGateCastEvent {
    pub caster: Entity,
    pub position: DVec3,
    pub qi_max: f64,
    pub stored_qi: f64,
}

#[derive(Debug, Clone, Default, Resource)]
pub struct TiandaoBlindZoneRegistry {
    zones: Vec<TiandaoBlindZone>,
}

impl TiandaoBlindZoneRegistry {
    pub fn add(&mut self, zone: TiandaoBlindZone) {
        self.zones.push(zone);
    }

    pub fn tick_expire(&mut self, current_tick: u64) {
        self.zones.retain(|z| !z.is_expired(current_tick));
    }

    pub fn is_player_hidden(&self, pos: DVec3) -> bool {
        self.zones.iter().any(|z| z.contains(pos))
    }

    pub fn active_count(&self) -> usize {
        self.zones.len()
    }

    pub fn zones(&self) -> &[TiandaoBlindZone] {
        &self.zones
    }
}

pub fn create_blind_zone_from_cast(
    event: &HeavenGateCastEvent,
    current_tick: u64,
) -> TiandaoBlindZone {
    TiandaoBlindZone {
        center: event.position,
        radius: effects::HEAVEN_GATE_RADIUS,
        ttl_ticks: effects::HEAVEN_GATE_BLIND_ZONE_TTL_TICKS,
        created_tick: current_tick,
    }
}

pub fn compute_heaven_gate_damage(staging_buffer: f64, distance: f64) -> f64 {
    let attenuation = (-effects::QI_SLASH_ATTENUATION_PER_BLOCK * distance).exp();
    staging_buffer * attenuation * effects::HEAVEN_GATE_DEFENSE_IGNORE as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_starts_empty() {
        let reg = TiandaoBlindZoneRegistry::default();
        assert_eq!(reg.active_count(), 0);
    }

    #[test]
    fn add_and_query() {
        let mut reg = TiandaoBlindZoneRegistry::default();
        reg.add(TiandaoBlindZone {
            center: DVec3::new(0.0, 64.0, 0.0),
            radius: 100.0,
            ttl_ticks: 6000,
            created_tick: 100,
        });
        assert_eq!(reg.active_count(), 1);
        assert!(reg.is_player_hidden(DVec3::new(50.0, 64.0, 0.0)));
        assert!(!reg.is_player_hidden(DVec3::new(200.0, 64.0, 0.0)));
    }

    #[test]
    fn tick_expire_removes_old() {
        let mut reg = TiandaoBlindZoneRegistry::default();
        reg.add(TiandaoBlindZone {
            center: DVec3::ZERO,
            radius: 50.0,
            ttl_ticks: 100,
            created_tick: 0,
        });
        reg.add(TiandaoBlindZone {
            center: DVec3::new(500.0, 0.0, 0.0),
            radius: 50.0,
            ttl_ticks: 200,
            created_tick: 0,
        });
        assert_eq!(reg.active_count(), 2);
        reg.tick_expire(150);
        assert_eq!(
            reg.active_count(),
            1,
            "first zone should expire at tick 100"
        );
        reg.tick_expire(250);
        assert_eq!(
            reg.active_count(),
            0,
            "second zone should expire at tick 200"
        );
    }

    #[test]
    fn multiple_zones_overlap() {
        let mut reg = TiandaoBlindZoneRegistry::default();
        reg.add(TiandaoBlindZone {
            center: DVec3::new(0.0, 0.0, 0.0),
            radius: 60.0,
            ttl_ticks: 1000,
            created_tick: 0,
        });
        reg.add(TiandaoBlindZone {
            center: DVec3::new(50.0, 0.0, 0.0),
            radius: 60.0,
            ttl_ticks: 1000,
            created_tick: 0,
        });
        assert!(
            reg.is_player_hidden(DVec3::new(25.0, 0.0, 0.0)),
            "point in overlap should be hidden"
        );
        assert!(
            reg.is_player_hidden(DVec3::new(-55.0, 0.0, 0.0)),
            "point in first zone only should be hidden"
        );
        assert!(
            reg.is_player_hidden(DVec3::new(105.0, 0.0, 0.0)),
            "point in second zone only should be hidden"
        );
        assert!(
            !reg.is_player_hidden(DVec3::new(200.0, 0.0, 0.0)),
            "point outside both should not be hidden"
        );
    }

    #[test]
    fn create_blind_zone_uses_plan_constants() {
        let event = HeavenGateCastEvent {
            caster: Entity::from_raw(1),
            position: DVec3::new(100.0, 64.0, 200.0),
            qi_max: 10700.0,
            stored_qi: 3000.0,
        };
        let zone = create_blind_zone_from_cast(&event, 5000);
        assert!(
            (zone.radius - 100.0).abs() < 1e-6,
            "radius should be HEAVEN_GATE_RADIUS=100"
        );
        assert_eq!(
            zone.ttl_ticks,
            effects::HEAVEN_GATE_BLIND_ZONE_TTL_TICKS,
            "ttl should match plan constant"
        );
        assert_eq!(zone.created_tick, 5000);
        assert_eq!(zone.center, event.position);
    }

    #[test]
    fn heaven_gate_damage_at_zero_distance() {
        let dmg = compute_heaven_gate_damage(13700.0, 0.0);
        assert!(
            (dmg - 13700.0 * 0.5).abs() < 1e-6,
            "at distance 0, damage = staging * defense_ignore = 13700 * 0.5 = 6850, got {dmg}"
        );
    }

    #[test]
    fn heaven_gate_damage_decays_with_distance() {
        let dmg_close = compute_heaven_gate_damage(13700.0, 10.0);
        let dmg_far = compute_heaven_gate_damage(13700.0, 50.0);
        assert!(
            dmg_close > dmg_far,
            "closer should deal more damage: {dmg_close} > {dmg_far}"
        );
    }

    #[test]
    fn heaven_gate_damage_at_100_blocks() {
        let dmg = compute_heaven_gate_damage(13700.0, 100.0);
        assert!(
            dmg < 700.0,
            "at 100 blocks, damage should be low (exponential decay), got {dmg}"
        );
        assert!(
            dmg > 100.0,
            "at 100 blocks, damage should still be nonzero, got {dmg}"
        );
    }

    #[test]
    fn heaven_gate_damage_zero_buffer() {
        let dmg = compute_heaven_gate_damage(0.0, 10.0);
        assert_eq!(dmg, 0.0);
    }
}
