# plan-test-cmd-v1

把"快速搭测试场景"的所有内核 mutate 入口收成一组 brigadier dev 命令：经脉打通、境界跳转、真元设置、功法增删/熟练度、物品给予/清空背包、区域灵气浓度、死亡/复活、时间快进。**全部走 valence_command brigadier 树（与现有 `/health` `/spawn` `/tptree` 同栈），客户端 Tab 自动补全，agent / client 零改动。**

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | `/meridian` — open / open_all / list（强制打通经脉，绕开 zone_qi/qi 阈值） | ✅ 2026-05-11 |
| P1 | `/realm set <id>` + `/qi set <v>` + `/qi max <v>`（境界 + 真元直写） | ✅ 2026-05-11 |
| P2 | `/technique` — list / add / remove / proficiency / active / reset_all | ✅ 2026-05-11 |
| P3 | `/give <template_id> [count]` + `/clearinv [pack\|all\|naked]` | ✅ 2026-05-11 |
| P4 | `/zone_qi set <name> <value>`（区域灵气浓度直写） | ✅ 2026-05-11 |
| P5 | `/kill self` + `/revive self`（PlayerTerminated / PlayerRevived event 触发） | ✅ 2026-05-11 |
| P6 | `/time advance <ticks>`（只推进 `CultivationClock.tick`；不推进 Bevy `Time`） | ✅ 2026-05-11 |
| P7 | 测试饱和 + brigadier 命令树 fixture 更新（`registry_pin::COMMAND_NAMES`） | ✅ 2026-05-11 |
| P8 | 文档同步（CLAUDE.md / 测试章节 / 命令清单） | ✅ 2026-05-11 |

---

## 接入面（必读：本 plan 是 dev-only 测试工具，**显式绕过** worldview/qi_physics 自然规则）

### 进料

- `cultivation::components::{Cultivation, MeridianSystem, MeridianId, Realm}`（read+write）
- `cultivation::known_techniques::{KnownTechniques, KnownTechnique, TECHNIQUE_DEFINITIONS, TECHNIQUE_IDS}`（read+write）
- `cultivation::meridian_open::MERIDIAN_CAPACITY_ON_OPEN`（read，复用 +10 capacity 常数）
- `cultivation::life_record::{LifeRecord, BiographyEntry::MeridianOpened}`（write，可选写入生平）
- `cultivation::tick::CultivationClock`（read+write）
- `inventory::{PlayerInventory, ItemRegistry, InventoryInstanceIdAllocator, add_item_to_player_inventory, MAIN_PACK_CONTAINER_ID}`（read+write）
- `world::zone::ZoneRegistry::find_zone_mut(name)`（write `Zone.spirit_qi`）
- `combat::events::{PlayerTerminated, PlayerRevived, CultivationDeathTrigger, CultivationDeathCause}`（emit）

### 出料

- 直接 mutate 玩家身上的 component（Cultivation / MeridianSystem / KnownTechniques / PlayerInventory）
- 写 `ZoneRegistry` 中的 `Zone.spirit_qi`
- emit `PlayerTerminated` / `PlayerRevived` event 触发既有死亡-重生管线
- 客户端 chat 反馈每条命令执行结果（`Client::send_chat_message`）
- `network::techniques_snapshot_emit` 自动 push 改动后的 `KnownTechniques` 到客户端（**已有 system，本 plan 不改**）

### 共享类型 / event

- 复用 `MeridianId` / `Realm` enum，**不新建**
- 复用 `KnownTechniques` Component，**不新建** technique entry 类型
- 复用 `PlayerTerminated` / `PlayerRevived` event，**不新建**死亡 event
- 复用 `add_item_to_player_inventory()` API，**不新建**给物品路径
- 新写一个工具函数 `inventory::clear_player_inventory(inv, scope)`（`enum ClearScope { PackOnly, PackAndHotbar, All }`），**这是本 plan 唯一新增的 inventory helper**，因为现状没有 clear 入口

