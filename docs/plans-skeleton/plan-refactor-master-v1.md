# plan-refactor-master-v1 — Client/Server 大重构总纲（重构计划族 R1-R10）

一句话：230+ 份点状 bughunt/feature plan 背后反复出现的是 **8 个系统性根因**（session 生命周期各写各的、持久化各漏各的、qi 账本可绕、C2S 无门禁、S2C 双轨散装、client store 断线裸奔、UI 无基类、AV 无单一事实源）。本计划族用 9 条重构轨道一次性把根因变成**共享基础设施**，以协议级 bot e2e 为主验收门，代码目标是干净直接无面条（拆 3 个 2 万行级 god file）。

> 撰写依据：2026-07-27 五路侦察（server/client 结构地图、84 active plan、146 skeleton、16 开放 PR 全量盘点）。各轨道 skeleton 文件见 §2 表。

## 0. 范围与铁律

- **重构实施只改 `server/` + `client/`**。`agent/`、`worldgen/`、`library-web/` 不由 R1-R10 修改，相关 plan 独立保留（§6.11-6.12）；R 轨需要 agent-side schema 变更时，必须由 §6.11 Agent 轨作为独立 production owner 先交付，R 轨只消费其冻结产物。
- 对外契约（Redis IPC、proto schema）原则上不动形状；确需变更走 buf breaking + samples 同步，agent 侧只做被动 regenerate。**不写兼容层**——切换一次到位，删旧路径。
- 真元守恒律、worldview 正典、招式 A/V 差异化红线全部继续生效。
- **测试方针（用户 2026-07-27 指示，仅限重构轨道，覆盖根 CLAUDE.md「饱和化测试」节）**：
  1. **bot e2e 场景是主验收门**——每条轨道自带 3-8 个 `scripts/bot/scenarios/` 场景，先于/伴随重构落地；
  2. 单测只保留**契约 pin**：schema sample 对拍、守恒断言、状态机转换、注册表强制扫描；
  3. 与被删实现绑定的旧单测允许随代码删除；不要求饱和覆盖；
  4. feature plan（非重构轨）不适用本条，仍按根 CLAUDE.md。
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
| R6 | `plan-refactor-wire-s2c-v1` | emit builder + client 双轨归一 + 作用域广播 | `network/*_emit.rs`、`proto_convert.rs`、client `network/` | ~12 |
| R7 | `plan-refactor-client-ui-base-v1` | Screen 基类 + InspectScreen 拆解 + 输入/线程纪律 | client Screen/hud/keybind | ~17 |
| R9 | `plan-refactor-cast-av-contract-v1` | cast_sync 契约 + SkillAvBinding 单一事实源 | server cast/AV emit + client cast store | ~13 |
| R10 | `plan-refactor-inventory-core-v1` | inventory 巨石拆分 + InventoryTxn 事务 | `server/src/inventory/**` | ~7 |
| V | `plan-bot-e2e-coverage-v1`（既有 skeleton 直接促升，不另立） | bot 场景 P1-P6 扩容 + CI 假绿修复 + build token 脚本 | `scripts/bot/**`、CI | ~9 |
| 基建 | `plan-registry-datafication-v1`（既有 skeleton 直接促升） | 硬编码配方/功法/方块表迁数据 + fail-fast | 三张表 | 自身 |

## 3. 波次与依赖

- **Wave 0（立即并行）**：V（bot 骨干 + build token 最先）、R3、R5、R2、registry-datafication；同时全部轨道的 P0（设计收口 + 吸收清单验真）都可开工。
- **Wave 1**：R6（R2 合入后）、R7 基础设施（R2 合入后）、R1 framework-only（仅 `InteractionSession`/registry/lifecycle 骨架，R3 P1 合入后；不得宣称 craft pause/resume 或 delivery 生产闭环）。
- **Wave 2**：R4（#1287 + R6 P1 后）、R9（R5/R6/R2 P1 后）、R10（R3 P1 后）；R1 宿主迁移按显式 gate 分批放行：craft 需 R3 P1 + R6 craft intents + R4 craft handler/gate + R7 P2 Craft Screen + R2 P1 已登记的 `CraftStore` + R10 P1 `deliver` contract + R10 P2 craft production delivery，alchemy/forge 同样需 R10 P2 对应生产调用点，`TsyPresence` 需 R3 P1 auxiliary Slice 与 R3 P4 restore parity。
- 近完成独立 plan（§6.9）在 Wave 0 窗口内优先收尾清场。
- R5 P1（字段收私有的全仓编译大爆破）挑在飞 PR 队列清空的窗口单独合入。

