# plan-wire-format-bridge-v1（骨架 / 决策菜单）

**主题**：proto→JSON 桥接契约修复 —— `plan-protobuf-ipc-v1` 把 S2C `bong:server_data` 从 legacy JSON 迁到 protobuf 后，客户端 `ProtoServerDataBridge` 只给 14 个 payloadCase 写了专属 fixup，其余 ~110 个走通用 `JsonFormat.printer()`。凡 handler 读**复合字段 / 枚举做小写比较 / 读被拍平的坐标**，而 proto3 canonical JSON 实际输出 flat / 枚举全名 / 字符串化 uint64，即**静默 null / noOp，无异常无日志**。全仓契约审计确认 **73 条真缺陷（43 critical）**。本骨架把它们按根因聚成决策菜单，收口架构决策（§8）后逐相位实施。

> **本 plan 性质**：纯基建 / 正确性修复。无 worldview 锚点、无 qi_physics、无新视听规格（症状恰恰是既有 HUD/交互/状态**已实现却静默失效**）。核心是跨仓库 wire 契约对齐。
> **证据底账**：`docs/wire-format-bridge-audit-report.md`（73 条逐条：proto 实际形状 / handler 期望 / 运行时后果 / file:line / 修法，按根因分节）。本 plan 的相位抓手与该报告一一对应。
> **审计方法**：sonnet workflow 全契约扫描（124 契约 / 25 batch）+ sonnet 对抗核验（原始 97 → CONFIRMED 73 / REFUTED 24 / clean 71），2026-07-02。

## 阶段总览

| 相位 | 根因 | 缺陷数 | 状态 |
|------|------|--------|------|
| **P0** | RC1 uint64/int64 → JSON 字符串，`readLong`/`readNonNegativeLong` 的 `isNumber()` 门恒 null | 14（8 crit） | ⬜ |
| **P1** | RC2 枚举名未剥前缀（`EXPOSURE_KIND_CHAT` vs handler 期望 `chat`） | 34（20 crit） | ⬜ |
| **P2** | RC3 坐标/复合被拍平（`world_pos_x/y/z`、`pos`、`from/to`）vs handler 读 `world_pos` 数组 | 12（10 crit） | ⬜ |
| **P3** | RC6 字段名漂移 / proto 里不存在 | 11（3 crit） | ⬜ |
| **P4** | RC4 proto string 内嵌 JSON（双重编码）+ RC5 其他形状不符 | 2（2 crit） | ⬜ |
| **P5** | 收尾：`faction_war_state` 无 handler（UNROUTED）+ 防复发回归 pin（全 payloadCase round-trip 守卫） | — | ⬜ |

验收日期：全相位 ✅ 后填。

## 接入面（跨仓库 wire 契约）

- **上游 / 进料**：server `server/src/schema/proto_convert.rs`（内部结构体 → proto 消息，坐标 `[f64;3]` 在此被拍平成 `*_x/*_y/*_z`；枚举经 `*_to_proto` 映射成真 proto enum）+ `proto/bong/envelope.proto` / `proto/bong/common.proto`（wire 契约 source of truth）。
- **下游 / 出料**：client `client/src/main/java/com/bong/client/network/ProtoServerDataBridge.java`（proto → legacy JSON 信封）→ `ServerDataRouter`（typeString → handler）→ 各 handler → HUD Planner / Store / Screen。
- **共享契约 symbol**：`Envelope.ServerDataEnvelope.PayloadCase`、`ProtoServerDataBridge.CASE_TO_TYPE`、typeString 常量、`stripEnumPrefix()` / `printAndNormalize()` / `wrapLegacy()`；各 handler 私有 reader `readLong` / `readNonNegativeLong` / `readDoubleTriple` / `readRequiredArray` / `readString`。
- **不新增 schema / event / component**：本 plan 只修既有契约的**编解码形状对齐**，不引入新 payload。任何"顺手加字段"都属越界（真要加字段走 `reference_server_data_payload_field` 六点双端流程，另开 plan）。
- **红旗自检**：不碰 qi 守恒、不碰 worldview、不新增物理常数——纯 client 解码修复 + 必要时 server proto 侧调整。

