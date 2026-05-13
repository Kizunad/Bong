# Bong · plan-video2anim-v1 · 骨架

**视频动捕→玩家动画生产线**。基于 MediaPipe 姿态估计从真人视频中提取动作，转换为 Emotecraft v3 JSON（PlayerAnimator 格式），接入现有 `client/tools/` 动画工作流。定位是"粗稿生成器"——视频出初版 → `render_animation.py` 验证 → 手工在 `gen_*.py` 中微调。

灵感来源：[Jaffe2718/video2geckolib4](https://github.com/Jaffe2718/video2geckolib4)（MediaPipe → GeckoLib animation.json），本 plan 不直接集成该项目，而是借鉴其坐标变换数学、重写输出层以直接对接 Emotecraft v3 + bend 分解。

---

## 接入面 Checklist

- **进料**：`.mp4` / `.mov` 等视频文件（真人武术/动作演示）
- **出料**：
  - `client/src/main/resources/assets/bong/player_animation/{name}.json` — Emotecraft v3 JSON（可直接 F3+T 热加载）
  - 可选：`client/tools/gen_{name}.py` — 从视频导出的 pose table 骨架脚本，供手工精修
- **共享类型 / 工具**：复用 `client/tools/anim_common.py`（`emit_json` / `render` / `VALID_PARTS` / `ANGLE_AXES`），不另建发射器
- **跨仓库契约**：纯 client 工具链，不涉及 server / agent
- **worldview 锚点**：无直接锚点（工具链 plan，非玩法 plan）
- **qi_physics 锚点**：无

---

## 阶段总览

| 阶段 | 内容 | 状态 | 验收日期 |
|------|------|------|----------|
| P0 | 核心转换器 `video2emotecraft.py` — 端到端视频→Emotecraft v3 | ⬜ | TBD |
| P1 | 工具链集成 — render 验证 / gen 脚本导出 / 批量 CLI | ⬜ | TBD |
| P2 | 质量提升 — 时域平滑 / easing 推断 / 参考姿态校准 | ⬜ | TBD |

---

## P0 — 核心转换器 ⬜

目标：`python3 client/tools/video2emotecraft.py input.mp4 -o punch_from_video` 产出可直接 F3+T 加载的 Emotecraft v3 JSON。

### P0.1 MediaPipe 姿态提取

文件：`client/tools/video2emotecraft.py`

- 类 `VideoPoser`：封装 MediaPipe Pose（`model_complexity=2` 高精度），按指定 FPS 采样视频帧
- 输出双通道：`pose_world_landmarks`（3D 度量坐标，用于旋转）+ `pose_landmarks`（归一化图像坐标，用于平移）
- 采样率默认 20 FPS（= MC 20 TPS，1 帧 = 1 tick，省去重采样）
- 依赖：`mediapipe`、`opencv-contrib-python`、`numpy`、`scipy`（写入 `client/tools/requirements-video2anim.txt`）

### P0.2 坐标系变换 + 骨骼旋转计算

类 `PoseToEmotecraft`，核心数学借鉴 video2geckolib4 的 `PoseConverter`：

**坐标系对齐**：
- MediaPipe：X 右（被试视角）/ Y 下 / Z 朝镜头
- MC PlayerAnimator：X 右 / Y 上 / Z 前
- 变换：`[-x, -y, -z]`（与 video2geckolib4 的 `_p()` 一致）

**10 → 7 骨骼映射 + bend 分解**（本 plan 核心难点）：

| video2geckolib4 骨骼 | MediaPipe landmarks | → Emotecraft 部件 | 映射方式 |
|---------------------|--------------------|--------------------|----------|
| Body (rotation) | 11,12,23,24 | `torso` (pitch/yaw/roll) | 直接取旋转，作为 torso 上半身旋转 |
| Body (translation) | 23,24 hip midpoint | `body` (x/y/z) | 髋中点位移 × scale |
| Head | 3,6,9,10 | `head` (pitch/yaw/roll) | 相对于 Body 的旋转，pitch 取反 |
| LeftUpperArm | 11,13,15 | `leftArm` (pitch/yaw/roll) | 相对于 Body 的旋转 → 直接映射 |
| LeftForearm | 13,17,19,21 | `leftArm` (bend/axis) | 前臂相对上臂的旋转 → 分解为 bend 幅度 + axis 方向 |
| RightUpperArm | 12,14,16 | `rightArm` (pitch/yaw/roll) | 同 LeftUpperArm |
| RightForearm | 14,18,20,22 | `rightArm` (bend/axis) | 同 LeftForearm |
| LeftThigh | 23,24,25 | `leftLeg` (pitch/yaw/roll) | 相对于 Body 的旋转 |
| LeftCalf | 23,24,25,27 | `leftLeg` (bend) | 小腿相对大腿的旋转 → bend（腿只有单轴弯曲，axis 固定 0） |
| RightThigh | 23,24,26 | `rightLeg` (pitch/yaw/roll) | 同 LeftThigh |
| RightCalf | 23,24,26,28 | `rightLeg` (bend) | 同 LeftCalf |

**bend 分解算法**（`_decompose_bend`）：
1. 取前臂/小腿相对于上臂/大腿的旋转矩阵 `R_rel`（已由 `_rel()` 计算）
2. 从 `R_rel` 提取欧拉角 `[pitch, yaw, roll]`
3. `bend = acos(clamp(R_rel[1][1], -1, 1))`（Y 轴夹角 = 弯曲幅度）
4. `axis = atan2(R_rel[2][1], R_rel[0][1])`（弯曲平面在 XZ 平面上的方向角）
5. 对腿：bend 主要由 pitch 贡献，axis ≈ 0（膝盖只能前弯），直接取 `abs(pitch)` 作为 bend

**单位转换**：
- 旋转：度 → 弧度（`math.radians()`，与 `anim_common.py` 的 `d = math.radians` 一致）
- 时间：帧索引 / 20 → tick 整数（20 FPS = 20 TPS 时帧号直接是 tick）
- 位移：MediaPipe 归一化坐标 × scale → MC meters（`anim_common.py` 中 body xyz 单位是 meters）

### P0.3 Emotecraft v3 JSON 发射

- 复用 `anim_common.emit_json()` 的输出格式，但输入是运行时生成的 pose_table（而非手写常量）
- 构建 `pose_table: Dict[int, dict]` — key 是 tick，value 是 `{body: {x,y,z}, torso: {pitch,yaw,roll}, head: {...}, leftArm: {..., bend, axis}, ...}`
- 角度值在 pose_table 中保持**度**单位（`emit_json` 内部负责 `math.radians` 转换）
- 调用 `emit_json(pose_table, name=..., end_tick=..., stop_tick=..., is_loop=False)` 输出 JSON

### P0.4 CLI 入口

```bash
python3 client/tools/video2emotecraft.py INPUT_VIDEO \
  -o NAME                    # 输出名（不含后缀）
  --fps 20                   # 采样帧率（默认 20 = MC TPS）
  --complexity 2             # MediaPipe model_complexity (0/1/2)
  --translate                # 是否提取 body 平移（默认关）
  --no-smooth                # 关闭角度 unwrap（默认开）
  --preview                  # 完成后自动调 render_animation.py 出预览图
```

输出文件：`client/src/main/resources/assets/bong/player_animation/{NAME}.json`

### P0 验收标准

- [ ] 录一段 3 秒挥拳视频 → 跑脚本 → 产出 `fist_from_video.json`
- [ ] `render_animation.py fist_from_video.json` 输出的三视图（正/侧/顶）能看出挥拳轮廓
- [ ] F3+T 热加载后 `/anim play fist_from_video` 在游戏内播放，手臂有明显弯曲（bend 值 > 0.3 rad）
- [ ] 对比手写的 `fist_punch_right.json`：动作方向一致（右手前伸），时长合理（±50%）

### P0 测试

文件：`client/tools/test_video2emotecraft.py`

- **坐标变换**：已知 MediaPipe landmark 坐标 → 断言 `_p()` 输出的 Blockbench 坐标符号正确
- **骨骼旋转**：构造人体站立 T-pose landmarks → 断言所有骨骼旋转 ≈ 0
- **bend 分解**：构造前臂弯曲 90° 的 landmarks → 断言 bend ≈ π/2, axis ≈ π（向前弯）
- **bend 分解（腿）**：构造膝盖弯曲 45° → 断言 bend ≈ π/4, axis ≈ 0
- **单位转换**：断言输出 JSON 中角度为弧度、degrees=false
- **pose_table 结构**：断言输出的 pose_table key 全是 int、part name 全在 `VALID_PARTS` 内
- **空帧处理**：MediaPipe 丢帧（landmarks=None）时，断言该 tick 被跳过或插值
- **角度连续性**：构造跨 ±180° 边界的角度序列 → 断言 smooth 后差值 < 180°
- **循环闭合**：`--loop` 模式下断言 tick 0 和 end_tick 值匹配（复用 `anim_common._check_loop_closure`）

---

## P1 — 工具链集成 ⬜

### P1.1 gen 脚本导出

`video2emotecraft.py --export-gen NAME` 模式：不直接输出 Emotecraft JSON，而是生成 `client/tools/gen_{NAME}.py` 骨架脚本——内含从视频导出的 pose_table 常量 + `emit_json()` 调用。开发者在此基础上手修关键帧。

- 只导出"关键"帧（角度变化超过阈值的帧），非逐帧；阈值默认 5°
- 每个 tick 行带注释标注原始帧号和时间戳
- 自动 round 角度到 0.5° 精度（手修友好）

### P1.2 render 集成

`--preview` 标志：转换完成后自动调用 `render_animation.py {output}.json -o /tmp/video2anim_preview/`，在终端显示预览路径。失败时只 warning 不 abort。

### P1.3 批量 CLI

`video2emotecraft.py batch INPUT_DIR -o OUTPUT_DIR`：扫描目录下所有 `.mp4` / `.mov`，逐个转换，跳过已存在的同名 JSON。用于从武术教学视频库批量生成粗稿。

### P1 验收标准

- [ ] `--export-gen` 产出的 gen 脚本可直接 `python3 gen_xxx.py` 跑通，输出合法 JSON
- [ ] 手工修改导出的 gen 脚本中 3 个关键帧 → 重新生成 → 动画明显变化
- [ ] batch 模式处理 5 个视频，全部成功产出 JSON

### P1 测试

- **gen 脚本语法**：导出的 .py 文件 `compile()` 不报错
- **关键帧过滤**：20 FPS × 3 秒 = 60 帧原始数据 → 关键帧 < 30（至少过滤一半静止帧）
- **角度精度**：round 后与原始值差异 < 0.5°
- **batch 幂等**：跑两次，第二次不重新生成已有文件

---

## P2 — 质量提升 ⬜

### P2.1 时域平滑

video2geckolib4 只做角度 unwrap（防 360° 跳变），并未真正地降噪。MediaPipe 在遮挡/快速运动时会产生逐帧抖动。

- 在 `_smooth_angle()` unwrap 之后，加一层 **Savitzky-Golay 滤波**（`scipy.signal.savgol_filter`）
- 窗口长度 5 帧（250ms），多项式阶数 2
- 可通过 `--smooth-window N` 调节（0 = 关闭）
- 在 bend 通道也做平滑（bend 抖动比旋转更明显）

### P2.2 easing 推断

手写动画会给关键帧配 easing（`INOUTSINE` / `OUTQUAD`），但视频动捕逐帧线性。

- 分析相邻关键帧间的角速度变化率
- 加速段 → `EASEINQUAD`，减速段 → `EASEOUTQUAD`，匀速 → `linear`
- 仅在 `--export-gen` 模式下标注（直接出 JSON 时不加 easing，保持逐帧精度）

### P2.3 参考姿态校准

MediaPipe 的 T-pose 检测不完美，导致"站直不动"时骨骼旋转不为零。

- `--calibrate FRAME_RANGE`：指定视频中 T-pose 参考帧范围（如 `0-20`）
- 计算参考帧的平均骨骼旋转作为零点偏移
- 后续所有帧减去该偏移
- 显著改善"静止时身体微晃"的问题

### P2 验收标准

- [ ] 同一段视频：有 Savitzky-Golay vs 无 → render 预览图中关节轨迹更平滑
- [ ] easing 推断：从减速挥拳视频导出的 gen 脚本中，结尾关键帧自动标注 `EASEOUTQUAD`
- [ ] T-pose 校准：录 2 秒站立 + 3 秒动作 → `--calibrate 0-40` → 站立段所有骨骼旋转 < 3°

### P2 测试

- **Savitzky-Golay**：构造含高频噪声的角度序列 → 滤波后标准差降低 > 50%
- **easing 推断**：构造匀加速角度序列 → 断言推断结果为 `EASEINQUAD`
- **校准**：构造偏移 10° 的 T-pose → 校准后残差 < 1°

---

## 已知局限与风险

| 风险 | 影响 | 缓解 |
|------|------|------|
| MediaPipe 对武术快速动作精度不足 | 出拳/劈砍关键帧跳变 | P2.1 时域平滑 + 定位为"粗稿"而非最终产出 |
| bend 分解信息损失（3DOF→2DOF） | 前臂扭转丢失 | 对修仙动画影响小（多为弯曲/伸展，少有前臂旋转） |
| 单人姿态限制 | 不支持双人对练视频 | 分别录单人动作 |
| torso/body 拆分不精确 | 弯腰/转身时 torso 和 body 耦合 | 参考 feedback_torso_legs_hinge 做鞠躬补偿 |
| 腿 bend axis 固定 0 | 侧踢等非前向弯曲丢失 | 大多数战斗动作是前踢/前弓步，影响有限 |

---

## 文件清单

| 文件 | 用途 |
|------|------|
| `client/tools/video2emotecraft.py` | 核心转换器（P0 全部 + P2 平滑/校准） |
| `client/tools/test_video2emotecraft.py` | 单测 |
| `client/tools/requirements-video2anim.txt` | Python 依赖（mediapipe / opencv / numpy / scipy） |
