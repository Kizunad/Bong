# Bong · plan-alchemy-recycle-v1 · 完成归档

**炼丹废料反哺灵田**。把 alchemy 失败丹 / 药渣 / 炮制下脚料导回 lingtian 作为新的 `ReplenishSource` 一档，闭合"种田 → 加工 → 炼丹 → 废料 → 补灵 → 再种田"的资源循环。废料补灵 effect 介于 LingShui 和 Zone 之间（量小、便宜、有副作用风险）。

**世界观锚点**：
- `worldview.md §十 资源与匮乏 / 灵气是零和的`——废料里残存的灵气仍然属于"已从 zone 取出"的部分，反哺 plot 不无中生有（SPIRIT_QI_TOTAL=100 物理事实）
- **末法噬蚀**：废料 72h 内反哺否则被天地噬散——锚点是 `plan-shelflife-v1` 的衰减机制 + `worldview.md §十 灵气是零和的` 派生（**不是 worldview §十二**——§十二 是死亡 / 重生 / 寿元章，与废料保鲜无关）
- **plot 级杂染（不是真元染色）**：本 plan §3 `dye_contamination` 是 **plot 级灵气污染**（接 `worldview.md §四 战斗系统 / 异体排斥`：异种真元在容器内形成污染累积），**不是** `worldview.md §六 真元染色`（§六 染色明文与"污染"是隔离的两层概念："染色是功法特征 / 污染是战斗机制 / 两者各走各的路"）

> **注**：§六 真元染色与异体污染在 worldview 是被明确隔离的——本 plan §3 `dye_contamination` 命名上易混淆，应理解为"plot 级灵气污染"（沿用 §四 异体排斥逻辑），与玩家身上的真元染色（§六）无关。

**library 锚点**：待写 `crafting-XXXX 末法循环录`（药渣还田的传统智慧 / 杂染风险警示）

**交叉引用**：
- `plan-alchemy-v1.md`（炼丹失败 / 废丹 / 残料的产出端）
- `plan-lingtian-v1.md §1.4 ReplenishSource`（4 来源已有：Zone / BoneCoin / BeastCore / LingShui，本 plan 加第 5 档 PillResidue）
- `plan-lingtian-process-v1.md`（加工失败的废料同样进此回收路径）
- `plan-cultivation-v1.md`（污染累积 —— 杂染废料带 plot 污染 risk）

---

## §0 设计轴心

- [x] 废料反哺 = **第 5 档 ReplenishSource**，不是新 session 类型
- [x] **量大、灵气加成中等、有 risk** —— 与 BoneCoin（量小、灵气加成中、安全）形成差异化选择
- [x] 废料保鲜 72h（与鲜采作物同），逾期 → 转为"枯渣"无法反哺
- [x] **杂染机制**：单次反哺多色废料叠加 → plot 染上杂污染 → 后续作物 quality_accum 衰减
- [x] 不做"废料分类机器" —— 玩家自己挑选/平衡（增加策略深度）

---

## §1 第一性原理（烬灰子四论挂点）

- **缚论·残灵未散**：失败丹 / 药渣里仍封着原作物的真元（缚未解），还能拆出来补 plot
- **音论·杂音相冲**：多种染色废料同时反哺 → 各色"音"相冲 → plot 留下杂染（与单色补灵 LingShui 干净的对照）
- **噬论·废料 72h**：废料没有 quality_accum 维持，灵气泄漏更快 —— 必须 72h 内用掉
- **影论·残镜印**：废丹的镜印残留 → 给 plot 一定概率掉"瑕疵作物"（quality 临时上限 -0.1）

---

## §2 废料分类与反哺效果

