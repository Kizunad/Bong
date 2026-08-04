# plan-refactor-cast-av-contract-v1 — 施法同步/技能栏/AV 单一事实源契约（重构轨 R9）

> 所属总纲：`plan-refactor-master-v1.md`。一句话：让每次玩家施法拥有服务端权威身份与完整终态，并把每招动画、粒子、音效、HUD、图标收敛到注册时唯一绑定，消除技能栏断链与 AV 双发、错接、缺失。
>
> 阶段总览：P0 ✅ 2026-08-03；P1 ⬜；P2 ⬜；P3 ⬜；P4 ⬜。

## 现状证据（2026-08-03 P0 复核）

- `SkillRegistry` 当前只保存 `skill_id → SkillFn`，生产初始化共注册 **68** 个 resolver；`TECHNIQUE_IDS` / `TECHNIQUE_DEFINITIONS` 各 **49** 条。两集合交集为 **46**，registry-only **22**，definition-only **3**，说明 resolver、玩家入口、AV 元数据没有共同事实源（`server/src/cultivation/skill_registry.rs:71-122`；`server/src/cultivation/known_techniques.rs:67-166`）。
- server `Casting` 已保存 `source` 与 `skill_id`，但 `CastSyncV1` 只发 `phase/slot/duration_ms/started_at_ms/outcome`；client `CastSyncHandler.sourceFor()` 因此只能从当前快照猜来源并默认 `QUICK_SLOT`（`server/src/combat/components.rs:421-447`；`server/src/schema/combat_hud.rs:97-106`；`client/src/main/java/com/bong/client/network/CastSyncHandler.java:19-51,97-103`）。
- `CastPhaseV1` 已有 `Idle/Casting/Complete/Interrupt`，所以本轨不重复“新增 phase 字段”；真正缺的是稳定 cast 身份、权威来源/技能/目标与所有退出路径的一致终态。循环动画停止仍由 `cast_emit.rs` 的 skill-id 特判表分散维护，而非注册契约。
- AV 元数据已有 `DuguSkillVisual`、`TuikeSkillVisual`、`WoliuSkillVisual`、`YidaoSkillSpec` 等局部结构，字段与消费路径各异；Baomai/Tuike 仍可同时走 resolver 直发与事件 consumer，证明局部映射不能充当全局唯一真相源。
- #1287 的总纲 §1 基线门已由 `origin/main` commit `9931a3a1fdd5b4d6b38f4da2fce43f400e26bf0d`（PR #1287）满足；这只关闭该历史等待项，不覆盖总纲 §3 Wave 2 的 R5 P1、R6 P2、R2 P1 前置。`dugu.penetrate` 当前也已改为 `visual_for(DuguSkillId::Penetrate)` 驱动 runtime animation/audio（`server/src/combat/dugu_v2/skills.rs:392-416`），旧错接结论已经关闭。

## 接入面

- **进料**：`SkillRegistry`、`TECHNIQUE_DEFINITIONS`、server `Casting`；R5 P1 的 qi 访问器；R6 P1 的 S2C emit builder 与 R6 P2 的通用 router registration API；R2 P1 的 client store 生命周期。
- **出料**：权威 `cast_sync` → client `CastStateStore`/HUD/FPV juice；`SkillAvBinding` → server AV emit 与 client `VfxBootstrap`/`BongAnimationRegistry`/audio recipe/SkillBar 图标。
- **共享类型**：P3 production cutover 同 PR 引入并接通 `SkillRegistration { resolver, audience, cast_mode, definition, av }`，取代裸 `skill_id → SkillFn`；其中 `definition` 持有完整 `TechniqueDefinition` gameplay 元数据，`SkillAvBinding` 是五件套唯一注册入口，禁止提前建立 test-only 平行模型或让 resolver/event consumer 再维护第二份 ID 表。
- **跨仓库契约**：server `CastSessionBegin` / `CastSyncV1` / protobuf `CastSync` / client `CastState` 同步增加会话与施法身份；`agent/packages/schema` 仅按 §8.1 #4 的有限范围偏差同步新增 `CastPlayAnim`/`CastStopAnim` TypeBox variant，并重建 schema dist/generated artifacts；client DTO、proto/JSON 样例、Rust roundtrip、Java handler/store 和 bot 深断言必须同 PR 对拍。天道 agent runtime/推演逻辑不参与。
- **worldview/AV 锚点**：每招独立可辨的 animation/VFX/SFX/HUD/icon 是根 `CLAUDE.md` 红线；audio 保持 Pattern A（使用施法时 `cast_center` 快照，不读取消费时实时 `Position`）。
- **qi_physics 锚点**：本轨不改变扣费、释放或账本语义；P1/P2 只消费 R5 接口，任何 resolver 迁移不得顺手直写 qi。

## P0 — 设计收口 + 吸收清单验真 ✅ 2026-08-03

### P0.0 Round 10 收敛分类

| finding | 分类 | 本轮收口 |
|---|---|---|
| `CastSessionBegin.target_player` 缺语义绑定测试 | **BLOCKING** | BEGIN 增加当前 tracking epoch 的 protocol entity ID，P1/P4 以 entity ID→UUID 对拍 owner/observer、跨玩家错绑和 session replacement |
| 同连接 session 重进可复活旧 cast | **BLOCKING** | BEGIN 增加 per-tracking floor；低于 floor 的同 session payload fail-closed，exhausted 用空 floor 拒绝全部 payload |
| #1287 基线与总纲前置表述冲突 | **ALIGNMENT** | §8.1 #3 记录已合入 commit，并声明不覆盖总纲 §3 Wave 2 三项前置 |
| TypeBox 主动 source 越过总纲范围 | **ALIGNMENT** | §8.1 #4 记录有限偏差：Rust/protobuf 决策，TypeBox 仅被动镜像；额外 agent 改动必须回总纲决策 |

本轮无 DEFERRABLE 项；四项均已在 P0 契约或开放问题决议中闭合，P1 不得另行解释或扩域。

### P0.1 全注册集合与玩家可达性普查

集合口径固定为生产 `init_registry()` 与 `TECHNIQUE_DEFINITIONS`，不是文档清单或测试 fixture：

