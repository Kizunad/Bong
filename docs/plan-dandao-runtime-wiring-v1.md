# plan-dandao-runtime-wiring-v1 — 丹道流派整链 runtime 接线（register → 变异视觉 → 天道叙事 → 催化炉 → 暴龙王 BOSS）

> **一句话主题**：丹道（毒蛊变异流）从机制到表现层的代码已写齐、测试已锁、client/agent 消费端全就绪，但 **`dandao::register(&mut app)` 在 `run_server()` 从未被调用**——整条流派 runtime 链彻底悬空。本 plan 把这条「写好却没插电」的流派一次性接通：P0 插上 register 总开关 + 服丹 emit、P1 变异视觉 server→client、P2 变异叙事 server→agent、P3 催化炉加成接 resolver、P4 暴龙王 BOSS 端到端遭遇战。
>
> **调查依据**：`gameplay-broken-links-audit`（2026-06-02，81-agent workflow，68 候选 / 67 确认 / 对抗式 opus 验证）theme `dandao-runtime-wiring`。**已过 `validate-dandao-runtime-wiring-plan` 工作流复核（4 维 opus/sonnet 评审 + 综合，2026-06-02）**——下列交付物已据评审 grep 证据修正落点/契约/守恒/worldview。
>
> **关系**：本 plan 收口 `docs/finished_plans/plan-dandao-path-v1.md` 的遗留——该 plan 被归档为 ✅ 但其 P0/P1 runtime system 实际从未 land（`plan-terrain-wiring-v1` §遗留:145 已标注「plan-dandao-path-v1 阶段表 P-1~P5 仍 ⬜ 应回退状态」）。本 plan 不重新设计丹道世界观/数值，只补接线，land 后 plan-dandao-path-v1 的 ✅ 才名副其实。

## 阶段总览

| 阶段 | 主题 | 断链 | 状态 |
|------|------|------|------|
| **P0** | `dandao::register` 接入 `run_server` + 生产服丹路径 emit `PillIntakeTracked`（总开关，解锁全链） | mutation-runtime-not-started / dandao-pill-intake-no-emitter | ⬜ |
| **P1** | 变异视觉 server→client：`MutationAdvanceEvent` → `bong:mutation_visual` payload（变异肢体 GeckoLib 渲染 + 丹毒 HUD） | dandao-mutation-advance-event-no-reader / dandao-mutation-visual-no-emit / mutation-visual-server-emit-missing | ⬜ |
| **P2** | 变异叙事 server→agent：新增 `RedisOutbound::MutationEvent` 发布 `bong:mutation_event` + 两端 schema 对齐 + agent `MutationNarrationRuntime` 启动 | mutation-runtime-not-started | ⬜ |
| **P3** | 催化炉加成接入 alchemy resolver：`catalyst_furnace_bonus` 变异丹成功率加成 | dandao-catalyst-furnace-unused | ⬜ |
| **P4** | 暴龙王 BOSS 端到端遭遇战：spawn + big-brain 集成层（包装现有评分函数）+ 掉落表 + 真元吸取光环（守恒） | dandao-boss-orphan | ⬜ |

> 依赖顺序：**P0 是总开关**——缺它任何下游单点修复都因 `register` 未调而无效，必须最先 land。P1/P2 是 `MutationAdvanceEvent` 的两个独立 reader（视觉 / 叙事），可并行设计、独立成 PR。P3 独立于变异链（炼丹成功率）。P4 体量最大（BOSS 遭遇战 + big-brain 集成层新写），依赖 P0 的 register 但与 P1/P2/P3 解耦。验收日期迁入 `finished_plans/` 时填。

## 接入面（防孤岛）

