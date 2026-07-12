//! plan-race-system-v1 P2a — `BodyPlanLayoutRegistry`：
//! `server/assets/body_plans/layouts/*.json` 独立目录加载（不进 `plans/` glob，见
//! `registry.rs` 顶部注释的目录边界——`plans/`、`races.json`、`layouts/` 三类资源目录
//! 互不重叠）。
//!
//! 每个文件是一份完整 `BodyPlanLayoutV1`（`body_plan_id` 主键取自文件内容而非文件名），
//! 加载期做跨 registry 校验（[`super::validate::validate_body_plan_layout`]）——目标
//! `BodyPlan` 必须已在 `BodyPlanRegistry` 中注册，且 layout 引用的 part id / channel id
//! 均不悬空。

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use valence::prelude::Resource;

use crate::schema::server_data::BodyPlanLayoutV1;

use super::registry::BodyPlanRegistry;
use super::types::BodyPlanId;
use super::validate::validate_body_plan_layout;

pub const DEFAULT_BODY_PLAN_LAYOUTS_DIR: &str = "assets/body_plans/layouts";

#[derive(Debug, Default, Clone)]
pub struct BodyPlanLayoutRegistry {
    by_id: HashMap<BodyPlanId, BodyPlanLayoutV1>,
}

impl Resource for BodyPlanLayoutRegistry {}

#[derive(Debug)]
pub enum BodyPlanLayoutLoadError {
    Io(std::io::Error),
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    DuplicateBodyPlanId {
        path: PathBuf,
        body_plan_id: String,
    },
    Invalid {
        path: PathBuf,
        body_plan_id: String,
        reason: String,
    },
}

