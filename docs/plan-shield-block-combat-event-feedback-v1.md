# plan-shield-block-combat-event-feedback-v1

> 一句话主题：盾格挡命中时，server 已把 `combat_event.kind` 发成 `shield_block`，但 client `CombatEventHandler` 仍把它落到默认 `HIT` 分支，导致玩家正常举盾格挡后的**数值飘字反馈按普通受击红字显示**，与专用盾格挡视听反馈链（`shield_block_hit` / `CombatJuiceEvent.Kind.SHIELD_BLOCK`）脱节。**收口日期 2026-07-04，骨架 → active。**

> 去重说明：**不重复** `plan-bughunt-r10-findings-v1` 的"破盾后 `ShieldBlock` / `ShieldBlocking` 残留"题。那一题是 **server ECS 状态泄漏**；本题是 **server→client `combat_event` kind 分类漏接**，即使盾未破也可稳定复现。

## 阶段总览

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | `shield_block` 飘字分类断链（client `parseKind`/`defaultColorFor` 加 `shield_block` 分支） | fix_pr | ⬜（决议见 §8.1，2026-07-04 收口，待 `/consume-plan` 实施） |

## 接入面（docs/CLAUDE.md §二 六要素）

- **进料**：server `combat_event_emit.rs::wire_kind()`（`server/src/network/combat_event_emit.rs:47-55`）已把 `DefenseKind::ShieldBlock` 编成 wire 字符串 `"shield_block"`，随 `combat_event` payload 的 `events[].kind` 字段下发；client `CombatEventHandler.handle()`（`client/src/main/java/com/bong/client/combat/handler/CombatEventHandler.java:36-86`）逐条解析该字段。这条进料链路已通，本 plan 不改 server、不改 wire 格式。
- **出料**：`CombatEventHandler.parseKind()` 产出的 `DamageFloaterStore.Kind` 连同 `defaultColorFor()` 产出的颜色一起打包进 `DamageFloaterStore.Floater`（`client/src/main/java/com/bong/client/combat/store/DamageFloaterStore.java:23-36`），经 `DamageFloaterStore.publish()` 存入环形队列，最终由 `DamageFloaterHudPlanner.buildCommands()`（`client/src/main/java/com/bong/client/hud/DamageFloaterHudPlanner.java:23-53`）读出渲染成屏幕飘字。本 plan 只改前半段分类/上色，不改 `DamageFloaterHudPlanner` 的渲染逻辑（复用 `Kind.BLOCK` 不新增枚举，见 §8.1 决议，`DamageFloaterHudPlanner` 无需改动）。
- **共享类型 / event**：复用既有 `DamageFloaterStore.Kind` 枚举（`HIT, CRIT, BLOCK, HEAL, QI_DAMAGE`，`DamageFloaterStore.java:19-21`），**不新增枚举变体**——`shield_block` 归类到已有 `BLOCK`（语义上两者都是"格挡"，避免为一次 wire 分类扩大下游 `DamageFloaterHudPlanner` / 未来任何 `switch(Kind)` 的分支面）。颜色层面单独给 `shield_block` 一个专属 hex（与 `"block"` 的灰色区分），复用 `defaultColorFor()` 现有 switch 结构，不新建颜色表。同一文件里 `toJuiceEvent()`/`juiceKind()`（`CombatEventHandler.java:136-177`）已有的 `case "shield_block" -> CombatJuiceEvent.Kind.SHIELD_BLOCK` 分支不受影响、不改动——那条链路本来就是对的，本 plan 只补齐飘字这条姊妹链路。
- **跨仓库契约**：纯 client 内部修复，不新增/改动 IPC schema、Redis key 或 `CustomPayload` type ID。`combat_event` wire 格式（`kind` 字段允许值集合）不变，只是 client 侧对既有合法值 `"shield_block"` 补上正确分类，server 侧 `wire_kind()`（已发出 `"shield_block"`，见 `combat_event_emit.rs:52`）不用改。
- **worldview 锚点**：本 plan 不涉及境界 / 经济 / 传承数值，是纯战斗反馈 UI 分类修复，无需 worldview 章节锚点（对齐 docs/CLAUDE.md §二"纯 server 逻辑 / 纯 UI 修复无 worldview 要求"的豁免范畴——但需注明：此修复保障的是"盾格挡"这一**已落地**玩法（`plan-shield-block-v1`）的正确视听闭环，不是新增玩法）。
- **qi_physics 锚点**：不涉及真元 / 灵气流转、无衰减常数、无 `QiTransfer`。纯客户端视觉分类 + 配色修复，N/A。

