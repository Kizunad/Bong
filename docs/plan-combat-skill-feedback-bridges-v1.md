# plan-combat-skill-feedback-bridges-v1 — 战斗流派反馈整链接线（VFX/HUD/agent 叙事最后一跳补桥）

> **一句话主题**：多条战斗流派（爆脉 v3/v4、我流虚蚀、暗器分身、蜕壳灰烬、毒蛊 v2、剑道人剑共生、经脉断脉）的**底层数值逻辑在 server 已生效、测试已锁、client receiver / HUD / agent runtime 消费端多半已就绪**，但 **server→client（VFX/HUD payload）与 server→agent（Redis 叙事 channel）的反馈桥系统性漏接在最后一跳**——事件 `EventWriter` 发出却零 `EventReader`、payload struct 定义却从不 `send_custom_payload`、component 永不 `insert`、Redis channel 常量已埋却无 `RedisOutbound` 变体 / 无 publish system、client HUD store 永不被写。本 plan 照已接通的范本桥（`baomai_v3_event_bridge.rs` / `void_erosion_visual_emit.rs` / `dugu_state_emit.rs` / `poison_trait_emit.rs`）**批量补接**这些「写好却没接线」的反馈链。
>
> **调查依据**：`investigate-combat-feedback-bridges`（2026-06-03，16-agent workflow，35 候选 / **27 确认**（18 high / 7 medium / 2 low）/ opus 对抗式验证 + 主题归类）。承接 `gameplay-broken-links-audit`（2026-06-02，81-agent）theme `combat-skill-feedback-bridges`（high，6 大主题排序第 2，仅次于已立的 `dandao-runtime-wiring`）。**待过 `validate-combat-feedback-bridges-plan` 工作流复核**——下列交付物落点/契约/守恒/worldview 已据调查 opus 验证证据修正（含 8 条关键纠偏，见各阶段「opus 验证纠偏」标注）。
>
> **关系**：本 plan **不重新设计任何流派世界观/数值/招式机制**——v2/v3/v4 各流派 plan（`plan-baomai-v3/v4`、`plan-woliu-path-v1`、`plan-anqi-v2`、`plan-tuike-v2`、`plan-dugu-v2`、`plan-sword-path-v2/v3`、`plan-meridian-severed-v1`，均已归档/active）已定义机制底盘与消费端资产。本 plan 只补接线，land 后这些 plan 承诺的「玩家可感知反馈」才名副其实。与已归档的 `plan-combat-feedback-v1`（通用战斗 game-feel）**不重叠**——后者是通用打击感，本 plan 是各流派专属事件的叙事/HUD 终端接线。

## 阶段总览

| 阶段 | 主题（流派子域） | 断链数 | 范本 | 状态 | 验收日期 |
|------|------|------|------|------|------|
| **P0** | 经脉断脉整链发布（`meridian_severed`：Redis 叙事 + 非主动断脉 VFX；**复用价值最高，7+ emit 源全受益**） | 3 | `poison_trait_emit.rs` + `void_erosion_visual_emit.rs` | ⬜ | — |
| **P1** | 爆脉 v4 反馈整桥（`baomai_v4`：scar circuit / iron cocoon / crack reading / resonance lock，server→agent + server→client） | 6 | `baomai_v3_event_bridge.rs` + `void_erosion_visual_emit.rs` | ⬜ | — |
| **P2** | 爆脉 v3 残余事件补桥（`baomai_v3`：disperse 失败 / mountain shake / blood burn / 超越到期 / 过载裂纹） | 5 | `baomai_v3_event_bridge.rs`（扩展，金标准范本） | ⬜ | — |
| **P3** | 我流虚蚀整链激活（`woliu_v2`：component insert + erosion writer + check_system + client receiver + agent runtime） | 4 | `void_erosion_visual_emit.rs` + `woliu_event_bridge.rs` | ⬜ | — |
| **P4** | 暗器分身 HUD 喂数据（`anqi`：decoy deploy + aim/charge/abrasion → `AnqiHudStateStore`） | 2 | `dugu_state_emit.rs` + `DuguPoisonStateHandler.java` | ⬜ | — |
| **P5** | 毒蛊 v2 HUD S2C 整链（`dugu_v2`：5 招 cast + 永久真元衰减 + 形貌暴露 → `DuguV2HudStateStore`） | 3 | `poison_trait_emit.rs`（Redis+S2C 双发） | ⬜ | — |
| **P6** | 剑道人剑共生 HUD + 蜕壳灰烬回收（`sword_path` + `tuike`） | 3 | `dugu_state_emit.rs` / `yidao_state_emit.rs` + `tuike_event_bridge.rs` | ⬜ | — |

> 阶段为 P0–P6 共 7 段；`验收日期` 各段在对应 PR merge 后填 `YYYY-MM-DD`，全段 ✅ 后迁入 `finished_plans/`。**链数核对**：3(P0)+6(P1)+5(P2)+4(P3)+2(P4)+3(P5)+3(P6) = **26 条入 P0–P6**；其中 `baomai-v4-voluntary-sever`（emit `MeridianSeveredEvent`）**仅计入 P0 的 3 条、不重复计入 P1**（P1 的 6 条全为 baomai_v4 自有事件）。第 27 条 `jingmai-sever-yidao-hud-count`（low）因实为「yidao 患者诊断面板整块未接（hp/contam/severed 三字段全空 + 需患者 Entity 解析）」的子项，**列入 §8 遗留/开放问题**，不在本 plan 单独接（见 §8 #4）。

> 依赖顺序：**P0 最先**——`meridian_severed` 发布链复用价值最高（7+ emit 源：detection_tick / zhenmai_v2 / baomai_v3 / baomai_v4 voluntary-sever / tribulation 全部受益），render 逻辑 + channel 常量已就绪，工作量最小，且**吸收了 baomai_v4 的 voluntary-sever 断链**（见 P0）。P1–P6 各对应一个流派子域、互相解耦，可按上表顺序独立成 PR。P2（baomai_v3 agent runtime 多通道重构）建议在补单事件前先做路由重构（见 P2 opus 纠偏）。

## 接入面（防孤岛）

> 本 plan 是**纯接线 plan**：每条断链的两端（producer 机制 + consumer 渲染/叙事）都已存在，缺的是中间桥。下列「进料/出料」即各断链的两端锚点。

