# Bong · plan-vfx-v1 · 模板

**视觉特效基础栈**。定义 Bong 客户端**与光影 mod 无关**的视觉表现层：粒子、实体渲染、BlockEntity 渲染、HUD 叠色、屏幕抖动等。**本 plan 不依赖 Iris**，所有效果在原版/Iris/OptiFine 下都能跑。

光影增强（shader pack、状态驱动 uniform）作为可选上层，独立于本 plan，见 `plan-iris-integration-v1.md`。

**交叉引用**：
- `plan-particle-system-v1.md`（粒子/实体/BE 子系统详细设计）
- `plan-player-animation-v1.md`（玩家动画子系统）
- `plan-iris-integration-v1.md`（**可选上层**：基于本 plan 的光影集成）
- `../plan-HUD-v1.md` · `plan-tribulation-v1.md` · `plan-narrative-v1.md`

---

## §0 设计轴心

- [ ] **零光影依赖**：本 plan 范围内所有效果不需要装 Iris/OptiFine 也能跑
- [ ] 修仙沙盒的视觉核心：**世界内 VFX**（剑气/飞剑/符阵）> 屏幕滤镜
- [ ] 短暂事件（顿悟/破境/天劫）可用强表演；常态不干扰视野
- [ ] **走标准 vanilla 渲染 API**：粒子 / EntityRenderer / BlockEntityRenderer / DrawContext，避开 GLSL 自写 program
- [ ] 装光影的玩家**自动享受加成**（飞剑被打阴影、剑气被加辉光），但**不依赖**

---

## §1 现状快照（2026-04-13）

### 1.1 已实现（`client/src/main/java/com/bong/client/visual/`）

- `VisualEffectState`：3 种屏幕级特效 —— `SCREEN_SHAKE`（8px / 75ms）、`FOG_TINT`（雾色染色）、`TITLE_FLASH`（标题闪烁）
- `VisualEffectController`：强度/时长上限、重触发窗口
- `VisualEffectProfile`：3 档预设（天道警示 / 顿悟 / 时代法旨）
- HUD 编排 `BongHudOrchestrator` 已分三层：Zone / Toast / VisualEffect

### 1.2 完全空白

- 粒子系统（零自定义 `ParticleType`）—— 见 `plan-particle-system-v1.md`
- 自定义实体渲染器（飞剑、法宝、符箓）—— 同上
- BlockEntity 渲染器（地面符阵、结界）—— 同上
- 玩家骨骼动画 —— 见 `plan-player-animation-v1.md`
- 屏幕级 shader / Mixin 渲染类 —— 不在本 plan 范围（见 `plan-iris-integration-v1.md`）

---

## §2 技术分层（本 plan 覆盖范围）

| 层 | 例子 | 实现路径 | 本 plan 处理 |
|----|------|---------|-------------|
| **A2** HUD 层叠色 | 血月、顿悟白屏、染血、入魔 | `DrawContext` + 半透明 quad | ✅ 包含（§3） |
| **B1** 自定义粒子 | 剑气、真气、符文 | `ParticleType` + 自定义 `buildGeometry` | ✅ 委派 particle-system |
| **B2** 自定义实体 | 飞剑、法宝 | `EntityRenderer` + `getEntityTranslucentEmissive` | ✅ 委派 particle-system |
| **B3** BlockEntity 渲染 | 符阵、结界 | `BlockEntityRenderer` + 标准 `RenderLayer` | ✅ 委派 particle-system |
| **骨骼动画** | 挥剑、打坐 | PlayerAnimator | ✅ 委派 player-animation |
| **镜头/FOV** | 抖屏、运功收 FOV、破境拉伸 | 改 camera 参数 | ✅ 包含（§4） |

**不在本 plan 范围**（光影上层，见 `plan-iris-integration-v1.md`）：
- 全屏 GLSL shader（水墨、灵压扭曲、景深）
- Iris shader pack 集成
- Mixin 注入 `CommonUniforms` 喂自定义 uniform
- 程序化切换 shader pack

---

## §3 HUD 叠色子系统（A2）

### 3.1 机制

`DrawContext` 在 HUD 渲染阶段画全屏半透明 quad / 边缘装饰贴图。**HUD 阶段在 vanilla 与 Iris 渲染管线之后**，光影完全无感知。

### 3.2 首批效果

| 效果 | 触发 | 实现 |
|------|------|------|
| 血月 | 夜间 + 血月事件 | 全屏红色叠层 alpha 0.2 |
| 入魔黑雾 | 玩家心魔状态 | 边缘深红/黑 vignette |
| 入定淡青 | 运功状态 | 全屏淡青 + 边缘渐暗 |
| 顿悟金光白屏 | 顿悟事件瞬闪 | 短暂全白 + 淡出 |
| 天劫压抑 | 天劫接近 | 全屏灰蓝 + vignette |
| 中毒酸绿 | 中毒 debuff | 全屏淡绿叠层 |
| 寒毒冰蓝 | 寒毒 debuff | 全屏冰蓝 + 边缘结霜贴图 |
| 濒死视界收缩 | HP < 10% | 黑色 vignette 收紧 |
| 水墨边框 | 入定/回忆 | 四角墨晕贴图（中心透明） |

