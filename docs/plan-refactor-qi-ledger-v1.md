# plan-refactor-qi-ledger-v1 — 真元守恒 Ledger 架构强制化（重构轨 R5）

> 所属总纲：`docs/plans-skeleton/plan-refactor-master-v1.md`（草案权威）。一句话：把 `qi_physics::ledger` 从“参与式记账”升级为“架构强制”——`qi_current` / `zone.spirit_qi` 的 gameplay 裸算术写入在类型层面不可能，全部真元流动只剩受控事务入口；mint、蒸发、zone-shadow 整簇根除。

## 现状证据（2026-07-27 侦察）

- `WorldQiAccount::transfer` 是现有真实余额事务入口；`QiTransfer` event / `push_transfer_audit` 只留轨迹，不是状态消费者。全仓没有生产 `EventReader<QiTransfer>` 负责代替物理结算。
- `Cultivation.qi_current` / `qi_max` / `qi_max_frozen` 当前仍为 `pub`。127 个 lexical 候选经逐点验真后：24 个噪声、1 个字段定义、2 个只读投影、100 个生产初始化或 mutation 写点。
- 100 个生产 `qi_current` 写点互斥分类：`set_for_init` 3、ledger→ECS mirror 2、`gain_from_zone` 2、`release_to_zone` 39、`transfer_to` 30、`resize_with_release` 15、dev-only 2、source/sink 未收口高危点 7。
- `Zone.spirit_qi` 275 个精确 mutation 候选经逐点验真后：187 个测试/fixture、19 个注释或字符串伪命中、69 个 runtime mutation。69 个 runtime 点分类：初始化/恢复 2、dev bypass 1、zone→actor 9、actor→zone 31、zone↔ledger/container/pending 7、world physics/lifecycle/control 19。
- 高危七点必须先定义真实 source / sink / escrow，不能机械替换：`recover_current_qi` 无来源 mint；full-power strike 释放后退款双记；伪皮制作无 sink；attack investment 无 escrow；服丹直加并 cap；涡流 cast debit 无 sink；void barrier dispel 无 sink。
- 长期把 Zone / live actor / item 同时镜像进 `WorldQiAccount` 会被 `summarize_world_qi` 与真实字段双计；`world/heartbeat.rs`、`npc/dormant/mod.rs` 的 shadow 路径属于 P0 基础事务要拆除的反例。
- 正典：`worldview.md §二 L30-L50` 定义正域 / 死域 / 负灵域及坍缩渊可低至 `-1.2`；`worldview.md §十 L870-L880` 定义灵气零和与缓慢重分配；`docs/CLAUDE.md §四 L49-L60` 明令禁止裸增减、emit-only 结算与离屏吞真元。

## 接入面

- **进料**：全部触真元的域（cultivation、combat 各流派、alchemy、dandao、lingtian、fauna、npc、zhenfa、tsy、yidao、bonecoin）。
- **出料**：live actor 真元由 `Cultivation` 持有；signed 环境灵压由 `Zone` 持有；无其他物理 owner 的稳定池由 `WorldQiAccount` 持有；`QiTransfer` / ledger audit 是同一事务提交后的可观察轨迹。
- **共享类型**：只扩 `qi_physics::ledger` 与 `Cultivation` 的受控事务 API，不创建第二套 ledger；测试引用 `DEFAULT_SPIRIT_QI_TOTAL` / `WorldQiBudget`，不写死历史占位值。
- **跨仓库契约**：本轨 P0-P3 是纯 server 内部重构；P4 bot e2e 通过既有 dev telemetry 校验，不新增 agent/client wire 形状。
- **worldview 锚点**：`worldview.md §二 L30-L50` 灵压环境；`worldview.md §十 L870-L880` 零和资源。
- **qi_physics 锚点**：`qi_physics::ledger::{WorldQiAccount, QiTransfer}`、`qi_physics::release::qi_release_to_zone`、`qi_physics::constants::QI_ZONE_UNIT_CAPACITY`；不新增旁路公式。

## 阶段

