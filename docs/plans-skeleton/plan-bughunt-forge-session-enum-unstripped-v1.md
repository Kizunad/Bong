# plan-bughunt-forge-session-enum-unstripped-v1

## §0 摘要

`ProtoServerDataBridge.bridgeForgeSession()` 只对顶层 `current_step` 字段调用 `stripEnumPrefix`，对被 `flattenOneofInPlace` 摊平进 `step_state` 的两组 proto 枚举字段——淬炼节拍 `pattern`（repeated `TemperBeat`）与开光步骤的 `min_realm`/`color_imprint`（`Realm`/`ColorKind`）——从未做剥离处理，全名字符串原样透出。这是**同一根因**在两个消费端各自炸出的两个症状：

1. **淬炼节拍**：`TemperingTrackComponent.normalizeBeat()` 只识别 `"L"/"LIGHT"/"H"/"HEAVY"/"F"/"FOLD"`，`"TEMPER_BEAT_LIGHT"` 不匹配任何 case，pattern 列表恒为空，节奏轨道恒显示"已完成"，整个节奏小游戏对玩家变成盲猜。
2. **开光门禁**：`ConsecrationPanelComponent.realmRank()` 只用 `equalsIgnoreCase` 匹配无前缀单词，`"REALM_SPIRIT"` 不匹配任何一项返回 -1，`realmAllowed()` 把 `min<0` 当"不限境界"直接放行，境界门禁被静默绕过——低境界玩家可对必败的开光真实烧真元（最多 80 点，相对全服 `SPIRIT_QI_TOTAL=100` 是极大比例）。

两条修复都落在 `bridgeForgeSession()` 同一函数、同一次 PR 范围内合理合并为一份骨架。

本 plan 仅是 BugHunt Skeleton Plan，不包含实际修复。

## §1 实际游玩体验影响

- **淬炼**：淬炼是 forge 系统的常规第二步，`ForgeStation` 是新手引导 POI（`server/world/poi_novice.rs`），教程级图谱 `qing_feng_v0.json` 本身就带真实节拍 pattern。任何玩家正常锻造任意带 tempering 步的图谱，进入该步瞬间即触发此 bug；J/K/L 击键仍可发往服务端（不依赖客户端渲染），但玩家完全看不到该按什么节拍、还剩几拍，整个节奏小游戏对普通玩家变成盲猜。
- **开光**：任何境界低于通灵（Spirit）的玩家学会 `ling_feng_v0`（`station_tier_min=2`，正常图谱学习/站点升级可达）这类要求 `min_realm: "Spirit"` 的图谱后，UI 上的开光按钮会被误判为可用，可持续按住注入；服务端仍会按 `qi_physics` ledger 正确扣减/转移玩家 `qi_current`，但 `resolve_consecration()` 会按 blueprint 的 `min_realm` 判定必然失败——玩家因此被 UI 误导，真实烧掉最多 80 点真元换来注定失败的结算。

## §2 复现路径

### 淬炼节拍

1. 玩家正常锻造任意带 tempering 步的图谱（例如新手教程级 `qing_feng_v0`，station_tier_min=1，带真实 10 拍 pattern `L,L,H,L,F,H,L,F,H,H`）。
2. 进入淬炼步骤，服务端 `build_step_state`（`server/src/network/forge_snapshot_emit.rs`）用 blueprint 的真实 pattern 填充 `ForgeStepStateTempering.pattern`，`proto_convert.rs:2814-2861` 转成 proto 枚举 int。
3. 现状预期：Java 侧 `JsonFormat.Printer`（无 `printingEnumsAsInts`）把枚举字段固定输出全名字符串，如 `"TEMPER_BEAT_LIGHT"`；`bridgeForgeSession()`（`ProtoServerDataBridge.java:693-715`）只对顶层 `current_step` 调用 `stripEnumPrefix`，`pattern` 数组原样透出全名；`TemperingTrackComponent.normalizeBeat()`（148-190）不识别全名，返回空串被 `readPattern()` 过滤，`drawTrack()` 落到"淬炼节拍已完成"。
4. 修复后预期：`pattern` 数组逐元素剥离 `TEMPER_BEAT_` 前缀后，`normalizeBeat` 正确识别，节奏轨道正常显示剩余拍数。

### 开光境界门禁

