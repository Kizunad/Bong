# Bong · plan-spawn-tutorial-v1

把 journey §L 30min 钩子表的环境线索注入 spawn_plain worldgen profile。**完全不显式提示**（journey O.13 已正典）——靠 POI 布置 + NPC 行为 + 状态机钩子驱动引导玩家完成"开棺取龛石 → 看灵气 → 打坐 → 打通第一条经脉 → 突破引气"。

**Primary Axis**：**沉默引导通过率**（30min 内玩家无 UI 提示完成醒灵→引气突破的概率）

## 阶段总览

| 阶段 | 状态 | 验收 |
|---|---|---|
| **P0** 出生点 POI（半埋石棺 + 棺中龛石 + 灰白残灰）+ 友善散修 NPC（可杀）+ 5 句 narration 风格基准 | ⬜ | — |
| **P1** 教学灵泉动态选址（worldgen 高灵气点 ×2）+ 开脉丹宝箱 + 噬元鼠动态刷出 + 状态机钩子触发链 | ⬜ | — |
| P2 v1 收口（饱和 testing + agent narration 对齐 + LifeRecord） | ⬜ | — |

> **vN+1 (plan-spawn-tutorial-v2)**：多种石棺纹饰文化差异 / 多 NPC 个性化对话 / 多种鼠 + 巢穴 / 分支教学路径 / 玩家自定时序 / 可选教程提示（accessibility）

---

## 世界观 / library / 交叉引用

**worldview 锚点**：
- §十三 初醒原（灵气 0.3 + 北边馈赠区 200-500 格 0.5+）
- §三 醒灵 → 引气期物理（line 100-110：第一条经脉打通 + qi_max 10 → 40）
- §八 天道行为准则（O.13 沉默引导原则的世界观基础——天道冷漠不主动教学）

**library 锚点**：
- `world-0002 末法纪略`（残土风貌锚定）
- `world-0001 天道口述残编`（narration 风格基准——半文言半白话 + 冷漠古意 + 禁现代腔）

**交叉引用**：
- `plan-gameplay-journey-v1` 🟡（still skeleton，2026-05-03 P2 状态更新）— **§L 30min 钩子表 + §P0 旅程** 是本 plan 的源头
- `plan-worldgen-v3.1` ✅（已落地）— `worldgen/scripts/terrain_gen/profiles/spawn_plain.py` 已 ✅，本 plan 扩展 POI 注入 + 高灵气点选址
- `plan-fauna-v1` ⬜（still skeleton，P2 必需）— **噬元鼠**实体未立；P0/P1 暂用 zombie 占位 + 改 AI"扣真元不扣 HP"
- `plan-narrative-v1` ⏳（部分实装）— 5 句 narration 风格基准台词由本 plan 触发，写入 narrative-v1 风格库
- `plan-cultivation-v1` ✅（已落地）— `Cultivation` / `MeridianSystem` / `BreakthroughRequest` 全 ✅，无新增
- `plan-poi-novice-v1` ⬜（still skeleton，下一个升 active 候选）— P1 引气期 spawn ± 1500 格新手 POI；本 plan 是其前置
- `plan-perception-v1.1` ✅（已落地）— 灵气条变色 / 神识感知（高灵气点提示）走已有 SenseEntry 路径

## 接入面 checklist（防孤岛 — 严格按 docs/CLAUDE.md §二）

- **进料**：`worldgen/scripts/terrain_gen/profiles/spawn_plain.py` ✅ + journey §L 30min 钩子表 + journey §L 5 句 narration 风格基准（line 749-754）
- **出料**：worldgen 生成的 spawn_plain instance 包含：半埋石棺（出生点）+ 棺中龛石（一次性）+ 灰白残灰地面 + 教学小灵泉 ×2（动态选址）+ 开脉丹宝箱 + 友善散修 NPC（可杀）+ 噬元鼠动态刷出路径
- **共享类型 / event**：复用 worldgen blueprint POI 体系；新增 `world::spawn_tutorial::TutorialState` resource（per-spawn instance 状态机）/ `tutorial_hook_state_machine` system / `dynamic_lingquan_selector`（worldgen helper）/ `dynamic_rat_swarm_spawner` system
- **跨仓库契约**：
  - server: `world::spawn_tutorial::TutorialState` resource / `tutorial_hook_state_machine` system / `dynamic_rat_swarm_spawner` system / `inventory::initial_grant_kanshicoffin`（一次性龛石授予改走"开棺"动作）/ Coffin BlockEntity 扩展（plan-worldgen 已有自定义 BlockEntity 框架）
  - worldgen: `worldgen/scripts/terrain_gen/profiles/spawn_plain.py` 扩展 — POI 注入 + 高灵气点扫描器（输出 2 处坐标到 raster export）
  - schema: `agent/packages/schema/src/spawn_tutorial.ts` → `TutorialHookEventV1`（5 句 narration 风格基准对齐 narrative-v1）
  - client: 复用现有 inventory pickup 路径；新增 `bong:world/coffin_open`（inbound, 新；玩家右键石棺触发开棺）
