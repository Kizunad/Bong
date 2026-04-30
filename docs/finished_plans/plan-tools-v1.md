# Bong · plan-tools-v1

**采集 / 加工凡器工具体系**。末法残土修士使用**凡器**（无真元加持的低品阶工具）执行采集 / 制作 / 加工动作；不持工具空手采特殊植物 / 矿物 / 兽骨 → 按 worldview §四 16 部位 × 6 档伤口模型触发**真伤口**（LACERATION / FRACTURE）+ §四 异体排斥（植物毒素 / 兽骨锐刃残留真元）。本 plan 把"凡器"作为末法残土凡夫日用品的统一抽象层落地：定义 7 件初版工具、`ToolKind` enum、`ToolTag` component、采集 / 制作 session 的 `required_tool` 接入点，承接 plan-botany-v2 / plan-mineral-v2 / plan-fauna-v1 / plan-forge-v1 的 `required_tool` 声明。

**世界观锚点**：
- `worldview.md §四 战斗系统 / 战力分层`（16 部位 × 6 档伤口模型——空手采高阶植物触发的 LACERATION / FRACTURE 完全沿用 §四 既有伤口档位，本 plan 不引入新伤口概念）
- `worldview.md §四 异体排斥`（凡器与法器的本质边界——凡器**不引入异种真元**，纯物理刃；空手采高阶植物吃异体排斥是因为植物体内含轻微异种真元 / 毒素）
- `worldview.md §四 暗器流 / 载体材质分级`（**凡器在 worldview 已有位置**——§四 暗器流明确"凡铁 / 木石（劣质）：飞 10 格损失 75% 真元"vs"异变兽骨 / 灵木（优良）：飞 50 格保留 80% 真元"。**凡器 = 凡铁 / 木石档**——它们配不上锁住真元的优良载体级别，所以命名禁用"灵\*"词头）
- `worldview.md §九 经济与交易`（凡器是日用品流通——§九 "金银如废土，唯一的硬通货是封灵骨币"暗示：凡器可以买卖但不能作为大额资产流转，与法器的真元锁不同）
- `worldview.md §十 资源与匮乏`（工具本身要消耗矿物 / 兽骨 / 灵木制作——与 plan-mineral-v2 / plan-fauna-v1 / plan-spiritwood-v1 资源闭环挂钩；§十 资源种类表里"载体材料"的"高"稀缺度档位由 fauna 异兽骨 / spiritwood 灵木独占，凡器材料属"中等"档）

> **注**：worldview 当前无"凡器"独立小节，但**已有完整支撑**——§四 暗器流载体分级隐含"凡铁/木石 = 凡器档"。本 plan 不强求 worldview 修改，但 P5 可作为正典化任务在 §四 末尾加 "§四.X 凡器与凡夫战力" 小节，明确凡器的工具语义边界。

**library 锚点**：
- `docs/library/ecology/ecology-0002 末法药材十七种.json`（夜枯藤 / 寒戟莲等高阶植物——空手采触发真伤的范例物种来源）
- 待写 `peoples-XXXX 残土凡器谱`（七件工具的工艺、材料、传承故事；anchor §四 暗器流载体分级 + §十 资源稀缺）
- 待写 `peoples-XXXX 凡夫修士行记`（凡器使用日常视角，anchor §九 经济流通）

**交叉引用**：
- `plan-botany-v2.md`（active；P0–P3 退化 `required_tool=None` → `dispersal_chance=1.0`，本 plan 落地后 P4 回填 WoundOnBareHand 真伤——见 plan-botany-v2 §"风险与缓解"。**反向依赖**——botany-v2 不等本 plan 也能推 P0–P3）
- `plan-fauna-v1.md`（骨架；屠宰刀 / 骨骸钳从异兽尸体取骨需特定工具——本 plan 与 fauna-v1 协同定义"屠宰会话"。fauna-v1 P0 输出异兽骨 → 本 plan P1 中两件工具材料）
- `plan-mineral-v2.md`（已归档 finished；MiningSession 已有 `tool_instance_id: Option<u64>` 占位——本 plan 在 ToolKind enum 里枚举 `Pickaxe` 类即可，无需改 mineral 代码）
- `plan-forge-v1.md`（已归档；锻造系统消费工具但不直接定义工具——本 plan 7 件工具走 forge blueprint 流程作为"产品"，与 forge 数据模型对齐）
- `plan-shelflife-v1.md`（已归档；工具是否有耐久度 / 损耗——见 §6 开放问题）
- `plan-zhenfa-v1.md`（active；阵法布置不需工具——但布置笔可作为后续 v2 扩展）
- `plan-combat-no_ui.md`（异体排斥真伤口 emit 通道——空手采触发的 LACERATION / FRACTURE 走现有 wound_event_emit 链路）

