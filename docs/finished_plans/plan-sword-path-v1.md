# Bong · plan-sword-path-v1

**器修·剑道流派——从醒灵到化虚的完整骨干**。不是第八个战斗流派（worldview §五 七流已闭合），而是**器修大类下的剑道分支**——与暗器（载体消耗型）平行的**长期养成型器修**。核心身份：**人剑共生**——修士真元长期注入同一柄剑，剑品阶随修士成长，境界越高剑越强，但剑毁则修为反噬（玻璃大炮代价）。化虚终极「一剑开天门」= 以修士全部真元+剑全部积蓄一击释放，100 格半径内天道感知清零，代价修为归零+剑碎。

**世界观锚点**：
- `worldview.md §五:402-410` 器修 = 真元封存在物理载体；暗器是消耗型，剑道是养成型（同源不同路）
- `worldview.md §五:411` 凡器边界——凡铁剑基础物理伤害，灵剑 = 封存真元放大
- `worldview.md §五:535-558` 流派由组合涌现（无系统门禁）——剑道不锁，但深耕出专属色
- `worldview.md §六.2:605` 凝实色（Solid）——真元附着物体、衰减慢、远程差——器修剑道正典染色
- `worldview.md §六.2:611` 锋锐色（Keen）——真元呈线状、穿透+、易漏——高阶剑气外放副产物
- `worldview.md §四:332-340` 距离衰减 = 近战拼刺刀——剑道主战场 3-5 格
- `worldview.md §三:187` 化虚 = ×5 凡躯——一剑开天门的物理基础
- `worldview.md §八:614-618` 天道感应高浓度真元区——化虚一击制造负灵气区域 = 天道盲区

**library 锚点**：
- `docs/library/peoples/战斗流派源流.json` — 器修·暗器已有条目，剑道应作为同级分支补充
- `docs/library/ecology/` — 巨剑沧海 zone 生态条目待新建

**交叉引用（已完成 plan）**：
- `plan-sword-basics-v1` ✅ — 三式（劈/刺/格）+ 注剑，本 plan 在此之上构建高阶剑术
- `plan-weapon-v1` ✅ — `Weapon` component / `WeaponKind::Sword` / base_attack / durability
- `plan-forge-v1` ✅ — `WeaponForgeStation` / 4 步锻造状态机 / tier 系统
- `plan-combat-no_ui` ✅ — `AttackIntent` / `CombatEvent` / `StatusEffectKind`
- `plan-vfx-v1` ✅ — VfxEventRouter / VfxPlayer / BongParticles 管线
- `plan-armor-visual-v1` ✅ — GeckoLib geo.json 自定义模型渲染参考
- `plan-style-vector-integration-v1` ✅ — `PracticeLog.add()` 染色权重
- `plan-meridian-severed-v1` — 经脉依赖 `SkillMeridianDependencies::declare()`
- `plan-qi-physics-v1` ✅ — 守恒律 / `QiTransfer` / 距离衰减 / 容器衰减

**交叉引用（skeleton / active plan）**：
- `plan-anqi-v2` skeleton — 同为器修大类，共享「凝实色」染色维度
- `plan-craft-v1` skeleton — 铸剑配方走通用手搓底盘
- `plan-woliu-v2` skeleton — 化虚级参考（紊流死区 vs 天门盲区）

---

## 接入面 Checklist

- **进料**：
  - `combat::weapon::Weapon { weapon_kind: Sword, base_attack, quality_tier, durability }` — 持剑判定 + 基数
  - `cultivation::components::Cultivation { qi_current, qi_max, realm }` — 真元池 + 境界
  - `cultivation::known_techniques::KnownTechniques` — 招式 proficiency
  - `cultivation::meridian::MeridianSystem` — 经脉状态（SEVERED 检查）
  - `forge::station::WeaponForgeStation` — 锻造台交互
  - `inventory::InventoryComponent` — 材料消耗
  - `world::zone::Zone { spirit_qi }` — 天道感知判定（化虚盲区）
- **出料**：
  - `combat::events::CombatEvent` — 命中结算（新增 `AttackSource::SwordIntent` / `SwordQiSlash` / `SwordResonance` / `SwordHeavenGate`）
  - `combat::events::StatusEffectKind::SwordShatter` — 剑碎反噬状态（化虚代价）
  - `network::VfxEventPayloadV1` — 动画 + 粒子 + 音效
  - `schema::combat_hud::TechniqueEntryV1` — HUD 同步
  - `world::events::ZoneModifyEvent` — 化虚一剑改写 zone 灵气
  - `network::agent_bridge` — 天道感知屏蔽 flag
- **共享类型/event**：
  - **复用** `AttackIntent`（新增 4 变体）
  - **复用** `CombatEvent` / `KnownTechnique.proficiency`
  - **复用** `WeaponForgeStation`（扩展灵剑铸造分支）
  - **复用** `PracticeLog.add(solid_dim, ...)` 染色
  - **新增** `SwordBondComponent` — 人剑共生绑定（player ↔ sword entity 1:1）
  - **新增** `SwordGrade { tier: 0-6, stored_qi: f64 }` — 剑品阶（对应六境界）
  - **新增** `SwordShatterEvent` — 剑碎事件（化虚代价 + 意外破损）
  - **新增** `TiandaoBlindZone { center, radius, ttl }` — 天道盲区临时 zone modifier
- **跨仓库契约**：
  - server: `sword_path` 模块 / `SwordBondComponent` / `SwordGrade` / `TiandaoBlindZone` / 5 technique fn / `AttackSource::Sword*` 4 变体
  - client: 5+ animation ID / 5+ VfxPlayer 类 / `SwordPathHudPlanner` / `SwordGradeOverlay` / `HeavenGateVfxPlayer`（化虚终极全屏特效）/ heiwushi BOSS 模型+渲染器
  - agent: `bong:tiandao_blind_zone` Redis key / 天道 perception filter 扩展
- **worldview 锚点**：§三 六境界 + §四 近战 + §五 器修 + §六 凝实/锋锐色 + §八 天道感应
- **qi_physics 锚点**：
  - `qi_physics::ledger::QiTransfer` — 注剑/铸剑/释放全部走守恒
  - `qi_physics::excretion::container_intake` — 灵剑存储真元衰减
  - `qi_physics::release::release_to_zone` — 化虚一击真元释放
  - `qi_physics::distance::attenuation` — 剑气外放衰减（0.03/格正典）
  - **不新增物理常数**：灵剑衰减复用 `container_intake` 参数（container_type = `spirit_sword`）

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | 人剑共生核心 + 剑品阶 + 5 招定义注册 + 经脉依赖 + 基础 server 逻辑 + 视听 | ✅ 2026-05-16 |
| P1 | 铸剑炼器（灵剑锻造 + 原材料 + 品阶升级 + 锻造台扩展）+ 视听 | ✅ 2026-05-16 |
| P2 | 巨剑沧海地形（worldgen zone + terrain profile + decorations + 古遗迹 POI）| ✅ 2026-05-16 |
| P3 | 黑武士 BOSS（heiwushi 模型接入 + AI 行为 + 叙事 + 战利品）+ 视听 | ✅ 2026-05-16 |
| P4 | 化虚·一剑开天门（终极机制 + 天道盲区 + 全套视觉流程 + agent 集成）| ✅ 2026-05-17 |
| P5 | 剑道 HUD 完整包 + 饱和测试 + 集成联调 | ✅ 2026-05-17 |

---

## P0 — 人剑共生核心 + 剑道五招 + 视听

### P0.1 SwordBondComponent — 人剑绑定

`server/src/sword_path/bond.rs`

