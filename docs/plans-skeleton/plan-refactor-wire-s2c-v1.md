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
- **跨仓库契约**：proto 形状原则上不动（收编旁路时如需并入 envelope 属破坏性变更，走 buf breaking + samples 同步；agent 侧 TS 只做被动 regenerate，不重构 agent 逻辑）。**不做双轨兼容层**——旁路收编是一次性切换。Craft ownership 与 target 以总纲 §4/§4.1 为准：Agent `A-CS` 批次拥有并先提交 TypeBox source/generated schema/`@bong/schema` dist；R6 只消费该批次记录 SHA 的冻结版本并负责 proto/Rust/client wire plumbing。`CraftOpen.target` 为 required `Handcraft | Workbench { workbench_key }`；`workbench_key` 逻辑/Rust 为 `u64`、protobuf 为 `uint64`、JSON/TypeBox 为无符号十进制字符串，取自成功 S2C `WorkbenchOpen.entity_id` 并由 screen 原样回传；它只在当前进程定位 runtime entity，R4 验证并解析到 R3 stable `placed_id` 后才交 R1 建持久 claim，wire/store/checkpoint 均不得把 `workbench_key` 当 durable identity。`CraftPause`/`CraftResume` 只携带 hydrated `session_key` + `generation`。R6 同时把冻结的 `CraftSessionStateV2` 一次性反映到 proto/Rust converter、`craft_emit`、bridge/router/handler/CraftStore，删除 V1 wire 分支；R7 只消费接口，不在 R6 提前实现 production Resume producer。

## 阶段

- ⬜ P0 设计收口 + 吸收清单验真：28 旁路逐个普查（收编 vs 豁免理由）；100 emit 文件的重复模式取样归纳 builder API；枚举前缀剥离点全量清点；冻结 scope 语义与 join 首包快照集清单；登记 `rotate-footprint-sync`、`bot-inventory-pack-feedback`，并冻结 `dropped_loot_sync` 分片 envelope（`snapshot_revision/page_index/page_count`、`DROPPED_LOOT_SYNC_PAGE_SIZE = 256`）。
- ⬜ P1 emit builder + scope 落地：前置为 Agent 轨已按总纲 §4 提交 craft TypeBox source/generated schema/`@bong/schema` dist；R6 不修改这些 artifacts。builder 上线，vfx/audio/env 挂 scope；跨位面时 env/season 全量重发。把冻结 craft schema 一次性反映为 proto/Rust mirror/converter、samples、client encode/send API 与 producer pins：`CraftOpen.target` 正反 roundtrip `Handcraft` 和 `Workbench { workbench_key }`，覆盖 u64/decimal-string 边界及 response→screen→request key 不变；`CraftPause`/`CraftResume` roundtrip required `session_key` + `generation`。同阶段交付 `CraftSessionStateV2` proto/Rust converter、`craft_emit` producer、bridge/router/handler/CraftStore；R6 pins 覆盖 phase/identity、stale generation 与 malformed/out-of-u64 key，R4 pins 覆盖 stale/despawned/跨维/越距 key 拒绝。R7 的匹配 `Paused`→恰发一次 `CraftResume` 留在 R7 P2。dropped-loot projection/page 子项仅在 R10 P2a owner/visibility metadata provider 与 R3 P4 migration/hydration consumer 均合入后启用：按固定页发送，同一 visibility key 只排序/编码一次，禁止 per-client 无界重建。
- ⬜ P2 client 桥接层收敛：枚举前缀剥离收敛到单点（含 forge-session 修复）；`ServerDataRouter` 注册表整备（分域注册文件，不再单个 1547 行 switch 追加）；dropped-loot store 仅在同 revision 全部分片收齐后原子替换，缺页/混 revision 保留旧视图并等待重发。
- ⬜ P3 旁路归一批次：28 channel 逐批收编入 server_data envelope 或登记豁免（资源包/握手类可豁免）；删除散装 receiver。
- ⬜ P4 契约 pin 全量化：R6 P1 落地三种 craft intent 后，以 116 C2S + 144 S2C 每变体至少一条正反 sample 对拍（此前 baseline 为 113；schema 改动连 sample 一起改）；emit 迁移长尾；inventory receipt 子批次要求 `InventoryEventV1::Moved`（或等价 accepted receipt）携带 request identity、revision、权威 item view，覆盖 schema/sample/convert/emit API、Fabric handler 与 Python decoder。dropped-loot 样本覆盖空/单页/恰 256/257/末页缺失/混 revision。
- ⬜ P5 bot 验收 + 吸收 plan 批量归档。

