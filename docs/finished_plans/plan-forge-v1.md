# Bong · plan-forge-v1

**炼器专项**（不含炼丹——炼丹见 `docs/plan-alchemy-v1.md`）。MVP 范围：**武器**。防具 / 法器 / 暗器留待 v2+。

**世界观锚点**：
- `worldview.md §三` — "材料/丹药永远是辅助"，武器同理：再好的器也替代不了境界，上限受真元池限制
- `worldview.md §六 染色谱` — 武器的"灵性"可沾染持有者真元色（详见 §1.3 开光）
- `worldview.md §九` — 器物可自由交易（**不做 bond**），顶级资产是信息而非武器
- `worldview.md §十` — 载体材料（异兽骨骼、灵木）**非强制**，但决定品阶上限

**关键复用**：
- `cultivation::forging` — 已有**经脉锻造**（流速/容量两轴，`ForgeAnvil` 命名冲突需避免，本 plan 用 `WeaponForgeStation`）
- 客户端塔科夫背包（`InventoryStateStore` / `BackpackGridPanel` / `DragState` / `ItemTooltipPanel`）—— **UI 结构复用，具体面板重做**

**交叉引用**：`plan-combat-v1.md §5`（ForgeWeapon 钩子）· `plan-inventory-v1.md`（武器 item）· `plan-alchemy-v1.md`（同栈 JSON 配方模式对齐）· `plan-botany-v1.md`（载体材料灵木等，待立）。

---

## §0 设计轴心

- [x] **独立锻炉**（不与炼丹共炉）—— 交互轴是"**锻打节奏**"而非"温度"
- [x] **四步串行**：坯料 → 淬炼 → 铭文 → 开光，每步可单独失败
- [x] **载体材料非强制**，但决定品阶上限（凡铁只能打到法器）
- [x] **不形成 bond** —— 武器可自由交易、掉落、拾取
- [ ] **开光 ≠ 绑定**：仅给武器染上持有者真元色（+小幅同色加成），换人用效果衰减但不失效
- [x] 品阶四阶：**凡器 / 法器 / 灵器 / 道器**（沿用 alchemy §3 约定）
- [x] 配方 JSON 加载 + 多结果分桶 + 残缺匹配（与 alchemy 同栈）
- [x] MVP 只做武器；防具 / 法器（飞剑/灵幡）/ 暗器留 §7

---

## §1 子系统拆解

### §1.1 图谱系统（JSON）

路径：`server/assets/forge/blueprints/*.json`。结构与 alchemy recipe 对齐（便于未来统一 `CraftingRegistry`）。

图谱结构要点：
- `steps[]` — 四步：`{ kind: "billet" | "tempering" | "inscription" | "consecration" }`
- 每步有独立的 `profile`（坯料配比 / 淬炼节奏 / 铭文公差 / 开光真元量）
- `tier_cap` — 本图谱能达到的最高品阶（由载体材料决定）
- `outcomes` — 与 alchemy 同五桶：perfect/good/flawed/waste/explode（explode = 武器断裂）
- `flawed_fallback` + `side_effect_pool` — 同 alchemy，产出残次武器 + 随机附加（可好可坏）

- [x] 加载器：启动期扫目录 → `BlueprintRegistry` resource
- [ ] IPC Schema 放 `agent/packages/schema`

### §1.2 锻炉系统（独立实体）

```rust
#[derive(Component)]
pub struct WeaponForgeStation {
    pub tier: u8,              // 凡铁砧 / 灵铁砧 / 玄铁砧 / 道砧
    pub owner: Option<Entity>,
    pub session: Option<ForgeSessionId>,
    pub integrity: f32,        // 断锤 / 砧裂扣完整度
}
```

- [ ] MVP 为方块 + BlockEntity（与 alchemy 并列，不共享）
- [x] tier 限制能使用的图谱（凡铁砧最高锻法器）
- [x] **多炉并行**：一玩家可绑多个砧（每炉独立 session_id，sessions 总表分发事件）

### §1.3 四步进程（核心循环）

