# plan-ambient-threat-v1 — 环境威胁刷新：把 danger_level 接活

> **一句话主题**：新增按 zone `danger_level` 驱动的周期性环境威胁 spawner（每 zone 威胁预算 + 距玩家距离环 + 超距回收），物种池按危险度分层（低危鼠群袭扰 → 高危主动妖兽 → 噬灵域接活 tsy_hostile 现成敌对），并给 rat 加低威胁袭扰行为——让 spawn 附近有零星威胁、往 north_wastes 走压迫感陡增，填上"末法世界没有外部威胁"的体验空洞。

**状态**：骨架（skeleton）。升 active 前按 docs/CLAUDE.md §五 收口 §8。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | ambient spawner 核心——danger 预算 / 距离环 / 密度上限 / 回收 | ⬜ |
| P1 | 物种池分层——danger 1~7 分层表 + tsy_hostile 自然涌现接活 | ⬜ |
| P2 | rat 袭扰行为——低威胁骚扰 AI + 咬击偷 qi（守恒）+ 视听 | ⬜ |
| P3 | 生态联动——兽潮门槛改造 / 负灵域威胁加成 / 与 horde 迁徙衔接 | ⬜ |

---

## 背景诊断（2026-07-02，代码实证）

玩家在主世界见不到任何威胁刷新，三重根因：

1. **没有 ambient 敌对 spawner**：历史 zombie 定时刷新器已删未补（`server/src/npc/spawn/mod.rs:192` 注释 "PostStartup zombie spawn 已移除"）。现存威胁 spawn 全挂特殊触发：兽潮要求邻 zone `spirit_qi < 0.15` 且先有灵脉/秘境塌缩事件（`world/heartbeat.rs:733`, `BEAST_TIDE_LOW_QI_THRESHOLD=0.15`）；植物招怪要玩家亲手采 `AttractsMobs` 植物（`botany/hazard.rs:228`）；tsy_hostile 全套敌对（道伥/执念/畸变体/skull_fiend/守灵）**只有 `/tsy_spawn` dev 命令入口**（`npc/tsy_hostile.rs:561`）；黑武士限 `giant_sword_sea`。
2. **`danger_level` 是死数据**：zones.json 28 个 zone 全标了 danger 1~7（spawn=1，north_waste_east_scorch=7），无任何系统读它刷怪。
3. **rat 对玩家中立**：thinker 只有 qi 源追踪/避枯竭/群聚/游荡（`npc/spawn_rat.rs:52-61`），无对玩家的攻击 Scorer；仅兽潮态咬打坐修士（`LOCUST_CULTIVATOR_BITE_RADIUS=6.0`, `LOCUST_BITE_QI_STEAL=1`, `world/events.rs`）。

**交叉预警**：`plan-zone-qi-economy-v1`（skeleton）P1 会把 spawn 稳在 equilibrium 0.35——qi 经济落地后，现存唯一自然威胁入口（兽潮 qi<0.15）被彻底焊死。威胁刷新必须改以 danger_level 为主驱动，这正是本 plan 的立项动因之一。

## 接入面（docs/CLAUDE.md §二 checklist）