- **进料（已存在的 producer 端 Bevy Event / Component / 机制）**：
  - `cultivation::meridian::severed::MeridianSeveredEvent`（`severed.rs:261` `apply_severed_event_system` 已消费写 component，5+ EventWriter 源：`severed.rs:230` detection_tick / `zhenmai_v2.rs:797` / `baomai_v3/skills.rs:870` / `baomai_v4/dead_armor.rs:264` / `tribulation.rs:3211/3452`）——P0 进料
  - `baomai_v4` 七事件（**当前全部纯 write-only，零跨模块 reader**）：`ScarCircuitFormedEvent`/`ScarCircuitBrokenEvent`（`scar_circuit.rs:154/155` + `dead_armor.rs:237`）、`IronCocoonStageUpEvent`（`iron_cocoon.rs:67-73`）、`CrackReadingResultEvent`（`crack_reading.rs:265-325`，`to_client_payload()`@:221 + `CrackReadingPayload`@:203 已写但标 `#[allow(dead_code)]`）、`ResonanceLockEvent`/`ResonanceLockEndEvent`（`resonance_lock.rs:236/379`）——P1 进料
  - `baomai_v3` 五事件：`DispersedQiEvent`（`skills.rs:579` 总发，但 `emit_skill_event` 被 `if profile.has_transcendence` 包裹@:591）、`MountainShakeEvent`（`skills.rs:372`）、`BloodBurnEvent`（`skills.rs:469`）、`BodyTranscendenceExpiredEvent`（`tick.rs:56`）、`OverloadMeridianRippleEvent`（`skills.rs:877`，已被 **`baomai_v4/scar_history.rs:56`** 跨模块消费——v4 reader 读 v3 event，新增 Redis 桥须保证该既有 reader 不破坏；**注意 `baomai_v3/` 目录下无 scar_history.rs**）——P2 进料
  - `woliu_v2::VoidErosion` component（`erosion.rs:132`）+ `VoidErosionAdvanceEvent`（`mod.rs:28` 已 `add_event`，`erosion.rs:251` struct）——P3 进料（**注意：component 从未 insert，且 `add_erosion`@:241 / `add_erosion_capped`@:434 两个 mutator 也从未被 runtime 调用，仅测试引用**）
  - `anqi_v2::DecoyDeployEvent`（`anqi_v2.rs:241` def / :270 add_event / :638 send，**零 reader**）+ `QiInjectionEvent` / `CarrierAbrasionEvent`（aim/charge/abrasion 数据源，**注意：这两个事件已被 `anqi_event_bridge.rs:120-143/:187-207` 读取 publish 到 Redis、agent 叙事半边已通**——P4 真实缺口仅 client HUD S2C 半边 + `DecoyDeployEvent` 零 reader）——P4 进料
  - `dugu_v2` 五招事件（`dugu_v2_event_bridge.rs:16` 已 publish 到 Redis，agent 已叙事）+ `PermanentQiMaxDecayApplied`（`tick.rs:45`，零 network reader）+ `DuguSelfRevealedEvent`（`skills.rs:303`）——P5 进料
  - `sword_path::SwordBondComponent`（`bond.rs:16`，`bond_strength`/`stored_qi`/`grade` 实时维护）——P6 进料
  - `tuike_v2::FalseSkinDecayedToAshEvent`（`tick.rs:177-195`，`output_item_id` 算完即丢）+ `PermanentTaintAbsorbedEvent`（`skills.rs:241-249`）——P6 进料
- **出料（已就绪的 consumer 端，等着喂）**：
  - agent：`renderMeridianSeveredNarration`（`meridian-severed-narration.ts:75`，schema `meridian-severed.ts` 齐全，`channels.ts:323` `MERIDIAN_SEVERED='bong:meridian_severed'` 已埋）——P0；`BaomaiV3NarrationRuntime`（`baomai-v3-runtime.ts`，**当前单通道，P2 需重构为多通道路由**）——P1/P2;（待建）`BaomaiV4NarrationRuntime` / `VoidErosionNarrationRuntime`——P1/P3
  - client VFX/HUD：`jiemai_sever_flash`（`VfxBootstrap.java:126` 已注册）——P0；（待建）`client/combat/baomai/v4/` CrackReading/ResonanceLock handler+HUD——P1；（待建）`VoidErosionVisualHandler`（范本 `registerMutationVisualChannel`@`BongNetworkHandler.java:395`）——P3；`AnqiHudStateStore.replace`（`AnqiHudStateStore.java:14`，`AnqiHudPlanner` 已接 `BongHudOrchestrator:305`，当前仅 debug 命令写）——P4；`DuguV2HudStateStore.replace`（`DuguV2HudStateStore.java:35`，`DuguV2HudPlanner` 已接 `BongHudOrchestrator:318`，**`.replace` 全仓零调用**）——P5；`SwordBondHudStateStore.replace`（`SwordBondHudStateStore.java:15`，`SwordPathHudPlanner` 已接 `BongHudOrchestrator:324`，**`.replace` 全仓零调用**）——P6
  - inventory：`add_item_to_player_inventory`（`inventory/mod.rs:1033`）——P6（蜕壳灰烬入包）
- **共享类型 / event（复用既有，本 plan 不重新定义机制事件；新增的是桥 system / payload 变体 / channel 常量）**：
  - 新增 server 桥文件：`meridian_severed_emit.rs`（P0）、`baomai_v4_event_bridge.rs`（P1）、`anqi_hud_emit.rs`（P4）、`sword_bond_state_emit.rs`（P6）、`tuike_ash_emit.rs`（P6）；扩展既有：`baomai_v3_event_bridge.rs`（P2）、`woliu_event_bridge.rs`（P3）、`dugu_v2_event_bridge.rs`（P5，追加 S2C）、`tuike_event_bridge.rs`（P6，追加 permanent-taint reader）
  - 新增 `RedisOutbound` 变体：`MeridianSevered`（P0）、`BaomaiV4*`（P1）、`VoidErosionEvent`（P3）；新增 `ServerDataPayloadV1` 变体：`AnqiHud`（P4）、`DuguV2*`/`PermanentQiMaxDecayApplied`（P5）、`SwordBondHudState`（P6）
  - **⚠️ 凡新增 `ServerDataPayloadV1` 变体（P4 `AnqiHud` / P5 `DuguV2*`+`PermanentQiMaxDecayApplied` / P6 `SwordBondHudState`）均须补完整 proto 链**——`proto_convert.rs:438` `server_data_to_proto_payload` 是 **119-arm 穷尽 match 无 catch-all**（生产 `agent_bridge.rs:55` 走 `#[cfg(not(test))] to_proto_bytes()`），漏 arm 直接**编译失败**；client `ProtoServerDataBridge.java:46/220` `CASE_TO_TYPE` 未映射则**静默返 null、HUD 永收不到**。三件齐：`proto_gen.rs`（prost 消息）+ `proto_convert.rs` 新增 arm + client `ProtoServerDataBridge` `CASE_TO_TYPE` 条目。**仅加变体 + JSON 单测会全绿假象（`#[cfg(test)]` 走 JSON 路径），但生产 proto 路径漏接 = 没修**（这正是 P5 `DuguV2HudStateStore.replace` 零调用的根因之一）
  - schema：`MeridianSeveredEventV1`（**server serde struct 当前不存在，仅 agent TypeBox 有——P0 须从 JSON Schema 导出/手写**）、`BaomaiV4*` TypeBox + samples（P1）、`VoidErosionEventV1`（schema 类型已存在，缺 outbound 包装，P3）
- **跨仓库契约**：server（Rust）↔ agent（TS）↔ client（Java）三端齐动。**消费端（client receiver / HUD store / agent runtime / VFX 注册）多数已实装就绪、缺发射端/桥/订阅**——但凡涉及新 schema 字段（P1 BaomaiV4* / P5 DuguV2* / P6 SwordBondHudState proto）的契约对齐缺口须当作**必做交付物 + 双端 sample 对拍测试**锁定，不是「纯接线」。agent schema 改动后须 `npm run build -w @bong/schema` 重建 dist（memory `project_schema_dist_rebuild`）。
- **worldview 锚点**（已据 `docs/worldview.md` 复核行号）：
  - 经脉断脉 = §四:282（损伤档位 `SEVERED 0.0`「该经脉承载的流派效果废」）+ §四:309（正经按肢分布，断臂 → 正经同步 SEVERED）——P0
  - 爆脉/体修流 = §五:401『攻击四流·1. 体修/爆脉流·破产式狂战士』+ §四:358（爆脉强行调动 20 点真元，灵脉高压冲出裂缝）——P1/P2
  - 我流虚蚀 = §:444-446（涡流流原理 + 反噬代价）——P3
  - 暗器流 = §五:407『攻击四流·2. 器修/暗器流』+ §:408（真元附着物理载体）+ §:464（载体封存比例）——P4
  - 毒蛊流 = §五:423『攻击四流·4. 毒蛊流·恶性寄生者』+ §:530（永久经脉损伤→qi_max 永久下降）+ §:531-533（形貌暴露：神识遮蔽失效被识破）——P5
  - 剑道人剑共生 = §六:613（剑修·锋锐色·真元线状流动）+ §五:514（流派是行为涌现非字段）——P6（人剑共生）；蜕壳流 = §:439（替尸/蜕壳流·伪灵皮蜕落化灰烬·物资回收节点）——P6（灰烬）
