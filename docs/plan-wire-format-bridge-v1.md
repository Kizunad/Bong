# plan-wire-format-bridge-v1

**主题**：proto→JSON 桥接契约修复 —— `plan-protobuf-ipc-v1` 把 S2C `bong:server_data` 从 legacy JSON 迁到 protobuf 后，客户端 `ProtoServerDataBridge` 只给 14 个 payloadCase 写了专属 fixup，其余走通用 `JsonFormat.printer()`。凡 handler 读**枚举做小写比较 / 读被拍平的坐标 / 字段名漂移**，而 proto3 canonical JSON 实际输出枚举全名 / flat 标量 / 改名字段，即**静默 null / noOp，无异常无日志**。全仓契约审计确认 **73 条**（底账 `docs/wire-format-bridge-audit-report.md`）。

> **本 plan 性质**：纯基建 / 正确性修复。无 worldview 锚点、无 qi_physics、无新视听规格（症状恰恰是既有 HUD/交互/状态**已实现却静默失效**）。核心是跨仓库 wire 契约对齐。
>
> **⚠️ 收口重估（2026-07-02，见 §8.1）**：升 active 前 3 个 Explore agent 实地核查代码，两处**推翻原骨架假设**：
> 1. **origin/main 已独立修掉 9 条**（#812/#814/#815：P2/RC3 坐标簇 8 条 + P1/RC2 枚举 2 条），§8 #4 的"撞车防护"**已过期**（fix agent 早合并）。
> 2. **P0/RC1（14 条 uint64）大概率已不是活 bug**——`ProtoServerDataBridge.printAndNormalize()` 里的 `normalizeNumericStrings()`（`ProtoServerDataBridge.java:1028-1073`，2026-05-24 引入，早于审计）递归把所有 uint64 字符串转回 JSON number，对**全部 124 payloadCase 无差别生效**（通用路径 + 全部 fixup 的公共前置步骤），且有生产路径测试兜底。审计报告全文没提这个函数，属**跨跳漏判**。P0 改为"fixture 逐条实证 → 摘除已覆盖 → 仅保留边界缺口"，**不要盲改 36 处 `readLong` reader**。
>
> 重估后**真实主体工作 = P1/RC2 枚举前缀 32 条**（HudRealmGate 恒判醒灵、锻造/炼丹全卡都在这），加 P2 剩 1 条 + P3 11 条 + P4 2 条 + P5 回归 pin。

> **证据底账**：`docs/wire-format-bridge-audit-report.md`（73 条逐条：proto 实际形状 / handler 期望 / 运行时后果 / file:line / 修法）。**审计后 origin 已修 9 条见 §8.1 #4 残余表**。

## 阶段总览

| 相位 | 根因 | 原始 | origin 已修 | **剩余待修** | 状态 |
|------|------|------|------------|-------------|------|
| **P0** | RC1 uint64→JSON 字符串 | 14 | — | **~0**（§8.1 #2：`printAndNormalize` 已全局覆盖，仅 fixture 实证 + `>Long.MAX_VALUE` 边界缺口） | ⬜ |
| **P1** | RC2 枚举名未剥前缀 | 34 | 2（#815） | **32**（主体工作，20 crit） | ⬜ |
| **P2** | RC3 坐标/复合被拍平 | 12 | 8（#812/#814） | **1 真活**（`LingtianSessionHandler`）+ `rift_portal_state.direction` 配套（属 P1）+ 2 warn（proto 无字段，归 P3） | ⬜ |
| **P3** | RC6 字段名漂移 / proto 里不存在 | 11 | — | **11**（含 2 条从 P2 移入） | ⬜ |
| **P4** | RC4 内嵌 JSON + RC5 其他形状 | 2 | — | **2** | ⬜ |
| **P5** | 收尾：`faction_war_state` UNROUTED + 防复发回归 pin（全 payloadCase round-trip 守卫） | — | — | — | ⬜ |

验收日期：全相位 ✅ 后填。

## 接入面（跨仓库 wire 契约）

