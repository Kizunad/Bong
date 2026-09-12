use super::*;

// ── map_defense_kind 全变体覆盖（编译 + 语义）──────────────────────────
// 锁住 combat_bridge.rs map_defense_kind 映射的三个变体全部正确。
#[test]
fn map_defense_kind_all_variants_map_correctly() {
    use crate::combat::events::DefenseKind;
    use crate::network::combat_bridge::map_defense_kind_pub;
    use crate::schema::combat_event::CombatDefenseKindV1;

    assert_eq!(
        map_defense_kind_pub(DefenseKind::JieMai),
        CombatDefenseKindV1::JieMai,
        "JieMai 应映射到 CombatDefenseKindV1::JieMai"
    );
    assert_eq!(
        map_defense_kind_pub(DefenseKind::SwordParry),
        CombatDefenseKindV1::SwordParry,
        "SwordParry 应映射到 CombatDefenseKindV1::SwordParry"
    );
    assert_eq!(
        map_defense_kind_pub(DefenseKind::ShieldBlock),
        CombatDefenseKindV1::ShieldBlock,
        "ShieldBlock 应映射到 CombatDefenseKindV1::ShieldBlock"
    );
}

// ── KnownTechniques 含 "shield_block" 注册（cultivation registry 断言）────
#[test]
fn known_techniques_registry_contains_shield_block() {
    use crate::cultivation::known_techniques::{KnownTechniques, TechniqueRegistry};
    let registry = TechniqueRegistry::load_for_tests();
    let default = KnownTechniques::dev_default(&registry);
    let found = default.entries.iter().any(|e| e.id == "shield_block");
    assert!(
        found,
        "KnownTechniques::dev_default(&registry) 应含 \"shield_block\" 条目（plan-shield-block-v1 P4 注册）；\
         实际 entries: {:?}",
        default
            .entries
            .iter()
            .map(|e| e.id.as_str())
            .collect::<Vec<_>>()
    );
}