## 根因与统一修法（架构决策见 §8）

三大簇（RC1/RC2/RC3，共 60 条 / 38 crit）**都源于同一咽喉点**：generic path 的 `JsonFormat.printer().preservingProtoFieldNames().includingDefaultValueFields()` 输出与 handler 期望不一致。proto3 canonical JSON 铁律：① 64 位整数 → **JSON 字符串**；② enum → **枚举值全名字符串**（带前缀）；③ 内部 `[f64;3]` 已在 `proto_convert.rs` 拍平成三标量，**JSON 里没有数组字段**。

修法有两个层级，**必须在 §8 先拍板**：
- **桥中心（bridge-central）**：在 `ProtoServerDataBridge` 做通用兜底（64 位字段字符串容忍 / 枚举名归一 / 已知坐标重组），一处修，覆盖面广，但"哪些字段是枚举/坐标"需 per-payload 元数据，纯通用做不到 100%。
- **消费端（handler-side）**：改各 handler 的 reader（`readLong` 容忍字符串 / 消费枚举全名 / 读三标量），点多面广（如 `readLong` 有 10+ 处私有副本），但语义最清晰、无桥层魔法。

推荐**双层**：桥层通用兜底（RC1 全部 + RC2/RC3 的机械可判定部分）+ 高危 payload 专属 fixup（`bridgeRiftPortalState` / `bridgeContainerState` 等，比照既有 `bridgeDeathScreen` / `bridgeMovementState` 先例）。

---

## P0 — RC1：uint64 → JSON 字符串（14 条 / 8 crit）⬜

**症状**：proto 的 `uint64/int64/sint64/fixed64` 经 proto3 JSON 序列化为**字符串**（`"123"`），但各 handler 私有 `readLong` / `readNonNegativeLong` 先做 `!primitive.isNumber()` 判空 → 对字符串恒返回 null → 触发 handler 顶部 null-gate → 整条消息 noOp。

**决策门**：修 reader（容忍 `isString()` + 数字串解析）＝一处语义修，覆盖所有 64 位字段；须核对 ~10+ 处私有 `readLong` 副本（`SocialServerDataHandler:396` / `LootContainerHandler:156` / `FullPowerStateHandler:113` / `DefenseWindowHandler:39` / `ExtractServerDataHandler:109` / `SkillEventHandler:196` / `PoisonTraitServerDataHandler` readNonNegativeLong / `DeathCinematicPayloadParser` / `WoundsSnapshotHandler` / `FalseSkinStateHandler` / `RecipeUnlockedHandler` …）是否全部同步，或抽一个共享 `JsonReaders.readLong`（注意 [[feedback_mixin_package_helper]] 不适用，此为普通包）。

**缺陷清单**（🔴crit / 🟡warn / ⚪info；file:line 见报告 RC1 节）：
- 🔴 `social_pact.tick`、`sparring_invite.expires_at_ms`、`trade_offer.expires_at_ms`、`trade_offer.offered_item.instance_id`（社交系统整片 noOp）
- 🔴 `poison_overdose_event.player_entity_id`（过量丧命警告 HUD 不触发）
- 🔴 `loot_container_update.session_id`（战利品容器内容永不刷新）
- 🔴 `defense_window.started_at_ms / expires_at_ms`（格挡窗口 HUD）
- 🔴 `death_screen.cinematic.phase_tick / phase_duration_ticks / total_elapsed_ticks / total_duration_ticks / rebirth_weakened_ticks`（死亡 cinematic 计时全断，`DeathCinematicPayloadParser:40-50`）
- 🟡 `social_renown_delta.tags_added[].last_seen_tick`、`niche_intrusion.items_taken`、`full_power_exhausted_state.started_tick / recovery_at_tick`、`wounds_snapshot.wounds[].updated_at_ms`、`false_skin_state.equipped_at_tick`
- ⚪ `recipe_unlocked.unlocked_at_tick`

**交付物 / 抓手**：`readLong` / `readNonNegativeLong` 全副本容忍字符串数字；每个受影响 handler 加 fixture 测试（喂字符串化 uint64 → 断言非 null + 解析正确）。

