# BugHunt：炼丹整叠投料的客户端槽位数量与 server 脱节

> 来源 Issue：#1709。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 炉槽显示、feed count 与 server session 使用同一数量语义 | ⬜ |
| P1 | 投料/拒绝/收炉回归 | ⬜ |

## §0 摘要

AlchemyScreen.attemptDrop 把整叠物品放进本地 furnaceItems，但 feedCountForSlot 只向 server 发送配方所需的部分数量。服务端扣部分，客户端继续显示整叠，后续 T 键或再次拖动会把幻影数量当成已投材料。

## §1 游玩影响

玩家看到炉槽有 5 个材料但 server 只收了 3 个，收炉/取回/重投时数量和背包快照跳变，可能重复发送同一材料或误以为整叠已锁定。

## §2 复现路径

准备一叠数量大于配方需求的材料，拖入炉槽，观察 furnaceItems 显示整叠而 encodeAlchemyFeedSlot 的 count 只为需求量；随后收炉或再次操作该槽。

## §3 今天 origin/main 的根因证据

- client/src/main/java/com/bong/client/alchemy/AlchemyScreen.java:731-760 将整叠写入 furnaceItems 后按较小 count 发送。
- client/src/main/java/com/bong/client/network/ClientRequestProtocol.java 的 encodeAlchemyFeedSlot 携带 count；server/src/network/client_request_handler.rs:12233-12252 只按该 count 选择/扣实例并将结果写入 AlchemySession::feed_stage。
- server alchemy_session/alchemy_furnace snapshot 是权威回推；当前 client 没有按 server accepted count 重建本地槽。

## §4 非重复比对与立项检查记录

- worldview：查炼丹材料消耗、丹炉锁槽和收炉；不改丹方/真元公式。
- finished_plans：查 plan-alchemy-client-v1、plan-alchemy-v1、plan-alchemy-recycle-v1；确认既有 feed/take-back 语义，不重复炉槽 take-back (#1558) 或残卷 handoff (#1720)。
- active plan：查 plan-bughunt-alchemy-freshness-feed-v1.md、plan-bughunt-alchemy-ui-session-stale-v1.md；两者分别是 freshness 与断线清理，不覆盖 count 镜像。
- skeleton：查 furnaceItems、feedCountForSlot、AlchemyFeedSlot；未发现同根因骨架。
- reminder.md：仓内无 docs/plans-skeleton/reminder.md，无相关条目。

## §5 修复骨架

明确“本次 accepted count”是 client/server 共享语义：本地只显示实际发送并被确认的数量，或在 pending 期间不乐观清整叠；拒绝/部分接受由 alchemy_furnace/inventory snapshot 原子覆盖。禁止把本地整叠当成 server 已扣数量。投料仍沿 server 既有 feed_stage、库存消费和真元账本路径。

## §6 接入面与跨仓契约

- Inputs：炉位置、slot index、材料 instance_id/template_id、拖拽 stack count、配方 stage 需求。
- Outputs：AlchemyIntent.FeedSlot C2S、server alchemy_session/alchemy_furnace/inventory snapshot；client 槽显示实际 accepted count。
- 共享类型或事件：复用 AlchemyIntent.FeedSlot、AlchemySession、AlchemyFurnace、inventory revision；不造第二套 count。
- server 符号：handle_alchemy_feed_slot、AlchemySession::feed_stage、consume_item_instance_once、alchemy_snapshot_emit。
- agent：无变更；炼丹 feed 不经过 agent IPC。
- client：AlchemyScreen.attemptDrop/feedCountForSlot、AlchemyFurnaceStore handler；需要 pending/accepted count 的可观察状态。
- worldview / qi_physics：炼丹材料消耗和真元注入必须继续走 server 既有 qi_physics ledger；本骨架不新增扣款或释放路径。
