# Bong · plan-zhenfa-content-v1 · 凡阵三式——警示/爆炸/缓速

地师流派的第一批可制作凡阵物品。三种基础阵法覆盖侦察、杀伤、控制三个战术位：警示阵（埋地下竖向感知 3 格）、爆炸阵（水平同层，可用方块遮蔽）、缓阵（埋地下竖向减速 3 格）。凡阵 = 任何修士都能用的低阶版本，不需要地师专精。

**物理推演**：
- 低能量效果（感知/场）的真元线极细（qi < 0.05），能穿透 1-3 格固体方块——类似无线电穿墙
- 高能量效果（爆炸）的真元体密度高，被固体方块吸收/衰减严重——必须同层无遮挡才能命中目标
- 所有阵法的真元会随时间逸散（shelflife Decay 路径）——末法残土不存在永久陷阱

**世界观锚点**：
- `worldview.md §五` 地师/阵法流：真元封入环境方块做成诡雷，唯一能在灵龛周围建防御体系的流派
- `worldview.md §五` 地师劣势：无人上套时预埋真元几小时后随载体朽坏白白流失
- `worldview.md §八` 灵物密度阈值：阵法真元浓度高了天道会注意（分仓逼迫同样适用于阵法网络）
- `worldview.md §十五` 万物皆有成本：埋阵 = 投入真元 + 时间 + 材料，空套 = 全亏

**前置依赖**：
- `plan-zhenfa-v1` ✅ → 阵法核心系统（TrapNode / TrapTrigger / 识色法 / 预埋 API）
- `plan-zhenfa-v2` ✅ → 阵法五招 + 地师专精等级 + 藏阵/破阵顿悟路径
- `plan-craft-v1` ✅ → 手搓配方框架（CraftRegistry / CraftSession）
- `plan-qi-physics-v1` ✅ → 真元逸散公式（qi_excretion）/ 守恒账本
- `plan-shelflife-v1` ✅ → Decay 路径（半衰期逸散）
- `plan-mineral-v1` ✅ → 矿物材料（铜粉/灵石碎片）
- `plan-botany-v1` ✅ → 灵草材料（植物覆盖伪装）
- `plan-perception-v1.1` ✅ → 神识感知系统（阵法被发现的判定）

**反向被依赖**：
- `plan-niche-defense-v1` ✅ → 灵龛防御已用 TrapNode，本 plan 的凡阵是其低阶替代品
- `plan-sou-da-che-v1` ⬜ active → 搜打撤循环中"撤"的掩护手段
- `plan-pvp-encounter-v1` ⬜ active → 遭遇博弈中的预埋伏击选项

---

## 接入面 Checklist

- **进料**：`zhenfa::TrapNode`（已有阵法节点 component）/ `craft::CraftRegistry`（配方注册）/ `qi_physics::excretion::qi_excretion`（逸散计算）/ `shelflife::DecayProfile`（半衰期）/ `perception::SpiritualSense`（神识感知判定）/ `inventory::ItemRegistry`（物品模板）/ `cultivation::Cultivation`（真元池 + 境界）
- **出料**：3 种凡阵物品模板（`warning_trap` / `blast_trap` / `slow_trap`）/ 3 个 craft 配方 / 放置交互（右键地面/方块侧面）/ 触发系统（竖向 3 格 / 水平同层）/ 逸散 tick / 拆除机制 / 3 种阵法 icon（gen.py）
- **共享类型 / event**：复用 `TrapNode` component（不新建）/ 复用 `TrapTriggeredEvent`（不新建）/ 新增 `TrapKind` enum 变体（`Warning` / `Blast` / `Slow`）/ 新增 `TrapPlacedEvent` / 新增 `TrapDecayedEvent`
- **跨仓库契约**：
  - server：`server/src/zhenfa/trap_content.rs`（新文件，3 种凡阵的放置/触发/逸散逻辑）
  - client：放置动画 + 触发 VFX + 警示阵通知 HUD
  - agent/schema：同步 `ZhenfaKindV1` 三种凡阵 kind、`ZhenfaTargetFaceV1` 与 `ZhenfaPlaceRequestV1.target_face`；天道运行时无新增干预逻辑
