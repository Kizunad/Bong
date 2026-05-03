# Bong · plan-woliu-v1

**绝灵·涡流流**（防御）。**算计型防御**：toggle 开关持涡，自身减速 80%，在身周强造**相对环境的局部负灵域**——敌之真元投射物入此涡流被天地法则抽干。**算计型 ≠ 反应型**（zhenmai 弹反 / tuike 物资 / **woliu 算计**）。

**Primary Axis**（worldview §五:467 已正典）：**真元流速 + 池效率**（**持久博弈型**，非爆发）

## 阶段总览

| 阶段 | 状态 | 验收 |
|---|---|---|
| **P0** 持涡 toggle + 凝脉 Δ=0.25 拦截 + 反噬 + 信息分层（神识/体感） + 修正 vortex_fake_skin 命名错位 | ⬜ | — |
| **P1** 境界分级 Δ + 多因子半径公式 + max 叠加（团战雏形）+ 抗灵压丹 + agent narration | ⬜ | — |
| P2 v1 收口（饱和 testing + 数值平衡） | ⬜ | — |

> **vN+1 (plan-woliu-v2)**：旋转方向（顺/逆）+ 顺逆相消叠加 + 短时瞬涡变体 + 减速降低功法
>
> **vN+2**：完整流体算法（涡心受合力位移 / 异向涡碰撞涡崩）+ 化虚 0.9 抽干 + 远程引导负灵压 + 太极反击

---

## 世界观 / library / 交叉引用

**worldview 锚点**：
- §五.防御.3 绝灵/涡流流（line 442-445：掌心/盾面局部负灵域 -0.8 / 真元抽干 / 反噬瞬间抽自身经脉永久残疾 / 算计型玩家）
- §五:467 流派 primary axis 表（**真元流速 + 池效率**——持久博弈型）
- §五:472 "**涡流流不是输出型**——primary axis 是流速 + 池效率，强在长时间博弈而非爆发，是'算计型'防御"（**正典化的算计型定位**）
- §五:450 克制关系（"毒蛊→涡流极亏 / 爆脉→涡流无可吸"）
- §二 负灵域（line 1406+：负压 / 灵气易挥发 / 离体真元 vs 活真元区分）
- §三 经脉系统（permanent SEVERED 反噬路径）

**library 锚点**：
- `peoples-0006 战斗流派源流`（防御三·绝灵/涡流流原文："**所修者，掌心/盾面强造一相对环境之负灵域；所打者，敌真元入涡流被天地抽干**"）
- `cultivation/烬灰子内观笔记 §二·论噬`（"风口"理论 — 涡流流 = 主动开风口，平常人怕风口）
- `ecology/末法药材十七种`（**空兽痕** → 抗灵压丹，减反噬概率）
- `geography/血谷残志`（参考负灵域案例）

**交叉引用**：
- `plan-cultivation-v1` ✅（已落地）— `Cultivation.qi_current` 持续耗 qi（5-10/s by realm）+ `Meridian` 永久 SEVERED 反噬路径
- `plan-anqi-v1` 🟡（active P0）— **拦截目标**：`QiProjectile.qi_payload` 进入涡流场被抽
- `plan-dugu-v1` 🟡（active P0）— **拦截目标**：`QiNeedle.qi_payload`（1 点）被抽 → DuguPoisonState 不挂载（worldview "毒蛊→涡流极亏"实现）
- `plan-baomai-v1` ✅（已落地）— **不拦截**：爆脉近战不入涡流场（worldview "爆脉→涡流无可吸"自然实现）
- `plan-zhenmai-v1` 🟡（active P0/P1）— 互补防御：jiemai 反应 / woliu 算计；二者 status 互斥（VortexCasting 期间 block_defense）
- `plan-tuike-v1` 🟡（active P0/P1）— 互补防御：tuike 物资 contam 过滤 / woliu 算计 qi 抽干；可同时装备但操作模式互斥
- `plan-perception-v1.1` ✅（已落地）— **核心接入**：`SenseEntryV1` 系统支持神识 inspection vs 普通玩家体感分层显示
- `plan-tsy-dimension-v1` ✅（已落地）— 坍缩渊负灵域是 woliu 师"必死之地"（env_qi 已极低，再造涡流必反噬）
- `world::Zone.spirit_qi` ✅（已落地）— `server/src/world/` 已有 zone qi 系统（`server/src/world/events.rs:213+`）

## 接入面 checklist（防孤岛 — 严格按 docs/CLAUDE.md §二）

- **进料**：`Cultivation.qi_current` 持续耗 qi（持涡时）→ `Cultivation.realm` 决定 Δ 上限 + radius + maintain_max → `Zone.spirit_qi` 读取 env_qi → `Combat.position` 计算涡流场坐标 → `Meridian` 反噬时永久 SEVERED 写入
- **出料**：`VortexField` component 写入 → `vortex_intercept_tick` 系统拦截 `QiProjectile` / `QiNeedle.qi_payload` → 命中事务时 contam 大减 → 反噬 emit `VortexBackfireEvent` + `Meridian.flow_capacity = 0`（手经脉永久废）→ `bong:woliu/vortex_state` outbound 同步 client HUD
- **共享类型 / event**：复用 `Cultivation` / `Meridian` / `Zone.spirit_qi` / `QiProjectile` / `QiNeedle` / `StatusEffectKind`；新增 `VortexField` component / `VortexCastIntent` event / `VortexCasting` status variant / `VortexBackfireEvent` / 修正 `DerivedAttrs.vortex_fake_skin_layers` 命名错位
- **跨仓库契约**：
  - server: `combat::woliu::VortexField` component / `combat::woliu::VortexCastIntent` event / `combat::woliu::vortex_intercept_tick` system / `combat::woliu::vortex_maintain_tick` (耗 qi + 反噬检查) / `combat::woliu::cast_vortex` 系统 / 命名修正 `tuike_layers` 拆分 / **新增 `Technique::WoliuVortex`** 学习写入 `cultivation.techniques`（plan-hotbar-modify-v1 已落地的 Technique 系统）
  - schema: `agent/packages/schema/src/woliu.ts` → `VortexFieldStateV1` / `VortexBackfireEventV1` / 扩展 `SenseEntryV1::AmbientLeyline` 支持相对感知
  - client: **走 hotbar 已落地路径** — 玩家学到 WoliuVortex Technique 后在 InspectScreen "战斗·修炼" tab 绑到 **1-9 数字键槽**（战斗·修炼栏，**不是** F1-F9 快捷使用栏——后者是 consumable）→ 战斗按 1-9 数字键 → `UseQuickSlot { slot }` packet → server 解析 slot binding → 触发 `VortexCastIntent`；**不新建 woliu 专属 packet**。outbound `bong:woliu/vortex_state` 仍需要（HUD payload）+ perception module 新 ambient text 渲染
