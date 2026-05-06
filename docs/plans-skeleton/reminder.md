# Skeleton 未决事项登记

本目录下 plan 在展开/落地过程中累积的**延后处理**事项。每条带 plan 锚点 + 上下文，后续回来解决时直接对号入座。

---

## plan-tribulation-v1

- [ ] **半步化虚 buff 强度**：当前 +10% 真元上限 / +200 年寿元是占位（已在 plan-tribulation-v1 §9 line 277 标注"延后决定"）。Phase 1-3 已上线，可观察"卡在半步化虚"的玩家比例后调整；名额空出时可重渡的升级机制也待确认（已在 plan-tribulation-v1 §8 标注）。**数据驱动调参，不需要另立 plan**

---

## v2 流派通用机制：熟练度生长二维划分

**2026-05-06 zhenmai-v2 首发，已编码进各 v2 plan 骨架**。此处保留为参考规范：

- **境界 = 威力上限**（K_drain / 反震点数 / 硬化抗性 / 中和兑换率 / 紊流半径 / 接经成功率 / 排异倍率等"大小"维度）
- **熟练度 = 响应速率**（冷却 cooldown / 弹反窗口 window / cast time / cast 充能时间）
- 公式：`cooldown(lv) = base + (min - base) × clamp(lv/100, 0, 1)`，线性递减
- 哲学：worldview §五:537 流派由组合涌现 + §五:506 末法残土后招原则物理化身——醒灵苦修 lv 100 弹反窗口 250ms / 化虚老怪 lv 0 仅 100ms（练得多胜过境界高）
- 待各 plan P0 决定是否派生 `plan-skill-proficiency-v1` 通用 plan 提取 cooldown/window curve helper

**需要回填此机制的 v2 plan**（各自 P0 决策门处理）：
- plan-woliu-v2（瞬涡 5s / 涡口 8s / 涡引 30s 改为按熟练度）
- plan-dugu-v2（蚀针 3s / 侵染 8s 改）
- plan-tuike-v2（蜕一层 8s / 转移污染 30s 改）
- plan-baomai-v3（待 P0 决定各招 base/min）
- plan-zhenmai-v2（已首发，弹反窗口 100→250ms 已定）
- plan-yidao-v1（接经术 cast_time 60→10s / 排异加速 cooldown 120→30s 已定）

---

> **约定**：每解决一条就从这里删。新增延后事项请直接追加到对应 plan 段，保持扁平。
