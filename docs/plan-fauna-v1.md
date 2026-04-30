# Bong · plan-fauna-v1 · 骨架

**妖兽骨系材料**。末法残土三类异兽（噬元鼠 / 拟态灰烬蛛 / 异变缝合兽）击杀后掉落**异兽骨骼**和**变异核心**，作为封灵骨币原料、暗器/器修载体材料、阵法高阶原料，形成"打兽 → 骨骼/兽核 → 骨币 / 武器 / 阵石"的资源闭环。本 plan 定义 item 体系、combat drop 链路、forge/alchemy 集成，并正典化 plan-forge-v1 中 `yi_beast_bone` 占位名。

**世界观锚点**：`worldview.md §七 动态生物生态`（噬元鼠 / 灰烬蛛 / 缝合兽行为逻辑）· `worldview.md §九 经济与交易 / 货币：封灵骨币`（异兽骨骼 + 阵法封死真元 = 通货）· `worldview.md §四 战斗系统 / 暗器流载体`（"异变兽骨/灵木：飞 50 格保留 80% 真元"——高级载体）· `worldview.md §十 资源与匮乏`（异变兽核 = 固元→通灵突破必需，极稀）· `worldview.md §十六.五 负压畸变体`（负压改造兽 = 变异核心 + 异兽骨骼特殊掉落）

**library 锚点**：`docs/library/ecology/ecology-0005 异兽三形考.json`（噬元鼠/灰烬蛛/缝合兽物理逻辑，核心参考）· 待写 `peoples-XXXX 骨币铸法简录`（封灵骨币制作工艺，anchor worldview §十 + §五.3 阵法流）

**交叉引用**：
- `plan-combat-no_ui`（前置；击杀 = drop 事件来源）
- `plan-npc-ai-v1`（前置；三类异兽 AI 已实装，本 plan 在其 death 事件挂 drop 钩子）
- `plan-forge-v1`（✅；`yi_beast_bone` placeholder → 正典名 `yi_shou_gu` 替换）
- `plan-niche-defense-v1`（高阶阵石消耗 `mutant_beast_core` 异变兽核）
- `plan-zhenfa-v1`（24 小时阵法载体：异变兽核镶嵌方块）
- `plan-mineral-v2`（骨币制作与灵石品阶形成经济博弈）
- `plan-shelflife-v1`（骨骼 / 兽核是否加 freshness 半衰期？）
- `plan-botany-v2`（active；P5 通过本 plan 接入"植物 AttractsMobs hazard"——v2 物种 lie_yuan_tai / yuan_ni_hong_yu 等吸引特定异兽聚集，需本 plan 提供"按区域 spawn 指定 BeastKind"的 hook）
- `plan-tools-v1`（骨架；骨骸钳从异兽尸体取骨——本 plan §3 drop 链路可选分支接入"屠宰会话"，由 tools-v1 P3 落地）

**阶段总览**：
- P0 ⬜ item 定义（骨骼分级 + 变异核心）+ drop 链路
- P1 ⬜ 封灵骨币制作 recipe（骨骼 + 真元注入 + 阵法封死）
- P2 ⬜ forge/alchemy 正典化（yi_beast_bone → yi_shou_gu，batch replace）
- P3 ⬜ shelflife 接入（骨币半衰期 + 骨骼保鲜）
- P4 ⬜ 高阶掉落（负压畸变体特殊掉落 + 变异核心稀有率）

---

## §0 设计轴心

- [ ] **骨系分级**：骨骼品质与异兽强度挂钩；低级兽骨只能做普通骨币 / 基础阵法材料，高阶缝合兽骨骼才能锁住足够真元做高价值骨币
- [ ] **变异核心稀有**：异变兽核（固元→通灵突破必需）打怪低概率掉落，是最硬的"准入门票"之一
- [ ] **骨币半衰期是设计核心**：骨骼制成骨币后真元缓慢流失（worldview §十），逼迫流转而非囤积——plan-shelflife 的 Exponential decay profile 承接这条
- [ ] **不做自动刷怪**：怪物密度由天道/worldgen 动态控制，本 plan 只处理 drop 链路，不新增刷怪逻辑