| 技能族 | registry | definitions 命中 | 权威可达性结论 | 五件套现状/本轨动作 |
|---|---:|---:|---|---|
| carrier/anqi v2 | 6 | 6 | 玩家可达 | 已有分散 AV；P3 纳入统一 binding |
| burst_meridian | 4 | 4 | 玩家可达 | 已有分散 AV；P3 纳入统一 binding |
| zhenmai v2 | 5 | 5 | 玩家可达 | AV 存在；`sever_chain` HUD 语义仍错，P3 修 |
| woliu v1/v2/v3 | 11 | 11 | 玩家可达 | 已有分散 AV；P3 纳入统一 binding |
| woliu 虚蚀路径 | 5 | 0 | **玩家定义断链** | 五招 animation 资源也缺失；P3 同时补 definition 与五件套 |
| yidao | 5 | 0 | **玩家定义断链** | resolver 有两段动画及 VFX/audio spec；P3 补权威定义/HUD/icon 后统一注册 |
| dugu v2 | 5 | 0 | **玩家定义断链** | 局部五件套结构存在但正式技能栏/HUD/icon 断链；P3 修 |
| baomai v3 | 6 | 2 | 4 招玩家定义断链 | resolver/event 双源仍在；P2 去重，P3 补 4 条定义 |
| tuike v2 | 3 | 3 | 玩家可达 | `shed` 音频已单源；其余视觉及 `don/transfer_taint` 音频仍双路，P2 收口 |
| sword_basics | 4 | 4 | 玩家可达 | 已有分散 AV；P3 纳入统一 binding |
| cultivation::dugu | 2 | 2 | 玩家可达 | 已有分散 AV；P3 纳入统一 binding |
| dandao | 3 | 0 | **玩家定义断链** | 三招仅局部粒子素材，正式 animation/VFX/SFX/HUD/icon 未闭环；P3 修 |
| sword_path | 5 | 5 | 玩家可达 | 已有独立事件 AV；P3 纳入统一 binding |
| npc-named skills | 3 | 3 | **Player+NPC 双受众**：既在玩家默认 definitions 中，也由 NPC AI 注册调用 | P3 用 `audience=Both` 显式化；玩家侧仍须五件套，NPC caster 使用专属粒子/audio 且明确无玩家骨架动画 |
| morph | 1 | 1 | 玩家可达 | 已有 AV；P3 纳入统一 binding |
| **合计** | **68** | **46** | **22 条 registry-only** | 22 = woliu 虚蚀 5 + yidao 5 + dugu v2 5 + baomai 4 + dandao 3 |

另有 definition-only 三条 `movement.dash`、`shield_block`、`body.guangbo_ticao`，它们走专用 intent/system 而非 `SkillRegistry`。P1 不创建或审计任何 registration 形状；P3 在完整 definition/AV 可验后首次落地 `cast_mode=Dedicated`，并与全部 resolver 一起原子切换到统一 registration，最终启动审计断言每条 player definition 恰有一个 resolver 或 dedicated handler。

本矩阵中的“五件套已有”只表示当前代码能找到对应局部映射/资产，不代表已由机器证明唯一消费。P1 建立新 schema/wire 但不切换生产 registration；P2 清除双源并补齐所有退出终态，同时按本阶段测试矩阵锁住双发与终态；P3 补齐已知资产/定义缺口后，一次性把全部 68 resolver + 3 dedicated 原子迁入统一 registration 并删除旧 canonical 表。P3 的 registry 精确集合测试逐条验证完整 definition 以及 animation、VFX/audio/HUD 的 start/release/complete/interrupt phase binding、icon 均非空且真实存在，缺口数必须为零。

### P0.2 `SkillAvBinding` 冻结

P3 production cutover 的数据形状冻结为：

```rust
enum SkillCastMode {
    Resolver,
    Dedicated { handler: DedicatedHandlerId },
}

struct SkillRegistration {
    resolver: Option<SkillFn>,
    audience: SkillAudience,
    cast_mode: SkillCastMode,
    definition: TechniqueDefinition,
    av: SkillVisualBinding,
}

enum SkillVisualBinding {
    Player(SkillAvBinding),
    Npc(NpcVisualBinding),
    Both { player: SkillAvBinding, npc: NpcVisualBinding },
}

struct TechniqueDefinition {
    id: &'static str,
    display_name: &'static str,
    grade: &'static str,
    description: &'static str,
    required_realm: &'static str,
    required_meridians: &'static [TechniqueRequiredMeridian],
    required_race: RaceGate,
    qi_cost: f32,
    stamina_cost: f32,
    cast_ticks: u32,
    cooldown_ticks: u32,
    range: f32,
    category: SkillCategory,
}

struct SkillAvBinding {
    animation: SkillAnimationBinding,
    vfx: SkillAvPhaseBinding,
    audio: SkillAvPhaseBinding,
    hud: SkillAvPhaseBinding,
    icon: SkillIconBinding,
}

struct SkillAvPhaseBinding {
    start: Option<&'static str>,
    release: Option<&'static str>,
    complete: Option<&'static str>,
    interrupt: Option<&'static str>,
}

struct SkillAnimationBinding {
    start: &'static str,
    release: Option<&'static str>,
    looping: bool,
}

enum SkillIconBinding {
    Asset(&'static str),
    ExplicitPlaceholder { asset: &'static str, blocker: &'static str },
}
```

`SkillRegistration.definition` 是所有 gameplay/技能栏元数据的唯一 owner；现有 `TechniqueDefinition.icon_texture` 删除，图标只来自 `av.icon`。`TECHNIQUE_DEFINITIONS`、`TECHNIQUE_IDS` 和 skillbar snapshot 均由 registration 投影生成，不再保留手写 canonical 数组。`cast_mode=Dedicated` 使用 `resolver=None`，但仍须携完整 definition、官方 handler 标识与玩家 AV binding。`SkillAvBinding.vfx/audio/hud` 的每个 phase 槽位明确声明该效果是否在 `start`、权威 `release`、`complete` 或 `interrupt` 发射；空槽表示该阶段不适用，消费者只读 binding，不得在 resolver、router 或 skill-id 特判表里补阶段语义。`release` 与 `complete` 仅 COMPLETE 可消费，`interrupt` 仅 INTERRUPT 可消费；任何 INTERRUPT 必须跳过 release/complete。

约束：

1. `SkillRegistry::register` 改收完整 `SkillRegistration`；同一 `skill_id` 或同一 cast 的多个 emit owner 均启动 fail-fast。不同玩家技能不得复用任一完整 AV 通道：任意两个不同的 Player/Both 技能，其 animation、VFX、SFX、HUD feedback、icon 五个绑定值必须逐字段全部不同；单个底层素材只有在不落入玩家技能绑定通道时才可复用（否则无法辨招）。resolver 只发送“技能已接受/命中/结算”领域事件，不得直发绑定中的 animation/VFX/audio，唯一 AV consumer 按 registration 发射；每个 `SkillAvPhaseBinding` 槽位只能由该唯一 consumer 按其声明 phase 发射。
2. `definition.id` 必须等于 registration key；resolver 模式要求 `resolver=Some`，Dedicated 模式要求 `resolver=None` 且恰有一个经启动审计确认的官方 handler。所有 definition 字段均来自 registration；旧 `TECHNIQUE_DEFINITIONS`/`TECHNIQUE_IDS` 只允许成为派生只读视图，不得再手写条目。
3. 玩家受众 (`audience=Player|Both`) 五字段全部必填并验证真实 client 资源/recipe/handler；纯 NPC 受众显式免除 HUD/icon，animation 不适用时也必须用明确 `NpcVisual` 类型，禁止空串冒充。
4. 占位只允许 `SkillIconBinding::ExplicitPlaceholder`，且必须携 `[BLOCKED: 需 /gen-image ...]` blocker、引用真实占位资产并出现在启动汇总；animation/VFX/audio/HUD 不允许 placeholder 或静默 fallback。P3 归零所有 placeholder 后才可完成。
5. 多阶段招式用 `start + optional release + looping`；施法专用 `CastPlayAnim` 与 `CastStopAnim` 都携完整 `CastIdentity`，STOP 只可停止客户端记录为同一 identity 的 `start` 循环层，release 只在权威完成时播。禁止另建 `looping_cast_anim_id(skill_id)` 特判表。
6. icon 单一真相源迁入 registration 后，由它派生 technique/skillbar/client icon snapshot；不得继续维护同一 skill 的第二份路径字面量。

