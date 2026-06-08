# Bong · plan-model-asset-v1 · 骨架

Tripo3D 会员 2026-05-17 到期，剩余 ~2900 网页积分 + ~250 API 积分。本 plan 在到期前批量生成项目所需的全部 3D 模型资产，建立从"AI 生成 → 减面 → 格式转换 → 游戏接入"的完整流水线，填补当前 ~200 项资产缺口中最关键的部分。

**截止日期**：2026-05-17（会员到期，积分清零）

**积分预算**：~3150 积分（网页 2900 + API 250），每模型 ~20 积分（text-to-model + PBR），可生成 **~150 个模型**。

---

## 接入面 Checklist

- **进料**：Tripo3D API（`text_to_model` / `image_to_model`）→ GLB(PBR) 原始模型
- **出料**：
  - `local_models/tripo_generated/{category}/{name}/` — 原始 GLB + 预览图 + meta.json
  - `local_models/tripo_generated/lowpoly/{name}/` — 减面后 GLB + OBJ（可导入 Blockbench）
  - `client/src/main/resources/assets/bong/models/item/{name}/` — 最终游戏内模型（OBJ + 贴图）
- **工具链**：`scripts/tripo_batch_gen.py`（批量生成）/ `trimesh` + `pyfqmr`（本地减面）/ Blockbench（手工精修）
- **下游消费**：
  - `client` — Fabric 物品渲染（OBJ CustomModelData）/ 实体渲染（GeckoLib / 自定义 EntityRenderer）
  - `plan-armor-visual-v*` — 护甲自定义模型（当前用 vanilla leather tint 兜底）
  - `plan-skull-fiend-v1` — 骨煞实体模型
  - `plan-entity-model-v1` ✅ — 已有 11 个 BBModel 实体，本 plan 补充物品/武器/环境模型
- **worldview 锚点**：§四 战斗七流派（武器）/ §八 经济（骨币/灵石）/ §十一 炼丹（丹药/灵草）/ §十二 炼器（锻造材料）/ §十六 坍缩渊（探索道具/怪物）

---

## 阶段总览

| 阶段 | 内容 | 预算 | 模型数 | 状态 |
|------|------|------|--------|------|
| P0 | 武器全系 + 护甲概念 | ~600 | ~30 | ⬜ |
| P1 | 丹药/灵草/材料/货币 | ~600 | ~30 | ⬜ |
| P2 | 坍缩渊道具 + 怪物参考 | ~500 | ~25 | ⬜ |
| P3 | 环境/建筑/氛围 | ~600 | ~30 | ⬜ |
| P4 | 变体/高阶/补漏 | ~600 | ~30 | ⬜ |
| P-done | 已完成（本次会话） | 350 | 19 | ✅ 2026-05-11 |

---

## P-done — 已完成资产（2026-05-11）✅

### 已生成模型（19 个，消耗 350 积分）

| # | 名称 | 类别 | 方式 | 文件 |
|---|------|------|------|------|
| 1 | iron_spear 铁枪 | weapon | text | tripo_generated/weapon/iron_spear/ |
| 2 | horsetail_whisk 拂尘 | weapon | text | tripo_generated/weapon/horsetail_whisk/ |
| 3 | iron_war_fan 铁扇 | weapon | text | tripo_generated/weapon/iron_war_fan/ |
| 4 | herb_knife 采药刀 | weapon | text | tripo_generated/weapon/herb_knife/ |
| 5 | silent_mirror 寂照镜 | spirit_treasure | text | tripo_generated/spirit_treasure/silent_mirror/ |
| 6 | jade_pendant 玉佩 | spirit_treasure | text | tripo_generated/spirit_treasure/jade_pendant/ |
| 7 | pill_gourd 葫芦丹瓶 | alchemy | text | tripo_generated/alchemy/pill_gourd/ |
| 8 | spirit_herb 灵草 | alchemy | text | tripo_generated/alchemy/spirit_herb/ |
| 9 | spirit_stone_cluster 灵石簇 | alchemy | text | tripo_generated/alchemy/spirit_stone_cluster/ |
| 10 | bone_coin_stack 骨币(text) | currency | text | tripo_generated/currency/bone_coin_stack/ |
| 11 | bone_coin_imgref 骨币(img) | currency | image | tripo_generated/currency/bone_coin_imgref/ |
| 12 | jade_slip 玉简 | prop | text | tripo_generated/prop/jade_slip/ |
| 13 | ancient_stone_stele 古石碑 | prop | text | tripo_generated/prop/ancient_stone_stele/ |
| 14 | formation_flag 阵旗 | prop | text | tripo_generated/prop/formation_flag/ |
| 15 | talisman_paper 符箓 | prop | text | tripo_generated/prop/talisman_paper/ |
| 16 | dead_drop_box 死信箱 | prop | text | tripo_generated/prop/dead_drop_box/ |
| 17 | ancient_scroll 残卷 | prop | text | tripo_generated/prop/ancient_scroll/ |
| 18 | skull_fiend 骨煞(减面) | creature | 外部+减面 | tripo_generated/lowpoly/skull_fiend_lowpoly/ |
| 19 | spirit_sealing_box 封灵匣(减面) | prop | 外部+减面 | tripo_generated/lowpoly/spirit_sealing_box_lowpoly/ |

