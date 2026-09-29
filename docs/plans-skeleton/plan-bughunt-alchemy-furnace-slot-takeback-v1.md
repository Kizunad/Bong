# plan-bughunt-alchemy-furnace-slot-takeback-v1

> 来源 Issue：#1661、#1558。炼丹炉槽取物只改 client 本地数组，没有服务端取回语义；拖回空槽会再次发送 FeedSlot，造成双扣，已锁炉材料也无法可靠取回。
>
> 阶段总览：P0 ⬜ 定义 server-owned 的槽取回或明确拒绝交互；P1 ⬜ 补 session/inventory 权威回推和重复投料回归。

## §0 摘要

AlchemyScreen.mouseClicked 在炉槽取物时只清空 furnaceItems/furnaceSlots 并把物品交给本地 dragState，没有 C2S take-back。拖回空槽时 attemptDrop 无条件发送 AlchemyIntent.FeedSlot。server 的 handle_alchemy_feed_slot 会按配方校验并 consume_item_instance_once；现有 handle_alchemy_take_back 不是“取回单槽”，而是推进剩余 tick、结束整炉并结算结果。client 的本地移动因此既不能回滚 server 已扣的材料，也不能代表 server 槽状态。

## §1 实际游玩体验影响

- 玩家从炉槽拿起材料时，画面看似取出，server 仍认为材料已经投入；重新拖回可能再次扣背包同类材料。
- 已经锁定的炼丹 session 没有可核验的单槽取回反馈，断线或拒绝后 client/server 的炉槽和背包视图会分叉。

## §2 复现路径

1. server 建立 active alchemy session，client 收到 alchemy_furnace/alchemy_session。
2. 将材料拖入炉槽；AlchemyScreen:749-760 发送 FeedSlot，server:5515-5682 校验后扣除 inventory 并回推快照。
3. 在炉槽点击材料；:680-686 只在本地清空槽，不发送请求。
4. 把该对象拖回空槽；:751-759 再次发送 FeedSlot，server 再次按背包实例/数量处理，形成双扣或拒绝后仍保持本地假状态。

## §3 根因证据

- client/src/main/java/com/bong/client/alchemy/AlchemyScreen.java:680-686 的 furnace slot 点击没有 intent sink/ClientRequestSender 调用，只操作 furnaceItems、furnaceSlots 和 dragState。
- :749-760 拖回空槽后立即 dispatch AlchemyIntent.FeedSlot；该路径没有“这是从 furnace slot 取回后再放回”的身份。
- server/src/network/client_request_handler.rs:5515-5682 的 handle_alchemy_feed_slot 校验 owned furnace/session/recipe/material/count，并在 :5648-5656 调用 consume_item_instance_once。
- server/src/network/client_request_handler.rs:5685-5868 的 handle_alchemy_take_back 会 tick 到 target_duration、end_session、resolve outcome 并发放产物；它不是可复用的单槽撤回入口。
- server/src/network/alchemy_snapshot_emit.rs:104-120、:219-337 推送炉和 session 快照；agent/packages/schema/src/server-data.ts:836-872 镜像 alchemy_session，但当前没有描述“槽撤回确认”的独立事件。

## §4 非重复比对

- 现有 alchemy take_back 语义是收炉结算；不能把本 issue 与炼丹结算/真元注入 bug 合并，也不能在 client 直接调用该入口假装撤回。
- inventory snapshot 的 revision/权威替换可用于收敛 UI，但不能替代炉 session 对槽状态的 server 校验。

### 立项检查记录（2026-09-28）

- docs/worldview.md：检索“炼丹、丹炉、材料、消耗、真元、锁定、收炉”，核对炼丹循环和资源消耗章节；未找到 client 本地炉槽可改写 server 状态的规则。
- docs/finished_plans/：检索 AlchemyScreen、handle_alchemy_feed_slot、handle_alchemy_take_back、alchemy_session；命中 alchemy 基础 plan，未发现单槽取回契约。
- active plan（docs/plan-*.md）：检索 FeedSlot、TakeBack、AlchemyScreen、alchemy_session；未发现 active plan 已拥有 slot take-back API。
- docs/plans-skeleton/：检索 furnace slot、alchemy feed、take_back、consume_item_instance_once；未发现同主题骨架。
- docs/plans-skeleton/reminder.md：检索炼丹炉、槽取回、双扣、FeedSlot；仓内无该主题延后事项。

## 接入面与跨仓契约

- **Inputs**：client furnacePos/slot index、session identity、InventoryItem instance identity；server AlchemyFurnace、AlchemySession、PlayerInventory、RecipeRegistry。
- **Outputs**：server 权威炉槽/session/inventory 快照；成功取回返还或保留材料，失败时 client 不标记已取出。
- **共享类型 / 事件**：AlchemyIntent.FeedSlot、现有 handle_alchemy_feed_slot、handle_alchemy_take_back（仅整炉结算）、alchemy_furnace、alchemy_session、inventory_snapshot、inventory revision。
- **server 契约符号**：with_owned_furnace_mut、handle_alchemy_feed_slot、consume_item_instance_once、send_furnace_from_furnace、send_session_from_furnace/send_session_from_completed_session。
- **agent**：若 P0 新增 C2S AlchemyTakeBackSlotRequest 或独立 S2C receipt，必须同步 agent/packages/schema/src/client-request.ts、server-data.ts、schema-registry.ts 和 samples；若产品选择“不允许撤回”而只做 client fail-closed，则 agent 无变更并写明现有 schema 足够。
- **client**：有变更；AlchemyScreen 的取槽交互、AlchemyIntent/ClientRequestSender、AlchemyFurnaceHandler/AlchemySessionHandler 的权威回推处理。
- **worldview 锚点**：docs/worldview.md 的炼丹循环、材料消耗和真元成本章节；撤回不得凭空复制材料或绕过炉位置/所有权。
- **qi_physics**：炼丹真元仍由 server 既有 session/ledger 路径负责；槽撤回不能在 client 修改 qi_current，也不能复用整炉 take_back 造成重复结算。

## §5 修复骨架

### P0 建立明确的槽撤回语义

- 首选新增一个与整炉结算分离的 server-owned slot take-back request，携带 furnace position、session identity、slot index 和材料/实例 identity；server 校验所有权与 staged 状态后原子地更新 session/inventory，并同时推送 alchemy_session 与 inventory_snapshot。
- 若设计决定不允许撤回，则点击炉槽必须保持本地状态不变或进入 pending，直到 server 拒绝；绝不能只清本地数组后让拖回再次走 FeedSlot。
- 不得把现有 handle_alchemy_take_back 当作单槽 API；它只能继续负责完成整炉并结算 outcome。

### P1 回归与验收

- 投入一次、撤回一次、重新投入一次只发生一次 server 消耗；重复 request、错误 slot、错误 session 和非 owner 都 fail closed。
- server 拒绝或断线后，client 由权威 furnace/session/inventory snapshot 恢复，不保留幽灵材料。
- 任何 qi 注入/消耗和炉结算的守恒断言保持原有路径，不因 UI 取回新增 event-only 真元流动。
