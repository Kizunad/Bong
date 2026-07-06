# plan-trade-offer-first-item-autopick-v1（骨架）

> **骨架（草案）**。一句话主题：当前正常玩法里，玩家对另一名玩家发起交易时，client 不会让发起方选择“我要拿哪件物品去换”，而是直接把库存里按 `displayName + instanceId` 排序后的第一件物品当作报价发给 server。影响是：**玩家会在面对面交易里稳定报错货，无法按自己意图拿出指定物品，甚至可能把本不想出的高价值物品暴露给对方并被立即成交**。

> 立项动机：`server/src/social/mod.rs` 的交易主链已经支持任意 `offered_instance_id`，目标方 UI 也已经能挑选回礼；当前缺口只在发起端 client 入口，属于 `social-ui / inventory / gameplay` 交叉主路径上的单跳断线，实际游玩可达、体感直接、修复面局部，适合先立 skeleton-only PR 固化证据。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 交易发起端错误自动选物回归 | fix_pr | ⬜ |

## P0 — 交易发起端错误自动选物回归

- **现象**：`client/src/main/java/com/bong/client/social/TradeOfferIntentHandler.java:18-43` 在 candidate/dispatch 两处都直接调用 `TradeOfferScreenBootstrap.firstTradeItem(...)`；该 helper（`TradeOfferScreenBootstrap.java:135-144`）会从 grid/hotbar 中挑出 `displayName` 最小、再按 `instanceId` 打破平手的那一件。随后 `ClientRequestSender.sendTradeOfferRequest(..., item.instanceId())`（`ClientRequestSender.java:245-246`）把这件物品直接发给 server。整条路径**完全不读取当前拖拽物、当前选中 hotbar、当前 inspect 选中态或任何“玩家明确想交易哪件物品”的 UI 状态**。
- **为什么这是 bug，不是设计**：`docs/finished_plans/plan-social-v1.md:220` 明写 `§6.2` 是“**双方面对面 + 拖拽物品 + 双方确认**”，`docs/finished_plans/plan-social-v1.md:322-325` 的 Phase 4 也写了“**交易：面对面拖拽 UI + 物品交换**”。而 server 侧 `dispatch_trade_offers`（`server/src/social/mod.rs:933-1005`）明确消费 `request.offered_instance_id` 并据此构造 `offered_item` payload，说明协议与权威逻辑本来就支持“发起方明确选择某个实例”；现在只是发起端 client 偷偷把“第一件字典序物品”代入了这个参数。
- **可达链路**：仓库内唯一的 `sendTradeOfferRequest(...)` 调用点就是 `TradeOfferIntentHandler`；而 `TradeOfferScreen` 注释也自承自己只是“`minimal trade response prompt`”，只负责目标方挑回礼。也就是说，**正常玩家路径没有第二个发起端选物入口**；只要玩家用现有交互键发起交易，就一定走到这个自动选物分支。
- **对实际游玩体验的影响**：玩家想拿丹药换草药、拿低价值杂物试探对方、或刻意隐藏自己包里高价值战利品时，都会被系统改成“自动报价第一件字典序物品”。对方看到的 `offered_item` 也是这件错物，可能直接接受，导致玩家在一次正常面对面交易里用错误物品成交；即便对方拒绝，发起方也无法用现有 UI 改成自己真正想出的货。
- **建议修复范围 / 模块**：优先收口 `client/src/main/java/com/bong/client/social/TradeOfferIntentHandler.java`、`client/src/main/java/com/bong/client/social/TradeOfferScreenBootstrap.java`，必要时接入 `client/src/main/java/com/bong/client/inventory/InspectScreen.java` 或独立的 outgoing trade picker。修复目标应是：**让 `sendTradeOfferRequest` 使用玩家显式选中的 instance，而不是 helper 里推导出的“第一件物品”**。
- **验收抓手**：至少补 4 组 pin。1) 背包里有多件可交易物品时，发起交易必须携带玩家显式选择的 `instance_id`，不能回退为字典序第一件。2) grid 与 hotbar 同时有物品时，当前选中态必须优先于排序结果。3) 未做显式选物时应拒绝发起或先弹选物 UI，而不是静默代入默认物品。4) 端到端里目标方收到的 `offered_item.instance_id/display_name` 必须与发起方实际选中的实例一致。

## 反方裁决摘要

1. Round 1 怀疑“这只是最小 MVP，默认拿第一件物品并不算 bug”。裁决：不成立；如果是设计决定，`plan-social-v1` 不会把交易定义成“拖拽物品 + 双方确认”，server 协议也不需要 `offered_instance_id` 这种实例级参数。当前不是“少了高级 UI”，而是**现有交互把错误实例送进了已存在的权威链路**。
2. Round 2 怀疑“也许别处还有发起端选物入口，`TradeOfferIntentHandler` 只是快捷键捷径”。裁决：不成立；全仓 `sendTradeOfferRequest` 只有这一处调用，`TradeOfferScreen` 又只覆盖目标方回礼选择，因此正常玩法下不存在第二条能指定报价物品的 client 主路径。
3. 人工复核进一步确认：`firstTradeItem` 的实现与“玩家意图”完全无关，只与 snapshot 排序有关；因此只要背包里有两件以上可交易物品，这个 bug 就会稳定显形，不依赖竞态或罕见状态。

## 开放问题

1. 发起端修复应该挂在 inspect/drag 流程里，还是补一份独立 outgoing trade screen？需要在 fix PR 中定一个最小且不绕路的入口。
2. 若未来允许交易装备栏 / 容器内物品，发起端选物 UI 需要同步对齐 `server/src/social/mod.rs` 当前“只接受 container/hotbar、拒绝 equipped”的语义边界，避免再引入第二层 client/server 分歧。

## 审计来源

bughunt 线程 Y（scope 收敛 `server/src/social/`、`client/src/main/java/com/bong/client/social/`、少量 `inventory` 读链）。候选经主代理逐点 read/grep 复核，并按两轮默认怀疑式裁决保留。当前结论是 **report-only**：先提交 skeleton plan，把玩家影响、设计偏离、修复面与验收抓手讲清，再由后续 fix PR 单独落地。当前会话未暴露用户要求的 `multi_agent_v1.spawn_agent` 工具，因此两轮对抗为本地等价复核而非真实工具调用。
