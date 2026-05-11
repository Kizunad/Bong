//! plan-qixiu-depth-v1 P3 — 法器染色。
//!
//! 法器染色复用 cultivation::color::PracticeLog / evolve_qi_color 的权重模型，
//! 但显式禁用"混元"：法器没有意识，只会被长期流过的真元印染。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component};

use crate::cultivation::color::{evolve_qi_color, PracticeLog, PRACTICE_DECAY_PER_TICK};
use crate::cultivation::components::{ColorKind, QiColor};

/// 法器内部沉积的真元色。`practice_log.total()==0` 表示未染色。
#[derive(Debug, Clone, Serialize, Deserialize, Component)]
pub struct ArtifactColor {
    pub practice_log: PracticeLog,
    pub main: ColorKind,
    pub secondary: Option<ColorKind>,
    pub is_chaotic: bool,
}

impl Default for ArtifactColor {
    fn default() -> Self {
        Self::uncolored()
    }
}

impl ArtifactColor {
    pub fn uncolored() -> Self {
        Self {
            practice_log: artifact_practice_log(),
            main: ColorKind::Mellow,
            secondary: None,
            is_chaotic: false,
        }
    }

    pub fn from_initial_color(color: ColorKind, amount: f64) -> Self {
        let mut artifact = Self::uncolored();
        artifact.practice_log.add(color, amount.max(0.0));
        artifact.evolve();
        artifact
    }

    pub fn is_uncolored(&self) -> bool {
        self.practice_log.total() <= f64::EPSILON
    }

    pub fn record_use(&mut self, user_color: ColorKind, flow_amount: f64) {
        let amount = (flow_amount * 0.1).max(0.0);
        if amount > 0.0 {
            self.practice_log.add(user_color, amount);
            self.evolve();
        }
    }

    pub fn decay_tick(&mut self) {
        self.practice_log.decay();
        self.evolve();
    }

    pub fn evolve(&mut self) {
        if self.practice_log.total() <= f64::EPSILON {
            self.main = ColorKind::Mellow;
            self.secondary = None;
            self.is_chaotic = false;
            return;
        }

        let mut qi_color = QiColor {
            main: self.main,
            secondary: self.secondary,
            is_chaotic: self.is_chaotic,
            is_hunyuan: false,
            permanent_lock_mask: Default::default(),
        };
        evolve_qi_color(&self.practice_log, &mut qi_color);

        // 法器无意识，不能进入混元；若统一演化函数判成混元，则保留当前主色并清空副色。
        if qi_color.is_hunyuan {
            qi_color.is_chaotic = false;
            qi_color.secondary = None;
        }
        self.main = qi_color.main;
        self.secondary = qi_color.secondary;
        self.is_chaotic = qi_color.is_chaotic;
    }
}

pub fn artifact_practice_log() -> PracticeLog {
    PracticeLog {
        weights: Default::default(),
        decay_per_tick: PRACTICE_DECAY_PER_TICK / 10.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_accumulates_on_use() {
        let mut color = ArtifactColor::uncolored();
        color.record_use(ColorKind::Solid, 20.0);

        assert_eq!(color.main, ColorKind::Solid);
        assert!(color.practice_log.total() > 0.0);
    }

    #[test]
    fn decay_rate_one_tenth() {
        let color = ArtifactColor::uncolored();

        assert!((color.practice_log.decay_per_tick - PRACTICE_DECAY_PER_TICK / 10.0).abs() < 1e-12);
    }

    #[test]
    fn artifact_color_never_becomes_hunyuan() {
        let mut color = ArtifactColor::uncolored();
        for c in [
            ColorKind::Sharp,
            ColorKind::Heavy,
            ColorKind::Mellow,
            ColorKind::Solid,
            ColorKind::Light,
        ] {
            color.record_use(c, 10.0);
        }

        assert!(!color.is_uncolored());
        assert!(!color.is_chaotic);
        assert_eq!(color.secondary, None);
    }

    #[test]
    fn changing_owner_shifts_weights_gradually() {
        let mut color = ArtifactColor::from_initial_color(ColorKind::Sharp, 100.0);
        color.record_use(ColorKind::Heavy, 1_600.0);

        assert_eq!(color.main, ColorKind::Heavy);
        assert_eq!(color.secondary, Some(ColorKind::Sharp));
    }

    #[test]
    fn ten_color_matrix_can_select_each_dominant_color() {
        let colors = [
            ColorKind::Sharp,
            ColorKind::Heavy,
            ColorKind::Mellow,
            ColorKind::Solid,
            ColorKind::Light,
            ColorKind::Intricate,
            ColorKind::Gentle,
            ColorKind::Insidious,
            ColorKind::Violent,
            ColorKind::Turbid,
        ];

        for color in colors {
            let artifact = ArtifactColor::from_initial_color(color, 80.0);
            assert_eq!(artifact.main, color);
            assert!(!artifact.is_chaotic);
        }
    }
}
