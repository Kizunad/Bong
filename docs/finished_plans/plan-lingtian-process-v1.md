# Bong · plan-lingtian-process-v1

**作物二级加工**（晾晒 / 碾粉 / 炮制 / 萃取）。在 plan-lingtian-v1 收获 ci_she_hao / ning_mai_cao / ling_mu_miao 等"原作物"基础上，加一层后处理产物作为 alchemy 丹方与日常消耗的中间形态。区分"鲜采直接投料"（损耗大、品质低）vs "炮制后投料"（损耗小、品质加成）。

**世界观锚点**：
- `worldview.md §十` 灵气零和——加工不无中生有灵气，只重新分配作物 quality 系数到产物
- `worldview.md §十二` 末法噬蚀（寿元 / 衰减节）——鲜采作物若不及时加工 / 收纳，72h 内灵气流失到 0（鼓励玩家加工锁定）
- `worldview.md §十七` 末法节律：夏冬二季——加工产物的 freshness 衰减速率与季节耦合（夏 ×1.5 / 冬 ×0.7 / 汐转 RNG ±30%）；详见 §4
- `worldview.md §六` 真元只有染色谱——**禁止**"火炒火稻 → 火属性药粉"五行联动

**library 锚点**：`docs/library/ecology/末法药材十七种.json`（每种药材的传统炮制法描述）· 待写 `docs/library/peoples/XXXX 末法炮制录.json`（各加工流程的世界观说明 + 工艺传承）

**交叉引用**：
- `plan-lingtian-v1.md`（active）—— herb item 输入端，PlotEnvironment 已有
- `plan-alchemy-v1.md`（已归档）—— 加工产物作为 pill_recipe 的高品质投料；本 plan P4 在 alchemy 已有框架上接入
- `plan-forge-v1.md`（已归档）—— 炮制器具（丹炉炮制模式）已有 forge 框架可挂入；本 plan P2 走该路径
- `plan-skill-v1.md`（已归档）—— herbalism / alchemy 双技艺，各加工类型有偏向
- `plan-inventory-v1.md`（active）—— 鲜采作物 vs 加工产物有不同保鲜度
- `plan-shelflife-v1.md`（active）—— freshness 衰减 profile（Linear / Exponential）；本 plan 复用 profile 注册机制
- `plan-anqi-v1.md`（骨架）—— 灵气囊锁鲜（freshness 流失 ×0.3）；P1 接入
- `plan-lingtian-weather-v1.md`（同期升 active）—— 季节修饰 freshness 衰减速率；P1 联动
- `plan-alchemy-recycle-v1.md`（骨架）—— 加工失败产物 / 枯样的反哺路径；P4 hook，不在本 plan 实装

**阶段总览**：

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | 晾晒 + 碾粉两类工艺 + `ProcessingKind` enum + `ProcessingSession` + `ProcessingRecipeRegistry` + 单测 ≥ 12 条 | ⬜ |
| P1 | `FreshnessTracker` Component + `freshness_tick_system`（game-tick 驱动）+ 季节耦合（worldview §十七）+ 灵气囊接入 plan-anqi-v1 | ⬜ |
| P2 | 炮制 + 萃取（forge `processing_mode` 接入；plan-forge-v1 已归档，框架可用） | ⬜ |
| P3 | client `ProcessingActionScreen` + HUD 进度 + freshness UI tag + schema `ProcessingSessionDataV1` 双端镜像 | ⬜ |
| P4 | alchemy 接入：加工产物作为 pill_recipe 的优选投料，给品质 / 成功率加成 + alchemy-recycle-v1 hook（枯样反哺） | ⬜ |

---

## §0 设计轴心

