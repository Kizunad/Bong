# Bong · 玩家动画标配与反僵硬要点

> 从 `bong:fist_punch_right` v1 → v3.4 → v10 的迭代血泪里提炼出来的实战经验。
> 适用对象：所有用 Java `KeyframeAnimation.AnimationBuilder` 或 JSON 资源（Emotecraft v3 格式）定义的玩家动画。
> 交叉引用：`plans-skeleton/plan-player-animation-v1.md`（动画系统总纲）。

**验收基线**：
- v3.4（2026-04-14 Java builder 路线）：反僵硬八条规则 + FPV 可见性 + kinetic chain 错峰的完整实现。
- v10（2026-04-14 JSON 路线）：迁移到 `fist_punch_right.json` 为真源，专注"双手内收 + 完全伸直 + 侧身 reach"的 orthodox cross。

两个版本都是可用基线。Java builder 便于单元测试和编译期校验，JSON 资源便于 LLM / headless 工具迭代。**新动画建议从 JSON 起步**（§9），除非需要动态参数化。

---

## §0 核心理念

> **不要追求关节数量，追求动态协同。** MC 的 vanilla player model 每肢物理上就只有 1 个 bend（肘/膝），PlayerAnimator 改不了这个。真实感不来自关节多，来自**峰值错开 + 反相节奏 + 全身分工**。

**僵硬的三大根源**（按出现频率）：

1. **所有骨骼峰值压在同一 tick** → "咔一下全到位"的机器人感
2. **辅助肢（护手/支撑腿）全程静止或只做被动跟随** → 主动作孤立、其他部位像挂件
3. **fade-in 过短，从 vanilla 姿态嗖一下跃迁到 guard pose** → 动画开场就"抬一下"

---

## §1 动画标配骨架（8 帧 tick 分段模板）

以右直拳 `fist_punch_right` 为例，所有**一次性动作**都用这个结构。循环姿态（打坐/悬浮）走单独模式，不在此列。

| 段 | tick | 职责 | 关键帧要做的事 |
|----|------|------|---------------|
| **guard** | 0 | 起始姿态 | 双臂/躯干已经是**可辨识的动作姿态**，不是 vanilla neutral |
| **anticipation** | 0→1 | 反向蓄势 | **只有 torso / body / head** 做反向（迪士尼第一原则） |
| **windup** | 1→3 | 蓄力顶峰 | 躯干扭到极限，发力肢单调深化，其他肢仍在 guard |
| **impact** | 3→5 | kinetic chain 爆发 | 峰值在此——**比动画中点略晚**，给回收段留时间 |
| **overshoot** | 5→6 | 弹性过冲 | 发力肢超出 impact 值 10°，消除"到位即冻结" |
| **recovery** | 6→8 | 回 guard | 所有骨骼回到 tick 0 值，可连击衔接 |

`endTick=8, stopTick=10`（stopTick 必须 > endTick 至少 2，否则构造函数会 +3 兜底，状态锁可能异常）。

---

## §2 八条"不会僵硬"的硬规则

### §2.1 **Guard-pose 框架**（tick 0 ≠ vanilla neutral）

tick 0 的姿态就应该是**静态 thumbnail 里能识别动作类型**的样子——拳击 guard、剑架、抱拳、运功结印。**不要**让动画从"双手自然下垂"开始——那是 vanilla 姿态，PlayerAnimator 动画系统会从 vanilla 直接跃迁到你的 tick 0，fade-in 期间观众看到的"从下垂到抬起"就是最廉价的僵硬感来源。

tick 0 = tick 8。起始与结束姿态一致，连续触发时无需经过"先垂手再举"的过渡。

### §2.2 **Kinetic chain 峰值错开**

力从地面经腿→腰→肩→肘→腕传递，视觉上这五段要**依次到峰值**：

```
tick 3: 躯干 yaw 到 LOAD 极限 (+32°), 后腿膝弯到最深 (45°)
tick 4: 躯干开始反旋, 前腿准备承重
tick 5: 肩 pitch 甩到 -95°, 肘 bend 伸直到 0°, 腕 roll 翻拳
tick 6: 腕收尾 overshoot
```

**错开不超过 2 tick**，否则动作会散；**全压同一 tick** 就是 v2 的失败教训。

### §2.3 **发力肢禁止反向 anticipation**

手臂/腿这些"做主要位移的肢"，在动画时间轴上**从 tick 0 到 tick 5 必须单调朝 impact 方向运动**。

❌ 错误：右臂 pitch `0 → +10° → -95°`（先反向抬一下再前伸）——拳头空间轨迹变 V 形，观众看到就是**"画三角形"/"甩了一下"**。v3 的血泪教训。

✅ 正确：右臂 pitch `-35° → -20° → -95°`（起始已前倾，LOAD 稍微回拉但仍在 guard 范畴，直接爆发前伸）。

