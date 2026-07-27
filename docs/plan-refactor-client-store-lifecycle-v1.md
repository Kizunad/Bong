# plan-refactor-client-store-lifecycle-v1 — Client 状态 Store 统一断线生命周期（重构轨 R2）

> 所属总纲：`plan-refactor-master-v1.md`。一句话：把 client 侧 108 个各自为政的静态 Store 收敛到统一 `SessionScopedStore` 契约 + 显式静态登记清理，让“断线残留/跨会话串味”这一整类 bug（现存 14 份断线核心）在架构上不可能再发生。

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
- **跨仓库契约**：零 wire 改动；loop 音效清理对齐 `feedback_audio_loop_lifecycle`（硬停连延迟层一起哑）。

## 阶段

- ⏳ P0 设计收口 + 吸收清单验真：以 FQCN manifest 对 108 个生产 `*Store.java` 做会话态 / 持久配置态 / 纯常量表分类；冻结 `SessionScopedStore`、显式静态 adapter registry 与源码扫描强制，不使用构造器自注册或运行时反射。P0 只落框架、分类和门禁，不改变既有 Store 的断线行为。
- ⬜ P1 在册 Store 平移：把当前 `clearClientStateOnDisconnect()` 中已存在的 Store 清理逐项迁入 registry；断线 helper 改为调用 registry 一次。非 Store 的 renderer / handler / ambience 等生命周期 hook 继续由 helper 显式拥有，P1 不借重构删除它们。
- ⬜ P2 裸奔状态收编：按 P0 manifest 显式接入 Freshness、`combat/store/`、炼丹 / 锻造 / 灵田 / 灵宝 / TSY / 医道等会话态 Store、玩家动画层缓存，并为灵田等循环音效提供“活动实例 + 延迟层 + 派生 flag”同边界硬停。
- ⬜ P3 全量强制 + 删旧：统一**生产生命周期入口**为 `clearOnDisconnect()`（不把清 listener / test seam 的 `resetForTests` 当生产清理）；删除手工 Store 清单；源码扫描强制“生产 `*Store.java` 必须恰好有一种分类，session 类必须以强类型 handle 登记”，新增漏分类或漏登记即红。
- ⬜ P4 断线 / 重连验收 + 归档：client 契约 pin 覆盖旧 handler 迟到断线、旧 session 排队 payload、清空后新首包重灌；bot `reconnect_state_freshness` 验首包集合。只批量归档被 R2 完整修复的 plan；部分吸收项保留其 UI hydration / freshness gate 等独立工作。

## P0 决议（pre-P0 收口，2026-07-27）

### #1 注册机制：显式静态 adapter registry

**决议**：
1. `SessionScopedStore` 只定义生产语义 `void clearOnDisconnect()`；中央 registry 保存显式、强类型 adapter（method reference / 命名 handle），不要求现有静态 utility Store 被实例化。
2. 禁止构造器自注册、运行时反射和注解扫描。多数 Store 是 private constructor + static state，构造器路径并不会被业务触发；显式 manifest 才能稳定 grep、review 和源码扫描。
3. `BongNetworkHandler.disconnectSession(...)` 的原子边界不变：先 `ClientConnectionStatusStore.invalidateSession(handler, disconnectedAtMs)`，仅 active token 失活成功才在同一 client-thread task 清 registry。旧 handler 的迟到 DISCONNECT 不得清新 session。

**落点**：`client/src/main/java/com/bong/client/BongNetworkHandler.java:305-315`；`client/src/main/java/com/bong/client/ui/ClientConnectionStatusStore.java:129-153`；plan P0/P1/P3。

### #2 生产清理与测试 reset 分离

**决议**：
1. `clearOnDisconnect()` 只清 session payload、entity/world 引用、活动交互、派生 HUD/VFX/audio/animation 状态；必须保留长期 listener、dispatcher wiring、built-in registry 和 test seam。
2. `resetForTests()` / `resetForTest()` / `clearForTests()` 继续只服务测试隔离，可额外清 listener、sequence 或替换 seam；生产代码不得调用它们。
3. 现有生产路径调用测试 reset 的 Store 在 P2 增加 data-only production clear，再迁入 registry；不以重命名掩盖语义差异。

**落点**：`client/src/main/java/com/bong/client/alchemy/state/AlchemySessionStore.java:42-68`；`client/src/main/java/com/bong/client/combat/QuickUseSlotStore.java:68-75`；`client/src/main/java/com/bong/client/combat/SkillBarStore.java:57-61`；`client/src/main/java/com/bong/client/animation/BongAnimationPlayer.java:343-351`；plan P2/P3。

### #3 分类与强制边界

