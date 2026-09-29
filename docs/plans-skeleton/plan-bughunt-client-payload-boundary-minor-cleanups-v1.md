# BugHunt：客户端 server-data 边界解析缺少类型/范围防护

> 来源 Issue：#1613、#1618、#1621、#1624、#1676、#1735。它们都是 payload 已到达 client 后，缺字段、错误 primitive 或非法标识符没有在边界 fail closed。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | Scroll、treasure、skill、coffin、cinematic、audio payload 统一安全解析 | ⬜ |

## §0 摘要

ScrollOpenHandler 丢弃 proto bridge 转成数字的纯数字正文；treasure/skill/coffin handler 对错误类型或越界值直接调用 Gson/narrowing cast；突破动画和音频 attenuation 对非法/缺失 wire 值直接构造或 switch。这些 malformed server-data 会被 router 吞掉整包，部分路径还会抛异常。

## §1 游玩影响

合法但形状漂移的卷轴无法打开；技能/法宝/教程坐标快照静默失效；恶意或失配的动画/音频 payload 可让客户端崩溃。所有问题都应在 payload 边界丢弃单字段/单事件，不影响下一条 server snapshot。

## §2 复现路径

1. 发送 body_pages:[123]、缺 treasure.instance_id、consumed_scrolls:{} 或超 int 坐标。
2. 发送含空格/非法字符的 animation_id，或缺 attenuation 的 audio recipe。
3. 观察过滤空结果、ClassCastException/NullPointerException/InvalidIdentifierException，以及整包状态不更新。

## §3 今天 origin/main 的根因证据

- client/src/main/java/com/bong/client/network/ProtoServerDataBridge.java:1540-1570 的 normalizeNumericStrings 会把纯数字字符串页变为 JSON number；ScrollOpenHandler.java:66-82 只接受 string。
- TreasureEquippedHandler.java:22-25 对 t.get(...) 直接 getAsLong/getAsString；SkillSnapshotHandler.java:50 对非数组直接 getAsJsonArray；TutorialCoffinPosHandler 的 readInt 只验整数/有限，不验 Integer 范围。
- BreakthroughCinematicHandler.java:195 直接 new Identifier("bong", animId)；AudioAttenuation.java:13-17 裸 switch(wire)，而 AudioEventEnvelope.parseRecipe 可把缺字段传成 null。
- server emitter/schema：server/src/network/treasure_emit.rs、skill_snapshot_emit.rs、tutorial_coffin_emit.rs、breakthrough_cinematic_emit.rs 与 audio_event_emit.rs 是 payload 权威；修复以 client 边界防御为主，不把 malformed 数据当正常状态。

## §4 非重复比对与立项检查记录

- worldview：查卷轴阅读、法宝、教程、突破视听和音频容错；不新增世界规则。
- finished_plans：查 plan-scroll-reading-v1、plan-breakthrough-cinematic-v1、plan-audio-v1 与各 server-data handler；协议生产者已有，不重开玩法 plan。
- active plan：查对应 handler、ProtoServerDataBridge、AudioEventEnvelope；未发现同一批边界 guard 的 active 修法。
- skeleton：查 readRequiredString、tryParse、handler noOp；没有覆盖这六处组合。
- reminder.md：仓内无 docs/plans-skeleton/reminder.md，无相关条目。

## §5 修复骨架

- 所有 handler 先验证字段存在、primitive 类型、有限性和范围；错误 entry 单独 no-op，不抛出、不丢弃同 payload 内其它合法 entry。
- 纯数字正文保留为文本（或 bridge 明确维持字符串）；动画用 Identifier.tryParse/白名单，音频 attenuation 缺失走明确默认或丢弃 recipe。
- 验收覆盖每个 malformed case、合法边界与未知值，断言 router 不崩且后续 snapshot 仍可消费。

## §6 接入面与跨仓契约

- Inputs：scroll_open、treasure_equipped、skill_snapshot、tutorial_coffin_pos、breakthrough_cinematic、audio 的现有 S2C payload。
- Outputs：对应 client store/Screen/VFX/audio；非法输入只产生 no-op/log，不回写 server 状态。
- 共享类型或事件：复用 ServerDataEnvelope、ProtoServerDataBridge、各 handler 的现有 JSON 字段和 Identifier/AudioAttenuation 类型。
- server 符号：各 *_emit 与 ServerDataPayloadV1/proto 转换保持兼容；无 server 业务变更。
- agent：无变更；这些 server-data 不经 agent Redis schema，或 agent 仅镜像既有 payload。
- client：六个 handler/bridge/VFX/audio parser 增加 fail-closed guard。
- worldview / qi_physics：突破/音频仅表现层；不改真元流。卷轴、技能与法宝的权威状态仍来自 server payload。
