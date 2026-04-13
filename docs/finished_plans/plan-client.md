# Client 路线详细计划（Java / Fabric 1.20.1）

> 从 CustomPayload 接收器推进到修仙沙盒的沉浸式视觉表现层。
> 纯 Client-Side Mod，不修改注册表，保持极致轻量。

---

## 当前代码结构（实际）

```
client/src/main/java/com/bong/client/
├── BongClient.java             [✓] Mod 入口 (K 键绑定 + HUD)
├── BongClientFeatures.java     [✓] Feature flags (toasts/visual/xml/debug)
├── BongHud.java                [✓] 总 HUD 渲染入口
├── BongNetworkHandler.java     [✓] CustomPayload 注册 + 调用 ServerDataRouter
├── network/
│   ├── ServerDataRouter.java   [✓] 统一消息分发路由器
│   ├── ServerDataEnvelope.java [✓] JSON 解析 + type 提取
│   ├── ServerDataHandler.java  [✓] Handler 接口
│   ├── ServerDataDispatch.java [✓] 路由结果（含 chat/toast/ui_open/visual_effect）
│   ├── NarrationHandler.java   [✓] 天道叙事解析 + 格式化
│   ├── ZoneInfoHandler.java    [✓] 区域信息处理
│   ├── EventAlertHandler.java  [✓] 事件警报（severity 三级 + visual effect hint）
│   ├── PlayerStateHandler.java [✓] 玩家状态同步
│   ├── UiOpenHandler.java      [✓] 动态 UI 打开（template + guarded XML）
│   └── LegacyMessageServerDataHandler.java [✓] welcome/heartbeat 兼容
├── state/
│   ├── NarrationState.java     [✓] Narration 状态模型（scope/style/toast eligibility）
│   ├── ZoneState.java          [✓] Zone 状态模型
│   ├── VisualEffectState.java  [✓] 视觉效果状态（SCREEN_SHAKE/FOG_TINT/TITLE_FLASH）
│   ├── PlayerStateStore.java   [✓] 玩家状态缓存
│   ├── PlayerStateViewModel.java [✓] 面板展示用 ViewModel
│   └── UiOpenState.java        [✓] UI 打开状态（template 或 dynamic XML）
├── hud/
│   ├── BongHudOrchestrator.java [✓] HUD 编排器（统一管理各层渲染）
│   ├── BongHudStateStore.java   [✓] HUD 状态管理
│   ├── BongHudStateSnapshot.java [✓] HUD 快照（线程安全）
│   ├── BongToast.java           [✓] 居中 Toast 提示
│   ├── BongZoneHud.java         [✓] 区域名 + 灵气条 + 危险等级
│   ├── ToastHudRenderer.java    [✓] Toast 渲染层
│   ├── ZoneHudRenderer.java     [✓] Zone HUD 渲染层
│   ├── VisualHudRenderer.java   [✓] 视觉效果渲染层
│   ├── HudRenderCommand.java    [✓] 渲染指令（数据驱动）
│   ├── HudRenderLayer.java      [✓] 渲染层级枚举
│   └── HudTextHelper.java       [✓] 文本裁剪 + alpha 工具
├── visual/
│   ├── VisualEffectController.java [✓] 效果接受/覆盖/重触发窗口
│   ├── VisualEffectPlanner.java    [✓] 效果计划调度
│   └── VisualEffectProfile.java    [✓] 三种效果配置（SCREEN_SHAKE/FOG_TINT/TITLE_FLASH）
└── ui/
    ├── CultivationScreen.java      [✓] owo-ui 修仙面板（K 键打开）
    ├── CultivationScreenBootstrap.java [✓] 面板启动胶水
    ├── DynamicXmlScreen.java       [✓] 动态 XML UI 渲染
    └── UiOpenScreens.java          [✓] 模板注册表
```

