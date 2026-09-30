# plan-bughunt-player-trade-npc-gate-v1

> **状态：已完成（2026-09-24）**。`server/src/social/mod.rs` 的玩家交易派发链已移除仅供 NPC 使用的 `npc_should_decline_trade()` 门禁；Low / Wanted 声名玩家现在可以正常向其他玩家发起交易，目标端收到 `trade_offer` payload，发起方不会再收到伪 NPC 拒绝文案。一句话主题：修复玩家交易误套 NPC 声名门禁。

> 立项动机：这条链路落在 `plan-social-v1` / `plan-input-binding-v1` 已上线的 **玩家↔玩家交易** 主玩法上，且当前仓库已经用单测把错误行为固定成“应拒绝”。它不属于你列出的 cross-dimension witness leak、social anonymity live refresh、identity/social renown bridge、silent signal runtime bridge 几条已知支线，适合作为 server/social 的另一条侧路径 bug skeleton。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 玩家交易误套 NPC 拒交易门禁 | fix_pr | ✅ 2026-09-24 |

## P0 — 玩家交易误套 NPC 拒交易门禁

- **复现路径**：
  1. client 侧 `TradeOfferIntentHandler` 只在准星命中 `PlayerEntity` 时产出 `InteractIntent::TradePlayer`，并发送 `sendTradeOfferRequest("entity:<protocol_id>", offered_instance_id)`（`client/src/main/java/com/bong/client/social/TradeOfferIntentHandler.java:18-42`；设计稿也明确写成“准星命中 `PlayerEntity`”与 `TradePlayer`，`docs/finished_plans/plan-input-binding-v1.md:137-140`）。
  2. server 侧 `ClientRequestV1::TradeOfferRequest` 进入 `resolve_trade_offer_target()`，该函数直接走实体目标解析，没有 NPC 专用分支（`server/src/network/client_request_handler.rs:1181-1202,9858-9864`）。
  3. 两名在线玩家站在 `CHAT_EXPOSURE_RADIUS` 内、双方背包各有可交易物品时，只要发起方 active identity 落入 Low / Wanted，旧版 `dispatch_trade_offers()` 就会在真正构造 `TradeOfferPayloadV1` 前被 `npc_should_decline_trade()` 提前拦截（旧代码 `server/src/social/mod.rs:1055-1117`）。
  4. 旧版单测 `trade_offer_dispatch_rejects_wanted_initiator_identity()` 直接断言目标玩家收不到 payload、`TradeOfferRegistry.pending` 为空；该错误断言现已由 `server/src/social/mod_tests.rs:1363-1429` 的 Low / Wanted 反向契约替换。

- **根因链路**：
  1. `npc_should_decline_trade()` 的定义和注释都写明语义是“NPC 是否应拒绝某玩家 active identity 的交易”，Low / Wanted 档属于 NPC 反应系统（`server/src/identity/reaction.rs:52-80`）。
  2. `server/src/identity/README.md:142` 进一步把它限定为“NPC 子系统自己 query 调用的 helper”，即它本应服务 `npc/trade.rs` / `npc/brain.rs` 一类路径，而不是 social 的玩家对玩家交易派发。
  3. `dispatch_trade_offers()` 的 Query 两端都是 `With<Client>` 玩家实体，却直接对发起方 active identity 调 `npc_should_decline_trade()`；代码把“NPC 因恶名拒绝服务”的规则误嫁接成了“任何玩家都不得和恶名玩家做交易”。
  4. 由于拦截发生在 payload 派发前，目标玩家完全无感；这不是 UI bug，而是 server authority 在 social 主链上提前短路。

- **影响面**：
  - `trade_offer_request -> dispatch_trade_offers -> TradeOfferRegistry -> TradeOfferScreen` 整条玩家交易链都会受影响；只要发起方 notoriety/revealed tag 把 active identity 推到 Low / Wanted，就会被静默打断。
  - 受影响的不是 NPC 交易，而是真实玩家社交互换、战后补给、熟人代购、赃物流转、带新人交接等玩家协作场景。
  - 现有单测方向错误，会让后续开发者把该限制继续当成预期行为维护，形成长期回归钉子。

