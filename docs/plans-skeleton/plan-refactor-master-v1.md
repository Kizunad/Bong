# plan-refactor-master-v1 — Client/Server 大重构总纲（重构计划族 R1-R10）

一句话：230+ 份点状 bughunt/feature plan 背后反复出现的是 **8 个系统性根因**（session 生命周期各写各的、持久化各漏各的、qi 账本可绕、C2S 无门禁、S2C 双轨散装、client store 断线裸奔、UI 无基类、AV 无单一事实源）。本计划族用 9 条重构轨道一次性把根因变成**共享基础设施**，以协议级 bot e2e 为主验收门，代码目标是干净直接无面条（拆 3 个 2 万行级 god file）。

> 撰写依据：2026-07-27 五路侦察（server/client 结构地图、84 active plan、146 skeleton、16 开放 PR 全量盘点）。各轨道 skeleton 文件见 §2 表。

## 0. 范围与铁律

- **玩法/运行时重构只动 `server/` + `client/`**。`worldgen/`、`library-web/` 不动，agent runtime/prompt/arbiter 等行为域独立保留（§6.11-6.12）；跨端 wire 的 TypeBox schema source 是本范围的唯一基础设施例外，按 §4.1 分 owner。
- **TypeBox source 是 repo-wide schema source of truth**。对外契约（Redis IPC、proto schema）原则上不动形状；确需变更时先改 TypeBox canonical content，再由 R6-owned generation/transport machinery 按 R6 plan 同步其 mirrors，并走必要的 breaking checks。**不写兼容层**——production activation 服从 §4.1 不变量。
- 真元守恒律、worldview 正典、招式 A/V 差异化红线全部继续生效。
- **测试方针（用户 2026-07-27 指示，仅限重构轨道，覆盖根 CLAUDE.md「饱和化测试」节）**：
  1. **bot e2e 场景是主验收门**——每条轨道自带 3-8 个 `scripts/bot/scenarios/` 场景，先于/伴随重构落地；
  2. 单测只保留**契约 pin**：schema sample 对拍、守恒断言、状态机转换、注册表强制扫描；
  3. 与被删实现绑定的旧单测允许随代码删除；不要求饱和覆盖；
  4. feature plan（非重构轨）不适用本条，仍按根 CLAUDE.md。
- **Headless 多端是产品需求（用户 2026-08-08 指示），不是测试策略**：所有 client 端侧请求路径必须做成适配无头请求形式（多端形式）——Java 游戏客户端只是多个客户端之一，不是参照实现；近期驱动是 agent 协助挂机刷宝类玩法，服务端以此为基础搭建、不做事后补装。可测判据：**一个无 UI、无渲染、无输入设备的客户端，能否只凭 wire（C2S 请求 + `bong:server_data`/授权投影）完成该路径，并机器可读地得知结果——含拒绝，且可与发出的请求关联**。这与上一条测试方针是两个不同命题：bot e2e 绿只证明重构没坏，不证明该路径 headless 可完成；凡核心闭环依赖 dev 命令旁路、人类可读文案或 client 渲染态的场景不计入 headless 证据。逐轨裁决、验收增量与已登记张力见 §4.3；未决项见 §9.5-§9.9。
- 代码风格：巨型 match/god function 拆注册表；复制粘贴生命周期抽共享框架；仓库既有范式「集中注册表 + 显式映射」保留（可 grep 性优先，不引入注解扫描/反射魔法）。

## 1. 基线：先清空在飞 PR（重构动核心文件前必须 merge/close）

| PR | 内容 | 对重构的影响 |
|---|---|---|
| #1287 | 冷却按 skill_id 全局重构（14 resolver + network 三文件） | R9/R4 基线，先 merge |
| #1289 | Lifecycle 持久化 + v39 迁移（**e2e 红，需先查清**） | R3 首批宿主；与 #1259 同改 `combat/lifecycle.rs`，注意 auto-merge 叠字段坑 |
| #1288 | KnownTechniques 载入守护 | R3 载入守护先例 |
| #1259 | satiety P0（新 `nourishment/` 模块 + lifecycle 大改） | R3/R5 基线；PR-2~5 与重构窗口协调（§5.6 冻结窗口） |
| #1261 | recipe 关服 flush + `scripts/` 生命周期重写 | R3 吸收对象；**全部 tmux 会话改用新脚本** |
| #1292/#1296/#1299 | carrier NaN / BossDrain zone-shadow / coffin 维度门禁 | R5/R4 吸收清单里标注"已闭环只归档" |
| #1282/#1290/#1294/#1253 | 新 skeleton ×10（Wounds 重连满血、player-slice 载入守护、炼丹锻器种植 ×7、block-break 集成层） | 已并入各轨吸收清单 / §6.10 |
| #1281/#1291/#1275 | race/ci-redis 归档回退、nested-pack WITHDRAWN | docs 基线，覆盖矩阵按其终态 |
| #1249 | fpv-cast-av P3（client juice） | R9 不吸收 fpv plan，契约对齐 |

多个 PR 的 `finalize` check 呈规律性 FAILURE（#1294/#1292/#1289/#1287/#1275/#1253）——疑似 review 工作流基建噪音，P0 核实一次，别当代码问题追。

## 2. 轨道总览

