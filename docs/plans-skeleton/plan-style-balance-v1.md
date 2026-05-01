# Bong · plan-style-balance-v1 · 骨架

流派克制系数 config 表 + telemetry 回填机制。承接 plan-gameplay-journey-v1 §P 流派克制理论推导,把 §P.1 ρ 表 / §P.3 W 表 / β 系数实装为 config,并提供 PVP telemetry 回喂修订机制。

**世界观锚点**：`worldview.md §四.战斗系统(异体排斥/距离衰减)` · `§五 七流派`

**library 锚点**：`cultivation-0002 烬灰子内观笔记`(缚/噬/音/影 四论)

**交叉引用**：7 流派 plan(`baomai-v1` ✅ + `anqi/dugu/zhenmai/tuike/woliu/zhenfa-v1` ⬜) · `plan-cross-system-patch-v1` ⏳(telemetry 增量) · `plan-gameplay-journey-v1` §P/O.9

---

## 接入面 Checklist

- **进料**：7 流派 P0 完成后的攻击 entity + 防御机制
- **出料**：server 战斗结算阶段查询的克制系数 + 30 对组合的单元测试 + telemetry 日志聚合
- **共享类型**：`StyleTag`(攻击 entity 标记) + `StyleMatrix` config 结构
- **跨仓库契约**：`combat::resolve` ✅ + 新增 `style_matrix.rs` + cross-system-patch 增 PVP 日志
- **worldview 锚点**：§四 + §五 + cultivation-0002 四论

---

## §0 设计轴心

- [ ] **方向不可变**：§P.1-P.4 物理模型(ρ表/W表/β)不允许任何 plan 单方面改
- [ ] **数值待校准**：§P.5 矩阵单元值是初始估算,telemetry 校准必修
- [ ] **校准流程**：每个流派 plan 升 active 时强制读 §P,不允许"我觉得这数应该改"
- [ ] **物理下限**：所有 DPS ≥ 1.0(基线),所有伤害 ∈ [15, 80]

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | StyleMatrix const 表 + combat::resolve 接入 + 30 对单测 | 30 对组合测试结果落在 §P.5 ±20% 区间 |
| **P1** ⬜ | PVP telemetry 日志(attacker_style, defender_style, distance, h_eff_actual, qi_drain_actual) | cross-system-patch 增 schema |
| **P2** ⬜ | telemetry 聚合脚本 + Grafana dashboard | 跑 200+ 真实对战后产数据 |
| **P3** ⬜ | 校准修订: 差距 > 30% → 修 ρ/W 表 + 同步更新 §P 矩阵 | 修订后 30 对再跑测试 |

---

## §2 数据契约

- [ ] `server/src/combat/style_matrix.rs` const 系数表
- [ ] `server/src/combat/style_resolve.rs` 应用克制系数到 `combat::resolve`
- [ ] `server/src/combat/style_telemetry.rs` PVP 日志埋点
- [ ] `agent/packages/schema/src/style-balance.ts` telemetry payload schema
- [ ] `scripts/balance/style_aggregate.py` 聚合分析脚本(从 redis/db 拉日志)

---

## §3 关键参数(从 plan-gameplay-journey-v1 §P.1-P.3 抽出)

```rust
// ρ 表(异体排斥系数, P.1 定律 1)
const RHO: [(StyleId, f32); 4] = [
    (Baomai, 0.65),
    (Anqi,   0.45),
    (Zhenfa, 0.35),
    (Dugu,   0.15),
];

// β 表(防御维持成本, P.2)
const BETA: [(DefenseId, f32); 3] = [
    (Jiemai, 0.6),
    (Tuike,  1.5),
    (Woliu,  2.0),
];

// W 矩阵(防御 vs 攻击类型衰减率, P.3)
const W_MATRIX: [[f32; 4]; 3] = [
    // [vs Baomai, Anqi, Zhenfa, Dugu]
    [0.5, 0.7, 0.2, 0.0],  // Jiemai
    [0.3, 0.4, 0.5, 0.7],  // Tuike
    [0.8, 0.85, 0.2, 0.4], // Woliu
];

const K_DRAIN: f32 = 1.0;  // 涡流吸取系数, clamp 反向获利 ≤ 0.5
const ALPHA: f32 = 0.3;    // 异体侵入消耗系数
const B_IDLE: f32 = 1.0;   // 防方真元基线
```

---

## §4 30 对组合单元测试目标值

来自 plan-gameplay-journey-v1 §P.5 校准后矩阵:

```
4 攻 × 3 防 (12 对): 体修/器修/地师/毒蛊 × 截脉/替尸/涡流
4 攻互克 (12 对): 4×4 - 4 自身对决
3 防互克 ( 6 对): 3×3 - 3 自身对决
                      = 30 对
```

允许误差: ±20%(数值) / 0%(克制方向)

---

## §5 开放问题

- [ ] PVP telemetry 数据隐私(玩家是否同意收集对战数据)?
- [ ] telemetry 阈值: 200 对战是否够? 1000 对战 + 95% 置信区间?
- [ ] 校准修订是热更新还是版本发布? 对正在战斗的玩家影响?
- [ ] 4 攻互克 + 3 防互克(总 18 对额外组合)是否同样需要 telemetry?

## §6 进度日志

- 2026-05-01：骨架创建。plan-gameplay-journey-v1 §P / O.9 派生。理论 baseline 已就位,等 7 流派 plan 升 active 后启动。