**测试**：21 个测试类 | 103 个 test case 全部通过
```

---

## M1 — 天道闭环 [✓]

### C1. Narration 频道监听与渲染 [✓]

**目标**：接收 server 转发的天道叙事，按风格分类渲染到聊天栏。

**实现状态**：✅ **完成**（比计划更完善的架构）

**实现细节**：

- ✅ `NarrationHandler` 接收 `ServerDataEnvelope`（type = "narration"），解析 `narrations[]` 数组
- ✅ 每条 narration 解析为 `NarrationState`（scope/target/text/style）
- ✅ 按 style 用 **MC Formatting API**（非原始颜色码）渲染带前缀聊天文本：
  - `SYSTEM_WARNING` → 红色粗体 `[天道警示]`
  - `PERCEPTION` → 灰色 `[感知]`
  - `NARRATION` → 白色 `[叙事]`
  - `ERA_DECREE` → 金色粗体 `[时代法旨]`
- ✅ 产出 `ServerDataDispatch`（含 chatMessages + toast trigger + narrationState）
- ✅ 支持多条 narration 批量处理，跳过无效条目并计数

---

### C2. Narration HUD Toast [✓]

**目标**：重要 narration 在屏幕中央弹出醒目提示。

**实现状态**：✅ **完成**（比计划更完善）

**实现细节**：

- ✅ `BongToast` — 线程安全的 volatile 单例 toast 管理
- ✅ 通过 `NarrationState.isToastEligible()` 判断是否触发 toast
  - `SYSTEM_WARNING` → 红色粗体 "天道警示：" + 内容
  - `ERA_DECREE` → 金色粗体 "时代法旨：" + 内容
  - `PERCEPTION` / `NARRATION` → 不触发 toast
- ✅ 渲染：半透明背景 + 居中文字 + 文本宽度裁剪
- ✅ feature flag 控制：`BongClientFeatures.ENABLE_TOASTS`
- ✅ toast 也可由 `EventAlertHandler` 独立触发（按 severity 着色）

---

### C3. 天象视觉反馈 [✓]

**目标**：天劫等事件时有视觉暗示。

**实现状态**：✅ **完成**（无 Mixin，纯数据驱动）

**实现细节**：

- ✅ `VisualEffectState` — 效果状态模型（effectType/intensity/duration/startedAt）
- ✅ `VisualEffectProfile` — 三种效果配置：
  - `SCREEN_SHAKE` (天道警示) → 橙色 0xF07C3E, maxIntensity 0.85, 最长 2.4s, 重触发窗口 1.2s
  - `FOG_TINT` (灵气感知) → 蓝灰色 0x5F7693, maxIntensity 0.55, 最长 4.5s, 重触发窗口 1.5s
  - `TITLE_FLASH` (时代法旨) → 金色 0xF2CC6B, maxIntensity 0.75, 最长 3.2s, 重触发窗口 2.2s
- ✅ `VisualEffectController` — 效果接受/覆盖逻辑 + retrigger window 防抖
- ✅ `VisualEffectPlanner` — 效果计划调度
- ✅ `VisualHudRenderer` — HUD 渲染层
- ✅ feature flag 控制：`BongClientFeatures.ENABLE_VISUAL_EFFECTS`
- ✅ `EventAlertHandler` 可携带 `effect` hint（JSON 对象/字符串）驱动视觉效果

**无 Mixin**：通过 HUD overlay + alpha 混合实现，不侵入原版渲染管线

---

## M2 — 有意义的世界 [✓]

### C4. 区域 HUD [✓]

**目标**：玩家进入不同区域时，屏幕显示区域名和灵气浓度。

**实现状态**：✅ **完成**（带淡入淡出 + 常驻 overlay）

**实现细节**：

- ✅ `BongZoneHud` — 纯静态工具，构建 `HudRenderCommand` 列表
- ✅ 居中大字区域名 "— {zone} —"，金色（0xFFD700），进入后 1.5s 全亮 → 2s 内线性淡出
- ✅ 常驻小字 overlay：`区域{name} 灵气[████████░░] 危☠☠☠`
- ✅ 灵气条 10 段（`█` 填充 + `░` 空余），危险等级 ☠ 重复
- ✅ `ZoneState` 状态模型 — zone name / spirit_qi / danger_level / changedAtMillis
- ✅ `ZoneInfoHandler` 解析 server 下发的 zone_info payload

---

### C5. CustomPayload 路由器 [✓]

**目标**：统一的消息分发框架，便于后续扩展。

**实现状态**：✅ **完成**（远超计划的架构完善度）

**实现细节**：

- ✅ `ServerDataRouter` — 类型安全的路由器（`Map<String, ServerDataHandler>`）
  - 已注册类型：`welcome`, `heartbeat`, `narration`, `zone_info`, `event_alert`, `player_state`, `ui_open`
- ✅ `ServerDataEnvelope` — JSON 解析 + type 提取 + payload 隔离
- ✅ `ServerDataDispatch` — 路由结果封装（含 chatMessages / toastSpec / narrationState / visualEffect / uiOpenState）
- ✅ `RouteResult` — 统一的成功/解析错误/无处理器 三状态
- ✅ 每个 Handler 返回结构化的 `ServerDataDispatch`，由上层统一执行副作用
- ✅ 未知 payload type 安全忽略 + 日志
- ✅ Handler 异常被安全捕获，不会崩溃客户端

**超出计划**：增加了 `event_alert`（M2 事件警报）和 `player_state`（M3 玩家状态同步）handler

---

## M3 — 修仙体验 [✓]

### C6. 修仙 UI 面板（owo-ui）[✓]

**目标**：按键打开修仙面板，显示境界、真元池、karma 等。

**实现状态**：✅ **完成**

**实现细节**：

- ✅ `CultivationScreen` — owo-ui BaseOwoScreen + FlowLayout
- ✅ K 键绑定（`BongClient.registerCultivationKeybinding`）
- ✅ 面板内容（全部从 `PlayerStateViewModel` 数据驱动）：
  - 境界名称
  - 真元条：`████████░░ 78/100`
  - 因果 (karma)：`+0.20`
  - 善恶刻度：`[═══════●════] 善 ←→ 恶`
  - 综合实力 + 四维细分（战斗/财富/社交/领地）
  - 当前区域 + 灵气浓度条 + 百分比
- ✅ 空数据时显示 placeholder："当前尚未同步修仙数据"
- ✅ `PlayerStateViewModel` — 纯粹的展示 ViewModel（从 `PlayerStateHandler` 解析 server payload）
- ✅ `PlayerStateStore` — 玩家状态缓存

---

### C7. 动态 UI 下发 [✓]

**目标**：Server 可以下发 UI 布局 XML，Client 动态渲染。

**实现状态**：✅ **完成**（比计划更安全）

**实现细节**：

- ✅ `UiOpenHandler` — 支持两种模式：
  1. **Template 模式** (`template_id`)：从 `UiOpenScreens` 预注册表查找，安全打开
  2. **Dynamic XML 模式** (`xml`/`xml_layout`)：解析后动态渲染
- ✅ `DynamicXmlScreen` — 使用 owo-ui `UIModel.load()` 渲染
- ✅ feature flag 独立控制：`ENABLE_XML_TEMPLATE_MODE` = true, `ENABLE_DYNAMIC_XML_UI` = false（默认关闭）

**安全措施**（远超计划要求）：
- ✅ XML 大小限制：**512 bytes / 384 characters**（比计划的 10KB 严格得多）
- ✅ XXE 防护：禁用 DOCTYPE/ENTITY/external entities/external DTD
- ✅ `XMLConstants.FEATURE_SECURE_PROCESSING` 启用
- ✅ **白名单组件**：仅允许 `flow-layout` 和 `label`（不允许任何属性）
- ✅ **结构验证**：必须 `<owo-ui><components>` 根结构，单根组件
- ✅ 递归 validateComponentTree 验证每个子节点

---

## 超出计划的新增部分

### EventAlertHandler（计划外，M2 级）

- ✅ 接收 `event_alert` payload（title/message/severity/duration_ms/effect）
- ✅ Severity 三级：INFO (蓝 3.5s) / WARNING (橙 5s) / CRITICAL (红 6.5s)
- ✅ 自动从 `event` 字段派生 title（下划线转首字母大写）
- ✅ 可携带 `effect` hint（字符串或对象），驱动 VisualEffectController
- ✅ 产出 toast + visual effect

### HUD 编排层（计划外）

- ✅ `BongHudOrchestrator` — 统一 HUD 渲染编排（Toast / Zone / Visual 三层）
- ✅ `HudRenderCommand` — 数据驱动渲染指令（text/toast 两种类型）
- ✅ `HudRenderLayer` 枚举控制渲染优先级
- ✅ `HudTextHelper` — 文本裁剪到最大像素宽度 + alpha 计算
- ✅ `BongHudStateStore` / `BongHudStateSnapshot` — 线程安全状态管理

---

## 开发历程总结

```
✅ M1 天道闭环 — 完全实现
   C1 Narration 聊天渲染（MC Formatting API，非原始颜色码）
   C2 Toast 提示（NarrationState 驱动 + EventAlert 驱动）
   C3 天象视觉（SCREEN_SHAKE / FOG_TINT / TITLE_FLASH，纯 HUD overlay）

