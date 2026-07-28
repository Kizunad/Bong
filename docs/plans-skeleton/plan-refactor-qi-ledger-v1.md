# plan-refactor-qi-ledger-v1 — 真元守恒 Ledger 架构强制化（重构轨 R5）

> 所属总纲：`plan-refactor-master-v1.md`。一句话：把 `qi_physics::ledger` 从"参与式记账"升级为"架构强制"——`qi_current`/`zone.spirit_qi` 的裸算术写入在类型层面不可能，全部真元流动只剩 `QiTransfer` 一条路；mint/蒸发/zone-shadow 整簇（20+ 份 plan）根除。

## 现状证据（2026-07-27 侦察）

- ledger 本体是真的复式记账（`ledger.rs:416` transfer 内做守恒校验、`:771` assert_conservation），`QiTransfer::new` 45 文件 ~130 处覆盖主干。
- 但 `Cultivation.qi_current`（`cultivation/components.rs:631`）是裸 pub 字段：全仓 `.qi_current =` 赋值 164 处、`+=/-=` 34 处（例 `cultivation/tick.rs:294` 无对应 zone 扣减）。
- `Zone.spirit_qi` 直接赋值 260 处（含 wire DTO 填充，需甄别）。
- bughunt 已确认的同构漏洞遍布：招式 overflow 只发事件不写账、boss/骨煞 drain 只写影子账不写 ZoneRegistry、NPC 日程回气凭空铸、qi_max 缩容 clamp 蒸发、负灵域 max(0) 抹赤字、蓄力打断退款铸真元、服丹直加 qi_current……全是"裸字段可写"派生的变体。
- 历史教训（docs/CLAUDE.md §四）：emit-only `QiTransferReason::AbstractCombat` 无 system 消费 = 真元蒸发——修法是**直接改账本状态**，不是再发一层事件。

## 接入面

- **进料**：全部触真元的域（cultivation/combat 各流派/alchemy/dandao/lingtian/fauna/npc/zhenfa/tsy/yidao/bonecoin）。
- **出料**：`WorldQiAccount`/`ZoneRegistry.spirit_qi` 单一事实源；`summarize_world_qi` 守恒审计恒绿。
- **共享类型**：`qi_physics::ledger` 既有 API 扩充（不新造第二套）；`SPIRIT_QI_TOTAL` const 引用（测试禁写字面 100）。
- **worldview 锚点**：worldview.md §二/§十 守恒律；唯一系统外流 = 天道时代衰减（`era_decay_accum` 沉降槽不变式）。
- **qi_physics 锚点**：本轨就是 qi_physics 的强制化本身；任何新常数先扩 `qi_physics::constants`。

## 阶段

- ⬜ P0 设计收口 + 吸收清单验真：~200 个直写点全量普查分类（合法初始化/regen/衰减/战斗/UI 镜像）；冻结封装方案（`qi_current` 收私有 + 类型化访问器：`gain_from_zone` / `release_to_zone` / `transfer_to` / `set_for_init`，每个访问器内嵌 ledger 记账）；负灵域语义、qi_max 缩容语义写成决议。
- ⬜ P1 类型封装落地：字段收私有，访问器上线；既有 ~130 处 QiTransfer 调用点平移；编译期扫清全部直写（编译器就是审计器）。
- ⬜ P2 修复批次 A（cultivation + 消耗品 + lingtian）：regen/服丹/plot_qi/经脉淬炼影子账全部走访问器归账。
- ⬜ P3 修复批次 B（combat 各流派 + fauna/npc/boss + 两项跨轨 API handoff）：overflow/drain/打断退款/日程回气/暗器 imprint 回滚；离屏死亡走 `release_dormant_qi_to_zone`；交付 distance effect/account 分腿与 scatter-bead 原子 release+close API（详见下节）。
- ⬜ P4 守恒审计常绿 + 归档：`assert_conservation` 进 bot e2e 每场景收尾断言；吸收 plan 批量归档。

## P3 跨轨冻结 API 与放行条件（R5 canonical owner）

以下两项登记为 R5 P3 的**实际实施队列**，不是 master 旁注，也不把下游 focused finding 整体吸收进 R5。R5 独占 `server/src/qi_physics/**` 的类型、helper、事务与静态 misuse gate；下游只消费合入后的冻结 API。

### P3-A distance effect/account 分腿 → `distance-decay-calibration`

- **R5 交付**：在 `server/src/qi_physics/distance.rs` 提供强类型 `DistanceEffectCoefficient` 与 ratio-only `distance_effect_coefficient(distance_blocks, medium, env) -> DistanceEffectCoefficient`；参数不得含 initial qi amount/account/ledger，返回值有限且钳在 `[0,1]`。删除可调用的 `qi_distance_atten(initial, ...)` / `qi_distance_atten_in_env(initial, ...)`，并把 carrier/collision 中混用的 effect leg 与真实 qi balance/transfer leg 分开；真实 source→target/zone/overflow 余额不乘 coefficient。
- **量纲与守恒 pin**：0/10/50 格分别 pin 数值锚、单调性及非法输入；完整命中→污染 tick→排异 fixture 逐项断言 source/target/zone/overflow、玩家 `qi_current`、`Contamination.entries`/amount、排异 qi delta 与 transfer reason/audit 数量跨距离完全不变。static gate 扫描 `DistanceEffectCoefficient` consumer，流入 `QiTransfer::amount`、任何 balance/residual/release、`ContamSource.amount` 或污染资源字段即红。
- **放行条件**：R5 P3-A PR 已合入；旧 amount-shaped API 和调用方为零；ratio-only API/量纲/static misuse/账户零变化测试全绿。满足后才向 `plan-bughunt-distance-decay-calibration-v1` 发出冻结 API handoff；focused plan 只拥有双锚点校准、combat effect consumer 与 bot damage/纯展示 feedback 验收。