### 外部下载模型（未减面原件）

| 文件 | 用途 | 面数 | 备注 |
|------|------|------|------|
| iridescent skull 3d model.glb | 骨煞原件 | 192 万 | 已减至 5000 面 |
| iron+spirit-sealing+box+3d+model.glb | 封灵匣原件 | 179 万 | 已减至 2.1 万面 |
| skeleton_drgaon.glb | 骨龙（5 动画） | 394 mesh | 自带动画，可做坍缩渊 boss |
| ruined+wooden+shelf+3d+model.glb | 残架 | 轻量 | 直接可用 |
| ant_minecraft_model_blockbench.glb | MC 蚁 | 25 mesh, 1 anim | MC 原生风格 |
| test1.glb (Wardensaurio) | MC 怪 | 34 mesh, 1 anim | MC 原生风格 |
| yog-sothoth-blockbench.zip | 克苏鲁体 | Blockbench | 可做执念/守灵 |

### 已建工具链

- `scripts/tripo_batch_gen.py` — API 批量生成脚本（ModelSpec 列表 → 并发生成 → 下载 GLB+预览）
- 本地减面流程 — `trimesh` + `pyfqmr`（支持多轮迭代、分件减面）
- Tripo SDK image-to-model — 支持参考图生成

---

## P0 — 武器全系 + 护甲概念（~30 模型，~600 积分）

**目标**：填满 `WeaponKind` 枚举全部 7 系 × 2-3 品质档 + 6 套护甲参考模型

### 武器（~20 模型）

现有 WeaponKind：Sword / Saber / Staff / Fist / Spear / Dagger / Bow

| 武器系 | 已有 | 需生成 |
|--------|------|--------|
| Sword 剑 | iron_sword, spirit_sword, flying_sword_feixuan, rusted_blade | cracked_iron_sword（破铁剑）, xuantie_sword（玄铁剑） |
| Saber 刀 | bronze_saber | rusty_saber（锈刀）, qinggang_saber（青钢刀） |
| Staff 杖 | wooden_staff | iron_staff（铁杖）, spirit_wood_staff（灵木杖） |
| Fist 拳 | hand_wrap | iron_gauntlet（铁拳套）, bone_knuckle（骨指虎） |
| Spear 枪 | iron_spear ✅ | wooden_spear（木枪）, xuantie_spear（玄铁枪） |
| Dagger 匕 | bone_dagger, crystal_shard_dagger | iron_dagger（铁匕）, poison_dagger（淬毒匕） |
| Bow 弓 | ❌ 无 | wooden_bow（木弓）, horn_bow（角弓）, spirit_bow（灵弓） |

**额外武器**（非标准分类 / 凡器 / 暗器）：

| 名称 | 说明 | 已有? |
|------|------|-------|
| 拂尘 | 道家武器 | ✅ horsetail_whisk |
| 铁扇 | 折扇暗器 | ✅ iron_war_fan |
| 采药刀 | 凡器 | ✅ herb_knife |
| 草镰 | 凡器采集 | ⬜ herb_sickle |
| 飞针 | 暗器 | ⬜ flying_needles |
| 袖箭 | 暗器 | ⬜ sleeve_crossbow |
| 铁链 | 软兵器 | ⬜ iron_chain_whip |
| 暗器·骨刺 | 上古骨暗器 | ⬜ bone_spike_anqi |

