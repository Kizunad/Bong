# plan-bughunt-npc-trade-bundle-count-bridge-v1（骨架）

> **骨架（草案）**。一句话主题：NPC 交易展示路把 `trade_offers.count` 明确下发并显示成“这笔交易交付 N 件”，但执行路从协议到结算全程丢失数量，只按 `requested_item_id` 单件发货，形成 **“看见 x3，付款后只拿 1”** 的 economy/trade 侧断桥。

> 立项动机：本轮 bughunt 避开 trade gate / trade offer autopick / 骨币示好 silent signal / forge step_state / BaiYanPeng 引怪漂移后，沿 economy/trade sidepath 继续追“展示契约是否被结算路完整消费”。结果确认：r4 已修掉“展示目录/买路目录不一致”后，**数量桥仍未接上**，属于新的 real-on-main 交易缺口。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 复现路径与表象 | plan_skeleton | ⬜ |
| P1 | 根因链路（display → protocol → settlement） | plan_skeleton | ⬜ |
| P2 | 影响面、修复建议、反方裁决 | plan_skeleton | ⬜ |

## P0 — 复现路径与表象

- **复现前提**：找任一可交易 `Commoner` / `Rogue` NPC，打开 `NpcTradeScreen`；只要刷到任一 `count > 1` 的报价即可。这个前提不是偶发假设，`TRADE_CATALOGUE` 明确允许 `spirit_grass 1..5`、`ling_xi_wan_flawed 1..3`、`ju_ling_dan_flawed 1..3`、两类卷轴 `1..2`（`server/src/npc/trade.rs:510-551`）。
- **玩家侧步骤**：
  1. 与可交易 NPC 对话，进入交易界面。
  2. 选中任一显示为 `x2/x3/...` 的货物；client 会把它渲染成 `offer.displayName() + " x" + offer.count()`，并把 `offer.priceBoneCoins()` 标成“总价”（`client/src/main/java/com/bong/client/npc/NpcTradeScreen.java:181-185`）。
  3. 点击“确认交易”；client 请求只发送 `npc_entity_id + requested_item_id`，协议里没有数量字段（`client/src/main/java/com/bong/client/network/ClientRequestProtocol.java:931-950`）。
  4. server 收到请求后，只按 `requested_item_id` 查目录并调用 `add_item_to_player_inventory(..., template_id, 1, ...)`，固定发 **1 件**（`server/src/network/client_request_handler.rs:1335-1480`）。
- **可观察结果**：
  - UI 事前承诺的是“`灵草 x3` / `灵息丸（次品）x2` / …，总价 N 骨币”。
  - 聊天和背包事后得到的是单件成交：`"你用 {price} 枚骨币从 {NPC} 手中买下 {template_id}"`，且没有 `xN`；背包只新增 1 件（`server/src/network/client_request_handler.rs:1474-1496`）。
- **设计意图侧证**：`TradeOffer.count` 的注释不是“库存剩余”，而是“可供购买的数量”（`server/src/npc/trade.rs:47-55`）；历史 plan 的 JSON 示例也把 `trade_offers` 写成 `{ "template_id": "lingcao", "display_name": "灵草", "count": 3, "price_bone_coins": 12 }`（`docs/finished_plans/plan-npc-combat-gear-v1.md:347-349`）。

## P1 — 根因链路

- **生成路有数量**：`assign_npc_trade_inventory()` 按 `count_min/count_max` 抽出 `item_count`，写入 `TradeOffer.count`（`server/src/npc/trade.rs:618-631`）。
- **S2C 路有数量**：`NpcTradeOfferS2c` 把 `count` 与 `price_bone_coins` 一起下发给 client（`server/src/network/npc_metadata.rs:73-80`）。
- **GUI 路消费数量**：`NpcTradeScreen` 在列表和详情面板都显示 `x<count>`，而且把 `priceBoneCoins` 直接标注为“总价”，没有任何“单价/库存”的文案（`client/src/main/java/com/bong/client/npc/NpcTradeScreen.java:181-185`, `229-231`）。
- **C2S 路丢数量**：`encodeNpcTradeRequest()` 只发 `requested_item_id`，没有 `count` / `offer_id` / `selected_quantity` / `bundle_size`（`client/src/main/java/com/bong/client/network/ClientRequestProtocol.java:931-950`）。
- **结算路再丢一次**：server 只用 `requested_item_id` 查 `npc_trade_catalog_entry()` 得到 `(template_id, base_price)`，随后固定 `add_item_to_player_inventory(..., 1, ...)`；结算逻辑完全不读取 `NpcTradeInventory.offers`，也不核对展示时那条 offer 的 `count`（`server/src/network/client_request_handler.rs:1335-1480`）。
- **因此形成的断链**：`count_min/max → TradeOffer.count → NpcTradeOfferS2c.count → GUI xN` 全通，但 **C2S 与 settlement 两端无任何字段承接**，最终 quantity 在成交瞬间塌成常量 `1`。

