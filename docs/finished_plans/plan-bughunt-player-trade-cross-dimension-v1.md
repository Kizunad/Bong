# plan-bughunt-player-trade-cross-dimension-v1

> **Active BugFix Plan（2026-09-27）**。一句话主题：玩家交易的发起与接受阶段都必须校验双方当前位面，禁止仅凭相近 XYZ 坐标完成跨维换货。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 第一性原理复现与边界确认 | ✅ 2026-09-27 |
| P1 | 发起/接受双阶段同维门禁与拒绝反馈 | ✅ 2026-09-27 |
| P2 | 跨维、切维、同维与既有拒绝契约测试 | ✅ 2026-09-27 |
| P3 | server 完整门禁、主线同步、归档与最终验证 | ✅ 2026-09-27 |

## Preflight（2026-09-27）

- `docs/worldview.md`：已只读核对位面、面对面交易与交易暴露相关锚点，本 plan 不修改该文件。
- `docs/finished_plans/`、`docs/plan-*.md`：grep `player-trade-cross-dimension` / `TradeOffer` / `CurrentDimension`，仅命中 `plan-refactor-c2s-gate-v1` 的矩阵记录与已归档的相邻交易计划；没有重复的 active plan。
- `docs/plans-skeleton/`：确认本 skeleton 是唯一同名占位，已升格为本 active plan；相邻 NPC 交易门禁属于不同根因。
- `reminder.md`：不存在。

## 接入面与决议

- **进料**：`TradeOfferRequest`、`TradeOfferResponseEvent`、双方 `Position` / `CurrentDimension` / `Lifecycle` / `PlayerInventory`。
- **出料**：仅同维且在 `CHAT_EXPOSURE_RADIUS` 内时生成 `PendingTradeOffer`、发送既有 `TradeOfferPayloadV1` 并调用 `exchange_inventory_items`；跨维请求向请求方发送拒绝提示。
- **共享类型 / event**：复用 `world::dimension::{CurrentDimension, DimensionKind}`，不新增位面枚举、pending 字段或协议字段；缺失 `CurrentDimension` 通过 `dimension_or_overworld` 回退 `Overworld`。
- **Pending 决议（2026-09-27）**：`PendingTradeOffer` 只保存交易双方实体、character id、物品实例与过期 tick，不保存发起时位面；接受阶段重新读取双方当前 `CurrentDimension`，成交必须同维，跨维响应清理 pending 并反馈拒绝。
  - **代码调研依据**：`server/src/social/mod.rs:127-136` 定义 pending 字段且没有位面快照；`server/src/social/mod.rs:1090-1097` 与 `server/src/social/mod.rs:1207-1216` 分别在发起、接受时重新读取双方位面并先于距离门禁拒绝跨维；`server/src/social/mod.rs:262-266` 将派发与接受排在 `DimensionTransferSet` 之后。
  - **测试依据**：`server/src/social/mod_tests.rs:1442` 的 `trade_offer_dispatch_reports_cross_dimension_before_distance_gate`、`server/src/social/mod_tests.rs:1493` 的 `trade_offer_dispatch_runs_after_same_tick_dimension_transfer`、`server/src/social/mod_tests.rs:1835` 的 `trade_response_rejects_dimension_changed_before_acceptance`、`server/src/social/mod_tests.rs:1887` 的 `trade_response_runs_after_same_tick_dimension_transfer` 覆盖反馈顺序、同 tick 调度和接受前切维。
  - **plan 锚点**：决议对应本 plan「接入面与决议·跨仓库契约」、`§P0 验真结论`、`§P1 最小修复` 与 `§P2 验收测试`；实现与验证证据汇总在 `§Finish Evidence`。
- **跨仓库契约**：继续消费 `trade_offer_request` / `trade_offer_response`，发送既有 `TradeOfferPayloadV1`；本修复只改变 server 门禁，client / agent / schema 无需改动。
- **worldview 锚点**：`worldview.md §九` 面对面交易、`§十一` 交易暴露、`§十六` 坍缩渊独立位面；面对面不能跨位面只靠 XYZ 成立。
- **qi_physics 锚点**：本 plan 只交换既有物品实例，不生成、衰减或转移真元，无新增 ledger 路径。

## P0 验真结论

- 对拍 C2S `resolve_trade_offer_target`、`dispatch_trade_offers` 与 `handle_trade_offer_responses`：主线在两个 social 阶段都只比较 `Position`，没有上游同维 gate 或下游补偿；漏洞仍真实存在。
- 旧本地提交 `04b203f21` 只把 skeleton 升格为 active plan，没有代码修复；本次按主线现状重写，不 cherry-pick 旧代码。
- `SparringInvite` 路径已复查，属于独立交互链路，本 plan 不混入；若后续确认需要同维约束，另开独立 plan。

## Bug 摘要

玩家对玩家交易的服务端链路只按 `Position` 三维距离判断双方是否“附近”，没有校验 `CurrentDimension`。因此两个玩家若处在 Overworld / TSY 的相近坐标，只要发起端能提交目标玩家的 protocol entity id，服务端会允许发出交易 offer；目标接受后，`handle_trade_offer_responses` 会再次只按坐标距离放行，并真实交换双方 `PlayerInventory` 中的物品。

