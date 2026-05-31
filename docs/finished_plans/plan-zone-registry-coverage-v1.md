# plan-zone-registry-coverage-v1 — 运行时 zone 注册表对齐全地图蓝图

**主题**：`server/zones.json`（运行时权威源，10 zone）严重落后于 `zones.worldview.example.json`（地图蓝图，26 zone），导致 `/zones`、`/tpzone` 及一切按 zone 名查找的运行时系统（zong_keeper / zong_formation / mob_spawn / extract_system / weather / tribulation）覆盖不全；同时修 `find_zone` 嵌套 zone 命中语义、`wangyintai` 与 `blood_valley` AABB 重叠 bug，并把 `danger_level` 上限从 5 放宽到 7 以保留蓝图的危险度梯度（跨 server/agent/client 三仓 IPC 契约）。

| 阶段 | 内容 | 状态 | 验收日期 |
|------|------|------|---------|
| P0 | `danger_level` 上限 5→7（server 常量 + schema 3 处 + client 2 套 HUD + generated） | ✅ 2026-06-01 | |
| P1 | `find_zone` 嵌套命中返回最小 AABB zone + pin 测试 | ✅ 2026-06-01 | |
| P2 | `zones.json` 补齐至 28 zone（现有10 + 蓝图缺18）、按面积升序、wangyintai 挪坐标 | ✅ 2026-06-01 | |
| P3 | wangyintai pin 测试改坐标 + 新增覆盖对齐/嵌套取小 pin 测试 | ✅ 2026-06-01 | |
| P4 | 全量回归（server fmt+clippy+test / schema test / client build） | ✅ 2026-06-01 | |

## 背景：根因与三层地理现实

`server/src/world/zone.rs` `DEFAULT_ZONES_PATH = "zones.json"` —— server 运行时只加载 `server/zones.json`。完整地图蓝图在 `server/zones.worldview.example.json`（worldgen 流水线种子），server 不读它，两份各自漂移：

- **可玩层** `zones.json`：10 zone（运行时真实）。
- **蓝图层** `zones.worldview.example.json`：26 zone（仅 worldgen 生成 raster）。
- 差集：蓝图有、运行时缺 **18 个**（celestial_isles / south_ash_dead_zone / zhanhun_plain / wuxing_abyss / 7×jiuzong_*_ruin / 4×rift_mouth_* / north_waste_east_scorch / blood_valley_east_scorch / drift_scorch_001）；运行时独有 **2 个**（giant_sword_sea / wangyintai，蓝图无）。

**14/18 缺失 zone 已有 Rust 代码按名引用**：`jiuzong_*`→`npc/zong_keeper.rs`/`worldgen/zong_formation.rs`/`cultivation/qi_field.rs`；`south_ash_dead_zone`→`world/mob_spawn.rs`/`movement/mod.rs`；`rift_mouth_north_001`→`world/extract_system.rs`；`*_scorch`→`lingtian/weather_profile.rs`/`tribulation/scorch_record.rs`。这些系统当前按名 `find_zone_by_name` 落空，是真 bug。

## P0 — danger_level 上限 5→7

**根因**：`MAX_ZONE_DANGER_LEVEL = 5`。蓝图 8 个 zone danger=6、`north_waste_east_scorch`=7。直接灌入触发 `validate_zone` 失败 → 整个 zones.json fallback 到只剩 spawn。`danger_level` 是跨三仓 IPC 契约。

**交付物**：`server/src/world/zone.rs` `MAX_ZONE_DANGER_LEVEL: u8 = 7`；`agent/packages/schema/src/{world-state,server-data,client-payload}.ts` `danger_level maximum: 7` + `npm run generate` 同步 6 个 `generated/*.json`；client `com/bong/client/ZoneState.java`（旧）+ `com/bong/client/state/ZoneState.java`（新）`MAX_DANGER_LEVEL=7`、`com/bong/client/hud/BongZoneHud.java` `MAX_DANGER_SYMBOLS=7`。（旧 `com/bong/client/BongZoneHud.java` 无独立常量，委托 `ZoneState.clampDangerLevel`，无需改。proto `danger_level` 是无上限 `uint32`，无需改。）

## P1 — find_zone 嵌套命中返回最小 AABB

**根因**：`ZoneRegistry::find_zone` 取第一个 3D `contains` 命中。补入后存在合法嵌套重叠（rift_mouth ⊂ jiuzong/血谷、血谷 ⊂ 战魂平野、剑海 ⊂ 深渊），第一个命中可能是大 zone。

**交付物**：`Zone::aabb_volume()`（体积 = `(max-min).x*.y*.z`）+ `find_zone` 改 `filter(...).min_by(volume)` 返回最具体 zone。不改 `find_zone_by_name`。

## P2 — zones.json 补齐至 28 zone

