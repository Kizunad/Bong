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
- **P2 运行时归属 pin**：`server/src/world/zone.rs` 的 `north_rift_and_scorch_are_adjacent_but_mutually_exclusive`，以及 `server/src/audio/ambient.rs`、`server/src/lingtian/weather_profile.rs`、`server/src/movement/mod.rs`、`server/src/tribulation/scorch_record.rs`、`server/src/world/tsy_integration_test.rs` 对生产坐标消费链的专属回归。
- **真实协议 bot**：`scripts/bot/proto_min.py` 解码权威 `ZoneInfo`，`scripts/bot/scenarios/terrain_north_rift_scorch_zone_identity.py` 通过真实 MC `/preview_tp`、`PlayerPositionLook`、`bong:server_data zone_info` 与 `bong:audio/ambient_zone` 对拍焦土旧点、迁移后渊口和焦土边界；`scripts/bot/run_scenarios.py` 将其隔离为 dedicated scenario。
- **完整 e2e 接线**：`scripts/e2e-redis.sh` 在普通 100 NPC/Tiandao 闭环之后停服，另起 `BONG_PREVIEW_MODE=1` 的无 rogue seed release server 跑唯一北荒场景，并把 preview server / bot 日志写入 manifest。
- **本地验真证据**：`.tmp/pr1207-gates-c86305882d33/` 保留 Rust、worldgen/dev-reload、Python bot、全部 e2e 成败轮与最终 PASS；`fabric-d198a7e56cfb/` 保留真实 Fabric client/server 全日志、双点 HUD/F3 截图和 SHA-256 摘录。失败轮与缓存修复轮均未删除。

### 关键 commit

- `b6d6bdf1`（2026-07-14）— 最终同步北荒渊口 runtime/blueprint 几何与 anchor，中心落在 Z=`-7300`。
- `d2b29f45`（2026-07-14）— 同步迁移后的渊口统一场 Qi 烘焙结果 `0.068602`。
- `96d61878`（2026-07-14）— 补齐相邻焦土统一场 Qi 烘焙结果 `0.290146` 及双 zone 对拍。
- `f42824dd`、`67507e63`、`f0953275`（2026-07-14）— 建立 overlap 策略、known-defect 基线并封堵缺失设计 overlap 的假绿。
- `bd968115`（2026-07-14）— 将三条 overlap 守护迁为 unittest 并纳入 worldgen preview CI。
- `2ef556e3`、`f0b33148`（2026-07-14）— 收紧运行时点位/边界断言，并直接 pin 两块 AABB 严格分离。
- `239af8d5`（2026-07-14）— 修正运行时 pin 的 AABB 边界解构并保持断言可编译。
- `6fb53819`（2026-07-14）— 无冲突合并 `origin/main@4ad0c170`，在最终合并后 HEAD 上完成本次全量验收。
- `2568c134`、`6cb161c5`、`1423cc4c`、`1d75be96`（2026-07-14）— 补齐位面契约、生产 zone 合并语义、天气/环境归属与生产传送链。
- `6d7ae8ee`（2026-07-15）— 修正 `scripts/dev-reload.sh` 后台任务脱离，保证 regen→validate→build→restart 的失败可见性。
- `0097f7a2`（2026-07-17）— 钉住北荒三个生产坐标在 zone、天气、环境音、移动和渡劫焦土记录中的消费语义。
- `9b631a93`（2026-07-17）— 接通 `ZoneInfo` minimal protobuf decoder、dedicated 真实 bot 场景与 e2e preview phase。
- `c8630588`（2026-07-17）— 合并 `origin/main@062cf636`，兼容最新 movement 拒绝测试后复验。
- `d198a7e5`（2026-07-17）— 把 bot 音景断言收紧为 state-aware 契约：`AMBIENT→ambient_wilderness`、`CULTIVATION→cultivation_meditate`，并拒绝 crossed pair、`COMBAT`、`TSY`、`TRIBULATION`、unknown 与缺失状态。

### 测试结果

