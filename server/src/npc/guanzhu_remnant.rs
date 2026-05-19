//! P2 观主残魂 (Guanzhu Remnant) — mentor NPC at wangyintai.
//!
//! The Guanzhu was the master of the Jingxu Sect who opened a "door to see
//! the other side of qi" — and succeeded. His body got stuck between void
//! and reality, becoming a semi-void entity. He wanders the guantiantai
//! ruins and is only visible to cultivators with VoidErosion stage >= VoidShadow.
//!
//! NOT a boss — purely a mentor / quest NPC who can teach the 5 erosion
//! skills (UnlockSource::Npc) and reveal the truth about the Jingxu Sect.

use crate::combat::woliu_v2::erosion::VoidErosionStage;

/// NPC archetype identifier, used for UnlockSource::Npc matching.
pub const GUANZHU_REMNANT_NPC_ID: &str = "guanzhu_remnant";

/// Display name shown in narration.
pub const GUANZHU_REMNANT_DISPLAY_NAME: &str = "观主残魂";

/// Zone where the guanzhu remnant spawns.
pub const GUANZHU_REMNANT_ZONE: &str = "wangyintai";

/// Flicker interval in ticks — the remnant blinks in and out of visibility
/// every 40 ticks (he is stuck between void and reality).
pub const GUANZHU_FLICKER_INTERVAL_TICKS: u64 = 40;

/// Minimum VoidErosion stage required for the remnant to be visible.
pub const GUANZHU_VISIBILITY_MIN_STAGE: VoidErosionStage = VoidErosionStage::VoidShadow;

/// Number of erosion skills the guanzhu can teach.
pub const GUANZHU_TEACHABLE_SKILL_COUNT: usize = 5;

/// Narration templates for the guanzhu remnant.
pub mod narration {
    /// First visibility narration (scope: player, style: perception).
    pub const FIRST_VISIBLE: &str =
        "你看见了一个……人？不，不完全是人。他的轮廓在空气中闪烁，像水面上的倒影——但没有水。";

    /// Dialogue trigger narration (scope: player, style: dialogue).
    pub const DIALOGUE_TRIGGER: &str =
        "你也开始'听到'了？——三秒前我说的那句话。是的。你现在听到的是三秒后我要说的。或者说，三秒前我已经说过了。你分不清。我也分不清了。";

    /// About the Jingxu Sect narration (scope: player, style: narrative).
    pub const ABOUT_JINGXU: &str =
        "我打开了那扇门。看到了灵气的背面——空无一物。真的什么都没有。然后'什么都没有'看到了我们。它不是恶意的。它只是……注意到了。然后声音就不对了。";

    /// All narration templates as a slice for iteration.
    pub const ALL: &[&str] = &[FIRST_VISIBLE, DIALOGUE_TRIGGER, ABOUT_JINGXU];
}

/// Determines whether the guanzhu remnant is visible to a player based on
/// their VoidErosion stage. Only stage 2+ (VoidShadow and above) can see him.
pub fn is_visible_to_stage(stage: VoidErosionStage) -> bool {
    stage >= GUANZHU_VISIBILITY_MIN_STAGE
}

/// Picks a narration template deterministically from the pool using a seed.
pub fn pick_narration(seed: u64) -> &'static str {
    narration::ALL[(seed as usize) % narration::ALL.len()]
}

