# Bong · plan-volatility-v1 · 骨架

**通用挥发 / 衰减系统**。末法时代一切都在衰败 — 本 plan 定义跨物品类别的统一衰减机制，供 mineral / fauna / botany / alchemy 等 plan import，避免各自重造。

**世界观锚点**：`worldview.md §63`（末法时代修士不配用上古称呼 → 一切物品都带衰败意象）· `worldview.md §518`（"金银如废土，唯一硬通货是被强制锁住的真元" → 骨币是**封印**真元，封印会磨损；灵石是**未封**灵气，自由挥发快）· `worldview.md §六 557`（矿脉挖完就没 — 供给侧衰败 + 本 plan 的消费侧衰败 = 双重稀缺）。

**交叉引用**：
- `plan-mineral-v1.md §0.1 §3`（灵石衰变五连）— 由本 plan 的"快速挥发"档承接
- `plan-fauna-v1.md`（待立 — 骨币、妖兽血精、内丹）— 由本 plan 的"慢衰减"与"封印冻结"承接
- `plan-botany-v1.md`（新鲜药草 vs 阴干药草）— 由本 plan 的"存储状态分支"承接
- `plan-alchemy-v1.md`（丹药过期）— 由本 plan 的"中速衰减"承接
- `plan-persistence-v1.md`（lazy freshness 持久化存档）
- `plan-economy-v1.md`（待立 — 骨币 vs 灵石 vs 丹药的跨时长价值差）

---

## §0 设计轴心

- [ ] **一切皆挥发，速率不同** — 骨币年级（~1y）、丹药月级（~30d）、干草药半年、鲜草药 / 血精日级、灵石时日级、兽血时辰级
- [ ] **lazy evaluation，不逐 tick 扣** — server 不 per-tick 减库存里每个 item 的 `current_qi`；只在**访问时**（inspect / 炼丹消费 / UI 查询）按 `(now_tick - mined_at_tick)` 计算当下值
- [ ] **封印 = 冻结衰减**（worldview §518 骨币锚点）— 特殊容器 / 阵法护罩把 `decay_rate` 临时乘 0，卸印即恢复
- [ ] **神识感知鉴真**（mineral §3 灵石鉴真 / fauna 骨币验伪）— 剩余真元 / 灵气值作 probe 返回的一部分
- [ ] **消费侧按当下折算**（不是按初始）— 炼丹 / 炼器用 item 时读**当下** freshness 算贡献，过期材料减效甚至触发 flawed_path
- [ ] **挥发不等于消失** — 挥发至 0 只是"死物"（`死灵石` / `腐骨币` / `药渣`），可回收 / 炼化，不直接 despawn

---

## §1 衰减公式

> 三档全局公式 + 物品 spec 选一档。禁新建第四档，v2+ 再扩。

### 1.1 指数（exponential）

```
current_qi(t) = initial_qi * 0.5 ^ (dt / half_life_ticks)
```

- 用于**生物制品 / 灵气类**：灵石、兽血、血精、丹药、鲜草药
- 参数：`half_life_ticks`
- 物理意象：灵气自由逸散，半衰期是对数的

### 1.2 线性（linear）

```
current_qi(t) = max(0, initial_qi - decay_per_tick * dt)
```

- 用于**阵法封印 / 工艺品衰减**：骨币（封印慢磨损）、残卷（纸张老化）
- 参数：`decay_per_tick`
- 物理意象：机械磨损，和时间一次项成比例

### 1.3 阶梯（stepwise）

```
current_qi(t) = initial_qi * storage_multiplier(current_container)
```

- 用于**跟容器强绑**：阴干药草（干燥架 1.0 / 普通箱子 0.7 / 露天 0.3）
- 参数：`storage_multipliers: HashMap<ContainerKind, f32>`
- 物理意象：不连续衰减，跟随 storage state 跳

---

## §2 Component / Item 字段设计

### 2.1 Item NBT 扩展（inventory item 层）