| 轨 | plan 文件 | 核心产出 | 主要文件域 | 吸收 plan 数 |
|---|---|---|---|---|
| R1 | `plan-refactor-server-session-v1` | server 统一 InteractionSession 框架 | `server/src/session/`（新）+ 7 域 session.rs | ~13 |
| R2 | `plan-refactor-client-store-lifecycle-v1` | client SessionScopedStore + 强制登记 | 108 个 `*Store.java` + 断线清理清单 | ~16 |
| R3 | `plan-refactor-persistence-slices-v1` | 持久化 Slice 框架 + persistence 巨石拆分 | `server/src/persistence/**` + autosave | ~25 |
| R4 | `plan-refactor-c2s-gate-v1` | C2S 声明式门禁 + handler 巨石拆分 | `client_request_handler.rs` + `network/gate/` | ~24 |
| R5 | `plan-refactor-qi-ledger-v1` | qi 账本架构强制化（字段收私有） | `qi_physics/**` + 全仓直写点 | ~20 |
| R6 | `plan-refactor-wire-s2c-v1` | S2C schema generation/transport machinery、emit builder、client 双轨归一与作用域广播 | TypeBox generation machinery、generated mirrors、`proto_convert.rs`、client bridge/router plumbing | ~12 |
| R7 | `plan-refactor-client-ui-base-v1` | Screen 基类 + InspectScreen 拆解 + 输入/线程纪律 | client Screen/hud/keybind | ~17 |
| R9 | `plan-refactor-cast-av-contract-v1` | cast TypeBox 内容语义 + reducer/state machine + SkillAvBinding 单一事实源 | TypeBox cast declarations、server cast/AV semantics、client cast store | ~13 |
| R10 | `plan-refactor-inventory-core-v1` | inventory 巨石拆分 + InventoryTxn 事务 | `server/src/inventory/**` | ~7 |
| V | `plan-bot-e2e-coverage-v1`（既有 skeleton 直接促升，不另立） | bot 场景 P1-P6 扩容 + CI 假绿修复 + build token 脚本 | `scripts/bot/**`、CI | ~9 |
| 基建 | `plan-registry-datafication-v1`（既有 skeleton 直接促升） | 硬编码配方/功法/方块表迁数据 + fail-fast | 三张表 | 自身 |

## 3. 波次与依赖

**本节 Wave 表是全部轨道 inter-track ordering/start/cutover claims 的唯一权威（SOLE authority）**；各轨 plan 只能引用本节已有顺序，不得自行新增跨轨前置。需新增或改变顺序时先 amendment 本节，再同步子 plan。

- **Wave 0（立即并行）**：V（bot 骨干 + build token 最先）、R3、R5、R2、registry-datafication；同时全部轨道的 P0（设计收口 + 吸收清单验真）都可开工；R6 的 contract-first 工作与 R9 的 cast domain contract-first 工作均可在本波次按各自 plan 开工，不等待 production activation 条件。
- **Wave 1**：R6、R7（R2 合入后）、R1（R3 P1 合入后）按各自 plan 推进；涉及 R2-owned production 接缝的工作须等待 R2 P1。
- **Wave 2**：R4、R9 production activation（R5、R6、R2 的所属责任按各自 plan 就绪后，服从 §4.1 的 ownership 与 atomicity invariants）、R10 P1（R3 P1 后）。**Craft activation 由 M-10 原子切换放行：M-09 的 R2/R6 contract-first artifacts 可在 Wave 0/1 交付，M-10 的 R1 producer、R3 persistence、R6 transport 与 R2 consumer 必须在同一 production merge unit 中闭合。** **R6 dropped-loot P3 production activation merge unit 仅在 R10 P2a `DroppedLootEntry.owner/visibility` metadata provider 与 R3 P4 dropped-loot migration/hydration consumer 均合入后放行；此前只允许 declared/unwired/test-only 的 contract 与 pin artifacts。R10 P2b `OwnerOnly` private-writer activation 必须在该 R6 P3 merge unit 合入后放行，不得把 R6 P1/P2 的 unwired artifacts 当作 production consumer。**本表只裁决跨轨顺序与 activation 边界；各轨具体 deliverable inventory、phase mapping 与验收证据由各自 plan 定义，不在总纲重述。
- **R3/R6 dropped-loot child-plan ordering**：R10 P1 migration helper 与 R3 P2 persistence seam/legacy compatibility pins 就绪后，R3 P4 dropped-loot hydration consumer 必须先于 R6 P3 dropped-loot projection/page production activation；R3 P4 inventory-layout overflow consumer 是独立子批次，仅在 R10 P3 合入后执行。
- **R10 child-plan ordering**：顺序固定为 **R3 P1 → R10 P1 → R3 P2 → R10 P2a → R3 P4 dropped-loot hydration → R6 P3 → (R5 P3 + R6 P4) → R10 P3 → R4 pickup consumer → R10 P2b → R3 P4 inventory-layout overflow → R10 P4**；其中 R10 P2a 不得早于 R3 P2，R10 P3 不得早于 R5 P3/R6 P4，R4 pickup consumer 不得早于 R10 P3，R10 P2b 必须等待前述 R3/R6/R5/R10/R4 production consumers 全部完成。具体 phase/artifact/验收仍以 R3、R6、R10、R4、R5 各 owner plan 为准。
- 近完成独立 plan（§6.9）在 Wave 0 窗口内优先收尾清场。
- R5 P1（字段收私有的全仓编译大爆破）挑在飞 PR 队列清空的窗口单独合入。

## 4. 文件所有权矩阵（防并行打架，冲突时以本表为准）

- `persistence/**`+autosave=R3；`session/`+7 域 session.rs=R1；`client_request_handler.rs`+`gate/`=R4；`*_emit.rs` 公共层+schema generation/transport machinery+`proto_convert.rs`=R6；`qi_physics/**`+qi 字段直写行=R5；`inventory/**`=R10；cast domain semantics/AV emit+skill 注册=R9。
- client：Store 生命周期+`clearClientStateOnDisconnect` 区段=R2；generated bridge+channel/`ServerDataRouter` registration plumbing=R6（与 R2 同文件不同区段，merge 前互 fetch）；Screen/hud/keybind/InspectScreen=R7；cast reducer/store 与具体 domain consumers=R9。
- 任何轨道碰他轨文件：只允许“消费对方冻结后的 API”，不允许改对方独占文件；接缝 API 归被依赖方定义。跨轨 wire 共同交付按 §4.1，不得用此通则拆开 activation merge unit。

