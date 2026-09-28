# plan-bughunt-non-humanoid-meridian-legacy-panic-v1

## §0 摘要

**来源 Issue：#1887、#1344。** 非人形经脉使用 `MeridianChannelId` 的专属通道时，战斗经脉系统仍把它强制转换成仅覆盖 humanoid 的 legacy `MeridianId`。`to_meridian_id()` 返回 `None` 后，`zhenmai_v2` 与渡劫失败惩罚路径用 `unwrap_or_else(panic!)` 直接杀死服务器。该骨架只记录今天 `origin/main` 的证据，不修改生产代码。

接入面：进料是 `MeridianSystem`/`MeridianChannelId` 和非人形 race 的经脉配置；出料应是可拒绝或按通道处理的结果，不得是进程 panic。共享类型是 `MeridianChannelId`、`MeridianId`、`MeridianSystem` 与 `Race`；本问题没有 agent/client wire 变更。依据 `docs/worldview.md` §四的经脉可见性约束，非人形专属经脉不能被假装成 humanoid 枚举。此路径不改变真元，qi_physics 仅需保持现有调用点的账本语义。

## §1 游玩影响

- 非人形玩家或 NPC 在正常使用经脉招式、打开经脉或触发渡劫失败惩罚时，可能让整个无头服务器 panic，而不是只拒绝该动作。
- 这是实体数据决定的可达路径，不需要开发命令或异常网络包；只要 race 的 `MeridianChannelId` 不在 legacy 映射表内即可触发。
- 服务器进程退出会中断同服其他玩家，持久化与正在进行的战斗也无法完成。

## §2 复现路径

1. 创建带非 humanoid 专属 channel（例如不能映射到 `MeridianId::ALL` 的 `MeridianChannelId`）的 `MeridianSystem`。
2. 让 `zhenmai_v2` 的 `first_open_meridian` 或 `open_meridians` 读取该通道；两者都调用 `meridian_channel_id_to_legacy`。
3. 或在渡劫失败惩罚中让 `apply_tribulation_failure_penalty` 关闭该通道。
4. `to_meridian_id()` 返回 `None`，当前 `unwrap_or_else` 执行 panic，服务器退出。

## §3 根因证据

- `server/src/combat/zhenmai_v2.rs:1409-1416` 的 `meridian_channel_id_to_legacy` 对 `to_meridian_id()` 的 `None` 分支调用 `panic!`；`first_open_meridian`（约 `1419-1425`）和 `open_meridians`（约 `1428-1437`）都无额外 race/通道守卫。
- `server/src/cultivation/tribulation.rs:4144-4152` 在 `apply_tribulation_failure_penalty` 中重复使用 `channel_id.to_meridian_id().unwrap_or_else(|| panic!(...))`。渡劫失败是正常玩法状态转换，不能以无法表示 legacy id 为进程级错误。
- `server/src/cultivation/dugu.rs:566` 仍是同一类数据完整性调用点，当前也以 `unwrap_or_else` 假设每条通道必有 legacy 映射；实现阶段必须一并审计，决定安全跳过、保留通用 channel 或返回拒绝，不能只修前两处而留下同类 panic。
- 反证：`server/src/combat/baomai_v4/crack_reading.rs:162-173` 已使用 `Option` 处理缺少 legacy 映射，说明该输入在今天代码中真实存在且可安全分支处理；`qi_zero_decay.rs:129`、`meridian/severed.rs:366` 等调用也采用 `if let`/`let Some` 守卫。

## §4 非重复比对

- #1954 对应的 `baomai_v4/crack_reading` 已安全处理并不属于本骨架；其修复不能覆盖 `zhenmai_v2` 或 `tribulation`。
- `docs/plans-skeleton/plan-bughunt-meridian-channel-legacy-map-v1.md`（如后续存在）若只处理数据映射表，仍需覆盖本骨架的运行时拒绝/跳过契约；当前搜索未发现同时拥有这两个 panic 调用点的既有 skeleton。
- `docs/finished_plans/plan-meridian-severed-v1.md` 处理断脉状态与技能门禁，不拥有 `MeridianChannelId` 到 legacy 枚举的 panic 边界。

## §5 修复计划骨架

### P0：非人形通道的安全边界

- 统一把 legacy-only 调用点改为 `Option`/显式错误分支；为能保留通用 channel 的路径优先保留 channel，必须降级为 legacy 行为时则记录可观察拒绝，不 panic。
- 明确 `zhenmai_v2`、渡劫失败惩罚和 `cultivation::dugu` 对非 humanoid channel 的责任边界，避免同一实体在一个系统被跳过、另一个系统被杀服。

### P1：最小回归契约

- 非人形专属 channel 经过经脉读取、渡劫惩罚和 Dugu 数据完整性路径时，进程保持运行且结果可断言。
- humanoid channel 的 legacy 映射行为保持不变。

## §6 验证计划

实现后在 server 栈运行 fmt、clippy 与测试；至少运行相关 `zhenmai_v2`、`tribulation`、`dugu` 单测及非人形回归测试。此 skeleton 阶段不编译。

## §7 跨仓契约与可核验锚点

- **Inputs：** `MeridianSystem`、`MeridianChannelId`、非人形 `Race` 经脉配置以及 legacy 映射调用点。
- **Outputs：** `meridian_channel_id_to_legacy` 的 `None` 走可观察拒绝/跳过，server 继续运行；可映射 humanoid channel 保持原结果。
- **共享类型/事件：** `MeridianChannelId`、`MeridianId`、`MeridianSystem`、`Race`；server 符号为 `meridian_channel_id_to_legacy`、`first_open_meridian`、`open_meridians`、`apply_tribulation_failure_penalty` 和 `cultivation::dugu` 调用点。
- **三端契约符号：** Server 负责 Option/拒绝边界；Agent：无变更，理由是只读 server ECS 经脉/境界状态；Client：无变更，理由是 Fabric payload、HUD 和 skill wire id 不变。
- **Qi：** 该 bug 不产生新的 qi 流动；保留既有 `QiTransfer` 语义。若回归建快照，断言用 `qi_physics::ledger::assert_conservation`、`QI_EPSILON` 与 `crate::schema::common::SPIRIT_QI_TOTAL`，不写总量字面量，也不从 player ledger 账户扣款。
- **worldview 锚点：** `docs/worldview.md` §四的经脉可见性和非人形通道边界；不能把专属 channel 强转 humanoid。