- **上游 / 进料**：server `server/src/schema/proto_convert.rs`（内部结构体 → proto 消息，坐标 `[f64;3]` 在此被拍平成 `*_x/*_y/*_z`；枚举经 `*_to_proto` 映射成真 proto enum）+ `proto/bong/envelope.proto` / `proto/bong/common.proto`（wire 契约 source of truth）。
- **下游 / 出料**：client `client/src/main/java/com/bong/client/network/ProtoServerDataBridge.java`（proto → legacy JSON 信封）→ `ServerDataRouter`（typeString → handler）→ 各 handler → HUD Planner / Store / Screen。
- **共享契约 symbol**：`Envelope.ServerDataEnvelope.PayloadCase`、`ProtoServerDataBridge.CASE_TO_TYPE`（`:46-183`，覆盖 124 case）、typeString 常量、`stripEnumPrefix()`（`:993-999`）/ `normalizeRealmField()`（`:1001-1010`）/ `printAndNormalize()`（`:1030-1037`，含 `normalizeNumericStrings` `:1039-1073`）/ `wrapLegacy()`（`:982-991`）；各 handler 私有 reader `readLong` / `readNonNegativeLong` / `readFlatVec3` / `readIntArray3` / `readString`。
- **不新增 schema / event / component**：本 plan 只修既有契约的**编解码形状对齐**，不引入新 payload。任何"顺手加字段"都属越界（真要加字段走 `reference_server_data_payload_field` 六点双端流程）。P3/P4 里少数 "proto 根本没这字段" 的例外确需 server 补字段时，逐条走双端 sample + `.proto` + `proto_convert.rs`（见 §8.1 #3）。
- **红旗自检**：不碰 qi 守恒、不碰 worldview、不新增物理常数——纯 client 解码修复 + 必要时 server proto 侧调整。

## 根因与统一修法（架构决策见 §8.1）

三大簇（RC1/RC2/RC3）**都源于同一咽喉点**：generic path 的 `JsonFormat.printer().preservingProtoFieldNames().includingDefaultValueFields()`（`:40-43`）输出与 handler 期望不一致。proto3 canonical JSON 铁律：① 64 位整数 → **JSON 字符串**（**但 `normalizeNumericStrings` 已在桥层全局转回 number**，见重估）；② enum → **枚举值全名字符串**（带前缀）；③ 内部 `[f64;3]` 已在 `proto_convert.rs` 拍平成三标量，**JSON 里没有数组字段**。

**§8.1 收口后的统一修法（现状即双层，维持不新造机制）**：
- **枚举归一（RC2 主体）**：消费端白名单式桥层 fixup —— 比照 `bridgeContainerState`（#815，`:684-695`）/ `bridgeDeathScreen`（`:732-751`）逐字段 `stripEnumPrefix(root, field, prefix)`。**不上"`.proto` 生成元数据表"**（生成器不存在，成本 > 收益）、**不上"运行时启发式剥全大写前缀"**（有误伤风险须白名单，本质绕回手写）。`player_state.realm` 已有现成 `normalizeRealmField()`（`:1001-1010`）helper，只差接到 `PLAYER_STATE` case——这类"helper 已存在只差接线"优先接线。
- **坐标重组（RC3 残余）**：6/7 高危 payload 已在消费端各自处理（`readFlatVec3`）。唯一剩 `LingtianSessionHandler`，就地改，不上桥层通用 helper（桥层重组成数组只是多一层中转，消费端仍要解析）。
- **RC1**：见 §8.1 #2（fixture 实证优先，不盲改 reader）。

---

## P0 — RC1：uint64 → JSON 字符串（14 条，⚠️ 重估后 ~0）⬜

**原症状**：proto 的 `uint64/int64/sint64/fixed64` 经 proto3 JSON 序列化为字符串，handler 私有 `readLong` / `readNonNegativeLong` 先做 `!isNumber()` 判空 → 对字符串恒 null → handler noOp。

