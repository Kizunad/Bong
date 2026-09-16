# plan-creature-wiring-v1

> **一句话主题**：把六个已经有 modelScript 建模包、但尚未完整接入妖兽运行时与客户端视觉契约的生物接上线；从 `BeastKind` / spawn / 掉落 / 威胁谱系进料，向 server `EntityKind` 与 client `FaunaVisualKind`、GeckoLib geo/animation/texture 出料，并以 `qi_physics` 守恒链约束缝合兽分裂。本文是骨架，实施前仍须按开放问题逐项人工收口。

## 范围与硬边界

本 plan 覆盖六个生物包：拟态蜘蛛、可可达鹅、马、腐羽鹫、呆怒狮、缝合兽。强度使用调度主干在 2026-09-16 定稿的 realm band，不在本骨架中重定数值：新增物种全部落在 `realm_tier` 0–1（醒灵～引气）与 `health_max` 8–35；拟态蜘蛛只复用既有 `BeastKind::Spider`，不新增变体、不改数值。

本 plan 是视觉与生物接入的骨架，不是一次性把六个包塞进一个 PR。纯文档阶段不修改 `.rs`、`.java`、`.json`、资源或 modelScript；转 active 后按 §10 拆成多个依赖有序 PR。马的骑乘、战利品箱、`HybridBeast` 既有融合/狂暴机制、以及任何新的 qi 物理常数均不在本 plan 的默认范围内。

## 阶段总览

| 阶段 | 主题 | 主要交付物 | 状态 |
|---|---|---|---|
| P0 | 拟态蜘蛛补视觉 | 自有 geo/texture/animation 资源、server/client 视觉映射、spawn 不再借用 `ASH_SPIDER_ENTITY_KIND`；零数值改动 | ⬜ |
| P1 | 醒灵档两只 | `KekedaGoose`（12）与 `Horse`（20）的 BeastKind、body-plan 注册、spawn、资源与反抗链；骑乘留后续 | ⬜ |
| P2 | 引气档两只 | `FuyuVulture`（24，飞行）与 `DainuLion`（32，捕食者）的完整注册、移动/威胁/视觉契约 | ⬜ |
| P3 | 缝合兽分裂 | `StitchedBeast`（35）本体、shard（每个 8.0）及分裂/死亡的 `QiTransfer` 守恒链；不与旧 `HybridBeast` 融合语义混淆 | ⬜ |
| P4 | 威胁接入与回归 | ambient spawn 池、威胁预算边界、掉落/生命周期/跨端 e2e 与资源包 manifest 回归 | ⬜ |

### 强度定稿表（只读输入）

| 生物 | server `BeastKind` | `realm_tier` | `health_max` | 运行时定位 |
|---|---|---:|---:|---|
| 拟态蜘蛛 | 复用 `Spider` | 1（不动） | 25（不动） | 只补自己的视觉；不新增 BeastKind |
| 可可达鹅 | `KekedaGoose` | 0 | 12 | 醒灵档，会啄击/反抗，不是无害背景板 |
| 马 | `Horse` | 0 | 20 | 醒灵档上沿，受惊踢击；骑乘不在本批 |
| 腐羽鹫 | `FuyuVulture` | 1 | 24 | 引气档下沿、食腐/群聚、飞行，`is_terrestrial() == false` |
| 呆怒狮 | `DainuLion` | 1 | 32 | 引气档捕食者，威胁谱系中 `is_predator` 为真 |
| 缝合兽 | `StitchedBeast` | 1 | 35 | 引气档上沿；分裂 shard 每个 8.0，数量由本体剩余血量决定 |

实施方不得自行调上表数字。若现有代码无法表达某个数值或定位，停在对应阶段并把冲突写入开放问题，不用“为了先绿”改成别的数。

## 接入面

### 进料

- server 以 `server/src/fauna/components.rs` 的 `BeastKind`、`FaunaTag`、`health_max()`、`realm_tier()`、`is_terrestrial()`、`is_prey_of()` 为妖兽身份、强度和威胁谱系入口；P0 的拟态蜘蛛继续消费 `botany/hazard.rs:345-350` 已有 `FaunaKind::MimicSpider => BeastKind::Spider` 映射。
- `server/src/npc/spawn/beast.rs:52-127` 的 `spawn_beast_npc_at` / `spawn_beast_npc_of_kind_at` 是显式物种 spawn 入口；ambient 调度、zone 预算、地形与现有掉落表是 P4 的进料，不另造第二套妖兽调度器。
- 视觉输入来自 `modelScript/creatures/{mimic_spider,kekeda_goose,horse,fuyu_vulture,dainu_lion,stitched_beast}/`。缝合兽目录已有 core/shard/fission 与头部变体共 13 个 `.animation.json`，实施时直接校验/导出，不重做这批动画。
- `server/assets/items/fauna.toml` 及既有 `fauna/drop.rs` 掉落契约负责物种产出；掉落 id、权重和材料 owner 必须先查现有表，不能在本 plan 里凭空造同名资源。

