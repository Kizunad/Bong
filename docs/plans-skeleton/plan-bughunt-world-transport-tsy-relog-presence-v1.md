# plan-bughunt-world-transport-tsy-relog-presence-v1（骨架）

> **骨架（草案）**。一句话主题：world transport / travel / load 边界链路 bug-hunt 确认 **1 个真实 bug**：**玩家在 TSY（坍缩渊）内断线或关服重启后，会按持久化位置重登回 TSY，但 `TsyPresence` 会丢失**，从而同时打断撤离/出关、负压抽真元、TSY 死亡掉落分流三条玩法闭环。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 🔴 TSY 内断线/重启后“维度与坐标保留，但 session presence 丢失” | fix_pr | ⬜ |

## P0 — 🔴 TSY 内断线/重启后重登到 TSY，但 `TsyPresence` 丢失

- **#1 major（fix_pr）**：`server/src/player/mod.rs:365-421` 与 `:514-570` 在断线保存、关服 flush 时，只把 `position + last_dimension` 写入 `save_player_slices_with_coffin(...)`；`server/src/player/state.rs:156-167` 的 `LoadedPlayerSlices` 也只承载 `position / last_dimension / inventory / lifespan / skill_set / known_techniques / ui_prefs`，没有任何 `TsyPresence`/`family_id`/`return_to`/`entry_inventory_snapshot` 持久字段。
- `server/src/player/state.rs:1084-1105` 的 slow slice 读取只查 `SELECT pos_x, pos_y, pos_z, last_dimension FROM player_slow`；`server/src/player/mod.rs:206-247` 重连时直接 `position.set(persisted.position)`、按 `last_dimension` 切 layer，并插回 `CurrentDimension(last_dimension)`。结果是：**重连会把玩家放回 TSY 坐标与 TSY layer，但不会恢复 `TsyPresence`**。
- 这与既有设计假设直接分叉：`docs/finished_plans/plan-tsy-zone-v1:618-619` 明写的是“`TsyPresence` 丢失（下线 = 强制出关）”且“重连玩家坐标传送到灵龛”。当前主干并没有实现“强制出关/回灵龛”，只实现了“presence 丢失”的一半。

## 复现路径

1. 启服后用现有 TSY 路径生成一个 family（如 `/tsy_spawn tsy_lingxu_01`），从主世界 Entry portal 进入 TSY。
2. 进入后玩家会拿到 `TsyPresence`，并被 `DimensionTransferRequest` 送入 `CurrentDimension::Tsy`。
3. 在 TSY 内原地断线，或让服务器正常关服再重启。
4. 同角色重连。
5. 观察：玩家会按持久化坐标直接出生在 TSY；但 `TsyPresence` 不存在，后续交互进入“人在 TSY、session 不在 TSY”的断裂态。

## 直接后果链路

- **出关失效**：`server/src/world/tsy_portal.rs:138-176` 的 `tsy_exit_portal_tick` query 强制要求 `(Entity, &Position, &TsyPresence, &CurrentDimension)`；丢了 presence 后，站进 Exit portal 也不会触发回传。
- **撤离失效**：`server/src/world/extract_system.rs:218-227` 在 `presence` 缺失时直接返回 `ExtractRejectionReason::NotInTsy`。也就是说，race-out / collapse tear / 正常 extract 都会被拒。
- **负压抽真元失效**：`server/src/world/tsy_drain.rs:114-154` 的 `tsy_drain_tick` 也是 `TsyPresence + CurrentDimension` 双 gate；presence 丢失后，玩家留在 TSY 却不再被抽真元。
- **死亡分流失效**：`server/src/inventory/mod.rs:3842-3905` 只有 `presences.get(ev.entity)` 成功时才走 TSY 掉落分流与干尸路径；重连后在 TSY 死亡会跌回主世界 50% 掉落逻辑，秘境所得 / 原带物不再分流。

## 影响面

