# plan-refactor-c2s-gate-v1 — C2S 请求统一门禁中间件 + client_request_handler 巨石拆分（重构轨 R4）

> 所属总纲：`plan-refactor-master-v1.md`。一句话：给 113 种 C2S 请求建统一的声明式门禁层（距离/维度/所有权/状态前置），同时把 20082 行的 `client_request_handler.rs`（单函数 2438 行、17 个 SystemParam）拆成按域注册的 handler 模块——"只信裸坐标可跨维远程操作"这一整簇（20+ 份 plan）从此在架构上不可能。

## 现状证据（2026-07-27 侦察）

- `network/client_request_handler.rs:522` `handle_client_request_payloads` 单函数跨 522-2960 行，巨型 match 覆盖 `ClientRequestV1` 的 113 个变体；`CombatRequestParams` 一个 SystemParam 30 个字段。
- 门禁分散手写：8 个文件各自定义 `*_MAX_DISTANCE`/`*_RANGE_SQ`（`client_request_handler.rs:468-469`、`craft/workbench.rs:76` Chebyshev、`mineral/probe.rs`、`npc/relic.rs`、`supply_coffin/authority.rs`、`zhenfa/network_array.rs` 等），距离度量都不统一；`CurrentDimension` 在 handler 内被 13 处内联比对，无统一 helper。
- 后果即 bughunt 大簇：跨维开工作台/布阵/夺舍/交易/拾取、无 reach 校验放方块、先扣物品后校验吞丹、无所有权校验拆棺。
- `zone-lookup-overworld-hardcode`：zone 查找硬编码主世界，是维度感知缺失的底层同源。

## 接入面

- **进料**：`bong:client_request` 单通道（既有，C2S 本就单轨）、玩家实体（Position/CurrentDimension/状态组件）、R1 session 忙态、R10 inventory 事务。
- **出料**：校验通过的请求进各域 handler；拒绝走统一 reject 回执（带原因码，client 侧 toast/HUD 可消费——对齐 unconsumed-event-feedback 的方向但只做 gate 拒绝部分）。
- **共享类型**：新 `server/src/network/gate/`——`GateSpec { max_distance(度量统一), same_dimension, ownership, state_preconditions }` 按请求类型声明；维度感知 zone 查找 helper。
- **跨仓库契约**：wire 形状不变（113 变体不动）；reject 回执若新增字段走 R6 的契约流程。

## 阶段

- ⬜ P0 设计收口 + 吸收清单验真：113 个变体普查（每个标注应有的门禁四元组现状）；冻结 `GateSpec` 与拒绝回执语义；等 #1287（冷却重构，同文件大改）merge 定基线。
- ⬜ P1 门禁中间件落地：gate 层上线，先给"已知漏洞簇"的 ~20 个请求类型挂 spec（吸收清单全命中），旧内联校验保留并行断言一个版本期。
- ⬜ P2 巨石拆分批次 A：巨型 match 拆为按域 handler 注册表（combat/production/world/social/npc 五组），行为不变，bot 场景锁住；inventory pickup handler 必须从 ECS `CurrentDimension`、authoritative position/observation range 与 owner/private permission 构造 R10 `PickupAuthorization`，禁止仅凭 client XYZ/instance id 调用 txn，且把 R10 accepted/rejected outcome 交给 R6 emit API。**本批次的 inventory pickup consumer 仅在 R10 P3 pickup/merge txn、R5 P3 attrition API 与 R6 P4 receipt API 均已合入后实施；此前不得以 mock 或旧 R6 P1 schema 接线。**
- ⬜ P3 巨石拆分批次 B + 全量挂 spec + 删旧：113 变体全部声明门禁（含显式 `no_gate` 声明，杜绝静默无门禁）；删除各域内联距离常量与重复维度判断。
- ⬜ P4 bot 验收 + 吸收 plan 批量归档。

## 吸收清单（短名省略 plan-bughunt- 前缀与 -v1 后缀）

active：duoshe-scope-gate、dying-elder-give-dan-server-gate、weapon-repair-station-bypass、workbench-cross-dimension-open、zhenfa-place-scope-gate、coffin-dimension-gate（#1299 已闭环则只核验归档）。
skeleton：alchemy-furnace-scope-gate、block-place-reach-gate、coffin-reclaim-owner-gate、combat-pill-toxin-gate、dropped-loot-g-pickup-range-desync（server 权威范围下发部分；client 对齐归 R6 契约）、forge-station-place-gate、lingtian-c2s-range-gate、dropped-loot-cross-dimension-pickup、player-trade-cross-dimension、tsy-spirit-niche-dimension-gate、workbench-cross-dimension-break、zone-lookup-overworld-hardcode、world-social-cross-dimension-witness-leak（witness 维度判定用本轨 helper）、voidaction-target-zone-lock（zone 锁定判定部分）、disciple-trade-gate-drift、player-trade-npc-gate、npc-trade-gate-desync（门禁与 UI 同步的 server 权威部分）；在飞 #1294：forge-session-range-dimension-gate。

## 文件所有权与边界

- 独占：`network/client_request_handler.rs`（拆解）、新 `network/gate/`、各域内联距离/维度校验行的删除。
- 不碰：`*_emit.rs` S2C 侧（R6）、session 内部（R1）、inventory 事务（R10）。
- 依赖：基线等 #1287 merge；建议在 R6 的 emit 侧稳定后开 P2（同在 network/ 目录，文件不相交但相邻）；P0/P1 可先行。

## bot 验收场景

1. `gate_cross_dimension`：bot 在 TSY 维度对主世界坐标发 workbench/zhenfa/coffin/trade/pickup 请求→全部拒绝且回执带原因；pickup 即使 dropped id 与 XYZ 已知，也必须以 server `CurrentDimension` 拒绝跨维请求。
2. `gate_reach`：超距放方块/开炉/采灵田→拒绝；贴脸→放行。
3. `gate_ownership`：拆他人棺/取他人容器→拒绝。
4. `gate_state_precondition`：给丹先校验后扣（满包/死亡目标不吞丹）；丹毒超阈值禁服。
5. `gate_matrix_sweep`：对 113 变体做参数化扫描（合法/超距/跨维三档），断言与声明的 GateSpec 一致——这是本轨的主回归门。

## 开放问题（pre-P0 收口）

1. 距离度量统一用哪种（欧氏平方 vs Chebyshev）、按交互类别几档半径？需对照 worldview 交互设定拍板。
2. 拒绝回执是复用既有 toast/error payload 还是新增统一 `request_rejected` 类型（涉及 R6 契约新增）？
