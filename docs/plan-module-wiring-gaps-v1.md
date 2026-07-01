# plan-module-wiring-gaps-v1

> 主题：模块图谱（`module-map/`）审计发现的**孤岛/未完全链接**模块——定义齐全（逻辑/测试/schema 都在）但生产/消费路径缺一环，对应 gameplay loop 在正常游戏中**永不触发**。本 v1 只落地**按先例接线/持久化的低设计风险项**；涉真元守恒或需 gameplay 设计抉择的（shader 触发源 / mineral 世界方块复活 / pvp 击杀 fire 点 / identity 声誉 fire 条件 / sword stored_qi 守恒 / era 间隔值）**留 v2 待人工拍板**（见 §9）。

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | client/dandao — MutationHudPlanner + MutationFeatureRenderer 接线 | ⬜ |
| P1 | server/craft — RecipeUnlockState 持久化（真实数据丢失修复） | ⬜ |
| P2 | client/npc — NpcInteractionLogStore 断线清理 | ⬜ |

来源：`module-map/index.html`「⚑ 缺口」tab（sonnet 调查 → opus 抽查证实无 producer/consumer）。全量清单见 `docs/plans-skeleton/`（本 plan 由该骨架促成，只取可实施子集）。

## 接入面

- **进料**：P0 读 `dandao` 变异状态 store（`mutation_visual`→state）；P1 读 `craft::unlock::RecipeUnlockState`（现纯内存 Resource）；P2 读 client 断线事件（`ClientPlayConnectionEvents.DISCONNECT`）。
- **出料**：P0 → HUD 编排器渲染 `DANDAO_MUTATION` 层 + PlayerEntityRenderer 叠加异化贴图；P1 → `persistence`（bong.db，跟 inventory/其他持久化同库）读写；P2 → 清空 `NpcInteractionLogStore` 条目。
- **共享类型 / event**：P0 复用现有 `MutationHudPlanner`/`MutationFeatureRenderer`/`HudRenderLayer.DANDAO_MUTATION`（**不新建**）；P1 复用现有 `RecipeUnlockState` 结构 + 现有 persistence 框架（跟 `ExhaustedMineralsLog` 落盘同模式）；P2 复用现有 `NpcInteractionLogStore`。
- **跨仓库契约**：三项均为**单端接线**（P0/P2 纯 client，P1 纯 server），无新 IPC schema / Redis key / CustomPayload。
- **worldview 锚点**：P0 丹道变异 = worldview 丹道走火入魔可视化；P1/P2 为持久化/UI 卫生，无新玩法。
- **qi_physics 锚点**：**三项均不涉真元/灵气流动**（纯 UI 接线 + 配方解锁状态持久化 + 日志清理），无 qi_physics 调用，无守恒风险。

---

## P0 — client/dandao：丹道 HUD 与异化贴图接线 ⬜

**现状（孤岛，已 grep 证实）**：
- `MutationHudPlanner.buildCommands()` 主代码无 HUD 编排器调用（仅测试引用）；`DANDAO_MUTATION` 层在 `HudRenderLayer`/`HudLayoutPreset` 已声明但 `buildCommands` 从未触发 → 丹道 HUD 面板从不渲染。
- `MutationFeatureRenderer` 从未注册到任何 PlayerEntityRenderer（主代码仅 WornPack 三处 javadoc 把它当"未注册孤岛反面教材"引用）→ 玩家身体异化叠加贴图永不显示。

**实施方案（已解决，按先例）**：
1. **HUD 接线**：找到现有 HUD 编排器主循环里其他 `*HudPlanner.buildCommands()` 的注册/调用点（如 `OverweightHudPlanner` 在其接线处，webui 记 `第155行`），把 `MutationHudPlanner` 按同样模式接入，条件门控（有变异状态才渲染，遵循 [[feedback_hud_conditional]] 未激活隐藏而非灰掉）。
2. **Renderer 注册**：把 `MutationFeatureRenderer` 注册进 `PlayerEntityRenderer` 的 FeatureRenderer 列表，参照现有 client FeatureRenderer 注册先例。
   - **⚠️ 硬约束**：`MutationFeatureRenderer` 若基于 GeckoLib GeoModel，**不能在 player FeatureRenderer 直接驱动 GeoModel**（[[feedback_mixin_package_helper]] / 套包实证）——必须转 vanilla `ModelPart` 叠加。实施 agent 先读 `MutationFeatureRenderer` 判定其模型类型，GeckoLib 就转 ModelPart，vanilla 叠加则直接注册。
- **deliverable**：`client/.../dandao/` 或 hud 接线点 + `MutationFeatureRenderer` 注册；`MutationHudPlanner.buildCommands` 有生产调用方；gradle build 通过。

