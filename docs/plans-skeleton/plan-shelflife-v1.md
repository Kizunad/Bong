# Bong · plan-shelflife-v1 · 骨架

**通用保质期 / 过期系统**（原 `plan-volatility-v1`，升格扩展）。末法时代一切都在衰败，但**陈化酒**、**老坛丹**等上古封印产物反例 = 时间亦可积淀。本 plan 以**一套 NBT + 一套 lazy eval** 承载三条不同降级路径，供所有"有时间敏感性"的物品（矿物 / 兽产 / 草药 / 丹药 / 食物 / 工艺品）import。

**世界观锚点**：`worldview.md L63`（末法命名 — 一切物品带衰败意象）· `worldview.md §六 L518`（骨币封印真元 — 陈化/冻结的正例）· `worldview.md §六 L557`（矿脉有限 — 消费侧衰败放大稀缺）· `worldview.md §九 L429`（鲸落遗骸 — 时间尺度极长的封印陈化意象）。

**交叉引用**：
- `plan-mineral-v1.md`（灵石走 Decay 路径）
- `plan-fauna-v1.md`（待立 — 骨币 Decay + 冻结 / 兽血 Spoil / 内丹混合）
- `plan-botany-v1.md`（鲜草 Spoil / 阴干 Stepwise Decay / 陈年灵茶可 Age）
- `plan-alchemy-v1.md`（丹药 Spoil 为主 / 老坛灵丹走 Age）
- `plan-food-v1.md`（待立 — 食物 Spoil 主流 / 陈酒陈醋 Age）
- `plan-persistence-v1.md`（lazy freshness 快照兼容）
- `plan-economy-v1.md`（待立 — 死物/腐败品/陈化品的次级市场经济）

---

## §0 设计轴心

### 0.1 三条降级路径

| 路径 | 末状态 | 消费后果 | 典型物品 |
|---|---|---|---|
| **Decay（衰减）** | 残值品 / 死物 | 灵气 / 真元含量折算减效，不致伤 | 灵石 → 死灵石 · 骨币 → 腐骨币 · 残卷 → 朽卷 |
| **Spoil（腐败）** | 腐败品 | 消费时触发 contam / 中毒 / alchemy flawed_path | 兽肉 · 兽血 · 鲜草 · 过期丹药 · 食物 |
| **Age（陈化）** | 峰值超值 → 过峰衰败 | 峰值窗口内**增强**，过峰返回 Decay/Spoil 曲线 | 陈酒 · 老坛丹 · 陈年灵茶 · 腌腊物 |

### 0.2 世界观贴合

- [ ] **三路分对世界观的不同侧面**：衰减 = 末法衰败底色；腐败 = 肉身凡俗；陈化 = 时间封印的正例（呼应 §518 骨币封印、§九 429 鲸落 + "末法前酒"的怀古意象）
- [ ] **陈化路径是末法世界稀缺性的逆运动**：挖完就没的矿物 vs 越久越贵的陈酒 — 前者供给端衰败，后者需求端溢价，两极支撑经济张力
- [ ] **三路共享 NBT + lazy eval 基础设施**，物品 spec 单独选路径 + 参数；消费侧按路径分支处理结果

### 0.3 结构原则

- [ ] **lazy evaluation** — 不逐 tick 扫库存，访问时现算（inspect / 消费 / UI snapshot），具体 access 点枚举见 §6.1
- [ ] **封印 = 冻结全路径** — 阵法护匣对 Decay/Spoil/Age 一律暂停，适用于骨币续印 / 药店封存样品
- [ ] **神识感知按路径显示不同语义**（§4）
- [ ] **消费侧后果按路径分支**（§5）

### 0.4 与 inventory plan 的边界约定

- [ ] **本 plan 定义**：`Freshness` NBT struct + `DecayProfile` registry + `compute_*` 纯函数 + `ContainerFreshnessBehavior` enum + Probe / Warning / BonusRoll 三个 event
- [ ] **`plan-inventory-v1` 实现**：在 `InventoryItem` struct 加 `freshness: Option<Freshness>` 字段（`#[serde(default)]`）；在 `Container` trait 实现层加 `freshness_behavior() -> ContainerFreshnessBehavior` 方法
- [ ] **不允许的循环**：本 plan 不引用 `InventoryItem` / `Container` 的具体类型，只定义 trait / data struct，避免 inventory ↔ shelflife 互依