**阶段总览**：
- P0 ⬜ 7 件工具 item toml + `ToolKind` enum + `ToolTag` component + 主手识别 helper（采集 / 制作 session 调用入口）
- P1 ⬜ 工具 forge blueprint 接入（每件工具一份 blueprint：材料 + 步骤 + 难度）
- P2 ⬜ botany-v2 WoundOnBareHand 真伤回填：玩家空手 / 错工具采 v2 高阶物种 → 触发 §四 既有 6 档伤口模型 LACERATION / FRACTURE 档（依 plan-botany-v2 P4 节奏）
- P3 ⬜ fauna 屠宰会话：骨骸钳 / 屠宰刀从异兽尸体取骨（drop 链可选分支，依 fauna-v1 P0）
- P4 ⬜ 工具耐久度 / 损耗（接 plan-shelflife-v1，可选；如最终决定不引耐久则 P4 删）
- P5 ⬜ worldview §四 末尾补 "§四.X 凡器与凡夫战力" 小节（正典化凡器边界，可选）

**接入面**（按 docs/CLAUDE.md "防孤岛" checklist）：
- **进料**：`inventory::ItemCategory`（需扩 `Tool` variant）+ `inventory::ItemInstance`（持有工具的 instance_id）+ botany / mineral / fauna 各自的 `required_tool` 字段
- **出料**：`tools::ToolKind` enum（7 variant）+ `tools::ToolTag` component + `tools::item_kind_to_tool` query → 各采集模块消费；空手采高阶物种 → 走 §四 wound emit 链路
- **共享类型**：复用 `ItemCategory` / `ItemInstance` / `WoundEvent`（既有，不新建）；**新建** `ToolKind` enum / `ToolTag` component / `tools` 模块
- **跨仓库契约**：
  - server: `tools::ToolKind` (CaiYaoDao / BaoChu / CaoLian / DunQiJia / GuaDao / GuHaiQian / BingJiaShouTao) / `tools::ToolTag` / `tools::item_kind_to_tool(item_id)` query
  - schema: 无新增（凡器不发独立 IPC——通过 inventory snapshot 同步即可）
  - client: 无新增（凡器在 inspect 屏的 inventory tab 显示，复用 InventoryStateStore）
  - agent: 无新增（凡器无 narration 触发——除了 P2 真伤事件走现有 wound narration）
- **Redis channel**: 无新增

---

## §0 设计轴心

- [ ] **凡器 ≠ 法器**：凡器无真元加持，纯物理刃；与 worldview §四 暗器流的"异变兽骨 / 灵木"优良载体在物质层就划清——凡器命名禁用"灵\*"词头，避免越级
- [ ] **采集即赌博**：高阶植物 / 矿物 / 兽骨**强制要求工具**（required_tool=Some）；空手采 → 按 §四 既有 6 档伤口模型触发真伤口（LACERATION / FRACTURE）+ §四 异体排斥（毒素 / 残留真元污染）
- [ ] **凡夫日用**：凡器是末法残土所有修士（包括醒灵期凡夫）的常态装备；不分流派、不参与战斗 stats（§四 战力分层模型不计凡器）
- [ ] **工具识别 = 主手扫描**：当玩家发起采集 / 制作 session，server 检查主手 ItemInstance 的 ToolKind→ 决定 dispersal_chance / wound_chance / drop_chance
- [ ] **不入 hotbar**：凡器走 inventory 主手装备槽（沿用 weapon-v1 §3 hotbar 解耦逻辑）；hotbar 仅放消耗品 / 技能卷轴
- [ ] **耐久度可选**：v1 不强制耐久（§4 资源闭环已有"工具消耗矿物 / 兽骨制作"代价）；耐久作为 P4 接 shelflife 的可选扩展，最终决定再留删
- [ ] **凡器命名禁灵字头**：与 worldview §四 暗器流的"灵\*"词头物体（灵铁砧 / 灵锋剑 / 灵木）划清——凡器是"采药刀 / 刨锄 / 骨骸钳"等凡俗手作名

