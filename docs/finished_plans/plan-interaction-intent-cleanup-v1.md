# plan-interaction-intent-cleanup-v1（finished）

> **Finished（已归档，2026-06-15）**。一句话主题：把「宽泛输入自动触发 C2S」的交互绑定改为**显式意图**——神识感知类（感矿脉 / 感保鲜 / 感灵龛）做成**主动专属键位 + 目标过滤**，战斗反应类（截脉）补**窗口守卫**——消除「每次右键/凝视/按键都可能弹境界门控 narration」的出戏噪声。
>
> 立项动机：worldgen-v4 P6 真机审阅时发现，空手右键任意方块即触发感矿脉、凝脉以下每次弹「凝脉方可感矿脉。神识未及」，既出戏又跟放方块抢交互。已先行**摘除感矿脉右键绑定**（`a6aa0004b`，删 `MixinClientPlayerInteractionManagerMineralProbe`，保留底层 `sendMineralProbe` 能力）。随后一个审计 workflow 扫出 3 个同类绑定（见下），本 plan 统一收口。

## 阶段总览

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 感矿脉主动化（专属键位 N 边沿单发 + 目标过滤；**走 Route B 客户端键位，不注册 server skill**，理由见 P0） | ✅ 2026-06-15 |
| P1 | 感保鲜（freshness_probe）收窄触发（Shift+F + freshness 数据判空）+ NoFreshness 静默 | ✅ 2026-06-15 |
| P2 | 感灵龛（spirit_niche_gaze）被动凝视改主动 M 键（gaze + mark 成对）+ 揭示反馈 | ✅ 2026-06-15 |
| P3 | 截脉窗口守卫（`DefenseWindowStore.snapshot().active()`）+ V 键冲突解决 | ✅ 2026-06-15 |

## 接入面 checklist（防孤岛——升 active 前据 docs/CLAUDE.md §二 核实）

- **进料**：复用 `ClientRequestSender.sendMineralProbe / sendFreshnessProbe / sendSpiritNicheGaze / sendJiemai`（底层 C2S 能力均已存在，本 plan 只改**触发入口**，不动协议/server 处理）。
- **出料**：server 侧 `mineral_probe` / `freshness_probe` / `spirit_niche_gaze` / `jiemai` 处理器与 S2C 回执**完全不变**；只改"由什么客户端动作发包"。
- **共享类型 / event**：复用既有 `MineralProbeResult` / freshness EventAlert / `DefenseWindowStore` / `SkillBar*`；不新增协议。
- **跨仓库契约**：纯客户端触发层改造为主；**P0 最终走 Route B（客户端专属键位），未注册 server skill**，故 `SkillMeridianDependencies::declare` 红线在本 plan 不适用（理由见 P0 段「Route B 决策」）。
- **worldview 锚点**：神识感知（`worldview.md` 神识/通灵章节——感矿脉/感保鲜/感灵龛属"以神识外放感知环境"，本 plan 把它们从被动/通配触发收窄为**玩家显式按键主动发起**）。
- **qi_physics 锚点**：本 plan **不涉及真元流动/守恒**（感知类只读传输，截脉是 HUD 窗口逻辑）；未引入神识消耗 / 新物理常数（见 §N.1 决议 #1）。

## P0 — 感矿脉主动化（Route B：客户端专属键位）

- **现状**：感矿脉右键绑定已摘（`a6aa0004b`）；`ClientRequestSender.sendMineralProbe(x,y,z)` + server `MineralProbeDenialReason`（含 `RealmTooLow`→「凝脉方可感矿脉。神识未及」）+ `MineralProbeResultHandler` 均在位，**无触发入口**（历史孤岛 C2S）。
- **目标**：把感矿脉做成**玩家主动发起**的探针——看向某方块时按专属键位边沿单发一次 `mineral_probe(x,y,z)`，不再通配右键、不再每 tick 轮询；境界不足 / 非灵脉 / 超距由 server resolver 统一裁决，denial 回执由 `MineralProbeResultHandler` 渲染为一条 actionbar overlay（一次性，非刷屏）。

