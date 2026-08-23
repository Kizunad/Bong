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
├── tests/        79 例，CI 在 build-resourcepack.yml 里跑
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
| `render_bbmodel.py` | z-buffer 软渲染 bbmodel（几何 + UV + 内嵌贴图）。**FRONT = yaw 180**（背面剔除保留法线朝 +z），SIDE 90，BACK 0；正 pitch 俯视。`shading="mc"` 用 MC 的六面明暗 |
| `render_player_pose.py` | vanilla 玩家模型的真 cuboid + `bend` 变形渲染，用来 headless 迭代 PlayerAnimator 姿态 |
| `pose_render.py` | 给一组 `{骨名: [rx,ry,rz]}` 就烘焙出静帧 |
| `armor_model_common.py` | 护甲专用：`Cube` / `ArmorPart` / mount 枢轴表 / box-UV 计算 / `write_material_assets` 一把出四件 |
| `animkit.py` · `anim_rig.py` · `voxel_rig.py` · `rigkit.py` | 骨树正解/逆解、关键帧轨道、体素生物调色板与 Rig 容器 |
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

## 与主仓的接线

脚本用 `Path(__file__).resolve().parents[N]` 定位仓库根，直接读写 `client/` 和 `server/` 的资源树：

- 读配方约束：`server/assets/craft/recipes/**`（决定一件甲能由哪些材料构成）
- 写运行时资产：`client/src/main/resources/assets/bong/{geo,textures}/**`
- **改了 client 资源就得同步资源包 sha1**（`client/resourcepack/manifest.json` +
  `server/src/network/resourcepack.rs`），否则 Build resource pack CI 红

CI 触发器在 `.github/workflows/build-resourcepack.yml`，`paths:` 盯着 `modelScript/**`。

## 视觉资产纪律

模型、贴图、布局这类东西**禁止一把 commit**：Round 1 first cut → Round 2 自评（渲染截图/ASCII 投影/
数值比对）→ Round 3 终轮，commit message 标 `(round N/3)`，终轮末尾写 `<PROMISE>` 担保块。
纯逻辑改动不适用。