### P0.3 cast_sync 契约增量冻结

P1 直接升级现有契约，不做 dual-form 兼容层：

```text
CastSessionBegin {
  target_player: string UUID,
  target_entity_id: int32 // 当前 tracking epoch 的 Minecraft protocol entity ID，不是 ECS Entity bits
  session_id: string UUID,
  minimum_cast_instance_id: optional uint64 // JSON/TypeBox/Java bridge 为 optional canonical 十进制字符串
}
```

`CastSessionBegin` 是独立的 S2C 控制消息：加入 `ServerDataEnvelope` 的新 oneof arm，不塞入 `CastSync` 或 `VfxEvent`；proto/Rust/Java/TypeBox（若该层暴露 envelope）必须各有同名字段与 roundtrip sample，且 observer 首次观测/重新进入 tracking 只能通过这条消息建立 gate。`target_player` 必须是该 producer 正在同步的 caster UUID，`target_entity_id` 必须是同一 caster 在 recipient 当前 tracking epoch 中的 Minecraft protocol entity ID；client 仅在 `target_entity_id` 当前解析到的玩家 UUID 等于 `target_player` 时安装 gate，错绑或已卸载 entity ID 一律忽略。`minimum_cast_instance_id` 是本次 tracking epoch 可接纳的最小 cast ID，缺失表示该 session 已 exhausted 且当前没有 active cast、所有 cast payload 均拒绝；若 exhausted session 仍有 active `u64::MAX` cast，BEGIN 必须携该 active ID，允许其既有 CASTING/PLAY/STOP/终态同步，但 server 仍拒绝任何新 cast admission。协议字段号在实现时一次分配并写入 bridge pin，禁止用未知字段或普通 `CastSync` 冒充 BEGIN。

`CastIdentity` 及其余 cast payload 形状如下：

```text
CastIdentity {
  session_id: string UUID,
  cast_instance_id: uint64 // protobuf/Rust；JSON/TypeBox/Java bridge 形状为十进制字符串
}

CastSync {
  identity: CastIdentity,
  source: QUICK_SLOT | SKILL_BAR | DEDICATED,
  skill_id: optional string,
  target: optional CastTargetRef,
  phase: IDLE | CASTING | COMPLETE | INTERRUPT,
  slot, duration_ms, started_at_ms, outcome
}

CastTargetRef = oneof { entity_uuid: string, block: { dimension_id, x, y, z } }
```

