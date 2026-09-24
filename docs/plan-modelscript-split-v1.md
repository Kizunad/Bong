# plan-modelscript-split-v1

**主题**：把 `modelScript/` 拆成「Bong 内的纯资产生成脚本」+「独立 public repo 的通用建模/渲染库」，Bong 侧只调用不实现。

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | 边界固化：`Workspace` 抽象消灭 `REPO = parents[2]`，掰正 core → `client/tools/` 反向依赖 | ✅ 2026-08-30 |
| P1 | 新 repo `Kizunad/bbmodel-maker`（public）：filter-repo 抽子树历史 → `bbmodel_maker` 包 → 发 `v0.1.0` | ✅ 2026-08-30 |
| P2 | Bong 接依赖：`pip install git+...@v0.1.1`，89 个文件换 import，golden 回归 | ✅ 2026-08-30 |
| P3 | 收口：删已搬走的 `core/`+`tools/`，改 CI，README 拆两半 | ✅ 2026-08-30（与 P2 合并执行，理由见下） |
| P4 | （机会主义，不设期限）25 个 SOLO generator 按需收编到库 API | ⬜ |

---

## 接入面

> `modelScript/` 是**纯离线工具链，不参与运行时**（README L5）。docs/CLAUDE.md §二 的 worldview 锚点 / qi_physics 锚点两项 **N/A**——本 plan 不新增任何 gameplay 组件、event、schema，不触碰真元流动。

- **进料**：`modelScript/assets/refs/`（参考图）、`modelScript/manifests/*.manifest.toml`（人写特征清单）、`models/` 下的 fmt5.0 手改稿
- **出料**：`modelScript/models/*.bbmodel`（作者资产）、`modelScript/out/`（校对图，gitignored）、`exporters/` 写进 `client/src/main/resources/assets/bong/geo/`
- **共享类型**：不新增。库暴露的 `Workspace` / `Cube` / `ArmorPart` / `Rig` 都是既有 `core/` 类型的原地提升
- **跨仓库契约**：
  - Python 包名 `bbmodel_maker`，git tag `v0.1.0`
  - 环境变量 `BBMODEL_PROJECT_ROOT`，配置文件 `<repo-root>/bbmodel.toml`
  - `modelScript/requirements.txt`（**新建**，现状是 `build-resourcepack.yml:56` 内联 `pip install numpy pillow`）
  - MC 资源命名空间由 `namespace="bong"` 默认参数注入（现硬编在 `core/armor_model_common.py:157`、`core/held_item_common.py:606,696,712`）
- **不涉及**：server / agent / client 运行时代码零改动（`exporters/` 的输出路径不变）

## 现状测绘（P0 的输入，已实测）

| 层 | LOC | 去向 |
|---|---|---|
| `core/` 18 模块 | 6797 | → 库 |
| `tools/` 10 CLI | 3472 | → 库（7 个）/ 留 Bong（3 个） |
| `generators/` 40 个 | 22199 | 留 Bong |
| `creatures/` 6 包 | 35872 | 留 Bong |
| `exporters/` 5 个 | 936 | 留 Bong |
| `tests/` 25 个 | 7850 | 4414 → 库 / 3436 留 Bong |

**generators 依赖现状**（origin/main 上共 **40** 个）：16 个 import core（`gen_{back_basket,bone_armor,bone_blocks,club_player_anim,grass_pouch,hide_armor,iron_armor,jian_player_anim,jian_player,knife_trio,linen_armor,mutated_bone_armor,remnant_scroll,scroll_wrap_armor,straw_armor,wooden_club}.py`），**25 个完全自给**——各自手抄 `build_bbmodel`(×18) / `make_texture`(×17) / `png_data_url`(×14) / `cube_faces_uv`(×14) / `stable_uuid`(×11)。P4 才碰它们。

## P0 —— 边界固化 ✅ 2026-08-30

目标：跑完 P0 后 `core/` 不再依赖「自己住在 Bong 仓库的第几层」，也不再反向依赖 `client/`。

### 已落地

1. **`modelScript/core/workspace.py`** —— `Workspace` dataclass，`root` / `lib` / `namespace` /
   `client_resources`，派生 `models` / `out` / `assets` / `manifests` / `client_assets` /
   `player_animations`；`rel()` 缩短打印路径，`resolve_texture()` 三级候选解析。
   发现顺序 `显式 → $BBMODEL_PROJECT_ROOT → bbmodel.toml → .git → cwd`，
   **`.git` 是文件时（git worktree）同样认**。`modelScript/tests/test_workspace.py` 27 例。
2. **仓库根 `bbmodel.toml`** —— 声明 `lib` / `namespace` / `client_resources`，
   让根解析从「靠 .git 猜」变成「按配置定」。