- [ ] 加工 = **作物 quality 转化器**：原作物 quality_accum [0.8, 1.5] → 加工产物 quality + duration_buff
- [ ] 4 类工艺（晾晒 / 碾粉 / 炮制 / 萃取）—— 每类一种器具、一种典型成品形态
- [ ] **保鲜度走 game-tick**（用户决策 2026-04-29）：仅服务器在线 tick 时推进 freshness——离线即停，回线续播。优势：不需持久化 wall-clock 时间戳、多人服务器累积时间逻辑天然一致、与 plan-shelflife-v1 / plan-lingtian-weather-v1 的 game-tick 模型一致。代价：单机玩家长时间不上线时作物不衰减（接受——这就是末法残土的"封闭世界"特性）
- [ ] **季节耦合**：freshness 衰减速率乘以 `Season::freshness_multiplier()`（夏 ×1.5 / 冬 ×0.7 / 汐转 RNG ±30%）—— worldview §十七 锚定
- [ ] 加工本身有失败率 / 损耗率，受 herbalism / alchemy 技艺等级影响
- [ ] 不引入"加工 → 加工 → 加工"链——最多 2 级（原 → 加工成品）
- [ ] **加工 session 期间 freshness 冻结**（防止玩家被迫"快加工"卡帧）

---

## §1 第一性原理（烬灰子四论挂点）

- **缚论·锁灵气**：鲜采作物的灵气尚未"定形"，会被天地噬散；加工 = 用工艺把灵气固定在产物结构里（晾晒去水分 / 萃取浓缩 / 炮制改性）
- **噬论·加工损耗**：加工过程同样被天地噬蚀——所以一定有 quality 损失或材料损耗
- **音论·器具偏向**：不同器具的"音"决定加工产物的属性（石臼 = 朴实色偏向 / 丹炉炮制 = 灼烈色偏向 / 萃瓶 = 凝实色偏向 / 晾架 = 自然色偏向）
- **影论·副本固化**：加工后的产物 = 原作物的"副本镜印"，不再是活物，所以可长期保存

---

## §2 4 类加工工艺

| 工艺 | 器具 | 输入 | 输出 | 时间（real-time + game-tick）| 主导技艺 |
|---|---|---|---|---|---|
| **晾晒**（drying）| 晾架（户外，需阳光 tick） | 鲜采草本 ×N | 干品（保鲜 14 game-day，quality ×0.9）| 1 in-game 日 ≈ 20 real-min | herbalism |
| **碾粉**（grinding）| 石臼 | 干品 ×N | 药粉（投料效率 +20%，保鲜 7 game-day）| 30 real-second / 单位（≈ 600 ticks）| herbalism |
| **炮制**（forging_alchemy）| 丹炉炮制模式（forge 接入）| 干品 / 鲜品 ×N + qi 5 | 炮制品（quality ×1.2，alchemy 加 +1 档成功率）| 5 real-min / 批（≈ 6000 ticks）| alchemy |
| **萃取**（extraction）| 萃瓶（高阶器具）| 鲜品 ×3 | 萃液（量小但 quality ×2.0）| 10 real-min / 批（≈ 12000 ticks）| alchemy + herbalism |

**game-tick 驱动**：所有工艺时间以 server tick 计（1 second = 20 ticks，1 game-day = 24000 ticks）；离线即暂停，session_id + 已完成 ticks 持久化到玩家数据。

---

## §3 数值梯度（按技艺等级）

```
herbalism Lv.0 ~ Lv.2：仅晾晒 / 碾粉，失败率 30% / 损耗 2 单位（仅查看配方，不可启动炮制 session）
herbalism Lv.3 ~ Lv.4：失败率 10% / 损耗 1 单位 + 解锁炮制查看（仍不可启动）
herbalism Lv.5+ + alchemy Lv.3+：解锁炮制启动权
herbalism Lv.6 ~ Lv.9 + alchemy Lv.3+：解锁萃取
alchemy Lv.3+：炮制成品 quality 加成 +0.1
alchemy Lv.6+：萃液产量 ×1.5
```

技艺与 plan-skill-v1 共用 progression。

---

## §4 保鲜度机制（plan-inventory-v1 / plan-shelflife-v1 / plan-lingtian-weather-v1 接入）

### 4.1 衰减模型

