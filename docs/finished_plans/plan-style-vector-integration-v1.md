# Bong · plan-style-vector-integration-v1

**流派身份 = 真元色向量**——把现有 `PracticeLog` + `QiColor` evolution 系统接入 6 流派 P0 实装路径，使流派"用什么打 → 染什么色"的物理因果链贯通。**取代已撤销的 plan-styles-emergent-refactor-v1**（错方向：试图扩 `UnlockedStyles` bool 字段；实际 `QiColor` 已实装向量演化）。

**Primary Axis**（worldview §五"流派由组合涌现" 2026-05-03 正典）：**真元色向量演化的物理因果链贯通度** — 玩家用什么流派 → PracticeLog 累积 → QiColor 演化 → 染色组合呈现的端到端链路完整性

## 阶段总览

| 阶段 | 状态 | 验收 |
|---|---|---|
| **P0** PracticeLog.add() 各流派 P0 接入（6 流派各自 P0 实装时同步加 hook）| ⬜ | — |
| **P1** 完整死亡清空 PracticeLog + QiColor（修 on_player_terminated bug）+ inspect 神识看对方 QiColor schema/client 通道 | ⬜ | — |
| P2 v1 收口（饱和 testing + 客户端 HUD 渲染 QiColor 自己/对方双视角） | ⬜ | — |

> **vN+1 (plan-style-vector-integration-v2)**：dugu 染色遮蔽（用户保留至天赋点系统做）/ 多周目"知识继承"机制 / 染色亲和度对各流派招式效率的精确加成

---

## 背景：方向修正

2026-05-03 user 反馈："**流派应自由涌现，类似于朝一个方向的向量，互相叠加最后得出的最终向量方向颜色。不具备可逆性，但是可以从大致颜色+经验判断，是什么流派的，同时也可以造成迷惑性**"。

调研发现：
- ✅ `cultivation::color::PracticeLog` 已实装权重累积 + 衰减
- ✅ `evolve_qi_color()` 纯函数已实装阈值判定（main > 60% / secondary > 25% / chaotic / hunyuan）
- ✅ library `真元十一色考` 已正典："染色非唯时日所致，更关乎'心之所向'" + 杂色 trade-off
- ❌ **缺**：`PracticeLog.add()` 无 caller — 注释 "P1：实际练习事件来源由上层后续接入" 至今未接
- ❌ **缺**：`on_player_terminated` 只清 Cultivation/MeridianSystem/Contamination，**漏清 PracticeLog/QiColor**
- ❌ **缺**：inspect 神识看对方 QiColor 的客户端通道（schema 有 UnlocksSyncV1 但用于"自己"看）

→ 真正缺口不是新建字段，是把**已实装的向量演化机制**接入流派 P0 + 修复死亡清空 bug + 加 inspect 通道。

---

## 世界观 / library / 交叉引用

**worldview 锚点**：
- §五"流派由组合涌现"（2026-05-03 正典）— 五维组合 / 不锁触发 / 流派 ⇄ 染色映射表
- §五:482 "流派反过来塑造染色" — 物理因果链
- §六.2 真元染色 — 长期修习的物理沉淀

**library 锚点**：
- `cultivation/真元十一色考` — 11 色 + 杂色 trade-off + "染色关乎心之所向"正典
- `peoples-0006 战斗流派源流` — 流派 ⇄ 染色对应原文

**交叉引用**：
- `plan-cultivation-v1` ✅（已落地）— **核心接入**：`PracticeLog` / `evolve_qi_color()` / `qi_color_evolution_tick` 已有
- 6 流派 plan：
  - `plan-baomai-v1` ✅ finished — P0 已实施，需补 PracticeLog.add(Heavy, X) hook（vN+1 或本 plan P0 协调补）
  - `plan-anqi-v1` 🟡 active — P0 实装时加 PracticeLog.add(Solid, X) hook
  - `plan-zhenfa-v1` 🟡 active — P0 实装时加 PracticeLog.add(Meticulous, X) hook
  - `plan-dugu-v1` 🟡 active — P0 实装时加 PracticeLog.add(Yin, X) hook
  - `plan-zhenmai-v1` 🟡 active — P1 实装时加 PracticeLog.add(Violent, X) hook
  - `plan-tuike-v1` 🟡 active — P0 实装时加 PracticeLog.add(Solid, X) hook（与器修同源）
  - `plan-woliu-v1` ✅ finished — P0 已实施，需补 PracticeLog.add(Meticulous, X) hook（vN+1 或本 plan 协调补）
