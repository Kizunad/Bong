# plan-refactor-cast-av-contract-v1 — 施法同步/技能栏/AV 单一事实源契约（重构轨 R9）

> 所属总纲：`plan-refactor-master-v1.md`。一句话：让每次玩家施法拥有服务端权威身份与完整终态，并把每招动画、粒子、音效、HUD、图标收敛到注册时唯一绑定，消除技能栏断链与 AV 双发、错接、缺失。
>
> 阶段总览：P0 ✅ 2026-08-03；P1 ⬜；P2 ⬜；P3 ⬜；P4 ⬜。

## 现状证据（2026-08-03 P0 复核）

- `SkillRegistry` 当前只保存 `skill_id → SkillFn`，生产初始化共注册 **68** 个 resolver；`TECHNIQUE_IDS` / `TECHNIQUE_DEFINITIONS` 各 **49** 条。两集合交集为 **46**，registry-only **22**，definition-only **3**，说明 resolver、玩家入口、AV 元数据没有共同事实源（`server/src/cultivation/skill_registry.rs:71-122`；`server/src/cultivation/known_techniques.rs:67-166`）。
- server `Casting` 已保存 `source` 与 `skill_id`，但 `CastSyncV1` 只发 `phase/slot/duration_ms/started_at_ms/outcome`；client `CastSyncHandler.sourceFor()` 因此只能从当前快照猜来源并默认 `QUICK_SLOT`（`server/src/combat/components.rs:421-447`；`server/src/schema/combat_hud.rs:97-106`；`client/src/main/java/com/bong/client/network/CastSyncHandler.java:19-51,97-103`）。
- `CastPhaseV1` 已有 `Idle/Casting/Complete/Interrupt`，所以本轨不重复“新增 phase 字段”；真正缺的是稳定 cast 身份、权威来源/技能/目标与所有退出路径的一致终态。循环动画停止仍由 `cast_emit.rs` 的 skill-id 特判表分散维护，而非注册契约。
- AV 元数据已有 `DuguSkillVisual`、`TuikeSkillVisual`、`WoliuSkillVisual`、`YidaoSkillSpec` 等局部结构，字段与消费路径各异；Baomai/Tuike 仍可同时走 resolver 直发与事件 consumer，证明局部映射不能充当全局唯一真相源。
- #1287 已在本分支基线历史中，不再是 P1 前置等待项；`dugu.penetrate` 当前也已改为 `visual_for(DuguSkillId::Penetrate)` 驱动 runtime animation/audio（`server/src/combat/dugu_v2/skills.rs:392-416`），旧错接结论已经关闭。

## 接入面

- **进料**：`SkillRegistry`、`TECHNIQUE_DEFINITIONS`、server `Casting`；R5 P1 的 qi 访问器；R6 P1 的 S2C emit builder；R2 P1 的 client store 生命周期。
- **出料**：权威 `cast_sync` → client `CastStateStore`/HUD/FPV juice；`SkillAvBinding` → server AV emit 与 client `VfxBootstrap`/`BongAnimationRegistry`/audio recipe/SkillBar 图标。
- **共享类型**：P3 production cutover 同 PR 引入并接通 `SkillRegistration { resolver, audience, cast_mode, definition, av }`，取代裸 `skill_id → SkillFn`；其中 `definition` 持有完整 `TechniqueDefinition` gameplay 元数据，`SkillAvBinding` 是五件套唯一注册入口，禁止提前建立 test-only 平行模型或让 resolver/event consumer 再维护第二份 ID 表。
- **跨仓库契约**：server `CastSessionBegin` / `CastSyncV1` / protobuf `CastSync` / client `CastState` 同步增加会话与施法身份；`agent/packages/schema` 作为被动 wire schema source-of-truth 同步新增 `CastPlayAnim`/`CastStopAnim` TypeBox variant，并重建 schema dist/generated artifacts；client DTO、proto/JSON 样例、Rust roundtrip、Java handler/store 和 bot 深断言必须同 PR 对拍。天道 agent runtime/推演逻辑不参与。
- **worldview/AV 锚点**：每招独立可辨的 animation/VFX/SFX/HUD/icon 是根 `CLAUDE.md` 红线；audio 保持 Pattern A（使用施法时 `cast_center` 快照，不读取消费时实时 `Position`）。
- **qi_physics 锚点**：本轨不改变扣费、释放或账本语义；P1/P2 只消费 R5 接口，任何 resolver 迁移不得顺手直写 qi。

