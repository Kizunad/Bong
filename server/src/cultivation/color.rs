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
pub fn detect_color_milestone(before: &QiColor, after: &QiColor) -> Option<&'static str> {
    // 杂色堕落（优先）
    if !before.is_chaotic && after.is_chaotic {
        // 两条示例，随机轮流 — 纯函数里用简单 hash 选择
        let texts: &[&str] = &[
            "你什么都练，什么都不精。真元在你体内像一锅乱炖。",
            "五色杂陈，互相掣肘。你的真元已经失去了方向。",
        ];
        return Some(texts[0]);
    }
    // 混元觉醒
    if !before.is_hunyuan && after.is_hunyuan {
        let texts: &[&str] = &[
            "五色均衡，无主无从。这不是退而求其次——这是另一种路。",
            "你不是什么都懂——你是站在所有路的交汇处，看见了更大的地图。",
        ];
        return Some(texts[0]);
    }
    // 首次主色涌现
    if before.main == ColorKind::Mellow && after.main != ColorKind::Mellow {
        let texts: &[&str] = &[
            "你的真元开始沉淀出一种倾向——尚不明朗，但已与从前不同。",
            "真元在你丹田之中，渐渐染上了某种性质。你说不清是什么，但它就在那里。",
        ];
        return Some(texts[0]);
    }
    // 色调转换（旧主色 ≠ Mellow 且变化到另一种主色）
    if before.main != ColorKind::Mellow && before.main != after.main {
        let texts: &[&str] = &[
            "旧日的沉淀在松动。你走向了另一条轨迹。",
            "有什么东西在你体内移位——不是破坏，是转向。",
        ];
        return Some(texts[0]);
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
            if let Some(text) = detect_color_milestone(&before, &color) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dominant_color_becomes_main() {
        let mut log = PracticeLog::default();
        log.add(ColorKind::Sharp, 70.0);
        log.add(ColorKind::Heavy, 30.0);
        let mut c = QiColor::default();
        evolve_qi_color(&log, &mut c);
        assert_eq!(c.main, ColorKind::Sharp);
        assert_eq!(c.secondary, Some(ColorKind::Heavy));
        assert!(!c.is_chaotic);
        assert!(!c.is_hunyuan);
    }

    #[test]
    fn three_over_15_percent_triggers_chaotic() {
        let mut log = PracticeLog::default();
        log.add(ColorKind::Sharp, 40.0);
        log.add(ColorKind::Heavy, 30.0);
        log.add(ColorKind::Mellow, 30.0);
        let mut c = QiColor::default();
        evolve_qi_color(&log, &mut c);
        assert!(c.is_chaotic);
    }

    #[test]
    fn uniform_under_25_triggers_hunyuan() {
        let mut log = PracticeLog::default();
        for k in [
            ColorKind::Sharp,
            ColorKind::Heavy,
            ColorKind::Mellow,
            ColorKind::Solid,
            ColorKind::Light,
        ] {
            log.add(k, 20.0);
        }
        let mut c = QiColor::default();
        evolve_qi_color(&log, &mut c);
        assert!(c.is_hunyuan);
        assert!(!c.is_chaotic);
    }

    #[test]
    fn is_hunyuan_requires_five_practiced_colors() {
        let mut log = PracticeLog::default();
        for k in [ColorKind::Sharp, ColorKind::Heavy, ColorKind::Mellow] {
            log.add(k, 20.0);
        }
        assert!(!is_hunyuan(&log));
    }

    #[test]
    fn is_hunyuan_rejects_dominant_color() {
        let mut log = PracticeLog::default();
        for (color, weight) in [
            (ColorKind::Sharp, 40.0),
            (ColorKind::Heavy, 15.0),
            (ColorKind::Mellow, 15.0),
            (ColorKind::Solid, 15.0),
            (ColorKind::Light, 15.0),
        ] {
            log.add(color, weight);
        }
        assert!(!is_hunyuan(&log));
    }

    #[test]
    fn seven_color_balance_is_hunyuan() {
        let mut log = PracticeLog::default();
        for k in [
            ColorKind::Sharp,
            ColorKind::Heavy,
            ColorKind::Mellow,
            ColorKind::Solid,
            ColorKind::Light,
            ColorKind::Intricate,
            ColorKind::Insidious,
        ] {
            log.add(k, 10.0);
        }
        assert!(is_hunyuan(&log));
        let mut c = QiColor::default();
        evolve_qi_color(&log, &mut c);
        assert!(c.is_hunyuan);
    }

    #[test]
    fn cultivation_session_practice_records_one_unit_per_minute() {
        let mut log = PracticeLog::default();
        // None qi_color → 1.0x bonus（无加成基准）
        let minutes = record_cultivation_session_practice(
            &mut log,
            ColorKind::Heavy,
            CULTIVATION_SESSION_PRACTICE_TICKS_PER_MINUTE * 60,
            None,
        );
        assert_eq!(minutes, 60, "期望 60 分钟，实际 {minutes}");
        assert_eq!(
            log.weights.get(&ColorKind::Heavy).copied(),
            Some(STYLE_PRACTICE_AMOUNT * 60.0),
            "期望 60 × STYLE_PRACTICE_AMOUNT（无加成），实际 {:?}",
            log.weights.get(&ColorKind::Heavy)
        );
    }

    #[test]
    fn cultivation_session_practice_ignores_sub_minute_noise() {
        let mut log = PracticeLog::default();
        let minutes = record_cultivation_session_practice(
            &mut log,
            ColorKind::Heavy,
            CULTIVATION_SESSION_PRACTICE_TICKS_PER_MINUTE - 1,
            None,
        );
        assert_eq!(minutes, 0, "期望 0 分钟（不足 1 分钟），实际 {minutes}");
        assert!(log.weights.is_empty(), "不足 1 分钟不应写入任何权重");
    }

    #[test]
    fn cultivation_session_practice_applies_main_color_bonus() {
        let mut log = PracticeLog::default();
        let qi_color = QiColor {
            main: ColorKind::Heavy,
            secondary: None,
            is_chaotic: false,
            is_hunyuan: false,
            permanent_lock_mask: Default::default(),
        };
        // 主色匹配 Heavy → 0.9x 倍率
        let minutes = record_cultivation_session_practice(
            &mut log,
            ColorKind::Heavy,
            CULTIVATION_SESSION_PRACTICE_TICKS_PER_MINUTE * 10,
            Some(&qi_color),
        );
        assert_eq!(minutes, 10, "期望 10 分钟，实际 {minutes}");
        let expected = STYLE_PRACTICE_AMOUNT * 10.0 * 0.9;
        let actual = log.weights.get(&ColorKind::Heavy).copied().unwrap_or(0.0);
        assert!(
            (actual - expected).abs() < 1e-9,
            "主色匹配打坐 10 分钟期望权重 {expected:.4}（×0.9），实际 {actual:.4}"
        );
    }

    #[test]
    fn cultivation_session_practice_applies_chaotic_penalty() {
        let mut log = PracticeLog::default();
        let qi_color = QiColor {
            main: ColorKind::Heavy,
            secondary: None,
            is_chaotic: true,
            is_hunyuan: false,
            permanent_lock_mask: Default::default(),
        };
        // 杂色 → 1.1x 倍率（惩罚）
        record_cultivation_session_practice(
            &mut log,
            ColorKind::Heavy,
            CULTIVATION_SESSION_PRACTICE_TICKS_PER_MINUTE * 5,
            Some(&qi_color),
        );
        let expected = STYLE_PRACTICE_AMOUNT * 5.0 * 1.1;
        let actual = log.weights.get(&ColorKind::Heavy).copied().unwrap_or(0.0);
        assert!(
            (actual - expected).abs() < 1e-9,
            "杂色打坐 5 分钟期望权重 {expected:.4}（×1.1），实际 {actual:.4}"
        );
    }

    // P2: 混元状态打坐应用 0.95x 代价（博而不精）
    #[test]
    fn cultivation_session_practice_applies_hunyuan_penalty() {
        let mut log = PracticeLog::default();
        let qi_color = QiColor {
            main: ColorKind::Sharp,
            secondary: None,
            is_chaotic: false,
            is_hunyuan: true,
            permanent_lock_mask: Default::default(),
        };
        // 混元 → 0.95x 倍率（博而不精，-5% 代价）
        record_cultivation_session_practice(
            &mut log,
            ColorKind::Sharp,
            CULTIVATION_SESSION_PRACTICE_TICKS_PER_MINUTE * 10,
            Some(&qi_color),
        );
        let expected = STYLE_PRACTICE_AMOUNT * 10.0 * 0.95;
        let actual = log.weights.get(&ColorKind::Sharp).copied().unwrap_or(0.0);
        assert!(
            (actual - expected).abs() < 1e-9,
            "混元打坐 10 分钟期望权重 {expected:.4}（×0.95 博而不精代价），实际 {actual:.4}"
        );
    }

    #[test]
    fn decay_drops_weights_to_zero() {
        let mut log = PracticeLog {
            decay_per_tick: 1.0,
            ..Default::default()
        };
        log.add(ColorKind::Sharp, 3.0);
        for _ in 0..5 {
            log.decay();
        }
        assert_eq!(log.total(), 0.0);
    }

    #[test]
    fn default_decay_matches_style_vector_plan() {
        let log = PracticeLog::default();
        assert_eq!(log.decay_per_tick, PRACTICE_DECAY_PER_TICK);
    }

    #[test]
    fn style_practice_uses_unified_amount_without_qi_color() {
        let mut log = PracticeLog::default();
        // None qi_color → 1.0x 基准，等于 STYLE_PRACTICE_AMOUNT
        record_style_practice(&mut log, ColorKind::Heavy, None);
        assert_eq!(
            log.weights.get(&ColorKind::Heavy).copied(),
            Some(STYLE_PRACTICE_AMOUNT),
            "期望 STYLE_PRACTICE_AMOUNT（无加成），实际 {:?}",
            log.weights.get(&ColorKind::Heavy)
        );
    }

    #[test]
    fn style_practice_applies_main_color_bonus() {
        let mut log = PracticeLog::default();
        let qi_color = QiColor {
            main: ColorKind::Heavy,
            secondary: None,
            is_chaotic: false,
            is_hunyuan: false,
            permanent_lock_mask: Default::default(),
        };
        record_style_practice(&mut log, ColorKind::Heavy, Some(&qi_color));
        let expected = STYLE_PRACTICE_AMOUNT * 0.9;
        let actual = log.weights.get(&ColorKind::Heavy).copied().unwrap_or(0.0);
        assert!(
            (actual - expected).abs() < 1e-9,
            "主色匹配 record_style_practice 期望权重 {expected:.4}（×0.9），实际 {actual:.4}"
        );
    }

    #[test]
    fn empty_log_leaves_color_untouched() {
        let log = PracticeLog::default();
        let mut c = QiColor {
            main: ColorKind::Violent,
            ..Default::default()
        };
        evolve_qi_color(&log, &mut c);
        assert_eq!(c.main, ColorKind::Violent);
    }

    #[test]
    fn evolution_tick_records_life_color_shift() {
        use valence::prelude::{App, Update};

        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 77 });
        app.add_systems(Update, qi_color_evolution_tick);
        let mut log = PracticeLog::default();
        log.add(ColorKind::Sharp, 70.0);
        log.add(ColorKind::Heavy, 30.0);
        let entity = app
            .world_mut()
            .spawn((
                log,
                QiColor::default(),
                LifeRecord::new("offline:ColorShift".to_string()),
            ))
            .id();

        app.update();

        let life = app.world().get::<LifeRecord>(entity).unwrap();
        assert!(matches!(
            life.biography.last(),
            Some(BiographyEntry::ColorShift {
                main: ColorKind::Sharp,
                secondary: Some(ColorKind::Heavy),
                tick: 77,
            })
        ));
    }

    // ──────────────────────────────────────────────────────────────────────────
    // P3: detect_color_milestone 纯函数测试（4 里程碑 + 边界 + 优先级）
    // ──────────────────────────────────────────────────────────────────────────

    fn mellow_qi_color() -> QiColor {
        QiColor::default() // main = Mellow, secondary = None, is_chaotic = false, is_hunyuan = false
    }

    fn sharp_qi_color() -> QiColor {
        QiColor {
            main: ColorKind::Sharp,
            secondary: None,
            is_chaotic: false,
            is_hunyuan: false,
            permanent_lock_mask: Default::default(),
        }
    }

    #[test]
    fn milestone_first_color_emergence_mellow_to_sharp() {
        // 首次主色涌现：before.main == Mellow → after.main == Sharp
        let before = mellow_qi_color();
        let after = sharp_qi_color();
        let result = detect_color_milestone(&before, &after);
        assert!(
            result.is_some(),
            "首次主色涌现（Mellow→Sharp）应触发里程碑 narration，因为 before.main==Mellow 而 after.main!=Mellow；实际返回 None"
        );
        let text = result.unwrap();
        assert!(
            !text.is_empty(),
            "里程碑文本不应为空字符串，因为该触发器有预设模板；实际为空"
        );
    }

    #[test]
    fn milestone_color_shift_sharp_to_heavy() {
        // 色调转换：before.main == Sharp（非 Mellow），after.main == Heavy
        let before = sharp_qi_color();
        let mut after = sharp_qi_color();
        after.main = ColorKind::Heavy;
        let result = detect_color_milestone(&before, &after);
        assert!(
            result.is_some(),
            "色调转换（Sharp→Heavy）应触发里程碑 narration，因为旧色非 Mellow 且主色发生变化；实际返回 None"
        );
    }

    #[test]
    fn milestone_chaotic_corruption_false_to_true() {
        // 杂色堕落：!before.is_chaotic && after.is_chaotic
        let before = sharp_qi_color();
        let mut after = sharp_qi_color();
        after.is_chaotic = true;
        let result = detect_color_milestone(&before, &after);
        assert!(
            result.is_some(),
            "杂色堕落（is_chaotic: false→true）应触发里程碑 narration；实际返回 None"
        );
        let text = result.unwrap();
        assert!(
            text.contains("杂") || text.contains("乱炖") || text.contains("五色"),
            "杂色堕落模板应包含杂色相关关键词；实际文本：「{text}」"
        );
    }

    #[test]
    fn milestone_hunyuan_awakening_false_to_true() {
        // 混元觉醒：!before.is_hunyuan && after.is_hunyuan
        let before = mellow_qi_color();
        let mut after = mellow_qi_color();
        after.is_hunyuan = true;
        let result = detect_color_milestone(&before, &after);
        assert!(
            result.is_some(),
            "混元觉醒（is_hunyuan: false→true）应触发里程碑 narration；实际返回 None"
        );
        let text = result.unwrap();
        assert!(
            text.contains("均衡") || text.contains("交汇") || text.contains("五色"),
            "混元觉醒模板应包含混元相关关键词；实际文本：「{text}」"
        );
    }

    #[test]
    fn milestone_no_change_returns_none() {
        // 无变化（Mellow→Mellow）→ None
        let color = mellow_qi_color();
        let result = detect_color_milestone(&color, &color.clone());
        assert!(
            result.is_none(),
            "相同色调（Mellow→Mellow）不应触发任何里程碑，因为没有状态转换发生；实际返回 Some({:?})",
            result
        );
    }

    #[test]
    fn milestone_same_non_mellow_color_returns_none() {
        // 非 Mellow 保持相同主色，无其他变化 → None
        let color = sharp_qi_color();
        let result = detect_color_milestone(&color, &color.clone());
        assert!(
            result.is_none(),
            "主色保持 Sharp 不变（非 Mellow）时不应触发里程碑；实际返回 Some({:?})",
            result
        );
    }

    #[test]
    fn milestone_chaotic_already_true_no_retrigger() {
        // 杂色已经是 true → false 不触发（只触发 false→true）
        let mut before = sharp_qi_color();
        before.is_chaotic = true;
        let mut after = sharp_qi_color();
        after.is_chaotic = false; // 杂色清除不触发任何里程碑
        let result = detect_color_milestone(&before, &after);
        assert!(
            result.is_none(),
            "杂色清除（is_chaotic: true→false）不应触发里程碑；实际返回 Some({:?})",
            result
        );
    }

    #[test]
    fn milestone_chaotic_takes_priority_over_hunyuan() {
        // 同 tick 杂色和混元同时转换时，杂色优先（plan §P3 优先级：chaotic > hunyuan）
        let before = mellow_qi_color();
        let mut after = mellow_qi_color();
        after.is_chaotic = true;
        after.is_hunyuan = true;
        let result = detect_color_milestone(&before, &after);
        assert!(
            result.is_some(),
            "杂色+混元同时触发时应返回 Some（杂色优先）；实际返回 None"
        );
        let text = result.unwrap();
        assert!(
            text.contains("杂") || text.contains("乱炖") || text.contains("五色"),
            "杂色优先于混元——模板应为杂色文本（含「杂」/「乱炖」/「五色」）；实际文本：「{text}」"
        );
    }

    #[test]
    fn milestone_chaotic_takes_priority_over_first_emergence() {
        // 同 tick 首次主色涌现与杂色同时触发，杂色优先
        let before = mellow_qi_color(); // before.main == Mellow
        let mut after = sharp_qi_color(); // after.main != Mellow → 首次主色涌现
        after.is_chaotic = true; // 同时杂色堕落
        let result = detect_color_milestone(&before, &after);
        let text = result.expect("杂色+首次涌现同时触发时应返回 Some（杂色优先）");
        assert!(
            text.contains("杂") || text.contains("乱炖") || text.contains("五色"),
            "杂色优先于首次涌现——模板应为杂色文本；实际文本：「{text}」"
        );
    }

    // ──────────────────────────────────────────────────────────────────────────
    // P3: 集成测试 — qi_color_evolution_tick 在里程碑触发时向 PendingGameplayNarrations 写入
    // ──────────────────────────────────────────────────────────────────────────

    #[test]
    fn evolution_tick_emits_narration_on_first_color_emergence() {
        use crate::player::gameplay::PendingGameplayNarrations;
        use crate::schema::common::NarrationScope;
        use valence::prelude::{App, Update};

        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 1 });
        app.insert_resource(PendingGameplayNarrations::default());
        app.add_systems(Update, qi_color_evolution_tick);

        // 构造一个 Sharp 主导的练习日志（Mellow → Sharp 首次涌现）
        let mut log = PracticeLog::default();
        log.add(ColorKind::Sharp, 70.0);
        log.add(ColorKind::Heavy, 29.9); // 保持 Sharp > 60%，Heavy < 25%，不触发杂色

        // 玩家初始为 Mellow（default），加上 Username 以便 narration 路由
        app.world_mut().spawn((
            log,
            QiColor::default(), // main = Mellow
            Username("AzureTest".to_string()),
        ));

        app.update();

        let mut narrations = app.world_mut().resource_mut::<PendingGameplayNarrations>();
        let drained = narrations.drain();
        assert!(
            !drained.is_empty(),
            "首次主色涌现（Mellow→Sharp）应触发 narration 入队 PendingGameplayNarrations；实际队列为空"
        );
        let n = &drained[0];
        assert!(
            matches!(n.scope, NarrationScope::Player),
            "里程碑 narration scope 应为 Player（只通知本玩家）；实际 scope={:?}",
            n.scope
        );
        assert_eq!(
            n.target.as_deref(),
            Some("AzureTest"),
            "narration target 应为玩家 Username「AzureTest」；实际 target={:?}",
            n.target
        );
        assert!(
            matches!(n.style, NarrationStyle::Perception),
            "里程碑 narration style 应为 Perception；实际 style={:?}",
            n.style
        );
        assert!(
            !n.text.is_empty(),
            "里程碑 narration 文本不应为空；实际文本为空"
        );
    }

    #[test]
    fn evolution_tick_emits_narration_on_chaotic_corruption() {
        use crate::player::gameplay::PendingGameplayNarrations;
        use valence::prelude::{App, Update};

        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 2 });
        app.insert_resource(PendingGameplayNarrations::default());
        app.add_systems(Update, qi_color_evolution_tick);

        // 三色均衡 > 15% 触发杂色
        let mut log = PracticeLog::default();
        log.add(ColorKind::Sharp, 40.0);
        log.add(ColorKind::Heavy, 30.0);
        log.add(ColorKind::Mellow, 30.0);

        // 初始为 Sharp 主色（非 Mellow，非杂色），使得杂色是新变化
        let before_color = QiColor {
            main: ColorKind::Sharp,
            secondary: None,
            is_chaotic: false,
            is_hunyuan: false,
            permanent_lock_mask: Default::default(),
        };

        app.world_mut()
            .spawn((log, before_color, Username("ZhaoPlayer".to_string())));

        app.update();

        let mut narrations = app.world_mut().resource_mut::<PendingGameplayNarrations>();
        let drained = narrations.drain();
        assert!(
            !drained.is_empty(),
            "杂色堕落应触发 narration 入队；三色均衡（各占 33%，均 > 15%）时 is_chaotic 应从 false→true；实际队列为空"
        );
        let text = &drained[0].text;
        assert!(
            text.contains("杂") || text.contains("乱炖") || text.contains("五色"),
            "杂色堕落 narration 应包含杂色相关关键词；实际文本：「{text}」"
        );
    }

    #[test]
    fn evolution_tick_emits_narration_on_hunyuan_awakening() {
        use crate::player::gameplay::PendingGameplayNarrations;
        use valence::prelude::{App, Update};

        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 3 });
        app.insert_resource(PendingGameplayNarrations::default());
        app.add_systems(Update, qi_color_evolution_tick);

        // 五色均衡 < 25% 触发混元
        let mut log = PracticeLog::default();
        for k in [
            ColorKind::Sharp,
            ColorKind::Heavy,
            ColorKind::Mellow,
            ColorKind::Solid,
            ColorKind::Light,
        ] {
            log.add(k, 20.0);
        }

        // 初始为非混元状态（普通主色 Sharp）
        let before_color = QiColor {
            main: ColorKind::Sharp,
            secondary: None,
            is_chaotic: false,
            is_hunyuan: false,
            permanent_lock_mask: Default::default(),
        };

        app.world_mut()
            .spawn((log, before_color, Username("LiPlayer".to_string())));

        app.update();

        let mut narrations = app.world_mut().resource_mut::<PendingGameplayNarrations>();
        let drained = narrations.drain();
        assert!(
            !drained.is_empty(),
            "混元觉醒应触发 narration 入队；五色均衡（各 20%，< 25%）时 is_hunyuan 应从 false→true；实际队列为空"
        );
        let text = &drained[0].text;
        assert!(
            text.contains("均衡") || text.contains("交汇") || text.contains("五色"),
            "混元觉醒 narration 应包含混元相关关键词；实际文本：「{text}」"
        );
    }

    #[test]
    fn evolution_tick_emits_narration_on_color_shift() {
        use crate::player::gameplay::PendingGameplayNarrations;
        use valence::prelude::{App, Update};

        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 4 });
        app.insert_resource(PendingGameplayNarrations::default());
        app.add_systems(Update, qi_color_evolution_tick);

        // Heavy 主导日志（> 60%）
        let mut log = PracticeLog::default();
        log.add(ColorKind::Heavy, 70.0);
        log.add(ColorKind::Sharp, 29.9);

        // 初始主色为 Sharp（非 Mellow），主色会从 Sharp → Heavy，触发色调转换
        let before_color = QiColor {
            main: ColorKind::Sharp,
            secondary: None,
            is_chaotic: false,
            is_hunyuan: false,
            permanent_lock_mask: Default::default(),
        };

        app.world_mut()
            .spawn((log, before_color, Username("WangPlayer".to_string())));

        app.update();

        let mut narrations = app.world_mut().resource_mut::<PendingGameplayNarrations>();
        let drained = narrations.drain();
        assert!(
            !drained.is_empty(),
            "色调转换（Sharp→Heavy）应触发 narration 入队；Heavy > 60% 导致主色变更；实际队列为空"
        );
        let text = &drained[0].text;
        assert!(
            text.contains("移位")
                || text.contains("轨迹")
                || text.contains("松动")
                || text.contains("转向"),
            "色调转换 narration 应包含转换相关关键词；实际文本：「{text}」"
        );
    }

    #[test]
    fn evolution_tick_no_narration_without_username() {
        use crate::player::gameplay::PendingGameplayNarrations;
        use valence::prelude::{App, Update};

        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 5 });
        app.insert_resource(PendingGameplayNarrations::default());
        app.add_systems(Update, qi_color_evolution_tick);

        // Sharp 主导日志会触发里程碑，但不带 Username
        let mut log = PracticeLog::default();
        log.add(ColorKind::Sharp, 70.0);
        log.add(ColorKind::Heavy, 29.9);

        app.world_mut().spawn((
            log,
            QiColor::default(), // main = Mellow，里程碑会触发
                                // 无 Username component
        ));

        app.update();

        let mut narrations = app.world_mut().resource_mut::<PendingGameplayNarrations>();
        let drained = narrations.drain();
        assert!(
            drained.is_empty(),
            "无 Username component 时不应发送 narration（无法路由给玩家）；实际入队了 {} 条",
            drained.len()
        );
    }

    #[test]
    fn evolution_tick_no_narration_without_color_change() {
        use crate::player::gameplay::PendingGameplayNarrations;
        use valence::prelude::{App, Update};

        let mut app = App::new();
        app.insert_resource(CultivationClock { tick: 6 });
        app.insert_resource(PendingGameplayNarrations::default());
        app.add_systems(Update, qi_color_evolution_tick);

        // 空日志 → evolve_qi_color 不做任何改变（total=0），无里程碑
        let log = PracticeLog::default();
        let color = QiColor {
            main: ColorKind::Sharp,
            ..Default::default()
        };

        app.world_mut()
            .spawn((log, color, Username("NoChange".to_string())));

        app.update();

        let mut narrations = app.world_mut().resource_mut::<PendingGameplayNarrations>();
        let drained = narrations.drain();
        assert!(
            drained.is_empty(),
            "日志为空（无信号）时不应发送任何 narration，因为 evolve_qi_color 在 total=0 时不改变色调；实际入队了 {} 条",
            drained.len()
        );
    }
}
