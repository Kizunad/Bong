# 玩法-代码断层修复调查记录

> 生成时间：2026-06-06
> 目的：按 git commit / finished plan / 代码落点，记录近期 Bong 仓库对“玩法承诺已写，但运行时 / client / agent / worldgen 未接线”的系统性修复脉络。

## 0. 调查方式

- 先用 `git log --grep` / `rg` 初筛关键词：`wiring`、`接线`、`断链`、`整链`、`补桥`、`runtime`、`payload`、`HUD`、`VFX`、`agent`。
- 尝试按用户要求拉起 10 个 `gpt-5.5` 调查节点：
  - `hive_think(models=["gpt-5.5" ×10])`：10 个节点均秒退无输出。
  - 临时注册 `~/.pi/agent/agents/gpt55-scout.md` 后用 `subagent` 跑：同样秒退无输出；临时 agent 文件已删除。
- 最终用现成 `subagent` tool 的 `scout` agent 分 10 个主题并行调查完成，并用本地 `rg` / `read` 补核关键证据。
- 本记录只读调查，不跑重型构建，不修改代码实现。

## 1. 一句话结论

近期“玩法-代码断层”修复主要集中在 **2026-05-10 至 2026-06-05**，形成了一个从零散表现层接线到系统化断链审计的演进：

1. 早期：补 VFX / client payload / HUD / 动画等“表现层最后一跳”。
2. 中期：`offscreen-war`、`terrain-wiring` 开始把世界模拟 / worldgen 的离屏与地形产物真正接进运行时。
3. 2026-06-02 起：`gameplay-broken-links-audit` 后集中修复两大主题：
   - `dandao-runtime-wiring-v1`：丹道整条 runtime 未启动、变异视觉/叙事/BOSS 等悬空。
   - `combat-skill-feedback-bridges-v1`：多战斗流派的 server 机制已生效，但 server→client / server→agent 的反馈桥系统性漏接。

这些修复的共性不是“新增玩法设计”，而是把已经存在的 producer / schema / consumer 用 **register、EventReader、RedisOutbound、ServerData/proto、client handler、agent runtime、worldgen manifest** 串成可运行闭环。

## 2. 时间线：关键 commits / plans

