//! plan-skill-v1 子技能系统（P0-P2）。
//!
//! 当前阶段：P0 数据契约 + 曲线 + 单测；P1 events + channel + IPC schema 双端；
//! P2 client 侧接入 InspectScreen 技艺 tab（仅服务端侧 event/channel/schema 对接点 + 消费 system）。
//!
//! 未启用 —— `main.rs` 的 `mod skill` 带 `#[allow(dead_code)]`，系统挂载等 P3+ 触发点接入时再 `register(app)`。

pub mod components;
pub mod config;
pub mod curve;
pub mod events;

use valence::prelude::{App, EventReader, EventWriter, IntoSystemConfigs, Query, Res, Update};

use crate::cultivation::breakthrough::skill_cap_for_realm;
use crate::cultivation::components::Cultivation;
use crate::cultivation::life_record::{LifeRecord, SkillMilestone};
use components::SkillSet;
use events::{SkillLvUp, SkillXpGain};

/// P1 阶段：注册 4 个 Event + 消费 `SkillXpGain` 的 system。
///
/// **尚未被 `main.rs` 调用**（见 plan §9 P1：各 plan 触发点对接在 P3+）。
/// 先提供 register 函数以便测试验证 framework 就绪。
pub fn register(app: &mut App) {
    app.init_resource::<config::SkillConfigStore>();
    app.insert_resource(config::SkillConfigSchemas::default());

    app.add_event::<SkillXpGain>();
    app.add_event::<SkillLvUp>();
    app.add_event::<events::SkillCapChanged>();
    app.add_event::<events::SkillScrollUsed>();

    app.add_systems(
        Update,
        (
            consume_skill_xp_gain,
            record_skill_lv_up.after(consume_skill_xp_gain),
        ),
    );
}

/// plan §8 事件消费：读 `SkillXpGain` → 更新对应玩家的 `SkillSet` →
/// 若跨级则每级写一条 `SkillLvUp`。Narration 字段由 agent 在 P5 补（这里留给 agent 消费 channel）。
pub fn consume_skill_xp_gain(
    gameplay_tick: Option<Res<crate::player::gameplay::GameplayTick>>,
    mut xp_events: EventReader<SkillXpGain>,
    mut lv_events: EventWriter<SkillLvUp>,
    mut sets: Query<(&mut SkillSet, Option<&Cultivation>)>,
) {
    let now = gameplay_tick.map(|t| t.current_tick()).unwrap_or(0);
    for evt in xp_events.read() {
        let Ok((mut set, cultivation)) = sets.get_mut(evt.char_entity) else {
            continue;
        };
        let entry = set.skills.entry(evt.skill).or_default();
        let cap = cultivation
            .map(|cultivation| skill_cap_for_realm(cultivation.realm))
            .unwrap_or(curve::SKILL_MAX_LEVEL);
        let scaled_amount = if entry.lv > cap && evt.amount > 0 {
            evt.amount.saturating_mul(3).div_ceil(10)
        } else {
            evt.amount
        };
        let leveled = curve::add_xp(entry, scaled_amount, now);
        for new_lv in leveled {
            lv_events.send(SkillLvUp {
                char_entity: evt.char_entity,
                skill: evt.skill,
                new_lv,
            });
        }
    }
}

pub fn record_skill_lv_up(
    gameplay_tick: Option<Res<crate::player::gameplay::GameplayTick>>,
    mut lv_events: EventReader<SkillLvUp>,
    mut players: Query<(&SkillSet, &mut LifeRecord)>,
) {
    let now = gameplay_tick.map(|t| t.current_tick()).unwrap_or(0);
    for event in lv_events.read() {
        let Ok((skill_set, mut life_record)) = players.get_mut(event.char_entity) else {
            continue;
        };
        let total_xp_at = skill_set
            .skills
            .get(&event.skill)
            .map(|entry| entry.total_xp)
            .unwrap_or(0);
        life_record.push_skill_milestone(SkillMilestone {
            skill: event.skill,
            new_lv: event.new_lv,
            achieved_at: now,
            narration: default_skill_lv_up_narration(event.skill, event.new_lv),
            total_xp_at,
        });
    }
}