3. **`REPO = parents[2]` 在 `core/` 与 `tools/` 清零**：`render_bbmodel` `render_player_pose`
   `contact_sheet` `bbmodel_to_pose` `held_item_pose` `preview_player_anim`
   `preview_armor_on_body` `render_jian_in_hand` 全部改走 `Workspace`。
   （`generators/` 的 35 处**有意保留**——它们永远住在 Bong 里，`parents[2]` 对它们正确且稳定。）
4. **掰正 core → client 反向依赖**：`client/tools/render_animation.py` 里的
   PlayerAnimator/Emotecraft 关键帧采样（`collect_keyframes` / `sample_part` / `sample_axis` /
   `apply_easing` 等）提进 **`modelScript/core/emote_anim.py`**，原处改为转发。
   提取用 300 个随机 emote × 21000 次采样与原实现对拍，**零差异**。
5. **`namespace` 参数化**：`armor_model_common.build_bbmodel` /
   `held_item_common.{build_bbmodel,build_mtl,build_model_json,write_assets,assert_host_is_claimable}` /
   `rigkit.*.bbmodel` / `voxel_rig.*.bbmodel` / `bbmodel_to_geckolib --namespace`
   全部接受 `namespace: str | None = None`，None 时取 `workspace.default().namespace`。
   `modelScript/tests/test_namespace_param.py` 11 例。
6. **golden 字节回归**（`tests/golden_runner.py` + `tests/test_golden_bytes.py` +
   `tests/fixtures/golden_bbmodel.json`）：40 个生成器 / 83 个产出文件的 uuid 归一化 sha256 基线。

### 验收证据

- `python3 -m unittest discover -s modelScript/tests -p "test_*.py"` → **661 tests, OK**
- golden 自证：连续两轮采集结果完全一致
- golden 变异验证：注入 0.01 几何改动（`gen_workbench` 腿底面 y 0.0→0.01）后精确报红，还原即绿
- 全部重构完成后 golden 仍绿 ⇒ **40 件资产逐字节未变**

### P0 期间发现的既有缺陷（均非本次引入）

| 发现 | 处置 |
|---|---|
| 10 个生成器在干净 checkout 上崩：往 gitignored 的 `out/` 落盘前漏 `mkdir` | **已修**（commit 88881a85a）。共享沙箱只暴露 2 个，逐个独立沙箱才全暴露 |
| `core/pose_render.py` 缺 `from pathlib import Path`，自 #2079 起不可导入 | **已修** |
| 铜/玉/凡棺已入库的是 fmt 5.0 手改稿，生成器写 4.10，重跑**静默覆盖**（`gen_jian_player` / `gen_herb_crate` 有守卫，这三个没有） | 仅记录，未动。补守卫是独立议题 |
| 赤髓草 / 化形根 / 守心草：生成器贴图已升 128×128，已入库 bbmodel 仍 64×64 | 仅记录，未动 |

### 有意推迟到 P1 的

- **`sys.path.insert` 的清除**（30+ 处）：它和「排成 `bbmodel_maker.*` 五个子包」是同一件事，
  P1 搬家时一次改完；P0 先改一遍等于做两遍。
- **`tools/` → `client/tools` 的反向依赖**：`preview_player_anim` / `held_item_pose` 借的不只是
  采样，还有 bendy 骨架数学（`solve_skeleton` / `CUBOIDS`）和 `anim_common`。这三块要一起搬，
  blast radius 比 P0 该吞的大。**core/ 的那条已断干净**。
- **`workspace.DEFAULT_NAMESPACE = "bong"`**：库拆出去后默认值不该是某个具体项目的命名空间。
  P1 决定是改中性默认还是缺配置即报错。

## P1 —— 新 repo `Kizunad/bbmodel-maker`（public）✅ 2026-08-30

仓库：https://github.com/Kizunad/bbmodel-maker · tag `v0.1.0` · CI `test` 绿（py3.11 + 3.12）

### 已落地

1. **历史抽取**：`git filter-repo` 按 47 条路径（35 现行 + 12 历史）抽子树，
   `--path-rename` 到 `src/bbmodel_maker/`。**19 个提交，`git log --follow` 可回溯到
   #394/#395**，跨过 2026-08-23 的 `scripts/models → modelScript` 重组。
2. **五个子包**：`workspace`（跨层）/ `model` / `rig` / `render` / `gates` / `workbench`，
   24 个模块。`render_jian_in_hand` → `render/held_item_render.py`。
