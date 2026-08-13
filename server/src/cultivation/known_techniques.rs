//! 功法元数据运行时注册表。
//!
//! `assets/cultivation/techniques.toml` 是玩家功法 metadata 的唯一真源；resolver
//! 函数指针仍留在 [`crate::cultivation::skill_registry::SkillRegistry`]。本模块刻意不提供
//! 零参或 `'static` 查询门面：ECS 系统应注入 `Res<TechniqueRegistry>`，纯函数应显式借用
//! `&TechniqueRegistry`，避免把启动期数据泄漏成全局第二真源。

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component, Resource};

use crate::body_plan::{RaceGateOwned, RaceRegistry};
use crate::cultivation::components::Realm;
use crate::cultivation::meridian::severed::SkillMeridianDependencies;
use crate::cultivation::skill_registry::SkillRegistry;
use crate::qi_physics::constants::QI_EPSILON;

/// 相对 server assets 根目录的功法 metadata 文件。
pub const DEFAULT_TECHNIQUES_PATH: &str = "assets/cultivation/techniques.toml";

/// 玩家已学功法的持久化切片。该 JSON 形状是存档契约，不能随 metadata 数据化改变。
#[derive(Debug, Clone, Component, Serialize, Deserialize, PartialEq, Default)]
pub struct KnownTechniques {
    pub entries: Vec<KnownTechnique>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KnownTechnique {
    pub id: String,
    pub proficiency: f32,
    pub active: bool,
}

/// 写保护标记：join 时功法持久化状态无法可靠读取——行存在但读取/解析失败
/// （sqlite 错误或 JSON 损坏），或连接打不开导致行状态完全不可知。
/// 挂上后所有 `KnownTechniques` 落盘路径（Changed flush / 断线保存 / 停服 flush）
/// 都必须跳过该玩家，防止会话内的 default 空表把可能存在的真实存档覆盖清零；
/// 组件只活在本次会话的实体上，下次重连成功加载即自然恢复正常持久化。
#[derive(Debug, Component)]
pub struct KnownTechniquesLoadFailed;

impl KnownTechniques {
    /// 开发命令的“授予全部”集合。顺序严格复用数据文件的声明顺序，保证 NPC deterministic
    /// selection、命令展示和 dev grants 不会因数据化而重排。
    pub fn dev_default(registry: &TechniqueRegistry) -> Self {
        Self {
            entries: registry
                .iter()
                .map(|definition| KnownTechnique {
                    id: definition.id.clone(),
                    proficiency: 0.5,
                    active: true,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillCategory {
    Attack,
    Heal,
    Buff,
    Control,
    Defense,
}

/// metadata 与执行入口的接线分类。`MetadataBacked` 由 `SkillRegistry` resolver
/// 执行；`DirectGeneric` 走通用 skill-bar cast 生命周期且必须登记自然完成消费者；
/// `DedicatedInput` 由独立 C2S intent 驱动，禁止绑定或施放到 skill bar。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TechniqueDispatch {
    MetadataBacked,
    DirectGeneric,
    DedicatedInput,
}

/// 运行时 owned metadata。所有字符串与经脉列表均来自启动期 TOML，不能借用临时解析缓冲区。
#[derive(Debug, Clone, PartialEq)]
pub struct TechniqueDefinition {
    pub id: String,
    pub display_name: String,
    pub grade: String,
    pub description: String,
    /// 已在加载期验证为六境界之一；保留 string 以维持既有 payload 语义。
    pub required_realm: String,
    pub required_meridians: Vec<TechniqueRequiredMeridian>,
    pub required_race: RaceGateOwned,
    pub qi_cost: f32,
    pub stamina_cost: f32,
    pub cast_ticks: u32,
    pub cooldown_ticks: u32,
    pub range: f32,
    pub icon_texture: String,
    pub category: SkillCategory,
    pub dispatch: TechniqueDispatch,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TechniqueRequiredMeridian {
    pub channel: String,
    pub min_health: f32,
}

impl TechniqueDefinition {
    /// 加载期已验证为六境界之一；运行时消费者复用同一 parser，避免另写 match 第二真源。
    pub fn required_realm_value(&self) -> Realm {
        parse_required_realm(&self.required_realm)
            .expect("validated TechniqueDefinition must contain a known required_realm")
    }
}

/// 有序功法 catalog。`definitions` 的顺序就是 TOML 内 `[[techniques]]` 的声明顺序；
/// `id_to_index` 只用于 O(1) 查询，不能替代或重排该 Vec。
#[derive(Debug, Clone, PartialEq)]
pub struct TechniqueRegistry {
    definitions: Vec<TechniqueDefinition>,
    id_to_index: HashMap<String, usize>,
}

impl Resource for TechniqueRegistry {}

impl TechniqueRegistry {
    #[cfg(test)]
    pub fn load_for_tests() -> Self {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_TECHNIQUES_PATH);
        Self::load_from_path(path, &RaceRegistry::default())
            .expect("checked-in technique catalog must load")
    }

    #[cfg(test)]
    pub(crate) fn load_for_tests_with_override(
        id: &str,
        override_definition: impl FnOnce(&mut TechniqueDefinition),
    ) -> Self {
        let mut registry = Self::load_for_tests();
        let index = *registry
            .id_to_index
            .get(id)
            .unwrap_or_else(|| panic!("test override references unknown technique {id:?}"));
        override_definition(&mut registry.definitions[index]);
        registry
    }

    pub fn get(&self, id: &str) -> Option<&TechniqueDefinition> {
        self.id_to_index
            .get(id)
            .and_then(|index| self.definitions.get(*index))
    }

    pub fn iter(&self) -> impl Iterator<Item = &TechniqueDefinition> {
        self.definitions.iter()
    }

    pub fn definitions(&self) -> &[TechniqueDefinition] {
        &self.definitions
    }

    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }

    /// 读取并验证任意 techniques TOML。读取、反序列化、跨表验证完成前不会构造任何可见
    /// registry，因此失败路径不可能向调用方泄漏部分内容。
    pub fn load_from_path(
        path: impl AsRef<Path>,
        races: &RaceRegistry,
    ) -> Result<Self, TechniqueLoadError> {
        let path = path.as_ref();
        let source = fs::read_to_string(path).map_err(|source| TechniqueLoadError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Self::from_toml_contents(path, &source, races)
    }

    /// 从部署期 assets 根加载生产 catalog。
    pub fn load_default(races: &RaceRegistry) -> Result<Self, TechniqueLoadError> {
        let path = crate::body_plan::resolve_assets_root().join(DEFAULT_TECHNIQUES_PATH);
        Self::load_from_path(path, races)
    }

    fn from_toml_contents(
        path: &Path,
        source: &str,
        races: &RaceRegistry,
    ) -> Result<Self, TechniqueLoadError> {
        let parsed: TechniqueFile =
            toml::from_str(source).map_err(|source| TechniqueLoadError::Toml {
                path: path.to_path_buf(),
                source,
            })?;

        if parsed.techniques.is_empty() {
            return Err(TechniqueLoadError::invalid(
                path,
                None,
                "techniques must not be empty",
            ));
        }

        let mut seen_ids = HashSet::new();
        let mut definitions = Vec::with_capacity(parsed.techniques.len());
        for raw in parsed.techniques {
            if !seen_ids.insert(raw.id.clone()) {
                return Err(TechniqueLoadError::invalid(
                    path,
                    Some(raw.id),
                    "duplicate technique id",
                ));
            }
            definitions.push(validate_and_convert(path, raw, races)?);
        }

        let id_to_index = definitions
            .iter()
            .enumerate()
            .map(|(index, definition)| (definition.id.clone(), index))
            .collect();
        Ok(Self {
            definitions,
            id_to_index,
        })
    }
}

#[derive(Debug)]
pub enum TechniqueLoadError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Toml {
        path: PathBuf,
        source: toml::de::Error,
    },
    Invalid {
        path: PathBuf,
        technique_id: Option<String>,
        reason: String,
    },
}

impl TechniqueLoadError {
    fn invalid(path: &Path, technique_id: Option<String>, reason: impl Into<String>) -> Self {
        Self::Invalid {
            path: path.to_path_buf(),
            technique_id,
            reason: reason.into(),
        }
    }
}

impl std::fmt::Display for TechniqueLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(
                    f,
                    "failed to read technique catalog {}: {source}",
                    path.display()
                )
            }
            Self::Toml { path, source } => {
                write!(f, "invalid technique TOML {}: {source}", path.display())
            }
            Self::Invalid {
                path,
                technique_id,
                reason,
            } => match technique_id {
                Some(id) => write!(
                    f,
                    "invalid technique catalog {} entry {id:?}: {reason}",
                    path.display()
                ),
                None => write!(f, "invalid technique catalog {}: {reason}", path.display()),
            },
        }
    }
}