## P0 — 设计收口 + 吸收清单验真 ✅ 2026-08-03

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

另有 definition-only 三条 `movement.dash`、`shield_block`、`body.guangbo_ticao`，它们走专用 intent/system 而非 `SkillRegistry`。P1 只建立 `cast_mode=Dedicated` 的统一形状与审计能力，不把它们伪造为 resolver；P3 在完整 definition/AV 可验后与全部 resolver 一起原子切换到统一 registration，最终启动审计断言每条 player definition 恰有一个 resolver 或 dedicated handler。

本矩阵中的“五件套已有”只表示当前代码能找到对应局部映射/资产，不代表已由机器证明唯一消费。P1 建立新 schema/wire 但不切换生产 registration；P2 清除双源并补齐所有退出终态；P3 补齐已知资产/定义缺口后，一次性把全部 68 resolver + 3 dedicated 原子迁入统一 registration 并删除旧 canonical 表。P3 的 registry 精确集合测试逐条验证完整 definition 以及 animation、VFX event、audio recipe、HUD hint、icon 均非空且真实存在，缺口数必须为零。

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
    vfx_event: &'static str,
    audio_recipe: &'static str,
    hud_hint: &'static str,
    icon: SkillIconBinding,
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

`SkillRegistration.definition` 是所有 gameplay/技能栏元数据的唯一 owner；现有 `TechniqueDefinition.icon_texture` 删除，图标只来自 `av.icon`。`TECHNIQUE_DEFINITIONS`、`TECHNIQUE_IDS` 和 skillbar snapshot 均由 registration 投影生成，不再保留手写 canonical 数组。`cast_mode=Dedicated` 使用 `resolver=None`，但仍须携完整 definition、官方 handler 标识与玩家 AV binding。

约束：

1. `SkillRegistry::register` 改收完整 `SkillRegistration`；同一 `skill_id` 或同一 cast 的多个 emit owner 均启动 fail-fast。不同技能可复用单个底层素材，但 Player 技能的完整五元组不得重复（否则无法辨招）；resolver 只发送“技能已接受/命中/结算”领域事件，不得直发绑定中的 animation/VFX/audio，唯一 AV consumer 按 registration 发射。
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
  session_id: string UUID
}