- **qi_physics 锚点**：**本 plan 全部断链都是只读上报，桥/payload 不得二次操作真元账**——producer 机制层已扣/已守恒（如 `dugu_v2/tick.rs:43` `qi_max -= loss`、`baomai_v3` `record_qi_transfer`、`tuike` `PermanentQiMaxDecay` remove），桥只转发已算出的值，**严禁在 emit/publish/HUD 路径再扣 qi 或重算**（守恒红线）。**P3 woliu 守恒纠偏（⚠️ 防双重记账）**：`add_erosion`（`erosion.rs:241`）/ `add_erosion_capped`（`erosion.rs:434-442`）**只更新 `cumulative_erosion` + `stage`、零 qi 字段**（doc 明写「只增不减，与丹道 `cumulative_toxin` 对齐」）——`cumulative_erosion` 是**纯伤害度量**，**禁止在 erosion 写入路径挂 `QiTransfer`**。涡流施法的真元流动**已在 skill 层 `skills.rs:1107-1151` 走 `QiTransfer{Channeling}`**；P3 只需在涡流路径后调 `add_erosion_capped` 累积 erosion（不动 qi），**若另在 erosion 路径塞平行 `QiTransfer` 会与 skill 层既有 transfer 双重记账（守恒红旗）**。其余阶段 qi_concern 标 none——叙事/HUD 桥纯观测。

---

## P0 — 经脉断脉整链发布（`meridian_severed`）

**断链 jingmai-sever-redis-publish（high）+ jingmai-sever-vfx-non-voluntary（medium）+ baomai-v4-voluntary-sever-agent-redis（medium，被本阶段吸收）**：`MeridianSeveredEvent`（`severed.rs:261` 内部 reader 写 component 正常）由 **5+ 个 EventWriter 源** emit，但 `RedisOutbound` enum（`redis_bridge.rs:128-240`，~70 变体）**无 `MeridianSevered` 变体**、无 `publish_meridian_severed_events` system、agent `main.ts:143` `startAuxiliaryRuntimes` 现挂十余个 aux runtime（insight/death/woliu/dugu_v2/baomai_v3 等）但**均未订阅 `bong:meridian_severed`**，`renderMeridianSeveredNarration` 仅纯函数未包 runtime。叠加：非主动断脉（`CombatWound`/`OverloadTear`/`BackfireOverload`/`TribulationFail`/`DuguDistortion` 来源）落档后无 `jiemai_sever_flash` VFX（该粒子仅 `zhenmai_v2.rs:845-853` SeverChain 主动技能发一次）。

> **opus 验证纠偏（必读）**：
> 1. **server 侧 `MeridianSeveredEventV1` serde struct 当前不存在**（仅 `agent/packages/schema/src/meridian-severed.ts` 有 TypeBox）——P0 须先从 JSON Schema 导出/手写 Rust serde struct，这是 publish 链硬前置（pre-P0 收口）。
> 2. **一个 `EventReader<MeridianSeveredEvent>` 即覆盖全部 5+ emit 源**，无需逐源接线——`SeveredSource` enum（`severed.rs:48-62`）带来源字段，publish system 统读即可。
> 3. **本阶段吸收 baomai_v4 voluntary-sever 断链**：`baomai_v4/dead_armor.rs:264` 的 `voluntary_sever_apply_system` emit 的正是 `MeridianSeveredEvent`（source=VoluntarySever），P0 publish 链一并覆盖，**P1 不再单列**。
> 4. VFX 过滤 `source != VoluntarySever`（voluntary 已由 zhenmai SeverChain skill 自发，避免重复）；publish system 与 VFX system 可**共用同一 `EventReader` 入口**（同 event 同 frame，读一次 event 同时 publish + 发 VFX 更省）。

交付物（可核验）：

- **模块 / 文件**：
  - `server/src/schema/`（或 `schema/meridian.rs`）— 新增 `MeridianSeveredEventV1` serde struct（字段对齐 agent `meridian-severed.ts` + 7 类 `SeveredSource`），`deny_unknown_fields`
  - `server/src/network/redis_bridge.rs`（`RedisOutbound` enum 末变体 `VoidAction`@:238 后、enum 闭合 `}`@:240 前）— `RedisOutbound::MeridianSevered(MeridianSeveredEventV1)` 变体 + `prepare_outbound_command` 序列化 arm publish 到 `bong:meridian_severed`
  - 新增 `server/src/network/meridian_severed_emit.rs` — `EventReader<MeridianSeveredEvent>` → ① `RedisOutbound::MeridianSevered`（agent 叙事）② `source != VoluntarySever` 时对 entity 位置发 `VfxEventRequest("bong:jiemai_sever_flash")`（client VFX）；`network::register` add_systems
  - agent `main.ts` — 新增 `MeridianSeveredNarrationRuntime`（包 `renderMeridianSeveredNarration` 纯函数为订阅 `MERIDIAN_SEVERED` 的 runtime，仿 `baomai-v3-runtime.ts` 结构）+ 纳入 `startAuxiliaryRuntimes` 的 `cleanupFns`
- **函数 / 符号**：`MeridianSeveredEventV1`、`RedisOutbound::MeridianSevered`、`publish_meridian_severed_events`、`MeridianSeveredEvent`、`SeveredSource`、`SEVER_FLASH_PARTICLE_ID`（`zhenmai_v2.rs:56`）；agent `MeridianSeveredNarrationRuntime`、`renderMeridianSeveredNarration`、`CHANNELS.MERIDIAN_SEVERED`
- **视听规格**：
  - VFX：复用既有 `bong:jiemai_sever_flash`（`VfxBootstrap.java:126` 已注册）——本阶段补 server emit 触发，不新增贴图；非主动断脉时在 entity 部位位置 burst
  - narration：`renderMeridianSeveredNarration` 已内置（scope=player，**style=narration**，以 `meridian-severed-narration.ts:82` runtime 模板实际值为准——非 perception）；样例方向：「肺经应声而断，你的飞剑再也认不出主人」/「这条经脉废了——它撑起的半边身法也跟着塌了」/「自断经脉的剧痛过后，是一片该死的清明」（VoluntarySever 分支已实装@`meridian-severed-narration.ts:53`）
- **测试声明**：
  - schema 对拍：`MeridianSeveredEventV1` Rust serde ↔ agent TypeBox `validateMeridianSeveredEventV1` 同一 sample 正反对拍（新增 sample，锁 7 类 `SeveredSource` 各一条 variant case）
  - publish：各来源（detection/voluntary/tribulation 至少 3 源）emit `MeridianSeveredEvent` → `bong:meridian_severed` 发布 1 条（mock redis 出站断言 payload + source）
  - VFX：非 voluntary 来源触发 1 条 `bong:jiemai_sever_flash` VfxEventRequest；voluntary 来源**不**重复发（边界 case）
  - agent：`MeridianSeveredNarrationRuntime` 收 `bong:meridian_severed` → 产 `AGENT_NARRATE`；main.ts 启动后 runtime 在 `cleanupFns`

## P1 — 爆脉 v4 反馈整桥（`baomai_v4`）

**断链（6 条，全 high，均 `baomai_v4` 自有；voluntary-sever 不在此列——它 emit 的是 `MeridianSeveredEvent`，已并入 P0 的 3 条）**：`baomai_v4` 全部事件**当前纯 write-only，零跨模块 `EventReader`**，`baomai_v4::register`（`mod.rs`）仅注册 intra-module gameplay system 不含网络。
- `baomai-v4-scar-circuit-formed-agent`（high）/ `baomai-v4-scar-circuit-broken-agent`（high）：经脉龟裂成型/断裂 → agent 叙事
- `baomai-v4-iron-cocoon-stage-up-agent-client`（high）：铁茧升档 → agent 叙事 + client event_flow（`events.rs:49` 注释明写消费方）
- `baomai-v4-crack-reading-client`（high）：裂读侦查 → client HUD overlay（`to_client_payload()`@`crack_reading.rs:221` 已写但 `#[allow(dead_code)]`，拆掉即用）
- `baomai-v4-resonance-lock-client-agent`（high）/ `baomai-v4-resonance-lock-end-client-agent`（high）：双拳共振锁定/解除 → client VFX + HUD meter + agent 叙事（`events.rs:96` 注释明写）