- **沉默引导原则严格遵守**（journey O.13）：v1 严格无 UI / 无弹窗 / 无任务面板 / 无 progress bar；所有教学**仅靠 POI 布置 + NPC 行为 + tiandao narration**

---

## §A 概览（设计导航）

> 30min 沉默引导：玩家从棺旁醒来 → 推开棺盖取龛石 → tiandao 第一句 narration → 移动看灵气变色 → 打坐 → 打通第一经脉 → 遭遇噬元鼠群（扣真元）→ 走到灵泉准备突破 → 醒灵→引气世界变色。**全程无 UI 教学**——靠环境布置 + NPC 行为 + 5 句 narration 风格基准。

### A.0 v1 实装范围（2026-05-03 拍板）

| 维度 | v1 实装 | 搁置 vN+1 |
|---|---|---|
| 出生点 POI | **半埋石棺 + 棺中龛石**（Q96: C 推开棺盖取龛石 = 教学"开容器"动作）| 多种石棺纹饰 / 文化差异 |
| 友善散修 NPC | **可杀**（Q97: B；杀了失去引导自负，玩家自找路径）| 不死 buff / 死亡惩罚机制 |
| 玩家偏离教学 | **天道不再 nudge**（Q98: A；玩家自负，自行探索）| 动态 POI 重定向 / nudge narration |
| 多人联机 | **同 spawn 共享教学**（Q99: D；后来玩家共享 POI 但无独立钩子）| 每人独立 instance / 跳教学选项 |
| 教学灵泉坐标 | **完全动态**（Q100: C；worldgen 生成时按高灵气点挑选 2 处，每 spawn_plain instance 不同）| 半动态 / 固定坐标 |
| 噬元鼠群路径 | **动态刷出**（Q101: B；玩家走向灵泉时实时生成鼠群在前方 20 格）| 静态 path / 巢穴系统 |
| 30min 钩子触发 | **状态机触发**（Q102: B；玩家行为驱动，不是时间硬触发）| 时间硬触发 / 混合 |
| narration 风格 | **5 句基准台词**（journey §L line 749-754）| 动态 / 玩家个性化 |
| 沉默引导（O.13）| **严格无 UI**（无弹窗 / 任务面板 / progress bar）| 可选教程提示 |

### A.1 30min 期望路径（状态机驱动，时间仅参考）

journey §L 30min 钩子表（line 736-746）改写为状态机版（Q102: B）：

| 状态触发器 | 期望时刻（参考）| 玩家应感知 | 后续触发 |
|---|---|---|---|
| `SpawnEntered` | 0:00 | "这是哪里？" | 初始化 TutorialState; tiandao 第一句 narration |
| `CoffinOpened` | 0:00-1:00 | "这棺里有东西" | 龛石授予 + 灵龛 narration |
| `Moved200Blocks` | 5:00 | 灵气条灰→淡绿（"灵气是真实存在的环境量"）| client 灵气条 ✅ |
| `FirstSitMeditate` | 10:00 | 真元缓涨（"要等的、不能一直跑"）| Cultivation tick ✅ |
| `FirstMeridianOpened` | 15:00 | 经脉图首次出现（"我能选打哪条"）| MeridianSystem ✅ |
| `RatSwarmEncounter` | 20:00 | 噬元鼠扣真元（"卧槽它在吃我的蓝"）| 动态刷出（玩家前方 20 格）|
| `LingquanReached` | 25:00 | 找到灵气 0.5+ 点（"这里安全吗"）| 灵泉坐标 = 动态选址结果 |
| `BreakthroughWindow` | 27:00-30:00 | 3 分钟突破窗口（脆弱期）| BreakthroughRequest ✅ |
| `RealmAdvancedToInduce` | 30:00 | 醒灵 → 引气，世界变色（"我能看见灵气了"）| realm-vision-induce ✅ |