| 日期 | commit / PR | plan / 主题 | 修复的断层类型 | 三栈范围 |
|---|---:|---|---|---|
| 2026-05-10 | `350980c89`、`5bfb5cbe5`、`963e498f3`、`ddb010358` | `plan-vfx-wiring-v1` | gameplay 事件已有，但 VFX request / client VFX player / registry 未全接 | server + client |
| 2026-05-13 | `863afd1f5` | `plan-client-wiring-gaps-v1` | server 已发 `mining_progress` / `lumber_progress` / `trade_offer`，client router / handler / HUD store 未接 | client 为主 |
| 2026-05-31 ~ 2026-06-02 | #357/#358/#361/#363/#364/#365/#366 | `plan-offscreen-war-v1` | 离屏战死、派系消长、战争结算、HUD、agent 叙事、远视野 LOD 未闭环 | server + agent + client |
| 2026-06-02 ~ 2026-06-03 | `9b60cd64a` → `62b2cdeaf` | `plan-terrain-wiring-v1` | worldgen layout 已写但 `run_layout`→`paste_results`→`placement_manifest`→server stamp 未接 | worldgen + server |
| 2026-06-03 | `03100bb22` / #372 | `dandao-runtime-wiring-v1` P0 | `dandao::register` 未进 `run_server`；`PillIntakeTracked` 有 reader 无 writer | server |
| 2026-06-03 | `4e825f8e3` / #374 | `dandao-runtime-wiring-v1` P1 | `MutationAdvanceEvent` 无视觉 reader；`bong:mutation_visual` server 端不发；payload 字段/大小写不对齐 | server + client |
| 2026-06-03 | `3afab4c55` / #376 | `dandao-runtime-wiring-v1` P2 | `MutationNarrationRuntime` 未启动；server 无 `RedisOutbound::MutationEvent`；schema 双端不对齐 | server + agent |
| 2026-06-03 | `e7231d8da` + `d22e9db84` / #378 | `dandao-runtime-wiring-v1` P3 | `catalyst_furnace_bonus` 无 runtime 调用；后续修正双重削减 | server |
| 2026-06-03 | `f86d5c199` / #379 | `dandao-runtime-wiring-v1` P4 | 暴龙王 component / AI 函数 / client 实体已存在，但无 spawn、big-brain 集成、loot、吸取光环 | server + client |
| 2026-06-03 | `138b952a0` / #381 | `combat-skill-feedback-bridges-v1` P0 | `MeridianSeveredEvent` 多源 emit，但无 Redis / agent runtime / 非主动断脉 VFX 桥 | server + agent + client VFX |
| 2026-06-03 | `4ffca6c0a` / #384 | combat P1 | 爆脉 v4 scar / cocoon / crack / resonance 事件 write-only，无跨模块 reader / client HUD / agent 叙事 | server + agent + client |
| 2026-06-03 | `cdb09d373` / #385 | combat P2 | 爆脉 v3 五类残余事件漏桥；agent runtime 单通道需路由化 | server + agent |
| 2026-06-04 | `a187b24eb` / #390 | combat P3 | `VoidErosion` component 未 insert / cumulative 未写 / advance event 未发 / client receiver 和 agent runtime 缺 | server + agent + client |
| 2026-06-04 | `1bed3a2dc` / #400 | combat P4 | `AnqiHudStateStore` 只有 debug 写；无 `ServerDataPayloadV1::AnqiHud`、proto、client handler | server + client |
| 2026-06-04 | `358e3e594` / #402 | combat P5 | 毒蛊 v2 Redis agent 半边已通，但 HUD S2C / permanent qi decay / self-revealed client 半边缺 | server + client |
| 2026-06-05 | `41672a35c` / #404 | combat P6 | 剑道 HUD store 无网络喂数；蜕壳灰烬入包 / VFX / agent ash narration 漏最后一跳 | server + agent + client |

## 3. 类型学：玩法-代码断层的常见形态

### 3.1 register / schedule 总开关未插电

- 典型：`dandao::register(&mut app)` 从未在 `run_server()` 调用。
- 影响：事件、system、reader 全部存在，但 runtime 永远不跑。
- 修复模式：在主注册路径接入 register，并补 register pin / 行为测试。
- 证据：`docs/finished_plans/plan-dandao-runtime-wiring-v1.md` P0 / Finish Evidence。

### 3.2 EventWriter 有源、EventReader 无桥

- 典型：`MeridianSeveredEvent`、`MutationAdvanceEvent`、`baomai_v4` 多事件、`baomai_v3` 残余事件、`DecoyDeployEvent`。
- 影响：server 数值/状态已变化，但玩家、agent、VFX/HUD 无感。
- 修复模式：新增 `server/src/network/*_emit.rs` / `*_event_bridge.rs`，统一 `EventReader`，同 tick 发 Redis / ServerData / CustomPayload / VFX。

### 3.3 RedisOutbound / channel / agent runtime 缺一环

- 典型：
  - 丹道 `MutationEvent`：无 `RedisOutbound::MutationEvent`，agent runtime 未启动。
  - 断脉 `MeridianSevered`：render 函数存在，但没 runtime 订阅。
  - 虚蚀 / 蜕壳 ash：channel 有或 publish 有，但 runtime 订阅缺。
- 修复模式：`RedisOutbound` 变体 + channel 常量 + schema sample + runtime + `main.ts startAuxiliaryRuntimes`。

### 3.4 ServerData/proto/client handler 三件套缺失

- 典型：`AnqiHud`、`DuguV2*`、`PermanentQiMaxDecayApplied`、`SwordBondHudState`。
- 风险点：只加 Rust `ServerDataPayloadV1` JSON 变体会让测试假绿；生产 proto 路径仍可能漏。
- 正确模式：
  1. `server/src/schema/server_data.rs` 加变体与 wire type；
  2. `server/src/schema/proto_gen.rs` 加 prost message；
  3. `server/src/schema/proto_convert.rs` 穷尽 match 加 arm；
  4. `client/src/main/java/com/bong/client/network/ProtoServerDataBridge.java` 加 `CASE_TO_TYPE`；
  5. `ServerDataRouter` 注册 handler；
  6. proto 双端 roundtrip 测试。
