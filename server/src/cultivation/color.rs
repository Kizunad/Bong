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
use valence::prelude::{bevy_ecs, Component, Entity, Event, EventReader, Query, Res};

use super::color_bonus::color_style_bonus;
use super::components::{ColorKind, QiColor};
use super::life_record::{BiographyEntry, LifeRecord};
use super::tick::CultivationClock;

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

pub fn qi_color_evolution_tick(
    clock: Res<CultivationClock>,
    mut players: Query<(&mut PracticeLog, &mut QiColor, Option<&mut LifeRecord>)>,
) {
    for (mut log, mut color, life_record) in players.iter_mut() {
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
}