### 3.3 复用现有抽象

- 沿用 `VisualEffectController`（强度/时长上限、重触发窗口）
- 沿用 `VisualEffectProfile` 预设机制
- 新增 `OverlayQuadRenderer` 处理全屏半透明 quad
- 新增 `EdgeDecalRenderer` 处理边缘装饰贴图

---

## §4 镜头 / FOV 反馈

改的是 camera / FOV 参数，**不碰任何 shader**：

- [ ] `SCREEN_SHAKE`（已实现，扩展强度档位）
- [ ] 运功 FOV 收缩（专注感）
- [ ] 破境 FOV 拉伸 + 短暂俯仰
- [ ] 受创镜头后退
- [ ] 灵压靠近时镜头细微晃动
- [ ] 天劫降临时镜头自动仰视

---

## §5 子系统委派

本 plan 是**总纲**，具体实现委派给三个子 plan：

| 子系统 | 子 plan | 范围 |
|--------|---------|------|
| 粒子 / 实体 / BlockEntity | `plan-particle-system-v1.md` | 渲染基类、Server↔Client VFX 协议、首批资产 |
| 玩家骨骼动画 | `plan-player-animation-v1.md` | PlayerAnimator 集成、动画注册表、首批动画 |
| 屏幕级 shader / Iris 集成 | `plan-iris-integration-v1.md`（**可选上层**） | shader pack、Mixin uniforms、状态驱动 |

---

## §6 与光影 mod 的兼容性（不依赖，但兼容）

本 plan 范围内所有效果**对 Iris/Oculus/OptiFine 零冲突**：

| 效果类型 | 玩家无光影 | 玩家装 Iris | 玩家装 OptiFine |
|---------|-----------|------------|----------------|
| 粒子 | ✅ 正常 | ✅ 正常（自动 shading）* | ✅ 正常 |
| 自定义实体 | ✅ 正常 | ✅ 正常（被打阴影） | ✅ 正常 |
| BlockEntity | ✅ 正常 | ✅ 正常 | ✅ 正常 |
| 玩家动画 | ✅ 正常 | ✅ 正常 | ✅ 正常 |
| HUD 叠色 | ✅ 正常 | ✅ 正常（HUD 在光影之后） | ✅ 正常 |
| 镜头/FOV | ✅ 正常 | ✅ 正常 | ✅ 正常 |

*例外：SEUS PTGI 类 path-tracing shader 下自定义粒子可能隐形（Iris #2499），需 README 声明推荐 shader 列表。

---

## §7 已知边界

- **画面表现天花板**：本 plan 只能做到"vanilla 渲染 API 能做的"。真水墨滤镜、屏幕级景深、灵压扭曲等需要 GLSL 全屏 shader 的效果**不在本 plan 范围**，归 `plan-iris-integration-v1.md`
- **不依赖光影 = 不能做光影级效果**：这是设计取舍，不是 bug
- **首批 70% 仙侠观感即可**：粒子 + 实体 + 动画 + HUD 叠色 + 镜头反馈 已经足够"修仙"，光影是锦上添花

---

## §8 实施节点

- [ ] §3 OverlayQuadRenderer / EdgeDecalRenderer 抽象
- [ ] §3.2 首批 HUD 叠色（血月、入魔、顿悟、天劫至少 4 个）
- [ ] §4 镜头 FOV 反馈（运功收 FOV、破境拉伸）
- [ ] 子 plan 委派验收：particle-system / player-animation 各自达成 Phase 1
- [ ] 整体集成 demo：服务端事件 → 粒子 + 动画 + HUD 叠色 + 抖屏同时触发

---

## §9 开放问题

- [ ] HUD 叠色与现有 `VisualEffectController` 的整合层级（`OverlayQuadRenderer` 是 controller 的子模块还是平级）？
- [ ] 镜头 FOV 改动是否需要 Mixin？vanilla `Camera` / `GameRenderer` API 是否够用？
- [ ] 美术资源（HUD 叠色贴图、边缘装饰贴图）的来源？
- [ ] 是否为 `plan-iris-integration-v1.md` 的"光影增强档位"预留切换 UI？

---

## §10 参考

**子 plan**：
- `plan-particle-system-v1.md`（粒子 / 实体 / BE）
- `plan-player-animation-v1.md`（玩家动画）
- `plan-iris-integration-v1.md`（光影增强，可选上层）

**Vanilla 渲染 API**：
- Fabric Particle API：https://fabricmc.net/wiki/tutorial:particles
- `RenderLayer` 文档（MC 1.20.1）

**Iris 兼容性背景**（决定为何把 Iris 拆出去）：
- 见 `plan-iris-integration-v1.md` §0 调研结论