### 出料

- server 出料为 `BeastKind` → `EntityKind` / `FaunaVisualKind` 的可验证映射、完整 NPC 组件与威胁行为；新物种应能由 `spawn_beast_npc_of_kind_at` 生成，而不是先随机生成再补写标签。
- client 出料为 `FaunaVisualKind` 的 geo/texture/animation 路径、碰撞盒/渲染缩放、raw entity id 与 GeckoLib renderer 可消费的资源；每个新增资源都要同步 committed resource-pack manifest 的 sha1、size、file_count 与 `server/src/network/resourcepack.rs`。
- P4 出料接既有 ambient scheduler、威胁预算、死亡/掉落/lifecycle 和 bot e2e；不新增一个与 `plan-ambient-threat-v1` 平行的 scheduler。
- 缝合兽出料包括 shard 实体及守恒审计；shard 死亡后真元回到 zone ledger，不能把实体删除当作释放。

### 共享类型 / event

- 复用 `BeastKind`、`FaunaTag`、`FaunaVisualKind`、`entity_kind_for_beast()`、`visual_kind_for_beast()`、`spawn_beast_npc_of_kind_at()`、`NpcArchetype::Beast`、`NpcCombatLoadout`/Navigator/MovementController，以及既有 `DeathEvent`/掉落/lifecycle 事件。
- 复用 `plan-ambient-threat-v1` 提供的 `AmbientSchedulerState`、`AmbientSchedulerConfig`、`AmbientMarkerData` 与 `ambient_scheduler_system`；若需要新 marker，必须说明为何不能复用已有 marker，而不是复制调度核。
- 缝合兽分裂复用 `qi_physics::ledger::QiTransfer` / `WorldQiAccount` / `QiTransferReason` 的现有语义与 `assert_conservation()`；新增事件（若确有需要）必须只表达分裂事实，不能成为只 emit 不消费的“假账本”。
- `MundaneFaunaKind` 是 `plan-mundane-fauna-v1` 的独立凡兽 enum，不能与本 plan 的妖兽 `BeastKind` 混用；鹅和马在本 plan 中仍是妖兽档的 `KekedaGoose` / `Horse`，不是把它们塞进凡兽 bundle。

### 跨仓库契约

- server ↔ client 的硬契约是 `server/src/fauna/visual.rs` 的 `EntityKind` raw id、`FaunaVisualKind`、`entity_kind_for_beast()`、`visual_kind_for_beast()` 与 client `com.bong.client.fauna.FaunaVisualKind` 的 enum 顺序/expected raw id、geo/texture/animation 路径。新增 raw id 必须避开既有 126–145 以及其他已占用注册区间，并由实施前的注册表核验决定具体编号。
- client 资源契约固定为 `assets/bong/geo/<id>.geo.json`、`textures/entity/fauna/<id>.png`、`animations/<id>.animation.json`；缺专属动画时只能明确复用通用 `fauna.animation.json`，不能留下 T-pose/路径静默回退。三端资源包元数据必须与实际构建产物对拍。
- server → agent 继续复用现有 NPC/fauna digest 与 Redis/world-state 字段；本骨架没有新 Redis key、TypeBox schema 或 CustomPayload。若实施发现 agent 需要新物种语义，先停下补跨仓库契约决议，不在某个生物 PR 里私自加半套 schema。
- bot e2e 应以玩家可观察的实体身份、视觉资源加载、威胁/死亡结果和跨端 payload 为断言对象；不能只测 enum 能编译。

### worldview 锚点

- `docs/worldview.md §七（动态生物生态）` 是物种定位、拟态蜘蛛伏击、异变缝合兽和生态联动的正典锚点；P4 的 ambient/兽潮接入要与其“生物是竞争者、寄生虫或天道清理程序”的基调一致。
- `docs/worldview.md §三` 的六境界名称与境界顺序是 `realm_tier` 注释和测试的词汇锚点：只写醒灵、引气、凝脉、固元、通灵、化虚，不引入旧称。
- `docs/worldview.md §二` 的灵压/真元易挥发物理与 `§七` 的生态威胁共同约束缝合兽分裂和死亡：分裂不能创造真元，死亡不能把携 qi 的实体静默丢弃。
- 威胁不是开关：鹅、马即使位于醒灵档，也必须有可触发的啄击/踢击/反抗路径；“被动背景板”不满足本 plan 的生态契约。

