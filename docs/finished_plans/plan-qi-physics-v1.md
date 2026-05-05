# Bong · plan-qi-physics-v1

修仙物理底盘——把 worldview §二「真元极易挥发」做成代码里**唯一**的真元/灵气物理实现入口；所有涉及真元衰减/逸散/距离损耗/异体排斥/吸力/反震的 plan 都向它取值，不再各自拍数。**这是工程化基建 plan，不是新玩法**。

**世界观锚点**：`worldview.md §二 真元极易挥发(核心)` · `§四 战斗系统(距离衰减 0.03/格 已正典 + 异体排斥)` · `§九 骨币半衰` · `§十六 末法残土抽真元` · `§十七 末法节律`

**library 锚点**：`cultivation-0002 烬灰子内观笔记`(缚/噬/音/影 四论) · `world-0004 骨币半衰录`

**反向被依赖**：`plan-qi-physics-patch-v1` 🆕 — **本 plan P1 完成后由它接管所有迁移工作**

**交叉引用（既有都是它的 client，迁移由 patch plan 执行）**：

- `plan-shelflife-v1` ✅ — 5 套独立 profile 待统一
- `plan-economy-v1` 🆕(skeleton) — 骨币半衰直接走 qi_physics::excretion
- `plan-cultivation-v1` ✅ — B_idle / qi_zero_decay / QI_REGEN_COEF 待迁
- `plan-combat-no_ui` ✅ — 距离衰减硬编 0.06 ⚠️ 与正典 0.03 冲突
- `plan-tsy-extract-v1` ✅ — TSY 1.5 次方非线性待整合
- `plan-tsy-zone-v1` ✅ — dead_zone 线性 1.0/分待整合
- `plan-woliu-v1` ✅ — 渡劫境界离散表 vs 1/r² 物理待决
- `plan-zhenmai-v1` (jiemai) ✅ — 反震/接触响应常数待迁
- `plan-baomai-v2` / `plan-multi-style-v1` ✅ — ρ 异体排斥待实装
- `plan-lingtian-v1` ✅ — ZONE_LEAK_RATIO 0.2 待迁
- `plan-lingtian-weather-v1` ⏳ — 节律 EnvField 数据源
- `plan-style-balance-v1` 🆕(skeleton) — 退化为 qi_physics 的 combat 应用层

---

## 接入面 Checklist

- **进料**：worldview §二/§四/§九/§十六/§十七 正典物理参数；audit 报告（§4）出的现有散常数
- **出料**：`qi_physics::*` 通用算子被所有真元相关 plan 调用；各 plan 只声明物理参数（注入率/纯度/容器/活动半径），底层公式归此处唯一实现
- **共享类型**：`ContainerKind`(SealedInBone/LooseInPill/WieldedInWeapon/AmbientField/SealedAncientRelic) + `MediumKind` + `EnvField`(节律状态/局部灵气浓度) + `StyleAttack` trait + `StyleDefense` trait + `CollisionOutcome`
- **跨仓库契约**：server 端 `qi_physics` module(主)；agent 端 narration 取统一数据源；client 端 tooltip 显"真元残量"语义化（已有 `spirit_quality (0..=1)` 通用字段）
- **worldview 锚点**：§二 是核心；§四 §九 §十六 §十七 都是 §二 的应用面
- **qi_physics 锚点**：N/A（**本 plan 即定义 qi_physics**）

---

## §0 设计轴心

- [ ] **守恒律（第一公理）**：worldview §二/§十 正典「全服灵气总量 `SPIRIT_QI_TOTAL` 恒定且缓慢衰减；修炼消耗 = 别人少掉」。**`SPIRIT_QI_TOTAL` 走 server config 初始化为 `WorldQiBudget` Resource（默认 100.0 只是配置默认值 / fixture 值）**——所有守恒断言取 `WorldQiBudget.current_total` / `initial_total`，不引用散落 const、不写字面 100。代码里所有真元/灵气流动都是**质量流向**，不是数值生成或消失：
  - 修士吸收 = `zone.spirit_qi -= drain` + `cultivation.qi_current += gain`（已实装于 `cultivation/tick.rs::compute_regen`，注释"零和"）
  - 修士释放（招式 / 阵法 / 容器封灵衰变）= 反向：`cultivation.qi_current -= cost` + 真元归还到环境（**当前代码未完全实装，多处只扣不还**）
  - 唯一允许的"系统外流出"= 天道每个时代衰减 1-3%（`QI_TIANDAO_DECAY_PER_ERA_*`）
  - 区域灵气低于阈值 → 天道引发灾劫**收割**真元（worldview §九/§十一 "灵物密度阈值" + §十六.一 坍缩渊压缩）