## 4. 文件所有权矩阵（防并行打架，冲突时以本表为准）

- `persistence/**`+autosave=R3；`session/`+7 域 session.rs=R1；`client_request_handler.rs`+`gate/`=R4；`*_emit.rs` 公共层+`proto_convert.rs`=R6；`qi_physics/**`+qi 字段直写行=R5；`inventory/**`=R10；cast/AV emit+skill 注册=R9。
- client：Store 生命周期+`clearClientStateOnDisconnect` 区段=R2；channel 注册区段+桥+router=R6（与 R2 同文件不同区段，merge 前互 fetch）；Screen/hud/keybind/InspectScreen=R7；combat cast store=R9。
- `agent/packages/schema/src/**`、generated JSON Schema 与提交的 `@bong/schema` dist=§6.11 Agent 轨；craft schema prerequisite 的 production owner 是 Agent 轨的独立 craft-schema 交付批次，必须在 R6 P1 前提交并验收 TypeBox source、generated schema/dist 与变体计数。理由：TypeBox 是 agent-side source of truth，生成物必须与 source 同 owner 原子提交；让 R6 同时改 source 和 wire 会违反本总纲的 agent 排除边界并形成双 owner。R6 独占 proto/Rust mirror/converter、client wire encode/send 与 bridge/router plumbing，只消费该冻结版本并负责一次性 wire 反映。R6、R4、R7、R1 的 craft gate 均以本处 ownership 决议为准。
- CraftOpen target bridge（跨轨 canonical contract）：`CraftOpen` 必须携带 required `target` 判别联合：`Handcraft` 或 `Workbench { workbench_key }`，不得省略。`workbench_key` 是现有成功 S2C `WorkbenchOpen.entity_id` 的 ECS `Entity::to_bits()` locator：逻辑/Rust 类型 `u64`、protobuf `uint64`、JSON/TypeBox 为无符号十进制字符串；它不是授权能力。普通手搓发送 `Handcraft`；`WorkbenchScreen` 必须从 response 保留该 key 并在初次 `CraftOpen` 原样回传。R4 将 key 解析为 entity 后重验实体存活且携带 `WorkbenchBlock`、玩家同维且在既有距离内，并执行 owner/busy/facility gate；R1 仅在校验通过后建立 facility claim。missing、malformed、stale、despawned、跨维或越距 key 均拒绝；R7 不得从 UI 猜测或改写 key。`CraftPause`/`CraftResume` 只携带 hydrated session identity/version，不重复 target。
- 任何轨道碰他轨文件：只允许"消费对方冻结后的 API"，不允许改对方独占文件；接缝 API 归被依赖方定义。

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
3. `qi_current` 裸写编译不过；client 无未登记的会话态 store；116 C2S 变体全部有显式 GateSpec/no_gate 声明；28 旁路 channel 收编或豁免登记；
4. bot 场景数从 ~30 增至 ≥80，CI e2e 是唯一主门禁且无已知假绿。
5. `flash-review` label 下 open issue 全部显式处置（fixed / dup / 验伪关闭 / 促升 skeleton，见 §10），无静默积压。

## 9. 开放问题（总纲级，pre-P0 收口）

1. **R8 编号空缺说明**：V 轨复用既有 `plan-bot-e2e-coverage-v1`，不占新编号——确认促升时版本号沿用 v1 还是升 v2（其 P0 已完成，建议原版本续写）。
2. 调度会话由谁跑（用户手动 / 一个常驻 claude 会话）；波次放行的判定权归属。
3. build token 的并发上限是否可按本机内存实测上调（默认 cargo≤2/gradle≤1）。
4. #1289 e2e 红的根因（自称 agent npm 依赖问题）需在基线阶段查实。

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
