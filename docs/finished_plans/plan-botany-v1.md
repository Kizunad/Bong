# Bong · plan-botany-v1

**野生植物生态 + 采集专项**。与 `plan-lingtian-v1`（人工种植）**职责分离**：本 plan 管野外生态（自然生长/凋零/采集），lingtian 管玩家经营的田块。alchemy / forge 的灵草、灵木等材料由本 plan 供给。

**世界观锚点**：
- `worldview.md §十` — 灵气零和（SPIRIT_QI_TOTAL = 100），灵草需 `zone.spirit_qi > 0.3` 才生长
- `worldview.md §十三` — 区域尺度：初醒原/青云残峰/灵泉湿地/血谷/幽暗地穴/北荒
- `worldview.md §七` — 灵物密度阈值（天道盯高聚集点）——本 plan 不直接触发，但提供材料上的零和约束

**正典植物源**（**必须对齐**，不得自造名）：
- `docs/library/ecology/末法药材十七种.json` — 十七味可采药材（常用 7 + 稀见 5 + 毒性 5）
- `docs/library/ecology/辛草试毒录.json` — 七味辛草（微辛 2 + 烈辛 3 + 绝辛 2），其中赤髓草/噬脉根与十七种重叠
- **合并去重后共 22 种**，alchemy/forge 配方只能引用这些 ID，不得新造（新造需先进 library）

**关键决策**：
- 植物动态行为**全部在 Rust server 侧**，不耦合 worldgen Python 侧
- server 只读 `zone.biome + zone.spirit_qi` 判定生成/分布
- worldgen 只负责地形/biome，不负责植物（现状保持）

**交叉引用**：`plan-alchemy-v1.md`（材料消费方）· `plan-forge-v1.md`（灵木等载体材料）· `plan-lingtian-v1.md`（人工种植姊妹 plan）· `plan-inventory-v1.md`（植物 item）· `worldview.md §十`。

---

## §0 设计轴心

- [ ] **零和生态**：植物生长消耗 `zone.spirit_qi`，死亡才归还
- [ ] **采集 ≠ 归还灵气**：采走植物 = 灵气被玩家带走（离开区域），只有**原地凋零死亡**才归还
- [ ] 混合生长模型：通用草本按区域刷新 · 稀有灵草固定点位 + 缓慢再生
- [ ] 采集是一次短 session（可打断，踩踏有概率弄死植物）
- [ ] 动物 / 异变兽骨**不在本 plan**（等 `plan-fauna-v1`，见 §7）

---

## §1 子系统拆解

### §1.1 植物分类

三层级 × 正典 22 种（数据来自 `末法药材十七种` + `辛草试毒录`，ID 用规范中文名对应的拼音蛇形）：

| 层级 | 生长模型 | 正典品种 |
|---|---|---|
| **通用** | 区域刷新（定期 spawn） | 刺舌蒿（ci_she_hao）· 回元芷（hui_yuan_zhi）· 凝脉草（ning_mai_cao）· 清浊草（qing_zhuo_cao）· 安神果（an_shen_guo） |
| **区域专属** | 混合（区域刷新 + 静态点） | 固元根（gu_yuan_gen，>0.6 深处）· 赤髓草（chi_sui_cao，血谷红砂岩）· 针芥子（zhen_jie_zi，青云/湿地）· 烧喉蔓（shao_hou_man，幽暗地穴）· 解蛊蕊（jie_gu_rui，幽暗地穴）· 夜枯藤（ye_ku_teng，幽暗深处） |
| **极稀 / 事件触发** | 事件 + 静态，采后长期不重生 | 灰烬苔（hui_jin_tai，残灰方块）· 养经苔（yang_jing_tai，死域边缘）· 噬脉根（shi_mai_gen，负灵域裂缝）· 空兽痕（kong_shou_hen，负灵域残灰）· 天怒椒（tian_nu_jiao，伪灵脉焦土）· 灵眼石芝（ling_yan_shi_zhi，灵眼上方 ⏳） |
| **毒性警戒**（可采不可炼） | 同通用/专属 | 蜉蝣花 · 无言果 · 黑骨菌 · 浮尘草 · 终焉藤（全部 deadly，误食触发特殊死亡） |

> **灵眼石芝** 依赖灵眼结构，灵眼未实装（reminder 已记），MVP 先禁用生成。
> **辛度**：微辛 / 烈辛 / 绝辛 是正典属性（辛草试毒录），影响丹毒色（Mellow/Sharp/Violent）—— alchemy 配方需对齐。
> **辛度与灵气反相关**：灵气越稀薄处辛度越高（正典已确立原理）。

### §1.2 生长模型