- [ ] **压强法则（worldview §二 / §三 line 32）**：「灵气的流动遵循**压强法则**——从高浓度流向低浓度，如同物理定律」。所有逸散 / 吸收都是压差驱动，不是单向衰减：
  - 容器内浓度 > zone 浓度 → 顺压差逸散（骨币漏回 zone）
  - 容器内浓度 = zone 浓度 → 平衡，逸散停止
  - 容器内浓度 < zone 浓度 → 理论上反向（zone 回灌容器，但密封容器不允许 → 静默 0）
  - **`qi_excretion` 的物理下限 = `env.local_zone_qi`**，不是 0
  - **真正归零的位置 = 死域 / 坍缩渊（`zone.spirit_qi == 0`）** —— 这正是 worldview §十六.三「带满灵骨币入坍缩渊 → 变成普通骨头」的物理来源；正常 zone 里骨币会渐近 zone 基底，不会归零
  - 玩法后果：储藏地点变成战略选择——聚灵阵 / 灵田保值，废地 / 末法残土真贬值；与 §十一「灵物密度阈值天道注视」形成张力（藏太多反招天道）
- [ ] **坍缩渊是质量中转站，不是终点**（worldview §十一 line 789「任由活坍缩渊自然塌缩为死坍缩渊，**高效回收灵气**」）：
  - 活坍缩渊吸进的真元（修士死亡 / 骨币归零 / 入口剥离 §十六.三）暂存在坍缩渊内部账本，**不离开 `WorldQiBudget` 守恒域**
  - 塌缩瞬间（最后一件遗物被取，§十六.一 step 4）→ `collapse_redistribute_qi(rift, surrounding_zones)`：**暂存真元一次性按压强法则分发回周围 zone**（低浓度方向多分，高浓度方向少分）
  - 死坍缩渊本身仍是 `zone_qi = 0`（真元已分发完毕，符合 §二 "死坍缩渊负压空洞"）
  - 净结果：坍缩渊 = 天道的"灵气拖把"，把局部聚集的真元擦回稀薄区域（worldview §十一 "等塌回收"）—— 守恒，但有时间延迟
- [ ] **唯一物理实现入口 + 物理参数/玩法配置分层**：worldview §二「真元极易挥发」在代码里**只有一份实现**——`qi_physics` 放底层常量（距离衰减率、异体排斥基础率、声学阈值等 worldview 锚定参数）；各 plan 只**注册物理参数实例**（流派纯度/频率、容器密封等级、招式注入率等），通用算子组合两者。散落各处的衰减/逸散常数全部回收进 `qi_physics::constants`，不允许 plan 内自定
- [ ] **加 plan 调底盘，不绕过**：新 plan 涉及真元相关的衰减/逸散公式 → 必查 `qi_physics`，不存在就先扩底盘再用。已写入 `docs/CLAUDE.md §四 红旗`
- [ ] **既有代码迁移而非并行**：`plan-qi-physics-patch-v1` 把现有散常数全部迁过来调用 `qi_physics`，**不允许"加一个新底盘但旧代码不动"留双轨**
- [ ] **常量唯一锚定 worldview**：每个 const 注释必须写出 worldview 章节出处；非 worldview 锚定的 const 要么补 worldview，要么挪出 qi_physics

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ✅ 2026-05-05 | **决策 + 脚手架 + 类型骨架**：4 红线 §5 决策门收口 + `server/src/qi_physics/{mod,constants,ledger,traits,env}.rs` 空壳模块 + `qi_physics::constants` 全部常量落地（worldview 锚定 11 个真值 + 5 个 v0.1 placeholder，见 §2）+ `ContainerKind` / `MediumKind` / `EnvField` enum/struct 定义 + `StyleAttack` / `StyleDefense` / `Container` trait 签名 + `WorldQiBudget` config/resource（server config 初始化，测试可注入）+ `WorldQiAccount` resource + `QiTransfer` event 类型 + `summarize_world_qi(world) -> WorldQiSnapshot` 签名（实现可 todo!()） | `cargo clippy/test/fmt` 全过；qi_physics 模块可被 import 但暂不被既有代码调用；4 红线决策结论写入 §5；不动 server 既有模块代码 |
| **P1** ✅ 2026-05-05 | **算子完整实装 + 守恒断言闭合 + 测试饱和**：5 算子全部物理 logic 落地（`qi_excretion` 含 zone 浓度下限 clamp / `qi_distance_atten` 接 `MediumKind` / `qi_collision` 写双向 ledger / `qi_channeling` 注入吸取统一接口 / `qi_release_to_zone` 显式释放）+ `WorldQiAccount::transfer` 实装 + `summarize_world_qi` 实装（遍历 ECS Cultivation/Inventory + Zone resource 求和）+ 守恒不变量断言 + StyleAttack/Defense trait 默认实装（7 流派可直接 impl）+ **测试矩阵 ≥ 40 单测**（见 §1.5 拆分） | qi_physics::ledger 守恒断言 24h fixture 模拟通过（不依赖既有代码）；trait 默认 impl 可被 7 流派 + 经济 / shelflife 直接 import；**仍不动既有代码**——衔接 / 迁移属 patch plan |
| ~~P2~~ | **迁移工作整体迁出到 `plan-qi-physics-patch-v1`** — 本 plan P1 完成后即冻结底盘 API，迁移由 patch plan 承接 | (承接,见 patch plan §1 P0/P1/P2) |
| ~~P3~~ | **新机制接入也迁出到 `plan-qi-physics-patch-v1`** — 坍缩渊 redistribute / 异体排斥 ρ / 时代衰减 / 阈值灾劫 / 节律对接 | (承接,见 patch plan §1 P3) |