### 护甲概念模型（~10 模型）

6 套护甲每套先出 1 个胸甲概念模型（后续 plan-armor-visual 精修全套）：

| 材质 | Prompt 关键词 |
|------|-------------|
| 骨甲 bone | beast bone plates, leather cord binding, primitive |
| 兽皮 hide | tanned beast hide vest, fur trim, stitched |
| 铁甲 iron | rusty iron lamellar armor, Chinese style |
| 铜甲 copper | hammered copper scale armor, green patina |
| 灵布 spirit_cloth | dark silk robe with faint rune glow, elegant |
| 卷甲 scroll_wrap | layered paper talisman armor, yellow with red seals |

**额外**：
- 骨盔 bone_helmet
- 铁臂甲 iron_vambrace
- 兽皮靴 hide_boots
- 灵布腰带 spirit_cloth_belt

### Prompt 模板

武器统一后缀：`worn and battle-damaged, dark metal/wood, xianxia wuxia fantasy style, single weapon on plain background, game asset`

护甲统一后缀：`ancient Chinese cultivation world armor, worn and weathered, xianxia fantasy, single item on plain background, game asset`

---

## P1 — 丹药/灵草/材料/货币（~30 模型，~600 积分）

### 丹药容器 & 成品丹（~8 模型）

| 名称 | 说明 |
|------|------|
| pill_gourd ✅ | 葫芦丹瓶（已有） |
| pill_jade_bottle | 玉瓶——高级丹药容器，白玉质地 |
| pill_red_round | 红色圆丹——通用回复丹造型 |
| pill_black_round | 黑色圆丹——禁药/毒丹 |
| pill_golden_pellet | 金丹——突破类高级丹药 |
| pill_green_cluster | 绿色药丸堆——解毒类 |
| pill_powder_packet | 药粉纸包——毒粉 5 种通用模型 |
| pill_life_extension | 续命丹——琥珀色半透明丹 |

### 灵草 & 种子（~10 模型）

| 名称 | 说明 |
|------|------|
| spirit_herb ✅ | 灵草（已有） |
| ning_mai_cao | 凝脉草——紫色茎叶，有韧性 |
| ci_she_hao | 刺蛇蒿——带刺红色灌木 |
| healing_herb_bundle | 疗伤药束——绿色草药扎 |
| poison_herb_dark | 暗色毒草——黑紫色叶片，有粘液 |
| spirit_wood_branch | 灵木枝——发光木质枝条 |
| seed_pouch | 种子袋——小布包装种子 |
| dried_herb_bundle | 干燥药草束——晾干后的药材 |
| mushroom_spirit | 灵菇——发光蘑菇 |
| root_ginseng_spirit | 灵参——人形根茎 |

### 矿石 & 金属材料（~7 模型）

| 名称 | 说明 |
|------|------|
| iron_ore_chunk | 铁矿石块——暗灰色原矿 |
| copper_ore_chunk | 铜矿石块——青绿色原矿 |
| xuan_iron_ingot | 玄铁锭——暗黑金属锭 |
| qing_steel_ingot | 青钢锭——青蓝色金属锭 |
| ling_iron_ingot | 灵铁锭——微光金属锭 |
| spirit_stone_cluster ✅ | 灵石簇（已有） |
| mutant_beast_core | 变异兽核——暗红色晶体球 |

### 货币 & 通用材料（~5 模型）

| 名称 | 说明 |
|------|------|
| bone_coin ✅ | 骨币（已有两版） |
| bone_coin_5_stack | 5 骨币小堆 |
| bone_coin_40_pile | 40 骨币大堆 |
| raw_beast_hide | 生兽皮——卷起的皮革 |
| ash_spider_silk | 灰蛛丝团——灰色丝球 |

---

## P2 — 坍缩渊道具 + 怪物参考（~25 模型，~500 积分）

### 坍缩渊探索道具（~12 模型）

