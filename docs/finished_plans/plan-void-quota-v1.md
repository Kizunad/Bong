# Bong · plan-void-quota-v1 · Finished

把 `plan-tribulation-v1` 已落地的 `player_count / 50` 化虚名额公式，替换为**世界灵气预算**驱动的硬上限：`化虚名额上限 = floor(total_world_qi / K)`。超额起劫不再结算为半步化虚，而是触发"绝壁劫"语义：天道直接判死，不给侥幸路线。

**本 plan 是覆盖式修正，不是并行新机制**：现有 `AscensionQuota` 持久化 / server-data / client store 已经存在；实现时优先复用并扩展这条链路，不另起一套 `VoidQuotaState` 平行协议。

**世界观锚点**：`worldview.md §三 line 145-150`(全服 1-2 人化虚,末法天道无力承担更多) · `§八 天道行为准则`(灵气总量调控)

**前置硬依赖**：`plan-tribulation-v1` ✅ finished（渡虚劫、`AscensionQuota`、半步化虚现有基线已归档） · `plan-qi-physics-v1` ✅ finished（`WorldQiBudget` / `summarize_world_qi` 底盘可用） · `plan-qi-physics-patch-v1` ⏳ active（守恒迁移仍在推进，本 plan P0 只读预算与快照，不扩新物理）

**交叉引用**：`plan-tribulation-v1` ✅(被本 plan 覆盖 quota 公式与超额结算) · `plan-qi-physics-v1` ✅ · `plan-qi-physics-patch-v1` ⏳ · `plan-cultivation-canonical-align-v1` ✅ · `plan-gameplay-journey-v1` §N.0/O.3

---

## 接入面 Checklist

- **进料**：`qi_physics::WorldQiBudget` / `summarize_world_qi` 快照 + SQLite `ascension_quota.occupied_slots` + 渡虚劫起劫事件
- **出料**：世界灵气预算驱动的 quota limit + 超额起劫的"绝壁劫"死亡结算 + quota snapshot 广播
- **共享类型**：复用 `AscensionQuotaV1` 兼容面，按需扩字段（`total_world_qi` / `quota_k` / `quota_basis`）；不新增并行 `VoidQuotaState`
- **跨仓库契约**：server quota check + agent "绝壁劫" narration + client inspect UI 显示当前名额
- **worldview 锚点**：§三 line 145-150 + §八

---

## 代码库考察（2026-05-08）

- **现有 quota 基线**：`server/src/cultivation/tribulation.rs` 已有 `AscensionQuotaOpened` / `AscensionQuotaOccupied` event；起劫时用 `ascension_quota_limit(player_count.iter().count())` 判定 `half_step_on_success`；公式是 `max(1, player_count / 50)` 且硬上限 3，测试锁定在 `ascension_quota_limit_scales_by_player_count_with_hard_cap`。
- **超额结算现状**：渡虚劫成功后若 `half_step_on_success == true`，当前会回到 `Realm::Spirit`、提升 `qi_max` 和寿元，结果为 `DuXuOutcomeV1::HalfStep`；这不是本 plan 要的"必死绝壁劫"。
- **持久化现状**：SQLite 已有单行表 `ascension_quota(row_id=1, occupied_slots, schema_version, last_updated_wall)`；`complete_tribulation_ascension` 增占用，`release_ascension_quota_slot` 安全递减。
- **释放 hook 现状**：`death_hooks.rs` 的重生 / 死透，以及 `cultivation/mod.rs` 的 `RealmRegressed { from: Void }`，都会释放化虚名额并发送 `AscensionQuotaOpened`。名额回流链路已基本可复用。
- **server-data 现状**：`server/src/network/ascension_quota_emit.rs` 会按 `AscensionQuotaV1::new(occupied, ascension_quota_limit(joined_count))` 广播；Rust schema 与 TS schema 都只有 `occupied_slots / quota_limit / available_slots`。
- **agent 现状**：`agent/packages/tiandao/src/tribulation-runtime.ts` 已有 `ascension_quota_open` 冷叙事，文本是"化虚有位，叩关者可往..."；未见"绝壁劫"、"天地装不下你了"专属分支。
- **client 现状**：`AscensionQuotaHandler` / `AscensionQuotaStore` / `ServerDataRouter` 已接 `ascension_quota`；只保存 snapshot，未见 `VoidQuotaDisplay.java`，inspect 面板尚未消费展示。
- **qi_physics 现状**：`WorldQiBudget` 由 `BONG_SPIRIT_QI_TOTAL` 或默认 `DEFAULT_SPIRIT_QI_TOTAL = 100.0` 初始化；`summarize_world_qi` 会读 `ZoneRegistry`、玩家 `Cultivation.qi_current`、inventory 和 ledger。注意：当前 `Zone.spirit_qi ∈ [-1, 1]` 是浓度，玩家 `qi_current` 是境界量级真元点，不能未经校准直接相加当 K=5000 的旧设计单位。
- **缺失项**：未找到 `void_quota.rs` / `total_qi.rs` / `VoidQuotaState` / `check_void_quota` / `jueb_strike.rs` / "绝壁劫" / "天地装不下你了"。