1. server 每次连接建立时生成不可复用的随机 UUID `session_id`，并在该连接内从 1 单调分配 `cast_instance_id`（0 保留为空）；每次施法尝试——包括所有前置 reject——恰分配一次。完整 `CastIdentity = (session_id, cast_instance_id)` 贯穿 accepted、complete、interrupt、reject、`CastPlayAnim` 与 `CastStopAnim`，client 以复合 identity 作为幂等、乱序和 supersession 的唯一 key。Rust/protobuf 内部类型固定为 `u64/uint64`；proto3 JSON、TypeBox schema、JSON samples 与 Java bridge 的 `cast_instance_id` **固定为 canonical 十进制字符串**，并使用范围约束 `^(?:[1-9][0-9]{0,18}|1[0-7][0-9]{18}|18[0-3][0-9]{17}|184[0-3][0-9]{16}|1844[0-5][0-9]{15}|18446[0-6][0-9]{14}|184467[0-3][0-9]{13}|1844674[0-3][0-9]{12}|184467440[0-6][0-9]{10}|1844674407[0-2][0-9]{9}|18446744073[0-6][0-9]{8}|1844674407370[0-8][0-9]{6}|18446744073709[0-4][0-9]{5}|184467440737095[0-4][0-9]{4}|1844674407370955[0-0][0-9]{3}|18446744073709551[0-5][0-9]{2}|184467440737095516[0-0][0-9]{1}|1844674407370955161[0-4]|18446744073709551615)$`，禁止 JSON number/coerce-to-double/前导零/符号/空串及大于 `u64::MAX` 的值；Java 用无符号十进制解析并比较（或保留 canonical string），Rust/TS 边界覆盖 `1`、`2^53-1`、`2^53`、`u64::MAX` 且相邻大值不得折叠。allocator 使用 `checked_add`，发出 `u64::MAX` 后将 session 标记 exhausted；后续新请求在 cast-attempt admission 处 fail-closed、不得生成无 identity 的 reject 或任何 gameplay/AV 副作用，连接必须重新建立新 `session_id` 后才能继续施法。该耗尽分支也必须有专门测试，禁止 `wrapping_add`、回到 1 或复用旧 identity。若 `u64::MAX` cast 已被分配且仍 active，exhausted 只禁止新 admission，不得阻止该 active identity 的 CASTING/PLAY/STOP/终态同步；其 tracking BEGIN 仍携 `minimum_cast_instance_id=u64::MAX`，终态后该 session 才完全拒绝 payload。P1 新增显式 `CastSessionBegin { target_player, target_entity_id, session_id, minimum_cast_instance_id }` 控制消息：owner 在 join 初始化收到，observer 在该 caster 首次进入可见范围或 session 更替时收到；server 必须先发 BEGIN，随后才允许发该 session 的 cast/AV payload。`target_player` 必须等于被同步 caster 的权威 `UniqueId`，`target_entity_id` 必须等于该 caster 对 recipient 可见的 Minecraft protocol entity ID，不得从 recipient、可见列表首项或 payload fallback 猜测；client 仅在 entity ID 当前解析出的玩家 UUID 与 `target_player` 相等时安装 gate。首次安装使用当前 active cast 的 ID（若有），否则使用 allocator 的 next ID；同连接重进 tracking 时重新计算同一 floor，因此卸载前已经终止的 cast ID 必然低于新 floor；allocator exhausted 且没有 active cast 时 floor 为 `None`，该 gate 拒绝全部 cast payload，直到重连换新 session；若 active cast 正是 `u64::MAX`，floor 必须为 `u64::MAX` 并允许该 identity 的既有同步。client 只允许 BEGIN 安装/切换每个 caster 的 `{ current_session_id, minimum_cast_instance_id }` gate，安装动作原子清理该 caster 旧 session 的 store/token/tombstone；普通 cast/AV payload 必须同时满足 `session_id == current_session_id`、floor 存在且 `cast_instance_id >= minimum_cast_instance_id`，否则一律 no-op，且不得自行切换 session。重连后 counter 可从 1 重启，但新 `session_id` 不会命中旧 gate；同 session 重进 tracking 后，卸载前 cast 的迟到 PLAY/CASTING/STOP 也无法越过 floor。ECS `Entity` bits 不上 wire；`target_entity_id` 只是已有 Minecraft protocol tracking identity：有 `UniqueId` 的玩家/稳定实体仍发 UUID，方块发维度 + 整数坐标，没有稳定身份的 cast target 省略 `target`。
2. per-caster session gate 生命周期与 client entity tracking 绑定：`ClientEntityEvents.ENTITY_UNLOAD` 对任意 `PlayerEntity` 调 `CastSessionRegistry.evict(player_uuid)`，原子清除该 caster 的 current session/floor、animation ownership、juice token 与 tombstone；本地玩家卸载仍走现有全局 teardown。`CastSessionBegin` 重进视野后以同 session 的新 `minimum_cast_instance_id` 重新安装，未重发 BEGIN、session 不符或低于 floor 的迟到 payload 继续 no-op。全连接 `DISCONNECT/JOIN` 由 `SessionScopedStoreRegistry` 清空 registry。tombstone map 每个 caster/session 固定容量 256，且 terminal high-water mark 与 map 同生命周期并在 tracking eviction 时一并清除；不得用无界 TTL map 替代 tracking eviction。测试循环装载/卸载至少 10,000 个不同 UUID，逐次确认 registry/ownership/token/tombstone/high-water 回到基线；另对一个持续 tracking 的 caster 在同 session 连续完成/中断至少 1,024 个 cast，断言 tombstone 容量恒为 256、最新 256 个 identity 仍受 tombstone/high-water 保护、最早被驱逐 identity 的 delayed PLAY 仍 no-op；并覆盖同连接 session S 下 cast 7 终止→unload→以 floor 8 重进→迟到 PLAY(S,7) 不复活、PLAY(S,8) 可正常武装。
3. `source` 直接取 server `Casting.source`；专用入口使用 `DEDICATED`。P1 删除 `CastSyncHandler.sourceFor()`，不保留按本地 snapshot 猜测的 fallback。
4. `skill_id` 对 SkillBar/DEDICATED 必填，QuickSlot 物品 cast 为 null；`target` 只在 server 已选定稳定目标时携带，无目标不是错误。
5. `CastPhaseV1` 已存在，不增加重复 phase 字段。STOP 是同一 `CastIdentity` 的权威终态副作用：移动、污染、控制、用户取消、死亡、逃劫与换维度均在 owner 仍连接时发 `INTERRUPT + outcome`；断线在 server 内先结束 cast 并向旁观者广播 STOP，owner 侧由 R2 disconnect teardown 清 store（不伪称能给已断开的连接回包）。client 收到终态后停止 binding 的 looping start animation；VFX schema 新增施法专用 `CastPlayAnim { identity, target_player, anim_id, priority, fade_in_ticks }` / `CastStopAnim { identity, target_player, anim_id, fade_out_ticks }` 判别式，client `AnimationLayerManager` 的 cast ownership 记录 `(player_uuid, channel) → { anim_id, identity }`。`CastStopAnim` 只有复合 identity 与当前 owner 完全相等时才停层；旧 A 的延迟 STOP 遇到同招新 B 必须 no-op。STOP 或任一 COMPLETE/INTERRUPT 终态先于 PLAY 到达时必须写入同 identity 的 terminal tombstone；每个 caster/session 的 tombstone map 固定容量为 256，保留最新 terminal identities，驱逐旧 map entry 不能丢失该 session 的 `terminal_high_water_mark`。该单调 high-water mark 在观察到 terminal identity `n` 时推进，任何 `cast_instance_id <= mark` 的 PLAY/CASTING 都先行 no-op，因此被 map 驱逐的 delayed PLAY 也不能复活；只有高于 mark 且未命中 tombstone 的 identity 才可进入正常 gate。后续同 identity `CastPlayAnim` 必须先查 high-water/tombstone 并 no-op，不能重新武装 animation 或 juice。既有通用 `PlayAnim`/`PlayAnimInline`/`StopAnim` 保持非 cast API，不与 cast ownership 互停。VFX cast STOP 仍只是终态派生的 transport 副作用，不能成为独立状态真相源。
6. 前置拒绝仍用 `IDLE + Reject*`，但必须携新 identity/source/skill；新增 `RejectSkillConfigInvalid`，覆盖缺配置、缺字段与非法字段，不插入 `Casting`、不扣费、不写 cooldown。每个拒绝分支的 wire pin 必须同时断言 `phase=IDLE`，禁止只校验 `Reject*` outcome 而放过 CASTING/COMPLETE/INTERRUPT。
7. `target` 不承载 FPV 手臂姿态；R9 只迁移 `plan-fpv-cast-av-v1` 当前临时 `(slot, startedAtMs)` identity 到完整 `CastIdentity`，保留其 accepted-only juice 与 teardown 语义，FPV 动画资产仍归独立 plan。R9 冻结 `ServerDataRouter` 所需的 `CastPlayAnim`/`CastStopAnim` payload 与 `CastFovController.onAnimPlayed(identity, target_player, anim_id)` 接缝，并通过 R6 P2 的通用 registration API 成对注册 PLAY/STOP 分发（R6 的 `ServerDataRouter` 注册表与通用 API 接入面见 `docs/plans-skeleton/plan-refactor-wire-s2c-v1.md:23,36`）：PLAY 在 animation bridge 成功后武装 token，STOP 仅按匹配 identity 停层并消费 token。controller 的 pending/anim token/terminal map 全部以复合 identity 为 key，`CastStopAnim`/终态消费同一 token。
8. VFX cast identity 由 Rust/protobuf 冻结形状，并被动同步到 `agent/packages/schema/src/vfx-event.ts` TypeBox 镜像、schema dist/generated artifacts、Java DTO 与 roundtrip sample；该有限范围偏差按 §8.1 #4 执行。`cast_stop_semantics` 必须覆盖“同玩家同 anim：A STOP 迟到、B 仍播放”和“重复 STOP 幂等”两条乱序回归。非 cast 驱动的通用动画继续使用既有 `PlayAnim`/`StopAnim` 与独立 ownership，不得伪造 cast identity 或进入 cast STOP 通道。

### P0.4 吸收清单第一性原理裁决

