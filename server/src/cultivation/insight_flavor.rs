//! 顿悟三轨文案模板。

use super::components::ColorKind;
use super::generic_talent::color_kind_to_chinese;
use super::insight::InsightAlignment;

pub fn flavor_for(
    trigger_id: &str,
    alignment: InsightAlignment,
    main_color: ColorKind,
    target_color: Option<ColorKind>,
    effect_desc: &str,
) -> String {
    let main = color_kind_to_chinese(main_color);
    let target = color_kind_to_chinese(target_color.unwrap_or(main_color));
    match (alignment, trigger_id) {
        (InsightAlignment::Converge, id) if id.starts_with("first_breakthrough") => {
            format!("你的真元已染{main}之意。此刻，它渴望更深。{effect_desc}")
        }
        (InsightAlignment::Neutral, id) if id.starts_with("first_breakthrough") => {
            format!("突破的余韵尚在。天地给了你一个平淡的馈赠。{effect_desc}")
        }
        (InsightAlignment::Diverge, id) if id.starts_with("first_breakthrough") => {
            format!("你感到体内有一缕不属于{main}的真元在涌动。{target}之意正在叩门。{effect_desc}")
        }
        (InsightAlignment::Converge, "survived_negative_zone") => {
            format!("负灵域没有杀死你——你的{main}之气比你以为的更韧。{effect_desc}")
        }
        (InsightAlignment::Diverge, "killed_higher_realm") => {
            format!("击杀强者的瞬间，你感到对方真元余韵里有{target}之意。{effect_desc}")
        }
        (InsightAlignment::Converge, _) => {
            format!("{main}之意向内收束。{effect_desc}")
        }
        (InsightAlignment::Neutral, _) => effect_desc.to_string(),
        (InsightAlignment::Diverge, _) => {
            format!("你放开旧路一角，转向{target}。{effect_desc}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gain_flavor_contains_color_name() {
        let text = flavor_for(
            "first_breakthrough_to_Induce",
            InsightAlignment::Converge,
            ColorKind::Sharp,
            None,
            "经脉流速提升。",
        );
        assert!(text.contains("锋锐"));
    }
}
