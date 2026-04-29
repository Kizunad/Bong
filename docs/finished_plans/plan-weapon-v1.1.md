# Bong · plan-weapon-v1.1

**武器 v1.1 补完专项**。本 plan 不重做 `plan-weapon-v1` 已合并的主链路，只补齐 v1 审核后发现的 Evidence 与验收缺口：schema 单一来源、武器 channel 语义、铁剑伤害验收、装备持久化、资源路径契约、以及可复核证据。

**来源**：`plan-weapon-v1` PR #41 合并后核对结论。

**落位**：
- agent：`agent/packages/schema/src/server-data.ts`、`schema-registry.ts`、`generated/*.json`、samples
- server：`server/src/schema/*`、`server/src/network/*`、`server/src/inventory/*`、`server/src/combat/*`、`server/assets/items/weapons.toml`
- client：`client/src/main/java/com/bong/client/weapon/`、`client/src/main/resources/assets/{bong,minecraft}/models/item/`
- evidence：`.sisyphus/evidence/weapon-v1.1/`

**交叉引用**：
- 前置：`docs/plan-weapon-v1.md`
- 依赖：`docs/plan-inventory-v1.md`
- 协作：`docs/plan-HUD-v1.md`、`docs/plan-forge-v1.md`
- 范围外：Treasure 展开 Entity 仍留 `plan-treasure-v1`

---

## §0 范围与不变量

### 0.1 必须补齐

1. TS schema 与 Rust/client 已实现的 weapon server-data payload 对齐。
2. 明确 `bong:combat/*` 独立 channel 与当前 `bong:server_data` 总线的取舍，并让文档、schema、代码一致。
3. 让“铁剑对比赤手：伤害 ×1.2 以上”成为真实可测验收，或修改验收口径并给出证据。
4. 证明装备状态跨会话保留，不再只依赖默认 loadout。
5. 固化武器资源路径契约，消除 `assets/bong/models/item/<template>.json` 与当前 vanilla host override 的歧义。
6. 留下可复核 Evidence artifacts，禁止只在 PR body 写“跑过”。

### 0.2 不做

- 不新增武器技能。
- 不做 Bow 远程弹药。
- 不做 Treasure 飞出 / Entity 展开。
- 不重构整个 inventory 持久化系统；只补武器装备跨会话所需的最小闭环。
- 不为了过测试改断言、skip 测试或注释掉测试。

### 0.3 Evidence 规则

所有任务完成时必须写入：

```text
.sisyphus/evidence/weapon-v1.1/task-{N}-{slug}.log
.sisyphus/evidence/weapon-v1.1/task-{N}-{slug}.txt
```

Evidence 文件只记录命令输出、关键 diff 摘要、PR/CI URL、截图路径或人工视觉验证说明。不写入 `docs/`。

---

## §1 核对基线

### 1.1 已确认 v1 已合并

- PR：`https://github.com/Kizunad/Bong/pull/41`
- 状态：merged
- 已有提交：`46c5fdd1`、`e92d63e4`、`80f7616a`、`63ec9f07`、`98fc144c`、`a7c28e4b`

### 1.2 v1 缺口列表

| 缺口 | 现状 | v1.1 目标 |
|---|---|---|
| TS server-data schema 缺 weapon payload | Rust/client 有 `weapon_equipped` 等，TS schema 没有 | TS/Rust/client/generated 全一致 |
| channel 语义不一致 | plan 写 `bong:combat/*`，实现走 `bong:server_data` | 统一为一个明确契约 |
| 铁剑 ×1.2 验收不成立 | `base_attack=8.0` 且公式 floor 到 1.0 | 测试铁剑真实倍率 ≥ 1.2 |
| 跨会话装备保留证据不足 | 默认 loadout 有剑，但没有装备变更持久化证据 | 装备/卸下后重载仍一致 |
| 资源路径契约偏离 | 实现走 vanilla host model override | 文档化并测试 registry/resource 可用 |
| Evidence 缺失 | 无 `.sisyphus/evidence/weapon-v1` | v1.1 每 task 留 log |

---

