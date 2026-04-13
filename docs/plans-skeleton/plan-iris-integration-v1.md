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

### 5.1 Phase A —— 技术可行性验证（必须先做）

- [ ] Mixin 注入 `bong_test_uniform = 1.0`
- [ ] 写最小 shader pack：`final.fsh` 读 `bong_test_uniform`，颜色按值线性偏红
- [ ] 装 Iris + 加载 pack，确认值能传通
- [ ] 验证 Iris 1.6.x → 1.7.x 升级时 Mixin 注入点是否变化

### 5.2 Phase B —— shader pack 起手

- [ ] §3.1 选 1 个效果先做（推荐 `bong_bloodmoon`，最简单）
- [ ] composite1 + final pass 实现
- [ ] 服务端事件 → 客户端 BongClientState → uniform → shader 联动
- [ ] 端到端 demo：触发血月事件，全屏变红

### 5.3 Phase C —— 全量 uniform 接入

- [ ] §3.1 全部 7 个效果实现
- [ ] §4.3 uniform 契约文档定稿
- [ ] 性能测试（多个效果同时激活 FPS 影响）

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