```rust
pub struct ForgeSession {
    pub blueprint: BlueprintId,
    pub station: Entity,
    pub caster: Entity,
    pub current_step: ForgeStep,
    pub step_state: StepState,   // 每步独立状态
    pub materials_in: Vec<(ItemId, u32)>,
    pub interventions: Vec<Intervention>,
}

pub enum ForgeStep {
    Billet,         // 坯料
    Tempering,      // 淬炼
    Inscription,    // 铭文（可跳过，仅影响上限）
    Consecration,   // 开光（可跳过）
    Done,
}
```

#### §1.3.1 坯料（Billet）

玩家从背包拖入基础金属 + 可选载体材料（异兽骨/灵木）到配比槽。

- [x] 配比公差（类似 alchemy 投料，但此处可多种混合）
- [x] 载体材料决定 `tier_cap`（无载体 = 法器上限；灵木 = 灵器上限；异兽骨 + 灵木 = 道器上限）
- [x] 失败 = 坯料废，材料不返还，session 终止

#### §1.3.2 淬炼（Tempering）

**核心交互：锻打节奏**（替代 alchemy 的温度滑块）。

- [ ] 屏幕出现节奏提示（类似音游滚动条）：Light / Heavy / Fold 三种指令
- [x] 玩家按 J（轻）/ K（重）/ L（折）键在节奏窗口内击中（服务端 `TemperingHit` 事件 + `apply_tempering_hit` 命中判定，客户端按键映射 UI 待做）
- [x] 连击 combo 提升品阶进度；错拍 / 过拍累积偏差（hits/misses/deviation 计入 TemperingState，resolve_tempering 分桶）
- [x] 每次按键消耗少量真元（体力向，非真元池主消耗）
- [ ] 淬炼时长 ~30-60 秒（单把武器），太久会腻

偏差累积超过 `tempering_profile.tolerance` → 走 flawed / waste。

#### §1.3.3 铭文（Inscription，可跳过）

- [x] 成功铭文 → 品阶 +1（最高 `tier_cap`）
- [x] 铭文槽位：根据武器类型固定 1-3 条（剑 1 / 长柄 2 / 双手重器 3）
- [x] 铭文内容来自**铭文残卷 item**（服务端 `InscriptionScrollSubmit` 事件 + `apply_scroll` 已实装；item 由 inventory plan 落地）
- [x] 失败 → 武器留疤，品阶锁在当前

#### §1.3.4 开光（Consecration，可跳过）

- [x] 注入持有者真元若干（`consecration_profile.qi_cost`，典型 30-100）
- [x] 武器沾染施术者的 `ColorKind`（单主色，不可覆盖除非重打）
- [ ] 效果：**同色攻击 +小幅**（10-20%），异色持有者使用**衰减**（70%）
- [x] **不形成 bond** —— 武器可正常交易，只是买家拿到会比卖家弱
- [x] 道器必须开光（否则最高锁灵器）

#### 材料消耗 / 失败策略

- [x] **投入即消耗**，起炉前右键取回，起炉后锁定
- [x] 任一步 waste/explode → 材料全失 + session 终止
- [x] explode 对 `WeaponForgeStation.integrity` 扣分（断锤伤人）

#### 残缺匹配（同 alchemy）

- [x] 坯料配比错 → 走 `flawed_fallback` + `side_effect_pool` 抽随机附加（可好可坏，记入 `LifeRecord`）
- [ ] 无 fallback → waste

#### 离线 / 持续性

- [x] 服务器常驻，session 持续 tick（与 alchemy 策略一致）
- [ ] BlockEntity 持久化 session_id

### §1.4 图谱学习与切换

```rust
#[derive(Component)]
pub struct LearnedBlueprints {
    pub ids: Vec<BlueprintId>,
    pub current_index: usize,
}
```

- [x] 机制同 `LearnedRecipes` —— 拖【图谱残卷】item 到图谱卷轴区学习
- [x] 翻页切换已学图谱
- [x] 已学再拖提示"此图已悟"，不消耗

---

## §2 品阶系统

| 品阶 | 达成条件 | 典型来源 |
|---|---|---|
| 凡器 | 坯料成，其余跳过 | 铁匠新手 |
| 法器 | 坯料 + 淬炼达标 | 散修常用 |
| 灵器 | + 铭文成 | 需灵木载体 + 铭文残卷 |
| 道器 | + 开光成 | 需异兽骨 + 灵木 + 高境界真元 |

