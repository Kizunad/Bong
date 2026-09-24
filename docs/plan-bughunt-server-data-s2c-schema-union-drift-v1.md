# plan-bughunt-server-data-s2c-schema-union-drift-v1

状态：Skeleton Plan
分区：BugHunt worker / agent-schema / 20260708 r01

## 1. 一句话结论

`@bong/schema` 的 `ServerDataType` / `ServerDataV1` / generated `server-data-v1.json` 漏掉一批 Rust/Proto/Java 已正式注册并消费的 `bong:server_data` S2C payload，导致 agent-schema 的 source-of-truth smoke 对玩家可见 IPC 契约假绿。

## 2. 实际游玩体验影响

这不是 production Fabric client 立即丢包：当前主路径仍由 Rust proto oneof 发包，Java `ProtoServerDataBridge` 转 legacy JSON，再交给 `ServerDataRouter` handler 消费。

真正影响是契约门禁假绿。手搓进度/出炉、物资棺搜刮、功法熟练度/丹药状态、暗器/毒蛊/震脉 HUD、出生引导棺坐标等玩家可见玩法，已经在 server 和 client 正式走 `bong:server_data`；但 `@bong/schema` 的 `ServerDataV1` union 与 `server-data-v1.json` 不承认这些 payload。后续字段改名、可选性漂移或枚举扩展时，schema smoke / generated freshness / 外部 JSON Schema 消费方都可能继续绿色，直到玩家看到制作 UI、搜刮 UI、战斗 HUD 或引导提示异常，排查时还会被“schema 契约已过”的结果误导。

## 3. 复现路径

1. 查看 `agent/packages/schema/src/server-data.ts:167-266`：`ServerDataType` 的字面量列表没有 `craft_session_state`、`craft_outcome`、`loot_container_open`、`technique_proficiency_update`、`pill_buff_status`、`anqi_hud`、`dugu_v2_skill_cast`、`zhenmai_hud`、`tutorial_coffin_pos` 等正式 S2C type。
2. 查看 `agent/packages/schema/src/server-data.ts:1806-1912`：`ServerDataV1` union 同样没有这些 wrapper。
3. 查看 `agent/packages/schema/src/schema-registry.ts:566` 与 `:1018`：`server-data-v1.json` 直接由 `SCHEMA_REGISTRY.serverDataV1` 生成；因此 generated JSON Schema 继承同一缺口。
4. 直接 grep `agent/packages/schema/generated/server-data-v1.json`：找不到上述 type 字面量。
5. 对照 Rust/Proto/Java：这些 type 已经不是未来设计项，而是正式生产/消费的 `bong:server_data` payload。

## 4. 根因证据

- Rust `ServerDataType` 已包含 craft、loot、combat HUD、tutorial 等变体：`server/src/schema/server_data.rs:244-287`。
- Rust `ServerDataPayloadV1` 已包含对应 payload：`server/src/schema/server_data.rs:532-598`。
- Proto oneof 已包含代表性 payload：`proto/bong/envelope.proto:38-71` 覆盖 craft / technique / pill，`:142-168` 覆盖 loot / anqi / dugu / zhenmai / tutorial。
- Java proto bridge 已把这些 oneof case 映射回 legacy type string：`client/src/main/java/com/bong/client/network/ProtoServerDataBridge.java:71-188`。
- Java server_data router 已注册对应 handler：`client/src/main/java/com/bong/client/network/ServerDataRouter.java:162-167`、`:235-248`、`:252-265`。
- `scripts/smoke-law-engine.sh:126-128` 明确把 `agent/packages/schema` 当作 `Schema smoke (@bong/schema source-of-truth)` 执行 build/test。
- `agent/packages/schema/tests/schema.test.ts:786-835` 以及后续大量样本用 `validate(ServerDataV1, data)` 验证 server-data sample；`:798-809` 还明确要求 TS `ServerDataV1` 对齐 Rust/Java envelope budget。
- `agent/packages/schema/src/generated-artifacts.ts:42-49` 的 freshness snapshot 只渲染 `GENERATED_SCHEMA_FILES`，不会发现 `ServerDataV1` union 本身漏掉已存在的 production S2C type。

## 5. 不重复性

