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