**交付物**：`server/zones.json` 10→28，只取 server `ZoneConfig` 字段（`name/aabb/spirit_qi/danger_level/active_events/patrol_anchors/blocked_tiles`），按 AABB 面积升序排列（双保险）。wangyintai 原 `aabb x[3200,4200] z[-2800,-1800]` 与 blood_valley 重叠 → 新址 `aabb min[3500,40,-2150] max[4500,200,-1150]`、center `(4000,120,-1650)`，patrol 平移到 `[[4000,92,-1650],[3800,88,-1850]]`（脚本验证与全部 27 zone 无 XZ 重叠）。新增 18 zone 不设 `ambient_recipe_id`（Option，`ambient.rs:346 ambient_recipe_for_zone` 兜底）。

## P3 — pin 测试

**交付物**：`server/src/world/zone.rs` 改 `wangyintai_bounds_match_plan_spec`（新 aabb）、`wangyintai_center_resolves_to_zone`（新 center）；新增 `zones_json_covers_all_overworld_blueprint_zones`（防漂移：缺 zone 即撞红，断言 28 zone 全命中）+ `find_zone_returns_smallest_containing_zone`（大套小取小）。client 随上限改：`hud/BongZoneHudTest.dangerSymbols(99)`→7☠、`state/ZoneStateTest`/`ZoneStateTest`/`network/ZoneInfoHandlerTest` 的 `dangerLevel` clamp 断言 5→7。

## 跨仓库契约 symbol

- **server**：`MAX_ZONE_DANGER_LEVEL=7`、`ZoneRegistry::find_zone`(min volume)、`Zone::aabb_volume`、`zones.json`(28 zone)
- **agent**：`world-state.ts` / `server-data.ts` / `client-payload.ts` `danger_level maximum=7` + generated JSON Schema
- **client**：`ZoneState.MAX_DANGER_LEVEL`（两套）、`hud/BongZoneHud.MAX_DANGER_SYMBOLS`

## 非目标（不在本 plan）

- TSY 12 zone 接入 `/tpzone`（走 `tsy_dev_command.rs` 动态注册，架构不同，留后续）。
- `zones.json` 与 `zones.worldview.example.json` 自动同步（本 plan 仅加防漂移 pin 测试）。
- worldview 的 `pois/worldgen/display_name` 字段接入运行时。
- HUD danger 符号超过 7 格的视觉重设计（仅扩容上限，不重排版）。

## Finish Evidence

### 落地清单
- **P0**：`server/src/world/zone.rs`（`MAX_ZONE_DANGER_LEVEL`）；`agent/packages/schema/src/{world-state,server-data,client-payload}.ts` + 6 个 `agent/packages/schema/generated/*.json`（agent-world-model-envelope/snapshot、client-payload、client-payload-zone-info、server-data、world-state）；`client/.../ZoneState.java`（旧+新两套）、`client/.../hud/BongZoneHud.java`。
- **P1**：`server/src/world/zone.rs` `Zone::aabb_volume` + `ZoneRegistry::find_zone`(`filter().min_by`)。
- **P2**：`server/zones.json` 10→28；wangyintai 挪到 `(3500..4500, 40..200, -2150..-1150)`，与全部 27 zone 无 XZ 重叠（problem_overlaps=0）。
- **P3**：`server/src/world/zone.rs` 改 2 个 wangyintai pin + 新增 `zones_json_covers_all_overworld_blueprint_zones`/`find_zone_returns_smallest_containing_zone`；client 4 测试 5→7 对齐。

### 关键 commit
- 见 PR #357（fix/zone-registry-coverage-v1，squash 合并入 main）。

### 测试结果
- server：`cargo fmt --check` 0 / `cargo clippy --all-targets -- -D warnings` 0 / `cargo test` **6753 passed; 0 failed; 1 ignored**。
- schema：`npm test` **411 passed**（含 `generated-artifacts.test.ts` 防漂移闸）。
- client：`./gradlew test build` **BUILD SUCCESSFUL**（1730 测试）。

### 跨仓库核验
- **server** `MAX_ZONE_DANGER_LEVEL=7` / `find_zone` min-volume / `zones.json` 28；**agent** schema 三处 maximum=7 + generated；**client** 两套 `MAX_DANGER_LEVEL=7` + `MAX_DANGER_SYMBOLS=7`。

### 遗留 / 后续
- TSY 12 zone 仍走 `tsy_dev_command.rs` 动态注册，`/tpzone` 不覆盖。
- `zones.json` ↔ `zones.worldview.example.json` 需手工同步；本 plan 加了防漂移 pin 测试，自动同步留后续。
- worldview `pois/worldgen/display_name` 未接入运行时。
- 合法纵向嵌套由 `find_zone` 最小 AABB 兜底；更精细的 dimension/depth 分辨需另设计。

<PROMISE>
我（Claude）确认本 plan 的全部交付物已逐项落地并实测通过：server 6753 测试全绿（含 4 个 wangyintai pin + 2 个新增 zone 测试）、schema 411 测试全绿、client BUILD SUCCESSFUL；zones.json 已达 28 zone 且 wangyintai 无 AABB 重叠；danger 上限 5→7 已在 server/agent/client 三仓一致放宽。本文件所列文件路径与 symbol 均经 grep 核验存在。
</PROMISE>
