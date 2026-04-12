# UI 草图索引（plan-combat-v1）

线框级 SVG 草图，表达 **布局意图 / 数据位置 / 交互流程 / 接入现有 owo-lib 基础设施的方式**，不约束视觉风格。
实际像素尺寸、字体、配色由 `plan-client.md` 及 owo-lib 实现阶段决定。

> **对齐原则**：所有草图必须对应到 `client/src/main/java/com/bong/client/ui/` 已有的渲染骨架
> （BaseOwoScreen / HudRenderCallback / Store 模式 / UiOpenState 三模式 / 16 BodyPart × 6 WoundLevel 枚举）。
> 不另起炉灶，草图只定义 **扩展点** 和 **数据接入位**。

## 三层渲染骨架

| 层 | 入口 | 使用场景 | 草图举例 |
|---|---|---|---|
| **A · HUD** | `HudRenderCallback` 注册 | 始终可见的叠加（血条/状态槽/防御环/渡劫广播） | `hud-combat.svg`, `defense-ui.svg`, `tribulation-ui.svg`（顶栏+渡劫者） |
| **B · Screen** | `BaseOwoScreen<FlowLayout>` 硬编码 | 玩家主动打开的模态面板（K/I 等键）或死亡等不可跳过场景 | `inspect-wounds.svg`, `inspect-status.svg`, `death-screens.svg`, `weapon-treasure.svg`（B 部分） |
| **C · Dynamic XML** | `DynamicXmlScreen` + `UIModel.load()` | server 运行时下发布局（阵法预览/观战弹窗等运行时决定的） | `attack-panels.svg`（阵法布置）, `tribulation-ui.svg`（观战询问） |

下发路径：`UiOpenState.template(screenId, templateId)` 走 A/B；`UiOpenState.dynamicXml(screenId, xml)` 走 C；
payload 受 `ServerDataEnvelope.MAX_PAYLOAD_BYTES` 约束，XML 过滤 `<!DOCTYPE>` / `<!ENTITY>`。

## 文件清单

| 文件 | 对应 plan-combat-v1 章节 | 层 | 内容 |
|---|---|---|---|
| [`ui-architecture.svg`](./ui-architecture.svg) | 总览 | 全部 | 三列架构图：Server payload → Client Store/Rendering → 草图落位 |
| [`hud-combat.svg`](./hud-combat.svg) | §12 + §1.5.4 | A | 战斗主 HUD · 真元 / Stamina / HP / 8 状态槽 / 快捷栏 / DerivedAttrs / 法术体积 / 防御预警 |
| [`inspect-wounds.svg`](./inspect-wounds.svg) | §3.4 · §5.6.8 | B | InspectScreen 新增"伤口"tab · 复用 BodyInspectComponent 的人体图 + WoundDetailPanel 扩展 |
| [`inspect-status.svg`](./inspect-status.svg) | §7.8 | B | InspectScreen 新增"状态"tab · 按 kind 分组的 active_effects + 来源染色 + DispelTag |
| [`attack-panels.svg`](./attack-panels.svg) | §3.5 · §5.2 · §5.3 | A + B + C | 法术体积（HUD）+ 暗器制作 ForgeWeaponScreen（硬编码 B）+ 阵法布置（server 下发 C） |
| [`defense-ui.svg`](./defense-ui.svg) | §4 | A | 截脉 200ms 弹反环 / 替尸伪皮层数 / 涡流冷却 / 姿态切换 / 克制矩阵 — 全 HUD 叠加 |
| [`death-screens.svg`](./death-screens.svg) | §8 | B | DeathScreen + TerminateScreen 两个独立硬编码 Screen · 遗念/终焉之言预生成 |
| [`tribulation-ui.svg`](./tribulation-ui.svg) | §10 | A + C | 顶栏红幅广播 + 渡劫者 HUD = A · 观战询问弹窗 = C（server 下发带距离/方向） |
| [`weapon-treasure.svg`](./weapon-treasure.svg) | §6 | A + B | tooltip 扩展现有 ItemTooltipPanel（A）+ TreasureDetailScreen + ForgeWeaponScreen（B） |

## 每张草图 · owo-lib 对齐表

### hud-combat.svg · 沉浸式极简

> ⚠ **常驻白名单仅 4 项**：两层快捷栏 / 三条精简状态条 / 通用事件流。其他全部按需渲染。已明确从常驻 HUD 移除：中上 8 槽状态面板、左下防御面板、常驻 DerivedAttrs 大图标。

