# BugHunt: 自定义战斗 VFX/SFX 跨维半径广播串场

## Bug 摘要

`bong:vfx_event` 与 `bong:audio/play` 的自定义半径广播只按 XYZ 距离筛选客户端，没有按 `CurrentDimension` 做同维度过滤。结果是 Overworld 与 TSY/其它维度里坐标数值接近的玩家，会误收到对方战斗招式的自定义粒子和音效。

本 PR 只新增 skeleton plan，不做实际代码修复。

## 实际游玩体验影响

多人游玩时，如果一名玩家在 TSY 同坐标附近施放涡流/剑道等主动技能，主世界同坐标附近的另一名玩家可能看到不属于当前维度的招式粒子；反向也成立。玩家会误判附近有人出招、怪物开战或场景触发。

音效影响更明显：当前 combat 示例大量 `PlaySoundRecipeRequest` 使用 `pos: None`，误收后客户端 `MinecraftSoundSink` 会 fallback 到本地玩家坐标播放。实际体感是玩家身边凭空响起别人跨维度的招式音，破坏“远近端可辨识反馈”和战斗读招可信度。

动画 payload 也走同一 `bong:vfx_event` envelope，但远端目标玩家通常不在当前 `ClientWorld`，可能只是 bridge miss。因此本 plan 的主影响限定为 `SpawnParticle` 与 `audio/play Radius`。

## 复现路径

1. 启动本地联调服，准备两个 Fabric 客户端 A/B。
2. 让 A 留在 Overworld 的 `(x, y, z)` 附近，让 B 进入 TSY，并移动到数值上接近的 `(x, y, z)`。
3. 让 B 通过技能栏施放会发自定义粒子和音效的战斗招式，例如涡流 v2 或剑道基础招。
4. 观察 A 客户端：即使 A 与 B 不在同一维度，A 仍可能收到 B 的 `bong:vfx_event` `SpawnParticle` 或 `bong:audio/play`，表现为跨维粒子串场、身边凭空响起招式音。
5. 反向把 A/B 角色互换，Overworld 施法也会污染 TSY 同坐标附近玩家。

## 根因证据

- 自定义 VFX request 不携带维度：`server/src/network/vfx_event_emit.rs:47-50` 的 `VfxEventRequest` 只有 `origin` 与 `payload`。
- 自定义 VFX 发射器只查位置：`server/src/network/vfx_event_emit.rs:339-342` 查询 `(Entity, &mut Client, &Position)`，没有 `CurrentDimension`；`server/src/network/vfx_event_emit.rs:368-379` 只调用 `is_within_vfx_broadcast_radius_for_event(..., request.origin, position.get())` 后发 `bong:vfx_event`。
- 对照通道证明维度隔离本应存在：`server/src/network/vfx_event_emit.rs:61-69` 的 `VanillaVfxParticleRequest` 带 `dimension`，`server/src/network/vfx_event_emit.rs:394-411` 通过 `DimensionLayers` 找对应 layer 后播放 vanilla particle。问题不在 vanilla particle，而在 Bong 自定义 JSON VFX 通道。
- 音效 recipient 只按距离：`server/src/network/audio_event_emit.rs:38-49` 的 `AudioRecipient::Radius` 只做 `origin.distance_squared(position) <= radius * radius`。
- `audio/play` 发射器同样不查维度：`server/src/network/audio_event_emit.rs:73-78` 查询 `(Entity, &mut Client, &Position)`，`server/src/network/audio_event_emit.rs:121-127` 仅用 `request.recipient.accepts(entity, position.get())` 后发 `bong:audio/play`。
- 战斗主动技能会触发这两条通道：涡流 v2 在 `server/src/combat/woliu_v2/skills.rs:1435-1456` 发送 `VfxEventRequest::SpawnParticle`，并在 `server/src/combat/woliu_v2/skills.rs:1461-1474` 发送 `PlaySoundRecipeRequest { pos: None, recipient: AudioRecipient::Radius { ... } }`。
- 剑道基础招也同类发射：`server/src/combat/sword_basics.rs:997-1008` 发送自定义粒子，`server/src/combat/sword_basics.rs:1017-1028` 发送 `pos: None` 的半径音效。
- 客户端粒子桥没有维度兜底：`client/src/main/java/com/bong/client/visual/particle/BongVfxParticleBridge.java:33-44` 收到 payload 后只查 `VfxRegistry` 和 `MinecraftClient` 并播放。
- 客户端音效会把缺失位置落到本地玩家身上：`client/src/main/java/com/bong/client/audio/MinecraftSoundSink.java:29-38` 在 `sound.pos()` 为空时使用 `client.player` 当前坐标。
- TSY 施法不是不可达路径：`server/src/network/client_request_handler.rs:10092-10264` 的 `handle_skill_bar_cast` 做 slot、known technique、cooldown、skill config、经脉、target resolver 等门控，但没有维度禁用；`server/src/network/client_request_handler.rs:10352-10371` 的 `resolve_skill_cast_target` 也不做维度 gate。
- 涡流 resolver 把维度作为玩法上下文而非拒绝条件：`server/src/combat/woliu_v2/skills.rs:331-335` 读取 `CurrentDimension`，`server/src/combat/woliu_v2/skills.rs:357-362` 用它算 zone context；`server/src/combat/woliu_v2/tests.rs:1684-1692` 在 `CurrentDimension(DimensionKind::Tsy)` 下断言 `resolve_woliu_v2_skill(... Heart)` 返回 `CastResult::Started`。