- **worldview 锚点**：§五 地师阵法流 + §八 灵物密度阈值 + §十五 万物皆有成本
- **qi_physics 锚点**：阵法逸散走 `qi_excretion(initial_qi, ContainerKind::EmbeddedTrap, elapsed, env)`；爆炸阵触发走 `qi_release_to_zone(blast_qi, region, env)`；不自定衰减公式

---

## §0 设计轴心

- [x] **三阵三定位**：警示 = 侦察（知道有人来了）/ 爆炸 = 杀伤（打人一下）/ 缓速 = 控制（让人跑不掉）。三者配合形成"发现→减速→击杀"战术链
- [x] **埋设规则的物理区分**：
  - 警示阵/缓阵 = 低能量，可埋地下，竖向穿透 3 格感知/作用
  - 爆炸阵 = 高能量，必须水平同层放置，但可用方块视觉遮蔽（墙角/草丛/残灰堆旁）
- [x] **隐蔽 vs 威力取舍**：真元浓度越高 → 越容易被神识发现。地师专精的 ZhenfaConcealment 顿悟降低发现率
- [x] **时间是敌人**：所有凡阵都有半衰期，真元逐渐逸散 → 失效。高投入的爆炸阵衰减最快（密度高 = 逸散快）
- [x] **任何人可用，地师更强**：凡阵不锁流派。但地师专精等级（plan-zhenfa-v2）给加成：持续时间 ×1.5 / 被发现概率 ×0.5 / 触发范围 +1 格
- [x] **天道密度阈值**：同一 chunk 内阵法总真元 > `QI_DENSITY_GAZE_THRESHOLD`（0.85）→ 天道注视 → 该 chunk 灵气强制归零。防止满地铺阵

---

## §1 三种凡阵规格

### 警示阵（warning_trap）

| 维度 | 规格 |
|------|------|
| 放置方式 | 右键地面 → 嵌入脚下方块内部（不可见） |
| 感知方向 | **竖向上方 3 格**（柱形检测区域，半径 1.5 格） |
| 触发条件 | 检测到放置者以外的真元体（修士/NPC/异变兽，qi > 0） |
| 效果 | 放置者收到 HUD 通知：方向箭头 + 距离（无论放置者在多远） |
| 触发次数 | 无限次（每次检测到新目标都通知，间隔 ≥ 5s 防刷屏） |
| 真元消耗 | 放置时 2 qi（从放置者 cultivation.qi_current 扣） |
| 半衰期 | 8h（in-game time） |
| 隐蔽性 | 极高（qi 仅 0.02，低于引气感知阈值 0.1） |
| 被发现条件 | 凝脉+ 且主动施神识扫描 1 格范围内（概率 30%）；地师破阵顿悟 → 100% |
| 外观 | 放置后方块无变化（完全不可见）；放置者自己能看到微弱蓝色粒子标记 |

### 爆炸阵（blast_trap）

| 维度 | 规格 |
|------|------|
| 放置方式 | 右键方块**侧面或顶面** → 贴附在该方块表面（**水平同层**） |
| 感知方向 | **水平同层 1.5 格半径**（不穿透方块——被方块挡住的方向不触发） |
| 触发条件 | 放置者以外的真元体进入水平 1.5 格内 + 无方块遮挡（line-of-sight） |
| 效果 | 真元爆发 → `wound_damage`（目标最近部位 LACERATION）+ `contam`（污染注入）+ 击退 2 格 |
| 触发次数 | **1 次**（触发后载体碎裂消失） |
| 真元消耗 | 放置时 15-30 qi（滑块选择投入量，投入越多伤害越高） |
| 半衰期 | 2h（qi 密度高，逸散快） |
| 隐蔽性 | **低**（qi 0.15-0.30，引气即可感知到"这里有东西"） |
| 遮蔽方式 | 贴附在**方块背面/墙角内侧/草丛方块旁** → 视觉遮蔽（玩家看不到但 qi 仍可被感知）。最佳放置：墙角转角处 / 门后 / 残灰堆旁 |
| 外观 | 方块表面有微弱红色纹路（近距离 2 格内可见）；被遮蔽方块挡住视线时看不到纹路 |
| 伤害公式 | `damage = sealed_qi × 0.6`（qi=15 → damage=9 / qi=30 → damage=18）。对比 bone_sword 攻击 10——满投入的爆炸阵 ≈ 两刀 |

### 缓阵（slow_trap）