**⚠️ 收口重估（见 §8.1 #2）**：`ProtoServerDataBridge.normalizeNumericStrings()`（`:1039-1073`）已在桥层递归把匹配 `INT64_STRING`（`:1028`，`-?\d{1,20}`）的 JSON 字符串**原地转回 JSON number**，是通用路径（`:315`）+ 全部 14 个专属 fixup 的公共前置（经 `printAndNormalize` `:1030-1037`），对**全部 124 payloadCase 无差别生效**。生产路径测试 `ProtoServerDataBridgeTest.java:696-756`（顶层）+ `:1087-1134`（嵌套/repeated）已断言 uint64 落地为 number。**故 14 条 RC1 大概率已被覆盖，非活 bug**。

**交付物 / 抓手（P0 = 实证 + 守卫，不是盲改）**：
1. 对 plan 列的 14 条 RC1 逐条写 fixture 测试（喂真实 proto3-JSON 形状 → `ProtoServerDataBridge.bridge()` → 断言对应 handler 非 noOp、字段落地）——这是 P5 round-trip 守卫的子集，提前做。
2. **预期多数 PASS**：PASS 的从清单摘除，在本节标注"已被 `printAndNormalize` 覆盖（fixture 实证 YYYY-MM-DD）"。
3. **残留缺口**：`normalizeNumericStrings` 对 `> Long.MAX_VALUE` 的 >19 位真 uint64 会 `catch(NumberFormatException)` 静默保留字符串（`:1046-1050`）——若某条受影响 handler 的 id/tick 可能触达该量级（游戏内罕见），才需修：优先抽共享 `JsonReaders.readLong`（容忍字符串数字），仓库已有 `SpiritTreasureJson.readLong` / `SkillBarConfigHandler.readLong`（package-visible static，`TechniquesSnapshotHandler:79` 已跨文件调用）两个先例。**不逐处改 36 个 reader 副本**（易改出 36 种不一致）。
4. 若 fixture 意外揭示某条**真的**破损（绕过 `printAndNormalize` 的路径），按残留缺口同法修 + case 锁死。

**受影响 handler reader 清单**（36 处，供 fixture 定位；`isNumber()` 门在这些行）：`SocialServerDataHandler:396`、`LootContainerHandler:156`、`DefenseWindowHandler:43`、`DeathCinematicPayloadParser:85`、`FullPowerStateHandler:113`、`ExtractServerDataHandler:108`、`CastSyncHandler:135` 等（全清单见底账 RC1 节）。

---

## P1 — RC2：枚举名未剥前缀（34 条，剩 32，主体工作）⬜

**症状**：proto enum 经 proto3 JSON 输出**枚举值全名**（`EXPOSURE_KIND_CHAT` / `REALM_CONDENSE` / `SKILL_ID_HERBALISM` / `FORGE_STEP_TEMPERING` …），handler 用无前缀小写字面量 `equals`/`switch` 比对 → 恒 default/noOp。generic path 无 `stripEnumPrefix`。

**修法（§8.1 #1）**：消费端白名单式桥层 fixup，比照 `bridgeContainerState`（#815）/ `bridgeDeathScreen`：每个受影响 payloadCase 加一个 `bridgeXxx` 方法（或扩现有），逐字段 `stripEnumPrefix(root, field, prefix)`；嵌套/repeated 字段遍历数组元素剥。`player_state.realm` **优先接现成 `normalizeRealmField()`** 到 `PLAYER_STATE` case（`bridgeCultivationDetail:880` / `bridgeCraftRecipeList:925` 已在用）。

**已修（#815）**：`container_state.kind`（剥 `CONTAINER_KIND_`）、`container_state.locked`（剥 `KEY_KIND_`）。