1. 低境界玩家学会 `ling_feng_v0`（consecration 步要求 `min_realm: "Spirit"`，`qi_cost: 80.0`）。
2. 玩家进入开光步骤，服务端 `min_realm` 通过 `realm_enum_to_proto` 映射进 wire message（`proto_convert.rs:2877-2888`），JsonFormat 渲染为全名 `"REALM_SPIRIT"`。
3. 现状预期：`bridgeForgeSession()` 未剥离 `min_realm`/`color_imprint`；`ConsecrationPanelComponent.realmRank()`（275-283）匹配不到全名返回 -1；`realmAllowed()`（247-252）把 `min<0` 当"允许"，开光按钮误判可用；玩家点击注入，`handle_forge_consecration_inject`/`handle_consecration_injects` 无境界检查，真实扣减真元；`resolve_consecration()`（`forge/steps.rs:236-237`）最终才判定境界不足强制失败。
4. 修复后预期：`min_realm`/`color_imprint` 剥离后境界门禁正确渲染，低境界玩家开光按钮直接置灰，不会误触发真元消耗。

## §3 根因证据

### 淬炼节拍（forge-tempering-pattern-enum-unstripped）

- `proto/bong/envelope.proto:1004-1009` 定义 `TemperBeat` 枚举 `TEMPER_BEAT_LIGHT`/`HEAVY`/`FOLD`。
- `server/src/network/forge_snapshot_emit.rs` `build_step_state()`（post 已归档修复 `1283cc302`）用 blueprint 的真实 `profile.pattern` 填充非空 pattern；`server/assets/forge/blueprints/qing_feng_v0.json` 确认 pattern `[L,L,H,L,F,H,L,F,H,H]`，`station_tier_min=1`，该 `ForgeStation` 是新手 POI（`server/src/world/poi_novice.rs`），正常玩家可达。
- `server/src/schema/proto_convert.rs:2814-2861` 把真实 pattern 转成 proto 枚举 int，装入 `repeated ForgeStepStateTempering.pattern`。
- `client/src/main/java/com/bong/client/network/ProtoServerDataBridge.java`：`PRINTER`（39-42）无 `printingEnumsAsInts()`，`bridgeForgeSession()`（693-715）只 `stripEnumPrefix(root, "current_step", "FORGE_STEP_")`（708），`pattern` 数组不在处理范围内。
- `client/src/main/java/com/bong/client/forge/screen/TemperingTrackComponent.java`：`readPattern()`（148-160）+ `normalizeBeat()`（183-190）只认 `"L"/"LIGHT"/"H"/"HEAVY"/"F"/"FOLD"`（trim+uppercase），`"TEMPER_BEAT_LIGHT"` 匹配不到任何 case 返回默认空串，`readPattern` 的 `!beat.isEmpty()` 过滤掉空串，`drawTrack()`（81-84）在 pattern 为空时恒显示"淬炼节拍已完成"。
- 现有测试掩盖此缺陷：`ProtoServerDataBridgeTest.forgeSessionTemperingFlattenedWithStepTag()`（1253-1270）从未 `.addPattern(...)`；`TemperingTrackComponentTest` 全部手写简化 JSON（如 `"pattern":["L","H","F"]`）而非走真实 bridge 输出。

### 开光境界/色印门禁（forge-consecration-min-realm-color-enum-unstripped）

