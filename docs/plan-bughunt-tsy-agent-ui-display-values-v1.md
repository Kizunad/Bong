# plan-bughunt-tsy-agent-ui-display-values-v1

> **Active**（2026-07-04 从 `docs/plans-skeleton/plan-bughunt-tsy-agent-ui-display-values-v1.md` 升级；来源 PR #850 已 merge，opus 已核过真问题 + 防孤岛调研）。一句话主题：修复 TSY 发现面板读取 zone snapshot 时错把 `family_id` 当成真实 zone 名，导致 `spirit_qi_display` / `danger_tier` 在正常游玩里长期回退为占位值（"0.50" / "中危"）。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|---|---|---|---|
| P0 | family → subzone 展示口径收口（决议见 §8.1） | plan_skeleton | ✅ 2026-07-04 |
| P1 | `processTsyZoneActivatedForUi` 接线改为按 `_shallow→_mid→_deep` 优先级读取真实 family 子 zone，而非 `z.name===family_id` | fix_pr | ⬜ |
| P2 | agent 侧回归测试：suffix 优先级、真实值优先于占位值、仅真缺失时才回退占位 | fix_pr | ⬜ |

## 接入面

- **进料**：`bong:tsy_event`（kind=`tsy_zone_activated`）→ `TsyZoneActivatedV1`（`agent/packages/schema/src/tsy.ts:123-140`）；`WorldStateV1.zones`（`ZoneSnapshot[]`，`agent/packages/schema/src/world-state.ts:154-165`，由 `server/src/network/mod.rs:1905-1928 collect_zone_snapshots` 从 `ZoneRegistry` 逐 zone 填充，`name` 字段即 `zone.name`，与 TSY runtime 注册名同源，不做任何变换）。
- **出料**：`AgentUiRuntime.triggerUi({ scenario: "tsy_discovery", params })`（`agent/packages/tiandao/src/runtime.ts:1070-1079`）→ `UiRenderer.renderUi()`（`agent/packages/tiandao/src/ui/uiRenderer.ts`）用 `TSY_DISCOVERY_TEMPLATE`（`agent/packages/tiandao/src/ui/xmlTemplates.ts:92-105`，`{{zone_name}}` / `{{spirit_qi_display}}` / `{{danger_tier}}` / `{{agent_narrative}}`）渲染 → `AgentUiRequestCommandV1` 走 `bong:agent_ui_cmd` 下发 client。
- **共享类型 / event**：复用既有 `TsyZoneActivatedV1`、`ZoneSnapshot`、`AgentUiRequestCommandV1`；不新增 schema。
- **跨仓库契约**：
  - server：`server/src/world/tsy_lifecycle.rs`（TSY zone 命名 `<family>_shallow/_mid/_deep` 的生产者，`daoxiang_home_zone` / `collect_family_aabbs` 已示范同类 suffix 匹配）、`server/src/world/tsy_dev_command.rs`（entry portal 恒落 `_shallow` 中心）、`server/src/network/mod.rs:1905-1928`（`collect_zone_snapshots` 把全部注册 zone 原样发布，含 TSY 子 zone）
  - agent：`agent/packages/tiandao/src/runtime.ts:processTsyZoneActivatedForUi`（本 plan 唯一修改点）
  - client：无需改动（本 plan 只影响 params 的值，不改 template 结构 / payload 字段名）
- **worldview 锚点**：`docs/worldview.md` §十六（活坍缩渊 / TSY 秘境生命周期），本 plan 不改变游戏机制，只修正展示口径，无需改 worldview 正典。
- **qi_physics 锚点**：无——本 plan 是纯展示层（读取既有 `ZoneSnapshot.spirit_qi` / `danger_level` 做字符串格式化），不涉及真元流动 / 衰减公式。

## P0 — family → subzone 展示口径收口（✅ 决议见 §8.1）

候选 bug（major，复述自骨架）：`agent/packages/tiandao/src/runtime.ts:1060` 的 `processTsyZoneActivatedForUi` 用 `state.zones.find((z) => z.name === event.family_id)` 查 zone snapshot；但 `TsyZoneActivatedV1.family_id`（`agent/packages/schema/src/tsy.ts:128`）不带 `_shallow/_mid/_deep` 后缀，实际 TSY zone 名恒为 `<family>_shallow/_mid/_deep`（`server/src/world/tsy_lifecycle.rs:381-383`），结构性 miss → `spirit_qi_display` 固定 `"0.50"`、`danger_tier` 固定 `resolveTsyDangerTier(3)`（中危）。

