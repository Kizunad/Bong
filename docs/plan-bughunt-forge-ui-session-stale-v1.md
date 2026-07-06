# plan-bughunt-forge-ui-session-stale-v1

BugHunt skeleton: Forge UI / HUD static stores survive disconnect and can show the previous server's forging session until a new forge payload happens to overwrite them.

## Scope

- 分区：client-ui，第 2 轮。
- 范围：Fabric client 非战斗 UI；锻造 ForgeScreen、processing HUD、client store 生命周期。
- 明确不含：combat A/V、server 权威状态修复、#999 炼器 C2S 起炉推进断链、#993 主 HUD zone/visual-effect 跨 session、#1049 mineral_probe_result 网络线程碰 HUD/SFX。

## 实际游玩体验影响

玩家在旧服或旧存档里收到过 `forge_session` / `forge_outcome` / `forge_station` / `forge_blueprint_book` 后断线，再进入新服或新存档：

- 若当前没有普通 Screen，HUD FULL 路径会继续用旧 `ForgeSessionStore` 渲染锻造 processing HUD，可能看到上一服的“淬火中 / 铭文刻划 / 祭炼中”进度。
- 重连后按默认 U 打开 ForgeScreen，会看到上一服的砧 owner、会话、图谱、上次结果。
- 如果旧会话停在 `tempering` / `inscription` / `consecration`，玩家操作 J/K/L、放铭文、注入真元时，客户端会向新服发送旧 `session_id` 请求；服务端有 guard 会拒绝，但玩家端已经被旧 UI 和本地反馈误导。
- 这不是服务端状态污染；影响是客户端 stale UI、错误操作反馈、服务端 warn 噪音。

## 复现路径

1. 进入服务器 A，触发任意 forge payload：开始一个锻造会话，或完成一次锻造让 `forge_outcome` 写入。
2. 断线，不退出 Minecraft 进程。
3. 进入服务器 B 或同进程新存档，在服务器 B 还没有发新的 forge payload 前观察 HUD。
4. 按 U 打开 ForgeScreen。
5. 若旧会话是 `tempering`，在 ForgeScreen 内按 J/K/L；客户端会基于旧 `session_id` 发 `forge_tempering_hit`，服务端拒绝。

## 根因证据

- `client/src/main/java/com/bong/client/forge/ForgeScreenBootstrap.java` 注册默认 U 键并直接 `client.setScreen(new ForgeScreen())`，没有发送 forge open / hydration 请求，也没有注册 disconnect 清理。
- `ForgeSessionStore`、`ForgeOutcomeStore`、`ForgeStationStore`、`BlueprintScrollStore` 都是 static/volatile store，只提供 `resetForTests()`，没有 `clearOnDisconnect()`。
- `client/src/main/java/com/bong/client/network/ServerDataRouter.java` 注册 `forge_station`、`forge_session`、`forge_outcome`、`forge_blueprint_book`，handler 只会 replace store；没有 session 边界。
- `client/src/main/java/com/bong/client/hud/BongHudOrchestrator.java` 在 HUD FULL 路径加入 `ForgeProgressHudPlanner.buildCommands(...)`。
- `ForgeProgressHudPlanner` 每帧读取 `ForgeSessionStore.snapshot()`；`active()` 为真时渲染 processing HUD，并且只要 `ForgeOutcomeStore.lastOutcome().sessionId() > 0` 就继续输出炼器完成 toast command。
- `ForgeScreen.render()` 直接读取四个 Forge store 渲染砧、会话、图谱、上次结果。
- `server/src/network/forge_snapshot_emit.rs` 的 join emit 是 placeholder；`send_forge_snapshots_to_player` 当前只有定义和注释，未发现调用点。因此没有 join/open hydration 保证会覆盖旧 store。
- 服务端 `require_owned_active_step` 会校验 session 存在、step 匹配、caster 匹配，因此旧 `session_id` 请求大概率被拒绝；这支持“客户端误导”而非“服务端污染”的定性。

## 修复计划骨架

P0. Client store 生命周期

- 为 `ForgeSessionStore`、`ForgeOutcomeStore`、`ForgeStationStore`、`BlueprintScrollStore` 增加生产用 clear/reset API，保留 listener 语义时不得复用 `resetForTests()`。
- 在 client disconnect 回调中清理四个 Forge store，并重置 `ForgeProgressHudPlanner` 的 `lastSessionId/lastStep/stepChangedAt`。

P1. Forge 打开 hydration

- 把 ForgeScreen 打开改为 C2S open/inspect 请求驱动，或在现有锻炉交互路径上要求服务端立即回发 station/session/blueprint/outcome 快照。
- 没有权威快照前，ForgeScreen 显示“等待锻炉同步”而不是读旧 store。

P2. 请求门禁

- `TemperingInputHandler`、铭文、祭炼注入在本地 store 未标记为当前连接的权威会话前禁止发请求。
- 对旧 session 拒绝结果补一个玩家可见提示，避免只在 server warn 里暴露。

## 验证计划

- Client 单测：模拟写入 forge store 后调用 disconnect clear，断言四个 store 回到 empty，`ForgeProgressHudPlanner.buildCommands` 不再产出 processing HUD / forge outcome toast。
- Client 单测：ForgeScreen 在未 hydration 状态下不展示旧会话、旧图谱、旧结果。
- Client 请求单测：旧 session 或未 hydration 时 J/K/L、铭文、祭炼不发送 `bong:client_request`。
- 集成/e2e：同一 Minecraft 进程连接 A 写入 forge payload，断线连接 B，确认 HUD 和 ForgeScreen 不继承 A 的 forge 状态。

## 对抗复核结论

已完成两轮反方复核。

- 第一轮质疑：断线菜单阶段不可见；ForgeScreen 不是 HUD FULL；U 键是否正式入口；可能是 hydration 缺口而非单纯 disconnect；服务端会拒绝旧 session；`lastOutcome` 同连接内可能是设计。
- 修正：题面收窄为“重连进入世界后 / 重连后按 U 打开 ForgeScreen”；不写服务端状态污染；把 open hydration 缺口纳入修复计划；`lastOutcome` 只要求跨 server / 跨角色 / 跨存档清理。
- 反方最终裁决：按收窄后的题面，这不是误报，候选成立；剩余反方意见只影响措辞和严重性，不足以推翻。