**关键**：状态触发器不依赖时间，玩家**任何时刻**达到触发条件即推进。时间列仅做"期望"参考用于 narrative-v1 节奏调优。

### A.2 沉默引导原则严格遵守（journey O.13）

v1 **不实装**：
- ❌ 教程弹窗 / 任务面板
- ❌ Progress bar / step indicator
- ❌ "下一步：移动 200 格" / "下一步：打坐" 等显式提示
- ❌ 高亮 POI（灵泉发光圈 / NPC 头顶感叹号）
- ❌ Quest log / mission tracker
- ❌ 玩家成就推送（"你打通了第一条经脉！"）

v1 **实装**：
- ✅ 环境布置（POI / 灵气浓度 / 地形色温）
- ✅ NPC 行为（友善散修偶尔走向馈赠区方向）
- ✅ tiandao narration（5 句风格基准 + journey §L 节奏）
- ✅ 灵气条变色（client 已有 HUD 沉浸式极简）
- ✅ 经脉图自然首次显现（FirstMeridianOpened 触发）

### A.3 v1 实施阶梯

```
P0  出生点 POI + 散修 + 第一句 narration
       半埋石棺 BlockEntity（玩家右键 → CoffinOpened state → 龛石授予）
       灰白残灰地面（spawn_plain 已 ✅，仅微调色温）
       友善散修 NPC（Rogue archetype ✅，可杀）
       tiandao 第一句 narration（5 句基准之首：新角色/重生分支）
       TutorialState resource 初始化
       ↓
P1  教学灵泉（动态选址）+ 开脉丹 + 噬元鼠动态刷 + 状态机钩子链
       worldgen dynamic_lingquan_selector（spawn_plain 生成时扫描灵气 0.5+ 点 ×2）
       开脉丹宝箱（灵泉 #1 旁 5 格）
       dynamic_rat_swarm_spawner（玩家走向灵泉路径 detection → 实时刷出）
       tutorial_hook_state_machine（9 个状态触发器 + 对应 narration）
       灵气条变色（已有 ✅，确保 spawn_plain 阈值正确）
       ↓ 饱和 testing
P2  v1 收口
       agent 5 句 narration 写入 narrative-v1 风格基准库
       LifeRecord "X 在 Y 时刻完成醒灵→引气"事件
       30min 通关率 telemetry（启动后玩家 30min 内完成突破比例）
```

### A.4 v1 已知偏离正典（vN+1 必须修复）

- [ ] **plan-fauna-v1 噬元鼠未立**（journey §L line 743 锚定）—— v1 用 zombie 占位 + 改 AI 扣真元；vN+1 plan-fauna-v1 active 后切真实噬元鼠
- [ ] **plan-narrative-v1 完整对齐**（5 句台词 + 风格基准）—— v1 仅写入 5 句基准，narrative-v1 完整 narrative 节奏 vN+1 落地
- [ ] **多人联机 D 选项的边界 case**：第一个玩家完成教学后第二个玩家加入，POI 已被消耗（龛石已取 / 开脉丹已拿）—— v1 接受现状（共享但 POI 消费后第二人没法再取）；vN+1 引入"教学 instance reset"机制
- [ ] **教学完成后 POI 是否清理**：龛石棺 / 灵泉 / 散修 NPC 是否在玩家突破引气后 despawn？v1 简化为永久保留（vN+1 引入 cleanup 机制）

### A.5 v1 关键开放问题

**已闭合**（Q96-Q102，7 个决策）：
- Q96 → C 棺中龛石（推开棺盖取，教学"开容器"动作）
- Q97 → B 散修可杀（自负，失引导）
- Q98 → A 天道不再 nudge（玩家偏离自负）
- Q99 → D 同 spawn 共享教学（多人共享 POI 无独立钩子）
- Q100 → C 完全动态灵泉选址（worldgen 高灵气点扫描）
- Q101 → B 噬元鼠动态刷出（玩家前方 20 格）
- Q102 → B 状态机触发（玩家行为驱动）

