# Bong · plan-spirit-eye-v1

**灵眼系统**。灵眼是末法残土**顶级情报资产**——凝脉→固元突破必需（灵气浓度 > 0.8，灵眼最佳）；坐标随天道变化，稀有不固定。当前代码仅占位（`botany/registry.rs:430` 注释"灵眼未实装 → MVP 禁用生成"）。本 plan 补全坐标注册 / worldgen 生成 / 突破条件接入 / 天道动态迁移 / 信息经济（坐标 = 商品 = 死亡遗念）全链路。

**Primary Axis**（worldview §九/§十 已正典）：**情报稀缺资产的探索-独占-博弈循环**——灵眼形成 → 被发现 → 私有/交易 → 被使用 → 天道压制迁移的循环节奏与稀缺度

## 阶段总览

| 阶段 | 状态 | 验收 |
|---|---|---|
| **P0** 数据结构 + worldgen 候选点 + server 启动初始化 + 按 zone 数量决定灵眼数（Q120: D）| ⬜ | — |
| **P1** 神识感知发现机制（proximity + perception check）+ 私有 narration | ⬜ | — |
| **P2** 凝脉→固元突破环境检查接入（灵眼内 / >0.8 浓度）+ 失败反馈 | ⬜ | — |
| **P3** 天道动态迁移（usage_pressure + 72h 周期偏移）（Q126: A）| ⬜ | — |
| **P4** 信息经济（坐标交易 + 死亡遗念记录 + 声名联动）（Q125: A）| ⬜ | — |
| P5 v1 收口（饱和 testing + agent narration 对齐） | ⬜ | — |

> **vN+1 (plan-spirit-eye-v2)**：玩家人造灵眼（聚灵阵长期运行 + 天道压制加倍）（Q121: D）/ lingtian plot_qi_cap 影响（Q123: C）/ 灵眼形态多样化 / 灵眼"传承"（玩家挖空一个灵眼后该地永久变贫）

---

**世界观锚点**：`worldview.md §三 修炼体系 / 凝脉→固元突破条件`（"12 正经全通 + 浓度 > 0.8 灵气环境静坐凝核，灵眼最佳"）· `worldview.md §十 资源与匮乏 / 资源表`（"灵眼坐标：探索发现，位置随天道变化，极稀，凝脉→固元突破必需"）· `worldview.md §九 经济与交易 / 顶级资产`（"灵眼坐标、安全闭关点、验证过的死亡遗念——情报换命"）· `worldview.md §八 天道行为准则 / 温和手段`（天道重分配灵气 + 驱散强者聚集——本 plan P3 动态迁移机制是此条延伸）· `worldview.md §十三 世界地理 / 血谷`（地图条目"灵眼（不固定）"）

**library 锚点**：`docs/library/cultivation/cultivation-0002 烬灰子内观笔记.json`（凝核体验 / 高浓度灵气静坐物理描述）· 待写 `geography-XXXX 灵眼踏查记`（探险者灵眼发现体验，anchor worldview §十 + §七 + §九，强调"你发现了灵眼，别人不知道——这就是你的优势"）

**交叉引用**：
- `plan-cultivation-v1` ✅（已落地）— 凝脉→固元突破条件接入；已有 `breakthrough` fn，需加灵眼邻近检测
- `plan-worldgen-v3.1` ✅（已落地）— 灵眼位置由 worldgen blueprint 候选点派生；需新增 `spirit_eye_candidates` raster channel
- `plan-botany-v1` ✅（已落地）— `botany/registry.rs:430` 有"灵眼未实装"占位，本 plan 接替
- `plan-perception-v1.1` ✅（已落地）— 神识感知检测灵眼，感知力 >= 阈值才能"发现"灵眼（远距离）
- `plan-narrative-v1` ⏳（部分实装）— 天道叙事触发：灵眼迁移广播 + 私有发现 narration（暗语式，不直接"你发现了灵眼"）
- `plan-tribulation-v1` ⏳（部分实装）— 天劫发生时灵眼附近灵气波动是否触发灵眼迁移
- `plan-lingtian-v1` ✅（部分实装）— 灵眼区 plot_qi_cap 修饰留 vN+1（Q123: C stub）
- `plan-social-v1` ✅（已落地）— 灵眼坐标交易走死信箱（plan-social §交易 ✅）
- `plan-identity-v1` ⬜（**未立 plan**，DEF 之一）— 信誉度 / 声名联动（"信使/向导"标签）vN+1 接入；v1 stub
- `plan-zhenfa-v1` 🟡（active P0/P1）— 聚灵阵 vN+1 才做（Q121: D），玩家人造灵眼留 plan-spirit-eye-v2

