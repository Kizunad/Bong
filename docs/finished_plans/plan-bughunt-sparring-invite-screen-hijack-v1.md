# plan-bughunt-sparring-invite-screen-hijack-v1

> **状态：已归档 ✅ 2026-07-11**。一句话主题：`client/social` 的 `sparringInvite` store → tick bootstrap 链路会在玩家正开其他 GUI 时强行抢屏，导致 UI 状态被中断；同目录 `trade offer` 已有“被其他 GUI 挡住时只 toast、不抢屏”的修复先例，因此这是高置信真实 bug，而不是预期 UX。

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
- 初版修复：`94ddd91a` 把当前屏幕分类后交给纯决策矩阵；`OTHER` 只发一次性非阻塞 toast，`NONE` 打开当前邀请，matching screen 保持 no-op，过期邀请继续自动拒绝并新增明确提示。
- 首轮对抗审查：Ultra read-only validator 对 `1dc14dc4ae70989d7975b8aebe45f759d22bdde8` 判定 FAIL，指出 store 的 clear/replace 竞态会吞新邀请、不同 inviteId 的邀请屏仍会互相抢占、关闭后迟到/重复 payload 可重开，以及测试仅覆盖纯决策而未锁运行态。
- 生命周期返工：`6e5363d7` 将 pending 邀请改为同步有序队列，按 `expiresAtMs + UUIDv7 inviteId` 拒绝迟到 payload，并用有界 settled tombstone 拒绝关闭后的重放；`d194e26c` 禁止任何不同邀请屏被新邀请替换，过期时也只关闭 identity 匹配的屏；`2e1b22b0` 用真实 `SparringInviteScreen.close()` 锁住精确清理与后继邀请提升。
- Targeted GREEN：Temurin JDK 17.0.19 强制重编译执行 bootstrap / screen / handler 三组聚焦套件，27/27 PASS，覆盖无 screen、同 screen、不同 screen、迟到邀请、重复邀请、关闭、过期边界及 clear/enqueue 并发交错。
- 最终生命周期收紧：`43960581` 将邀请清理改成精确 identity-CAS，只有首次 claim 成功者发送 C2S；`a38f50c3` 在应战时原子 tombstone 并清空其余 pending，避免进入切磋后继续弹邀请；`ef953fbd` 将网络可写 pending 队列限制为 64 项，容量拒绝不推进版本高水位。
- 最新主线与门禁：`ed360c4a` 合入 `origin/main@340d7776`，主线只带入 botany server/docs，未触及本修复客户端文件；Temurin JDK 17.0.19 聚焦 37/37 与完整 3832/3832 均为 GREEN。
- 推送前最新主线复核：`129bf6ab` 合入 `origin/main@f3a2709a`；新增 client 音频/断线回归与 worldgen 变更均未触及本修复的 `client/social` 目标文件。Temurin JDK 17.0.19 完整强制重编译门禁为 450 suites / 3858 tests，0 skipped / failures / errors，13/13 tasks 实际执行。
- 断线提示复位：`5464f21b` / `10817e55` 以生产 `CombatHudBootstrap.resetOnDisconnect()` 对拍，补齐 blocked-toast inviteId 去重状态的跨 session 清理；本轮 Temurin JDK 17.0.19 聚焦四组套件强制重跑为 38/38 PASS，11/11 tasks 实际执行。
- 前次主线同步：`aec09c58` 普通合入 `origin/main@3123e60f`；主线新增工作台/容器交互与 forge 修复，未触及本 PR 的 `client/social` 文件。合并后 Temurin JDK 17.0.19 完整强制重跑为 451 suites / 3880 tests，0 skipped / failures / errors，并成功产出客户端 jar。
- High validator 返工：全新 `fork_context:false`、`gpt-5.6-sol` high 只读 validator 对 `528edfa5` 判定 FAIL；代码行为无 finding，阻塞项是审查期间主线前进及 4 个历史提交缺少 `Model:` trailer。`0e4bf610` 随后普通合入 `origin/main@d0f1a766`，Temurin JDK 17.0.19 完整强制重跑 451 suites / 3886 tests 全绿，13/13 tasks 实际执行。
- 旧 e2e 归因：run `29144858927` / artifact `8246606606` 中 Task 13 smoke 8/8、Redis 15/15、Bot 23/24；唯一失败是共享 `production_forge_station_real_place` 的 `forge_session current_step=tempering` 45 秒超时。同一 run 的 Java 17 client stage 实际执行 `./gradlew test`，11/11 tasks 全执行并成功，目标 screen 测试未失败。
- 独立复审：全新 `fork_context:false`、`gpt-5.6-sol` Ultra 只读 validator 对 `ed360c4ae11b3e85f8146852f9a14e1c1818d409` 判定 PASS，确认屏幕身份隔离、队列线性化/容量、精确结算、断线清理成立，旧 e2e forge 超时与客户端变更无关。
- 范围：未修改 server、schema、依赖、生产配置、工具链或视觉资产；同一 bug 的重复 skeleton `plan-bughunt-v-sparring-invite-screen-hijack-v1` 留给后续主干去重处理，本 PR 不跨 plan 修改。
- 后续门禁：plan 证据提交会改变 HEAD，因此该文档新 HEAD 仍须再跑一次全新只读 Ultra validator；push 后重新等新 HEAD 的 e2e 与 `/review`。