- **特性接入面（v1 仅留 hook 不实装）**：worldview §五:467 "持久博弈型，非爆发" — woliu 不绑染色 / 不绑特性
  - **真元流速特性**（zhenmai primary axis 泛型化）→ 持涡 qi cost ↓ — vN+1 接入
  - **真元池效率特性** → maintain_max ↑ — vN+1 接入
  - **缜密色（Solid 阵法师）**（worldview §六）→ 灵气差值感知敏锐，Δ 上限 +0.05 — vN+1 接入

---

## §A 概览（设计导航）

> 涡流流 = **算计型防御**：toggle 开持涡 → 减速 80% + 持续耗 qi → 形成相对负灵域吸干飞入的真元投射物 → 维持过久 → 反噬手经脉永久 SEVERED。worldview §五:472 已正典化"持久博弈型，非爆发"——不绑染色 / 不绑特性 / 不靠瞬间反应。

### A.0 v1 实装范围（2026-05-03 拍板）

| 维度 | v1 实装 | 搁置 vN+1 |
|---|---|---|
| **核心模式** | **算计型**（worldview §五:472 正典）| 反应型 / 数值堆型 |
| 涡流形态 | **持涡 only（toggle 开关，非按住）**（Q84: C 加细化）| 短时瞬涡变体 / 永久全场涡 |
| 减速代价 | **立即减速 80%**（move_speed_mul: 0.2）（Q84 + 问 2 A）| 减速降低功法 |
| Δ 公式 | **`local_qi = env_qi - vortex_delta`** 相对值 | 绝对值 -0.8 |
| 境界分级 Δ | **P0 凝脉默认 0.25 / P1 全境界 4 档**（引气 0.10 / 凝脉 0.25 / 固元 0.45 / 通灵 0.65）| 化虚 0.80（已断绝）|
| 拦截公式 | **`qi_payload × (1 - delta/0.8)`**（Q85: B 比例抽）| 化虚 0.9 抽干 / 远程引导 / 太极反击 |
| 反噬触发 | **维持过久 → 手经脉永久 SEVERED**（Q86: C，超 Δ 上限 v1 不可能因无超限功法）| 超限功法 hook |
| 环境 qi 显示 | **信息分层**（神识 inspection 数字 / 普通玩家相对体感）（Q87 reframe + 问 3 P0 实装）| 复杂多层境界差异 |
| 多人叠加 | **不叠加（独立 VortexField）**（问 1 v1 P0 简化）| max 叠加（v1 P1）/ 旋转方向 vN+1 / 完整流体 vN+2 |
| 涡流半径 | **基础 1.5 + 境界系数**（Q89 多因子起步）| + 功法 + 熟练度 + 真元流量 |
| 染色 / 特性 | ❌ 不绑（worldview §五:467/472 正典）| 缜密色 / 真元流速 / 池效率特性接入 |

### A.1 物理模型（worldview "风口"理论的逆向应用）

```
玩家在 InspectScreen "战斗·修炼" tab 绑 Technique::WoliuVortex 到 1-9 数字键槽
（plan-hotbar-modify-v1 已落地路径；F1-F9 是 consumable 不适用）
战斗按 1-9 数字键 → server 收 UseQuickSlot { slot } → 解析 binding =
WoliuVortex → 发 VortexCastIntent

VortexCastIntent { caster, toggle: On } →
  立即施加 StatusEffectKind::VortexCasting
    move_speed_mul: 0.2 (减速 80%)
    block_attack: true
    block_defense: true (block jiemai DefenseIntent)
    block_sprint: true

  read zone.spirit_qi (env_qi)
  determine vortex_delta by realm
  if env_qi < vortex_delta:
    cast 失败 + 立即反噬（试图在已是负灵域的地方再造涡流）
    return

  spawn VortexField {
    center: caster.hand_pos,
    radius: base_radius + realm_modifier,
    delta: vortex_delta,
    cast_at: now,
    maintain_max: 5s by realm,
    caster: caster,
  }

  vortex_intercept_tick (每 server tick):
    for each QiProjectile / QiNeedle 在 VortexField 范围内:
      抽干量 = qi_payload × (delta / 0.8)
      qi_payload -= 抽干量
      emit ProjectileQiDrainedEvent

  vortex_maintain_tick (每秒):
    caster.cultivation.qi_current -= 5-10/s by realm
    if elapsed > maintain_max:
      触发反噬 → caster.meridian[hand].flow_capacity = 0 永久 SEVERED
      despawn VortexField + remove VortexCasting status
      emit VortexBackfireEvent

  玩家 cast vortex（toggle off）→
    despawn VortexField
    remove VortexCasting status
```

### A.2 跨 plan 拦截范围

| 攻击源 | 涡流可拦截 | 备注 |
|---|---|---|
| anqi 暗器（`QiProjectile`）| ✅ | qi_payload 抽干，载体物理保留（"朽木兽骨"），命中 contam 大减 |
| anqi 凝针（`QiNeedle`）| ✅ | qi_payload (1) 抽干 → 0 |
| dugu 凝针 + 灌毒蛊 | ✅ | qi_payload 被抽 → DuguPoisonState 不挂载（worldview "毒蛊→涡流极亏"）|
| 爆脉 / 拳 / 剑 / 灌毒蛊近战 | ❌ | 不入涡流场（贴脸距离穿过涡流瞬间），worldview "爆脉→涡流无可吸" |
| 法术（plan-magic 未立）| ⬜ | hooks 留 |
| zhenfa 诡雷（地面方块）| ❌ | 静态方块不入空中涡流；vN+1 决议 |
| 普通投射物（无 qi_payload）| ❌ | 凡铁箭等无真元的物理投射物，涡流无可吸（worldview "天地法则只对真元起作用"）|

### A.3 信息分层显示（Q87 reframe）

worldview §六.3 "灵气浓度需要修士神识感知" 严格遵守。client 渲染：

```rust
fn ambient_qi_perception(
    previous_qi: f32,            // 玩家上次进入此 zone 时记录
    current_qi: f32,             // 当前 zone.spirit_qi
    has_inspect_skill: bool,     // 神识 / inspect skill 是否解锁
) -> Option<String> {
    // 神识者直读数值
    if has_inspect_skill {
        return Some(format!("灵气浓度: {:.2}", current_qi));
    }

    // 普通玩家：仅相对差值变化（zone 切换时触发 ambient text）
    let ratio = current_qi / previous_qi.max(0.01);
    match ratio {
        r if r > 1.5 => Some("此地灵气骤然浓郁，呼吸间元气盈满"),
        r if r > 1.2 => Some("似觉灵气稍浓"),
        r if (0.8..=1.2).contains(&r) => None,             // 无明显变化，不显示
        r if r < 0.8 && r >= 0.5 => Some("灵气稀薄，引气如吸沙"),
        r if r < 0.5 => Some("灵气几近断绝，此地有不祥预感"),
        _ => None,
    }.map(String::from)
}
```

