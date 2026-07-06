# BugHunt: ClientRequest 共享 schema 漂移导致协议 gate 假绿

> 状态：Skeleton Plan，仅记录本轮 BugHunt 发现。不要消费，不要归档。

## Bug 摘要

Rust/Java 已上线的 `bong:client_request` 主路径 C2S 请求，和 TypeScript `@bong/schema` 导出的 `ClientRequestV1` union 已明显漂移。Rust 与 Java 都声明该协议应和 `agent/packages/schema/src/client-request.ts` 1:1 对齐，但 TS union 没有收录多条真实玩家会发送、服务端也已经处理的请求。

本 plan 不覆盖已知重复项：

- 不重复 #974：`alchemy_learn_recipe_fragment` 丹方残卷链路。
- 不重复 #988：`give_dan_to_elder` 垂死大能默认 G 输入链路。

独立缺口包括：

- `craft_start` / `craft_cancel`
- `external_container_move` / `external_container_close`
- `container_open` / `workbench_open` / `supply_coffin_open`
- `coffin_break` / `coffin_menu_reclaim`
- `lingtian_start_till` / `lingtian_start_renew` / `lingtian_start_planting` / `lingtian_start_harvest` / `lingtian_start_drain_qi`
- `qi_scatter_bead_use`
- `jiemai`

## 对实际游玩体验的影响

这不是“当前 Java 客户端按按钮一定被 Rust 服务端拒收”的单点 runtime bug；Java → Rust 主链上很多请求仍能被服务端处理。真正影响是共享协议契约已经失真：schema gate、agent schema registry、bot/e2e 契约测试无法验证这些真实玩家主路径 C2S。

玩家实际会通过制作、世界容器、制作台、物资棺、灵田、截脉、散灵珠等功能触发这些请求。如果任一字段继续漂移，例如 `quantity`、`session_id`、`entity_id`、`InventoryLocationV1` 形状变化，现有 `@bong/schema` 和 e2e gate 仍会显示绿色，因为这些请求根本不在 `ClientRequestV1` union 内。结果是玩家主路径可能在下一次协议改动时直接断裂，而 CI/e2e 给出假安全信号。

## 证据定位

- `server/src/schema/client_request.rs:3`：Rust 注释声明与 TypeScript `agent/packages/schema/src/client-request.ts` 1:1。
- `client/src/main/java/com/bong/client/network/ClientRequestProtocol.java:15`：Java 注释声明与 Rust schema 和 TS schema 1:1 对齐。
- `agent/packages/schema/src/client-request.ts:1131`：TS `ClientRequestV1 = Type.Union([...])` 组装处缺少上述请求。
- `agent/packages/schema/src/schema-registry.ts:634`：`clientRequestV1` 被导出到 schema registry，作为共享 IPC 契约。
- `agent/packages/schema/tests/schema.test.ts:1727`、`2130`、`2469`：schema gate 使用 `validate(ClientRequestV1, data)` 校验 client-request 样本，但缺失请求没有样本覆盖。
- `client/src/main/java/com/bong/client/network/ClientRequestSender.java:514`、`548`：真实客户端 sender 会 dispatch `craft_start`、`external_container_move` 等缺失请求。
- `client/src/main/java/com/bong/client/network/ClientRequestProtocol.java:1291`、`1336`、`1343`、`1352`、`1441`：真实 Java encoder 生成 `craft_start`、`supply_coffin_open`、`container_open`、`workbench_open`、`external_container_move`。
- `server/src/network/client_request_handler.rs:2267`、`2301`、`2333`、`2365`、`2401`、`2608`：Rust handler 已处理物资棺、世界容器、制作台、外部容器、灵田、手搓制作等请求。
- `scripts/bot/bot.py:232`：bot e2e 的 `intent()` 按 Rust schema 手写 JSON，不依赖 TS `ClientRequestV1`，因此现有 bot e2e 不会暴露 TS schema 漂移。

本地枚举差异命令输出显示，排除 #974/#988 后仍有独立缺失项：

```text
coffin_break
coffin_menu_reclaim
container_open
craft_cancel
craft_start
external_container_close
external_container_move
jiemai
lingtian_start_drain_qi
lingtian_start_harvest
lingtian_start_planting
lingtian_start_renew
lingtian_start_till
qi_scatter_bead_use
supply_coffin_open
workbench_open
```

## 触发路径

1. 玩家在 Fabric 客户端触发真实玩法入口：
   - 背包手搓页点击开始或取消制作；
   - 右键世界容器、制作台或物资棺；
   - 外部容器 UI 内移动物品或关闭；
   - 灵田开垦、翻新、种植、收获、偷灵；
   - 战斗中触发截脉；
   - 使用散灵珠。