#### §1.2.1 通用草本（区域刷新）

每 `BOTANY_TICK_INTERVAL`（默认 5 分钟）对每个 zone 执行：

```
target_count(plant_kind) = floor(zone.spirit_qi × plant_kind.density_factor)
current_count = 统计 zone 内该 plant_kind 活体数
delta = target_count - current_count

if delta > 0 AND zone.spirit_qi >= plant_kind.growth_cost:
    for _ in 0..delta:
        pick random empty block in zone (符合 soil/water 要求)
        spawn plant
        zone.spirit_qi -= plant_kind.growth_cost
```

- [ ] 生成位置：白天优先草地/沼泽表面，洞穴有少量阴性植物
- [ ] `growth_cost` 典型 0.001-0.005（百株才扣 0.1 灵气）
- [ ] 区域灵气不足 → 不再生长（目标计数压不满是常态）

#### §1.2.2 区域专属（混合）

- 区域刷新部分：同 §1.2.1 但受 biome 过滤
- 静态点位部分：worldgen raster 或 server 启动期随机选 N 个点作为"古灵植源头"——采后走再生倒计时（数小时至数天）

#### §1.2.3 极稀（事件触发，全对齐正典）

- 残灰方块表面 → 生成灰烬苔（`hui_jin_tai`，40min 重生）
- 死域边缘 → 生成养经苔（`yang_jing_tai`）
- 异变兽死亡 → 尸旁 3 块概率生成空兽痕（`kong_shou_hen`）或死后落残灰
- 伪灵脉消散焦土 → 片刻内可采天怒椒（`tian_nu_jiao`，稍纵即逝，天道陷阱）
- 负灵域裂缝 → 噬脉根（`shi_mai_gen`），每处裂隙仅一株，采完即无
- **不扣区域灵气** 的特殊路径：负灵域 / 残灰基底产的植物（它们本就在零或负灵气环境）

#### §1.2.4 凋零 / 死亡归还

```
每 tick 对活体植物：
  if zone.spirit_qi < plant_kind.survive_threshold:
      plant.wither_progress += 1
      if plant.wither_progress >= WITHER_LIMIT:
          plant dies → zone.spirit_qi += plant_kind.growth_cost × RESTORE_RATIO
```

- [ ] `RESTORE_RATIO` 典型 0.8（死亡归还 80%，体现"生长有浪费"）
- [ ] 自然寿命：达到 `plant_kind.max_age` 也触发凋零
- [ ] **关键**：只有**死亡**才归还，被玩家采走的不归还 → 玩家活跃度直接造成灵气流出

### §1.3 采集交互（短 session，小浮窗）

> 草图 `docs/svg/harvest-popup.svg`。UI 层级 = **A 层 HUD 浮窗**（非全屏 Screen，不遮挡游戏），锚在植物方块屏幕投影附近，可拖拽，玩家开着窗仍能看场景。

玩家右键活体植物 → 弹出浮窗，**二选一**：

| 模式 | 时长 | 条件 | 打断 | XP | 品质 |
|---|---|---|---|---|---|
| **手动采集** | 2-3s | 任意玩家 | WASD/ESC/受击立即断 | 基础（+1~2）| 随节奏抖动 |
| **自动采集** | 5-8s | 采药经验 ≥ 阈值（默认 Lv.3）| 仅受击断；可慢速走动 | 熟练加成（+3~6，数倍于手动）| 稳定（不抖动）|

- [ ] **浮窗 modal 但非阻塞**：同时只能开一个，场景仍可见，不暂停游戏
- [ ] **按键**：E 手动 / R 自动（自动未解锁时 R 置灰）
- [ ] **采药经验系统**：以击杀/炼丹等通用 XP 体系分叉出的子技能（`BotanySkillStore`），每次完成采集 +XP；等级决定自动解锁门槛、手动时长、品质分布
- [ ] **踩踏判定**：玩家/实体路径经过植物方块 → 5% 概率植物死（触发 §1.2.4 归还）；浮窗打开期间踩到**目标植物**同样生效
- [ ] **工具钩子**：右键时检测主手 item，有采药刀 / 灵铲时走工具路径 —— 工具系统未立，见 reminder.md
- [ ] 采集 session 是**轻量**的，不持久化到 BlockEntity（和炼丹/锻造对比）

**为什么"自动 = 高 XP 不是懒人奖励"**：自动需要先练到门槛（新手只能手动磨经验），且自动要求环境安全（受击仍断）。在安全区用自动等于"熟手懒得专注，但仍承担 5s 不能跑的风险"，合理。

### §1.4 server 侧数据结构（Rust 耦合点）

