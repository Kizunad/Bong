# plan-refactor-client-store-lifecycle-v1 — Client 状态 Store 统一断线生命周期（重构轨 R2）

> 所属总纲：`plan-refactor-master-v1.md`。一句话：把 client 侧 108 个各自为政的静态 Store 收敛到统一 `SessionScopedStore` 契约 + 显式静态登记清理；P0 仅建立分类、框架与分阶段门禁，待 P3 全量登记、P4 重连验收完成后，系统性阻止“断线残留/跨会话串味”这一整类 bug（现存 14 份断线核心）。

## 现状证据（2026-07-27 侦察）

- 全仓生产源码 `*Store.java` 共 108 个：106 个 session-scoped、1 个 persistent-config（`HudLayoutPreferenceStore`）、1 个 constant（`ArmorProfileStore`）。现有 `BongNetworkHandler.clearClientStateOnDisconnect()`（`BongNetworkHandler.java:1119-1200`）直接清 25 个 Store，另有一批 Store 由分散 bootstrap / controller 拥有或完全漏清；helper 还混有 renderer / handler / audio 等非 Store hook。
- 裸奔实例：`processing/state/FreshnessStore.java`（只有 `clearForTests`）、`combat/store/` 一整排（Wounds/Vortex/Terminate/DeathState/DuguPoison/Tribulation/AscensionQuota）只有 `resetForTests()`，且被 `BongHudOrchestrator` 直接消费。
- 清理动作方法名三种混用（`clearOnDisconnect`/`clear`/`reset`），无共同接口。
- 会话状态机本身是干净的：`ui/ClientConnectionStatusStore`（initialize/activate/invalidateSession）——问题只在"业务 store 没挂上这个生命周期"。
- 既有守护测试 `BongNetworkHandlerTest.java:560-575` 用源码字符串扫描断言 DISCONNECT 路由，可扩展为全量强制。

## 接入面

- **进料**：`ClientPlayConnectionEvents.DISCONNECT`（既有唯一权威入口）、`ClientConnectionStatusStore` 会话状态机。
- **出料**：所有 HUD planner / Screen / tooltip 读到的 store 快照保证是本会话的。
- **共享类型**：新 `SessionScopedStore` 接口（单一 `clearOnDisconnect()` 生产语义）+ 显式静态 adapter registry / FQCN manifest；Store 继续保留既有静态业务 API，registry 不依赖构造器实例化、运行时反射或注解扫描。
- **跨仓库契约**：通常零 wire 改动；craft 例外消费 master M-09 的 A-06/A-08 handler contract。`CraftStore` 持有当前 accepted `session_key`、该 identity 的 highest generation/`phase_revision`、当前 `open_request_id` 与 matching `OpenPending`；普通 A-06 accepted hydration 必须回显该 `open_request_id`，并携带 `session_transition=Initial | Rollover { previous_session_key }`。断线后的 S-10 guarded restore 不依赖已清除的 request/latch：server 是 reconnect guard 的唯一 producer。S-07 durable checkpoint 成功时，R1 生成一次性 `ReconnectGuard { owner_key, session_key, generation, phase_revision, restore_token }`，R3 将它与 Suspended checkpoint 同事务持久化；新连接时 R3 先加载该 guard，R1/R6 按同一有序 server-data stream 先发送独立 `CraftRestoreGuard { owner_key, session_key, generation, phase_revision, restore_token }` control frame，再发送独立 A-06 `Restore { restore_token }` variant 的 Paused projection。client bridge 只把独立 frame 交给 `CraftStore.armReconnectGuard(...)`，不生成 token；CraftStore 只在当前连接已 arm 的 guard、owner、session identity、generation 与 `phase_revision` 全部对拍时接受 Restore，成功后立即消费该 guard。R1 S-10 同时校验并消费 server-side guard；Restore 不依赖已清除的 `open_request_id`/`OpenPending`，旧 guard 不能跨连接或重放。仅 `Initial` 作用于无 session 的 Idle/OpenPending；只有 previous key 等于当前 key 的 `Rollover` 才授权新 `session_key` 替换旧 identity。

  CraftStore 的 authoritative phase acceptance 明确按下表执行；所有同 identity/generation 的更新都要求 `phase_revision > current_phase_revision`，较低或相等 revision 一律 no-op，不能用 phase 名称判断新旧：

  | 当前 accepted phase | 更高 revision 的 incoming phase | 结果 |
  |---|---|---|
  | `Running` | `Running` / `Paused` / `HandoffPreparing` / `Ended` | 接受；`Paused` 是权威 pause，后续更高 revision 的 `Running` 仍可按 S-19 接受 |
  | `Paused` | `Paused` / `Running` / `HandoffPreparing` / `Ended` | 接受；`Running` 是 matching Resume 后的合法单调 successor，不得被旧的“Paused 禁止被 Running 覆盖”规则拦截 |
  | `HandoffPreparing` | `HandoffPreparing` / `Ended` | 接受终态推进；`Running`/`Paused`/`Suspended` no-op |
  | `Ended` | `Ended` | 仅接受更高 revision 的终态重放；其他 phase no-op |
  | disconnect 后 `Empty` + armed guard | 仅 `Restore { restore_token }` + `Paused` 且 token/binding/revision 与 guard 完全匹配 | 接受为本连接首个 craft projection；`Initial`/`Rollover`/普通 phase no-op，不发送普通 Pause、不重开第二个 Screen |

  对 `Suspended` 只走上行 server-side S-10 与下发的 guarded `Restore`; client 不把未经 guard 的 `Suspended` 或 `Running` 当作 reconnect hydration。较旧 generation、未授权 key 变化、同 generation 且 `phase_revision <= current` 的 snapshot 均 no-op。仅同 request_id 的 A-08 清 pending 并记录 reason；不可关联 parse rejection 不触碰 pending，stale/mismatched rejection/hydration no-op；disconnect 原子清空全部 craft state 与旧 reconnect guard。loop 音效清理对齐 `feedback_audio_loop_lifecycle`（硬停连延迟层一起哑）。