| 草图元素 | 常驻? | owo-lib 组件 | 接入 Store | 下发方式 |
|---|---|---|---|---|
| 战斗快捷栏（下层·1-9） | ✓ 常驻 | 沿用 MC 原生 hotbar，叠加 qi_invest 指示 | `PlayerStateStore` | A |
| 快捷使用栏（上层·F1-F9） | ✓ 常驻 | 自定义 9 格 Component + cast bar | `QuickUseSlotStore`（新增）+ `CastStateStore`（新增） | A · 配置入口在 I 键 UI · 触发走 `bong:combat/quickslot_use` |
| 左下角状态小控件 | ✓ 常驻 | 迷你人体剪影（红点叠加=伤口位置/严重度）+ 真元竖条 + 体力竖条（均百分比 0-100%，**不显示裸数字**） | `PhysicalBodyStore`（伤口）+ `CombatStateStore { qi_percent, stamina_percent }` | A |
| 通用事件流（右侧） | ✓ 常驻 | `Containers.verticalFlow()` + `Components.label()` 条目 | `UnifiedEventStore`（新增·合流战斗/修炼/天象/社交/系统） | A · 多 channel 合流 |
| 状态效果 8 槽详情 | ✗ 移除 | — | — | 改走 I 键 InspectScreen · 状态 tab |
| 防御状态面板 | ✗ 移除 | — | — | 用音效 / 屏幕边缘闪烁替代反馈 |
| 法术体积滑块 | △ 条件 | 自定义 SliderComponent | `SpellVolumeState`（新增，本地） | 仅「持法术 AND 按住 R」时渲染 |
| 截脉弹反环 | △ 条件 | Canvas Component（屏幕中心） | `DefenseStanceStore`（新增） | 仅 `DefenseWindow` 200ms 内显示 |
| 伪皮层数 / 涡流冷却 | △ 条件 | 小角标 | `DefenseStanceStore` | 仅当前姿态 AND 已解锁时显示 |
| DerivedAttrs（飞行/虚化/渡劫锁定） | △ 条件 | 屏幕边缘短显 | `CombatStateStore.derived` | 事件发生时短显，非常驻 |
| cast bar（F 键施法进度） | △ 条件 | 快捷使用格下方 5px 条 | `CastStateStore` | 仅施法中渲染 |

### inspect-wounds.svg

| 草图元素 | owo-lib 组件 | 接入 Store | 下发方式 |
|---|---|---|---|
| 人体图（168×236） | 复用现有 `BodyInspectComponent`（`Layer.PHYSICAL` 切片） | `PhysicalBodyStore`（已有，扩展 wound_level 字段） | B · InspectScreen 新增 tab |
| 6 级伤口染色 | 按 `WoundLevel` 枚举着色（INTACT→SEVERED） | 同上 | B |
| WoundDetailPanel | 新增 `Component extends BaseComponent` 右侧面板 | `PhysicalBodyStore.selectedPart` | B |
| 疗愈拖拽 | 复用现有 `DragState` + `physicalApplied` EnumMap | `DragState` + `InventoryStateStore` | B |

### inspect-status.svg

| 草图元素 | owo-lib 组件 | 接入 Store | 下发方式 |
|---|---|---|---|
| 分组头（DoT/控制/减益/增益） | `Components.label()` + 折叠小三角 | `StatusEffectStateStore`（新增） | B · InspectScreen 新增 tab |
| Effect Row（图标 + magnitude + duration） | `Containers.horizontalFlow()` + 小条 | 同上 | B |
| DispelTag 标记 | 彩色 badge `Components.label()` | 同上 | B |
| 来源染色块 | 小色块 Component，色值 = source_tag hash | 同上 | B |
| tooltip | owo-lib 内建 `Component.tooltip()` API | 同上 | B |

### attack-panels.svg

| 子面板 | 层 | owo-lib 组件 | 接入 Store | 下发方式 |
|---|---|---|---|---|
| §3.5 法术体积双滑块 | A | 自定义 SliderComponent × 2 + 球形预览 | `SpellVolumeState` | HUD |
| §5.2 暗器制作 | B | 硬编码 `ForgeWeaponScreen`（复用现有骨架） | `ForgeWeaponCarrier`（新增） | `UiOpenState.template()` |
| §5.3 阵法布置 | C | 运行时 XML · 网格按需生成 | 无 Store（一次性） | `UiOpenState.dynamicXml()` |
| 按键速查浮层 | A | `Components.label()` 半透明 | 本地静态 | HUD |