### 4.1 R9 ↔ R6 schema 与 production cutover 裁决（2026-08-04）

本节是本计划族对 cast wire 的唯一上位裁决；R6/R9 子 plan 若冲突必须先同步到本节，禁止各自声明第二套 authority 或交付顺序。

1. **Schema authority**：TypeBox source 是 repo-wide schema source of truth；它拥有 shape、discriminant 与 validation semantics。所有生成或受约束 mirror 均不得反向定义 TypeBox 为“被动镜像”。每个 generated/constrained artifact（含 protobuf/Rust conversion、Java bridge 与 samples）必须携可核验的 TypeBox source hash 或 version reference；CI 必须 fail-closed 检查该 pin 与当前 TypeBox source 一致，禁止 stale mirror 通过。
2. **Domain content vs machinery**：R9 负责 author/review cast domain 的 TypeBox content、domain semantics/reducer 与 `SkillAvBinding`；R6 负责 repo-wide schema generation/transport machinery 及其生成或受约束 artifacts。R9 定义“cast 消息/状态是什么意思”，R6 定义“canonical schema 如何生成、转换和运输”；双方不得复制或重新拥有对方 artifact。具体 deliverable inventory、phase mapping 与 acceptance evidence 仅由 owner track plan 定义。R6 plan 的 P1 必须交付 declared/unwired/test-only 的 schema generation chain contract 与 CI freshness/pin gate，P3 必须在单一 production activation merge unit 内生成 mirrors 并接入 transport/router；在 P1 contract 完成前，不得排期依赖该 generation chain 的 production cutover。
3. **Contract-first 可早合**：R9 contract-first 工作不等待 R6 production machinery；R6 的 contract-first 工作也不因 production 接缝尚未完成而停止。contract-first artifacts 必须保持 declared/unwired/test-only，不得切换 production traffic 或宣称 live reachable。
4. **Atomic production activation**：迁移某 channel 时，其 producer、transport plumbing、全部 consumers 与旧路径移除必须在**同一 merge unit**激活。若无法做到单一 merge unit，则旧 producer/receiver 必须原样保留，直到最终原子 activation；禁止 receiver-removed-before-consumer-installed，也禁止长期 dual emit。R6 dropped-loot 必须把 paginated producer、generated mirror/conversion、`ServerDataRouter` transport registration、完整 revision page assembler 的 atomic store replace 与旧路径移除绑定为一个 P3 merge unit；该 merge unit 的跨轨放行只取 §3 Wave 表。
5. **Contract pin-test invariant**：任何 contract-first schema 或 reducer/state-machine artifact 在首次提交时即必须携 pin tests；不得以后续 production/e2e 阶段补测。TypeBox pin 至少覆盖必填字段、每个 enum/discriminant 变体、invalid/unknown 变体与缺字段拒绝；reducer pin 至少覆盖每条合法转换、非法转换拒绝，以及 STOP/INTERRUPT 等终止路径。cast schema 与 cast reducer 是本不变量的强制命中项。
6. **Generalized cross-track dependency rule**：
   - §3 Wave 表是 inter-track ordering claim 的**唯一权威**；track plan 不得引入本表没有的 start/order dependency。
   - master 只裁决 owner、跨轨顺序与 invariants；只有在 owner plan 已定义 phase/artifact/consumer 后才可引用其名称，不得替 owner plan 首次定义交付细节或验收步骤。
   - 若所需 upstream artifact 尚不存在，consumer track 只能先落 contract-first stub（declared、unwired、test-only，不接 production）；真实 artifact 的名称、phase 与验收由 owner track plan 定义，且 production dependency **不得反写成 start gate**。
   - 两份 track plan 出现 sequencing conflict 时，先 amendment §3 Wave 表并形成唯一裁决，再同步双方；禁止靠任一子 plan prose 抢占 authority。R9 contract-first start 以 Wave 0 为准，production activation 以 Wave 2 与本节 atomicity invariant 为准。
7. **Contract-first 与 freshness gate 的兼容规则**：§4.1.1 的 mirror freshness/pin gate 只约束**已存在 generation pipeline 且已实际产出 committed mirrors** 的 generated/constrained artifact——即 R6 P1 generation chain contract 落地（manifest/tooling/pin inventory 就绪）、R6 P3 运行 pipeline 实际产出 committed mirrors 之后，CI 才对当前 TypeBox source 与已生成 mirror 做 fail-closed 一致性检查。此前的 declared/unwired/test-only contract-first TypeBox 提交（如 R9 P1 的 cast `source`/`target`/`phase` 与 STOP/INTERRUPT 变更）不触发 mirror freshness gate（尚无 committed mirrors 可对 pin）；R6 P3 产出 committed mirrors 时若 source 已变化，由 R6 P3 一并刷新 mirrors 并更新 pin——该提交只须随附 §4.1.5 的 contract pin tests，不得要求 contract-first 提交方更新 R6-owned mirrors，也不得因镜像刷新义务被阻塞。mirror 的生成与刷新仍归 R6 P3 的 atomic activation merge unit。wire 接线状态与 artifact freshness 是独立关注点，禁止用 freshness gate 把 contract-first 提交变相升级为 production activation 的 start gate。

### 4.2 Craft lifecycle artifact ledger（M-09/M-10）

为使 R1/R2/R6/R7 对 craft restore 的引用可执行，以下两行是 `M-09` 与 `M-10` 的唯一 artifact、owner、阶段和验收登记；子计划不得另造同名含义。

