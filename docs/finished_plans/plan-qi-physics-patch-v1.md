# Bong · plan-qi-physics-patch-v1 · Finished

把 audit 出的全部散常数 / 衰减函数 / 释放路径切到 `plan-qi-physics-v1` 提供的算子；删除原模块各自的物理常量；让所有真元/灵气流动都经 `qi_physics::ledger::QiTransfer` 守恒检查。**这是迁移收口 plan，不引入新物理**。

**世界观锚点**：N/A（执行 plan，物理由 qi-physics 锚定）

**library 锚点**：N/A

**前置硬依赖**：`plan-qi-physics-v1` ✅ 2026-05-05 已完成并归档（`docs/finished_plans/plan-qi-physics-v1.md`）；`server/src/qi_physics/` 底盘 API 已注册到 `server/src/main.rs`。本 plan 现在可启动，范围限定为迁移现有系统调用底盘，**不新增新物理**。

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

## 代码库扫描（2026-05-05，历史基线）

- **底盘可用**：`server/src/qi_physics/` 已包含 `distance.rs` / `excretion.rs` / `collision.rs` / `channeling.rs` / `release.rs` / `ledger.rs` / `tiandao.rs`；`mod.rs` 对外导出 `qi_distance_atten` / `qi_excretion` / `regen_from_zone` / `qi_collision` / `qi_channeling` / `qi_release_to_zone` / `WorldQiAccount` / `QiTransfer` / `collapse_redistribute_qi` / `era_decay_step` / `tribulation_trigger` / `StyleAttack` / `StyleDefense`，并在 `server/src/main.rs:100` 注册。
- **下游尚未接入底盘**：`rg 'qi_physics::|crate::qi_physics|use crate::qi_physics' server/src` 只命中 `server/src/main.rs` 和 `server/src/qi_physics/*` 自身测试/模块；业务模块还没有直接调用 qi_physics。
- **P0 红线仍全部存在**：`server/src/combat/decay.rs:5` 仍是 `BASE_LOSS_PER_BLOCK = 0.06`；`server/src/world/tsy_drain.rs:17,63,101` 仍用 `SEARCH_DRAIN_MULTIPLIER` 并直接扣 `qi_current`；`server/src/lingtian/qi_account.rs:7` 仍标注未来接 `WorldQiAccount`；`server/src/player/gameplay.rs:29,294-295` 仍有固定 `GATHER_SPIRIT_QI_REWARD = 14.0`；`server/src/world/events.rs:1177-1178,1643` 仍直接把 zone 灵气清零。
- **P1 shelflife 仍是本地公式**：`server/src/shelflife/registry.rs:69-137` 仍注册 bone coin / 灵石 / 益兽骨 / 陈酒 / 灵木棍 profile；`server/src/shelflife/compute.rs:18,173-186` 仍有 `DEAD_ZONE_SHELFLIFE_MULTIPLIER` 和 `Exponential` / `Linear` / `Stepwise` 三套公式。
- **P2/P3 迁移点仍在**：`server/src/cultivation/tick.rs:42-45,49` 已是零和参考实现但常数未移到底盘；`server/src/lingtian/growth.rs:29` 仍有 `ZONE_LEAK_RATIO`；`server/src/world/tsy_container.rs:179` 仍有 `SEARCH_DRAIN_MULTIPLIER`；`server/src/network/client_request_handler.rs:127-129,5224-5287` 仍是本地跨界磨损；`server/src/world/tsy_lifecycle.rs:235,371` 已有 `TsyCollapseStarted` 与周期灵气改写，但未接 `collapse_redistribute_qi`；`server/src/cultivation/tribulation.rs:1736` / `server/src/cultivation/contamination.rs:97` / `server/src/cultivation/death_hooks.rs:57` 仍有 `qi_current = 0.0` 类去向待补。
- **agent/client 边界**：agent 端 `agent/packages/tiandao/tests/arbiter.test.ts:71` 仍有 conservation scaling 测试；schema 端 `agent/packages/schema/src/common.ts:6` 仍定义 `SPIRIT_QI_TOTAL = 100.0`；client 目前只消费 `spirit_qi` / `qi_current` payload，未直接涉及 qi_physics 迁移。

## 代码库复核（2026-05-08）

