# plan-bughunt-non-humanoid-meridian-legacy-panic-v1

主题：非人形经脉在 legacy-only 战斗与渡劫边界安全跳过，不再以进程 panic 结束服务器。

阶段总览：P0 ✅ 2026-09-28；P1 ✅ 2026-09-28；验收日期：2026-09-28。

## §0 摘要

**来源 Issue：#1887、#1344。** 非人形经脉使用 `MeridianChannelId` 的专属通道时，战斗经脉系统仍把它强制转换成仅覆盖 humanoid 的 legacy `MeridianId`。`to_meridian_id()` 返回 `None` 后，`zhenmai_v2` 与渡劫失败惩罚路径用 `unwrap_or_else(panic!)` 直接杀死服务器；Dugu 的 legacy 注入还会把非人形命中的占位部位映射到目标不存在的经脉并触发 `MeridianSystem::get` panic。现已按 legacy-only 边界改为显式跳过，同时保留非人形通道的关闭与通用战斗路由。

接入面：进料是 `MeridianSystem`/`MeridianChannelId` 和非人形 race 的经脉配置；出料应是可拒绝或按通道处理的结果，不得是进程 panic。共享类型是 `MeridianChannelId`、`MeridianId`、`MeridianSystem` 与 `Race`；本问题没有 agent/client wire 变更。依据 `docs/worldview.md` §四的经脉可见性约束，非人形专属经脉不能被假装成 humanoid 枚举。此路径不改变真元，qi_physics 仅需保持现有调用点的账本语义。

## §1 游玩影响

- 非人形玩家或 NPC 在正常使用经脉招式、打开经脉或触发渡劫失败惩罚时，可能让整个无头服务器 panic，而不是只拒绝该动作。
- 这是实体数据决定的可达路径，不需要开发命令或异常网络包；只要 race 的 `MeridianChannelId` 不在 legacy 映射表内即可触发。
- 服务器进程退出会中断同服其他玩家，持久化与正在进行的战斗也无法完成。

## §2 复现路径

1. 创建带非 humanoid 专属 channel（例如不能映射到 `MeridianId::ALL` 的 `MeridianChannelId`）的 `MeridianSystem`。
2. 让 `zhenmai_v2` 的 `first_open_meridian` 或 `open_meridians` 读取该通道；两者都调用 `meridian_channel_id_to_legacy`。
3. 或在渡劫失败惩罚中让 `apply_tribulation_failure_penalty` 关闭该通道。
4. 验真测试在修复前命中上述 panic；修复后 `zhenmai_v2` 过滤无法逆映射的通道、渡劫仍关闭通道但不发不可表示的 legacy 事件，Dugu 在目标不拥有 legacy 经脉时安全拒绝本次注入。

## §3 根因证据

- `server/src/combat/zhenmai_v2.rs:1409-1432` 的 legacy 读取器现在对 `to_meridian_id()` 使用 `find_map`/`filter_map`；非 humanoid channel 被跳过，可映射的 humanoid 结果保持不变。
- `server/src/cultivation/tribulation.rs:4144-4146` 在 `apply_tribulation_failure_penalty` 中仍关闭选中的真实 channel，仅在存在 legacy id 时追加 `MeridianSeveredEvent` 所需的 id；非人形 channel 不再触发 panic 或伪造 id。
- `server/src/cultivation/dugu.rs:572-587` 将 `body_part_to_meridian` 改为 `Option<MeridianId>`；`on_attack_resolved_dugu_handler:298-316` 同时检查映射结果和目标 `MeridianSystem::contains`，非人形目标安全跳过一次性注入，避免对不存在的 legacy channel 调用 `get`。
- 反证：`server/src/combat/baomai_v4/crack_reading.rs:162-173` 已使用 `Option` 处理缺少 legacy 映射，说明该输入在今天代码中真实存在且可安全分支处理；`qi_zero_decay.rs:129`、`meridian/severed.rs:366` 等调用也采用 `if let`/`let Some` 守卫。

## §4 非重复比对

- #1954 对应的 `baomai_v4/crack_reading` 已安全处理并不属于本骨架；其修复不能覆盖 `zhenmai_v2` 或 `tribulation`。
- `docs/plans-skeleton/plan-bughunt-meridian-channel-legacy-map-v1.md`（如后续存在）若只处理数据映射表，仍需覆盖本骨架的运行时拒绝/跳过契约；当前搜索未发现同时拥有这两个 panic 调用点的既有 skeleton。
- `docs/finished_plans/plan-meridian-severed-v1.md` 处理断脉状态与技能门禁，不拥有 `MeridianChannelId` 到 legacy 枚举的 panic 边界。

## §5 修复计划骨架

### P0：非人形通道的安全边界 ✅ 2026-09-28

- `zhenmai_v2` 的 `meridian_channel_id_to_legacy`、`first_open_meridian`、`open_meridians` 用 `Option` 过滤 legacy-only 结果；通用 channel 不被伪造成人形枚举。
- `apply_tribulation_failure_penalty` 先完成真实 channel 的关闭与 `qi_max` 重算，再仅为可表示的 humanoid channel 生成 legacy id。
- Dugu 的 `body_part_to_meridian` 返回 `Option`，攻击处理器以 `MeridianSystem::contains` 守卫 legacy 查询；非人形目标明确跳过 Dugu 注入并清除一次性 pending 状态。