**Hotbar 接入声明**（2026-05-03 user 正典化"所有技能走 hotbar"）：灵眼系统**无主动技能 cast**——发现走 perception 自动检测 + proximity，突破走 cultivation 已有 breakthrough fn。无 hotbar 绑定需求。

## 接入面 checklist（防孤岛 — 严格按 docs/CLAUDE.md §二）

- **进料**：worldgen `spirit_eye_candidates` raster channel（新）→ server 启动时按 zone 数量初始化 N 个灵眼 → `Cultivation.realm` 校验是否为凝脉期 → `zone.spirit_qi` 读取局部灵气浓度 → `perception::SenseEntryV1` 检查感知力是否达阈值
- **出料**：`SpiritEyeRegistry` Resource → `BreakthroughResult::EnvInsufficient` 拒绝条件 → `bong:spirit_eye/migrate` Redis channel（agent 订阅）→ `bong:spirit_eye/discovered` (outbound, 私有；仅该玩家可见 HUD 标记)→ `DeathInsight` payload 加"已知灵眼坐标"字段
- **共享类型 / event**：复用 `Cultivation` / `Realm` / `Zone` / `SenseEntryV1` / `DeathInsight`；新增 `SpiritEye` struct / `SpiritEyeId` / `SpiritEyeRegistry` Resource / `SpiritEyeMigrateV1` schema / `SpiritEyeDiscoveredEvent`
- **跨仓库契约**：
  - server: `world::spirit_eye::SpiritEyeRegistry` / `world::spirit_eye::SpiritEye` / `world::spirit_eye::spirit_eye_discovery_tick` / `world::spirit_eye::spirit_eye_migration_tick` / `cultivation::breakthrough::attempt_breakthrough_guyuan` 扩展 / `BreakthroughResult::EnvInsufficient` variant / `world::spirit_eye::xueguai_eye_unstable_init`（血谷灵眼特殊处理 Q124: C）
  - worldgen: `worldgen/scripts/terrain_gen/fields.py` LAYER_REGISTRY 新增 `spirit_eye_candidates` channel / `worldgen/scripts/terrain_gen/spirit_eye_selector.py`（候选区筛选算法）
  - schema: `agent/packages/schema/src/spirit_eye.ts` → `SpiritEyeMigrateV1` / `SpiritEyeDiscoveredV1` (private) / `SpiritEyeUsedForBreakthroughV1`
  - client: HUD 私有标记（仅 discovered_by 名单内的玩家看见）；`bong:spirit_eye/private_marker` outbound payload；perception module 已有 SenseEntry 渲染路径复用

---

## §A 概览（设计导航）

> 灵眼是**顶级情报资产**——稀有 + 私有发现 + 可交易 + 天道动态迁移。v1 实装全 P0-P4（5 个阶段都纳入 v1，Q125+Q126: A）+ P5 收口。**关键设计**：按 zone 数量决定灵眼数（Q120: D，每个非负 zone 至少 1 个候选）+ 不加密存储（Q122: B 简单可调试）+ 血谷高风险高奖励（Q124: C）+ 玩家人造灵眼留 v2（Q121: D）+ lingtian 影响留 vN+1（Q123: C）。

### A.0 v1 实装范围（2026-05-03 拍板）

