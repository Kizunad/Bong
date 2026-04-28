# Bong · plan-death-lifecycle-v2 · 骨架

**死亡生命后续**：补齐 v1 遗留的三个未落项——生平卷善终/横死结构化分类、风烛状态实装、寿元系统与化虚基线 / 亡者博物馆的交叉验证；同时落地「遗念 agent deathInsight tool」。

**前置**：`plan-death-lifecycle-v1`（finished；`PillExtensionContract` / `CollapseCoreExtensionContract` / `EnlightenmentExtensionContract` 已实装于 `lifespan.rs` 94–154；`LIFESPAN_TICKS_PER_YEAR = 60*60*20` 已实装）

**世界观锚点**：
- `worldview.md §三`（化虚修士寿命 51.5h 基线；末法修士晚年"风烛残年"描述）
- `worldview.md §十一`（亡者博物馆——死者 NPC 在死亡地点残存）
- `worldview.md §八`（天道冷漠语调；遗念叙事的语气参照）
- `worldview.md §九`（情报换命；遗念可成为"情报残留"的一种形式）

**library 锚点**：待写 `cultivation-XXXX 风烛余年录`（记录末法修士进入风烛状态的身心变化及叙事案例，anchoring worldview §三 寿元末期描述）

**交叉引用**：
- `plan-death-lifecycle-v1`（数据基础：`BiographyEntry`、`LifespanComponent`、续命合约）
- `plan-tribulation-v1`（渡虚劫截杀死亡 → ViolentDeath；渡劫失败退境 ≠ 死亡，不触发此分类）
- `plan-HUD-v1`（风烛状态 HUD 顶栏提示）
- `plan-narrative-v1`（天道叙事接口；遗念 agent tool 依赖）
- `plan-cultivation-v1`（境界 → 风烛阈值；MovementSpeedModifier / QiCapModifier 接口对齐）

**阶段总览**：
- P0 ⬜ 善终/横死 `DeathKind` 枚举 + 生平卷字段落地
- P1 ⬜ 风烛状态 `WindCandleComponent` + buff/debuff 数值实装
- P2 ⬜ 寿元系统交叉验证（化虚基线 / 亡者博物馆时间戳 / 续命路径 QA）
- P3 ⬜ 遗念 agent deathInsight tool

---

## §0 设计轴心

- [ ] **善终/横死二元**：老死（寿元归零）= 善终；一切非自然终结（战斗 / 域崩 / 天罚 / 被截杀）= 横死。两者在亡者博物馆展示、续命门限、agent 叙事权重上有不同处理
- [ ] **风烛**：寿元告急时身体衰退的物理表现，不是"惩罚性 debuff"而是自然状态——天道冷漠，不因此施救也不因此加速死亡；agent 可据此撰写更丰富的临终叙事
- [ ] **遗念 deathInsight**：死亡瞬间 agent 读取生平卷（含心魔劫记录 + 真元染色 + 重大事件），生成「遗念」——后续以 NPC 对白或地点随机台词形式在死亡坐标附近触发

---

## §1 善终 vs 横死 `DeathKind`（P0）

**现状**：`server/src/cultivation/life_record.rs` 中 `BiographyEntry::Terminated { cause: String, tick: u64 }` 仅记录 cause 字符串，无结构化死亡类型。

**实装**：

```rust
// server/src/cultivation/life_record.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeathKind {
    GoodDeath,     // 善终：寿元归零，自然老死
    ViolentDeath,  // 横死：战斗 / 域崩 / 天罚 / 截杀（一切非寿元耗尽的死亡）
}

// BiographyEntry::Terminated 增加 death_kind 字段
Terminated { cause: String, death_kind: DeathKind, tick: u64 }
```

- 触发路径：
  - `lifespan.rs` 寿元归零系统 → `DeathKind::GoodDeath`
  - `death_lifecycle.rs` 战斗死亡 / `tribulation.rs` 截杀死亡 / 域崩横死 → `DeathKind::ViolentDeath`
- 亡者博物馆（`plan-death §5`）按 `death_kind` 区分展示样式（建议：善终金色文本 / 横死红色文本）
- 续命门限设计参考：`DeathKind::GoodDeath` 路径才允许"含笑而终"特殊遗言（横死无此分支）；续命后再自然老死仍为 `GoodDeath`