2. Java 端通过 `ClientRequestProtocol` 编码对应 `type`，再由 `ClientRequestSender` dispatch 到 `bong:client_request`。
3. Rust server 端 `ClientRequestV1` 能反序列化并进入 handler。
4. 但 TS `@bong/schema` 的 `ClientRequestV1` 不认识这些 `type`，所以 schema registry、generated JSON schema、schema sample 测试和任何依赖 TS schema 的 agent/e2e gate 都不会验证这些主路径。

## 反方审查记录

第一轮反方质疑：

- 这是否只是文档或非生产 schema，不影响真实玩家。
- 是否重复 #974/#988。
- TS 是否另有模块合并进 `ClientRequestV1`。

第一轮结论：PASS，但必须收窄口径。不能写成 Java → Rust runtime 必然失败；应写成 TS `ClientRequestV1` union 与 Rust/Java 已上线请求漂移，导致 schema gate 和 agent schema registry 无法验证真实主路径。#974 的 `alchemy_learn_recipe_fragment` 与 #988 的 `give_dan_to_elder` 必须排除。

第二轮反方质疑：

- “契约假绿”是否只是测试债，不足以算真实体验风险。
- 是否应该改成单个请求缺失，而不是系统性漂移。
- Rust/proto 是否已经有另一个 source of truth，TS 本来不需要覆盖全部 C2S。

第二轮结论：PASS。Rust/proto exhaustive guard 只保护 Rust/proto 编码，不生成也不校验 TS TypeBox；`@bong/schema` 仍导出 `clientRequestV1` 作为共享 IPC 契约。plan 应用 `craft_start`、`external_container_move`、`container_open`、`workbench_open` 等具体主路径作为抓手，系统性漂移作为根因。

## Skeleton Fix Plan

1. 在 `agent/packages/schema/src/client-request.ts` 补齐缺失的 C2S TypeBox object，并加入 `ClientRequestV1` union。
2. 已有独立 schema 可复用时优先复用字段定义，但 wire shape 必须对齐 Rust `ClientRequestV1`：
   - `craft_start` 是 `{ v, type, recipe_id, quantity? }`，不是 `craft.ts` 里带 `player_id` / `ts` 的 agent IPC shape。
   - `external_container_move` 必须复用或镜像 `InventoryLocationV1`。
   - `lingtian_start_*` 的 `mode` / `source` 约束需要与 Rust parser 行为一致。
3. 为每个缺失请求新增正样本；关键字段新增负样本：
   - `craft_start`：缺 `recipe_id`、`quantity=0`、`quantity>MAX_CRAFT_QUANTITY`。
   - `external_container_move`：缺 `session_id`、非法 `from/to`。
   - `container_open` / `workbench_open` / `supply_coffin_open`：非法 `entity_id`。
   - `jiemai`：额外字段拒绝。
4. 更新 `agent/packages/schema/generated/client-request-v1.json` 等生成物。
5. 增加 drift guard：从 Rust enum 或 proto generated C2S type 列表对拍 TS `ClientRequestV1` 的 `type` 字面量，至少覆盖“Rust 有、TS 无”红线。
6. 在 bot/e2e 中选 2-3 条真实主路径补 smoke：
   - `craft_start` happy path + `craft_cancel`。
   - `container_open` 或 `workbench_open` entity-based open。
   - `external_container_move` 基于 session 的移动。

## 验收测试计划

- `cd agent && npm run build -w @bong/schema`
- `cd agent/packages/schema && npm test`
- 仓库根执行：`export BONG_SKIP_SKIN_PREFETCH=1 && bash scripts/smoke-test-e2e.sh`
- 若补 bot 场景，再在仓库根执行：`export BONG_SKIP_SKIN_PREFETCH=1 && bash scripts/smoke-test.sh`
- 增加一个自动对拍测试：Rust/Java 已声明的 C2S `type` 在 TS `ClientRequestV1` union 中必须存在；排除清单只允许有明确注释和过期时间。

## 风险

- `craft.ts` 里的 `CraftStartReqV1` 不是 `bong:client_request` wire shape，直接纳入 union 会误收/误拒，需要单独建 `CraftStartRequestV1`。
- TS schema 补齐后，既有样本或 agent 调用可能暴露更多旧字段漂移；应按请求族分批提交，避免一次性大改难定位。
- 生成物漏更新会让 `agent/packages/tiandao` 继续引用旧 dist/generate 结果。
- e2e 新增真实玩法路径可能需要稳定 fixture，避免把世界生成或库存准备的不稳定性误判成协议失败。
