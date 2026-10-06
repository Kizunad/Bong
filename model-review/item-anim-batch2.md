# item-use-anim 第 2 批 Round 1：青锋剑 / 灵剑 / 飞旋剑攻击动画（2026-10-06）

状态：Round 1 完成，三件 gate-waiting，等调度放上审阅页、用户看图。未接线、未开 PR。
分支：`feat/item-use-anim-batch2`（基于 origin/main 494ed14e6）。

| 物品 | template_id | 动画 id | 动作 | 官方刃线判据 | 状态 |
|---|---|---|---|---|---|
| 青锋剑 | `qing_feng_sword` | `qing_feng_sword_use` | 水平横切（右到左），剑平于地面，8 tick | `--strike 3-5`：0.986 / 0.000，通过 | gate-waiting |
| 灵剑 | `spirit_sword` | `spirit_sword_use` | 直线前刺，剑尖前送约 1 格，8 tick | 不适用（突刺），改看剑尖前送 16.4 px、剑身朝前 0.97 | gate-waiting |
| 飞旋剑 | `flying_sword_feixuan` | `flying_sword_feixuan_use` | 自下而上回旋撩斩，腰部转动，10 tick | `--strike 2.5-4.5`：0.922 / 0.267，通过 | gate-waiting |

三件彼此不同，也与已通过的铁剑（过顶直劈）、骨剑（右上斜劈到左下）不同：
- 青锋剑：不过顶、不下劈，剑身平于肩高横扫。
- 灵剑：只有直线前送，没有扫、没有劈。
- 飞旋剑：方向向上（铁剑、骨剑都向下）。

## 产物

- 生成器：`client/tools/gen_qing_feng_sword_use.py`、`gen_spirit_sword_use.py`、`gen_flying_sword_feixuan_use.py`（只用 `anim_common.emit_json`，从零写，不抄旧动画关键帧）。
- 动画 JSON：`client/src/main/resources/assets/bong/player_animation/{qing_feng_sword_use,spirit_sword_use,flying_sword_feixuan_use}.json`。三份都没有 `rightItem` 关键帧。
- 审阅产物：`model-review/anim/<id>/`（`use.gif`、`use.png`，飞旋剑另有 `vs_ref.gif`）+ `model-review/anim/<id>.md`。该目录在 `.agent-worktrees/model-review/`，不进仓库，沿用第 1 批的位置。

## 检查方法与局限（调度需要知道的）

1. **官方 `check_blade_edge.py` 只算手臂，不算躯干。** 青锋剑与飞旋剑的挥击都有躯干转动，所以另用预览的真实手持链路（`preview_player_anim.hand_transform`，含躯干、肩、肘、roll、display）复核。两者在骨剑、铁剑上的数字接近（骨剑官方 0.941 vs 真实链路 0.927（刃身中部）；铁剑官方 0.956 vs 真实链路 0.948（刃身中部）），说明大体一致，但不是逐帧相同。
2. **突刺不适用刃线判据。** 灵剑用剑尖前送距离（≥ 1 格）与剑身朝前（≥ 0.85）代替。
3. **预览的 `body.*` 位移渲染小了 16 倍**（`modelScript/tools/preview_player_anim.py` 的 `BODY_PX_PER_BLOCK_TRUE` 注释）。所以本批的突刺没有用 `body.z` 做身体前冲，全部靠肘、肩、躯干旋转完成。
4. **预览工具依赖 `bbmodel-maker@v0.1.1`**（`modelScript/requirements.txt`）。本机在 `/home/serverkizuna/.venvs/bong-modelscript`（system-site-packages，已装 0.1.1）中运行。系统 python3 没有这个包，直接运行会报 `No module named 'bbmodel_maker'`。
5. **`vs_ref.gif` 只能在帧数相同时生成**（`compose_side_by_side.py` 的规则）。铁剑时间表是 10 tick，飞旋剑（10 tick）有并排图；青锋剑与灵剑（8 tick）没有，靠 `use.gif` 与 `use.png` 对比。

## 设计中踩过的坑（后续做挥击动画要知道）

- **roll 在 yaw 之后作用**（`Rz(roll)·Ry(yaw)·Rx(pitch)`）。手臂水平前伸（pitch -90）时，用 yaw 横扫，手速会被 roll 转成竖直方向，刃线对不上。改用 pitch 摆动 + roll -90，手速就变成水平的。
- **挥击段的手速来自肘时，刃线对不上。** 如果挥击段内 bend 大幅变化（肘伸直），握点速度沿前臂的垂直方向，主要是竖直的。挥击段内 bend 要基本不动，让肩或躯干承担速度。
- **角度归一化。** 预览与检查脚本里 display 缩放 0.8 会让刃轴向量长度为 0.8。打分前必须归一化，否则点积上限被压成 0.8，会误判为「不过」。

## 未做（按任务卡）

- Round 2 人工闸门：等调度放上审阅页，用户看 GIF。
- Round 3 与接线：`server/src/network/vfx_animation_trigger.rs` 的 `held_attack_anim` 表、`BongAnimations` 注册、表驱动测试、客户端 manifest。用户通过后再做。
- 门禁：本轮只落资产与生成器，没有改 Java / Rust 代码，未跑 gradle / cargo。Round 3 接线时再跑完整门禁。
- 未开 PR。

## 修订记录

- 2026-10-06：Round 1 首版。三件生成器、JSON、预览与 md 完成。