## §2 Task 1：Schema 单一来源补齐

**What to do**：补齐 `agent/packages/schema/src/server-data.ts` 中的 weapon/treasure server-data payload，确保 TS source、generated JSON、Rust mirror、client handler 四方一致。

**要求**：
- 新增 `WeaponViewV1`、`WeaponEquippedV1`、`WeaponBrokenV1`、`TreasureViewV1`、`TreasureEquippedV1`。
- `ServerDataType` union 加入：`weapon_equipped`、`weapon_broken`、`treasure_equipped`。
- `ServerDataV1` union 加入三类 payload。
- `schema-registry.ts` 暴露单项 schema，generated 输出包含对应字段。
- samples 至少包含：装备铁剑、清空 main_hand、weapon_broken、treasure_belt_0 法宝装备。
- Rust `server/src/schema/combat_hud.rs` 与 `server/src/schema/server_data.rs` 不得和 TS 字段漂移。

**Acceptance**：
- `agent/packages/schema/generated/server-data-v1.json` 能 grep 到 `weapon_equipped`、`weapon_broken`、`treasure_equipped`。
- TS 校验能接受 weapon payload 样本并拒绝未知字段。
- Rust serde roundtrip 测试仍通过。
- Client router 对新 payload 类型有 handler。

**Validation**：
```bash
cd agent/packages/schema && npm test && npm run check && npm run generate
cd server && cargo test schema::combat_hud schema::server_data -- --nocapture
cd client && JAVA_HOME="/usr/lib/jvm/java-17-openjdk-amd64" ./gradlew test --tests "com.bong.client.network.*Weapon*" --tests "com.bong.client.network.*Treasure*" --tests "com.bong.client.network.ServerDataRouterTest"
```

**Evidence**：
- `.sisyphus/evidence/weapon-v1.1/task-1-schema.log`
- `.sisyphus/evidence/weapon-v1.1/task-1-schema-grep.txt`

---

## §3 Task 2：Channel 契约收口

**Decision**：v1.1 默认采用 **统一 `bong:server_data` channel + payload type 分发**，不新增独立 `bong:combat/*` CustomPayload channel。

**理由**：当前 client router、server serializer、payload size guard、日志体系都以 `bong:server_data` 为统一 S2C 总线；另开三个 channel 会复制路由与限流逻辑，收益低。`bong:combat/*` 保留为逻辑 payload type，不再作为物理 channel。

**What to do**：
- 在代码注释或 schema 注释中明确：weapon 走 `bong:server_data`，`type=weapon_equipped/weapon_broken/treasure_equipped`。
- 删除或修正任何仍声称物理 channel 是 `bong:combat/weapon_equipped` 的注释。
- 添加测试证明发送出的 CustomPayload channel 是 `bong:server_data`，payload type 是 weapon 对应类型。

**Acceptance**：
- server 测试能 decode `CustomPayloadS2c`，channel 为 `bong:server_data`，JSON `type` 为 `weapon_equipped` 或 `weapon_broken`。
- 文档/注释不再要求 v1.1 新增物理 `bong:combat/*` channel。

**Validation**：
```bash
cd server && cargo test weapon_equipped weapon_broken -- --nocapture
```

**Evidence**：
- `.sisyphus/evidence/weapon-v1.1/task-2-channel.log`
- `.sisyphus/evidence/weapon-v1.1/task-2-payload.json`

---

## §4 Task 3：铁剑伤害验收修正

**Problem**：v1 文档要求“铁剑对比赤手：伤害 ×1.2 以上”，但 v1 实现公式为：

```text
weapon_attack_multiplier = max(1.0, base_attack / 10.0)
```

`iron_sword base_attack=8.0` 时满耐久倍率为 `1.0`，半损默认装备倍率更低，无法达成验收。

**Decision**：v1.1 采用数据修正而非改公式：将 `iron_sword.base_attack` 调整到 `12.0`。原因：公式已用于多武器分层，调整全局公式会影响所有武器；铁剑作为“正经起手武器”满足 ×1.2 更贴合验收。

