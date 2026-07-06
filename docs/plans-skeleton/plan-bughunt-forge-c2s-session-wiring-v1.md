# plan-bughunt-forge-c2s-session-wiring-v1（骨架）

> **骨架（BugHunt G4 / e2e-protocol r04）**。一句话主题：已暴露的炼器/锻炉 UI 与 `ClientRequestV1` 契约没有补齐起炉、图谱翻页/学习、步骤推进 C2S 主链，玩家能获得/放置炼器砧并打开锻炉界面，但自然操作无法进入完整炼器闭环。

## Bug 摘要

玩家可获得炼器砧、放置炼器砧并打开锻炉界面；协议层也已经声明 `forge_start_session`、`forge_step_advance`、`forge_blueprint_turn_page`、`forge_learn_blueprint`。但当前实现只接了站点放置和三类步骤内动作，起炉/翻页/学习/推进链路没有完整 client -> server -> Forge system 闭环：

- server `ClientRequestV1` 声明 Forge 起炉、淬炼、铭文、开光、推进、图谱翻页、学习图谱请求（`server/src/schema/client_request.rs:603`）。
- server handler 已处理 `ForgeStepAdvance` 和 `ForgeLearnBlueprint`，但 `ForgeStartSession` 与 `ForgeBlueprintTurnPage` 明确落入 `plan-forge-v1 client_request not yet wired` 分支（`server/src/network/client_request_handler.rs:2579`、`:2600`）。
- client `ClientRequestSender` 只暴露 `sendForgeStationPlace`、`sendForgeTemperingHit`、`sendForgeInscriptionScroll`、`sendForgeConsecrationInject`，没有 `forge_start_session`、`forge_step_advance`、`forge_blueprint_turn_page`、`forge_learn_blueprint` 发送入口（`client/src/main/java/com/bong/client/network/ClientRequestSender.java:162`、`:275`）。
- ForgeScreen 图谱左右键只调用本地 `BlueprintScrollStore.turn()`，不发 C2S（`client/src/main/java/com/bong/client/forge/ForgeScreen.java:234`）。

## 对实际游玩体验的影响

炼器不是当前 100h 路线硬门槛，但它已作为可选生产路线对玩家暴露：锻炉 UI 注册在 `U` 键（`client/src/main/java/com/bong/client/forge/ForgeScreenBootstrap.java:19`），界面展示砧、会话、图谱和结果（`client/src/main/java/com/bong/client/forge/ForgeScreen.java:58`），服务端也有炼器砧物品/配方入口。玩家会自然尝试放砧、开锻炉、选图谱、开始打造，但实际会停在 UI 空壳或步骤死胡同：

- 无自然入口发起 `StartForgeRequest`，因此正常玩家无法从锻炉 UI 创建武器炼器 session。
- 即便通过测试/dev 状态获得 session，步骤完成也没有客户端入口发送 `forge_step_advance`，无法结算到 `ForgeOutcome`，成品不会入包。
- 图谱翻页只改本地 store，不改变服务端状态；图谱学习虽然 server 有 handler，但 client 没有入口，残卷学习/书本刷新链路断裂。
- 现有 bot/e2e 没覆盖该链路，只覆盖到 Forge station 或组件级 JSON，因此会假绿。

## 证据定位

- `server/src/schema/client_request.rs:603`：`ForgeStartSession` / `ForgeTemperingHit` / `ForgeInscriptionScroll` / `ForgeConsecrationInject` / `ForgeStepAdvance` / `ForgeBlueprintTurnPage` / `ForgeLearnBlueprint` 均已进入 C2S schema。
- `server/src/network/client_request_handler.rs:2579`：`ForgeStepAdvance` 会转发到 `handle_forge_step_advance`。
- `server/src/network/client_request_handler.rs:2587`：`ForgeLearnBlueprint` 会转发到 `handle_forge_learn_blueprint`。
- `server/src/network/client_request_handler.rs:2600`：`ForgeStartSession` 与 `ForgeBlueprintTurnPage` 被归入 not-yet-wired 分支。
- `server/src/forge/events.rs:11`：真实起炉事件是 `StartForgeRequest`。
- `server/src/forge/mod.rs:141`：`handle_start_forge_requests` 消费 `StartForgeRequest` 创建 session。
- `server/src/forge/mod.rs:583` 与 `server/src/forge/mod.rs:686`：`StepAdvance` 是步骤结算、推进、最终产出 `ForgeOutcome` 的收束点。
- `client/src/main/java/com/bong/client/network/ClientRequestSender.java:162`、`:275`：client sender 只覆盖放砧和步骤内三类动作。
- `client/src/main/java/com/bong/client/forge/ForgeScreen.java:226`：ForgeScreen 只处理淬炼键、关闭、左右翻页；没有起炉/推进/学习图谱发包。
- `scripts/bot/`、`scripts/smoke-test-e2e.sh`、`scripts/bot-e2e.sh` 检索未发现 `forge_start_session` / `forge_step_advance` 覆盖。

