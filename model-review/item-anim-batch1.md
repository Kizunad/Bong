# item-use-anim 第 1 批 Round 1 接触表（修订版，2026-10-04）

状态：gate-waiting（Round 2 人工闸门，等调度转用户；本轮未接线、未开 PR）
执行：claude-anim（Sonnet 5）

---

## 修订记录

- 调度审（2b248af86）要求的三处修复已完成：
  1. 中文标签改用 `/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc`（标题加粗用 Bold），不再是豆腐块。
  2. 木杖左手关键帧改到杖身中段偏上（双手持杖），生成器与 JSON 一起改，见动画 6。
  3. 先 `git fetch origin && git merge origin/main`（合并提交 `e6ab6ff72`，带进 #2386 的 v2 模型）；手持物全部改用 v2 网格与它自己的 display，不再用 0.3 格立方体示意。
- 本轮未改的部分：批次结构、其余 6 条动画的关键帧、骨镐循环、注册表。
- 上一轮（a5ff9e3cd）的火柴人帧图已删除，见 2b248af86。

## 本轮做了什么（渲染口径）

- `client/tools/render_block_figure.py`：方块人按 MC 1.20.1 玩家模型盒尺寸画头、躯干、上臂/前臂、大腿/小腿；前臂与小腿按 bendy-lib 语义折弯。手持物按原版手持链路放进手里；每条动画一条带图，列为关键帧 tick，行为正面 / 侧面（玩家右侧）/ 3/4 斜前；同类旧动画对照在条带下方。
- 产物：`model-review/img/item-anim/batch1/<动画 id>.png`（上：新动画；下：同类旧动画），总接触表 `batch1/batch1_contact.png`。

### 手持链路（读 1.20.1 字节码确认，不是凭记忆）

| 环节 | 依据 |
|------|------|
| 手臂姿态 = `translate(pivot/16)` 后接 ZYX 旋转 | `BipedEntityModel.setArmAngle` → `ModelPart.rotate` |
| 手持物先 `rotX(-90°)`、`rotY(180°)`，再 `translate((±1/16, 0.125, -0.625))` | `HeldItemFeatureRenderer` |
| display = `translate(d)` · `rotationXYZ(rot)` · `scale(s)`，末尾 `translate(-0.5)` | `Transformation.apply`、`ItemRenderer` |
| 手持物握点落在前臂末端（约 y=10px），与手掌对得上 | 上式代入的结果，旁证 |

PlayerAnimator 的 `rightItem` 插在 display 之后、手持偏移之前（与 `anim_common.item_spin` 注释一致）。

### 与 `render_animation.py` 的两处差异

- `body` 与部件的 x/y/z 按方块解释、乘 16 换算成像素。`render_animation.py` 直接当像素用，位移偏小 16 倍。依据：`docs/player-animation-conventions.md` §0 与 `anim_common.py` 的 `ITEM_PARTS` 注释。
- `rightItem` / `leftItem` 关键帧由渲染器自己收集。`bbmodel_maker` 的 `BODY_PART_NAMES` 不含它们，直接用现成采样会静默丢帧。

## 渲染的近似（看图前必读）

- 手持物随前臂折弯整体刚性转动；原版 bend 只弯前臂网格，手持物不做网格变形。
- 正交投影，没有光照与 FOV，物品只用面法线做明暗；手持物颜色刻意避开肤色。
- 握持是否自然、拳面是否贴手，最终判断仍以 runClient 为准。
- 手持物一律用 v2 网格与其 `display`（与生成器读取的 JSON 一致）。注册表登记的旧模型与 v2 不一致，见「模型台问题」。

---

## 批次概览（tick 与循环均取自 JSON）

| # | 动画 ID | 关键帧 tick | endTick | 循环 | 手持（v2 网格） | 同类旧动画（对照行） |
|---|---------|-------------|---------|------|-----------------|----------------------|
| 1 | `hand_wrap_jab_left` | 0,1,3,5,6,8 | 8 | 否 | 双手缠手 `hand_wrap_v2` | `fist_punch_left` |
| 2 | `hand_wrap_jab_right` | 0,2,4,6,7,9 | 9 | 否 | 双手缠手 `hand_wrap_v2` | `fist_punch_right` |
| 3 | `bing_jia_heavy_left` | 0,2,5,8,9,10,12 | 12 | 否 | 双手兵甲 `bing_jia_shou_tao_v2` | `fist_punch_left` |
| 4 | `bing_jia_heavy_right` | 0,2,6,9,10,11,13 | 13 | 否 | 双手兵甲 `bing_jia_shou_tao_v2` | `fist_punch_right` |
| 5 | `pickaxe_bone_use` | 0,3,5,6,8 | 8 | **是** | 右手 `pickaxe_bone_v2` | `pickaxe_iron_v2_use`（右手铁镐 v2） |
| 6 | `wooden_staff_atk` | 0,2,5,8,9,11 | 11 | 否 | 右手 `wooden_staff_v2`；左手贴杖身 | `sword_swing_horiz`（空手） |
| 7 | `bone_sword_slash` | 0,2,5,7,8,9 | 9 | 否 | 右手 `bone_sword_v2`（含 rightItem 关键帧） | `iron_sword_v2_use`（右手铁剑 v2） |