impl std::error::Error for TechniqueLoadError {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TechniqueFile {
    techniques: Vec<TechniqueToml>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TechniqueToml {
    id: String,
    display_name: String,
    grade: String,
    description: String,
    required_realm: String,
    required_meridians: Vec<TechniqueRequiredMeridianToml>,
    required_race: RaceGateOwned,
    qi_cost: f32,
    stamina_cost: f32,
    cast_ticks: u32,
    cooldown_ticks: u32,
    range: f32,
    icon_texture: String,
    category: SkillCategory,
    dispatch: TechniqueDispatch,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TechniqueRequiredMeridianToml {
    channel: String,
    min_health: f32,
}

fn validate_and_convert(
    path: &Path,
    raw: TechniqueToml,
    races: &RaceRegistry,
) -> Result<TechniqueDefinition, TechniqueLoadError> {
    let technique_id = raw.id.clone();
    for (field, value) in [
        ("id", raw.id.as_str()),
        ("display_name", raw.display_name.as_str()),
        ("grade", raw.grade.as_str()),
        ("description", raw.description.as_str()),
        ("required_realm", raw.required_realm.as_str()),
        ("icon_texture", raw.icon_texture.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(TechniqueLoadError::invalid(
                path,
                Some(technique_id.clone()),
                format!("{field} must not be empty"),
            ));
        }
    }

    if parse_required_realm(&raw.required_realm).is_none() {
        return Err(TechniqueLoadError::invalid(
            path,
            Some(technique_id.clone()),
            format!("unknown required_realm {:?}", raw.required_realm),
        ));
    }
    if !matches!(
        raw.grade.as_str(),
        "common" | "yellow" | "profound" | "earth" | "rare"
    ) {
        return Err(TechniqueLoadError::invalid(
            path,
            Some(technique_id.clone()),
            format!("unknown grade {:?}", raw.grade),
        ));
    }

    for (field, value) in [
        ("qi_cost", raw.qi_cost),
        ("stamina_cost", raw.stamina_cost),
        ("range", raw.range),
    ] {
        if !value.is_finite() || value < 0.0 {
            return Err(TechniqueLoadError::invalid(
                path,
                Some(technique_id.clone()),
                format!("{field} must be finite and non-negative, got {value}"),
            ));
        }
    }
    if raw.qi_cost != 0.0 && f64::from(raw.qi_cost) <= QI_EPSILON {
        return Err(TechniqueLoadError::invalid(
            path,
            Some(technique_id.clone()),
            format!(
                "qi_cost must be exactly zero or greater than QI_EPSILON ({QI_EPSILON}), got {}",
                raw.qi_cost
            ),
        ));
    }

    let mut seen_meridians = HashSet::new();
    let mut required_meridians = Vec::with_capacity(raw.required_meridians.len());
    for meridian in raw.required_meridians {
        if meridian.channel.trim().is_empty() {
            return Err(TechniqueLoadError::invalid(
                path,
                Some(technique_id.clone()),
                "required_meridians[].channel must not be empty",
            ));
        }
        let Some(parsed_channel) =
            crate::cultivation::technique_scroll::parse_meridian_id(&meridian.channel)
        else {
            return Err(TechniqueLoadError::invalid(
                path,
                Some(technique_id.clone()),
                format!("unknown required meridian {:?}", meridian.channel),
            ));
        };
        if !seen_meridians.insert(parsed_channel) {
            return Err(TechniqueLoadError::invalid(
                path,
                Some(technique_id.clone()),
                format!("duplicate required meridian {:?}", meridian.channel),
            ));
        }
        if !meridian.min_health.is_finite()
            || meridian.min_health <= 0.0
            || meridian.min_health > 1.0
        {
            return Err(TechniqueLoadError::invalid(
                path,
                Some(technique_id.clone()),
                format!(
                    "required meridian {:?} min_health must be finite and in (0, 1], got {}",
                    meridian.channel, meridian.min_health
                ),
            ));
        }
        required_meridians.push(TechniqueRequiredMeridian {
            channel: meridian.channel,
            min_health: meridian.min_health,
        });
    }

    validate_race_gate(path, &technique_id, &raw.required_race, races)?;

    Ok(TechniqueDefinition {
        id: raw.id,
        display_name: raw.display_name,
        grade: raw.grade,
        description: raw.description,
        required_realm: raw.required_realm,
        required_meridians,
        required_race: raw.required_race,
        qi_cost: raw.qi_cost,
        stamina_cost: raw.stamina_cost,
        cast_ticks: raw.cast_ticks,
        cooldown_ticks: raw.cooldown_ticks,
        range: raw.range,
        icon_texture: raw.icon_texture,
        category: raw.category,
        dispatch: raw.dispatch,
    })
}

fn validate_race_gate(
    path: &Path,
    technique_id: &str,
    gate: &RaceGateOwned,
    races: &RaceRegistry,
) -> Result<(), TechniqueLoadError> {
    let RaceGateOwned::Species { species } = gate else {
        return Ok(());
    };
    if species.is_empty() {
        return Err(TechniqueLoadError::invalid(
            path,
            Some(technique_id.to_string()),
            "species race gate must list at least one race",
        ));
    }
    let mut seen = HashSet::new();
    for race in species {
        if !seen.insert(race.clone()) {
            return Err(TechniqueLoadError::invalid(
                path,
                Some(technique_id.to_string()),
                format!("species race gate repeats race {:?}", race.as_str()),
            ));
        }
        if races.get(race).is_none() {
            return Err(TechniqueLoadError::invalid(
                path,
                Some(technique_id.to_string()),
                format!(
                    "species race gate references unknown race {:?}",
                    race.as_str()
                ),
            ));
        }
    }
    Ok(())
}

/// 单一、可复用的 realm string parser。loader 用它做启动期校验；运行时学习路径复用同一
/// 映射，避免 TOML 通过而运行时才因 string 漂移拒绝。
pub fn parse_required_realm(raw: &str) -> Option<Realm> {
    match raw {
        "Awaken" => Some(Realm::Awaken),
        "Induce" => Some(Realm::Induce),
        "Condense" => Some(Realm::Condense),
        "Solidify" => Some(Realm::Solidify),
        "Spirit" => Some(Realm::Spirit),
        "Void" => Some(Realm::Void),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TechniqueWiringError(String);

impl std::fmt::Display for TechniqueWiringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for TechniqueWiringError {}

/// 验证当前 metadata、resolver 与经脉依赖表的逐条关系。只有三份候选都完整构造后才可
/// 调用；调用者必须在成功后才把它们 insert 为 Bevy resources。resolver-only 与
/// dependency-only 条目有意合法，不强迫非 metadata 内容反向补表。
pub fn validate_startup_wiring(
    techniques: &TechniqueRegistry,
    skills: &SkillRegistry,
    dependencies: &SkillMeridianDependencies,
) -> Result<(), TechniqueWiringError> {
    for definition in techniques.iter() {
        match definition.dispatch {
            TechniqueDispatch::MetadataBacked => {
                if skills.lookup(&definition.id).is_none() {
                    return Err(TechniqueWiringError(format!(
                        "metadata_backed technique {:?} has no SkillRegistry resolver",
                        definition.id
                    )));
                }
                if !dependencies.is_declared(&definition.id) {
                    return Err(TechniqueWiringError(format!(
                        "metadata_backed technique {:?} lacks an explicit meridian dependency declaration",
                        definition.id
                    )));
                }
            }
            TechniqueDispatch::DirectGeneric => {
                if skills.lookup(&definition.id).is_some() {
                    return Err(TechniqueWiringError(format!(
                        "direct_generic technique {:?} unexpectedly has a SkillRegistry resolver",
                        definition.id
                    )));
                }
                if !crate::network::cast_emit::has_direct_generic_completion_consumer(
                    &definition.id,
                ) {
                    return Err(TechniqueWiringError(format!(
                        "direct_generic technique {:?} has no registered completion consumer",
                        definition.id
                    )));
                }
            }
            TechniqueDispatch::DedicatedInput => {
                if skills.lookup(&definition.id).is_some() {
                    return Err(TechniqueWiringError(format!(
                        "dedicated_input technique {:?} unexpectedly has a SkillRegistry resolver",
                        definition.id
                    )));
                }
                if crate::network::cast_emit::has_direct_generic_completion_consumer(&definition.id)
                {
                    return Err(TechniqueWiringError(format!(
                        "dedicated_input technique {:?} unexpectedly has a generic completion consumer",
                        definition.id
                    )));
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
#[path = "known_techniques_legacy_oracle.rs"]
mod legacy_oracle;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body_plan::{RaceGate, RaceId};
    use legacy_oracle::{LEGACY_TECHNIQUE_DEFINITIONS, LEGACY_TECHNIQUE_IDS};

    fn production_registry() -> TechniqueRegistry {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_TECHNIQUES_PATH);
        TechniqueRegistry::load_from_path(path, &RaceRegistry::default())
            .expect("checked-in techniques.toml must load with Any/Humanoid gates")
    }

    fn load(text: &str) -> Result<TechniqueRegistry, TechniqueLoadError> {
        TechniqueRegistry::from_toml_contents(
            Path::new("test-techniques.toml"),
            text,
            &RaceRegistry::default(),
        )
    }

    fn minimal_toml() -> String {
        r#"
[[techniques]]
id = "test.skill"
display_name = "测试"
grade = "common"
description = "测试功法"
required_realm = "Awaken"
required_meridians = []
required_race = { kind = "any" }
qi_cost = 0.0
stamina_cost = 0.0
cast_ticks = 0
cooldown_ticks = 0
range = 0.0
icon_texture = "bong-client:textures/gui/items/skill_scroll_test_skill.png"
category = "attack"
dispatch = "metadata_backed"
"#
        .to_string()
    }

    fn legacy_gate_to_owned(gate: RaceGate) -> RaceGateOwned {
        match gate {
            RaceGate::Any => RaceGateOwned::Any,
            RaceGate::Humanoid => RaceGateOwned::Humanoid,
            RaceGate::Species(species) => RaceGateOwned::Species {
                species: species.iter().map(|id| RaceId::new(*id)).collect(),
            },
        }
    }

    #[test]
    fn toml_preserves_legacy_entries_as_an_ordered_compatibility_subset() {
        let registry = production_registry();
        let legacy_ids = LEGACY_TECHNIQUE_IDS.into_iter().collect::<HashSet<_>>();
        let current_legacy_order = registry
            .iter()
            .map(|definition| definition.id.as_str())
            .filter(|id| legacy_ids.contains(id))
            .collect::<Vec<_>>();
        assert_eq!(
            current_legacy_order, LEGACY_TECHNIQUE_IDS,
            "new metadata may be inserted, but historical entries must retain relative order"
        );

        let historical_dedicated_input = ["movement.dash", "shield_block"];
        for legacy in LEGACY_TECHNIQUE_DEFINITIONS {
            let actual = registry
                .get(legacy.id)
                .unwrap_or_else(|| panic!("legacy technique {:?} must remain present", legacy.id));
            assert_eq!(actual.id, legacy.id, "id mismatch for {}", legacy.id);
            assert_eq!(
                actual.display_name, legacy.display_name,
                "display_name mismatch for {}",
                legacy.id
            );
            assert_eq!(
                actual.grade, legacy.grade,
                "grade mismatch for {}",
                legacy.id
            );
            assert_eq!(
                actual.description, legacy.description,
                "description mismatch for {}",
                legacy.id
            );
            assert_eq!(
                actual.required_realm, legacy.required_realm,
                "realm mismatch for {}",
                legacy.id
            );
            assert_eq!(
                actual.required_race,
                legacy_gate_to_owned(legacy.required_race),
                "race gate mismatch for {}",
                legacy.id
            );
            assert_eq!(
                actual.qi_cost, legacy.qi_cost,
                "qi_cost mismatch for {}",
                legacy.id
            );
            assert_eq!(
                actual.stamina_cost, legacy.stamina_cost,
                "stamina_cost mismatch for {}",
                legacy.id
            );
            assert_eq!(
                actual.cast_ticks, legacy.cast_ticks,
                "cast_ticks mismatch for {}",
                legacy.id
            );
            assert_eq!(
                actual.cooldown_ticks, legacy.cooldown_ticks,
                "cooldown mismatch for {}",
                legacy.id
            );
            assert_eq!(
                actual.range, legacy.range,
                "range mismatch for {}",
                legacy.id
            );
            assert_eq!(
                actual.icon_texture, legacy.icon_texture,
                "icon mismatch for {}",
                legacy.id
            );
            assert_eq!(
                actual.category, legacy.category,
                "category mismatch for {}",
                legacy.id
            );
            assert_eq!(
                actual.required_meridians.len(),
                legacy.required_meridians.len(),
                "meridian count mismatch for {}",
                legacy.id
            );
            for (actual_meridian, legacy_meridian) in actual
                .required_meridians
                .iter()
                .zip(legacy.required_meridians.iter())
            {
                assert_eq!(
                    actual_meridian.channel, legacy_meridian.channel,
                    "meridian channel mismatch for {}",
                    legacy.id
                );
                assert_eq!(
                    actual_meridian.min_health, legacy_meridian.min_health,
                    "meridian min_health mismatch for {}",
                    legacy.id
                );
            }
            let expected_dispatch = if historical_dedicated_input.contains(&legacy.id) {
                TechniqueDispatch::DedicatedInput
            } else if legacy.id == "body.guangbo_ticao" {
                TechniqueDispatch::DirectGeneric
            } else {
                TechniqueDispatch::MetadataBacked
            };
            assert_eq!(
                actual.dispatch, expected_dispatch,
                "historical dispatch mismatch for {}",
                legacy.id
            );
        }
    }

    #[test]
    fn dev_default_uses_registry_order_without_changing_persistence_shape() {
        let registry = production_registry();
        let known = KnownTechniques::dev_default(&registry);
        assert_eq!(known.entries.len(), registry.len());
        assert_eq!(
            known
                .entries
                .iter()
                .map(|entry| entry.id.as_str())
                .collect::<Vec<_>>(),
            registry
                .iter()
                .map(|definition| definition.id.as_str())
                .collect::<Vec<_>>()
        );
        assert!(known
            .entries
            .iter()
            .all(|entry| { entry.active && (entry.proficiency - 0.5).abs() <= f32::EPSILON }));
    }

    #[test]
    fn category_and_realm_variants_are_all_exercised_by_checked_in_catalog() {
        let registry = production_registry();
        let categories: HashSet<SkillCategory> = registry
            .iter()
            .map(|definition| definition.category)
            .collect();
        assert_eq!(
            categories,
            HashSet::from([
                SkillCategory::Attack,
                SkillCategory::Heal,
                SkillCategory::Buff,
                SkillCategory::Control,
                SkillCategory::Defense,
            ])
        );
        for realm in ["Awaken", "Induce", "Condense", "Solidify", "Spirit", "Void"] {
            assert!(
                registry
                    .iter()
                    .any(|definition| definition.required_realm == realm),
                "checked-in catalog must exercise realm variant {realm}"
            );
        }
    }

    #[test]
    fn rejects_unknown_top_level_or_entry_fields_without_partial_registry() {
        let top_level = format!("unknown = true\n{}", minimal_toml());
        let entry = format!("{}unexpected = true\n", minimal_toml());
        for text in [&top_level, &entry] {
            let error = load(text).expect_err("unknown field must reject catalog atomically");
            assert!(format!("{error}").contains("test-techniques.toml"));
        }
    }

    #[test]
    fn rejects_malformed_empty_and_missing_required_fields() {
        let malformed = "[[techniques]\nid = \"missing quote";
        let empty = "techniques = []";
        let missing = "[[techniques]]\nid = \"only-id\"";
        assert!(load(malformed).is_err(), "malformed TOML must fail");
        assert!(load(empty).is_err(), "empty technique catalog must fail");
        assert!(
            load(missing).is_err(),
            "missing required metadata fields must fail"
        );
    }

    #[test]
    fn rejects_duplicate_id_unknown_enums_and_invalid_dispatch() {
        let duplicate = format!("{}\n{}", minimal_toml(), minimal_toml());
        let invalid_realm = minimal_toml().replace(
            "required_realm = \"Awaken\"",
            "required_realm = \"Ancient\"",
        );
        let invalid_grade = minimal_toml().replace("grade = \"common\"", "grade = \"legendary\"");
        let invalid_category =
            minimal_toml().replace("category = \"attack\"", "category = \"mystery\"");
        let invalid_dispatch =
            minimal_toml().replace("dispatch = \"metadata_backed\"", "dispatch = \"other\"");
        for text in [
            duplicate,
            invalid_realm,
            invalid_grade,
            invalid_category,
            invalid_dispatch,
        ] {
            assert!(
                load(&text).is_err(),
                "invalid catalog input must reject: {text}"
            );
        }
    }

    #[test]
    fn rejects_invalid_numbers_and_bad_meridian_references() {
        let negative_cost = minimal_toml().replace("qi_cost = 0.0", "qi_cost = -0.1");
        let nan_range = minimal_toml().replace("range = 0.0", "range = nan");
        let zero_health = minimal_toml().replace(
            "required_meridians = []",
            "required_meridians = [{ channel = \"Lung\", min_health = 0.0 }]",
        );
        let over_health = minimal_toml().replace(
            "required_meridians = []",
            "required_meridians = [{ channel = \"Lung\", min_health = 1.1 }]",
        );
        let unknown_meridian = minimal_toml().replace(
            "required_meridians = []",
            "required_meridians = [{ channel = \"Imaginary\", min_health = 0.1 }]",
        );
        let duplicate_meridian = minimal_toml().replace(
            "required_meridians = []",
            "required_meridians = [{ channel = \"Lung\", min_health = 0.1 }, { channel = \"Lung\", min_health = 0.2 }]",
        );
        for text in [
            negative_cost,
            nan_range,
            zero_health,
            over_health,
            unknown_meridian,
            duplicate_meridian,
        ] {
            assert!(
                load(&text).is_err(),
                "invalid numeric/reference input must reject: {text}"
            );
        }
    }

    #[test]
    fn rejects_species_gates_that_are_empty_duplicate_or_unknown() {
        let empty = minimal_toml().replace(
            "required_race = { kind = \"any\" }",
            "required_race = { kind = \"species\", species = [] }",
        );
        let duplicate = minimal_toml().replace(
            "required_race = { kind = \"any\" }",
            "required_race = { kind = \"species\", species = [\"human\", \"human\"] }",
        );
        let unknown = minimal_toml().replace(
            "required_race = { kind = \"any\" }",
            "required_race = { kind = \"species\", species = [\"unknown\"] }",
        );
        for text in [empty, duplicate, unknown] {
            assert!(
                load(&text).is_err(),
                "invalid species gate must reject: {text}"
            );
        }
    }

    fn noop_skill(
        _world: &mut bevy_ecs::world::World,
        _caster: bevy_ecs::entity::Entity,
        _slot: u8,
        _target: Option<bevy_ecs::entity::Entity>,
    ) -> crate::cultivation::skill_registry::CastResult {
        crate::cultivation::skill_registry::CastResult::Interrupted
    }

    #[test]
    fn qi_cost_accepts_only_exact_zero_or_values_above_epsilon() {
        let zero = load(&minimal_toml()).expect("exact zero is a legal free technique");
        assert_eq!(zero.get("test.skill").unwrap().qi_cost, 0.0);

        let at = QI_EPSILON as f32;
        let below = f32::from_bits(at.to_bits() - 1);
        let above = f32::from_bits(at.to_bits() + 1);
        for (label, value) in [("below", below), ("at", at)] {
            let error = load(&minimal_toml().replace(
                "qi_cost = 0.0",
                &format!("qi_cost = {value}"),
            ))
            .expect_err("positive qi dust that settlement would omit must be rejected");
            assert!(
                error.to_string().contains("exactly zero or greater"),
                "{label} threshold rejection should explain the atomic qi contract: {error}"
            );
        }
        let accepted = load(&minimal_toml().replace(
            "qi_cost = 0.0",
            &format!("qi_cost = {above}"),
        ))
        .expect("the first representable f32 above QI_EPSILON must load");
        assert!(f64::from(accepted.get("test.skill").unwrap().qi_cost) > QI_EPSILON);
    }

    #[test]
    fn consumerless_direct_generic_is_rejected_but_dedicated_input_is_admitted() {
        let direct_generic = load(&minimal_toml().replace(
            "dispatch = \"metadata_backed\"",
            "dispatch = \"direct_generic\"",
        ))
        .expect("dispatch syntax is valid before cross-registry wiring validation");
        let error = validate_startup_wiring(
            &direct_generic,
            &SkillRegistry::default(),
            &SkillMeridianDependencies::default(),
        )
        .expect_err("consumerless direct_generic must fail startup");
        assert!(error.to_string().contains("test.skill"));
        assert!(error.to_string().contains("no registered completion consumer"));

        let dedicated_input = load(&minimal_toml().replace(
            "dispatch = \"metadata_backed\"",
            "dispatch = \"dedicated_input\"",
        ))
        .expect("dedicated input metadata loads");
        validate_startup_wiring(
            &dedicated_input,
            &SkillRegistry::default(),
            &SkillMeridianDependencies::default(),
        )
        .expect("dedicated input is intentionally outside the generic cast lifecycle");
    }

    #[test]
    fn metadata_backed_accepts_data_only_metadata_when_runtime_wiring_exists() {
        let registry = load(&minimal_toml()).expect("minimal metadata loads");
        let mut skills = SkillRegistry::default();
        skills.register("test.skill", noop_skill);
        skills.register("resolver.only", noop_skill);
        let mut dependencies = SkillMeridianDependencies::default();
        dependencies.declare("test.skill", Vec::new());
        dependencies.declare("dependency.only", Vec::new());

        validate_startup_wiring(&registry, &skills, &dependencies).expect(
            "existing resolver plus explicit empty dependency must admit metadata without Rust allowlists",
        );
    }

    #[test]
    fn startup_wiring_rejects_each_metadata_relationship_violation_with_the_id() {
        let metadata_backed = load(&minimal_toml()).expect("minimal metadata loads");
        let no_skills = SkillRegistry::default();
        let mut declared = SkillMeridianDependencies::default();
        declared.declare("test.skill", Vec::new());
        let missing_resolver = validate_startup_wiring(&metadata_backed, &no_skills, &declared)
            .expect_err("metadata_backed without resolver must fail");
        assert!(missing_resolver.to_string().contains("test.skill"));
        assert!(missing_resolver
            .to_string()
            .contains("no SkillRegistry resolver"));

        let mut skills = SkillRegistry::default();
        skills.register("test.skill", noop_skill);
        let missing_dependency = validate_startup_wiring(
            &metadata_backed,
            &skills,
            &SkillMeridianDependencies::default(),
        )
        .expect_err("metadata_backed without an explicit dependency declaration must fail");
        assert!(missing_dependency.to_string().contains("test.skill"));
        assert!(missing_dependency
            .to_string()
            .contains("explicit meridian dependency declaration"));

        let direct_generic = load(&minimal_toml().replace(
            "dispatch = \"metadata_backed\"",
            "dispatch = \"direct_generic\"",
        ))
        .expect("direct_generic metadata loads");
        let resolver_conflict = validate_startup_wiring(
            &direct_generic,
            &skills,
            &SkillMeridianDependencies::default(),
        )
        .expect_err("direct_generic with a resolver must fail");
        assert!(resolver_conflict.to_string().contains("test.skill"));
        assert!(resolver_conflict
            .to_string()
            .contains("unexpectedly has a SkillRegistry resolver"));
    }

    #[test]
    fn checked_in_production_wiring_satisfies_dynamic_relationships() {
        let techniques = production_registry();
        let skills = crate::cultivation::skill_registry::init_registry();
        let dependencies = crate::cultivation::skill_registry::init_meridian_dependencies();

        validate_startup_wiring(&techniques, &skills, &dependencies)
            .expect("checked-in metadata, resolvers, and dependencies must satisfy startup wiring");
    }

    #[test]
    fn u32_max_cooldown_remains_a_valid_one_shot_sentinel() {
        let input = minimal_toml()
            .replace("id = \"test.skill\"", "id = \"sword_path.heaven_gate\"")
            .replace("cooldown_ticks = 0", "cooldown_ticks = 4294967295");
        assert_eq!(
            load(&input)
                .unwrap()
                .get("sword_path.heaven_gate")
                .unwrap()
                .cooldown_ticks,
            u32::MAX
        );
    }
}
