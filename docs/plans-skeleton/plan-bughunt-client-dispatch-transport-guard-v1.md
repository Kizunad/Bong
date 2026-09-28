# BugHunt：客户端请求传输断线时 dispatch 异常穿出

> 来源 Issue：#1564、#1663、#1697。三处都是客户端入口直接调用会抛异常的 dispatch，传输窗口不可用时应 fail closed。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | Agent UI、炼丹开炉、矿脉探测入口改用非抛发送或捕获失败 | ⬜ |
| P1 | 断线/未连接回归，保证 UI 清理与 tick 继续运行 | ⬜ |

## §0 摘要

AgentUiScreen.sendResponse、AlchemyScreenBootstrap.requestOpenAlchemyScreen 和 MineralSenseBootstrap.onEndClientTick 在没有可用 play connection 时调用抛异常的 ClientRequestSender.dispatch。异常会越过 screen close 或 END_CLIENT_TICK，导致客户端崩溃、面板卡死或后续 tick 被中断。

## §1 游玩影响

断线瞬间按 ESC/按钮、右键炼丹炉或按 N 感矿都可能触发未捕获 IllegalStateException；玩家看到崩溃/致命错误，Agent UI 甚至不会执行后续清理。

## §2 复现路径

1. 保持 Agent UI 或炼丹开炉入口处于可点击状态，断开 play connection。
2. 触发 ESC/按钮、右键炉子或 mineral_sense 键。
3. 观察 ClientRequestSender.dispatch 在传输不可用时抛出并越过调用方。

## §3 今天 origin/main 的根因证据

- client/src/main/java/com/bong/client/agentui/AgentUiScreen.java:315 直接 sendAgentUiResponse；ClientRequestSender.java:672-680 走抛异常 dispatch，而 close 清理在其后。
- client/src/main/java/com/bong/client/alchemy/AlchemyScreenBootstrap.java:20-24 的 client.execute lambda 直接发 sendAlchemyOpenFurnace，同样未捕获。
- client/src/main/java/com/bong/client/mineral/MineralSenseBootstrap.java:64-84 的 END_CLIENT_TICK 直接发 sendMineralProbe；对比 ClientRequestSender.tryDispatch 的非抛入口，三处均缺 fail-closed。

## §4 非重复比对与立项检查记录

- worldview：查断线、交互失败和炼丹/感知入口；不改变玩法规则。
- finished_plans：查 plan-agent-ui-*、plan-alchemy-client-v1、plan-interaction-intent-cleanup-v1；确认各协议已存在，本 issue 只补 transport guard。
- active plan：查 AgentUiScreen、AlchemyScreenBootstrap、MineralSenseBootstrap、dispatch；未找到三处统一修法。
- skeleton：查 ClientRequestSender.dispatch/tryDispatch 与断线入口；无同一批 guard 骨架。
- reminder.md：仓内无 docs/plans-skeleton/reminder.md，无相关条目。

## §5 修复骨架

统一采用 tryDispatch/返回失败结果：请求失败不抛到 UI/tick；Agent UI 无论发送是否成功都继续 clear/super.close，炼丹入口不打开半成品界面，矿脉按键失败只记录可控日志并继续 tick。测试覆盖已连接成功、断线失败和重复触发。

## §6 接入面与跨仓契约

- Inputs：agent_ui_response、alchemy_open_furnace、mineral_probe 的既有 C2S JSON。
- Outputs：成功时保持既有 server handlers/回执；失败时只有本地 no-op/日志，不伪造状态。
- 共享类型或事件：复用 ClientRequestProtocol、ClientRequestSender.dispatch/tryDispatch、AgentUiStore、MineralProbeResult。
- server 符号：handle_agent_ui_response、handle_alchemy_open_furnace、mineral_probe handler；协议无变更。
- agent：Agent UI 的 agent command/schema 无变更，失败发生在 client transport 末端；矿脉/炼丹无 agent 变更。
- client：三处入口与 sender failure seam 有变更，保持 server payload 名称不变。
- worldview / qi_physics：本骨架只处理连接失败，不改变炼丹或矿脉真元结算；既有 server ledger 仍是唯一权威。