**反向 anticipation 只给支撑部位做**：torso 扭转、body 后坐、head 微抬。这些部位本身不承担"发力轨迹"的视觉焦点，反向对它们是"蓄势"；对发力肢却是"抽搐"。

### §2.4 **辅助肢的"呼吸弹性"（load-snap）**

护手、支撑腿这些"不做主要动作的肢"，**不能完全静止**。否则观众会觉得那只手是挂在肩上的假肢。

标配做法——真实运动员的肌肉 load-snap 循环：

| tick | 辅助肢状态 |
|------|-----------|
| 0 (guard) | 基准姿态 |
| 3 (LOAD) | **微放松/展开**（反相位）——pitch/bend 值稍减 |
| 5 (IMPACT) | **猛收紧**（counter-pull，对抗主动肢反作用力）——pitch/bend 值激增 |
| 8 (guard) | 回基准 |

示例（v3.4 左臂护胸，右拳出拳时）：

```
pitch: -45° → -38° (LOAD 微展) → -55° (IMPACT 猛收) → -45°
bend :  90° →  80° (LOAD 微展) → 115° (IMPACT 猛折) →  90°
```

关键：LOAD 和 IMPACT **要反相**。都朝同方向变就只是"慢慢收紧"，读作"跟随"不是"呼吸"。

### §2.5 **正架分工：身体承担视觉位移**

躯干扭转的幅度决定了"蓄力感"的厚度。不要指望手臂自己走一个 60° 弧就能读作"蓄力"——观众看到的是手臂在本地坐标里的动作加上躯干在世界空间里的扭转。

示例（右直拳，orthodox 正架）：

| 部位 | LOAD (tick 3) | IMPACT (tick 5) | 视觉效果 |
|------|---------------|-----------------|----------|
| torso.yaw | **+32°** | **-22°** | 整个上半身扭了 54°——右肩被拽到身后 |
| body.yaw | +18° | -14° | 胯同步扭动一半幅度 |
| body.x | +0.02 | -0.10 | LOAD 回右坐蹬地 → IMPACT 踏左承重 |
| rightArm.pitch | -20° | -95° | 自身 75° 弧，叠加躯干扭转后右拳走过了 >120° 世界弧 |

如果你发现"这一拳读作抬手不像蓄力"，首先检查**躯干幅度够不够**，再调手臂。手臂幅度 ≠ 视觉幅度。

### §2.6 **Overshoot 5→6 tick**

IMPACT 到位后，再给最末端关节（腕/踝）加 1 tick 的"过冲"：
- pitch 再深 10°
- bend 反弹 5-10°

这 1 tick 把"到位即冻结"的 CG 感打碎。弹簧物理：任何高速运动到极限都会有一个微小的 overshoot，LINEAR 停住就是假的。

### §2.7 **Fade-in ≥ 3 tick**

vanilla 下垂（pitch=0, bend=0）→ guard pose（pitch=-45°, bend=90°）的跨度很大。**1 tick fade-in 会让观众看到"突然抬起"**，这是僵硬感的首要来源。

标配：`BongAnimationPlayer.play(player, id, priority)` 默认 **3 tick fade-in**，足够掩盖过渡。只有连击第二拳（已经在 guard 状态）才有理由用 1 tick 急启。

v3.4 里 `BongPunchCombo.trigger` 已经改成 3 tick，后续新动画遵循此默认。

### §2.8 **Ease 选择：INOUTSINE 为主、OUTQUAD 只用 impact**

- **LINEAR**：只用在"必须匀速"的程式化动作（物理演示、调试）。正常人体动作用 LINEAR 永远像机器人。
- **INOUTSINE**：默认。所有 guard→LOAD、overshoot→recovery 段都用它。自然界的加速/减速几乎都是 sinusoidal。
- **OUTQUAD**：只用在 **tick 3→5 impact** 这一段——给爆发动作一个"急刹到位"的顿挫。
- **OUTBOUNCE/OUTELASTIC**：慎用。装饰性太强，容易滑稽。

---

## §3 FPV 可见性要求

第一人称视角下使用 `FirstPersonMode.THIRD_PERSON_MODEL`（见 `BongAnimationPlayer.java:75`）时，观众看到的是完整的上半身模型。**guard 位置必须在视野内**，否则观众根本不知道是哪只手在出拳。

**判断标准**（FPV 视野上沿到胸口下沿）：

| 骨骼位置 | FPV 可见？ |
|---------|-----------|
| 拳头在右颊/耳侧（rightArm.pitch=-15°, bend=130°） | ❌ 出视野上沿 |
| 拳头在右肩前方（rightArm.pitch=-35°, bend=115°） | ✅ 视野右下可见 |
| 拳头在胸口中线（leftArm.pitch=-45°, bend=90°, yaw=+22°） | ✅ 视野前方可见 |

v3.3 → v3.4 的核心修复就是把右拳 guard 从"右颊"降到"右肩前方"，让 FPV 观众能看到出拳侧的准备动作。

**记住**：即使动作在第三人称好看，FPV 下看不到 = 对出拳玩家自己来说毫无反馈。