```rust
// 植物类型表
pub struct PlantKind {
    pub id: PlantId,
    pub item: ItemId,                  // 采集 drop 的 item
    pub biomes: Vec<BiomeTag>,         // 允许生长的 biome
    pub density_factor: f32,           // 区域刷新的目标密度
    pub growth_cost: f32,              // 生长消耗 spirit_qi
    pub survive_threshold: f32,        // zone.spirit_qi 低于此开始凋零
    pub max_age: u32,                  // 自然寿命（ticks）
    pub regen_ticks: u32,              // 静态点位再生时长
    pub spawn_mode: SpawnMode,         // ZoneRefresh / StaticPoint / EventTriggered
}

// 活体实例（挂在方块或轻 entity）
#[derive(Component)]
pub struct Plant {
    pub kind: PlantId,
    pub planted_at: u64,
    pub wither_progress: u32,
    pub source_point: Option<StaticPointId>,  // 若来自静态点，记录以便再生
}
```

- [ ] PlantKind 注册表：server/assets/botany/plants/*.json 或编译期常量表
- [ ] 不直接读 terrain_profiles（Python 侧）；只消费 server 侧已有的 `ZoneInfo { biome, spirit_qi }`

---

## §2 灵气闭环

```
zone.spirit_qi
   │
   ├── 生长消耗 ──→ 植物（活体持有灵气）
   │                  │
   │                  ├── 玩家采走 ──→ 灵气永远离开 zone（流失）
   │                  └── 自然凋零 ──→ RESTORE_RATIO × growth_cost 归 zone
   │
   └── 天道回收（worldview §七，与本 plan 并行）
```

**玩家行为的零和后果**：
- 采得多的区域 → 灵气下降 → 下一批生长停滞 → 该区域贫瘠
- 留原地自然枯萎 → 灵气回补，但该植物白白消耗了一遍
- 长线来看：**采集 = 以灵气换材料**，不是"白给"

---

## §3 MVP

### §3.1 MVP 测试植物（全部用正典名）

| 植物 ID | 层级 | biome / 条件 | density | growth_cost | 再生 | 验证意图 |
|---|---|---|---|---|---|---|
| `ci_she_hao`（刺舌蒿，微辛）| 通用 | Any · `spirit_qi 0.2-0.4` | 4.0 | 0.002 | — | 区域刷新基线（量大，新手易得）|
| `ning_mai_cao`（凝脉草）| 通用 | 馈赠区 · `spirit_qi > 0.4` | 2.0 | 0.003 | — | 基础材料链（alchemy 凝脉散引）|
| `hui_yuan_zhi`（回元芷）| 通用 | 湿地 | 1.5 | 0.003 | — | 回元丹主料 |
| `chi_sui_cao`（赤髓草，烈辛）| 区域专属 | BloodValley 红砂岩 | 1.0 | 0.005 | — | biome 过滤 + 凝脉/固元丹主料 |
| `gu_yuan_gen`（固元根）| 区域专属 + 静态点 | 深处 · `spirit_qi > 0.6` | 0.3 | 0.01 | 6h | 静态点再生测试 |
| `hui_jin_tai`（灰烬苔）| 事件触发 | 残灰方块表面 | — | 0 | 40min | 基底触发（残灰方块） |
| xue_cao（血草） | 区域专属 | BloodValley | 1.0 | 0.005 | — | biome 过滤 |
| shi_xin_hua（尸心花） | 极稀 | 任意，兽尸旁 3 块 | event | 0 | 一次性 | 事件触发生长 |

### §3.2 初始 biome 标签映射

| worldview 区域 | BiomeTag（server 侧） |
|---|---|
| 初醒原 | Plains |
| 青云残峰 | Mountain |
| 灵泉湿地 | LingquanMarsh |
| 血谷 | BloodValley |
| 幽暗地穴 | UndergroundCave |
| 北荒 | NorthWastes |

### §3.3 交互 MVP

- [ ] 右键植物方块 → 2s 进度条 HUD（A 层最简，不走 DynamicXmlScreen）
- [ ] 打断：移动 / 受击
- [ ] 踩踏：5% 概率植物死亡（走 §1.2.4 归还）
- [ ] drop 走现有 `InventoryStateStore` 收入背包

---

## §4 数据契约

### Server 侧

- [ ] `PlantKindRegistry` resource（启动期加载）
- [ ] `Plant` component（挂方块 / 轻 entity）
- [ ] `StaticPlantPoint` resource（静态点位表 + 再生倒计时）
- [ ] `BotanyTick` system（定期刷新 + 凋零检查）
- [ ] `HarvestSession` resource（进行中的采集，map: player_id → session）
- [ ] Events：`PlantSpawn` / `PlantWither` / `HarvestStart` / `HarvestComplete` / `HarvestInterrupt`
- [ ] Channel：`bong:botany/spawn` · `bong:botany/wither` · `bong:botany/harvest_progress`
- [ ] 接入 `zone.spirit_qi` 双向流动（生长扣 / 死亡归还）

### Client 侧

- [ ] `HarvestSessionStore`（当前采集进度 + mode: Manual/Auto，HUD 渲染用）
- [ ] `BotanySkillStore`（采药经验 Lv + XP + 解锁门槛）
- [ ] 植物方块贴图（MVP 占位色块）
- [ ] 复用：`InventoryStateStore`（drop 入包）

---

## §5 阶段划分

| Phase | 内容 | 验收 |
|---|---|---|
| P0 | PlantKindRegistry + ZoneRefresh 生长 tick | 百草能在 spirit_qi > 0.3 的 zone 周期性生成 |
| P1 | 凋零 + 灵气归还 | 灵气降到阈值下植物死，spirit_qi 回补可观测 |
| P2 | 区域专属 + StaticPlantPoint 再生 | 灵木在灵泉湿地静态点，采后 3h 再生 |
| P3 | HarvestSession 浮窗 + 手动/自动二选 + BotanySkill + 打断/踩踏 | 新手只能手动；练到 Lv.3 解锁自动；踩踏概率弄死 |
| P4 | 事件触发（兽尸 → 尸心花） | 击杀异变兽后尸旁生成 |
| P5 | alchemy / forge 接入真实 item 替换 placeholder | 开脉丹炼制不再用 string id |

---

## §6 跨 plan 钩子

- [ ] **plan-alchemy-v1 §3.2**：替换 placeholder material ID → 正典名（`kai_mai_cao` → `ci_she_hao` 或 `ning_mai_cao` 按配方重审；`xue_cao` → `chi_sui_cao`；`bai_cao` → `hui_yuan_zhi`）；同时对齐辛度 → 丹毒色映射
- [ ] **plan-forge-v1 §3.2**：灵木 / 异兽骨（本 plan 只管植物，骨骼归 fauna）等载体材料
- [ ] **plan-lingtian-v1**：人工田块作物 = PlantKind 亚种（`cultivable: true`）· 共用 PlantKindRegistry + harvest-popup + BotanySkill · 混合灵气模型（plot_qi 为主 + 区域漏吸兜底，详见 lingtian §1.3）
- [ ] **plan-inventory-v1**：植物 item 定义（尺寸 / 堆叠 / 操作磨损）
- [ ] **plan-worldgen-v3.1.md**：worldgen 只负责 biome 标签，不管植物分布
- [ ] **plan-npc-ai-v1**：散修 NPC 采集行为（共用 HarvestSession）
- [ ] **plan-tribulation-v1**：天劫遗址触发 shi_xin_hua / 雷霆竹生成

---

## §7 TODO / 开放问题（v2+）

- [ ] **采药工具**：采药刀 / 灵铲 / 灵镰（品质 + 安全度修正）— 见 reminder.md
- [ ] **季节系统**：某些植物仅在特定 in-game 季节生长
- [ ] **灵眼系统立项后**：极稀植物（真正的灵眼旁植）回补
- [ ] **plan-fauna-v1**（待立）：异变兽 / 灵兽骨骼 / 妖丹 —— 供给 forge 的 `yi_beast_bone` / `shou_gu` / `huo_jing`
- [ ] **植物变异**：天劫余波 / 负灵域 / 玩家真元色沾染 → 产生稀有变种
- [ ] **炼丹废料反哺**：alchemy 失败时的残渣作为肥料 → lingtian 田块加成（跨 plan 闭环）
- [ ] **生态系统可视化**：管理员/agent 工具查看区域 spirit_qi 曲线、植物密度热图

---

## §8 风险与对策

| 风险 | 对策 |
|---|---|
| 区域刷新扫整图性能爆炸 | 只扫已加载区块；zone 级聚合而非 per-block；tick 间隔 5 分钟足够 |
| 玩家集中收割导致某区永久贫瘠 | worldview §七 天道灵气重分配本就会缓慢回补；采集流失是设计，不是 bug |
| 静态点再生被单个玩家垄断 | 再生倒计时 + 点位坐标不显示给玩家（靠探索），匿名系统天然防垄断 |
| 踩踏概率触发过多"无意义死亡" | 5% 是基线，可调低；踩踏不给玩家反馈（植物悄悄死，玩家不会专门踩） |
| alchemy/forge 先行依赖 placeholder，接入时大量 item 重命名 | placeholder 命名已对齐（kai_mai_cao 等），接入只需定义 item 不改引用 |