R10 dropped-loot 契约优先：编码前按 recipient dimension/range/owner 投影，仅同 visibility key 复用；rejected receipt 携带 reason/instance/from/to，并测两 recipient 正反可见性。

## 吸收清单（短名省略 plan-bughunt- 前缀与 -v1 后缀）

active：server-data-s2c-schema-union-drift（TS union 补齐走 regenerate）、spirit-treasure-chat-key-conflict 除外（归 R7）。
skeleton：vfx-audio-dimension-bleed、q-world-season-dimension-env-resync、forge-session-enum-unstripped（#1294 在飞）、client-request-schema-drift（C2S 契约 pin 部分）、cl-ningmai-meridian-target-drop（payload 字段丢失）、alchemy-recipe-fragment-handoff（id 前缀契约）、vfx-event-slash-contract（event_id 格式契约；agent 侧改动最小化）、npc-trade-bundle-count-bridge（展示/结算数量桥）、dropped-loot-g-pickup-range-desync（拾取范围下发对齐部分）、rotate-footprint-sync（R10 typed outcome → 权威 item view）、bot-inventory-pack-feedback（动作级回执）、skillbar-cast-source-drift 与 skillconfig-castsync 除外（归 R9）。
注：server↔agent 方向的桥（anticheat-tiandao-drop、niche-guardian-redis-dispatch、npc-combat-relic-schema-drift、pseudo-vein-agent-deadwire、war-participate-agent-command-drift、天道叙事簇 14 项）**不吸收**——agent 不在本次重构范围，独立保留（见总纲 §6 独立轨）。

## 文件所有权与边界

- 独占：server `network/*_emit.rs` 公共模式与新 `network/emit/`、`schema/proto_convert.rs`；client `network/`（ProtoServerDataBridge、ServerDataRouter、BongNetworkHandler 的 channel 注册区段）。
- 不碰：`BongNetworkHandler.clearClientStateOnDisconnect` 区段（R2 域，同文件分区段，merge 前互相 fetch）；`client_request_handler.rs`（R4）；各 emit 的业务语义。
- 依赖：Agent `A-CS`（`plan-agent-craft-schema-v1`）先合并并冻结 TypeBox source/schema/dist；R2 先合（同文件低冲突区段）。R1 craft adapter 不得在本轨 P1 的 craft 契约 pins 合入前宣称 pause/resume 可达；R4 handler/gate 随后落地。dropped-loot projection/page 子项另硬依赖 R10 P2a owner/visibility provider 与 R3 P4 migration/hydration consumer。

## bot 验收场景

1. `wire_scope_dimension`：主世界 bot + TSY bot 双开，主世界触发 vfx/audio→断言 TSY bot 收不到（P6 protobuf 深断言配合）。
2. `wire_dimension_transfer_resync`：bot 跨位面→断言 env/season/zone 大气全量重发。
3. `wire_contract_sweep`：对 144 S2C 变体的 sample 对拍在 CI 常绿（配 proto-breaking 深检，联动 V 轨）；`CraftSessionStateV2` 另有 phase 全变体、identity/generation required/forbidden 组合、stale generation 拒绝、proto→bridge→handler→CraftStore roundtrip pins。
4. `wire_join_snapshot`：重连首包快照集完整（与 R2 的 `reconnect_state_freshness` 同场景）；checkpointed craft guarded restore 后必须包含单个权威 `CraftSessionStateV2` hydration，server `Paused`/identity/generation 与 client store 对拍，idle/terminal/delivery-pending 不得被伪装成可 Resume。

## 开放问题（pre-P0 收口）

1. 28 旁路的收编/豁免分界（低频大 payload 如资源包显然豁免；npc bubble 这类高频小包是否值得并入 envelope）。
2. join 首包快照集的权威清单放哪维护（emit builder 注册时声明 `replay_on_join` 标志？）。
3. R7 production `CraftResume` producer 的验收证明不在 R6 P1 提前实现；由 R7 P2 在 R6 wire/bridge、R4 handler/gate 与 R2 store 前置合入后完成，依据总纲 `plan-refactor-master-v1.md §3 Wave 2`。R6 P1 只关闭 identity/phase/schema/bridge/store 的负向契约，避免复制未来 producer。
4. `CraftSessionStateV2.generation` 的 numeric boundary pins（`0`、`u64::MAX`、`MAX→MAX` overflow refusal、off-by-one）本轮只冻结 wire 类型与基本 roundtrip，不扩展 P1 实施；延期到总纲 `plan-refactor-master-v1.md §3 Wave 2` 的 R6/R7 接缝验收窗口，理由是边界策略需与 producer/store generation-CAS 一起收口。