impl std::fmt::Display for BodyPlanLayoutLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BodyPlanLayoutLoadError::Io(e) => write!(f, "io: {e}"),
            BodyPlanLayoutLoadError::Json { path, source } => {
                write!(f, "json: {}: {source}", path.display())
            }
            BodyPlanLayoutLoadError::DuplicateBodyPlanId { path, body_plan_id } => write!(
                f,
                "duplicate body plan layout for body_plan_id {body_plan_id:?} encountered at {}",
                path.display()
            ),
            BodyPlanLayoutLoadError::Invalid {
                path,
                body_plan_id,
                reason,
            } => write!(
                f,
                "invalid body plan layout {} (body_plan_id={body_plan_id:?}): {reason}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for BodyPlanLayoutLoadError {}

impl From<std::io::Error> for BodyPlanLayoutLoadError {
    fn from(e: std::io::Error) -> Self {
        BodyPlanLayoutLoadError::Io(e)
    }
}

impl BodyPlanLayoutRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    pub fn get(&self, id: &BodyPlanId) -> Option<&BodyPlanLayoutV1> {
        self.by_id.get(id)
    }

    pub fn contains(&self, id: &BodyPlanId) -> bool {
        self.by_id.contains_key(id)
    }

    /// 供测试直接构造 registry，不走文件 IO。
    #[cfg(test)]
    pub fn from_layouts(
        layouts: Vec<BodyPlanLayoutV1>,
        body_plans: &BodyPlanRegistry,
    ) -> Result<Self, BodyPlanLayoutLoadError> {
        let mut reg = Self::new();
        for layout in layouts {
            reg.insert_validated(PathBuf::from("<memory>"), layout, body_plans)?;
        }
        Ok(reg)
    }

    fn insert_validated(
        &mut self,
        path: PathBuf,
        layout: BodyPlanLayoutV1,
        body_plans: &BodyPlanRegistry,
    ) -> Result<(), BodyPlanLayoutLoadError> {
        let body_plan_id = layout.body_plan_id.clone();
        if body_plan_id.trim().is_empty() {
            return Err(BodyPlanLayoutLoadError::Invalid {
                path,
                body_plan_id,
                reason: "body_plan_id must not be empty".to_string(),
            });
        }
        let plan_id = BodyPlanId::new(body_plan_id.clone());
        let plan = body_plans
            .get(&plan_id)
            .ok_or_else(|| BodyPlanLayoutLoadError::Invalid {
                path: path.clone(),
                body_plan_id: body_plan_id.clone(),
                reason: format!(
                    "no BodyPlan with id {body_plan_id} is registered — layouts must reference \
                     an already-loaded body plan"
                ),
            })?;
        if let Err(reason) = validate_body_plan_layout(&layout, plan) {
            return Err(BodyPlanLayoutLoadError::Invalid {
                path,
                body_plan_id,
                reason,
            });
        }
        if self.by_id.contains_key(&plan_id) {
            return Err(BodyPlanLayoutLoadError::DuplicateBodyPlanId { path, body_plan_id });
        }
        self.by_id.insert(plan_id, layout);
        Ok(())
    }

    /// 扫描 `dir` 下全部 `*.json`（不存在的目录视为「未配置」，返回空 registry——与
    /// `BodyPlanRegistry::load_dir` 同一语义，见其文档）。
    pub fn load_dir(
        path: impl AsRef<Path>,
        body_plans: &BodyPlanRegistry,
    ) -> Result<Self, BodyPlanLayoutLoadError> {
        let dir = path.as_ref();
        let mut reg = Self::new();
        if !dir.exists() {
            tracing::warn!(
                "[bong][body_plan] body_plans/layouts dir {} does not exist — registry empty",
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
        paths.sort();

        for path in paths {
            let text = fs::read_to_string(&path)?;
            let layout: BodyPlanLayoutV1 =
                serde_json::from_str(&text).map_err(|e| BodyPlanLayoutLoadError::Json {
                    path: path.clone(),
                    source: e,
                })?;
            tracing::info!(
                "[bong][body_plan] loaded body plan layout body_plan_id={}",
                layout.body_plan_id
            );
            reg.insert_validated(path, layout, body_plans)?;
        }

        Ok(reg)
    }
}

/// 供**无 ECS 访问权限**的纯函数消费点使用的 humanoid layout 单例（仿
/// `registry::humanoid_plan_static` 先例）——`network::wounds_snapshot_emit::body_part_wire`
/// 是唯一消费方，不必牵动其函数签名去接 `Res<BodyPlanLayoutRegistry>`。
pub fn humanoid_layout_static() -> &'static BodyPlanLayoutV1 {
    static LAYOUT: OnceLock<BodyPlanLayoutV1> = OnceLock::new();
    LAYOUT.get_or_init(|| {
        let path = super::resolve_assets_root()
            .join(DEFAULT_BODY_PLAN_LAYOUTS_DIR)
            .join(format!("{}.json", super::registry::HUMANOID_BODY_PLAN_ID));
        let text = fs::read_to_string(&path).unwrap_or_else(|error| {
            panic!(
                "[bong][body_plan] humanoid_layout_static failed reading {}: {error}",
                path.display()
            )
        });
        let layout: BodyPlanLayoutV1 = serde_json::from_str(&text).unwrap_or_else(|error| {
            panic!(
                "[bong][body_plan] humanoid_layout_static failed parsing {}: {error}",
                path.display()
            )
        });
        let plan = super::registry::humanoid_plan_static();
        if let Err(reason) = validate_body_plan_layout(&layout, plan) {
            panic!(
                "[bong][body_plan] humanoid_layout_static loaded an invalid layout from {}: {reason}",
                path.display()
            );
        }
        layout
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::server_data::{
        BodyPlanPartAnchorV1, BodyPlanPartDisplayMappingV1, BodyPlanPoint2V1,
        BodyPlanSilhouettePartV1,
    };

    fn p(x: f64, y: f64) -> BodyPlanPoint2V1 {
        BodyPlanPoint2V1 { x, y }
    }

    fn fixture_layout(body_plan_id: &str) -> BodyPlanLayoutV1 {
        BodyPlanLayoutV1 {
            body_plan_id: body_plan_id.to_string(),
            silhouette: vec![BodyPlanSilhouettePartV1 {
                part_id: "chest".to_string(),
                polygon: vec![p(0.3, 0.1), p(0.7, 0.1), p(0.7, 0.3), p(0.3, 0.3)],
            }],
            anchors: vec![BodyPlanPartAnchorV1 {
                part_id: "chest".to_string(),
                point: p(0.5, 0.2),
            }],
            meridian_paths: vec![],
            part_display_map: vec![BodyPlanPartDisplayMappingV1 {
                server_part_id: "chest".to_string(),
                display_segment_id: "chest".to_string(),
            }],
        }
    }

    fn tempdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "bong-body-plan-layout-registry-test-{}-{}",
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

    fn write_json(dir: &Path, name: &str, layout: &BodyPlanLayoutV1) {
        let path = dir.join(name);
        let json = serde_json::to_string_pretty(layout).expect("serialize layout");
        std::fs::write(&path, json).expect("write json file");
    }

    fn cleanup(dir: &Path) {
        let _ = std::fs::remove_dir_all(dir);
    }

    fn plan_registry_with_humanoid_like() -> BodyPlanRegistry {
        BodyPlanRegistry::from_plans(vec![crate::body_plan::humanoid_plan_static().clone()])
            .expect("humanoid plan should validate")
    }

    #[test]
    fn load_dir_happy_path_loads_the_layout() {
        let body_plans = plan_registry_with_humanoid_like();
        let dir = tempdir();
        write_json(&dir, "humanoid.json", &fixture_layout("humanoid"));

        let registry =
            BodyPlanLayoutRegistry::load_dir(&dir, &body_plans).expect("layout should load");
        assert_eq!(registry.len(), 1);
        assert!(registry.contains(&BodyPlanId::new("humanoid")));

        cleanup(&dir);
    }

    #[test]
    fn load_dir_on_nonexistent_directory_returns_empty_registry() {
        let body_plans = plan_registry_with_humanoid_like();
        let dir = std::env::temp_dir().join(format!(
            "bong-body-plan-layout-registry-test-missing-{}",
            std::process::id()
        ));
        let registry = BodyPlanLayoutRegistry::load_dir(&dir, &body_plans)
            .expect("nonexistent dir should warn, not error");
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn load_dir_on_empty_directory_returns_empty_registry_not_error() {
        let body_plans = plan_registry_with_humanoid_like();
        let dir = tempdir();
        let registry = BodyPlanLayoutRegistry::load_dir(&dir, &body_plans)
            .expect("empty dir is not a load error");
        assert_eq!(registry.len(), 0);
        assert!(registry.is_empty());
        cleanup(&dir);
    }

    #[test]
    fn load_dir_rejects_duplicate_body_plan_id_across_files() {
        let body_plans = plan_registry_with_humanoid_like();
        let dir = tempdir();
        write_json(&dir, "a.json", &fixture_layout("humanoid"));
        write_json(&dir, "b.json", &fixture_layout("humanoid"));

        let err = BodyPlanLayoutRegistry::load_dir(&dir, &body_plans)
            .expect_err("duplicate body_plan_id across two files must fail fast");
        assert!(
            matches!(err, BodyPlanLayoutLoadError::DuplicateBodyPlanId { .. }),
            "expected DuplicateBodyPlanId, got {err:?}"
        );
        cleanup(&dir);
    }

    #[test]
    fn load_dir_rejects_layout_referencing_unknown_body_plan() {
        let body_plans = plan_registry_with_humanoid_like();
        let dir = tempdir();
        write_json(&dir, "ghost.json", &fixture_layout("does_not_exist"));

        let err = BodyPlanLayoutRegistry::load_dir(&dir, &body_plans)
            .expect_err("layout referencing an unregistered body plan must fail fast");
        match err {
            BodyPlanLayoutLoadError::Invalid {
                body_plan_id,
                reason,
                ..
            } => {
                assert_eq!(body_plan_id, "does_not_exist");
                assert!(reason.contains("not registered") || reason.contains("no BodyPlan"));
            }
            other => panic!("expected Invalid, got {other:?}"),
        }
        cleanup(&dir);
    }

    #[test]
    fn load_dir_rejects_validation_failure_with_located_reason() {
        let body_plans = plan_registry_with_humanoid_like();
        let dir = tempdir();
        let mut layout = fixture_layout("humanoid");
        layout.silhouette.clear();
        write_json(&dir, "broken.json", &layout);

        let err = BodyPlanLayoutRegistry::load_dir(&dir, &body_plans)
            .expect_err("validate_body_plan_layout failure must propagate as Invalid");
        match err {
            BodyPlanLayoutLoadError::Invalid { reason, .. } => {
                assert!(
                    reason.contains("at least one silhouette part"),
                    "got: {reason}"
                );
            }
            other => panic!("expected Invalid, got {other:?}"),
        }
        cleanup(&dir);
    }

    #[test]
    fn load_dir_rejects_missing_required_field() {
        let body_plans = plan_registry_with_humanoid_like();
        let dir = tempdir();
        std::fs::write(dir.join("broken.json"), r#"{"body_plan_id":"humanoid"}"#)
            .expect("write broken.json");

        let err = BodyPlanLayoutRegistry::load_dir(&dir, &body_plans)
            .expect_err("missing required field must fail with a Json error");
        assert!(
            matches!(err, BodyPlanLayoutLoadError::Json { .. }),
            "expected Json parse error, got {err:?}"
        );
        cleanup(&dir);
    }

    #[test]
    fn load_dir_ignores_non_json_files() {
        let body_plans = plan_registry_with_humanoid_like();
        let dir = tempdir();
        write_json(&dir, "humanoid.json", &fixture_layout("humanoid"));
        std::fs::write(dir.join("readme.txt"), "not json").expect("write readme");

        let registry = BodyPlanLayoutRegistry::load_dir(&dir, &body_plans)
            .expect("should ignore non-json file");
        assert_eq!(registry.len(), 1);
        cleanup(&dir);
    }

    // ───────────────────────── humanoid_layout_static ─────────────────────────

    #[test]
    fn humanoid_layout_static_loads_the_real_humanoid_id() {
        let layout = humanoid_layout_static();
        assert_eq!(layout.body_plan_id, "humanoid");
        assert!(!layout.part_display_map.is_empty());
    }

    #[test]
    fn humanoid_layout_static_is_cached_across_calls() {
        let first: *const BodyPlanLayoutV1 = humanoid_layout_static();
        let second: *const BodyPlanLayoutV1 = humanoid_layout_static();
        assert_eq!(
            first, second,
            "humanoid_layout_static 必须缓存单例，不应每次调用都重新分配"
        );
    }

    #[test]
    fn humanoid_layout_static_matches_registry_loaded_from_same_asset_dir() {
        let body_plans_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(super::super::registry::DEFAULT_BODY_PLANS_DIR);
        let body_plans = BodyPlanRegistry::load_dir(&body_plans_dir)
            .expect("real assets/body_plans/plans dir must load");
        let layouts_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_BODY_PLAN_LAYOUTS_DIR);
        let registry = BodyPlanLayoutRegistry::load_dir(&layouts_dir, &body_plans)
            .expect("real assets/body_plans/layouts dir must load");
        let from_registry = registry
            .get(&BodyPlanId::new("humanoid"))
            .expect("real humanoid.json layout must be present in assets/body_plans/layouts");
        assert_eq!(
            humanoid_layout_static(),
            from_registry,
            "humanoid_layout_static() 必须与 load_dir() 加载同一份 humanoid.json 得到完全一致的布局"
        );
    }

    #[test]
    fn humanoid_layout_static_part_display_map_covers_all_eight_legacy_parts() {
        let layout = humanoid_layout_static();
        for expected_id in [
            "head", "chest", "back", "abdomen", "arm_l", "arm_r", "leg_l", "leg_r",
        ] {
            assert!(
                layout
                    .part_display_map
                    .iter()
                    .any(|m| m.server_part_id == expected_id),
                "layouts/humanoid.json part_display_map 必须覆盖 server 部位 {expected_id}"
            );
        }
    }

    // ───────────── humanoid.json ↔ client 原硬编码逐值对拍 pin ─────────────
    //
    // plan-race-system-v1 P2 红线：「humanoid 布局从现 BodyInspectComponent 硬编码坐标
    // **原样抽取**，首版渲染与现状像素级一致」。原硬编码位于
    // `client/.../BodyInspectComponent.java`：
    //   - `bodyPartRect`（L775-794，`[x1,y1,x2,y2]` 相对 center_x=W/2 的像素表）
    //   - `bodyPartAnchor`（L796-815）
    //   - `MERIDIAN_PATHS`（L827-912，cx 相对折线路径）
    // 画布 `W = 168`、`H = DETAIL_TOP + DETAIL_H = 236`。归一化公式：
    //   x_norm = (84 + px) / 168，y_norm = py / 236
    // json 内数值按 6 位小数落盘，因此逐值对拍容差 = 5e-7（四舍五入最大误差）。
    // 改动 humanoid.json 任何一个坐标（或 client 侧改表未同步）都会撞红这组 pin。

    const CLIENT_CANVAS_W: f64 = 168.0;
    const CLIENT_CANVAS_H: f64 = 236.0;
    const CLIENT_CENTER_X: f64 = 84.0;
    const PIN_TOLERANCE: f64 = 5.0e-7;

    fn nx(px: i32) -> f64 {
        (CLIENT_CENTER_X + f64::from(px)) / CLIENT_CANVAS_W
    }

    fn ny(py: i32) -> f64 {
        f64::from(py) / CLIENT_CANVAS_H
    }

    fn assert_point_pins(actual: &BodyPlanPoint2V1, px: i32, py: i32, context: &str) {
        assert!(
            (actual.x - nx(px)).abs() < PIN_TOLERANCE,
            "{context}: x 期望 {} (= (84 + {px}) / 168，client 原硬编码)，实际 {}",
            nx(px),
            actual.x
        );
        assert!(
            (actual.y - ny(py)).abs() < PIN_TOLERANCE,
            "{context}: y 期望 {} (= {py} / 236，client 原硬编码)，实际 {}",
            ny(py),
            actual.y
        );
    }

    /// client `bodyPartRect` 的 16 段 `[x1, y1, x2, y2]` 原表。
    const CLIENT_BODY_PART_RECTS: [(&str, [i32; 4]); 16] = [
        ("head", [-11, 6, 11, 28]),
        ("neck", [-4, 28, 4, 34]),
        ("chest", [-22, 34, 22, 66]),
        ("abdomen", [-22, 66, 22, 98]),
        ("left_upper_arm", [-34, 36, -26, 72]),
        ("left_forearm", [-36, 72, -28, 108]),
        ("left_hand", [-38, 105, -28, 115]),
        ("right_upper_arm", [26, 36, 34, 72]),
        ("right_forearm", [28, 72, 36, 108]),
        ("right_hand", [28, 105, 38, 115]),
        ("left_thigh", [-19, 110, -7, 156]),
        ("left_calf", [-18, 156, -8, 186]),
        ("left_foot", [-22, 186, -6, 192]),
        ("right_thigh", [7, 110, 19, 156]),
        ("right_calf", [8, 156, 18, 186]),
        ("right_foot", [6, 186, 22, 192]),
    ];

    /// client `bodyPartAnchor` 的 16 段 `[x, y]` 原表。
    const CLIENT_BODY_PART_ANCHORS: [(&str, [i32; 2]); 16] = [
        ("head", [0, 10]),
        ("neck", [0, 30]),
        ("chest", [0, 48]),
        ("abdomen", [0, 82]),
        ("left_upper_arm", [-36, 53]),
        ("left_forearm", [-40, 89]),
        ("left_hand", [-41, 110]),
        ("right_upper_arm", [36, 53]),
        ("right_forearm", [40, 89]),
        ("right_hand", [41, 110]),
        ("left_thigh", [-22, 132]),
        ("left_calf", [-22, 170]),
        ("left_foot", [-19, 190]),
        ("right_thigh", [22, 132]),
        ("right_calf", [22, 170]),
        ("right_foot", [19, 190]),
    ];

    /// client `MERIDIAN_PATHS` 的 20 条折线原表（`MeridianChannel` → snake_case
    /// channel id 映射与 `cultivation` 侧 wire 命名一致）。
    fn client_meridian_paths() -> Vec<(&'static str, Vec<[i32; 2]>)> {
        vec![
            (
                "lung",
                vec![[-8, 40], [-18, 50], [-28, 74], [-34, 100], [-36, 112]],
            ),
            (
                "heart",
                vec![[-16, 48], [-22, 64], [-26, 88], [-28, 108], [-26, 113]],
            ),
            (
                "pericardium",
                vec![[-2, 50], [-14, 66], [-22, 86], [-30, 107], [-32, 113]],
            ),
            (
                "large_intestine",
                vec![[8, 40], [18, 50], [28, 74], [34, 100], [36, 112]],
            ),
            (
                "small_intestine",
                vec![[16, 48], [22, 64], [26, 88], [28, 108], [26, 113]],
            ),
            (
                "triple_energizer",
                vec![[2, 50], [14, 66], [22, 86], [30, 107], [32, 113]],
            ),
            (
                "spleen",
                vec![[-17, 188], [-14, 170], [-11, 140], [-8, 110], [-14, 90]],
            ),
            (
                "kidney",
                vec![[-13, 190], [-11, 170], [-7, 140], [-4, 105], [-2, 72]],
            ),
            (
                "liver",
                vec![[-15, 188], [-12, 170], [-9, 142], [-6, 112], [-10, 82]],
            ),
            (
                "stomach",
                vec![[17, 188], [14, 170], [11, 140], [8, 110], [14, 90]],
            ),
            (
                "bladder",
                vec![[13, 190], [11, 170], [7, 140], [4, 105], [2, 72]],
            ),
            (
                "gallbladder",
                vec![[15, 188], [12, 170], [9, 142], [6, 112], [10, 82]],
            ),
            (
                "ren",
                vec![[-3, 98], [-3, 80], [-3, 62], [-3, 44], [-3, 30]],
            ),
            ("du", vec![[3, 98], [3, 80], [3, 62], [3, 44], [3, 30]]),
            ("chong", vec![[0, 94], [0, 74], [0, 54], [0, 34]]),
            (
                "dai",
                vec![[-20, 84], [-10, 86], [0, 87], [10, 86], [20, 84]],
            ),
            ("yin_wei", vec![[-12, 100], [-16, 78], [-16, 54], [-10, 36]]),
            ("yang_wei", vec![[12, 100], [16, 78], [16, 54], [10, 36]]),
            (
                "yin_qiao",
                vec![[-8, 180], [-6, 145], [-5, 110], [-4, 75], [-2, 26]],
            ),
            (
                "yang_qiao",
                vec![[8, 180], [6, 145], [5, 110], [4, 75], [2, 26]],
            ),
        ]
    }

    #[test]
    fn humanoid_layout_silhouette_pins_client_body_part_rects_verbatim() {
        let layout = humanoid_layout_static();
        assert_eq!(
            layout.silhouette.len(),
            CLIENT_BODY_PART_RECTS.len(),
            "humanoid.json 剪影段数必须与 client bodyPartRect 的 16 段一致"
        );
        for (part_id, [x1, y1, x2, y2]) in CLIENT_BODY_PART_RECTS {
            let part = layout
                .silhouette
                .iter()
                .find(|p| p.part_id == part_id)
                .unwrap_or_else(|| panic!("humanoid.json 缺剪影段 {part_id}"));
            assert_eq!(
                part.polygon.len(),
                4,
                "剪影段 {part_id} 由 client 矩形抽取，必须恰好 4 个顶点"
            );
            // client 矩形 [x1,y1,x2,y2] → 顺时针 4 顶点：(x1,y1) (x2,y1) (x2,y2) (x1,y2)。
            let expected = [(x1, y1), (x2, y1), (x2, y2), (x1, y2)];
            for (i, (ex, ey)) in expected.into_iter().enumerate() {
                assert_point_pins(
                    &part.polygon[i],
                    ex,
                    ey,
                    &format!("剪影段 {part_id} 顶点 #{i}"),
                );
            }
        }
    }

    #[test]
    fn humanoid_layout_anchors_pin_client_body_part_anchors_verbatim() {
        let layout = humanoid_layout_static();
        assert_eq!(
            layout.anchors.len(),
            CLIENT_BODY_PART_ANCHORS.len(),
            "humanoid.json 锚点数必须与 client bodyPartAnchor 的 16 段一致"
        );
        for (part_id, [ax, ay]) in CLIENT_BODY_PART_ANCHORS {
            let anchor = layout
                .anchors
                .iter()
                .find(|a| a.part_id == part_id)
                .unwrap_or_else(|| panic!("humanoid.json 缺锚点 {part_id}"));
            assert_point_pins(&anchor.point, ax, ay, &format!("锚点 {part_id}"));
        }
    }

    // ───────────── plan-race-system-v1 P2c — 合成非人 layout（6 段构型）端到端 ─────────────
    //
    // §P5 whale 草案部位集合（颅 / 躯干 / 背鳍 / 左胸鳍 / 右胸鳍 / 尾鳍）不是生产数据
    // （真实 whale.json 归 P5），本节只验证 P2 的渲染管线在「部位集合与 humanoid 完全
    // 不同」的非人形构型上端到端跑通：BodyPlan（PartBoxes 命中几何，对齐 §P5「只用
    // PartBoxes 模式」）→ BodyPlanLayoutV1 校验 → 加载进 registry → 序列化/反序列化
    // wire 往返 → 坐标不越界 [0,1] + 每个部位红点锚点都落在其剪影多边形范围内。

    fn synthetic_whale_body_plan() -> crate::body_plan::types::BodyPlan {
        use crate::body_plan::types::{
            BodyPartDef, BodyPlan, HitGeometry, PartBox, PartConsequence,
        };

        let parts = vec![
            "skull",
            "torso",
            "dorsal_fin",
            "left_pectoral_fin",
            "right_pectoral_fin",
            "tail_fin",
        ];
        BodyPlan {
            id: BodyPlanId::new("whale_synthetic"),
            display_name: "合成飞鲸测试构型".to_string(),
            is_humanoid: false,
            parts: parts
                .iter()
                .map(|id| BodyPartDef {
                    id: (*id).into(),
                    damage_mul: if *id == "skull" { 2.0 } else { 1.0 },
                    contam_mul: 1.0,
                    bleed_mul: 1.0,
                    consequence: match *id {
                        "skull" => PartConsequence::Sensory,
                        "tail_fin" => PartConsequence::Locomotion,
                        "torso" => PartConsequence::Core,
                        _ => PartConsequence::Manipulator { main_hand: false },
                    },
                })
                .collect(),
            hit_geometry: HitGeometry::PartBoxes {
                boxes: parts
                    .iter()
                    .map(|id| PartBox {
                        part_id: (*id).into(),
                        offset: [0.0, 1.0, 0.0],
                        half_extents: [0.5, 0.5, 0.5],
                        priority: 0,
                    })
                    .collect(),
            },
            equip_slots: vec![],
            meridian_profile: None,
            mutation_slot_mapping: HashMap::new(),
        }
    }

    /// 6 段部位各配一个 3 点最小三角剪影 + 落在剪影内的锚点——覆盖率与
    /// `synthetic_whale_body_plan()` 的部位集合完全一致（非 humanoid 的 8/16 段）。
    fn synthetic_whale_layout() -> BodyPlanLayoutV1 {
        // 沿 x 轴从左到右排布 6 段互不重叠的三角形，锚点取各自重心（必然落在三角形内）。
        let slots: [(&str, f64); 6] = [
            ("skull", 0.0),
            ("torso", 0.15),
            ("dorsal_fin", 0.3),
            ("left_pectoral_fin", 0.45),
            ("right_pectoral_fin", 0.6),
            ("tail_fin", 0.75),
        ];
        let silhouette = slots
            .iter()
            .map(|(id, x0)| BodyPlanSilhouettePartV1 {
                part_id: (*id).to_string(),
                polygon: vec![p(*x0, 0.1), p(x0 + 0.1, 0.1), p(x0 + 0.05, 0.9)],
            })
            .collect();
        let anchors = slots
            .iter()
            .map(|(id, x0)| BodyPlanPartAnchorV1 {
                part_id: (*id).to_string(),
                // 三角形 (x0,0.1) (x0+0.1,0.1) (x0+0.05,0.9) 的重心。
                point: p(x0 + 0.05, 0.1 + (0.9 - 0.1) / 3.0),
            })
            .collect();
        let part_display_map = slots
            .iter()
            .map(|(id, _)| BodyPlanPartDisplayMappingV1 {
                server_part_id: (*id).to_string(),
                display_segment_id: (*id).to_string(),
            })
            .collect();
        BodyPlanLayoutV1 {
            body_plan_id: "whale_synthetic".to_string(),
            silhouette,
            anchors,
            meridian_paths: vec![],
            part_display_map,
        }
    }

    #[test]
    fn synthetic_six_part_non_humanoid_layout_validates_and_loads_into_registry() {
        let body_plans = BodyPlanRegistry::from_plans(vec![synthetic_whale_body_plan()])
            .expect("synthetic whale plan (6 PartBoxes 部位) must validate as a BodyPlan");
        let dir = tempdir();
        write_json(&dir, "whale_synthetic.json", &synthetic_whale_layout());

        let registry = BodyPlanLayoutRegistry::load_dir(&dir, &body_plans).expect(
            "synthetic 6-part non-humanoid layout must pass validate_body_plan_layout and load",
        );
        assert_eq!(registry.len(), 1);
        let loaded = registry
            .get(&BodyPlanId::new("whale_synthetic"))
            .expect("loaded registry must contain the synthetic whale layout");
        assert_eq!(
            loaded.silhouette.len(),
            6,
            "6 段构型必须全部保留，不能被静默丢段"
        );
        assert_eq!(loaded.anchors.len(), 6);
        assert_eq!(loaded.part_display_map.len(), 6);

        cleanup(&dir);
    }

    #[test]
    fn synthetic_six_part_non_humanoid_layout_coordinates_stay_within_unit_range() {
        let layout = synthetic_whale_layout();
        for part in &layout.silhouette {
            for v in &part.polygon {
                assert!(
                    (0.0..=1.0).contains(&v.x) && (0.0..=1.0).contains(&v.y),
                    "非人形合成构型剪影顶点必须归一化落在 [0,1]，部位 {} 得到 ({}, {})",
                    part.part_id,
                    v.x,
                    v.y
                );
            }
        }
        for anchor in &layout.anchors {
            assert!(
                (0.0..=1.0).contains(&anchor.point.x) && (0.0..=1.0).contains(&anchor.point.y),
                "非人形合成构型锚点必须归一化落在 [0,1]，部位 {} 得到 ({}, {})",
                anchor.part_id,
                anchor.point.x,
                anchor.point.y
            );
        }
    }

    /// 每个部位的红点锚点必须落在**该部位自己的**剪影多边形内（而非任意其它段），
    /// 用简单的射线法（三角形专属：叉积同号判定）核验——防止「渲染不越界」退化成
    /// 「坐标凑巧都在 [0,1] 但锚点其实点在别的段上」这种更隐蔽的错位。
    #[test]
    fn synthetic_six_part_non_humanoid_layout_anchor_falls_within_its_own_silhouette() {
        fn sign(p1: &BodyPlanPoint2V1, p2: &BodyPlanPoint2V1, p3: &BodyPlanPoint2V1) -> f64 {
            (p1.x - p3.x) * (p2.y - p3.y) - (p2.x - p3.x) * (p1.y - p3.y)
        }
        fn point_in_triangle(pt: &BodyPlanPoint2V1, tri: &[BodyPlanPoint2V1]) -> bool {
            assert_eq!(tri.len(), 3, "fixture 三角形固定 3 顶点");
            let d1 = sign(pt, &tri[0], &tri[1]);
            let d2 = sign(pt, &tri[1], &tri[2]);
            let d3 = sign(pt, &tri[2], &tri[0]);
            let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
            let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
            !(has_neg && has_pos)
        }

        let layout = synthetic_whale_layout();
        for anchor in &layout.anchors {
            let silhouette = layout
                .silhouette
                .iter()
                .find(|s| s.part_id == anchor.part_id)
                .unwrap_or_else(|| panic!("锚点部位 {} 必须有对应剪影段", anchor.part_id));
            assert!(
                point_in_triangle(&anchor.point, &silhouette.polygon),
                "部位 {} 的锚点 ({}, {}) 必须落在自己的剪影多边形内，不能漂到相邻段",
                anchor.part_id,
                anchor.point.x,
                anchor.point.y
            );
        }
    }

    #[test]
    fn synthetic_six_part_non_humanoid_layout_survives_json_wire_round_trip() {
        let layout = synthetic_whale_layout();
        let wire = serde_json::to_string(&layout).expect("layout must serialize to JSON wire");
        let decoded: BodyPlanLayoutV1 =
            serde_json::from_str(&wire).expect("wire JSON must decode back into BodyPlanLayoutV1");
        assert_eq!(
            layout, decoded,
            "非人形合成 layout 经 JSON wire 往返（serialize→deserialize）必须逐字段相等"
        );

        // 往返之后仍必须通过跨 registry 校验（防止序列化丢字段导致的隐性损坏）。
        let body_plans = BodyPlanRegistry::from_plans(vec![synthetic_whale_body_plan()])
            .expect("synthetic whale plan must validate");
        let plan = body_plans
            .get(&BodyPlanId::new("whale_synthetic"))
            .expect("synthetic whale plan must be registered");
        assert!(
            validate_body_plan_layout(&decoded, plan).is_ok(),
            "wire 往返解码后的 layout 必须仍能通过 validate_body_plan_layout"
        );
    }

    // ───────────── plan-race-system-v1 P2c — 缺段 / 冗余段 wire 容错 ─────────────
    //
    // 部位集合不完全重叠（layout 声明的部位 ≠ 实际会在 wounds/status wire 上出现的
    // server 部位 id 集合）是非人形构型的常态——不同种族的 wounds 系统仍按各自
    // `BodyPlan.parts` 上报，layout 只是渲染层数据，二者不保证 1:1。这里锁定的是
    // **server 端**校验/加载层面对该错配的容错边界：display_map 引用未声明的
    // wounds 部位（冗余段——layout 比 plan.parts 多）在**加载期**是 fail-fast 悬空
    // 引用（已有 `part_display_map_unknown_server_part_rejected` 覆盖）；而"缺段"
    // ——plan 声明了部位但 layout 未覆盖（如此处 whale 6 段只给 4 段配 anchor/
    // silhouette）——必须是合法状态，不能因为没覆盖全部 parts 就在加载期被拒绝
    // （client 侧渲染缺段时的 fallback 行为见
    // `MiniBodyHudPlannerGeometryTest.partMissingFromLoadedLayoutFallsBack` /
    // `BodyInspectComponentGeometryTest.bodyPartMissingFromLayoutFallsBackToHardcodedTable`）。
    #[test]
    fn layout_covering_only_a_subset_of_plan_parts_is_valid_missing_segments_are_not_an_error() {
        let plan = synthetic_whale_body_plan();
        let body_plans =
            BodyPlanRegistry::from_plans(vec![plan]).expect("synthetic whale plan must validate");

        let mut partial = synthetic_whale_layout();
        // 只保留前 2 段（skull/torso），故意漏掉 dorsal_fin/left_pectoral_fin/
        // right_pectoral_fin/tail_fin 4 段——这是"缺段"场景。
        partial.silhouette.truncate(2);
        partial.anchors.truncate(2);
        partial.part_display_map.truncate(2);

        let dir = tempdir();
        write_json(&dir, "whale_partial.json", &partial);
        let registry = BodyPlanLayoutRegistry::load_dir(&dir, &body_plans).expect(
            "layout 覆盖 plan.parts 的一个真子集必须合法加载——缺段不是校验错误，是渲染层 fallback 的正常输入",
        );
        let loaded = registry
            .get(&BodyPlanId::new("whale_synthetic"))
            .expect("partial layout must still load under its body_plan_id");
        assert_eq!(
            loaded.silhouette.len(),
            2,
            "缺段 layout 必须原样保留其声明的子集，不做隐式补全"
        );
        cleanup(&dir);
    }

    #[test]
    fn layout_part_display_map_referencing_a_part_absent_from_the_target_plan_is_rejected_as_dangling(
    ) {
        // "冗余段"——layout 引用了一个 plan.parts 里根本不存在的部位 id——必须在
        // 加载期 fail-fast（不是静默丢弃，也不是放到 client 才炸），呼应
        // `validate_body_plan_layout` 文档中 `part_display_map` 悬空引用检测。
        let plan = synthetic_whale_body_plan();
        let body_plans =
            BodyPlanRegistry::from_plans(vec![plan]).expect("synthetic whale plan must validate");

        let mut redundant = synthetic_whale_layout();
        redundant
            .part_display_map
            .push(BodyPlanPartDisplayMappingV1 {
                server_part_id: "phantom_blowhole".to_string(),
                display_segment_id: "phantom_blowhole".to_string(),
            });

        let dir = tempdir();
        write_json(&dir, "whale_redundant.json", &redundant);
        let err = BodyPlanLayoutRegistry::load_dir(&dir, &body_plans)
            .expect_err("引用 plan 未声明部位的冗余 display_map 条目必须被拒绝，不能悄悄放过");
        match err {
            BodyPlanLayoutLoadError::Invalid { reason, .. } => {
                assert!(reason.contains("unknown server part id"), "got: {reason}");
            }
            other => panic!("expected Invalid, got {other:?}"),
        }
        cleanup(&dir);
    }

    #[test]
    fn humanoid_layout_meridian_paths_pin_client_meridian_paths_verbatim() {
        let layout = humanoid_layout_static();
        let expected_paths = client_meridian_paths();
        assert_eq!(
            layout.meridian_paths.len(),
            expected_paths.len(),
            "humanoid.json 经脉折线条数必须与 client MERIDIAN_PATHS 的 20 条一致"
        );
        for (channel_id, waypoints) in expected_paths {
            let path = layout
                .meridian_paths
                .iter()
                .find(|p| p.channel_id == channel_id)
                .unwrap_or_else(|| panic!("humanoid.json 缺经脉折线 {channel_id}"));
            assert_eq!(
                path.points.len(),
                waypoints.len(),
                "经脉 {channel_id} 折线点数必须与 client 原表一致"
            );
            for (i, [wx, wy]) in waypoints.into_iter().enumerate() {
                assert_point_pins(
                    &path.points[i],
                    wx,
                    wy,
                    &format!("经脉 {channel_id} 路径点 #{i}"),
                );
            }
        }
    }
}
