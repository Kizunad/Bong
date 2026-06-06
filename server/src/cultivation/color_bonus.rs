//! 真元色练习效率加成（plan-color-v1 §1 / §6 收口决议）。
//!
//! `color_style_bonus(qi_color, active_color)` 是纯函数，返回练习权重倍率（rate multiplier）：
//! **倍率 > 1.0 表示积累更快（加成）；< 1.0 表示积累更慢（惩罚）**。
//! 乘进 `PracticeLog.add(color, amount * bonus)` — higher = 更多权重积累 = 主色演化更快。
//!
//! | 状态 | 说明 | 倍率 |
//! |------|------|------|
//! | main == active_color | 主色匹配 — 事半功倍（worldview §六.1）| 1.1（积累+10%，更快） |
//! | secondary == Some(active_color) | 次色匹配 | 1.05 |
//! | 其他 | 非专色 | 1.0 |
//! | is_chaotic | 杂色 — 专精失效（worldview §六.2）| 0.9（积累-10%，惩罚） |
//! | is_hunyuan | 混元 — 博而不精（worldview §六.2:644「修炼总效率永久 -20%」）| 0.8 |
//!
//! 注：1.1 > 1.05 > 1.0 > 0.9 > 0.8 确保专精主色仍最优（main 1.1 > hunyuan 0.8），
//! 混元比杂色更差（0.8 < 0.9），对应 worldview §六.2 的「博而不精」定性。
//! is_chaotic 优先于 is_hunyuan（两者同为 true 时取 chaotic 0.9）。
//!
//! **硬约束（worldview §六.2）**：
//! 该倍率仅作用于 `PracticeLog.add(color, amount)` 的 amount 参数，
//! 不进任何战斗伤害/消耗公式，不改 `qi_current`，严禁复用到战斗路径。

use crate::cultivation::components::{ColorKind, QiColor};

/// 返回练习权重倍率（> 1.0 表示积累更快/加成，< 1.0 表示积累更慢/惩罚）。
///
/// **is_chaotic 优先于 is_hunyuan**（两者同为 true 时取 chaotic 0.9x）。
///
/// 不进任何战斗公式 — 仅用于 `PracticeLog.add(color, amount * bonus)`.
/// 返回 f64 以避免 f32→f64 精度损失影响权重累积。
pub fn color_style_bonus(qi_color: &QiColor, active_color: ColorKind) -> f64 {
    // is_chaotic 优先：专精失效，积累减慢（worldview §六.2）
    if qi_color.is_chaotic {
        return 0.9;
    }
    // 混元：博而不精，修炼总效率永久 -20%（worldview §六.2:644）
    if qi_color.is_hunyuan {
        return 0.8;
    }
    // 主色匹配：事半功倍（worldview §六.1），积累加速
    if qi_color.main == active_color {
        return 1.1;
    }
    // 次色匹配：小幅加速
    if qi_color.secondary == Some(active_color) {
        return 1.05;
    }
    1.0
}

#[cfg(test)]
mod tests {
    use crate::cultivation::components::QiColor;

    use super::*;

    fn make_color(
        main: ColorKind,
        secondary: Option<ColorKind>,
        is_chaotic: bool,
        is_hunyuan: bool,
    ) -> QiColor {
        QiColor {
            main,
            secondary,
            is_chaotic,
            is_hunyuan,
            permanent_lock_mask: Default::default(),
        }
    }

    // ① 主色匹配 → 1.1（事半功倍，积累+10%，worldview §六.1）
    #[test]
    fn main_color_match_returns_1_1() {
        let qi_color = make_color(ColorKind::Sharp, None, false, false);
        let bonus = color_style_bonus(&qi_color, ColorKind::Sharp);
        assert_eq!(
            bonus, 1.1,
            "主色匹配应返回 1.1（事半功倍，积累+10%），实际得到 {bonus}"
        );
    }

    // ② 次色匹配 → 1.05
    #[test]
    fn secondary_color_match_returns_1_05() {
        let qi_color = make_color(ColorKind::Heavy, Some(ColorKind::Solid), false, false);
        let bonus = color_style_bonus(&qi_color, ColorKind::Solid);
        assert_eq!(
            bonus, 1.05,
            "次色匹配应返回 1.05（小幅加速），实际得到 {bonus}"
        );
    }

    // ③ 杂色 → 0.9（积累-10% 惩罚，worldview §六.2「专精失效」）
    #[test]
    fn chaotic_returns_0_9() {
        let qi_color = make_color(ColorKind::Sharp, None, true, false);
        let bonus = color_style_bonus(&qi_color, ColorKind::Sharp);
        assert_eq!(
            bonus, 0.9,
            "杂色应返回 0.9（专精失效积累-10% 惩罚），即使与主色相同，实际得到 {bonus}"
        );
    }