---

## §1 衰减公式（4 档）

> v1 定四档，物品 spec 选一档 + 参数。禁新建第五档，v2+ 再扩。

### 1.1 指数衰减（Exponential Decay）

```
current_qi(t) = initial_qi * 0.5 ^ (dt / half_life_ticks)
```

- 路径：Decay / Spoil 都可挂（参数决定快慢）
- 用于：灵石、兽血、血精、鲜草、丹药衰减段
- 参数：`half_life_ticks`

### 1.2 线性衰减（Linear Decay）

```
current_qi(t) = max(0, initial_qi - decay_per_tick * dt)
```

- 路径：Decay（稳步磨损）
- 用于：骨币封印磨损、残卷纸张老化
- 参数：`decay_per_tick`

### 1.3 阶梯衰减（Stepwise）

```
current_qi(t) = initial_qi * storage_multiplier(current_container)
```

- 路径：Decay / Spoil 都可挂
- 用于：阴干草药（干燥架 1.0 / 箱子 0.7 / 露天 0.3）
- 参数：`storage_multipliers: HashMap<ContainerKind, f32>`

### 1.4 峰值陈化（PeakAndFall — Age 路径专用）

```
if dt < peak_at_ticks:
  current_qi(t) = initial_qi * (1 + peak_bonus * (dt / peak_at_ticks))
else:
  post_peak_dt = dt - peak_at_ticks
  current_qi(t) = initial_qi * (1 + peak_bonus) * 0.5 ^ (post_peak_dt / post_peak_half_life)
```

- 路径：Age
- 用于：陈酒、老坛丹、陈年灵茶
- 参数：`peak_at_ticks`, `peak_bonus`（比如 0.5 = 峰值是初始的 1.5×）, **`peak_window_ratio`**（`Peaking` 窗口宽度 ±ratio × peak，0.1 = ±10%；陈年灵茶可宽 0.2、老坛丹可窄 0.05）, `post_peak_half_life`, **`post_peak_spoil_threshold`**（current_qi 跌至此值时强制路径迁移 Age → Spoil，**仅在 `effective_dt > peak_at_ticks` 后生效** — 避免 malformed `initial < threshold` 时物品一创建就误判 Spoiled）, **`post_peak_spoil_profile`**（迁移后用哪个 Spoil profile）
- 物理意象：灌入灵气 / 药性随时间析出成熟，过峰则封印失效开始外泄；外泄到一定程度变质腐败（陈酒 → 醋 → 毒）
- **路径迁移规则**：当 `effective_dt > peak_at_ticks` 且 `current_qi ≤ post_peak_spoil_threshold` 时，`Freshness.track` 由 `Age` 改为 `Spoil`，`profile` 改为 `post_peak_spoil_profile`，`created_at_tick` 重置为迁移当下 tick（重新开始 Spoil 的衰减计时）— 实装在 §6 lazy eval 的访问点统一处理

---

## §2 Item NBT 扩展

### 2.1 核心字段

```rust
pub struct Freshness {
    /// 物品 mined/harvested/crafted 时的 tick
    pub created_at_tick: u64,
    /// 初始灵气 / 真元 / 药力 / 品质含量
    pub initial_qi: f32,
    /// 走哪条路径：Decay / Spoil / Age
    pub track: DecayTrack,
    /// 公式 + 参数（按 track 选）
    pub profile: DecayProfileId,
    /// 累积已冻结 ticks（历史进 Freeze 容器时长，lazy eval 时从 dt 减去）
    #[serde(default)]
    pub frozen_accumulated: u64,
    /// 当前进入 Freeze 容器的 tick；`Some` = 正在冻结，`None` = 未冻结
    /// 离开容器时 `frozen_accumulated += now - frozen_since_tick`，然后置 None
    #[serde(default)]
    pub frozen_since_tick: Option<u64>,
}

pub enum DecayTrack {
    Decay,
    Spoil,
    Age,
}
```