- **这个 bug 对实际游玩体验的影响**：
  - 玩家一旦因为毒蛊、背盟、恶名累积等进入 Low / Wanted 档，就会发现**自己不能向其他玩家发起任何交易**；对方没有弹窗、没有拒绝动作，只会像“按钮坏了/网络没反应”一样静默失败。
  - 这会直接切断高风险身份最需要的黑市交换、队友补给、赃物转手和临场救急，体感上不是“NPC 不待见你”，而是**整个人际交易系统突然失灵**。
  - 因为 server 给出的文案是“对方听过这张面孔的事，不愿交易”，玩家还会被误导成“所有玩家都被系统代替做了态度判定”，产生明显的规则错觉。

- **建议修复范围 / 模块**：
  - `server/src/social/mod.rs`：从 `dispatch_trade_offers()` 移除这条 `npc_should_decline_trade()` 门禁，或至少把该门禁下沉到真正的 NPC trade / NPC engagement 链，不得继续出现在 `TradeOfferRequest` 的玩家路径。
  - `server/src/social/mod.rs` tests：把 `trade_offer_dispatch_rejects_wanted_initiator_identity()` 改成反向 pin，确认恶名玩家对玩家交易仍能派发 payload。
  - 若设计上确实需要“玩家可以手动拒绝与恶名者交易”，应通过目标玩家 UI / 响应路径表达，而不是 server 在发起阶段假扮 NPC 代拒。

## 反方裁决摘要

> 下列两轮为 skeleton 立项时按“默认怀疑”标准记录的反方论点与驳回理由；最终 HEAD 的独立只读 validator 结果记录在下方 Finish Evidence。

1. **Round 1 反方论点**：“也许 `TradeOfferRequest` 其实同时服务 NPC 和玩家，`npc_should_decline_trade()` 放这里是统一门禁，不算 bug。”
   **驳回理由**：client `TradeOfferIntentHandler` 只命中 `PlayerEntity`；server `dispatch_trade_offers()` / `handle_trade_offer_responses()` 两端 Query 也都是 `With<Client>`；`resolve_trade_offer_target()` 没有任何 NPC 专用支路。代码证据表明这是纯玩家↔玩家链，而不是共用 trade abstraction。
2. **Round 2 反方论点**：“也许设计上就是要让恶名玩家连玩家交易也一起被封禁，所以这只是 harsh-by-design。”
   **驳回理由**：`npc_should_decline_trade()` 命名、注释、README 都把语义限定在 NPC 反应 helper；`plan-social-v1` 也把“高 notoriety → 商人拒绝交易”写在 NPC 态度基线段，而不是玩家交易规则。若真要封禁玩家交易，应该有独立命名与 UI/协议表达，而不是复用 NPC helper 并向发起方回一条伪 NPC 文案。

## 开放问题

1. 是否要在修复 PR 里顺手补一条 client 端 toast / chat regression case，明确“玩家对玩家交易不会再被 notoriety 档位静默吞掉”？
2. 是否存在其他把 `npc_should_decline_trade()` / `npc_should_seek_attack()` 误接到 `With<Client>` 玩家链路的旁支，值得同 PR 一并全仓 grep 排查？

## 实施证据

