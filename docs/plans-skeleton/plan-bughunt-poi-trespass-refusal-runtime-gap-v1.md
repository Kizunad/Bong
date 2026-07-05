# plan-bughunt-poi-trespass-refusal-runtime-gap-v1（骨架）

> **骨架（草案）**。一句话主题：`plan-poi-novice-v1` 承诺的“散修聚居点屠村后 1 周 NPC 拒绝交易”在生产 runtime 中是**整链断开的假闭环**。`TrespassEvent` 在生产代码里没有发送者；即便未来补了发送者，当前实现也只是把结果写进内存 `PoiTradeRefusalStore`，没有任何 NPC 交易入口读取；同时桥接 payload 还会重新 `now + 7d` 伪造截止时间，不读 store。结果是玩家屠村后依旧能立刻照常交易，重启后更不可能保留惩罚。

> 立项动机：world runtime / sidepath 角度复核 POI 新手链路，避开本轮明确排除的 zone ecology global refuge / zone_info stale / tide sky omen client bridge / TSY presence reload / pseudo vein restart loss。结论：这是一个 **real-on-main 的 player-facing runtime 断链**，不是单纯文档 TODO。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|---|---|---|---|
| P0 | 散修聚居点 trespass → 1 周拒交易链路在生产 runtime 断开 | fix_pr | ⬜ |

## P0 — 散修聚居点 trespass → 1 周拒交易链路断开

### 复现路径

1. 按 `docs/finished_plans/plan-poi-novice-v1:66,80` 的承诺，进入 spawn 附近散修聚居点，击杀聚居点 Rogue NPC，期待触发 `TrespassEvent` 并开启“1 周 NPC 拒绝交易”。
2. 立刻再次找散修 NPC 交易。实际交易 gating 只走信誉/声望路径：`server/src/network/client_request_handler.rs:1394-1442` 仅依据 `RepTier` / `TradeEligibility` 判定是否拒绝，`server/src/social/mod.rs:977-985` 也只看 `npc_should_decline_trade(active_identity)`。
3. 全仓搜索 `send_event(TrespassEvent` / `TrespassEvent {`，生产代码零命中；仅命中定义与测试：`server/src/world/poi_novice.rs:165`、`server/src/world/poi_novice.rs:739`、`server/src/network/poi_novice_bridge.rs:145`。也就是说“屠村事件”在生产环境根本不会被发出。
4. 即便手工假设未来有人补发 `TrespassEvent`，`server/src/world/poi_novice.rs:283-307` 也只会把结果写进内存 `PoiTradeRefusalStore`；全仓 `PoiTradeRefusalStore` / `refusal_until` 无任何生产 consumer。
5. 若服务器重启，这个 Resource 直接消失；代码中不存在任何 hydrate/persist/flush/AppExit 路径。玩家下次上线依旧不会被拒绝交易。

### 根因链路

1. **文档承诺已定稿**：`plan-poi-novice-v1` 把“屠村惩罚 = 1 周 NPC 拒绝交易”写成 v1 实装项，而不是纯未来设想（`docs/finished_plans/plan-poi-novice-v1:66,80,302,333`）。
2. **事件生产端缺失**：`TrespassEvent` 只有定义，没有任何生产发送者；`npc/poi_rogue_village.rs:1-45` 也只是 log contract 的 stub，没有屠村检测或事件派发逻辑。
3. **状态只落内存 sidepath**：`record_trespass_trade_refusal_stub` 把惩罚写入 `PoiTradeRefusalStore`（`server/src/world/poi_novice.rs:283-307`），而 store 键还是 `format!("{:?}", event.player)` 的临时 Entity debug 字符串（`:289-293`），不是稳定的 player id / char id。
4. **交易主路径完全不读**：NPC 交易入口走 `RepTier` / `TradeEligibility`，拒绝条件是身份/声望，不是 trespass refusal（`server/src/network/client_request_handler.rs:1394-1442`）；社交交易 sidepath 也只看 `npc_should_decline_trade`（`server/src/social/mod.rs:977-985`）。
5. **桥接 payload 还在伪造“已生效”假象**：`publish_trespass_events` 没读 store，直接 `now + TRADE_REFUSAL_SECONDS` 写进 `TrespassEventV1.refusal_until_wall_clock_secs`（`server/src/network/poi_novice_bridge.rs:40-54`）。这让 agent/narration 侧看起来像“处罚已经生效”，但 server gameplay 并未执行。

