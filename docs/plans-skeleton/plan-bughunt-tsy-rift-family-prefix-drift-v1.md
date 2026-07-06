# plan-bughunt-tsy-rift-family-prefix-drift-v1

> **Skeleton Plan / report-only**。一句话主题：worldgen 裂缝 POI 的 `family_id` 使用 `daneng_01/zongmen_01/...` 无前缀值，但 server runtime 的 TSY family 契约是 `tsy_daneng_01/tsy_zongmen_01/...`。玩家经真实 worldgen 裂缝入场后，entry/exit 因同错同源可能表面可用，但 loot、敌人、生命周期、塌缩清理和化虚压制会按错 family 断链。

## Bug 摘要

`server/zones.worldview.example.json` 的主世界 rift entry POI 和 `server/zones.tsy.json` 的 TSY exit POI 都把 `family_id` 写成无 `tsy_` 前缀的短名，例如 `family_id:daneng_01`、`family_id:zongmen_01`、`family_id:lingxu_01`。但运行态 `Zone::tsy_family_id()` 从 `tsy_daneng_01_shallow` 这类 zone 名派生出的 canonical family 是 `tsy_daneng_01`，`TsyPresence.family_id` 注释和绝大多数 TSY 测试也都以带前缀 family 为准。

POI consumer 当前没有任何 normalize：它直接把 tag 写进 `RiftPortal.family_id`，入场系统再把这个值写进 `TsyPresence` 和 `TsyEnterEmit`。于是真实 worldgen 裂缝入场会注册一个无前缀 lifecycle family，同时后续系统继续拿带前缀 family 查 zone/state，形成运行态身份错配。

## 对实际游玩体验的影响

玩家从主世界塌缩裂缝进入 TSY 后，最先看到的体验可能是“能进也能出”，因为 entry portal 和 exit portal 都带同一个无前缀 `family_id`，`tsy_exit_portal_tick` 只做字符串相等比较。这会掩盖问题。

真正的体验破坏发生在秘境内容层：上古遗物按 `Zone::tsy_family_id() == family_id` 找分层 zone，会找不到 `daneng_01_mid/deep` 对应的 `tsy_daneng_01_mid/deep`；敌人生成按 `format!("{family_id}_{depth}")` 查 zone，也会查 `zongmen_01_shallow` 这种不存在的名字；lifecycle、塌缩清理、collapse tear 和化虚压制也会在无前缀 state 与带前缀 zone family 之间错开。玩家体感会是“裂缝能传送，但秘境空心、奖励/敌人/塌缩反馈缺失，后期化虚互动也对不上实际 TSY 区域”。

## 证据定位

- `server/zones.worldview.example.json:390,441,492,543,760,963`：主世界 rift entry POI 使用 `family_id:daneng_01` / `family_id:zongmen_01`；同一 zone 的 `worldgen.tsy_zone_link` 却指向 `tsy_daneng_01_shallow` / `tsy_zongmen_01_shallow`。
- `server/zones.tsy.json:16,40,122,220,327`：TSY zone 名是 `tsy_lingxu_01_shallow` 等带前缀形式，但 exit POI tag 使用 `family_id:lingxu_01/zongmen_01/daneng_01/gaoshou_01`。
- `server/src/world/zone.rs:162-174`：`Zone::tsy_family_id()` 明确从 `tsy_lingxu_01_shallow` 派生 `tsy_lingxu_01`。
- `server/src/world/tsy.rs:47-49`：`TsyPresence.family_id` 注释给出的规范例子是 `tsy_lingxu_01`。
- `server/src/world/tsy_poi_consumer.rs:116-144,179-195`：entry / exit portal 都直接从 POI tag 取 `family_id`，没有 canonicalize。
- `server/src/world/tsy_portal.rs:108-127,155-160`：入场把 `portal.family_id` 直接写入 `TsyPresence` / `TsyEnterEmit`；出场仅比较 `portal.family_id == presence.family_id`，因此同错同源会表面可用。
- `server/src/inventory/tsy_loot_spawn.rs:85-94,130-171`：入场事件的 `family_id` 用于找 TSY 分层 zone；无前缀值匹配不到 `zone.tsy_family_id()`。
- `server/src/npc/tsy_hostile.rs:599-603`：敌人生成按 `{family_id}_{depth}` 拼 zone 名，无前缀会查不存在的 `zongmen_01_mid`。
- `server/src/world/tsy_lifecycle.rs:883-889`：塌缩清理同样按 `{family}{suffix}` 收集 AABB。
- `server/src/cultivation/void/actions.rs:365-378`：化虚压制从当前 zone 派生带前缀 family，再查 lifecycle state；若 state 是 POI 入场注册的无前缀键，会查不到。
- `worldgen/scripts/terrain_gen/blueprint.py:247-258` 与 `worldgen/scripts/terrain_gen/bakers/raster_export.py:549-567`：POI tags 从 blueprint 到 raster manifest 原样拷贝。
- `worldgen/scripts/terrain_gen/harness/raster_check.py:283-302,367-386`：当前 gate 只检查 TSY family 有 exit、entry POI 有 `family_id` 和 `target_family_pos_xyz`，没有校验 family 是否等于 canonical `tsy_*`。

## 触发路径