- **进料**：
  - 生产服丹路径 `handle_alchemy_take_pill`（`server/src/network/client_request_handler.rs:8249`，内部 `consume_pill` 调用点 `:8593`）——P0 在此 emit `PillIntakeTracked`（注意：`alchemy/mod.rs:606` 是 `#[cfg(test)]` 测试调用，**非**生产路径）
  - 变异推进 `dandao::mutation::mutation_advance_system`（`mutation.rs:238`）——读 `&DandaoStyle`，按 `DandaoStyle::stage_for_toxin(style.cumulative_toxin)` 推进 `MutationState`（`mutation.rs:19`）、emit `MutationAdvanceEvent`（`mutation.rs:219`）。**`cumulative_toxin` 由 `DandaoStyle::advance_toxin`（`components.rs:41`）喂入，调用方是 dandao 招式 resolver（已随 `register_skills` 接通）+ `internal_brew`（`internal_brew.rs:65`）+ 化丹为血 progression（`progression.rs:44`）；`PillIntakeTracked` 的 reader `track_pill_intake_system`（`toxin_tracker.rs:26`）只记 `PracticeLog` Mellow（服丹偏温润色权重），不驱动 mutation**
  - alchemy `RecipeRegistry` 变异丹配方（`assets/alchemy/recipes/*.json`，带 `furnace_tier_min:4`）——P3 催化炉加成进料
  - 坍缩渊边缘 zone `baolongwang_cavern_deep`（`server/src/world/zone.rs:1173` 已存在）——P4 BOSS spawn 锚点
- **出料**：
  - **P0** 服丹 emit `PillIntakeTracked` → `track_pill_intake_system` 累积 `PracticeLog` Mellow（温润色）；register 接通后变异链（toxin→stage→`MutationAdvanceEvent`）随既有 toxin 喂入路径开始工作
  - **P1** server→client `bong:mutation_visual` `CustomPayload` → client `MutationFeatureRenderer`（变异肢体 GeckoLib）+ `MutationHudPlanner`（变异阶段 + 丹毒进度条）
  - **P2** server→agent `bong:mutation_event` Redis → 天道 `MutationNarrationRuntime` → `AGENT_NARRATE` → 玩家聊天栏变异阶段旁白
  - **P3** 变异丹炼制成功率加成（进 alchemy 成功率 resolver）
  - **P4** 暴龙王三阶段 BOSS 遭遇战 + 真元吸取光环 + 掉落经济（见 P4 loot 表），客户端走已注册实体 `bong:baolongwang`（raw_id 160）渲染
- **共享类型 / event（复用既有，本 plan 不重新定义；P1/P2 须扩字段者已标注）**：
  - server：`dandao::register`（`mod.rs:33`）/ `PillIntakeTracked`（`toxin_tracker.rs:16`）/ `track_pill_intake_system`（`toxin_tracker.rs:26`）/ `DandaoStyle::advance_toxin`（`components.rs:41`）/ `MutationState`（`mutation.rs:19`）/ `MutationAdvanceEvent`（`mutation.rs:219`）/ `mutation_advance_system`（`mutation.rs:238`）/ `MutationVisualSyncPayload`（`visual_sync.rs:26`，**P1 须扩 `cumulative_toxin` 字段**）/ `MUTATION_VISUAL_CHANNEL = "bong:mutation_visual"`（`visual_sync.rs:14`）/ `catalyst_furnace_bonus`（`catalyst_furnace.rs:18`）/ `BaolongwangBoss`（`boss.rs:22`）/ boss_ai 评分自由函数 `score_*` + `pick_best_action`（`boss_ai.rs:93`，**非 big-brain 组件**）/ `compute_loot`（`boss.rs:129`）+ loot 常量（`boss.rs:121-128`）
  - schema：`MutationStateV1`（`server/src/schema/dandao.rs:62`）/ `MutationEventV1`（`schema/dandao.rs:75`，serde `deny_unknown_fields`）↔ agent TypeBox `MutationEventV1`（`agent/packages/schema/src/mutation-event.ts:22`，`additionalProperties:false`）——**两端字段当前不对齐，P2 须先对齐再对拍（见 P2）**
  - agent：`MutationNarrationRuntime`（`agent/packages/tiandao/src/mutation-narration-runtime.ts:79`，runtime 走 `validateMutationEventV1`@:127）/ `CHANNELS.MUTATION_EVENT = "bong:mutation_event"`（定义于 `agent/packages/schema/src/channels.ts:327`，`redis-ipc.ts:89` 为 destructuring import 端，已在 `REDIS_V1_CHANNELS`）
  - client：`MutationPayloadHandler`（`BongNetworkHandler.java:396` 已 register `bong:mutation_visual` receiver；JSON 读 `cumulative_toxin` fallback 0.0@`MutationPayloadHandler.java:29`）/ `MutationVisualState` / `MutationFeatureRenderer` / `MutationHudPlanner` / `MutationKind`（`fromServerString` 正则按分隔符）/ `MutationInspectLabel`（`translateKind` CamelCase / `translateBodySlot` UPPER）；`BaolongwangEntities`（`EXPECTED_RAW_ID=160`，`BongClient.java:125` 已 register）/ `BaolongwangModel` / `BaolongwangRenderer` / `BaolongwangRenderBootstrap`