| Artifact | owner / phase | canonical deliverable | acceptance / cutover evidence |
|---|---|---|---|
| **M-09** CraftStore lifecycle + A-06/A-08 handler contract | A-CS P3 冻结 A-row；R2 P1 生产 `CraftStore` lifecycle/freshness/request-latch；R6 P1 craft machinery 生产 declared、unwired、test-only bridge/router contract | `CraftStore` 是 client craft session-state 唯一 owner，维护 accepted identity/generation/phase revision、`OpenPending` 与 armed `CraftRestoreGuard`；R6 registration 对拍 A-CS SHA 的 ordinary A-06 (`Initial | Rollover`)、guarded `Restore`、A-08 correlation/reason 和 control-frame 字段；contract-first 阶段不得接 production traffic | R2 Store state/revision/guard pins、R6 converter/router roundtrip pins、A-CS source/generated/dist SHA 对拍全部通过；M-09 只证明 contract-first artifacts 冻结，不证明 producer→consumer live reachability |
| **M-10** Craft production atomic activation | R1 P1/P4 authoritative state/rejection producer；R3 P1 M-04/M-12 guard/checkpoint persistence；R6 P3 proto/generated/converter/transport/router；R2 P1 bridge/`CraftStore` consumer；R4 admission gate 与 R7 intent 只接入各自 owner contract | 单一 merge unit 原子启用 S-01 correlated A-08、S-07 `ReconnectGuard` persistence、`CraftRestoreGuard` control frame、A-06 guarded `Restore` 与 client arm/accept；Restore 必须满足 owner/session/generation/token 匹配且 `phase_revision > guard.phase_revision`，成功消费 guard，不保留旧 producer/receiver 双轨 | producer→persistence→frame→A-06 Restore→`CraftStore` 的全链 trace、A-08 matching reject、strictly-higher revision、stale/replay no-op、旧路径移除和 bot/e2e evidence 全部在同一 activation merge unit 中通过；未闭合前仅保留 M-09 contract-first artifacts |

M-09 是 contract-first handoff，M-10 是唯一 craft production cutover；两者均服从本节 §4.1 的 TypeBox ownership、atomicity 与 pin-test invariants。

### 4.3 Headless 多端硬约束逐轨裁决（2026-08-08，§0 headless 铁律的执行面）

本节按 §0 headless 铁律对 R1-R10、V、基建逐轨裁决影响面。按 §4.1.6：本表只对各 owner plan **已定义**的 artifact 附加不变量与验收判据，不替 owner 首次定义 deliverable/phase/跨轨顺序；证据列引自各轨 plan 正文。「受影响=确认」表示该轨契约形状本就是 headless-正确的，增量是把这一形状 pin 成 headless 验收判据，防止实现期退化成只有 Java client 能消费的形态。

| 轨 | 裁决 | 轨内证据 | headless 验收增量（附加不变量） |
|---|---|---|---|
| R1 | 受影响=确认 | S-01 对 admitted Open 返回"与 `CraftOpen.request_id` 关联的 typed `CraftOpenRejected`"；§5 craft wire 行生产 authoritative `CraftSessionStateV2` | S-01..S-26 的每个转换必须仅由 wire 事件 + server 时间驱动并经权威投影可观察；§6 derived index 不得出现以 client 渲染态（Screen 存在性、HUD 状态等）为 admission/restore 前置的 trace。S-01 的 request 关联 typed 拒绝是全族 headless 拒绝反馈的范式 |
| R2 | 不受影响（Java client 内实现域），附边界不变量 | "bot 是协议级客户端，测不了 client 内存——本轨主验收是 client 单测"；"跨仓库契约：通常零 wire 改动" | `CraftStore` acceptance 表（phase_revision 单调、guard 消费）只能是 M-09/A-CS wire 契约的投影：headless 客户端必须能仅凭 wire 契约重建等价的消费状态机。凡只存在于 R2 Java 实现/测试、wire 契约侧未表达的消费规则，视为契约缺口上报 M-09 修契约，不得沉淀为 Java-only 隐性语义（张力 #3） |
| R3 | 不受影响，一处缺口挂 §9 | "跨仓库契约：零 wire 改动"；开放问题 1"载入守护的玩家体验：只读降级 vs 拒绝进服" | 无新增不变量。载入守护"只读降级"目前无机器可读信号，headless 客户端无法得知自己处于降级态——signal 形状归 R6，见 §9.8 |
| R4 | 受影响=核心 | P0 决议 4 自认 EventAlert"没有 request/reason/request_id 字段，不能被宣称为结构化 ack"；决议 3"预算耗尽即丢弃/合并，不 decode" | ① 每类 GateSpec 拒绝最终必须对请求者机器可读且可与请求关联（request_kind + 安全折叠 reason_code + request_id，即 R4 P0 决议 4 已命名、走 R6 契约流程的 `request_rejected`）；P1 的 EventAlert 临时反馈是人类面文案，**不计入 headless 验收**，R4 P4 归档不得以 toast 在场作为拒绝反馈的完成证据（张力 #1）。② ingress 预算静默丢弃对 agent 客户端等价于网络丢包；背压信号未收口（§9.7）前，R4 必须在 plan 内显式登记该限制，不得默认 agent 客户端能与人类同速（张力 #2） |
| R5 | 不受影响 | "本轨 P0-P3 是纯 server 内部重构；P4 bot e2e 通过既有 dev telemetry 校验，不新增 agent/client wire 形状" | 无 |
| R6 | 受影响=核心 | "出料：`bong:server_data` 单通道……join/重连首包快照集契约"；P0 已冻结 producer ledger 与 31 旁路 28 收编/3 豁免；P4"113 C2S + 144 S2C 每变体至少一条正反 sample" | ① 豁免登记附加 headless 判定：3 个豁免通道逐项证明只承载渲染/资产/握手类信息——凡 gameplay-authoritative 状态不得走豁免通道，headless 客户端跳过全部豁免通道后 gameplay 状态无损。② join 首包 + 后续 `server_data` 必须足以让无本地渲染缓存的客户端重建全部 gameplay-authoritative 可见状态；P0 producer ledger 的 strict join/join-derived/active replay 分类即该判据的证据面。③ P4 双向 sample 集是第三方 headless 客户端实现的机器可验字典：sample 缺失即 headless 缺口，不是可延期的 nice-to-have |
| R7 | 不受影响 | "server 全部不碰；无 wire/schema/Redis 变更" | 无。键位/HUD/Screen 只属 Java 端；"gameplay 前置不得依赖键位/Screen 态"由 R4 server 权威保证，不在 R7 |
| R9 | 受影响=确认 | I-04"只有它（CastSync）能建立或终结 authoritative active cast"、I-06 PLAY/STOP 仅 advisory；A-13/Iris 节"server 只 emit semantic effect ID/tier；Iris 缺失不影响 gameplay" | 把 gameplay/AV 分层 pin 成 headless 验收：仅订阅 `cast_session_begin`+`cast_sync`、忽略两个 `vfx_event` arm 的客户端必须获得完整施法 gameplay 生命周期与 P-09 全部 17 种 typed outcome。招式 A/V 五件套红线约束的是 Java client 交付物，不得反向成为 gameplay 前置；Iris 能力分层（A-13）是通例，headless = 渲染能力为零的极端档 |
| R10 | 受影响=确认 | "snapshot 仅作状态修正，不是动作级反馈"；容量超限"返回 `{current, required, limit}`，所有状态与 DB 不变"；deferred #2 要求 receipt 贯通 `request_id` | 动作级 accepted/rejected receipt（request 关联 + 机器可读 reason/limit 字段）是 headless 客户端唯一成功信号，snapshot 不得替代。挂机刷宝核心循环（dropped-loot 投影→pickup→merge/capacity→receipt）必须全程 wire 闭环；R6 P4 已列的 bot decoder 对拍即 headless 消费方证据 |
| V | 受影响=角色变化 | "让 CI 在无真人客户端条件下锁住玩家可感知行为"；P0 已交付 `mc_protocol.py`/`bot.py` 的动作与断言集 | `scripts/bot/` 是事实上的第一个 headless 客户端原型，其能力面即 headless 能力下限。分类不变量：bot 场景绿不自动构成 headless 证据——凡核心路径（非前置铺垫）依赖 dev 命令旁路的场景不计入 §0 headless 判据；计入者的核心闭环必须仅由生产 wire 消息驱动，dev 命令只允许出现在铺垫段。V P6 protobuf 深断言是 headless 消费 `server_data` 的参考解码实现。bot 框架是否促升为受支持 reference client 见 §9.9 |
| 基建 | 不受影响 | "跨仓库契约：零 wire / proto / schema 改动；client 零改动……agent 零改动" | 无 |