## 设计与复核（逐件）

以下躯干扭量是从 JSON 采样算出的实测值（tick 0..endTick 内 `torso.yaw` 的极差），不是设计注释里的估计。
「复核」段是在总接触表与条带图上粗看得出的结论，不是逐帧量测；逐帧细节留给调度与 Round 2。

### 1. hand_wrap_jab_left（缠手左直拳）
- 设计：8 tick 快出快收；左拳 impact 时 `leftArm.roll` 从 -20° 翻到 +15°，手腕翻转为主；躯干扭量 30°；腿 pitch ≤ 18°。
- 与 `fist_punch_left`（10 tick，躯干扭量与出拳幅度更大）的区别：时长更短，靠手腕翻转而非躯干大转。
- 复核：v2 拳套两手都戴着，护颊姿态可见；出拳侧拳头在 impact 帧前伸。**设计注释的「肩膀旋转 ≤15°」与实测 30° 不符**，以实测为准，待调度决定是否收紧。

### 2. hand_wrap_jab_right（缠手右直拳）
- 设计：左式的镜像式对位，但躯干扭量 42°，比左式大，两式并不完全对称。
- 复核：与左式的区别主要在躯干，不是手腕；在远距离可能读作同一个动作。**待调度判断是否需要统一两式的躯干幅度。**

### 3. bing_jia_heavy_left（兵甲手套左重拳）
- 设计：12 tick；windup 在 tick 5 把左臂提到头侧（pitch -148°）；impact 在 tick 8 落到 -104°，之后 hold（tick 9）与 overshoot（tick 10）各一帧消化冲击；躯干扭量 54°。
- 与缠手的区别：时长（12 vs 8）、躯干扭量（54° vs 30°）、有 hold 段。与 `fist_punch_left` 的区别同理，时长与躯干幅度都更大。
- 复核：兵甲网格（铁色）与肤色、袖子蓝分得开；重拳落点在前下方。

### 4. bing_jia_heavy_right（兵甲手套右重拳）
- 设计：镜像结构；13 tick；躯干扭量 62°。
- 复核：比左式更大幅度，左右两式不对称（右式 62° vs 左式 54°）。**这是生成器里的不对称，调度需决定是否统一。**

### 5. pickaxe_bone_use（骨镐挥凿，循环）
- 设计：单手右持，8 tick 一周期，循环闭合（tick 0 与 endTick 同值，已校验）；腕内翻 roll 约 -25°；躯干扭量仅 7°，靠手腕甩而非身体大转。
- 与 `pickaxe_iron_v2_use`（14 tick，双手，躯干大幅参与）的区别：单手、短周期、几乎不转身。
- 复核：挥凿轨迹在三视图中可辨；骨镐用 `pickaxe_bone_v2` 的镐形。
- **未决（接线阶段核实）**：本动画是循环（`isLoop=true`）。采集是循环播放，能否直接按采集节拍重复播放 `pickaxe_bone_use`，要先确认服务端能拿到手持工具与采集节拍。本轮不接线，未核实。

### 6. wooden_staff_atk（木杖斜向截击）
- 设计：双手持杖，11 tick；右臂从右肩高位（pitch -130°）上举到 -166°，再斜扫到左下（-58°）；躯干扭量 62°。右手握在杖的近握把端。
- 左手：按调度要求，左手关键帧改到**杖身中段偏上**（离右手沿杖轴约 6px，即 0.375 格）。
  - 解法：一次性反解脚本（未入库，在 scratchpad），把左臂 pitch/yaw/roll/bend 坐标下降到使左手前臂末端中心贴到杖轴上。
  - 结果：关键帧上左手到杖轴的垂直距离最大 **1.49px**；0.25 tick 步长密采样最大 **1.73px**（约 0.1 格）。
  - 生成器 `tools/gen_wooden_staff_atk.py` 与 JSON 已同步更新，文件头「双手持杖」段写明了这个口径。
