use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use valence::prelude::{bevy_ecs, Resource};

use crate::combat::components::Casting;
use crate::cultivation::components::MeridianId;
use crate::cultivation::known_techniques::technique_definition;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SkillConfig {
    pub fields: BTreeMap<String, Value>,
}

impl SkillConfig {
    pub fn new(fields: BTreeMap<String, Value>) -> Self {
        Self { fields }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillConfigSnapshot {
    pub configs: BTreeMap<String, SkillConfig>,
}

#[derive(Debug, Default, Resource)]
pub struct SkillConfigStore {
    configs: HashMap<String, BTreeMap<String, SkillConfig>>,
}

impl SkillConfigStore {
    pub fn config_for(&self, player_id: &str, skill_id: &str) -> Option<&SkillConfig> {
        self.configs
            .get(player_id)
            .and_then(|by_skill| by_skill.get(skill_id))
    }

    pub fn set_config(&mut self, player_id: &str, skill_id: &str, config: SkillConfig) {
        self.configs
            .entry(player_id.to_string())
            .or_default()
            .insert(skill_id.to_string(), config);
    }

    pub fn clear_config(&mut self, player_id: &str, skill_id: &str) {
        if let Some(by_skill) = self.configs.get_mut(player_id) {
            by_skill.remove(skill_id);
            if by_skill.is_empty() {
                self.configs.remove(player_id);
            }
        }
    }

    pub fn replace_player_configs(
        &mut self,
        player_id: &str,
        configs: BTreeMap<String, SkillConfig>,
    ) {
        if configs.is_empty() {
            self.configs.remove(player_id);
        } else {
            self.configs.insert(player_id.to_string(), configs);
        }
    }

    pub fn snapshot_for_player(&self, player_id: &str) -> SkillConfigSnapshot {
        SkillConfigSnapshot {
            configs: self.configs.get(player_id).cloned().unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SkillConfigSchema {
    pub skill_id: &'static str,
    pub fields: Vec<ConfigField>,
}

#[derive(Debug, Clone)]
pub struct ConfigField {
    pub key: &'static str,
    #[allow(dead_code)]
    pub label: &'static str,
    pub kind: ConfigFieldKind,
    pub required: bool,
    pub default: Option<Value>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ConfigFieldKind {
    Enum { options: Vec<&'static str> },
    MeridianId { allowed: Vec<MeridianId> },
    IntRange { min: i32, max: i32, step: i32 },
    FloatRange { min: f32, max: f32, step: f32 },
    Bool,
}

#[derive(Debug, Clone, Resource)]
pub struct SkillConfigSchemas {
    schemas: HashMap<&'static str, SkillConfigSchema>,
}

impl SkillConfigSchemas {
    pub fn empty() -> Self {
        Self {
            schemas: HashMap::new(),
        }
    }

    pub fn new(schemas: impl IntoIterator<Item = SkillConfigSchema>) -> Self {
        let mut registry = Self::empty();
        for schema in schemas {
            registry.register(schema);
        }
        registry
    }

    pub fn register(&mut self, schema: SkillConfigSchema) {
        self.schemas.insert(schema.skill_id, schema);
    }

    pub fn get(&self, skill_id: &str) -> Option<&SkillConfigSchema> {
        self.schemas.get(skill_id)
    }

    #[allow(dead_code)]
    pub fn has_schema(&self, skill_id: &str) -> bool {
        self.schemas.contains_key(skill_id)
    }
}

impl Default for SkillConfigSchemas {
    fn default() -> Self {
        Self::new([SkillConfigSchema {
            skill_id: "zhenmai.sever_chain",
            fields: vec![
                ConfigField {
                    key: "meridian_id",
                    label: "选定经脉",
                    kind: ConfigFieldKind::MeridianId {
                        allowed: MeridianId::ALL.to_vec(),
                    },
                    required: true,
                    default: Some(json!("Lung")),
                },
                ConfigField {
                    key: "backfire_kind",
                    label: "反震加成攻击类型",
                    kind: ConfigFieldKind::Enum {
                        options: vec!["real_yuan", "physical_carrier", "tainted_yuan", "array"],
                    },
                    required: true,
                    default: Some(json!("real_yuan")),
                },
            ],
        }])
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillConfigRejectReason {
    UnknownSkill,
    NoSchema,
    StoreUnavailable,
    UnknownField(String),
    MissingRequiredField(String),
    InvalidFieldValue(String),
    CurrentlyCasting,
}

pub fn validate_skill_config(
    skill_id: &str,
    config: BTreeMap<String, Value>,
    schemas: &SkillConfigSchemas,
) -> Result<SkillConfig, SkillConfigRejectReason> {
    let Some(schema) = schemas.get(skill_id) else {
        return Err(SkillConfigRejectReason::NoSchema);
    };
    let mut fields = config;
    for key in fields.keys() {
        if !schema.fields.iter().any(|field| field.key == key) {
            return Err(SkillConfigRejectReason::UnknownField(key.clone()));
        }
    }
    for field in &schema.fields {
        match fields.get(field.key) {
            Some(value) => validate_field_value(field, value)
                .map_err(|_| SkillConfigRejectReason::InvalidFieldValue(field.key.to_string()))?,
            None if field.required => {
                return Err(SkillConfigRejectReason::MissingRequiredField(
                    field.key.to_string(),
                ));
            }
            None => {
                if let Some(default) = field.default.clone() {
                    fields.insert(field.key.to_string(), default);
                }
            }
        }
    }
    Ok(SkillConfig::new(fields))
}

pub fn handle_config_intent(
    player_id: &str,
    skill_id: &str,
    config: BTreeMap<String, Value>,
    current_casting: Option<&Casting>,
    store: &mut SkillConfigStore,
    schemas: &SkillConfigSchemas,
) -> Result<SkillConfigSnapshot, SkillConfigRejectReason> {
    if technique_definition(skill_id).is_none() {
        return Err(SkillConfigRejectReason::UnknownSkill);
    }
    if current_casting
        .and_then(|casting| casting.skill_id.as_deref())
        .is_some_and(|casting_skill_id| casting_skill_id == skill_id)
    {
        return Err(SkillConfigRejectReason::CurrentlyCasting);
    }
    if config.is_empty() {
        store.clear_config(player_id, skill_id);
        return Ok(store.snapshot_for_player(player_id));
    }
    let validated = validate_skill_config(skill_id, config, schemas)?;
    store.set_config(player_id, skill_id, validated);
    Ok(store.snapshot_for_player(player_id))
}

pub fn skill_config_snapshot_for_cast(
    store: Option<&SkillConfigStore>,
    player_id: &str,
    skill_id: &str,
) -> Option<SkillConfig> {
    store
        .and_then(|store| store.config_for(player_id, skill_id))
        .cloned()
}

fn validate_field_value(field: &ConfigField, value: &Value) -> Result<(), ()> {
    match &field.kind {
        ConfigFieldKind::Enum { options } => {
            let Some(value) = value.as_str() else {
                return Err(());
            };
            if options.contains(&value) {
                Ok(())
            } else {
                Err(())
            }
        }
        ConfigFieldKind::MeridianId { allowed } => {
            let Some(value) = value.as_str() else {
                return Err(());
            };
            let meridian = serde_json::from_value::<MeridianId>(Value::String(value.to_string()))
                .map_err(|_| ())?;
            if allowed.contains(&meridian) {
                Ok(())
            } else {
                Err(())
            }
        }
        ConfigFieldKind::IntRange { min, max, step } => {
            let Some(value) = value.as_i64() else {
                return Err(());
            };
            let value = i32::try_from(value).map_err(|_| ())?;
            if value < *min || value > *max {
                return Err(());
            }
            if *step > 0 && (value - *min) % *step != 0 {
                return Err(());
            }
            Ok(())
        }
        ConfigFieldKind::FloatRange { min, max, step } => {
            let Some(value) = value.as_f64() else {
                return Err(());
            };
            let value = value as f32;
            if !value.is_finite() || value < *min || value > *max {
                return Err(());
            }
            if *step > 0.0 {
                let ticks = ((value - *min) / *step).round();
                if ((ticks * *step + *min) - value).abs() > 0.0001 {
                    return Err(());
                }
            }
            Ok(())
        }
        ConfigFieldKind::Bool => {
            if value.is_boolean() {
                Ok(())
            } else {
                Err(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_schemas() -> SkillConfigSchemas {
        SkillConfigSchemas::new([SkillConfigSchema {
            skill_id: "burst_meridian.beng_quan",
            fields: vec![
                ConfigField {
                    key: "stance",
                    label: "架势",
                    kind: ConfigFieldKind::Enum {
                        options: vec!["short", "long"],
                    },
                    required: true,
                    default: None,
                },
                ConfigField {
                    key: "meridian_id",
                    label: "经脉",
                    kind: ConfigFieldKind::MeridianId {
                        allowed: vec![MeridianId::LargeIntestine, MeridianId::Pericardium],
                    },
                    required: true,
                    default: None,
                },
                ConfigField {
                    key: "confirm",
                    label: "确认",
                    kind: ConfigFieldKind::Bool,
                    required: false,
                    default: Some(json!(false)),
                },
            ],
        }])
    }

    fn numeric_schemas() -> SkillConfigSchemas {
        SkillConfigSchemas::new([SkillConfigSchema {
            skill_id: "burst_meridian.beng_quan",
            fields: vec![
                ConfigField {
                    key: "intensity",
                    label: "强度",
                    kind: ConfigFieldKind::IntRange {
                        min: 0,
                        max: 10,
                        step: 2,
                    },
                    required: true,
                    default: None,
                },
                ConfigField {
                    key: "ratio",
                    label: "比例",
                    kind: ConfigFieldKind::FloatRange {
                        min: 0.0,
                        max: 1.0,
                        step: 0.25,
                    },
                    required: true,
                    default: None,
                },
            ],
        }])
    }

    #[test]
    fn validates_required_enum_meridian_and_bool_defaults() {
        let mut config = BTreeMap::new();
        config.insert("stance".to_string(), json!("short"));
        config.insert("meridian_id".to_string(), json!("Pericardium"));

        let validated =
            validate_skill_config("burst_meridian.beng_quan", config, &test_schemas()).unwrap();

        assert_eq!(validated.fields.get("stance"), Some(&json!("short")));
        assert_eq!(validated.fields.get("confirm"), Some(&json!(false)));
    }

    #[test]
    fn rejects_unknown_missing_and_invalid_fields() {
        let schemas = test_schemas();
        assert_eq!(
            validate_skill_config("burst_meridian.beng_quan", BTreeMap::new(), &schemas)
                .unwrap_err(),
            SkillConfigRejectReason::MissingRequiredField("stance".to_string())
        );

        let mut unknown = BTreeMap::new();
        unknown.insert("typo".to_string(), json!(true));
        assert_eq!(
            validate_skill_config("burst_meridian.beng_quan", unknown, &schemas).unwrap_err(),
            SkillConfigRejectReason::UnknownField("typo".to_string())
        );

        let mut invalid = BTreeMap::new();
        invalid.insert("stance".to_string(), json!("side"));
        invalid.insert("meridian_id".to_string(), json!("Pericardium"));
        assert_eq!(
            validate_skill_config("burst_meridian.beng_quan", invalid, &schemas).unwrap_err(),
            SkillConfigRejectReason::InvalidFieldValue("stance".to_string())
        );
    }

    #[test]
    fn validates_numeric_ranges_and_rejects_boundary_violations() {
        let schemas = numeric_schemas();
        let valid = BTreeMap::from([
            ("intensity".to_string(), json!(10)),
            ("ratio".to_string(), json!(0.75)),
        ]);
        assert!(validate_skill_config("burst_meridian.beng_quan", valid, &schemas).is_ok());

        let invalid_int_step = BTreeMap::from([
            ("intensity".to_string(), json!(3)),
            ("ratio".to_string(), json!(0.75)),
        ]);
        assert_eq!(
            validate_skill_config("burst_meridian.beng_quan", invalid_int_step, &schemas)
                .unwrap_err(),
            SkillConfigRejectReason::InvalidFieldValue("intensity".to_string())
        );

        let overflowing_int = BTreeMap::from([
            ("intensity".to_string(), json!(i64::from(i32::MAX) + 1)),
            ("ratio".to_string(), json!(0.75)),
        ]);
        assert_eq!(
            validate_skill_config("burst_meridian.beng_quan", overflowing_int, &schemas)
                .unwrap_err(),
            SkillConfigRejectReason::InvalidFieldValue("intensity".to_string())
        );

        let invalid_float_step = BTreeMap::from([
            ("intensity".to_string(), json!(8)),
            ("ratio".to_string(), json!(0.3)),
        ]);
        assert_eq!(
            validate_skill_config("burst_meridian.beng_quan", invalid_float_step, &schemas)
                .unwrap_err(),
            SkillConfigRejectReason::InvalidFieldValue("ratio".to_string())
        );

        let out_of_range_float = BTreeMap::from([
            ("intensity".to_string(), json!(8)),
            ("ratio".to_string(), json!(1.25)),
        ]);
        assert_eq!(
            validate_skill_config("burst_meridian.beng_quan", out_of_range_float, &schemas)
                .unwrap_err(),
            SkillConfigRejectReason::InvalidFieldValue("ratio".to_string())
        );
    }

    #[test]
    fn handle_intent_saves_clears_and_rejects_casting_same_skill() {
        let schemas = test_schemas();
        let mut store = SkillConfigStore::default();
        let mut config = BTreeMap::new();
        config.insert("stance".to_string(), json!("short"));
        config.insert("meridian_id".to_string(), json!("Pericardium"));

        let snapshot = handle_config_intent(
            "offline:Azure",
            "burst_meridian.beng_quan",
            config,
            None,
            &mut store,
            &schemas,
        )
        .unwrap();
        assert!(snapshot.configs.contains_key("burst_meridian.beng_quan"));
        assert!(skill_config_snapshot_for_cast(
            Some(&store),
            "offline:Azure",
            "burst_meridian.beng_quan"
        )
        .is_some());

        let casting = Casting {
            source: crate::combat::components::CastSource::SkillBar,
            slot: 0,
            started_at_tick: 0,
            duration_ticks: 1,
            started_at_ms: 0,
            duration_ms: 50,
            bound_instance_id: None,
            start_position: valence::prelude::DVec3::ZERO,
            complete_cooldown_ticks: 1,
            skill_id: Some("burst_meridian.beng_quan".to_string()),
            skill_config: None,
        };
        assert_eq!(
            handle_config_intent(
                "offline:Azure",
                "burst_meridian.beng_quan",
                BTreeMap::from([("stance".to_string(), json!("long"))]),
                Some(&casting),
                &mut store,
                &schemas,
            )
            .unwrap_err(),
            SkillConfigRejectReason::CurrentlyCasting
        );

        let snapshot = handle_config_intent(
            "offline:Azure",
            "burst_meridian.beng_quan",
            BTreeMap::new(),
            None,
            &mut store,
            &schemas,
        )
        .unwrap();
        assert!(!snapshot.configs.contains_key("burst_meridian.beng_quan"));
    }

    #[test]
    fn default_registry_contains_zhenmai_sever_chain_fixture() {
        let schemas = SkillConfigSchemas::default();
        let mut config = BTreeMap::new();
        config.insert("meridian_id".to_string(), json!("Pericardium"));
        config.insert("backfire_kind".to_string(), json!("tainted_yuan"));

        let validated = validate_skill_config("zhenmai.sever_chain", config, &schemas).unwrap();

        assert_eq!(
            validated.fields.get("meridian_id"),
            Some(&json!("Pericardium"))
        );
        assert_eq!(
            validated.fields.get("backfire_kind"),
            Some(&json!("tainted_yuan"))
        );
    }

    #[test]
    fn zhenmai_sever_chain_intent_saves_with_default_registry() {
        let schemas = SkillConfigSchemas::default();
        let mut store = SkillConfigStore::default();
        let snapshot = handle_config_intent(
            "offline:Azure",
            "zhenmai.sever_chain",
            BTreeMap::from([
                ("meridian_id".to_string(), json!("Pericardium")),
                ("backfire_kind".to_string(), json!("array")),
            ]),
            None,
            &mut store,
            &schemas,
        )
        .unwrap();

        assert!(snapshot.configs.contains_key("zhenmai.sever_chain"));
    }
}