- **鲜采作物**：`Linear` profile，72 game-hour（≈ 60 real-hour 在线时间）线性 1.0 → 0.0
- **干品**：`Exponential` profile，half_life ≈ 14 game-day
- **药粉**：`Exponential` profile，half_life ≈ 7 game-day
- **炮制品**：`Linear` profile，30 game-day 1.0 → 0.0（季节修饰仍生效）
- **萃液**：`Exponential` profile，half_life ≈ 3 game-day（量小品质高 → 半衰短，逼迫玩家"打 boss 前嗑"）

### 4.2 季节修饰（worldview §十七）

```rust
impl Season {
    pub fn freshness_multiplier(self) -> f32 {
        match self {
            Summer => 1.5,                    // 夏散——衰减加速
            Winter => 0.7,                    // 冬聚——衰减减缓
            SummerToWinter | WinterToSummer => {
                // 汐转 RNG ±30%
                let r = thread_rng().gen::<f32>();
                0.7 + r * 0.6  // 0.7..1.3
            }
        }
    }
}
```

freshness_tick 每个 game-day 取一次 multiplier（避免每 tick RNG 抖动太剧烈）。

### 4.3 关键状态转换

- **freshness == 0** → item 转为"枯样"（quality ×0.3，仍可投料但效率极低；plan-alchemy-recycle-v1 可反哺）
- **进入加工 session** → freshness 冻结（防止"快加工"卡帧 meta）
- **加工完成** → 产物自带新 freshness 时长（profile 由 §4.1 决定）
- **存入"灵气囊"**（plan-anqi-v1 灵物锁鲜）→ freshness 流失速率 ×0.3（与季节修饰相乘）

---

## §5 数据契约（下游 grep 抓手）

### 5.1 Server (Rust)

```rust
// server/src/lingtian/processing.rs（新文件）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessingKind {
    Drying,         // 晾晒
    Grinding,       // 碾粉
    ForgingAlchemy, // 炮制（forge 接入）
    Extraction,     // 萃取
}

#[derive(Component, Debug)]
pub struct ProcessingSession {
    pub player: Entity,
    pub kind: ProcessingKind,
    pub input_items: Vec<ItemStack>,        // 冻结快照，session 期间不变
    pub recipe_id: String,
    pub started_at_tick: u64,
    pub expected_duration_ticks: u32,       // §2 表
    pub progress_ticks: u32,
}

#[derive(Resource, Default)]
pub struct ProcessingRecipeRegistry {
    by_id: HashMap<String, ProcessingRecipe>,
}

pub struct ProcessingRecipe {
    pub id: String,
    pub kind: ProcessingKind,
    pub inputs: Vec<RecipeInput>,           // item_id + count + optional mineral_id
    pub outputs: Vec<RecipeOutput>,         // item_id + count + quality_multiplier
    pub duration_ticks: u32,
    pub skill_req: SkillRequirement,        // herbalism / alchemy 等级
    pub failure_rate: f32,                  // 0.0..1.0
    pub failure_output: Option<RecipeOutput>, // 失败时产出（如"焦料"），None 视为损耗
}

pub fn processing_session_tick_system(/* ... */) {
    /* 每 tick 推进 progress_ticks，到达 duration → 抽 RNG 决定成功/失败 → 产出 → freshness 解冻 */
}

// server/src/inventory/freshness.rs（新文件）
#[derive(Component, Clone, Debug)]
pub struct FreshnessTracker {
    pub profile_name: String,           // 引用 plan-shelflife-v1 注册的 profile
    pub born_at_tick: u64,
    pub initial_qi: f32,
    pub current_freshness: f32,         // 0.0..1.0，缓存值，每 game-day 重算
    pub frozen_until_tick: Option<u64>, // session 期间冻结
}

pub fn freshness_tick_system(/* ... */) {
    /* 每 game-day（24000 ticks）触发一次：
       1. 跳过 frozen_until_tick > now 的 entry
       2. 取 current Season（plan-lingtian-weather-v1 ZoneSeasonState）
       3. 计算 multiplier = season.freshness_multiplier() * (in_anqi ? 0.3 : 1.0)
       4. 按 profile 公式衰减 current_freshness
       5. freshness <= 0 → 转 "withered"（item_id 替换 + quality ×0.3） */
}

// server/src/forge/processing_mode.rs（新文件，扩展 forge）
pub fn forge_processing_mode_handler(/* ... */) {
    /* 丹炉炮制模式入口——P2 启动；前置 plan-forge-v1（已归档）框架已有 */
}
```