**P0 决策门**：完成前 §5 四个红线问题必须有答案，否则 qi_physics 出生就分裂。

**Plan 范围**：本 plan **只做 P0 + P1**（决策 + 立底盘 + 算子 + 账本 + trait——核心 phy 做到位）。P1 完成 = 底盘 API 冻结、单测齐备、可被 import。**所有迁移 / 新机制接入工作转 `plan-qi-physics-patch-v1` 承接**——两 plan 并行不撞 PR：本 plan **不动既有 server 模块**（combat/cultivation/shelflife/lingtian 等），只在新建的 `qi_physics/` 模块内部完整实装；patch plan 切既有代码到 qi_physics 算子，不动 qi_physics 内部 API。

---

## §1.5 P1 测试矩阵（饱和化测试要求）

CLAUDE.md `## Testing — 饱和化测试`：每函数测 ① happy ② 边界 ③ 错误分支 ④ 状态转换。下限 **40 单测**：

| 模块 | 测试组 | 下限 |
|---|---|---|
| `qi_excretion` | happy（高浓度顺压差） + 边界（容器=zone 平衡 / 容器<zone 静默 0） + 错误（initial 负值 / elapsed 0） + ContainerKind 全 variant | 6 |
| `qi_distance_atten` | happy（短距） + 边界（dist=0 / 极远归零） + MediumKind 全 variant（ColorKind × CarrierGrade） | 6 |
| `qi_collision` | happy（攻防对打写 ledger 双向） + 边界（攻方耗尽 / 防方反向获利 clamp 0.5）+ 错误（无效 trait） + 7 流派对位 sample | 8 |
| `qi_channeling` | 注入 / 吸取 / 阵法聚灵 三方向各 happy + 边界 | 5 |
| `qi_release_to_zone` | happy（释放到 zone） + 边界（zone qi 满 cap / 多人同 zone += 顺序无关） + ledger event 写入断言 | 5 |
| `WorldQiAccount::transfer` | 单 transfer 改两端 + 100 连续 transfer 求和不变（属性测试） + 拒绝超额 + 跨账户类型（player↔zone↔container） + era_decay 累计 | 5 |
| `summarize_world_qi` | 空 world + 单玩家 + 多玩家 + 含容器 + 含 zone | 5 |

**P1 验收**：`grep -rcE '#\[test\]' server/src/qi_physics/` ≥ 40。守恒不变量：跨 24h tick fixture 模拟（不依赖既有代码，纯 qi_physics 内 fixture），`Σall ≈ WorldQiBudget.current_total`，且 `current_total = initial_total - era_decay_accum`。

---

## §2 物理常量目录（P0 落地清单）

worldview 锚定的底层常量，全局唯一：

