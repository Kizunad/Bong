# dark_tiger 补动画（Round 1）

骨架：`ALL` > `Air_origin`（静止旋转 X90）> 前后躯 / 腿 / 尾 / 颈肩 / 头。腿、脊柱、尾的静止旋转都只绕 X，动画里绕 X 即世界俯仰（正 = 腿向后 / 头颈向下）。
已有段 `idle` `melee_lion1` `walk` 未改（脚本断言 + 与 HEAD 逐段比对）。

| 段 | 时长 | 循环 | 说明 | GIF |
|---|---|---|---|---|
| `run` | 0.56s | 是 | 二拍跳跃步态：后腿成对蹬地、前腿成对前伸，脊柱一屈一伸，尾巴向后拖直，嘴随步伐微张 | `dark_tiger/run.gif` |
| `attack` | 0.9s | 否 | 后坐蓄力（前身抬起、双爪高举、张嘴）→ 整体前冲 15px + 双爪下劈 + 合颌 → 收势；与旧 `melee_lion1` 独立 | `dark_tiger/attack.gif` |
| `hurt` | 0.45s | 否 | 头颈后仰、整体后挫 4.5px、张嘴吃痛、尾巴甩一下 | `dark_tiger/hurt.gif` |
| `death` | 2.0s | 否 | 受击 → 前腿先软、身体左右晃 → 侧身砸倒，下颌松开、尾瘫软，终态保持 | `dark_tiger/death.gif` |

## 自查
- 入地（相对静止姿态最低点）：run 0.17 / attack 0.31 / hurt 0.38 / death 0.31 px，均 ≤ 0.6。旧 `walk` 自己有 2.7px 入地，未动。
- 循环段首尾同值已程序核验。
- death 的 `ALL.position.x` 用 +23 抵消侧倒后质心外移（GeckoLib 的 position X 与世界 X 反号，渲染实测）；倒向是模型的左侧。
- 受限于单机位 GIF，run 的左右腿相位差只有 ±0.04 周期，肉眼接近「成对」，是有意的。
- 脚本数值从零写，未复制旧 walk / melee_lion1 的关键帧。
