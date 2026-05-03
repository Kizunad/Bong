# Bong · plan-lifespan-v1 · Active

> **状态**：⏳ active（2026-05-04 升级，user 拍板）。前置 plan-death-lifecycle-v1 / plan-cultivation-v1 全 ✅ finished；P3 续命丹依赖 plan-alchemy-v2（同期升 active）StatusEffect 映射；P4 悟道延寿等 plan-cultivation-v1 顿悟池深度定义；P5 坍缩渊换寿等 plan-terrain-rift-mouth-v1（同期升 active）+ plan-tsy-zone-v1。

寿元系统精细实装 + 风烛状态 + 老死分类 + 续命路径四分支。对应 `plan-death-lifecycle-v1`（finished）§4a/§4b/§4c/§4e/§4f 中标记未实装的 Phase 3/6/7 功能块。

**世界观锚点**：`worldview.md §十二 死亡重生与一生记录`（寿元上限 / 老死 / 续命代价 / 末法无转世记忆继承）

**交叉引用**：
- `plan-death-lifecycle-v1`（finished）— 基础 `Lifecycle` component / `apply_revive_penalty` 已落；本 plan 补寿元 tick 推进层
- `plan-multi-life-v1`（skeleton）— 覆盖多周目/运数 per-life；本 plan 只管单世寿命计时与续命
- `plan-cultivation-v1`（finished）— 顿悟池深度定义（`plan-alchemy-v2 §4f` 悟道延寿需要）
- `plan-npc-ai-v1`（finished）— NPC `NpcLifespan` 参考；本 plan 的 `LifespanComponent` 以玩家为主，NPC 扩展见 §8

---

## 接入面 Checklist

- **进料**：`Lifecycle.fortune_remaining` + `Realm`（境界 → 寿元上限） + DeathEvent（触发扣寿） + 离线登出/登入事件（offline tick 补算）
- **出料**：
  - 寿元耗尽 → `DeathEvent { cause: NaturalDeath }` 进 death-lifecycle 终结流水线
  - 风烛 → `StatusEffect::Frailty`（真元回复减半）+ agent narration 请求
  - 续命事件 → `LifespanEvent` Redis channel + `life_record::lifespan_events[]`
- **共享类型**：复用 `Realm` enum ✅ · 新增 `LifespanComponent` / `TickRateModifier` / `LifespanEvent`
- **跨仓库契约**：
  - server：`server/src/cultivation/lifespan.rs`（新文件）
  - agent：`bong:aging` channel 订阅 → 风烛/老死 narration
  - client：inspect 面板 `已活 X / 上限 Y` 展示（IPC schema 扩展）
- **worldview 锚点**：§十二"寿元宽裕——正常玩家不会老死，卡两类人：极端苟者 / 续命丹吊着的 NPC"

---

## §0 设计轴心

- [ ] **寿元不是常态约束**——只卡极端苟者和拒绝前进的 NPC；正常推进节奏在化虚前不会耗尽
- [ ] **离线寿元继续消耗，倍率 ×0.1**——防"退游续命"
- [ ] **续命必须明码标价，代价超线性递增**——阻止无限叠加；单角色总续命 ≤ 当前境界上限 ×2
- [ ] **老死是自然归宿**——善终分类，不掉物品，遗物就地留容器
- [ ] **夺舍限凡人/醒灵**——降低滥用为 PK 武器的风险

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | `LifespanComponent` + `LifespanCapTable` + 基础 tick 推进 | 单元：每境界上限值、tick rate 切换、死亡扣寿 5%、≤0 转老死 |
| **P1** ⬜ | 风烛状态 (<10% 寿元) + inspect 面板展示 | 风烛 buff 激活/解除单测；inspect IPC 字段校验 |
| **P2** ⬜ | 老死结算（善终分类 + 遗物容器 + 生平卷 `death_kind=natural`） | 老死终结流水集成测试（不走运数/劫数） |
| **P3** ⬜ | 续命丹（真元上限永久扣除，P0/P1 依赖 plan-alchemy-v2 StatusEffect） | 续命丹代价曲线单测；累计上限硬校验 |
| **P4** ⬜ | 悟道延寿（顿悟名额兑换，依赖 plan-cultivation 顿悟池深度定义） | 一生一次校验；悟境天花板 -1 写入生平卷 |
| **P5** ⬜ | 坍缩渊换寿（高风险，依赖 worldgen 负灵域 POI + `plan-tsy-zone-v1`） | "寿核"掉落集成；失败直接老死路径 |
| **P6** ⬜ | 夺舍（业力路径，限凡人/醒灵，双卷交叉引用） | 夺舍者/被夺舍者生平卷交叉引用单测 |

---