- ⏳ P0 设计收口 + 吸收清单验真：完成全部候选分类；冻结 owner 模型、类型化事务入口、signed-zone、缩容、持久化与稳定 overflow 语义；用饱和单测锁失败原子性。
- ⬜ P1 类型封装落地：`Cultivation.qi_current` / `qi_max` / `qi_max_frozen` 与 `Zone.spirit_qi` 收私有；既有调用按受控 API 平移；all-target 编译扫清全部生产裸写。
- ⬜ P2 修复批次 A（cultivation + 消耗品 + lingtian + botany）：regen、服丹、plot qi、经脉淬炼影子账、attrition、骨币面值、qi_max 缩容、植物生长/采收全部归账。
- ⬜ P3 修复批次 B（combat + fauna/npc + world/tsy + inventory pickup）：overflow、drain、日程回气、暗器 imprint、locust、负域针、dormant、骨煞、TSY filter、医道 cap leak；离屏死亡统一真实释放；为 R10 `PickupAttritionBasis { target_instance_id, incoming_instance_id, incoming_stack_count, incoming_abs_qi_before }` 提供稳定 incoming-only pickup attrition API，只磨损 incoming 绝对真元并把损耗守恒归还 zone，merge 后不得重扣既有 stack 的绝对真元。
- ⬜ P4 守恒审计常绿 + 归档：`assert_conservation` 进入 bot e2e 场景收尾；吸收项归档；补齐 `## Finish Evidence` 后迁入 `docs/finished_plans/`。

## P0 冻结契约

### 1. Owner 模型

- live actor current qi：`Cultivation` 是唯一物理 owner，`WorldQiAccount` 不长期镜像 player / npc balance。
- signed zone pressure：`Zone.spirit_qi` 是唯一物理 owner，ledger 不长期镜像 `zone:*` balance。
- inventory/item/container qi：由对应 item / container 物理字段持有；进入无 owner 稳定池时才真实 credit ledger。
- ownerless durable qi：仅进入 `persistent_runtime_qi_accounts()` 完整枚举、snapshot、hydrate 的固定账户。
- `WorldQiBudget` 只承载宏观预算与 `era_decay_accum`，不充当 gameplay 转账账户。

### 2. 类型化 API

- `set_for_init(CultivationQiInit)`：仅构造、验证后的持久化恢复、迁移和测试 fixture；finite、非负、`current <= max`、`frozen <= max * BREAKTHROUGH_FAIL_FROZEN_CAP_RATIO`，失败零写入。
- `gain_from_zone`：同一调用完成 signed zone debit、actor credit 与 audit；`zone <= 0` 是真 no-op，room 不足的余量留在 zone。
- `release_to_zone`：同一调用完成 actor debit、signed zone credit、固定 `qi_flow_overflow` 真实 credit 与 audit；强制 `source_debited == zone_accepted + overflow_credited`。
- `transfer_to`：目标只能是绑定 canonical `LifeRecord` identity 的 actor capability，或 `PersistentQiSink` 固定白名单；禁止调用者把任意 `QiAccountId` 冒充物理 target / durable sink。
- `resize_qi_max_and_release_excess`：先原子释放 `max(current - new_max, 0)`，成功后再更新 max/current/frozen；失败时 actor、zone、ledger、audit 全不变。
- `qi_snapshot()` / `qi_current()` / `qi_max()` / `qi_max_frozen()`：只读投影；不得返回 `&mut f64`、通用 setter 或 `DerefMut`。
- dev command 另保留显式 `set_for_dev_only`，只能由 dev gate 调用，不进入 production gameplay。

### 3. 失败原子性与 audit

- 数值、same-account、insufficient、destination overflow、identity、zone state 全部在外部物理字段提交前 preflight。
- 真实 ledger 写失败时，不得改变 actor、zone 或 audit history。
- `QiTransfer` event / `push_transfer_audit` 永远不能替代真实 owner debit/credit；只在真实状态成功提交后投影。
- outcome 的 `transfers` 顺序必须与 ledger audit 实际提交顺序一致，不承诺按 target kind 排序。

## P2/P3 吸收清单（以 2026-07-27 origin/main 验真为准）

### P2 吸收