### qi_physics 锚点

- 只有 P3 明确进入 qi 账本面：本体分裂前后以 `QiTransfer { from, to, amount, reason }` 记录本体→shard 的转移，并用 `qi_physics::ledger::assert_conservation(before, after, era_decay)` 验证总量不漂移。
- shard 死亡必须调用现有 `release_dormant_qi_to_zone`（最终落到 `QiTransferReason::ReleaseToZone` / zone ledger 的 canonical 路径）；不得 `store.remove`、只发未消费 event、或把 qi 写成临时字段后丢弃。
- 本 plan 不新增 `*_DECAY` / `*_DRAIN` / `*_ATTEN` / `RHO` / `BETA` 或任何形似衰减率的常数。若分裂确实需要扩展 `qi_physics::constants` 或 `QiTransferReason`，先停在 P3 交人工；底层物理只能由 `plan-qi-physics-v1` 的唯一实现承载。
- P0–P2 的视觉/生物登记不能顺手给新物种加真元吸收或生成路径；凡涉及 qi 的现有死亡、吸收和 dormant 生命周期都必须复用既有 canonical owner。

## Integration preflight（2026-09-17）

以下是起骨架前按 `docs/CLAUDE.md §一` 完成的五类扫描。结论是“合并接口、拆分职责”，不是把已有 plan 当作不存在；转 active 前仍需在对应阶段按当前 `origin/main` 重验。

### 1. `docs/worldview.md`

- 已查 `§二` 灵压/真元易挥发、`§三` 六境界、`§七` 动态生物生态与 `§七` 生态联动段。它们支持本 plan 的境界带、拟态蜘蛛伏击、缝合兽生态身份和兽潮/威胁语义。
- 结论：本 plan 只实现既有正典的物种接入与视觉落地，不改 worldview；不把 horse 骑乘、战利品箱或新的生态规则伪装成视觉接线。

### 2. `docs/finished_plans/`

- `plan-fauna-v1` 已归档：提供 `BeastKind` / `FaunaTag` / 妖兽 drop 与实体底盘。本 plan 扩展其登记面，不另造 fauna registry 或掉落权威。
- `plan-fauna-mimic-spider-v1` 已归档：拟态蜘蛛的伪装、吸收、nameplate 和行为语义已经存在；本 plan 的 P0 只修复自有视觉导出与 spawn 借用 `ASH_SPIDER_ENTITY_KIND` 的缺口，不重复行为/qi 机制。
- `plan-fauna-stitched-beast-v1` 已归档：它描述既有 `HybridBeast` 的普通野兽融合、灵压狂暴、兽核吸收幻觉与对应守恒。这里的 `StitchedBeast` 是本批低境界分裂接入，不能直接把 `HybridBeast` 的数值、事件或视觉语义拷贝过来；名称/运行时边界必须在 P3 前人工收口。
- `plan-fauna-experience-v1` 已归档：提供既有异变兽视觉/资源与经验侧参考，实施时复用资源包和 GeckoLib 校验惯例，不覆盖旧模型。
- `plan-ambient-threat-v1` 已归档：提供通用 ambient threat scheduler；本 plan 只接物种池、预算与 marker，不复制 scheduler。
- `plan-npc-combat-ai-v1` 已归档：提供 big-brain Scorer/Action、NPC movement/LOD/战斗 qi 记账接口；本 plan 复用通用 thinker，物种特化只在阶段交付物明确的威胁谱系内增加。
- `plan-mundane-fauna-v1` 已归档：其 `MundaneFaunaKind`、原版动物 Rail A bundle、凡兽无灵规则与凡兽生态链是另一层；本 plan 不把可可达鹅/马改成凡兽，也不改其文件或 `MundaneFaunaKind`。
- 结论：上述 plan 的接口纳入“共享类型 / event”与阶段验收，历史决议保持原样；若现状和归档 evidence 不一致，先停下报告，不在本骨架内覆盖历史。

### 3. `docs/plan-*.md` active plans

- 当前直接相交的是 `docs/plan-beast-horde-v1.md`：它负责兽潮 flow field、批量迁徙、负压灭杀和 dormant 同步。本 plan 只提供可被其识别的物种/视觉/生命周期契约，不重做 horde movement，也不擅自改变迁徙预算。
- `plan-npc-realm-distribution-v1` 与 `plan-zone-qi-economy-v1` 的 reminder 关联 NPC 境界人口/zone qi budget；本 plan 使用已定 `realm_tier`，不拥有 NPC 吸灵系数、inflow 或人口预算，数值变更回原 owner。
- 未发现另一个 active `plan-fauna-*` 直接占用这六个 `BeastKind` 名称。结论：接口可合并，职责拆分；P4 接 beast-horde/ambient 时以最新 active plan 为准，产生冲突即回到人工排序。