---

## §1 第一性原理（烬灰子四论挂点）

- **缚论·骨骼锁真元**：异兽在末法环境中体内真元高度压缩，骨骼结构致密——击杀后骨骼仍封存一部分残余真元（这是骨币价值的物理基础）
- **噬论·骨币流失**：即便骨骼结构致密，末法天地仍会缓慢噬散封存的真元——半衰期机制的设定依据
- **音论·兽核震荡**：兽核内部是大量压缩灵气的"驻波"，吸收时强烈震荡感知系统 → 幻觉（worldview §七 缝合兽击杀描述）；高阶载体用途就是利用这种"固化驻波"
- **影论·载体次级投影**：异兽骨骼 / 灵木作为暗器载体，其高密度结构能维持投射物上的真元镜印更久（"飞 50 格保留 80% 真元"的物理解释）

---

## §2 异兽 item 分级

| item_id | 来源 | 品阶 | 主要用途 |
|---|---|---|---|
| `shu_gu`（噬元鼠骨）| 噬元鼠群击杀 | 凡 | 最低档骨币原料；封真元极少 |
| `zhu_gu`（拟态蛛骨）| 灰烬蛛击杀 | 下品灵 | 骨币原料 + 初级阵法载体 |
| `feng_he_gu`（缝合兽骨）| 异变缝合兽击杀 | 上品灵 | 高价值骨币 + 暗器流载体材料 |
| `yi_shou_gu`（异兽骨，通称）| 各类异兽；plan-forge-v1 已用 | 混合 | forge 原料（正典名，替换 `yi_beast_bone`）|
| `bian_yi_hexin`（变异核心）| 异变缝合兽低概率 + 负压畸变体 | 极稀 | 固元→通灵突破必需 + 高阶阵石原料 |
| `fu_ya_hesui`（负压核碎）| 负压畸变体特殊掉落 | 稀 | 高阶阵法 + 待立 tsy plan |

---

## §3 drop 链路

```
击杀事件：
  NPC death event (plan-npc-ai-v1 已有 EntityDead event)
  → FaunaDropSystem 读取 NPC 的 FaunaTag::BeastKind
  → 按 BeastKind 抽取 DropTable
  → spawn ItemEntity 掉落到死亡坐标附近

DropTable 设计：
  noise_seed: 每次确定性 RNG（基于 NPC entity_id + tick）
  guaranteed: [ (item_id, quantity_range) ]
  rare:        [ (item_id, probability, quantity_range) ]
  variant:     按 NPC 的 BeastVariant (Normal / Thunder / Tainted) 修饰掉落率
```

| 兽种 | 保底掉落 | 稀有（≤10%）|
|---|---|---|
| 噬元鼠（群）| `shu_gu` × 1–3 | — |
| 拟态灰烬蛛 | `zhu_gu` × 1–2 | `zhen_shi_chu`（初级阵石原料）5% |
| 异变缝合兽 | `feng_he_gu` × 2–4 | `bian_yi_hexin` 8% |
| 负压畸变体 | `feng_he_gu` × 3–5 + `fu_ya_hesui` × 1 | `bian_yi_hexin` 20% |

---

## §4 封灵骨币制作

> worldview §十：修士将真元注入异变兽骨骼并用阵法封死，制成骨币。

- [ ] **制作路径**：`BoneCoinCraftSession`（新 session 类型）— 玩家手持骨骼 + 消耗真元 X → 注入 → 阵法封闭（消耗 `zhen_shi_chu` 或真元量 × 系数）→ 产出 `bone_coin_N`（N = 封入真元量取整）
- [ ] **骨骼品阶上限**：`shu_gu` 封上限 5 真元 / `zhu_gu` 封上限 15 / `feng_he_gu` 封上限 40（真元超上限则自动封满，多余溢出散逸）
- [ ] **骨币 shelflife**：接 plan-shelflife-v1 — `bone_coin` 挂 Exponential profile，half_life 依面值（5 真元骨币 ≈ 3 天，40 真元 ≈ 14 天）；避免囤积稳定价值
- [ ] **多人见证**（可选，v2+）：高价值骨币制作时周围有其他玩家"见证"，提升封印成功率 → 鼓励社交

