# plan-forge-session-entry-wiring-v1 — 武器锻造起炉入口接线：打通 ForgeStartSession/ForgeBlueprintTurnPage 双端断链

> **一句话**：锻造 session 引擎、四步状态机、UI 面板、schema/proto 全部已实装，但**起炉入口两个 C2S 变体的分发被丢弃**（server handler 打 debug log 吞掉、client 无 sender）——整个武器炼器玩法对真实客户端不可达，只能测试驱动。本 plan 补最后一层接线 + 全链路 bot e2e。
>
> 来源：2026-07-07 三产三用 bot 测试（PR #1072）实测「协议死路」，用户拍板立 plan 收口。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | server 分发接线（ForgeStartSession → StartForgeRequest / TurnPage → 图谱书） | ⬜ |
| P1 | client sender（ForgeScreen 起炉按钮 + 图谱书翻页发包） | ⬜ |
| P2 | 放砧/起炉/结算回执补齐 + 全链路 bot e2e（fan_tie×3 → iron_sword 入包） | ⬜ |

---

## §1 接入面（docs/CLAUDE.md §二 checklist）

- **进料**（全部已实装，本 plan 只做接线，引用两份归档 plan 为源）：
  - `docs/finished_plans/plan-forge-v1.md`（引擎 MVP）+ `plan-forge-leftovers-v1.md`（UI 面板/中途三步接线）
  - server 引擎：`forge/session.rs`（`ForgeSession` 四步状态机 Billet/Tempering/Inscription/Consecration + `ForgeSessions` 总表）、`forge/mod.rs::handle_start_forge_requests`（:141，**现为空读**——`StartForgeRequest` 全仓零 send）、`forge/steps.rs`（纯函数+单测齐）、`forge/blueprint.rs`（`assets/forge/blueprints/*.json`，最简 `iron_sword_v0` = `fan_tie`×3）
  - schema/proto：`ClientRequestV1::ForgeStartSession{v,station_id,blueprint_id,materials}`（client_request.rs:605）/`ForgeBlueprintTurnPage`（:636）、proto 桥 proto_convert.rs:3934/3981——**wire 层就绪**
  - client：`ForgeScreen.java` + Tempering/Inscription/Consecration 面板组件 + 4 个 store + 4 个 handler——**面板就绪、无 sender**
- **出料**：起炉后走既有 `ForgeSessions` → 中途三步（已接线的 `forge_tempering_hit`/`forge_inscription_scroll`/`forge_consecration_inject`/`forge_step_advance`）→ `inventory_bridge` 结算武器入包 → `ForgeOutcomeEvent`
- **共享类型**：复用 `StartForgeRequest`（forge/events.rs:11，已定义未消费——接线即救活）；**不新造任何 event/schema**
- **跨仓库契约**：server `handle_client_request` 两分支分发；client `ForgeScreen`/blueprint book 发 `forge_start_session`/`forge_blueprint_turn_page`；agent 不参与
- **worldview 锚点**：炼器归 worldview 锻造/法宝章（plan-forge-v1 已锚定，本 plan 纯接线不动正典）
- **qi_physics 锚点**：开光注真元已由既有 `forge_consecration_inject` 链路走守恒（bughunt qc-P1 #580 修过通胀），本 plan 不新增真元路径

## §2 断链证据（2026-07-07 实测 + Explore 库存）

- `client_request_handler.rs:2601-2606`：`ForgeStartSession | ForgeBlueprintTurnPage => tracing::debug!("plan-forge-v1 client_request not yet wired")` —— 请求被静默丢弃
- `StartForgeRequest` 的 `.send()` 全仓 **0 处**（仅测试 `insert_test_forge_session` 塞会话）→ `handle_start_forge_requests` 永远空读
- client 全仓 grep `ForgeStartSession`/`BlueprintTurnPage` **0 命中**——UI 面板存在但没有发包入口
- 两份归档 plan 的「遗留/后续」都**没登记**这个缺口（归档遗漏）；bot 场景 `production_forge_station_real_place` 已用宽容断言钉住现状
- 另两处观察面弱点一并收：放砧成功无专属回执（只有 inventory 扣除）；forge 结算 `forge_outcome` payload 需确认起炉打通后真实可达

## §3 阶段划分

- **P0 server 分发**：`:2601` 分支改为解析 `station_id/blueprint_id/materials` → 校验（station 存在/blueprint 已学或可用/材料足）→ send `StartForgeRequest`；`ForgeBlueprintTurnPage` 分发到图谱书翻页状态。拒因走 chat/forge payload（材料不足/砧不存在/blueprint 未学，对齐炼丹 `send_alchemy_error` 模式）。单测：受理/三类拒因/materials 形状（`Vec<(String,u32)>`）
- **P1 client sender**：`ForgeScreen` 起炉按钮接 `ClientRequestSender`（对齐 dying_elder G 键给丹模式）；blueprint book 翻页发包。gradle test + 面板可视验证
- **P2 e2e + 回执**：升级 `scripts/bot/scenarios/production_forge_station_real_place.py` → 全链路（放砧→learn_blueprint→start_session→tempering_hit×N→step_advance→outcome→iron_sword 入包）；放砧回执补 `forge_station` payload 推送（对齐 alchemy open_furnace 模式）。**bot 场景是本 plan 验收门**（AGENTS.md §15）

## §4 开放问题（转 active 前收口）

1. **起炉是否需要先 learn blueprint**：`forge_learn_blueprint` 已接线；起炉校验是否强制已学（推荐强制，图谱书是玩法核心循环）还是 tier0 蓝图免学（新手 iron_sword_v0 白名单）。
2. **TurnPage 的语义落点**：图谱书是纯 client 状态（翻页无需 server）还是 server 权威页码（防作弊翻到未学页）。推荐 server 权威（已有 `forge_blueprint_book` payload 通道）。
3. **station_id 契约**：wire 字段是 `station_id: u64`，但砧实体用 BlockPos 定位（alchemy 用 furnace_pos）——接线时统一成哪种寻址（推荐对齐 alchemy 的 pos 寻址，wire 已定型则 station_id→pos 映射表）。