### Route B 决策（doc↔code 显式记录，2026-06-15）

立项时 P0 设想的是 **Route A：技能化**——把感矿脉注册进 `cultivation::skill` / `SkillRegistry`，走客户端技能栏 + F 键 cast，并按红旗清单走 `SkillMeridianDependencies::declare` 声明依赖经脉。实施时核查 server 技能签名后**改走 Route B：纯客户端专属键位（默认 N，可在原版控制里重绑），不注册 server skill**。技术理由：

- **`SkillFn` 签名无法承载方块坐标**。`server/src/cultivation/skill_registry.rs:37` 定义 `pub type SkillFn = fn(&mut World, caster: Entity, slot: u8, target: Option<Entity>) -> CastResult;` —— 技能 cast 路径**只携带 `Option<Entity>` 作为目标**，没有 `BlockPos`。而感矿脉的语义是「对**准星方块**发探针」，必须携带 `(x, y, z)` 坐标。把感矿脉塞进 skill cast 路径会强行把「方块目标」挤进「实体目标」槽位，要么扩 `SkillFn` 签名（牵动全部已注册流派招式）、要么在 cast 内重新做一次客户端→服务端的坐标回传——两者都比「客户端键位直接发已有的 `mineral_probe(x,y,z)` C2S」更绕、更脆。
- **既有 `mineral_probe` 全链路已天然承载坐标**。`mineral_probe` C2S → `MineralProbeIntent` → resolver → `mineral_probe_result` S2C 这条链路本就以坐标为参数、与 skill 系统正交。Route B 只是把客户端键位边沿接到这条现成链路的发包点，**零新增协议、零改动 server 处理、零碰 skill 系统**。
- **因此 `SkillMeridianDependencies::declare` 红线在本 plan 不适用**：本 plan 没有任何 `SkillRegistry::register` / `register_skills` 新增调用，不存在「招式注册漏声明依赖经脉」的风险面。境界 / 经脉门控仍由 server resolver 侧裁决（`MineralProbeDenialReason::RealmTooLow` 等），不绕过任何门控。

> 备注：这是对立项 §N 开放问题 #2/#4（感知是否做成独立技能 / 是否技能化）的实现期收口——感知类**不进 skill 系统**，统一走客户端专属键位主动触发。若未来要让感矿脉吃神识消耗 / 经脉依赖 / 熟练度成长，需要的是给 `mineral_probe` resolver 链路单独挂这些规则，而非把它塞进 `SkillFn`。

- **落地交付物**：`client/.../mineral/MineralSenseBootstrap.java`（专属键位 `key.bong-client.mineral_sense` 默认 `GLFW_KEY_N`，`ClientTickEvents.END_CLIENT_TICK` 内 `while (senseKey.wasPressed())` 边沿单发，空准星 no-op）→ `ClientRequestSender.sendMineralProbe(x,y,z)`；`BongClient.register()` 接线；lang `mineral_sense` / `category.bong-client.mineral`（en/zh）。
- **测试**：`MineralSenseBootstrapTest`（4）——看向方块单发 / 空准星不发 / 多次按键多次单发 / sender seam 注入。

## P1 — 感保鲜（freshness_probe）收窄 + 静默

- **现状（🔴 jarring）**：`InspectScreen.java` —— 历史上 `if (item != null && hasShiftDown()) sendFreshnessProbe(item.instanceId())`，**唯一过滤 `item != null`**。凝脉以下每次 Shift+右键任意背包物品都弹 `server/src/network/freshness_probe_emit.rs` 的 `RealmTooLow`→「神识未及，凝脉方可感知保鲜」；非保鲜物再弹「此物无时气流转」。
- **目标 / 落地**：① 触发从「任意物品 Shift+右键」迁出鼠标路径，改为焦点槽位 **Shift+F**（`InspectScreen.keyPressed` 内 `GLFW_KEY_F + hasShiftDown()`）；② 发包前先判物品有 freshness 数据——仅当 `FreshnessStore.get(instance_id) != null`（server 曾主动推过 `freshness_update`）才发探针，凡物不发；③ server `freshness_probe_emit.rs` 对 `NoFreshness` **一律静默**（不再 emit EventAlert），兜住误触/竞态时的灰字刷屏。
- **测试**：`InspectScreenFreshnessProbeTest`（4）——有 freshness 记录才发 / 无记录不发 / id 不匹配不发 / null 不发；server `denied_no_freshness_is_fully_silent_no_packet`（锁完全静默，不发任何 S2C）+ `denied_realm_too_low_sends_event_alert_not_freshness_update`（其余 denial 分支仍发 EventAlert，不回归）。

