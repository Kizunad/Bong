# plan-bughunt-breakthrough-billboard-session-leak-v1

> **Active plan（由 bughunt promotion）**。一句话主题：`BreakthroughRenderStateStore` 在断线/切服时未清理，若玩家在突破远景标记的 1.5-5s 剩余窗口内快速进入新 session，且旧坐标仍满足 distant/global 可见条件，新世界会短暂渲染上一 session 的“劫/成/破”远景标记。

> 立项动机：这是一个低严重度但高置信的 client visual session hygiene 缺口。它不是长期串档，也不是所有重连必现；问题集中在突破演出 billboard 的短视觉窗口内，当前 store 没有 session/world/dimension 绑定，也没有进入 `BongNetworkHandler.clearClientStateOnDisconnect()` 的清理表。该 plan 已按 active fix plan 收口，允许后续流水线消费。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 突破远景 billboard 断线不清导致短窗口跨 session 幻影标记 | fix_pr | ⬜ |

## P0 — 突破远景 billboard 短窗口跨 session 残留

- **现象**：`BreakthroughRenderStateStore` 用静态 `AtomicReference` 缓存最近一次 `breakthrough_cinematic` 的远景渲染状态；生产代码只有 `replace(...)`，没有 disconnect clear。`BreakthroughBillboardWorldRenderer` 在新 world/player 非空后只看该状态是否未过期，然后继续用 payload 里的旧 `worldX/Y/Z` 绘制“劫/成/破”billboard。
- **第一性原理**：进程级 static client store 只能保存“当前连接 / 当前世界”仍成立的视觉事实；一旦 session 边界跨过，旧 payload 的坐标、world 语义和玩家上下文都失效。自然过期只能限制残留时长，不能证明旧状态属于新 session。
- **触发路径**：
  1. A session 收到一条带 `distant_billboard=true` 的 `breakthrough_cinematic`。
  2. `BreakthroughCinematicHandler` 写入 `BreakthroughRenderStateStore`，过期时间为 `now + visualDurationMillis`。
  3. 玩家在 1.5-5s 演出窗口内断线、切服或回标题后快速进入 B session。
  4. `BongNetworkHandler.clearClientStateOnDisconnect()` 清理大量 HUD/store，但没有清 `BreakthroughRenderStateStore`。
  5. B session 的 `client.world` / `client.player` 恢复后，旧状态仍未过期且坐标满足 billboard 可见条件时，新世界短暂显示 A session 的突破远景标。
- **实际游玩体验影响**：
  - 玩家刚进入新世界时，可能在远处看到与当前世界无关的“劫 / 成 / 破”突破方位标，误以为附近有人正在突破或刚突破完成。
  - 对刚重连后的空间判断有干扰：玩家可能朝旧标记方向移动、回避并不存在的突破事件，或误判当前区域有高境界活动。
  - 影响窗口受限，通常最多 5 秒；完全退出客户端会清空 static 状态，且旧坐标不满足 distant/global 可见条件时不会显示。因此本题应按短窗口视觉串 session 处理，不夸大成长期状态污染。
- **根因**：`BreakthroughRenderStateStore` 的生命周期比连接生命周期长，但它没有生产态清理入口，也没有被纳入 `clearClientStateOnDisconnect()` 的统一清理表。renderer 端只做“状态未过期”和“world/player 非空”判断，不具备识别旧 session 的信息。
- **修复边界 / 不变量**：
  - disconnect / world unload / 新连接边界必须清空突破远景状态。
  - 同一 session 内未过期的突破远景标仍按原视觉窗口显示，不改变 `visualDurationMillis` 与 renderer 的自然过期语义。
  - 修复不应引入跨维观测的隐式特例；若未来需要跨维 billboard，payload 必须显式携带可验证的 session/world 语义。
  - 不修改突破 phase、toast、粒子、SFX 或服务端突破结算，只修客户端 session hygiene。

## 证据定位

