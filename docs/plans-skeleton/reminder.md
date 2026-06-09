# Skeleton 未决事项登记

本目录下 plan 在展开/落地过程中累积的**延后处理**事项。每条带 plan 锚点 + 上下文，后续回来解决时直接对号入座。

---

> **约定**：每解决一条就从这里删。新增延后事项请直接追加到对应 plan 段，保持扁平。

---

## 套包系统 4-plan 族（PR #467，Pi review 2026-06-10 产出的 §8.1 收口待办）

升 active 做 §8.1 收口时一并处理（Pi 标"升 active 前做"，非阻塞 merge）：

- **`plan-nested-pack-base-v1` §8 已决项搬正文**：#1 嵌套深度=1、#2 全计入负重 已写"用户已确认"，#3 子容器持久化时机其实是测试点（移到 P3/P5 测试项）——这三条从"开放问题"移入实现正文，§8 只留真正待拍板项（#4 grid 尺寸、#5 浮窗拖拽 P0 spike 风险）。
- **`plan-container-filter-and-completion-v1` §8 已决项搬正文**：Q1 筛选粒度（已决 ItemCategory 升变体）、Q3 `accept=[]` 全收（唯一合理选项）移入 P0 正文；§8 只留 Q2 `moisture_guard` SpoilOnly **精确 rate** —— 这是唯一需 P0 前拍板的数值（否则 `inventory_snapshot_emit` 永远 Normal 兜底，保鲜对玩家隐形）。
- **`plan-container-filter-and-completion-v1` 12 容器验收表加"落地阶段"列**：标每个容器在哪 plan 哪阶段落地（如 `herb_pouch`→A-P5+B-P3，`trade_crate`→C），降实施混淆。
- **Plan A 与 Plan D §8 交叉引用**：两者皆无依赖根 plan，A 嵌套深度=1 与 D 方块表示策略（vanilla 占位 vs bong_blocks）将来可能联动（bong_blocks 容器方块被放入套包时），各加一条交叉引用。
- **A-P5 / B-P3 改同一 TOML 协调**：两阶段都改 `workbench_materials.toml` 同 5 个随身子包条目，依赖图（B 依赖 A）已保证顺序，实施时注意相邻段 merge conflict。
