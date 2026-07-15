# plan-bughunt-ac-alchemy-hud-zero-targets-v1

> **状态：active（2026-07-15 promotion）**。来源：
> `docs/plans-skeleton/plan-bughunt-ac-alchemy-hud-zero-targets-v1.md`（2026-07-05）。
> 一句话主题：真实炼丹链路里，server 发给 client 的 `alchemy_session` payload 把
> **`target_ticks` / `temp_target` / `temp_band` / `qi_target` / `stages`** 全部写成 0
> 或空数组，导致炼丹 HUD 与炉内 UI 在玩家真正起炉后失去核心引导信息。

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

## 接入面与收口决议

- 只修改 server 的真实 session snapshot 构造与五个现有重推调用点；wire schema、client
  planner/UI、配方 JSON 均已具备完整能力，不在本 plan 重构。
- 从 `send_session_from_furnace` 抽出可纯测的 snapshot builder，并显式接收
  `RecipeRegistry`。调用点为 open furnace、ignite、intervention、feed slot、take back。
- 已知配方的目标值只取权威 `Recipe.fire_profile`：
  `target_duration_ticks`、`target_temp`、`tolerance.temp_band`、`qi_cost`，禁止复制常数。
- 每个 `RecipeStage` 映射为一个 `AlchemyStageHintV1`；`summary` 按配方声明顺序拼成
  `material×count + material×count`，`completed` / `missed` 按 stage index 从 session
  集合读取。空材料 stage 允许空 summary，不伪造提示。
- 空炉继续发送 `active=false`、`recipe_id=None` 的“未起炉”快照，用于清除旧 HUD。
- 已结束但尚在炉组件中的已知 session 发送 `active=false`，同时保留真实配方目标与阶段
  状态，避免边界快照退回零目标。
- session 指向未知配方时 fail closed：记录 warning，并发送 `active=false` 的显式
  “丹方数据缺失”快照，保留 recipe id 供诊断，但不再伪装为 `active=true + 零目标`。
- 本修复只恢复既有 HUD 数据，不新增炼丹招式/技能，不触及真元转移、配方数值、A/V
  资产或 `docs/worldview.md`。

## 实施阶段

- [x] P0：加入修复前失败的 snapshot builder 契约测试，锁定真实目标值与阶段提示。✅ 2026-07-15
- [x] P1：接通 `RecipeRegistry`，实现已知配方 snapshot 与未知配方 fail-closed。✅ 2026-07-15
- [x] P2：更新五个真实调用点，完成定向测试与 server 完整门禁。✅ 2026-07-15
- [x] P3：同步最新主线、对抗自审、填写 Finish Evidence 并归档。✅ 2026-07-15

## 验收与测试矩阵

| 场景 | 必须断言 |
|---|---|
| 已知 active session | `active=true`；四个目标字段等于 registry 配方；运行态字段保持 session 值 |
| 多材料 stages | stage 数量/顺序、`at_tick`、`window`、`material×count + ...` summary 全匹配 |
| completed / missed | 对应 stage index 的两个布尔值分别准确，未命中 stage 均为 false |
| 空炉 | `active=false`、`recipe_id=None`、零运行态、空 stages、状态“未起炉” |
| 未知 recipe | `active=false`、保留 recipe id、状态“丹方数据缺失”，不得发 active 零目标 |
| finished session | `active=false`，但已知 recipe 的目标字段与 stages 仍完整 |
| 调用接线 | open / ignite / intervention / feed / take-back 均传入同一权威 registry |
| 完整门禁 | `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test` 全绿 |

## 非目标

- 不改变炼丹配方目标值、阶段窗口或结果算法。
- 不新增 client fallback，也不让 client 从 recipe book 重建 server snapshot。
- 不顺手重构 `client_request_handler.rs` 的大型 dispatch 或其它炼丹 freshness/背包逻辑。

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

## Finish Evidence

### 落地清单

- `server/src/network/alchemy_snapshot_emit.rs` 从真实 `RecipeRegistry` 构造 session
  snapshot：恢复时长、温度目标/容差、注气目标、逐阶段材料摘要与 completed/missed 状态。
- open furnace、ignite、intervention、feed slot、take back 五条生产重推路径均传入同一权威
  registry；未改 wire schema 或 client fallback。
- 空炉继续清除 HUD；finished session 变为 inactive 但保留配方引导；未知 recipe 记录 warning
  并发送“丹方数据缺失”的 inactive 快照，禁止 active 零目标。

### 关键提交

- `30ad7653`：升格炼丹 HUD 零目标 BugFix 计划。
- `55dc5553`：加入修复前失败的 snapshot 契约；红灯为 5 项中 3 项失败，分别锁定
  目标字段为 0、未知 recipe 仍 active、finished 状态缺失。
- `210349e2`：从权威配方恢复目标/阶段提示并完成 fail-closed。

### 测试结果

- 定向：`cargo test network::alchemy_snapshot_emit::tests -- --nocapture` → 5 passed，
  0 failed（active、stage summary、completed/missed、空炉、未知 recipe、finished 边界）。
- 完整 server 门禁（`BONG_SKIP_SKIN_PREFETCH=1`）：
  `cargo fmt --check` 通过；`cargo clippy --all-targets -- -D warnings` 通过；
  `cargo test` 通过（lib 11695 passed / 1 ignored，main 11 passed，startup 1 passed，
  backpack e2e 4 passed，doc-tests 0 failed / 5 ignored）。

### 主线与对抗核验

- 2026-07-15 fetch 后 `origin/main=6f1faea5`，是修复 HEAD `210349e2` 的祖先，判定
  already-up-to-date；无需合并，完整门禁证据仍绑定当前 HEAD。
- 主 agent 绑定干净 HEAD `210349e22324b5b08cfb19368928a7f05f9619dd` 做对抗自审：
  五条生产调用全接线；client handler 会替换 store，`active=false` 经 `isActive()` 清除
  HUD/阶段闪烁；所有新增 commit 均含 `Model: gpt-5`。结论：
  `PASS 210349e22324b5b08cfb19368928a7f05f9619dd`。

### 跨栈核验与遗留

- 本次只改变 server payload 的既有字段取值，schema 与 client consumer 未改；通过源码对拍确认
  `AlchemyProgressHudPlanner` 和 `AlchemyScreen` 直接消费这些字段，因此无需 client 构建。
- 无阻塞项，无真元流动/配方数值/A/V 资产变化，无已知遗留。