### 4. `docs/plans-skeleton/`

- 已扫描 skeleton 文件名与物种关键词，未发现同名 `plan-creature-wiring-v1` 或另一个同时覆盖这六个 modelScript 包的骨架；因此本文件不是重复立项。
- 已特别检查 `plan-fauna-*`、`plan-ambient-threat-v1`、`plan-beast-horde-v1`、`plan-npc-combat-ai-v1`、`plan-mundane-fauna-skeleton`、`plan-fauna-mimic-spider-v1` 相关痕迹：已完成/active 的工作保留，本 plan 只补它们没有拥有的跨端视觉与新增物种 wiring。
- `plan-mundane-fauna-skeleton` 这个文件名当前未找到；实际存在且已归档的是 `plan-mundane-fauna-v1`，本 plan 按后者的 `MundaneFaunaKind` 边界执行，不以“未找到 skeleton”推断凡兽接口可复用。

### 5. `docs/plans-skeleton/reminder.md`

- reminder 当前显式记录 `plan-npc-realm-distribution-v1 → plan-zone-qi-economy-v1` 的 NPC 境界人口/qi 预算复核；它不拥有本 plan 的物种 visual/entity wiring，也不授权本 plan 改其常数。
- 结论：本 plan 在 §开放问题登记“新增妖兽数量对 ambient/zone budget 的影响需由原 owner 复核”，不把 reminder 的 qi 预算待办吞进本 plan；不存在可合并的 creature wiring reminder 条目。

## 现状锚点与实施不变量

### server 物种与 spawn

- `server/src/fauna/components.rs:9-33` 是 `BeastKind` 当前 16 变体；`:36-54` 的 `as_str()` 是 `races.json` 解析字符串来源；`:57-75`、`:78-86`、`:89-118` 分别是 `health_max()`、`realm_tier()`、`is_terrestrial()` 与 terrestrial 集合。新增五个变体必须在所有 match、边界测试和 terrestrial 语义中逐项决定，飞行的 `FuyuVulture` 不得误入 `ALL_TERRESTRIAL`。
- `server/src/body_plan/race_registry.rs:31-58` 的 `ALL_BEAST_KINDS` / `parse_beast_kind()` 注释明确它必须覆盖全部变体；新增物种必须同步 `server/assets/body_plans/races.json:10-30` 的 `beast_common.beast_kinds`，否则 registry 加载会在运行时拒绝或漏种族。
- `server/src/npc/spawn/beast.rs:52-127` 的显式 spawn 路径必须产出正确 `MarkerEntityBundle.kind`、`FaunaTag`、NPC combat/movement/lifecycle 组件；不要在随机 spawn 后补标签，因为视觉、掉落和 thinker 可能已经读取旧种类。
- `server/src/botany/hazard.rs:345-350` 已把 `MimicSpider` 映射为 `BeastKind::Spider`；`server/src/npc/spawn_spider.rs:17,63-87` 仍使用 `ASH_SPIDER_ENTITY_KIND` / `FaunaVisualKind::AshSpider`。P0 的最小改动是自有视觉 shell 和 spawn kind，不能复制一个新 `BeastKind::MimicSpider`。

### server ↔ client 视觉

- `server/src/fauna/visual.rs:7-31` 当前 raw EntityKind 资源已使用到 145，`:33-55` 是 server `FaunaVisualKind`，`:84-123` 是两条映射函数；新增编号、顺序、映射必须由同一 PR 的契约测试锁住。
- `client/src/main/java/com/bong/client/fauna/FaunaVisualKind.java:6-26` 当前 enum、expected raw id、碰撞盒、缩放与动画路径是 client 侧对拍面；`:56-69` 将 `<id>` 派生到 geo/texture/animation 资源。`FUYA`（`:14`）是腐鸦 `NpcArchetype::Fuya`，绝不是本批的 `fuyu_vulture`，实施中必须使用独立 `FuyuVulture` 命名，禁止把两者合并。
- 每个物种的资源清单至少包括 `client/src/main/resources/assets/bong/geo/<id>.geo.json`、`textures/entity/fauna/<id>.png` 与专属或明确复用的 `animations/<id>.animation.json`；碰撞盒、render scale、shadow radius 需要从实际模型/玩法定位核定，不可复制相邻物种数字。
- modelScript 已有六个包；实施阶段负责导出、资源完整性校验和三轮视觉自评，不在本骨架阶段重新建模。缝合兽已有 13 个动画 JSON，直接导出/接入并验证 core/shard/fission 与头部变体，不重做。