**关键设计**：
- 玩家**不感知绝对数值**——只感知"刚才那里灵气浓 / 这里稀薄"的相对变化
- ratio 0.8-1.2 区间不触发文本（避免抖动 / 玩家被刷屏）
- 神识者（plan-perception "inspect skill"）才能读绝对数值
- 这接入 plan-perception-v1.1 的 SenseEntryV1 系统，新文本类型

**触发时机**：
- 玩家进入新 zone（zone change event）
- 玩家激活涡流尝试 cast（cast 前自动 inspect 当前 env_qi）

### A.4 v1 实施阶梯

```
P0  持涡闭环（最 close-loop）
       VortexField component { center, radius, delta, cast_at, maintain_max, caster }
       VortexCastIntent + cast_vortex (toggle on/off)
       立即施加 VortexCasting status (减速 80% / block attack/defense/sprint)
       凝脉 Δ=0.25 + 基础半径 1.5
       vortex_intercept_tick (拦截 QiProjectile / QiNeedle)
       vortex_maintain_tick (耗 qi + maintain_max 检查)
       反噬：维持过久 → meridian[hand].flow_capacity = 0
       Zone.spirit_qi 读取
       信息分层（神识 inspection vs 相对体感）
       修复 DerivedAttrs.vortex_fake_skin_layers 命名错位（拆为 tuike_layers）
       ↓
P1  境界分级 + 多因子半径 + max 叠加 + 抗灵压丹 + agent narration
       Δ 公式按境界（引气 0.10 / 凝脉 0.25 / 固元 0.45 / 通灵 0.65）
       半径多因子公式（基础 + 境界系数）
       max 叠加（同点位置多 woliu 师 → 取最大 Δ；不放大不减弱）
       抗灵压丹接入（library ecology-0002 空兽痕 → 减反噬概率）
       agent narration: VortexBackfireEvent + ProjectileQiDrainedEvent
       ↓ 饱和 testing
P2  v1 收口
       数值平衡（fight room：anqi 重狙 vs woliu / dugu 飞针 vs woliu）
       LifeRecord "X 在 N 战中开 vortex 抽干 Y 的暗器"
```

### A.5 v1 已知偏离正典（vN+1 必须修复）

- [ ] **短时瞬涡变体（弹反式）**（worldview §五.防御.3 + skeleton §2 掌心瞬涡）—— v1 仅做持涡 toggle，瞬涡留 vN+1
- [ ] **化虚 0.9 抽干 + 远程引导 + 太极反击**（worldview line 442 + user 提议）—— v1 不实装，记录在头部 vN+2 扩展
- [ ] **涡流叠加 + 旋转方向**（user 提议高复杂度团战机制）—— v1 P0 不叠加，P1 max 叠加，vN+1 旋转方向
- [ ] **持续防御涡（永久 / 全场涡）**（worldview "化虚理论 永久 全场涡"）—— v1 不实装
- [ ] **顿悟"识规则"加成**（skeleton §4 通灵 +0.05）—— 接 plan-cultivation 顿悟系统，v1 不实装
- [ ] **染色加成（缜密色 / 阴诡色）+ 真元流速/池效率特性**（worldview §五:467 已确认 woliu 物资派性质）—— v1 不实装
- [ ] **plan-tsy-dimension 坍缩渊负灵域真实演练**（涡流在负灵域必反噬）—— v1 仅理论数值
- [ ] **DerivedAttrs.vortex_fake_skin_layers 命名错位**（实际是替尸字段）—— P0 修正：拆为 `tuike_layers` + 真正的 `vortex_active`

### A.6 v1 关键开放问题

**已闭合**（Q84-Q89 + 问 1/2/3，9 个决策）：
- Q84 reframe → C 持涡 toggle + 减速 80%
- Q85 → B 按 Δ 比例抽（v1 化虚 0.9 + 远程引导 + 太极反击留 vN+1+）
- Q86 → C 双红线（v1 仅"维持过久"实际触发，超 Δ 上限留 vN+1 超限功法）
- Q87 reframe → 信息分层（神识 inspection 数字 / 普通玩家相对体感）
- Q88 → 复杂度拆分：v1 P0 不叠加 / v1 P1 max 叠加 / vN+1 旋转方向 / vN+2 流体算法
- Q89 → 多因子半径公式（v1 = 基础 + 境界）
- 问 1 → v1 P0 不叠加（团战留 P1+）
- 问 2 → A 立即减速（worldview 算计型 = 不能反悔）
- 问 3 → P0 实装信息分层（不留 P1）

**仍 open**（v1 实施时拍板）：
- [ ] **Q90. maintain_max 时长**：建议 **5s 凝脉默认**（worldview "瞬时" + 时间压力）；P1 境界分级（引气 2s / 凝脉 5s / 固元 8s / 通灵 12s）
- [ ] **Q91. 在已是负灵域 cast 失败的反应**：建议**立即反噬**（试图在风口再造风口必反吸自身；worldview line 444）
- [ ] **Q92. 持涡时 hand-slot 占用**：建议**主手或副手任一可（类比 anqi Q32）**，未持物 = "掌心涡流"，持盾 = "盾面涡流"（视觉差异，数值同）
- [ ] **Q93. 信息分层 ratio 阈值**：v1 起手 0.8/1.2/1.5/0.5，P0 实装时单测调
- [ ] **Q94. 修复 vortex_fake_skin_layers 命名**：DerivedAttrs 拆为 `tuike_layers: u8` + `vortex_active: bool`，需要同步改 plan-HUD 引用

---



## §0 设计轴心

- [ ] **涡流幅度是相对值**——`local = environment - Δ`，不是绝对 -0.8
- [ ] -0.8 是化虚级理论极限，普通修士能做到"低于环境"就不错
- [ ] **环境依赖**：馈赠区好用，死域勉强，负灵域几乎无法用
- [ ] 末法约束：维持过久 → 心镜被自吸 → 永久残疾
- [ ] **关键设计变更**：原 worldview 写"约 -0.8" = 著者推演的理想极限，实战需按境界分梯度

## §1 第一性原理（烬灰子四论挂点）

- **噬论·主动开风口**：修士在掌心/盾面**主动制造一个微型负灵域**——本质是让天地之炉的吞噬力局部加剧。这是"风口"理论的逆向应用——平常人怕风口，涡流流主动用风口当武器
- **缚论·镜身共振**：主动调动自身镜身去"匹配"低境环境——让局部灵气浓度低于周围。代价是心镜不断和负灵域共振
- **影论·镜身被自吸**：维持涡流时间过长 → 心镜本身被吸入风口 → 永久残疾（书里"涡流反噬瞬间抽自身经脉，致永久残疾"原话）
- **音论·识规则透解者**：要成功制造负灵域需对天地法则有深透理解——故"算计型、对世界规则有透解者"才能修

## §2 形态分级

