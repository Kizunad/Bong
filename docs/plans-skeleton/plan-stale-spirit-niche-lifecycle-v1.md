# plan-stale-spirit-niche-lifecycle-v1（骨架）

> **骨架（草案）**。一句话主题：旧角色灵龛在 `social_spirit_niches` 持久化里残留，`create new character` 只清 username 级复活点、不清旧 `owner=char_id` 行；后续旧灵龛被 reveal 时又会反向清空**当前角色**的持久化复活点，形成“前世幽灵灵龛”与跨角色复活点误伤。

> 立项动机：bughunt 线程 E 在限定 scope（`social` / `player` / `combat/lifecycle.rs` / `persistence`）内确认的 1 个高置信跨会话 bug。已对现有 bughunt plans 去重；不重复棺、派系、关服刷盘等已立项题。

## 范围

- `server/src/social/mod.rs`
- `server/src/combat/lifecycle.rs`
- `server/src/player/state.rs`
- `server/src/persistence/mod.rs`（`social_spirit_niches` 表语义）

## 核心证据

- `reset_for_new_character` 只做两件与灵龛相关的事：`save_player_shrine_anchor_slice(username, None)` + `lifecycle.spawn_anchor = None`，并写明“新角色与前角色无机制关联；灵龛归属同样不继承”（`combat/lifecycle.rs:1718-1740`）。
- 但全仓 **无任何** `DELETE FROM social_spirit_niches`，也无按旧 `lifecycle.character_id` 清理 `SpiritNicheRegistry` 的路径；旧角色灵龛行会继续留在 `social_spirit_niches`。
- `hydrate_spirit_niche_registry` 会把库里所有 `social_spirit_niches` 无差别回灌进全局 registry（`social/mod.rs:381-394`），不要求 owner 在线。
- `handle_spirit_niche_coordinate_reveals` 直接扫描 `registry.active_niches()` 触发 reveal（`social/mod.rs:1999-2026`），因此旧角色离线灵龛仍可被正常揭露。
- `apply_spirit_niche_reveals` 在 reveal 后用 `player_username_from_character_id(event.owner)` 反解出用户名，再调用 `save_player_shrine_anchor_slice(username, None)`（`social/mod.rs:2033-2068`）；这一步是 **username 级** 清空，不校验该用户名当前活跃角色是否仍等于 `event.owner`。
- 同一函数只会给 `lifecycle.character_id == event.owner` 的在线实体清 runtime `spawn_anchor`（`social/mod.rs:2077-2084`）；若玩家已换新角色，当前在线角色不会命中这条保护，但**持久化复活点已经被清掉**。

## 玩家可达路径

1. 玩家 A 正常游玩并放置灵龛。
2. 玩家 A 死透后创建新角色；系统切换 `current_char_id`，但旧角色灵龛行残留。
3. 玩家 A 用新角色继续游玩，并重新放置/持有新的复活点。
4. 另一名玩家后来通过坐标揭露或破坏链路触发旧角色灵龛 reveal。
5. 旧灵龛 reveal 会把 A 的 username 级 shrine anchor 清空；A 下一次重连/关服重启后失去当前角色复活点，回落默认出生逻辑。

## 实际游玩影响

- 前世灵龛会以“幽灵灵龛”形式继续存在、占位、可被揭露，违反“新角色与前角色无机制关联”。
- 旧灵龛的 reveal 会误伤当前角色的持久化复活点；玩家正常重连或服务器重启后，可能发现自己不再从现角色灵龛复活。
- 若玩家想在旧坐标重建灵龛，还可能被旧 registry 条目判成“目标已占用”。

## 修复方向（待展开）

- 新角色创建路径按**旧角色 char_id** 删除/失活 `social_spirit_niches` 行，并同步清理运行时 registry / 方块占位。
- `apply_spirit_niche_reveals` 不得仅凭 `username` 清 anchor；需要先校验该用户名当前 `current_char_id` 是否仍等于被 reveal 的 owner。
- 为“旧角色灵龛应删除”还是“转化为 inert 遗址”做单点决策，但无论哪条都不能继续影响现角色复活点。

## 验收

- 创建新角色后，旧角色灵龛不会在重启后重新 hydrate 为 active niche。
- reveal 旧角色灵龛不会清空当前角色的 persisted shrine anchor。
- 当前角色灵龛在重连与关服重启后仍能保持正确复活点。

## 反方裁决摘要

- **Round 1 怀疑**：是否有现成清理或设计允许旧灵龛保留？结论：未发现任何删除路径；文档与注释反而明确“灵龛归属同样不继承”。
- **Round 2 怀疑**：即使旧灵龛残留，是否只影响旧角色不影响现角色？结论：否。reveal 路径按 `username` 清持久化 anchor，而不是按当前 `char_id` 校验，足以误伤现角色的跨会话复活点。
