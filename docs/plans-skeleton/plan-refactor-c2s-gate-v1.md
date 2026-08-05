# plan-refactor-c2s-gate-v1 — C2S 请求统一门禁中间件 + client_request_handler 巨石拆分（重构轨 R4）

> 所属总纲：`plan-refactor-master-v1.md`。一句话：给 authoritative registry 派生的全部 C2S 请求建立声明式门禁层（距离/维度/所有权/状态前置），并把 20082 行的 `client_request_handler.rs` 拆成按域注册的 handler 模块。

## 现状证据（2026-07-27 侦察）

- `network/client_request_handler.rs:522` `handle_client_request_payloads` 单函数跨 522-2960 行；当前 production Rust match 与 TypeBox union 已知存在 drift，因此 P0 必须分别 inventory 并在 A-CS/R6 完成后以 authoritative registry 派生目标集合，禁止手写 113/116。`CombatRequestParams` 一个 SystemParam 30 个字段。
- 门禁分散手写：8 个文件各自定义 `*_MAX_DISTANCE`/`*_RANGE_SQ`（`client_request_handler.rs:468-469`、`craft/workbench.rs:76` Chebyshev、`mineral/probe.rs`、`npc/relic.rs`、`supply_coffin/authority.rs`、`zhenfa/network_array.rs` 等），距离度量都不统一；`CurrentDimension` 在 handler 内被 13 处内联比对，无统一 helper。
- 后果即 bughunt 大簇：跨维开工作台/布阵/夺舍/交易/拾取、无 reach 校验放方块、先扣物品后校验吞丹、无所有权校验拆棺。
- `zone-lookup-overworld-hardcode`：zone 查找硬编码主世界，是维度感知缺失的底层同源。

## 接入面

- **进料**：`bong:client_request` 单通道（既有，C2S 本就单轨）、玩家实体（Position/CurrentDimension/状态组件）、R1 session 忙态、R10 inventory 事务。
- **出料**：校验通过的请求进各域 handler；拒绝走统一 typed reject 回执（带 request correlation/reason，client store/UI 必须消费——对齐 unconsumed-event-feedback 的方向但只做 gate 拒绝部分）。CraftOpen 中已经解析出合法 `request_id` 的 malformed/业务 admission rejection 必须产 A-08 `CraftOpenRejected { request_id, reason }`，不得静默 drop；envelope decode failure 或缺失/非法 `request_id` 属于不可关联 parse rejection，只记录边界拒绝/metric，不伪造 A-08，且不得改变 session/claim/obligation。
- **共享类型**：新 `server/src/network/gate/`——`GateSpec { max_distance(度量统一), same_dimension, ownership, state_preconditions }` 按请求类型声明；维度感知 zone 查找 helper。
- **跨仓库契约**：wire 形状通常不变（现行 enum 全量）；reject 回执新增字段走 A-CS A-08 + R6 machinery，不由 R4 修改 Agent schema。R1 craft lifecycle 消费 R6 新增的 `CraftOpen`/`CraftStart`/`CraftPause`/`CraftResume`/generation-bound `CraftCancel`，本轨负责 production decode/dispatch，并在 reducer 前对所有 existing-session intent 按 `session_key/generation` 全比较：key/owner mismatch、generation `< current`、`> current`、wrong phase 均 typed reject且 no mutation，只有 `= current` 才进入相应 S-row；CraftStart 仅 matching Running 可进 S-26。Workbench 初次 `CraftOpen` 的 `workbench_key` 只解析当前进程 runtime entity；R4 重验实体/维度/距离/facility 后还必须从 R3 P4 registry取得 stable `placed_id` 并把它交给 R1 建 claim，mapping 缺失/重复/未 hydrate 一律 A-08 reject，禁止把 `Entity::to_bits()` 持久化。

## 阶段