> **注**：M0 落地时将字段名 `frozen_until_tick` 改为 `frozen_since_tick`（语义更准 —
> 记录**进入 Freeze 容器的起点**，不是未来时间点）。

### 2.2 Spec Registry

**M0 实装选 enum 分支**（每路径独立字段，避免 Option 堆砌）：

```rust
pub enum DecayProfile {
    Decay {
        id: DecayProfileId,
        formula: DecayFormula,
        floor_qi: f32,
    },
    Spoil {
        id: DecayProfileId,
        formula: DecayFormula,
        spoil_threshold: f32,
    },
    Age {
        id: DecayProfileId,
        peak_at_ticks: u64,
        peak_bonus: f32,
        peak_window_ratio: f32,          // ±ratio × peak 的 Peaking 窗口（0.1 = ±10%）
        post_peak_half_life_ticks: u64,
        post_peak_spoil_threshold: f32,
        post_peak_spoil_profile: DecayProfileId,
    },
}

pub enum DecayFormula {
    Exponential { half_life_ticks: u64 },
    Linear { decay_per_tick: f32 },
    Stepwise,
}
```

> **校验**：`DecayProfile::validate()` 在 registry 加载时 reject malformed config（`peak_at_ticks == 0` / `peak_bonus < 0` / `peak_window_ratio` 出 `[0, 1]` / 任意 `NaN`）。

---

## §3 容器 / 保存机制

- [ ] **凡俗箱子** — 无效果（基准）
- [ ] **玉盒 / 灵匣** — 全路径衰减率 ×0.5
- [ ] **阵法护匣** — 冻结（全路径 rate ×0）；卸印恢复
- [ ] **阴干架 / 干燥架** — Stepwise 专用 multiplier 1.0
- [ ] **冰窖**（待 `plan-food-v1`）— Spoil 路径专用，rate ×0.3；Decay / Age 路径无效
- [ ] **陈化窖**（Age 路径专用）— Age 路径 peak_at_ticks ×0.7（加速陈化）；Decay / Spoil 无效
- [ ] **biome 修饰**：血谷 / 负灵域对裸露 item 全路径 ×1.5（异气加速衰败）；青云残峰 ×0.8（灵气护养）— 仅对非容器裸物生效

---

## §4 神识感知（分路径显示）

- [ ] **客户端 tooltip**（凡修基础档）按 track 显示不同档语义：
  - **Decay**：`鲜品 / 微损 / 半衰 / 残留 / 死物`
  - **Spoil**：`新鲜 / 可用 / 即将变质 / 腐败 / 剧毒`
  - **Age**：`青涩 / 成熟 / 巅峰 / 过峰 / 过老`
- [ ] **神识感知**（修为 ≥ 凝脉）：`FreshnessProbeIntent` 返回精确 `current_qi` + `track` + `predicted_event`（距离死物 / 腐败 / 峰值还剩多久）
- [ ] 凡修只能见模糊档（5 档一）；中修可见百分比；高修可见公式参数

---

## §5 消费侧后果分支

### 5.1 Decay 路径

- [ ] **炼丹 / 炼器**：按当下 `current_qi` 折算贡献；低于 floor_qi 时 item ID 变体为"死 X"，可作回收 / 杂料
- [ ] **修炼吸收**：返还 current_qi 对应真元，不是 initial_qi
- [ ] **骨币交易**：按 current_qi / initial_qi 比率打折流通（worldview §518 "封印磨损打折"感）

### 5.2 Spoil 路径

- [ ] **消费时鲜度校验**：
  - 高于 `spoil_threshold` — 正常消费
  - 低于 threshold — 触发 `SpoilConsumeWarning` event；强行消费时按腐败程度施加 contam（Sharp / Violent 档，对标 `plan-alchemy-v1` 丹毒色）
  - 极低（<10%）— 拒绝自动消费，需玩家二次确认（像吃屎）