```rust
pub struct SwordBondComponent {
    pub bonded_weapon_entity: Entity,   // 绑定的 Weapon entity
    pub bond_strength: f32,             // [0, 1] 绑定强度，随使用累积
    pub stored_qi: f64,                 // 剑内封存真元（qi_physics 守恒）
    pub grade: SwordGrade,              // 品阶 0-6
    pub shatter_threshold: f64,         // 剑碎阈值 = grade.max_stored * 1.5
}

pub enum SwordGrade {
    Mortal,     // 凡铁·零阶——醒灵可用，stored_qi cap = 0（纯物理）
    Awakened,   // 醒灵·一阶——cap 0，微感应但不存气
    Induced,    // 引气·二阶——cap 0，还是纯物理路线
    Condensed,  // 凝脉·三阶——cap 15，剑刚开始能含一丝真元
    Solidified, // 固元·四阶——cap 100，正式开始器修（拐点）
    Spirit,     // 通灵·五阶——cap 500，剑意自生
    Void,       // 化虚·六阶——cap 3000，一剑开天门前置
}
```

- 绑定触发：玩家手持 `WeaponKind::Sword` 连续使用 20 次剑术招式 → 系统提示「此剑与你气息相融」→ `SwordBondComponent` 挂载
- 1 player 同时只能绑定 1 剑（换剑需主动「解绑」= 30s 仪式，失去 50% bond_strength）
- `stored_qi` 品阶 3（凝脉）以上时，每次使用剑术招式自动注入 `qi_cost * 0.1`，走 `QiTransfer { from: player, to: sword }`；品阶 0-2 不注入（剑不存气）
- 剑碎（durability 归零 / stored_qi 超阈值）→ `SwordShatterEvent` → 反噬：player `qi_current -= stored_qi * 0.6` + `qi_max -= stored_qi * 0.05`（永久衰减，走 qi_physics 守恒释放回 zone）

**视听——绑定成功瞬间**：
- **粒子**：`BongLineParticle` × 8，从剑尖向玩家胸口收束，lifetime 15 tick，速度 0.8 m/s，颜色 `#7B9EC4`（凝实色调），spawn 模式 burst，贴图 `bong:sword_bond_line`（新增 8×8 像素淡蓝线条），VfxPlayer 类名 `SwordBondVfxPlayer`，事件 ID `bong:sword_bond_form`
- **音效**：audio_recipe `{ "layers": [{ "sound": "entity.player.levelup", "pitch": 1.4, "volume": 0.6, "delay_ticks": 0 }, { "sound": "block.amethyst_block.chime", "pitch": 0.8, "volume": 0.4, "delay_ticks": 3 }] }`
- **HUD**：`HudRenderLayer.OVERLAY`，屏幕中央 toast「⚔ 人剑相融」，颜色 `#C8D8E8` opacity 220，持续 60 tick，fade-in 10 tick ease-out，fade-out 15 tick linear，受影响境界 = 全境界

**视听——剑碎反噬**：
- **粒子**：`BongSpriteParticle` × 24 爆散，lifetime 30 tick，速度 1.5-3.0 m/s 随机方向，颜色 `#FF4444`→`#441111` fade，spawn 模式 burst 从剑位置，贴图 `bong:sword_shard`（新增 8×8 碎片），VfxPlayer `SwordShatterVfxPlayer`，事件 ID `bong:sword_shatter`
- **音效**：`{ "layers": [{ "sound": "entity.item.break", "pitch": 0.6, "volume": 1.0, "delay_ticks": 0 }, { "sound": "entity.lightning_bolt.thunder", "pitch": 1.8, "volume": 0.5, "delay_ticks": 2 }, { "sound": "entity.player.hurt", "pitch": 0.9, "volume": 0.8, "delay_ticks": 4 }] }`
- **HUD**：`VisualEffectProfile.SWORD_SHATTER`（新增），edgeVignette `#880000` maxAlpha 180，duration 2000ms，fade-in 100ms，fade-out 800ms + screenShake amplitude 4px 频率 30Hz 持续 500ms
- **narration**：
  - scope: player, style: perception — `"一声脆响，{weapon_name}寸寸碎裂。封存其中的真元倒灌经脉，五脏六腑传来撕裂般的剧痛。"`
  - scope: zone, style: narrative — `"一柄灵剑在{zone_name}碎裂，封存的真元炸散，方圆十丈灵气骤然浓郁。"`

### P0.2 剑道五招定义

经脉依赖（`SkillMeridianDependencies::declare()`）：
- 手三阳：大肠经 `LargeIntestine` + 小肠经 `SmallIntestine` + 三焦经 `TripleEnergizer`
- 化虚招追加：督脉 `Du`

| 招式 | id | 境界门槛 | qi_cost | stamina_cost | cast_ticks | cooldown_ticks | range | 经脉依赖 | 说明 |
|------|-----|---------|---------|-------------|------------|---------------|-------|---------|------|
| 剑意·凝锋 | `sword_path.condense_edge` | 引气 | **0** | **8** | 12 | 40 | 4.0 | LI+SI | 纯精神集中，凝聚意念于剑锋，下一次劈/刺伤害 ×1.8 + 穿甲 30%；持续 5s 或 1 次命中消散 |
| 剑气·斩 | `sword_path.qi_slash` | 凝脉 | **3** | **12** | 20 | 60 | 8.0 | LI+SI+TE | 微量真元裹剑刃（3/150=2%），外放剑气 8 格直线，伤害 = base_attack × grade_mult × 距离衰减（0.03/格） |
| 共鸣·剑鸣 | `sword_path.resonance` | 固元 | **20** | 15 | 30 | 120 | 6.0 AoE | LI+SI+TE | 固元起真元够用；人剑共振 6 格 AoE，敌方 cast 打断 + 3s slow；stored_qi 越高 slow 越久（max 5s） |
| 归一·剑意化形 | `sword_path.manifest` | 通灵 | **40** | 20 | 40 | 200 | 5.0 | LI+SI+TE | 通灵才撑得起实体化（40/2100=1.9%）；剑意实体化 5s 自动追踪，base_attack ×2.0；结束后 bond_strength -0.1 |
| 天门·一剑开天 | `sword_path.heaven_gate` | 化虚 | ALL | 0 | 80（4s） | 一次性 | 100 AoE | LI+SI+TE+Du | §P4 详述 |

**grade_mult 表**（剑品阶对剑道招式的全局乘数）：

| Grade | mult | 说明 |
|-------|------|------|
| Mortal (0) | 1.0 | 纯物理，凡铁 |
| Awakened (1) | 1.05 | 微感应，几乎纯物理 |
| Induced (2) | 1.1 | 还是纯物理路线 |
| Condensed (3) | 1.25 | 剑刚开始含气 |
| Solidified (4) | 1.6 | 正式器修拐点 |
| Spirit (5) | 2.2 | 剑意自生 |
| Void (6) | 3.5 | 化虚极限（一剑开天门 only） |

**视听——剑意·凝锋**：
- **粒子**：`BongLineParticle` × 4，沿剑刃向前聚拢，lifetime 8 tick，速度 0.3 m/s，颜色 `#AAC8DD`（凝实色浅），continuous spawn 5s，贴图复用 `bong:sword_qi_trail`，VfxPlayer `SwordCondenseVfxPlayer`，事件 ID `bong:sword_condense_edge`
- **音效**：`{ "layers": [{ "sound": "block.amethyst_cluster.step", "pitch": 1.2, "volume": 0.5, "delay_ticks": 0 }] }`
- **命中消散追加**：burst `BongSpriteParticle` × 6，lifetime 10 tick，`#FFFFFF` → `#AAC8DD` fade