| 名称 | 说明 | 已有? |
|------|------|-------|
| ancient_scroll ✅ | 残卷 | ✅ |
| talisman_paper ✅ | 符箓 | ✅ |
| formation_flag ✅ | 阵旗 | ✅ |
| dead_drop_box ✅ | 死信箱 | ✅ |
| jade_slip ✅ | 玉简 | ✅ |
| ancient_key_iron | 古铁钥匙——骷髅柄锈铁钥匙 | ⬜ |
| ancient_key_jade | 古玉钥匙——翡翠材质精致钥匙 | ⬜ |
| jade_token | 令牌——方形玉令，刻印 | ⬜ |
| broken_sword_hilt | 碎剑柄——上古遗物碎片 | ⬜ |
| cracked_spirit_orb | 碎灵珠——裂纹透明球 | ⬜ |
| spirit_sealing_box ✅ | 封灵匣 | ✅（已减面） |
| inscription_stone | 铭文石——刻有阵法的扁石块 | ⬜ |
| qi_compass | 灵气罗盘——铜制指南针装置 | ⬜ |

### 怪物参考模型（~13 模型）

即使无骨骼绑定，高质量参考模型用于：① 确定美术方向 ② 提取贴图 ③ 后续接骨骼

**已有**：
- 骨煞 skull_fiend ✅（已减面 5000 面）
- 骨龙 skeleton_drgaon ✅（自带 5 动画，759KB，直接可用）
- MC 蚁 ant ✅ / Wardensaurio ✅ / 犹格·索托斯 ✅

**需生成**（参考模型，面数 ≤ 50000）：

| 名称 | 说明 | worldview 对应 |
|------|------|---------------|
| dao_chang_basic | 道伥——干尸修士，裹破袍 | §七 道伥 |
| dao_chang_elite | 执念——半透明残念体，穿残甲 | §十六.五 执念 |
| tsy_sentinel | 守灵偶——石质人形，阵法纹路 | §十六.五 守灵 |
| void_distorted | 畸变体——扭曲兽骸，负压变异 | §十六.五 畸变体 |
| devour_rat | 噬灵鼠——大型变异鼠 | §七 动态生物 |
| devour_rat_thunder | 雷鼠变体——带电弧 | §七 |
| ash_spider | 灰烬蛛——大型蜘蛛，灰色 | §七 |
| hybrid_beast | 缝合异兽——多种生物拼接体 | §七 |
| gui_ying | 诡影——半透明漩涡形体 | §七 诡影/空兽 |
| fuya | 伏牙——伏击型暗色爬行兽 | fauna |
| spirit_fox | 灵狐——白色通灵狐 | 灵兽 |
| spirit_crane | 灵鹤——仙鹤，长颈白羽 | 灵兽 |
| bone_horse | 骨马——骸骨马，可骑乘参考 | 坍缩渊坐骑 |

---

## P3 — 环境/建筑/氛围（~30 模型，~600 积分）

### 宗门遗迹（~8 模型）

| 名称 | 说明 |
|------|------|
| ruined_gate | 残门——倒塌石拱门，龙柱 |
| ruined_shelf ✅ | 残架（已有，362KB） |
| ruined_altar | 祭坛——方石台，符阵顶面 |
| ruined_pillar | 断柱——半截石柱，有刻纹 |
| ruined_wall_section | 残墙——断壁，有壁画残留 |
| ruined_statue_head | 石像残头——巨大石雕头颅落地 |
| ruined_cauldron | 破鼎——翻倒的青铜大鼎 |
| ruined_bell | 古钟——落地裂纹铜钟 |

### 自然/灵气环境（~8 模型）

| 名称 | 说明 |
|------|------|
| spirit_spring_pool | 灵泉——发光水池 |
| qi_eating_moss | 噬灵藓——暗紫色发光苔藓 |
| ancient_tree_stump | 古树桩——巨木残桩，有空洞 |
| crystal_formation | 水晶簇——洞穴内大型晶体 |
| bone_pile | 骨堆——散落的兽骨堆 |
| dry_corpse_ground | 干尸（地面）——趴伏的干尸 |
| spirit_flame_lantern | 灵火灯笼——悬浮石灯 |
| rift_crack_ground | 裂地——地面裂缝，有微光渗出 |

