# Bong · plan-tribulation-balance-v1 · active

渡虚劫**系统性平衡矩阵**——承接 plan-halfstep-buff-v1 P0 遥测数据，对化虚名额公式、三波难度曲线、半步化虚 buff 强度、重渡间隔做整体校准，防止"名额长期空缺"或"名额永久饱和"导致渡虚劫系统性失效。v1 不改变渡虚劫机制本身，只调整可配置参数和监控看板。

**背景**（派生自 plan-halfstep-buff-v1 §反向被依赖）：
- plan-halfstep-buff-v1 P1 校准单个 buff 常数（`HALFSTEP_QI_MAX_BONUS` / `HALFSTEP_LIFESPAN_BONUS_YEARS`）
- 本 plan 做**更大尺度的系统平衡**：化虚名额 × 玩家数 × 境界分布 × 半步化虚比例 × 重渡频率的联合矩阵
- 属于长期运营配套工具，不是单次修复

**交叉引用**：`plan-tribulation-v1.md` ✅（`AscensionQuotaStore` / quota 公式 `player_count/50` 硬上限 3 / 三波强度曲线 L1128 / 半步化虚结算）· `plan-halfstep-buff-v1.md` ⬜（buff 常数 + 重渡机制，本 plan 的数据来源）· `plan-cultivation-v1.md` ✅（境界分布数据 + `qi_current` / `qi_max`）· `plan-qi-physics-v1.md` ✅（守恒律：buff 改 qi_max 必走 ledger）

**worldview 锚点**：
- **§三:78 化虚稀缺性**：天道不允许更多化虚修士——名额制是世界观底线，平衡调参不能突破"化虚极稀"的定性
- **§三:65 境界间距**：通灵满级到化虚的质变——半步 buff 永远是量变，平衡校准不能让半步化虚"实质等同化虚"
- **§十二:1043 生死循环**：渡虚劫系统是修士寿元轮回的核心驱动力，平衡失效 = 世界观物理机制失效

**qi_physics 锚点**：
- 调整 `HALFSTEP_QI_MAX_BONUS` 时 qi_max 容量扩张，通过 `qi_physics::ledger::QiTransfer` 标记守恒影响
- 本 plan 不引入新物理常数；所有调参只改现有常数的值，不新建函数

**前置依赖**：
- `plan-tribulation-v1` ✅ — 底盘：AscensionQuotaStore / quota 公式 / 三波强度 / 半步结算 / 遥测计数器基础（P7 平衡回归 53 单测）
- `plan-halfstep-buff-v1` P0+ ⬜ — 遥测仪表盘数据（半步化虚玩家比例 / quota 满时占比），**本 plan P0 依赖此数据**

**反向被依赖**：
- 长期运营配套；无其他 plan 依赖本 plan 产出

---

## 接入面 Checklist

- **进料**：`AscensionQuotaStore`（当前 quota / max）+ `tribulation_halfstep_count` / `tribulation_ascended_count` / `ascension_quota_full_duration_ticks` / `halfstep_stuck_duration_ticks`（halfstep-buff-v1 P0 遥测指标）+ 玩家境界分布（`cultivation::realm` 查询）+ 三波强度常数（`tribulation.rs L1128`）
- **出料**：校准后的配置常数集（`QUOTA_PER_PLAYER` / `QUOTA_MAX` / `WAVE_*_INTENSITY` / `HALFSTEP_*_BONUS`）+ 平衡监控看板指令 `/balance tribulation` + 单测回归锚点
- **共享类型**：复用 `AscensionQuotaStore` / `DuXuOutcomeV1` / 遥测计数器；新增 `TribulationBalanceConfig` resource
- **跨仓库契约**：纯 server 逻辑调参；无 agent / client 接口变更（监控看板为 dev-only 命令）
- **worldview 锚点**：§三:78 化虚稀缺性 + §三:65 境界间距 + §十二 生死循环
- **qi_physics 锚点**：buff 改 qi_max 时走 ledger 标记（继承 halfstep-buff-v1 规则）

---

## §0 设计轴心

- **平衡目标**：任意 30 天内，化虚名额满载率 30-70%（太低 = 名额形同虚设；太高 = 化虚机会极难）；半步化虚玩家滞留超 1 month in-game 比例 < 25%
- **校准维度**：
  1. 名额上限公式：`quota_max = clamp(player_count / QUOTA_PER_PLAYER, 1, QUOTA_MAX_HARD_CAP)`（当前 QUOTA_PER_PLAYER=50，QUOTA_MAX=3）
  2. 三波难度系数：tribulation.rs L1128 定义的 WAVE_1/WAVE_2/WAVE_3 强度乘数
  3. 心魔劫触发概率：HEART_DEMON_TRIGGER_RATIO（当前 MVP 三选项）
  4. 半步化虚 buff 强度：HALFSTEP_QI_MAX_BONUS / HALFSTEP_LIFESPAN_BONUS_YEARS
- **只调 const，不改机制**：本 plan 产出是校准后的 const 值集，不重写渡劫逻辑

---

## 阶段总览