## Finish Evidence

### 落地清单

- `client/src/main/java/com/bong/client/social/SparringInviteScreenBootstrap.java`
  - 新增 `ScreenKind` / `Decision` 纯决策矩阵。
  - 其他 GUI 或不同 inviteId 的切磋屏打开时走 `BLOCKED_TOAST`，不再调用 `setScreen` 抢屏。
  - 保留无屏开邀请、matching no-op、过期自动拒绝；过期只关闭 identity 匹配的邀请屏。
  - 过期响应先原子 claim identity，重复 screen/tick 不能重复发送 C2S。
  - blocked toast 按 inviteId 去重，新邀请可重新提示。
- `client/src/main/java/com/bong/client/social/SocialStateStore.java`
  - pending 邀请按到达顺序排队，所有读取、入队与 identity 清理在同一 monitor 下线性化。
  - `expiresAtMs + UUIDv7 inviteId` 固定新旧顺序；重复、迟到和已结算 payload 不进入队列。
  - `clearSparringInvite` 只接受非空精确 identity 并返回首次 claim 结果；空/未知/已结算 identity 安全 no-op。
  - 应战时原子清空并 tombstone 其余 pending；拒绝/超时只清当前并提升后继。
  - pending 与 settled tombstone 均有界保留 64 个 ID；生产 `CombatHudBootstrap.resetOnDisconnect` 会同时清空队列、tombstone 与版本高水位。
- `client/src/main/java/com/bong/client/network/SocialServerDataHandler.java`
  - 只为首次接受的邀请发布 HUD 事件；重复、迟到、已结算、容量溢出和非法 payload 安全 no-op。
- `client/src/test/java/com/bong/client/social/SparringInviteScreenBootstrapTest.java`
  - 10 个测试覆盖 null、过期边界、NONE、matching、不同邀请屏、OTHER、真实 screen identity 与 toast 去重/重触发。
- `client/src/test/java/com/bong/client/social/SparringInviteScreenTest.java`
  - 5 个真实 screen 测试验证精确关闭、后继提升、迟到/重复 screen 零重复 C2S，以及应战后原子结清队列。
- `client/src/test/java/com/bong/client/network/SocialServerDataHandlerTest.java`
  - 16 个测试覆盖同 ID 重放、版本迟到、同毫秒 UUIDv7 顺序、settled 重放、空 identity、64 项容量与 32 轮 clear/enqueue 并发交错。
- `client/src/test/java/com/bong/client/combat/CombatHudBootstrapTest.java`
  - 经生产断线 helper 验证 pending、tombstone、版本高水位全复位，并允许新 session 重新接受旧 identity。

### 关键 commit

- `33aaae28`（2026-07-11）：提升 canonical BugFix plan。
- `b8614841`（2026-07-11）：先写 RED 契约，锁定不得抢占其他界面。
- `94ddd91a`（2026-07-11）：最小修复切磋邀请屏幕调度。
- `72775467`（2026-07-11）：记录第一性原理 RED / GREEN 证据。
- `a4ce546c`（2026-07-11）：同步最新 `origin/main`，无冲突且未触及本修复三文件。
- `6e5363d7`（2026-07-11）：原子排队邀请并拒绝迟到、重复与已结算重放。
- `d194e26c`（2026-07-11）：禁止不同 inviteId 的切磋邀请屏互相替换。
- `2e1b22b0`（2026-07-11）：锁定真实 screen 关闭后的 identity 清理与队列提升。
- `0ef01f0b`（2026-07-11）：合并最新 `origin/main@37447572`，无冲突且未触及本修复文件。
- `ac9efcf0`（2026-07-11）：以 RED 锁定空 identity、重复/迟到 screen 与生产断线清理边界。
- `43960581`（2026-07-11）：原子 claim 精确 identity，禁止重复响应与空 ID 清当前。
- `9512defd` / `a38f50c3`（2026-07-11）：以 RED/GREEN 锁定应战后结清排队邀请。
- `eae7f4e9` / `ef953fbd`（2026-07-11）：以 RED/GREEN 锁定 64 项 pending 容量。
- `ed360c4a`（2026-07-11）：合并最新 `origin/main@340d7776`，未触及本修复客户端文件。
- `129bf6ab`（2026-07-11）：推送前合并最新 `origin/main@f3a2709a`，无冲突且未触及本修复 `client/social` 文件。
- `5464f21b` / `10817e55`（2026-07-11）：以 RED/GREEN 锁定生产断线入口复位切磋邀请 blocked-toast 去重状态。
- `0e4bf610`（2026-07-12）：普通合入最新 `origin/main@d0f1a766`，覆盖 race-system 的 proto/schema/client 请求链变更。
- 历史模型 provenance 补记：执行会话记录确认 `129bf6ab` / `f16b6f16` 为 `gpt-5.6-sol-ultra`，`5464f21b` / `10817e55` 为 `gpt-5.6-sol-high`。用户要求保留这些提交且禁止 amend/rebase，因此仅以追加证据补记，不改写既有 SHA。