| plan | 2026-08-03 裁决 | R9 落点 |
|---|---|---|
| `dugu-v2-technique-definition-gap` | **仍成立**：5 resolver 全部 registry-only | P3 补定义/HUD/icon/回归 |
| `woliu-voidpath-missing-animations` | **仍成立且范围扩大**：五招 animation 缺失，同时 registry-only | P3 补定义与五件套 |
| `dandao-basic-skillbar-bridge` | **仍成立**：三 resolver registry-only；只有局部粒子素材 | P3 完整接入 |
| `dugu-v2-hud-skill-hint` | **仍成立**：局部 `hud_hint` 未进入 skillbar/runtime HUD 契约 | P3 由 binding 下发/渲染 |
| `skillbar-cast-source-drift` | **仍成立**：wire 无 source，client `sourceFor()` 猜测 | P1 权威 source + identity |
| `skillconfig-castsync` | **仍成立**：配置 fail-close 分支无纠正回执 | P1 新 reject outcome |
| `zhenmai-sever-marker-hud` | **仍成立**：client 固定显示“断链增幅”，无 amplification 语义分支 | P3 修 payload/HUD |
| `baomai-v3-av-double-source` | **仍成立**：resolver 直发与 `BaomaiSkillEvent` consumers 并存 | P2 唯一 consumer |
| `dugu-penetrate-av-mismatch` | **已关闭，不再实施**：当前 runtime 已取 `visual_for(Penetrate)` 的针掷/针嘶映射 | P4 只归档并记录现状证据 |
| `meditate-sit-leg-pitch` | **仍成立**：`meditate_sit.json` 双腿 pitch 为 -1.396rad（约 -80°），超过约 40°红线 | P2 调低 pitch、以 bend 承担折腿并 headless 回归 |
| `tribulation-fled-brace-stop` | **仍成立**：Fled 生命周期未形成绑定式权威 STOP | P2 接统一终态 |
| `tuike-v2-duplicate-av` | **部分关闭**：`shed` 主动签名音已改由事件单源；`don/transfer_taint` 音频及三招视觉仍同时有 resolver 直发和事件 consumer | P2 仅修剩余双源，不回退已完成项 |
| `combat-event-juice-runtime-bridge-gap` | **仅部分吸收**：吸收 cast identity/source/phase/target 与施法 juice 所需字段；命中侧 UUID/school/direction/kill 富化仍归原 plan | P1/P4 限定 cast 子域 |

`plan-fpv-cast-av-v1` **不吸收**：它已有实质进度并独立收尾；R9 P1 只在共享 `CastStateStore`/wire identity 处迁移对齐，不接管 FPV 手臂动画、signature 音频资产或其验收。

原吸收表全部已在本节收口；实施与归档以本裁决为准，不以旧 skeleton 行号或旧结论为准。

## P1 — cast wire/juice 生产闭环 ⬜

- `CastSessionBegin`、`CastSyncV1`、protobuf `CastSync`、Rust convert/sample、Java `CastState`/handler/store 同步升级到完整 `CastIdentity`；`agent/packages/schema/src/vfx-event.ts`、dist/generated artifacts、Rust schema/protobuf payload 与 Java consumer DTO 同步落地 `CastPlayAnim`/`CastStopAnim`。R9 走 R6 已冻结的 builder/bridge API，不编辑 R6 独占 router 区段；删除 client source heuristic。
- 生产 cast owner 从每连接 `CastSession { session_id, next_cast_instance_id }` 分配 identity；accepted/complete/interrupt/reject 主路径均真实 emit 并由 client store 消费。R6 P2 先冻结通用 router registration API；R9 在 P1 提供 `AnimationLayerManager`/`CastFovController` 的 identity-aware consumer API，并通过该扩展点成对注册 `CastPlayAnim` 与 `CastStopAnim` 分发，不编辑 R6 独占 router 区段。R6 P2 与 R9 P1 都合入后生产链才算可达：动画 bridge 成功后以同一 identity 武装 juice token，终态/STOP 经 `CastStopAnim` 注册项以同一 identity 停止层并消费 token。
- P1 同 PR 必过跨层契约测试，不推迟到 P4：
  1. **wire shape matrix**：Rust/TypeBox/protobuf/Java 正反 roundtrip；`CastSessionBegin.session_id`、`CastSessionBegin.target_player` 与 `CastIdentity.session_id` 各自在 schema、bridge、DTO、handler 边界覆盖合法 UUID roundtrip，并拒绝空值、非 UUID 与非字符串；`target_entity_id` 覆盖合法 int32、错绑另一玩家、未知/已卸载 entity ID 与类型错误；`CastSessionBegin.minimum_cast_instance_id` 在存在时与 identity 共用 canonical uint64 边界，缺失只允许表示 allocator exhausted 且 client gate 必须拒绝全部 cast payload。另做语义绑定测试：owner A 与 observer B 收到的 BEGIN 均必须让 `target_entity_id` 当前解析到 A 且 `target_player=A`，只安装到 A 的 entity tracking record，随后 A 的 cast/AV payload 才获准；伪造 `target_player=B`、把 A 的 UUID 绑定到 B 的 entity ID、或引用未知/已卸载 ID 的 BEGIN 均不得为 A 或 B 安装/授权 gate。session replacement 后旧 session BEGIN/payload 仍拒绝，新的 BEGIN 也不得借另一玩家的 UUID/实体 ID 覆盖其 gate。`cast_instance_id` JSON 必须为 canonical decimal string，覆盖 `1`、`2^53-1`、`2^53`、`u64::MAX`、相邻大值去重，以及拒绝 number/0/负数/符号/前导零/overflow/非数字字符串。同步执行 `buf breaking --against '../.git#branch=origin/main,subdir=proto'`（或仓库等价的 main 基线命令），并对新增 `ServerDataEnvelope` oneof arm、字段号、samples 与生成 artifacts 记录通过证据；缺失或不可验证基线必须失败，不得跳过。
  2. **session lifecycle matrix**：`CastSessionBegin` 必须先于同 session 的 owner/observer cast payload；未 BEGIN、旧 session、普通 payload 试图切换 session 均拒绝；远端 `PlayerEntity` unload 清除该 caster 全部 session/floor/ownership/token/tombstone，10,000 个 UUID churn 后容量回基线，重进视野须新 BEGIN。专门覆盖同连接 session S 下 cast 7 终止→unload→BEGIN(S,floor=8)→迟到 PLAY/CASTING/STOP(S,7) 全部 no-op，且首个 PLAY(S,8) 正常武装；若卸载时 cast 8 仍 active，则 BEGIN floor=8 并允许该 active cast 继续同步。
  3. **source/target matrix**：逐一 roundtrip/bridge/handler 断言 `QUICK_SLOT`、`SKILL_BAR`、`DEDICATED`；`target=None`、`entity_uuid`、`block { dimension_id, x, y, z }` 三种合法形状；拒绝 entity+block 同时出现、空 oneof wrapper、非法 UUID、空 dimension、坐标越界/类型错误。每个 source 同时 pin `skill_id` 必填/为空规则，禁止 enum fallback 或 unknown 映射。
  4. **production consumer matrix**：R9 侧 server producer → wire DTO → client handler/store，以及经 R6 通用 registration API 成对注册的 `CastPlayAnim`/`CastStopAnim` → identity-aware `AnimationLayerManager` + `CastFovController` 分发链深断言；PLAY 成功武装同 identity token，STOP 只有 identity 匹配时停止层并消费 token。R6 P2 独立 contract test 只冻结通用 router registration API，R9 P1 提供两个 cast-specific registration fixture/API pin。R9 PR 不编辑 router；A/B supersession、迟到/重复 STOP、terminal-before-PLAY（STOP/终态先到时写入 terminal tombstone，随后同 identity PLAY 必须先查 terminal high-water/tombstone 并 no-op，不能重新武装 animation/juice；同 caster/session 连续 1,024 个 terminal cast 后容量仍固定 256，最早被驱逐 identity 的 delayed PLAY 仍 no-op）以及断线 tombstone 后新 session 首次 cast 可正常武装。
  5. **config-reject conservation matrix**：缺整份配置、每个 mandatory 字段分别缺失、每类非法字段（非法 enum/id、越界数值、缺失资源）逐项触发 `RejectSkillConfigInvalid`；每一 case 对比 before/after 快照，断言 reject sync 的 `phase` 固定为 `IDLE` 且 `outcome` 为对应 `Reject*`，`Casting` 不存在、qi 与 qi ledger 均不变、stamina 不变、cooldown map/事件不变、inventory/target 状态不变、animation/VFX/audio/HUD 事件均为 0，只允许 allocator +1 与一个 reject sync。任何新 config validator 分支必须加入该矩阵。
  6. **allocator exhaustion matrix**：先发出 `u64::MAX`，再提交下一次请求；断言 checked overflow 进入 fail-closed admission，不发 0/回绕值/无 identity reject，不写 gameplay 或 AV 副作用，并要求新连接以新 `session_id` 从 1 恢复。若 `u64::MAX` cast 在耗尽前已 active，先 unload/re-entry，断言 BEGIN 携 `minimum_cast_instance_id=u64::MAX` 且该 cast 的 CASTING/PLAY/STOP/终态仍可同步；终态后及任何新请求均拒绝，直到新 session。