---

## §1 凡器 vs 法器边界（worldview 锚定）

| | 凡器（本 plan） | 法器 / 暗器流载体（§四） |
|---|---|---|
| 真元加持 | 无 | 有（封入 80% 真元） |
| 材质 | 凡铁 / 木石 / 凡兽骨 | 异变兽骨 / 灵木 |
| 命名 | 凡俗手作名（采药刀 / 骨骸钳 / 钝气夹） | "灵\*"词头（灵铁砧 / 灵锋剑 / 灵木） |
| 用途 | 采集 / 制作 / 屠宰（非战斗） | 攻击 / 防御（战斗核心） |
| 战斗参与 | 不参与 §四 战力分层模型 | 参与（base_attack / quality_tier） |
| 工艺难度 | 低（forge blueprint 标准 step） | 高（封灵阵 + 真元注入） |
| 耐久 | 可选（v1 不做） | durability + max + 破损降级 |

> **凡器在 worldview 的位置**：§四 暗器流明确"凡铁 / 木石（劣质）：飞 10 格损失 75% 真元"——这就是凡器档位。凡器**配不上**锁住真元的优良载体级别，所以它们的语义就是"工具"而非"法器"。

---

## §2 7 件初版工具清单

| template_id | 显示名 | ToolKind | 主用途 | 材料档（来源） | 占格 | 加成场景 |
|---|---|---|---|---|---|---|
| `cai_yao_dao` | 采药刀 | `CaiYaoDao` | 采集草本植物 | 凡铁（mineral-v2） | 1×1 | botany 基础物种 dispersal_chance=1.0 |
| `bao_chu` | 刨锄 | `BaoChu` | 翻土 / 挖根 | 凡铁 + 木柄（spiritwood 凡木档） | 1×2 | botany 根系类 + 灵田初始翻土 |
| `cao_lian` | 草镰 | `CaoLian` | 割草 / 大批量收割 | 凡铁 | 1×2 | botany 草本批量采集 + 灵田收割 |
| `dun_qi_jia` | 钝气夹 | `DunQiJia` | 安全捏取剧毒 / 锐刺植物 | 凡铁 + 皮革 | 1×1 | botany v2 高阶毒草（无此具空手必触发 §四 异体排斥） |
| `gua_dao` | 刮刀 | `GuaDao` | 刮树皮 / 取脂 / 剥矿膜 | 凡铁（细刃） | 1×1 | botany 树皮类 + mineral 矿膜剥离 |
| `gu_hai_qian` | 骨骸钳 | `GuHaiQian` | 从兽尸取骨 / 拆筋 | 凡铁 + 异兽骨柄（**fauna-v1 阻塞**） | 1×2 | fauna 屠宰会话核心工具 |
| `bing_jia_shou_tao` | 冰甲手套 | `BingJiaShouTao` | 处理低温 / 寒系植物 | 凡铁衬 + 寒兽皮（**fauna-v1 阻塞**） | 1×1 | botany v2 寒系物种 + 矿物冷敷处理 |

**工艺命名**：均为凡俗手作名，无"灵\*"词头（worldview §四 暗器流"灵\*"档 ≠ 凡器档）。

---

## §3 ToolKind enum + ToolTag component

```rust
// server/src/tools/kinds.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolKind {
    CaiYaoDao,        // 采药刀
    BaoChu,           // 刨锄
    CaoLian,          // 草镰
    DunQiJia,         // 钝气夹
    GuaDao,           // 刮刀
    GuHaiQian,        // 骨骸钳
    BingJiaShouTao,   // 冰甲手套
}

// server/src/tools/components.rs
#[derive(Component, Debug, Clone)]
pub struct ToolTag {
    pub kind: ToolKind,
    pub instance_id: u64,  // 回指 inventory 里的 ItemInstance
    // v1 不带 durability 字段；P4 引入耐久时扩展
}

// server/src/tools/registry.rs
pub fn item_kind_to_tool(item_id: &str) -> Option<ToolKind> {
    match item_id {
        "cai_yao_dao" => Some(ToolKind::CaiYaoDao),
        "bao_chu" => Some(ToolKind::BaoChu),
        // ... 7 件
        _ => None,
    }
}

pub fn main_hand_tool(player: Entity, world: &World) -> Option<ToolKind> {
    let inv = world.get::<PlayerInventory>(player)?;
    let main_hand_id = inv.equipped.main_hand?;
    let template_id = inv.find_instance(main_hand_id)?.template_id.as_str();
    item_kind_to_tool(template_id)
}
```