- [ ] **丹药过期**：除 contam 外还 **减效 + 额外 side_effect_tag**（对接 alchemy plan）
- [ ] **腐败品回收**：败体可走 botany 堆肥 / alchemy "败药粉" 作 Violent 辅料

### 5.3 Age 路径

- [ ] **峰前消费**：按当下 qi 折算，品质略低于 initial（还没熟）
- [ ] **峰值消费**：品质 = initial × (1 + peak_bonus)，**额外触发** `AgeBonusRoll`（如 alchemy 成丹率 +10%）
- [ ] **过峰消费**：按 post-peak 曲线折算；进入**危险区**后自动降级为 Spoil 路径（陈酒放烂了就是醋，再放就是毒）
- [ ] **峰值窗口提示**：客户端 HUD 到峰时标"巅峰"给玩家决策窗口

---

## §6 tick 架构（lazy eval）

### 6.1 核心原则 + access-time 枚举

- [ ] **核心原则**：不开后台 tick 扫描全库存。衰减是**函数**不是**状态**，需要时现算
- [ ] **完整 access-time 枚举**（M0 必须穷尽以下事件触发 lazy compute + 状态迁移）：
  1. **Inventory snapshot emit**（默认每 N tick / item 变化时）— 批量算 `current_qi` + `track_state` 塞 client payload
  2. **Consume intent**（alchemy / forge / 修炼吸收 / 食用 / 骨币交易）— 取当下值参与 §5 分支处理
  3. **`FreshnessProbeIntent`**（神识感知请求）— 算 + 返回精度按修为分档
  4. **Container in / out 事件** — 记 `frozen_accumulated`（封印冻结路径）
  5. **Death drop / pickup** — item entity 落地时算一次（裸露 biome 修饰开始计入），拾取回 inventory 时算一次
  6. **Item transfer**（玩家→玩家 / 玩家→容器）— 算一次 + 进出容器记账
  7. **Server tick boundary 200**（与 worldstate publish 同节拍） — 全局 sweep 触发**只对**`track_state` 边界跨越的 item 做 ID 变体切换（见 §6.3），其它 item 不触

### 6.2 批量查询优化

- [ ] **inventory snapshot emit** 一次性算好所有 item 的 `current_qi` + `track_state` + `predicted_event`
- [ ] **持久化**（归 `plan-persistence-v1`）：存 `created_at_tick` + `frozen_accumulated` + `track` + `profile_id`，**不**存 `current_qi`（下次开档即时算）

### 6.3 死物 / 腐败 / 过峰 item ID 变体策略

> v1 不做"所有 Decay 物品都注册死变体"（N×2 物品膨胀）。仅对**经济意义大**的物品做变体切换：

| 物品 | track | 触发条件 | 变体后 item ID |
|---|---|---|---|
| `ling_shi_*` | Decay | `current_qi <= floor_qi` | `dead_ling_shi_*`（仍可走 botany 堆肥 / alchemy 杂料） |
| `骨币` | Decay | `current_qi <= floor_qi` | `rotten_bone_coin`（次级市场回收） |
| `兽肉` / `兽血` | Spoil | `current_qi < spoil_threshold` | NBT `is_spoiled = true`（**不**换 item ID，由 NBT 标识；消费时按 §5.2 校验） |
| `常规丹药` | Spoil | 同上 | NBT `is_spoiled = true`（同上） |
| `老坛灵丹` | Age 迁 Spoil | 见 §1.4 路径迁移 | NBT 同上，无 item ID 切换 |
| `陈酒` | Age 迁 Spoil | 同上 | `chen_cu`（陈酒 → 醋）这种**有文化语义**的特例做 item ID 切换；普通 Age 物品只换 NBT |

- [ ] **决策原则**：item ID 切换仅当**有独立经济市场 / 文化命名**才做（死灵石、腐骨币、陈醋）；否则用 NBT 内部状态表示
- [ ] **lazy 触发**：变体切换在上述 access-time 第 5/6/7 条触发；不在 §6.1 第 1-4 条切（避免渲染时直接换物品 ID 带来的 Bevy ECS 混乱）