---

## §5 forge 正典化

> plan-forge-v1（已归档）及 plan-mineral-v2 使用占位名 `yi_beast_bone`，需批量替换为 `yi_shou_gu`。

- [ ] **server 替换**：`server/src/forge/` 中所有 `"yi_beast_bone"` 字符串 → `"yi_shou_gu"`（含 steps.rs:412 / mod.rs:617 / 617 等）
- [ ] **assets 替换**：`server/assets/forge/blueprints/*.json` 中的 `material_id`
- [ ] **agent schema 替换**：`agent/packages/schema/src/forge.ts` 如有占位名同步替换
- [ ] **tests**：forge blueprint 加载单测命中 `yi_shou_gu`；drop chain 单测产出 `yi_shou_gu` 类型

---

## §6 数据契约（下游 grep 抓手）

| 契约 | 位置 |
|---|------|
| `FaunaTag { beast_kind: BeastKind, variant: BeastVariant }` component | `server/src/fauna/components.rs`（新文件）|
| `BeastKind` enum (Rat / Spider / HybridBeast / VoidDistorted) | `server/src/fauna/components.rs` |
| `DropTable` + `FaunaDropSystem` | `server/src/fauna/drop.rs`（新文件）|
| item toml × 6 | `server/assets/items/fauna/` |
| `BoneCoinCraftSession` | `server/src/fauna/bone_coin.rs`（新文件）|
| `yi_shou_gu` 全量替换（批量 sed）| `server/src/forge/*.rs` + `server/assets/forge/blueprints/*.json` |
| shelflife profile `bone_coin_*` | `server/src/shelflife/registry.rs` |

---

## §7 实施节点

- [ ] **P0**：`FaunaTag` + `BeastKind` + 6 种 item toml + `FaunaDropSystem`（挂 EntityDead hook）+ drop table 保底路径 + 单测（每 BeastKind 保底 drop × 3）
- [ ] **P1**：骨币制作 session + 品阶封印上限 + shelflife profile 注册 + e2e（制作骨币 → freshness 挂载 → 半衰期衰减）
- [ ] **P2**：forge 正典化（批量替换 + test 全绿）
- [ ] **P3**：shelflife 骨骼保鲜（击杀后骨骼有限 freshness，逾期品阶下降或无法用于高价值骨币）
- [ ] **P4**：负压畸变体特殊掉落 + 变异核心稀有率 RNG + 兽核吸收幻觉 VFX（StatusEffect::InsightHallucination 短暂）

---

## §8 开放问题

- [ ] 噬元鼠击杀是否给骨骼掉落（其骨骼极小，可能仅给"鼠须"等 junk item 而非有效材料）
- [ ] `bian_yi_hexin`（变异核心）是否与 plan-cultivation-v1 的固元→通灵突破直接挂钩（消耗型突破材料）or 只是"需要在附近击杀"触发？
- [ ] 骨币面值颗粒度：5 / 15 / 40 真元三档是否够用？还是需要更细的档位（1 / 5 / 10 / 30 / 50）？
- [ ] 骨骼保鲜期：是否加 freshness decay（骨骼放久了封印能力下降）——避免屯货，与骨币流通设计一致
- [ ] `fu_ya_hesui`（负压核碎）的后续用途：待 plan-tsy-* 系列定义坍缩渊材料体系后对接
- [ ] 阵法制作傀儡（worldview §十"游商傀儡"用异变兽骨骼 + 阵法炼制）：归 plan-zhenfa 还是 plan-fauna？

---

## §9 进度日志

- **2026-04-27**：骨架立项。来源：`docs/plans-skeleton/reminder.md` "通用/跨 plan"节 (`plan-fauna-v1 待立`) + plan-forge-v1 正典化缺口（`yi_beast_bone` placeholder）。`docs/library/ecology/ecology-0005 异兽三形考.json` 已于 2026-04-24 落库，物理逻辑锚点完备。server 侧当前仅有 `mutant_beast_core` item（lingtian 补灵用），无完整 fauna drop 系统。