## P2 — 感灵龛（spirit_niche_gaze）被动改主动

- **现状（🟡 borderline）**：`SpiritNicheRevealBootstrap.java` —— `ClientTickEvents.END_CLIENT_TICK` 每 tick 轮询准星方块，凝视任意方块 ≥60 tick（~3s）自动发 `spirit_niche_gaze`（同坐标去重）。无境界/物品/键位过滤。server 命中他人灵龛才 reveal（并推「灵龛再无庇佑」给灵龛主），无灵龛静默丢弃。**被动揭示/摧毁他人灵龛对凝视者完全不透明**。
- **目标 / 落地**：删除被动凝视轮询（`observeBlock` / `focusedTicks` / `lastGazeSentPos` 等全删），统一为玩家看向方块按 **M 键**（已注册 `key.bong-client.spirit_niche_mark_coordinate` 专属键位，默认 `GLFW_KEY_M`）时一次性**同时发** `spirit_niche_gaze` + `spirit_niche_mark_coordinate`，空准星 no-op。**在 server gaze 路径加任何 narration 之前先收窄触发**，避免「此处无灵龛」升 jarring。
- **测试**：`SpiritNicheRevealBootstrapTest`（3）——M 键成对发 gaze+mark / 空准星不发 / 多次成对。

## P3 — 截脉窗口守卫 + 键位冲突

- **现状（🟡 borderline）**：`CombatHudBootstrap.java` —— `onJiemaiPressed(){ DefenseWindowStore.open(...); sendJiemai(); }` **缺 `DefenseWindowStore.snapshot().active()` 守卫**（注释 §7 称应仅窗口期响应）。任意时刻按 V 发无用 `jiemai` C2S + 本地假激活 HUD 截脉环。且 `CombatKeybindings.java` 截脉默认键 `GLFW_KEY_V` 与 `MovementKeybindings` 冲刺默认 V **冲突**（单按 V 两个 `wasPressed` 都触发）。server 有醒灵境门控但 Rejected 仅 debug 日志（问题在 HUD 幻像 + 无用洪流 + 键冲突）。
- **目标 / 落地**：① `onJiemaiPressed` 加 `if (!DefenseWindowStore.snapshot().active()) return;`——窗口未开时按截脉键既不发 jiemai C2S 也不点亮本地截脉环（消除 HUD 幻像 + 无用 C2S 洪流；方法改包级可见以便单测直接驱动）；② 解 V 键冲突——`CombatKeybindings` 截脉键默认从 `GLFW_KEY_V` 改为 `GLFW_KEY_UNKNOWN`（截脉是严格 server 窗口期反应技，改由玩家显式绑定，避免与冲刺撞默认键）。
- **测试**：`CombatHudBootstrapTest`（5）——窗口未开/过期/连按 → 零发包零开环；窗口开启 → 恰发一条 jiemai 包且环维持 active；`JiemaiKeyConflictTest`（3）——源码扫描锁「全仓仅冲刺默认 V」不变量（同 `NoDuplicateDefaultGKeybindingTest` 范式）。

## 审计来源

