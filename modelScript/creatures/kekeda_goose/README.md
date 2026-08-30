# 珂珂达（kekeda_goose）· 大白鹅

三层体素流水线做的一只鹅。**平时只需要看 `renders/1_final/`。**

```bash
python3 modelScript/creatures/kekeda_goose/preview.py            # 重生成三层 + 渲染全部视图
python3 modelScript/creatures/kekeda_goose/preview.py --skip-gen # 只重渲染
python3 modelScript/creatures/kekeda_goose/gen_plume.py --check  # 只跑成品层自检

python3 modelScript/creatures/kekeda_goose/gen_anim.py           # 生成九条动画
python3 modelScript/creatures/kekeda_goose/check_anim.py         # 动画后验（九项，见下）
python3 modelScript/creatures/kekeda_goose/render_anim.py --gif  # 连拍图 + GIF
```

## 产物在哪

渲染图 `modelScript/creatures/kekeda_goose/renders/`，按**看的目的**分目录，编号前缀让成品永远排最前：

| 目录 | 内容 |
|---|---|
| `1_final/` | 成品三视图 + 特写（脸 / 收翼 / 后侧 / 游戏观看距离） |
| `2_layers/` | 三层并排对照；半剖（左半骨+肌，右半外观） |
| `3_skeleton/` | 骨架三视图 + 特写（头喙栉板 / 蹼足 / 收翼俯视 / 龙骨） |
| `4_muscle/` | 骨+肌 / 纯软组织 / 延展视图 / 逐肌群图集 |
| `5_anim/` | 逐动画连拍（`<名字>_<视角>.png`）+ `key_poses.png` 四个关键姿特写 |

模型文件 `modelScript/models/kekeda_goose/`（gitignored）：
`KekedaSkeleton` / `KekedaMuscle`（+`_bare` `_explode` `_<肌群>`）/ `KekedaPlume`（+`_anatomy`）/
`KekedaGooseRig`（带动画）/ `kekeda_goose.animation.json`（GeckoLib，参考用未过引擎）。

## 三层各自在干什么

| 层 | 脚本 | 块数 | 作用 |
|---|---|---|---|
| 骨架 | `gen_skeleton.py` | ~302 | 按雁形目解剖建骨，定关节位置与比例 |
| 肌肉 | `gen_muscle.py` | ~80 软组织 | 定体表包络；导出 `body_profile` 等给下游 |
| 外观 | `gen_plume.py` | **40** | 进游戏的那一层，MC 原版口径 |

前两层是**设计依据**，不进游戏。它们把"这只鹅该多宽多深、腿该多长"算清楚了，
外观层才敢只用四十块就定形。

## 外观层为什么只有 40 块

走过两条弯路，都写在 `gen_plume.py` 头部，这里只记结论：

1. 羽簇版（355 块旋转小方块）→ 刺球，表面全是噪点。
2. 分带椭球版（481 块，台阶 0.86 单位）→ 圆是圆了，但那是拿几百个小台阶逼近曲面，
   体素不该这么用。
3. **原版口径（当前，40 块）**：躯干五块（主体 + 四块倒角）、头两、颈四片、喙三 +
   眼四、翼各三片平板、尾两片、腿脚各七。台阶少而大，是**设计出来的面**，不是逼近
   曲面时漏出的锯齿 —— 后者才是"块状堆叠"难看的真正原因。
   （颈原本是一块，为了动画里伸得开才改成四片，见下。）

`gen_plume.py` 的自检里焊了 `CUBE_BUDGET = 44`，超了就报违例，防止再走回头路。

## 动画（九条）

`bbmodel_maker.rig.anim_rig` 是与物种无关的底座（骨树 / 正解 / 逆解 / 关键帧导出），
`rig.py` 放鹅特有的四件事（鸟腿链与限位、两足平衡、17 节颈的两个旋钮、泄殖腔口），
`gen_anim.py` 写动作，`check_anim.py` 做后验。

