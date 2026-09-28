# plan-bughunt-resonance-lock-meter-tick-domain-v1

> 来源 Issue：#1821。共振锁定 HUD 把 epoch 毫秒除以 50 当作当前 tick，却与 server 的 started_at/ends_at tick 没有共同时间原点。
>
> 阶段总览：P0 ⬜ 建立 server tick 到 client tick 的明确换算；P1 ⬜ 验证倒计时、闪烁和结束事件。

## §0 摘要

BongHud.renderSurface 在每帧使用 System.currentTimeMillis()/50L 生成 estimatedTick。ResonanceLockHudStateStore 的 startedAtTick/endsAtTick 来自 server 事件；ResonanceLockMeterHud 直接用 estimatedTick 计算 remainingTicks/progress。两者都叫 tick，却没有 epoch 对齐，导致倒计时可能瞬间结束、负数归零或进度跳变。

## §1 实际游玩体验影响

- 玩家刚进入共振锁定就看到 0 tick 或临近到期闪烁，无法判断弹反窗口是否仍在。
- 服务器发送 resonance_lock_end 后 HUD 虽会清空，但在结束前的本地进度不能作为可靠的战斗提示。

## §2 复现路径

1. server 以 ResonanceLockEvent.started_at/ends_at（服务器运行 tick）发送 bong:resonance_lock。
2. client ResonanceLockHandler:45-56 写入 ResonanceLockHudStateStore。
3. 渲染帧用 BongHud:210 的 epochMillis/50L 作为 currentTick，传入 ResonanceLockMeterHud:45-46。
4. 因 server tick 通常从进程启动计数而不是 Unix epoch 计数，remainingTicks 与 progress 立即偏离真实锁定窗口。

## §3 根因证据

- client/src/main/java/com/bong/client/BongHud.java:206-218 以 System.currentTimeMillis()/50L 构造 estimatedTick，并把它传给共振 HUD。
- client/src/main/java/com/bong/client/combat/baomai/v4/ResonanceLockMeterHud.java:29-46 把 currentTick 注释为“客户端本地”却直接与 server state 的 endsAtTick 相减。
- client/src/main/java/com/bong/client/combat/baomai/v4/ResonanceLockHudStateStore.java:52-87 的 State 只保存 server tick，未保存 client 接收锚点或时钟偏移。
- server/src/network/baomai_v4_event_bridge.rs:219-267 从 ResonanceLockEvent 发 started_at/ends_at 到 Redis 与 bong:resonance_lock；server/src/schema/baomai_v4.rs:198-223、agent/packages/schema/src/baomai-v4.ts:135-149 将两字段定义为非负整数。
- client/src/main/java/com/bong/client/combat/baomai/v4/ResonanceLockHandler.java:45-56 直接把 wire 数值写入 store；:67-74 收到结束事件才清空。

## §4 非重复比对

- baomai v4 的 lock/end 事件、partner identity 和 VFX 处理已有契约；本 plan 只处理 HUD 时钟，不改变锁定判定或 combat event。
- 其他 HUD 的毫秒倒计时不能证明 server tick 可以与 epoch tick 直接相减；不得复制该错误。

### 立项检查记录（2026-09-28）

- docs/worldview.md：检索“共振、锁定、弹反、战斗窗口、tick、经脉”，核对战斗/经脉可见性章节；未找到客户端时钟实现约定。
- docs/finished_plans/：检索 ResonanceLockMeterHud、BaomaiV4ResonanceLockV1、started_at、ends_at；命中 baomai/技能反馈基础文档，未发现 tick 原点修复。
- active plan（docs/plan-*.md）：检索 resonance_lock、estimatedTick、currentTick；未发现 active plan 负责该换算。
- docs/plans-skeleton/：检索 resonance_lock、tick domain、started_at/ends_at；未发现同主题骨架。
- docs/plans-skeleton/reminder.md：检索共振锁定、HUD tick、时间原点；仓内无该主题延后事项。

## 接入面与跨仓契约

- **Inputs**：server ResonanceLockEvent/EndEvent 的 started_at、ends_at、fighter IDs，client 本地 client tick 时钟和接收时刻。
- **Outputs**：ResonanceLockHudStateStore 的本地 deadline、ResonanceLockMeterHud 的 remaining/progress/闪烁；结束事件仍清空状态。
- **共享类型 / 事件**：ResonanceLockEvent、ResonanceLockEndEvent、BaomaiV4ResonanceLockV1、bong:resonance_lock、bong:resonance_lock_end、ResonanceLockHudStateStore.State。
- **server 契约符号**：emit_resonance_lock_payloads、BaomaiV4ResonanceLockV1::new、started_at、ends_at。
- **agent**：无变更；agent/packages/schema/src/baomai-v4.ts 已镜像这两个字段，进度换算是 client presentation 问题。
- **client**：有变更；BongHud 的时间来源、ResonanceLockHandler 的接收锚点、ResonanceLockHudStateStore/ResonanceLockMeterHud 的计算。
- **worldview 锚点**：docs/worldview.md 中战斗窗口和经脉/弹反的可感知反馈章节；不新增境界或战斗规则。
- **qi_physics**：无真元计算；HUD 只能展示事件，不得因 tick 修复重算或修改真元。

## §5 修复骨架

### P0 统一时钟原点

- 建立明确的 client tick clock（随 client tick 递增），在收到 lock payload 时把 server 的持续区间 ends_at-started_at 转换为本地 deadline；渲染只比较同一 client 时钟，禁止再使用 epochMillis/50L 冒充 server tick。
- 若需要补偿网络延迟，扩展 payload 提供可核验的 server current tick/发送时刻，并同步 Rust schema、agent TypeBox 与 client parser；不能靠无依据的常量偏移。
- 保留 started_at/ends_at 的非负校验，结束事件仍是权威清理。

### P1 回归与验收

- 刚收到 lock、过半、到期前 10 tick 三个锚点的 remaining/progress 与定义的区间一致，闪烁只在同一单位下触发。
- client tick 暂停/恢复、丢包后收到 end、重复 lock payload 都不会把 HUD 置于负数或旧会话。
