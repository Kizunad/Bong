# Bong · plan-style-balance-v1

修复 `qi_collision` 公式缺陷 + 引入 `rejection_rate` trait 让流派克制从物理参数**涌现**,不查表。然后建立验证框架（模拟 + telemetry）确保涌现结果符合 worldview 预期。

**设计原则**：不加 4×3 const 矩阵硬编克制关系。所有攻防优劣从 purity / rejection_rate / resistance / distance / medium 等物理属性自然算出。style-balance 是**验证层**而非 override 层。

**世界观锚点**：`worldview.md §四.战斗系统(异体排斥/距离衰减)` · `§五 七流派` · `§二 真元极易挥发`

**library 锚点**：`cultivation-0002 烬灰子内观笔记`(缚/噬/音/影 四论)

**交叉引用**：`plan-qi-physics-v1` ✅ · `plan-qi-physics-patch-v1` ✅ · 7 流派 v1 ✅ + v2 各自 plan · `plan-gameplay-journey-v1` §P/O.9

---

## 当前代码实地核验（2026-05-11）

- **前置已满足**：`plan-qi-physics-v1` / `plan-qi-physics-patch-v1` 已归档；当前 `server/src/qi_physics` 已有 `StyleAttack` / `StyleDefense` / `qi_collision()` 稳定接入点。
- **公式缺口仍在**：`server/src/qi_physics/traits.rs` 的 `StyleAttack` 只有 `style_color()` / `injected_qi()` / `purity()` / `medium()`，尚无 `rejection_rate()`；`server/src/qi_physics/collision.rs` 仍用 `1.0 - purity + resistance * 0.5` 计算 rejection，并用 `effective_hit * (1.0 - resistance)` 造成 `resistance == 1.0` 时 `defender_lost == 0.0`。
- **测试证据显示缺口真实存在**：`server/src/qi_physics/collision.rs` 仍有高 resistance 断言 `assert_eq!(outcome.defender_lost, 0.0)`，这正是本 plan 要替换的旧行为。
- **模拟器已是脚手架，不是落地**：`scripts/balance/style_collision_sim.py` / `.html` 已能对比 CURRENT、FIX_A、FIX_B，并标注 FIX_A（`resistance` cap 0.95 + `rejection_rate`）为当前设计选择；消费本 plan 时要把模拟器公式同步到 Rust，而不是只更新 HTML。
- **telemetry 只到颜色快照**：`server/src/combat/style_telemetry.rs`、`server/src/schema/style_balance.rs`、`agent/packages/schema/src/style-balance.ts` 已有 `attacker_color` / `defender_color` / `cause` / `resolved_at_tick`，但还没有 `attacker_style` / `defender_style` / ρ 等用于平衡聚合的字段。

**结论**：可升 active。当前代码已经有足够接入面和验证脚手架；本 plan 的实现重点是替换旧公式、补 trait/流派实现、把模拟器与 schema 回归锁住。

---

## 接入面 Checklist

- **进料**：7 流派各自的 `impl StyleAttack` / `impl StyleDefense`（purity / resistance / drain_affinity / medium 等现有参数）
- **出料**：修正后的 `qi_collision` 公式 + 新增 `rejection_rate()` trait 方法 + 饱和组合验证测试 + PVP telemetry 日志
- **共享类型**：`StyleAttack` trait 扩展 `rejection_rate()` / telemetry schema 加 `attacker_style` / `defender_style` / 可聚合物理参数字段
- **跨仓库契约**：`combat::resolve` ✅ + `qi_physics::collision` 公式修正 + `combat::style_telemetry` 增强
- **worldview 锚点**：§四 + §五 + cultivation-0002 四论
- **qi_physics 锚点**：`qi_collision()` 公式修正是本 plan 核心;不新增物理常数,修正现有公式使 ρ/resistance 行为符合 worldview

---

## §0 设计轴心：涌现而非查表

当前 `qi_collision` 有两个公式缺陷,导致涌现结果偏离 worldview 预期:

### 缺陷 1：resistance ≥ 1.0 = 无敌