---

## §0 设计轴心

- [x] **不再按人数定名额**：删除 / 替换 `ascension_quota_limit(player_count)` 的行为入口；人数只可作为 telemetry，不再参与 quota 上限。
- [x] **世界灵气预算是公式输入**：v1 以 `WorldQiBudget.current_total` 为权威预算；`summarize_world_qi(...).total_observed()` 先用于 debug / drift 日志，避免把 `Zone.spirit_qi` 浓度和玩家 `qi_current` 生硬混单位。
- [x] **K 值为 server config**：新增 `BONG_VOID_QUOTA_K`（默认按当前 `DEFAULT_SPIRIT_QI_TOTAL = 100.0` 校准为 50.0，使开服初期 quota≈2）；后续 telemetry 再调。
- [x] **允许 quota=0**：`total_world_qi < K` 时无人能新化虚；既有化虚者不强制掉境，但新起劫必须走绝壁劫。
- [x] **化虚者死亡名额回流**：复用现有 `release_ascension_quota_slot` hook；不凭空 `+K` 造灵气，数值回流由 qi_physics 释放路径承接。
- [x] **超额惩罚直接**：超额玩家试图渡虚劫 → "绝壁劫" → `DuXuOutcomeV1::Killed` 或等价死亡结算，**不再半步化虚**。
- [x] **兼容优先**：保留 `ascension_quota` payload 名称与 client handler；只扩字段 / 增 reason，避免平行 `void_quota` IPC 双轨。

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ✅ 2026-05-08 | quota 公式切换：`quota = floor(WorldQiBudget.current_total / K)` + `check_void_quota` 替换 player-count 入口 | 起劫和 snapshot 广播都不再调用 `ascension_quota_limit(player_count)`；P0 单测覆盖 quota=0/1/2/边界 |
| **P1** ✅ 2026-05-08 | 超额起劫改为"绝壁劫"死亡结算 + schema/agent reason | 超额玩家试图起劫时直接触发 `VoidQuotaExceeded` 真死；agent 输出"天地装不下你了。"语义 |
| **P2** ✅ 2026-05-08 | 化虚者死亡 / 降境 / 死透 → 名额立刻再开 | 保留并回归验证现有 release hook，新的 `check_void_quota` snapshot 会立刻反映回流名额 |
| **P3** ✅ 2026-05-08 | client inspect UI 显示当前世界化虚名额: X / Y + quota 来源 | handler/store 保存扩展字段；通灵/化虚检视面板显示当前名额和来源 |

---

## §2 关键公式

```text
total_world_qi = WorldQiBudget.current_total
quota_max      = floor(total_world_qi / BONG_VOID_QUOTA_K)
can_void       = current_void_count < quota_max

绝壁劫触发(超额时玩家起劫):
  reason = "void_quota_exceeded"
  narration: "天地装不下你了。"
  结果: 100% 渡劫失败,真死
```

K 值校准（运维 config）:

