# plan-modelscript-split-v1

**主题**：把 `modelScript/` 拆成「Bong 内的纯资产生成脚本」+「独立 public repo 的通用建模/渲染库」，Bong 侧只调用不实现。

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | 边界固化：`Workspace` 抽象消灭 `REPO = parents[2]`，掰正 core → `client/tools/` 反向依赖 | ✅ 2026-08-30 |
| P1 | 新 repo `Kizunad/bbmodel-maker`（public）：filter-repo 抽子树历史 → `bbmodel_maker` 包 → 发 `v0.1.0` | ⬜ |
| P2 | Bong 接依赖：`pip install git+...@v0.1.0`，16 个 LIB generator + 6 个 creatures 包换 import，golden 回归 | ⬜ |
| P3 | 收口：删已搬走的 `core/`+`tools/`，改 CI 与 `scripts/test-all.sh` 测试范围，README 拆两半 | ⬜ |
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

## P1 —— 新 repo `Kizunad/bbmodel-maker`（public）

1. `git filter-repo --path modelScript/core --path modelScript/tools --path-rename modelScript/core:src/bbmodel_maker/ ...` 抽子树历史（**保留 blame**，用户已拍板）
2. 重排成五个子包：

   | 子包 | 收谁 |
   |---|---|
   | `bbmodel_maker.model` | `armor_model_common` `to_fmt410` `workspace` |
   | `bbmodel_maker.rig` | `animcore` `animkit` `anim_rig` `voxel_rig` `rigkit` `bb_anim_axes` |
   | `bbmodel_maker.render` | `render_bbmodel` `framing` `pose_render` `render_player_pose` `held_item_common` `held_item_render` |
   | `bbmodel_maker.gates` | `gatekit` `animgate` `manifest` |
   | `bbmodel_maker.workbench` | `contact_sheet` `preview_armor_on_body` `preview_player_anim` `make_variants` `held_item_pose` `bbmodel_to_pose` `bbmodel_to_geckolib` |

3. `pyproject.toml`：依赖 `numpy` `pillow`，`[project.optional-dependencies] geckolib = ["playwright"]`（`bbmodel_to_geckolib` 才需要），console_scripts 暴露 workbench CLI
4. 迁 11 个库测试（4414 行）：`test_{animcore,animgate,anim_preview_fidelity,anim_retime,bb_anim_roundtrip,contact_sheet,framing,gatekit,manifest,held_item_pose,preview_armor_on_body}.py`
5. 新 repo CI：`python -m unittest discover` + `ruff`
6. LICENSE / README（英文 quickstart + 中文踩坑段落照搬）
7. **验收**：新 repo CI 绿；`pip install git+https://github.com/Kizunad/bbmodel-maker@v0.1.0` 在干净 venv 里能装能 import；打 tag `v0.1.0`

## P2 —— Bong 接依赖

1. 新建 `modelScript/requirements.txt`：`bbmodel-maker @ git+https://github.com/Kizunad/bbmodel-maker@v0.1.0`（**版本锁死**，库改动不静默影响 Bong）
2. 16 个 LIB generator + 6 个 `creatures/*` 包换 import（`from bbmodel_maker.render import render_bbmodel` 等）
3. 3 个 Bong 专属 tool 留下并改为调库：`tools/gen_pipeline_refs.py`（读 `scripts/images/.env`，:28,91,92）、`tools/transform_laoshu.py`、`tools/recolor_animate_rat.py`
4. **验收**：`test_golden_bytes.py` 全绿（重跑生成器字节不变）；`test_gen_*.py` 13 个 + `test_club_anims.py` + `test_jian_player_tools.py` + `test_export_coffin_assets.py` 全绿

## P3 —— 收口

1. `git rm` 掉已搬走的 `modelScript/core/` 与 7 个 `tools/`
2. `.github/workflows/build-resourcepack.yml`：`:56` 的 `pip install numpy pillow` → `pip install -r modelScript/requirements.txt`；`:66` 的 discover 范围不变（Bong 侧测试仍在 `modelScript/tests/`）
3. `scripts/test-all.sh:561,578` 与 `scripts/test-all-owners.tsv` 跟着改
4. `modelScript/README.md` 拆两半：库那半搬去新 repo，Bong 这半只留「怎么出一件资产 + 本仓踩过的坑」
5. **验收**：`bash scripts/test-all.sh` 绿；`bash scripts/build-resourcepack.sh` 产物 sha1 不变

## P4 —— SOLO generator 收编（机会主义）

25 个自给 generator **不设期限、不作为本 plan 的完成条件**（用户 2026-08-30 拍板：「solo gen 能起到效用就好，代码整洁度优先级不高，之后按照流程来就行」）。今后谁碰哪件资产，顺手把它的 `build_bbmodel` / `make_texture` 换成库 API，逐件 golden 比对。

## 风险

- **产物字节漂移**：uuid 生成顺序、贴图像素、face 顺序差一点，重跑生成器就覆盖掉已入库 bbmodel。`test_golden_bytes.py` 是 P0 的第一件事，不是最后一件
- **fmt5.0 手改稿**：`models/` 里 `format_version 5.0` 的一律是 Blockbench 手改过的，生成器只写 4.10，**重跑会覆盖**（README「models/ 什么入库什么不入库」）。golden 测试要跳过这批
- **本地改库的回路**：版本锁死的代价是本地迭代要 `pip install -e ../bbmodel-maker`。P1 的 README 要写清这条