### defense-ui.svg

> ⚠ **条件渲染规则**：草图同屏画出三流派仅为文档穷举。运行时 `HudRenderCallback` 必须先查 `PlayerIdentityStore.unlocked_styles`（或等价标志），**未解锁的流派一律不渲染**——不灰掉、不占位、直接隐藏。未激活姿态的对应指示器也不显示。

| 草图元素 | owo-lib 组件 | 接入 Store | 渲染条件 | 下发方式 |
|---|---|---|---|---|
| 截脉 200ms 弹反环 | 自定义 Canvas Component（`MatrixStack` 旋转） | `DefenseStanceStore`（新增） | `jiemai` 已解锁 AND `DefenseWindow` 激活中 | A |
| 替尸伪皮层数 | `Components.texture()` × N 堆叠 | 同上 | `tishi` 已解锁 AND `fake_skin.layers > 0` | A |
| 涡流冷却 | 环形进度 Component | 同上 | `jueling` 已解锁 AND（`vortex_active` OR 冷却中） | A |
| 当前姿态（左下） | `Containers.verticalFlow()` + icon | 同上 | 至少一个流派已解锁 | A |
| 克制矩阵说明 | F1/F2/F3 `KeyBinding` 注册 | （键位映射） | — | — |

**同类规则也适用于**：
- `hud-combat.svg` 的法术体积滑块 / DerivedAttrs 特殊图标 → 无对应能力时不显示
- `attack-panels.svg` 的暗器制作 / 阵法布置 → 未学会对应流派时不开放入口
- 状态槽 → 无激活效果时不占位
- 常驻 HUD 只保留：血条 / 真元 / Stamina / 快捷栏 / 当前激活的状态槽

### death-screens.svg

| 场景 | owo-lib 组件 | 接入 Store | 下发方式 |
|---|---|---|---|
| DeathScreen | `BaseOwoScreen<FlowLayout>`（新增硬编码） | `DeathPayloadStore`（新增） | B · `UiOpenState.template("death_screen", "death")` |
| TerminateScreen | 同上，独立 Screen | 同上 | B · `UiOpenState.template("death_screen", "terminate")` |
| 遗念卡片 | `Components.label()` wrapped | server 预生成文本 | — |
| 运数进度 | 自定义 FortuneMeter Component | `PlayerIdentityStore` | — |
| 按钮 | `Components.button()` | 本地回调 → server | — |

### tribulation-ui.svg

| 草图元素 | 层 | owo-lib 组件 | 接入 Store | 下发方式 |
|---|---|---|---|---|
| 顶栏红字广播 | A | `Components.label()` 全宽，HudRenderCallback | `TribulationStateStore`（新增） | A |
| 渡劫者 HUD（真元/雷劫倒计时） | A | 大号 Component 集 | 同上 | A |
| 危险半径可视化 | A | `MatrixStack` 地面圆环 | 同上 | A |
| 观战询问弹窗 | C | server 下发 XML（按距离/方向生成） | 无 Store | C · `UiOpenState.dynamicXml()` |

### weapon-treasure.svg

| 场景 | 层 | owo-lib 组件 | 接入 Store | 下发方式 |
|---|---|---|---|---|
| 普通武器 tooltip | A | 扩展现有 `ItemTooltipPanel`（添加耐久 / 伤害行） | `InventoryStateStore` | 叠加在背包 |
| 法宝展开 tooltip | A | 同上 + 折叠段（bond / qi_pool / abilities / prev_owners） | `TreasureInfoStore`（新增） | 叠加 |
| TreasureDetailScreen | B | `BaseOwoScreen<FlowLayout>` 新增 | 同上 | B · 右键法宝触发 |
| ForgeWeaponScreen（铸造/修复） | B | 独立硬编码 Screen | `ForgeWeaponCarrier` | B |

## 新增 Store 一览

