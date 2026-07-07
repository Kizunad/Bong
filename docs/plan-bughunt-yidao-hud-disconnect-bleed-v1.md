# plan-bughunt-yidao-hud-disconnect-bleed-v1

> Skeleton Plan。主题：`YidaoHudStateStore` 是客户端静态进程态，但断线 / 换服清理路径没有重置它；玩家上一会话的医道面板会继续被 `BongHudOrchestrator` 渲染，直到新会话收到新的 `yidao_hud_state` 覆盖。

## 一句话 bug

医道 HUD 只依赖 server payload 或测试态 reset 清空，断线 / 换服时没有生产态清理；旧的“医道 续命 / 患者 N / 业力 / 结契”等面板可串到下一次连接。

## 实际游玩体验影响

- 玩家在旧服 / 旧存档作为医者，或刚经历医道技能、患者合同、业力变化后断线，立刻进入新服时，右侧医道面板可能继续显示上一局的医道状态。
- 如果新服当前玩家没有医道组件或医道状态，服务端很可能不会主动发 `yidao_hud_state` baseline empty；旧面板就不只是首帧闪烁，而可能持续到下一次相关 payload 或客户端进程结束。
- 结果是跨服身份与战斗辅助信息串场：玩家会看到不存在的患者数、旧业力、旧结契数或旧 active skill，误导医道支援、战斗判断和 UI 信任。

## 复现路径

1. 在一次连接中让客户端收到 active `yidao_hud_state`，例如含 `active_skill="life_extension"`、`patient_ids`、`karma` 或 `contract_count` 的 payload。
2. 确认 `YidaoHudPlanner` 产生命令，右侧出现医道面板。
3. 断线或切换到另一个不下发医道 HUD baseline 的服务器 / 存档。
4. 观察结果：`YidaoHudStateStore.snapshot()` 仍是上一会话 snapshot，`BongHudOrchestrator` 每帧继续把它交给 `YidaoHudPlanner`，旧医道面板可继续显示。

## 根因证据

- `client/src/main/java/com/bong/client/yidao/YidaoHudStateStore.java:47-61` 只有 `snapshot()` / `replace()` / `resetForTests()`，没有生产态 `clearOnDisconnect()`。
- `client/src/main/java/com/bong/client/hud/BongHudOrchestrator.java:362-367` 每帧直接读取 `YidaoHudStateStore.snapshot()` 并调用 `YidaoHudPlanner.buildCommands(...)`，没有连接状态门禁。
- `client/src/main/java/com/bong/client/yidao/YidaoHudStateStore.java:33-44` 的 `active()` 只要 `activeSkill`、患者、信誉、平和、业力、污染、断脉、结契、群体预览任一非空就为真；旧 snapshot 因此能继续显示。
- `client/src/main/java/com/bong/client/hud/YidaoHudPlanner.java:28-31` 的隐藏条件只是 `!safe.active()`，不会自行判断该 snapshot 属于旧连接。
- `client/src/main/java/com/bong/client/BongNetworkHandler.java:857-900` 的全局断线清理列表重置了大量 static store，但没有包含 `YidaoHudStateStore`。
- `client/src/main/java/com/bong/client/combat/CombatHudBootstrap.java:96-120` 的 combat 断线清理也没有覆盖医道 HUD store。
- `server/src/network/yidao_state_emit.rs:28-55` 只在同一 server tick 内发现“本玩家从有医道状态变成无状态”时发送 cleared `yidao_hud_state`；断线 / 换服时新服不知道旧客户端 static store 的内容，不能替客户端兜底清理。

## 修复计划骨架

- [ ] 给 `YidaoHudStateStore` 增加生产态 `clearOnDisconnect()`，语义为恢复 `Snapshot.EMPTY`。
- [ ] 在 `BongNetworkHandler.clearClientStateOnDisconnect()` 接入 `YidaoHudStateStore.clearOnDisconnect()`。
- [ ] 仅作为内存一致性，可同时评估是否要清 `YidaoNpcAiStateStore`；但本 plan 的可见 bug 只以 `YidaoHudStateStore -> BongHudOrchestrator -> YidaoHudPlanner` 为验收主链。

## 验证计划

- [ ] client 回归：先 route active `yidao_hud_state`，断言 `YidaoHudPlanner` 有 `YIDAO` 命令。
- [ ] 调用真实断线清理 helper 后，断言 `YidaoHudStateStore.snapshot().active() == false`，且 `YidaoHudPlanner` 不再产生命令。
- [ ] 加源码 pin 或 bootstrap 测试，确保 `clearClientStateOnDisconnect()` 包含医道 HUD store 清理，防未来新增 static HUD store 时再次漏接。
- [ ] 保留现有“收到 empty `yidao_hud_state` 可清空面板”的测试，不把 server empty payload 当作断线清理替代品。

## 对抗复核结论

两轮对抗 subagent 均接受候选，但要求收窄：

1. Round 1：确认 `YidaoHudStateStore` 会实际渲染到 HUD，disconnect hook 未清理，服务端空 payload 不能覆盖断线 / 换服场景；同时指出 `YidaoNpcAiStateStore` 当前未找到生产 HUD 消费者，不应作为主 bug。
2. Round 2：偏反方复核未找到间接清理路径；确认现有 `plan-yidao-v1` 与测试只覆盖“收到空 payload 后清面板”，不覆盖断线 reset；与 #1094 / #1100 属同类生命周期缺口但对象不同，不重复 #1051 / #1057 / #1063 / #1074 / #1085，也不属于 #1068-#1072。

## 审计来源

BugHunt worker：`client-combat` 分区，r10。方法：PR 去重、client HUD / server_data / disconnect 清理链路只读审计、两轮对抗 subagent 复核。当前结论为 **report-only**：高置信、可通过 client 单测稳定钉住；本 PR 只新增 plan，不修改代码 / 配置 / 资源 / 依赖。