**剩余 32 条清单**（🔴crit / 🟡warn / ⚪info；file:line 见底账 RC2 节）：
- 🔴 `social_exposure.kind`、`rift_portal_state.direction`（**配套 P2 rift world_pos 才真正恢复 TSY 撤离/Y 键**）、`search_aborted.reason`、`yidao_hud_state.active_skill`、`skill_xp_gain.skill` / `skill_lv_up.skill` / `skill_cap_changed.skill` / `skill_scroll_used.skill`（四招式事件同源 `skill_id_to_proto`）、`player_state.realm`（**HudRealmGate 恒判 Awaken，所有境界门控 HUD 永不解锁**；接 `normalizeRealmField` 到 `PLAYER_STATE`）、`alchemy_outcome_resolved.bucket`、`forge_session.current_step`（`bridgeForgeSession:553-568` 只摊平 `step_state` oneof，顶层 `current_step` 未剥）、`alchemy_contamination.levels[].color`、`botany_plant_v2_render_profiles.profiles[].model_overlay`、`gathering_session.target_type`、`lingtian_session.kind`、`carrier_state.phase`、`false_skin_state.layers[].tier`、`qi_color_observed.main/secondary`、`spiritual_sense_targets.entries[].kind`、`event_alert.event`
- 🟡 `rift_portal_state.kind`、`extract_started.portal_kind`、`extract_aborted.reason`、`extract_failed.reason`、`inventory_snapshot.forge_color`、`forge_outcome.bucket/color`、`gathering_session.quality_hint`、`realm_vision_params.fog_shape`、`spirit_treasure_dialogue.tone`
- ⚪ `craft_outcome.failed.reason`、`false_skin_state.kind`

**交付物 / 抓手**：新增/扩桥层 `bridgeXxx` 枚举归一；每受影响 handler fixture 测试（喂枚举全名 → 断言映射到期望小写值 / 命中正确 switch 分支）。**特别锁 `player_state.realm`**：喂 `REALM_CONDENSE` → 断言 HudRealmGate 判凝脉而非醒灵。

---

## P2 — RC3：坐标/复合被拍平（12 条，剩 1 真活）⬜

**症状**：server 内部 `[f64;3]` / 嵌套坐标在 `proto_convert.rs` 拍平成 `world_pos_x/y/z` 等，proto3 JSON 无数组字段；handler 却 `readDoubleTriple` / `readIntArray3(... "pos")` → null / 错误 fallback。

**已修（#812/#814）8 条**：`rift_portal_state.world_pos`、`container_state.world_pos`、`alchemy_furnace.pos`、`botany_harvest_progress.target_pos`、`breakthrough_cinematic.world_pos`、`inventory_event.from/to`（顺带剥 `EQUIP_SLOT_`）、`inventory_event.world_pos`、`dropped_loot_sync.drops[].world_pos`。

**剩余真活 bug（1 条）**：
- 🔴 `lingtian_session.pos` —— `LingtianSessionHandler.java:38` 用 `readIntArray3(p, "pos")`（`:91-101`，要求 `isJsonArray()`），但 `envelope.proto:1352-1354` 是 `pos_x/pos_y/pos_z` flat int32，无 `pos` 数组 → 缺失时**静默 fallback `{0,0,0}`**（比 noOp 更隐蔽）。修：改读 `pos_x/pos_y/pos_z`（复制 `ExtractServerDataHandler.readFlatVec3` 的 int 版本，`:128-136`）。

**配套（跨相位）**：`rift_portal_state.world_pos` 虽已修，但同 payload 的 `rift_portal_state.direction`（P1 枚举）不修则 TSY 撤离/Y 键功能仍不通——**P1 修 direction 时一并验证 rift_portal_state 端到端恢复**。

**移入 P3 的 2 条 warn**（proto 里根本无此字段，非坐标问题）：`player_state.zone_label / zone_spirit_qi`（`PlayerStateHandler:48,67,69` 读，proto 无）、`mining_progress.display_name / mineral_id`（proto `MiningProgress` 无）——逐条判"server 补字段 or 删读"，归 P3/RC6。