- `plan-perception-v1.1` ✅（已落地）— P1 加"看对方 QiColor"通道
- `plan-death-lifecycle-v1` ✅（已落地）— P1 修 `on_player_terminated` 漏清 bug
- `plan-styles-emergent-refactor-v1` ❌（**已撤销**，本 plan 取代）

## 接入面 checklist（防孤岛）

- **进料**：6 流派 P0 各自的"成功命中" event/system → 对应 `PracticeLog.add(对应 ColorKind, amount)` 调用
- **出料**：`PracticeLog` 累积 → `qi_color_evolution_tick` 已有 → `QiColor` 演化 → schema/client 推 inspect HUD
- **共享类型 / event**：复用 `PracticeLog` / `QiColor` / `ColorKind` / `evolve_qi_color()`；新增 inspect schema `QiColorObservedV1`（看对方）+ 修复 `on_player_terminated` 清空范围
- **跨仓库契约**：
  - server: `cultivation::color::PracticeLog` 已 ✅，本 plan 接入 6 流派 caller / `cultivation::death_hooks::on_player_terminated` 扩 PracticeLog + QiColor 移除 / `network::qi_color_observed_emit.rs` (新)
  - schema: `agent/packages/schema/src/qi_color_observed.ts` (新) — `QiColorObservedV1` payload 给"看对方"
  - client: `bong:cultivation/qi_color_observed` (outbound, 新) HUD inspect 渲染对方 QiColor / 自己 QiColor 走已有 `cultivation_state` 路径

---

## §A 概览（设计导航）

> 流派身份 = 真元色向量。玩家用什么流派打架 → PracticeLog 对应色 +N → qi_color_evolution_tick 演化 QiColor.main/secondary/is_chaotic/is_hunyuan → 渲染 HUD。完整死亡清空全套，inspect 神识看对方染色推流派。**v1 不动 UnlockedStyles**（保留现状作 HUD 门禁），仅在向量层接入。

### A.0 v1 实装范围（2026-05-03 拍板）

| 维度 | v1 实装 | 搁置 vN+1 |
|---|---|---|
| PracticeLog.add() 接入 | **6 流派 P0 各自加 hook**（每次成功命中 +N） | 凝核/丹药/遗念等其他来源 |
| 流派 → ColorKind 映射 | **worldview §五"流派 ⇄ 染色"表锚定** | 多色混合贡献 |
| 完整死亡清空 | **修 on_player_terminated 漏清 PracticeLog/QiColor** | 寿元未尽死亡保留（已实装：不触发 PlayerTerminated）|
| inspect 神识看对方 QiColor | **schema 新增 QiColorObservedV1 + client mirror** | 神识等级 / 距离衰减 |
| 染色亲和度对招式效率 | **不实装具体加成数值**（仅留 hook，各流派 vN+1 接入） | 完整加成数值表 |
| dugu 染色遮蔽 | ❌ **不实装**（user 决议留 vN+1 天赋点系统） | dugu 师 inspect 时遮蔽真实 QiColor |
| UnlockedStyles | **不动**（保留现状作 HUD 门禁） | 完全废弃 / 改"已用过标记" |
| 客户端 QiColor HUD | **自己已有 cultivation_state 路径** + 新增看对方 inspect 通道 | 双方 QiColor 对照 / 历史变化曲线 |

### A.1 流派 → ColorKind 映射（worldview §五 锚定）