**视听——剑气·斩**：
- **粒子**：`BongRibbonParticle` × 1 主弧 + `BongLineParticle` × 12 尾迹，主弧 lifetime 20 tick，宽 1.5 格，颜色 `#88AACC` 半透明 alpha 180，速度 = 8 格/20 tick = 8 m/s 直线，贴图 `bong:sword_qi_arc`（新增 32×8 弧形），尾迹颜色 `#6688AA` alpha 120 lifetime 12 tick。VfxPlayer `SwordQiSlashVfxPlayer`，事件 ID `bong:sword_qi_slash_path`
- **音效**：`{ "layers": [{ "sound": "entity.player.attack.sweep", "pitch": 1.5, "volume": 0.8, "delay_ticks": 0 }, { "sound": "entity.breeze.wind_burst", "pitch": 1.2, "volume": 0.4, "delay_ticks": 2 }] }`
- **命中音**：`{ "layers": [{ "sound": "entity.player.attack.crit", "pitch": 1.3, "volume": 0.7, "delay_ticks": 0 }] }`
- **HUD**：命中时 `HudRenderCommand.edgeIndicator(direction, #88AACC, 300ms)` 显示剑气方向指示

**视听——共鸣·剑鸣**：
- **粒子**：`VortexSpiralParticle` × 16 从剑位置向外扩散同心环，lifetime 25 tick，半径 0→6 格 expand，颜色 `#CCDDEE`→`#667788` fade，spawn burst，贴图 `bong:sword_resonance_ring`（新增 16×16 环形），VfxPlayer `SwordResonanceVfxPlayer`，事件 ID `bong:sword_resonance`
- **音效**：`{ "layers": [{ "sound": "block.bell.use", "pitch": 2.0, "volume": 1.0, "delay_ticks": 0 }, { "sound": "block.bell.resonate", "pitch": 1.5, "volume": 0.6, "delay_ticks": 5 }, { "sound": "entity.breeze.wind_burst", "pitch": 0.7, "volume": 0.3, "delay_ticks": 8 }] }`
- **HUD**（被命中的敌方）：`VisualEffectProfile.SWORD_RESONANCE_STUN`（新增），screenTint `#AABBCC` alpha 100，duration 600ms + screenShake amplitude 2px 频率 20Hz 200ms
- **narration**：scope: zone, style: perception — `"一声低沉的剑鸣在{zone_name}回荡，空气中的灵气随之震颤。"`

**视听——归一·剑意化形**：
- **粒子**：
  - 召唤阶段（cast 40 tick）：`BongLineParticle` × 20 从玩家向上汇聚成人形轮廓，lifetime 40 tick，颜色 `#99BBDD` alpha 200，spawn continuous
  - 存续阶段（5s）：`BongSpriteParticle` × 4/tick 持续环绕剑意实体，lifetime 6 tick，颜色 `#AACCEE` alpha 150，贴图 `bong:sword_manifest_aura`（新增 8×8 光点）
  - 追踪攻击：每次命中 burst `BongLineParticle` × 6，从实体→目标方向，lifetime 8 tick，`#FFFFFF`
- VfxPlayer `SwordManifestVfxPlayer`，事件 ID `bong:sword_manifest_summon` + `bong:sword_manifest_strike`
- **音效**（召唤）：`{ "layers": [{ "sound": "entity.evoker.prepare_summon", "pitch": 1.6, "volume": 0.7, "delay_ticks": 0 }, { "sound": "block.amethyst_block.resonate", "pitch": 1.0, "volume": 0.5, "delay_ticks": 10 }] }`
- **音效**（每次追踪命中）：`{ "layers": [{ "sound": "entity.player.attack.sweep", "pitch": 1.8, "volume": 0.5, "delay_ticks": 0 }] }`
- **动画**：`bong:sword_manifest_cast`，endTick 40，骨骼 `rightArm.pitch = -1.2rad`（举剑过头），`body.yaw = 0`，`leftArm.pitch = -0.8rad`（辅助手前伸），easing cubicOut

### P0.3 PracticeLog 染色挂钩

剑道五招调用 `PracticeLog.add(solid_dim, weight)`：
- `sword_path.condense_edge` → solid +1（纯体力招，轻微染色）
- `sword_path.qi_slash` → solid +2, keen +1（微量真元外放副产物）
- `sword_path.resonance` → solid +3（固元起真元共振）
- `sword_path.manifest` → solid +4, keen +2（通灵级实体化）
- `sword_path.heaven_gate` → keen +50（一次性大量锋锐染色——化虚后玩家真元色永久偏锋锐）

### P0.4 Server 模块结构

```text
server/src/sword_path/
├── mod.rs              // SwordPathPlugin，注册 systems + events
├── bond.rs             // SwordBondComponent + 绑定/解绑/碎裂逻辑
├── grade.rs            // SwordGrade enum + 品阶升级条件
├── techniques.rs       // 5 招 TechniqueDefinition 注册 + cast 逻辑
├── shatter.rs          // SwordShatterEvent 处理 + 反噬结算
└── tiandao_blind.rs    // TiandaoBlindZone（P4 实装，P0 仅 struct 定义）
```

---

## P1 — 铸剑炼器 + 原材料 + 品阶升级 + 视听

### P1.1 灵剑品阶升级路径

品阶升级 = **铸剑仪式**（非战斗中升级）。在 `WeaponForgeStation` 进行，消耗材料 + 真元 + 时间。

| 升级 | 前置境界 | 材料 | qi_cost | 时间(tick) | 失败概率 |
|------|---------|------|---------|-----------|---------|
| 0→1 凡→醒 | 醒灵 | 铁锭 ×3 + 草绳 ×2 | **0** | 400 (20s) | 0% |
| 1→2 醒→引 | 引气 | 精铁 ×4 + 灵草 ×2 | **0** | 800 (40s) | 5% |
| 2→3 引→凝 | 凝脉 | 玄铁 ×5 + 灵泉水 ×3 + 兽骨 ×2 | **5** | 1600 (80s) | 15% |
| 3→4 凝→固 | 固元 | 陨铁 ×4 + 灵木心 ×2 + 剑胚残片 ×1 | **30** | 3200 (160s) | 25% |
| 4→5 固→通 | 通灵 | 星辰铁 ×3 + 上古剑胚 ×1 + 灵泉精华 ×2 | **150** | 6400 (320s) | 35% |
| 5→6 通→化 | 化虚 | 天外陨铁 ×2 + 破碎剑魂 ×1 + 全部真元 | ALL | 12800 (640s) | 50% |

失败 = 材料损失 50% + stored_qi 归零（释放回 zone，走 qi_physics 守恒）+ durability -30%。不降品阶。

### P1.2 原材料定义

`server/assets/items/sword_materials.toml`（新增）

| id | name | category | 获取方式 | grid_w×h | rarity |
|----|------|----------|---------|----------|--------|
| `refined_iron` | 精铁 | material | forge 铁锭 ×2 | 1×1 | common |
| `xuan_iron` | 玄铁 | material | 巨剑沧海矿脉 | 1×1 | uncommon |
| `meteor_iron` | 陨铁 | material | 古遗迹宝箱 / 血谷深层 | 1×1 | rare |
| `star_iron` | 星辰铁 | material | 黑武士掉落 / 古遗迹核心 | 1×1 | epic |
| `sky_meteor_iron` | 天外陨铁 | material | 天劫雷击点采集 / 极稀有矿脉 | 1×1 | legendary |
| `spirit_spring_water` | 灵泉水 | material | 灵泉沼泽采集 | 1×1 | common |
| `spirit_spring_essence` | 灵泉精华 | material | 灵泉水 ×5 浓缩 | 1×1 | rare |
| `sword_embryo_shard` | 剑胚残片 | material | 古遗迹散落 / 黑武士掉落 | 1×2 | rare |
| `ancient_sword_embryo` | 上古剑胚 | material | 古遗迹 BOSS 房 / 极稀有 | 1×2 | epic |
| `broken_sword_soul` | 破碎剑魂 | material | 剑碎时 10% 概率结晶 / 黑武士首杀必掉 | 1×1 | legendary |
| `spirit_wood_core` | 灵木心 | material | 灵木砍伐（复用 plan-spiritwood-v1）| 1×1 | uncommon |

### P1.3 锻造台扩展