| 维度 | 规格 |
|------|------|
| 放置方式 | 右键地面 → 嵌入脚下方块内部（不可见，同警示阵） |
| 感知方向 | **竖向上方 3 格**（柱形区域，半径 2 格） |
| 触发条件 | 放置者以外的真元体进入柱形区域 |
| 效果 | 目标进入后：移速 -50% + 真元回复暂停，持续 3s（离开区域后 1s 恢复） |
| 触发次数 | **3 次**（每次触发消耗 1/3 封存真元，3 次后耗尽消失） |
| 真元消耗 | 放置时 8 qi |
| 半衰期 | 4h |
| 隐蔽性 | 中等（qi 0.08，引气感知阈值边缘——引气 50% 概率感知，凝脉+ 必然感知） |
| 被发现条件 | 引气 50% / 凝脉 100%（主动神识扫描 2 格内）；地师破阵顿悟 → 3 格内自动标记 |
| 外观 | 放置后方块无变化（不可见）；触发时地面泛起淡蓝色涟漪（目标和旁观者可见 = 暴露了阵法位置） |

---

## §2 隐蔽与反制

### 隐蔽手段（放置者视角）

| 手段 | 适用阵法 | 效果 | 代价 |
|------|---------|------|------|
| **残灰环境** | 全部 | 在残灰方块区域放置 → 被发现概率 -20%（背景噪声高） | 残灰区资源少，值得守吗？ |
| **植物覆盖** | 警示/缓阵 | 在阵上方种一株灵草 → 灵草 qi 遮蔽阵法 qi → 被发现概率 -30% | 灵草有生长周期（2h），且灵草本身吸引采集者（可能是好事） |
| **方块遮蔽** | 爆炸阵 | 贴附在方块背面 → 视觉不可见 → 但 qi 仍可被感知 | 方块遮蔽 = 触发方向受限（只有从正面走来才触发） |
| **深埋** | 警示/缓阵 | 埋入 2-3 格深 → qi 信号衰减（被发现距离 -2 格） | 穿透衰减：效果到达地面时延迟 0.15s / 范围略缩 |
| **骨币诱饵** | 爆炸阵 | 在阵旁丢 1-2 枚骨币 → 吸引贪婪者弯腰 → 进入触发范围 | 骨币有半衰期（shelflife），放久了贬值 = 没人捡 |
| **连锁诱导** | 全部 | 放一个故意明显的假阵（高 qi 让人发现）→ 真阵在绕路方向 | 两个阵的材料成本 × 2 |

### 反制手段（目标视角）

| 手段 | 谁能用 | 效果 |
|------|--------|------|
| **神识扫描** | 凝脉+（主动施放，消耗 3 qi，范围 5 格） | 检测到阵法 → 位置高亮 3s |
| **地师破阵** | 拥有 ZhenfaDisenchant 顿悟的修士 | 被动：3 格内阵法自动标记 + 可主动拆除（右键长按 2s → 回收 30% 真元） |
| **高速通过** | 任何人 | 缓阵触发有 0.3s 延迟 → 疾跑 + 跳跃可能在减速生效前离开范围 |
| **牺牲品探路** | 有 NPC 跟班 or 扔骨币引怪 | 让别的实体先触发爆炸阵 → 安全通过（但暴露了你知道这里有阵） |
| **负灵域干扰** | 在负灵域边缘 | 负压加速阵法真元逸散 → 半衰期 ×0.5（负灵域里阵法很快失效） |

---

## §3 制作配方

所有凡阵通过 `plan-craft-v1` ✅ 的手搓系统制作（inventory 内，无需方块/站）。

| 阵法 | 材料 | 制作时间 | 解锁方式 |
|------|------|---------|---------|
| 警示阵 ×3 | 铜粉 ×1 + 残灰块 ×2 | 30s | 默认已知（新手即可制作） |
| 爆炸阵 ×1 | 异变兽骨碎片 ×1 + 灵石碎片 ×1 + 铜粉 ×2 | 60s | 残卷"爆阵符箓"（散修 NPC 交易 or 道伥掉落） |
| 缓阵 ×2 | 灵草（任意）×1 + 铜粉 ×1 + 残灰块 ×1 | 45s | 残卷"缓阵符箓"（散修 NPC 交易） |

**产出单位**：警示阵一次做 3 个（便宜量大，用来铺网）/ 爆炸阵一次 1 个（贵且消耗大）/ 缓阵一次 2 个（配合使用）。

