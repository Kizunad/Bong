# plan-refactor-client-ui-base-v1 — Client UI 公共基类 + InspectScreen 拆解 + 输入/线程纪律（重构轨 R7）

> 所属总纲：`plan-refactor-master-v1.md`。一句话：给 15 个直接 owo Screen 建公共基类（Screen-local 订阅/tick/关闭清理），把 diff-then-patch、默认键冲突检测、纯 client-thread helper 与礼貌抢屏策略收成共享契约，并以 tab-first 拆掉 4647 行的 `InspectScreen`；P0 仅冻结文档/fixture/contract pin，**ZERO production behavior change**。

## 现状证据（P0 复核，2026-07-30）

- `client/src/main/java/com/bong/client` 下共有 **29 个 production Screen**：15 个直接 `BaseOwoScreen<FlowLayout>`、14 个直接 vanilla `Screen`。文件名口径有陷阱：29 个 `*Screen.java` 中只有 28 个是真 Screen，`TechniqueScrollReadScreen` 是 helper；另有真实 Screen `LegacyAssignPanel.java` 不带 `Screen.java` 后缀。权威逐文件清单在 `client/src/test/resources/bong/ui/r7-screen-inventory.tsv`。
- 原始源码共有 **92 个 `Sizing.fill(100)`** token（20 文件）：87 个 executable + 5 个 comment。context-aware 分类为 82 `LEGAL` + 5 `RISK` + 5 `COMMENT`；若只按几何语义计，79 个 axis-safe、8 个 horizontal main-axis overflow，其中 3 个是源码明确标注且末位挂载的 `TERMINAL_INTENTIONAL` workaround。4 个确定会顶飞后续兄弟节点和 1 个未被接受的末位顺序依赖全部在 `r7-fill100-inventory.tsv` 锁定。
- owo `Sizing.fill(100).inflate(space, ...)` 返回完整 `space`，不是“同轴兄弟拿完后的剩余空间”；horizontal flow 中每个 child 对同一个 child space inflate 后再累计宽度。因此主轴 fill 是否安全取决于容器方向、兄弟顺序和 deliberate terminal placement，不能做纯文本全量替换。
- 生产 `clearChildren()` 当前 16 处；`CraftRecipeListWidget` 已证明 ordered id 相同则原地 patch、id 序列改变才 rebuild。R7 只把需要保留 mounted identity/selection/callback/scroll 的列表迁 `DiffListWidget`，不把一次性面板重建机械改写。
- `InspectScreen.java` 4647 行，是 client 最大 UI 聚合点；它同时持有 tab 组合、订阅 intake、drag/drop、context menu、tooltip、hotbar 和 overlay arbitration，拆分必须保留唯一 screen-level intake 与共享交互 owner。
- 默认键冲突仍在：T/vanilla chat、L/vanilla advancements、O/O、U/U、R/R；垂死大能 G/H/J 的生产默认均为 UNKNOWN，但 HUD 文案硬承诺 `[G]/[H]/[J]`，属于 effective-binding 展示/路由不一致，而不是允许第二个默认 G。
- 网络线程 premise 已变化：`BongNetworkHandler` 的 server-data bridge/router/handler/store/listener 整段已由 R6-owned `clientExecutor.accept(() -> processServerDataPayload(...))` 包住。`cast-sync-config-window-thread` 与 `mineral-probe-result-network-thread-ui` 的原始生产缺陷已由公共边界闭环；R7 不重复接线。
- 抢屏 premise 也已变化：切磋邀请已采用“被别的屏挡住时 toast、保留 domain-store invite、空屏再开”的礼貌逻辑，`v-sparring-invite-screen-hijack` 是已完成 canonical plan 的重复项。顿悟屏仍可强制覆盖普通屏，且 replacement/removal 未汇聚到 exactly-once settlement，是 `ScreenOpenPolicy` 的有效输入。

## 接入面

