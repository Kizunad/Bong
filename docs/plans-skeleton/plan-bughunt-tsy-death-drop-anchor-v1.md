# plan-bughunt-tsy-death-drop-anchor-v1

> 骨架：来源 #1358。TSY 死亡掉落使用复活后的当前位置，死亡点锚点没有传入分流函数。

## §0 摘要

`server/src/inventory/mod.rs:4375-4378` 在复活事件处理器读取当前 `Position` 作为 `base`，随后 `apply_tsy_death_drop`（`:4392-4398`）和地面条目（`:4421-4423`）都使用它。死亡时由 `combat/lifecycle.rs:508-513` 写入的 `DeathDropAnchor` 未被该路径读取。

## §1 游玩影响

玩家死亡后若被传送、复活点重定位或位置同步，TSY 战利品和干尸出现在复活点，死亡现场无法找回，可能与其他玩家争夺错误地点的掉落。

## §2 复现路径

1. 在 TSY 坐标 A 触发死亡，记录 `DeathDropAnchor`。
2. 在复活前把实体位置变为 B（复活点）。
3. 观察 `DroppedLootEntry.world_pos`、干尸位置都取 B 而非 A。

## §3 今天 `origin/main` 根因证据

- `server/src/combat/lifecycle.rs:508-513` 产生死亡锚点。
- `server/src/inventory/mod.rs:4375-4378` 复活处理器无条件读取复活时 `Position`。
- `server/src/inventory/mod.rs:4392-4398,4421-4423` 将该当前位置传给 TSY 掉落和 `DimensionKind::Tsy` 条目。

## §4 非重复比对

`plan-tsy-lifecycle-v1` 负责秘境存在与塌缩生命周期，`plan-bughunt-dropped-loot-cross-dimension-pickup-v1` 负责拾取维度门禁；两者都不修复死亡坐标在事件链中的传递。

## §5 立项检查记录

本记录已按 `docs/worldview.md`、`docs/finished_plans/`、`docs/plan-*.md`、`docs/plans-skeleton/` 与 `docs/plans-skeleton/reminder.md` 逐项检索。

- **worldview**：查“末法秘境、死亡、干尸、遗留物”；`worldview.md §十二` 要求遗留物留在事件发生地。
- **finished_plans**：查 `DeathDropAnchor`、`apply_tsy_death_drop`、`corpse_pos`；未见复活前后坐标契约。
- **active plan**：查 `PendingTsyDeathDrop`、`DeathDropAnchor`；没有占用锚点到 TSY 分流的实现。
- **skeleton**：查 `tsy death`、`anchor`、`corpse_pos`；无同根因骨架。
- **reminder.md**：查 `TSY`、`death drop`、`anchor`；仓内无对应条目。

## §6 接入面与跨仓契约

- **Inputs**：`DeathDropAnchor`、`PendingTsyDeathDrop`、`TsyPresence`、死亡维度与物品快照。
- **Outputs**：`TsyDeathDropOutcome`、`DroppedLootEntry.world_pos`、`CorpseEmbalmed` 的死亡位置。
- **共享类型或事件**：复用 `DeathDropAnchor`、`TsyDeathDropOutcome`、`DroppedLootRegistry`；禁止另造复活位置字段。
- **server 符号**：`combat::lifecycle`、`inventory::revive_death_drop_system`、`inventory::tsy_death_drop::apply_tsy_death_drop`。
- **agent**：无变更；掉落位置不是 Redis/agent 契约。
- **client**：无变更；客户端继续渲染 server 下发的掉落实体。
- **worldview 锚点**：`docs/worldview.md §十二` TSY 遗留物与死亡现场。

## 阶段总览

| 阶段 | 状态 | 交付物 |
| --- | --- | --- |
| P0 | ⬜ | 以死亡锚点为唯一位置源，贯穿 TSY 掉落、干尸和 registry 条目 |
| P1 | ⬜ | 复活传送、无锚点回退、跨维度拾取回归测试 |

## 来源 issue

- #1358 `[flash-review][major] TSY死亡掉落错落复活点而非死亡点`
