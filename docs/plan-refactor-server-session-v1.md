# plan-refactor-server-session-v1 — Server 交互 Session 统一生命周期框架（重构轨 R1）

> 所属总纲：`plan-refactor-master-v1.md`。一句话：把炼丹、手搓、锻造、采集、灵田、矿脉、灵木及相邻世界交互的 server session 收敛到统一生命周期框架，使断线、跨维、关服、重连、忙态与完成交付只保留一套权威语义。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 设计收口、吸收清单验真、冻结 `InteractionSession` 合同 | ✅ 2026-08-03 |
| P1 | `server/src/session/` 落地，craft 作为首个宿主 | ⬜ |
| P2 | 迁移 alchemy、forge、lingtian | ⬜ |
| P3 | 迁移 gathering、mineral、spiritwood 与世界交互锁 | ⬜ |
| P4 | bot e2e、吸收项结案、归档 | ⬜ |

## 1. P0 第一性现状

### 1.1 七域存储矩阵

| 域 | 当前权威状态 | P0 结论 |
|---|---|---|
| alchemy | `server/src/alchemy/session.rs:68` `AlchemySession`，挂在 furnace | 炉与 session 共同 checkpoint；不能只存玩家侧半张状态 |
| craft | `server/src/craft/session.rs:40` ECS `CraftSession` | 作为 P1 首宿主；保留已落地的 DB/join hydration 行为 |
| forge | `server/src/forge/session.rs:157` `ForgeSessions` Resource | station/session/已扣材料共同 checkpoint |
| gathering | `server/src/gathering/session.rs:57` `GatheringSessionStore` | 短时易失；中断时确定性 teardown |
| lingtian | `server/src/lingtian/systems.rs:196` `ActiveLingtianSessions` | 六类 actor session 统一按易失交互处理 |
| mineral | `server/src/mineral/session.rs:16` `MiningSession` | 短时易失；解除矿点与工具 claim |
| spiritwood | `server/src/spiritwood/session.rs:59` `WoodSessionStore` | 短时易失；`settling` 必须参与原子 teardown |

当前不存在 `SessionManager`。七域分别以 Component、Resource、facility-owned state 保存；`server/src/network/craft_emit.rs:541` 只用 `With<Client>` 限制 craft tick，不表示 UI pause，也不能覆盖其他 store。

### 1.2 生命周期缺口仍可达

- 玩家保存与 despawn 从 `server/src/player/mod.rs:350` 开始；七域没有一个统一的、排在持久化前的 teardown/checkpoint 门。
- 普通 external container 已有 `server/src/world/container_open.rs:184` 断线清锁，但跨维仍可保留 `opened_by`；局部补丁不能替代统一 hook。
- TSY 搜刮入口 `server/src/world/tsy_container_search.rs:306` 与撤离入口 `server/src/world/extract_system.rs:182` 没有双向 busy 声明，搜刮锁也没有统一断线清理。
- `server/src/cultivation/insight_flow.rs:229` 插入 `PendingInsightOffer`，选择、拒绝或校验失败会移除，但没有 deadline；同连接内 client 未展示/未回应时可长期悬挂。
- 灵田 Resource 会对所有 entry 持续 `tick_all`（`server/src/lingtian/systems.rs:249`），没有 actor live-state gate。
- 炼丹取回在 `server/src/network/client_request_handler.rs:16764-16766` 先 `end_session`，随后才从 `:16838` 尝试交付；交付失败时已经失去可重领状态。

## 2. 冻结的 `InteractionSession` 合同

P1 必须在 `server/src/session/` 暴露以下可 grep 的合同 symbol；实现可以按 Rust 借用约束拆成 trait + adapter，但不得改变本节语义：

