# plan-bughunt-sparring-invite-screen-hijack-v1

> **状态：✅ 2026-07-11**。一句话主题：`client/social` 的 `sparringInvite` store → tick bootstrap 链路会在玩家正开其他 GUI 时强行抢屏，导致 UI 状态被中断；同目录 `trade offer` 已有“被其他 GUI 挡住时只 toast、不抢屏”的修复先例，因此这是高置信真实 bug，而不是预期 UX。

## 1. 结论

- **bug**：`SocialStateStore.replaceSparringInvite(...)` 写入 invite 后，`SparringInviteScreenBootstrap.onEndClientTick(...)` 每 tick 都会检查 `client.currentScreen`；只要当前屏幕不是同一份 `SparringInviteScreen`，就直接 `client.setScreen(new SparringInviteScreen(invite))`。
- **结果**：切磋邀请到达时，会把背包、锻造、炼丹、身份面板、卷轴阅读等当前 GUI 直接顶掉。
- **范围**：`client/UI state/store/runtime bridge`，具体是 `server_data → SocialStateStore.sparringInvite → END_CLIENT_TICK bootstrap → Screen`。

## 2. 复现路径

1. 玩家先打开任意非切磋 GUI，例如 `InspectScreen`、`ForgeScreen`、`AlchemyScreen`、`IdentityPanelScreen`。
2. 服务端下发 `sparring_invite` payload，`SocialServerDataHandler.handleSparringInvite` 调 `SocialStateStore.replaceSparringInvite(invite)`。
3. 下一帧 `SparringInviteScreenBootstrap.onEndClientTick` 读取到 invite 存在。
4. 因 `currentScreen` 不是 `SparringInviteScreen`，命中 `client.setScreen(new SparringInviteScreen(invite))`。
5. 玩家当前 GUI 被强行关闭，切到切磋邀请屏。

## 3. 证据链

- `client/src/main/java/com/bong/client/network/SocialServerDataHandler.java:181-207`
  `handleSparringInvite` 在 payload 合法时无额外 gating，直接 `SocialStateStore.replaceSparringInvite(invite)`。
- `client/src/main/java/com/bong/client/social/SocialStateStore.java:72-80`
  `replaceSparringInvite` 只是覆盖 `volatile sparringInvite`，没有屏幕分类或阻塞态。
- `client/src/main/java/com/bong/client/social/SparringInviteScreenBootstrap.java:16-37`
  tick 钩子里只区分“无 invite / 已超时 / 其他所有情况”；第 34-36 行对任何“不是当前 invite 对应切磋屏”的场景一律 `setScreen(...)`。
- `client/src/main/java/com/bong/client/BongClient.java:138-139`
  `SparringInviteScreenBootstrap.register()` 和 `TradeOfferScreenBootstrap.register()` 同时注册，说明两者是平行的“社交邀请入屏”通道。
- `client/src/main/java/com/bong/client/social/TradeOfferScreenBootstrap.java:34-124`
  交易邀请已经专门引入 `ScreenKind/Decision/BLOCKED_TOAST`；当 `screenKind == OTHER` 时只 toast，不抢走当前 GUI。这是同域、同模式、更新更近的直接先例。
- `client/src/test/java/com/bong/client/social/TradeOfferScreenBootstrapTest.java:16-219`
  交易邀请已有“其他 GUI 挡住时只给非阻塞提示”的测试锁定；全仓没有 `SparringInviteScreenBootstrap` 的对等测试，说明这条链路没有被类似回归保护覆盖。

## 4. 根因链路

1. `sparring_invite` 的 client store 只有单 slot 覆盖语义，没有“被其他屏挡住”状态表达。
2. `SparringInviteScreenBootstrap` 采用 tick 轮询，但没有像 trade offer 那样先做 `ScreenKind` 分类。
3. 因缺少 `OTHER` 分支的非阻塞策略，代码把“有 invite 且当前不是 matching sparring screen”误当成“应立即开屏”。
4. 最终形成 `payload/store` 正常、`runtime bridge` 过度激进、UI 状态被抢占的断层。

## 5. 这个 bug 对实际游玩体验的影响

- 玩家在整理背包、锻造、炼丹、看身份/卷轴时，会被一份切磋邀请直接打断并换屏。
- 很多界面状态是瞬时的：输入框内容、拖拽中物品、配方筛选、阅读上下文、临时 hover/选择都可能被打断或丢失。
- 体感上像“别人发来邀请就能强制把我的界面抢走”，比普通 toast 更侵入，也容易让玩家误以为客户端卡顿、误触或 UI 自己崩了。

## 6. 修复建议

- 复用 `TradeOfferScreenBootstrap` 的模式，为切磋邀请补 `ScreenKind + Decision` 判定。
- 当 `currentScreen == null` 时可开 `SparringInviteScreen`；当 `currentScreen` 是别的 GUI 时，改为一次性 toast / HUD 提示，不抢屏。
- 保留超时自动拒绝，但应在被挡住期间给玩家明确提示“有切磋邀请待处理”。
- 为 `SparringInviteScreenBootstrap` 增加和 trade offer 对等的纯决策单测，至少覆盖：
  `NONE / MATCHING / OTHER_SPARRING / OTHER / expired`。

## 7. Skeleton 反方裁决记录

> 下列两轮是 skeleton 立项时记录的反方论点与驳回理由；实施完成后仍须由全新无上下文只读 validator 对最终 HEAD 独立裁决。

### Round 1