- **进料**：现有会话态 Store 的 listener/remove-listener API 与 snapshot；R2 只保证断线数据清理。`SessionScopedStore` 当前只有 `clearOnDisconnect()`，**不是订阅接口**，R7 基类不得持有或调用它。
- **出料**：Screen/HUD 展示、用户输入到既有 C2S sender；本轨不改变 wire/schema/Redis key。
- **共享类型**：冻结 `BongScreenBase<R extends ParentComponent>`、`DiffListWidget<T, K, C extends Component>`、`BongKeybindRegistry`、`ClientThreadMarshal`、`ScreenOpenPolicy` 五个 R7 类型；准确签名与 invariant 在 `r7-foundation-contract.tsv`。
- **跨轨接缝**：R2 独占 Store disconnect lifecycle；R6 独占 channel 注册、`ProtoServerDataBridge`、`ServerDataRouter` 与网络 receive-boundary marshal。`ClientThreadMarshal` 只冻结纯 helper API，P0/P1 不把它接进 R6 文件；若后续发现非网络来源需要 helper，R7 仅在自有 Screen/HUD owner 内消费。
- **worldview / qi_physics**：纯 client 基础设施重构，不新增玩法、真元公式、境界、经济或世界观名词；零 qi ledger 变更。

## 阶段总览

- ✅ 2026-07-30 **P0 设计收口 + 吸收清单验真**：29 Screen/92 fill 全量 fixture、五类型 API/策略/默认键目标冻结、R2/R6 边界 pin；仅 docs/tests/resources，ZERO production behavior change。
- ⬜ **P1 基础组件落地**：五个 R7 类型上线；全部 production keybind constructor sites 经 global registry；默认键、vanilla reservation、空 exemption manifest 收口；botany backlog 与 dying-elder effective-binding 显示按 fixture 验收。
- ⬜ **P2 Screen 迁移批次 A**：炼丹/手搓/交易等迁基类；随迁修 fill 风险和 identity-sensitive clearChildren；outgoing trade 在 `TradeOfferIntentHandler`/`TradeOfferScreenBootstrap` 改为显式 picker。
- ⬜ **P3 InspectScreen tab-first 拆解**：shell + tab panel + oversized section leaf，行为不变。
- ⬜ **P4 Screen 迁移批次 B + R7-owned UI thread/open-policy 强制 + 删旧**：验真四个 `client.execute` consumer；Insight settlement 接 `ScreenTransitionController`；`BongHudOrchestrator` 恢复 qi radar main path。
- ⬜ **P5 验收 + 被完整吸收 plan 批量归档**。

## P0 冻结契约

### 1. `BongScreenBase<R extends ParentComponent>`

- 继承 `BaseOwoScreen<R>`，保留 abstract `createAdapter()` 与 `build(R)`；不能硬编码 `Containers::verticalFlow`，因为 `AgentUiScreen` / `DynamicXmlScreen` 使用 `UIModel.createAdapter(...)`。
- 只拥有 **Screen-local listener/unsubscriber** `Runnable`：登记顺序确定、关闭时 LIFO、exactly once；绝不能把 R2 的 `SessionScopedStore.clearOnDisconnect()` 当 Screen teardown。
- `removed()` 先标 closed，再依序执行 business `onRemoved()`、LIFO cleanup、`super.removed()`；即使任何一步抛错，后续阶段仍执行一次。首个异常为 primary，后续异常按执行顺序 `addSuppressed`，最终抛 primary；重复 `removed()` 为 no-op，不吞异常。
- P1 行为 pin：正常 removed、重复 removed、cleanup 抛异常、business hook 抛异常、LIFO、late refresh、XML adapter preservation。

### 2. `DiffListWidget<T, K, C extends Component>`

- `keyOf` 以 constructor-injected `Function<? super T, ? extends K>` 提供，组件保持 `final`；同时注入 row factory 与 idempotent patcher。相同 key + 相同顺序只 patch 已 mounted rows，结构变更才 rebuild。
- equal-key patch 必须保留 component identity、selection/callback 和 scroll（通过“不 clear children”实现，不虚构 owo 0.11.2 没公开的 scroll-offset getter/setter）。
- null list/item/key 和 duplicate key 在 mutation 前 fail-fast。equal-key patch 的 generic `BiConsumer` 不可事务回滚：patch 异常原样抛出，已经执行的 component 外部 mutation 可以保留；但 widget 内部 committed ordered keys/items 只在全部 patch 成功后提交，`renderedKeys()` 失败后仍返回上一 committed sequence，下一次 `update()` 从第一行重试整列，因此 patcher 必须 idempotent。P1 pin empty→items、equal keys、reorder/add/remove、duplicate/null 和 patch failure/partial-mutation/full-retry。