---

## P1 — RC2：枚举名未剥前缀（34 条 / 20 crit）⬜

**症状**：proto enum 经 proto3 JSON 输出**枚举值全名**（`EXPOSURE_KIND_CHAT` / `REALM_CONDENSE` / `SKILL_ID_HERBALISM` / `FORGE_STEP_TEMPERING` / `COLOR_KIND_MELLOW` …），handler 用无前缀小写字面量 `equals`/`switch` 比对 → 恒 default/noOp。generic path 无 `stripEnumPrefix`（该函数目前仅在 `bridgeMovementState/CastSync/EventStreamPush/DeathScreen/CultivationDetail/CraftRecipeList` 内被调）。

**决策门**：① 桥层通用枚举归一（需知道哪些字段是枚举——可从 `.proto` 生成一张 `payloadCase→{field→enumPrefix}` 表驱动，或运行时按 `[A-Z_]+` 全大写值启发式剥前缀，后者有误伤风险须白名单）；② 或每个受影响 payload 加专属 `stripEnumPrefix`（显式安全，但 20+ 处）。**注意 `player_state.realm` 已有现成 `normalizeRealmField()` helper（`bridgeCultivationDetail` 在用）只是没接到 player_state**——这类"helper 已存在只差接线"优先接线。

**缺陷清单**（file:line 见报告 RC2 节）：
- 🔴 `social_exposure.kind`、`rift_portal_state.direction`、`search_aborted.reason`、`yidao_hud_state.active_skill`、`skill_xp_gain.skill` / `skill_lv_up.skill` / `skill_cap_changed.skill` / `skill_scroll_used.skill`（四招式事件同源 `skill_id_to_proto`）、`player_state.realm`（**HudRealmGate 恒判 Awaken，所有境界门控 HUD 永不解锁**）、`alchemy_outcome_resolved.bucket`、`forge_session.current_step`（锻造三子面板全卡死）、`alchemy_contamination.levels[].color`、`botany_plant_v2_render_profiles.profiles[].model_overlay`、`gathering_session.target_type`、`lingtian_session.kind`、`carrier_state.phase`、`false_skin_state.layers[].tier`、`qi_color_observed.main/secondary`、`spiritual_sense_targets.entries[].kind`、`event_alert.event`
- 🟡 `rift_portal_state.kind`、`extract_started.portal_kind`、`extract_aborted.reason`、`extract_failed.reason`、`container_state.kind`、`inventory_snapshot.forge_color`、`forge_outcome.bucket/color`、`gathering_session.quality_hint`、`realm_vision_params.fog_shape`、`spirit_treasure_dialogue.tone`
- ⚪ `container_state.locked`、`craft_outcome.failed.reason`、`false_skin_state.kind`

**交付物 / 抓手**：新增/接线枚举归一路径；受影响 handler fixture 测试（喂枚举全名 → 断言映射到期望小写值 / 命中正确 switch 分支）。

---

## P2 — RC3：坐标/复合被拍平（12 条 / 10 crit）⬜

**症状**：server 内部 `[f64;3]` / `Option<(i32,i32,i32)>` / 嵌套坐标在 `proto_convert.rs` 拍平成 `world_pos_x/y/z`、`pos_x/y/z`、`from_x/from_y/...`，proto3 JSON 里**没有 `world_pos` / `pos` 数组字段**；handler 却 `readDoubleTriple("world_pos")` / `readRequiredArray("world_pos")` / `p.getAsJsonArray("pos")` → 恒 null → noOp。**（`dropped_loot_sync.drops[].world_pos` 即本次触发审计的原始 bug；你的 fix agent 正在修的那条属本簇。）**

**决策门**：桥层重组数组（加 `bridgeRiftPortalState` / `bridgeContainerState` / … 读 `*_x/*_y/*_z` 合成 `world_pos:[x,y,z]`，比照现有 fixup 先例）＝ handler 零改；或改 handler 读三标量。**须与 fix agent 对齐**避免同文件撞车（见 §10 撞车防护）。

