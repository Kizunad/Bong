# void_distorted_v2（渊空畸变体）动画（Round 1）

- 状态：round-1-rework（idle / attack / hurt / death 调度已过；walk 按调度意见返工，见下方「返工记录」）
- 类型：生物自身动画（GeckoLib）
- 模型：`client/src/main/resources/assets/bong/geo/void_distorted_v2.geo.json`（11 根骨、1456 cube；已在 origin/main，本轮未改模型，只补动画）
- 产物：`client/src/main/resources/assets/bong/animations/void_distorted_v2.animation.json`
- 生成脚本：`modelScript/creatures/fauna_v2/gen_anim.py`（`void_distorted_clips`）
- 预览目录：`model-review/anim/void_distorted_v2/`（每段一张原速 GIF，3/4 单机位、20fps，按游戏约定 -Z 取景）

## 动画清单

| 动画 | 时长 | 循环 | 说明 |
|---|---|---|---|
| idle | 4.0s | 是 | 低伏身躯缓慢起伏扭动；虚空口无规律地张合两次（长短不等，像无意识的喘息）；两侧前肢各自在不同时刻独立抽搐（不对称，不是同步呼吸感），是「畸变」的辨识度所在 |
| walk | 1.3s | 是 | 对角步态：右前爪配左后腿、左前爪配右后腿同步，前肢（挂在 body 上）只在肩部小幅摆动探地、前臂始终朝下，收回靠爪子关节自己往回勾（不抬肩）；后腿（挂在 root 上）蹬地推进；两条对角线步幅/屈曲量故意不对称；身体随步伐起伏扭动，虚空口随之小幅晃动 |
| attack | 0.9s | 否 | 身体后蓄 → 向前猛扑（身体位移前冲）、虚空口张到最大后合拢作咬合、两侧前爪同步前扑下压，后腿蹬地助推 |
| hurt | 0.4s | 否 | 全身一震后仰、身体侧甩，虚空口痛苦地张开，两侧前肢反向甩动、爪子抽搐蜷缩 |
| death | 1.8s | 否 | 身体整体瘫塌贴地（由 check_ground 贴地），前肢爪子松开摊平、后腿向外滑开，虚空口张开后不再合拢 |

## 参考前例

- 骨架：`gen_rig.py` 的 `VOID_DISTORTED_BONES` / `_void_distorted_side_bones`（本轮未改）。
- 任务卡：「低伏兽躯，前肢探地、后腿推进，虚空口和两侧骷髅可张合/扭动」。
- `void_front_limb` / `void_rear_leg` 两个摆腿辅助函数采用本仓已有的「摆动相 / 支撑相」两段式步态结构（类似 `fuya_step` 的写法思路），具体数值与相位分配从零重写，未复制任何既有创作的关键帧数值。

## 自查发现的问题

- death 的下沉幅度（`VOID_SINK = -2.2`）在视觉上不够明显——这只生物 idle 时本来就很低伏，「死态」和「待机」的外观区分度不够强，Round 3 可以考虑加大下沉或让身体进一步塌扁。
- maw（虚空口）在 attack 里只用了单一 x 轴张合，没有像任务卡描述的「扭动」那样加入 y/z 分量，画面上更像单纯「张嘴」，Round 3 可以补一点侧向歪斜。

## 返工记录（walk，调度 Round 1 核对后打回）

- **问题**：walk 约 1/4 周期那一帧，远侧前肢整条甩到背上方竖起来，不是贴地探爪。
- **根因**：`void_front_limb` 收回相原来用 `pos=(0, raise_y, 0)` 直接平移 `arm_{side}` 的肩关节枢轴（最大 8px），而不是转它——相当于把整条前肢从肩窝里拔出来悬空，不是摆动角度本身超限。
- **改法**：肩部（`arm_{side}`）全程只转不平移，摆幅从 ±10° 收到 ±6~9°；收回相改成转爪子自己的关节（`claw_{side}` 正向折，类比腿部屈膝的手法）把爪子往回勾，结构上爪尖只能勾到腕关节附近的高度，做不到甩过肩。同时按调度意见，把两条对角线（右前+左后 / 左前+右后）的肩摆幅、爪勾曲、后腿步幅和屈膝都调成不对称的两组数值，不再是镜像对称的协调步态。
- **验证**：精确采样 t=0.325s / 0.975s（两条对角线各自的收回相峰值帧）看三视角接触表，前肢都贴在身体下方，没有再出现抬手；只有 `walk` 一段的 JSON 字节变化（逐段 diff 确认），`idle`/`attack`/`hurt`/`death` 原样未动；`modelScript/tests` 的 fauna_v2 相关 8 个测试仍 0 失败。