```rust
pub mod constants {
    // §四 距离衰减(已正典)
    pub const QI_DECAY_PER_BLOCK: f64 = 0.03;

    // §二 离体真元被末法残土分解(基础速率,容器衰减由 ContainerKind 调制)
    pub const QI_AMBIENT_EXCRETION_PER_SEC: f64 = 0.001;  // v0.1 placeholder,patch P3 校准

    // §四 异体排斥基础率(攻方注入真元被宿主排斥的最低耗散)
    pub const QI_EXCRETION_BASE: f64 = 0.30;          // 来自 §P.2 α

    // §四 声学激发阈值(纯度低于此 → 截脉不激发,W=0)
    pub const QI_ACOUSTIC_THRESHOLD: f64 = 0.40;

    // §四 涡流 1/r² 系数 + 反向获利上限
    pub const QI_NEGATIVE_FIELD_K: f64 = 1.0;
    pub const QI_DRAIN_CLAMP: f64 = 0.50;             // 涡流反向获利 ≤ 0.5 (§P.2)

    // §九 骨币半衰参考(worldview "1 月剩 ~20%",对应半衰期 ≈ 13 天)
    pub const QI_HALFLIFE_REFERENCE_DAYS: f64 = 13.0;  // v0.1 placeholder(从 worldview 推),patch P3 校准

    // §四 防御维持基线(B_idle)
    pub const QI_MAINTENANCE_IDLE: f64 = 1.0;

    // §十六 末法残土抽真元强度(独立于 ambient,因坍缩渊有强负压)
    pub const QI_TSY_DRAIN_FACTOR: f64 = 0.5;          // v0.1 placeholder(取自 tsy_drain.rs:24 BASE_DRAIN_PER_TICK),patch P3 校准
    pub const QI_TSY_DRAIN_NONLINEAR_EXPONENT: f64 = 1.5;  // 同上(来自 tsy_drain.rs:23)

    // §十七 节律基础乘数(冬/凝汐=1.0 / 夏/炎汐=1.2 / 汐转=random)
    pub const QI_RHYTHM_NEUTRAL: f64 = 1.0;
    pub const QI_RHYTHM_ACTIVE:  f64 = 1.2;
    pub const QI_RHYTHM_TURBULENT_RANGE: (f64, f64) = (0.7, 1.5);

    // ───── 守恒律 / 天道收割（worldview §二/§十/§十一/§十六）─────

    // §十 全服灵气预算默认值；生产值由 server config 初始化为 WorldQiBudget
    // 守恒代码只读 WorldQiBudget，不直接依赖该默认值
    pub const DEFAULT_SPIRIT_QI_TOTAL: f64 = 100.0;

    // §十 天道每时代衰减(唯一允许的系统外流出)
    pub const QI_TIANDAO_DECAY_PER_ERA_MIN: f64 = 0.01;
    pub const QI_TIANDAO_DECAY_PER_ERA_MAX: f64 = 0.03;

    // §十一 灵物密度阈值(区块灵气浓度超此 → 天道注视,清零或刷高阶道伥)
    pub const QI_DENSITY_GAZE_THRESHOLD: f64 = 0.85;    // v0.1 placeholder(zone 范围 [-1, 1],85% 高密度),patch P3 校准

    // 区域灵气低于阈值 → 天道触发劫数收割(worldview §九 死域 + §十一 隐性收割)
    pub const QI_REGION_STARVATION_THRESHOLD: f64 = 0.1;  // v0.1 placeholder(取自 events.rs:REALM_COLLAPSE_LOW_QI_THRESHOLD),patch P3 校准

    // §四 修炼吸纳系数(已在 cultivation/tick.rs:42-45,迁移过来)
    pub const QI_REGEN_COEF: f64 = 0.01;
    pub const QI_PER_ZONE_UNIT: f64 = 50.0;             // 1.0 zone unit 兑换多少玩家 qi
}
```

**v0.1 placeholder**：5 个数值标 placeholder 是因为这些**不是设计裁决**（数值校准而已）——P0 给可运行的初值（来自现有代码 grep 或 worldview 推导），P1 单测固定它们，patch P3 实跑模拟时校准。**P0 决策门只覆盖 §5 四红线**（设计层面），数值校准不阻塞 P0。

---

## §3 数据契约

```
server/src/qi_physics/
├── mod.rs         — 模块入口 + re-exports + Plugin(注册 Resource/Event/system)
├── constants.rs   — §2 全部底层常量(worldview 锚注释)
├── env.rs         — EnvField struct(节律/局部浓度/末法强度/tsy_intensity);
│                   ContainerKind/MediumKind enum + variant 物理参数表
├── traits.rs      — StyleAttack / StyleDefense / Container trait + 默认 impl
├── ledger.rs      — WorldQiAccount Resource(只读快照接口,不存储余额)
│                  + summarize_world_qi(world: &World) → WorldQiSnapshot
│                    (遍历 ECS:Cultivation.qi_current + Zone.spirit_qi
│                     + ItemInstance.spirit_quality 求和; era_decay_accum 单独 track)
│                  + QiTransfer { from, to, amount, reason } Event(事件流,telemetry)
│                  + 守恒不变量函数: assert_conservation(snap_t0, snap_t1, transfers, era_decay)
│                    检查 snap_t1 - snap_t0 == sum_of_transfers - era_decay
├── excretion.rs   — **被动**:时间驱动 tick-system 调用
│                   qi_excretion(initial, container, elapsed, env) → f64
│                   (容器衰变;**下限 clamp 到 env.local_zone_qi**,压强法则)
│                   regen_from_zone(zone_qi, rate, integrity, room) → (gain, drain)
│                   (修炼吸纳,移植 cultivation/tick.rs::compute_regen,反向逸散)
│                   两者都生成 QiTransfer 写 ledger
├── release.rs     — **主动**:事件驱动显式调用
│                   qi_release_to_zone(amount, region, env) → ()
│                   (招式释放/死亡真元归还/阈值消失;生成 QiTransfer:player → zone)
│                   ❌ **不含**容器衰变(那是 excretion 的事)
├── distance.rs    — qi_distance_atten(initial, dist, medium) → f64
│                   (medium = MediumKind { color: ColorKind, carrier: CarrierGrade })
│                   纯计算函数,**不写 ledger**(由 collision 调用并写)
├── collision.rs   — qi_collision(atk: &dyn StyleAttack, def: &dyn StyleDefense,
│                                   dist: f64, env: &EnvField) → CollisionOutcome
│                   (内部直接写 QiTransfer 双向到 ledger,不递归调用其他算子;
│                    内部调 qi_distance_atten 做距离衰减纯计算;
│                    ρ/W/β/K_drain 在此组装出 H_eff + DPS_qi)
├── channeling.rs  — qi_channeling(注入/吸取统一接口,涡流/截脉/聚灵阵共用)
│                   生成 QiTransfer 写 ledger
└── tiandao.rs     — tribulation_trigger(env, ledger) → Option<TribulationCause>
                    (区域灵气 < QI_REGION_STARVATION_THRESHOLD 触发收割劫;
                     密度 > QI_DENSITY_GAZE_THRESHOLD 触发注视劫;
                     **本 plan 只定义函数 + 类型,挂 server event chain 是 patch P3**)
                   + collapse_redistribute_qi(rift, surrounding_zones)
                    (坍缩渊塌缩时把暂存真元按压强法则分发回周围 zone,
                     **本 plan 提供算法,接 TsyCollapseStarted event 是 patch P3**)
                   + era_decay_step(ledger, era_factor) → ()
                    (天道时代衰减步进,本 plan 提供算法,挂定时器是 patch P3)
```

