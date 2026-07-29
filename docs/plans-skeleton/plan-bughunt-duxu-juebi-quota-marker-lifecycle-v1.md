# plan-bughunt-duxu-juebi-quota-marker-lifecycle-v1（骨架）

> **骨架（草案）**。一句话主题：把 `JueBiAfterDuXuQuota` 从“挂在实体上、无 attempt 身份且只在绝壁正常结算清理”的瞬时 marker，收束为当前渡劫 attempt 的唯一配额后续意图：先统一 accepted-start / terminal cleanup 不变量，阻断旧 marker 伪造绝壁，再补 durable intent 与重启水合，保证真正超额的渡虚不会因重启丢失绝壁与最终晋升。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | attempt-scoped ECS marker 生命周期：accepted start replace/clear + 全终止路径统一 cleanup | ⬜ |
| P1 | 配额 follow-up intent 持久化 / 水合 + restart 正反闭环 + orphan/reset hardening | ⬜ |

## 接入面

- **进料**：`server/src/cultivation/tribulation.rs:860-1023` 的 `start_tribulation_system` 计算 void quota，并在成功持久化渡虚 attempt 后于 `:939-957,983-987` 构造/插入 `JueBiAfterDuXuQuota`；`start_due_juebi_triggers_system` 另有 independent JueBi 入口。
- **出料**：`tribulation_wave_system` 在渡虚最后一波读取 marker 并转入 `VoidQuotaExceeded` 绝壁；`juebi_settlement_system` 读取同一意图决定原子化虚晋升及结算 reason；failure、disconnect-as-fled、boundary flee、intercept-death、普通成功与 reset/despawn 必须结束该 attempt 的 marker 所有权。
- **共享类型 / event**：复用 `TribulationState`、`TribulationOriginDimension`、`JueBiAfterDuXuQuota`、`JueBiRuntimeContext`、`ActiveTribulationRecord`、`TribulationSource::VoidQuotaExceeded` 与既有 persistence API；禁止另造与 active tribulation 平行的第二套状态机。
- **跨仓库契约**：纯 server ECS + SQLite persistence 生命周期修复；不改 agent/client/public IPC，不新增 CustomPayload。若 P1 扩 `active_tribulations` 持久化形状，migration 与 join hydration 必须同 PR/同 bugfix 分支完成。
- **worldview 锚点**：`worldview.md §三 L72,L81,L126-L130` 规定化虚名额稀缺且通灵→化虚必须渡虚；`docs/finished_plans/plan-tribulation-v2.md §6 L816-L826` 规定“超额 attempt 正常渡虚后追加绝壁，存活才由天道承认”。旧 marker 不得把过去的天道裁决嫁接到普通新 attempt，重启也不得抹掉当前超额裁决。
- **qi_physics 锚点**：本 plan 只修 attempt metadata ownership，不新增 `QiTransfer`、不改配额公式、绝壁 drain 或 failure/flee 释放。现有失败/逃遁/JueBi 真元路径必须保持单次执行并继续通过 ledger 守恒；任何 cleanup helper 不得顺手重复释放真元。

## Canonical Finding Mapping

- 唯一 canonical owner 来自 `docs/finished_plans/plan-bughunt-r6-findings-v1.md:53-61` 的 r6 #6：`JueBiAfterDuXuQuota` 插入/读取存在，但终止态无统一 cleanup invariant；本 successor 正式承接 `plan-bughunt-duxu-juebi-quota-marker-lifecycle-v1`。
- r6 历史正文 `:32-41` 描述 failure/fled/intercept 后旧 marker 被下一次非超额渡虚读取并伪造绝壁；本 plan 以当前主线重新验真，不把旧行号当当前证据。
- persistence/restart false-negative 是同一 attempt-intent 不变量的第一性补全，不另计第二条 round finding：只修 stale-positive 会让合法超额 attempt 在重启后静默失去绝壁或化虚晋升。

## 第一性验真（`origin/main @ de75f14e43daf1105ea978c43d187acbb7f12f14`，2026-07-29）