| 形态 | 维持时长 | Δ 幅度 | 真元成本 |
|---|---|---|---|
| **掌心瞬涡**（弹反式）| 0.5–1s | 同境界上限 | 5–10 |
| **盾面持涡** | 1–5s | 境界上限 -0.05 | 持续 5 / s |
| **持续防御涡流** | 持续 | 境界上限 -0.10 | 持续 10 / s |
| **化虚理论·全场涡** | 永久 | 0.80（书里上限）| 几乎不存在 |

## §3 数值幅度梯度（按境界）—— 关键校准

| 境界 | Δ 上限（相对环境）| 持续时间 | 反噬阈值 |
|---|---|---|---|
| **引气** | 0.10–0.15 | 0.5–1s（弹反窗口）| Δ > 0.15 即反噬 |
| **凝脉** | 0.20–0.30 | 1–2s | Δ > 0.30 |
| **固元** | 0.40–0.50 | 3–5s | Δ > 0.50 |
| **通灵 + 顿悟** | 0.60–0.70 | 持续防御 | Δ > 0.70 |
| **化虚理论** | 0.80（书里上限）| 全场 | 几乎无反噬 |

**核心公式**：
```
local_qi = environment_qi - vortex_delta
（vortex_delta 受境界 + 染色 + 顿悟决定）
```

**环境依赖示例**：
- 馈赠区（env=0.6）+ 凝脉期（Δ=0.25）→ 局部 0.35（吸力可观，可吸普通暗器）
- 死域（env=0）+ 凝脉期 → 局部 -0.25（吸力极弱）
- 负灵域（env=-0.5）+ 任何境界 → 已经是风口，再造涡流极难，**正是涡流流去坍缩渊会死的物理原因**

## §3.1 涡流·v1 规格（P0 阶段）

> worldview §五.防御.3 + §五:472 "算计型防御 / 持久博弈型" 锚定。v1 收敛到**持涡 toggle**（凝脉 Δ=0.25）+ **拦截 QiProjectile / QiNeedle**（按 Δ 比例抽 qi_payload）+ **维持过久反噬**（手经脉永久 SEVERED）+ **信息分层**（神识/相对体感）+ **修复命名错位**。所有触发走 hotbar `UseQuickSlot` 路径（plan-hotbar-modify-v1 已落地）。

### 3.1.A Hotbar 接入（无专属技能键 — 1-9 战斗·修炼栏）

按 user 设计原则："**所有技能走 hotbar，不另建专属按键**"。woliu 涡流通过 plan-hotbar-modify-v1 已落地路径触发：

**关键键位区分**（plan-hotbar-modify-v1 line 31/42 已正典）：
- **1-9 数字键**：下层"战斗·修炼"栏 — 绑功法 / 技能（spell/skill cast，含 woliu/dugu/anqi/zhenmai/tuike/zhenfa 主动技能）
- **F1-F9**：上层"快捷使用"栏 — 绑丹药 / 绷带 / consumable（含 cast time）
- **woliu 涡流绑 1-9，不是 F1-F9**

**触发流程**：

```
玩家学习 Technique::WoliuVortex（vN+1 接 plan-cultivation 修炼系统；v1 简化为玩家可手动添加）→
  cultivation.techniques.push(Technique::WoliuVortex)

玩家 InspectScreen "战斗·修炼" tab → 选 WoliuVortex → 绑到 1-9 任一槽 →
  QuickSlotBindings.slots[slot_index] = TechniqueId::WoliuVortex.to_instance_id()

战斗按 1-9 数字键 →
  client mixin 截获 keypress → 发 UseQuickSlot { v: 1, slot: <0-8> } →
  server::handle_use_quick_slot 查 slot binding →
  if technique == WoliuVortex:
    let toggle = if vortex_active(caster) { Off } else { On };
    emit VortexCastIntent { caster, toggle };
```

**v1 不新建专属 packet**。所有玩家技能 input 走 `UseQuickSlot { slot }`，server 内部分发到对应技能 system。

**与原版 1-9 hotbar 切换的兼容**（plan-hotbar-modify-v1 §238）：当 1-9 槽位绑物品（不是 technique），按 1-9 退化回现网 server-driven hotbar 切换路径——client mixin 不 cancel 该 keypress。绑 technique 时才 cancel + 发 UseQuickSlot。

### 3.1.B VortexField component + cast_vortex 系统

```rust
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct VortexField {
    pub center: Vec3,                  // caster.hand_pos at cast time
    pub radius: f32,                   // base 1.5 + realm modifier
    pub delta: f32,                    // 0.25 凝脉默认 / P1 境界分级
    pub cast_at: GameTime,             // 用于 maintain_max 检查
    pub maintain_max_secs: f32,        // 5.0 凝脉默认
    pub caster: Entity,
    pub env_qi_at_cast: f32,           // 记录 cast 瞬间 env_qi（避免每 tick 重读）
}

#[derive(Event)]
pub struct VortexCastIntent {
    pub caster: Entity,
    pub toggle: VortexToggle,
    pub source: IntentSource,
}

pub enum VortexToggle {
    On,
    Off,
}
```

**`cast_vortex` 系统**（消费 VortexCastIntent）：