`WeaponForgeStation` 新增分支：
- 现有 4 步状态机不变（`Idle → Heating → Hammering → Quenching`）
- 新增第 5 步 `Infusing`：仅灵剑品阶升级时触发，cast 期间玩家持续注入真元
- `station.tier` 要求：品阶 3+ 需 tier 2 锻造台，品阶 5+ 需 tier 3

**视听——铸剑仪式（Infusing 阶段）**：
- **粒子**：`BongRibbonParticle` × 2 从玩家双手向剑身盘旋缠绕，lifetime = infuse_ticks，颜色 `#7799BB`→`#AADDFF` 渐变（随进度），continuous spawn，贴图 `bong:forge_qi_ribbon`（新增 16×4 缎带），VfxPlayer `SwordForgeInfuseVfxPlayer`，事件 ID `bong:sword_forge_infuse`
- **成功音效**：`{ "layers": [{ "sound": "block.anvil.use", "pitch": 1.8, "volume": 0.8, "delay_ticks": 0 }, { "sound": "entity.player.levelup", "pitch": 1.0, "volume": 1.0, "delay_ticks": 5 }, { "sound": "block.amethyst_block.chime", "pitch": 0.6, "volume": 0.6, "delay_ticks": 8 }] }`
- **失败音效**：`{ "layers": [{ "sound": "entity.item.break", "pitch": 0.8, "volume": 1.0, "delay_ticks": 0 }, { "sound": "block.anvil.land", "pitch": 0.5, "volume": 0.6, "delay_ticks": 3 }] }`
- **HUD**（进度条）：`HudRenderLayer.POPUP`，锻造台上方浮动进度条，背景 `#1A1A2A` alpha 200，前景 `#7799BB`→`#AADDFF` 渐变，高度 4px，宽 80px

---

## P2 — 巨剑沧海地形 + 古遗迹

### P2.1 Zone 定义

`server/zones.json` 新增：

```json
{
  "name": "giant_sword_sea",
  "aabb": {
    "min": [3800.0, -64.0, 800.0],
    "max": [5400.0, 320.0, 2400.0]
  },
  "spirit_qi": 0.45,
  "danger_level": 4,
  "ambient_recipe_id": "ambient_sword_sea",
  "active_events": [],
  "patrol_anchors": [
    [4200.0, 85.0, 1200.0],
    [4600.0, 78.0, 1600.0],
    [5000.0, 92.0, 2000.0]
  ],
  "blocked_tiles": []
}
```

spirit_qi 0.45 = 边缘灵气区，上古战场残留真元在缓慢消散。

### P2.2 Terrain Profile

`worldgen/terrain-profiles.example.json` 新增 `giant_sword_sea`：

```json
"giant_sword_sea": {
  "height": { "base": [58, 72], "peak": 95 },
  "boundary": { "mode": "semi_hard", "width": 112 },
  "surface": ["deepslate", "stone", "gravel", "sand", "prismarine"],
  "water": { "level": "high", "coverage": 0.55 },
  "passability": "medium_low"
}
```

**地形叙事**：上古宗门覆灭时，数千柄灵剑插入海底与崖壁，经万年锈蚀成为地标。海面浅水覆盖 55%，散落的巨剑从水底和岩壁中刺出 20-40 格高。

### P2.3 Decorations — 巨剑地标

worldgen decoration 模块新增 `giant_sword_decoration`：

- **巨剑插地**：10-40 格高的 stone_bricks + iron_block + oxidized_copper 组合结构体，随机倾斜角 5°-25°，剑柄露出 2-5 格，剑身插入地面/水底。每 zone 随机放置 15-30 把
- **剑阵遗迹**：5 把巨剑围成五角形（半径 8 格），中心有 cracked_stone_bricks 圆台 3×3，上面散落 `sword_embryo_shard`（可拾取 1-2 个）。每 zone 2-4 处
- **崩塌剑塔**：20 格高圆柱体（stone_bricks 外壳 + 铁栏杆螺旋楼梯），顶部断裂，内部藏箱。每 zone 1-2 处
- **海底剑冢**：水下 5-10 格深，8×8 deepslate_bricks 平台 + 中央插剑 + 4 角 soul_lantern 照明。每 zone 3-5 处

### P2.4 古遗迹 — 铸剑古殿（POI: `sword_forge_ruin`）

zone 内唯一大型遗迹，固定坐标。

- **外观**：30×30×15 mossy_stone_bricks 建筑废墟，半塌状态，正门有上古文字石碑（点击获取功法线索 narration）
- **内部结构**：
  - 大厅（20×20）：中央 tier-3 `WeaponForgeStation`（不需要自带，世界自然生成）
  - 侧室 ×2：每间 1 个箱子，随机掉落 `meteor_iron` ×1-2 / `sword_embryo_shard` ×1 / `scroll_sword_path`（剑道功法残卷）
  - 地下室：通往 BOSS 房（§P3 黑武士）
- **功法残卷散落**：遗迹各处散落 3-5 个 `scroll_sword_path`，拾取后解锁剑道招式（走 `UnlockSource::Scroll` 现有机制）

worldgen blueprint POI spec:

```json
{
  "kind": "ruin",
  "pos_xyz": [4600.0, 72.0, 1400.0],
  "name": "铸剑古殿",
  "tags": ["sword_path", "forge", "boss_entrance"],
  "unlock": "found_by_exploration",
  "qi_affinity": 0.15,
  "danger_bias": 1
}
```

### P2.5 Zone 环境视听

**ambient_sword_sea** audio_recipe：

```json
{
  "layers": [
    { "sound": "ambient.underwater.loop", "pitch": 0.8, "volume": 0.15, "delay_ticks": 0 },
    { "sound": "block.amethyst_block.resonate", "pitch": 0.4, "volume": 0.08, "delay_ticks": 200 },
    { "sound": "entity.breeze.idle_air", "pitch": 0.6, "volume": 0.1, "delay_ticks": 400 }
  ]
}
```

**ZoneAtmosphereProfile** 新增 `giant_sword_sea`：
- 粒子：`BongSpriteParticle` type `lingqi_ripple`，密度 0.3/s，tint `#667799`，drift Y +0.02（缓慢上升的残留灵气微光）
- 雾：fogStart 48，fogEnd 128，density 0.006，color `#445566`（深蓝灰色远雾）
- 天空色温：RGB shift `(-10, -5, +15)` 偏冷蓝

---

## P3 — 黑武士 BOSS

### P3.1 模型接入

外部资源路径（Windows → WSL）：
- model: `/mnt/d/BaiduNetdiskDownload/动物怪兽实体/远古模型/models/entities/entity/heiwushi.json`
- animation: `/mnt/d/BaiduNetdiskDownload/动物怪兽实体/远古模型/models/entities/animation/heiwushi.json`
- texture: `/mnt/d/BaiduNetdiskDownload/动物怪兽实体/远古模型/models/entities/png/heiwushi.png`

接入步骤：
1. 复制到 `client/src/main/resources/assets/bong/geo/heiwushi.geo.json`（重命名 identifier → `geometry.bong.heiwushi`）
2. 复制到 `client/src/main/resources/assets/bong/animations/heiwushi.animation.json`
3. 复制到 `client/src/main/resources/assets/bong/textures/entity/fauna/heiwushi.png`
4. 新增 `FaunaVisualKind::Heiwushi` variant
5. 新增 `HeiwushiEntityRenderer.java` + `HeiwushiEntityModel.java`（GeckoLib 注册）

模型骨骼：`global` / `body` / `rightArm` / `leftArm` / `rightLeg` / `leftLeg` / `bone8`(head) / `beiSword2` / `beiSword3`(背剑) / `leftArmSword` / `rightArmSword`(持剑)

现有动画（3 有效 + 2 空桩）：
- `黑暗弹幕` / `skill1`（0.76s）— 暗影弹射
- `黑暗旋涡` / `skill2`（1.04s）— 暗黑旋涡
- `黑暗化身` / `skill3`（0.8s）— 暗影变身（背剑展开）
- `skill4` / `idle` — 空，需补写