```text
当前 qi_physics 单位:
  DEFAULT_SPIRIT_QI_TOTAL = 100.0
  BONG_VOID_QUOTA_K 默认 50.0
  total_world_qi = 100.0 → quota_max = 2
  total_world_qi = 50.0  → quota_max = 1
  total_world_qi < 50.0  → quota_max = 0

注意:
  旧骨架的 K=5000 / total_qi~10000 是未接 qi_physics 前的设计单位。
  当前代码里 Zone.spirit_qi 是 [-1,1] 浓度, Cultivation.qi_current 是玩家真元点,
  P0 不允许直接 `sum(zone.spirit_qi) + sum(qi_current)` 混单位定 quota。
```

---

## §3 数据契约

- [x] `server/src/cultivation/tribulation.rs`：`VoidQuotaConfig` / `compute_void_quota_limit` / `check_void_quota`
- [x] `server/src/network/ascension_quota_emit.rs`：snapshot 广播改读同一个 quota 计算函数，不再按 joined client count 算上限
- [x] `server/src/schema/server_data.rs` + `agent/packages/schema/src/server-data.ts` + generated schema：扩展 `ascension_quota` payload（`total_world_qi` / `quota_k` / `quota_basis`）
- [x] `server/src/schema/tribulation.rs` + `agent/packages/schema/src/tribulation.ts`：为绝壁劫添加 `reason = "void_quota_exceeded"`，避免 agent 从普通 `killed` 猜语义
- [x] `server/src/cultivation/death_hooks.rs` / `server/src/cultivation/mod.rs`：保留现有 release hook，并补新公式下的回归测试
- [x] `agent/packages/tiandao/src/tribulation-runtime.ts`："天地装不下你了。" narration 分支 + fallback 测试
- [x] `client/src/main/java/com/bong/client/combat/store/AscensionQuotaStore.java`：保存扩展字段
- [x] `client/src/main/java/com/bong/client/combat/inspect/StatusPanelExtension.java` + `client/src/main/java/com/bong/client/inventory/InspectScreen.java`：通灵期及化虚期显示"当前世界化虚名额: X / Y"

---

## §4 开放问题

- [x] **是否新增 `VoidQuotaState` 并行协议？** 不新增。复用 `AscensionQuotaV1` 兼容面并扩字段，KISS，避免 server/client/agent 双轨。
- [x] **是否保留半步化虚？** 不保留为超额结果。本 plan 覆盖 `plan-tribulation-v1` 的 Phase 7 半步化虚语义：超额应真死。
- [x] **K 值如何动态调整？** 初版固定 server config；默认 `BONG_VOID_QUOTA_K = 50.0`，后续 telemetry 再调。
- [x] **同 tick 多人同时起劫如何抢名额？** FCFS（系统读事件顺序），`start_tribulation_system_reserves_void_quota_fcfs_within_tick` 锁定。
- [x] **绝壁劫死亡是否需要新增 death cause？** 已新增 `CultivationDeathCause::VoidQuotaExceeded`，death arbiter 将其作为终局死亡处理。
- [x] **client 展示门槛**：默认通灵期及化虚期可见。

## §5 进度日志

- 2026-05-01：骨架创建。plan-gameplay-journey-v1 §N.0 / O.3 派生。
- 2026-05-08：实地核验后升为 active。确认 `plan-tribulation-v1` 已完成但当前实现是 `player_count/50` + 硬上限 3 + `half_step_on_success`；`qi_physics` 已提供 `WorldQiBudget` / `summarize_world_qi`；server/client/agent 现有 `ascension_quota` 链路可复用，缺口集中在世界灵气公式、绝壁劫死亡语义、inspect 展示。
- 2026-05-08：完成覆盖式实现。`ascension_quota_limit(player_count)` 被 `check_void_quota(WorldQiBudget.current_total, BONG_VOID_QUOTA_K)` 取代；超额起劫直接触发 `VoidQuotaExceeded` 真死；`ascension_quota` payload 扩展到 server/agent/client；inspect 展示当前世界化虚名额。

## Finish Evidence

### 落地清单