> **opus 验证纠偏**：① 全 6 事件共享一个新桥 `baomai_v4_event_bridge.rs`；② 桥是首个 reader，须在 `baomai_v4::register` 或 `run_server` wiring 显式 `add_systems` 注册（v4 mod 现不含网络）；③ resonance lock/end 强对称（同 handler、同 meter），合一个交付物；④ crack-reading 复用既有 `to_client_payload()`；⑤ qi_concern 全 none/low——桥只转发，真元守恒已在 gameplay 物理层（dead_armor/resonance_lock）处理，**桥不得操作真元账**；⑥ voluntary-sever 已并入 P0。

交付物（可核验）：

- **模块 / 文件**：
  - 新增 `server/src/network/baomai_v4_event_bridge.rs` — 多个 `EventReader<...>` → Redis（`RedisOutbound::BaomaiV4*`）+ S2C（`send_custom_payload(ident!("bong:iron_cocoon_stage_up" / "bong:crack_reading" / "bong:resonance_lock" / "bong:resonance_lock_end"))`）；range `network::register` add_systems
  - `server/src/network/redis_bridge.rs` — `RedisOutbound::BaomaiV4ScarCircuit{Formed,Broken}` / `BaomaiV4IronCocoonStageUp` / `BaomaiV4ResonanceLock{,End}` 变体 + arm
  - `agent/packages/schema/src/` — `BaomaiV4*` TypeBox payload + `channels.ts` 加 `bong:baomai_v4/*` + S2C ident；`samples/` 对拍；`npm run build -w @bong/schema`
  - agent 新增 `baomai-v4-runtime.ts`（订阅 `bong:baomai_v4/*`，仿 `baomai-v3-runtime.ts`）+ `main.ts` 启动
  - client 新增 `client/src/main/java/com/bong/client/combat/baomai/v4/` — `CrackReadingHandler`+`CrackReadingHud`、`ResonanceLockHandler`+`ResonanceLockMeterHud`（开始/结束两 channel 同 handler）、`IronCocoon` event_flow；`BongNetworkHandler.register()` 注册各 channel（仿 `registerMutationVisualChannel`@:395 / `registerVfxEventChannel`）
- **函数 / 符号**：`baomai_v4` 七事件、`to_client_payload`（`crack_reading.rs:221`，去 `#[allow(dead_code)]`）、`RedisOutbound::BaomaiV4*`；agent `BaomaiV4NarrationRuntime`；client `CrackReadingHandler`/`ResonanceLockHandler`/`ResonanceLockMeterHud`
- **视听规格**：
  - resonance lock：client 新增 HUD meter（共振进度条）+ 锁定/解除 VFX。**新资产视听精度（粒子基类/数量/lifetime/颜色 hex、HUD overlay 类型/opacity）须在 PR 实施前补到 docs/CLAUDE.md §视听 精度并内联本块——skeleton 阶段先占位**；各阶段（锁定/解除）差异化（memory `feedback_skill_av_diff`），不接受单方向 stub
  - crack-reading：client HUD overlay 显示敌手经脉裂纹侦查结果；payload 复用 `CrackReadingPayload` 字段
  - narration：scar circuit 成型/断裂、iron cocoon 升档、resonance 锁定/解除各一条天道旁白（scope=player/zone，style=perception/narrative）；样例须在实施前补 2-3 条
- **测试声明**：
  - schema 对拍：`BaomaiV4*` server serde ↔ agent TypeBox 同 sample 正反对拍（每变体一条）
  - emit：各事件触发后发对应 Redis/S2C 1 条；无事件不发射
  - e2e：server emit → client handler 收到 → HUD store 更新 / agent runtime 产 narrate

## P2 — 爆脉 v3 残余事件补桥（`baomai_v3`）

**断链（5 条）**：`baomai_v3_event_bridge.rs`（金标准范本，主桥 `BaomaiSkillEvent` 已通）旁，5 个机制事件漏桥：
- `baomai-v3-disperse-failed-narration`（high）：`DispersedQiEvent` 总发（`skills.rs:579`）但 `BaomaiSkillEvent` 仅 `has_transcendence` 时 emit（守卫@`skills.rs:591`），凡躯境界散功失败时 agent 完全无感知
- `baomai-v3-mountain-shake-event-agent`（medium）：`MountainShakeEvent`（AoE 命中实体数）漏桥
- `baomai-v3-blood-burn-event-agent`（medium）：`BloodBurnEvent`（`ended_in_near_death` 近死 flag）漏桥
- `baomai-v3-body-transcendence-expired-agent`（medium）：`BodyTranscendenceExpiredEvent`（涣散超越到期）漏桥
- `baomai-v3-overload-ripple-agent`（low）：`OverloadMeridianRippleEvent`（经脉过载裂纹累积）漏桥（已被 **`baomai_v4/scar_history.rs:56`** 跨模块消费——v4 reader 读 v3 event，新增 Redis reader 不破坏）

> **opus 验证纠偏（必读，防实施踩坑）**：
> 1. **disperse-failed 最简修复**：agent `baomai-v3-runtime.ts:54-58` **已有 else 失败分支**（`强行散功，凡躯没有应声`，触发条件 `flow_rate_multiplier<10`），**无需新增 agent 分支**。`BaomaiSkillEventV1.flow_rate_multiplier` schema `minimum:1`（`baomai-v3.ts:28`），**用 0 会被 schema 拒收**。正确修复 = `skills.rs` cast_disperse 改无条件 `emit_skill_event`，失败路径用 `flow_rate_multiplier=1.0`（`physics.rs` 非超越档正是 1.0）即命中既有 else。**单端单文件 + 一条 pin 测试**，优先做。
> 2. **agent runtime 当前单通道**：`BaomaiV3NarrationRuntime` 只 subscribe `BAOMAI_V3_SKILL_EVENT`，`onMessage` 对其他 channel 直接 return，`renderBaomaiV3Narration` 只 switch 6 个 skill_id。其余 4 条须**先把 `onMessage` 单通道 if 重构为 channel→handler 路由表**，再逐事件接，避免 4 次重复改 connect/onMessage。调查所标 `consumer_ready=true` 对这 4 条**不成立**，须双端动工。
> 3. overload-ripple 桥接时保持 `baomai_v4/scar_history.rs:56` 既有 reader 不破坏（Bevy 默认支持多 reader；注意该 reader 在 v4 模块跨模块读 v3 event，`baomai_v3/` 目录无 scar_history.rs）。

交付物（可核验）：

- **模块 / 文件**：
  - `server/src/combat/baomai_v3/skills.rs`（disperse）— `cast_disperse` 无条件 `emit_skill_event`，失败 `flow_rate_multiplier=1.0`
  - `server/src/network/baomai_v3_event_bridge.rs` — 新增 `publish_mountain_shake_event` / `publish_blood_burn_event` / `publish_body_transcendence_expired` / `publish_overload_ripple_event`（各 `EventReader` → `RedisOutbound::BaomaiV3*`），`network::register` add_systems
  - `server/src/network/redis_bridge.rs` — `RedisOutbound::BaomaiV3{MountainShake,BloodBurn,TranscendenceExpired,OverloadRipple}` 变体；`agent/packages/schema` 对应 channel + contract + samples
  - agent `baomai-v3-runtime.ts` — **重构为 channel→handler 路由表** + 新增 mountain-shake（命中数）/ blood-burn（近死分支）/ transcendence-expired / overload-ripple（`total_severity` 危机感）叙述