```rust
pub struct Freshness {
    /// 物品 mined/harvested/crafted 时的 tick
    pub created_at_tick: u64,
    /// 初始灵气 / 真元 / 药力含量
    pub initial_qi: f32,
    /// 衰减档（按 spec 表索引）
    pub decay_profile: DecayProfileId,
    /// 上一次进入"封印"容器 / 出来的 tick（用于冻结区间累积）
    pub frozen_until_tick: Option<u64>,
    /// 累积已冻结 ticks（lazy eval 时减去）
    pub frozen_accumulated: u64,
}
```

### 2.2 Spec Registry

```rust
pub struct DecayProfile {
    pub id: DecayProfileId,
    pub kind: DecayFormulaKind, // Exponential / Linear / Stepwise
    pub params: DecayParams,     // half_life / decay_per_tick / storage_multipliers
    pub floor_qi: f32,           // 挥发至 0 后的"死物残值"（便于回收）
}
```

---

## §3 保鲜机制

- [ ] **容器类型**（`plan-inventory-v1` 扩展）：
  - 凡俗箱子 — 无效果
  - 玉盒 / 灵匣 — 衰减率 ×0.5（半衰期加倍）
  - 阵法护匣 — 衰减率 ×0（完全冻结，离开即失效）
  - 阴干架 / 冷室 — Stepwise storage_multiplier 1.0
- [ ] **biome 修饰**：血谷 / 负灵域高挥发（全局 ×1.5）；青云残峰低挥发（×0.8）— 仅裸露 item entity 生效，容器里不受
- [ ] **阵法封印**（worldview §518 骨币锚点）：骨币制作阶段由阵法师写入 `frozen_until_tick`，到期需重新续印（经济持续性设计）
- [ ] **冻结区间记账**：进封印容器记 `frozen_until_tick = now`，出来时 `frozen_accumulated += now - frozen_until_tick`；lazy eval 用 `dt - frozen_accumulated`

---

## §4 查询 / 感知机制

- [ ] **客户端 tooltip**：hover 时显示"剩余 N%"（按当下 lazy eval），颜色分档（>80% 绿 / 50-80% 黄 / <50% 橙 / ≤0% 灰"死物"）
- [ ] **神识感知**（修为 ≥ 凝脉）：`FreshnessProbeIntent` 返回精确 `current_qi` + `decay_profile` + 预估"可用还剩多久"
- [ ] **凡修只能见模糊档**（绿/黄/橙/灰），精确数值需神识 — 鉴真门槛 + 掺假经济学空间

---

## §5 消费侧折算

- [ ] **炼丹**（`plan-alchemy-v1`）：读材料当下 `current_qi`，低于阈值（如 initial × 0.3）判 flawed_path + side effect
- [ ] **炼器**（`plan-forge-v1`）：同上，低鲜度矿/木/骨降低 quality 系数
- [ ] **修炼吸收**（灵石烧入）：按当下 `current_qi` 返还真元，不是 `initial_qi`
- [ ] **骨币交易**：验真时算 `current_qi / initial_qi` 比率，<0.7 视作贬值骨币（需打折流通）
- [ ] **死物回收**：`current_qi ≤ floor_qi` 时物品 ID 变体为"死 X"（灵石→死灵石 / 骨币→腐骨币 / 药草→药渣），可作**杂料**走不同消耗路径

---

## §6 tick 架构（lazy eval）

- [ ] **核心原则**：不开后台 tick 扫描全库存。衰减是**函数**不是**状态**，需要时现算
- [ ] **热点**：access-time 计算 + 容器进出时记账
- [ ] **批量查询优化**：inventory snapshot emit 时一次性把所有 item 的 `current_qi` 算好塞进 client payload，避免客户端逐次请求
- [ ] **持久化**（归 `plan-persistence-v1`）：存 `created_at_tick` + `frozen_accumulated`，不存 `current_qi`（下次开档即时算）

---

## §7 跨 plan 钩子表