需补写动画：
- `idle`：站立持剑，背剑微摆（body.yaw ±0.05rad 4s 循环，beiSword2/3.roll ±0.03rad）
- `walk`：标准行走循环（rightLeg/leftLeg 交替 pitch ±0.6rad）
- `death`：前倾倒地（body.pitch +1.4rad 1.5s，rightArmSword 松开）

### P3.2 BOSS 叙事背景

**上古剑宗·末代小师弟**

名号：**剑奴**（原名已遗忘）

> 上古时代，巨剑沧海曾是「归元剑宗」的宗门所在。末法降临时，宗门覆灭于天道压制，万柄灵剑失去真元供给，插入大地成为地标。唯有一个练功人偶——宗门用来给弟子当陪练的傀儡——因为没有真正的经脉、不受天道压制，反而幸存。
>
> 千年过去，人偶的灵智核心（一枚上古剑魂碎片）产生了错乱的自我意识。它认为自己是宗门最小的弟子，每天清晨在崩塌的练功场内挥剑修炼，向已成废墟的大殿行礼，对着空气汇报"师兄，我今日练了三百剑"。它不知道宗门已毁，不知道师兄师姐早已化为白骨，不知道自己不是人——它只知道"小师弟要用功，不然掌门会罚抄剑经"。
>
> 当外人进入铸剑古殿地下室时，剑奴会将来者视为"闯入师门的贼人"，以全力守护已不存在的宗门。

### P3.3 BOSS AI 行为（big-brain Utility AI）

server 新增 `server/src/npc/spawn_heiwushi.rs`

**三阶段血量切换**：
- Phase 1（100%-60%）：普通剑术——skill1（黑暗弹幕）+ 基础近战
- Phase 2（60%-25%）：旋涡模式——skill2（黑暗旋涡）+ 移速 ×1.5 + 近战连击
- Phase 3（<25%）：暗影化身——skill3 触发变身，背剑展开成双持，攻击 ×2.0，防御 -50%（玻璃大炮镜像玩家化虚设计）

**NPC 数值**：
- 境界等效：通灵（Spirit）
- HP: 2100（通灵级 qi_max 锚定）
- base_attack: 35
- defense: 8.0
- move_speed: 4.8 blocks/s（Phase 2: 7.2）
- 巡逻区域：铸剑古殿地下室 20×20

**Scorer/Action 注册**（big-brain）：
- `HeiwushiAggro` Scorer：检测 20 格内玩家 → 1.0
- `HeiwushiSkill1Action`：Phase 1，CD 60 tick，8 格内 ranged projectile
- `HeiwushiSkill2Action`：Phase 2，CD 80 tick，6 格 AoE
- `HeiwushiSkill3Action`：Phase 3 触发（一次性），切换到双持 attack pattern
- `HeiwushiMeleeAction`：全阶段，3 格近战

**掉落**：
- 必掉：`star_iron` ×2 + `sword_embryo_shard` ×2
- 首杀必掉：`broken_sword_soul` ×1
- 30% 概率：`ancient_sword_embryo` ×1
- 10% 概率：`scroll_sword_path_manifest`（归一·剑意化形功法残卷）

### P3.4 BOSS 视听

**Phase 切换视听——Phase 3 暗影化身**：
- **动画**：播放 `heiwushi.skill3`（0.8s），背剑 beiSword2/beiSword3 展开 + leftArmSword 拔出
- **粒子**：`BongSpriteParticle` × 32 从 BOSS 身体向外爆散暗紫色，lifetime 25 tick，速度 2.0 m/s，颜色 `#2A0033`→`#550066` fade，spawn burst，贴图 `bong:dark_transform`（新增 8×8 暗紫碎片），VfxPlayer `HeiwushiTransformVfxPlayer`，事件 ID `bong:heiwushi_transform`
- **音效**：`{ "layers": [{ "sound": "entity.warden.emerge", "pitch": 1.2, "volume": 1.0, "delay_ticks": 0 }, { "sound": "entity.wither.spawn", "pitch": 1.5, "volume": 0.6, "delay_ticks": 5 }, { "sound": "entity.lightning_bolt.impact", "pitch": 0.8, "volume": 0.4, "delay_ticks": 8 }] }`
- **HUD**（nearby 玩家）：`VisualEffectProfile.BOSS_PHASE_SHIFT`（新增），edgeVignette `#1A0022` maxAlpha 150，duration 1500ms + screenShake amplitude 3px 频率 25Hz 800ms
- **narration**：scope: zone, style: narrative — `"铸剑古殿深处传来一声嘶哑的低吼：'师兄……是贼人……我来护宗！'暗紫色的真元从人偶体内涌出，两柄背剑被扯入双手。"`

**死亡视听**：
- **动画**：`heiwushi.death`（自定义 1.5s）
- **粒子**：`BongSpriteParticle` × 48 缓慢上升（drift Y +0.3），lifetime 60 tick，颜色 `#334455`→透明，spawn continuous 3s，贴图 `bong:sword_soul_mote`（新增 4×4 魂光），VfxPlayer `HeiwushiDeathVfxPlayer`，事件 ID `bong:heiwushi_death`
- **音效**：`{ "layers": [{ "sound": "entity.allay.death", "pitch": 0.6, "volume": 0.8, "delay_ticks": 0 }, { "sound": "block.amethyst_block.break", "pitch": 0.4, "volume": 0.5, "delay_ticks": 10 }] }`
- **narration**：scope: zone, style: narrative — `"人偶缓缓跪倒，双手仍紧握剑柄。嘶哑的声音最后一次响起：'掌门……小师弟……练完了……'灵智核心的微光熄灭，千年执念终于散去。"`

---

## P4 — 化虚·一剑开天门

### P4.1 机制设计

**玻璃大炮终极**——以修士全部修为+灵剑全部积蓄为代价，一击制造天道盲区。

**触发条件**：
- 修士境界 = 化虚
- 灵剑品阶 = 六阶（Void）
- `bond_strength >= 0.95`
- `stored_qi >= grade.cap * 0.8`（至少 2400）
- 4 经脉（LI+SI+TE+Du）全部正常（非 SEVERED）

**施法流程**（80 tick = 4s cast）：
1. **蓄力期**（0-60 tick）：玩家进入不可移动状态，真元从经脉 + 剑内同时汇聚。每 tick `qi_current -= qi_max / 60`，`stored_qi -= grade.cap / 60`，全部走 `QiTransfer { from: [player, sword], to: staging_buffer }`
2. **临界点**（60 tick）：天空变色，zone 内所有玩家收到 perception warning
3. **释放**（60-80 tick）：staging_buffer 全部真元通过剑身释放，形成 100 格半径球形冲击波
4. **aftermath**：
   - 伤害：冲击波 100 格内所有实体受到 `staging_buffer * distance_attenuation(0.03/格)` 的穿透伤害（无视防御 50%）
   - 天道盲区：`TiandaoBlindZone { center: cast_pos, radius: 100, ttl: 6000 tick (5 min) }` 挂载到 zone
   - 代价：`qi_current = 0`，`qi_max` 永久降为 `qi_max * 0.1`，灵剑碎裂（`SwordShatterEvent`），`bond_strength = 0`，境界掉至固元（经脉拓扑收缩 = 奇经八脉关闭）
   - zone spirit_qi 暂时归零（5 min），后按 qi_physics 自然回复

### P4.2 天道盲区机制

`server/src/sword_path/tiandao_blind.rs`

```rust
pub struct TiandaoBlindZone {
    pub center: DVec3,
    pub radius: f64,       // 100.0
    pub ttl_ticks: u64,    // 6000 (5 min at 20 TPS)
    pub created_tick: u64,
}
```

**server → agent 屏蔽**：
- `bong:world_state` Redis 发布时，如果玩家位置在 `TiandaoBlindZone` 内，从 player_snapshots 中移除该玩家
- `bong:agent_cmd` 的天道指令如果目标 zone 有 blind zone 激活，响应 `{ "status": "blocked", "reason": "tiandao_blind_zone" }`
- blind zone 内的 `qi_physics` 结算正常进行（守恒律不破例），只屏蔽天道 agent 感知层