**决议**：
1. P0 manifest 以 production source root 下的 FQCN 为唯一键，覆盖 P0 基线的全部 108 个业务 `*Store.java`；每个业务文件必须恰好落入 session-scoped、persistent-config、constant 三类之一。新增的 `SessionScopedStore.java` 接口虽匹配文件后缀，但作为 lifecycle infrastructure 由 source guard 单独显式排除，不能被误计为第 109 个业务 Store；registry / catalog 类自身不以 `*Store.java` 命名。
2. 当前明确例外是 `HudLayoutPreferenceStore`（persistent-config，断线保留用户 HUD 偏好）和 `ArmorProfileStore`（constant，固定护甲 profile 表）。106 个 session-scoped Store 中，`ClientConnectionStatusStore` 是 token-gated 连接状态机：它由 `invalidateSession(handler, disconnectedAtMs)` 在 registry 之前按 handler 精确失活，不得再被无参全局 clear；其余业务 Store 必须在 P3 前全部有生产 clear 并登记。
3. 测试以 `Files.walk` 扫描 `client/src/main/java/com/bong/client`，用排序后的相对路径 / FQCN 比较“发现集 = manifest 三类并集”，并断言分类互斥、session 集 = registry 集；不只硬编码数量 108，也不扫描 test / gametest / build-generated。

**落点**：`client/src/main/java/com/bong/client/hud/HudLayoutPreferenceStore.java:15-51`；`client/src/main/java/com/bong/client/combat/ArmorProfileStore.java:45-86`；`client/src/test/java/com/bong/client/BongNetworkHandlerTest.java:560-575`；plan P0/P3。

### #4 手工 helper 与非 Store 生命周期职责

**决议**：
1. P1 只把 `clearClientStateOnDisconnect()` 现有 Store 调用平移到 registry，并用 behavior pin 证明行为不变。
2. `NpcDialogueBubbleRenderer`、disguise handlers、`MusicStateMachine`、`MutationVisualState`、`EraAmbianceState`、`BongToast` 等非 `*Store.java` 清理不得因删除手工 Store 清单而消失；它们继续留在 token-gated helper，直到各自所属轨道另有明确生命周期 owner。
3. P2 新增的动画 / generic audio lifecycle handle 可登记为 session resource，但不能伪装成 108 Store 分类项；Store manifest 和 adjunct resource registry 必须可区分。

**落点**：`client/src/main/java/com/bong/client/BongNetworkHandler.java:1119-1200`；plan P1/P2/P3。

### #5 吸收清单边界

**决议**：
1. R2 可覆盖 14 份计划的断线残留核心；炼丹、锻造、灵宝仅部分吸收，仍需独立处理 Screen 关闭 / 权威 hydration / current-session freshness gate。
2. `ambient-zone-audio-stale-anchor` 不由 R2 吸收：它是同一在线 session 内位置 anchor / 去重键不刷新。
3. `zone-environment-audio-loop-fallback` 不由 R2 吸收：它是 `EnvironmentAudioController.soundFor()` 漏映射。R2 只保证现有 / 新增 loop 的断线硬停，不替代映射数据修复。

**落点**：`client/src/main/java/com/bong/client/audio/MusicStateMachine.java:214-236`；`client/src/main/java/com/bong/client/audio/SoundRecipePlayer.java:76-123`；`client/src/main/java/com/bong/client/environment/EnvironmentAudioController.java:57-63,117-124`；plan P2/P4 与吸收清单。

原开放问题全部以本节决议为准；实施阶段不得重新引入构造器自注册、生产 test reset 或反射扫描。

## 吸收清单（active 13 + skeleton 若干；短名省略 plan-bughunt- 前缀与 -v1 后缀）

active：alchemy-ui-session-stale、breakthrough-billboard-session-leak、client-freshness-store-session-stale、forge-ui-session-stale、full-power-charging-session-bleed、lingtian-session-disconnect-ui、perception-edge-session-leak、playeranim-reconnect-stale-layer、poison-trait-hud-disconnect、spirittreasure-session-leak、tsy-extract-disconnect-stale、woliu-vortex-disconnect-residue、yidao-hud-disconnect-bleed。
skeleton：niche-guardian-cross-session-leak。

不吸收：`ambient-zone-audio-stale-anchor`（在线 session 内 anchor / 去重键刷新问题）、`zone-environment-audio-loop-fallback`（recipe id 映射缺口）。R2 只为 generic loop / pending layer 提供断线硬停契约。

## 文件所有权与边界

- 独占：全部 `*Store.java` 的生命周期接口与登记、`BongNetworkHandler.java` 的 `clearClientStateOnDisconnect` 区段。
- 不碰：`BongNetworkHandler.register()` 的 channel 注册区（R6 域，同文件分区段协作，两轨 merge 前互相 fetch）；Screen 结构（R7 域）；store 的业务字段语义。
- 依赖：无前置，Wave 0 即可动工。R7/R9 依赖本轨接口，先于它们合入。

## bot 验收场景

bot 是协议级客户端，测不了 client 内存——本轨主验收是 client 单测（注册表强制扫描 + 每类 store 断线清理 pin 测试），bot 侧配合场景：
1. `reconnect_state_freshness`：bot 断线重连后 server 重发的首包快照集完整（联动 R6 join 首包契约），保证"清干净之后能重新灌满"。

## 开放问题（pre-P0 已收口）

1. 注册机制采用显式静态 adapter registry + FQCN manifest，见「P0 决议」#1/#3；不采用构造器自注册。
2. 生产 `clearOnDisconnect()` 与 test reset 明确分离，见「P0 决议」#2；不删除仍用于测试隔离的 reset API。
