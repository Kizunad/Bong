# plan-negzone-epsilon-siphon-v1 — 负灵域 epsilon siphon 的可表示性与致死触发闭环

> 一句话主题：在不改变 `SIPHON_FACTOR` 玩法数值的前提下，让负灵域抽吸在 IEEE-754 无法产生可观察扣减时成为守恒的本 tick no-op，并保证抽干分支的死亡触发不被可表示性错误吞掉。
>
> 本 plan 是纯 server 物理/修炼路径 BugFix；不改 schema、client、agent、zone 配置或其它玩法数值。

## 防孤岛调研（2026-09-07）

### 正典与既有实现

- `docs/worldview.md:30-47` 定义灵压环境与负灵域：`spirit_qi < 0` 是天地倒吸，境界越高（`qi_max` 越大）抽吸越快；`docs/worldview.md:870-879` 及 `:1361-1367` 约束灵气总量为零和、流动不能凭空生成/消失。
- `docs/finished_plans/plan-qi-physics-v1.md` 与 `plan-qi-physics-patch-v1.md` 已将底层物理算子、signed zone、`QiTransfer`、`assert_conservation` 和错误闭环归口到 `server/src/qi_physics/`；本 plan 只扩展所需的通用可表示性判据，不在 cultivation 自建数值常量。
- `docs/finished_plans/plan-death-lifecycle-v1.md` 已落地 `CultivationDeathTrigger` → combat lifecycle 的死亡契约；`NegativeZoneDrain` 是既有 cause，本 plan 不造第二种死亡事件。
- `docs/plan-bughunt-qi-needle-negative-zone-release-v1.md`（active）已确认 `qi_release_to_zone` 接受 signed negative zone，说明负灵域不是 release 的无效目标；其范围是气针过期路径，不覆盖本文件的 live-player siphon tick。
- `docs/finished_plans/plan-zone-qi-economy-v1.md` 的 §8.1 #5 明确负灵域 inflow 继续排除；本修复不把 siphon 变成 inflow，也不改变负灵域回正规则。

### 活跃计划、骨架与去重结论

- 已检索 `docs/plan-*.md`、`docs/plans-skeleton/plan-*.md` 与 `docs/reminder.md` 的 `negative zone` / `siphon` / `epsilon` / `qi_physics` / `CultivationDeathTrigger`；未发现同名 `plan-negzone-epsilon-siphon-v1`，也未发现覆盖本 tick 可表示性和抽干失败触发的已合入修复。
- `docs/plans-skeleton/plan-bughunt-dormant-negative-qi-release.md` 与 `docs/plan-bughunt-qi-needle-negative-zone-release-v1.md` 分别属于 dormant 死亡释放与气针容器释放；本 plan 只处理 `server/src/cultivation/negative_zone.rs` 的 live-player `negative_zone_siphon_tick`，不合并它们的范围。
- 代码检索确认 `server/src/cultivation/negative_zone.rs:20-28` 是唯一 `SIPHON_FACTOR` / `siphon_amount` 实现，`server/src/cultivation/negative_zone.rs:32-165` 是目标 tick；没有将其复制到第二个模块的必要。

### 接入面

- **进料**：`ZoneRegistry::find_zone` 提供当前维度/位置的 signed `Zone.spirit_qi`（`server/src/world/zone.rs:34-53`）；`Cultivation.qi_current/qi_max` 提供 live actor 真元；`LifeRecord` 提供既有 actor identity。
- **出料**：可表示的抽吸继续经 `Cultivation::release_to_zone` → `release_external_qi_to_zone`（`server/src/cultivation/components/qi_flow.rs:361-377,607-737`）扣玩家并回写 signed zone/overflow；抽干后 emit 既有 `CultivationDeathTrigger { cause: NegativeZoneDrain }`。
- **共享类型**：复用 `ZoneRegistry`、`Cultivation`、`LifeRecord`、`QiTransfer`、`WorldQiAccount`、`CultivationDeathTrigger`；不新建 balance、event 或 zone mirror。
- **跨仓库契约**：本 bug 是纯 server 内部物理/死亡事件修复，不新增 IPC/schema/client/agent symbol；既有 combat lifecycle 消费 `NegativeZoneDrain` 的契约保持不变。
- **worldview 锚点**：负灵域倒吸对应 `worldview.md §二`；零和守恒对应 `worldview.md §十`。
- **qi_physics 锚点**：新增的判据（如需要）必须先落在 `server/src/qi_physics`，以实际 `before - amount != before` 的 IEEE-754 可表示进展判断，不在 `cultivation` 写魔数 epsilon；守恒断言引用 `qi_physics::ledger::assert_conservation` 与 `DEFAULT_SPIRIT_QI_TOTAL`，不写死总量字面值。

