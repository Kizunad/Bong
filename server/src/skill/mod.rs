//! plan-skill-v1 子技能系统（P0-P2）。
//!
//! 当前阶段：P0 数据契约 + 曲线 + 单测；P1 events + channel + IPC schema 双端；
//! P2 client 侧接入 InspectScreen 技艺 tab（仅服务端侧 event/channel/schema 对接点 + 消费 system）。
//!
//! F26 — 已挂载：`main.rs` 顶部 `use bong_server::{ ..., skill, ... };` 正常声明本模块
//! （无 `#[allow(dead_code)]`），且 `main.rs` 内 `skill::register(&mut app)` 已被实际调用，
//! 本模块的 4 个 Event + [`consume_skill_xp_gain`] / [`record_skill_lv_up`] 两个 system 均已
//! 在运行时挂载执行。

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
/// F26 — 已被 `main.rs`（`skill::register(&mut app)`）实际调用挂载，非仅供测试。
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