- **worldview 锚点**：`docs/worldview.md §一 L17-L26` 的“匮乏、信息差、搜打撤”要求跨会话观察必须可信；Store 生命周期属于 client 基础设施，不新增玩法、境界、经济或真元物理规则。

## 阶段

- ✅ 2026-07-27 **P0 设计收口 + 吸收清单验真**：以 FQCN manifest 对 108 个生产 `*Store.java` 做会话态 / 持久配置态 / 纯常量表分类；冻结 `SessionScopedStore`、显式静态 adapter registry 与源码扫描强制，不使用构造器自注册或运行时反射。P0 只落框架、分类和门禁，不改变既有 Store 的断线行为。
  - **模块 / symbol**：`client/src/main/java/com/bong/client/lifecycle/{ClientStoreScopeManifest,SessionScopedStore,SessionStoreHandle,SessionScopedStoreRegistry}.java`；`SessionScopedStore.clearOnDisconnect()`；`SessionStoreHandle.forStore(...)`；`SessionScopedStoreRegistry.clearAllOnDisconnect()`。
  - **测试抓手**：`ClientStoreScopeManifestTest` 精确 pin 108 个 Store 的三分类、106 个 session Store、`ClientConnectionStatusStore` 外部 token 管理与 P0 空 registry；`SessionScopedStoreRegistryTest` pin 声明顺序、重复 FQCN fail-fast、Store / reporter 异常隔离和 `Error` 透传。
  - **跨仓库契约**：纯 client 生命周期基础设施；schema、Redis key、CustomPayload 均无新增或变更。
