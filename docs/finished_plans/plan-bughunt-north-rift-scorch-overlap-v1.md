# plan-bughunt-north-rift-scorch-overlap-v1（Finished）

> **Finished（P0/P1/P2 已完成并通过 worldgen、server 窄测与完整 Rust 门禁）**。一句话主题：消除 `rift_mouth_north_002` 与 `north_waste_east_scorch` 的意外 3-D AABB 重叠，并用 worldgen 全局策略与 server 运行时 pin 防止回归。

## 阶段总览

| 阶段 | 状态 | 可核验结果 |
|---|---|---|
| P0 数据修正 | ✅ 2026-07-14 | 渊口最终中心改为 `[2000,-7300]`，运行时与 blueprint AABB/anchors/portal/POI 同步，渊口/焦土统一场烘焙值同步为 `0.068602` / `0.290146` |
| P1 worldgen 守护 | ✅ 2026-07-14 | 非白名单 3-D overlap 守护、known-defect 基线、几何/统一场对拍均纳入 `unittest` 与 CI |
| P2 runtime pin | ✅ 2026-07-14 | server pin 已补齐严格分离、邻接、归属与边界断言；zone 窄测与完整 `fmt + clippy -D warnings + test` 门禁全绿 |

## Bug 摘要

- **核心 bug（修复前）**：`rift_mouth_north_002` AABB 为 `[1850,50,-7950]..[2150,100,-7650]`，完整落入 `north_waste_east_scorch` 的 `[1500,60,-8500]..[2700,100,-7500]` 的 Y=60..100 切片。旧入口点 `[2000,74,-7800]` 同时命中两者。
- **运行时结果（修复前）**：`ZoneRegistry::find_zone` 对重叠命中取 AABB 体积最小的 zone，所以旧入口点返回 `rift_mouth_north_002`，不是 `north_waste_east_scorch`。
- **设计冲突**：归档 `plan-terrain-rift-mouth-v1` 明确写 `rift_mouth_north_002` 是化虚遗迹邻接区，并备注“zone 互斥不重叠”，不是合法嵌套。
- **非重复项**：不是 #986（`giant_sword_sea` / `wuxing_abyss` 重叠），也不是 #998 TSY Y 分层、#1008 pipeline cwd、#971 mineral anchors 或 #992 start.sh 环境变量。

## 对实际游玩体验的影响

玩家靠近北荒东陲塌缩裂缝时，画面位置仍在“北荒东陲焦土”大区内，但服务端把玩家归属成小的渊口 zone。结果是焦土区应有的雷暴 profile、焦土脚感、环境音和渡劫焦土记录在裂缝附近被遮蔽；玩家会看到一个世界观上应当“焦土与化虚遗迹邻接”的地点，实际却像从焦土 zone 中被挖掉一块，相关反馈和后续事件不连续。

## 修复前证据定位

- `server/zones.json:60`：`rift_mouth_north_002` AABB `[1850,50,-7950]..[2150,100,-7650]`，patrol/portal anchor `[2000,74,-7800]`。
- `server/zones.json:567`：`north_waste_east_scorch` AABB `[1500,60,-8500]..[2700,100,-7500]`，active_events 含 `tribulation_scorch` / `tianjie_ascension_pit`。
- `server/zones.worldview.example.json:273` 与 `server/zones.worldview.example.json:403`：blueprint 同样重叠，说明不是只写坏了运行时导出。
- `docs/finished_plans/plan-terrain-rift-mouth-v1.md:197`：该渊口被定义为化虚遗迹邻接区，且“zone 互斥不重叠”。
- `server/src/world/zone.rs:301`：`find_zone` 过滤同维度命中后按 `aabb_volume()` 取最小 zone。
- `server/src/audio/ambient.rs:232`：环境音用 `find_zone` 得到的 zone 决定 `zone_name` / `recipe_id`。
- `server/src/movement/mod.rs:872`、`server/src/movement/mod.rs:902`：移动 zone kind 取 `find_zone`，只有命中 zone 名含 ash 或事件含 `tribulation_scorch` / `no_cadence` 才允许焦土表面特殊脚感。
- `server/src/tribulation/scorch_record.rs:59`、`server/src/tribulation/scorch_record.rs:79`：渡劫焦土记录先 `find_zone`，再判断命中 zone 是否 scorch。
- `server/src/world/weather_physics/vision.rs:67`、`server/src/world/weather_physics/vision.rs:79` 与 `server/weather_profiles.json:8`：天气视距/profile 也按命中 zone name 查询，`north_waste_east_scorch` 有专门雷暴 profile，渊口没有。

## 修复前触发路径