- 不重复 #1054：该题是离屏战果/遗物 shared schema 漂移，本题是 `bong:server_data` S2C union 漂移。
- 不重复 #1059：该题是 bot combat server_data 类型断言假阳性，本题是 `@bong/schema` / generated `server-data-v1.json` 不覆盖正式 S2C type。
- 不重复 #1061：该题是 schema 生成物覆盖假绿，本题锚定 `ServerDataV1` union 的 production S2C type-set 漂移。
- 不重复 #1116：该题是暗器 HUD agent schema 漂移，本题覆盖更底层的 `bong:server_data` S2C union，不讨论暗器 HUD 单项字段语义。
- 不重复 `docs/plans-skeleton/plan-bughunt-agent-schema-generated-registry-gap-v1.md`：该 plan 是 server -> agent Redis V1 payload 没进 registry/generated；本 plan 是 server -> Fabric client `bong:server_data` S2C payload 没进 `ServerDataType` / `ServerDataV1` / generated `server-data-v1.json`。
- 不重复 `docs/plans-skeleton/plan-bughunt-client-request-schema-drift.md`：该题是 C2S `ClientRequestV1` union 漂移，本题是 S2C `ServerDataV1` union 漂移。

## 6. 修复计划骨架

1. 新增 type-set parity guard：从 Rust/Proto/Java 已正式注册且 Java router 已消费的 `bong:server_data` type 集合，与 TS `ServerDataType` / `ServerDataV1` 覆盖集合对拍。
2. 允许显式豁免表，但豁免必须写明原因；例如已迁出 `bong:server_data` 专属 channel 的 payload，不能被误算为缺口。
3. 先补代表性高价值 payload 的 TypeBox wrapper、sample 与 `ServerDataV1` union：`craft_session_state`、`craft_outcome`、`loot_container_open`、`technique_proficiency_update`、`pill_buff_status`、`anqi_hud`、`dugu_v2_skill_cast`、`zhenmai_hud`、`tutorial_coffin_pos`。
4. 根据 parity guard 输出继续收敛相邻缺口；不要一次性承诺补完整个 S2C schema 宇宙，也不要把没有生产路径的历史残留强行纳入。
5. 重建 `@bong/schema` dist/generated，确保 `agent/packages/schema/generated/server-data-v1.json` 和必要的单项 schema 文件能看到新增 type。
6. 作为 master M-16 的 Agent production batch，补齐 inventory accepted/rejected action receipt 的 TypeBox wrapper、正反 sample、`ServerDataV1`/`SCHEMA_REGISTRY` membership、单项 generated schema 与 committed dist；accepted 固定 request identity/result revision/instance/from/to/authoritative item view，rejected 固定 request correlation/reason/instance/from/to。记录冻结 SHA、registry-derived membership、freshness 与 runtime import evidence供 R6 P4 消费；本 plan 不改 Rust/proto/Java。

## 7. 验证计划

1. 修复前新增 parity 测试应失败，并指出上述 production S2C type 缺席 TS `ServerDataV1` 覆盖。
2. 修复后运行：`cd agent/packages/schema && npm test`。
3. 修复后运行：`cd agent && npm run build -w @bong/schema`，确保 dist export 与 generated schema 同步。
4. 修复后运行：`bash scripts/smoke-law-engine.sh`，确认 source-of-truth smoke 不再假绿。
5. 抽查 `agent/packages/schema/generated/server-data-v1.json` 包含代表性新增 type；删除任一新增 schema/union entry 时 freshness 或 parity gate 应失败。
6. M-16 receipt 对 accepted/rejected 各执行 required-field、wrong-type、missing correlation、authoritative item view/理由边界正反样本；source→union→registry→generated→dist→clean runtime import 任一层删除都必须撞红，并在 handoff 记录冻结 SHA。

## 8. 对抗复核记录

第一轮反驳：production Fabric client 主路径不依赖 TS `ServerDataV1`，候选不能写成“线上客户端立刻丢包”；Rust/Proto/Java 已有更强 runtime 契约，最多是中风险契约/生成物覆盖缺口。

回应：接受降级。证据改为 `@bong/schema` source-of-truth smoke 与 generated `server-data-v1.json` 假绿，不再声称 Java client 当前拒包。

第二轮反驳：候选和已有 generated-registry gap 形似，且 payload 横跨 craft/loot/combat/tutorial，范围容易失控；实际游玩体验影响必须落在未来漂移与排障误导，而不是当前功能失效。

最终结论：PASS，medium confidence / medium risk。提交 skeleton plan，但标题和正文必须收窄为 `bong:server_data` S2C union drift；修复计划以 type-set parity guard 和代表性样本为边界，避免与 server -> agent Redis generated-registry gap 重复。