- ⬜ **P1 在册 Store 平移**：把当前 `clearClientStateOnDisconnect()` 中已存在的 Store 清理逐项迁入 registry；断线 helper 改为调用 registry 一次。非 Store 的 renderer / handler / ambience 等生命周期 hook 继续由 helper 显式拥有，P1 不借重构删除它们。R1 craft 接缝以本阶段已登记的 `CraftStore` 为唯一 client session-state owner：M-09 handler 维护 highest accepted generation/current open request，matching A-08 清 `OpenPending` 并允许重试，duplicate/stale/mismatched state/rejection no-op，disconnect 清 request/session/latch 与已 arm 的 reconnect guard；R6/M-09 的 server bridge 在 Restore 前先消费独立 `CraftRestoreGuard` control frame，调用 `CraftStore.armReconnectGuard(...)`，不得由 client store 生成或猜测 token；R7 P2 只消费该 Store 实现 open/start/close/pause/cancel/resume，不再建立第二份状态。
  - **CraftStore state acceptance pin**：同 `session_key + generation` 只接受 `phase_revision > current_phase_revision`；`Running→Paused`、`Paused→Running`（S-19 Resume）、`Paused→HandoffPreparing`、`HandoffPreparing→Ended` 均可按更高 revision 接受，Running/Paused 不因 phase 名称互相覆盖而被拒绝；低/等 revision、旧 generation、未授权 key 变化均 no-op。`Empty + armed guard` 只接受 token/binding/revision 全匹配的 `Restore { phase=Paused }`，成功后消费 guard；`Initial`/`Rollover` 不得绕过断线后的 guard，`Running`/`Ended`/`HandoffPreparing` Restore 均 no-op。

  - **模块 / symbol**：`client/src/main/java/com/bong/client/lifecycle/SessionScopedStoreRegistry.java` 的显式 `REGISTERED`；`client/src/main/java/com/bong/client/BongNetworkHandler.java` 的 `disconnectSession(...)` / `clearClientStateOnDisconnect()`。
  - **测试抓手**：`ClientStoreScopeManifestTest` 精确 pin P1 已迁移 FQCN 集；`BongNetworkHandlerTest` pin token invalidation 先于 registry、迟到旧 handler 不清新 session、非 Store hook 仍保留；每个 adapter 以目标 Store 的状态级行为测试证明 method reference 未错绑。CraftStore 另锁 `OpaqueId` 的空串/空白/Unicode/超长与 `U64DecimalString` 的负数/小数/科学计数法/前导零/overflow 反例，以及 generation 0/1/max 单调、同 generation identity mismatch、matching A-08 clear/retry、stale/mismatched A-08 no-op、disconnect 后迟到 state/rejection 不复活旧 pending；正向 rollover 另以 authoritative `session_key=K2` 替换既有 `K1`，断言新 identity 被接受、generation floor 从 K2 的起点重建，旧 K1 state/rejection 不再写回；S-10 restore 另覆盖 disconnect 清除 `OpenPending` 后仅凭 matching `restore_token` + reconnect guard + owner/session identity/generation + strictly higher `phase_revision` 接受 server-authoritative `Paused`，missing/wrong/stale token、owner/key/generation/revision mismatch、Running/Ended/HandoffPreparing projection 均 no-op，且不得发送普通 Pause 或重开第二个 Screen。
  - **跨仓库契约**：非 craft Store 保持现有 wire、schema、Redis key 与 CustomPayload 不变；CraftStore 只消费 M-09 已冻结的 A-06/A-08 router 输出，不修改 schema/converter。
- ⬜ **P2 裸奔状态收编**：按 P0 manifest 显式接入 Freshness、`combat/store/`、炼丹 / 锻造 / 灵田 / 灵宝 / TSY / 医道等会话态 Store、玩家动画层缓存，并为灵田等循环音效提供“活动实例 + 延迟层 + 派生 flag”同边界硬停。
  - **模块 / symbol**：各目标 `*Store.clearOnDisconnect()`；`processing/state/FreshnessStore.java`；`combat/store/*.java`；`animation/BongAnimationPlayer.java`；generic audio / animation adjunct lifecycle handle（不得计入 108 Store manifest）。
  - **测试抓手**：逐 Store pin data-only production clear 不删除 listener / dispatcher / built-in registry / test seam；动画 pin 旧 layer 不跨重连；循环音频 pin active instance、pending layer 与派生 flag 同时归零；`ClientStoreScopeManifestTest` 精确 pin P2 累积登记集。
  - **跨仓库契约**：零 schema / Redis / wire 变更；既有 server 首包仍是重灌来源。
- ⬜ **P3 全量强制 + 删旧**：统一**生产生命周期入口**为 `clearOnDisconnect()`（不把清 listener / test seam 的 `resetForTests` 当生产清理）；删除手工 Store 清单；源码扫描强制“生产 `*Store.java` 必须恰好有一种分类，session 类必须以强类型 handle 登记”，新增漏分类或漏登记即红。
  - **模块 / symbol**：`ClientStoreScopeManifest.registryManagedSessionStores()`；`SessionScopedStoreRegistry.registeredFqcnsForTests()`；`BongNetworkHandler.clearClientStateOnDisconnect()` 仅保留一次 registry 调用与非 Store hooks。
  - **测试抓手**：source-scan 精确断言 registry-managed session FQCN 集等于 registry 集；生产源码不得从断线路径调用 `resetForTests` / `resetForTest` / `clearForTests`；新增 Store 未分类、session Store 未登记、重复登记均失败。
  - **跨仓库契约**：仍为 client-only；schema、Redis key、CustomPayload 均不变。