| 维度 | v1 实装 | 搁置 vN+1 |
|---|---|---|
| 灵眼数量公式 | **按 zone 数量决定**（每个非负 zone 至少 1 个候选灵眼，由迁移机制激活子集）（Q120: D）| `max(3, player_count/10)` 等公式 |
| 候选区筛选 | worldgen `spirit_eye_candidates` raster channel + 地形/灵气浓度规则 | 玩家偏好驱动选址 |
| 发现机制 | **proximity 自动 + perception 远距离检测**（神识阈值）| 探测器物品 / 仪器辅助 |
| 信息加密 | **不加密**（Q122: B，明文 `discovered_by` Vec，简单可调试 + 信任 server）| 加密 / client-side 自存 |
| 突破环境检查 | 灵眼内 / spirit_qi >= 0.8 才允许 | 多档阈值 / 部分通过 |
| 灵眼内凝核加成 | 失败率 -30% | 成功率分级 / 染色加成 |
| 天道动态迁移 | **完整实装**（usage_pressure + 72h 周期偏移）（Q126: A）| 复杂迁移规则 / 联动天劫 |
| 信息经济 | **完整纳入 v1**（坐标交易死信箱 + 死亡遗念记录 + 声名联动 stub）（Q125: A）| plan-identity-v1 完整信誉度 |
| 血谷灵眼 | **高风险高奖励**（凝核加成 +50% + 凝核时天道注意力加倍 → 异变兽触发率高）（Q124: C）| 多种特殊灵眼 |
| 玩家人造灵眼 | **不实装**（Q121: D）| plan-spirit-eye-v2 + 聚灵阵 vN+1 |
| lingtian plot_qi_cap 影响 | **stub**（Q123: C，等 plan-lingtian-process-v1 落地一起调）| 完整 PlotEnvironment 接入 |

### A.1 跨 plan 接入面

| 接入对象 | 关系 | v1 实装 |
|---|---|---|
| plan-cultivation-v1 | 突破条件加灵眼检查 | ✅ P2 实装 |
| plan-worldgen-v3.1 | 候选区 raster channel | ✅ P0 实装 |
| plan-perception-v1.1 | 神识感知发现 | ✅ P1 实装 |
| plan-narrative-v1 | 私有 narration + 迁移广播 | ✅ P1/P3 实装（5 句基准对齐 spawn-tutorial 风格库）|
| plan-social-v1 | 死信箱坐标交易 | ✅ P4 实装 |
| plan-identity-v1 | 声名联动（信使/向导标签）| ⚠️ P4 stub（vN+1 完整接入）|
| plan-tribulation-v1 | 天劫附近灵气波动 → 迁移触发 | ❌ vN+1（v1 仅 usage_pressure + 72h）|
| plan-lingtian-v1 | plot_qi_cap 灵眼影响 | ❌ vN+1 stub（Q123: C）|
| plan-zhenfa-v1 聚灵阵 | 玩家人造灵眼 | ❌ vN+1（Q121: D）|

### A.2 v1 实施阶梯

```
P0  数据结构 + worldgen + server init
       SpiritEyeRegistry Resource + SpiritEye struct
       worldgen spirit_eye_candidates raster channel
       spirit_eye_selector.py 候选区筛选
       server 启动初始化（按 zone 数量 Q120: D）
       血谷灵眼特殊处理（位置不固定 + 服务器重启换坐标 Q124 part）
       ↓
P1  神识感知发现 + 私有 narration
       proximity 自动发现（无需神识，低境兜底）
       neural perception check 远距离发现（最远 50 格）
       SenseEntryV1 私有渲染（仅 discovered_by 见 HUD 标记）
       agent 私有 narration（暗语式："此间有什么凝聚着，说不清的稠"）
       ↓
P2  凝脉→固元突破环境检查接入
       attempt_breakthrough_guyuan 扩展 env_ok check
       BreakthroughResult::EnvInsufficient variant
       灵眼内凝核失败率 -30%
       血谷灵眼凝核 +50% + 异变兽触发率加倍（Q124: C）
       失败反馈暗语 narration
       ↓
P3  天道动态迁移
       usage_pressure 累积 + 衰减 tick
       迁移触发（usage_pressure >= 1.0）→ 新坐标在候选区随机
       72h real-time 周期兜底偏移（防永久固定化）
       agent narration 暗语广播（"东方某处天地凝气散了几分"）
       ↓
P4  信息经济
       坐标交易接入 plan-social 死信箱（私有商品）
       DeathInsight payload 加"已知灵眼坐标"字段
       声名标签 stub（"信使/向导" tag emit；vN+1 plan-identity-v1 完整反应）
       ↓ 饱和 testing
P5  v1 收口
       数值平衡 + LifeRecord "X 在 N 时刻在灵眼 Y 完成凝脉→固元突破"
```

### A.3 v1 已知偏离正典（vN+1 必须修复）

