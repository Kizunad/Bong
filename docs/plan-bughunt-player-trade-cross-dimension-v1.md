# plan-bughunt-player-trade-cross-dimension-v1

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

## Skeleton Fix Plan

- [ ] 在 `TradeOfferRequest` 派发前，按发起者与目标玩家的 `CurrentDimension` 做同维度校验；缺失维度时采用和邻近交互一致的默认策略，并写清测试期望。
- [ ] 在 `PendingTradeOffer` 中记录发起时双方维度，或在 `handle_trade_offer_responses` 接受阶段重新查询双方 `CurrentDimension`，确保接受时仍同维。
- [ ] 若跨维拒绝，向发起者或接受者发送明确反馈，例如“对方不在此界，无法交易”，避免 UI 静默。
- [ ] 保持现有距离、终止态、character id、物品存在、装备物品拒绝等交易保护不回退。
- [ ] 复查 `SparringInvite` 是否同样只按距离建立运行态；若发现同类跨维缺口，另开独立 plan，不混入本修复。

## 验收测试计划

- [ ] server 单测：Overworld 发起者与 TSY 目标同坐标时，`dispatch_trade_offers` 不生成 pending trade，也不向目标发送 `TradeOffer` payload。
- [ ] server 单测：发起时同维、接受前目标切到 TSY，同一 offer 接受不得交换物品，pending 应被清理或拒绝。
- [ ] server 单测：同维且距离内的正常玩家交易仍能完成，双方 inventory revision、LifeRecord、SocialExposure 行为保持现有预期。
- [ ] server 单测：同维但超距、终止态、物品缺失、装备物品等既有拒绝用例继续通过。
- [ ] 协议/集成测试：伪造 `target: "entity:<id>"` 指向跨维玩家时，服务端拒绝而不是依赖客户端准星可见性。

## 风险

- `CurrentDimension` 缺失实体的默认语义必须与现有 server 交互门禁一致；否则测试 helper 需要补齐维度组件，避免误把测试默认当生产行为。
- 如果 pending 中记录维度，跨维返回后是否允许继续接受需要明确：建议接受阶段必须“当前同维”，而不是只要求“发起时同维”。
- 交易拒绝反馈要避免泄露目标实体是否存在；面向普通玩家只提示“不在此界/无法交易”即可。
