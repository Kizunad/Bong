# BugHunt: 炼丹 UI/HUD 断线串会话

## Bug 摘要

炼丹客户端把炉体、活跃 session、预测概率、丹毒、试药史、背包重量等状态分别存在静态 store 里，但这些 store 只有 `resetForTests()`，没有生产态断线清理。`BongNetworkHandler.clearClientStateOnDisconnect()` 已清理多类跨 session UI store，却没有清理任何炼丹 store。

结果是玩家在 A 服炼丹进行中断线/切服后，B 服首个炼丹 payload 到达前，客户端仍可能在 HUD 和炼丹炉界面显示 A 服的炼制进度、炉坐标、配方、预测概率、丹毒和最近结算历史。

## 实际游玩体验影响

- 玩家刚进新服或新存档时，屏幕下方仍显示上一局“炼制 xx%”和旧配方状态，误以为当前服务器有一炉丹正在跑。
- 打开炼丹炉界面时，左/中/右栏会读旧炉体、旧 session、旧预测、旧丹毒、旧试药史和旧背包重量，玩家可能按旧上下文投料、点火或取回。
- 如果上一局刚炸炉或刚成丹，`AlchemyAttemptHistoryStore` 的最近结果 toast 仍可能在新会话窗口内出现，造成“新服刚进来就炸炉/成丹”的错误反馈。

## 证据定位

- `client/src/main/java/com/bong/client/alchemy/state/AlchemySessionStore.java:37-51`：活跃炼丹 session 是静态 `snapshot`，只提供 `resetForTests()`。
- `client/src/main/java/com/bong/client/alchemy/state/AlchemyFurnaceStore.java:13-27`：炉体快照是静态 `snapshot`，只提供 `resetForTests()`。
- `client/src/main/java/com/bong/client/hud/AlchemyProgressHudPlanner.java:32-36`：HUD 直接读取 `AlchemyFurnaceStore` / `AlchemySessionStore` / `AlchemyOutcomeForecastStore` / `AlchemyAttemptHistoryStore` 渲染炼制进度和最近结果。
- `client/src/main/java/com/bong/client/alchemy/AlchemyScreen.java:471-498`：炼丹炉界面直接读取炉体、session 的坐标、温度、进度和真元目标。
- `client/src/main/java/com/bong/client/alchemy/AlchemyScreen.java:516-532`：中途投料高亮完全由旧 session 的 `stages` 驱动。
- `client/src/main/java/com/bong/client/alchemy/AlchemyScreen.java:549-613`：预测概率、试药史、丹毒面板分别读取静态 store。
- `client/src/main/java/com/bong/client/BongNetworkHandler.java:857-900`：统一断线清理覆盖 realm collapse、NPC、TSY、采集、棺、天道 UI、遗骸、craft 等，但没有任何炼丹 store。

## 触发路径

1. 玩家在 A 服打开炼丹炉并起炉，客户端收到 `alchemy_furnace`、`alchemy_session`、`alchemy_outcome_forecast`、`alchemy_contamination` 等 payload。
2. 这些 payload 写入炼丹静态 store；HUD 开始显示炼制进度，炼丹界面显示炉体和 session 详情。
3. 玩家断线、切服或进入另一个存档。
4. 断线清理没有清炼丹 store；客户端进 B 服后，B 服首帧 payload 到达前仍保留 A 服快照。
5. HUD 或炼丹界面继续按旧快照渲染，直到 B 服碰巧推送新的炼丹 payload 覆盖。

## 非重复确认

- 不重复 #1049：#1049 是 `mineral_probe_result` 网络线程直触 HUD/SFX；本 bug 是炼丹静态 UI store 断线生命周期。
- 不重复 #1066：#1066 是 Forge 静态 store 断线 stale UI；本 bug 限定炼丹炉 UI/HUD。
- 不重复 #1077：#1077 是 `LingtianSessionStore` 断线未清；本 bug 限定炼丹 store。
- 不重复 `docs/plans-skeleton/plan-bughunt-alchemy-furnace-persistence.md`：该骨架是服务端炼丹炉/炼丹 session 重启持久化缺失。
- 不重复 `docs/plans-skeleton/plan-bughunt-alchemy-furnace-scope-gate.md`：该骨架是服务端距离/维度门禁缺失。
- 不重复 `docs/plans-skeleton/plan-bughunt-ac-alchemy-hud-zero-targets-v1.md`：该骨架是真实炼丹 session payload 目标值为 0/空数组。
- 不重复身份、季节、灵龛守护残留：这些已有 skeleton，分别是 `plan-bughunt-client-identity-panel-stale-session-v1`、`plan-bughunt-q-world-season-dimension-env-resync-v1`、`plan-bughunt-niche-guardian-cross-session-leak-v1`。

## 对抗审查记录

- Round 1 候选挖掘：子代理提出炼丹 UI/HUD 跨服残留，指出 `AlchemySessionStore` / `AlchemyFurnaceStore` 等静态 store 无断线清理，HUD 和 screen 直接读取。
- Round 1 怀疑者：确认 Forge、灵田、灵宝、矿脉探测等题不可重复；认为炼丹 stale screen state 仍可能是新 bug，但必须限定为炼丹炉 UI/丹方/丹毒/预测跨 session 残留。
- Round 2 反驳：本地优先候选灵龛守护被判为 #945 skeleton 重复，已放弃；身份、季节也已有 skeleton。炼丹当前只发现服务端持久化/门禁/零目标值骨架，未覆盖客户端断线 stale store。

## Skeleton Fix Plan

- [ ] 给炼丹客户端 store 增加生产态断线清理入口，至少覆盖 `AlchemySessionStore`、`AlchemyFurnaceStore`、`AlchemyOutcomeForecastStore`、`ContaminationWarningStore`、`AlchemyAttemptHistoryStore`、`InventoryMetaStore`；是否清 `RecipeScrollStore` 需按丹方书是否跨服可信单独定。
- [ ] 在客户端断线回调中调用炼丹清理入口，保留测试专用 reset 与生产态 clear 的边界。
- [ ] 若当前屏幕是 `AlchemyScreen`，断线清理后应关闭或刷新为无炉态，避免用户继续对旧 `furnacePos` 操作。
- [ ] 明确 reconnect 首帧前的空态 UX：HUD 不显示炼制进度，炼丹炉界面不显示旧预测/旧丹毒/旧试药史。

## 验收测试计划

- [ ] client 单测：写入活跃 `AlchemySessionStore` + `AlchemyFurnaceStore` 后执行断线清理，HUD planner 不再产生炼制进度命令。
- [ ] client 单测：写入 `AlchemyAttemptHistoryStore` 后执行断线清理，最近结果 toast 不再跨 session 渲染。
- [ ] client 单测：写入预测、丹毒、背包重量后执行断线清理，`AlchemyScreen` 描述/刷新逻辑回到空态或关闭态。
- [ ] 回归：正常收到新的 `alchemy_*` payload 后，炼丹 HUD 和界面仍能重新显示新 session。
- [ ] 回归命令：在 `client/` 跑 `./gradlew test build`。

## 风险

- 丹方书 `RecipeScrollStore` 可能既像账号知识又像服务器状态；修复前需确认是否应随断线清理，避免误删合法本地丹方展示。
- 清理时如果 screen 仍开着，直接清 store 但不关闭 screen 可能留下空标签和旧 `furnacePos` 操作按钮；修复应覆盖屏幕生命周期。
- 不应把测试用 `resetForTests()` 直接当生产清理长期复用，避免未来 listener/dispatcher 类状态被误清。