**What to do**：
- 修改 `server/assets/items/weapons.toml` 中 `iron_sword` 的 `base_attack` 为 `12.0`。
- 同步所有测试 fixture 中铁剑 `base_attack` 期望。
- 增加专名测试：`iron_sword_increases_damage_by_at_least_20_percent_vs_unarmed`。
- 测试必须使用真实 `ItemRegistry` 或和 `weapons.toml` 一致的 fixture，不得用 `strong_sword` 替代。
- 额外覆盖半损默认铁剑：若默认 loadout 仍为 `durability=0.5`，则验收测试使用满耐久铁剑；默认半损只用于世界观，不用于“×1.2”验收。

**Acceptance**：
- 满耐久 `iron_sword` 攻击同等目标时，damage >= unarmed damage * 1.2。
- `strong_sword` 旧测试不能作为此验收唯一证据。
- `weapons.toml` 与测试 fixture 值一致。

**Validation**：
```bash
cd server && cargo test iron_sword_increases_damage_by_at_least_20_percent_vs_unarmed -- --nocapture
cd server && cargo test weapon -- --nocapture
```

**Evidence**：
- `.sisyphus/evidence/weapon-v1.1/task-3-iron-sword-damage.log`
- `.sisyphus/evidence/weapon-v1.1/task-3-damage-numbers.txt`

---

## §5 Task 4：装备跨会话持久化闭环

**Problem**：v1 只能证明默认 loadout 自带 `main_hand=iron_sword`，不能证明玩家拖动装备后跨会话保留。

**What to do**：
- 查明当前 player persistence 对 `PlayerInventory` 的支持状态。
- 若已有 inventory persistence：把 `equipped`、`durability`、`hotbar`、containers 的保存/加载接上并补测试。
- 若没有完整 inventory persistence：实现最小武器装备持久化 mirror，至少保存 per-player equipped slots 的 `instance_id/template_id/durability` 与容器位置，保证 main_hand 装备/卸下后重载一致。
- 不改变死亡掉落/修复规则。
- 不将默认 loadout 误当持久化证据。

**Acceptance**：
- 测试 A：玩家把背包铁剑移动到 `main_hand`，触发保存，重建 world/player 后 `main_hand` 仍为同一 `template_id`，durability 保持。
- 测试 B：玩家把 `main_hand` 铁剑拖回背包，触发保存，重建后 `main_hand` 为空，铁剑在容器内。
- 测试 C：武器损坏后 durability=0，重建后仍为 0 且不在 `main_hand`。
- Evidence 必须包含持久化文件/数据库查询或序列化快照片段。

**Validation**：
```bash
cd server && cargo test inventory_persistence weapon_persistence -- --nocapture
cd server && cargo test sync_weapon_component_from_equipped -- --nocapture
```

**Evidence**：
- `.sisyphus/evidence/weapon-v1.1/task-4-persistence.log`
- `.sisyphus/evidence/weapon-v1.1/task-4-persisted-snapshot.json`

---

## §6 Task 5：资源路径契约固化

**Decision**：v1.1 接受当前实现路径：Bong 武器不注册 vanilla Item，而是通过 fake vanilla host item + SML override 渲染 OBJ。

**Canonical contract**：

```text
template_id
  -> BongWeaponModelRegistry.Entry
  -> host vanilla item model: assets/minecraft/models/item/<host>.json
  -> parent: sml:builtin/obj
  -> model: bong:models/item/<template_id>/<template_id>.obj
  -> mtl/png: assets/bong/models/item/<template_id>/<template_id>.mtl + assets/bong/textures/item/<template_id>/...
```

**What to do**：
- 为 7 把 v1 武器补齐 registry entries 与 host model JSON。
- 添加资源存在性测试：每个 registry entry 的 OBJ、MTL、host JSON、贴图目录存在。
- 添加 JSON 解析测试：host JSON `parent == sml:builtin/obj` 且 `model` 指向 registry 中 OBJ。
- 删除或标注旧 placeholder 资源，不让 v1 武器继续依赖 `placeholder_sword` / `wooden_totem`。

