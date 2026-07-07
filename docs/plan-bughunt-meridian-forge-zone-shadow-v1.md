# plan-bughunt-meridian-forge-zone-shadow-v1

> Skeleton Plan / BugHunt server-qi r14 / 2026-07-07

## 0. 一句话

经脉淬炼 `MeridianForge` 成功后扣玩家 `Cultivation.qi_current`，但 `credit_forge_cost` 仍把消耗真元手写加到会被 `Zone.spirit_qi` 镜像覆盖的 `WorldQiAccount zone:<name>`，没有迁移到 `credit_pending_inflow`，导致淬炼消耗的真元落入影子账并可能在后续同步中蒸发。

## 1. 实际游玩体验影响

玩家在经脉 Inspect 面板点击“淬炼·流速”或“淬炼·容量”会真实消耗当前真元并提升经脉属性。按末法资源守恒口径，这笔真元应回到可追踪的环境或待分配池，之后通过区域灵气回流影响玩家/NPC 修炼生态。

当前实现下，玩家反复淬炼会看到自己的真元减少、经脉变强，但 `pending_inflow` 不增加，区域回流体系也不会收到这笔成本。后续 heartbeat / dormant / LOD 类系统按 `zone.spirit_qi * QI_ZONE_UNIT_CAPACITY` 重建 zone ledger 镜像时，这笔 `zone:<name>` 余额会被旧字段值覆盖，等价于把玩家投入淬炼的真元从可游玩环境和守恒账本里抹掉。

## 2. 触发路径

1. 客户端 Inspect 经脉面板选择已开启经脉，点击“淬炼·流速”或“淬炼·容量”。
2. 客户端发送 `ForgeRequest`，服务端 `ClientRequestV1::ForgeRequest` 转发为 `ForgeRequest` event。
3. `cultivation::forging_system` 在 `breakthrough_system` 后执行，调用 `forge_with_ledger`。
4. `try_forge` 扣除 `cultivation.qi_current -= cost` 并提升对应经脉 tier。
5. `credit_forge_cost` 把 `cost` 加到 `QiAccountId::zone(zone_name)` 并追加 `QiTransferReason::MeridianForge` audit。
6. `pending_inflow_account` 未变化；下一次 zone mirror 同步可覆盖该 ledger 余额。

## 3. 证据定位

- `client/src/main/java/com/bong/client/inventory/InspectScreen.java:397` / `:407`：经脉面板有“淬炼·流速”“淬炼·容量”按钮，调用 `ClientRequestSender.sendForgeRequest`。
- `server/src/network/client_request_handler.rs:762`：服务端消费 `ClientRequestV1::ForgeRequest` 并发送 `ForgeRequest` event。
- `server/src/cultivation/mod.rs:336`：`forging_system.after(breakthrough_system)` 已注册进修炼 tick 链。
- `server/src/cultivation/forging.rs:99`：`try_forge` 直接扣 `cultivation.qi_current -= cost`。
- `server/src/cultivation/forging.rs:214`：成功后调用 `credit_forge_cost`。
- `server/src/cultivation/forging.rs:263` 到 `:275`：`credit_forge_cost` 只 `set_balance(zone, old + amount)` 并 `push_transfer_audit`，没有 `credit_pending_inflow`。
- `server/src/cultivation/forging.rs:400` 到 `:435`：现有测试只断言 `zone:<spawn>` ledger balance 增加，未断言 `zone.spirit_qi` 或 `pending_inflow`。
- `server/src/qi_physics/ledger.rs:481` 到 `:489`：注释明确 `zone:<name>` 会被 `zone.spirit_qi * QI_ZONE_UNIT_CAPACITY` 整体覆写，旧“只写 ledger 不动真实字段”是蒸发路径。
- `server/src/qi_physics/ledger.rs:503` 到 `:523`：`credit_pending_inflow` 是开脉/突破消耗真元回充独立待分配池的范本。
- `server/src/cultivation/meridian_open.rs:299` 到 `:318`、`server/src/cultivation/breakthrough.rs:608` 到 `:625`：开脉/突破已迁到 `credit_pending_inflow`，并点名不再注水 `zone:<name>`。
- `server/src/world/heartbeat.rs:2285` 到 `:2313`：zone inflow 方向是先用 `zone.spirit_qi` 覆写 zone ledger 镜像，再从 `pending_inflow` transfer 后写回 `zone.spirit_qi`；没有 `zone:<name> ledger -> ZoneRegistry.spirit_qi` 的反向同步。