### 跨仓库契约

- **server**：新增 `server/src/cmd/dev/{meridian,realm,qi,technique,give,clearinv,zone_qi,kill,revive,time}.rs`；`registry_pin::COMMAND_NAMES` + `COMMAND_TREE_PATHS` 必须同步加入；新增 `inventory::clear_player_inventory()` helper
- **agent**：无改动（dev 命令不通过 Redis IPC，agent 不感知）
- **client**：无改动（Fabric 微端通过原版 brigadier 协议自动收到命令树和 Tab 补全；`KnownTechniques` snapshot 经现有 `network::techniques_snapshot_emit` 自动推送）
- **worldgen**：无改动

### worldview 锚点

**显式声明：本 plan 是 dev 测试工具，绕过 worldview 第一/二/四章的自然修炼规则**——`/realm set` 跳过经脉数量门槛、`/qi set` 凭空生成真元、`/zone_qi set` 直写区域灵气、`/meridian open` 跳过邻接 + zone_qi 阈值。所有命令必须：

1. 仅注册在 dev 命令树（`server/src/cmd/dev/`），不进 gameplay；
2. 执行时打 `tracing::warn!("[dev-cmd] bypass worldview rule: ...")`，便于排查"为什么这个玩家境界这么离谱"；
3. 在 chat 反馈中带 `[dev]` 前缀，让在场玩家看清楚这是测试操作。

### qi_physics 锚点

**显式声明：本 plan 绕过 qi_physics::ledger 的守恒律**（worldview §二/§十「全服灵气总量恒定」）。`/qi set`、`/zone_qi set` 都不发 `QiTransfer`，凭空创/销真元 —— 这是测试场景必需，但**生产路径任何代码不允许复用本 plan 的写入路径**。

具体规避红旗：
- 不在 `qi_physics` 模块加新常数/新公式；
- 不复用本 plan 的 `Cultivation.qi_current = X` 直写到 cultivation/combat/zone 任何系统系统；
- 命令实现里 `tracing::warn!` 标记 "bypass ledger" 字样，方便审计；
- 单测专门验"`/qi set` 不 emit QiTransfer event"（pin 行为，防止有人不小心改成走 ledger 后引入双写 bug）。

---

## P0 — `/meridian` 强制打通

`/meridian open <id>`：把指定经脉打通（绕开 `MIN_ZONE_QI_TO_OPEN` / `OPEN_COST_FACTOR` 检查），等价于 `advance_open_progress_at` 的"成功"分支。
`/meridian open_all`：一键打通全部 20 条（12 正经 + 8 奇经）。
`/meridian list`：列出当前已开经脉、open_progress、flow_capacity。

**交付物**：

- `server/src/cmd/dev/meridian.rs`：`enum MeridianCmd { Open { id: String }, OpenAll, List }`，`assemble_graph` 注册 `meridian open <id:string>` / `meridian open_all` / `meridian list` 三条 brigadier 路径
- handler 里直接写：
  ```rust
  let m = meridians.get_mut(target);
  if !m.opened {
      m.opened = true;
      m.opened_at = clock.tick;
      m.open_progress = 1.0;
      m.flow_capacity = m.flow_capacity.max(MERIDIAN_CAPACITY_ON_OPEN);
      cultivation.qi_max += MERIDIAN_CAPACITY_ON_OPEN;
      life.push(BiographyEntry::MeridianOpened { id: target, tick: clock.tick });
      // 灵根：若是首脉，写 spirit_root_first
  }
  ```
- `MeridianId` 字符串 ↔ enum 解析：新写 `parse_meridian_id(&str) -> Option<MeridianId>`，覆盖 20 条全名和常用简称（`lung` / `large_intestine` / `ren` / `du` / ...）；测试覆盖大小写不敏感
- `cmd::dev::mod.rs` 注册 `meridian::register(app)`
- `registry_pin::COMMAND_NAMES` 加 `"meridian"`；`COMMAND_TREE_PATHS` 加 `meridian list` / `meridian open <id:string>` / `meridian open_all`

