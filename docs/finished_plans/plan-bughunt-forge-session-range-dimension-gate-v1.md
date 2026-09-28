# plan-bughunt-forge-session-range-dimension-gate-v1

## §0 摘要

整条炼器（武器）会话生命周期——起炉 `ForgeStartSession`、淬炼 `ForgeTemperingHit`、铭文 `ForgeInscriptionScroll`、开光 `ForgeConsecrationInject`、推进 `ForgeStepAdvance`——从服务端到客户端全程从不校验玩家与锻炉站点的距离或维度，只查 owner。玩家可以在自己合法拥有的锻炉前起炉后直接走开甚至换维度，继续用 J/K/L 等按键把整个锻造流程跑完，服务端照常推进会话、消耗材料/图谱、结算成品。这比已有的 `forge-station-place-gate`（仅覆盖放置阶段）覆盖面更大——覆盖起炉到成品产出的整条链路。

本 plan 仅是 BugHunt Skeleton Plan，不包含实际修复。

## §1 实际游玩体验影响

- 破坏"玩家必须待在锻炉旁操作"这一世界交互基本预期：起炉后因分心/被打断/去搬运材料/用位移类招式/掉入地穴/穿越至 TSY 等任何理由离开，锻造小游戏仍会照常推进到结算。
- `ForgeStationStore` 缓存最近一次 `forge_station` 快照且跨 session 甚至跨服不清（`forge-ui-session-stale` 骨架已指出），玩家哪怕本地没有当前 screen 打开过，只要重新按 U 打开 ForgeScreen 时手里还有旧的 station_pos 缓存，同样能对着一个自己压根不在附近的（自己拥有的）站点发起全新 `forge_start_session`。
- ForgeScreen 是裸 `extends Screen`（非绑定 vanilla ScreenHandler），没有"玩家走远自动关闭"机制，客户端侧同样零把关。

## §2 复现路径

1. 玩家在自己合法放置的锻炉前按 U 打开 ForgeScreen，选图谱投料点火起炉（`forge_start_session` 正常发送，`handle_start_forge_requests` 接受）。
2. 在"淬火/铭文/开光"多 tick 小游戏进行途中，玩家直接走开甚至换维度（无需改包、无需 dev 命令）。
3. ForgeScreen 不会因为玩家远离而关闭，玩家继续按 J/K/L、投铭文残卷、点"开光"注入真元、点"下一步"。
4. 现状预期：服务端 `require_owned_active_step`（`client_request_handler.rs:3522`）只检查 `session_state.caster != entity` 和 `current_step` 匹配，没有距离/维度参数，照常推进会话，消耗材料/图谱，结算 `achieved_tier` 与最终武器写入背包。
5. 修复后预期：距离/维度校验失败时拒绝推进，给出 chat 回执，不消耗材料/不推进 step_index/不扣真元；客户端 ForgeScreen 在玩家离站点过远时自动 close。

## §3 根因证据

- `server/src/forge/mod.rs:182` `handle_start_forge_requests` 的 system 参数列表（ev/registry/minerals/sessions/stations/learned/inventories/accepted/outcomes/feedback）不含任何 `Query<&Position>` 或 `Query<&CurrentDimension>`；全函数只校验图谱已学、station tier、材料是否足量，从未读玩家坐标。
- `server/src/network/client_request_handler.rs:3308` `find_owned_forge_station` 只按 `station.pos == Some(station_pos)` 找实体、`station.owner == player` 判权限，同样没有玩家 `Position`/`CurrentDimension` 输入。
- `server/src/network/client_request_handler.rs:3522` `require_owned_active_step`（tempering_hit/inscription_scroll/consecration_inject 共用的守卫）只检查 `session_state.caster != entity` 和 `current_step` 匹配，没有距离/维度参数。
- `server/src/forge/mod.rs:418` `handle_tempering_hits` 与 `:556` `handle_consecration_injects` 的 system 参数同样不含 `Position`/`CurrentDimension`。
- 对照 `server/src/forge/artifact_meridian.rs:560` 同模块下另一个子系统确实用了 `Query<&Position>`，证明并非"Bevy 查询不可用"，而是这条会话链路整条漏做。
- 对照已确认的established 门禁模式：`server/src/world/container_open.rs` 用 `Position + CurrentDimension + OPEN_RANGE_BLOCKS=4.0 + dimension_or_overworld` 校验同维 4 格距离。
- 客户端侧同样零把关：`client/src/main/java/com/bong/client/forge/ForgeScreen.java` 是裸 `extends Screen`（非绑定 ScreenHandler 的 vanilla container，没有 vanilla 自带的"玩家走远自动关闭"机制）；`client/.../forge/input/ForgeStartInputHandler.java:39` `tryStartForge` 用的 `stationPos` 来自 `ForgeStationStore.snapshot().pos()`——上次收到的 S2C 快照缓存，不是玩家当前射线/位置；`client/.../forge/input/TemperingInputHandler.java:22-35` `handleKey` 只检查 `snapshot.sessionId()>0` 和 `currentStep=="tempering"`，同样没有距离判断。

## §4 非重复比对

