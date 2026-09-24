//! 动态环境 overlay（plan-dense-fog-v1 P1 前置）。
//!
//! `weather_environment_sync_system` 每 tick 用 `replace_for_dimension` 全量重算
//! zone effects，直接 `ZoneEnvironmentRegistry::add` 的效果活不过一帧。本模块提供
//! 参与 sync 组装的动态雾堤源：dev `/fog` 命令（后续天道 agent 命令）写入这里，
//! sync 时按 AABB 相交附着到 zone，寿命到期自动摘除。

use valence::prelude::{bevy_ecs, Resource};

use crate::world::environment::EnvironmentEffect;
use crate::world::zone::Zone;

/// 雾堤默认色（末法灰白，压向冷调）。
pub const DEFAULT_FOG_BANK_TINT: [u8; 3] = [120, 126, 133];

/// 一片动态雾堤：任意 AABB + 密度 + 可选寿命（tick）。
#[derive(Debug, Clone, PartialEq)]
pub struct FogBank {
    pub id: String,
    /// dimension ident 字符串（与 `Zone::dimension.ident_str()` 同口径）。
    pub dimension: String,
    pub aabb_min: [f64; 3],
    pub aabb_max: [f64; 3],
    pub tint_rgb: [u8; 3],
    pub density: f32,
    /// `None` = 常驻直到显式清除。
    pub remaining_ticks: Option<u64>,
}

#[derive(Debug, Default, Resource)]
pub struct EnvironmentOverlays {
    fog_banks: Vec<FogBank>,
    next_id: u64,
}

impl EnvironmentOverlays {
    /// 登记一片雾堤，返回分配的 id（`fog_<n>`）。
    ///
    /// 入参防御：min/max 逐轴归一化；density 非有限值归 0，否则钳到 0..=1。
    pub fn spawn_fog_bank(
        &mut self,
        dimension: impl Into<String>,
        aabb_a: [f64; 3],
        aabb_b: [f64; 3],
        tint_rgb: [u8; 3],
        density: f32,
        duration_ticks: Option<u64>,
    ) -> String {
        self.next_id += 1;
        let id = format!("fog_{}", self.next_id);
        let mut aabb_min = [0.0; 3];
        let mut aabb_max = [0.0; 3];
        for axis in 0..3 {
            aabb_min[axis] = aabb_a[axis].min(aabb_b[axis]);
            aabb_max[axis] = aabb_a[axis].max(aabb_b[axis]);
        }
        let density = if density.is_finite() {
            density.clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.fog_banks.push(FogBank {
            id: id.clone(),
            dimension: dimension.into(),
            aabb_min,
            aabb_max,
            tint_rgb,
            density,
            remaining_ticks: duration_ticks,
        });
        id
    }

    /// 按 id 移除，返回是否命中。
    pub fn remove_fog_bank(&mut self, id: &str) -> bool {
        let before = self.fog_banks.len();
        self.fog_banks.retain(|bank| bank.id != id);
        self.fog_banks.len() != before
    }

    /// 清空全部雾堤，返回清除数量。
    pub fn clear_fog_banks(&mut self) -> usize {
        let count = self.fog_banks.len();
        self.fog_banks.clear();
        count
    }

    pub fn fog_banks(&self) -> &[FogBank] {
        &self.fog_banks
    }

    /// 每 tick 调一次：递减剩余寿命，摘除到期雾堤，返回到期 id 列表。
    pub fn tick_expiry(&mut self) -> Vec<String> {
        let mut expired = Vec::new();
        self.fog_banks.retain_mut(|bank| {
            let Some(remaining) = bank.remaining_ticks.as_mut() else {
                return true;
            };
            *remaining = remaining.saturating_sub(1);
            if *remaining == 0 {
                expired.push(bank.id.clone());
                false
            } else {
                true
            }
        });
        expired
    }

    /// 与该 zone 同 dimension 且 AABB 相交（闭区间）的雾堤，映射为 FogVeil 效果。
    pub fn fog_effects_for_zone(&self, zone: &Zone) -> Vec<EnvironmentEffect> {
        let zone_dimension = zone.dimension.ident_str();
        let (zone_min, zone_max) = zone.bounds;
        let zone_min = [zone_min.x, zone_min.y, zone_min.z];
        let zone_max = [zone_max.x, zone_max.y, zone_max.z];
        self.fog_banks
            .iter()
            .filter(|bank| bank.dimension == zone_dimension)
            .filter(|bank| aabb_overlaps(bank.aabb_min, bank.aabb_max, zone_min, zone_max))
            .map(|bank| EnvironmentEffect::FogVeil {
                aabb_min: bank.aabb_min,
                aabb_max: bank.aabb_max,
                tint_rgb: bank.tint_rgb,
                density: bank.density,
            })
            .collect()
    }
}

fn aabb_overlaps(a_min: [f64; 3], a_max: [f64; 3], b_min: [f64; 3], b_max: [f64; 3]) -> bool {
    (0..3).all(|axis| a_min[axis] <= b_max[axis] && a_max[axis] >= b_min[axis])
}