**测试**（≥ 8 单测）：
- `parse_meridian_id` 全 20 条命中 + 大小写不敏感 + 简称 + unknown 拒绝
- `/meridian open lung` 写入 `MeridianSystem.lung.opened = true`、`open_progress = 1.0`、`qi_max += 10`
- `/meridian open lung` 重复执行 idempotent（已开不再 +qi_max）
- `/meridian open_all` 全 20 条 opened 且 `qi_max += 200`
- `/meridian list` 输出格式断言（chat 反馈包含已开经脉名）
- 写 LifeRecord：BiographyEntry 数量 = 新打通条数
- 首脉触发 `spirit_root_first` 写入

**验收**：
- 手测：`/meridian open lung` → `/qi list`（或现有 status push）显示 qi_max 从 10 → 20
- `cargo test cmd::dev::meridian` 全绿

---

## P1 — `/realm` + `/qi`（境界 + 真元直写）

`/realm set <id>`：直接写 `Cultivation.realm`，不走 `breakthrough_system`（跳过经脉门槛、SpiritEye、季节修正、qi cost）。
`/qi set <value>`：写 `Cultivation.qi_current`（自动 clamp 到 `qi_max`）。
`/qi max <value>`：写 `Cultivation.qi_max`。

**交付物**：

- `server/src/cmd/dev/realm.rs`：`enum RealmCmd { Set { id: String } }`，`realm set <id:string>` brigadier 路径
- `server/src/cmd/dev/qi.rs`：`enum QiCmd { Set { value: f64 }, Max { value: f64 } }`，`qi set <value:double>` / `qi max <value:double>`
- `parse_realm(&str) -> Option<Realm>`：覆盖 6 个境界中文/英文（`awaken/醒灵`、`induce/引气`、`condense/凝脉`、`solidify/固元`、`spirit/通灵`、`void/化虚`）
- handler 写入时 `tracing::warn!("[dev-cmd] bypass breakthrough: realm {prev:?} -> {next:?}")`
- 注意：`/realm set` 不自动调整 qi_max（用户应配合 `/qi max`）；不写 LifeRecord（因为不是真突破）
- `registry_pin` 加 `"realm"` / `"qi"`，paths 加 `realm set <id:string>` / `qi set <value:double>` / `qi max <value:double>`

**测试**（≥ 10 单测）：
- `parse_realm` 6 境界中文 + 英文命中 + unknown 拒绝
- `/realm set void` 写入 Realm::Void
- `/realm set` 不发 `BreakthroughEvent`、不写 LifeRecord（pin 行为）
- `/qi set 50` 在 qi_max=100 时写入 50
- `/qi set 999` 在 qi_max=100 时 clamp 到 100
- `/qi set -10` 拒绝（或 clamp 到 0，二选一在 plan 实施时定）
- `/qi max 200` 写入 qi_max=200，且 qi_current 不变
- `/qi max 50` 在 qi_current=100 时同步 clamp qi_current 到 50
- `/qi set` / `/qi max` 不 emit `QiTransfer` event（pin 守恒律豁免）

**验收**：
- 手测：`/realm set void` + `/qi max 1000` + `/qi set 1000` 后能直接释放需要 Void 境界 + 高 qi 的招式（如果有该约束）
- `cargo test cmd::dev::{realm,qi}` 全绿

---

## P2 — `/technique`（功法 list / add / remove / proficiency / active / reset_all）

⚠️ **关键现状**：`KnownTechniques::default()` 当前预装全部 25 条 `proficiency=0.5 active=true`（见 `cultivation/known_techniques.rs:16-32`）。本 plan 同时支持"修改已存在 entry"和"add/remove entry"，**面向未来 default 改空集后仍能用**。

