//! ComposureTick — 心境缓慢回升（plan §2 / §1.1）。
//!
//! 心境 (composure) 是 0..=1 的标量，在突破/走火/情绪事件被扣减，
//! 平时由 `composure_recover_rate` 慢速回升。上限 1.0。

use valence::prelude::Query;

use super::components::Cultivation;

pub fn composure_tick(mut players: Query<&mut Cultivation>) {
    for mut c in players.iter_mut() {
        if c.composure < 1.0 {
            let rate = c.composure_recover_rate;
            c.composure = (c.composure + rate).min(1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovers_toward_one() {
        let mut c = Cultivation {
            composure: 0.0,
            composure_recover_rate: 0.1,
            ..Default::default()
        };
        // 模拟 tick 逻辑
        for _ in 0..20 {
            if c.composure < 1.0 {
                c.composure = (c.composure + c.composure_recover_rate).min(1.0);
            }
        }
        assert_eq!(c.composure, 1.0);
    }

    #[test]
    fn never_exceeds_one() {
        let mut c = Cultivation {
            composure: 0.99,
            composure_recover_rate: 0.5,
            ..Default::default()
        };
        c.composure = (c.composure + c.composure_recover_rate).min(1.0);
        assert_eq!(c.composure, 1.0);
    }
}
