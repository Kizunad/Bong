# plan-refactor-wire-s2c-v1 — S2C Wire 层统一：emit builder + client 双轨归一 + 作用域广播（重构轨 R6）

> 所属总纲：`plan-refactor-master-v1.md`。一句话：server 侧 ~100 个 `*_emit.rs` 收敛到共享 emit builder（带维度/zone 作用域），client 侧把 28 个散装 CustomPayload 旁路归入 `ServerDataRouter` 单轨、枚举前缀剥离收敛到桥接层一处——契约漂移与跨维串场两簇从结构上封死。

## 现状证据（2026-07-27 侦察）

- server：144 个 `ServerDataType` 变体 × ~100 个独立 emit 文件，各自"读 ECS→建 DTO→序列化→EventWriter"无共享 builder；`schema/proto_gen.rs` 12423 行（生成）+ `proto_convert.rs` 8419 行（手写）并存，漏改一处即 drift。
- client：主干 `ServerDataRouter`（~90 handler 手写注册，`ServerDataRouter.java:111-289`）之外，`BongNetworkHandler.register()` 还有 ~28 个独立 channel 各写各的 receiver+parse（npc metadata/bubble、vfx_event、audio play/stop、agent_ui、era_ambiance 等）。
- `ProtoServerDataBridge.java`（1547 行）大 switch 手转 JSON，proto3 枚举前缀在 cast_sync/movement_state/container_state 等处各剥一遍（`:846-999`）——#1294 的 forge-session-enum-unstripped 就是又漏了一处的实证。
- 广播作用域缺失：vfx/audio 跨维 bleed、跨位面 env 不重发（q-world-season-dimension-env-resync）。

## 接入面

- **进料**：各域游戏事件（emit 调用点）、`world` 维度/zone 信息（作用域过滤）。
- **出料**：`bong:server_data` 单通道（目标态：28 旁路全部收编或显式豁免登记）；join/重连首包快照集契约（R2 清干净后靠它灌满）。
- **共享类型**：新 server `network/emit/` builder（`scope: Global | Dimension | Zone | Player`）；client 桥接层唯一的枚举前缀剥离函数。
- **跨仓库契约**：proto 形状原则上不动；破坏性变化走 buf/samples gate，不做双轨兼容。craft domain shape 只消费 A-CS A-01..A-06 的冻结 SHA；R6 拥有 proto/Rust/client generation machinery、converter、encode/send API、bridge/router 与 emit API。contract-first artifacts 可先合入但不得宣称 live；真实 producer/consumer 切换只按 master §3/§4.1 与 PR 1902 atomic activation 执行。`workbench_key` 仅为当前进程 runtime locator，wire/store/checkpoint 不把它当 durable identity。

## 阶段

- ⬜ P0 设计收口 + 吸收清单验真：28 旁路逐个普查（收编 vs 豁免理由）；100 emit 文件的重复模式取样归纳 builder API；枚举前缀剥离点全量清点；冻结 scope 语义与 join 首包快照集清单；登记 `rotate-footprint-sync`、`bot-inventory-pack-feedback`，并冻结 `dropped_loot_sync` 分片 envelope（`snapshot_revision/page_index/page_count`、`DROPPED_LOOT_SYNC_PAGE_SIZE = 256`）。
- ⬜ P1 emit builder + scope、craft machinery contracts：builder 上线，vfx/audio/env 挂 scope；跨位面时 env/season 重发。对 A-CS A-01..A-06 的冻结 SHA 实现 proto/Rust mirrors、converter、samples、client encode/send API、bridge/router/store contract pins 和 `craft_emit` **API/stub**。P1 acceptance 只覆盖 R6 自有边界：两种 target、u64 decimal-string、Pause/Resume identity、Cancel、WorkbenchOpen、StateV2 phase/generation 的 wire roundtrip 与 malformed reject；不要求尚未允许存在的 R1 state producer、R4 gate 或 R7 intent producer，不宣称 production reachable。dropped-loot projection/page 只在 master M-14 artifact gate 后启用。
- ⬜ P2 client 桥接层收敛：枚举前缀剥离收敛到单点（含 forge-session 修复）；`ServerDataRouter` 注册表整备（分域注册文件，不再单个 1547 行 switch 追加）；dropped-loot store 仅在同 revision 全部分片收齐后原子替换，缺页/混 revision 保留旧视图并等待重发。
- ⬜ P3 旁路归一批次：28 channel 逐批收编入 server_data envelope 或登记豁免（资源包/握手类可豁免）；删除散装 receiver。
- ⬜ P4 契约 pin 全量化：从当时 authoritative registries 派生 C2S/S2C type sets，对每个实际变体至少一条正反 sample；禁止手写 116/144 充当 freshness 证据。emit 迁移长尾；inventory receipt 与 dropped-loot 样本沿用本 plan 既有 contract。
- ⬜ P5 bot 验收 + 吸收 plan 批量归档。