- 证据：combat Finish Evidence 明确记录 AnqiHud(123)、DuguV2(124-127)、SwordBondHudState(128) 齐补。

### 3.5 payload wire 字段 / 大小写 / 类型错配

- 典型：
  - 丹道 mutation visual：server 输出 kind/body_slot 与 client enum 解析不一致，且缺 `cumulative_toxin`。
  - Anqi `PocketPouch`：Debug parse 得 `pocketpouch` 与 wire `pocket_pouch` 不一致，被 @hive 抓出。
  - QuickSlotConfig 历史风险：`quick_slot_config` vs `quickslot_config`。当前代码已通过显式 rename 和测试固定：`server/src/schema/server_data.rs` 注释说明默认 snake_case 会错，client `ProtoServerDataBridgeTest` 锁 `quickslot_config`。

### 3.6 component / 数据源本身未落地

- 典型：虚蚀 `VoidErosion`：不仅 component 未 insert，`add_erosion_capped` 也没有 runtime 调用，导致后续视觉/叙事即使接了也永远空跑。
- 修复模式：先补数据源 insert + writer + stage check system，再接下游 client/agent。

### 3.7 worldgen 产物未进入 server runtime

- 典型：terrain layout 定义、NBT、POI、flatten 逻辑已有，但 `run_layout` 不产 `paste_results`，主流程不导出 `placement_manifest.json`，server 不消费 manifest。
- 修复模式：worldgen `COMPOUND_LAYOUT_REGISTRY` / `run_layout_pass` / manifest export + server `PlacementManifest` / chunk 预分桶 stamp / block palette pin。

### 3.8 文档 / plan 自报与代码实际不一致

- 典型：`plan-terrain-wiring-v1` 遗留提到 `plan-dandao-path-v1` 阶段表虚报、`plan-woliu-path-v1` 王印台 zone 自报但 grep=0。
- 修复模式：以 grep / git log / Finish Evidence / 测试为准，过期 active plan 副本删除或归档。

## 4. 重点 plan 证据

### 4.1 `plan-dandao-runtime-wiring-v1`

主题：丹道机制到表现层代码已写齐，但 `dandao::register` 未调用，整条流派 runtime 悬空。

落地：

- P0 `03100bb22`：`server/src/main.rs` 增 `dandao::register(&mut app)`；生产服丹路径 emit `PillIntakeTracked`。
- P1 `4e825f8e3`：`server/src/network/mutation_visual_emit.rs` 发 `bong:mutation_visual`；payload 扩 `cumulative_toxin` 并对齐大小写。
- P2 `3afab4c55`：`MutationEventV1` 双端 schema 对齐；`RedisOutbound::MutationEvent`；agent `MutationNarrationRuntime` 启动。
- P3 `e7231d8da` + `d22e9db84`：催化炉加成接入 alchemy resolver，并修复双重削减。
- P4 `f86d5c199`：暴龙王 spawn、big-brain 集成层、BossDrain 守恒、loot template。

测试证据：server `7083 passed / 0 failed / 1 ignored`；agent tiandao `479 passed`、schema `446 passed`。

遗留：item 图标、BOSS VFX/HUD/audio polish、loot 实际掉落物 spawn 下游接入仍是后续项；核心 wiring 已落地。

### 4.2 `plan-combat-skill-feedback-bridges-v1`

主题：多条战斗流派底层数值逻辑已生效，但 server→client / server→agent 反馈桥漏最后一跳。

落地：