**交付物 / 抓手**：`LingtianSessionHandler` 改 flat 读 + fixture（喂 flat `pos_x/y/z` → 断言 `pos != {0,0,0}` 得正确坐标）。若顺手收敛 `readFlatVec3` 重复副本（`ContainerInteractionHandler:151` / `ExtractServerDataHandler:128` 两份几乎逐行相同）成共享工具，可与 §8.1 #2 的共享 reader 一并做。

---

## P3 — RC6：字段名漂移 / proto 里不存在（11 条 + 从 P2 移入 2 条）⬜

**症状**：handler 读的字段名在 proto 消息里根本不存在（命名漂移 / 多别名兜底 / 被拍平重命名）。**逐条判**：proto 侧字段名错、handler 读错名、还是需 server 补字段（后者动 `.proto` + `proto_convert.rs` + samples 双端，见 §8.1 #3）。

**缺陷清单**（file:line 见底账 RC6 节）：
- 🔴 `craft_recipe_list.recipes[].station`（proto 需加字段）、`event_alert.severity`、`ui_open.template_id`
- 🟡 `techniques_snapshot.aliases`、`combat_event.*`（多别名兜底 school/uuid/direction/kill/perfect 一大簇，需核对 proto 实际字段名）、`heart_demon_offer.choices[].alignment / cost_summary / cost_flavor`、`event_alert.effect`
- ⚪ `zone_info.display_name`、`craft_session_state.error`、`combat_event.color`、`event_alert.duration_ms`
- **（从 P2 移入）** 🟡 `player_state.zone_label / zone_spirit_qi`、`mining_progress.display_name / mineral_id`

**交付物 / 抓手**：逐条定位 proto 真字段名 → 改 handler 读名 或 server proto 侧补字段（双端）；每条 fixture 锁死。

---

## P4 — RC4/RC5：内嵌 JSON 字符串 + 其他形状（2 条）⬜

- 🔴 **RC4** `loot_container_open.source_kind`（`LootContainerHandler:97-127`）：proto `string source_kind` 承载 serde 外部标签 JSON（`"{\"supply_coffin\":{\"grade\":\"legendary\"}}"`），JsonFormat 只当普通字符串输出 → handler 的 `isJsonObject()` 分支恒不进 → grade 恒 "common"。**决议（§8.1 #3）**：client 二次 `JsonParser.parseString(getAsString())` 解码（纯 client，不破 wire）。
- 🔴 **RC5** `recipe_unlocked.source.kind`（`RecipeUnlockedHandler:33-38`）：oneof 无 discriminator，形状不符，见底账 RC5 节。

**交付物 / 抓手**：二次解析；fixture 断言 grade/kind 正确还原。

---

## P5 — 收尾 + 防复发回归 pin ⬜

- **UNROUTED**：`faction_war_state` 有 proto 消息 + `CASE_TO_TYPE` 映射（`ProtoServerDataBridge:162` / `extractInner:449`）但 **`ServerDataRouter` 无 handler 注册**（grep `"faction_war_state"` 零命中）→ bridge 转出的 JSON 无人消费。决策：该 feature 是否要 client 端？要则补 handler，不要则从 `CASE_TO_TYPE` 摘除并注释。
- **★防复发回归 pin（饱和化测试硬约束）**：加一个 **round-trip 守卫测试**遍历 `CASE_TO_TYPE` 全部 payloadCase：构造非默认 proto → `ProtoServerDataBridge.bridge()` → 断言 `BridgeResult.isSuccess()` **且** 路由到的 handler 能非 noOp 解析（对含枚举/64位/坐标的消息尤其断言字段落地）。目标：任何未来"server 改 proto 形状 / 加 payload 忘写 fixup"立刻撞红。**这条是本 plan 的长效价值，不可省。**（注：#807 已有 `CASE_TO_TYPE` 映射完整性 pin，但只测映射不测 handler 语义——本守卫补 handler 侧。）
- 全绿门：`cd client && ./gradlew test build`。

---

## §8 开放问题（原骨架，保留追溯）

