# plan-npc-trade-bundle-count-loss-v1

> 一句话主题：NPC 交易 UI / metadata / server 买路三者对 `NpcTradeOffer.count` 的语义不一致。client 明确把报价渲染成 bundle（如“灵草 x3 / 总价 12 骨币”），但 `NpcTradeRequest` 落地时服务端只按静态 catalogue 解析 `template_id + price`，最终固定 `add_item_to_player_inventory(..., 1, ...)`，导致玩家按 bundle 价付款却只拿到 1 件货。

> **这个 bug 对实际游玩体验的影响**：正常游玩里，玩家和散修/凡人 NPC 交易时会直接看到“灵草 x3”“次品丹药 x2”这类 bundle 报价，也会看到对应总价；但真正点“确认交易”后，服务端永远只给 1 件。结果是玩家在完全合法、无 dev 命令、无改包的正常交易流程里，被系统稳定少发货：付了 3 份货的骨币，只收到 1 份，剩下 2 份凭空消失。这会直接破坏 NPC 交易的可信度，也会让前期通过 NPC 买灵草/次品丹药补给的经济曲线失真。

## 阶段总览

| 阶段 | 主题 | 状态 |
|---|---|---|
| P0 | 核心修复：服务端成交必须消费 live `NpcTradeInventory` offer，而不是只看静态 catalogue | ⬜ |
| P1 | 契约与 UI 对齐：`count` / `price_bone_coins` / 当前 offer subset 全链路锁定 | ⬜ |
| P2 | 饱和测试：bundle 成交、单件成交、不可见报价拒绝、金额扣减全覆盖 | ⬜ |

验收日期：全部 P ✅ 后填写（当前为 skeleton，2026-07-05）。

## 证据链

1. **生成端明确生产 bundle 数量**：`server/src/npc/trade.rs:618-631` 从 `TRADE_CATALOGUE.count_min/count_max` 生成 `TradeOffer { template_id, display_name, count: item_count, price_bone_coins }`；其中 `spirit_grass` 明确允许 `count_min=1, count_max=5`（`trade.rs:508-520`）。
2. **同步链完整保留 `count`**：`server/src/network/npc_metadata.rs:305-313` 把 `offer.count` 原样下发为 `NpcTradeOfferS2c.count`；测试 `npc_metadata_trade_offers_serializes` 也专门构造了 `count: 3`（`npc_metadata.rs:824-851`）。
3. **client 明确把它当成交 bundle 渲染，而不是装饰文案**：
   - `client/src/main/java/com/bong/client/npc/NpcTradeScreen.java:181-185` 右侧选中栏显示 `offer.displayName() + " x" + offer.count()` 与 `"总价: " + offer.priceBoneCoins() + " 骨币"`；
   - 规格文档 `docs/finished_plans/plan-npc-combat-gear-v1.md:488-511` 也把成交物写成 `selectedItem ("灵草 ×3")` + `priceTotal ("总价: 12 骨币")`，并明确“交易成功后 close screen + 显示事件流消息”。
4. **服务端成交时丢失 live offer 语义**：
   - `server/src/network/client_request_handler.rs:1302-1384` 处理 `NpcTradeRequest` 时，只用 `npc_trade_catalog_entry(target.archetype, &requested_item_id)` 取回 `(template_id, base_price)`；
   - `NpcEngagementTarget` 本身不携带 `NpcTradeInventory`（`client_request_handler.rs:9867-9885`），因此成交路径完全看不到当前 NPC 实际生成了哪几条报价、每条报价的 `count` 是多少；
   - 最终发货处 `add_item_to_player_inventory(..., template_id, 1, combat_clock.tick)`（`client_request_handler.rs:1474-1481`）把入包数量硬编码成 `1`。
5. **结论**：`trade_offers.count` 不是“客户端装饰字段”，而是已经被 server 生产、被 wire 传输、被 UI 呈现、被规格文档定义的成交语义；唯独最终 buy path 把它丢掉了。

## P0 — 核心修复：成交必须消费 live offer

### 交付物

1. **`server/src/network/client_request_handler.rs`**
   - 扩展 NPC 交易请求读取面：`NpcEngagementRequestParams` / `NpcEngagementItem` 需要能读取 `Option<&NpcTradeInventory>`，或者在 `resolve_npc_engagement_target` 返回值里携带当前 NPC 的 live offer 快照。
   - `NpcTradeRequest` 处理逻辑不再只调用 `npc_trade_catalog_entry(archetype, requested_item_id)` 取“静态 template + 价格”，而是必须先从当前 NPC 的 live `NpcTradeInventory.offers` 中找到本次点击对应的 **具体 offer**。
   - 成功成交后发货数量改为 `offer.count`，价格取该 offer 的 `price_bone_coins` 再叠加既有信誉修正；不得继续把数量硬编码成 `1`。
