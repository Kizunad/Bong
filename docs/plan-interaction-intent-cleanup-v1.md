# plan-interaction-intent-cleanup-v1（骨架）

> **骨架（skeleton / 草案）**。一句话主题：把「宽泛输入自动触发 C2S」的交互绑定改为**显式意图**——神识感知类（感矿脉 / 感保鲜 / 感灵龛）做成**主动功法技能 / 专属键位 + 目标过滤**，战斗反应类（截脉）补**窗口守卫**——消除「每次右键/凝视/按键都可能弹境界门控 narration」的出戏噪声。
>
> 立项动机：worldgen-v4 P6 真机审阅时发现，空手右键任意方块即触发感矿脉、凝脉以下每次弹「凝脉方可感矿脉。神识未及」，既出戏又跟放方块抢交互。已先行**摘除感矿脉右键绑定**（`a6aa0004b`，删 `MixinClientPlayerInteractionManagerMineralProbe`，保留底层 `sendMineralProbe` 能力）。随后一个审计 workflow 扫出 3 个同类绑定（见下），本 plan 统一收口。

## 阶段总览

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 神识感知技能框架 + 感矿脉技能化（接 skill 系统 + 专属键位/技能栏 + 目标过滤） | ⬜ |
| P1 | 感保鲜（freshness_probe）收窄触发 + NoFreshness 静默 | ⬜ |
| P2 | 感灵龛（spirit_niche_gaze）被动凝视改主动 M 键 + 揭示反馈 | ⬜ |
| P3 | 截脉 V 键窗口守卫 + 键位冲突解决 | ⬜ |

## 接入面 checklist（防孤岛——升 active 前据 docs/CLAUDE.md §二 核实）

- **进料**：复用 `ClientRequestSender.sendMineralProbe / sendFreshnessProbe / sendSpiritNicheGaze / sendJiemai`（底层 C2S 能力均已存在，本 plan 只改**触发入口**，不动协议/server 处理）；技能化走 `cultivation::skill` / `SkillRegistry` + 客户端技能栏（`SkillBarStore` / F 键 / `MixinKeyboardSkillKeys`）。
- **出料**：server 侧 `mineral_probe` / `freshness_probe` / `spirit_niche_gaze` / `jiemai` 处理器与 S2C 回执**完全不变**；只改"由什么客户端动作发包"。
- **共享类型 / event**：复用既有 `MineralProbeResult` / freshness EventAlert / `DefenseWindowStore` / `SkillBar*`；不新增协议。
- **跨仓库契约**：纯客户端触发层改造为主；P0 技能化若需 server 注册技能则动 `cultivation::skill`（**必走 `SkillMeridianDependencies::declare`**，见红旗清单）。
- **worldview 锚点**：神识感知（待核 `worldview.md` 神识/通灵章节——感矿脉/感保鲜/感灵龛属"以神识外放感知环境"，应是**主动消耗神识的功法**而非被动/通配触发）。
- **qi_physics 锚点**：本 plan **不涉及真元流动/守恒**（感知类只读传输，截脉是 HUD 窗口逻辑）；若技能化引入神识消耗，神识是否走 qi_physics 待 P0 决议（默认不引入新物理常数）。

## P0 — 神识感知技能框架 + 感矿脉技能化

- **现状**：感矿脉右键绑定已摘（`a6aa0004b`）；`ClientRequestSender.sendMineralProbe(x,y,z)` + server `MineralProbeDenialReason`（含 `RealmTooLow`→「凝脉方可感矿脉。神识未及」）+ `MineralProbeResultHandler` 均在位，**无触发入口**。
- **目标**：把感矿脉做成**主动功法技能**——
  - 客户端：技能栏可装备「感矿脉」技能，按键（F 快捷使用栏 / 专属键位）cast → 对准星方块发 `mineral_probe`；**仅在玩家主动 cast 时发**，不再通配右键。
  - 境界不足的反馈从「每次右键弹 toast」改为「装备/cast 时一次性提示需凝脉境」（技能解锁门控，HUD 条件显示——未达境界技能不常驻，见 memory feedback_hud_conditional）。
  - 抽出可复用的「神识感知技能」基类/意图（感保鲜、感灵龛共用），避免每个感知各写一套触发。
- **可核验交付物**：`cultivation::skill`（或客户端技能定义）新增 mineral_sense 技能 id；技能→`sendMineralProbe` 接线；`SkillMeridianDependencies::declare` 声明依赖经脉；技能未解锁不显示（HUD 条件）。
- **待决**：神识感知是否消耗资源（神识值？）；技能解锁门槛（凝脉境 + 神识阈值）落点。