**已登记张力（写下不抹平；收口路径见 §9）**：

1. **`request_rejected` 有名无 phase、无 wave 槽位**：R4 P0 决议 4 已命名该 artifact（request_kind / 安全折叠 reason_code / 可选 request_id）并指定走 R6 契约流程，但 R6 plan 各 phase 均未登记它——headless 铁律使它从"可选增强"变为 R4 headless 完成的必要条件。按 §4.1.6 本表不替 R6 首次定义交付细节：需 R6 plan amendment 补 phase/artifact/consumer 后，再 amendment §3 裁决其与 R4 Wave 2 production activation 的顺序关系。在此之前，R4 的拒绝反馈停留在人类面文案，是 headless 判据下的已知未闭合缺口（§9.6）。
2. **反 oracle 折叠/反 flood 静默 vs agent 背压**：R4 的安全折叠（外部不可区分）与预算耗尽 decode 前静默丢弃是有意的安全设计；headless 铁律要求结果机器可读——两者在"预算耗尽"一格正面相撞。不在本表拍板，见 §9.7。
3. **消费侧 wire 语义沉淀在 Java-only 载体**：R2 `CraftStore` acceptance 表是当前最大的一份——语义正确，但载体是 Java 实现 + 单测（R2 自述 bot 测不了 client 内存）。M-09 ledger 已对拍 A-CS SHA 是正确方向；本表的通用不变量是：任何"客户端必须如何消费 wire"的规则须在 wire 契约侧（TypeBox/A-CS/master ledger）有对应表达，Java 实现只是其一份投影。

## 5. 工作流（GPT tmux 多会话）

1. **一轨 = 一个 tmux 会话**（claude-code 映射的 gpt-5.6-sol-xhigh，多轮迭代，可自 spawn subagent）。10+ 会话时：9 轨 + V + registry-datafication + 若干近完成收尾会话。
2. **认领**：沿用 bugfix 原子 claim——分支 `refactor/<plan-basename>`，create-ref API 创建即认领（201 到手 / 422 甄别）；促升 skeleton→active 在自己分支内完成（每轨一次 `git mv`）。
3. **编译并发治理（硬约束）**：cargo build/test 全局并发 **≤2**、gradle **≤1**（历史 3 并行 cargo OOM + 塞盘 444G 实录）。V 轨 P0 先落地 `scripts/build-token.sh`（flock 计数令牌，包住 cargo/gradle 调用），**所有会话必须经它跑构建**；写代码不受限。
4. **磁盘纪律**：常驻 slot/worktree 复用热缓存，严禁每任务新建 worktree 堆积；`bash scripts/wt-janitor.sh` 周期巡检。
5. **merge 纪律**：push 前 `git fetch origin && git merge origin/main`（紧邻执行）→ 受影响栈门禁重跑（auto-merge 叠字段 E0062/E0415 坑）→ 才 push。多轨同文件（见 §4）互相盯 in-flight PR。
6. **冻结窗口**：feature plan（satiety PR-2~5、fpv P4/P5、dense-fog 等）触碰重构独占文件时，等对应轨道当前批次合入后再动；反向同理。由跑总纲的调度会话协调。
7. **每 PR**：中文 commit + `Model:` trailer（真实模型 id）→ `gh pr create`（标题/body 带 plan basename）→ 评论 `/review` → 等 e2e 绿 + CodeRabbit。重构 PR 的验收证据 = bot 场景绿 + 契约 pin 绿。
8. **调度会话**（可选第 11+ 个 tmux）：盯全族 in-flight PR 的 review 返工、波次放行、冻结窗口协调——只调度不写码，对齐 BugFix 工作流主干职责。