## P2 — 影响面、修复建议、反方裁决

### 这个 bug 对实际游玩体验的影响

- 玩家会直接遇到“明明摊位上写的是 `x3`，付了整单价格却只拿到 1 个”的体感欺骗，属于非常硬的 **所见即所得破坏**。
- economy 侧后果不是纯 UI 文案瑕疵，而是**真实结算短发货**：玩家亏掉骨币、错误评估 NPC 交易性价比，还会误以为某些物资（尤其灵草/次品丹）产出被暗砍。
- 这条断桥还会污染后续平衡判断：设计者以为 Commoner/Rogue 报价在卖 bundle，实际 live 服上玩家拿到的是单件，trade 入口的补给效率被系统性低估。

### 影响面

- **所有 `count > 1` 的 NPC 报价** 都受影响，不限某个模板；当前目录里最容易命中的就是 `spirit_grass` 和两种次品丹（`server/src/npc/trade.rs:510-535`）。
- **所有 client 版本** 都会被误导，因为显示逻辑已经固化为 `x<count>` + “总价”（`client/src/main/java/com/bong/client/npc/NpcTradeScreen.java:181-185`）。
- **现有测试盲区**：仓库里有 catalogue 对齐、价格传播、count range 边界测试，但没有一条测试把“GUI 展示 count”与“结算发货数量”做端到端 pin，因此这条 bridge 缺失一直未被撞红。

### 修复建议

1. **优先方案**：把 `NpcTradeRequest` 扩成携带 `offer_id` 或 `template_id + quoted_count + quoted_price`，server 结算时必须回查当前 `NpcTradeInventory.offers` 命中的那条 offer，再按 `offer.count` 发货。
2. 若设计上其实要表达“库存数量”而非“本次成交数量”，那就反过来统一语义：client 改文案为“库存/剩余”，server 增加库存递减与多次购买逻辑；在现状下这是更大改动，因为当前请求没有库存语义，交易后也不会减少同 NPC 的报价。
3. 无论选哪条路，都应补一条端到端回归：给定 `trade_offers.count = 3` 的报价，点击购买后断言背包新增数量必须为 3，或断言 UI/协议明确声明这是库存而非 bundle。

### 反方裁决

> 退化处理：当前 Codex 会话没有可用的 subagent / delegate 能力，本轮无法再开外部怀疑者代理；以下两轮反方裁决由同一会话手工执行，方法是“先立最强反方论点，再逐条回到代码和既有 plan 找反证”。

#### 第一轮反方：`count` 也许只是库存，不代表一次成交发多少

- **反方论点**：`x3` 可能只是 NPC 还有 3 个库存；点一次买 1 个并不一定错。
- **驳回理由**：
  - UI 明确写的是“总价: N 骨币”，不是“单价”；若 `count` 是库存，当前文案至少应该出现“单价/库存”二分（`client/src/main/java/com/bong/client/npc/NpcTradeScreen.java:181-185`）。
  - `TradeOffer.count` 注释写的是“可供购买的数量”，不是“剩余库存槽位”或“库存余量”（`server/src/npc/trade.rs:47-55`）。
  - 更关键的是，若它真是库存，现有协议和 server 都没有库存递减状态，也没有二次购买消耗同一条 offer 的逻辑；库存语义在实现上根本不闭合。

#### 第二轮反方：也许这是故意简化，`xN` 只是展示 stack 外观，不算 bug

- **反方论点**：设计上可能只想让 NPC 看起来“货多”，实际成交固定 1 件，是一种轻量表现法。
- **驳回理由**：
  - 既有 plan 的 JSON 示例把 `count: 3` 和 `price_bone_coins: 12` 写在同一条 trade offer 上，语义是“这条报价”而不是“纯装饰用 stack 外观”（`docs/finished_plans/plan-npc-combat-gear-v1.md:347-349`）。
  - `assign_npc_trade_inventory()` 特地为每条报价抽 `count_min/count_max`，说明数量是交易数据的一部分，而不是只给 UI 做摆设（`server/src/npc/trade.rs:618-631`）。
  - 如果这是故意简化，最低限度也应在 confirm 文案、聊天回执或协议层显式把成交数量固定为 1；目前所有可见层都把玩家引向“这单是 xN”的理解，因此不能归类为 harmless simplification。

## 审计来源

bughunt 2026-07-05，范围限定 economy / trade / currency / market sidepaths；显式避开 trade gate、trade offer autopick、骨币示好 silent signal、forge step_state、BaiYanPeng 引怪漂移。结论基于静态代码审计 + 既有 plan/协议/GUI 文案对照；本次未运行游戏内复现脚本，也未修改任何源码。