## P1 — server/craft：RecipeUnlockState 持久化 ⬜

**现状（真实数据丢失 bug）**：`server/src/craft/unlock.rs` 的 `RecipeUnlockState` 是纯内存 `Resource`，`persistence` 层零引用 → server 重启清空全玩家配方解锁进度（对照 [[player_inventory_persist_migration_gap]] 同类持久化缺口）。

**实施方案（已解决，按先例）**：
1. 参照现有持久化模式（如 `mineral/persistence.rs::ExhaustedMineralsLog` 的 hydrate/flush 节流落盘，或 inventory bong.db 表），给 `RecipeUnlockState` 加持久化：启动 hydrate、变更标 dirty、节流 flush 到 bong.db（或 data/ JSON，跟现有 craft 数据落点一致——实施 agent 先查 craft/其他模块的持久化落点惯例）。
2. 按玩家 character_id 分键存解锁状态。
- **deliverable**：`craft::unlock` 或 `craft::persistence` 持久化读写函数 + `register` 里 hydrate + Update 里 flush 系统；`cargo test` 覆盖：解锁→flush→重 hydrate 状态一致、空态、多玩家隔离。

## P2 — client/npc：NpcInteractionLogStore 断线清理 ⬜

**现状（孤岛）**：`NpcInteractionLogStore` 只有 `resetForTests`，无 `clearOnDisconnect` → 重连后旧交互日志条目残留（跨会话串味）。

**实施方案（已解决，按先例）**：加 `clearOnDisconnect()`（或复用清空逻辑），在 `ClientPlayConnectionEvents.DISCONNECT`（参照 client 其他 store 的断线清理先例，如 combat/ 各 store 的 join/disconnect hook）注册调用。
- **deliverable**：`NpcInteractionLogStore.clear*()` + DISCONNECT 注册调用；gradle build 通过。

---

## §9 — 留 v2 待人工拍板（本 plan 不实施）

以下孤岛涉真元守恒或 gameplay 设计抉择，**禁止本 plan 自动拍板**（docs/CLAUDE.md §五 + qi 守恒红线）：

- **shader ↔ iris**（跨层两端断）：需定「哪些 gameplay 事件驱动 shader」+ client Iris uniform 注入方案。
- **mineral 矿脉再生**：需定世界方块复活语义（ChunkLayer 写 + mineral_id→BlockState 映射 + 未加载区块处理）。
- **social PvpEncounter producer**：需定击杀/PvP 结算 fire 点与 PvP 判定。
- **identity DuguRevealed producer + IdentityReactionScorer 挂载**：需定 reveal fire 条件 + scorer 挂哪些 NPC。
- **★sword `SwordBondComponent.stored_qi` 脱守恒账**（可能 critical）：`summarize_world_qi` 不计 stored_qi，疑大额脱账——**需先核验是否真守恒漏洞**，涉 qi_physics 守恒律（[[project_bughunt_qi_conservation]]），必须人工/专项 plan。
- **agent era intervalMs=36,000,000ms(10h)**：疑似笔误，但改值需定"演绎时代"推演频率（gameplay 节奏抉择）。

## §10 — 实施工作流

本 plan scope < 4 PR、纯逻辑/接线（无建筑/视觉资产多轮），三项相互独立、单端接线。

- **10.1** 三项各一 commit，合成**一个 PR**（`feat/module-wiring-fixes`）：`docs: 提 active + P0 dandao 接线 + P1 craft 持久化 + P2 npc 日志清理`。评审量适中，同属"接线/持久化卫生"主题。
- **10.2** 实施用 **Sonnet 5**（`model:'sonnet'`）逐项 agent，各自读码→按先例实施→build/test→commit；主线核验 diff + 跑测试。（本轮目的之一：评估 Sonnet 5 实施质量。）
- **10.3** 每项 agent 必须：先读现有先例（同类 HudPlanner 注册 / persistence 落盘 / disconnect hook），**不臆造**接口；GeckoLib 门（P0-2）先判定模型类型。
- **10.4** build/test 门：P0/P2 `cd client && ./gradlew build`（Java 17）；P1 `cd server && cargo test`。全绿才合。
- **10.5** PR 后等 CodeRabbit（`ScheduleWakeup` 1200s，≤3 回合），无阻塞 finding 且 CI 绿 → squash merge。
- **10.6** 全 P ✅ + 本节 Finish Evidence 填好 → `git mv` 入 `finished_plans/`。

## Finish Evidence

（实施完成后填：落地清单 / 关键 commit / 测试结果 / 遗留。）