**缺陷清单**（file:line 见报告 RC3 节）：
- 🔴 `rift_portal_state.world_pos`（+ direction 枚举双杀 → 整个 TSY 撤离功能 client 端从不注册裂口、Y 键撤离永不触发）、`container_state.world_pos`（所有容器广播被丢）、`alchemy_furnace.pos`（右键炼丹炉永远打不开 GUI）、`botany_harvest_progress.target_pos`、`lingtian_session.pos`、`breakthrough_cinematic.world_pos`、`inventory_event.from/to`、`inventory_event.world_pos`、`dropped_loot_sync.drops[].world_pos`、`trade_offer.requested_items[].instance_id`
- 🟡 `player_state.zone_label / zone_spirit_qi`、`mining_progress.display_name / mineral_id`

**交付物 / 抓手**：桥层坐标重组 fixup 或 handler 三标量读取；每 payload fixture 测试（喂 flat 三标量 → 断言 handler 得到正确 BlockPos/坐标，`pos != null`）。

---

## P3 — RC6：字段名漂移 / proto 里不存在（11 条 / 3 crit）⬜

**症状**：handler 读的字段名在 proto 消息里根本不存在（命名漂移 / 多别名兜底 / 被拍平重命名）。**逐条判**：是 proto 侧字段名错、handler 读错名、还是需 server 补字段。

**缺陷清单**（file:line 见报告 RC6 节）：
- 🔴 `craft_recipe_list.recipes[].station`、`event_alert.severity`、`ui_open.template_id`
- 🟡 `techniques_snapshot.aliases`、`combat_event.*`（多别名兜底 school/uuid/direction/kill/perfect 等一大簇，需核对 proto 实际字段名）、`heart_demon_offer.choices[].alignment / cost_summary / cost_flavor`、`event_alert.effect`
- ⚪ `zone_info.display_name`、`craft_session_state.error`、`combat_event.color`、`event_alert.duration_ms`

**交付物 / 抓手**：逐条定位 proto 真字段名 → 改 handler 读名 或 server proto 侧补字段（后者动 `.proto` + `proto_convert.rs` + samples 双端）。

---

## P4 — RC4/RC5：内嵌 JSON 字符串 + 其他形状（2 条 / 2 crit）⬜

- 🔴 **RC4** `loot_container_open.source_kind`（`LootContainerHandler:97-127`）：proto `string source_kind` 承载 serde 外部标签 JSON（`"{\"supply_coffin\":{\"grade\":\"legendary\"}}"`），JsonFormat 只当普通字符串输出 → handler 的 `isJsonObject()` 分支恒不进 → grade 恒 "common"、kind 变原始 JSON 文本。修：handler 二次 `JsonParser.parseString(getAsString())`，或 server 把 `LootContainerSourceKindV1` 拆成 proto 专属字段（kind/grade/is_herb）不再双重编码。
- 🔴 **RC5** `recipe_unlocked.source.kind`（`RecipeUnlockedHandler:33-38`）：形状不符，见报告 RC5 节。

**交付物 / 抓手**：二次解析或 server 拆字段；fixture 测试断言 grade/kind 正确还原。

---

## P5 — 收尾 + 防复发回归 pin ⬜

- **UNROUTED**：`faction_war_state` 有 proto 消息 + `CASE_TO_TYPE` 映射（`ProtoServerDataBridge` line 162）但 **`ServerDataRouter` 无 handler 注册** → bridge 转出的 JSON 无人消费。决策：该 feature 是否要 client 端？要则补 handler，不要则从 `CASE_TO_TYPE` 摘除并注释。
- **★防复发回归 pin（饱和化测试硬约束）**：加一个 **round-trip 守卫测试**遍历 `CASE_TO_TYPE` 全部 payloadCase：构造非默认 proto → `ProtoServerDataBridge.bridge()` → 断言 `BridgeResult.isSuccess()` **且** 路由到的 handler 能非 noOp 解析（对含枚举/64位/坐标的消息尤其断言字段落地）。目标：任何未来"server 改 proto 形状 / 加 payload 忘写 fixup"立刻撞红，而非再等一次全仓审计。这条是本 plan 的**长效价值**，不可省。
- 全绿门：`cd client && ./gradlew test build`。

---

## §8 开放问题（P0 决策门前需收口）