**Acceptance**：
- 7 把 v1 武器都能通过资源存在性测试。
- `BongWeaponModelRegistryTest` 覆盖全部 7 把，而不是只抽样。
- 若保留 legacy `rusted_blade`，必须单独标注为 legacy，不计入 v1 七把验收。

**Validation**：
```bash
cd client && JAVA_HOME="/usr/lib/jvm/java-17-openjdk-amd64" ./gradlew test --tests "com.bong.client.weapon.*"
```

**Evidence**：
- `.sisyphus/evidence/weapon-v1.1/task-5-resources.log`
- `.sisyphus/evidence/weapon-v1.1/task-5-resource-manifest.txt`

---

## §7 Task 6：视觉验收证据

**What to do**：补齐“第一人称看到 3D 武器模型 / 无武器看到手 / weapon broken HUD 弹通知”的可复核证据。

**最低实现**：
- 若可运行 `runClient`：生成截图或日志，记录铁剑装备、卸下、破损三场景。
- 若 headless 环境不能截图：使用 `WeaponScreenshotHarness` 或测试 harness 输出 renderer path 证据，证明：
  - equipped iron_sword 时注入 fake stack；
  - unequipped 时不注入 fake stack；
  - SML host model JSON 指向 OBJ；
  - `weapon_broken` handler 产生 toast + `WEAPON_BREAK_FLASH`。

**Acceptance**：
- Evidence 至少包含 3 个场景：armed、unarmed、broken notification。
- 不能只说“代码看起来会渲染”。

**Validation**：
```bash
cd client && JAVA_HOME="/usr/lib/jvm/java-17-openjdk-amd64" ./gradlew test --tests "com.bong.client.weapon.*" --tests "com.bong.client.network.WeaponBrokenHandlerTest"
# 可选人工/WSLg：cd client && JAVA_HOME="/usr/lib/jvm/java-17-openjdk-amd64" ./gradlew runClient
```

**Evidence**：
- `.sisyphus/evidence/weapon-v1.1/task-6-visual.log`
- `.sisyphus/evidence/weapon-v1.1/task-6-armed.txt` 或截图路径
- `.sisyphus/evidence/weapon-v1.1/task-6-unarmed.txt` 或截图路径
- `.sisyphus/evidence/weapon-v1.1/task-6-broken-notice.txt` 或截图路径

---

## §8 Task 7：CI / Evidence 汇总

**What to do**：建立 v1.1 最终核对矩阵，证明上述 task 全部闭合。

**Validation matrix**：

```bash
cd agent/packages/schema && npm test && npm run check && npm run generate
cd server && cargo fmt --check && cargo test
cd client && JAVA_HOME="/usr/lib/jvm/java-17-openjdk-amd64" ./gradlew test build
```

**Clippy policy**：
- 目标命令仍是 `cd server && cargo clippy --all-targets -- -D warnings`。
- 如果 clippy 仍被非 weapon 存量 warning 阻塞，Evidence 必须包含完整日志和 grep 摘要，明确“无新增 weapon 模块 warning”。
- 若 warning 属于本 plan 修改范围，必须修。

**PR body 必须包含**：
- 每个 task 的 Evidence 文件路径。
- 本地验证命令和结果。
- CI URL。
- Review 意见处理摘要。

**Acceptance**：
- `.sisyphus/evidence/weapon-v1.1/README.md` 或 `summary.txt` 汇总全部 Evidence。
- PR 合并前能从 Evidence 反推出每个验收项如何证明。

**Evidence**：
- `.sisyphus/evidence/weapon-v1.1/task-7-final-matrix.log`
- `.sisyphus/evidence/weapon-v1.1/summary.txt`

---

## §9 最终验收标准