## P0 — `shield_block` 飘字分类断链

### 问题现状（已用 grep 核验，2026-07-04，origin/main HEAD `306660964`）

- **#1 major（fix_pr）**：`server/src/network/combat_event_emit.rs:47-55` 的 `wire_kind()` 明确把 `DefenseKind::ShieldBlock` 编成 `kind="shield_block"`；`server/src/combat/resolve.rs:8947-9072`（测试 `shield_block_front_face_reduces_severity_bleeding_and_contam`）的前方举盾 happy-path 测试证明这条分支在**正常游玩可达**（正面命中 + `ShieldBlocking` 状态 → `defense_kind=ShieldBlock`），且格挡后 `physical_damage` 仍保留残值（断言 `phys_dmg < 0.5` 而非 `== 0`，见 `resolve.rs:9058-9063`）——不是"永远 0 伤所以看不到飘字"的死路。
- 但 `client/src/main/java/com/bong/client/combat/handler/CombatEventHandler.java` 的 `parseKind()`（88-97 行）与 `defaultColorFor()`（103-111 行）**只识别** `crit/block/heal/qi_damage`；`shield_block` 落入两个 switch 的 `default` 分支，分别产出 `DamageFloaterStore.Kind.HIT` 和颜色 `0xFFE04040`（普通受击红），与 `"hit"` wire 的表现完全同构。
- 同一文件的 `toJuiceEvent()` → `juiceKind()`（136-177 行，尤其 169 行 `case "shield_block" -> CombatJuiceEvent.Kind.SHIELD_BLOCK`）却**显式**把 `shield_block` 映射到独立 juice kind。这证明 client 契约层面已经承认 `shield_block` 是独立分类，唯独数值飘字这半条链没跟上——两处分类逻辑出现语义分叉。
- `client/src/main/java/com/bong/client/network/ShieldBlockHitHandler.java`（19-109 行，实地核验为 `shield_block_hit` payload 专用 handler）只补粒子 / 音效 / toast / HUD 瞬态盾弧，**不携带伤害数值**，不能替代 `combat_event` 的"本次被挡后实际掉了多少"数值反馈。
- **实际影响**：玩家正常举盾、正面吃到一击并成功触发盾格挡时，屏幕上仍会出现与普通挨打几乎同构的红色伤害飘字；玩家无法从数值飘字层分辨"这次是盾格挡后的残余伤害"还是"完全没挡到的普通受击"，主战斗反馈被误导。

### 交付物（可核验）

1. **`client/src/main/java/com/bong/client/combat/handler/CombatEventHandler.java`**
   - `parseKind()`（当前 88-97 行）新增一条 `case "shield_block" -> DamageFloaterStore.Kind.BLOCK;`（决议见 §8.1 #1，复用既有 `BLOCK` 枚举，不新增变体）。
   - `defaultColorFor()`（当前 103-111 行）新增一条 `case "shield_block" -> 0xFF6FA8DC;`（决议见 §8.1 #1，专属"盾蓝"色值，区别于 `"block"` 的灰色 `0xFFA0A0A0`、`"qi_damage"` 的浅蓝 `0xFF80A0FF`、`"crit"` 的琥珀 `0xFFFFC040`、默认 `HIT` 的红 `0xFFE04040`）。
   - 两处改动均为 switch 语句新增一个 `case`，不改变既有 `case` 行为、不改函数签名、不影响 `toJuiceEvent()`/`juiceKind()` 既有的 `shield_block` 分支。