## 6. 覆盖矩阵（全量 plan → 归属；短名省略 plan-/plan-bughunt- 前缀与 -v1 后缀）

> 各轨道文件内的「吸收清单」是权威明细；本节只列**不进 9 条轨道**的部分，保证 84 active + 146 skeleton + 在飞新增 10 全部有归属。促升任何一轨时，P0 必须跑一次「覆盖审计」：枚举 docs 两目录全部 plan 文件 diff 本矩阵，新增 skeleton 即时归类。

- **6.1-6.9 已入轨**：见 R1-R10 各文件吸收清单（合计 ~130 份）。
- **6.10 V 轨（bot 骨干 + 测试诚实性）**：bot-e2e-coverage（促升本体）、bot-combat-server-data-type-false-positive、bot-multibot-chat-visibility、bot-multibot-entity-spawn-visibility、e2e-command-anchor-rejected、task13-mutation-qi-zero-green、proto-breaking-check-shallow-skip（深检部分，与 R6 P4 联动）；已知 server 侧缺口「fallback 平台 centered on origin 非 spawn」一并修。
- **6.11 Agent 轨（本次不重构，独立保留逐个消费）**：active——anticheat-tiandao-drop、niche-guardian-redis-dispatch、npc-combat-relic-schema-drift、pseudo-vein-agent-deadwire、war-participate-agent-command-drift、tiandao-schema-dist-start、server-data-s2c-schema-union-drift 的 TS 侧；skeleton——agent-ui-tiandao-revelation-vfx-flag-loss、alchemy-start-intervention-agent-drop、anqi-carrier-charged-agent-narration、arbiter-cjk-redaction-bypass、heart-demon-late-pregen-fallback、narration-target-prefix-routing、poi-novice-tiandao-narration-drain、technique-feedback-bridge、tiandao-agent-ui-click-context-loss、tsy-agent-ui-wrong-player-routing、tsy-enter-exit-agent-silent-drop、worldmodel-rollback-stub、rebirth-tiandao-bridge-gap、tsy-discovery-ui-target-fallback、player-chat-list-unbounded。
- **6.12 Worldgen 轨（独立保留）**：active——anomaly-raster-runtime-consumer、baolongwang-poi-consumer-gap、raster-check-required-layers、spirit-eye-raster-candidate-disconnect、structure-manifest-loot-consumer、tribulation-scorch-mineral-node-gap、worldgen-pipeline-root-cwd、worldgen-raster-check-cli-noop；skeleton——animal-air-spawn-gravity、spawn-safe-y-surface-drift、spawn-tutorial-poi-y-drift、sword-sea-zone-overlap、tsy-start-raster-env-gap、tsy-y-strata-overlay、worldgen-uint8-maximum-blend、zone-ecology-global-refuge、qi-density-same-source。
- **6.13 接线拍板轨（module-wiring-gaps-v2 为决策菜单，人工拍板后逐个拆实施 plan；重构后接线成本大降）**：module-wiring-gaps-v2、forge-lingtian-processing-deadpath、poi-trespass-refusal-runtime-gap、silent-signal-runtime-bridge、social-runtime-bridge-gap、k2-identity-social-renown-bridge、war-emergent-group-reputation-gap、npc-combat-gear-v2、social-anonymity-live-refresh-gap、unconsumed-event-feedback、zhenfa-array-flag-e2e-wiring、woliu-dying-master-runtime-gap。
- **6.14 Feature 轨（独立，注意 §5.6 冻结窗口）**：active——beast-horde、client-login-ux、container-filter-and-completion、gameplay-journey、gathering-tool-bind、halfstep-buff-calibration、iris-integration、nested-pack（已 WITHDRAWN）、social-v2、sou-da-che、satiety-hydration（在飞）、ci-redis-pull-resilience（#1291 返工中）；skeleton——ancient-relic-payoff、bonecoin-wallet-bridge、craft-chain-items、dandao-mutation-gameplay、dazuo、first-technique-grant、lootcrate、neardeath-ux、newbie-30min-hooks-audit、block-break-integration（#1253，基建 skeleton，建议 Wave 2 后评估与 R4 关系）。
- **6.15 近完成独立收尾（Wave 0 清场，重构不吞）**：craft-refund-full-inventory-loss（余 P4）、dead-armor-contamination-wiring、dense-fog、fpv-cast-av、life-record-epitaph、tribulation-balance。
- **6.16 Round bundle 拆散复核（✅ 2026-07-28，不整体消费）**：r1/r2/r6/r7/r8-modifier-audit/r8/r9/r10 已逐 finding 第一性验真、登记唯一 owner 类别并归档 mapping。八张 `Finding Mapping` 表共有 **61 个物理数据行 = 60 个 finding rows + 1 个 audit-history row**；逐表为 r1=7、r2=10、r6=5、r7=10、r8-modifier-audit=6（5 finding + 1 history）、r8=11、r9=6、r10=6。60 个 finding rows 的分类严格为 32 already-fixed + 1 invalid/retired + 23 independent-domain-fix + 4 absorbed-by-track；r8 bundle/audit 的来源重复仍按各自 finding row 保留映射，不形成第二 implementation owner。四条 absorbed finding 中三条登记 R3、一条登记 R5；23 条 independent finding 的候选短名仅保留在归档 mapping，successor skeleton 按一个 skeleton 一个后续 docs PR 另行建立。本轮是 §7 授权的 docs-only 批量归档例外，不宣称任何未实施 finding 已完成。
- **6.16a Round bundle 后续 successor 队列（短名；本 PR 不创建 skeleton）**：`dandao-pill-rush-dead-realm-guard`、`breakthrough-freeze-factor-align`、`modifier-effect-consumer-completion`、`duxu-juebi-quota-marker-lifecycle`、`botany-drag-release-lifecycle`、`tsy-collapse-hostile-cleanup`、`scatter-bead-ledger-account-cleanup`、`shield-break-state-cleanup`。每个 skeleton 必须在独立后续 docs PR 中第一性收口后再成为可消费 implementation owner；r1 P6 与 r10 #1/#2 由 R3 吸收，r8 #6 由 R5 吸收，Freeze 仍指向既有 `container-filter-and-completion` P2。
- **6.17 孤立域修复（量少不并簇，随缘消费）**：alchemy-freshness-feed、gathering-mineral-origin-position-break、zone-atmosphere-zoneid-profile-mismatch、zone-environment-audio-loop-fallback（音效映射数据部分）、lingtian-quality-accum-harvest（#1294 在飞）。