| 阶段 | 状态 | 主要交付物 | 验收标准 |
|------|------|-----------|---------|
| **P0** | ✅ 2026-06-13 | 平衡监控看板 + 指标定义 | `/balance tribulation` 命令显示全部平衡指标 |
| **P1** | ⬜ | 名额公式校准（`QUOTA_PER_PLAYER` / `QUOTA_MAX`）| 名额满载率落入 30-70% 区间（模拟数据验证）|
| **P2** | ⬜ | 三波强度 + 心魔触发概率校准 | 渡虚劫成功率落入设计目标区间 |
| **P3** | ⬜ | 半步化虚 buff 强度终版 + 联合回归测试 | 全参数调整后回归 53 单测 green |

---

## P0 — 平衡监控看板

**前置**：halfstep-buff-v1 P0 遥测计数器已部署（`tribulation_halfstep_count` / `ascension_quota_full_duration_ticks` 等）

- [ ] `TribulationBalanceConfig` resource（`server/src/cultivation/tribulation_balance.rs`）：
  ```rust
  pub struct TribulationBalanceConfig {
      pub quota_per_player: u32,        // 当前 50
      pub quota_max_hard_cap: u32,      // 当前 3
      pub wave_1_intensity: f32,        // tribulation.rs L1128 同步
      pub wave_2_intensity: f32,
      pub wave_3_intensity: f32,
      pub heart_demon_trigger_ratio: f32,
  }
  ```
- [ ] `/balance tribulation` dev-only 命令（brigadier）显示：
  - 当前 quota_current / quota_max / 满载率百分比
  - 近 N tick 渡虚劫次数：halfstep / ascended / failed
  - 半步化虚玩家平均滞留 tick / in-game 月
  - 当前 TribulationBalanceConfig 所有参数值
- [ ] ≥ 6 单测（命令正确读取 config / quota 满载率计算 / 滞留 tick 计算）

**P0 验收**：dev 环境 mock 10 次渡劫结算后 `/balance tribulation` 显示正确统计

---

## P1 — 名额公式校准

**P1 触发条件**（P0 数据观察期 ≥ 2 weeks 后）：
- 名额满载率 < 20% → `QUOTA_PER_PLAYER` 减小（名额变多）
- 名额满载率 > 80% → `QUOTA_PER_PLAYER` 增大（名额变少）或 `QUOTA_MAX_HARD_CAP` +1

- [ ] 基于 P0 数据选定 `QUOTA_PER_PLAYER` 新值 + `QUOTA_MAX_HARD_CAP` 新值
- [ ] 更新 `TribulationBalanceConfig` default 值 + 提取为具名 const（`server/src/cultivation/tribulation.rs`）：
  ```rust
  pub const QUOTA_PER_PLAYER: u32 = XX;     // P0 数据后填入，**测试引用 const 禁止写字面值**
  pub const QUOTA_MAX_HARD_CAP: u32 = XX;
  ```
- [ ] 回归测试：`assert_eq!(config.quota_per_player, QUOTA_PER_PLAYER)` 引用 const（防止 const 改了测试不跟）
- [ ] ≥ 5 单测（quota 公式正确 / 并发结算不超 quota_max / halfstep-buff-v1 P2 并发测试仍 green）

**P1 验收**：const PR 合并 + 5 单测 green；满载率模拟在 30-70% 区间

---

## P2 — 难度曲线校准

**P2 触发条件**（P1 名额稳定后）：
- 渡虚劫总体失败率 > 80% → 三波强度下调
- 渡虚劫总体成功率 > 60% → 三波强度上调 or 心魔触发概率提高

- [ ] 基于 P0 数据选定三波强度新值：
  ```rust
  pub const WAVE_1_INTENSITY: f32 = X.X;  // tribulation.rs L1128 对应常数
  pub const WAVE_2_INTENSITY: f32 = X.X;
  pub const WAVE_3_INTENSITY: f32 = X.X;
  pub const HEART_DEMON_TRIGGER_RATIO: f32 = X.X;
  ```
- [ ] 回归测试：tribulation.rs 现有 53 单测全部 green（难度曲线变化不破坏波形逻辑）
- [ ] 若调整心魔触发概率：`heart_demon_runtime.ts` vitest fallback / arbiter 单测仍 green

**P2 验收**：53 单测 green + agent vitest green；渡虚劫成功率落入目标区间（模拟验证）

---

## P3 — 半步 buff 终版 + 联合回归

**P3 依赖**：halfstep-buff-v1 P1 已校准 `HALFSTEP_QI_MAX_BONUS` / `HALFSTEP_LIFESPAN_BONUS_YEARS`

- [ ] 将 halfstep-buff-v1 的 buff 常数值与本 plan 平衡矩阵交叉验证：
  - 半步 buff 在校准后名额公式下的"期望化虚等待时长"是否合理（≤ 3 months in-game）
  - 若不合理，联合调整 buff 强度
- [ ] 联合回归测试矩阵：同时运行 tribulation-v1 53 单测 + halfstep-buff-v1 5 单测 + P1/P2 新增单测
- [ ] 更新 `TribulationBalanceConfig::default()` 为最终校准值

**P3 验收**：联合回归全 green；平衡监控看板显示所有指标在目标区间（模拟 1000 次渡虚劫）