- `proto/bong/common.proto:11-19` 定义 `Realm` 枚举；`proto/bong/envelope.proto:1050-1054` 定义 `ForgeStepStateConsecration` 含 `optional Realm min_realm` / `optional ColorKind color_imprint`。
- `server/src/network/forge_snapshot_emit.rs:376-377` 用 blueprint 真实 `StepSpec::Consecration` profile 填充 `min_realm`；`server/src/schema/proto_convert.rs:2877-2888` `realm_enum_to_proto` 映射进 wire message，JsonFormat 渲染全名 `"REALM_SPIRIT"`。
- `server/assets/items/forge.toml:99-101` 定义可正常获取的稀有物品 `blueprint_scroll_ling_feng` → `ling_feng_v0`；`server/assets/forge/blueprints/ling_feng_v0.json:34-40` consecration profile 要求 `min_realm:"Spirit"`, `qi_cost:80.0`。
- `client/.../ProtoServerDataBridge.java:693-715` `bridgeForgeSession()` 同样只处理顶层 `current_step`，`min_realm`/`color_imprint` 摊平进 `step_state` 后未做任何剥离。
- `client/.../forge/screen/ConsecrationPanelComponent.java`：`realmRank()`（275-283）只 `equalsIgnoreCase` 匹配 `["Awaken","Induce","Condense","Solidify","Spirit","Void"]` 这组无前缀单词，`"REALM_SPIRIT"` 不匹配任何一项返回 -1；`realmAllowed()`（247-252）`return min < 0 || caster >= min;`——`min=-1` 时不论 `caster` 是什么境界都直接放行。`ColorKind.fromWire` 同理对 `"COLOR_KIND_X"` 失败（次要的显示问题）。
- `handle_start_forge_requests`（`forge/mod.rs:182+`）只校验图谱已学/station tier/材料，无境界检查；`handle_forge_consecration_inject`（`client_request_handler.rs:3664-3695`）/`handle_consecration_injects`（`forge/mod.rs:556+`）也无境界检查，真实把玩家 `Cultivation.qi_current` 扣减转入 zone ledger（守恒，非 qi_physics 违反，但真实个人资源损失）；境界检查只在最终 `resolve_consecration()`（`forge/steps.rs:236-237`）触发，此时 qi 已经花掉。
- 现有测试掩盖此缺陷：`ProtoServerDataBridgeTest.java:1308` `forgeSessionConsecrationFlattenedWithStepTag` 只设置 `qi_injected`/`qi_required`，从未设置 `min_realm`/`color_imprint`；`ConsecrationPanelComponentTest.inject_button_disabled_when_realm_insufficient()` 手写 `"min_realm":"Spirit"`（简化形态，非真实 wire），从未验证过真实的 `"REALM_SPIRIT"` 全名输入。

## §4 非重复比对

- 已读 `docs/finished_plans/plan-bughunt-forge-stepstate-contract-drift-v1.md`：该 finished plan 描述的是不同根因——「路径 A」server `build_step_state` 曾把 pattern 硬编码成空数组（已由 `1283cc302`(2026-07-06) 修复）；「路径 C」是 Rust/TypeBox forge schema 完全没有 `min_realm` 字段（同一 commit 在 `server/src/schema/forge.rs` 补上该字段）。`git show --stat 1283cc302` 确认只改了 server/schema/agent-schema，未触及 `ProtoServerDataBridge.java` 或 `TemperingTrackComponent.java`/`ConsecrationPanelComponent.java`——两组 bug 根因不同，client 侧剥前缀缺陷完全没被那次修复触及。该 finished plan 自己的 Finish Evidence 写明"2026-07-26 审计为只读核验……本次审计未逐一重新核验 client 代码现状"，即从未真正验证过 client 侧行为。
- Grep 全部 `docs/plans-skeleton`/`docs/plan-*.md`/`docs/finished_plans` 未见同类"枚举全名未剥"的 `forge_session.step_state.tempering.pattern` 或 `consecration.min_realm`/`color_imprint` 报告。

## §5 修复计划骨架

### P0 客户端剥离两组 step_state 枚举字段

- 在 `ProtoServerDataBridge.bridgeForgeSession()` 中，`flattenOneofInPlace(stepState, ...)` 之后：
  - 若 `stepState` 含 `pattern` 数组，逐元素剥离 `TEMPER_BEAT_` 前缀（新增作用于纯字符串数组的 helper，例如 `stripEnumPrefixInStringArray(stepState, "pattern", "TEMPER_BEAT_")`，区别于已有的作用于对象数组的 `stripEnumPrefixInArray`）。
  - 若当前 step 为 consecration，对 `min_realm` 调用 `stripEnumPrefixCapitalized(stepState, "min_realm", "REALM_")`（与既有 `normalizeRealmField` 同款大小写，产出 `"Spirit"` 匹配 `realmRank` 期望），对 `color_imprint` 调用 `stripEnumPrefixCapitalized(stepState, "color_imprint", "COLOR_KIND_")`（匹配 `ColorKind.fromWire` 的单段大写风格）。

### P1 真实契约回归测试