### 5.2 配方格式（TOML）

```toml
# server/assets/recipes/processing/dry_ci_she_hao.toml
id = "dry_ci_she_hao"
kind = "drying"
duration_ticks = 24000  # 1 game-day

[[inputs]]
item_id = "ci_she_hao"
count = 5
min_freshness = 0.5     # 鲜度低于 0.5 不可入晾架

[[outputs]]
item_id = "dry_ci_she_hao"
count = 5
quality_multiplier = 0.9
freshness_profile = "drying_v1"  # 引用 shelflife profile

[skill_req]
herbalism = 0  # 不限等级

failure_rate = 0.30  # Lv.0-2 玩家面对的失败率（base，技艺加成在 runtime 计算）

[failure_output]
item_id = "withered_dry_ci_she_hao"  # 焦料
count = 3
```

### 5.3 Schema (agent ↔ server / server ↔ client)

```typescript
// agent/packages/schema/src/processing.ts
export const ProcessingKindV1 = Type.Union([
  Type.Literal("drying"),
  Type.Literal("grinding"),
  Type.Literal("forging_alchemy"),
  Type.Literal("extraction"),
]);

export const ProcessingSessionDataV1 = Type.Object({
  session_id: Type.String(),
  kind: ProcessingKindV1,
  recipe_id: Type.String(),
  progress_ticks: Type.Integer({ minimum: 0 }),
  duration_ticks: Type.Integer({ minimum: 0 }),
  player_id: Type.String(),
});

export const FreshnessUpdateV1 = Type.Object({
  item_uuid: Type.String(),
  freshness: Type.Number({ minimum: 0, maximum: 1 }),
  profile_name: Type.String(),
});
```

### 5.4 Client (Java / Fabric)

```java
// client/src/main/java/.../processing/ProcessingActionScreen.java
public class ProcessingActionScreen extends Screen {
    /** 4 类工艺统合浮窗：左侧选 kind → 中央 inputs → 右侧 outputs preview + 进度条 */
}

// client/src/main/java/.../hud/FreshnessTooltipHook.java
public class FreshnessTooltipHook {
    /** ItemStack tooltip 上加 "鲜度: X/100" + Linear/Exponential profile 视觉提示 */
}
```

### 5.5 数据契约表

| 契约 | 位置 |
|---|---|
| `ProcessingKind` enum / `ProcessingSession` Component / `ProcessingRecipeRegistry` Resource | `server/src/lingtian/processing.rs` |
| `processing_session_tick_system` | `server/src/lingtian/processing.rs` |
| `FreshnessTracker` Component | `server/src/inventory/freshness.rs` |
| `freshness_tick_system` (game-tick 驱动 + Season 耦合) | `server/src/inventory/freshness.rs` |
| `forge_processing_mode_handler` | `server/src/forge/processing_mode.rs` |
| 配方 TOML × N | `server/assets/recipes/processing/*.toml` |
| 加工产物 item TOML | `server/assets/items/processed/{dry,powder,processed,extract}_*.toml` |
| `ProcessingKindV1` / `ProcessingSessionDataV1` / `FreshnessUpdateV1` | `agent/packages/schema/src/processing.ts` + Rust 镜像 |
| `ProcessingActionScreen` / `FreshnessTooltipHook` | `client/src/main/java/.../processing/` + `.../hud/` |
| Redis pub: `bong:processing_session_update` / `bong:freshness_update` | server → agent ↔ client |
| shelflife profiles: `drying_v1` / `grinding_v1` / `forging_alchemy_v1` / `extraction_v1` / `fresh_herb_v1` | `server/src/shelflife/registry.rs`（新增注册） |