- **P0 quota 公式**：`server/src/cultivation/tribulation.rs` 新增 `VoidQuotaConfig` / `compute_void_quota_limit` / `check_void_quota`，`start_tribulation_system` 改用 `WorldQiBudget.current_total` + `BONG_VOID_QUOTA_K`；`server/src/network/ascension_quota_emit.rs` 的 snapshot 同源读取该函数。
- **P1 绝壁劫死亡语义**：`server/src/cultivation/death_hooks.rs` 新增 `CultivationDeathCause::VoidQuotaExceeded`，`server/src/combat/lifecycle.rs` 将其作为终局死亡处理；`DuXuResultV1.reason = "void_quota_exceeded"`；`agent/packages/tiandao/src/tribulation-runtime.ts` 输出"天地装不下你了。"。
- **P2 名额回流**：保留 `release_ascension_quota_slot` 的重生 / 死透 / 降境 hook，`AscensionQuotaOpened` 继续驱动新的 quota snapshot。
- **P3 client inspect 展示**：`AscensionQuotaStore` / `AscensionQuotaHandler` 保存扩展字段；`StatusPanelExtension` + `InspectScreen` 在通灵/化虚检视状态条展示当前世界化虚名额、来源和 tooltip 数据。
- **Review 收口**：移除 `TribulationState.half_step_on_success` 的死分支；起劫 quota 检查把活跃渡虚劫计入占位，跨 tick 抢位也会被绝壁劫拒绝；quota 读写失败改为 fail-closed，不再制造未持久化化虚者；旧 IPC 字段保持 `false` 输出以兼容现有 client/schema。

### 关键 commit

- `dfa0080e1`（2026-05-08）`docs(plan-void-quota-v1): 升 active 并同步实地核验`
- `6d3b92715`（2026-05-08）`server(void-quota): 用世界灵气预算收口化虚名额`
- `d21cff4f5`（2026-05-08）`agent(void-quota): 扩展名额契约和绝壁劫叙事`
- `6c73f28ee`（2026-05-08）`client(void-quota): 在检视面板展示化虚名额`
- `8b671d5bf`（2026-05-08）`docs(plan-void-quota-v1): finish evidence 并归档`
- `e522a871c`（2026-05-08）`server(void-quota): 收紧化虚名额竞态`
- `35c3dbf8d`（2026-05-08）`fix(void-quota): 收紧名额持久化边界`
- `49fbc0f79`（2026-05-08）`fix(void-quota): 收紧跨端契约和检视刷新`

### 测试结果

- `cd server && cargo test void_quota -- --nocapture` → 5 passed
- `cd server && cargo test start_tribulation_system_counts_in_flight_void_tribulations_across_ticks -- --nocapture` → 1 passed
- `cd server && cargo test quota -- --nocapture` → 30 passed
- `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --quiet` → 2944 passed
- `cd agent && npm run build && npm test -w @bong/schema && npm test -w @bong/tiandao` → schema 305 passed；tiandao 271 passed
- `cd client && JAVA_HOME="/usr/lib/jvm/java-17-openjdk-amd64" ./gradlew test build` → BUILD SUCCESSFUL

### 跨仓库核验

- **server**：`WorldQiBudget` / `VoidQuotaConfig` / `check_void_quota` / `CultivationDeathCause::VoidQuotaExceeded` / `VOID_QUOTA_EXCEEDED_REASON` / 活跃渡虚劫占位
- **agent/schema**：`AscensionQuotaV1.total_world_qi` / `quota_k` / `quota_basis`，`DuXuResultV1.reason`
- **agent runtime**：`tribulation-runtime.ts` 对 `reason === "void_quota_exceeded"` 返回"天地装不下你了。"
- **client**：`AscensionQuotaStore.State(totalWorldQi, quotaK, quotaBasis)`，`StatusPanelExtension.ascensionQuotaLine`，`InspectScreen` store listener

### 遗留 / 后续

- `BONG_VOID_QUOTA_K` 仍是固定 server config；后续可由 telemetry/运营配置 plan 调整。
- 本 plan 不修改 qi_physics 守恒迁移；`plan-qi-physics-patch-v1` 继续负责底层真元账本迁移。