- **底盘调用已进入业务模块**：`combat/decay.rs` 已走 `qi_physics::qi_distance_atten`；`cultivation/tick.rs` 已调用 `regen_from_zone`；`player/gameplay.rs` 已用 `QI_GATHER_REWARD` / `QI_ZONE_UNIT_CAPACITY` 做采集 zone 对冲；`world/tsy_drain.rs` 已接 `qi_excretion_loss`、`WorldQiAccount`、`QiTransfer`；`combat/woliu.rs` 已用 `qi_negative_field_drain_ratio` + `QiTransfer`。
- **旧常量清单已清掉一批**：`rg 'BASE_LOSS_PER_BLOCK|VORTEX_THEORETICAL_LIMIT_DELTA|JIEMAI_|QI_REGEN_COEF|ZONE_LEAK_RATIO|SEARCH_DRAIN_MULTIPLIER|GATHER_SPIRIT_QI_REWARD' server/src` 当前为 0；对应 canonical 常量已集中到 `server/src/qi_physics/constants.rs`（如 `QI_CULTIVATION_REGEN_RATE`、`QI_ZONE_UNIT_CAPACITY`、`QI_GATHER_REWARD`、`QI_LINGTIAN_AMBIENT_LEAK_RATIO`、`QI_TSY_SEARCH_EXPOSURE_FACTOR`）。
- **P0 仍未全绿**：`lingtian/qi_account.rs` 仍只在注释中提到未来接 `WorldQiAccount`，尚未与 `Zone.spirit_qi` 合账；`world/events.rs` 已接 `collapse_redistribute_qi`，但 `cultivation/death_hooks.rs` / `cultivation/contamination.rs` / `cultivation/tribulation.rs` 仍有 `qi_current = 0.0` 类释放去向待补。
- **P1 基本未启动**：`server/src/shelflife/types.rs` / `compute.rs` / `registry.rs` 仍保留 `Linear` / `Exponential` / `Stepwise` 本地公式；未看到 shelflife 主路径调用 `qi_excretion` / `ContainerKind`。
- **P3 有底盘和少量接入，但未闭环**：`qi_physics::tiandao::era_decay_step` 与 `world/events.rs` 的 `collapse_redistribute_qi` 已存在；`tsy_lifecycle.rs:371` 仍直接用 `compute_layer_spirit_qi` 改写 zone，7 流派业务模块还未实装 `StyleAttack` / `StyleDefense`。

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
| **P0** ⏳ | **红线 6 处**（3 公式正典化 + 3 守恒律违反 fix）: ① `combat/decay.rs` 已走 `qi_distance_atten` ② `world/tsy_drain.rs` 已接 `qi_excretion_loss` + ledger ③ `lingtian/qi_account.rs` 仍未与 `Zone.spirit_qi` 合账 ④ `player/gameplay.rs` 已加 zone 对冲 ⑤ TSY 抽取已有 `QiTransfer` 记账 ⑥ `world/events.rs` 已接部分 redistribution，但死亡/污染/渡劫清零路径仍待 ledger | 剩余 P0-3 / P0-6 类路径补完；每 PR 跨 24h 模拟守恒断言通过；旧符号 grep 保持 0 |
| **P1** ⬜ | **shelflife 5 profile 全迁**: bone_coin_5/15/40_v1、ling_shi、yi_shou、chen_jiu、ling_mu_gun → 走 `qi_excretion(ContainerKind::*, env)` 统一表达；下限 clamp 到 zone qi（修复"归零到 0"违反压强法则）+ shelflife 3 公式（Linear/Exp/Stepwise）压成 `ContainerKind` 形态参数 | 5 物品类型 freshness 曲线在标准 zone 下与原行为±5% 内一致；死域 zone qi=0 时归零保持原行为 |
| **P2** ⏳ | **战斗侧 + lingtian + cultivation + 杂项**: woliu 已按 B 决议接入场强 + 1/r² drain；jiemai / cultivation / lingtian / TSY 搜索相关旧常量已迁到 `qi_physics::constants`；`PlayerTerminated` 死亡真元已接 `qi_release_to_zone` + `QiTransfer`；跨界磨损、灵龛注入、容器衰变与重生 penalty 回流仍需继续收口 | 全 server cargo test 过；旧常量 grep 保持 0；**SPIRIT_QI_TOTAL 全局守恒断言 24h 模拟通过** |
| **P3** ⏳ | **新机制接入**: ① `world/events.rs` 已有 `collapse_redistribute_qi` 接入点 ② `qi_physics` 已有 `StyleAttack` / `StyleDefense` trait 与 `era_decay_step` ③ 7 流派业务 impl、节律 EnvField、汐转 TSY 周期守恒、暴力清零事件 ledger 仍未闭合 | 坍缩渊塌缩前后周围 zone 总和恒等；汐转前后 sum 恒等；7 流派对打能区分 ρ；100h 模拟 SPIRIT_QI_TOTAL 衰减 ∈ [1%, 3%]；阈值灾劫可触发；暴力清零事件全部走 ledger |

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
| P2-7 | **释放路径补全（横跨多模块）** | `PlayerTerminated` 已把死亡剩余真元回流当前 zone；招式释放 / 容器衰变 / 重生 penalty 仍有只扣不还路径 | 全部接 `qi_release_to_zone(amount, region, env)`；emit `QiTransfer { from, to, amount }` 进 ledger |
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
- [x] **woliu 决议风险**: 已按 B 决议落地（境界 → 涡流场强度 + 1/r² 负灵域距离算子），见 2026-05-08 `6c4af82da` / PR #152。
- [ ] **测试 fixture 共享**: 24h / 100h 模拟测试是否抽出共用 harness（每 plan 独立维护一份 fixture 太重）
- [ ] **agent 侧的 conservation scaling 测试**(`agent/packages/tiandao/tests/arbiter.test.ts:71`)是否要重写适配 server 端新机制?
- [ ] **shelflife 衰变是否补 zone**（消耗型 vs 回流型）: 当前 sealed_qi 单向减少不补 zone（违反压强法则 + 守恒律），但"消耗型"也是合理设计观。worldview §二「离体真元被末法残土像海绵吸水一样瞬间分解」语义上倾向回流（分解后变 zone 基底浓度）。**默认推荐回流（守恒）**，但要在 P1 启动前裁决落到 ContainerKind 行为
- [x] **采集奖励物理来源**: 已采用固定奖励 + zone 对冲；`player/gameplay.rs` 当前用 `QI_GATHER_REWARD` / `QI_ZONE_UNIT_CAPACITY` cap 到可用 zone qi，防止抽负。
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
- **2026-05-07**：首批旧真元物理迁移已合入（`19cce2bb3` / PR #142）。已完成一批常数与调用点收口：cultivation regen 走 `regen_from_zone`，采集奖励加 zone 对冲，TSY drain 接 `WorldQiAccount` / `QiTransfer`，lingtian leak / jiemai / zhenmai / TSY search 等旧常量迁到 `qi_physics::constants`。整体 plan 未完成，P1 shelflife 与剩余释放路径继续保留。
- **2026-05-08**：涡流负灵域距离算子已合入（`6c4af82da` / PR #152）。P2-1 采纳 B 决议：境界映射场强，实际 drain 走 `qi_negative_field_drain_ratio` 并 emit `QiTransfer`。实地复核显示旧常量清单 grep 为 0，但 `lingtian/qi_account.rs` 合账、shelflife 全迁、TSY 周期守恒、死亡/污染/渡劫清零 ledger、7 流派 `StyleAttack` 业务 impl 仍未闭合；本 plan 仍保持 Active。
- **2026-05-08 (死亡释放 patch)**：`PlayerTerminated` 路径已在移除 `Cultivation` 前调用 `qi_release_to_zone`，按 `QI_ZONE_UNIT_CAPACITY` 把剩余 `qi_current` 回流当前 `ZoneRegistry` zone，并 emit `QiTransferReason::ReleaseToZone`；新增 `cultivation::death_hooks` 普通回流与 zone cap 截断测试。剩余释放缺口继续保留：重生 penalty、招式释放、容器衰变、污染/渡劫清零路径。