- **进料**：`Zone.danger_level`（`world/zone.rs:25-36`，zones.json）；现成 spawn 函数 `spawn_beast_npc_at`（`npc/spawn/beast.rs:51`）/ `spawn_rat_npc_at`（`spawn_rat.rs:63`）/ `spawn_natural_mob_at`（`world/mob_spawn.rs:54`）/ `spawn_tsy_hostiles_for_family`（`tsy_hostile.rs:561`）；时代密度门 `era_beast_spawn_gate`（`mob_spawn.rs:117`）；`CultivationClock`；玩家 Position 查询。
- **出料**：真实 ECS beast/rat/tsy 实体（走现有 thinker/掉落/骨币链，fauna-v1 正典）；死亡→`plan-fauna-v1` 掉落表；agent `world_state` 的 npc_count 自然反映威胁密度；P2 咬击走 `QiTransfer`。
- **共享类型 / event**：复用 `BeastKind` / `FaunaVisualKind` / `NaturalMobKind`——**P0/P1 不新增任何实体种类**，只造调度器。复用 `TsySpawnRequested` 事件链（ambient 触发端仿 dev 命令 emit，不复制 spawn 逻辑）。
- **跨仓库契约**：纯 server plan。无新 payload/schema/Redis key（threat 实体走既有 NPC 下发链路，client 零改动）。
- **worldview 锚点**：worldview §四 战力分层（妖兽/经脉损伤体系）+ `plan-fauna-v1` / `plan-tsy-hostile-v1`（finished，正典物种与掉落）。「危险度地理分布」（spawn 安逸 → 焦土死地凶险）现无 worldview 明文——若收口时判定需补正典条目，worldview 修改单独 PR 人工 review（红线）。
- **qi_physics 锚点**：spawner 本身不动灵气。P2 rat 咬击偷 qi **复用兽潮咬击的既有守恒路径**（`LOCUST_BITE_QI_STEAL` 的 QiTransfer 实现，`world/events.rs`），不自拍新常数、不新写扣减公式；若需独立的袭扰偷 qi 速率常数，先扩 `qi_physics/constants.rs` 再 import。

---

## P0 ambient spawner 核心 ⬜

新模块 `server/src/npc/spawn/ambient_threat.rs`，`register` 挂进 `npc/mod.rs`（对齐现有 `spawn::register` 模式，防孤岛）：

- **威胁预算**：每 zone 按 `danger_level` 查预算表（`fn threat_budget(danger: u8) -> ThreatBudget { max_alive, spawn_interval_ticks, pack_size_range }`，数值表见 §8 #1 收口）；zone 内存活 ambient 威胁计数 ≥ `max_alive` 则跳过。
- **距离环**：只在"有玩家在 zone 内"时刷；spawn 点取距最近玩家 **24~64 格**环带内 Poisson 备选点（复用 `spawn/mod.rs:78` 采样器最小间距思路），不贴脸、不超视距太远。
- **回收**：ambient 威胁带 `AmbientThreatMarker { spawned_at, home_zone }`；距所有玩家 > 96 格持续 N tick 或存活超上限时长 → despawn（**必须 `insert(Despawned)`**，Valence 层实体裸 despawn 会崩服——[[feedback_valence_despawn_layer_entity]]）。
- **门控**：`era_beast_spawn_gate` 时代倍率照吃；`REALM_COLLAPSE` 事件 zone 跳过（塌缩另有兽潮）；spawn 保护：danger=1 的 zone 预算给到"零星、非包围"档而非零（新手也要见到世界有牙）。
- **测试**：预算表边界（danger 0/1/7/未知 zone）；计数≥上限不刷；无玩家不刷；距离环 off-by-one；despawn 走 Despawned 断言；era 倍率 clamp。

## P1 物种池分层 ⬜

`fn threat_pool(danger: u8, dimension: DimensionKind) -> &[ThreatEntry]`（entry = 物种 + 权重 + 群体大小），全部复用现有物种：

| danger | 池（草案，§8 #2 收口） |
|---|---|
| 1~2 | rat 小群（2~4 只，中立袭扰档，见 P2） |
| 3~4 | rat 群 + Spider/Zombie（`spawn_natural_mob_at`，主动） |
| 5~6 | 主动 beast 组 + AshSpider（死域白名单物种，`mob_spawn.rs:30`） |
| 7 | 精英 beast 群（大 pack + 更短间隔） |
| TSY subzone | **接活 tsy_hostile**：ambient 端按 zone 配置周期 emit `TsySpawnRequested`（复用 dev 命令同一条事件链），道伥/执念/畸变体自然涌现 |

- tsy_hostile 接活需先核对 `plan-tsy-hostile-v1` 是否有意保留 dev-only（§8 #3）；接活只加触发端，spawn pool/分层/掉落零改动。
- **测试**：每 danger 档池命中专属 case；TSY 维度隔离（主世界不出道伥）；权重抽样分布 pin。