### 3. `BongKeybindRegistry`

- 显式注册、可 grep，无 annotation/reflection discovery；每个 logical binding 以 non-blank、全局唯一的 `BindingOwner.id` 作为 owner identity，`BindingSpec` 与可观察 `Registration` 都携带该 identity；owner identity 与 translation key 均不得重复。物理默认冲突 identity 为 `(InputUtil.Type, defaultCode)`；UNKNOWN/unbound 不参与物理唯一性。`r7-keybind-production-sites.tsv` 冻结当前 26 个 production constructor sites 的逐 binding contract（owner id、source-site、translation、type/code、category、runtime cardinality 与消费路由；含 Combat quick-slot loop 的展开语义），P1 必须全部改经 `BongKeybindRegistry.global().register(...)`，不能只迁 11 个冲突项。
- vanilla reserved defaults 与 deliberate exemptions 必须进入显式 manifest：P0 冻结 `vanilla.chat = KEYSYM+T`、`vanilla.advancements = KEYSYM+L`，且授权 exemption 集为空；未来每条 exemption 都必须是 canonical `BindingOwner.id` pair + exact type/code + non-empty reason，不能用文件路径、translation key 或模糊模块名代替 owner identity。
- P0 的 `r7-keybind-migration.tsv` 同时冻结 current/target `InputUtil.Type`、code、production owner 和 behavior resolution，不改生产键位。P1 目标：保留 identity=KEYSYM+O、forge=KEYSYM+U、spell-volume=KEYSYM+R；将 spirit-treasure T、lingtian L、void-action O、extract-cancel U、botany-auto R 改 UNKNOWN。botany blocked/inactive 路径必须 drain queued presses 并证明稍后不 replay；垂死大能 G/H/J 继续 UNKNOWN，HUD 从 effective binding 生成标签，未绑定明确显示“未绑定”；统一 G router 仍是唯一默认 G owner。

### 4. `ClientThreadMarshal`

- 纯 helper：already client thread 时同步执行一次；off-thread 时 enqueue 一次；注入 predicate/executor 供测试；null/unknown client state fail closed，不得在未知线程 inline。
- **R6 接缝边界**：网络 bridge/router/handler 的公共 receive-boundary 已在 client executor 内，R7 不增加第二层 marshal，不修改 `BongNetworkHandler`、`ServerDataRouter`、`ProtoServerDataBridge` 或 channel 注册。P4 只在 R7-owned `ui/CultivationScreenBootstrap`、`inventory/InspectScreenBootstrap`、`inventory/LootContainerScreenBootstrap`、`insight/InsightOfferScreenBootstrap` 四个现有 `client.execute(...)` 来源中验真并迁移真实 consumer；没有真实 consumer 的 helper 不得以 dead production class 落地。若必须改 R6 owner，先停并协调。

### 5. `ScreenOpenPolicy`