```rust
pub trait InteractionSession {
    fn session_key(&self) -> SessionKey;
    fn owner_key(&self) -> &PlayerKey;
    fn durability(&self) -> SessionDurability;
    fn busy_claim(&self) -> BusyClaim;
    fn on_disconnect(&mut self, ctx: &mut SessionLifecycleCtx) -> SessionTransition;
    fn on_dimension_change(
        &mut self,
        from: DimensionKind,
        to: DimensionKind,
        ctx: &mut SessionLifecycleCtx,
    ) -> SessionTransition;
    fn on_shutdown(&mut self, ctx: &mut SessionLifecycleCtx) -> SessionTransition;
    fn on_reconnect(
        &mut self,
        player: Entity,
        ctx: &mut SessionLifecycleCtx,
    ) -> SessionTransition;
}
```

配套冻结类型：

- `SessionRegistry`：唯一 server 权威 owner/busy/lifecycle registry。
- `SessionKey { domain, id }`：稳定 session 身份；持久记录不得以 Bevy `Entity` 为主键。
- `PlayerKey`：canonical player id；`Entity` 只作当前连接的 runtime binding。
- `SessionDurability::{Checkpointed, Volatile}`：每个 adapter 注册时必须显式声明，不允许默认值。
- `SessionPhase::{Running, Paused, Suspended, AwaitingDelivery, Terminal}`：同一 session 不允许同时处于多个 phase。
- `TerminationCause::{VoluntaryCancel, Disconnect, DimensionChange, Shutdown, InvalidRestore, SuspensionExpired}`。
- `SuspensionPolicy`：声明 checkpointed session 的 `suspended_at_tick`、有限 `max_suspended_ticks`、过期扫描周期、管理员强制结案入口和 facility claim 回收动作；默认值冻结为 `SESSION_SUSPENSION_TTL_TICKS = 1_728_000`（20 TPS 下 24 小时）与 `SESSION_SUSPENSION_SCAN_INTERVAL_TICKS = 1_200`，不得配置为无限期保留。
- `SessionTransition::{Keep, Pause, SuspendAndCheckpoint, Teardown, AwaitDelivery, CommitTerminal}`。
- `BusyClaim`：声明 owner 与 world target 上占用的 busy classes；冲突矩阵集中注册。

### 2.1 不变量

1. `SessionRegistry` 中一个 `SessionKey` 只能有一个 owner；一个 runtime `Entity` 只能绑定同一 `PlayerKey` 的 session。
2. `Checkpointed` session 断线/关服后进入 `Suspended`，不得离线推进；必须记录 `suspended_at_tick` 并受 `SuspensionPolicy.max_suspended_ticks` 约束。重连通过 R3 guarded restore 后才重新绑定 `Entity`；超过 TTL 或管理员结案时转入 `InvalidRestore`/`SuspensionExpired` 结算，先完成 escrow delivery/refund，再释放 facility/target claim 并进入 `Terminal`。过期扫描和管理员入口必须幂等，且不会等待 owner 重连。
3. `Volatile` session 遇到断线、跨维或关服必须在同一生命周期门内 teardown；不得留下 owner entity、设施锁、target claim 或 `settling` 标记。
4. 所有 dimension-scoped session 在维度切换前终止。`TsyPresence` 是 transport 辅助状态，单独 checkpoint/restore，不得用“保留旧交互 session”修复 presence 撕裂。
5. client 的 screen/store 只能改善 UX，不能授予 session 或 busy 权限；恶意包、重复包和同 tick 竞态最终都由 registry 拒绝。
6. session 完成后先进入 `AwaitingDelivery`；只有 R10 `InventoryTxn::deliver(items)` 返回 `Delivered` 或 `Spilled(fallback)` 后，才可 `CommitTerminal` 并清除 escrow/session。依赖未就绪或事务未执行时保留可重试 completion，不吞产物。
7. 涉及真元的 refund/release 必须通过 `qi_physics::ledger::QiTransfer`；session adapter 不得裸写 `qi_current` 或 zone qi。

### 2.2 生命周期顺序

**断线**：

1. `SessionRegistry` 在 `despawn_disconnected_clients` 及 R3 player save 前接收 disconnect。
2. `Checkpointed`：停止 tick → 写 checkpoint（包含 escrow，但不执行 refund）→ 解绑 runtime `Entity` → 保持 stable owner/设施 claim → `Suspended`。只有恢复窗口结束、管理员结案或恢复校验失败时，才按对应终止原因进入 delivery/refund 结算。
3. `Volatile`：停止 tick → 按非自愿中断结算 escrow/refund → 释放 target/busy → `Terminal`。
4. 完成上述变更后，R3 才保存 player slices，随后 Valence despawn。