1. **#1 桥中心 vs 消费端 vs 双层**：RC1/RC2/RC3 的统一修法层级。
2. **#2 RC1 reader 修法**：改各处私有 `readLong`，还是抽共享 `JsonReaders`？
3. **#3 server 侧是否参与**：RC4（source_kind 双重编码）、部分 RC6（字段缺失）修在 client 还是 server？
4. **#4 与在跑 fix agent 的边界**：用户另派 agent 修 `dropped_loot_sync` world_pos（P2/RC3）。

> 全部已在 §8.1 收口。原表保留以备追溯，**实施时以 §8.1 决议为准**。

## §8.1 决议（pre-P0 收口，2026-07-02，靠 3 个 Explore agent 实地核查代码产出）

### #1 桥中心 vs 消费端 vs 双层

**决议**：
1. **维持现状双层，不新造机制**。核实：当前代码已是事实上的双层（`printAndNormalize` 全局数值归一 + 14 个白名单式专属 fixup）。
2. 枚举归一（RC2）走**消费端白名单式桥层 fixup**（逐字段 `stripEnumPrefix`），比照 `bridgeContainerState`（`:684-695`）。坐标（RC3）残余走**消费端就地改**（唯一剩 `LingtianSessionHandler`）。
3. **拒绝**：`.proto` 生成元数据表（生成器不存在，成本 > 收益）；运行时启发式剥全大写前缀（误伤风险须白名单，绕回手写）；桥层通用 `reassembleVec3`（多一层中转无收益，消费端仍要解析）。

**落点**：`ProtoServerDataBridge.java:684-695`（bridgeContainerState 范例）/ `:993-999`（stripEnumPrefix）/ `:1001-1010`（normalizeRealmField，`player_state.realm` 待接线）/ plan §P1 §P2。

### #2 RC1 reader 修法

**决议**：
1. **P0 先 fixture 实证，不盲改 reader**。`normalizeNumericStrings`（`:1039-1073`，2026-05-24 引入）已在桥层把 uint64 字符串全局转回 number，14 条 RC1 大概率已非活 bug。
2. 逐条 fixture 测试（喂真实 proto3-JSON → `bridge()` → 断言 handler 非 noOp），PASS 的从 P0 摘除并注明。
3. **残留缺口**（`> Long.MAX_VALUE` 的 >19 位真 uint64，`:1046-1050` 静默保留字符串）才修：抽共享 `JsonReaders.readLong`（仓库已有 `SpiritTreasureJson.readLong` / `SkillBarConfigHandler.readLong` 先例），**不逐处改 36 副本**（易改出不一致）。

**落点**：`ProtoServerDataBridge.java:1028-1073`（normalizeNumericStrings）/ `ProtoServerDataBridgeTest.java:696-756,1087-1134`（现有 uint64 测试）/ plan §P0。

### #3 server 侧是否参与

**决议**：
1. **RC4（source_kind 双重编码）在 client 修**：`LootContainerHandler` 二次 `JsonParser.parseString(getAsString())` 解码内嵌 JSON。纯 client、不破 wire、无双端 sample 改动。（若未来要消除 server 端"serde 外部标签塞 proto string"这个设计债，另立 plan 走 server 拆 `LootContainerSourceKindV1` 成 proto 专属字段 + 双端 sample —— 不在本 plan scope。）
2. **P3/RC6 里"proto 根本无此字段"的少数条**（`craft_recipe_list.station` / `player_state.zone_label/zone_spirit_qi` / `mining_progress.*`）确需 server 补字段：逐条判，动 `.proto` + `proto_convert.rs` + `agent/packages/schema/samples/*.json` 双端（[[feedback_resourcepack_sha1_sync]] 式同步纪律）。这类归 P3，与纯 client 改的条目分 PR。

**落点**：`LootContainerHandler.java:97-127`（RC4）/ `proto/bong/envelope.proto` + `server/src/schema/proto_convert.rs`（RC6 server 补字段）/ plan §P3 §P4。

### #4 与在跑 fix agent 的边界