---

## §7 跨 plan 钩子表

> **半衰期数值是建议骨架值，不是真实物理**。游戏化简化：兽肉真实常温 4-8h 就坏，本表 1d 是为玩家体验调慢；陈酒 365d 真实合理但玩家时间预算紧 — 实施时按 **"游戏内时间 vs 现实时间"换算**重新调（建议 1 现实日 = 游戏内 N 日，参数 N 由 cultivation/agent 时间系统决定）。所有数字 M5 联调时按实际玩家行为再校。

| 消费 plan | 物品 | 路径 | 公式 + 参数建议 |
|---|---|---|---|
| `plan-mineral-v1` | `ling_shi_fan/zhong/shang/yi` | Decay | Exp, half_life 3/5/7/14 days（按品阶递增） |
| `plan-mineral-v1` | `dan_sha` / `zhu_sha` | Decay | Exp, half_life ≈ 60 days |
| `plan-fauna-v1` | `骨币` | Decay | Linear, ~1y 完全衰减（需续印） |
| `plan-fauna-v1` | `兽血` | **Spoil** | Exp, half_life ≈ 12 real-hours |
| `plan-fauna-v1` | `兽肉` | **Spoil** | Exp, half_life ≈ 1 real-day |
| `plan-fauna-v1` | `内丹` / `血精` | Age→Spoil 迁 | PeakAndFall, peak ≈ 7d, post_peak_spoil_threshold ≈ 0.3 → 自动迁 Spoil |
| `plan-botany-v1` | 鲜草药 | **Spoil** | Exp, half_life ≈ 2 real-days |
| `plan-botany-v1` | 阴干草药 | Decay | Stepwise（容器挂钩） |
| `plan-botany-v1` | 陈年灵茶 | **Age** | PeakAndFall, peak ≈ 90d |
| `plan-alchemy-v1` | 常规丹药 | **Spoil** | Exp, half_life ≈ 30 real-days |
| `plan-alchemy-v1` | 老坛灵丹 | **Age** | PeakAndFall, peak ≈ 180d |
| `plan-alchemy-v1` | 丹方残卷 | Decay | Linear, ~500 days（v2+ 是否做调参） |
| `plan-food-v1` | 凡俗食物 | **Spoil** | Exp, half_life ≈ 3 days |
| `plan-food-v1` | 陈酒 / 陈醋 | **Age** | PeakAndFall, peak ≈ 365d |
| `plan-forge-v1` | 图谱残卷 | Decay | Linear, ~1000+ days（仅装饰怀古，可考虑去除） |
| `plan-cultivation-v1` | 灵石作修炼燃料 | — | 不挂自己的 profile，按当下 `current_qi` 吸收（cultivation plan 需自家 blessing 加入"烧灵石"机制） |

---

## §8 数据契约

- [ ] `DecayTrack` enum（`Decay` / `Spoil` / `Age`）
- [ ] `DecayFormulaKind` enum（`Exponential` / `Linear` / `Stepwise` / `PeakAndFall`）
- [ ] `DecayProfileId` + `DecayProfileRegistry` resource
- [ ] Item NBT `freshness: Freshness { created_at_tick, initial_qi, track, profile, frozen_accumulated }`
- [ ] `FreshnessProbeIntent` / `FreshnessProbeResponse` event
- [ ] `SpoilConsumeWarning` event（Spoil 路径消费时的危险警告）
- [ ] `AgeBonusRoll` event（Age 路径峰值消费的加成触发）
- [ ] 纯函数：
  - `compute_current_qi(freshness, now_tick) -> f32`
  - `compute_track_state(freshness, now_tick) -> TrackState`（`Fresh` / `Declining` / `Dead` / `Spoiled` / `Peaking` / `PastPeak`）
- [ ] `ContainerFreshnessBehavior` enum（`Normal` / `Halve` / `Freeze` / `Stepwise(multipliers)` / `SpoilOnly(rate)` / `AgeAccelerate(factor)`）

---

## §9 实施节点