## P1 — 感保鲜（freshness_probe）收窄 + 静默

- **现状（🔴 jarring）**：`client/.../inventory/InspectScreen.java:1799` —— `if (item != null && hasShiftDown()) sendFreshnessProbe(item.instanceId())`，**唯一过滤 `item != null`**。凝脉以下每次 Shift+右键任意背包物品都弹 `server/src/network/freshness_probe_emit.rs:92` 的 `RealmTooLow`→「神识未及，凝脉方可感知保鲜」；非保鲜物再弹「此物无时气流转」。
- **目标**：① 客户端发包前先判物品有 freshness 数据（仅对可保鲜物发）；② 触发改为焦点槽位专属键位（如 Shift+F）而非通配 Shift+右键；③ server `NoFreshness` 路径**静默**（不弹 EventAlert）；④（可选）一并纳入 P0 神识感知技能框架。
- **可核验交付物**：InspectScreen 发包前 freshness 字段判空；`freshness_probe_emit.rs` NoFreshness 不 emit EventAlert；测试覆盖「无保鲜物不发包」「非凝脉不刷屏」。

## P2 — 感灵龛（spirit_niche_gaze）被动改主动

- **现状（🟡 borderline）**：`client/.../social/SpiritNicheRevealBootstrap.java:52` —— `ClientTickEvents.END_CLIENT_TICK` 每 tick 轮询准星方块，凝视任意方块 ≥60 tick（~3s）自动发 `spirit_niche_gaze`（同坐标去重）。无境界/物品/键位过滤。server `social/mod.rs:1872` 命中他人灵龛才 reveal（并推「灵龛再无庇佑」给灵龛主），无灵龛静默丢弃。**被动揭示/摧毁他人灵龛对凝视者完全不透明**。
- **目标**：去掉被动凝视轮询，改为玩家主动按 **M 键**（已注册 `spirit_niche_mark_coordinate` 专属键位）时一并发 gaze；若保留被动则准星旁加凝视进度细微 HUD。**务必在 server gaze 路径加任何 narration 之前先收窄触发**（否则一加「此处无灵龛」立即升 jarring）。
- **可核验交付物**：删除/门控被动凝视分支；M 键触发 gaze；揭示结果有可感知反馈。

## P3 — 截脉 V 键窗口守卫 + 键位冲突

- **现状（🟡 borderline）**：`client/.../combat/CombatHudBootstrap.java:52` —— `onJiemaiPressed(){ DefenseWindowStore.open(...); sendJiemai(); }` **缺 `DefenseWindowStore.snapshot().active()` 守卫**（注释 §7 称应仅窗口期响应）。任意时刻按 V 发无用 `jiemai` C2S + 本地假激活 HUD 截脉环。且 `CombatKeybindings.java:58` 默认键 `GLFW_KEY_V` 与 `MovementKeybindings` 冲刺默认 V **冲突**。server 有醒灵境门控但 Rejected 仅 debug 日志（无 narration spam，问题在 HUD 幻像 + 无用洪流 + 键冲突）。
- **目标**：`onJiemaiPressed` 加 `if (!DefenseWindowStore.snapshot().active()) return;`；解决 V 键冲突（改其一默认键 / 路由层互斥）。
- **可核验交付物**：守卫断言「窗口未开时按 V 不发包不开环」；键位冲突测试/默认值调整。

## 审计来源

本 plan 的 P1-P3 来自 worldgen-v4 P6 审阅期的「突兀交互绑定」审计 workflow（5 路 sonnet 扫 interactBlock/interactEntity/attackEntity/键鼠/门控 narration 反查 → opus 汇总）。已排除 ~20 个合理绑定（特定持有物 / 特定实体类型 / 显式键位 / 纯渲染）。判据：**宽泛输入（空手/任意方块/任意实体/任何右键左键/被动凝视）自动发 C2S + 可能弹境界·经脉·神识门控 narration + 抢交互 + 无显式入口 = 需改造**。

## §N 开放问题（升 active 前收口）

1. 神识是否作为**资源**引入（消耗/恢复）——若是则定义在哪、是否需扩 qi_physics（默认不引入）。
2. 感矿脉/感保鲜/感灵龛是**三个独立技能**还是一个「神识外感」技能的多目标分支。
3. 各感知的境界/神识解锁门槛（正典锚点待核 worldview 神识章节）。
4. P1-P3 是否随本 plan 一起做，还是 P0（感矿脉技能化）先行、P1-P3 拆后续 PR。
