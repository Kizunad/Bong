# Bong · plan-input-binding-v1 · 骨架

通用交互键体系。**G 键作为统一交互入口**——TSY 容器搜刮、item 摄取、未来其他容器/NPC 对话/资源点交互都绑定到此键,不让玩家记 5 个键。

**世界观锚点**：(纯工程,无世界观锚点)

**交叉引用**：`plan-tsy-container-v1` ✅(搜刮) · `plan-inventory-v1` ⏳(摄取) · `plan-gameplay-journey-v1` §G/§Q.1 Wave 0/O.5（决策来源）

---

## 接入面 Checklist

- **进料**：client 当前的零散键位（E 交互、F 快捷使用、各 UI 自定义键）
- **出料**：统一的 `InteractKeyRouter` 调度器 + 优先级表 + 现有 E 键迁移
- **共享类型**：`InteractIntent` enum(`PickupItem | SearchContainer | TalkNpc | ActivateShrine | HarvestResource | ...`)
- **跨仓库契约**：client 新增 `network/InteractRequest` + server `network/handlers/interact.rs` 路由
- **worldview 锚点**：(无)

---

## §0 设计轴心

- [ ] **G 是入口**——未来所有"靠近物体按一下"交互全走这里
- [ ] **优先级路由**：玩家瞄准物体 → 路由器决定调用哪个 handler(容器 > NPC > 拾取 > 默认)
- [ ] **不与 F 冲突**：F 是快捷使用栏(主动技能),G 是环境交互(被动响应)
- [ ] **可扩展**：新增交互类型只需注册 IntentHandler,不动核心路由逻辑

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | InteractKeyRouter 框架 + IntentHandler trait + 现有 E 键迁移 | 旧 E 键调用都走新路由器,行为不变 |
| **P1** ⬜ | TSY 容器搜刮接入 G(plan-tsy-container-v1 路径切换) | 容器搜刮可通过 G 触发 |
| **P2** ⬜ | item 摄取(地面物品/资源点)接入 G | 拾取走 G,inventory 移动仍 LMB |
| **P3** ⬜ | NPC 对话/灵龛激活/灵眼标记/未来扩展点接入 | 多 handler 优先级冲突测试 |

---

## §2 优先级路由表（设计草案）

```
玩家面前 5 格内 raycast 命中:
  TSY/普通容器        → SearchContainer (优先级 100)
  NPC(可对话)         → TalkNpc         (优先级 90)
  灵龛                → ActivateShrine  (优先级 80)
  地面物品            → PickupItem      (优先级 70)
  资源点(灵草/矿物)   → HarvestResource (优先级 60)
  其他/无             → null + 客户端反馈"什么也没有"
```

**冲突解决**：raycast 一次只命中一个目标实体;若多个重叠,按优先级最高的处理。

---

## §3 数据契约

- [ ] `client/.../input/InteractKeyRouter.java` 核心调度器
- [ ] `client/.../input/InteractIntent.java` 意图 enum
- [ ] `client/.../input/IntentHandler.java` 注册接口
- [ ] `client/.../input/InteractPriorityResolver.java` 优先级解析
- [ ] `agent/packages/schema/src/client-request.ts` 增 `InteractRequest` payload
- [ ] `server/src/network/handlers/interact.rs` 服务端路由
- [ ] `client/.../config/Keybindings.java` G 键默认绑定 + 用户可改

---

## §4 开放问题

- [ ] 长按 G 是否触发不同 handler(如长按对话,短按拾取)?
- [ ] G 键被玩家覆盖怎么处理(是否禁止)?
- [ ] 组合键预留(Shift+G / Ctrl+G) 给 future 用法?
- [ ] 多个目标重叠时是否给 UI 选择(避免错按)?

## §5 进度日志

- 2026-05-01：骨架创建。plan-gameplay-journey-v1 §Q Wave 0 派生 / O.5 决策落点。