- **函数 / 符号**：`DispersedQiEvent`/`MountainShakeEvent`/`BloodBurnEvent`/`BodyTranscendenceExpiredEvent`/`OverloadMeridianRippleEvent`、`emit_skill_event`、`RedisOutbound::BaomaiV3*`；agent 路由表 + 各 render 分支
- **视听规格**：本阶段纯 agent 叙事补桥（无新 client 资产）；narration scope=player/zone style=perception/narrative，各事件 2-3 条样例实施前补
- **测试声明**：
  - disperse-failed pin：凡躯境界 cast_disperse → `BaomaiSkillEvent` 发射且 `flow_rate_multiplier=1.0` → agent else 分支命中失败叙述（**断言取 physics 非超越档返回值，不写字面**）
  - 各事件 publish：emit → 对应 Redis 1 条（mock 出站断言关键字段：mountain_shake `affected.len`、blood-burn `ended_in_near_death`、overload `total_severity`）
  - agent 路由：多通道路由表对每 channel 分发到正确 render 分支（每 channel 一条 case）

## P3 — 我流虚蚀整链激活（`woliu_v2`）

**断链（4 条，3 high + 1 medium）**：整条 `server→client/agent` 虚蚀链有严格依赖序的连续断点：
- `void-erosion-component-never-inserted`（high）：`VoidErosion` component 全仓无任何 runtime `insert`（仅 `#[cfg(test)]`），`emit_void_erosion_visual_sync`（`void_erosion_visual_emit.rs:40` 已排 schedule@`network/mod.rs:807`）查到 0 实体永久空跑
- `void-erosion-advance-event-never-emitted`（high）：`VoidErosionAdvanceEvent`（`mod.rs:28` 已 add_event）全仓无 `EventWriter`——缺 `void_erosion_check_system`（`plan-woliu-path-v1 §5.2` 设计的每 600 tick 阶段检测，未实装）
- `void-erosion-visual-client-no-receiver`（high）：`emit_void_erosion_visual_sync` 已 `send_custom_payload("bong:void_erosion_visual")`，但 client `BongNetworkHandler.register()` 无 `registerVoidErosionVisualChannel()`（client 全树 `void_erosion` 零命中）
- `void-erosion-agent-no-channel-no-runtime`（medium）：`RedisOutbound` 无 `VoidErosionEvent` 变体，`channels.ts` 无 `VOID_EROSION_EVENT`，agent 无 runtime，`woliu_event_bridge.rs` 不处理 `VoidErosionAdvanceEvent`

> **opus 验证纠偏（必读，调查低估了数据源缺口）**：
> 1. **比「只缺 insert」更深**：`cumulative_erosion` 的两个 mutator `add_erosion`（`erosion.rs:241`）/ `add_erosion_capped`（`erosion.rs:434`）**同样从未被任何 runtime system 调用**（仅测试引用）。即便 join 时 `insert VoidErosion::default()`，component 也永久停在 `stage=None`/`cumulative=0`——必须**同时补一条把虚蚀写入 `cumulative_erosion` 的 runtime 路径**（涡流施法/反噬后 `add_erosion_capped`），否则链路依旧空转。
> 2. **依赖序严格**：P3 内部 P0(数据源 insert + writer) → P1(check_system emit) → P2(client+agent 双下游 fan-out，可并行)，**切勿把 client/agent 半边先做**——在数据源落地前它们无 payload 可收。
> 3. `emit_echo_replay_vfx`（`void_erosion_visual_emit.rs:118`，`#[allow(dead_code)]` 无 runtime 调用）属本主题死代码，本阶段顺带接入 `ScheduledEcho` 触发或显式标注延后。

交付物（可核验，本阶段内部分子步骤但同一 PR 收口）：

- **模块 / 文件**：
  - 数据源：`server/src/player/mod.rs:236`（`attach_player_state_to_joined_clients` insert tuple）追加 `VoidErosion::default()`；**+ 涡流施法/反噬 system 后调 `add_erosion_capped` 累积虚蚀（仅更新 `cumulative_erosion`+`stage`，不动 qi——真元流动已在 `skills.rs:1107-1151` 走 `QiTransfer{Channeling}`，禁止在 erosion 路径重复记账）**
  - 阶段检测：`server/src/combat/woliu_v2/erosion.rs`（或 `tick.rs`）新增 `void_erosion_check_system`（`Query<(Entity,&mut VoidErosion)>`，每 `VOID_EROSION_CHECK_INTERVAL`=600 tick 比对 `computed_stage()` vs `stage`，跨阶段 emit `VoidErosionAdvanceEvent` 并更新 stage）；`woliu_v2::register` add_systems
  - client：新增 `client/.../visual/VoidErosionVisualHandler.java` + `VoidErosionVisualStore.java`；`BongNetworkHandler` 追加 `registerVoidErosionVisualChannel()`（仿 `registerMutationVisualChannel`@:395，解析 `VoidErosionVisualSyncPayloadV1` → 半透明 alpha + 声音扭曲 HUD overlay）
  - agent：`RedisOutbound::VoidErosionEvent(VoidErosionEventV1)`（schema 类型已存在）+ `woliu_event_bridge.rs` 新增 `publish_void_erosion_advance_events`（仿 `publish_woliu_v2_backfire_events`@:74）+ `channels.ts` `VOID_EROSION_EVENT:'bong:void_erosion_event'` + 新增 `void_erosion_runtime.ts`（仿 `woliu_v2_runtime.ts`）+ `main.ts` 启动
- **函数 / 符号**：`VoidErosion`、`add_erosion_capped`、`void_erosion_check_system`、`VoidErosionAdvanceEvent`、`emit_void_erosion_visual_sync`、`VOID_EROSION_VISUAL_CHANNEL`；client `VoidErosionVisualHandler`/`VoidErosionVisualStore`；agent `VoidErosionNarrationRuntime`、`RedisOutbound::VoidErosionEvent`
- **视听规格**：复用既有 `VoidErosionVisualSyncPayloadV1`（半透明 alpha 渐变 + 声音扭曲 overlay）——client handler 接既有 payload 字段驱动；新增 overlay 视听精度（tint 颜色 hex/opacity/fade 曲线/受影响境界范围）实施前补到 §视听 精度
- **测试声明**：
  - 数据源：玩家 join 后实体持 `VoidErosion`；`add_erosion_capped` 被涡流路径调用后 `cumulative_erosion` 增长（**断言 erosion 累积值 + stage 推进，不涉 qi**）；若涡流施法新增真元流动则断言其走 `QiTransfer{reason:Channeling}`（仿 `woliu_v2/tests.rs:351`）且**不与 skill 层既有 transfer 双重记账**（守恒回归取 `DEFAULT_SPIRIT_QI_TOTAL` const 引用，不写字面）
  - check_system：`cumulative_erosion` 跨 stage 阈值 → emit `VoidErosionAdvanceEvent` 且更新 stage；未跨阈值不 emit（边界 off-by-one）
  - client e2e：server emit → `VoidErosionVisualHandler` 收 `bong:void_erosion_visual` → `VoidErosionVisualStore` 更新
  - agent：`VoidErosionNarrationRuntime` 收 `bong:void_erosion_event` → 产 narrate

## P4 — 暗器分身 HUD 喂数据（`anqi`）

**断链（2 条 high，合并接线）**：`AnqiHudStateStore`（`AnqiHudPlanner` 已接 `BongHudOrchestrator:305`）仅 debug 命令写，无任何 server→client payload 喂数据：
- `anqi-decoy-deploy-no-consumer`（high）：`DecoyDeployEvent`（`anqi_v2.rs:638`，`echo_count` 已由 `density_echo` 物理函数算出）零 reader
- `anqi-hud-state-no-network-feed`（high）：`ServerDataPayloadV1` 无 `AnqiHud` 变体、无 emit system、`ServerDataRouter` 无 `anqi_hud` key