- **代价（已拉到关节边界，需调度看）**：
  - 六个关键帧（0/2/5/8/9/11）的左臂 `bend` 全部顶在 20° 下限（几乎伸直）；
  - tick 5 `pitch` 到 -180°，tick 9 `yaw` 到 90°；
  - `roll` 最大 -84°（前臂明显扭转）。
- **根因（不在本轮改动范围）**：右手握点在右肩外侧（约 x=-4px、y=-6px），杖身从握点斜向右下前方延伸（轴向约 (-0.29, +0.83, -0.48)）。左手要横跨胸前到身体右侧才够得着杖身。在舒适关节范围（pitch -170°~-20°，yaw/roll ±60°，bend ≥30°）内，误差超过 1.5px，**不可达**。
  - 要做出自然的双手持杖，需要改右臂姿态或杖的挂点，让杖横过胸前；这是右手关键帧的改动，本轮按调度只改左手，未动。
  - 原版只有右手能持物，左手在游戏里只是姿态，不挂物品；这里的贴合只在审图里成立，需要调度确认是否接受。

### 7. bone_sword_slash（骨剑侧弧切）
- 设计：9 tick；右手单持；剑刃 rightItem 从 +30° 翻到 -60° 再到 -70°，弧切路径从右腰后侧到左前上；躯干扭量 46°。
- 与 `iron_sword_v2_use`（过顶直劈，10 tick）的区别：横弧 vs 纵劈；骨剑 9 tick 更快。
- 复核：剑在 t=0 从右肩上方起势，t=5~8 剑尖扫到左前上；rightItem 关键帧确实生效（剑刃翻转肉眼可见）。

---

## 模型台问题（记一笔，接线阶段再定）

按调度意见，注册表与 v2 的不一致本轮不修，统一留到接线阶段，正式通道见骨架 `docs/plans-skeleton/plan-item-model-channel-v1.md`。

1. **`hand_wrap.obj` 与现有配置不匹配**：网格 x 跨 3.75 格、z 跨 6 格，`asset_configs/hand_wrap.json` 的 scale 0.35 仍约 2 格长。注册表仍指向它；审图已改用 `hand_wrap_v2`。
2. **`bing_jia_shou_tao` 注册表与注释不一致**：`BongWeaponModelRegistry.java:187-189` 的注释写「借用 hand_wrap.obj」，但 `Entry` 的 `objPath` 是 `null`，实际手持是原版皮革 2D 贴图。注册表尚未接入 `bing_jia_shou_tao_v2`。
3. **`bone_sword` 注册表 `objPath=null`**：借原版石剑贴图；`bone_sword_v2` 未接入注册表。
4. **`wooden_staff.obj` 高 10 格、无 display 缩放配置**：注册表登记的是它，尺寸明显异常；`wooden_staff_v2`（1.76 格）与设计的 1.6m 一致。
5. **`pickaxe_bone.obj` 仅 16 顶点薄板**：注册表登记的是它；`pickaxe_bone_v2` 是镐形。

第 2~5 条都需要在接线阶段决定注册表是否切到 v2 模型，这会影响接线时的模型 id。

## 给调度的开放问题（Round 2 前）

- **木杖的双手握持**：左手已贴杖身，但左臂关节被拉到边界，根因在右手握杖的位置（见动画 6）。是否改右臂姿态，还是接受单手截击的口径。
- `pickaxe_bone_use` 循环与采集节拍的关系（接线阶段核实）。
- 缠手两式、兵甲两式的躯干幅度是否统一（见动画 2、4）。
- 模型台问题 2~5 的处理方式（接线阶段）。

---

## 技术合规（已核对 JSON）

- 7 个动画均 `degrees=false`，`bend` 写为 `axis` 键。
- 没有使用 `body.yaw`，整体旋转走 `torso.yaw`；`body` 轴只有 x/y/z。
- 腿 pitch 最大 22°（远低于 40° 上限）。
- `pickaxe_bone_use` 是循环动画，endTick 闭合帧已由 `emit_json` 的 `_check_loop_closure` 校验。
- 木杖左臂的 `axis` 固定 180°，所有关键帧通过 `assert_joint_fold_is_anatomical`（肘向身前折）。
- 设计注释中的部分数字与 JSON 不符（如缠手肩扭 ≤15°、兵甲两式 52°/56°、骨镐 12 tick）。以 JSON 为准，本文档只写实测值。

## 调度审 / 人工反馈

（预留，调度与用户填写）