---

## §6 测试饱和（CLAUDE.md 饱和化测试）

### P0 单测（≥ 12 条）
- `processing_kind_enum_4_variants_distinct`
- `processing_session_progress_tick_increments`
- `processing_session_completion_at_duration`
- `processing_session_freezes_input_freshness`
- `processing_recipe_registry_lookup_by_id`
- `processing_recipe_registry_unknown_id_returns_none`
- `drying_recipe_lv0_failure_rate_30_percent`（statistical: 1000 trial 收敛 0.3 ± 0.05）
- `grinding_recipe_high_skill_lower_failure`
- `processing_session_offline_pause_resume`（game-tick 驱动验证）
- `processing_session_input_quality_multiplier_applied`
- `processing_session_failure_produces_failure_output`
- `processing_session_failure_no_output_when_none`

### P1 单测（≥ 10 条）
- `freshness_tracker_default_initial_value_1_0`
- `freshness_tick_decreases_per_game_day`
- `freshness_tick_skips_frozen_entries`
- `freshness_tick_offline_pauses`
- `freshness_with_summer_multiplier_1_5`
- `freshness_with_winter_multiplier_0_7`
- `freshness_with_tide_multiplier_random_0_7_to_1_3`
- `freshness_in_anqi_multiplier_0_3_combines_with_season`
- `freshness_zero_transitions_to_withered_item`
- `freshness_withered_item_has_quality_0_3`

### P2 e2e（≥ 5 条）
- `forging_alchemy_session_via_dan_furnace`
- `extraction_session_high_quality_low_quantity`
- `forging_alchemy_quality_x1_2_modifier`
- `extraction_quality_x2_0_modifier`
- `cross_skill_req_herbalism_5_alchemy_3_unlocks_extraction`

### P3 e2e（≥ 4 条）
- `client_receives_processing_session_data_payload`
- `client_processing_action_screen_renders_progress_bar`
- `freshness_tooltip_renders_quantitative_value`
- `freshness_update_payload_pushes_on_threshold_change`

### P4 集成（≥ 3 条）
- `alchemy_pill_recipe_with_processed_input_quality_bonus`
- `alchemy_pill_recipe_with_extracted_input_success_rate_bonus`
- `withered_item_routes_to_alchemy_recycle_hook`（plan-alchemy-recycle-v1 hook 验证）

---

## §7 实施节点（详细）

- [ ] **P0**：`ProcessingKind` 4 变体 + `ProcessingSession` Component + `ProcessingRecipeRegistry` + `processing_session_tick_system` + 晾晒 / 碾粉两类工艺的 TOML 配方 ≥ 4 份 + §6 P0 单测全绿（12 条）；不动 freshness；不动 forge / alchemy
- [ ] **P1**：`FreshnessTracker` Component + `freshness_tick_system`（game-tick 驱动）+ Season 耦合（依 plan-lingtian-weather-v1 P0 提供 `Season::freshness_multiplier()` API）+ 灵气囊接入（plan-anqi-v1 hook，stub 即可）+ §6 P1 单测（10 条）
- [ ] **P2**：炮制 + 萃取（forge `processing_mode` 接入；plan-forge-v1 已归档框架可用）+ §6 P2 e2e（5 条）；技艺等级解锁矩阵
- [ ] **P3**：schema 双端镜像 + Redis pub + client `ProcessingActionScreen` + HUD freshness tooltip + §6 P3 e2e（4 条）
- [ ] **P4**：alchemy pill_recipe 加工产物投料品质加成 + 萃液 / 炮制品成功率加成 + alchemy-recycle-v1 hook（枯样反哺）+ §6 P4 集成（3 条）

---

## §8 验收