- [ ] **plan-identity-v1 信誉度系统未立** — Q125 P4 信誉度联动仅 stub（"信使/向导"标签），vN+1 plan-identity-v1 完整信誉度反应
- [ ] **plan-zhenfa-v1 聚灵阵 vN+1 才做** — Q121 玩家人造灵眼留 plan-spirit-eye-v2
- [ ] **plan-lingtian-process-v1 ⏳** — Q123 lingtian plot_qi_cap 影响留 vN+1 stub
- [ ] **plan-tribulation-v1 部分实装** — 天劫附近灵气波动触发灵眼迁移留 vN+1（v1 仅 usage_pressure + 72h）
- [ ] **加密存储**（Q122 选 B 不加密）— vN+1 若发现 server admin 滥用偷坐标问题再补加密层

### A.4 v1 关键开放问题

**已闭合**（Q120-Q126，7 个决策 + skeleton §10 全闭合）：
- Q120 → D 按 zone 数量决定灵眼数（每个非负 zone 至少 1 个候选）
- Q121 → D 玩家人造灵眼留 plan-spirit-eye-v2
- Q122 → B 不加密（明文 discovered_by Vec，简单可调试）
- Q123 → C lingtian plot_qi_cap 影响 vN+1 stub
- Q124 → C 血谷灵眼高风险高奖励（+50% 凝核加成 + 异变兽触发率加倍）
- Q125 → A v1 完整纳入 P4 信息经济
- Q126 → A v1 完整纳入 P3 天道动态迁移

**仍 open**（v1 实施时拍板）：
- [ ] **Q127. usage_pressure 衰减率 / 触发阈值**：每次玩家凝核 +0.1 / 每天衰减 0.05 / 阈值 1.0 起手；P3 实装时按运营数据调
- [ ] **Q128. 血谷灵眼"异变兽触发率加倍"具体值**：触发冷却 / 数量 / 等级；P2 实装时按 plan-fauna ✅ 已有数据微调
- [ ] **Q129. 神识感知阈值具体值**：远距离发现的 perception 力阈值具体数；P1 实装时按 plan-perception 已落地数据调
- [ ] **Q130. 灵眼内 narration 风格基准**：v1 起手 "此间有什么凝聚着，说不清的稠" / "东方某处天地凝气散了几分"；P1 实装时与 plan-narrative-v1 协调（对齐 spawn-tutorial 5 句基准的"半文言半白话 + 冷漠古意 + 禁现代腔"标尺）

---

## §0 设计轴心

- [ ] **稀缺性**：全服同时存在灵眼数量极少（初期 3–5 个），分布在不同 zone
- [ ] **发现 ≠ 广播**：玩家发现灵眼，server 不广播坐标——发现是私有信息，需要玩家自己决定是否共享（匿名系统延续）
- [ ] **天道不偏袒**：灵眼位置随天道变化（worldview §十）——被大量玩家利用的灵眼会被天道"注意到"并逐渐迁移
- [ ] **不是永久安全点**：灵眼只提供凝核环境（灵气浓度加成），不是灵龛；高浓度反而吸引野兽和其他修士

---

## §1 第一性原理（烬灰子四论挂点）

- **缚论·凝核**：凝脉→固元突破需要把 12 条正经的流动模式"缚定"成一个稳定核——高浓度灵气环境让缚定过程所需的外部真元压差更容易维持
- **影论·灵眼投影**：灵眼本质上是天地灵气在特定地形构型下形成的"自发性镜印聚焦点"——如同凸透镜聚光，灵气在此处天然高度（浓度 > 0.8）
- **音论·天道察觉**：修士在灵眼处持续凝核 → 局部高灵气消耗 → 天道感知到"音的聚集" → 触发压制（灵气归零 / 异变兽增生）——这是"被大量利用后迁移"的物理根因
- **噬论·灵眼衰竭**：灵眼不是取之不尽的——高密度使用后局部灵气被噬散加速，灵眼强度逐渐下降直到迁移

---

## §2 灵眼数据结构