- [ ] 品阶影响 `base_damage` / `qi_capacity`（法器以上可存真元） / `durability` / `拾取权重`
- [x] 不做"仙器"——worldview §三 明禁

---

## §3 MVP

### §3.1 测试武器（3 种，验证四步路径）

| 武器 | 品阶目标 | 步骤 | 验证意图 |
|---|---|---|---|
| 铁剑 | 凡器 | Billet 一步 | 最短路径，验证闭环 |
| 青锋剑 | 法器 | Billet + Tempering | 锻打节奏交互 |
| 灵锋 | 灵器 → 道器 | 全四步 | 残卷/铭文/开光/真元染色全跑 |

### §3.2 测试图谱 JSON（仅测试）

```json
// blueprints/iron_sword_v0.json — 铁剑（凡器，最简）
{
  "id": "iron_sword_v0",
  "name": "铁剑（测试）",
  "station_tier_min": 1,
  "tier_cap": 1,
  "steps": [
    {
      "kind": "billet",
      "profile": {
        "required": [{ "material": "iron_ingot", "count": 3 }],
        "optional_carriers": [],
        "tolerance": { "count_miss": 0 }
      }
    }
  ],
  "outcomes": {
    "perfect": { "weapon": "iron_sword",         "quality": 1.0 },
    "good":    { "weapon": "iron_sword",         "quality": 0.8 },
    "flawed":  { "weapon": "iron_sword_flawed",  "quality": 0.5 },
    "waste":   null,
    "explode": { "damage": 6.0, "station_wear": 0.02 }
  }
}
```

```json
// blueprints/qing_feng_v0.json — 青锋剑（法器，坯料+淬炼）
{
  "id": "qing_feng_v0",
  "name": "青锋剑（测试）",
  "station_tier_min": 1,
  "tier_cap": 2,
  "steps": [
    { "kind": "billet", "profile": {
      "required": [{ "material": "iron_ingot", "count": 4 }, { "material": "qing_steel", "count": 1 }],
      "optional_carriers": [],
      "tolerance": { "count_miss": 0 }
    }},
    { "kind": "tempering", "profile": {
      "pattern": ["L","L","H","L","F","H","L","F","H","H"],
      "window_ticks": 12,
      "qi_per_hit": 0.5,
      "tolerance": { "miss_allowed": 2 }
    }}
  ],
  "outcomes": {
    "perfect": { "weapon": "qing_feng_sword",        "quality": 1.0 },
    "good":    { "weapon": "qing_feng_sword",        "quality": 0.75 },
    "flawed":  { "weapon": "qing_feng_sword_flawed", "quality": 0.45 },
    "waste":   null,
    "explode": { "damage": 15.0, "station_wear": 0.05 }
  },
  "flawed_fallback": {
    "weapon": "qing_feng_sword_flawed",
    "quality_scale": 0.5,
    "side_effect_pool": [
      { "tag": "+5_durability",        "weight": 1 },
      { "tag": "-10_durability",       "weight": 2 },
      { "tag": "minor_qi_leak_on_hit", "weight": 1 }
    ]
  }
}
```

```json
// blueprints/ling_feng_v0.json — 灵锋（四步全流程，验证道器路径）
{
  "id": "ling_feng_v0",
  "name": "灵锋（测试）",
  "station_tier_min": 2,
  "tier_cap": 4,
  "steps": [
    { "kind": "billet", "profile": {
      "required": [{ "material": "xuan_iron", "count": 3 }],
      "optional_carriers": [
        { "material": "ling_wood",    "unlocks_tier": 3 },
        { "material": "yi_beast_bone","unlocks_tier": 4 }
      ],
      "tolerance": { "count_miss": 0 }
    }},
    { "kind": "tempering", "profile": {
      "pattern": ["H","L","F","H","L","F","F","H","L","F","H","H","F","L","H"],
      "window_ticks": 8,
      "qi_per_hit": 0.8,
      "tolerance": { "miss_allowed": 1 }
    }},
    { "kind": "inscription", "profile": {
      "slots": 2,
      "required_scroll_count": 2,
      "tolerance": { "fail_chance": 0.2 }
    }},
    { "kind": "consecration", "profile": {
      "qi_cost": 80.0,
      "min_realm": "Tongling",
      "tolerance": { "qi_miss_ratio": 0.05 }
    }}
  ],
  "outcomes": {
    "perfect": { "weapon": "ling_feng_sword",        "quality": 1.0 },
    "good":    { "weapon": "ling_feng_sword",        "quality": 0.8 },
    "flawed":  { "weapon": "ling_feng_sword_flawed", "quality": 0.4 },
    "waste":   null,
    "explode": { "damage": 40.0, "station_wear": 0.12 }
  },
  "flawed_fallback": {
    "weapon": "ling_feng_sword_flawed",
    "quality_scale": 0.4,
    "side_effect_pool": [
      { "tag": "random_color_tint",    "color": "Insidious", "weight": 1 },
      { "tag": "qi_cap_perm_minus_1",  "perm": true,          "weight": 1 },
      { "tag": "rare_weapon_insight",  "weight": 1 }
    ]
  }
}
```

