# Bong · plan-woliu-v2 · 涡流流五招完整包

涡流功法五招完整包：动画 / 粒子 / 音效 / 伤害 / 真元消耗 / HUD / 全流程。承接 `plan-woliu-v1` ✅ finished（P0 基础真空吸入已实装）—— v2 引入**真空场物理**（半径 r 内负压区 → 目标被拉向施法者 → 真元从目标向施法者逆流）+ **涡流共振**（多目标时涡流叠加增益）+ **紊流爆发**（蓄力后释放真空场碎裂为物理冲击波），五招完整规格。**严守 worldview §五 涡流"以空制有"哲学**。

**世界观锚点**：`worldview.md §五:440-455 涡流核心`（展掌开涡 / 真空吸扯 / 以空制有 / 紊流窒息）· `§四:500 零距离贴脸施法`（涡流的"吸"让距离主动缩短）· `§五:460 涡流真空吸音`（施法时周围声音被吸走——音效设计依据）· `§P 异体排斥`（涡流 ρ 中等，靠真空负压而非注入突破防御）

**library 锚点**：`cultivation-0004 涡流散人手札`（真空场与灵气压差的关系）

**前置依赖**：
- `plan-skill-v1` ✅ + `plan-hotbar-modify-v1` ✅ → SkillRegistry / Casting / cooldown
- `plan-combat-no_ui` ✅ + `plan-combat-ui_impl` ✅ → 战斗事件垫
- `plan-multi-style-v1` ✅ → QiColor / PracticeLog / StyleAttack trait
- `plan-qi-physics-v1` ✅ → qi_collision / field / constants
- `plan-vfx-v1` ✅ → 粒子基类 / 屏幕效果
- `plan-particle-system-v1` ✅ → BongSpriteParticle / BongLineParticle
- `plan-audio-v1` ✅ → SoundRecipePlayer / AudioTriggerS2c
- `plan-HUD-v1` ✅ → BongHudOrchestrator
- `plan-cultivation-v1` ✅ → Realm / Cultivation / Meridian
- `plan-meridian-severed-v1` ✅ → SkillMeridianDependencies

**反向被依赖**：
- `plan-style-balance-v1` → 5 招数值进平衡矩阵
- `plan-audio-implementation-v1` → 涡流流 5 条专属音效 recipe

---

## 接入面 Checklist

- **进料**：`cultivation::Cultivation` / `qi_physics::field` / `SkillRegistry` / `SkillSet` / `Casting` / `PracticeLog` / `Realm` / `combat::CombatState`
- **出料**：5 招 `WoliuSkillId` enum 注册到 SkillRegistry / server `combat::woliu_v2::*` 模块 / client 5 动画 + 5 粒子 player + 5 音效 recipe + 2 HUD planner / `WoliuVacuumFieldEvent` / `WoliuVortexPullEvent` / `WoliuResonanceEvent` / `WoliuTurbulenceBurstEvent` / `WoliuSiphonEvent`
- **跨仓库契约**：server `combat::woliu_v2::*` → client `bong:woliu_vfx` + `bong:woliu_audio` CustomPayload / agent `tiandao::woliu_v2_runtime` narration
- **qi_physics 锚点**：涡流 ρ=0.35（中等排斥——不靠注入而是靠负压）走 `qi_physics::constants::WOLIU_RHO` / 真空场走 `qi_physics::field::vacuum_field_pull(center, radius, strength)` / 紊流爆发走 `qi_physics::field::turbulence_burst(center, radius, damage)` / **禁止 plan 内自写负压/吸力公式**
- **经脉依赖声明**：`SkillMeridianDependencies::declare(woliu_*, vec![手太阴肺经, 手少阴心经])` — 涡流依赖呼吸+心脉控制真空

---

## §0 设计轴心

- [ ] **以空制有**：涡流不"打出去"——它"吸进来"。所有招式核心是制造负压/真空
- [ ] **反向 shake**：涡流命中时 camera 向攻击源方向"被吸"（区别于体修向外震）
- [ ] **真空吸音**：施法时周围环境音 volume →0（"声音被吸走了"）→ 释放时恢复+爆音
- [ ] **淡紫色调**：涡流 qi_color = #9966CC，所有粒子/泛色/HUD 统一淡紫
- [ ] **每招差异化视听**：5 招各自独立动画+粒子+音效+HUD 反馈，不共用

---

## 五招规格

### 招式一：吸涡掌（Vacuum Palm）
- **定位**：基础攻击，单目标近距吸引+真元逆流
- **机制**：展掌 → 8 格内单目标被拉向施法者 2 block/s × 1.5s → 接触时真元逆流（从目标吸取 qi 15 点 → 施法者回复）
- **真元消耗**：20
- **冷却**：3s
- **动画**：单掌前推 → 掌心朝目标 → 手指微张（`woliu_vacuum_palm.json`，UPPER_BODY 6 tick）
- **粒子**：目标→施法者 淡紫色螺旋线（`BongLineParticle` 螺旋轨迹 × 4，从目标向掌心汇聚）
- **音效**：`woliu_vacuum_palm.json`（`minecraft:entity.enderman.teleport`(pitch 0.3, volume 0.3) + 周围环境音 ducking 0.5s）
- **HUD**：目标被吸时其位置出现淡紫箭头指向施法者（2s）

