# plan-bughunt-agent-schema-generated-registry-gap-v1

状态：✅ 2026-07-13
分区：BugHunt worker / agent-schema / 第 2 轮

阶段总览：P0 契约清单 ✅ 2026-07-13；P1 registry/generated 接入 ✅ 2026-07-13；P2 freshness 饱和门禁 ✅ 2026-07-13；P3 对抗复核与全栈验证 ✅ 2026-07-13

## 1. 一句话结论

若干已被 Tiandao runtime 实际消费的 server -> agent Redis V1 payload 只存在 TypeBox validator 和 sample-pin 测试，没有进入 `SCHEMA_REGISTRY` / `GENERATED_SCHEMA_FILES`，导致生成 JSON Schema 与 freshness gate 对这些公开契约假绿。

## 2. 实际游玩体验影响

这不是“线上消息会立刻丢失”的 runtime bug：当前 runtime 仍会 import 对应 validator、订阅 Redis channel 并做校验。

真正风险是契约审计假绿。经脉断裂、蜕壳灰烬、大能遭遇、战事结算、具名势力状态这些玩家可感知叙事/调试路径，已经是 server -> agent 公开 wire 契约；但外部 JSON Schema 生成物、CI freshness gate、跨语言契约审计都看不到它们。后续 server 字段改名、可选性变化或枚举扩展时，`generate:check` 仍可能保持绿色，直到玩家看到天道叙事缺失、HUD/日志定位困难，或下游工具继续引用过期契约。

## 3. 复现路径

1. 在 `agent/packages/schema/src/generated-artifacts.ts:42-65` 可见 freshness snapshot 只遍历 `GENERATED_SCHEMA_FILES`，未在该 map 的 schema 不会被 expected/missing/changed 检查覆盖。
2. 在 `agent/packages/schema/src/schema-registry.ts:968-1082` 可见生成物白名单包含 `tuike-v2-skill-event-v1.json` 等 combat payload，但没有 `meridian-severed-event-v1.json`、`tuike-ash-decay-v1.json`、`elder-encounter-event-v1.json`、`faction-war-event-v1.json`、`named-faction-state-v1.json`。
3. 直接列目录可复现缺失：`find agent/packages/schema/generated -maxdepth 1 -type f | sort` 没有上述 5 个 JSON 文件。
4. 运行 `cd agent/packages/schema && npm run generate:check` 当前仍会只按 `GENERATED_SCHEMA_FILES` 的 expected set 判定 freshness；由于这些 payload 没在 expected set，命令不会因为缺少它们而红。

## 4. 根因证据

- `agent/packages/schema/src/meridian-severed.ts:1-12` 注释明确说 schema-registry/sample 留给首个 runtime 调用方接入时补；现在首个 runtime 已存在。`agent/packages/tiandao/src/main.ts:207-209` 启动经脉断裂 runtime，`agent/packages/tiandao/src/meridian-severed-runtime.ts:11-16,68-76,105` import validator、订阅 `MERIDIAN_SEVERED` 并校验 payload。
- `agent/packages/schema/src/tuike-v2.ts:77-98` 定义 `TuikeAshDecayV1`，注释写明 server 发布、agent 订阅；`agent/packages/tiandao/src/tuike_ash_runtime.ts:11-17,83-89,115` 实际 import validator、订阅 `TUIKE_V2_ASH_DECAY` 并校验。
- `agent/packages/schema/src/elder-encounter.ts:34-83` 定义 `ElderEncounterEventV1`；`agent/packages/tiandao/src/elder-encounter-narration.ts:11-19,76-83,103` 实际 import validator、订阅 `ELDER_ENCOUNTER` 并校验。
- `agent/packages/schema/src/npc.ts:172-212` 定义 `FactionWarEventV1`；`agent/packages/tiandao/src/war-outcome-narration.ts:17-25,121-127,155-168` 实际 import validator、订阅 `FACTION_WAR` 并校验。
- `agent/packages/schema/src/npc.ts:293-318` 定义 `NamedFactionStateV1`；`agent/packages/tiandao/src/named-faction-narration.ts:1-9,69-75,97-107` 实际 import validator、订阅 `NAMED_FACTION_STATE` 并校验。
- `agent/packages/schema/tests/generated-artifacts.test.ts:38-69` freshness gate 覆盖已登记生成物的 missing/changed/unexpected，但没有“runtime-consumed Redis V1 payload 必须登记生成物或显式豁免”的覆盖规则。

## 5. 不重复性

- 不重复 #1054 NPC combat/relic shared schema parity：本计划不讨论 NPC combat/relic 共享 schema，而是讨论已被 Tiandao runtime 消费的 Redis payload 缺少生成物覆盖。
- 不重复 `docs/plans-skeleton/plan-bughunt-client-request-schema-drift.md`：该题是 C2S union 漂移，本计划是 server -> agent Redis V1 JSON Schema 生成物覆盖缺口。
- 不重复 `docs/plans-skeleton/plan-module-wiring-gaps-v2.md` 的 T10：本计划不主张 `FactionCensusStore` 未实例化，也不把消费链设计作为 bug；只收敛到 schema registry/generated coverage。

## 6. 修复计划骨架

