# 检视界面 (Inspect Screen)

塔科夫风格的角色检视 UI，按 `I` 键打开。左侧为角色状态，右侧为多格占位背包网格，支持拖拽、Shift 快捷操作、多容器切换。

```
┌─────────────────────────────────────────────────┐
│ [装备] [修仙]              [主背包][小口袋][前挂包]│
│                                                 │
│  ┌──────────┐  ┌─────────────────────────┐      │
│  │  装备槽   │  │                         │      │
│  │          │  │     背包网格 (7×5)        │      │
│  │ HEAD     │  │     支持多格物品          │      │
│  │ OFF MAIN │  │     (1×1 / 2×1 /         │      │
│  │ CHEST 2H │  │      1×2 / 2×2)          │      │
│  │ LEGS FT  │  │                         │      │
│  │          │  ├─────────────────────────┤      │
│  ├──────────┤  │ 物品详情 tooltip         │      │
│  │ 境界/真元 │  └─────────────────────────┘      │
│  │ 体魄条   │                                   │
│  └──────────┘                                   │
│                                                 │
│  1 2 3 4 5 6 7 8 9         重量: 12.3/50.0     │
│  └── 快捷栏 ──┘                                  │
└─────────────────────────────────────────────────┘
```

---

## 文件结构

```
inventory/
├── InspectScreen.java            # 主屏幕，管理所有交互和渲染
├── InspectScreenBootstrap.java   # I 键注册 + 屏幕工厂
├── component/
│   ├── BackpackGridPanel.java    # 多格占位背包网格 (默认 5×7)
│   ├── GridSlotComponent.java    # 单格槽位 (28×28px，棋盘格背景)
│   ├── EquipmentPanel.java       # 装备槽布局 (十字排列 7 槽)
│   ├── EquipSlotComponent.java   # 单个装备槽
│   ├── BodyInspectComponent.java # 双层人体检视 (体表 + 经脉)
│   ├── ItemTooltipPanel.java     # 物品详情浮窗
│   ├── StatusBarsPanel.java      # 境界/真元/体魄进度条
│   └── BottomInfoBar.java        # 底栏：重量
├── model/
│   ├── InventoryModel.java       # 背包完整快照 (Builder 模式)
│   ├── InventoryItem.java        # 物品定义 (id, 尺寸, 重量, 稀有度)
│   ├── EquipSlotType.java        # 装备槽枚举 (7 种)
│   ├── PhysicalBody.java         # 肉体状态快照 (16 部位)
│   ├── BodyPart.java             # 身体部位枚举 (16 个)
│   ├── BodyPartState.java        # 部位状态 (伤势/出血/愈合/夹板)
│   ├── WoundLevel.java           # 伤势等级 (完好→断肢, 6 级)
│   ├── MeridianBody.java         # 经脉状态快照 (20 经 + 3 丹田)
│   ├── MeridianChannel.java      # 经脉枚举 (12 正经 + 8 奇经)
│   ├── ChannelState.java         # 经脉状态 (容量/流量/损伤/污染)
│   ├── MockInventoryData.java    # 测试数据：背包物品
│   ├── MockMeridianData.java     # 测试数据：经脉状态
│   └── MockPhysicalData.java     # 测试数据：肉体伤势
└── state/
    ├── InventoryStateStore.java  # 背包状态 volatile 存储
    ├── MeridianStateStore.java   # 经脉状态 volatile 存储
    ├── PhysicalBodyStore.java    # 肉体状态 volatile 存储
    └── DragState.java            # 拖拽状态机 (IDLE/DRAGGING)
```

---

## 核心功能

### 拖拽系统

所有区域之间的物品移动通过统一拖拽状态机完成：

```
IDLE ──[左键物品]──> DRAGGING ──[松开有效目标]──> IDLE (移动成功)
                                ├─[松开无效区域]──> IDLE (返回原位)
                                ├─[ESC/右键]──────> IDLE (取消)
                                └─[松开已占位置]──> 交换
```

**来源/目标：** 背包格子、装备槽、快捷栏、经脉、身体部位、丢弃区

**快捷操作：** Shift+左键在背包↔装备槽之间快捷移动

### 多格物品

背包网格支持 1×1 到 4×4 的物品占位。使用锚点系统——多格物品仅在左上角格子存储引用，`boolean[][] occupied` 追踪碰撞。

### 多容器

右侧支持多个容器 tab 切换（主背包 5×7、小口袋 3×3、前挂包 3×4），每个容器独立管理网格和物品。

### 装备限制

装备槽会检查肉体状态：
- 手臂断肢 → 对应手不能装备武器（`PhysicalBody.canUseHand()`）
- 装备槽拖放时显示红色高亮提示不可用

---

## 双层人体检视

修仙 tab 下的 `BodyInspectComponent` 提供体表/经脉两层可切换视图：

### 体表层 (Physical)

像素画人体轮廓，16 个可交互部位，每个部位根据伤势着色：

| 等级 | 颜色 | 功能比 | 说明 |
|------|------|--------|------|
| 完好 INTACT | 暗绿 | 100% | 正常 |
| 淤伤 BRUISE | 黄绿 | 85% | 轻微 |
| 擦伤 ABRASION | 黄色 | 70% | 表面伤 |
| 割裂 LACERATION | 橙色 | 45% | 开放伤口 |
| 骨折 FRACTURE | 红色 | 15% | 致残 |
| 断肢 SEVERED | 灰色 | 0% | 永久失能 |

- 出血部位有脉冲红色叠加动画
- 断肢部位绘制为半透明灰色
- 底部状态文字提示行动不便/出血警告
- 可将治疗物品拖到受伤部位上使用

### 经脉层 (Meridian)

12 条经脉 + 3 个丹田的可视化：
- 经脉按损伤等级着色（完好/微裂/撕裂/断裂）
- 丹田根据真元量显示发光效果
- 可将丹药/灵物拖到经脉上使用（如凝脉散→受损经脉）
- 悬浮显示经脉详细状态（容量/流量/污染度/修复进度）

---

## 状态管理

三个全局 Store 采用相同模式：`volatile` 快照 + `CopyOnWriteArrayList<Consumer<T>>` 监听器。

```java
// 服务端网络层推送新数据
InventoryStateStore.replace(newModel);

// UI 层订阅变化
InventoryStateStore.addListener(model -> refreshUI(model));

// 读取当前快照
InventoryModel current = InventoryStateStore.snapshot();
```

Store 仅做存储和通知，不含业务逻辑。当前使用 Mock 数据，后续接入服务端网络协议后由 packet handler 调用 `replace()`。

---

## 物品纹理

20 张 128×128 PNG 图标位于 `textures/gui/items/`，文件名与 `InventoryItem.itemId` 一一对应：

```
textures/gui/items/{itemId}.png
```

在格子中缩放到 28×28 显示，应用到身体/经脉上时缩放到 14×14。

---

## 开发调试

```bash
cd client && ./gradlew runClient   # 启动后按 I 键打开检视界面
```

无服务端数据时自动 fallback 到 `Mock*Data.create()` 生成测试数据，可直接看到完整 UI 效果。