- Python overlap policy：3/3 通过；覆盖 runtime/blueprint 全量 pair、最终几何与 anchors、渊口与相邻焦土统一场 Qi bake。
- Git whitespace：`git diff --check` 通过。
- Worldgen/dev reload：`bash scripts/dev-reload.sh` 完整执行四阶段；overworld 306 tiles 与 TSY 9 tiles 均通过 raster 后验，server dev build 与 restart 成功。真实 preview runtime 随后从同一 manifest 加载 306 terrain tiles / 84 POIs / 112 decorations / 138969 placements，另加载 TSY 9 terrain tiles / 56 POIs。
- Rust zone 窄测：45 passed，0 failed，0 ignored（初始归档门禁）。
- Rust 最终完整门禁（`d198a7e56cfb`，2026-07-17）：`cargo fmt --check` 通过；`cargo clippy --all-targets -- -D warnings` 通过；lib 11719 passed / 0 failed / 1 ignored，main 11 passed，`full_app_startup` 1 passed，`tarkov_backpack_p0_e2e` 4 passed，doc tests 0 failed / 5 ignored。日志：`.tmp/pr1207-gates-c86305882d33/server-full-gate-d198a7e56cfb.log`。
- Python bot protocol：`python3 -m unittest scripts.bot.test_protocol -v` 为 124/124 PASS；其中 `NorthRiftScenarioContractTest` 定向 8/8 PASS，覆盖三点顺序、权威坐标、zone/perception、两种合法音乐状态与全部拒绝分支。
- 真实 Redis/server/Tiandao/protocol-bot e2e（`d198a7e56cfb`）：run id `20260717-093000-1902656-pr1207-d198a7e56cfb-final-hot`，manifest `status=PASS` / `stage=complete`，17 passed / 0 failed；100 NPC TPS=20.0；dedicated north-rift `preview_tp + zone_info + ambient_zone` bot PASS 且专用 server 完整清理。最终 manifest：`.tmp/pr1207-gates-c86305882d33/task-13-e2e-redis-manifest-d198-final-hot-pass.txt`。
- 真实 Fabric renderer/runtime（`d198a7e56cfb`）：Java 17.0.19、Fabric 1.20.1、Mesa llvmpipe，在 `DISPLAY=:99` 真实连接预览服；client 收到 `north_waste_east_scorch` 与 `rift_mouth_north_002` 的 `zone_info`，并分别处理 `CULTIVATION/cultivation_meditate` 的 `ambient_zone`。server 权威日志记录 `/preview_tp 2000 74 -7800 0 0` 与 `/preview_tp 2000 74 -7303 0 0`；四张 1280×720 HUD/F3 截图已人工查看，F3 命中 X=2000、Z=-7800/-7303，且两处真实 raster 地貌可辨。证据：`.tmp/pr1207-gates-c86305882d33/fabric-d198a7e56cfb/fabric-runtime-evidence.txt`。取证完成后显式终止长期运行的 `runClient`，因此 Gradle 末尾 exit 143 仅表示人工停进程，不计作 build PASS。

### 跨仓库核验

- **server**：`ZoneRegistry::find_zone`、`ZoneRegistry::zones_are_adjacent` 与 `north_rift_and_scorch_are_adjacent_but_mutually_exclusive` 在合并后 HEAD 上共同通过。
- **worldgen / CI**：`ZoneOverlapPolicyTest` 的全局 overlap 策略、几何/anchor 对拍、统一场 Qi bake 对拍 3/3 通过，且 `.github/workflows/worldgen-preview.yml` 已显式纳入该测试。
- **server 生产消费者**：环境音、天气 profile、移动焦土语义、渡劫焦土记录、TSY/Overworld 合并与 preview teleport 都以真实 `ZoneRegistry` 和生产坐标通过专属 pin；没有以 mock zone 替代归属证明。
- **Python bot / MC protocol**：minimal decoder 的 oneof/字段号与 `proto/bong/envelope.proto::ServerDataEnvelope.zone_info`、`ZoneInfo` 对拍；真实 bot 同时要求 `PlayerPositionLook`、`zone_info` 与 `ambient_zone` 在同一 watermark 后一致，拒绝 stale/wrong packet 和错误音乐状态。
- **agent / Redis / e2e**：非 mock Tiandao one-tick、`bong:world_state`、`bong:agent_command`、`bong:agent_narrate`、server command anchor 与 100 NPC TPS 门禁均在同一 17/17 run 中通过；北荒 dedicated bot 是其后独立 preview phase，不污染普通 TPS server。
- **client / Fabric**：本 plan 未改 client source 或 schema，但已用真实 Java 17 Fabric runtime 消费 server payload、切换两处 zone/audio 状态并实际渲染同一 raster 世界；截图、client log、server log 三方互证，不以 headless mock 代替 renderer 证据。

### 遗留 / 后续

- 功能与本地跨栈验收无遗留；本次证据 commit 后仍须对新的最终 HEAD 重新执行无上下文 validator、push、独立 `/review` 评论与 GitHub e2e，再按 review gate 合并 #1207。该流程不会删除现有 worktree、分支、缓存或历史失败轮证据。
- `giant_sword_sea` / `wuxing_abyss` 仍是独立 plan 所有的已知缺陷，本 plan 不跨界修复或加入设计白名单。

## 风险

- 移动渊口可能破坏 `target_family_pos_xyz` 的空间叙事、portal_anchor、截图路线或玩家已知坐标；修复应优先做小位移或缩边，而不是重排整片北荒。
- 移动焦土边界可能影响 `ascension_pit_xz`、巡逻锚点、天气 profile 覆盖和化虚遗迹区域大小；必须保证化虚遗迹仍在 `north_waste_east_scorch` 内。
- 加 overlap guard 时要保留历史合法嵌套，否则会误伤 `rift_mouth_blood_001` 嵌入血谷等既定设计。

## 审计来源

BugHunt F6 worldgen 第六轮（2026-07-06）。本轮按要求只新增 skeleton plan，不消费/归档 plan，不修改实际代码、配置、依赖或资源；已先查开放 PR 并避开 #971 / #986 / #992 / #998 / #1008。
