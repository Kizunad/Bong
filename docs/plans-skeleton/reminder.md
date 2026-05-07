# Skeleton 未决事项登记

本目录下 plan 在展开/落地过程中累积的**延后处理**事项。每条带 plan 锚点 + 上下文，后续回来解决时直接对号入座。

---

## plan-tribulation-v1

- [ ] **半步化虚 buff 强度**：当前 +10% 真元上限 / +200 年寿元是占位（已在 plan-tribulation-v1 §8 line 277 标注"延后决定"）。Phase 1-3 已上线，可观察"卡在半步化虚"的玩家比例后调整；名额空出时可重渡的升级机制也待确认（已在 plan-tribulation-v1 §8 标注）。**无需新 plan，直接在 tribulation-v1 §8 补决定**

---

## 通用机制备忘：v2 流派 熟练度生长二维划分

适用所有 v2 流派 plan 的 P0 决策门。首发 plan-zhenmai-v2。

- **境界 = 威力上限**（K_drain / 反震点数 / 紊流半径 / 自蕴乘数等"大小"维度）
- **熟练度 = 响应速率**（冷却 cooldown / 弹反窗口 window / cast time / cast 充能时间）
- 公式：`cooldown(lv) = base + (min - base) × clamp(lv/100, 0, 1)`，线性递减
- 哲学：worldview §五:537 流派由组合涌现 + §五:506 末土后招原则物理化身——醒灵苦修 lv 100 弹反窗口 250ms / 化虚老怪 lv 0 仅 100ms（练得多胜过境界高）
- **各 plan P0 需决定**：(a) 公式 vs plan-skill-v1 lv 映射区间；(b) 各自招式 base/min 值；(c) 是否派生 plan-skill-proficiency-v1 通用 plan 提取 cooldown/window curve helper
- **应回填 v2 plan**（各自 P0 决策门统一处理）：plan-woliu-v2（瞬涡 5s / 涡口 8s / 涡引 30s 改为按熟练度）/ plan-dugu-v2（蚀针 3s / 侵染 8s 改）/ plan-tuike-v2（蜕一层 8s / 转移污染 30s 改）— 早立骨架时未含熟练度机制，需 P0 补入

---

## 依赖链关键路径（仍 active）

```
plan-qi-physics-v1 P0 红线决议 → P1 算子 ship
  → plan-qi-physics-patch-v1 P0/P1/P2/P3 逐 PR 迁移
    → plan-economy-v1 / plan-style-balance-v1 / 其他 ~9 个 plan 解阻
```

---

> **约定**：每解决一条就从这里删。新增延后事项请直接追加到对应 plan 段，保持扁平。
>
> **2026-05-07 清理记录**：
> - `plan-tribulation-v1` Drowsy / `plan-npc-virtualize-v1` Drowsy 中间态 → 已拆为独立骨架 `plan-npc-virtualize-v2`
> - `plan-npc-virtualize-v3` 占位 → 已拆为独立骨架
> - `plan-yidao-v2` 占位 → 已拆为独立骨架
> - 2026-04-27 / 2026-05-01 / 2026-05-05 / 2026-05-06 各批次"已转为独立骨架"记录 → 核实均存在（finished_plans 或 plans-skeleton），已清除