下游模块（botany / mineral / fauna / forge）通过 `main_hand_tool(player)` query 检查主手工具，决定采集 / 制作 / 屠宰判定。

---

## §4 inventory 集成

### 4.1 ItemCategory 扩展

worldview 锚定凡器为"日用品"——本 plan 在 inventory 的 `ItemCategory` 加 `Tool` variant：

```rust
// server/src/inventory/mod.rs:138-145（修改）
pub enum ItemCategory {
    Pill,
    Herb,
    Weapon,
    Treasure,
    BoneCoin,
    Tool,          // 新增
    Misc,
}

// parse_item_category 加分支（行 1155-1168）
"tool" => ItemCategory::Tool,
```

### 4.2 装备槽兼容

凡器走 `equipped.main_hand`（沿用 weapon-v1 §3 装备槽规则），与武器互斥但**不参与战斗 stats**——战斗 resolve 时检查 `Weapon` component 而非 `Tool`：

```rust
// server/src/combat/resolve.rs（已有，不改）
let weapon_mul = world.get::<Weapon>(attacker)
    .map(Weapon::damage_multiplier)
    .unwrap_or(1.0);  // 主手是凡器或空手 → 拳套基线 1.0
```

凡器主手时 weapon component 不存在 → damage_multiplier 走默认拳套基线（1.0）。这是 worldview §四 "赤手可战"原则的自然延伸——凡器≠战斗武器。

### 4.3 采集 / 制作 session 接入

botany / mineral / fauna 各自定义 `required_tool` 字段，session 启动时检查：

```rust
// 示例：botany v2 PlantHarvestSession
struct HarvestRequest {
    plant_id: PlantId,
    required_tool: Option<ToolKind>,
    wound_chance_no_tool: f32,  // 无工具空手采的真伤概率
    dispersal_chance_no_tool: f32,  // 无工具空手采的散失概率
}

// resolve 时
fn resolve_harvest(req: HarvestRequest, player: Entity, world: &World) -> HarvestResult {
    let tool = main_hand_tool(player, world);
    match (req.required_tool, tool) {
        (Some(req_kind), Some(actual)) if req_kind == actual => HarvestResult::Success,
        (Some(_), _) => {
            // 无对应工具空手 / 错工具
            // 触发 §四 6 档伤口（LACERATION / FRACTURE）+ §四 异体排斥
            emit_wound_event(player, WoundKind::Laceration, BodyPart::Hand);
            emit_异体_event(player, ContaminationDelta::small());
            HarvestResult::PartialOrFailed { dispersal: req.dispersal_chance_no_tool }
        }
        (None, _) => HarvestResult::Success,  // 不要求工具的物种
    }
}
```

---

## §5 真伤口 + 异体排斥（worldview §四 沿用）

**核心原则**：本 plan **不引入新伤口概念**——空手采高阶植物 / 矿物 / 兽骨触发的伤害**完全走 worldview §四 既有 6 档伤口模型**：

| 物种类 | 空手采伤口档 | 异体排斥强度 | 真元污染 |
|---|---|---|---|
| 普通植物 | INTACT（不触发） | 0 | 0 |
| botany v2 高阶毒草（夜枯藤等） | LACERATION（割裂） | 中（毒素污染） | 0.1-0.3 |
| botany v2 寒系物种 | ABRASION（擦伤）+ 体表冰碴 | 弱 | 0 |
| botany v2 锐刺物种 | FRACTURE（指骨折） | 0 | 0 |
| 异兽尸体取骨（无骨骸钳） | LACERATION + 异种残留真元 | 强（兽骨残留） | 0.3-0.5 |
| 矿膜剥离（无刮刀） | ABRASION | 0 | 0 |

> **关键**：所有伤口档 / 部位 / 异体排斥流程都已在 `plan-combat-no_ui.md` 落地（C1-C3 已 finished），本 plan 只在 botany / mineral / fauna 的 session 调用 `emit_wound_event` 即可。**无新代码新概念**。

---

## §6 平衡考量

### 6.1 工具不参与战斗 = 凡器边界

- 战斗 resolve 不读 `ToolTag`——只读 `Weapon`
- 凡器主手时 damage_multiplier=1.0（拳套基线）
- 这与 worldview §四 "赤手可战"一致——凡器在战斗维度透明

