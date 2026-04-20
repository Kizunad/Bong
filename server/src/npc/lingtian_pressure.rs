//! plan-lingtian-v1 §5.1 — npc 系统消费 `ZonePressureCrossed{ level: High }`
//! 在该 zone 某个 plot 周围 spawn 3×3 道伥（worldview §八.1 注视规则）。
//!
//! 本模块跨边界订阅 lingtian 事件而不让 lingtian 反向引用 npc — 单向依赖。
//!
//! 道伥实体当前用 zombie archetype 兜底（与 spawn::spawn_zombie_npc_at 同档）。
//! 后续如有专门的"道伥" archetype（异色 / 高伤），换 archetype 即可。

use valence::prelude::{
    App, ChunkLayer, Commands, DVec3, Entity, EntityLayer, EventReader, IntoSystemConfigs, Query,
    Update, With,
};

use crate::lingtian::pressure::PressureLevel;
use crate::lingtian::{LingtianPlot, ZonePressureCrossed};

use super::spawn::spawn_zombie_npc_at;

/// 触发时在 zone 某 plot 周围 spawn 9 个 zombie 作为 "道伥"。
pub fn spawn_daoshen_on_pressure_high(
    mut events: EventReader<ZonePressureCrossed>,
    plots: Query<&LingtianPlot>,
    layers: Query<Entity, (With<ChunkLayer>, With<EntityLayer>)>,
    mut commands: Commands,
) {
    for e in events.read() {
        if !matches!(e.level, PressureLevel::High) {
            continue;
        }
        // 当前简化：取第一个 plot 作为道伥围殴目标（plot ↔ zone 1:1 假设）
        let Some(target_plot) = plots.iter().next() else {
            tracing::warn!(
                "[bong][npc][daoshen] zone `{}` HIGH triggered but no LingtianPlot found",
                e.zone
            );
            continue;
        };
        let Ok(layer) = layers.get_single() else {
            tracing::warn!("[bong][npc][daoshen] no chunk+entity layer; skip spawn");
            continue;
        };
        let center = DVec3::new(
            target_plot.pos.x as f64,
            target_plot.pos.y as f64,
            target_plot.pos.z as f64,
        );
        // plan §5.1 — 3×3 范围 9 个
        let mut spawned = 0;
        for dx in -1..=1i32 {
            for dz in -1..=1i32 {
                let pos = DVec3::new(center.x + dx as f64, center.y, center.z + dz as f64);
                spawn_zombie_npc_at(&mut commands, layer, &e.zone, pos, pos);
                spawned += 1;
            }
        }
        tracing::warn!(
            "[bong][npc][daoshen] zone `{}` HIGH pressure ({:.3}) → spawned {spawned} 道伥 around plot {:?}",
            e.zone,
            e.raw_pressure,
            target_plot.pos
        );
    }
}

pub fn register(app: &mut App) {
    app.add_systems(Update, spawn_daoshen_on_pressure_high.chain());
}