- P0 `138b952a0`：`meridian_severed_emit.rs`，断脉 Redis 叙事 + 非主动断脉 VFX。
- P1 `4ffca6c0a`：`baomai_v4_event_bridge.rs`，爆脉 v4 server→agent + server→client。
- P2 `cdb09d373`：`baomai_v3_event_bridge.rs` 扩 5 个残余事件 publish。
- P3 `a187b24eb`：虚蚀 component insert / check system / visual client / agent runtime。
- P4 `1bed3a2dc`：暗器 HUD `AnqiHud` S2C + proto + client handler；后续修正 per-dim store、tick CAS、wire str。
- P5 `358e3e594`：毒蛊 v2 HUD S2C + permanent qi decay + self-revealed client 半边。
- P6 `41672a35c`：剑道人剑共生 HUD + 蜕壳灰烬入包/VFX/agent 叙事。

测试证据：server `7301 passed`；client `./gradlew test build` 成功；agent tiandao `574 passed`、schema `530 passed`。

遗留：第 27 条 `jingmai-sever-yidao-hud-count`（yidao 患者诊断面板 hp/contam/severed 字段全空）超出该 plan 范围。

### 4.3 `plan-terrain-wiring-v1`

主题：建筑 layout 子系统已写但两端悬空，丹宗 / 王印台建筑未真正刷进世界。

落地：

- `9b60cd64a`：桥接 `run_layout`→`paste_results`；实装 `stamp_radial` / `block_grid`；facing/axis 旋转。
- `2695caaa8`：`architectural_layout` / `compound_flatten_radius` 透传。
- `e66c07de3`：`COMPOUND_LAYOUT_REGISTRY` + stitcher flatten/mask + export pass。
- `255c7a6a4`：server 消费 `placement_manifest`，按 chunk stamp。
- P2/P3 激活丹宗、王印台 NBT / zone / POI。

测试证据：worldgen `412 passed`；server `6997 passed`；关键 pin 包含 `authored_nbt_palette_zero_drop_in_build_placement_index`、`test_nbt_block_palette`、`test_flatten_density_mask`。

遗留：giant_sword/ambient 归 `plan-sword-path-v2`；ColumnSample 9 层消费缺口需核验已归档 plan 是否真的覆盖；block-entity 降级、iron_nugget→AIR 是后续 polish。

### 4.4 `plan-offscreen-war-v1`

主题：离屏 NPC / 散修战争从纯后台状态推进到玩家可感知的 HUD / agent 叙事 / LOD 渲染闭环。

关键修复：

- P4 #358：`OffscreenWarNarrationRuntime` 消费离屏战死，接天道叙事。
- P5 #361：`FactionStateV1` + `bong:faction_state` + agent census store。
- P6 #363/#364：`FactionWar` 四态、玩家参与、settle、Renown、`FactionWarHudLayer`、war outcome 叙事。
- P7 #365/#366：Drowsy LOD 三态、`bong:npc_lod` S2C、client `NpcLodWorldRenderer`。

测试证据：server `6968 passed`；agent schema `419` + tiandao `450`；client `./gradlew test build` +64；e2e `scripts/e2e-offscreen-war.sh` 在 PR 阶段全量/分段通过。

遗留：P7 远视野视觉仍需人工 `runClient` 目视；live↔dormant 往返丢 emergent_group 留 P6+；部分 skeleton / 孤儿副本需人工清理。

### 4.5 `plan-client-wiring-gaps-v1`

主题：server 已经发送三条 payload，但 client 端缺 handler / router / store。

落地：

- commit `863afd1f5`：注册 `trade_offer`；新增 `MiningProgressHandler` / `LumberProgressHandler`；接 `GatheringSessionStore` 和 `GatheringProgressHud`；断线清理。
- 测试：`GatheringProgressHandlerTest`、`ServerDataRouterTest`、`SocialServerDataHandlerTest`；`./gradlew test build` 1394 tests 全过。

### 4.6 `plan-vfx-wiring-v1`

主题：游戏事件存在，但 VFX request 和 client particle player 未成体系接线。

落地：