## 7. 促升与归档机制（被吸收 plan 的出口）

- 各轨 P0「吸收清单验真」：逐个复读被吸收 plan，第一性验真仍是真缺陷才吸收；已被在飞 PR 修掉的标「已闭环只归档」；验伪的写结论证据。
- 被吸收 plan 的归档：对应轨道的修复 PR merge 后，**每轨一个 docs-only 批量归档 PR**——每份被吸收 plan 补 `## Finish Evidence`（指向重构 PR + bot 场景 + 验真结论）后 `git mv` 入 `finished_plans/`。这是对「一个 PR 只动一个 plan」的**总纲授权例外**，仅限归档、不改其他内容。**§6.16 唯一一次性例外**：2026-07-28 Round bundle triage 可在同一 docs-only PR 中逐 finding 验真并归档八份聚合 bundle、记录后续 successor 短名并只更新命中的 canonical absorb-list 行；不得创建 successor skeleton，不得改写 Rx 或其他 plan 正文，不得改代码或配置，也不得把未实施 finding/track 写成已完成。本例外随 §6.16 归档闭环即耗尽，不扩展到后续 plan。
- 覆盖审计脚本化：枚举 `docs/plan-*.md` + `docs/plans-skeleton/*.md` 与本矩阵 diff，未归属项报红（V 轨 P0 顺手落地）。

## 8. 计划族完成定义

1. 9 条轨道全部归档（各自 bot 场景常绿 + 吸收 plan 全部归档/验伪结案）；
2. 三个 2 万行级 god file（inventory/mod.rs、client_request_handler.rs、persistence/mod.rs）不复存在，最大单文件 < 3000 行；
3. `qi_current` 裸写编译不过；client 无未登记的会话态 store；届时现行 `ClientRequestV1` 全部变体均有显式 GateSpec/no_gate 声明（2026-08-03 P0 基线为 104，新增变体自动纳入）；28 旁路 channel 收编或豁免登记；
4. bot 场景数从 ~30 增至 ≥80，CI e2e 是唯一主门禁且无已知假绿。
5. `flash-review` label 下 open issue 全部显式处置（fixed / dup / 验伪关闭 / 促升 skeleton，见 §10），无静默积压。
6. §4.3 裁决为"受影响"的轨道，其 headless 验收增量全部有证据：施法、session 生命周期、inventory/拾取、拒绝反馈四条核心闭环可由无 UI/渲染/输入的客户端仅经生产 wire 完成，结果与拒绝机器可读且可关联请求；§4.3 张力 #1/#2 已按 §9.6/§9.7 决议收口。

## 9. 开放问题（总纲级，pre-P0 收口）

1. **R8 编号空缺说明**：V 轨复用既有 `plan-bot-e2e-coverage-v1`，不占新编号——确认促升时版本号沿用 v1 还是升 v2（其 P0 已完成，建议原版本续写）。
2. 调度会话由谁跑（用户手动 / 一个常驻 claude 会话）；波次放行的判定权归属。
3. build token 的并发上限是否可按本机内存实测上调（默认 cargo≤2/gradle≤1）。
4. #1289 e2e 红的根因（自称 agent npm 依赖问题）需在基线阶段查实。
5. **Headless 客户端的接入身份与认证**：生产为 offline mode，握手 `Username` 可由客户端冒充（R4 P0 决议 5）；agent 常驻玩家的账号/凭证模型（一 agent 一角色？凭证如何签发/轮换/撤销）未定，且按同决议不得向 offline transport 引入可重放 bearer credential。是随 R3/R6/R4 owner amendment 纳入本计划族，还是独立 plan，需人工拍板。
6. **`request_rejected` 的 owner phase 与 wave 槽位**（§4.3 张力 #1）：R4 已命名字段与 R6 契约流程，R6 phase 未登记；需 R6 amendment 定义 phase/artifact/consumer，再 amendment §3 裁决其相对 R4 Wave 2 production activation 的顺序（是否同一 merge unit）。
7. **agent 客户端的背压信号**（§4.3 张力 #2）：ingress 预算耗尽在 decode 前静默丢弃，headless 客户端无法与网络丢包区分。是否提供机器可读的 RateLimited 信号、如何在安全折叠下不成为探测 oracle、预算数值（32 容量 / 每 tick +8）是否按 agent 玩法校准，均未定。
8. **服务端降级态的机器可读信号**：R3 载入守护"只读降级"等服务端全局/玩家级降级态，headless 客户端如何得知（现状仅人类可感反馈）；signal 形状归 R6，需 owner amendment 定义。
9. **`scripts/bot/` 的定位与挂机经济边界**：bot 框架是促升为受支持的 reference headless client（含协议兼容承诺与版本纪律），还是仅测试工具？agent 7×24 挂机刷宝对匮乏经济（worldview §一）的长时段影响——真元守恒（R5）已防 mint，loot/discard 配额（R10）已有界，但自动化收益速率本身未校准——是否需要独立评估/plan。