---

## §8 开放问题（P0 决策门收口）

1. **动态调参**：是否实装"服务器运行时自动微调 QUOTA_PER_PLAYER"（类似 ELO 调分）or 维持人工 patch 调参
2. **跨周目名额**：plan-multi-life-v1 ✅ 的跨周目继承与本 plan 名额公式是否需要联合校准（化虚修士死亡后名额释放速度）
3. **天道介入**：是否允许天道 agent 在"名额长期空缺"时主动降低难度（触发 plan-tiandao-hunt 等辅助机制）or 维持被动观察
4. **平衡监控开放**：`/balance tribulation` 是否在某时期向玩家公开（促进社区自研战术），还是永远 dev-only

---

## P0 落地记录（多 PR plan 阶段进展，非归档 Finish Evidence）

平衡监控看板落地：`TribulationBalanceConfig` resource + `/balance tribulation` dev-only 命令，dev env mock 渡劫结算后正确显示统计、**真实暴露失衡**（含名额=0 超载这一最严重态）。

### 落地清单

- `server/src/cultivation/tribulation_balance.rs` — `TribulationBalanceConfig` resource（字段 default 镜像真实代码常数 + const-引用 pin 测试防漂移）
- `server/src/cmd/dev/balance.rs` — `/balance tribulation` brigadier 命令 + `build_balance_report`/`format_balance_report`/`handle`，挂 `cmd/dev/mod.rs`，`registry_pin.rs` 同步 pin
- 指标源（零新增 telemetry，全读既有 state）：quota_current/max ← `QuotaFullTracker`；满载率 = occupied/limit（**occupied>0 且 limit==0 → `[OVER-FULL: 名额=0 仍有 N 占用]`，不掩盖**）；halfstep/ascended 次数 ← `TribulationMetrics`；半步滞留 tick ← 在场 `HalfStepState.entered_at`；in-game 月 ← `TICKS_PER_MONTH`；**quota_k 读 live `VoidQuotaConfig`（含 env 覆盖，看板不撒谎）**

### 关键 commit（5，2026-06-13）

`f6fa8374f` Config resource · `099ac47a7` /balance 命令 · `a315d0f25` over-full/pending 测试 · `85a9616f3` 暴露名额=0 超载+live quota_k+清理 · `f511c97d2` 真实 handle() 契约测试+off-by-one

### 测试

`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` → **8890 passed / 0 failed**（含 balance 32 测试：mock 10 渡劫结算统计 / 满载率边界 / over-full INFINITY / pending 满载 / avg-stay 回拨 / live quota_k 契约 / failed n/a）

### ⚠️ 设计偏差决议（reviewer 须有意识接受）

**plan P0 规定的字段（`quota_per_player=50` / `quota_max_hard_cap=3` / `wave_1/2/3_intensity` / `heart_demon_trigger_ratio`）在真实代码里根本不存在**：真实 quota = `floor(world_qi/quota_k)`（无 player_count 维度、无硬上限 3，`quota_k`=DEFAULT_VOID_QUOTA_K=50.0）；真实心魔劫是**确定性** choice_idx 映射（`heart_demon_outcome_for_choice`，无 rand/概率）。实现做了正确工程判断——**镜像真实常数而非 plan 虚构常数**（`quota_per_player→quota_k`、`heart_demon_trigger_ratio→penalty_ratio`、`wave_*_intensity→DUXU_AOE_DAMAGE_BASE/QI_DRAIN_BASE/JUEBI_INTENSITY_BASE`），并在 `tribulation_balance.rs:18-41` doc-comment 如实记录映射。镜像现实优于镜像虚构。

### 遗留 / 后续

- **P1/P2 需 plan 层修正**：P1（`名额公式校准 QUOTA_PER_PLAYER/QUOTA_MAX`）、P2（`心魔触发概率校准`）仍引用上述**不存在的常数/概率模型**，后续阶段实施前须先按真实模型（quota_k / 确定性心魔）修正 plan 数值模型，否则撞同一断链。**此 plan-doc 修正超出 P0 consume 的 docs 写权限，留人工/下阶段处理。**
- **failed 次数无源**：`TribulationMetrics` 无 `failed_count`（settlement 把 Killed/Failed/Fled 合并只清理不计数），看板对 failed 显 `n/a（无遥测，halfstep-buff-v1 未埋点）`，**不显假 0**；failed 埋点应由数据源 plan（halfstep-buff-v1）补，非本平衡 plan 自造。
- **flaky 测试（已知 minor）**：`handle_emits_live_void_quota_k_in_chat_not_default` 在首次冷进程并行负载下偶发 1/70 失败（Bevy 资源解析时序），~70 次复跑稳定通过；生产代码正确，属测试间冷启动不确定性，CI 偶发可重跑。
- **本 P0 不产生 QiTransfer**：纯只读统计，不改 qi/quota 真实值——reverify 守恒确认。
- **CodeRabbit 日期误判**：CR 将完成日期 `2026-06-13` 标为"未来日期"，实为 CR 时钟慢；系统日期已是 2026-06-13，日期正确，保留不改。
