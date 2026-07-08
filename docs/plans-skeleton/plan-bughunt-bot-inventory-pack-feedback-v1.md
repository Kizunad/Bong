# plan-bughunt-bot-inventory-pack-feedback-v1（骨架）

> **骨架（草案）**。一句话主题：Bot e2e 覆盖发现 `inventory_move_intent` 的 pack stow 路径缺少稳定可观察回执；当前可稳定观测到 `inventory_snapshot` revision 推进，但 `bong:inventory_pack_stow` VFX / `inventory_event::moved` 不保证到达，导致黑盒 bot 无法做深断言。

## 背景

- 来源：PR #983 `inventory_pack_move_intents` live bot run。
- 失败 run：28792915156 / job 85375924078。
- 现象：stow intent 后已收到 `inventory_snapshot revision=4`，但 10s 内未收到 `inventory_pack_stow` VFX 或 `inventory_event::moved`。
- 本 PR 只做 bot 覆盖，已将场景降级为 revision 水位 + snapshot 状态断言。
- 追加观察：同一 bot 链路尝试脱下非空穿戴背包时，当前 live server 可出现 `connection_lost`，需要单独确认是产品限制、拒绝反馈缺失，还是断连 bug。

## 目标

- 明确 pack stow / unequip / equip 三类 inventory move 的可观察回执契约。
- 让 bot 和真实客户端都能区分：请求被接受、请求被拒绝、请求生效但仅 resync。
- 消除“只能等全量 snapshot 推断成功”的黑盒测试盲区。

## 非目标

- 不在本骨架内定义背包格子布局规则。
- 不改变 `inventory_move_intent` 请求 schema。
- 不要求所有库存移动都强制发 VFX；只要求有稳定机器可读反馈。

## P0 — 现状复盘

- 对照 `server/src/schema/client_request.rs` 的 `inventory_move_intent` variant。
- 梳理 stow 到穿戴背包容器时 server 当前 emit 的事件集合。
- 确认 `inventory_snapshot` revision 推进是否总是发生，是否覆盖拒绝分支。
- 对照 unequip / equip 路径是否已有稳定 VFX 或 `inventory_event`。
- 复现“非空 pack unequip 后断连”并确认 server 日志中的真实失败点。

## P1 — 回执契约

- 成功移动：至少一个稳定 `bong:server_data/inventory_event::moved` 或等价机器可读 payload。
- 拒绝移动：稳定 `inventory_move_rejected`，带 reason、instance_id、from、to。
- 非空背包脱下：若设计不允许，必须走拒绝 payload；若允许，必须保持连接并给出 moved/resync。
- 视觉反馈：VFX 可以作为用户体验增强，但不作为唯一成功回执。
- 全量 resync：`inventory_snapshot` 可作为状态修正，但不应是唯一动作结果信号。

## P2 — Bot 覆盖收紧

- `inventory_pack_move_intents` 先保持 revision 水位 + 状态断言。
- 回执契约落地后，重新加入 stow 的 `inventory_event::moved` 深断言。
- 对 unequip / equip 同步断言 moved event 与 snapshot revision 一致。
- 补 malformed / stale revision 场景，防止 bot 误读旧 snapshot。

## P3 — 验收

- live bot：stow / unequip / equip 三步均能观测到稳定机器回执。
- live bot：缺少 VFX 不会导致协议级测试误判。
- 单测：纯 Python server_data decoder 能解析新增/既有库存回执。
- 文档：把 bot 场景断言从“临时降级”改回“协议契约断言”。

## 开放问题

1. stow 到穿戴背包容器是否应与普通 container move 共用 `inventory_event::moved`。
2. VFX 的 channel 是否需要携带 instance_id，便于 bot 与客户端做动作归因。
3. `inventory_snapshot` revision 是否应在拒绝移动时推进，还是只通过 rejected payload 表达。