## P2 rat 袭扰行为 + 视听 ⬜

给 ambient 档 rat 加"低威胁骚扰"：新增 `PlayerHarassScorer`（玩家 ≤ 8 格且冷却就绪时起评）→ `HarassBiteAction`（冲近咬一口 → 偷 qi 1 点（复用兽潮咬击 QiTransfer 路径）→ 立即进逃逸/游荡冷却 20s）。不伤血、不锁定追杀——是"烦人的末法鼠患"不是战斗怪；打坐时被咬会打断凝神（对齐兽潮咬击既有语义）。

**视听（全复用既有原语，无新增资产）**：
- 粒子：咬中瞬间复用既有 qi 抽取类 VfxPlayer 事件（兽潮咬击现有表现；若无则 `BongSpriteParticle` 复用既有灰白 sprite，burst 4 粒、lifetime 8 tick、色 `#9B9B8C`、自咬点向上飘 0.03 b/t），经 `VfxBootstrap.registerDefaults()` 注册核对，漏注册=静默孤岛。
- SFX：`entity.silverfish.ambient` pitch 0.7 vol 0.5（咬击）+ `entity.rat` 无原版音，逃逸复用现有 rat 移动音（无则不加）。
- HUD：不加常驻元素；被咬走既有事件流（通用非战斗专用）出一条条目。
- narration：无（低威胁事件不值得天道开口）。
- **测试**：Scorer 距离/冷却边界；偷 qi 守恒对拍（玩家 -1 → 咬击者 +1 或按既有路径归宿）；打坐打断分支。

## P3 生态联动 ⬜

- **兽潮门槛改造**：`BEAST_TIDE_LOW_QI_THRESHOLD` 判定改双因子（qi 骤降速率或塌缩事件 × danger 加权），使 qi-economy 落地后兽潮仍可达——具体公式与 `plan-zone-qi-economy-v1` 收口联动（§8 #5）。
- **负灵域/死域加成**：`spirit_qi < 0` 的 zone 威胁预算乘区（末法「灵气枯竭生异变」叙事方向，正典措辞待 §8 #4 确认）。
- **horde 衔接**：ambient 刷出的常驻 beast 使 `beast_horde_detect_system`（`fauna/migration.rs:388`）的 `beast_count==0` 短路自然消失，迁徙系统开始有料可迁——**只声明衔接，不吞 `plan-beast-horde-v1` P2 领地争夺 scope**。
- **测试**：双因子门槛矩阵；负灵域乘区；ambient 存量触发迁徙的集成 case。

---

## §8 开放问题（升 active / P0 决策门前收口）

1. **预算表数值**：danger 1~7 各档 `max_alive / spawn_interval / pack_size` 具体值；服务器性能预算（与 dormant `max_dormant_count=5000` 及 hydrate 半径 64 共存时的实体峰值上限）。
2. **物种池终表**：P1 草案表逐档确认；danger 7 "精英"是否需要新 buff 修饰（倾向：只调 pack/间隔，不造新怪）。
3. **tsy_hostile dev-only 是否有意**：查 `plan-tsy-hostile-v1` Finish Evidence / 遗留节是否预留了自然涌现 Phase2 口子；若原设计刻意 dev-only（等某上游 plan），改为在此接活须写明理由。
4. **worldview 正典补充**：「危险度地理分布」「灵气枯竭生异变」是否需要 worldview 明文条目（若需，单独 PR 人工 review，本 plan 归档前 land）。
5. **兽潮双因子公式**：与 `plan-zone-qi-economy-v1` 的 equilibrium/inflow 数值联动收口（两 plan 若同期 active，先后顺序与共享常数归属）。
6. **昼夜/天气权重**：夜间加成是否引入（现无昼夜 spawn 概念；引入则 P0 加权重钩子，数值后调）。

## §10（升 active 时补）

scope 预估 4 PR（P0~P3 各一），按 docs/CLAUDE.md §六补完整工作流；P2 含视听但全复用既有原语，无建筑/资产多轮要求。
