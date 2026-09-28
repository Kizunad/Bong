# BugHunt：库存拒绝提示去重表无界增长

> 来源 Issue：#1440。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 为拒绝 reason 去重表建立有界 TTL/容量策略 | ⬜ |

## §0 摘要

InventoryMoveRejectedHandler.lastShownAtMillis 以 server 可控的 reason 字符串为 key，只 put 不 prune；持续发送唯一 reason 会让客户端静态 ConcurrentHashMap 单调增长。

## §1 游玩影响

库存操作本身会被拒绝并提示，但长时间在线或被恶意服务端发送大量唯一 reason 时，客户端常驻内存持续增长；问题是提示限流表的生命周期，不改变 server inventory 权威。

## §2 复现路径

连续收到带唯一 reason 的 inventory_move_rejected payload，观察 lastShownAtMillis.size() 只增不减，即使旧 reason 已不再需要去重。

## §3 今天 origin/main 的根因证据

- client/src/main/java/com/bong/client/network/InventoryMoveRejectedHandler.java:42,82-93 使用 ConcurrentHashMap<String,Long>，生产 recordShown 只有 put；清空方法只供测试。
- server/src/network/inventory_move_rejected_emit.rs 产生现有 inventory_move_rejected server-data，reason 来自拒绝结果；agent schema 的 server-data 镜像保持同一 payload。

## §4 非重复比对与立项检查记录

- worldview：查物品所有权、拒绝反馈和资源边界；只做客户端限流内存治理。
- finished_plans：查 inventory intent、toast 与 client freshness plan；未找到该 map 的容量/TTL 规范。
- active plan：查 docs/plan-*.md 的 InventoryMoveRejectedHandler/lastShownAtMillis；未覆盖 prune。
- skeleton：查 inventory_move_rejected、UNKNOWN_TYPE_LOG_TIMES、lastShownAtMillis；无同主题骨架。
- reminder.md：仓内无 docs/plans-skeleton/reminder.md，无相关条目。

## §5 修复骨架

采用与同类限流表一致的固定容量 + 时间窗裁剪：记录前删除过期项，超容量按最旧时间淘汰；reason 过长或空白统一归类，不能以服务端字符串制造无限 key。保留 clearOnDisconnect/测试清理语义。

## §6 接入面与跨仓契约

- Inputs：inventory_move_rejected payload 的 reason 与当前时间。
- Outputs：本地 toast 及有界去重状态；server inventory snapshot/拒绝语义不变。
- 共享类型或事件：复用 InventoryMoveRejected server-data、InventoryMoveRejectedHandler、既有限流时间窗；不新增 wire 字段。
- server 符号：inventory_move_rejected_emit / inventory mutation rejection；无 server 业务变更。
- agent：无变更；该拒绝 payload 不经过 Redis agent IPC。
- client：InventoryMoveRejectedHandler.lastShownAtMillis 的 prune/容量实现和行为测试。
- worldview / qi_physics：物品拒绝反馈属于库存权威；不涉及真元流。
