# plan-bughunt-q-world-season-dimension-env-resync-v1（骨架）

> **骨架（草案）**。一句话主题：`world/season/environment` 主链里的 `zone_environment` 只会在“新客户端加入”或“zone 环境状态自身变脏”时重发，而跨位面传送只更新 `CurrentDimension` / `VisibleChunkLayer`；client 端却会在 `ClientWorld` 切换时直接 `clear()` 本地 environment registry。结果是：**玩家正常进出 TSY 或其他位面后，雾幕、灰烬、雪飘、闪电柱等环境特效会整段消失，直到后续天气/环境再次产生 dirty update 才恢复**。

> 立项动机：这条缺口位于正式环境同步路径，不是 dev-only，也不重复刚立项的 ambient audio 题。它落在 `server/src/world/dimension_transfer.rs`、`server/src/network/zone_environment_bridge.rs`、`client/.../EnvironmentEffectController.java` 的正常联动链上，且 TSY 进出是明确可达的主玩法路径，值得先立 skeleton-only plan 收口证据、修复面与验收抓手，再由后续 fix PR 落地。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 跨位面后 zone environment 快照不重发，client 环境特效整段丢失 | fix_pr | ⬜ |

## P0 — 跨位面后 zone environment 快照不重发，client 环境特效整段丢失

- **现象**：`server/src/network/zone_environment_bridge.rs:9-16` 的 `mark_zone_environment_dirty_for_new_clients` 只看 `Added<Client>`，有新连接时才 `mark_all_dirty_for_snapshot()`；`zone_environment_broadcast_system` 也只会广播 `registry.drain_dirty()` 拿到的 zone（`:18-70`）。但 `server/src/world/dimension_transfer.rs:34-85` 的正式跨位面实现只是处理 `DimensionTransferRequest`，更新 `EntityLayerId`、`VisibleChunkLayer`、`Position`、`CurrentDimension`，没有任何一步去把目标维度的 zone environment 标脏或给该玩家补发快照。
- **为什么这会稳定触发**：`server/src/world/environment.rs:154-173` 的 `replace_for_dimension` 在“effect 列表没变、dimension 没变”时直接 return，不 bump generation、不进 dirty；`sync_zone_environment_effects` 每 tick 虽然会重算（`:276-302`），但只要天气/zone 状态没变，就不会产生新广播。换句话说，**跨位面本身不会制造 environment dirty**。
- **client 侧放大器**：`client/src/main/java/com/bong/client/environment/EnvironmentEffectController.java:65-68` 检测到 `lastWorld != world` 就立刻 `clear()`；`clear()` 会把 `REGISTRY`、audio、fog 全清空（`:104-109`）。而 `server/src/world/dimension_transfer.rs:5-6` 注释明确说明，Valence 会因 `Changed<VisibleChunkLayer>` 触发 respawn，让客户端“看见位面变化”。这意味着**正常位面切换时 client 会主动丢掉旧环境状态，但 server 没有对应补发**。
- **可达链路**：`server/src/world/tsy_portal.rs:54-126` / `:138-180` 的 entry/exit portal 都会给玩家发 `DimensionTransferRequest`；TSY zone 同时是 `server/src/world/environment.rs:329-331` 明确挂默认 `FogVeil + AshFall` 的区域。因此这不是理论缝隙，而是**玩家正常入渊、出渊就会踩中的正式路径**。
- **对实际游玩体验的影响**：玩家进出 TSY、灾劫区或未来其他独立位面后，会看到本该立刻存在的环境反馈整段缺席，例如 TSY 的黑雾/灰烬、天气映射出来的雪飘或闪电柱不会出现；视觉上像“位面切过去了，但天象和环境系统没跟上”。这会直接削弱区域辨识、危险预警和沉浸感，而且恢复时机取决于下一次天气/环境 dirty，体感上是随机失灵，不是短暂过渡。
- **建议修复范围 / 模块**：优先收口 `server/src/network/zone_environment_bridge.rs`、`server/src/world/dimension_transfer.rs`、必要时补 `client/src/main/java/com/bong/client/environment/EnvironmentEffectController.java` 测试。修复方向至少要保证其一：1) 玩家 `CurrentDimension` 变化时，对目标维度相关 zone 触发 snapshot 补发；2) 或保留 client 现有 registry，但在 world 切换后按维度重放缓存；推荐前者，因为 server 已经掌握 authoritative zone→dimension 映射和 generation。
- **验收抓手**：至少补 4 组 pin。1) Overworld → TSY 传送后，无需等待天气变化，也能立即收到 TSY zone environment payload。2) TSY → Overworld 返回后，同样立即恢复 Overworld zone 的 fog/effect。3) 若传送前后 zone effects 完全相同，仍要验证 snapshot 补发不依赖 generation bump。4) 客户端集成测试或最小 harness 里，`lastWorld != world` 清空后，目标维度 payload 会在下一帧/同次传送链路内重新填回 registry。

## 反方裁决摘要

1. Round 1 反方怀疑“`sync_zone_environment_effects` 每 tick 都跑，也许换位面后会自然重发”。复核 `server/src/world/environment.rs:154-173` 与 `:276-302` 后排除：没有 effect/dimension 变化就不会 mark dirty，重算本身不等于广播。
2. Round 2 反方怀疑“即使 server 不补发，client 可能保留旧 registry，不构成玩家可见 bug”。复核 `client/.../EnvironmentEffectController.java:65-68` 与 `:104-109` 后排除：`ClientWorld` 一换就 `clear()`，环境状态被明确清空；再结合 `dimension_transfer.rs:5-6` 的 respawn 注释，可确认换位面不是纯逻辑切换，而是会触发这条清空路径。
3. 人工最终裁决：server 端“只给新连接补快照”、client 端“换 world 必清空”这两条同时成立，且中间没有 `Changed<CurrentDimension>` 或传送后补发钩子，因此该候选在两轮证伪后继续存活，置信度高。

## 开放问题

1. snapshot 补发应按“目标维度全部 zone”发，还是按玩家落点附近 zone 裁剪发？前者实现最稳，后者更省包但要额外引入位置过滤。
2. 是否要顺手给 `zone_environment_bridge` 增一条“dimension transfer / respawn snapshot”回归测试，避免未来再次出现“Added<Client>` 有快照、换位面没快照”的调度裂缝。

## 审计来源

bughunt 线程 Q，限定 `world/season/environment` 与直接相邻 network/state 主路径：`server/src/world/`、`server/src/world/season/`、`server/src/world/weather_physics/`、`client/src/main/java/com/bong/client/environment/`、`client/src/main/java/com/bong/client/atmosphere/`、`client/src/main/java/com/bong/client/season/`。当前结论是 **report-only**：先提交 skeleton plan，把触发链、玩家影响、两轮反方裁决和修复抓手讲清，再由后续 fix PR 单独落地跨位面补快照。
