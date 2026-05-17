# Bong · plan-iris-integration-v1 · 模板

**Iris 光影集成专项**（**可选上层**，建立在 `plan-vfx-v1.md` 基础栈之上）。引入 Iris 后能做到 vanilla 渲染 API 做不到的事：全屏水墨/血月/灵压扭曲 shader、修仙状态驱动光影。

**前置依赖**：
- `plan-vfx-v1.md`（VFX 基础栈）—— 必须先完成 Phase 1
- `plan-particle-system-v1.md` / `plan-player-animation-v1.md` 子系统稳定

**本 plan 是可选层**：基础栈玩家不装 Iris 也能拿到 70% 修仙观感。本 plan 解锁剩下 30% 的 shader 级表现。

**交叉引用**：`plan-tribulation-v1.md`（天劫画面）· `plan-narrative-v1.md`（顿悟画面）

---

## §0 设计轴心

- [ ] **基础栈先行**：本 plan 启动前必须 `plan-vfx-v1.md` Phase 1 完成
- [ ] **Iris 设为软依赖**：玩家不装 Iris 仍能玩，只是少了 shader 级效果
- [ ] **修仙状态驱动光影**：境界/灵压/天劫进度通过 Mixin 喂给 shader，**这是 Bong 独家差异化**（其他 mod 没做过）
- [ ] **一份 shader pack 走天下**：用 uniform 控制效果切换，不做 pack 切换（更稳）

---

## §1 调研结论速览（2026-04-13）

### 1.1 为什么要走 Iris

vanilla `PostEffectProcessor` 在 Iris 激活时**被完全绕过**（`MixinPostChain.java` 空实现），且**没有 `support_vanilla_post` 开关**。想做全屏 shader 效果的唯一稳路是：成为 Iris 生态的一部分。

### 1.2 Iris API 能力清单（亲眼源码）

| 能力 | API | 稳定性 |
|------|-----|--------|
| 检测 shader pack 激活 | `IrisApi.getInstance().isShaderPackInUse()` | ✅ 正式 |
| 检测是否阴影通道 | `IrisApi.isRenderingShadowPass()` | ✅ 正式 |
| 启/禁 shader | `getConfig().setShadersEnabledAndApply(bool)` | ✅ 正式 |
| 打开选择界面 | `openMainIrisScreenObj(parent)` | ✅ 正式 |
| 太阳角度 | `getSunPathRotation()` | ✅ 正式（v0.2） |
| 程序化切换 shader pack | `Iris.getIrisConfig().setShaderPackName(name)` + `Iris.reload()` | ⚠️ 内部 API，版本风险 |

### 1.3 最大瓶颈

**Iris 没有公开"mod → shader uniform 注入" API**。修仙状态喂给 shader 必须 Mixin 注入 `CommonUniforms` / `HardcodedCustomUniforms`，每次 Iris 大版本升级要回归测试。

### 1.4 禁区

- ❌ Mixin 改 `WorldRenderer` —— 与 Iris Mixin 硬冲（Botania 前车之鉴）
- ❌ 自写 `GlProgram` —— Iris 官方不兼容列表点名
- ❌ 假设 vanilla `PostEffectProcessor` 在 Iris 下能跑

---

## §2 依赖与定位

### 2.1 软依赖声明

**当前状态（2026-04-13 审计）**：`client/src/main/resources/fabric.mod.json` 只有 `depends`（fabricloader / minecraft / java / fabric-api / owo-lib），**尚未声明 `recommends`**。实施阶段需在该文件新增：

```json
"recommends": {
  "iris": ">=1.6.0"
}
```

mod id 确认为 `iris`（非 `iris-flywheel-compat`）。

**不做硬依赖**——保留"不装 Iris 也能跑"的玩家路径，只是没有 shader 级效果。

### 2.2 启动检测与提示

- [ ] 客户端启动检测 `FabricLoader.getInstance().isModLoaded("iris")`
- [ ] 检测到则启用本 plan 全部能力
- [ ] 未检测到则在主菜单加一个非阻塞提示："安装 Iris 解锁修仙光影"
- [ ] 玩家可关闭提示

---

## §3 Bong 专属 shader pack（`bong_xianxia.zip`）

### 3.1 内容范围

一份 shader pack，所有效果通过 uniform 控制：

