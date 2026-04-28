# Bong · plan-spirit-eye-v1 · 骨架

**灵眼系统**。灵眼是末法残土最顶级的情报资产之一——"凝脉→固元突破必需（灵气浓度 > 0.8，灵眼最佳）"；其坐标随天道变化，稀有且位置不固定。当前代码已有占位（botany/registry.rs 注释"灵眼未实装 → MVP 禁用生成"），worldgen 和 server 均无实体。本 plan 补全灵眼坐标注册、worldgen 生成规则、修炼突破条件接入、天道动态迁移机制，以及信息经济（灵眼坐标作为顶级情报的价值链路）。

**世界观锚点**：`worldview.md §三 凝脉→固元突破条件`（"12 正经全通 + 浓度 > 0.8 灵气环境静坐凝核，灵眼最佳"）· `worldview.md §十 资源表`（"灵眼坐标：探索发现，位置随天道变化，极稀，凝脉→固元突破必需"）· `worldview.md §九 顶级资产`（"灵眼坐标、安全闭关点、验证过的死亡遗念——情报换命"）· `worldview.md §七 天道动态`（天道重分配灵气 + 驱散强者聚集）· `worldview.md §七 negative 区域`（血谷"灵眼(不固定)"）

**library 锚点**：`docs/library/cultivation/cultivation-0002 烬灰子内观笔记.json`（凝核体验 / 高浓度灵气静坐物理描述）· 待写 `geography-XXXX 灵眼踏查记`（探险者灵眼发现体验，anchor worldview §十 + §七 + §九，强调"你发现了灵眼，别人不知道——这就是你的优势"）

**交叉引用**：
- `plan-cultivation-v1`（凝脉→固元突破条件接入；已有 `breakthrough` fn，需加灵眼邻近检测）
- `plan-worldgen-v3.1`（灵眼位置由 worldgen blueprint 候选点派生）
- `plan-botany-v1`（✅；`botany/registry.rs:430` 有"灵眼未实装"占位，本 plan 接替）
- `plan-perception-v1`（神识感知检测灵眼，感知 >= 阈值才能"发现"灵眼）
- `plan-narrative-v1`（天道叙事触发：灵眼迁移广播，暗语式提示不是直接 "你发现了灵眼"）
- `plan-tribulation-v1`（天劫发生时灵眼附近灵气波动是否触发灵眼迁移）
- `plan-lingtian-v1`（灵眼区 plot_qi_cap 修饰：附近灵田可能吸收灵眼溢散 qi）

**阶段总览**：
- P0 ⬜ 数据结构 + worldgen 候选点 + 服务器启动初始化
- P1 ⬜ 神识感知发现机制（player 接近 + perception check → "感知到浓密灵眼"）
- P2 ⬜ 凝脉→固元突破条件接入（需灵眼 20 格内 or 高灵气环境）
- P3 ⬜ 天道动态迁移（周期性坐标偏移 + narration 暗示）
- P4 ⬜ 信息经济接入（灵眼坐标可交易、可存入死亡遗念）

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

- [ ] 灵眼全服数量公式：`max(3, player_count / 10)` 是否合理？少量玩家时 3 个已足够，大服会否因玩家太多导致灵眼争夺过于激烈？
- [ ] 玩家是否可以"人为制造"灵眼（如聚灵阵长期运行 → 局部灵气浓度持续 > 1.0 → 触发灵眼形成）？与 worldview §七 聚灵阵天道阈值悖论的关系？
- [ ] 灵眼发现信息是否加密存储（防止 server admin 直接查玩家笔记获取坐标优势）？
- [ ] 灵眼区域是否影响 lingtian 的 `plot_qi_cap`（附近灵田获得环境加成）？与 `PlotEnvironment` 计算的接入点？
- [ ] 血谷灵眼的特殊属性：高奖励（凝核加成更强？）还是高风险（凝核时天道注意力加倍 → 异变兽触发率更高）？

---

## §11 进度日志

- **2026-04-27**：骨架立项。来源：`docs/plans-skeleton/reminder.md` "通用/跨 plan"节（"灵眼结构未实装"）+ worldview §三/§十/§九 灵眼设计锚点。`server/src/botany/registry.rs:430` 有"灵眼未实装 → MVP 禁用生成（EventTriggered 占位，永不 spawn）"注释，是现有唯一代码痕迹。worldgen 无 `spirit_eye_candidates` raster channel。cultivation 突破流程无灵眼环境检测。
