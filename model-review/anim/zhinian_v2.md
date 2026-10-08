# zhinian_v2（执念）动画（Round 1）

- 状态：round-1（等调度核对后进入 Round 2 人工闸门）
- 类型：生物自身动画（GeckoLib）
- 模型：`client/src/main/resources/assets/bong/geo/zhinian_v2.geo.json`（20 根骨、288 cube；已在 origin/main，本轮未改模型，只补动画）
- 产物：`client/src/main/resources/assets/bong/animations/zhinian_v2.animation.json`
- 生成脚本：`modelScript/creatures/fauna_v2/gen_anim.py`（`zhinian_clips`）
- 预览目录：`model-review/anim/zhinian_v2/`（每段一张原速 GIF，3/4 单机位、20fps，按游戏约定 -Z 取景）

## 动画清单

| 动画 | 时长 | 循环 | 说明 |
|---|---|---|---|
| idle | 3.6s | 是 | 髋部轻浮、躯干缓慢前倾摆动，头部漂移中夹一次无征兆的抽动；长袍四片与三绺飘发各自错相飘动；右手每周期无征兆地攥紧剑柄一次（握紧时 forearm_r 回收、剑身一抖）——「执念」放不下的意象 |
| walk | 1.4s | 是 | 髋部驱动双腿摆动（单节腿，无膝），躯干前倾，左臂自然摆动、持剑右臂摆幅收着不乱甩；长袍拖后、飘发后甩 |
| attack | 0.85s | 否 | 右臂后撤上扬蓄力 → 躯干拧转挥臂斜劈 → 收势；左臂配合摆动平衡，长袍随挥击甩动，飘发被带起 |
| hurt | 0.42s | 否 | 躯干后仰、头部甩动，双臂一同回缩，长袍受冲击向外炸开 |
| death | 1.8s | 否 | 先一个趔趄（握剑手先一松），再整个人向前折叠塌下去、髋部下沉（由 check_ground 贴地），长袍摊平、发丝垂落；腿没有膝关节，只在髋部略向外撇 |

## 参考前例

- 骨架：`gen_rig.py` 的 `ZHINIAN_BONES`（本轮未改）。
- 任务卡：「拖地长袍的持剑残魂」——单节腿不能屈膝，走动靠髋部摆腿；长袍/飘发各自分骨才能错相摆。
- 布料 / 飘发摆动手法参照道伥 `daoxiang_rags` 的「多骨错相」思路写，具体数值从零重写，未复制任何既有创作的关键帧数值。

## 自查发现的问题

- walk 循环中整体姿态变化偏小：长袍几乎完全遮住双腿摆动，从预览图上很难一眼看出「在走」还是「站着晃」。Round 3 可以加大长袍摆动幅度或髋部左右位移，让移动感更明显。
- attack 的挥砍更像「伸臂探刺」，剑尖划过的弧度不够大；Round 3 可以把 `forearm_r` 的 z 轴分量调大，做出更清楚的横扫弧线。
- `claw_l` / `claw_r` 的独立小动作（idle 里的指节轻颤）幅度很小，在实机小尺寸下大概率看不出来，可能是可以砍掉的无效细节。
