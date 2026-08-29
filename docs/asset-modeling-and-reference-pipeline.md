# Bong 3D 资产程序化建模与参考生图全流程规范 (Asset Modeling & Reference Pipeline Guide)

本文档定义了 Bong 体系下从 **AI 参考图分步生成**、**程序化 3D 建模 (.bbmodel)**、**贴图与动画解算**、**3 轮视觉打磨与人工闸门** 到 **最终提 PR 闭环** 的完整标准化工作流程。

---

## 目录
1. [全流程阶段总览](#一-全流程阶段总览)
2. [Phase 1: AI 参考图生成流程 (TokensKingdom API)](#二-phase-1-ai-参考图生成流程)
3. [Phase 2: 程序化 3D 建模与贴图绘制 (modelScript)](#三-phase-2-程序化-3d-建模与贴图绘制)
4. [Phase 3: 视觉打磨纪律与 Round 2 人工闸门](#四-phase-3-视觉打磨纪律与-round-2-人工闸门)
5. [Phase 4: 客户端/服务端资产接线与测试](#五-phase-4-客户端服务端资产接线与测试)
6. [Phase 5: 提交规范与 PR 创建规范](#六-phase-5-提交规范与-pr-创建规范)
7. [关键文件与模块索引](#七-关键文件与模块索引)

---

## 一、 全流程阶段总览

```
[需求定义] (资产名称/设定/世界观品味)
    │
    ▼
[Phase 1: AI 参考图流水线]
    1. 生成概念图 (Concept Art) ──> 👤 用户审核确认
    2. 生成黑底物品图标 (Item Icon) ──> 👤 用户审核确认 (生物跳过)
    3. 生成 MC 体素正交三视图 (Three-View) ──> 👤 用户审核确认 (物品佩戴在纯灰模特上)
    4. 生成 MC 体素爆炸分解图 (Exploded View) ──> 👤 用户审核确认
    │
    ▼
[Phase 2: 3D 程序化建模与贴图 (modelScript)]
    1. 编写 generators/gen_<asset>.py (Cube 分件几何 + 64x64 贴图绘制)
    2. 无共面 Z-Fighting 校验 (_assert_no_coplanar_faces)
    3. 编写 manifests/<Asset>.manifest.toml 特征清单
    │
    ▼
[Phase 3: 3 轮视觉打磨与人工闸门]
    Round 1: First Cut 资产生成
    Round 2: 运行 contact_sheet.py 产出 6 视角接触表 ──> 👤 停下等待用户确认
    Round 3: 细节修复，生成终版资产，附带 <PROMISE> 担保块
    │
    ▼
[Phase 4: 双端接线与门禁测试]
    1. 生成 Java 字面量并注入 client (ArmorPartModel 等)
    2. 资源包 SHA1 更新 (manifest.json / resourcepack.rs)
    3. 运行完整单元测试套件
    │
    ▼
[Phase 5: 提交与提 PR]
    1. 分支创建与 Atomic Commit (携带 Model: trailer)
    2. gh pr create 创建 Pull Request
```

---

## 二、 Phase 1: AI 参考图生成流程

参考图生成工具为 `modelScript/tools/gen_pipeline_refs.py`，采用分步可控生成，**每生成一步必须将图片展示给用户审阅，确认无误后方可进入下一步**。

### 1. 基础调用与后端配置
- **API 端点**: `https://image.tokenskingdom.com`
- **模型**: `gpt-image-2`
- **配置文件**: `scripts/images/.env` 中的 `TK_IMAGE_API_KEY`（或环境变量 `OPENAI_API_KEY`）

### 2. 四步分步生图指令
```bash
# 步骤 1: 生成世界观概念图 (纯文生图) -> 注入末法残土灰暗修仙画风
python3 modelScript/tools/gen_pipeline_refs.py <asset_name> --step concept --prompt "<资产详细概念描述>"

# 步骤 2: 生成黑底物品图标 (基于概念图的图生图，生物跳过)
python3 modelScript/tools/gen_pipeline_refs.py <asset_name> --step icon

# 步骤 3: 生成 MC 正交三视图 (基于概念图的图生图)
# 物品会自动提示装备在纯灰色模特玩家身上
python3 modelScript/tools/gen_pipeline_refs.py <asset_name> --step three_view --type item

# 步骤 4: 生成 MC 爆炸分解图 (基于三视图的图生图，带结构与文字标注)
python3 modelScript/tools/gen_pipeline_refs.py <asset_name> --step exploded
```

---

## 三、 Phase 2: 程序化 3D 建模与贴图绘制

### 1. 挂载点与坐标系规则
- **ArmorPartModel 仅 6 个挂载点 (Mount)**:
  `HEAD / BODY / LEFT_LEG / RIGHT_LEG / LEFT_FOOT / RIGHT_FOOT`
  - ⚠ 肩甲只能挂 `BODY`（不跟随手臂晃动）。
  - ⚠ 腿甲必须劈开归入 `LEFT_LEG` 与 `RIGHT_LEG`。
- **坐标与视角系统**:
  - `yaw=180` 为正面 (FRONT，法线朝向 `+z`)，`yaw=0` 为背面 (BACK)。
  - 正确使用 `core/framing.py` 提供的正交视角。

### 2. 避免共面 Z-Fighting
- 生成器必须内嵌 `_assert_no_coplanar_faces(all_parts)` 校验。任何同向外表面若投影重叠且共面，必须微调微小偏置 (0.01~0.05)。

### 3. 特征清单 (Manifest) 契约
在 `modelScript/manifests/<Asset>.manifest.toml` 中明确人写特征契约：
```toml
facing = "+z"
mirror_x = 0.0
size = 300

[features]
main_plate = { elements = ["chest_sternum_"], must_show_in = ["FRONT", "3/4"], min_px = 800 }
shoulder_skull = { elements = ["skull_"], must_show_in = ["FRONT", "SIDE_L"], asym = "left" }
```

---

## 四、 Phase 3: 视觉打磨纪律与 Round 2 人工闸门

1. **Round 1 (First Cut)**:
   - 运行 `python3 modelScript/generators/gen_<asset>.py` 输出初版。
2. **Round 2 (人工闸门 - 停下等人看)**:
   - 运行接触表工具：
     ```bash
     bbmodel-contact-sheet modelScript/models/<Asset>.bbmodel \
         --gates gen_<asset> --prev modelScript/out/<Asset>_round1.bbmodel
     ```
   - 工具合成 6 个诚实视角（FRONT/BACK/SIDE_L/SIDE_R/3-4/TOP）、上轮对比、Manifest 点名与几何门自检。
   - **停下等待人工确认外观与剪影**。
3. **Round 3 (Final Cut)**:
   - 根据审阅意见修整几何与贴图，完成最终交付。

---

## 五、 Phase 4: 客户端/服务端资产接线与测试

1. **输出 Java 代码**:
   ```bash
   python3 modelScript/generators/gen_<asset>.py --emit-java
   ```
   注入 `client/src/main/java/.../ArmorPartModel.java` 的 Cube 表中。
2. **人体试穿覆盖度核验**:
   ```bash
   bbmodel-armor-preview gen_<asset> --slot chest --coverage
   ```
3. **跑通完整测试套件**:
   ```bash
   python3 -m unittest discover -s modelScript/tests -p "test_*.py"
   ```

---

## 六、 Phase 5: 提交规范与 PR 创建规范

### 1. Git Commit 规范
- Commit 消息使用**中文**，遵循原子提交（Atomic Commit）。
- 每个 commit 末尾必须包含真实执行模型署名 trailer：
  ```
  feat(modelScript): 完成 <资产名称> 3D 程序化建模与贴图系统 (round 3/3)

  <PROMISE>已完成 3 轮打磨与人工闸门确认，贴图已核验无共面 z-fighting</PROMISE>

  Model: gpt-5.6-sol-xhigh
  ```

### 2. PR 创建规范
使用 `gh pr create` 创建 PR，附带清晰说明与生成模型：
```bash
gh pr create --head feat/<branch-name> \
  --title "feat(modelScript): 实现 <资产名称> 3D 程序化建模与参考图资产" \
  --body "## 概要\n- 完成 <资产名称> 概念图、黑底图标、MC 三视图、爆炸分解图设计\n- 完成 gen_<asset>.py 程序化建模与 64x64 贴图生成\n- 通过 Round 2 人工闸门与单元测试\n\n主导模型: gpt-5.6-sol-xhigh"
```

---

## 七、 关键文件与模块索引

| 路径 | 说明 |
| :--- | :--- |
| `modelScript/tools/gen_pipeline_refs.py` | 概念图、图标、MC 三视图、爆炸图 4 阶段分步生成工具 |
| `modelScript/generators/` | 各资产程序化建模生成脚本 (`gen_*.py`) |
| `bbmodel_maker.model.armor_model_common` | 护甲 Cube 结构体、挂载点基准、UV 计算及资产写入共通库 |
| `bbmodel_maker.render.render_bbmodel` | Headless z-buffer 软渲染器与固定取景器 |
| `bbmodel_maker.workbench.contact_sheet` | Round 2 人工闸门接触表合成工具 |
| `bbmodel_maker.workbench.preview_armor_on_body` | 真原版 Minecraft 玩家流民人体模型试穿与覆盖度检测工具 |
| `modelScript/manifests/` | 人写特征清单 TOML 目录 |
| `modelScript/assets/refs/` | 参考生图与设计底稿目录 |
| `scripts/images/.env` | 生图 API 端点与 Key 配置文件 |