| 废料来源 | plot_qi 加成 | 杂染风险 | 时长 | 备注 |
|---|---|---|---|---|
| **失败丹**（alchemy 完全失败） | +0.4 | 高（30% 杂染累积 +0.1）| 5s | 量最多，最便宜 |
| **废丹**（alchemy 成品但效果偏低） | +0.6 | 中（10% 杂染 +0.05）| 4s | 玩家挑选保留 |
| **药渣**（炮制 / 萃取下脚料） | +0.3 | 低（3% 杂染 +0.02）| 3s | 来自 plan-lingtian-process-v1 |
| **加工废料**（晾晒过期 / 碾粉散落） | +0.2 | 极低（<1%） | 3s | 数量稀少，主要是"清扫"用 |

加成介于 LingShui (+0.3) 和 BeastCore (+2.0) 之间，但带 risk —— 玩家若"懒人补灵"会累积污染。

---

## §3 杂染机制

- [x] `LingtianPlot` 加 `dye_contamination: f32`（[0, 1.0]）
- [x] dye_contamination 影响 quality_accum：`quality_multiplier *= 1.0 - dye_contamination * 0.3`
- [x] dye_contamination 衰减：每 lingtian-tick -0.001（自然净化），翻新（plan-lingtian-v1 §1.5 RenewSession）清零
- [x] 警戒线 0.3 → HUD 给 plot 加 "已染杂" tag

---

## §4 数据契约

- [x] `server/src/lingtian/session.rs::ReplenishSource` 加 `PillResidue` 变体（含 `residue_kind: PillResidueKind`）
  ```rust
  pub enum PillResidueKind { FailedPill, FlawedPill, ProcessingDregs, AgingScraps }
  ```
- [x] `server/src/lingtian/plot.rs::LingtianPlot` 加 `dye_contamination: f32`
- [x] `server/src/alchemy/residue.rs`（新文件）—— `PillResidue` item + `produce_residue_on_failure` 钩子
- [x] `assets/items/residue/` —— 4 种 residue toml 定义
- [x] `server/src/lingtian/contamination.rs`（新文件）—— `apply_dye_contamination_on_replenish` system + `dye_contamination_decay_tick`
- [x] schema `LingtianReplenishSource` 扩展 PillResidue 变体
- [x] client `LingtianActionScreen` 补灵 4→5 子按钮 + dye_contamination HUD tag

---

## §5 实施节点

- [x] **P0**：`PillResidue` item + `FailedPill` 一种废料 + ReplenishSource::PillResidue 变体接入 + 单测覆盖反哺成功路径
- [x] **P1**：`dye_contamination` field + 杂染累积 + quality_multiplier 衰减 + e2e 测累积到 0.3 警戒线
- [x] **P2**：4 种废料全接入 + 各自 risk 概率 + RNG 确定性测试
- [x] **P3**：客户端 LingtianActionScreen 第 5 子按钮 + 杂染 HUD tag + 翻新清零路径
- [x] **P4**：与 plan-lingtian-process-v1 联动 —— 加工废料自动产出 + 与 plan-narrative 接入（高杂染 plot 触发天道 narration）

---

## §6 后续问题（未纳入本 plan）

- 失败丹的产出概率与 plan-alchemy-v1 现有失败率挂钩（每次失败必产 1 残料 vs RNG）？
- 玩家是否可手动"丢弃废料"避免累积 inventory 压力？还是必须反哺 / 烧毁？
- 杂染累积过 0.3 后是否会影响相邻 plot（zone 内扩散）？
- 废料是否可作为 `plan-zhenfa-v1` 阵法的低品载体（替代石块）？跨 plan 复用值得探讨
- NPC 散修（plan-lingtian-npc-v1）是否也走废料反哺循环？还是只用 BoneCoin / Zone？

---

## §7 进度日志

- 2026-04-27：骨架创建。前置 `plan-lingtian-v1` ✅；`plan-alchemy-v1` 仅 P0 框架（炼丹失败路径需先实装才能挂废料钩子）；`plan-lingtian-process-v1` 同骨架，二者实施节点可并行。本 plan P0 只依赖 alchemy 现有失败路径（不依赖 process plan），可优先启动。

---

## Finish Evidence

### 落地清单