### 生活/交互家具（~8 模型）

| 名称 | 说明 |
|------|------|
| meditation_cushion | 蒲团——草编圆垫 |
| incense_burner | 香炉——三足铜炉 |
| bone_trap | 骨刺陷阱——地面骨刺环 |
| hanging_lantern | 挂灯——纸灯笼，破旧 |
| wooden_sign_post | 木路牌——刻字木桩 |
| prayer_flag_line | 经幡串——挂在绳上的布条 |
| weapon_rack | 兵器架——木制双层兵器架 |
| scroll_shelf | 卷轴架——竖式卷轴存放架 |

### 坍缩渊内部专属（~6 模型）

| 名称 | 说明 |
|------|------|
| rift_crystal_pillar | 负压水晶柱——暗色大晶柱 |
| collapsed_entrance | 塌方入口——碎石堆半封通道 |
| soul_cage | 魂笼——悬挂铁笼，有幽光 |
| chained_coffin | 锁链棺——铁链缠绕的石棺 |
| void_obelisk | 虚空方尖碑——悬浮黑色石碑 |
| tsy_exit_portal | 秘境出口——与 rift_portal 不同的裂缝 |

---

## P4 — 变体/高阶/补漏（~30 模型，~600 积分）

### 武器品质变体

每系核心武器出 2 个额外品质档：

| 基础 | 凡铁档 (已有) | 灵铁档 | 玄铁档 |
|------|-------------|--------|--------|
| 剑 | iron_sword | ling_iron_sword | xuan_iron_sword |
| 刀 | bronze_saber | ling_steel_saber | xuan_steel_saber |
| 枪 | iron_spear | ling_iron_spear | xuan_iron_spear |
| 弓 | wooden_bow | horn_bow | spirit_bow |

（~8 模型，部分 P0 已覆盖）

### 护甲全件补充

P0 出了 6 个胸甲概念，这里补每套的 helmet + boots（12 模型）：

### 特殊物品

| 名称 | 说明 |
|------|------|
| spirit_treasure_bell | 灵宝·魂钟——小型铜钟 |
| spirit_treasure_sword | 灵宝·断仙剑——半截上古剑 |
| spirit_treasure_brush | 灵宝·判官笔——朱砂毛笔 |
| spirit_treasure_gourd | 灵宝·吞天壶——黑色小葫芦 |
| zong_keeper_mask | 宗门守卫面具——石质半脸面具 |
| identity_token | 身份令牌——木质腰牌 |
| qi_amplify_scroll | 真元增幅卷——展开的符阵纸 |
| anvil_iron | 铁砧——锻造用铁砧 |
| anvil_spirit | 灵铁砧——有微光的锻造砧 |

### image-to-model 批量

用已有 GUI 图标作为参考图生成 3D 版本（~5 模型，每个 30 积分）：

```
client/src/main/resources/assets/bong-client/textures/gui/items/
├── fengling_bone_coin.png    → 3D 封灵骨币
├── bone_spike.png            → 3D 骨刺暗器
├── anqi_shanggu_bone.png     → 3D 上古骨暗器
├── anqi_shanggu_bone_charged.png → 3D 充能版
└── (其他有特色的 GUI 图标)
```

---

## 资产处理流水线

### 1. 生成（Tripo3D）

```
网页端：tripo3d.ai → 手动输入 prompt → 下载 GLB
API 端：scripts/tripo_batch_gen.py → 并发生成 → 自动下载
```

**生成参数标准**：
- `model_version`: v2.5-20250123（默认，性价比最优）
- `texture`: true, `pbr`: true
- `texture_quality`: standard（节省积分）
- `face_limit`: 物品 20000-50000 / 怪物 50000-80000 / 环境 30000-50000
- `negative_prompt`: "low quality, blurry, modern, sci-fi, cartoon, anime"

### 2. 减面（本地）

```python
# 大模型（>1MB）走 trimesh + pyfqmr 减面
# 目标：物品 ≤ 5000 面 / 实体 ≤ 8000 面 / 环境 ≤ 15000 面
python3 scripts/tripo_decimate.py <input.glb> <target_faces> <output_dir>
```

### 3. 格式转换

