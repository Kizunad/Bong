//! QiColorEvolutionTick — 真元色演化（plan §2 / §1.1）。
//!
//! 简化模型：每个玩家维护 `PracticeLog`（Component），记录各色的练习权重。
//! 按窗口内比例判定：
//!   * 任一项 > 60% → main = 该色
//!   * 次项 > 25% → secondary
//!   * ≥3 项 > 15% → is_chaotic = true
//!   * 至少 5 色且全部 < 25% → is_hunyuan = true
//!
//! P1：实际"练习事件"来源（打坐/战斗动作/丹药）由上层后续接入，这里只提供
//! tick + 纯函数 + PracticeLog Component 作为接口。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, Component, Entity, Event, EventReader, Query, Res, ResMut, Username,
};

use super::color_bonus::color_style_bonus;
use super::components::{ColorKind, QiColor};
use super::life_record::{BiographyEntry, LifeRecord};
use super::tick::CultivationClock;
use crate::player::gameplay::PendingGameplayNarrations;
use crate::schema::common::NarrationStyle;

pub const STYLE_PRACTICE_AMOUNT: f64 = 1.0;
pub const PRACTICE_DECAY_PER_TICK: f64 = 0.001;
pub const CULTIVATION_SESSION_PRACTICE_TICKS_PER_MINUTE: u64 = 20 * 60;

/// 玩家修习累积日志 — 权重值可由 gameplay 系统增加，tick 会慢慢衰减。
#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct PracticeLog {
    pub weights: HashMap<ColorKind, f64>,
    pub decay_per_tick: f64,
}

impl Default for PracticeLog {
    fn default() -> Self {
        Self {
            weights: HashMap::new(),
            decay_per_tick: PRACTICE_DECAY_PER_TICK,
        }
    }
}

impl PracticeLog {
    pub fn add(&mut self, color: ColorKind, amount: f64) {
        *self.weights.entry(color).or_insert(0.0) += amount;
    }

    pub fn decay(&mut self) {
        if self.decay_per_tick <= 0.0 {
            return;
        }
        for w in self.weights.values_mut() {
            *w = (*w - self.decay_per_tick).max(0.0);
        }
        self.weights.retain(|_, w| *w > 0.0);
    }

    pub fn total(&self) -> f64 {
        self.weights.values().sum()
    }
}

/// 记录一次招式练习，并按玩家当前 QiColor 应用效率倍率。
///
/// `qi_color` 为 `None` 时（例如 NPC、无色状态初始化阶段）退化为不带加成的默认 1.0x。
pub fn record_style_practice(log: &mut PracticeLog, color: ColorKind, qi_color: Option<&QiColor>) {
    let bonus = qi_color
        .map(|qc| color_style_bonus(qc, color))
        .unwrap_or(1.0);
    log.add(color, STYLE_PRACTICE_AMOUNT * bonus);
}

pub fn is_hunyuan(log: &PracticeLog) -> bool {
    let total = log.total();
    if total <= f64::EPSILON || log.weights.len() < 5 {
        return false;
    }
    log.weights.values().all(|weight| (*weight / total) < 0.25)
}

#[derive(Debug, Clone, Copy, Event)]
pub struct CultivationSessionPracticeEvent {
    pub entity: Entity,
    pub active_color: ColorKind,
    pub elapsed_ticks: u64,
}

/// 记录一次打坐练习（按分钟计），并按玩家当前 QiColor 应用效率倍率。
///
/// 返回实际累积的分钟数（< 1 分钟不累积）。
/// `qi_color` 为 `None` 时退化为 1.0x（无加成）。
pub fn record_cultivation_session_practice(
    log: &mut PracticeLog,
    active_color: ColorKind,
    elapsed_ticks: u64,
    qi_color: Option<&QiColor>,
) -> u64 {
    let minutes = elapsed_ticks / CULTIVATION_SESSION_PRACTICE_TICKS_PER_MINUTE;
    if minutes > 0 {
        let bonus = qi_color
            .map(|qc| color_style_bonus(qc, active_color))
            .unwrap_or(1.0);
        log.add(active_color, STYLE_PRACTICE_AMOUNT * minutes as f64 * bonus);
    }
    minutes
}

pub fn record_cultivation_session_practice_events(
    mut events: EventReader<CultivationSessionPracticeEvent>,
    mut logs: Query<(&mut PracticeLog, Option<&QiColor>)>,
) {
    for event in events.read() {
        if let Ok((mut log, qi_color)) = logs.get_mut(event.entity) {
            record_cultivation_session_practice(
                &mut log,
                event.active_color,
                event.elapsed_ticks,
                qi_color,
            );
        }
    }
}