```rust
pub fn cast_vortex(
    mut intents: EventReader<VortexCastIntent>,
    mut commands: Commands,
    mut casters: Query<(&mut Cultivation, &mut StatusEffects, Option<&mut VortexField>, &Position)>,
    zones: Res<ZoneRegistry>,
    mut backfires: EventWriter<VortexBackfireEvent>,
    mut status_intents: EventWriter<ApplyStatusEffectIntent>,
) {
    for intent in intents.read() {
        let Ok((cult, _, vortex_opt, pos)) = casters.get_mut(intent.caster) else { continue };

        match intent.toggle {
            VortexToggle::Off => {
                // 主动关闭
                if vortex_opt.is_some() {
                    commands.entity(intent.caster).remove::<VortexField>();
                    // 移除 VortexCasting status（dispel）
                    status_intents.send(ApplyStatusEffectIntent {
                        target: intent.caster,
                        kind: StatusEffectKind::VortexCasting,
                        duration_ms: 0, // dispel marker
                        source: intent.source,
                    });
                }
            }
            VortexToggle::On => {
                if vortex_opt.is_some() { continue; } // 已有涡流，忽略

                let env_qi = zones.qi_at(pos.0);
                let realm_delta = vortex_delta_for_realm(cult.realm);

                // Q91 拍：在已是负灵域 cast 失败 = 立即反噬
                if env_qi < realm_delta {
                    backfires.send(VortexBackfireEvent {
                        caster: intent.caster,
                        cause: BackfireCause::EnvQiTooLow,
                        meridian_severed: pick_hand_meridian(cult.realm),
                    });
                    continue;
                }

                // 立即施加 VortexCasting status (问 2: A 立即减速 80%)
                status_intents.send(ApplyStatusEffectIntent {
                    target: intent.caster,
                    kind: StatusEffectKind::VortexCasting,
                    duration_ms: u32::MAX, // 持续到主动 dispel
                    source: intent.source,
                });

                // spawn VortexField
                commands.entity(intent.caster).insert(VortexField {
                    center: pos.0,
                    radius: vortex_radius_for_realm(cult.realm),
                    delta: realm_delta,
                    cast_at: time.now,
                    maintain_max_secs: vortex_maintain_max_for_realm(cult.realm),
                    caster: intent.caster,
                    env_qi_at_cast: env_qi,
                });
            }
        }
    }
}

fn vortex_delta_for_realm(realm: Realm) -> f32 {
    match realm {
        Realm::XingLing => 0.0,        // 醒灵不可学
        Realm::YinQi => 0.10,          // P1 实装
        Realm::NingMai => 0.25,        // P0 默认
        Realm::GuYuan => 0.45,         // P1 实装
        Realm::TongLing => 0.65,       // P1 实装
        Realm::HuaXu => 0.80,          // 化虚已断绝（worldview）
    }
}

fn vortex_radius_for_realm(realm: Realm) -> f32 {
    let base = 1.5;
    let realm_mod = match realm {
        Realm::YinQi => -0.5,    // 1.0
        Realm::NingMai => 0.0,   // 1.5 (P0 default)
        Realm::GuYuan => 0.5,    // 2.0
        Realm::TongLing => 1.5,  // 3.0
        _ => 0.0,
    };
    base + realm_mod
    // vN+1: + skill modifier + flux modifier
}

fn vortex_maintain_max_for_realm(realm: Realm) -> f32 {
    match realm {
        Realm::YinQi => 2.0,
        Realm::NingMai => 5.0,    // P0 default
        Realm::GuYuan => 8.0,
        Realm::TongLing => 12.0,
        _ => 0.0,
    }
}
```

### 3.1.C VortexCasting StatusEffect（减速 80% + block）

新增 `StatusEffectKind::VortexCasting` variant：

```rust
pub enum StatusEffectKind {
    // ... existing variants ...
    /// 涡流流持涡状态 - worldview §五.防御.3 锚定 "算计型防御需主动维持"
    VortexCasting,
}
```

**`StatusEffectRegistry` 行为**：
- `move_speed_mul: 0.2`（**减速 80%**，问 2: A 立即生效）
- `block_attack: true`（持涡时不可 attack）
- `block_defense: true`（持涡时不可 jiemai parry）
- `block_sprint: true`
- `dispellable: true`（玩家可主动 dispel = toggle off）
- `duration_ms: u32::MAX`（实际由 `vortex_maintain_tick` 控制移除）

### 3.1.D 拦截系统：`vortex_intercept_tick`（Q85: B 比例抽）

```rust
/// 每 server tick：检查所有 in-flight projectile 是否在 VortexField 范围内
pub fn vortex_intercept_tick(
    fields: Query<&VortexField>,
    mut projectiles: Query<(&Position, &mut QiProjectile)>,
    mut needles: Query<(&Position, &mut QiNeedle)>,
    mut events: EventWriter<ProjectileQiDrainedEvent>,
) {
    for field in fields.iter() {
        // anqi QiProjectile
        for (pos, mut proj) in projectiles.iter_mut() {
            if pos.0.distance(field.center) <= field.radius {
                let drain_ratio = (field.delta / 0.8).clamp(0.0, 1.0);
                let drained = proj.qi_payload as f32 * drain_ratio;
                proj.qi_payload -= drained;
                events.send(ProjectileQiDrainedEvent {
                    field_caster: field.caster,
                    projectile: proj.shooter,
                    drained_amount: drained,
                });
            }
        }

        // dugu QiNeedle
        for (pos, mut needle) in needles.iter_mut() {
            if pos.0.distance(field.center) <= field.radius {
                let drain_ratio = (field.delta / 0.8).clamp(0.0, 1.0);
                let drained = needle.qi_payload * drain_ratio;
                needle.qi_payload -= drained;
                // QiNeedle.qi_payload 接近 0 → 命中后 DuguPoisonState 不挂载
                events.send(ProjectileQiDrainedEvent { ... });
            }
        }
    }
}
```

**抽干公式（Q85: B）**：
- `drain_ratio = delta / 0.8`
- 凝脉 Δ=0.25 → 抽 31% (drain_ratio 0.31)
- 通灵 Δ=0.65 → 抽 81% (drain_ratio 0.81)
- 化虚 Δ=0.80 → 抽 100%（v1 化虚已断绝；vN+1+ 化虚扩展上限到 0.9 = 抽 112% 即"反推回去"= 太极反击）

**载体物理保留**：projectile 仍然飞行，命中目标走标准 attack 事务。但 `qi_payload` 大减 → contam 大减 / DuguPoisonState 不挂载 / wound 不变（因为 wound 不依赖 qi_payload，依赖载体物理）。这就是 worldview "致命骨刺变成普通朽木棍" 的物理化。

### 3.1.E 维持反噬：`vortex_maintain_tick`（Q86: C）

```rust
/// 每秒：扣 qi + 检查 maintain_max + 反噬
pub fn vortex_maintain_tick(
    time: Res<GameTime>,
    mut commands: Commands,
    mut casters: Query<(Entity, &mut Cultivation, &mut MeridianSystem, &VortexField)>,
    mut backfires: EventWriter<VortexBackfireEvent>,
    mut status_intents: EventWriter<ApplyStatusEffectIntent>,
) {
    for (entity, mut cult, mut ms, field) in casters.iter_mut() {
        let elapsed = (time.now - field.cast_at).as_secs_f32();

        // 1. 扣 qi（每秒 5-10 by realm）
        let qi_cost = vortex_qi_cost_per_sec(cult.realm);
        cult.qi_current -= qi_cost;

        // 2. qi 不够 → 主动关闭（不算反噬）
        if cult.qi_current <= 0.0 {
            cult.qi_current = 0.0;
            commands.entity(entity).remove::<VortexField>();
            status_intents.send(/* dispel VortexCasting */);
            continue;
        }

        // 3. 维持过久 → 反噬（Q86: C 双红线之"维持过久"，v1 实际触发条件）
        if elapsed > field.maintain_max_secs {
            let hand_meridian = pick_hand_meridian(cult.realm);
            if let Some(m) = ms.get_mut(hand_meridian) {
                m.flow_capacity = 0.0;             // 永久 SEVERED（不可恢复）
                m.opened = false;
            }
            cult.recompute_qi_max(&ms);
            commands.entity(entity).remove::<VortexField>();
            status_intents.send(/* dispel VortexCasting */);
            backfires.send(VortexBackfireEvent {
                caster: entity,
                cause: BackfireCause::ExceedMaintainMax,
                meridian_severed: hand_meridian,
            });
        }

        // 4. 超 Δ 上限 v1 不可能（无超限功法），但留 hook（Q86: C 双红线之"超 Δ"）
        // P1 + 超限功法 vN+1 接入时这里加另一分支
    }
}

fn vortex_qi_cost_per_sec(realm: Realm) -> f64 {
    match realm {
        Realm::YinQi => 5.0,
        Realm::NingMai => 6.0,        // P0 default
        Realm::GuYuan => 8.0,
        Realm::TongLing => 10.0,
        _ => 0.0,
    }
}
```