✅ M2 有意义的世界 — 完全实现
   C4 区域 HUD（居中大字淡出 + 常驻灵气条/危险等级）
   C5 CustomPayload 路由器（ServerDataRouter + 7 种类型 handler）

✅ M3 修仙体验 — 完全实现
   C6 修仙 UI 面板（owo-ui CultivationScreen，K 键打开）
   C7 动态 UI 下发（template + guarded XML 双模式，严格安全校验）

额外完成：
   EventAlertHandler（severity 三级 + visual effect hint）
   HUD 编排层（BongHudOrchestrator + 数据驱动渲染指令）
   Feature flags（BongClientFeatures：toasts / visual / xml / debug）
   完整状态层（state/ 包：Narration / Zone / VisualEffect / PlayerState / UiOpen）
```

**数字**：38 个 Java 源文件 + 21 个测试类 + 103 个 test case 全部通过

---

## 构建与测试

```bash
# 编译
cd client && ./gradlew build

# 开发态测试（WSLg）
sdk use java 17.0.18-amzn
./gradlew runClient
# MC 窗口 → 多人游戏 → localhost:25565

# 单元测试
./gradlew test
```

**手动测试快捷方式**：
- `redis-cli PUBLISH bong:agent_narrate '{"v":1,"narrations":[{"scope":"broadcast","text":"天道测试消息","style":"system_warning"}]}'`
- Server 收到后转发给所有 client
- Client 聊天栏应显示红色 `[天道警示] 天道测试消息`

---

## 依赖清单

| 依赖 | 版本 | 用途 |
|------|------|------|
| Fabric Loader | 0.16.10 | Mod 加载 |
| Fabric API | 0.92.3+1.20.1 | Networking, Rendering |
| owo-lib | 0.11.2+1.20 | UI 框架 |
| Minecraft | 1.20.1 | 基座 |
| Yarn Mappings | 1.20.1+build.10 | 反编译映射 |
