# plan-bughunt-alchemy-furnace-persistence（skeleton）

> 状态：Skeleton Plan，仅记录 BugHunt H5（persistence 第五轮）确认的高置信 bug。不要消费、不要归档，待人工提升为 active 后再进入 plan 流水线。

## Bug 摘要

玩家放置并使用炼丹炉后，服务端不持久化/重建权威 `AlchemyFurnace` 组件及其 `AlchemySession`。服务器重启后，该坐标即使仍有 vanilla `FURNACE` 方块，也无法继续作为 Bong 炼丹炉使用，后续 open / ignite / feed / take_back 会走“炼丹炉不存在”分支。

## 实际游玩体验影响

- 玩家消耗炉类物品放下炼丹炉后，重启服务器可能只剩一个视觉上的普通炉，Bong 炼丹 UI 无法重新打开。
- 若已经起炉并投料，投料已从背包扣除，但 `AlchemySession.staged` 只在内存里；重启后玩家既拿不回材料，也拿不到丹药结果。
- 多炉并行玩家最容易踩中：前一晚布置的个人炉、正在跑的丹火、炉体完整度和 owner 权限都会丢失，表现为同一坐标突然报“炼丹炉不存在”。

## 证据定位

- `server/src/alchemy/mod.rs:422`-`430`：放炉流程明确写明消耗物品、`commands.spawn(AlchemyFurnace::placed(...))`、`layer.set_block(FURNACE)`，并注明“纯内存：炉状态不落盘，服务器重启 = 炉丢失”。
- `server/src/alchemy/mod.rs:478`-`491`：成功路径先 `consume_item_instance_once` 扣掉炉物品，再只 spawn ECS 组件并写内存方块。
- `server/src/alchemy/furnace.rs:1`-`4`：模块注释说明 BlockEntity 持久化仍待 `plan-persistence-v1` 对接；当前只是内存表现。
- `server/src/alchemy/furnace.rs:19`-`31`：`AlchemyFurnace` 虽可序列化且有 `bound_entity` 预留，但没有落库/恢复调用。
- `server/src/network/client_request_handler.rs:11953`-`11986`：打开炉时只通过 `with_owned_furnace_mut` 查 ECS 组件；缺失即发“炼丹炉不存在”。
- `server/src/network/client_request_handler.rs:12622`-`12627`：权威路由按 `furnace.pos == Some(furnace_pos)` 查 `Query<(Entity, &mut AlchemyFurnace)>`，查不到直接 `Missing`。
- `docs/finished_plans/plan-alchemy-v1.md:327`-`329`：原计划仍把 `AlchemyFurnace component + BlockEntity（持久化 session_id）` 和 `AlchemySession resource` 标为未完成。

## 触发路径

1. 玩家持有炼丹炉物品，发送 `alchemy_furnace_place`，服务端扣除该物品并在坐标生成 `AlchemyFurnace` 组件。
2. 玩家打开该炉并 `ignite`，可选继续 `feed_slot` 投入材料，材料从背包扣除并进入内存 `AlchemySession.staged`。
3. 服务器重启，ECS 里的 `AlchemyFurnace` / `AlchemySession` 没有从持久层恢复。
4. 玩家重连后对同坐标发送 `alchemy_open_furnace` 或后续炼丹操作。
5. 服务端按坐标查不到 `AlchemyFurnace` 组件，返回“炼丹炉不存在”；已消耗炉物品/投料没有恢复路径。

## 非重复确认

- 不重复 #972：该 PR 是 dormant NPC Redis dirty 预清导致快照回滚。
- 不重复 #985：该 PR 是矿脉 exhausted 日志半写截断。
- 不重复 #991：该 PR 是 SurfaceStash 生命周期易失状态。
- 不重复 #996：该 PR 是 botany 采药断线 session 残留。
- 不重复 #953/#990：两者聚焦 placed container / `ExternalContainer.opened_by` 会话锁和断线生命周期，不覆盖 `AlchemyFurnace` / `AlchemySession` 重启恢复。
- 不重复 #981：#981 修炼丹炉距离/维度门禁，假设炉组件存在；本 bug 是重启后组件不存在。

## 反方审查记录

- Round 1：反方尝试寻找通用 ChunkLayer / BlockEntity / persistence 恢复链，未发现 `AlchemyFurnace` 或 `AlchemySession` 被保存/加载；确认现有 C2S 链路已可实机触发，不只是未来 TODO。
- Round 2：反方指出不能断言 vanilla 炉方块必然消失；准确表述应收窄为“权威 `AlchemyFurnace` 组件和 `AlchemySession` 丢失”。即使方块仍可见，Bong 炼丹交互仍会按组件查询失败。

## Skeleton Fix Plan

- [ ] P0：定义炼丹炉持久化模型，至少包含 pos、tier、owner、integrity、bound_entity、server_run/migration metadata。
- [ ] P0：在放置、炸炉完整度变化、owner 变更、炉销毁时写入持久层；写入失败必须回滚或明确拒绝玩家操作，避免“物品已扣但炉未入档”。
- [ ] P1：定义 `AlchemySession` 持久化模型，覆盖 recipe、caster_id、elapsed_ticks、temp_current/temp_track、qi_injected、staged materials、interventions、finished。
- [ ] P1：在 ignite / feed_slot / intervention / take_back 的状态变更点持久化 session；重启后按 pos 恢复到对应 `AlchemyFurnace.session`。
- [ ] P2：启动期从持久层重建 `AlchemyFurnace` ECS 组件和视觉 marker；若世界后端已有 vanilla `FURNACE` 方块但缺组件，只对有持久记录的坐标恢复权威炉，避免误收普通炉。
- [ ] P2：补 orphan 处理策略：持久记录存在但方块不存在、方块存在但记录不存在、owner 玩家已换角色、session 处于 finished / corrupted 状态时分别给出可测试行为。

## 验收测试计划

- server 单测：放炉成功后持久层存在一条 furnace row，且 inventory revision / 炉物品扣除与 row 写入保持一致。
- server 重启模拟测试：放炉后重建 App，加载 persistence，按原坐标 `open_furnace` 能返回 furnace snapshot，而不是“炼丹炉不存在”。
- server session 恢复测试：ignite + feed_slot 后重建 App，恢复的 `AlchemySession` 保留 recipe、caster_id、staged materials、elapsed/temp/qi 状态，take_back 可继续结算。
- 负向测试：只有 vanilla `FURNACE` 方块但没有 Bong 持久记录时，不应伪造 owner/session；仍拒绝为 Bong 炼丹炉。
- 回归命令：在 `server/` 跑 `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`。

## 风险

- 世界后端可能持久化 vanilla 方块，但这不能等价于 Bong 炉；修复必须以服务端权威记录为准。
- mid-session 恢复会影响材料返还/继续炼丹/失败结算语义，需要先定不变量，避免重启后复制材料或吞材料。
- 持久化写入必须和物品扣除保持同一失败边界，否则会制造新的“扣物品但无炉”或“有炉但物品未扣”分叉。
- 旧档中已存在的普通 `FURNACE` 方块无法可靠推断 owner / tier / session，不应自动迁移为 Bong 炉。