## 阶段总览

| 阶段 | 状态 | 交付物 | 验收 |
|---|---|---|---|
| P0 | ✅ 2026-09-08 | 第一性复现微负 zone 的 `UnrepresentableFlow`，确认 no-op 判据与抽干失败分支可达性；锁定 `SIPHON_FACTOR` 不变 | 复现日志/数值对拍 + 代码行号证据 + 决策写入 §8.1 |
| P1 | ✅ 2026-09-08 | `qi_physics` 可表示减法判据 + `negative_zone_siphon_tick` epsilon no-op；抽干分支对数值不可表示 release 走既有可追踪 overflow 兜底并继续 emit death | 饱和单测：微负 no-op、正常释放、正/零 zone、边界相等、抽干成功/失败；`QiTransfer` 与守恒断言通过 |
| P2 | ✅ 2026-09-08 | 完整 server 门禁、无上下文 validator、最新主线合并复验、Finish Evidence、PR | fmt/clippy/test 全绿，validator 绑定最终 HEAD PASS，CI/Kody 交调度会话 |

## P0 第一性验真与决策门

### 现象验证

1. `siphon_amount(zone_qi, qi_max)` 仅在 `zone_qi < 0` 返回 `(-zone_qi) * qi_max * SIPHON_FACTOR`；不改 `SIPHON_FACTOR`。
2. 当 `qi_current >= siphon > 0` 且 `qi_current - siphon == qi_current` 时，`release_external_qi_to_zone` 在 `server/src/cultivation/components/qi_flow.rs:628-632` 返回 `UnrepresentableFlow`，当前 tick 只 warn 并重试，构成重复告警。
3. 判据必须描述“正扣减是否改变来源 f64 表示”，优先直接核验 `before - amount != before`，而不是引入 cultivation 私有绝对阈值；该判据本身归 `qi_physics`。

### 抽干分支可达性

- `qi_current < siphon` 时 `drained = max(qi_current, 0)`；若 `drained == qi_current`，来源可精确置零，但 signed zone 的极小增量仍可能让 `zone.spirit_qi + drained / QI_ZONE_UNIT_CAPACITY == zone.spirit_qi`，从而在 `qi_flow.rs:653-659` 失败。
- P0 必须用真实可构造的有限 `qi_current`、负 zone 与现有 `QI_ZONE_UNIT_CAPACITY` 证明该错误路径；若确认可达，P1 仅对该数值错误使用既有 `qi_flow_overflow` 持久 sink 后再 emit `NegativeZoneDrain`，其它 actor/ledger/invalid-state 错误仍 fail-closed + warn，不伪造死亡。

## P1 修复与测试矩阵

### 修复边界

- `negative_zone_siphon_tick`（`server/src/cultivation/negative_zone.rs:58-75`）在进入正常释放分支前调用 `qi_physics::subtraction_makes_progress`（`server/src/qi_physics/mod.rs:176-191`）；判定为“正金额但来源扣减不前进”时直接 continue，不写 source、zone、ledger，不 warn。
- 可表示的正常 siphon 继续复用 `release_qi_amount_to_zone`；不裸写 `qi_current -=` 或 `zone.spirit_qi +=`。
- 抽干分支的成功路径保持“释放实际 `drained` → emit `CultivationDeathTrigger::NegativeZoneDrain`”；仅当 zone 端加法因 IEEE-754 不可表示而失败时，重试相同实际金额到既有 overflow sink，成功后 emit 相同 death event；真正的 identity/state/ledger 错误继续 fail-closed。

### 饱和测试

1. 微负 zone + 正常玩家 qi：siphon 为正但 `qi_current - siphon` 不变；多次 tick 后 `qi_current`、zone、ledger transfers 均不变，无 release warning 路径。
2. 正常负 zone：siphon 进入 ledger-backed release，玩家扣减、zone 回写、`QiTransfer` 金额与 `qi_physics::ledger::assert_conservation` 对齐。
3. `zone_qi >= 0`（含 `0`）：siphon 为 0，不进入转账。
4. `qi_current == siphon`：来源精确归零，仍完成一次 release，不被 no-op 判据跳过。
5. `qi_current < siphon` 且 zone 加法可表示：发一条 `NegativeZoneDrain`，实际 `drained` 只转一次。
6. `qi_current < siphon` 且 zone 加法不可表示：验证 overflow sink 接收实际 `drained`、玩家归零、death trigger 仍发；无第二次扣款或凭空生成。
7. 缺失 `CurrentDimension` / zone / `LifeRecord` 等既有错误分支：保持原 fail-closed 语义，不因 fallback 伪造 actor 或死亡。