## 4. 非重复说明

- 不重复 #1050：#1050 是 Crafting 真元固定落 spawn zone 账户；本题是 `server/src/cultivation/forging.rs` 的经脉淬炼 `QiTransferReason::MeridianForge`。
- 不重复 #1046 / #1102：二者是骨煞 / BossDrain 落入 zone 镜像账；本题是玩家 Inspect 面板可触发的经脉淬炼生产路径。
- 不重复 #1056：#1056 是 NPC daily-life 回气凭空造真元；本题是玩家主动消耗真元后回充落点错误。
- 不重复 #1066 / #1087：这些是锻造 UI / forge session 入口问题；本题不是武器炼器 session，而是 `cultivation::forging` 经脉淬炼。

## 5. 修复计划

### P0：把 `MeridianForge` 迁到待分配池

- 将 `credit_forge_cost` 改为调用 `qi_physics::credit_pending_inflow(..., QiTransferReason::MeridianForge)`，对齐开脉/突破的 zone-qi economy 决议。
- 保留失败回滚语义：pending inflow credit 失败时撤销 `Cultivation` 与 `MeridianSystem` 变更。
- 不扩大到 BossDrain / SkullFiendDrain / Crafting 等其它路径。

### P1：修正测试契约

- 更新 `forge_system_credits_spent_qi_to_zone_ledger`，不再把 `zone:<name>` balance 增加视为正确结果。
- 新增成功淬炼后 `pending_inflow` 的权威落点断言。
- 新增一次 zone mirror 重同步/heartbeat 回流后的守恒回归，证明 `MeridianForge` 消耗不会被 `zone.spirit_qi` 覆写蒸发。

### P2：覆盖边界

- 缺 `WorldQiAccount`、找不到 zone、角色缺稳定 id 时，淬炼不得扣真元或改变经脉。
- NPC 触发 `ForgeRequest` 时使用 `QiAccountId::npc`，玩家触发时使用 `QiAccountId::player`。
- 负灵域 / 满区 / 零 cost / 非有限输入沿用 `credit_pending_inflow` 的既有守恒语义。

## 6. 验证计划

- server 单测：`cultivation::forging` happy path、缺 ledger、缺 zone、缺 stable id、mirror 覆写回归。
- server gate：`cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`。
- e2e 后续：经脉面板点击“淬炼·流速/容量”后，断言玩家真元减少、经脉 tier 提升、对应 `pending_inflow` 增加，且 `bong:qi/ledger` 守恒快照不掉量。

## 7. 对抗复核结论

- Round 1：SURVIVES。反方未找到 `zone:<name>` ledger 反向同步；确认现有同步方向是 `zone.spirit_qi` 覆写 ledger mirror，`MeridianForge` 仍使用旧手写加账。
- Round 2：SURVIVES_WITH_SCOPE_CHANGE。候选成立，但范围应收窄为 `server/src/cultivation/forging.rs::credit_forge_cost` 漏迁移到 `credit_pending_inflow(..., QiTransferReason::MeridianForge)`；不要扩大为所有 Boss / 骨煞 / Crafting zone-field 语义重定。反方确认玩家 Inspect 面板、C2S handler、`ForgeRequest` event、`forging_system` 注册均已接通，且 #1046/#1050/#1102 不覆盖 `MeridianForge`。

