# plan-voidaction-target-zone-lock-v1（骨架）

> **骨架（草案）**。一句话主题：`client/cultivation/voidaction` 的目标 zone 状态从未接线，`VoidActionStore` 永远落回默认 `"spawn"`；结果是 `suppress_tsy` 恒打到非 TSY 区而被 server 拒绝，`explode_zone` / `barrier` 也会稳定作用到错误区域。更糟的是 client 发送后立即本地起长冷却、却没有任何 response/state 回写校正，导致**化虚玩家第一次误触就可能在整场会话里把对应 action 自锁 7/30/90 天**。

> 立项目机：这是 `plan-void-actions-v1` 已落地主链里的高频交互缺口，不是边角兼容问题。O 键打开的化虚行事屏是正式玩家入口，三招世界级 action（镇压坍缩渊 / 引爆区域 / 化虚障）全部依赖 `zone_id`；但 client 侧既没有当前 zone 自动写入，也没有手动选区 UI，也没有后续 server 回执修正，属于**玩家可直接踩中的主链 bug**。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 化虚行事 target zone 锁死 `spawn` + 本地伪冷却 | plan_skeleton | ⬜ |

## P0 — 化虚行事 target zone 锁死 `spawn` + 本地伪冷却

- **现象**：`client/src/main/java/com/bong/client/cultivation/voidaction/VoidActionStore.java` 把 `DEFAULT_ZONE_ID` 写死为 `"spawn"`；全 client 侧唯一写 target 的 API 是 `setTargetZone(String)`，但全仓 grep 仅定义、**零调用点**。`VoidActionScreen` 渲染时始终显示 `目标: ` + `snapshot.targetZoneId()`，派发 `SUPPRESS_TSY` / `EXPLODE_ZONE` / `BARRIER` 时也都直接把这个值塞进请求；屏幕本身没有任何 target zone 编辑控件。
- **server 为什么一定会误执行/拒绝**：
  - `server/src/cultivation/void/actions.rs::cast_suppress_tsy` 先按 `zone_id` 在 `ZoneRegistry` 里精确找 zone，再要求 `zone.tsy_family_id().ok_or(TargetNotTsy)`；`server/src/world/zone.rs::tsy_family_id()` 只对 `tsy_*_(shallow|mid|deep)` 命名返回 family id。
  - `server/zones.json` 的 `spawn` 是普通 danger=1 新手区，不是 TSY zone，也没有 `tsy_*` 命名。于是 `SuppressTsy("spawn")` 会稳定走到 `TargetNotTsy` reject。
  - `cast_explode_zone` / `cast_barrier` 同样按传入 `zone_id` 落账、记日志、写 public_text；因此它们不会“自动改成当前区”，而是会**真的把 spawn 当目标区**。
- **为什么这不是“默认值占位但会被别处覆盖”**：`setTargetZone(...)` 无调用方；`VoidActionStore.replace(...)` 也没有任何 network handler / bootstrap 使用。`server/src/schema/void_actions.rs` 虽然定义了 `VoidActionResponseV1` / `VoidActionStateV1`，`docs/finished_plans/plan-void-actions-v1.md §3` 也明确写了 `VoidActionHandler.java — VoidActionRequest 发包 + Response 接收`，但现状是 **client/server 两侧都没有任何 void action response/state 实际收发链**，不存在隐藏同步源。
- **连带后果（同根，不单独立题）**：`VoidActionHandler` 在 `ClientRequestSender.sendVoidAction*` 之后立即 `VoidActionStore.markDispatched(kind, nowTick)`，本地直接写入 7/30/90 天冷却；而 server 失败路径只 `tracing::warn!`，没有回执纠正 client。于是化虚玩家第一次在默认 target=`spawn` 下点 `镇压坍缩渊`，即便 server 完全拒绝，client 仍会把按钮显示成长冷却，直到重登/清状态前都不能再试。
- **对实际游玩体验的影响**：
  - `镇压坍缩渊` 在正式 UI 上**事实上不可用**：默认目标恒为非 TSY 的 `spawn`，一次点击就 server reject，玩家只看到屏幕关掉、动作没发生、自己按钮却进入 30 天冷却。
  - `引爆区域` / `化虚障` 会稳定打到错误 zone：玩家站在别的区域释放，日志、公屏文案、ledger 账户和 barrier 归属仍写 `spawn`；实际博弈对象与施法地点割裂，世界级 action 失去战术意义。
  - 因为这是化虚主通道，不是调试命令，受影响的是**所有首次接触该 UI 的化虚玩家**，不是边缘脚本调用者。
