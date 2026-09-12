# 暴龙王附加装饰

沿用 `models/baolongwang/` 的原版 Bedrock 几何、贴图与五条动画，追加嵌入胸腹的炼丹炉和两条部分融合的人臂。概念、三视、分解参考图已由用户在 2026-09-10 逐步确认。

## 当前产物

- `modelScript/models/baolongwang/BaolongwangBase.bbmodel`：Blockbench 5.1.6 官方 codec 导入的底本，281 个方块、10 根骨骼、5 条原动画。
- `BaolongwangDecorated_round1.bbmodel`：首轮附加稿。
- `BaolongwangDecorated_round2.bbmodel`：第二轮附加稿，346 个方块、18 根骨骼。调整了原前肢遮住的小人臂。
- `BaolongwangDecorated_round3.bbmodel`：按第二轮反馈“炉子可以做旧”完成炉体材质修订。上缘烟垢、铜绿、炉栅锈蚀与下沿积灰；几何、人体手臂和动画保持第二轮状态。
- `modelScript/out/baolongwang/contact_BaolongwangDecorated_round3.png`：六个轴向标注视角、第二轮同取景对比、特征点名、六项门禁与缺陷注入结果。
- `modelScript/out/baolongwang/Baolongwang_furnace_aged_comparison.png`：真实 Blockbench 炉体特写，左侧第二轮、右侧第三轮。
- `modelScript/assets/refs/baolongwang/decorations_materials.png`：经用户指定服务生成的材质源；提示词同目录保留，密钥不落盘。

**第二轮已收到人工反馈，第三轮做旧稿已完成；尚未提交或替换客户端资产。**

第三轮最初尝试沿用用户指定服务，以 imagegen CLI / `gpt-image-2` 生成六格做旧材质，两次均返回 403 `The channel is temporarily unavailable`，没有生成新图。请求提示词留在 `assets/refs/baolongwang/furnace_aged_materials_prompt.md`。最终使用 `aged_furnace_patches()` 对已有 AI 材质离线调色、提取斑块并叠加烟灰；原人臂、融合组织、余火材质不变。

## 生成与检查

已有底本可直接离线生成。目标模型或同名 PNG 已存在且内容不同时会拒绝写出；手改后请使用新的 `--out` 路径。

```bash
python3 modelScript/creatures/baolongwang/gen_decorations.py --round 1
python3 modelScript/creatures/baolongwang/gen_decorations.py --round 2
python3 modelScript/creatures/baolongwang/gen_decorations.py --round 3
python3 -m modelScript.creatures.baolongwang.gates
python3 -m unittest discover -s modelScript/tests -p 'test_baolongwang_decorations.py'
```

原 PNG 实际为 427×427，UV 空间为 1024×1024。附加材质使用横向扩展的 854×427 图集、2048×1024 UV 空间，原图左半区逐像素保留，旧面的 UV 坐标不改。所有贴图嵌入 bbmodel。

## Blockbench 预览

`import_source.py` 和 `preview.py` 需要 Playwright 与 Chromium，并需访问 `web.blockbench.net`。本次临时环境为 `/tmp/baolongwang-tools`。`--chromium` 可指定已有浏览器；没有传参时使用 Playwright 默认浏览器。

```bash
python3 modelScript/creatures/baolongwang/preview.py \
  modelScript/models/baolongwang/BaolongwangDecorated_round3.bbmodel --animations
```

预览使用 Blockbench 实际场景；五条动画各采样九帧，检测炉体和人臂肩根相对躯干的漂移。原头颌有默认骨骼旋转，故额外从 Blockbench 世界矩阵生成 `*_posed.bbmodel`，供软渲接触表使用。这些烘焙文件只作静态预览，不是可编辑动画资产。

```bash
python3 -m bbmodel_maker.workbench.contact_sheet \
  modelScript/out/baolongwang/BaolongwangDecorated_round3_posed.bbmodel \
  --manifest modelScript/manifests/BaolongwangDecorated.manifest.toml \
  --gates modelScript.creatures.baolongwang.gates \
  --prev modelScript/out/baolongwang/BaolongwangDecorated_round2_posed.bbmodel \
  --size 640 --shading mc \
  --out modelScript/out/baolongwang/contact_BaolongwangDecorated_round3.png
```

## 验收边界

- 第三轮相关 3 项测试通过；五条动画各采样九帧，附件最大漂移约 `2.84e-14`。
- 第二轮与第三轮仅 14 个炉体元素的面 UV 变化，所有几何、骨树、动画及原皮肤图区一致；炉体同取景特写有 101419 个像素变化，透明轮廓一致。
- 原模型几何、骨树、动画、贴图和 UV 比例均有保留检查。
- 炉体至少三分之二体积落在原胸腹的旋转盒并集中，以均匀采样验证。
- 两条人臂各自有肩、肘、腕分组，目前随躯干运动，未新增独立手臂动作。
- 小人臂仍会受原大前肢遮挡；同取景整身图和胸腹特写均交人工判断。
- 炉膛暖色来自漫反射贴图，尚无发光层、粒子、炉体交互或客户端新接线。
- 特征清单转录自用户确认的需求和参考图；数值检查只验证保留、存在与可见，不能替代外观验收。