子命令：
- `/technique list` — 输出当前 entries 全清单（id / proficiency / active）
- `/technique add <id>` — 若不存在则插入 `{id, proficiency: 0.5, active: true}`；若已存在 no-op + 提示
- `/technique remove <id>` — 删除 entry；若不存在 no-op + 提示
- `/technique proficiency <id> <value>` — 设置 proficiency（0.0..=1.0 clamp）
- `/technique active <id> <bool>` — 设置 active flag
- `/technique reset_all` — 把 `KnownTechniques` 整个重置为 `KnownTechniques::default()`

**交付物**：

- `server/src/cmd/dev/technique.rs`：`enum TechniqueCmd { List, Add { id }, Remove { id }, Proficiency { id, value }, Active { id, value }, ResetAll }`
- 6 条 brigadier 路径
- id 解析：与 `TECHNIQUE_IDS` 静态表对照；unknown id 在 `add` / `proficiency` / `active` 时拒绝（**`add` 拒绝 unknown 是关键约束，否则会污染 client snapshot**）；`remove` 允许任意 id（兼容老 entry）
- 修改 `KnownTechniques` 后由现有 `network::techniques_snapshot_emit` system（`Changed<KnownTechniques>` 触发器）自动 push 客户端，本 plan **不改** snapshot emit 逻辑
- `registry_pin` 加 `"technique"`，paths 加 6 条

**测试**（≥ 12 单测）：
- `/technique list` 输出包含全 25 条 default
- `/technique add burst_meridian.beng_quan` 在已存在时 no-op，entries 长度不变
- `/technique add` unknown id（如 `"foo.bar"`）拒绝并不改 entries
- `/technique remove dugu.shoot_needle` 后 entries 少一条
- `/technique remove unknown_id` no-op + 提示
- `/technique proficiency anqi.echo_fractal 0.9` 写入 0.9
- `/technique proficiency <id> 1.5` clamp 到 1.0
- `/technique proficiency <id> -0.1` clamp 到 0.0（或拒绝，二选一）
- `/technique active <id> false` 写入 active=false
- `/technique reset_all` 把任意状态恢复成 default 25 条
- 改动后 `Changed<KnownTechniques>` 触发器 fire 一次（pin snapshot push 行为）
- 端到端：`add` 后 `list` 包含新条；`remove` 后不包含

**验收**：
- 手测：`/technique active burst_meridian.beng_quan false` 后 client 端 HUD 招式栏看不到崩拳
- `cargo test cmd::dev::technique` 全绿

---

## P3 — `/give` + `/clearinv`

`/give <template_id> [count]`：调 `add_item_to_player_inventory()`；count 默认 1。
`/clearinv [scope]`：scope ∈ {`pack`, `all`, `naked`}，默认 `pack`。
- `pack` — 只清 `MAIN_PACK_CONTAINER_ID`，保留 hotbar + 装备槽
- `all` — 清所有 containers + hotbar，**保留装备槽**（避免裸装走进死亡判定边界）
- `naked` — 清所有 + 装备槽（彻底归零）

**交付物**：

- `server/src/cmd/dev/give.rs`：`enum GiveCmd { Item { id, count: Option<u32> } }`，`give <id:string>` + `give <id:string> <count:integer>` 两条路径
- `server/src/cmd/dev/clearinv.rs`：`enum ClearInvCmd { Clear { scope: Option<String> } }`，`clearinv` + `clearinv <scope:string>` 两条路径
- 新写 `inventory::clear_player_inventory(inv: &mut PlayerInventory, scope: ClearScope)` helper（`enum ClearScope { PackOnly, PackAndHotbar, All }`）；放 `inventory/mod.rs` 旁，**不放 cmd 模块**（其他系统未来可能复用，例如新角色重置）
- `parse_clear_scope(&str) -> Option<ClearScope>` 解析 `pack` / `all` / `naked`
- `give` 失败原因覆盖：unknown template_id / inventory full / count == 0
- `registry_pin` 加 `"give"` / `"clearinv"`，paths 加对应 4 条