1. `dandao-skill-overflow-ledger`
2. `lingtian-plot-qi-ledger-gap`
3. `meridian-forge-zone-shadow`
4. `qi-recovery-consumable-ledger`
5. `attrition-overflow-ledger`
6. `bonecoin-qi-facevalue`
7. `qimax-shrink-clamp-leak`
8. `botany-growth-cost-harvest-ledger`

### P3 吸收

1. `anqi-throw-imprint-drop`
2. `locust-zone-qi-ledger`
3. `npc-daily-life-qi-mint`
4. `qi-needle-negative-zone-release`
5. `dormant-negative-qi-release`
6. `skull-fiend-drain-zone-shadow`
7. `tsy-entry-filter-ledger`
8. `yidao-healing-cap-leak`

### 隔离与排除

- 隔离：`carrier-resonance-seal-mint`、`fullpower-interrupt-refund-mint` 有历史远端 claim/ref 责任边界，本轨不抢占；API 冻结后由原任务迁移。
- 排除：`baolongwang-bossdrain-zone-shadow` 已由 #1296 闭环；`heartbeat-pseudo-vein-qi-mint` 生产 injection/settlement 已修，陈旧 skeleton 不作为待实施缺陷，但 heartbeat 的长期 zone mirror 由本轨 P0/P1 基础事务清理。

## 文件所有权与边界

- 独占：`server/src/qi_physics/**`、`Cultivation.qi_current` / `qi_max` / `qi_max_frozen`、`Zone.spirit_qi` 字段定义及生产直写迁移。
- 冲突面最大：P1 是全仓行级横切替换。落地前必须 `gh pr list --state open`，等大型代码 PR 窗口清空；紧邻 `git fetch origin && git merge origin/main`，快速全量私有化，不留 public 双轨。
- P0 persistence migration version 必须在 push 前以最新 `origin/main` 为准递增；不得与在飞 migration 复用同一 `PRAGMA user_version`。
- R1/R3 接缝仅限 session 返还与持久化恢复；R5 定义 qi transaction / init boundary，R1/R3 调用。

## bot 验收场景

1. `qi_conservation_sweep`：bot 顺序执行修炼、施法、服丹、采集、受击、死亡复活；每步后断言 `current_total + era_decay_accum == initial_total`。
2. `qi_skill_roundtrip`：单招释放前后 actor + zone + stable ownerless pool 总量不变。
3. `qi_death_release`：击杀带真元 NPC / 离屏战死，断言 signed zone 与 overflow 合计收到等额释放。
4. `qi_negative_zone`：负灵域内释放先偿还赤字；普通吸收为 0；不得用通用 `.max(0)` 抹 signed state。
5. `qi_pickup_merge_incoming_only`：已有 stack 与同 identity dropped item 合并，断言 attrition 只基于 receipt 的 `incoming_abs_qi_before`，既有 stack 原绝对真元不变、目标 stack + zone 总量守恒，且不依赖 consumed dropped instance id 在 inventory 中仍存在。
6. 所有新 server 模块同时补 `scripts/bot/scenarios/` 场景并进入 CI bot e2e stage。
1. `qi_conservation_sweep`：bot 顺序执行修炼、施法、服丹、采集、受击、死亡复活；每步后断言 `current_total + era_decay_accum == initial_total`。
2. `qi_skill_roundtrip`：单招释放前后 actor + zone + stable ownerless pool 总量不变。
3. `qi_death_release`：击杀带真元 NPC / 离屏战死，断言 signed zone 与 overflow 合计收到等额释放。
4. `qi_negative_zone`：负灵域内释放先偿还赤字；普通吸收为 0；不得用通用 `.max(0)` 抹 signed state。
5. 所有新 server 模块同时补 `scripts/bot/scenarios/` 场景并进入 CI bot e2e stage。

## 开放问题（pre-P0 收口）

1. UI/HUD 镜像读值走只读快照 accessor 的命名与位置。
2. 负灵域（负 `spirit_qi`）的正典语义边界。
3. P1 大爆破 PR 一次全仓还是渐进双轨。
4. `qi_max` 缩容与 `qi_max_frozen` 边界。
5. 持久化 restore 是否产生 transfer。
6. zone missing/full overflow 的可恢复落点。