| 效果 | 触发 uniform | 备注 |
|------|------------|------|
| 水墨化 | `bong_inkwash` (0-1) | Sobel 边缘检测 + 灰阶 + 纸纹 |
| 血月红化 | `bong_bloodmoon` (0-1) | 全屏色调偏红 + 高光蓝色压制 |
| 灵压扭曲 | `bong_lingqi` (0-1) | 屏幕扭动 + 径向涟漪 |
| 顿悟泛光 | `bong_enlightenment` (0-1) | 全局 bloom 强化 + 色相微偏金 |
| 天劫黑云 | `bong_tribulation` (0-1) | 顶部边缘暗化 + 雷光闪烁 mask |
| 入定景深 | `bong_meditation` (0-1) | 假 DoF（按 depth buffer 模糊） |
| 入魔黑暗 | `bong_demonic` (0-1) | 全局降饱和 + vignette 浓化 |
| 强风 | `bong_wind_strength` (0-1) + `bong_wind_angle` (0-2π) | 方向性屏幕拉丝 + 雾气偏移 + 草/叶顶点摆幅加剧（gbuffers_terrain.vsh） |

### 3.2 资源结构

```
bong_xianxia.zip/
├── shaders/
│   ├── shaders.properties
│   ├── final.fsh               # 最终 composite
│   ├── composite1.fsh          # 第一个后处理 pass
│   ├── composite2.fsh          # ...
│   └── lib/
│       ├── inkwash.glsl        # 水墨函数
│       ├── bloodmoon.glsl
│       └── ...
└── pack.png
```

### 3.3 分发

无 Iris 官方支持的"mod 内 shader pack 自动加载"机制，但可自行实现：

- [ ] mod resources 内带 `bong_xianxia.zip`
- [ ] `ClientModInitializer` 启动时检查 `.minecraft/shaderpacks/bong_xianxia.zip` 是否存在
- [ ] 不存在则复制（可选弹窗征求同意）
- [ ] 调 `Iris.getIrisConfig().setShaderPackName("bong_xianxia.zip")` 提示启用（不强制）

---

## §4 状态驱动 uniform 注入（核心差异化）

### 4.1 Mixin 路径

```java
@Mixin(CommonUniforms.class)
public class MixinCommonUniforms {
    @Inject(method = "addNonDynamicUniforms", at = @At("TAIL"))
    private static void bongInjectUniforms(UniformHolder holder, CallbackInfo ci) {
        holder.uniform1f(PER_FRAME, "bong_realm",         () -> BongClientState.getRealmLevel());
        holder.uniform1f(PER_FRAME, "bong_lingqi",        () -> BongClientState.getLingQi());
        holder.uniform1f(PER_FRAME, "bong_tribulation",   () -> BongClientState.getTribulation());
        holder.uniform1f(PER_FRAME, "bong_enlightenment", () -> BongClientState.getEnlightenment());
        holder.uniform1f(PER_FRAME, "bong_inkwash",       () -> BongClientState.getInkwash());
        holder.uniform1f(PER_FRAME, "bong_bloodmoon",     () -> BongClientState.getBloodmoon());
        holder.uniform1f(PER_FRAME, "bong_meditation",    () -> BongClientState.getMeditation());
        holder.uniform1f(PER_FRAME, "bong_demonic",       () -> BongClientState.getDemonic());
        holder.uniform1f(PER_FRAME, "bong_wind_strength", () -> BongClientState.getWindStrength());
        holder.uniform1f(PER_FRAME, "bong_wind_angle",    () -> BongClientState.getWindAngle());
    }
}
```

### 4.2 `BongClientState` 抽象

- [ ] 客户端单例，所有 uniform supplier 从这里读
- [ ] 状态来源：服务端 `bong:server_data` payload + 客户端事件淡入淡出
- [ ] 提供平滑过渡：`BongClientState.tickInterpolate()` 每帧让 0/1 状态柔和过渡
- [ ] 调试命令 `/bong shader uniform <name> <value>` 临时覆写

### 4.3 Uniform 契约文档

- [ ] 单独维护 `docs/iris_uniform_contract.md`
- [ ] 列每个 uniform 的：值范围、语义、更新频率、客户端淡入规则、来自服务端字段
- [ ] shader pack 作者按此文档写 GLSL（开源后第三方 shader pack 也能对接）

---

## §5 实施节点

### 5.0 Phase 0 —— 接入管线 + 测试平台（先做这个）

**目标**：搭好完整的数据通路和调试工具，让后续每个 shader 效果都能"写 GLSL → 热改 uniform → 即时看结果"，不再有环境障碍。

#### 5.0.1 依赖声明 & 运行时检测

- [ ] `fabric.mod.json` 新增 `"recommends": { "iris": ">=1.6.0" }`
- [ ] 启动检测 `FabricLoader.getInstance().isModLoaded("iris")`，结果写入 `BongIrisCompat.isAvailable()`
- [ ] 未检测到 Iris 时：所有 Iris 代码路径 no-op，零副作用
- [ ] 检测到 Iris 时：日志打印版本号，激活 Mixin 注入路径