2. **成交语义保持单一真相来源**
   - 静态 `npc_trade_catalog_entry` 仍可保留为“目录合法性 / archetype 白名单”辅助函数，但不能再作为 bundle 成交的唯一数据源。
   - 最终成交必须以 live offer 为准，因为 live offer 才同时包含：
     - 当前 NPC 此刻实际展示给玩家的 subset；
     - 本次成交的 bundle 数量 `count`；
     - 与 UI 一致的 `price_bone_coins`。
3. **防御性拒绝**
   - 如果 `requested_item_id` 不在该 NPC 当前 `NpcTradeInventory.offers` 中，应拒绝成交并给出可见反馈，而不是按 catalogue 悄悄放行。
   - 这条不是因为正常 UI 会发送非法值，而是为了堵上“当前成交路径绕开 live offer subset”的根因洞。

### 关键约束

- 不改 client 协议字段名。`NpcTradeRequest.requested_item_id` 继续沿用现有字符串字段即可；问题不在协议缺字段，而在 server 没有把该字符串重新绑定回 live `TradeOffer`。
- 不把 `count` 再手抄一份进第二套 catalogue/映射表。`count` 的唯一运行时来源应继续是 `NpcTradeInventory.offers[*].count`。

## P1 — 契约与 UI 对齐

### 交付物

1. **规格回归口径**
   - 明确“`price_bone_coins` 是该条 bundle 的总价，不是单件价”。
   - 明确“`NpcTradeOffer.count` 是实际发货数量，不是纯展示字段”。
2. **服务端反馈文案**
   - 成交成功文案从当前的 `买下 {template_id}` 升级为能反映 bundle 数量的文案，例如“买下 灵草 x3”，避免修完发货后消息仍旧误导。
3. **可选收口**
   - 如果 `npc_trade_catalog_entry` 继续存在，需新增测试确保它只承担“目录合法性”职责，不再偷偷覆盖 live offer 的 `count` / `price`。

## P2 — 饱和测试

### 必加测试

1. `npc_trade_request_grants_full_bundle_count`
   - 构造 `NpcTradeInventory { offers: [TradeOffer { template_id: "spirit_grass", count: 3, price_bone_coins: 12 }] }`
   - 发送一次正常 `NpcTradeRequest`
   - 断言：
     - 背包实际新增数量是 `3`，不是 `1`
     - 骨币只扣一次 bundle 总价 `12`（再叠加信誉修正时用修正后总价）
2. `npc_trade_request_single_count_offer_still_grants_one`
   - 锁住单件报价不回归。
3. `npc_trade_request_rejects_offer_not_present_in_live_inventory`
   - 当前 NPC live offers 不含请求项时拒绝成交。
4. `npc_trade_request_uses_live_offer_count_not_catalogue_default`
   - catalogue 中 template 合法，但 live offer `count=5` 时仍必须发 `5`。
5. `npc_trade_request_success_message_includes_bundle_count`
   - 锁住玩家可见反馈不再把 bundle 成交说成单件成交。

## 开放问题

1. `NpcEngagementTarget` 是直接扩成携带 `Vec<TradeOffer>`，还是只携带 `Entity` 然后在成交分支二次 query `NpcTradeInventory`？
   - 倾向后者：避免把大块交易数据塞进所有 inspect/dialogue 路径共用的 target struct。
2. `npc_trade_catalog_entry` 的最终定位是什么？
   - 倾向保留为 catalogue 对齐 / 历史 alias（`lingcao` -> `spirit_grass`）辅助，但 bundle 数量与价格一律由 live offer 决定。

## 两轮反方裁决摘要

- **第 1 轮反方**：怀疑 `count` 只是 UI 装饰，真实成交可能本来就是“单件价 / 单件发货”。裁决：不成立。`plan-npc-combat-gear-v1` 明确把报价定义成 `灵草 ×3 / 总价 12 骨币` 的同一笔成交，`npc_metadata` 也不是只传 `display_name`，而是专门有 `count` 字段和测试；因此 `count` 是协议语义，不是装饰文本。
- **第 2 轮反方**：怀疑 `add_item_to_player_inventory(..., 1, ...)` 里的 `1` 也许不是数量，或者后面还有别的地方按 `offer.count` 补发。裁决：不成立。`server/src/inventory/mod.rs:1495-1568` 明确把第五参命名为 `stack_count`，`stack_count == 0` 还会报错；全链路 grep 也没有任何后续补发 `offer.count - 1` 的逻辑。现状就是固定只发 1 件。

## 审计来源

bughunt 线程 AD（worktree `.worktree/bughunt-loop-20260705-ad`，分支 `bughunt-loop-20260705-ad-preview-npc-ui`，基线 `origin/main@fb41c96a4`）在 `npc/world/preview/inventory-ui` 主路径专项扫描中确认。该题与既有 `npc trade gate` 主题不同：这里不是“能不能交易”的门控错误，而是“正常交易后稳定少发货”的成交语义断裂。