---

## §4 参考实现

- **`BongAnimations.buildFistPunchRight()`**（`client/src/main/java/com/bong/client/animation/BongAnimations.java`）——v3.4 基线，7 根骨骼（rightArm/leftArm/torso/body/head/rightLeg/leftLeg），所有标配规则的完整实现。新动画**从复制它开始**，不要从空 builder 起步。
- **`BongPunchCombo`**（同目录）——动画 + 屏幕震动 + 拳风声 + 粒子的 **"一次触发多层效果同步"**模板。未来所有战斗类动画都应该有对应的 combo 类整合 VFX。
- **`BongAnimationPlayer`**（同目录）——播放抽象，默认 fade-in 3 tick、fade-out 5 tick，同 id 重触发走 `replaceAnimationWithFade` 平滑过渡。

---

## §5 新动画 Checklist

做新动画前过一遍这份清单：

- [ ] tick 0 是 guard pose（可辨识姿态），不是 vanilla neutral
- [ ] tick 0 == tick 8（起始与收势一致，连击友好）
- [ ] 反向 anticipation **只**给 torso/body/head，发力肢单调朝 impact 方向
- [ ] Kinetic chain 峰值按 腿→腰→肩→肘→腕 顺序错开 1-2 tick
- [ ] IMPACT 在动画 60% 处（8 帧动画 impact 在 tick 5）
- [ ] 辅助肢有 load-snap 反相节奏，不全程静止
- [ ] 最末关节（腕/踝）有 overshoot（impact +10°，1 tick）
- [ ] 躯干扭转幅度 > 手臂自身 yaw 幅度（身体承担视觉位移）
- [ ] 默认 `INOUTSINE`，只有 impact 段用 `OUTQUAD`
- [ ] fade-in 使用默认 3 tick（除非明确是连击急启）
- [ ] FPV 下 guard 位置在视野内（必要时降低拳头高度）
- [ ] 有对应的 combo 类整合 VFX（屏幕震 / 声音 / 粒子）
- [ ] 实机跑 `/anim test <id>` 第一人称 + 第三人称各看一遍

---

## §6 已知不完美 / 未来改进

**v3.4 验收时用户反馈"左臂还是感觉像只是抬一下"**。分析原因：

1. MC vanilla 模型每肢 1 bend 的物理上限，左臂在 guard 保持期（tick 0→3→5→8）能变化的维度只有 pitch/yaw/bend/roll 四个数字，反相幅度给再大也难以完全消除"挂在那里"的感觉
2. FPV 下左臂占屏面积大（横挡胸前），任何微动都被放大，所以即使有 load-snap 弹性观众也容易觉得"单调"
3. fade-in 3 tick 仍然偏短，观众能看到"左臂从下垂快速抬到 guard"的痕迹

**后续可尝试**：
- [ ] 实验 4-5 tick fade-in，代价是启动延迟感
- [ ] 给左臂加 `bendDirection` 的微小摆动（±3°），用前臂独立摆动破静态感
- [ ] 考虑 `FirstPersonMode.VANILLA`（仅手臂跟动画，身体用 vanilla 渲染）对护手感知的差异
- [ ] 天道 Agent 远期生成 JSON 动画时，用 LLM 自检："这个动画有没有 load-snap 反相？有没有 kinetic chain 错峰？"

---

## §7 库内部坑（PlayerAnimator 源码读过才知道）

KosmX `PlayerAnimator 1.0.2-rc1+1.20` 的三条非直觉行为。都是查库源码（`dev.kosmx.playerAnim.*`）才定位出来的根因——从 builder API 签名完全猜不到。同样的结论 + 排错速查表记在 `~/.claude/projects/-home-kiz-Code-Bong/memory/feedback_playeranimator_gotchas.md`，未来 Claude 会话可直接调取；这里是项目内的固化引用。

### §7.1 循环动画单帧 axis 被衰减回 `defaultValue`

`isLooped=true` 时，若某 axis 只在 tick 0 加了单个 keyframe，`KeyframeAnimationPlayer.Axis.findAfter` 的 `isInfinite()` 分支会 fabricate 一个 `endTick+1` 的虚拟 keyframe，值是 `StateCollection` 的 `defaultValue`（pitch/yaw/roll/bend 都是 0°，`rightLeg.z` 是 0.1f 等）。中段就被线性插值回默认值——**你以为的 guard pose 其实大部分时间看不到**。

```java
// KeyframeAnimationPlayer.java:299
if (isInfinite()) {
    return new KeyframeAnimation.KeyFrame(getData().endTick + 1, keyframes.defaultValue);
}
```

**修法**：循环动画里每一个用到的 axis 都必须在 `endTick` 补一个同值 keyframe。一次性动画（`isLooped=false`）不踩这个坑。

**已踩坑动画（v4 批量补 endTick 帧）**：
- `bong:sword_ride`
- `bong:meditate_sit`
- `bong:cultivate_stand`
- `bong:levitate`
- `bong:tribulation_brace`