| 消费 plan | 物品 | `decay_profile` | 参数建议 |
|---|---|---|---|
| `plan-mineral-v1` | `ling_shi` | Exponential | half_life ≈ 3 real-days |
| `plan-mineral-v1` | `dan_sha` / `zhu_sha` | Exponential | half_life ≈ 60 days（矿物类较稳） |
| `plan-fauna-v1` | `骨币` | Linear | decay_per_tick 对应 1y 完全衰减（需续印） |
| `plan-fauna-v1` | `兽血` | Exponential | half_life ≈ 12 real-hours |
| `plan-fauna-v1` | `内丹` / `血精` | Exponential | half_life ≈ 7 days |
| `plan-botany-v1` | 鲜草药 | Exponential | half_life ≈ 2 real-days |
| `plan-botany-v1` | 阴干草药 | Stepwise | 干燥架 1.0 / 凡俗箱 0.7 / 露天 0.3 |
| `plan-alchemy-v1` | 丹药 | Exponential | half_life ≈ 30 real-days |
| `plan-forge-v1` | 图谱残卷 | Linear | 1000+ days（纸张老化，基本不用担心） |

---

## §8 数据契约

- [ ] `DecayProfileId` enum + `DecayProfileRegistry` resource
- [ ] Item NBT `freshness: Freshness { created_at_tick, initial_qi, decay_profile, frozen_accumulated }`
- [ ] `FreshnessProbeIntent` / `FreshnessProbeResponse` event
- [ ] `compute_current_qi(freshness, now_tick) -> f32` 纯函数（可单测、可跨 plan 调用）
- [ ] `ContainerFreshnessBehavior` enum（`Normal` / `Halve` / `Freeze` / `Stepwise(multipliers)`）挂 container 类型

---

## §9 实施节点

- [ ] **M0 — 纯函数层**：`compute_current_qi` + `DecayFormulaKind` 三档 + 100% 单元测试
- [ ] **M1 — Freshness item NBT**：扩展 `plan-inventory-v1` item 字段，持久化兼容（`#[serde(default)]`）
- [ ] **M2 — 容器行为**：`ContainerFreshnessBehavior` 挂 container；进/出事件记 frozen accumulation
- [ ] **M3 — tooltip + snapshot**：客户端 HUD 分档颜色 + inventory snapshot 塞 current_qi
- [ ] **M4 — 神识感知**：`FreshnessProbeIntent` 接入
- [ ] **M5 — 消费侧接入**：alchemy / forge / 修炼吸收 / 骨币验真按本 plan 读当下值
- [ ] **M6 — 死物变体 + 回收**：floor_qi 触发 item ID 切换（灵石→死灵石…）
- [ ] **M7 — 跨 plan DecayProfile 定稿**：mineral / fauna / botany / alchemy 各自在自家 plan 正式 blessing §7 表的参数

---

## §10 开放问题

- [ ] **半衰期数值正式调参**：§7 表的参数只是骨架建议，需在 M5 联调时按实际玩家行为测（丹药 30d 是否太短？骨币 1y 是否太长？）
- [ ] **冻结区间的记账并发**：玩家同 tick 多次进出容器是否会重复记 frozen_accumulated（需要事件合流 + idempotent key）
- [ ] **biome 修饰是否重算**：裸露 item entity 在 biome 边界移动时衰减率切换 — 是"进入 blood_valley 的那 tick 即时调整"还是"每 secs 采样一次" — 折中到 §6 lazy eval 的访问时计算即可
- [ ] **神识感知的阶差**：凡修看绿/黄/橙/灰 4 档是否太粗？中修看多一档百分比区间？
- [ ] **死物经济学**：死灵石 / 腐骨币 / 药渣 是否形成第二经济层（末法废品市场 / 收购商）— 归 `plan-economy-v1`？
- [ ] **丹药过期是否产生负效果**：单纯减效 vs 减效 + 额外 contam（类似过期食物）— 与 alchemy side_effect 合流设计
- [ ] **续印骨币的成本**：worldview §518 阵法锁真元 — 续印是 alchemy？forge？阵法师？还是骨币携带者自己冲真元？定位影响经济游戏循环

---

> 本 plan 立项目标：跨 mineral / fauna / botany / alchemy 共用衰减基础设施。先以骨架定锁 **lazy eval + 三档公式** 的架构，M0 纯函数层可与各消费 plan 并行推进；M5 消费侧联调时是关键拐点。
