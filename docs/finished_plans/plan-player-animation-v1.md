# Bong · plan-player-animation-v1 · 模板

**玩家动画系统专项**。基于 KosmX 的 PlayerAnimator 库，定义 Bong 的玩家动画栈：纯代码 / LLM 生成 JSON 的动画生产线、Server↔Client 触发同步、首批动画资产清单。

**核心理念**：**不依赖 Blockbench/Blender**，全部用 Java 代码 + LLM 生成 JSON 完成动画制作。这是 Bong AI-Native 路线的天然延伸。

**交叉引用**：`plan-vfx-v1.md`（VFX 基础栈）· `plan-particle-system-v1.md`（VFX 协议复用）· `../plan-HUD-v1.md` · `../plan-combat-no_ui.md` / `../plan-combat-ui_impl.md`。

---

## §0 设计轴心

- [x] **零美术工具依赖**：纯 Java 代码 / LLM 生成 JSON，禁用 Blender/Blockbench
- [x] **AI-Native 生产线**：天道 Agent 或开发期 LLM 直接吐 keyframe 代码
- [x] **服务端权威触发**：动画播放是表演层，触发由 server 决定
- [x] **协议复用**：和 VFX 共用 `bong:vfx_event` CustomPayload 通道
- [x] **多层叠加**：上半身动作 + 下半身行走 + 全身姿态独立通道

---

## §1 技术基础

### 1.1 库依赖