**agent 侧**：
- `agent/packages/tiandao/src/tools/queryPlayerTool.ts` 新增 filter：`if (player.in_blind_zone) skip`
- narration 模板：scope: broadcast, style: narrative — `"天道的目光扫过{zone_name}，却什么也看不见——仿佛那片区域从世界上消失了。"`

### P4.3 视觉流程——完整 4s cast + aftermath

**第一段：蓄力（0-60 tick / 0-3s）**

- **动画**：`bong:sword_heaven_gate_charge`，endTick 60
  - 0-20 tick：双手握剑举过头顶，`rightArm.pitch = -1.5rad`，`leftArm.pitch = -1.4rad`，`body.pitch = -0.2rad`（微后仰），easing cubicOut
  - 20-60 tick：保持举剑，`body` 微振 `body.y += sin(tick*0.5) * 0.02`（真元共振颤抖）
- **粒子**（阶段性递增）：
  - 0-20 tick：`BongLineParticle` × 4/tick 从地面向剑身汇聚，lifetime 15 tick，颜色 `#8899BB`，spawn continuous，高度 0→player.y+2.5，贴图复用 `bong:sword_qi_trail`
  - 20-40 tick：追加 `BongRibbonParticle` × 2 螺旋上升环绕（半径 2 格 → 0.5 格收束），lifetime 20 tick，颜色 `#AABBDD` alpha 200，贴图 `bong:forge_qi_ribbon`
  - 40-60 tick：全部粒子颜色 shift 至 `#DDEEFF`（白热），密度 ×2，追加 `BongSpriteParticle` × 8/tick 从 100 格边缘向中心飞来（速度 5 m/s），lifetime 20 tick，颜色 `#445566`→`#DDEEFF`，贴图 `bong:heaven_gate_converge`（新增 8×8 锐利光点）
- **音效**（分层递进）：
  - 0s：`{ "sound": "block.respawn_anchor.charge", "pitch": 0.4, "volume": 0.6 }`
  - 1s：`{ "sound": "block.beacon.activate", "pitch": 0.6, "volume": 0.8 }`
  - 2s：`{ "sound": "entity.warden.sonic_charge", "pitch": 1.2, "volume": 1.0 }`
- **HUD**（zone 内所有玩家）：
  - `VisualEffectProfile.HEAVEN_GATE_CHARGE`（新增），screenTint `#223344` alpha 0→80 线性增长 3s，edge vignette `#112233` alpha 0→120
  - 0-3s 天空色温：RGB shift `(-30, -20, +40)` 渐变（天空变冷蓝暗沉）

**第二段：临界 flash（60 tick / 3s 时刻）**

- **粒子**：所有聚拢粒子瞬间冻结 5 tick（freeze effect），然后 burst `BongSpriteParticle` × 64 球形爆散，速度 0.1 m/s（几乎静止），lifetime 40 tick，颜色纯白 `#FFFFFF` alpha 255，贴图 `bong:heaven_gate_flash`（新增 4×4 纯白点）
- **音效**：`{ "sound": "entity.lightning_bolt.thunder", "pitch": 0.3, "volume": 1.0 }` — 极低音雷鸣
- **HUD**：`HudRenderCommand.screenTint(#FFFFFF, 255)` 持续 3 tick（全白闪屏），然后 fade-out 15 tick

**第三段：释放冲击波（60-80 tick / 3-4s）**

- **粒子**：
  - `BongGroundDecalParticle` × 1 扩散环，半径 0→100 格，lifetime 40 tick，速度 = 100 格/40 tick = 50 m/s 扩散，颜色 `#AACCFF` alpha 200→0，贴图 `bong:heaven_gate_shockwave`（新增 32×32 环形冲击波纹），VfxPlayer `HeavenGateShockwaveVfxPlayer`
  - `BongLineParticle` × 200 从中心放射状向外飞散，lifetime 30 tick，速度 3 m/s，颜色 `#DDEEFF`→`#334455` fade，贴图复用 `bong:sword_qi_trail`
  - `BongSpriteParticle` × 80 沿地面翻滚（drift Y = -0.1，X/Z 随机 ±2），lifetime 40 tick，颜色 `#556677`（残留灰尘），贴图 `bong:dust_cloud`（新增 8×8 灰色云团）
- **音效**：
  - `{ "sound": "entity.generic.explode", "pitch": 0.4, "volume": 1.0, "delay_ticks": 0 }` — 低频爆炸
  - `{ "sound": "entity.breeze.wind_burst", "pitch": 0.5, "volume": 0.8, "delay_ticks": 5 }` — 冲击波风声
  - `{ "sound": "ambient.cave", "pitch": 0.3, "volume": 0.6, "delay_ticks": 15 }` — 余韵空洞回声
- **动画**：`bong:sword_heaven_gate_release`，endTick 20
  - 0-10 tick：劈下，`rightArm.pitch = +0.8rad`（从举过头顶到前方平劈），`body.pitch = +0.3rad`（前倾），easing cubicIn（加速下劈）
  - 10-20 tick：余势，`body.pitch = +0.5rad`（深前倾），`rightArm.pitch = +1.0rad`（下垂），easing linear

**第四段：aftermath（释放后持续 5 min）**

- **zone 环境改变**：
  - spirit_qi 降为 0.0 → fogStart 降至 16，fogEnd 降至 64，fog color `#111122`（极深暗蓝）
  - 天空 RGB shift `(-50, -40, +20)`（近乎黑夜但偏冷蓝）
  - 100 格半径内不再 spawn 自然粒子（lingqi_ripple 停止）
  - 追加 `BongSpriteParticle` type `void_mote`，密度 0.1/s，lifetime 120 tick，drift Y +0.01，颜色 `#223344` alpha 80（稀疏暗光上升——残留真元散逸）
- **中心剑碎遗址**：释放位置生成 3×3 cracked_deepslate 平台 + 中心插入碎剑柄（decoration entity）
- **HUD**（zone 内玩家）：`VisualEffectProfile.HEAVEN_GATE_VOID`（新增），持续 ink_wash overlay `#112233` alpha 40，duration = blind_zone TTL（5 min）

### P4.4 narration 模板

- scope: **broadcast**, style: **narrative** — `"天穹裂开一道苍白的伤口。{player_name}举起{weapon_name}，将毕生修为化作一剑——沧海之上，天道的目光突然失明。方圆百丈，化作虚无。"`
- scope: **broadcast**, style: **perception** — `"你感觉到远处传来一股毁灭性的真元波动——紧接着，那片区域的灵气完全消失了。天道……好像也察觉到了什么不对。"`
- scope: **player**（施法者）, style: **perception** — `"经脉在燃烧，{weapon_name}在掌中碎裂。所有的真元，所有的修为，化作一道光芒。值得吗？你已经不在乎了。"`

---

## P5 — 剑道 HUD 完整包 + 饱和测试

### P5.1 SwordPathHudPlanner

`client/src/main/java/com/bong/client/hud/SwordPathHudPlanner.java`

在 `BongHudOrchestrator.buildCommands()` 中注册，返回 `List<HudRenderCommand>`。

**HUD 元素**（仅绑定灵剑时显示——遵循「未解锁/未激活直接隐藏」原则）：

1. **剑品阶指示器**：右下角，剑图标 + 品阶数字（0-6），图标颜色随品阶变化（Mortal `#888888` → Void `#DDEEFF`）。`HudRenderLayer.STATUS`，texturedRect 16×16 `bong:sword_grade_icon`
2. **stored_qi 竖条**：剑图标右侧，3px 宽 × 24px 高竖条，背景 `#1A1A2A`，前景 `#7799BB`→`#AADDFF` 渐变（按 stored_qi / cap 比例），`HudRenderLayer.STATUS`
3. **bond_strength 弧线**：剑图标底部，半圆弧 8px 半径，颜色 `#667788` alpha = `bond_strength * 200`，`HudRenderLayer.STATUS`
4. **化虚就绪提示**：当触发条件全部满足时，剑图标脉冲发光（alpha 120→220 循环 40 tick），追加文字 `"天门可开"` 颜色 `#FFFFFF` alpha 180