### §3.3 交互 UI（B 层 BaseOwoScreen，UI 重做但复用塔科夫背包）

> 草图 TODO：`docs/svg/forge-station.svg`（下一步画）。

- [ ] 层级：BaseOwoScreen（右键砧方块打开）
- [ ] 三列布局 1560×900 居中：
  - 左：**图谱卷轴**（四步进度指示 + 翻页 + 拖图谱残卷学习）
  - 中：**当前步骤主舞台**（按 `current_step` 切换视图）
    - Billet：坯料配比槽 + 载体材料槽 + 品阶上限指示
    - Tempering：锻打节奏轨道（滚动条 + J/K/L 键提示）
    - Inscription：铭文槽 + 残卷拖入区
    - Consecration：真元注入进度 + 当前真元色预览
  - 右：**塔科夫背包**（复用 `BackpackGridPanel` 5×7 + 多 tab）
- [ ] 底栏：五结果桶实时概率 + 当前步骤偏差条
- [ ] **不做**配方校验 UI（投错走残缺）

---

## §4 数据契约

### Server 侧

- [x] `BlueprintRegistry` resource
- [ ] `WeaponForgeStation` component + BlockEntity（component 已实装，BlockEntity 持久化层未接入）
- [x] `ForgeSession` resource（含 step_state / materials_in）
- [x] `LearnedBlueprints` component
- [x] `LifeRecord.forge_attempts: Vec<ForgeAttempt>`（同 alchemy_attempts，亡者博物馆可见）
- [x] Events：`StartForgeRequest` / `TemperingHit`（锻打节奏击键）/ `StepAdvance` / `ForgeOutcome`（额外含 `InscriptionScrollSubmit` / `ConsecrationInject`）
- [ ] Channel：`bong:forge/start` · `bong:forge/tick` · `bong:forge/hit` · `bong:forge/outcome`

### Client 侧（新增 Store）

- [ ] `WeaponForgeStationStore`
- [ ] `ForgeSessionStore`（step / step_state / rhythm_track 实时）
- [ ] `BlueprintScrollStore`
- [ ] 复用：`InventoryStateStore` · `BackpackGridPanel` · `DragState` · `ItemTooltipPanel`

---

## §5 阶段划分

| Phase | 内容 | 验收 |
|---|---|---|
| P0 | BlueprintRegistry + JSON 加载 + IPC Schema | 三份测试图谱加载无错 ✅ server |
| P1 | WeaponForgeStation BlockEntity + 打开 Screen | 右键打开，RecipeScroll 可翻页 |
| P2 | Billet 步骤 + 材料消耗 + tier_cap 计算 | 铁剑 MVP 跑通（凡器） ✅ server |
| P3 | Tempering 锻打节奏轨道 + 按键时序 | 青锋剑跑通（法器） ✅ server（事件/解析；UI 轨道未做） |
| P4 | Inscription 铭文槽 + 残卷消耗 | 达到灵器 ✅ server（事件/解析；UI 槽位未做） |
| P5 | Consecration 真元注入 + 真元色染色 | 道器跑通（灵锋全流程） ✅ server（注入事件 + 染色判定；UI 未做） |
| P6 | 残缺匹配 + side_effect_pool + LifeRecord | 缺料能走 fallback，试错史可见 ✅ server |

---

## §6 跨 plan 钩子

