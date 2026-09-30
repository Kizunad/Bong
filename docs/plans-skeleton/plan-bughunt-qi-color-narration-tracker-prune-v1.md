# BugHunt：真元变色叙事 tracker 长期保留离线玩家

> 来源 Issue：#1524。只处理 agent 侧 `QiColorNarrationTracker.seen` 的 roster 生命周期；真元颜色计算和 server 世界状态 producer 不改。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 按当前 world_state roster 清理离线玩家快照 | ⬜ |
| P1 | 离线/重连、颜色变化与 map 上限回归 | ⬜ |

## §0 摘要

`QiColorNarrationTracker.seen` 在每次 `ingest` 只 set 当前玩家，从不删除不再出现在 `WorldStateV1.players` 的 key。offline id 以 `offline:<name>` 为主，长期运行并经历大量不同玩家后，Map 会随着历史玩家数增长；这属于 agent 内存生命周期缺口。

## §1 游玩影响

短期没有可见错误，长时间运行会积累无用 `QiColorSnapshot`，增大每轮遍历与进程内存，最终影响天道 runtime 稳定性；不改变 server 真元值或颜色判定。

## §2 复现路径

1. 让 `bong:world_state` 先后包含大量不同 `PlayerProfile.uuid`。
2. 每次调用 `QiColorNarrationTracker.ingest` 后让旧玩家不再出现在当前 players 列表。
3. 观察 `seen` 仍保留旧 uuid；没有 prune 或容量边界。

## §3 今天 `origin/main` 的根因证据

- `agent/packages/tiandao/src/qi-color-narration.ts:11-31` 声明 `seen` Map，`ingest` 只 `get/set` 当前玩家，没有根据当前 roster 删除。
- `agent/packages/tiandao/src/runtime.ts:1326` 每个 runtime 创建 tracker；没有跨 tick 的清理任务或 TTL。
- server `server/src/network/mod.rs:1313-1445,1681-1725` 的 `publish_world_state_to_redis/build_world_state_snapshot` 每次发布当前在线 `players`，`PlayerProfile.uuid` 来自 `canonical_player_id`；因此当前 roster 是可用的权威清理边界。

## §4 非重复比对与立项检查记录

- **worldview**：查 `docs/worldview.md` 的“真元颜色”“在线玩家”“天道感知”关键词；这里只做缓存生命周期，不改真元物理或正典描述。
- **finished_plans**：查 `plan-agent-v1/v2`、qi-color narration、world-state persistence 归档；没有 tracker prune 的落地规则。
- **active plan**：查 `docs/plan-*.md` 的 `QiColorNarrationTracker`、`seen`、`WORLD_STATE`；未发现同一修法。
- **skeleton**：查 `qi-color-narration.ts`、`runtime.ts`、`world_state` 与已有内存上限关键词；无重复骨架。
- **reminder.md**：已查 `docs/plans-skeleton/reminder.md`；没有 qi-color tracker 条目。

## §5 修复骨架

- P0：每次 ingest 先建立当前 uuid 集合，处理颜色变化后删除不在集合中的 `seen`；空 roster 也要完成清理。不要用任意时间 TTL 代替 server roster。
- P1：测试首帧、颜色变化、玩家离线、同名重连和大量 roster churn；断言离线 key 被删除、在线 key 仍可比较且只发一次叙事。
- 验收：`seen` 只包含当前在线 roster；颜色变更叙事仍按 `player.uuid` 路由，重连不会误报历史颜色。

## §6 接入面与跨仓契约

- **Inputs**：Redis `bong:world_state`、`WorldStateV1.players`、`PlayerProfile.uuid/name/cultivation.qi_color_*`。
- **Outputs**：`QiColorNarrationTracker.ingest` 返回 `NarrationV1`，由 runtime 发布到 `AGENT_NARRATE`；仅增量反馈，不写 server 状态。
- **共享类型或事件**：复用 TypeBox `WorldStateV1/PlayerProfile`、`QiColorState` 字段和 `NarrationV1`；不新增 roster event。
- **server 符号**：`publish_world_state_to_redis`、`build_world_state_snapshot`、`canonical_player_id`、`CH_WORLD_STATE`；server 继续提供当前在线 roster。
- **agent 符号**：`QiColorNarrationTracker.ingest`、`renderQiColorNarration`、`runtime.ts` 的 tracker 生命周期、`CHANNELS.WORLD_STATE/AGENT_NARRATE`。
- **client**：无变更；client 不持有该 tracker，颜色 UI 仍消费既有 server payload。
- **worldview / qi_physics**：真元颜色仅作既有 cultivation 观测；不增加/扣除真元，不改 `qi_physics` ledger 或 `SPIRIT_QI_TOTAL`。