CastIdentity {
  session_id: string UUID,
  cast_instance_id: uint64
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

1. server 每次连接建立时生成不可复用的随机 UUID `session_id`，并在该连接内从 1 单调分配 `cast_instance_id`（0 保留为空）；每次施法尝试——包括所有前置 reject——恰分配一次。完整 `CastIdentity = (session_id, cast_instance_id)` 贯穿 accepted、complete、interrupt、reject、`CastPlayAnim` 与 `CastStopAnim`，client 以复合 identity 作为幂等、乱序和 supersession 的唯一 key。P1 新增显式 `CastSessionBegin { target_player, session_id }` 控制消息：owner 在 join 初始化收到，observer 在该 caster 首次进入可见范围或 session 更替时收到；server 必须先发 BEGIN，随后才允许发该 session 的 cast/AV payload。client 只允许 BEGIN 安装/切换每个 caster 的 current session，安装动作原子清理该 caster 旧 session 的 store/token/tombstone；普通 cast/AV payload 先校验 `session_id == current_session_id`，不相等一律 no-op，且不得自行切换 session。重连后 counter 可从 1 重启，但新 `session_id` 不会命中旧 tombstone，旧 session 的迟到 PLAY/CASTING/STOP 也无法越过 BEGIN gate。ECS `Entity` bits 不上 wire：有 `UniqueId` 的玩家/实体发 UUID；方块发维度 + 整数坐标；没有稳定身份的目标省略 `target`。
2. `source` 直接取 server `Casting.source`；专用入口使用 `DEDICATED`。P1 删除 `CastSyncHandler.sourceFor()`，不保留按本地 snapshot 猜测的 fallback。
3. `skill_id` 对 SkillBar/DEDICATED 必填，QuickSlot 物品 cast 为 null；`target` 只在 server 已选定稳定目标时携带，无目标不是错误。
4. `CastPhaseV1` 已存在，不增加重复 phase 字段。STOP 是同一 `CastIdentity` 的权威终态副作用：移动、污染、控制、用户取消、死亡、逃劫与换维度均在 owner 仍连接时发 `INTERRUPT + outcome`；断线在 server 内先结束 cast 并向旁观者广播 STOP，owner 侧由 R2 disconnect teardown 清 store（不伪称能给已断开的连接回包）。client 收到终态后停止 binding 的 looping start animation；VFX schema 新增施法专用 `CastPlayAnim { identity, target_player, anim_id, priority, fade_in_ticks }` / `CastStopAnim { identity, target_player, anim_id, fade_out_ticks }` 判别式，client `AnimationLayerManager` 的 cast ownership 记录 `(player_uuid, channel) → { anim_id, identity }`。`CastStopAnim` 只有复合 identity 与当前 owner 完全相等时才停层；旧 A 的延迟 STOP 遇到同招新 B 必须 no-op。既有通用 `PlayAnim`/`PlayAnimInline`/`StopAnim` 保持非 cast API，不与 cast ownership 互停。VFX cast STOP 仍只是终态派生的 transport 副作用，不能成为独立状态真相源。
5. 前置拒绝仍用 `IDLE + Reject*`，但必须携新 identity/source/skill；新增 `RejectSkillConfigInvalid`，覆盖缺配置、缺字段与非法字段，不插入 `Casting`、不扣费、不写 cooldown。
6. `target` 不承载 FPV 手臂姿态；R9 只迁移 `plan-fpv-cast-av-v1` 当前临时 `(slot, startedAtMs)` identity 到完整 `CastIdentity`，保留其 accepted-only juice 与 teardown 语义，FPV 动画资产仍归独立 plan。`VfxEventRouter` 的 `CastPlayAnim` 分支必须在 animation bridge 成功后调用 `CastFovController.onAnimPlayed(identity, target_player, anim_id)`；controller 的 pending/anim token/terminal map 全部以复合 identity 为 key，`CastStopAnim`/终态消费同一 token。
7. VFX cast identity 同步修改 Rust schema、`agent/packages/schema/src/vfx-event.ts` TypeBox source、schema dist/generated artifacts、protobuf bridge、Java DTO 与 roundtrip sample；`cast_stop_semantics` 必须覆盖“同玩家同 anim：A STOP 迟到、B 仍播放”和“重复 STOP 幂等”两条乱序回归。非 cast 驱动的通用动画继续使用既有 `PlayAnim`/`StopAnim` 与独立 ownership，不得伪造 cast identity 或进入 cast STOP 通道。

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

- `CastSessionBegin`、`CastSyncV1`、protobuf `CastSync`、Rust convert/sample、Java `CastState`/handler/store 同步升级到完整 `CastIdentity`；`agent/packages/schema/src/vfx-event.ts`、dist/generated artifacts、Rust schema/protobuf bridge 与 Java DTO/router 同步落地 `CastPlayAnim`/`CastStopAnim`。走 R6 builder，删除 client source heuristic。
- 生产 cast owner 从每连接 `CastSession { session_id, next_cast_instance_id }` 分配 identity；accepted/complete/interrupt/reject 主路径均真实 emit 并由 client store 消费。`VfxEventRouter → AnimationLayerManager + CastFovController` 是 P1 必须可达的生产 consumer 链：动画 bridge 成功后以同一 identity 武装 juice token，终态/STOP 以同一 identity 消费。
- P1 同 PR 必过跨层契约测试，不推迟到 P4：Rust/TypeBox/protobuf/Java 正反 roundtrip；`CastSessionBegin` 必须先于同 session 的 owner/observer cast payload，未 BEGIN、旧 session、普通 payload 试图切换 session 均拒绝；server producer → wire bridge → client handler/store 的 source/skill/target/identity 深断言；`CastPlayAnim` → router → `CastFovController` token 的真实 consumer 断言；A/B supersession、旧 session 消息拒绝、断线 tombstone 后新 session 首次 cast 可正常武装。
- P1 同 PR 固定 allocator 生命周期：0 永不发出；每个 session 首次为 1；accepted 与每一种 reject 尝试均恰递增一次且不复用；同一尝试的所有 phase/AV 共用 identity；新连接生成不同 `session_id` 且 counter 从 1 重启；旧 session tombstone 不得吞新 session 的 `(new_session, 1)`。
- `SkillRegistration` / `SkillVisualBinding` 不在 P1 创建 test-only 平行模型；完整类型、validator、producer 与 consumer 一并留到 P3 production cutover。
- 前置：R5 P1、R6 P1、R2 P1 已 merge；若任一未满足，只更新本 plan 的依赖状态，不提前复制其职责。

## P2 — 双源清除 + 全退出终态 ⬜

- Baomai/Tuike 剩余 resolver/event AV 双源收敛为各技能唯一领域事件 owner；同一 `CastIdentity` 每种 AV 恰发一次。P2 不创建 registration 平行模型，避免 Baomai 的 4 条 registry-only definition 缺口制造无生产 consumer 的半迁移状态。
- 接移动、污染、控制、用户取消、tribulation Fled、死亡、断线、换维度全部 STOP/终态路径；P2 完成后才宣称“所有 cast 退出路径”闭环。每条 observer STOP 必须携原复合 identity，覆盖旧 A STOP 不得停止新 B。
- 修 `meditate_sit` 腿 pitch；`dugu.penetrate` 不再改代码，只保留防回归 pin。

## P3 — 缺口资产/定义 + 原子全量迁移 ⬜

- **先补前置再注册**：补齐 22 条 registry-only 玩家技能的完整 `TechniqueDefinition`；为 woliu 虚蚀五招和 dandao 三招生成缺失 animation/VFX/SFX/HUD/icon，为 Yidao/Dugu/Baomai 补齐各自缺项。不得以空串或 animation/VFX/audio/HUD placeholder 过渡。
- 在 production cutover 同一 PR 首次落地 `SkillRegistration` / `SkillVisualBinding` 类型和 validator，并立即由 `init_registry()` 生产全部 **68 resolver + 3 dedicated registration**；skill lookup、skillbar/technique projection 与唯一 AV consumer 全部改读该 registry，同时删除旧手写 `TECHNIQUE_DEFINITIONS`/`TECHNIQUE_IDS` canonical 表（兼容调用点只可保留即时派生只读 API）。类型、生产 producer、生产 consumer 与旧源删除不可拆 PR，不存在 test-only 或双表并行阶段。
- 原子切换时启用全量 fail-fast：每条 Player/Both registration 的完整 definition、五件套真实资源、唯一 resolver/dedicated handler 均须通过；精确计数固定 68 resolver + 3 dedicated。
- 修 zhenmai sever amplification 语义；所有 icon placeholder 清零。
- 每招 animation/VFX/SFX/HUD/icon 精确集合测试 + 视觉/听觉差异化人工回归。

## P4 — bot 验收 + 被吸收 plan 归档 ⬜

1. `cast_registry_reachability`：枚举统一 registration；每条 Player 技能可经官方入口触发，瞬发也必须产生同一 identity 的 accepted + complete，Dedicated 入口按声明触发。
2. `cast_stop_semantics`：移动/污染/控制/用户取消/死亡/逃劫/换维度逐条断言同复合 identity 的 owner 终态与旁观者 `CastStopAnim`；断线断言 server cast 已退出、旁观者 STOP、重连 store 为 idle；额外固定同玩家同 `anim_id` 的 A→B supersession 后延迟 A STOP 为 no-op、B STOP 才真正停层，以及重复 STOP 幂等。
3. `cast_av_uniqueness`：每次 cast 的 animation/VFX/audio 事件计数各等于 1；拒绝路径均为 0。
4. `cast_wire_identity`：P1 契约 gate 在 P4 e2e 重跑，protobuf 深断言 source/skill/target/复合 identity；覆盖 `CastSessionBegin` 必须先于 owner/observer payload、未 BEGIN/旧 session/普通 payload 越权切 session 均拒绝、0 禁止、每 session 首值 1、accepted + 全 reject 混排逐次 +1、同尝试跨 phase/AV 身份一致、新连接换 `session_id` 且 counter 重置、乱序终态、同槽连发、无目标与 skill-config reject。
5. `cast_juice_identity_bridge`：P1 生产链在 P4 e2e 重跑，真实 `CastPlayAnim` 经 `VfxEventRouter` 成功播放后把同一复合 identity 交给 `CastFovController`，匹配 COMPLETE/INTERRUPT/STOP 消费；断线旧 tombstone 后重连首个 `(new_session, 1)` 必须正常武装，旧 session 迟到 PLAY/CASTING 必须 no-op。
6. runClient 人工验收远处读招、两层 hotbar 归属、HUD hint/icon 及循环动画停止；不能执行 UI 时如实标 blocker，不以单测替代。
7. 逐份归档 P0.4 中 13 份被吸收 plan；已关闭/部分吸收项在 Finish Evidence 记录边界，不篡改历史结论。

## 文件所有权与边界

- **R9 独占**：server cast/AV emit 点、skill registration、`network/cast_emit.rs`；client cast handler/store、AV binding bootstrap、`VfxEventRouter` 到 `CastFovController` 的 identity 接线。
- **被动 schema 更新**：允许且要求修改 `agent/packages/schema/src/vfx-event.ts` 及其 dist/generated artifacts；不得改天道 agent runtime、prompt、arbiter 或推演逻辑。
- **只消费不改语义**：R5 qi 访问器、R6 emit builder、R2 store lifecycle。
- **不碰**：FPV 手臂动画与 signature 音频资产；combat hit-event 富化；天道 agent runtime；worldview。
- **Wave 门**：P0 属 Wave 0；P1-P4 属 Wave 2，必须等待 R5/R6/R2 P1。

## §8 开放问题（历史，已收口）

1. `SkillAvBinding` fail-fast 是否容忍占位资源。
2. cast_sync 增量如何与 `plan-fpv-cast-av-v1` 对齐。

全部已在 §8.1 收口。原问题保留以备追溯，**实施时以 §8.1 决议为准**。

## §8.1 决议（pre-P0 收口，2026-08-03）

### #1 占位资源容忍度

**决议**：仅 icon 允许带 blocker 的 `ExplicitPlaceholder`；其它四件套不允许占位、空串或隐式 fallback。P1/P2 不创建 registration 平行模型；P3 先补齐缺失资产，再在同一 production cutover PR 首次落类型/validator、迁入 68 resolver + 3 dedicated、接通所有生产 consumer，并在 P3/P4 验收时把 icon placeholder 归零。

**落点**：`server/src/cultivation/skill_registry.rs:78-95`（现有 register 门）；`server/src/cultivation/known_techniques.rs:128-146`（现有 icon 字段）；plan §P0.2、§P3。

### #2 FPV 对齐窗口

**决议**：R9 不沿用 FPV 因旧 wire 缺字段而采用的 `(slot, startedAtMs)` 临时身份；P1 将其迁为 server `CastIdentity { session_id, cast_instance_id }`，并把 `VfxEventRouter → CastFovController` 生产链与 tombstone/session gate 同 PR 接通，但不接管 FPV 资产/播放实现。迁移与 cast wire 同 PR 完成，避免两个 identity 并存或新 AV variant 绕过 juice consumer。

**落点**：`server/src/schema/combat_hud.rs:97-106`、`server/src/combat/components.rs:421-447`（`CastSession` allocator + `CastSessionBegin` producer）；`agent/packages/schema/src/vfx-event.ts:67-128,183-189`（cast AV TypeBox variants）；`client/src/main/java/com/bong/client/network/CastSyncHandler.java:19-51,97-103`（session gate/store）；`client/src/main/java/com/bong/client/network/VfxEventRouter.java:64-100` 与 `client/src/main/java/com/bong/client/combat/juice/CastFovController.java:687-726`（juice token/tombstone）；`plan-fpv-cast-av-v1` P3 生命周期契约；本 plan §P0.3、§P1、§文件所有权与边界。