**反噬经脉选择**：`pick_hand_meridian(realm)` 按修炼 path 选 — v1 简化为"手太阴肺经"（worldview §六.1 "手三阴"出口；用户选择"涡流 = 算计型"对应"气"维度）。vN+1 按修炼 path 动态选。

### 3.1.F 信息分层显示（Q87 reframe，问 3 P0 实装）

接 `plan-perception-v1.1` 已落地的 SenseEntryV1 系统，新 ambient text 类型：

```rust
// server 侧：玩家进入新 zone / cast vortex 时触发
pub fn ambient_qi_perception(
    previous_qi: f32,
    current_qi: f32,
    has_inspect_skill: bool,
) -> Option<String> {
    if has_inspect_skill {
        return Some(format!("灵气浓度: {:.2}", current_qi));
    }

    let ratio = current_qi / previous_qi.max(0.01);
    match ratio {
        r if r > 1.5 => Some("此地灵气骤然浓郁，呼吸间元气盈满"),
        r if r > 1.2 => Some("似觉灵气稍浓"),
        r if (0.8..=1.2).contains(&r) => None,
        r if r < 0.8 && r >= 0.5 => Some("灵气稀薄，引气如吸沙"),
        r if r < 0.5 => Some("灵气几近断绝，此地有不祥预感"),
        _ => None,
    }.map(String::from)
}
```

**触发时机**（v1 P0 实装）：
- 玩家进入新 zone（zone change event）→ 比较 `previous_zone.spirit_qi` vs `current_zone.spirit_qi`
- 玩家激活涡流（VortexCastIntent on）→ 在 cast 前 inspect 当前 env_qi
- 已有 inspect skill 的玩家进入新 zone 也显示数字（一律走神识路径）

**输出**：通过现有 `SenseEntryV1` 推 client，client perception 模块渲染成屏幕边缘文字（已有 UI 路径）。

### 3.1.G 反噬事件 + 命名错位修正

**新增 `VortexBackfireEvent`**：

```rust
#[derive(Event, Serialize)]
pub struct VortexBackfireEvent {
    pub caster: Entity,
    pub cause: BackfireCause,
    pub meridian_severed: MeridianId,
}

pub enum BackfireCause {
    EnvQiTooLow,         // cast 时 env_qi < delta
    ExceedMaintainMax,   // 维持超时
    ExceedDeltaCap,      // 超 Δ 上限（vN+1 超限功法时触发）
}
```

**修正 `DerivedAttrs.vortex_fake_skin_layers` 命名错位**：

当前 `server/src/combat/components.rs:282-285`：
```rust
pub vortex_fake_skin_layers: u8,  // 注释写"替尸伪皮剩余层数"——名字与功能错位
pub vortex_ready: bool,           // 注释写"绝灵涡流可触发/激活态"——这个是对的
```

**P0 修正**：
```rust
pub tuike_layers: u8,             // 替尸伪皮剩余层数（拆给 plan-tuike-v1）
pub vortex_active: bool,          // woliu 涡流当前激活态（取代 vortex_ready 命名）
```

需要同步改：
- `plan-HUD-v1` 引用（条件渲染门禁）
- 所有 `Default::default` 初始化处
- client `combat/store.ts` 镜像字段

---

## §3.2 涡流·v1 规格（P1 阶段）

延续 §3.1 设计，扩展境界分级 + 多因子半径 + max 叠加 + 抗灵压丹 + agent narration。

### 3.2.A 境界分级 Δ + 半径 + maintain_max + qi cost

| 境界 | Δ 上限 | 基础半径 | maintain_max | qi cost/s | 反噬经脉（建议）|
|---|---|---|---|---|---|
| 醒灵 | 不可学 | — | — | — | — |
| **引气** | 0.10 | 1.0 格 | 2s | 5 | 手太阴肺经 |
| **凝脉**（P0 default）| 0.25 | 1.5 格 | 5s | 6 | 手太阴肺经 |
| **固元** | 0.45 | 2.0 格 | 8s | 8 | 手太阴肺经 |
| **通灵** | 0.65 | 3.0 格 | 12s | 10 | 手太阴肺经 |
| **化虚（断绝）** | 0.80（理论）| 全场 | 永久 | — | — |

### 3.2.B max 叠加（Q88 v1 P1）

```rust
/// 多人涡流叠加（v1 P1）：取最大 Δ，半径合并取联合区域
pub fn vortex_aggregate_at(pos: Vec3, fields: &Query<&VortexField>) -> Option<f32> {
    fields.iter()
        .filter(|f| pos.distance(f.center) <= f.radius)
        .map(|f| f.delta)
        .reduce(f32::max)
    // 不累加（v1 P1 简化）；vN+1 旋转方向 + 顺逆相消
}
```

**应用**：`vortex_intercept_tick` 改为按"每个 projectile 所在位置的 aggregate delta"计算抽干率，不是单 field 单 projectile 配对。

### 3.2.C 抗灵压丹接入

**library `ecology-0002` 空兽痕 → 抗灵压丹**：减少反噬概率。

```rust
/// 服用抗灵压丹后施加 status，30 min 期间反噬触发概率 -50%
pub enum StatusEffectKind {
    // ...
    AntiSpiritPressurePill,
}

/// vortex_maintain_tick 中触发反噬时检查
fn check_backfire_resistance(status: &StatusEffects, rng: &mut Rng) -> bool {
    if has_active_status(status, StatusEffectKind::AntiSpiritPressurePill) {
        return rng.gen::<f32>() < 0.50;  // 50% 概率"扛过去"，不反噬
    }
    false
}
```

接 plan-alchemy-v1 配方：`AntiSpiritPressurePillRecipe`（空兽痕 + ... → 抗灵压丹）。

### 3.2.D agent narration

`VortexBackfireEvent` / `ProjectileQiDrainedEvent` → agent 触发 narration：

| Event | narration 模板 |
|---|---|
| `ProjectileQiDrainedEvent` (drain > 50%) | "X 的暗器入了 Y 的涡流，真元被天地抽得干净，载体落地化朽" |
| `VortexBackfireEvent (ExceedMaintainMax)` | "Y 的涡流维持过久，反噬之意瞬息倒卷——一根经脉就此永封" |
| `VortexBackfireEvent (EnvQiTooLow)` | "Y 在贫瘠之地强造涡流，差值不足，反吸自身" |

---