/// 纯函数：基于日志权重演化 QiColor（plan §2 QiColorEvolutionTick 规则）。
pub fn evolve_qi_color(log: &PracticeLog, out: &mut QiColor) {
    let total = log.total();
    if total <= 0.0 {
        return;
    }
    let mut sorted: Vec<(ColorKind, f64)> =
        log.weights.iter().map(|(k, v)| (*k, v / total)).collect();
    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let over15 = sorted.iter().filter(|(_, r)| *r > 0.15).count();
    // 混元：至少 5 色且所有项均 < 25%
    if is_hunyuan(log) {
        out.is_hunyuan = true;
        out.is_chaotic = false;
        out.secondary = None;
        return;
    }
    // 杂色：≥3 项 > 15%
    if over15 >= 3 {
        out.is_chaotic = true;
        out.is_hunyuan = false;
        out.secondary = None;
        return;
    }

    out.is_chaotic = false;
    out.is_hunyuan = false;
    if let Some(&(main_k, main_r)) = sorted.first() {
        if main_r > 0.60 {
            out.main = main_k;
        }
    }
    if let Some(&(sec_k, sec_r)) = sorted.get(1) {
        if sec_r > 0.25 {
            out.secondary = Some(sec_k);
        } else {
            out.secondary = None;
        }
    } else {
        out.secondary = None;
    }
}

/// 检测真元色演化里程碑，返回对应的 narration 文本（如有）。
///
/// 四种里程碑（plan §P3）：
/// 1. 首次主色涌现：before.main == Mellow && color.main != Mellow
/// 2. 色调转换：before.main != Mellow && before.main != color.main
/// 3. 杂色堕落：!before.is_chaotic && color.is_chaotic
/// 4. 混元觉醒：!before.is_hunyuan && color.is_hunyuan
///
/// 优先级：杂色 > 混元 > 首次涌现 > 色调转换（同 tick 只触发一条）
///
/// `hint` 是调用方提供的区分 hint（例如 `entity.to_bits() ^ tick`），用于在
/// 多条文本变体间轮替，避免死代码。传 0 则取第一条。
pub fn detect_color_milestone(
    before: &QiColor,
    after: &QiColor,
    hint: u64,
) -> Option<&'static str> {
    // 杂色堕落（优先）
    if !before.is_chaotic && after.is_chaotic {
        let texts: &[&str] = &[
            "你什么都练，什么都不精。真元在你体内像一锅乱炖。",
            "五色杂陈，互相掣肘。你的真元已经失去了方向。",
        ];
        return Some(texts[(hint as usize) % texts.len()]);
    }
    // 混元觉醒
    if !before.is_hunyuan && after.is_hunyuan {
        let texts: &[&str] = &[
            "五色均衡，无主无从。这不是退而求其次——这是另一种路。",
            "你不是什么都懂——你是站在所有路的交汇处，看见了更大的地图。",
        ];
        return Some(texts[(hint as usize) % texts.len()]);
    }
    // 首次主色涌现
    if before.main == ColorKind::Mellow && after.main != ColorKind::Mellow {
        let texts: &[&str] = &[
            "你的真元开始沉淀出一种倾向——尚不明朗，但已与从前不同。",
            "真元在你丹田之中，渐渐染上了某种性质。你说不清是什么，但它就在那里。",
        ];
        return Some(texts[(hint as usize) % texts.len()]);
    }
    // 色调转换（旧主色 ≠ Mellow 且变化到另一种主色）
    // 额外排除：before 是杂色/混元时退出杂色/混元同 tick 换主色不触发「色调转换」narration，
    // 避免混淆（真正的事件是「退出杂色/混元」，不是「色调转换」）
    if !before.is_chaotic
        && !before.is_hunyuan
        && before.main != ColorKind::Mellow
        && before.main != after.main
    {
        let texts: &[&str] = &[
            "旧日的沉淀在松动。你走向了另一条轨迹。",
            "有什么东西在你体内移位——不是破坏，是转向。",
        ];
        return Some(texts[(hint as usize) % texts.len()]);
    }
    None
}

pub fn qi_color_evolution_tick(
    clock: Res<CultivationClock>,
    mut players: Query<(
        Entity,
        &mut PracticeLog,
        &mut QiColor,
        Option<&mut LifeRecord>,
    )>,
    usernames: Query<&Username>,
    mut pending_narrations: Option<ResMut<PendingGameplayNarrations>>,
) {
    for (entity, mut log, mut color, life_record) in players.iter_mut() {
        let before = color.clone();
        let had_signal = log.total() > 0.0;
        log.decay();
        evolve_qi_color(&log, &mut color);
        if had_signal
            && (before.main != color.main
                || before.secondary != color.secondary
                || before.is_chaotic != color.is_chaotic
                || before.is_hunyuan != color.is_hunyuan)
        {
            if let Some(mut life_record) = life_record {
                life_record.push(BiographyEntry::ColorShift {
                    main: color.main,
                    secondary: color.secondary,
                    tick: clock.tick,
                });
            }
            // P3: emit narration for color milestones
            let milestone_hint = entity.to_bits() ^ clock.tick;
            if let Some(text) = detect_color_milestone(&before, &color, milestone_hint) {
                if let Some(ref mut narrations) = pending_narrations {
                    if let Ok(username) = usernames.get(entity) {
                        narrations.push_player(
                            username.0.as_str(),
                            text,
                            NarrationStyle::Perception,
                        );
                    }
                }
            }
        }
    }
}
