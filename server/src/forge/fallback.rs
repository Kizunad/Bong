//! plan-forge-v1 §1.3 残缺匹配 + side_effect_pool。
//!
//! 走 flawed_fallback 时，按 weight 抽取副作用标签；记录到 LifeRecord。

use super::blueprint::{FlawedFallback, SideEffectEntry};

/// 简易 weighted pick：给定 `roll ∈ [0, total_weight)` 返回命中的 entry。
/// 方便测试可注入确定性 seed。
pub fn weighted_pick(pool: &[SideEffectEntry], roll: u32) -> Option<&SideEffectEntry> {
    if pool.is_empty() {
        return None;
    }
    let total: u32 = pool.iter().map(|e| e.weight).sum();
    if total == 0 {
        return None;
    }
    let r = roll % total;
    let mut acc = 0u32;
    for e in pool {
        acc += e.weight;
        if r < acc {
            return Some(e);
        }
    }
    pool.last()
}

/// 计算 flawed 成品品质。
pub fn flawed_quality(fallback: &FlawedFallback, base_quality: f32) -> f32 {
    (base_quality * fallback.quality_scale).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool() -> Vec<SideEffectEntry> {
        vec![
            SideEffectEntry {
                tag: "a".into(),
                weight: 1,
                color: None,
                perm: false,
            },
            SideEffectEntry {
                tag: "b".into(),
                weight: 3,
                color: None,
                perm: false,
            },
            SideEffectEntry {
                tag: "c".into(),
                weight: 1,
                color: None,
                perm: false,
            },
        ]
    }

    #[test]
    fn weighted_pick_hits_deterministic() {
        let p = pool();
        // total=5. roll=0 → a (0..1), roll=2 → b (1..4), roll=4 → c (4..5)
        assert_eq!(weighted_pick(&p, 0).unwrap().tag, "a");
        assert_eq!(weighted_pick(&p, 2).unwrap().tag, "b");
        assert_eq!(weighted_pick(&p, 4).unwrap().tag, "c");
        // roll beyond total wraps.
        assert_eq!(weighted_pick(&p, 7).unwrap().tag, "b"); // 7 % 5 = 2
    }

    #[test]
    fn weighted_pick_empty_pool() {
        assert!(weighted_pick(&[], 0).is_none());
    }

    #[test]
    fn flawed_quality_scales() {
        let fb = crate::forge::blueprint::FlawedFallback {
            weapon: "x".into(),
            quality_scale: 0.5,
            side_effect_pool: vec![],
        };
        assert!((flawed_quality(&fb, 1.0) - 0.5).abs() < 1e-6);
        // clamp upper
        let fb2 = crate::forge::blueprint::FlawedFallback {
            weapon: "x".into(),
            quality_scale: 2.0,
            side_effect_pool: vec![],
        };
        assert_eq!(flawed_quality(&fb2, 1.0), 1.0);
    }
}
