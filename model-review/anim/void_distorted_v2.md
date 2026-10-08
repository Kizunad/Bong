# void_distorted_v2（渊空畸变体）动画（Round 1）

- 状态：round-1（等调度核对后进入 Round 2 人工闸门）
- 类型：生物自身动画（GeckoLib）
- 模型：`client/src/main/resources/assets/bong/geo/void_distorted_v2.geo.json`（11 根骨、1456 cube；已在 origin/main，本轮未改模型，只补动画）
- 产物：`client/src/main/resources/assets/bong/animations/void_distorted_v2.animation.json`
- 生成脚本：`modelScript/creatures/fauna_v2/gen_anim.py`（`void_distorted_clips`）
- 预览目录：`model-review/anim/void_distorted_v2/`（每段一张原速 GIF，3/4 单机位、20fps，按游戏约定 -Z 取景）

## 动画清单

| 动画 | 时长 | 循环 | 说明 |
|---|---|---|---|
| idle | 4.0s | 是 | 低伏身躯缓慢起伏扭动；虚空口无规律地张合两次（长短不等，像无意识的喘息）；两侧前肢各自在不同时刻独立抽搐（不对称，不是同步呼吸感），是「畸变」的辨识度所在 |
| walk | 1.3s | 是 | 对角步态：右前爪配左后腿、左前爪配右后腿同步，前肢（挂在 body 上）向前探地抓拽，后腿（挂在 root 上）蹬地推进；身体随步伐起伏扭动，虚空口随之小幅晃动 |
| attack | 0.9s | 否 | 身体后蓄 → 向前猛扑（身体位移前冲）、虚空口张到最大后合拢作咬合、两侧前爪同步前扑下压，后腿蹬地助推 |
| hurt | 0.4s | 否 | 全身一震后仰、身体侧甩，虚空口痛苦地张开，两侧前肢反向甩动、爪子抽搐蜷缩 |
| death | 1.8s | 否 | 身体整体瘫塌贴地（由 check_ground 贴地），前肢爪子松开摊平、后腿向外滑开，虚空口张开后不再合拢 |

## 参考前例

- 骨架：`gen_rig.py` 的 `VOID_DISTORTED_BONES` / `_void_distorted_side_bones`（本轮未改）。
- 任务卡：「低伏兽躯，前肢探地、后腿推进，虚空口和两侧骷髅可张合/扭动」。
- `void_front_limb` / `void_rear_leg` 两个摆腿辅助函数采用本仓已有的「摆动相 / 支撑相」两段式步态结构（类似 `fuya_step` 的写法思路），具体数值与相位分配从零重写，未复制任何既有创作的关键帧数值。

## 自查发现的问题

- death 的下沉幅度（`VOID_SINK = -2.2`）在视觉上不够明显——这只生物 idle 时本来就很低伏，「死态」和「待机」的外观区分度不够强，Round 3 可以考虑加大下沉或让身体进一步塌扁。
- walk 的对角步态是完全对称镜像的（右前+左后一组、左前+右后一组），没有像 idle 里强调的「不对称抽搐」那样体现畸变感；如果想要更强的错位感，Round 3 可以给两组步幅或节奏故意调得不对称。
- maw（虚空口）在 attack 里只用了单一 x 轴张合，没有像任务卡描述的「扭动」那样加入 y/z 分量，画面上更像单纯「张嘴」，Round 3 可以补一点侧向歪斜。