- **跨仓库契约**：server（Rust）↔ agent（TS，P2）↔ client（Java，P1/P4）三端齐动。**消费端（client receiver / agent runtime / client 实体渲染）已实装就绪、缺生产端/发射端**——但 P1（payload 字段+大小写）、P2（两端 schema 字段对齐 + 新 RedisOutbound 变体）的契约**对齐缺口须当作必做交付物 + 对拍测试锁定**，不是「纯接线」。
- **worldview 锚点**（已据 worldview.md 复核行号）：丹道毒蛊流 = `worldview.md` §五『战斗流派』(line 397) 之『攻击四流·4. 毒蛊流』(line 423)；变异/身体改造机制 = §五『真元染色：长期身体改造』(line 650) + §十六『负压畸变体』(line 1572，耗真元光环抽玩家真元 +50% 的正典依据)。**暴龙王 BOSS 的数值/设定由已归档 `plan-dandao-path-v1` 锚定，worldview 无直接条目**（若要正典地位须人工单开 worldview PR，不在本 plan 范围）。
- **qi_physics 锚点**：**复用既有 + P4 新增吸取光环须守恒**。`dandao::skills`（`skills.rs:75`）施法 + `internal_brew` 直扣 `player.qi_current` 已走 `QiTransfer{reason:ReleaseToZone}` 经 ledger。**注意既有债务**：`consume_pill`（`pill.rs:187`）服丹恢复 qi 是直接 `cultivation.qi_current = (before+effective_q).min(qi_max)`，**未走 ledger**（既有 alchemy 行为）——P0 不改动它、不引入新凭空路径，该守恒债务留给后续 alchemy 守恒专项 plan。**P4 红线**：暴龙王真元吸取光环是全新代码，必须走 `qi_physics::ledger::QiTransfer{from:player, to:zone/boss account}`，吸来的真元不得凭空消失（守恒测试断言取 `SPIRIT_QI_TOTAL` const 引用，不写字面）。

## P0 — `dandao::register` 接入 run_server + 生产服丹路径 emit `PillIntakeTracked`

**断链 mutation-runtime-not-started（根因）+ dandao-pill-intake-no-emitter**：`dandao::register(app)`（`mod.rs:33`，add 2 event + 2 system）从未在 `run_server()` 调用（`main.rs:127-161` register 列表无 dandao，仅 mod 声明在 `main.rs:18`，及 `cultivation::skill_registry.rs:77` 以库身份调 `register_skills` 注册 3 招）。后果：`mutation_advance_system` / `track_pill_intake_system` 从未进 schedule。叠加：`PillIntakeTracked` 有 reader（`track_pill_intake_system`）无 writer。

交付物（可核验）：

- **模块 / 文件**：
  - `server/src/main.rs` — `run_server()` 增 `dandao::register(&mut app);`（置于 `alchemy::register`@138 之后，因服丹链在 alchemy/network）
  - `server/src/network/client_request_handler.rs:8593`（`handle_alchemy_take_pill`@:8249 内）— `consume_pill` 成功后 `EventWriter<dandao::toxin_tracker::PillIntakeTracked>.send(PillIntakeTracked{ entity, toxin_amount })`，携带服丹 toxin 增量