### 测试结果

- JDK 17 targeted RED：`compileTestJava` 因生产类缺少 `Decision` / `ScreenKind` / `decide` / toast 接口而失败，共 29 个缺失符号。
- JDK 17 targeted GREEN：`./gradlew test --tests com.bong.client.social.SparringInviteScreenBootstrapTest` → `BUILD SUCCESSFUL`，9/9 PASS。
- Temurin JDK 17.0.19 lifecycle targeted：bootstrap 10 + screen 3 + social handler/store 14 = 27/27 PASS，`11 actionable tasks: 11 executed`。
- 首轮 Ultra read-only validator：`FAIL — SHA 1dc14dc4ae70989d7975b8aebe45f759d22bdde8`；四项 finding 已由 `6e5363d7` / `d194e26c` / `2e1b22b0` 返工并补测。
- JDK 17 pre-merge full gate：`./gradlew test build` → `BUILD SUCCESSFUL`。
- JDK 17 post-merge full gate：`./gradlew test build` → `BUILD SUCCESSFUL`，3827 tests，0 skipped / failures / errors，13 tasks（3 executed / 10 up-to-date）。
- 最终 RED：精确结算套件 25 tests 中 3 项按预期失败；应战清场 5 tests 中 1 项按预期失败；容量测试在缺失 `CAPACITY` enum 处按预期编译失败。
- 最终 targeted GREEN：Temurin JDK 17.0.19，bootstrap 10 + screen 5 + handler/store 16 + `CombatHudBootstrapTest` 6（其中 1 条锁生产断线清理）= 37 tests，0 skipped / failures / errors，11/11 tasks 强制执行。
- 最终 full gate：`./gradlew test build --rerun-tasks` → `BUILD SUCCESSFUL`，447 suites / 3832 tests，0 skipped / failures / errors，13/13 tasks 实际执行。
- 最新主线合并后 full gate：Temurin JDK 17.0.19，`./gradlew test build --rerun-tasks` → `BUILD SUCCESSFUL`，450 suites / 3858 tests，0 skipped / failures / errors，13/13 tasks 实际执行。
- 断线复位聚焦门禁：Temurin JDK 17.0.19，bootstrap + screen + handler/store + `CombatHudBootstrapTest` 四组套件共 38 tests，0 skipped / failures / errors，11/11 tasks 强制执行。
- 最终主线合并后 full gate：Temurin JDK 17.0.19，`./gradlew test build --rerun-tasks` 实际生成 451 suites / 3880 tests，0 skipped / failures / errors；测试后成功产出 `bong-client-0.1.0.jar`。
- Validator 返工后 full gate：Temurin JDK 17.0.19，`./gradlew test build --rerun-tasks` → `BUILD SUCCESSFUL in 2m 6s`，451 suites / 3886 tests，0 skipped / failures / errors，13/13 tasks 实际执行。
- 最终 Ultra read-only validator：`PASS — SHA ed360c4ae11b3e85f8146852f9a14e1c1818d409`；模型 `gpt-5.6-sol`、reasoning `ultra`、`fork_context:false`。
- 旧共享 e2e：run `29144858927` 的 Java 17 client `./gradlew test` 成功；artifact `e2e-evidence` 显示 Task 13 smoke 8/8、Redis 15/15、Bot 23/24，唯一红项为无关 forge station 超时。

### 跨仓库核验

- client-only 修复；未修改 server / agent / schema / proto。
- `SocialServerDataHandler → SocialStateStore.sparringInvite → SparringInviteScreenBootstrap` 接收链保持不变；client 内部补充队列、版本/tombstone 与 identity 调度。
- 独立 NPC/玩家切磋协议、server 超时与响应语义均未改变。
- 最新同步点 `origin/main@d0f1a766` 已由 `0e4bf610` 普通合入；race-system 的 proto/schema/client 请求链变更未改动本修复 `client/social` 行为，PR 目标差异仍只有 client 邀请生命周期 + 本 plan 证据变更。

### 遗留 / 后续

- `docs/plans-skeleton/plan-bughunt-v-sparring-invite-screen-hijack-v1.md` 是同 bug 的重复 skeleton，留给主干在本 PR 合并后按锁运维/去重流程处理；本 PR 遵守一个 plan 一个 PR，不跨 plan 删除。
- 无头测试通过真实 `SparringInviteScreen.close()`、接受回调与可注入 C2S backend 验证关闭/应战副作用；`MinecraftClient#setScreen` 分支由真实 screen identity + 纯决策矩阵和 runtime switch 对拍锁定。