1. **marker 无 attempt 身份**：`server/src/cultivation/tribulation.rs:304-310` 的 `JueBiAfterDuXuQuota` 只有 `occupied_slots`、`quota_limit`、`total_world_qi`、`quota_k`，没有 character/started_tick/generation key；同一实体上的任意旧 marker 都会被当前 attempt 接受。
2. **accepted non-over-quota start 不清旧值**：`start_tribulation_system` 在成功保存 active row 后，只有 `Some(marker)` 才 `insert`（`:939-957,973-987`）；`None` 分支不 remove。因此 Attempt A 超额插 marker → failure/flee/intercept → Attempt B 非超额成功启动后，A 的 snapshot 仍在实体上。
3. **旧值会真实改变结果**：`tribulation_wave_system` query marker（`:3166-3177`）并在最终渡虚 wave 于 `:3216-3258` 无 attempt 对拍地追加 JueBi；`juebi_intensity_for_quota_marker`（`:1285-1292`）又使用旧 `occupied_slots/quota_limit` 决定强度。`juebi_settlement_system`（`:1900-2081`）以 marker presence 选择 quota ascension 和 quota-exceeded reason。
4. **只有正常 JueBi settlement 清 marker**：当前唯一 production remove 在 `juebi_settlement_system:2070-2080`。failure（`:3334-3427`）、disconnect abort（`:3433-3494`）、boundary flee / shared settle（`:3498-3670`）、intercept death（`:3673-3749`）及普通渡虚成功 cleanup（`:3260-3315`）均无统一 marker cleanup；independent JueBi start（`:1158-1243`）也不清旧 DuXu quota marker。
5. **调度允许在正确边界清理**：`server/src/cultivation/mod.rs:428-469` 串行 failure → disconnect abort → boundary escape → wave → JueBi settlement，且 disconnect abort 在 player despawn 前；failure/flee 先同步标 `failed/Settle`，再 deferred remove，后续 wave 会跳过。测试必须跑 schedule/apply-deferred 后再断言。
6. **durable shape 丢失 follow-up intent**：`ActiveTribulationRecord`（`server/src/persistence/mod.rs:467-478`）不存 quota flag/snapshot；active row API（`:3101-3262`）也无法区分普通与超额 DuXu。join hydration（`server/src/cultivation/mod.rs:819-879,997-1005`）重建 state/origin，并仅为 `kind == "jue_bi"` 重建 runtime context，从不重建 `JueBiAfterDuXuQuota`。
7. **重启出现双 false-negative**：超额 DuXu 中途重启会被水合成普通 DuXu，末波不追加绝壁；已转入 quota-origin JueBi 后重启虽恢复 `JueBiRuntimeContext.source`，却无 marker，存活者不会进入 marker-gated atomic ascension，反而落到 HalfStep。
8. **现有测试只锁 presence**：`tribulation.rs:4405-9053` 与 `cultivation/mod.rs:2154-2593` 已覆盖配额 FCFS、failure/flee/disconnect、JueBi settlement 和 active-row restore，但 marker 专项断言（如 `tribulation.rs:4477,4572,7128`）只看插入，不覆盖 terminal absence、同实体 retry 或 quota intent persistence round-trip。

## P0 — Attempt-scoped ECS lifecycle closure

- [ ] 冻结核心不变量：`JueBiAfterDuXuQuota` **iff** 当前 active attempt 是“已接受且成功持久化的超额 DuXu”或其直接追加的 quota-origin JueBi。marker 不能脱离 `TribulationState` 独存，不能附着于 independent JueBi，也不能由旧 attempt 被新 attempt 继承。
- [ ] accepted start 原子替换：只有新 DuXu 已通过全部校验且 `persist_active_state` 成功后才 mutate ECS；超额则 insert/replace 当前 snapshot，非超额则显式 remove 旧 marker。duplicate/rejected start、DB persist 失败或已有 active state 时不得清除合法当前 marker。
- [ ] independent JueBi 成功接受并持久化时显式清理 DuXu quota marker；不得让旧 marker 把 independent JueBi settlement 误分类为 `void_quota_exceeded`。
- [ ] 建立单一 `clear_tribulation_attempt_runtime` / `clear_duxu_quota_follow_up`（名称可等义）并接入所有真 terminal boundary：普通 DuXu success、failure、shared fled settlement（boundary + disconnect）、intercept death、JueBi settlement；不得在 DuXu 最终 wave → appended quota JueBi 的 intentional transition 清理。
- [ ] cleanup helper 只清 attempt runtime component，不重复删 DB row、不重复发 lifecycle/qi event；各终止系统保留现有“DB 删除失败 warning、ECS 仍终止”政策，除非 §8.1 另有双锚点决议。
- [ ] orphan hardening：production invariant check 对 `marker + no TribulationState`、`marker + independent JueBi` fail-closed 清理并留可观测诊断；不得仅靠每 tick sweep 掩盖 terminal path 漏接，源码/测试仍须逐路径 pin。
- [ ] 饱和测试：`accepted_non_over_quota_start_clears_stale_quota_marker`、`failed_over_quota_du_xu_retry_does_not_append_stale_juebi`、`fled_over_quota_du_xu_retry_does_not_append_stale_juebi`、`intercepted_over_quota_du_xu_clears_quota_marker`、`disconnect_fled_du_xu_clears_quota_marker_before_despawn`、`standard_duxu_success_clears_invalid_quota_marker`、`independent_juebi_start_clears_stale_duxu_quota_marker`、`quota_marker_survives_duxu_to_juebi_transition`、`quota_juebi_settlement_clears_follow_up_marker`、rejected/persist-failure 保留合法 marker，以及 deferred-command 前后断言。
- [ ] 可核验 production symbols：`JueBiAfterDuXuQuota`、`start_tribulation_system`、`start_due_juebi_triggers_system`、`tribulation_failure_system`、`abort_du_xu_on_client_removed`、`tribulation_escape_boundary_system`、`settle_fled_tribulation`、`tribulation_intercept_death_system`、`tribulation_wave_system`、`juebi_settlement_system`、`clear_duxu_quota_follow_up`（名称可等义）。

