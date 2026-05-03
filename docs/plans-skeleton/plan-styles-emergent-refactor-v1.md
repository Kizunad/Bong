# Bong · plan-styles-emergent-refactor-v1 · 骨架

`UnlockedStyles` 字段重定位 + 7 流派字段扩展。

**起源**：2026-05-03 user 反馈"流派应自由涌现，无系统门禁"，worldview §五"流派由组合涌现"已正典化（commit pending）。当前 `combat/components.rs:353` `UnlockedStyles` 仅含 jiemai/tishi/jueling 三防御流字段，且语义是"HUD 渲染门禁"。本 plan 把它**重定位为"已用过标记"**——任何玩家都能试任何招式，字段只标记"曾经成功演示过"。

**世界观锚点**：`worldview.md §五 "流派由组合涌现"`（2026-05-03 正典）· `worldview.md §五:482 "流派反过来塑造染色"` · `worldview.md §A.5 全流派精通路径`（O.14）

**library 锚点**：`peoples-0006 战斗流派源流`（流派定义本质）· `cultivation/真元十一色考`（染色与流派的物理副产物关系）

**交叉引用**：
- `plan-cultivation-v1` ✅（QiColor + 经脉路径 + 顿悟系统的源头）
- 6 流派 plan（baomai/anqi/zhenfa/dugu/jiemai/tuike/woliu）✅ 全 active — 每个流派需要在 `UnlockedStyles` 注册"已用过"标记
- `plan-hotbar-modify-v1` ✅（已落地）— Technique cast 不再依赖 UnlockedStyles 门禁
- `plan-perception-v1.1` ✅（神识 inspect 看到对方 `UnlockedStyles` 标记）
- `plan-identity-v1` ⬜ 派生（NPC 识破玩家流派标签时读 `UnlockedStyles`）

---

## §0 设计轴心

- [ ] **不锁触发**：任何 Technique / 招式触发不再校验 `UnlockedStyles` — 玩家可以试任何招式，效果取决于适配度
- [ ] **已用过标记**：`UnlockedStyles` 字段语义改为"曾经成功演示过此流派招式"
- [ ] **7 流派扩展**：从 3 字段（jiemai/tishi/jueling）扩到 7 字段（baomai/anqi/zhenfa/dugu + 防御 3）
- [ ] **HUD 重渲染**：客户端 `UnlockedStylesStore` mirror 改为渲染"已用过状态" + 染色亲和度提示，不再是"解锁/未解锁"

---

## §1 字段扩展

```rust
// 当前（components.rs:353）
pub struct UnlockedStyles {
    pub jiemai: bool,
    pub tishi: bool,
    pub jueling: bool,
}

// 重构后
pub struct UnlockedStyles {
    // 攻击四流
    pub baomai: bool,
    pub anqi: bool,
    pub zhenfa: bool,
    pub dugu: bool,
    // 防御三流
    pub jiemai: bool,
    pub tishi: bool,
    pub jueling: bool,
}
```

**Default 改为全 false**（取代当前的全 true placeholder）—— 玩家从未用过任何流派开始，每首次成功使用一个招式 → 对应字段 mutate 为 true。

---

## §2 触发逻辑改造

**当前**（隐式 / placeholder）：
- 各流派 Technique cast 假设要校验 UnlockedStyles 字段
- HUD 按字段渲染流派指示器

**重构后**：
- Technique cast 不校验 UnlockedStyles
- 各流派 P0 实装时在"首次成功演示"位置 mutate 字段 → emit `StyleUsedEvent`
  - baomai: 首次成功 burst meridian attack → `UnlockedStyles.baomai = true`
  - anqi: 首次成功 ThrowCarrierIntent + 命中 → `UnlockedStyles.anqi = true`
  - zhenfa: 首次成功 PlaceTrap + trigger → `UnlockedStyles.zhenfa = true`
  - dugu: 首次成功 InfuseDuguPoisonIntent + 命中 → `UnlockedStyles.dugu = true`
  - jiemai: 首次成功 parry（DefenseTriggered::JieMai effectiveness > 0）→ `UnlockedStyles.jiemai = true`
  - tishi: 首次脱壳（Shed event layers_shed > 0）→ `UnlockedStyles.tishi = true`
  - jueling: 首次成功 vortex 拦截（ProjectileQiDrainedEvent）→ `UnlockedStyles.jueling = true`

---

## §3 HUD 重渲染

