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

/// metadata 与 resolver 的接线分类。`MetadataBacked` 由 `SkillRegistry` resolver
/// 执行；`DirectGeneric` 则走通用 skill-bar cast 生命周期。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TechniqueDispatch {
    MetadataBacked,
    DirectGeneric,
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
    pub qi_cost: f64,
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

    #[cfg(test)]
    pub(crate) fn load_for_tests_with_definition(definition: TechniqueDefinition) -> Self {
        let mut registry = Self::load_for_tests();
        assert!(
            !registry.id_to_index.contains_key(&definition.id),
            "test extension duplicates technique {:?}",
            definition.id
        );
        let index = registry.definitions.len();
        registry.id_to_index.insert(definition.id.clone(), index);
        registry.definitions.push(definition);
        registry
    }

    /// 从任意 TOML 文本构造 registry（仅测试用），让其他模块的测试能直接 pin loader
    /// 的拒绝边界。
    #[cfg(test)]
    pub(crate) fn load_from_contents_for_tests(text: &str) -> Result<Self, TechniqueLoadError> {
        Self::from_toml_contents(
            Path::new("test-techniques.toml"),
            text,
            &RaceRegistry::default(),
        )
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

    /// 最坏情况下（玩家学会全部 catalog 条目）techniques_snapshot 聚合 payload 的保守
    /// 字节上界。每个 JSON 字符串按控制字符的 `\\u00XX` 六字节逃逸计算，再加结构开销，
    /// 与 wire 上限 `MAX_PAYLOAD_BYTES` 比对。编码检查在发送端逐玩家逐 tick 执行，
    /// 启动期先做一次聚合门禁（`validate_startup_wiring`），保证 catalog 被接受后，
    /// registry 文本不会让完整快照在发送端才被丢弃。
    ///
    /// 基底只取固定信封开销，不能按 `MAX_PAYLOAD_BYTES` 比例预留，否则小 catalog 会被
    /// 错误拒绝。wire 条目 `TechniqueEntryV1` 不携带 `icon_texture`，故不计入。
    pub fn aggregate_snapshot_size(&self) -> usize {
        const ENVELOPE_OVERHEAD: usize = 1024;
        const PER_ENTRY_OVERHEAD: usize = 192;
        const PROFICIENCY_LABEL_UPPER_BOUND: usize = 98;
        let mut total = ENVELOPE_OVERHEAD;
        for definition in &self.definitions {
            let text_bytes = [
                definition.id.as_str(),
                definition.display_name.as_str(),
                definition.grade.as_str(),
                definition.description.as_str(),
                definition.required_realm.as_str(),
            ]
            .into_iter()
            .map(json_string_upper_bound)
            .sum::<usize>()
                + PROFICIENCY_LABEL_UPPER_BOUND;
            let meridian_bytes: usize = definition
                .required_meridians
                .iter()
                .map(|m| json_string_upper_bound(&m.channel) + 16)
                .sum();
            total += text_bytes + meridian_bytes + PER_ENTRY_OVERHEAD;
        }
        total
    }

    fn json_string_upper_bound(value: &str) -> usize {
        2 + value
            .chars()
            .map(|character| match character {
                '"' | '\\' => 2,
                '\u{0000}'..='\u{001f}' => 6,
                _ => character.len_utf8(),
            })
            .sum::<usize>()
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
        // M18：catalog 总量同样受 `MAX_PAYLOAD_BYTES` 聚合边界约束。checked-in catalog
        // 是 49 条，512 条是约 10 倍余量——单条小成本 × 数千条也能撑爆 32 KiB 聚合
        // payload，因此先卡总量再在启动期量一次真实编码大小。
        const MAX_CATALOG_ENTRIES: usize = 512;
        if parsed.techniques.len() > MAX_CATALOG_ENTRIES {
            return Err(TechniqueLoadError::invalid(
                path,
                None,
                format!(
                    "techniques catalog must not exceed {MAX_CATALOG_ENTRIES} entries, got {}",
                    parsed.techniques.len()
                ),
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
    qi_cost: f64,
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
    // M18：description 会原样进 techniques_snapshot 的聚合 payload（上限
    // `MAX_PAYLOAD_BYTES = 32_768`）。单个字段不设上限时，一条超长 description 就能让
    // 已接受的 catalog 在发送端被 `PayloadBuildError::Oversize` 整包丢弃。这里按
    // UTF-8 字节数封顶（checked-in catalog 实测最长 41 字节，1024 留 25 倍余量）；
    // 截断语义违背"发送什么就是什么"的契约，因此拒绝而不是静默截断。catalog
    // 总数（`definitions` 声明上限）与聚合编码大小由启动期 `aggregate_snapshot_size`
    // 一并门禁。
    const MAX_DESCRIPTION_BYTES: usize = 1024;
    if raw.description.len() > MAX_DESCRIPTION_BYTES {
        return Err(TechniqueLoadError::invalid(
            path,
            Some(technique_id.clone()),
            format!(
                "description must not exceed {MAX_DESCRIPTION_BYTES} bytes (UTF-8), got {}",
                raw.description.len()
            ),
        ));
    }
    const MAX_DISPLAY_NAME_BYTES: usize = 256;
    if raw.display_name.len() > MAX_DISPLAY_NAME_BYTES {
        return Err(TechniqueLoadError::invalid(
            path,
            Some(technique_id.clone()),
            format!(
                "display_name must not exceed {MAX_DISPLAY_NAME_BYTES} bytes (UTF-8), got {}",
                raw.display_name.len()
            ),
        ));
    }
    const MAX_ID_BYTES: usize = 128;
    if raw.id.len() > MAX_ID_BYTES {
        return Err(TechniqueLoadError::invalid(
            path,
            Some(technique_id.clone()),
            format!(
                "id must not exceed {MAX_ID_BYTES} bytes (UTF-8), got {}",
                raw.id.len()
            ),
        ));
    }
    const MAX_ICON_TEXTURE_BYTES: usize = 512;
    if raw.icon_texture.len() > MAX_ICON_TEXTURE_BYTES {
        return Err(TechniqueLoadError::invalid(
            path,
            Some(technique_id.clone()),
            format!(
                "icon_texture must not exceed {MAX_ICON_TEXTURE_BYTES} bytes (UTF-8), got {}",
                raw.icon_texture.len()
            ),
        ));
    }

    if !is_valid_icon_texture(&raw.icon_texture) {
        return Err(TechniqueLoadError::invalid(
            path,
            Some(technique_id.clone()),
            format!(
                "icon_texture must be a Minecraft GUI PNG identifier, got {:?}",
                raw.icon_texture
            ),
        ));
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
        ("stamina_cost", f64::from(raw.stamina_cost)),
        ("range", f64::from(raw.range)),
    ] {
        if !value.is_finite() || value < 0.0 {
            return Err(TechniqueLoadError::invalid(
                path,
                Some(technique_id.clone()),
                format!("{field} must be finite and non-negative, got {value}"),
            ));
        }
    }
    if raw.id == "body.guangbo_ticao" && raw.qi_cost <= 0.0 {
        return Err(TechniqueLoadError::invalid(
            path,
            Some(technique_id.clone()),
            "body.guangbo_ticao qi_cost must be strictly positive",
        ));
    }
    // qi ledger quantum 边界：`release_qi_amount_to_zone` 对 `amount <= QI_EPSILON` 直接
    // 返回（不落 zone/overflow、不发 QiTransfer）。若接受 `0 < qi_cost <= QI_EPSILON`，
    // 消费方会先扣玩家真元再以同一金额回灌——release helper 提前返回造成单边扣减、
    // 真元永久销毁（M01 blocker）。因此任何非零 qi_cost 必须大于 ledger quantum；
    // `body.guangbo_ticao` 上方的 `<= 0.0` 特判保持零成本专属拒绝语义。
    if raw.qi_cost > 0.0 && raw.qi_cost <= crate::qi_physics::constants::QI_EPSILON {
        return Err(TechniqueLoadError::invalid(
            path,
            Some(technique_id.clone()),
            format!(
                "qi_cost must be zero or exceed the qi ledger quantum ({}), got {}",
                crate::qi_physics::constants::QI_EPSILON,
                raw.qi_cost
            ),
        ));
    }
    if raw.id == "sword_path.heaven_gate" && raw.range > 100.0 {
        return Err(TechniqueLoadError::invalid(
            path,
            Some(technique_id.clone()),
            "sword_path.heaven_gate range must not exceed 100 blocks",
        ));
    }
    // M30：`sword_path.resonance` 是全世界 entity fan-out（扫 Position+StatusEffects），
    // 无界 range 会让一次 cast 入队 O(world entities) 事件。给该招式加 operational
    // 上限（checked-in 6 格远低于 30 上限），与 heaven_gate 的 100 上限同一类防护。
    if raw.id == "sword_path.resonance" && raw.range > 30.0 {
        return Err(TechniqueLoadError::invalid(
            path,
            Some(technique_id.clone()),
            "sword_path.resonance range must not exceed 30 blocks (entity fan-out bound)",
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

fn is_valid_icon_texture(value: &str) -> bool {
    let Some((namespace, path)) = value.split_once(':') else {
        return false;
    };
    !namespace.is_empty()
        && !path.is_empty()
        && path.starts_with("textures/gui/")
        && path.ends_with(".png")
        && namespace
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '-' | '.'))
        && path.chars().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '-' | '/' | '.')
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
    // M18：catalog 被接受 ⇒ 学会全部条目的玩家必能收到完整快照。编码检查（发送端
    // `PayloadBuildError::Oversize`）是逐玩家逐 tick 的，启动期必须先行量一次最坏
    // 聚合大小，否则超限 catalog 会让快照在发送端被整包丢弃。
    let aggregate = techniques.aggregate_snapshot_size();
    if aggregate > crate::schema::common::MAX_PAYLOAD_BYTES {
        return Err(TechniqueWiringError(format!(
            "techniques catalog worst-case snapshot is ~{aggregate} bytes, exceeding MAX_PAYLOAD_BYTES = {}; \
             reduce catalog size or per-entry text before startup",
            crate::schema::common::MAX_PAYLOAD_BYTES
        )));
    }
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

        let historical_direct_generic = ["movement.dash", "shield_block", "body.guangbo_ticao"];
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
            assert!(
                (actual.qi_cost - f64::from(legacy.qi_cost)).abs() <= f64::from(f32::EPSILON),
                "qi_cost mismatch for {}: runtime={} legacy={}",
                legacy.id,
                actual.qi_cost,
                legacy.qi_cost
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
            let expected_dispatch = if historical_direct_generic.contains(&legacy.id) {
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
        // M10：序列化必须钉住完整 persistence shape（含 `active` key）——若把 active
        // 序列化成别的名字或省略，旧存档 JSON 就无法按既有契约加载，这里直接撞红。
        let json = serde_json::to_string(&known).expect("KnownTechniques must serialize");
        let first_entry_id = known
            .entries
            .first()
            .expect("dev_default grants the full catalog")
            .id
            .as_str();
        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("serialized JSON must parse");
        let entries = parsed
            .get("entries")
            .expect("persistence shape must keep top-level `entries` key");
        assert_eq!(
            entries
                .get(0)
                .and_then(|entry| entry.get("id"))
                .and_then(serde_json::Value::as_str),
            Some(first_entry_id),
            "persisted entries[0].id must match the registry-first technique"
        );
        assert_eq!(
            entries
                .get(0)
                .and_then(|entry| entry.get("active"))
                .and_then(serde_json::Value::as_bool),
            Some(true),
            "persistence shape must keep per-entry `active` key (M10)"
        );
        assert!(
            entries
                .get(0)
                .and_then(|entry| entry.get("proficiency"))
                .and_then(serde_json::Value::as_f64)
                .is_some(),
            "persistence shape must keep per-entry `proficiency` key"
        );
        // 反序列化 round-trip：序列化产物必须能被既有存档加载路径读回。
        let round_trip: KnownTechniques =
            serde_json::from_str(&json).expect("serialized shape must round-trip");
        assert_eq!(round_trip, known);
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
    fn rejects_blank_required_text_fields_per_field() {
        let blank_id = minimal_toml().replace("id = \"test.skill\"", "id = \"\"");
        let whitespace_id = minimal_toml().replace("id = \"test.skill\"", "id = \"  \"");
        let blank_display_name =
            minimal_toml().replace("display_name = \"测试\"", "display_name = \"\"");
        let whitespace_display_name =
            minimal_toml().replace("display_name = \"测试\"", "display_name = \" \t \"");
        let blank_description =
            minimal_toml().replace("description = \"测试功法\"", "description = \"\"");
        for text in [
            blank_id,
            whitespace_id,
            blank_display_name,
            whitespace_display_name,
            blank_description,
        ] {
            assert!(
                load(&text).is_err(),
                "blank or whitespace-only required text field must reject: {text}"
            );
        }
    }

    #[test]
    fn rejects_each_numeric_field_invalid_value_class_independently() {
        let negative_qi = minimal_toml().replace("qi_cost = 0.0", "qi_cost = -0.1");
        let nonfinite_qi = minimal_toml().replace("qi_cost = 0.0", "qi_cost = inf");
        let negative_stamina = minimal_toml().replace("stamina_cost = 0.0", "stamina_cost = -0.1");
        let nonfinite_stamina = minimal_toml().replace("stamina_cost = 0.0", "stamina_cost = nan");
        let negative_range = minimal_toml().replace("range = 0.0", "range = -1.0");
        let nonfinite_range = minimal_toml().replace("range = 0.0", "range = inf");
        let nonfinite_health = minimal_toml().replace(
            "required_meridians = []",
            "required_meridians = [{ channel = \"Lung\", min_health = nan }]",
        );
        for text in [
            negative_qi,
            nonfinite_qi,
            negative_stamina,
            nonfinite_stamina,
            negative_range,
            nonfinite_range,
            nonfinite_health,
        ] {
            assert!(
                load(&text).is_err(),
                "invalid numeric value class must reject independently: {text}"
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

    #[test]
    fn accepts_known_species_gate() {
        // `RaceRegistry::default()` 是空注册表——物种门对未知 race 必须拒绝，
        // 而"已知"需要真实注册表。这里构造带 human race 的注册表证明非空物种门
        // 能被接受（区别于 `rejects_species_gates_that_are_empty_duplicate_or_unknown`）。
        let input = minimal_toml().replace(
            "required_race = { kind = \"any\" }",
            "required_race = { kind = \"species\", species = [\"human\"] }",
        );
        let body_plans = crate::body_plan::BodyPlanRegistry::from_plans(vec![
            (*crate::body_plan::humanoid_plan_static()).clone(),
        ])
        .expect("humanoid plan must validate");
        let races = crate::body_plan::RaceRegistry::from_parts_for_test(
            vec![crate::body_plan::race_registry::RaceEntry {
                id: crate::body_plan::RaceId::new(crate::body_plan::HUMAN_RACE_ID),
                display_name: "人族".to_string(),
                body_plan_id: crate::body_plan::BodyPlanId::new("humanoid"),
                beast_kinds: vec![],
            }],
            vec![],
            &body_plans,
        )
        .expect("human-only test registry must validate");
        let registry = TechniqueRegistry::from_toml_contents(
            Path::new("test-techniques.toml"),
            &input,
            &races,
        )
        .expect("known non-empty species gate must load");
        let definition = registry.get("test.skill").unwrap();
        assert!(definition
            .required_race
            .allows(&RaceId::new("human"), false));
        assert!(!definition
            .required_race
            .allows(&RaceId::new("whale"), false));
    }

    #[test]
    fn rejects_invalid_gui_icon_identifiers() {
        for icon in [
            "missing_namespace.png",
            "Bong:textures/gui/skill.png",
            "bong:textures/item/skill.png",
            "bong:textures/gui/skill.jpg",
            "bong:textures/gui/Skill.png",
            "bong:textures/gui/skill icon.png",
        ] {
            let input = minimal_toml().replace(
                "bong-client:textures/gui/items/skill_scroll_test_skill.png",
                icon,
            );
            let error = load(&input).expect_err("invalid runtime icon must reject catalog");
            assert!(
                error.to_string().contains("icon_texture"),
                "error must identify icon_texture, got {error}"
            );
        }
    }

    #[test]
    fn rejects_sub_epsilon_qi_costs_but_accepts_zero_and_above_epsilon() {
        // M01 blocker 边界：`release_qi_amount_to_zone` 对 `amount <= QI_EPSILON` 直接
        // 返回，接受 `0 < qi_cost <= QI_EPSILON` 会造成扣玩家却不落目的账户的单边销毁。
        // 因此非零 qi_cost 必须大于 ledger quantum。
        let sub_epsilon = minimal_toml().replace("qi_cost = 0.0", "qi_cost = 0.0000005");
        let exactly_epsilon = minimal_toml().replace(
            "qi_cost = 0.0",
            &format!("qi_cost = {}", crate::qi_physics::constants::QI_EPSILON),
        );
        for text in [sub_epsilon, exactly_epsilon] {
            assert!(
                load(&text).is_err(),
                "non-zero qi_cost at or below the ledger quantum must reject: {text}"
            );
        }

        // 零成本仍合法（免费招，由各消费方决定 zero-cost 语义）；刚好超过 epsilon 合法。
        let zero_cost = load(&minimal_toml()).expect("zero qi_cost remains a valid catalog value");
        assert_eq!(zero_cost.get("test.skill").unwrap().qi_cost, 0.0);
        let above_epsilon = minimal_toml().replace(
            "qi_cost = 0.0",
            &format!(
                "qi_cost = {}",
                crate::qi_physics::constants::QI_EPSILON * 2.0
            ),
        );
        assert!(
            load(&above_epsilon).is_ok(),
            "qi_cost strictly above the ledger quantum must load"
        );
    }

    #[test]
    fn rejects_zero_cost_guangbo_and_oversized_heaven_gate() {
        let zero_cost_guangbo =
            minimal_toml().replace("id = \"test.skill\"", "id = \"body.guangbo_ticao\"");
        let oversized_heaven_gate = minimal_toml()
            .replace("id = \"test.skill\"", "id = \"sword_path.heaven_gate\"")
            .replace("range = 0.0", "range = 100.01");

        let guangbo_error = load(&zero_cost_guangbo)
            .expect_err("body.guangbo_ticao must not admit a free proficiency loop");
        assert!(guangbo_error.to_string().contains("strictly positive"));
        let heaven_gate_error =
            load(&oversized_heaven_gate).expect_err("global entity fan-out range must be bounded");
        assert!(heaven_gate_error.to_string().contains("100 blocks"));
    }

    #[test]
    fn rejects_oversized_resonance_fan_out_range() {
        // M30：resonance 是世界级 entity fan-out，range 必须有 operational 上限。
        let oversized = minimal_toml()
            .replace("id = \"test.skill\"", "id = \"sword_path.resonance\"")
            .replace("range = 0.0", "range = 30.01");
        let error = load(&oversized)
            .expect_err("sword_path.resonance range must be bounded to prevent world fan-out");
        assert!(error.to_string().contains("30 blocks"));

        // 恰好 30 与 6（checked-in 值）仍合法。
        for valid in ["30.0", "6.0"] {
            let input = minimal_toml()
                .replace("id = \"test.skill\"", "id = \"sword_path.resonance\"")
                .replace("range = 0.0", &format!("range = {valid}"));
            assert!(
                load(&input).is_ok(),
                "resonance range {valid} must remain valid"
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
    fn arbitrary_direct_generic_metadata_is_data_owned() {
        let registry = load(&minimal_toml().replace(
            "dispatch = \"metadata_backed\"",
            "dispatch = \"direct_generic\"",
        ))
        .expect("an arbitrary valid id may opt into the generic cast path");
        let skills = SkillRegistry::default();

        validate_startup_wiring(&registry, &skills, &SkillMeridianDependencies::default())
            .expect("direct_generic does not require a dependency declaration");

        let mut dependencies = SkillMeridianDependencies::default();
        dependencies.declare("test.skill", Vec::new());
        validate_startup_wiring(&registry, &skills, &dependencies)
            .expect("direct_generic may also have an explicit dependency declaration");
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
        let dependencies =
            crate::cultivation::skill_registry::init_meridian_dependencies(&techniques);

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

    #[test]
    fn oversize_aggregate_snapshot_is_rejected_at_startup_wiring() {
        // M18：catalog 聚合快照超过 `MAX_PAYLOAD_BYTES`（32 KiB）时必须启动期拒绝，
        // 而不是等发送端 `PayloadBuildError::Oversize` 整包丢弃。单条 1024 字节上限
        // 与 512 条总量上限都放行时，聚合大小门禁是最后一道防线——这里用 400 条
        // description 都顶到 900 字节（仍低于 1024 单条上限）来压过 32 KiB 上限：
        // 400 × (3×900 + 192) ≈ 1.1 MB ≫ 32 KiB。
        let mut catalog = String::new();
        for index in 0..400 {
            catalog.push_str(&format!(
                r#"
[[techniques]]
id = "bulk.{index}"
display_name = "批量功法"
grade = "common"
description = "{}"
required_realm = "Awaken"
required_meridians = []
required_race = {{ kind = "any" }}
qi_cost = 1.0
stamina_cost = 0.0
cast_ticks = 10
cooldown_ticks = 30
range = 3.0
icon_texture = "bong-client:textures/gui/items/skill_scroll_sword_cleave.png"
category = "attack"
dispatch = "direct_generic"
"#,
                "刀".repeat(300)
            ));
        }
        let registry = load(&catalog).expect("bulk catalog within per-entry limits must load");
        assert!(
            registry.aggregate_snapshot_size() > crate::schema::common::MAX_PAYLOAD_BYTES,
            "400×1000-byte descriptions must exceed the 32 KiB wire limit (aggregate = {})",
            registry.aggregate_snapshot_size()
        );
        let error = validate_startup_wiring(
            &registry,
            &SkillRegistry::default(),
            &SkillMeridianDependencies::default(),
        )
        .expect_err("oversize aggregate snapshot must fail startup wiring");
        assert!(
            error.to_string().contains("MAX_PAYLOAD_BYTES"),
            "wiring rejection should name the payload limit, got {error}"
        );
    }

    #[test]
    fn checked_in_catalog_aggregate_snapshot_fits_wire_limit() {
        let techniques = production_registry();
        let aggregate = techniques.aggregate_snapshot_size();
        assert!(
            aggregate <= crate::schema::common::MAX_PAYLOAD_BYTES,
            "checked-in catalog worst-case snapshot ~{aggregate} bytes must fit {}",
            crate::schema::common::MAX_PAYLOAD_BYTES
        );
    }
}
