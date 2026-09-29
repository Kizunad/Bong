# plan-bughunt-wounds-severity-scale-v1

> 来源 Issue：#1869。server 发送绝对伤害值，client 却按 0..1 严重度截断，真实伤口被错误显示为断肢。
>
> 阶段总览：P0 ⬜ 统一 wounds_snapshot 的 severity 量纲；P1 ⬜ 锁定 server/client/schema 回归契约。

## §0 摘要

combat::resolve 将最低伤害钳到 1.0，并把该绝对值写入 Wound.severity。wounds_snapshot_emit::wound_to_wire 原样发送 severity。client 的 WoundsStore.Wound 构造函数却将 severity 当作 0..1 clamp，WoundLayerBinding 再用 0.05、0.25、0.55、0.85 的归一化阈值分档。除骨折特殊分支外，任何大于 1 的真实伤害都会先被截成 1，最终显示为 SEVERED。

## §1 实际游玩体验影响

- 轻微刀伤、擦伤和较重裂伤在 inspect 画面中都可能显示为断肢，玩家无法按伤势判断还能否战斗。
- HUD 的出血、愈合和功能提示建立在错误档位上，治疗与战斗决策会被误导；server 的实际生命/伤口状态没有改变，问题集中在 wire 与 client 映射。

## §2 复现路径

1. 让 resolve 产生非 bone_fracture 的 5.0 或 35.0 伤害；resolve 会以绝对值写入 Wound.severity。
2. emit_wounds_snapshot_payloads 原样生成 wounds_snapshot，client handler 读取该数值。
3. WoundsStore.Wound 构造函数将 5.0 或 35.0 clamp 为 1.0。
4. WoundLayerBinding.toWoundLevel 以 s >= 0.85 返回 SEVERED，观察轻伤与重伤被合并。

## §3 根因证据

- server/src/combat/resolve.rs:917-918 将 base_damage 最低钳为 1.0；:1268-1275 直接以 damage 填充 Wound.severity。
- server/src/network/wounds_snapshot_emit.rs:74-88 的 wound_to_wire 原样输出 wound.severity，没有单位转换或版本标记。
- server/src/combat/arm_wound.rs:125-140 仍以 5、15、35、70 的绝对值划分伤势档位，与 client 的归一化阈值不是同一量纲。
- server/src/schema/combat_hud.rs:16-35 的 WoundEntryV1 只有 severity: f32，没有说明绝对值还是归一化值。
- client/src/main/java/com/bong/client/combat/store/WoundsStore.java:19-34 注释和构造函数把 severity 当 0..1 并 clamp；:123-128 明确执行 clamp01。
- client/src/main/java/com/bong/client/combat/inspect/WoundLayerBinding.java:76-86 用 0.05/0.25/0.55/0.85 分档，导致被截成 1.0 的条目进入 SEVERED。

## §4 非重复比对

- docs/worldview.md §一/§五的体表模型定义连续伤口数值映射到六档；本 plan 处理 wire 量纲，不重写伤口传播或治疗规则。
- 现有 combat HUD 文档只规定 wounds_snapshot 字段形状，不含当前 server 绝对 damage 与 client 归一化假设的冲突；不要与经脉损伤或污染 plan 合并。

### 立项检查记录（2026-09-28）

- docs/worldview.md：检索“伤口、体表、六档、severity、断肢、骨折”，核对 §五 L215-L260 的连续值与六档表。
- docs/finished_plans/：检索 wounds_snapshot、WoundEntryV1、WoundLayerBinding、wound_severity_to_grade；命中 HUD/战斗基础 plan，未发现该量纲漂移的修复证据。
- active plan（docs/plan-*.md）：检索上述符号及 severity clamp；未发现 active plan 已拥有 wounds_snapshot 的单位迁移。
- docs/plans-skeleton/：检索 wounds_snapshot、WoundEntryV1、severity clamp、SEVERED；未发现同主题骨架。
- docs/plans-skeleton/reminder.md：检索 wounds_snapshot、severity、体表伤口；仓内无该主题延后事项。

## 接入面与跨仓契约

- **Inputs**：server Wound.severity、WoundKind、arm_wound 的绝对档位、Changed<Wounds> 查询。
- **Outputs**：wounds_snapshot 的 severity 和 client WoundsStore/WoundLayerBinding 的同一量纲；伤口档位、出血和愈合显示可核验。
- **共享类型 / 事件**：Wound、Wounds、WoundEntryV1、WoundsSnapshotV1、ServerDataPayloadV1::WoundsSnapshot、wounds_snapshot。
- **server 契约符号**：emit_wounds_snapshot_payloads、wounds_to_wire、wound_to_wire、WoundEntryV1::severity、arm_wound::wound_severity_to_grade。
- **agent**：无变更；当前 agent/packages/schema 没有 WoundsSnapshot 的 TypeBox source，且该 server_data 由 Rust schema 直接发给 client，修复不经过 agent IPC。
- **client**：有变更；WoundsSnapshotHandler、WoundsStore.Wound、WoundLayerBinding.toWoundLevel 必须采用 server 明确的单位。
- **worldview 锚点**：docs/worldview.md §五 L215-L260 的体表 16 部位和六档伤口；归一化或绝对值只能选一个并在 schema 注释中固定。
- **qi_physics**：无真元转移；不要把伤口 severity 当作 qi 数值，也不新增 qi_physics 常量。

## §5 修复骨架

### P0 固定 severity 的唯一单位

- 在 server/client/schema 三端先写出同一份单位契约：severity 是归一化 0..1，或是与 arm_wound 相同的绝对伤害值；禁止 server 原样发送而 client 静默 clamp。
- 优先在 server 的 wound_to_wire 建立有名字的转换 helper，把绝对伤害映射到 worldview 的六档归一化边界，再保留 client 的 0..1 分档；若代码审查确认绝对值才是正式 wire，则同步移除 client clamp 并复用 server 档位常量。无论选择哪条，不能用字面 1.0 把未知单位截断。
- 在 WoundEntryV1 注释、client handler 和 schema contract test 中写明单位，覆盖 0、各档边界、超出最大档和 bone_fracture 特殊分支。

### P1 回归与验收

- 代表性绝对伤害 1、5、15、35、70 在 client 上分别映射到预定档位，非骨折路径不再全部 SEVERED。
- wounds_snapshot 的感染、scar、state、updated_at_ms 行为保持不变；只验证 severity 量纲和可观察档位。
- server、client 和 wire fixture 的同一组边界值通过契约测试，agent 继续保持无变更。