- pure decision layer：输入 raw `Request(kind, identity, expiresAtMs, terminalPriority, alreadyNotified)`、raw `Current(kind, identity, terminalPriority, combatActive)` 与 `nowMs`，policy 自行按非空 identity 相等推导 matching、按 `nowMs >= expiresAtMs` 推导 finite expiry；输出 `OPEN | PREEMPT | NOOP_MATCHING | DEFER_NOTIFY | DEFER_SILENT | BLOCK_DROP | EXPIRE`。policy 不直接 `setScreen()`，不新建第二份 pending offer store；`alreadyNotified` 由现有 domain/bootstrap owner 按 identity 持有，并与 `ScreenTransitionController` 的 pending/current cancellation protocol 组合而非绕过。
- passive social invite：domain Store 保持权威；战斗中或已有屏时，首次同 identity 阻塞 `DEFER_NOTIFY`，已经通知过则 `DEFER_SILENT`；新 identity 重新取得通知资格。战斗结束且 `currentScreen == null`、TTL 仍有效才 `OPEN`；先到 TTL 则 `EXPIRE`。
- ordinary hotkey：无屏 `OPEN`、同屏 `NOOP_MATCHING`、任何 nonmatching ordinary/modal/system-terminal block 都 `BLOCK_DROP`；**物理按键永不排队重放**。
- insight：可 `PREEMPT` 普通 non-modal UI；同 trigger id no-op；在 equal/higher modal 或 death/terminate system terminal 后按 caller-owned 状态 `DEFER_NOTIFY/DEFER_SILENT`；过期 `EXPIRE`。`InsightOfferScreen` 是 settlement owner，以 trigger id identity guard 收敛 `InsightDecision.chosen(triggerId, choiceId)`、`declined(triggerId)`、`timedOut(triggerId)`、ESC/replacement/exceptional removal；replacement 组合 `ScreenTransitionController.CurrentScreenCancellationHandler`，异常 removal 通过 final `BongScreenBase.removed()` 调用 `InsightOfferScreen.onRemoved()` hook。首个 terminal path 在任何可抛的 decision send / screen switch 前原子提交 trigger id + terminal cause；随后 send，再 close/switch，send 失败仍尝试 transition；transition 失败不回滚 winner；两者都失败时首个异常为 primary、后者按执行顺序 suppressed；后续 terminal path 一律 NOOP。完整冻结于 `r7-insight-settlement.tsv`，每个 cause 的 settlement/owner/identity/order/failure/observable effect 逐行 exact pin。
- system terminal：同 identity `NOOP_MATCHING`；`TerminateScreen > DeathScreen > ordinary/modal`，可抢占低优先 UI；非 matching equal-priority peer 与更高优先 terminal 都 `BLOCK_DROP`。finite expiry 先于 identity matching 判定（expiry 与 matching 同时为真仍 `EXPIRE`）；P0 raw decision vectors 在 `r7-screen-open-policy.tsv`，30 行全部字段逐行 exact pin。

## InspectScreen P3 决议

采用 **tab-first** top-level extraction，在超大 tab 内再按 section 拆 leaf；不按任意行数切碎，也不与 R10 server inventory 拆分同窗口进行。

- `InspectScreen` shell 保留 root composition、唯一一次 screen-level snapshot/subscription intake、input/render routing，以及 drag/drop、context-menu、tooltip、hotbar、overlay arbitration。
- tab panels 至少覆盖 equipment、cultivation、skills、现有 techniques panel 与 craft entry；tab 内 section 只接 immutable snapshot/view-model + intent callback，不直接再订阅同一个 Store。
- body/container/tooltip 等已有组件作为 leaf 继续复用；P3 契约 pin shell 只有一个 authoritative intake、tab switch 不重复 listener、drag/tooltip/context overlay 跨 tab 行为不变。
- R10 只改 server inventory core；R7 不等待或联动其内部文件重排，双方只守现有 wire/view-model 契约。

## 吸收清单验真（2026-07-30）

| finding | P0 verdict | R7 边界 / 证据 |
|---|---|---|
| spirit-treasure-chat-key-conflict | still-valid | T 撞 vanilla chat；P1 改 UNKNOWN，并纳入 registry。 |
| alchemy-screen-fill-overflow | still-valid, canonical | 4 个 definite sibling eviction + 1 terminal order risk；P2 随迁修。 |
| alchemy-screen-fill100-eviction | duplicate | 与上一 canonical 同根同点，不建第二 implementation owner。 |
| techniques-tab-scroll-bounce | still-valid | `TechniquesTabPanel` identity-sensitive refresh 仍 clear children；迁 DiffListWidget。 |
| botany-rkey-backlog-dispatch | still-valid | R/R 虽有局部仲裁，botany backlog/stale snapshot 仍有效；P1 收口。 |
| client-input-keybind-collision | still-valid | O/O、U/U 无统一注册门；P1 收口。 |
| dying-elder-give-dan-input | still-valid (client slice) | G/H/J 默认 UNKNOWN 与 HUD 硬标签不一致；不创建第二默认 G。 |
| lingtian-advancements-key-conflict | still-valid | L 撞 vanilla advancements；P1 改 UNKNOWN。 |
| trade-offer-first-item-autopick | still-valid (client picker slice) | outgoing trade 仍自动选第一件；R7 后续改显式 picker。 |
| hud-qi-radar-mainpath-regression | still-valid | `BongHudOrchestrator` production planner 调用仍被注释；R7 HUD slice。 |
| client-insight-offer-strand | still-valid (client modal slice) | 强制覆盖 + replacement/removal settlement 缺口；纳 ScreenOpenPolicy。 |
| cast-sync-config-window-thread | already-fixed | R6 common receive boundary 已整体 client-thread marshal。 |
| mineral-probe-result-network-thread-ui | already-fixed | 同一 R6 boundary 已覆盖；R7 不重复 handler 接线。 |
| surface-stash-search-hud-label-gap | already-fixed | `TsyContainerView` 已有 `surface_stash -> 散修遗缴`。 |
| v-sparring-invite-screen-hijack | duplicate/already-fixed | canonical 修复已用 deferred domain-store + one-shot toast；仅抽通用 policy。 |
| preview-config-dead-server | out-of-track | Gradle/preview harness tooling，不属 Screen UI-base。 |
| weather-visual-overlay-collapse | out-of-track | environment/VFX emitter identity，不属 Screen UI-base。 |