1. 启动默认 `ZoneRegistry::load()`，加载 `server/zones.json` 并合并 TSY blueprint。
2. 玩家或 NPC 位于 `[2000,74,-7800]` 附近，即 `rift_mouth_north_002` 的入口/巡逻锚点。
3. `find_zone(Overworld, pos)` 同时命中 `rift_mouth_north_002` 和 `north_waste_east_scorch`。
4. 因小 AABB 体积更小，返回 `rift_mouth_north_002`。
5. 后续环境音、移动、天气视距、渡劫焦土记录、风险热力图等都按渊口 zone 消费，北荒焦土语义在重叠区域被遮蔽。

## 反方审查记录

### Round 1

- **反方尝试**：也许这是合法嵌套；`find_zone` 注释确实支持 `rift_mouth_blood_001` 嵌入 `blood_valley` 一类设计。
- **裁决**：未推翻。合法嵌套的历史例子不包含 `rift_mouth_north_002` / `north_waste_east_scorch`，而该 pair 的归档设计明确要求互斥不重叠。

### Round 2

- **反方尝试**：即使几何重叠，也可能只是归属显示差异，实际体验影响不足。
- **裁决**：未推翻。至少环境音、焦土移动、天气视距、渡劫焦土记录都直接消费 `find_zone` 的命中 zone；入口点会把焦土反馈切成渊口反馈，属于可达玩家体验 bug。

## 实施状态

### P0 数据修正 — ✅ 2026-07-14

- 最终采用中心 `[2000,-7300]`，取代骨架中的旧中心/候选 `[2000,-7800]`；没有修改全局 `find_zone` 的最小 AABB 语义。
- `server/zones.json` 与 `server/zones.worldview.example.json` 的 `rift_mouth_north_002` 均同步为 AABB `[1850,50,-7450]..[2150,100,-7150]`。`north_waste_east_scorch` 的北侧边界仍为 Z=`-7500`，两者沿 Z 轴保留 `50` 格间隙，保持邻接但不重叠。
- runtime `patrol_anchors`、blueprint `center_xz` / `patrol_anchors` / `worldgen.portal_anchor_xz` / 首个 `rift_portal` POI 全部同步到 `[2000,74,-7300]`（XZ 字段为 `[2000,-7300]`）；焦土 ascension pit 仍保持 `[2100,80,-8000]`。
- 迁移后重新对拍统一场导出，`server/zones.json` 的 `rift_mouth_north_002.spirit_qi` 为 `0.068602`，相邻 `north_waste_east_scorch.spirit_qi` 为 `0.290146`；blueprint 仍保存输入权重 `0.05` / `0.28`，runtime 保存烘焙结果。

### P1 worldgen 守护 — ✅ 2026-07-14

- `worldgen/tests/test_zone_overlap_policy.py` 对 runtime/blueprint 同时扫描所有 zone pair，拒绝任何既不在设计白名单、也不在已知缺陷集合中的 3-D overlap。
- `DESIGNED_OVERLAPS` 仅允许以下已审设计嵌套：
  - `rift_mouth_blood_001` / `blood_valley`
  - `rift_mouth_north_001` / `jiuzong_beiling_ruin`
  - `baolongwang_cavern_deep` / `zhanhun_plain`
  - `blood_valley` / `zhanhun_plain`
  - `north_waste_east_scorch` / `north_wastes`
- `KNOWN_DEFECT_OVERLAPS_BY_FILE` 仅在 `zones.json` 保留由 `plan-bughunt-sword-sea-zone-overlap-v1` 负责的 `giant_sword_sea` / `wuxing_abyss`；`zones.worldview.example.json` 的 known-defect 集合为空。`rift_mouth_north_002` / `north_waste_east_scorch` 不在任一集合中。
- 三条守护已迁为 `unittest.TestCase`，并由 `.github/workflows/worldgen-preview.yml` 显式 discover：全局 overlap 策略、北荒渊口几何/anchor 对拍、渊口与相邻焦土统一场 Qi bake 对拍。

### P2 runtime pin — ✅ 2026-07-14

- `server/src/world/zone.rs::north_rift_and_scorch_are_adjacent_but_mutually_exclusive` 已直接断言两块 AABB 至少沿一个轴严格分离，同时保留 `zones_are_adjacent(..., 100.0)`。
- 运行时归属 pin 覆盖：新 portal anchor `[2000,74,-7300]` 命中渊口；旧点 `[2000,74,-7800]` 与 ascension pit `[2100,80,-8000]` 均只命中焦土；Z=`-7500` 焦土边界不再被渊口遮蔽；渊口 inclusive min/max 边界仍可达。
- `cargo test world::zone` 实跑覆盖 45 条 zone 测试，新增 runtime pin 与既有最小 AABB 归属、维度过滤、邻接语义共同通过。
- 完整 server 门禁 `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` 实跑全绿，P2 验收闭环。

## 实际验收记录

