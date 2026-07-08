# plan-world-social-cross-dimension-witness-leak-v1（骨架）

> **骨架（草案）**。一句话主题：`server/social` 的 witness 与 zone 判定**维度失明**，把不同维度但坐标接近的玩家错误当成同场见证者，并把非主世界事件写成 Overworld zone。结果不是单纯日志脏数据，而是会把**匿名暴露、死亡见证、仇怨地点、agent 传闻**一起串错。

> 本轮范围刻意避开：`social anonymity live refresh`、`identity/social renown bridge`、`silent signal runtime bridge`、既有 `world-social` 老题、`world-audio`。这里抓到的是**新的 world/social 维度串线 bug**，核心在 `social/mod.rs` witness 采集与 zone helper，而不是前述几条链路。

## 结论

- **判定**：REAL
- **严重度**：major
- **类别**：fix_pr
- **核心 bug 一句话**：跨维度玩家只要坐标接近，就会被 `social` 错当成聊天/死亡 witness；同时非主世界事件的 zone 名还会被硬写成 Overworld。

## 复现路径

### 路径 A：聊天暴露串维度

1. 玩家 A 进入 TSY/其他非 Overworld 维度，停在坐标例如 `[20, 64, 20]`。
2. 玩家 B 留在 Overworld，也站在接近坐标 `[20, 64, 20]`。
3. 玩家 A 发言，`network/chat_collector.rs` 产出 `PlayerChatCollected`。
4. `social::expose_chat_speakers` 用 `nearby_player_char_ids` 找 witness，但查询只带 `Position`，不带 `CurrentDimension`（`server/src/social/mod.rs:405-430`）。
5. `nearby_player_char_ids` 直接做三维坐标差平方，不校验维度（`server/src/social/mod.rs:3209-3234`）。
6. 玩家 B 被错误写入 `SocialExposureEvent.witnesses`，随后 `apply_social_exposures` 把 B 持久化进 A 的 `Anonymity.exposed_to`（`server/src/social/mod.rs:508-599`）。

### 路径 B：死亡见证与仇怨地点串维度

1. 玩家 A 在非 Overworld 维度被击杀，附近只有跨维度“镜像坐标”的玩家 B。
2. `handle_death_social_effects` 再次调用同一个 `nearby_player_char_ids`（`server/src/social/mod.rs:435-505`），B 被错误计入死亡 witness。
3. 同一个函数还调用 `zone_name_for_position` 写 `Feud.place` 和 `SocialExposureEvent.zone`。
4. 但 `zone_name_for_position` 把维度**硬编码为 `DimensionKind::Overworld`**（`server/src/social/mod.rs:3310-3318`），所以 TSY/其他维度的死亡会被记到主世界 zone，找不到时还回落 `spawn`。

## 证据链

### 1. witness 采集完全不看维度

- `expose_chat_speakers` 的玩家查询签名是 `Query<(Entity, &Lifecycle, &Position), With<Client>>`，没有 `CurrentDimension`（`server/src/social/mod.rs:405-430`）。
- `handle_death_social_effects` 的 victim/witness 查询也只有 `Position`，没有 `CurrentDimension`（`server/src/social/mod.rs:435-505`）。
- `nearby_player_char_ids` 内部仅比较 `position.get() - origin` 的距离平方，唯一过滤条件是“不是自己 / 不是同 char_id / 不是 Terminated”，没有任何维度条件（`server/src/social/mod.rs:3209-3234`）。

### 2. 错 witness 会被真正落盘并广播，不是 harmless telemetry

- `apply_social_exposures` 直接把 `exposure.witnesses` 写入 `anonymity.expose_to(...)`，随后持久化到 `social_anonymity` 与 `social_exposures`（`server/src/social/mod.rs:515-574`）。
- 同一个函数还会把 `SocialExposure` payload 发给 actor 和所有 witness（`server/src/social/mod.rs:577-599`）。
- 这意味着错误 witness 不只是“日志看错了”，而是会**永久改变匿名可见性**，并当场把曝光消息推给本不该看到的人。

### 3. zone 名 helper 也是维度失明

- `social/mod.rs` 的 `zone_name_for_position` 把 `find_zone` 维度写死成 `Overworld`（`server/src/social/mod.rs:3310-3318`）。
- `network/chat_collector.rs` 的同名 helper 也一样写死 `Overworld`（`server/src/network/chat_collector.rs:427-432`），说明聊天来源 zone 本身就可能在非主世界时写错。
- 仓库里已有正确参照：`identity/gossip.rs` 的 `zone_name_for` 显式接收 `DimensionKind`，并有 `dimension_kind(Option<&CurrentDimension>)` helper（`server/src/identity/gossip.rs:260-271`）。也就是说，**项目里不是没有正确模式，而是 social 这条链路没对齐**。