只有 `still-valid` 的 R7 slice 可在 P1-P5 修；duplicate/already-fixed 不重复改，out-of-track 保留其独立 owner。

## 文件所有权与边界

- **R7 独占**：client Screen/`ui/`/HUD 结构性改动、keybind 注册、`InspectScreen.java`；本 P0 实际只改本 plan、`client/src/test/java/com/bong/client/ui/R7*` 与 `client/src/test/resources/bong/ui/r7-*`。
- **R2 独占且不碰**：`client/lifecycle/**`、Store clear/registry、`clearClientStateOnDisconnect` 区段及 R2 gate tests。R7 只消费各 Store 已有 listener/snapshot API。
- **R6 独占且不碰**：`BongNetworkHandler` channel/receive dispatch、`network/` bridge/router/handler integration。R7 不因 helper freeze 取得接线权。
- **server 全部不碰**；无 wire/schema/Redis 变更。

## 验收

- P0 pin：`R7InventoryContractTest` 对拍 29 Screen、92 fill lexical inventory、owo inflate 语义和 production-zero-change；`R7FoundationContractTest` 对拍五类型签名、keybind target、ScreenOpenPolicy vectors、plan anchors、R2/R6 ownership。
- P1 behavior gate：`BongKeybindRegistryTest` 覆盖 translation/physical duplicate、vanilla reservation、空/精确 exemption、UNKNOWN 非冲突与 registrations immutable/order；production-site source gate 确认 26 个 constructor sites 全部迁 global registry；`BotanyHudBootstrapTest` 覆盖 blocked/inactive drain 后不 replay；dying-elder HUD 测试覆盖 rebound key 与“未绑定”。
- P2 trade gate：`TradeOfferIntentHandlerTest` + picker test 必须证明多 item 时只有 explicit selection 的 exact `instance_id` 被 dispatch；grid/hotbar/sort 不得替用户决定；无 selection 拒绝 dispatch 或打开 picker；P5 e2e 对拍 target 收到相同 instance/displayName。
- P4 thread/open/HUD gate：四个命名 `client.execute` 来源逐个给出迁移或“不需要 helper”的证据；`InsightOfferScreenTest` 覆盖所有 `r7-insight-settlement.tsv` terminal causes；`BongHudOrchestratorTest` 必须证明凝脉及以上 main path 产出 `HudRenderLayer.QI_RADAR`、低境界隐藏，并经 main path 命中 negative-qi、TSY false-signal、nearby-cultivator markers。
- P5 汇总 gate：上述 keybind/trade/insight/radar acceptance 全部在 Java 17 `test build` 与 UI C2S/e2e 可达证据中闭环后，才可归档对应 absorbed findings。
- 完整 client gate：Java 17 下 `flock /tmp/bong-gradle.lock -c "cd client && ./gradlew test build"`；人工 `./gradlew runClient` 验五大屏留到实际生产迁移 PR。
- bot 配合：`ui_c2s_smoke` 只证明各屏原 C2S 动作链路仍可达；bot 无法替代 client UI contract tests。

## §8 开放问题（P0 决策门前需收口）

1. InspectScreen 拆解粒度（按 tab 还是按 section）；拆解与 R10 server 侧 inventory 拆分是否同窗口进行。
2. ScreenOpenPolicy 的排队语义（战斗中挂起邀请到何时弹出）。