- [ ] **M0 — 纯函数层**：`compute_current_qi` + `compute_track_state` 四档公式 + 100% 单元测试（含 PeakAndFall 峰值前后边界 / 死物 / 腐败阈值）
- [ ] **M1 — Freshness item NBT**：扩展 `plan-inventory-v1` item 字段，持久化兼容（`#[serde(default)]`，旧档 item 视为 `track=Decay, fresh`）
- [ ] **M2 — 容器行为**：`ContainerFreshnessBehavior` 挂 container；进 / 出事件记 frozen accumulation
- [ ] **M3 — tooltip + snapshot**：客户端 HUD 分路径档位色 + inventory snapshot 塞当下 qi + track_state
- [ ] **M4 — 神识感知**：`FreshnessProbeIntent` + 修为阶差 probe 精度
- [ ] **M5 — 消费侧接入**：
  - alchemy / forge / 修炼吸收按 §5.1 读当下值（Decay）
  - 丹药 / 兽血 消费按 §5.2 校验 spoil_threshold（Spoil）
  - 陈酒陈丹按 §5.3 触发 peak bonus roll（Age）
- [ ] **M6 — 死物 / 腐败 / 过峰 item 变体**：floor_qi / spoil_threshold / peak 窗口触发 item ID 切换
- [ ] **M7 — 跨 plan DecayProfile 定稿**：mineral / fauna / botany / alchemy / food 各自在自家 plan 正式 blessing §7 表的参数

---

## §10 开放问题

- [ ] **半衰期数值正式调参 + 时间换算**：§7 表只是骨架建议，需 M5 联调按实际玩家行为测；同时定义"1 现实日 = N 游戏日"的换算（与 cultivation/agent 时间系统协调）
- [ ] **陈化物的最佳消费窗口 UX**：Age 路径的"巅峰"提示是硬通知（dialog 中断）还是软提示（HUD 角标）？玩家是否可关闭通知？
- [x] ~~**DecayProfile spec：Option struct vs enum 分支**~~ — **M0 选 enum 分支**（已在 §2.2 落地）
- [ ] **骨币续印成本**：worldview §六 L518 续印路径（alchemy/forge/阵法师）— 影响骨币 Linear 衰减速率是否可在续印时重置 / `created_at_tick` 是否归零
- [ ] **冻结区间记账并发**：玩家同 tick 多次进出容器 — 需要事件合流 + idempotent key（`(item_uuid, tick)` 去重）
- [ ] **神识感知的阶差粒度**：凡修 / 中修 / 高修 3 档粒度差是否太大？是否按 worldview §一 L68-72 的 6 境界（醒灵 / 引气 / 凝脉 / 固元 / 通灵 / 化虚）逐档细化感知精度
- [ ] **死物 / 腐败 / 过峰 的次级经济**：死灵石 / 腐骨币 / 败药粉 / 陈醋 是否自成市场（ragpickers / 垃圾收购商 / 腐料炼毒）— 归 `plan-economy-v1`
- [ ] **biome 修饰与裸露 item 的实现**：worldgen biome 边界跨越是按 tick 采样还是纯 lazy-on-read — 倾向后者（§6.1 第 5 条 death drop / pickup 计入），简单且一致
- [ ] **cultivation plan blessing 灵石燃料**：cultivation 现无"烧灵石作修炼燃料"机制，需该 plan 独立 PR 加入消费路径，本 plan 才能落地灵石的实际用途
- [ ] **forge 图谱残卷是否真需 Decay**：1000+ days 几乎无影响，仅装饰怀古意象 — 是否取消该挂钩简化 v1 范围
- [ ] **`SpoilConsumeWarning` 事件的客户端 UX 通道**：拒绝自动消费 + 二次确认走 dialog 还是 chat 提示 + 命令？（与 `plan-HUD-v1` 协调）

---

> 本 plan 立项目标：跨 mineral / fauna / botany / alchemy / food / forge 共用保质期基础设施。三条路径（衰减 / 腐败 / 陈化）覆盖末法世界所有"时间敏感"物品类别。M0 纯函数层可与各消费 plan 并行推进；M5 消费侧联调是关键拐点。
