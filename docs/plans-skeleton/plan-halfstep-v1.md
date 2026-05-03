# Bong · plan-halfstep-v1 · 骨架

**半步化虚精细化**：buff 强度校准（真元上限/寿元加成从占位常量迁移到可调 config）+ 重渡机制明确（名额空出时半步化虚玩家的重起劫规则与 UI 提示）。

**世界观锚点**：`worldview.md §三 line 72`（服务器内仅容 1-2 人化虚，末法天道无力承担更多）· `§三 通灵→化虚` 突破代价

**交叉引用**：`plan-tribulation-v1` ✅（渡虚劫主流程，半步结算已实装）· `plan-void-quota-v1` ⬜（名额公式大改，若先落地则本 plan P0 参数源需对接）· `plan-death-lifecycle-v1` ✅（化虚者死亡 → `release_ascension_quota_slot` → `AscensionQuotaOpened` 事件链）

---

## 接入面 Checklist

- **进料**：`server/src/cultivation/tribulation.rs` 两个占位常量（`HALF_STEP_QI_MAX_MULTIPLIER = 1.10` / `HALF_STEP_LIFESPAN_YEARS = 200`，第 73-74 行）· `AscensionQuotaOpened` 事件（`combat/lifecycle.rs`）· `ascension_quota` 持久表
- **出料**：可热调的 config 字段（不重新编译即可调参）· 半步化虚玩家定向通知（`AscensionQuotaOpened` 时区分广播 vs 定向提示）· client 修炼面板"半步化虚"状态展示
- **共享类型**：`CharacterCultivation.is_half_step: bool`（新增字段，标记永久半步状态）
- **跨仓库契约**：server `HalfStepConfig` config 字段 + client 修炼面板状态展示 + agent narration（定向通知文案）
- **worldview 锚点**：§三 化虚名额上限机制

---

## §0 设计轴心

当前实装状态：
- `HALF_STEP_QI_MAX_MULTIPLIER: f64 = 1.10` 与 `HALF_STEP_LIFESPAN_YEARS: u32 = 200` 是**硬编码占位**（`server/src/cultivation/tribulation.rs:73-74`）
- 半步玩家无持久标记，外部系统无法区分"通灵圆满+半步 buff"和"普通通灵圆满"
- `AscensionQuotaOpened` 已触发 agent 全服广播，但无针对半步玩家的定向通知

本 plan 解决：
- [ ] **buff 可调**：迁移两个常量到 server config（无需重新编译即可调参）
- [ ] **状态可见**：角色数据新增 `is_half_step` 持久字段，确保下游系统（client UI / agent / 统计）可识别
- [ ] **重渡规则明确**：名额空出时半步玩家的起劫前置条件、buff 叠加上限、定向通知 UX
- [ ] **buff 叠加上限**：同一角色二次半步结算时，buff 不重复累加（qi_max 上限取 max 而非再乘，寿元同理）

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | buff 常量迁移 config + `is_half_step` 持久字段 | 运维改 config 不重编译；`is_half_step` 落 SQLite |
| **P1** ⬜ | buff 叠加上限（二次半步不重复累加）+ 重渡起劫规则确认 | 二次半步场景单测通过 |
| **P2** ⬜ | `AscensionQuotaOpened` 时给半步玩家定向 HUD 提示 | 半步玩家收到专属提示；非半步通灵收到全服广播 |
| **P3** ⬜ | client 修炼面板展示"半步化虚"状态（区分普通通灵圆满） | 半步玩家修炼面板可见专属状态 + 重渡按钮提示 |

---

## §2 P0 详细：config 迁移 + 持久字段

### buff 迁移

```rust
// server/src/cultivation/tribulation.rs 中删除
// const HALF_STEP_QI_MAX_MULTIPLIER: f64 = 1.10;
// const HALF_STEP_LIFESPAN_YEARS: u32 = 200;

// 改为从 TribulationSettings 读取
pub struct TribulationSettings {
    // ...已有字段...
    pub half_step_qi_max_multiplier: f64,  // 默认 1.10
    pub half_step_lifespan_years: u32,     // 默认 200
}
```

- [ ] `server/src/cultivation/tribulation.rs`：删除硬编码常量，改读 `TribulationSettings`
- [ ] `server/src/settings.rs`（或 config 文件）：新增两个字段，附默认值注释说明"观察期占位，运营后按玩家滞留率调整"
- [ ] 相关单测：验证默认值 = 旧常量，确保无回归

### `is_half_step` 持久字段

