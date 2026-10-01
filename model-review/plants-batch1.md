# 灵植第一批：苔藓 / 藻 Round 1 评审

状态：`gate-waiting`

Round 1 first cut 已完成，Round 2 人工闸门接触表已生成，等待逐株视觉意见；本轮不进入 Round 3，不写 `<PROMISE>`，不推送、不创建 PR。

接触表：[`model-review/img/plants/batch1_contact.png`](img/plants/batch1_contact.png)。每行依次为 GUI 原画、CPA 三视图参考、当前作者稿的六视角接触表。六视角标签由工具写出实际轴面：`FRONT (+z)`、`BACK (-z)`、`SIDE_L (-x)`、`SIDE_R (+x)`、`3-4`、`TOP (+y)`。当前与 Round 1 基线使用同一取景并排，便于看出本轮尚无人工返工的事实。

## 本批接线

`shared/botany/plants.json` 已在主线，成熟阶段按现有十种模型的契约接为 `kind: "geo"`，包含 `geometry`、`texture`、`offset: [0.0, 0.0, 0.0]` 与 `scale: 1.0`；seedling / growing 阶段仍是 billboard。模型作者稿由 `modelScript/exporters/export_plant_assets.cjs` 一次性导出到客户端 geo 与 PNG，未手写导出几何或资源摘要。

## 逐株造型记录

### 灰烬苔 `hui_jin_tai`

- 描述依据：残灰方块表面的一层灰黑薄苔，外敷止血，不能久服。
- first cut：`ash_bed` 做低矮的灰烬铺面，`soot_crust` 压出黑色接触层，`lichen_rosettes` 用三簇灰绿薄片读出苔面，`dry_highlights` 留少量干燥黄灰边。
- 真实符号：[`modelScript/generators/gen_hui_jin_tai.py`](../modelScript/generators/gen_hui_jin_tai.py)、`modelScript/models/HuiJinTai.bbmodel`、`client/src/main/resources/assets/bong/geo/plants/hui_jin_tai.geo.json`、`client/src/main/resources/assets/bong/textures/entity/plants/hui_jin_tai.png`、`modelScript/manifests/HuiJinTai.manifest.toml`。
- 参考：`model-review/img/plants/hui_jin_tai/gui_original.png` 与 `three_view.png`。

### 裂渊苔 `lie_yuan_tai`

- 描述依据：坍缩渊裂缝口的紫黑苔，靠两界压差的微弱流维生。
- first cut：`rift_bed` 压低成裂缝底床，`rift_ridges` 形成左右紫黑苔脊，`pressure_filaments` 用两侧上行细脉表现压差泵流，`dry_black` 留黑色断面。
- 真实符号：[`modelScript/generators/gen_lie_yuan_tai.py`](../modelScript/generators/gen_lie_yuan_tai.py)、`modelScript/models/LieYuanTai.bbmodel`、`client/src/main/resources/assets/bong/geo/plants/lie_yuan_tai.geo.json`、`client/src/main/resources/assets/bong/textures/entity/plants/lie_yuan_tai.png`、`modelScript/manifests/LieYuanTai.manifest.toml`。
- 参考：`model-review/img/plants/lie_yuan_tai/gui_original.png` 与 `three_view.png`。

### 噬灵藓 `shi_ling_xian`

- 描述依据：负灵域蔓生的黑藓，每一步都吸食踩踏者真元，不可采集、不可入药。
- first cut：`null_bed` 是黑青贴地苔垫，`siphon_tufts` 形成短簇，`siphon_spines` 向上收束成吸附刺，`wet_tips` 以湿青和枯尖做危险边缘；没有做可采集的高茎或花序。
- 真实符号：[`modelScript/generators/gen_shi_ling_xian.py`](../modelScript/generators/gen_shi_ling_xian.py)、`modelScript/models/ShiLingXian.bbmodel`、`client/src/main/resources/assets/bong/geo/plants/shi_ling_xian.geo.json`、`client/src/main/resources/assets/bong/textures/entity/plants/shi_ling_xian.png`、`modelScript/manifests/ShiLingXian.manifest.toml`。
- 参考：`model-review/img/plants/shi_ling_xian/gui_original.png` 与 `three_view.png`。

### 玄绒苔 `xuan_rong_tai`