全部已在下节收口。原表保留以备追溯，**实施时以下述 §7.1 决议为准**。

## §7.1 决议（pre-P0 收口，2026-07-27）

### #1 UI/HUD 只读投影

**决议**：
1. `Cultivation::qi_snapshot()` 返回值 DTO `CultivationQiSnapshot { current, max, frozen, effective_max, room }`；简单读点可用标量 getter。
2. API 不返回字段引用，不允许 client/schema 投影层获得 mutation capability。
3. P1 私有化时，schema、HUD、action snapshot 只迁移读法，不创建 `QiTransfer`。

**落点**：`server/src/cultivation/components/qi_flow.rs:226-255`（`Cultivation::{qi_current, qi_max, qi_max_frozen, effective_qi_max, qi_room, qi_snapshot}`）/ plan “P0 冻结契约 §2”与 P1。

### #2 signed 负灵域语义

**决议**：
1. `Zone.spirit_qi` 保持 signed；普通权威 `server/zones.json` 的加载路径才使用 `SpiritQiFloor::RuntimeBounded`，其运行时下限为 `-1.0`（`server/src/world/zone.rs:25-27`、`:631-641`、`:699-715`）。这是普通 authoritative path 的配置边界，不是全局 canonical signed-zone floor。
2. `ZoneRegistry::load()` 先加载普通配置，再通过 `merge_tsy_blueprint_from_path` 合并 TSY blueprint，并使用 `SpiritQiFloor::TsyBlueprint`（`server/src/world/zone.rs:225-234`、`:401-426`）；该路径不套用普通 `-1.0` floor。`server/zones.tsy.json:76`、`:170`、`:274` 已有 `-1.1` / `-1.15`，TSY/collapse signed pressure 必须保留到正典示例 `-1.2` 的语义，不能为了普通 runtime path 而统一数值。
3. R5 transaction/release helpers（`qi_release_to_zone`、`release_to_zone`、`release_external_qi_to_zone` 及其 ledger 提交边界）必须原样保留并提交 signed zone value；禁止用通用 `clamp(-1.0, 1.0)` 或等价写回覆盖 signed state。actor 向负域释放正量时先偿还赤字；zone `<= 0` 时普通吸收收益为 0 且 zone 原值不变。
4. `.max(0)` 只允许计算正向 availability 或消除吸收后的浮点尾差，不能用于通用 signed state 写回。后续必须把所有 zone mutation caller 扫描并迁移到统一 signed-zone API，已知 offender 包括 `server/src/world/tsy_lifecycle.rs:533`；在该 sweep 完成前，不得重新考虑扩大普通 authoritative runtime floor。

**落点**：`docs/worldview.md:30-50`、`server/src/qi_physics/release.rs:12-47`（`qi_release_to_zone`）、`server/src/qi_physics/constants.rs:88`（`QI_ZONE_UNIT_CAPACITY`）；`server/src/cultivation/components/qi_flow.rs:259-271`（`set_for_init`）、`:280-342`（`gain_from_zone`）、`:344-363`（`release_to_zone` wrapper）、`:365-420`（`transfer_to`）、`:422-482`（`transfer_to_external_actor`）、`:593-728`（`release_external_qi_to_zone` 实际 signed-zone/overflow 提交）；真实 ledger/audit 边界见 `server/src/qi_physics/ledger.rs:459-480`（`QiTransfer`）、`:492-532`（audit-only gate 与 preflight）、`:554-590`（`WorldQiAccount::transfer`）、`:600-606`（`push_transfer_audit`）/ plan “P0 冻结契约 §2”与 P3。

### #3 P1 私有化策略

**决议**：
1. 采用一次全仓私有化，不使用 feature gate、deprecated public alias 或渐进双轨。
2. P0 先冻结并饱和测试所有 capability；P1 在开放大型代码 PR 清空窗口里改字段 visibility，让 all-target 编译成为漏点审计器。
3. P1 开始前与 push 前均紧邻 fetch/merge main；merge 带入任何变化就重跑完整 server gate 和新 HEAD validator。