### 招式二：涡流护体（Vortex Shield）
- **定位**：防御技，身周真空层偏转来袭攻击
- **机制**：开启后 5s 内身周 2 格真空场 → 远程投射物偏转（命中率 -60%）+ 近战攻击者被微推 1 block → 持续消耗真元 5/s
- **真元消耗**：25 初始 + 5/s 持续
- **冷却**：12s
- **动画**：双掌环抱 → 缓慢旋转（`woliu_vortex_shield.json`，FULL_BODY loop）
- **粒子**：身周 2 格半透明淡紫球面（`BongSpriteParticle` 球面分布 × 16 持续旋转 + 偶发偏转闪光）
- **音效**：`woliu_vortex_shield.json`（`minecraft:block.portal.ambient`(pitch 2.0, volume 0.08) loop — 低频嗡鸣持续）
- **HUD**：真元条旁出现紫色"护体"小 icon + 持续时间倒计时弧线（auto-hide 5s 后消失）

### 招式三：真空锁（Vacuum Lock）
- **定位**：控制技，锁定目标移动
- **机制**：指定 12 格内目标 → 目标周围形成真空笼（3s 内移速 -80% + 无法跳跃）→ 被锁目标真元逸散加速（qi_drain ×2）
- **真元消耗**：35
- **冷却**：15s
- **动画**：双手合十 → 猛然张开（`woliu_vacuum_lock.json`，UPPER_BODY 8 tick）
- **粒子**：目标位置出现淡紫色笼状线框（`BongLineParticle` 球形线框 radius 1.5 block × 12 线条，lifetime 60 tick loop）
- **音效**：`woliu_vacuum_lock.json`（`minecraft:entity.generic.drink`(pitch 0.2, volume 0.4) — 真空吸力声 + 目标处环境音完全消失 3s）
- **HUD**：被锁目标头顶出现紫色锁链 icon（对双方可见）+ 施法者 HUD 显示剩余锁定时间

### 招式四：涡流共振（Vortex Resonance）
- **定位**：群体技，多目标涡流叠加
- **机制**：以施法者为中心 6 格球形 → 区域内所有敌对目标同时被轻微拉向中心（1 block/s）+ 每增加 1 个目标涡流强度 +20%（3 目标时 pull 1.6 block/s）→ 持续 4s
- **真元消耗**：50
- **冷却**：20s
- **动画**：双臂缓慢展开 → 掌心朝天 → 身体微浮 0.2 block（`woliu_resonance.json`，FULL_BODY 80 tick loop）
- **粒子**：施法者为中心的淡紫色涡旋平面（地面 `BongGroundDecalParticle` 漩涡纹 + 空中 `BongSpriteParticle` 螺旋向内收缩 × 8 per target）
- **音效**：`woliu_resonance.json`（`minecraft:entity.warden.sonic_boom`(pitch 0.2, volume 0.1) + `minecraft:block.portal.ambient`(pitch 1.5, volume 0.12) — 低频共振+嗡鸣叠加。目标数越多 volume 越大）
- **HUD**：施法者脚下出现紫色涡旋 indicator（范围 6 格圆）+ 每个被拉目标出现紫色←箭头

### 招式五：紊流爆发（Turbulence Burst）
- **定位**：终结技，真空场碎裂为物理冲击波
- **机制**：需要先 charge 2s（期间移速 -50% + 身周形成真空场）→ 释放：6 格球形冲击波（伤害 60 + 击退 4 block + 被击中目标 1s 眩晕）+ 施法者自身后退 2 block（反冲力）
- **真元消耗**：80
- **冷却**：30s
- **动画**：charge 阶段双掌合拢收紧 → release 双掌猛然推出 + 身体后仰（`woliu_burst_charge.json` FULL_BODY loop + `woliu_burst_release.json` FULL_BODY 6 tick）
- **粒子**：charge 期间周围粒子向内收缩（环境粒子被"吸入"） → release 瞬间淡紫色球形冲击波向外扩散（`BongSpriteParticle` 球面 × 32 快速向外 + `BongLineParticle` 径向线条 × 8）
- **音效**：charge `woliu_burst_charge.json`（环境音渐消 → 2s 内 volume 0→完全静音）→ release `woliu_burst_release.json`（`minecraft:entity.generic.explode`(pitch 1.5, volume 0.5) + 环境音瞬间恢复 — "真空碎裂的爆响"）
- **HUD**：charge 期间屏幕边缘微收紧（vignette 淡紫）+ release 时全屏白闪 0.1s + camera 向后 shake

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | server `combat::woliu_v2` 模块骨架 + SkillRegistry 注册 5 招 + 吸涡掌 server 全实装 + qi_physics 真空场 API 接入 + 经脉依赖声明 + 30 单测 | ⬜ |
| P1 | 涡流护体 + 真空锁 server 实装 + client 前 3 招动画/粒子/音效/HUD 全流程 | ⬜ |
| P2 | 涡流共振 + 紊流爆发 server 实装 + client 后 2 招动画/粒子/音效/HUD 全流程 | ⬜ |
| P3 | 环境音 ducking 系统（涡流施法时周围声音被吸走）+ HUD auto-hide 策略 + 全 5 招视听 polish | ⬜ |
| P4 | agent narration 接线（天道对涡流的评价模板）+ 5 招 × 全境界 × 全距离 饱和化测试 | ⬜ |

---

## Finish Evidence（待填）

- **落地清单**：server `combat::woliu_v2::*` / 5 招 SkillRegistry / qi_physics vacuum_field / client 5 动画 + 5 粒子 + 5 音效 + 2 HUD / 环境音 ducking / agent narration
- **关键 commit**：P0-P4 各自 hash
- **测试结果**：5 招 server 单测 + client VFX e2e
- **遗留 / 后续**：涡流 + 其他流派交互效果（吸+毒？吸+爆？）→ style-balance-v1