```rust
/// 全服灵眼注册表（Resource）
pub struct SpiritEyeRegistry {
    pub eyes: Vec<SpiritEye>,
}

pub struct SpiritEye {
    pub id: SpiritEyeId,
    pub pos: (i64, i64),        // 世界坐标（中心点）
    pub radius: u32,            // 有效范围（默认 20 格）
    pub qi_concentration: f32,  // 当前浓度（0.8 ~ 1.2，基线 1.0）
    pub discovered_by: Vec<CharId>,   // 已发现的玩家列表（私有信息）
    pub usage_pressure: f32,    // 使用压力累积（触发迁移阈值）
    pub last_migrate_tick: u64,
}
```

- [ ] 全服初始灵眼 N = `max(3, player_count / 10)` 个（上限 8）
- [ ] `qi_concentration` 影响凝核成功率：1.0 基线，高使用后下降
- [ ] `usage_pressure` 每次有玩家在此凝核 → +0.1；每天自然衰减 0.05；达到 1.0 触发迁移

---

## §3 worldgen 候选点

> 灵眼不是完全随机——出现在满足"特殊地形构型"的候选区域。

- [ ] **候选区筛选规则**（worldgen pipeline 计算，存入 raster channel `spirit_eye_candidates`）：
  - 地表 Y 高度处于 [80, 200]（高地）or 地下洞穴内
  - 周围 50 格内无其他灵眼候选点
  - 地形 feature_scale 在特定窗口（地形变化丰富但不过于险峻）
  - 与 worldgen zone 的 spirit_qi 基线相关：`ling_quan_marsh` / `qingyun_peaks` 候选密度最高；`north_wastes` 极低
- [ ] **血谷灵眼**（worldview §七）：血谷灵眼"不固定"——每次服务器重启在血谷候选区内随机选一个点，不保持跨重启稳定（高风险区域的高奖励设计）
- [ ] **raster 新增 channel**：`spirit_eye_candidates`（uint8 mask，候选点 = 1，其余 = 0）

---

## §4 P1 — 神识感知发现机制

> 玩家不能直接"看到"灵眼——需要通过神识感知（plan-perception-v1）或接近触发。

- [ ] **发现触发条件**（任一）：
  - 玩家进入灵眼 `radius` 范围内（自动发现，无需神识；给低境界玩家的兜底路径）
  - 神识感知检测 + 感知力 >= 阈值（可在更远距离发现，最远 50 格外）
- [ ] **发现效果**：
  - `SpiritEye.discovered_by.push(char_id)`（私有，不广播）
  - 客户端个人 narration（天道叙事暗语式）："此间有什么凝聚着，说不清的稠。"（worldview 叙事风格，不写"恭喜！你发现了灵眼！"）
  - HUD 显示个人标记（仅该玩家可见）
- [ ] **地图坐标**：发现后进入玩家私有笔记（plan-social §情报系统预留），可向其他玩家出售

---

## §5 P2 — 凝脉→固元突破条件接入

> plan-cultivation-v1 `breakthrough` fn 已有境界门槛校验，需加灵眼/高浓度环境前置。

- [ ] **前置条件检查**（在 `attempt_breakthrough_guyuan` 中新增）：
  ```rust
  let qi_at_pos = zone_qi_at(player_pos);
  let in_spirit_eye = spirit_eye_registry.eye_at(player_pos).is_some();
  let env_ok = qi_at_pos >= 0.8 || in_spirit_eye;
  if !env_ok { return BreakthroughResult::EnvInsufficient; }
  ```
- [ ] **灵眼加成**：灵眼内凝核 → 失败率 -30%（相比仅 0.8 浓度场景），成功后触发轻微全服 narration（天道感知到某处凝核，暗语式，不带坐标）
- [ ] **失败反馈**：`EnvInsufficient` → 客户端提示（天道叙事风格）："此地灵气稀薄，你的内力涣散如沙……" 不告知玩家需要找灵眼
- [ ] **tests**：灵眼内凝核 → success 概率上升；非灵眼低灵气（< 0.8）→ `EnvInsufficient` 拒绝；馈赠区峰值（0.9+）非灵眼 → 允许（只是更难）

---

## §6 P3 — 天道动态迁移