- `350980c89`：server 侧 cultivation/combat/forge/alchemy/lingtian/zhenfa/social/poison 等事件发 VFX。
- `5bfb5cbe5`：client 注册 VFX players。
- `963e498f3`：补社交关系 VFX。
- `server/src/network/gameplay_vfx.rs` 统一 helper；`VfxBootstrap.java` 与 `VfxRegistryTest` 覆盖新增 route。

遗留：未新增贴图资产，复用既有 particle sprites/providers；当时 server 大 test binary 链接在本地环境曾 SIGKILL，建议 CI 单独暴露 server `cargo test` job。

## 5. 当前值得复核的风险清单

| 优先级 | 风险 | 证据 / 状态 | 建议 |
|---|---|---|---|
| High | yidao 患者诊断面板仍可能是完整 HUD 断链 | combat plan 遗留第 27 条：`jingmai-sever-yidao-hud-count`，hp/contam/severed 全空 | 单独立/消费 yidao HUD wiring plan，需患者 Entity 解析 |
| High | ColumnSample 9 个 terrain raster 层是否真实被消费 | terrain plan 遗留 #12 指向已归档 plan，但提示需核验 Finish Evidence | 做一次 terrain layer-query 实地 grep + 测试核验 |
| Medium | 暴龙王 P4 可感知 polish / loot spawn | dandao Finish Evidence：VFX/HUD/audio/item icon、loot inventory spawn 留后续 | 若要玩家完整体验，单独做 BOSS presentation/loot spawn follow-up |
| Medium | offscreen-war 远视野 LOD 目视验证 | headless 测不到 client 真实视觉 | 人工 `./gradlew runClient` checklist |
| Medium | QuickSlotConfig wire 名称历史易复发 | 当前已修：server 显式 rename `quickslot_config`，client tests 锁定 | 保留现有 pin；新增 payload 时照此加 wire-vs-label 测试 |
| Medium | proto 新增变体时 JSON 假绿 | combat plan 多次强调 proto 生产路径与 JSON 测试路径差异 | 新增 `ServerDataPayloadV1` 必做 proto 双端 roundtrip |
| Low | plan 自报与代码实际不一致的历史债务 | terrain plan 提到 dandao-path / woliu / sword Finish Evidence 红旗 | 定期跑 plans-status / audit-plans-progress |

## 6. 可复用审计 checklist

后续任何玩法 plan 验收前，建议用以下断链 checklist 扫一遍：

1. **register**：模块 `register(&mut App)` 是否被 `run_server()` 或对应 bootstrap 调用？
2. **EventWriter/EventReader**：所有 gameplay event 是否至少有一个跨模块 reader 负责表现/叙事/telemetry？
3. **Redis**：新增 agent 叙事是否同时具备 `RedisOutbound`、channel 常量、schema sample、runtime、`main.ts` 启动？
4. **ServerData/proto/client**：新增 HUD payload 是否同时补 `server_data.rs`、`proto_gen.rs`、`proto_convert.rs`、client `ProtoServerDataBridge.CASE_TO_TYPE`、`ServerDataRouter`、handler/store？
5. **wire 名称**：server wire type、proto case、client handler key 是否一致？是否有测试锁住 snake_case / camelCase / 特殊缩写？
6. **producer 数据源**：component 是否 insert？累计值是否真实写入？check system 是否能推进？
7. **consumer 真用**：client store 是否被写？planner/renderer 是否每帧读取？agent runtime 是否产出 `AGENT_NARRATE`？
8. **worldgen**：layout/asset 是否从 generator 输出 manifest？server 是否消费 manifest？方块 palette 是否 zero-drop？
9. **守恒**：桥接 payload 只能只读上报，不得二次扣 qi / 双重记账。
10. **测试反演**：测试是否 red-when-reverted，避免“绕过 emit 函数”“只测 JSON 不测 proto”“只测 store 不测 planner”。

## 7. 本次记录的边界

- 没有改动任何业务代码。
- 没有运行 cargo/gradle/npm 全量测试；测试结果取自 finished plans 的 Finish Evidence，并辅以本地 grep 核验。
- `gpt-5.5` subagent 在当前 pi harness 下未能成功产出；实际 10 个调查分片由现成 `scout` subagent 完成。