客户端 `UnlockedStylesStore`：
- 已 used 流派：常驻 HUD 槽位 + 高亮（"你掌握过此流派"）
- 未 used 流派：HUD 槽位灰显但不隐藏 — 提示"未曾尝试"
- 染色亲和度提示：玩家当前 QiColor 主色 + 流派对应染色 → 显示"亲和" tag（如凝实色玩家看 anqi 槽位带"亲和"标签）

接 plan-hotbar-modify-v1 的 InspectScreen "战斗·修炼" tab — 显示已 used 状态 + 染色亲和度，但**不影响绑定权限**（任何 Technique 都能绑到 1-9 槽，无门禁）。

---

## §4 跨 plan 同步影响

需要 6 流派 plan 在各自 P0 实装时加 "首次成功演示 → emit StyleUsedEvent → mutate UnlockedStyles" 逻辑：

| 流派 | 首次成功标志 | 在哪个 plan 加 |
|---|---|---|
| baomai | 首次 BurstMeridian attack 成功 | plan-baomai-v1（已实装 ✅）— 需补 mutate |
| anqi | 首次 ThrowCarrierIntent 命中 | plan-anqi-v1 P0 实装时加 |
| zhenfa | 首次诡雷 trigger 命中 | plan-zhenfa-v1 P0 实装时加 |
| dugu | 首次 DuguPoisonState 挂载 | plan-dugu-v1 P0 实装时加 |
| jiemai | 首次 jiemai effectiveness > 0 | plan-zhenmai-v1 P1 实装时加 |
| tishi | 首次 ShedEvent layers_shed > 0 | plan-tuike-v1 P0 实装时加 |
| jueling | 首次 ProjectileQiDrainedEvent | plan-woliu-v1 P0 实装时加 |

---

## §5 数据契约

- [ ] `server/src/combat/components.rs::UnlockedStyles` 字段扩展（3 → 7）
- [ ] `server/src/combat/events.rs::StyleUsedEvent` 新增
- [ ] `server/src/combat/style_used_tracker.rs` 系统（消费 StyleUsedEvent → mutate UnlockedStyles）
- [ ] `agent/packages/schema/src/styles.ts::UnlockedStylesV1` 扩展 7 字段
- [ ] `client/.../combat/UnlockedStylesStore.java` 重新渲染逻辑
- [ ] `client/.../hud/StyleIndicatorPlanner.java` 灰显未 used vs 高亮 used
- [ ] 6 流派 plan 各自 P0 加 mutate 逻辑（plan 间协调）

---

## §6 实施节点

- [ ] **P0** 字段扩展（components.rs）+ Default 改全 false + StyleUsedEvent + style_used_tracker 系统 + schema 扩展
- [ ] **P1** 6 流派 plan 同步加 mutate 逻辑（跨 plan 协调，commit 同期）
- [ ] **P2** HUD 重渲染（灰显未 used + 染色亲和度 tag）+ InspectScreen "战斗·修炼" tab 更新
- [ ] **P3** plan-perception-v1.1 接入（神识 inspect 看到对方 UnlockedStyles）+ plan-identity-v1 接入（NPC 识破玩家流派标签）

---

## §7 开放问题

- [ ] 玩家"已用过"标记是否可被 dugu obfuscation 隐藏？（dugu 师 always-on 遮蔽下，对方 inspect 看不到 dugu 标签）
- [ ] 多人协作触发（A 用 anqi + B 用 dugu 配合命中）— 谁的 UnlockedStyles mutate？
- [ ] 标记是否随死亡 / 多周目继承？（journey M.3 多周目"实力归零"原则下，新角色 UnlockedStyles 应清零）
- [ ] 染色亲和度 tag 的具体公式（QiColor 主色 % 阈值）？
- [ ] inspect 神识看到对方"已用过"流派标签的境界差阈值（plan-perception-v1.1 默认 Δ≥2 完全识破）？

---

## §8 进度日志

- 2026-05-03：骨架立项。来源：user 反馈"流派应自由涌现，无系统门禁"+ worldview §五"流派由组合涌现" 2026-05-03 正典化。当前 `UnlockedStyles` (combat/components.rs:353) 仅 3 防御流字段，语义是"HUD 门禁"——本 plan 重定位为"已用过标记"+ 扩 7 流派字段。**关键设计**：取代已撤销的 plan-style-pick-v1（user 反馈"不应让玩家选择流派，流派应自由"）。
