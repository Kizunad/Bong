# P6 架势亮相终轮三视图人工验收证据

plan-skill-anim-fidelity-v1 P0 精度标准把「重心转移 / 全身协调」这类**无法机械判定**的项列为逐招人工验收证据，要求批次 PR 附 `render_animation.py` 三视图 PNG + 对照 checklist。本目录是 P6 批次（`stance_woliu` / `stance_zhenmai` 两招习得亮相重制）的终轮（round 3/3）产物。

每张 `*_grid.png` 纵向拼接该动画**全部关键帧**，每帧三面板：FRONT（正对面孔）/ SIDE（角色右侧）/ TOP（俯视）。帧头一行标出该帧的 `body xyz / torso / rArm / lArm` 实际角度，可与 `client/tools/gen_stance_*.py` 的 POSE 表逐帧对拍。

## 重新生成（决定性，与本目录字节一致）

```bash
cd client
for n in stance_woliu stance_zhenmai; do
  python3 tools/render_animation.py \
    "src/main/resources/assets/bong/player_animation/${n}.json" \
    -o "/tmp/anim_render/${n}"
  cp "/tmp/anim_render/${n}/${n}_grid.png" tools/renders/stance_p6/
done
```

> `-o` 收的是**输出目录**，除 `_grid.png` 外还会产出每帧单张 PNG；本目录只保留 grid（与 `yidao_p4/` 同惯例），故先渲到临时目录再拷 grid 回来。

## 本批的特殊背景：这两招原本不该是循环

两张资产原为 `isLoop:true` 站桩，但发射侧 `emit_technique_learned_stance_triggers` 在 `TechniqueLearnedEvent` 上**只单发一次** `PlayAnim`，全仓无任何持续架势状态可驱动循环、也无任何 `StopAnim` 停止路径 —— 即 conventions §13 #6 红线违例。P6 按 plan §8.1 #2 第 4 条决议改为一次性亮相（`isLoop:false` + 收势回中立）。因此本批验收除姿态质量外，还要确认**收势确实回到中立**（末帧全轴归零），否则一次性动画播完会僵在半空。

## 逐招人工验收 checklist（对照 plan §动画精度标准 #1-#5）

| 招 | 双手职责 | 姿态母题兑现 | 重心 / 全身协调 | 收势归中立 | 远距离可辨性（互相对比） |
|---|---|---|---|---|---|
| **stance_woliu**（涡流习得亮相，32t） | 双手同职但**不镜像**：托举段右臂 `pitch -106`、左臂 `-84`，外分 `yaw -36 / +32`，错开约 22° | 「吸 → 吐」：0→8 沉身收拢（`body.y -0.06`、`torso.pitch +9` 塌腰、双臂收胸前 `bend 74`）＝合；8→20 双掌外旋托举撑开涡场＝开。合在前、开在后，与涡流吞噬意象同向 | `body.y` 走 -0.06 → +0.03 完成「下坐蓄 → 拔起撑」的重心转移；`torso.pitch +9 → -5` 由塌腰到挺身；`torso.yaw → +7` 轻拧使整体成螺旋而非平举。腿只做微沉（`leg.pitch ≤ 7°`） | t32 全轴归零，且收势段（20→32）双臂恢复对称——螺旋只存在于托举段，避免播完歪着 | 峰值剪影 = **双臂上举、左右不等高**。round 1 的完全镜像版被自评打回：对称双臂上举是全仓最拥挤的剪影区间 |
| **stance_zhenmai**（针脉习得亮相，28t） | **主辅分工**：右手主（以指代针前点），左手辅（胸前虚扶取穴，`yaw +22 / bend 58` 全程不参与发力） | 「提指取穴 → 下针」：0→8 右臂自体侧提起（`pitch 0 → -46`）、`torso.yaw` 拧到 +14 蓄劲；8→20 前点下针（`pitch → -104`、`bend 62 → 14` 近伸直），`head.pitch +8` 低头看穴交代「点的是一个具体位置」 | 发力由 `torso.yaw +14 → +28` 送肩承担，不是单纯甩胳膊；`body.z → +0.11` 前送把重心压过去；`body.y -0.045 → -0.005` 由沉到起。腿 `leg.pitch ≤ 6°` | t28 全轴归零，`torso.yaw` 解拧回 0 | 峰值剪影 = **单臂斜向伸出 + 另一臂胸前虚扶 + 躯干拧转**。round 1 是正前方直点、正面几乎透视缩短成一个点，被自评打回：正前方直刺是最差的剪影 |

**互不混淆判定**：两招峰值一个是「双臂都在上、左右不等高」，一个是「一臂斜伸一臂收」，加上针脉有明显躯干拧转而涡流是正面挺身——远距离仅凭剪影即可分辨，满足 CLAUDE.md「玩家能从远处分辨对面在用 X 不是 Y」的红线。

## 已由机器断言、**不依赖**肉眼的项

以下各项已在 `AnimCastTicksAlignmentTest#specManifestsEnforcePrecisionStandardMechanically` 经 `anim_spec_manifests/stance_{woliu,zhenmai}.json` 三段式 manifest 机械锁定，本 checklist 不重复用眼睛核：

- 三段边界（anticipation / strike / recovery）合法、有序、不重叠，且 `recovery[1] == endTick`；
- 每段 ≥ 2 个帧点；
- 主打击轴相邻帧点间隔 ≤ 4 tick（两招全程帧点均为每 4t 一帧）；
- 每帧 `easing` 显式声明，主打击轴禁 `linear`；
- `leg.*.pitch ≤ 40°`（§13 #5 库坑二：MC 无 IK）。

另由 `BongAnimationAssetManifestTest`（资产存在性 / v3 / `name == filename` / 弧度制 / `head.roll`·`torso.roll` 归零）与 `AnimWiringManifestTest`（经生产 resource-reload 回调真实解析 + `BongAnimationRegistry.contains`）覆盖。发射侧 `fade_in = 3`（冷起手淡入，conventions §2.7）由 server `stance_reveal_play_anim_carries_explicit_cold_start_fade_in` 锁定。