| 流派 | ColorKind | hook 位置 | 触发条件 |
|---|---|---|---|
| 体修·爆脉 | `ColorKind::Heavy` | plan-baomai-v1 burst meridian attack 命中 | hit + qi_invest > 0 |
| 器修·暗器 | `ColorKind::Solid` | plan-anqi-v1 ThrowCarrierIntent 命中 | hit + qi_payload > 0 |
| 地师·阵法 | `ColorKind::Meticulous` | plan-zhenfa-v1 诡雷 trigger 命中 | trigger + 范围内有 entity |
| 毒蛊 | `ColorKind::Yin` | plan-dugu-v1 DuguPoisonState 挂载 | DuguPoisonState attached |
| 截脉·震爆（防）| `ColorKind::Violent` | plan-zhenmai-v1 jiemai effectiveness > 0 | DefenseTriggered::JieMai with effectiveness > 0 |
| 替尸·蜕壳（防）| `ColorKind::Solid` | plan-tuike-v1 ShedEvent layers_shed > 0 | ShedEvent triggered（与器修同源）|
| 绝灵·涡流（防）| `ColorKind::Meticulous` | plan-woliu-v1 ProjectileQiDrainedEvent | vortex 拦截成功（与阵法同源）|

### A.2 触发数值（PracticeLog.add() 的 amount）

```rust
// 流派成功使用一次 → 对应色 +N
const STYLE_PRACTICE_AMOUNT: f64 = 1.0;       // v1 起手所有流派统一 1.0
                                               // vN+1 可按招式威力分级（如重狙 +2.0 / 凝针 +0.5）

// PracticeLog.decay_per_tick 已实装但 default 0
// v1 设为 0.001（每 tick 衰减 0.001 / 大约 50min 衰减 1 单位）
// 含义：流派认证不会很快失效，但完全不练会慢慢淡化
const PRACTICE_DECAY_PER_TICK: f64 = 0.001;
```

**含义**（v1 数值表）：
- 玩家连续 60 次成功用 anqi（1h 内）→ Solid 色权重 = 60
- 玩家不练任何流派 50 min → 现有权重各色 -1
- 如果 Solid 已 60 而 Heavy 0 → main = Solid（>60% 阈值满足）
- 玩家"故意混用"3 流派各 30 → is_chaotic = true（worldview "三色相争威力 -50%"）

### A.3 死亡清空机制（修 on_player_terminated bug）

**当前 `cultivation::death_hooks::on_player_terminated` 只移除**：
- `Cultivation` ✅
- `MeridianSystem` ✅
- `Contamination` ✅

**v1 P1 补充移除**：
- `PracticeLog`（流派"心之所向"清空）
- `QiColor`（染色清空）

**触发条件**（v1 不变）：
- ✅ 完整死亡（NearDeath 后未救回 / 寿元到期 / 渡劫失败）→ `PlayerTerminated` 触发 → 清空
- ❌ 寿元未尽死亡（运数复活 / NearDeath 自救成功）→ 不触发 `PlayerTerminated` → **保留** PracticeLog/QiColor（journey M.3 多周目实力归零原则）

### A.4 inspect 神识看对方 QiColor

**schema 新增**（`agent/packages/schema/src/qi_color_observed.ts`）：

```typescript
export const QiColorObservedV1 = Type.Object({
    observer: PlayerIdV1,        // 谁施神识
    observed: PlayerIdV1,        // 看的对象
    main: ColorKindV1,
    secondary: Type.Optional(ColorKindV1),
    is_chaotic: Type.Boolean(),
    is_hunyuan: Type.Boolean(),
    realm_diff: Type.Number(),   // observer.realm.tier - observed.realm.tier
});
```

**境界差阈值**（沿用 plan-perception-v1.1 默认）：
- Δ ≥ 2 完全识破（看到全部 main/secondary/chaotic/hunyuan）
- Δ = 1 模糊化（仅看到 main，不看 secondary/flags）
- Δ ≤ 0 屏蔽（看不到任何 QiColor）

**v1 不实装 dugu 染色遮蔽**（user 决议留 vN+1 天赋点系统）—— v1 dugu 师阴诡色直接被高境 inspect 看到 = 暴露。这是 v1 已知偏离正典，等 plan-talent-v1 落地后通过"染色遮蔽"天赋点修复。

### A.5 v1 实施阶梯