fn default_skill_lv_up_narration(skill: components::SkillId, new_lv: u8) -> String {
    let skill_name = match skill {
        components::SkillId::Herbalism => "采药",
        components::SkillId::Alchemy => "炼丹",
        components::SkillId::Forging => "锻造",
        components::SkillId::Combat => "战斗",
        components::SkillId::Mineral => "采矿",
        components::SkillId::Cultivation => "修行",
    };
    format!("{skill_name}至 Lv.{new_lv}。手眼未必更快，只是旧误不再反复。")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::{Cultivation, Realm};
    use crate::skill::components::{SkillEntry, SkillId};
    use crate::skill::events::XpGainSource;
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

    #[test]
    fn xp_above_cap_is_scaled_down_to_thirty_percent() {
        let mut app = App::new();
        app.add_event::<SkillXpGain>();
        app.add_event::<SkillLvUp>();
        app.add_systems(Update, consume_skill_xp_gain);

        let mut skill_set = SkillSet::default();
        skill_set.skills.insert(
            SkillId::Herbalism,
            SkillEntry {
                lv: 5,
                xp: 0,
                total_xp: 0,
                last_action_at: 0,
                recent_repeat_count: 0,
            },
        );
        let entity = app
            .world_mut()
            .spawn((
                skill_set,
                Cultivation {
                    realm: Realm::Awaken,
                    ..Default::default()
                },
            ))
            .id();
        app.world_mut().send_event(SkillXpGain {
            char_entity: entity,
            skill: SkillId::Herbalism,
            amount: 100,
            source: events::XpGainSource::Action {
                plan_id: "botany",
                action: "harvest_manual",
            },
        });
        app.update();

        let set = app.world().get::<SkillSet>(entity).unwrap();
        let entry = set.skills.get(&SkillId::Herbalism).unwrap();
        assert_eq!(entry.lv, 5);
        assert_eq!(entry.xp, 30);
    }

    #[test]
    fn xp_below_cap_is_not_scaled() {
        let mut app = App::new();
        app.add_event::<SkillXpGain>();
        app.add_event::<SkillLvUp>();
        app.add_systems(Update, consume_skill_xp_gain);
        let entity = app
            .world_mut()
            .spawn((
                SkillSet::default(),
                Cultivation {
                    realm: Realm::Awaken,
                    ..Default::default()
                },
            ))
            .id();

        app.world_mut().send_event(SkillXpGain {
            char_entity: entity,
            skill: SkillId::Herbalism,
            amount: 100,
            source: events::XpGainSource::Action {
                plan_id: "botany",
                action: "harvest_manual",
            },
        });
        app.update();

        let set = app.world().get::<SkillSet>(entity).unwrap();
        let entry = set.skills.get(&SkillId::Herbalism).unwrap();
        assert_eq!(entry.lv, 1);
        assert_eq!(entry.xp, 0);
    }

    #[test]
    fn record_skill_lv_up_appends_milestone() {
        let mut app = App::new();
        app.add_event::<SkillLvUp>();
        app.add_systems(Update, record_skill_lv_up);
        let mut skill_set = SkillSet::default();
        skill_set.skills.insert(
            SkillId::Forging,
            SkillEntry {
                lv: 3,
                xp: 0,
                total_xp: 700,
                last_action_at: 0,
                recent_repeat_count: 0,
            },
        );
        let entity = app
            .world_mut()
            .spawn((skill_set, LifeRecord::default()))
            .id();
        app.world_mut().send_event(SkillLvUp {
            char_entity: entity,
            skill: SkillId::Forging,
            new_lv: 3,
        });

        app.update();

        let life = app.world().get::<LifeRecord>(entity).unwrap();
        assert_eq!(life.skill_milestones.len(), 1);
        assert_eq!(life.skill_milestones[0].total_xp_at, 700);
        assert_eq!(life.skill_milestones[0].new_lv, 3);
    }

    #[test]
    fn consume_skill_xp_gain_applies_over_cap_penalty_in_system() {
        let mut app = App::new();
        app.add_event::<SkillXpGain>();
        app.add_event::<SkillLvUp>();
        app.add_systems(Update, consume_skill_xp_gain);

        let entity = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Induce,
                    ..Default::default()
                },
                SkillSet {
                    skills: std::collections::HashMap::from([(
                        SkillId::Herbalism,
                        SkillEntry {
                            lv: 6,
                            xp: 10,
                            total_xp: 100,
                            last_action_at: 0,
                            recent_repeat_count: 0,
                        },
                    )]),
                    consumed_scrolls: Default::default(),
                },
            ))
            .id();

        app.world_mut().send_event(SkillXpGain {
            char_entity: entity,
            skill: SkillId::Herbalism,
            amount: 10,
            source: XpGainSource::Action {
                plan_id: "lingtian",
                action: "harvest_auto",
            },
        });

        app.update();

        let set = app
            .world()
            .get::<SkillSet>(entity)
            .expect("skill set should remain attached");
        let entry = set
            .skills
            .get(&SkillId::Herbalism)
            .expect("entry should exist");
        assert_eq!(
            entry.xp, 13,
            "10 xp over cap should be reduced to 3 before adding"
        );
        assert_eq!(
            entry.total_xp, 103,
            "total_xp should track effective awarded xp"
        );
        assert_eq!(
            entry.last_action_at, 0,
            "missing GameplayTick resource should fall back to tick 0"
        );
    }

    #[test]
    fn consume_skill_xp_gain_does_not_level_when_penalty_drops_below_threshold() {
        let mut app = App::new();
        app.add_event::<SkillXpGain>();
        app.add_event::<SkillLvUp>();
        app.add_systems(Update, consume_skill_xp_gain);

        let entity = app
            .world_mut()
            .spawn((
                Cultivation {
                    realm: Realm::Induce,
                    ..Default::default()
                },
                SkillSet {
                    skills: std::collections::HashMap::from([(
                        SkillId::Herbalism,
                        SkillEntry {
                            lv: 6,
                            xp: 4_891,
                            total_xp: 9_991,
                            last_action_at: 0,
                            recent_repeat_count: 0,
                        },
                    )]),
                    consumed_scrolls: Default::default(),
                },
            ))
            .id();

        app.world_mut().send_event(SkillXpGain {
            char_entity: entity,
            skill: SkillId::Herbalism,
            amount: 10,
            source: XpGainSource::Action {
                plan_id: "lingtian",
                action: "harvest_auto",
            },
        });

        app.update();

        let set = app
            .world()
            .get::<SkillSet>(entity)
            .expect("skill set should remain attached");
        let entry = set
            .skills
            .get(&SkillId::Herbalism)
            .expect("entry should exist");
        assert_eq!(
            entry.lv, 6,
            "penalized xp should no longer be enough to level up"
        );
        assert_eq!(
            entry.xp, 4_894,
            "only 3 effective xp should be added over cap"
        );
        assert_eq!(entry.total_xp, 9_994);

        let lv_events = app.world().resource::<Events<SkillLvUp>>();
        assert_eq!(
            lv_events.len(),
            0,
            "no SkillLvUp event should be emitted when the penalty prevents leveling"
        );
    }

    #[test]
    fn xp_gain_full_coverage_accumulates_for_every_skill_id() {
        let mut app = App::new();
        app.add_event::<SkillXpGain>();
        app.add_event::<SkillLvUp>();
        app.add_systems(Update, consume_skill_xp_gain);

        let entity = app
            .world_mut()
            .spawn((
                SkillSet::default(),
                Cultivation {
                    realm: Realm::Awaken,
                    ..Default::default()
                },
            ))
            .id();

        for skill in SkillId::ALL {
            app.world_mut().send_event(SkillXpGain {
                char_entity: entity,
                skill,
                amount: 1,
                source: XpGainSource::Action {
                    plan_id: "coverage",
                    action: skill.as_str(),
                },
            });
        }

        app.update();

        let set = app.world().get::<SkillSet>(entity).unwrap();
        for skill in SkillId::ALL {
            let entry = set.skills.get(&skill).expect("entry should be created");
            assert_eq!(entry.xp, 1, "{} xp should increment", skill.as_str());
            assert_eq!(
                entry.total_xp,
                1,
                "{} total_xp should increment",
                skill.as_str()
            );
        }
    }
}
