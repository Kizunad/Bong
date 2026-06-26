# plan-inventory-hint-panel-v1 — inventory tab hover 通用警示面板（骨架）

> 背包/装备界面里，很多操作被 server 静默拒绝（拖入弹回），玩家不知道**为什么**。配一个**通用警示/提示面板**：hover 到 inventory tab（或装备槽 / 操作失败时）弹出上下文化警示文案，把 server 的拒绝原因（境界不足 / 容器非空 / worn cap 满 / 双手锁定 等）显式告诉玩家。源于真机：拖伪皮进胸槽弹回(`realm too low for SpiderSilk`)、挪非空背包弹回(`container not empty`) 全是静默回弹，体验差。

> **状态**：骨架草案，后续人工细化 + 走 consume。

## 接入面（立 plan 时需坐实）
- **进料**：server `validate_move_semantics` / `apply_inventory_move` 的拒绝原因（现为 Err String + WARN 日志，未下发 client）；client tab/槽位 hover 事件（owo `mouseEnter`）。
- **出料**：client 浮层面板渲染（owo overlay / tooltip 风格）；可能新增一条 S2C「操作拒绝原因」payload 或复用 inventory_snapshot 的 rejection reason 字段。
- **共享类型**：复用现有 owo tooltip / BongToast 风格；拒绝原因枚举跨端（server reject reason → wire → client 文案映射）。
- **跨仓库契约**：server reject reason 枚举（schema）↔ client 文案表；hover 纯 client。
- **worldview 锚点**：UI 辅助，无玩法正典依赖（参 [[feedback_hud_immersive_minimal]] / [[feedback_hud_conditional]]：按需显示、不常驻）。

## 大致阶段
- **P0**：server 拒绝原因结构化 —— `validate_move_semantics` 的 Err 改成带 reason code 的枚举；`apply_inventory_move` 拒绝时把 reason 随 rejection snapshot 下发（新增 wire 字段或独立 payload）。
- **P1**：client 接收 reject reason → 失败操作时弹通用警示面板（飘红 toast 或槽位旁浮层），文案表覆盖：境界不足/容器非空/worn cap 满/手持互斥/双手锁定/分类不符。
- **P2**：tab hover 上下文提示 —— hover inventory tab / 装备槽时，主动显示该容器/槽的约束（cap、可装类别、当前境界能否装），不必等失败才提示。
- **P3**：视听规格（面板位置/颜色 hex/淡入淡出 tick/不遮挡拖拽 ghost；避 owo `Sizing.fill(100)` 顶飞，见 [[feedback_owo_fill_overflow]]）。

## 开放问题（consume 前收口）
1. 警示走「失败后 toast」还是「hover 预警」还是两者都要？（P1 vs P2 取舍）
2. reject reason 是新 S2C payload 还是塞进 inventory_snapshot？
3. 文案与 worldview 语感对齐（境界名用正典：醒灵/引气/…，见 [[feedback_worldview_canonical]]）。