**落点**：`server/src/cultivation/components/qi_flow.rs:64-137`（`ActorQiKind`、`ActorQiIdentity` capability 与 `PersistentQiSink`）、`:792-800`（`reject_same_account`）；目标字段仍见 `server/src/cultivation/components.rs:641-647` 与 `server/src/world/zone.rs:34-53`（当前仍为 `pub`，私有化尚未完成）/ plan P1 与“文件所有权与边界”。

### #4 qi_max 缩容与 frozen

**决议**：
1. `excess = max(old_current - new_max, 0)`，必须先走 `release_to_zone` 或固定 overflow，再提交 capacity 变化。
2. `qi_max_frozen` 是容量 metadata，不是 qi balance；缩容后 clamp 到 `new_max * BREAKTHROUGH_FAIL_FROZEN_CAP_RATIO`，当前 canonical ratio 为 `0.5`。
3. stored current 可以合法高于 `effective_qi_max`（冻结影响恢复 room，不反向蒸发现有真元）；仅要求 `current <= raw qi_max`。

**落点**：`server/src/cultivation/breakthrough.rs:50`（`BREAKTHROUGH_FAIL_FROZEN_CAP_RATIO`）、`server/src/cultivation/components/qi_flow.rs:484-516`（`resize_qi_max_and_release_excess`；含先释放 excess 再提交 capacity）、`:531-590`（`transfer_cultivation_to_external_owner`）、`:597-728`（外部 owner release 的 signed-zone/overflow 提交）/ plan “P0 冻结契约 §2”与 P2。

### #5 持久化初始化边界

**决议**：
1. 合法 snapshot restore 不产生 transfer；它恢复既有 owner 状态，不是新生成真元。
2. production persistence 必须 decode 专用 wire DTO，再以 `set_for_init` 验证 qi 三字段；非法 finite/range/frozen snapshot fail closed，不 silent clamp。
3. P1 后 domain `Cultivation` 不再作为可直接 `Deserialize` 后整体替换的运行时入口；非 qi 字段由 validated conversion 原样保留。

**落点**：`server/src/cultivation/components/persisted.rs:10-70`（`PersistedCultivationV1`、`TryFrom`、`decode_persisted_cultivation`）、`server/src/cultivation/mod.rs:573-681`（joined-client bundle hydrate/reject path）、`server/src/cultivation/components/qi_flow.rs:226-271`（只读 projection + `set_for_init`；`valid_snapshot` 在 `:781-790`）/ plan P0 与 P1。

### #6 stable overflow 与持久化

**决议**：
1. zone 缺失、已满或不接收的释放量真实进入固定 `overflow:qi_flow_overflow`，禁止动态 `overflow:<entity>` event-only id。
2. 该账户加入 `persistent_runtime_qi_accounts()` 完整 whitelist，与 pending inflow / dying-elder pools 一起原子 snapshot/hydrate；缺行或非法余额 fail closed。
3. migration 从历史已知 0 起步；最终 migration version 在 push 前按最新 main 顺延，避免与在飞 migration 冲突。

**落点**：`server/src/cultivation/components/qi_flow.rs:344-363`（`release_to_zone` wrapper）、`:593-728`（`release_external_qi_to_zone` 实际 zone/overflow 提交）；`server/src/qi_physics/release.rs:12-47`（`qi_release_to_zone`）、`server/src/qi_physics/ledger.rs:629-671`（external owner→ledger）、`:679-755`（ledger→signed zone）、`:763-817`（signed zone→ledger）、`:835-880`（固定 overflow identity 与 persistent whitelist）、`:1083-1101`（`assert_conservation`）；持久化落点为 `server/src/persistence/mod.rs:2369-2385`（v40 migration）、`:5829-5866`（snapshot upsert）、`:6679-6733`（load/hydrate）、`:719-768`（startup hydrate）；真实 balance/audit 入口见 `server/src/qi_physics/ledger.rs:459-480`（`QiTransfer`）、`:554-590`（`WorldQiAccount::transfer`）、`:600-606`（`push_transfer_audit`）/ plan “P0 冻结契约 §1/§3”与 P0。