**测试**（≥ 14 单测）：
- `/give qicao_grass`（或任一现有 template）后 inventory main_pack 出现一条 stack=1
- `/give qicao_grass 32` stack=32
- `/give unknown_template` 错误返回 + chat 反馈 unknown
- `/give qicao_grass 0` 拒绝（count >= 1）
- `/give` inventory 满时返回 `inventory full` 错误
- `/clearinv pack` 清空 main_pack，hotbar 不变
- `/clearinv all` 清 main_pack + hotbar，装备槽不变
- `/clearinv naked` 全清
- `/clearinv` 默认走 `pack`
- `/clearinv unknown_scope` 拒绝
- `parse_clear_scope` 三档全命中 + unknown 拒绝
- `clear_player_inventory(PackOnly)` 单元测试
- `clear_player_inventory(PackAndHotbar)` 单元测试
- `clear_player_inventory(All)` 单元测试

**验收**：
- 手测：`/give bone_coin 100` 后 HUD 货币 +100；`/clearinv all` 后背包 + hotbar 全空
- `cargo test inventory::tests::clear_player_inventory` + `cmd::dev::{give,clearinv}` 全绿

---

## P4 — `/zone_qi set`

`/zone_qi set <name> <value>`：直接写 `ZoneRegistry::find_zone_mut(name).spirit_qi = value`，绕开自然 `Zone.spirit_qi` tick 演化。

**交付物**：

- `server/src/cmd/dev/zone_qi.rs`：`enum ZoneQiCmd { Set { name: String, value: f64 } }`，`zone_qi set <name:string> <value:double>` brigadier
- handler 调 `ZoneRegistry.find_zone_mut(name)`；不存在时 chat 反馈 zone 列表（最多 10 条 hint）
- value 是否 clamp：spirit_qi 当前类型是 `f64`，正典允许负值（瘟疫带）；不 clamp，但加 `tracing::warn!` 标记 "bypass zone qi tick"
- 不发 `QiTransfer` event（pin 守恒律豁免）
- `registry_pin` 加 `"zone_qi"`，paths 加 `zone_qi set <name:string> <value:double>`

**测试**（≥ 5 单测）：
- `/zone_qi set spawn 0.8` 写入 spawn zone spirit_qi
- `/zone_qi set unknown_zone 0.5` 拒绝
- `/zone_qi set <name> -0.3` 允许负值（瘟疫带语义）
- `/zone_qi set` 不发 QiTransfer event
- 设置后立即生效：下一 tick `meridian_open_tick` 在该 zone 内能 advance（端到端断言）

**验收**：
- 手测：`/tpzone spawn` + `/zone_qi set spawn 1.0` + `/meridian open lung` 推进速度变快
- `cargo test cmd::dev::zone_qi` 全绿

---

## P5 — `/kill` + `/revive`

`/kill self`：emit `PlayerTerminated { entity }`，触发既有死亡管线（life_record / 装备掉落 / qi_zero_decay 窗口）。
`/revive self`：emit `PlayerRevived { entity }`，触发既有重生管线。

**交付物**：

- `server/src/cmd/dev/kill.rs`：`enum KillCmd { Self_ }`，`kill self` brigadier（literal `self` 区分未来可能的 `kill <player_name>`）
- `server/src/cmd/dev/revive.rs`：`enum ReviveCmd { Self_ }`，`revive self` brigadier
- 死亡原因：用 `CultivationDeathCause::DevCommand`（**新增 enum 变体**，不复用既有原因，便于死亡日志区分）
- handler emit event；不直接 mutate health（让 `combat::lifecycle` 既有 system 接管）
- `registry_pin` 加 `"kill"` / `"revive"`，paths 加 `kill self` / `revive self`

**测试**（≥ 6 单测）：
- `/kill self` emit 一次 `PlayerTerminated`
- `/kill self` 在玩家已死状态下 no-op + 提示
- `/revive self` emit 一次 `PlayerRevived`
- `/revive self` 在玩家未死状态下 no-op + 提示
- `CultivationDeathCause::DevCommand` enum variant 存在
- 端到端：`/kill self` → `combat::lifecycle::handle_player_terminated` 把 `Cultivation.realm` 退一阶（worldview §四 死亡惩罚）