**跨仓库契约边界**：本 plan **不引入跨仓库 schema** —— qi_physics 是 server 端内部模块。agent 端 schema (`qi_physics.ts`) + client 端 tooltip + IPC channel 全部由 **patch plan P3** 接入（跟节律 / 坍缩渊 redistribute 一起做）。本 plan P1 只暴露 server 端 `pub fn snapshot_for_ipc()` 占位接口（实装可 todo!()），patch P3 接 IPC。

---

## §4 既有代码 audit 清单（来自 2026-05-05 审计）

P2 迁移目标——按严重度排序：

### §4.1 红线（P0 必决 = 设计裁决；代码修复全归 patch）

| 位置 | 现状 | 问题 |
|---|---|---|
| `combat/decay.rs:5` | `BASE_LOSS_PER_BLOCK: f32 = 0.06` | **与 worldview §四 正典 0.03/格直接冲突，翻倍** |
| `world/tsy_drain.rs:22-26` 非线性 vs `cultivation/dead_zone.rs:11-26` 线性 | 两套衰减公式互不相识 | 玩家在重叠区可能被扣两次 |
| `combat/woliu.rs:598-641` | 涡流按渡劫境界离散梯度（Induce 0.10 → Void 0.80），无 1/r² | 物理推导 vs 查表二选一 |
| `lingtian/qi_account.rs:3-7` 注释 | "ZoneQiAccount 与 world::zone::Zone.spirit_qi 暂未合账，等 WorldQiAccount 落地时再合账" | **本 plan P0 提供 WorldQiAccount snapshot 接口**；ZoneQiAccount facade 化是 patch P0-3 |
| 守恒律边界缺失 | `cultivation/tick.rs::compute_regen` ✅ 实装"吸收=zone-"；但**释放回 zone 路径多处缺** | 守恒方案在 §5 红线 4 收口（方案 A）；代码修复 = patch plan P2-7 释放路径补全 |

**守恒律违反清单（事实证据，patch plan P0-4/5/6 修复）**：
- 采集奖励 `player/gameplay.rs:294 GATHER_SPIRIT_QI_REWARD = 14.0` 凭空生灵气
- TSY 抽取 `cultivation/tsy_drain.rs:101` drain 去向无记账 → 灵气消失
- 域崩 `world/events.rs:1178` `zone.spirit_qi = 0.0` 暴力清零，可能 = 数百单位灵气消失

本 plan §5 红线 4 给出的"释放回 zone（方案 A）"是上述 3 项的物理依据；本 plan 不修代码，仅在 P1 提供 `qi_release_to_zone` / `assert_conservation` 算子让 patch plan P0-4/5/6 可调用。

### §4.2 待整合（P1-P2）

| 位置 | 常数 / 函数 | 处置 |
|---|---|---|
| `combat/decay.rs:26-44` | `ColorKind`/`CarrierGrade` 各自 +0.018~0.046/格 | 移入 `qi_distance_atten(medium)` 的 `MediumKind` 维度 |
| `combat/components.rs:23-26` | `JIEMAI_*` 多常数 | 截脉作为 `StyleDefense` impl 注册物理参数 |
| `combat/jiemai.rs:115-133` | 护甲减速 / 距离响应梯度 | 同上，归 `qi_collision` |
| `combat/woliu.rs:25-29` | `VORTEX_THEORETICAL_LIMIT_DELTA = 0.8` | 拆为 `QI_NEGATIVE_FIELD_K` + 涡流参数 |
| `shelflife/compute.rs:18` | `DEAD_ZONE_SHELFLIFE_MULTIPLIER = 3.0` | 移入 `EnvField` 死域因子 |
| `shelflife/registry.rs` 5 profile | 骨币年度线性 / 灵石天级指数 / 兽骨指数 / 陈酒混合 / 灵木 | 统一 `qi_excretion(ContainerKind)` 表达；**下限 clamp 到 `env.local_zone_qi` 而非 0**（压强法则） |
| `shelflife/compute.rs:171-189` | 3 公式（Linear/Exponential/Stepwise）**全部归零到 0** | 违反压强法则——必须在 P2 迁移时修正为"渐近 zone 浓度"；只在死域 / 坍缩渊 (zone_qi=0) 时真归零 |
| `cultivation/tick.rs:42-45` | `QI_REGEN_COEF = 0.01` / `QI_PER_ZONE_UNIT = 50.0` | regen 是反向逸散，归 `qi_physics::regen` |
| `lingtian/growth.rs:29` | `ZONE_LEAK_RATIO = 0.2`（20% 真元逸散） | 直接调 `qi_excretion(ContainerKind::AmbientField)` |
| `world/tsy_container.rs:179` | `SEARCH_DRAIN_MULTIPLIER = 1.5` | 接入 `EnvField.tsy_intensity` |
| `network/client_request_handler.rs:127-129` | 跨界磨损 0.01-0.05 | 归 `qi_physics::wear` |