本 plan 的 P1-P3 来自 worldgen-v4 P6 审阅期的「突兀交互绑定」审计 workflow（5 路 sonnet 扫 interactBlock/interactEntity/attackEntity/键鼠/门控 narration 反查 → opus 汇总）。已排除 ~20 个合理绑定（特定持有物 / 特定实体类型 / 显式键位 / 纯渲染）。判据：**宽泛输入（空手/任意方块/任意实体/任何右键左键/被动凝视）自动发 C2S + 可能弹境界·经脉·神识门控 narration + 抢交互 + 无显式入口 = 需改造**。

## §N 开放问题（实现期收口，2026-06-15）

> 立项时列的开放问题已在实施时全部收口；原表保留以备追溯，**实际落地以本节决议为准**。

1. **神识是否作为资源引入** → **不引入**。本 plan 只改触发入口，未引入神识值 / 消耗 / 恢复，**不扩 qi_physics、不加任何物理常数**。感知类是只读探针，不走真元守恒账本。
2. **三个独立技能 vs 一个多目标分支** → **都不做成技能**。见 P0「Route B 决策」：`SkillFn` 只携带 `Option<Entity>` 无 `BlockPos`，感知类需要方块坐标，故统一走客户端专属键位主动触发，不进 `SkillRegistry`。
3. **各感知的境界/神识解锁门槛** → 沿用 server 既有 resolver 门控（如 `MineralProbeDenialReason::RealmTooLow`），本 plan 不新增门槛，只把「每次右键刷屏」收窄成「按键时一次性 denial 回执」。
4. **P1-P3 随本 plan 一起做 vs 拆后续** → **一起做**。P0/P1/P2 在 Stage1（`5557cb96b`）一并落地，P3 在 Stage2（`e1e70f126`）落地，单 plan 两次 commit 收口。

## Finish Evidence

### 落地清单

**P0 — 感矿脉主动化（Route B 客户端键位）**

- 新增 `client/src/main/java/com/bong/client/mineral/MineralSenseBootstrap.java` —— 专属键位 `key.bong-client.mineral_sense`（默认 `GLFW_KEY_N`），`ClientTickEvents.END_CLIENT_TICK` 内 `while (senseKey.wasPressed())` 边沿单发 `ClientRequestSender.sendMineralProbe(x,y,z)`，空准星 no-op；`probeSender` seam 供测试注入。类 javadoc 显式记录「不注册 server SkillRegistry（故 `SkillMeridianDependencies::declare` 红线不适用）」。
- `client/.../BongClient.java` 接线 `MineralSenseBootstrap.register()`。
- lang `assets/bong-client/lang/{en_us,zh_cn}.json` 新增 `category.bong-client.mineral` + `key.bong-client.mineral_sense`。
- **Route B（非技能化）技术依据**：`server/src/cultivation/skill_registry.rs:37` `type SkillFn = fn(&mut World, caster: Entity, slot: u8, target: Option<Entity>) -> CastResult;` —— 无 `BlockPos`，无法承载感矿脉所需的准星方块坐标；既有 `mineral_probe(x,y,z)` 链路天然带坐标，故复用之，不进 skill 系统。

**P1 — 感保鲜收窄 + 静默**

- `client/.../inventory/InspectScreen.java` —— 触发从鼠标 Shift+右键迁至 `keyPressed` 内焦点槽位 **Shift+F**（`GLFW_KEY_F` + `hasShiftDown()` + 非拖拽态）；发包前 `FreshnessStore.get(Long.toString(item.instanceId())) == null` 判空，凡物不发（仅对 server 曾推过 `freshness_update` 的物品发探针）。
- `server/src/network/freshness_probe_emit.rs` —— `ProbeDenialReason::NoFreshness` 一律静默丢弃（不发任何 S2C、不 emit EventAlert）；其余 denial（`RealmTooLow` 等）行为不变。

**P2 — 感灵龛被动改主动**

- `client/.../social/SpiritNicheRevealBootstrap.java` —— 删被动凝视 3 秒自动 gaze 轮询（`observeBlock` / `focusedTicks` / `lastGazeSentPos` 全删），统一为看向方块按 **M 键**（`key.bong-client.spirit_niche_mark_coordinate`，默认 `GLFW_KEY_M`）一次性同时发 `sendSpiritNicheGaze` + `spirit_niche_mark_coordinate`，空准星 no-op。