全部已在 §8.1 收口。原问题保留以备追溯，实施时以 §8.1 决议为准。

## §8.1 决议（pre-P0 收口，2026-07-30）

### #1 InspectScreen 采用 tab-first，和 R10 解耦

**决议**：top-level 按 tab 拆，超大 tab 内再拆 section leaf；shell 保留唯一 store intake 和跨 tab 交互 arbitration。R7 与 R10 不在同窗口重构，只守既有 view-model/wire 边界。

**落点**：`client/src/main/java/com/bong/client/inventory/InspectScreen.java:57`；本 plan §InspectScreen P3 决议；`client/src/test/resources/bong/ui/r7-screen-inventory.tsv`。

### #2 只 deferred passive offer，不 replay physical hotkey

**决议**：被 combat/屏幕挡住的 passive social offer 保留在既有 domain Store；bootstrap 按 identity 持有 `alreadyNotified`，首次阻塞 `DEFER_NOTIFY`，重复同 identity `DEFER_SILENT`，新 identity 恢复通知资格；空屏且未过 TTL 时打开。普通 hotkey 被任意 nonmatching screen 挡住即 drop。Insight 可抢普通 UI，但在 equal/higher modal 与 system terminal 后 defer；`InsightOfferScreen` + `CurrentScreenCancellationHandler` 以 trigger id 将所有 terminal path 收敛为 exactly-once settlement。

**落点**：`client/src/main/java/com/bong/client/social/SparringInviteScreenBootstrap.java:42-57`；`client/src/main/java/com/bong/client/insight/InsightOfferScreenBootstrap.java:35-53`；本 plan §ScreenOpenPolicy；`client/src/test/resources/bong/ui/r7-screen-open-policy.tsv`。

## §10 实施工作流

### §10.1 适用边界

纯 client 逻辑重构，无 NBT/layout/model/texture，不适用视觉资产三轮与 `<PROMISE>`。每个 PR 使用中文 atomic commit，带真实 `Model:` trailer；任何生产迁移必须以 P0 fixture/contract 为基线，不能顺便越界改 R2/R6/server。

### §10.2 多 PR 依赖顺序

1. **PR-1 / P0 contract freeze**：docs + R7 tests/resources only，ZERO production behavior change。
2. **PR-2 / P1 foundations + keybind**：五类型；所有 production KeyBinding sites 迁 global registry；vanilla reserved/空 exemption、botany backlog、dying-elder effective binding 全部按 P1 gate 收口；不接 R6 network files。
3. **PR-3 / P2 migration A**：alchemy/craft/trade 等，随迁 fill/list defects；outgoing trade 显式 picker 必须通过 exact instance-id gate。
4. **PR-4 / P3 Inspect split**：tab-first shell/panels，行为不变。
5. **PR-5 / P4 migration B + R7-owned enforcement**：只验真四个命名 UI `client.execute` 来源；Insight settlement 接 transition cancellation；qi radar 恢复 `BongHudOrchestrator` main path。
6. **PR-6 / P5 acceptance + absorbed-plan archive**：keybind/trade/insight/radar 四组 acceptance 绿后才归档。

前一 PR 的最终 HEAD 未通过 Java 17 gate、fresh-context SHA validator、`/review`、e2e 与 CodeRabbit 并 merge 前，不提前实施下一阶段。

### §10.3 每个 PR 的闭环门

1. 独立锁定 worktree/branch，不改脏 main checkout。
2. push 前 `git fetch origin` 后紧邻 `git merge origin/main`；HEAD 变化即重验。
3. Java 17 串行执行 global-lock client gate。
4. 对最终 SHA 启动 explicit-worktree、read-only fresh-context validator；任何 HEAD 变化使旧 PASS 失效。
5. push 后确认 PR head 等于已验证 SHA，独立评论 `/review`；review 修复产生新 HEAD 时重跑全部门并重新评论。
6. orchestrator owns merge；实施 agent 不 merge。

### §10.4 单次 consume-plan 全自动到 merge

后续完整 `/consume-plan plan-refactor-client-ui-base-v1` 串行消费 P1-P5；每阶段保持本 plan 的 R2/R6 ownership boundary。除产品方向或不可逆操作外不中途回问；终态补 `## Finish Evidence` 并迁入 `docs/finished_plans/`。