**仍 open**（v1 实施时拍板）：
- [ ] **Q103. 龛石授予方式**：开棺动作触发 `inventory::initial_grant_kanshi(player)`？还是棺中物理放置一颗龛石 item，玩家"拾取"？建议**物理放置**（玩家走完整 pickup 流程，符合"无 UI 教学"原则）—— P0 实装时拟
- [ ] **Q104. 噬元鼠群刷出条件**：玩家移动方向 + 灵泉距离 < 50 格触发？还是直接经脉首通后触发？建议**经脉首通后第一次走向灵泉时触发**（与 RatSwarmEncounter 状态对齐）—— P1 实装时拟
- [ ] **Q105. 状态机持久化**：TutorialState 是否随玩家 save？玩家中途下线再上线状态保留吗？建议**保留**（per-player TutorialState 存 PlayerUiPrefs / dedicated component）—— P0 实装时拟
- [ ] **Q106. 多人联机 D 选项的 POI 消耗**：龛石只有 1 颗，第二人加入还能取吗？建议**v1 简化为每个玩家独立的"开棺权限"**（同棺被不同玩家分别开 → 各自得 1 颗龛石；POI 视觉上保持有棺）—— P0 实装时拟
- [ ] **Q107. 高灵气点扫描范围**：spawn ± 多少格内找？skeleton 给的 (50,65,100) 距离 spawn ~111 格 / (-30,64,-80) ~85 格。建议**扫描半径 200 格**，挑灵气 0.5+ 的 2 处—— P1 实装时拟



## 接入面 Checklist

- **进料**：`worldgen/scripts/terrain_gen/profiles/spawn_plain.py` ✅ + 30min 钩子表(§L)
- **出料**：注入后的 spawn_plain 包含: 半埋石棺 + 一次性龛石 + 教学小灵泉 ×2 + 友善散修 + 开脉丹宝箱 + 噬元鼠群路径
- **共享类型**：复用 worldgen blueprint POI 体系 + `inventory::initial_grant`
- **跨仓库契约**：worldgen export → server runtime POI 加载 → tiandao narration 分支
- **worldview 锚点**：§十三 初醒原(灵气浓度 0.3 + 北边 200-500 格 0.5+ 馈赠区)

---

## §0 设计轴心

- [ ] **沉默引导**(O.13)：所有教学不通过 UI/弹窗/任务面板,只靠环境布置
- [ ] **路径暗示**：灵气条变色 + 散修走向馈赠区 + tiandao 偶发台词("风从东北来,那里的空气稍微厚一点") → 玩家自己悟
- [ ] **可被打破**：玩家完全不按教学走也能玩——只是失去 30min 内的最佳路径

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | 半埋石棺出生点 + 一次性龛石 + 灰白残灰地面 | 玩家从棺旁醒来,5 格内可拾龛石 |
| **P1** ⬜ | 教学小灵泉 ×2(灵气 0.5+,半径 5 格) + 开脉丹小宝箱 | 灵泉肉眼可见(色温更暖 + 草叶绿色) |
| **P2** ⬜ | 友善散修 NPC(`Rogue` archetype ✅) + tiandao narration 分支 | 1 名散修在 spawn 200 格内,会主动 narration |
| **P3** ⬜ | 噬元鼠群路径(玩家走向灵泉时 80% 遭遇) | 鼠扣真元不扣 HP,死亡的玩家在灵龛重生 |

---

## §2 spawn_plain POI 注入清单

| POI | 位置(相对 spawn) | 内容 |
|---|---|---|
| 半埋石棺 | (0, 64, 0) | 自定义 BlockEntity,出生点视觉锚 |
| 龛石 item | spawn 身边 | inventory::initial_grant 一次性 |
| 教学小灵泉 #1 | (50, 65, 100) 灵气 0.5,半径 5 | 草叶绿色 + 色温暖 |
| 教学小灵泉 #2 | (-30, 64, -80) 灵气 0.5,半径 5 | 备选路径 |
| 开脉丹小宝箱 | 灵泉 #1 旁 5 格内 | 1 颗开脉丹 |
| 友善散修 NPC | spawn 200 格内随机 | `Rogue` archetype + 教学 narration trigger |
| 噬元鼠群 ×2-3 | spawn ↔ 灵泉路径上 | 扣真元不扣 HP |

---

## §3 30min 钩子触发链(对齐 plan-gameplay-journey-v1 §L)

```
0:00  玩家在棺旁醒来
0:00  tiandao 第一句 narration("你又醒了..." 重生 / "你醒了..." 新角色)
5:00  玩家移动 200 格触发灵气条变色
10:00 玩家右键长按打坐(无 UI 提示,客户端 input 监听)
15:00 第一条正经打通(自动选最近邻接的)
20:00 噬元鼠群遭遇(扣真元) — 真元 < 30 触发逃跑视觉
25:00 玩家走到灵泉(灵气 0.5+) 准备突破
30:00 醒灵 → 引气突破成功 + 世界变色
```