**跨维**：

1. 在 `world/dimension_transfer` 写入新 layer/position 前停止接收该 owner 的 session 请求。
2. 对 dimension-scoped session 执行非自愿 teardown 与返还；释放 busy/target。`Checkpointed` session 在跨维时不进入 `Suspended`，而是终止并全退未消费 escrow；`Disconnect`/`Shutdown` 的 checkpoint 保留语义不适用于跨维。
3. teardown 成功后才应用维度转移。失败必须 fail closed，不能让旧维 session 跟随玩家进入新维度。

**关服**：

1. 关闭新 session intake；先结算 `AwaitingDelivery`。
2. `Checkpointed` 调 `on_shutdown` 生成 R3 checkpoint；`Volatile` 在玩家与 inventory 仍可访问时 teardown。
3. lifecycle registry 静止后才执行 R3 `flush_on_shutdown`。R3 在 `plan-refactor-persistence-slices-v1.md` 接入面冻结的 `load(guarded) / autosave / flush_on_shutdown / tick_rebase` 是唯一持久化出口。

**重连**：

1. R3 guarded load 先恢复 checkpoint，再由 registry 以 `PlayerKey` 绑定新 `Entity`。
2. adapter 重验设施/target 存在、owner、维度和版本；通过后恢复为 `Paused` 或 `Running` 并 hydrate client。
3. 恢复失败按 `InvalidRestore` 非自愿结案；有 escrow 时必须返还/交付。busy 冲突时保留 `Suspended` 并 fail closed，不得静默覆盖另一 session。

### 2.2.1 Suspended retention and reclamation

`Checkpointed → Suspended` 是为短期断线/重启恢复服务的有限窗口，不是永久租约。P1 必须冻结并实现：

- 每个 checkpoint 写入 `suspended_at_tick`、`last_rebase_epoch`、owner `PlayerKey`、facility/target claim 和 escrow 摘要；不得只靠数据库行存在判断是否仍可恢复。
- 每个宿主显式选择 `SuspensionPolicy.max_suspended_ticks`；P1 craft、P2 alchemy/forge 使用冻结默认值 `SESSION_SUSPENSION_TTL_TICKS = 1_728_000`，扫描 cadence 为 `SESSION_SUSPENSION_SCAN_INTERVAL_TICKS = 1_200`。R3 `tick_rebase` 保持剩余 TTL 的相对时长，关服时间不得让 lease 永久延长；P3 世界交互不得继承无限值。
- registry 每 tick 或固定 cadence 扫描过期 session；管理员/维护命令可提前结案。扫描、管理员结案、重连与关服 flush 竞争时以 `SessionKey` CAS/registry lock 保证一次结算。
- 过期结案按 `TerminationCause::SuspensionExpired` 执行：停止 tick → 经 R10 `deliver` 返还未消费 escrow/交付已完成产物 → 清理 checkpoint → 释放 facility/target/busy claim → `Terminal`。R10 不可用或 delivery 未提交时保留 `AwaitingDelivery` 和 claim，重试不得重复退款。
- claim 回收必须有审计事件（session key、owner key hash、cause、released claims、delivery result），不记录原 payload；恢复窗口内仍允许 owner 正常 reopen，过期后 reopen 明确拒绝。

P1/P4 必须覆盖：永不重连的 abandoned session 最终释放设施、TTL 边界前后、管理员结案、过期与重连竞态、重复扫描幂等、delivery 失败重试，以及回收后另一玩家可获得同一 furnace/station claim。
### 2.3 durability 决议矩阵