### §4.3 缺口（P3 才补）

- 异体排斥 ρ：grep 全库无 `rejection`/`多系排斥` 物理实现，**baomai/multi-style 物理层未落地** ← P3 补
- 节律 cadence：grep 全库无 `cadence`/`rhythm`/`节律` 实装 ← 等 plan-lingtian-weather-v1
- **天道阈值收割触发器**：`tribulation.rs`(4981 行)实装了主动渡劫，但"区域灵气 < 阈值 → 天道引发灾劫收割"未实装 ← P3 补
- **天道时代衰减 1-3%**：worldview §十 正典但代码无定时器；ZoneQiAccount 也没"era 边界"概念 ← P3 补
- **坍缩渊塌缩时的灵气 redistribute**：`world/tsy_lifecycle.rs::TsyCollapseStarted` event + `extract_system::on_tsy_collapse_completed` 已挂事件链，但**塌缩时只触发 race-out 物理（参 §十六.一 step 4），未把坍缩渊暂存的真元按压强法则分发回周围 zone**（worldview line 789 "高效回收灵气" 正是这条）← P3 补；接入点已就绪，只缺 `collapse_redistribute_qi` 物理算子 + ledger 写入

---

## §5 开放问题 / 决策门（P0 启动前必须收口）

### 红线 1：距离衰减 0.03 vs 0.06

worldview 写 0.03/格（已正典），代码 `combat/decay.rs:5` 是 0.06。要先 `git log --follow combat/decay.rs` 看历史，决定：

- **A**：worldview 是后正典化的，代码 0.06 才是早期校准结果 → 改 worldview
- **B**：代码是漏看 worldview 的 bug → 改代码到 0.03 + 重新校准 ColorKind/CarrierGrade bonus
- **C**：worldview 0.03 是"基础物理"，0.06 是"末法时代叠加" → worldview 加注解，代码 const 拆 base + era_multiplier

**默认推荐 B**——worldview 是正典，代码偏离要被纠正；0.06 → 0.03 后 ColorKind bonus 重新校准。

### 红线 2：tsy_drain 非线性 × dead_zone 线性

`world/tsy_drain.rs` 用 `(qi_max/100)^1.5 × 0.5`，`cultivation/dead_zone.rs` 用 `1.0/分钟` 线性。两者描述的都是"区域真元被抽走"——同源现象。

- **A**：合并成一个 `qi_zone_drain(zone, env)`，TSY 是 EnvField 的高强度子集
- **B**：保留两套，明确"末法残土" vs "死域"是不同 EnvField，rate 公式同形不同参
- **C**：TSY 公式取代 dead_zone，dead_zone 退化为 `EnvField.tsy_intensity == 1.0` 的特例

**默认推荐 C** —— worldview §十六 把死域归在末法残土框架内。

### 红线 3：涡流离散梯度 vs 1/r² 物理

`woliu.rs:598-641` 把涡流参数按渡劫境界**列死表**（Induce 0.10 → Void 0.80），不是物理 1/r² 计算。

- **A**：保留离散表，`qi_physics` 只暴露"涡流强度系数"接口由 woliu plan 自己填表
- **B**：拆成"境界 → 涡流场强度"映射表 + `qi_physics` 的 `1/r²` 物理算子，二者乘积
- **C**：纯物理化，离散表退场，按"修士真元上限 → 涡流场强度"连续函数

**默认推荐 B** —— 境界离散感保留（玩家可感知"突破后涡流变强"），物理形态由底盘统一。

### 红线 4：守恒律边界（释放路径 + 压强下限）

worldview 正典 "总量恒定且缓慢衰减；修炼消耗 = 别人少掉" + §二/§三 压强法则。`cultivation/tick.rs::compute_regen` 已实装"吸收=zone-"零和路径，但**释放路径多处单向扣减**（招式/容器衰变/死亡真元都没归还到 zone）。

**已收口**：方案 A —— 释放即归还到 zone（worldview "压强法则"明确"流动从高到低"，没给"系统外暂存"留空间，B/C 方案与 worldview 冲突，废弃）。

具体落地（待 P0 设计阶段细化）：