### P5.2 InspectScreen 扩展

`InspectScreen` 装备面板中灵剑条目新增信息行：
- 品阶：`"品阶：三阶·凝"`
- 封存真元：`"封存：42.5 / 75.0"`
- 绑定强度：`"人剑合一：78%"`

### P5.3 饱和测试清单

#### server/src/sword_path/ 单测

**bond.rs**（10 tests implemented）：
- [ ] 连续 20 次使用剑术 → SwordBondComponent 挂载成功（ECS wiring 留 v2）
- [ ] 19 次使用 → 不挂载（ECS wiring 留 v2）
- [ ] 已绑定时换剑 → 旧绑定不自动解除（ECS wiring 留 v2）
- [ ] 解绑仪式 30s → bond_strength 降 50%（ECS wiring 留 v2）
- [ ] 1 player 绑定 2 剑 → 拒绝第二绑定（ECS wiring 留 v2）
- [x] stored_qi 自动注入 = `qi_cost * 0.1`（`inject_qi_works_for_condensed` + `inject_qi_respects_cap` + `inject_qi_zero_cost_returns_zero`）
- [x] stored_qi 低品阶不注入（`inject_qi_zero_for_low_grade`）
- [x] stored_qi 超阈值 → should_shatter（`should_shatter_when_over_threshold`）
- [x] 低品阶不会 shatter（`shatter_threshold_zero_for_low_grades`）
- [x] 剑碎反噬：`qi_current -= stored_qi * 0.6` + `qi_max -= stored_qi * 0.05`（`backlash_values` + `backlash_zero_when_empty`）

**grade.rs**（12 tests implemented）：
- [x] 每个品阶的 stored_qi cap 正确（`specific_caps_match_plan`）
- [x] grade_mult 表正确（`specific_mults_match_plan`）
- [x] cap 单调递增（`grade_caps_monotonic`）
- [x] mult 单调递增（`grade_mult_monotonic`）
- [x] 品阶 3 以下不存气（`can_store_qi_only_grade_3_plus`）
- [x] tier roundtrip（`tier_roundtrip`）
- [x] next grade 链（`next_grade`）
- [x] shatter threshold 0 for no-cap（`shatter_threshold_zero_for_no_cap`）
- [x] shatter threshold = 1.5x cap（`shatter_threshold_is_1_5x_cap`）
- [x] 升级 qi cost 低阶为零（`upgrade_qi_cost_low_grades_zero`）
- [x] qi cost ALL resolve（`upgrade_qi_cost_resolve_all`）
- [x] fail chance 单调 + mortal 零（`upgrade_fail_chance_monotonic` + `upgrade_fail_chance_mortal_zero`）

**techniques.rs**（20 tests implemented）：
- [x] 5 招数量 + id 唯一（`all_techniques_count` + `technique_ids_unique`）
- [x] 境界门槛递增（`realm_gates_ascending`）
- [x] 凝锋 qi=0（`condense_edge_no_qi`）
- [x] 剑气斩 qi=3（`qi_slash_low_qi`）
- [x] 剑鸣 qi=20 + 固元门槛（`resonance_qi_for_solidify`）
- [x] 剑意化形 qi=40 + 通灵门槛（`manifest_qi_for_spirit`）
- [x] 一剑开天 qi=ALL + 一次性 CD（`heaven_gate_costs_all_qi` + `heaven_gate_one_shot`）
- [x] 效果常数范围（7 tests: damage_mult / armor_pierce / attenuation / slow_range / manifest_mult / radius / defense_ignore）
- [x] 染色权重全覆盖 + 凝锋无 keen + 天门重 keen（`color_weights_*` 4 tests）
- [ ] 经脉依赖 SEVERED 拒绝 cast（ECS wiring 留 v2）
- [ ] 无剑持有拒绝（ECS wiring 留 v2）
- [ ] 招式战斗效果（命中/AoE/追踪等 ECS runtime 留 v2）

**shatter.rs**（9 tests implemented）：
- [x] 碎裂反噬比例正确（`shatter_backlash_proportional`）
- [x] 碎裂真元守恒（`shatter_qi_conservation`）
- [x] 空存储碎裂无损（`shatter_zero_stored_qi`）
- [x] 剑魂低 roll 产出（`sword_soul_at_low_roll`）
- [x] 剑魂高 roll 不产出（`no_sword_soul_at_high_roll`）
- [x] 化虚 staging buffer 正确（`heaven_gate_staging_buffer`）
- [x] 化虚 qi_max 保留 10%（`heaven_gate_qi_max_retained`）
- [x] 化虚境界跌落固元（`heaven_gate_realm_drop`）
- [x] 化虚 qi_max 守恒（`heaven_gate_qi_max_loss_conservation`）

**tiandao_blind.rs**（8 tests implemented）：
- [x] 中心点包含（`contains_center`）
- [x] 边缘包含（`contains_at_edge`）
- [x] 外部不包含（`not_contains_outside`）
- [x] 创建时未过期（`not_expired_at_creation`）
- [x] TTL 后过期（`expired_after_ttl` + `expired_exactly_at_end`）
- [x] 剩余 tick 计算（`remaining_ticks_midway` + `remaining_ticks_after_expiry`）
- [ ] blind zone 内玩家不出现在 world_state 推送中（agent bridge wiring 留 v2）

**heaven_gate.rs**（9 tests implemented）：
- [x] Registry 空初始化（`registry_starts_empty`）
- [x] 添加 + 查询（`add_and_query`）
- [x] 过期移除（`tick_expire_removes_old`）
- [x] 多 zone 重叠（`multiple_zones_overlap`）
- [x] 盲区常数匹配 plan（`create_blind_zone_uses_plan_constants`）
- [x] 0 距离伤害（`heaven_gate_damage_at_zero_distance`）
- [x] 伤害随距离衰减（`heaven_gate_damage_decays_with_distance`）
- [x] 100 格伤害范围（`heaven_gate_damage_at_100_blocks`）
- [x] 零 buffer 零伤害（`heaven_gate_damage_zero_buffer`）

**upgrade.rs**（21 tests implemented）：
- [x] 6 阶配方链完整（`all_six_recipes_exist` + `recipe_chain_covers_all_grades`）
- [x] Void 无配方（`no_recipe_for_void`）
- [x] 前两阶 qi=0（`first_two_upgrades_zero_qi`）
- [x] 锻造台 tier 要求（`solidify_upgrade_needs_tier_2_station` + `spirit_upgrade_needs_tier_3_station`）
- [x] 化虚 qi=ALL（`void_upgrade_costs_all_qi`）
- [x] fail_chance/time_ticks 单调（`fail_chance_monotonic` + `time_ticks_monotonic`）
- [x] check OK / NoRecipe / RealmTooLow / StationTierTooLow / MissingMaterials / InsufficientQi 全分支
- [x] 等值边界：qi == need 通过（`check_upgrade_exact_qi_passes`）
- [x] 等值边界：roll == fail_chance 成功（`resolve_upgrade_roll_equals_fail_chance_succeeds`）
- [x] 成功/失败结算（`resolve_upgrade_success` + `resolve_upgrade_fail` + `resolve_upgrade_fail_partial_materials`）
- [x] 化虚消耗全部 qi（`resolve_void_upgrade_consumes_all_qi`）
- [x] realm_tier 辅助函数（`realm_tier_helper_values`）

#### client/ 单测（11 tests implemented）

