# 搜打撤（坍缩渊）· plan-tsy-v1

> 把 Bong 的主玩法补齐为「搜打撤」循环——活坍缩渊（TSY）作为世界里的**有限时限副本**，玩家在其中冒真元换遗物。
> 交叉引用：`worldview.md §十六`（秘境：活坍缩渊）· `worldview.md §二`（坍缩渊）· `worldview.md §十`（搜打撤循环）· `plan-inventory-v1.md §-1`（DroppedLoot）· `plan-death-lifecycle-v1.md §7`（LifeRecord）

本 plan 是**方向性 meta**，不含实现细节。实施由 **P-1 + 6 个子 plan** 线性推进：

- `plan-tsy-dimension-v1.md` (**P-1 位面基础设施** — ✅ **PR #47 已合并 2026-04-26**，commit 579fc67e，1252 单测全绿) — Valence `DimensionType` 注册、TSY `LayerBundle`、跨位面传送 API、per-dimension `TerrainProvider`、`CurrentDimension` component、`Zone.dimension` 字段 + `find_zone(dim,pos)` 签名。**所有其他子 plan 的前置已解冻**
- `plan-tsy-zone-v1.md` (**P0 基础**) — TSY zone type、负压抽真元、裂缝入口、入场过滤
- `plan-tsy-loot-v1.md` (**P1 物资与死亡**) — 99/1 遗物分布、秘境内死亡 100% 掉落、干尸化、keepInventory mixin
- `plan-tsy-lifecycle-v1.md` (**P2 塌缩与道伥**) — 遗物骨架、塌缩事件、race-out、道伥生态
- `plan-tsy-container-v1.md` (**P3 容器与搜刮**) — 5 档容器（干尸/骨架/储物袋/石匣/法阵核心）、搜刮倒计时、钥匙/令牌、搜刮时真元 1.5× 加速
- `plan-tsy-hostile-v1.md` (**P4 敌对 NPC**) — 4 类敌对 archetype（道伥/执念/秘境守灵/负压畸变体）、AI tree、起源 × 层深 spawn pool、drop table、Fuya 耗真元光环
- `plan-tsy-extract-v1.md` (**P5 撤离点**) — 3 种 portal（主裂缝/深层缝/塌缩裂口）、撤离倒计时、中断规则、race-out 模式切换、塌缩清场
- `plan-tsy-worldgen-v1.md` (**骨架** — 独立能力，与 P3/P4/P5 可并行) — 地形 / POI / NPC anchor 自动生成，产出两份 manifest（主世界 + TSY dim）

> **2026-04-24 架构反转备忘**：`worldview.md §十六 世界层实现注` 已锁坍缩渊以**独立位面**实现（类 Nether）。原 P0 `§-1 点 5` "传送是同一 MC world 内的坐标传送" 已被推翻，改由 `plan-tsy-dimension-v1` 承载基础设施，P0 / worldgen / lifecycle / extract 的传送 / 锚点 / find_zone 调用均需带位面信息。各子 plan 顶部有连带修订备忘。

---

## §-1 现状

**已就位**（不在本系列 plan 的重做范围）：

| 层 | 能力 | 位置 |
|----|------|------|
| Zone 框架 | name/bounds/spirit_qi/danger_level | `server/src/world/zone.rs:23-31`，`ZoneRegistry` from `zones.json` |
| Player State | 真元池 + 境界 + karma | `server/src/player/state.rs:27-34` |
| 库存 | 多容器 + 装备 + 重量 + spirit_quality | `server/src/inventory/mod.rs:192-200`（`PlayerInventory`）、`135-150`（`ItemInstance`） |
| 掉落物 | `DroppedLootRegistry` + 50% 随机掉落 on revive | `server/src/inventory/mod.rs:1090-1368` |
| 死亡事件 | `DeathEvent { target, cause, at_tick }` | `server/src/combat/events.rs:82-87` |
| 死亡惩罚 | 降一阶 + qi=0 + 关脉 + 虚弱 | `server/src/cultivation/death_hooks.rs:45-75` |
| NPC 框架 | archetype + brain + spawn | `server/src/npc/{brain,faction,lifecycle,spawn,patrol,navigator}.rs` |
| IPC Schema | CombatRealtimeEventV1（已有 `attacker_id?`）+ CultivationDeathV1 | `agent/packages/schema/src/combat-event.ts:31-46` |
| 真元条 HUD | 已有 client-side 渲染（plan-combat-ui_impl 完成） | `client/src/main/java/...` |

**已知缺口**（本系列 plan 要解决）：

- **无独立 Dimension 基础设施**（2026-04-24 架构反转新增缺口）— 无 TSY `DimensionType`、无 TSY `LayerBundle`、无跨位面传送 API (`DimensionTransferRequest`)、无 per-dimension `TerrainProvider` routing、无 `CurrentDimension` component、无 `Zone.dimension` 字段；由 **P-1 `plan-tsy-dimension-v1`** 承担，是 P0 及后续所有子 plan 的硬前置
- 无 TSY zone type / 无负压抽真元机制 / 无入场过滤器 / 无裂缝入口 POI
- 无 "zone-aware" 死亡结算 — `apply_death_drop_on_revive` 不区分主世界 vs 秘境
- 无 Fabric keepInventory mixin — MC 原生掉落可能和自研掉落 double-fire
- `DeathEvent` 无 `attacker_player_id` — 秘境 PVP 掠夺追溯断链
- 无遗物骨架概念 / 无 zone 生命周期 / 无塌缩事件
- 无道伥 NPC archetype — 尽管 `worldview.md §七` 已定义

---

## §0 设计轴心（不可违反）

以下公理来自 `worldview.md §十六`，所有子 plan 必须严格遵守。违反 = 世界观断链。

1. **负压即秒表** — 不挂外部 tick 计时器；压力完全来自 `spirit_qi` 被抽取的速率。玩家看着自己的真元条作决定（§十六.二）
2. **非线性抽取** — 抽取速率与真元池呈非线性关系（`rate ∝ |灵压| × 池^n`，`n ≈ 1.5-2`）。境界越高、池越大，在同一灵压下**绝对**抽得越猛。这保证"深层对低阶友好、对高阶是禁区"的分层悖论（§十六.二）
3. **入场不限境界** — 任何玩家（含醒灵）都能进裂缝。每个境界自行权衡"甜区"与"死区"（§十六.二）
4. **入场过滤**：高灵质物品过关口即散失真元 — 修士只能带凡铁、干灵草、退活骨壳、低灵质杂物进秘境（§十六.四）
5. **99 / 1 loot 分布**：99% 来自**前人遗物**（凡物），1% 是**上古遗物**（高强度 + 低耐久）。每一层都成立这个比例，但 1% 倾向深层（§十六.三）
6. **上古遗物谁都能用**：不认主、不激活、不因换人而失效；唯一代价是**耐久极低**（一到三五次即碎）。低耐久源于**长期在低/负灵压下被淬炼脆化**（§十六.三）
7. **秘境内死亡 ≠ 主世界死亡**：
   - 运数 / 劫数 / 寿元扣除 / 境界降一阶 — 和 §十二 一致
   - **但秘境所得 100% 掉落**（无论凡物还是上古遗物），**身上原带的非秘境物品仍按 §十二 的 50% 规则**
   - 遗骸**干尸化**（死状特别）（§十六.六）
8. **塌缩由玩家行为驱动，非天道定时** — 活坍缩渊塌缩 = 某修士亲手取走最后一件遗物的那一刻。不取走就不塌。这自然产生哄抢压力（§十六.一 生命周期）
9. **Race-out 机制** — 最后一件被取走 → 负压瞬间加倍 → 还没撤出的所有人（含拿走那件的人）要拼命跑到裂缝口（§十六.一 第 4 步）
10. **灵龛失效于秘境内** — 龛石封印阵在深负压下被吞噬，秘境内无安全点（§十六.五 / §十一 补丁）

---

## §1 子 plan 依赖图

```
              ┌─────────────┐
              │ plan-tsy-v1 │  ← 本 meta（本文件）
              │ (overview)  │
              └──────┬──────┘
                     │
              ┌──────▼─────────────────┐
              │ P-1 tsy-dimension      │  ← Valence DimensionType + LayerBundle
              │  （位面基础设施）         │    跨位面传送 API + per-dim TerrainProvider
              └──────┬─────────────────┘    **所有 P0-P5 / worldgen 的共同前置**
                     │
     ┌───────────────┴───────────────┐
     ↓                               ↓ （并行独立轨）
┌───────────┐                ┌────────────────┐
│  P0 zone  │                │  tsy-worldgen  │
└─────┬─────┘                │ （地形 / POI   │
      │                      │   自动生成）    │
      ↓                      └────────────────┘
┌───────────┐
│  P1 loot  │
└─────┬─────┘
      │
      ↓
┌──────────────┐   ← 核心闭环完成（搜打撤骨架 demoable）
│ P2 lifecycle │
└──────┬───────┘
       │
    ┌──┴───────────────────┬──────────────────┐
    ↓                      ↓                  ↓
┌──────────────┐   ┌──────────────┐   ┌─────────────┐
│ P3 container │   │ P4 hostile   │   │ P5 extract  │   ← 玩法层（需 P0-P2 闭环）
└──────────────┘   └──────────────┘   └─────────────┘
    │                   │                    │
    │                   │                    └── 监听 P2 塌缩事件 + 传出玩家
    │                   └── drop 接 P1 ownerless + 道伥 archetype 来自 P2
    └── 读 P2 relics_remaining（发 RelicExtracted 事件给 P2）

P3/P4/P5 之间也有互相依赖：
  P4 Fuya aura × P3 search drain（drain multiplier 叠加）
  P4 NPC drop 钥匙 → P3 容器用
  P5 忙态互斥 P3 搜刮（互相拒绝启动）
```

**消费顺序**：**P-1 必须最先落地**（基础设施，否则 P0 `CurrentDimension` / `DimensionTransferRequest` / `find_zone(dim,pos)` 全部悬空）；之后 P0 → P1 → P2 严格顺序（核心闭环）；P3/P4/P5 之间可灵活排序但都必须在 P2 之后；worldgen 骨架与 P0/P1/P2 无强耦合，可并行推进但升级 active 要求 P-1 落地 + P0 merged + P3/P4/P5 至少一个开工。每个子 plan 独立 demoable，不可跳过前置。

| 阶段 | 依赖 | demoable 终态 |
|------|------|---------------|
| P0 zone | 无（只依赖现有 Zone 系统） | 手动 `/tsy-spawn` 生成一个 TSY zone；玩家走进裂缝 → 传送进 zone；真元被持续抽；带附灵武器进 → 武器变凡铁；走到边界 → 传送出 |
| P1 loot | P0 demoable 通过 | 进 TSY 后捡到遗物（先 hardcoded spawn 几件）；死亡 → 秘境所得 100% 掉、原带物 50% 掉；主世界死亡仍是 50%（回归现状） |
| P2 lifecycle | P0 + P1 | 一个 TSY 注册 5 件遗物；玩家逐个取走；骨架松动日志；取最后一件 → 塌缩事件触发 + 负压加倍；死掉的遗骸过 N 分钟变道伥 |
| P3 container | P0 + P1 + P2 | `/tsy-spawn` 同时生成容器；玩家按 E 搜刮干尸 4 秒；真元消耗速率从 baseline × 1.5；石匣需石匣匙 → 搜完即消耗 1 把；搜空最后一个 RelicCore → 发 `RelicExtracted` 给 P2 triggering 塌缩 |
| P4 hostile | P0 + P2（P3 可选） | `/tsy-spawn` 按起源填 NPC：浅层几个道伥、中层道伥+执念、深层道伥+执念+守灵+畸变体；Fuya 光环让玩家 drain × 1.5 叠加；击杀 NPC 掉 drop（道伥 5% 钥匙、守灵必掉钥匙 + 上古遗物） |
| P5 extract | P0 + P1 + P2（P3/P4 可选但推荐） | `/tsy-spawn` 生成 2-3 个 portal；玩家按 E 启动撤离 → 8 秒倒计时 → 传出；撤离中被击 → 中断归零；真元归零 → 干尸化；P2 塌缩事件 → portal 时长压到 3 秒 + spawn 3-5 临时 CollapseTear；塌缩完成 → 未出 TSY 的玩家化灰 |

---

## §2 横切修改清单（跨多个子 plan）

以下改动在多个子 plan 里都会用到，避免重复定义，集中列在这里由 P0 或 P1 接纳：

### 2.1 `DeathEvent` 扩展 `attacker_player_id`

**位置**：`server/src/combat/events.rs:82-87`

**当前**：
```rust
pub struct DeathEvent {
    pub target: Entity,
    pub cause: String,
    pub at_tick: u64,
}
```

**目标**：
```rust
pub struct DeathEvent {
    pub target: Entity,
    pub cause: String,
    pub attacker: Option<Entity>,           // ← 新增
    pub attacker_player_id: Option<Uuid>,   // ← 新增（用于 PVP 掠夺链路）
    pub at_tick: u64,
}
```

**接纳方**：**P1 plan-tsy-loot-v1** 承担此改动（因为它第一次真正用 `attacker_player_id`）

**IPC schema 同步**：`agent/packages/schema/src/combat-event.ts` 的 `CombatRealtimeEventV1.attacker_id` 已是 `Optional<string>`，无需改 schema，只需 Rust 端对齐

### 2.2 Fabric `keepInventory` mixin

**位置**：新建 `client/src/main/java/com/bong/client/mixin/MixinDeathScreen.java` 或 `MixinPlayerManager.java`

**原因**：MC 原生 death drop 机制会和 server 端 `apply_death_drop_on_revive` double-fire，导致物品被掉两次/掉错位置

**方案**：
- 禁用 vanilla `PlayerEntity.dropInventory()`
- 所有掉落走 server 端 `DroppedLootRegistry`（其 sync 机制已在 `server/src/network/dropped_loot_sync_emit.rs`）

**接纳方**：**P1 plan-tsy-loot-v1**（同 2.1，第一次需要 "禁用原生掉落" 的时机）

**mixins.json 补丁**：`client/src/main/resources/bong-client.mixins.json` 的 `client` array 新增 `"mixin.MixinDeathScreen"` 或 `"mixin.MixinPlayerManager"`

### 2.3 Zone name 约定（不改 Zone 结构）

Zone 用 `name: String` 识别（而非 enum variant），好处是加新 zone 无需改代码只改 zones.json。TSY 的约定：

- **命名 pattern**：`tsy_<来源>_<序号>`，例如：
  - `tsy_tankuozun_01`（上古大能陨落类）
  - `tsy_zongmen_lingxu_01`（宗门遗迹类：灵墟宗）
  - `tsy_zhanchang_beihuang_01`（战场沉淀类：北荒战场）
- **识别 helper**（P0 plan 提供）：`pub fn is_tsy(zone_name: &str) -> bool { zone_name.starts_with("tsy_") }`

**rationale**：坚持 name 字符串而非加 enum，降低扩展摩擦

### 2.4 TSY 跨 plan 事件（P3 → P2）

**事件**：`RelicExtracted { family_id, at_tick }` / `TsyZoneInitialized { family_id, relic_count }`

**用途**：P3 plan 的 container 搜空一个 RelicCore → 发 `RelicExtracted`，P2 lifecycle `relics_remaining -= 1`；P3 zone 初始化完成 → 发 `TsyZoneInitialized`，P2 lifecycle 设 `relics_remaining` 初值。

**接纳方**：**P3 plan-tsy-container-v1**（producer 在 P3；P2 plan 预先声明 reader，P3 plan 实装时定义 Event struct）

### 2.5 `TsyOrigin` 共享 enum

**位置**：`server/src/world/tsy_origin.rs`

```rust
pub enum TsyOrigin {
    DanengLuoluo,      // 大能陨落
    ZongmenYiji,       // 上古宗门遗迹
    ZhanchangChendian, // 上古战场沉淀
    GaoshouShichu,     // 近代高手死处
}

impl TsyOrigin {
    pub fn from_zone_name(name: &str) -> Option<Self> { ... }
}
```

**用途**：P3 plan 用来查 container spawn multiplier；P4 plan 用来查 NPC spawn pool。

**接纳方**：**P4 plan-tsy-hostile-v1**（P4 需要 enum 做 pattern match；P3 仅需映射表，可 hashmap 兜底；enum 定义集中在 P4）

### 2.6 `PlayerBusyState` 忙态互斥

**问题**：P3 搜刮、P5 撤离都会给玩家挂 Component 表示"正在做耗时事"。任意一个启动前要检查另一个不在进行。

**方案**：共享 enum + query 合并：

```rust
// 位置：server/src/player/busy.rs（新建）

#[derive(Component, Debug)]
pub struct BusyMarker {
    pub kind: BusyKind,
    pub started_at_tick: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum BusyKind {
    Searching,   // P3 SearchProgress 挂载同时 insert BusyMarker { Searching }
    Extracting,  // P5 ExtractProgress 挂载同时 insert BusyMarker { Extracting }
}
```

启动前：`query.get(player).is_ok()` → reject。清除前：`commands.remove::<BusyMarker>()`。

**接纳方**：**P5 plan-tsy-extract-v1**（P5 定义 enum；P3 plan 改 `SearchProgress` 实装时同步 insert/remove）

---

## §3 非目标（本系列 plan 不做）

明确不涉及以下内容；它们有独立 plan 或推迟到后续迭代：

| 功能 | 状态 | 说明 |
|------|------|------|
| 敌对 NPC（道伥 / 执念 / 守灵 / 畸变体） | **✅ P4 plan-tsy-hostile-v1 覆盖** | 见 P4；spawn pool 按起源，drop 表按 archetype |
| 容器搜刮机制 | **✅ P3 plan-tsy-container-v1 覆盖** | 5 档容器 + 钥匙 + 搜刮倒计时 |
| 撤离点机制 | **✅ P5 plan-tsy-extract-v1 覆盖** | 3 种 portal + race-out 切换 |
| 入口感知 HUD（"靠近时显示负压"） | 独立 plan | UX polish |
| 上古遗物 inspect 特效（反光逆转、灵纹） | 独立 plan | 视觉 polish |
| 封灵匣 / 负灵袋 器物合成 | 独立 plan | 保养容器，plan-forge 扩展；非 P3 容器 plan 范围（容器 = 搜刮目标；封灵匣 = 玩家持有器物） |
| 上古宗门遗迹 **自然生成**（worldgen） | 独立 plan | 本系列用 `/tsy-spawn` 命令手动测试；正式发布要用 worldgen |
| 天道 agent narration 接入（塌缩 / 撤离 / 守灵死叙事） | 独立 plan | Agent 层扩展，见 `plan-narrative`；P3/P4/P5 都发 IPC schema 等 narration 消费 |
| 道伥出坍缩渊后在主世界的行为 | 独立 plan / 扩展 `plan-npc-ai` | P2 lifecycle 只负责 "塌缩时挤出到主世界" 的 spawn 事件；主世界行为独立 |
| Portal 视觉 / 粒子 | client polish plan | 本系列只实装 HUD text + 进度条 |
| extraction 场次匹配 / MMR | 不做 | Bong 不是赛制游戏 |
| 秘境组队机制（系统强化） | 不做 | 玩家自行在游戏外约定，系统不强化 |
| 玩家**伪造道伥**潜入秘境（PVP gimmick） | 不做 | lore 不支持 |
| 多人同容器 / 同 portal **排队** | 不做 | 互斥 + 先到先得，保持 "抢" 的紧张感 |
| Zhinian 使用**玩家录像**的招式 | 独立 plan | MVP hardcoded combo；后续接 LifeRecord |
| Fuya 种族变种（战场蛇 / 古兽 / 妖蝠） | 独立 plan | MVP 所有 Fuya 外观 / stat 相同 |

---

## §4 术语表

统一术语，三个子 plan 必须对齐：

| 术语 | 含义 | 世界观锚点 |
|------|------|------------|
| **TSY / 坍缩渊** | 坍缩渊（Tān-Suō-Yuān） | §二 |
| **活坍缩渊** | 尚可进入的秘境状态 | §十六.一 |
| **死坍缩渊** | 已塌尽的负压空洞，不可进入 | §二 + §十六.一 |
| **裂缝** | 活坍缩渊外缘的入口 POI | §十六.一 |
| **骨架（遗物）** | 构成活坍缩渊结构的上古遗物。取完 → 塌缩 | §十六.一 step 1, §十六.三 |
| **race-out** | 塌缩触发瞬间剩余玩家撤离的拼命阶段 | §十六.一 step 4 |
| **干尸** | 秘境内死亡的修士遗骸（血肉被负压抽尽） | §十六.六 |
| **道伥** | 干尸被负压激活后形成的行走残骸 | §七 + §十六.六 |
| **上古遗物** | 1% jackpot 类物品（强度高 + 耐久低） | §十六.三 |
| **探索者遗物** | 99% 来自前人死亡的凡物 | §十六.三 |
| **封灵匣 / 负灵袋** | 保养容器（玩家持有器物，非搜刮容器）；独立 plan | §十六.四 |
| **灵质 spirit_quality** | 物品附着的真元浓度 [0.0, 1.0]。入场过滤看此字段 | `inventory/mod.rs:135` 已有 |
| **容器**（TSY 内） | loot 的唯一载体：干尸 / 骨架 / 储物袋 / 石匣 / 法阵核心 5 档 | §十六.三 容器与搜刮 / P3 plan |
| **搜刮**（search） | 玩家对容器发起的倒计时交互（3-40 秒）；期间真元抽吸 × 1.5 | §十六.三 / P3 plan |
| **钥匙 / 令牌** | 石匣匙 / 玉棺纹 / 阵核钤，单次使用即碎 | §十六.三 / P3 plan §3 |
| **道伥 / 执念 / 秘境守灵 / 负压畸变体** | 4 档 TSY PvE archetype | §十六.五 敌对 NPC / P4 plan |
| **起源**（TSY origin） | 4 类：大能陨落 / 宗门遗迹 / 战场沉淀 / 近代高手死处 | §十六.一 / P4 plan §1.3 |
| **耗真元光环**（Fuya aura） | 畸变体被动 AOE，玩家在范围内真元 drain × 1.5 | §十六.五 畸变体行 / P4 plan §3 |
| **主裂缝 / 深层缝 / 塌缩裂口** | 3 种 RiftPortal：双向 / 单向出 / race-out 临时 | §十六.四 撤离点 / P5 plan §1.1 |
| **撤离**（extract） | 在 portal 附近按 E 启动的倒计时（3 / 8 / 12 秒）；移动 / 战斗 / 受击即中断 | §十六.四 / P5 plan §2 |
| **塌缩裂口**（CollapseTear） | race-out 时临时 spawn 的 portal，3 秒撤离时长 | §十六.四 / P5 plan §3 |

---

## §5 依赖对齐与风险

### 上游 plan 的状态依赖

| 上游 plan | 依赖点 | 当前状态 | 风险 |
|-----------|--------|---------|------|
| plan-inventory-v1 | `DroppedLootRegistry` + `ItemInstance.spirit_quality` + `apply_death_drop_on_revive` | ✅ 完成 | 低 |
| plan-combat-no_ui | `DeathEvent` + `Wounds` + `bleed_out` | ✅ 完成，但 DeathEvent 要扩 `attacker` | 中 — 改 schema 要和现有 combat 对齐 |
| plan-cultivation-v1 | `PlayerState.spirit_qi / realm` + `death_hooks::apply_revive_penalty` | ✅ 完成 | 低 |
| plan-death-lifecycle-v1 | 运数 / 劫数 / 寿元扣除规则 | ✅ 完成 | 低 |
| plan-persistence-v1 | SQLite player_core + biography append | ✅ 完成 | 低 |
| plan-npc-ai-v1 | NpcArchetype + brain + spawn | ✅ 完成 | 中 — lifecycle plan 要新增 `Daoxiang` archetype |
| plan-ipc-schema-v1 | TypeBox → JSON schema → Rust serde 流水线 | ✅ 完成 | 低 |

### 下游 plan 的潜在影响

| 下游 / 并行 plan | 潜在冲突 | 缓解 |
|------------------|----------|------|
| plan-HUD-v1 | 入场/出关可能要 HUD 变化（负压提示） | P0 先不做 HUD，P3 补 |
| plan-shelflife-v1 | 凡物出关后会自然衰变 | 已有 `ItemInstance.freshness`，loot plan 复用 |
| plan-tribulation-v1 | "通灵躲天劫入负灵域" 的路径和 TSY 有交集 | lifecycle plan 里说明：通灵可进 TSY 躲劫，但被抽更快 |

---

## §6 总验收（全系列收尾）

三个子 plan 全部 merge 后，端到端验收：

**E2E 场景**（目标 demo）：

1. 玩家 A（引气 3）进入一个有 3 件遗物的 TSY
2. 浅层（-0.3）能苟 ≈ 10+ 分钟，中层（-0.7）2-3 分钟，深层（-1.0）ok 但紧迫
3. A 在深层拿到 1 件上古遗物（封灵袋里的残卷）
4. 玩家 B（固元 5）从裂缝进 → 在浅层埋伏 A
5. A 出关路上被 B 截杀 → `DeathEvent` 带 `attacker_player_id = B`
6. A 的干尸留在裂缝附近；秘境所得 100% 在死亡点掉落
7. A 身上带进的凡铁剑按 50% 规则掉落
8. B 拾取 A 掉的上古残卷 → 背包一列
9. B 返回浅层继续等下一个猎物
10. 玩家 C 进 TSY → 下深层 → 拿走最后一件遗物 → **塌缩触发** → 负压加倍 → C 能不能跑出来看他真元够不够
11. 塌缩完成 → TSY name 从 `tsy_*_active` 改为 `tsy_*_dead`（或从 registry 剔除），裂缝消失
12. 15 分钟后，A 的干尸变道伥 → 若在 C 塌缩时被挤出，则出现在主世界；否则留在死坍缩渊 zone 内不再生效

**自动化脚本**（P2 完成后）：`bash scripts/smoke-tsy.sh`
- 跑一个 headless 脚本模拟上述 E2E
- 期望输出每一步的事件链（server log + agent log）
- exit 0 = 全过 / 非 0 = 某环节挂了

---

## §7 命名与版本

- 3 个子 plan 文件：`plan-tsy-zone-v1.md`、`plan-tsy-loot-v1.md`、`plan-tsy-lifecycle-v1.md`
- 本 meta：`plan-tsy-v1.md`
- 完成后归档：`docs/finished_plans/plan-tsy-v1.md` + 3 个子 plan（同时归档）
- v2 的触发条件：P3 后续（浪潮 / HUD / 封灵匣 / worldgen）开始时启动 `plan-tsy-v2.md` 作新 meta

---

## §8 落地节奏建议

```
week 0: plan-tsy-dimension-v1 → /consume-plan tsy-dimension → PR → review → merge
         （骨架升 active 前：Q10 client mixin 手动 audit + Q2/Q4/Q5 标关闭）
week 1: plan-tsy-zone-v1      → /consume-plan tsy-zone      → PR → review → merge
week 2: plan-tsy-loot-v1      → /consume-plan tsy-loot      → PR → review → merge
week 3: plan-tsy-lifecycle-v1 → /consume-plan tsy-lifecycle → PR → review → merge
---  ↑ 核心闭环完成（搜打撤骨架 demoable）  ↓ 玩法层扩展  ---
week 4: plan-tsy-container-v1 → /consume-plan tsy-container → PR → review → merge
week 5: plan-tsy-hostile-v1   → /consume-plan tsy-hostile   → PR → review → merge
week 6: plan-tsy-extract-v1   → /consume-plan tsy-extract   → PR → review → merge
week 7: smoke-tsy-full.sh + manual E2E + 归档
（worldgen 并行轨：骨架→active 触发于 week 4 之后，消费时机按 P3/P4/P5 真实 POI 数据驱动需求）
```

**关键里程碑**：
- **M-dim**（week 0 结束）：P-1 merged → P0 才能开工（`DimensionKind`/`DimensionTransferRequest`/`CurrentDimension`/`TerrainProviders` 全部就位）
- **M-core**（week 3 结束）：P0+P1+P2 merged → 骨架版搜打撤可 demo（进 TSY + 捡遗物 + 死亡掉 + 塌缩）
- **M-full**（week 6 结束）：+P3+P4+P5 merged → 完整搜打撤玩法（容器 + NPC + 撤离）

P3/P4/P5 之间可顺序互换或并行开工（看 PR 冲突风险）。实际节奏可能更快，每周一 plan 是保守估计。

---

## §9 风险 / 未决

| 风险 | 级别 | 缓解 |
|------|------|------|
| 非线性抽取公式参数化难调平衡 | 高 | P0 用保守参数（`n=1.5`、每 tick 1% 池），真实 playtest 再调 |
| `DeathEvent.attacker_player_id` 改动破坏 existing tests | 中 | P1 改时同步修所有引用点（`grep -r 'DeathEvent'`）+ 跑完 combat test suite |
| Fabric mixin 和现有 6 个 mixin 冲突 | 中 | MixinDeathScreen 和现有 Camera/GameRenderer 无重叠 target，低冲突概率 |
| TSY zone overlapping 导致 "走到边界" 判定错误 | 中 | P0 规定 TSY zone 必须和现有 zone 不相交（load 时校验） |
| 塌缩时 race-out 的玩家来不及退出 → 卡死 | 中 | lifecycle plan 加 fallback：塌缩完成 + 5 秒还在内部 → force 传送到裂缝外，真元扣尽 |
| 道伥 archetype 新增后和现有 NPC AI 系统冲突 | 低 | lifecycle plan 做小步走：先复用 `NpcArchetype::Elite` 改 tag，再独立 variant |

---

## §10 进度日志

- 2026-04-25：审计 server/src/ 下无任何 `tsy_*` / `DimensionKind` / `CurrentDimension` / `LayerBundle` / `DimensionTransferRequest` 实装，`TerrainProvider` 仍为单实例（非 per-dimension）；P-1 ~ P5 全部子 plan 尚未 consume，meta 维持原计划，下一步仍为启动 P-1 `tsy-dimension`。
- **2026-04-26**：**P-1 解冻** — PR #47（merge 579fc67e）落地 `plan-tsy-dimension-v1` §1.1–§3.2 + §4.2 完整链路：`world/dimension.rs` + `world/dimension_transfer.rs` 新建（`DimensionKind` enum / `DimensionLayers` resource / `CurrentDimension` 组件 / `register_tsy_dimension` / `DimensionTransferRequest` event），双 LayerBundle 与 `TerrainProviders` 多 provider routing 就位，`Zone` struct + `find_zone(dim,pos)` 签名升级（50+ caller 扫荡），DB migration v13 持久化 `last_dimension`。1252 单测全绿。**P0 `tsy-zone` 现可开工**；P1/P2/P3/P4/P5 等 P0 demoable；worldgen 可与 P3/P4/P5 并行（已有 `TerrainProviders.tsy: Option` 占位）。

---

**下一步**：`/consume-plan tsy-zone` 启动 P0（P-1 已落地，硬前置已解冻）。

---

## Finish Evidence

### 落地清单

- **P-1 plan-tsy-dimension-v1**（位面基础设施）：
  - `docs/plan-tsy-dimension-v1.md`（已含独立 Finish Evidence）
  - `server/src/world/dimension.rs`（`DimensionKind` enum / `DimensionLayers` / `CurrentDimension` / `register_tsy_dimension`）
  - `server/src/world/dimension_transfer.rs`（`DimensionTransferRequest` event + `apply_dimension_transfers` system）
  - `server/src/world/zone.rs` `Zone.dimension` + `find_zone(dim, pos)` 签名升级
  - `server/src/world/terrain/` 多位面 `TerrainProviders` routing
  - DB migration v13：`last_dimension` 持久化
- **P0 plan-tsy-zone-v1**（zone 识别 + 负压 + 跨位面 portal + 入场过滤）：
  - `server/src/world/tsy.rs`（`TsyPresence` / `RiftPortal` / `DimensionAnchor`）
  - `server/src/world/tsy_drain.rs`（负压 tick）
  - `server/src/world/tsy_portal.rs`（双向 portal）
  - `server/src/world/tsy_filter.rs`（入场过滤）
  - `server/src/world/tsy_dev_command.rs`（`!tsy-spawn` 调试命令）
  - `agent/packages/schema/src/tsy.ts`（`TsyEnterEventV1` / `TsyExitEventV1` / `TsyDimensionAnchorV1` / `TsyFilteredItemV1`）
- **plan-tsy-zone-followup-v1**（集成测 + Server→Redis 桥）：
  - `docs/plan-tsy-zone-followup-v1.md`（已含独立 Finish Evidence）
  - `server/src/world/tsy_integration_test.rs`
  - `server/src/network/tsy_event_bridge.rs`（Bevy event → `bong:tsy_event` Redis publish）
- **P1 plan-tsy-loot-v1**（99/1 上古遗物 + 秘境分流死亡 + 干尸 + Mixin）：
  - `server/src/inventory/ancient_relics.rs`（`ItemRarity::Ancient` + 遗物模板池）
  - `server/src/inventory/tsy_loot_spawn.rs`（`tsy_loot_spawn_on_enter` + `relic_count_for_source`）
  - `server/src/inventory/tsy_death_drop.rs`（`apply_tsy_death_drop` + 干尸标记）
  - `server/src/combat/events.rs` `DeathEvent { attacker, attacker_player_id }`（§2.1 横切）
  - `client/src/main/java/com/bong/client/mixin/MixinPlayerEntityDrop.java`（§2.2 禁用 vanilla 死亡掉落）
  - `client/src/main/resources/bong-client.mixins.json` 注册 `MixinPlayerEntityDrop`
- **P2 plan-tsy-lifecycle-v1**（状态机 + 塌缩 + 道伥转化）：
  - `server/src/world/tsy_lifecycle.rs`（family 状态机 + 塌缩清理 + 道伥 spawn API）
  - `server/src/npc/mod.rs` `NpcArchetype::Daoxiang` variant
  - `agent/packages/schema/samples/server-data.tsy-collapse-started-ipc.sample.json`（`TsyZoneActivated` / `TsyCollapseStarted` / `TsyCollapseCompleted` / `DaoxiangSpawned` V1）
  - `server/src/world/tsy_lifecycle_integration_test.rs`
- **P3 plan-tsy-container-v1**（5 档容器 + 钥匙 + 真元 1.5×）：
  - `server/src/world/tsy_container.rs`（`LootContainer` / `SearchProgress` / `KeyKind`）
  - `server/src/world/tsy_container_search.rs`（搜刮 system + `RelicExtracted` / `TsyZoneInitialized` event）
  - `server/src/world/tsy_container_spawn.rs`（按 origin 分布 spawn）
  - `server/src/world/loot_pool.rs`（`LootPoolRegistry`）
  - `client/src/main/java/com/bong/client/hud/SearchProgressHudPlanner.java`
- **P4 plan-tsy-hostile-v1**（4 类敌对 archetype + AI tree + spawn pool + drop table）：
  - `server/src/npc/tsy_hostile.rs`（`TsyOriginSpawnPool` / sentinel phase / Fuya aura）
  - `server/src/world/tsy_origin.rs`（`TsyOrigin` enum + `from_zone_name`）
  - `server/src/schema/tsy_hostile.rs`（`TsyHostileArchetypeV1` / `TsyNpcSpawnedV1` / `TsySentinelPhaseChangedV1`）
  - `agent/packages/schema/src/tsy-hostile-v1.ts` + samples
  - `server/src/network/tsy_event_bridge.rs` 扩展 publish hostile events
- **P5 plan-tsy-extract-v1**（3 种 portal + 撤离倒计时 + race-out 切换）：
  - `server/src/world/extract_system.rs`（`ExtractProgress` 组件 + `ExtractProgressPulse` event）
  - `server/src/world/rift_portal.rs`（`RiftPortalKind` 三种）
  - `server/src/schema/server_data.rs`（`RiftPortalState` / `RiftPortalRemoved` / `ExtractProgress` V1）
  - `agent/packages/schema/src/extract-v1.ts`
  - `server/src/network/extract_emit.rs`（撤离 IPC 状态同步）
  - `client/src/main/java/com/bong/client/network/ExtractServerDataHandler.java` + `hud/ExtractProgressHudPlanner.java`
- **plan-tsy-worldgen-v1**（双 manifest worldgen + POI consumer）：
  - `docs/plan-tsy-worldgen-v1.md`（已含独立 Finish Evidence）
  - `worldgen/scripts/terrain_gen/profiles/tsy_{daneng_crater,zongmen_ruin,zhanchang,gaoshou_hermitage}.py`
  - `server/src/world/tsy_poi_consumer.rs`（`BONG_TSY_RASTER_PATH` 接入）
  - `scripts/dev-reload.sh` 双 manifest 改造

### 关键 commit

- `579fc67e` (2026-04-26) — plan-tsy-dimension-v1: TSY 位面基础设施（DimensionType + 跨位面传送 + Zone.dimension）(#47)
- `bd349286` (2026-04-26) — plan-tsy-zone-v1: 活坍缩渊 P0 基础设施（zone 识别 + 负压 tick + 跨位面 portal + 入场过滤）(#49)
- `29f8033c` (2026-04-27) — plan-tsy-zone-followup-v1: 集成测 + Server→Redis 桥（zone-v1 §5.2 / §1.4 收尾）(#50)
- `77d042fb` (2026-04-27) — plan-tsy-worldgen-v1: TSY 双 manifest worldgen 流水线 + POI consumer (#51)
- `9fb8d2b7` (2026-04-27) — plan-tsy-loot-v1: TSY 物资 99/1 + 秘境分流死亡 + 干尸 + Mixin (#53)
- `99c29ebd` (2026-04-27) — plan-tsy-lifecycle-v1: 状态机 + 塌缩 + 道伥转化（TSY 核心闭环）(#54)
- `d6e84e37` (2026-04-27) — plan-tsy-container-v1: TSY 容器搜刮 5 档 + 钥匙 + 真元 1.5× (#55)
- `f94be967` (2026-04-27) — Merge pull request #59: plan-tsy-extract-v1（撤离 IPC + 客户端 HUD + 服务端 portal 闭环）
- `9d05e622` (2026-04-27) — plan-tsy-hostile-v1: 接入 TSY 敌对 NPC（已含前置 schema/IPC commits 链）

### 测试结果

- `cd server && cargo test` — 1536 个 `#[test]` （仓库总量；P-1 落地时记录 1252，新增 P0–P5 + worldgen 增量）
- TSY 模块单测分布：dimension 6 + dimension_transfer 5 + tsy_drain 11 + tsy_portal 7 + tsy_filter 8 + tsy_dev_command 5 + tsy_lifecycle 19 + tsy_container 8 + tsy_container_search 7 + tsy_loot_spawn 9 + ancient_relics 7 + extract_system 8 + rift_portal 3 + tsy_poi_consumer 8 + tsy_hostile 9 + tsy_integration_test 4 + tsy_lifecycle_integration_test 8（合计 130+）
- `cd agent/packages/schema && npm test` — 7 个 vitest 文件，含 `tsy.test.ts` / `tsy-hostile-v1.test.ts` / `extract-v1.test.ts` / `container-interaction.test.ts`
- Smoke 脚本：`scripts/smoke-tsy-zone.sh` / `scripts/smoke-tsy-loot.sh` / `scripts/smoke-tsy-lifecycle.sh`

### 跨仓库核验

- **server**：
  - 位面基础：`DimensionKind` / `DimensionLayers` / `CurrentDimension` / `DimensionTransferRequest` @ `server/src/world/dimension*.rs`
  - Zone 识别：`Zone::is_tsy()` / `find_zone(dim, pos)` @ `server/src/world/zone.rs`
  - 数据模型：`LootContainer` / `SearchProgress` / `KeyKind` / `RelicCore` @ `server/src/world/tsy_container.rs`
  - 状态机：`TsyLifecycle` family state machine + `RelicExtracted` / `TsyZoneInitialized` events @ `server/src/world/tsy_lifecycle.rs` + `tsy_container_search.rs`
  - 撤离：`ExtractProgress` / `RiftPortal` @ `server/src/world/extract_system.rs` + `rift_portal.rs`
  - 敌对：`TsyOrigin` / `TsyOriginSpawnPool` / `TsyHostileArchetype` @ `server/src/world/tsy_origin.rs` + `server/src/npc/tsy_hostile.rs`
  - 死亡分流：`apply_tsy_death_drop` + `DeathEvent.attacker_player_id` @ `server/src/inventory/tsy_death_drop.rs` + `server/src/combat/events.rs`
- **agent**：
  - `TsyEnterEventV1` / `TsyExitEventV1` / `TsyDimensionAnchorV1` / `TsyFilteredItemV1` @ `agent/packages/schema/src/tsy.ts`
  - `TsyZoneActivatedV1` / `TsyCollapseStartedV1` / `TsyCollapseCompletedV1` / `DaoxiangSpawnedV1` @ `agent/packages/schema/src/tsy.ts`
  - `TsyHostileArchetypeV1` / `TsyNpcSpawnedV1` / `TsySentinelPhaseChangedV1` @ `agent/packages/schema/src/tsy-hostile-v1.ts`
  - `RiftPortalStateV1` / `RiftPortalRemovedV1` / `ExtractProgressV1` @ `agent/packages/schema/src/extract-v1.ts`
  - 双端 sample：`agent/packages/schema/samples/tsy-{enter,exit,npc-spawned,sentinel-phase-changed}.sample.json` + `server-data.tsy-collapse-started-ipc.sample.json`
- **client**：
  - `MixinPlayerEntityDrop` @ `client/src/main/java/com/bong/client/mixin/MixinPlayerEntityDrop.java`（vanilla 死亡掉落禁用）
  - `SearchProgressHudPlanner` / `SearchHudState` / `ExtractProgressHudPlanner` @ `client/src/main/java/com/bong/client/hud/`
  - `ExtractServerDataHandler` @ `client/src/main/java/com/bong/client/network/ExtractServerDataHandler.java`
- **worldgen**：
  - 4 个 TSY profile：`profiles/tsy_{daneng_crater,zongmen_ruin,zhanchang,gaoshou_hermitage}.py`
  - 双 manifest 流水线：`scripts/dev-reload.sh` + `bakers/raster_export.py` `layer_whitelist`
  - server 消费：`server/src/world/tsy_poi_consumer.rs` + `BONG_TSY_RASTER_PATH`

### 遗留 / 后续

- 迁移注释（plan-server-cmd-system-v1，2026-05-01）：历史 `!tsy-spawn` 调试命令已迁移为 Valence brigadier `/tsy_spawn <family_id>`；正文保留历史记录不重写。
- §3 非目标列出的扩展项（worldgen 自然生成上古宗门遗迹、入口感知 HUD、上古遗物 inspect 特效、封灵匣/负灵袋合成、Zhinian 用 LifeRecord 录像招式、Fuya 种族变种）独立 plan 推进
- 天道 agent narration 接入（塌缩 / 撤离 / 守灵死叙事）由 `plan-narrative` 独立承担
- §2.6 `BusyMarker` 共享 enum 未集中实装为单独 component；当前 P3 `SearchProgress` 与 P5 `ExtractProgress` 各自互锁，功能等价但未共享标记类型。如后续需统一查询，留 v2 收口
- §6 `bash scripts/smoke-tsy.sh` 全场景 E2E 单脚本未单独产出；已有按子 plan 的 smoke 脚本（`smoke-tsy-zone.sh` / `smoke-tsy-loot.sh` / `smoke-tsy-lifecycle.sh`）覆盖关键路径
- v2 触发条件按本 plan §7 约定：浪潮 / HUD polish / 封灵匣 / worldgen 自然生成等阶段开工时启动 `plan-tsy-v2.md` 作新 meta