- ⬜ **P4 断线 / 重连验收 + 归档**：client 契约 pin 覆盖旧 handler 迟到断线、旧 session 排队 payload、清空后新首包重灌；bot `reconnect_state_freshness` 验首包集合。只批量归档被 R2 完整修复的 plan；部分吸收项保留其 UI hydration / freshness gate 等独立工作。
  - **模块 / symbol**：`ClientConnectionStatusStore.invalidateSession(...)`；`BongNetworkHandler.disconnectSession(...)`；`scripts/bot/scenarios/reconnect_state_freshness.*`；被完全吸收 plan 的 `## Finish Evidence`。
  - **测试抓手**：Java 17 `./gradlew test build`；断线→旧 payload 到达→重连→新首包重灌的 client 契约测试；bot e2e 精确验完整首包集合；source-scan 终态全集门禁。
  - **跨仓库契约**：只复用既有 join 首包 CustomPayload symbol，不新增 schema / Redis key；bot 验收 server 重发与 client 新 session 接收的现有契约。

## §8 开放问题（P0 决策门前需收口）

1. 注册机制采用显式静态 adapter registry，还是构造器自注册 / 运行时发现？
2. 生产 `clearOnDisconnect()` 与 test reset 是否共用语义？
3. 108 个 Store 如何分类，`ClientConnectionStatusStore` 是否进入无参 registry，强制门在哪一阶段收紧？
4. 删除手工 Store 清单后，非 Store renderer / handler / audio hook 由谁拥有？
5. 14 份断线核心及 audio follow-up 哪些被 R2 完整吸收，哪些只吸收生命周期切片？

全部已在 §8.1 收口。原问题保留以备追溯，实施时以 §8.1 决议为准。

## §8.1 决议（pre-P0 收口，2026-07-27）

### #1 注册机制：显式静态 adapter registry

**决议**：
1. `SessionScopedStore` 只定义生产语义 `void clearOnDisconnect()`；中央 registry 保存显式、强类型 adapter（由目标 Store 的 `Class<?>` 派生 FQCN 身份，并绑定 method reference / 命名 handle），不要求现有静态 utility Store 被实例化。调用方不得手填与 clearer 相互独立的身份字符串；P1/P2 接入每个实际 adapter 时另以 Store 行为 pin 证明清的是对应 Store。
2. 禁止构造器自注册、运行时反射和注解扫描。多数 Store 是 private constructor + static state，构造器路径并不会被业务触发；显式 manifest 才能稳定 grep、review 和源码扫描。
3. `BongNetworkHandler.disconnectSession(...)` 的原子边界不变：先 `ClientConnectionStatusStore.invalidateSession(handler, disconnectedAtMs)`，仅 active token 失活成功才在同一 client-thread task 清 registry。旧 handler 的迟到 DISCONNECT 不得清新 session。

**落点**：`client/src/main/java/com/bong/client/BongNetworkHandler.java:305-315`；`client/src/main/java/com/bong/client/ui/ClientConnectionStatusStore.java:129-153`；plan §P0 / §P1 / §P3。

### #2 生产清理与测试 reset 分离

**决议**：
1. `clearOnDisconnect()` 只清 session payload、entity/world 引用、活动交互、派生 HUD/VFX/audio/animation 状态；必须保留长期 listener、dispatcher wiring、built-in registry 和 test seam。
2. `resetForTests()` / `resetForTest()` / `clearForTests()` 继续只服务测试隔离，可额外清 listener、sequence 或替换 seam；生产代码不得调用它们。
3. 现有生产路径调用测试 reset 的 Store 在 P2 增加 data-only production clear，再迁入 registry；不以重命名掩盖语义差异。

**落点**：`client/src/main/java/com/bong/client/alchemy/state/AlchemySessionStore.java:42-68`；`client/src/main/java/com/bong/client/combat/QuickUseSlotStore.java:68-75`；`client/src/main/java/com/bong/client/combat/SkillBarStore.java:57-61`；`client/src/main/java/com/bong/client/animation/BongAnimationPlayer.java:343-351`；plan §P2 / §P3。

### #3 分类与强制边界