- 已读 `docs/plans-skeleton/plan-bughunt-forge-c2s-session-wiring-v1.md`：只讲 `ForgeStartSession`/`BlueprintTurnPage`/`LearnBlueprint` 缺 handler，现状已由已归档的 `docs/finished_plans/plan-forge-session-entry-wiring-v1.md`（#1141）补齐真实分发，`handle_forge_start_session`/`handle_forge_blueprint_turn_page` 已是生产实现，非死分支——该骨架不覆盖距离/维度门禁。
- 已读 `docs/plans-skeleton/plan-bughunt-forge-station-place-gate-v1.md`：明确只覆盖 `ForgeStationPlace`（放置新站点）的坐标门禁，其 §5 修复计划、§4 非重复说明都只字未提 `ForgeStartSession`/`ForgeTemperingHit`/`ForgeInscriptionScroll`/`ForgeConsecrationInject`/`ForgeStepAdvance`（对已放置站点的后续操作）。该骨架用作对照的 `container_open.rs`/`workbench.rs` 先例证明"操作已有设施需要距离校验"是本仓established 模式，与本 finding 互为佐证而非重复。
- 已读 `docs/plan-bughunt-forge-ui-session-stale-v1.md`：客户端 store 断线残留，不同故障模式。
- 已读 `docs/plan-bughunt-meridian-forge-zone-shadow-v1.md`：`cultivation::forging` 经脉淬炼真元记账问题，是完全不同的子系统。
- 已读 `docs/plans-skeleton/plan-bughunt-alchemy-furnace-scope-gate.md`：对炼丹炉做了几乎相同的诊断（`AlchemyFurnace` 操作全程无 `Position`/`CurrentDimension` 校验），但那是 alchemy 模块、不同文件，不构成重复；两者可视为同一 bug class 在不同产出侧的姊妹案例。

## §5 修复计划骨架

### P0 服务端权威门禁

- 比照 alchemy furnace 门禁骨架（`plan-bughunt-alchemy-furnace-scope-gate`）与 `forge-station-place-gate` 修复方向：给 `WeaponForgeStation` 补 dimension 字段（当前只能 Overworld，可先硬编码校验维度 == Overworld）。
- `StartForgeRequest`/`TemperingHit`/`InscriptionScrollSubmit`/`ConsecrationInject`/`StepAdvance` 的 C2S handler（`client_request_handler.rs` 中对应 `handle_forge_*` 函数）改为额外查询玩家 `Position` + `CurrentDimension`。
- 在 `find_owned_forge_station`/`require_owned_active_step` 内新增"与 `station.pos` 同维 + 距离 ≤ 交互半径（对齐 `container_open.rs`/`workbench.rs` 口径）"校验，失败时拒绝并给出 chat 回执，不消耗材料/不推进 session。

### P1 客户端与测试

- ForgeScreen 增补"玩家离站点过远时自动 close + 清空 billetSelection"，避免继续无意义发包。
- server 单测：session 起炉/tempering_hit/inscription_scroll/consecration_inject/step_advance 各补"玩家距离站点过远"、"玩家跨维度"两类拒绝路径的单测，断言不消耗材料、不推进 step_index、不扣真元。

## §6 验证计划

- `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
- `cd client && ./gradlew test build`
- 手工/bot 复现矩阵：近距合法推进、远距各 step 均拒绝、跨维度各 step 均拒绝。

## §7 接入面与守恒说明

- 进料：`ClientRequestV1::ForgeStartSession/TemperingHit/InscriptionScrollSubmit/ConsecrationInject/StepAdvance`、玩家 `Position`/`CurrentDimension`、`WeaponForgeStation`。
- 出料：拒绝态 chat 回执、ForgeScreen 自动 close 信号。
- 跨端契约：C2S payload 字段不必变；服务端新增权威拒绝语义，客户端新增基于最新玩家位置的自动 close。
- qi_physics：开光步骤会真实消耗玩家真元（走既有 ledger），本 finding 本身不改动真元转移逻辑，只新增前置距离/维度门禁，不涉及新常数或新 ledger 流。

## §8 对抗复核结论

- 候选证据：起炉/淬炼/开光/推进四类 handler 与其守卫函数（`find_owned_forge_station`/`require_owned_active_step`）均无 `Position`/`CurrentDimension` 校验；同模块 `artifact_meridian.rs` 确实用了 `Query<&Position>`，证明并非框架限制；established 反例 `container_open.rs` 用 `OPEN_RANGE_BLOCKS=4.0` + `dimension_or_overworld` 校验；客户端 ForgeScreen 非 vanilla container 无自动关闭，`ForgeStartInputHandler` 自身注释承认 station_pos 来自缓存快照而非当前交互上下文。
- 反方质疑：正常客户端是否会主动限制远距发包？是否与 `forge-station-place-gate`/`forge-c2s-session-wiring`/`forge-ui-session-stale`/`meridian-forge-zone-shadow` 重复？
- 修正/反驳：可达性强于典型 dev-only/改包类 finding——完全正常游玩即可触发（起炉后走开/换维度，继续正常按键），不需要伪造 payload；逐一核对 `forge-station-place-gate`（明确只覆盖放置阶段，非会话链路）、`forge-c2s-session-wiring`（已由 #1141 修复为真实分发，不含门禁缺失）、`forge-ui-session-stale`（客户端 store 陈旧问题）、`meridian-forge-zone-shadow`（cultivation::forging 记账问题）均不覆盖本题；`alchemy-furnace-scope-gate` 诊断同类模式但作用于不同模块/文件，非重复。
- 反方最终裁决：通过（`is_real: true`, `reachable: true`, `severity_adjust: unchanged`，保持 high）。无重复，可达性强于同批多数 finding，适合开 Skeleton Plan PR。