- KosmX **PlayerAnimator** ([Modrinth](https://modrinth.com/mod/playeranimator) | [GitHub](https://github.com/KosmX/minecraftPlayerAnimator))
- **当前状态（2026-04-13 审计）**：✗ 尚未加入 `client/build.gradle`，实施第一步即是添加 `modImplementation` 依赖并验证编译
- Fabric 1.20.1 兼容（库侧已确认，gradle 接入后需跑 `./gradlew test build` 复核）
- 纯客户端库（**服务端无需安装**，对 Valence Rust 服务端友好）
- 是 Better Combat / Epic Fight 的底层动画引擎

### 1.2 核心 API（亲眼源码确认，2026-04-13）

| 类 / 方法 | 用途 |
|----------|------|
| `KeyframeAnimation.AnimationBuilder` | 构建动画 |
| `StateCollection.State.addKeyFrame(tick, value, ease, rotate, degrees, easingArg)` | 添加关键帧 |
| `PlayerAnimationAccess.getPlayerAnimLayer(player)` | 获取玩家动画栈 |
| `AnimationStack.addAnimLayer(priority, layer)` | 多层叠加 |
| `ModifierLayer<KeyframeAnimationPlayer>` | 动画播放器（带 null 安全） |
| `AbstractFadeModifier.standardFadeIn(ticks, ease)` | 淡入淡出 |
| `AdjustmentModifier` | 运行时实时调整骨骼变换 |

### 1.3 可控骨骼

`head` · `torso` · `rightArm` · `leftArm` · `rightLeg` · `leftLeg` · `rightItem` · `leftItem` · `body`（整体）

每根骨骼可调：`x, y, z, pitch, yaw, roll, bend, bendDirection, scaleX, scaleY, scaleZ`

**单位**：度数（`degrees=true` 时）；时间单位 tick（20 tick = 1 秒）

### 1.4 缓动函数

`Ease.LINEAR / EASEINQUAD / EASEOUTQUAD / EASEINOUTCUBIC / EASEINBOUNCE / ...`（详见 `dev.kosmx.playerAnim.core.util.Ease`）

---

## §2 两条生产路径

### 2.1 路径 A：纯 Java 代码（推荐）

```java
public static KeyframeAnimation buildSwordSwing() {
    var b = new KeyframeAnimation.AnimationBuilder(AnimationFormat.UNKNOWN);
    b.endTick = 10;  // 0.5 秒
    b.isLoop = false;

    b.rightArm.pitch.addKeyFrame(0,  -80f, Ease.LINEAR,    0, true, null);
    b.rightArm.pitch.addKeyFrame(10,  60f, Ease.EASEOUTQUAD, 0, true, null);
    b.rightArm.roll .addKeyFrame(0,  -10f, Ease.LINEAR,    0, true, null);
    b.rightArm.roll .addKeyFrame(10,  10f, Ease.LINEAR,    0, true, null);

    return b.build();
}
```

**优势**：编译期检查、IDE 跳转、可单元测试  
**适用**：固定动画、需要参数化的动画（按境界缩放幅度）

### 2.2 路径 B：JSON 资源（LLM 生成）

```json
{
  "name": "sword_swing",
  "version": 3,
  "emote": {
    "beginTick": 0,
    "endTick": 10,
    "isLoop": false,
    "moves": [
      { "tick": 0,  "right_arm": { "pitch": -80 }, "easing": "linear" },
      { "tick": 10, "right_arm": { "pitch": 60 },  "easing": "easeOutQuad" }
    ]
  }
}
```

放 `client/src/main/resources/assets/bong/player_animation/<name>.json`

**优势**：不重编译可热替换、LLM 一句话生成、非程序员可改  
**适用**：大量招式变体、剧情演绎动画、玩家 UGC（远期）

### 2.3 何时用哪条

| 场景 | 路径 |
|------|------|
| 核心战斗动画（剑挥/格挡/受击） | A（参数化、单元测试） |
| 修仙姿态（打坐/突破/渡劫） | A 或 B |
| 大量招式变体（不同心法的挥剑微变） | B（LLM 批量生成） |
| 情景演绎（NPC 对话动作、剧情） | B |
| 天道 Agent 实时命名 + 生成的招式 | B（运行时 JSON 字符串注入） |

---

## §3 注册表与生命周期

### 3.1 `BongAnimationRegistry`

```java
public class BongAnimationRegistry {
    static Map<Identifier, KeyframeAnimation> ANIMATIONS = new HashMap<>();

    public static void register(Identifier id, KeyframeAnimation anim) { ... }
    public static void registerJson(Identifier id, Identifier resource) { ... }  // 从 assets 加载
    public static KeyframeAnimation get(Identifier id) { ... }
}
```

- [x] 静态注册（编译期）+ 动态注册（运行时 JSON 字符串）
- [x] 客户端启动时扫描 `assets/bong/player_animation/*.json` 自动注册
- [x] 提供 `/bong anim test <id>` debug 命令本地试播

### 3.2 播放抽象

```java
public class BongAnimationPlayer {
    public static void play(AbstractClientPlayerEntity player,
                            Identifier animId,
                            int priority,
                            int fadeInTicks);

    public static void stop(AbstractClientPlayerEntity player,
                            Identifier animId,
                            int fadeOutTicks);
}
```

- [x] 自动 fade in/out（默认 3 tick）
- [x] 同 priority 自动替换
- [x] 维护"当前播放层"映射表，支持精确停止

### 3.3 多层 Priority 约定

| Priority | 用途 |
|----------|------|
| 100-499 | 持续姿态（打坐、悬浮、运功） |
| 500-999 | 移动相关（修改步态、轻功） |
| 1000-1999 | 战斗动作（挥剑、出掌、御剑） |
| 2000-2999 | 受击 / 倒地 / 复活 |
| 3000+ | 剧情演绎（不可被打断的天劫、突破） |

---

## §4 Server → Client 触发协议

### 4.1 复用 `bong:vfx_event`

不新开 channel，复用 `plan-particle-system-v1.md §2.2` 的 VFX 通道。新增事件类型：

```json
{
  "type": "play_anim",
  "target_player": "uuid",
  "anim_id": "bong:sword_swing",
  "priority": 1000,
  "fade_in_ticks": 3,
  "speed": 1.0
}
```

```json
{
  "type": "stop_anim",
  "target_player": "uuid",
  "anim_id": "bong:sword_swing",
  "fade_out_ticks": 5
}
```

### 4.2 广播范围

- 动画必须**广播给附近所有玩家**（不只是 target_player 自己）—— 旁观也要能看到挥剑
- 默认范围：64 格（可配）
- 配合 §1.4 plan-particle-system-v1 的 ChunkLayer viewer 过滤

### 4.3 客户端自演 vs 服务端广播

| 类型 | 归属 |
|------|------|
| **持续姿态**（运功、打坐、悬浮） | 客户端读 `player_state` 状态位自演 |
| **一次性动作**（挥剑、出掌、御剑、突破、渡劫） | 服务端广播 `play_anim` |
| **环境/idle 动画**（呼吸、扫视） | 客户端自己加，无需广播 |

### 4.4 动态 JSON 注入（远期）

天道 Agent 可生成完整 JSON payload，server 转发给 client：

```json
{
  "type": "play_anim_inline",
  "target_player": "uuid",
  "anim_json": "{ ... 完整 KeyframeAnimation JSON ... }"
}
```

客户端解析后注册到 `BongAnimationRegistry` 临时表 + 立即播放。**这是 LLM 生成动画的关键路径**。

---

## §5 首批动画资产清单

### 5.1 战斗类（Phase 1）

| id | 时长 | 描述 | 优先级 |
|----|------|------|--------|
| `bong:sword_swing_horiz` | 10t | 横扫 | 1000 |
| `bong:sword_swing_vert` | 10t | 下劈 | 1000 |
| `bong:sword_stab` | 8t | 直刺 | 1000 |
| `bong:fist_punch_left` | 6t | 左拳 | 1000 |
| `bong:fist_punch_right` | 6t | 右拳 | 1000 |
| `bong:palm_thrust` | 12t | 推掌（带气劲） | 1000 |
| `bong:guard_raise` | 4t | 举手格挡 | 1000 |
| `bong:dodge_back` | 8t | 后跃闪避 | 1000 |
| `bong:beng_quan` | 8t | 爆脉流崩拳（收肘蓄力 + 零距前冲贯拳） | 1000 |
| `bong:hit_recoil` | 6t | 受击退缩 | 2000 |

### 5.2 修仙姿态类

| id | 时长 | 描述 | 优先级 |
|----|------|------|--------|
| `bong:meditate_sit` | loop | 打坐运功（双手结印） | 200 |
| `bong:cultivate_stand` | loop | 站桩运功 | 200 |
| `bong:levitate` | loop | 御空悬浮 | 300 |
| `bong:sword_ride` | loop | 御剑飞行 | 300 |
| `bong:cast_invoke` | 15t | 引动法宝（双手抬起） | 1000 |
| `bong:rune_draw` | 20t | 凌空画符 | 1000 |

### 5.3 剧情演绎类

| id | 时长 | 描述 | 优先级 |
|----|------|------|--------|
| `bong:breakthrough_burst` | 60t | 境界突破（手臂展开 + 仰天） | 3000 |
| `bong:tribulation_brace` | loop | 抗劫姿态（双手交叉） | 3000 |
| `bong:enlightenment_pose` | 40t | 顿悟（双手合十低头） | 3000 |
| `bong:death_collapse` | 30t | 道消身陨 | 3000 |
| `bong:bow_salute` | 25t | 抱拳行礼 | 500 |

### 5.4 资源量预估

约 21 个动画（首批 20 + beng_quan 增量）× 平均 2 行 keyframe = **总 keyframe 数 < 110**。LLM 生成 JSON 一次性出货，**1 天可完成全部首批**。

---

## §6 LLM 生产工作流

1. **需求描述**：开发者用自然语言描述（"剑修横扫，右臂从左肩位置 90° 横扫到右侧 -90°，0.5 秒，末尾有顿挫感"）
2. **LLM 生成**：直接吐 JSON 或 Java AnimationBuilder 代码
3. **本地预览**：`/bong anim test <id>` 命令在客户端立即试播
4. **微调**：调整 keyframe 数值或 ease 函数，重新加载
5. **入库**：满意后提交进 `assets/bong/player_animation/` 或 `BongAnimations.java`
6. **审核**：开发期由人 review，运行时由天道 Agent 自审（远期）

---

## §7 实施节点

- [x] §1.1 引入 PlayerAnimator gradle 依赖，编译通过
- [x] §3.1 `BongAnimationRegistry` 骨架
- [x] §3.2 `BongAnimationPlayer` 播放抽象
- [x] §3.1 `/anim test` debug 命令（见 `BongAnimCommand`）
- [x] 第一个动画原型：`bong:sword_swing_horiz`（纯 Java 实现）
- [x] 第二个动画原型：从 JSON 加载（Phase 2 已完成全量迁移——20 个 Phase 1 动画全部入 `assets/bong/player_animation/*.json`，PlayerAnimator resource reload listener 自动加载；`BongAnimationRegistry` 走 JSON-first fallback Java，Java 源现已空）
- [x] §4.1 协议 schema：`play_anim` / `stop_anim` 加入 VfxEvent TypeBox（2026-04-14 完成：`agent/packages/schema/src/vfx-event.ts` 双 variant Union + `server/src/schema/vfx_event.rs` roundtrip + sample.json 双端对齐；客户端 `VfxEventEnvelope` / `VfxEventRouter` / `ClientAnimationBridge` 解析 `bong:vfx_event` CustomPayload 并派发到 `BongAnimationPlayer`，16 个单测覆盖 play/stop/错误三档；未包含 `speed`，待 KeyframeAnimationPlayer 接 setSpeed API 再扩）
- [ ] 端到端 demo：服务端发 `play_anim` → 附近玩家看到挥剑（schema + client receiver 已通；服务端 `network/vfx_event_emit.rs` 已实装 `VfxEventRequest::PlayAnim/StopAnim` 派发器与 `/bong-vfx play <anim_id>` 调试命令，按 `VFX_BROADCAST_RADIUS` 距离过滤广播，**手动触发链路已贯通**；剩 combat/cultivation 业务 system → `VfxEventV1::play_anim` 自动映射调用点）
- [x] §5.1 战斗类 10 个动画批量生产（sword_swing_horiz/vert/stab、fist_punch_left/right、palm_thrust、guard_raise、dodge_back、hit_recoil、beng_quan ← 2026-04-29 增量）
- [x] §5.2 修仙姿态 6 个动画（meditate_sit、cultivate_stand、levitate、sword_ride、cast_invoke、rune_draw）
- [x] §5.3 剧情演绎 5 个动画（breakthrough_burst、tribulation_brace、enlightenment_pose、death_collapse、bow_salute）
- [x] §3.3 多层 priority 叠加测试（行走 + 挥剑同时）—— `BongAnimationPlayerMultiLayerTest` 6 个 case 覆盖：两档同播 / 三档同播（姿态+移动+战斗）/ priority 升序排列 / stop 单条不影响其它 / 同 id 重触发走 replaceAnimationWithFade 不新增层 / stop 不存在 id 不误伤；测试用 `BongAnimationPlayer.playOnStack` seam 绕开 Mixin 依赖
- [ ] §4.4 动态 JSON 注入原型（天道 Agent 生成）

**Phase 1 资产**（§5.1/§5.2/§5.3 共 21 个 = 首批 20 + 2026-04-29 增量 beng_quan）全部落地，见 `BongAnimations.java` v3.4 conventions。本地 `/anim test <id>` 即可单独验证每个。首批视觉验证通过 PunchCombo demo 已验收（见 `docs/player-animation-conventions.md` §7 迭代简史）。

**§4.1 协议层已通**（2026-04-14）：`bong:vfx_event` CustomPayload 通道双端对齐 + 客户端 `ClientAnimationBridge` 派发到 `BongAnimationPlayer`。下一步瓶颈挪到端到端 demo——服务端 Bevy system 要把战斗/剧情事件翻译成 `VfxEventV1::play_anim` 并按 §4.2 距离过滤广播；schema + client receiver 已经准备好接这个调用点。

---

## §8 已知风险

- **第一人称视角需显式配置**：PlayerAnimator 动画默认不在第一人称渲染（`FirstPersonMode.NONE`）。需要在每个动画上设 `FirstPersonMode.VANILLA`（手臂跟动画）或 `FirstPersonMode.THIRD_PERSON_MODEL`（直接渲染第三人称模型上半身）。不是硬限制，但每个新动画都要记得配。
- **PlayerAnimator 升级风险**：API 不算稳定，作者偶尔重命名包/类。锁定具体版本号
- **多人动画带宽**：20 人战斗场景每秒可能数百次动画事件，依赖 §4.2 的距离过滤 + §4.3 的客户端自演分流
- **运行时 JSON 注入安全**：天道 Agent 生成的 JSON 必须做 schema 校验，避免恶意 keyframe（极大值导致客户端崩溃）
- **vanilla 行为冲突**：PlayerAnimator 的高优先级层会覆盖 vanilla 的攻击挥手动画，可能让玩家感觉"两个动画打架"

---

## §9 开放问题

- [ ] 第一人称视角下的默认 `FirstPersonMode` 选哪个？`VANILLA`（仅手臂）适合挥剑/出掌，`THIRD_PERSON_MODEL`（全身）适合翻滚/抱拳/打坐——可能按动画类型分组配置
- [ ] 是否需要"动画事件回调"（动画进行到某 tick 触发音效/粒子）？PlayerAnimator 是否原生支持？
- [ ] 持物变换（rightItem / leftItem 骨骼）的 vanilla 兼容如何处理？
- [ ] LLM 生成的 JSON schema 校验由谁做（client / server / agent）？
- [ ] 是否需要"动画即时录制"工具：玩家在游戏内通过命令/按键记录关键帧，导出为 JSON？
- [ ] 非人形 NPC（如灵兽、傀儡）的动画是否也走 PlayerAnimator？还是单独走 GeckoLib？

---

## §10 参考

**调研报告**（2026-04-13 sonnet 调研，见对话历史）：
- PlayerAnimator GitHub：https://github.com/KosmX/minecraftPlayerAnimator
- PlayerAnimator Modrinth：https://modrinth.com/mod/playeranimator
- 核心源码路径：`dev.kosmx.playerAnim.core.data.KeyframeAnimation`（含内嵌 `AnimationBuilder`）
- 客户端 API 入口：`dev.kosmx.playerAnim.minecraftApi.PlayerAnimationAccess`

**关联设计**：
- `plan-vfx-v1.md`（光影栈总纲）
- `plan-particle-system-v1.md`（VFX 协议复用 `bong:vfx_event` channel）
- Better Combat 调研结论（2026-04-13）：放弃 BC 集成，改用 PlayerAnimator 自建动画层 —— 本 plan 是该决策的落地

**LLM 协作样式**：
- 鬼谷八荒招式动画（参考"剑修横扫/突刺"的视觉感）
- 太吾绘卷武学动作设计（参考"内外功不同动作风格"）

---

## §11 进度日志

- 2026-04-25：审计代码现状——Phase 1 全部 20 个动画 JSON 已落 `client/src/main/resources/assets/bong/player_animation/`（对应 `client/tools/gen_*.py` 生成器全套齐备）；client 侧 `BongAnimationRegistry/Player/Bridge` + `BongAnimCommand` 已就绪；server 侧 `network/vfx_event_emit.rs` 已实装 PlayAnim/StopAnim 广播器与 `/bong-vfx play` 调试命令，端到端手动链路贯通；剩 combat/cultivation system 自动映射 + §4.4 inline JSON 注入两项未启动。
- 2026-04-29：战斗资产增量——`beng_quan.json`（爆脉流崩拳，8t，priority 1000）入库，对应 `client/tools/gen_beng_quan.py` 生成器；commit `b0302396` "feat: 落地爆脉崩拳真实结算"。Phase 1 资产数从 20 → 21，§5.1 战斗类 9 → 10。

## Finish Evidence

- 服务端业务事件自动映射完成：`AttackIntent` / `DefenseIntent` / `CombatEvent` / `BreakthroughOutcome` / `TribulationAnnounce` / `TribulationFailed` 统一转换为 `VfxEventRequest::PlayAnim`，并保持 `BurstMeridian` 走已有 `bong:beng_quan` 专用链路；commit `531438c5 feat(player-animation): 自动触发业务动画`。
- §4.4 动态 JSON 注入原型完成：`play_anim_inline` 已加入 Rust schema、TypeBox/generated schema、Java client envelope/router/bridge/registry；客户端运行时解析 PlayerAnimator JSON 后注册 inline 源并立即播放；commit `c76d8988 feat(player-animation): 支持 inline 动画 JSON 注入`。
- 验证通过：`server/ cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`。
- 验证通过：`client/ JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 PATH="/usr/lib/jvm/java-17-openjdk-amd64/bin:$PATH" ./gradlew test build`。
- 验证通过：`agent/ npm run build && (cd packages/tiandao && npm test) && (cd packages/schema && npm test)`。
