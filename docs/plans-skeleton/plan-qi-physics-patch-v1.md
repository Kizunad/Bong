# Bong · plan-qi-physics-patch-v1 · 骨架

把 audit 出的全部散常数 / 衰减函数 / 释放路径切到 `plan-qi-physics-v1` 提供的算子；删除原模块各自的物理常量；让所有真元/灵气流动都经 `qi_physics::ledger::QiTransfer` 守恒检查。**这是迁移收口 plan，不引入新物理**。

**世界观锚点**：N/A（执行 plan，物理由 qi-physics 锚定）

**library 锚点**：N/A

**前置硬依赖**：`plan-qi-physics-v1`(skeleton) P0 决策门 + P1 算子稳定。本 plan **不能在底盘 P1 之前启动**——没算子可调

**反向被依赖**：`plan-economy-v1` P1 / `plan-style-balance-v1` P0 / 任何"等 qi-physics 落地"的 plan，本 plan 完成后它们才能动

**交叉引用**：
- `plan-qi-physics-v1` 🆕 — **本 plan 唯一的 import 源**
- `plan-shelflife-v1` ✅ — 5 套 profile 全迁
- `plan-cultivation-v1` / `plan-cultivation-mvp-cleanup-v1` ✅ — `tick.rs` regen 已是参考实装,搬常数即可
- `plan-combat-no_ui` ✅ — `combat/decay.rs` 红线必修
- `plan-tsy-extract-v1` ✅ + `plan-tsy-zone-v1` ✅ — `tsy_drain` × `dead_zone` 合并
- `plan-tsy-lifecycle-v1` ✅ — `TsyCollapseStarted` event 接 `collapse_redistribute_qi`
- `plan-woliu-v1` ✅ — 渡劫境界离散表 vs 1/r² 决议落地
- `plan-zhenmai-v1` (jiemai) ✅ — 多常数迁移
- `plan-lingtian-v1` ✅ — `ZONE_LEAK_RATIO` 切 `qi_excretion`
- `plan-baomai-v2` / `plan-multi-style-v1` ✅ — 异体排斥 ρ 物理实装(原本缺,本 plan P3 补)
- `plan-niche-defense-v1` ✅ — 灵龛真元注入走 `qi_release_to_zone`
- `plan-tribulation-v1` ✅ — 接 `tribulation_trigger` 灾劫信号源

---

## 接入面 Checklist

- **进料**：`qi_physics::*` 全部算子（`qi_excretion` / `qi_distance_atten` / `qi_collision` / `qi_channeling` / `qi_release_to_zone` / `WorldQiAccount::transfer` / `tribulation_trigger` / `collapse_redistribute_qi`）+ `ContainerKind` / `MediumKind` / `EnvField` / `StyleAttack/Defense` trait
- **出料**：`server/src/` 下所有原物理常数 const 全删 / 衰减函数全替换为 qi_physics 调用 / 释放路径全闭合走 `QiTransfer`
- **共享类型**：完全不新增（本 plan 不引入新物理；只迁移）
- **跨仓库契约**：server 主战场；agent 端 `tiandao` arbiter 已有 conservation scaling 测试（`agent/packages/tiandao/tests/arbiter.test.ts:71`），patch 后保持不破；client 无变化
- **worldview 锚点**：N/A
- **qi_physics 锚点**：**100% 调用底盘**——这是 patch plan 存在的全部理由

---

## §0 设计轴心

