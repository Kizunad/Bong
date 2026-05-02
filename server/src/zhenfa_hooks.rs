use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ZhenfaHookManifest {
    version: u8,
    hooks: Vec<ZhenfaHook>,
}

#[derive(Debug, Deserialize)]
struct ZhenfaHook {
    hook_id: String,
    future_plan: String,
    materials: Vec<String>,
    intended_use: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    #[test]
    fn botany_v2_zhenfa_hook_mentions_reserved_materials() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/zhenfa_hooks/botany_v2_hooks.json");
        let content = fs::read_to_string(path).expect("zhenfa hook manifest should load");
        let manifest: ZhenfaHookManifest =
            serde_json::from_str(&content).expect("zhenfa hook manifest should parse");
        assert_eq!(manifest.version, 1);
        let hook = manifest
            .hooks
            .iter()
            .find(|hook| hook.hook_id == "di_shi_wei_xu_array_carrier")
            .expect("botany v2 zhenfa hook should exist");
        assert_eq!(hook.future_plan, "plan-zhenfa-v1");
        assert!(hook.materials.iter().any(|id| id == "lie_yuan_tai"));
        assert!(hook.materials.iter().any(|id| id == "bei_wen_zhi"));
        assert!(!hook.intended_use.is_empty());
    }

    #[test]
    fn spiritwood_zhenfa_hook_mentions_ling_mu_carriers() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets/zhenfa_hooks/spiritwood_v1_hooks.json");
        let content = fs::read_to_string(path).expect("spiritwood zhenfa hook should load");
        let manifest: ZhenfaHookManifest =
            serde_json::from_str(&content).expect("spiritwood zhenfa hook should parse");
        assert_eq!(manifest.version, 1);
        let hook = manifest
            .hooks
            .iter()
            .find(|hook| hook.hook_id == "spiritwood_ling_mu_array_carrier")
            .expect("spiritwood zhenfa hook should exist");
        assert_eq!(hook.future_plan, "plan-zhenfa-v1");
        assert!(hook.materials.iter().any(|id| id == "ling_mu_gun"));
        assert!(hook.materials.iter().any(|id| id == "ling_mu_ban"));
        assert!(hook.intended_use.contains("12 小时"));
    }
}