- 玩家死亡 → cultivation.qi_current 全部加到当地 `zone.spirit_qi`（一次性释放）
- 招式释放 → 真元注入到目标方向的 zone（按距离衰减分布到沿途 / 命中 zone）
- 容器衰变 → 渐近释放到容器所在 zone，**下限 `zone.spirit_qi`**（容器内 ≥ zone 时停）
- 修士死亡 + 装备掉落（worldview §十六.三 "满灵骨币变普通骨头"）→ 自动走压强法则：死域 zone_qi=0 时容器内被抽空到 0，正常 zone 时降到 zone 浓度
- **坍缩渊吸入的真元不消失** —— 暂存于活坍缩渊内部账本，塌缩瞬间 `collapse_redistribute_qi(rift, surrounding_zones)` 按压强法则分发回周围 zone（low-pressure 方向多分），守恒等式 `Σ ≡ WorldQiBudget.current_total`（resource 引用，不字面化）在跨坍缩渊事件链上仍闭合

**唯一允许的系统外流出**：天道每时代衰减 1-3%（worldview §十）—— 这是天道一台"冷漠的平衡机器"对总量的硬干预，不是 plan 自由度。

留给 P0 决策的：

- **招式释放的 zone 写入：即时 vs 分摊** —— **默认推荐即时** `+=` 在 collision system 内（系统执行顺序保证 attacker 写完前 zone 不被读）。分摊会让守恒断言难写（每 tick 末要 reconcile 所有 pending transfers，引入额外状态）。除非有具体场景需要"招式真元逐 tick 沉降"（worldview 没明示），不要给自己加复杂度
- **多人同 zone 同时死亡的并发**：用 `+=` 操作数学上顺序无关；ECS 调度若给 zone resource 加锁则串行无忧。**默认推荐**直接 `+=`，不引入排序机制

### 其他未决

- [ ] 容器型衰减（5 套 shelflife profile）能否用统一 `excretion(container, elapsed, env)` 表达？指数/线性/Stepwise/Age-Spoil 是否都是同一公式不同参？— P1 设计时落
- [x] **regen 归属**：已收口 → 并入 `excretion.rs`（regen = 反向 excretion，方向相反但同源）。`cultivation/tick.rs::compute_regen` 作参考实装移植为 `excretion.rs::regen_from_zone(zone_qi, rate, integrity, room) → (gain, drain)`，**不独立 regen.rs 模块**
- [ ] EnvField 数据源：节律来自 lingtian-weather-v1（⏳）；本 plan 先做 mock 接口
- [ ] 异体排斥 ρ 实装责任：是 plan-multi-style-v1 / plan-baomai-v2 之一负责，还是本 plan P3 补？
- [ ] **`QI_DENSITY_GAZE_THRESHOLD` / `QI_REGION_STARVATION_THRESHOLD` 数值**：worldview §十一 只说"超阈值天道注视" 没给数字，需要 P0 估算 → P3 校准
- [x] **`SPIRIT_QI_TOTAL` 是否要从 const 改为 Resource / config**：已裁决 **B**。改成 `Resource<WorldQiBudget>` 由 server config 初始化；默认值 100.0 只作兜底 / 本地 fixture，测试可注入 50，生产可按服规模配置。agent 端不持有静态 const，patch P3 通过 IPC 同步 budget snapshot。P0 定义类型与 config loader，P1 所有守恒断言只读该 resource。

---

## §6 进度日志