**配方注册**：加入 `CraftRegistry`，category = `TrapCraft`。

---

## §4 放置交互

### 放置流程

1. 玩家从 hotbar 选中阵法物品
2. 对地面/方块右键 → 放置预览（半透明轮廓 + 触发范围线框）
3. 确认放置 → 扣真元（从 `cultivation.qi_current`）→ 阵法嵌入/贴附
4. 放置动画：手掌按地 0.5s（警示/缓阵）/ 手掌贴墙 0.5s（爆炸阵）
5. 放置粒子：微弱蓝色/红色灵纹闪烁 0.3s 后消失

### 竖向嵌入（警示阵 / 缓阵）

- 嵌入放置点方块内部（y 坐标 = 放置点 y）
- 感知柱形区域：以放置点为底，向上延伸 3 格，半径 1.5/2 格
- 可叠加深埋：如果玩家站在 y=65 的方块上放置，阵法在 y=65 → 检测 y=65~68
- 如果玩家挖坑到 y=62 再放，阵法在 y=62 → 检测 y=62~65（地表以下 3 格仍能感知到地表行人）

### 水平贴附（爆炸阵）

- 贴附在右键目标方块的点击面上
- 触发范围：以贴附点为中心，水平半径 1.5 格，高度 ±1 格
- **line-of-sight 检查**：贴附点到目标之间如果有固体方块 → 不触发（爆炸被方块吸收）
- 最佳放置位：
  - 走廊墙角内侧（转角时目标进入 LOS）
  - 门框内侧（开门进入时触发）
  - 残灰堆旁地面（视觉混淆，但 LOS 无遮挡）
  - 草丛方块后方（草丛不算固体方块 → LOS 不被阻断 → 视觉遮蔽但仍可触发）

---

## §5 逸散与失效

所有凡阵走 `qi_excretion()` 逸散：

```
current_qi = initial_qi × 0.5^(elapsed / half_life)
当 current_qi < threshold → 阵法失效消失
```

| 阵法 | 半衰期 | 失效阈值 | 最长存活（从满到失效） |
|------|--------|---------|-------------------|
| 警示阵 | 8h | 0.005 | ~50h（极低消耗，活很久） |
| 爆炸阵 | 2h | 1.0（低于此伤害太弱，自动失效） | ~8h（4 个半衰期） |
| 缓阵 | 4h | 0.5（低于此减速不够） | ~12h |

**环境修正**：
- 馈赠区（zone_qi > 0.3）：半衰期 ×1.2（环境真元补充减缓逸散）
- 死域（zone_qi = 0）：半衰期 ×0.8（无补充，逸散加速）
- 负灵域（zone_qi < 0）：半衰期 ×0.3（负压猛抽，阵法快速失效）

**qi_physics 守恒**：阵法逸散的真元回归 zone（`qi_release_to_zone`）。爆炸阵触发时释放的真元也回归 zone（不凭空消失）。

---

## §6 天道密度阈值约束

同一 chunk（16×16 格）内所有阵法的**总封存真元** > `QI_DENSITY_GAZE_THRESHOLD`（0.85）→ 天道注视 → 该 chunk 灵气强制归零 + 全部阵法加速逸散（半衰期 ×0.1）。

约束计算：
- 警示阵 2qi × N + 爆炸阵 30qi × M + 缓阵 8qi × K < 0.85
- 实际上：同一 chunk 最多放 ~42 个警示阵 or ~2 个满投入爆炸阵 or ~10 个缓阵
- 混合放置要自己算总量

**设计意图**：防止"地雷田"——满地铺阵不是策略，是滥用。真正的地师靠的是**选位精准**，不是数量暴力。

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | 3 种凡阵物品模板（TOML）+ craft 配方注册 + `TrapKind` enum 扩展 + GUI icon 生成 | ✅ 2026-05-12 NZST |
| P1 | 放置交互（右键 → 预览 → 确认 → 嵌入/贴附）+ 放置动画/粒子 + 真元扣除 | ✅ 2026-05-12 NZST |
| P2 | 触发系统（竖向柱形检测 / 水平 LOS 检测）+ 效果应用（通知/伤害/减速）| ✅ 2026-05-12 NZST |
| P3 | 逸散 tick + 失效移除 + 天道密度阈值检查 + 环境修正 | ✅ 2026-05-12 NZST |
| P4 | 隐蔽性判定（感知 vs 阵法 qi + 环境 + 地师专精加成）+ 拆除机制 | ✅ 2026-05-12 NZST |
| P5 | Client 视觉（放置者标记粒子 / 触发 VFX / 警示 HUD 通知 / 神识扫描高亮） | ✅ 2026-05-12 NZST |
| P6 | 饱和化测试（3 阵 × 放置位 × 触发条件 × 逸散 × 隐蔽 × 天道阈值） | ✅ 2026-05-12 NZST |