R10 dropped-loot 契约优先：编码前按 recipient dimension/range/owner 投影，仅同 visibility key 复用；rejected receipt 携带 reason/instance/from/to，并测两 recipient 正反可见性。

## 吸收清单（短名省略 plan-bughunt- 前缀与 -v1 后缀）

active：server-data-s2c-schema-union-drift（TS union 补齐走 regenerate）、spirit-treasure-chat-key-conflict 除外（归 R7）。
skeleton：vfx-audio-dimension-bleed、q-world-season-dimension-env-resync、forge-session-enum-unstripped（#1294 在飞）、client-request-schema-drift（C2S 契约 pin 部分）、cl-ningmai-meridian-target-drop（payload 字段丢失）、alchemy-recipe-fragment-handoff（id 前缀契约）、vfx-event-slash-contract（event_id 格式契约；agent 侧改动最小化）、npc-trade-bundle-count-bridge（展示/结算数量桥）、dropped-loot-g-pickup-range-desync（拾取范围下发对齐部分）、rotate-footprint-sync（R10 typed outcome → 权威 item view）、bot-inventory-pack-feedback（动作级回执）、skillbar-cast-source-drift 与 skillconfig-castsync 除外（归 R9）。
注：server↔agent 方向的桥（anticheat-tiandao-drop、niche-guardian-redis-dispatch、npc-combat-relic-schema-drift、pseudo-vein-agent-deadwire、war-participate-agent-command-drift、天道叙事簇 14 项）**不吸收**——agent 不在本次重构范围，独立保留（见总纲 §6 独立轨）。

## 文件所有权与边界

- 独占：server `network/*_emit.rs` 公共模式与新 `network/emit/`、`schema/proto_convert.rs`；client `network/`（ProtoServerDataBridge、ServerDataRouter、BongNetworkHandler 的 channel 注册区段）。
- 不碰：`BongNetworkHandler.clearClientStateOnDisconnect` 区段（R2 域，同文件分区段，merge 前互相 fetch）；`client_request_handler.rs`（R4）；各 emit 的业务语义。
- 依赖与 production activation 只引用 master §3/§4.1 及 PR 1902。R6 P1 可在 A-CS A-01..A-06 冻结后交付 machinery contracts；R1 state producer、R4 runtime rejection、R7 intent producer 的 production evidence 分别留给其 owner phase，不构成 R6 P1 completion。dropped-loot projection/page 同理消费 master 列出的真实 provider。

## bot 验收场景

1. `wire_scope_dimension`：主世界 bot + TSY bot 双开，主世界触发 vfx/audio→断言 TSY bot 收不到（P6 protobuf 深断言配合）。
2. `wire_dimension_transfer_resync`：bot 跨位面→断言 env/season/zone 大气全量重发。
3. `wire_contract_sweep`：对 registry-derived S2C type set 做 sample 对拍；A-06 另覆盖 phase/identity/generation wire roundtrip。runtime stale/rejection 与 producer behavior 引用 owner trace，不在本场景伪造。
4. `wire_join_snapshot`：重连首包快照集完整；craft hydration 只在 master activation row 完成后接入，并引用 R1 S-10/S-16。

## 开放问题（pre-P0 收口）

1. 28 旁路的收编/豁免分界（低频大 payload 如资源包显然豁免；npc bubble 这类高频小包是否值得并入 envelope）。
2. join 首包快照集的权威清单放哪维护（emit builder 注册时声明 `replay_on_join` 标志？）。
3. production `CraftResume`、runtime-key rejection 与 real `craft_emit` activation 不在 R6 P1 验收；分别由 R7/R4/R1 owner phase 按 master atomic cutover row 提供。R6 只验 converter/API/stub contract。
4. `CraftSessionStateV2.generation` 的 `0`、`u64::MAX`、overflow refusal 属 R6 wire pin；producer/store CAS behavior 属其 owner activation trace，不混入上游 phase。