**P3 — 截脉窗口守卫 + V 键冲突解**

- `client/.../combat/CombatHudBootstrap.java` —— `onJiemaiPressed` 首行加 `if (!DefenseWindowStore.snapshot().active()) return;`（窗口未开既不发 `sendJiemai` 也不点亮本地截脉环）；方法改包级可见供单测驱动。
- `client/.../combat/CombatKeybindings.java` —— 截脉键默认从 `GLFW_KEY_V` 改为 `GLFW_KEY_UNKNOWN`（解与 `MovementKeybindings` 冲刺默认 V 的冲突，截脉改由玩家显式绑定）。

### 关键 commit

- `9893197bf`（2026-06-15）— chore(plan)：plan-interaction-intent-cleanup-v1 骨架→active git mv（零内容 rename）
- `5557cb96b`（2026-06-15）— Stage1：三感知主动化收窄（P0/P1/P2）—— 新增 MineralSenseBootstrap + InspectScreen Shift+F + SpiritNiche M 键，server freshness NoFreshness 静默，10 文件 / +507 -87
- `e1e70f126`（2026-06-15）— Stage2：截脉窗口守卫 + V 键冲突解（P3）—— CombatHudBootstrap active() 守卫 + CombatKeybindings V→UNKNOWN，4 文件 / +195 -4

### 测试结果

- **client**：`cd client && ./gradlew test build` BUILD SUCCESSFUL —— 5 个相关测试类全绿：`MineralSenseBootstrapTest`(4) / `InspectScreenFreshnessProbeTest`(4) / `SpiritNicheRevealBootstrapTest`(3) / `CombatHudBootstrapTest`(5) / `JiemaiKeyConflictTest`(3) = 19 个新增/改动测试方法。
- **server**：`cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` 全绿（0 failed）；`freshness_probe_emit` 模块 10 passed，含 `denied_no_freshness_is_fully_silent_no_packet`（锁 NoFreshness 完全静默不发包）+ `denied_realm_too_low_sends_event_alert_not_freshness_update`（其余 denial 分支不回归）。Stage1 commit 记录 server 全量 9197 passed。

### 跨仓库核验

- **client**：`MineralSenseBootstrap`（key N，主动 probe）/ `InspectScreen`（Shift+F + freshness 判空）/ `SpiritNicheRevealBootstrap`（M 键 gaze+mark，删被动轮询）/ `CombatHudBootstrap`（`DefenseWindowStore.snapshot().active()` 守卫）/ `CombatKeybindings`（jiemai 默认 `GLFW_KEY_UNKNOWN`）—— 全部命中、编译 + 测试绿。
- **server**：`freshness_probe_emit.rs`（`ProbeDenialReason::NoFreshness` 静默）唯一改动文件；`mineral_probe` / `spirit_niche_gaze` / `jiemai` resolver 与 S2C 回执**零改动**（本 plan 只改客户端触发入口）。`skill_registry.rs` 仅作 Route B 决策的只读依据，未改动。
- **agent**：无命中 symbol（纯客户端触发层 + server 既有 resolver，天道 agent 不参与交互意图）。

### 遗留 / 后续

- 感知类若未来要吃神识消耗 / 经脉依赖 / 熟练度成长，应在 `mineral_probe` / `freshness_probe` / `spirit_niche_gaze` 各自的 server resolver 链路上单独挂规则，**不要**把它们塞进 `SkillFn`（坐标承载受限，见 P0 Route B 决策）。
- 截脉键默认 `GLFW_KEY_UNKNOWN`（未绑定）—— 玩家需在原版控制里手动绑定才能触发截脉反应技；这是有意取舍（消除与冲刺的默认键冲突），如后续要给一个不冲突的默认键，调 `CombatKeybindings` 单处即可，`JiemaiKeyConflictTest` 会守住「不得与冲刺撞默认 V」不变量。