- ⬜ P0 设计收口 + 吸收清单验真：分别普查 production Rust match/decode 的全部可达 request variants 与 TypeBox authoritative registry，记录两集合及对称 drift；冻结 `GateSpec` 与拒绝回执。P3/P4 的 trust-boundary type set 是两集合的 reconciled union；任一 Rust-only live variant 必须先纳入 registry 或从 production decode 删除，不得因 registry 未登记而漏扫。每个 reconciled variant 标注门禁四元组。
- ⬜ P1 门禁中间件落地：gate 层上线，先给已知漏洞簇挂 spec；craft production decode/dispatch 只在 master M-07/M-10 activation 时接入 A-CS A-01..A-04/A-07，A-08 rejection producer 同批 activation，runtime key/generation negatives 属本阶段 owner evidence。contract-first handler declarations 可先合入，但不以未激活 stub 宣称 live。
- ⬜ P2 巨石拆分批次 A：巨型 match 拆为按域 handler 注册表（combat/production/world/social/npc 五组），行为不变，bot 场景锁住；pickup handler 的 authorization/txn/receipt 接线只在 master M-14/M-15 与 R5 ledger artifact 可用后实施，不得以 mock 或旧 schema 接线。
- ⬜ P3 巨石拆分批次 B + 全量挂 spec + 删旧：P0 reconciled union 中全部 production-decodable C2S variant 均声明 `GateSpec` 或显式 `no_gate`；TypeBox-only drift 必须有 production consumer/cutover evidence，Rust-only drift 必须先补 authoritative registry 或删除 decode。删除重复距离/维度判断。
- ⬜ P4 bot 验收 + 吸收 plan 批量归档。

## 吸收清单（短名省略 plan-bughunt- 前缀与 -v1 后缀）

active：duoshe-scope-gate、dying-elder-give-dan-server-gate、weapon-repair-station-bypass、workbench-cross-dimension-open、zhenfa-place-scope-gate、coffin-dimension-gate（#1299 已闭环则只核验归档）。
skeleton：alchemy-furnace-scope-gate、block-place-reach-gate、coffin-reclaim-owner-gate、combat-pill-toxin-gate、dropped-loot-g-pickup-range-desync（server 权威范围下发部分；client 对齐归 R6 契约）、forge-station-place-gate、lingtian-c2s-range-gate、dropped-loot-cross-dimension-pickup、player-trade-cross-dimension、tsy-spirit-niche-dimension-gate、workbench-cross-dimension-break、zone-lookup-overworld-hardcode、world-social-cross-dimension-witness-leak（witness 维度判定用本轨 helper）、voidaction-target-zone-lock（zone 锁定判定部分）、disciple-trade-gate-drift、player-trade-npc-gate、npc-trade-gate-desync（门禁与 UI 同步的 server 权威部分）；在飞 #1294：forge-session-range-dimension-gate。

## 文件所有权与边界

- 独占：`network/client_request_handler.rs`（拆解）、新 `network/gate/`、各域内联距离/维度校验行的删除。
- 不碰：`*_emit.rs` S2C 侧（R6）、session 内部（R1）、inventory 事务（R10）。
- 依赖/order/cutover 只引用 master M-02/M-05/M-07/M-10 与 PR 1902；本 plan 不建立第二套 sequencing。R4 P1 的 runtime rejection pins 在 R4 自有阶段完成，不作为 R6 P1 验收。

## bot 验收场景

1. `gate_cross_dimension`：bot 在 TSY 维度对主世界坐标发 workbench/zhenfa/coffin/trade/pickup 请求→全部拒绝且回执带原因；pickup 即使 dropped id 与 XYZ 已知，也必须以 server `CurrentDimension` 拒绝跨维请求。
2. `gate_reach`：超距放方块/开炉/采灵田→拒绝；贴脸→放行。
3. `gate_ownership`：拆他人棺/取他人容器→拒绝。
4. `gate_state_precondition`：给丹先校验后扣（满包/死亡目标不吞丹）；丹毒超阈值禁服。
5. `gate_matrix_sweep`：从 P0 reconciled union 派生 type set，先断言无未处置 Rust-only/TypeBox-only drift，再对每个 production-decodable variant 执行声明的合法/超距/跨维/no_gate trace；craft 另覆盖 owner/key mismatch、每 intent 的 generation `< /= /> current`、wrong phase、conflicting busy claim、duplicate/replay、runtime-key malformed/stale/despawned/cross-dimension/out-of-range、CraftStart quantity/recipe gate 与合法 S-02 admission。已解析且含合法 `request_id` 的 CraftOpen rejection 必须携 A-08 reason 且 R1 session/claim/obligation 不变；缺失/非法 request_id 或 envelope decode failure 只命中不可关联 parse rejection，禁止伪造 A-08。

## 开放问题（pre-P0 收口）

1. 距离度量统一用哪种（欧氏平方 vs Chebyshev）、按交互类别几档半径？需对照 worldview 交互设定拍板。
2. 通用拒绝复用统一 `request_rejected` envelope；CraftOpen 必须使用 A-08 `CraftOpenRejected` 以供 `OpenPending` matching clear。两者的 TypeBox owner/生成物分别引用对应 Agent artifact，R4 只生产 reason/correlation，R6 负责 wire machinery。
