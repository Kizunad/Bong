# plan-bughunt-lingtian-growth-block-overwrite-v1

> 骨架：来源 #1870。生长 tick 无条件用 AIR/HAY 覆盖田顶已有玩家方块。

## §0 摘要

`server/src/lingtian/systems.rs:1993-2024` 每次 growth tick 直接把顶部方块写成 `AIR`/`HAY_BLOCK`，没有确认仍是本灵田登记的 crop block 或占用 token。玩家放置的方块会被覆盖。

## §1 游玩影响

灵田附近玩家建筑、箱子和装饰可能在作物生长时被破坏，且没有掉落/拒绝提示。

## §2 复现路径

1. 在灵田 crop 顶部放置任意非 crop 方块。
2. 等待 growth tick。
3. 观察 systems 无条件 `set_block(AIR/HAY)` 覆盖玩家方块。

## §3 今天 `origin/main` 根因证据

- `lingtian/systems.rs:1993-2024` 直接计算并写顶部 block state。
- 写入前没有读取当前 block 并与灵田登记的初始/占用状态比对。
- growth component 本身是唯一保护，玩家改方块后没有冲突分支。

## §4 非重复比对

`plan-bughunt-lingtian-harvest-full-inventory-spill-v1` 处理产出满包；`plan-block-lifecycle-v1` 处理通用 block 生命周期。本骨架只处理灵田 growth 的玩家覆盖保护。

## §5 立项检查记录

本记录已按 `docs/worldview.md`、`docs/finished_plans/`、`docs/plan-*.md`、`docs/plans-skeleton/` 与 `docs/plans-skeleton/reminder.md` 逐项检索。

- **worldview**：查“灵田、作物生长、玩家建筑”；`worldview.md §七/§十三` 要求耕地与世界建筑边界明确。
- **finished_plans**：查 `growth tick`、`set_block`、`Lingtian`；未见占用 token/冲突保护。
- **active plan**：查 lingtian systems/block lifecycle；无同一覆盖修复。
- **skeleton**：查 `growth block overwrite`、`HAY_BLOCK`、`crop block`；无重复骨架。
- **reminder.md**：查 `lingtian`、`growth`、`overwrite`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：灵田/crop registry、当前 block state、growth tick、玩家 block mutation。
- **Outputs**：仅在占用仍匹配时更新 crop block；冲突时保留玩家方块并记录可重试状态。
- **共享类型或事件**：复用 `Lingtian`/crop components、block place/break funnel、`BlockState`。
- **server 符号**：`lingtian::systems::growth_tick`、`world::block_place/block_break`。
- **agent**：无变更；生长 block 状态不进 agent schema。
- **client**：无变更；既有 block update 足以表现保护结果。
- **worldview 锚点**：`docs/worldview.md §七` 灵田生态与 §十三`建筑边界`。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 以 crop token/当前 block 校验保护玩家改动，冲突不覆写 |
| P1 | ⬜ | 正常成长、玩家放置/破坏、重启 hydrate 与重复 tick 回归测试 |

## 来源 issue

- #1870 `[flash-review][major] 生长 tick 用 Air/HayBlock 覆盖田顶玩家方块`