### P1 验收证据（2026-09-08）

- 未改代码基线：`flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo test negative_zone --lib'`，34 passed、0 failed。
- 修复后同命令：40 passed、0 failed；覆盖微负 no-op 多 tick、正常负 zone 释放与守恒、正/零 zone、相等边界、抽干成功/zone 精度失败 overflow fallback，以及缺 canonical identity 的 fail-closed。
- `server/src/qi_physics/mod.rs:176-191` 的判据测试锁定 `before - amount != before`，并覆盖零金额和非法输入；完整 helper 过滤测试结果在 P2 Finish Evidence 汇总。

## §8 开放问题（P0 决策门收口）

1. `qi_physics` 判据采用“实际 f64 减法是否产生不同结果”还是复用已有 helper？实施前以代码探索确认唯一落点，禁止在 `cultivation` 自建 epsilon。
2. 抽干分支的 zone 不可表示错误是否在当前有限参数域可达？若可达，仅允许用既有 overflow sink 保持守恒后继续死亡触发；若不可达，保留验证证据并不扩大行为。
3. 如何证明 no-op 是守恒中性：跳过前后两个物理权威字段与 ledger 审计均不变，且不产生 `QiTransfer`。

全部已在 §8.1 收口。原表保留以备追溯，**实施时以 §8.1 决议为准**。

## §8.1 决议（pre-P0 收口，2026-09-08）

### #1 可表示性判据与 epsilon no-op

**决议**：

1. 初步定位成立：实测 `238.52849866722318 - 1.4174837940073883e-14` 仍为 `238.52849866722318`；这不是可忽略的玩法阈值，而是来源 `f64` 无法产生可观察扣减。
2. 在 `qi_physics` 增加通用“正减法是否产生可表示进展”helper，直接判断 `before - amount != before`，并由 `negative_zone_siphon_tick`（`server/src/cultivation/negative_zone.rs:58-75`）在调用 release 前使用；不改变 `SIPHON_FACTOR`（`negative_zone.rs:20-28`），不在 cultivation 新造 epsilon。
3. 判定为正金额但来源减法不前进时，本 tick 是守恒中性的 no-op：不触碰玩家、zone、ledger、`QiTransfer`，也不 warn；真正的无效输入/物理错误仍沿原 fail-closed + warn 路径。

**落点**：`server/src/cultivation/negative_zone.rs:58-75`、`server/src/qi_physics/mod.rs:176-191`、`server/src/cultivation/components/qi_flow.rs:607-632`（现有错误语义）/ 本 plan §P1「修复边界」、§P1「饱和测试」#1/#4。

### #2 抽干分支的可达性与失败收口

**决议**：

1. 数值失败可达：有限 fixture `zone.spirit_qi=-1.0`、`qi_max=100.0`、`qi_current=1e-16` 满足 `qi_current < siphon`，但 `zone.spirit_qi + qi_current / 50.0 == zone.spirit_qi`；对应现有 `qi_flow.rs:653-659` 的 `UnrepresentableFlow` 会吞掉 death trigger。
2. 成功 release 仍先走既有 `release_qi_amount_to_zone`；仅当失败明确为 zone 字段不可表示时，使用同一实际 `drained` 重试到既有 `qi_flow_overflow` 持久 sink，确认真实入账后再发既有 `CultivationDeathTrigger::NegativeZoneDrain`。
3. 缺失 `LifeRecord`、非法 Cultivation 状态、ledger 失败等非该数值错误继续 fail-closed + warn，不发伪造死亡事件；不裸写 `qi_current` 或 `zone.spirit_qi`，守恒验证用 `assert_conservation`。

**落点**：`server/src/cultivation/negative_zone.rs:96-165`、`server/src/cultivation/components/qi_flow.rs:633-737`、`server/src/qi_physics/ledger.rs:1083-1100` / 本 plan §P0「抽干分支可达性」、§P1「修复边界」、§P1「饱和测试」#5-#7。

