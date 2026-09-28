# plan-bughunt-anqi-multishot-hud-bound-v1

> 来源 Issue：#1360。multishot HUD 把 wire 的宽泛 echo_count 直接当作要创建的方块数，客户端没有显示域资源上限。
>
> 阶段总览：P0 ⬜ 分离 wire domain 与 HUD display domain；P1 ⬜ 锁定 server event、schema 和客户端资源上限。

## §0 摘要

生产 MultiShotEvent 的 projectile_count 是 u8，但 server AnqiHudV1、agent TypeBox 和 client AnqiHudServerDataHandler 对 echo_count 的上限是 i32::MAX。multishot kind 复用 echo_count 字段，handler:99-102 直接 intValue 后写入状态，AnqiHudPlanner:64-75 按 count 逐发创建 HudRenderCommand。合法事件通常只有 255 发，但 wire 或异常 payload 可以携带 2,147,483,647，导致整数溢出、超大布局或 OOM/假死。

## §1 实际游玩体验影响

- 恶意、损坏或未来错误发送的 multishot payload 能让 client 在渲染前分配海量命令，阻塞主线程。
- 即使正常 server event 不超过 u8，当前 schema 没有表达 multishot 的窄域，客户端也没有 fail-closed 边界；echo 维度与 multishot 维度被错误共用。

## §2 复现路径

1. 构造 kind=multishot、echo_count=2147483647 的 anqi_hud payload（仓内 sample 已覆盖该 wire 上限）。
2. AnqiHudServerDataHandler:66-73 通过 MAX_ECHO_COUNT 接受该值，:99-102 写入 multiShotCount。
3. AnqiHudPlanner:64-75 计算 total 并循环 count 次生成命令，观察主线程长时间占用或整数布局异常。
4. 对照 server MultiShotEvent.projectile_count=u8，确认生产事件域与 wire/display 域没有被单独验证。

## §3 根因证据

- server/src/combat/anqi_v2.rs:197-204 的 MultiShotEvent 使用 projectile_count: u8。
- server/src/network/anqi_hud_emit.rs:185-201 将 projectile_count 转为 u32 放入复用的 echo_count 字段，未携带 multishot 专属上限。
- server/src/schema/server_data.rs:66-71 定义 ANQI_HUD_ECHO_COUNT_MAX=i32::MAX；:1104-1114 的 AnqiHudV1 对 echo_count 使用该通用上限。
- agent/packages/schema/src/server-data.ts:178-194 同样把 echo_count 上限设为 2,147,483,647；server-data.anqi-hud.multishot.sample.json 直接使用 2147483647。
- client/src/main/java/com/bong/client/combat/handler/AnqiHudServerDataHandler.java:43-47、:66-73 接受 MAX_ECHO_COUNT；:99-102 不区分 kind 地写入 multiShotCount。
- client/src/main/java/com/bong/client/hud/AnqiHudPlanner.java:64-75 以 count 计算总宽度并逐发循环，没有资源或屏幕上限。

## §4 非重复比对

- echo kind 的宽上限可能有自己的叙事用途；本 plan 不把所有 echo_count 全局改成 255，而是处理 multishot 复用字段造成的条件域丢失。
- 暗器战斗本身的 projectile_count 生成和真元消耗不是本 issue 的根因；server 已有 MultiShotEvent，问题在 server-data 到 HUD 的边界。

### 立项检查记录（2026-09-28）

- docs/worldview.md：检索“暗器、多发、齐射、弹数、资源、HUD、战斗反馈”，核对器修/暗器流与战斗可见性章节；没有允许客户端按无界输入创建 UI 对象的规则。
- docs/finished_plans/：检索 MultiShotEvent、AnqiHudV1、AnqiHudPlanner、multishot；命中 combat skill feedback 基础文档，未发现 wire/display bound 的修复。
- active plan（docs/plan-*.md）：检索 echo_count、projectile_count、AnqiHudServerDataHandler、multishot；未发现 active plan 负责该条件上限。
- docs/plans-skeleton/：检索 anqi_hud、multishot、echo_count 上限、HUD OOM；未发现同主题骨架。
- docs/plans-skeleton/reminder.md：检索暗器 multishot、echo_count、HUD 上限；仓内无该主题延后事项。

## 接入面与跨仓契约

- **Inputs**：MultiShotEvent.projectile_count、AnqiHudV1 kind/echo_count、agent TypeBox validator、client handler 和 HUD state。
- **Outputs**：multishot 在 wire 验证和 display 规划阶段都有有限上限；超界 payload fail closed，不创建海量命令。
- **共享类型 / 事件**：MultiShotEvent、AnqiHudV1、AnqiHudKindV1::Multishot、ServerDataPayloadV1::AnqiHud、anqi_hud、AnqiHudState.multiShotCount。
- **server 契约符号**：MultiShotEvent.projectile_count、emit_anqi_hud_payloads、AnqiHudV1::echo_count、ANQI_HUD_ECHO_COUNT_MAX。
- **agent**：有变更可能性；当前 TypeBox 只能表达 echo_count 的通用上限，若引入 multishot 专属字段/union，必须同步 server-data.ts、schema registry 和 sample；若只在 handler 做 kind-specific fail-closed，则 agent 无变更并写明它仍校验通用 envelope。
- **client**：有变更；AnqiHudServerDataHandler 按 kind 校验、AnqiHudState.multiShotCount、AnqiHudPlanner.appendMultiShot。
- **worldview 锚点**：docs/worldview.md 的器修/暗器流和战斗反馈章节；显示上限是资源安全边界，不改变实际 projectile 数或真元消耗。
- **qi_physics**：HUD 只读 projectile_count，不重算真元；任何 payload 拒绝不能补扣或补发 qi。

## §5 修复骨架

### P0 分离 multishot wire/display 上限

- 至少在 client handler 对 kind=multishot 使用不超过生产 projectile_count 类型的窄上限（u8::MAX），并在 AnqiHudPlanner 前再做独立 display cap；越界直接 no-op/记录，不进入 intValue 循环。
- 更完整的方案是把 multishot_count 从 echo_count 拆为带明确上限的 wire 字段，并同步 Rust schema、agent TypeBox、protobuf/sample 与 client parser；echo kind 保留原有上限，避免破坏其他契约。
- total 宽度计算使用有界整数或固定布局，不能因 count 乘法溢出；显示 cap 不得反向修改 server 的 projectile_count。

### P1 回归与验收

- 0、1、u8 上限、display cap、i32 max 和负数/非整数 payload 分别验证：合法值可显示，越界值 fail closed，主线程命令数量始终有界。
- server 生产 MultiShotEvent 到 anqi_hud 的正常样例仍能渲染；echo/aim/charge/abrasion 四种 kind 的既有上限和独立 expiry 不受影响。