**排错特征**：如果循环动画"参数翻倍也看不出来效果"，先把 tick 0 改成故意夸张的值（如 pitch 90°），还是看不见就基本确诊。

### §7.2 MC 模型 rigging 没有 IK / skinning —— 大 `leg.pitch` 必然断腿

MC vanilla player model 是 hard-rigid cuboid 拼接，每个 ModelPart 有独立变换矩阵但**没有权重蒙皮也没有 IK solver**。`leftLeg/rightLeg` 的旋转 pivot 在 (±1.9, 12, 0.1)，`leg.pitch=θ` 让腿 cuboid 整体绕 pivot 旋转——顶面前 corner 翘起 `sin(θ)·2 px`、后 corner 下沉同样幅度，而 body 底面是静态的。θ=55° 时 ±1.64 px 的错位肉眼明显可见（"腿腹断连"）；θ=40° 时 ±1.29 px 几乎看不出。

**推论**：
1. 想做"蹲"/"抬腿"/"踢"这种大幅度动作时，**优先用 bend（小腿后折）**堆视觉强度，pitch 控制在 ~40° 以内
2. 反直觉错误：给腿加 `z` 偏移试图"把腿贴回腹部"——这只会把 pivot 挪走，后侧 gap 更大、断连更严重（`sword_ride` v3 的踩坑，v5 撤销）
3. MC 就这样——不要幻想能修到"完全无缝"

**sword_ride 迭代佐证**：v3 (`pitch 55° + leg.z=-0.25`) 断连最严重 → v5 (`pitch 40° + bend 105°`，撤销 `leg.z`) 接近无感。

### §7.3 `body` axis 作用在 MatrixStack，不是 ModelPart

PlayerAnimator 把"整体重心位移"和"上半身扭转"分别走两条通路：

| axis prefix | 作用 | 实现 | 谁一起动 |
|-------------|------|------|---------|
| `body.x/y/z/pitch/yaw/roll` | 整体位移/旋转 | `PlayerRendererMixin.applyBodyTransforms` 在 `setupRotations` RETURN 时改 MatrixStack | 整个玩家（头/发/盔甲/手持物） |
| `torso.x/y/z/pitch/yaw/roll` | 上半身躯干 ModelPart | `PlayerModelMixin.updatePart` 喂 torso | 仅 torso 本身（头/臂/腿各自独立） |
| `head/leftArm/rightArm/leftLeg/rightLeg.*` | 对应 ModelPart | 同上 `updatePart` | 仅该肢 |

注意 **body 不在 updatePart 白名单里**——想"胯扭肩不扭"只能用 `torso.yaw`；`body.yaw` 会把头也一起带过去，变成"整个玩家转身"。

**叠加技巧**：`torso.yaw +30°` + `body.yaw +15°` → 胯扭 15° + 躯干相对胯再扭 15° → 视觉上肩相对胯扭了 30°，适合想要"蓄力时胯先转"的错峰感。

### §7.4 JSON 里 `bendDirection` 字段实际是 `axis` —— 写错名字静默失效

Emotecraft v3 JSON 把 `part.bendDirection`（ModelPart 在 bendy-lib 中的 bend 轴旋转量）的 JSON 键命名为 **`"axis"`**，不是 `"bendDirection"`。写 `"bendDirection": 3.1416` 会被 Gson 完全忽略——parser 只在 `"axis"` 键上取值。

```java
// player-anim-src/dev/kosmx/playerAnim/core/data/gson/AnimationJson.java:147
addPartIfExists(part.bendDirection, "axis", partNode, degrees, tick, easing, turn);
```

**症状**：动画其他部分完全正常、改 bend 值有效、但改 `bendDirection` 值"像没改"——渲染和默认（axis=0）完全一致。v7b → v8 的血泪教训，3 次迭代才定位到是键名错了。

**修法**：JSON 写 `"axis"` 不写 `"bendDirection"`。Java builder 走 `state.bendDirection.addKeyFrame(...)` API 不踩这个坑（那条路径对应的是字段不是 JSON 键）。

**单位陷阱**：`degrees: false` 时 `axis` 值也是**弧度**（不是"轴 ID"）。想表示 180°（让 forearm 往反向折）就写 `3.1415927`，不是 `180`。

---

## §8 版本与迭代记录