### 6.2 工具与法器同槽 = 战斗准备的取舍

- 同时只能装备一件主手——凡器或法器二选一
- 采集时手持凡器 → 战斗时只能赤手 / 切换装备
- 这强化 worldview §四 "拼刺刀"语境——你随身带的工具/武器组合就是你的战斗准备度

### 6.3 7 件工具的功能不重叠

每件工具对应一类采集 / 制作动作，**不允许"用 A 工具替代 B 工具"**：

- 采药刀 ≠ 草镰（前者单株，后者批量）
- 钝气夹 ≠ 骨骸钳（前者捏剧毒植物，后者拆兽骨）
- 刨锄 ≠ 刮刀（前者翻土，后者剥膜）

→ 设计上玩家**必须备齐 7 件**才能覆盖所有采集场景，符合 worldview §一 "末法残土"的物资匮乏感

### 6.4 耐久 v1 不做的理由

- worldview §十 已有"工具消耗矿物 / 兽骨制作"代价
- 引入耐久会让玩家"反复回去补 forge"——增加不必要的回合 friction
- 凡器在 worldview §四 不参与战斗 stats，耐久不影响战斗时机
- P4 接 shelflife 是可选扩展——如果实测玩家觉得"凡器太耐用"再引入

---

## §7 数据契约（下游 grep 抓手）

### server

- [ ] `inventory::ItemCategory::Tool` variant + `parse_item_category("tool")` 分支 — `server/src/inventory/mod.rs:138-145, 1155-1168`
- [ ] `tools::ToolKind` enum (7 variant) — `server/src/tools/kinds.rs`（新文件）
- [ ] `tools::ToolTag` component — `server/src/tools/components.rs`（新文件）
- [ ] `tools::item_kind_to_tool(item_id: &str) -> Option<ToolKind>` query — `server/src/tools/registry.rs`（新文件）
- [ ] `tools::main_hand_tool(player: Entity, world: &World) -> Option<ToolKind>` helper — 同上
- [ ] `server/src/tools/mod.rs` 模块入口（新文件）

### asset

- [ ] 7 件工具 toml — `server/assets/items/tools/tools.toml`
- [ ] 7 件工具 forge blueprint（P1）— `server/assets/forge/blueprints/tool_*.toml`

### schema / agent / client

- [ ] **无新增**——凡器通过现有 InventorySnapshot / WoundEvent 链路自然同步

### Redis channel

- [ ] **无新增**

---

## §8 实施节点

- [ ] **P0**：基础数据结构 + 7 件工具 toml
  - `inventory::ItemCategory::Tool` + `parse_item_category` 分支
  - `tools::ToolKind` enum + `ToolTag` component + `item_kind_to_tool` + `main_hand_tool`
  - `server/src/tools/{mod, kinds, components, registry}.rs` 4 个新文件
  - 7 件工具 toml（5 件无外部依赖：cai_yao_dao / bao_chu / cao_lian / dun_qi_jia / gua_dao；2 件 fauna 阻塞用占位 ID 写）
  - 单测：ItemCategory parse 7 个 → Tool / ToolKind 反查 / main_hand_tool 主手识别 / 主手非凡器返回 None
  - **fauna-v1 阻塞**：bing_jia_shou_tao / gu_hai_qian 工具的兽骨 / 寒兽皮材料用占位 ID（fauna-v1 P0 落地后回填）

- [ ] **P1**：forge blueprint
  - 7 份 forge blueprint toml（材料 + 步骤 + 难度）
  - 凡铁 / 凡木 / 凡皮等基础材料的 blueprint 接入
  - 单测：每件工具 blueprint 走完 forge 流程产出对应 ItemInstance

- [ ] **P2**：botany-v2 WoundOnBareHand 真伤回填（接 plan-botany-v2 P4）
  - botany v2 高阶物种 HarvestSession 加 `required_tool: Option<ToolKind>` 字段
  - 空手 / 错工具采 → 按 §5 表格触发 wound_event_emit + 异体_event_emit（沿用 §四 既有伤口 + 异体排斥模型）
  - 单测：钝气夹采夜枯藤成功 / 空手采夜枯藤触发 LACERATION / 错工具（采药刀采兽骨）触发 FRACTURE
  - **botany-v2 阻塞**：本 P 依 botany-v2 P4 节奏；botany-v2 P0-P3 时本 P 用占位 dispersal_chance=1.0 不阻塞