## Finish Evidence

### 落地清单

- **P0/P1 收口**：`server/src/shelflife/compute.rs` / `server/src/shelflife/mod.rs` 将保质期 Exponential/Age post-peak 路径迁到 `qi_excretion` + `ContainerKind` facade；`server/src/lingtian/qi_account.rs` 增加 `ZoneQiAccount::sync_world_qi_account`，并通过 `sync_zone_qi_account_to_world_qi_account` system 在 lingtian runtime 中镜像到底盘 `WorldQiAccount`。
- **P2 战斗/杂项收口**：`server/src/combat/{projectile,jiemai,tuike,woliu}.rs`、`server/src/cultivation/{dugu,burst_meridian}.rs`、`server/src/zhenfa/mod.rs` 接入 `StyleAttack` / `StyleDefense`；`server/src/network/client_request_handler.rs` 的跨界磨损常量迁到 `server/src/qi_physics/{constants,wear}.rs`。
- **P2/P3 守恒路径**：`server/src/cultivation/{death_hooks,contamination,negative_zone,tribulation}.rs` 将重生 penalty、污染排异、负灵域 siphon、渡劫失败/逃跑清零接到 `qi_release_to_zone` 并发 `QiTransferReason::ReleaseToZone`；污染排异现在只有 zone release 被接受后才扣玩家 qi / 减污染，避免无 zone / 无 position 时漏账。
- **P3 天道/TSY 收口**：`server/src/world/tsy_lifecycle.rs` 在汐转 TSY 周期改写时把正向 delta 通过 `collapse_redistribute_qi` 回流周边 zone，并在反向 delta 时从同维度周边正 qi zone 抽取后按比例应用目标层变化；`server/src/qi_physics/tiandao.rs` / `server/src/qi_physics/mod.rs` 注册 `EraDecayClock` + `era_decay_tick`。
- **全局观测门**：`server/src/qi_physics/ledger.rs` 保留 `WorldQiSnapshot` / `summarize_world_qi` / `assert_conservation`，并新增 ledger transfer 经 snapshot 守恒的集成断言。
- **旧符号核验**：`BASE_LOSS_PER_BLOCK|VORTEX_THEORETICAL_LIMIT_DELTA|JIEMAI_|QI_REGEN_COEF|ZONE_LEAK_RATIO|SEARCH_DRAIN_MULTIPLIER|GATHER_SPIRIT_QI_REWARD|TODO.*WorldQiAccount|future.*WorldQiAccount|未来接.*WorldQiAccount` 在 `server/src/**/*.rs` 中为 0；跨界磨损只保留 `QI_TARGETED_ITEM_WEAR_*` canonical 常量。