> **opus 验证纠偏**：① 两条共用同一 consumer 终点（`AnqiHudStateStore`）+ 同一新建产物（`anqi_hud` ServerData 通道），合并到一个 P 阶段，拆开会重复建管线；② **范本用 `dugu_state_emit.rs` + `DuguPoisonStateHandler.java`（ServerDataRouter store 三段）**，**不用** `void_erosion_visual_emit.rs`（后者走 VFX `send_custom_payload` 不走 ServerDataRouter store，与终点栈不一致）；③ 守恒：decoy 只读已算 `echo_count`、abrasion 只读 `after_qi`，全程无二次扣 qi；④ `anqi-echo-fractal-narration` 与 `anqi-carrier-state-hud` 调查标 non-broken（已接通），不入本阶段。

交付物（可核验）：

- **模块 / 文件**：
  - `server/src/schema/server_data.rs` — `ServerDataPayloadV1::AnqiHud(AnqiHudV1)` 变体 + `ServerDataType::AnqiHud` → `"anqi_hud"` 映射
  - **proto 链（⚠️ 必做，否则编译失败 / HUD 永收不到）**：`proto_gen.rs` 加 `AnqiHud` prost 消息 + `proto_convert.rs:438` `server_data_to_proto_payload` 新增 `ServerDataPayloadV1::AnqiHud => ...` arm（穷尽 match 无 catch-all，漏 arm 编译失败）+ client `ProtoServerDataBridge.java` `CASE_TO_TYPE` 加 `ANQI_HUD` 条目
  - 新增 `server/src/network/anqi_hud_emit.rs` — `emit_anqi_hud_payloads`（`EventReader<DecoyDeployEvent>` → echo count；`EventReader<QiInjectionEvent>`/`EventReader<CarrierAbrasionEvent>` → aim/charge/abrasion）→ `send_server_data_payload`，仿 `dugu_state_emit.rs`；`network::register` add_systems
  - client 新增 `AnqiHudServerDataHandler.java` — 实现 `ServerDataHandler`，`ServerDataRouter.createDefault()` 注册 `handlers.put("anqi_hud", ...)` → `AnqiHudStateStore.replace(echo/aim/charge/abrasion)`
- **函数 / 符号**：`DecoyDeployEvent`、`ServerDataPayloadV1::AnqiHud`、`emit_anqi_hud_payloads`、`server_data_to_proto_payload`；client `AnqiHudServerDataHandler`、`ProtoServerDataBridge.CASE_TO_TYPE`、`AnqiHudStateStore.replace`、`AnqiHudPlanner.appendEcho`
- **视听规格**：复用既有 `AnqiHudPlanner`（分身 echo count / 瞄准 aim / 蓄力 charge / 载体磨损 abrasion 渲染已实装）——本阶段纯补 payload 喂数据，无新 client 资产
- **测试声明**：
  - **proto 双端对拍**：`ServerDataPayloadV1::AnqiHud` server serde → proto 字节 → client `ProtoServerDataBridge` 反序列化得 `anqi_hud` type（锁住 proto 链不漏，不只测 JSON 路径）
  - server emit：`DecoyDeployEvent` → `anqi_hud` payload 携 `echo_count`（**取事件值不重算**）；aim/charge/abrasion 同
  - ServerData 路由：`anqi_hud` key → `AnqiHudServerDataHandler` → `AnqiHudStateStore.replace`
  - e2e：server emit → client store → `AnqiHudPlanner.buildCommands` 产非空 HudRenderCommand

## P5 — 毒蛊 v2 HUD S2C 整链（`dugu_v2`）

**断链（3 条 high，同一 client orphan）**：`DuguV2HudStateStore`（`DuguV2HudPlanner` 已接 `BongHudOrchestrator:318`）**`.replace` 全仓零调用**——5 招事件已 publish 到 Redis（agent 已叙事），但缺 server→client S2C：
- `dugu-v2-hud-store-never-written`（high，母断链）：`dugu_v2_event_bridge.rs` 五招（eclipse/penetrate/shroud/self_cure/reverse）只 `redis.tx_outbound.send`，无 `send_server_data_payload`；`ServerDataPayloadV1` 无 `DuguV2*` 变体；`ServerDataRouter` 无 `dugu_v2_*` key
- `dugu-permanent-qi-decay-no-bridge`（high）：`PermanentQiMaxDecayApplied`（`tick.rs:45`，`qi_max` 实扣@`tick.rs:43`）无 network reader
- `dugu-self-revealed-no-bridge`（high，**收窄**）：`DuguSelfRevealedEvent`（`skills.rs:303`）client HUD `selfRevealed` 永为 false

> **opus 验证纠偏（必读）**：
> 1. **self-revealed 的 agent 叙事半边已通**：`DuguSelfRevealedEvent` 与 `SelfCureProgressEvent` 在 `apply_self_cure`（`skills.rs:300-323`）成对 emit，后者带 `self_revealed` 字段经 `publish_dugu_v2_self_cure_events` → `DuguSelfCureProgressV1.self_revealed` → agent `dugu_v2_runtime.ts:51` **已叙事**。**不要**重复加 `DUGU_V2_SELF_REVEALED` channel + agent 订阅——真实缺口仅 **client S2C 半边**（HUD `selfRevealed` 更新）。独立 `DuguSelfRevealedEvent` 是否还需专用桥存疑（功能被 self_cure 字段覆盖）。
> 2. **范本用 `poison_trait_emit.rs`**（同 tick 内 Redis + S2C 双发，client `PoisonTraitServerDataHandler` → `PoisonTraitHudStateStore`），**不用** `void_erosion_visual_emit.rs`（`send_custom_payload` 直发，与 `DuguV2HudStateStore` 路由不一致）。
> 3. 守恒：三链 payload 均只读已扣量（`qi_max` 已在 tick/skills 实扣），**不可二次扣账**。

交付物（可核验）：

- **模块 / 文件**：
  - `server/src/schema/server_data.rs` — `ServerDataPayloadV1::{DuguV2SkillCast, DuguV2SelfCure, DuguV2ShroudActive, PermanentQiMaxDecayApplied}` 变体（+ `ServerDataType` 映射）
  - **proto 链（⚠️ 必做，4 个新变体逐一）**：`proto_gen.rs` 加各 prost 消息 + `proto_convert.rs:438` `server_data_to_proto_payload` 各加 arm（穷尽 match 无 catch-all）+ client `ProtoServerDataBridge.java` `CASE_TO_TYPE` 各加条目
  - `server/src/network/dugu_v2_event_bridge.rs:16` — 五招既有 publish fn 内**追加** `send_server_data_payload(client, ...)`（Redis + S2C 同 tick 双发）；新增 `publish_permanent_qi_max_decay_to_client`（`EventReader<PermanentQiMaxDecayApplied>`）；self-revealed 走 S2C（client 半边）
  - client 新增 `DuguV2ServerDataHandler.java` — `ServerDataRouter` 注册各 key → `DuguV2HudStateStore.replace`（含 `selfRevealed`/`selfCurePercent`/`tainted`）
- **函数 / 符号**：`dugu_v2_event_bridge.rs` 五招 fn、`PermanentQiMaxDecayApplied`、`DuguSelfRevealedEvent`、`ServerDataPayloadV1::DuguV2*`、`server_data_to_proto_payload`；client `DuguV2ServerDataHandler`、`ProtoServerDataBridge.CASE_TO_TYPE`、`DuguV2HudStateStore.replace`、`DuguV2HudPlanner.buildCommands`
- **视听规格**：复用既有 `DuguV2HudPlanner`（5 招状态 / 永久真元上限衰减条 / 形貌暴露标记渲染已实装）——本阶段纯补 S2C payload + handler
- **测试声明**：
  - **proto 双端对拍**：各 `DuguV2*` / `PermanentQiMaxDecayApplied` 变体 server serde → proto 字节 → client `ProtoServerDataBridge` 反序列化得对应 type（锁 proto 链）
  - server S2C：五招各 emit → 对应 `ServerDataPayloadV1::DuguV2*` 1 条（同 tick Redis 仍发，回归不破坏 agent 链）
  - permanent-qi-decay：`PermanentQiMaxDecayApplied` → S2C payload 携 `loss`（**只读，不重扣**）
  - self-revealed：触发 → client HUD `selfRevealed=true`（agent 链回归仍走 self_cure 字段）
  - e2e：server → `DuguV2ServerDataHandler` → `DuguV2HudStateStore.replace` → `DuguV2HudPlanner` 产非空命令