**决议**：
1. P0 manifest 以 production source root 下的 FQCN 为唯一键，覆盖 P0 基线的全部 108 个业务 `*Store.java`；每个业务文件必须恰好落入 session-scoped、persistent-config、constant 三类之一。新增的 `SessionScopedStore.java` 接口虽匹配文件后缀，但作为 lifecycle infrastructure 由 source guard 单独显式排除，不能被误计为第 109 个业务 Store；registry / catalog 类自身不以 `*Store.java` 命名。
2. 当前明确例外是 `HudLayoutPreferenceStore`（persistent-config，断线保留用户 HUD 偏好）和 `ArmorProfileStore`（constant，固定护甲 profile 表）。106 个 session-scoped Store 中，`ClientConnectionStatusStore` 是 token-gated 连接状态机：它由 `invalidateSession(handler, disconnectedAtMs)` 在 registry 之前按 handler 精确失活，不得再被无参全局 clear；其余业务 Store 必须在 P3 前全部有生产 clear 并登记。
3. 测试以 `Files.walk` 扫描 `client/src/main/java/com/bong/client`，用排序后的相对路径 / FQCN 比较“发现集 = manifest 三类并集”，并断言分类互斥；登记强制分阶段启用：P0 明确 pin registry 为空且登记项只能属于 registry-managed session 集，P1/P2 对迁移清单做精确阶段 pin，P3 再断言“registry-managed session 集 = registry 集”。不只硬编码数量 108，也不扫描 test / gametest / build-generated。

**落点**：`client/src/main/java/com/bong/client/hud/HudLayoutPreferenceStore.java:15-51`；`client/src/main/java/com/bong/client/combat/ArmorProfileStore.java:45-86`；`client/src/test/java/com/bong/client/BongNetworkHandlerTest.java:560-575`；plan §P0 / §P3。

### #4 手工 helper 与非 Store 生命周期职责

**决议**：
1. P1 只把 `clearClientStateOnDisconnect()` 现有 Store 调用平移到 registry，并用 behavior pin 证明行为不变。
2. `NpcDialogueBubbleRenderer`、disguise handlers、`MusicStateMachine`、`MutationVisualState`、`EraAmbianceState`、`BongToast` 等非 `*Store.java` 清理不得因删除手工 Store 清单而消失；它们继续留在 token-gated helper，直到各自所属轨道另有明确生命周期 owner。
3. P2 新增的动画 / generic audio lifecycle handle 可登记为 session resource，但不能伪装成 108 Store 分类项；Store manifest 和 adjunct resource registry 必须可区分。

**落点**：`client/src/main/java/com/bong/client/BongNetworkHandler.java:1119-1200`；plan §P1 / §P2 / §P3。

### #5 吸收清单边界

**决议**：
1. R2 可覆盖 14 份计划的断线残留核心；炼丹、锻造、灵宝仅部分吸收，仍需独立处理 Screen 关闭 / 权威 hydration / current-session freshness gate。
2. `ambient-zone-audio-stale-anchor` 不由 R2 吸收：它是同一在线 session 内位置 anchor / 去重键不刷新。
3. `zone-environment-audio-loop-fallback` 不由 R2 吸收：它是 `EnvironmentAudioController.soundFor()` 漏映射。R2 只保证现有 / 新增 loop 的断线硬停，不替代映射数据修复。

**落点**：`client/src/main/java/com/bong/client/audio/MusicStateMachine.java:214-236`；`client/src/main/java/com/bong/client/audio/SoundRecipePlayer.java:76-123`；`client/src/main/java/com/bong/client/environment/EnvironmentAudioController.java:57-63,117-124`；plan §P2 / §P4 与 §吸收清单。

原开放问题全部以 §8.1 决议为准；实施阶段不得重新引入构造器自注册、生产 test reset 或反射扫描。

## 吸收清单（active 13 + skeleton 若干；短名省略 plan-bughunt- 前缀与 -v1 后缀）

active：alchemy-ui-session-stale、breakthrough-billboard-session-leak、client-freshness-store-session-stale、forge-ui-session-stale、full-power-charging-session-bleed、lingtian-session-disconnect-ui、perception-edge-session-leak、playeranim-reconnect-stale-layer、poison-trait-hud-disconnect、spirittreasure-session-leak、tsy-extract-disconnect-stale、woliu-vortex-disconnect-residue、yidao-hud-disconnect-bleed。
skeleton：niche-guardian-cross-session-leak。

不吸收：`ambient-zone-audio-stale-anchor`（在线 session 内 anchor / 去重键刷新问题）、`zone-environment-audio-loop-fallback`（recipe id 映射缺口）。R2 只为 generic loop / pending layer 提供断线硬停契约。

## 文件所有权与边界

- 独占：全部 `*Store.java` 的生命周期接口与登记、`BongNetworkHandler.java` 的 `clearClientStateOnDisconnect` 区段。
- 不碰：`BongNetworkHandler.register()` 的 channel 注册区（R6 域，同文件分区段协作，两轨 merge 前互相 fetch）；Screen 结构（R7 域）；除 master M-09 明确登记的 `CraftStore` freshness/request/latch 外的 store 业务字段语义。
- 依赖：Wave/start/order/cutover 只引用 master §3/§4.1 与 PR 1902；R7/R9 只消费本轨冻结的 Store interface，不在 R2 复制跨轨箭头。

