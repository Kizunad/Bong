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