```
P0  PracticeLog.add() 6 流派 hook 接入
       baomai ✅ finished — 已落地 plan 不动，本 plan P0 加 hook 通过 cross-system patch
       anqi P0 实装时同步加 PracticeLog.add(Solid, 1.0)
       zhenfa P0 实装时同步加 PracticeLog.add(Meticulous, 1.0)
       dugu P0 实装时同步加 PracticeLog.add(Yin, 1.0)
       zhenmai P1 实装时同步加 PracticeLog.add(Violent, 1.0)
       tuike P0 实装时同步加 PracticeLog.add(Solid, 1.0)
       woliu ✅ finished — 已落地 plan 不动，本 plan P0 加 hook 通过 cross-system patch
       PracticeLog.decay_per_tick = 0.001 全玩家 default
       ↓
P1  完整死亡清空修复 + inspect 通道
       on_player_terminated 扩展移除 PracticeLog + QiColor
       schema/client_request.qi_color_observed.ts 新增
       network/qi_color_observed_emit.rs 监听神识 inspect intent
       client UnlocksSyncHandler-style 镜像 + HUD 渲染对方 QiColor
       ↓ 饱和 testing
P2  v1 收口
       客户端 inspect HUD 整合（自己 QiColor + 看对方 QiColor 双视角）
       LifeRecord "X 染色演化为 main=Y / chaotic" 事件
       agent narration 接入（QiColor 重大变化时触发暗语）
```

### A.6 v1 已知偏离正典（vN+1 必须修复）

- [ ] **dugu 染色遮蔽**（worldview "毒蛊师暴露 = 全陆追杀" 矛盾）—— v1 dugu 师阴诡色直接 inspect 暴露；vN+1 天赋点"染色遮蔽"修复（user 2026-05-03 决议）
- [ ] **染色亲和度对招式效率精确加成**（如凝实色 + anqi 衰减 -0.03/格 已正典化 plan-combat-no_ui §3.2）—— v1 仅留 hook，各流派 vN+1 实装具体加成数值
- [ ] **染色养成路径**（凝核/丹药/遗念等 PracticeLog 来源）—— v1 仅 6 流派招式贡献，丹药等 vN+1（接 plan-color-v1 / plan-alchemy）
- [ ] **dual amount by 招式威力**（v1 全统一 1.0；vN+1 按 hit_qi 分级，如重狙 +2.0 / 凝针 +0.5）

### A.7 v1 关键开放问题

**已闭合**（基于代码调研 + worldview）：
- 字段实装形态：✅ 不需要新字段（PracticeLog + QiColor 已实装）
- 跨 plan 协调时序：✅ 6 流派 plan 实装时同步加 hook（顺序解耦）
- Default 改造：✅ 不改 UnlockedStyles，PracticeLog default 已是空 weights
- dugu obfuscation：✅ 留 vN+1 天赋点系统（user 决议）
- 多周目继承：✅ 完整死亡清空 / 寿元未尽死亡保留（journey M.3 + 当前代码逻辑）

**仍 open**（v1 实施时拍板）：
- [ ] **Q148. 各流派"成功"阈值具体定义**：v1 起手"命中 + qi_invest/qi_payload > 0 / 触发了对应 component"；P0 实装时各流派各自细化（jiemai effectiveness > 0 vs > 0.3 / anqi hit + contam > 0 等）
- [ ] **Q149. PracticeLog.decay_per_tick 数值校准**：v1 起手 0.001（50min 衰减 1 单位）；P2 实装时按运营数据调
- [ ] **Q150. 多人协作触发归属**：A 用 anqi 凝针 + B 用 dugu 灌毒 → 命中 target → 谁的 PracticeLog +N？建议 **attacker (anqi 玩家) +Solid + dugu infuser (灌毒玩家) +Yin 双方独立累积**（不冲突）—— P0 拟
- [ ] **Q151. inspect 神识看对方 QiColor 的 trigger**：是 perception_tick 自动推（玩家进入感知范围即看到）还是玩家主动 inspect intent 触发？建议**主动 inspect intent**（避免高频 server emit）—— P1 拟
- [ ] **Q152. 已 finished 的 baomai/woliu plan 加 PracticeLog hook 走 cross-system patch 还是直接修代码**：建议**直接修代码 + 在 plan finish_evidence 加 follow-up 备注**（不动 plan 本身的归档状态）—— P0 拟

---

## §1 数据契约

### v1 P0 落地清单