| 版本 | 日期 | 关键改动 | 解决问题 |
|------|------|---------|---------|
| v1 | 2026-04-14 | 4 骨骼、6 tick | baseline |
| v2 | 2026-04-14 | +`rightArm.bend`, `body.z`, `head.pitch` | 用户："挥挥手一样" |
| v3 | 2026-04-14 | 7 骨骼 8 tick，kinetic chain 错峰 | 用户："只有一个关节吗" |
| v3.1 | 2026-04-14 | 移除右臂反向 anticipation | 用户："画三角形/甩一下" |
| v3.2 | 2026-04-14 | Guard pose 框架（双臂预举） | 用户："左臂没动作" |
| v3.3 | 2026-04-14 | 正架分工 + body.x 左步 + torso 大扭 | 用户："右臂读作抬手" |
| v3.4 | 2026-04-14 | FPV 可见性降 guard + 左臂 load-snap + fade-in 3 tick | 用户："左臂还是抬一下"（部分解决） |
| v6-v8 | 2026-04-14 | 迁移到 JSON；v7/v7b 的 `"bendDirection"` 键被静默忽略（§7.4） | v8 改成 `"axis"` 后 forearm 终于翻上来 |
| v9 | 2026-04-14 | JSON 正架 cross punch 初版 | 用户：需要"双手向内收 + 完全伸直 + 侧身" |
| v10 | 2026-04-14 | 双手 roll±35° 内收到中线、impact bend=3° 全伸直、torso 62° 扭矩、body.z +0.22m 前冲 | headless Python 渲染工具（§11）迭代，无需进游戏验证 |

**验收**：v3.4（Java）/ v10（JSON）作为两条基线。新动画按 §5 checklist + §9 workflow 起手。

---

## §9 JSON 源文件工作流（v6 起推荐路线）

### §9.1 为什么迁 JSON

Java `AnimationBuilder` 的优势是编译期校验、参数可以跑单测、可以在运行时根据玩家属性调参。劣势是**每次改 3° 都要 `./gradlew build` + 重启客户端**。对于"艺术感觉"驱动的姿态调参（绝大多数战斗动画），反馈循环 2-3 分钟完全是浪费。

JSON 资源文件：
- 改完 `python3 tools/gen_fist_punch_right.py` + `python3 tools/render_animation.py ...` → 1 秒出 PNG → Claude `Read` 看姿态对不对 → 不对再改 → 循环
- 上线只需重 sync jar（`scripts/windows-client.sh --sync-only`），不需要 compile
- LLM-friendly：天道 Agent 未来自动生成战斗动画时直接 emit JSON，比 emit Java 代码简单

### §9.2 Emotecraft v3 JSON 结构

路径：`client/src/main/resources/assets/bong/player_animation/<name>.json`

```jsonc
{
  "version": 3,
  "author": "Bong",
  "name": "fist_punch_right",
  "description": "...",
  "emote": {
    "beginTick": 0,
    "endTick": 10,
    "stopTick": 12,         // 必须 > endTick + 1，否则构造函数 +3 兜底
    "isLoop": false,
    "returnTick": 0,
    "nsfw": false,
    "degrees": false,       // ★ 关键：false = 所有角度用弧度。true（缺省）= 度数。
    "moves": [
      { "tick": 0, "easing": "INOUTSINE", "rightArm": { "pitch": -1.5359 } },
      { "tick": 0, "easing": "INOUTSINE", "rightArm": { "bend": 1.7453 } },
      { "tick": 0, "easing": "INOUTSINE", "rightArm": { "axis": 3.1416 } },
      // ... 每个 (tick, part, axis) 一条记录
    ]
  }
}
```

**move 记录粒度**：官方 parser 支持 `{ "tick": 0, "rightArm": { "pitch": -1.5, "yaw": -0.1, "bend": 1.4 } }` 这种"一个 tick 塞多 axis"的写法，也支持上面那种"一条记录一个 axis"的拆分写法。我们选拆分写法是因为 LLM 生成 JSON 时拆开更不容易出错（每条记录最多只有一个值会让错字的盲区大幅缩小）。

**每个 part 允许的 axis**：`x / y / z / pitch / yaw / roll / bend / axis`。其中 `axis` 对应 `part.bendDirection`（见 §7.4）。

**`degrees: false` 时所有角度用弧度**。Bong 的动画统一这样配，避免度数/弧度混用——`pitch: -1.5359` = -88°、`axis: 3.1416` = π = 180°。

### §9.3 Bong 的 JSON 生成工作流

脚本：`client/tools/gen_fist_punch_right.py`

核心数据结构是一个 pose 表——每个 tick 对应一个 dict，列出该帧所有非零的 part-axis。角度写**度数**（脚本 emit 时转弧度）、xyz 写米。示例：

```python
POSE_V10 = {
    0: dict(  # guard
        easing="INOUTSINE",
        body=dict(x=+0.05, y=-0.05, z=+0.00),
        head=dict(pitch=-5, yaw=-8),
        torso=dict(pitch=+5, yaw=+15),
        rightArm=dict(pitch=-88, yaw=-10, roll=+35, bend=100, axis=180),
        leftArm=dict(pitch=-88, yaw=+10, roll=-35, bend=100, axis=180),
        leftLeg=dict(pitch=-18, yaw=+6, bend=25, z=-0.15),
        rightLeg=dict(pitch=+10, yaw=+5, bend=15, z=+0.05),
    ),
    3: dict(easing="INOUTSINE", ...),   # chamber / LOAD
    5: dict(easing="OUTQUAD",   ...),   # IMPACT
    7: dict(easing="OUTQUAD",   ...),   # recover
    10: dict(easing="INOUTSINE", ...),  # back to guard (= tick 0)
}
```

