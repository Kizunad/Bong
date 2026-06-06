use super::constants::QI_DECAY_PER_BLOCK;
use super::env::{EnvField, MediumKind};

pub fn qi_distance_atten(initial: f64, distance_blocks: f64, medium: MediumKind) -> f64 {
    if !initial.is_finite() || initial <= 0.0 {
        return 0.0;
    }
    if !distance_blocks.is_finite() || distance_blocks <= 0.0 {
        return initial;
    }

    let loss_per_block = (QI_DECAY_PER_BLOCK + medium.loss_bonus_per_block()).clamp(0.0, 0.95);
    initial * (1.0 - loss_per_block).powf(distance_blocks)
}

pub fn qi_distance_atten_in_env(
    initial: f64,
    distance_blocks: f64,
    medium: MediumKind,
    env: &EnvField,
) -> f64 {
    qi_distance_atten(
        initial,
        distance_blocks * env.law_disruption_distance_multiplier(),
        medium,
    )
}

#[cfg(test)]
mod tests {
    use crate::cultivation::components::ColorKind;

    use super::*;
    use crate::qi_physics::env::{CarrierGrade, MediumKind};

    #[test]
    fn zero_distance_keeps_initial_qi() {
        assert_eq!(qi_distance_atten(10.0, 0.0, MediumKind::default()), 10.0);
    }

    #[test]
    fn negative_initial_returns_zero() {
        assert_eq!(qi_distance_atten(-1.0, 4.0, MediumKind::default()), 0.0);
    }

    #[test]
    fn finite_distance_loses_qi() {
        let out = qi_distance_atten(10.0, 3.0, MediumKind::default());
        assert!(out > 0.0 && out < 10.0);
    }

    #[test]
    fn far_distance_approaches_zero() {
        let out = qi_distance_atten(10.0, 1_000.0, MediumKind::default());
        assert!(out < 0.001);
    }

    #[test]
    fn ancient_relic_carries_farther_than_bare_qi() {
        let bare = qi_distance_atten(10.0, 20.0, MediumKind::bare(ColorKind::Mellow));
        let relic = qi_distance_atten(
            10.0,
            20.0,
            MediumKind {
                color: ColorKind::Mellow,
                carrier: CarrierGrade::AncientRelic,
            },
        );
        assert!(relic > bare);
    }

    #[test]
    fn violent_color_loses_more_than_solid_color() {
        let violent = qi_distance_atten(10.0, 10.0, MediumKind::bare(ColorKind::Violent));
        let solid = qi_distance_atten(10.0, 10.0, MediumKind::bare(ColorKind::Solid));
        assert!(violent < solid);
    }

    /// P0 color plan §table.行60 — 凝实(Solid)色暗器距离衰减减免由 qi_physics 传输层
    /// 实现：MediumKind::loss_bonus_per_block(Solid) = -0.004（每格减少衰减），
    /// 相同距离 Solid 到达真元严格多于其他非减免色（如 Heavy +0.004）。
    /// §六.2 硬约束：此减免在传输层计算，不进战斗公式/不影响 wound 乘子。
    #[test]
    fn solid_color_anqi_distance_atten_bonus_canonical_path() {
        let distance = 15.0_f64;
        let initial = 100.0_f64;
        // Solid: loss_bonus_per_block = -0.004（减免，到达更多真元）
        let solid_arrived =
            qi_distance_atten(initial, distance, MediumKind::bare(ColorKind::Solid));
        // Heavy: loss_bonus_per_block = +0.004（加损，到达更少真元）
        let heavy_arrived =
            qi_distance_atten(initial, distance, MediumKind::bare(ColorKind::Heavy));
        // Mellow(default): loss_bonus_per_block = 0.0（无加减）
        let mellow_arrived = qi_distance_atten(initial, distance, MediumKind::default());

        assert!(
            solid_arrived > mellow_arrived,
            "P0 凝实色暗器距离衰减减免：期望 Solid({solid_arrived:.6}) > Mellow({mellow_arrived:.6})，\
             Solid loss_bonus=-0.004/block 应使更多真元到达目标"
        );
        assert!(
            solid_arrived > heavy_arrived,
            "P0 凝实色暗器距离衰减减免：期望 Solid({solid_arrived:.6}) > Heavy({heavy_arrived:.6})，\
             Solid(-0.004) vs Heavy(+0.004) 同距离差异应明显"
        );
        // 验证 loss_bonus_per_block 的数值正确：Solid=-0.004, Heavy=+0.004，两者差值由衰减公式决定
        assert!(
            solid_arrived > heavy_arrived * 1.001,
            "P0 凝实色 vs Heavy 色到达真元差值应可测量（{solid_arrived:.6} vs {heavy_arrived:.6}），\
             差值={:.6}",
            solid_arrived - heavy_arrived
        );
    }

    #[test]
    fn law_disruption_offsets_effective_hit_distance() {
        let calm =
            qi_distance_atten_in_env(10.0, 10.0, MediumKind::default(), &EnvField::default());
        let disrupted = qi_distance_atten_in_env(
            10.0,
            10.0,
            MediumKind::default(),
            &EnvField::default().with_law_disruption(1.0),
        );
        assert!(disrupted < calm);
    }
}