---

## P0 — 物品 + 配方 + icon ✅ 2026-05-12 NZST

### 交付物

1. **TOML 物品定义**（`server/assets/items/traps.toml`，新文件）

   ```toml
   [[item]]
   id = "warning_trap"
   name = "警示符"
   category = "trap"
   grid_w = 1
   grid_h = 1
   base_weight = 0.1
   rarity = "common"
   spirit_quality_initial = 1.0
   description = "埋入地下后感知上方三格内的真元体。触发时向放置者传递方向与距离。"

   [[item]]
   id = "blast_trap"
   name = "爆阵符"
   category = "trap"
   grid_w = 1
   grid_h = 1
   base_weight = 0.3
   rarity = "uncommon"
   spirit_quality_initial = 1.0
   description = "贴附在方块表面，水平感知来者后引爆封存真元。一次性。"

   [[item]]
   id = "slow_trap"
   name = "缓阵符"
   category = "trap"
   grid_w = 1
   grid_h = 1
   base_weight = 0.2
   rarity = "common"
   spirit_quality_initial = 1.0
   description = "埋入地下后释放负压场减速上方来者。三次触发后耗尽。"
   ```

2. **Craft 配方注册**（`server/assets/recipes/traps.toml` 或加入现有 recipes 文件）

   ```toml
   [[recipe]]
   id = "craft_warning_trap"
   output = "warning_trap"
   output_count = 3
   category = "TrapCraft"
   duration_sec = 30
   unlock = "default"
   ingredients = [
     { item = "copper_dust", count = 1 },
     { item = "ash_block", count = 2 },
   ]

   [[recipe]]
   id = "craft_blast_trap"
   output = "blast_trap"
   output_count = 1
   category = "TrapCraft"
   duration_sec = 60
   unlock = "scroll:blast_trap_talisman"
   ingredients = [
     { item = "mutant_bone_shard", count = 1 },
     { item = "spirit_stone_shard", count = 1 },
     { item = "copper_dust", count = 2 },
   ]

   [[recipe]]
   id = "craft_slow_trap"
   output = "slow_trap"
   output_count = 2
   category = "TrapCraft"
   duration_sec = 45
   unlock = "scroll:slow_trap_talisman"
   ingredients = [
     { item = "herb_any", count = 1 },
     { item = "copper_dust", count = 1 },
     { item = "ash_block", count = 1 },
   ]
   ```

3. **`TrapKind` enum 扩展**（`server/src/zhenfa/mod.rs` 或 `trap_content.rs`）

   ```rust
   pub enum TrapKind {
       Warning,   // 竖向感知 3 格，无限次通知
       Blast,     // 水平同层 LOS，一次性爆炸
       Slow,      // 竖向感知 3 格，3 次减速
   }
   ```

4. **GUI icon 生成**

   ```bash
   python scripts/images/gen.py \
     "a small talisman paper with faint blue rune, folded into triangle, detection ward, xianxia style" \
     --name warning_trap --style item --transparent \
     --out client/src/main/resources/assets/bong-client/textures/gui/items/

   python scripts/images/gen.py \
     "a small talisman paper with angry red explosive rune, edges charred, blast ward, xianxia style" \
     --name blast_trap --style item --transparent \
     --out client/src/main/resources/assets/bong-client/textures/gui/items/

   python scripts/images/gen.py \
     "a small talisman paper with swirling cyan slow rune, frost-touched edges, binding ward, xianxia style" \
     --name slow_trap --style item --transparent \
     --out client/src/main/resources/assets/bong-client/textures/gui/items/
   ```

### 验收抓手

- `cargo test inventory::tests` — TOML 解析 3 种阵法物品成功
- `cargo test craft::tests` — 3 个配方注册正确（材料/产出/解锁条件）
- gen.py 产出 3 张 icon → 客户端背包可见
- `TrapKind` enum 3 变体存在

---