- [ ] **迁移触发**：`SpiritEye.usage_pressure >= 1.0` → 天道叙事（全服，暗语："东方某处天地凝气散了几分。"）→ 灵眼在候选区内随机迁移到新坐标（旧坐标 `discovered_by` 清空）
- [ ] **迁移逻辑**：新坐标必须离旧坐标 >= 500 格（防止就近刷新）+ 仍在候选区内
- [ ] **周期兜底**：即便无高压力，每 72h real-time 至少一次小幅偏移（±50 格）——防止灵眼永久固定化
- [ ] **天道 agent 集成**：迁移事件发 `bong:spirit_eye/migrate` → tiandao 可以感知并编入叙事
- [ ] **tests**：usage_pressure 到阈值 → 迁移 + 旧 discovered_by 清空；新坐标在候选区内；迁移 narration 发出

---

## §7 P4 — 信息经济接入

> "灵眼坐标是顶级资产——情报换命"（worldview §九）。

- [ ] **死亡遗念记录**：玩家死亡时，若 `discovered_by` 包含该玩家 → `DeathInsight` payload 加入"已知灵眼坐标"字段（agent 使用）
- [ ] **坐标可交易**：灵眼坐标可作为"情报商品"存入盲盒死信箱（plan-social §交易），以骨币换取——server 侧只需保证"玩家私有笔记"中灵眼坐标的数据结构（具体交易 UI 归 plan-social）
- [ ] **声名联动**（plan-social §4）：频繁分享灵眼坐标 → 积累"信使/向导"声名标签；独占灵眼 → 隐性声名（玩家传说层面）

---

## §8 数据契约（下游 grep 抓手）

| 契约 | 位置 |
|---|------|
| `SpiritEyeRegistry` Resource + `SpiritEye` struct | `server/src/world/spirit_eye.rs`（新文件）|
| `SpiritEyeId` + `discovered_by: Vec<CharId>` | `server/src/world/spirit_eye.rs` |
| `bong:spirit_eye/migrate` channel | `server/src/schema/channels.rs` |
| raster channel `spirit_eye_candidates` | `worldgen/scripts/terrain_gen/fields.py`（LAYER_REGISTRY）|
| `attempt_breakthrough_guyuan` env check | `server/src/cultivation/breakthrough.rs` |
| `BreakthroughResult::EnvInsufficient` variant | `server/src/cultivation/breakthrough.rs` |
| `SpiritEyeMigrateV1` schema | `server/src/schema/spirit_eye.rs` + `agent/packages/schema/src/` |

---

## §9 实施节点

- [ ] **P0**：`SpiritEyeRegistry` + `SpiritEye` struct + worldgen 候选区筛选规则（blueprint 集成）+ server 启动初始化 N 个灵眼 + 单测（注册 / 范围检测 / usage_pressure 累积）
- [ ] **P1**：神识感知发现（proximity 自动 + perception check）+ 私有 narration + `discovered_by` 记录 + HUD 私有标记
- [ ] **P2**：`attempt_breakthrough_guyuan` env check 接入 + 失败反馈 + 灵眼内凝核概率加成 + e2e 测（灵眼内成功率上升）
- [ ] **P3**：usage_pressure 迁移 + 72h 周期偏移 + 全服叙事 narration + `bong:spirit_eye/migrate` 发布 + tiandao 感知
- [ ] **P4**：死亡遗念记录灵眼坐标 + 坐标可交易数据结构 + 声名联动 stub

---

## §10 开放问题

### 已闭合（2026-05-03 拍板，7 个决策）

- [x] **Q120** → D 按 zone 数量决定灵眼数（skeleton §10 #1 灵眼数量公式 deprecated）
- [x] **Q121** → D 玩家人造灵眼留 plan-spirit-eye-v2（skeleton §10 #2 闭合）
- [x] **Q122** → B 不加密（skeleton §10 #3 闭合）
- [x] **Q123** → C lingtian 影响 vN+1 stub（skeleton §10 #4 闭合）
- [x] **Q124** → C 血谷灵眼高风险高奖励（skeleton §10 #5 闭合）
- [x] **Q125** → A v1 完整纳入 P4 信息经济
- [x] **Q126** → A v1 完整纳入 P3 天道动态迁移

### 仍 open（v1 实施时拍板）