---

## §4 数据契约

### v1 P0 落地清单

| 模块 | 文件路径 | 核心内容 |
|---|---|---|
| Coffin BlockEntity | `server/src/world/coffin.rs` (新) | 半埋石棺 BlockEntity 扩展 + 右键 Open 动作 + 一次性 inventory grant 龛石 item |
| TutorialState resource | `server/src/world/spawn_tutorial.rs` (新) | per-player `TutorialState { entered_at, hooks_triggered: HashSet<TutorialHook> }` resource，持久化（Q105 P0 拟）|
| 散修 NPC spawner | `server/src/npc/tutorial_rogue.rs` (新) | spawn_plain 200 格内随机生成 1 名 Rogue NPC（可杀，无 immunity buff，对应 Q97: B）|
| Initial grant 调整 | `server/src/inventory/initial_grant.rs` | 龛石**移除**初始 grant；改为 Coffin Open 动作触发授予（对齐 Q96: C）；凡铁刀仍走原 path（教学外的基础工具）|
| Worldgen POI 扩展 | `worldgen/scripts/terrain_gen/profiles/spawn_plain.py` | 半埋石棺 POI 注入（spawn 中心点）+ 灰白残灰地面色温微调 |
| Inbound packet | `client/.../net/CoffinPackets.java` (新) | `bong:world/coffin_open { coffin_pos }` |
| Schema | `agent/packages/schema/src/spawn_tutorial.ts` (新) | `TutorialHookEventV1` (state_machine 推进时 emit) / `CoffinOpenedV1` |
| Agent narration 第一句 | `agent/packages/tiandao/src/era-narration.ts` (扩展) | 新角色 vs 重生角色分支（journey §L line 749-750 锚定）|

### v1 P1 落地清单

| 模块 | 文件路径 | 核心内容 |
|---|---|---|
| Dynamic 灵泉选址 | `worldgen/scripts/terrain_gen/profiles/spawn_plain.py` | `dynamic_lingquan_selector(spawn_center, radius=200)` 扫描 spirit_qi >= 0.5 的 2 处坐标，输出到 raster export（Q107 半径 200 格起手）|
| 开脉丹宝箱 | `worldgen/scripts/terrain_gen/profiles/spawn_plain.py` | 灵泉 #1 旁 5 格内随机位置注入 chest with 1 颗 `bong:alchemy/kaimai_dan`（plan-alchemy-v1 已有配方） |
| 噬元鼠动态刷出 | `server/src/world/dynamic_rat_spawner.rs` (新) | `dynamic_rat_swarm_spawner` system：玩家移动方向 + 灵泉距离 detection → 实时刷 2-3 zombie（占位 fauna-v1）在前方 20 格（Q104: 经脉首通后第一次走向灵泉触发；Q101: B）|
| Zombie AI 改写 | `server/src/npc/zombie_tutorial_ai.rs` (新或扩展) | 教学期间的 zombie attack 改为扣 `Cultivation.qi_current` 而非 wound（journey §L line 743 锚定）|
| 状态机钩子 | `server/src/world/spawn_tutorial.rs` | `tutorial_hook_state_machine` system：监听玩家 movement / meditate / meridian_open / breakthrough events → 推进 `TutorialState.hooks_triggered` → emit narration trigger |
| Outbound packet | `bong:world/tutorial_state` (新) | client HUD 不显示，仅给 agent / LifeRecord 用 |
| 5 句 narration 写入 | `agent/packages/tiandao/src/narration-style-baseline.ts` (新或扩展 narrative-v1) | journey §L line 749-754 五句台词作为风格基准库 |

### v1 P2 落地清单

| 模块 | 文件路径 | 核心内容 |
|---|---|---|
| LifeRecord | `server/src/lore/life_record.rs` | "X 在 spawn 后 N 分钟完成醒灵→引气突破"事件 |
| 30min 通关率 telemetry | `server/src/observability/tutorial_telemetry.rs` (新或扩展) | server-side metric: spawn 后 30min 内完成 BreakthroughRequest 的玩家比例 |
| 单测 | `server/src/world/spawn_tutorial_tests.rs` | 状态机 9 个钩子触发顺序 / 多人共享教学（Q99: D 边界）/ Coffin 开棺多次（Q106 边界）/ 玩家偏离教学不再 nudge（Q98: A 边界）|