| 名字 | 时长 | 说明 |
|---|---|---|
| `idle` | 4.00s 循环 | 呼吸 + 偶尔瞥一眼 |
| `walk` | 0.90s 循环 | 摇摆步 |
| `run` | 0.68s 循环 | 小跑，颈前伸 + 双翼半张扑打 |
| `honk` | 1.30s | 引颈高鸣（喙尖上到 y≈21.6） |
| `threat` | 1.80s | 威吓：颈**平伸压低** + 张翼（和 honk 的上举必须一眼分得开） |
| `poop` | 1.50s | 顿 → 蹲 → 绷 → 弹 → 抖 → 若无其事 |
| `lay_egg` | 4.20s | 察看 → 蹲坐 → 三次递强努责 → 蛋出 → 起身 → 回头看蛋 |
| `hurt` | 0.45s | 硬顿挫 + 衰减抖动，双翼炸开 |
| `death` | 2.20s | 腿软 → 胸着地 → 侧翻 → 颈瘫下去 |

**掉东西的时刻**在 `gen_anim.RELEASE`（`poop` 0.545 / `lay_egg` 0.78），生成物的世界
坐标走 `rig.Goose.vent(pose)`。客户端别自己数帧 —— 这两个数是动画节拍的一部分。

三条**不手调**的机制（改动作时别绕过它们）：

- **摇摆幅度是反解的**。给定"质心要压到支撑脚上多少"，侧倾角由 `asin` 算出来
  （质心高 8.22 / 脚距 ±1.88 → ±9.2°）。鹅之所以摇摆着走，就是因为髋距只有体宽四成。
- **质心跟的是压力中心的基频**，不是压力中心本身。CoP 在双支撑段几乎瞬移，直接追它
  会让侧倾在 0.09 秒里扫 17°，看着像抽搐。
- **原地动作一律 `settle()`**，质心自动稳在静止姿的位置上。颈是个大杠杆（伸直往前
  能把质心带出去 0.43），手写的躯干前倾角每改一次颈姿就过时。

## 三个只有渲图才看得见的坑

后验的最后三项都是先在渲图里栽了跟头才补上的 —— 物理指标全绿不等于好看：

- **头缩进肩里就没有鹅了**。首版拉粑粑/下蛋把颈缩到 `straight −0.28`，中段整只读成
  一团白。通则：**aim 压得越低，颈必须越长**（`straight ≳ (66 − aim)/200`），安全区
  表在 `rig.Goose.neck` 的文档里。
- **一整块几何跟着单根骨走，骨一转就同时脱开两头**。颈原本是一块，威吓时上不着头下
  不着身，渲出来是白方块悬空 → 改成沿颈链四片、静止姿重叠 0.75。下喙同理：`jaw` 的
  pivot 比真实铰链靠后 2.5 单位，张嘴过 20° 整根被拽出来 → 后端延进头里当口腔内壁。
- **薄片的旋转在侧视里读不出来**。尾羽就两块，翘 46° 几乎看不见 —— "翘尾"得靠**整只
  前倾撅臀**一起演。

## 两个容易栽的坑

- **判"圆不圆滑"必须用 `render(..., shading="mc")`**。`render_bbmodel` 默认的 lambert
  光让相邻朝向差 2.5 倍，会把阶梯面照成一身竖条纹；MC 原版只差 1.33 倍
  （up 1.0 / down 0.5 / 南北 0.8 / 东西 0.6）。曾误判成"截面不够密"而一路加密。
- **轴对齐盒子没有"贴面"概念，谁在前谁把谁盖死**。眼珠必须在每个要被看到的方向上
  都比头凸出去，高光再比眼珠凸一点。栽过两次（白眼圈盖住黑眼珠、半颗眼被脸埋掉）。

## 还没做

接游戏：`FaunaVisualKind`（client）+ `server/src/fauna/visual.rs` + raw_id 分配
（加一个 fauna 物种会顺移后面的 entity id）；`RELEASE` 两个时刻要接上粒子 / 掉落物，
掉落点取 `rig.Goose.vent(pose)`。bbmodel → GeckoLib 走
`modelScript/out/bbmodel_to_geckolib.py`（Blockbench 官方 codec），别直接用
`kekeda_goose.animation.json` —— 那份是参考/兜底，没过引擎侧验证。
