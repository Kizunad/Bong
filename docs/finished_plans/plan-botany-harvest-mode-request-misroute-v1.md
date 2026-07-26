# plan-botany-harvest-mode-request-misroute-v1

> 一句话主题：`botany_harvest_request` 当前没有按“切换采集模式 / 消费既有 session”落地，而是把 `session_id` 错接进旧的 `GameplayAction::Gather.resource` 通道；结果是 **E/R 手动/自动按钮不会真正切换 server 侧采集 session，且每次按钮请求还可能白拿一笔 gather 真元/karma/叙事奖励**。影响是：玩家在 botany 主路径里看到“手动采集 / 自动采集”浮窗，却既切不了模式，也能凭空触发一次伪采集收益，直接破坏采集节奏与真元经济。

> 立项动机：这是 `botany` 当前 client→server 主交互链上的高频可达问题，不是边角 case。玩家只要进入采集浮窗、按一次 `E` 或 `R`，就会命中这条错误接线；它既让 UI 承诺失效，又把本应“只切模式”的请求错误地走成了旧 gather 奖励路径，值得先立 skeleton 固化证据、玩家影响、修复面与验收抓手，再由后续 fix PR 单独落地。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | botany 采集模式请求错接旧 gather 主链 | fix_pr | ✅ 2026-07-26 |

## P0 — botany 采集模式请求错接旧 gather 主链

- **现象**：
  - client HUD 在 `client/src/main/java/com/bong/client/botany/BotanyHudBootstrap.java:115-126` 里，只有 `session.interactive()` 且已有 `session.sessionId()` 时才会发 `ClientRequestSender.sendBotanyHarvestRequest(session.sessionId(), mode)`。
  - 协议层 `client/src/main/java/com/bong/client/network/ClientRequestProtocol.java:302-312` 把这个值原样编码成 `botany_harvest_request.session_id`。
  - server 侧 `server/src/network/client_request_handler.rs:765-786` 没有做任何 botany session lookup，而是直接 `queue.enqueue(... GameplayAction::Gather(GatherAction { resource: session_id, target_entity: None, mode: Some(...) }))`。
  - 旧 gather 落点 `server/src/player/gameplay.rs:293-345` 把 `resource` 当“采集资源名”走 `canonicalize_herb_id(resource_name)`；只有它真的是草药 id 时才会 `start_or_resume_harvest(...)`。否则这一步静默跳过，但后面的 `gather_qi_from_zone(...)`、`inventory_score += ...`、`karma += ...`、`GameEvent{ action: "gather" }`、`pending_narrations.push_player("你采得 {resource_name}...")` 仍会照常执行。

- **第二层根因（不是只有字段错接）**：
  - 就算把 `resource` 临时改成草药 id，当前消费函数仍不对。`server/src/botany/harvest.rs:52-64` 的 `start_or_resume_harvest` 对“该玩家已存在 session”会直接 `return`。
  - 但 client 侧按钮请求本来就只会在已有 interactive session 时发出（`HarvestSessionViewModel.interactive()`：`client/src/main/java/com/bong/client/botany/HarvestSessionViewModel.java:160-162`）。
  - 这意味着当前 botany 模式请求链不是“更新已有 session.mode”，而是错误地复用了“若无 session 则启动一个新 gather”的旧入口；两端语义天然不匹配。

- **可达链路**：
  - server 会把活跃 botany session 的 `session.player_id` 作为 `BotanyHarvestProgress.session_id` 下发（`server/src/botany/mod.rs:238-291`，关键字段在 `:260` 和 `:275`）。
  - client `BotanyHarvestProgressHandler` 会把这个 `session_id` 存进 `HarvestSessionStore`（`client/src/main/java/com/bong/client/network/BotanyHarvestProgressHandler.java:25-42`）。
  - HUD 文案 `client/src/main/java/com/bong/client/hud/BotanyHudPlanner.java:222-224` 还明确提示“右键植物开始采集；选 E 或 R 启动模式”；按钮区也把 `E`/`R` 显示为两条真实交互分支（`:242-310`）。
  - 因此这不是测试专用死路，而是玩家正常进入采集浮窗后就会命中的主路径。

- **为什么这是 bug，不是设计**：
  - `BotanyHarvestRequestV1` / `BotanyHarvestProgressV1` 的 schema 都把 `session_id` 明确定义成 session token，而不是草药资源名（`server/src/schema/botany.rs:22-38`、`agent/packages/schema/generated/client-request-botany-harvest-v1.json`）。
  - client 测试也把它当 session token 编码（`client/src/test/java/com/bong/client/network/ClientRequestSenderTest.java:426-434`、`ClientRequestProtocolTest.java:586-592`）。
  - 真正的 bug 在于：server 没有按“session_id → 找到现有 HarvestSession → 更新 mode / request_pending”实现，而是把它塞进了旧 gather 命令的 `resource` 字段。

- **这个 bug 对实际游玩体验的影响**：
  - 玩家最直接的体感是：浮窗明明有“手动采集 / 自动采集”两个按钮，但按下去不会真正切换 server 侧 session，`2.0s · 专注` / `5.0s · 仅受击断` 这些模式差异在 authoritative 逻辑上根本不成立。
  - 更糟的是，按钮请求会误触发 `apply_gather_action` 的后半段旧奖励路径：玩家可能在**没有成功切模式、甚至没有重新选中草药**的情况下，白拿一笔 zone→player 的 gather 真元、karma / inventory_score 增量，以及一条“你采得 offline:Azure / session-botany-01”之类的错误叙事。
  - 这会同时破坏三层体验：1) botany UI 承诺失真；2) 采集模式数值（手动/自动时长、受击/移动打断、XP 节奏）失真；3) zone 真元与玩家收益口径被按钮 spam 污染。

