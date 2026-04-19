//! plan-skill-v1 子技能系统（P0-P2）。
//!
//! 当前阶段：P0 数据契约 + 曲线 + 单测；P1 events + channel + IPC schema 双端；
//! P2 client 侧接入 InspectScreen 技艺 tab（仅服务端侧 event/channel/schema 对接点 + 消费 system）。
//!
//! 未启用 —— `main.rs` 的 `mod skill` 带 `#[allow(dead_code)]`，系统挂载等 P3+ 触发点接入时再 `register(app)`。

pub mod components;
pub mod curve;
pub mod events;

use valence::prelude::{App, EventReader, EventWriter, Query, Res, Update};

use components::SkillSet;
use events::{SkillLvUp, SkillXpGain};

/// P1 阶段：注册 4 个 Event + 消费 `SkillXpGain` 的 system。
///
/// **尚未被 `main.rs` 调用**（见 plan §9 P1：各 plan 触发点对接在 P3+）。
/// 先提供 register 函数以便测试验证 framework 就绪。
pub fn register(app: &mut App) {
    app.add_event::<SkillXpGain>();
    app.add_event::<SkillLvUp>();
    app.add_event::<events::SkillCapChanged>();
    app.add_event::<events::SkillScrollUsed>();

    app.add_systems(Update, consume_skill_xp_gain);
}

/// plan §8 事件消费：读 `SkillXpGain` → 更新对应玩家的 `SkillSet` →
/// 若跨级则每级写一条 `SkillLvUp`。Narration 字段由 agent 在 P5 补（这里留给 agent 消费 channel）。
pub fn consume_skill_xp_gain(
    gameplay_tick: Option<Res<crate::player::gameplay::GameplayTick>>,
    mut xp_events: EventReader<SkillXpGain>,
    mut lv_events: EventWriter<SkillLvUp>,
    mut sets: Query<&mut SkillSet>,
) {
    let now = gameplay_tick.map(|t| t.current_tick()).unwrap_or(0);
    for evt in xp_events.read() {
        let Ok(mut set) = sets.get_mut(evt.char_entity) else {
            continue;
        };
        let entry = set.skills.entry(evt.skill).or_default();
        let leveled = curve::add_xp(entry, evt.amount, now);
        for new_lv in leveled {
            lv_events.send(SkillLvUp {
                char_entity: evt.char_entity,
                skill: evt.skill,
                new_lv,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::Events;

    #[test]
    fn register_adds_all_four_events() {
        let mut app = App::new();
        register(&mut app);
        assert!(app.world().contains_resource::<Events<SkillXpGain>>());
        assert!(app.world().contains_resource::<Events<SkillLvUp>>());
        assert!(app
            .world()
            .contains_resource::<Events<events::SkillCapChanged>>());
        assert!(app
            .world()
            .contains_resource::<Events<events::SkillScrollUsed>>());
    }
}