## 触发路径

1. 玩家通过配方或测试道具获得炼器砧并放置。
2. 玩家按 `U` 打开锻炉 UI，看到砧、图谱、会话/结果区域。
3. 玩家尝试选图谱、学习图谱、起炉或完成当前步骤。
4. 客户端没有对应 C2S 发送入口，或 server 对请求直接 not-yet-wired；服务端不会创建/推进 session。
5. 炼器流程无法自然到达 `ForgeOutcome`，背包不会收到带 forge runtime 字段的成品。

## 反方审查记录

- 第 1 轮反方结论：保留。未找到替代起炉路径、自动 step advance、bot/e2e 覆盖或开放 PR 覆盖；确认 `StartForgeRequest` 无生产侧 writer，`StepAdvance` 无自动系统生产。
- 第 2 轮反方结论：保留但收窄。不要表述为全服主线必断，应表述为“已暴露的炼器/器修可选生产路线进入死胡同”。#911 是 forge step_state 契约漂移，#877 是 processing deadpath，#982 只覆盖 `forge_station_place`，均不重复本问题。

## Skeleton Fix Plan

### P0 — 补齐 server C2S 分发

- 为 `ForgeStartSession` 实现 handler：解析/校验 `station_id`、`blueprint_id`、`materials`，定位玩家附近/拥有权限的 `WeaponForgeStation`，发 `StartForgeRequest`。
- 为 `ForgeBlueprintTurnPage` 实现 handler：更新玩家 `LearnedBlueprints` 当前页/索引，并回推 `forge_blueprint_book`。
- `ForgeLearnBlueprint` 成功消耗残卷并写入 `LearnedBlueprints` 后，回推更新后的 `forge_blueprint_book`，避免 client 继续看旧书。

### P1 — 补齐 client 编码与 UI 发包

- 在 `ClientRequestProtocol` / `ClientRequestSender` 添加 `forge_start_session`、`forge_step_advance`、`forge_blueprint_turn_page`、`forge_learn_blueprint` 编码/发送入口。
- ForgeScreen 图谱左右键改为发 C2S 翻页，不只本地 `BlueprintScrollStore.turn()`。
- 增加起炉动作：从当前 station、选中 blueprint、材料槽构造 `forge_start_session`。
- 增加步骤完成动作：淬炼/铭文/开光阶段达到玩家可提交状态后发 `forge_step_advance`。
- 增加图谱学习动作：拖图谱残卷到图谱区时发 `forge_learn_blueprint`。

### P2 — 补 e2e 覆盖，防止协议假绿

- 新增 bot/e2e 场景覆盖：放砧 -> 学图谱 -> 起炉 -> 淬炼/铭文/开光 -> step advance 到结算 -> 收到 `forge_outcome` -> 背包出现带 forge runtime 字段的成品。
- e2e 断言不能只看 schema parse 或单个 CustomPayload 被接受；必须看最终 server state / inventory state 改变。

## 验收测试计划

- server 单测：`forge_start_session` valid payload 发出 `StartForgeRequest`；非法 station/blueprint/material 不发事件并可观测拒绝。
- server 单测：`forge_blueprint_turn_page` 更新 `LearnedBlueprints` 并触发 `forge_blueprint_book` 回推。
- server 单测：`forge_learn_blueprint` 消耗残卷、写入 `LearnedBlueprints`、回推最新图谱书。
- server 单测：`forge_step_advance` 仍要求 session owner 匹配，并能推进到 `ForgeOutcome`。
- client 单测：四个新增 C2S encoder/sender JSON 与 TypeBox/Rust schema 对齐。
- client 单测：ForgeScreen 翻页、学图谱、起炉、步骤完成按钮/动作发对应 C2S，而不是只改本地 store。
- bot/e2e：完整炼器成功路径与至少一个拒绝路径进入 `bash scripts/smoke-test-e2e.sh` 或专门 bot scenario。

## 风险

- 起炉 handler 需要明确 `station_id` 与 MC entity/ECS entity 的映射，避免接受客户端伪造远程 station。
- `ForgeBlueprintTurnPage` 是 UI 状态请求，不应让客户端伪造学习状态；只能改当前页/索引。
- `forge_start_session` 材料列表必须以服务端 inventory 校验为准，不能相信客户端数量。
- StepAdvance 提交时机要避免“按一下直接跳过所有 QTE”；应只结算当前 step，且保留现有 owner/step 校验。