- **建议修复范围 / 模块**：优先收口 `client/.../voidaction/VoidActionStore.java`、`VoidActionScreen.java`、`VoidActionHandler.java`、必要时补 `client/network/*` 与 `server/schema/void_actions.rs` / `server/network/*`。至少要一次性补齐两条：
  - ① **target 来源**：进入屏幕时自动解析玩家当前 zone，或提供显式 zone picker / inspect target，不能继续依赖永不更新的默认 `"spawn"`。
  - ② **accepted 后再上冷却**：要么接通 `VoidActionResponseV1` / `VoidActionStateV1`，要么用等价回执链，保证 reject 不写本地冷却、server 才是最终真相。
- **验收抓手**：至少补 5 组 pin。1) 打开 `VoidActionScreen` 时 target zone 与玩家所在 zone 一致，非空且不回落 `spawn`。2) `SuppressTsy` 对非 TSY 当前区明确拒绝且**不写 client 冷却**。3) 在 TSY 当前区成功施放 `SuppressTsy` 后，client/server 冷却一致。4) `ExplodeZone` / `Barrier` 的日志 target、ledger 账户、public_text 与玩家真实选区一致，不再固定 `spawn`。5) 断线重连或二次开屏时，client 状态能从 server 真值恢复，不保留误触伪冷却。

## 反方裁决摘要

1. **Round 1 反方**：怀疑“也许有隐藏写入者在别处调用 `setTargetZone` / `replace`”。裁决：全 client grep 仅命中 `VoidActionStore` 自身与读侧 UI；没有任何 network handler、bootstrap、screen lifecycle 或 state bridge 写入 target/cooldown，怀疑不成立。
2. **Round 2 反方**：怀疑“也许默认 `spawn` 是设计，化虚行事只允许对 spawn 生效”。裁决：server `cast_suppress_tsy` 明确要求目标 zone 具备 `tsy_family_id()`，而 `spawn` 按 `server/zones.json` 与 `world/zone.rs` 规则根本不是 TSY；若这是设计，则正式 UI 的第一招被设计成必失败，与 `plan-void-actions-v1` 对三类 zone action 的目标语义自相矛盾。再加上 `VoidActionResponseV1` / `VoidActionStateV1` 全量死类型、client 无回执校正，这更像半接线而非刻意产品决策。

## 开放问题

1. target zone 应该走“当前所在 zone 自动填充”还是“显式选择器/准星选区”？前者修得快，后者更适合跨区博弈，但需要额外 UI 与合法性反馈。
2. 回执链是补真正的 `VoidActionResponseV1` / `VoidActionStateV1`，还是复用现有 event/hint payload 做最小纠偏？需要在修复 PR 中定成唯一真相源，避免再次出现 client 先写假状态。

## 审计来源

bughunt 线程 W（限定 `alchemy/cultivation/voidaction` 主路径，人工主代理复核）。候选先由主代理定位到 `VoidActionStore.DEFAULT_ZONE_ID="spawn"` 与 `setTargetZone` 零调用，再补读 `server/src/cultivation/void/actions.rs`、`server/src/world/zone.rs`、`server/zones.json`、`docs/finished_plans/plan-void-actions-v1.md §3` 做两轮反方证伪。结论为 **report-only**：先以 skeleton-only PR 固化玩家影响、根因链、反方裁决与验收抓手，后续再单独修 client/server 接线。