- 把 `ProtoServerDataBridgeTest` 的 tempering 用例改为真实 `.addPattern(Envelope.TemperBeat.TEMPER_BEAT_LIGHT)` 等构造并断言剥离后的值；`forgeSessionConsecrationFlattenedWithStepTag` 扩展为真实 `.setMinRealm(Envelope.Realm.REALM_SPIRIT)`/`.setColorImprint(...)` 构造并断言剥离后值。
- `TemperingTrackComponentTest`/`ConsecrationPanelComponentTest` 各补至少一条走真实 bridge JSON 形状（而非手写简化字符串）的回归用例，避免测试继续绕过真实契约、防止再次假绿。

## §6 验证计划

- `cd client && ./gradlew test build`
- 手工复现矩阵：淬炼节拍轨道正常显示剩余拍数（教程级 `qing_feng_v0`）；开光低境界玩家开光按钮正确置灰（`ling_feng_v0`）；高境界玩家开光按钮正常可用（回归不受影响）。

## §7 接入面与守恒说明

- 进料：`bong:forge_session` S2C payload（`step_state.tempering.pattern`/`step_state.consecration.min_realm`/`color_imprint`）。
- 出料：`TemperingTrackComponent` 渲染的节奏轨道、`ConsecrationPanelComponent` 的境界门禁按钮状态。
- 跨端契约：不改 proto/schema 定义，只在 client `ProtoServerDataBridge` 补齐已有 `stripEnumPrefix` 系列 helper 对这两组字段的覆盖，是纯 bridge 层修复。
- qi_physics：开光步骤本身消耗玩家真元并转入 zone ledger（既有守恒路径，本 finding 不改动该转移逻辑）；本修复目的是让 UI 门禁与服务端最终判定一致，避免玩家在必败开光上被误导消耗真元，不新增 qi 常数或 ledger 流。

## §8 对抗复核结论

### 淬炼节拍（forge-tempering-pattern-enum-unstripped）

- 候选证据：`build_step_state`（post `1283cc302`）填充真实非空 pattern；client `PRINTER` 无 `printingEnumsAsInts`；`bridgeForgeSession` 只剥 `current_step`；`normalizeBeat` 不识别全名，pattern 恒被过滤为空；教程级 `qing_feng_v0` 证实正常新玩家可达。
- 反方质疑：是否与 `forge-stepstate-contract-drift-v1` 重复？
- 修正/反驳：`git show --stat 1283cc302` 确认该 commit 只碰 server/schema/agent-schema 文件，从未碰 `ProtoServerDataBridge.java`/`TemperingTrackComponent.java`，且没有后续 commit 修复 client 侧剥前缀；现有 `ProtoServerDataBridgeTest`/`TemperingTrackComponentTest` 均未覆盖真实 wire 形状，是缺陷从未被 CI 撞见的原因。
- 反方最终裁决：通过（`is_real: true`, `reachable: true`, `severity_adjust: unchanged`，保持 high）。

### 开光境界/色印门禁（forge-consecration-min-realm-color-enum-unstripped）

- 候选证据：`realmRank()` 只匹配无前缀单词，`"REALM_SPIRIT"` 返回 -1；`realmAllowed()` 把 `min<0` 当"不限境界"放行；`ling_feng_v0` 是正常可获取图谱，consecration 步真实要求 `min_realm:"Spirit"` + `qi_cost:80.0`；服务端注入路径无境界前置检查，真元在最终判定失败前已被扣减转移。
- 反方质疑：是否与 `forge-stepstate-contract-drift-v1` 的"路径 C"（schema 缺 `min_realm` 字段）重复？
- 修正/反驳：该 finished plan 已在 server/schema 侧补上 `min_realm` 字段（根因是"字段不存在"），但从未验证/修复 client 侧枚举全名未剥这一独立缺陷（根因是"字段存在但格式不匹配消费端"）；其 Finish Evidence 明确写"本次审计未逐一重新核验 client 代码现状"，证明这是遗留的未闭合缺口而非同一 bug 的重复报告；现有测试只设置 `qi_injected`/`qi_required`，从未覆盖 `min_realm`/`color_imprint` 的真实 wire 形状。
- 反方最终裁决：通过（`is_real: true`, `reachable: true`, `severity_adjust: unchanged`，保持 high）。两条均非重复、可达性强（正常游玩路径），已合并为一份骨架统一在 `bridgeForgeSession()` 范围内修复。