```
GLB → OBJ（Blockbench 导入用）: trimesh export
GLB → BBModel（实体用）: Blockbench 手工转换 + 分件 + UV 调整
GLB → MC JSON（物品用）: Blockbench 转换 + CustomModelData 绑定
```

### 4. 游戏接入

**物品模型**：
```
local_models/tripo_generated/{cat}/{name}/{name}.glb
  → 减面 → OBJ
  → 导入 Blockbench → 导出 MC JSON + 贴图
  → client/src/main/resources/assets/bong/models/item/{name}/
  → client/src/main/resources/assets/bong/textures/item/{name}/
  → ItemModelRegistry 注册 CustomModelData
```

**实体模型**：
```
local_models/tripo_generated/{cat}/{name}/{name}.glb
  → 减面 → Blockbench 重拓扑 + 绑骨骼
  → 导出 GeckoLib JSON（geo + animation）
  → client/src/main/resources/assets/bong/geo/entity/{name}.geo.json
  → client/src/main/resources/assets/bong/animations/entity/{name}.animation.json
  → 自定义 EntityRenderer + Model 类
```

**环境模型**：
```
local_models/tripo_generated/{cat}/{name}/{name}.glb
  → 减面
  → 导入 Blockbench → BBModel（含多状态贴图）
  → 走 plan-entity-model-v1 同模式的 Block Entity Renderer
```

---

## 批量生成执行计划

### 网页端（2900 积分，手动操作）

**Day 1（5/12）— P0 武器**：~20 个武器模型
**Day 2（5/13）— P0 护甲 + P1 丹药**：~10 护甲 + ~8 丹药
**Day 3（5/14）— P1 灵草/材料**：~15 模型
**Day 4（5/15）— P2 坍缩渊道具+怪物**：~25 模型
**Day 5（5/16）— P3 环境+P4 变体**：~30 模型
**Day 6（5/17）— 补漏 + image-to-model**：剩余积分全部用完

每天 prompt 清单见本 plan 各阶段表格，直接复制到 Tripo 网页端。

### API 端（250 积分，自动化）

```bash
# 编辑 scripts/tripo_batch_gen.py 的 MODELS 列表
# 每批 10-15 个（并发 8），跑完换下一批
TRIPO_API_KEY=tsk_xxx python3 scripts/tripo_batch_gen.py
```

优先用 API 跑**不需要视觉判断的简单物品**（矿石、药丸、材料），复杂模型（武器、怪物、建筑）用网页端手动生成可以实时预览和重试。

---

## 命名规范

```
local_models/tripo_generated/
├── weapon/          # 武器
│   ├── {weapon_name}/
│   │   ├── {name}_pbr.glb    # 原始 PBR 模型
│   │   ├── {name}_preview.webp  # 预览图
│   │   └── meta.json         # 生成参数记录
├── armor/           # 护甲
├── alchemy/         # 丹药/灵草/材料
├── currency/        # 货币
├── prop/            # 道具/环境物件
├── creature/        # 怪物参考
├── structure/       # 建筑/遗迹
├── spirit_treasure/ # 灵宝
└── lowpoly/         # 减面后的模型
    └── {name}/
        ├── {name}.glb    # 减面 GLB
        ├── {name}.obj    # 减面 OBJ（Blockbench 用）
        └── meta.json
```

**meta.json 必填字段**：
```json
{
  "name": "iron_spear",
  "category": "weapon",
  "prompt": "...",
  "source": "text_to_model | image_to_model | external",
  "task_id": "xxx (API) | manual (web)",
  "timestamp": 1715...,
  "original_faces": 50000,
  "target_faces": 5000,
  "game_item_id": "bong:iron_spear (绑定后填)"
}
```

---

## 测试 & 验收

- [ ] 每个生成的模型有 preview.webp 可查看
- [ ] 需接入游戏的模型完成减面（物品 ≤5000 / 实体 ≤8000 / 环境 ≤15000）
- [ ] 减面后模型 GLB 大小 ≤ 500KB
- [ ] 导出 OBJ 可在 Blockbench 中正常打开
- [ ] 全部 meta.json 完整
- [ ] 最终积分余额 ≤ 100（尽量用完）

---

## Finish Evidence

（迁入 finished_plans/ 前填写）