1. 用 worldgen pipeline 从 `server/zones.worldview.example.json` 生成主世界 raster manifest，从 `server/zones.tsy.json` 生成 TSY raster manifest。
2. 通过 `scripts/dev-reload.sh` 或修复 #992 后的启动链路把 `BONG_TERRAIN_RASTER_PATH` 和 `BONG_TSY_RASTER_PATH` 传给 server。
3. `TerrainProvider.pois()` 暴露 manifest 中的 rift POI，`spawn_rift_portals` 在 startup 生成 entry / exit `RiftPortal`。
4. 玩家踩入主世界 entry portal，`tsy_entry_portal_tick` 写入无前缀 `TsyPresence.family_id`，并发送无前缀 `TsyEnterEmit.family_id`。
5. loot / hostile / lifecycle / extract / void action 后续以该 family 查 `tsy_*` zone 或 state，因前缀不一致跳过或查空。

## 反方审查记录

Round 1 反方尝试证明无前缀 family 是 portal 专用 canonical id，或存在隐藏 normalize。审查结果：未找到任何代码把 `daneng_01` 映射成 `tsy_daneng_01`；`tsy_poi_consumer`、Python blueprint parser 和 raster export 都是原样透传。`/tsy_spawn` dev 路径确实不受影响，因为它直接以 `tsy_lingxu_01` 这类输入生成 zone/portal/容器/敌人，但这只说明 dev smoke 可能漏掉真实 worldgen POI 路径。

Round 2 反方进一步尝试把无前缀解释成旧 worldgen plan 的设计意图。最强证据是 `docs/finished_plans/plan-tsy-worldgen-v1.md` 的早期 POI 表和 JSON 模板确实使用 `family_id:zongmen_01/daneng_01`，并曾设想跨 manifest family check。但实际代码没有实现 `resolve_tsy_shallow_center` 或 cross manifest normalize，且 `Zone::tsy_family_id()`、`TsyOrigin::from_zone_name`、loot spawn、lifecycle、hostile、extract、void action 都站在 `tsy_*` 契约一侧。

去重结论：不重复 #992/#998/#1011。#992 是 `BONG_TSY_RASTER_PATH` 未传导致 TSY provider 缺失；#998 是 TSY Y 分层被 2D overlay 压成 deep 单层；#1011 是 TSY enter/exit agent 事件丢失。本 bug 是 provider 正常加载后，worldgen POI tag 与 server runtime family identity 漂移。

## Skeleton Fix Plan

P0：收敛 canonical family 契约。

- 选定唯一权威：建议所有 rift POI `family_id` 都写 canonical `tsy_*` family，和 `Zone::tsy_family_id()` / `TsyPresence` / lifecycle registry 对齐。
- 修正 `server/zones.worldview.example.json` 与 `server/zones.tsy.json` 中 rift POI 的 `family_id` tag；主世界 entry 应与 `worldgen.tsy_zone_link` 去掉 `_shallow/_mid/_deep` 后一致。
- 如需兼容旧 manifest，新增受控 normalize helper，并在日志中 warn；不要让不同系统各自拼前缀。

P1：补 worldgen / raster 契约 gate。

- 在 TSY manifest check 中从 zone 名派生 canonical family 集合，要求每个 exit portal `family_id` 精确命中集合。
- 在主世界 manifest check 中要求 entry portal `family_id` 为 canonical `tsy_*`，并与同 zone 的 `worldgen.tsy_zone_link` family 一致。
- 补跨 manifest check：主世界 entry 的 family 必须存在于 TSY manifest family 集合，`target_family_pos_xyz` 必须落在该 family 的 shallow AABB 内。

P2：补 server 端回归 pin。

- 用实际 blueprint/manifest fixture 驱动 `spawn_rift_portals`，断言生成的 entry / exit `RiftPortal.family_id` 都是 `tsy_*`。
- 用真实 rift 入场事件驱动 `tsy_loot_spawn_on_enter`，断言 `TsySpawnedFamilies` / `TsyZoneStateRegistry` 以 `tsy_*` key 注册，并能在 mid/deep 层放置 relic。
- 覆盖一个负例：无前缀 `family_id` manifest 应被 validator 拦下，或被 normalize helper warn 后统一为 canonical key。

## 验收测试计划

- `cd worldgen && python -m pytest tests` 中新增/更新 family-id 契约测试，至少覆盖主世界 entry、TSY exit、跨 manifest 三类一致性。
- `cd worldgen && python -m scripts.terrain_gen ...` 后运行 `python scripts/terrain_gen/harness/raster_check.py`，确认无前缀 `family_id` 会红、canonical `tsy_*` 会绿。
- `cd server && cargo test tsy_poi_consumer tsy_loot_spawn_on_enter tsy_lifecycle`，确认真实 POI 入场后 lifecycle / relic spawn key 为 `tsy_*`。
- `bash scripts/dev-reload.sh` 后进服踩一个真实主世界 rift：确认能进入对应 TSY family，能看到该 family 的遗物/敌人/塌缩反馈，exit 后返回主世界且不被同一 entry 下一 tick 重新吸入。

## 风险

- 如果选择在 runtime normalize 旧无前缀 tag，需要避免把非 TSY family 或未来跨 family 命名误补成 `tsy_*`；validator hard-fail 更干净，但会要求现有蓝图和生成产物同步修正。
- 主世界多个 entry 指向同一 TSY family 时，`worldgen.tsy_zone_link` 与 POI `family_id` 的关系要定义为 n:1 合法，而不是误判重复。
- 修复 #992 后 TSY provider 会更稳定加载，本 bug 会从“被 provider 缺失遮蔽”变成玩家可见问题；建议同一修复窗口补上 family-id gate。

## 审计来源

BugHunt F7 worldgen 第七轮，范围限定为 worldgen Python、terrain/raster harness、layout/zone placement、server/zones.json 接口、地形流水线产物契约。本次只新增 skeleton plan，不修改实际代码、配置、依赖或资源。