3. **35 处 `sys.path.insert` 全部换成真 package import**（P0 有意推迟的那项，在此一次改完）。
4. **`pyproject.toml`**：依赖 `numpy` + `pillow`，`[geckolib]` extra 才要 `playwright`；
   5 个 console_scripts（`bbmodel-render` / `bbmodel-contact-sheet` / `bbmodel-armor-preview` /
   `bbmodel-to-pose` / `bbmodel-to-geckolib`），入口全部验证可解析可运行。
5. **测试 248 例**：animcore 43 / animgate 69 / framing 38 / gatekit 60 / workspace 27 /
   namespace 11。
6. **CI**：py3.11 + 3.12 矩阵，额外一步「**空目录下 import 全部模块**」——库不该假设调用方
   配了 `bbmodel.toml`。
7. README（含从 Bong 带过来的踩坑清单）+ MIT LICENSE。

### 验收证据

- 干净 venv 里 `pip install git+https://github.com/Kizunad/bbmodel-maker@v0.1.0` 成功，
  **29 个模块在仓库之外全部可导入**，`bbmodel-render --help` 可运行
- 新 repo CI `test` 绿

### P1 期间发现并修掉的

| 发现 | 处置 |
|---|---|
| `bbmodel_to_pose` 的 `ANIM_DIR = _WS.player_animations` 在**模块级**求值。Bong 里配了 `client_resources` 所以没暴露；独立库里没配就**连 import 都失败**，而该模块另一半功能根本用不着它 | 改成惰性 `anim_dir()`；CI 加「空目录 import」步骤钉死这类回归 |
| `DEFAULT_NAMESPACE = "bong"` —— 公共库的兜底值不该是某个具体项目的名字 | 改为 `"minecraft"`。Bong 侧不受影响（`bbmodel.toml` 显式声明了 `namespace = "bong"`） |
| `test_namespace_param.test_repository_default_is_bong` 搬进库后**靠兜底值偶然通过**，等于没测 | 重写成「兜底值必须是中性的」 |

### 未搬（连同理由）

- **6 个测试文件留在 Bong**：`test_manifest` / `test_contact_sheet` / `test_gatekit` 的
  `MigratedGeneratorsTest` / `test_anim_retime` / `test_bb_anim_roundtrip` /
  `test_preview_armor_on_body`。它们拿**真实资产和生成器**当 fixture（某个具体的草包 /
  背篓 bbmodel、`gen_hide_armor`、client 的动画 JSON），是「库 × 调用方资产」的集成测试。
  库这边缺等价的**合成 fixture**，已写进新 repo README「已知缺口」。
- **`preview_player_anim` / `held_item_pose` 两个工作台入口**：链式依赖
  `client/tools/anim_common.assert_joint_fold_is_anatomical` 与 `render_animation` 的
  biped 骨架数学（`bend_center` / `bent_end_local` / `part_rotation_matrix` /
  `rotate_about_axis` / `rot_x|y|z`）。那批是通用 MC bendy-lib 变换数学，该搬，但要连
  `anim_common` 一起，放 **v0.2**。

## P2 + P3 —— Bong 接依赖并删除本地副本 ✅ 2026-08-30

> **两阶段合并执行**。原计划 P2 只换 import、P3 才删本地副本，但中间态会让同一批模块
> 在进程里存在**两个副本**（`framing` 与 `bbmodel_maker.render.framing` 是两个不同的
> module 对象）。一旦有代码在两侧之间传对象，`isinstance` 就会莫名失配——这种 bug
> 极难定位，为省一次 review 的分割去承担它不划算。

### 已落地

1. **`modelScript/requirements.txt`**：`bbmodel-maker @ git+…@v0.1.1` + numpy + pillow。
   **版本锁死在 tag 上**，库改动不会静默改变本仓资产产出。
2. **89 个文件换成真 package import**（generators 16 / creatures 40 / exporters 1 /
   tools 6 / tests 18 + client/tools/render_animation.py）。
3. **删除本地副本**：`modelScript/core/` 全部 19 个模块 + 5 个已迁移 tool
   （`bbmodel_to_pose` `contact_sheet` `make_variants` `preview_armor_on_body`
   `render_jian_in_hand`）。
4. **`tools/` 只剩 5 个 Bong 专属**：`gen_pipeline_refs`（读 `scripts/images/.env` 出参考图）、
   `transform_laoshu`、`recolor_animate_rat`、`preview_player_anim`、`held_item_pose`
   （后两个依赖 `client/tools/anim_common` 的关节解剖判据与 biped 骨架数学，随 v0.2 一起搬）。
5. **CI**：`build-resourcepack.yml` 的 `pip install numpy pillow` → `pip install -r
   modelScript/requirements.txt`。`scripts/test-all.sh` 的 discover 路径不变，无需改。