---

## §5 开放问题

### 已闭合（2026-05-03 拍板，7 个决策）

- [x] **Q96** → C 棺中龛石（推开棺盖取，教学"开容器"动作）
- [x] **Q97** → B 散修可杀（杀了失去引导自负）
- [x] **Q98** → A 天道不再 nudge（玩家偏离自负）
- [x] **Q99** → D 同 spawn 共享教学（多人共享 POI 无独立钩子）
- [x] **Q100** → C 完全动态灵泉选址（worldgen 高灵气点扫描）
- [x] **Q101** → B 噬元鼠动态刷出（玩家前方 20 格）
- [x] **Q102** → B 状态机触发（玩家行为驱动，不是时间硬触发）

### 仍 open（v1 实施时拍板）

- [ ] **Q103. 龛石授予方式**：建议**物理放置**（玩家走完整 pickup 流程，符合"无 UI 教学"原则）—— P0 拟
- [ ] **Q104. 噬元鼠群刷出条件**：建议**经脉首通后第一次走向灵泉时触发**（与 RatSwarmEncounter 状态对齐）—— P1 拟
- [ ] **Q105. 状态机持久化**：建议**保留**（per-player TutorialState 存 PlayerUiPrefs / dedicated component）—— P0 拟
- [ ] **Q106. 多人联机 D 选项的 POI 消耗**：建议**v1 简化为每个玩家独立的"开棺权限"**（同棺被不同玩家分别开 → 各自得 1 颗龛石）—— P0 拟
- [ ] **Q107. 高灵气点扫描范围**：建议**扫描半径 200 格**（spawn 中心 ± 200 格内挑 2 处灵气 0.5+）—— P1 拟

### vN+1 留待问题（plan-spawn-tutorial-v2 时拍）

- [ ] **多种石棺纹饰** / 文化差异
- [ ] **多 NPC 个性化对话**
- [ ] **plan-fauna-v1 真实噬元鼠**（v1 zombie 占位）
- [ ] **plan-narrative-v1 完整对齐**（v1 仅 5 句基准）
- [ ] **教学完成后 POI 清理**（v1 永久保留）
- [ ] **多人联机 instance reset**（v1 共享 POI 消费后第二人没法再取）
- [ ] **可选教程提示（accessibility）** —— v1 严格无 UI

## §6 进度日志

- 2026-05-01：骨架创建。plan-gameplay-journey-v1 §L 钩子表 + P0 决策派生。
- 2026-05-03：从 skeleton 升 active。§A 概览 + §4 v1 P0/P1/P2 数据契约落地（7 个决策点闭环 Q96-Q102，5 个 v1 实装时拍板 Q103-Q107）。primary axis = **沉默引导通过率**（30min 内玩家无 UI 提示完成醒灵→引气突破的概率，可量化 telemetry 验收）。**关键设计**：状态机钩子触发（Q102 B）取代时间硬触发，灵泉完全动态选址（Q100 C）取代固定坐标，噬元鼠动态刷出（Q101 B）取代静态 path。**沉默引导原则严格遵守**（journey O.13）：v1 严格无 UI / 无弹窗 / 无任务面板 / 无 progress bar / 无 quest log。多人联机 D 选项（共享 POI）— 后来玩家共享同 spawn_plain instance 但无独立 30min 钩子。下一个升 active 候选：plan-poi-novice-v1（P1 引气期 spawn ± 1500 格新手 POI）。

## Finish Evidence

### 落地清单

- P0 出生点 POI / 开棺链路：
  - `worldgen/scripts/terrain_gen/profiles/spawn_plain.py` 注入 `spawn_tutorial_coffin` / `tutorial_rogue_anchor` / `tutorial_chest` / `tutorial_rat_path` / `tutorial_lingquan` POI。
  - `server/src/world/terrain/structures.rs` 按 POI 生成半埋石棺视觉（残灰、石砖、中心 `CHISELED_STONE_BRICKS`）。
  - `client/src/main/java/com/bong/client/mixin/MixinClientPlayerInteractionManagerAlchemy.java` 右键出生点石棺发送 `coffin_open`。
  - `server/src/world/spawn_tutorial.rs` 处理 `CoffinOpenRequest`，每玩家一次授予 `spirit_niche_stone`，并从 POI 生成可杀 Rogue 与开脉丹宝箱。
  - `server/assets/inventory/loadouts/default.toml` 移除起手龛石，`server/loot_pools.json` 新增 `tutorial_kaimai_chest`。