**验收**：
- 手测：`/kill self` 后看到死亡画面，`/revive self` 后回到 spawn
- `cargo test cmd::dev::{kill,revive}` 全绿

---

## P6 — `/time advance <ticks>`

`/time advance <ticks>`：把 `CultivationClock.tick += N`，触发依赖 tick 的衰减/cooldown/shelflife 管线。

**交付物**：

- `server/src/cmd/dev/time.rs`：`enum TimeCmd { Advance { ticks: u64 } }`，`time advance <ticks:integer>` brigadier
- handler 写 `CultivationClock.tick += ticks`
- **明确不 advance**：Bevy `Time` resource（影响所有 system 时间步）、real-time 时钟、player session counter——只动 cultivation 时钟
- **明确同步推**（在 plan 实施时 grep 确认）：依赖 `CultivationClock.tick` 的 system（meridian_open_tick / qi_zero_decay / shelflife / cooldown / season transition）
- `tracing::warn!("[dev-cmd] advance cultivation clock by {ticks} ticks: {prev} -> {next}")`
- 上限：单次 `ticks <= 1_000_000`（防止误填爆掉 i64），超限拒绝
- `registry_pin` 加 `"time"`，paths 加 `time advance <ticks:integer>`

**测试**（≥ 6 单测）：
- `/time advance 100` 写入 tick += 100
- `/time advance 0` no-op + 提示
- `/time advance 2_000_000` 超限拒绝
- 端到端：`/time advance 1000` 后 `qi_zero_decay` system 在玩家 qi=0 状态下触发（命中既有阈值常数）
- 端到端：`/time advance N` 后 cooldown 计时器减少（具体 N 视 cooldown 实现）
- `Time` resource 未被 advance（pin 行为：dev cmd 只动 cultivation 时钟）

**验收**：
- 手测：`/qi set 0` + `/time advance <qi_zero_decay_threshold>` 后境界自动跌一阶
- `cargo test cmd::dev::time` 全绿

---

## P7 — 测试饱和 + brigadier 命令树 fixture

按 CLAUDE.md "饱和化测试" 原则补：

- 每条命令 happy / 边界 / 错误分支 / enum variant 全覆盖
- 端到端协议层：mock client 发 `CommandSuggestionsRequest` → server 回包包含全部新命令名
- `registry_pin::COMMAND_NAMES` 排序 + 唯一性测试已存在，新命令必须排序插入
- `registry_pin::COMMAND_TREE_PATHS` 必须含本 plan 所有新增子命令路径
- 每个 `parse_*` helper（`parse_meridian_id` / `parse_realm` / `parse_clear_scope`）≥ 4 单测覆盖大小写、同义词、unknown

**新增命令清单**（按字母排序，对照 fixture）：

```
clearinv
give
kill
meridian
qi
realm
revive
technique
time
zone_qi
```

**新增 paths**（按字母排序，写入 `COMMAND_TREE_PATHS`）：

```
clearinv
clearinv <scope:string>
give <id:string>
give <id:string> <count:integer>
kill self
meridian list
meridian open <id:string>
meridian open_all
qi max <value:double>
qi set <value:double>
realm set <id:string>
revive self
technique active <id:string> <value:bool>
technique add <id:string>
technique list
technique proficiency <id:string> <value:double>
technique remove <id:string>
technique reset_all
time advance <ticks:integer>
zone_qi set <name:string> <value:double>
```

**验收**：
- `cargo test cmd::registry_pin` 全绿
- `cargo test --all-targets cmd::` 全绿（含 P0-P6 全部命令）

---

## P8 — 文档同步