- **2026-05-05 (下午-1)**：骨架创建。基于 2026-05-05 真元物理分散度审计（详见 §4）。同步 `docs/CLAUDE.md §二 接入面 / §四 红旗` 各加一条，约束新 plan 不再自己拍真元常数。前置触发：plan-economy-v1 §1.5 衰变曲线裁决无解 → 上钻发现是底盘缺位
- **2026-05-05 (下午-2)**：补"守恒律"作为 §0 第一公理（worldview §二/§十）。核现有代码：`SPIRIT_QI_TOTAL` 已是 server+agent 双端 const（当前值 100.0，**标注为暂定可配置 placeholder**——骨架不传播字面 100）；`cultivation/tick.rs::compute_regen` 已实装"吸收=zone-"零和路径；`lingtian/qi_account.rs` 注释明确等 `WorldQiAccount` 合账。新增 §4.1 红线 4（合账缺位）+ §4.3 缺口（天道阈值收割 + 时代衰减）+ §5 红线 4（释放路径守恒边界）。`qi_physics::ledger` + `qi_physics::tiandao` 模块进 §3 数据契约
- **2026-05-05 (下午-3)**：补"压强法则"作为 §0 第二公理（worldview §二 line 20 / §三 line 32）。逸散下限 = `env.local_zone_qi` 而非 0；shelflife 5 profile 现行"归零到 0"违反压强法则，P2 迁移修复。**红线 4 收口** —— 方案 A 落地，B/C 与 worldview 冲突废弃；只在死域/坍缩渊（zone_qi=0）时容器才真归零，正好对应 worldview §十六.三「满灵骨币变普通骨头」物理来源。储藏地点变战略选择（聚灵阵保值 vs 末法残土真贬值），与 §十一 灵物密度阈值天道注视形成张力
- **2026-05-05 (下午-4)**：补"坍缩渊是质量中转站不是终点"为 §0 第三条机制（worldview §十一 line 789 "高效回收灵气"）。修正之前误读——活坍缩渊吸进的真元暂存内部账本，塌缩时 `collapse_redistribute_qi` 按压强法则分发回周围 zone（low-pressure 方向多分），死坍缩渊 zone_qi=0 是分发完毕状态。`tiandao.rs` 加 `collapse_redistribute_qi` 函数，挂在 `world/tsy_lifecycle.rs::TsyCollapseStarted` event 后端（接入点已就绪，缺物理算子 + ledger 写入）。坍缩渊由此是天道的"灵气拖把"——把局部聚集擦回稀薄区
- **2026-05-05 (下午-5)**：拆分——本 plan **只做 P0 + P1**（决策 + 立底盘 + 算子 + 账本），P2 + P3（迁移工作 + 新机制接入）整体迁出到新立的 `plan-qi-physics-patch-v1`。两 plan 并行不撞 PR，本 plan 不动 server 既有代码，patch plan 不动 qi_physics 内部 API
- **2026-05-05 (下午-6)**：自审 + 修订。澄清 P0/P1 范围 = "phy 核心做到位"（不只是文档）：P0 = 决策 + 脚手架 + constants/config + 类型骨架；P1 = 5 算子完整物理实装 + ledger snapshot + 40+ 测试。**仍不动既有代码**——衔接 / 迁移属 patch。修订内容：① §1 表 P0/P1 验收边界明确 ② §1.5 测试矩阵拆 7 组（excretion/distance/collision/channeling/release/ledger/snapshot 各 5-8 测）③ 5 个 `??` 常数改 v0.1 placeholder（不阻塞 P0）④ §3 模块结构补齐：ledger 是只读快照接口、`QiTransfer` 是事件流、excretion 被动 vs release 主动边界 + collision 不递归调其他算子 ⑤ 跨仓库契约边界明确（IPC 全归 patch P3，本 plan 留 `snapshot_for_ipc()` 占位）⑥ §4.1 加守恒律违反清单 cross-ref patch plan P0-4/5/6 ⑦ §5 红线 4 末段补"招式释放即时 vs 分摊" + "多人并发 +=" 决策推荐
- **2026-05-05 (下午-7)**：P0 决策门追加裁决：`SPIRIT_QI_TOTAL` 走 config，不保留静态总量 const 作为守恒源头。P0 建 `WorldQiBudget` Resource + config loader；P1 守恒断言统一读 `WorldQiBudget.current_total` / `initial_total`，agent 同步留给 patch P3。

## Finish Evidence

### 落地清单

- **P0**：`server/src/qi_physics/mod.rs` 注册 `WorldQiBudget` / `WorldQiAccount` / `QiTransfer`，并由 `server/src/main.rs` 接入；`constants.rs` 落地 worldview 锚定常量；`env.rs` / `traits.rs` 落地 `ContainerKind`、`MediumKind`、`EnvField`、`StyleAttack`、`StyleDefense`、`Container`。
- **P1**：`distance.rs`、`excretion.rs`、`collision.rs`、`channeling.rs`、`release.rs`、`ledger.rs`、`tiandao.rs` 落地距离衰减、压强逸散、碰撞、导引、释放、账本 snapshot、守恒断言、坍缩渊再分配与时代衰减算法。

### 关键 commit

- `a190e98b`（2026-05-05）`feat(qi-physics): 建立真元物理底盘`

### 测试结果

- `cd server && cargo fmt --check` ✅
- `cd server && cargo clippy --all-targets -- -D warnings` ✅
- `cd server && cargo test qi_physics` ✅ 53 passed
- `cd server && cargo test` ✅ 2374 passed
- `grep -R -c -E '#\[test\]' server/src/qi_physics/*.rs` 合计 53，满足 §1.5 ≥ 40 单测下限

### 跨仓库核验

- **server**：`qi_physics::constants`、`WorldQiBudget`、`WorldQiAccount`、`QiTransfer`、`summarize_world_qi`、`qi_excretion`、`qi_distance_atten`、`qi_collision`、`qi_channeling`、`qi_release_to_zone`、`collapse_redistribute_qi` 均已落地。
- **agent / client**：本 plan 不引入跨仓库 schema；`snapshot_for_ipc()` 仅作为 server 端占位接口，agent schema、IPC 同步、client tooltip 由 `plan-qi-physics-patch-v1` 的 P3 承接。

### 遗留 / 后续

- 既有 `combat` / `cultivation` / `shelflife` / `lingtian` / TSY 常数迁移、`SPIRIT_QI_TOTAL` 双端 const 下线、IPC budget snapshot、client tooltip、新机制接入与数值校准全部保留给 `plan-qi-physics-patch-v1`；本 plan 按范围只建立底盘 API 和饱和测试。
