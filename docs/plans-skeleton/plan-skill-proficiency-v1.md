# Bong · plan-skill-proficiency-v1 · 骨架

通用**熟练度生长算子** —— 把 v2 流派 plan 各自重复写的 cooldown / window 线性递减公式提取为 `server/src/skill/proficiency.rs` 共享模块。源自 plan-zhenmai-v2 首发的「熟练度生长二维划分」机制（2026-05-06），并在 8 个 v2 流派 plan 内复用。

**核心公式**：
```
cooldown(lv) = base + (min - base) × clamp(lv/100, 0, 1)   （线性递减，lv 越高冷却越短）
window(lv)   = base + (max - base) × clamp(lv/100, 0, 1)   （线性递增，lv 越高窗口越长）
```

**设计哲学**：
- **境界 = 威力上限**（K_drain / 伤害 / 半径 / 效果数量等「大小」维度，各流派 plan 自定）
- **熟练度 = 响应速率**（cooldown / 弹反窗口 / cast time / 充能时间，本 plan 统一提取）
- worldview §五:506 末土后招原则物理化身：醒灵苦修 lv 100 弹反窗口 250 ms / 化虚老怪 lv 0 仅 100 ms（**练得多胜过境界高**）

**世界观锚点**：`worldview.md §五:537`（流派由组合涌现）· `§五:506`（末土后招原则：熟练度跑赢境界）

**交叉引用**：`plan-skill-v1.md` ✅（SkillSet.skill_lv + SkillXpGain event，本 plan 基础）

已含本机制的 v2 plan（需 P1 回填替换硬编常数）：
- `plan-zhenmai-v2` ✅（首发，cooldown 30→5s / 弹反窗口 100→250ms）
- `plan-baomai-v3` ✅ · `plan-anqi-v2` ✅ · `plan-zhenfa-v2` ✅ · `plan-yidao-v1` ✅

待 P2 回填（早立未含熟练度机制）：
- `plan-woliu-v2` ✅（瞬涡 5s / 涡口 8s / 涡引 30s 改 lv 线性）
- `plan-dugu-v2` ✅（蚀针 3s / 侵染 8s 改）
- `plan-tuike-v2` ✅（蜕一层 8s / 转移污染 30s 改）

---

## 接入面 Checklist

- **进料**：`skill::SkillSet.skill_lv`（plan-skill-v1 §P2 已实装）→ 传入 `proficiency_cooldown(growth, lv)` 计算实际冷却时长
- **出料**：`skill::proficiency::ProficiencyGrowth` struct + `proficiency_cooldown` / `proficiency_window` helper；各流派 cast 系统调用替换硬编常数
- **共享类型**：`ProficiencyGrowth { base_ms: u32, min_ms: u32 }` 注册在 `SkillRegistry` 侧（不改 IPC schema / SkillEntry 结构）
- **跨仓库契约**：无新 IPC schema；client cooldown 显示已有 `SkillEntry.cooldown_remaining`，server 端用本 helper 算实际值后传入该字段
- **worldview 锚点**：§五:537 + §五:506
- **qi_physics 锚点**：无（cooldown / window 是时间参数，不涉及灵气守恒）

---

## §0 核心接口

```rust
// server/src/skill/proficiency.rs

/// 注册在 SkillRegistry 的生长参数
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ProficiencyGrowth {
    pub base_ms: u32,   // lv=0 时的值（最慢/最短窗口）
    pub min_ms: u32,    // lv=100 时的值（最快/最长窗口）
}

/// cooldown: lv 越高越短（base > min）
/// window:   lv 越高越长（base < min，此时函数名 proficiency_window 语义更清晰）
pub fn proficiency_cooldown(growth: ProficiencyGrowth, skill_lv: u8) -> Duration {
    let t = (skill_lv as f32 / 100.0).clamp(0.0, 1.0);
    let ms = growth.base_ms as f32 + (growth.min_ms as f32 - growth.base_ms as f32) * t;
    Duration::from_millis(ms as u64)
}

pub fn proficiency_window(growth: ProficiencyGrowth, skill_lv: u8) -> Duration {
    proficiency_cooldown(growth, skill_lv)  // 同公式，base/min 含义由注册方决定
}
```

**招式注册示例**（每个 v2 流派 plan 在 P0 决策门定 base/min 值）：

```rust
// plan-zhenmai-v2: 弹反
SkillRegistry::register(SkillId::BounceBack)
    .with_proficiency_cooldown(ProficiencyGrowth { base_ms: 30_000, min_ms: 5_000 })
    .with_proficiency_window(ProficiencyGrowth  { base_ms: 100,     min_ms: 250   })
    ...
```

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | `proficiency.rs` 模块 + `ProficiencyGrowth` struct + `proficiency_cooldown` / `proficiency_window` + 单元测试（lv=0/50/100 边界 + base=min 退化） | `cargo test skill::proficiency::*` 全绿 |
| **P1** ⬜ | plan-zhenmai-v2 / plan-baomai-v3 / plan-anqi-v2 / plan-zhenfa-v2 / plan-yidao-v1 的硬编 cooldown 常数替换为本 helper | 5 plan 对应测试全绿，cooldown 数值行为与改前一致（lv=0 对齐原常数） |
| **P2** ⬜ | plan-woliu-v2 / plan-dugu-v2 / plan-tuike-v2 回填（早立未含，各自 P0 决策门定 base/min 值） | 回填 PR 绿 |

---

## §2 数据契约

- [ ] `server/src/skill/proficiency.rs` — 新建模块，含 `ProficiencyGrowth` + `proficiency_cooldown` + `proficiency_window` + 饱和化单元测试
- [ ] `server/src/skill/mod.rs` — `pub mod proficiency;`
- [ ] `server/src/skill/registry.rs`（若已存在）— `SkillRegistryEntry` 加 `cooldown_growth: Option<ProficiencyGrowth>` + `window_growth: Option<ProficiencyGrowth>`
- [ ] 5 个 v2 流派 cast 系统（P1）— 替换硬编 cooldown 为 `proficiency_cooldown(entry.cooldown_growth?, skill_lv)`
- [ ] 3 个早立 v2 流派（P2）— 同上回填

---

## §3 开放问题

- [ ] **`SkillRegistry` 是否已存在**：plan-skill-v1 提到 SkillEntry 存储 skill_lv，但 registry-based 注册 API 是否实装？P0 前先 `grep -rn "SkillRegistry" server/src/` 确认
- [ ] **v2 plan base/min 值确认**：各自 P0 决策门定（woliu-v2 涡口 8s 改用 lv 线性 / dugu-v2 蚀针 3s / tuike-v2 蜕一层 8s），P2 回填时需同步更新各 plan P0 决策记录
- [ ] **窗口语义防混淆**：`proficiency_window` 复用同一公式但方向相反（base 小 min 大），是否应分两个函数 `proficiency_cooldown_dec` / `proficiency_window_inc` 防止传参方向错误？
- [ ] **非线性曲线需求**：当前线性公式是否足够？若某流派想要开始慢后来快（S 曲线）→ 扩展为 `CurveKind { Linear, SCurve { midpoint } }`，P0 决定是否需要

## §4 进度日志

- 2026-05-10：骨架创建。源自 reminder.md「v2 流派 plan 通用机制：熟练度生长二维划分」章节（2026-05-06 zhenmai-v2 首发）提取为独立 plan。