2. **测试**：`client/src/test/java/com/bong/client/combat/handler/CombatHandlersTest.java` 新增专属 pin 测试（该文件当前对 `CombatEventHandler` 只有 `combatEventHandlerAcceptsEvents` / `combatEventHandlerRejectsWhenNoArray` 两条泛化测试，未覆盖任何 `kind` 的分类/配色断言，是真实测试缺口）：
   - `combatEventHandlerShieldBlockKindNotDefaultHit()`：以 `{"kind":"shield_block","amount":8}` 构造 envelope → `new CombatEventHandler().handle(...)` → 断言 `DamageFloaterStore.snapshot(now)` 中该条目 `.kind() == DamageFloaterStore.Kind.BLOCK`（**不是** `Kind.HIT`，失败信息需写明"回归到 default 分支会导致此断言失败"）。
   - 同一测试内断言该条目 `.color() == 0xFF6FA8DC`，且 `!= 0xFFE04040`（默认 HIT 红）、`!= 0xFFA0A0A0`（`"block"` 的灰色，验证"专属颜色"确实专属而非误接到 `BLOCK` 分支的颜色）。
   - 追加一条对照用例：同一测试或独立用例内以 `{"kind":"block","amount":5}` 走一遍，断言其 `.color() == 0xFFA0A0A0`，与 `shield_block` 的 `0xFF6FA8DC` 形成显式对比锚点，防止未来重构把两者配色改混。
   - 回归锚点（不新增断言，仅注释指向）：`toJuiceEvent()` 对 `shield_block` 的 `CombatJuiceEvent.Kind.SHIELD_BLOCK` 分类已有既存覆盖路径（`CombatJuiceTest` 等），本 plan 不重复测，只保证飘字这半条链补齐。

### 验收标准

- `cd client && ./gradlew test` 全绿，新增的 `combatEventHandlerShieldBlockKindNotDefaultHit` 等用例可复现撞红（临时还原 `default` 分支验证测试确实锁住行为）后再提交修复版本。
- 无 server 改动、无 schema 改动，`cargo test`（server）不受影响。

## §8 开放问题（骨架遗留，已在 §8.1 收口）

1. `shield_block` 飘字是否直接复用现有 `BLOCK` 视觉语义，还是要在 `DamageFloaterStore.Kind` 新增独立 `SHIELD_BLOCK`？
2. 是否要顺手补一条"server `wire_kind("shield_block")` → client `CombatEventHandler` 不得 default HIT"的跨端对拍测试，防以后再漏新 `kind`？

全部已在 §8.1 收口。原表保留以备追溯，**实施时以 §8.1 决议为准**。

## §8.1 决议（pre-P0 收口，2026-07-04）

### #1 `shield_block` 飘字复用 `BLOCK` 分类还是新增独立枚举？

**决议**：
1. **复用既有 `DamageFloaterStore.Kind.BLOCK` 枚举，不新增 `SHIELD_BLOCK` 变体**——`parseKind()` 新增 `case "shield_block" -> DamageFloaterStore.Kind.BLOCK`。理由：`Kind` 枚举当前仅在两处被 `switch`/`==` 消费——`DamageFloaterHudPlanner.buildCommands()`（`client/src/main/java/com/bong/client/hud/DamageFloaterHudPlanner.java:46-47`，只对 `CRIT`/`HEAL` 做文本前后缀处理）和 `DamageFloaterStore.Floater` 构造器的 null 兜底（`DamageFloaterStore.java:34`）。新增枚举变体不会带来任何额外渲染差异化收益（`DamageFloaterHudPlanner` 目前对 `BLOCK` 无特殊处理，加了 `SHIELD_BLOCK` 也一样无特殊处理），反而会在未来任何新增的 `switch(Kind)` 处多出一个必须覆盖的分支——扩大不必要的维护面。真正需要视觉区分的是**颜色**，不是分类语义（两者本质都是"命中被格挡，未完全免伤"）。
2. **颜色层面单独区分**：`defaultColorFor()` 新增 `case "shield_block" -> 0xFF6FA8DC`（盾蓝，argb），与 `"block"` 的 `0xFFA0A0A0`（灰）在视觉上明确可辨——满足"格挡语义非普通命中"的核心诉求（都不走默认 HIT 红），同时保留"盾格挡"与"截脉格挡"两种不同来源在色相上的专属区分，不需要靠新枚举达成。
3. **拒绝的路线**：直接复用 `"block"` 的灰色（即两者颜色也合并）——拒绝理由：`toJuiceEvent()` 已经证明 client 契约层面视 `shield_block` 为独立事件类别（169 行显式路由 `SHIELD_BLOCK` juice kind，注释明说"不复用 PARRY，其音效硬编码剑击声"），若飘字颜色再合并回 `"block"` 的灰色，会让"盾格挡"这一已落地玩法（`plan-shield-block-v1`）的视听反馈在飘字层面重新退化为不可辨识，与 juice 层的既有差异化决策自相矛盾。

