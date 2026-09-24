#![allow(dead_code, unused_imports)]

use bong_server::cultivation::components::MeridianId;
use bong_server::cultivation::meridian::severed::SkillMeridianDependencies;
use bong_server::cultivation::skill_registry::SkillRegistry;
use bong_server::sword_path::skill_register::{
    declare_meridian_dependencies, register_skills, SWORD_PATH_CONDENSE_EDGE_ID,
    SWORD_PATH_HEAVEN_GATE_ID, SWORD_PATH_MANIFEST_ID, SWORD_PATH_QI_SLASH_ID,
    SWORD_PATH_RESONANCE_ID,
};

/// P1.4 — SkillRegistry 注册 5 招后可 lookup 命中。
#[test]
fn registry_lookup_finds_all_five_techniques() {
    let mut registry = SkillRegistry::default();
    register_skills(&mut registry);
    for id in [
        SWORD_PATH_CONDENSE_EDGE_ID,
        SWORD_PATH_QI_SLASH_ID,
        SWORD_PATH_RESONANCE_ID,
        SWORD_PATH_MANIFEST_ID,
        SWORD_PATH_HEAVEN_GATE_ID,
    ] {
        assert!(
            registry.lookup(id).is_some(),
            "招式 {id} 必须可 lookup，否则 SkillBar cast 走不通"
        );
    }
}

/// P1.5 — 经脉依赖按 plan §P1.5 声明落地。
#[test]
fn meridian_dependencies_match_plan_table() {
    let mut deps = SkillMeridianDependencies::default();
    declare_meridian_dependencies(&mut deps);
    assert_eq!(
        deps.lookup(SWORD_PATH_CONDENSE_EDGE_ID),
        &[MeridianId::LargeIntestine, MeridianId::SmallIntestine][..]
    );
    assert_eq!(
        deps.lookup(SWORD_PATH_QI_SLASH_ID),
        &[
            MeridianId::LargeIntestine,
            MeridianId::SmallIntestine,
            MeridianId::TripleEnergizer,
        ][..]
    );
    assert_eq!(deps.lookup(SWORD_PATH_HEAVEN_GATE_ID).len(), 4);
    assert!(deps
        .lookup(SWORD_PATH_HEAVEN_GATE_ID)
        .contains(&MeridianId::Du));
}
