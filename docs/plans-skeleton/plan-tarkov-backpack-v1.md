# plan-tarkov-backpack-v1 — 塔科夫式套包系统（嵌套容器 + 上身渲染）（骨架）

> 背包不只是"装备槽生成一个 tab"，而是**塔科夫式套包**：背包作为物品本身携带其容器数据；**非空背包可整体卸下/移动**（连包带货一起走，不再强制先掏空）；**双击背包打开**其内含物视图；**重量正确上卷**（背包负重 = 包自重 + 内含物重，嵌套递归）；拖入背包的物品**持久化保存**。再加：穿戴的背包**在玩家身上渲染**（破草包配了模型却没上身）。

> **背景**：当前 `plan-layered-equip-v1`（已落地）下，背包是身体槽 worn 层 + 生成容器 tab，但「非空背包不能卸下」(`cannot unequip backpack: container not empty`) 与塔科夫体验冲突；且背包内含物未随包移动、无嵌套、无上身模型。本 plan 在其上做套包升级。

> **状态**：骨架草案，后续人工细化 + 走 consume。取代 layered-equip 的「非空背包拒卸」规则。

## 接入面（立 plan 时需坐实）
- **进料**：`inventory::PlayerInventory.containers` / `ContainerState`（现 flat 容器列表）；`ContainerSpec`（背包件规格，weight_capacity 等）；`ItemInstance`（背包件本身）。
- **出料**：嵌套容器数据随背包件移动；负重 `calculate_current_weight` 改递归上卷；client 容器视图（双击打开）；玩家模型渲染（worn 背包 → 第三人称 cosmetic）。
- **共享类型**：复用 `ContainerState` / `ContainerSpec`，**避免另造**；嵌套关系 = 容器归属某 `instance_id`（背包件）而非全局 flat。
- **跨仓库契约**：server 容器嵌套模型 + wire（容器随背包件下发）；client 容器视图 + 双击交互 + InventorySnapshotHandler 解析嵌套；玩家模型走 [[project_model_linkage_audit]] 的护甲/cosmetic feature renderer 接线（worn 背包件 → 模型）。
- **worldview 锚点**：worldview §五 装备分层穿戴已锚定「容器按形制穿对应部位、自重计入负重」（plan-layered-equip 补的那节）；套包/嵌套是否需补正典一句（背包套背包的合理性）—— consume 前确认。
- **qi_physics 锚点**：纯储物/重量，不涉真元（除非内含真元 carrier 件——其 qi 守恒沿用既有 equipped/容器求和，不新增公式）。

## 大致阶段
- **P0 嵌套容器数据模型**：容器归属改为「挂在某背包件 instance_id 下」（而非 flat `containers` + 固定 id）；移动背包件 = 其容器数据随迁；持久化（嵌套 serde + 迁移现有 flat 容器）。**移除「非空背包拒卸」规则**——卸下背包连货一起进目标位（手上/地面/更大的包）。
- **P1 重量递归上卷**：`calculate_current_weight` 改递归 = Σ(背包件自重 + 其容器内含物重量[递归])；`compute_max_weight` 背包加成不变；嵌套深度上限（防套娃无限）。负重 pin 测试（套 2 层包重量正确）。
- **P2 拖入持久化**：拖物品进背包容器 → 写入该背包件的嵌套容器 → flush 持久化；拖出/跨包移动；容器满/重量超限拒绝（接 [[plan-inventory-hint-panel-v1]] 提示）。
- **P3 client 双击打开容器视图**：双击背包件（在装备槽 worn / 手上 / 地面）→ 打开其内含物网格视图（owo screen / panel）；显示内含物 + 重量；可拖入拖出。
- **P4 背包上身渲染**：worn 背包件 → 玩家第三人称模型 cosmetic（破草包等已配模型）。**先核实模型资产存在**（grep worn_grass_pouch 的 geo/model 配置），按 [[project_model_linkage_audit]] 的 ArmorFeatureRenderer / cosmetic 接线，FPV+TPV 双入口（[[project_skill_av_wiring]]）。
- **P5 视听 + 平衡**：套包容量/重量平衡数值；双击打开动效/音效；拖拽手感。

## 开放问题（consume 前收口）
1. 嵌套深度上限几层？（防无限套娃 + 重量爆炸）
2. "非空背包卸下" 后去哪：手持？掉地面（连货）？还是必须有目标容器位？
3. 背包件本身的重量 vs 内含物重量在 UI 怎么分别显示（包重 / 总重）。
4. 上身渲染：背包穿在哪个身体部位的模型挂点（背/腰/前——对应 ContainerSpec.equip_slot 的身体槽）？破草包模型资产路径核实。
5. 与 plan-layered-equip 已落地的「身体槽 worn 背包」如何衔接（套包是 worn 背包的内含物视图升级，不重造 equipped 模型）。