- [ ] **plan-combat-v1 §5**：ForgeWeapon 原钩子被本 plan 实装；攻击时读取武器 `quality / color / side_effects`
- [ ] **plan-inventory-v1**：
  - 武器 item 扩展字段：`quality: f32` / `color: Option<ColorKind>` / `side_effects: Vec<Tag>` / `tier: u8` / `durability`
  - **图谱残卷 item**（1×2，类比丹方残卷，携带 `blueprint_id`）
  - **铭文残卷 item**（1×1，携带 `inscription_id`）
  - 载体材料：`ling_wood` / `yi_beast_bone` / `xuan_iron` / `qing_steel` 等（测试 placeholder）
- [ ] **plan-botany-v1**（待立）：灵木采集
- [ ] **plan-npc-ai-v1**：散修 NPC 使用武器时读 quality/color；worldview §九 "游商傀儡"未来可作为 NPC 铁匠
- [ ] **plan-HUD-v1**：武器 tooltip 扩展（quality / color / side_effects 显示）见 `weapon-treasure.svg`
- [x] **plan-cultivation-v1**：开光步骤要求 `min_realm`（通灵+）才能打道器

---

## §7 TODO / 开放问题（v2+）

- [ ] **防具 / 护体**
- [ ] **法器**（飞剑 / 灵幡，可注真元远程驱动）
- [ ] **暗器**（client 已有 `ForgeWeaponScreen` 骨架，但本 plan 重做）
- [ ] **修复系统**（崩口武器的修缮流程）
- [ ] **熔毁回收**（废品武器回炉取部分材料）
- [ ] **铭文自创**（玩家编辑铭文内容，非模板，长远方向）
- [ ] **NPC 铁匠 / 游商傀儡**（worldview §九 钩子）
- [ ] **传承武器**：高境界玩家死透后，武器进亡者博物馆，可被后来者获取（worldview §十二 "道统遗物"）
- [ ] **真元色衰减**：武器被异色持有者长期使用，原主染色是否缓慢褪去？

---

## §8 风险与对策

| 风险 | 对策 |
|---|---|
| 锻打节奏 QTE 变成"手速壁垒"劝退非硬核玩家 | 容差 `miss_allowed` 宽松；单次 30-60s 不拖长；提供慢速模式（未来）|
| 武器数值膨胀 → power creep | 品阶上限硬约束（凡铁→法器封顶）；高阶需稀缺载体；`quality` 线性而非指数 |
| 开光染色与 combat 真元色系统耦合复杂 | color 只加小幅系数（同色 +10-20% / 异色 0.7x），不叠其他机制 |
| 残缺匹配变"试造赌博" | `LifeRecord.forge_attempts` 公开到亡者博物馆；waste 成本高（材料全失） |
| 锻打节奏轨道客户端/服务端同步 | 服务器权威 tick；客户端只负责击键上报，命中判定在服务器 |

---

## §9 进度日志

- 2026-04-25：P0 落地确认（server forge/ 2379 行 + 3 份测试 blueprint）。已实装范围：BlueprintRegistry JSON 加载、ForgeSessions 总表、四步状态机（Billet/Tempering/Inscription/Consecration）、纯函数解析层（resolve_billet / apply_tempering_hit / resolve_tempering / apply_scroll / resolve_inscription / inject_qi / resolve_consecration）、bucket 汇总、flawed_fallback + side_effect_pool 抽取、ForgeHistory（亡者博物馆前身）、skill_hook（Lv 加成 XP 桥）、station tier 校验 + integrity 损耗。**未落地**：BlockEntity 持久化、IPC schema/Channel（`bong:forge/*`）、客户端 UI（节奏轨道、铭文槽、真元注入、塔科夫背包面板复用）、装备 item 数据契约（quality / color / side_effects 字段，依赖 inventory plan）。

## Finish Evidence

### 落地清单