| 阶段 | 验收条件 |
|---|---|
| P0 | 4 ProcessingKind + session + registry 落地；晾晒 / 碾粉单测全绿；TOML 配方加载 ≥ 4 份 |
| P1 | freshness 衰减按 game-tick 推进；季节修饰命中（夏冬汐转分别测）；离线暂停可验证 |
| P2 | 炮制 / 萃取通过 forge processing_mode；quality 加成 / 成功率加成测得 |
| P3 | client 浮窗 + tooltip 渲染；schema 双端 sample roundtrip 通过 |
| P4 | alchemy 投料品质加成 + 枯样反哺 hook 全部命中；end-to-end：种 ci_she_hao → 收 → 晾晒 → 碾粉 → 入药 → freshness 全程可观测 |

---

## §9 风险与缓解

| 风险 | 缓解 |
|---|---|
| game-tick 驱动 → 单机玩家长时间不上线作物不衰减（"末法残土的封闭世界"特性）| 这是设计意图（用户 2026-04-29 决策 B）；不补救 |
| 季节修饰 ×1.5 / ×0.7 + 灵气囊 ×0.3 + profile half_life 多重叠加，玩家难预测 | freshness tooltip（P3）显示当前 effective half_life；HUD 显示当前 zone 季节；玩家可推算 |
| 萃液 quality ×2.0 + 半衰期 3 game-day → 强行成"打 boss 前嗑" meta | 设计意图——稀缺、易腐、高效是末法残土的合理三连；不平衡 |
| 失败品 "焦料" 大量堆积 inventory | plan-alchemy-recycle-v1 反哺路径（P4 hook） |
| 加工 session 持续数小时（萃取 10 real-min × 多批）阻塞玩家 | session 不阻塞玩家移动——只是 ProcessingSession Component 在 tick；玩家可同时干其他事 |
| FreshnessTracker NBT 持久化负担 | 每 ItemStack 一份 NBT（profile_name + born_at_tick + initial_qi）—— ~32 bytes；可接受 |

---

## §10 开放问题（升 active 后再决议）

- [ ] 加工失败产物（焦料）的具体反哺通道：plan-alchemy-recycle-v1 vs plan-lingtian-v1 复种？P4 启动时与 recycle-v1 共同决议
- [ ] 萃液保鲜短 / 量小 / quality 高，是否会成为"打 boss 前必嗑" meta 物？需要平衡吗？v1 暂不平衡，看玩家反馈
- [ ] 4 类工艺的 UI 是统合一个 Screen 还是各 1 个浮窗？P3 启动时与 plan-HUD-v1 协调
- [ ] 作物"枯样"是否还能拿来反哺 lingtian（plan-alchemy-recycle 衔接）？归 recycle-v1 决策
- [ ] 是否引入"加工连击"（连续做 N 批同一配方时小幅技艺加成）？v1 不引入，避免 grinding meta

---

## §11 进度日志

- **2026-04-27**：骨架创建。前置 `plan-lingtian-v1` ✅；`plan-alchemy-v1` / `plan-forge-v1` 状态待核（炮制依赖之）。
- **2026-04-29**：实地核验修正——`plan-alchemy-v1` / `plan-forge-v1` 已归档（finished_plans/），框架完备；P2 不再受阻。**用户决策 B**（2026-04-29）：freshness tick 走 **game-tick**（离线即停，回线续播），写入 §0 设计轴心 + §4 + §5 + §6 P1。同步 worldview §十七 二季 + 汐转，freshness multiplier 与 plan-lingtian-weather-v1 `Season::freshness_multiplier()` API 耦合。补 `ProcessingSession` struct 草稿、TOML 配方格式、schema、client interface、测试饱和（≥ 34 条）+ Finish Evidence 占位。准备升 active。

---

## Finish Evidence