## 10. flash-review issue 消化流程（2026-08-02 增补，用户指示）

背景：flash-review 只读扫描会话（独立 tmux，deepseek-v4-flash 全仓扫描）持续对本仓提 GitHub issue（label `flash-review`，标题带 [blocker]/[major]/[minor] 分级），2026-08-02 已 389 个 open 且持续增长。本节把 issue 消化正式纳入计划族闭环，防止重构收官后积压无人认领。

### 10.1 在途 triage（重构进行期间，调度会话周期跑）

1. **节奏**：每积累 ~100 个新 issue 或每轮 sweep 收口后跑一批；只做 issue 操作与源码只读核对，不改代码。
2. **去重**：同根因多 issue 收敛为一个（保留证据最全者），其余以 `dup of #N` 评论关闭。
3. **验真**：flash 模型误报率高——blocker/major 逐个对照源码验真；minor 按目录抽查。验伪的关闭并留结论证据。
4. **归轨**：每个验真 issue 必须恰有一个实现 owner label：`track:R1`、`track:R2`、`track:R3`、`track:R4`、`track:R5`、`track:R6`、`track:R7`、`track:R9`、`track:R10`、`track:V`、`track:registry-datafication`、`agent`、`worldgen` 或 `standalone`；`R8` 是编号空缺，绝不打 `track:R8`。已合入 `origin/main` 的 PR 经复核确实覆盖后才评论关联并以 fixed 关闭；仅在飞的 PR 只评论关联、保留 issue open，待其合入后复核再关闭，PR 撤回/未覆盖则回到本步骤重新归轨。
5. **升级出口**：blocker 验真即入调度队列单独修；可由既有轨道吸收的 major 随该轨道收尾；需独立立项的 major 聚类走 §10.1.1；minor 留 label 等批量窗口。

### 10.1.1 major 聚类促升 skeleton（独立出口）

1. 调度会话先在 cluster intake 中列出 source issue、验真证据、唯一 owner label 和**明确的 implementation owner**（既有轨道，或命名的 standalone 工人）；未定 owner 的 source issue 保持 open，不得以“待建 skeleton”关闭。
2. 需独立立项时，owner 必须在本仓库同一提交树内读取已存在的根 `CLAUDE.md`「Plan 工作流」和 `docs/CLAUDE.md` §§五-六（Plan 演进 / consume-plan），并在独立 docs PR 中创建或补充 `docs/plans-skeleton/plan-<name>-v1.md`；不得用外部或会话文档替代，任一路径缺失时不得创建 skeleton 或关闭 source issue，先转人工恢复仓库流程文档。该 skeleton 按普通 plan 工作流进入调度/消费队列。此出口不走 §7，§7 只处理已被轨道吸收的 plan 归档。
3. skeleton 合入 `origin/main` 且 implementation owner 已入队后，triage 才在每个 source issue 留下 skeleton 路径、commit/PR 与 owner 的关联证据，并以 `promoted to <skeleton>` 关闭；任何一步未完成都保留 source issue open。

### 10.2 轨道收尾挂钩

每轨进入最后一个 implementation phase 前，必须扫描本轨 owner label 下的 open issue：能修的纳入该最后 phase PR，或另开一个 closeout implementation PR；两者都必须合入 `origin/main` 后，才可跑 §7 的 docs-only 批量归档。不能由本轨修的 issue 保持 open，并评论移交后改到其唯一接收 owner；不得用归档 PR 携带代码修复，也不得把“已关联在飞 PR”当作已结案。

最后一个 implementation PR 合入后、§7 归档前再查一次本轨 owner label：发现仍需本轨代码的 issue 就新开 closeout implementation PR 并重复本段；发现可移交或促升的 issue 则按 §10.1/§10.1.1 完成其 open 状态迁移。仅当本轨没有 open 或待合入的 issue，才可提交 docs-only 归档 PR。

### 10.3 完成清算（§8 第 5 条的执行细则）

九轨归档后，`flash-review` 生产者仍在运行时不得宣告计划族完成。调度会话必须先请求 producer 停产、等待正在执行的最后一轮 sweep 结束并确认其已写完全部 issue；记录 final-sweep watermark（sweep ID、扫描的 `origin/main` SHA、该轮产出的 issue ID 集合），且 producer 在下列清算与归档期间持续停止。

以该 watermark 为边界，`flash-review` label 下每个仍 open 的 issue 只可完成为 fixed（关联已合入 PR）/ dup / 验伪关闭（留结论）/ 按 §10.1.1 促升 skeleton。若残量 >50，开专门收尾窗口（1-2 工人）批量消化，调度会话排期跟踪；在 producer 停止的前提下，复查 open issue 为零、无待合入关联 PR 后才归档计划族并在完成证据中写入 watermark 与最终查询结果。此后若要恢复 flash-review，必须先建立并指定一个 successor owner/plan 接管新 issue；不得在本计划族完成屏障内恢复 producer。

### 10.4 职责边界

sweep（产 issue 与停产确认）= flash-review 独立会话；triage（分类/验真/关闭、owner 指派、final-sweep 清算）= 调度会话；修复 = 工人正常 PR 流程。三者不互相越界；工人不得自行触发 review。