6. **README 拆两半**：`modelScript/README.md` 只留「Bong 有哪些资产、每件怎么生成」
   + 本仓特有的坑（Mount 只有六个 / 身体表面是精确平面 / 共面必须跨 mount 比 / fmt5.0
   手改稿 / out 目录 mkdir）；通用底座与通用坑指向新 repo。
7. 顺带把 9 个文件里指向已删路径的注释/文档改成新位置
   （creatures 的 rig 说明、`docs/asset-modeling-and-reference-pipeline.md` 等）。

### 验收证据

- `python3 -m unittest discover -s modelScript/tests -p "test_*.py"` → **661 tests, OK**
- **golden 全绿** ⇒ 40 个生成器改用抽出去的库之后，**产出逐字节未变**

### P2 期间发现的（都是「只有真装出去才显形」的）

| 发现 | 处置 |
|---|---|
| **库 bug**：`manifest.py` 的 `MANIFEST_DIR = Path(__file__).parents[1] / "manifests"` 装进 site-packages 后指向包内部，报「清单必须人写」把责任推给人 | 库侧改走 `workspace.manifests` 并加 3 例钉住，**发 v0.1.1**，本仓 requirements 跟着升 |
| **golden 沙箱不忠实**：沙箱只复制 `modelScript` + `client/tools` + 动画 JSON，**漏了仓库根的 `bbmodel.toml`**。库的命名空间兜底是中性的 `minecraft`，于是产出的 namespace 整体漂掉——而这种漂移在渲染图上完全看不出来 | 沙箱树补上 `bbmodel.toml`；golden 由红转绿 |
| 改写脚本把 `for _d in (a, b):` 的元组删剩一项后**丢了尾逗号**，`("generators")` 变成字符串，迭代它是逐字符往 sys.path 塞 `'g'/'e'/'n'`——**不报错，静默失效** | 4 个文件补回逗号；后续所有批量改写都加了 `ast.parse` 自检 |
| `client/tools/render_animation.py` 仍指着已删的 `modelScript/core` | 改为直接 import 库 |
| `test_jian_player_tools` 按文件路径起子进程跑 CLI，路径已变 | 加 `_module_script()` 按已安装模块解析真实路径 |

### `CLAUDE.md` 的 3 处失效引用 —— 已按用户批准修正

P3 删掉 `modelScript/core/` 与 5 个 tool 后，`CLAUDE.md` 的「视觉资产纪律」一节有 3 条
命令失效。仓库规矩是 `CLAUDE.md` 只人工改，故先列出交人工；**用户 2026-08-30 批准后**
在本 PR 内一并改掉：

| 行 | 原 | 现 |
|---|---|---|
| L252 | `python3 modelScript/tools/contact_sheet.py <模型> --gates … --prev …` | `bbmodel-contact-sheet <模型> --gates … --prev …` |
| L255 | `modelScript/core/gatekit.py` 每道门旁边就是它的注入器（`animgate.py`） | `bbmodel_maker.gates.gatekit`（`animgate`） |
| L256 | bbmodel 真长相用 `modelScript/core/render_bbmodel.py` 看 | bbmodel 真长相用 `bbmodel-render <模型>` 看 |

只改路径，**规则本身一字未动**（Round 2 仍是人工闸门、清单仍必须人写、门禁仍必须先做
差分注入）。

另有两份 **skeleton** plan 引用旧路径，按「一个 PR 只动一个 plan」未动：
`plan-split-body-animation-v1.md`（`modelScript/core/render_player_pose.py`）、
`plan-held-item-registration-v1.md`（`modelScript/core/held_item_common.py`）。
它们是草案，升 active 时顺手改即可。

## P4 —— SOLO generator 收编（机会主义）

25 个自给 generator **不设期限、不作为本 plan 的完成条件**（用户 2026-08-30 拍板：「solo gen 能起到效用就好，代码整洁度优先级不高，之后按照流程来就行」）。今后谁碰哪件资产，顺手把它的 `build_bbmodel` / `make_texture` 换成库 API，逐件 golden 比对。

## 风险

- **产物字节漂移**：uuid 生成顺序、贴图像素、face 顺序差一点，重跑生成器就覆盖掉已入库 bbmodel。`test_golden_bytes.py` 是 P0 的第一件事，不是最后一件
- **fmt5.0 手改稿**：`models/` 里 `format_version 5.0` 的一律是 Blockbench 手改过的，生成器只写 4.10，**重跑会覆盖**（README「models/ 什么入库什么不入库」）。golden 测试要跳过这批
- **本地改库的回路**：版本锁死的代价是本地迭代要 `pip install -e ../bbmodel-maker`。P1 的 README 要写清这条