## 去重边界

- 不重复 #1051：那是绝脉断链 HUD false positive，本 plan 不涉及 HUD 状态语义。
- 不重复 #1012：那是 `vfx_event` slash event_id schema/envelope 契约漂移，本 plan 的 payload 能解析，问题是 server recipient 集合跨维。
- 不重复 #1033/#1018：那些是单技能视听双源发射，本 plan 是共享 VFX/audio emitter 的维度隔离缺口。
- 不重复 #1038：那是 PlayerAnimator 循环姿态衰减，本 plan 不改动画定义。
- 不重复已有 social witness、dropped loot、workbench 等跨维题：这些是各自 gameplay 交互的同坐标门禁，本 plan 聚焦自定义战斗 VFX/SFX 半径广播。

## Skeleton Fix Plan

1. 为自定义 VFX 半径广播补维度语义：
   - 首选给 `VfxEventRequest` 增加 `dimension: DimensionKind`，由发射端从 caster/事件来源 `CurrentDimension` 填入。
   - `emit_vfx_event_payloads` 查询 `(Entity, &mut Client, &Position, &CurrentDimension)` 或等价维度组件，只向同维度且半径内客户端发 `bong:vfx_event`。
   - debug/dev 命令路径明确默认 Overworld 或从执行者 `CurrentDimension` 取值，避免测试命令丢维度。
2. 为 `bong:audio/play` 的半径 recipient 补维度语义：
   - 把 `AudioRecipient::Radius` 扩为 `{ dimension, origin, radius }`，或新增 `DimensionRadius` 并迁移战斗调用点。
   - `emit_audio_play_payloads` 与需要时的 `emit_audio_stop_payloads` 同步查询客户端 `CurrentDimension`，要求同维度才发包。
   - 保留 `Single` / `All` 的既有语义，但审计是否有跨维全服提示类音效需要显式 opt-in。
3. 迁移战斗发射端：
   - 涡流 v2、剑道基础招、其它 combat skill VFX/SFX 发射点从 caster 或 source position owner 取 `CurrentDimension`。
   - 对缺失 `CurrentDimension` 的实体使用明确 fallback，并打 warn 或测试覆盖，避免静默回到跨维广播。
4. 补协议/测试 pin：
   - server 单测覆盖 Overworld caster 与 TSY client 同坐标时不收自定义 VFX/audio。
   - server 单测覆盖同维度且半径内仍正常收到。
   - 增加 `pos: None` 音效回归，证明跨维客户端不会因为 fallback 到本地玩家坐标而误响。
5. 手动验收：
   - 双客户端 Overworld/TSY 同坐标施法，确认粒子和音效只在施法者所在维度出现。
   - 同维度远近端确认 64 格半径内仍可辨识招式粒子/SFX，半径外不收。

## 验证计划

- server targeted tests：新增 `vfx_event_emit` 维度过滤单测、`audio_event_emit` 维度过滤单测，覆盖同维/跨维、半径内/半径外、缺维度 fallback。
- combat regression：涡流 v2 与剑道基础招各至少一个发射路径 pin，证明 request/recipient 携带 caster 维度。
- client targeted tests：如果协议 payload 结构变动，补 Java envelope/router/audio parse 测试；若 S2C JSON 不变，则客户端只需保留现有 parse 回归。
- 集成验证：按 AGENTS 命令矩阵在 `server/` 跑 `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`；涉及 client payload 结构时在 `client/` 用 JDK 17 跑 `./gradlew test build`；最终可用 `bash scripts/smoke-test-e2e.sh` 做双端联调兜底。

## 对抗复核结论

第一轮反方质疑：候选不能泛化到 vanilla particle；动画 payload 可能因目标玩家不在当前 `ClientWorld` 而 bridge miss；音效体验应改成“误收后在本地玩家身边凭空响”，因为 combat 示例多是 `pos: None`。我已采纳，收窄为自定义 `bong:vfx_event` `SpawnParticle` 与 `bong:audio/play` `Radius`。

第二轮反方最终裁决：候选成立。最强反证应是 cast 入口禁止 TSY/异维施法或客户端按维度丢弃 payload，但当前证据相反；`handle_skill_bar_cast` 无维度禁用，涡流 resolver 在 TSY 下可 `Started`，自定义 VFX/audio emitter 只按 XYZ 半径发包。重复风险低，不重复 #1051/#1012/#1033/#1018/#1038；足够开只含 skeleton plan 的 PR。
