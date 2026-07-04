# plan-bughunt-tsy-agent-ui-display-values-v1（骨架）

> **骨架（草案）**。一句话主题：修复 TSY 发现面板读取 zone snapshot 时错把 `family_id` 当成真实 zone 名，导致 `spirit_qi_display` / `danger_tier` 在正常游玩里长期回退为占位值。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|---|---|---|---|
| P0 | family → subzone 展示口径收口（TSY 面板该显示哪一层/哪种汇总） | plan_skeleton | ⬜ |
| P1 | `processTsyZoneActivatedForUi` 接线改为读取真实 family 子 zone，而非 `z.name===family_id` | fix_pr | ⬜ |
| P2 | agent 侧回归测试：family 命名、suffix 匹配、真实值优先、仅真缺失时才用占位 | fix_pr | ⬜ |

## P0 — TSY 发现面板展示值取错 key（major）

- **候选 bug（major）**：`agent/packages/tiandao/src/runtime.ts` 的 `processTsyZoneActivatedForUi` 在生成 `tsy_discovery` 面板时，用 `state.zones.find((z) => z.name === event.family_id)` 查 zone snapshot；但 `TsyZoneActivatedV1` 明确约定 `family_id` 不带 `_shallow/_mid/_deep` 后缀，而实际 TSY zone 名就是 `<family>_shallow/_mid/_deep`。结果是结构性 miss，`spirit_qi_display` 固定回退 `"0.50"`，`danger_tier` 固定回退 `resolveTsyDangerTier(3)`（中危），玩家看到的探查面板并非真实秘境状态。

## 玩家影响

- 该链路属于 `tsy_zone_activated` → `processTsyZoneActivatedForUi` → `AgentUiRuntime.triggerUi("tsy_discovery")` 的正常主路径，不是 dev-only。
- 玩家会根据错误的“残留灵压 / 危险等级”决定是“踏入探寻”“神识探查”还是“离开”，属于真实决策误导。
- 高危/低危、富灵/贫灵 TSY 都可能被压成统一占位观感，削弱面板作为探索前置信息的价值。

## 读码证据

- `agent/packages/schema/src/tsy.ts`：`TsyZoneActivatedV1.player_id` 注释已写明“无需再靠 zone 名匹配（zone 名带 `_shallow/_mid/_deep` 后缀，family_id 不带）”。
- `server/src/network/tsy_event_bridge.rs`：server 发往 agent 的激活事件 payload 只有 `family_id`，不会携带某个具体的 `_shallow/_mid/_deep` zone 名。
- `server/src/world/tsy_dev_command.rs`、`server/src/world/tsy_lifecycle.rs`：TSY 实际 zone 命名与生命周期清理都按 `<family>_shallow/_mid/_deep` 运作。
- `agent/packages/tiandao/src/runtime.ts`：当前实现仍按 `z.name === event.family_id` 精确匹配，并在 miss 时回退占位值。

## 接入面

- **进料**：`bong:tsy_event` / `TsyZoneActivatedV1`、`WorldStateV1.zones`
- **出料**：`AgentUiRuntime.triggerUi()` → `UiRenderer.renderUi()` → `AgentUiRequestCommandV1`
- **共享契约**：`TsyZoneActivatedV1`、`PlayerProfile`、`AgentUiRequestCommandV1`
- **跨仓库符号**：`processTsyZoneActivatedForUi` / `TsyZoneActivatedV1` / `TSY_DISCOVERY_TEMPLATE`

## 开放问题

1. `danger_tier` 应取 family 下所有子 zone 的最高 `danger_level`，还是取某个代表层（如 deepest active）？
2. `spirit_qi_display` 应显示 family 聚合值、最危险层值，还是最接近触发玩家的层值？
3. `event.player_id` miss 时的 `state.players[0]` fallback 是否与本 bug 同修，还是拆成独立 follow-up？

## 审计来源

bughunt 第二轮线程 D，限定 scope：`agent/packages/tiandao` + `agent/packages/schema` + `server/src/network/agent_ui.rs` 相关 UI-as-data 链路。已排除现有 `docs/plan-bughunt-r*.md` / `docs/plans-skeleton/plan-bughunt-r*.md` 题目；本题未被现有 bughunt plan 收录。