- [ ] TS/Rust/client/generated schema 均包含 `weapon_equipped`、`weapon_broken`、`treasure_equipped`。
- [ ] weapon S2C 物理 channel 契约统一为 `bong:server_data`，payload type 区分 weapon 事件。
- [ ] 铁剑满耐久攻击对比赤手伤害 >= 1.2x，有专名测试和实际数字 Evidence。
- [ ] 装备到 `main_hand` 后，`Weapon` component 插入；卸回背包后移除。
- [ ] 装备/卸下/破损后的 inventory 状态跨会话保留，有持久化 Evidence。
- [ ] 原生 hotbar 与 E 背包接管路径仍通过测试。
- [ ] 第一人称 armed/unarmed/weapon broken HUD 通知有可复核 Evidence。
- [ ] 7 把 v1 武器的 TOML、registry、host JSON、OBJ、MTL、贴图资源全部可检测。
- [ ] 所有 Evidence 写入 `.sisyphus/evidence/weapon-v1.1/`。
- [ ] PR body 引用 Evidence 并说明 review 处理结果。

---

## §10 备注

- 本 plan 是 v1 的补完，不要求修改 `docs/plan-weapon-v1.md`。
- 若实际执行中发现 inventory persistence 需要大范围重构，停止并把 Task 4 拆成独立 `plan-inventory-persistence-v1.md`，不要在 v1.1 中扩大范围。
- 若决定改回独立 `bong:combat/*` 物理 channel，必须同步改 server emit、client register、schema、测试和 Evidence；不能只改注释。

---

## Finish Evidence

### 落地清单

- Task 1 Schema 单一来源：`agent/packages/schema/src/server-data.ts`、`agent/packages/schema/src/schema-registry.ts`、`agent/packages/schema/generated/server-data-v1.json`、`agent/packages/schema/generated/server-data-weapon-equipped-v1.json`、`agent/packages/schema/generated/server-data-weapon-broken-v1.json`、`agent/packages/schema/generated/server-data-treasure-equipped-v1.json`、`agent/packages/schema/samples/server-data.weapon-equipped.sample.json`、`agent/packages/schema/samples/server-data.weapon-equipped-empty.sample.json`、`agent/packages/schema/samples/server-data.weapon-broken.sample.json`、`agent/packages/schema/samples/server-data.treasure-equipped.sample.json`。
- Task 2 Channel 契约：`server/src/network/weapon_equipped_emit.rs`、`server/src/network/treasure_equipped_emit.rs`、`server/src/network/agent_bridge.rs`，物理 channel 固定为 `bong:server_data`，通过 JSON `type` 分发 `weapon_equipped` / `weapon_broken` / `treasure_equipped`。
- Task 3 铁剑伤害验收：`server/assets/items/weapons.toml`、`server/src/combat/weapon.rs`、`server/src/combat/resolve.rs`，`iron_sword.base_attack=12.0`，专名测试覆盖满耐久铁剑对赤手倍率。
- Task 4 装备持久化：`server/src/player/state.rs`，覆盖装备到 `main_hand`、卸回背包、破损后 `durability=0.0` 三类 SQLite player slice 保存/加载闭环。
- Task 5 资源路径契约：`client/src/main/java/com/bong/client/weapon/BongWeaponModelRegistry.java`、`client/src/main/java/com/bong/client/weapon/WeaponRenderBootstrap.java`、`client/src/test/java/com/bong/client/weapon/BongWeaponModelRegistryTest.java`，七把 v1 武器 registry、host JSON、OBJ、MTL、贴图目录均有测试。
- Task 6 视觉验收证据：`client/src/test/java/com/bong/client/weapon/WeaponVisualEvidenceHarnessTest.java`、`client/src/test/java/com/bong/client/network/WeaponBrokenHandlerTest.java`，覆盖 armed / unarmed / broken notification 三场景。
- Task 7 Evidence 汇总：`.sisyphus/evidence/weapon-v1.1/summary.txt`、`.sisyphus/evidence/weapon-v1.1/task-7-final-matrix.log`。

### 关键 commit

