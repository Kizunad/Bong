use crate::botany::registry::{
    alias_of, canonicalize_herb_id, BotanyPlantId, BAI_CAO_ALIAS, KAI_MAI_CAO_ALIAS, XUE_CAO_ALIAS,
};

#[allow(dead_code)]
pub fn resolve_placeholder_material_id(raw: &str) -> Result<BotanyPlantId, String> {
    canonicalize_herb_id(raw)
}

#[allow(dead_code)]
pub fn is_placeholder_material(raw: &str) -> bool {
    alias_of(raw).is_some()
}

#[allow(dead_code)]
pub fn placeholder_mapping_pairs() -> [(&'static str, &'static str); 3] {
    [
        (KAI_MAI_CAO_ALIAS, "ning_mai_cao"),
        (XUE_CAO_ALIAS, "chi_sui_cao"),
        (BAI_CAO_ALIAS, "hui_yuan_zhi"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_material_alignment() {
        let pairs = placeholder_mapping_pairs();
        assert_eq!(pairs[0].0, "kai_mai_cao");
        assert_eq!(pairs[0].1, "ning_mai_cao");
        assert_eq!(pairs[1].0, "xue_cao");
        assert_eq!(pairs[1].1, "chi_sui_cao");
        assert_eq!(pairs[2].0, "bai_cao");
        assert_eq!(pairs[2].1, "hui_yuan_zhi");

        assert_eq!(
            resolve_placeholder_material_id("kai_mai_cao").unwrap(),
            BotanyPlantId::NingMaiCao
        );
        assert_eq!(
            resolve_placeholder_material_id("xue_cao").unwrap(),
            BotanyPlantId::ChiSuiCao
        );
        assert_eq!(
            resolve_placeholder_material_id("bai_cao").unwrap(),
            BotanyPlantId::HuiYuanZhi
        );

        assert!(is_placeholder_material("kai_mai_cao"));
        assert!(!is_placeholder_material("ci_she_hao"));
    }
}
