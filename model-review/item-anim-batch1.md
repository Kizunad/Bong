# item-use-anim 第 1 批 Round 1 接触表

状态：gate-waiting

---

## 批次概览

| # | 动画 ID | 描述 | JSON 路径 | 生成器 |
|---|---------|------|-----------|--------|
| 1 | `hand_wrap_jab_left` | 缠手左直拳（布缠·轻快·手腕翻转·短距离） | `player_animation/hand_wrap_jab_left.json` | `gen_hand_wrap_jab_left.py` |
| 2 | `hand_wrap_jab_right` | 缠手右直拳（布缠·轻快·镜像） | `player_animation/hand_wrap_jab_right.json` | `gen_hand_wrap_jab_right.py` |
| 3 | `bing_jia_heavy_left` | 兵甲手套左重拳（铁甲·慢起步·髋关节驱动·落地震荡） | `player_animation/bing_jia_heavy_left.json` | `gen_bing_jia_heavy_left.py` |
| 4 | `bing_jia_heavy_right` | 兵甲手套右重拳（铁甲·重压·镜像） | `player_animation/bing_jia_heavy_right.json` | `gen_bing_jia_heavy_right.py` |
| 5 | `pickaxe_bone_use` | 骨镐挥镐（轻凿·单手·快节奏·12 tick 循环） | `player_animation/pickaxe_bone_use.json` | `gen_pickaxe_bone_use.py` |
| 6 | `wooden_staff_atk` | 木杖截击（杖头向下压制·双手握·12 tick） | `player_animation/wooden_staff_atk.json` | `gen_wooden_staff_atk.py` |
| 7 | `bone_sword_slash` | 骨剑弧切（右侧横扫·躯干反转 42°·9 tick） | `player_animation/bone_sword_slash.json` | `gen_bone_sword_slash.py` |

---

## 设计要点

### hand_wrap（缠手）两式
- 区分手套类别的核心参数：**时长短（6–9 tick）、手腕翻转主导、肩膀旋转量 ≤ 15°**
- 左右式通过 torso yaw 方向和主臂切换区分，不是简单镜像
- 刻意避免与 `fist_punch_right` / `fist_punch_left` 雷同：缠手拳头路径更直、收拳更快、身体重心几乎不变

### bing_jia_shou_tao（兵甲手套）两式
- 区分手套类别的核心参数：**时长 12–13 tick、起步慢（INOUTSINE）、落地 recoil 帧明显**
- 重拳冲程中躯干大幅前倾 (pitch +12°)、髋侧移 (x ±0.10 m)，体感"砸"而非"刺"
- 两式之间 torso yaw 总旋转量约 52°（左）/ 56°（右），可从第三人称分辨

### pickaxe_bone（骨镐）
- 单手握持（左手辅助为副手位），12 tick 循环
- 镐头向下弧砸，抬起时有后甩；铁镐已有 `pickaxe_iron_v2_use`（双手，重），骨镐刻意做成单手轻凿对比
- 循环 closure：每个用到的 axis 在 endTick=12 都有闭合关键帧

### wooden_staff（木杖）
- 双手持杖（非刀剑握姿）：右手高位在杖上端，左手低位在杖下端
- 截击路径向下压制，收势时杖头拉回体侧
- 12 tick，非循环

### bone_sword（骨剑）
- 弧切：右侧拉剑 → 右后下蓄力 → 弧扫左前上 → 越过顶点回收，9 tick
- 躯干反转 42°（yaw: −24° → +18°）是区分度最高的视觉信号
- rightItem 随各帧赋静态 pitch/yaw/roll dict（未使用 item_spin，剑刃方向由手臂角度决定）

---

## 接触表

`model-review/img/item-anim/batch1/batch1_contact.png`（1176 × 23852 px，7 行，每行含该动画所有关键帧的三视图条带）

---

## 调度审 / 人工反馈

（预留——调度与用户填写）

---

## 已确认的技术合规项

- 所有动画均以空白 POSE 设计，未以任何 v2_use 或现有动画为底稿
- `bend` axis 均写为 `axis` 键（非 `bendDirection`）
- 循环动画（`pickaxe_bone_use`）的所有用到的 axis 在 `endTick` 均有闭合帧
- `leg.pitch` 均 ≤ 40°（骨镐循环最大 28°，其余攻击动画 ≤ 22°）
- 未使用 `body.yaw`，整体旋转走 `torso.yaw`
- 7 个 JSON 均由 `emit_json` 验证通过，无报错