1. **#1 桥中心 vs 消费端 vs 双层**：RC1/RC2/RC3 的统一修法层级。推荐双层（桥通用兜底 + 高危专属 fixup），但需定：桥层枚举归一走"`.proto` 生成元数据表驱动"还是"运行时启发式剥全大写前缀 + 白名单"？坐标重组是否统一走一个 `reassembleVec3(root, "world_pos")` helper？
2. **#2 RC1 reader 修法**：改各处私有 `readLong` 副本，还是抽共享 `JsonReaders`？共享则一处修但要改所有调用点 import。
3. **#3 server 侧是否参与**：RC4（source_kind 双重编码）、部分 RC6（字段缺失）修在 client 还是 server？动 server 则连 `.proto` + `proto_convert.rs` + `agent/packages/schema/samples/*.json` 双端（[[feedback_resourcepack_sha1_sync]] 式同步纪律），scope 更大。
4. **#4 与在跑 fix agent 的边界**：用户已另派 agent 修 `dropped_loot_sync` world_pos（属 P2/RC3）。P2 实施前须 `git fetch` 对齐，避免 `ProtoServerDataBridge.java` / `DroppedLootSyncHandler.java` 撞车（[[feedback_parallel_agent_shared_worktree]] / [[feedback_consume_e2e_merge_artifact]]）。

> 收口方式见 `docs/CLAUDE.md` §五（§8 → §8.1 决议，每条落"文件:行号 + plan 章节"双锚点，靠 Explore agent 并行核查代码现状）。**全部收口才能开 P0。**

## §10 实施工作流

scope ≫ 4 PR，单 plan 内多 PR 序列化（`docs/CLAUDE.md` §六）：

- **§10.1 推荐拆分点**（按根因，各自独立可 merge，桥层改动集中避免撞车）：
  1. **PR-1 P0 RC1**：reader 64 位字符串容忍 + fixture（纯 client，撞车面小，先行）
  2. **PR-2 P1 RC2**：枚举归一（桥层或 handler，视 §8.1 #1）
  3. **PR-3 P2 RC3**：坐标重组（**与 fix agent 对齐后**再开，见 §8 #4）
  4. **PR-4 P3+P4 RC6/RC4/RC5**：逐条 + 可能的 server proto 侧改动
  5. **PR-5 P5**：UNROUTED 收尾 + round-trip 守卫回归 pin
- **§10.2 撞车防护**：每 PR 开前 `git fetch origin && git log origin/main`，比对 `ProtoServerDataBridge.java` / 目标 handler 是否被 fix agent 动过；被动过则先 merge main 进分支 + 本地 `./gradlew test` 验组合再 PR（[[feedback_consume_e2e_merge_artifact]]）。
- **§10.3 测试要求**：每条修复配 fixture 测试（喂真实 proto3-JSON 形状 → 断言 handler 非 noOp + 字段落地）；P5 的 round-trip 守卫覆盖全 payloadCase。契约测不测实现（断 handler 可观察输出，不绑内部调用）。
- **§10.4 CR 等待**：每 PR 走 `ScheduleWakeup` 1200s × ≤3 回合等 CodeRabbit（[[feedback_wait_coderabbit_approve.md]]），修完重等 re-review。
- **§10.5 subagent 实施**：每 PR 独立 `claude` subagent（opus + `ultrathink`），主线只收 result + merge。
- **§10.6 单次 consume 全自动到 merge**：收口 §8 后 `/consume-plan` 即可，醒来看是否入 `finished_plans/`。

## 落地证据链

- 审计底账：`docs/wire-format-bridge-audit-report.md`（73 条逐条）
- 上游 plan：`docs/finished_plans/plan-protobuf-ipc-v1.md`（proto 迁移，本 plan 修其遗留 handler 未对齐）、`docs/finished_plans/plan-combat-skill-feedback-bridges-v1.md`（已示范 `bridgeEventStreamPush` 等枚举归一 fixup 先例）
- 关键先例代码：`ProtoServerDataBridge.stripEnumPrefix` / `normalizeRealmField` / `bridgeDeathScreen`（比照写新 fixup）