脚本把 pose 表展开成若干"一条一个 axis"的 move 记录（~120 条），写 JSON。整个 emit 逻辑见 `gen_fist_punch_right.py::emit()`。

**新动画复用**：复制 `gen_fist_punch_right.py` → `gen_<new_name>.py`，改 `POSE_V??`, `DESCRIPTION`, output path 三处。

---

## §10 PlayerAnimator + bendy-lib 变换管线（精确数学）

读源码得到的精确实现。写自定义工具（§11 渲染器、未来的 Agent 自检 prompt 等）必须知道这些。

### §10.1 ModelPart 旋转顺序 = ZYX

`class_630.method_33425(pitch, yaw, roll)` 把值存到 `field_3654/3675/3674`。渲染时 `ModelPart.rotate` 调：

```java
matrices.multiply(new Quaternionf().rotationZYX(roll, yaw, pitch));
```

JOML `rotationZYX(angleZ, angleY, angleX)` = 构造四元数使得应用到向量时相当于**先绕 X 转 pitch、再绕 Y 转 yaw、再绕 Z 转 roll**（内蕴序）。等价于 3×3 矩阵 `M = Rz(roll) · Ry(yaw) · Rx(pitch)`，向量 `v` 变换后 `v' = M · v`。

**意义**：如果某个 part 有 pitch + yaw，**yaw 是在 pitched 后的坐标里转**，不是在世界坐标里转。推论：`rightArm.pitch=-85°` 后的 yaw 其实是在"arm 向前伸"的坐标系里做转动，效果是"整条前伸的胳膊左右扫"，不是"抬在肩部往前后扫"。调参时必须用这个心智模型。

### §10.2 bendy-lib `IBendable.applyBend(bendAxis, bendValue)` 的精确语义

源码：`/tmp/bendy-lib-src/io/github/kosmx/bendylib/impl/IBendable.java:22-68`

```java
Vector3f axis = new Vector3f((float) Math.cos(bendAxis), 0, (float) Math.sin(bendAxis));
Matrix3f m = new Matrix3f().set(getBendDirection().method_23224());
axis.mul(m);
// bend = 绕 axis 轴旋转 bendValue 弧度，旋转中心 = (bendX, bendY, bendZ)
// 仅对"靠近 basePlane 一侧"的 cuboid 顶点做 transform；另一半保持不动
```

核心几何：
- `bendAxis` 控制折弯方向（绕 cuboid 主轴转多少弧度），**不是选轴 ID**
- `bendValue` 控制折弯角度
- 旋转中心 `(bendX, bendY, bendZ)` = cuboid 几何中心：`(sizeX/2 + offsetX, sizeY/2 + offsetY, sizeZ/2 + offsetZ)`
- `getBendDirection()` 在 PlayerAnimator 给两只手/两条腿注册时都是 `Direction.UP`（见 `BipedEntityModelMixin`），所以 `method_23224()` 返回接近单位四元数，`axis` 基本就是 `(cos(bendAxis), 0, sin(bendAxis))`

**对应关系（Direction.UP 下）**：
| bendAxis | axis 向量 | forearm 折弯方向（pitch=0 时） |
|----------|-----------|------------------------------|
| 0 | (+1, 0, 0) = 右 | 绕 +X 轴折 → 在 YZ 平面折 |
| π/2 | (0, 0, +1) = 后 | 绕 +Z 轴折 → 在 XY 平面折（横折） |
| π | (-1, 0, 0) = 左 | 绕 -X 轴折 → 在 YZ 平面折（反向）|
| 3π/2 | (0, 0, -1) = 前 | 绕 -Z 轴折 → 在 XY 平面折（反向）|

**实战经验（pitch=-85° + bend=80° 下手部最终世界位置）**：
| axis | world hand (x, y, z) | 语义 |
|------|---------------------|------|
| 180° | (-6.0, -3.5, -5.5) | 肩外贴脸前（current v10 guard） |
| 200° | (-4.0, -3.1, -5.5) | 稍微内收至肩 |
| 225° | (-1.8, -1.7, -5.4) | 中线但过低（掉到颈部） |
| 270° | (-0.1, +2.4, -5.0) | 死中线但到胸前（太低） |

**axis 改变既影响 x 也影响 y**——不是独立维度，不能靠它单独"把手拉中线"而不掉下来。要同时靠 roll 和 yaw 补偿（见 §12 示例）。

### §10.3 应用顺序（per-frame 渲染）