- P0/P2：`server/src/alchemy/residue.rs` 定义 `PillResidueKind`、72h TTL、4 类废料规格、失败炼丹 outcome → `FailedPill` 残料映射、库存可用性/扣除 helper；`server/assets/items/residue/alchemy_residue.toml` 注册 4 种废料 item。
- P0/P2：`server/src/network/client_request_handler.rs::grant_alchemy_outcome_item` 在 `Waste` / `Mismatch` / `Explode` 失败 outcome 上发放 `AlchemyItemData::PillResidue`，保留成丹路径。
- P0/P1/P2：`server/src/lingtian/session.rs::ReplenishSource::PillResidue`、`server/src/lingtian/systems.rs::handle_start_replenish` / `apply_replenish_completion` 接入第 5 档补灵来源，按废料规格注入 plot_qi、扣库存、拒绝过期废料。
- P1/P3：`server/src/lingtian/plot.rs`、`server/src/lingtian/contamination.rs`、`server/src/lingtian/growth.rs` 落地 `dye_contamination`、自然衰减、翻新清零、quality_accum 衰减与 0.3 警戒线。
- P3：`server/src/lingtian/network_emit.rs`、`server/src/schema/lingtian.rs`、`client/src/main/java/com/bong/client/lingtian/*` 推送/解析 `source`、`dye_contamination`、`dye_contamination_warning`，客户端补灵入口扩展到废料按钮并在 HUD 显示"已染杂"。
- P4：`server/src/lingtian/events.rs::DyeContaminationWarning` 与 `record_dye_contamination_warning_recent_events` 把首次跨 0.3 警戒线写入 `ActiveEventsResource.recent_events`，目标 `lingtian_plot_dye_contamination_warning` 进入天道 world_state recent_events 管道；加工过期物 `withered_processed_*` / `withered_dry_*` 已映射到 `ProcessingDregs` / `AgingScraps`。
- Schema：`agent/packages/schema/src/client-request.ts`、`agent/packages/schema/src/inventory.ts`、generated JSON schema 与 `agent/packages/schema/tests/schema.test.ts` 覆盖 pill residue metadata 与废料补灵 request source。

### 关键 commit

- `87165319` · 2026-05-06 · `plan-alchemy-recycle-v1: 接入废料反哺灵田`
- `3846a216` · 2026-05-06 · `plan-alchemy-recycle-v1: 扩展废料协议 schema`
- `7e2797a6` · 2026-05-06 · `plan-alchemy-recycle-v1: 补齐废料补灵客户端入口`
- `567b673e` · 2026-05-06 · `plan-alchemy-recycle-v1: 对齐 NPC 灵田测试字段`

### 测试结果

- `cd server && cargo fmt --check` ✅
- `cd server && cargo clippy --all-targets -- -D warnings` ✅
- `cd server && cargo test` ✅ `2477 passed; 0 failed`
- `cd agent && npm run build` ✅
- `cd agent && npm test -w @bong/schema` ✅ `277 passed`
- `cd agent && npm test -w @bong/tiandao` ✅ `241 passed`
- `cd client && JAVA_HOME="$HOME/.sdkman/candidates/java/17.0.18-amzn" ./gradlew test build` ✅

### 跨仓库核验

- server：`PillResidueKind`、`AlchemyItemData::PillResidue`、`ReplenishSource::PillResidue`、`apply_dye_contamination_on_replenish`、`DyeContaminationWarning`、`lingtian_plot_dye_contamination_warning`。
- agent/schema：`PillResidueKindV1`、`AlchemyItemDataV1.kind = "pill_residue"`、`LingtianStartReplenishRequestV1.source = "pill_residue_*"`。
- client：`LingtianActionScreen.REPLENISH_SOURCES`、`LingtianSessionStore.Snapshot.dyeContaminationWarning`、`LingtianSessionHud` 的"已染杂" tag。

### 遗留 / 后续

- 本 plan 不新增独立废料分类机器，也不扩展相邻 plot 污染扩散、阵法载体复用或 NPC 专用策略；这些仍归后续 plan 决策。