设计口径已在 §8.1 #1/#2 收口：**取 `_shallow` 子 zone 优先（entry portal 恒落 shallow_center，玩家激活面板时物理上正站在 shallow 层），`_mid`→`_deep` 依次兜底，三层皆缺才回退占位值**。本阶段交付物 = 本文档 §8.1（决议 + 双锚点），无代码改动。

## P1 — `processTsyZoneActivatedForUi` 按 suffix 优先级读取真实 zone

**交付物**：

- `agent/packages/tiandao/src/runtime.ts`：新增私有 helper（建议命名 `resolveTsyFamilyZoneSnapshot(state: WorldStateV1, familyId: string): ZoneSnapshot | undefined`，紧邻 `resolveTsyDangerTier`，约 line 1105 前后），按 `["_shallow", "_mid", "_deep"]` 顺序（与 `server/src/world/tsy_lifecycle.rs:884 collect_family_aabbs` 的 suffix 数组顺序保持一致，不发明新约定）在 `state.zones` 里找 `${familyId}${suffix}` 精确匹配，命中即返回，三者皆缺返回 `undefined`。
- `processTsyZoneActivatedForUi`（`runtime.ts:1038-1092`）第 1060 行 `const zoneSnap = state.zones.find(...)` 改为调用该 helper；`spiritQiDisplay` / `dangerTier` 的“真实值优先，仅 `zoneSnap` 为 `undefined` 时才回退占位”逻辑保持不变（1061-1064 行结构不变，只换输入源）。
- 不改动 `zone_name` 参数（仍为 `event.family_id`，面板展示 family 层级标签，超出本 bug 范围）、不改 `player_id` 解析逻辑（`runtime.ts:1049-1050` 已是修好的目标形态，见 §8.1 #3）。

## P2 — agent 侧回归测试

**交付物**（全部落在 `agent/packages/tiandao/tests/runtime.test.ts` 的 `describe("processTsyZoneActivatedForUi (Fix①: triggerUi production path)", ...)` 块内，`≥5` 条新增/改写用例，均通过在测试内联构造 `{...createTestWorldState(), zones: [...]}` 携带真实 TSY 子 zone，不改动共享 `agent/packages/tiandao/tests/support/fakes.ts` 的 `createTestWorldState()` 默认 zones 数组，避免影响其他 24+ 处引用该 fixture 的用例）：

1. **`_shallow` 命中 → 展示真实值**：state.zones 含 `tsy_lingxu_01_shallow`（`spirit_qi: -0.42`, `danger_level: 6`），断言 `params.spirit_qi_display === "-0.42"` 且 `params.danger_tier === "极危"`（非占位 `"0.50"`/`"中危"`）——锁定 P1 主路径。
2. **`_shallow` 缺失、`_mid` 命中 → 回退到 `_mid`**：state.zones 只含 `tsy_lingxu_01_mid`，断言取到 `_mid` 的值，不回退占位。
3. **仅 `_deep` 存在 → 回退到 `_deep`**：state.zones 只含 `tsy_lingxu_01_deep`，断言取到 `_deep` 的值。
4. **优先级 pin**：state.zones 同时含 `_shallow` 与 `_deep`（数值不同）→ 断言取到 `_shallow` 的值，锁定“shallow 优先于 deep”不被未来重构颠倒。
5. **三层皆缺 → 回退占位（真缺失场景）**：state.zones 不含任何 `tsy_lingxu_01_*` zone，断言 `params.spirit_qi_display === "0.50"` 且 `params.danger_tier === "中危"`——区分“真实值优先”与“合理兜底”两条路径都被测试覆盖，而不是像现有 line 2291 `expect(params["danger_tier"]).toBeTruthy()` 那样弱断言（任何值都能通过，测不出 bug）。
6. 改写现有 line 2270-2293 用例：附加真实 `_shallow` zone 到该测试的 state，把 `toBeTruthy()` 断言升级为具体值断言。

**验收命令**：`cd agent/packages/tiandao && npm test`。

## §8 开放问题（原骨架遗留，已在 §8.1 全部收口；保留以备追溯）

1. `danger_tier` 应取 family 下所有子 zone 的最高 `danger_level`，还是取某个代表层（如 deepest active）？
2. `spirit_qi_display` 应显示 family 聚合值、最危险层值，还是最接近触发玩家的层值？
3. `event.player_id` miss 时的 `state.players[0]` fallback 是否与本 bug 同修，还是拆成独立 follow-up？

## §8.1 决议（pre-P0 收口，2026-07-04）

### #1 `danger_tier` 取值口径

