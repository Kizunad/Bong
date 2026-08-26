# modelScript

Bong 的**离线 3D 资产工具链**：程序化生成 Blockbench `.bbmodel`、烘焙动画、headless 渲染核验、导出成
client 能吃的 GeckoLib / vanilla ModelPart 资产。

纯离线，不参与运行时。运行时真相始终在 `client/` 那边（`ArmorPartModel.CUBE_TABLES` 的 Java 表、
`assets/bong/geo/*.geo.json`），这里产出的是**作者资产和校对图**。

```
modelScript/
├── core/         共用底座：渲染器 / 骨骼动画 / 装备骨架 / 格式转换
├── generators/   gen_*.py —— 一个脚本一件资产，程序化建模
├── creatures/    生物流水线：骨架 → 肌肉 → 皮毛 → 动画，每种一个子包
├── exporters/    bbmodel → GeckoLib geo.json / 贴图，写进 client 资源树
├── tools/        摆位预览、变体派生、手绘稿改造
├── tests/        466 例，CI 在 build-resourcepack.yml 里跑
├── manifests/    人写的特征清单（*.manifest.toml），点名器照它核对
├── assets/       手工输入：参考图 + Tripo 生成索引（入库）
├── models/       .bbmodel 产物与手工源（大宗产物 gitignored，见下）
└── out/          渲染产物：三视图 / 动画 GIF / 拼版（全部 gitignored）
```

## 快速上手

```bash
# 出一件资产（生成器自己写 models/，顺带渲三视图到 out/）
python3 modelScript/generators/gen_hide_armor.py

# 看 bbmodel 真长相（别只信平涂示意图）
python3 modelScript/core/render_bbmodel.py modelScript/models/Workbench.bbmodel

# Round 2 人工闸门：六视角接触表 + 点名 + 差分自证，出完停下等人看
python3 modelScript/tools/contact_sheet.py modelScript/models/GrassPouch.bbmodel \
    --gates gen_grass_pouch --prev modelScript/out/GrassPouch_round1.bbmodel

# 单独跑点名器 / 单独跑门禁的差分自证
python3 modelScript/core/manifest.py modelScript/models/GrassPouch.bbmodel
python3 modelScript/generators/gen_grass_pouch.py --self-test

# 把护甲戴到真 MC 玩家模型上，查有没有漏盖
python3 modelScript/tools/preview_armor_on_body.py gen_hide_armor --slot chest --coverage

# 生物流水线（顺序不能乱：骨→肌→皮→动画）
python3 modelScript/creatures/kekeda_goose/gen_skeleton.py
python3 modelScript/creatures/kekeda_goose/gen_muscle.py
python3 modelScript/creatures/kekeda_goose/gen_plume.py
python3 modelScript/creatures/kekeda_goose/gen_anim.py

# 导出给 client
python3 modelScript/exporters/export_coffin_assets.py

# 测试
python3 -m unittest discover -s modelScript/tests -p "test_*.py"
```

## core/ ——共用底座

| 模块 | 干什么 |
|------|--------|
| `render_bbmodel.py` | z-buffer 软渲染 bbmodel（几何 + UV + 内嵌贴图）。视角名与角度由 `framing` 按声明朝向派生，三视图共用一个固定取景。`shading="mc"` 用 MC 的六面明暗 |
| `framing.py` | **诚实的视角命名 + 固定取景**。资产声明 `facing`（+z/-z/+x/-x），六视角名由它派生，标签写出实际照到的轴面；`focus_for()` 出一组视角共用的取景，跨图跨轮才比得动 |
| `manifest.py` | **人写的特征清单 + 点名器**。差分判据（抽掉该特征前后变了多少像素），自带遮挡正确性；材质普查走色相单位向量分类 |
| `gatekit.py` | 七道几何门（孤儿/越界/退化/悬空/穿模/贴面就位/镜像），**每道门自带缺陷注入器** |
| `animcore.py` | 动画层公共底座：旋转/曲线/关键帧落盘。`animkit` 与 `anim_rig` 都转发到它 |
| `animgate.py` | 动画后验的通用判据（贴地/滑步/循环接缝/逐帧互穿/链断裂/质心平衡）+ 注入器 |
| `render_player_pose.py` | vanilla 玩家模型的真 cuboid + `bend` 变形渲染，用来 headless 迭代 PlayerAnimator 姿态 |
| `pose_render.py` | 给一组 `{骨名: [rx,ry,rz]}` 就烘焙出静帧 |
| `armor_model_common.py` | 护甲专用：`Cube` / `ArmorPart` / mount 枢轴表 / box-UV 计算 / `write_material_assets` 一把出四件 |
| `animkit.py` · `anim_rig.py` · `voxel_rig.py` · `rigkit.py` | 骨树正解/逆解、关键帧轨道、体素生物调色板与 Rig 容器。前两者的公共部分已并入 `animcore`；`creatures/{dainu_lion,horse}/rig.py` 里还各有一份 `rotmat`/`euler`/`affine`，是已知的剩余重复 |
| `to_fmt410.py` | bbmodel 5.0 → 4.10 降级。**5.0 在 4.x 里打开是一个 cube 都看不见**，不报错，只是空场景 |
| `bbmodel_to_geckolib.py` | 驱动 web Blockbench 官方 codec 做转换（Playwright），不手搓格式 |