## §4 材料 / 资源链

涡流流不依赖材料制作，但有以下辅助：

| 辅助 | library 来源 | 用途 |
|---|---|---|
| **空兽痕** → 抗灵压丹 | ecology-0002（稀见五味）| 减少负灵域反噬 |
| 顿悟"识规则" | worldview §六.3 | 通灵期 Δ 上限 +0.05 |

**长期成本**：涡流流不烧材料但烧时间——需要长期在不同灵气浓度环境训练"差值感"

## §5 触发 / 流程

```
准备：选 hand-slot 或 shield-slot
触发：玩家按 cast → 读取当前 environment_qi →
  根据境界/染色/顿悟决定 vortex_delta →
  local_qi = environment - delta
  若 local_qi < (env_qi 当前) → 形成涡流场（半径 1-3 格）

战斗判定：远程投射物（暗器流真元载体）飞入涡流半径 →
  载体真元被抽干（concrete imprint × (delta / 0.8)）→
  载体落地变"朽木"

维持：每秒扣真元 5-10（按境界）+ 检查反噬阈值
反噬：Δ 超阈值 → 涡流"反吸自身" → 掌心经脉永久 SEVERED
```

## §6 反噬 / 失败代价

- [ ] **超 Δ 上限** → 涡流反吸自身 → 掌心经脉永久 SEVERED（worldview 原话"永久性残疾"）
- [ ] **维持过久**（超持续时间）→ 经脉污染 +0.5
- [ ] **在已是负灵域的地方使用** → Δ 不够 → 反吸自身（连"差值"都做不出来）
- [ ] 真元持续耗损（高阶 10 / s）→ 长持续战不可行
- [ ] 装备重量影响（盾面持涡需自由手）

## §7 克制关系

- **克**：暗器流（高爆发飞入涡流被天地法则没收）；远程流派整体
- **被克**：**毒蛊流**（一点脏真元开黑洞极亏，涡流自身反噬代价更大）；爆脉流（不外放真元 → 涡流无可吸）
- **染色关联**：世界观 §六.2 明确防御三流是战术选择，不绑定染色。阵法师（缜密色）对灵气差值感知敏锐，天然顺手但非专属；沉重色（体修）求损者不会算计差值，反噬频发

## §8 数据契约

### v1 P0 落地清单（按 §3.1 规格）

| 模块 | 文件路径 | 核心内容 |
|---|---|---|
| VortexField component | `server/src/combat/woliu.rs` (新文件) | `VortexField { center, radius, delta, cast_at, maintain_max_secs, caster, env_qi_at_cast }` |
| VortexCastIntent event | `server/src/combat/events.rs` | 新增 `VortexCastIntent { caster, toggle: VortexToggle, source }` |
| cast_vortex 系统 | `server/src/combat/woliu.rs` | 消费 VortexCastIntent → 校验 env_qi → spawn / despawn VortexField + 施加/移除 status |
| vortex_intercept_tick | `server/src/combat/woliu.rs` | 拦截 QiProjectile / QiNeedle qi_payload 抽干（`drain_ratio = delta / 0.8`） |
| vortex_maintain_tick | `server/src/combat/woliu.rs` | 每秒扣 qi + 维持过久反噬 + 经脉永久 SEVERED |
| 数值函数 | `server/src/combat/woliu.rs` | `vortex_delta_for_realm` / `vortex_radius_for_realm` / `vortex_maintain_max_for_realm` / `vortex_qi_cost_per_sec` |
| VortexCasting status | `server/src/combat/events.rs::StatusEffectKind` + `combat/status.rs::EFFECT_REGISTRY` | 新 variant + speed×0.2 + block_attack/defense/sprint |
| VortexBackfireEvent | `server/src/combat/events.rs` | 新事件 + BackfireCause enum |
| 命名错位修复 | `server/src/combat/components.rs` | `vortex_fake_skin_layers` → `tuike_layers` / `vortex_ready` → `vortex_active`（双拆分，对应 plan-tuike + plan-woliu） |
| **Hotbar 接入** | `server/src/network/client_request_handler.rs::handle_use_quick_slot` | 扩展 binding 解析：if `Technique::WoliuVortex` → emit VortexCastIntent；**不新建 packet** |
| Technique enum | `server/src/cultivation/technique.rs` (新或扩展) | 新增 `Technique::WoliuVortex` variant；techniques_snapshot 走已有 schema |
| 信息分层 | `server/src/perception/ambient_qi.rs` (新) | `ambient_qi_perception(prev, curr, has_inspect_skill)` 函数 + zone_change 触发 + cast 触发 |
| Schema | `agent/packages/schema/src/woliu.ts` | `VortexFieldStateV1` / `VortexBackfireEventV1`；扩展 `SenseEntryV1` 支持 ambient text |
| Outbound | `bong:woliu/vortex_state` (HUD payload) — 当前 Δ / maintain_remaining / 拦截统计 |
| 单测 | `server/src/combat/woliu_tests.rs` | cast 校验 / maintain 反噬 / env_qi 不足反噬 / 拦截抽干公式 / status 减速 / 醒灵不可学 / 命名错位修复 |

### v1 P1 落地清单（按 §3.2 规格）

| 模块 | 文件路径 | 核心内容 |
|---|---|---|
| 境界分级 | `combat/woliu.rs` | 上述数值函数全境界 4 档落地 |
| 多因子半径 | `combat/woliu.rs` | `vortex_radius_for_realm` 加 base + realm modifier |
| max 叠加 | `combat/woliu.rs` | `vortex_aggregate_at(pos, fields)` 多 field 取 max delta |
| 抗灵压丹 | `combat/status.rs` + `crafting/alchemy.rs` (plan-alchemy 扩展) | `StatusEffectKind::AntiSpiritPressurePill` + `AntiSpiritPressurePillRecipe`（空兽痕主料）|
| Agent narration | `agent/packages/tiandao/src/woliu-narration.ts` | `ProjectileQiDrainedEvent` / `VortexBackfireEvent` 触发 |

### v1 P2 落地清单

| 模块 | 文件路径 | 核心内容 |
|---|---|---|
| 数值平衡 | `server/src/combat/woliu_balance.rs` | fight room: anqi 重狙 vs woliu / dugu 飞针 vs woliu |
| LifeRecord | `server/src/lore/life_record.rs` | "X 在 N 战中开 vortex 抽干 Y 的暗器"事件 |

### 跨 plan hotbar 同步修正备注（重要）

**user 已确认设计原则**："所有技能走 hotbar，不另建专属按键"。

前 5 个流派 plan（zhenfa / anqi / dugu / zhenmai / tuike）的"专属 packet" 都应该改走 hotbar `UseQuickSlot` 路径。具体涉及：