```rust
// 现状 (collision.rs)
let defender_lost = effective_hit * (1.0 - resistance);
// resistance = 1.0 → defender_lost = 0,完全免疫所有攻击
```

**受影响**：截脉·通灵（realm_factor = 1.0）、替尸·朽木甲（contam_cap/30 = 1.0）— 全行 defender_lost = 0。

**修正**：resistance 加递减收益,永远留穿透缝隙:

```rust
// 修正
let diminished_r = 1.0 - (1.0 - resistance.clamp(0.0, 1.0)).powf(0.6);
let defender_lost = effective_hit * (1.0 - diminished_r);
// resistance=1.0 → diminished_r≈1.0 但 powf(0.6) 让高值压缩
// 实际效果：resistance=1.0 仍有 ~0% 穿透——需要调 exponent
```

**最终公式**（exponent 待模拟确认,初始 0.5）:

```rust
// 方案 A (已选定 2026-05-10)
let r_eff = resistance.clamp(0.0, 0.95);  // hard cap 95%,永远留 5% 穿透缝隙
let defender_lost = effective_hit * (1.0 - r_eff);
```

**决策(2026-05-10)：选方案 A。模拟验证方案 B 防御层次感塌了(截脉通灵只挡 19%),方案 A 保留层次(挡 96%)且无无敌盾。**

### 缺陷 2：purity 承担了 ρ（异体排斥）的职责

worldview §四/§五 的 ρ 是**攻方真元被防方免疫系统排斥的容易程度**:
- 体修 ρ=0.65：真元浑厚密度大,容易被认出是外来的,排斥高
- 暗器 ρ=0.45：载体投射,中等排斥
- 阵法 ρ=0.35：阵法灵气弥散,较低排斥
- 毒蛊 ρ=0.05：脏真元伪装成宿主真元,几乎无排斥

当前代码里 `purity` 同时承担声学激发阈值 + rejection 公式的 `(1-purity)` 项,语义混淆。

**修正**：在 `StyleAttack` trait 加 `rejection_rate()`:

```rust
// qi_physics/traits.rs
pub trait StyleAttack {
    fn style_color(&self) -> ColorKind;
    fn injected_qi(&self) -> f64;
    fn purity(&self) -> f64 { 1.0 }           // 声学纯度,仅用于 threshold check
    fn rejection_rate(&self) -> f64 { 0.30 }   // ρ 异体排斥率,worldview §四
    fn medium(&self) -> MediumKind { MediumKind::bare(self.style_color()) }
}
```

**rejection 公式改为**:

```rust
// 现状
let rejection = attenuated * QI_EXCRETION_BASE * (1.0 - purity + resistance * 0.5);

// 修正：purity 退出 rejection,由 rejection_rate 接管
let rejection = attenuated * QI_EXCRETION_BASE * (rejection_rate + resistance * 0.5);
```

各流派 impl 补上 `rejection_rate()`:

| 流派 | rejection_rate | 物理依据 |
|------|---------------|---------|
| 体修·崩拳 | 0.65 | §五:399 真元浑厚密度大,异体排斥最高 |
| 暗器 | 0.45 | §五 载体投射,中等排斥 |
| 阵法 | 0.35 | §五 弥散灵气,较低排斥 |
| 涡流 | 0.30 | §五 涡旋场非直接注入,排斥与裸真元持平 |
| 毒蛊 | 0.05 | §五:425 脏真元伪装,几乎无排斥 |

### 不修的：阵法高投 > 体修单发

