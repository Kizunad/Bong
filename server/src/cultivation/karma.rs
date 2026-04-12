//! 业力（plan §1.1, §11-5）。
//!
//! `Karma.weight` 每游戏日极慢衰减。来源通过 `KarmaSource` 记录到生平卷。
//! 本 plan 不做"善恶"判定（公理 §0-5 天道冷漠），只提供数值与衰减。

use serde::{Deserialize, Serialize};
use valence::prelude::{Query, Res};

use super::components::Karma;
use super::tick::CultivationClock;

/// 每 tick 的默认衰减量 — 约对应 1 游戏日降 1.0 单位（假设 20 TPS，每日 24 * 60 * 60 * 20 tick）。
pub const DEFAULT_DECAY_PER_TICK: f64 = 1.0 / (24.0 * 60.0 * 60.0 * 20.0);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KarmaSource {
    pub reason: String,
    pub weight: f64,
    pub tick: u64,
}

pub fn karma_decay_tick(_clock: Res<CultivationClock>, mut players: Query<&mut Karma>) {
    for mut k in players.iter_mut() {
        if k.weight > 0.0 {
            k.weight = (k.weight - DEFAULT_DECAY_PER_TICK).max(0.0);
        } else if k.weight < 0.0 {
            k.weight = (k.weight + DEFAULT_DECAY_PER_TICK).min(0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decay_moves_toward_zero() {
        let mut k = Karma { weight: 0.01 };
        for _ in 0..1_000_000 {
            k.weight = if k.weight > 0.0 {
                (k.weight - DEFAULT_DECAY_PER_TICK).max(0.0)
            } else {
                (k.weight + DEFAULT_DECAY_PER_TICK).min(0.0)
            };
            if k.weight == 0.0 {
                break;
            }
        }
        assert_eq!(k.weight, 0.0);
    }
}