这不是 NPC 交易链路：NPC 交互已有同维度门禁。缺口只在玩家对玩家 `TradeOfferRequest` / `TradeOfferResponseEvent` 社交交易链路。

## 对实际游玩体验的影响

- 玩家可能收到来自“另一个位面”的交易邀请，界面上看不到对方、无法通过空间关系判断风险，却能完成换货。
- 跨维同坐标或旧目标 id 残留时，玩家背包物品会被服务端权威交换，造成“隔空交易”“跨界换货”的体验断裂。
- 普通未改客户端通常需要准星命中玩家才能发起交易，但服务端协议只信任 `target: "entity:<id>"`，不能把客户端可见性当作安全边界。

## 证据定位

- `server/src/network/client_request_handler.rs:1204`-`1225`：`TradeOfferRequest` 分支解析 target 后直接发 `TradeOfferRequest` 事件，没有传入/检查发起者与目标的 `CurrentDimension`。
- `server/src/network/client_request_handler.rs:10352`-`10371`：`resolve_skill_cast_target` 只把 `entity:<protocol_id>` 解析到 ECS entity。
- `server/src/network/client_request_handler.rs:10464`-`10470`：`resolve_trade_offer_target` 只拒绝空 target 和 `entity_bits:`，随后复用上述 entity resolver。
- `server/src/network/client_request_handler.rs:10403`-`10455`：同文件的 `QiColorInspect` 是正确对照，显式比较 observer / observed 的 `CurrentDimension`。
- `server/src/network/client_request_handler.rs:10494`-`10508`：NPC engagement resolver 也是正确对照，先比较玩家与 NPC 维度再判距离。
- `server/src/social/mod.rs:1024`-`1056`：`dispatch_trade_offers` 的玩家查询没有 `CurrentDimension`，只以 `initiator_pos.get().distance(target_pos.get()) > CHAT_EXPOSURE_RADIUS` 拦截。
- `server/src/social/mod.rs:1110`-`1155`：`handle_trade_offer_responses` 接受阶段同样没有 `CurrentDimension`，再次只按 3D 距离判断，然后进入物品交换。
- `server/src/social/mod.rs:1168`-`1173`：通过 `exchange_inventory_items` 对双方背包实例做真实交换。
- `server/src/social/mod.rs:127`：pending trade 不保存发起时双方维度，接受阶段无法核对“仍在同维”。

## 触发路径

1. 玩家 A 在 Overworld，玩家 B 在 TSY，二者 `Position` 坐标距离小于 `CHAT_EXPOSURE_RADIUS`。
2. A 的客户端或调试/恶意 C2S 发送 `trade_offer_request`，`target` 为 B 的 `entity:<protocol_id>`，`offered_instance_id` 为 A 背包内物品。
3. `resolve_trade_offer_target` 全局解析 protocol id 为 B 的 ECS entity；`dispatch_trade_offers` 只看距离和物品，给 B 下发 `TradeOffer` 并登记 pending。
4. B 接受并回传 `requested_instance_id`。
5. `handle_trade_offer_responses` 只校验双方还活着、character id 未变、3D 距离仍在范围内、物品仍存在；随后交换背包物品。

## 反方审查记录

第一轮反方结论：真实，不是重复。反方确认 `TradeOfferIntentHandler` 的正常准星路径不能替代服务端门禁；协议只有 `target` 字符串，服务端 resolver 只做 entity id 解析；`EntityManager` 不携带维度约束；`dispatch_trade_offers` 与接受阶段都没有读取 `CurrentDimension`。

第二轮反方结论：仍成立，不是误报。反方继续核对客户端路径、Valence entity id 解析、跨维传送位置语义和重复 PR，结论是服务端交易链路没有“同维度”约束；#930 是 social witness/exposure 跨维，#940 是 NPC 拒交易误套，#882 是发起端自动选物，均不覆盖本 bug 的玩家交易成交链路。

## P1：最小修复

- [x] 在 `dispatch_trade_offers` 派发前按双方当前 `CurrentDimension` 做同维度校验；缺失组件由 `dimension_or_overworld` 按 `Overworld` 处理。
- [x] 在 `handle_trade_offer_responses` 接受阶段重新查询双方当前 `CurrentDimension`，成交瞬间不再依赖发起时快照。
- [x] 跨维发起和接受都发送明确拒绝反馈，且不生成或消费物品交换状态。
- [x] 保持现有距离、终止态、character id、物品存在、装备物品拒绝与容量拒绝保护不回退。
- [x] 已复查 `SparringInvite`；该独立链路不在本修复范围内。

## P2：验收测试