## P1 — 放置交互 ✅ 2026-05-12 NZST

### 交付物

1. **放置 intent 处理**（`server/src/zhenfa/trap_content.rs`）

   - 玩家手持阵法物品 + 右键 → `PlaceTrapIntent { trap_kind, target_pos, target_face }`
   - 校验：真元够不够 / 目标位置是否合法 / chunk 密度阈值是否超
   - 扣真元 + 消耗物品 → 创建 `TrapNode` entity

2. **放置规则**

   - 警示阵/缓阵：target_face 必须是 TOP（地面） → 嵌入该方块
   - 爆炸阵：target_face 可以是任意面（TOP/BOTTOM/NORTH/SOUTH/EAST/WEST）→ 贴附该面
   - 同一方块不能放两个阵法

3. **Client 放置预览**

   - 手持阵法物品时，crosshair 指向的方块显示半透明轮廓 + 触发范围线框（蓝色/红色）
   - 竖向阵：显示向上延伸的半透明柱
   - 水平阵：显示水平圆形范围

4. **放置动画 + 粒子**

   - 警示/缓阵：手掌按地 0.5s → 地面微弱蓝色灵纹闪烁 → 消失
   - 爆炸阵：手掌贴面 0.5s → 表面微弱红色灵纹闪烁 → 淡化为纹路

### 验收抓手

- 测试：`zhenfa::trap_content::tests::place_warning_deducts_qi`（放置扣 2 qi）
- 测试：`zhenfa::trap_content::tests::place_blast_requires_side_face`（爆炸阵不能放地下）
- 测试：`zhenfa::trap_content::tests::place_rejected_insufficient_qi`
- 测试：`zhenfa::trap_content::tests::place_rejected_chunk_density_exceeded`
- 手动：手持警示符 → 对地面右键 → 看到放置动画 → 阵法消失嵌入地面

---

## P2 — 触发系统 ✅ 2026-05-12 NZST

### 交付物

1. **竖向柱形检测**（警示阵 / 缓阵）

   每 10 tick 检查一次：以 TrapNode 位置为底，向上 3 格，半径 1.5/2 格的柱形内是否有非放置者实体。

   ```rust
   fn detect_vertical_column(trap_pos, radius, height, ignore_entity) -> Vec<Entity>
   ```

2. **水平 LOS 检测**（爆炸阵）

   每 5 tick 检查一次：以 TrapNode 位置为中心，水平 1.5 格半径内是否有非放置者实体，且该实体到阵法之间无固体方块（raycast）。

   ```rust
   fn detect_horizontal_los(trap_pos, radius, ignore_entity) -> Option<Entity>
   ```

3. **效果应用**

   - 警示阵：`TrapTriggeredEvent` → 放置者收到 `WarningAlertS2c { direction, distance, trap_pos }`
   - 爆炸阵：`TrapTriggeredEvent` → 目标受 `wound + contam + knockback` → TrapNode 移除 → `qi_release_to_zone`
   - 缓阵：`TrapTriggeredEvent` → 目标 debuff `MovementSlow(0.5) + QiRegenPause` 持续 3s → 触发次数 -1

### 验收抓手

- 测试：`zhenfa::trap_content::tests::warning_detects_above_3_blocks`
- 测试：`zhenfa::trap_content::tests::warning_ignores_placer`
- 测���：`zhenfa::trap_content::tests::blast_requires_los`（方块遮挡不触发）
- 测试：`zhenfa::trap_content::tests::blast_one_shot_removes_node`
- 测试：`zhenfa::trap_content::tests::slow_three_charges_then_remove`
- 测试：`zhenfa::trap_content::tests::slow_debuff_duration_3s`
- 测试：`zhenfa::trap_content::tests::blast_damage_scales_with_sealed_qi`（qi=15 → dmg=9 / qi=30 → dmg=18）
- 测试：`zhenfa::trap_content::tests::blast_qi_returns_to_zone`（守恒律）

---

## P3-P6 — 逸散/隐蔽/Client/测试 ✅ 2026-05-12 NZST

（结构同 P0-P2 模式，具体交付物见 §5/§2/§4 对应节）

### 验收抓手（关键 pin）