- 所有“玩家在 TSY 内下线再上线”的路径都会中招，包括主动断线、崩溃重连、服务器正常重启。
- 这是一个 **transport + load 边界态撕裂**：传送系统把玩家送进 TSY，持久化系统却只记住了“你在哪个维度的哪个坐标”，没记住“你为什么在这里、该怎么出去、该承受哪些 TSY 规则”。
- 它同时造成 **软锁** 与 **可利用漏洞**：
  - 软锁：玩家无法通过 exit / extract 正常离开 TSY。
  - 漏洞：玩家可以通过重连抹掉 TSY 负压与 TSY 死亡惩罚，变成“留在秘境里但不再按秘境规则结算”。

## 这个 bug 对实际游玩体验的影响

- 玩家在秘境里掉线或遇到日常重启后，重登会发现自己还在 TSY 地图里，但出口不认、撤离不认、负压不再扣、死亡掉落也变味，体感上就是“世界把我留在秘境里，规则却忘了我是秘境内玩家”。
- 对普通玩家，这是高概率的困惑与卡死；对熟悉机制的玩家，这是可重复利用的规避风险手段，只要重连一次，就能绕开 TSY 的核心压力闭环。

## 修复建议

- **推荐最小修法**：兑现既有文档语义。断线保存与关服 flush 发现 `CurrentDimension::Tsy + TsyPresence` 时，不保存 TSY 坐标；改为把玩家强制结算到 `presence.return_to`（或灵龛）后再落盘，并清掉 TSY session 运行态。
- **备选修法**：正式持久化 `TsyPresence`（至少 `family_id / entered_at_tick / entry_inventory_snapshot / return_to`），重连时完整回填，并补齐 reconnect 后 exit / extract / drain / death-drop 集成测试。这个方案更完整，但 blast radius 更大。
- 无论选哪条，都应新增“TSY 内断线重连”端到端测试，覆盖断线与关服重启两条入口，并锁住 exit / extract / drain / death-drop 四个回归点。

## 反方裁决（退化处理）

- 当前会话没有可用 subagent；本轮按要求做**同会话两轮反方裁决**，显式记录反方论点与驳回理由。
- **Round 1 反方论点**：这不是 bug，因为 `plan-tsy-zone-v1` 已经写了“MVP 假设 `TsyPresence` 丢失”。
- **Round 1 驳回**：文档写的是“`TsyPresence` 丢失 = 下线强制出关 + 重连回灵龛”，但代码实际行为是“保留 TSY 坐标与维度，只丢 presence”。也就是只实现了一半语义，留下不一致中间态，不能用原计划假设为当前行为背书。
- **Round 2 反方论点**：presence 丢失也未必有问题，因为玩家仍有 `CurrentDimension::Tsy`，TSY 规则也许可以只靠维度继续工作。
- **Round 2 驳回**：关键系统都没有“只靠维度”工作。`tsy_exit_portal_tick`、`start_extract_request`、`tsy_drain_tick`、`apply_death_drop_on_revive` 都显式以 `TsyPresence` 为 gate；因此这不是轻微状态漂移，而是多个玩法闭环同步失效。

## 开放问题

1. 产品语义应选“下线=强制出关”还是“允许在 TSY 内重连并继续 session”？
2. 若选强制出关，落盘时是回 `presence.return_to` 还是统一回灵龛？两者与现有 plan 文案需再对齐一次。
3. 若选完整持久化 `TsyPresence`，`entered_at_tick` 是否应跨关服延续，还是重连时重置并额外记录累计停留时长？

## 审计来源

- 本轮 bug-hunt 聚焦 world transport / teleport / travel / load 边界链路，排除了用户显式避开的 dying elder TSY unloaded、dimension env resync、preview config dead server 题。
- 证据来自 `player` 持久化/重连链、`world/tsy_*` 出关/撤离/负压链、`inventory` TSY 死亡分流链，以及 `plan-tsy-zone-v1` 已归档设计语义对照。