- P1 同 PR 固定 allocator 生命周期：0 永不发出；每个 session 首次为 1；accepted 与**每一种 reject outcome/validator branch**混排时均恰递增一次且不复用；同一尝试的所有 phase/AV 共用 identity；新连接生成不同 `session_id` 且 counter 从 1 重启；旧 session tombstone 不得吞新 session 的 `(new_session, 1)`。
- P1 同 PR 新增并执行 `scripts/bot/scenarios/cast_wire_identity.py` 作为生产接线主验收门：走真实施法入口与 live `bong:server_data` envelope，先观察 `CastSessionBegin`，再断言同 identity 的 cast payload、`CastPlayAnim` 与 `CastStopAnim` 可达；owner/observer 路径均断言 BEGIN.target_entity_id 当前解析到实际 caster 且 BEGIN.target_player 等于其 UUID，UUID/实体 ID 任一错绑另一玩家、未知或已卸载 ID 均不安装 gate，session replacement 后旧 BEGIN/payload 不得覆盖新 gate；覆盖未 BEGIN/旧 session 拒绝、A→B 后迟到 A STOP 不停 B、B STOP 停层及重复 STOP 幂等，并覆盖 terminal-before-PLAY：先投递 STOP/终态，再投递同 identity PLAY，必须保持 tombstone、不得重新出现动画或 juice；再覆盖同 session 重入 floor：已终止 `(S,7)` 在 unload/re-entry 的 BEGIN(S,8) 后不得复活，而 `(S,8)` 正常播放。该场景必须与 producer/wire/PLAY+STOP registration 同 PR 常绿，不得推迟到 P4；P4 只原样复跑并扩展其余矩阵。
- `SkillRegistration` / `SkillVisualBinding` 不在 P1 创建 test-only 平行模型；完整类型、validator、producer 与 consumer 一并留到 P3 production cutover。
- 前置：R5 P1、R6 P2、R2 P1 已 merge；R6 P2 必须先落地可扩展的 VFX channel/router registration API，并以 contract tests 固定注册/分发语义，且不依赖 R9 cast payload 或 consumer 类型。该 API 的生产归属是 R6 的 `ServerDataRouter` 注册表与 client network 区段（`docs/plans-skeleton/plan-refactor-wire-s2c-v1.md:23,36`）。随后 R9 P1 通过该 API 成对注册 PLAY/STOP cast consumer，不修改 `ServerDataRouter` 独占文件或区段；若任一前置未满足，只更新本 plan 的依赖状态，不提前复制其职责。

## P2 — 双源清除 + 全退出终态 ⬜

- Baomai/Tuike 剩余 resolver/event AV 双源收敛为各技能唯一领域事件 owner；P2 只锁定现有领域事件 producer/consumer 的 `start/release/complete/interrupt` phase 行为，不创建或读取 P3 才落地的 `SkillRegistration`/`SkillAvBinding`。每个适用 phase 恰发一次，不适用 phase 必须为 0；阶段不得由 skill-id 特判或 resolver/router fallback 补发。P2 不创建 registration 平行模型，避免 Baomai 的 4 条 registry-only definition 缺口制造无生产 consumer 的半迁移状态。
- 接移动、污染、控制、用户取消、tribulation Fled、死亡、断线、换维度全部 STOP/终态路径；P2 完成后才宣称“所有 cast 退出路径”闭环。每条 observer STOP 必须携原复合 identity，覆盖旧 A STOP 不得停止新 B。
- 修 `meditate_sit` 腿 pitch；`dugu.penetrate` 不再改代码，只保留防回归 pin。
- P2 PR 同步落地饱和回归矩阵，不得把保护推迟到 P4：
  1. `p2_av_single_owner` 对 Baomai/Tuike 每个受影响技能、每个适用 AV phase（start/release/complete/interrupt）分别断言唯一领域事件 owner 恰发一次，resolver 直发与 event consumer 不能同时出现；不适用 phase 必须为 0，重复事件或第二 owner 立即失败。
  2. `p2_terminal_state_matrix` 逐一驱动移动、污染、控制、用户取消、Fled、死亡、断线、换维度，断言 owner 的 `INTERRUPT + outcome`、旁观者同 identity 的 `CastStopAnim`、looping 层停止；断线只断言 server/observer 可达效果，不伪造 owner 已断连接收包。
  3. `p2_stop_reordering` 构造同玩家同 `anim_id` 的 cast A/B supersession，验证 A 的迟到 STOP 与重复 STOP 均 no-op，B 的 STOP 才停层；同时覆盖每个终态来源和 observer 广播，防止任一退出分支绕过 identity gate。
  4. `p2_meditate_animation_pin` 用 headless 渲染/姿态断言双腿 pitch 不超过约 40°，bend 承担折腿，且循环动画每个使用轴在 endTick 有同值关键帧。
  5. `p2_av_phase_binding` 对 start/release/complete/interrupt 四个槽位做正反构造：COMPLETE 允许 start+release+complete，INTERRUPT 只允许 start+interrupt；缺槽位不得由 resolver/router/skill-id fallback 补发，complete/interrupt 不得串槽。任何新增退出或 AV owner 分支必须扩展本矩阵。

## P3 — 缺口资产/定义 + 原子全量迁移 ⬜