#### 5.0.2 BongClientState 单例

- [ ] `client/src/main/java/com/bong/client/iris/BongClientState.java`
- [ ] 内部 `float[] uniforms` 数组，按 enum `BongUniform` 索引（realm / lingqi / tribulation / enlightenment / inkwash / bloodmoon / meditation / demonic / wind_strength / wind_angle）
- [ ] `set(BongUniform, float)` / `get(BongUniform)` → 直写直读
- [ ] `tickInterpolate()` 每 client tick 对所有 uniform 做 lerp 平滑（速率可配，默认 0.1/tick）
- [ ] 来源：服务端 `bong:shader_state` CustomPayload（S2C 频率 = 状态变化时 + 每 20 tick heartbeat）
- [ ] 不依赖 Iris 本身——即使没装 Iris，`BongClientState` 照常更新（其他系统也可读）

#### 5.0.3 Mixin 注入点

- [ ] `MixinCommonUniforms.java`：§4.1 代码，注入全部 10 个 uniform
- [ ] Mixin 配置仅在 `BongIrisCompat.isAvailable()` 为 true 时注册（条件 Mixin plugin / `@Pseudo` / refmap 隔离）
- [ ] 注入失败（Iris 内部 API 变动）时 catch + 日志警告，不崩客户端

#### 5.0.4 最小测试 shader pack

- [ ] `client/src/main/resources/assets/bong/iris/bong_test.zip`（仅用于开发/CI，不随正式包分发）
- [ ] 内容：`shaders/final.fsh` 读 `uniform float bong_test_uniform`，按值线性叠加红色 tint
- [ ] 启动时自动复制到 `.minecraft/shaderpacks/bong_test.zip`（仅 dev 环境，release 不带）
- [ ] README 写明手动测试步骤：装 Iris → 选 bong_test → `/bong shader set bong_test_uniform 0.5` → 屏幕半红

#### 5.0.5 调试命令

- [ ] `/bong shader list` —— 列出所有 uniform 当前值
- [ ] `/bong shader set <name> <value>` —— 临时覆写（跳过服务端，直写 BongClientState）
- [ ] `/bong shader reset` —— 清除覆写，恢复服务端驱动
- [ ] `/bong shader dump` —— 打印 Iris 检测状态、当前 shader pack 名、注入是否成功

#### 5.0.6 服务端 payload 定义

- [ ] `server/src/iris/mod.rs`：`ShaderStatePayload` 结构体，字段对应 10 个 uniform float
- [ ] 触发逻辑暂时只挂 dev 命令：`/shader_push <uniform> <value>` 直接广播 S2C payload
- [ ] 后续各系统（天劫、血月、风场等）在自己的 plan 里往 `ShaderStatePayload` 写值

#### 5.0.7 验收标准

- [ ] `./gradlew runClient` 启动，日志出现 `[BongIris] Iris detected v1.x.x, uniform injection active`
- [ ] 加载 `bong_test` shader pack，执行 `/bong shader set bong_test_uniform 1.0`，屏幕明显偏红
- [ ] 执行 `/bong shader reset`，屏幕恢复正常
- [ ] 不装 Iris 启动，日志出现 `[BongIris] Iris not found, shader features disabled`，无报错
- [ ] 单测：`BongClientState` 的 lerp 逻辑、uniform 枚举完整性、set/get 正确性

---

### 5.1 Phase A —— 首个效果端到端（血月）

**前置**：Phase 0 验收通过

- [ ] 正式 shader pack `bong_xianxia.zip` 骨架：`shaders.properties` + `composite1.fsh` + `final.fsh` + `lib/common.glsl`
- [ ] `lib/bloodmoon.glsl`：读 `bong_bloodmoon`，全屏色调偏红 + 高光蓝色压制
- [ ] 服务端血月事件 → 写 `ShaderStatePayload.bloodmoon = 1.0` → S2C → `BongClientState` lerp 渐入
- [ ] 端到端验收：触发血月 → 2 秒内全屏渐红 → 事件结束 → 2 秒渐回

### 5.2 Phase B —— 核心效果组（5 个）

- [ ] `bong_inkwash`：Sobel 边缘 + 灰阶 + 纸纹 noise
- [ ] `bong_lingqi`：径向涟漪 + 屏幕微扭
- [ ] `bong_tribulation`：顶部暗化 + 雷光闪烁 mask
- [ ] `bong_enlightenment`：bloom 强化 + 色相微偏金
- [ ] `bong_meditation`：depth buffer 假 DoF
- [ ] 每个效果独立 `.glsl` lib，composite pass 按 uniform > 0.01 条件跳过（零开销）
- [ ] 性能基准：6 效果同时 1.0 时 RTX 3060 保持 60fps（1080p）