- [ ] **不引入新物理**：发现迁移过程中需要新公式 / 新常数 → 停下，回 `plan-qi-physics-v1` 扩底盘，再回本 plan 调用。本 plan 不允许"顺手新增"
- [ ] **守恒等式逐 PR 闭合**：每个 patch PR 必须自带 "before / after `SPIRIT_QI_TOTAL` 守恒断言相同" 测试，**不允许把守恒漏洞合并进 main**
- [ ] **删除原 const，不留双轨**：迁移完成 = 原 const 在 codebase 中 grep 应为 0；不允许 "新加调用 + 旧 const 还在" 的过渡态停留超过 1 个 PR
- [ ] **PR 粒度可逆**：每个 patch 单元独立 PR，可单独 revert；不允许"迁了 5 个模块的大 PR"
- [ ] **测试饱和**：迁移触动既有功能 → 既有测试必须全过；新增的守恒断言独立写一组

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | **红线 6 处**（3 公式正典化 + 3 守恒律违反 fix）: ① `combat/decay.rs:5` 0.06 → 0.03 + bonus 校准 ② `tsy_drain.rs` × `dead_zone.rs` 合并 ③ `lingtian/qi_account.rs` × `Zone.spirit_qi` 合账接 `WorldQiAccount` ④ **采集奖励守恒**: `player/gameplay.rs:294` `GATHER_SPIRIT_QI_REWARD = 14.0` 凭空生 → 加 zone 对冲 ⑤ **TSY 抽取记账**: `cultivation/tsy_drain.rs:101` drain 去向走 ledger ⑥ **域崩清零记账**: `world/events.rs:1178` `zone.spirit_qi = 0.0` 走 `tribulation_trigger` 而非暴力赋零 | 6 PR 各自独立合入；每 PR 跨 24h 模拟守恒断言通过；旧符号 grep 0 |
| **P1** ⬜ | **shelflife 5 profile 全迁**: bone_coin_5/15/40_v1、ling_shi、yi_shou、chen_jiu、ling_mu_gun → 走 `qi_excretion(ContainerKind::*, env)` 统一表达；下限 clamp 到 zone qi（修复"归零到 0"违反压强法则）+ shelflife 3 公式（Linear/Exp/Stepwise）压成 `ContainerKind` 形态参数 | 5 物品类型 freshness 曲线在标准 zone 下与原行为±5% 内一致；死域 zone qi=0 时归零保持原行为 |
| **P2** ⬜ | **战斗侧 + lingtian + cultivation + 杂项**: woliu 离散表按 §5 决策落地（B 推荐: 境界 → 涡流场强度映射 + 1/r² 算子）/ jiemai 多常数迁 / cultivation tick QI_REGEN_COEF / lingtian ZONE_LEAK_RATIO / tsy_container SEARCH_DRAIN_MULTIPLIER / 跨界磨损 / 灵龛真元注入 + **守恒释放路径补全**（招式释放 / 容器衰变 / 死亡真元归还全走 `qi_release_to_zone`）| 全 server cargo test 过；`grep -rE 'BASE_LOSS_PER_BLOCK\|VORTEX_THEORETICAL_LIMIT_DELTA\|JIEMAI_*\|QI_REGEN_COEF\|ZONE_LEAK_RATIO\|SEARCH_DRAIN_MULTIPLIER' server/src/` 全 0；**SPIRIT_QI_TOTAL 全局守恒断言 24h 模拟通过** |
| **P3** ⬜ | **新机制接入**: ① `TsyCollapseStarted` event 后端挂 `collapse_redistribute_qi`(worldview §十一 line 789) ② 异体排斥 ρ 物理实装 → `StyleAttack` trait impl 给 7 流派（baomai/anqi/dugu/zhenmai/tuike/woliu/zhenfa）③ 天道时代衰减 1-3% 定时器 ④ 阈值灾劫触发器接 `tribulation.rs` ⑤ 节律 EnvField 数据源接 `plan-lingtian-weather-v1` ⑥ **汐转 TSY 周期重置守恒检查**: `tsy_lifecycle.rs:371` `compute_layer_spirit_qi` 周期改写 zone，sum 必须守恒 ⑦ **暴力清零事件灵气去向**: 渡劫完成 `tribulation.rs:1736` / 污染爆发 `contamination.rs:97` / TSY-collapse `events.rs:1643` 三处 `qi_current=0` / `zone=0` 灵气去向需明示 | 坍缩渊塌缩前后周围 zone 总和恒等；汐转前后 sum 恒等；7 流派对打能区分 ρ；100h 模拟 SPIRIT_QI_TOTAL 衰减 ∈ [1%, 3%]；阈值灾劫可触发；暴力清零事件全部走 ledger |

---

## §2 各 patch 单元详细清单（按优先级）

每条一行：`位置 | 当前 | 切到 | PR 大小估计`

### P0 红线（6 PR）—— 3 公式正典化 + 3 守恒律违反 fix

