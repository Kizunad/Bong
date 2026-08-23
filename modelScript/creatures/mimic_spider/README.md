# 拟态灰烬蛛（mimic_spider）建模/动画流水线

worldview §七:731 拟态灰烬蛛。外骨骼两层建模 + 逆解动画，设计从拟态态倒推：
伪装态 client 渲**真方块**（拟态方块可配置——落地采样 + 白名单），模型只需在
折叠姿收进 16×16×16 方块体积；暴起瞬间切回模型 + 该方块的破碎粒子。

## 生成链（顺序执行，每步有断言）

```bash
python3 gen_frame.py            # ① 节肢框架：43 骨 101 cube，落地/镜像/高拱自检
python3 gen_shell.py            # ② 甲壳层（最终进游戏层）：+88 件，唯一暖色=八眼橙
python3 preview.py              # ③ 框架折叠断言（16³ − 0.5 甲壳预留）+ 渲染
python3 preview.py --model shell  # ④ 甲壳折叠断言（16³ 实测）
python3 gen_anim.py             # ⑤ 九条动画 → MimicSpiderRig.bbmodel + geckolib json
python3 check_anim.py           # ⑥ 动画物理断言（滑步/接缝/burst/fold/bite/idle/death）
python3 render_anim.py --only walk --view 34 --gif   # 连拍/GIF 预览
```

产物在 `modelScript/models/mimic_spider/`（gitignored）；进游戏走
`bbmodel_to_geckolib.py`（官方 codec）→ `client/.../geo|animations/`，原地替换
`ash_spider` id 保全部接线。

## 文件

| 文件 | 职责 |
|------|------|
| `gen_frame.py` | 框架层：体块/螯肢/触肢/8 腿×4 节；再生短腿(4_r) 0.8 缩比防"修对称"断言 |
| `gen_shell.py` | 甲壳层：穹顶/灰烬霜/眼球(内嵌防粘连)/腿甲/刚毛(再生腿光秃) |
| `spider_rig.py` | 二连杆闭式腿逆解 + 折叠姿定义 + 共轭旋转(方向→Blockbench 欧拉) |
| `gen_anim.py` | 九条动画：burst/bite/walk/run/retreat/idle/fold/hurt/death，恐惧参数内联 |
| `check_anim.py` | 恐惧参数变断言：burst 首帧=折叠姿、突刺≥蓄力 2.5×、idle 腿死静等 |
| `preview.py` | 折叠 16³ 包围盒断言 + 姿态渲染 |
| `render_anim.py` | 连拍图 / GIF（固定取景 + 地平线） |

## 恐惧设计三杠杆（动画的存在理由）

1. **静与爆**：伪装=真方块绝对静止；burst 5 tick 蓄势压缩→过冲 15%→弹定，
   首帧即正对目标（无转身——"它早就知道你在哪"）。
2. **被注视**：八眼不对称簇、唯一暖色；折叠姿里眼朝外。头追踪/freeze-and-stare
   由引擎层做（GeckoLib head bone / 循环间切 idle）。
3. **不追的冷**：bite 命中后引擎接 fold——当着断经猎物的面从容叠回一块方块。

## 已知边界

- 折叠姿甲壳余量 x 1.08 / y 0.53 / z 0.14——再加大甲壳件前先跑 ④。
- retreat 的 freeze-and-stare、burst 的 GeckoLib transition=0、方块渲染切换时机
  均为 client/server 接线层职责（见后续 plan）。