### 5.3 Phase C —— 环境效果组（风 + 入魔）

- [ ] `bong_wind_strength` + `bong_wind_angle`：
  - composite pass：方向性 motion blur 采样（沿 wind_angle 的屏幕空间拉丝）
  - composite pass：雾气浓度 += wind_strength * 0.3
  - `gbuffers_terrain.vsh`：草/叶顶点 `mc_Entity` 识别，sin 偏移幅度 *= (1 + wind_strength * 4)
- [ ] `bong_demonic`：全局降饱和 + vignette 浓化
- [ ] uniform 契约文档 `docs/iris_uniform_contract.md` 定稿（值范围、语义、更新频率、淡入规则）

### 5.4 Phase D（可选）—— 程序化切 pack

- [ ] 顿悟/天劫瞬间用内部 API 切到强化 pack
- [ ] 仅在 Phase C 不足以表达时才做（更脆弱）

---

## §6 已知风险

- **Iris 升级适配**：每次 Iris 大版本升级需回归测试 Mixin 注入点，可能需要适配代码
- **OptiFine 玩家被排除**：本 plan 只支持 Iris，OptiFine shaders 不支持。基础栈仍可用
- **SEUS PTGI 等第三方 shader pack 兼容**：玩家用别的 pack 时本 plan 的 uniform 会被忽略（不会崩，效果失效）—— 设计上接受
- **shader 编写门槛**：GLSL + Iris pipeline 知识门槛高，可能需要外包或社区协作
- **性能开销**：多个全屏 pass 同时激活在低端机上可能掉帧，需档位选项
- **首次复制 shader pack 用户感知**：玩家可能困惑"为什么 shaderpacks 多了一个文件"，需要清晰的弹窗说明

---

## §7 开放问题

- [ ] 是否提 PR 推动 Iris 官方加 mod-injectable uniform API？长期看可省 Mixin 维护成本
- [ ] shader pack 自动复制是否需要"首次启动同意"对话？还是静默复制？
- [ ] uniform 更新频率：每 tick（20Hz）还是每帧（60-144Hz）？前者带宽小但视觉卡顿
- [ ] 第三方 shader pack 作者支持：是否提供"Bong uniform 兼容 SDK"（一份 .glsl include 文件）？
- [ ] 提供"光影画质档位"UI（关 / 低 / 中 / 高）？低档关掉部分 uniform / pass
- [ ] Phase D 切 pack 的内部 API 风险评估：是否值得做？

---

## §8 参考资料

**调研报告**（2026-04-13 sonnet 调研，见对话历史）：
- Iris API 源码：`net.irisshaders.iris.api.v0`
- Iris GitHub：https://github.com/IrisShaders/Iris
- Iris custom uniforms 文档：https://shaders.properties/current/reference/shadersproperties/custom_uniforms/
- shaderLABS 管线 wiki：https://shaderlabs.org/wiki/Rendering_Pipeline_(OptiFine,_ShadersMod)
- Iris 官方不兼容列表：https://github.com/IrisShaders/Iris/blob/26.1/docs/usage/incompatible-mods.md

**关键 issue / 前例**：
- Iris #2499（SEUS PTGI 粒子隐形）
- Ars Nouveau #1976（skyweave 与光影冲突）
- Botania（Iris 官方不兼容典型，因自写 GlProgram）
- Puzzle #6（Iris API 正确使用讨论）
- Modrinth: Dimension Based Shader Switch（程序化切 pack 前例）

**前置 plan**：
- `plan-vfx-v1.md`（VFX 基础栈，必须先完成）
- `plan-particle-system-v1.md`
- `plan-player-animation-v1.md`

---

## §9 进度日志

- 2026-04-25：审计 client/ 实际代码 —— `fabric.mod.json` 无 `recommends.iris` 声明，`client/src` 无 `IrisApi`/`CommonUniforms` Mixin / `BongClientState` / `bong_xianxia` shader pack 资源，全 plan 仍处 §1 调研结论阶段，§2–§5 任务全部未启动，所有 `[ ]` 维持原状。Phase A 技术验证尚未开跑；启动需待 `plan-vfx-v1.md` Phase 1 完成后再决定。
- 2026-05-17：重构实施节点——新增 Phase 0（接入管线 + 测试平台），将原 Phase A 改为"首个效果端到端"，新增风效果 uniform（`bong_wind_strength` / `bong_wind_angle`），Phase B/C 按效果复杂度分组。优先级：先把调试工具链和数据通路跑通，后续效果开发变成纯 GLSL 迭代。