## P0 — 拟态蜘蛛补视觉

**目标**：让 `FaunaKind::MimicSpider` 仍以 `BeastKind::Spider` 运行，但客户端看到自己的拟态蜘蛛视觉，而不是误借灰烬蛛 shell。

1. 从 `modelScript/creatures/mimic_spider/` 导出自有 `mimic_spider.geo.json`、贴图与所需 animation；资源落到 client 的 `geo/`、`textures/entity/fauna/`、`animations/` 约定路径。导出前后验证几何、贴图、动画 key 可加载。
2. 在 server `fauna/visual.rs` 与 client `FaunaVisualKind.java` 各增加一条同名视觉契约，并分配经过注册表核验的 raw EntityKind；保持 `BeastKind::Spider` 的 `health_max=25`、`realm_tier=1`、掉落、伪装状态和 qi 行为完全不变。
3. 在 `spawn_spider.rs:63-87` 换自有 visual/entity kind；`FaunaTag::new(BeastKind::Spider)`、`SpiderDisguiseState`、`NameVisible(false)` 和既有 thinker 仍须保留。`botany/hazard.rs:348` 不改映射。
4. 资源包构建后更新 committed manifest 与 `resourcepack.rs` 的 sha1/size/file_count；zip 仍按仓库发布流程处理，不进入 git。

**验收抓手**：MimicSpider spawn 的 server raw id 与 client expected raw id 对拍；伪装/显形两态仍切换；没有新增 BeastKind 或改变 25/1；geo、texture、animation 资源加载成功；资源包 manifest 与实际构建产物一致；bot e2e 能观察到拟态蜘蛛而非 `ASH_SPIDER_ENTITY_KIND`。

## P1 — 醒灵档：可可达鹅与马

**目标**：新增 `KekedaGoose`（tier 0 / 12）与 `Horse`（tier 0 / 20），接入统一妖兽而非凡兽 `MundaneFaunaKind`；两者都必须能反抗。

- `BeastKind`、`as_str()`、`health_max()`、`realm_tier()`、`is_terrestrial()` / `ALL_TERRESTRIAL` 的处理要逐项决定并测试；随后同步 `ALL_BEAST_KINDS` 与 `races.json` 的 `beast_common` 列表。
- 通过 `spawn_beast_npc_of_kind_at()` 接入完整 NPC bundle、FaunaTag、movement、lifecycle、drop 与 thinker；鹅的啄击、马的受惊踢击是最小反抗契约，不把两者实现为空壳/无害 ambient 标记。具体 attack 数值若超出现有通用 loadout，列入 P1 开放问题并由人工收口。
- 两个物种各自接 server `EntityKind`/`FaunaVisualKind` 与 client enum、碰撞盒、geo/texture/animation。马的 tack/骑乘 modelScript 资产不等于骑乘功能，本阶段只接站立/移动/受击视觉；骑乘、鞍具与 mount protocol 明确留后续。
- 按 `fauna.toml` 现有掉落 owner 选择 drop table；若新增掉落 id，先与 `plan-fauna-v1` 的物品契约对拍，不在视觉 PR 里临时立素材。

**验收抓手**：两项 registry 能冷启动；`KekedaGoose` / `Horse` 的 health/tier 逐位 pin；鹅/马各至少一条真实反抗路径和拒绝/死亡回归；`MundaneFaunaKind` 未被修改；server/client raw id、路径和碰撞盒对拍；资源包元数据更新。

## P2 — 引气档：腐羽鹫与呆怒狮

**目标**：新增 `FuyuVulture`（tier 1 / 24）与 `DainuLion`（tier 1 / 32），分别表达飞行食腐群聚和陆地捕食者。

- `FuyuVulture::is_terrestrial()` 必须为 `false`，不能进入 `ALL_TERRESTRIAL` 或沿用只支持地面寻路的假设；实施前确认现有 `MovementCapabilities` / navigator 是否能表达飞行。不能表达时停在开放问题，禁止把飞行物种偷偷降成地面物种。
- `DainuLion` 必须接入捕食者威胁谱系；`is_predator`/捕食关系的具体 owner 要与现有 fauna/AI 表对拍，不能只把名字写进 enum 就声称有威胁。
- 两者各自完成 body-plan registry、spawn、drop/lifecycle、server/client visual mapping 与模型资源；`FUYA`（腐鸦）和 `FuyuVulture`（腐羽鹫）必须由测试锁成两个不同标识。
- 视觉回归应能区分飞行/捕食者的 silhouette、动画和贴图；不以“共用通用 fauna 动画”掩盖缺失的物种动画。

