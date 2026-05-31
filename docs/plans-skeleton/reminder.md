# Skeleton 未决事项登记

本目录下 plan 在展开/落地过程中累积的**延后处理**事项。每条带 plan 锚点 + 上下文，后续回来解决时直接对号入座。

---

> **约定**：每解决一条就从这里删。新增延后事项请直接追加到对应 plan 段，保持扁平。

---

## plan-offscreen-war-v1 · P5 前置决策

**锚点**：`docs/plan-offscreen-war-v1.md §P5`

**事项**：P5（具名势力系统）启动前需要**人工决定**世界观方向。当前两个选项：

- **(a)** 在 `worldview.md` 新增 §十八 具名势力章节（散修群体 / 隐修门庭 / 遗族守卫 三类）
- **(b)** 将势力改写为"散修群体自发涌现消长"形态，复用现有 §七 动态生物生态框架，不新增世界观章节（**当前倾向**）

`worldview.md` 修改属于**人工决策**范畴，agent 不能自动改。**P5 必须等此决策落地后才能进入 active 消费。**

---

## plan-offscreen-war-v1 · P5 完成后废弃 4 个骨架

**锚点**：`docs/plan-offscreen-war-v1.md §遗留后续`

**事项**：以下 4 个骨架被 `plan-offscreen-war-v1` 覆盖，完成后执行：

```bash
git rm docs/plans-skeleton/plan-npc-virtualize-v2.md
git rm docs/plans-skeleton/plan-npc-virtualize-v3.md
git rm docs/plans-skeleton/plan-faction-wars-v1.md
git rm docs/plans-skeleton/plan-faction-expansion-v1.md
```

同步更新 `plan-social-v2.md` 中引用这 4 个骨架的交叉引用行。**在 offscreen-war P5 验收 Finish Evidence 写完后执行，不要提前删。**