```
AnimationApplier.updatePart(partName, part):
  1. part.field_3657/3656/3655 = x/y/z     (pivot 偏移)
  2. part.method_33425(pitch, yaw, roll)    (存角度)
  3. IBendHelper.bend(part, (bendAxis, bendValue))  (bend 顶点)

// 渲染阶段（ModelPart.render）:
  4. matrices.translate(pivot / 16)
  5. matrices.multiply(rotationZYX(roll, yaw, pitch))
  6. BendableCuboid.render(matrices, ...)  // 已 bent 的 vertices 被变换
```

所以顶点从 cuboid local 到世界的流水线：
```
v_cuboid_local
  → applyBend(axis, value)     // 在 cuboid local 空间变形
  → × Rz(roll)·Ry(yaw)·Rx(pitch) // ModelPart 绕自己 pivot 转
  → + pivot                    // 平移到 pivot 位置
  → × body_rot                 // MatrixStack 级 body 旋转（§7.3）
  → + body_pos                 // body 平移
  → 世界坐标
```

Python 复刻版在 `client/tools/render_animation.py::solve_skeleton`。

### §10.4 vanilla BipedEntityModel 枢轴表（MC 1.20.1）

MC ModelPart 内部坐标：`+X = 玩家左`、`+Y = 向下`、`+Z = 向后`（玩家面对 `-Z`）。

| Part | pivot | cuboid offset | cuboid size | bend center |
|------|-------|---------------|-------------|-------------|
| head | (0, 0, 0) | (-4, -8, -4) | (8, 8, 8) | — (不 bendable) |
| body (= torso in v3) | (0, 0, 0) | (-4, 0, -2) | (8, 12, 4) | — |
| rightArm | (-5, 2, 0) | (-3, -2, -2) | (4, 12, 4) | (-1, 4, 0) |
| leftArm | (5, 2, 0) | (1, -2, -2) | (4, 12, 4) | (3, 4, 0) |
| rightLeg | (-1.9, 12, 0) | (-2, 0, -2) | (4, 12, 4) | (0, 6, 0) |
| leftLeg | (1.9, 12, 0) | (0, 0, -2) | (4, 12, 4) | (0, 6, 0) |

**bend center 是 cuboid 几何中心**，不是 pivot。pivot 在 cuboid 上边界之上 2 单位（arm）或 cuboid 顶部（leg）。

---

## §11 Headless 骨架渲染工具（Python）

工具：`client/tools/render_animation.py`

### §11.1 做什么

读 emotecraft v3 JSON，按 §10 管线在 Python 里解算每个 tick 的骨架位置，用 PIL 画成三视图（FRONT 正面 / SIDE 右侧 / TOP 俯视）PNG 网格。**不依赖 OpenGL、不依赖 MC、不启动客户端**。

目的：Claude 或人类可以直接 `Read` PNG 判断姿态是否对，改 pose 表→重新渲染→再读图，循环。反馈 2 秒。

### §11.2 用法

```bash
cd client
python3 tools/gen_fist_punch_right.py          # 生成 JSON
python3 tools/render_animation.py \
  src/main/resources/assets/bong/player_animation/fist_punch_right.json \
  -o /tmp/anim_out                              # 每个 keyframe tick 一张 PNG + 合并的 grid.png
# 可选 --ticks 0,5 只渲染指定 tick
```

输出：
- `<name>_t00.png`, `<name>_t03.png`, `<name>_t05.png`, ...（每 keyframe tick 一张三视图）
- `<name>_grid.png`（全部垂直拼接）

每张 PNG 顶部标注：tick 编号、description 摘要、数值摘要（body xyz / torso yaw / rArm pyr+bend / lArm 同理）。

### §11.3 限制

1. **torso.yaw 在 stick figure 看不出**：torso 在 render 里只是一根从 pivot (0,0,0) 到 (0,12,0) 沿 Y 的线段，绕 Y 轴转对线段无视觉影响。要判断 torso 扭转看 **TOP view 的头部朝向 + SIDE view 的腰部位置**，或直接看数值摘要。
2. **只渲染骨架不渲染 mesh**：没有肌肉/皮肤/体积感。"手会不会穿过头"这种碰撞判断靠 Python 看不出，要进游戏验证。
3. **bend 近似值**：只画 upper-arm → elbow → hand 两段直线，没有画 bendable cuboid 的真实 mesh 变形。
4. **腿的 Direction.UP 方向假设**：和 arm 一样用 `(cos(axis), 0, sin(axis))` 构造 bend 轴，但 leg 的 cuboid 和 mesh 布局可能有细微不同。需要进游戏校准。

### §11.4 迭代工作流（推荐）

```
1. 在 gen_<name>.py::POSE_V?? 里改数值
2. python3 tools/gen_fist_punch_right.py         (写 JSON)
3. python3 tools/render_animation.py <json> -o /tmp/anim_out
4. Read /tmp/anim_out/<name>_t05.png            (看 impact 帧)
5. 不对就回 1；对了就：
6. bash scripts/windows-client.sh --sync-only   (build jar 并 deploy)
7. 进游戏 /anim test <id> 最终验证
```

前 5 步 < 30 秒，后 2 步 ~30 秒 build + 用户进游戏看。对比全走游戏的 2-3 分钟循环大大加速。

