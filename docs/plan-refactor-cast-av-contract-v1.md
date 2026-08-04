# plan-refactor-cast-av-contract-v1 — 施法同步/技能栏/AV 单一事实源契约（重构轨 R9）

> 所属总纲：`plan-refactor-master-v1.md`。一句话：让每次玩家施法拥有服务端权威身份与完整终态，并把每招动画、粒子、音效、HUD、图标收敛到注册时唯一绑定，消除技能栏断链与 AV 双发、错接、缺失。
>
> 阶段总览：P0 ✅ 2026-08-03；P1 ⬜；P2 ⬜；P3 ⬜；P4 ⬜。

## 现状证据（2026-08-03 P0 复核）

- `SkillRegistry` 当前只保存 `skill_id → SkillFn`，生产初始化共注册 **68** 个 resolver；`TECHNIQUE_IDS` / `TECHNIQUE_DEFINITIONS` 各 **49** 条。两集合交集为 **46**，registry-only **22**，definition-only **3**，说明 resolver、玩家入口、AV 元数据没有共同事实源（`server/src/cultivation/skill_registry.rs:71-122`；`server/src/cultivation/known_techniques.rs:67-166`）。
- server `Casting` 已保存 `source` 与 `skill_id`，但 `CastSyncV1` 只发 `phase/slot/duration_ms/started_at_ms/outcome`；client `CastSyncHandler.sourceFor()` 因此只能从当前快照猜来源并默认 `QUICK_SLOT`（`server/src/combat/components.rs:421-447`；`server/src/schema/combat_hud.rs:97-106`；`client/src/main/java/com/bong/client/network/CastSyncHandler.java:19-51,97-103`）。
- `CastPhaseV1` 已有 `Idle/Casting/Complete/Interrupt`，所以本轨不重复“新增 phase 字段”；真正缺的是稳定 cast 身份、权威来源/技能/目标与所有退出路径的一致终态。循环动画停止仍由 `cast_emit.rs` 的 skill-id 特判表分散维护，而非注册契约。
- AV 元数据已有 `DuguSkillVisual`、`TuikeSkillVisual`、`WoliuSkillVisual`、`YidaoSkillSpec` 等局部结构，字段与消费路径各异；Baomai/Tuike 仍可同时走 resolver 直发与事件 consumer，证明局部映射不能充当全局唯一真相源。
- #1287 的总纲 §1 基线门已由 `origin/main` commit `9931a3a1fdd5b4d6b38f4da2fce43f400e26bf0d`（PR #1287）满足；这只关闭该历史等待项，不覆盖总纲 §3 Wave 2 的 **R5 P1 + R6 P2 + R2 P1** release gate。R6 P3 是 §P0.3.2 中 P1-B production cutover 交付门，不是 R9-owned P1-A 的启动门。`dugu.penetrate` 当前也已改为 `visual_for(DuguSkillId::Penetrate)` 驱动 runtime animation/audio（`server/src/combat/dugu_v2/skills.rs:392-416`），旧错接结论已经关闭。

## 接入面

- **进料**：`SkillRegistry`、`TECHNIQUE_DEFINITIONS`、server `Casting`，以及 §P0.3.2/P0.3.3 中 R5 P1、R6 P2/P3、R2 P1 的冻结 handoff；开始门与 cutover 门不得混写。
- **出料**：权威 `CastSessionBegin`/`CastSync`/`CastPlayAnim`/`CastStopAnim` 进入 `bong:server_data` → `ProtoServerDataBridge` → `ServerDataRouter` → R9 cast store/HUD/FPV juice/animation consumer；`SkillAvBinding` → server AV emit 与 client `VfxBootstrap`/`BongAnimationRegistry`/audio recipe/SkillBar 图标。
- **共享类型**：P3 production cutover 同 PR 引入并接通 `SkillRegistration { resolver, audience, cast_mode, definition, av }`，取代裸 `skill_id → SkillFn`；其中 `definition` 持有完整 `TechniqueDefinition` gameplay 元数据，`SkillAvBinding` 是五件套唯一注册入口，禁止提前建立 test-only 平行模型或让 resolver/event consumer 再维护第二份 ID 表。
- **跨仓库契约**：server `CastSessionBegin` / `CastSyncV1` / `CastPlayAnim` / `CastStopAnim` / `ServerDataEnvelope` 与 client `CastState` 同步增加会话与施法身份；具体 source mirror、conversion、generated artifact、DTO、sample 与 bot evidence 按 §P0.3.2/P0.3.3 分 owner 交接，最终在 P1-C 对拍。天道 agent runtime/推演逻辑不参与。
- **worldview/AV 锚点**：每招独立可辨的 animation/VFX/SFX/HUD/icon 是根 `CLAUDE.md` 红线；audio 保持 Pattern A（使用施法时 `cast_center` 快照，不读取消费时实时 `Position`）。
- **qi_physics 锚点**：本轨不改变扣费、释放或账本语义；P1/P2 只消费 R5 接口，任何 resolver 迁移不得顺手直写 qi。

## P0 — 设计收口 + 吸收清单验真 ✅ 2026-08-03

### P0.0 Round 10 收敛分类