| 状态族 | durability | 断线/关服 | 跨维 |
|---|---|---|---|
| craft | `Checkpointed` | 保存进度、批次与 escrow；重连保持 paused，显式 reopen 后 resume；到 TTL 结算 `SuspensionExpired` | 非自愿 teardown，全退未消费 escrow |
| alchemy furnace/session | `Checkpointed` | furnace + session 原子 checkpoint，不离线推进；到 TTL 结算并回收 facility claim | 非自愿 teardown；退款/产物先落交付事务 |
| forge station/session | `Checkpointed` | station + session + 已扣材料原子 checkpoint；到 TTL 结算并回收 facility claim | 非自愿 teardown；不得遗留 `station.session` |
| gathering | `Volatile` | teardown | teardown |
| lingtian actor sessions | `Volatile` | teardown | teardown |
| mineral | `Volatile` | teardown，解除 ore claim | teardown |
| spiritwood | `Volatile` | teardown，清 session 与 `settling` | teardown |
| external container / TSY search / extract | `Volatile` | 清 owner、进度与 target claim | teardown |
| `PendingInsightOffer` | `Volatile` + deadline | disconnect/timeout 清除 | 清除 |
| `TsyPresence` | R3 checkpointed auxiliary state | R3 Slice 保存 `family_id`、`entered_at_tick`、`return_to` 与版本；guarded restore 通过后才重新 attach component、开放 TSY 请求 | 由 transport 事务显式 enter/exit |

后续某个 volatile 域若改成“起 session 即预扣不可重建资源”，必须先把该域改为 `Checkpointed` 或证明 teardown 可无损返还；不得维持默认易失再补日志。

### 2.4 busy 语义

- `SessionRegistry::try_acquire(BusyClaim)` 是唯一生产入口；各域私有 `has_session` 只能作为迁移期断言，P3 结束时删除。
- busy 至少区分 player-exclusive、target-exclusive、facility-exclusive，并由集中 conflict matrix 判断；不能以“两个不同 Component 可以共存”代表允许并发。
- TSY `Search` 与 `Extract` 必须双向冲突：搜刮中拒绝撤离，撤离中拒绝搜刮；取消/完成/断线/跨维均释放两侧 claim。
- persistent session `Suspended` 时保留其逻辑 facility/escrow claim，避免其他玩家覆盖炉/站状态；runtime `Entity` binding 必须释放。
- R4 的 `GateSpec` 消费 R1 busy 查询 API。距离、维度、所有权检查仍由 R4 实现，R1 不修改 `client_request_handler.rs`。

### 2.5 pause、cancel、refund 与 delivery

- screen close 是 `Pause`，不是 `VoluntaryCancel`。当前 wire 只有 `CraftCancel`（`proto/bong/envelope.proto:257,1309`），P1 必须与 R6/R4 协调显式 open/pause/resume intent；不能只删除 client cancel 后让 server 继续 tick。
- `VoluntaryCancel` 保留域内已公开的经济规则，例如 craft 未完成部分返还 70%；UI 必须有明确取消动作，不能由 Esc、断线、跨维或关服冒充。
- cause/refund matrix 固定如下，adapter 与 contract pin 必须逐项覆盖：
  - `VoluntaryCancel`：按域内公开规则结算；craft 未完成部分返还 70%。
  - `Disconnect`：`Checkpointed` 进入 `Suspended`，不退款，checkpoint 保留未消费 escrow；`Volatile` 全退并终止。
  - `DimensionChange`：dimension-scoped session 全退未消费 escrow，经 delivery 后终止，不进入 `Suspended`。
  - `Shutdown`：`Checkpointed` 进入 `Suspended`，不退款，checkpoint 保留未消费 escrow；`Volatile` 在 inventory 可访问时全退并终止。
  - `InvalidRestore`：全退未消费 escrow；已完成产物只走一次 delivery，不与 inputs 双发。
  - `SuspensionExpired`：全退未消费 escrow/交付已完成产物，delivery 成功后释放 claim 并终止。
- `Disconnect`/`Shutdown` 的 checkpoint 保留与 terminal cause 的 refund 互斥；任何 delivery 未提交时保持 `AwaitingDelivery` 和 claim，禁止清 session 或重复退款。
- refund 也走 R10 delivery 垫层；满包不得退化为日志告警。R10 冻结的 `deliver(items) -> Delivered | Spilled(fallback)` 见 `plan-refactor-inventory-core-v1.md` 接入面。