**验收抓手**：腐羽鹫不进入 terrestrial 集合且飞行移动/落地边界有测试；呆怒狮有真实捕食/反抗触发；两物种的 `health_max`/tier 为 24/1、32/1；`FUYA`/`FuyuVulture` 跨端枚举不混淆；资源与 manifest 对拍。

## P3 — 缝合兽：分裂与真元守恒

**目标**：接入新 `BeastKind::StitchedBeast`（tier 1 / 35），把 modelScript 的 core/shard/fission/头部变体转成可验证的实体与动画，并在不改生产物理常数的前提下完成分裂守恒。

### 分裂不变量

1. 分裂前读取本体真实 qi 与剩余血量；shard 数量由本体剩余血量的既定份数规则决定，shard 的每个 `health_max` 为 **8.0**。具体取整、最少/最多 shard 数和分裂时机是开放问题，未决前不得实施。
2. 每个本体→shard 的 qi 转移必须构造 `qi_physics::ledger::QiTransfer`，由 canonical ledger 消费；不能直接给 shard 写 qi，也不能只 `send_event` 后无 consumer。
3. 分裂前后建立 `WorldQiSnapshot` 对拍，调用 `assert_conservation(before, after, era_decay)`；本阶段不凭空生成血量对应的真元，shard 没有继承来源的 qi 就应保持 0。
4. shard 死亡走 `release_dormant_qi_to_zone` → `QiTransferReason::ReleaseToZone` → zone ledger；禁止 `store.remove` 丢掉携带 qi 的快照，禁止只回收一部分而无明确 canonical 记录。
5. 若发现需要新增 transfer reason、常数或账本账户语义，立即停下交人工给 `plan-qi-physics-v1` owner，不在本 plan 自行扩物理。

### 视觉与兼容边界

- `modelScript/creatures/stitched_beast/` 中的 13 个 `.animation.json` 按 core、shard、fission 和 7 个头部变体逐项校验；导出后 client controller 必须能在本体/分裂态/头部变体之间切换。
- 本阶段的新 `StitchedBeast` 不等于已存在的 `HybridBeast`。旧 `plan-fauna-stitched-beast-v1` 的融合触发、灵压狂暴、兽核吸收幻觉和 `HybridBeast` raw id 不能被重命名覆盖；P3 前需人工决定两者是并存 kind、别名适配还是拆出共享视觉层。

**验收抓手**：本体 35、tier 1、shard 8.0 的数值 pin；分裂份数边界、零 qi、满 qi、部分 qi 与 shard 死亡全覆盖；每次转移有 ledger audit，`assert_conservation` 通过；非 canonical 删除路径会在测试中失败；旧 `HybridBeast` 仍按原行为/视觉工作；core/shard/fission/head animation、geo/texture 与 raw id 对拍。

## P4 — 威胁接入、掉落与跨端回归

- 把六物种接进既有 ambient scheduler / threat budget / zone 物种池；P4 只消费 `plan-ambient-threat-v1` 的调度核，并与 active `plan-beast-horde-v1` 的兽潮 flow field / dormant 同步接口对接。不得复制调度器、改变 horde 的 owner，或把视觉注册误算为 spawn 完成。
- 每种生物必须有明确掉落、死亡、超距回收和 dormant 行为；所有带 qi 的死亡/回收路径都走 canonical release，零 qi 物种也要证明不会误进 qi regen/吸收路径。
- 生态威胁回归：鹅、马可反抗；腐羽鹫飞行边界；呆怒狮捕食；拟态蜘蛛伏击/伪装；缝合兽分裂/死亡。bot e2e 以玩家可观察行为和 payload 为断言，不以 spawn 函数被调用替代真实链路。
- 资源包回归：构建完整 client resource pack，核对三类资源路径、文件计数、sha1、size；同步 `client/resourcepack/manifest.json` 与 `server/src/network/resourcepack.rs`，zip 仍不提交。
- 跨端回归：server raw EntityKind ↔ client expected raw id、server visual kind ↔ client enum、geo/texture/animation 资源加载、nameplate/碰撞盒/scale、agent 现有 NPC digest 均要有正反 pin；新增物种不得静默落入 `FUYA`、`ASH_SPIDER` 或通用 fallback。

## 测试与可核验交付物

每个阶段按“契约驱动的必要测试”写最小但完整的正反案例，不能只做编译或 happy path：

