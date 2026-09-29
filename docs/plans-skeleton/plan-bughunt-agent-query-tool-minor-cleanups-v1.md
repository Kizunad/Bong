# BugHunt：天道查询工具的快照、关系规模与趋势窗口清理

> 来源 Issue：#1746、#1674、#1607。三条都发生在 agent 对 server `world_state` 的镜像/查询边界，合并为一份小型 query-tool contract cleanup。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 保留 `ZoneSnapshot.status` 并限制 relationships 输出 | ⬜ |
| P1 | 统一 bounded history 与趋势窗口的语义并补查询回归 | ⬜ |

## §0 摘要

`cloneZoneSnapshot` 与 `sanitizeZoneSnapshot` 丢弃 schema 已声明的 zone `status`；`query-player` 把 server 全量 relationships 原样塞入工具结果；`query-zone-history` 同时展示用户 limit 窗口的 local delta 和全历史 trend，两个方向可能相反。三者都会让 LLM 看到与 server 当前事实不一致或无界的上下文。

## §1 游玩影响

天道可能把已坍缩/逸出的区域当作 normal，社交关系过大时单次工具结果膨胀，灵气趋势摘要出现互相矛盾的方向，进而产生错误的灾劫、社交或区域判断；不修改 server 状态和真元。

## §2 复现路径

1. 从 `bong:world_state` 接收 `ZoneSnapshot.status = "collapsed"`，经 `WorldModel` clone/restore 后观察 status 消失。
2. 给一个玩家构造超过合理规模的 `social.relationships`，调用 `query-player`，观察结果无切片。
3. 给 zone history 构造 bounded window 上升但全历史趋势下降的序列，调用 `query-zone-history`，观察 summary 同时出现 `+delta` 和 `falling`。

## §3 今天 `origin/main` 的根因证据

- `agent/packages/schema/src/world-state.ts:154-165` 已声明可选 `ZoneSnapshot.status`，但 `agent/packages/tiandao/src/world-model.ts:1089-1097` 的 `cloneZoneSnapshot` 和 `:1181-1213` 的 `sanitizeZoneSnapshot` 返回对象均没有该字段。
- `agent/packages/tiandao/src/tools/query-player.ts:282-287` 直接 map 全量 `player.social.relationships`；server `server/src/network/mod.rs:1824-1971` 的社会快照也未提供 agent-side cap。
- `agent/packages/tiandao/src/tools/query-zone-history.ts:116-127` 使用 `fullHistory.slice(-limit)` 计算 `localDelta`，却把 `WorldModel.getZoneTrendSummary` 的全历史窗口 `:383-400,775-795` 作为另一套 trend 输出。

## §4 非重复比对与立项检查记录

- **worldview**：查 `docs/worldview.md` 的“区域状态”“灵气”“社交/声名”关键词；这里只修镜像与上下文界限，不改区域物理。
- **finished_plans**：查 `plan-agent-v1/v2`、world model、query tools 和 zone economy 归档；未发现这三个当前实现缺口的统一修法。
- **active plan**：查 `docs/plan-*.md` 的 `ZoneSnapshot.status`、`relationships`、`getZoneTrendSummary`；没有同一组 query-tool owner。
- **skeleton**：查 `world-model.ts`、`query-player.ts`、`query-zone-history.ts` 与 `world_state`；未发现重复骨架，故合并为本 minor cleanup。
- **reminder.md**：已查 `docs/plans-skeleton/reminder.md`；仅有套包、放置、经济等条目，无 query-tool 待办。

## §5 修复骨架

- P0：clone/sanitize/JSON round-trip 原样保留可选 status；为 relationships 结果设有理由的上限并在截断时显式标注。
- P1：明确 history 查询的比较窗口；要么趋势只在 bounded window 内计算，要么 summary 明确区分两种窗口且不把异号值写成一个方向结论。补 status、截断和异号序列测试。
- 验收：`race_out/collapsed` 经过 restore 仍可被 context 读取；relationships 结果有稳定上限；summary 的数字与 direction 使用同一窗口。

## §6 接入面与跨仓契约

- **Inputs**：Redis `bong:world_state`、TypeBox `WorldStateV1/ZoneSnapshot/PlayerSocialSnapshot`、query-player/query-zone-history 的工具参数。
- **Outputs**：`WorldModel.latestState/zoneHistory`、`query-player` 与 `query-zone-history` 的工具结果，供 `context.ts` 和三 Agent 提示词使用。
- **共享类型或事件**：复用 `ZoneSnapshot.status`、`PlayerProfile.social.relationships`、`WorldStateV1` 和工具 result schema；不另造区域状态或趋势类型。
- **server 符号**：`publish_world_state_to_redis`、`build_world_state_snapshot`、`collect_player_social_snapshot`、`CH_WORLD_STATE`；server 继续提供权威 status/relationships，是否加 wire cap 必须与 schema 对拍。
- **agent 符号**：`WorldModel.cloneZoneSnapshot`、`sanitizeZoneSnapshot`、`getZoneTrendSummary`、`queryPlayerTool`、`queryZoneHistoryTool`。
- **client**：无变更；client 不消费这些 agent 工具结果，world_state 的 agent 镜像与 server→client payload 是两条既有路径。
- **worldview / qi_physics**：只保留和展示已有区域状态、灵气观测；不增加真元转移，不改 `qi_physics` ledger 或 `SPIRIT_QI_TOTAL`。
