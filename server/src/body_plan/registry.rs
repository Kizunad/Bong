//! plan-race-system-v1 P0 — `BodyPlanRegistry`：`server/assets/body_plans/plans/*.json`
//! glob 加载，仿 `combat::armor::ArmorProfileRegistry::load_dir` 的 fail-fast 模式
//! （重复 id / 缺字段 / 校验失败一律 `Err`，不静默丢弃坏条目）。
//!
//! **glob 边界**：本 registry 只扫描 `plans/` 子目录。`races.json`（`race_registry`）与
//! 未来的 `layouts/`（P2）各有独立 loader，三类资源目录互不重叠——防止异构 JSON 混入
//! `plans/*.json` 被误当成 `BodyPlan` 解析（会在 `serde_json::from_str::<BodyPlan>` 阶段
//! 因缺少必填字段而报 `Json` 错误，而不是静默跳过）。

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use valence::prelude::Resource;

use super::types::{BodyPlan, BodyPlanId};
use super::validate::validate_body_plan;

pub const DEFAULT_BODY_PLANS_DIR: &str = "assets/body_plans/plans";
/// 唯一强制存在的基线构型 id——`resolve_body_plan` 的第三优先级兜底、`BodyPlanPlugin`
/// 启动期存在性断言均以此为准（见 `body_plan::register`）。
pub const HUMANOID_BODY_PLAN_ID: &str = "humanoid";

/// plan-race-system-v1 P0b —— 供**无 ECS 访问权限**的纯函数消费点使用的 humanoid plan
/// 单例。`combat::raycast::classify_body_part`/`standing_humanoid_aabb` 与
/// `combat::arm_wound`/`movement::leg_wound` 的六维伤势分发在 P0b 之前就已是不依赖
/// Bevy `World` 的纯函数（`combat/needle.rs`、`combat/anqi_v2.rs`、`movement/mod.rs`
/// 等大量既有调用点直接调用，不经过任何 system 参数），P0b 要求它们"改走 body_plan
/// 数据"又不能改变函数签名去牵动这些既有调用点——因此不能像 `resolve_attack_intents`
/// 那样接 `Res<BodyPlanRegistry>`。
///
/// 本函数与 `load_dir()` 读同一份磁盘文件（`CARGO_MANIFEST_DIR`-relative，与
/// `body_plan::register()` 启动期加载路径一致），`OnceLock` 保证进程生命周期内只做一次
/// 文件 IO + JSON 解析，此后是一次原子读的开销——不产生第二份硬编码数据源，humanoid
/// 的部位倍率 / 命中几何 / PartConsequence 标签只在 `humanoid.json` 里写一次。
pub fn humanoid_plan_static() -> &'static BodyPlan {
    static PLAN: OnceLock<BodyPlan> = OnceLock::new();
    PLAN.get_or_init(|| {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(DEFAULT_BODY_PLANS_DIR)
            .join(format!("{HUMANOID_BODY_PLAN_ID}.json"));
        let text = fs::read_to_string(&path).unwrap_or_else(|error| {
            panic!(
                "[bong][body_plan] humanoid_plan_static failed reading {}: {error}",
                path.display()
            )
        });
        let plan: BodyPlan = serde_json::from_str(&text).unwrap_or_else(|error| {
            panic!(
                "[bong][body_plan] humanoid_plan_static failed parsing {}: {error}",
                path.display()
            )
        });
        if let Err(reason) = validate_body_plan(&plan) {
            panic!(
                "[bong][body_plan] humanoid_plan_static loaded an invalid plan from {}: {reason}",
                path.display()
            );
        }
        plan
    })
}

#[derive(Debug, Default, Clone)]
pub struct BodyPlanRegistry {
    by_id: HashMap<BodyPlanId, BodyPlan>,
}

impl Resource for BodyPlanRegistry {}

#[derive(Debug)]
pub enum BodyPlanLoadError {
    Io(std::io::Error),
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    DuplicatePlanId {
        path: PathBuf,
        plan_id: String,
    },
    Invalid {
        path: PathBuf,
        plan_id: String,
        reason: String,
    },
}