- **枚举/注册**：所有新增 `BeastKind` 的 `as_str`、serde/races.json round-trip、`ALL_BEAST_KINDS` 完整性、`health_max`/`realm_tier`/`is_terrestrial` 表格 pin；遗漏 match 必须编译期或测试期暴露。
- **spawn/行为**：显式物种 spawn 后 `FaunaTag`、EntityKind、visual kind、NPC movement/lifecycle/thinker 组件齐全；鹅/马反抗、呆怒狮捕食、腐羽鹫飞行、拟态蜘蛛伪装各有真实触发，不用只测组件存在。
- **跨端资源**：server/client raw id 对拍；geo、texture、animation 文件存在且可解析；专属 animation key 与 `FaunaVisualKind.idleAnimationName()` 对拍；资源包 manifest sha1/size/file_count 与构建产物对拍。
- **生命周期/掉落**：死亡、超距、dormant、掉落与回收路径各有正反案例；任何携 qi 的实体都检查 ledger；无灵/非 qi 物种不会误扣 zone qi。
- **缝合兽守恒**：分裂前后、部分分裂、零 qi、shard 死亡、异常输入/重复结算均不能漂移或双重转移；测试断言使用 `qi_physics` 的常量/快照，不硬编码全局总量。
- **e2e**：至少一条 bot/client→server→visual/lifecycle 的完整链路，覆盖 `FuyuVulture`/`FUYA` 名称区分、拟态蜘蛛自有视觉和资源包加载；单测不能替代跨端集成。

## 开放问题（转 active、进入 P0/P3 前必须收口）

1. **新 raw EntityKind / client enum 编号**：当前 server `FaunaVisualKind` raw id 已到 145，client 还有其他注册区间。需要按当前 bootstrap 顺序确定五个新物种及拟态蜘蛛的编号，写入 server/client 双端对拍测试；不能凭空选择下一个整数。
2. **`StitchedBeast` 与 `HybridBeast` 的边界**：二者是否并存为两个 `BeastKind`、共享视觉层，还是新名只作为旧融合体的显示别名？必须结合 `plan-fauna-stitched-beast-v1` 的已归档 `HybridBeast` 事件/掉落/视觉决议人工收口；在收口前不改旧 raw id。
3. **缝合兽分裂份数规则**：本体剩余血量如何取整为 shard 数、最小/最大份数、重复分裂是否允许、分裂中断如何回滚，必须给出状态机和 qi transfer 原子性证明；固定事实只有每 shard health 8.0 与 qi 守恒。
4. **qi transfer reason / dormant owner**：现有 `QiTransferReason` 是否已能表达本体→shard 与 shard→zone；若不能，决定由 `plan-qi-physics-v1` 扩展还是本 plan 暂缓 P3。不得在这里自定义常数或 emit-only 事件。
5. **飞行能力**：`FuyuVulture` 的 movement capability、导航高度、落地/被击落和 dormant 模拟接口是否已有可复用实现；若没有，先立独立基础设施或把 P2 标为 blocked，不能把 `is_terrestrial=false` 写成只改一个 match 分支。
6. **物种威胁与掉落 owner**：呆怒狮捕食表、鹅/马反抗动作、腐羽鹫食腐/群聚、六物种掉落表和物品 id 由哪些现有模块拥有；P4 前需与 `plan-fauna-v1`、`plan-ambient-threat-v1`、`plan-beast-horde-v1` 对拍，不能在视觉 PR 里立新经济规则。
7. **ambient / horde 预算**：新增五个妖兽对现有 threat budget、兽潮流场、dormant 数量和 zone qi budget 的影响如何计入；本 plan 不拥有 `plan-zone-qi-economy-v1` 的常数，须由对应 owner 审批。
8. **资源包发布边界**：确认每个 PR 的资源包构建与 committed `manifest.json` / `resourcepack.rs` 元数据更新顺序，以及大型 zip 只走 Release、不进 git 的发布责任。
9. **马的后续能力**：骑乘、鞍具、马具模型与 mount protocol 是否另立 plan；本 plan 只交付妖兽视觉/移动/反抗，未决项不能阻塞 P1 的非骑乘部分。
10. **P0 拟态蜘蛛现有行为 owner**：确认 `plan-fauna-mimic-spider-v1` 的 `FaunaVisualKind::AshSpider` / `SpiderDisguiseState` 迁移方式，使自有视觉不会破坏伪装态、nameplate 隐藏和显形 VFX。

## §开放问题决议门

开放问题全部收口前不得自动消费 P0，更不得让实施 agent 代替人工拍板。每条决议必须同时落到：