- 逸散：半衰期 8h/2h/4h pin + 负灵域 ×0.3 pin + 失效阈值 pin
- 天道：chunk 总 qi > 0.85 → 灵气归零 + 阵法加速逸散 pin
- 隐蔽：引气感知 qi>0.1 / 凝脉感知 qi>0.05 / 地师破阵 3 格自动标记 pin
- Client：放置者蓝色标记粒子可见 / 其他人不可见 / 触发 VFX / 警示 HUD 箭头

---

## Finish Evidence

- **落地清单**：
  - P0：`server/assets/items/zhenfa.toml` 注册 `warning_trap` / `blast_trap` / `slow_trap`；`server/src/craft/mod.rs` 注册 3 个 `ZhenfaTrap` 配方；`client/src/main/resources/assets/bong-client/textures/gui/items/{warning_trap,blast_trap,slow_trap}.png` 落图标。
  - P1：`server/src/zhenfa/mod.rs` 扩展 `ZhenfaKind::{WarningTrap,BlastTrap,SlowTrap}`、`ZhenfaPlaceRequest.item_instance_id/target_face`、真元扣除、物品消耗、chunk 密度拒绝；`client/src/main/java/com/bong/client/mixin/MixinClientPlayerInteractionManagerAlchemy.java` 接手持凡阵右键放置入口，`ZhenfaLayoutScreen` 负责确认请求。
  - P2：`server/src/zhenfa/trap_content.rs` 提供竖向柱形/水平同层检测、真元成本、伤害、半衰期、隐蔽 profile；`tick_zhenfa_registry` 落警示通知、爆炸一次性伤害并回流 zone qi、缓阵 3 次触发和 `QiRegenPaused`。
  - P3-P4：`survival_ticks_with_environment()` 接 zone qi 环境修正；过期阵法走 `ArrayDecayEvent` + `qi_release_to_zone`；`discovery_profile()` 进入 reveal threshold；拆除沿用 `ZhenfaDisarmRequest` / `ArrayBreakthroughEvent`。
  - P5-P6：client/server/schema 对齐 `target_face` 和 3 个凡阵 kind；生成 schema 更新 `client-request-v1.json` / `client-request-zhenfa-place-v1.json`；边界回归覆盖放置扣 qi、无阵旗可用、LOS 阻挡、忽略放置者、缓阵耗尽、密度阈值、半衰期/环境倍率。
- **关键 commit**：
  - `dce9e0e99`（2026-05-12 NZST）实现凡阵内容服务端契约。
  - `ca9d26b3a`（2026-05-12 NZST）接入凡阵客户端放置与图标。
  - `2b97746d2`（2026-05-12 NZST）同步凡阵放置请求生成 schema。
  - `edfab7d72`（2026-05-12 NZST）补齐凡阵环境逸散运行时。
  - `af079f418`（2026-05-12 NZST）补充凡阵触发边界回归。
- **测试结果**：
  - `server/`: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` → 4500 passed。
  - `agent/`: `npm run build && cd packages/tiandao && npm test && cd ../schema && npm test` → tiandao 358 passed；schema 377 passed。
  - `client/`: `JAVA_HOME=$HOME/.sdkman/candidates/java/17.0.18-amzn PATH=$HOME/.sdkman/candidates/java/17.0.18-amzn/bin:$PATH ./gradlew test build` → BUILD SUCCESSFUL。
- **跨仓库核验**：
  - server：`OrdinaryTrapKind` / `ZhenfaKind::WarningTrap` / `ZhenfaKind::BlastTrap` / `ZhenfaKind::SlowTrap` / `StatusEffectKind::QiRegenPaused` / `ZhenfaPlaceRequest.target_face`。
  - agent/schema：`ZhenfaKindV1` 包含 3 个凡阵 kind；`ZhenfaTargetFaceV1`；`ZhenfaPlaceRequestV1.target_face`。
  - client：`ClientRequestProtocol.ZhenfaKind` / `ZhenfaTargetFace` / `ClientRequestSender.sendZhenfaPlace(... itemInstanceId, targetFace)` / `ZhenfaLayoutScreen`。
- **遗留 / 后续**：
  - 高阶阵法（地师专精版：更大范围/更长持续/连锁触发）→ plan-zhenfa-v3。
  - 阵法网络（多阵联动：警示→缓速→爆炸自动连锁）→ plan-zhenfa-v3。
  - 反制道具（破阵符/探阵玉）→ plan-tools-v2。
  - 阵法交易（帮别人布阵 = 地师的经济来源）→ plan-economy-v2。