### P1：最小回归契约 ✅ 2026-09-28

- `server/src/combat/zhenmai_v2_tests.rs:1080-1093` 断言非人形开放通道在 legacy-only 读取器中安全跳过。
- `server/src/cultivation/tribulation_tests.rs:227-259` 断言 17 条非人形通道触发失败惩罚时会关闭多余通道、重算状态且不产生不可表示的 legacy 事件。
- `server/src/cultivation/dugu.rs:1456-1513` 以非人形目标接收 QiNeedle 攻击，断言不崩服、不挂 `DuguPoisonState`，并消费一次性注入。
- `body_part_to_meridian` 的 humanoid Q58 映射测试继续锁定 `Some(MeridianId)`，保证既有映射不变。

## §6 验证计划

实现后在 server 栈运行 fmt、clippy 与测试；针对性回归已覆盖 `zhenmai_v2`、`tribulation`、`dugu` 及现有非人形链路，最终门禁按 server 栈完整执行。

## §7 跨仓契约与可核验锚点

- **Inputs：** `MeridianSystem`、`MeridianChannelId`、非人形 `Race` 经脉配置以及 legacy 映射调用点。
- **Outputs：** `meridian_channel_id_to_legacy` 的 `None` 走过滤；`apply_tribulation_failure_penalty` 保留真实 channel 关闭并跳过不可表示事件；Dugu 的 `None`/`contains == false` 走拒绝分支，server 继续运行；可映射 humanoid channel 保持原结果。
- **共享类型/事件：** `MeridianChannelId`、`MeridianId`、`MeridianSystem`、`Race`；server 符号为 `meridian_channel_id_to_legacy`、`first_open_meridian`、`open_meridians`、`apply_tribulation_failure_penalty`、`body_part_to_meridian` 与 `on_attack_resolved_dugu_handler`。
- **同类调用点审计：** `cmd/dev/meridian.rs:142`、`combat/baomai_v4/crack_reading.rs:162,234`、`cultivation/burst_meridian.rs:966`、`cultivation/forging.rs:165`、`cultivation/meridian/severed.rs:366`、`cultivation/meridian_open.rs:209,396`、`cultivation/qi_zero_decay.rs:129`、`network/client_request_handler.rs:732`、`network/cultivation_bridge.rs:96` 均已是 `map_or_else`/`match`/`let Some`/组合守卫，未发现同类 panic；本 plan 处理今天仅剩的三个直接 panic 桥接与 Dugu 目标 profile 查询。
- **三端契约符号：** Server 负责 Option/拒绝边界；Agent：无变更，理由是只读 server ECS 经脉/境界状态；Client：无变更，理由是 Fabric payload、HUD 和 skill wire id 不变。
- **Qi：** 该 bug 不产生新的 qi 流动；保留既有 `QiTransfer` 语义。若回归建快照，断言用 `qi_physics::ledger::assert_conservation`、`QI_EPSILON` 与 `crate::schema::common::SPIRIT_QI_TOTAL`，不写总量字面量，也不从 player ledger 账户扣款。
- **worldview 锚点：** `docs/worldview.md` §四的经脉可见性和非人形通道边界；不能把专属 channel 强转 humanoid。

## Finish Evidence

- **落地清单：** `server/src/combat/zhenmai_v2.rs` 的 legacy 经脉读取过滤；`server/src/cultivation/tribulation.rs` 的失败惩罚通道关闭与 legacy 事件边界；`server/src/cultivation/dugu.rs` 的 Option 映射、目标 profile 守卫与非人形攻击契约测试；对应单测位于 `zhenmai_v2_tests.rs`、`tribulation_tests.rs`。
- **关键 commit：** `2135fee3f`（2026-09-28，promotion：骨架转 active）；`72d73c027`（2026-09-28，修复 legacy 映射 panic 并补非人形回归契约）。
- **测试结果：** `scripts/build-token.sh cargo fmt` 已通过；`scripts/build-token.sh cargo test --lib non_humanoid -- --nocapture`：37 passed；Dugu、zhenmai、tribulation 三条新增针对性用例分别通过。
- **跨仓库核验：** server 命中 `MeridianChannelId`、`MeridianSystem`、`meridian_channel_id_to_legacy`、`apply_tribulation_failure_penalty`、`body_part_to_meridian`、`on_attack_resolved_dugu_handler`；agent 无变更且无消费这些 server ECS 内部符号；client 无变更，Fabric payload/HUD 与 skill wire id 不变。
- **遗留 / 后续：** `zhenmai_v2`、Dugu 及渡劫的 legacy-only 旧接口仍待后续 P1 开放化时迁移到 `MeridianChannelId` 原生表达；本 plan 不改 agent/client wire，也不引入新的 `qi_physics` 流动。