---

## §12 v10 正架右直拳参考值

Bong 当前"正架 cross punch"基线。所有数值都经过 §11 渲染工具确认世界坐标符合 orthodox 拳击要求（手在中线 / 冲击时完全伸直 / 大身体扭矩 + 前冲 lunge）。

### §12.1 数值表（度数，radians 版本见 `fist_punch_right.json`）

| tick | part | pitch | yaw | roll | bend | axis | x | y | z |
|------|------|-------|-----|------|------|------|---|---|---|
| 0 guard | body | — | — | — | — | — | +0.05 | -0.05 | 0 |
| 0 | head | -5 | -8 | — | — | — | | | |
| 0 | torso | +5 | +15 | — | — | — | | | |
| 0 | rightArm | -88 | -10 | +35 | 100 | 180 | | | |
| 0 | leftArm | -88 | +10 | -35 | 100 | 180 | | | |
| 0 | leftLeg | -18 | +6 | — | 25 | — | | | -0.15 |
| 0 | rightLeg | +10 | +5 | — | 15 | — | | | +0.05 |
| 3 LOAD | body | — | — | — | — | — | +0.10 | +0.03 | -0.05 |
| 3 | head | -5 | -18 | — | — | — | | | |
| 3 | torso | +10 | +30 | — | — | — | | | |
| 3 | rightArm | -55 | -10 | +28 | 145 | 180 | | | |
| 3 | leftArm | -90 | +8 | -25 | 85 | 180 | | | |
| 5 IMPACT | body | — | — | — | — | — | -0.10 | -0.02 | +0.22 |
| 5 | head | -6 | +12 | — | — | — | | | |
| 5 | torso | +4 | -32 | — | — | — | | | |
| 5 | rightArm | **-100** | **-22** | +10 | **3** | 180 | | | |
| 5 | leftArm | -88 | +10 | -35 | 100 | 180 | | | |
| 7 recover | body | — | — | — | — | — | -0.02 | +0.02 | +0.10 |
| 7 | rightArm | -88 | -10 | +30 | 60 | 180 | | | |
| 10 guard | — | (= tick 0) | | | | | | | |

### §12.2 世界坐标验证（Python 渲染器算出）

| tick | 右手（punching） | 左手（guard） | 评估 |
|------|-----------------|---------------|------|
| 0 guard | (-2.0, -3.1, -3.3) | (+3.7, -4.2, -3.6) | 双手内收至中线附近 ✓ |
| 3 LOAD | (-4.0, -1.3, -1.4) | (+4.7, -4.4, -5.0) | 右拳收到胸膛 chambered ✓ |
| 5 IMPACT | (-2.1, +0.4, **-9.2**) | (+3.9, -4.0, -4.3) | 右臂前伸 9.2 过中线 ✓ |
| 7 | (-3.6, -1.5, -8.0) | (+4.6, -4.2, -4.5) | 回收路径 ✓ |
| 10 | (= tick 0) | | |

**"双手向内收"怎么做到的**：靠 `roll=±35°` 扭转整条胳膊的方向 + `yaw=∓10°` 朝中线偏转，而不是靠 `yaw` 硬转。roll 把 forearm 的折弯方向从"纯向上折"变成"斜向内上折"，手落在中线而 y 不掉下来。纯靠 yaw 转同样角度会把手甩到身后侧。

**"完全伸直"**：`bend=3°` 不用 0°——0° 会让 cuboid 硬直接，1-3° 的残余 bend 给 forearm 一点自然弹性。`pitch=-100°` 比 -90° 让拳头略高于肩线（因为 pitch 过了水平）。

**"侧身 reach"**：不靠 body.yaw（会带头一起转）。靠：
- `torso.yaw` 从 +15°（guard）到 +30°（load，后背）到 -32°（impact，前转）= 62° 总扭矩
- `body.z` 从 0 → +0.22m 前冲（头朝向前挪 22cm，相当于前腿 lunge）
- `rightArm.yaw=-22°` 把拳头从肩线拉到过中线 2 单位

### §12.3 如何调参（pose 表里什么改什么）

- 想让 **guard 拳头更贴脸**：加大 `rightArm.roll`（+35 → +40），或减小 `rightArm.bend`（100 → 90）让 forearm 没那么折
- 想让 **impact 拳头飞更远**：`rightArm.pitch` 往 -95 靠（-100 会让拳头略上扬）、`rightArm.bend` 保持 ≤ 5°、`body.z` 加大
- 想让 **冲击更猛烈**：`torso.yaw` 差值加大（当前 +30 → -32 = 62°，可以推到 70-80°）；`body.x` LOAD → IMPACT 差值加大（+0.10 → -0.10 已经是很大的 20cm 侧向 shift）
- 想让 **左手护得更紧**：`leftArm.bend` guard 从 100 → 110（forearm 更折贴脸），impact 时再加一波 load-snap 到 115

---
