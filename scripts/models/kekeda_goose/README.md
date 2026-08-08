# 珂珂达（kekeda_goose）· 大白鹅

三层体素流水线做的一只鹅。**平时只需要看 `renders/1_final/`。**

```bash
python3 scripts/models/kekeda_goose/preview.py            # 重生成三层 + 渲染全部视图
python3 scripts/models/kekeda_goose/preview.py --skip-gen # 只重渲染
python3 scripts/models/kekeda_goose/gen_plume.py --check  # 只跑成品层自检
```

## 产物在哪

渲染图 `scripts/models/kekeda_goose/renders/`，按**看的目的**分目录，编号前缀让成品永远排最前：

| 目录 | 内容 |
|---|---|
| `1_final/` | 成品三视图 + 特写（脸 / 收翼 / 后侧 / 游戏观看距离） |
| `2_layers/` | 三层并排对照；半剖（左半骨+肌，右半外观） |
| `3_skeleton/` | 骨架三视图 + 特写（头喙栉板 / 蹼足 / 收翼俯视 / 龙骨） |
| `4_muscle/` | 骨+肌 / 纯软组织 / 延展视图 / 逐肌群图集 |

模型文件 `local_models/kekeda_goose/`（gitignored）：
`KekedaSkeleton` / `KekedaMuscle`（+`_bare` `_explode` `_<肌群>`）/ `KekedaPlume`（+`_anatomy`）。

## 三层各自在干什么

| 层 | 脚本 | 块数 | 作用 |
|---|---|---|---|
| 骨架 | `gen_skeleton.py` | ~302 | 按雁形目解剖建骨，定关节位置与比例 |
| 肌肉 | `gen_muscle.py` | ~80 软组织 | 定体表包络；导出 `body_profile` 等给下游 |
| 外观 | `gen_plume.py` | **37** | 进游戏的那一层，MC 原版口径 |

前两层是**设计依据**，不进游戏。它们把"这只鹅该多宽多深、腿该多长"算清楚了，
外观层才敢只用三十几块就定形。

## 外观层为什么只有 37 块

走过两条弯路，都写在 `gen_plume.py` 头部，这里只记结论：

1. 羽簇版（355 块旋转小方块）→ 刺球，表面全是噪点。
2. 分带椭球版（481 块，台阶 0.86 单位）→ 圆是圆了，但那是拿几百个小台阶逼近曲面，
   体素不该这么用。
3. **原版口径（当前，37 块）**：躯干五块（主体 + 四块倒角）、头颈各一、喙三、
   翼各三片平板、尾两片、腿脚各五。台阶少而大，是**设计出来的面**，
   不是逼近曲面时漏出的锯齿 —— 后者才是"块状堆叠"难看的真正原因。

`gen_plume.py` 的自检里焊了 `CUBE_BUDGET = 44`，超了就报违例，防止再走回头路。

## 两个容易栽的坑

- **判"圆不圆滑"必须用 `render(..., shading="mc")`**。`render_bbmodel` 默认的 lambert
  光让相邻朝向差 2.5 倍，会把阶梯面照成一身竖条纹；MC 原版只差 1.33 倍
  （up 1.0 / down 0.5 / 南北 0.8 / 东西 0.6）。曾误判成"截面不够密"而一路加密。
- **轴对齐盒子没有"贴面"概念，谁在前谁把谁盖死**。眼珠必须在每个要被看到的方向上
  都比头凸出去，高光再比眼珠凸一点。栽过两次（白眼圈盖住黑眼珠、半颗眼被脸埋掉）。

## 还没做

绑定与动画（`rig.py` / `gen_anim.py`）、接游戏（`FaunaVisualKind` +
`server/src/fauna/visual.rs` + raw_id 分配）。骨架已按"两种剪影"留好机关：
收着是团子，威吓时颈弹直、双翼张开、张嘴亮出喙缘栉板。