## 3.1 决议（原开放问题 §N.1）

### 决议 1：不是“全部持久化”或“复制 craft 表”二选一，而是显式 durability + R3 单一出口

- **明确结论**：需要重连/重启恢复的 session 全部注册为 R3 Slice/registry checkpoint；短时可无损结束的 session 显式标 `Volatile`。禁止为每个域复制 craft 私表 + join 自愈代码。
- **实施方案**：craft 作为 P1 adapter，把现有表/hydration 行为收进 R3 暴露的 guarded load/flush API；P2 的 alchemy/forge 用同一 API；其余按 §2.3 teardown。join hydration 由 `on_reconnect` 统一触发。
- **边界与拒绝理由**：全部 session 持久化会把 Entity/短时 target claim 写进 DB，扩大 stale restore 面；全部易失会丢已扣材料和长进度；复制 craft 表会继续制造七套 migration、flush 与 hydration。R1 不实现 `persistence/**`，只消费 R3 API。
- **双锚点**：本 plan §2.2-§2.3；`plan-refactor-persistence-slices-v1.md` 接入面（`load(guarded) / autosave / flush_on_shutdown / tick_rebase`）；现有 join 基准 `server/src/network/craft_emit.rs:884`。

### 决议 2：Checkpointed 暂停保留 escrow；terminal 非自愿中断全退

- **明确结论**：`Checkpointed` 的 `Disconnect`/`Shutdown` 是可恢复暂停，不退款，未消费 escrow 与 checkpoint 一起保留；`DimensionChange`、`InvalidRestore`、`SuspensionExpired` 等 terminal 非自愿原因才全退未消费 escrow。`Volatile` 遇到非自愿中断立即全退并终止。只有玩家明确触发 `VoluntaryCancel` 时，才执行域内既有损耗规则。
- **实施方案**：所有 adapter 以 §2.5 的完整 `TerminationCause` matrix 分支；checkpoint 和 refund 在同一次 transition 中互斥；refund/output 经 R10 delivery，qi 经 ledger。craft 保留 70% 主动取消规则，但关屏只 pause。
- **边界与拒绝理由**：若断线/关服同时 checkpoint escrow 与退款，会在恢复时双发；若一律把断线变 terminal，又会破坏短期恢复。terminal 非自愿原因统一全退，避免网络故障、跨维强制 teardown 或恢复失败施加进度折损；主动取消保留成本，避免套利。
- **双锚点**：本 plan §2.1、§2.5；`plan-craft-close-pause-loss-v1.md` P0-P3；当前先 teardown 后 grant 反例 `server/src/network/client_request_handler.rs:16764-16861`；R10 `plan-refactor-inventory-core-v1.md` 接入面。

## 4. 吸收清单验真（2026-08-03）