**可核验交付物**：
- `server/src/cultivation/life_record.rs` 新增 `DeathKind` enum + `BiographyEntry::Terminated` 字段扩展
- `death_lifecycle::death_kind_*` 单测（至少 6 条）：
  - 寿元归零 → `GoodDeath`
  - 战斗死亡 → `ViolentDeath`
  - 域崩横死 → `ViolentDeath`
  - 截劫被杀 → `ViolentDeath`
  - serde round-trip（JSON 序列化 / 反序列化）
  - 亡者博物馆 schema 含 `death_kind` 字段

---

## §2 风烛状态 `WindCandleComponent`（P1）

**现状**：worldview §三 描述修士晚年进入"风烛残年"状态，但 `WindCandle` 相关 component 不存在，无数值、无触发条件。

**设计**（建议值，待平衡确认后定案）：

| 参数 | 建议值 | 说明 |
|---|---|---|
| 触发阈值 | 剩余寿元 ≤ 5%（`years_remaining / cap_years`）| 化虚 51.5y → 约 2.5y（2.5h real time）内触发 |
| 移速减益 | −20% | `MovementSpeedModifier` 接口 |
| 真元上限减益 | −10% | `QiCapModifier` 接口 |
| HUD 提示 | 顶栏红色"风烛残年 · 寿元 X%" | `plan-HUD-v1` 顶栏 API |

**实装**：

```rust
// server/src/cultivation/lifespan.rs
#[derive(Component)]
pub struct WindCandleComponent {
    pub triggered_at_tick: u64,
}
```

- `lifespan_tick_system` 每 lingtian-tick 检查剩余寿元比例：
  - `< 5%` 且无 `WindCandleComponent` → insert（触发减益 + HUD 提示）
  - `>= 5%`（续命后）且有 `WindCandleComponent` → remove（解除）
- 减益通过 `MovementSpeedModifier` / `QiCapModifier`（与 plan-combat / plan-cultivation 对齐，若尚无统一接口则 P1 先定义最小接口）

**可核验交付物**：
- `WindCandleComponent` struct + 触发/解除逻辑（`lifespan.rs`）
- `wind_candle_*` 单测（至少 6 条）：
  - 剩余寿元刚过 5% 阈值 → 触发 `WindCandleComponent`
  - 剩余寿元高于阈值 → 不触发
  - 续命后寿元回升 → `WindCandleComponent` 被 remove
  - 减益数值（−20% 移速 / −10% 真元上限）正确应用
  - 风烛状态下老死 → `DeathKind::GoodDeath`（不因风烛而变横死）

---

## §3 寿元系统交叉验证（P2）

**现状**：`LIFESPAN_TICKS_PER_YEAR = 60*60*20`（1 real hour = 1 game year）已在 `lifespan.rs`（行 29–30）实装。各续命合约（行 94–154）亦已实装，但以下三类对齐尚未验证。

**验证项 A — 化虚基线 51.5h**：
- worldview §三：化虚修士寿限 ≈ 51.5h（real time）= 51.5 game years
- 核查 `cap_by_realm[化虚]` 常量是否确实为 51.5（或等效值）
- 单测：`cap_for_realm(HuaXu) == 51.5 years`

**验证项 B — 亡者博物馆时间戳**：
- `BiographyEntry` 的 `tick` 字段展示时需统一：是换算为 game year 显示（`tick / LIFESPAN_TICKS_PER_YEAR` 年），还是 real timestamp？
- 建议：亡者博物馆显示 game year（"活了 X 年"），管理后台保留 real timestamp（tick 原值）

**验证项 C — agent 叙事节奏**：
- 1h real = 1 game year，天道长线叙事中"这修士活了 30 年"对应 30h real time
- 核实 agent 叙事素材生成频率（每多少 game year 生成一次 narration）是否覆盖完整生命周期

**验证项 D — 续命路径 QA**：
- `PillExtensionContract`（cost_factor = 0.01）/ `CollapseCoreExtensionContract` / `EnlightenmentExtensionContract` 三路径各手动走一遍
- 核实：续命后 cap 是否正确刷新；续命后自然老死仍为 `GoodDeath`；续命后寿元与 `WindCandleComponent` 交互（续命解除风烛）

**可核验交付物**：
- `lifespan::realm_cap_alignment` 单测（化虚 cap = 51.5 年常量断言）
- `lifespan::biography_tick_display` 单测（tick 换算为 game year 正确）
- 手动 QA 记录：三条续命路径各走一遍，记 cost + 最终寿元 + death_kind

---

## §4 遗念 agent deathInsight（P3）