### #3 no-op 的守恒证据

**决议**：

1. no-op 不移动任何物理量：前后 `Cultivation.qi_current` 与 `Zone.spirit_qi` 保持 bitwise 数值不变，`WorldQiAccount` balance/transfer history 与 `Events<QiTransfer>` 均为空；多次 tick 仍不产生重试副作用。
2. 正常释放和 overflow fallback 逐笔检查玩家、signed zone、稳定 ledger sink 的总量，使用 `qi_physics::ledger::assert_conservation`，总量基准引用 `DEFAULT_SPIRIT_QI_TOTAL` 所属的预算 fixture而不写字面总量。

**落点**：`server/src/qi_physics/ledger.rs:924-939,1013-1054,1083-1100`、`server/src/cultivation/negative_zone.rs:52-165` / 本 plan §P1「饱和测试」#1/#2/#6。

## Finish Evidence

> 归档前填写：落地清单、关键 commit、fmt/clippy/test 结果、validator 最终 HEAD、守恒/死亡契约核验与遗留范围。

### 落地清单

- `server/src/qi_physics/mod.rs:176-191` 新增通用 `subtraction_makes_progress`，以实际 `f64` 减法结果判断来源是否产生可表示进展；判据留在 `qi_physics`，未在 cultivation 自建 epsilon。
- `server/src/cultivation/negative_zone.rs:48-169` 在 live-player siphon tick 中对来源不前进的正金额执行守恒中性 no-op；正常释放仍走 `release_qi_amount_to_zone`；zone 增量不可表示时复用 `qi_flow_overflow`，真实入账后才发 `CultivationDeathTrigger::NegativeZoneDrain`，其它错误继续 fail-closed。
- `server/src/cultivation/negative_zone.rs:343-566` 饱和测试覆盖微负多 tick、正/零 zone、相等边界、正常抽干、zone 精度 fallback、守恒、死亡触发与非数值错误 fail-closed。

### 关键 commit

- `b53982e8b`（2026-09-07）：建立 `plan-negzone-epsilon-siphon-v1` skeleton。
- `8b56c9f2d`（2026-09-07）：promotion 至 active plan 并进入 BugFix 实施。
- `5efd6516a`（2026-09-08）：完成第一性验真，确认来源与 signed zone 的不可表示路径可达。
- `2e6669025`（2026-09-08）：落地 qi_physics 判据、siphon no-op、overflow fallback 与回归测试。
- `0917e2f79`（2026-09-08）：补充 P1 验收证据。
- `152d447ad`（2026-09-08）：格式化并固定最终修复代码。
- `bc0f4ba22`（2026-09-08）：紧邻 `git fetch origin && git merge origin/main` 合入最新主线（含 tribulation 测试外置变更）。

### 测试结果

- 基线：`flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo test negative_zone --lib'`，34 passed、0 failed。
- 修复后定向：`negative_zone --lib` 40 passed、0 failed；`subtraction_progress --lib` 1 passed、0 failed。
- 合并主线后的完整门禁：`flock /tmp/bong-cargo.lock -c 'cd server && ../scripts/build-token.sh cargo fmt --check && ../scripts/build-token.sh cargo clippy --all-targets -- -D warnings && ../scripts/build-token.sh cargo test'`，fmt/clippy 通过；server lib 12078 passed、1 ignored，main 18 passed，外置 `tribulation_unit` 103 passed，所有测试 0 failed；doc-tests 3 passed、5 ignored。

### Validator 与守恒/死亡契约核验

- 无上下文只读 validator 首步对拍 `git rev-parse HEAD` 与 `152d447ad48d34be689ea99ca52981c3ca2aaf8c`，结论 PASS；合并主线后需绑定新最终 HEAD 重新核验。
- `qi_physics::ledger::assert_conservation` 覆盖正常释放、overflow fallback 与 no-op；`CultivationDeathTrigger::NegativeZoneDrain` 仅在真实 release/overflow 入账成功后发送。
- 本 plan 未修改 `SIPHON_FACTOR`、schema、client、agent、依赖版本或其它 gameplay 行为；不新增跨仓库契约，server / agent / client 接入面分别为既有 server 物理路径、无 agent 变更、无 client 变更。

### 遗留 / 后续

- 仅保留既有 `docs/plan-bughunt-qi-needle-negative-zone-release-v1` 等其它 plan 的范围；本 plan 不处理气针容器释放、dormant 死亡释放或浮点常数玩法调整。