## models/ ——什么入库什么不入库

`.gitignore` 的规则是：**大宗生成物不入库，"没有生成器的手工源"必须入库。**

| 入库 | 内容 |
|------|------|
| `models/*.bbmodel`（45） | 顶层小件。含 Blockbench 手改稿（`format_version 5.0`）——生成器只写 4.10，**5.0 一律是手改过的，重跑生成器会覆盖掉** |
| `models/*.png`（9） | 手工贴图：斗笠 / 蓑衣 / 草包 / steve 参考皮等 |
| `models/handmade/` | 生物目录里捞出来的 fmt5.0 手改件与 `*.user-backup-*` |
| `models/armor/warhelm/` | 战盔全套，**没有生成器**，删了就没了 |
| `models/{baolongwang,lootcrate,rat}/` | 小件资产集 |

| 不入库 | 理由 |
|--------|------|
| `models/{dainu_lion,fuyu_vulture,horse,kekeda_goose,mimic_spider}/` | 生物流水线产物，动辄上百 M；跑 4 步脚本就从零重建（已实测） |
| `out/` | 全部渲染产物；不入库意味着**干净 checkout 上它不存在**，落盘前一律 `mkdir(parents=True, exist_ok=True)` |

改 `.gitignore` 加新目录时记住这条判据：**问"删了能不能用脚本重造出来"，不能就必须入库。**

## 踩过的坑

- **1 texel ≈ 1 单位**：护甲 cube 的贴图分辨率就这么低，任何"逐 texel 的描边/虚线/交替缝线"放大后
  都是棋盘格。细节要么做成几何，要么别做。
- **共面 z-fighting**：两块 cube 同向外表面落在同一平面且投影相交 → 渲染器逐像素乱挑 → 一片高频噪点，
  极容易被误读成"贴图脏了"。`gen_hide_armor.py` 里有个 `_assert_no_coplanar_faces()` 守卫，**必须跨
  mount 比**（左右腿是两个 mount，但静止姿下已在同一片世界空间）。
- **身体表面是精确平面**：臂顶 y=24、脚底 y=0。甲片停在那个值 = 和身体面共面打架。而且正交三视图
  **结构上看不见水平面**，只能靠逐面采样（`preview_armor_on_body.py --coverage`）查出来。
- **`ArmorPartModel.Mount` 只有六个**：`HEAD / BODY / LEFT_LEG / RIGHT_LEG / LEFT_FOOT / RIGHT_FOOT`。
  **没有手臂 mount**（肩甲只能挂 BODY，不跟手臂摆动）；**LEGS 没有 BODY mount**（胯带必须按腿劈开，
  跨步会错位，跨腿的绳结构做不出来）。
- **复杂模型分部件做**：拆 `part_base()` / `part_body()` / … 逐件单独预览，最后 `all_cubes()` 拼接。
  整件一把梭会埋掉单件缺陷。
- **PlayerAnimator 四大库坑**见 `docs/player-animation-conventions.md`，写动画前必读。
- **`yaw=180` 渲的是 −z 面**：背面剔除只画法线转到 +z 的面。视角名一律走 `framing`，别再手写角度表。
- **自动取景不能跨图比较**：`render()` 不传 `focus` 时每张图各算各的包围盒中心与缩放。要比就传
  `framing.focus_for()` 出来的公共取景。
- **渲出来的 PNG 读不回来**：文件工具读不了图像内容，验证只能靠 PIL/numpy 逐像素采样；把采样结果
  打成人类可读的数字表。数材质像素用**色相单位向量**（归一化后比距离，`< 0.035~0.05` 判命中）——
  绝对亮度阈值在不同光照/朝向下会失效，色相不会。

## 与主仓的接线

脚本用 `Path(__file__).resolve().parents[N]` 定位仓库根，直接读写 `client/` 和 `server/` 的资源树：