| 模块 | 文件路径 | 核心内容 |
|---|---|---|
| PracticeLog amount/decay 常量 | `server/src/cultivation/color.rs` | `STYLE_PRACTICE_AMOUNT = 1.0` / 默认 `decay_per_tick = 0.001` |
| 6 流派 hook 接入 | 各流派 P0 实施 PR 同期 | `plan-baomai/anqi/zhenfa/dugu/zhenmai/tuike/woliu` 各自成功事件后加 `practice_log.add(对应色, STYLE_PRACTICE_AMOUNT)` |
| Default decay_per_tick | `server/src/inventory/mod.rs` `insert(PracticeLog::default())` | 改为 `insert(PracticeLog { decay_per_tick: 0.001, ..Default::default() })` |

### v1 P1 落地清单

| 模块 | 文件路径 | 核心内容 |
|---|---|---|
| 完整死亡清空修复 | `server/src/cultivation/death_hooks.rs::on_player_terminated` | 在现有移除 Cultivation/MeridianSystem/Contamination 之后加 `PracticeLog` + `QiColor` 移除 |
| inspect schema | `agent/packages/schema/src/qi_color_observed.ts` (新) | `QiColorObservedV1` |
| inspect emit 系统 | `server/src/network/qi_color_observed_emit.rs` (新) | 监听玩家 inspect intent → 按境界差判定 → emit `QiColorObservedV1` |
| client mirror | `client/src/main/java/com/bong/client/network/QiColorObservedHandler.java` (新) | 接 schema + 写 store |
| HUD 渲染对方 QiColor | `client/.../hud/InspectQiColorPlanner.java` (新或扩展) | 在 inspect 屏内显示对方 QiColor main/secondary 染色色块 |

### v1 P2 落地清单

| 模块 | 文件路径 | 核心内容 |
|---|---|---|
| LifeRecord QiColor 变化 | `server/src/lore/life_record.rs` | "X 染色演化为 main=Y" / "X 真元已染杂色" 事件类型 |
| Agent narration | `agent/packages/tiandao/src/qi-color-narration.ts` | QiColor.main 改变 / is_chaotic 翻转 / is_hunyuan 触发时 narration |
| 单测 | `server/src/cultivation/color.rs::tests` | 已有；本 plan 加 6 流派 hook 集成测 + 死亡清空测 + inspect 境界差测 |

---

## §2 实施节点

详见 §A.5 v1 实施阶梯。三阶段：

- [ ] **P0** 6 流派 PracticeLog hook 接入 + decay 默认值 — 见 §A.5
- [ ] **P1** 死亡清空修复 + inspect 通道 — 见 §A.5
- [ ] **P2** LifeRecord + agent narration + HUD 整合 — 见 §A.5

---

## §3 进度日志

- 2026-05-03：plan 立项。**起源**：plan-styles-emergent-refactor-v1 撤销（user 反馈"流派身份 = 向量，不是 bool 字段"）；本 plan 取代。**核心方向**：把已实装的 PracticeLog + QiColor evolution 系统接入 6 流派 P0；修 on_player_terminated 漏清 bug；新加 inspect 神识看对方 QiColor 通道。**关键正典**：worldview §五"流派由组合涌现"（2026-05-03 commit 94c32a04）+ library 真元十一色考"染色非唯时日所致，更关乎心之所向"+ 杂色 trade-off。**v1 不实装 dugu 染色遮蔽**（user 决议留 vN+1 天赋点系统）。

## Finish Evidence

### 落地清单

- **P0 PracticeLog hook / decay**：
  - `server/src/cultivation/color.rs`：新增 `STYLE_PRACTICE_AMOUNT = 1.0`、`PRACTICE_DECAY_PER_TICK = 0.001`、`record_style_practice()`；`PracticeLog::default()` 直接带默认衰减。
  - `server/src/cultivation/burst_meridian.rs`：爆脉命中后记录 `ColorKind::Heavy`。
  - `server/src/zhenfa/mod.rs`：诡雷/阵法触发成功后记录 `ColorKind::Intricate`（缜密色）。
  - `server/src/combat/resolve.rs`：截脉震爆成功防御后记录 `ColorKind::Violent`。
  - `server/src/combat/woliu.rs`：涡流成功抽干投射真元后记录 `ColorKind::Intricate`。
