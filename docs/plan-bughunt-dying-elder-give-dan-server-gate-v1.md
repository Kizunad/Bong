> **Skeleton（bughunt）**。一句话主题：垂死大能 `give_dan_to_elder` C2S 在服务端消耗回元丹前没有校验目标类型、状态、距离和维度，导致 stale / 跨距 / 跨维请求可先删丹再被下游静默拒绝，或远程推进大能给丹状态。

## 摘要

`ClientRequestV1::GiveDanToElder` 入口只把 `pill_instance_id`、`elder_entity_id` 交给 `handle_give_dan_to_elder`（`server/src/network/client_request_handler.rs:2632`）。该 helper 校验玩家背包里确实有 `huiyuan_pill`，再用 `EntityManager::get_by_id` 解析目标，然后立刻 `consume_item_instance_once` 删除丹（`server/src/network/client_request_handler.rs:14394`），最后才 emit `GiveDanToElderIntent`（`server/src/network/client_request_handler.rs:14417`）。

这条路径缺少普通 NPC 交互已有的服务端门禁：同文件 `resolve_npc_engagement_target` 会先比对 `CurrentDimension`，再校验 6 格范围（`server/src/network/client_request_handler.rs:10505`、`server/src/network/client_request_handler.rs:10517`）。给丹路径没有等价的 `Position` / `CurrentDimension` / `DyingElderBlackboard` / `DyingElderState` 前置查询。

下游 `dying_elder_give_dan_system` 才查询 `(&mut DyingElderBlackboard, &mut DyingElderState)`；目标不是垂死大能会 `continue`（`server/src/fauna/dying_elder.rs:540`），目标已 `Betrayal` / `Dead` 也会 `continue`（`server/src/fauna/dying_elder.rs:564`）。这些拒绝都发生在丹已从玩家背包删除之后。

## 实际游玩体验影响

玩家在垂死大能遭遇中切维、离开区域、目标死亡/状态结束、或客户端保留了过期 encounter id 后再点给丹，会看到回元丹被服务端扣掉，但大能不回血、不计入 `dan_received`、不触发收丹反馈和后续守信/背叛结局。恶意或异常客户端还可以绕过距离/维度限制，对远处或另一维度的大能推进给丹流程，破坏这条稀有遭遇“近身施救并承担风险”的玩法边界。

## 去重

- 不重复 `docs/plan-bughunt-r2-findings-v1.md`：该题是垂死大能给丹 entity id / sentinel 命名空间漂移。
- 不重复 `docs/plans-skeleton/plan-bughunt-dying-elder-give-dan-input-v1.md`：该题是默认 G 输入链路到不了 C2S。
- 不重复 `docs/plans-skeleton/plan-bughunt-dying-elder-release-overflow-v1.md`：该题是垂死大能死亡释放真元 overflow。
- 本题专注服务端 C2S 权威门禁与消耗顺序：消耗回元丹前必须证明目标是同维、近距、可接丹的垂死大能。

## 对抗复核

- Round 1：通过。反方确认 `EntityManager::get_by_id` 只是全局 protocol entity id 映射，不含同维、可见、距离语义；下游状态/类型校验在删丹之后，不能防止吞丹。
- Round 2：通过。反方确认这不是纯恶意客户端问题：`DyingElderEncounterStore` 激活后主要靠死亡类事件/断线清理，未见离区、超距、跨维清理；客户端给丹发送只读 store 中的 `elderEntityId`，不复核距离/维度。反方同时确认 `consume_item_instance_once` 不返回可恢复的完整物品，handler 丢弃返回值后下游 `continue` 没有补偿路径。

## TODO

| ID | 任务 | 状态 |
|---|---|---|
| P0 | 给丹 C2S 消耗前加入目标类型/状态/距离/维度权威校验 | ⬜ |
| P1 | 调整消耗顺序：只有目标校验和事件资源存在后才删除丹；下游拒绝不得吞丹 | ⬜ |
| P2 | 补回归：非垂死大能、Dead/Betrayal、跨维、超距、stale id 均不扣丹 | ⬜ |
| P3 | 补 e2e：近距 Plea/Recovering 大能给丹仍能扣丹、回血、推进 `dan_received` 和反馈 | ⬜ |