- **函数 / 符号**：`dandao::register`、`PillIntakeTracked`、`track_pill_intake_system`、`mutation_advance_system`、`DandaoStyle::stage_for_toxin` / `advance_toxin`、`handle_alchemy_take_pill`、`consume_pill`
- **测试声明**（`server/src/dandao/` + `network/` 集成）：
  - register pin：构造 App 调 `dandao::register` 后断言 `Events::<PillIntakeTracked>` / `Events::<MutationAdvanceEvent>` 资源存在 + 两 system 在 `Update`（行为断言而非内部调用次数）
  - writer（生产路径）：`handle_alchemy_take_pill` 成功服丹后 `PillIntakeTracked` 事件计数 +1 → `track_pill_intake_system` 给该 entity `PracticeLog` 记一次 Mellow；失败/拒服路径不 emit
  - 变异链（register 接通后）：给 entity 的 `DandaoStyle.cumulative_toxin` 经 `advance_toxin` 越过 `stage_for_toxin` 阈值 → `mutation_advance_system` 推进 `MutationState.stage` 并 emit `MutationAdvanceEvent`（阶段 N→N+1）
  - 守恒回归：P0 只追加 emit `PillIntakeTracked`，**不动** `consume_pill` 现有 qi 恢复数值（`consume_pill_normal_appends_contam_and_restores_qi` 行为不变）

## P1 — 变异视觉 server→client（`bong:mutation_visual`）

**断链 dandao-mutation-advance-event-no-reader / dandao-mutation-visual-no-emit / mutation-visual-server-emit-missing**：`MutationAdvanceEvent` 全仓无跨模块 reader；`MutationVisualSyncPayload`（`visual_sync.rs:26`）+ `MUTATION_VISUAL_CHANNEL`（`bong:mutation_visual`）已定义，但 server 端从无任何 `send_custom_payload` 发射；client `MutationPayloadHandler` 全套 GeckoLib 渲染 + HUD 已就绪却饿死。

交付物（可核验）：

- **模块 / 文件**：
  - 新增 `server/src/network/mutation_visual_emit.rs`（仿近同构范本 `network/void_erosion_visual_emit.rs`）— system 读 `EventReader<MutationAdvanceEvent>`（或 `Changed<MutationState>`），查询 `(&MutationState, &DandaoStyle, player uuid/name)` → `MutationVisualSyncPayload::from_state(entity_id, &MutationState)` → `send_custom_payload(client, MUTATION_VISUAL_CHANNEL, payload)`；在 `network::register` 或 `dandao::register` 排进 schedule
  - **payload 扩字段（必做）**：`MutationVisualSyncPayload`（`visual_sync.rs:26-35` 当前无 `cumulative_toxin`）须扩 `cumulative_toxin: f64`，由发射 system 从 `DandaoStyle.cumulative_toxin` 填（`MutationState` 不持有 toxin）——否则 client `MutationHudPlanner` 丹毒条恒空（`MutationPayloadHandler.java:29` fallback 0.0）
  - **大小写对齐（必做）**：`from_state`（`visual_sync.rs:47-48`）当前 `format!("{:?}").to_ascii_lowercase()` 输出全小写（`goldeniris`/`head`），但 client `MutationInspectLabel.translateKind` 按 CamelCase（`"GoldenIris"`）、`translateBodySlot` 按 UPPER（`"HEAD"`）、`MutationKind.fromServerString` 正则 `([a-z])([A-Z])` 对无分隔小写得 `GOLDENIRIS`≠`GOLDEN_IRIS` → 全显「未知」。**择一**：改 `from_state` 输出 CamelCase（去 `to_ascii_lowercase`）+ body_slot 保 enum Debug 形态；或 client 侧统一归一化
  - client（验证已接收，无需改逻辑）：`MutationPayloadHandler`（`BongNetworkHandler.java:396` 已注册）→ `MutationVisualState.replace` → `MutationFeatureRenderer` + `MutationHudPlanner`