| Store | 草图使用 | 消费的 CustomPayload Channel |
|---|---|---|
| `CombatStateStore` | hud-combat | `bong:combat/state_sync` |
| `StatusEffectStateStore` | hud-combat · inspect-status | `bong:combat/status_sync` |
| `DefenseStanceStore` | defense-ui · hud-combat | `bong:combat/defense_sync` |
| `SpellVolumeState` | hud-combat · attack-panels | 本地（无网络） |
| `DeathPayloadStore` | death-screens | `bong:lifecycle/death` |
| `TribulationStateStore` | tribulation-ui | `bong:event/tribulation` |
| `TreasureInfoStore` | weapon-treasure | `bong:item/treasure_info` |
| `ForgeWeaponCarrier` | attack-panels · weapon-treasure | `bong:craft/forge_state` |
| `QuickUseSlotStore` | hud-combat（快捷使用栏） | `bong:combat/quickslot_config` |
| `CastStateStore` | hud-combat（cast bar） | `bong:combat/cast_sync` |
| `UnifiedEventStore` | hud-combat（事件流） | 合流多 channel：`bong:combat/event` + `bong:cultivation/event` + `bong:world/event` + `bong:social/chat` + `bong:system/notice` |

## 使用方式

- 在 plan-combat-v1.md 对应章节引用：`[见 UI 草图](./svg/xxx.svg)`
- 查看：任意现代浏览器打开 SVG 文件（Chrome / Firefox）
- 修改：直接编辑 XML · 无依赖外部库

## 命名与风格约定

- **viewBox**：全屏类 1920x1080 · 面板类 1600x1000 · 复杂类 1920x1200
- **主色板**：
  - 背景 `#0a0a12` / `#14141f`（面板）
  - 伤害/敌意 `#ff4040` 系
  - 真元/法宝 `#c040ff` / `#40d0d0`
  - 增益/成功 `#40ff80`
  - 提示/警告 `#ffcc40`
  - 说明文字 `#888` / `#aaaa`
- **字号**：标题 22 / 子标题 14-18 / 正文 11-13 / 注脚 10-11

## 覆盖的 14 项 UI（§12 表）

- [x] inspect 伤口层 → `inspect-wounds.svg`（InspectScreen 新 tab · 复用 BodyInspectComponent）
- [x] 真元条 / Stamina 条 / 血条 → `hud-combat.svg`（owo-lib 图形 Component · 非字符）
- [x] 攻击 HUD（快捷栏 + qi_invest） → `hud-combat.svg` + `attack-panels.svg`
- [x] 状态效果 HUD（8 槽） → `hud-combat.svg`（顶部） + `inspect-status.svg`（详情 tab）
- [x] DerivedAttrs HUD（特殊图标） → `hud-combat.svg`（左上）
- [x] 法术体积滑块 → `hud-combat.svg`（右下） + `attack-panels.svg`（独立面板）
- [x] 防御 UI（弹反 / 涡流 / 伪皮） → `defense-ui.svg`
- [x] 暗器制作面板 → `attack-panels.svg`（ForgeWeaponScreen 硬编码）
- [x] 阵法布置 UI → `attack-panels.svg`（server 下发 Dynamic XML）
- [x] 死亡画面 + 终结画面 → `death-screens.svg`
- [x] 全服天劫广播 + 观战镜头 → `tribulation-ui.svg`
- [x] 武器 / 法宝展开 → `weapon-treasure.svg`

## 未出草图的 UI（由其他 plan 负责）

| UI | 负责 plan | 备注 |
|---|---|---|
| 经络层 inspect | 修炼 plan §7 | 已有 client 骨架（`BodyInspectComponent.Layer.MERIDIAN`） |
| 突破闭关 | 修炼 plan | 新 Screen |
| 顿悟选择 | 修炼 plan | 新 Screen |
| 淬炼面板 | 修炼 plan | 新 Screen |
| 背包 | plan-inventory-v1.md（TODO） | 已有 `InventoryStateStore` + Tarkov 式 Screen |
| 食物/生存 | plan-food-v1.md（TODO） | — |
| 搜打撤 | plan-stealth-chase-v1.md（TODO） | — |

## 下一步

草图定稿后，客户端侧按每个 C 阶段（C1-C7）顺序实现：

- C1 → `hud-combat.svg` 中的 Stamina/真元/HP 条 + DerivedAttrs 状态图标
- C2 → `hud-combat.svg` 状态槽 + `inspect-status.svg` + 防御预警
- C3 → `death-screens.svg` 死亡画面
- C4 → `death-screens.svg` 终结画面
- C5 → `attack-panels.svg` 三个子面板 + `defense-ui.svg`
- C6 → `tribulation-ui.svg`
- 并行：`inspect-wounds.svg` 在 §5.6 疗愈落地时接入 / `weapon-treasure.svg` 在 §6 武器实施阶段