/// Logs the guanzhu remnant contract on startup for debugging.
pub fn log_guanzhu_remnant_contract() {
    tracing::debug!(
        "[bong][woliu-v4] guanzhu_remnant contract npc_id={} zone={} \
         visibility_min_stage={:?} flicker_interval={} teachable_skills={}",
        GUANZHU_REMNANT_NPC_ID,
        GUANZHU_REMNANT_ZONE,
        GUANZHU_VISIBILITY_MIN_STAGE,
        GUANZHU_FLICKER_INTERVAL_TICKS,
        GUANZHU_TEACHABLE_SKILL_COUNT,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::woliu_v2::erosion::VoidErosionStage;

    // ----- visibility gating -----

    #[test]
    fn stage_none_cannot_see_guanzhu() {
        assert!(
            !is_visible_to_stage(VoidErosionStage::None),
            "stage None (0) should NOT see guanzhu remnant"
        );
    }

    #[test]
    fn stage_low_pressure_cannot_see_guanzhu() {
        assert!(
            !is_visible_to_stage(VoidErosionStage::LowPressure),
            "stage LowPressure (1) should NOT see guanzhu remnant"
        );
    }

    #[test]
    fn stage_void_shadow_can_see_guanzhu() {
        assert!(
            is_visible_to_stage(VoidErosionStage::VoidShadow),
            "stage VoidShadow (2) should see guanzhu remnant"
        );
    }

    #[test]
    fn stage_echo_body_can_see_guanzhu() {
        assert!(
            is_visible_to_stage(VoidErosionStage::EchoBody),
            "stage EchoBody (3) should see guanzhu remnant"
        );
    }

    #[test]
    fn stage_void_eroded_can_see_guanzhu() {
        assert!(
            is_visible_to_stage(VoidErosionStage::VoidEroded),
            "stage VoidEroded (4) should see guanzhu remnant"
        );
    }

    #[test]
    fn visibility_gating_exhaustive() {
        let all_stages = [
            VoidErosionStage::None,
            VoidErosionStage::LowPressure,
            VoidErosionStage::VoidShadow,
            VoidErosionStage::EchoBody,
            VoidErosionStage::VoidEroded,
        ];
        let expected = [false, false, true, true, true];
        for (stage, &expect) in all_stages.iter().zip(expected.iter()) {
            assert_eq!(
                is_visible_to_stage(*stage),
                expect,
                "visibility for {:?} should be {}, got {}",
                stage,
                expect,
                is_visible_to_stage(*stage)
            );
        }
    }

    // ----- constants pin -----

    #[test]
    fn npc_id_matches_plan_spec() {
        assert_eq!(GUANZHU_REMNANT_NPC_ID, "guanzhu_remnant");
    }

    #[test]
    fn zone_is_wangyintai() {
        assert_eq!(GUANZHU_REMNANT_ZONE, "wangyintai");
    }

    #[test]
    fn flicker_interval_is_40_ticks() {
        assert_eq!(
            GUANZHU_FLICKER_INTERVAL_TICKS, 40,
            "flicker interval should be 40 ticks per plan spec"
        );
    }

    #[test]
    fn teachable_skills_count_is_5() {
        assert_eq!(
            GUANZHU_TEACHABLE_SKILL_COUNT, 5,
            "guanzhu should teach 5 erosion skills per plan spec"
        );
    }

    #[test]
    fn visibility_min_stage_is_void_shadow() {
        assert_eq!(
            GUANZHU_VISIBILITY_MIN_STAGE,
            VoidErosionStage::VoidShadow,
            "minimum visibility stage should be VoidShadow (stage 2)"
        );
    }

    // ----- narration -----

    #[test]
    fn narration_templates_count_is_3() {
        assert_eq!(
            narration::ALL.len(),
            3,
            "guanzhu should have exactly 3 narration templates per plan spec"
        );
    }

    #[test]
    fn narration_first_visible_matches_plan() {
        assert!(
            narration::FIRST_VISIBLE.contains("轮廓在空气中闪烁"),
            "first visible narration should match plan spec text"
        );
    }

    #[test]
    fn narration_dialogue_trigger_matches_plan() {
        assert!(
            narration::DIALOGUE_TRIGGER.contains("三秒前"),
            "dialogue trigger narration should reference temporal displacement"
        );
    }

    #[test]
    fn narration_about_jingxu_matches_plan() {
        assert!(
            narration::ABOUT_JINGXU.contains("灵气的背面"),
            "about jingxu narration should reference seeing the other side of qi"
        );
    }

    #[test]
    fn pick_narration_deterministic() {
        let a = pick_narration(42);
        let b = pick_narration(42);
        assert_eq!(a, b, "same seed should yield same narration");
    }

    #[test]
    fn pick_narration_returns_valid_template() {
        for seed in 0..100 {
            let picked = pick_narration(seed);
            assert!(
                narration::ALL.contains(&picked),
                "pick_narration({}) returned '{}' which is not in the narration pool",
                seed,
                picked
            );
        }
    }

    #[test]
    fn pick_narration_covers_all_templates() {
        let mut seen = std::collections::HashSet::new();
        for seed in 0..100 {
            seen.insert(pick_narration(seed));
        }
        assert_eq!(
            seen.len(),
            narration::ALL.len(),
            "100 seeds should cover all {} narration templates",
            narration::ALL.len()
        );
    }

    // ----- display name -----

    #[test]
    fn display_name_is_chinese() {
        assert_eq!(GUANZHU_REMNANT_DISPLAY_NAME, "观主残魂");
    }
}
