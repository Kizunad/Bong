# plan-bughunt-ac-alchemy-hud-zero-targets-v1（骨架）

> **骨架（2026-07-05）**。一句话主题：真实炼丹链路里，server 发给 client 的 `alchemy_session` payload 把 **`target_ticks` / `temp_target` / `temp_band` / `qi_target` / `stages`** 全部写成 0 或空数组，导致炼丹 HUD 与炉内 UI 在玩家真正起炉后失去核心引导信息。

> **这个 bug 对实际游玩体验的影响**：玩家按 `I` 起炉后，中央炼丹 HUD 会长期显示 **`炼制 0%`**，温度条颜色判断失真，炉内面板会显示 **`elapsed / 0t`**、`当前火候 / 0.00`、`已注真元 / 0.0`，而且中途投料槽位不会按窗口闪烁提示。结果是玩家只能盲炼，无法根据 UI 判断何时收丹、火候是否贴近目标、还差多少注气、哪一槽该在当前 tick 投料；这直接削弱 `plan-alchemy-v1` 设计里“火候+阶段窗”的可玩性。

## P0 — `alchemy_session` 活跃链路零目标值（fix_pr）

- **高置信 bug（fix_pr）**：`server/src/network/alchemy_snapshot_emit.rs:218-245` 的真实 ECS 发送函数 `send_session_from_furnace`，在 `Some(s)` 分支里把 `target_ticks` 写死为 `0`、`temp_target` 写死为 `0.0`、`temp_band` 写死为 `0.0`、`qi_target` 写死为 `0.0`、`stages` 写成 `vec![]`；只有 `elapsed_ticks` / `temp_current` / `qi_injected` / `interventions_recent` 取自真实 session。与之相对，mock 通路 `mock_session()` 明明发送了完整目标值（同文件 `121-145` 行），说明 wire/schema/客户端消费端都支持这些字段，问题在真实 emit 缺字段，不在协议能力。
- **活跃可达性**：这不是 dormant mock。`BONG_ALCHEMY_JOIN_MOCKS` 默认关闭（`alchemy_snapshot_emit.rs:1-10,31-36,79-89`），正常玩家实际走的是 `client_request_handler.rs` 中 `handle_alchemy_open_furnace` / `handle_alchemy_ignite` / `handle_alchemy_intervention` / `handle_alchemy_feed_slot` / `handle_alchemy_take_back` 的重推路径，都会调用 `send_session_from_furnace`（`11349-11351`、`11518-11519`、`11447`、`11705`、`11894-11895`）。
- **客户端直接信任这些字段**：`client/src/main/java/com/bong/client/hud/AlchemyProgressHudPlanner.java:40-58,71-80` 用 `targetTicks` 算进度、用 `tempTarget/tempBand` 算温度条颜色；`client/src/main/java/com/bong/client/alchemy/AlchemyScreen.java:482-530` 直接把 `targetTicks` / `tempTarget` / `qiTarget` 画进文案，并且用 `stages` 驱动中途投料槽闪烁。server 发 0/空，client 就稳定渲染 0/空，不存在二次补算。
- **与原始设计相悖**：`docs/finished_plans/plan-alchemy-v1.md:310-314` 明确把中列定义为“状态行 + 进度条 + 温度条 + qi 条 + 干预 log”，`server/src/schema/alchemy.rs:176-190` 也把这些目标字段列为正式 `AlchemySessionDataV1` 契约，不是可选 embellishment。

## 修复方向

- `send_session_from_furnace` 需要拿到 `RecipeRegistry`（或等价 recipe lookup）并按 `session.recipe` 查配方，把 `fire_profile.target_duration_ticks / target_temp / tolerance.temp_band / qi_cost` 填入 payload。
- `stages` 需要由 `recipe.stages` + `session.staged.completed_stages/missed_stages` 生成 `AlchemyStageHintV1`，让 client 能恢复投料窗提示。
- 建议补 server 测试或 schema/emit pin：真实 session payload 不得再出现“active=true 但目标字段全 0、stages 空”的回归。

## 两轮反方裁决摘要

### Round 1

- **反方说法**：这可能只是 alchemy mock / vertical slice 残留，正常 gameplay 不会走到。
- **裁决**：不成立。mock 只在显式设置 `BONG_ALCHEMY_JOIN_MOCKS=1` 时给 joined client 推送；默认联机路径完全由 `handle_alchemy_open_furnace`、`handle_alchemy_ignite`、`handle_alchemy_intervention`、`handle_alchemy_feed_slot`、`handle_alchemy_take_back` 调 `send_session_from_furnace` 驱动。也就是说，玩家平时真正使用丹炉时看到的正是这条零目标值链。

### Round 2

- **反方说法**：就算 server 不发目标值，client 也许能从 recipe book 或本地硬编码自行补齐，所以不一定是 player-facing bug。
- **裁决**：不成立。HUD planner 和 `AlchemyScreen.refreshSessionText()` 都只读 `AlchemySessionStore`；它们没有从 `RecipeScrollStore` 反查目标值，也没有本地按 `recipe_id` 重建 `target_ticks/temp_target/qi_target/stages` 的逻辑。`refreshStageFlash()` 更是完全依赖 `s.stages()`。因此 server 发 0/空会稳定导致进度、温控和阶段提示全部失真，属于直接可见的 gameplay UI bug。

## 审计结论

- 主题去重：未重复 dandao mutation、voidaction target lock、processing deadpath、movement dash reject。
- 置信度：高。证据链覆盖 schema 定义、真实 emit、活跃调用点、client 消费点、原 plan 设计目标，且两轮反证均未推翻 player-facing 结论。