- [x] server 单测：Overworld 发起者与 TSY 目标同坐标时，`dispatch_trade_offers` 不生成 pending trade，也不向目标发送 `TradeOffer` payload。
- [x] server 单测：发起时同维、接受前目标切到 TSY，同一 offer 接受不得交换物品，pending 被清理并反馈拒绝。
- [x] server 单测：同维且距离内的正常玩家交易仍能完成，双方 inventory revision、LifeRecord、SocialExposure 行为保持现有预期。
- [x] server 单测：同维但超距、终止态、物品缺失、装备物品与容量拒绝等既有用例继续通过。
- [x] 协议路径沿用既有 `entity:<id>` resolver，跨维目标在 social server gate 被拒绝，不依赖客户端准星可见性。

## P3：闭环门禁

- [x] `git fetch origin && git merge origin/main`：Already up to date，修复基于最新主线复验。
- [x] `scripts/build-token.sh cargo fmt --check`、`scripts/build-token.sh cargo clippy --all-targets -- -D warnings`、`scripts/build-token.sh cargo test` 全部通过。
- [x] 归档前 HEAD 与关键文件未被主线更新覆盖；门禁与主线复验已完成，推送和 PR 将在归档提交后执行。

## 风险

- `CurrentDimension` 缺失实体的默认语义必须与现有 server 交互门禁一致；否则测试 helper 需要补齐维度组件，避免误把测试默认当生产行为。
- 已决（2026-09-27）：`PendingTradeOffer` 不保存发起时位面；接受阶段重新读取双方当前 `CurrentDimension`，只有当前同维才允许成交，跨维响应清理 pending 并反馈拒绝。
- 交易拒绝反馈要避免泄露目标实体是否存在；面向普通玩家只提示“不在此界/无法交易”即可。

## Finish Evidence

### Kody review 修订（2026-09-27）

- 调度时序意见成立：`dispatch_trade_offers` 与 `handle_trade_offer_responses` 均显式排在 `DimensionTransferSet` 后，接受处理另排在派发之后，避免同 tick 传送仍读取旧位面。
- 校验顺序意见成立：发起与接受都先完成生命周期与当前位面门禁，再执行 `CHAT_EXPOSURE_RADIUS` 距离校验；超距跨维请求仍收到跨维拒绝反馈。
- 文档意见成立：本节与上方风险项已收口 pending 规则；pending 不记录位面，成交瞬间重新读取双方当前位面并要求同维。
- 文档可核验性意见成立：决议已补 `server/src/social/mod.rs` 文件/行号、四个交易契约测试名及 `§跨仓库契约`、`§P0`、`§P1`、`§P2`、`§Finish Evidence` 章节锚点。

### 落地清单

- `server/src/social/mod.rs`：`dispatch_trade_offers` 与 `handle_trade_offer_responses` 使用 `CurrentDimension` 双阶段同维门禁；两者均排在 `DimensionTransferSet` 后，`dimension_or_overworld` 统一缺省维度语义并发送拒绝反馈。
- `server/src/social/mod_tests.rs`：新增超距跨维反馈、同 tick 发起传送、同 tick 接受传送三条调度与顺序契约测试；既有交易行为继续通过。
- `docs/plan-bughunt-player-trade-cross-dimension-v1.md`：记录 Preflight、验真、接入面、阶段状态与最终证据。

### 关键 commit

- `7ce8f9562`（2026-09-27）：将同名 skeleton 升格为 active plan。
- `9ebc8b9c0`（2026-09-27）：在交易发起与接受阶段加入当前位面校验并锁定回归测试，`Model: gpt-6-luna`。
- `5f7c4c89a`（2026-09-27）：按 Kody review 修复交易调度时序与跨维距离校验顺序，补充同 tick 契约测试，`Model: gpt-6-luna`。
- 旧本地提交 `04b203f21`（2026-07-18）仅含文档升格，已保留为 `backup/plan-bughunt-player-trade-cross-dimension-v1-local-20260927`，代码修复按今日主线重写。

### 测试结果

- `scripts/build-token.sh cargo test -p bong-server 'social::tests::trade_'`：16 passed，0 failed。
- `scripts/build-token.sh cargo fmt --check`：passed。
- `scripts/build-token.sh cargo clippy --all-targets -- -D warnings`：passed。
- `scripts/build-token.sh cargo test`：passed；lib 10,381 tests、各 integration/unit binaries 与 doc-tests 均通过（doc-tests 3 passed、5 ignored）。
- PR check run `36293023250`：`bot-e2e (1)`、`bot-e2e (2)` 及全套 checks 均 passed；重跑的 `production_craft_preparation_return` 已通过，与本 PR 交易改动无关。

### 跨仓库核验

- server 命中 `TradeOfferRequest`、`TradeOfferResponseEvent`、`TradeOfferPayloadV1`、`CurrentDimension` 与 `exchange_inventory_items`。
- client / agent / schema 的既有 `trade_offer_request` / `trade_offer_response` wire shape 未改变，无需改动。

### 遗留 / 后续

- `SparringInvite` 未并入本 plan；若确认其独立链路需要同维约束，另开 plan。
- PR 合并前保留 slot-2、远端 claim 与 backup 分支；由主干在 PR 合并后统一清理。