| Phase | 内容 | 文件路径 |
|---|---|---|
| P0 | BlueprintRegistry + JSON 加载 | `server/src/forge/blueprint.rs` (已存在) |
| P0 | IPC Schema（双端） | `server/src/schema/forge.rs` (新增) · `agent/packages/schema/src/forge.ts` (新增) |
| P0 | Channel 常量 | `server/src/schema/channels.rs` + `agent/packages/schema/src/channels.ts` |
| P0 | ClientRequest 变体 | `server/src/schema/client_request.rs` + `agent/packages/schema/src/client-request.ts` |
| P0 | ServerData 变体 | `server/src/schema/server_data.rs` + `agent/packages/schema/src/server-data.ts` |
| P0 | JSON Schema 生成 | `agent/packages/schema/generated/*.json`（+24 份 forge） |
| P1 | 客户端 forge stores | `client/.../forge/state/{ForgeStation,ForgeSession,ForgeOutcome,BlueprintScroll}Store.java` |
| P1 | 客户端 forge handlers | `client/.../network/forge/{ForgeStation,ForgeSession,ForgeOutcome,ForgeBlueprintBook}Handler.java` |
| P1 | 客户端 forge UI 占位 | `client/.../forge/{ForgeScreen,ForgeScreenBootstrap}.java` |
| P1 | 服务端 forge snapshot emit | `server/src/network/forge_snapshot_emit.rs` |
| P2-P6 | 服务端核心逻辑（已存在） | `server/src/forge/{session,steps,events,history,fallback,skill_hook,learned,station,mod}.rs` |

### 关键 commit

| Hash | 日期 | 消息 |
|---|---|---|
| `510a9330` | 2026-04-27 | feat(forge-v1): P1 client 端 forge stores + handlers + 基础 UI 占位 |
| `9e5590e3` | 2026-04-27 | feat(forge-v1): P0 IPC Schema — 炼器（武器）双端数据契约 |

### 测试结果

- **server**: `cargo test` → **1456 passed**, 0 failed
- **client**: `./gradlew test build` → **BUILD SUCCESSFUL**
- **agent/schema**: `npm test` → **137 passed**, 0 failed (6 test files)

### 跨仓库核验

| 层 | Symbol |
|---|---|
| server | `server/src/schema/forge.rs` — `WeaponForgeStationDataV1` / `ForgeSessionDataV1` / `ForgeOutcomeDataV1` / `ForgeBlueprintBookDataV1` |
| server | `server/src/network/forge_snapshot_emit.rs` — `emit_join_forge_snapshots` / `send_forge_outcome_to_player` |
| agent | `agent/packages/schema/src/forge.ts` — `ForgeStep` / `TemperBeat` / `ForgeOutcomeBucket` / `ForgeStepState` / `WeaponForgeStationDataV1` / `ForgeSessionDataV1` / `ForgeOutcomeDataV1` / `ForgeBlueprintBookDataV1` |
| agent | channels — `CHANNELS.FORGE_START` (`bong:forge/start`) · `CHANNELS.FORGE_OUTCOME` (`bong:forge/outcome`) |
| client | `ForgeStationStore` / `ForgeSessionStore` / `ForgeOutcomeStore` / `BlueprintScrollStore` |
| client | `ServerDataRouter` 注册 `forge_station` / `forge_session` / `forge_outcome` / `forge_blueprint_book` |

### 遗留 / 后续

- **BlockEntity 持久化**（plan §1.3）：alchemy 同栈也未实现，待 inventory plan 落地区块 entity 注册机制后一并处理
- **锻炉方块放置**（plan §1.2）：需 `ForgeStationPlace` 请求 handler + 放置校验（依赖 inventory plan 的锻造砧类 item）
- **客户端节奏轨道 UI**（plan §3.3 Tempering 视图）：锻打节奏滚动条 + J/K/L 键映射，待 UI 专项
- **客户端铭文槽 / 真元注入条 UI**（plan §3.3 Inscription/Consecration 视图）：同上
- **装备 item 数据契约**（plan §6 plan-inventory-v1 钩子）：`quality` / `color` / `side_effects` / `tier` / `durability` 字段依赖 inventory plan 落地
- **图谱残卷 / 铭文残卷 item**（plan §6 plan-inventory-v1 钩子）：同上
- **combat 武器钩子**（plan §6 plan-combat-v1 §5）：攻击时读取武器 quality/color/side_effects
- **`bong:forge/start` / `bong:forge/outcome` Redis 推送**：channel 常量已定义，实际 publish 逻辑待 forge 系统接入 agent bridge 后进行
- **载材材料 item**（plan §6 plan-botany-v1 钩子）：灵木/异兽骨等 placeholder 待 botany plan