## bot 验收场景

bot 是协议级客户端，测不了 client 内存——本轨主验收是 client 单测（注册表强制扫描 + 每类 store 断线清理 pin 测试），bot 侧配合场景：
1. `reconnect_state_freshness`：bot 断线重连后 server 重发的首包快照集完整（联动 R6 join 首包契约），保证"清干净之后能重新灌满"。

## §10 实施工作流

### §10.1 适用边界

本 plan 是纯 client 生命周期逻辑重构，不产出 NBT、worldgen layout、模型或贴图，因此不适用视觉资产 3 轮打磨与 `<PROMISE>`。每个逻辑单元必须使用中文 atomic commit，agent 产生的每个 commit 都必须包含真实执行模型 ID 的 `Model:` trailer；每个 PR 仍须饱和行为测试和精确最终 HEAD 证据。

### §10.2 多 PR 依赖顺序

1. **PR-1 / P0 框架与分类**：冻结 108 Store manifest、`SessionScopedStore`、强类型 handle、空 registry 与 source-scan 门禁；不改变生产断线行为。
2. **PR-2 / P1 在册平移**：仅在 PR-1 merge 后开始，将现有 25 个 Store 清理行为不变地迁入 registry，并保留全部非 Store hooks。
3. **PR-3 / P2 裸 Store 与 adjunct resource**：仅在 PR-2 merge 后开始，补齐 data-only production clear、动画缓存与循环音频硬停。
4. **PR-4 / P3 全集强制**：仅在 PR-3 merge 后开始，删除手工 Store 清单并将 source-scan 收紧为 registry-managed session 集与 registry 集精确相等。
5. **PR-5 / P4 重连验收与归档**：仅在 PR-4 merge 后开始，完成旧 handler / 迟到 payload / 新首包重灌与 bot e2e；只归档被完整吸收的计划。

前一 PR 的最终 HEAD 未通过 Java 17 gate、fresh-context SHA validator、`/review`、e2e 与 CodeRabbit 并 merge 前，不得提前实施下一阶段。

### §10.3 每个 PR 的闭环门

1. 在独立、锁定的 worktree / branch 中实施本阶段，不修改脏 main checkout，也不越界触碰 `BongNetworkHandler.register()`、`ProtoServerDataBridge`、`ServerDataRouter` 或 Screen 结构。
2. `git fetch origin` 后紧邻 `git merge origin/main`；merge 带入任何变更即对新 HEAD 重跑本阶段全部验证。
3. 使用 Java 17 串行执行 `flock /tmp/bong-gradle.lock -c "cd client && ./gradlew test build"`，不得以定向测试替代完整 gate。
4. 对最终完整 SHA 启动 fresh-context、read-only 对抗 validator；任何后续 HEAD 变化都使旧 PASS 失效。
5. push 后确认 PR head 精确等于已验证 SHA，独立评论 `/review`，等待并通过该 HEAD 对应的 e2e 与 CodeRabbit；任何根据 review finding 产生的新 HEAD 都必须从步骤 2 重跑，并重新独立评论 `/review`、等待该新 HEAD 的 e2e 与 CodeRabbit re-review，旧 HEAD 的通过证据不得复用。blocker / major 必须返工直到新 HEAD 收敛。
6. review 收敛并 merge 后才释放下一阶段；归档 PR 追加 `## Finish Evidence`，包含阶段落点、commit、测试结果、跨仓库核验及遗留项。

### §10.4 单次 consume-plan 全自动到 merge

用户提交一次 `/consume-plan plan-refactor-client-store-lifecycle-v1` 后，调度方按 §10.2 串行完成 PR-1 至 PR-5：每个 PR 使用独立实施 / 返工上下文，自动执行 §10.3 门禁、等待 review、修复真 finding；每次修复产生新 HEAD 后自动重新独立评论 `/review`，并等待该新 HEAD 对应的 e2e 与 CodeRabbit re-review 通过后才 merge，绝不复用旧 HEAD 证据。除必须由用户裁决的产品方向或不可逆操作外不中途回问。终态为计划全部阶段 `✅ YYYY-MM-DD`、`## Finish Evidence` 完整，并迁入 `docs/finished_plans/plan-refactor-client-store-lifecycle-v1.md`。