**决议**：
1. 不做“family 下所有子 zone 取最高 `danger_level`”的聚合，也不是笼统的“deepest active”——取 **`_shallow` 子 zone 的 `danger_level`（三层缺失时按 `_shallow→_mid→_deep` 依次兜底）**。
2. 理由：`server/src/world/tsy_dev_command.rs:261-271` 显示 entry portal 恒把玩家传送到 `shallow_center`（TSY 唯一入口），`server/src/inventory/tsy_loot_spawn.rs:119-145 tsy_loot_spawn_on_enter` 监听的 `TsyEnterEmit` 正是这次落地触发的 `TsyZoneActivated`——即事件触发瞬间玩家物理上**必然**站在 `_shallow` 层。面板叙事“天道感知到…出现活坍缩渊，宜速做决断”对应的正是玩家脚下这层的即时风险，而非全 family 抽象聚合值。聚合值需要额外设计“多层怎么合并成一个数”的规则（无既有代码先例），复杂度与本 bug（纯展示口径读取错 key）不成比例。
3. 边界条件：若 `_shallow` 子 zone 尚未在 `WorldStateV1.zones` 中出现（如 lifecycle 刚注册、下一次 world_state publish 还没追上），依次尝试 `_mid`→`_deep`；三层全缺时保留现有占位 `resolveTsyDangerTier(3)`（中危）——这是"真缺失"分支，不是本 bug 修复目标，属合理兜底。

**落点**：`agent/packages/tiandao/src/runtime.ts:1060`（改动点）/ `agent/packages/tiandao/src/runtime.ts:1105`（新增 helper 落点，紧邻 `resolveTsyDangerTier`）/ 本 plan §P1。

### #2 `spirit_qi_display` 取值口径

**决议**：
1. 与 #1 同一套 suffix 优先级：`_shallow` 优先，`_mid`→`_deep` 依次兜底——**不做 family 聚合，也不取“最危险层”**，取“最接近触发玩家的层值”这一支，且该支在本场景下是确定性的（entry 恒落 shallow），不是概率性猜测。
2. 与 server 端既有约定复用同一顺序：`server/src/world/tsy_lifecycle.rs:884 collect_family_aabbs` 的 `let suffixes = ["_shallow", "_mid", "_deep"];`——agent 侧不发明新顺序，直接对齐。
3. 拒绝聚合值路线的理由：`ZoneSnapshot`（`agent/packages/schema/src/world-state.ts:154-165`）没有 family 分组信息，聚合需要 agent 侧现场按 family_id 前缀扫描 `state.zones` 全表、决定“聚合用平均 / 最低 / 加权”，这是一个新设计面，超出“纯展示层修 key”的 P0 bug 范围；且没有玩家心智模型支持“显示三层平均灵压”这种抽象值。

**落点**：`agent/packages/tiandao/src/runtime.ts:1061-1064`（沿用现有真实值优先/占位兜底结构，只换输入源）/ 本 plan §P1。

### #3 `event.player_id` miss fallback 是否同修

**决议**：
1. **已经修好，不属于本 plan 范围**——读码确认 `agent/packages/tiandao/src/runtime.ts:1046-1050` 当前实现已是 `state.players.find((p) => p.uuid === event.player_id) ?? state.players[0]`，且行内注释（1046-1048）明确写着旧的 `p.zone === event.family_id` 匹配逻辑“已弃用”。这与骨架 §审计来源描述的“候选 bug”不是同一处代码路径——骨架文档写作时机早于该修复落地，或审计时误将两条 fallback 逻辑混在一起描述。
2. 现有测试 `agent/packages/tiandao/tests/runtime.test.ts:2295-2317`（"player_id 直接命中目标玩家"）与 `:2319-2337`（"falls back to first online player when player_id not found"）已覆盖该行为的两个分支（直接命中 / miss 兜底），不需要在本 plan 内补测试。
3. 不拆独立 follow-up plan——已完成的代码不需要 plan 跟踪；本节存在只是为了在决议阶段留证据，避免后续审计重复标记为“未解决”。

**落点**：`agent/packages/tiandao/src/runtime.ts:1046-1050`（现状确认，无需改动）/ `agent/packages/tiandao/tests/runtime.test.ts:2295-2337`（既有测试覆盖确认）。

> 全部已在 §8.1 收口。原 §8 表保留以备追溯，**实施时以 §8.1 决议为准**。

## 审计来源

bughunt 第二轮线程 D，限定 scope：`agent/packages/tiandao` + `agent/packages/schema` + `server/src/network/agent_ui.rs` 相关 UI-as-data 链路。已排除现有 `docs/plan-bughunt-r*.md` / `docs/plans-skeleton/plan-bughunt-r*.md` 题目；本题未被现有 bughunt plan 收录。PR #850（骨架）已 merge，opus 已核过真问题 + 防孤岛。
