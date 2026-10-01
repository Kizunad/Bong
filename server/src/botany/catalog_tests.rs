use super::*;
use crate::botany::catalog::{PlantCatalog, CATALOG_JSON};
use serde_json::{json, Value};

// 迁移前的野生规则快照替代旧注册表中逐物种的字段断言，保护采集风险、
// 灵气消耗和刷新规则；已有生命周期及采集行为测试仍在原模块运行。
#[test]
fn catalog_preserves_existing_wild_rules() {
    let baseline: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/wild_catalog_before_migration.json")).unwrap();
    let registry = BotanyKindRegistry::default();
    for mut raw in baseline {
        let id = raw.as_object_mut().unwrap().remove("id").unwrap();
        let id = id.as_str().unwrap();
        let mut expected: BotanyPlantKind = serde_json::from_value(raw).unwrap();
        expected.id =
            BotanyPlantId::from_canonical(id).expect("existing plant must remain registered");
        assert_eq!(
            registry.get(id),
            Some(&expected),
            "wild rules changed for {id}"
        );
    }
}

#[test]
fn catalog_registers_new_species_and_alias_without_code_changes() {
    let mut doc: Value = serde_json::from_str(CATALOG_JSON).unwrap();
    let mut plant = doc["plants"][0].clone();
    plant["id"] = json!("test_new_herb");
    plant["aliases"] = json!(["test_alias"]);
    doc["plants"].as_array_mut().unwrap().push(plant);
    let catalog = PlantCatalog::from_json(&doc.to_string()).unwrap();
    let registry = BotanyKindRegistry::from_catalog(&catalog);
    let id = registry.canonicalize(" TEST_ALIAS ").unwrap();
    assert_eq!(id.as_str(), "test_new_herb");
    assert!(registry.get(&id).is_some());
    assert_eq!(catalog.resolve("test_alias").unwrap().id, id);
    assert!(registry.canonicalize("missing_herb").is_err());
}

#[test]
fn visual_only_species_does_not_spawn_in_wild_registry() {
    let mut doc: Value = serde_json::from_str(CATALOG_JSON).unwrap();
    let mut plant = doc["plants"][0].clone();
    plant["id"] = json!("visual_only_herb");
    plant.as_object_mut().unwrap().remove("wild");
    doc["plants"] = json!([plant]);
    let catalog = PlantCatalog::from_json(&doc.to_string()).unwrap();
    assert!(catalog.get("visual_only_herb").is_some());
    assert!(BotanyKindRegistry::from_catalog(&catalog)
        .get("visual_only_herb")
        .is_none());
}

#[test]
fn catalog_rejects_ambiguous_ids_and_invalid_configuration() {
    let original: Value = serde_json::from_str(CATALOG_JSON).unwrap();
    for (pointer, invalid) in [
        ("/schema_version", json!(2)),
        ("/plants/1/id", original["plants"][0]["id"].clone()),
        ("/plants/1/aliases", json!([original["plants"][0]["id"]])),
        ("/plants/0/visual/stages/mature/scale", json!(0)),
        (
            "/plants/0/visual/stages/mature/texture",
            json!("bong:../escape.png"),
        ),
        ("/plants/0/wild/growth_cost", json!(-1)),
    ] {
        let mut doc = original.clone();
        *doc.pointer_mut(pointer).unwrap() = invalid;
        assert!(
            PlantCatalog::from_json(&doc.to_string()).is_err(),
            "accepted invalid {pointer}"
        );
    }
    let mut doc = original;
    doc["plants"][0]["wild"]["growth_cost_typo"] = json!(1);
    assert!(
        PlantCatalog::from_json(&doc.to_string()).is_err(),
        "unknown keys must not silently change gameplay"
    );
}

#[test]
fn aliases_remain_explicit_and_canonical_roundtrips() {
    for (alias, expected) in [
        ("kai_mai_cao", BotanyPlantId::NingMaiCao),
        ("xue_cao", BotanyPlantId::ChiSuiCao),
        ("bai_cao", BotanyPlantId::HuiYuanZhi),
    ] {
        assert_eq!(canonicalize_herb_id(alias).unwrap(), expected);
    }
    for plant in PlantCatalog::builtin().iter() {
        assert_eq!(
            BotanyPlantId::from_canonical(plant.id.as_str()).as_ref(),
            Some(&plant.id)
        );
    }
    assert!(BotanyPlantId::from_canonical("kai_mai_cao").is_none());
}