**背景**：`plan-combat-no_ui` §1.5 标注"遗念 agent `deathInsight` tool：跨到修炼 plan + agent-v2 scope，等 death-lifecycle 立项时再对齐"。death-lifecycle-v1 已归档，本 plan 承接该项。

**设计**：

```
死亡事件 →
  server 发 Redis: bong:death_insight_request
    { char_id, biography_json, realm, qi_color, location: BlockPos }
  → agent deathInsight tool 读取：
      - 生平卷（最近 5 大事件 + 心魔劫记录 `heart_demon_record`）
      - 真元染色
      - 死亡类型（GoodDeath / ViolentDeath）
  → 生成 1–3 段「遗念」叙事（100–200 字，天道冷漠语调 worldview §八）
  → server 存 Remnant { char_id, location, text, expires_at_tick }
  → 死亡坐标 30 格内 NPC 对白 / 地点随机台词触发
```

- 遗念过期：按 game year（如 5 年 = 5h real time）自动清除，亡者博物馆可永久存档快照
- 遗念不可主动查询（天道不偏袒；玩家只能在死亡现场偶遇）

**前置依赖**：
- `plan-narrative-v1`（agent 叙事接口）或 agent 独立 tool 扩展
- `plan-death-lifecycle-v1` §5 亡者博物馆（Remnant 展示入口）

**可核验交付物**：
- `bong:death_insight_request` Redis channel schema（`agent/packages/schema/src/death_insight.ts`，新文件）
- agent `deathInsight` tool 接口定义
- `Remnant` struct（`server/src/cultivation/remnant.rs`，新文件）
- 遗念触发系统（死亡坐标 30 格内检测 + NPC 台词注入）

---

## §5 数据契约

| 契约 | 位置 |
|---|---|
| `DeathKind` enum | `server/src/cultivation/life_record.rs` |
| `BiographyEntry::Terminated.death_kind` | `server/src/cultivation/life_record.rs` |
| `WindCandleComponent` | `server/src/cultivation/lifespan.rs` |
| `LIFESPAN_REALM_CAP_*` 常量（含化虚 = 51.5 验证）| `server/src/cultivation/lifespan.rs`（已有，需核实）|
| `bong:death_insight_request` channel | `agent/packages/schema/src/death_insight.ts`（新文件）|
| `Remnant` struct | `server/src/cultivation/remnant.rs`（新文件）|

---

## §6 实施节点

- [ ] **P0**：`DeathKind` enum + `BiographyEntry::Terminated` 字段扩展 + 所有死亡触发点分类 + serde round-trip + 单测（≥6）
- [ ] **P1**：`WindCandleComponent` + 触发/解除 tick 系统 + `MovementSpeedModifier` / `QiCapModifier` 减益接入 + HUD 顶栏提示 + 单测（≥6）
- [ ] **P2**：化虚 cap 对齐单测 + 时间戳显示统一 + 续命路径手动 QA 记录
- [ ] **P3**：`bong:death_insight_request` schema + agent `deathInsight` tool 接口 + `Remnant` 存储 + 死亡现场触发系统

---

## §7 开放问题

- [ ] `DeathKind::ViolentDeath` 是否需要细化子类型（战斗 / 域崩 / 天罚 / 截劫），还是 `cause` 字符串已足够？（建议先用字符串，v2+ 再结构化）
- [ ] 风烛减益数值（−20% 移速 / −10% 真元上限）待 Phase 0–1 上线后根据玩家反馈调整
- [ ] 遗念 `Remnant` 过期机制：按 game year（5 年 = 5h）还是永久保留到亡者博物馆审查？
- [ ] 遗念能否被玩家"驱散"？（叙事层面：强行驱散遗念是否有道德 / 因果代价，还是天道漠视此行为）
- [ ] 遗念与被截劫死亡的处理：截杀者是否在遗念中被点名？（天道不偏袒，建议只写修士自身的执念，不暴露凶手）

---

## §8 进度日志

- 2026-04-28：骨架立项。来源：`docs/plans-skeleton/reminder.md` plan-death-lifecycle-v1 节（4 条遗留）+ plan-combat-no_ui 节（`deathInsight` tool）。代码核查：续命路径已实装（`lifespan.rs` 94–154）、`LIFESPAN_TICKS_PER_YEAR` 已实装（行 29）→ 两条已实装项从 reminder 删除；`DeathKind` 枚举、`WindCandleComponent` 均不存在，为新建目标；`deathInsight` tool 无任何实装。