## P6 — 剑道人剑共生 HUD + 蜕壳灰烬回收（`sword_path` + `tuike`）

**断链（3 条，2 high + 1 medium）**：
- `sword-bond-hud-state-emit`（high）：`SwordBondComponent`（`bond.rs:16`，`bond_strength`/`stored_qi`/`grade` 实时维护）无 emit system；`SwordBondHudStateStore.replace` 生产代码零调用（仅 `SwordPathHudPlannerTest.java:99` 引用，`SwordPathHudPlanner` 已接 `BongHudOrchestrator:323`）
- `tuike-ash-decay-no-reader`（high）：`FalseSkinDecayedToAshEvent`（`tick.rs:177-195`）零 reader——灰烬道具未入背包、无 VFX、无叙事
- `tuike-permanent-taint-no-reader`（medium）：`PermanentTaintAbsorbedEvent`（`skills.rs:241-249`）零 reader——上古皮吸收永久衰败无叙事/VFX 差异化

> **opus 验证纠偏（必读）**：
> 1. **sword_bond client 接线 = `ServerDataRouter` handler-map 模式**（仿 `DuguPoisonStateHandler` → `handlers.put("sword_bond_hud_state", ...)`），**不是** `applyDispatch` getter（`ServerDataDispatch.java` 无 `swordBondHudState()` getter）；**且须补齐完整 proto 链**：`server_data.rs` 加变体 + `proto_gen.rs`（prost 消息）+ `proto_convert.rs`（`ServerDataPayloadV1::SwordBondHudState` → `Payload::SwordBondHudState`）+ client `ProtoServerDataBridge.java` 加 case——单加 `server_data.rs` 一个变体不够。`HEAVEN_GATE_THRESHOLD` 常量当前不存在，实现时新建。范本 `dugu_state_emit.rs`/`yidao_state_emit.rs`（每秒节拍 Query 组件单向推送）。
> 2. **tuike 灰烬道具 id 必须用 `event.output_item_id`**（事件 payload 已携带）——**不可写死 `FALSE_SKIN_ASH_ITEM_ID`**：`state.rs:83-88` `residue_output_item_id()` 对 **Ancient tier 返回 `FALSE_SKIN_ANCIENT_RELIC_SHARD_ITEM_ID`**，写死会让上古皮误掉普通灰烬。
> 3. tuike permanent-taint 范本照 `tuike_event_bridge.rs:62-79`（`ContamTransferredEvent` 读取并发 `TuikeV2SkillEvent`）补 reader；qi 守恒：`PermanentQiMaxDecay` 已 remove（`skills.rs:232`），叙事须明示永久衰败被皮吸收、不再扣 `qi_max`。

交付物（可核验）：

- **模块 / 文件**：
  - sword_path：新增 `server/src/network/sword_bond_state_emit.rs`（`emit_sword_bond_hud_state_payloads`，每秒节拍 `Query<(Entity,&mut Client,&Username,Option<&SwordBondComponent>)>` → `send_server_data_payload(ServerDataPayloadV1::SwordBondHudState)`，仿 `dugu_state_emit.rs`）+ `server_data.rs` 变体 + `proto_gen.rs`/`proto_convert.rs` + `network::register` 注册；client 新增 `SwordBondHudStateHandler.java` + `ServerDataRouter` 注册 `"sword_bond_hud_state"` + `ProtoServerDataBridge` case
  - tuike 灰烬：新增 `server/src/network/tuike_ash_emit.rs`（`EventReader<FalseSkinDecayedToAshEvent>` → `add_item_to_player_inventory(event.output_item_id)` + `VfxEventRequest("bong:tuike_ash_burst")` + `RedisOutbound::TuikeAshDecay`）+ `channels` `TUIKE_ASH_DECAY` + `network::register` 注册（**粒子 id 用 `bong:tuike_ash_burst`——裸 `ash_burst` 已被 `AshFootprintTracker.java:27/38` 占用为脚印 kind**）
  - tuike permanent-taint：`tuike_event_bridge.rs` 的 `publish_tuike_v2_skill_events` 追加 `EventReader<PermanentTaintAbsorbedEvent>` 分支（构 `TuikeSkillEventV1(skill_id=TransferTaint, permanent_absorbed=...)` send）
- **函数 / 符号**：`SwordBondComponent`、`ServerDataPayloadV1::SwordBondHudState`、`emit_sword_bond_hud_state_payloads`、`SwordBondHudStateStore.replace`、`SwordPathHudPlanner`；`FalseSkinDecayedToAshEvent`、`add_item_to_player_inventory`、`residue_output_item_id`；`PermanentTaintAbsorbedEvent`、`publish_tuike_v2_skill_events`
- **视听规格**：
  - sword_bond：复用既有 `SwordPathHudPlanner`（品阶图标 / storedQi 竖条 / bond 弧 / `heavenGateReady` 脉冲已实装）——纯补 payload；`storedQiRatio = stored_qi / grade.stored_qi_cap()` 只读
  - tuike 灰烬：VFX `bong:tuike_ash_burst`（蜕壳化灰）须差异化于既有 `bong:tuike_shed_burst`（`skills.rs:172` / `BongAnimations.java:57`）；新粒子视听精度实施前补到 §视听
  - narration：灰烬回收 / 上古皮吸永久衰败各一条（scope=player，permanent-taint 须明示守恒「衰败已被皮吸收」）
- **测试声明**：
  - sword_bond：proto 双端对拍（`SwordBondHudState` server serde ↔ wire ↔ client）；emit system Query 命中 + INACTIVE 不漏发；e2e server→handler→store→planner 产非空命令
  - tuike 灰烬：`FalseSkinDecayedToAshEvent` → `add_item_to_player_inventory` 调用，**Ancient tier 入 relic shard / 其余入 ash（取 `output_item_id` 分支，专属边界 case）**；VFX + Redis 各发 1 条
  - tuike permanent-taint：emit → `TuikeV2SkillEvent(permanent_absorbed)` 1 条；agent 叙事含守恒文本

## §8 开放问题（P0 决策门前需收口）

> **严禁带着开放问题进 P0 实施**（docs/CLAUDE.md §五）。实施前须追加 `## §8.1 决议（pre-P0 收口，YYYY-MM-DD）`，每条对应「文件:行号 + plan 章节」双锚点，决议数据靠 Explore agent 并行核查代码产出。
>
> **⚠️ 行号锚点刷新**：本 plan 全部 `文件:行号` 锚点经验证 workflow（2026-06-03）抽查，符号/文件名/范本均真实，但存在若干 ±1~17 行 off-by-one（代码持续演进所致）。**consume-plan §8.1 收口阶段须对全部行号锚点统一 grep 刷新一次**（符号名为准、行号为辅）。已知偏移已就地修正主要落点；其余按符号 grep 定位即可。

