use serde::{Deserialize, Serialize};

pub const PSEUDO_VEIN_PROFILE: &str = "pseudo_vein_oasis";
pub const PSEUDO_VEIN_ZONE_PREFIX: &str = "pseudo_vein_";
pub const PSEUDO_VEIN_DISPLAY_NAME: &str = "伪灵脉";
pub const PSEUDO_VEIN_SPIRIT_QI: f64 = 0.60;
pub const PSEUDO_VEIN_DANGER_LEVEL: u8 = 4;
pub const PSEUDO_VEIN_SIZE_XZ: [i32; 2] = [300, 300];
pub const PSEUDO_VEIN_DEFAULT_BASE_Y: i32 = 60;
pub const PSEUDO_VEIN_HEIGHT: i32 = 30;
pub const PSEUDO_VEIN_CORE_RADIUS: i32 = 60;
pub const PSEUDO_VEIN_RIM_RADIUS: i32 = 120;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundaryTemplate {
    pub mode: String,
    pub width: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransientWorldgenTemplate {
    pub terrain_profile: String,
    pub shape: String,
    pub boundary: BoundaryTemplate,
    pub landmarks: Vec<String>,
    pub core_radius: i32,
    pub rim_radius: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransientBlueprintZoneTemplate {
    pub name: String,
    pub display_name: String,
    pub aabb: AabbTemplate,
    pub center_xz: [i32; 2],
    pub size_xz: [i32; 2],
    pub spirit_qi: f64,
    pub danger_level: u8,
    pub worldgen: TransientWorldgenTemplate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AabbTemplate {
    pub min: [i32; 3],
    pub max: [i32; 3],
}

pub fn is_pseudo_vein_zone_name(name: &str) -> bool {
    name.starts_with(PSEUDO_VEIN_ZONE_PREFIX) && name.len() > PSEUDO_VEIN_ZONE_PREFIX.len()
}

pub fn pseudo_vein_zone_name(id: &str) -> Result<String, String> {
    let trimmed = id.trim();
    if trimmed.is_empty() {
        return Err("pseudo vein id must not be empty".to_string());
    }
    if trimmed.contains(char::is_whitespace) {
        return Err("pseudo vein id must not contain whitespace".to_string());
    }
    Ok(format!("{PSEUDO_VEIN_ZONE_PREFIX}{trimmed}"))
}

pub fn build_pseudo_vein_blueprint_zone(
    id: &str,
    center_xz: [i32; 2],
    base_y: i32,
) -> Result<TransientBlueprintZoneTemplate, String> {
    let name = pseudo_vein_zone_name(id)?;
    let half_x = PSEUDO_VEIN_SIZE_XZ[0] / 2;
    let half_z = PSEUDO_VEIN_SIZE_XZ[1] / 2;
    let max_y = base_y + PSEUDO_VEIN_HEIGHT;

    Ok(TransientBlueprintZoneTemplate {
        name,
        display_name: PSEUDO_VEIN_DISPLAY_NAME.to_string(),
        aabb: AabbTemplate {
            min: [center_xz[0] - half_x, base_y, center_xz[1] - half_z],
            max: [center_xz[0] + half_x, max_y, center_xz[1] + half_z],
        },
        center_xz,
        size_xz: PSEUDO_VEIN_SIZE_XZ,
        spirit_qi: PSEUDO_VEIN_SPIRIT_QI,
        danger_level: PSEUDO_VEIN_DANGER_LEVEL,
        worldgen: TransientWorldgenTemplate {
            terrain_profile: PSEUDO_VEIN_PROFILE.to_string(),
            shape: "circular".to_string(),
            boundary: BoundaryTemplate {
                mode: "soft".to_string(),
                width: 32,
            },
            landmarks: vec![
                "phantom_qi_pillar".to_string(),
                "tiandao_seal_stele".to_string(),
            ],
            core_radius: PSEUDO_VEIN_CORE_RADIUS,
            rim_radius: PSEUDO_VEIN_RIM_RADIUS,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pseudo_vein_zone_name_requires_non_empty_id() {
        assert_eq!(
            pseudo_vein_zone_name("abc").expect("valid id"),
            "pseudo_vein_abc"
        );
        assert!(pseudo_vein_zone_name("").is_err());
        assert!(pseudo_vein_zone_name("bad id").is_err());
    }

    #[test]
    fn is_pseudo_vein_zone_name_matches_dynamic_blueprint_prefix() {
        assert!(is_pseudo_vein_zone_name("pseudo_vein_42"));
        assert!(!is_pseudo_vein_zone_name("pseudo_vein_"));
        assert!(!is_pseudo_vein_zone_name("waste_plateau"));
    }

    #[test]
    fn builds_plan_pinned_dynamic_blueprint_template() {
        let zone = build_pseudo_vein_blueprint_zone("unit", [100, -40], PSEUDO_VEIN_DEFAULT_BASE_Y)
            .expect("pseudo vein template should build");

        assert_eq!(zone.name, "pseudo_vein_unit");
        assert_eq!(zone.display_name, "伪灵脉");
        assert_eq!(zone.aabb.min, [-50, 60, -190]);
        assert_eq!(zone.aabb.max, [250, 90, 110]);
        assert_eq!(zone.center_xz, [100, -40]);
        assert_eq!(zone.size_xz, [300, 300]);
        assert_eq!(zone.spirit_qi, 0.60);
        assert_eq!(zone.danger_level, 4);
        assert_eq!(zone.worldgen.terrain_profile, "pseudo_vein_oasis");
        assert_eq!(zone.worldgen.shape, "circular");
        assert_eq!(zone.worldgen.boundary.mode, "soft");
        assert_eq!(zone.worldgen.boundary.width, 32);
        assert_eq!(zone.worldgen.core_radius, 60);
        assert_eq!(zone.worldgen.rim_radius, 120);
        assert_eq!(
            zone.worldgen.landmarks,
            vec!["phantom_qi_pillar", "tiandao_seal_stele"]
        );
    }

    #[test]
    fn build_uses_base_y_as_actual_zone_floor_without_double_offset() {
        let zone = build_pseudo_vein_blueprint_zone("unit", [0, 0], 72)
            .expect("pseudo vein template should build");

        assert_eq!(zone.aabb.min, [-150, 72, -150]);
        assert_eq!(zone.aabb.max, [150, 102, 150]);
    }
}