这符合 worldview。体修优势是**不依赖外物**（破产狂战士）,不是单发伤害最高。阵法高投需要灵器载体 + 布阵时间 + 高真元投入,体修只需要一拳。经济成本和时间窗口差异在 qi_collision 层看不到,是 gameplay 层的事。

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | 修复 qi_collision: resistance cap + rejection_rate trait + 各流派 impl 补 ρ 值 | `cargo test` 全绿 + 模拟器验证涌现结果无"无敌盾" + 毒蛊低排斥符合预期 |
| **P1** ⬜ | 饱和攻防验证测试（主矩阵 + 距离/载体/防御边界）+ 模拟器 HTML 对比报告 | 组合测试全绿,无 0.00 行（除声学阈值 fail）,克制方向符合 worldview |
| **P2** ⬜ | PVP telemetry 增强：加 attacker_style / defender_style / ρ 观测字段 | Rust schema / TS schema / Redis 推送对齐，事件仍兼容颜色快照聚合 |
| **P3** ⬜ | telemetry 聚合 + 校准脚本：偏差 >30% 的调底层物理参数（rejection_rate / resistance 系数）,不改公式结构 | 离线 replay + 小样本实战报告能定位偏差；大规模真实对战样本作为后续运营校准输入 |

---

## §2 数据契约

- [ ] `server/src/qi_physics/traits.rs` — `StyleAttack` trait 加 `fn rejection_rate(&self) -> f64 { 0.30 }`
- [ ] `server/src/qi_physics/collision.rs` — rejection 公式用 `rejection_rate` 替换 `(1-purity)`;defender_lost 的 resistance 加 hard cap 或去除二次减免
- [ ] `server/src/cultivation/burst_meridian.rs` — `BengQuanStyleAttack` 加 `rejection_rate() -> 0.65`
- [ ] `server/src/combat/projectile.rs` — `AnqiStyleAttack` 加 `rejection_rate() -> 0.45`
- [ ] `server/src/cultivation/dugu.rs` — `PendingDuguInfusion` 加 `rejection_rate() -> 0.05`
- [ ] `server/src/combat/woliu.rs` — `VortexField` 加 `rejection_rate() -> 0.30`
- [ ] `server/src/zhenfa/mod.rs` — `ZhenfaInstance` 加 `rejection_rate() -> 0.35`
- [ ] `server/src/combat/style_telemetry.rs` — 加 attacker_style / defender_style 字段
- [ ] `scripts/balance/style_collision_sim.py` — 模拟器同步更新公式 + 对比报告

---

## §3 验证矩阵

```
4 攻（体修/暗器/阵法/毒蛊）× 3 防（截脉/替尸/涡流）= 12 对主组合
4 攻互克 = 12 对（各攻击方 vs 无防御下的相对效率排序）
3 防互克 = 6 对（各防御方 vs 标准攻击下的减免率排序）
= 30 对
```

**验证标准**（方向而非数值）:
- 毒蛊 vs 任何防御的 rejection 比率 < 其他攻击方（ρ=0.05 渗透最强）
- 体修 vs 任何防御的 rejection 比率 > 其他攻击方（ρ=0.65 排斥最高,但 qi 量大补偿）
- 截脉·通灵 defender_lost > 0 对所有攻击（不再无敌）
- 替尸·朽木甲 defender_lost > 0 对所有攻击（不再无敌）
- 涡流 defender_absorbed > 其他防御（drain_affinity 高）
- 距离 0→20 衰减曲线：体修衰减 > 暗器骨针衰减（BareQi vs SpiritWeapon 载体）

---

## §4 开放问题

- [x] resistance 修正选方案 A（hard cap 0.95）— 2026-05-10 模拟对比后定,方案 B 防御层次感塌了
- [ ] 涡流既是攻击又是防御（`StyleAttack` + 负场 drain）,rejection_rate 0.30 是否合理?
- [ ] 各流派 v2 上线后 rejection_rate 是否需要按招式细分(同流派不同招不同 ρ)?

## §5 进度日志

- 2026-05-01：骨架创建。plan-gameplay-journey-v1 §P / O.9 派生。
- 2026-05-10：重写方向——从 const 矩阵查表改为涌现验证框架。发现 resistance≥1.0 无敌 bug + purity/ρ 语义混淆。`scripts/balance/style_collision_sim.py` 模拟器 v2 三方案对比,选定方案 A（cap 0.95 + rejection_rate ρ）。
- 2026-05-11：实地核验当前 Rust / TS / simulator 状态，确认前置已满足且旧公式缺口仍存在；升 active，后续可直接消费实现。