- **反方论点**：切磋邀请本来就是高优先级 modal，强制弹屏可能是产品设计，不算 bug。
- **驳回理由**：同目录同类“社交邀请”中的 `TradeOfferScreenBootstrap` 已明确把“其他 GUI 挡住时”定义为 `BLOCKED_TOAST` 而非抢屏；仓库现有 UX 先例已经表明“有邀请 ≠ 可以强制夺走当前屏幕”。

### Round 2

- **反方论点**：被切到切磋屏后，玩家可以立刻拒绝；这只是打断，不会造成持久数据错误。
- **驳回理由**：本次 bug 归类本就不是数据损坏，而是 UI state/store/runtime bridge 行为错误。`setScreen(...)` 发生在玩家同意前，已经足以破坏当前 GUI 的瞬时状态与操作连续性；没有 toast 预告、没有 defer、没有恢复路径，因此仍是实质游玩缺陷。

## 8. 建议修复落点

- `client/src/main/java/com/bong/client/social/SparringInviteScreenBootstrap.java`
- 参考：`client/src/main/java/com/bong/client/social/TradeOfferScreenBootstrap.java`
- 可选补测：`client/src/test/java/com/bong/client/social/SparringInviteScreenBootstrapTest.java`

## 实施证据

- Promotion：`33aaae28`，canonical skeleton 已独立升格为 active plan。
- 第一性原理 RED：`b8614841` 新增纯决策与 toast 契约后，JDK 17 targeted 在 `compileTestJava` 以 29 个缺失符号失败；生产类确实没有 `ScreenKind`、`Decision`、`decide`、blocked/expired toast 或去重状态，且现有 tick 路径会直接 `setScreen` 抢占其他 GUI。
- 最小修复：`94ddd91a` 仅调整 client `SparringInviteScreenBootstrap`，把当前屏幕分类后交给纯决策矩阵；`OTHER` 只发一次性非阻塞 toast，`NONE` / 陈旧切磋屏正常打开当前邀请，matching screen 保持 no-op，过期邀请继续自动拒绝并新增明确提示。
- Targeted GREEN：JDK 17 执行 `./gradlew test --tests com.bong.client.social.SparringInviteScreenBootstrapTest` 为 `BUILD SUCCESSFUL`，9 个契约测试全部通过，覆盖 null/expired 边界、NONE/matching/stale/OTHER 分支与 toast 去重/重触发。
- 范围：未修改 server、schema、依赖、生产配置、工具链或视觉资产；同一 bug 的重复 skeleton `plan-bughunt-v-sparring-invite-screen-hijack-v1` 留给后续主干去重处理，本 PR 不跨 plan 修改。
- 后续门禁：fresh validator PASS 后，以 JDK 17 执行完整 `./gradlew test build`；合并最新 `origin/main` 后任何 HEAD 变化都重新验证。

## Finish Evidence

### 落地清单

- `client/src/main/java/com/bong/client/social/SparringInviteScreenBootstrap.java`
  - 新增 `ScreenKind` / `Decision` 纯决策矩阵。
  - 其他 GUI 打开时走 `BLOCKED_TOAST`，不再调用 `setScreen` 抢屏。
  - 保留无屏开邀请、陈旧邀请屏替换、matching no-op、过期自动拒绝与 store 清理语义。
  - blocked toast 按 inviteId 去重，新邀请可重新提示。
- `client/src/test/java/com/bong/client/social/SparringInviteScreenBootstrapTest.java`
  - 9 个测试覆盖 null、过期边界、NONE、matching、stale、OTHER 与 toast 去重/重触发。

### 关键 commit

- `33aaae28`（2026-07-11）：提升 canonical BugFix plan。
- `b8614841`（2026-07-11）：先写 RED 契约，锁定不得抢占其他界面。
- `94ddd91a`（2026-07-11）：最小修复切磋邀请屏幕调度。
- `72775467`（2026-07-11）：记录第一性原理 RED / GREEN 证据。
- `a4ce546c`（2026-07-11）：同步最新 `origin/main`，无冲突且未触及本修复三文件。

### 测试结果

- JDK 17 targeted RED：`compileTestJava` 因生产类缺少 `Decision` / `ScreenKind` / `decide` / toast 接口而失败，共 29 个缺失符号。
- JDK 17 targeted GREEN：`./gradlew test --tests com.bong.client.social.SparringInviteScreenBootstrapTest` → `BUILD SUCCESSFUL`，9/9 PASS。
- JDK 17 pre-merge full gate：`./gradlew test build` → `BUILD SUCCESSFUL`。
- JDK 17 post-merge full gate：`./gradlew test build` → `BUILD SUCCESSFUL`，13 tasks（7 executed / 6 up-to-date）。
- Fresh read-only validator：`VERDICT: PASS — HEAD a4ce546c4b4729e188f73df147f15d2af7d5afd9`。

### 跨仓库核验

- client-only 修复；未修改 server / agent / schema / proto。
- `SocialServerDataHandler → SocialStateStore.sparringInvite → SparringInviteScreenBootstrap` 接收链保持不变，仅收紧最后一段 screen 调度。
- 独立 NPC/玩家切磋协议、server 超时与响应语义均未改变。

### 遗留 / 后续

- `docs/plans-skeleton/plan-bughunt-v-sparring-invite-screen-hijack-v1.md` 是同 bug 的重复 skeleton，留给主干在本 PR 合并后按锁运维/去重流程处理；本 PR 遵守一个 plan 一个 PR，不跨 plan 删除。
- 测试以纯决策与 toast 状态为主，未直接 mock `MinecraftClient#setScreen`；validator 已静态核对 runtime switch 中 `OTHER` 分支无 `setScreen` 路径。