- 描述依据：深渊二层温差带的漆黑绒苔，银光只有靠近掌心才浮出。
- first cut：`velvet_bed` 铺出黑色绒面，`velvet_lobes` 叠三块柔软绒团，`silver_hairs` 是低对比银色绒毛，`cold_shadow` 压住温差带的冷暗接缝。
- 真实符号：[`modelScript/generators/gen_xuan_rong_tai.py`](../modelScript/generators/gen_xuan_rong_tai.py)、`modelScript/models/XuanRongTai.bbmodel`、`client/src/main/resources/assets/bong/geo/plants/xuan_rong_tai.geo.json`、`client/src/main/resources/assets/bong/textures/entity/plants/xuan_rong_tai.png`、`modelScript/manifests/XuanRongTai.manifest.toml`。
- 参考：`model-review/img/plants/xuan_rong_tai/gui_original.png` 与 `three_view.png`。

### 养经苔 `yang_jing_tai`

- 描述依据：死域边缘的铁锈绿苔，可养经脉裂痕，爆脉后服用。
- first cut：`rust_bed` 以锈褐做底，`meridian_mats` 从中心向四方分叉成绿脉，`meridian_leaves` 是压低的绿苔片，`scar_dark` 保留爆脉后的暗色裂痕。
- 真实符号：[`modelScript/generators/gen_yang_jing_tai.py`](../modelScript/generators/gen_yang_jing_tai.py)、`modelScript/models/YangJingTai.bbmodel`、`client/src/main/resources/assets/bong/geo/plants/yang_jing_tai.geo.json`、`client/src/main/resources/assets/bong/textures/entity/plants/yang_jing_tai.png`、`modelScript/manifests/YangJingTai.manifest.toml`。
- 参考：`model-review/img/plants/yang_jing_tai/gui_original.png` 与 `three_view.png`。

### 井心藻 `jing_xin_zao`

- 描述依据：灵泉眼吐纳开期才舒展的翠青藻，合期采摘会被井口反吸。
- first cut：`well_stone` 围出低矮井心，`water_heart` 保留青水和深色水心，`algae_fronds` 从水心向四方舒展，`algae_lit` 留少量翠亮尖端。
- 真实符号：[`modelScript/generators/gen_jing_xin_zao.py`](../modelScript/generators/gen_jing_xin_zao.py)、`modelScript/models/JingXinZao.bbmodel`、`client/src/main/resources/assets/bong/geo/plants/jing_xin_zao.geo.json`、`client/src/main/resources/assets/bong/textures/entity/plants/jing_xin_zao.png`、`modelScript/manifests/JingXinZao.manifest.toml`。
- 参考：`model-review/img/plants/jing_xin_zao/gui_original.png` 与 `three_view.png`。

## 接触表与门禁证据

六件各自用 `bbmodel-contact-sheet` 生成了 `CURRENT` 与同取景 `PREV`（Round 1 first-cut 基线）的六视角并排表；批次总表把 GUI 原画、CPA 三视图和这六视角表按株并列。每份 manifest 都点名了材料与关键部件，终端结果为 `0` 缺项、全部 5 种声明材质至少一次上镜。

每个生成器的 `PlantGates` 都通过四道结构门：孤儿 element、0..16 方块越界、退化薄片（<0.2px）、地面/高度范围；四道门的差分自证均为 `clean=0 → injected=1`，即 `4/4` 具有鉴别力。门禁声明位于 `modelScript/generators/plant_geo_common.py`，六个生成器分别暴露 `build()` 与 `GATES`，接触表实际调用了它们。

资源导出由 `node modelScript/exporters/export_plant_assets.cjs` 完成（16 种：主线原有 10 种 + 本批 6 种），客户端测试清单已加入本批六个 ID，确保共享目录的成熟阶段、geo 和纹理能一起解析。

## Round 2 闸门

当前状态明确为 `gate-waiting`。请逐株审阅接触表中 `FRONT (+z)`、`BACK (-z)`、`SIDE_L (-x)`、`SIDE_R (+x)`、`3-4`、`TOP (+y)` 六个诚实视角，并给出需要 Round 3 修改的部件名；在收到意见前不继续终审、不写 `<PROMISE>`、不开 PR。
