//! 真元色练习效率加成（plan-color-v1 §1 / §6 收口决议）。
//!
//! `color_style_bonus(qi_color, active_color)` 是纯函数，返回练习权重倍率：
//!
//! | 状态 | 说明 | 倍率 |
//! |------|------|------|
//! | is_chaotic | 杂色 — 专精失效（worldview §六.2）| 1.1（-10% 效率惩罚） |
//! | is_hunyuan | 混元 — 通达各色但无专精优势 | 1.0（无加速无惩罚） |
//! | main == active_color | 主色匹配 — 事半功倍 | 0.9 |
//! | secondary == Some(active_color) | 次色匹配 | 0.95 |
//! | 其他 | 非专色 | 1.0 |
//!
//! **硬约束（worldview §六.2）**：
//! 该倍率仅作用于 `PracticeLog.add(color, amount)` 的 amount 参数，
//! 不进任何战斗伤害/消耗公式，不改 `qi_current`，严禁复用到战斗路径。

use crate::cultivation::components::{ColorKind, QiColor};

/// 返回练习权重倍率（< 1.0 表示加速，> 1.0 表示惩罚）。
///
/// **is_chaotic 优先于 is_hunyuan**（两者同为 true 时取 chaotic 1.1x）。
///
/// 不进任何战斗公式 — 仅用于 `PracticeLog.add(color, amount * bonus)`.
/// 返回 f64 以避免 f32→f64 精度损失影响权重累积。
pub fn color_style_bonus(qi_color: &QiColor, active_color: ColorKind) -> f64 {
    // is_chaotic 优先
    if qi_color.is_chaotic {
        return 1.1;
    }
    if qi_color.is_hunyuan {
        return 1.0;
    }
    if qi_color.main == active_color {
        return 0.9;
    }
    if qi_color.secondary == Some(active_color) {
        return 0.95;
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

    // ① 主色匹配 → 0.9（加速）
    #[test]
    fn main_color_match_returns_0_9() {
        let qi_color = make_color(ColorKind::Sharp, None, false, false);
        let bonus = color_style_bonus(&qi_color, ColorKind::Sharp);
        assert_eq!(
            bonus, 0.9,
            "主色匹配应返回 0.9（事半功倍），实际得到 {bonus}"
        );
    }

    // ② 次色匹配 → 0.95
    #[test]
    fn secondary_color_match_returns_0_95() {
        let qi_color = make_color(ColorKind::Heavy, Some(ColorKind::Solid), false, false);
        let bonus = color_style_bonus(&qi_color, ColorKind::Solid);
        assert_eq!(bonus, 0.95, "次色匹配应返回 0.95，实际得到 {bonus}");
    }

    // ③ 杂色 → 1.1（惩罚）
    #[test]
    fn chaotic_returns_1_1() {
        let qi_color = make_color(ColorKind::Sharp, None, true, false);
        let bonus = color_style_bonus(&qi_color, ColorKind::Sharp);
        assert_eq!(
            bonus, 1.1,
            "杂色应返回 1.1（-10% 效率惩罚），即使与主色相同，实际得到 {bonus}"
        );
    }

    // ④ 混元 → 1.0（无加速无惩罚）
    #[test]
    fn hunyuan_returns_1_0() {
        let qi_color = make_color(ColorKind::Mellow, None, false, true);
        let bonus = color_style_bonus(&qi_color, ColorKind::Sharp);
        assert_eq!(
            bonus, 1.0,
            "混元应返回 1.0（覆盖全局但无加速），实际得到 {bonus}"
        );
    }

    // ⑤ 不匹配（既非主色也非次色，非杂色非混元）→ 1.0
    #[test]
    fn no_match_returns_1_0() {
        let qi_color = make_color(ColorKind::Sharp, Some(ColorKind::Heavy), false, false);
        let bonus = color_style_bonus(&qi_color, ColorKind::Mellow);
        assert_eq!(bonus, 1.0, "非专色不匹配应返回 1.0，实际得到 {bonus}");
    }

    // ⑥ 边界：is_chaotic 同时 is_hunyuan=true → chaotic 优先（返回 1.1）
    #[test]
    fn chaotic_takes_priority_over_hunyuan() {
        let qi_color = make_color(ColorKind::Sharp, None, true, true);
        let bonus = color_style_bonus(&qi_color, ColorKind::Sharp);
        assert_eq!(
            bonus, 1.1,
            "is_chaotic 与 is_hunyuan 同为 true 时 chaotic 优先返回 1.1，实际得到 {bonus}"
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

    // ⑧ 全部 10 种 ColorKind 对自身主色均返回 0.9（enum 全覆盖）
    #[test]
    fn all_color_kinds_as_main_return_0_9() {
        use ColorKind::*;
        let all = [
            Sharp, Heavy, Mellow, Solid, Light, Intricate, Gentle, Insidious, Violent, Turbid,
        ];
        for color in all {
            let qi_color = make_color(color, None, false, false);
            let bonus = color_style_bonus(&qi_color, color);
            assert_eq!(
                bonus, 0.9,
                "ColorKind::{color:?} 作为主色匹配时应返回 0.9，实际得到 {bonus}"
            );
        }
    }

    // ⑨ 混元时所有 active_color 都返回 1.0（混元对全色一视同仁）
    #[test]
    fn hunyuan_returns_1_0_for_all_colors() {
        use ColorKind::*;
        let all = [
            Sharp, Heavy, Mellow, Solid, Light, Intricate, Gentle, Insidious, Violent, Turbid,
        ];
        let qi_color = make_color(Mellow, None, false, true);
        for color in all {
            let bonus = color_style_bonus(&qi_color, color);
            assert_eq!(
                bonus, 1.0,
                "混元状态下 ColorKind::{color:?} 应返回 1.0，实际得到 {bonus}"
            );
        }
    }
}