### P3-B 原子 release+strict-close → `scatter-bead-ledger-account-cleanup`

- **R5 交付**：在 `server/src/qi_physics/ledger.rs` 冻结通用事务 API `release_all_to_zone_then_close(source, ZoneReleaseTarget { zone, overflow, current, capacity }, reason) -> Result<CloseReceipt, QiLedgerError>`（最终 Rust 命名可在 P0 统一，但输入/返回和原子语义不可弱化）：从 ledger 读取真实 source balance，校验 target snapshot 与 ledger/zone mirror 一致，有限正余额按 capacity 逐腿转入 zone/overflow，source **严格等于 `0.0`** 后才删除 key；`QI_EPSILON` 只作比较/断言容差，不授权销毁残量。
- **原子性与守恒 pin**：普通 zone、满 zone→overflow、初始严格零、`0 < balance <= QI_EPSILON`、source 缺失、非有限余额、zone snapshot/ledger mirror 不一致、zone lookup/任一 transfer 失败全覆盖；失败时所有 balance/audit 与 source key 原样保留，成功时逐腿 amount/reason/audit 对拍且 key 不再出现在 `iter_balances`。static gate 禁止调用方以裸 `remove_balance` 代替 release。
- **放行条件**：R5 P3-B PR 已合入并通过逐腿守恒、失败原子性、strict-zero 与 misuse pins 后，才向 `plan-bughunt-scatter-bead-ledger-account-cleanup-v1` 发出冻结 API handoff；focused plan 只拥有 zhenfa burial/直接使用/自然耗尽的领域编排、失败 reinstate、restart/telemetry 与 bot/server 验收。

## 吸收清单（短名省略 plan-bughunt- 前缀与 -v1 后缀）

active：anqi-throw-imprint-drop、baolongwang-bossdrain-zone-shadow（#1296 已闭环则只核验归档）、dandao-skill-overflow-ledger、lingtian-plot-qi-ledger-gap、locust-zone-qi-ledger、meridian-forge-zone-shadow、npc-daily-life-qi-mint、qi-needle-negative-zone-release、qi-recovery-consumable-ledger。
skeleton：attrition-overflow-ledger、bonecoin-qi-facevalue、carrier-resonance-seal-mint、dormant-negative-qi-release、fullpower-interrupt-refund-mint、heartbeat-pseudo-vein-qi-mint、qimax-shrink-clamp-leak、skull-fiend-drain-zone-shadow、tsy-entry-filter-ledger、yidao-healing-cap-leak；在飞 #1294：botany-growth-cost-harvest-ledger。
跨轨 API handoff（R5 只承接 `qi_physics/**` 切片，不吸收 focused gameplay owner）：`distance-decay-calibration` → P3-A；`scatter-bead-ledger-account-cleanup` → P3-B。

## 文件所有权与边界

- 独占：`server/src/qi_physics/**`、`Cultivation.qi_current`/`Zone.spirit_qi` 字段定义及全部直写行的改造。
- 冲突面最大的轨：改动是"横切的行级替换"，遍布 cultivation/combat——与 R9（combat AV/cast）约定按文件排队：本轨先合，R9 rebase；与 R1/R3 只在"session 返还/持久化恢复"两个接缝碰面，接缝 API 归本轨定义。
- 依赖：无前置，Wave 0 即可动工（P1 的字段收私有是全仓编译大爆破，选一个在飞 PR 队列清空的窗口合入）。P3-A/P3-B 分别是 distance/scatter focused implementation 的硬前置；未满足各自放行条件时，下游保持 BLOCKED。

## bot 验收场景

1. `qi_conservation_sweep`：bot 顺序执行修炼/施法/服丹/采集/受击/死亡复活一整轮，每步后由 dev 命令拉 `summarize_world_qi`，断言 `current_total + era_decay_accum == initial_total`（取 const 引用）。
2. `qi_skill_roundtrip`：单招释放前后玩家+zone 总量不变。
3. `qi_death_release`：击杀带真元的 NPC/离屏战死→断言 zone 收到等额释放。
4. `qi_negative_zone`：负灵域内释放/吸收→断言赤字被记账不被 max(0) 抹平。

## 开放问题（pre-P0 收口）

1. UI/HUD 镜像读值走只读快照 accessor 的命名与位置（避免 client 镜像被误判为直写）。
2. 负灵域（负 spirit_qi）的正典语义边界——需查 worldview §十并人工确认。
3. P1 大爆破 PR 的切分策略：一次全仓 vs 按模块 feature-gate 渐进（倾向一次全仓 + 并行断言期，不留双轨）。
