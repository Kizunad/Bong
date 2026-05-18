//! plan-baomai-v4 §1.1 — ScarHistory 组件。
//!
//! 上层聚合器：从 `MeridianRippleScar.accumulated_overloads` 读取初始值（首次插入时
//! migration），之后通过订阅 `OverloadMeridianRippleEvent` 同步递增。
//! **不替代 MeridianRippleScar**——后者仍负责 severity 追踪和经脉 integrity 扣减。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component, EventReader, Query, Without};

use crate::combat::baomai_v3::events::{BaomaiSkillId, OverloadMeridianRippleEvent};
use crate::combat::baomai_v3::state::MeridianRippleScar;
use crate::cultivation::components::MeridianId;

/// 过载历史聚合器——累计过载事件次数 + 按技能/经脉分桶计数。
///
/// 只增不减，跨境界/死亡保留。首次插入时从 `MeridianRippleScar.accumulated_overloads` 同步。
#[derive(Component, Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScarHistory {
    /// 累计过载事件次数（只增不减，跨境界/死亡保留）。
    pub total_overloads: u32,
    /// 按技能分桶计数（崩拳 / 全力 / 撼山 / 焚血 / 散功）。
    pub overloads_by_skill: HashMap<BaomaiSkillId, u32>,
    /// 按经脉分桶计数（哪条脉被过载最多）。
    pub overloads_by_meridian: HashMap<MeridianId, u32>,
}

impl ScarHistory {
    /// 从现有 MeridianRippleScar 迁移构建。
    pub fn from_ripple_scar(scar: &MeridianRippleScar) -> Self {
        Self {
            total_overloads: scar.accumulated_overloads,
            overloads_by_skill: HashMap::new(),
            overloads_by_meridian: HashMap::new(),
        }
    }

    /// 记录一次过载事件。
    pub fn record_overload(&mut self, skill: BaomaiSkillId, meridians: &[MeridianId]) {
        self.total_overloads = self.total_overloads.saturating_add(1);
        let skill_count = self.overloads_by_skill.entry(skill).or_insert(0);
        *skill_count = skill_count.saturating_add(1);
        for &mid in meridians {
            let mid_count = self.overloads_by_meridian.entry(mid).or_insert(0);
            *mid_count = mid_count.saturating_add(1);
        }
    }
}

/// 订阅 `OverloadMeridianRippleEvent`，更新 `ScarHistory` 计数器。
///
/// 对无 `ScarHistory` 但有 `MeridianRippleScar` 的 entity，首次插入时调用
/// `from_ripple_scar()` 迁移。
pub fn scar_history_track_system(
    mut events: EventReader<OverloadMeridianRippleEvent>,
    mut with_history: Query<&mut ScarHistory>,
    without_history: Query<&MeridianRippleScar, Without<ScarHistory>>,
    mut commands: valence::prelude::Commands,
) {
    for event in events.read() {
        if let Ok(mut history) = with_history.get_mut(event.caster) {
            history.record_overload(event.skill, &event.meridian_ids);
        } else if let Ok(ripple_scar) = without_history.get(event.caster) {
            // First overload event for this entity — migrate from MeridianRippleScar.
            let mut history = ScarHistory::from_ripple_scar(ripple_scar);
            history.record_overload(event.skill, &event.meridian_ids);
            commands.entity(event.caster).insert(history);
        } else {
            // Entity has neither ScarHistory nor MeridianRippleScar — create fresh.
            let mut history = ScarHistory::default();
            history.record_overload(event.skill, &event.meridian_ids);
            commands.entity(event.caster).insert(history);
        }
    }
}