- 读配方约束：`server/assets/craft/recipes/**`（决定一件甲能由哪些材料构成）
- 写运行时资产：`client/src/main/resources/assets/bong/{geo,textures}/**`
- **改了 client 资源就得同步资源包 sha1**（`client/resourcepack/manifest.json` +
  `server/src/network/resourcepack.rs`），否则 Build resource pack CI 红

CI 触发器在 `.github/workflows/build-resourcepack.yml`，`paths:` 盯着 `modelScript/**`。

## 视觉资产纪律

模型、贴图、布局这类东西**禁止一把 commit**：Round 1 first cut → **Round 2 人工闸门** →
Round 3 终轮，commit message 标 `(round N/3)`，终轮末尾写 `<PROMISE>` 担保块。纯逻辑改动不适用。

### Round 2 是人工闸门，不是模型自评

原本 round 2 写的是「模型自评」。改掉的理由是两次实测，**都恰好发生在自评这一步**：

- **视角标签会骗人。** `yaw=180` 名义叫 FRONT，实渲的是模型 −z 面。小草包的骨扣长在 +z 前檐上，
  于是「正面看不见骨扣」这个**假 bug** 让人连试三个亮度阈值（`r>120`、`r>95`、`mean+20`），全在
  错的视角上找一个本就不该出现的东西 —— 几何、UV、材质从头到尾都是对的。
- **参考图特征会被整件丢掉，而所有数值门都是绿的。** 小草包前两轮**整件漏掉背带**（参考图里画面
  占比仅次于包身，没它那件读作「放在地上的篮子」而不是穿戴容器），当时七道门全绿：有没有背带
  根本不在任何一道门的问题域里。

人看图三十秒能发现「背带呢」，模型跑四十分钟数值门也发现不了。所以 round 2 固定产出**一张给人看
的接触表**，然后**停下等人一句话**：

```bash
python3 modelScript/tools/contact_sheet.py <模型.bbmodel> --gates <生成器模块> --prev <上一轮.bbmodel>
```

表里四样，缺一不可：

1. **六个诚实命名的视角**（FRONT/BACK/SIDE_L/SIDE_R/3-4/TOP），标签上写出这一张实际照到的轴面，
   全部共用一个固定取景；
2. **上一轮的同一批视角、同一个取景** —— 自动取景下每张图各算各的包围盒，跨轮对比全是噪声；
3. **manifest 点名结果** —— 人写的特征清单，缺一项就红；
4. **门禁的差分自证结果** —— 报不出自己该抓的缺陷的门，算失效。

工具**不替人做那个判断**。数值门和点名器只能回答「有没有」，回答不了「像不像、好不好看、是不是
那个东西」。任何「让模型自己判断像不像参考图」的设计都是错的 —— 那是自己出题自己判卷。

### 特征清单必须人写

`modelScript/manifests/<Asset>.manifest.toml` 声明这件资产**必须在哪几张图上看得见什么**：

```toml
facing = "+z"        # 正面朝哪个轴
mirror_x = 8.0       # 中轴 x（居中建模空间 0；平移进方块空间的资产 8）
size = 260           # 门限所依据的渲染边长（min_px 跟着面积缩放，必须和它锁在一起）

[features]
shoulder_strap = { elements = ["strap_"], must_show_in = ["FRONT", "SIDE_R"], min_px = 1500, mirror = true }
side_pocket    = { elements = ["pocket_", "sprig_"], must_show_in = ["SIDE_R"], asym = "right" }
```

点名器只**核对**这份清单，没有任何「从参考图/从模型自动推断该有哪些特征」的路径。
判据用差分：某特征的上镜量 = 完整模型的图与抽掉该特征后的图之间变了多少像素 —— 自带遮挡正确性
（被挡住的件贡献就是 0），也免疫光照（两次渲染条件相同）。范本见 `manifests/GrassPouch.manifest.toml`。

### 「自检全绿」在做差分注入之前，信息量是零

判据本身会假绿，而模型不会怀疑它。两个实证：

- 某版穿模判据的材质白名单是反的（把「柱头扎穿皮盖」这个正要抓的缺陷放行、去抓合法的
  bamboo×weave），**坏版本和修好的版本都报 17 处违例** —— 零区分力，而两边都「有输出」。
- 第一版编带判据统计「明度翻转」，对 11 条带数出 82 flips —— 数的是贴图颗粒不是编缝。换成按行
  连通域数暗带后：3/4 视 seam 3460px 分 7 道，抽掉 `band_*` 件后掉到 264px 分 2 道。**13 倍差距
  才叫有区分力。**

所以 `gatekit` 的每道门旁边就是它的注入器，`--self-test` 先注入缺陷再跑，报不出违例的门直接算
失效。动画侧同理，见 `animgate`。新写判据时先问一句：**把它该抓的东西造出来，它报得出来吗？**