| # | 位置 | 当前 | 切到 |
|---|---|---|---|
| P0-1 | `combat/decay.rs:5,26-44` | `BASE_LOSS_PER_BLOCK = 0.06` + ColorKind/CarrierGrade bonus 散写 | `qi_physics::qi_distance_atten(initial, dist, MediumKind { color, carrier })`；正典 0.03 落地 |
| P0-2 | `world/tsy_drain.rs:22-26` + `cultivation/dead_zone.rs:11-26` | 非线性 1.5 次方 vs 线性 1.0/分钟，互不相识 | 合并到 `qi_physics::qi_excretion` + `EnvField.tsy_intensity` 调制（dead_zone 是 `tsy_intensity == 1.0` 特例） |
| P0-3 | `lingtian/qi_account.rs:3-7` 注释 TODO | `ZoneQiAccount` 与 `world::zone::Zone.spirit_qi` 暂未合账；现有 `drain_qi_to_player=0.8 / to_zone=0.2` 分账比例 | `qi_physics::ledger::WorldQiAccount` 合账层；ZoneQiAccount 退化为 facade；分账比例迁入 qi_physics constants |
| **P0-4** | `player/gameplay.rs:294` | `GATHER_SPIRIT_QI_REWARD = 14.0` 玩家采集 → +14 qi_current **无 zone 对冲**（凭空生灵气） | 改为 `qi_excretion(zone, ContainerKind::PickedHerb, ...)` 模式 → zone -= 真实抽取 + 玩家 += 抽取量；或 §4 决策"采集奖励物理来源"裁决后落地 |
| **P0-5** | `cultivation/tsy_drain.rs:101` | 玩家 `qi_current -= drain` 后 drain 去向**未记账** —— 灵气消失到无 | `WorldQiAccount::transfer(QiTransfer { from: Player, to: TsyAccumulator, amount: drain })`；坍缩渊积累的真元在 P3-1 `collapse_redistribute_qi` 时回流 |
| **P0-6** | `world/events.rs:1178` (RealmCollapse) + `events.rs:1643` (TSY collapse) | `zone.spirit_qi = 0.0` 暴力清零，灵气**消失**（一个域崩可能 = 数百单位灵气蒸发） | 改为 `tribulation_trigger(env, ledger)` 路径：清零前先 `redistribute_qi_to_neighbors(zone, surrounding)` 按压强法则分发；保留"该 zone 视觉效果归零"语义但守恒不破 |

### P1 shelflife 全迁（1-2 PR）

| # | 位置 | 当前 | 切到 |
|---|---|---|---|
| P1-1 | `shelflife/registry.rs` | 5 套 profile（bone_coin × 3 / ling_shi / yi_shou / chen_jiu / ling_mu_gun） | `qi_physics::ContainerKind` 各档；`qi_excretion(initial, container, elapsed, env)` 统一调用 |
| P1-2 | `shelflife/compute.rs:171-189` | 3 公式 Linear/Exponential/Stepwise | `ContainerKind` 内嵌曲线参数；compute 退化为 facade |
| P1-3 | `shelflife/compute.rs:18` | `DEAD_ZONE_SHELFLIFE_MULTIPLIER = 3.0` | `EnvField` 死域因子；shelflife 不再单独乘数 |

### P2 战斗 + 系统（多 PR）