## P1 — Durable quota follow-up intent + hydration

- [ ] 在 §8.1 冻结 durable representation 后，把 quota-origin/follow-up intent 与 active attempt **同事务**持久化；普通 DuXu、超额 DuXu、independent JueBi、quota-origin JueBi 四种 durable state 必须可区分。禁止仅依赖易失 ECS marker，也禁止 join 时按当前配额重算过去裁决。
- [ ] 明确 snapshot 口径：JueBi intensity 继续使用 attempt 接受时冻结的 `occupied_slots/quota_limit`（或等价 precomputed intensity），最终化虚名额仍由现有原子 settlement 读取当前 quota；重启不得改变这两个时点的职责。
- [ ] join/restart hydration 必须恢复：① over-quota DuXu 的 follow-up marker，使末波仍追加 JueBi；② quota-origin JueBi 的 settlement intent，使存活者仍走 atomic ascension；③ normal DuXu 不得生成 marker；④ independent JueBi 不得生成 quota marker。
- [ ] schema migration 覆盖旧 row：旧 active DuXu/JueBi 缺 follow-up 数据时必须有明确 fail-closed 处理和诊断，不能把所有旧 DuXu 猜成超额或把所有 `source=void_quota_exceeded` 静默降级；SQLite migration、round-trip、旧版本 fixture 与 rollback/error 分支同包测试。
- [ ] `total_world_qi` / `quota_k` 若不参与恢复或审计则删除；若保留则进入 typed durable shape 并有消费/diagnostic，禁止继续作为 marker 内永不读取的装饰字段。
- [ ] 扩大 lifecycle hygiene 至 fresh-character reset（`server/src/combat/lifecycle.rs:1803,1959`）、dev reset（`server/src/cmd/dev/reset.rs:338-368`）及 NPC same-entity retry；NPC dormancy/rehydration 若不能保留 active attempt，则必须先走统一 terminal cleanup，不能脱水遗留或吞掉 follow-up intent。
- [ ] 饱和测试：`over_quota_duxu_follow_up_intent_round_trips_persistence`、`restored_over_quota_duxu_still_appends_juebi`、`restored_quota_origin_juebi_can_complete_ascension`、`restored_normal_duxu_has_no_quota_follow_up_marker`、`restored_independent_juebi_has_no_quota_marker`、旧 row 缺字段、DB error、fresh/dev reset、NPC dormancy/rehydration、重复 hydration 幂等。
- [ ] 可核验 symbols：`ActiveTribulationRecord`、`persist_active_state`、`attach_cultivation_to_joined_clients`、`JueBiRuntimeContext`、`TribulationSource::VoidQuotaExceeded`、`complete_tribulation_ascension` / `try_complete_tribulation_ascension`、`quota_follow_up`（最终字段名可等义）及上述 test symbols。

## 范围边界 / 已排除项

- 不改 void quota 公式、quota K、绝壁 intensity 平衡、HalfStep 机制或原子挤位策略；仅使既有 start-time judgment 在正确 attempt 上生效并可恢复。
- 不重做绝壁 VFX/SFX/HUD/narration、地形三相或观战/截胡玩法；本 plan 是纯 server lifecycle/persistence 修复，不触发新 A/V 资产。
- 不重写 death lifecycle、disconnect-as-fled 或 NPC virtualization；只补它们对当前 quota follow-up intent 的 cleanup/persistence 契约。
- 不把 marker cleanup 伪装成 qi flow；不新增或修改 qi_physics 常数，不允许同一路径重复 release/drain。
- 不消费 r3 其他 JueBi 孤儿 finding，除非 canonical Finding Mapping 明确指向本 successor；当前唯一 owner 仅 r6 #6 及其同根 persistence 完整性。