- [ ] **P3**：fauna 屠宰会话（接 plan-fauna-v1）
  - 异兽尸体 + 主手骨骸钳 / 屠宰刀 → 启动 ButcherSession
  - 工具决定 drop 链（骨骸钳 → 异兽骨；屠宰刀 → 兽肉/兽皮）
  - 无工具空手拆 → LACERATION + 异种残留真元污染
  - 单测：骨骸钳拆兽骨成功 / 空手拆触发污染 + 真伤
  - **fauna-v1 阻塞**：本 P 依 fauna-v1 P0 异兽尸体 entity

- [ ] **P4**：工具耐久度（接 plan-shelflife-v1，可选）
  - `ToolTag.durability_current / max` 字段（引入则改 P0 component）
  - 每次采集 / 制作 session 完成 → durability -= 1（可调）
  - durability=0 → 主手凡器 broken，不再生效
  - shelflife 接入：凡器 DecayProfile（凡铁档——慢衰减）
  - 单测：100 次采集后 durability=0 / broken 后空手判定 / forge 修复
  - **决策点**：本 P 启动前先实测 P0-P3 玩家对"凡器太耐用"的反馈；若反馈不强烈则 P4 删

- [ ] **P5**：worldview §四 末尾补 "§四.X 凡器与凡夫战力" 小节（可选正典化）
  - 在 worldview §四 末尾加段落，明确：
    - 凡器 = §四 暗器流分级"凡铁 / 木石"档的工具化
    - 凡器不参与 §四 战力分层（不计入 base_attack）
    - 凡器命名禁"灵\*"词头（与暗器流"灵\*"载体划清）
    - 空手采高阶物种触发 §四 6 档伤口模型 + 异体排斥
  - 关联 §九 经济与交易（凡器流通价位档）+ §十 资源与匮乏（凡器材料消耗）
  - **此为 worldview 修改 PR，单独提交，不能在本 plan 内自动改**

---

## §9 开放问题

- [ ] 凡器是否能在战场上"反手当武器"？v1 不做（damage_multiplier=1.0 拳套基线）；但 worldview §四 "拼刺刀"原话允许任何物体反手用——后续可考虑给凡器一个 `improvised_weapon: bool` 字段允许低伤反击
- [ ] 7 件工具是否覆盖所有采集场景？mineral-v2 已有 MiningSession 不依赖凡器（v1 直接 BlockBreakEvent drop）——是否要在 v2 引入凡器门槛由 mineral-v2 决定，本 plan 不规定
- [ ] 工具是否有"质量分级"（凡铁 / 精铁 / 异变兽骨 工艺）？v1 全部凡铁档；高阶工艺留 v2 扩展
- [ ] 多人协作采集（一人持骨骸钳一人空手）：v1 不做"工具持有者得 drop"判定，session 主玩家持工具即视为成功
- [ ] 工具掉地后的 ItemInstance 行为：复用 inventory-v1 `DroppedLootRegistry`，无特殊处理
- [ ] 凡器在 forge 修复时是否有"工艺累积"加成（同一工匠多次修同种工具效率提升）？留 forge v2

---

## §10 进度日志

- **2026-04-29**：骨架立项，从 reminder.md 提炼。承接 plan-botany-v2 / plan-mineral-v2 / plan-fauna-v1 / plan-forge-v1 的 `required_tool` 声明。
- **2026-04-30**：从 skeleton 升 active（commit c5ea3e03）。`/plans-status` 调研评级 ⚠️ 弱阻塞——P0 可推进，P2 反向阻塞 botany-v2 P0。修正 worldview 锚点：删除虚指 §三 末法命名原则（worldview §三 实际是修炼体系），改为 §四 战斗系统 / §五 战斗流派 / §十 资源与匮乏 实指。
- **2026-04-30**（重写）：核心认知调整——凡器**不是新概念**，在 worldview §四 暗器流的"凡铁 / 木石（劣质）"档里早已有支撑。重写后 plan 不再"创造凡器概念"，而是把凡器作为 worldview 既有"载体材质分级"的工具化延伸落地。空手采真伤完全走 §四 既有 6 档伤口模型，不引入新伤口概念。新增 §1 "凡器 vs 法器边界"对照表锚定 worldview，§5 "真伤口 + 异体排斥（worldview §四 沿用）"明确不新建概念。新增 P5 worldview 正典化小节作为可选后续。