- `CLAUDE.md`：在 "Quick commands" 后加一节 "Dev test commands"，列出本 plan 全部命令的一句话说明（注明 dev-only、绕过 worldview 自然规则）
- `docs/local-test-env.md`（如存在）：补"快速搭测试场景"流程示例（`/realm set void` + `/qi max 1000` + `/qi set 1000` + `/meridian open_all` + `/give bone_coin 1000`）
- 不改 `worldview.md`（本 plan 不引入新世界观规则）
- 不改 `qi_physics` 任何文档（本 plan 显式绕过 ledger，不是 ledger 的一部分）

---

## 跨仓库影响

| 仓库 | 改动 |
|----|----|
| `server/` | 新增 10 个 `src/cmd/dev/*.rs` 文件；`registry_pin` 加 10 个命令 + 20 条 paths；`inventory/mod.rs` 加 `clear_player_inventory()` helper；`combat/events.rs` 加 `CultivationDeathCause::DevCommand` variant |
| `agent/` | 无 |
| `client/` | 无（brigadier 树自动同步；KnownTechniques snapshot 走现有 emit 路径） |
| `worldgen/` | 无 |

## 风险 / 开放点

- **`/time advance` 范围**：当前只动 `CultivationClock`，但实际"测试时间快进"可能还想推 `Time` resource、`SeasonClock` 等。开放：实施时 grep `CultivationClock.tick` 找出所有 reader，确认覆盖面；如发现 `cooldown` 等用 `Time` 而非 `CultivationClock`，要么加 P6.1 子命令 `time advance_all`，要么文档里明确说"cooldown 不受影响"
- **`/kill self` 死亡链路完整性**：依赖 `combat::lifecycle::handle_player_terminated` 处理 PlayerTerminated；如果该 system 假设"死亡来自 combat 上下文"（如读 attacker entity），dev 命令可能要补 `attacker = self` 或新加判断。实施时 P5 第一步先 grep `PlayerTerminated` 的 reader，确认无非空依赖
- **`/give` template 列表**：玩家不知道有哪些 template_id 合法。开放：是否加 `/give list` 子命令列出全部 template？或文档维护一份 template 清单？建议 P3 实施时若 `ItemRegistry::templates.len()` 较小（< 100）则加 `/give list`
- **`/technique add` 后 client 端 UI**：客户端 HUD 招式栏是否能动态加新条？依赖 `network::techniques_snapshot_emit` 已有的 `Changed<KnownTechniques>` 触发；如果 client 端 UI 只在登录时初始化 slot 数量，可能要先重连才能看到新招——这是 client 端 plan 的范围，不在本 plan 内修，但 P2 文档要标注
- **`CultivationDeathCause::DevCommand` 是否进 LifeRecord biography**：dev 死亡是否记入生平？建议**记入但加标记**（如 `BiographyEntry::Death { cause: DevCommand }`），便于排查"这玩家怎么死了三次还在玩"
- **多人服 / 安全**：本 plan 全部命令对**任何**玩家都开放（与 `/health set` 一样）。生产环境应通过 `/op` permission gate；本 plan 不做权限层（与现有 dev 命令一致），但**文档必须警告**：dev build only

## Finish Evidence

### 落地清单