| finding | 分类 | 本轮收口 |
|---|---|---|
| `CastSessionBegin.target_player` 缺语义绑定测试 | **BLOCKING** | BEGIN 增加当前 tracking epoch 的 protocol entity ID，P1/P4 以 entity ID→UUID 对拍 owner/observer、跨玩家错绑，并以单调 generation 拒绝 superseded BEGIN |
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
  session_generation: uint64 // server 进程级单调世代；JSON/TypeBox/Java bridge 为 canonical 十进制字符串
  allocator_exhausted: bool // 当前 session 已发出 u64::MAX，禁止新 admission
  active_cast_instance_id: optional uint64 // observer 安装 gate 时 server 已存在的 active；JSON/TypeBox/Java 为 canonical 十进制字符串
  minimum_cast_instance_id: optional uint64 // 最低可同步 identity；JSON/TypeBox/Java 为 optional canonical 十进制字符串
}
```

`CastSessionBegin` 是 `ServerDataEnvelope` 的独立新 oneof arm，不塞入 `CastSync`/`VfxEvent`；proto/Rust/Java/TypeBox（`ServerDataV1` union）须有同名字段与 roundtrip sample。`target_entity_id` 在 recipient 当前 tracking epoch 必须解析到 `target_player`，错绑/卸载 ID 一律忽略。`session_generation` 由进程级 `CastSessionGeneration` 从 1 全局单调分配；同 session 重发复用。BEGIN 仅在 generation 更高时替换 gate；等值只允许同 `session_id`、exhaustion 不回退、floor 不回退且 `active_cast_instance_id` 不增删/改写的幂等更新；其余均 no-op 且不得清现有 store/token/tombstone。字段组合只有 §P0.3.1 列出的四种合法形状：有 active 时 `active_cast_instance_id == minimum_cast_instance_id`；无 active 的 open session 必须有 next floor；无 active 的 exhausted session 两个 optional 字段都缺失。这样 open active、`u64::MAX` active，以及 active A 后 max attempt B 被 reject 三者都能显式表示，client 不从 floor 或 AH 猜 active。allocator `checked_add` 耗尽后拒绝新 admission；server restart 仅因 transport 全断且 R2 在新 BEGIN 前清 gate，才可从 1 重置。字段号一次分配并写 bridge pin，禁止未知字段或普通 `CastSync` 冒充 BEGIN。

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

#### P0.3.1 client 消息全序与 supersession 表（唯一权威）

client 每个 caster 只允许以下七种 session state：`U`（无 gate）、`O`（allocator open、有 floor、无 advertised/client active）、`R`（allocator open、BEGIN 已 advertised active、client 尚未收到其 CASTING）、`A`（allocator open、有 client active）、`X`（exhausted、无 floor/advertised/client active）、`XR`（exhausted、BEGIN 已 advertised active、client 尚未收到其 CASTING）、`XA`（exhausted、有 client active；active 不要求等于 `u64::MAX`）。每个已绑定 session 维护 `{allocator_exhausted, reserved_active_identity, attempt_high_water_mark, latest_attempt_disposition, active_identity, reject_feedback: optional { identity, outcome }, terminal_high_water_mark, terminal_tombstones, animation_owner/token}`；其中 disposition 为 `REJECTED | CASTING | TERMINAL`。比较符号固定如下，表外不得另造排序规则：

- BEGIN：结构合法且字段组合符合下列四形状后才参与比较，否则归 `B!`：open/no-active=`allocator_exhausted=false, active=None, floor=Some(next)`；open/active=`false, active=Some(a), floor=Some(a)`；exhausted/no-active=`true, active=None, floor=None`；exhausted/active=`true, active=Some(a), floor=Some(a)`。`B+` = generation 更高；`B=` = generation 与 session 相同、exhaustion 不回退、floor 不回退且 reserved/active identity 不变；`B-` = generation 更低；`B!` = 结构非法，或等 generation 不同 session、exhausted→open、floor 回退/排除 current identity、增删/改写 advertised active。`U` 中首个合法 BEGIN 视为 `B+`。
- 其它消息：`S=` = session 等于 gate、identity 不低于 floor；`R/XR` 另允许其 `reserved_active_identity`，`XA` 另允许 `active_identity`，即使该 identity 低于后来 reject 推高的 `AH`。`Sl` = exhausted state 中同 session 的 `n=u64::MAX`，且该 identity 尚未被 `AH/TH/disposition` 分类的唯一迟到 final attempt；只允许首个 REJECT/COMPLETE/INTERRUPT/STOP 选择一种 disposition。`Sr` = `X` 中同 session、identity=`AH`、disposition=`REJECTED` 且 feedback identity 相等的 reject 幂等重放。`Sc` = `X` 中同 session，identity 已存在于 terminal tombstone/`TH`、仍是 animation owner，或尚未分类的 `Sl` COMPLETE/INTERRUPT/STOP cleanup。`S≠` = 无 gate、session 不同、低于 floor且不是 reserved/active/`Sl`，或 X 中除 `Sl/Sr/Sc` 外的 payload；`Sl` 一次性分类 final attempt，`Sr` 只重放 reject，`Sc` 只处理 terminal/STOP，均不授权 CASTING/PLAY 或新 admission。
- `n` = incoming `cast_instance_id`，`AH`/`TH` = attempt/terminal high-water。无 advertised active 的新 gate 初始化 `AH=TH=0`、disposition unset；有 advertised active `a` 的新 gate 初始化 `reserved_active_identity=a, AH=a, TH=0, disposition=unset`。新 reject (`n>old_AH`) 将 feedback 替换为 `{identity=n, outcome}`；新非 reject attempt (`n>old_AH` 的 CASTING/COMPLETE/INTERRUPT/STOP) 清除更旧 feedback。terminal/STOP reducer 先保存 `old_AH`，再令 `TH=max(TH,n)`、`AH=max(old_AH,n)`；仅当 `n>=old_AH` 时 disposition=`TERMINAL`，旧 identity 的迟到 terminal/STOP 不得覆盖较新 attempt disposition/feedback。`ACC` 接受并按单元格变更，`IGN` 零副作用，`SUP` 接受并 supersede 所列旧状态；`ACC-idem` 不得二次播放/消费 token。条件自上而下首个匹配，故每个 message × state 恰有一个结果。

| incoming message / identity comparison | `U` | `O` | `R` | `A` | `X` | `XR` | `XA` |
|---|---|---|---|---|---|---|---|
| `CastSessionBegin B+` | `ACC`：按四形状安装→`O/R/X/XR` | `SUP`：清旧 session，安装→`O/R/X/XR` | `SUP`：清 reserved/session，安装→`O/R/X/XR` | `SUP`：停 ownership/token、清旧 session，安装→`O/R/X/XR` | `SUP`：清旧 session，安装→`O/R/X/XR` | `SUP`：清 reserved/session，安装→`O/R/X/XR` | `SUP`：停 ownership/token、清旧 session，安装→`O/R/X/XR` |
| `CastSessionBegin B=` | `IGN`（首个合法 BEGIN 必为 `B+`） | `ACC-idem`：floor 可提高；首次置 exhaustion 且无 active→`X` | `ACC-idem`：保持 advertised active；open→`R`，首次置 exhaustion→`XR` | `ACC-idem`：保持 active；open→`A`，首次置 exhaustion→`XA` | `ACC-idem`：保持`X` | `ACC-idem`：保持`XR` | `ACC-idem`：保持`XA` |
| `CastSessionBegin B-` 或 `B!` | `IGN` | `IGN` | `IGN` | `IGN` | `IGN` | `IGN` | `IGN` |
| 未知 cast variant、字段非法或 identity 缺失 | `IGN` | `IGN` | `IGN` | `IGN` | `IGN` | `IGN` | `IGN` |
| 任意已知非 BEGIN，`S≠` | `IGN` | `IGN` | `IGN` | `IGN` | `IGN` | `IGN` | `IGN` |
| `CastSync(IDLE + Reject*) S=/Sl` 且 `n > max(AH, TH)` | `IGN` | `ACC`：置 `AH/REJECTED/feedback`；`n=max` 时→`X` | `ACC`：置 `AH/REJECTED/feedback`，保留 reserved；`n=max` 时→`XR` | `ACC`：置 `AH/REJECTED/feedback`，保留 active；`n=max` 时→`XA` | `Sl` 时 `ACC`：置 `AH=max`、disposition=`REJECTED`、feedback→`X`，否则 `IGN` | `Sl` 时 `ACC`：置 `AH=max`、disposition=`REJECTED`、feedback并保留 reserved→`XR`，否则 `IGN` | `Sl` 时 `ACC`：置 `AH=max`、disposition=`REJECTED`、feedback并保留 active→`XA`，否则 `IGN` |
| `CastSync(IDLE + Reject*) S=/Sr` 且 `n == AH` | `IGN` | disposition=`REJECTED` 时 `ACC-idem`，否则 `IGN` | disposition=`REJECTED` 时 `ACC-idem` 并保留 reserved，否则 `IGN` | disposition=`REJECTED` 时 `ACC-idem` 并保留 active，否则 `IGN` | `Sr` 时 `ACC-idem` 并保持 feedback→`X`，否则 `IGN` | disposition=`REJECTED` 时 `ACC-idem` 并保留 reserved，否则 `IGN` | disposition=`REJECTED` 时 `ACC-idem` 并保留 active，否则 `IGN` |
| `CastSync(IDLE + Reject*) S=` 且 `n < AH` 或 `n <= TH` | `IGN` | `IGN` | `IGN` | `IGN` | `IGN` | `IGN` | `IGN` |
| `CastSync(IDLE)` 无 `Reject*` | `IGN` | `IGN`（invalid） | `IGN`（invalid，保留 reserved） | `IGN`（invalid，保留 active） | `IGN` | `IGN`（invalid，保留 reserved） | `IGN`（invalid，保留 active） |
| `CastSync(CASTING) S=` 且 identity=reserved/active、无 tombstone，且 `n<AH` 或（`n==AH` 且 disposition 非 `REJECTED/TERMINAL`） | `IGN` | `IGN`（无 reserved/active） | `ACC`：建立 active；`n==AH` 时 disposition=`CASTING`；保留较新 AH/feedback→`A` | `ACC-idem`：保留 active/较新 AH/feedback→`A` | `IGN` | `ACC`：建立 active；`n==AH` 时 disposition=`CASTING`；保留较新 AH/feedback→`XA` | `ACC-idem`：保留 active/较新 AH/feedback→`XA` |
| `CastSync(CASTING) S=` 且 `n > max(AH, TH)` | `IGN` | `ACC`：置 `AH/CASTING`、清旧 feedback、建立 active；`n=max`→`XA`，否则→`A` | `SUP`：清 reserved，置 `AH/CASTING`、清旧 feedback、建立新 active；`n=max`→`XA`，否则→`A` | `SUP`：停旧 ownership/token，置 `AH/CASTING`、清旧 feedback、建立新 active；`n=max`→`XA`，否则→`A` | `IGN` | `IGN`（exhausted） | `IGN`（exhausted） |
| `CastSync(CASTING) S=` 且 `n == AH` | `IGN` | `IGN`（同 identity 已 reject/terminal） | `IGN`（非 reserved 的 AH 已 reject/terminal） | identity=active 且 disposition=`CASTING` 时 `ACC-idem`，否则 `IGN` | `IGN` | `IGN`（非 reserved 的 AH 已 reject/terminal） | identity=active 且 disposition=`CASTING` 时 `ACC-idem`，否则 `IGN` |
| `CastSync(CASTING) S=` 且 `n < AH` 或 `n <= TH` | `IGN` | `IGN` | `IGN`（reserved 例外已由上方首行消费） | `IGN`（active 例外已由上方首行消费） | `IGN` | `IGN`（reserved 例外已由上方首行消费） | `IGN`（active 例外已由上方首行消费） |
| `CastSync(COMPLETE) S=` 且 `n == AH`、disposition=`REJECTED` | `IGN` | `IGN` | `IGN`（保留 reserved） | `IGN`（保留 active） | `IGN` | `IGN`（保留 reserved） | `IGN`（保留 active） |
| `CastSync(COMPLETE) S=` 且 `n > TH` | `IGN` | `ACC`：推进 reducer/tombstone；`n=max`→`X`，否则保持`O` | identity=reserved 时 `ACC` 并清 reserved（`n=max`→`X`，否则→`O`）；否则推进 reducer并保留`R`（`n=max`→`XR`） | identity=active 时 `ACC` 并清 active（`n=max`→`X`，否则→`O`）；否则推进 reducer并保留`A`（`n=max`→`XA`） | `IGN` | identity=reserved 时 `ACC` 并清 reserved→`X`；identity=AH 且非 reject 时 `ACC/ACC-idem` 并保留`XR`，否则 `IGN` | identity=active 时 `ACC` 并清 active→`X`；identity=AH 且非 reject 时 `ACC/ACC-idem` 并保留`XA`，否则 `IGN` |
| `CastSync(COMPLETE) Sc/Sl` | `IGN` | `IGN` | `IGN` | `IGN` | `Sl` 首次到达时 `ACC`：分类 max 为 terminal、写 reducer/tombstone→`X`；已知 `Sc` 为 `ACC-idem` | `IGN` | `IGN` |
| `CastSync(COMPLETE) S=` 且 `n <= TH` | `IGN` | `n==TH` 时 `ACC-idem`，否则 `IGN` | identity=reserved 时 `ACC` 清 reserved→`O`；否则仅 `n==TH` 为 `ACC-idem` | identity=active 时 `ACC` 清 active→`O`；否则仅 `n==TH` 为 `ACC-idem` | `IGN` | identity=reserved 时 `ACC` 清 reserved→`X`；否则仅 identity=AH 且 `n==TH` 为 `ACC-idem` | identity=active 时 `ACC` 清 active→`X`；否则仅 identity=AH 且 `n==TH` 为 `ACC-idem` |
| `CastSync(INTERRUPT) S=` 且 `n == AH`、disposition=`REJECTED` | `IGN` | `IGN` | `IGN`（保留 reserved） | `IGN`（保留 active） | `IGN` | `IGN`（保留 reserved） | `IGN`（保留 active） |
| `CastSync(INTERRUPT) S=` 且 `n > TH` | `IGN` | `ACC`：推进 reducer/tombstone/outcome；`n=max`→`X`，否则保持`O` | identity=reserved 时 `ACC` 并清 reserved（`n=max`→`X`，否则→`O`）；否则推进 reducer并保留`R`（`n=max`→`XR`） | identity=active 时 `ACC` 并清 active（`n=max`→`X`，否则→`O`）；否则推进 reducer并保留`A`（`n=max`→`XA`） | `IGN` | identity=reserved 时 `ACC` 并清 reserved→`X`；identity=AH 且非 reject 时 `ACC/ACC-idem` 并保留`XR`，否则 `IGN` | identity=active 时 `ACC` 并清 active→`X`；identity=AH 且非 reject 时 `ACC/ACC-idem` 并保留`XA`，否则 `IGN` |
| `CastSync(INTERRUPT) Sc/Sl` | `IGN` | `IGN` | `IGN` | `IGN` | `Sl` 首次到达时 `ACC`：分类 max 为 terminal、写 reducer/tombstone/outcome→`X`；已知 `Sc` 为 `ACC-idem` | `IGN` | `IGN` |
| `CastSync(INTERRUPT) S=` 且 `n <= TH` | `IGN` | `n==TH` 时 `ACC-idem`，否则 `IGN` | identity=reserved 时 `ACC` 清 reserved→`O`；否则仅 `n==TH` 为 `ACC-idem` | identity=active 时 `ACC` 清 active→`O`；否则仅 `n==TH` 为 `ACC-idem` | `IGN` | identity=reserved 时 `ACC` 清 reserved→`X`；否则仅 identity=AH 且 `n==TH` 为 `ACC-idem` | identity=active 时 `ACC` 清 active→`X`；否则仅 identity=AH 且 `n==TH` 为 `ACC-idem` |
| `CastPlayAnim S=` 且 identity=active、`n > TH`、无 tombstone | `IGN` | `IGN` | `IGN`（CASTING 尚未建立 active） | `ACC/ACC-idem`：仅首次播放并武装 token | `IGN` | `IGN`（CASTING 尚未建立 active） | `ACC/ACC-idem`：仅首次播放并武装 token |
| 其它 `CastPlayAnim S=` | `IGN` | `IGN` | `IGN` | `IGN` | `IGN` | `IGN` | `IGN` |
| `CastStopAnim Sc` 且 identity=animation owner | `IGN` | `IGN` | `IGN` | `IGN` | `ACC`：停 ownership/token，补齐 tombstone→`X` | `IGN` | `IGN` |
| `CastStopAnim Sc/Sl` 且非 owner | `IGN` | `IGN` | `IGN` | `IGN` | `Sl` 首次到达时 `ACC`：分类 max 为 terminal、推进 reducer/tombstone→`X`；已知 `Sc` 为 `ACC-idem` | `IGN` | `IGN` |
| `CastStopAnim S=` 且 `n == AH`、disposition=`REJECTED` | `IGN` | `IGN` | `IGN`（保留 reserved） | `IGN`（保留 active/ownership） | `IGN` | `IGN`（保留 reserved） | `IGN`（保留 active/ownership） |
| `CastStopAnim S=` 且 identity=animation owner | `IGN` | `IGN`（无 owner） | `IGN`（尚无 owner） | `ACC`：停 owner/token，不单独清 active；`n=max`→`XA`，否则保持`A` | `IGN` | `IGN`（尚无 owner） | `ACC`：停 owner/token，不单独清 active→`XA` |
| `CastStopAnim S=` 且非 owner、`n > TH` | `IGN` | `ACC`：推进 reducer/tombstone；`n=max`→`X`，否则保持`O` | `ACC`：推进 reducer/tombstone并保留 reserved；`n=max`→`XR`，否则保持`R` | `ACC`：推进 reducer/tombstone并保留 active；`n=max`→`XA`，否则保持`A` | `IGN` | identity=reserved/`AH` 时 `ACC/ACC-idem` 并保留`XR`，否则 `IGN` | identity=active/`AH` 时 `ACC/ACC-idem` 并保留`XA`，否则 `IGN` |
| 其它 `CastStopAnim S=` | `IGN` | 仅 `n==TH` 为 `ACC-idem`，否则 `IGN` | identity=reserved 或 `n==TH` 时 `ACC-idem`，否则 `IGN` | identity=active 或 `n==TH` 时 `ACC-idem`，否则 `IGN` | `IGN` | identity=reserved/`AH` 且 `n==TH` 时 `ACC-idem`，否则 `IGN` | identity=active/`AH` 且 `n==TH` 时 `ACC-idem`，否则 `IGN` |

表的直接推论也是验收 oracle：reject B→accepted/rejected C→迟到/重复 reject B 时，accepted C 先清 B feedback，rejected C/D 原子替换自身 feedback，迟到 B 因 `n<AH` 必须 `IGN`；active/reserved A→reject B 时 B 可更新 feedback 但 A 保持，且 BEGIN 先到、B reject 后到的 `XR/XA` 走同一规则；new CASTING C 可 `SUP` A，迟到 A terminal 只清 A/成为 no-op，不能回滚 C 或恢复 B feedback；STOP 只控制 AV ownership，CastSync terminal 才清 active/reserved state；exhausted BEGIN 先到时，唯一 `Sl=max` 由首个 reject 或 terminal/STOP 原子分类，之后仅对应 `Sr/Sc` 幂等重放可达，绝不重新授权 CASTING/PLAY。未知、malformed、identity 缺失或比较失败均按表全 `IGN`。

#### P0.3.2 Wave / dependency 表（唯一权威）

本表区分“总纲允许 R9 开始自有工作”与“跨轨生产 cutover 完成”。R9 全文所有“前置/等待/Wave/合入/可达”表述只引用本表；总纲 §3 与 R6 phase list 必须保持同义，不得另加或删减门。

| work slice | 可开始的 canonical gate | 可宣称完成的 additional gate | 结果/禁止事项 |
|---|---|---|---|
| P0 设计收口 | 总纲 Wave 0，立即 | 本节三张权威表与吸收裁决闭合 | 仅设计，不宣称生产接线 |
| P1-A R9 domain contract | 总纲 Wave 2：**R5 P1 + R6 P2 + R2 P1** 已合入 | R9-owned wire shape、state reducer、domain producer/DTO/TypeBox mirror 与 contract fixtures 冻结，向 R6 交付可实现版本 | 不等待 R6 P3；不得编辑 R6 独占 converter/bridge/router |
| P1-B R6 shared-wire integration | P1-A 契约已冻结，且 R6 已进入 P3 | §P0.3.3 中全部 **R6 P3** artifact/conversion 与其 contract tests 合入：BEGIN/CastSync conversion、VFX side-channel migration、nested dispatch、旧 receiver 删除 | R6 不改 R9 reducer/consumer 语义；未完成时 live path 仍不可宣称可达 |
| P1-C R9 production cutover | P1-B 全部交付已合入 | R9 consumer registration、真实 channel matrices 与 `cast_wire_identity.py` 通过 | 只有此时 P1 才完成；fixture 直调 router 不能替代 live path |
| P2 双源/终态 | P1-C 完成 | P2 五套矩阵通过 | 无额外跨轨门；不得重开 wire 双轨 |
| P3 registration/资产迁移 | P2 完成 | 68 resolver + 3 dedicated 原子 cutover 与 projection tests 通过 | 无额外跨轨门 |
| P4 总验收/归档 | P1-P3 完成 | P1-P3 全矩阵原样复跑、人工/UI blocker 如实记录 | 不得把 P1-B/P1-C 缺口留到 P4 补 |

#### P0.3.3 artifact / conversion ownership 表（唯一权威）

“语义 owner”不等于可以编辑共享接缝文件；下表按可提交 artifact 拆分，每项只有一个 owning track/phase。接入面、P0.3 条款、P1、§文件所有权与边界和 §8.1 只引用本表，不再另行分配 owner。未列的新 artifact 或需要跨 owner 文件的改动必须先回本表/总纲裁决，禁止实施者就地扩域。

| artifact / conversion | concrete symbol / path class | owning track / phase | required evidence / handoff |
|---|---|---|---|
| cast identity 与 attempt/session allocator | `CastIdentity`、`CastSessionGeneration`、`CastSession { session_id, session_generation, next_cast_instance_id, allocator_exhausted }`、server cast admission/emit points | **R9 P1-A** | allocator/session/reject/exhaustion matrices；向 R6 交付冻结字段与样例 |
| cast domain wire declarations | protobuf/Rust 的 `CastSessionBegin`（含 generation/exhaustion/active/floor）message、扩展 `CastSync` fields、`CastPlayAnim`/`CastStopAnim` variants 与 domain DTO | **R9 P1-A** | Rust/protobuf roundtrip、field-number/buf pin；不得在 `proto_convert.rs` 私接 |
| client cast domain DTO 与全序 reducer | Java `CastIdentity`/BEGIN/CastSync/PLAY/STOP DTO、`CastSessionRegistry`、combat cast store、§P0.3.1 reducer | **R9 P1-A** | message×state 全表参数化测试、tracking churn、reject/terminal reorder tests |
| VFX domain TypeBox source mirror | `agent/packages/schema/src/vfx-event.ts` 的 PLAY/STOP variants 与 source-level tests/samples | **R9 P1-A** | TypeBox 正反 samples 与 Rust shape 对拍；仅被动镜像，不在本阶段认领 package-wide generated outputs |
| protobuf generated bindings | P1-A proto declarations 派生的 Rust/Java generated message/oneof bindings | **R6 P3** | 在 shared conversion 前统一 regenerate；生成结果与 field-number/buf/sample pins 对拍 |
| server-data shared envelope declaration/integration | `ServerDataEnvelope` 新 BEGIN arm、扩展 CastSync arm wiring、`ServerDataEnvelope.vfx_event` nested arm/variant integration | **R6 P3** | 消费 P1-A 冻结 message；oneof/field-number/buf breaking + envelope samples |
| Rust shared conversion | `server/src/schema/proto_convert.rs` 中 BEGIN、扩展 CastSync、PLAY/STOP nested envelope 的双向 conversion | **R6 P3** | 每个 arm 正反 conversion test；R9 不编辑该文件 |
| client shared bridge conversion | `ProtoServerDataBridge` 中 BEGIN、扩展 CastSync、`vfx_event` PLAY/STOP DTO conversion/normalization | **R6 P3** | proto→Java 边界值/unknown variant fail-closed tests |
| shared router dispatch | `ServerDataRouter` nested key extraction/dispatch：`vfx_event.cast_play_anim`、`vfx_event.cast_stop_anim` | **R6 P3** | registration/dispatch/duplicate/unknown-key tests；只提供通用扩展点 |
| live VFX channel migration | `bong:vfx_event` → `bong:server_data` producer/channel registration 与 `BongNetworkHandler` 旧 cast receiver 删除 | **R6 P3** | 单通道 contract/e2e；旧 receiver 对两 cast variants 零命中 |
| server-data TypeBox source mirror + package regeneration | `agent/packages/schema/src/server-data.ts` 的 `ServerDataV1` union、BEGIN/CastSync/envelope mirrors；schema package dist/JSON Schema/generated artifacts（含 P1-A VFX source mirror 的最终输出） | **R6 P3** | 与 protobuf/envelope/VFX samples 对拍并执行 package-wide regenerate；仅被动镜像，不改 agent runtime |
| R9 cast-specific consumer registration | R9-owned registration file/consumer：PLAY→`AnimationLayerManager`/`CastFovController.onAnimPlayed`，STOP→identity-matched stop/token consume | **R9 P1-C** | 真实 `ServerDataRouter` 扩展点注册两 key；PLAY/STOP ownership tests |
| FPV juice identity migration | `CastFovController` pending/anim token/tombstone API 从 `(slot, startedAtMs)` 迁为 `CastIdentity` | **R9 P1-C** | accepted-only juice、terminal/STOP/disconnect/reorder matrices；不接管 FPV 资产 |
| R9 production wire tests/bot | 七套 P1 matrices、`scripts/bot/scenarios/cast_wire_identity.py`、P4 原样复跑 | **R9 P1-C / P4** | 走真实 `bong:server_data`，不得以 fixture 绕 transport |
| client lifecycle framework | `SessionScopedStoreRegistry`、`clearClientStateOnDisconnect` 通用登记/teardown 区段 | **R2 P1** | R2 lifecycle contract；R9 只消费 API |
| cast lifecycle adapter | `CastSessionRegistry.clearOnDisconnect`、`evict(player_uuid)` 与 R2 registry registration request | **R9 P1-A** | 10,000 UUID churn、disconnect/join baseline tests |
| R5 qi boundary | R5 P1 qi accessor/ledger API | **R5 P1** | R9 只消费；本 plan 不引入 qi conversion/直写 |
| terminal/AV single-owner migration | Baomai/Tuike domain-event owner、全退出终态、`meditate_sit.json` 修复及 P2 五套 pin | **R9 P2** | phase cardinality、STOP reorder、完整姿态 oracle |
| unified skill registration contract | `SkillRegistration`、`TechniqueDefinition` projection、`SkillAvBinding`/phase binding、validator 与唯一 AV consumer | **R9 P3** | 68 resolver + 3 dedicated 原子 cutover、projection/uniqueness tests |
| missing skill AV/icon assets and bindings | P3 清单中的 animation/VFX/SFX/HUD/icon、真实 client resource/recipe/handler binding | **R9 P3** | 五通道逐技能唯一、placeholder 归零、资源存在性测试 |
| absorbed-plan archive evidence | P0.4 的 13 份 plan Finish Evidence/归档 | **R9 P4** | P1-P3 gates 全绿后逐份记录边界并归档 |

1. server 每次连接建立时生成不可复用的随机 UUID `session_id`，并从进程级 `CastSessionGeneration` allocator 分配全局单调递增的非零 `session_generation`；同一连接/session 的所有 BEGIN 重发复用 generation，换连接或真正 session replacement 才分配下一值，process restart 仅因 transport 全断且 R2 先清 gate 才允许从 1 重置。连接内从 1 单调分配 `cast_instance_id`（0 保留为空）；每次施法尝试——包括所有前置 reject——恰分配一次。完整 `CastIdentity = (session_id, cast_instance_id)` 贯穿 accepted、complete、interrupt、reject、`CastPlayAnim` 与 `CastStopAnim`，client 按 §P0.3.1 处理全部幂等、乱序与 supersession；不得在条款或实现中另造第二套排序规则。Rust/protobuf 内部类型固定为 `u64/uint64`；proto3 JSON、TypeBox schema、JSON samples 与 Java bridge 的 `session_generation`/`cast_instance_id` **固定为 canonical 十进制字符串**，并使用范围约束 `^(?:[1-9][0-9]{0,18}|1[0-7][0-9]{18}|18[0-3][0-9]{17}|184[0-3][0-9]{16}|1844[0-5][0-9]{15}|18446[0-6][0-9]{14}|184467[0-3][0-9]{13}|1844674[0-3][0-9]{12}|184467440[0-6][0-9]{10}|1844674407[0-2][0-9]{9}|18446744073[0-6][0-9]{8}|1844674407370[0-8][0-9]{6}|18446744073709[0-4][0-9]{5}|184467440737095[0-4][0-9]{4}|1844674407370955[0-0][0-9]{3}|18446744073709551[0-5][0-9]{2}|184467440737095516[0-0][0-9]{1}|1844674407370955161[0-4]|18446744073709551615)$`，禁止 JSON number/coerce-to-double/前导零/符号/空串及大于 `u64::MAX` 的值；Java 用无符号十进制解析并比较（或保留 canonical string），Rust/TS 边界覆盖 `1`、`2^53-1`、`2^53`、`u64::MAX` 且相邻大值不得折叠。allocator 使用 `checked_add`，发出 `u64::MAX` 后将 session 标记 exhausted；后续新请求在 cast-attempt admission 处 fail-closed、不得生成无 identity 的 reject 或任何 gameplay/AV 副作用，连接必须重新建立新 `session_id` 后才能继续施法。该耗尽分支也必须有专门测试，禁止 `wrapping_add`、回到 1 或复用旧 identity。若 `u64::MAX` cast 已被分配且仍 active，exhaustion 只禁止新 admission，不得阻止该 active identity 的 CASTING/PLAY/STOP/终态同步。P1 新增的 `CastSessionBegin` 形状、字段合法组合、generation/floor/exhaustion/advertised-active 与七态安装规则只以 §P0.3/§P0.3.1 为准：owner 在 join 初始化收到，observer 在该 caster 首次进入可见范围或 session 更替时收到；server 必须先发 BEGIN，随后才允许发该 session 的 cast/AV payload。`target_player` 必须等于被同步 caster 的权威 `UniqueId`，`target_entity_id` 必须等于该 caster 对 recipient 可见的 Minecraft protocol entity ID，不得从 recipient、可见列表首项或 payload fallback 猜测；client 仅在 entity ID 当前解析出的玩家 UUID 与 `target_player` 相等时安装 gate。tracking 重入时 server 按权威 active/allocator 快照生成四种形状，active A 后 max attempt B 被拒绝必须广告 A 并进入 `XR`，不得把 B 或 floor 猜成 active。client 的 replacement、floor、ordinary payload 与 cleanup 判定全部以 §P0.3.1 为准；普通 payload 不得自行切换 session。新连接的 cast counter 可从 1 重启，但进程内的新 session 必须取得更高 generation；只有 server restart 造成所有 transport 连接断开且 R2 清 gate 后，generation allocator 才可从 1 重置。同 session 重进 tracking 后，卸载前 cast 的迟到 PLAY/CASTING/STOP 也无法越过 floor。ECS `Entity` bits 不上 wire；`target_entity_id` 只是已有 Minecraft protocol tracking identity：有 `UniqueId` 的玩家/稳定实体仍发 UUID，方块发维度 + 整数坐标，没有稳定身份的 cast target 省略 `target`。
2. per-caster session gate 生命周期与 client entity tracking 绑定；消息接收后的 accept/ignore/supersede 与 high-water 更新只按 §P0.3.1。`ClientEntityEvents.ENTITY_UNLOAD` 对任意 `PlayerEntity` 调 `CastSessionRegistry.evict(player_uuid)`，原子清除该 caster 的 current session/generation/floor、animation ownership、juice token 与 tombstone；本地玩家卸载仍走现有全局 teardown。`CastSessionBegin` 重进视野后以同 generation 的新 `minimum_cast_instance_id` 重新安装；未重发 BEGIN、session 不符或低于 floor 的迟到 payload 按表 no-op。全连接 `DISCONNECT/JOIN` 由 `SessionScopedStoreRegistry` 清空 registry。tombstone map 每个 caster/session 固定容量 256，且 terminal high-water mark 与 map 同生命周期并在 tracking eviction 时一并清除；不得用无界 TTL map 替代 tracking eviction。测试循环装载/卸载至少 10,000 个不同 UUID，逐次确认 registry/ownership/token/tombstone/high-water 回到基线；另对一个持续 tracking 的 caster 在同 session 连续完成/中断至少 1,024 个 cast，断言 tombstone 容量恒为 256、最新 256 个 identity 仍受 tombstone/high-water 保护、最早被驱逐 identity 的 delayed PLAY 仍 no-op；并覆盖 §P0.3.1 的 generation replacement、equal-generation invalid BEGIN 与 tracking floor cases。
3. `source` 直接取 server `Casting.source`；专用入口使用 `DEDICATED`。P1 删除 `CastSyncHandler.sourceFor()`，不保留按本地 snapshot 猜测的 fallback。
4. `skill_id` 对 SkillBar/DEDICATED 必填，QuickSlot 物品 cast 为 null；`target` 只在 server 已选定稳定目标时携带，无目标不是错误。
5. `CastPhaseV1` 已存在，不增加重复 phase 字段。terminal 与 STOP 的全部接收顺序、active 清理和 tombstone/high-water reducer 只按 §P0.3.1；任何退出 producer 都必须发同一 `CastIdentity` 的权威终态与相应 AV 副作用。移动、污染、控制、用户取消、死亡、逃劫与换维度均在 owner 仍连接时发 `INTERRUPT + outcome`；断线在 server 内先结束 cast 并向旁观者广播 STOP，owner 侧由 R2 disconnect teardown 清 store。client `AnimationLayerManager` 的 cast ownership 记录 `(player_uuid, channel) → { anim_id, identity }`；既有通用 `PlayAnim`/`PlayAnimInline`/`StopAnim` 保持非 cast API，不与 cast ownership 互停。
6. 前置拒绝仍用 `IDLE + Reject*`，但必须携新 identity/source/skill；新增 `RejectSkillConfigInvalid`，覆盖缺配置、缺字段与非法字段，不插入新 `Casting`、不扣费、不写 cooldown。reject 对 active、feedback、AH/disposition 的影响严格按 §P0.3.1；每个拒绝分支的 wire pin 必须同时覆盖无 active 与 active A 两种前态，并证明 B 不清除 A。
7. `target` 不承载 FPV 手臂姿态；R9 仅实施 §P0.3.3 分配给 R9 的 FPV identity/consumer artifacts，R6 P2/P3 交付的 registration、converter、bridge 与 channel migration 由同表定义。P1 的交接顺序严格按 §P0.3.2 P1-A→P1-B→P1-C，任何一轨不得复制另一轨职责。
8. VFX cast identity、BEGIN/扩展 CastSync、TypeBox mirrors、generated artifacts、Java DTO 与 roundtrip samples 的逐项 owner 只见 §P0.3.3；有限 agent 范围偏差仍按 §8.1 #4。非 cast 通用动画不得伪造 cast identity 或进入 cast STOP 通道。

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

- P1 按 §P0.3.2 的 P1-A→P1-B→P1-C 交接实施，artifact 与可编辑文件只按 §P0.3.3：R9 先冻结 cast domain messages/reducer/DTO/TypeBox VFX mirror；R6 P3 再完成共享 envelope、Rust/Java bridge conversion、server-data TypeBox mirror、nested dispatch 与旧 receiver 删除；R9 最后注册 cast-specific consumers、迁移 FPV juice identity 并删除 client source heuristic。P1-C 通过前不得标记 P1 完成。
- 生产 identity owner 从每连接 `CastSession { session_id, session_generation, next_cast_instance_id, allocator_exhausted }` 分配 identity，generation 来自进程级 allocator；accepted/complete/interrupt/reject 均真实 emit。当前 VFX 是 `vfx_event_emit.rs:344-385` → `bong:vfx_event` → `BongNetworkHandler.java:566-589` → `VfxEventRouter`，故 R6 P2 的 registration API 本身不构成 production reachability；目标生产流与唯一 owner/handoff 以 §P0.3.2/P0.3.3 为准。
- P1-A/B/C 各自在 owning PR 同步提交 §P0.3.3 指定的 contract evidence；P1-C 汇总以下七套跨层矩阵并全部通过，不推迟到 P4：
  1. **wire shape matrix**：Rust/TypeBox/protobuf/Java 正反 roundtrip；`CastSessionBegin.session_id/session_generation/target_player/target_entity_id/allocator_exhausted/active_cast_instance_id/minimum_cast_instance_id` 与 `CastIdentity.session_id/cast_instance_id` 全字段覆盖合法边界，并逐项拒绝空值、非 UUID、number、非 canonical uint64、错绑/未知/已卸载 entity ID。必须正向覆盖 §P0.3.1 四种 BEGIN 形状，反向拒绝 open 无 floor、无 active exhausted 有 floor、active 与 floor 不等、active=0、active/floor overflow；owner A/observer B 的 entity ID 都只能安装到 A。G1→G2 后迟到 G1、等 generation 不同 session、exhaustion/floor/active 回退或改写均 no-op，G2 全状态 byte-for-byte 保持。canonical decimal string 覆盖 `1`、`2^53-1`、`2^53`、`u64::MAX` 与相邻大值去重，拒绝 number/0/负数/符号/前导零/overflow/非数字。同步执行 `buf breaking --against '../.git#branch=origin/main,subdir=proto'`（或仓库等价 main 基线命令），并记录 envelope oneof、字段号、samples 与 generated artifacts；缺失/不可验证基线必须失败。
  2. **ordering/supersession matrix**：把 §P0.3.1 每个 incoming row 参数化跑过 `U/O/R/A/X/XR/XA` 七态，逐格断言 `ACC/ACC-idem/IGN/SUP` 与全部副作用；任何新增 cast variant/state 必须先扩权威表和本矩阵。序列至少包含 reject B→accepted C→迟到/重复 reject B、reject B→reject D→迟到 B、同 identity reject replay、reject 后同 identity CASTING/terminal/STOP fail-closed、active/reserved A→reject B→迟到 A terminal（只清 A、不覆盖 B feedback/disposition）、A→accepted C→迟到 A terminal/STOP（不回滚 C）、terminal-before-PLAY、BEGIN G2 后迟到 G1、equal-generation mismatched session/exhaustion/active/floor，以及 open observer `R→A→O`、exhausted observer `XR→XA→X`、active A→max reject B 后 `A→XA`。对 exhausted BEGIN 先于 max payload 的排列，分别证明首个 max reject/COMPLETE/INTERRUPT/STOP 只能走 `Sl` 原子分类，之后仅匹配的 `Sr/Sc` 幂等重放获准，冲突类型与 CASTING/PLAY 全部 `IGN`。每个 `IGN` 必须证明 store/feedback/high-water/tombstone/ownership/token 全部 byte-for-byte 不变。
  3. **session lifecycle matrix**：`CastSessionBegin` 必须先于同 session 的 owner/observer cast payload；未 BEGIN、旧 session、普通 payload 试图切换 session 均拒绝；远端 `PlayerEntity` unload 清除该 caster 全部 session/generation/floor/ownership/token/tombstone，10,000 个 UUID churn 后容量回基线，重进视野须新 BEGIN。专门覆盖 generation G1→G2 后迟到 BEGIN(G1) 不回滚 gate、等 generation 不同 session 不切换；并覆盖同连接 session S 下 cast 7 终止→unload→BEGIN(S,generation,floor=8)→迟到 PLAY/CASTING/STOP(S,7) 全部 no-op，且首个 PLAY(S,8) 正常武装；若卸载时 cast 8 仍 active，则 BEGIN floor=8 并允许该 active cast 继续同步。
  4. **source/target matrix**：逐一 roundtrip/bridge/handler 断言 `QUICK_SLOT`、`SKILL_BAR`、`DEDICATED`；`target=None`、`entity_uuid`、`block { dimension_id, x, y, z }` 三种合法形状；拒绝 entity+block 同时出现、空 oneof wrapper、非法 UUID、空 dimension、坐标越界/类型错误。每个 source 同时 pin `skill_id` 必填/为空规则，禁止 enum fallback 或 unknown 映射。
  5. **production consumer matrix**：走真实 live channel，不允许 fixture 绕过 transport：server cast producer → `bong:server_data` envelope → `ProtoServerDataBridge` → `ServerDataRouter` registration dispatch → R9 `CastPlayAnim`/`CastStopAnim` consumer → identity-aware `AnimationLayerManager` + `CastFovController`；同时断言旧 `bong:vfx_event` receiver 不再接收这两个 cast variant，避免双轨。逐项对拍 §P0.3.3 的 R6 P3 conversion evidence 与 R9 P1-C registration evidence；PLAY/STOP 状态行为复用 ordering matrix，不在本矩阵另造规则。
  6. **config-reject conservation matrix**：缺整份配置、每个 mandatory 字段分别缺失、每类非法字段（非法 enum/id、越界数值、缺失资源）逐项触发 `RejectSkillConfigInvalid`；每一 case 都跑两种前态：无 active cast 时保持 idle；cast A active 时 attempt B reject 后 server/client 仍保持 A 的 identity、`Casting`、animation/juice/token 与后续 COMPLETE/INTERRUPT 能力。两种前态均断言 B 的 reject sync 固定为 `phase=IDLE + Reject*` 且只更新 attempt feedback，qi/ledger、stamina、cooldown、inventory/target 不变，B 的 animation/VFX/audio/HUD/STOP 为 0，只允许 allocator +1 与一个 reject sync；任何新 validator 分支必须加入该矩阵。
  7. **allocator exhaustion matrix**：先发出 `u64::MAX`，再提交下一次请求；断言 checked overflow fail-closed，不发 0/回绕值/无 identity reject，不写 gameplay/AV，并要求新 session 才能从 1 恢复。覆盖三条 active 分支：max 本身 active 时 BEGIN 为 `exhausted+active=max`；较早 A active、max B reject 时 BEGIN 为 `exhausted+active=A` 并进入 `XR`，随后到达的 B reject 建立 feedback、A CASTING/PLAY 仍建立 active，重复 B 走 `Sr/S=` 幂等；无 active 时 BEGIN 为 `exhausted+active=None+floor=None` 并进入 `X`。前两条的 CASTING/PLAY/STOP/终态仍可同步，终态后全部新 admission/payload（除 `Sr/Sc` 幂等重放）拒绝直到新 session。
- P1-A 固定 allocator 生命周期；P1-C 新增并执行 `scripts/bot/scenarios/cast_wire_identity.py`，并汇总验证 P1-B 的真实 bridge/channel handoff。所有消息接收断言调用 §P0.3.1 对应 table row，不复制判断逻辑；P4 只原样复跑并扩展其余矩阵。
- `SkillRegistration` / `SkillVisualBinding` 不在 P1 创建 test-only 平行模型；完整类型、validator、producer 与 consumer 一并留到 P3 production cutover。
- P1 的 release/cutover/completion 门只见 §P0.3.2；具体 artifact owner 只见 §P0.3.3。未满足 P1-B 时 P1-C 排队但 P1-A 可工作，任一轨不得通过复制 converter/bridge/router/consumer 职责绕门。

## P2 — 双源清除 + 全退出终态 ⬜

- Baomai/Tuike 剩余 resolver/event AV 双源收敛为各技能唯一领域事件 owner；P2 只锁定现有领域事件 producer/consumer 的 `start/release/complete/interrupt` phase 行为，不创建或读取 P3 才落地的 `SkillRegistration`/`SkillAvBinding`。每个适用 phase 恰发一次，不适用 phase 必须为 0；阶段不得由 skill-id 特判或 resolver/router fallback 补发。P2 不创建 registration 平行模型，避免 Baomai 的 4 条 registry-only definition 缺口制造无生产 consumer 的半迁移状态。
- 接移动、污染、控制、用户取消、tribulation Fled、死亡、断线、换维度全部 STOP/终态路径；P2 完成后才宣称“所有 cast 退出路径”闭环。每条 observer STOP 必须携原复合 identity，覆盖旧 A STOP 不得停止新 B。
- 修 `meditate_sit` 腿 pitch；`dugu.penetrate` 不再改代码，只保留防回归 pin。
- P2 PR 同步落地饱和回归矩阵，不得把保护推迟到 P4：
  1. `p2_av_single_owner` 对 Baomai/Tuike 每个受影响技能、每个适用 AV phase（start/release/complete/interrupt）分别断言唯一领域事件 owner 恰发一次，resolver 直发与 event consumer 不能同时出现；不适用 phase 必须为 0，重复事件或第二 owner 立即失败。
  2. `p2_terminal_state_matrix` 逐一驱动移动、污染、控制、用户取消、Fled、死亡、断线、换维度，断言 owner 的 `INTERRUPT + outcome`、旁观者同 identity 的 `CastStopAnim`、looping 层停止；断线只断言 server/observer 可达效果，不伪造 owner 已断连接收包。
  3. `p2_stop_reordering` 构造同玩家同 `anim_id` 的 cast A/B supersession，验证 A 的迟到 STOP 与重复 STOP 均 no-op，B 的 STOP 才停层；同时覆盖每个终态来源和 observer 广播，防止任一退出分支绕过 identity gate。
  4. `p2_meditate_animation_pin` 用 headless 渲染/姿态断言 `meditate_sit.json` 维持直立 torso（`torso.pitch=0`）、垂目 head pitch 约 `+0.2094395rad`（+12°）及双腿目标盘坐姿态：两腿 pitch 必须落在明确的修复区间 `[-0.698132, 0.0]rad`（[-40°, 0°]），双腿 yaw 保持相反符号且绝对值约 `0.436332rad`（25°），双腿 bend 约 `1.570796rad`（90°）承担折腿；不得再出现当前 `-1.3962634rad`（-80°）过旋。循环动画每个使用轴在 endTick 有同值关键帧，P4 复跑同一完整姿态 oracle。
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
4. `cast_wire_identity`：P4 e2e 原样重跑 P1 的 wire/ordering/session/source-target/production-consumer/config-reject/allocator-exhaustion 七矩阵、`cast_wire_identity.py` 与 buf breaking，不得抽样。重点复验 §P0.3.1 全表逐格 oracle，尤其 delayed reject、旧 terminal/STOP 不覆盖较新 attempt、BEGIN freshness、floor/exhaustion/tombstone；generation 只可在 transport 全断并完成 R2 teardown 的 server restart 后重置。
5. `cast_av_phase_regression`：P2 的 `p2_av_single_owner`、`p2_terminal_state_matrix`、`p2_stop_reordering`、`p2_meditate_animation_pin`、`p2_av_phase_binding` 五套矩阵在 P4 原样重跑；每条退出路径、每个 Baomai/Tuike 受影响技能和每个 phase 均不得缩成代表样例。
6. `cast_registration_projection`：P3 的 `p3_av_binding_projection` 与完整 registration validator/精确集合测试在 P4 重跑；逐条确认 P2 领域事件 phase → canonical `SkillAvPhaseBinding` 的 owner/cardinality 投影无漂移，68 resolver + 3 dedicated、definition、五件套和 icon placeholder 计数全部对拍。
7. `cast_juice_identity_bridge`：按 §P0.3.2/P0.3.3 对拍 R6 P2 API、R6 P3 全部 shared-wire artifacts 与 R9 P1-C 两项注册后，重跑 production-consumer matrix；真实 PLAY/STOP 必须走 `bong:server_data` → bridge → `ServerDataRouter` 嵌套 key，旧 `bong:vfx_event` receiver 零命中。
8. runClient 人工验收远处读招、两层 hotbar 归属、HUD hint/icon 及循环动画停止；不能执行 UI 时如实标 blocker，不以单测替代。
9. 逐份归档 P0.4 中 13 份被吸收 plan；已关闭/部分吸收项在 Finish Evidence 记录边界，不篡改历史结论。

## 文件所有权与边界

- 每个新 artifact/conversion 的唯一 owner、phase、handoff 与证据见 §P0.3.3；总纲 §4 冲突时先同步两份权威表，不得靠本节散文改派。跨轨时只消费 owner 已冻结的 API/artifact，merge 前互 fetch。
- Wave release、P1-A/B/C cutover 与后续 phase 门只见 §P0.3.2；R6 P3 是 production handoff，不是 R9 P1-A 启动门。
- TypeBox 修改仅限 §P0.3.3 两项被动 mirror 和 §8.1 #4 的有限偏差，不得触碰 tiandao runtime、prompt、arbiter 或其它 agent 行为。
- **不碰**：FPV 手臂动画与 signature 音频资产；combat hit-event 富化；天道 agent runtime；worldview。

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

**决议**：P1 的 FPV/shared-wire 交接严格按 §P0.3.2 P1-A→P1-B→P1-C；每个 converter、bridge、mirror、consumer 的 owner 只按 §P0.3.3。R9 P1-A 可在 Wave 2 release gate 后冻结 identity/reducer，不等待 R6 P3；R9 P1-C 必须等 R6 P3 shared-wire handoff 可达后才迁 FPV identity 并注册 PLAY/STOP consumer。

**落点**：本 plan §P0.3.1（唯一 ordering reducer）、§P0.3.2（唯一 dependency table）、§P0.3.3（唯一 ownership table）；`plan-fpv-cast-av-v1` P3 生命周期契约；总纲 §3/§4；R6 P2/P3。

### #3 #1287 基线与 Wave 2 前置

**决议**：总纲 §1 的 #1287 已满足。P0 属 Wave 0；R9 P1-A 的 Wave 2 release gate 是 **R5 P1 + R6 P2 + R2 P1**，不得遗漏或增加。R6 P3 不是 Wave 2 release gate，而是 P1-B shared-wire integration；P1-C 必须等待它，且只有 P1-C live e2e 通过后 P1 才完成。全部后续 phase 门见 §P0.3.2。

**落点**：总纲 §3；R6 phase list；本 plan §现状证据、§P0.3.2、§P1、§文件所有权与边界。

### #4 TypeBox 被动镜像范围偏差

**决议**：总纲 §0 的默认范围仍是 `server/ + client/`，agent 侧只允许被动 mirror/regenerate。两份 TypeBox source 与 generated artifacts 的唯一 owner/phase 按 §P0.3.3：VFX domain source mirror 归 R9 P1-A，server-data source mirror 与 package-wide final regeneration 归 R6 P3；均只镜像 Rust/protobuf 已冻结 shape，不拥有字段、编号或语义决策权。禁止触碰 tiandao runtime、prompt、arbiter 或其它 agent 行为；额外 agent source 必须回总纲裁决。

**落点**：总纲 §0；本 plan §P0.3.3、§接入面、§P1、§文件所有权与边界。