## 根因链路

1. `social` 侧 witness 模型把“空间接近”错误实现成“同一坐标系里的欧氏距离接近”，但没有附带“同一维度”前置条件。
2. `chat` 与 `death` 两条入口都复用了这个维度失明的 helper，所以 bug 同时污染匿名暴露和死亡见证。
3. zone helper 又把 `find_zone` 维度硬编码成 Overworld，导致非主世界事件的地点标签进一步错上加错。
4. `apply_social_exposures` 会把错 witness 持久化到 `Anonymity.exposed_to`，把“瞬时串线”升级成“长期状态污染”。

## 影响面

- `SocialExposureEvent` witness 列表
- `Anonymity.exposed_to` 持久化状态
- client 收到的 `ServerDataPayloadV1::SocialExposure`
- 死亡触发的 `SocialRelationshipEvent`（feud 的 `place` metadata）
- Redis `bong:social/exposure` / `bong:social/feud` 下游叙事与 agent 消费
- 所有依赖“谁见过你 / 你在哪个 zone 出事”的后续系统

## 这个 bug 对实际游玩体验的影响

- 玩家在 TSY/其他维度说话或死亡，**主世界里根本不在场的人**会被系统认定为 witness，匿名身份被错误揭露。
- 受害者会看到莫名其妙的曝光记录，像是“隔着维度也被人看见”；这会直接破坏匿名博弈的可信度。
- 击杀、死亡、结仇的地点会被写成主世界 zone 或 `spawn`，玩家回看事件、agent 写江湖传闻时会出现“死在渊里却记成主世界”的割裂感。
- 因为 `exposed_to` 会落盘，这不是掉线自愈的小毛病，而是会在后续会话里继续影响“谁认识你”。

## 修复建议

1. 给 `expose_chat_speakers`、`handle_death_social_effects`、`nearby_player_char_ids` 补 `CurrentDimension`，把 witness 判定收紧为“同维度且半径内”。
2. 把 `social/mod.rs` 与 `network/chat_collector.rs` 的 `zone_name_for_position` 改成显式接收 `DimensionKind`，不要再硬编码 `Overworld`。
3. 对齐 `identity/gossip.rs` 的正确模式：统一提供 `dimension_kind(Option<&CurrentDimension>)` helper，避免同类 bug 再分叉。
4. 补回归测试：
   - `chat exposure`：跨维度同坐标玩家**不能**成为 witness。
   - `death exposure`：跨维度同坐标玩家**不能**成为 witness。
   - `zone naming`：非 Overworld 死亡/聊天应保留所在维度的真实 zone，而不是 `spawn`/主世界 zone。

## 反方裁决

> 当前会话没有可再开的 subagent 工具，本轮按要求做**退化处理**：两轮反方论证与驳回均由本会话手工完成，论点与反驳显式记录如下。

### Round 1

- **反方论点**：Valence/世界层可能已经保证不同维度的玩家不会出现在同一组 `Client` 查询里，所以这里不查 `CurrentDimension` 也没事。
- **驳回理由**：现有查询签名只是 `With<Client>` + `Position`，代码中没有任何维度过滤；`nearby_player_char_ids` 也确实对所有 client 统一做坐标差（`server/src/social/mod.rs:3209-3234`）。如果引擎层真有“天然按维度隔离”的保证，就不需要项目里另写 `CurrentDimension` 和 `identity/gossip.rs` 那套显式维度查 zone 的模式了，但仓库事实恰好相反。

### Round 2

- **反方论点**：即便 witness 串了，也只是 exposure feed/agent narration 脏一点，不会影响真实玩法。
- **驳回理由**：`apply_social_exposures` 会把 witness 直接写进 `Anonymity.exposed_to` 并持久化，再广播给 actor/witness（`server/src/social/mod.rs:515-599`）。这已经是**真实状态污染**，会改变后续“别人是否认识你”的判定，不是单纯旁路日志。死亡链路还会把 feud `place` 一起写错，影响后续关系/传闻语义。

## 建议 PR 形态

- 单 PR，范围控制在 `server/src/social/mod.rs` + `server/src/network/chat_collector.rs` 的维度参数下传与测试补齐。
- 若修复时发现 `spirit_niche` reveal/break 路径同样存在“只看 Position 不看维度”，建议作为**同根因同 PR 顺手收口**；若改动扩散过大，再拆第二个 PR。

## 审计来源

- worktree: `bughunt-loop-20260705-bs`
- branch: `bughunt-loop-20260705-bs-world-social2`
- 方法：repo 内代码检索 + 路径对拍 + 两轮手工反方裁决
- 结论：这是新的 `world/social` 维度串线 bug，不属于已排除的 anonymity live refresh / renown bridge / silent signal bridge / world-audio / 旧 world-social 题。