| plan 短名 | P0 裁决 | R1 处理 |
|---|---|---|
| craft-close-pause-loss | **真缺陷**：client 关屏发 cancel，server 无 paused gate | P1 首宿主冻结 `Running/Paused` 与显式 cancel；跨端 intent 由 R6/R4 接缝 |
| craft-session-reconnect-lock | **已闭环只归档**：CraftStore 已登记 disconnect clear；join 同发 idle/active session state | 不重复实现，以现有 hydration 作为 `on_reconnect` 基准 |
| placed-container-session-lifecycle-gap | **部分闭环**：断线清锁已有测试；跨维 owner lock 仍缺 | P3 收编 lifecycle/lock teardown；请求距离门归 R4 |
| tsy-container-disconnect-lock-leak | **真缺陷**：search progress/`searched_by` 无统一 disconnect cleanup | P3 收编 volatile target claim |
| tsy-search-extract-concurrent-busy | **真缺陷**：search/extract 只查各自进度 | P3 以集中 conflict matrix 双向互斥 |
| world-transport-tsy-relog-presence | **真缺陷**：位置/维度持久化而 `TsyPresence` 未同事务恢复 | R1 定 transport 生命周期，R3 保存 auxiliary state |
| client-insight-offer-strand | **部分真实**：server pending 只有 chosen/reject 清理，无 deadline；client modal 属 R7 | R1 收编 server timeout/teardown；R7 处理展示 |
| alchemy-furnace-persistence | **真缺陷**：furnace/session 仍为内存权威 | P2 adapter + R3 checkpoint |
| alchemy-takeback-full-inventory-loss | **真缺陷**：先 `end_session` 后 grant | R1 改 teardown/commit 顺序；R10 提供 delivery |
| forge-c2s-session-wiring | **已闭环只归档**：start session 与 blueprint page 已真实分发 | 不重复实现 |
| bot-handcraft-craft-outcome-timeout | **旧证据不足，不形成 R1 owner**：报告来自脏 debug server；当前已有 `scripts/bot/scenarios/production_handcraft_stone_knife.py` | P4 仍跑 clean-main craft bot，失败再以新证据立 owner |
| forge-outcome-full-inventory-loss | **真缺陷、R10 主责**：满包只有 `grant skipped` | R1 只提供 AwaitingDelivery→CommitTerminal 合同；R10 实现 fallback |
| lingtian-session-disconnect-server | **真缺陷**：Resource 持续 tick，未按 actor 断线清理 | P2 迁移六类 actor session |

### 4.1 覆盖审计差分

按总纲 §6 要求枚举 active/skeleton 的 session、disconnect、busy、container、full-inventory 与 dimension-gate 候选后：

- `forge-session-range-dimension-gate` 已由 `plan-refactor-c2s-gate-v1.md` 吸收清单明确登记为在飞项；实现落在 R4 的 `GateSpec`/`client_request_handler.rs`，不追加 R1 owner。R1 的 `on_dimension_change` 与 busy API 仅供 R4 查询。
- `alchemy-furnace-scope-gate` 同样已由 R4 吸收；R1 负责炉 session 生命周期，不复制距离/维度 gate。
- `forge-session-enum-unstripped` 是 R6/client bridge 契约修复，不是 server session 生命周期。
- `tsy-extract-disconnect-stale`、`woliu-vortex-disconnect-residue`、`niche-guardian-cross-session-leak` 及其他 client `*Store` 残留归 R2/R7，不进入 R1 文件域。
- 本轮未发现应新增到 R1 权威吸收清单、但尚无 owner 的 plan。

## 5. 文件所有权与接缝

- **R1 独占**：`server/src/session/`、七域 `session.rs`、`network/craft_emit.rs` 的 session tick 区，以及迁移时删除的域内私有生命周期代码。
- **R3 独占**：`server/src/persistence/**`、player load/autosave/shutdown flush；R1 仅消费 checkpoint/restore/flush hook。
- **R4 独占**：`server/src/network/client_request_handler.rs` 与 `network/gate/`；R1 仅暴露 busy/session query。
- **R6 独占**：proto/S2C bridge 与跨端 pause/resume 契约变更。
- **R7/R2 独占**：client Screen/HUD 与 Store disconnect 清理。
- **R10 独占**：`server/src/inventory/**` 与 `InventoryTxn::deliver`；R1 只决定何时允许 commit teardown。
- **阶段放行矩阵**：
  - framework-only：R3 P1 合入后可落 `InteractionSession`、registry、lifecycle 与不触达生产 wire/delivery 的 contract pins。
  - craft producer path：R6 P1 交付 `CraftOpen`/`CraftPause`/`CraftResume` proto/schema/bridge → R4 P1 交付 production decode/dispatch 与 owner/phase/busy gate → R7 P2 交付 close/pause、显式 cancel、reopen/resume UI producer/consumer（R2 P1 已登记的 `CraftStore` 继续提供 disconnect lifecycle）→ R10 P1 冻结 `deliver` commit contract，且 R10 P2 把 craft production 调用点迁入 `InventoryTxn::deliver`；全部合入后 R1 才能启用并验收 craft adapter。
  - alchemy/forge：还须 R10 P2 将两域 production delivery 调用点迁入 `InventoryTxn::deliver`，否则只能停在 `AwaitingDelivery`。
  - `TsyPresence`：R3 P1 注册 auxiliary Slice，且 R3 P4 restore parity 常绿后，R1 P3 才能 attach 新 runtime `Entity` 并开放 TSY 请求。
