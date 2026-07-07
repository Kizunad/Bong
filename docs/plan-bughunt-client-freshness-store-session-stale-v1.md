# plan-bughunt-client-freshness-store-session-stale-v1

> 一句话主题：client `FreshnessStore` 未在断线 / 切服 / 重连时清理，旧 `instance_id` 鲜度缓存会污染新会话 tooltip，并让 InspectScreen 对新会话同 id 物品误发 `freshness_probe` 与本地音效反馈。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | `FreshnessStore` 跨会话残留导致保鲜 tooltip / 感保鲜探针误判 | fix_pr | ⬜ |

## P0 - `FreshnessStore` 跨会话残留导致保鲜 tooltip / 感保鲜探针误判

- **问题定义（fix_pr）**：`client/src/main/java/com/bong/client/processing/state/FreshnessStore.java:10-25` 把 `freshness_update` 写入进程级 static `ConcurrentHashMap`，生产代码只有 `upsert/get`，清理入口只有测试用 `clearForTests()`。断线清理主路径 `client/src/main/java/com/bong/client/BongNetworkHandler.java:857-900` 未清它；背包断线清理 `client/src/main/java/com/bong/client/inventory/InspectScreenBootstrap.java:59-64` 也只清 `InventoryStateStore`、装备 store 与 `QiColorObservedStore`。结果是上一连接的保鲜缓存会留到下一连接。

- **复现路径**：
  1. 在会话 A 中，对 instance id 为 `42` 的可保鲜物品触发感保鲜，server 回 `freshness_update`。
  2. `ProcessingServerDataHandler.handleFreshnessUpdate()` 将 `item_uuid="42"` 写入 `FreshnessStore`（`client/src/main/java/com/bong/client/network/processing/ProcessingServerDataHandler.java:41-48`）。
  3. 断线 / 切服 / 重连到会话 B；client 清了 inventory snapshot，但没有清 `FreshnessStore`。
  4. 会话 B 的新背包中如果存在 instance id 同为 `42` 的物品，`FreshnessTooltipHook.tooltipLine("42")` 会直接显示会话 A 的旧鲜度（`client/src/main/java/com/bong/client/hud/FreshnessTooltipHook.java:11-16`）。
  5. 玩家在 InspectScreen 中对该物品按感保鲜键时，`maybeProbeFreshness()` 用 `Long.toString(item.instanceId())` 查旧缓存；命中后会发送 `freshness_probe` 并播放本地提示音（`client/src/main/java/com/bong/client/inventory/InspectScreen.java:2606-2624`）。

- **根因链路**：
  1. server 回包里的 `item_uuid` 实际是 `instance_id.to_string()`，不是跨 server / 跨世界唯一 UUID；`server/src/network/freshness_probe_emit.rs:58-60` 明确写入 `ev.instance_id.to_string()`，测试也 pin 住 `"42"`。
  2. client 侧门禁也使用 `Long.toString(item.instanceId())` 查询同一个缓存 key。
  3. `FreshnessStore` 没有连接维度、世界维度、玩家维度、过期时间或生产清理。
  4. `inventory_snapshot` 只会刷新新会话背包，不能删除 `FreshnessStore` 中旧 key；client 解析背包物品时也不会用 snapshot 覆盖 freshness cache。
  5. `plan-interaction-intent-cleanup-v1` 把感保鲜触发收窄为“`FreshnessStore` 有记录才发包”，但这让旧缓存从单纯 tooltip 残留升级为输入门禁误判。

## 实际游玩体验影响

- 玩家切服或重连后，可能在新角色 / 新世界的普通物品上看到上一会话的“鲜度: xx/100” tooltip，误以为这件物品具备保鲜状态或已经被探测过。
- 更明显的是 InspectScreen 会把旧缓存当作“该物品已知有保鲜数据”，玩家按感保鲜键时会听到本地提示音、发出 `freshness_probe`，但 server 可能因为当前物品无 freshness 而静默拒绝；玩家看到的是“我明明触发了探针但没有有效结果”的错反馈。
- 如果新会话同 id 物品本身也有 freshness，第一次权威回包最终会覆盖旧缓存，但覆盖前的 tooltip 与交互反馈仍可能短暂显示上一会话数据。
- 该问题不破坏服务端权威状态；风险集中在 client UI 可信度、误音效、误发包和玩家对保鲜信息的错误判断。

## 修复建议

1. 给 `FreshnessStore` 增加生产用 `clearOnDisconnect()`，断线时清空 `ENTRIES`。
2. 在 `BongNetworkHandler.clearClientStateOnDisconnect()` 或 `InspectScreenBootstrap.clearInventorySnapshot()` 中调用该清理，和 inventory snapshot 生命周期对齐。
3. 增加 client 单测：写入 `FreshnessStore` 后模拟 disconnect 清理，断言 tooltip 为空，`InspectScreen.maybeProbeFreshness()` 对同 instance id 新物品不再发包 / 不播放本地反馈。
4. 可选强化：如果后续 `inventory_snapshot` 携带 freshness 权威字段，再改为按 snapshot 重建 cache；当前不能让旧 cache 继续充当跨会话 truth source。

## 对抗审查

- **第 1 轮反方检查**：支持。确认 `FreshnessStore` 是 static `ConcurrentHashMap` 且无生产清理；`freshness_update` 只 upsert 不删除；tooltip 和 InspectScreen 都直接消费该缓存；server 回包 key 是 `instance_id.to_string()`，不能证明跨会话绝不碰撞。弱点是 server 权威会校验 `freshness_probe`，通常不造成服务端状态破坏。
- **第 2 轮反方检查**：支持开单一 plan PR，定级为 minor/medium client UI bug。确认不重复 #1068-#1072，也不重复 `plan-bughunt-forge-lingtian-processing-deadpath-v1`：后者是 processing 主链死代码 / 无生产 emit / 无 UI 打开，本题是已有 `freshness_update` 消费后的 client static cache 生命周期 bug。与 forge UI stale、toast stale、identity stale 属同类生命周期问题，但对象、入口和玩家后果不同。

## 去重说明

- 不重复禁题：不是 #1049 mineral_probe_result 网络线程碰 HUD/SFX，不是 #1066 Forge 静态 store stale，不是 #1077 LingtianSessionStore stale，不是 #1086 炼丹 UI stale，不是 #1092 TSY ExtractStateStore stale，不是 #1099 PerceptionEdgeStateStore stale。
- 不重复 `docs/plans-skeleton/plan-bughunt-forge-lingtian-processing-deadpath-v1`：本题不讨论 processing 启动 / 结算 / UI 创建是否可用，只讨论 `freshness_update` 一旦进入 client cache 后，断线生命周期不正确。
- 不重复 `plan-interaction-intent-cleanup-v1`：该 plan 的门禁策略正是本 bug 的前置条件；旧缓存会误通过“有记录才探针”的门禁。

## 验收建议

- Client 单测覆盖 `FreshnessStore.clearOnDisconnect()`。
- Client 单测覆盖断线清理后 `FreshnessTooltipHook.tooltipLine(oldId)` 返回空字符串。
- Client 单测覆盖断线清理后 `InspectScreen.maybeProbeFreshness()` 对同 id 新物品返回 false，不发送 `freshness_probe`，不触发本地音效。