## §2 核心数据契约

```rust
// server/src/cultivation/lifespan.rs（新文件）

pub struct LifespanComponent {
    pub born_at_tick: u64,
    pub years_lived: f64,
    pub cap_by_realm: u32,         // 当前境界对应上限（年）
    pub offline_pause_tick: Option<u64>,
    pub accumulated_extension_years: u32,  // 续命累计，用于代价曲线
}

pub struct LifespanCapTable;
impl LifespanCapTable {
    // 醒灵→化虚：120/200/350/600/1000/2000
    pub fn cap(realm: Realm) -> u32 { ... }
}

pub struct TickRateModifier {
    pub source: TickRateSource,   // Online / Offline / ZoneDeath / ZoneVoid
    pub multiplier: f32,          // 1.0 / 0.1 / 2.0 / 2.0
}

pub struct LifespanEvent {
    pub char_id: String,
    pub at_tick: u64,
    pub kind: LifespanEventKind,  // Aging | DeathPenalty | Extension | NaturalDeath
    pub delta_years: f64,
    pub source: String,
}

pub enum LifespanEventKind { Aging, DeathPenalty, Extension, NaturalDeath }
```

**Tick 速率规则**：
| 场景 | 倍率 | 1 real hour = X in-game year |
|------|------|------|
| 在线正常 | ×1.0 | 1 年 |
| 离线 | ×0.1 | 0.1 年 |
| 死域/负灵域 | ×2.0 | 2 年 |

**死亡扣寿**：= 当前境界寿元上限 × 5%；扣后剩余 ≤ 0 → 转老死终结（跳过运数/劫数）

**续命代价曲线**：
```
cost(Δyears) = base(method) × Δyears × (1 + accumulated / cap_by_realm)^1.5
```

---

## §3 IPC / Redis Channel

| Channel | 方向 | 内容 |
|---------|------|------|
| `bong:aging` | server→agent | 风烛进入/老死预告/tick rate 切换 narration 请求 |
| `bong:lifespan_event` | server→agent | 所有续命/夺舍/悟道延寿流水（公开可查） |

Client IPC 扩展（inspect 面板）：
```typescript
// agent/packages/schema/src/server-data.ts — LifespanDetailV1
{
  type: "lifespan_detail",
  years_lived: number,
  cap_by_realm: number,
  tick_rate: number,         // 当前有效倍率（1.0 / 0.1 / 2.0）
  frail: boolean,            // 风烛状态
  accumulated_extension: number,  // 已累计续命年数
}
```

---

## §4 风烛（Frailty）buff

- 触发：`years_lived / cap_by_realm >= 0.9`（剩余 < 10%）
- 解除：续命使剩余 > 10%（但"续命后风烛判定不刷新"——只要剩余 < 10% 仍风烛；仅在真正超过 10% 后解除）
- 实装位置：`server/src/cultivation/lifespan.rs::frailty_check_system`

**境界差异化数值**（user Q-L2 决策，2026-05-04）——高境风烛更狠，符合 worldview "修士越强越苟" 叙事：

| 境界 | 真元回复倍率 | 遗念 narration 周期 | 强制老化 narration 周期 |
|---|---:|---:|---:|
| 醒灵/引气 | ×0.7 | 2 年 / 次 | 20 年 / 条 |
| 凝脉 | ×0.6 | 1.5 年 / 次 | 15 年 / 条 |
| 固元 | ×0.5 | 1 年 / 次 | 10 年 / 条 |
| 通灵 | ×0.4 | 0.7 年 / 次 | 7 年 / 条 |
| 化虚 | ×0.3 | 0.5 年 / 次 | 5 年 / 条 |

设计意图：低境界风烛只是"提醒"，高境界风烛是"窒息"——化虚修士若真到风烛，真元 -70% 战力大跌（对应 worldview "化虚后世界变量"——化虚修士不可能轻易战死，但寿元到了真活不下去）。

---

## §5 老死结算

1. 不触发运数/劫数 roll，直接进终结流水线
2. 生平卷写入 `death_kind = "natural"`（与 `combat_kill` / `tribulation` 区分；亡者博物馆"善终"筛选依赖此字段）
3. 不掉落物品——身旁就地生成 `NaturalDeathCorpse` entity（**复用 TSY `CorpseEmbalmed` / `BodyInstance` 模式**，user Q-L1 决策 2026-05-04）：
   - 共享底层 `BodyInstance` 结构（pose / inventory snapshot / origin / 流派）
   - 与 `CorpseEmbalmed` 区别：deathkind=natural（不转道伥；冬季 7 天 / 夏季 3 天后自然降解为骨堆 → 最终消失）
   - 容器化：其他玩家右键打开看到死者 inventory 全副 + 取走（worldview §十二"老死遗物就地留容器"）
   - 视觉：复用 PlayerAnimator 死亡 pose（参考 third_party/serious-player-animations）
