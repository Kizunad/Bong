# plan-bughunt-world-weather-physics-minor-cleanups-v1（骨架）

> **来源 issue**：#1781、#1609。两条都属于天气物理实体生命周期：restore vision 使用过期快照，lightning entity 没有结束清理。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | vision 恢复以当前会话状态为准；闪电实体具备可验证的结束生命周期 | ⬜ |

## §0 摘要

离开雾区时 `restore_vision` 用进入雾区前的旧快照覆盖玩家当前视距，可能回退其他系统已更新的设置。天气闪电只生成 `LightningEntityBundle`，没有明确 despawn/过期系统，长跑会积累实体。修复要保持天气效果和玩家会话边界一致。

## §1 游玩影响

玩家可能被错误恢复到旧视距；长时间运行服务器会积累无用闪电实体，增加查询和网络开销。两条均是 server weather 生命周期问题，不涉及 agent 推演或真元账本。

## §2 复现路径

1. 进入雾区，记录 vision；雾中由其他设置改变 render distance，再离雾，观察旧快照覆盖新值。
2. 周期生成 lightning，等待可见效果结束，查询 ECS 中同一实体仍存在；重复生成观察数量单调增加。

## §3 今天 `origin/main` 的证据

- `server/src/world/weather_physics/vision.rs:32-49,79-93,116-123` 的 restore 路径直接套用旧 snapshot。
- `server/src/world/weather_physics/lightning.rs:37-50,59-109` 只 spawn `LightningEntityBundle`，没有对应 lifetime/despawn system。

## §4 非重复比对

已查 `docs/finished_plans/plan-weather*`、active plan、skeleton 的 `restore_vision`、`LightningEntityBundle`、weather lifecycle；没有覆盖这两个入口的现有骨架。两条共享“天气临时状态必须有当前值和结束边界”，分别写回归验收。

## §5 立项检查记录

- `docs/worldview.md`：查天气、雾堤、雷劫视觉关键词；不增加新的天象规则。
- `docs/finished_plans/`：查 weather/tribulation 计划，确认复用现有视觉状态和 ECS 生命周期。
- active plan：查 `restore_vision`、`LightningEntityBundle`、`weather_physics`，未发现同入口 active 修改。
- skeleton：查 #1781、#1609 和上述函数名，无同主题骨架。
- `docs/plans-skeleton/reminder.md`：已检查，没有天气生命周期条目；不改该文件。

## §6 接入面与跨仓契约

- **Inputs**：server `VisionSnapshot`/当前玩家设置、天气 lightning spawn tick 与 world entity。
- **Outputs**：更新玩家视距状态、在生命周期结束 despawn lightning；不新增 payload。
- **共享类型/事件**：复用 `VisionState`、`LightningEntityBundle`、天气 clock/system；不新增并行缓存。
- **三端契约符号**：server `weather_physics::vision`、`weather_physics::lightning`；agent **无变更**、client **无变更**（现有天气渲染协议不变）。
- **worldview 锚点**：`docs/worldview.md` 天候/雾堤/雷劫表现；无真元转移，不调用 `qi_physics`。

## P0 验收

- restore 不覆盖自 snapshot 之后的合法设置；只恢复仍属于该雾会话的字段。
- 每个 lightning entity 有明确到期或事件驱动 despawn，重复天气 tick 不泄漏。