**落点**：
- `client/src/main/java/com/bong/client/combat/handler/CombatEventHandler.java:88-97`（`parseKind()`，加 `shield_block` → `Kind.BLOCK` 分支）
- `client/src/main/java/com/bong/client/combat/handler/CombatEventHandler.java:103-111`（`defaultColorFor()`，加 `shield_block` → `0xFF6FA8DC` 分支）
- plan §P0 "交付物" 第 1 条

### #2 是否补跨端对拍测试防未来再漏新 `kind`？

**决议**：
1. **本 plan P0 范围内不做**——跨端对拍测试（server `wire_kind()` 输出的字符串全集 ↔ client `parseKind()`/`defaultColorFor()` switch 覆盖的字符串全集）是一个通用护栏机制，价值独立于本次修复，且需要新增测试基础设施（例如一份双端共享的"合法 kind 字符串"清单，或 server 侧导出全部 `wire_kind()` 可能返回值供 client 测试读取），工作量与本 plan"补一个 switch 分支"的 P0 体量不对等，会让本已很小的 fix_pr 膨胀成基建改动。
2. **不升级为本 plan 的 P1**，也不阻塞本 plan 归档——本 plan §P0 的专属 pin 测试（`combatEventHandlerShieldBlockKindNotDefaultHit`）已经锁住 `shield_block` 这一具体 kind 不再回归 default，满足当前唯一已知红旗的诉求。
3. **登记为独立后续项**：若未来 `server/src/combat/resolve.rs` 或其他模块的 `DefenseKind`/`CombatEvent.defense_kind` 变体增多，`combat_event_emit.rs::wire_kind()` 每新增一条 wire 字符串分支，都应在 `CombatEventHandler.parseKind()`/`defaultColorFor()` 同步补分支——这条规则记入本 plan"遗留 / 后续"，供下一个碰到同类断链的 plan（或 bughunt 轮次）参考，不在本 plan 内建立自动化护栏。

**落点**：
- `server/src/network/combat_event_emit.rs:47-55`（`wire_kind()`，未来新增 `DefenseKind` 分支时的同步点）
- `client/src/main/java/com/bong/client/combat/handler/CombatEventHandler.java:88-97,103-111`（未来新增分支的同步点）
- plan "遗留 / 后续" 章节（见下）

## 遗留 / 后续

- §8.1 #2 提到的跨端对拍护栏机制不在本 plan 范围，留给未来碰到同类断链时的 plan 或 bughunt 轮次处理。
- `DamageFloaterHudPlanner` 目前对 `BLOCK`（含新接入的 `shield_block`）无任何文本前缀/后缀处理（不同于 `CRIT`/`HEAL`）；若未来想让盾格挡飘字额外带图标/前缀，需要新开 plan 评估，不在本次"分类断链修复"范围内。

## 审计来源

bughunt loop 20260704-i。两轮怀疑式证伪后保留：

1. 反证一：这不是"已有 `shield_block_hit` 专用通道，所以 `combat_event` 怎么分都无所谓"——专用通道只补视听，不补数值飘字。
2. 反证二：这不是不可达死码——`resolve.rs` happy-path 测试已锁住正常前方举盾会真实产出 `DefenseKind::ShieldBlock`，且仍可能存在残余 `physical_damage`，因此玩家常规战斗可见。