impl std::fmt::Display for BodyPlanLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BodyPlanLoadError::Io(e) => write!(f, "io: {e}"),
            BodyPlanLoadError::Json { path, source } => {
                write!(f, "json: {}: {source}", path.display())
            }
            BodyPlanLoadError::DuplicatePlanId { path, plan_id } => write!(
                f,
                "duplicate body plan id {plan_id:?} encountered at {}",
                path.display()
            ),
            BodyPlanLoadError::Invalid {
                path,
                plan_id,
                reason,
            } => write!(
                f,
                "invalid body plan {} (id={plan_id:?}): {reason}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for BodyPlanLoadError {}

impl From<std::io::Error> for BodyPlanLoadError {
    fn from(e: std::io::Error) -> Self {
        BodyPlanLoadError::Io(e)
    }
}

impl BodyPlanRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    pub fn get(&self, id: &BodyPlanId) -> Option<&BodyPlan> {
        self.by_id.get(id)
    }

    pub fn contains(&self, id: &BodyPlanId) -> bool {
        self.by_id.contains_key(id)
    }

    pub fn ids(&self) -> impl Iterator<Item = &BodyPlanId> {
        self.by_id.keys()
    }

    /// 唯一强制存在的基线构型——`register()` 启动期已断言存在，故直接 `expect`
    /// （面板/命中几何解析矩阵的第三优先级兜底，见 `resolve::resolve_body_plan`）。
    pub fn humanoid_default(&self) -> &BodyPlan {
        self.get(&BodyPlanId::new(HUMANOID_BODY_PLAN_ID))
            .expect("humanoid body plan must always be loaded — register() asserts this at startup")
    }

    /// 供测试直接构造 registry，不走文件 IO（跨 registry 校验 / resolve 矩阵测试用）。
    #[cfg(test)]
    pub fn from_plans(plans: Vec<BodyPlan>) -> Result<Self, BodyPlanLoadError> {
        let mut reg = Self::new();
        for plan in plans {
            reg.insert_validated(PathBuf::from("<memory>"), plan)?;
        }
        Ok(reg)
    }

    fn insert_validated(&mut self, path: PathBuf, plan: BodyPlan) -> Result<(), BodyPlanLoadError> {
        let plan_id = plan.id.as_str().to_string();
        if plan_id.trim().is_empty() {
            return Err(BodyPlanLoadError::Invalid {
                path,
                plan_id,
                reason: "id must not be empty".to_string(),
            });
        }
        if let Err(reason) = validate_body_plan(&plan) {
            return Err(BodyPlanLoadError::Invalid {
                path,
                plan_id,
                reason,
            });
        }
        if self.by_id.contains_key(&plan.id) {
            return Err(BodyPlanLoadError::DuplicatePlanId { path, plan_id });
        }
        self.by_id.insert(plan.id.clone(), plan);
        Ok(())
    }

    /// 扫描 `dir` 下全部 `*.json`（不存在的目录视为「未配置」，返回空 registry——
    /// 「humanoid 必须存在」这条业务规则由 `register()` 在装载后集中断言，不在这里
    /// 隐式假设目录一定存在，二者分离让「空目录」场景可被独立测试）。
    pub fn load_dir(path: impl AsRef<Path>) -> Result<Self, BodyPlanLoadError> {
        let dir = path.as_ref();
        let mut reg = Self::new();
        if !dir.exists() {
            tracing::warn!(
                "[bong][body_plan] body_plans/plans dir {} does not exist — registry empty",
                dir.display()
            );
            return Ok(reg);
        }

        let mut paths: Vec<PathBuf> = fs::read_dir(dir)?
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|s| s.to_str()) == Some("json"))
            .collect();
        // 确定性加载顺序（跨平台 read_dir 顺序未定义），保证重复 id 报错时命中的是
        // 稳定可复现的第二个文件，而不是依 OS 目录遍历顺序抖动。
        paths.sort();

        for path in paths {
            let text = fs::read_to_string(&path)?;
            let plan: BodyPlan =
                serde_json::from_str(&text).map_err(|e| BodyPlanLoadError::Json {
                    path: path.clone(),
                    source: e,
                })?;
            tracing::info!(
                "[bong][body_plan] loaded body plan id={} is_humanoid={}",
                plan.id,
                plan.is_humanoid
            );
            reg.insert_validated(path, plan)?;
        }

        Ok(reg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body_plan::types::{
        BodyPartDef, HeightBand, HeightBandAssignment, HitGeometry, PartConsequence,
        StandingAabbSpec,
    };
    use std::io::Write;

    fn minimal_plan(id: &str) -> BodyPlan {
        BodyPlan {
            id: BodyPlanId::new(id),
            display_name: "测试构型".to_string(),
            // plan-race-system-v1 P1a：validate_body_plan 现在要求 is_humanoid==true 的
            // plan 必须提供 meridian_profile；本 fixture 只用于练registry加载机制
            // （重复 id / glob / 校验失败等），与 humanoid 语义无关，故设 false 避免
            // 每处调用都要多写一份 profile 数据。
            is_humanoid: false,
            parts: vec![BodyPartDef {
                id: "core".into(),
                damage_mul: 1.0,
                contam_mul: 1.0,
                bleed_mul: 1.0,
                consequence: PartConsequence::Core,
            }],
            hit_geometry: HitGeometry::HeightBands {
                aabb: StandingAabbSpec {
                    half_width: 0.3,
                    height: 1.8,
                },
                bands: vec![HeightBand {
                    min_rel_y: -1.0,
                    assignment: HeightBandAssignment::Single {
                        part: "core".into(),
                    },
                }],
                lateral_threshold: 0.19,
            },
            equip_slots: vec![],
            meridian_profile: None,
            mutation_slot_mapping: HashMap::new(),
        }
    }

    #[test]
    fn load_dir_happy_path_loads_all_json_files() {
        let dir = tempdir();
        write_json(&dir, "a.json", &minimal_plan("plan_a"));
        write_json(&dir, "b.json", &minimal_plan("plan_b"));

        let registry = BodyPlanRegistry::load_dir(&dir).expect("both plans should load");
        assert_eq!(registry.len(), 2);
        assert!(registry.contains(&BodyPlanId::new("plan_a")));
        assert!(registry.contains(&BodyPlanId::new("plan_b")));

        cleanup(&dir);
    }

    #[test]
    fn load_dir_rejects_duplicate_plan_id_across_files() {
        let dir = tempdir();
        write_json(&dir, "a.json", &minimal_plan("dup"));
        write_json(&dir, "b.json", &minimal_plan("dup"));

        let err = BodyPlanRegistry::load_dir(&dir)
            .expect_err("duplicate plan id across two files must fail fast");
        assert!(
            matches!(err, BodyPlanLoadError::DuplicatePlanId { .. }),
            "expected DuplicatePlanId, got {err:?}"
        );

        cleanup(&dir);
    }

    #[test]
    fn load_dir_rejects_missing_required_field() {
        let dir = tempdir();
        let path = dir.join("broken.json");
        // 缺 `parts` 必填字段。
        std::fs::write(
            &path,
            r#"{"id":"broken","display_name":"broken","is_humanoid":true,"hit_geometry":{"mode":"height_bands","aabb":{"half_width":0.3,"height":1.8},"bands":[],"lateral_threshold":0.19},"equip_slots":[]}"#,
        )
        .expect("write broken.json");

        let err = BodyPlanRegistry::load_dir(&dir)
            .expect_err("missing required field must fail with a Json error");
        assert!(
            matches!(err, BodyPlanLoadError::Json { .. }),
            "expected Json parse error for missing field, got {err:?}"
        );

        cleanup(&dir);
    }

    #[test]
    fn load_dir_rejects_validation_failure_with_located_reason() {
        let dir = tempdir();
        let mut plan = minimal_plan("invalid_bands");
        plan.hit_geometry = HitGeometry::HeightBands {
            aabb: StandingAabbSpec {
                half_width: 0.3,
                height: 1.8,
            },
            // 未覆盖到 -inf 的 band（不合法：rel_y=0 会落空）。
            bands: vec![HeightBand {
                min_rel_y: 0.5,
                assignment: HeightBandAssignment::Single {
                    part: "core".into(),
                },
            }],
            lateral_threshold: 0.19,
        };
        write_json(&dir, "invalid.json", &plan);

        let err = BodyPlanRegistry::load_dir(&dir)
            .expect_err("validate_body_plan failure must propagate as Invalid with plan id");
        match err {
            BodyPlanLoadError::Invalid {
                plan_id, reason, ..
            } => {
                assert_eq!(plan_id, "invalid_bands");
                assert!(
                    reason.contains("-inf") || reason.contains("coverage"),
                    "reason should carry actionable location info, got: {reason}"
                );
            }
            other => panic!("expected Invalid, got {other:?}"),
        }

        cleanup(&dir);
    }

    #[test]
    fn load_dir_on_empty_directory_returns_empty_registry_not_error() {
        let dir = tempdir();
        // 目录存在但没有任何 json 文件。
        let registry = BodyPlanRegistry::load_dir(&dir).expect("empty dir is not a load error");
        assert_eq!(registry.len(), 0);
        assert!(registry.is_empty());

        cleanup(&dir);
    }

    #[test]
    fn load_dir_on_nonexistent_directory_returns_empty_registry() {
        let dir = std::env::temp_dir().join(format!(
            "bong-body-plan-registry-test-missing-{}",
            std::process::id()
        ));
        let registry =
            BodyPlanRegistry::load_dir(&dir).expect("nonexistent dir should warn, not error");
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn load_dir_ignores_non_json_files() {
        let dir = tempdir();
        write_json(&dir, "a.json", &minimal_plan("plan_a"));
        std::fs::write(dir.join("readme.txt"), "not json").expect("write readme");

        let registry = BodyPlanRegistry::load_dir(&dir).expect("should ignore non-json file");
        assert_eq!(registry.len(), 1);

        cleanup(&dir);
    }

    /// 「混装文件进错目录」反例：`races.json` 的形状（顶层 `races`/`morph_pairs` 数组，
    /// 不是 `BodyPlan` 的 `id`/`parts`/`hit_geometry` 字段）被误放进 `plans/` 目录——
    /// `deny_unknown_fields` + 缺失必填字段必须让 `serde_json::from_str::<BodyPlan>`
    /// 在解析阶段就 fail-fast（`Json` 错误），而不是静默吞掉或误解析出一个残缺 `BodyPlan`。
    #[test]
    fn load_dir_rejects_races_file_shape_misplaced_in_plans_dir() {
        let dir = tempdir();
        let races_shaped_json = r#"{
            "races": [
                {"id": "human", "display_name": "人族", "body_plan_id": "humanoid", "beast_kinds": []}
            ],
            "morph_pairs": []
        }"#;
        std::fs::write(dir.join("races.json"), races_shaped_json)
            .expect("write misplaced races.json-shaped file");

        let err = BodyPlanRegistry::load_dir(&dir)
            .expect_err("races.json 形状的文件混进 plans/ 目录必须在解析期报错，而不是被静默接受");
        assert!(
            matches!(err, BodyPlanLoadError::Json { .. }),
            "expected Json parse error for a races.json-shaped file lacking BodyPlan required \
             fields (id/parts/hit_geometry/...), got {err:?}"
        );

        cleanup(&dir);
    }

    #[test]
    fn humanoid_default_returns_the_humanoid_plan() {
        let registry = BodyPlanRegistry::from_plans(vec![minimal_plan(HUMANOID_BODY_PLAN_ID)])
            .expect("humanoid plan should validate");
        assert_eq!(registry.humanoid_default().id.as_str(), "humanoid");
    }

    #[test]
    #[should_panic(expected = "humanoid body plan must always be loaded")]
    fn humanoid_default_panics_when_missing() {
        let registry = BodyPlanRegistry::from_plans(vec![minimal_plan("not_humanoid")])
            .expect("plan should validate");
        let _ = registry.humanoid_default();
    }

    // ───────────────────────── humanoid_plan_static ─────────────────────────

    #[test]
    fn humanoid_plan_static_loads_the_real_humanoid_id() {
        let plan = humanoid_plan_static();
        assert_eq!(plan.id.as_str(), HUMANOID_BODY_PLAN_ID);
        assert!(plan.is_humanoid);
    }

    #[test]
    fn humanoid_plan_static_matches_registry_loaded_from_same_asset_dir() {
        // 单例读的是与 `load_dir()` 完全相同的磁盘文件——本测试直接核验二者产出的
        // BodyPlan 值相等，锁死"不产生第二份硬编码数据源"这条设计红线。
        let plans_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(super::DEFAULT_BODY_PLANS_DIR);
        let registry = BodyPlanRegistry::load_dir(&plans_dir)
            .expect("real assets/body_plans/plans dir must load for this parity test");
        let from_registry = registry
            .get(&BodyPlanId::new(HUMANOID_BODY_PLAN_ID))
            .expect("real humanoid.json must be present in assets/body_plans/plans");
        assert_eq!(
            humanoid_plan_static(),
            from_registry,
            "humanoid_plan_static() 必须与 load_dir() 加载同一份 humanoid.json 得到完全一致的 BodyPlan"
        );
    }

    #[test]
    fn humanoid_plan_static_is_cached_across_calls() {
        // OnceLock 缓存：两次调用必须返回同一内存地址（同一个 'static 引用），
        // 证明不是每次调用都重新读盘解析。
        let first: *const BodyPlan = humanoid_plan_static();
        let second: *const BodyPlan = humanoid_plan_static();
        assert_eq!(
            first, second,
            "humanoid_plan_static 必须缓存单例，不应每次调用都重新分配"
        );
    }

    #[test]
    fn humanoid_plan_static_covers_all_eight_legacy_parts() {
        let plan = humanoid_plan_static();
        for expected_id in [
            "head", "chest", "back", "abdomen", "arm_l", "arm_r", "leg_l", "leg_r",
        ] {
            assert!(
                plan.parts.iter().any(|def| def.id.as_str() == expected_id),
                "humanoid.json 必须声明部位 {expected_id}（legacy BodyPart 8 段全覆盖前提）"
            );
        }
    }

    #[test]
    fn humanoid_plan_static_declares_the_four_armor_equip_slots() {
        // plan-race-system-v1 §P0 交付物 pin："humanoid.json 首个条目：8 部位 / 倍率表
        // / 1.8m AABB 分段阈值 / 4 护甲槽，与现状硬编码值逐项 bit-for-bit 对齐"——
        // 这条断言逐一核对 head/chest/legs/feet 四个装备槽集合（既不多也不少，用
        // `HashSet` 对拍而非只查"contains"，防止悄悄多出第 5 个槽或漏掉一个仍然通过）。
        use crate::schema::inventory::EquipSlotV1;
        use std::collections::HashSet;

        let plan = humanoid_plan_static();
        let actual: HashSet<EquipSlotV1> = plan.equip_slots.iter().cloned().collect();
        let expected: HashSet<EquipSlotV1> = [
            EquipSlotV1::Head,
            EquipSlotV1::Chest,
            EquipSlotV1::Legs,
            EquipSlotV1::Feet,
        ]
        .into_iter()
        .collect();
        assert_eq!(
            actual, expected,
            "humanoid.json equip_slots 必须恰好是 head/chest/legs/feet 四个护甲槽 \
             （不多不少）——实际值 {actual:?}"
        );
        assert_eq!(
            plan.equip_slots.len(),
            4,
            "humanoid.json equip_slots 必须没有重复条目（长度应与去重后的集合大小一致）"
        );
    }

    #[test]
    fn humanoid_plan_static_declares_mutation_slot_mapping_for_all_five_body_slot_variants() {
        // 全变体饱和 pin（数据侧）：humanoid.json 的 `mutation_slot_mapping` 必须覆盖
        // `dandao::mutation::BodySlot` 全部 5 个变体——不是"覆盖率"这种笼统断言，是
        // 逐变体键存在性对拍。查询函数 `body_part_for_mutation_slot` 逐变体值对拍见
        // `resolve.rs::body_part_for_mutation_slot_covers_every_body_slot_variant_on_real_humanoid_plan`。
        use crate::dandao::mutation::BodySlot;

        let plan = humanoid_plan_static();
        for slot in [
            BodySlot::Head,
            BodySlot::Forearm,
            BodySlot::Back,
            BodySlot::Torso,
            BodySlot::Lower,
        ] {
            assert!(
                plan.mutation_slot_mapping.contains_key(&slot),
                "humanoid.json mutation_slot_mapping 缺失 {slot:?} 变体的映射"
            );
        }
    }

    #[test]
    fn humanoid_plan_static_mutation_slot_mapping_matches_client_shared_resource() {
        // plan-race-system-v1 P0 review r3（major×3 收口）—— client
        // `MutationFeatureRenderer` 不得维护私有硬编码定位表（此前 `MutationKind`
        // 携带的 `defaultBodySlot` 是完全未被消费的死代码，且与本文件的
        // `mutation_slot_mapping` 各自独立演进，随时可能漂移）。唯一映射契约现在落在
        // `client/src/main/resources/assets/bong/body_plans/humanoid_mutation_slots.json`
        // （client `MutationSlotLayoutRegistry` 启动加载，供 `MutationFeatureRenderer`
        // 按 slot→part→锚 定位渲染）——本测试从 server 侧跨目录读取该文件（precedent:
        // `network::resourcepack` 用 `include_str!("../../../client/resourcepack/
        // manifest.json")` 读同一仓库下 client 目录的文件），逐个 `BodySlot` 变体断言
        // 两份数据的 `part_id` 完全一致，防止静默漂移。
        use crate::dandao::mutation::BodySlot;

        let plan = humanoid_plan_static();
        let json_str = include_str!(
            "../../../client/src/main/resources/assets/bong/body_plans/humanoid_mutation_slots.json"
        );
        let parsed: serde_json::Value = serde_json::from_str(json_str)
            .expect("client shared mutation slot resource must be valid JSON");
        let slots_obj = parsed
            .get("slots")
            .and_then(|v| v.as_object())
            .expect("shared resource must declare a `slots` object");

        let known_slots: [(&str, BodySlot); 5] = [
            ("Head", BodySlot::Head),
            ("Forearm", BodySlot::Forearm),
            ("Back", BodySlot::Back),
            ("Torso", BodySlot::Torso),
            ("Lower", BodySlot::Lower),
        ];

        for (key, slot) in known_slots {
            let entry = slots_obj
                .get(key)
                .unwrap_or_else(|| panic!("client shared resource missing slot key '{key}'"));
            let resource_part_id = entry
                .get("part_id")
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| panic!("client shared resource slot '{key}' missing part_id"));
            let plan_part_id = plan
                .mutation_slot_mapping
                .get(&slot)
                .unwrap_or_else(|| panic!("humanoid.json mutation_slot_mapping missing {slot:?}"));
            assert_eq!(
                resource_part_id,
                plan_part_id.as_str(),
                "BodySlot::{slot:?} 在 humanoid.json（{plan_part_id}）与 client 共享资源\
                 humanoid_mutation_slots.json（{resource_part_id}）之间的映射不一致——\
                 两份数据已漂移，必须同步更新"
            );
        }

        assert_eq!(
            slots_obj.len(),
            known_slots.len(),
            "client 共享资源的 slots 键数量应恰好是 5 个 BodySlot 变体，不多不少——\
             实测 {} 个键：{:?}",
            slots_obj.len(),
            slots_obj.keys().collect::<Vec<_>>()
        );
    }

    fn tempdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "bong-body-plan-registry-test-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        std::fs::create_dir_all(&dir).expect("create tempdir");
        dir
    }

    fn unique_suffix() -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        COUNTER.fetch_add(1, Ordering::Relaxed)
    }

    fn write_json(dir: &Path, name: &str, plan: &BodyPlan) {
        let path = dir.join(name);
        let mut file = std::fs::File::create(&path).expect("create json file");
        let json = serde_json::to_string_pretty(plan).expect("serialize plan");
        file.write_all(json.as_bytes()).expect("write json file");
    }

    fn cleanup(dir: &Path) {
        let _ = std::fs::remove_dir_all(dir);
    }
}
