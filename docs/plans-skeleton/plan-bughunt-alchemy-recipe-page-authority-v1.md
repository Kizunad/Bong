# plan-bughunt-alchemy-recipe-page-authority-v1（骨架）

> **来源 issue**：#1792。生产翻页路径在玩家没有 `LearnedRecipes` 时调用 mock recipe book，客户端会把示例丹方当成真实可用内容。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 炼丹翻页只下发权威玩家配方，空数据明确为空，不再回退 mock | ⬜ |

## §0 摘要

`send_recipe_book` 在 `server/src/network/alchemy_snapshot_emit.rs:352-369` 调用 `mock_recipe_book_at`；`client_request_handler` 对空 `LearnedRecipes` 走 fallback。UI 因而拿到示例配方，权限/学习状态与显示脱钩。修法必须以玩家的权威学习状态为输入，空结果不能伪造配方。

## §1 游玩影响

玩家翻页时可能看到并尝试未学习的丹方，造成“配方已解锁”的误导，严重时会把 UI 预览当成生产入口。此骨架不改变炼制扣除或物品产出，只修快照来源。

## §2 复现路径

1. 使用没有 `LearnedRecipes` 的玩家请求炼丹配方页。
2. 观察 `send_recipe_book` 走 `mock_recipe_book_at`，`ServerDataPayloadV1::AlchemyRecipeBook` 含示例条目。
3. 将返回 payload 与权威学习集合对照，发现条目并非玩家状态。

## §3 今天 `origin/main` 的证据

- `server/src/network/alchemy_snapshot_emit.rs:352-369` 的生产发送路径直接调用 `mock_recipe_book_at`。
- `server/src/network/client_request_handler.rs:5176-5218` 对空 `LearnedRecipes` 进入 fallback。
- payload 类型是 `ServerDataPayloadV1::AlchemyRecipeBook`，不是仅用于测试的本地 UI 数据。

## §4 非重复比对

已查 `docs/finished_plans/plan-alchemy-v1.md`、active plan、skeleton 中的 `AlchemyRecipeBook`、`LearnedRecipes`、`mock_recipe_book_at`；没有现有骨架覆盖“翻页权威源”这一断点。#1792 作为唯一来源 issue 保留。

## §5 立项检查记录

- `docs/worldview.md`：查丹药/炼制/传承条目；计划不新增丹方或货币规则。
- `docs/finished_plans/`：查 `plan-alchemy-v1` 的学习与配方快照契约。
- active plan：查 `AlchemyRecipeBook`、`LearnedRecipes`、recipe page，未发现同一发送函数的改动。
- skeleton：查 #1792、`mock_recipe_book_at`、`ServerDataAlchemyRecipeBookV1`，无重复骨架。
- `docs/plans-skeleton/reminder.md`：已检查，未找到配方页条目；不改该文件。

## §6 接入面与跨仓契约

- **Inputs**：server 的玩家 `LearnedRecipes`/炼丹会话和翻页请求。
- **Outputs**：`ServerDataPayloadV1::AlchemyRecipeBook`，序列化为 `ServerDataAlchemyRecipeBookV1`。
- **共享类型/事件**：复用 `AlchemyRecipeBookDataV1`、`ServerDataV1` 和既有 recipe schema；不新造 mock 类型。
- **三端契约符号**：server `alchemy_snapshot_emit::send_recipe_book`、`client_request_handler`；agent `ServerDataAlchemyRecipeBookV1` 仅作 schema registry 对齐、**无运行时变更**；client `AlchemyRecipeBookHandler` 与 `ServerDataRouter` 消费 payload。
- **worldview 锚点**：`docs/worldview.md` 丹药/传承与骨币经济；本修复不产生真元转移，不改 `qi_physics`。

## P0 验收

- 空 `LearnedRecipes` 下发空且带清晰状态的 recipe book，不出现 mock 条目。
- 已学习条目与玩家权威集合一一对应；翻页、刷新和重连不改变权限。
- agent schema、client handler 与 server payload 仍能互相解析。
