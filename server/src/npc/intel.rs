//! NPC purchasable intel helpers for anonymous PvP encounters.

#![allow(dead_code)] // P3 exposes the pure purchase contract; dialogue wiring is a later NPC UI slice.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncounterIntelAsset {
    pub zone: String,
    pub appearance_hint: Option<String>,
    pub observed_style: Option<String>,
    pub qi_color_hint: Option<String>,
    pub identity_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurchasedEncounterIntel {
    pub zone: String,
    pub clues: Vec<String>,
    pub identity_disclosed: bool,
}

pub fn purchase_partial_encounter_intel(
    asset: &EncounterIntelAsset,
    paid_bone_coins: u32,
) -> PurchasedEncounterIntel {
    let clue_limit = match paid_bone_coins {
        0..=2 => 1,
        3..=9 => 2,
        _ => 3,
    };
    let mut clues = Vec::new();
    push_clue(
        &mut clues,
        "appearance",
        asset.appearance_hint.as_deref(),
        clue_limit,
    );
    push_clue(
        &mut clues,
        "style",
        asset.observed_style.as_deref(),
        clue_limit,
    );
    push_clue(
        &mut clues,
        "qi_color",
        asset.qi_color_hint.as_deref(),
        clue_limit,
    );

    let identity_value = asset
        .identity_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let identity_disclosed = paid_bone_coins >= 20 && identity_value.is_some();
    if identity_disclosed {
        push_clue(&mut clues, "identity", identity_value, clue_limit + 1);
    }

    PurchasedEncounterIntel {
        zone: asset.zone.clone(),
        clues,
        identity_disclosed,
    }
}

fn push_clue(clues: &mut Vec<String>, label: &str, value: Option<&str>, limit: usize) {
    if clues.len() >= limit {
        return;
    }
    let Some(value) = value else {
        return;
    };
    if value.trim().is_empty() {
        return;
    }
    clues.push(format!("{label}:{value}"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn npc_intel_purchase_partial_info() {
        let asset = EncounterIntelAsset {
            zone: "blood_valley".to_string(),
            appearance_hint: Some("右手持骨刺".to_string()),
            observed_style: Some("涡流气息".to_string()),
            qi_color_hint: Some("青白".to_string()),
            identity_name: Some("玄锋".to_string()),
        };

        let cheap = purchase_partial_encounter_intel(&asset, 2);
        assert_eq!(
            cheap.zone, "blood_valley",
            "expected source zone to be preserved for cheap intel, actual {}",
            cheap.zone
        );
        assert_eq!(
            cheap.clues,
            vec!["appearance:右手持骨刺"],
            "expected one appearance clue because 2 coins allows clue_limit=1, actual {:?}",
            cheap.clues
        );
        assert!(
            !cheap.identity_disclosed,
            "expected identity_disclosed=false because 2 coins is below identity threshold, actual {}",
            cheap.identity_disclosed
        );

        let expensive = purchase_partial_encounter_intel(&asset, 20);
        assert!(
            expensive.clues.contains(&"identity:玄锋".to_string()),
            "expected identity clue because 20 coins reaches disclosure threshold, actual {:?}",
            expensive.clues
        );
        assert!(
            expensive.identity_disclosed,
            "expected identity_disclosed=true because 20 coins reaches disclosure threshold, actual {}",
            expensive.identity_disclosed
        );
    }

    #[test]
    fn npc_intel_blank_identity_is_not_disclosed() {
        let asset = EncounterIntelAsset {
            zone: "blood_valley".to_string(),
            appearance_hint: Some("右手持骨刺".to_string()),
            observed_style: None,
            qi_color_hint: None,
            identity_name: Some("   ".to_string()),
        };

        let intel = purchase_partial_encounter_intel(&asset, 20);

        assert!(
            !intel.identity_disclosed,
            "blank identity should not report identity_disclosed=true"
        );
        assert!(
            intel
                .clues
                .iter()
                .all(|clue| !clue.starts_with("identity:")),
            "blank identity should not emit identity clue"
        );
    }

    #[test]
    fn npc_intel_purchase_thresholds_pin_clue_limits() {
        let asset = full_asset();

        for (paid_bone_coins, expected_clues, identity_disclosed) in [
            (1, vec!["appearance:右手持骨刺"], false),
            (2, vec!["appearance:右手持骨刺"], false),
            (3, vec!["appearance:右手持骨刺", "style:涡流气息"], false),
            (9, vec!["appearance:右手持骨刺", "style:涡流气息"], false),
            (
                10,
                vec!["appearance:右手持骨刺", "style:涡流气息", "qi_color:青白"],
                false,
            ),
            (
                19,
                vec!["appearance:右手持骨刺", "style:涡流气息", "qi_color:青白"],
                false,
            ),
            (
                20,
                vec![
                    "appearance:右手持骨刺",
                    "style:涡流气息",
                    "qi_color:青白",
                    "identity:玄锋",
                ],
                true,
            ),
            (
                u32::MAX,
                vec![
                    "appearance:右手持骨刺",
                    "style:涡流气息",
                    "qi_color:青白",
                    "identity:玄锋",
                ],
                true,
            ),
        ] {
            let intel = purchase_partial_encounter_intel(&asset, paid_bone_coins);

            assert_eq!(
                intel.clues, expected_clues,
                "paid {paid_bone_coins} should return deterministic truncated clues"
            );
            assert_eq!(
                intel.identity_disclosed, identity_disclosed,
                "paid {paid_bone_coins} should flip identity only at 20+ coins"
            );
        }
    }

    #[test]
    fn npc_intel_clue_limit_truncates_before_identity_threshold() {
        let asset = full_asset();

        let intel = purchase_partial_encounter_intel(&asset, 3);

        assert_eq!(
            intel.clues,
            vec!["appearance:右手持骨刺", "style:涡流气息"],
            "paid 3 should cap clues at two in deterministic priority order"
        );
        assert!(
            !intel.identity_disclosed,
            "paid 3 should not disclose identity before the 20 coin threshold"
        );
    }

    fn full_asset() -> EncounterIntelAsset {
        EncounterIntelAsset {
            zone: "blood_valley".to_string(),
            appearance_hint: Some("右手持骨刺".to_string()),
            observed_style: Some("涡流气息".to_string()),
            qi_color_hint: Some("青白".to_string()),
            identity_name: Some("玄锋".to_string()),
        }
    }
}