- **先补前置再注册**：补齐 22 条 registry-only 玩家技能的完整 `TechniqueDefinition`；为 woliu 虚蚀五招和 dandao 三招生成缺失 animation/VFX/SFX/HUD/icon，为 Yidao/Dugu/Baomai 补齐各自缺项。不得以空串或 animation/VFX/audio/HUD placeholder 过渡。
- 在 production cutover 同一 PR 首次落地 `SkillRegistration` / `SkillVisualBinding` 类型和 validator，并立即由 `init_registry()` 生产全部 **68 resolver + 3 dedicated registration**；skill lookup、skillbar/technique projection 与唯一 AV consumer 全部改读该 registry，同时删除旧手写 `TECHNIQUE_DEFINITIONS`/`TECHNIQUE_IDS` canonical 表（兼容调用点只可保留即时派生只读 API）。类型、生产 producer、生产 consumer 与旧源删除不可拆 PR，不存在 test-only 或双表并行阶段。
- 原子切换时启用全量 fail-fast：每条 Player/Both registration 的完整 definition、五件套真实资源、唯一 resolver/dedicated handler 均须通过；精确计数固定 68 resolver + 3 dedicated。注册契约测试固定合法组合为：`audience=Player` 只能配 `SkillVisualBinding::Player`，`audience=Npc` 只能配 `Npc`，`audience=Both` 只能配 `Both`；Resolver 必须有 resolver、Dedicated 必须无 resolver 且有唯一 handler；Player/Both 的 icon 可暂为带 blocker 的 `ExplicitPlaceholder`，但 P3 结束前必须全归零。测试同时穷举这些合法组合与所有非法交叉（resolver option 与 cast mode 不符、audience/binding 不匹配、Both 缺任一侧、Player 缺五件套、Npc 空 visual、definition.id 与 key 不同、dedicated handler 重复/缺失），逐字段 pin 完整 `TechniqueDefinition` 投影，并逐通道扫描 Player/Both 技能的 animation、VFX、SFX、HUD feedback、icon 五个绑定集合：任意两个不同技能在任一通道上复用同一绑定值都 fail-fast，不得只检查完整五元组重复。
- P3 新增 `p3_av_binding_projection`：把 P2 已锁定的每个领域事件 phase 逐条投影到 canonical `SkillAvPhaseBinding`，断言 owner、phase、事件 cardinality 一一对应；COMPLETE 只可消费 release/complete，INTERRUPT 只可消费 interrupt，缺槽位不 fallback，任何 projection 漂移立即失败。

## P4 — bot 验收 + 被吸收 plan 归档 ⬜

1. `cast_registry_reachability`：枚举统一 registration；每条 Player 技能可经官方入口触发，瞬发也必须产生同一 identity 的 accepted + complete，Dedicated 入口按声明触发。
2. `cast_stop_semantics`：移动/污染/控制/用户取消/死亡/逃劫/换维度逐条断言同复合 identity 的 owner 终态与旁观者 `CastStopAnim`；断线断言 server cast 已退出、旁观者 STOP、重连 store 为 idle；额外固定同玩家同 `anim_id` 的 A→B supersession 后延迟 A STOP 为 no-op、B STOP 才真正停层、重复 STOP 幂等，以及 STOP/终态先于 PLAY 到达时先写 tombstone、后续同 identity PLAY 不得复活 animation/juice。
3. `cast_av_uniqueness`：先对统一 registration 做玩家技能逐通道唯一性扫描，Player/Both 技能的 animation、VFX、SFX、HUD feedback、icon 任一通道不得与另一个玩家技能重复；再按 `(CastIdentity, av_kind, phase, emit_owner)` 计数，不把整次 cast 粗暴限制为一个动画。单阶段完成：`start animation=1`，无 release；多阶段完成：`start=1 + release=1`；looping COMPLETE：`start=1 + CastStopAnim=1`，有 release 时再 `release=1`；looping INTERRUPT：`start=1 + CastStopAnim=1`，release 必须为 0。VFX/audio/HUD 依 binding 声明的 start/release/complete/interrupt 槽位各恰一次，COMPLETE 不消费 interrupt，INTERRUPT 不消费 release/complete。每个合法 phase 只允许 registration 指定的唯一 owner emit，重复 owner/重复同 phase 事件为失败；不适用 phase 必须为 0；所有 reject 路径 animation/VFX/audio/HUD/STOP 均为 0。
4. `cast_wire_identity`：原样复跑 P1 已落地的 `scripts/bot/scenarios/cast_wire_identity.py`，并把 P1 的完整 wire/session/source-target/production-consumer/config-reject/allocator-exhaustion 六套矩阵在 P4 e2e 原样重跑，不得缩成代表性样例；protobuf 深断言 `CastSessionBegin` 独立 envelope arm、source/skill/target/复合 identity，并同步复跑 `buf breaking --against '../.git#branch=origin/main,subdir=proto'`，记录 main 基线可验证、samples/generated artifacts 同步与通过证据。覆盖 BEGIN 必须先于 owner/observer payload；owner/observer 的 target_player 必须语义绑定实际 caster，跨玩家错绑和 session replacement 均不得安装/覆盖 gate；未 BEGIN/旧 session/普通 payload 越权切 session均拒绝；同 session unload/re-entry 必须用新 floor 拒绝卸载前终态 cast，同时允许 floor 对应 active/新 cast；另覆盖 0 禁止、每 session 首值 1、accepted + 全 reject 混排逐次 +1、所有 reject sync 均为 `phase=IDLE + Reject*`、`u64::MAX` 后 exhausted floor fail-closed、新连接换 `session_id` 且 counter 重置、terminal-before-PLAY tombstone、同槽连发、无目标、三 source、三 target 形状与 skill-config reject。
5. `cast_av_phase_regression`：P2 的 `p2_av_single_owner`、`p2_terminal_state_matrix`、`p2_stop_reordering`、`p2_meditate_animation_pin`、`p2_av_phase_binding` 五套矩阵在 P4 原样重跑；每条退出路径、每个 Baomai/Tuike 受影响技能和每个 phase 均不得缩成代表样例。
6. `cast_registration_projection`：P3 的 `p3_av_binding_projection` 与完整 registration validator/精确集合测试在 P4 重跑；逐条确认 P2 领域事件 phase → canonical `SkillAvPhaseBinding` 的 owner/cardinality 投影无漂移，68 resolver + 3 dedicated、definition、五件套和 icon placeholder 计数全部对拍。
7. `cast_juice_identity_bridge`：R6 的通用 router registration API 与 R9 的 consumer API/cast-specific PLAY+STOP 注册都合入后，在 P4 e2e 重跑完整生产链；真实 `CastPlayAnim` 经 R6 所有的 `ServerDataRouter` 调用 R9 注册的 PLAY 分发并成功播放后，把同一复合 identity 交给 R9 所有的 `CastFovController`；真实 `CastStopAnim` 经成对注册的 STOP 分发仅在 identity 匹配时停止层并消费 token，覆盖 COMPLETE/INTERRUPT、迟到与重复 STOP。该注册入口属于 R6 P2 的 `ServerDataRouter` 通用 registration API（`docs/plans-skeleton/plan-refactor-wire-s2c-v1.md:23,36`），R9 不直接编辑 router。断线旧 tombstone 后重连首个 `(new_session, 1)` 必须正常武装，旧 session 迟到 PLAY/CASTING/STOP 必须 no-op。
8. runClient 人工验收远处读招、两层 hotbar 归属、HUD hint/icon 及循环动画停止；不能执行 UI 时如实标 blocker，不以单测替代。
9. 逐份归档 P0.4 中 13 份被吸收 plan；已关闭/部分吸收项在 Finish Evidence 记录边界，不篡改历史结论。

## 文件所有权与边界