- **函数 / 符号**：`mutation_visual_emit_system`、`MutationVisualSyncPayload::from_state(entity_id, &MutationState)`、`MUTATION_VISUAL_CHANNEL`、client `MutationPayloadHandler` / `MutationVisualState` / `MutationFeatureRenderer` / `MutationHudPlanner`
- **视听规格**（复用既有 client GeckoLib 渲染——本阶段补齐 payload 字段使既有渲染可驱动）：
  - HUD：`MutationHudPlanner` 渲染变异阶段进度 + 丹毒条；payload 须携带 `stage` / `slots[{kind,body_slot,level}]` / `cumulative_toxin` / `meridian_penalty`
  - 模型：`MutationFeatureRenderer` 按 `MutationKind` 叠加变异肢体（已有资产）；kind/body_slot 枚举值须 server↔client 双端字面对齐（见上「大小写对齐」）
  - 本阶段不新增粒子/贴图/动画资产，纯接既有渲染器 + 补 payload 字段
- **测试声明**：
  - payload serde 对拍：`MutationVisualSyncPayload`（含新 `cumulative_toxin`）server 序列化 ↔ client 反序列化 sample，**含 kind/body_slot 大小写 round-trip**（锁住 server 输出 ↔ client 枚举解析一致）
  - emit：`MutationAdvanceEvent` 触发后发射 1 条 `bong:mutation_visual`；无变异时不发射
  - e2e：server emit → client `MutationPayloadHandler` 收到 → `MutationVisualState` 更新 + `MutationHudPlanner` 丹毒条读到非 0 `cumulative_toxin`

## P2 — 变异叙事 server→agent（`bong:mutation_event`）+ 两端 schema 对齐 + agent runtime 启动

**断链 mutation-runtime-not-started（agent 侧）**：`MutationNarrationRuntime`（`mutation-narration-runtime.ts:79`，订阅 `MUTATION_EVENT`，render→`AGENT_NARRATE`）定义完整含测试，但 `agent/.../main.ts` 从不 import/启动它。叠加 server 侧：`RedisOutbound`（`redis_bridge.rs:128`）无 `MutationEvent` 变体、`agent_bridge.rs:102` 无 channel 映射、无任何 system publish `bong:mutation_event`。**且两端 schema 字段不对齐**（下）。

交付物（可核验）：

- **schema 两端对齐（必做，先于发射）**：server serde `MutationEventV1`（`schema/dandao.rs:73-82`：`entity:String` / `new_meridian_penalty:f64` / `server_tick:u64`，`deny_unknown_fields`）与 agent TypeBox（`mutation-event.ts:22-32`：`v:Literal(1)` / `entity_id:Integer` / `at_tick`，`additionalProperties:false`）字段不同且两端都 strict，runtime 走 `validateMutationEventV1`。全仓无 mutation_event sample。须：决定权威形状（建议对齐到 agent TypeBox：`{v:1, entity_id, from_stage, to_stage, cumulative_toxin, at_tick}`），server 发射端组成该形状（`entity`→`entity_id` 且 String→Integer 映射、`server_tick`→`at_tick`、加 `v:1`、决定 `new_meridian_penalty` 去留），新增 server↔agent 共享 sample（`agent/packages/schema/samples/`）
- **模块 / 文件**：
  - server：新增 `RedisOutbound::MutationEvent(MutationEventV1)`（`redis_bridge.rs:128` 枚举）+ redis_bridge 序列化 arm + `agent_bridge.rs:102` channel→`"mutation_event"` 映射（`CHANNELS.MUTATION_EVENT` 已是 `bong:mutation_event` 且已在 `REDIS_V1_CHANNELS`）+ 新增 emit system 读 `EventReader<MutationAdvanceEvent>`→组对齐后形状→publish，排进 register（**范式仿 `network/poison_trait_emit.rs:28`**）
  - agent：`main.ts` import `MutationNarrationRuntime` + 新增 start helper（仿 `main.ts:482` `new BaomaiV3NarrationRuntime({sub,pub})`）+ 纳入 `startAuxiliaryRuntimes`（`main.ts:143`）的 `cleanupFns`