- 落地清单：
  - P0：`server/src/lingtian/processing.rs` 落地 `ProcessingKind` / `ProcessingSession` / `ProcessingRecipeRegistry` / `processing_session_tick_system`，`server/assets/recipes/processing/{drying,grinding}.toml` 提供晾晒 / 碾粉配方。
  - P1：`server/src/inventory/freshness.rs` 落地 `FreshnessTracker` / `FreshnessEnvironment` / `freshness_tick_system` / `advance_tracker_to_tick`，按 `LingtianClock` game-tick 推进，并通过 resource + tracker flag 接入 Season multiplier 与灵气囊 ×0.3；`server/src/shelflife/registry.rs` 注册 `fresh_herb_v1` / `drying_v1` / `grinding_v1` / `forging_alchemy_v1` / `extraction_v1`。
  - P2：`server/src/forge/processing_mode.rs` 接入丹炉炮制 / 萃取入口，请求验证通过后为玩家挂载 `ProcessingSession`，`server/assets/recipes/processing/{forge_processing,extraction}.toml` 提供炮制 / 萃取配方，技艺矩阵由 `ProcessingSkillLevels` 锁定。
  - P3：`agent/packages/schema/src/processing.ts` + generated JSON + `server/src/schema/processing.rs` 镜像 `ProcessingSessionDataV1` / `FreshnessUpdateV1`；`ServerDataProcessingSessionV1` / `ServerDataFreshnessUpdateV1` 保持 closed schema；`ProcessingSessionDataV1.active` 可清空客户端 session；`client/src/main/java/com/bong/client/processing/ProcessingActionScreen.java`、`FreshnessTooltipHook.java`、`ProcessingServerDataHandler.java` 接入进度和 freshness UI。
  - P4：`server/src/alchemy/processed_input.rs` 落地加工产物品质 / 成功率加成与 `alchemy_recycle_v1` 枯样 hook。
- 关键 commit：
  - `7d2abd530686b09e749efe1a5a27e1fd8f6b0804`（2026-05-06）：实现 plan-lingtian-process-v1 服务端加工核心
  - `e042dbce975c570ed084fd1231ae80c0ee729e02`（2026-05-06）：接入 plan-lingtian-process-v1 加工契约
  - `c23aff67c4e1842453c0594a92e9df2867e24e21`（2026-05-06）：补齐 plan-lingtian-process-v1 客户端加工面板
  - `98b48d134ba8060366326400c6687403c0dbdc93`（2026-05-06）：补齐 review 验证面，覆盖 `ProcessingStartError` 变体、closed schema 与 forge session spawn
- 测试结果：
  - `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`（server）：2429 passed
  - `npm run generate -w @bong/schema`（agent）：281 schemas exported，生成后无额外 diff
  - `npm run build && npm test -w @bong/schema && npm test -w @bong/tiandao`（agent）：schema 271 passed；tiandao 236 passed
  - `source "$HOME/.sdkman/bin/sdkman-init.sh" && sdk use java 17.0.18-amzn && ./gradlew test build`（client）：BUILD SUCCESSFUL（Java 17.0.18-amzn）
  - `git diff --check`：通过
- 跨仓库核验：
  - server：`ProcessingKind` / `ProcessingSession` / `ProcessingRecipeRegistry` / `validate_processing_start`（覆盖 5 个 `ProcessingStartError` 变体）/ `FreshnessTracker` / `FreshnessEnvironment` / `freshness_tick_system` / `forge_processing_mode_handler` / `processed_alchemy_bonus`
  - agent：`ProcessingKindV1` / `ProcessingSessionDataV1` / `FreshnessUpdateV1` / `ServerDataProcessingSessionV1` / `ServerDataFreshnessUpdateV1`
  - client：`ProcessingActionScreen` / `ProcessingSessionStore` / `FreshnessStore` / `FreshnessTooltipHook` / `ProcessingServerDataHandler`
- 遗留 / 后续：
  - 启动加工的 client intent 与 inventory/forge 交互动作留给后续切片；本 plan 先固定 UI/进度/freshness 数据面。
  - Season 真实 zone weather 状态读取与灵气囊实体判定留给 `plan-lingtian-weather-v1` / `plan-anqi-v1` 深接入；当前提供可测试 multiplier hook。
  - 枯样反哺只落 `alchemy_recycle_v1` hook，不在本 plan 内实现完整回收经济。