- P1 动态灵泉 / 鼠群 / 状态机：
  - `dynamic_lingquan_selector` 在 spawn 半径 200 内选择 2 个高灵气点，`raster_export.py` 合并动态 POI。
  - `server/src/world/spawn_tutorial.rs` 落地 `TutorialState`、`TutorialHook`、`tutorial_hook_state_machine`、`dynamic_rat_swarm_spawner`、`tutorial_rat_qi_drain_tick`。
  - `server/assets/items/pills.toml` 新增 `kaimai_dan`。
- P2 收口：
  - `server/src/cultivation/life_record.rs` / `server/src/schema/cultivation.rs` / `server/src/persistence/mod.rs` 写入 `SpawnTutorialCompleted` 与 `tutorial_state` 持久化。
  - `agent/packages/schema/src/spawn-tutorial.ts`、`agent/packages/schema/src/client-request.ts`、generated schema 对齐 `CoffinOpenRequestV1` / `TutorialHookEventV1`。
  - `agent/packages/tiandao/src/narration/spawn-tutorial-narration.ts` 写入 5 句沉默引导旁白基准。

### 关键 commit

- `2a518283` 2026-05-03 `plan-spawn-tutorial-v1: 接入出生教学状态机与开棺奖励`
- `b4385791` 2026-05-03 `plan-spawn-tutorial-v1: 注入动态灵泉与教学 POI`
- `d0a0d229` 2026-05-03 `plan-spawn-tutorial-v1: 接入客户端开棺请求`
- `80086b50` 2026-05-03 `plan-spawn-tutorial-v1: 对齐教学 schema 与旁白基准`
- `0719b001` 2026-05-03 `plan-spawn-tutorial-v1: 补齐散修与开脉丹宝箱落点`

### 测试结果

- `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`：2112 passed。
- `cd client && JAVA_HOME=/home/kiz/.sdkman/candidates/java/17.0.18-amzn ./gradlew test build`：BUILD SUCCESSFUL（Java 17）。
- `cd agent && npm run build && (cd packages/tiandao && npm test) && (cd packages/schema && npm test)`：tiandao 211 passed；schema 253 passed。
- `cd worldgen && python3 -m unittest discover -s tests`：9 passed。
- `cd worldgen/scripts/terrain_gen && python3 -m unittest test_anvil_export test_anvil_region_writer test_ash_dead_zone`：42 passed。
- `cd worldgen && python3 -m scripts.terrain_gen`：生成 1600 tiles 计划，208 synthesized tiles，输出 raster manifest。
- `validate_rasters("worldgen/generated/terrain-gen/rasters")`：All 208 tiles passed validation。

### 跨仓库核验

- server：`TutorialState`、`TutorialHookEvent`、`CoffinOpenRequest`、`dynamic_rat_swarm_spawner`、`TUTORIAL_KAIMAI_LOOT_POOL_ID`、`BiographyEntry::SpawnTutorialCompleted`。
- worldgen：`dynamic_lingquan_selector`、`spawn_tutorial_pois_for_zone`、`tutorial_lingquan` / `tutorial_chest` / `tutorial_rogue_anchor` / `tutorial_rat_path` POI。
- client：`ClientRequestProtocol.encodeCoffinOpen`、`ClientRequestSender.sendCoffinOpen`、出生点石棺右键 mixin。
- agent/schema：`CoffinOpenRequestV1`、`TutorialHookV1`、`TutorialHookEventV1`、`CoffinOpenedV1`。
- agent/tiandao：`SPAWN_TUTORIAL_HOOK_KEYS`、`SPAWN_TUTORIAL_NARRATION_BASELINES`。

### 遗留 / 后续

- 噬元鼠 v1 仍按 plan 约定用 zombie 占位，只扣真元不扣 HP；真实噬元鼠等待 `plan-fauna-v1`。
- v1 采用右键开棺后直接入包的无 UI 授予，未生成可掉落实体；物理拾取可在 v2 扩展。
- 教学 POI v1 永久保留，不做完成后 despawn / instance reset。
- `plan-narrative-v1` 完整动态 narration 节奏仍留给后续；本 plan 只落 5 句基准。