- **P1 死亡清空 + inspect 通道**：
  - `server/src/cultivation/death_hooks.rs`：`on_player_terminated` 同步移除 `PracticeLog` 与 `QiColor`。
  - `agent/packages/schema/src/client-request.ts` / `server/src/schema/client_request.rs`：新增 `qi_color_inspect` C2S request。
  - `agent/packages/schema/src/server-data.ts` / `server/src/schema/server_data.rs`：新增 `QiColorObservedV1` S2C payload。
  - `server/src/network/qi_color_observed_emit.rs`：按境界差 `>=2 / =1 / <=0` 生成完整、模糊或屏蔽的对方真元色观察。
  - `client/src/main/java/com/bong/client/network/QiColorObservedHandler.java`、`client/src/main/java/com/bong/client/cultivation/QiColorObservedStore.java`：客户端镜像观察结果。
  - `client/src/main/java/com/bong/client/inventory/InspectScreenBootstrap.java`、`client/src/main/java/com/bong/client/inventory/InspectScreen.java`、`client/src/main/java/com/bong/client/inventory/model/MeridianBody.java`：打开检视时向准星目标发起 inspect，并在检视界面渲染自己/对方 QiColor。
- **P2 收口 / narration**：
  - `server/src/cultivation/color.rs`：`qi_color_evolution_tick` 在 main/secondary/chaotic/hunyuan 变化时写入 `BiographyEntry::ColorShift`。
  - `server/src/schema/cultivation.rs`、`agent/packages/schema/src/cultivation.ts`、`agent/packages/schema/generated/world-state-v1.json`：WorldState cultivation snapshot 携带 `qi_color_chaotic` / `qi_color_hunyuan`，供 agent 判定重大变化。
  - `agent/packages/tiandao/src/qi-color-narration.ts`、`agent/packages/tiandao/src/runtime.ts`：`QiColorNarrationTracker` 订阅 fresh world state 的玩家真元色变化，发布 `bong:agent_narrate` narration。

### 关键 commit

- `3d2fbc72` · 2026-05-04 · `feat(style-vector): 接入真元色实践链`
- `4f253df2` · 2026-05-04 · `feat(style-vector): 增加真元色神识通道`
- `f50c1811` · 2026-05-04 · `feat(client): 渲染检视真元色`
- `8753391d` · 2026-05-04 · `feat(style-vector): 接入真元色叙事`

### 测试结果

- `server/`：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` ✅ `2223 passed`
- `agent/`：`npm run build` ✅
- `agent/packages/tiandao`：`npm test` ✅ `33 passed / 229 tests`
- `agent/packages/schema`：`npm test` ✅ `9 passed / 267 tests`
- `client/`：`JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 PATH=/usr/lib/jvm/java-17-openjdk-amd64/bin:$PATH ./gradlew test build` ✅ `BUILD SUCCESSFUL`

### 跨仓库核验

- **server**：`PracticeLog` / `record_style_practice` / `STYLE_PRACTICE_AMOUNT` / `PRACTICE_DECAY_PER_TICK` / `QiColorInspectRequest` / `emit_qi_color_observed_payloads` / `BiographyEntry::ColorShift` / `CultivationSnapshotV1.qi_color_chaotic`
- **agent/schema**：`ClientRequestQiColorInspectV1` / `QiColorObservedV1` / `CultivationSnapshotV1.qi_color_chaotic` / `CultivationSnapshotV1.qi_color_hunyuan`
- **agent/tiandao**：`QiColorNarrationTracker` / `renderQiColorNarration`
- **client**：`QiColorObservedHandler` / `QiColorObservedStore` / `InspectScreenBootstrap.requestQiColorInspectForCrosshairTarget` / `MeridianBody.qiColorMain`

### 遗留 / 后续

- `dugu` 染色遮蔽仍按本 plan 决议留给 vN+1 天赋点系统。
- `anqi`、`dugu`、`tuike` 运行时模块在本次消费基线 `origin/main` 尚未全部落入主线；本 plan 已提供统一 `record_style_practice()` 接口和当前主线可落地点，后续对应流派 PR 合入后按同一接口补各自成功事件 hook。
- 染色亲和度对招式效率的精确加成、丹药/凝核/遗念等非流派 PracticeLog 来源仍留给 vN+1。