- Promotion：`8467f570`，骨架已独立升格为 active plan。
- 第一性原理 RED：`4f777deb` 所在测试 HEAD 上，`trade_offer_dispatch_allows_wanted_initiator_identity_between_players` 编译通过后按行为失败；目标玩家收到的 `TradeOffer` 数量为 `0`，而契约期望为 `1`，证明 server authority 的 NPC 声名门禁真实吞掉玩家 offer。
- 最小修复：`76fbb974` 从 `dispatch_trade_offers` 的纯 `With<Client>` 玩家链移除 `npc_should_decline_trade()` 及其冗余 query 字段；NPC reaction helper 与 NPC 子系统保持不变。
- 主线复验：`1f0a7b0d1` 合并最新 `origin/main`（一个冲突，`server/src/social/mod.rs` 的测试模块已迁移为 `mod_tests.rs`；保留主线结构并移植修复），随后 `d5c628bbd` 完成格式化。
- Targeted GREEN：`BONG_SKIP_SKIN_PREFETCH=1 scripts/build-token.sh cargo test social::tests::trade_offer -- --nocapture` 为 `4 passed / 0 failed`，覆盖正常派发、Low/Wanted 发起者派发、非法请求拒绝、装备中物品拒绝；Low/Wanted 断言目标收到 offer、pending 已登记、发起方没有伪 NPC 拒绝文案。
- 完整 server gate：`scripts/build-token.sh cargo fmt --check` PASS；`scripts/build-token.sh cargo clippy --all-targets -- -D warnings` PASS；在允许 Unix socket 与临时端口的执行环境中，`BONG_SKIP_SKIN_PREFETCH=1 scripts/build-token.sh cargo test` PASS（lib `10364 passed / 0 failed / 1 ignored`，doc-tests `3 passed / 0 failed / 5 ignored`，其余测试二进制均无失败）。
- Fresh validator（2026-09-24，HEAD `d5c628bbd`）：`VERDICT: PASS`；确认 `dispatch_trade_offers` 不再读取 `PlayerIdentities` 或调用 `npc_should_decline_trade`，Low / Wanted 回归覆盖 payload、pending 和无伪拒绝文案，且最新 `origin/main` 已在 HEAD 祖先链上。
- 旧分支的历史署名保持不改；现行流程要求执行模型写入 PR body，当前新增提交均带 `Model: gpt-6-luna`，无需 amend、rebase 或 force-push。

## Finish Evidence

### 落地清单

- `server/src/social/mod.rs:dispatch_trade_offers`：玩家 Query 不再读取 active identity，也不再调用 NPC 专用拒绝门禁；合法玩家交易继续构造并登记 `TradeOfferPayloadV1`。
- `server/src/social/mod_tests.rs:trade_offer_dispatch_allows_low_and_wanted_initiators_between_players`：Low / Wanted 表驱动回归锁定 payload、pending 登记和无伪拒绝文案；原先的错误拒绝测试已移除。
- `docs/finished_plans/plan-bughunt-player-trade-npc-gate-v1.md`：本文件归档并记录最终证据。

### 关键 commit

- `76fbb9740`（2026-07-11）：移除玩家交易链上的 NPC 声名门禁。
- `1f0a7b0d1`（2026-09-24）：合并最新主线并把修复移植到 `social/mod_tests.rs` 新布局。
- `d5c628bbd`（2026-09-24）：格式化玩家交易派发查询，提交带 `Model: gpt-6-luna`。

### 测试结果

- `BONG_SKIP_SKIN_PREFETCH=1 scripts/build-token.sh cargo test social::tests::trade_offer -- --nocapture`：4 passed / 0 failed。
- `scripts/build-token.sh cargo fmt --check`：PASS。
- `scripts/build-token.sh cargo clippy --all-targets -- -D warnings`：PASS。
- `BONG_SKIP_SKIN_PREFETCH=1 scripts/build-token.sh cargo test`：lib 10364 passed / 0 failed / 1 ignored；doc-tests 3 passed / 0 failed / 5 ignored；全套无失败。

### 跨仓库核验

- server：`TradeOfferRequest` → `dispatch_trade_offers` → `TradeOfferPayloadV1` 链路保留，`npc_should_decline_trade` 只留在 NPC reaction 路径。
- agent：本修复不改变 Redis IPC 或 agent schema，未触及 agent 代码。
- client：`TradeOfferIntentHandler` 仍以 `PlayerEntity` 为目标，`trade_offer` payload 契约未变；本修复不改 client 代码。

### 遗留 / 后续

- 本修复于 2026-07-11 完成，曾因旧流程的历史署名阻塞搁置；本次已合并主线并完成复验。
- 若未来需要玩家主动拒绝恶名交易，应在目标玩家 UI / 响应路径增加明确契约，不得把 NPC reaction helper 重新接回玩家派发链。

## 审计来源

bug-hunt 定点轮（server/social 侧路径，避开 cross-dimension witness leak、social anonymity live refresh、identity/social renown bridge、silent signal runtime bridge）。证据来自 client 交互入口、server request handler、social 派发主链、identity helper 文档与仓内现有单测的交叉复核；**skeleton 立项阶段**的结论为 report-only，当时仅新增骨架、不改源码，随后已进入本 plan 记录的 BugFix 实施与验证阶段。