- [ ] **Q127. usage_pressure 衰减率 / 触发阈值**：每次凝核 +0.1 / 每天衰减 0.05 / 阈值 1.0 起手；P3 实装时按运营数据调
- [ ] **Q128. 血谷灵眼"异变兽触发率加倍"具体值**：触发冷却 / 数量 / 等级；P2 实装时按 plan-fauna ✅ 已有数据微调
- [ ] **Q129. 神识感知阈值具体值**：远距离发现的 perception 力阈值具体数；P1 实装时按 plan-perception 已落地数据调
- [ ] **Q130. 灵眼 narration 风格基准对齐**：v1 起手 "此间有什么凝聚着，说不清的稠" / "东方某处天地凝气散了几分"；P1 实装时与 plan-narrative-v1 协调（对齐 spawn-tutorial 5 句基准的"半文言半白话 + 冷漠古意 + 禁现代腔"标尺）

### vN+1 留待问题（plan-spirit-eye-v2 时拍）

- [ ] **玩家人造灵眼**（聚灵阵长期运行 → 局部灵气持续 > 1.0 → 形成临时灵眼 + 天道压制加倍）—— 接 plan-zhenfa-v1 聚灵阵 vN+1
- [ ] **lingtian plot_qi_cap 影响**（灵眼 50 格内 plot_qi_cap +0.2）—— 接 plan-lingtian-process-v1
- [ ] **天劫附近灵气波动 → 灵眼迁移触发**（plan-tribulation-v1 联动）
- [ ] **完整信誉度反应**（plan-identity-v1 接入"信使/向导"标签触发的 NPC 反应）
- [ ] **加密存储**（若发现 server admin 滥用问题再补）
- [ ] **灵眼形态多样化**（不同地形 / 不同 qi_color 的特殊灵眼）
- [ ] **灵眼"传承"**（玩家挖空一个灵眼后该地永久变贫；接 worldview "天道不偏袒"思路深化）

---

## §11 进度日志

- **2026-04-27**：骨架立项。来源：`docs/plans-skeleton/reminder.md` "通用/跨 plan"节（"灵眼结构未实装"）+ worldview §三/§十/§九 灵眼设计锚点。`server/src/botany/registry.rs:430` 有"灵眼未实装 → MVP 禁用生成（EventTriggered 占位，永不 spawn）"注释，是现有唯一代码痕迹。worldgen 无 `spirit_eye_candidates` raster channel。cultivation 突破流程无灵眼环境检测。
- **2026-05-03**：**plan 文档规范化升级**（去骨架化 + 加 Primary Axis + §A 概览 + 7 决策闭环 Q120-Q126）。primary axis = **情报稀缺资产的探索-独占-博弈循环**（worldview §九/§十 锚定）。**v1 完整范围**（P0-P5 五阶段全做）：数据结构 + 发现 + 突破接入 + 动态迁移 + 信息经济 + 收口。**关键设计**：按 zone 数量决定灵眼数（Q120 D 取代旧公式）+ 不加密（Q122 B）+ 血谷高风险高奖励（Q124 C）+ 玩家人造灵眼留 v2（Q121 D）+ lingtian 影响留 vN+1 stub（Q123 C）。**Hotbar 接入声明**：灵眼无主动技能 cast，无 hotbar 绑定需求。下一个候选：plan-fauna-v1 已 ✅ finished（journey plan 状态待同步）/ plan-style-pick-v1 ⬜ 派生（P2 流派分化必需）/ plan-identity-v1 ⬜ 派生（DEF 之一）。

## Finish Evidence

### 落地清单