**决议**：
1. **已过期，无需再防**。原 §8 #4 担心的 dropped_loot fix agent 已合并（#812/#814/#815，origin/main HEAD `0c3342003`）。
2. **反而缩减了 scope**：P2/RC3 从 12 条缩到 1 条真活（`LingtianSessionHandler`），P1/RC2 从 34 缩到 32。
3. 实施时仍按 §10.2 每 PR 开前 `git fetch` 比对 `ProtoServerDataBridge.java` / 目标 handler 是否被其他 orchestrator 动过。

**落点**：origin/main `0c3342003`（已修基线）/ 底账残余表 / plan §10.2。

---

## §10 实施工作流

scope ≫ 4 PR，单 plan 内多 PR 序列化（`docs/CLAUDE.md` §六）：

- **§10.1 推荐拆分点**（按根因，各自独立可 merge，桥层改动集中避免撞车）：
  1. **PR-1 P0 实证 + P2 Lingtian**：14 条 RC1 fixture 逐条实证（预期多数摘除）+ `LingtianSessionHandler` flat 坐标修 + 残留缺口（若有）抽共享 `JsonReaders`。纯 client，撞车面小，先行。
  2. **PR-2 P1 枚举（crit 子集）**：`player_state.realm`（接 `normalizeRealmField`）+ 四招式事件 + `social_exposure` + `rift_portal_state.direction`（配套验 TSY 撤离恢复）+ `forge_session.current_step` 等 20 crit。
  3. **PR-3 P1 枚举（warn/info 子集）**：剩余 12 条枚举归一。
  4. **PR-4 P3 RC6**：逐条定位真字段名；纯 client 改名 与 server 补字段（双端）分成两个 commit 或子 PR。
  5. **PR-5 P4 + P5**：RC4/RC5 二次解析 + `faction_war_state` UNROUTED 收尾 + round-trip 守卫回归 pin。
- **§10.2 撞车防护**：每 PR 开前 `git fetch origin && git log origin/main`，比对 `ProtoServerDataBridge.java` / 目标 handler 是否被其他 orchestrator 动过；被动过则先 merge main 进分支 + 本地 `./gradlew test` 验组合再 PR（[[feedback_consume_e2e_merge_artifact]]）。
- **§10.3 测试要求**：每条修复配 fixture 测试（喂真实 proto3-JSON 形状 → `bridge()` → 断言 handler 非 noOp + 字段落地）；P5 的 round-trip 守卫覆盖全 payloadCase。契约测不测实现（断 handler 可观察输出，不绑内部调用）。
- **§10.4 CR 等待**：每 PR `ScheduleWakeup` 1200s × ≤3 回合等 CodeRabbit（[[feedback_wait_coderabbit_approve]]），修完重等 re-review。
- **§10.5 subagent 实施**：每 PR 独立 `claude` subagent（opus + `ultrathink`），主线只收 result + merge。
- **§10.6 单次 consume 全自动到 merge**：收口已完成（本 §8.1），`/consume-plan` 即可，醒来看是否入 `finished_plans/`。

## 落地证据链

- 审计底账：`docs/wire-format-bridge-audit-report.md`（73 条逐条）
- 收口调研（2026-07-02，3 个 Explore agent 实地核查）：origin 已修 9 条（#812/#814/#815）；`printAndNormalize`/`normalizeNumericStrings`（`ProtoServerDataBridge.java:1028-1073`）已全局覆盖 RC1；桥层 14 个专属 fixup + helper 清单；36 处 reader 副本 + 两个共享先例；`LingtianSessionHandler` 唯一残余坐标 bug。
- 上游 plan：`docs/finished_plans/plan-protobuf-ipc-v1.md`（proto 迁移，本 plan 修其遗留 handler 未对齐）、`docs/finished_plans/plan-combat-skill-feedback-bridges-v1.md`（已示范 `bridgeEventStreamPush` 等枚举归一 fixup 先例）
- 关键先例代码：`ProtoServerDataBridge.stripEnumPrefix` / `normalizeRealmField` / `bridgeContainerState`（#815，比照写新 fixup）