- **R9 独占**：server cast/AV emit 点、skill registration、`network/cast_emit.rs`；client `CastSessionRegistry`（含 `ClientEntityEvents.ENTITY_UNLOAD` 远端 eviction）、combat cast store、CastFovController 的 identity/tombstone API 与 session cleanup 接线。R9 只定义并测试 router 所需的 `CastPlayAnim`/`CastStopAnim` identity payload/consumer 接口，不直接编辑 `ServerDataRouter`。
- **R6 独占**：client channel registration、`ServerDataRouter`、bridge/router 分发区段；R6 在其 **P2** 落地并测试不依赖 R9 类型的通用 router registration API。R9 P1 随后通过该扩展点成对注册 `CastPlayAnim` → `AnimationLayerManager`/`CastFovController.onAnimPlayed(identity, target_player, anim_id)` 与 `CastStopAnim` → identity-matched layer/token stop consumer，不编辑 R6 独占区段。R9 与 R6 merge 前必须互 fetch；同一文件不同区段仍以总纲矩阵为准。
- **R2 接缝**：`SessionScopedStoreRegistry`/`clearClientStateOnDisconnect` 的登记与通用 teardown 框架归 R2；R9 提供 `CastSessionRegistry.clearOnDisconnect`、`evict(player_uuid)` 和测试，R2 只负责把它登记进清理清单，不由 R9 改 R2 独占生命周期区段。
- **被动 schema 更新（总纲 §0 有限偏差，决议见 §8.1 #4）**：允许且要求修改 `agent/packages/schema/src/vfx-event.ts` 及其 dist/generated artifacts；该 TypeBox 文件仅镜像本 PR 由 proto/Rust 定义的冻结 wire shape，不拥有字段增删或语义决策权，不得改天道 agent runtime、prompt、arbiter 或推演逻辑。
- **只消费不改语义**：R5 qi 访问器、R6 emit builder 与 router 接缝 API、R2 store lifecycle 通用框架；R9 只实现自己的 cast/session 数据和 contract tests。
- **不碰**：FPV 手臂动画与 signature 音频资产；combat hit-event 富化；天道 agent runtime；worldview。
- **Wave 门**：P0 属 Wave 0；P1-P4 属 Wave 2，必须等待 R5 P1、R6 P2、R2 P1；其中 R6 P2 只提供通用 registration API，R9 P1 随后完成 cast-specific 注册。

## §8 开放问题（历史，已收口）

1. `SkillAvBinding` fail-fast 是否容忍占位资源。
2. cast_sync 增量如何与 `plan-fpv-cast-av-v1` 对齐。

全部已在 §8.1 收口。原问题保留以备追溯，**实施时以 §8.1 决议为准**。

补充收口项：#1287 基线与总纲 Wave 2 的关系，以及 TypeBox 镜像是否越过总纲 `server/ + client/` 范围，分别在 §8.1 #3/#4 显式裁决。

## §8.1 决议（pre-P0 收口，2026-08-03）

### #1 占位资源容忍度

**决议**：仅 icon 允许带 blocker 的 `ExplicitPlaceholder`；其它四件套不允许占位、空串或隐式 fallback。P1/P2 不创建 registration 平行模型；P3 先补齐缺失资产，再在同一 production cutover PR 首次落类型/validator、迁入 68 resolver + 3 dedicated、接通所有生产 consumer，并在 P3/P4 验收时把 icon placeholder 归零。

**落点**：`server/src/cultivation/skill_registry.rs:78-95`（现有 register 门）；`server/src/cultivation/known_techniques.rs:128-146`（现有 icon 字段）；plan §P0.2、§P3。

### #2 FPV 对齐窗口

**决议**：R9 不沿用 FPV 因旧 wire 缺字段而采用的 `(slot, startedAtMs)` 临时身份；R6 P2 先落地并测试不依赖 R9 类型的通用 router registration API，R9 P1 再将 cast identity 迁为 server `CastIdentity { session_id, cast_instance_id }`，冻结 `CastFovController` 的 identity-aware consumer/tombstone/session gate API，并通过该扩展点成对注册 `CastPlayAnim` 与 `CastStopAnim` 分发。R6 P2 与 R9 P1 都合入后生产链闭环；R9 不编辑 router，也不接管 FPV 资产/播放实现，避免双 owner 和两个 identity 并存。

**落点**：`server/src/schema/combat_hud.rs:97-106`、`server/src/combat/components.rs:421-447`（R9：`CastSession` allocator + `CastSessionBegin` producer）；`agent/packages/schema/src/vfx-event.ts:67-128,183-189`（R9：cast AV TypeBox variants）；`client/src/main/java/com/bong/client/network/CastSyncHandler.java:19-51,97-103` 与 `client/src/main/java/com/bong/client/combat/juice/CastFovController.java:687-726`（R9：session gate/juice token/tombstone API）；`client/src/main/java/com/bong/client/network/ServerDataRouter.java:111-289`（R6 独占 router 注册表；R6 P2 通用 registration API 的计划归属与 R9 consumer 接入边界见 `docs/plans-skeleton/plan-refactor-wire-s2c-v1.md:23,36`，R9 仅经该 API 注入成对的 `CastPlayAnim`/`CastStopAnim` 分发）；`plan-fpv-cast-av-v1` P3 生命周期契约；总纲 §4 ownership matrix；本 plan §P0.3、§P1、§文件所有权与边界。

### #3 #1287 基线与 Wave 2 前置

**决议**：总纲 §1 要求先合入的 #1287 已由 `origin/main` commit `9931a3a1fdd5b4d6b38f4da2fce43f400e26bf0d` 满足，因此它不再是等待中的依赖；这不构成 R9 提前进入实施的授权。P0 仍按总纲 §3 属 Wave 0，P1-P4 严格等待 R5 P1、R6 P2、R2 P1 全部合入，不以“#1287 已在 baseline”绕过任一 Wave 2 门。

**落点**：`docs/plans-skeleton/plan-refactor-master-v1.md` §1 行 23、§3 行 55；本 plan §现状证据、§P1 前置、§文件所有权与边界。

### #4 TypeBox 被动镜像范围偏差

**决议**：总纲 §0 的默认范围仍是 `server/ + client/`，agent 侧只允许被动 regenerate。当前仓库的 TypeBox 源文件本身是生成 dist/JSON Schema 的输入，无法仅改 generated artifact 而保持 schema 对拍，因此 R9 记录一个**有限且显式的范围偏差**：仅允许在 `agent/packages/schema/src/vfx-event.ts` 镜像本 PR 已由 Rust/protobuf 冻结的 `CastPlayAnim`/`CastStopAnim` wire 字段，并重建 dist/generated artifacts；TypeBox 不拥有字段增删、编号或语义决策权。禁止触碰 tiandao runtime、prompt、arbiter 或其它 agent 行为。若实现发现还需任何额外 agent 源码改动，必须回到总纲另行决策，不能由 R9 静默扩域。

**落点**：`docs/plans-skeleton/plan-refactor-master-v1.md` §0 行 9-10；`agent/packages/schema/src/vfx-event.ts:67-128,183-189`；本 plan §接入面、§P0.3 #8、§P1、§文件所有权与边界。