- ✅ `cd worldgen && python3 -m unittest discover -s tests -p 'test_zone_overlap_policy.py' -v`：`Ran 3 tests`，全部 `OK`（2026-07-14）。
- ✅ `git diff --check`：通过（2026-07-14）。
- ✅ `cd server && cargo test world::zone`：45 passed，0 failed，0 ignored（2026-07-14）。
- ✅ `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`：fmt 通过；clippy 0 warning；lib 11648 passed / 0 failed / 1 ignored，main 11 passed，`full_app_startup` 1 passed，`tarkov_backpack_p0_e2e` 4 passed，doc tests 0 failed / 5 ignored（2026-07-14）。
- ⏸ `bash scripts/smoke-test-e2e.sh`：按调度约束留给主 agent 在 PR 后串行执行，不作为本次 P2 本地归档前置门禁。

## Finish Evidence

### 落地清单

- **P0 数据与统一场**：`server/zones.json`、`server/zones.worldview.example.json`。
- **P1 overlap 策略与 CI**：`worldgen/tests/test_zone_overlap_policy.py`、`.github/workflows/worldgen-preview.yml`。
- **P2 运行时归属 pin**：`server/src/world/zone.rs` 的 `north_rift_and_scorch_are_adjacent_but_mutually_exclusive`。

### 关键 commit

- `b6d6bdf1`（2026-07-14）— 最终同步北荒渊口 runtime/blueprint 几何与 anchor，中心落在 Z=`-7300`。
- `d2b29f45`（2026-07-14）— 同步迁移后的渊口统一场 Qi 烘焙结果 `0.068602`。
- `96d61878`（2026-07-14）— 补齐相邻焦土统一场 Qi 烘焙结果 `0.290146` 及双 zone 对拍。
- `f42824dd`、`67507e63`、`f0953275`（2026-07-14）— 建立 overlap 策略、known-defect 基线并封堵缺失设计 overlap 的假绿。
- `bd968115`（2026-07-14）— 将三条 overlap 守护迁为 unittest 并纳入 worldgen preview CI。
- `2ef556e3`、`f0b33148`（2026-07-14）— 收紧运行时点位/边界断言，并直接 pin 两块 AABB 严格分离。
- `239af8d5`（2026-07-14）— 修正运行时 pin 的 AABB 边界解构并保持断言可编译。
- `6fb53819`（2026-07-14）— 无冲突合并 `origin/main@4ad0c170`，在最终合并后 HEAD 上完成本次全量验收。

### 测试结果

- Python overlap policy：3/3 通过；覆盖 runtime/blueprint 全量 pair、最终几何与 anchors、渊口与相邻焦土统一场 Qi bake。
- Git whitespace：`git diff --check` 通过。
- Rust zone 窄测：45 passed，0 failed，0 ignored。
- Rust 完整门禁：fmt 通过；clippy `--all-targets -- -D warnings` 通过；lib 11648 passed / 0 failed / 1 ignored，main 11 passed，`full_app_startup` 1 passed，`tarkov_backpack_p0_e2e` 4 passed，doc tests 0 failed / 5 ignored。

### 跨仓库核验

- **server**：`ZoneRegistry::find_zone`、`ZoneRegistry::zones_are_adjacent` 与 `north_rift_and_scorch_are_adjacent_but_mutually_exclusive` 在合并后 HEAD 上共同通过。
- **worldgen / CI**：`ZoneOverlapPolicyTest` 的全局 overlap 策略、几何/anchor 对拍、统一场 Qi bake 对拍 3/3 通过，且 `.github/workflows/worldgen-preview.yml` 已显式纳入该测试。
- **agent / client**：本 plan 不改跨端协议、schema 或客户端资产，无需跨栈门禁。

### 遗留 / 后续

- PR 后的 `/review` 与 `smoke-test-e2e.sh` 由主 agent 按串行调度收口；本 plan 的本地实现与 P2 Rust 门禁已完成。
- `giant_sword_sea` / `wuxing_abyss` 仍是独立 plan 所有的已知缺陷，本 plan 不跨界修复或加入设计白名单。

## 风险

- 移动渊口可能破坏 `target_family_pos_xyz` 的空间叙事、portal_anchor、截图路线或玩家已知坐标；修复应优先做小位移或缩边，而不是重排整片北荒。
- 移动焦土边界可能影响 `ascension_pit_xz`、巡逻锚点、天气 profile 覆盖和化虚遗迹区域大小；必须保证化虚遗迹仍在 `north_waste_east_scorch` 内。
- 加 overlap guard 时要保留历史合法嵌套，否则会误伤 `rift_mouth_blood_001` 嵌入血谷等既定设计。

## 审计来源

BugHunt F6 worldgen 第六轮（2026-07-06）。本轮按要求只新增 skeleton plan，不消费/归档 plan，不修改实际代码、配置、依赖或资源；已先查开放 PR 并避开 #971 / #986 / #992 / #998 / #1008。