- **建议修复范围 / 模块**：
  - 优先收口 `server/src/network/client_request_handler.rs`、`server/src/player/gameplay.rs`、`server/src/botany/harvest.rs`、必要时补 `server/src/botany/components.rs`。
  - 修复方向应改成真正的“session 消费”路径，而不是继续复用 `GameplayAction::Gather`：
    - `botany_harvest_request` 应携带 / 消费能唯一定位当前 `HarvestSession` 的 key；
    - server 应在 `HarvestSessionStore` 上**更新现有 `session.mode / request_pending / duration_ticks / started_at_tick`**，或新增专门的 `BotanyHarvestModeRequest` event/system；
    - 同时移除这条请求对 `gather_qi_from_zone` / karma / narration 的任何副作用，避免再走旧 gather 奖励路径。
  - **不建议**继续在 `GameplayAction::Gather` 上打补丁硬兼容：它的语义是“采集资源”，不是“切换既有 session 的模式”；继续复用只会把 botany session 逻辑和旧命令奖励链越缠越死。

- **验收抓手**：
  - 至少补 5 组 pin。1) `botany_harvest_request(session_id, manual/auto)` 必须命中现有 `HarvestSession`，而不是入队旧 `GatherAction.resource`。2) 已存在 session 时切到 `AUTO`，server authoritative `mode/duration_ticks` 必须真的变更。3) 按 `E/R` 不得产生额外 `gather_qi_from_zone` 转账、karma、inventory_score 或 narration。4) 非法 / 过期 `session_id` 只应 no-op / warn，不得给玩家任何 gather 收益。5) HUD “请求中” / 模式高亮与 server 回传状态一致，不再出现 client 自己切了但 server 没切的假象。

## 反方裁决摘要

1. **Round 1：怀疑“`session_id` 也许在别处会被重新解释成 herb id / session lookup”**  
   复核结果：未找到第二消费方。`botany_harvest_request` 进入 server 后唯一落点就是 `client_request_handler.rs:765-786` 的 `GameplayAction::Gather { resource: session_id }`；仓内没有任何 `BotanyHarvestRequest` 专用 session lookup，也没有 `session_id -> herb id` 归一逻辑。该反证失败。

2. **Round 2：怀疑“即使字段错接，最多只是按钮没反应，不会有真实收益污染”**  
   复核结果：`player/gameplay.rs:313-343` 明确证明不是单纯 no-op。`canonicalize_herb_id` 失败后，`gather_qi_from_zone`、`inventory_score`、`karma`、`GameEvent(action=gather, resource=...)` 和 `"你采得 {resource_name}"` narration 仍照常执行；同时 `start_or_resume_harvest` 又因已有 session 直接返回（`botany/harvest.rs:62-64`），所以这条请求会同时造成“模式没切成”与“伪采集收益仍发放”的双重错误。该反证失败。

## 开放问题

1. `session_id` 的 authoritative 语义是否继续沿用 `session.player_id`，还是应改成独立、可过期的真正 session nonce？fix PR 里需要一次定清，避免再把“玩家 id”混进交互层。
2. `request_pending` 目前 client 只在本地 optimistic 设置；fix 时要不要补 server authoritative ack / reject，避免高 ping 下 UI 长时间自嗨？
3. `plan-lingtian-v1` 复用 harvest-popup 的链路是否也有同型“mode 请求复用旧 gather 入口”问题？建议 fix PR 顺手对比一遍，防止只补 botany 一处。

## 审计来源

bughunt 线程 AH 定点轮（仅收窄 `server/src/botany/`、`client/src/main/java/com/bong/client/botany/` 及其直接 network/gameplay 接线）。已排除既有立项的“满包吞产出”“离线卧棺幽灵棺”等主题。外部子代理审查因隐私/网络审批被拒，当前结论基于仓内只读证据完成两轮人工反方裁决；先提交 skeleton plan 固化玩家影响、根因层次、修复面与验收抓手，再由后续 fix PR 单独落地。

## 验证结论（2026-07-26 整理审计追认）

server `client_request_handler.rs` 的 `BotanyHarvestRequest` 分支已不再入队 `GameplayAction::Gather`，改为调用 `botany::harvest::request_harvest_mode` 更新既有 `HarvestSession`，堵住了模式切换错接旧 gather 奖励路径的问题。修复 commit 19f1eab8e（2026-07-06，PR #897）已合入 origin/main，4 条 pin 测试锁定该行为。

## Finish Evidence

- **落地清单**：`server/src/network/client_request_handler.rs`（`BotanyHarvestRequest` 分支改调 `botany::harvest::request_harvest_mode`）
- **关键 commit**：19f1eab8e（2026-07-06，修复 botany 采集模式请求错接旧 gather 主链，PR #897 已 merge）
- **测试结果**：`botany_harvest_request_updates_existing_session_without_gather_enqueue` 等 4 条 pin；2026-07-26 审计为只读核验（Read+grep+git log 对拍 origin/main），未重跑测试套件
- **跨仓库核验**：server-only（`client_request_handler.rs` / `botany::harvest`）
- **遗留 / 后续**：无