| Plan | 原专属 packet（待修正） | 改走 hotbar |
|---|---|---|
| zhenfa-v1 | `bong:combat/place_trap` / `bong:combat/trigger_trap` | `Technique::ZhenfaPlaceTrap` / `Technique::ZhenfaTrigger` 绑 1-9 |
| anqi-v1 | `bong:combat/charge_carrier` / `bong:combat/throw_carrier` | `Technique::AnqiChargeCarrier` / `Technique::AnqiThrowCarrier` 绑 1-9 |
| dugu-v1 | `bong:combat/shoot_needle` / `bong:combat/infuse_dugu_poison` / `bong:cultivation/self_antidote` | 三个 Technique 分别绑 1-9（self_antidote 可考虑 F1-F9，因为是消耗性服药动作）|
| zhenmai-v1 | `bong:combat/defense_stance`（已实装） | 保留现状 — 已是直接 packet，但建议 P1 改为 Technique::ZhenmaiParry 绑 1-9 |
| tuike-v1 | `bong:armor/equip_false_skin` | 装备类操作可保留（不是技能 cast） |

**建议**：在每个 plan v1 实施时**统一**走 hotbar 路径，不新建专属 packet。本 plan §11 进度日志记录此原则。

## §9 实施节点

详见 §A.4 v1 实施阶梯。三阶段：

- [ ] **P0** 持涡 toggle + 凝脉 Δ=0.25 + 拦截基础 + 反噬 + 信息分层 + 命名错位修复 —— 见 §3.1
- [ ] **P1** 境界分级 + 多因子半径 + max 叠加 + 抗灵压丹 + agent narration —— 见 §3.2
- [ ] **P2** v1 收口（数值平衡 + LifeRecord）

## §10 开放问题

### 已闭合（2026-05-03 拍板，9 个决策 + 3 个细化）

- [x] **Q84 reframe** → C 持涡 toggle（开关，非按住）+ 自身减速 80%
- [x] **Q85** → B 按 Δ 比例抽（化虚 0.9 抽干 + 远程引导 + 太极反击留 vN+1+）
- [x] **Q86** → C 双红线（v1 仅"维持过久"实际触发；超 Δ 上限留 vN+1 超限功法 hook）
- [x] **Q87 reframe** → 信息分层（神识 inspection 数字 / 普通玩家相对体感 ratio 比较）
- [x] **Q88** → 复杂度拆分（v1 P0 不叠加 / v1 P1 max 叠加 / vN+1 旋转方向 / vN+2 流体算法）
- [x] **Q89** → 多因子半径公式（v1 = 基础 + 境界）
- [x] **问 1** → v1 P0 不叠加（团战留 P1+）
- [x] **问 2** → A 立即减速（worldview 算计型 = 不能反悔）
- [x] **问 3** → P0 实装信息分层
- [x] **Hotbar 接入** → 1-9 数字键（战斗·修炼栏），不是 F1-F9（consumable 栏）；走已落地 UseQuickSlot 路径

### 仍 open（v1 实施时拍板）

- [ ] **Q90. maintain_max 时长**：建议 5s 凝脉默认；P1 境界分级（引气 2s / 凝脉 5s / 固元 8s / 通灵 12s）
- [ ] **Q91. 在已是负灵域 cast 失败的反应**：建议**立即反噬**（worldview line 444 锚定）
- [ ] **Q92. 持涡时 hand-slot 占用**：建议**主手或副手任一可（类比 anqi Q32）**，未持物 = "掌心涡流"，持盾 = "盾面涡流"（视觉差异，数值同）
- [ ] **Q93. 信息分层 ratio 阈值**：v1 起手 0.8/1.2/1.5/0.5，P0 实装时单测调
- [ ] **Q94. 修复 vortex_fake_skin_layers 命名**：DerivedAttrs 拆为 `tuike_layers: u8` + `vortex_active: bool`，需要同步改 plan-HUD-v1 引用
- [ ] **Q95. WoliuVortex Technique 学习路径**：v1 简化为玩家手动添加？还是接 plan-cultivation 修炼系统？建议**v1 简化为 debug command 添加**（类似其他流派的简化路径）

### vN+1 留待问题（plan-woliu-v2 时拍）

- [ ] **短时瞬涡变体（弹反式）**（worldview §五.防御.3 + skeleton §2 掌心瞬涡）—— 类似 zhenmai parry 但拦截远程
- [ ] **化虚 0.9 抽干 + 远程引导 + 太极反击**（user 提议高阶能力）—— 接 vN+2 完整流体算法
- [ ] **涡流叠加 + 旋转方向**（顺/逆 + 顺逆相消）—— 复杂团战机制
- [ ] **持续防御涡（永久 / 全场涡）**（化虚理论态）—— 已断绝
- [ ] **顿悟"识规则"加成**（skeleton §4 通灵 +0.05）—— 接 plan-cultivation 顿悟
- [ ] **染色加成（缜密色）+ 真元流速/池效率特性**（worldview §五:467 物资派性质，但允许 hook）
- [ ] **plan-tsy-dimension 坍缩渊负灵域真实演练**（涡流在负灵域必反噬）
- [ ] **HUD 显示 environment_qi 是否破坏神识设定**（已通过 Q87 reframe 信息分层解决，但 UI 长期演化）
- [ ] **涡流流 vs 阵法流互动**（阵法师布"反涡流阵"是否合理？）
- [ ] **完整 hand-slot 视觉差异**（盾面涡 / 掌心涡 / 双手涡）

## §11 进度日志

- 2026-04-26：骨架创建。**关键校准**：原 worldview "-0.8" 改为相对值 + 境界梯度（化虚级理论上限保留 0.8）。依赖 plan-cultivation-v1 染色 + plan-tsy-dimension 负灵域 + worldview §二/§五.防御.3。无对应详写功法书。
- 2026-05-03：从 skeleton 升 active。§A 概览 + §3.1 P0 + §3.2 P1 涡流·v1 规格落地（9 个决策点闭环 Q84-Q89 + 3 个细化问 1/2/3，6 个 v1 实装时拍板 Q90-Q95）。primary axis = 真元流速 + 池效率（worldview §五:467，**算计型 / 持久博弈型，非爆发**）。**v1 关键设计**：持涡 toggle 模式 + 立即减速 80% + 维持过久反噬手经脉永久 SEVERED + 信息分层（神识 inspection vs 相对体感）+ 命名错位修复（vortex_fake_skin_layers 拆为 tuike_layers + vortex_active）。**Hotbar 接入正典化**（user 设计原则）：所有技能走 1-9 数字键 "战斗·修炼" 栏（不是 F1-F9 consumable 栏），通过 plan-hotbar-modify-v1 已落地 UseQuickSlot 路径触发；本 plan 不新建专属 packet。**6 个流派 plan 同步原则**：前 5 个流派 plan（zhenfa / anqi / dugu / zhenmai / tuike）的"专属 packet"建议在 v1 实施时统一改走 hotbar 路径（详见 §8 跨 plan hotbar 同步修正备注）。