- `client/src/main/java/com/bong/client/cultivation/BreakthroughRenderStateStore.java:13`：静态 `AtomicReference<BreakthroughRenderState>` 保存最近状态。
- `client/src/main/java/com/bong/client/cultivation/BreakthroughRenderStateStore.java:17`：生产入口只有 `replace(...)`。
- `client/src/main/java/com/bong/client/cultivation/BreakthroughRenderStateStore.java:27`：只有测试态 `resetForTests()`，没有生产态 `clearOnDisconnect()`。
- `client/src/main/java/com/bong/client/network/BreakthroughCinematicHandler.java:115`：每次收到有效 `breakthrough_cinematic` 都写入 `BreakthroughRenderStateStore`。
- `client/src/main/java/com/bong/client/cultivation/BreakthroughSpectacleRenderer.java:20`：视觉窗口按阶段限制在约 1.5-5s。
- `client/src/main/java/com/bong/client/cultivation/BreakthroughBillboardWorldRenderer.java:42`：renderer 只检查 `client.world/player != null`。
- `client/src/main/java/com/bong/client/cultivation/BreakthroughBillboardWorldRenderer.java:47`：读取 store 后只按过期时间过滤。
- `client/src/main/java/com/bong/client/cultivation/BreakthroughBillboardWorldRenderer.java:71`：绘制使用 payload 中旧 `worldX/Y/Z`。
- `client/src/main/java/com/bong/client/BongNetworkHandler.java:857`：disconnect 清理表覆盖大量 HUD/store，但没有清理突破远景状态。

## 反方审查记录

### Round 1

- **反方论点**：状态会自然过期，最多 1.5-5s，影响可能太弱。
- **裁决**：保留但降级。短窗口不消除跨 session 错渲染；仓库已有 toast 等 3-8s 短窗口 session leak 记录，说明“自然过期”不是充分反证。

### Round 2

- **反方论点**：5 秒内切服/重进不常见，`billboardFor` 还会按 `distant_billboard`、距离和可见半径收窄复现面。
- **裁决**：保留并收窄表述。它不是必现，也不是长期残留；但 dev/local server、快速 reconnect、服务器列表直连都可能在窗口内回到新 session。renderer 没有 session/world/dimension guard，JOIN 后旧状态未过期就可继续绘制。

### 去重结论

- 不重复 #1099 / #1105 / #1108 / #1110 等近期 client-ui/client-combat HUD 残留题；那些落在神识、医道、保鲜、毒性真元等具体 HUD store。
- 不被 `plan-bughunt-hud-state-session-reset` 吸收：该题聚焦 `BongHudStateStore.zoneState/visualEffectState`，突破远景是 `cultivation` world renderer 的独立 static store。
- 全仓 docs 精确检索未发现 `BreakthroughRenderStateStore` / `BreakthroughBillboardWorldRenderer` / `breakthrough_cinematic disconnect` 同题 active plan。

## 修复计划

TODO:

- [ ] 给 `BreakthroughRenderStateStore` 增加生产态清理入口，例如 `clearOnDisconnect()` 或 `clear()`。
- [ ] 在 `BongNetworkHandler.clearClientStateOnDisconnect()` 中清空突破远景状态。
- [ ] 评估是否在 world unload / JOIN 时也清理，避免非标准切换路径绕过 disconnect。
- [ ] 保持自然过期语义不变：同一 session 内 1.5-5s 的突破远景标仍按原计划显示。

## 验收测试计划

- [ ] 单测：写入未过期 `BreakthroughRenderState` 后调用 disconnect 清理入口，`BreakthroughRenderStateStore.snapshot()` 应为 `null`。
- [ ] 单测：同一 session 内未触发 disconnect 时，未过期状态仍可被读取，过期状态仍由 renderer 过滤。
- [ ] 源码守护：`BongNetworkHandler.clearClientStateOnDisconnect()` 包含突破远景状态清理调用。
- [ ] 手工验证：A session 触发远景突破标后快速断线进入 B session，B session 首屏不显示 A session 的“劫/成/破”标记。
- [ ] 回归验证：同一 session 内普通突破演出仍能显示远景 billboard，不因新增清理入口被提前抹掉。

## 风险

- 清理过早可能让同一 session 内短暂网络抖动期间的远景标消失；应只挂在明确 disconnect/world unload 边界。
- 如果未来突破演出需要跨维观测，应显式在 payload 中携带可验证 session/world 语义，而不是依赖进程级 static store。