- [ ] `server/src/cultivation/mod.rs` 或 `CharacterCultivation` component：新增 `is_half_step: bool`，默认 `false`
- [ ] 半步结算时（`tribulation.rs:1128` 附近）：`c.is_half_step = true`
- [ ] SQLite schema：`alter table characters add column is_half_step integer not null default 0`
- [ ] 加载/存储路径：`persistence/mod.rs` 读写 `is_half_step`

---

## §3 P1 详细：叠加上限 + 重渡规则

### buff 叠加上限

二次半步结算时不重复累加：
```rust
// qi_max 叠加上限：取 max 而非再乘
let target_qi_max = original_base_qi_max * settings.half_step_qi_max_multiplier;
c.qi_max = c.qi_max.max(target_qi_max);

// 寿元叠加上限：同理取 max
let target_lifespan_cap = LifespanCapTable::SPIRIT.saturating_add(settings.half_step_lifespan_years);
lifespan.cap_by_realm = lifespan.cap_by_realm.max(target_lifespan_cap);
```

- [ ] 单测：第一次半步 → qi_max 和寿元正确；第二次半步 → 不再增加（取 max 结果相同）

### 重渡规则（明确确认以下各点）

- [ ] **前置条件**：半步玩家（`is_half_step = true`，通灵境，奇经八脉全通）在名额 `< quota_limit` 时可直接起劫，**无额外代价**
- [ ] **buff 保留**：重渡失败（天劫中阵亡或逃劫）→ 半步 buff 保留，`is_half_step` 仍为 `true`，退境后可再次重渡
- [ ] **成功化虚**：`is_half_step` 重置为 `false`（已化虚，不再是半步状态）
- [ ] **不重复占名额**：半步玩家起劫时仍走 `check_void_quota` 正常占名额，成功化虚才计入 `current_void_count`

---

## §4 P2 详细：定向通知

`AscensionQuotaOpened` 触发时当前只有全服 agent 广播。新增定向路径：

- [ ] `combat/lifecycle.rs`：`AscensionQuotaOpened` 处理时查询所有在线 `is_half_step = true` 的玩家
- [ ] 对这些玩家发送定向 HUD 通知（通灵修炼面板顶部横幅）："化虚有位，汝可叩关"
- [ ] agent narration 文案区分：全服广播文案保持"化虚有位，叩关者可往"；定向文案更私密（天道单独"点名"）
- [ ] 离线玩家：下次登录时检查是否存在可用名额且 `is_half_step = true` → 登录后弹一次横幅提示

---

## §5 P3 详细：client 修炼面板

- [ ] `client/.../CultivationScreen.java`（或对应 owo-lib Screen）：新增"半步化虚"状态展示区
  - 与"通灵圆满"状态区分（专属图标或描述文字）
  - 显示当前化虚名额（从 `AscensionQuotaStore` 读取）
  - 名额可用时亮起"可叩关"提示 + 高亮起劫按钮
- [ ] `client` → `server` 协议：`AscensionQuotaStore` 已有 client 存储（`plan-tribulation-v1 §6`），补充 `is_half_step` 字段同步

---

## §6 数据契约

```rust
// 角色持久化新增字段
pub struct CharacterCultivation {
    // ...已有字段...
    pub is_half_step: bool,  // 已获半步化虚 buff，名额空出时可重渡
}

// config 新增字段
pub struct TribulationSettings {
    // ...已有字段...
    pub half_step_qi_max_multiplier: f64,  // 默认 1.10
    pub half_step_lifespan_years: u32,     // 默认 200
}
```

IPC：无新 channel，定向通知复用现有 HUD 机制（参考 `TribulationBroadcastHudPlanner` 模式）

---

## §7 开放问题

- [ ] `half_step_qi_max_multiplier` / `half_step_lifespan_years` 的**调参判据**是什么？建议先跑 2 周观察"半步玩家中位数等待名额时间"，若 > 30 天 in-game 则可适当上调 buff 作为补偿
- [ ] 半步玩家被截胡杀死后：物品全归截胡者（走正常死亡），`is_half_step` 保留（还是清除？）——**默认保留**，截胡者拿走物品但半步状态随角色走
- [ ] 名额长时间（> 1 in-game year）为 0 时，半步玩家的 HUD 提示是否应该有 "等待超时" 的 narration 补充？（天道嘲讽"汝苦等多年，名额终来"）
- [ ] P3 client 面板是否需要展示"预计名额开放时间"？（化虚者寿元剩余 → 粗略推算）—— **初版不做**，避免过度透露信息

## §8 进度日志

- 2026-05-03：骨架创建。源自 `plans-skeleton/reminder.md` plan-tribulation-v1 条目（buff 强度占位 + 重渡机制待确认）。