- `ac9ebb3a`（2026-04-28）`docs(plan): 新增 weapon-v1.1 补完计划`
- `8d22922f`（2026-04-28）`feat(schema): 补齐武器装备推送契约`
- `5640a63b`（2026-04-28）`test(server): 固化武器推送 channel 契约`
- `f0e658ac`（2026-04-28）`fix(server): 提升铁剑伤害验收倍率`
- `c91177c4`（2026-04-28）`test(server): 固化武器装备持久化`
- `3cac6240`（2026-04-28）`test(client): 固化武器资源路径契约`
- `c6823544`（2026-04-28）`test(client): 补齐武器视觉验收证据`
- `57004062`（2026-04-28）`test: 汇总 weapon-v1.1 验证证据`
- 合并 PR：`a0ed60fe`（2026-04-27）`Merge pull request #69 from Kizunad/auto/plan-weapon-v1.1`

### 测试结果

- `cd agent/packages/schema && npm test && npm run check && npm run generate`：PASS；`7` 个 test files、`175` tests passed，`154` schemas exported。
- `cd server && cargo fmt --check && cargo test`：PASS；`1567` tests passed。
- `cd server && cargo clippy --all-targets -- -D warnings`：PASS；无 weapon-v1.1 clippy 例外。
- `cd client && JAVA_HOME="/usr/lib/jvm/java-17-openjdk-amd64" ./gradlew test build`：PASS；`BUILD SUCCESSFUL`。
- 专项命令与原始输出见 `.sisyphus/evidence/weapon-v1.1/task-1-schema.log`、`task-2-channel.log`、`task-3-iron-sword-damage.log`、`task-4-persistence.log`、`task-5-resources.log`、`task-6-visual.log`、`task-7-final-matrix.log`。

### 跨仓库核验

- server symbols：`ServerDataPayloadWireV1::WeaponEquipped`、`ServerDataPayloadWireV1::WeaponBroken`、`ServerDataPayloadWireV1::TreasureEquipped`、`emit_weapon_equipped_payloads`、`emit_weapon_broken_payloads`、`emit_treasure_equipped_payloads`、`iron_sword_increases_damage_by_at_least_20_percent_vs_unarmed`。
- agent symbols：`WeaponEquippedV1`、`WeaponBrokenV1`、`TreasureEquippedV1`、`ServerDataWeaponEquippedV1`、`ServerDataWeaponBrokenV1`、`ServerDataTreasureEquippedV1`、`SCHEMA_REGISTRY.serverDataWeaponEquippedV1`。
- client symbols：`ServerDataRouterTest`、`WeaponBrokenHandlerTest`、`BongWeaponModelRegistry.V1_WEAPON_TEMPLATE_IDS`、`BongWeaponModelRegistryTest`、`WeaponVisualEvidenceHarnessTest`。

### Evidence artifacts

- `.sisyphus/evidence/weapon-v1.1/task-1-schema.log`
- `.sisyphus/evidence/weapon-v1.1/task-1-schema-grep.txt`
- `.sisyphus/evidence/weapon-v1.1/task-2-channel.log`
- `.sisyphus/evidence/weapon-v1.1/task-2-payload.json`
- `.sisyphus/evidence/weapon-v1.1/task-3-iron-sword-damage.log`
- `.sisyphus/evidence/weapon-v1.1/task-3-damage-numbers.txt`
- `.sisyphus/evidence/weapon-v1.1/task-4-persistence.log`
- `.sisyphus/evidence/weapon-v1.1/task-4-persisted-snapshot.json`
- `.sisyphus/evidence/weapon-v1.1/task-5-resources.log`
- `.sisyphus/evidence/weapon-v1.1/task-5-resource-manifest.txt`
- `.sisyphus/evidence/weapon-v1.1/task-6-visual.log`
- `.sisyphus/evidence/weapon-v1.1/task-6-armed.txt`
- `.sisyphus/evidence/weapon-v1.1/task-6-unarmed.txt`
- `.sisyphus/evidence/weapon-v1.1/task-6-broken-notice.txt`
- `.sisyphus/evidence/weapon-v1.1/task-7-final-matrix.log`
- `.sisyphus/evidence/weapon-v1.1/summary.txt`

### 遗留 / 后续

- Treasure 展开 Entity、Bow 远程弹药、武器技能不在本 plan 范围，继续留给后续专项。
- `rusted_blade` 仍作为 legacy registry entry 保留，不计入 v1 七把武器验收。