- **函数 / 符号**：`RedisOutbound::MutationEvent`、`mutation_event_publish_system`、`MutationEventV1`（两端对齐后）、`bong:mutation_event`；agent `MutationNarrationRuntime`、`startAuxiliaryRuntimes`
- **narration 模板**（`MutationNarrationRuntime` 已内置渲染——本阶段验证 scope/style，必要时补样例）：scope=player；style=perception/narrative；样例方向（最终以 runtime 模板为准）：「服下化形丹，你的左臂泛起暗鳞，骨节发出不属于人的脆响」/「毒蛊在经脉里安了家，你与人之间隔了一层」/「这具躯体越来越不像你了——但力量是真的」
- **测试声明**：
  - schema 对拍（升级）：**先对齐两端字段**，再 `MutationEventV1` Rust serde ↔ agent TypeBox `validateMutationEventV1` 用同一 sample 正反对拍（新增 sample，锁住 `entity_id` 类型 + `v:1` + `at_tick`）
  - server publish：`MutationAdvanceEvent` → `bong:mutation_event` 发布 1 条（mock redis 出站断言 payload 为对齐后形状）
  - agent：`MutationNarrationRuntime` 收 `bong:mutation_event` → 产 `AGENT_NARRATE`（runtime 已有单测）；main.ts 启动后 runtime 在 `cleanupFns`（启动 wiring 测试）

## P3 — 催化炉加成接入 alchemy resolver

**断链 dandao-catalyst-furnace-unused**：`catalyst_furnace_bonus(furnace_tier, recipe_id)`（`catalyst_furnace.rs:18`）全仓无 runtime 调用方——催化炉对变异丹成功率加成的承诺未落地。

交付物（可核验）：

- **模块 / 文件**：
  - `server/src/alchemy/resolver.rs:148`（`resolve_raw`）或其成功率计算处 — 引入 furnace tier 入参，对变异丹配方（`is_mutation_recipe`）调 `dandao::catalyst_furnace::catalyst_furnace_bonus(tier, recipe_id)` 叠加成功率
  - 炉具 tier 来源：alchemy 炉 component / 配方 `furnace_tier_min`（经 `assets/alchemy/recipes/*.json` 进 `RecipeRegistry`）
- **函数 / 符号**：`catalyst_furnace_bonus`、`resolve_raw`、`is_mutation_recipe`、`RecipeRegistry`
- **测试声明**：
  - 高 tier 炉 + 变异丹配方 → 成功率加成（**倍率断言取 `catalyst_furnace_bonus` 返回值，不写字面**）
  - tier 不足 → 无加成；非变异丹配方 → 无加成（边界 + 错误分支）
  - 守恒/回归：加成只改成功率，不改产出 qi/数量

## P4 — 暴龙王 BOSS 端到端遭遇战

**断链 dandao-boss-orphan**：`BaolongwangBoss` component（`boss.rs:22`）+ boss_ai 三阶段**评分自由函数**（`boss_ai.rs`，头注释明写「纯 Scorer 链组合，不扩展 big-brain 框架」）+ `compute_loot`（`boss.rs:129`）+ client 实体（raw_id 160 全套渲染）已就绪，但**无 spawn 路径**、**无 big-brain 集成层**、无 loot 落地、无吸取光环代码。

交付物（可核验）：