## §8 开放问题（实施前须追加 §8.1 决议）

1. `JueBiAfterDuXuQuota` 是否继续作为 ECS projection，还是改名为明确的 attempt runtime intent；是否需要 `started_tick`/attempt generation 作为防御性身份？
2. accepted start 的 replace/remove 应集中在哪个“DB persist 成功后”helper，如何证明 rejected/duplicate/DB-failure 不会误清合法 marker？
3. 哪一个 centralized cleanup helper 覆盖全部 terminal/reset 路径；orphan invariant check 是诊断+清理还是只 fail-fast 测试门？
4. durable shape 复用现有 `source`/`intensity`，还是给 `ActiveTribulationRecord` 新增 typed quota-follow-up 字段；SQLite migration 版本和旧 row 策略是什么？
5. restart 后绝壁强度使用 start-time 完整 quota snapshot、precomputed intensity，还是重算；如何保持“强度用开始时裁决、最终名额用 settlement 原子检查”的现语义？
6. `total_world_qi` / `quota_k` 是持久审计证据还是 dead fields；保留时哪个 production diagnostic 消费它们？
7. fresh-character reset、dev reset、player despawn、NPC dormancy 各自是 terminal cleanup 还是 durable pause；哪些状态允许跨实体/进程恢复？
8. terminal DB delete 失败时维持当前 warning + ECS cleanup，还是 fail-closed 保留 attempt；如何避免下一次启动时 stale row 与无 marker 再次分叉？

> 在上述八项全部按 `docs/CLAUDE.md §五` 追加当前 `file:line + plan 章节` 双锚点决议之前，P0/P1 不得实施；不得由 bugfix subagent临场决定 persistence 语义。

## §10 实施工作流

### §10.1 BugFix 单 skeleton / 单 PR

本文件必须走根 `CLAUDE.md` BugFix 专用流程：一个 skeleton = 一个修复 subagent = 一个常驻 slot = 一个 PR，禁止交给 `/consume-plan` 或拆多个 implementation PR。subagent 先 promotion，再第一性复验；按 P0 lifecycle、P1 persistence/hydration、integration 三个中文原子 commit 批次完成，最后一次性归档并开唯一 `bugfix/plan-bughunt-duxu-juebi-quota-marker-lifecycle-v1` PR。

### §10.2 验收门

- 每批实现必须同包接通 production caller/schedule/persistence/hydration，测试从真实 Bevy schedule 与 SQLite fixture 驱动；直接调用 cleanup helper 只能作单元补充。
- 在 `server/` 下通过 `flock /tmp/bong-cargo.lock` 或 `scripts/build-token.sh` 运行 `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`；严禁本地运行 `scripts/test-tmux-shutdown-order.sh` 或任何调用它的 suite，关停覆盖留给 GitHub e2e。
- 全部 batch 完成后对显式 slot 绝对路径 + exact HEAD SHA 启动 fresh-context read-only validator；任何 HEAD 变化都重验。push 前紧邻 `git fetch origin && git merge origin/main`，merge 带进改动则重跑受影响 gate/validator。
- push 唯一 bugfix 分支后开一个 PR 并独立评论 `/review`；返工留在同 PR，不重复 promotion/归档。提交均为中文原子 commit，带 `Model: <精确模型 id>` 与 `Co-Authored-By` trailer。

### §10.3 归档前验收

- 同实体 stale-positive 链必须完整对拍：Attempt A 超额 → failure/flee/intercept → Attempt B 非超额 → final wave，不得追加 JueBi；marker 在每个 terminal apply-deferred 后均不存在。
- restart false-negative 链必须完整对拍：超额 DuXu restart 后仍追加 quota JueBi；quota-origin JueBi restart 后存活仍走 atomic ascension；normal/independent attempt 不得生成 quota intent。
- `JueBiAfterDuXuQuota` / durable projection 与 active attempt 的组合矩阵全部合法，unknown/legacy row fail-closed；failure/flee/JueBi qi 路径保持单次 ledger 守恒。
- P0/P1 全部 ✅、review 无 blocker/major、server gate 与 GitHub e2e 通过后，才填写 `## Finish Evidence` 并迁入 `docs/finished_plans/`。