1. **#1 `MeridianSeveredEventV1` server serde struct 形状（P0 前置）**：从 agent `meridian-severed.ts` TypeBox 导出 JSON Schema 再生成 Rust serde，还是手写对齐？7 类 `SeveredSource` 的 wire 表示（字符串 enum vs tagged）？权威形状定哪端？
2. **#2 baomai_v4 / dugu_v2 新增 schema payload 的权威形状（P1/P5）**：`BaomaiV4*` 与 `DuguV2*` payload 字段集、S2C ident 命名（`bong:baomai_v4/*` vs `bong:iron_cocoon_stage_up` 扁平）须先定，再双端对拍。
3. **#3 P2 agent runtime 多通道路由重构边界（P2）**：`BaomaiV3NarrationRuntime` 重构为 channel→handler 路由表的接口形态——是否同时影响其他单通道 runtime（统一抽象 vs 仅 baomai_v3 局部）？
4. **#4 yidao 患者诊断面板整块接线（第 27 条 `jingmai-sever-yidao-hud-count`，本 plan 范围外）**：`build_yidao_hud_state`（函数@`yidao_state_emit.rs:125`，`severed_meridian_count:0` 硬编@:162）的 `severed_meridian_count` 与兄弟字段 `patient_hp_percent`/`patient_contam_total`（行 160-161/178-179 全硬编码 None/0）**是医者当前患者的诊断数据**，不是医者自身——需从 `HealerProfile.contracts[].patient_id`（`yidao.rs:235`，String wire-id 非 Entity）解析患者 Entity（`UniqueId→Entity` 索引 + 二次 query），`combat/yidao.rs:1993` 已有现成 `.severed_count()` 读法可借鉴。**决议：本 plan 不接此条**（属 yidao 患者诊断专项，单点接 severed 会留 hp/contam 半残），列入 `reminder.md` 或单立 `plan-yidao-patient-diagnosis-vN`。
5. **#5 P3 woliu `add_erosion_capped` runtime 触发点（P3）**：虚蚀写入 `cumulative_erosion` 挂在哪个涡流 system 后？**注意守恒已澄清（见 qi_physics 锚点）：`add_erosion_capped` 只更新 `cumulative_erosion`+`stage` 零 qi，禁止挂 `QiTransfer`；涡流真元流动已在 `skills.rs:1107-1151` 走 `QiTransfer{Channeling}`，本阶段不在 erosion 路径重复记账**。`emit_echo_replay_vfx` 死代码本阶段接入 `ScheduledEcho` 还是显式延后？
6. **#6 ServerData proto 链落点 + `HEAVEN_GATE_THRESHOLD` 常量（P4/P5/P6）**：**P4 `AnqiHud` / P5 `DuguV2*`+`PermanentQiMaxDecayApplied` / P6 `SwordBondHudState` 三阶段新增的 `ServerDataPayloadV1` 变体均须补完整 proto 链**（`proto_gen.rs` + `proto_convert.rs:438` 穷尽 match arm + client `ProtoServerDataBridge.CASE_TO_TYPE`），且都会触发现有 ServerData proto 测试夹具更新——三阶段实施前确认夹具改动范围。P6 `HEAVEN_GATE_THRESHOLD` 常量值（`stored_qi` 阈值）取自 worldview/已有设计还是新定（当前不存在，须新建）？

> §8 全部已列为开放问题，实施时以 §8.1 决议为准。**P0 仅依赖 #1 收口**即可启动（其余按对应阶段 PR 前收口）。

## §10 消费本 plan 的工作流约束（consume-plan agent 必读）

> 本 plan = 多流派反馈整链批量接线，7 个阶段对应 7 个 PR。通用约束（worktree / atomic commit / 测试全绿 / 不绕 hooks）全部生效。结构参 `plan-dandao-runtime-wiring-v1` §10 / `plan-terrain-wiring-v1` §10。

### §10.1 视觉资产 + 视听规格

- 本 plan **无 NBT 建筑**——docs/CLAUDE.md §6.1 三轮 `<PROMISE>` 不适用。
- **多数阶段复用既有 client 渲染器/HUD/VFX**（断链本质 = 消费端已就绪缺接线），纯接线按 atomic commit + 测试全绿。
- **新 client 资产**（P1 resonance lock HUD meter + crack-reading overlay、P3 void erosion overlay、P6 `bong:tuike_ash_burst` 粒子）的视听规格须在对应 PR 实施前补到 docs/CLAUDE.md §视听 精度（粒子基类/数量/lifetime/颜色 hex、HUD overlay 类型/opacity/fade、audio_recipe 层）并内联对应阶段块；各招差异化（memory `feedback_skill_av_diff`），不接受单方向 stub。
- **新 ItemTemplate**（P6 蜕壳灰烬 `tuike_false_skin_ash` / `FALSE_SKIN_ANCIENT_RELIC_SHARD` 若未注册）须补 `ItemTemplate` + `/gen-image item` 生成图标（memory `feedback_item_icon_gen`）——实施前 grep `assets/items/` 核查。

### §10.2 多 PR 序列化（依赖顺序，前一个 merge 后开下一个）

1. **PR-1（P0）** 经脉断脉整链发布 — 复用价值最高、吸收 baomai_v4 voluntary-sever，建议最先 land
2. **PR-2（P1）** 爆脉 v4 反馈整桥（含新 client v4 目录 + schema）
3. **PR-3（P2）** 爆脉 v3 残余事件补桥（含 agent runtime 多通道重构）
4. **PR-4（P3）** 我流虚蚀整链激活（含 component insert + erosion writer 守恒）
5. **PR-5（P4）** 暗器分身 HUD 喂数据
6. **PR-6（P5）** 毒蛊 v2 HUD S2C 整链
7. **PR-7（P6）** 剑道人剑共生 HUD + 蜕壳灰烬回收

> 各 PR 互相解耦（除 P0 吸收 baomai_v4 voluntary-sever 外无强依赖），但建议按序避免 `redis_bridge.rs` / `server_data.rs` / `ServerDataRouter.java` 多 PR 并行改同一文件撞 conflict。agent 改动（P0/P1/P2/P3）注意 memory `project_schema_dist_rebuild`：动 `@bong/schema` src 后须 `npm run build -w @bong/schema` 重建 dist。

### §10.3 PR 实施用独立 subagent + 模型路由

> ⚠️ **偏离 docs/CLAUDE.md §6.4 的 `model:"opus"`**——依用户强约束（memory `feedback_workflow_model_routing` + `feedback_workflow_opus_concurrency_cap`）：**写代码（实施）一律 sonnet，opus 只用于验证且并发 ≤3**。

```text
Agent(subagent_type: "claude", model: "sonnet",
      prompt: "...本 PR 范围 + §视听规格 + 测试饱和化要求 + 对应阶段 opus 验证纠偏...\n\nthink hard")
```

- 每 PR 一个独立 sonnet subagent 实施 + 提 PR；主线只收 result。
- 实施后如需对抗式核验（守恒律 / 契约对拍 / e2e 完整性 / opus 纠偏是否落实），起 opus 验证 agent，**同时并行 ≤3 个**。
- 主线 merge 命令亲自做。

### §10.4 CodeRabbit + Pi agent 等待协议

- 每 PR 等 **CodeRabbit + Pi agent (github-actions)** 两 bot（memory `feedback_wait_coderabbit_approve`）；`gh pr checks` 看状态。
- `pending` → `ScheduleWakeup delaySeconds=1200`（20min/回合，最多 3 回合 = 60min 卡死交人工）；修完意见**重新等 CR re-review**，不自判通过。
- snapshot CI 长期坏（memory `project_snapshot_ci_broken`，缺 env 非代码缺陷）不阻塞 merge；e2e 才是真集成 gate。
- 多 PR 各自走完整等待协议，前一个 APPROVED + merge 后才开下一个。

### §10.5 单次 consume-plan 全自动到 merge + 归档

用户提交 `/consume-plan combat-skill-feedback-bridges-v1` 后即可下班——consume-plan agent 在 worktree 内按 §10.2 七 PR 序列依次实施（sonnet subagent）、依次等 CR/Pi approve（ScheduleWakeup 驱动）、依次 merge，全部 land 后填 `## Finish Evidence` 并 `git mv` 入 `docs/finished_plans/`。

## Finish Evidence

> 迁入 `docs/finished_plans/` 前必填（落地清单 / 关键 commit / 测试结果 / 跨仓库核验 / 遗留）。当前 P0–P6 均 ⬜，未填。