4. 触发长遗念 agent 请求（category = `natural`，agent 读 LifeRecord 合成回顾性 narration）
5. 最终走 `plan-death-lifecycle-v1::terminate_character` 流水（生平卷冻结 + 亡者博物馆）

---

## §6 开放问题

- [x] **Q-L1 ✅**（user 2026-05-04 B）：遗骸做成尸体 entity（不是 chest）。**复用 TSY `CorpseEmbalmed` / `BodyInstance` 模式**（`server/src/world/tsy_lifecycle.rs:531`）：新 entity `NaturalDeathCorpse` 共享底层 `BodyInstance` 结构（pose / inventory / origin），但 deathkind=natural（不转道伥；冬季 7 天后自然降解 / 夏季 3 天）。容器内物品 = 死者 inventory 全副（worldview §十二"老死不掉物品"= 不强制丢，仍由其他玩家可开取）。详 §5。
- [x] **Q-L2 ✅**（user 2026-05-04 B）：风烛 buff 按境界差异化。详 §4（数值表）。
- [x] **Q-L3 ✅**（user 2026-05-04）：~~plan-cultivation 尚未定义~~——顿悟系统已完整实装（`server/src/cultivation/insight.rs`：InsightQuota + realm_quota + InsightCategory）。P4 直接消费 `InsightQuota::has_quota` + `apply_accumulation`：悟道延寿 = 走一次 `InsightCategory::Composure` 类 effect（一次性 +X 年 + 悟境天花板 -1）。
- [x] **Q-L4 ✅**（user 2026-05-04 A）：寿核 P5 stub，等 plan-tsy-zone-v1 / plan-terrain-rift-mouth-v1 P3 联动后再拍数值。
- [x] **Q-L5 ✅**（user 2026-05-04 B）：`LifespanComponent` 玩家 NPC 共用（减少重复开发）。NPC 直接挂同 component；性能优化用"NPC 寿命 tick rate ×0.01"低频 + only-online-chunk 判定。详 §8。

---

## §7 NPC 共用扩展（user Q-L5 决策 2026-05-04）

`LifespanComponent` 玩家 + NPC 共用同一 component，但 tick rate 差异化：

| Entity | 在线 tick 倍率 | 离线 tick 倍率 | 备注 |
|---|---:|---:|---|
| 玩家 | 1.0 | 0.1 | §2 默认表 |
| NPC（chunk 加载中）| 0.1 | 0.001 | 1 real h ≈ 6 NPC year（活在线 chunk）|
| NPC（chunk 卸载）| 0 | 0 | 完全暂停（防 chunk 流失 NPC 自然老化大批死） |

**性能保护**：
- NPC 寿命 tick 系统每 100 game tick 跑一次（5 秒），不每 tick 跑
- only-online-chunk 判定：unloaded chunk 的 NPC 不入循环
- 老死的 NPC：直接 despawn + 在原位 spawn `NaturalDeathCorpse` entity（NPC 也走与玩家相同的遗骸路径）

**NPC 老死对玩法的影响**：
- NPC 散修可能"老死前传授残卷"——触发 narration "X 临死前将口诀告知与你"
- 长寿 NPC（化虚遗老）寿元到了→ 老死事件成为全服话题（worldview §十二"亡者博物馆 + 善终筛选"）

---

## §8 进度日志

- 2026-05-01：从 plan-death-lifecycle-v1 reminder 整理立项。现有代码：`Lifecycle` component + `apply_revive_penalty` ✅（`server/src/combat/lifecycle.rs`）；`LifespanComponent` / `LifespanCapTable` / `TickRateModifier` / `LifespanEvent` 均未实装。
- **2026-05-04**：skeleton → active 升级（user 拍板）。前置 plan-death-lifecycle-v1 / plan-cultivation-v1 ✅ finished。P0/P1/P2 可立刻起；P3-P5 待并行 plan（alchemy-v2 / cultivation 顿悟池 / rift-mouth）补齐再做。下一步起 P0 worktree（LifespanComponent + LifespanCapTable + tick 推进）。
- **2026-05-04**：§6 全部 5 决策闭环（Q-L1/L2/L3/L4/L5 详 §6）。范围扩展：§4 加境界差异化风烛表；§5 老死遗骸复用 TSY `CorpseEmbalmed` / `BodyInstance` 模式；§7 新增 NPC 共用 LifespanComponent 章节。P4 悟道延寿直接接 `server/src/cultivation/insight.rs`（顿悟系统已实装），不依赖新接口。