### 这个 bug 对实际游玩体验的影响

玩家在散修聚居点屠村后，不会遇到设计承诺里的“一周拒交易”后果。体感上就是：你刚杀完散修，旁白/事件还能说“他们会记仇一周”，但同一批或下一批散修依旧正常卖货，重启后更像什么都没发生。对玩家来说，这直接削弱了 POI 的风险感、信誉感和末法江湖的记忆性，形成“文案在吓人，系统没跟上”的割裂。

### 影响面

- **散修聚居点主玩法失真**：`Q109` 的风险收益权衡失效，屠村从“有持续代价”退化为“即时收益、几乎无后果”。
- **agent / narration 与 gameplay 口径分叉**：桥接 payload 会宣称存在 `refusal_until_wall_clock_secs`，但 server 主交易链路不执行。
- **重启恢复必定失忆**：哪怕后续临时补了某个 consumer，不做 persistence 也会在重启后失去惩罚状态。
- **未来接线容易踩空**：store 用临时 `Entity` debug id 记键，不是稳定玩家标识；后补 consumer/persistence 时若沿用这个键，会把同一玩家的跨重连/跨会话判定继续做错。

### 修复建议

1. 在散修聚居点真实屠村检测路径里补生产级 `TrespassEvent` 发送者，而不是只保留 schema/test stub。
2. 把 `PoiTradeRefusalStore` 的键从 `format!("{:?}", Entity)` 改为稳定 `player_id` / `character_id`，并明确 village 粒度。
3. 在 NPC 交易主入口接入 trespass refusal gate：`client_request_handler` 的 NPC 买路必须先查“该村是否仍在拒绝窗口内”，再落回 `RepTier` 逻辑。
4. 统一 payload 与真实状态来源：`publish_trespass_events` 应读取已经写入的 refusal record，而不是独立再算一遍 `now + 7d`。
5. 若 v1 仍坚持“real-time 一周”，就必须补 persistence / shutdown flush / startup hydrate；否则文档要降级成“仅 narration stub，不保证 runtime 生效”，不能继续写成已落地功能。

## 反方裁决

### 退化说明

本会话没有可用的 subagent / delegate 工具，无法像常规 bughunt 流水线那样再开独立 finder/judge 子代理。本轮改为**人工两轮反方裁决**，把反方论点和驳回理由显式记录如下。

### Round 1

- **反方论点**：这不是 bug，只是 `plan-poi-novice-v1` 明说的“stub”；既然完整信誉度系统留给 `plan-identity-v1`，那 v1 不真的拒交易也算符合预期。
- **驳回理由**：文档写的是“1 周 NPC 拒绝交易（Q109: B）”，并且把它列进 v1 实装表与 Finish Evidence（`docs/finished_plans/plan-poi-novice-v1:66,80,302,333`）。“stub”只说明实现是简化版，不等于允许 runtime 完全不生效。当前不是“效果弱化”，而是**没有任何生产路径触发、没有任何交易路径消费**。

### Round 2

- **反方论点**：也许 runtime 真正的拒交易依赖身份/声望系统；屠村后别处会下调 reputation，所以不需要 `PoiTradeRefusalStore` 直接接交易 gate。
- **驳回理由**：若真如此，`PoiTradeRefusalStore` 与 `TrespassEventV1.refusal_until_wall_clock_secs` 就不该存在独立“1 周”语义；但当前实现同时保留了专门的 refusal store、专门的一周常量和专门的 trespass payload，却没有任何地方把 trespass 事件映射到 reputation 变化。换言之，这不是“通过别的系统实现了同一效果”，而是**专门为这个效果搭了一半 sidepath，然后主线没接上**。

## 审计来源

bughunt 2026-07-05（world runtime / sidepaths 角度，report-only）。本轮候选最终收敛为 1 个高置信真 bug：**散修聚居点屠村后一周拒交易只是文档/事件壳，生产 runtime 不会触发、不会执行、不会持久化。**