- **模块 / 文件**：
  - spawn：新增 dev 命令 `server/src/cmd/dev/baolongwang.rs`（仿 `cmd/dev/whale.rs` / `heiwushi.rs` 真实范本）+ 注册进 `cmd/dev/mod.rs`；并接 zone `baolongwang_cavern_deep`（`zone.rs:1173`）触发 spawn（坍缩渊边缘自然遭遇）
  - **big-brain 集成层（本阶段最重的新写部分，勿低估为接线）**：`boss_ai.rs` 是裸 `pub fn score_*(boss,..)->f32` + 自定义 `pick_best_action`（`:93`），**无** `impl ScorerBuilder/ActionBuilder/Thinker`。须新写 big-brain `ScorerBuilder`/`ActionBuilder` 组件 + 对应 system **包装现有 `score_*` 函数**，spawn 时挂 `Thinker`（**范本 `server/src/npc/skull_fiend.rs:165/175/197`**）；保留 `boss_ai.rs:83` phase_gate 不变量（同一时刻仅一阶段评分 >0）
  - 真元吸取光环（全新代码 + **守恒**）：`BaolongwangBoss` 真元吸取必须走 `qi_physics::ledger::QiTransfer{from:player, to:zone（或 boss account）}`（worldview §十六:1572 负压畸变体抽玩家真元 +50% 为依据）；若无 boss account 模型，转入 zone（参 `collapse_redistribute_qi`）
  - loot：BOSS 死亡 reader → 调 `compute_loot`（`boss.rs:129`）落地掉落（见下）
  - **item icon（视觉资产）**：核查确认 `dandao.baolongwang_core` / `dandao.baolongwang_horn` / `dandao.baolongwang_scale` / `dandao.xu_yuan_dan` 及 `LOOT_ANCIENT_RECIPE` / `LOOT_FURNACE_REMNANT` 当前**未注册为 `ItemTemplate`**（`assets/items/` grep 0 命中）→ P4 须补 `ItemTemplate` + `/gen-image item` 生成图标（memory `feedback_item_icon_gen`）
  - client（验证已接收）：`BaolongwangEntities`（raw_id 160，`BongClient.java:125` 已 register）/ `BaolongwangRenderer` / `BaolongwangModel` 已注册渲染
- **函数 / 符号**：`BaolongwangBoss`、boss_ai `score_*` / `pick_best_action`、新 `BaolongwangScorer` / `BaolongwangAction`（big-brain 组件）、`spawn_baolongwang`（新）、`compute_loot`、loot 常量、client `BaolongwangEntity`
- **视听规格**（client 模型/渲染器已就绪，本阶段补遭遇战可感知层）：
  - 三阶段攻击 telegraph：每阶段 Action 须差异化 VFX/SFX（memory `feedback_skill_av_diff`，各阶段不可单方向 stub）；真元吸取光环须持续粒子 + 玩家真元被吸的 HUD 反馈
  - narration：BOSS 现身 / 阶段转换 / 击杀 各一条天道旁白（scope broadcast 或 zone）
  - **本阶段视听规格须在 PR-5 实施前补到 docs/CLAUDE.md §视听 精度**（粒子基类/数量/lifetime/颜色 hex、audio_recipe 层、HUD overlay）——skeleton 阶段先占位
- **测试声明**：
  - spawn：dev 命令 / zone 触发后 `BaolongwangBoss` 实体存在 + `Thinker` 挂载
  - AI 状态机：health_ratio 跨阶段阈值 → 评分切换（每阶段一条 case，同一时刻仅一阶段评分 >0，`boss_ai.rs:83` 不变量）
  - loot（**对齐 `compute_loot`@`boss.rs:129-159`，断言取实际概率/复用既有 loot 测试，不写理想化字面**）：`LOOT_BOSS_CORE` 100%×1 / `LOOT_ANCIENT_RECIPE` 100%×3 / `LOOT_BOSS_HORN` 50% **且需 `has_entered_rage` 门控**（未进 Rage→不掉 horn，专属边界 case）/ `LOOT_BOSS_SCALE` 80%×3-8 / `LOOT_FURNACE_REMNANT` 100%（仅 `!furnace_intact`）/ `LOOT_XU_YUAN_DAN` 70%×5-10
  - 守恒：真元吸取光环 — 玩家被吸 X = zone/boss 增 X，total 不变（断言取 `SPIRIT_QI_TOTAL` const 引用）
  - e2e：spawn → client 收到实体 160 → 击杀 → loot 落地

## §10 消费本 plan 的工作流约束（consume-plan agent 必读）

> 本 plan = 一条流派整链接线，5 个阶段对应 5 个 PR。通用约束（worktree / atomic commit / 测试全绿 / 不绕 hooks）全部生效。结构参 `plan-terrain-wiring-v1` §10 / `plan-dandao-path-v1` §10。