    // ④ 混元 → 0.8（博而不精 -20% 惩罚，worldview §六.2:644「修炼总效率永久 -20%」）
    #[test]
    fn hunyuan_returns_0_8() {
        let qi_color = make_color(ColorKind::Mellow, None, false, true);
        let bonus = color_style_bonus(&qi_color, ColorKind::Sharp);
        assert_eq!(
            bonus, 0.8,
            "混元应返回 0.8（博而不精 -20% 修炼总效率代价，worldview §六.2:644），实际得到 {bonus}"
        );
    }

    // ⑤ 不匹配（既非主色也非次色，非杂色非混元）→ 1.0
    #[test]
    fn no_match_returns_1_0() {
        let qi_color = make_color(ColorKind::Sharp, Some(ColorKind::Heavy), false, false);
        let bonus = color_style_bonus(&qi_color, ColorKind::Mellow);
        assert_eq!(bonus, 1.0, "非专色不匹配应返回 1.0，实际得到 {bonus}");
    }

    // ⑥ 边界：is_chaotic 同时 is_hunyuan=true → chaotic 优先（返回 0.9）
    #[test]
    fn chaotic_takes_priority_over_hunyuan() {
        let qi_color = make_color(ColorKind::Sharp, None, true, true);
        let bonus = color_style_bonus(&qi_color, ColorKind::Sharp);
        assert_eq!(
            bonus, 0.9,
            "is_chaotic 与 is_hunyuan 同为 true 时 chaotic 优先返回 0.9，实际得到 {bonus}"
        );
    }

    // ⑦ secondary=None，active_color 与 main 不同 → 1.0（secondary None 不匹配）
    #[test]
    fn no_secondary_no_match_returns_1_0() {
        let qi_color = make_color(ColorKind::Sharp, None, false, false);
        let bonus = color_style_bonus(&qi_color, ColorKind::Heavy);
        assert_eq!(
            bonus, 1.0,
            "无次色且与主色不同时应返回 1.0，实际得到 {bonus}"
        );
    }

    // ⑧ 全部 10 种 ColorKind 对自身主色均返回 1.1（enum 全覆盖，主色匹配事半功倍）
    #[test]
    fn all_color_kinds_as_main_return_1_1() {
        use ColorKind::*;
        let all = [
            Sharp, Heavy, Mellow, Solid, Light, Intricate, Gentle, Insidious, Violent, Turbid,
        ];
        for color in all {
            let qi_color = make_color(color, None, false, false);
            let bonus = color_style_bonus(&qi_color, color);
            assert_eq!(
                bonus, 1.1,
                "ColorKind::{color:?} 作为主色匹配时应返回 1.1（事半功倍），实际得到 {bonus}"
            );
        }
    }

    // ⑨ 混元时所有 active_color 都返回 0.8（博而不精，全色一律 -20%，worldview §六.2:644）
    #[test]
    fn hunyuan_returns_0_8_for_all_colors() {
        use ColorKind::*;
        let all = [
            Sharp, Heavy, Mellow, Solid, Light, Intricate, Gentle, Insidious, Violent, Turbid,
        ];
        let qi_color = make_color(Mellow, None, false, true);
        for color in all {
            let bonus = color_style_bonus(&qi_color, color);
            assert_eq!(
                bonus, 0.8,
                "混元状态下 ColorKind::{color:?} 应返回 0.8（-20% 修炼总效率代价，worldview §六.2:644），实际得到 {bonus}"
            );
        }
    }

    // ⑩ 专精最优，hunyuan 最差：验证顺序 main(1.1) > secondary(1.05) > unmatched(1.0) > chaotic(0.9) > hunyuan(0.8)
    #[test]
    fn bonus_ordering_main_gt_secondary_gt_unmatched_gt_chaotic_gt_hunyuan() {
        let main_match = color_style_bonus(
            &make_color(ColorKind::Sharp, None, false, false),
            ColorKind::Sharp,
        );
        let secondary_match = color_style_bonus(
            &make_color(ColorKind::Heavy, Some(ColorKind::Sharp), false, false),
            ColorKind::Sharp,
        );
        let unmatched = color_style_bonus(
            &make_color(ColorKind::Sharp, None, false, false),
            ColorKind::Heavy,
        );
        let chaotic = color_style_bonus(
            &make_color(ColorKind::Sharp, None, true, false),
            ColorKind::Sharp,
        );
        let hunyuan = color_style_bonus(
            &make_color(ColorKind::Mellow, None, false, true),
            ColorKind::Sharp,
        );
        assert!(
            main_match > secondary_match
                && secondary_match > unmatched
                && unmatched > chaotic
                && chaotic > hunyuan,
            "期望顺序 main({main_match}) > secondary({secondary_match}) > unmatched({unmatched}) > chaotic({chaotic}) > hunyuan({hunyuan})；\
             专精主色仍最优（worldview §六.1「事半功倍」），混元最差（§六.2:644「-20%」）"
        );
    }
}