- 禁止用 mock、registry 单测或临时持久层越过上述门宣称端到端完成；依赖未齐时 phase 保持未完成。

## 6. 后续阶段交付物

### P1 — 框架 + craft 首宿主

- 新增 `server/src/session/{mod.rs,registry.rs,lifecycle.rs}`，包含 §2 全部 symbol；framework-only 可在 R3 P1 后落地，但不启用生产 craft adapter。
- R6 `CraftOpen`/`CraftPause`/`CraftResume` 契约、R4 production handler/gate、R7 P2 Craft Screen producer/consumer（消费 R2 P1 已登记的 `CraftStore`）、R10 P1 `deliver` contract 与 R10 P2 craft production delivery 全部合入后，craft 才迁移到 `SessionRegistry`：关闭 screen pause、显式 cancel、重开 resume，现有 recipe/session join hydration 行为不变。
- contract pins：五态转换、stable owner 重绑、disconnect-before-save、dimension-before-transfer、busy 冲突、delivery commit gate，以及 §2.5 六个 `TerminationCause` 的逐变体正反测试；明确断言 `Disconnect`/`Shutdown` checkpoint 不退款，terminal refund 不保留可恢复 escrow。
- `SuspensionPolicy` pins：TTL 前后、永不重连、管理员结案、过期/重连/关服竞态、重复扫描幂等、delivery 失败重试与 claim 回收后另一玩家可获取同一设施。

### P2 — alchemy / forge / lingtian

- R3 checkpoint API 与 R10 P2 production delivery 都就绪后，alchemy furnace/session 与 forge station/session 才原子 checkpoint；修复先 teardown 后 delivery，失败保持 `AwaitingDelivery` 与 claim。
- 灵田六类 `ActiveSession` 共用 volatile adapter；断线/跨维/关服不再 tick 或结算离线 actor。
- qi refund/release 测试从 `SPIRIT_QI_TOTAL` 与 ledger 不变量取值，不写新物理常数。

### P3 — gathering / mineral / spiritwood / 世界交互

- 删除三域私有 store 生命周期分支，迁入 registry。
- external container、TSY search/extract 使用 target claim；所有终态释放 busy。
- R3 `TsyPresence` auxiliary Slice 的 guarded load/autosave/shutdown flush/tick-rebase 与 restore parity 常绿后，才把 presence 与 player position/dimension 对拍；新 `Entity` 只在身份/版本/维度校验通过后 attach，随后开放 TSY 请求，不再出现“人在 TSY、presence 不在”。

### P4 — bot e2e + 归档

加入并常绿：

1. `session_disconnect_cleanup`
2. `session_dimension_transfer`
3. `session_restart_recovery`
4. `session_busy_mutex`
5. `session_full_inventory_delivery`
6. `session_suspension_reclamation`：永不重连、TTL 边界、管理员结案、过期与重连竞态、重复扫描和 delivery 失败重试均不泄漏或双发 claim/escrow。
7. `session_termination_cause_matrix`：逐项命中 `VoluntaryCancel`、`Disconnect`、`DimensionChange`、`Shutdown`、`InvalidRestore`、`SuspensionExpired`，对拍 checkpoint/refund 互斥与 inputs/output 不双发。
8. `session_craft_pause_resume_wire`：真实 client producer → R6 proto/bridge → R4 production handler/gate → R1 registry → S2C hydrate → client reopen consumer 全链路。
9. `session_tsy_presence_relog`：R3 guarded presence Slice 与 position/dimension 同事务恢复，校验失败不 attach、不开放 TSY 请求。

另回归现有 `production_craft_disconnect_resume.py`、`production_craft_cancel_full_inventory_refund.py`、`production_handcraft_stone_knife.py`。对应 implementation PR 合入后，按总纲 §7 为吸收项补 Finish Evidence 并做每轨一次 docs-only 批量归档。