### §10.1 视觉资产：P4 item icon + 视听规格

- P4 须新建 6 个 BOSS 掉落 `ItemTemplate`（core/horn/scale/xu_yuan_dan/ancient_recipe/furnace_remnant），配套 `/gen-image item` 生成图标（memory `feedback_item_icon_gen`），图标在 PR-5 阶段批量产出。
- P4 三阶段 BOSS 攻击 + 真元吸取光环视听规格须在 PR-5 实施前补到 docs/CLAUDE.md §视听 精度并内联于 P4 块；各阶段差异化（`feedback_skill_av_diff`），不接受单方向 stub。
- 本 plan **无 NBT 建筑**——§6.1 三轮 `<PROMISE>` 不适用；纯逻辑 + 既有渲染接线 + item 图标，按 atomic commit + 测试全绿（item 图标若走多轮打磨另议）。

### §10.2 多 PR 序列化（依赖顺序，前一个 merge 后开下一个）

1. **PR-1（P0）** `dandao::register` 接入 + 生产服丹 emit `PillIntakeTracked` — **总开关，必须最先 land**
2. **PR-2（P1）** 变异视觉 server→client `bong:mutation_visual`（含 payload 扩字段 + 大小写对齐）— 依赖 PR-1
3. **PR-3（P2）** 两端 schema 对齐 + `RedisOutbound::MutationEvent` 发布 + agent runtime 启动 — 依赖 PR-1
4. **PR-4（P3）** 催化炉加成接 resolver — 独立于变异链，依赖 PR-1
5. **PR-5（P4）** 暴龙王 BOSS（spawn + big-brain 集成层 + 吸取光环守恒 + loot + 图标）— 体量最大，依赖 PR-1

> agent 改动（PR-3）注意 memory `project_schema_dist_rebuild`：动 `@bong/schema` src 后须 `npm run build -w @bong/schema` 重建 dist，否则天道启动崩 ESM export not found。

### §10.3 PR 实施用独立 subagent + 模型路由

> ⚠️ **偏离 docs/CLAUDE.md §6.4 的 `model:"opus"`**——依用户强约束（memory `feedback_workflow_model_routing` + `feedback_workflow_opus_concurrency_cap`）：**写代码（实施）一律 sonnet，opus 只用于验证且并发 ≤3**。

```
Agent(
  subagent_type: "claude",
  model: "sonnet",                   # 实施=sonnet（非 opus），无"精细/守恒关键"例外
  prompt: "...本 PR 范围 + 测试饱和化要求...\n\nthink hard"
)
```

- 每 PR 一个独立 sonnet subagent 实施 + 提 PR；主线只收 result。
- 实施后如需对抗式核验（守恒律 / 契约对拍 / e2e 完整性），起 opus 验证 agent，**同时并行 ≤3 个**。
- 主线 merge 命令亲自做（不耗 context）。

### §10.4 CodeRabbit + Pi agent 等待协议

- 每 PR 等 **CodeRabbit + Pi agent (github-actions)** 两 bot（memory `feedback_wait_coderabbit_approve`）；`gh pr checks` 看状态。
- `pending` → `ScheduleWakeup delaySeconds=1200`（20min/回合，最多 3 回合 = 60min 卡死交人工）；修完意见**重新等 CR re-review**，不自判通过。
- 多 PR 各自走完整等待协议，前一个 APPROVED + merge 后才开下一个。

### §10.5 单次 consume-plan 全自动到 merge + 归档

用户提交 `/consume-plan dandao-runtime-wiring-v1` 后即可下班——consume-plan agent 在 worktree 内按 §10.2 五 PR 序列依次实施（sonnet subagent）、依次等 CR/Pi approve（ScheduleWakeup 驱动）、依次 merge，全部 land 后填 `## Finish Evidence` 并 `git mv` 入 `docs/finished_plans/`。

## Finish Evidence

> 迁入 `docs/finished_plans/` 前必填（落地清单 / 关键 commit / 测试结果 / 跨仓库核验 / 遗留）。当前 P0–P4 均 ⬜，未填。
