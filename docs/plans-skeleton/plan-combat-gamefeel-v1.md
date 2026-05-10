# Bong · plan-combat-gamefeel-v1 · 骨架

战斗手感/打击感（game feel / juice）专项。当前战斗系统功能完备（hit/damage/qi_transfer/wounds/parry/dodge 全有）但**体感反馈偏软**——命中敌人和砍空气差别不大。本 plan 补全 6 层 juice：hit-stop / 屏幕微震 / 命中粒子 / 音效反馈 / 受击动画 / 击杀慢动作。不碰战斗数值/判定——纯表现层。

**世界观锚点**：`worldview.md §四` 战斗是"真元汇率兑换"——近战肉搏、零距离灌真元、过载撕裂的痛 · `§五` 七流派的攻击质感不同（体修沉重、暗器尖锐、毒蛊阴渗、涡流真空吸扯）· `§P` 异体排斥物理（攻击方真元侵入防守方体内——应该有"侵入"的视觉）

**library 锚点**：`cultivation-0003 爆脉流正法`（体修战斗的"撞击感"描述）

**前置依赖**：
- `plan-combat-no_ui` ✅ + `plan-combat-ui_impl` ✅ → 战斗命中/招架/闪避事件
- `plan-qi-physics-v1` ⏳ active → qi_collision 事件（异体排斥）
- `plan-vfx-v1` ✅ → 屏幕效果通道（shake/vignette/flash）
- `plan-particle-system-v1` ✅ → BongLineParticle / BongRibbonParticle 渲染
- `plan-audio-implementation-v1` 🆕 skeleton → 战斗音效 recipe
- `plan-player-animation-implementation-v1` 🆕 skeleton → 受击 stagger 动画
- `plan-HUD-v1` ✅ → MiniBodyHudPlanner（伤口实时渲染）

**反向被依赖**：
- `plan-baomai-v3` / `plan-dugu-v2` / etc → 各流派招式 juice profile

---

## 接入面 Checklist

- **进料**：`combat::HitEvent` / `combat::ParryEvent` / `combat::DodgeEvent` / `qi_physics::collision::QiCollisionEvent` / `combat::KillEvent` / `cultivation::Wounds`（体表伤口）/ VFX 屏幕效果系统 / 音效系统
- **出料**：`CombatJuiceProfile`（per-attack-type 参数: hit_stop_ticks / shake_intensity / impact_particle / hit_sound_recipe / kill_slowmo_duration）+ `CombatJuiceSystem`（消费 combat event → 播放 juice）+ 6 层果汁的参数预设
- **跨仓库契约**：纯 client 侧——server combat event 已有，client 侧新增 consumer

---

## §0 设计轴心

- [ ] **不碰 combat 数值**——只改表现，不改判定
- [ ] **6 层 juice 按强度分层**：轻击 = 低 juice / 过载撕裂一击 = 满 juice
- [ ] **流派质感差异**：体修 hit = 沉重低音 + 大 shake / 暗器 hit = 锐利高音 + 微震 / 毒蛊 hit = 阴渗嗡 + 微绿粒子 / 涡流 hit = 真空吸音 + 吸入粒子
- [ ] **异体排斥可视化**：QiCollisionEvent → 攻击方真元颜色粒子（红色锋锐 / 古铜沉重 / 墨绿阴诡）灌入防守方体内 + 防守方身体短暂泛对应色

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | `CombatJuiceProfile` 数据结构 + `CombatJuiceSystem` consumer 骨架 + hit-stop（命中瞬间 2-8 tick freeze frame）+ 屏幕微震（hit 时 camera shake 参数化）+ 命中粒子（默认白色火花粒子）+ 3 个基础音效（light_hit / heavy_hit / block）插入 combat event consumer | ⬜ |
| P1 | 异体排斥可视化：`QiCollisionEvent` → 攻击方 QiColor 对应颜色粒子束灌入防守方 + 防守方短暂泛色（0.3s color tint）+ 过载撕裂 visual（爆脉时红色裂痕粒子从关节喷出 + 屏幕 red vignette 闪）+ 全力一击 release juice（max hit-stop + max shake + 金色爆发粒子 + 低音吼 audio） | ⬜ |
| P2 | 招架/弹反 juice：成功招架 → 清脆金属音 + 蓝色火花 + 攻守双方微退 stagger 动画 / 弹反成功（截脉·震爆）→ 爆炸粒子 + 高亮白闪 + 特殊音效 / 闪避 → 残影粒子（短暂 afterimage）+ 微风音 | ⬜ |
| P3 | 伤口 juice：不同伤口档在 MiniBodyHudPlanner 之外增加世界内表现——骨折（FRACTURE）→ 对应部位模型微偏移+红光 / 断肢（SEVERED）→ 断口粒子喷血（暗红修仙风格）+ 移速跛行 / 污染（contamination）→ 经脉路线泛墨绿微光 | ⬜ |
| P4 | 击杀 juice：杀 NPC → 魂散粒子（已有 DeathSoulDissipatePlayer 接入）+ 击杀慢动作（0.3× speed × 1s）+ 杀玩家 → 全服 narration 同步弹出 + 对方掉落物 3D 弹射可视化 | ⬜ |
| P5 | 流派专属 juice profile：7 流派 × 3 强度（轻/中/重击）各一套 juice 参数 → PVP 实测校准手感 + 饱和化测试（5 玩家同时战斗不掉帧） | ⬜ |

---

## Finish Evidence（待填）

- **落地清单**：`CombatJuiceProfile` / `CombatJuiceSystem` / hit-stop / shake / 粒子 / audio / 异体排斥 vis / wound vis / kill juice / 7 流派 profile
- **关键 commit**：P0-P5 各自 hash
- **遗留 / 后续**：不同武器材质的 hit 音/粒子差异（未来 `plan-weapon-v2` 联动）