- **P0 数据结构 / worldgen / server init**：`server/src/world/spirit_eye.rs` 新增 `SpiritEyeRegistry`、`SpiritEye`、`SpiritEyeId`、候选初始化和血谷高风险候选；`worldgen/scripts/terrain_gen/fields.py` 新增 `spirit_eye_candidates` layer；`worldgen/scripts/terrain_gen/spirit_eye_selector.py` + `broken_peaks.py` / `spring_marsh.py` / `rift_valley.py` 写入候选 mask。
- **P1 神识感知发现 / 私有标记**：`SpiritEyeRegistry::discover_for` 实装 proximity + realm perception 检测；`server/src/cultivation/spiritual_sense/scanner.rs` / `push.rs` 输出私有 `SenseKindV1::SpiritEye`；client `SenseKind.SPIRIT_EYE` + `PerceptionEdgeRenderer` 颜色接入。
- **P2 凝脉→固元环境检查**：`server/src/cultivation/breakthrough.rs` 新增 `MIN_ZONE_QI_TO_GUYUAN`、`BreakthroughError::EnvInsufficient`、灵眼内成功率 bonus、使用后 `usage_pressure` 记录、失败私有 narration；`server/src/cultivation/life_record.rs` 记录 `SpiritEyeBreakthrough`。
- **P3 天道动态迁移**：`SpiritEyeRegistry::tick_migration` 覆盖 `usage_pressure >= 1.0` 与 72h 周期偏移；`server/src/schema/channels.rs` / `server/src/network/redis_bridge.rs` 发布 `bong:spirit_eye/migrate`；`agent/packages/tiandao/src/redis-ipc.ts` 订阅并缓冲灵眼迁移 / 发现 / 使用事件。
- **P4 信息经济 / 死亡遗念 / 声名 stub**：`server/src/combat/lifecycle.rs` 把 `known_spirit_eyes` 写入 `DeathInsightRequestV1`；`server/src/world/spirit_eye.rs` 提供 `coordinate_note_for` DTO stub 与 `spirit_eye_coordinate_share_renown_stub`；`agent/packages/tiandao/src/death-insight-runtime.ts` 在遗念 fallback 中保留已知灵眼坐标。
- **P5 收口 / schema 对齐**：`server/src/schema/spirit_eye.rs` 与 `agent/packages/schema/src/spirit-eye.ts` 对齐 `SpiritEyeMigrateV1` / `SpiritEyeDiscoveredV1` / `SpiritEyeUsedForBreakthroughV1` / `SpiritEyeCoordinateNoteV1` / `DeathInsightSpiritEyeV1`；`agent/packages/schema/generated/*.json` 已重新生成。

### 关键 commit

- `2fc9e15a` · 2026-05-03 · `feat(plan-spirit-eye-v1): 落地灵眼系统链路`
- `871403a0` · 2026-05-03 · `fix(plan-spirit-eye-v1): 收窄灵眼突破加成与私有标记`

### 测试结果

- `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings`：通过。
- `cd server && cargo test`：2117 passed。
- `cd server && cargo test spirit_eye --quiet`：11 passed。
- `cd server && cargo test breakthrough --quiet`：36 passed。
- `cd agent && npm run build && (cd packages/tiandao && npm test) && (cd packages/schema && npm test)`：build 通过；tiandao 210 passed；schema 252 passed。
- `cd client && JAVA_HOME="$HOME/.sdkman/candidates/java/17.0.18-amzn" ./gradlew test build`：通过。
- `cd client && JAVA_HOME="$HOME/.sdkman/candidates/java/17.0.18-amzn" ./gradlew test --tests "com.bong.client.visual.realm_vision.PerceptionEdgeRendererTest"`：通过。
- `cd worldgen && PYTHONPATH="." python3 -m unittest discover -s "tests"`：9 passed。
- `cd worldgen && python3 -m scripts.terrain_gen`：通过，生成 layer 列表包含 `spirit_eye_candidates`。
- `git diff --check`：通过。

### 跨仓库核验

- **server**：`world::spirit_eye::SpiritEyeRegistry`、`spirit_eye_discovery_tick`、`spirit_eye_migration_tick`、`BreakthroughError::EnvInsufficient`、`CH_SPIRIT_EYE_MIGRATE`、`DeathInsightRequestV1.known_spirit_eyes`。
- **agent schema / tiandao**：`SpiritEyeMigrateV1`、`SpiritEyeDiscoveredV1`、`SpiritEyeUsedForBreakthroughV1`、`DeathInsightSpiritEyeV1`、`CHANNELS.SPIRIT_EYE_MIGRATE`、`RedisIpc.onCrossSystemEvent`。
- **client**：`SenseKind.SPIRIT_EYE`、wire mapping `"SpiritEye"`、灵眼 HUD marker 颜色 `0x70FFD6`。
- **worldgen**：`LAYER_REGISTRY["spirit_eye_candidates"]`、`select_spirit_eye_candidates`、三类 terrain profile 输出 `spirit_eye_candidates` layer。

### 遗留 / 后续

- `plan-identity-v1` 尚未立项，P4 声名 / 信誉度只保留事件 stub，不扩展 NPC 完整反应。
- 玩家人造灵眼、灵田 `plot_qi_cap`、天劫触发迁移、加密坐标存储、灵眼形态多样化均按本 plan 约定留给 `plan-spirit-eye-v2` 或对应后续 plan。