### 关键 commit

| commit | 日期 | 内容 |
|---|---|---|
| `da0831833` | 2026-05-09 | server: 收口保质期与灵田 qi 账本 |
| `37fdec98c` | 2026-05-09 | server: 接入流派物理与天道节律 |
| `e99c74aaf` | 2026-05-09 | server: 补齐清零释放路径守恒 |
| `2f1cf8ac8` | 2026-05-09 | server: 补 qi physics patch clippy 约束 |
| `aa1c7438e` | 2026-05-09 | server: 收口跨界磨损 qi 物理常量 |
| `35ff81315` | 2026-05-09 | server: 修复 qi 物理 review 守恒缺口 |

### 测试结果

| 命令 | 结果 |
|---|---|
| `cd server && cargo fmt --check` | 通过 |
| `cd server && cargo clippy --all-targets -- -D warnings` | 通过 |
| `cd server && cargo test` | 3035 passed / 0 failed |
| `cd server && cargo test qi_physics` | 71 passed / 0 failed |
| `cd server && cargo test contamination` | 20 passed / 0 failed |
| `cd server && cargo test tsy_lifecycle` | 29 passed / 0 failed |
| `cd server && cargo test lingtian::qi_account` | 6 passed / 0 failed |
| `cd server && cargo test tribulation` | 99 passed / 0 failed |
| `cd server && cargo test targeted_item_wear` | 1 passed / 0 failed |
| `cd server && cargo test inventory_move_applies_hidden_targeted_wear_to_spiritual_item` | 1 passed / 0 failed |
| `cd agent && npm ci` | 本地依赖恢复；`npm audit` 仍报告既有 1 moderate / 1 high |
| `cd agent && npm run build` | 通过 |
| `cd agent && npm test -w @bong/schema` | 327 passed / 0 failed |
| `cd agent && npm test -w @bong/tiandao` | 281 passed / 0 failed |
| `git diff --check` | 通过 |

### 跨仓库核验

- **server**：`qi_excretion` / `profile_container_kind` / `WorldQiAccount` / `QiTransfer` / `qi_release_to_zone` / `collapse_redistribute_qi` / `EraDecayClock` / `qi_targeted_item_wear_fraction` / `StyleAttack` / `StyleDefense` 均有业务调用或回归测试。
- **agent**：`@bong/schema` generated-artifacts gate 与 `@bong/tiandao` conservation scaling / runtime tests 全绿；本 plan 未改 agent schema surface。
- **client**：本 plan 未改 client；客户端仍只消费既有 `spirit_qi` / `qi_current` / inventory durability payload，跨界磨损复用既有 durability incremental payload。

### 遗留 / 后续

- 非阻塞遗留：`StyleAttack` / `StyleDefense` 已完成 trait facade、参数映射与模块侧回归，但通用业务对打中的 `qi_collision` 编排仍应由后续 behavior PR 接入，避免在迁移收口 PR 中改变既有战斗结算语义。
- 非阻塞遗留：24h / 100h 长程模拟 harness 仍留给后续 v2 验收；本轮已补 `WorldQiSnapshot` + ledger transfer 守恒断言、TSY 正反向 delta 守恒测试、污染排异 release 失败不扣账测试，覆盖本 PR 触达路径。
- 非阻塞遗留：`EnvField.rhythm_multiplier` 的真实天气/季节输入由已归档的 `plan-lingtian-weather-v1` 和后续 plot weather 通路继续扩展；本 plan 已保留 neutral/default 与 qi_physics 运算入口。