| # | 位置 | 当前 | 切到 |
|---|---|---|---|
| P2-1 | `combat/woliu.rs:25-29,598-641,653-662` | `VORTEX_THEORETICAL_LIMIT_DELTA = 0.8` + 渡劫境界离散表 + 线性吸取（无 1/r²） | qi-physics §5 红线 3 决议（B 推荐）落地：境界 → 场强度 + `qi_collision` 1/r² 算子 |
| P2-2 | `combat/jiemai.rs:115-133` + `combat/components.rs:23-26` | `JIEMAI_*` 多常数 / 护甲减速 / 距离响应梯度 | 截脉作为 `StyleDefense` impl 注册物理参数 |
| P2-3 | `cultivation/tick.rs:42-45` | `QI_REGEN_COEF = 0.01` / `QI_PER_ZONE_UNIT = 50.0` | 移到 `qi_physics::constants`；tick.rs `compute_regen` 调用底盘（**已是零和参考实装，搬常数即可**） |
| P2-4 | `lingtian/growth.rs:29` | `ZONE_LEAK_RATIO = 0.2` | `qi_excretion(ContainerKind::AmbientField, env)` |
| P2-5 | `world/tsy_container.rs:179` | `SEARCH_DRAIN_MULTIPLIER = 1.5` | `EnvField.tsy_intensity` 调制 |
| P2-6 | `network/client_request_handler.rs:127-129` | 跨界磨损 0.01-0.05 | `qi_physics::wear` |
| P2-7 | **释放路径补全（横跨多模块）** | 招式释放 / 容器衰变 / 死亡真元归还**只扣不还** | 全部接 `qi_release_to_zone(amount, region, env)`；emit `QiTransfer { from, to, amount }` 进 ledger |
| P2-8 | `niche-defense` | 灵龛真元注入 | 走 `qi_release_to_zone` |
| **P2-9** | NPC 修炼路径（`npc/brain.rs` + `cultivation/tick.rs::compute_regen`） | 待查：`compute_regen` 在 NPC pathway 是否被调用？若否 → NPC 修炼真元增长但 zone 不减扣（违反守恒） | grep 验证 + 若漏接，把 NPC 修炼也走 `compute_regen` 或等价调用；零和测试加 NPC fixture |

### P3 新机制接入（多 PR）

| # | 位置 | 当前 | 切到 |
|---|---|---|---|
| P3-1 | `world/tsy_lifecycle.rs::TsyCollapseStarted` | event 已挂，但塌缩时只跑 race-out 物理 | 后端挂 `collapse_redistribute_qi(rift, surrounding_zones)` 按压强法则分发 |
| P3-2 | 7 流派模块（baomai/anqi/dugu/zhenmai/tuike/woliu/zhenfa） | grep 全库无 `rejection`/`多系排斥` 物理实现 | 各模块给攻方 entity 实装 `StyleAttack` trait（注入率/纯度/载体等参数）；ρ 系数从 worldview §四 抽 |
| P3-3 | （新建）`qi_physics::tiandao::era_decay_tick` | 无 | 新增定时器，每"时代"边界扣 SPIRIT_QI_TOTAL × [0.01, 0.03]（worldview §十） |
| P3-4 | `cultivation/tribulation.rs`(4981 行) | 主动渡劫已实装，被动"灵气阈值灾劫"未实装 | 接 `tribulation_trigger(env, ledger)`；emit `TribulationTrigger { reason: QiStarvation \| QiDensityGaze, region }` |
| P3-5 | `EnvField.rhythm` 数据源 | mock | 接 `plan-lingtian-weather-v1` 真实节律 |

---

## §3 验收（每 PR + 整体）

**每 PR 必须**：
- [ ] 守恒断言新增测试：迁移前后 `SPIRIT_QI_TOTAL` 求和相等（差 < 1e-6）
- [ ] 既有测试 100% 通过（既有行为不破）
- [ ] 旧符号 grep 输出为空（无双轨）
- [ ] PR 描述列出迁移单元 # 编号 + 切到的 qi_physics 算子

**P2 完成时整体**：
- [ ] 跨 24h 模拟（玩家修炼 + 死亡 + 招式释放 + 容器衰变 + 各种 zone 转换）守恒断言：`Σall ∈ [SPIRIT_QI_TOTAL - era_decay, SPIRIT_QI_TOTAL]`（const 引用，**不写字面 100**——SPIRIT_QI_TOTAL 是可配置 placeholder）
- [ ] `grep -rE '常数清单(见 §2)' server/src/` 全 0

**P3 完成时整体**：
- [ ] 100h 长程模拟：`SPIRIT_QI_TOTAL` 衰减落在 [1%, 3%]/时代 范围
- [ ] 坍缩渊塌缩前后周围 zone 总和恒等
- [ ] 7 流派对打的 ρ 区分可见（telemetry 维度）

---

## §4 开放问题