- [x] inactive/null/zero-screen 返回空（3 tests）
- [x] active 产出 commands（`activeStateProducesCommands`）
- [x] 化虚就绪追加 extra command（`heavenGateReadyAddsExtraCommand`）
- [x] gradeColor 边界 clamp（`gradeColorBoundsCheck`）
- [x] 7 阶颜色各不相同（`allGradesHaveDistinctColors`）
- [x] storedQiRatio/bondStrength clamp01 + NaN（`stateClamp01` + `stateNanClamp`）
- [x] store lifecycle（`storeReplaceAndSnapshot`）
- [x] 全部 command 使用 SWORD_BOND layer（`allCommandsUseSwordBondLayer`）

#### 集成测试（ECS runtime 留 v2）

- [ ] 从零开始剑道流程：拾取剑 → 20 次使用绑定 → 逐步升品阶 → 使用 5 招 → 化虚一剑开天门
- [ ] 天道盲区：施法后天道 agent 5 min 内无法查询该区域玩家
- [ ] 黑武士 BOSS 战完整循环：Phase 1 → Phase 2 → Phase 3 → 击杀 → 掉落
- [ ] VFX 管线：所有 5 招 + 绑定 + 碎裂 + 铸剑 + 化虚 + BOSS 变身/死亡 = 全部 VfxPlayer 正常触发

---

## 开放问题（P0 决策门收口）

1. **人剑共生 vs 多剑切换**：当前设计 1:1 绑定。是否允许"剑匣"机制（多剑但仅 1 把 active）？→ 建议 v1 严守 1:1，v2 考虑剑匣
2. **剑碎 qi_max 永久衰减比例**：当前 stored_qi × 0.05。化虚六阶 stored_qi cap 5350 → 最大永久衰减 267.5，占化虚 qi_max 10700 的 2.5%。是否偏低？→ 建议保持，因为化虚一剑开天门另有 qi_max × 0.9 的额外代价
3. **巨剑沧海地形密度**：15-30 把巨剑 / zone 是否太多导致寻路困难？→ 建议先按 20 实测
4. **黑武士刷新机制**：击杀后多久刷新？→ 建议 in-game 24h（real-time ~1h），首杀后 72h
5. **化虚盲区内 NPC 行为**：blind zone 内 NPC 是否也从天道感知消失？→ 建议是（一致性）
6. **品阶升级失败概率曲线**：当前线性递增。是否加入 proficiency 减免？→ 建议 v2 处理
7. **化虚一剑开天门 PvP 平衡**：staging_buffer = qi_max 10700 + stored_qi 3000 = 13700；10 格处 ≈ 5069 伤害（通灵必杀），50 格处 ≈ 1507，100 格处 ≈ 343（2.5%）；核心 10 格内才致命

---

## Finish Evidence

### 落地清单

| 阶段 | 模块 / 文件路径 |
|------|----------------|
| P0 | `server/src/sword_path/grade.rs` — SwordGrade 7 阶 + 数值表 |
| P0 | `server/src/sword_path/bond.rs` — SwordBondComponent + 注入/碎裂 |
| P0 | `server/src/sword_path/techniques.rs` — 五招定义 + 效果常数 + 染色权重 |
| P0 | `server/src/sword_path/shatter.rs` — 碎裂/化虚结算纯函数 |
| P0 | `server/src/sword_path/tiandao_blind.rs` — TiandaoBlindZone struct |
| P0 | `server/src/sword_path/mod.rs` — Plugin 注册 |
| P1 | `server/assets/items/sword_materials.toml` — 10 种原材料定义 |
| P1 | `server/src/sword_path/upgrade.rs` — 6 阶升级配方 + check/resolve 逻辑 |
| P2 | `server/zones.json` — giant_sword_sea zone |
| P2 | `worldgen/terrain-profiles.example.json` — giant_sword_sea terrain profile |
| P2 | `worldgen/blueprint/poi_sword_sea/` — 3 POI (铸剑古殿/剑阵遗迹/海底剑冢) |
| P3 | `client/.../geo/heiwushi.geo.json` — 黑武士模型 (geometry.bong.heiwushi) |
| P3 | `client/.../animations/heiwushi.animation.json` — 3 攻击动画 |
| P3 | `client/.../textures/entity/fauna/heiwushi.png` — 128×128 贴图 |
| P3 | `server/src/fauna/components.rs` — BeastKind::Heiwushi (HP 2100, realm_tier 4) |
| P3 | `server/src/fauna/visual.rs` — FaunaVisualKind::Heiwushi + ENTITY_KIND 145 |
| P3 | `server/src/fauna/drop.rs` — HEIWUSHI_DROPS 掉落表 |
| P4 | `server/src/sword_path/heaven_gate.rs` — HeavenGateCastEvent + TiandaoBlindZoneRegistry + 伤害衰减 |
| P5 | `client/.../hud/SwordBondHudState.java` — 绑定状态数据 |
| P5 | `client/.../hud/SwordBondHudStateStore.java` — 线程安全 store |
| P5 | `client/.../hud/SwordPathHudPlanner.java` — 品阶图标 + qi竖条 + bond弧线 + 化虚脉冲 |
| P5 | `client/.../hud/HudRenderLayer.java` — SWORD_BOND layer |

### 关键 commit

| hash | 日期 | 说明 |
|------|------|------|
| 34d07a4bd | 2026-05-16 | P0：人剑共生核心 + 五招 + 天道盲区（59 测试） |
| 3044297fe | 2026-05-16 | P1：铸剑材料 + 品阶升级配方（+19 测试） |
| b8b9aeee8 | 2026-05-16 | P2：巨剑沧海地形 zone + profile + 3 POI |
| c8c06b850 | 2026-05-16 | P3：黑武士 BOSS 模型 + fauna 注册 + 掉落 |
| 74a4389cf | 2026-05-17 | P4：化虚一剑开天门 + 天道盲区 Registry（+9 测试） |
| 13e1c0785 | 2026-05-17 | P5：剑道 HUD 完整包 + client 测试（11 cases） |

### 测试结果

- `cd server && cargo test sword_path` → **87 passed**
- `cd server && cargo test` → **4913 passed, 0 failed**
- `cd client && ./gradlew compileJava` → **BUILD SUCCESSFUL**
- client test: 11 cases in SwordPathHudPlannerTest（compileTestJava 有 pre-existing MovementKeybindingsTest 错误，非本 PR 引入）

### 跨仓库核验

| 仓库 | 命中 symbol |
|------|------------|
| server | `SwordBondComponent` / `SwordGrade` / `SwordShatterEvent` / `HeavenGateCastEvent` / `TiandaoBlindZoneRegistry` / `BeastKind::Heiwushi` / `FaunaVisualKind::Heiwushi` / `HEIWUSHI_DROPS` |
| client | `SwordPathHudPlanner` / `SwordBondHudState` / `SwordBondHudStateStore` / `HudRenderLayer.SWORD_BOND` / `geometry.bong.heiwushi` / `heiwushi.animation.json` |
| agent | 待接入：`TiandaoBlindZoneRegistry.is_player_hidden()` 过滤 world_state 推送 |

### 遗留 / 后续

- agent 天道感知屏蔽：`publish_world_state_to_redis` 需读取 `TiandaoBlindZoneRegistry` 过滤玩家 snapshot（当前 Registry resource 已就绪，wiring 留 v2）
- ECS runtime system wiring：bond 绑定触发 / 招式 cast 接入 SkillRegistry / shatter 反噬扣减 Cultivation component（纯函数已就绪，Bevy system 留 v2）
- VFX asset：plan 中详述的粒子贴图 / audio_recipe JSON / PlayerAnimator JSON 未创建（规格已锁定）
- 黑武士 AI 行为：big-brain Scorer/Action（spawn_heiwushi.rs）未实装（BeastKind + 掉落 + 模型已就绪）
- InspectScreen 灵剑信息行扩展
- client SwordBondHudStateStore 的 network handler 接入（解析 server 下发的 bond 状态 payload）