1. 具体文件:行号/符号（例如 `server/src/fauna/visual.rs:84-123`、`client/.../FaunaVisualKind.java:6-69`、`server/src/qi_physics/ledger.rs:460,1083`）；
2. 本 plan 的阶段与测试抓手；
3. 拒绝的替代路线及其越界理由。

特别是 P3 的分裂份数、qi 账户 owner、`HybridBeast`/`StitchedBeast` 命名边界和 P2 飞行能力，必须由第一性原理代码核验后再写决议；不能以 modelScript 文件存在作为运行时设计证明。

## §10 实施工作流

本 plan scope 至少四个 PR，按依赖序列化，不拆成六份互相漂移的 plan。每个 PR 使用独立 subagent 与独立 claim/slot，主线只负责调度、等待审查、验收和在前一 PR merge 后派下一批；任何 PR 都不自动 merge。

### §10.1 视觉资产的三轮打磨

P0–P3 只要导出或修改 geo、texture、animation、碰撞盒或渲染比例，就遵守视觉资产纪律：

1. Round 1 first cut：导出/接入并完成资源路径、server/client raw id 对拍，提交 `(round 1/3)`；
2. Round 2 自评：用仓库提供的 preview/render/check 脚本或等价的资源解析/截图证据检查比例、碰撞盒、贴图 UV、动画 key 与跨端命名，修正后提交 `(round 2/3)`；
3. Round 3 终审：按物种定位与威胁可辨识度复核，检查资源包构建和 manifest 对拍，提交 `(round 3/3)`；终轮 commit 追加 `<PROMISE>`，诚实写明已检查项目和仍有限制。

缝合兽 13 个既有动画是“已有资产的导出与接线”，仍需三轮接入/验证；不要求重新建模，但不能以“文件已存在”替代资源可加载证据。

### §10.2 推荐 PR 序列

1. **PR-1 P0 拟态蜘蛛**：独立完成自有视觉 shell、spawn kind 替换和资源包回归；不得混入新增 BeastKind。
2. **PR-2 P1 醒灵档**：`KekedaGoose` + `Horse` 的 server registry/body-plan/spawn 与 client 视觉，骑乘明确留后续。
3. **PR-3 P2 引气档**：`FuyuVulture` + `DainuLion`，先收口飞行能力与捕食表，再接资源和 ambient。
4. **PR-4 P3 缝合兽**：先完成 `StitchedBeast`/`HybridBeast` 边界决议，再实现分裂 ledger/死亡 release 与 core/shard/fission 视觉。
5. **PR-5 P4 集成回归**：ambient/horde/drop/lifecycle、跨端 bot e2e、资源包构建与最终契约回归；只在前四批都 merge 后开始。

每一批都保持“一批一个主题/一个 PR”，合并前必须完成 validator、受影响栈门禁、最新 `origin/main` 对拍与 Kody review；main 冲突时保留双方新增的 evidence/target 条目，不删除其他 plan 的交付物。

### §10.3 独立 subagent 与验证

- 每个 PR 启动独立实施 subagent，显式传入本节和对应阶段边界；生产代码 PR 按完整 validator → 门禁 → fetch/merge → 新 HEAD validator → push/PR 流程执行。纯文档骨架本身不运行 cargo，但要通过 markdown、路径、链接和 `git diff --check` 校验。
- 每个新 HEAD 都重新绑定 validator；validator 必须显式核对 worktree 绝对路径与 SHA，只读且结论后关闭。validator 未取得结论时，PR body 必须如实记录降级，不写成 PASS。
- client/server 跨栈 PR 分别运行对应门禁；资源包改动必须运行 `Build resource pack` 等相关 check。P3 额外要求真实 ledger 守恒输出，不能以编译绿代替守恒证明。

### §10.4 审查等待与归档

- push 后等待 CI 与 Kody 针对当前 HEAD 的结论；审查意见若涉及代码，另派返工 subagent 从同一远端分支进驻，修复后重新 validator/门禁/等待，不在骨架 PR 中顺手写实现。
- 全部阶段完成并补齐 Finish Evidence 后，才由消费流程把 active plan 归档；骨架阶段不创建 Finish Evidence、不把本文移入 `docs/finished_plans/`。
- 资源发布仍由仓库发布流程负责：zip 不提交，manifest 与 `resourcepack.rs` 的元数据必须反映已构建产物，发布失败或未发布时如实标注阻塞，不伪造“玩家已可见”。

### §10.5 单次消费边界

提交 `/consume-plan` 前，人工必须完成 §开放问题决议门；消费时按 PR-1→PR-5 顺序逐批派发并等待每批 review/e2e。未完成阶段保持 `⬜`/`⏳`，不得把骨架直接标为完成或提前迁入 `finished_plans/`。