- [ ] **PR 粒度细化**: P0 现 6 PR，P2 估计 ~9 个独立 PR；超 200 行 diff 的 PR 要再拆吗？或按"逻辑单元"打包（如"shelflife 5 profile"算 1 PR）
- [ ] **既有 plan 是否需要回写 Finish Evidence**: 比如 plan-shelflife-v1 已 finished，本 plan 把它的 profile 全迁了，需不需要在 plan-shelflife-v1 文末补"已被 qi-physics-patch P1 取代"？还是只在本 plan 进度日志记？
- [ ] **woliu 决议风险**: P2-1 取决于 qi-physics §5 红线 3 决议（A/B/C），若决议改变本 plan P2 节奏要重排
- [ ] **测试 fixture 共享**: 24h / 100h 模拟测试是否抽出共用 harness（每 plan 独立维护一份 fixture 太重）
- [ ] **agent 侧的 conservation scaling 测试**(`agent/packages/tiandao/tests/arbiter.test.ts:71`)是否要重写适配 server 端新机制?
- [ ] **shelflife 衰变是否补 zone**（消耗型 vs 回流型）: 当前 sealed_qi 单向减少不补 zone（违反压强法则 + 守恒律），但"消耗型"也是合理设计观。worldview §二「离体真元被末法残土像海绵吸水一样瞬间分解」语义上倾向回流（分解后变 zone 基底浓度）。**默认推荐回流（守恒）**，但要在 P1 启动前裁决落到 ContainerKind 行为
- [ ] **采集奖励物理来源**: `GATHER_SPIRIT_QI_REWARD = 14.0` 是不是应该改成"按 zone 浓度抽取"模式（同 `compute_regen`）？还是保留固定 14 奖励但加 zone -= 14 对冲？**默认推荐固定奖励 + zone 对冲**——简单且不破坏当前采集体验，但 zone -= 14 / cap=zone_qi 防止抽到负
- [ ] **`drain_qi_to_player=0.8 / to_zone=0.2` 比例去向**: lingtian/qi_account.rs 现有的灵田分账比例（80% 进玩家 / 20% 留 zone）是否合理？合账到 WorldQiAccount 后这个比例由谁主张？**默认推荐**：作为 `qi_physics::constants::LINGTIAN_DRAIN_PLAYER_RATIO` 保留，灵田场景专属
- [ ] **agent IPC 精度损失**(f64→f32): server schema 是 f64，agent worldmodel 用 f32（`agent/packages/worldmodel/`），跨边界可能丢精度；100h 守恒断言时累积误差或被吃掉。需在 P3 节律对接 PR 内一并修

---

## §5 进度日志

- **2026-05-05**：骨架创建。承接 plan-qi-physics-v1（底盘 plan）—— qi-physics-v1 专注"立底盘 + 算子 + 守恒律"，本 plan 专注"逐 plan 把散常数 / 释放路径 / 新机制切到底盘"。两 plan 并行不撞 PR。前置触发：用户指令"做个 phy patch plan，把所有的都连接上 phy"。原 qi-physics-v1 的 P2/P3 内容**整体迁入本 plan**——qi-physics-v1 P2/P3 后续应缩减为"接口稳定锁定，迁移工作交 patch"
- **2026-05-05 (下午-2)**：穷尽 audit（500+ 处 qi 相关代码点）。发现 partial audit 漏掉的 8 项关键缺口，其中 **3 项直接违反守恒律**：
  - **采集奖励 +14 qi 无源**（`player/gameplay.rs:294 GATHER_SPIRIT_QI_REWARD`）→ P0-4 新增
  - **TSY 抽取 drain 去向不明**（`cultivation/tsy_drain.rs:101`）→ P0-5 新增
  - **域崩 RealmCollapse 暴力清零 zone**（`world/events.rs:1178`）→ P0-6 新增
  - 加：NPC 修炼 zone 对冲验证（P2-9）+ 汐转周期重置守恒（P3-6）+ 渡劫/污染暴力清零去向（P3-7）
  - §4 加 4 条决策点：shelflife 是否补 zone / 采集奖励物理来源 / lingtian 分账比例去向 / agent IPC f64→f32 精度
  - **P0 从 3 PR 扩到 6 PR**——3 守恒违反 fix 是 P0 因为它们在违反 worldview §二/§十 第一公理，不修就跑不出 24h 守恒断言
