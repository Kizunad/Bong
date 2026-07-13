# plan-bughunt-craft-outcome-network-thread-sound-v1

> **Active BugFix plan**。来源：`docs/plans-skeleton/plan-bughunt-craft-outcome-network-thread-sound-v1.md`；升格日期：2026-07-13。主题：验证并修复 `craft_outcome` 完成反馈可能在 Fabric network thread 触达 screen / player / sound state 的线程契约违规。

## 阶段总览

| 阶段 | 主题 | 状态 | 验收日期 |
|------|------|------|----------|
| P0 | 第一性原理证真：闭合生产可达路径与失败复现 | ⏳ | — |
| P1 | 最小线程边界修复与饱和回归 | ⬜ | — |
| P2 | JDK 17 完整门禁、主线同步与三轮独立验证 | ⬜ | — |

## Bug 摘要

候选问题是：Fabric raw `ClientPlayNetworking.registerGlobalReceiver(Identifier, PlayChannelHandler)` 的 receiver 运行在 network thread，但 `BongNetworkHandler.registerServerDataChannel()` 在该 callback 内同步执行 `ROUTER.route(...)`。`ServerDataRouter.route(...)` 又同步调用 `CraftOutcomeHandler.handle(...)`，后者写入 `CraftStore` 并同步通知当前 screen 的 outcome listener。

当 payload 为 `type=craft_outcome`、`kind=completed` 时，`CraftScreen` / `WorkbenchScreen` listener 会写 `flashTicks` 并调用 `client.player.playSound(...)`；如果上述调用链确实未切回 client thread，即违反 Fabric 对 game state / screen / sound 访问的线程契约。

本 plan 先假设报告可能错误：必须核验 raw API 的真实线程语义、生产 server emit、router 注册、store listener 生命周期、现有 `client.execute(...)` 防护，以及同根因修复是否已在最新主线覆盖。只有证真后才允许修改代码。

## 范围与决策

- **主范围**：`bong:server_data` raw receiver 到 `ServerDataRouter` handler side effect 的线程边界，以及 `craft_outcome` completed / failed 对 `CraftStore`、`CraftScreen`、`WorkbenchScreen` 的可达链。
- **最小修复决策**：receiver 在 network thread 只复制/解码网络 buffer；`ROUTER.route(...)`、store/listener side effect 与 `applyDispatch(...)` 必须在同一个 `client.execute(...)` client-thread task 内有序执行。不得只把最终 `applyDispatch(...)` 包进主线程而让 handler 先跨线程落地。
- **局部兜底边界**：只有证据证明 `CraftOutcomeHandler.handle(...)` 还有绕过该 receiver 的生产异步调用方时，才给 craft listener 增加独立调度；否则不叠加双重 `client.execute(...)`，避免反馈顺序和测试语义复杂化。
- **排重**：不单独修 #1016 的 `cast_sync -> SkillConfigPanelManager` UI 生命周期；但若主 receiver 边界修复自然覆盖它，必须增加回归证明该 handler 不再在 network thread 执行。
- **禁止扩项**：不改制作数值、配方、server craft 状态机、声音资产、UI 视觉规格或协议 schema；不做全量 handler 重构。
- **玩家可感知规格**：保留既有完成音效、`flashTicks = 6` 闪光和输出预览刷新；本修复只保证三者在 client thread 按原顺序稳定发生，不新增粒子、动画、HUD、narration 或新资产。
- **worldview 锚点**：制作属于 §十资源与匮乏的产出/加工闭环；本 plan 不改变资源、真元或经济规则，不触碰 `qi_physics` ledger。

## P0：第一性原理证真

- 核验当前 Fabric 1.20.1 / Fabric API sources 对 raw receiver 的 network-thread 契约。
- 闭合正常玩家路径：server craft completion emit → `bong:server_data` → receiver → router → `CraftOutcomeHandler` → `CraftStore` → 当前 `CraftScreen` / `WorkbenchScreen` listener。
- 检查现有防护：payload copy/decode、`client.execute(...)` 包围范围、screen refresh scheduling、session/disconnect 清理、listener 注销与重复 payload 语义。
- 先增加修复前可失败的线程契约测试：从模拟 network thread 触发 receiver/提取出的调度边界，证明 router/store/listener 不得在调用线程同步执行。
- 若候选已被主线覆盖或生产不可达，则转 `NOT_BUG`：只写入反证、`file:line` 与测试结果，不造空修复。

## P1：最小修复与饱和回归

- 将 raw receiver 的 handler side effect 统一排入 client executor，并保持单 payload 内 `route → applyDispatch` 顺序。
- 覆盖 `CraftScreen` 与 `WorkbenchScreen` 的 completed outcome：完成音效、闪光、输出预览刷新只在 client thread 执行且各发生一次。
- 覆盖 failed outcome、未知 payload、route 返回空 dispatch、handler 抛错/坏 JSON、连续 payload 顺序与断线/无 player 边界，防止调度后吞包、重复反馈或延迟异常。
- 回归 `cast_sync` 同根因入口：handler side effect 只在 client executor 中发生；不改变其业务语义。
- 测试断言外部可观察契约与线程身份，不绑定私有实现调用次数；失败信息必须带实际线程/队列/事件值。

## P2：闭环验证

- 首次修复 HEAD：工作区干净后由全新无上下文 read-only validator 对抗检查真伪、可达性、主线程边界、顺序、回归与测试饱和度。
- 完整 client 门禁：JDK 17 下 `cd client && ./gradlew test build`。
- `git fetch origin` 后按 merge-base 分类同步最新主线；任何 HEAD 变化均重跑 client 完整门禁并启动新的 `REBASE_VALIDATING` validator。
- 填写 `## Finish Evidence`、受控归档并提交后，对最终归档 HEAD 再启动新的 `FINAL_VALIDATING` validator；最终 PASS 后禁止再修改分支。

## 验收测试矩阵

| 场景 | 预期 |
|------|------|
| network thread 收到 completed | receiver 返回前 router/store/screen/sound 均未执行；client executor drain 后按序执行一次 |
| network thread 收到 failed | 失败 outcome 只在 client thread 写 store/通知 listener，不播放完成音 |
| CraftScreen completed | `flashTicks`、完成音效、刷新均在 client thread，顺序稳定且不重复 |
| WorkbenchScreen completed | 同上，且不影响 workbench session / output preview |
| `cast_sync` | 配置窗相关 handler side effect 不在 network thread 执行 |
| 空/未知/坏 payload | 不越过既有错误处理边界，不触发 craft side effect，不破坏后续合法 payload |
| 连续两个合法 payload | client executor 保持提交顺序，各 payload 恰好应用一次 |
| 无 player / screen 已关闭 | 不崩溃、不播放伪完成音，store 与 listener 生命周期遵循既有契约 |

## 风险

- 把 route 移入 client executor 会把 handler 内 JSON 解析和所有 store 更新一起移到 client thread；需用现有 payload 尺寸与测试确认不会引入可感知卡顿，不顺手重构解析层。
- 旧测试可能默认 `ROUTER.route(...)` 在 receiver 返回前同步生效；必须改成显式 drain executor，而不是放宽断言。
- 双重调度会改变反馈顺序并扩大 race surface，因此局部 listener 兜底必须以额外生产调用方证据为前提。
- 线程修复不能吞掉 route exception、未知 payload 诊断或既有 dispatch；错误路径需保持可观测。