- P0 `/meridian`：`server/src/cmd/dev/meridian.rs`，注册到 `server/src/cmd/dev/mod.rs`，覆盖 20 条经脉解析、强制 open、open_all、list、LifeRecord / spirit_root_first 写入。
- P1 `/realm` + `/qi`：`server/src/cmd/dev/realm.rs`、`server/src/cmd/dev/qi.rs`，直接 mutation `Cultivation.realm` / `qi_current` / `qi_max`，并 pin 不发 `BreakthroughEvent` / `QiTransfer`。
- P2 `/technique`：`server/src/cmd/dev/technique.rs`，覆盖 list / add / remove / proficiency / active / reset_all，unknown add/proficiency/active 拒绝，reset 回 `KnownTechniques::default()`。
- P3 `/give` + `/clearinv`：`server/src/cmd/dev/give.rs`、`server/src/cmd/dev/clearinv.rs`、`server/src/inventory/mod.rs`，新增 `ClearScope` 与 `clear_player_inventory()` helper。
- P4 `/zone_qi set`：`server/src/cmd/dev/zone_qi.rs`，直写 `ZoneRegistry` 中的 `Zone.spirit_qi`，允许负值并 pin 不发 `QiTransfer`。
- P5 `/kill self` + `/revive self`：`server/src/cmd/dev/kill.rs`、`server/src/cmd/dev/revive.rs`，接入 `PlayerTerminated` / `PlayerRevived`，并在 `server/src/combat/lifecycle.rs`、`server/src/cultivation/death_hooks.rs` 支持 `CultivationDeathCause::DevCommand`。
- P6 `/time advance`：`server/src/cmd/dev/time.rs`，只推进 `CultivationClock.tick`，不推进 Bevy `Time` resource。
- P7 registry pin：`server/src/cmd/registry_pin.rs`、`server/src/cmd/mod.rs`，同步 10 个根命令与 20 条 executable paths。
- P8 文档：`CLAUDE.md` 增加 "Dev test commands"，`docs/local-test-env.md` 增加快速搭测试场景示例（P8 明确要求，且执行中经用户明确授权后提交；不是默认 `/consume-plan` docs 白名单写入）。

### 关键 commit

- `a403217c7`（2026-05-10）`feat(dev-cmd): 增加测试场景命令`
- `2fe3848f3`（2026-05-11）`fix(dev-cmd): 跟随默认功法数量校验 technique 测试`
- `4c2771e80`（2026-05-11）`docs(dev-cmd): 补测试场景命令说明`
- `5809b1b7`（2026-05-11）`fix(dev-cmd): 采纳 review 补死亡原因与 pin 测试`
- `dec87a94`（2026-05-11）`fix(dev-cmd): 收紧 review 指出的边界输入测试`
- `61f1ac01`（2026-05-11）`fix(dev-cmd): 补 dev kill 终结持久化`
- `767575ef`（2026-05-11）`fix(dev-cmd): 拒绝缺持久化上下文的 dev kill`

### 测试结果

- `cd server && cargo fmt --check`：通过。
- `cd server && cargo clippy --all-targets -- -D warnings`：通过。
- `cd server && cargo test`：`4249 passed; 0 failed; 0 ignored`。
- `cd server && cargo test cmd::dev`：`96 passed; 0 failed`。
- `cd server && cargo test inventory::tests::clear_player_inventory`：`3 passed; 0 failed`。
- `cd server && cargo test cmd::dev::technique`：`6 passed; 0 failed`（rebase 后默认功法数量漂移回归；review 后补 non-finite proficiency 回归）。
- `cd server && cargo test cmd::dev::kill`：`4 passed; 0 failed`（review 后补 LifeRecord / terminated snapshot 持久化回归；缺持久化上下文拒绝）。
- `cd server && cargo test cmd::tests::command_registry && cargo test cmd::tests::command_tree_packet_contains_pinned_root_literals`：`4 passed; 0 failed`（确认可选 persistence resource 不破坏命令树 fixture）。

### 跨仓库核验

- server：命中 `server/src/cmd/dev/{meridian,realm,qi,technique,give,clearinv,zone_qi,kill,revive,time}.rs`、`CommandTreePath`、`ClearScope`、`clear_player_inventory()`、`CultivationDeathCause::DevCommand`。
- agent：无改动；dev 命令不通过 Redis IPC。
- client：无改动；命令树通过 Valence brigadier 协议下发，`KnownTechniques` snapshot 沿用现有 `network::techniques_snapshot_emit`。
- worldgen：无改动。

### 遗留 / 后续

- `/time advance` 按本 plan 范围只推进 `CultivationClock`；如未来需要推进 Bevy `Time`、SeasonClock 或全局 cooldown，应另起 plan 定义 `advance_all` 类命令。
- dev 命令沿用现有 dev 命令权限面；生产服 op/permission gate 不在本 plan 范围。