1. 梳理“server -> agent Redis V1 且已被 Tiandao runtime import `validate*Contract` 消费”的契约清单，先纳入本 bug 已确认的 5 项：`MeridianSeveredEventV1`、`TuikeAshDecayV1`、`ElderEncounterEventV1`、`FactionWarEventV1`、`NamedFactionStateV1`。
2. 将上述 TypeBox schema 补入 `SCHEMA_REGISTRY`，再补入 `GENERATED_SCHEMA_FILES`，生成并提交对应 JSON 文件。
3. 新增覆盖测试：Tiandao runtime import 的 server -> agent Redis V1 validator，必须存在于 `GENERATED_SCHEMA_FILES`，或进入 `intentionalNonGeneratedContracts` 豁免表并写明原因。
4. 谨慎复核 `FactionStateV1` 等相邻契约是否也属于“runtime-consumed public Redis payload”；不要把未实例化 store 或未来设计项混入本修复。
5. 保持生成物 gate 原有语义：不要求所有 TypeBox validator 都必须生成，只要求公开且 runtime-consumed 的 wire contract 有生成物或显式豁免。

## 7. 验证计划

1. 修复前新增覆盖测试应能复现失败，明确指出上述 payload 缺少 generated schema registration。
2. 修复后运行：`cd agent/packages/schema && npm run generate:check`。
3. 修复后运行：`cd agent/packages/schema && npm test`。
4. 修复后运行：`cd agent && npm run build -w @bong/schema`，确保 dist 构建产物包含新增 registry/export 变化。
5. 抽查 `agent/packages/schema/generated/` 出现上述 5 个 JSON 文件，且删除任一文件时 freshness gate 会失败。

## 8. 对抗复核记录

候选证据：我最初提出“多个已消费 Redis V1 payload 缺少生成 JSON Schema，导致 freshness gate 对公开契约假绿”，并列出 meridian severed、tuike ash、elder encounter、faction war、named faction state 五类证据。

反方质疑第一轮：不能夸大为线上消息丢失；不能宣称所有 validator 都必须生成；还要排除已有 BugHunt 题目与 module-wiring-gaps-v2 的重复。

修正/反驳：候选降级为中风险契约门禁 bug，只主张 runtime-consumed server -> agent Redis V1 public payload 必须纳入 generated coverage 或显式豁免；移除 `FactionCensusStore` 未实例化等消费链论点，不再声称 runtime 丢消息。

反方最终裁决第二轮：PASS，medium risk。成立点是 `GENERATED_SCHEMA_FILES` 驱动 freshness gate，而多个已被 Tiandao runtime 消费的公开 payload 不在 expected set；风险表述必须限定为未来漂移/外部 JSON Schema/CI 审计盲点，而非即时主路径崩溃。


## Finish Evidence

- **落地清单**：P0 实地追踪 `agent/packages/tiandao/src/main.ts` 启动链与 `validate*Contract` imports，确认原 5 项并补发现 live 的 `CraftOutcomeV1`、`RecipeUnlockedV1`、`SkillLvUpPayloadV1`；排除未实例化的 `FactionCensusStore` / `FactionStateV1`。P1 在 `agent/packages/schema/src/schema-registry.ts` 注册 8 项契约并生成 `agent/packages/schema/generated/{meridian-severed-event,tuike-ash-decay,elder-encounter-event,faction-war-event,named-faction-state,craft-outcome,recipe-unlocked,skill-lv-up-payload}-v1.json`。P2 在 `agent/packages/schema/tests/generated-artifacts.test.ts` 固定 runtime contract→generated file 映射，并验证删除运行时契约文件会触发 freshness 失败。P3 对抗追踪所有实际启动 Tiandao validator 后 clean。
- **关键 commit**：`4a1f7474`（2026-07-13）纳入首批 5 项天道运行时契约；`909edf4c`（2026-07-13）对抗复核后补齐 craft/recipe/skill-lv-up 三项相邻 live 契约。
- **测试结果**：`cd agent/packages/schema && npm test`（29 files / 850 tests PASS）；`npm run generate:check`（405 generated schemas fresh）；`cd agent && npm run build -w @bong/schema` PASS；`cd agent/packages/tiandao && npm test` PASS；`cd agent && npm run build` PASS。合并最新 `origin/main` 后再次执行 schema 850 tests 与 schema build 均 PASS。
- **跨仓库核验**：server 发布侧契约由现有 Redis V1 emitters 保持；agent runtime 消费侧命中 `meridian-severed-runtime.ts`、`tuike_ash_runtime.ts`、`elder-encounter-narration.ts`、`war-outcome-narration.ts`、`named-faction-narration.ts`、`craft-runtime.ts`、`skill-lv-up-runtime.ts`；schema 侧命中 `SCHEMA_REGISTRY` / `GENERATED_SCHEMA_FILES` 与 8 个 JSON artifacts。client 不消费这些 server→agent Redis 契约，故无 client 改动。
- **遗留 / 后续**：`FactionStateV1` 对应 `FactionCensusStore` 当前未在 `main.ts` 实例化，不属于 runtime-consumed 范围；未来接线时必须同步纳入 generated coverage 或显式豁免。
