# BugHunt：库存交互来源、距离与体表拖放小问题

> 来源 Issue：#1381、#1658、#1664。三个问题都发生在 client inventory intent 边界，但分别锁定来源位置、遗骸距离和体表/经脉落位；合并只为一次性补齐客户端 fail-closed 与 server 权威回推契约。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 统一库存拖放/搜刮 intent 的来源与距离门 | ⬜ |
| P1 | 体表/经脉落位接入 server 权威请求与回推 | ⬜ |

## §0 摘要

`InspectScreen` 把装备/快捷栏来源拖入搜刮面板时仍以空字符串和 `-1` 行列构造 `external_container_move`；遗骸候选没有复用 server 的 2.5 格拾取范围；体表/经脉落位只写 `BodyInspectComponent` 本地 map，不发 C2S。三条路径都会产生客户端乐观状态与 server 权威状态分叉，前两条还会发送必然被拒的请求。

## §1 游玩影响

- 装备或快捷栏物品拖入 loot 面板后显示已放入，server 拒绝后又被快照弹回。
- 远处按 G 会选中遗骸并发送无效请求，玩家没有距离反馈。
- 药材落到体表/经脉后看似装备，下一次库存快照会把它放回背包，且没有实际经脉/身体效果。

## §2 复现路径

1. 在 `InspectScreen` 从 EQUIP/HOTBAR 拖物品到 loot grid，观察 `srcCid == null` 仍发送 external move。
2. 在遗骸同步范围外按 G，观察 `RemainsLootIntentHandler.candidateAt` 仍返回候选。
3. 将 1×1 物品拖入 physical 或 meridian layer，观察只调用 `apply*` 后本地落位，server 没有相应 intent。

## §3 今天 `origin/main` 的根因证据

- `client/src/main/java/com/bong/client/inventory/InspectScreen.java:1648-1654` 将空 `sourceContainerId`、`sourceRow=-1`、`sourceCol=-1` 填进 `sendMove`；server `server/src/network/client_request_handler.rs:7610-7686` 对非 container 来源直接拒绝。
- `client/src/main/java/com/bong/client/inventory/RemainsLootIntentHandler.java:30-42` 没有距离判断；server `server/src/inventory/mod.rs:1146` 的 `REMAINS_PICKUP_RANGE_SQ` 是 2.5 格平方范围，`server/src/network/client_request_handler.rs:2235-2247` 才是权威 intent 入口。
- `InspectScreen.java:1682-1699` 只调用 `BodyInspectComponent.applyPhysicalItem/applyMeridianItem` 和 `dragState.drop()`；这些方法只改 `BodyInspectComponent` 本地 map，server 侧现有 `ClientRequestSender.sendApplyPill*`/inventory intent 才会产生权威效果。

## §4 非重复比对与立项检查记录

- **worldview**：查 `docs/worldview.md` 的装备、经脉、搜打撤、物品所有权关键词；本骨架不改正典或真元规则。
- **finished_plans**：查 `plan-inventory-v1.md`、`plan-inspect-cleanup-v1.md`、`plan-dropped-loot*`、`plan-meridian-severed-v1.md`；确认只复用既有 inventory/meridian intent，不重复 server 掉落结算。
- **active plan**：查 `docs/plan-*.md` 的 `external_container_move`、`RemainsLootIntent`、`BodyInspectComponent`；未发现同三处调用点的 active 修法。
- **skeleton**：查 `external_container_move`、`REMAINS_PICKUP_RANGE_SQ`、`applyPhysicalItem`；未发现同一 client 交互边界骨架。
- **reminder.md**：仓内无 `docs/plans-skeleton/reminder.md`，无相关条目。

## §5 修复骨架

- P0：来源类型不是 server `ContainerLoc` 时 fail closed，先 `returnDragToSource`，不发送空容器位置；遗骸候选复用 server 的 2.5 格平方范围，超距不生成请求。
- P1：为体表/经脉落位接入已有的 server-owned inventory/药丸 intent（或新增明确的 body/meridian intent），成功后等权威 snapshot；拒绝、断线和重复请求都恢复本地拖拽，不把本地 map 当成成功。
- 验收：非法来源零 C2S；远距遗骸零 C2S；体表/经脉成功后 server inventory/body/meridian 状态和 client 快照一致，失败不丢物品。

## §6 接入面与跨仓契约

- **Inputs**：`InspectScreen` 的 `DragState`/`InvLocation`、`RemainsSync` 候选与玩家位置、`BodyPart`/`MeridianChannel` 和物品实例 ID。
- **Outputs**：`external_container_move`、`RemainsLoot` 或 body/meridian intent；server 回推 inventory/body/meridian snapshot。
- **共享类型或事件**：复用 `ClientRequestProtocol.ContainerLoc`、`ClientRequestV1::ExternalContainerMove`、`RemainsLootIntent`、`InventorySnapshot`；不造第二份拖拽状态。
- **server 符号**：`handle_external_container_move`、`RemainsLootIntent`、`REMAINS_PICKUP_RANGE_SQ`、现有 inventory mutation funnel；必须由 server 做距离、所有权和实例校验。
- **agent**：无变更；库存与 client interaction 不经 Redis agent IPC，依据 `server/src/network` 现有 request/response 链。
- **client**：`InspectScreen`、`RemainsLootIntentHandler`、`BodyInspectComponent`、`ClientRequestSender` 有变更；失败时保持 fail-closed。
- **worldview / qi_physics**：对应 `worldview.md` 的物品所有权、搜打撤和经脉交互；本骨架不新增真元流，若 body intent 触发丹药真元效果仍沿 server 既有 `qi_physics` ledger 入口。
